//! **The one-way ANOVA as an analysis of the user's own data (Research OS,
//! Phase 8G, decisions 312 to 317): `stats.analyze.one_way_anova`.**
//!
//! Not a reproduction of a reported result: that is Level 1's
//! `stats.verify.one_way_anova`. This module computes, on a numeric outcome
//! split by a group of 2 to [`MAX_GROUP_LEVELS`] levels:
//!
//! * the group table: n, mean, SD, SE, a 95% interval of the mean
//!   (`stats_ttest::interval_95`, `stats_dist::t_quantile_975` at n − 1),
//!   min and max;
//! * the ANOVA table: the between, within and total sums of squares, their
//!   df, the mean squares, F and its p;
//! * Welch's ANOVA, B. L. Welch, "On the comparison of several mean values:
//!   an alternative approach", Biometrika 38(3/4), 330–336, 1951,
//!   <https://doi.org/10.2307/2332579>: with wᵢ = nᵢ / sᵢ², W = Σ wᵢ, the
//!   weighted mean m = Σ wᵢ meanᵢ / W, λ = Σ (1 − wᵢ/W)² / (nᵢ − 1),
//!   F = [Σ wᵢ (meanᵢ − m)² / (k − 1)] / [1 + 2 (k − 2) λ / (k² − 1)],
//!   df1 = k − 1 and df2 = (k² − 1) / (3λ);
//! * Levene's test, H. Levene (1960) in Contributions to Probability and
//!   Statistics, Stanford University Press, 278–292, based on the mean, and
//!   based on the median (M. B. Brown and A. B. Forsythe, "Robust tests for
//!   the equality of variances", JASA 69(346), 364–367, 1974,
//!   <https://doi.org/10.1080/01621459.1974.10482955>): the ANOVA of
//!   |x − centreᵢ|, information and never advice;
//! * eta squared, SS between / SS total, and omega squared,
//!   (SS between − (k − 1) MS within) / (SS total + MS within), which can be
//!   negative and is shown as computed ([`AnovaCode::OmegaSquaredNegative`]);
//! * the post hoc tests the user ticks, none by default: Tukey HSD
//!   (Tukey–Kramer for unequal n), Games–Howell and Bonferroni (decision
//!   312's formulas, at each pair's own place in the level order: the first
//!   level minus the second).
//!
//! **One formula, one place (decisions 62 and 313).** F, its df and the two
//! sums of squares come from `stats_verify::one_way_anova` (its p, statrs's
//! 1 − cdf, is never read); every p of F from `stats_dist::f_upper`, every p
//! and critical value of the studentized range from
//! `stats_dist::studentized_range_upper` and `studentized_range_quantile`,
//! Bonferroni's from `stats_dist::t_two_tailed` and `t_quantile`; each
//! group's mean and variance from `stats_verify::mean` and
//! `sample_variance`, fed the values ascending, so permuting the rows changes
//! no bit. Levene's test is `stats_verify::one_way_anova` on the deviations,
//! each group's sorted ascending. Welch's ANOVA, the effect sizes and the
//! post hoc statistics are written once, here.
//!
//! **The data (decision 312).** The group's levels are
//! `stats_ttest::group_levels`', in code-point order (decision 78). All are
//! included unless the spec names some, at least two, by index; the rest are
//! left out as `other_level`. A row is left out for the first reason that
//! applies: `missing_group`, `other_level`, `missing_outcome`,
//! `missing_code` (a value the user ticked, the outcome only).
//!
//! **Edge cases are decided from the values before each call, never from an
//! error's text (decision 312).** A group of fewer than two values is refused
//! before any run (`group_too_small`). The outcome constant across every row
//! leaves only the group table (`outcome_constant`); every group constant
//! with different means gives the sums of squares and eta squared, and
//! nothing that divides by MS within (`within_variance_zero`); a group of
//! variance 0 makes Welch's ANOVA not computable (`welch_variance_zero`), and
//! a Games–Howell pair of two such groups (`pair_both_constant`); Levene's
//! test, for each centre on its own, is not computable when every group's
//! deviations are equal within the group, by construction in a group of two
//! and otherwise bit for bit (`levene_deviations_equal`).
//!
//! **Pure** (`tests/contracts_are_pure.rs`): no file, network, process,
//! clock or model. The run's check points are the caller's, one before the
//! arithmetic and one before each post hoc pair (decision 317).
//!
//! **Privacy.** [`AnovaOutcome`] holds the level labels, for the user's own
//! screen and export; its `Debug` prints them as counts. The values never
//! leave [`Split`], which derives nothing.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::ops::ControlFlow;

use serde::{Deserialize, Serialize};

