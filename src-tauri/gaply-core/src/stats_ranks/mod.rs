//! **The Mann-Whitney and Kruskal-Wallis tests as an analysis of the user's
//! own data (Research OS, Phase 8Y-1, decisions 601 to 604):
//! `stats.analyze.independent_ranks`.**
//!
//! One method with the one-way ANOVA's inputs, a numeric outcome split by a
//! group of 2 to [`MAX_GROUP_LEVELS`] levels, compared by the ranks of its
//! values. The test is chosen by the count of included levels, as SPSS's
//! independent-samples nonparametric tests choose:
//!
//! * two, the Mann-Whitney U test, H. B. Mann and D. R. Whitney, "On a test
//!   of whether one of two random variables is stochastically larger than
//!   the other", Annals of Mathematical Statistics 18(1), 50–60, 1947,
//!   <https://doi.org/10.1214/aoms/1177730491>: the first group is the
//!   included level with the lower index. U1 = R1 − n1 (n1 + 1) / 2, R's W;
//!   U2 = n1 n2 − U1; z = (U1 − n1 n2 / 2) / σ with
//!   σ² = (n1 n2 / 12) ((N + 1) − Σ(t³ − t) / (N (N − 1))), no continuity
//!   correction; its two-sided p, `stats_dist::normal_two_tailed`; the exact
//!   two-sided p, min(1, 2 min(P(U ≤ U1), P(U ≥ U1))) from
//!   `stats_dist::rank_sum_exact_tails`, only when no two included values are
//!   tied and each group has fewer than 50; the rank-biserial correlation,
//!   2 U1 / (n1 n2) − 1, positive when the first group's values tend to be
//!   the larger (D. S. Kerby, "The simple difference formula: an approach to
//!   teaching nonparametric correlation", Comprehensive Psychology 3, 2014,
//!   <https://doi.org/10.2466/11.IT.3.1>);
//! * three or more, the Kruskal-Wallis H test, W. H. Kruskal and W. A.
//!   Wallis, "Use of ranks in one-criterion variance analysis", JASA
//!   47(260), 583–621, 1952, <https://doi.org/10.1080/01621459.1952.10483441>:
//!   H = (12 / (N (N + 1))) Σ nᵢ (R̄ᵢ − (N + 1) / 2)² / C with
//!   C = 1 − Σ(t³ − t) / (N³ − N), the deviation form, which has none of the
//!   cancellation of 12 / (N (N + 1)) Σ Rᵢ² / nᵢ − 3 (N + 1) when N is large
//!   and H small; df = k − 1; its p, `stats_dist::chi_square_upper`;
//!   epsilon-squared, H / (N − 1) (M. Tomczak and E. Tomczak, "The need to
//!   report effect size estimates revisited", Trends in Sport Sciences
//!   21(1), 19–25, 2014).
//!
//! **One formula, one place (decision 602).** Every included value is
//! ranked once, together, through `stats_verify::average_ranks`, Spearman's
//! ranks, the one implementation (ties take their average rank, as R's
//! `rank(ties.method = "average")`); the tie runs' sizes t through
//! `stats_corr::runs`, their Σ(t³ − t) in u128, converted to a double once;
//! each group's median through `stats_anova::median`. Every p comes from
//! `stats_dist`.
//!
//! **The data (decision 602).** The group's levels are
//! `stats_ttest::group_levels`', in code-point order (decision 78), over the
//! whole column. All are included unless the spec names some, at least two,
//! by index; the rest are left out as `other_level`. A row is left out for
//! the first reason that applies: `missing_group`, `other_level`,
//! `missing_outcome`, `missing_code` (a value the user ticked, the outcome
//! only). An included level with no value after the exclusions is refused
//! (`group_empty`).
//!
//! **Edge cases are decided from the values, never from an error's text.**
//! Every included value equal leaves σ and C at 0: z and its p, and H, its p
//! and epsilon-squared, are not computable (`all_values_tied`), while U1, U2,
//! the rank-biserial correlation, the rank sums and the mean ranks are
//! given.
//!
//! **Pure** (`tests/contracts_are_pure.rs`): no file, network, process,
//! clock or model. The run's one check point is the caller's, before the
//! arithmetic.
//!
//! **Privacy.** [`IndependentRanksOutcome`] holds the level labels, for the
//! user's own screen and export; their `Debug` prints them as counts. The
//! values never leave [`Split`] and [`Ready`], which derive nothing.

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
use crate::stats_anova::median;
use crate::stats_corr::runs;
use crate::stats_desc::{
    input_ref, is_prepared_column, preparation_holds, score_code_count, suspect_codes_unless_score,
    Alias,
};
use crate::stats_dist::{chi_square_upper, normal_two_tailed, rank_sum_exact_tails};
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{column, group_levels, non_finite_cells, MAX_GROUP_LEVELS};
use crate::stats_verify::average_ranks;

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.independent_ranks";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// A group of fewer values than this is noted: the normal or chi-square
/// approximation is rougher there.
pub const GROUP_NOTE_BELOW: u64 = 5;
/// The size from which a group's exact p is no longer given: each group
/// must have fewer values than this (decision 602).
pub const EXACT_BELOW: u64 = 50;

