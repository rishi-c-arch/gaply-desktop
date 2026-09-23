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

use crate::evidence::{ClaimKind, EvidenceRecord};
use crate::extract::sections::SectionKind;
use crate::extract::{ExtractionResult, Location};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertaintyTier {
    MathematicallyCertain,
    AiAssessedModerate,
    ReconsideredAfterPeerReview,
}

impl CertaintyTier {
    /// Human-readable label — shown verbatim in the report.
    /// Delegates to [`crate::vocabulary::tier_label`] — the wording is unchanged
    /// and now lives with every other label, so a copy edit happens in one place.
    pub fn label(&self) -> &'static str {
        crate::vocabulary::tier_label(*self)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: FindingSeverity,
    pub tier: CertaintyTier,
    /// The tier's human label, embedded so serialized reports are self-describing.
    pub certainty_label: String,
    pub agent: AgentKind,
    /// WHAT editorial statement this finding makes. REQUIRED — every literal
    /// must supply it, so a new constructor cannot silently default to
    /// `ManuscriptDefect` (§26.5). Do NOT derive `Default` on this type.
    pub claim: ClaimKind,
    pub title: String,
    pub detail: String,
    /// Confidence as used by the debate (rescaled for soft findings; 1.0 for
    /// deterministic verdicts).
    pub confidence: f64,
    /// Non-empty for every finding: rule ids, evidence refs, debate weights,
    /// source URLs — whatever grounds this finding.
    pub provenance: Vec<String>,
    /// WHERE in the manuscript this finding is, when the finding is about one
    /// place. Resolved to a quotation by `report_build::into_report_model`
    /// through [`crate::extract::paragraph_at`], and shown as
    /// `LocalFinding::nearby_text`.
    ///
    /// # `Option`, and the alternative was measured
    ///
    /// A REQUIRED field breaks every cached report: the `CACHED_REPORT_V2`
    /// release artifact has no `location`, and `evidence::tests::
    /// previously_written_cached_reports_still_deserialize` fails with
    /// `missing field \`location\`` — run both ways before choosing this shape.
    /// `Option` deserializes to `None` on a missing key, so the pin passes and
    /// `CACHED_REPORT_SCHEMA_VERSION` must NOT be bumped (`pipeline.rs`'s rule:
    /// bumping for a compatible change trains reflexive bumping).
    ///
    /// # `None` IS A DECISION AT EVERY SITE
    ///
    /// Most findings are about the whole document — a stylometric ratio, a
    /// reference-year count, an agent's aggregate opinion — and for those `None`
    /// is CORRECT, not missing. A few families have an address the type does not
    /// carry yet; those are marked GAP at their site. The two are distinguished
    /// in the comment at every construction, and
    /// `only_located_families_carry_a_location` pins the wired set so a new
    /// family cannot default into silence.
    pub location: Option<Location>,
    /// **The OTHER places this same finding was raised.** `location` holds the
    /// first; together they are every place, which is what a grouped row must
    /// not lose — the locations ARE the evidence a reader checks.
    ///
    /// `serde(default)` + `skip_serializing_if` for the reason
    /// `ChecklistItem::source_span` has them: a required field breaks every
    /// stored report, and skipping the empty case keeps an UNGROUPED finding
    /// byte-identical to what it serialized before §11 D180 — so the golden
    /// moves only where grouping actually happened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_at: Vec<Location>,
}

/// **How a finding raised in several places is titled. ONE formatter, two
/// callers.** `review_lens::review` has produced this sentence since it was
/// written; `compile_report` now groups too, and the phrasing must not drift
/// between the two surfaces a reader can see. It was an inline `format!` inside
/// `review`'s per-criterion loop, bound to types `report.rs` cannot reach, so it
/// is extracted here rather than synthesised a second time. §11 D180.
///
/// **`quoted` is not decoration.** `review_lens` holds the spans and can say
/// *"all are quoted"*; `compile_report` holds LOCATIONS, which become quotations
/// one layer later in `report_build` and do not all resolve — 41 of 301 fail, by
/// the count in `report::evaluate`'s own comment. Claiming a quote this layer
/// cannot produce is the truncated-span defect from the other side, so the
/// caller states which claim it can back.
pub fn raised_at_phrase(summary: &str, places: usize, quoted: bool) -> String {
    if places <= 1 {
        return summary.to_string();
    }
    if quoted {
        format!("{summary} (raised at {places} places; all are quoted)")
    } else {
        format!("{summary} (raised at {places} places)")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistSource {
    pub guideline_source: String,
    pub source_span: String,
    /// The article type THIS source bound the requirement to. Per-source, not
    /// per-item: `matters-arising` states a competing-interests requirement for
    /// one article type while `editorial-policies/competing-interests` states it
    /// for all, and collapsing them would lose exactly the distinction a reader
    /// needs.
    pub article_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub requirement: String,
    pub passed: bool,
    pub detail: String,
    /// Provenance of the guideline this item was derived from (RAG source
    /// URL), or None for the always-on structural checks.
    pub guideline_source: Option<String>,
    /// **The journal's own sentence.** Prompt 5 item 12: a checklist item shows
    /// which requirement it came from. A rule without its sentence has to be
    /// trusted; one with it can be checked in a glance (CLAUDE.md's span rule).
    ///
    /// `Option` + `serde(default)` because `CACHED_REPORT_V2` predates it — the
    /// same shape `Finding::location` uses, and for the same reason: a required
    /// field breaks every stored report.
    #[serde(default)]
    pub source_span: Option<String>,
    /// The article type the requirement was bound to, where the journal said.
    /// `None` means NOT STATED, never "applies to everything".
    #[serde(default)]
    pub article_type: Option<String>,
    /// Which extraction / research-state field this item read. Named so an
    /// item with nothing to read is visibly unevaluable rather than silently
    /// passing.
    #[serde(default)]
    pub checked_field: Option<String>,
    /// **`passed` is a bool and compliance has three states.**
    ///
    /// `passed: false` means "we looked and it is missing". An item nobody
    /// could decide is neither passed nor failed, and rendering it as `false`
    /// tells a researcher their manuscript failed a check that was never run.
    /// Measured on a real Nature Medicine run: CONSORT 6a reads only the
    /// declined scientific layer (§11 D165) and printed as a flag against a
    /// manuscript that had done nothing wrong.
    ///
    /// `#[serde(default)]` because stored reports predate it — the same shape
    /// `source_span` uses, for the same reason.
    ///
    /// **`skip_serializing_if` is load-bearing, not tidiness.** The golden
    /// report is pinned byte-for-byte
    /// (`the_report_is_byte_identical_to_the_pre_research_state_capture`) and a
    /// field emitted unconditionally changes every stored report's bytes for a
    /// flag that is false almost everywhere. Written only when TRUE, the wire
    /// format stays additive: a report with no undecidable item is unchanged,
    /// and one that has them says so.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unevaluable: bool,
    /// **Every OTHER page on which the journal states this requirement. §11 D188.**
    ///
    /// `guideline_source`/`source_span` hold the first; this holds the rest —
    /// the same first-plus-the-others shape `Finding::location` and
    /// `Finding::also_at` use, for the same reason.
    ///
    /// # Why the code stopped choosing
    ///
    /// `requirements_for` is `ORDER BY id DESC` and this loop used to take the
    /// FIRST match per statement, so the row a researcher read was whichever
    /// the crawl stored LAST. On Nature Medicine that surfaced
    /// *"all fast track submissions must include…"* for data availability while
    /// *"a Data Availability Statement must be included with all original
    /// research manuscripts"* sat unread in the same table. 13 of 25 statement
    /// groups across the nine seeded journals are contested this way.
    ///
    /// **Three selection rules were measured and all three fail** (D188):
    /// preferring the row with no condition picks the WORSE row twice;
    /// preferring a topical URL picks a post-publication comments page;
    /// obligation language separates 2 of 13 because both spans are genuine
    /// obligations. What separates them is SCOPE BREADTH — "all fast track
    /// submissions" versus "all original research manuscripts" — and that is
    /// not a property of the span.
    ///
    /// So every source is carried and none is privileged. A reader can tell
    /// which scope covers their manuscript; the code demonstrably cannot, and
    /// that asymmetry is the whole argument for showing both.
    ///
    /// `skip_serializing_if` for the reason `unevaluable` documents above: the
    /// golden report is pinned byte-for-byte, and a structural-only checklist
    /// has no sources, so its bytes are unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_from: Vec<ChecklistSource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DebateSummary {
    pub rounds_run: usize,
    pub converged: bool,
    pub overridden_by_constraint: bool,
    pub rejected_agents: Vec<AgentKind>,
    pub revised_agents: Vec<AgentKind>,
}

/// **A statement about Gaply's own execution — never about the manuscript.**
///
/// # Why these left the findings list
///
/// `"Verification output rejected by its internal gate"` was a `Minor` FINDING,
/// and it appeared in **20 of 22** stored reports. A researcher opening their
/// report saw it in the same list, in the same shape, with the same severity
/// vocabulary as *"a primary statistical claim reports a p-value but no
/// confidence interval"* — one is a defect in their paper, the other is a note
/// about our harness, and nothing on the page distinguished them.
///
/// §11's `ClaimKind::ProcessState` already stopped these changing the
/// recommendation (`reviewer_agent.rs:1096`), which was the half that altered a
/// verdict. This is the other half: they stop being findings at all.
///
/// **They are not deleted.** An author is entitled to know a lane did not
/// produce usable output — "we could not check this" is true and worth showing.
/// It belongs in a record of the RUN, which is what this is: durable (the report
/// is cached), readable by the chat when asked, and outside the list of things
/// wrong with the paper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessNote {
    /// Which subsystem the note is about.
    pub agent: AgentKind,
    /// What happened, in the agent's own words.
    pub detail: String,
    /// Why it is recorded — the machine-readable reason, so a consumer can
    /// group without parsing prose.
    pub reason: HarnessNoteReason,
}

/// Why a harness note exists. Deliberately small and total; a new variant is an
/// additive change, and the `#[serde(other)]`-free totality means a consumer
/// cannot silently mis-bucket one.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessNoteReason {
    /// The agent produced output and our own gate refused it before the debate.
    OutputRejectedByInternalGate,
}

