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

/// Bump when the [`EvidenceRecord`] shape changes (the Evidence Store persists it).
pub const EVIDENCE_SCHEMA_VERSION: u32 = 1;

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
        severity: FindingSeverity,
        raw_confidence: f64,
        provenance: Vec<String>,
    ) -> Self {
        let evidence_refs =
            provenance.iter().filter(|p| is_structured_provenance(p)).cloned().collect();
        EvidenceRecord {
            id: id.into(),
            agent,
            severity,
            confidence: raw_confidence,
            confidence_kind: confidence_kind(agent),
            provenance,
            evidence_refs,
            routing_hint: routing_hint(agent),
            limitations: limitations(agent),
            schema_version: EVIDENCE_SCHEMA_VERSION,
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
            severity: finding.severity,
            confidence: finding.confidence,
            confidence_kind: confidence_kind_v,
            provenance: finding.provenance.clone(),
            evidence_refs,
            routing_hint: routing_hint_v,
            limitations: limitations_v,
            schema_version: EVIDENCE_SCHEMA_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::CertaintyTier;

    fn finding(agent: AgentKind, confidence: f64, provenance: &[&str]) -> Finding {
        Finding {
            severity: FindingSeverity::Major,
            tier: CertaintyTier::AiAssessedModerate,
            certainty_label: CertaintyTier::AiAssessedModerate.label().into(),
            agent,
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
            FindingSeverity::Major,
            0.91,
            vec!["similarity:0.910".into(), "match_type:high word overlap".into(), "prose note".into()],
        );
        assert_eq!(rec.id, "f1");
        assert_eq!(rec.confidence, 0.91); // RAW, unrescaled
        assert_eq!(rec.confidence_kind, ConfidenceKind::WiredReal);
        assert_eq!(rec.routing_hint, RoutingHint::ThresholdEligible);
        assert_eq!(rec.schema_version, EVIDENCE_SCHEMA_VERSION);
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
