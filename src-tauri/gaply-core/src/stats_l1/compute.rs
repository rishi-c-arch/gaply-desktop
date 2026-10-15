//! **Level 1's computation: a live token, the data read again from its bytes,
//! the bound rows, and one registered method, to an exact statistic, df, p
//! and n for every variant the method has.**
//!
//! This module computes; it does not compare and it chooses no verdict
//! (Step 4 does both). In order, [`compute`]:
//!
//! 1. opens [`L1RunState::gate`] with the token, so only a confirmed binding
//!    whose token is live reaches a number;
//! 2. compares the fingerprint of the dataset the caller read AGAIN from the
//!    file's bytes with the one the binding was confirmed on;
//! 3. refuses a weighted file (decisions.md: a declared weight is
//!    `CouldNotCheck`, never an unweighted result);
//! 4. looks the method up in [`L1Registry`], the implemented subset of the
//!    catalogue, and refuses anything outside it with
//!    [`L1Refusal::MethodNotAvailable`], never the nearest method;
//! 5. applies the include-filters (through the same predicate the confirm
//!    pass counted with), then listwise deletion on the bound columns only,
//!    then level selection, and counts each exclusion by a closed reason;
//! 6. runs the method's adapter.
//!
//! # Adapters, not math
//!
//! Every statistic comes from [`crate::stats_verify`], unchanged where it
//! existed: Welch's and Student's t, one-way ANOVA, Pearson, Spearman, the
//! chi-square test of independence and OLS. Phase 2, Step 3 added beside them
//! the paired t, the 2×2 continuity-corrected chi-square and OLS's overall F.
//! **No p from `stats_verify` is used**: it computes tails as `1 - cdf`, and
//! decisions.md requires every Level 1 p to come from Level 0's survival-
//! function engine, [`crate::stats_l0::pvalue`]. The adapters below take the
//! statistic and df and call that engine.
//!
//! The adapters do check, before calling `stats_verify`, every condition that
//! would make it fail, so that each refusal is a closed code rather than its
//! error text: zero variance is tested as "every value equal", which is exact,
//! where `stats_verify`'s `se == 0.0` misses a constant column whose floating
//! mean is not the value itself (three cells of 0.1 have a sample variance of
//! about 1e-34, not 0).
//!
//! # Tails and signs
//!
//! p for t, r and rho is two-sided; p for F and chi-square is the upper tail.
//! Step 4 applies Phase 1's one-tailed rule. An independent t is the first
//! selected level minus the second, in the column's level order (numeric
//! codes by value, text codes by string), which is R's factor order for
//! numeric codes. A paired t is `First - Second`.
//!
//! No value is rounded anywhere here. Nothing here derives `Serialize`: a
//! computed statistic is a statistic of the data and stays on the device.
//!
//! # The file's own precision (Step 8b)
//!
//! Every result also carries a [`Precision`]: how far the rounding of the
//! cells as stored could move it ([`super::precision`]). Exact when every
//! bound numeric column holds whole numbers (or none is bound, as for a
//! contingency table of counts).

use super::binding::{passes_filters, ConfirmedBinding};
use super::catalogue::{L1MethodId, RoleName};
use super::claim::ClaimId;
use super::dataset::{Cell, ColumnAlias, Dataset, DatasetIssue};
use super::precision::{self, StatRange, Stored};
use super::run_state::{GateRejection, L1RunState};
use super::token::ConfirmationToken;
use super::ENGINE_VERSION;
use crate::stats_l0::pvalue;
use crate::error::GaplyError;
use crate::stats_verify::{self, RangeFault};

/* ------------------------------------------------------------------------ *
 *  The registry
 * ------------------------------------------------------------------------ */

/// A method Level 1 can compute: an implementation, R golden fixtures and a
/// tolerance rule, as Phase 1 required of a Level 0 method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegisteredL1Method {
    pub method: L1MethodId,
    /// The `stats_verify` functions the adapter calls for the statistic.
    pub statistic_from: &'static [&'static str],
    /// The golden file, relative to the `gaply-core` crate root; rows are
    /// selected by the method's string id.
    pub fixtures: &'static str,
    /// The tolerance rule: the largest relative difference from R allowed on
    /// any number (statistic, df, p) over the method's fixtures. Engine error
    /// only; it says nothing about a manuscript's rounding, which Step 4
    /// handles. Set on 14 Oct 2026 at one decade above the worst difference
    /// `tests/l1_r_golden.rs` measured (quoted beside each), rounded up to a
    /// power of ten.
    pub relative_gate: f64,
}

const GOLDEN: &str = "tests/fixtures/stats_l1/golden.csv";

/// The implemented subset of [`super::catalogue::MethodCatalogue`].
pub struct L1Registry;

impl L1Registry {
    pub const METHODS: &'static [RegisteredL1Method] = &[
        RegisteredL1Method {
            method: L1MethodId::IndependentT,
            statistic_from: &["t_test_student", "t_test_welch"],
            fixtures: GOLDEN,
            relative_gate: 1e-12, // measured 3.2e-14
        },
        RegisteredL1Method { method: L1MethodId::PairedT, statistic_from: &["t_test_paired"], fixtures: GOLDEN, relative_gate: 1e-13 }, // measured 1.8e-15
        RegisteredL1Method { method: L1MethodId::OneWayAnova, statistic_from: &["one_way_anova"], fixtures: GOLDEN, relative_gate: 1e-12 }, // measured 2.8e-14
        RegisteredL1Method { method: L1MethodId::Pearson, statistic_from: &["pearson"], fixtures: GOLDEN, relative_gate: 1e-12 }, // measured 5.0e-14
        RegisteredL1Method { method: L1MethodId::Spearman, statistic_from: &["spearman"], fixtures: GOLDEN, relative_gate: 1e-12 }, // measured 4.5e-14
        RegisteredL1Method {
            method: L1MethodId::ChiSquareIndependence,
            statistic_from: &["chi_square_independence", "chi_square_2x2_yates"],
            fixtures: GOLDEN,
            relative_gate: 1e-13, // measured 4.2e-15
        },
        // Measured 2.0e-13 (ols_three_coef_x3), by Householder QR as R's `lm`
        // fits. `stats_verify::ols` (normal equations) on the same 20 rows
        // measured 9.7e-11, on the predictors correlated at .999.
        RegisteredL1Method { method: L1MethodId::Ols, statistic_from: &["ols_qr"], fixtures: GOLDEN, relative_gate: 1e-11 },
    ];

    pub fn get(method: L1MethodId) -> Option<&'static RegisteredL1Method> {
        L1Registry::METHODS.iter().find(|m| m.method == method)
    }

    /// A method named by its string id: anything not registered is
    /// [`L1Refusal::MethodNotAvailable`], whatever it resembles.
    pub fn by_id(id: &str) -> Result<&'static RegisteredL1Method, L1Refusal> {
        L1MethodId::from_id(id).and_then(L1Registry::get).ok_or(L1Refusal::MethodNotAvailable)
    }
}

