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
use crate::refverify::ReferenceVerification;
use crate::swarm::{AgentKind, DebateOutcome, ANSWER_CONCERN};
use crate::validate::StatsValidityReport;
use crate::verify_agent::{Verdict, VerificationReport};
use crate::{Database, GaplyError};

/// Human match-type label for a plagiarism span. Mirrors the frontend
/// `plagiarismToReport` (checks/adapters.ts) EXACTLY so the desktop-app report
/// and the backend PublishReady report read identically. Both sides are pinned
/// by tests (`match_type_label_wording_is_supported_by_the_algorithm` here,
/// `matchTypeLabel wording` in checks/plagiarism.vitest.tsx) — change one and
/// the other's counterpart must change with it.
///
/// WHY THESE WORDS — do not "improve" them back.
///
/// `m.similarity` is cosine over vectors from [`crate::embed::HashEmbedder`]
/// (`embed.rs:33-53`), which is a 384-dim FNV FEATURE-HASHING BAG-OF-WORDS
/// encoder. It is the only `Embedder` implementation in the tree. That means
/// the number measures WORD OVERLAP, and nothing else:
///
///   * it is word-order-blind — two passages with identical word multisets in
///     different order score 1.0, so "verbatim" was never verifiable here;
///   * it has no semantic capability — synonyms and rephrasing are invisible.
///
/// The old label set therefore named the inverse of what the engine detects.
/// "paraphrase" was the clearest case: a genuine paraphrase (synonym
/// substitution) produces LOW word overlap, scores below
/// [`crate::plagiarism::DEFAULT_THRESHOLD`] (0.80), and is never reported at
/// all — so nothing labelled "paraphrase" could have been one. What sat in that
/// band is partial lexical reuse.
///
/// These words state only what the cosine supports. If a real semantic embedder
/// is wired behind the `Embedder` trait, revisit them — with a benchmark, not a
/// rename.
fn match_type_label(m: &MatchSpan) -> &'static str {
    if matches!(m.source, MatchSource::SelfManuscript { .. }) {
        // "(same manuscript)", not "(self-plagiarism)": `MatchSource` establishes
        // WHERE the match is, never that reuse was illegitimate. The old parenthetical
        // asserted a determination that `ISOLATION_NOTE` (plagiarism.rs) explicitly
        // disclaims — and it is the label users see most, since self-matches need no
        // seeded corpus to fire.
        "internal duplication (same manuscript)"
    } else if m.similarity >= 0.98 {
        "near-identical wording"
    } else if m.similarity >= 0.85 {
        "high word overlap"
    } else {
        "partial lexical overlap"
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
///
/// `extraction` unlocks the three EXTRACTION-DERIVED finding families below
/// (document stylometry, tables, reference recency). All three are pure
/// functions of the extraction — no model, no network — and were previously
/// computed-and-discarded (stylometry) or extracted-and-never-surfaced (tables,
/// reference years) on the PublishReady path. `None` skips all three.
///
/// `current_year` is injected rather than read from the clock so this stays
/// pure and deterministically testable (the `now_epoch()` convention); it is
/// only used by the reference-recency count.
pub fn compile_report(
    outcome: &DebateOutcome,
    validation: &StatsValidityReport,
    verification: Option<&VerificationReport>,
    plagiarism: Option<&PlagiarismReport>,
    extraction: Option<&ExtractionResult>,
    current_year: i32,
    checklist: Vec<ChecklistItem>,
    // Per-reference registry evidence from the verification lane. Empty when
    // verification did not run. See `registry_year_findings`.
    registry: &[ReferenceVerification],
) -> PublishReadyReport {
    let mut items: Vec<ReportFinding> = Vec::new();

    // --- deterministic rule flags -------------------------------------------
    //
    // Severity comes from the RULE (`flag.severity`, i.e. `RuleId::severity()`),
    // not from the fact that the flag is deterministic.
    //
    // This previously hardcoded Critical under the heading "hard constraints:
    // every deterministic rule flag is CRITICAL", which conflated two orthogonal
    // axes. "Hard constraint" is an EPISTEMIC claim from the swarm — the Maths
    // engine's verdicts are never voted on — and it already has two correct
    // homes: `Opinion::hard_constraint` and `CertaintyTier::MathematicallyCertain`.
    // `FindingSeverity` is an EDITORIAL URGENCY claim. A missing effect size is
    // certainly missing and only moderately serious; the hardcode said "fatal"
    // because the producer meant "never voted on".
    //
    // The contradiction was visible in the output: the finding rendered
    // `[critical]` while its own provenance line read `rule:MissingEffectSize
    // (MAJOR)`, and `store_validation` wrote Major to the `findings` table for
    // the same flag. Gap E2 (ONTOLOGY §2.1) records this confusion on the
    // CertaintyTier axis; this was the same confusion on the severity axis.
    for flag in &validation.flags {
        items.push(paired(
            Finding {
                severity: match flag.severity {
                    crate::validate::Severity::Critical => FindingSeverity::Critical,
                    crate::validate::Severity::Major => FindingSeverity::Major,
                },
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

    items.extend(citation_count_findings(registry));

    // --- verification verdicts (tier depends on whether a reconsideration ran)
    let verification_tier = if outcome.revised_agents.contains(&AgentKind::Verification) {
        CertaintyTier::ReconsideredAfterPeerReview
    } else {
        CertaintyTier::AiAssessedModerate
    };
    if let Some(vr) = verification {
        // Refuted and Supported fan out PER CITATION: each names a specific
        // citation the author must act on, so the per-item shape carries real
        // per-item information.
        //
        // Unknown does NOT. Its reason is a service-level fact, identical across
        // every citation, so fanning it out produced N identical findings — 28 of
        // 48 on one real manuscript, plus a 29th restating them in aggregate.
        // Collapsed into ONE finding that names the ids, the same disclosure shape
        // `reference_recency_findings` uses for its counts.
        //
        // Collapsing PRESENTATION must not collapse PROVENANCE: where a bundle was
        // assembled for a citation, its `evidence_refs` survive the merge.
        let mut unknown_ids: Vec<String> = Vec::new();
        let mut unknown_refs: Vec<String> = Vec::new();
        for v in &vr.verdicts {
            if v.verdict == Verdict::Unknown {
                unknown_ids.push(v.citation_id.clone());
                for r in &v.evidence_refs {
                    let tag = format!("evidence:{r}");
                    if !unknown_refs.contains(&tag) {
                        unknown_refs.push(tag);
                    }
                }
                continue;
            }
            let (severity, title) = match v.verdict {
                Verdict::Refuted => (
                    FindingSeverity::Major,
                    format!("citation {} REFUTED by evidence", v.citation_id),
                ),
                Verdict::Supported => (
                    FindingSeverity::Info,
                    format!("citation {} supported by evidence", v.citation_id),
                ),
                Verdict::Unknown => unreachable!("Unknown is collected above"),
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
        if !unknown_ids.is_empty() {
            let total = vr.verdicts.len();
            let shown: Vec<String> = unknown_ids.iter().take(UNKNOWN_ID_LIMIT).cloned().collect();
            let extra = unknown_ids.len().saturating_sub(shown.len());
            let ids = if extra > 0 {
                format!("{}, and {extra} more", shown.join(", "))
            } else {
                shown.join(", ")
            };
            // Author-facing: say the check did not RUN and what that means for
            // them. The per-verdict rationale describes OUR infrastructure
            // ("model returned no verdict") and does not belong in their report.
            let detail = format!(
                "{} of {total} citation(s) were not checked against the literature, so nothing \
                 is claimed about them either way: {ids}. This check needs the citation \
                 verification service, which was unavailable for this run — it is not a finding \
                 about your references.",
                unknown_ids.len()
            );
            items.push(paired(
                Finding {
                    severity: FindingSeverity::Minor,
                    tier: verification_tier,
                    certainty_label: verification_tier.label().into(),
                    agent: AgentKind::Verification,
                    title: format!("{} of {total} citation(s) could not be checked", unknown_ids.len()),
                    detail,
                    confidence: 0.0,
                    provenance: {
                        let mut p = vec![
                            "signal:citations_unchecked".into(),
                            format!("evidence:unchecked={};total={total}", unknown_ids.len()),
                            "agent:verification (service unavailable — no verdict attempted)".into(),
                        ];
                        p.extend(unknown_refs.iter().take(UNKNOWN_REF_LIMIT).cloned());
                        p
                    },
                },
                0.0,
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
                    // "word overlap", not "similarity": the number is cosine over
                    // a bag-of-words encoder (see `match_type_label`). Mirrored in
                    // checks/adapters.ts.
                    title: format!("{label} — {:.0}% word overlap", m.similarity * 100.0),
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

    // --- extraction-derived findings (pure; no model, no network) -------------
    // Signals that were already being computed (or already extracted) on this
    // path and then dropped. Each goes through `paired()` like every other
    // finding, so the EvidenceRecord's ConfidenceKind / RoutingHint /
    // limitations are DERIVED from the agent, never hand-set.
    if let Some(ex) = extraction {
        items.extend(stylometry_findings(ex));
        items.extend(table_findings(ex));
        items.extend(reference_recency_findings(ex, current_year));
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
// Extraction-derived findings (pure: no model, no network, no manuscript text)
// ============================================================================
//
// Three signals that PublishReady already had and threw away:
//
//  1. document stylometry — `ai_signals::document_features` is pure and runs in
//     microseconds off the extraction, but was only ever called by AI Check's
//     `analyze_stage1`; the PublishReady pipeline calls `detect_extraction`, so
//     these were computed for AI Check runs and simply never computed here.
//  2. tables — extracted into `ExtractionResult.tables` and never surfaced.
//  3. reference years — parsed into `Reference.year` and never aggregated.
//
// Every value below is a COUNT, RATIO or ENUM. No manuscript prose enters a
// finding's `title` or `detail` (and `detail` is dropped at the proxy boundary
// regardless — see `reviewer_agent::build_review_payload`). Provenance uses the
// existing `signal:` / `evidence:` / `agent:` prefixes from
// `crate::evidence::STRUCTURED_PREFIXES`, so these reach the reviewer through
// the existing filter with no payload change.

/// Coarse confidence for a stylometric finding. Mirrors the constants the
/// AI-detection swarm opinion already uses (`swarm::adapters::from_ai_detection`):
/// there is no calibrated per-signal confidence for stylometry, which is exactly
/// what `ConfidenceKind::DeliberatelyCoarse` records. Never threshold on these.
const STYLO_CONF_HIGH: f64 = 0.6;
const STYLO_CONF_MODERATE: f64 = 0.5;

/// Structural counts carry NO confidence: `AgentKind::Extraction` maps to
/// `ConfidenceKind::NoSignal` + `RoutingHint::HeldOut`, i.e. "placeholder, never
/// used for routing". 0.0 rather than a plausible-looking number, so nothing
/// implies a precision the record's own semantics say does not exist.
const STRUCTURAL_CONF: f64 = 0.0;

/// A reference older than this many years counts as pre-dating the "recent
/// literature" window. Ten years is a conventional review horizon. This is a
/// COUNT for a human to weigh, never a judgement about any single reference —
/// foundational work is legitimately old.
const REFERENCE_RECENCY_YEARS: i32 = 10;

/// Above this share of DATED references falling outside the window, the count is
/// worth a reviewer's attention (Minor); at or below, it is Info context.
const STALE_REFERENCE_MINOR_SHARE: f64 = 0.5;

/// Deviation strength for one stylometric signal. Local to this mapping: AI
/// Check's `SignalLevel` also carries a `Low` rendering row, but a non-deviating
/// signal produces NO finding here (a non-event is not a finding — the same rule
/// the plagiarism block above follows for zero matches).
#[derive(Clone, Copy)]
enum Deviation {
    Notable,
    Moderate,
}

impl Deviation {
    fn confidence(self) -> f64 {
        match self {
            Deviation::Notable => STYLO_CONF_HIGH,
            Deviation::Moderate => STYLO_CONF_MODERATE,
        }
    }
}

/// Build one stylometric finding. Always `Minor` — these are soft, proficiency-
/// correlated signals, never a hard problem with the manuscript.
fn stylo_finding(signal: &str, dev: Deviation, title: String, detail: String, evidence: String) -> ReportFinding {
    let confidence = dev.confidence();
    paired(
        Finding {
            severity: FindingSeverity::Minor,
            tier: CertaintyTier::AiAssessedModerate,
            certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
            agent: AgentKind::AiDetection,
            title,
            detail,
            confidence,
            provenance: vec![
                format!("signal:{signal}"),
                format!("evidence:{evidence}"),
                "agent:ai_detection (document-level stylometry)".into(),
            ],
        },
        confidence,
    )
}

/// Document-level writing/citation-hygiene signals from
/// [`crate::ai_signals::document_features`] — PURE, µs, no model.
///
/// SUBSET, deliberately. The AI-AUTHORSHIP tells in `DocumentFeatures`
/// (`em_dash_per100`, `template_density`, `function_word_ratio`) are EXCLUDED:
/// they answer "was this written by a model", which is AI Check's job, not a
/// reviewer's. What is kept answers "is this well written and consistently
/// cited", which is.
///
/// Thresholds mirror `ai_signals::document_evidence`'s rendering thresholds so
/// the two consumers agree on what "deviating" means; they are stated here
/// rather than reused because that function also emits `Low` rows and rows this
/// subset excludes.
fn stylometry_findings(ex: &ExtractionResult) -> Vec<ReportFinding> {
    let norms = crate::ai_signals::StyloNorms::bundled();
    let f = crate::ai_signals::document_features(ex, &norms);
    let h = &norms.human_academic;
    let mut out = Vec::new();

    // Sentence-length variation: uniform sentence length reads as monotonous.
    if let Some(cv) = f.sentence_length_cv {
        let dev = if cv < h.sentence_length_cv_median * 0.6 {
            Some(Deviation::Notable)
        } else if cv < h.sentence_length_cv_median * 0.85 {
            Some(Deviation::Moderate)
        } else {
            None
        };
        if let Some(dev) = dev {
            out.push(stylo_finding(
                "sentence_length_variation",
                dev,
                "low sentence-length variation".into(),
                format!(
                    "sentence-length CV {cv:.2} against a human-academic reference of \
                     {:.2} — uniform sentence length reads as monotonous to a reader",
                    h.sentence_length_cv_median
                ),
                format!("cv={cv:.2};reference={:.2}", h.sentence_length_cv_median),
            ));
        }
    }

    // Lexical diversity (MTLD): two-sided — unusually low OR high both deviate.
    if let (Some(m), Some(deviation)) = (f.mtld, f.mtld_deviation) {
        if deviation > h.mtld_median * 0.35 {
            out.push(stylo_finding(
                "lexical_diversity",
                Deviation::Moderate,
                "lexical diversity deviates from the academic reference".into(),
                format!(
                    "MTLD {m:.0}, deviating {deviation:.0} from a human-academic reference of {:.0}",
                    h.mtld_median
                ),
                format!("mtld={m:.0};deviation={deviation:.0};reference={:.0}", h.mtld_median),
            ));
        }
    }

    // Repetition: repeated 3-grams.
    if let Some(r) = f.ngram_repetition {
        if r > 0.15 {
            out.push(stylo_finding(
                "ngram_repetition",
                Deviation::Moderate,
                format!("{:.0}% of 3-grams are repeated", r * 100.0),
                format!(
                    "{:.0}% repeated 3-grams across the document — check for redundant phrasing",
                    r * 100.0
                ),
                format!("repetition={r:.3}"),
            ));
        }
    }

    // Citation density, with the honest parse-failure state kept DISTINCT from a
    // genuinely sparsely-cited document (the same distinction `document_evidence`
    // makes: references present + zero attributable in-text citations is a PARSE
    // failure, not evidence about the manuscript).
    if let Some(d) = f.citation_density {
        let refs = f.reference_count.unwrap_or(0);
        if d == 0.0 && refs > 0 {
            // Info, not Minor: this is a statement about OUR parse, not the paper.
            out.push(paired(
                Finding {
                    severity: FindingSeverity::Info,
                    tier: CertaintyTier::AiAssessedModerate,
                    certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
                    agent: AgentKind::AiDetection,
                    title: "citation density could not be measured".into(),
                    detail: format!(
                        "{refs} reference entries parsed but no in-text citations could be \
                         attributed — the in-text citation style was not recognised, so density \
                         is unavailable rather than zero"
                    ),
                    confidence: STYLO_CONF_MODERATE,
                    provenance: vec![
                        "signal:citation_density".into(),
                        format!("evidence:in_text=0;references={refs};status=unavailable"),
                        "agent:ai_detection (document-level stylometry)".into(),
                    ],
                },
                STYLO_CONF_MODERATE,
            ));
        } else if d < 1.0 {
            out.push(stylo_finding(
                "citation_density",
                Deviation::Notable,
                format!("low citation density ({d:.1} per 1000 words)"),
                format!("{d:.1} in-text citations per 1000 words across {refs} reference entries"),
                format!("density={d:.2};references={refs}"),
            ));
        } else if d < 5.0 {
            out.push(stylo_finding(
                "citation_density",
                Deviation::Moderate,
                format!("moderate citation density ({d:.1} per 1000 words)"),
                format!("{d:.1} in-text citations per 1000 words across {refs} reference entries"),
                format!("density={d:.2};references={refs}"),
            ));
        }
    }

    // Citation style consistency — a journal-compliance signal (mixed styles are
    // a common desk-reject trigger). No `document_evidence` row exists for this
    // field, so the threshold is stated here, matching the fraction-shaped
    // convention `doi_syntax_validity` uses below.
    if let Some(c) = f.citation_style_consistency {
        if c < 0.9 {
            out.push(stylo_finding(
                "citation_style_consistency",
                Deviation::Moderate,
                format!("mixed in-text citation styles ({:.0}% dominant)", c * 100.0),
                format!(
                    "{:.0}% of in-text citations follow the dominant style — the remainder mix \
                     styles, which most journals reject on format",
                    c * 100.0
                ),
                format!("consistency={c:.2}"),
            ));
        }
    }

    // DOI syntax validity — malformed DOIs in the reference list.
    if let Some(v) = f.doi_syntax_validity {
        if v < 0.9 {
            out.push(stylo_finding(
                "doi_syntax_validity",
                Deviation::Moderate,
                format!("{:.0}% of DOIs are well-formed", v * 100.0),
                format!(
                    "{:.0}% of DOI-bearing references have syntactically valid DOIs — the rest \
                     will not resolve",
                    v * 100.0
                ),
                format!("valid={v:.2}"),
            ));
        }
    }

    out
}

/// Table presence + caption completeness. A structural COUNT over
/// `ExtractionResult.tables`, which extraction has always produced and nothing
/// has ever surfaced. No table content is read — only the label and whether a
/// caption was detected.
///
/// `AgentKind::Extraction` is the accurate producer, and its
/// `ConfidenceKind::NoSignal` / `RoutingHint::HeldOut` mapping is correct by
/// construction: a count is not a probability, and the record says so.
fn table_findings(ex: &ExtractionResult) -> Vec<ReportFinding> {
    let total = ex.tables.len();
    if total == 0 {
        // A paper with no tables is not a finding.
        return Vec::new();
    }
    let captioned = ex.tables.iter().filter(|t| t.caption.is_some()).count();
    let complete = captioned == total;
    paired(
        Finding {
            severity: if complete { FindingSeverity::Info } else { FindingSeverity::Minor },
            tier: CertaintyTier::AiAssessedModerate,
            certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
            agent: AgentKind::Extraction,
            title: format!("{total} table(s) detected, {captioned} with captions"),
            detail: if complete {
                format!("all {total} detected table(s) have a caption")
            } else {
                format!(
                    "{} of {total} detected table(s) have no caption — most journals require a \
                     caption on every table",
                    total - captioned
                )
            },
            confidence: STRUCTURAL_CONF,
            provenance: vec![
                "signal:tables".into(),
                format!("evidence:tables={total};captioned={captioned}"),
                "agent:extraction (deterministic structural count)".into(),
            ],
        },
        STRUCTURAL_CONF,
    )
    .into_vec()
}

/// Descriptive citation-count summary over references a registry resolved.
///
/// DESCRIPTIVE ONLY — no threshold, no judgement, nothing above `Info`.
///
/// # A finding deliberately NOT built
///
/// **"References with unusually low citation counts" is refused**, and refused on
/// EVIDENCE rather than difficulty — it would be easy to write. It is an absence
/// claim in §4.9's sense: "this work is under-cited" rests on the citation count
/// being *known*, and a reference Semantic Scholar failed to resolve is
/// indistinguishable from one with a genuinely low count. Emitting it would need
/// a coverage argument — a measured Semantic Scholar resolution rate — and **no
/// such measurement exists**.
///
/// Recorded here so it is not rediscovered later as an apparently cheap feature:
/// the blocker is missing evidence, not missing effort.
fn citation_count_findings(registry: &[ReferenceVerification]) -> Vec<ReportFinding> {
    let mut counts: Vec<i64> = registry
        .iter()
        .filter_map(|rv| rv.enrichment.as_ref().and_then(|e| e.citation_count))
        .collect();
    if counts.is_empty() {
        return Vec::new();
    }
    counts.sort_unstable();
    let median = counts[counts.len() / 2];
    let total = registry.len();
    paired(
        Finding {
            severity: FindingSeverity::Info,
            tier: CertaintyTier::AiAssessedModerate,
            certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
            agent: AgentKind::Verification,
            title: format!("citation counts resolved for {} of {total} reference(s)", counts.len()),
            detail: format!(
                "median citation count {median} across the {} reference(s) a public registry \
                 resolved; {} reference(s) were not resolved and are not described here",
                counts.len(),
                total - counts.len()
            ),
            confidence: 1.0,
            provenance: vec![
                "signal:citation_count".into(),
                format!("evidence:resolved={};total={total};median={median}", counts.len()),
                "agent:verification (descriptive count, no threshold applied)".into(),
            ],
        },
        1.0,
    )
    .into_vec()
}

/// Bibliography entries never cited in the body.
///
/// # DELIBERATELY NOT WIRED INTO `compile_report`
///
/// This is not `plagiarism_exact` — that one is unwired by neglect
/// (ARCHITECTURE_MAP B6). This one is unwired because it MUST NOT emit yet, and
/// the reason is recorded here so it is found at the code and not only in git
/// history.
///
/// Measured on a real manuscript: **6 findings, 6 false positives, precision
/// 0.00.** The reference side was guarded correctly — all four parse fragments
/// became `NotEvaluated` — but the citation side had no guard, and two defects in
/// `extract_in_text` mean a cited work can be missing from `ex.citations`:
///   * `narrative_cite` (`extract/stats.rs:84`) matches `and`/`&` without
///     consuming the surname after it, so `"Gordon and Burford (1984)"` extracts
///     with authors `"Burford"`;
///   * `paren_group` (`extract/stats.rs:88`) splits only on `;`, so
///     `"(A 1993, B 1998, C 2002)"` collapses to ONE citation.
///
/// # Wiring precondition
///
/// Fixing those two defects is NECESSARY BUT NOT SUFFICIENT. This finding has the
/// form "X is absent from Y", so its operand IS an absence and can never be
/// positively identified (ONTOLOGY §4.9). What it requires instead is positive
/// evidence that citation extraction is COMPLETE — an established recall figure
/// for `extract_in_text` against a hand-counted ground truth. **No such figure
/// exists.** Until it does, "no matching citation" means "no matching citation,
/// and our own recall is unknown", which is not grounds for telling an author
/// their reference is uncited.
///
/// Governed by ONTOLOGY §4.6: the evidence is deterministic but the MAPPING is
/// not, because a bibliography entry carries no number field and must be joined
/// to the body by author + year. Every ambiguous case is refused —
/// `classify_reference_use` (`extract/citations.rs`) emits `Uncited` only for
/// entries it could evaluate.
///
/// Both numbers are reported. "N of M references were checked" states M - N in
/// the open rather than hiding it, the same disclosure shape
/// `reference_recency_findings` uses for undated entries.
///
/// Tier follows the `364e106` precedent (the citation-recency finding): this is
/// deterministic but NOT a hard constraint, so it must not outrank a failed
/// statistical rule. `CertaintyTier` still has no tier for that (ONTOLOGY Gap
/// E2); `AiAssessedModerate` is the honest choice available, not the accurate
/// one, and one finding does not justify widening the enum.
// Unused outside tests BY DESIGN — the wiring precondition above is not met.
// Delete this attribute at the same time you add the `compile_report` call site.
#[allow(dead_code)]
fn uncited_reference_findings(ex: &ExtractionResult) -> Vec<ReportFinding> {
    use crate::extract::citations::{classify_reference_use, CitationUse};

    let total = ex.references.len();
    if total == 0 {
        // No reference list is already covered by the structural checklist.
        return Vec::new();
    }
    let verdicts = classify_reference_use(&ex.references, &ex.citations);
    if verdicts.is_empty() {
        // Style not determined (mixed, numeric-dominant, or no citations to
        // reason from). Refusing is the designed outcome, not a failure.
        return Vec::new();
    }

    let evaluated = verdicts
        .iter()
        .filter(|v| !matches!(v, CitationUse::NotEvaluated(_)))
        .count();
    let uncited: Vec<usize> = verdicts
        .iter()
        .enumerate()
        .filter(|(_, v)| matches!(v, CitationUse::Uncited))
        .map(|(i, _)| i)
        .collect();

    if uncited.is_empty() {
        // A non-event is not a finding (see the plagiarism fan-out at :288).
        return Vec::new();
    }

    let not_evaluated = total - evaluated;
    let mut examples: Vec<String> = uncited
        .iter()
        .take(UNCITED_EXAMPLE_LIMIT)
        .filter_map(|i| ex.references.get(*i))
        .map(|r| {
            let who = r.authors.split_whitespace().next().unwrap_or("?");
            match r.year {
                Some(y) => format!("{who} {y}"),
                None => who.to_string(),
            }
        })
        .collect();
    if uncited.len() > examples.len() {
        examples.push(format!("and {} more", uncited.len() - examples.len()));
    }

    let detail = format!(
        "{} of {total} reference(s) were checked; {} appear(s) never cited in the text ({}). \
         {not_evaluated} reference(s) could not be checked and are NOT counted as uncited.",
        evaluated,
        uncited.len(),
        examples.join("; ")
    );

    paired(
        Finding {
            severity: FindingSeverity::Minor,
            tier: CertaintyTier::AiAssessedModerate,
            certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
            agent: AgentKind::Extraction,
            title: format!(
                "{} of {evaluated} checked reference(s) appear never cited in the text",
                uncited.len()
            ),
            detail,
            confidence: STRUCTURAL_CONF,
            provenance: vec![
                "signal:uncited_reference".into(),
                format!("evidence:checked={evaluated}/{total} uncited={}", uncited.len()),
                "agent:extraction (deterministic, author-year matching)".into(),
            ],
        },
        STRUCTURAL_CONF,
    )
    .into_vec()
}

/// How many unchecked citation ids to name before summarising the rest.
const UNKNOWN_ID_LIMIT: usize = 8;

/// Cap on evidence refs carried through the collapse — grounding is preserved,
/// but the provenance list must stay readable.
const UNKNOWN_REF_LIMIT: usize = 12;

/// How many uncited entries to name in the detail before summarising the rest.
#[allow(dead_code)] // see `uncited_reference_findings` — deliberately unwired
const UNCITED_EXAMPLE_LIMIT: usize = 5;

/// Reference recency: how much of the bibliography pre-dates the recent-literature
/// window. Deterministic arithmetic over `Reference.year`, which extraction parses
/// from the manuscript's OWN reference list — no network, no connector, so this
/// works offline and is genuinely `AgentKind::Extraction` rather than Verification.
///
/// (`refverify` also resolves a registry-confirmed `matched_year` per reference,
/// which is strictly better data; it lives in the verification lane's per-reference
/// results and is not threaded here. Preferring it is an additive refinement.)
fn reference_recency_findings(ex: &ExtractionResult, current_year: i32) -> Vec<ReportFinding> {
    let total = ex.references.len();
    if total == 0 {
        // No reference list is already covered by the structural checklist.
        return Vec::new();
    }
    let years: Vec<i32> = ex.references.iter().filter_map(|r| r.year).collect();
    let dated = years.len();
    let undated = total - dated;
    let cutoff = current_year - REFERENCE_RECENCY_YEARS;
    let older = years.iter().filter(|y| **y < cutoff).count();
    // Share is over DATED references only — undated ones are reported separately
    // rather than silently counted as either recent or old.
    let share = if dated == 0 { 0.0 } else { older as f64 / dated as f64 };
    let severity = if share > STALE_REFERENCE_MINOR_SHARE {
        FindingSeverity::Minor
    } else {
        FindingSeverity::Info
    };
    let detail = if dated == 0 {
        format!("no publication year could be parsed from any of the {total} reference entries")
    } else {
        format!(
            "{older} of {dated} dated reference(s) pre-date {cutoff} ({:.0}%); \
             {undated} reference(s) had no parseable year",
            share * 100.0
        )
    };
    paired(
        Finding {
            severity,
            tier: CertaintyTier::AiAssessedModerate,
            certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
            agent: AgentKind::Extraction,
            title: format!(
                "{older} of {dated} dated reference(s) are older than {REFERENCE_RECENCY_YEARS} years"
            ),
            detail,
            confidence: STRUCTURAL_CONF,
            provenance: vec![
                "signal:citation_recency".into(),
                format!(
                    "evidence:references={total};dated={dated};undated={undated};\
                     older_than={REFERENCE_RECENCY_YEARS}y;count={older}"
                ),
                "agent:extraction (deterministic, local reference years)".into(),
            ],
        },
        STRUCTURAL_CONF,
    )
    .into_vec()
}

impl ReportFinding {
    /// Single-element vec, so the three builders above share one return type.
    fn into_vec(self) -> Vec<ReportFinding> {
        vec![self]
    }
}

// ============================================================================
// PublishReady checklist (journal guidelines from the RAG corpus)
// ============================================================================

/// Build the checklist by retrieving the target journal's guidelines from the
/// RAG corpus and running DETERMINISTIC checks against the manuscript.
/// Guideline text never reaches an LLM from here.
/// Build the journal-compliance checklist for ONE guideline document.
///
/// `guidelines_url` is the document the user asked for, threaded from the
/// frontend rather than re-derived here. Two reasons, and the second is the
/// urgent one:
///
/// * **Completeness.** This scans EVERY chunk of that document instead of a
///   semantic top-5. `checklist_from_guidelines` is exact matching —
///   `contains("conflict")`, a word-limit regex — so a KNN stage in front of it
///   is a lossy pre-filter that can only discard matches. Measured on PLOS ONE:
///   the strings sit in chunks 1, 6, 7, 8; the top-5 returned 3, 20, 21, 4, 12.
/// * **Correctness.** The corpus is persistent and accumulates across runs, and
///   `Some("journal_guideline")` scopes to a TYPE, not a journal. With two
///   journals ingested, a checklist could state another journal's requirements
///   as the target journal's — an INCORRECT report, not merely an incomplete
///   one.
///
/// `None` — no guidelines requested — yields the structural checks only, which
/// is the honest empty case rather than whatever happens to be in the corpus.
pub fn build_checklist(
    db: &Database,
    extraction: &ExtractionResult,
    manuscript_text: &str,
    guidelines_url: Option<&str>,
) -> Result<Vec<ChecklistItem>, GaplyError> {
    let hits = match guidelines_url {
        Some(url) => crate::rag::chunks_for_source(db, url, Some("journal_guideline"))?,
        None => Vec::new(),
    };
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

        // WORD LIMIT — DISABLED. Not a scoping defect; a pre-existing detector
        // defect that scoping EXPOSED. It was starved of input while retrieval
        // returned the wrong chunks; fed real guideline text it fabricates.
        //
        // Measured on PLOS ONE: it emitted "word limit (300 words) — manuscript
        // has 5144 words (limit 300)". PLOS ONE has no 300-word manuscript
        // limit. 300 is almost certainly the ABSTRACT limit, scraped from an
        // abstract-context chunk and applied to the whole manuscript.
        //
        // "Your 5144-word paper exceeds a 300-word limit" is confident, false
        // and actionable — the §4.4 class, and WORSE than the empty checklist it
        // replaced, because silence misleads no one.
        //
        // Re-enabling needs its own chunk scan: is the limit recoverable in
        // context (which number governs which artifact), or is the requirement
        // simply not extractable by regex? Until that is answered, silence.
        let _ = (&extract_word_limit, word_count);
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


    // DEDUPE BY REQUIREMENT. Scoping the scan to every chunk of the document
    // (instead of a semantic top-5) means a requirement stated in more than one
    // chunk now triggers its detector more than once — PLOS states the numbered
    // reference style in chunks 1 and 7, and the checklist showed it twice.
    //
    // Same shape as the Unknown-verdict collapse: the duplication is
    // PRESENTATION, not analysis. First occurrence wins, so the earliest chunk
    // in `seq` order supplies the detail and the source.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    items.retain(|i| seen.insert(i.requirement.clone()));

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
