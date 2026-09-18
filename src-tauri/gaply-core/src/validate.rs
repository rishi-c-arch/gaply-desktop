//! Validation / Maths Agent — a DETERMINISTIC, rules-based statistics
//! checker.
//!
//! This module contains **no LLM or AI call of any kind**. It consumes the
//! `StatClaim` records produced by the Extraction Agent plus the surrounding
//! paragraph text, and applies five fixed rules. Given the same extraction
//! input it always returns the same verdicts — no model output can override
//! or influence them. That is the point: it is the unoverridable arbiter of
//! statistical validity.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::GaplyError;
use crate::extract::stats::Stat;
use crate::extract::{ExtractionResult, Location, SectionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleId {
    TestGroupMismatch,
    PValueOverclaim,
    MissingEffectSize,
    MissingConfidenceInterval,
    SmallSampleCausalClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Critical,
    Major,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::Major => "MAJOR",
        }
    }
}

impl RuleId {
    pub const ALL: [RuleId; 4] = [
        RuleId::TestGroupMismatch,
        RuleId::PValueOverclaim,
        RuleId::MissingEffectSize,
        RuleId::MissingConfidenceInterval,
        // **`SmallSampleCausalClaim` is DECLINED and deliberately absent — §11 D178.**
        //
        // The variant is kept so stored reports carrying its flags still
        // deserialize, but it emits no `RuleOutcome`, because an outcome is a
        // claim that the check RAN. `review_lens::collect` reads every rule in
        // `checks` into `executed`, and a criterion whose checks all executed
        // and none fired earns a STRENGTH — so leaving the rule in while never
        // firing it would convert a decline into a clean bill of health, which
        // is the defect `applies_to` was just repaired for in
        // `claim_strength.rs`.
    ];

    pub fn severity(&self) -> Severity {
        match self {
            // wrong test / underpowered causal claim invalidate the analysis
            RuleId::TestGroupMismatch | RuleId::SmallSampleCausalClaim => Severity::Critical,
            RuleId::PValueOverclaim
            | RuleId::MissingEffectSize
            | RuleId::MissingConfidenceInterval => Severity::Major,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RuleId::TestGroupMismatch => "test-vs-group-count mismatch",
            RuleId::PValueOverclaim => "p-value overclaiming",
            RuleId::MissingEffectSize => "missing effect size",
            RuleId::MissingConfidenceInterval => "missing confidence interval",
            RuleId::SmallSampleCausalClaim => "small sample with causal claim",
        }
    }
}

/// One rule violation at a specific location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flag {
    pub rule: RuleId,
    pub severity: Severity,
    pub location: Location,
    pub explanation: String,
}

/// Per-rule pass/fail summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleOutcome {
    pub rule: RuleId,
    pub passed: bool,
    pub flags: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatsValidityReport {
    /// True when no rule fired.
    pub passed: bool,
    pub checks: Vec<RuleOutcome>,
    pub flags: Vec<Flag>,
}

impl StatsValidityReport {
    pub fn outcome(&self, rule: RuleId) -> &RuleOutcome {
        self.checks.iter().find(|c| c.rule == rule).unwrap_or_else(|| {
            panic!(
                "{rule:?} has no outcome. Every rule in `RuleId::ALL` does; a rule \
                 DECLINED out of `ALL` deliberately does not, because an outcome is a \
                 claim that the check ran (§11 D178)."
            )
        })
    }
    pub fn flags_for(&self, rule: RuleId) -> Vec<&Flag> {
        self.flags.iter().filter(|f| f.rule == rule).collect()
    }
}

struct Patterns {
    group_count: Regex,
    overclaim: Regex,
    causal: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        group_count: Regex::new(
            r"(?i)\b(\d+|two|three|four|five|six|seven|eight|nine|ten)[\s-]+(?:groups?|arms?|cohorts?|conditions?)\b",
        )
        .unwrap(),
        overclaim: Regex::new(
            r"(?i)\b(proves?|proven|proved|confirms?|confirmed|conclusively|definitively)\b",
        )
        .unwrap(),
        causal: Regex::new(
            r"(?i)\b(causes?|caused|causal|causes|leads?\s+to|led\s+to|results?\s+in|resulted\s+in|responsible\s+for|drives?|produces?|induces?|proves?)\b",
        )
        .unwrap(),
    })
}

fn word_to_num(tok: &str) -> Option<i64> {
    match tok.to_lowercase().as_str() {
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        d => d.parse().ok(),
    }
}