/* ------------------------------------------------------------------------ *
 *  Refusals and counts
 * ------------------------------------------------------------------------ */

/// Why nothing was computed. Closed codes with numeric parameters and column
/// aliases only: no header, level, cell or text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L1Refusal {
    /// The token did not open the gate.
    Gate(GateRejection),
    /// The data read again from the file is not the data the binding was
    /// confirmed on.
    FingerprintChanged,
    /// The source declares a weight variable.
    WeightedFile,
    /// The method is not in the Level 1 registry.
    MethodNotAvailable,
    /// Fewer complete cases than the method needs: for a two-group or
    /// grouped method, `n` is the smallest group.
    TooFewCompleteCases { n: usize, minimum: usize },
    /// The values the statistic divides by do not vary.
    ZeroVariance,
    /// After filtering and listwise deletion, fewer than two groups have rows.
    SingleGroup { groups: usize },
    /// A contingency table with an all-zero row or column.
    EmptyRowOrColumn { empty_rows: usize, empty_columns: usize },
    /// OLS: a constant predictor, or predictors that are collinear.
    SingularDesign,
    /// A cell in a numeric role is not a number (a categorical predictor
    /// whose column changed type, for one).
    NotNumeric { column: ColumnAlias },
    /// A value that is not finite: in a bound cell, or in the computation
    /// (an intermediate that overflowed, which `stats_verify` refuses on, or
    /// an infinite t from a perfect fit or |r| = 1).
    NonFinite { at: NonFiniteAt },
    /// A variance, sum of squares or Householder norm underflowed to zero
    /// (or a subnormal) although the values differ. Not `NonFinite`: every
    /// number involved is finite, and wrong.
    Underflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonFiniteAt {
    Input,
    /// In the computation: an intermediate or the result.
    Result,
}

/// Why a row read from the file did not enter the computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionReason {
    /// An include-filter removed it.
    FilteredOut,
    /// A bound column is missing in it (listwise, on the bound columns only).
    MissingInBoundColumn,
    /// Its group level is not one the binding selected. The confirm pass
    /// rejects a binding that leaves a level unselected, so this is zero
    /// unless that rule changes.
    LevelNotSelected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exclusion {
    pub reason: ExclusionReason,
    pub rows: usize,
}

/// Counts, never rows: how many were read, excluded by each reason, and used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputCounts {
    pub rows_read: usize,
    /// Only reasons that excluded at least one row, in [`ExclusionReason`]
    /// order.
    pub excluded: Vec<Exclusion>,
    pub n_used: usize,
}

/* ------------------------------------------------------------------------ *
 *  Output
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticKind {
    T,
    F,
    ChiSquare,
    R,
    Rho,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    TwoSided,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlsTerm {
    Intercept,
    Predictor(ColumnAlias),
}

/// Which computation a result is. A method with variants returns one result
/// per variant it has, whatever the binding states; Step 4 picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedVariant {
    StudentPooled,
    Welch,
    Paired,
    OneWay,
    Pearson,
    /// Spearman's rho with the t-approximation p. The exact p is deferred.
    SpearmanTApproximation,
    ChiSquareUncorrected,
    /// 2×2 only: R's `chisq.test(correct = TRUE)`.
    ChiSquareYatesR,
    OlsOverallF,
    OlsCoefficient(OlsTerm),
}

/// One computed result. Exact `f64`s, never rounded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComputedResult {
    pub variant: ComputedVariant,
    pub statistic: StatisticKind,
    pub value: f64,
    /// t, r, rho and chi-square: their one df. F: the numerator df.
    pub df1: f64,
    /// F: the denominator df.
    pub df2: Option<f64>,
    pub p: f64,
    pub tail: Tail,
    pub n: usize,
    /// How far the stored cells' rounding could move this result.
    pub precision: Precision,
}

/// How far the rounding of the bound cells as stored could move a result
/// (Step 8b, rule 1). First order, worst case: `[S - b, S + b]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Precision {
    /// Every bound numeric cell is a whole number, no numeric cell is bound,
    /// or (Spearman) rounding cannot reorder or tie a value.
    Exact,
    /// The cells are stored to `decimals` (the most of any bound column).
    Range {
        decimals: u32,
        statistic: (f64, f64),
        /// Welch's df only.
        df: Option<(f64, f64)>,
        /// From the ends of the statistic (and df) range, with Level 0's
        /// p engine.
        p: (f64, f64),
    },
    /// Spearman: rounding could reorder or tie values, so rho has no
    /// first-order range; any miss is not checked.
    Unbounded { decimals: u32 },
}

/// The stored decimals of a bound numeric column, over all of its cells.
fn stored_decimals(d: &Dataset, alias: ColumnAlias) -> Result<u32, L1Refusal> {
    Ok(precision::column_decimals(cells(d, alias)?.iter().filter_map(|c| match c {
        Cell::Number(v) if v.is_finite() => Some(*v),
        _ => None,
    })))
}

/// The ends of a signed range as magnitudes.
fn magnitudes(lo: f64, hi: f64) -> (f64, f64) {
    if lo <= 0.0 && hi >= 0.0 {
        (0.0, lo.abs().max(hi.abs()))
    } else {
        (lo.abs().min(hi.abs()), lo.abs().max(hi.abs()))
    }
}

/// A t's precision from its range (and, for Welch, its df's range); the
/// two-sided p from the ends.
fn t_precision(df: f64, range: StatRange, df_range: Option<StatRange>, decimals: u32) -> Precision {
    if decimals == 0 {
        return Precision::Exact;
    }
    let (lo, hi) = match range {
        StatRange::Exact => return Precision::Exact,
        StatRange::Unbounded => return Precision::Unbounded { decimals },
        StatRange::Range(lo, hi) => (lo, hi),
    };
    let df_ends = match df_range {
        None | Some(StatRange::Exact) => None,
        Some(StatRange::Unbounded) => return Precision::Unbounded { decimals },
        Some(StatRange::Range(a, b)) => Some((a.max(f64::MIN_POSITIVE), b)),
    };
    let (dlo, dhi) = df_ends.unwrap_or((df, df));
    let (alo, ahi) = magnitudes(lo, hi);
    let p = |tt: f64, dd: f64| pvalue::t_two_tailed(tt, dd).unwrap_or(f64::NAN);
    let (p_lo, p_hi) = (p(ahi, dhi), p(alo, dlo));
    Precision::Range { decimals, statistic: (lo, hi), df: df_ends, p: (p_lo.min(p_hi), p_lo.max(p_hi)) }
}

/// An F's precision from its range; the upper-tail p from the ends.
fn f_precision(df1: f64, df2: f64, range: StatRange, decimals: u32) -> Precision {
    if decimals == 0 {
        return Precision::Exact;
    }
    let (lo, hi) = match range {
        StatRange::Exact => return Precision::Exact,
        StatRange::Unbounded => return Precision::Unbounded { decimals },
        StatRange::Range(lo, hi) => (lo.max(0.0), hi.max(0.0)),
    };
    let p = |x: f64| pvalue::f_upper(x, df1, df2).unwrap_or(f64::NAN);
    let (p_lo, p_hi) = (p(hi), if lo == 0.0 { 1.0 } else { p(lo) });
    Precision::Range { decimals, statistic: (lo, hi), df: None, p: (p_lo.min(p_hi), p_lo.max(p_hi)) }
}

/// An r's precision from its range, inside [-1, 1]; the two-sided p from the
/// ends.
fn r_precision(df: f64, range: StatRange, decimals: u32) -> Precision {
    if decimals == 0 {
        return Precision::Exact;
    }
    let (lo, hi) = match range {
        StatRange::Exact => return Precision::Exact,
        StatRange::Unbounded => return Precision::Unbounded { decimals },
        StatRange::Range(lo, hi) => (lo.max(-1.0), hi.min(1.0)),
    };
    let (alo, ahi) = magnitudes(lo, hi);
    let p = |x: f64| if x >= 1.0 { 0.0 } else { pvalue::r_two_tailed(x, df).unwrap_or(f64::NAN) };
    let (p_lo, p_hi) = (p(ahi), p(alo));
    Precision::Range { decimals, statistic: (lo, hi), df: None, p: (p_lo.min(p_hi), p_lo.max(p_hi)) }
}

/// Everything one computation produced. Local only.
#[derive(Debug, Clone, PartialEq)]
pub struct L1Computation {
    pub claim: ClaimId,
    pub method: L1MethodId,
    pub counts: InputCounts,
    pub results: Vec<ComputedResult>,
    /// Spearman only: whether either column holds a tie among the rows used.
    /// R's `cor.test` uses its exact p when n < 1290 and there is none, and
    /// Level 1 does not compute that p (Step 4 reads this). `None` for every
    /// other method.
    pub ties: Option<bool>,
    pub engine_version: &'static str,
}

/* ------------------------------------------------------------------------ *
 *  The entry
 * ------------------------------------------------------------------------ */