use crate::contracts::codes::closed;
use crate::contracts::{
    CapabilityId, ContentHash, Diagnostic, Exclusion, OperationId, OperationSpec, ParamKey,
    ParamValue, Platform, Reproducibility, ResourceEstimate, RunManifest, SchemaVersion,
    ValidationReport, WorkspaceId,
};
use crate::stats_desc::{input_ref, Alias};
use crate::stats_dist::{
    f_upper, studentized_range_quantile, studentized_range_upper, t_quantile, t_two_tailed,
    SRANGE_P_FLOOR,
};
use crate::stats_l1::binding::suspect_codes_in;
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{column, group_levels, interval_95, non_finite_cells, MAX_GROUP_LEVELS};
use crate::stats_verify::{mean, one_way_anova, sample_variance, RangeFault};

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.one_way_anova";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The intervals' coverage: each group's, Tukey's and Games–Howell's
/// simultaneous ones, and Bonferroni's at 1 − 0.05 / m per pair.
pub const CONFIDENCE_LEVEL: f64 = 0.95;
/// The most levels a post hoc test takes (decisions 317 and 322), the one
/// place the cap lives. Measured in a release build at 50 levels and
/// 100,000 rows, all three tests, 31 Oct 2026: 29.07 to 29.31 s idle before
/// decision 322's changes to `stats_dist::studentized_range`, 8.33 to
/// 8.39 s over three runs after (Games-Howell 7.9 s, Tukey 0.46 s), within
/// decision 322's 15 s, so the cap is the group's own limit.
pub const POST_HOC_MAX_LEVELS: usize = MAX_GROUP_LEVELS;

closed! {
    /// Every finding, outcome, exclusion, note and limitation of the one-way
    /// ANOVA.
    pub enum AnovaCode in "stats.anova." {
        /// The spec names another capability.
        UnknownCapability => "unknown_capability",
        /// The spec was written for another version of this capability.
        CapabilityVersionMismatch => "capability_version_mismatch",
        /// The spec does not name exactly one input.
        InputCount => "input_count",
        /// The spec's input is not this dataset.
        InputMismatch => "input_mismatch",
        /// No column has that alias. Param `column`.
        UnknownColumn => "unknown_column",
        /// The outcome and the group are one column.
        SameColumn => "same_column",
        /// The outcome holds text. Params `column`, `count`.
        OutcomeHoldsText => "outcome_holds_text",
        /// The outcome holds NaN or an infinity. Params `column`, `count`.
        OutcomeNonFinite => "outcome_non_finite",
        /// The group has fewer than two levels. Params `column`, `count`.
        GroupOneLevel => "group_one_level",
        /// The group has more levels than a group may. Params `count`,
        /// `limit`.
        TooManyLevels => "too_many_levels",
        /// A level index is not one of the group's. Params `index`, `limit`.
        LevelOutOfRange => "level_out_of_range",
        /// A level is named twice. Param `index`.
        LevelRepeated => "level_repeated",
        /// Fewer than two levels are included. Param `count`.
        TooFewLevels => "too_few_levels",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// A candidate missing code the rule found in the outcome and the
        /// user did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Refused: an included group has fewer than two values after the
        /// exclusions. Params `index` (the level), `count`.
        GroupTooSmall => "group_too_small",
        /// Refused: a post hoc test was asked for on more levels than it
        /// takes (decision 317). Params `count`, `limit`.
        PostHocTooManyLevels => "post_hoc_too_many_levels",
        /// Not computable: the outcome is the same number in every row, so
        /// the total sum of squares is 0.
        OutcomeConstant => "outcome_constant",
        /// Not computable: every group is constant, so MS within is 0.
        WithinVarianceZero => "within_variance_zero",
        /// Not computable: Welch's ANOVA weights a group by n over its
        /// variance, and a group's variance is 0.
        WelchVarianceZero => "welch_variance_zero",
        /// Not computable: every group's deviations from its centre are
        /// equal within the group, so Levene's MS within is 0.
        LeveneDeviationsEqual => "levene_deviations_equal",
        /// Not computable: both groups of a Games–Howell pair have a
        /// variance of 0.
        PairBothConstant => "pair_both_constant",
        /// Not computable: a distribution is not measured at this many
        /// degrees of freedom (above 1e7).
        DistributionOutOfRange => "distribution_out_of_range",
        /// Not computable: a sum, square, quotient or root a double cannot
        /// hold.
        Overflow => "overflow",
        /// Not computable: a sum of squares underflowed though the values
        /// differ.
        Underflow => "underflow",
        /// Not computable: the arithmetic refused for a reason it did not
        /// name.
        ArithmeticRefused => "arithmetic_refused",
        /// Note: a studentized-range p is below the floor it is computed to
        /// (`stats_dist::SRANGE_P_FLOOR`, 1e-10), and is shown as below it.
        PBelowFloor => "p_below_floor",
        /// Note: omega squared is negative, as computed: MS between is
        /// below MS within.
        OmegaSquaredNegative => "omega_squared_negative",
        /// Excluded: the row's group is empty or a missing-value token.
        MissingGroup => "missing_group",
        /// Excluded: the row's group is a level left out.
        OtherLevel => "other_level",
        /// Excluded: the row's outcome is empty or a missing-value token.
        MissingOutcome => "missing_outcome",
        /// Excluded: the row's outcome is a value the user listed.
        MissingCode => "missing_code",
        /// Limitation: normality is not assessed.
        NormalityNotAssessed => "normality_not_assessed",
        /// Limitation: eta and omega squared have no confidence interval.
        EffectSizesHaveNoInterval => "effect_sizes_have_no_interval",
        /// Limitation: no Brown–Forsythe test of means, no trend tests, no
        /// planned contrasts.
        NoContrasts => "no_contrasts",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// A post hoc test the user may tick. None is ticked by default; Gaply says
/// which the user chose, never which they should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostHoc {
    /// Tukey's HSD, Tukey–Kramer for unequal n: pooled MS within, df N − k.
    Tukey,
    /// Games–Howell: each pair's own variances and Welch df.
    GamesHowell,
    /// Bonferroni: pooled MS within, df N − k, p times the number of pairs,
    /// at most 1.
    Bonferroni,
}

/// The one-way ANOVA's parameters. `levels` names the included levels by
/// their index in code-point order; `None` includes them all. `post_hoc`
/// lists the tests the user ticked. `missing_codes` apply to the outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnovaParams {
    pub outcome: Alias,
    pub group: Alias,
    pub levels: Option<Vec<u32>>,
    pub post_hoc: Vec<PostHoc>,
    pub missing_codes: Vec<f64>,
}

