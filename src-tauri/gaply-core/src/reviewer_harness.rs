//! Box 4 — shadow-comparison harness (Stage 1 tooling, PURE).
//!
//! Compares the deterministic shadow synthesis against the wholesale reviewer
//! and emits a [`ShadowComparisonReport`]. This is dev/validation tooling: it is
//! logged and test-consumed, it drives NO production behavior, and the wholesale
//! reviewer stays authoritative.
//!
//! # Provenance enforced by construction
//! Every metric is a [`Metric<T>`], which cannot be serialized without its
//! provenance: `Observed` REQUIRES a value AND a source; `Unavailable` REQUIRES
//! a source AND the reason it isn't observable yet. There is no bare-number
//! path, so a report can NEVER imply a metric was observed when it wasn't — the
//! same "derive truth from the actual event, not from parsing its description"
//! stance that closed D4. In particular, token/latency/model metrics are emitted
//! `Unavailable` today (the proxy envelope is `{model, stop_reason, text}` and
//! `verify()` discards even `model`/`stop_reason`), and they will flip to
//! `Observed` — with no code change here — once the proxy surfaces them.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::reviewer_agent::{Recommendation, ReviewerEvaluation, SeverityByStateCounts};

/// WHERE a metric's value comes from — attached to every metric, always.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    /// Computed deterministically in-process (the shadow aggregator).
    DeterministicLocal,
    /// A field of a gated [`ReviewerEvaluation`] (shadow or wholesale).
    ReviewerOutput,
    /// Measured client-side (wall-clock around the call).
    ClientWallClock,
    /// The proxy success envelope `result.model` / `result.stop_reason` —
    /// present server-side but currently DISCARDED by `verify()`.
    ProxyEnvelope,
    /// Token / server-latency usage metadata — NOT in today's proxy envelope.
    ProxyUsageMetadata,
}

/// WHY a metric is (or isn't) observable. For an `Unavailable` metric this is
/// the condition under which it WOULD be observed; `AvailableNow` on an
/// `Unavailable` means "obtainable now, simply not captured this run".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricAvailability {
    AvailableNow,
    /// Needs a real `/verify` call for a meaningful value (LLM narrative / cloud).
    RequiresLiveProxy,
    /// Needs the proxy to EMIT the datum (envelope / usage instrumentation).
    RequiresProxyInstrumentation,
}

/// A metric that cannot exist without its provenance. `Observed` binds a value
/// to its source; `Unavailable` binds a source to the reason it's missing.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Metric<T> {
    Observed { value: T, source: MetricSource },
    Unavailable { source: MetricSource, requires: MetricAvailability },
}

impl<T> Metric<T> {
    pub fn observed(value: T, source: MetricSource) -> Self {
        Metric::Observed { value, source }
    }
    pub fn unavailable(source: MetricSource, requires: MetricAvailability) -> Self {
        Metric::Unavailable { source, requires }
    }
    pub fn is_observed(&self) -> bool {
        matches!(self, Metric::Observed { .. })
    }
}

/// Client-side wall-clock timings (one per path); `None` = not captured.
#[derive(Debug, Clone, Default)]
pub struct HarnessTiming {
    pub shadow: Option<Duration>,
    pub wholesale: Option<Duration>,
}

/// Proxy-surfaced metadata. Every field is `None` today — the envelope does not
/// carry usage, and `verify()` discards `model`/`stop_reason`. Populated only
/// once the proxy/client surface these (Tier 2c / small client change).
#[derive(Debug, Clone, Default)]
pub struct ProxyMeta {
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub server_latency_ms: Option<u64>,
}

/// Everything the harness needs to build a report — borrowed, no ownership.
pub struct HarnessInputs<'a> {
    pub run_id: &'a str,
    pub shadow: &'a ReviewerEvaluation,
    pub shadow_breakdown: &'a SeverityByStateCounts,
    pub shadow_findings_sent: usize,
    /// STRUCTURAL flag set at narrative construction — NOT a body string-match.
    pub shadow_narrative_available: bool,
    pub wholesale: &'a ReviewerEvaluation,
    pub timing: HarnessTiming,
    pub proxy_meta: Option<ProxyMeta>,
}

