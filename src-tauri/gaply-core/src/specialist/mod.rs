//! **§4.2's methodological specialists, and the three ways §4.2's own signature
//! is wrong about this code.**
//!
//! §4.2 gives every specialist one interface:
//!
//! ```text
//! fn assess(record: &AnalysisRecord, claims: &[Claim]) -> Vec<Opinion>
//! ```
//!
//! Measured against the tree on 15 Sep 2026, each of its three parts does not
//! hold, and each correction changes what a specialist can be built from.
//!
//! ## 1. `record: &AnalysisRecord` makes the record MANDATORY. It is absent on
//! every real run.
//!
//! No upload path in the app accepts an analysis file — every picker is
//! `pdf, docx, txt, md` — and the researcher corpus contains zero of them (see
//! [`crate::analysis`]). `Artifact::AnalysisRecord` is SUPPLIED, so a specialist
//! declaring it in `requires` is *not invoked*: written to §4.2's signature, the
//! frequentist specialist would never run at all. The record is therefore an
//! `Option` enrichment, and the specialist must have something to say without
//! it.
//!
//! ## 2. `claims: &[Claim]` is not enough, because of what the claims are
//!
//! With the scientific layer switched on across the six real manuscripts:
//! **123 claims, of which 109 (89%) are sentence fragments** — *"higher than the
//! untreated control."*, *"compared to unidirectional models."* — because the
//! extractor cuts at its trigger word and keeps the tail. And the cross-links a
//! methodological specialist would navigate are empty: **claim -> variable 0 of
//! 123, claim -> method 0 of 123, claim -> dataset 0 of 123**, claim -> statistic
//! 6 of 123. A specialist handed only `&[Claim]` would be reasoning over
//! subordinate clauses with no subject and no route to the analysis they rest
//! on.
//!
//! So [`SpecialistInput`] carries the whole [`ExtractionResult`] — the sections,
//! and the `StatClaim`s with their locations, which ARE well-formed and DO
//! carry resolvable spans. The scientific layer is an enrichment beside it, not
//! the substrate.
//!
//! ## 3. `-> Vec<Opinion>` cannot be honoured, and should not be
//!
//! [`crate::swarm::Opinion`] carries an [`crate::swarm::AgentKind`] — a closed
//! six-variant enum with **321 references across the tree** and a frontend wire
//! contract. Adding forty specialists to it is the change §11 D156 already
//! declined once for `CertaintyTier`, for the same reason. Specialists emit
//! [`SpecialistFinding`], and a later adapter maps a cluster's findings into one
//! `Opinion` if the round-table ever needs one — which is the shape
//! `swarm::adapters` already uses for the six lanes.
//!
//! # THE EVIDENCE GATE IS THE POINT, NOT THE CHECKS
//!
//! §4.3: *"An opinion emitted without its policy satisfied is rejected at the
//! gate with the reason recorded."* [`Specialist::assess`] is therefore not the
//! public entry point — [`run`] is, and it applies
//! [`crate::agent_graph::EvidencePolicy`] to every finding before any of them
//! escape. The rejections are RETURNED, not dropped: a gate that silently
//! swallows what it rejects is indistinguishable from a specialist that found
//! nothing, which is the failure this project has recorded three times.

pub mod frequentist;
pub mod ml;

use serde::{Deserialize, Serialize};

use crate::agent_graph::{Cluster, EvidencePolicy, EvidenceSource, TrustTier};
use crate::analysis::AnalysisRecord;
use crate::epistemic::EpistemicStatus;
use crate::extract::{ExtractionResult, Location};
use crate::report::FindingSeverity;
use crate::scientific_model::ScientificExtraction;

/// Everything a specialist may read. One struct so adding an input is a change
/// here and not at forty call sites.
pub struct SpecialistInput<'a> {
    pub extraction: &'a ExtractionResult,
    /// The derived scientific layer, when some agent in the graph asked for it.
    /// `None` means the extractor was never run, NOT that the paper has no
    /// claims — the same typed absence `ResearchState::science` carries.
    pub science: Option<&'a ScientificExtraction>,
    /// Parsed from files the researcher uploaded. `None` on every run today.
    pub analysis: Option<&'a AnalysisRecord>,
}

/// **A finding, with the sentence it rests on.**
///
/// A finding carrying only a value has to be trusted; one carrying its sentence
/// can be refuted by a reader in one glance. [`Finding::span`] is stored WHOLE —
/// clipping at display time is the same defect as never storing it, from the
/// reader's side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// The specialist that produced it, matching its `AgentSpec` id in the graph.
    pub specialist: String,
    /// Stable identifier for the CHECK, so a report can group and a test can pin.
    pub code: String,
    pub severity: FindingSeverity,
    pub status: EpistemicStatus,
    /// What was found, in one sentence.
    pub summary: String,
    /// The manuscript sentence or command this rests on, verbatim and whole.
    pub span: Option<String>,
    /// Where `span` came from, when it is a manuscript location.
    pub location: Option<Location>,
    /// Which kinds of evidence this finding carries. The gate counts these.
    pub evidence: Vec<EvidenceSource>,
    /// What the check could NOT determine. Required by policies that set
    /// `uncertainty_required`, and the honest half of every heuristic finding.
    pub uncertainty: Option<String>,
}

