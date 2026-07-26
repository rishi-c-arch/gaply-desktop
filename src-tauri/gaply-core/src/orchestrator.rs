//! Evidence Orchestrator / Confidence Manager (Box 1).
//!
//! A PURE function: no I/O, no proxy, no database. It reads a run's
//! [`EvidenceRecord`]s (from `report.evidence`) and decides, per finding, whether
//! to accept it locally, pass it through untouched, or escalate it to the large
//! LLM — honouring each record's [`ConfidenceKind`]/[`RoutingHint`] (Box 0's
//! frozen evidence-tier reality) and a swappable [`RoutingPolicy`].
//!
//! The dispatch is driven by `routing_hint` (data, not logic); all thresholds
//! live in the policy. Provider-blind.

use std::cmp::Ordering;

use crate::evidence::{ConfidenceKind, EvidenceRecord, RoutingAction, RoutingDecision, RoutingHint};
use crate::report::FindingSeverity;
use crate::swarm::AgentKind;

// The approved thresholds — named constants, the DefaultRoutingPolicy's values.
pub const CONF_VERIFY_ESCALATE: f64 = 0.75;
pub const PLAG_GRAY_LOW: f64 = 0.80;
pub const PLAG_GRAY_HIGH: f64 = 0.95;
pub const RAG_CONTEXT_MIN: f64 = 0.50;
pub const ESCALATION_BUDGET: usize = 8;

/// Escalation policy — separated from the Orchestrator so thresholds/rules are
/// swappable without touching dispatch. Implementations own the numbers.
pub trait RoutingPolicy {
    fn escalation_budget(&self) -> usize;
    /// For a `ThresholdEligible`/`PolicyEligible` record: is it an escalation
    /// CANDIDATE? (Branches on agent internally so the Orchestrator arm is uniform.)
    fn is_escalation_candidate(&self, rec: &EvidenceRecord) -> bool;
    /// For a `ContextOnly` (RAG) record: does its context qualify for attachment?
    fn context_qualifies(&self, rec: &EvidenceRecord) -> bool;
    /// The `Accept` reason when an eligible record is NOT a candidate.
    fn non_candidate_reason(&self, rec: &EvidenceRecord) -> String;
}

/// The approved default policy. Fields are public so a caller can tweak one
/// threshold (`DefaultRoutingPolicy { escalation_budget: 4, ..Default::default() }`).
pub struct DefaultRoutingPolicy {
    pub conf_verify_escalate: f64,
    pub plag_gray_low: f64,
    pub plag_gray_high: f64,
    pub rag_context_min: f64,
    pub escalation_budget: usize,
}

impl Default for DefaultRoutingPolicy {
    fn default() -> Self {
        Self {
            conf_verify_escalate: CONF_VERIFY_ESCALATE,
            plag_gray_low: PLAG_GRAY_LOW,
            plag_gray_high: PLAG_GRAY_HIGH,
            rag_context_min: RAG_CONTEXT_MIN,
            escalation_budget: ESCALATION_BUDGET,
        }
    }
}

impl RoutingPolicy for DefaultRoutingPolicy {
    fn escalation_budget(&self) -> usize {
        self.escalation_budget
    }

    fn is_escalation_candidate(&self, rec: &EvidenceRecord) -> bool {
        match rec.agent {
            // VERIFICATION — threshold-only on confidence. SIMPLIFIED from the
            // original doc's "Refuted OR Unknown OR low-confidence" rule to
            // threshold-only confidence, because Box 0's EvidenceRecord does not
            // carry a verdict enum — only severity and confidence. Severity DOES
            // already correlate with verdict (report.rs: Refuted->Major,
            // Unknown->Minor, Supported->Info, independent of confidence), so a
            // confident Refuted still surfaces as Major and, when it IS escalated,
            // is prioritised severity-first; adaptive escalation only spends the
            // cloud budget on genuinely UNCERTAIN verdicts (low confidence).
            AgentKind::Verification => rec.confidence < self.conf_verify_escalate,
            // PLAGIARISM — gray-zone band: ambiguous matches (real plagiarism vs.
            // legitimate quotation/boilerplate) get the LLM; clearly-a-match and
            // clearly-not are accepted locally.
            AgentKind::Plagiarism => {
                rec.confidence >= self.plag_gray_low && rec.confidence < self.plag_gray_high
            }
            // AI-DETECTION — by SEVERITY (a flagged concern), NEVER the coarse
            // confidence number. Competes for budget like everything else.
            AgentKind::AiDetection => {
                matches!(rec.severity, FindingSeverity::Critical | FindingSeverity::Major)
            }
            // These agents never carry an escalation hint (RAG=ContextOnly,
            // Validation=NeverEscalate, Extraction=HeldOut) so this is unreachable;
            // explicit (no wildcard) so a 7th agent forces a decision here.
            AgentKind::Rag | AgentKind::ValidationMaths | AgentKind::Extraction => false,
        }
    }