/// Compute the result `claim`'s confirmed binding asks for. `reread` is the
/// dataset the caller built again from the file's bytes for this call; it
/// must carry the fingerprint the binding was confirmed on.
pub fn compute(state: &L1RunState, token: &ConfirmationToken, claim: ClaimId, reread: &Dataset) -> Result<L1Computation, L1Refusal> {
    let binding = state.gate(token, claim).map_err(L1Refusal::Gate)?;
    if reread.schema.fingerprint.digest() != binding.fingerprint_digest() {
        return Err(L1Refusal::FingerprintChanged);
    }
    let weighted = reread.schema.weight_variable.is_some()
        || reread.schema.issues.iter().any(|i| matches!(i, DatasetIssue::WeightVariableDeclared { .. } | DatasetIssue::WeightVariableUnknown));
    if weighted {
        return Err(L1Refusal::WeightedFile);
    }
    let registered = L1Registry::get(binding.method()).ok_or(L1Refusal::MethodNotAvailable)?;

    let (counts, rows) = select_rows(binding, reread)?;
    let results = match registered.method {
        L1MethodId::IndependentT => independent_t(binding, reread, &rows)?,
        L1MethodId::PairedT => paired_t(binding, reread, &rows)?,
        L1MethodId::OneWayAnova => one_way_anova(binding, reread, &rows)?,
        L1MethodId::Pearson => correlation(binding, reread, &rows, false)?,
        L1MethodId::Spearman => correlation(binding, reread, &rows, true)?,
        L1MethodId::ChiSquareIndependence => chi_square(binding, reread, &rows)?,
        L1MethodId::Ols => ols(binding, reread, &rows)?,
    };
    if results.iter().any(|r| !(r.value.is_finite() && r.df1.is_finite() && r.df2.map_or(true, f64::is_finite) && r.p.is_finite())) {
        return Err(L1Refusal::NonFinite { at: NonFiniteAt::Result });
    }
    tracing::debug!(
        rows_read = counts.rows_read,
        n_used = counts.n_used,
        results = results.len(),
        "stats_l1: computed"
    );
    let ties = if registered.method == L1MethodId::Spearman { Some(has_ties(binding, reread, &rows)?) } else { None };
    Ok(L1Computation { claim, method: registered.method, counts, results, ties, engine_version: ENGINE_VERSION })
}

/* ------------------------------------------------------------------------ *
 *  Rows
 * ------------------------------------------------------------------------ */

fn cells<'a>(d: &'a Dataset, alias: ColumnAlias) -> Result<&'a [Cell], L1Refusal> {
    // The fingerprint matched, so the schema is the one confirmed on; a
    // missing column means it is not, and nothing is computed.
    d.column_cells(alias).ok_or(L1Refusal::FingerprintChanged)
}

/// Include-filters, then listwise deletion on the bound columns, then level
/// selection. The rows that remain, in file order, and the counts.
fn select_rows(b: &ConfirmedBinding, d: &Dataset) -> Result<(InputCounts, Vec<usize>), L1Refusal> {
    let mut bound: Vec<&[Cell]> = Vec::new();
    for r in b.roles() {
        for &c in &r.columns {
            bound.push(cells(d, c)?);
        }
    }
    let mut groups: Vec<(&[Cell], &[String])> = Vec::new();
    for sel in b.levels() {
        let column = *b.columns_for(sel.role).first().ok_or(L1Refusal::FingerprintChanged)?;
        groups.push((cells(d, column)?, sel.levels.as_slice()));
    }
    let (mut filtered, mut missing, mut unselected) = (0, 0, 0);
    let mut rows = Vec::new();
    for i in 0..d.rows() {
        if !passes_filters(d, b.filters(), i) {
            filtered += 1;
        } else if bound.iter().any(|c| c[i].is_missing()) {
            missing += 1;
        } else if groups.iter().any(|(c, levels)| !c[i].level_code().is_some_and(|code| levels.contains(&code))) {
            unselected += 1;
        } else {
            rows.push(i);
        }
    }
    let excluded = [
        (ExclusionReason::FilteredOut, filtered),
        (ExclusionReason::MissingInBoundColumn, missing),
        (ExclusionReason::LevelNotSelected, unselected),
    ]
    .into_iter()
    .filter(|(_, n)| *n > 0)
    .map(|(reason, rows)| Exclusion { reason, rows })
    .collect();
    Ok((InputCounts { rows_read: d.rows(), excluded, n_used: rows.len() }, rows))
}

