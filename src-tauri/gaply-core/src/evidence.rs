//! Evidence Model (Box 0) — the foundational types every PublishReady-rebuild
//! box consumes/emits.
//!
//! THE HONESTY INVARIANT: every [`EvidenceRecord`] carries a [`ConfidenceKind`]
//! that a consumer reads BEFORE `confidence`, so a placeholder (Extraction) or a
//! deliberately-coarse value (AI-detection) can never be silently thresholded
//! like a real one. [`confidence_kind`], [`routing_hint`] and [`limitations`]
//! are TOTAL over [`AgentKind`] with NO wildcard arm: adding a seventh agent
//! will not compile until its evidence tier is explicitly decided here. This is
//! where the spike-series reality (which agents have real confidence) is frozen.
//!
//! Provider-blind: pure local data, no proxy/LLM types.

use serde::{Deserialize, Serialize};

use crate::report::{Finding, FindingSeverity};
use crate::swarm::AgentKind;

/// Bump when a change AFFECTS CACHED-REPORT COMPATIBILITY — not on every
/// modification.
///
/// **Renamed from `EVIDENCE_SCHEMA_VERSION`, value carried forward so the rename
/// itself invalidates nothing.** The old name described the `EvidenceRecord`
/// shape while the rule was always stated in terms of the CACHED REPORT's
/// compatibility — a mismatch latent since `8af96a7` that began to bite when the
/// report itself became typed. One number meaning *"the cached report's bytes
/// changed shape"* answers the question once; two versions in one key would ask
/// *"did this touch the report shape, the evidence shape, or both?"* at every
/// change. An optional field with a serde default leaves cached reports
/// readable and needs no bump; requiring one for every change would train
/// reflexive bumping, which is how versions stop meaning anything.
///
/// * **1 → 2** — `claim: ClaimKind` added as a REQUIRED field
///   (ARCHITECTURE_TRACE §26). An optional-with-default would have been
///   compatible and WRONG: stale cached reports would take the default, so
///   process-state findings would count toward the verdict — §23.4's measured
///   defect, reintroduced for cached data and invisible. The version is in the
///   report cache key, so stale entries miss and recompute.
pub const CACHED_REPORT_SCHEMA_VERSION: u32 = 2;

/// Structured-provenance prefixes — THE canonical list (single source of truth;
/// `reviewer_agent` imports [`is_structured_provenance`], it does not keep a copy).
/// Mirrors the frontend `structuredProvenance` filter.
pub const STRUCTURED_PREFIXES: &[&str] = &[
    "rule:",
    "evidence:",
    "swarm:",
    "agent:",
    "gate:",
    "similarity:",
    "match_type:",
    "source:",
    "signal:",
];

/// True when a provenance tag is a groundable structured ref (vs free prose).
pub fn is_structured_provenance(p: &str) -> bool {
    STRUCTURED_PREFIXES.iter().any(|prefix| p.starts_with(prefix))
}

/// WHY a `confidence` number is what it is — READ THIS BEFORE the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceKind {
    /// Genuine per-item model confidence (Verification).
    RealNative,
    /// Mathematically certain (Validation): 1.0, not probabilistic.
    Deterministic,
    /// Real signal, production-wired (RAG distance, Plagiarism similarity).
    WiredReal,
    /// Real magnitude exists but calibration does NOT support fine granularity
    /// (AI-detection) — present, but explicitly low-resolution.
    DeliberatelyCoarse,
    /// Placeholder — NEVER usable for routing (Extraction; gate-rejected).
    NoSignal,
}

