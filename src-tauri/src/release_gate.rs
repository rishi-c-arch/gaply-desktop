//! **Pre-ship release gate — the full-command-path instrument.**
//!
//! `read_report.rs` is the fast deterministic development harness; this is its
//! complement. ARCHITECTURE_TRACE §14 is the authoritative statement of which
//! production stages each reaches and why two instruments exist rather than one.
//!
//! # Three outcomes, not two plus a log line
//!
//! [`GateOutcome::Skipped`] is a FIRST-CLASS result. ONTOLOGY §4.12 requires
//! absence to be typed rather than silent, and this is that rule at a fourth
//! level: it already governs fields (`Metric<T>`), reports (§9.2's empty
//! corpus) and artifacts (the silent-no-record case). A gate summary that
//! reports "5/5 passed" while two checks never executed is the same erasure of
//! absence one level up — **an instrument that cannot distinguish "not run"
//! from "passed" has the defect it exists to catch.**
//!
//! [`GateReport`] therefore exposes no pass count on its own. [`GateReport::counts`]
//! returns all three together and [`GateReport::summary`] always prints all three,
//! so "5 PASS, 1 SKIPPED" cannot collapse to "5/5".

use serde_json::Value;

/// One invariant's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    Pass,
    /// The invariant is named so a regression reports the CONTRACT, not
    /// "release gate failed".
    Fail { invariant: &'static str, detail: String },
    /// Not run. Never an implicit pass.
    Skipped { reason: String },
}

impl GateOutcome {
    fn fail(invariant: &'static str, detail: impl Into<String>) -> Self {
        GateOutcome::Fail { invariant, detail: detail.into() }
    }
    fn skip(reason: impl Into<String>) -> Self {
        GateOutcome::Skipped { reason: reason.into() }
    }
    pub fn is_pass(&self) -> bool {
        matches!(self, GateOutcome::Pass)
    }
    pub fn is_skipped(&self) -> bool {
        matches!(self, GateOutcome::Skipped { .. })
    }
}

/// All three counts, only ever obtainable together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub pass: usize,
    pub fail: usize,
    pub skipped: usize,
}