/// The bound role's single column.
fn column(b: &ConfirmedBinding, role: RoleName) -> Result<ColumnAlias, L1Refusal> {
    b.columns_for(role).first().copied().ok_or(L1Refusal::FingerprintChanged)
}

/// The numbers of `alias` on `rows`.
fn numbers(d: &Dataset, alias: ColumnAlias, rows: &[usize]) -> Result<Vec<f64>, L1Refusal> {
    let c = cells(d, alias)?;
    rows.iter()
        .map(|&i| match &c[i] {
            Cell::Number(v) if v.is_finite() => Ok(*v),
            Cell::Number(_) => Err(L1Refusal::NonFinite { at: NonFiniteAt::Input }),
            Cell::Text(_) | Cell::Missing => Err(L1Refusal::NotNumeric { column: alias }),
        })
        .collect()
}

/// The selected levels of a group role, in the column's level order.
fn ordered_levels(b: &ConfirmedBinding, d: &Dataset, role: RoleName, alias: ColumnAlias) -> Vec<String> {
    let selected: &[String] = b.levels().iter().find(|s| s.role == role).map(|s| s.levels.as_slice()).unwrap_or(&[]);
    match d.schema.column(alias).and_then(|c| c.levels.as_ref()) {
        Some(levels) => levels.codes().filter(|c| selected.iter().any(|s| s == c)).map(str::to_string).collect(),
        None => selected.to_vec(),
    }
}

/// `values` split by the group column into one sample per level, in level
/// order.
fn split_by_group(d: &Dataset, group: ColumnAlias, levels: &[String], rows: &[usize], values: &[f64]) -> Result<Vec<Vec<f64>>, L1Refusal> {
    let g = cells(d, group)?;
    let mut out = vec![Vec::new(); levels.len()];
    for (k, &i) in rows.iter().enumerate() {
        if let Some(pos) = g[i].level_code().and_then(|code| levels.iter().position(|l| *l == code)) {
            out[pos].push(values[k]);
        }
    }
    Ok(out)
}

/// A `stats_verify` error as a refusal: an overflow or underflow it refused
/// on, typed; anything else is `otherwise`, the one condition the adapter
/// had not already ruled out.
fn refused(e: GaplyError, otherwise: L1Refusal) -> L1Refusal {
    match RangeFault::of(&e) {
        Some(RangeFault::Overflow) => L1Refusal::NonFinite { at: NonFiniteAt::Result },
        Some(RangeFault::Underflow) => L1Refusal::Underflow,
        None => otherwise,
    }
}

/// A tie in either correlated column, by exact equality, as ranking sees one.
fn has_ties(b: &ConfirmedBinding, d: &Dataset, rows: &[usize]) -> Result<bool, L1Refusal> {
    let tied = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v.windows(2).any(|w| w[0] == w[1])
    };
    Ok(tied(numbers(d, column(b, RoleName::X)?, rows)?) || tied(numbers(d, column(b, RoleName::Y)?, rows)?))
}

fn constant(xs: &[f64]) -> bool {
    xs.windows(2).all(|w| w[0] == w[1])
}

fn finite_p(p: Option<f64>) -> Result<f64, L1Refusal> {
    p.ok_or(L1Refusal::NonFinite { at: NonFiniteAt::Result })
}

/// Fewer than two samples with any rows: a single group. Then every sample
/// must have `minimum` rows.
fn check_groups(samples: &[Vec<f64>], minimum: usize) -> Result<(), L1Refusal> {
    let present = samples.iter().filter(|s| !s.is_empty()).count();
    if present < 2 {
        return Err(L1Refusal::SingleGroup { groups: present });
    }
    let smallest = samples.iter().map(Vec::len).min().unwrap_or(0);
    if smallest < minimum {
        return Err(L1Refusal::TooFewCompleteCases { n: smallest, minimum });
    }
    Ok(())
}

/* ------------------------------------------------------------------------ *
 *  Adapters
 * ------------------------------------------------------------------------ */

fn t_result(variant: ComputedVariant, t: &stats_verify::TTestResult, n: usize, precision: Precision) -> Result<ComputedResult, L1Refusal> {
    Ok(ComputedResult {
        variant,
        statistic: StatisticKind::T,
        value: t.t,
        df1: t.df,
        df2: None,
        p: finite_p(pvalue::t_two_tailed(t.t, t.df))?,
        tail: Tail::TwoSided,
        n,
        precision,
    })
}

/// Student's and Welch's t. Each group needs two rows: `stats_verify`
/// requires it for both, though R's Student t accepts one group of one.
fn independent_t(b: &ConfirmedBinding, d: &Dataset, rows: &[usize]) -> Result<Vec<ComputedResult>, L1Refusal> {
    let outcome = column(b, RoleName::Outcome)?;
    let group = column(b, RoleName::Group)?;
    let levels = ordered_levels(b, d, RoleName::Group, group);
    let y = numbers(d, outcome, rows)?;
    let samples = split_by_group(d, group, &levels, rows, &y)?;
    check_groups(&samples, 2)?;
    let (a, c) = (&samples[0], &samples[1]);
    if constant(a) && constant(c) {
        return Err(L1Refusal::ZeroVariance);
    }
    let n = a.len() + c.len();
    // The conditions stats_verify refuses on were checked above; any error
    // left is a range fault or its own zero-variance test.
    let student = stats_verify::t_test_student(a, c).map_err(|e| refused(e, L1Refusal::ZeroVariance))?;
    let welch = stats_verify::t_test_welch(a, c).map_err(|e| refused(e, L1Refusal::ZeroVariance))?;
    let decimals = stored_decimals(d, outcome)?;
    let delta = precision::half_unit(decimals);
    let ranges = precision::independent_t_parts(a, c, delta);
    let student_p = t_precision(student.df, ranges.student.range(), None, decimals);
    let welch_p = t_precision(welch.df, ranges.welch.range(), Some(ranges.welch_df.range()), decimals);
    Ok(vec![
        t_result(ComputedVariant::StudentPooled, &student, n, student_p)?,
        t_result(ComputedVariant::Welch, &welch, n, welch_p)?,
    ])
}

