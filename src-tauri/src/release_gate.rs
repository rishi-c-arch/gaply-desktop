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
/// Premium only. PRIVACY's counterpart on the route that DOES send manuscript
/// text: everything sent is covered by a stored consent, in both directions.
pub const TIER: &str = "TIER";

/// The four invariants that are about the payload's SHAPE rather than its
/// contents, and are therefore common to both routes. Named once so the two
/// enumerations below cannot drift apart by an edit to one of them.
fn structural(gate: &mut GateReport, payload: &Value, record: Option<&Value>, lines: Option<&[String]>) {
    gate.record(PROVENANCE, check_provenance(payload));
    gate.record(SELECTION, check_selection(payload));
    gate.record(COMPARISON, check_comparison(record));
    gate.record(PERSISTENCE, check_persistence(lines));
}

/// **THE FREE ROUTE'S GATE — the five invariants, PRIVACY included, unchanged.**
///
/// Both the runner and the all-unavailable fixture call this, so a new
/// invariant is covered by the fixture ON THE DAY IT IS WRITTEN — nobody has to
/// remember to add it in two places. That is precisely what LIVENESS could not
/// do: it guarded two hard-coded NAMES, so a new check was outside it until
/// someone classified it (ARCHITECTURE_TRACE §33).
///
/// # This was `run_all`, and the rename is the point
///
/// There is no longer a gate that runs "all" the invariants, because TIER and
/// PRIVACY are mutually exclusive by design: a payload that satisfies one
/// cannot satisfy the other. A single enumeration would have had to take a tier
/// argument and skip one of them — which is a tier-aware PRIVACY wearing a
/// different name. See [`run_premium_gate`].
pub fn run_free_gate(
    payload: &Value,
    report_details: &[String],
    record: Option<&Value>,
    lines: Option<&[String]>,
) -> GateReport {
    let mut gate = GateReport::default();
    gate.record(PRIVACY, check_privacy(payload, report_details));
    structural(&mut gate, payload, record, lines);
    gate
}

/// **THE PREMIUM ROUTE'S GATE — the four structural invariants plus TIER.**
///
/// # Why PRIVACY is absent, and why that is not a relaxation
///
/// `check_privacy` is NOT made tier-aware, and the reason is that its worth is
/// that it takes no argument. *"No manuscript text crosses this boundary"* is a
/// sentence that survives only while nothing can weaken it; the moment it grows
/// an `if tier == Premium` it stops being a gate and becomes a policy wearing a
/// gate's name, and the free tier's central claim is then only as strong as
/// whoever last edited the condition.
///
/// So PRIVACY stays unconditional and guards the free route alone. The premium
/// route is guarded by a DIFFERENT unconditional invariant — [`check_tier`] —
/// which asserts the thing that is actually true there: everything sent is
/// covered by a stored consent record, in both directions.
///
/// Manuscript text is not an exception the premium gate tolerates. It is the
/// precondition the premium gate REQUIRES (`check_tier` fails a payload that
/// carries none), which is what makes the two routes a partition rather than
/// one route with a bypass.
///
/// `resolve` is the seam onto the `consent_records` table Phase 1 creates. It
/// is a parameter rather than a global so the invariant is testable without the
/// table, and so that a gate run can never silently consult a different store
/// than the one the payload was built against.
pub fn run_premium_gate(
    payload: &Value,
    report_details: &[String],
    record: Option<&Value>,
    lines: Option<&[String]>,
    resolve: &dyn Fn(&str) -> Option<gaply_core::consent::ConsentRecord>,
) -> GateReport {
    let mut gate = GateReport::default();
    gate.record(TIER, check_tier(payload, report_details, resolve));
    structural(&mut gate, payload, record, lines);
    gate
}

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
    // ABSENT is not EMPTY. `findings: []` is a clean manuscript and Pass is
    // correct there; a MISSING or non-array `findings` means nothing was
    // readable, and reporting Pass on that is a check passing on input it never
    // got. Found by the fixture that replaced LIVENESS, on its first run (§33).
    // Typed absence per §4.12.
    if payload["summary"]["findings"].as_array().is_none() {
        return GateOutcome::skip("payload has no summary.findings array — nothing was readable");
    }
    match manuscript_text_in(payload, report_details) {
        Some(reason) => GateOutcome::fail(PRIVACY, reason),
        None => GateOutcome::Pass,
    }
}