/* ------------------------------------------------------------------------ *
 *  The result
 * ------------------------------------------------------------------------ */

/// A number, or the closed reason it is not defined.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Value(f64),
    NotComputable(AnovaCode),
}

/// A studentized-range or Bonferroni p: a number, or below the floor the
/// studentized range is computed to (decision 314), never shown as a number.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostHocP {
    Value(f64),
    Below(f64),
}

/// One included group. The label is the user's own text.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRow {
    pub level: u32,
    pub label: String,
    pub n: u64,
    pub mean: f64,
    pub sd: f64,
    pub se: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    pub min: f64,
    pub max: f64,
}

impl std::fmt::Debug for GroupRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroupRow")
            .field("level", &self.level)
            .field("label_chars", &self.label.chars().count())
            .field("n", &self.n)
            .finish_non_exhaustive()
    }
}

/// The ANOVA table.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnovaTable {
    pub ss_between: f64,
    pub ss_within: f64,
    pub ss_total: f64,
    pub df_between: f64,
    pub df_within: f64,
    pub ms_between: f64,
    pub ms_within: f64,
    pub f: Stat,
    pub p: Stat,
}

/// The ANOVA table, or why there is none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Table {
    Computed(AnovaTable),
    NotComputable(AnovaCode),
}

/// An F test: Welch's ANOVA, or Levene's on one centre.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FTest {
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

/// An F test, or why there is none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FOutcome {
    Computed(FTest),
    NotComputable(AnovaCode),
}

/// One pair's post hoc numbers: the first level minus the second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairStats {
    pub difference: f64,
    /// The difference's standard error.
    pub se: f64,
    /// q for Tukey and Games–Howell, t for Bonferroni.
    pub statistic: f64,
    pub df: f64,
    pub p: PostHocP,
    pub ci_lower: f64,
    pub ci_upper: f64,
}

/// One pair, by the levels' indices, first before second in the level
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    pub first: u32,
    pub second: u32,
    pub outcome: PairOutcome,
}

/// A pair's numbers, or why it has none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PairOutcome {
    Computed(PairStats),
    NotComputable(AnovaCode),
}

/// One chosen post hoc test's pairs, or why it has none.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostHocTable {
    pub test: PostHoc,
    pub pairs: PostHocPairs,
}

/// The pairs of a post hoc test, or why there are none.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostHocPairs {
    Computed(Vec<Pair>),
    NotComputable(AnovaCode),
}

/// The one-way ANOVA's result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnovaOutcome {
    /// In the level order.
    pub groups: Vec<GroupRow>,
    pub table: Table,
    pub welch: FOutcome,
    pub levene_mean: FOutcome,
    pub levene_median: FOutcome,
    pub eta_squared: Stat,
    pub omega_squared: Stat,
    /// The chosen tests, in [`PostHoc`]'s order.
    pub post_hoc: Vec<PostHocTable>,
    /// The rows left out, by reason; reasons with no row are not listed.
    pub exclusions: Vec<Exclusion<AnovaCode>>,
    /// `p_below_floor` and `omega_squared_negative`, when they apply.
    pub notes: Vec<AnovaCode>,
    pub limitations: Vec<AnovaCode>,
}

impl AnovaOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and strings serialises")
    }
}

/// What every result says it did not assess.
const LIMITATIONS: [AnovaCode; 3] = [
    AnovaCode::NormalityNotAssessed,
    AnovaCode::EffectSizesHaveNoInterval,
    AnovaCode::NoContrasts,
];

/* ------------------------------------------------------------------------ *
 *  The one splitting pass
 * ------------------------------------------------------------------------ */

/// The outcome's values in each included group, each ascending, and the
/// rows left out. Never leaves this module, and derives nothing.
struct Split {
    values: Vec<Vec<f64>>,
    /// `missing_group`, `other_level`, `missing_outcome`, `missing_code`.
    excluded: [u64; 4],
}

const EXCLUSION_CODES: [AnovaCode; 4] = [
    AnovaCode::MissingGroup,
    AnovaCode::OtherLevel,
    AnovaCode::MissingOutcome,
    AnovaCode::MissingCode,
];

