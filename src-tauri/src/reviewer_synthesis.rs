//! Box 4 — Reviewer Synthesis, app-crate integration (Stage 1: additive shadow).
//!
//! The PURE synthesis logic (aggregate / build request / gate / synthesize)
//! lives in `gaply_core::reviewer_agent`. This module is the I/O bridge: it
//! assembles a [`ReviewerInput`] by JOINING the Evidence Store's per-finding
//! verdicts (`evidence_by_run`) with the compiled report's presentation fields
//! (titles / checklist / journal / verdict), then drives the pure pipeline.
//!
//! # D1, used correctly
//! `verified_findings` and `local_findings` both come from the SAME store query
//! (`evidence_by_run`). They are NOT two queries and NOT partitioned here — the
//! flat `ReviewerInput.findings` carries every row, and consumers distinguish
//! escalated from local purely via `ReviewerFinding::verification_state()`
//! (the single partition path).
//!
//! # Join
//! Store rows carry `finding_id` (`f{N}`); the report carries titles positionally
//! (`report.findings[N-1]`). We join by PARSING `N` out of the id, so the join is
//! robust to store row order — not by assuming the vecs line up.
//!
//! # Stage 1 (this commit): ADDITIVE SHADOW
//! `run_shadow_synthesis` runs ALONGSIDE the wholesale reviewer; its letter is
//! returned as `PublishReadyOutcome.shadow_reviewer` for comparison. The
//! wholesale reviewer remains authoritative. Stage 2 (switch) and Stage 3
//! (remove wholesale) are separate future decisions, gated on the gaply-proxy
//! escalate endpoint + reviewer-quality spike landing.

use serde_json::Value;

use gaply_core::evidence::ClaimKind;
use gaply_core::evidence_store::evidence_by_run;
use gaply_core::report::{FindingSeverity, PublishReadyReport};
use gaply_core::reviewer_agent::{LaneExamination, VerdictWithheld};
use gaply_core::swarm::AgentKind;
use gaply_core::reviewer_agent::{
    aggregate_reviewer_verdict, build_reviewer_request, gate_reviewer_narrative,
    synthesize_reviewer_letter, ReviewerEvaluation, ReviewerFinding, ReviewerInput, ReviewerMeta,
    ReviewerNarrative, TargetJournal, VerdictAggregation,
};
use gaply_core::verify_agent::ProxyClient;
use gaply_core::{Database, GaplyError};

/// Parse the positional index out of an `f{N}` finding id (1-based → 0-based).
fn parse_finding_index(id: &str) -> Option<usize> {
    id.strip_prefix('f')?.parse::<usize>().ok().and_then(|n| n.checked_sub(1))
}

/// Parse a stored `AgentKind` back from the store's TEXT column.
///
/// `None` on failure, and that is TYPED ABSENCE rather than a fallback: unlike
/// severity, an unrecognised family has NO safe default — it can be assumed
/// neither eligible nor ineligible (§21). Empty for rows written before
/// migration 13.
///
/// **This function is the fix for the loss that started ARCHITECTURE_TRACE
/// §22:** `AgentKind` was written as an enum, persisted as a string by
/// `enum_text`, and never restored here.
fn parse_agent(s: &str) -> Option<AgentKind> {
    serde_json::from_value(Value::String(s.to_string())).ok()
}

/// Parse a stored `ClaimKind`. `None` for rows written before migration 13,
/// whose claim is genuinely unrecorded — never defaulted to `ManuscriptDefect`,
/// which would assert something that was never stored (§26.4).
fn parse_claim(s: &str) -> Option<ClaimKind> {
    serde_json::from_value(Value::String(s.to_string())).ok()
}

/// Parse a stored severity string (snake_case, written from `FindingSeverity`)
/// back to the enum. Defensive fallback to `Info` — stored values are always
/// valid, so this only guards against corruption.
fn parse_severity(s: &str) -> FindingSeverity {
    serde_json::from_value(Value::String(s.to_string())).unwrap_or(FindingSeverity::Info)
}