#[derive(Debug, Serialize, Deserialize)]
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
    /// **Statements about GAPLY'S OWN RUN, kept out of `findings`.**
    ///
    /// See [`HarnessNote`]. Defaulted on deserialize so every report cached
    /// before this field existed still parses (the §11 D53/D103 serde-drift
    /// discipline: a new field is additive only if old rows keep loading).
    #[serde(default)]
    pub harness_notes: Vec<HarnessNote>,
    /// Mandatory, never empty: explains the certainty tiers this report uses.
    pub disclaimer: String,
}

/// The two tiers every report can carry.
///
/// **§11 D220: the first clause described a label no finding carries, and a
/// claim none supports.** It read *"'mathematically certain' findings are
/// deterministic rule verdicts and require correction"*. Since D214/D217/D219
/// no finding in this report is LABELLED "mathematically certain" — the only
/// producer of that tier is the validation lane, whose labels come from
/// `vocabulary::rule_certainty_label` and say what a check found — and D216
/// measured one rule wrong on 4 of 5 real firings, so "require correction" was
/// false. The clause now says what those findings state. It does not name the
/// tier: the disclaimer explains the labels a reader sees (see `disclaimer_for`).
const DISCLAIMER_BASE: &str = "Certainty tiers: findings from deterministic checks state \
what an automated check did or did not find in the text, not that the manuscript is wrong; \
'AI-assessed, moderate confidence' findings are statistical or model-derived signals — \
indicators for human review, never definitive proof";

/// The third clause — appended ONLY when an agent actually revised.
const DISCLAIMER_REVISED: &str = "; 'reconsidered after peer review' findings were revised \
by the verification agent after seeing other agents' evidence and remain non-definitive";

/// **The disclaimer explains the tiers THIS report has, not the tiers the type
/// can express.**
///
/// # Why this stopped being a constant
///
/// It named all three tiers unconditionally. Nothing in production can revise —
/// every participant is a `PrecomputedAgent` and [`crate::swarm::SwarmAgent::revise`]
/// returns `None` — so `revised_agents` is empty on every run, measured empty in
/// **22 of 22** stored reports. A researcher reading the third clause would
/// reasonably conclude a peer-review step ran and found nothing to revise.
///
/// **That is worse than an absent explanation.** A missing sentence leaves a
/// reader uninformed; this one described a mechanism that exists in the code and
/// did not execute, printed beside findings that did — and it was the sentence
/// most likely to be quoted as evidence the review was thorough.
///
/// The clause is not deleted. When [`crate::swarm::RevisingVerificationAgent`]
/// is wired into the pipeline (§5.1 of the premium architecture) revisions will
/// happen, and the clause returns on exactly the runs that have them. Both
/// directions are pinned:
/// `the_disclaimer_omits_the_revision_tier_when_nothing_was_revised` and
/// `the_disclaimer_restores_the_revision_tier_when_an_agent_revises`.
fn disclaimer_for(revised: bool) -> String {
    let mut s = String::from(DISCLAIMER_BASE);
    if revised {
        s.push_str(DISCLAIMER_REVISED);
    }
    s.push('.');
    s
}

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
/// What a LANE-LEVEL round-table opinion asserts. TOTAL — no wildcard arm — so a
/// seventh `AgentKind` cannot compile until its opinion's claim is decided here.
///
/// The split is §22.5's, traced from what each opinion answers:
/// AI-detection's opinion answers *"was this written by a model"*; Extraction's
/// and RAG's answer *"did our lane work"*. Validation is `hard_constraint` and
/// Verification/Plagiarism are covered in detail elsewhere, so their aggregate
/// opinions never reach a `Finding` — but they are classified here anyway,
/// because totality is the point and an unreachable arm costs nothing.
fn opinion_claim(agent: AgentKind) -> ClaimKind {
    match agent {
        AgentKind::AiDetection => ClaimKind::AuthorshipSignal,
        AgentKind::Extraction => ClaimKind::ProcessState,
        AgentKind::Rag => ClaimKind::ProcessState,
        AgentKind::ValidationMaths => ClaimKind::ManuscriptDefect,
        AgentKind::Verification => ClaimKind::ProcessState,
        AgentKind::Plagiarism => ClaimKind::ManuscriptDefect,
    }
}

/// **An authorship signal may never be louder than `info` (§11 D153).**
///
/// # The withdrawal, and why it is not a threshold change
///
/// `ai_detect.rs`'s thresholds carry their own verdict in a comment:
/// *"Interim, deliberately conservative thresholds for the heuristic proxy. NOT
/// calibrated against real GPT-2 output."* `classify()` takes only
/// `(mean_ppl, burstiness)`, so **every tier is scored against those same
/// unvalidated constants** — the 7B changes which model computes perplexity, not
/// where the line sits. Measured before this cap: `major` on **19 of 22** stored
/// reports, including a real human-written paper.
///
/// So this is not "the heuristic is too loud". There is no tier on which the
/// lane has a measured accuracy, and a finding that says *major* is telling a
/// researcher a reviewer would require it changed.
///
/// # Keyed on the CLAIM, not the agent — deliberately
///
/// AI-detection produces BOTH the authorship opinion and the document
/// stylometry findings (lexical diversity, citation density), and the latter
/// carry [`ClaimKind::ManuscriptDefect`] at `Minor` and are documented as
/// reviewer-relevant. `reviewer_agent::claim_is_eligible` already settled this
/// exact question for editorial admissibility — *"deliberately keyed on the
/// claim rather than on the producer"* — and capping by `AgentKind` here would
/// silently demote those too, applying an argument about perplexity thresholds
/// to computations that do not use them.
///
/// # The route back
///
/// A labelled set. Not a better prompt, not a bigger model, not a tuned
/// constant: until there are texts of known provenance scored by this lane,
/// there is no number to put beside the finding and nothing to raise it on.
fn authorship_capped(claim: ClaimKind, proposed: FindingSeverity) -> FindingSeverity {
    match claim {
        ClaimKind::AuthorshipSignal => FindingSeverity::Info,
        ClaimKind::ProcessState | ClaimKind::ManuscriptDefect => proposed,
    }
}

/// THE single funnel from `Finding` to its `EvidenceRecord` — and the one place
/// the user-facing labels are attached, so no constructor can forget them.
fn paired(finding: Finding, raw_confidence: f64) -> ReportFinding {
    let evidence = if finding.provenance.iter().any(|p| p.starts_with("swarm:rejected")) {
        EvidenceRecord::from_finding(&finding, String::new())
    } else {
        EvidenceRecord::at_source(
            String::new(),
            finding.agent,
            finding.claim,
            finding.severity,
            raw_confidence,
            finding.provenance.clone(),
        )
    };
    ReportFinding { finding, evidence }
}

/// How much of a reference to show when naming it in a finding title.
const CITATION_LABEL_CHARS: usize = 80;

/// A name the AUTHOR can resolve, for a citation id they cannot.
///
/// # `c3` names nothing
///
/// `citation_id` is `c{i+1}` over `verify_citations`' `items` slice — an
/// INTERNAL INDEX into a list the author never sees, and one that
/// `pipeline.rs` builds by SKIPPING references whose refverify call errored, so
/// it is not even the position in their bibliography.
///
/// `registry` is `items.into_iter().map(|(_, rv)| rv)` — the SAME order — so
/// `registry[i]` is the verification of `c{i+1}`, and `reference_raw` is the
/// entry as the manuscript wrote it. That mapping is already in scope here; it
/// was simply never used.
///
/// **A miss is typed, not guessed:** when the id does not parse or the registry
/// is shorter than it claims, the finding says "a reference" rather than
/// inventing one or falling back to the index (§4.12).
fn citation_label(citation_id: &str, registry: &[ReferenceVerification]) -> String {
    // A TITLE must name a subject, so it falls back; a LIST must not repeat
    // "a reference" N times, so it uses the Option form below and omits.
    citation_label_opt(citation_id, registry).unwrap_or_else(|| "a reference".to_string())
}