/// WHAT EDITORIAL STATEMENT a finding makes — the dimension `Finding` lacked
/// (ARCHITECTURE_TRACE §23.7).
///
/// # A default and two carve-outs, NOT a taxonomy
///
/// All three variants answer one question: *what is this claim about* — our own
/// execution, model authorship, or a manuscript flaw. [`ConfidenceKind`] is what
/// describes epistemic character; [`ClaimKind::AuthorshipSignal`] describes what
/// the claim ASSERTS.
///
/// The GRAIN is deliberately uneven. `ProcessState` and `AuthorshipSignal` are
/// each one narrow thing; `ManuscriptDefect` covers statistics, citations,
/// textual overlap, stylometry, tables and reference recency. **It is a default
/// plus two carve-outs, and the carve-outs are exactly the two traced
/// instances.**
///
/// **This enum is intentionally instance-driven rather than taxonomically
/// complete. Future variants should only be introduced when demanded by traced
/// evidence.**
///
/// # Why it exists
///
/// * `ProcessState` — run 22's f4 (*"35 of 35 citation(s) could not be
///   checked"*) and f5 (*"Verification output rejected by its internal gate"*)
///   were two of four `Minor` findings and **changed the recommendation**
///   (§23.4). Both describe Gaply's execution, not the manuscript. Showing the
///   author *"we couldn't check your citations"* is correct; letting it change
///   their recommendation is not.
/// * `AuthorshipSignal` — `ai_detect.rs`'s own disclaimer says *"NOT proof of AI
///   authorship"*, yet `report.rs` maps the swarm's `concern` answer to `Major`.
///   It cannot be excluded by `AgentKind`, because the same agent also produces
///   the stylometric findings `report.rs` explicitly documents as
///   reviewer-relevant (§22.5).
///
/// # Extension cost
///
/// A fourth VARIANT is compatible (additive). A second optional field with a
/// serde default is compatible. **Changing a plain variant into a DATA-CARRYING
/// one is INCOMPATIBLE** — the wire form goes from `"process_state"` to
/// `{"process_state": {…}}` and every historical fixture fails. The bet taken
/// here is that the carve-outs stay parameterless; if a future one needs a
/// parameter, add a new parameterless variant or a second field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// A statement about GAPLY'S OWN EXECUTION — a lane that could not run, a
    /// gate that rejected an output, a parse that failed. True and worth showing;
    /// never evidence about the manuscript.
    ProcessState,
    /// A statement that the text may be model-generated. An observation about
    /// provenance, not a publishability defect.
    AuthorshipSignal,
    /// A statement about the MANUSCRIPT that bears on publishability — the
    /// default, and everything not carved out above.
    ManuscriptDefect,
}

/// The per-agent routing POLICY, carried as DATA (the record holds no logic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingHint {
    /// Deterministic — pass through untouched (Validation).
    NeverEscalate,
    /// Real signal — the orchestrator applies RoutingPolicy thresholds
    /// (Verification, Plagiarism).
    ThresholdEligible,
    /// Escalate by POLICY, never by a raw threshold (AI-detection).
    PolicyEligible,
    /// Real confidence, but used to gate CONTEXT-attachment, not escalation (RAG).
    ContextOnly,
    /// Never enters adaptive routing (Extraction; gate-rejected).
    HeldOut,
}

/// The Orchestrator's action for one record (shape only; the LOGIC is Box 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingAction {
    Accept,
    Escalate,
    PassThrough,
}

/// The Orchestrator's decision for one record (shape only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub action: RoutingAction,
    /// Human-auditable ("verification UNKNOWN -> escalate").
    pub reason: String,
    /// 0 = most urgent (matches the existing `severity.rank()` convention).
    pub priority: u8,
}

/// The common record every box consumes/emits.
///
/// Derives `Deserialize` (added in Box 2) so the Orchestrator can reconstruct
/// records from the cached report JSON (`report["evidence"]`); this also pulled
/// `Deserialize` onto `AgentKind`/`FindingSeverity`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    /// Stable per-report finding id (reuses the `f{N}` scheme from
    /// `build_review_payload`), assigned by the caller.
    pub id: String,
    pub agent: AgentKind,
    /// WHAT this finding asserts — the dimension the record previously lacked.
    /// Carried from the `Finding` that produced it; never re-derived from
    /// `agent`, which answers a different question (§23.2).
    pub claim: ClaimKind,
    pub severity: FindingSeverity,
    pub confidence: f64,
    pub confidence_kind: ConfidenceKind,
    pub provenance: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub routing_hint: RoutingHint,
    pub limitations: Option<String>,
    pub schema_version: u32,
}

/// `agent -> confidence_kind`: the spike-series evidence tiers, frozen. TOTAL —
/// no wildcard, so a new `AgentKind` must be classified here to compile.
pub fn confidence_kind(agent: AgentKind) -> ConfidenceKind {
    match agent {
        AgentKind::Verification => ConfidenceKind::RealNative,
        AgentKind::ValidationMaths => ConfidenceKind::Deterministic,
        AgentKind::Plagiarism => ConfidenceKind::WiredReal,
        AgentKind::Rag => ConfidenceKind::WiredReal,
        AgentKind::AiDetection => ConfidenceKind::DeliberatelyCoarse,
        AgentKind::Extraction => ConfidenceKind::NoSignal,
    }
}