/// Assemble the flat [`ReviewerInput`] for a run: JOIN the Evidence Store's
/// decided verdicts with the report's presentation fields.
///
/// The store is the SOLE source of per-finding verdicts (`report.evidence` is a
/// pre-escalation snapshot with none); the report supplies titles, checklist,
/// journal metadata and the overall verdict.
pub fn assemble_reviewer_input(
    db: &Database,
    run_id: &str,
    report: &PublishReadyReport,
    journal: &TargetJournal,
    supplementary: &[Value],
    withheld: Option<VerdictWithheld>,
    lanes: LaneExamination,
) -> Result<ReviewerInput, GaplyError> {
    let rows = evidence_by_run(db, run_id)?;
    let report_findings = &report.findings;

    let findings = rows
        .into_iter()
        .map(|row| {
            // Join the title by PARSED f{N} index (not row position).
            // TYPED: a missing index still yields "" (the join is positional and
            // a store row may outlive its report entry), but a PRESENT finding
            // can no longer have a non-string title silently become "".
            let title = parse_finding_index(&row.finding_id)
                .and_then(|idx| report_findings.get(idx))
                .map(|f| f.title.clone())
                .unwrap_or_default();
            ReviewerFinding {
                severity: parse_severity(&row.severity),
                confidence: row.confidence,
                title,
                evidence_refs: row.evidence_refs,
                verdict: row.llm_verdict,
                verified: row.verified,
                gate_flags: row.gate_flags.unwrap_or_default(),
                id: row.finding_id,
                agent: parse_agent(&row.agent),
                claim: parse_claim(&row.claim),
            }
        })
        .collect();

    let checklist = report
        .checklist
        .iter()
        .map(|c| serde_json::to_value(c).unwrap_or(Value::Null))
        .collect();
    let metadata = ReviewerMeta {
        run_id: run_id.to_string(),
        overall_verdict: report.verdict.clone(),
        combined_confidence: report.combined_confidence,
    };

    Ok(ReviewerInput {
        withheld,
        lanes,
        findings,
        checklist,
        supplementary: supplementary.to_vec(),
        journal: journal.clone(),
        metadata,
    })
}

/// Result of a shadow synthesis run. Carries the STRUCTURAL signals the
/// comparison harness needs — `narrative_available` is set at the point the
/// narrative is (or isn't) obtained, never inferred from the letter's body text.
pub struct ShadowOutcome {
    /// Carried so the harness can OBSERVE the withheld state rather than reading
    /// the letter's inert `Unknown`/0.0 filler as a real verdict.
    pub withheld: Option<VerdictWithheld>,
    pub letter: ReviewerEvaluation,
    pub aggregation: VerdictAggregation,
    /// True iff the narrative came from a real proxy response (not the honest
    /// degraded placeholder). The harness reads THIS, not `letter.body`.
    pub narrative_available: bool,
    /// Number of finding ids actually sent (issue-coverage denominator).
    pub findings_sent: usize,
}