/// **THE ONE DEFINITION of "does this payload carry manuscript text".**
///
/// Returns the reason it does, or `None`.
///
/// # Why this is extracted rather than inlined in PRIVACY
///
/// PRIVACY and TIER are opposite verdicts on the SAME question. PRIVACY fails
/// when the answer is yes; TIER requires the answer to be yes and demands a
/// consent record for it. If each grew its own detector, a payload could read
/// as clean to one and as text-bearing to the other — and a payload that passed
/// both gates would exist, with every test still green, because no test
/// compares the two detectors.
///
/// That is §11 D134's rule — *"a second copy of a privacy claim is a second
/// thing to keep true"* — applied to a PREDICATE rather than to a sentence, and
/// it is the property the whole two-gate partition rests on.
/// `both_gates_key_on_one_definition_of_manuscript_text` is the guard.
///
/// Two detections, deliberately different in kind:
///
/// * **structural** — a `detail` field, which is where `manuscript_excerpt`
///   lives (`reviewer_agent.rs:345`);
/// * **end to end** — a 60-character probe from any compiled-report detail
///   appearing anywhere in the serialized payload, which is what makes this a
///   measurement rather than a schema check.
pub fn manuscript_text_in(payload: &Value, report_details: &[String]) -> Option<String> {
    if let Some(findings) = payload["summary"]["findings"].as_array() {
        for f in findings {
            if !f["detail"].is_null() {
                return Some(format!("payload finding {} carries a `detail` field", f["id"]));
            }
        }
    }
    let serialized = payload.to_string();
    for d in report_details {
        let probe: String = d.chars().take(60).collect();
        if probe.chars().count() >= 20 && serialized.contains(probe.as_str()) {
            return Some(format!(
                "manuscript text from a finding detail appears in the payload: {probe:?}"
            ));
        }
    }
    None
}

/// **TIER — everything sent is covered by a stored consent, in both
/// directions.**
///
/// PRIVACY's counterpart on the route that exists to send manuscript text.
/// Unconditional, like PRIVACY: it takes no tier argument, because it IS the
/// tier's invariant.
///
/// Three things, and the third is the one that makes the routes a partition:
///
/// 1. **Forward** — a payload carrying manuscript text carries a consent id.
/// 2. **Backward** — a consent id in a payload resolves to a stored row whose
///    scope covers what was sent. An id that resolves to nothing is a CLAIM
///    about a row, and an unresolvable claim is not a consent.
/// 3. **The premium route is for payloads that carry manuscript text.** A
///    structurally clean payload does not belong here even with a valid consent
///    attached — it belongs on the free route, where PRIVACY can vouch for it.
///    Without this, a clean payload with a consent id would pass BOTH gates and
///    the partition would be a pair of overlapping sets.
///
/// # What this does NOT yet check
///
/// Scope is checked against `MANUSCRIPT` only, because manuscript text is the
/// only thing the payload builder can currently send. The analysis scopes
/// (`ANALYSIS_CODE`, `ANALYSIS_DATA`, …) have no producer yet; when one exists,
/// the check becomes `record.scope().covers(what_was_actually_sent)` and the
/// `what_was_actually_sent` term is the new work — not this function's shape.
pub fn check_tier(
    payload: &Value,
    report_details: &[String],
    resolve: &dyn Fn(&str) -> Option<gaply_core::consent::ConsentRecord>,
) -> GateOutcome {
    use gaply_core::consent::ConsentScope;
    if payload["summary"]["findings"].as_array().is_none() {
        return GateOutcome::skip("payload has no summary.findings array — nothing was readable");
    }
    let carries = manuscript_text_in(payload, report_details);
    let id = payload["consent"]["record_id"].as_str();

    match (carries, id) {
        (Some(what), None) => GateOutcome::fail(
            TIER,
            format!("payload carries manuscript text with no consent record id: {what}"),
        ),
        (None, None) => GateOutcome::fail(
            TIER,
            "no consent record id in the payload — the premium route requires one".to_string(),
        ),
        (carries, Some(id)) => {
            let Some(record) = resolve(id) else {
                return GateOutcome::fail(
                    TIER,
                    format!("consent record id {id:?} in the payload resolves to no stored row"),
                );
            };
            if carries.is_none() {
                return GateOutcome::fail(
                    TIER,
                    format!(
                        "consent record id {id:?} but the payload carries no manuscript text — \
                         a payload with nothing to consent to belongs on the free route"
                    ),
                );
            }
            if !record.scope().covers(ConsentScope::MANUSCRIPT) {
                return GateOutcome::fail(
                    TIER,
                    format!(
                        "consent record {id:?} scope {:#08b} does not cover Manuscript",
                        record.scope().bits()
                    ),
                );
            }
            GateOutcome::Pass
        }
    }
}