#[derive(Debug, Default)]
/// # WHY THERE IS NO `ship_ready` — ARCHITECTURE_TRACE §32
///
/// There was one. It returned `fail == 0 && skipped == 0`, and it was **false
/// by construction**: the runner passed literal `None` to `check_comparison`
/// and `check_persistence`, so two invariants always skipped and the boolean
/// could never be `true` on any run, in any environment. It was read as a
/// verdict about the SYSTEM when it was a fact about the RUNNER.
///
/// **The defect was not that it was always false. The defect is that one
/// boolean had to answer two independent questions:**
///
/// > *"Did anything fail?"* and *"Was everything checked?"*
///
/// `no_failures()` answers the first. `coverage()` answers the second, as a
/// statement rather than a bit. A caller wanting a ship decision must read
/// both — which is correct, because the decision needs both.
///
/// ## Computing the boolean differently was CONSIDERED AND REJECTED
///
/// Excluding skips (`fail == 0` alone, called `ship_ready`) is the **worst**
/// option available. On the run that exposed all of this it would have
/// returned **`true`** — on a run where a third of the invariants never
/// executed. It converts a visible structural gap into a green light.
///
/// `GateOutcome::Skipped` already carries its reason as typed absence (§4.12).
/// **The summary was discarding it. The fix is to stop collapsing, not to
/// collapse differently.**
pub struct GateReport {
    results: Vec<(&'static str, GateOutcome)>,
}

impl GateReport {
    pub fn record(&mut self, invariant: &'static str, outcome: GateOutcome) {
        self.results.push((invariant, outcome));
    }
    pub fn results(&self) -> &[(&'static str, GateOutcome)] {
        &self.results
    }
    /// All three counts together. There is deliberately no `pass_count()`.
    pub fn counts(&self) -> Counts {
        let mut c = Counts { pass: 0, fail: 0, skipped: 0 };
        for (_, o) in &self.results {
            match o {
                GateOutcome::Pass => c.pass += 1,
                GateOutcome::Fail { .. } => c.fail += 1,
                GateOutcome::Skipped { .. } => c.skipped += 1,
            }
        }
        c
    }
    /// Always prints all three counts, so a pass count can never be read alone.
    pub fn summary(&self) -> String {
        let c = self.counts();
        format!("{} PASS, {} FAIL, {} SKIPPED", c.pass, c.fail, c.skipped)
    }
    /// **Did anything FAIL?** Varies per run, and is false only on a real
    /// failure.
    ///
    /// This is HALF of what `ship_ready` used to claim, and it is the half that
    /// can honestly be a boolean.
    pub fn no_failures(&self) -> bool {
        self.counts().fail == 0
    }

    /// **What was EVALUATED, and what was not — and why.**
    ///
    /// A STATEMENT, not a verdict. It names each skipped invariant with the
    /// reason the skip already carries, instead of collapsing all of them into
    /// one bit.
    pub fn coverage(&self) -> String {
        let c = self.counts();
        let total = c.pass + c.fail + c.skipped;
        let evaluated = c.pass + c.fail;
        let mut out = format!("{evaluated} of {total} invariants evaluated");
        let skipped: Vec<String> = self
            .results
            .iter()
            .filter_map(|(name, o)| match o {
                GateOutcome::Skipped { reason } => Some(format!("{name} ({reason})")),
                _ => None,
            })
            .collect();
        if !skipped.is_empty() {
            out.push_str("; not evaluated: ");
            out.push_str(&skipped.join(", "));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Invariants
// ---------------------------------------------------------------------------

pub const PRIVACY: &str = "PRIVACY";
pub const PROVENANCE: &str = "PROVENANCE";
pub const SELECTION: &str = "SELECTION";
pub const COMPARISON: &str = "COMPARISON";
pub const PERSISTENCE: &str = "PERSISTENCE";
pub const LIVENESS: &str = "LIVENESS";

/// **PRIVACY — no manuscript text crosses the proxy boundary.**
///
/// The product's central claim, and until now never asserted end to end.
/// Assertable WITHOUT a proxy because `build_review_payload` is pure — so the
/// most important invariant is also the most reliable, by construction rather
/// than by luck.
///
/// Two checks: the payload carries no `detail` field (the field that holds
/// `manuscript_excerpt`, `reviewer_agent.rs:345`), and no `detail` STRING from
/// the compiled report appears anywhere in the serialized payload. The second is
/// what makes this end to end rather than structural.
pub fn check_privacy(payload: &Value, report_details: &[String]) -> GateOutcome {
    let empty = Vec::new();
    let findings = payload["summary"]["findings"].as_array().unwrap_or(&empty);
    for f in findings {
        if !f["detail"].is_null() {
            return GateOutcome::fail(
                PRIVACY,
                format!("payload finding {} carries a `detail` field", f["id"]),
            );
        }
    }
    let serialized = payload.to_string();
    for d in report_details {
        let probe: String = d.chars().take(60).collect();
        if probe.chars().count() >= 20 && serialized.contains(probe.as_str()) {
            return GateOutcome::fail(
                PRIVACY,
                format!("manuscript text from a finding detail appears in the payload: {probe:?}"),
            );
        }
    }
    GateOutcome::Pass
}

/// **PROVENANCE — only the nine permitted structured prefixes.**
pub fn check_provenance(payload: &Value) -> GateOutcome {
    let empty = Vec::new();
    for f in payload["summary"]["findings"].as_array().unwrap_or(&empty) {
        for e in f["evidence"].as_array().unwrap_or(&empty) {
            let Some(s) = e.as_str() else {
                return GateOutcome::fail(PROVENANCE, format!("non-string evidence entry: {e}"));
            };
            if !gaply_core::evidence::is_structured_provenance(s) {
                return GateOutcome::fail(
                    PROVENANCE,
                    format!("unstructured provenance reached the payload: {s:?}"),
                );
            }
        }
    }
    GateOutcome::Pass
}

/// **SELECTION — the payload caps are never exceeded.**
pub fn check_selection(payload: &Value) -> GateOutcome {
    use gaply_core::reviewer_agent::{MAX_CHECKLIST, MAX_FINDINGS};
    let empty = Vec::new();
    let f = payload["summary"]["findings"].as_array().unwrap_or(&empty).len();
    if f > MAX_FINDINGS {
        return GateOutcome::fail(SELECTION, format!("{f} findings exceeds MAX_FINDINGS {MAX_FINDINGS}"));
    }
    let c = payload["summary"]["checklist"].as_array().unwrap_or(&empty).len();
    if c > MAX_CHECKLIST {
        return GateOutcome::fail(SELECTION, format!("{c} checklist items exceeds MAX_CHECKLIST {MAX_CHECKLIST}"));
    }
    GateOutcome::Pass
}

/// **COMPARISON — the two sent-counts agree on a healthy run.**
///
/// Divergence is diagnostic, not cosmetic: both draw from the same population
/// in the same order and cap at the same constant, so `shadow_findings_sent == 0`
/// against a non-zero wholesale count means escalation failed to populate the
/// `evidence` table (see `reviewer_harness::HarnessInputs` docs).
pub fn check_comparison(record: Option<&Value>) -> GateOutcome {
    let Some(r) = record else {
        return GateOutcome::skip("no comparison record — the full command path did not run");
    };
    let shadow = r["shadow_findings_sent"]["value"].as_u64();
    let wholesale = r["wholesale_findings_sent"]["value"].as_u64();
    match (shadow, wholesale) {
        (Some(s), Some(w)) if s == w => GateOutcome::Pass,
        (Some(0), Some(w)) if w > 0 => GateOutcome::fail(
            COMPARISON,
            format!("shadow_findings_sent=0 against wholesale={w}: the evidence table was not populated by escalation"),
        ),
        (Some(s), Some(w)) => GateOutcome::fail(COMPARISON, format!("shadow={s} != wholesale={w}")),
        _ => GateOutcome::skip("one or both sent-counts were not Observed in the record"),
    }
}

/// **PERSISTENCE — exactly one JSONL comparison record.**
pub fn check_persistence(lines: Option<&[String]>) -> GateOutcome {
    let Some(l) = lines else {
        return GateOutcome::skip("comparison sink not readable — the full command path did not run");
    };
    let parsed: Vec<_> = l.iter().filter(|s| !s.trim().is_empty()).collect();
    match parsed.len() {
        1 => match serde_json::from_str::<Value>(parsed[0]) {
            Ok(_) => GateOutcome::Pass,
            Err(e) => GateOutcome::fail(PERSISTENCE, format!("record is not parseable JSON: {e}")),
        },
        0 => GateOutcome::fail(PERSISTENCE, "no comparison record was written"),
        n => GateOutcome::fail(PERSISTENCE, format!("{n} records written, expected exactly 1")),
    }
}

/// **LIVENESS — a proxy-dependent invariant must never report Pass when the
/// proxy was unavailable.**
///
/// The meta-invariant: it checks the REPORT, not the product. §4.12 applied to
/// the instrument itself.
pub fn check_liveness(report: &GateReport, proxy_available: bool) -> GateOutcome {
    if proxy_available {
        return GateOutcome::Pass;
    }
    for (name, outcome) in report.results() {
        if matches!(*name, COMPARISON | PERSISTENCE) && outcome.is_pass() {
            return GateOutcome::fail(
                LIVENESS,
                format!("{name} reported Pass with no proxy available — an implicit pass"),
            );
        }
    }
    GateOutcome::Pass
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload(findings: Value) -> Value {
        json!({ "summary": { "findings": findings, "checklist": [] } })
    }

    #[test]
    fn privacy_rejects_a_detail_field_in_the_payload() {
        let p = payload(json!([{ "id": "f1", "title": "t", "detail": "anything", "evidence": [] }]));
        assert!(matches!(
            check_privacy(&p, &[]),
            GateOutcome::Fail { invariant: PRIVACY, .. }
        ));
    }

    #[test]
    fn privacy_rejects_manuscript_text_appearing_anywhere_in_the_payload() {
        // End-to-end, not structural: an excerpt leaking through ANY field.
        let excerpt = "the haemolymph biochemical profile of the bivoltine silkworm hybrid";
        let p = payload(json!([{ "id": "f1", "title": excerpt, "evidence": [] }]));
        match check_privacy(&p, &[excerpt.to_string()]) {
            GateOutcome::Fail { invariant, .. } => assert_eq!(invariant, PRIVACY),
            o => panic!("expected PRIVACY failure, got {o:?}"),
        }
    }

    #[test]
    fn privacy_passes_a_clean_payload() {
        let p = payload(json!([{ "id": "f1", "title": "statistical rule failed", "evidence": ["rule:X"] }]));
        assert_eq!(check_privacy(&p, &["some manuscript sentence that is long enough".into()]), GateOutcome::Pass);
    }

    #[test]
    fn provenance_rejects_an_unstructured_prefix() {
        let p = payload(json!([{ "id": "f1", "evidence": ["location:Results paragraph 5"] }]));
        assert!(matches!(check_provenance(&p), GateOutcome::Fail { invariant: PROVENANCE, .. }));
    }

    #[test]
    fn provenance_accepts_the_permitted_prefixes() {
        let p = payload(json!([{ "id": "f1", "evidence": ["rule:X", "signal:y", "agent:z"] }]));
        assert_eq!(check_provenance(&p), GateOutcome::Pass);
    }

    #[test]
    fn selection_rejects_more_than_max_findings() {
        let many: Vec<Value> = (0..13).map(|i| json!({ "id": format!("f{i}"), "evidence": [] })).collect();
        assert!(matches!(
            check_selection(&payload(json!(many))),
            GateOutcome::Fail { invariant: SELECTION, .. }
        ));
    }

    #[test]
    fn comparison_names_the_evidence_table_when_shadow_is_zero() {
        let r = json!({
            "shadow_findings_sent": { "status": "observed", "value": 0 },
            "wholesale_findings_sent": { "status": "observed", "value": 12 }
        });
        match check_comparison(Some(&r)) {
            GateOutcome::Fail { invariant, detail } => {
                assert_eq!(invariant, COMPARISON);
                assert!(detail.contains("evidence table"), "{detail}");
            }
            o => panic!("expected COMPARISON failure, got {o:?}"),
        }
    }

    #[test]
    fn persistence_rejects_zero_and_many_records() {
        assert!(matches!(check_persistence(Some(&[])), GateOutcome::Fail { .. }));
        let two = vec!["{}".to_string(), "{}".to_string()];
        assert!(matches!(check_persistence(Some(&two)), GateOutcome::Fail { .. }));
        assert_eq!(check_persistence(Some(&["{}".to_string()])), GateOutcome::Pass);
    }

    /// GAP A REGRESSION. The gate must be able to evaluate COMPARISON from a
    /// record the HARNESS actually built — not a hand-written fixture.
    ///
    /// Run 20 exposed this: `shadow_findings_sent` was an input to
    /// `ShadowInputs`, consumed only for `shadow_issue_coverage`, and never
    /// serialized. Every unit test here passed because they all constructed the
    /// JSON by hand, so only one side of the comparison ever reached the
    /// artifact and nobody noticed until a real capture was read.
    #[test]
    fn comparison_is_evaluable_from_a_harness_built_record() {
        use gaply_core::reviewer_agent::ReviewerEvaluation;
        use gaply_core::reviewer_harness::{
            build_comparison_report, HarnessInputs, HarnessTiming, ShadowInputs,
        };
        let ev = ReviewerEvaluation::unavailable_offline();
        let bd = Default::default();
        let report = build_comparison_report(&HarnessInputs {
            run_id: "r",
            manuscript_sha256: "sha",
            recorded_at: 1,
            shadow: Some(ShadowInputs {
                withheld: None,
                letter: &ev,
                breakdown: &bd,
                findings_sent: 8,
                narrative_available: false,
            }),
            wholesale: &ev,
            wholesale_findings_sent: 8,
            findings_projection_digest: "d",
            summary_digest: "s",
            summary_format_version: gaply_core::reviewer_agent::SUMMARY_FORMAT_VERSION,
            journal_name: Some("J"),
            guidelines_url: None,
            timing: HarnessTiming::default(),
            proxy_meta: None,
        });
        let json: Value = serde_json::from_str(&report.to_jsonl_line().unwrap()).unwrap();

        // BOTH sides must be present in the serialized artifact.
        assert_eq!(json["shadow_findings_sent"]["value"], 8, "shadow side must be persisted");
        assert_eq!(json["wholesale_findings_sent"]["value"], 8);

        // And the invariant must actually evaluate, not skip.
        let outcome = check_comparison(Some(&json));
        assert_eq!(outcome, GateOutcome::Pass, "matching counts must PASS, got {outcome:?}");
        assert!(!outcome.is_skipped(), "a persisted record must not read as Skipped");
    }

    // --- THE SKIP PATH ITSELF ------------------------------------------------

    #[test]
    fn the_skip_path_produces_skipped_not_pass() {
        // Without this the instrument's FIRST run in a proxy-less environment
        // would report five green and one skipped, with two of the five never
        // having executed — the defect it exists to catch, on first use.
        let c = check_comparison(None);
        let p = check_persistence(None);
        assert!(c.is_skipped(), "comparison must be Skipped, got {c:?}");
        assert!(p.is_skipped(), "persistence must be Skipped, got {p:?}");
        assert!(!c.is_pass() && !p.is_pass(), "a skip must never read as a pass");
    }

    #[test]
    fn liveness_fails_when_a_proxy_dependent_invariant_passed_without_a_proxy() {
        let mut r = GateReport::default();
        r.record(COMPARISON, GateOutcome::Pass);
        match check_liveness(&r, false) {
            GateOutcome::Fail { invariant, detail } => {
                assert_eq!(invariant, LIVENESS);
                assert!(detail.contains("implicit pass"), "{detail}");
            }
            o => panic!("expected LIVENESS failure, got {o:?}"),
        }
    }

    #[test]
    fn liveness_accepts_a_skip_without_a_proxy() {
        let mut r = GateReport::default();
        r.record(COMPARISON, GateOutcome::skip("no proxy"));
        assert_eq!(check_liveness(&r, false), GateOutcome::Pass);
    }

    #[test]
    fn the_summary_can_never_report_passes_without_skips() {
        let mut r = GateReport::default();
        r.record(PRIVACY, GateOutcome::Pass);
        r.record(PROVENANCE, GateOutcome::Pass);
        r.record(COMPARISON, GateOutcome::skip("no proxy"));
        let s = r.summary();
        assert!(s.contains("2 PASS"), "{s}");
        assert!(s.contains("1 SKIPPED"), "{s}");
        // A skip is NOT a failure — that distinction is the whole point of the
        // two-value summary. `no_failures` stays true; `coverage` is what says
        // the run is incomplete, and it must NAME the invariant and its reason.
        assert!(r.no_failures(), "a skip is not a failure");
        let cov = r.coverage();
        assert!(cov.contains("not evaluated:"), "coverage must state what was not evaluated: {cov}");
        assert!(cov.contains(COMPARISON), "coverage must name the skipped invariant: {cov}");
        assert_eq!(r.counts(), Counts { pass: 2, fail: 0, skipped: 1 });
    }
}