/// `agent -> routing_hint`: the per-agent policy from the design doc. TOTAL, no
/// wildcard. RAG is `ContextOnly` (gates context-attachment, not escalation).
pub fn routing_hint(agent: AgentKind) -> RoutingHint {
    match agent {
        AgentKind::ValidationMaths => RoutingHint::NeverEscalate,
        AgentKind::Verification => RoutingHint::ThresholdEligible,
        AgentKind::Plagiarism => RoutingHint::ThresholdEligible,
        AgentKind::Rag => RoutingHint::ContextOnly,
        AgentKind::AiDetection => RoutingHint::PolicyEligible,
        AgentKind::Extraction => RoutingHint::HeldOut,
    }
}

/// `agent -> limitations`: the honest caveat shown to any consumer. TOTAL, no
/// wildcard. `None` for the trustworthy agents.
pub fn limitations(agent: AgentKind) -> Option<String> {
    match agent {
        AgentKind::AiDetection => Some(
            "AI-detection: coarse signal — escalate by policy, never threshold on this number"
                .to_string(),
        ),
        AgentKind::Extraction => {
            Some("Extraction: confidence is a placeholder; never used for routing".to_string())
        }
        AgentKind::Verification
        | AgentKind::ValidationMaths
        | AgentKind::Plagiarism
        | AgentKind::Rag => None,
    }
}

/// True for agents whose `compile_report` Finding carries the swarm-RESCALED
/// weight rather than the raw signal (the soft round-table loop). Verification,
/// Validation and Plagiarism findings carry raw values by construction.
fn confidence_is_rescaled_in_report(agent: AgentKind) -> bool {
    matches!(agent, AgentKind::Extraction | AgentKind::AiDetection | AgentKind::Rag)
}

fn merge_limitations(base: Option<String>, extra: Option<&str>) -> Option<String> {
    match (base, extra) {
        (Some(b), Some(e)) => Some(format!("{b}; {e}")),
        (Some(b), None) => Some(b),
        (None, Some(e)) => Some(e.to_string()),
        (None, None) => None,
    }
}

impl EvidenceRecord {
    /// AT-SOURCE construction (the TRUSTWORTHY path — used by `compile_report`):
    /// the caller passes the RAW, pre-rescale confidence that is in scope at the
    /// finding's creation site. `confidence_kind` / `routing_hint` /
    /// `limitations` / `evidence_refs` are all derived by construction.
    pub fn at_source(
        id: impl Into<String>,
        agent: AgentKind,
        claim: ClaimKind,
        severity: FindingSeverity,
        raw_confidence: f64,
        provenance: Vec<String>,
    ) -> Self {
        let evidence_refs =
            provenance.iter().filter(|p| is_structured_provenance(p)).cloned().collect();
        EvidenceRecord {
            id: id.into(),
            agent,
            claim,
            severity,
            confidence: raw_confidence,
            confidence_kind: confidence_kind(agent),
            provenance,
            evidence_refs,
            routing_hint: routing_hint(agent),
            limitations: limitations(agent),
            schema_version: CACHED_REPORT_SCHEMA_VERSION,
        }
    }