/// Run the Stage-1 shadow synthesis end to end: assemble → deterministic verdict
/// → build request → (proxy narrative or honest degradation) → gated letter.
///
/// The verdict is deterministic, so a letter is produced even when `proxy` is
/// `None` or the call fails — only the narrative degrades (and `narrative_available`
/// records exactly that). `letter.available` stays true (the verdict is real).
pub fn run_shadow_synthesis(
    db: &Database,
    proxy: Option<&dyn ProxyClient>,
    run_id: &str,
    report: &PublishReadyReport,
    journal: &TargetJournal,
    supplementary: &[Value],
    withheld: Option<VerdictWithheld>,
    lanes: LaneExamination,
) -> Result<ShadowOutcome, GaplyError> {
    let input =
        assemble_reviewer_input(db, run_id, report, journal, supplementary, withheld, lanes)?;

    // Deterministic verdict — from the already-decided severities, no LLM.
    let aggregation = aggregate_reviewer_verdict(&input);

    // The SOLE payload/SentIds pair-producer (D4 closed by construction).
    let request = build_reviewer_request(&input);
    let findings_sent = request.sent_finding_count();

    // narrative_available is set HERE, from the actual event — never parsed back
    // out of the letter body.
    let (narrative, narrative_available) = match proxy {
        Some(p) => match p.verify(request.payload()) {
            Ok(resp) => (gate_reviewer_narrative(&resp, &request), true),
            Err(e) => {
                tracing::warn!(error = %e, "shadow synthesis cloud narrative failed; degrading");
                (ReviewerNarrative::unavailable(), false)
            }
        },
        None => (ReviewerNarrative::unavailable(), false),
    };

    let letter = synthesize_reviewer_letter(&aggregation, narrative);
    Ok(ShadowOutcome { withheld, letter, aggregation, narrative_available, findings_sent })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::evidence::EvidenceRecord;
    use gaply_core::evidence_store::{evidence_persist, evidence_record_escalation, EscalationOutcome};
    use gaply_core::report::FindingSeverity as Sev;
    use gaply_core::reviewer_agent::Recommendation;
    use gaply_core::swarm::AgentKind;
    use gaply_core::verify_agent::MockProxyClient;
    use serde_json::json;

    /// Every denominator lane examined something — the normal case, so a test
    /// that is not ABOUT lane state is not silently exercising the starved path.
    fn all_examined() -> LaneExamination {
        LaneExamination {
            verification_examined: true,
            validation_examined: true,
            plagiarism_examined: true,
            ai_detection_examined: true,
            extraction_examined: true,
        }
    }

    fn journal() -> TargetJournal {
        TargetJournal { name: "Nature".into(), quartile: "Q1".into() }
    }

    // Build a minimal report JSON whose findings line up positionally with the
    // f{N} ids we persist.
    /// A minimal well-formed report. Deserialized from the same JSON the fixture
    /// always built — which only became possible with PublishReadyReport's
    /// Deserialize derive, and is itself the smallest demonstration of it.
    fn report(titles: &[&str]) -> PublishReadyReport {
        let findings: Vec<Value> = titles
            .iter()
            .map(|t| {
                json!({
                    "severity": "minor",
                    "tier": "ai_assessed_moderate",
                    "certainty_label": "AI-assessed, moderate confidence",
                    "agent": "extraction",
                    "claim": "manuscript_defect",
                    "title": t,
                    "detail": "d",
                    "confidence": 0.5,
                    "provenance": ["signal:test"],
                })
            })
            .collect();
        serde_json::from_value(json!({
            "verdict": "concern",
            "combined_confidence": 0.66,
            "findings": findings,
            "evidence": [],
            "checklist": [
                { "requirement": "IMRaD present", "passed": true, "detail": "ok",
                  "guideline_source": null }
            ],
            "debate": { "rounds_run": 1, "converged": true,
                        "overridden_by_constraint": false,
                        "rejected_agents": [], "revised_agents": [] },
            "disclaimer": "d",
        }))
        .expect("the fixture must be a well-formed report")
    }

    fn record(id: &str, agent: AgentKind, severity: Sev, confidence: f64) -> EvidenceRecord {
        EvidenceRecord::at_source(
            id.to_string(),
            agent,
            ClaimKind::ManuscriptDefect,
            severity,
            confidence,
            vec![],
        )
    }

    /// THE property whose absence started ARCHITECTURE_TRACE §22.
    ///
    /// `AgentKind` was written as an enum, persisted as a string by `enum_text`,
    /// and never restored — so the aggregator received `agent` as a `String` it
    /// never inspected, and family identity was not actionable. Covering
    /// `ClaimKind` alone would ship that restoration untested, which is why both
    /// enums are exercised here, over EVERY variant of each.
    ///
    /// Asserts VALUE PRESERVATION, not representational identity. The DB form is
    /// a snake_case string via `enum_text`, so asserting bit-for-bit sameness at
    /// every boundary would assert something false; what must hold is that the
    /// value written is the value that arrives.
    #[test]
    fn producer_and_claim_survive_the_write_persist_restore_round_trip() {
        use gaply_core::evidence::ClaimKind as CK;
        const AGENTS: &[AgentKind] = &[
            AgentKind::Extraction,
            AgentKind::ValidationMaths,
            AgentKind::AiDetection,
            AgentKind::Plagiarism,
            AgentKind::Rag,
            AgentKind::Verification,
        ];
        const CLAIMS: &[CK] = &[CK::ProcessState, CK::AuthorshipSignal, CK::ManuscriptDefect];

        for (i, agent) in AGENTS.iter().enumerate() {
            for (j, claim) in CLAIMS.iter().enumerate() {
                let db = Database::in_memory().unwrap();
                let run_id = format!("run-rt-{i}-{j}");
                let id = "f1";
                let written = EvidenceRecord::at_source(
                    id.to_string(),
                    *agent,
                    *claim,
                    Sev::Major,
                    0.5,
                    vec!["source:test".into()],
                );
                evidence_persist(&db, &run_id, &[written], gaply_core::now_epoch()).unwrap();

                let input =
                    assemble_reviewer_input(&db, &run_id, &report(&["t"]), &journal(), &[], None, all_examined()).unwrap();
                assert_eq!(input.findings.len(), 1, "one record in, one finding out");
                let got = &input.findings[0];

                assert_eq!(
                    got.agent,
                    Some(*agent),
                    "PRODUCER lost across write -> persist -> restore for {agent:?}"
                );
                assert_eq!(
                    got.claim,
                    Some(*claim),
                    "CLAIM lost across write -> persist -> restore for {claim:?}"
                );
            }
        }
    }

    /// A row written before migration 13 carries an empty `claim` (the column's
    /// DEFAULT ''). It must restore as `None` — typed absence — never as
    /// `ManuscriptDefect`, which would assert something that was never stored
    /// (§26.4). Asserted at the mapping, since the app crate has no SQL escape
    /// hatch to age a row; the round-trip test above covers the populated path.
    #[test]
    fn an_unrecorded_claim_restores_as_absent_not_defaulted() {
        assert_eq!(parse_claim(""), None, "migration 13's default is UNKNOWN, not a claim");
        assert_eq!(parse_claim("not_a_claim"), None, "an unrecognised claim is absent");
        assert_eq!(parse_claim("process_state"), Some(ClaimKind::ProcessState));
        // The producer has no safe default either — §21: an unknown family can be
        // assumed neither eligible nor ineligible.
        assert_eq!(parse_agent(""), None);
        assert_eq!(parse_agent("grammar"), None, "an unknown family is absent, not guessed");
        assert_eq!(parse_agent("verification"), Some(AgentKind::Verification));
    }

    /// SYNTHETIC. Constructed to exercise the claim filter across the shapes it
    /// must distinguish — **not drawn from any run**, and it must not be read as
    /// evidence about one.
    ///
    /// The three findings are chosen so each carve-out is load-bearing: one
    /// `AuthorshipSignal` Major (removing it must drop the Major tier to zero),
    /// one `ProcessState` Minor and one `ManuscriptDefect` Minor (removing only
    /// the first must leave exactly one Minor, below
    /// `MINOR_REVISION_THRESHOLD`). A filter that excluded neither, either, or
    /// both incorrectly changes the outcome.
    ///
    /// This test was `pr1_identity_does_not_change_the_recommendation`, and it
    /// asserted `MajorRevision` with `minor.total() == 2` — noting in its own
    /// comment that "the process claim STILL COUNTS, which is precisely what
    /// PR-3 changes". **PR-3 flipped it, which is the pin working**: the change
    /// was detected by an assertion written before it, not discovered after.
    ///
    /// An earlier version of this comment said the fixture "mirrors run 22's
    /// decisive shape". That was false — the shape came from `report:v2:21`, a
    /// different manuscript (ARCHITECTURE_TRACE §23.4.1). A regression test must
    /// not imply provenance it does not have, so it now claims none.
    #[test]
    fn synthetic_pr3_claim_filter_case() {
        use gaply_core::reviewer_agent::aggregate_reviewer_verdict;
        let db = Database::in_memory().unwrap();
        let run_id = "run-pr3";
        let records = vec![
            EvidenceRecord::at_source("f1".to_string(), AgentKind::AiDetection, ClaimKind::AuthorshipSignal, Sev::Major, 0.6, vec![]),
            EvidenceRecord::at_source("f2".to_string(), AgentKind::Verification, ClaimKind::ProcessState, Sev::Minor, 0.5, vec![]),
            EvidenceRecord::at_source("f3".to_string(), AgentKind::Extraction, ClaimKind::ManuscriptDefect, Sev::Minor, 0.5, vec![]),
        ];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();
        let input =
            assemble_reviewer_input(&db, run_id, &report(&["a", "b", "c"]), &journal(), &[], None, all_examined()).unwrap();
        let agg = aggregate_reviewer_verdict(&input);

        // BEFORE PR-3 this was MajorRevision at 0.30, driven entirely by f1.
        assert_eq!(
            agg.verdict.recommendation(),
            Some(Recommendation::Accept),
            "the AuthorshipSignal Major and the ProcessState Minor must both stop counting"
        );
        assert_eq!(agg.breakdown.major.total(), 0, "f1 is not counted");
        assert_eq!(agg.breakdown.minor.total(), 1, "only the ManuscriptDefect Minor counts");

        // ATTRIBUTED, not merely dropped (§4.14).
        let ids: Vec<&str> = agg.excluded.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["f1", "f2"]);
        assert!(agg.excluded[0].reason.contains("authorship"));
        assert!(agg.excluded[1].reason.contains("Gaply's own execution"));

        // AND IT MUST REACH THE LETTER. §26.11's lesson: asserting `agg.excluded`
        // alone would be the right property at the wrong boundary — the UI reads
        // the letter, not the aggregation, so a resolver that excludes correctly
        // and never says so would pass.
        use gaply_core::reviewer_agent::{synthesize_reviewer_letter, ReviewerNarrative};
        let letter = synthesize_reviewer_letter(
            &agg,
            ReviewerNarrative { body: "b".into(), issues: vec![], warnings: vec![] },
        );
        let surfaced: Vec<&String> = letter
            .warnings
            .iter()
            .filter(|w| w.starts_with("excluded from the recommendation:"))
            .collect();
        assert_eq!(surfaced.len(), 2, "both exclusions must reach the letter: {:?}", letter.warnings);
        assert!(surfaced[0].contains("f1") && surfaced[0].contains("authorship"));
        assert!(surfaced[1].contains("f2") && surfaced[1].contains("Gaply's own execution"));
    }

    /// The resolver across the WHOLE severity x claim space.
    ///
    /// # What this establishes, and what it does not
    ///
    /// **Establishes:** the resolver behaves as SPECIFIED across every
    /// combination — an eligible claim contributes its severity, an ineligible
    /// one contributes nothing and is attributed.
    ///
    /// **Does NOT establish:** what fraction of real manuscripts change verdict.
    /// Only run 22 speaks to that, and **n = 1**. Runs 20 and 21 are schema-2
    /// records without claim identity, and no other real report exists.
    ///
    /// **Consequence:** the deterministic verdict's distribution shifts by an
    /// UNMEASURED amount. That matters when the Box 4 comparison resumes —
    /// §26.10 already records that the two sides then compute over different
    /// evidence sets, and this is the second reason the delta is not a
    /// model-quality measurement.
    #[test]
    fn the_resolver_is_specified_across_every_severity_and_claim() {
        use gaply_core::reviewer_agent::{aggregate_reviewer_verdict, claim_is_eligible};
        use gaply_core::evidence::ClaimKind as CK;
        const CLAIMS: &[CK] = &[CK::ProcessState, CK::AuthorshipSignal, CK::ManuscriptDefect];
        const SEVS: &[Sev] = &[Sev::Critical, Sev::Major, Sev::Minor, Sev::Info];

        for (i, claim) in CLAIMS.iter().enumerate() {
            for (j, sev) in SEVS.iter().enumerate() {
                let db = Database::in_memory().unwrap();
                let run_id = format!("run-matrix-{i}-{j}");
                let rec = EvidenceRecord::at_source(
                    "f1".to_string(), AgentKind::Verification, *claim, *sev, 0.5, vec![],
                );
                evidence_persist(&db, &run_id, &[rec], gaply_core::now_epoch()).unwrap();
                let input =
                    assemble_reviewer_input(&db, &run_id, &report(&["t"]), &journal(), &[], None, all_examined())
                        .unwrap();
                let agg = aggregate_reviewer_verdict(&input);

                if claim_is_eligible(*claim) {
                    let counted = agg.breakdown.critical.total()
                        + agg.breakdown.major.total()
                        + agg.breakdown.minor.total()
                        + agg.breakdown.info.total();
                    assert_eq!(counted, 1, "{claim:?}/{sev:?} is eligible and must be counted");
                    assert!(agg.excluded.is_empty());
                    let expected = match sev {
                        Sev::Critical => Recommendation::Reject,
                        Sev::Major => Recommendation::MajorRevision,
                        // One Minor is below MINOR_REVISION_THRESHOLD = 3.
                        Sev::Minor | Sev::Info => Recommendation::Accept,
                    };
                    assert_eq!(agg.verdict.recommendation(), Some(expected), "{claim:?}/{sev:?}");
                } else {
                    assert_eq!(agg.breakdown.critical.total(), 0);
                    assert_eq!(agg.breakdown.major.total(), 0);
                    assert_eq!(
                        agg.verdict.recommendation(),
                        Some(Recommendation::Accept),
                        "{claim:?}/{sev:?} is ineligible — even Critical must not reach the verdict"
                    );
                    assert_eq!(agg.excluded.len(), 1, "and it must be ATTRIBUTED, not silently dropped");
                    assert_eq!(agg.excluded[0].claim, Some(*claim));
                }
            }
        }
    }

    /// §25.10's defect, closed. Malformed evidence must WITHHOLD, never yield
    /// `Accept` at 0.92 — the silent wrong answer this PR exists to remove.
    #[test]
    fn uninterpretable_evidence_withholds_rather_than_accepting() {
        use gaply_core::reviewer_agent::{aggregate_reviewer_verdict, VerdictWithheld};
        let db = Database::in_memory().unwrap();
        let run_id = "run-withheld";
        // Nothing persisted — exactly what escalation.rs:72's failure path leaves
        // behind. Before this PR that produced zero findings and Accept at 0.92.
        let input = assemble_reviewer_input(
            &db,
            run_id,
            &report(&[]),
            &journal(),
            &[],
            Some(VerdictWithheld::EvidenceUninterpretable),
            all_examined(),
        )
        .unwrap();
        let agg = aggregate_reviewer_verdict(&input);

        assert_eq!(agg.verdict.recommendation(), None, "no recommendation may be produced");
        assert_eq!(agg.verdict.probability(), None, "and no probability — not 0.05, not 0.92");
        assert_eq!(
            agg.verdict.withheld_reason(),
            Some(VerdictWithheld::EvidenceUninterpretable),
            "the absence must carry its reason (§4.14)"
        );
        // The breakdown still reports what WAS counted — all-zero is a fact, and
        // it must never be overloaded to carry the withheld reason.
        assert_eq!(agg.breakdown.critical.total(), 0);
        assert_eq!(agg.breakdown.minor.total(), 0);
    }

    /// THE INVARIANT: if a verdict is withheld, EVERY consumer must observe that
    /// it is withheld. No consumer may see `recommendation: unavailable` while
    /// another sees `MajorRevision` from the same run.
    ///
    /// The risk is concrete and structural: the withheld state passes through a
    /// TRANSFORMATION before every consumer sees it. `synthesize_reviewer_letter`
    /// is the sole reader of the aggregation's verdict, the harness reads
    /// `s.letter.recommendation` rather than the aggregation, and the UI reads
    /// the letter. So it is asserted at every boundary, the way PR-1 asserted the
    /// round-trip at every boundary.
    #[test]
    fn a_withheld_verdict_is_observed_as_withheld_at_every_boundary() {
        use gaply_core::reviewer_agent::{
            aggregate_reviewer_verdict, synthesize_reviewer_letter, Recommendation,
            ReviewerNarrative, VerdictWithheld,
        };
        use gaply_core::reviewer_harness::{
            build_comparison_report, HarnessInputs, HarnessTiming, ShadowInputs,
        };

        let db = Database::in_memory().unwrap();
        let run_id = "run-invariant";
        let input = assemble_reviewer_input(
            &db,
            run_id,
            &report(&[]),
            &journal(),
            &[],
            Some(VerdictWithheld::EvidenceUninterpretable),
            all_examined(),
        )
        .unwrap();

        // BOUNDARY 1 — the aggregation.
        let agg = aggregate_reviewer_verdict(&input);
        assert_eq!(agg.verdict.recommendation(), None, "boundary 1: aggregation");

        // BOUNDARY 2 — the letter, which is what the other two read.
        let letter = synthesize_reviewer_letter(
            &agg,
            ReviewerNarrative { body: "ignored".into(), issues: vec![], warnings: vec![] },
        );
        assert!(!letter.available, "boundary 2: the letter must not present itself as available");
        assert_eq!(
            letter.publication_probability, None,
            "boundary 2: no probability at all — not 0.0, not PROB_REJECT's 0.05"
        );
        assert!(
            letter.body.contains("No recommendation was produced"),
            "boundary 2: the reason must be stated to the author, got {:?}",
            letter.body
        );
        assert!(
            letter.warnings.iter().any(|w| w.contains("evidence_uninterpretable")),
            "boundary 2: the machine-readable reason must survive"
        );

        // BOUNDARY 3 — the harness record.
        let wholesale = gaply_core::reviewer_agent::ReviewerEvaluation::unavailable_offline();
        let rec = build_comparison_report(&HarnessInputs {
            run_id,
            manuscript_sha256: "sha",
            recorded_at: 0,
            shadow: Some(ShadowInputs {
                withheld: Some(VerdictWithheld::EvidenceUninterpretable),
                letter: &letter,
                breakdown: &agg.breakdown,
                findings_sent: 0,
                narrative_available: false,
            }),
            wholesale: &wholesale,
            wholesale_findings_sent: 0,
            findings_projection_digest: "d",
            summary_digest: "s",
            summary_format_version: 1,
            journal_name: None,
            guidelines_url: None,
            timing: HarnessTiming::default(),
            proxy_meta: None,
        });
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(
            v["shadow_recommendation"]["status"], "unavailable",
            "boundary 3: the record must not observe the letter's inert filler as a verdict"
        );
        assert_eq!(v["shadow_recommendation"]["requires"], "verdict_withheld");
        assert_eq!(
            v["verdict_withheld"]["value"], "evidence_uninterpretable",
            "boundary 3: breakdown all-zero beside an unavailable verdict must be unambiguous"
        );

        // BOUNDARY 4 — the UI payload. The frontend reads the letter, so what it
        // receives is what boundary 2 produced, serialized.
        let ui = serde_json::to_value(&letter).unwrap();
        assert_eq!(ui["available"], false, "boundary 4: UI payload");
        // STRUCTURAL, not a runtime guard: the key is OMITTED from the wire, so
        // no consumer can read a number for a run where none was computed. A
        // plain f64 emitted 0.0 into every artifact for such a run (§4.4).
        assert!(
            ui.get("publication_probability").is_none(),
            "boundary 4: the probability key must be absent, got {ui}"
        );
        assert_ne!(
            ui["recommendation"],
            serde_json::to_value(Recommendation::MajorRevision).unwrap(),
            "boundary 4: no consumer may read a real recommendation from a withheld run"
        );
    }

    fn lanes(v: bool, val: bool, plag: bool, ai: bool, ex: bool) -> LaneExamination {
        LaneExamination {
            verification_examined: v,
            validation_examined: val,
            plagiarism_examined: plag,
            ai_detection_examined: ai,
            extraction_examined: ex,
        }
    }

    /// F6. Zero eligible findings has TWO causes and only the lane state
    /// separates them: genuinely clean, versus nothing checked. With nothing
    /// examined, `Accept` at 0.92 would assert an absence of defects that was
    /// never looked for.
    #[test]
    fn nothing_examined_withholds_rather_than_accepting() {
        use gaply_core::reviewer_agent::{aggregate_reviewer_verdict, VerdictWithheld};
        let db = Database::in_memory().unwrap();
        let run_id = "run-nothing";
        let input = assemble_reviewer_input(
            &db, run_id, &report(&[]), &journal(), &[], None,
            lanes(false, false, false, false, false),
        )
        .unwrap();
        let agg = aggregate_reviewer_verdict(&input);
        assert_eq!(agg.verdict.recommendation(), None, "no findings AND nothing examined");
        assert_eq!(agg.verdict.withheld_reason(), Some(VerdictWithheld::NothingExamined));
        assert_eq!(agg.not_examined.len(), 5, "every denominator lane is named");
    }

    /// The same empty finding set, but something WAS examined — the honest
    /// `Accept`. This is the pair that makes the disambiguator's job visible:
    /// identical findings, opposite verdicts, decided only by the lane state.
    #[test]
    fn nothing_found_but_something_examined_accepts() {
        use gaply_core::reviewer_agent::aggregate_reviewer_verdict;
        let db = Database::in_memory().unwrap();
        let input = assemble_reviewer_input(
            &db, "run-clean", &report(&[]), &journal(), &[], None,
            lanes(true, true, true, true, true),
        )
        .unwrap();
        let agg = aggregate_reviewer_verdict(&input);
        assert_eq!(agg.verdict.recommendation(), Some(Recommendation::Accept));
        assert!(agg.not_examined.is_empty());
    }

    /// THE PARTIAL CASE, decided in §26: citations starved, statistics examined
    /// the whole manuscript. `Accept` is honest as "clean as far as we looked"
    /// and dishonest as "clean" — so it does NOT withhold, and the caveat must
    /// reach every consumer. Asserted from the aggregation THROUGH the letter,
    /// per §26.11's rule: a test entered downstream of the defect cannot detect
    /// it, and the UI reads the letter rather than the aggregation.
    #[test]
    fn one_starved_lane_caveats_rather_than_withholding_and_reaches_the_letter() {
        use gaply_core::reviewer_agent::{
            aggregate_reviewer_verdict, synthesize_reviewer_letter, ReviewerNarrative,
        };
        let db = Database::in_memory().unwrap();
        let input = assemble_reviewer_input(
            &db, "run-partial", &report(&[]), &journal(), &[], None,
            // Citations starved; everything else examined.
            lanes(false, true, true, true, true),
        )
        .unwrap();
        let agg = aggregate_reviewer_verdict(&input);

        assert_eq!(
            agg.verdict.recommendation(),
            Some(Recommendation::Accept),
            "a manuscript with real evidence must NOT be withheld over one starved lane"
        );
        assert_eq!(agg.not_examined.len(), 1);
        assert_eq!(agg.not_examined[0].lane, "Citation verification");

        // AND IT MUST REACH THE LETTER — the caveat is the whole reason not
        // withholding is honest.
        let letter = synthesize_reviewer_letter(
            &agg,
            ReviewerNarrative { body: "b".into(), issues: vec![], warnings: vec![] },
        );
        let caveats: Vec<&String> =
            letter.warnings.iter().filter(|w| w.starts_with("not examined:")).collect();
        assert_eq!(caveats.len(), 1, "the caveat must reach the letter: {:?}", letter.warnings);
        assert!(caveats[0].contains("Citation verification"));
        assert!(caveats[0].contains("no references were parsed"));
    }

    /// BLAST RADIUS. The typed-consumer refactor must change no verdict on a
    /// well-formed report, so a parse regression and a decision regression stay
    /// attributable — the same separation PR-1's pin enforced.
    #[test]
    fn typing_the_report_does_not_change_the_recommendation() {
        use gaply_core::reviewer_agent::aggregate_reviewer_verdict;
        let db = Database::in_memory().unwrap();
        let run_id = "run-typed";
        let records = vec![
            EvidenceRecord::at_source("f1".to_string(), AgentKind::ValidationMaths, ClaimKind::ManuscriptDefect, Sev::Major, 1.0, vec![]),
            EvidenceRecord::at_source("f2".to_string(), AgentKind::Extraction, ClaimKind::ManuscriptDefect, Sev::Minor, 0.5, vec![]),
        ];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();
        let input = assemble_reviewer_input(
            &db, run_id, &report(&["a", "b"]), &journal(), &[], None, all_examined(),
        )
        .unwrap();
        let agg = aggregate_reviewer_verdict(&input);
        assert_eq!(agg.verdict.recommendation(), Some(Recommendation::MajorRevision));
        assert_eq!(agg.verdict.probability(), Some(0.30));
        assert_eq!(agg.breakdown.major.total(), 1);
        assert_eq!(agg.breakdown.minor.total(), 1);
    }

    /// THE DEFECT THE TYPING REMOVES. A malformed title used to become "" via
    /// `unwrap_or("")` — and `title` sits inside BOTH digests (§31.7), so a
    /// malformed report produced a STABLE BUT WRONG identity rather than
    /// failing. It must now fail to parse.
    #[test]
    fn a_malformed_title_fails_to_parse_instead_of_yielding_a_digest() {
        use gaply_core::report::PublishReadyReport;
        let malformed = json!({
            "verdict": "concern",
            "combined_confidence": 0.5,
            "findings": [{
                "severity": "major", "tier": "ai_assessed_moderate",
                "certainty_label": "x", "agent": "extraction",
                "claim": "manuscript_defect",
                "title": 42,                       // not a string
                "detail": "d", "confidence": 0.5, "provenance": []
            }],
            "evidence": [], "checklist": [],
            "debate": {"rounds_run":1,"converged":true,"overridden_by_constraint":false,
                       "rejected_agents":[],"revised_agents":[]},
            "disclaimer": "d"
        });
        let parsed: Result<PublishReadyReport, _> = serde_json::from_value(malformed);
        assert!(
            parsed.is_err(),
            "a non-string title must FAIL, not silently become \"\" inside the digests"
        );

        // A MISSING title fails too — the old code returned "" for both.
        let missing = json!({
            "verdict": "concern", "combined_confidence": 0.5,
            "findings": [{
                "severity": "major", "tier": "ai_assessed_moderate",
                "certainty_label": "x", "agent": "extraction",
                "claim": "manuscript_defect",
                "detail": "d", "confidence": 0.5, "provenance": []
            }],
            "evidence": [], "checklist": [],
            "debate": {"rounds_run":1,"converged":true,"overridden_by_constraint":false,
                       "rejected_agents":[],"revised_agents":[]},
            "disclaimer": "d"
        });
        assert!(serde_json::from_value::<PublishReadyReport>(missing).is_err());
    }

    #[test]
    fn assemble_joins_store_verdicts_with_report_titles() {
        let db = Database::in_memory().unwrap();
        let run_id = "run-assemble";
        let records = vec![
            record("f1", AgentKind::Verification, Sev::Major, 0.6),
            record("f2", AgentKind::ValidationMaths, Sev::Critical, 1.0),
        ];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();
        // Escalate f1 -> gate-passed verdict; f2 stays local.
        evidence_record_escalation(
            &db,
            run_id,
            "f1",
            &EscalationOutcome {
                llm_verdict: "REFUTED".into(),
                llm_rationale: "citation not found".into(),
                verified: true,
                gate_flags: vec![],
                provider: "mock".into(),
                provider_model: "mock-1".into(),
            },
        )
        .unwrap();

        let input =
            assemble_reviewer_input(&db, run_id, &report(&["refuted citation", "impossible SD"]), &journal(), &[], None, all_examined())
                .unwrap();

        assert_eq!(input.findings.len(), 2);
        let f1 = input.findings.iter().find(|f| f.id == "f1").unwrap();
        assert_eq!(f1.title, "refuted citation"); // joined by parsed f{N} index
        assert_eq!(f1.verified, Some(true));
        assert_eq!(f1.verdict.as_deref(), Some("REFUTED"));
        let f2 = input.findings.iter().find(|f| f.id == "f2").unwrap();
        assert_eq!(f2.title, "impossible SD");
        assert_eq!(f2.verified, None); // never escalated -> NotEscalated
        assert_eq!(input.metadata.run_id, run_id);
        assert_eq!(input.metadata.overall_verdict, "concern");
    }

    #[test]
    fn shadow_synthesis_produces_deterministic_verdict_without_proxy() {
        let db = Database::in_memory().unwrap();
        let run_id = "run-shadow-offline";
        let records = vec![record("f1", AgentKind::ValidationMaths, Sev::Critical, 1.0)];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();

        // proxy = None -> narrative degrades, verdict still deterministic.
        let outcome =
            run_shadow_synthesis(&db, None, run_id, &report(&["impossible SD"]), &journal(), &[], None, all_examined()).unwrap();
        assert_eq!(outcome.letter.recommendation, Recommendation::Reject);
        assert!(outcome.letter.available);
        assert!(!outcome.narrative_available); // structural flag, not body-string
        assert!(outcome.letter.body.contains("narrative synthesis unavailable"));
    }

    #[test]
    fn shadow_synthesis_grounds_narrative_against_sent_ids() {
        let db = Database::in_memory().unwrap();
        let run_id = "run-shadow-mock";
        let records = vec![record("f1", AgentKind::Verification, Sev::Major, 0.7)];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();

        // The model returns a body + one grounded (f1) and one hallucinated (f99)
        // issue; the gate keeps only the grounded one, and the verdict stays
        // deterministic regardless.
        let proxy = MockProxyClient::returning(json!({
            "body": "The manuscript has a refuted citation.",
            "issues": [
                { "finding_ref": "f1", "severity": "major", "rationale": "citation refuted" },
                { "finding_ref": "f99", "severity": "major", "rationale": "hallucinated" },
            ],
        }));
        let outcome = run_shadow_synthesis(
            &db,
            Some(&proxy as &dyn ProxyClient),
            run_id,
            &report(&["refuted citation"]),
            &journal(),
            &[],
            None,
            all_examined(),
        )
        .unwrap();
        // Deterministic verdict from the Major finding, regardless of narrative.
        assert_eq!(outcome.letter.recommendation, Recommendation::MajorRevision);
        assert_eq!(outcome.letter.issues.len(), 1);
        assert_eq!(outcome.letter.issues[0].finding_ref, "f1");
        assert!(outcome.letter.available);
        assert!(outcome.narrative_available); // a real proxy response arrived
        assert_eq!(outcome.findings_sent, 1);
    }
}