/// The comparison report. Every field carries its own provenance.
#[derive(Debug, Clone, Serialize)]
pub struct ShadowComparisonReport {
    pub run_id: String,
    pub schema_version: u32,

    // --- Deterministic shadow metrics (AvailableNow) ---
    pub shadow_recommendation: Metric<Recommendation>,
    pub shadow_publication_probability: Metric<f64>,
    pub finding_breakdown: Metric<SeverityByStateCounts>,
    pub shadow_path_available: Metric<bool>,
    pub wholesale_path_available: Metric<bool>,

    // --- Comparison metrics (RequiresLiveProxy for real values) ---
    pub wholesale_recommendation: Metric<Recommendation>,
    pub wholesale_publication_probability: Metric<f64>,
    pub recommendation_agreement: Metric<bool>,
    pub shadow_grounded_issues: Metric<usize>,
    pub wholesale_grounded_issues: Metric<usize>,
    pub shadow_hallucination_drops: Metric<usize>,
    pub wholesale_hallucination_drops: Metric<usize>,
    pub shadow_issue_coverage: Metric<f64>,
    pub shadow_narrative_complete: Metric<bool>,
    pub shadow_runtime_ms: Metric<u64>,
    pub wholesale_runtime_ms: Metric<u64>,

    // --- Proxy instrumentation (RequiresProxyInstrumentation / ProxyEnvelope) ---
    pub prompt_tokens: Metric<u64>,
    pub completion_tokens: Metric<u64>,
    pub server_latency_ms: Metric<u64>,
    pub model_identifier: Metric<String>,
    pub stop_reason: Metric<String>,
}

const SCHEMA_VERSION: u32 = 1;

/// Count the gate's dropped-hallucination warnings (the `potential_hallucination`
/// prefix pushed by both reviewer gates).
fn count_hallucination_drops(warnings: &[String]) -> usize {
    warnings.iter().filter(|w| w.starts_with("potential_hallucination")).count()
}

/// A metric from an `Option`: `Some` → `Observed`, `None` → `Unavailable`.
fn opt_metric<T>(v: Option<T>, source: MetricSource, requires: MetricAvailability) -> Metric<T> {
    match v {
        Some(value) => Metric::observed(value, source),
        None => Metric::unavailable(source, requires),
    }
}