fn split(outcome: &[Cell], group: &[Cell], included: &[&str], codes: &[f64]) -> Split {
    let position: BTreeMap<&str, usize> =
        included.iter().enumerate().map(|(i, l)| (*l, i)).collect();
    let mut s = Split {
        values: vec![Vec::new(); included.len()],
        excluded: [0; 4],
    };
    for (y, g) in outcome.iter().zip(group) {
        let Some(level) = g.level_code() else {
            s.excluded[0] += 1;
            continue;
        };
        let Some(&k) = position.get(level.as_str()) else {
            s.excluded[1] += 1;
            continue;
        };
        match y {
            Cell::Missing => s.excluded[2] += 1,
            Cell::Number(v) if codes.contains(v) => s.excluded[3] += 1,
            Cell::Number(v) => s.values[k].push(*v),
            // Refused by validation before any split is used.
            Cell::Text(_) => s.excluded[2] += 1,
        }
    }
    for v in &mut s.values {
        v.sort_by(f64::total_cmp);
    }
    s
}

/* ------------------------------------------------------------------------ *
 *  validate
 * ------------------------------------------------------------------------ */

/// The capability's id, parsed.
pub fn capability_id() -> CapabilityId {
    CapabilityId::parse(CAPABILITY_ID).expect("the capability id is well formed")
}

/// A spec for this capability on this dataset.
pub fn spec(
    operation_id: OperationId,
    dataset: &Dataset,
    parameters: AnovaParams,
) -> OperationSpec<AnovaParams> {
    OperationSpec {
        schema_version: SchemaVersion,
        operation_id,
        workspace_id: WorkspaceId::Statistics,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        inputs: vec![input_ref(dataset)],
        parameters,
        random_seed: None,
        confirmation_refs: Vec::new(),
    }
}

fn column_param(alias: Alias) -> ParamValue<AnovaCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

struct Checked<'d> {
    errors: Vec<Diagnostic<AnovaCode>>,
    ready: Option<Ready<'d>>,
}

struct Ready<'d> {
    outcome: &'d [Cell],
    group: &'d [Cell],
    labels: Vec<String>,
    included: Vec<u32>,
}

impl Ready<'_> {
    fn split(&self, codes: &[f64]) -> Split {
        let included: Vec<&str> = self
            .included
            .iter()
            .map(|i| self.labels[*i as usize].as_str())
            .collect();
        split(self.outcome, self.group, &included, codes)
    }
}

/// Shared by `validate` and `run`: the errors in a fixed order (missing
/// codes, the columns, the levels, the post hoc cap, then the group sizes).
fn check<'d>(dataset: &'d Dataset, params: &AnovaParams) -> Checked<'d> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(AnovaCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
            );
        }
    }
    let outcome = column(dataset, params.outcome);
    let group = column(dataset, params.group);
    for (alias, found) in [
        (params.outcome, outcome.is_some()),
        (params.group, group.is_some()),
    ] {
        if !found {
            errors.push(
                Diagnostic::error(AnovaCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(alias)),
            );
        }
    }
    let mut ready = None;
    if let (Some((o_summary, o_cells)), Some((_, g_cells))) = (outcome, group) {
        if params.outcome == params.group {
            errors.push(Diagnostic::error(AnovaCode::SameColumn));
        } else {
            let non_finite = non_finite_cells(o_summary);
            let texts = o_cells
                .iter()
                .filter(|c| matches!(c, Cell::Text(_)))
                .count() as i64;
            if non_finite > 0 {
                errors.push(
                    Diagnostic::error(AnovaCode::OutcomeNonFinite)
                        .with(ParamKey::Column, column_param(params.outcome))
                        .int(ParamKey::Count, non_finite as i64),
                );
            } else if texts > 0 {
                errors.push(
                    Diagnostic::error(AnovaCode::OutcomeHoldsText)
                        .with(ParamKey::Column, column_param(params.outcome))
                        .int(ParamKey::Count, texts),
                );
            }
            let labels: Vec<String> = group_levels(dataset, params.group)
                .expect("the group column exists")
                .into_iter()
                .map(|(label, _)| label)
                .collect();
            let k = labels.len() as u32;
            if k < 2 {
                errors.push(
                    Diagnostic::error(AnovaCode::GroupOneLevel)
                        .with(ParamKey::Column, column_param(params.group))
                        .int(ParamKey::Count, i64::from(k)),
                );
            } else if labels.len() > MAX_GROUP_LEVELS {
                errors.push(
                    Diagnostic::error(AnovaCode::TooManyLevels)
                        .int(ParamKey::Count, i64::from(k))
                        .int(ParamKey::Limit, MAX_GROUP_LEVELS as i64),
                );
            } else {
                let mut included = match &params.levels {
                    None => (0..k).collect::<Vec<u32>>(),
                    Some(levels) => levels.clone(),
                };
                let mut ok = true;
                for i in &included {
                    if *i >= k {
                        ok = false;
                        errors.push(
                            Diagnostic::error(AnovaCode::LevelOutOfRange)
                                .int(ParamKey::Index, i64::from(*i))
                                .int(ParamKey::Limit, i64::from(k - 1)),
                        );
                    }
                }
                included.sort_unstable();
                for w in included.windows(2) {
                    if w[0] == w[1] {
                        ok = false;
                        errors.push(
                            Diagnostic::error(AnovaCode::LevelRepeated)
                                .int(ParamKey::Index, i64::from(w[0])),
                        );
                    }
                }
                if included.len() < 2 {
                    ok = false;
                    errors.push(
                        Diagnostic::error(AnovaCode::TooFewLevels)
                            .int(ParamKey::Count, included.len() as i64),
                    );
                }
                if ok && !params.post_hoc.is_empty() && included.len() > POST_HOC_MAX_LEVELS {
                    errors.push(
                        Diagnostic::error(AnovaCode::PostHocTooManyLevels)
                            .int(ParamKey::Count, included.len() as i64)
                            .int(ParamKey::Limit, POST_HOC_MAX_LEVELS as i64),
                    );
                }
                if ok {
                    ready = Some(Ready {
                        outcome: o_cells,
                        group: g_cells,
                        labels,
                        included,
                    });
                }
            }
        }
    }
    if errors.is_empty() {
        if let Some(r) = &ready {
            let s = r.split(&params.missing_codes);
            for (k, values) in s.values.iter().enumerate() {
                if values.len() < 2 {
                    errors.push(
                        Diagnostic::error(AnovaCode::GroupTooSmall)
                            .int(ParamKey::Index, i64::from(r.included[k]))
                            .int(ParamKey::Count, values.len() as i64),
                    );
                }
            }
        }
    }
    Checked { errors, ready }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for a candidate missing code in the outcome that the