closed! {
    /// Every refusal, outcome, exclusion, note and limitation of the rank
    /// tests.
    pub enum RanksCode in "stats.ranks." {
        /// The spec names another capability.
        UnknownCapability => "unknown_capability",
        /// The spec was written for another version of this capability.
        /// Params `expected`, `found`.
        CapabilityVersionMismatch => "capability_version_mismatch",
        /// The spec does not name exactly one input. Param `count`.
        InputCount => "input_count",
        /// The spec's input is not this dataset.
        InputMismatch => "input_mismatch",
        /// The spec's preparation does not describe this table (decision 417).
        PreparationMismatch => "preparation_mismatch",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// No column has that alias. Param `column`.
        UnknownColumn => "unknown_column",
        /// A score is never a group (decision 418). Param `column`.
        ScoreAsGroup => "score_as_group",
        /// A ticked missing code equals a value in the chosen score (decision
        /// 418, ADR-011 question 5). Params `column`, `count`.
        CodeEqualsScore => "code_equals_score",
        /// The outcome and the group are one column.
        SameColumn => "same_column",
        /// The outcome holds NaN or an infinity. Params `column`, `count`.
        OutcomeNonFinite => "outcome_non_finite",
        /// The outcome holds text. Params `column`, `count`.
        OutcomeHoldsText => "outcome_holds_text",
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
        /// Refused: an included level has no value after the exclusions.
        /// Param `index` (the level).
        GroupEmpty => "group_empty",
        /// A candidate missing code the rule found in the outcome and the
        /// user did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: every included value is equal, so the variance of
        /// U and the tie correction of H are 0.
        AllValuesTied => "all_values_tied",
        /// Not computable: the chi-square is not measured at this many
        /// degrees of freedom.
        DistributionOutOfRange => "distribution_out_of_range",
        /// Not computable: a sum, product, quotient or root a double cannot
        /// hold.
        Overflow => "overflow",
        /// Not computable: the arithmetic refused for a reason it did not
        /// name.
        ArithmeticRefused => "arithmetic_refused",
        /// Note: the exact p is not given, two included values being tied.
        ExactNotGivenTies => "exact_not_given_ties",
        /// Note: the exact p is not given, a group having 50 values or more.
        ExactNotGivenSize => "exact_not_given_size",
        /// Note: an included group has fewer than 5 values, where the normal
        /// or chi-square approximation is rougher.
        GroupBelowFive => "group_below_five",
        /// Excluded: the row's group is empty or a missing-value token.
        MissingGroup => "missing_group",
        /// Excluded: the row's group is a level left out.
        OtherLevel => "other_level",
        /// Excluded: the row's outcome is empty or a missing-value token.
        MissingOutcome => "missing_outcome",
        /// Excluded: the row's outcome is a value the user listed.
        MissingCode => "missing_code",
        /// Limitation: no pairwise comparisons after the Kruskal-Wallis
        /// test, such as Dunn's test.
        NoPairwiseComparisons => "no_pairwise_comparisons",
        /// Limitation: each Mann-Whitney p is two-sided.
        TwoSidedOnly => "two_sided_only",
        /// Limitation: no Hodges-Lehmann estimate of the shift, and no
        /// interval for it.
        NoLocationShiftEstimate => "no_location_shift_estimate",
        /// Limitation: the exact p only without ties and below 50 values a
        /// group.
        ExactWithoutTiesOnly => "exact_without_ties_only",
        /// Limitation: no median test and no Jonckheere-Terpstra test.
        NoFurtherKSampleTests => "no_further_k_sample_tests",
        /// Limitation: each row counts once; no weights.
        NoWeights => "no_weights",
        /// Limitation: the groups are in code-point order, which sets the
        /// first group and the signs of z and the rank-biserial correlation.
        CodePointOrder => "code_point_order",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// The rank tests' parameters. `levels` names the included levels by their
/// index in code-point order; `None` includes them all. `missing_codes`
/// apply to the outcome only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndependentRanksParams {
    pub outcome: Alias,
    pub group: Alias,
    pub levels: Option<Vec<u32>>,
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
    NotComputable(RanksCode),
}

/// One included group, in the levels' code-point order. The label is the
/// user's own text.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankGroup {
    pub level: u32,
    pub label: String,
    pub n: u64,
    pub median: f64,
    pub rank_sum: f64,
    pub mean_rank: f64,
}