    fn context_qualifies(&self, rec: &EvidenceRecord) -> bool {
        rec.confidence >= self.rag_context_min
    }

    fn non_candidate_reason(&self, rec: &EvidenceRecord) -> String {
        match rec.agent {
            AgentKind::Verification => format!(
                "verification confidence {:.2} ≥ {:.2} — accepted locally",
                rec.confidence, self.conf_verify_escalate
            ),
            AgentKind::Plagiarism => {
                if rec.confidence >= self.plag_gray_high {
                    format!(
                        "similarity {:.2} ≥ {:.2} — clear match, accepted",
                        rec.confidence, self.plag_gray_high
                    )
                } else {
                    format!(
                        "similarity {:.2} < {:.2} — no concern, accepted",
                        rec.confidence, self.plag_gray_low
                    )
                }
            }
            AgentKind::AiDetection => {
                "ai-detection not flagged as a concern — accepted (coarse signal)".to_string()
            }
            AgentKind::Rag | AgentKind::ValidationMaths | AgentKind::Extraction => {
                "accepted".to_string()
            }
        }
    }
}

/// RAG context contributed to the escalation payloads (a shared POOL, not a
/// per-finding map: current RAG is one aggregate opinion, so its context is
/// manuscript-level, not finding-specific).
#[derive(Debug, Clone, PartialEq)]
pub struct ContextAttachment {
    pub source_finding_id: String,
    pub refs: Vec<String>,
}

/// The Orchestrator's output for one run.
#[derive(Debug, Clone, PartialEq)]
pub struct OrchestratorOutput {
    /// One decision per input record, in INPUT ORDER (== report.evidence / f{N}).
    pub decisions: Vec<(String, RoutingDecision)>,
    /// finding_ids selected for escalation (≤ budget), in priority order.
    pub escalate: Vec<String>,
    /// RAG context ≥ threshold, for Box 2 to attach to escalation payloads.
    pub context_pool: Vec<ContextAttachment>,
    /// Eligible-but-unselected (budget exhausted) — honesty counter, never a
    /// silent drop (mirrors `findings_omitted`).
    pub omitted_count: usize,
}

/// Trustworthiness rank for budget priority (lower = more trustworthy signal, so
/// escalated first). Deterministic/NoSignal never reach candidate selection but
/// are ranked last for totality.
fn kind_rank(k: ConfidenceKind) -> u8 {
    match k {
        ConfidenceKind::RealNative => 0,
        ConfidenceKind::WiredReal => 1,
        ConfidenceKind::DeliberatelyCoarse => 2,
        ConfidenceKind::Deterministic => 3,
        ConfidenceKind::NoSignal => 4,
    }
}