fn paired_t(b: &ConfirmedBinding, d: &Dataset, rows: &[usize]) -> Result<Vec<ComputedResult>, L1Refusal> {
    let (first_col, second_col) = (column(b, RoleName::First)?, column(b, RoleName::Second)?);
    let first = numbers(d, first_col, rows)?;
    let second = numbers(d, second_col, rows)?;
    if first.len() < 2 {
        return Err(L1Refusal::TooFewCompleteCases { n: first.len(), minimum: 2 });
    }
    let diffs: Vec<f64> = first.iter().zip(&second).map(|(x, y)| x - y).collect();
    // Differences that overflowed compare equal (inf == inf) and are not
    // "no variation": stats_verify refuses them as an overflow.
    if diffs.iter().all(|d| d.is_finite()) && constant(&diffs) {
        return Err(L1Refusal::ZeroVariance);
    }
    let t = stats_verify::t_test_paired(&first, &second).map_err(|e| refused(e, L1Refusal::ZeroVariance))?;
    let (d1, d2) = (stored_decimals(d, first_col)?, stored_decimals(d, second_col)?);
    let range = precision::paired_t_parts(
        Stored { values: &first, delta: precision::half_unit(d1) },
        Stored { values: &second, delta: precision::half_unit(d2) },
    )
    .range();
    Ok(vec![t_result(ComputedVariant::Paired, &t, first.len(), t_precision(t.df, range, None, d1.max(d2)))?])
}

fn one_way_anova(b: &ConfirmedBinding, d: &Dataset, rows: &[usize]) -> Result<Vec<ComputedResult>, L1Refusal> {
    let outcome = column(b, RoleName::Outcome)?;
    let group = column(b, RoleName::Group)?;
    let levels = ordered_levels(b, d, RoleName::Group, group);
    let y = numbers(d, outcome, rows)?;
    let samples = split_by_group(d, group, &levels, rows, &y)?;
    // Every selected level must keep a row: R would drop an empty level and
    // test fewer groups than the binding names.
    check_groups(&samples, 1)?;
    let k = samples.len();
    if y.len() <= k {
        return Err(L1Refusal::TooFewCompleteCases { n: y.len(), minimum: k + 1 });
    }
    if samples.iter().all(|s| constant(s)) {
        return Err(L1Refusal::ZeroVariance);
    }
    let refs: Vec<&[f64]> = samples.iter().map(Vec::as_slice).collect();
    let a = stats_verify::one_way_anova(&refs).map_err(|e| refused(e, L1Refusal::ZeroVariance))?;
    let decimals = stored_decimals(d, outcome)?;
    let range = precision::one_way_f_parts(&refs, precision::half_unit(decimals)).range();
    let precision = f_precision(a.df_between, a.df_within, range, decimals);
    Ok(vec![ComputedResult {
        variant: ComputedVariant::OneWay,
        statistic: StatisticKind::F,
        value: a.f,
        df1: a.df_between,
        df2: Some(a.df_within),
        p: finite_p(pvalue::f_upper(a.f, a.df_between, a.df_within))?,
        tail: Tail::Upper,
        n: y.len(),
        precision,
    }])
}

/// Pearson's r, or Spearman's rho on average ranks; both with df `n - 2` and
/// the two-sided p of `t = r √(df / (1 - r²))`. An |r| of 1 has no finite t
/// and is refused as a non-finite result.
fn correlation(b: &ConfirmedBinding, d: &Dataset, rows: &[usize], spearman: bool) -> Result<Vec<ComputedResult>, L1Refusal> {
    let (x_col, y_col) = (column(b, RoleName::X)?, column(b, RoleName::Y)?);
    let x = numbers(d, x_col, rows)?;
    let y = numbers(d, y_col, rows)?;
    if x.len() < 3 {
        return Err(L1Refusal::TooFewCompleteCases { n: x.len(), minimum: 3 });
    }
    if constant(&x) || constant(&y) {
        return Err(L1Refusal::ZeroVariance);
    }
    let (c, variant, statistic) = if spearman {
        (stats_verify::spearman(&x, &y), ComputedVariant::SpearmanTApproximation, StatisticKind::Rho)
    } else {
        (stats_verify::pearson(&x, &y), ComputedVariant::Pearson, StatisticKind::R)
    };
    let c = c.map_err(|e| refused(e, L1Refusal::ZeroVariance))?;
    let (dx, dy) = (stored_decimals(d, x_col)?, stored_decimals(d, y_col)?);
    let sx = Stored { values: &x, delta: precision::half_unit(dx) };
    let sy = Stored { values: &y, delta: precision::half_unit(dy) };
    let precision = if spearman {
        // Ranks move only when rounding could reorder or tie two values.
        if precision::could_reorder(sx) || precision::could_reorder(sy) {
            Precision::Unbounded { decimals: dx.max(dy) }
        } else {
            Precision::Exact
        }
    } else {
        r_precision(c.df, precision::pearson_parts(sx, sy).range(), dx.max(dy))
    };
    Ok(vec![ComputedResult {
        variant,
        statistic,
        value: c.coefficient,
        df1: c.df,
        df2: None,
        p: finite_p(pvalue::r_two_tailed(c.coefficient, c.df))?,
        tail: Tail::TwoSided,
        n: x.len(),
        precision,
    }])
}

/// The r × c table of the two group columns' selected levels, then the
/// uncorrected statistic, and for a 2×2 table R's continuity correction.
/// No other correction is computed: a plain 0.5 with no floor was dropped in
/// Step 3, because the only SPSS-style formula found (Origin's CROSSTABS)
/// equals R's, and what SPSS prints is unconfirmed (for Step 8's record).
fn chi_square(b: &ConfirmedBinding, d: &Dataset, rows: &[usize]) -> Result<Vec<ComputedResult>, L1Refusal> {
    let row_col = column(b, RoleName::RowVariable)?;
    let col_col = column(b, RoleName::ColumnVariable)?;
    let row_levels = ordered_levels(b, d, RoleName::RowVariable, row_col);
    let col_levels = ordered_levels(b, d, RoleName::ColumnVariable, col_col);
    let (rc, cc) = (cells(d, row_col)?, cells(d, col_col)?);
    let mut table = vec![vec![0.0f64; col_levels.len()]; row_levels.len()];
    for &i in rows {
        let r = rc[i].level_code().and_then(|code| row_levels.iter().position(|l| *l == code));
        let c = cc[i].level_code().and_then(|code| col_levels.iter().position(|l| *l == code));
        if let (Some(r), Some(c)) = (r, c) {
            table[r][c] += 1.0;
        }
    }
    let empty_rows = table.iter().filter(|r| r.iter().all(|&v| v == 0.0)).count();
    let empty_columns = (0..col_levels.len()).filter(|&j| table.iter().all(|r| r[j] == 0.0)).count();
    if empty_rows > 0 || empty_columns > 0 || table.len() < 2 || col_levels.len() < 2 {
        return Err(L1Refusal::EmptyRowOrColumn { empty_rows, empty_columns });
    }
    let n = rows.len();
    let make = |variant, r: stats_verify::ChiSquareResult| -> Result<ComputedResult, L1Refusal> {
        Ok(ComputedResult {
            variant,
            statistic: StatisticKind::ChiSquare,
            value: r.chi_square,
            df1: r.df,
            df2: None,
            p: finite_p(pvalue::chi_square_upper(r.chi_square, r.df))?,
            tail: Tail::Upper,
            n,
            // Counts of levels: nothing stored is rounded.
            precision: Precision::Exact,
        })
    };
    // Every condition chi_square_independence refuses on (shape, negative
    // counts, an empty margin) was ruled out above.
    let uncorrected = stats_verify::chi_square_independence(&table).map_err(|e| refused(e, L1Refusal::EmptyRowOrColumn { empty_rows, empty_columns }))?;
    let mut out = vec![make(ComputedVariant::ChiSquareUncorrected, uncorrected)?];
    if table.len() == 2 && col_levels.len() == 2 {
        let yates = stats_verify::chi_square_2x2_yates(&table).map_err(|e| refused(e, L1Refusal::EmptyRowOrColumn { empty_rows, empty_columns }))?;
        out.push(make(ComputedVariant::ChiSquareYatesR, yates)?);
    }
    Ok(out)
}