/// Largest explicit group/arm/condition count mentioned in `text`.
fn max_group_count(text: &str) -> Option<i64> {
    let mut max = None;
    for c in patterns().group_count.captures_iter(text) {
        if let Some(n) = word_to_num(&c[1]) {
            max = Some(max.map_or(n, |m: i64| m.max(n)));
        }
    }
    max
}

/// Whether rule 4 applies: a p-value in a *primary* results-bearing section.
fn is_primary(section: SectionKind) -> bool {
    matches!(section, SectionKind::Abstract | SectionKind::Results)
}

/// The text a rule evaluates. Thin wrapper over [`crate::extract::paragraph_at`],
/// which is where this function's body now lives — the report quotes the SAME
/// string through the same resolver, so a finding and its quotation cannot
/// disagree. An unresolvable location yields `""`, which no rule matches, which
/// is the pre-existing behaviour.
fn paragraph<'a>(result: &'a ExtractionResult, loc: &Location) -> &'a str {
    crate::extract::paragraph_at(result, loc).unwrap_or("")
}

/// Run all five rules over an extraction result. Deterministic and total.
#[tracing::instrument(skip(result), fields(claims = result.statistics.len()))]
pub fn validate(result: &ExtractionResult) -> StatsValidityReport {
    // Index claim locations by kind (BTreeSet → deterministic ordering).
    let mut pvalue_locs: BTreeSet<Location> = BTreeSet::new();
    let mut ci_locs: BTreeSet<Location> = BTreeSet::new();
    let mut ttest_locs: BTreeSet<Location> = BTreeSet::new();
    // (no `small_n` — rule 5 is declined, see below)

    for claim in &result.statistics {
        match &claim.stat {
            Stat::PValue { .. } => {
                pvalue_locs.insert(claim.location.clone());
            }
            Stat::ConfidenceInterval { .. } => {
                ci_locs.insert(claim.location.clone());
            }
            Stat::Test { name, .. } if name == "t-test" => {
                ttest_locs.insert(claim.location.clone());
            }
            _ => {}
        }
    }

    let mut flags = Vec::new();
    let mk = |rule: RuleId, location: Location, explanation: String| Flag {
        rule,
        severity: rule.severity(),
        location,
        explanation,
    };

    // Rule 1: t-test used where the text indicates 3+ groups.
    for loc in &ttest_locs {
        if let Some(count) = max_group_count(paragraph(result, loc)) {
            if count >= 3 {
                flags.push(mk(
                    RuleId::TestGroupMismatch,
                    loc.clone(),
                    format!(
                        "A t-test compares exactly two groups, but the surrounding text refers to \
                         {count} groups. Comparing 3+ groups with pairwise t-tests inflates the \
                         false-positive rate; use ANOVA (or an equivalent omnibus test)."
                    ),
                ));
            }
        }
    }

    // Rule 2: overclaiming language near a p-value.
    for loc in &pvalue_locs {
        if let Some(m) = patterns().overclaim.find(paragraph(result, loc)) {
            flags.push(mk(
                RuleId::PValueOverclaim,
                loc.clone(),
                format!(
                    "Overclaiming language (\"{}\") appears alongside a p-value. A p-value \
                     quantifies evidence against the null hypothesis; it cannot prove, confirm, \
                     or conclusively establish a hypothesis.",
                    m.as_str()
                ),
            ));
        }
    }

    // Rule 3: p-value reported with no effect size nearby.
    //
    // A TYPED REGION QUERY, not a text scan (§48, §49 shape 3). Asking
    // "is an effect-size TOKEN present in this paragraph" let `\bf2\b` match
    // formulation batch labels F1/F2/F3 and suppress six real findings on one
    // measured paper. The question is whether an effect size was REPORTED, and
    // a typed `Stat::EffectSize` with a value is what answers it.
    //
    // A REGION rather than a `Location` key: rule 3 is a §47.1 WINDOW consumer
    // and must stay one, so item 1b's decision about what a region is widens
    // `Region` rather than changing what this rule asks.
    for loc in &pvalue_locs {
        if !crate::extract::has_effect_size_in(result, &crate::extract::Region::paragraph(loc)) {
            flags.push(mk(
                RuleId::MissingEffectSize,
                loc.clone(),
                "A p-value is reported without an accompanying effect size (e.g. Cohen's d, \
                 eta-squared, r, odds ratio). Statistical significance does not convey the \
                 magnitude or practical importance of an effect."
                    .to_string(),
            ));
        }
    }

    // Rule 4: primary claim (p-value in Abstract/Results) lacks a CI.
    for loc in &pvalue_locs {
        if is_primary(loc.section) && !ci_locs.contains(loc) {
            flags.push(mk(
                RuleId::MissingConfidenceInterval,
                loc.clone(),
                "A primary statistical claim reports a p-value but no confidence interval. \
                 Report a CI so readers can judge the precision and plausible range of the \
                 estimate, not merely whether it crossed a significance threshold."
                    .to_string(),
            ));
        }
    }

    // **Rule 5 (small sample + causal claim) is DECLINED. §11 D178.**
    //
    // It asked "is there an `n < 10` in a paragraph with causal language". The
    // question it MEANT to ask is "is this STUDY's sample small" — a
    // manuscript-level fact — and `Stat::SampleSize` produces paragraph-level
    // numbers. Measured over 20 manuscripts, precision on sub-10 reads was
    // **0 of 15**, and the errors are not one mechanism:
    //
    // ```text
    // n = 3   "We removed responses that had over 20 per cent missing data
    //          (n = 3), which resulted in 150 respondents"   <- EXCLUSION count
    // n = 5   "the other gender made up 3.3 percent (n=5)"   <- SUBGROUP count
    // n = 3   "all tests were performed in triplicate (n=3)" <- REPLICATES, x7
    // n = 0   "the release exponent n = 0.52"                <- a severed decimal
    // ```
    //
    // The `n = 0` family was a real parser defect and is FIXED in
    // `extract/stats.rs` — that repair stands on its own and is not this
    // decline. It took the corpus from 2 CRITICAL findings to 1. The survivor is
    // the dangerous one: a user is told their causal conclusions are
    // "unreliable and unlikely to generalize" because n = 3, when the true
    // sample is stated **eleven words later in the same sentence** as 150.
    //
    // **What makes this a decline rather than a parser to tune: the corpus
    // contains ZERO true positives.** Every genuine study sample in 20
    // manuscripts is >= 12 (12 months, 21/26 sparse strata, 27, 30 pilot, 150,
    // 600). So no change to this rule could be validated as PRESERVING a true
    // finding — only as removing false ones, which is a guard with no negative
    // control (§11 D176's bar, met from the other side).
    //
    // Reopening needs both halves: a sample-size reading that identifies the
    // STUDY's n rather than any local n, and a corpus containing genuinely
    // underpowered studies to measure recall against.

    // Per-rule outcomes, in fixed rule order.
    let checks = RuleId::ALL
        .iter()
        .map(|rule| {
            let count = flags.iter().filter(|f| f.rule == *rule).count();
            RuleOutcome { rule: *rule, passed: count == 0, flags: count }
        })
        .collect();

    StatsValidityReport { passed: flags.is_empty(), checks, flags }
}