/// Build the comparison report. Each metric is `Observed` ONLY when its source
/// genuinely produced a value this run; otherwise `Unavailable` with the honest
/// reason. Deterministic shadow metrics and availability flags are always
/// observable; LLM/proxy-derived metrics degrade to `Unavailable`.
pub fn build_comparison_report(inp: &HarnessInputs) -> ShadowComparisonReport {
    use MetricAvailability::*;
    use MetricSource::*;

    let wholesale_up = inp.wholesale.available;
    let shadow_narr = inp.shadow_narrative_available;
    let pm = inp.proxy_meta.as_ref();

    // Deterministic — always Observed.
    let shadow_recommendation = Metric::observed(inp.shadow.recommendation, DeterministicLocal);
    let shadow_publication_probability =
        Metric::observed(inp.shadow.publication_probability, DeterministicLocal);
    let finding_breakdown = Metric::observed(*inp.shadow_breakdown, DeterministicLocal);
    // Availability flags are meta-observations about the paths — honestly
    // observable even fully offline (they report the degraded state itself).
    let shadow_path_available = Metric::observed(inp.shadow.available, DeterministicLocal);
    let wholesale_path_available = Metric::observed(inp.wholesale.available, ReviewerOutput);

    // Wholesale verdict — Observed only if the cloud produced it.
    let wholesale_recommendation = if wholesale_up {
        Metric::observed(inp.wholesale.recommendation, ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };
    let wholesale_publication_probability = if wholesale_up {
        Metric::observed(inp.wholesale.publication_probability, ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };

    // Agreement needs BOTH real: the shadow recommendation is always real
    // (deterministic), so this gates on the wholesale side being up.
    let recommendation_agreement = if wholesale_up {
        Metric::observed(inp.shadow.recommendation == inp.wholesale.recommendation, ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };

    // Grounded issues / hallucination drops — need a narrative.
    let shadow_grounded_issues = if shadow_narr {
        Metric::observed(inp.shadow.issues.len(), ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };
    let wholesale_grounded_issues = if wholesale_up {
        Metric::observed(inp.wholesale.issues.len(), ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };
    let shadow_hallucination_drops = if shadow_narr {
        Metric::observed(count_hallucination_drops(&inp.shadow.warnings), ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };
    let wholesale_hallucination_drops = if wholesale_up {
        Metric::observed(count_hallucination_drops(&inp.wholesale.warnings), ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };

    // Coverage: grounded ÷ findings-sent (numerator needs a narrative).
    let shadow_issue_coverage = if shadow_narr && inp.shadow_findings_sent > 0 {
        Metric::observed(
            inp.shadow.issues.len() as f64 / inp.shadow_findings_sent as f64,
            ReviewerOutput,
        )
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };

    // Narrative completeness — from the STRUCTURAL flag, never body text.
    let shadow_narrative_complete = if shadow_narr {
        Metric::observed(!inp.shadow.body.trim().is_empty(), ReviewerOutput)
    } else {
        Metric::unavailable(ReviewerOutput, RequiresLiveProxy)
    };

    // Runtime — client wall-clock; Observed when captured this run.
    let shadow_runtime_ms = match inp.timing.shadow {
        Some(d) => Metric::observed(d.as_millis() as u64, ClientWallClock),
        None => Metric::unavailable(ClientWallClock, AvailableNow),
    };
    let wholesale_runtime_ms = match inp.timing.wholesale {
        Some(d) => Metric::observed(d.as_millis() as u64, ClientWallClock),
        None => Metric::unavailable(ClientWallClock, AvailableNow),
    };

    // Proxy instrumentation — None today, everywhere.
    let prompt_tokens =
        opt_metric(pm.and_then(|m| m.prompt_tokens), ProxyUsageMetadata, RequiresProxyInstrumentation);
    let completion_tokens = opt_metric(
        pm.and_then(|m| m.completion_tokens),
        ProxyUsageMetadata,
        RequiresProxyInstrumentation,
    );
    let server_latency_ms = opt_metric(
        pm.and_then(|m| m.server_latency_ms),
        ProxyUsageMetadata,
        RequiresProxyInstrumentation,
    );
    let model_identifier = opt_metric(pm.and_then(|m| m.model.clone()), ProxyEnvelope, RequiresLiveProxy);
    let stop_reason = opt_metric(pm.and_then(|m| m.stop_reason.clone()), ProxyEnvelope, RequiresLiveProxy);

    ShadowComparisonReport {
        run_id: inp.run_id.to_string(),
        schema_version: SCHEMA_VERSION,
        shadow_recommendation,
        shadow_publication_probability,
        finding_breakdown,
        shadow_path_available,
        wholesale_path_available,
        wholesale_recommendation,
        wholesale_publication_probability,
        recommendation_agreement,
        shadow_grounded_issues,
        wholesale_grounded_issues,
        shadow_hallucination_drops,
        wholesale_hallucination_drops,
        shadow_issue_coverage,
        shadow_narrative_complete,
        shadow_runtime_ms,
        wholesale_runtime_ms,
        prompt_tokens,
        completion_tokens,
        server_latency_ms,
        model_identifier,
        stop_reason,
    }
}

/// Render one metric line with its provenance. Works for any `T: Serialize` by
/// reading the metric's own JSON — so an `Observed` shows value+source and an
/// `Unavailable` shows the reason, never a placeholder number.
fn metric_line<T: Serialize>(label: &str, m: &Metric<T>) -> String {
    let v = serde_json::to_value(m).unwrap_or(Value::Null);
    let source = v.get("source").and_then(Value::as_str).unwrap_or("?");
    match v.get("status").and_then(Value::as_str) {
        Some("observed") => {
            let value = match v.get("value") {
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => "?".to_string(),
            };
            format!("- **{label}** — `{value}` | source: `{source}` | status: observed")
        }
        Some("unavailable") => {
            let requires = v.get("requires").and_then(Value::as_str).unwrap_or("?");
            format!("- **{label}** — _unavailable_ | source: `{source}` | requires: `{requires}`")
        }
        _ => format!("- **{label}** — ?"),
    }
}

impl ShadowComparisonReport {
    /// Human-readable comparison, every metric annotated with source + status.
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "## Box 4 shadow-comparison report\n- run_id: `{}`\n- schema_version: {}\n\n### Metrics\n",
            self.run_id, self.schema_version
        );
        for line in [
            metric_line("shadow_recommendation", &self.shadow_recommendation),
            metric_line("shadow_publication_probability", &self.shadow_publication_probability),
            metric_line("finding_breakdown", &self.finding_breakdown),
            metric_line("shadow_path_available", &self.shadow_path_available),
            metric_line("wholesale_path_available", &self.wholesale_path_available),
            metric_line("wholesale_recommendation", &self.wholesale_recommendation),
            metric_line("wholesale_publication_probability", &self.wholesale_publication_probability),
            metric_line("recommendation_agreement", &self.recommendation_agreement),
            metric_line("shadow_grounded_issues", &self.shadow_grounded_issues),
            metric_line("wholesale_grounded_issues", &self.wholesale_grounded_issues),
            metric_line("shadow_hallucination_drops", &self.shadow_hallucination_drops),
            metric_line("wholesale_hallucination_drops", &self.wholesale_hallucination_drops),
            metric_line("shadow_issue_coverage", &self.shadow_issue_coverage),
            metric_line("shadow_narrative_complete", &self.shadow_narrative_complete),
            metric_line("shadow_runtime_ms", &self.shadow_runtime_ms),
            metric_line("wholesale_runtime_ms", &self.wholesale_runtime_ms),
            metric_line("prompt_tokens", &self.prompt_tokens),
            metric_line("completion_tokens", &self.completion_tokens),
            metric_line("server_latency_ms", &self.server_latency_ms),
            metric_line("model_identifier", &self.model_identifier),
            metric_line("stop_reason", &self.stop_reason),
        ] {
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reviewer_agent::ReviewerIssue;

    fn eval(
        available: bool,
        rec: Recommendation,
        prob: f64,
        issues: usize,
        drops: usize,
        body: &str,
    ) -> ReviewerEvaluation {
        ReviewerEvaluation {
            recommendation: rec,
            publication_probability: prob,
            novelty_score: 0.0,
            novelty_assessment: String::new(),
            journal_fit_score: 0.0,
            journal_fit_note: String::new(),
            body: body.to_string(),
            issues: (0..issues)
                .map(|i| ReviewerIssue {
                    finding_ref: format!("f{}", i + 1),
                    severity: "major".into(),
                    rationale: "r".into(),
                    gate_flags: vec![],
                })
                .collect(),
            alternatives: vec![],
            warnings: (0..drops)
                .map(|i| format!("potential_hallucination: issue cites \"x{i}\" not provided; dropped"))
                .collect(),
            available,
        }
    }

    fn inputs<'a>(
        shadow: &'a ReviewerEvaluation,
        breakdown: &'a SeverityByStateCounts,
        shadow_narr: bool,
        wholesale: &'a ReviewerEvaluation,
    ) -> HarnessInputs<'a> {
        HarnessInputs {
            run_id: "run-1",
            shadow,
            shadow_breakdown: breakdown,
            shadow_findings_sent: 4,
            shadow_narrative_available: shadow_narr,
            wholesale,
            timing: HarnessTiming::default(),
            proxy_meta: None,
        }
    }

    // Walk the serialized report; return the set of field names that are Observed.
    fn observed_fields(report: &ShadowComparisonReport) -> Vec<String> {
        let v = serde_json::to_value(report).unwrap();
        let obj = v.as_object().unwrap();
        obj.iter()
            .filter(|(_, val)| val.get("status").and_then(Value::as_str) == Some("observed"))
            .map(|(k, _)| k.clone())
            .collect()
    }

    // 1. Every metric serializes with BOTH status and source — no bare value.
    #[test]
    fn provenance_always_present() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::Accept, 0.92, 0, 0, "");
        let wh = eval(true, Recommendation::Accept, 0.9, 0, 0, "body");
        let report = build_comparison_report(&inputs(&sh, &bd, true, &wh));
        let v = serde_json::to_value(&report).unwrap();
        for (k, val) in v.as_object().unwrap() {
            if k == "run_id" || k == "schema_version" {
                continue;
            }
            assert!(val.get("status").is_some(), "{k} missing status");
            assert!(val.get("source").is_some(), "{k} missing source");
        }
    }

    // 2. Deterministic metrics are Observed even with wholesale down + no narrative.
    #[test]
    fn deterministic_metrics_always_observed() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::Reject, 0.05, 0, 0, "");
        let wh = ReviewerEvaluation::unavailable_offline();
        let report = build_comparison_report(&inputs(&sh, &bd, false, &wh));
        assert!(report.shadow_recommendation.is_observed());
        assert!(report.shadow_publication_probability.is_observed());
        assert!(report.finding_breakdown.is_observed());
        assert!(report.shadow_path_available.is_observed());
        assert!(report.wholesale_path_available.is_observed());
    }

    // 3. Offline honesty: wholesale-derived metrics are Unavailable{RequiresLiveProxy},
    //    NEVER Observed(Unknown/0).
    #[test]
    fn offline_wholesale_metrics_unavailable_not_zero() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::MajorRevision, 0.30, 0, 0, "");
        let wh = ReviewerEvaluation::unavailable_offline();
        let report = build_comparison_report(&inputs(&sh, &bd, false, &wh));
        assert_eq!(
            report.wholesale_recommendation,
            Metric::unavailable(MetricSource::ReviewerOutput, MetricAvailability::RequiresLiveProxy)
        );
        assert!(!report.wholesale_publication_probability.is_observed());
        assert!(!report.wholesale_grounded_issues.is_observed());
        assert!(!report.recommendation_agreement.is_observed());
    }

    // 4. Agreement: unavailable when wholesale down; Observed(bool) when both up.
    #[test]
    fn recommendation_agreement_needs_both() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::MajorRevision, 0.30, 0, 0, "b");
        let down = ReviewerEvaluation::unavailable_offline();
        assert!(!build_comparison_report(&inputs(&sh, &bd, true, &down))
            .recommendation_agreement
            .is_observed());

        let agree = eval(true, Recommendation::MajorRevision, 0.3, 0, 0, "b");
        let r = build_comparison_report(&inputs(&sh, &bd, true, &agree));
        assert_eq!(
            r.recommendation_agreement,
            Metric::observed(true, MetricSource::ReviewerOutput)
        );

        let disagree = eval(true, Recommendation::Accept, 0.9, 0, 0, "b");
        let r2 = build_comparison_report(&inputs(&sh, &bd, true, &disagree));
        assert_eq!(
            r2.recommendation_agreement,
            Metric::observed(false, MetricSource::ReviewerOutput)
        );
    }

    // 5. Grounded issues + hallucination drops computed from output.
    #[test]
    fn grounded_and_drops_computation() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::MajorRevision, 0.3, 2, 1, "b");
        let wh = eval(true, Recommendation::MajorRevision, 0.3, 3, 2, "b");
        let r = build_comparison_report(&inputs(&sh, &bd, true, &wh));
        assert_eq!(r.shadow_grounded_issues, Metric::observed(2, MetricSource::ReviewerOutput));
        assert_eq!(r.shadow_hallucination_drops, Metric::observed(1, MetricSource::ReviewerOutput));
        assert_eq!(r.wholesale_grounded_issues, Metric::observed(3, MetricSource::ReviewerOutput));
        assert_eq!(r.wholesale_hallucination_drops, Metric::observed(2, MetricSource::ReviewerOutput));
    }

    // 6. Issue coverage = grounded / findings_sent.
    #[test]
    fn issue_coverage_ratio() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::MajorRevision, 0.3, 2, 0, "b"); // 2 issues, 4 sent
        let wh = eval(true, Recommendation::MajorRevision, 0.3, 0, 0, "b");
        let r = build_comparison_report(&inputs(&sh, &bd, true, &wh));
        assert_eq!(r.shadow_issue_coverage, Metric::observed(0.5, MetricSource::ReviewerOutput));
    }

    // 7. Narrative completeness uses the STRUCTURAL flag, not body text: a
    //    non-empty body with narrative_available=false is still Unavailable.
    #[test]
    fn narrative_completeness_is_structural_not_string_match() {
        let bd = SeverityByStateCounts::default();
        // Non-empty body, but the flag says the narrative did NOT come from cloud.
        let sh = eval(true, Recommendation::Accept, 0.92, 0, 0, "a real-looking body");
        let wh = eval(true, Recommendation::Accept, 0.9, 0, 0, "b");
        let r = build_comparison_report(&inputs(&sh, &bd, false, &wh));
        assert!(!r.shadow_narrative_complete.is_observed(), "flag=false must win over body text");

        let r2 = build_comparison_report(&inputs(&sh, &bd, true, &wh));
        assert_eq!(r2.shadow_narrative_complete, Metric::observed(true, MetricSource::ReviewerOutput));
    }

    // 8. Tokens + server latency: ALWAYS Unavailable{ProxyUsageMetadata, RequiresProxyInstrumentation}.
    #[test]
    fn tokens_and_latency_always_unavailable_today() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::Accept, 0.92, 0, 0, "b");
        let wh = eval(true, Recommendation::Accept, 0.9, 0, 0, "b");
        let r = build_comparison_report(&inputs(&sh, &bd, true, &wh));
        let want = Metric::unavailable(
            MetricSource::ProxyUsageMetadata,
            MetricAvailability::RequiresProxyInstrumentation,
        );
        assert_eq!(r.prompt_tokens, want);
        assert_eq!(r.completion_tokens, want);
        assert_eq!(r.server_latency_ms, want);
    }

    // 9. Model id + stop_reason: Unavailable{ProxyEnvelope, RequiresLiveProxy} today.
    #[test]
    fn model_and_stop_reason_unavailable_today() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::Accept, 0.92, 0, 0, "b");
        let wh = eval(true, Recommendation::Accept, 0.9, 0, 0, "b");
        let r = build_comparison_report(&inputs(&sh, &bd, true, &wh));
        let want = Metric::<String>::unavailable(
            MetricSource::ProxyEnvelope,
            MetricAvailability::RequiresLiveProxy,
        );
        assert_eq!(r.model_identifier, want);
        assert_eq!(r.stop_reason, want);
    }

    // 10. Markdown annotates each metric; unavailable renders "requires", not a number.
    #[test]
    fn markdown_annotates_provenance() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::Reject, 0.05, 0, 0, "");
        let wh = ReviewerEvaluation::unavailable_offline();
        let md = build_comparison_report(&inputs(&sh, &bd, false, &wh)).to_markdown();
        assert!(md.contains("source:"));
        assert!(md.contains("status: observed"));
        assert!(md.contains("requires:"));
        // The offline wholesale recommendation must render unavailable, not a value.
        let line = md.lines().find(|l| l.contains("**wholesale_recommendation**")).unwrap();
        assert!(line.contains("_unavailable_"), "got: {line}");
    }

    // 11. Runtime: Observed when a Duration is supplied, Unavailable otherwise.
    #[test]
    fn runtime_observed_only_when_captured() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::Accept, 0.92, 0, 0, "b");
        let wh = eval(true, Recommendation::Accept, 0.9, 0, 0, "b");
        let mut inp = inputs(&sh, &bd, true, &wh);
        inp.timing = HarnessTiming { shadow: Some(Duration::from_millis(12)), wholesale: None };
        let r = build_comparison_report(&inp);
        assert_eq!(r.shadow_runtime_ms, Metric::observed(12, MetricSource::ClientWallClock));
        assert!(!r.wholesale_runtime_ms.is_observed());
    }

    // 12. Zero-overclaim capstone: a fully-offline report's Observed set is EXACTLY
    //     the deterministic metrics + the two availability flags — nothing else.
    #[test]
    fn fully_offline_observes_only_deterministic_and_flags() {
        let bd = SeverityByStateCounts::default();
        let sh = eval(true, Recommendation::Reject, 0.05, 0, 0, "");
        let wh = ReviewerEvaluation::unavailable_offline();
        let report = build_comparison_report(&inputs(&sh, &bd, false, &wh));
        let mut got = observed_fields(&report);
        got.sort();
        let mut want = vec![
            "finding_breakdown".to_string(),
            "shadow_path_available".to_string(),
            "shadow_publication_probability".to_string(),
            "shadow_recommendation".to_string(),
            "wholesale_path_available".to_string(),
        ];
        want.sort();
        assert_eq!(got, want, "offline report observed exactly the deterministic metrics + flags");
    }
}