/// **PROVENANCE — only the nine permitted structured prefixes.**
pub fn check_provenance(payload: &Value) -> GateOutcome {
    let empty = Vec::new();
    let Some(findings) = payload["summary"]["findings"].as_array() else {
        return GateOutcome::skip("payload has no summary.findings array — nothing was readable");
    };
    for f in findings {
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
    let (Some(fs), Some(cs)) = (
        payload["summary"]["findings"].as_array(),
        payload["summary"]["checklist"].as_array(),
    ) else {
        return GateOutcome::skip(
            "payload has no summary.findings/checklist arrays — nothing was readable",
        );
    };
    let f = fs.len();
    if f > MAX_FINDINGS {
        return GateOutcome::fail(SELECTION, format!("{f} findings exceeds MAX_FINDINGS {MAX_FINDINGS}"));
    }
    let c = cs.len();
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


#[cfg(test)]
mod all_unavailable {
    //! **THE REPLACEMENT FOR LIVENESS.**
    //!
    //! LIVENESS asked *"did a proxy-dependent invariant report Pass with no
    //! proxy?"* and answered it by consulting a hard-coded list of two names.
    //! This asks the same question by CONSTRUCTION: feed every check inputs it
    //! cannot read, and assert that none of them claims Pass.
    //!
    //! **Stronger on every axis.** It tests the property rather than a name
    //! list; it needs no `proxy_available`, so it cannot be disabled by a
    //! keychain entry; it runs at build time rather than on a gate run; and it
    //! cannot go stale when an invariant changes its inputs, because the
    //! assertion is over behaviour rather than over a classification.
    use super::*;
    use serde_json::json;

    /// A comparison record whose every metric is `Unavailable`.
    ///
    /// `Metric` is `#[serde(tag = "status")]` with `Unavailable { source,
    /// requires }` — **no `value` key** — so `record["x"]["value"]` is `Null`
    /// and a check literally cannot read what is not there. That is a
    /// STRUCTURAL property of the type, not a convention the check follows.
    fn all_unavailable_record() -> serde_json::Value {
        let u = json!({ "status": "unavailable", "source": "reviewer_output",
                        "requires": "requires_live_proxy" });
        json!({
            "shadow_findings_sent": u,
            "wholesale_findings_sent": u,
            "wholesale_publication_probability": u,
            "recommendation_agreement": u,
        })
    }

    /// **No check may report Pass on inputs it could not read.**
    ///
    /// Iterating the gate rather than naming COMPARISON is the entire point.
    /// Today COMPARISON is the only check that reads a metric, so a fixture
    /// naming it would assert the same thing and look equivalent. **The
    /// difference appears when a seventh check arrives — which is exactly when
    /// LIVENESS failed.**
    #[test]
    fn no_invariant_reports_pass_on_inputs_it_could_not_read() {
        let report = run_free_gate(
            &serde_json::Value::Null,          // no payload
            &[],                               // no report details
            Some(&all_unavailable_record()),   // every metric Unavailable
            None,                              // sink unreadable
        );
        assert_eq!(report.results().len(), 5, "the free gate must enumerate every invariant");
        // The premium enumeration is held to the same property in
        // `partition::neither_gate_reports_pass_on_inputs_it_could_not_read`.
        for (name, outcome) in report.results() {
            assert!(
                !outcome.is_pass(),
                "{name} reported Pass on inputs it could not read: {outcome:?}"
            );
        }
        let cov = report.coverage();
        assert!(cov.contains("not evaluated:"), "coverage must name what went unevaluated: {cov}");
    }

    /// The distinction the guards turn on: ABSENT is not EMPTY.
    ///
    /// A genuinely clean manuscript has `findings: []` and every check must
    /// still PASS on it — otherwise the guards above would have converted a
    /// correct result into a false skip.
    #[test]
    fn an_empty_findings_array_still_passes() {
        let payload = json!({ "summary": { "findings": [], "checklist": [] } });
        let report = run_free_gate(&payload, &[], None, None);
        for (name, outcome) in report.results() {
            if matches!(*name, COMPARISON | PERSISTENCE) {
                continue; // no record, no sink — skipped for unrelated reasons
            }
            assert!(outcome.is_pass(), "{name} must pass on a clean empty payload: {outcome:?}");
        }
    }
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

#[cfg(test)]
mod partition {
    //! **DECISION A — two unconditional gates, and the payload passes exactly one.**
    //!
    //! `check_privacy` is NOT made tier-aware. Its worth is that it takes no
    //! argument: *"no manuscript text crosses this boundary"* is a sentence that
    //! survives only while nothing can weaken it. A tier-aware version is a
    //! policy wearing a gate's name.
    //!
    //! So there are two gates, each unconditional:
    //!
    //! | gate | invariants | guards |
    //! |---|---|---|
    //! | [`run_free_gate`] | PRIVACY + the four structural | the free route |
    //! | [`run_premium_gate`] | the four structural + TIER | the premium route |
    //!
    //! PRIVACY is absent from the premium gate because manuscript text is what
    //! that tier exists to send — not because it was relaxed there.
    //!
    //! # THIS MODULE WAS WRITTEN BEFORE EITHER GATE EXISTED
    //!
    //! It did not compile, which is the negative control: a partition assertion
    //! whose first run is green proves nothing about whether it partitions.
    //!
    //! # WHERE THE STATED PROPERTY DOES NOT HOLD, AND WHY THAT IS THE POINT
    //!
    //! The property as first stated was *"no payload can pass both or neither."*
    //! **The first half holds universally. The second half is false for exactly
    //! one payload shape, and that shape is the one the whole design exists to
    //! reject:**
    //!
    //! | payload | free | premium |
    //! |---|---|---|
    //! | no manuscript text, no consent id | **PASS** | fail (TIER) |
    //! | manuscript text + resolving consent id | fail (PRIVACY) | **PASS** |
    //! | no manuscript text, consent id | **PASS** | fail (TIER) |
    //! | **manuscript text, NO consent id** | fail (PRIVACY) | fail (TIER) |
    //!
    //! The fourth row passes NEITHER gate. That is not a crack in the partition
    //! — it is the safety property, and a formulation that made it pass
    //! something would be a worse design that satisfied a nicer sentence. It is
    //! asserted here as a REQUIRED outcome rather than tolerated as an
    //! exception.
    //!
    //! # THE DRIFT THE PARTITION ACTUALLY DEPENDS ON
    //!
    //! Both gates key on ONE definition of "does this payload carry manuscript
    //! text" — [`manuscript_text_in`]. If PRIVACY and TIER each grew their own,
    //! the two gates would disagree about the same payload and the partition
    //! would open silently. That is §11 D134's rule (one definition of a privacy
    //! claim, never two copies) applied to a predicate rather than to a
    //! sentence, and `both_gates_key_on_one_definition_of_manuscript_text` is
    //! what holds it.

    use super::*;
    use gaply_core::consent::{store, ConsentRecord, ConsentScope, Tier};
    use gaply_core::Database;
    use serde_json::json;

    /// A real manuscript sentence. Long enough to clear `check_privacy`'s
    /// 20-char probe floor, so it is detectable by the shared predicate.
    const EXCERPT: &str = "the haemolymph biochemical profile of the bivoltine silkworm hybrid";

    /// A detail string the payload does NOT contain — so a clean payload is
    /// clean against a non-empty `report_details`, not against an empty one.
    const UNSENT: &str = "a different manuscript sentence that never reaches the payload at all";

    fn base(findings: Value, consent: Option<&str>) -> Value {
        let mut p = json!({ "summary": { "findings": findings, "checklist": [] } });
        if let Some(id) = consent {
            p["consent"] = json!({ "record_id": id });
        }
        p
    }

    /// Structurally valid, carries NO manuscript text.
    fn clean(consent: Option<&str>) -> Value {
        base(
            json!([{ "id": "f1", "title": "statistical rule failed", "evidence": ["rule:X"] }]),
            consent,
        )
    }

    /// Carries manuscript text — the excerpt appears in a rendered field.
    fn text_bearing(consent: Option<&str>) -> Value {
        base(json!([{ "id": "f1", "title": EXCERPT, "evidence": ["rule:X"] }]), consent)
    }

    /// A record that is REALLY IN A DATABASE.
    ///
    /// `ConsentRecord::for_test` is no longer reachable from this crate — it is
    /// `pub(crate)` to `gaply-core` — so these tests can no longer fabricate a
    /// consent, which is the point. Every record here is written through
    /// `store::record` and read back, so the partition is asserted against the
    /// same object a production run would hold.
    fn db_with(id: &str) -> (Database, ConsentRecord) {
        let db = Database::in_memory().expect("in-memory db");
        let rec = store::record(
            &db,
            id,
            "m1",
            1_700_000_000,
            ConsentScope::MANUSCRIPT,
            gaply_core::consent::ProviderClass::CloudLlmViaProxy,
            1,
        )
        .expect("consent recorded");
        (db, rec)
    }

    /// The REAL resolver, closing over a real database — what production passes.
    fn resolving(id: &'static str) -> impl Fn(&str) -> Option<ConsentRecord> {
        let (db, _) = db_with(id);
        move |q: &str| store::resolve(&db, q).expect("resolve must not error")
    }

    /// A database with no consent rows at all: every id resolves to nothing.
    fn resolving_nothing() -> impl Fn(&str) -> Option<ConsentRecord> {
        let db = Database::in_memory().expect("in-memory db");
        move |q: &str| store::resolve(&db, q).expect("resolve must not error")
    }

    /// Both gates, on one payload, with COMPARISON and PERSISTENCE supplied so
    /// neither gate skips for a reason unrelated to the partition.
    fn both(payload: &Value, details: &[String]) -> (bool, bool) {
        let record = json!({
            "shadow_findings_sent": { "status": "observed", "value": 1 },
            "wholesale_findings_sent": { "status": "observed", "value": 1 }
        });
        let lines = vec!["{}".to_string()];
        let free = run_free_gate(payload, details, Some(&record), Some(&lines));
        let prem = run_premium_gate(payload, details, Some(&record), Some(&lines), &resolving("c1"));
        (
            free.no_failures() && free.counts().skipped == 0,
            prem.no_failures() && prem.counts().skipped == 0,
        )
    }

    // --- the partition -------------------------------------------------------

    /// **NO PAYLOAD PASSES BOTH.** The universal half, over every shape.
    #[test]
    fn no_payload_passes_both_gates() {
        let d = vec![EXCERPT.to_string(), UNSENT.to_string()];
        for (name, p) in [
            ("clean, no consent", clean(None)),
            ("clean, consent", clean(Some("c1"))),
            ("text, no consent", text_bearing(None)),
            ("text, consent", text_bearing(Some("c1"))),
        ] {
            let (free, prem) = both(&p, &d);
            assert!(!(free && prem), "{name}: passed BOTH gates — the routes are not disjoint");
        }
    }

    /// **EVERY WELL-FORMED PAYLOAD PASSES EXACTLY ONE.**
    #[test]
    fn each_well_formed_payload_passes_exactly_one_gate() {
        let d = vec![EXCERPT.to_string(), UNSENT.to_string()];
        for (name, p, expect_free) in [
            ("clean, no consent", clean(None), true),
            ("clean, consent id but nothing to consent to", clean(Some("c1")), true),
            ("manuscript text under consent", text_bearing(Some("c1")), false),
        ] {
            let (free, prem) = both(&p, &d);
            assert_eq!(free, expect_free, "{name}: free gate");
            assert_eq!(prem, !expect_free, "{name}: premium gate");
            assert!(free ^ prem, "{name}: must pass exactly one gate, got free={free} premium={prem}");
        }
    }

    /// **THE ONE PAYLOAD THAT MUST PASS NEITHER**, asserted as required rather
    /// than tolerated. Manuscript text with no consent record is the violation
    /// both gates exist to stop; a design in which it passed something would be
    /// worse than one that leaves it in the crack.
    #[test]
    fn manuscript_text_without_consent_passes_neither_gate() {
        let d = vec![EXCERPT.to_string()];
        let (free, prem) = both(&text_bearing(None), &d);
        assert!(!free, "PRIVACY must reject manuscript text on the free route");
        assert!(!prem, "TIER must reject manuscript text with no consent record");
    }

    // --- the premium gate's precondition -------------------------------------

    /// **THE PREMIUM GATE CANNOT BE REACHED WITHOUT A CONSENT RECORD.**
    ///
    /// Two halves. The type-level half is that `Tier::Premium` holds a
    /// `ConsentRecord` by construction, so there is no value of `Tier` that
    /// names the premium route without one — asserted by pattern match, since a
    /// missing record would not compile. The runtime half is that a consent id
    /// which resolves to NOTHING fails TIER: an id in a payload is a claim about
    /// a stored row, and an unresolvable claim is not a consent.
    #[test]
    fn the_premium_gate_cannot_be_reached_without_a_consent_record() {
        let (_db, rec) = db_with("c1");
        match Tier::Premium(rec) {
            Tier::Premium(r) => assert_eq!(r.scope(), ConsentScope::MANUSCRIPT),
            Tier::Free => panic!("constructed Premium, matched Free"),
        }

        let d = vec![EXCERPT.to_string()];
        let record = json!({
            "shadow_findings_sent": { "status": "observed", "value": 1 },
            "wholesale_findings_sent": { "status": "observed", "value": 1 }
        });
        let lines = vec!["{}".to_string()];
        let g = run_premium_gate(
            &text_bearing(Some("c1")),
            &d,
            Some(&record),
            Some(&lines),
            &resolving_nothing(),
        );
        assert!(!g.no_failures(), "an unresolvable consent id must fail TIER");
        let named = g.results().iter().any(|(n, o)| {
            *n == TIER && matches!(o, GateOutcome::Fail { detail, .. } if detail.contains("c1"))
        });
        assert!(named, "TIER must name the id that did not resolve: {:?}", g.results());
    }

    /// PRIVACY is absent from the premium gate and present in the free one —
    /// stated as an assertion so a later edit that "harmonises" the two
    /// enumerations fails here rather than silently weakening one route.
    #[test]
    fn the_two_gates_enumerate_what_they_are_supposed_to() {
        let d = vec![UNSENT.to_string()];
        let free = run_free_gate(&clean(None), &d, None, None);
        let prem = run_premium_gate(&clean(None), &d, None, None, &resolving("c1"));

        let names = |g: &GateReport| -> Vec<&'static str> { g.results().iter().map(|(n, _)| *n).collect() };
        let f = names(&free);
        let p = names(&prem);

        assert!(f.contains(&PRIVACY), "the free gate must run PRIVACY: {f:?}");
        assert!(!f.contains(&TIER), "the free gate must not run TIER: {f:?}");
        assert!(p.contains(&TIER), "the premium gate must run TIER: {p:?}");
        assert!(!p.contains(&PRIVACY), "the premium gate must not run PRIVACY: {p:?}");
        for structural in [PROVENANCE, SELECTION, COMPARISON, PERSISTENCE] {
            assert!(f.contains(&structural), "free gate missing {structural}");
            assert!(p.contains(&structural), "premium gate missing {structural}");
        }
        assert_eq!(f.len(), 5, "the free gate is five invariants: {f:?}");
        assert_eq!(p.len(), 5, "the premium gate is five invariants: {p:?}");
    }

    /// **THE DRIFT GUARD.** Both gates must decide "is there manuscript text
    /// here" from the SAME predicate. If they ever grew separate ones, a payload
    /// could read as clean to PRIVACY and as text-bearing to TIER (or the
    /// reverse) and pass both routes — the partition would open with every
    /// existing test still green.
    #[test]
    fn both_gates_key_on_one_definition_of_manuscript_text() {
        // Consent id deliberately ABSENT. TIER fails on either shape here, so
        // "did TIER fail" carries no information — what must track the shared
        // predicate is WHICH failure it reports. Comparing the bare fail/pass
        // bit would be an assertion that cannot distinguish the two reasons,
        // and it would pass even if TIER stopped detecting text entirely.
        let d = vec![EXCERPT.to_string()];
        for (name, p, expect_text) in
            [("clean", clean(None), false), ("text", text_bearing(None), true)]
        {
            let shared_says_text = manuscript_text_in(&p, &d).is_some();
            assert_eq!(shared_says_text, expect_text, "{name}: the shared predicate itself is wrong");

            let privacy_sees_text =
                matches!(check_privacy(&p, &d), GateOutcome::Fail { invariant: PRIVACY, .. });
            assert_eq!(
                privacy_sees_text, shared_says_text,
                "{name}: PRIVACY diverged from the shared predicate"
            );

            let tier_sees_text = match check_tier(&p, &d, &resolving_nothing()) {
                GateOutcome::Fail { invariant: TIER, ref detail } => {
                    detail.contains("carries manuscript text")
                }
                o => panic!("{name}: TIER must fail without a consent id, got {o:?}"),
            };
            assert_eq!(
                tier_sees_text, shared_says_text,
                "{name}: TIER diverged from the shared predicate"
            );
        }
    }

    /// A skip is not a pass on either gate — the §32 property, restated for the
    /// premium enumeration so it cannot be lost when a sixth check is added.
    #[test]
    fn neither_gate_reports_pass_on_inputs_it_could_not_read() {
        let free = run_free_gate(&Value::Null, &[], None, None);
        let prem = run_premium_gate(&Value::Null, &[], None, None, &resolving_nothing());
        for (label, g) in [("free", &free), ("premium", &prem)] {
            for (name, outcome) in g.results() {
                assert!(!outcome.is_pass(), "{label}/{name} passed on unreadable input: {outcome:?}");
            }
        }
    }
}