/// Persist validation flags as `findings` rows with CRITICAL/MAJOR severity.
pub fn store_validation(
    db: &Database,
    manuscript_id: i64,
    extraction_id: Option<i64>,
    report: &StatsValidityReport,
) -> Result<usize, GaplyError> {
    for flag in &report.flags {
        let where_ = format!("{:?}¶{}", flag.location.section, flag.location.paragraph + 1);
        let message = format!("[{}] {} ({})", flag.rule.label(), flag.explanation, where_);
        db.conn()?.execute(
            "INSERT INTO findings
                 (manuscript_id, extraction_id, severity, category, message, created_at)
             VALUES (?1, ?2, ?3, 'stats_validity', ?4, ?5)",
            params![manuscript_id, extraction_id, flag.severity.as_str(), message, crate::now_epoch()],
        )?;
    }
    tracing::info!(manuscript_id, flags = report.flags.len(), "validation stored");
    Ok(report.flags.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;

    /// Wrap a single results paragraph in a minimal document and validate it.
    fn analyze(body: &str) -> StatsValidityReport {
        let doc = format!("A Study\n\nResults\n{body}");
        validate(&extract_from_text(&doc))
    }

    // ---- Rule 1: test-vs-group-count mismatch ----

    #[test]
    fn rule1_ttest_with_three_groups_flags() {
        let r = analyze("We compared three groups with a t-test (p < 0.05, Cohen's d = 0.4, 95% CI: 0.1 to 0.7).");
        assert!(!r.outcome(RuleId::TestGroupMismatch).passed);
        assert_eq!(r.flags_for(RuleId::TestGroupMismatch)[0].severity, Severity::Critical);
    }

    #[test]
    fn rule1_ttest_with_two_groups_passes() {
        let r = analyze("We compared two groups with a t-test (p < 0.05).");
        assert!(r.outcome(RuleId::TestGroupMismatch).passed);
    }

    #[test]
    fn rule1_anova_with_three_groups_passes() {
        // three groups is fine for ANOVA; rule only targets t-tests
        let r = analyze("We compared three groups with a one-way ANOVA (p < 0.05).");
        assert!(r.outcome(RuleId::TestGroupMismatch).passed);
    }

    // ---- Rule 2: p-value overclaiming ----

    #[test]
    fn rule2_proves_near_pvalue_flags() {
        let r = analyze("This proves the treatment works (p < 0.001, Cohen's d = 0.9, 95% CI: 0.5 to 1.3).");
        assert!(!r.outcome(RuleId::PValueOverclaim).passed);
    }

    #[test]
    fn rule2_measured_language_passes() {
        let r = analyze("The result suggests an association (p = 0.03, r = 0.4, 95% CI: 0.1 to 0.6).");
        assert!(r.outcome(RuleId::PValueOverclaim).passed);
    }

    // ---- Rule 3: missing effect size ----

    #[test]
    fn rule3_pvalue_without_effect_size_flags() {
        let r = analyze("The difference was significant (p = 0.01, 95% CI: 1.1 to 2.0).");
        assert!(!r.outcome(RuleId::MissingEffectSize).passed);
    }

    #[test]
    fn rule3_pvalue_with_cohens_d_passes() {
        let r = analyze("The difference was significant (p = 0.01, Cohen's d = 0.6, 95% CI: 1.1 to 2.0).");
        assert!(r.outcome(RuleId::MissingEffectSize).passed);
    }

    // ---- Rule 4: missing confidence interval ----

    #[test]
    fn rule4_primary_pvalue_without_ci_flags() {
        let r = analyze("The effect was significant (p = 0.02, Cohen's d = 0.5).");
        assert!(!r.outcome(RuleId::MissingConfidenceInterval).passed);
    }

    #[test]
    fn rule4_pvalue_with_ci_passes() {
        let r = analyze("The effect was significant (p = 0.02, Cohen's d = 0.5, 95% CI: 0.2 to 0.8).");
        assert!(r.outcome(RuleId::MissingConfidenceInterval).passed);
    }

    #[test]
    fn rule4_non_primary_section_not_flagged() {
        // a p-value mentioned in the Methods section is not a "primary claim"
        let doc = "A Study\n\nMethods\nPower was set so that p < 0.05 would be detectable.";
        let r = validate(&extract_from_text(doc));
        assert!(r.outcome(RuleId::MissingConfidenceInterval).passed);
    }

    // ---- Rule 5: DECLINED (§11 D178) ----

    /// **The three tests replaced here asserted the rule fired, and passed.**
    /// Their fixture was `"In a pilot (n = 6), the drug caused a dramatic
    /// improvement"` — a sentence written by the rule's author, in the rule's
    /// own shape. No manuscript in the 20-document corpus contains anything
    /// like it: every genuine study sample is >= 12. That is the hand-written-
    /// fixture-inherits-the-premise case, and the corpus was its first
    /// independent vote.
    #[test]
    fn rule5_is_declined_and_reports_no_outcome_at_all() {
        // The exact fixture the old tests passed on.
        let r = analyze(
            "In a pilot (n = 6), the drug caused a dramatic improvement \
             (p = 0.04, d = 1.9, 95% CI: 0.2 to 3.6).",
        );
        assert!(
            r.flags.iter().all(|f| f.rule != RuleId::SmallSampleCausalClaim),
            "the declined rule must emit no flag: {:?}",
            r.flags
        );
        // **And no RuleOutcome, which is the half that matters.** An outcome
        // with `passed: true` is "we checked and it passed" — `review_lens`
        // reads every rule in `checks` into `executed` and can award a STRENGTH
        // from it. A decline that reports a pass is worse than the false
        // positive it replaced.
        assert!(
            r.checks.iter().all(|c| c.rule != RuleId::SmallSampleCausalClaim),
            "a declined rule must not appear as a check that RAN: {:?}",
            r.checks
        );
        assert!(
            !RuleId::ALL.contains(&RuleId::SmallSampleCausalClaim),
            "ALL is what produces `checks`; the decline lives there"
        );
    }

    /// The variant is KEPT so a stored report written before the decline still
    /// deserializes. Pinning it here so nobody removes the variant as dead.
    #[test]
    fn the_declined_rules_variant_still_round_trips() {
        let json = serde_json::to_string(&RuleId::SmallSampleCausalClaim).unwrap();
        let back: RuleId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, RuleId::SmallSampleCausalClaim);
        assert_eq!(RuleId::SmallSampleCausalClaim.severity(), Severity::Critical);
    }

    // ---- clean bill of health ----

    #[test]
    fn fully_reported_claim_passes_all_rules() {
        let r = analyze("Across two groups, the intervention was associated with higher scores \
                         (p = 0.01, Cohen's d = 0.6, 95% CI: 0.2 to 1.0).");
        assert!(r.passed, "unexpected flags: {:?}", r.flags);
    }

    #[test]
    fn is_deterministic_across_runs() {
        let body = "This proves it (p < 0.001) across three groups with a t-test in a pilot (n = 4) that caused change.";
        let a = analyze(body);
        let b = analyze(body);
        assert_eq!(a, b);
    }

    #[test]
    fn store_writes_findings_with_uppercase_severity() {
        let db = Database::in_memory().unwrap();
        let manuscript_id = db.create_manuscript("A Study", "", "").unwrap();
        let report =
            analyze("This proves it (p = 0.01) using three groups with a t-test (n = 5) that caused change.");
        assert!(!report.passed);

        let n = store_validation(&db, manuscript_id, None, &report).unwrap();
        assert_eq!(n, report.flags.len());

        let critical: i64 = db
            .count_rows("findings")
            .and_then(|_| {
                Ok(db
                    .conn()?
                    .query_row(
                        "SELECT COUNT(*) FROM findings WHERE severity = 'CRITICAL' AND category = 'stats_validity'",
                        [],
                        |r| r.get(0),
                    )?)
            })
            .unwrap();
        assert!(critical >= 1, "expected at least one CRITICAL finding");
    }

    /// **A DECLARED CRITERION MUST NOT REACH `pvalue_locs`** — ARCHITECTURE_TRACE
    /// §42, and the defect that motivated the whole item.
    ///
    /// Before this, `"significance was determined at p < 0.05"` produced
    /// `MissingEffectSize` ("A p-value is reported without an accompanying
    /// effect size") about a sentence that reports no result. On the frozen
    /// reference manuscript that was FIVE of five located findings.
    ///
    /// The three rules keyed on `pvalue_locs` are fixed by a change that touches
    /// no rule: `validate`'s `match` carries a wildcard, so a statistic that is
    /// no longer a `Stat::PValue` is simply never indexed.
    #[test]
    fn a_declared_criterion_produces_no_finding() {
        let text = "Study\n\nMethods\nStatistical significance was determined at p < 0.05 \
                    unless otherwise stated.\n";
        let ex = crate::extract::extract_from_text(text);
        assert!(
            ex.statistics.iter().any(|s| matches!(
                s.stat,
                crate::extract::stats::Stat::SignificanceThreshold { .. }
            )),
            "fixture must extract a criterion: {:?}",
            ex.statistics
        );
        let report = validate(&ex);
        assert!(
            report.flags.is_empty(),
            "a threshold declaration reports no result, so nothing can be missing from it: {:?}",
            report.flags
        );
        assert!(report.passed);
    }

    /// The other direction, so the fix cannot be "stop flagging p-values".
    #[test]
    fn a_reported_result_still_produces_its_finding() {
        let text = "Study\n\nResults\nThe intervention improved recall (p = 0.002).\n";
        let ex = crate::extract::extract_from_text(text);
        let report = validate(&ex);
        assert!(
            report.flags.iter().any(|f| f.rule == RuleId::MissingEffectSize),
            "a genuine reported p-value must still be flagged: {:?}",
            report.flags
        );
    }

    /// **Both in one paragraph**, which is the case the paragraph unit would get
    /// wrong: the declaration is skipped and the result is still flagged.
    #[test]
    fn a_declaration_beside_a_result_flags_only_the_result() {
        let text = "Study\n\nResults\nSignificance was determined at p < 0.05. The \
                    intervention improved recall (p = 0.002).\n";
        let ex = crate::extract::extract_from_text(text);
        let report = validate(&ex);
        let missing: Vec<_> =
            report.flags.iter().filter(|f| f.rule == RuleId::MissingEffectSize).collect();
        assert_eq!(
            missing.len(),
            1,
            "exactly one MissingEffectSize — for the result, not the criterion: {:?}",
            report.flags
        );
    }

    // =======================================================================
    // §48–§49 — rule 3 asks a TYPED REGION query, not a text scan
    // =======================================================================

    /// **THE MEASURED DEFECT.** `\bf2\b` is Cohen's f²; a formulation paper
    /// names its gel batches F1…F4. Before shape 3, `is_match` over the
    /// paragraph text saw "F2", concluded an effect size was present, and
    /// suppressed the flag. Measured on a real manuscript: SIX suppressions,
    /// every one of them a batch label.
    #[test]
    fn a_bare_batch_label_no_longer_suppresses_the_missing_effect_size_flag() {
        let text = "Study\n\nResults\nBatch F2 was the optimized formulation and                     released faster than F1 and F3 (p = 0.01).\n";
        let ex = crate::extract::extract_from_text(text);
        assert!(
            !ex.statistics.iter().any(|s| matches!(s.stat, Stat::EffectSize { .. })),
            "no effect size is REPORTED here — only a batch label: {:?}",
            ex.statistics
        );
        let report = validate(&ex);
        assert!(
            report.flags.iter().any(|f| f.rule == RuleId::MissingEffectSize),
            "a p-value with no reported effect size must be flagged: {:?}",
            report.flags
        );
    }

    /// **THE LEGITIMATE SUPPRESSION MUST SURVIVE.** BMW PDSA ¶159 —
    /// "(+36.5 pp, Cohen's d = 2.14, p < 0.001)" — genuinely reports an effect
    /// size, and the flag must stay suppressed.
    #[test]
    fn a_reported_effect_size_still_suppresses_the_flag() {
        let text = "Study\n\nResults\nMedical doctors improved from 52.8% to 89.3%                     (+36.5 pp, Cohen's d = 2.14, p < 0.001).\n";
        let ex = crate::extract::extract_from_text(text);
        assert!(
            ex.statistics.iter().any(|s| matches!(s.stat, Stat::EffectSize { .. })),
            "the fixture must yield a typed effect size: {:?}",
            ex.statistics
        );
        let report = validate(&ex);
        assert!(
            !report.flags.iter().any(|f| f.rule == RuleId::MissingEffectSize),
            "a genuinely reported effect size must still suppress: {:?}",
            report.flags
        );
    }

    /// **A NAME WITHOUT A VALUE IS NOT A REPORTED EFFECT SIZE.** The old text
    /// scan matched the phrase "effect size" itself, so a Methods sentence
    /// describing the analysis suppressed the flag for every p-value beside it.
    #[test]
    fn naming_a_measure_without_reporting_one_does_not_suppress() {
        let text = "Study\n\nMethods\nEffect sizes were computed as Cohen's d where                     appropriate, and differences were tested (p = 0.02).\n";
        let ex = crate::extract::extract_from_text(text);
        let report = validate(&ex);
        assert!(
            report.flags.iter().any(|f| f.rule == RuleId::MissingEffectSize),
            "a NAME is not a reported value: {:?}",
            report.flags
        );
    }

    /// **THE QUERY IS A REGION, NOT A KEY** — §47.1's window form, kept.
    ///
    /// Today a `Region` is exactly one paragraph, so this asserts the shape
    /// rather than a behavioural difference: an effect size in a DIFFERENT
    /// paragraph does not satisfy the current region, and one in the SAME
    /// paragraph does. When item 1b widens what a region is, this test is what
    /// notices that the rule's answer changed with it.
    #[test]
    fn the_region_query_is_scoped_to_the_region_it_is_given() {
        use crate::extract::{has_effect_size_in, Location, Region};
        let text = "Study\n\nResults\nThe first paragraph reports a difference                     (p = 0.01).\n\nA later paragraph reports Cohen's d = 0.42.\n";
        let ex = crate::extract::extract_from_text(text);
        let p0 = Location { section: SectionKind::Results, paragraph: 0, section_index: None };
        let p1 = Location { section: SectionKind::Results, paragraph: 1, section_index: None };
        assert!(!has_effect_size_in(&ex, &Region::paragraph(&p0)), "not in ¶1's region");
        assert!(has_effect_size_in(&ex, &Region::paragraph(&p1)), "it is in ¶2's region");

        // and a region spanning both DOES contain it — the widening 1b would do
        let both = Region { section: SectionKind::Results, first_paragraph: 0, last_paragraph: 1 };
        assert!(has_effect_size_in(&ex, &both), "a widened region finds it");
    }
}