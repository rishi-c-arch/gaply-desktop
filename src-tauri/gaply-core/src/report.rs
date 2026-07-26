//! Report Compiler — the sixth agent role: aggregate the swarm debate into a
//! single, priority-ordered [`PublishReadyReport`].
//!
//! Uncertainty is preserved END-TO-END via three distinct certainty tiers that
//! every finding carries:
//!   * [`CertaintyTier::MathematicallyCertain`] — the Maths agent's
//!     deterministic rule verdicts (hard constraints; never probabilistic);
//!   * [`CertaintyTier::AiAssessedModerate`] — soft-consensus / heuristic
//!     findings (AI-detection signals, similarity matches, LLM verdicts) —
//!     NEVER presented as definitive proof;
//!   * [`CertaintyTier::ReconsideredAfterPeerReview`] — verdicts the
//!     Verification agent revised in a peer-informed reconsideration round.
//!
//! Priority ordering: CRITICAL (hard constraints) first, then MAJOR
//! soft-consensus concerns, then MINOR, then informational — and within a
//! severity band, mathematically-certain findings outrank AI-assessed ones,
//! then higher confidence first.
//!
//! The PublishReady checklist cross-references the manuscript against the
//! target journal's guidelines retrieved from the RAG corpus (Prompt 4).
//! Guideline text is UNTRUSTED web content: it is used only for deterministic
//! keyword/number matching here — it is never placed in an LLM prompt by this
//! module — and every checklist item carries the guideline's provenance URL.

use serde::{Deserialize, Serialize};

use crate::evidence::EvidenceRecord;
use crate::extract::sections::SectionKind;
use crate::extract::ExtractionResult;
use crate::plagiarism::{MatchSource, MatchSpan, PlagiarismReport};
use crate::rag::RagHit;
use crate::swarm::{AgentKind, DebateOutcome, ANSWER_CONCERN};
use crate::validate::StatsValidityReport;
use crate::verify_agent::{Verdict, VerificationReport};
use crate::{Database, GaplyError};

/// Human match-type label for a plagiarism span. Mirrors the frontend
/// `plagiarismToReport` (checks/adapters.ts) EXACTLY so the desktop-app report
/// and the backend PublishReady report read identically.
fn match_type_label(m: &MatchSpan) -> &'static str {
    if matches!(m.source, MatchSource::SelfManuscript { .. }) {
        "internal duplication (self-plagiarism)"
    } else if m.similarity >= 0.98 {
        "verbatim"
    } else if m.similarity >= 0.85 {
        "near-verbatim"
    } else {
        "paraphrase"
    }
}

// ============================================================================
// Certainty tiers + severity
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CertaintyTier {
    MathematicallyCertain,
    AiAssessedModerate,
    ReconsideredAfterPeerReview,
}