    /// FALLBACK mapping from an already-compiled [`Finding`] (for tests /
    /// external callers). WARNING: for soft-loop agents (Extraction,
    /// AI-detection, RAG) a Finding's `confidence` is the swarm-RESCALED weight,
    /// not the raw signal — so a rescale limitation is attached. A gate-rejected
    /// finding (provenance `swarm:rejected…`) is forced to `NoSignal`/`HeldOut`
    /// regardless of agent. Prefer [`EvidenceRecord::at_source`] where the raw
    /// value is available.
    pub fn from_finding(finding: &Finding, id: impl Into<String>) -> Self {
        let rejected = finding.provenance.iter().any(|p| p.starts_with("swarm:rejected"));
        let evidence_refs =
            finding.provenance.iter().filter(|p| is_structured_provenance(p)).cloned().collect();

        let (confidence_kind_v, routing_hint_v, limitations_v) = if rejected {
            (
                ConfidenceKind::NoSignal,
                RoutingHint::HeldOut,
                Some("rejected by internal gate; signal void".to_string()),
            )
        } else {
            let note = if confidence_is_rescaled_in_report(finding.agent) {
                Some("confidence is swarm-rescaled, not the raw agent signal")
            } else {
                None
            };
            (
                confidence_kind(finding.agent),
                routing_hint(finding.agent),
                merge_limitations(limitations(finding.agent), note),
            )
        };

        EvidenceRecord {
            id: id.into(),
            agent: finding.agent,
            claim: finding.claim,
            severity: finding.severity,
            confidence: finding.confidence,
            confidence_kind: confidence_kind_v,
            provenance: finding.provenance.clone(),
            evidence_refs,
            routing_hint: routing_hint_v,
            limitations: limitations_v,
            schema_version: CACHED_REPORT_SCHEMA_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::CertaintyTier;

    // =====================================================================
    // CACHED_REPORT_SCHEMA_VERSION compatibility pin
    // =====================================================================
    //
    // A cached report carries a serialized `Vec<EvidenceRecord>`, and
    // `escalation.rs:72` deserializes it with `unwrap_or_default()`. If an
    // older binary's record no longer parses, the vector is EMPTY, the
    // aggregator sees zero findings, and the run yields `Accept` at 0.92 — a
    // silent wrong answer (ARCHITECTURE_TRACE §25.10).
    //
    // DIRECTION. Cache entries are read only by the same binary or a NEWER one,
    // never an older one. So:
    //   COMPATIBLE, no bump   — adding an enum variant; adding an optional
    //                           field with a serde default
    //   INCOMPATIBLE, bump    — removing/renaming a variant; renaming a field;
    //                           adding a required field; changing a field type
    //
    // A shape assertion (pinning "these variants exist") would fire on
    // ADDITIONS, which is the wrong direction — claim-identity work will add
    // variants. The property that matters is "previously written data still
    // deserializes", so these fixtures are CHECKED-IN LITERALS. They must never
    // be generated from the live types: a fixture produced by serializing the
    // current struct moves with the code and catches nothing.
    //
    // Historical fixtures are RELEASE ARTIFACTS, not test data. They represent
    // wire formats that have existed in production. Never edit or regenerate an
    // existing fixture. Only append new ones. Deleting an entry to make a test
    // pass deletes the evidence of the break.
    //
    // ----- BLIND SPOT 1: STRUCTURAL -----
    // Compatibility fixtures cannot detect every silent schema change. The known
    // case is RENAMING A FIELD TO AN OPTIONAL-WITH-DEFAULT: the old key is
    // ignored (there is no `deny_unknown_fields`) and the new key defaults, so
    // the old value is silently replaced and no fixture fires. Recorded rather
    // than papered over. `deny_unknown_fields` would catch it but is a RUNTIME
    // behaviour change, so it is deliberately not bundled here.
    //
    // ----- BLIND SPOT 2: SEMANTIC -----
    // These fixtures intentionally do NOT detect changes where cached data still
    // deserializes but its EDITORIAL MEANING has changed. F1 is the instance:
    // validation flags went Critical -> Major, so a pre-fix cached report parses
    // perfectly afterwards, carries the OLD severity, and maps to a DIFFERENT
    // recommendation. Nothing about the serialization is wrong, so a
    // compatibility fixture should not fail.
    //
    // The release constraint's wording covers this — "affects cached-report
    // COMPATIBILITY" is about compatibility, not parsing — but NOTHING ENFORCES
    // THAT HALF, and this pin's existence could make a reader believe it does.
    // It does not.
    //
    // ----- ENFORCEMENT -----
    // This test fires as a side effect of work done anyway (`cargo test -p
    // gaply_core`), NOT as an automatic gate. There is no CI that runs on push
    // (ARCHITECTURE_TRACE §21).

    /// RETIRED AT VERSION 2 — no longer parses, because `claim` became a
    /// required field. Kept as the RELEASE ARTIFACT that documents WHY the
    /// version was bumped. Never delete it; a retired fixture is the evidence of
    /// the break, and `retired_fixtures_no_longer_parse` asserts it stays broken.
    const RETIRED_AT_V2_RECORD: &str = r#"{
        "id": "f1",
        "agent": "plagiarism",
        "severity": "major",
        "confidence": 0.91,
        "confidence_kind": "wired_real",
        "provenance": ["similarity:0.910", "match_type:high word overlap"],
        "evidence_refs": ["similarity:0.910", "match_type:high word overlap"],
        "routing_hint": "threshold_eligible",
        "limitations": null,
        "schema_version": 1
    }"#;

    /// RETIRED AT VERSION 2, same reason. Kept for the same purpose.
    const RETIRED_AT_V2_RECORD_WITH_LIMITATIONS: &str = r#"{
        "id": "f7",
        "agent": "ai_detection",
        "severity": "info",
        "confidence": 0.6,
        "confidence_kind": "deliberately_coarse",
        "provenance": ["agent:ai_detection (document-level stylometry)"],
        "evidence_refs": ["agent:ai_detection (document-level stylometry)"],
        "routing_hint": "policy_eligible",
        "limitations": "AI-detection: coarse signal — escalate by policy, never threshold on this number",
        "schema_version": 1
    }"#;