impl std::fmt::Debug for RankGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RankGroup")
            .field("level", &self.level)
            .field("label_chars", &self.label.chars().count())
            .field("n", &self.n)
            .finish_non_exhaustive()
    }
}

/// The Mann-Whitney test of two included groups: `first` and `second` by
/// their level indexes, the first the lower.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MannWhitney {
    pub first: u32,
    pub second: u32,
    /// R1 − n1 (n1 + 1) / 2: R's W.
    pub u1: f64,
    /// n1 n2 − U1.
    pub u2: f64,
    pub z: Stat,
    /// The normal approximation's two-sided p, without a continuity
    /// correction.
    pub p: Stat,
    /// The exact two-sided p, given only without ties and below 50 values
    /// a group; the notes say why it is absent.
    pub exact_p: Option<f64>,
    /// 2 U1 / (n1 n2) − 1.
    pub rank_biserial: f64,
}

/// The Kruskal-Wallis test of three or more included groups.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KruskalWallis {
    pub h: Stat,
    /// k − 1.
    pub df: f64,
    pub p: Stat,
    /// H / (N − 1).
    pub epsilon_squared: Stat,
}

/// The test the count of included levels chose.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RankTest {
    MannWhitney(MannWhitney),
    KruskalWallis(KruskalWallis),
}

/// The rank tests' result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndependentRanksOutcome {
    /// In the levels' code-point order.
    pub groups: Vec<RankGroup>,
    /// The included values, N.
    pub n: u64,
    /// The runs of two or more equal values among them.
    pub tie_runs: u64,
    pub test: RankTest,
    /// The rows left out, by reason; reasons with no row are not listed.
    pub exclusions: Vec<Exclusion<RanksCode>>,
    /// `exact_not_given_ties`, `exact_not_given_size` and
    /// `group_below_five`, when they apply, in that order.
    pub notes: Vec<RanksCode>,
    pub limitations: Vec<RanksCode>,
}

impl IndependentRanksOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and strings serialises")
    }
}

/// What every result says it does not give.
const LIMITATIONS: [RanksCode; 7] = [
    RanksCode::NoPairwiseComparisons,
    RanksCode::TwoSidedOnly,
    RanksCode::NoLocationShiftEstimate,
    RanksCode::ExactWithoutTiesOnly,
    RanksCode::NoFurtherKSampleTests,
    RanksCode::NoWeights,
    RanksCode::CodePointOrder,
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

const EXCLUSION_CODES: [RanksCode; 4] = [
    RanksCode::MissingGroup,
    RanksCode::OtherLevel,
    RanksCode::MissingOutcome,
    RanksCode::MissingCode,
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
    parameters: IndependentRanksParams,
) -> OperationSpec<IndependentRanksParams> {
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
        preparation: None,
    }
}

fn column_param(alias: Alias) -> ParamValue<RanksCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

struct Checked<'d> {
    errors: Vec<Diagnostic<RanksCode>>,
    ready: Option<Ready<'d>>,
}

/// The columns and the included levels, once the checks pass. Derives
/// nothing.
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