/// Route a run's evidence into per-finding decisions + an escalation set.
/// PURE: no I/O. `decisions` is 1:1 with `evidence` in input order.
pub fn route_evidence(evidence: &[EvidenceRecord], policy: &dyn RoutingPolicy) -> OrchestratorOutput {
    let budget = policy.escalation_budget();
    let mut decisions: Vec<Option<RoutingDecision>> = vec![None; evidence.len()];
    let mut context_pool: Vec<ContextAttachment> = Vec::new();
    let mut candidates: Vec<usize> = Vec::new();

    // Pass 1: dispatch on routing_hint. Terminal hints decide now; escalation
    // hints either become candidates (budget decides) or accept immediately.
    for (i, rec) in evidence.iter().enumerate() {
        match rec.routing_hint {
            RoutingHint::NeverEscalate => {
                decisions[i] = Some(passthrough(rec, "pass-through — deterministic (mathematically certain)"));
            }
            RoutingHint::HeldOut => {
                decisions[i] = Some(passthrough(rec, "held out — no confidence signal"));
            }
            RoutingHint::ContextOnly => {
                if policy.context_qualifies(rec) {
                    context_pool.push(ContextAttachment {
                        source_finding_id: rec.id.clone(),
                        refs: rec.evidence_refs.clone(),
                    });
                    decisions[i] =
                        Some(passthrough(rec, "context-only — attached to escalation payloads"));
                } else {
                    decisions[i] = Some(passthrough(
                        rec,
                        "context-only — retrieval below threshold, not attached",
                    ));
                }
            }
            RoutingHint::ThresholdEligible | RoutingHint::PolicyEligible => {
                if policy.is_escalation_candidate(rec) {
                    candidates.push(i); // decided in Pass 2 (budget)
                } else {
                    decisions[i] = Some(RoutingDecision {
                        action: RoutingAction::Accept,
                        reason: policy.non_candidate_reason(rec),
                        priority: rec.severity.rank(),
                    });
                }
            }
        }
    }

    // Pass 2: budget selection. Priority (ascending = highest first):
    //   severity.rank -> kind trustworthiness -> confidence (lower/more-uncertain
    //   first) -> id (deterministic).
    candidates.sort_by(|&a, &b| {
        let (ra, rb) = (&evidence[a], &evidence[b]);
        ra.severity
            .rank()
            .cmp(&rb.severity.rank())
            .then(kind_rank(ra.confidence_kind).cmp(&kind_rank(rb.confidence_kind)))
            .then(ra.confidence.partial_cmp(&rb.confidence).unwrap_or(Ordering::Equal))
            .then(ra.id.cmp(&rb.id))
    });

    let mut escalate: Vec<String> = Vec::new();
    let mut omitted_count = 0usize;
    for (pos, &idx) in candidates.iter().enumerate() {
        let rec = &evidence[idx];
        if pos < budget {
            escalate.push(rec.id.clone());
            decisions[idx] = Some(RoutingDecision {
                action: RoutingAction::Escalate,
                reason: format!(
                    "escalated — {:?} candidate, priority sev {} / kind {}",
                    rec.agent,
                    rec.severity.rank(),
                    kind_rank(rec.confidence_kind)
                ),
                priority: rec.severity.rank(),
            });
        } else {
            omitted_count += 1;
            decisions[idx] = Some(RoutingDecision {
                action: RoutingAction::Accept,
                reason: format!(
                    "eligible but not selected — escalation budget ({budget}) exhausted"
                ),
                priority: rec.severity.rank(),
            });
        }
    }

    let decisions = evidence
        .iter()
        .zip(decisions)
        .map(|(rec, d)| (rec.id.clone(), d.expect("every record receives a decision")))
        .collect();

    OrchestratorOutput { decisions, escalate, context_pool, omitted_count }
}