    // RELEASE ARTIFACTS — wire strings that have existed in production. Removing
    // or renaming a variant makes its entry here fail to deserialize, which is
    // the point. Append only; never edit or regenerate.
    const HISTORICAL_AGENTS: &[&str] = &[
        "extraction",
        "validation_maths",
        "ai_detection",
        "plagiarism",
        "rag",
        "verification",
    ];
    const HISTORICAL_SEVERITIES: &[&str] = &["critical", "major", "minor", "info"];
    const HISTORICAL_CONFIDENCE_KINDS: &[&str] = &[
        "real_native",
        "deterministic",
        "wired_real",
        "deliberately_coarse",
        "no_signal",
    ];
    const HISTORICAL_ROUTING_HINTS: &[&str] = &[
        "never_escalate",
        "threshold_eligible",
        "policy_eligible",
        "context_only",
        "held_out",
    ];

    // The four `*_wire` functions below are TOTAL — no wildcard arm — for one
    // reason: adding a variant must BREAK COMPILATION here.
    //
    // COMPILE ERROR (totality failure):
    //   A new enum variant has no compatibility fixture. Add a checked-in
    //   fixture for this variant. Do NOT bump CACHED_REPORT_SCHEMA_VERSION merely
    //   because a new variant was added.
    //
    // That is TEST COMPLETENESS, a different responsibility from the assertion
    // below (compatibility detection) and from bumping the constant (release
    // policy). Collapsing any two is how the constant loses its meaning.