/// OLS with an intercept: the overall F on (k, n - k - 1) and every
/// coefficient's t on n - k - 1, whatever the binding's target (Step 4
/// selects it).
fn ols(b: &ConfirmedBinding, d: &Dataset, rows: &[usize]) -> Result<Vec<ComputedResult>, L1Refusal> {
    let y = numbers(d, column(b, RoleName::Outcome)?, rows)?;
    let predictors = b.columns_for(RoleName::Predictors);
    let mut xs = Vec::with_capacity(predictors.len());
    for &p in predictors {
        if !d.schema.column(p).is_some_and(|c| c.is_numeric()) {
            return Err(L1Refusal::NotNumeric { column: p });
        }
        xs.push(numbers(d, p, rows)?);
    }
    let k = predictors.len();
    if y.len() < k + 2 {
        return Err(L1Refusal::TooFewCompleteCases { n: y.len(), minimum: k + 2 });
    }
    if constant(&y) {
        return Err(L1Refusal::ZeroVariance);
    }
    if xs.iter().any(|x| constant(x)) {
        return Err(L1Refusal::SingularDesign);
    }
    // Every other condition stats_verify::ols_qr refuses on (n, lengths) was
    // ruled out above; what is left is a design rank-deficient at R's
    // tolerance.
    let r = stats_verify::ols_qr(&y, &xs).map_err(|e| refused(e, L1Refusal::SingularDesign))?;
    let n = y.len();
    let y_dec = stored_decimals(d, column(b, RoleName::Outcome)?)?;
    let x_dec: Vec<u32> = predictors.iter().map(|&p| stored_decimals(d, p)).collect::<Result<_, _>>()?;
    let decimals = x_dec.iter().copied().chain([y_dec]).max().unwrap_or(0);
    let stored_xs: Vec<Stored<'_>> = xs.iter().zip(&x_dec).map(|(v, &dd)| Stored { values: v, delta: precision::half_unit(dd) }).collect();
    let beta: Vec<f64> = r.coefficients.iter().map(|c| c.estimate).collect();
    // A design that cannot be inverted here, though QR fitted it, gives no
    // first-order range: any miss is then not checked.
    let bounds = precision::ols_parts(Stored { values: &y, delta: precision::half_unit(y_dec) }, &stored_xs, &beta);
    let unbounded = || if decimals == 0 { Precision::Exact } else { Precision::Unbounded { decimals } };
    let mut out = vec![ComputedResult {
        variant: ComputedVariant::OlsOverallF,
        statistic: StatisticKind::F,
        value: r.f_statistic,
        df1: r.df_model,
        df2: Some(r.df_residual),
        p: finite_p(pvalue::f_upper(r.f_statistic, r.df_model, r.df_residual))?,
        tail: Tail::Upper,
        n,
        precision: match &bounds {
            Some((f, _)) => f_precision(r.df_model, r.df_residual, f.range(), decimals),
            None => unbounded(),
        },
    }];
    for (i, coef) in r.coefficients.iter().enumerate() {
        let term = if i == 0 { OlsTerm::Intercept } else { OlsTerm::Predictor(predictors[i - 1]) };
        out.push(ComputedResult {
            variant: ComputedVariant::OlsCoefficient(term),
            statistic: StatisticKind::T,
            value: coef.t,
            df1: r.df_residual,
            df2: None,
            p: finite_p(pvalue::t_two_tailed(coef.t, r.df_residual))?,
            tail: Tail::TwoSided,
            n,
            precision: match &bounds {
                Some((_, ts)) => t_precision(r.df_residual, ts[i].range(), None, decimals),
                None => unbounded(),
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::SectionKind;
    use crate::stats_l0::number::Relation;
    use crate::stats_l0::result::Family;
    use crate::stats_l1::binding::{BindingDraft, OlsTarget, ProposalSource, Rejection};
    use crate::stats_l1::catalogue::{MethodCatalogue, VariantChoice};
    use crate::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
    use crate::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
    use crate::stats_l1::run_state::ConfirmError;
    use crate::stats_l1::token::RevokedBecause;

    const C: fn(u32) -> ColumnAlias = ColumnAlias;

    fn raw(headers: &[&str], rows: &[&[&str]]) -> RawTable {
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        )
    }

    fn dataset(raw: RawTable, bytes: &[u8]) -> Dataset {
        build_dataset(raw, DataFingerprint::of_bytes(bytes, "p", TableRef::FirstTable)).unwrap()
    }

    fn draft(method: L1MethodId, roles: &[(RoleName, &[ColumnAlias])]) -> BindingDraft {
        let mut d = BindingDraft::new(method, VariantChoice::not_stated(MethodCatalogue::entry(method).variant_kind));
        for (r, c) in roles {
            d = d.role(*r, c);
        }
        if method == L1MethodId::Ols {
            d = d.target(OlsTarget::OverallF);
        }
        d
    }

    /// A run with one claim and `draft` confirmed: the state, the token, and
    /// the data read again from the same bytes.
    fn confirmed(raw: RawTable, draft: BindingDraft) -> (L1RunState, ConfirmationToken, ClaimId, Dataset) {
        let mut s = L1RunState::new();
        s.load_dataset(dataset(raw.clone(), b"bytes v1"));
        let claim = s
            .add_claim(&UserClaim {
                family: Family::T,
                statistic: "2.00",
                df: vec!["10"],
                n: None,
                p: UserP::Reported { raw: ".05", relation: Relation::Eq },
                location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
                sentence: None,
            })
            .unwrap();
        let id = s.add_proposal(claim, draft, ProposalSource::Manual { edited_from: None }).unwrap();
        let token = s.confirm(id).unwrap().1;
        (s, token, claim, dataset(raw, b"bytes v1"))
    }

    fn run(raw: RawTable, draft: BindingDraft) -> Result<L1Computation, L1Refusal> {
        let (s, t, c, d) = confirmed(raw, draft);
        compute(&s, &t, c, &d)
    }

    fn t_draft() -> BindingDraft {
        draft(L1MethodId::IndependentT, &[(RoleName::Outcome, &[C(1)]), (RoleName::Group, &[C(2)])])
    }

    #[test]
    fn the_engine_version_names_the_p_engine_it_computes_with() {
        assert!(ENGINE_VERSION.contains(pvalue::ENGINE_VERSION), "{ENGINE_VERSION} does not name {}", pvalue::ENGINE_VERSION);
        assert!(!ENGINE_VERSION.contains("engine 0"), "the engine version must move with the first computation");
    }

    #[test]
    fn the_registry_is_a_subset_of_the_catalogue_and_names_its_fixtures() {
        for m in L1Registry::METHODS {
            assert!(MethodCatalogue::by_id(m.method.id()).is_some(), "{:?} is registered but not in the catalogue", m.method);
            assert!(!m.statistic_from.is_empty());
            assert!(m.fixtures.ends_with(".csv"));
            assert!(m.relative_gate > 0.0 && m.relative_gate <= 1e-9, "{:?}: {}", m.method, m.relative_gate);
        }
        assert_eq!(L1Registry::METHODS.len(), 7);
    }

    #[test]
    fn a_method_outside_the_registry_is_method_not_available_never_the_nearest_one() {
        for id in ["mann_whitney", "welch_anova", "one_sample_t", "chi_square_goodness_of_fit", "kendall", "", "Pearson", "independent_t "] {
            assert_eq!(L1Registry::by_id(id), Err(L1Refusal::MethodNotAvailable), "{id:?}");
        }
        assert!(L1Registry::by_id("independent_t").is_ok());
    }

    #[test]
    fn counts_record_filtered_missing_and_used_rows_by_reason_never_which() {
        let r = raw(
            &["y", "g", "site"],
            &[&["1.0", "1", "a"], &["2.5", "2", "a"], &["", "1", "a"], &["4.0", "2", "b"], &["3.0", "1", "a"], &["5.5", "2", "a"], &["2.0", "1", "a"]],
        );
        let c = run(r, t_draft().filter(C(3), &["a"])).unwrap();
        assert_eq!(c.counts.rows_read, 7);
        assert_eq!(
            c.counts.excluded,
            vec![
                Exclusion { reason: ExclusionReason::FilteredOut, rows: 1 },
                Exclusion { reason: ExclusionReason::MissingInBoundColumn, rows: 1 },
            ]
        );
        assert_eq!(c.counts.n_used, 5);
        assert_eq!(c.results.iter().map(|r| r.variant).collect::<Vec<_>>(), vec![ComputedVariant::StudentPooled, ComputedVariant::Welch]);
        assert!(c.results.iter().all(|r| r.n == 5 && r.tail == Tail::TwoSided));
        assert_eq!(c.engine_version, ENGINE_VERSION);
    }

    #[test]
    fn only_a_live_token_reaches_a_computation() {
        let r = raw(&["y", "g"], &[&["1", "1"], &["2", "1"], &["3", "2"], &["5", "2"]]);
        let (mut s, t, c, d) = confirmed(r, t_draft());
        assert!(compute(&s, &t, c, &d).is_ok());
        assert_eq!(compute(&s, &ConfirmationToken::presented(&"0".repeat(64)), c, &d), Err(L1Refusal::Gate(GateRejection::Unknown)));
        assert_eq!(compute(&s, &t, ClaimId(9), &d), Err(L1Refusal::Gate(GateRejection::ClaimMismatch)));
        s.withdraw(&t);
        assert_eq!(compute(&s, &t, c, &d), Err(L1Refusal::Gate(GateRejection::Revoked(RevokedBecause::Withdrawn))));
    }

    /// The data read again must be the data confirmed on: one changed byte in
    /// the file is a refusal, even with a live token.
    #[test]
    fn data_read_again_with_another_fingerprint_is_refused() {
        let r = raw(&["y", "g"], &[&["1", "1"], &["2", "1"], &["3", "2"], &["5", "2"]]);
        let (s, t, c, _) = confirmed(r.clone(), t_draft());
        let changed = dataset(r, b"bytes v2");
        assert_eq!(compute(&s, &t, c, &changed), Err(L1Refusal::FingerprintChanged));
    }

    #[test]
    fn a_weighted_file_is_refused() {
        let mut r = raw(&["y", "g", "w"], &[&["1", "1", "1"], &["2", "1", "2"], &["3", "2", "1"], &["5", "2", "1"]]);
        r.weight_variable = Some("w".into());
        assert_eq!(run(r.clone(), t_draft()), Err(L1Refusal::WeightedFile));
        // A declared weight that names no column is refused too.
        r.weight_variable = Some("gone".into());
        assert_eq!(run(r, t_draft()), Err(L1Refusal::WeightedFile));
    }

    #[test]
    fn too_few_complete_cases_is_refused_with_the_count_and_the_minimum() {
        // A group of one after listwise deletion.
        let r = raw(&["y", "g"], &[&["1", "1"], &["", "1"], &["3", "2"], &["5", "2"], &["4", "2"]]);
        assert_eq!(run(r, t_draft()), Err(L1Refusal::TooFewCompleteCases { n: 1, minimum: 2 }));
        // Two complete pairs for a correlation.
        let r = raw(&["x", "y"], &[&["1", "2"], &["2", ""], &["3", "5"], &["", "1"]]);
        let d = draft(L1MethodId::Pearson, &[(RoleName::X, &[C(1)]), (RoleName::Y, &[C(2)])]);
        assert_eq!(run(r, d), Err(L1Refusal::TooFewCompleteCases { n: 2, minimum: 3 }));
        // OLS with two predictors needs four rows.
        let r = raw(&["y", "a", "b"], &[&["1", "2", "7"], &["2", "3", "1"], &["4", "1", "5"]]);
        let d = draft(L1MethodId::Ols, &[(RoleName::Outcome, &[C(1)]), (RoleName::Predictors, &[C(2), C(3)])]);
        assert_eq!(run(r, d), Err(L1Refusal::TooFewCompleteCases { n: 3, minimum: 4 }));
    }

    #[test]
    fn zero_variance_is_refused_exactly_even_where_the_floating_mean_is_not_the_value() {
        let r = raw(&["y", "g"], &[&["0.1", "1"], &["0.1", "1"], &["0.1", "1"], &["0.7", "2"], &["0.7", "2"]]);
        assert_eq!(run(r, t_draft()), Err(L1Refusal::ZeroVariance));
        // The premise: stats_verify alone would not have refused it.
        assert!(stats_verify::t_test_welch(&[0.1, 0.1, 0.1], &[0.7, 0.7]).is_ok());
        let r = raw(&["pre", "post"], &[&["1", "0"], &["2", "1"], &["5", "4"]]);
        let d = draft(L1MethodId::PairedT, &[(RoleName::First, &[C(1)]), (RoleName::Second, &[C(2)])]);
        assert_eq!(run(r, d), Err(L1Refusal::ZeroVariance));
        let r = raw(&["x", "y"], &[&["1", "2"], &["2", "2"], &["3", "2"]]);
        let d = draft(L1MethodId::Spearman, &[(RoleName::X, &[C(1)]), (RoleName::Y, &[C(2)])]);
        assert_eq!(run(r, d), Err(L1Refusal::ZeroVariance));
        let r = raw(&["y", "a"], &[&["3", "1"], &["3", "2"], &["3", "4"]]);
        let d = draft(L1MethodId::Ols, &[(RoleName::Outcome, &[C(1)]), (RoleName::Predictors, &[C(2)])]);
        assert_eq!(run(r, d), Err(L1Refusal::ZeroVariance));
    }

    #[test]
    fn a_single_group_after_deletion_is_refused() {
        // Every group-2 outcome is missing.
        let r = raw(&["y", "g"], &[&["1", "1"], &["2", "1"], &["4", "1"], &["", "2"], &["", "2"]]);
        assert_eq!(run(r.clone(), t_draft()), Err(L1Refusal::SingleGroup { groups: 1 }));
        let d = draft(L1MethodId::OneWayAnova, &[(RoleName::Outcome, &[C(1)]), (RoleName::Group, &[C(2)])]);
        assert_eq!(run(r, d), Err(L1Refusal::SingleGroup { groups: 1 }));
    }

    #[test]
    fn a_contingency_table_with_an_empty_row_or_column_is_refused() {
        // Level `c` of the column variable occurs only where the row variable
        // is missing, so its column is all zeros after deletion.
        let r = raw(
            &["a", "b"],
            &[&["x", "p"], &["y", "q"], &["x", "q"], &["y", "p"], &["", "c"], &["x", "p"]],
        );
        let d = draft(L1MethodId::ChiSquareIndependence, &[(RoleName::RowVariable, &[C(1)]), (RoleName::ColumnVariable, &[C(2)])]);
        assert_eq!(run(r, d), Err(L1Refusal::EmptyRowOrColumn { empty_rows: 0, empty_columns: 1 }));
    }

    #[test]
    fn a_singular_design_is_refused() {
        let collinear = raw(&["y", "a", "b"], &[&["1", "1", "2"], &["3", "2", "4"], &["2", "3", "6"], &["5", "4", "8"], &["4", "5", "10"]]);
        let d = draft(L1MethodId::Ols, &[(RoleName::Outcome, &[C(1)]), (RoleName::Predictors, &[C(2), C(3)])]);
        assert_eq!(run(collinear, d.clone()), Err(L1Refusal::SingularDesign));
        let constant = raw(&["y", "a", "b"], &[&["1", "1", "7"], &["3", "2", "7"], &["2", "3", "7"], &["5", "4", "7"]]);
        assert_eq!(run(constant, d), Err(L1Refusal::SingularDesign));
    }

    /// A categorical predictor never reaches the computation: the confirm
    /// pass rejects it, and the computation's own check refuses a non-number
    /// in a numeric role with the column's alias.
    #[test]
    fn a_categorical_predictor_is_refused() {
        let r = raw(&["y", "a", "kind"], &[&["1", "1", "x"], &["3", "2", "y"], &["2", "3", "x"], &["5", "4", "z"]]);
        let mut s = L1RunState::new();
        let data = dataset(r.clone(), b"b");
        s.load_dataset(data.clone());
        let claim = s
            .add_claim(&UserClaim {
                family: Family::T,
                statistic: "2",
                df: vec!["1"],
                n: None,
                p: UserP::Reported { raw: ".05", relation: Relation::Eq },
                location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
                sentence: None,
            })
            .unwrap();
        let d = draft(L1MethodId::Ols, &[(RoleName::Outcome, &[C(1)]), (RoleName::Predictors, &[C(2), C(3)])]);
        let id = s.add_proposal(claim, d, ProposalSource::Manual { edited_from: None }).unwrap();
        assert!(matches!(s.confirm(id).unwrap_err(), ConfirmError::Rejected(Rejection::WrongType { role: RoleName::Predictors, column, .. }) if column == C(3)));
        assert_eq!(numbers(&data, C(3), &[0, 1]), Err(L1Refusal::NotNumeric { column: C(3) }));
    }

    #[test]
    fn a_non_finite_value_is_refused() {
        // Squares of 1e200 overflow: Welch's df is inf/inf.
        let r = raw(&["y", "g"], &[&["1e200", "1"], &["-1e200", "1"], &["3e200", "2"], &["-2e200", "2"]]);
        assert_eq!(run(r, t_draft()), Err(L1Refusal::NonFinite { at: NonFiniteAt::Result }));
        // A perfect correlation whose r comes out exactly 1 has no finite t.
        // Sums of squares of 4 make sqrt exact, so r = 4 / (2 * 2) = 1.
        let d = draft(L1MethodId::Pearson, &[(RoleName::X, &[C(1)]), (RoleName::Y, &[C(2)])]);
        let r = raw(&["x", "y"], &[&["0", "0"], &["0", "0"], &["2", "2"], &["2", "2"]]);
        assert_eq!(run(r, d.clone()), Err(L1Refusal::NonFinite { at: NonFiniteAt::Result }));
        // Measured, not chosen: on y = 2x, stats_verify's r is 1 - 2.2e-16
        // and the result is computed, where R prints r = 1, t = Inf, p = 0.
        // Both agree with any printed r = 1.00, p < .001; the Step 3 report
        // lists the boundary as open.
        let r = raw(&["x", "y"], &[&["1", "2"], &["2", "4"], &["3", "6"], &["4", "8"]]);
        let c = run(r, d).unwrap();
        assert!((1.0 - c.results[0].value) < 1e-15 && c.results[0].p < 1e-15, "{:?}", c.results);
        // A non-finite number in a bound cell. The schema types such a column
        // as text, so this is the computation's own check, reached directly.
        let mut data = dataset(raw(&["y"], &[&["1"], &["2"]]), b"b");
        data.replace_cell_for_test(C(1), 1, Cell::Number(f64::INFINITY));
        assert_eq!(numbers(&data, C(1), &[0, 1]), Err(L1Refusal::NonFinite { at: NonFiniteAt::Input }));
    }

    #[test]
    fn an_independent_t_is_the_first_level_minus_the_second_in_level_order() {
        // Level order is numeric for numeric codes: 2 before 10.
        let r = raw(&["y", "g"], &[&["5", "10"], &["6", "10"], &["7", "10"], &["1", "2"], &["2", "2"], &["4", "2"]]);
        let c = run(r, t_draft()).unwrap();
        assert!(c.results.iter().all(|r| r.value < 0.0), "group 2 (mean 2.33) minus group 10 (mean 6): {:?}", c.results);
    }
}