/// A finding the gate refused, and why. **Returned, never dropped.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    pub finding: Finding,
    pub reason: String,
}

/// What a specialist run produced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialistReport {
    pub specialist: String,
    pub admitted: Vec<Finding>,
    pub rejected: Vec<Rejected>,
    /// Set when the specialist declined to run at all — its inputs were absent.
    /// Distinct from "ran and found nothing", which the four zeros of the
    /// journal phase showed is the distinction that matters.
    pub not_applicable: Option<String>,
}

/// §4.2's interface, corrected — see the module header for each correction.
pub trait Specialist {
    /// Must equal this specialist's `AgentSpec` id in `data/agent_graph.json`.
    fn id(&self) -> &'static str;
    fn cluster(&self) -> Cluster;
    fn trust_tier(&self) -> TrustTier;
    /// The policy [`run`] enforces. It must be the SAME policy the graph
    /// declares — `specialists_match_the_graph` is the test that pins it.
    fn evidence_policy(&self) -> EvidencePolicy;
    /// **Does this specialist apply to this manuscript at all?**
    ///
    /// `None` means it does; `Some(reason)` means it declined, and the reason is
    /// carried into [`SpecialistReport::not_applicable`]. The distinction is the
    /// one the journal phase's four zeros turned on: *ran and found nothing* and
    /// *never ran* produce the same empty list and mean opposite things. Default
    /// is "applies", so a specialist that has no gate says nothing about one.
    fn applies_to(&self, _input: &SpecialistInput<'_>) -> Option<String> {
        None
    }
    /// Produce findings. **Not the entry point**: call [`run`], which gates.
    fn assess(&self, input: &SpecialistInput<'_>) -> Vec<Finding>;
}

/// **Run a specialist and apply its evidence policy.**
///
/// The gate is the deliverable, not the checks: §4.3 exists to stop a
/// sophisticated conclusion built on insufficient evidence, and a policy nothing
/// enforces is a comment. [`EvidencePolicy::admits`] is the one predicate shared
/// with the build-time validator, so the two cannot drift.
pub fn run(s: &dyn Specialist, input: &SpecialistInput<'_>) -> SpecialistReport {
    let policy = s.evidence_policy();
    let mut report = SpecialistReport { specialist: s.id().to_string(), ..Default::default() };

    if let Some(reason) = s.applies_to(input) {
        report.not_applicable = Some(reason);
        return report;
    }

    for f in s.assess(input) {
        if !policy.admits(&f.evidence) {
            let reason = format!(
                "carries {:?}, policy requires {} of {:?}",
                f.evidence, policy.min_sources, policy.permitted_sources
            );
            report.rejected.push(Rejected { finding: f, reason });
            continue;
        }
        if policy.citation_required && f.span.is_none() {
            report
                .rejected
                .push(Rejected { finding: f, reason: "policy requires a citation, none carried".into() });
            continue;
        }
        if policy.uncertainty_required && f.uncertainty.is_none() {
            report.rejected.push(Rejected {
                finding: f,
                reason: "policy requires a stated uncertainty, none carried".into(),
            });
            continue;
        }
        report.admitted.push(f);
    }
    report
}

/// The sentence at a location, whole. `None` when the location does not resolve.
///
/// **53% of the scientific layer's own spans name an ambiguous section kind**
/// (measured, six manuscripts), because `Location::paragraph` restarts at every
/// section and `paragraph_at` resolves to the FIRST section of a kind. Findings
/// here are anchored to `StatClaim` locations and section paragraphs, which are
/// produced and consumed by the same loop — but a caller must still handle the
/// `None`, and a finding whose span does not resolve carries no span rather than
/// a wrong one.
pub(crate) fn span_at(r: &ExtractionResult, loc: &Location) -> Option<String> {
    crate::extract::paragraph_at(r, loc).map(str::to_string)
}

/// The two specialists this phase ships.
pub fn shipped() -> Vec<Box<dyn Specialist>> {
    vec![
        Box::new(frequentist::FrequentistSpecialist),
        Box::new(ml::MachineLearningSpecialist),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_graph::{shipped_graph, Artifact};

    /// **Two declarations of one thing drift the first time only one is
    /// edited.** The graph file says what a specialist reads and what evidence
    /// it may cite; the code says the same. §11 D129's shape, so it is pinned
    /// rather than trusted.
    #[test]
    fn every_specialist_matches_its_node_in_the_shipped_graph() {
        let g = shipped_graph();
        for s in shipped() {
            let node = g
                .agents
                .iter()
                .find(|a| a.id.0 == s.id())
                .unwrap_or_else(|| panic!("`{}` has no node in data/agent_graph.json", s.id()));
            assert_eq!(node.cluster, s.cluster(), "{}: cluster", s.id());
            assert_eq!(node.trust_tier, s.trust_tier(), "{}: trust tier", s.id());
            assert_eq!(
                node.evidence_policy,
                s.evidence_policy(),
                "{}: the graph's evidence policy and the specialist's disagree — the gate \
                 would enforce one and the validator check the other",
                s.id()
            );
            assert_eq!(
                node.model,
                crate::agent_graph::ModelRequirement::None,
                "{}: a Tier-1 rule specialist calls no model",
                s.id()
            );
            // **Neither specialist REQUIRES anything, and that is the design.**
            // `AnalysisRecord` has no input (no upload path accepts one) and
            // `ScientificExtraction` is declined on measurement (§11 D165,
            // 5.9% precision against a 50% no-skill baseline). A `requires`
            // entry for either would mean the specialist never runs, or runs on
            // evidence the project has no grounds to trust.
            assert!(
                node.requires.is_empty(),
                "{}: requires {:?} — a specialist that cannot run without an absent or \
                 untrusted artifact is a specialist that does not run",
                s.id(),
                node.requires
            );
            assert!(
                node.optional.contains(&Artifact::ScientificExtraction),
                "{}: the declined layer belongs in `optional`, so reopening it (§11 D165) is \
                 a change to one graph field rather than a rewrite",
                s.id()
            );
        }
    }

    /// The gate is the deliverable. A finding carrying nothing the policy
    /// permits is REJECTED, and the rejection is returned with its reason —
    /// a gate that silently swallows what it rejects is indistinguishable from
    /// a specialist that found nothing.
    #[test]
    fn the_gate_rejects_an_unevidenced_finding_and_says_why() {
        struct Sloppy;
        impl Specialist for Sloppy {
            fn id(&self) -> &'static str {
                "sloppy"
            }
            fn cluster(&self) -> Cluster {
                Cluster::MethodologicalSoundness
            }
            fn trust_tier(&self) -> TrustTier {
                TrustTier::Rule
            }
            fn evidence_policy(&self) -> EvidencePolicy {
                EvidencePolicy {
                    min_sources: 1,
                    permitted_sources: vec![EvidenceSource::ManuscriptSpan],
                    citation_required: true,
                    uncertainty_required: true,
                }
            }
            fn assess(&self, _: &SpecialistInput<'_>) -> Vec<Finding> {
                let base = Finding {
                    specialist: "sloppy".into(),
                    code: "c".into(),
                    severity: FindingSeverity::Major,
                    status: EpistemicStatus::Detected,
                    summary: "something".into(),
                    span: Some("a sentence".into()),
                    location: None,
                    evidence: vec![EvidenceSource::ManuscriptSpan],
                    uncertainty: Some("what it could not tell".into()),
                };
                let mut no_evidence = base.clone();
                no_evidence.evidence.clear();
                let mut no_span = base.clone();
                no_span.span = None;
                let mut no_uncertainty = base.clone();
                no_uncertainty.uncertainty = None;
                vec![base, no_evidence, no_span, no_uncertainty]
            }
        }

        let r = crate::extract::extract_from_text("Introduction\n\nA paragraph.\n");
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let report = run(&Sloppy, &input);
        assert_eq!(report.admitted.len(), 1, "only the complete finding survives");
        assert_eq!(report.rejected.len(), 3, "and the other three are RETURNED, not dropped");
        let reasons: Vec<&str> = report.rejected.iter().map(|r| r.reason.as_str()).collect();
        assert!(reasons.iter().any(|r| r.contains("policy requires")), "{reasons:?}");
        assert!(reasons.iter().any(|r| r.contains("citation")), "{reasons:?}");
        assert!(reasons.iter().any(|r| r.contains("uncertainty")), "{reasons:?}");
    }

    /// `not_applicable` is a different answer from an empty `admitted`, and the
    /// four zeros of the journal phase are why.
    #[test]
    fn declining_to_run_is_distinguishable_from_finding_nothing() {
        let r = crate::extract::extract_from_text(
            "Introduction\n\nThe lake was sampled at three stations.\n",
        );
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let report = run(&ml::MachineLearningSpecialist, &input);
        assert!(report.admitted.is_empty());
        assert!(
            report.not_applicable.is_some(),
            "a limnology chapter is not an ML paper, and the report must say that rather \
             than look like a clean bill of health"
        );
    }
}