    fn agent_wire(a: AgentKind) -> &'static str {
        match a {
            AgentKind::Extraction => "extraction",
            AgentKind::ValidationMaths => "validation_maths",
            AgentKind::AiDetection => "ai_detection",
            AgentKind::Plagiarism => "plagiarism",
            AgentKind::Rag => "rag",
            AgentKind::Verification => "verification",
        }
    }

    fn severity_wire(s: FindingSeverity) -> &'static str {
        match s {
            FindingSeverity::Critical => "critical",
            FindingSeverity::Major => "major",
            FindingSeverity::Minor => "minor",
            FindingSeverity::Info => "info",
        }
    }

    fn confidence_kind_wire(c: ConfidenceKind) -> &'static str {
        match c {
            ConfidenceKind::RealNative => "real_native",
            ConfidenceKind::Deterministic => "deterministic",
            ConfidenceKind::WiredReal => "wired_real",
            ConfidenceKind::DeliberatelyCoarse => "deliberately_coarse",
            ConfidenceKind::NoSignal => "no_signal",
        }
    }

    fn routing_hint_wire(r: RoutingHint) -> &'static str {
        match r {
            RoutingHint::NeverEscalate => "never_escalate",
            RoutingHint::ThresholdEligible => "threshold_eligible",
            RoutingHint::PolicyEligible => "policy_eligible",
            RoutingHint::ContextOnly => "context_only",
            RoutingHint::HeldOut => "held_out",
        }
    }

    /// ASSERTION FAILURE (compatibility failure) — the message every assertion
    /// in this test carries.
    fn incompatible(what: &str) -> String {
        format!(
            "{what}\n\n\
             A previously serialized EvidenceRecord no longer deserializes with the current \
             schema. Cached reports written by older binaries are no longer compatible, and \
             escalation.rs:72 will parse them to an empty evidence vector yielding Accept at \
             0.92 (ARCHITECTURE_TRACE §25.10).\n\n\
             If this incompatibility is INTENTIONAL, bump CACHED_REPORT_SCHEMA_VERSION — it is in \
             the report cache key, so stale entries will miss and recompute. Otherwise restore \
             compatibility."
        )
    }

    /// Records as written by the CURRENT schema. RELEASE ARTIFACTS — never edit
    /// or regenerate from `EvidenceRecord`. Append new ones on a bump.
    const RECORD_V2: &str = r#"{
        "id": "f1",
        "agent": "plagiarism",
        "claim": "manuscript_defect",
        "severity": "major",
        "confidence": 0.91,
        "confidence_kind": "wired_real",
        "provenance": ["similarity:0.910", "match_type:high word overlap"],
        "evidence_refs": ["similarity:0.910", "match_type:high word overlap"],
        "routing_hint": "threshold_eligible",
        "limitations": null,
        "schema_version": 2
    }"#;

    /// The optional field POPULATED, and the `process_state` carve-out on the
    /// wire — the two shapes most likely to break silently.
    const RECORD_V2_PROCESS_STATE: &str = r#"{
        "id": "f4",
        "agent": "verification",
        "claim": "process_state",
        "severity": "minor",
        "confidence": 0.5,
        "confidence_kind": "real_native",
        "provenance": ["agent:verification (harness-gated, via proxy)"],
        "evidence_refs": ["agent:verification (harness-gated, via proxy)"],
        "routing_hint": "threshold_eligible",
        "limitations": "AI-detection: coarse signal — escalate by policy, never threshold on this number",
        "schema_version": 2
    }"#;

    #[test]
    fn previously_written_evidence_records_still_deserialize() {
        for (name, fixture) in
            [("RECORD_V2", RECORD_V2), ("RECORD_V2_PROCESS_STATE", RECORD_V2_PROCESS_STATE)]
        {
            let parsed: Result<EvidenceRecord, _> = serde_json::from_str(fixture);
            assert!(parsed.is_ok(), "{}", incompatible(&format!("{name} failed to parse: {parsed:?}")));
        }
    }

    /// The other half of the release-artifact rule. A retired fixture is kept as
    /// the evidence of a break, and this asserts it STAYS broken — so the record
    /// of why a version was bumped cannot quietly become false.
    ///
    /// If one of these starts parsing again, compatibility was restored: move it
    /// back into the live set above rather than deleting it here.
    ///
    /// # It guards the DECISION, not just the data
    ///
    /// Second-order value: this test fires if someone later makes `claim`
    /// OPTIONAL-WITH-DEFAULT. The retired fixture would parse again — and that
    /// is exactly §26.4's compatible-and-wrong case, where old records silently
    /// acquire a default claim they never carried, reintroducing §23.4's
    /// measured defect for cached data and invisibly. The assertion therefore
    /// protects the reasoning behind the required field, not merely the bytes.
    /// The CACHED REPORT's shape, pinned for the same reason `EvidenceRecord`'s
    /// is: the whole report is now parsed strictly, so a field rename breaks
    /// every cached report at upgrade time. `CACHED_REPORT_SCHEMA_VERSION` is in
    /// the cache key, so a bump makes stale entries MISS and recompute — but the
    /// bump only happens if someone notices, and this is what notices.
    ///
    /// A RELEASE ARTIFACT, not test data. Never edit or regenerate; append.
    const CACHED_REPORT_V2: &str = r#"{
        "verdict": "concern",
        "combined_confidence": 0.66,
        "findings": [{
            "severity": "major",
            "tier": "ai_assessed_moderate",
            "certainty_label": "AI-assessed, moderate confidence",
            "agent": "validation_maths",
            "claim": "manuscript_defect",
            "title": "statistical rule failed: missing effect size",
            "detail": "A p-value is reported without an accompanying effect size.",
            "confidence": 1.0,
            "provenance": ["rule:MissingEffectSize (MAJOR)"]
        }],
        "evidence": [],
        "checklist": [{
            "requirement": "required section: Abstract",
            "passed": true,
            "detail": "Abstract section found",
            "guideline_source": null
        }],
        "debate": {
            "rounds_run": 1, "converged": true, "overridden_by_constraint": false,
            "rejected_agents": [], "revised_agents": []
        },
        "disclaimer": "Certainty tiers: ..."
    }"#;

    #[test]
    fn previously_written_cached_reports_still_deserialize() {
        let parsed: Result<crate::report::PublishReadyReport, _> =
            serde_json::from_str(CACHED_REPORT_V2);
        assert!(
            parsed.is_ok(),
            "{}",
            incompatible(&format!("CACHED_REPORT_V2 failed to parse: {parsed:?}"))
        );
        let r = parsed.unwrap();
        assert_eq!(r.findings.len(), 1);
        assert_eq!(r.findings[0].title, "statistical rule failed: missing effect size");
        assert_eq!(r.checklist.len(), 1);
    }

    #[test]
    fn retired_fixtures_no_longer_parse() {
        for (name, fixture, retired_at) in [
            ("RETIRED_AT_V2_RECORD", RETIRED_AT_V2_RECORD, 2u32),
            ("RETIRED_AT_V2_RECORD_WITH_LIMITATIONS", RETIRED_AT_V2_RECORD_WITH_LIMITATIONS, 2),
        ] {
            let parsed: Result<EvidenceRecord, _> = serde_json::from_str(fixture);
            assert!(
                parsed.is_err(),
                "{name} parses again, but it is recorded as retired at \
                 CACHED_REPORT_SCHEMA_VERSION {retired_at}. If compatibility was restored, move it \
                 into the live fixture set; do not delete it."
            );
            assert!(
                CACHED_REPORT_SCHEMA_VERSION >= retired_at,
                "{name} is retired at version {retired_at}, which is ahead of \
                 CACHED_REPORT_SCHEMA_VERSION {CACHED_REPORT_SCHEMA_VERSION}"
            );
        }
    }

    #[test]
    fn previously_written_enum_values_still_deserialize() {
        let v = serde_json::Value::String;
        for s in HISTORICAL_AGENTS {
            let a: AgentKind = serde_json::from_value(v(s.to_string()))
                .unwrap_or_else(|e| panic!("{}", incompatible(&format!("AgentKind {s:?}: {e}"))));
            assert_eq!(agent_wire(a), *s, "{}", incompatible(&format!("AgentKind {s:?} no longer round-trips")));
        }
        for s in HISTORICAL_SEVERITIES {
            let x: FindingSeverity = serde_json::from_value(v(s.to_string()))
                .unwrap_or_else(|e| panic!("{}", incompatible(&format!("FindingSeverity {s:?}: {e}"))));
            assert_eq!(severity_wire(x), *s, "{}", incompatible(&format!("FindingSeverity {s:?} no longer round-trips")));
        }
        for s in HISTORICAL_CONFIDENCE_KINDS {
            let x: ConfidenceKind = serde_json::from_value(v(s.to_string()))
                .unwrap_or_else(|e| panic!("{}", incompatible(&format!("ConfidenceKind {s:?}: {e}"))));
            assert_eq!(confidence_kind_wire(x), *s, "{}", incompatible(&format!("ConfidenceKind {s:?} no longer round-trips")));
        }
        for s in HISTORICAL_ROUTING_HINTS {
            let x: RoutingHint = serde_json::from_value(v(s.to_string()))
                .unwrap_or_else(|e| panic!("{}", incompatible(&format!("RoutingHint {s:?}: {e}"))));
            assert_eq!(routing_hint_wire(x), *s, "{}", incompatible(&format!("RoutingHint {s:?} no longer round-trips")));
        }
    }

    fn finding(agent: AgentKind, confidence: f64, provenance: &[&str]) -> Finding {
        Finding {
            severity: FindingSeverity::Major,
            tier: CertaintyTier::AiAssessedModerate,
            certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
            agent,
            claim: ClaimKind::ManuscriptDefect,
            title: "t".into(),
            detail: "d".into(),
            confidence,
            provenance: provenance.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn validation_is_deterministic_never_escalate() {
        assert_eq!(confidence_kind(AgentKind::ValidationMaths), ConfidenceKind::Deterministic);
        assert_eq!(routing_hint(AgentKind::ValidationMaths), RoutingHint::NeverEscalate);
        assert!(limitations(AgentKind::ValidationMaths).is_none());
    }

    #[test]
    fn extraction_is_nosignal_heldout_with_limitation() {
        assert_eq!(confidence_kind(AgentKind::Extraction), ConfidenceKind::NoSignal);
        assert_eq!(routing_hint(AgentKind::Extraction), RoutingHint::HeldOut);
        assert!(limitations(AgentKind::Extraction).is_some(), "never silently routable");
    }

    #[test]
    fn verification_is_real_native_threshold_eligible() {
        assert_eq!(confidence_kind(AgentKind::Verification), ConfidenceKind::RealNative);
        assert_eq!(routing_hint(AgentKind::Verification), RoutingHint::ThresholdEligible);
        assert!(limitations(AgentKind::Verification).is_none());
    }

    #[test]
    fn plagiarism_is_wired_real_threshold_eligible() {
        assert_eq!(confidence_kind(AgentKind::Plagiarism), ConfidenceKind::WiredReal);
        assert_eq!(routing_hint(AgentKind::Plagiarism), RoutingHint::ThresholdEligible);
    }

    #[test]
    fn ai_detection_is_coarse_policy_eligible_with_limitation() {
        assert_eq!(confidence_kind(AgentKind::AiDetection), ConfidenceKind::DeliberatelyCoarse);
        assert_eq!(routing_hint(AgentKind::AiDetection), RoutingHint::PolicyEligible);
        assert!(limitations(AgentKind::AiDetection).is_some());
    }

    #[test]
    fn rag_is_wired_real_context_only() {
        assert_eq!(confidence_kind(AgentKind::Rag), ConfidenceKind::WiredReal);
        assert_eq!(routing_hint(AgentKind::Rag), RoutingHint::ContextOnly);
    }

    #[test]
    fn at_source_stamps_version_derives_refs_and_keeps_raw_confidence() {
        let rec = EvidenceRecord::at_source(
            "f1",
            AgentKind::Plagiarism,
            ClaimKind::ManuscriptDefect,
            FindingSeverity::Major,
            0.91,
            vec!["similarity:0.910".into(), "match_type:high word overlap".into(), "prose note".into()],
        );
        assert_eq!(rec.id, "f1");
        assert_eq!(rec.confidence, 0.91); // RAW, unrescaled
        assert_eq!(rec.confidence_kind, ConfidenceKind::WiredReal);
        assert_eq!(rec.routing_hint, RoutingHint::ThresholdEligible);
        assert_eq!(rec.schema_version, CACHED_REPORT_SCHEMA_VERSION);
        // evidence_refs = only the structured tags; "prose note" filtered out.
        assert_eq!(rec.evidence_refs, vec!["similarity:0.910", "match_type:high word overlap"]);
        assert!(rec.limitations.is_none());
    }

    #[test]
    fn from_finding_maps_a_real_plagiarism_finding() {
        let f = finding(AgentKind::Plagiarism, 0.91, &["similarity:0.910", "source:corpus"]);
        let rec = EvidenceRecord::from_finding(&f, "f3");
        assert_eq!(rec.confidence_kind, ConfidenceKind::WiredReal);
        assert_eq!(rec.routing_hint, RoutingHint::ThresholdEligible);
        assert_eq!(rec.confidence, 0.91);
        assert_eq!(rec.evidence_refs, vec!["similarity:0.910", "source:corpus"]);
        assert!(rec.limitations.is_none(), "plagiarism finding confidence is raw");
    }

    #[test]
    fn from_finding_attaches_rescale_note_for_soft_loop_agent() {
        // RAG's compiled finding carries the swarm-rescaled weight, not raw
        // rag_confidence — from_finding must say so, even though RAG has no base
        // limitation (WiredReal).
        let f = finding(AgentKind::Rag, 0.7, &["swarm:round-table"]);
        let rec = EvidenceRecord::from_finding(&f, "f4");
        assert_eq!(rec.confidence_kind, ConfidenceKind::WiredReal);
        let lim = rec.limitations.expect("rescale note attached");
        assert!(lim.contains("rescaled"), "must disclose the confidence is rescaled: {lim}");
    }

    #[test]
    fn from_finding_forces_nosignal_heldout_for_rejected() {
        // A gate-rejected finding may be ANY agent (here Verification, normally
        // RealNative/ThresholdEligible) — rejection voids it regardless.
        let f = finding(AgentKind::Verification, 0.0, &["swarm:rejected-before-debate"]);
        let rec = EvidenceRecord::from_finding(&f, "f5");
        assert_eq!(rec.confidence_kind, ConfidenceKind::NoSignal);
        assert_eq!(rec.routing_hint, RoutingHint::HeldOut);
        assert!(rec.limitations.unwrap().contains("rejected"));
    }
}