/// user did not list. Nothing needs input: every level is included unless
/// the spec says otherwise, and no post hoc test runs unless it is ticked.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<AnovaParams>,
) -> ValidationReport<AnovaCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(AnovaCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(AnovaCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(AnovaCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(AnovaCode::InputCount).int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    if let Some((_, cells)) = column(dataset, params.outcome) {
        for s in suspect_codes_in(params.outcome.0, cells) {
            if !params.missing_codes.contains(&f64::from(s.code)) {
                warnings.push(
                    Diagnostic::warning(AnovaCode::SuspectMissingCode)
                        .with(ParamKey::Column, column_param(params.outcome))
                        .int(ParamKey::Value, i64::from(s.code))
                        .int(ParamKey::Count, s.cells as i64),
                );
            }
        }
    }
    let rows = dataset.rows() as u64;
    let estimate = ResourceEstimate {
        rows,
        columns: 2,
        cells: rows.saturating_mul(2),
    };
    ValidationReport::new(unsupported, errors, Vec::new(), warnings, estimate)
}

/* ------------------------------------------------------------------------ *
 *  run
 * ------------------------------------------------------------------------ */

/// Why [`run`] returned no result.
#[derive(Debug, Clone, PartialEq)]
pub enum Halt<B> {
    /// The parameters are wrong for this dataset.
    Refused(Diagnostic<AnovaCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run of `params` on `levels` included levels
/// calls: one before the arithmetic, and one before each pair of each
/// chosen post hoc test (decision 317). The run manager's progress counts
/// them.
pub fn check_points(params: &AnovaParams, levels: usize) -> usize {
    let mut tests = params.post_hoc.clone();
    tests.sort_unstable();
    tests.dedup();
    1 + tests.len() * levels * levels.saturating_sub(1) / 2
}

/// [`run`] with a check point that never stops.
pub fn compute(
    dataset: &Dataset,
    params: &AnovaParams,
) -> Result<AnovaOutcome, Diagnostic<AnovaCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The one-way ANOVA. `checkpoint(0)` is called after the parameters are
/// checked and before the arithmetic, and `checkpoint(i)` before the i-th
/// post hoc pair, counted across the chosen tests from 1; a `Break` stops
/// the run there. What a stop means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &AnovaParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<AnovaOutcome, Halt<B>> {
    let checked = check(dataset, params);
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let ready = checked
        .ready
        .expect("no error means the columns and the levels are good");
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let s = ready.split(&params.missing_codes);
    let mut tests = params.post_hoc.clone();
    tests.sort_unstable();
    tests.dedup();
    let mut notes = Vec::new();
    let groups = group_rows(&s.values, &ready).map_err(refuse)?;
    let k = s.values.len();
    let n_total: usize = s.values.iter().map(Vec::len).sum();
    let sorted = Summary::of(&s.values).map_err(refuse)?;
    let dead = if sorted.all_equal {
        Some(AnovaCode::OutcomeConstant)
    } else if sorted.every_group_constant {
        Some(AnovaCode::WithinVarianceZero)
    } else {
        None
    };
    let (table, effects) = anova_table(&s.values, &sorted, k, n_total).map_err(refuse)?;
    if matches!(effects.1, Stat::Value(w) if w < 0.0) {
        notes.push(AnovaCode::OmegaSquaredNegative);
    }
    let welch = match dead {
        Some(code) => FOutcome::NotComputable(code),
        None if sorted.variances.iter().any(|v| *v == 0.0) => {
            FOutcome::NotComputable(AnovaCode::WelchVarianceZero)
        }
        None => welch_anova(&sorted).map_err(refuse)?,
    };
    let levene = |centre: Centre| match dead {
        Some(code) => Ok(FOutcome::NotComputable(code)),
        None => levene(&s.values, centre),
    };
    let levene_mean = levene(Centre::Mean).map_err(refuse)?;
    let levene_median = levene(Centre::Median).map_err(refuse)?;
    let mut post_hoc = Vec::new();
    let mut point = 0usize;
    let mut below = false;
    for test in tests {
        let pairs = match (dead, &table) {
            (Some(code), _) => PostHocPairs::NotComputable(code),
            (None, Table::Computed(t)) => {
                let ctx = PostHocContext {
                    sorted: &sorted,
                    k,
                    ms_within: t.ms_within,
                    df_within: t.df_within,
                };
                let mut pairs = Vec::with_capacity(k * (k - 1) / 2);
                let critical = ctx.critical(test).map_err(refuse)?;
                for i in 0..k {
                    for j in i + 1..k {
                        point += 1;
                        if let ControlFlow::Break(reason) = checkpoint(point) {
                            return Err(Halt::Stopped { reason });
                        }
                        let outcome = ctx.pair(test, i, j, critical).map_err(refuse)?;
                        if let PairOutcome::Computed(PairStats {
                            p: PostHocP::Below(_),
                            ..
                        }) = outcome
                        {
                            below = true;
                        }
                        pairs.push(Pair {
                            first: ready.included[i],
                            second: ready.included[j],
                            outcome,
                        });
                    }
                }
                PostHocPairs::Computed(pairs)
            }
            (None, Table::NotComputable(code)) => PostHocPairs::NotComputable(*code),
        };
        post_hoc.push(PostHocTable { test, pairs });
    }
    if below {
        notes.push(AnovaCode::PBelowFloor);
    }
    let exclusions = EXCLUSION_CODES
        .iter()
        .zip(s.excluded)
        .filter(|(_, n)| *n > 0)
        .map(|(code, count)| {
            let alias = match code {
                AnovaCode::MissingGroup | AnovaCode::OtherLevel => params.group,
                _ => params.outcome,
            };
            Exclusion {
                code: *code,
                count,
                params: BTreeMap::from([(ParamKey::Column, column_param(alias))]),
            }
        })
        .collect();
    Ok(AnovaOutcome {
        groups,
        table,
        welch,
        levene_mean,
        levene_median,
        eta_squared: effects.0,
        omega_squared: effects.1,
        post_hoc,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    })
}

fn refuse<B>(code: AnovaCode) -> Halt<B> {
    Halt::Refused(Diagnostic::error(code))
}

/// `v`, or `overflow` when it is infinite or NaN.
fn finite(v: f64) -> Result<f64, AnovaCode> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(AnovaCode::Overflow)
    }
}

/// A refusal from `stats_verify`, by the range fault it carries.
fn refused(fault: Option<RangeFault>) -> AnovaCode {
    match fault {
        Some(RangeFault::Overflow) => AnovaCode::Overflow,
        Some(RangeFault::Underflow) => AnovaCode::Underflow,
        None => AnovaCode::ArithmeticRefused,
    }
}

/// Each group's n, mean and variance, from `stats_verify` on the ascending
/// values, and what is constant.
struct Summary {
    ns: Vec<f64>,
    means: Vec<f64>,
    variances: Vec<f64>,
    all_equal: bool,
    every_group_constant: bool,
}

impl Summary {
    fn of(values: &[Vec<f64>]) -> Result<Summary, AnovaCode> {
        let mut s = Summary {
            ns: Vec::new(),
            means: Vec::new(),
            variances: Vec::new(),
            all_equal: true,
            every_group_constant: true,
        };
        let first = values[0][0];
        for v in values {
            s.ns.push(v.len() as f64);
            s.means.push(finite(mean(v))?);
            s.variances
                .push(sample_variance(v).map_err(|e| refused(RangeFault::of(&e)))?);
            let constant = v[0] == v[v.len() - 1];
            s.every_group_constant &= constant;
            s.all_equal &= constant && v[0] == first;
        }
        Ok(s)
    }
}

/// The group table.
fn group_rows(values: &[Vec<f64>], ready: &Ready<'_>) -> Result<Vec<GroupRow>, AnovaCode> {
    values
        .iter()
        .zip(&ready.included)
        .map(|(v, level)| {
            let n = v.len();
            let m = finite(mean(v))?;
            let var = sample_variance(v).map_err(|e| refused(RangeFault::of(&e)))?;
            let sd = finite(var.sqrt())?;
            let se = finite(sd / (n as f64).sqrt())?;
            let [lo, hi] = interval_95(
                m,
                se,
                (n - 1) as f64,
                AnovaCode::DistributionOutOfRange,
                AnovaCode::Overflow,
            )?;
            Ok(GroupRow {
                level: *level,
                label: ready.labels[*level as usize].clone(),
                n: n as u64,
                mean: m,
                sd,
                se,
                ci_lower: lo,
                ci_upper: hi,
                min: v[0],
                max: v[n - 1],
            })
        })
        .collect()
}

/// The ANOVA table, eta squared and omega squared.
fn anova_table(
    values: &[Vec<f64>],
    sorted: &Summary,
    k: usize,
    n_total: usize,
) -> Result<(Table, (Stat, Stat)), AnovaCode> {
    let df_between = (k - 1) as f64;
    let df_within = (n_total - k) as f64;
    if sorted.all_equal {
        let nc = Stat::NotComputable(AnovaCode::OutcomeConstant);
        return Ok((Table::NotComputable(AnovaCode::OutcomeConstant), (nc, nc)));
    }
    if sorted.every_group_constant {
        // MS within is 0, so stats_verify::one_way_anova refuses: the
        // within sum is 0 and the between sum is the total sum, (N − 1)
        // times the sample variance of every value (stats_verify's).
        let mut all: Vec<f64> = values.iter().flatten().copied().collect();
        all.sort_by(f64::total_cmp);
        let ss_total = finite(
            sample_variance(&all).map_err(|e| refused(RangeFault::of(&e)))? * (n_total - 1) as f64,
        )?;
        let nc = Stat::NotComputable(AnovaCode::WithinVarianceZero);
        let table = AnovaTable {
            ss_between: ss_total,
            ss_within: 0.0,
            ss_total,
            df_between,
            df_within,
            ms_between: finite(ss_total / df_between)?,
            ms_within: 0.0,
            f: nc,
            p: nc,
        };
        let eta = Stat::Value(finite(ss_total / ss_total)?);
        return Ok((Table::Computed(table), (eta, nc)));
    }
    let refs: Vec<&[f64]> = values.iter().map(Vec::as_slice).collect();
    let a = one_way_anova(&refs).map_err(|e| refused(RangeFault::of(&e)))?;
    let ss_total = finite(a.ss_between + a.ss_within)?;
    let ms_within = finite(a.ss_within / a.df_within)?;
    let p = match f_upper(a.f, a.df_between, a.df_within) {
        Some(p) => Stat::Value(p),
        None => Stat::NotComputable(AnovaCode::DistributionOutOfRange),
    };
    let table = AnovaTable {
        ss_between: a.ss_between,
        ss_within: a.ss_within,
        ss_total,
        df_between: a.df_between,
        df_within: a.df_within,
        ms_between: finite(a.ss_between / a.df_between)?,
        ms_within,
        f: Stat::Value(a.f),
        p,
    };
    let eta = finite(a.ss_between / ss_total)?;
    let omega = finite((a.ss_between - a.df_between * ms_within) / (ss_total + ms_within))?;
    Ok((
        Table::Computed(table),
        (Stat::Value(eta), Stat::Value(omega)),
    ))
}

/// Welch's ANOVA (the module doc's formula). Every variance is above 0,
/// checked before.
fn welch_anova(s: &Summary) -> Result<FOutcome, AnovaCode> {
    let k = s.ns.len() as f64;
    let w: Vec<f64> =
        s.ns.iter()
            .zip(&s.variances)
            .map(|(n, v)| finite(n / v))
            .collect::<Result<_, _>>()?;
    let total: f64 = finite(w.iter().sum())?;
    let weighted = finite(w.iter().zip(&s.means).map(|(w, m)| w * m).sum::<f64>() / total)?;
    let a = finite(
        w.iter()
            .zip(&s.means)
            .map(|(w, m)| w * (m - weighted) * (m - weighted))
            .sum::<f64>()
            / (k - 1.0),
    )?;
    let lambda = finite(
        w.iter()
            .zip(&s.ns)
            .map(|(w, n)| (1.0 - w / total) * (1.0 - w / total) / (n - 1.0))
            .sum::<f64>(),
    )?;
    let b = finite(1.0 + 2.0 * (k - 2.0) * lambda / (k * k - 1.0))?;
    let f = finite(a / b)?;
    let df2 = finite((k * k - 1.0) / (3.0 * lambda))?;
    Ok(match f_upper(f, k - 1.0, df2) {
        Some(p) => FOutcome::Computed(FTest {
            f,
            df1: k - 1.0,
            df2,
            p,
        }),
        None => FOutcome::NotComputable(AnovaCode::DistributionOutOfRange),
    })
}

#[derive(Clone, Copy)]
enum Centre {
    Mean,
    Median,
}

/// The median of ascending values: the middle one, or the mean of the two
/// middle ones (decision 313).
fn median(v: &[f64]) -> f64 {
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Levene's test on one centre: `stats_verify::one_way_anova` on the
/// absolute deviations, each group's sorted ascending, and its p from
/// `stats_dist::f_upper`.
fn levene(values: &[Vec<f64>], centre: Centre) -> Result<FOutcome, AnovaCode> {
    let mut deviations = Vec::with_capacity(values.len());
    let mut all_equal_within = true;
    for v in values {
        let c = match centre {
            Centre::Mean => finite(mean(v))?,
            Centre::Median => finite(median(v))?,
        };
        let mut d: Vec<f64> = v.iter().map(|x| (x - c).abs()).collect::<Vec<_>>();
        for x in &d {
            finite(*x)?;
        }
        d.sort_by(f64::total_cmp);
        // Equal within the group: by construction in a group of two (both
        // centres are its mean), otherwise bit for bit.
        all_equal_within &= d.len() == 2 || d[0] == d[d.len() - 1];
        deviations.push(d);
    }
    if all_equal_within {
        return Ok(FOutcome::NotComputable(AnovaCode::LeveneDeviationsEqual));
    }
    let refs: Vec<&[f64]> = deviations.iter().map(Vec::as_slice).collect();
    let a = one_way_anova(&refs).map_err(|e| refused(RangeFault::of(&e)))?;
    Ok(match f_upper(a.f, a.df_between, a.df_within) {
        Some(p) => FOutcome::Computed(FTest {
            f: a.f,
            df1: a.df_between,
            df2: a.df_within,
            p,
        }),
        None => FOutcome::NotComputable(AnovaCode::DistributionOutOfRange),
    })
}

/// What every pair of a post hoc test reads.
struct PostHocContext<'a> {
    sorted: &'a Summary,
    k: usize,
    ms_within: f64,
    df_within: f64,
}

impl PostHocContext<'_> {
    fn pairs(&self) -> f64 {
        (self.k * (self.k - 1) / 2) as f64
    }

    /// The critical value every pair of the test shares: Tukey's q at 0.05
    /// with df N − k, Bonferroni's t at 0.05 / m with df N − k; Games–Howell
    /// has none (each pair has its own df).
    fn critical(&self, test: PostHoc) -> Result<f64, AnovaCode> {
        match test {
            PostHoc::Tukey => studentized_range_quantile(0.05, self.k as u32, self.df_within)
                .ok_or(AnovaCode::DistributionOutOfRange),
            PostHoc::Bonferroni => t_quantile(0.05 / self.pairs(), self.df_within)
                .ok_or(AnovaCode::DistributionOutOfRange),
            PostHoc::GamesHowell => Ok(0.0),
        }
    }

    /// The studentized-range p of `q` on `df`, below the floor shown as
    /// below it.
    fn range_p(&self, q: f64, df: f64) -> Result<PostHocP, AnovaCode> {
        let p = studentized_range_upper(q, self.k as u32, df)
            .ok_or(AnovaCode::DistributionOutOfRange)?;
        Ok(if p < SRANGE_P_FLOOR {
            PostHocP::Below(SRANGE_P_FLOOR)
        } else {
            PostHocP::Value(p)
        })
    }

    fn pair(
        &self,
        test: PostHoc,
        i: usize,
        j: usize,
        critical: f64,
    ) -> Result<PairOutcome, AnovaCode> {
        let s = self.sorted;
        let (ni, nj) = (s.ns[i], s.ns[j]);
        let difference = finite(s.means[i] - s.means[j])?;
        let stats = match test {
            PostHoc::Tukey => {
                let se = finite((self.ms_within * (1.0 / ni + 1.0 / nj)).sqrt())?;
                let q = finite(std::f64::consts::SQRT_2 * difference.abs() / se)?;
                let half = finite(critical * se / std::f64::consts::SQRT_2)?;
                PairStats {
                    difference,
                    se,
                    statistic: q,
                    df: self.df_within,
                    p: self.range_p(q, self.df_within)?,
                    ci_lower: finite(difference - half)?,
                    ci_upper: finite(difference + half)?,
                }
            }
            PostHoc::GamesHowell => {
                if s.variances[i] == 0.0 && s.variances[j] == 0.0 {
                    return Ok(PairOutcome::NotComputable(AnovaCode::PairBothConstant));
                }
                let a = finite(s.variances[i] / ni)?;
                let b = finite(s.variances[j] / nj)?;
                let se = finite((a + b).sqrt())?;
                let df = finite((a + b) * (a + b) / (a * a / (ni - 1.0) + b * b / (nj - 1.0)))?;
                let q = finite(std::f64::consts::SQRT_2 * difference.abs() / se)?;
                let crit = studentized_range_quantile(0.05, self.k as u32, df)
                    .ok_or(AnovaCode::DistributionOutOfRange)?;
                let half = finite(crit * se / std::f64::consts::SQRT_2)?;
                PairStats {
                    difference,
                    se,
                    statistic: q,
                    df,
                    p: self.range_p(q, df)?,
                    ci_lower: finite(difference - half)?,
                    ci_upper: finite(difference + half)?,
                }
            }
            PostHoc::Bonferroni => {
                let se = finite((self.ms_within * (1.0 / ni + 1.0 / nj)).sqrt())?;
                let t = finite(difference / se)?;
                let p = t_two_tailed(t, self.df_within).ok_or(AnovaCode::DistributionOutOfRange)?;
                let half = finite(critical * se)?;
                PairStats {
                    difference,
                    se,
                    statistic: t,
                    df: self.df_within,
                    p: PostHocP::Value((p * self.pairs()).min(1.0)),
                    ci_lower: finite(difference - half)?,
                    ci_upper: finite(difference + half)?,
                }
            }
        };
        Ok(PairOutcome::Computed(stats))
    }
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows
/// used are the groups' n; with the exclusions and the data rows they add
/// up. The output hash is SHA-256 of [`AnovaOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<AnovaParams>,
    dataset: &Dataset,
    result: &AnovaOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<AnovaCode> {
    let parameters = serde_json::to_vec(&spec.parameters)
        .expect("parameters of aliases, indices, words and numbers serialise");
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        inputs: spec.inputs.iter().map(|i| i.content_hash.clone()).collect(),
        parameters_hash: ContentHash::of_bytes(&parameters),
        seed: spec.random_seed,
        rows_read: dataset.rows() as u64,
        rows_used: result.groups.iter().map(|g| g.n).sum(),
        exclusions: result.exclusions.clone(),
        platform,
        elapsed_ms,
        output_hash: ContentHash::of_bytes(&result.canonical_bytes()),
        reproducibility: Reproducibility::Bitwise,
    }
}