/// The resolvable form. `None` when the id does not map to a reference.
fn citation_label_opt(citation_id: &str, registry: &[ReferenceVerification]) -> Option<String> {
    let raw = citation_id
        .strip_prefix('c')
        .and_then(|n| n.parse::<usize>().ok())
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| registry.get(i))
        .map(|rv| rv.reference_raw.trim());
    match raw {
        Some(r) if !r.is_empty() => {
            if r.chars().count() <= CITATION_LABEL_CHARS {
                format!("\u{201c}{r}\u{201d}")
            } else {
                let cut: String = r.chars().take(CITATION_LABEL_CHARS).collect();
                // Snap to a word boundary so a truncated reference does not end
                // mid-token — ONTOLOGY §4.20's TEXT class, same rule the report
                // quotation follows.
                let cut = match cut.rfind(char::is_whitespace) {
                    Some(i) => &cut[..i],
                    None => cut.as_str(),
                };
                format!("\u{201c}{}\u{2026}\u{201d}", cut.trim_end())
            }
        }
        _ => return None,
    }
    .into()
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
    // The manuscript's lines, for the Tier-0 equation engine (§6b). Empty when
    // the caller has no text — every existing caller passes `&[]` and the
    // report is byte-identical for them, which is what the golden capture pins.
    manuscript_lines: &[String],
) -> PublishReadyReport {
    let equations = manuscript_lines;
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
                also_at: Vec::new(),
                // a deterministic statistical rule failed in the manuscript
                //
                // THE ONE LOCATED FAMILY. `flag.location` is the location the
                // rule itself evaluated (`validate::paragraph`), so quoting it
                // shows the reader the exact text that produced this finding.
                location: Some(flag.location.clone()),
                claim: ClaimKind::ManuscriptDefect,
                severity: match flag.severity {
                    crate::validate::Severity::Critical => FindingSeverity::Critical,
                    crate::validate::Severity::Major => FindingSeverity::Major,
                },
                tier: CertaintyTier::MathematicallyCertain,
                // **Per RULE, not per tier — D214, D217.** The TIER stays
                // (ordering, colour, the consensus override); the CLAIM does
                // not: each rule's detection is deterministic, and none of them
                // establishes a certain defect (D216 measured it firing by
                // firing). One source, `vocabulary::rule_certainty_label`;
                // `adapters.ts` reads the same value through the mirror.
                certainty_label: crate::vocabulary::rule_certainty_label(flag.rule).into(),
                agent: AgentKind::ValidationMaths,
                title: format!("statistical rule failed: {}", flag.rule.label()),
                detail: flag.explanation.clone(),
                confidence: 1.0,
                provenance: vec![
                    format!("rule:{:?} ({})", flag.rule, flag.rule.severity().as_str()),
                    crate::report_model::provenance_location(&flag.location),
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
                    format!(
                        "citation refuted by the literature: {}",
                        citation_label(&v.citation_id, registry)
                    ),
                ),
                Verdict::Supported => (
                    FindingSeverity::Info,
                    format!(
                        "citation supported by the literature: {}",
                        citation_label(&v.citation_id, registry)
                    ),
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
                    also_at: Vec::new(),
                    // a citation is refuted or supported by external evidence
                    //
                    // GAP, not a document-level finding. This IS about one
                    // place, and no join to it is safe: `citation_id` is
                    // `c{i+1}` over the `items` slice, which `pipeline.rs`
                    // builds by SKIPPING references whose refverify call
                    // errored. `items[i]` is therefore not `references[i]`, and
                    // `Reference` carries no `Location` to join to anyway.
                    location: None,
                    claim: ClaimKind::ManuscriptDefect,
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
            // Same defect, same call site: this listed c1, c2, c3 verbatim.
            //
            // Unresolvable ids are OMITTED rather than rendered as a repeated
            // placeholder — "a reference, a reference, and 20 more" names
            // nothing and reads as a bug. When none resolve the sentence simply
            // carries the count, which is the honest content.
            let resolved: Vec<String> = unknown_ids
                .iter()
                .filter_map(|id| citation_label_opt(id, registry))
                .take(UNKNOWN_ID_LIMIT)
                .collect();
            let extra = unknown_ids.len().saturating_sub(resolved.len());
            let ids = if resolved.is_empty() {
                String::new()
            } else if extra > 0 {
                format!(": {}, and {extra} more", resolved.join(", "))
            } else {
                format!(": {}", resolved.join(", "))
            };
            // Author-facing: say the check did not RUN and what that means for
            // them. The per-verdict rationale describes OUR infrastructure
            // ("model returned no verdict") and does not belong in their report.
            let detail = format!(
                "{} of {total} citation(s) were not checked against the literature, so nothing \
                 is claimed about them either way{ids}. This check needs the citation \
                 verification service, which was unavailable for this run — it is not a finding \
                 about your references.",
                unknown_ids.len()
            );
            items.push(paired(
                Finding {
                    also_at: Vec::new(),
                    // OUR verification lane could not check them (§23.4 f4)
                    //
                    // CORRECT None: one finding covering N citations. Any single
                    // location would name one of them and misrepresent the rest.
                    location: None,
                    claim: ClaimKind::ProcessState,
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
                    also_at: Vec::new(),
                    // this text overlaps another document
                    //
                    // CORRECT None, on two independent grounds. (1) The span's
                    // address is `manuscript_chunk_seq`, an index into 512-token
                    // chunks stepping 448 over the WHOLE text — one chunk spans
                    // many paragraphs and can cross a section boundary, so the
                    // mapping is one-to-many, and §12.3.3 measured that a
                    // preprocessing change shifts every boundary. (2) This
                    // finding ALREADY quotes the manuscript, in `detail` below.
                    location: None,
                    claim: ClaimKind::ManuscriptDefect,
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

    // --- Tier-0 equation findings (§6b) ---------------------------------------
    //
    // Deterministic: exact rational arithmetic over equations the manuscript
    // wrote, with no model, no network and no I/O on the path — enforced by
    // `tests/equation_is_llm_free.rs`, not by this comment. The mapping onto
    // `Finding` lives in `crate::equation_report` so the whole translation is
    // in one auditable place; the pairing stays here so these cannot become a
    // second hand-set construction site for an `EvidenceRecord`.
    for finding in equation_findings(equations, extraction) {
        items.push(paired(finding, 1.0));
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
        let claim = opinion_claim(op.agent);
        let severity = if op.answer == ANSWER_CONCERN {
            authorship_capped(claim, FindingSeverity::Major)
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
                also_at: Vec::new(),
                // depends on WHICH lane opined — total match below
                //
                // CORRECT None: an `Opinion` is an agent's aggregate stance on
                // the whole manuscript by construction (`swarm::Opinion`).
                location: None,
                claim,
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

    // --- gate-rejected opinions become HARNESS NOTES, not findings ----------
    //
    // They used to be pushed here as `Minor` findings and reached 20 of 22
    // stored reports. See [`HarnessNote`] for why that was wrong and why they
    // are still recorded.
    let harness_notes: Vec<HarnessNote> = outcome
        .rejected
        .iter()
        .map(|op| HarnessNote {
            agent: op.agent,
            detail: op.explanation.clone(),
            reason: HarnessNoteReason::OutputRejectedByInternalGate,
        })
        .collect();

    // --- grouping: rows a reader cannot tell apart are ONE row (§11 D180) -----
    //
    // Measured on `Revised Health Economics Paper FINAL (1).docx` through the
    // product path: 21 findings, of which the first FOURTEEN were three
    // sentences repeated — 8 x "missing effect size", 3 x "p-value
    // overclaiming", 3 x "missing confidence interval" — each row carrying the
    // identical title AND the identical detail paragraph. The one finding unique
    // to that manuscript (its own weighted-provision equation) ranked 15th,
    // below all fourteen, because severity sorts before specificity and nothing
    // grouped. `ReportViewerPage` renders `findings.map` with no dedupe, so
    // fourteen visually identical buttons is what a researcher opens.
    //
    // **The key is (agent, title, detail) — EXACTLY what the row displays.**
    // Grouping anything a reader could tell apart would hide a real difference;
    // grouping less would leave duplicates on screen. Findings whose title
    // carries their own subject — an equation, a reference count — differ in
    // title and are untouched.
    //
    // Grouped BEFORE the sort and before `f{N}` id assignment, so findings and
    // evidence stay the lockstep pair the unzip below depends on.
    {
        let mut merged: Vec<_> = Vec::with_capacity(items.len());
        for it in items {
            let hit = merged.iter_mut().find(|p: &&mut ReportFinding| {
                p.finding.agent == it.finding.agent
                    && p.finding.title == it.finding.title
                    && p.finding.detail == it.finding.detail
            });
            match hit {
                Some(prev) => {
                    // EVERY location survives: `location` holds the first, and
                    // `also_at` the rest. Losing them would trade fourteen
                    // checkable addresses for one, which is the truncated-span
                    // defect wearing a tidier list.
                    if let Some(loc) = it.finding.location.clone() {
                        prev.finding.also_at.push(loc);
                    }
                    // Provenance is per-occurrence ("location:Results paragraph
                    // 0"), so the merged rows' trail is carried too rather than
                    // dropped with their evidence record.
                    for p in it.finding.provenance.into_iter().chain(it.evidence.provenance) {
                        if !prev.finding.provenance.contains(&p) {
                            prev.finding.provenance.push(p.clone());
                        }
                        if !prev.evidence.provenance.contains(&p) {
                            prev.evidence.provenance.push(p);
                        }
                    }
                }
                None => merged.push(it),
            }
        }
        for m in &mut merged {
            // `quoted: false` — this layer holds LOCATIONS. They become
            // quotations in `report_build`, and not all of them resolve.
            m.finding.title =
                raised_at_phrase(&m.finding.title, m.finding.also_at.len() + 1, false);
        }
        items = merged;
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
        harness_notes,
        debate: DebateSummary {
            rounds_run: outcome.rounds_run,
            converged: outcome.converged,
            overridden_by_constraint: outcome.result.overridden_by_constraint,
            rejected_agents: outcome.rejected.iter().map(|o| o.agent).collect(),
            revised_agents: outcome.revised_agents.clone(),
        },
        // Derived from the SAME `outcome.revised_agents` the `DebateSummary`
        // above reports, read at one site, so the prose and the structured
        // field cannot disagree about whether anything was revised.
        disclaimer: disclaimer_for(!outcome.revised_agents.is_empty()),
    }
}

/// Run the Tier-0 equation engine over the manuscript and return whatever it
/// found worth reporting.
///
/// **Empty lines in, empty findings out, and that is load-bearing.** Every
/// caller that predates this passes `&[]`, so their reports are byte-identical
/// — which is what makes the golden capture a check on this change rather than
/// a fixture to regenerate.
/// `ex` is `None` when the caller ran no extraction. Findings are still
/// produced — the engine reads the equation lines, not the sections — but they
/// carry no anchor, which is the state §12.1 recorded for every equation
/// finding before this.
fn equation_findings(lines: &[String], ex: Option<&ExtractionResult>) -> Vec<Finding> {
    if lines.is_empty() {
        return Vec::new();
    }
    let graph = crate::equation::graph::graph_from_lines(lines);
    let values = graph.bound_values();
    let env = graph.unit_env();
    let mut out = Vec::new();
    for node in &graph.nodes {
        for f in crate::equation::check::check_equation(&node.equation, &values) {
            let at = ex.and_then(|e| crate::extract::locate_line(e, &f.source_line));
            if let Some(finding) = crate::equation_report::arithmetic_finding(&f, at) {
                out.push(finding);
            }
        }
        for (l, r) in node.equation.claims() {
            let v = crate::equation::units::check_sides(&l.expr, &r.expr, &env);
            let at = ex.and_then(|e| crate::extract::locate_line(e, &node.equation.text));
            if let Some(finding) =
                crate::equation_report::dimension_finding(&node.equation.text, &v, at)
            {
                out.push(finding);
            }
        }
    }
    out
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
            also_at: Vec::new(),
            // writing quality — report.rs's own scoping excludes the authorship tells
            //
            // CORRECT None, for all SEVEN branches that call this constructor.
            // Every one reports a document-level aggregate — a sentence-length
            // CV, an MTLD, a repeated-3-gram rate, a citations-per-1000-words
            // density, a dominant-style share, a valid-DOI share. None of them
            // is computed at a place, so none of them has one.
            location: None,
            claim: ClaimKind::ManuscriptDefect,
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
                    also_at: Vec::new(),
                    // OUR parse could not measure citation density
                    //
                    // CORRECT None: a statement about our parse of the whole
                    // document.
                    location: None,
                    claim: ClaimKind::ProcessState,
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

/// **WITHDRAWN — this finding is no longer shown. §11 D167.**
///
/// It reported *"{total} table(s) detected, {captioned} with captions"* as a
/// structural count over `ExtractionResult.tables`. **The count is wrong, and
/// wrong in a way a user cannot see.** `extract::detect_table` fires on any
/// paragraph opening `Table N`, so a thesis list-of-tables is a run of matches:
/// measured over 20 real manuscripts, **178 of 414 detections (43%) were
/// front-matter rows with no body**. A researcher whose paper has six tables
/// was being told it has fourteen.
///
/// **The caption half is corrupted by the same defect and was the more
/// misleading of the two.** A contents-page row carries the rest of its line as
/// a "caption", so those entries count as captioned; `complete` can therefore
/// read TRUE on a document whose real tables have no captions at all, and the
/// finding's severity — `Info` when complete, `Minor` when not — is decided by
/// front matter.
///
/// **Withdrawn rather than annotated.** A source comment saying the number is
/// untrustworthy does not reach the person reading the report, and a wrong
/// number displayed with confidence is worse than a number absent: it is the
/// §11 D163 shape, plausible and displayed. Honest silence beats a confident
/// count.
///
/// **What restores it:** the two `detect_table` defects fixed, with a
/// before/after measurement over this corpus showing the count matches a hand
/// count. The body below is kept — unreachable but intact — so restoring is
/// deleting the early return, not rewriting the finding from the doc comment.
fn table_findings(ex: &ExtractionResult) -> Vec<ReportFinding> {
    // THE WITHDRAWAL. Everything below is dead until the extractor is fixed.
    if true {
        return Vec::new();
    }

    #[allow(unreachable_code)]
    let total = ex.table_mentions.len();
    if total == 0 {
        // A paper with no tables is not a finding.
        return Vec::new();
    }
    let captioned = ex.table_mentions.iter().filter(|t| t.caption.is_some()).count();
    let complete = captioned == total;
    paired(
        Finding {
            also_at: Vec::new(),
            // tables and their captions are the manuscript's
            //
            // CORRECT None for THIS finding, though the address exists.
            // `TableRef.location` is real, but this is one aggregate count over
            // every table; a single location would name one of them. Per-table
            // findings would be a different feature, and would carry one.
            location: None,
            claim: ClaimKind::ManuscriptDefect,
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
            also_at: Vec::new(),
            // how many references WE resolved counts for
            //
            // CORRECT None: a count over the registry, not a place in the text.
            location: None,
            claim: ClaimKind::ProcessState,
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
            also_at: Vec::new(),
            // references the manuscript never cites
            //
            // GAP (and moot while this is unwired). Each uncited entry has an
            // address — `parse_reference_list` maps paragraph i of the
            // References section to `references[i]` — but `Reference` carries no
            // `Location`, and this finding is an aggregate over several entries
            // regardless.
            location: None,
            claim: ClaimKind::ManuscriptDefect,
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
            also_at: Vec::new(),
            // the manuscript's references are old
            //
            // CORRECT None: an arithmetic summary over the whole bibliography.
            // (Per-reference findings would hit the same missing-`Location`-on-
            // `Reference` gap the uncited-reference builder records.)
            location: None,
            claim: ClaimKind::ManuscriptDefect,
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
    journal_key: Option<&str>,
) -> Result<Vec<ChecklistItem>, GaplyError> {
    let hits = match guidelines_url {
        Some(url) => crate::rag::chunks_for_source(db, url, Some("journal_guideline"))?,
        None => Vec::new(),
    };
    let mut items = checklist_from_guidelines(extraction, manuscript_text, &hits);

    // **The journal's OWN extracted requirements supersede the keyword guesses.
    // §11 D183.**
    //
    // `checklist_from_guidelines` can contribute at most three rows, each from a
    // hardcoded keyword over a RAG chunk (`g.contains("conflict")` matched a
    // sample-reference TITLE — §11 D181). `journal_requirements` holds what the
    // crawl actually extracted: 39 rows for Nature Medicine, where the keyword
    // path contributes ZERO.
    //
    // The keyword rows are DROPPED rather than merged, because both paths emit a
    // competing-interests row and a reader would meet the same requirement
    // twice, once weakly. Rows with `guideline_source: None` are the always-on
    // structural checks and are kept — that field is also the neutral signal the
    // UI reads for "no journal guidance yet", so it must keep meaning that.
    //
    // **`bindings: &[]` and `design_independent` together**, per D177: the
    // design gate is declined, so no row may depend on knowing the study's
    // design. Measured on Nature Medicine, the unfiltered call yields 46 rows of
    // which 25 are hedged and 14 undecided.
    if let Some(key) = journal_key {
        let requirements = crate::journal_store::requirements_for(db, key)?;
        if !requirements.is_empty() {
            let words = manuscript_text.split_whitespace().count();
            let from_requirements = design_independent(checklist_from_requirements(
                extraction,
                manuscript_text,
                words,
                &requirements,
                &[],
            ));
            items.retain(|i| i.guideline_source.is_none());
            items.extend(from_requirements);
        }
    }
    Ok(items)
}

/// **The journal scoped this requirement to an article type, and nothing here
/// knows the manuscript's. THE FIRST PRODUCER OF `unevaluable` (§11 D188).**
///
/// `Some(caveat)` means the row cannot be decided. The two `None` arms are the
/// negative controls and are the reason this is a function rather than a
/// constant:
///
/// * `required_for == None` — the journal stated the requirement for every
///   article type, so there is nothing to scope and the row decides normally.
/// * `manuscript_type == Some(_)` — somebody established the type, so the row
///   decides normally. **Nothing does today**, which is why the only call site
///   passes `None`; when something does, it is passed here and this arm starts
///   carrying traffic. Note what this arm deliberately does NOT do: it does not
///   compare the two types and conclude NOT_APPLICABLE. Deciding that a
///   requirement does not apply needs evidence this function does not have
///   (`docs/A3_APPLICABILITY_MEASUREMENT.md`, part 8: the subject lexicon
///   missed "Participant consent obtained prior to interview", so absence of
///   evidence is not evidence of absence).
fn undecidable_for_article_type(
    required_for: Option<&str>,
    manuscript_type: Option<&str>,
) -> Option<String> {
    match (required_for, manuscript_type) {
        (Some(t), None) => Some(format!(
            "the journal states this for {t} articles; whether it applies depends on the \
             article type you are submitting, which this analysis does not know"
        )),
        _ => None,
    }
}

/// Apply [`undecidable_for_article_type`] to a freshly built row.
///
/// `passed` is set to `false` alongside `unevaluable`, matching the three other
/// producers in this file. That is the safe direction for a consumer that has
/// not been taught the third state: it reads "not met", which is what the row
/// said before this change — never a pass.
fn scoped(mut item: ChecklistItem, manuscript_type: Option<&str>) -> ChecklistItem {
    if item.unevaluable {
        return item;
    }
    if let Some(caveat) = undecidable_for_article_type(item.article_type.as_deref(), manuscript_type)
    {
        item.detail = format!("{} — {caveat}.", item.detail);
        item.passed = false;
        item.unevaluable = true;
    }
    item
}

/// One stored requirement as a carried source. Kept beside the two grouping
/// sites so both produce the same shape.
fn source_of(r: &crate::journal_store::StoredRequirement) -> ChecklistSource {
    ChecklistSource {
        guideline_source: r.source_url.clone(),
        source_span: r.source_span.clone(),
        article_type: r.article_type.clone(),
    }
}

/// Phrasings that satisfy a data availability statement.
const DATA_AVAILABILITY_NAMES: &[&str] =
    &["data availability", "data availability statement", "availability of data"];

/// **Synonyms, because a journal's name for a statement is not the author's.**
///
/// Measured on one manuscript against Nature Medicine: the journal requires a
/// *competing interests* statement and the manuscript writes *"Conflicts of
/// Interest: The authors declare no conflicts of interest."* Checking only the
/// journal's word reports a missing statement that is on the page. The failure
/// direction of a missing synonym is a false FLAG, which is the direction that
/// costs a researcher work, so the lists err towards admitting.
fn synonyms_for(needle: &'static str) -> Vec<&'static str> {
    let list: &[&str] = match needle {
        "competing interest" => {
            &["competing interest", "conflict of interest", "conflicts of interest",
              "declaration of interest", "disclosure statement"]
        }
        "data availability" => DATA_AVAILABILITY_NAMES,
        "code availability" => &["code availability", "code is available", "software availability"],
        "funding" => &["funding", "financial support", "grant support", "supported by a grant"],
        "ethics" => &["ethics", "ethical approval", "ethics approval", "institutional review board",
                      "ethics committee", "ethical clearance"],
        "informed consent" => &["informed consent", "consent was obtained", "consent to participate"],
        "author contribution" => &["author contribution", "authors' contribution", "credit author"],
        // A statement with no recorded synonym is checked under the journal's
        // own word, which is the honest default: inventing synonyms would admit
        // statements the journal did not ask for.
        _ => return vec![needle],
    };
    list.to_vec()
}

/// The sentence a statement occurs in, whole. `None` when none of `names`
/// occurs anywhere in the manuscript.
///
/// **Returns the sentence rather than a bool**, because a checklist item saying
/// *"found"* has to be checkable: the manuscript's own words are what let a
/// reader refute it in one glance.
fn statement_in_text(lower: &str, original: &str, names: &[&str]) -> Option<String> {
    // **`.min()` found the EARLIEST mention, and in a thesis that is the table of
    // contents.** §11 D182. Measured over 20 manuscripts, the old rule was right
    // about funding 1 time in 6 and about ethics 1 in 4, and the wrong matches
    // had three shapes: TOC lines ("7  Funding and Financial Constraints    45"),
    // a reference title ("The ethics of ChatGPT: Exploring the ethical issues"),
    // and discussion prose ("Both regulatory and financial support are
    // required…"). The bias was structural: the earliest occurrence of any topic
    // word in a thesis IS its contents entry.
    //
    // So this scans EVERY occurrence for one that looks like a DECLARATION,
    // rather than taking the first mention of the topic. Two conditions,
    // measured against the corpus (7 of 7 real statements kept, false matches
    // 8 -> 2):
    //
    //   * the name begins its sentence (within 40 chars) — real declarations
    //     read "Conflicts of Interest: …", "Data Availability: …", "Funding
    //     Source of Research …"; the latest genuine one sits at char 34.
    //   * the sentence is not TOC-shaped — it does not end in a page number.
    //
    // The two that survive are a reference title (excluded separately by the
    // caller, which searches non-References text) and one prose sentence whose
    // only tell is semantic — "are required" versus "was obtained" — which is
    // not attempted here.
    for (at, _) in names.iter().flat_map(|n| lower.match_indices(n)) {
        let start = original[..at].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
        let rest = &original[at..];
        let end = rest.find(['.', '\n']).map(|i| at + i + 1).unwrap_or(original.len());
        let sentence = original[start..end].trim();
        if at - start >= 40 {
            continue;
        }
        let trimmed = sentence.trim_end();
        let toc_shaped = trimmed
            .split_whitespace()
            .last()
            .is_some_and(|w| w.len() <= 4 && w.chars().all(|c| c.is_ascii_digit()));
        if toc_shaped {
            continue;
        }
        return Some(sentence.to_string());
    }
    None
}

/// The manuscript minus its reference list, for statement searching.
///
/// **A statement name inside a reference TITLE is never the author's
/// declaration.** §11 D182: *"The ethics of ChatGPT: Exploring the ethical
/// issues of an emerging technology."* satisfied an ethics-statement
/// requirement. Searching the body only removes that class outright, rather
/// than trying to tell a citation from a declaration by shape.
fn text_without_references(extraction: &ExtractionResult, fallback: &str) -> String {
    let body: Vec<&str> = extraction
        .sections
        .iter()
        .filter(|s| s.kind != SectionKind::References)
        .flat_map(|s| s.paragraphs.iter().map(String::as_str))
        .collect();
    if body.is_empty() {
        return fallback.to_string();
    }
    body.join("\n")
}

/// **A checklist built from a journal's OWN extracted requirements.**
///
/// Prompt 5 item 7: point `build_checklist` at `journal_requirements` and the
/// standard bindings. Item 12: each item shows which requirement it came from,
/// with the source sentence, and which field it checked.
///
/// # THE WORD LIMIT IS RE-ENABLED, AND WHY THAT IS SAFE NOW
///
/// `checklist_from_guidelines` disabled its word-limit detector, and the
/// comment there records exactly why: fed real PLOS ONE guideline text it
/// emitted *"word limit (300 words) — manuscript has 5144 words (limit 300)"*.
/// PLOS ONE has no 300-word manuscript limit; 300 is the ABSTRACT limit,
/// scraped from an abstract-context chunk and applied to the whole manuscript.
///
/// That detector read RAG chunks — a window of text with no structure — so it
/// could not tell which artefact a number governed. This one reads
/// `journal_requirements`, where the distinction is a column:
/// `journal_extract` classifies a limit whose sentence mentions the abstract as
/// `abstract_limit`, and `a_journal_with_no_length_limit_yields_no_word_limit`
/// pins that against PLOS ONE's own sentence. The fabrication is prevented at
/// the source, not filtered here.
///
/// **Where the journal states limits for several article types, no item is
/// emitted.** Nature Medicine binds 4,000 words to Article and 2,000 to Brief
/// Communication; nothing in the pipeline knows which the manuscript is, and
/// picking one would be the choice the CONFLICTED rule refuses one layer up.
/// The item says both and passes no verdict.
/// # A REQUIRED STATEMENT IS DECIDED FROM THE FULL TEXT, NOT FROM A HEADING
///
/// This function used to decide *"data availability statement"* and every other
/// required statement by scanning `extraction.sections[heading]`. §4.5 [v8]
/// classes `sections` as a MEDIATED field: a heading classifier that fails to
/// recognise a heading is indistinguishable from a manuscript that has none.
///
/// **Measured against Nature Medicine and `Revised Health Economics Paper
/// FINAL (1).docx`, that produced FOUR false compliance failures on one real
/// manuscript**, because the paper writes its statements as inline run-in
/// labels rather than as headings:
///
/// ```text
/// Data Availability: Available from the corresponding author on reasonable request.
/// Ethical Approval: Ministry of Health, Oman (Grant MOH/CSR/24/29387). …
/// Conflicts of Interest: The authors declare no conflicts of interest.
/// Funding: Ministry of Health, Oman (Grant MOH/CSR/24/29387).
/// ```
///
/// The classifier produced eight sections and not one heading contains any of
/// those phrases. A researcher would have been told to add four statements they
/// had already written, against a journal that really does require them.
///
/// So `manuscript_text` is searched, and the item carries **the manuscript's own
/// sentence** as the evidence — which a heading check could never supply. The
/// heading remains as corroboration in `checked_field`, never as the decider.
pub fn checklist_from_requirements(
    extraction: &ExtractionResult,
    manuscript_text: &str,
    manuscript_words: usize,
    requirements: &[crate::journal_store::StoredRequirement],
    bindings: &[crate::journal_standards::StandardBinding],
) -> Vec<ChecklistItem> {
    use crate::journal_extract::RequirementKind;
    let mut items = Vec::new();
    // **Nothing in the pipeline classifies the manuscript's article type.** Not
    // the extractor, not a user declaration, not the journal profile — measured
    // in `docs/A3_APPLICABILITY_MEASUREMENT.md` part 6. It is a local rather
    // than a literal at the four call sites so that the day something does
    // establish it, there is exactly one line to change.
    let manuscript_article_type: Option<&str> = None;

    // --- word limit ------------------------------------------------------
    let word_limits: Vec<&crate::journal_store::StoredRequirement> =
        requirements.iter().filter(|r| r.kind == RequirementKind::WordLimit).collect();
    match word_limits.len() {
        0 => {}
        1 => {
            let r = word_limits[0];
            if let Ok(limit) = r.value.parse::<usize>() {
                let passed = manuscript_words <= limit;
                items.push(scoped(ChecklistItem {
                    // One source: this row is not derived from journal_requirements.
                    also_from: Vec::new(),
                    requirement: format!("word limit: {limit}"),
                    passed,
                    detail: format!(
                        "manuscript has {manuscript_words} words against a stated limit of {limit}"
                    ),
                    guideline_source: Some(r.source_url.clone()),
                    source_span: Some(r.source_span.clone()),
                    article_type: r.article_type.clone(),
                    checked_field: Some("manuscript word count".into()),
                    unevaluable: false,
                }, manuscript_article_type));
            }
        }
        _ => {
            // Several limits, one per article type. Report them; judge nothing.
            let stated = word_limits
                .iter()
                .map(|r| {
                    format!("{} ({})", r.value, r.article_type.as_deref().unwrap_or("type not stated"))
                })
                .collect::<Vec<_>>()
                .join("; ");
            items.push(ChecklistItem {
                // Same shape as the statement groups (§11 D188): the detail
                // already names every limit, so carrying one span and dropping
                // the others left the row unable to show where each came from.
                also_from: word_limits[1..].iter().map(|r| source_of(r)).collect(),
                requirement: "word limit depends on article type".into(),
                passed: true,
                detail: format!(
                    "the journal states {} word limits — {stated}. Your manuscript has \
                     {manuscript_words} words; which limit applies depends on the article type \
                     you are submitting, which this analysis does not know.",
                    word_limits.len()
                ),
                guideline_source: Some(word_limits[0].source_url.clone()),
                source_span: Some(word_limits[0].source_span.clone()),
                article_type: None,
                checked_field: Some("manuscript word count".into()),
                unevaluable: false,
            });
        }
    }

    // --- abstract limit --------------------------------------------------
    if let Some(r) = requirements.iter().find(|r| r.kind == RequirementKind::AbstractLimit) {
        if let Ok(limit) = r.value.parse::<usize>() {
            let abstract_words = extraction
                .sections
                .iter()
                .filter(|s| s.kind == SectionKind::Abstract)
                .flat_map(|s| s.paragraphs.iter())
                .map(|p| p.split_whitespace().count())
                .sum::<usize>();
            // An abstract the extractor did not find is not an abstract over
            // the limit. Absence is UNEVALUABLE, not a failure.
            if abstract_words > 0 {
                items.push(scoped(ChecklistItem {
                    // One source: this row is not derived from journal_requirements.
                    also_from: Vec::new(),
                    requirement: format!("abstract limit: {limit} words"),
                    passed: abstract_words <= limit,
                    detail: format!("abstract has {abstract_words} words against a limit of {limit}"),
                    guideline_source: Some(r.source_url.clone()),
                    source_span: Some(r.source_span.clone()),
                    article_type: r.article_type.clone(),
                    checked_field: Some("extraction.sections[Abstract]".into()),
                    unevaluable: false,
                }, manuscript_article_type));
            }
        }
    }

    // --- data availability ------------------------------------------------
    //
    // **Searched over the BODY, not the whole manuscript. §11 D182.** A
    // statement name inside a reference title is not the author's declaration.
    let body_text = text_without_references(extraction, manuscript_text);
    let manuscript_text = body_text.as_str();
    let lower_text = manuscript_text.to_lowercase();
    // **EVERY data-policy source, not the newest one. §11 D188.** `.find()` took
    // the first row `ORDER BY id DESC` returned, i.e. the last the crawl stored:
    // on Nature Medicine that is the fast-track page, and on PLOS ONE it is one
    // of six. The requirement is the same either way; which SCOPE it was stated
    // under is what differs, and the code cannot tell which scope covers this
    // manuscript.
    let data_rows: Vec<&crate::journal_store::StoredRequirement> =
        requirements.iter().filter(|r| r.kind == RequirementKind::DataPolicy).collect();
    if let Some((first, rest)) = data_rows.split_first() {
        let found = statement_in_text(&lower_text, manuscript_text, DATA_AVAILABILITY_NAMES);
        items.push(scoped(ChecklistItem {
            requirement: "data availability statement".into(),
            passed: found.is_some(),
            detail: match &found {
                Some(sentence) => format!("found in the manuscript: {sentence}"),
                None => format!(
                    "none of {} phrasings was found anywhere in the manuscript",
                    DATA_AVAILABILITY_NAMES.len()
                ),
            },
            guideline_source: Some(first.source_url.clone()),
            source_span: Some(first.source_span.clone()),
            article_type: first.article_type.clone(),
            checked_field: Some("manuscript full text".into()),
            unevaluable: false,
            also_from: rest.iter().map(|r| source_of(r)).collect(),
        }, manuscript_article_type));
    }

    // --- other required statements (§11 D163) -----------------------------
    //
    // The same check as data availability, for the statements the generalised
    // rule now finds. `journal_extract::REQUIRED_STATEMENTS` carries the
    // heading substring that satisfies each; the value is the statement name
    // as the journal stated it.
    // **GROUPED, not deduplicated. §11 D188.** This used to `continue` past every
    // row after the first for a statement, which threw away up to five of six
    // sources and kept whichever the crawl happened to store last. Grouping
    // keeps the row count identical — one row per statement, as before — and
    // stops the discarding.
    //
    // Insertion order is preserved so the output is deterministic; it carries no
    // authority, and nothing downstream may read `guideline_source` as "the"
    // source. It is the first of several, exactly as `Finding::location` is.
    let mut order: Vec<&'static str> = Vec::new();
    let mut grouped: std::collections::BTreeMap<
        &'static str,
        Vec<&crate::journal_store::StoredRequirement>,
    > = Default::default();
    for r in requirements.iter().filter(|r| r.kind == RequirementKind::SectionRequired) {
        let Some(needle) = crate::journal_extract::heading_for_statement(&r.value) else {
            continue;
        };
        if !grouped.contains_key(needle) {
            order.push(needle);
        }
        grouped.entry(needle).or_default().push(r);
    }
    for needle in order {
        let rows = &grouped[needle];
        let (first, rest) = rows.split_first().expect("a group exists only when a row made it");
        let names = synonyms_for(needle);
        let found = statement_in_text(&lower_text, manuscript_text, &names);
        items.push(scoped(ChecklistItem {
            requirement: first.value.clone(),
            passed: found.is_some(),
            detail: match &found {
                Some(sentence) => format!("found in the manuscript: {sentence}"),
                None => format!(
                    "none of {} phrasings ({}) was found anywhere in the manuscript",
                    names.len(),
                    names.join(", ")
                ),
            },
            guideline_source: Some(first.source_url.clone()),
            source_span: Some(first.source_span.clone()),
            article_type: first.article_type.clone(),
            checked_field: Some("manuscript full text".into()),
            unevaluable: false,
            also_from: rest.iter().map(|r| source_of(r)).collect(),
        }, manuscript_article_type));
    }

    // --- reporting standards ----------------------------------------------
    //
    // One item per BOUND standard. An unbound mention selects nothing — see
    // `journal_standards`: a standard named is not a standard bound.
    let mut seen: std::collections::BTreeSet<(&str, &str)> = std::collections::BTreeSet::new();
    // **A standard's ITEMS are evaluated once, however many designs bind it.**
    // Nature Medicine binds CONSORT to three designs through six sentences. The
    // binding rows differ and all are worth showing; the item verdicts do not
    // depend on the design, so emitting them per binding produced ten identical
    // rows on a real run.
    let mut evaluated: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for b in bindings {
        if !seen.insert((b.standard.as_str(), b.design.as_str())) {
            continue;
        }
        // **Evaluate, do not merely announce.** This used to emit one row per
        // binding saying the standard applied, with the coverage buried in its
        // prose. It now runs the standard's items against the manuscript and
        // emits one row each — and every row carries the coverage fraction,
        // because a reader who sees "2 met" and no denominator has been told
        // the manuscript passed STROBE when 20 of its 22 items were never read.
        let eval = evaluate(b.standard, extraction, manuscript_text);
        let coverage = eval.coverage_phrase();
        let published = eval.published_items;
        // **A journal that RECOMMENDS a standard has not REQUIRED it.** Nature
        // Medicine writes "We recommend following the ARRIVE 2.0 reporting
        // guidelines" and "Observational studies must be reported according to
        // the STROBE statement" on the same site; rendering both as "requires"
        // overstates one of them. The span carries the modality and the item
        // must not flatten it — §7's requirement/convention/expectation
        // separation is about exactly this kind of upgrade.
        let span_lower = b.source_span.to_lowercase();
        let mandatory = span_lower.contains("must ")
            || span_lower.contains("are required")
            || span_lower.contains("is required")
            || span_lower.contains("shall ");
        let verb = if mandatory { "requires" } else { "recommends" };
        let article = if b.design.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        items.push(ChecklistItem {
            // One source: this row is not derived from journal_requirements.
            also_from: Vec::new(),
            requirement: format!("{} applies to {article} {}", b.standard.as_str(), b.design),
            // Whether the manuscript IS that design is not known here, so this
            // reports the binding rather than judging compliance.
            //
            // **And "reports rather than judges" is exactly `unevaluable`, not
            // `passed`. §11 D182.** A green PASS on a row whose own detail opens
            // "IF your study is a clinical trial" is read as "my manuscript
            // satisfies CONSORT". It is worst where `published` is `None` —
            // *"[PASS] CHEERS applies to an economic evaluation … Gaply has no
            // evaluator for this standard yet"* — a pass on a check that does
            // not exist. Same defect as D177's specialist and D178's rule 5,
            // and D177 is precisely why the design is unknown here.
            passed: false,
            detail: match published {
                Some(_) => format!(
                    "if your study is {article} {}, this journal {verb} {} reporting. Gaply {}.",
                    b.design,
                    b.standard.as_str(),
                    coverage
                ),
                None => format!(
                    "if your study is {article} {}, this journal {verb} {} reporting. Gaply \
                     has no evaluator for this standard yet.",
                    b.design,
                    b.standard.as_str()
                ),
            },
            guideline_source: None,
            source_span: Some(b.source_span.clone()),
            article_type: None,
            // The BINDING is what this row reports, and the binding comes from
            // the journal's sentence. It used to name
            // `research_state.science.methods`, which is now a declined layer
            // (§11 D165) and was never what decided this row anyway.
            checked_field: Some("journal_requirements.reporting_standard (binding)".into()),
            unevaluable: true,
        });

        // --- one row per item, each carrying the fraction -------------------
        if !evaluated.insert(b.standard.as_str()) {
            continue;
        }
        let applies = {
            let mut d: Vec<&str> = bindings
                .iter()
                .filter(|o| o.standard == b.standard)
                .map(|o| o.design.as_str())
                .collect();
            d.sort_unstable();
            d.dedup();
            format!("if your study is a {}", d.join(", a "))
        };
        for v in &eval.verdicts {
            items.push(ChecklistItem {
                // One source: this row is not derived from journal_requirements.
                also_from: Vec::new(),
                // **THE FRACTION IS IN THE ROW.** Not a footnote, not a header
                // the reader scrolled past: every item restates how much of the
                // standard was examined, so no single row can be read as a
                // verdict on the standard.
                // **The conditional is on EVERY item row, not only the binding
                // row.** Whether the manuscript IS that design is not knowable
                // here — study design lives in the declined scientific layer —
                // so an unconditional "CONSORT item 1b: MET" on a
                // cross-sectional survey asserts compliance with a standard that
                // may not apply to it at all. Measured on a real run.
                requirement: format!(
                    "{} item {} ({applies}; {coverage}): {}",
                    v.standard.as_str(),
                    v.item,
                    v.requirement
                ),
                // `Unevaluable` is NOT passed. An item nobody could decide must
                // not render as a tick; that is the difference between "we
                // looked and it is there" and "we cannot look".
                passed: v.status == ItemStatus::Met,
                unevaluable: v.status == ItemStatus::Unevaluable,
                detail: match (&v.evidence_span, v.status) {
                    (Some(span), ItemStatus::Met) => format!("{} — {span}", v.detail),
                    _ => v.detail.clone(),
                },
                guideline_source: None,
                // The JOURNAL's sentence — the one that bound this standard to
                // this design, and the reason the item is being applied at all.
                source_span: Some(b.source_span.clone()),
                article_type: None,
                checked_field: Some(v.reads.join(" / ")),
            });
        }
    }

    items.extend(unbound_standard_findings(requirements, &seen));
    items
}

/// **A standard this journal did not bind is a FINDING about the journal, not a
/// silent omission by the product.**
///
/// A researcher submitting a randomised trial who sees no CONSORT rows has two
/// possible explanations — Gaply cannot evaluate CONSORT, or this journal never
/// asked for it — and they are opposite messages. Only the second is checkable,
/// and stating it plainly invites the correction that a guess would not: the
/// researcher can open the page and tell us we read it wrong.
///
/// Two shapes, because they mean different things:
///
/// * **named, but no design stated.** Measured on Nature Medicine, 15 Sep 2026:
///   *"Studies reporting biomarkers in association with clinical outcomes must
///   follow the STARD guidelines"* is a requirement with mandatory force, and
///   `bindings_from` produced nothing from it because "biomarkers" is not a
///   design phrase it knows. **The lexicon is NOT extended from this one
///   sentence** — a list written from one observation is the mistake the
///   heading-vocabulary measurement caught — so the product reports what it saw
///   and says it could not route it.
/// * **never named at all**, and only for standards that have an evaluator.
///   Saying "this journal does not require CHEERS" when nothing could have
///   checked CHEERS anyway is clutter, not information.
fn unbound_standard_findings(
    requirements: &[crate::journal_store::StoredRequirement],
    bound: &std::collections::BTreeSet<(&str, &str)>,
) -> Vec<ChecklistItem> {
    use crate::journal_extract::RequirementKind;
    use crate::journal_standards::{items_for, Standard};

    let bound_standards: std::collections::BTreeSet<&str> =
        bound.iter().map(|(s, _)| *s).collect();
    let pages: std::collections::BTreeSet<&str> =
        requirements.iter().map(|r| r.source_url.as_str()).collect();
    // With nothing read, "this journal does not require X" is a claim about an
    // empty crawl wearing a claim about a journal.
    if pages.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for s in [
        Standard::Consort,
        Standard::Prisma,
        Standard::Strobe,
        Standard::Arrive,
        Standard::Tripod,
        Standard::Cheers,
        Standard::Spirit,
        Standard::Stard,
    ] {
        if bound_standards.contains(s.as_str()) {
            continue;
        }
        let named = requirements.iter().find(|r| {
            r.kind == RequirementKind::ReportingStandard
                && Standard::parse(&r.value) == Some(s)
        });
        let has_evaluator = !items_for(s).is_empty();

        let (requirement, detail, span, url) = match named {
            Some(r) => (
                format!("{}: named by this journal, but no study design stated", s.as_str()),
                format!(
                    "This journal's guidance names {} without stating which study designs it                      applies to, so Gaply could not select it. If your study is one it covers,                      apply the checklist yourself — and if the sentence below does name a                      design, we read it wrong and would like to know.",
                    s.as_str()
                ),
                Some(r.source_span.clone()),
                Some(r.source_url.clone()),
            ),
            None if has_evaluator => (
                format!("{}: not required by this journal", s.as_str()),
                format!(
                    "This journal does not state a {} requirement on the {} page(s) we read.                      Gaply can evaluate {}, so this is a fact about the journal's published                      guidance rather than a gap in the check — and it is checkable: if the                      journal requires it somewhere we did not read, please tell us.",
                    s.as_str(),
                    pages.len(),
                    s.as_str()
                ),
                None,
                None,
            ),
            None => continue,
        };

        out.push(ChecklistItem {
            // One source: this row is not derived from journal_requirements.
            also_from: Vec::new(),
            requirement,
            // Neither shape is a compliance failure. `passed` is the wrong axis
            // for a statement about what the journal asked, and `false` would
            // render as a red mark against a manuscript that did nothing wrong.
            //
            // **Neither is right, and `true` was the wrong half of the choice.
            // §11 D182.** This row is not a verdict on the manuscript at all —
            // it says the JOURNAL named a standard without stating which designs
            // it covers. `passed: true` renders green, which a reader takes as
            // "my manuscript satisfies this", on a check that was never run.
            // That is `Unevaluable` rendered as `Met`, the same defect repaired
            // in `claim_strength::applies_to` (D177) and `validate`'s rule 5
            // (D178). The third state is the honest one and it already exists.
            passed: false,
            detail,
            guideline_source: url,
            source_span: span,
            article_type: None,
            checked_field: Some("journal_requirements.reporting_standard (absence)".into()),
            unevaluable: true,
        });
    }
    out
}

/// **The checklist rows that do NOT depend on knowing the study's design.**
///
/// §11 D182. `checklist_from_requirements` also emits reporting-standard rows —
/// one per binding ("CONSORT applies to a clinical trial") and one per named-but-
/// unbound standard. Both are hedged on a design the product cannot determine,
/// because §11 D177 DECLINED the gate that would: its input read `randomised`
/// out of *"randomised controlled trials could evaluate"* and missed four
/// live-animal studies entirely. Measured on `Revised Health Economics` against
/// Nature Medicine, shipping them gives a researcher 46 rows of which 25 are
/// hedged and 14 undecided — including an ARRIVE animal-study item evaluated
/// against a cross-sectional employer survey, which is the gate's absence made
/// visible.
///
/// **USE IT WITH `bindings: &[]`, and both halves are needed.** The per-BINDING
/// rows ("CONSORT applies to a clinical trial") and the per-ITEM rows ("CONSORT
/// item 1b (if your study is a clinical trial…)") are emitted inside
/// `for b in bindings`, so passing none suppresses them at the source; the
/// per-item rows cannot be filtered afterwards because their `checked_field`
/// names the extraction field each item read, which is real information and not
/// a marker. What passing none does NOT suppress is
/// `unbound_standard_findings`, which then reports every standard as unbound —
/// and that is what this filter removes.
///
/// **Filtering here rather than inside `checklist_from_requirements`** keeps
/// that function's contract whole: `bindings.is_empty()` is NOT the same
/// statement as "this caller does not want standards" — a journal may genuinely
/// bind none — and gating on it inside the function broke the two tests written
/// to exercise exactly that case. The caller says what it wants; the function
/// keeps saying what it knows.
pub fn design_independent(items: Vec<ChecklistItem>) -> Vec<ChecklistItem> {
    items
        .into_iter()
        .filter(|i| {
            !i.checked_field
                .as_deref()
                .is_some_and(|f| f.starts_with("journal_requirements.reporting_standard"))
        })
        .collect()
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
            // One source: this row is not derived from journal_requirements.
            also_from: Vec::new(),
            requirement: format!("required section: {name}"),
            passed: present,
            detail: if present {
                format!("{name} section found")
            } else {
                format!("{name} section missing")
            },
            guideline_source: None,
            source_span: None,
            article_type: None,
            // A structural check reads the extraction's own section list.
            checked_field: Some("extraction.sections".into()),
            unevaluable: false,
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
        //
        // **The requirement is "structured"; the check is "exists". §11 D181.**
        // `passed: true` on any Abstract section claimed the STRUCTURE had been
        // verified when only its presence had — the detail string admitted it
        // ("structure itself needs editorial review") while `passed` said
        // otherwise, and `passed` is what a reader sees. Measured: 6 of 20
        // manuscripts would have taken that PASS. Nothing here parses headed
        // subsections, so the honest state is the third one — `unevaluable` —
        // which is exactly what that field exists for. A missing abstract is
        // still a real FAIL, because that much IS decidable.
        if g.contains("structured abstract") {
            let has_abstract =
                extraction.sections.iter().any(|s| s.kind == SectionKind::Abstract);
            items.push(ChecklistItem {
                // One source: this row is not derived from journal_requirements.
                also_from: Vec::new(),
                requirement: "structured abstract".into(),
                passed: false,
                detail: if has_abstract {
                    "an abstract is present, but whether it is STRUCTURED (headed \
                     subsections such as Background / Methods / Results) is not checked \
                     here. This item is undecided, not passed."
                        .into()
                } else {
                    "no abstract section found".into()
                },
                guideline_source: src.clone(),
                source_span: statement_in_text(&g, &hit.content, &["structured abstract"]),
                article_type: None,
                checked_field: Some("extraction.sections".into()),
                unevaluable: has_abstract,
            });
        }
        // conflict-of-interest declaration
        //
        // **`contains("conflict of interest")` — singular — reported a FALSE FAIL
        // on a manuscript containing the statement. §11 D181.**
        // `Revised Health Economics Paper FINAL (1).docx` writes *"Conflicts of
        // Interest: The authors declare no conflicts of interest."* and was told
        // to add one. `synonyms_for` lists that exact plural, and its doc comment
        // cites that exact sentence as the case it was written for — it was
        // called only from `checklist_from_requirements`, which has no
        // production caller. So did `statement_in_text`, which returns the
        // SENTENCE precisely so a "found" row can be checked in a glance.
        // Measured over 20 manuscripts: the two agree on 19 and the singular
        // check is wrong on the 1.
        // **The TRIGGER was as wrong as the check.** `g.contains("conflict")`
        // matched PLOS ONE on a sample-reference TITLE — *"Amino acid metabolism
        // conflicts with protein diversity."* — inside the ICMJE example list,
        // while PLOS's actual policy says *"Competing interests"*. The bare word
        // never touched the requirement. Keying the trigger on the same
        // REQUIREMENT phrasings the check uses means the sentence that raises
        // the item is the sentence that states it, which is what `source_span`
        // then shows. §11 D181.
        let coi_names = synonyms_for("competing interest");
        if coi_names.iter().any(|n| g.contains(n)) {
            let names = coi_names.clone();
            let found = statement_in_text(&text_lower, manuscript_text, &names);
            items.push(ChecklistItem {
                // One source: this row is not derived from journal_requirements.
                also_from: Vec::new(),
                requirement: "conflict-of-interest declaration".into(),
                passed: found.is_some(),
                detail: match &found {
                    Some(sentence) => format!("found in the manuscript: {sentence}"),
                    None => format!(
                        "none of {} phrasings was found anywhere in the manuscript",
                        names.len()
                    ),
                },
                guideline_source: src.clone(),
                source_span: statement_in_text(&g, &hit.content, &coi_names),
                article_type: None,
                checked_field: Some("manuscript full text".into()),
                unevaluable: false,
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
                // One source: this row is not derived from journal_requirements.
                also_from: Vec::new(),
                requirement: "numbered (Vancouver) reference style".into(),
                passed,
                detail: format!("{numbered}/{total} reference entries are numbered"),
                guideline_source: src.clone(),
                // **The trigger is a keyword and the row now shows it.** §11 D181
                // audited all three branches after one was found wrong; this
                // one's LOGIC is correct on the corpus (PLOS fires it on
                // *"References are listed at the end of the manuscript and
                // numbered in the order that they appear in the text"*, and says
                // "Vancouver" in two chunks). But `g.contains("numbered")` would
                // equally fire on *"tables should be numbered consecutively"*.
                // No instance of that exists in the six ingested journals, so it
                // is named as a risk rather than claimed as a defect — and the
                // journal's own sentence is carried so a reader can see which
                // one triggered it.
                source_span: statement_in_text(&g, &hit.content, &["vancouver", "numbered"]),
                article_type: None,
                checked_field: None,
                unevaluable: false,
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

// ---------------------------------------------------------------------------
// Reporting-standard evaluation — THE MANUSCRIPT x JOURNAL JOIN
// ---------------------------------------------------------------------------
//
// **This lives in `report.rs` because it is the only place it may.**
//
// `tests/journal_layer_has_no_manuscript.rs` refuses any `journal_*` module a
// route to the manuscript layer: a `JournalFingerprint` is built once per
// journal and served to every user, so a journal fact depending on one user's
// manuscript could neither be shared nor kept private. The first draft of this
// evaluator was written in `journal_standards.rs` and **that guard failed the
// build**, correctly — `evaluate` reads one user's `ExtractionResult`.
//
// The guard's own header names `report::checklist_from_requirements` as the
// legitimate join, and warns that a future violation could hide by moving code
// into `report.rs` because `report.rs` is not scanned. So this note is the
// compensating record: the join is here DELIBERATELY, it is the sanctioned
// location, and `journal_standards.rs` keeps only what is true of a standard
// independent of any manuscript — the item lists, the bindings, the published
// counts, and `ItemCheck`, which names the EVIDENCE an item needs without
// naming a manuscript type.

/// What an item's test decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    /// The evidence the item asks for is present.
    Met,
    /// The item could be decided and the evidence is absent.
    NotFound,
    /// **The item was not decided.** Distinct from `NotFound`, and the
    /// distinction is the whole point: *"we looked and it is missing"* and
    /// *"we cannot look"* are opposite messages to a researcher, and an
    /// evaluator that renders the second as the first invents a compliance
    /// failure.
    Unevaluable,
}

/// One item's verdict, with the manuscript evidence it rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemVerdict {
    pub standard: crate::journal_standards::Standard,
    pub item: &'static str,
    pub requirement: &'static str,
    pub status: ItemStatus,
    pub reads: &'static [&'static str],
    /// The manuscript sentence or paragraph the verdict rests on, **whole**.
    /// `None` for `NotFound` and `Unevaluable` — there is nothing to quote, and
    /// a fabricated quotation would be worse than none.
    pub evidence_span: Option<String>,
    pub location: Option<Location>,
    /// Why, in one sentence, for a reader who will not read the code.
    pub detail: String,
}

/// A standard evaluated against one manuscript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StandardEvaluation {
    pub standard: crate::journal_standards::Standard,
    pub verdicts: Vec<ItemVerdict>,
    /// Items this engine carries at all.
    pub items_implemented: usize,
    /// Items it can actually decide — `items_implemented` minus those whose
    /// evidence lives only in a field the project does not produce.
    pub items_evaluable: usize,
    /// What the PUBLISHED standard has, WITH ITS UNIT — see
    /// [`crate::journal_standards::PublishedCount`]. `None` for standards with
    /// no evaluator.
    pub published_items: Option<crate::journal_standards::PublishedCount>,
}

impl StandardEvaluation {
    pub fn met(&self) -> usize {
        self.verdicts.iter().filter(|v| v.status == ItemStatus::Met).count()
    }
    pub fn not_found(&self) -> usize {
        self.verdicts.iter().filter(|v| v.status == ItemStatus::NotFound).count()
    }

    /// **The fraction, phrased for the ROW rather than for a footnote.**
    ///
    /// The number that belongs beside a verdict is not how many items this
    /// engine ships — it is how many it could DECIDE, over how many the
    /// published standard has. Those differ by more than half: CONSORT ships 5
    /// items of 25 and can decide 4; TRIPOD ships 5 of 22 and can decide 1.
    ///
    /// **A passing evaluator is not a passed checklist**, and this sentence is
    /// the only thing standing between the two for a reader who sees "3 of 4
    /// met" and stops there.
    pub fn coverage_phrase(&self) -> String {
        let Some(count) = self.published_items else {
            return format!("no evaluator for {}", self.standard.as_str());
        };
        let name = self.standard.as_str();
        match (count.sub_items, self.standard.items_are_sub_items()) {
            // The unit matches: both sides count checklist rows.
            (Some(rows), _) => {
                format!("checks {} of {name}'s {rows} checklist rows", self.items_evaluable)
            }
            // The numerator counts sub-items and the only denominator recorded
            // counts numbered items. **Say so rather than pick a number**: a
            // fabricated sub-item total would look precise and be unsourced.
            (None, true) => format!(
                "checks {} of {name}'s {} numbered items — Gaply's items include sub-items, so the true fraction is smaller",
                self.items_evaluable, count.numbered
            ),
            (None, false) => format!(
                "checks {} of {name}'s {} numbered items",
                self.items_evaluable, count.numbered
            ),
        }
    }
}

/// **Evaluate a standard against a manuscript. Deterministic, no model.**
///
/// Reads [`ExtractionResult`] only — never the scientific layer
/// (§11 D165). Every `Met` carries the paragraph it was decided from, so a
/// reader can refute it; `NotFound` and `Unevaluable` carry none, because there
/// is nothing honest to quote.
/// # A MISSING SECTION MUST NOT BE A MISSING HEADING
///
/// `AbstractPresent` and `ResultsSectionPresent` used to read
/// `extraction.sections` alone, which §4.5 [v8] classes as a MEDIATED field:
/// `classify_heading` knows a fixed vocabulary, and a heading outside it is
/// indistinguishable from a section that is not there.
///
/// **Measured over 20 real manuscripts: 29 section-absence verdicts, of which
/// 11 (38%) are contradicted by the manuscript's own text**, every one a
/// heading the classifier does not know:
///
/// ```text
/// IV. EXPERIMENTS AND RESULTS          4.8 INTERPRETATION OF RESULTS
/// Synopsis Abstract                    3.18 CHAPTER SUMMARY
/// ```
///
/// Each became a MAJOR concern in the reviewer report. So `manuscript_text` is
/// scanned for a HEADING-SHAPED line before `NotFound` is returned, which turns
/// a mediated read into an exhaustive one — the same correction
/// `checklist_from_requirements` needed for required statements.
pub fn evaluate(
    standard: crate::journal_standards::Standard,
    extraction: &ExtractionResult,
    manuscript_text: &str,
) -> StandardEvaluation {
    use crate::extract::stats::Stat;

    use crate::journal_standards::{items_for, published_item_count, ItemCheck};
    let items = items_for(standard);
    let mut verdicts = Vec::with_capacity(items.len());

    // The first statistic of each kind, with where it was found. Computed once
    // rather than per item — four passes over the same vector for the same
    // answer is the mistake the regex-caching entry records.
    //
    // **A statistic whose paragraph does not RESOLVE is skipped, not quoted
    // empty.** 41 of 301 spans in this corpus fail to resolve and 159 name an
    // ambiguous `SectionKind` (§12.1's `Location` defect). `Some("")` would be a
    // verdict claiming evidence and showing none — the shape a truncated span
    // already cost this project once — so the search moves to the next
    // statistic of that kind and reports `NotFound` if none of them resolves.
    let first = |pred: &dyn Fn(&Stat) -> bool| {
        extraction.statistics.iter().filter(|s| pred(&s.stat)).find_map(|s| {
            crate::extract::paragraph_at(extraction, &s.location)
                .filter(|p| !p.trim().is_empty())
                .map(|p| (p.to_string(), s.location.clone()))
        })
    };

    for it in items {
        let (status, evidence, location, detail) = match it.check {
            ItemCheck::NotImplemented => (
                ItemStatus::Unevaluable,
                None,
                None,
                format!(
                    "not checked: the evidence for this item lives only in {}, which Gaply \
                     does not produce (§11 D165)",
                    it.reads.join(" / ")
                ),
            ),
            ItemCheck::AbstractPresent => {
                section_check(extraction, manuscript_text, SectionKind::Abstract, &["abstract", "summary", "synopsis"])
            }
            ItemCheck::ResultsSectionPresent => {
                section_check(extraction, manuscript_text, SectionKind::Results, &["results", "findings"])
            }
            ItemCheck::SampleSizeReported => decide(
                first(&|s| matches!(s, Stat::SampleSize { .. })),
                "an explicit sample size is reported",
                "no explicit sample size (`n = …`) was found anywhere in the manuscript",
            ),
            ItemCheck::StatisticalTestReported => decide(
                first(&|s| matches!(s, Stat::Test { .. } | Stat::TestStatistic { .. })),
                "a named statistical test is reported",
                "no named statistical test or test statistic was found",
            ),
            ItemCheck::EffectSizeReported => decide(
                first(&|s| matches!(s, Stat::EffectSize { .. })),
                "an effect size is reported with its value",
                "no effect size with a value was found",
            ),
            ItemCheck::ConfidenceIntervalReported => decide(
                first(&|s| matches!(s, Stat::ConfidenceInterval { .. })),
                "a confidence interval is reported",
                "no confidence interval was found",
            ),
        };

        verdicts.push(ItemVerdict {
            standard,
            item: it.item,
            requirement: it.requirement,
            status,
            reads: it.reads,
            evidence_span: evidence,
            location,
            detail,
        });
    }

    StandardEvaluation {
        standard,
        items_implemented: items.len(),
        items_evaluable: items.iter().filter(|i| i.check.is_evaluable()).count(),
        published_items: published_item_count(standard),
        verdicts,
    }
}

type Decided = (ItemStatus, Option<String>, Option<Location>, String);

fn decide(
    found: Option<(String, Location)>,
    met: &str,
    missing: &str,
) -> Decided {
    match found {
        Some((span, loc)) => (ItemStatus::Met, Some(span), Some(loc), met.to_string()),
        None => (ItemStatus::NotFound, None, None, missing.to_string()),
    }
}


/// **Is a section of this kind present?** Classifier first, then the full text.
///
/// The second pass is what stops a heading the classifier does not know from
/// being reported as a missing section — see [`evaluate`]'s header for the 11
/// of 29 that were.
fn section_check(
    extraction: &ExtractionResult,
    manuscript_text: &str,
    kind: SectionKind,
    words: &[&str],
) -> Decided {
    if let Some((sec_idx, sec)) = extraction
        .sections
        .iter()
        .enumerate()
        .find(|(_, s)| s.kind == kind && !s.paragraphs.is_empty())
    {
        return (
            ItemStatus::Met,
            sec.paragraphs.first().cloned(),
            Some(Location::in_section(kind, sec_idx, 0)),
            format!("a {kind:?} section is present with content"),
        );
    }
    // The classifier found nothing. Look for a HEADING-SHAPED line: short, not
    // a sentence, containing the word. "IV. EXPERIMENTS AND RESULTS" qualifies;
    // a sentence mentioning results does not.
    if let Some(line) = heading_shaped_line(manuscript_text, words) {
        return (
            ItemStatus::Met,
            Some(line.clone()),
            None,
            format!(
                "no {kind:?} section was classified, but the manuscript carries the heading \
                 \"{line}\" — the heading vocabulary did not recognise it, which is a fact \
                 about the extractor and not about the manuscript"
            ),
        );
    }
    (
        ItemStatus::NotFound,
        None,
        None,
        format!(
            "no {kind:?} section was classified and no heading-shaped line matching {words:?} \
             was found anywhere in the manuscript"
        ),
    )
}

/// A line that looks like a heading and names one of `words`.
///
/// Heading-shaped: 60 characters or fewer, does not end in a sentence
/// terminator, and is not a table-of-contents row (those end in a page number
/// after a tab or a run of dots).
fn heading_shaped_line(text: &str, words: &[&str]) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.chars().count() <= 60)
        .filter(|l| !l.ends_with(['.', '!', '?', ',', ';', ':']))
        .filter(|l| !l.contains('\t') && !l.contains("..."))
        .find(|l| {
            let lower = l.to_lowercase();
            words.iter().any(|w| lower.split_whitespace().any(|t| t.trim_matches(|c: char| !c.is_alphanumeric()) == *w))
        })
        .map(str::to_string)
}