/// Shared by `validate` and `run`: the errors in the one-way ANOVA's order
/// (the missing codes, the columns, the levels), then, only when there is
/// no other, an included group with no value.
fn check<'d>(dataset: &'d Dataset, params: &IndependentRanksParams) -> Checked<'d> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(RanksCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
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
                Diagnostic::error(RanksCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(alias)),
            );
        }
    }
    let group = if is_prepared_column(dataset, params.group) {
        errors.push(
            Diagnostic::error(RanksCode::ScoreAsGroup)
                .with(ParamKey::Column, column_param(params.group)),
        );
        None
    } else {
        group
    };
    let collisions = score_code_count(dataset, params.outcome, &params.missing_codes);
    if collisions > 0 {
        errors.push(
            Diagnostic::error(RanksCode::CodeEqualsScore)
                .with(ParamKey::Column, column_param(params.outcome))
                .int(ParamKey::Count, collisions as i64),
        );
    }
    let mut ready = None;
    if let (Some((o_summary, o_cells)), Some((_, g_cells))) = (outcome, group) {
        if params.outcome == params.group {
            errors.push(Diagnostic::error(RanksCode::SameColumn));
        } else {
            let non_finite = non_finite_cells(o_summary);
            let texts = o_cells
                .iter()
                .filter(|c| matches!(c, Cell::Text(_)))
                .count() as i64;
            if non_finite > 0 {
                errors.push(
                    Diagnostic::error(RanksCode::OutcomeNonFinite)
                        .with(ParamKey::Column, column_param(params.outcome))
                        .int(ParamKey::Count, non_finite as i64),
                );
            } else if texts > 0 {
                errors.push(
                    Diagnostic::error(RanksCode::OutcomeHoldsText)
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
                    Diagnostic::error(RanksCode::GroupOneLevel)
                        .with(ParamKey::Column, column_param(params.group))
                        .int(ParamKey::Count, i64::from(k)),
                );
            } else if labels.len() > MAX_GROUP_LEVELS {
                errors.push(
                    Diagnostic::error(RanksCode::TooManyLevels)
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
                            Diagnostic::error(RanksCode::LevelOutOfRange)
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
                            Diagnostic::error(RanksCode::LevelRepeated)
                                .int(ParamKey::Index, i64::from(w[0])),
                        );
                    }
                }
                if included.len() < 2 {
                    ok = false;
                    errors.push(
                        Diagnostic::error(RanksCode::TooFewLevels)
                            .int(ParamKey::Count, included.len() as i64),
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
                if values.is_empty() {
                    errors.push(
                        Diagnostic::error(RanksCode::GroupEmpty)
                            .int(ParamKey::Index, i64::from(r.included[k])),
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
/// the spec says otherwise.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<IndependentRanksParams>,
) -> ValidationReport<RanksCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(RanksCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(RanksCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(RanksCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(RanksCode::InputCount).int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    if !preparation_holds(dataset, spec.preparation.as_ref()) {
        errors.push(Diagnostic::error(RanksCode::PreparationMismatch));
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    if let Some((_, cells)) = column(dataset, params.outcome) {
        for s in suspect_codes_unless_score(dataset, params.outcome, cells) {
            if !params.missing_codes.contains(&f64::from(s.code)) {
                warnings.push(
                    Diagnostic::warning(RanksCode::SuspectMissingCode)
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
    Refused(Diagnostic<RanksCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run calls: one, before the arithmetic (decision
/// 602). The run manager's progress counts it.
pub fn check_points() -> usize {
    1
}

/// [`run`] with a check point that never stops.
pub fn compute(
    dataset: &Dataset,
    params: &IndependentRanksParams,
) -> Result<IndependentRanksOutcome, Diagnostic<RanksCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The rank tests. `checkpoint(0)` is called after the parameters are
/// checked and before the arithmetic; a `Break` stops the run there. What a
/// stop means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &IndependentRanksParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<IndependentRanksOutcome, Halt<B>> {
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
    let ranked = Ranked::of(&s.values);
    let groups: Vec<RankGroup> = s
        .values
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let level = ready.included[k];
            RankGroup {
                level,
                label: ready.labels[level as usize].clone(),
                n: v.len() as u64,
                median: median(v),
                rank_sum: ranked.rank_sums[k],
                mean_rank: ranked.rank_sums[k] / v.len() as f64,
            }
        })
        .collect();
    let mut notes = Vec::new();
    let test = if groups.len() == 2 {
        let (t, exact_notes) = mann_whitney(&groups, &ranked).map_err(refuse)?;
        notes.extend(exact_notes);
        RankTest::MannWhitney(t)
    } else {
        RankTest::KruskalWallis(kruskal_wallis(&groups, &ranked).map_err(refuse)?)
    };
    if groups.iter().any(|g| g.n < GROUP_NOTE_BELOW) {
        notes.push(RanksCode::GroupBelowFive);
    }
    let exclusions = EXCLUSION_CODES
        .iter()
        .zip(s.excluded)
        .filter(|(_, n)| *n > 0)
        .map(|(code, count)| {
            let alias = match code {
                RanksCode::MissingGroup | RanksCode::OtherLevel => params.group,
                _ => params.outcome,
            };
            Exclusion {
                code: *code,
                count,
                params: BTreeMap::from([(ParamKey::Column, column_param(alias))]),
            }
        })
        .collect();
    Ok(IndependentRanksOutcome {
        groups,
        n: ranked.n,
        tie_runs: ranked.tie_runs,
        test,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    })
}

fn refuse<B>(code: RanksCode) -> Halt<B> {
    Halt::Refused(Diagnostic::error(code))
}

/// `v`, or `overflow` when it is infinite or NaN.
fn finite(v: f64) -> Result<f64, RanksCode> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(RanksCode::Overflow)
    }
}

/// The ranks' facts: every included value ranked once, together, through
/// `stats_verify::average_ranks`, each group's rank sum, and the tie runs
/// through `stats_corr::runs`. Derives nothing.
struct Ranked {
    /// N.
    n: u64,
    /// Each group's rank sum, in the groups' order.
    rank_sums: Vec<f64>,
    /// Runs of two or more equal values.
    tie_runs: u64,
    /// Σ(t³ − t) over the runs, in u128, converted to a double once.
    tie_sum: f64,
    /// Every included value equal: one run of N.
    all_tied: bool,
}

impl Ranked {
    fn of(values: &[Vec<f64>]) -> Ranked {
        let all: Vec<f64> = values.iter().flatten().copied().collect();
        let ranks = average_ranks(&all);
        let mut rank_sums = Vec::with_capacity(values.len());
        let mut at = 0;
        for v in values {
            rank_sums.push(ranks[at..at + v.len()].iter().sum());
            at += v.len();
        }
        let mut sorted = all;
        sorted.sort_by(f64::total_cmp);
        let sizes = runs(&sorted);
        let tie_sum: u128 = sizes.iter().map(|t| t * t * t - t).sum();
        Ranked {
            n: sorted.len() as u64,
            rank_sums,
            tie_runs: sizes.iter().filter(|t| **t >= 2).count() as u64,
            tie_sum: tie_sum as f64,
            all_tied: sizes.len() == 1,
        }
    }
}

/// The Mann-Whitney test of the two groups, and the notes saying why the
/// exact p is absent, when it is.
fn mann_whitney(
    groups: &[RankGroup],
    ranked: &Ranked,
) -> Result<(MannWhitney, Vec<RanksCode>), RanksCode> {
    let (first, second) = (&groups[0], &groups[1]);
    let (n1, n2) = (first.n as f64, second.n as f64);
    let big_n = ranked.n as f64;
    let u1 = first.rank_sum - n1 * (n1 + 1.0) / 2.0;
    let u2 = n1 * n2 - u1;
    let (z, p) = if ranked.all_tied {
        let nc = Stat::NotComputable(RanksCode::AllValuesTied);
        (nc, nc)
    } else {
        let variance =
            (n1 * n2 / 12.0) * ((big_n + 1.0) - ranked.tie_sum / (big_n * (big_n - 1.0)));
        let z = finite((u1 - n1 * n2 / 2.0) / finite(variance.sqrt())?)?;
        (Stat::Value(z), Stat::Value(normal_two_tailed(z)))
    };
    let mut notes = Vec::new();
    if ranked.tie_runs > 0 {
        notes.push(RanksCode::ExactNotGivenTies);
    }
    if first.n >= EXACT_BELOW || second.n >= EXACT_BELOW {
        notes.push(RanksCode::ExactNotGivenSize);
    }
    let exact_p = if notes.is_empty() {
        let tails = rank_sum_exact_tails(u1 as u64, first.n, second.n)
            .ok_or(RanksCode::DistributionOutOfRange)?;
        Some((2.0 * tails.lower.min(tails.upper)).min(1.0))
    } else {
        None
    };
    Ok((
        MannWhitney {
            first: first.level,
            second: second.level,
            u1,
            u2,
            z,
            p,
            exact_p,
            rank_biserial: finite(2.0 * u1 / (n1 * n2) - 1.0)?,
        },
        notes,
    ))
}

/// The Kruskal-Wallis test of three or more groups, H in its deviation
/// form.
fn kruskal_wallis(groups: &[RankGroup], ranked: &Ranked) -> Result<KruskalWallis, RanksCode> {
    let df = (groups.len() - 1) as f64;
    if ranked.all_tied {
        let nc = Stat::NotComputable(RanksCode::AllValuesTied);
        return Ok(KruskalWallis {
            h: nc,
            df,
            p: nc,
            epsilon_squared: nc,
        });
    }
    let big_n = ranked.n as f64;
    let centre = (big_n + 1.0) / 2.0;
    let spread: f64 = groups
        .iter()
        .map(|g| g.n as f64 * (g.mean_rank - centre).powi(2))
        .sum();
    let cube = (u128::from(ranked.n).pow(3) - u128::from(ranked.n)) as f64;
    let correction = 1.0 - ranked.tie_sum / cube;
    let h = finite((12.0 / (big_n * (big_n + 1.0))) * spread / correction)?;
    let p = match chi_square_upper(h, df) {
        Some(p) => Stat::Value(p),
        None => Stat::NotComputable(RanksCode::DistributionOutOfRange),
    };
    Ok(KruskalWallis {
        h: Stat::Value(h),
        df,
        p,
        epsilon_squared: Stat::Value(finite(h / (big_n - 1.0))?),
    })
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows
/// used are N; with the exclusions they add up to the data rows. The output
/// hash is SHA-256 of [`IndependentRanksOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<IndependentRanksParams>,
    dataset: &Dataset,
    result: &IndependentRanksOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<RanksCode> {
    let parameters = serde_json::to_vec(&spec.parameters)
        .expect("parameters of aliases and numbers serialise");
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        inputs: spec.inputs.iter().map(|i| i.content_hash.clone()).collect(),
        parameters_hash: ContentHash::of_bytes(&parameters),
        seed: spec.random_seed,
        rows_read: dataset.rows() as u64,
        rows_used: result.n,
        exclusions: result.exclusions.clone(),
        platform,
        elapsed_ms,
        output_hash: ContentHash::of_bytes(&result.canonical_bytes()),
        reproducibility: Reproducibility::Bitwise,
        preparation: spec.preparation.clone(),
    }
}

/// FIND FIRST (decision 602): the three functions this module takes from
/// elsewhere in the crate, checked where a `pub(crate)` function can be
/// reached. The ranks are R's `rank(c(3, 1, 2, 1, -0, 0, 7.5, 2),
/// ties.method = "average")` (R 4.6.1, 10 Nov 2026), -0 and 0 tied.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_ranks_are_rs_average_ranks_with_minus_zero_and_zero_tied() {
        let x = [3.0, 1.0, 2.0, 1.0, -0.0, 0.0, 7.5, 2.0];
        assert_eq!(average_ranks(&x), [7.0, 3.5, 5.5, 3.5, 1.5, 1.5, 8.0, 5.5]);
        // 100,000 values with ties. Not timed here: this module is pure, and
        // `tests/contracts_are_pure.rs` refuses a clock or stdout in any of its
        // lines, tests included; the method's release probe times the run.
        let v: Vec<f64> = (0..100_000u64).map(|i| ((i * 7919) % 1000) as f64).collect();
        let r = average_ranks(&v);
        assert_eq!(r.len(), 100_000);
        // Every value is tied 100 times: value j takes ranks 100 j + 1 to
        // 100 j + 100, whose average is 100 j + 50.5.
        assert!(v.iter().zip(&r).all(|(x, r)| *r == 100.0 * x + 50.5));
        assert_eq!(r.iter().sum::<f64>(), 100_000.0 * 100_001.0 / 2.0);
    }

    #[test]
    fn runs_are_the_tie_runs_sizes_and_median_the_middle_of_ascending_values() {
        let mut x = [3.0, 1.0, 2.0, 1.0, -0.0, 0.0, 7.5, 2.0];
        x.sort_by(f64::total_cmp);
        assert_eq!(runs(&x), [2u128, 2, 2, 1, 1]);
        assert_eq!(runs(&[]), Vec::<u128>::new());
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[1.0, 3.0, 5.0]), 3.0);
        assert_eq!(median(&[4.0]), 4.0);
    }
}