fn passthrough(rec: &EvidenceRecord, reason: &str) -> RoutingDecision {
    RoutingDecision {
        action: RoutingAction::PassThrough,
        reason: reason.to_string(),
        priority: rec.severity.rank(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::EvidenceRecord;

    fn rec(id: &str, agent: AgentKind, sev: FindingSeverity, conf: f64) -> EvidenceRecord {
        EvidenceRecord::at_source(id, agent, sev, conf, vec!["source:test".into()])
    }

    fn decision<'a>(out: &'a OrchestratorOutput, id: &str) -> &'a RoutingDecision {
        &out.decisions.iter().find(|(fid, _)| fid == id).expect("id present").1
    }

    #[test]
    fn validation_always_passes_through() {
        let ev = vec![rec("f1", AgentKind::ValidationMaths, FindingSeverity::Critical, 1.0)];
        let out = route_evidence(&ev, &DefaultRoutingPolicy::default());
        let d = decision(&out, "f1");
        assert_eq!(d.action, RoutingAction::PassThrough);
        assert!(d.reason.contains("deterministic"));
        assert!(out.escalate.is_empty());
    }

    #[test]
    fn extraction_held_out_with_distinct_reason() {
        let ev = vec![rec("f1", AgentKind::Extraction, FindingSeverity::Info, 0.9)];
        let out = route_evidence(&ev, &DefaultRoutingPolicy::default());
        let d = decision(&out, "f1");
        assert_eq!(d.action, RoutingAction::PassThrough);
        assert!(d.reason.contains("no confidence signal"), "reason: {}", d.reason);
        assert!(!d.reason.contains("deterministic"), "must NOT read like Validation");
        assert!(out.escalate.is_empty());
    }

    #[test]
    fn rag_never_escalated_but_pooled_above_threshold() {
        let ev = vec![
            rec("f1", AgentKind::Rag, FindingSeverity::Info, 0.70), // >= 0.50 → pooled
            rec("f2", AgentKind::Rag, FindingSeverity::Info, 0.30), // < 0.50  → not pooled
        ];
        let out = route_evidence(&ev, &DefaultRoutingPolicy::default());
        assert!(out.escalate.is_empty(), "RAG is never an escalation target");
        // Only the above-threshold RAG record is in the context pool.
        assert_eq!(out.context_pool.len(), 1);
        assert_eq!(out.context_pool[0].source_finding_id, "f1");
        assert_eq!(out.context_pool[0].refs, vec!["source:test"]);
        assert!(decision(&out, "f1").reason.contains("attached"));
        assert!(decision(&out, "f2").reason.contains("below threshold"));
    }

    #[test]
    fn verification_threshold_routing() {
        let ev = vec![
            rec("lo", AgentKind::Verification, FindingSeverity::Minor, 0.60), // < 0.75 → escalate
            rec("hi", AgentKind::Verification, FindingSeverity::Major, 0.90), // ≥ 0.75 → accept
        ];
        let out = route_evidence(&ev, &DefaultRoutingPolicy::default());
        assert_eq!(decision(&out, "lo").action, RoutingAction::Escalate);
        assert_eq!(decision(&out, "hi").action, RoutingAction::Accept);
        assert!(decision(&out, "hi").reason.contains("accepted locally"));
        assert_eq!(out.escalate, vec!["lo"]);
    }

    #[test]
    fn plagiarism_gray_band() {
        let ev = vec![
            rec("gray", AgentKind::Plagiarism, FindingSeverity::Major, 0.85), // band → escalate
            rec("clear", AgentKind::Plagiarism, FindingSeverity::Major, 0.97), // ≥ HIGH → accept
            rec("none", AgentKind::Plagiarism, FindingSeverity::Minor, 0.60), // < LOW → accept
        ];
        let out = route_evidence(&ev, &DefaultRoutingPolicy::default());
        assert_eq!(decision(&out, "gray").action, RoutingAction::Escalate);
        assert_eq!(decision(&out, "clear").action, RoutingAction::Accept);
        assert!(decision(&out, "clear").reason.contains("clear match"));
        assert_eq!(decision(&out, "none").action, RoutingAction::Accept);
        assert!(decision(&out, "none").reason.contains("no concern"));
    }

    #[test]
    fn ai_detection_by_severity_not_number() {
        let ev = vec![
            rec("flag", AgentKind::AiDetection, FindingSeverity::Major, 0.60), // concern → candidate
            rec("info", AgentKind::AiDetection, FindingSeverity::Info, 0.60),  // not a concern → accept
        ];
        let out = route_evidence(&ev, &DefaultRoutingPolicy::default());
        assert_eq!(decision(&out, "flag").action, RoutingAction::Escalate);
        assert_eq!(decision(&out, "info").action, RoutingAction::Accept);
        assert!(decision(&out, "info").reason.contains("coarse signal"));
    }

    #[test]
    fn budget_selects_top_n_and_omits_rest_honestly() {
        // Budget 2, three candidates. Priority: severity, then kind
        // (RealNative < DeliberatelyCoarse), then confidence.
        let policy = DefaultRoutingPolicy { escalation_budget: 2, ..Default::default() };
        let ev = vec![
            // Major + RealNative (verification) → highest priority.
            rec("a", AgentKind::Verification, FindingSeverity::Major, 0.60),
            // Major + DeliberatelyCoarse (ai) → same severity, worse kind → 2nd.
            rec("b", AgentKind::AiDetection, FindingSeverity::Major, 0.60),
            // Minor (verification) → lowest severity → omitted under budget 2.
            rec("c", AgentKind::Verification, FindingSeverity::Minor, 0.60),
        ];
        let out = route_evidence(&ev, &policy);
        // Exactly the top 2 escalate, in priority order (a before b).
        assert_eq!(out.escalate, vec!["a", "b"]);
        assert_eq!(decision(&out, "a").action, RoutingAction::Escalate);
        assert_eq!(decision(&out, "b").action, RoutingAction::Escalate);
        // c was eligible but cut — accepted honestly, counted.
        assert_eq!(decision(&out, "c").action, RoutingAction::Accept);
        assert!(decision(&out, "c").reason.contains("budget (2) exhausted"));
        assert_eq!(out.omitted_count, 1);
    }

    #[test]
    fn decisions_are_one_to_one_in_input_order() {
        let ev = vec![
            rec("f1", AgentKind::ValidationMaths, FindingSeverity::Critical, 1.0),
            rec("f2", AgentKind::Verification, FindingSeverity::Minor, 0.6),
            rec("f3", AgentKind::Extraction, FindingSeverity::Info, 0.9),
        ];
        let out = route_evidence(&ev, &DefaultRoutingPolicy::default());
        assert_eq!(out.decisions.len(), ev.len());
        let ids: Vec<&str> = out.decisions.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["f1", "f2", "f3"], "same order as input");
    }
}