impl CertaintyTier {
    /// Human-readable label — shown verbatim in the report.
    pub fn label(&self) -> &'static str {
        match self {
            CertaintyTier::MathematicallyCertain => "mathematically certain",
            CertaintyTier::AiAssessedModerate => "AI-assessed, moderate confidence",
            CertaintyTier::ReconsideredAfterPeerReview => "reconsidered after peer review",
        }
    }
    fn rank(&self) -> u8 {
        match self {
            CertaintyTier::MathematicallyCertain => 0,
            CertaintyTier::ReconsideredAfterPeerReview => 1,
            CertaintyTier::AiAssessedModerate => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    /// Hard constraints — deterministic failures. Always first.
    Critical,
    /// Soft-consensus concerns (refuted citations, similarity matches, …).
    Major,
    /// Uncertain / needs-attention items (UNKNOWN verdicts, weak signals).
    Minor,
    /// Informational (clean passes, context notes).
    Info,
}

impl FindingSeverity {
    /// Priority rank (0 = most urgent). `pub` so the Evidence Orchestrator can
    /// order escalation candidates severity-first without duplicating the order.
    pub fn rank(&self) -> u8 {
        match self {
            FindingSeverity::Critical => 0,
            FindingSeverity::Major => 1,
            FindingSeverity::Minor => 2,
            FindingSeverity::Info => 3,
        }
    }
}

// ============================================================================
// Findings + report
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: FindingSeverity,
    pub tier: CertaintyTier,
    /// The tier's human label, embedded so serialized reports are self-describing.
    pub certainty_label: String,
    pub agent: AgentKind,
    pub title: String,
    pub detail: String,
    /// Confidence as used by the debate (rescaled for soft findings; 1.0 for
    /// deterministic verdicts).
    pub confidence: f64,
    /// Non-empty for every finding: rule ids, evidence refs, debate weights,
    /// source URLs — whatever grounds this finding.
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChecklistItem {
    pub requirement: String,
    pub passed: bool,
    pub detail: String,
    /// Provenance of the guideline this item was derived from (RAG source
    /// URL), or None for the always-on structural checks.
    pub guideline_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DebateSummary {
    pub rounds_run: usize,
    pub converged: bool,
    pub overridden_by_constraint: bool,
    pub rejected_agents: Vec<AgentKind>,
    pub revised_agents: Vec<AgentKind>,
}

#[derive(Debug, Serialize)]
pub struct PublishReadyReport {
    /// Overall verdict from the debate (hard constraint wins if present).
    pub verdict: String,
    pub combined_confidence: f64,
    /// Priority-ordered: CRITICAL hard constraints first.
    pub findings: Vec<Finding>,
    /// One EvidenceRecord per finding, SAME order and `f{N}` ids as `findings`
    /// (both derived together from a single paired source — see `ReportFinding`
    /// — so they cannot desync). EvidenceRecord is produced now so downstream
    /// components (Evidence Store, Orchestrator) can adopt it incrementally
    /// without changing report generation again. Nothing consumes it yet.
    pub evidence: Vec<EvidenceRecord>,
    pub checklist: Vec<ChecklistItem>,
    pub debate: DebateSummary,
    /// Mandatory, never empty: explains the three certainty tiers.
    pub disclaimer: String,
}

const DISCLAIMER: &str = "Certainty tiers: 'mathematically certain' findings are \
deterministic rule verdicts and require correction; 'AI-assessed, moderate confidence' \
findings are statistical or model-derived signals — indicators for human review, never \
definitive proof; 'reconsidered after peer review' findings were revised by the \
verification agent after seeing other agents' evidence and remain non-definitive.";

// ============================================================================
// Compiler
// ============================================================================

/// INTERNAL to compile_report: a finding and its EvidenceRecord, constructed
/// together at the SAME site so the two public vectors (`findings`, `evidence`)
/// are derived from one ordered source and can never desync (not merely
/// asserted equal in length). Never exported.
struct ReportFinding {
    finding: Finding,
    evidence: EvidenceRecord,
}

/// Pair a freshly-built Finding with its EvidenceRecord. `raw_confidence` is the
/// agent's REAL pre-rescale signal in scope at the creation site (for soft-loop
/// agents this differs from the Finding's rescaled `confidence`). A gate-rejected
/// finding (provenance `swarm:rejected…`) routes through `from_finding`, which
/// forces NoSignal/HeldOut regardless of agent. Id is assigned post-sort.
fn paired(finding: Finding, raw_confidence: f64) -> ReportFinding {
    let evidence = if finding.provenance.iter().any(|p| p.starts_with("swarm:rejected")) {
        EvidenceRecord::from_finding(&finding, String::new())
    } else {
        EvidenceRecord::at_source(
            String::new(),
            finding.agent,
            finding.severity,
            raw_confidence,
            finding.provenance.clone(),
        )
    };
    ReportFinding { finding, evidence }
}

/// Aggregate the debate outcome + underlying agent reports into the final,
/// priority-ordered report.
pub fn compile_report(
    outcome: &DebateOutcome,
    validation: &StatsValidityReport,
    verification: Option<&VerificationReport>,
    plagiarism: Option<&PlagiarismReport>,
    checklist: Vec<ChecklistItem>,
) -> PublishReadyReport {
    let mut items: Vec<ReportFinding> = Vec::new();

    // --- hard constraints: every deterministic rule flag is CRITICAL ---------
    for flag in &validation.flags {
        items.push(paired(
            Finding {
                severity: FindingSeverity::Critical,
                tier: CertaintyTier::MathematicallyCertain,
                certainty_label: CertaintyTier::MathematicallyCertain.label().into(),
                agent: AgentKind::ValidationMaths,
                title: format!("statistical rule failed: {}", flag.rule.label()),
                detail: flag.explanation.clone(),
                confidence: 1.0,
                provenance: vec![
                    format!("rule:{:?} ({})", flag.rule, flag.rule.severity().as_str()),
                    format!("location:{:?} paragraph {}", flag.location.section, flag.location.paragraph),
                    "agent:validation_maths (deterministic)".into(),
                ],
            },
            1.0, // deterministic — raw == Finding.confidence
        ));
    }

    // --- verification verdicts (tier depends on whether a reconsideration ran)
    let verification_tier = if outcome.revised_agents.contains(&AgentKind::Verification) {
        CertaintyTier::ReconsideredAfterPeerReview
    } else {
        CertaintyTier::AiAssessedModerate
    };
    if let Some(vr) = verification {
        for v in &vr.verdicts {
            let (severity, title) = match v.verdict {
                Verdict::Refuted => (
                    FindingSeverity::Major,
                    format!("citation {} REFUTED by evidence", v.citation_id),
                ),
                Verdict::Unknown => (
                    FindingSeverity::Minor,
                    format!("citation {} could not be verified (UNKNOWN)", v.citation_id),
                ),
                Verdict::Supported => (
                    FindingSeverity::Info,
                    format!("citation {} supported by evidence", v.citation_id),
                ),
            };
            let mut provenance: Vec<String> =
                v.evidence_refs.iter().map(|r| format!("evidence:{r}")).collect();
            provenance.push("agent:verification (harness-gated, via proxy)".into());
            for gf in &v.gate_flags {
                provenance.push(format!("gate:{gf}"));
            }
            items.push(paired(
                Finding {
                    severity,
                    tier: verification_tier,
                    certainty_label: verification_tier.label().into(),
                    agent: AgentKind::Verification,
                    title,
                    detail: v.rationale.clone(),
                    confidence: v.confidence,
                    provenance,
                },
                v.confidence, // native per-verdict confidence — already raw
            ));
        }
    }

    // --- plagiarism: one finding PER MATCH (not the aggregate opinion) ----------
    // The swarm still votes with a single plagiarism opinion (consensus); here we
    // fan the individual spans into per-match findings carrying the RAW cosine
    // similarity as confidence (the actual evidence signal — deliberately NOT the
    // swarm-rescaled opinion weight). Empty matches → zero findings (a non-event
    // is not a finding). Shape mirrors the frontend `plagiarismToReport`.
    if let Some(pr) = plagiarism {
        for m in pr.corpus_matches.iter().chain(&pr.self_matches) {
            let label = match_type_label(m);
            let (source_kind, source_label) = match &m.source {
                MatchSource::Corpus { title, .. } => ("corpus", format!("corpus: {title}")),
                MatchSource::SelfManuscript { other_chunk_seq, .. } => {
                    ("self_manuscript", format!("self · chunk {other_chunk_seq}"))
                }
            };
            items.push(paired(
                Finding {
                    severity: if m.similarity >= pr.threshold {
                        FindingSeverity::Major
                    } else {
                        FindingSeverity::Minor
                    },
                    tier: CertaintyTier::AiAssessedModerate,
                    certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
                    agent: AgentKind::Plagiarism,
                    title: format!("{label} — {:.0}% similarity", m.similarity * 100.0),
                    detail: format!("\u{201c}{}\u{201d} matches {source_label}.", m.manuscript_excerpt),
                    confidence: m.similarity,
                    provenance: vec![
                        format!("similarity:{:.3}", m.similarity),
                        format!("match_type:{label}"),
                        format!("source:{source_kind}"),
                    ],
                },
                m.similarity, // raw cosine — already the evidence signal
            ));
        }
    }

    // --- soft round-table opinions (Verification + Plagiarism are covered in
    //     detail above, so their aggregate opinions are skipped here) -----------
    for op in &outcome.opinions {
        if op.hard_constraint
            || op.agent == AgentKind::Verification
            || op.agent == AgentKind::Plagiarism
        {
            continue;
        }
        let weight = outcome
            .result
            .weights
            .iter()
            .find(|(k, _)| *k == op.agent)
            .map(|(_, w)| *w)
            .unwrap_or_else(|| crate::swarm::rescale_confidence(op.agent, op.confidence));
        let severity = if op.answer == ANSWER_CONCERN {
            FindingSeverity::Major
        } else {
            FindingSeverity::Info
        };
        let tier = if outcome.revised_agents.contains(&op.agent) {
            CertaintyTier::ReconsideredAfterPeerReview
        } else {
            CertaintyTier::AiAssessedModerate
        };
        items.push(paired(
            Finding {
                severity,
                tier,
                certainty_label: tier.label().into(),
                agent: op.agent,
                title: format!("{:?}: {}", op.agent, op.answer),
                detail: op.explanation.clone(),
                confidence: weight,
                provenance: vec![format!(
                    "swarm:round-table ({} round(s), rescaled weight {:.3})",
                    outcome.rounds_run, weight
                )],
            },
            // RAW pre-rescale signal for the EvidenceRecord — NOT the rescaled
            // `weight` the Finding shows (Flag B: soft-loop agents differ here).
            op.confidence,
        ));
    }

    // --- gate-rejected opinions surface as MINOR findings (flagged, not lost)
    for op in &outcome.rejected {
        items.push(paired(
            Finding {
                severity: FindingSeverity::Minor,
                tier: CertaintyTier::AiAssessedModerate,
                certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
                agent: op.agent,
                title: format!("{:?} output rejected by its internal gate", op.agent),
                detail: op.explanation.clone(),
                confidence: 0.0,
                provenance: vec!["swarm:rejected-before-debate (internal gate failed)".into()],
            },
            // Rejected: `paired` detects the provenance and forces NoSignal/HeldOut
            // via from_finding; raw_confidence is unused for that path.
            0.0,
        ));
    }

    // --- priority ordering ----------------------------------------------------
    // CRITICAL hard constraints first — severity outranks EVERYTHING, including
    // any soft finding's confidence. Within a band: certainty tier, then
    // confidence (desc), then agent for determinism. Sorted ONCE on the paired
    // items (by `.finding`), so findings + evidence stay in lockstep.
    items.sort_by(|a, b| {
        a.finding
            .severity
            .rank()
            .cmp(&b.finding.severity.rank())
            .then(a.finding.tier.rank().cmp(&b.finding.tier.rank()))
            .then(
                b.finding
                    .confidence
                    .partial_cmp(&a.finding.confidence)
                    .expect("finite confidence"),
            )
            .then(format!("{:?}", a.finding.agent).cmp(&format!("{:?}", b.finding.agent)))
    });

    // Assign the `f{N}` ids ONCE, from the sorted paired vec (matches the id
    // scheme build_review_payload uses when enumerating `findings`).
    for (i, rf) in items.iter_mut().enumerate() {
        rf.evidence.id = format!("f{}", i + 1);
    }

    // Derive BOTH public vectors from the single ordered source, atomically —
    // they cannot desync because they come from the same paired items in order.
    let (findings, evidence): (Vec<Finding>, Vec<EvidenceRecord>) =
        items.into_iter().map(|rf| (rf.finding, rf.evidence)).unzip();

    PublishReadyReport {
        verdict: outcome.result.answer.clone(),
        combined_confidence: outcome.result.combined_confidence,
        findings,
        evidence,
        checklist,
        debate: DebateSummary {
            rounds_run: outcome.rounds_run,
            converged: outcome.converged,
            overridden_by_constraint: outcome.result.overridden_by_constraint,
            rejected_agents: outcome.rejected.iter().map(|o| o.agent).collect(),
            revised_agents: outcome.revised_agents.clone(),
        },
        disclaimer: DISCLAIMER.into(),
    }
}

// ============================================================================
// PublishReady checklist (journal guidelines from the RAG corpus)
// ============================================================================

/// Build the checklist by retrieving the target journal's guidelines from the
/// RAG corpus and running DETERMINISTIC checks against the manuscript.
/// Guideline text never reaches an LLM from here.
pub fn build_checklist(
    db: &Database,
    embedder: &dyn crate::embed::Embedder,
    extraction: &ExtractionResult,
    manuscript_text: &str,
    journal_query: &str,
) -> Result<Vec<ChecklistItem>, GaplyError> {
    let hits = crate::rag::search(db, embedder, journal_query, 5, Some("journal_guideline"))?;
    Ok(checklist_from_guidelines(extraction, manuscript_text, &hits))
}

/// Deterministic checklist core (separated for direct testing).
pub fn checklist_from_guidelines(
    extraction: &ExtractionResult,
    manuscript_text: &str,
    guidelines: &[RagHit],
) -> Vec<ChecklistItem> {
    let mut items: Vec<ChecklistItem> = Vec::new();

    // --- always-on structural checks (no guideline needed) -------------------
    for (kind, name) in [
        (SectionKind::Abstract, "Abstract"),
        (SectionKind::Methods, "Methods"),
        (SectionKind::Results, "Results"),
        (SectionKind::References, "References"),
    ] {
        let present = extraction.sections.iter().any(|s| s.kind == kind);
        items.push(ChecklistItem {
            requirement: format!("required section: {name}"),
            passed: present,
            detail: if present {
                format!("{name} section found")
            } else {
                format!("{name} section missing")
            },
            guideline_source: None,
        });
    }

    // No guideline docs in the store → "not provided yet", NOT a failure. Return
    // only the always-on structural checks above and emit NO item — the old
    // passed:false "journal guidelines available: FAILED" item dishonestly
    // rendered "the user didn't provide guidelines" as a red ✗ manuscript
    // failure. Absence of guideline-derived items (every item has
    // guideline_source == None) is the neutral signal the UI reads to show an
    // honest "add your journal's guidelines to enable these checks" note.
    if guidelines.is_empty() {
        return items;
    }

    // --- guideline-derived checks (deterministic keyword/number matching) ----
    let word_count = manuscript_text.split_whitespace().count();
    let text_lower = manuscript_text.to_lowercase();
    for hit in guidelines {
        let g = hit.content.to_lowercase();
        let src = Some(hit.source_url.clone());

        // word limit: "... limit of 3000 words" / "3000 words"
        if let Some(limit) = extract_word_limit(&g) {
            items.push(ChecklistItem {
                requirement: format!("word limit ({limit} words)"),
                passed: word_count <= limit,
                detail: format!("manuscript has {word_count} words (limit {limit})"),
                guideline_source: src.clone(),
            });
        }
        // structured abstract
        if g.contains("structured abstract") {
            let has_abstract =
                extraction.sections.iter().any(|s| s.kind == SectionKind::Abstract);
            items.push(ChecklistItem {
                requirement: "structured abstract".into(),
                passed: has_abstract,
                detail: if has_abstract {
                    "abstract present (structure itself needs editorial review)".into()
                } else {
                    "no abstract section found".into()
                },
                guideline_source: src.clone(),
            });
        }
        // conflict-of-interest declaration
        if g.contains("conflict") {
            let declared = text_lower.contains("conflict of interest");
            items.push(ChecklistItem {
                requirement: "conflict-of-interest declaration".into(),
                passed: declared,
                detail: if declared {
                    "conflict-of-interest statement found".into()
                } else {
                    "no conflict-of-interest statement found".into()
                },
                guideline_source: src.clone(),
            });
        }
        // numbered (Vancouver) reference style
        if g.contains("vancouver") || g.contains("numbered") {
            let total = extraction.references.len();
            let numbered = extraction
                .references
                .iter()
                .filter(|r| {
                    let t = r.raw.trim_start();
                    t.starts_with(|c: char| c.is_ascii_digit())
                        || t.starts_with('[')
                })
                .count();
            let passed = total > 0 && numbered == total;
            items.push(ChecklistItem {
                requirement: "numbered (Vancouver) reference style".into(),
                passed,
                detail: format!("{numbered}/{total} reference entries are numbered"),
                guideline_source: src.clone(),
            });
        }
    }

    items
}

/// Parse a word limit out of guideline text, e.g. "a limit of 3000 words" or
/// "maximum 3,000 words". Deterministic; first match wins.
fn extract_word_limit(guideline_lower: &str) -> Option<usize> {
    let bytes = guideline_lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut j = i;
            let mut digits = String::new();
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b',') {
                if bytes[j] != b',' {
                    digits.push(bytes[j] as char);
                }
                j += 1;
            }
            let rest = guideline_lower[j..].trim_start();
            if rest.starts_with("words") || rest.starts_with("word limit") {
                if let Ok(n) = digits.parse::<usize>() {
                    // plausibility guard (10..=100k): section-level limits can be
                    // small (e.g. a 50-word highlights blurb); 0/absurd rejected.
                    if (10..=100_000).contains(&n) {
                        return Some(n);
                    }
                }
            }
            i = j.max(start + 1);
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests;
