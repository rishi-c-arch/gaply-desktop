//! Statistical Analysis Verifier — result-matching + the verified-vs-advisory
//! verdict (Set 3).
//!
//! The user says what they ran — a typed [`AnalysisSpec`] (which test, which
//! columns play which role, the value they reported) — this module recomputes
//! it with the Set 2 engine ([`crate::stats_verify`]) and compares reported vs
//! recomputed within a documented tolerance. The output is split into two
//! STRUCTURALLY DISTINCT lanes:
//!
//! - [`VerifiedResult`] — a deterministic recomputation-and-comparison. It is
//!   the ONLY type that carries [`CertaintyTier::MathematicallyCertain`]. Its
//!   verdict is [`Verdict::Match`] (🟢 — reported ≈ recomputed) or
//!   [`Verdict::Mismatch`] (🔴 — reported differs; BOTH values are shown). The
//!   recomputed value is ALWAYS shown, tolerance always stated.
//! - [`AdvisoryNote`] — a methodology observation (🟡), reusing
//!   [`crate::validate`]'s five deterministic rules. It carries a REQUIRED,
//!   never-empty disclaimer and has NO certainty-tier and NO verdict field.
//!
//! The separation is enforced BY CONSTRUCTION: there is no `From`/`Into`, no
//! function, no field that turns an [`AdvisoryNote`] into a [`VerifiedResult`].
//! `MathematicallyCertain` lives on `VerifiedResult` alone, so an advisory note
//! is structurally incapable of rendering as verified.
//!
//! Purity: this module contains **no model, no proxy, no network, no I/O**. The
//! verified lane is deterministic arithmetic (via the Set 2 engine, `k = 1.0`);
//! the advisory lane reuses `validate.rs`, itself deterministic and LLM-free.
//! Set 4 will add the interpretive (model-backed) advisory; THIS set's advisory
//! is the deterministic `validate.rs` rules only. It modifies neither the Set 2
//! engine nor `validate.rs` — it consumes them.

use serde::{Deserialize, Serialize};

use crate::error::GaplyError;
use crate::extract::stats::{Stat, StatClaim};
use crate::extract::Location;
use crate::report::CertaintyTier;
use crate::stats_verify::{
    chi_square_independence, numeric_column, one_way_anova, ols, pearson, spearman, t_test_student,
    t_test_welch, MissingPolicy,
};
use crate::validate::{Flag, RuleId, Severity, StatsValidityReport};

/// Advisory notes are ADVICE, never a verification. Required on every note.
pub const ADVISORY_DISCLAIMER: &str =
    "This is ADVICE about methodology, NOT a verification — a human statistician should judge.";

/// The lane-distinguishing disclosure. Required, never empty, on every report.
pub const LANE_DISCLOSURE: &str =
    "VERIFIED results are deterministic recomputations from your data. ADVISORY notes are \
     methodology observations, NOT verifications — consult a statistician.";

// ---------------------------------------------------------------------------
// The analysis spec — what the user says they ran
// ---------------------------------------------------------------------------

/// Which statistical test the user reports running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    /// Independent-samples t-test, Welch's (unequal variances — the default).
    TTestWelch,
    /// Independent-samples t-test, Student's pooled.
    TTestStudent,
    Anova,
    Pearson,
    Spearman,
    ChiSquare,
    Ols,
}

impl TestKind {
    /// The name of the statistic this test's verdict compares (t / F / r / …).
    pub fn statistic_name(&self) -> &'static str {
        match self {
            TestKind::TTestWelch | TestKind::TTestStudent => "t",
            TestKind::Anova => "F",
            TestKind::Pearson => "r",
            TestKind::Spearman => "rho",
            TestKind::ChiSquare => "chi_square",
            TestKind::Ols => "beta",
        }
    }
}

/// Which columns play which role. The app CANNOT know which columns produced a
/// reported number without the user saying so — this is the user-confirmed
/// mapping. Column references are header names (matching [`numeric_column`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "layout")]
pub enum ColumnRoles {
    /// Independent samples, one per column (wide format): t-test = 2 columns,
    /// ANOVA = k. Each column is typed independently (missing values skipped).
    Samples { columns: Vec<String> },
    /// Long format: a numeric value column split by a categorical group column
    /// (t-test = 2 distinct groups, ANOVA = k). Row-aligned complete cases.
    Grouped { value: String, group: String },
    /// Two paired numeric columns (Pearson / Spearman). Row-aligned complete
    /// cases (a missing cell drops the pair — never misaligns the columns).
    Paired { x: String, y: String },
    /// A contingency table: each row is one category level, each named column
    /// an observed-count column for the other categorical.
    Contingency { count_columns: Vec<String> },
    /// Regression: one outcome column + one or more predictors. Complete cases.
    Regression { outcome: String, predictors: Vec<String> },
}

/// The value(s) the user reported — the thing being checked against the data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReportedStatistic {
    /// The primary test statistic the user reported (t / F / r / χ² / β).
    pub statistic: Option<f64>,
    /// The p-value the user reported.
    pub p_value: Option<f64>,
    /// For OLS: which coefficient the reported statistic/p refers to
    /// (0 = intercept, 1 = first predictor, …). Defaults to 1 (the slope).
    pub coefficient_index: Option<usize>,
}

impl ReportedStatistic {
    /// Pre-fill from extracted claims. Extraction (`extract/stats.rs`) captures
    /// the p-VALUE but NOT the test-statistic value, so only `p_value` can be
    /// pre-filled honestly; the statistic stays user-provided.
    pub fn from_claims(claims: &[StatClaim]) -> Self {
        let p_value = claims.iter().find_map(|c| match &c.stat {
            // an `=`-reported p is a point value we can compare; inequalities
            // (p < .05) are bounds, not values — left for the user to confirm.
            Stat::PValue { operator, value, .. } if operator == "=" => Some(*value),
            _ => None,
        });
        ReportedStatistic { statistic: None, p_value, coefficient_index: None }
    }
}

/// Comparison tolerance. Reported statistics are usually rounded to 2–3
/// decimals, so an absolute slack of 0.005 admits a correctly-rounded 2-dp
/// value (whose true error is ≤ 0.005), plus a small relative term for large
/// statistics reported to few significant figures.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    /// Absolute slack on the statistic.
    pub statistic_abs: f64,
    /// Relative slack on the statistic (fraction of |recomputed|).
    pub statistic_rel: f64,
    /// Absolute slack on the p-value.
    pub p_abs: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        // 0.005 = the maximum rounding error of a value given to 2 decimals.
        Tolerance { statistic_abs: 0.005, statistic_rel: 0.01, p_abs: 0.005 }
    }
}

impl Tolerance {
    fn statistic_matches(&self, reported: f64, recomputed: f64) -> bool {
        (reported - recomputed).abs() <= self.statistic_abs + self.statistic_rel * recomputed.abs()
    }
    fn p_matches(&self, reported: f64, recomputed: f64) -> bool {
        (reported - recomputed).abs() <= self.p_abs
    }
    /// One-line, human-readable statement of the slack applied.
    pub fn describe(&self) -> String {
        format!(
            "statistic within ±{} (+{}·|value|), p within ±{} — sized for values rounded to ~2 decimals",
            self.statistic_abs, self.statistic_rel, self.p_abs
        )
    }
}

/// The full user-confirmed specification of one reported analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisSpec {
    pub test_kind: TestKind,
    pub roles: ColumnRoles,
    pub reported: ReportedStatistic,
    #[serde(default = "Tolerance::default")]
    pub tolerance: Tolerance,
}

// ---------------------------------------------------------------------------
// The recomputed ground truth
// ---------------------------------------------------------------------------

/// The engine's recomputation, reduced to the comparable (statistic, p) pair
/// plus the full detail (df, means, R², …) — the ALWAYS-shown ground truth.
/// Output type (Serialize-only, like the report.rs findings it sits beside).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Recomputed {
    pub test_kind: TestKind,
    /// "t" / "F" / "r" / "rho" / "chi_square" / "beta".
    pub statistic_name: &'static str,
    pub statistic: f64,
    pub p_value: f64,
    /// df, means, R², which coefficient — human-readable supporting detail.
    pub detail: String,
    /// Rows dropped as incomplete during typing (complete-case lanes).
    pub rows_dropped: usize,
}

// ---------------------------------------------------------------------------
// The verified lane
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Reported ≈ recomputed within tolerance (🟢).
    Match,
    /// Reported differs beyond tolerance (🔴) — both values shown as evidence.
    Mismatch,
}

/// A deterministic recomputation compared against what the user reported. This
/// is the ONLY type carrying [`CertaintyTier::MathematicallyCertain`].
/// Output type (Serialize-only — it embeds the Serialize-only `CertaintyTier`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerifiedResult {
    pub verdict: Verdict,
    /// Always [`CertaintyTier::MathematicallyCertain`] — a recomputation is
    /// certain whether it matches or not. The 🟢/🔴 is `verdict`, not this.
    pub tier: CertaintyTier,
    /// The engine's ground truth — ALWAYS present.
    pub recomputed: Recomputed,
    /// What the user reported — echoed so the mismatch evidence is self-contained.
    pub reported: ReportedStatistic,
    /// reported − recomputed for the statistic (when a statistic was reported).
    pub statistic_delta: Option<f64>,
    /// reported − recomputed for the p-value (when a p-value was reported).
    pub p_delta: Option<f64>,
    pub tolerance: Tolerance,
    /// The honest evidence line, e.g. "you reported t = 2.41; recomputing from
    /// your data gives t = 2.08 (Δ = 0.33, beyond ±0.005) → MISMATCH".
    pub explanation: String,
}

// ---------------------------------------------------------------------------
// The advisory lane (reuses validate.rs — no conversion to the verified lane)
// ---------------------------------------------------------------------------

/// A methodology observation (🟡). Deliberately has NO certainty-tier field and
/// NO verdict field: it cannot be constructed or coerced into a
/// [`VerifiedResult`]. Its only constructor is [`AdvisoryNote::from_flag`],
/// which always sets the required, non-empty [`ADVISORY_DISCLAIMER`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdvisoryNote {
    pub rule: RuleId,
    pub severity: Severity,
    pub location: Location,
    /// The methodology observation (from the validate.rs rule).
    pub observation: String,
    /// REQUIRED, never empty — this is advice, not a verification.
    pub disclaimer: String,
}

impl AdvisoryNote {
    /// The only way to build an advisory note: from a validate.rs [`Flag`]. The
    /// disclaimer is always the non-empty [`ADVISORY_DISCLAIMER`].
    pub fn from_flag(flag: &Flag) -> Self {
        AdvisoryNote {
            rule: flag.rule,
            severity: flag.severity,
            location: flag.location.clone(),
            observation: flag.explanation.clone(),
            disclaimer: ADVISORY_DISCLAIMER.to_string(),
        }
    }
}

/// Map a validate.rs report into advisory notes (🟡). Pure reuse — validate.rs
/// is untouched and produced the flags; we only re-dress each as advice.
pub fn advisory_notes(report: &StatsValidityReport) -> Vec<AdvisoryNote> {
    report.flags.iter().map(AdvisoryNote::from_flag).collect()
}

// ---------------------------------------------------------------------------
// The combined verdict report
// ---------------------------------------------------------------------------

/// The two lanes side by side, with the required lane disclosure.
/// Output type (Serialize-only — embeds the Serialize-only `VerifiedResult`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerificationReport {
    /// The verified lane — present when the data could be recomputed.
    pub verified: Option<VerifiedResult>,
    /// Honest error when recomputation was impossible (bad columns, non-numeric
    /// data, wrong roles for the test). No silent fallback.
    pub recompute_error: Option<String>,
    /// The advisory lane (methodology).
    pub advisory: Vec<AdvisoryNote>,
    /// Required, never empty — distinguishes the lanes.
    pub disclosure: String,
}

// ---------------------------------------------------------------------------
// Column typing helpers (complete-case where alignment matters)
// ---------------------------------------------------------------------------

fn missing_token(cell: &str) -> bool {
    let t = cell.trim();
    t.is_empty()
        || matches!(
            t.to_ascii_uppercase().as_str(),
            "NA" | "N/A" | "NAN" | "NULL" | "NONE" | "." | "#N/A"
        )
}

fn col_index(headers: &[String], header: &str) -> Result<usize, GaplyError> {
    headers
        .iter()
        .position(|h| h == header)
        .ok_or_else(|| GaplyError::Validation(format!("column '{header}' not found in the table")))
}

/// Row-aligned complete-case read of several numeric columns: a row is kept
/// only if every selected cell is present and numeric; a missing cell drops the
/// whole row (returned as `dropped`); a non-numeric non-missing cell is an
/// honest error. Guarantees all returned columns share one length.
fn complete_cases(
    headers: &[String],
    rows: &[Vec<String>],
    cols: &[&str],
) -> Result<(Vec<Vec<f64>>, usize), GaplyError> {
    let idxs: Vec<usize> = cols.iter().map(|c| col_index(headers, c)).collect::<Result<_, _>>()?;
    let mut out: Vec<Vec<f64>> = vec![Vec::new(); cols.len()];
    let mut dropped = 0usize;
    for (r, row) in rows.iter().enumerate() {
        let cells: Vec<&str> = idxs.iter().map(|&i| row.get(i).map(|s| s.as_str()).unwrap_or("")).collect();
        if cells.iter().any(|c| missing_token(c)) {
            dropped += 1;
            continue;
        }
        let mut parsed = Vec::with_capacity(cols.len());
        for (c, cell) in cols.iter().zip(&cells) {
            let v: f64 = cell.trim().parse().map_err(|_| {
                GaplyError::Validation(format!(
                    "column '{c}' has a non-numeric value {cell:?} at row {} — refusing to guess",
                    r + 1
                ))
            })?;
            if !v.is_finite() {
                return Err(GaplyError::Validation(format!("column '{c}' has a non-finite value at row {}", r + 1)));
            }
            parsed.push(v);
        }
        for (k, v) in parsed.into_iter().enumerate() {
            out[k].push(v);
        }
    }
    Ok((out, dropped))
}

/// Long-format grouping: numeric `value` split by categorical `group` labels,
/// row-aligned. Groups are returned in first-appearance order (deterministic).
/// Rows with a missing value or blank group are dropped (counted).
fn grouped_samples(
    headers: &[String],
    rows: &[Vec<String>],
    value: &str,
    group: &str,
) -> Result<(Vec<(String, Vec<f64>)>, usize), GaplyError> {
    let vi = col_index(headers, value)?;
    let gi = col_index(headers, group)?;
    let mut order: Vec<String> = Vec::new();
    let mut buckets: Vec<Vec<f64>> = Vec::new();
    let mut dropped = 0usize;
    for (r, row) in rows.iter().enumerate() {
        let vcell = row.get(vi).map(|s| s.as_str()).unwrap_or("");
        let gcell = row.get(gi).map(|s| s.as_str()).unwrap_or("");
        if missing_token(vcell) || gcell.trim().is_empty() {
            dropped += 1;
            continue;
        }
        let v: f64 = vcell.trim().parse().map_err(|_| {
            GaplyError::Validation(format!(
                "value column '{value}' has a non-numeric value {vcell:?} at row {} — refusing to guess",
                r + 1
            ))
        })?;
        if !v.is_finite() {
            return Err(GaplyError::Validation(format!("value column '{value}' has a non-finite value at row {}", r + 1)));
        }
        let label = gcell.trim().to_string();
        match order.iter().position(|l| *l == label) {
            Some(k) => buckets[k].push(v),
            None => {
                order.push(label);
                buckets.push(vec![v]);
            }
        }
    }
    Ok((order.into_iter().zip(buckets).collect(), dropped))
}

/// Read an r×c contingency table of counts from the named columns.
fn contingency(
    headers: &[String],
    rows: &[Vec<String>],
    count_columns: &[String],
) -> Result<Vec<Vec<f64>>, GaplyError> {
    if count_columns.len() < 2 {
        return Err(GaplyError::Validation("chi-square needs at least 2 count columns".into()));
    }
    let refs: Vec<&str> = count_columns.iter().map(String::as_str).collect();
    let (cols, _dropped) = complete_cases(headers, rows, &refs)?;
    // cols is column-major; transpose to row-major (each row = one category).
    let n = cols[0].len();
    if n < 2 {
        return Err(GaplyError::Validation("chi-square needs at least 2 category rows".into()));
    }
    let observed: Vec<Vec<f64>> = (0..n).map(|i| cols.iter().map(|c| c[i]).collect()).collect();
    Ok(observed)
}

// ---------------------------------------------------------------------------
// Recompute dispatch
// ---------------------------------------------------------------------------

fn independent_samples(
    headers: &[String],
    rows: &[Vec<String>],
    roles: &ColumnRoles,
) -> Result<(Vec<(String, Vec<f64>)>, usize), GaplyError> {
    match roles {
        ColumnRoles::Samples { columns } => {
            // Independent columns: each typed on its own (skip missing).
            let mut groups = Vec::with_capacity(columns.len());
            let mut dropped = 0usize;
            for c in columns {
                let col = numeric_column(headers, rows, c, MissingPolicy::Skip)?;
                dropped += col.missing;
                groups.push((c.clone(), col.values));
            }
            Ok((groups, dropped))
        }
        ColumnRoles::Grouped { value, group } => grouped_samples(headers, rows, value, group),
        other => Err(GaplyError::Validation(format!(
            "this test needs Samples or Grouped column roles, got {other:?}"
        ))),
    }
}

/// Recompute the spec's test on the mapped columns. Honest error if the roles
/// are incompatible with the test or the data can't be typed.
pub fn recompute(
    spec: &AnalysisSpec,
    headers: &[String],
    rows: &[Vec<String>],
) -> Result<Recomputed, GaplyError> {
    let kind = spec.test_kind;
    match kind {
        TestKind::TTestWelch | TestKind::TTestStudent => {
            let (groups, dropped) = independent_samples(headers, rows, &spec.roles)?;
            if groups.len() != 2 {
                return Err(GaplyError::Validation(format!(
                    "a t-test compares exactly 2 groups, but the roles yielded {}",
                    groups.len()
                )));
            }
            let (a, b) = (&groups[0].1, &groups[1].1);
            let r = if kind == TestKind::TTestWelch {
                t_test_welch(a, b)?
            } else {
                t_test_student(a, b)?
            };
            Ok(Recomputed {
                test_kind: kind,
                statistic_name: "t",
                statistic: r.t,
                p_value: r.p_two_sided,
                detail: format!(
                    "{} t-test: df = {:.4}, mean({}) = {:.6}, mean({}) = {:.6}",
                    r.method, r.df, groups[0].0, r.mean_a, groups[1].0, r.mean_b
                ),
                rows_dropped: dropped,
            })
        }
        TestKind::Anova => {
            let (groups, dropped) = independent_samples(headers, rows, &spec.roles)?;
            if groups.len() < 2 {
                return Err(GaplyError::Validation("ANOVA needs at least 2 groups".into()));
            }
            let refs: Vec<&[f64]> = groups.iter().map(|g| g.1.as_slice()).collect();
            let r = one_way_anova(&refs)?;
            Ok(Recomputed {
                test_kind: kind,
                statistic_name: "F",
                statistic: r.f,
                p_value: r.p,
                detail: format!(
                    "one-way ANOVA: {} groups, df_between = {}, df_within = {}",
                    groups.len(),
                    r.df_between,
                    r.df_within
                ),
                rows_dropped: dropped,
            })
        }
        TestKind::Pearson | TestKind::Spearman => {
            let (x, y) = match &spec.roles {
                ColumnRoles::Paired { x, y } => (x, y),
                other => {
                    return Err(GaplyError::Validation(format!(
                        "correlation needs Paired column roles, got {other:?}"
                    )))
                }
            };
            let (cols, dropped) = complete_cases(headers, rows, &[x, y])?;
            let r = if kind == TestKind::Pearson {
                pearson(&cols[0], &cols[1])?
            } else {
                spearman(&cols[0], &cols[1])?
            };
            Ok(Recomputed {
                test_kind: kind,
                statistic_name: if kind == TestKind::Pearson { "r" } else { "rho" },
                statistic: r.coefficient,
                p_value: r.p_two_sided,
                detail: format!("{} correlation: df = {}, t = {:.6}", r.method, r.df, r.statistic),
                rows_dropped: dropped,
            })
        }
        TestKind::ChiSquare => {
            let cols = match &spec.roles {
                ColumnRoles::Contingency { count_columns } => count_columns,
                other => {
                    return Err(GaplyError::Validation(format!(
                        "chi-square needs Contingency column roles, got {other:?}"
                    )))
                }
            };
            let observed = contingency(headers, rows, cols)?;
            let r = chi_square_independence(&observed)?;
            Ok(Recomputed {
                test_kind: kind,
                statistic_name: "chi_square",
                statistic: r.chi_square,
                p_value: r.p,
                detail: format!("chi-square test of independence: df = {}", r.df),
                rows_dropped: 0,
            })
        }
        TestKind::Ols => {
            let (outcome, predictors) = match &spec.roles {
                ColumnRoles::Regression { outcome, predictors } => (outcome, predictors),
                other => {
                    return Err(GaplyError::Validation(format!(
                        "regression needs Regression column roles, got {other:?}"
                    )))
                }
            };
            let mut names: Vec<&str> = vec![outcome.as_str()];
            names.extend(predictors.iter().map(String::as_str));
            let (cols, dropped) = complete_cases(headers, rows, &names)?;
            let y = &cols[0];
            let preds: Vec<Vec<f64>> = cols[1..].to_vec();
            let r = ols(y, &preds)?;
            let idx = spec.reported.coefficient_index.unwrap_or(1).min(r.coefficients.len() - 1);
            let c = &r.coefficients[idx];
            let label = if idx == 0 { "(intercept)".to_string() } else { predictors[idx - 1].clone() };
            Ok(Recomputed {
                test_kind: kind,
                statistic_name: "beta",
                statistic: c.estimate,
                p_value: c.p_two_sided,
                detail: format!(
                    "OLS: coefficient '{}' (β = {:.6}, SE = {:.6}, t = {:.6}), R² = {:.6}, df_resid = {}",
                    label, c.estimate, c.std_error, c.t, r.r_squared, r.df_residual
                ),
                rows_dropped: dropped,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------------

/// Recompute + compare → a [`VerifiedResult`]. The verified lane only; the
/// recomputed ground truth is always present, tolerance always stated.
pub fn verify(
    spec: &AnalysisSpec,
    headers: &[String],
    rows: &[Vec<String>],
) -> Result<VerifiedResult, GaplyError> {
    let recomputed = recompute(spec, headers, rows)?;
    let tol = spec.tolerance;

    let statistic_delta = spec.reported.statistic.map(|r| r - recomputed.statistic);
    let p_delta = spec.reported.p_value.map(|r| r - recomputed.p_value);

    // Mismatch if ANY reported value falls outside tolerance. With nothing
    // reported, the recomputation stands as ground truth (vacuous match).
    let stat_ok = spec.reported.statistic.map_or(true, |r| tol.statistic_matches(r, recomputed.statistic));
    let p_ok = spec.reported.p_value.map_or(true, |r| tol.p_matches(r, recomputed.p_value));
    let verdict = if stat_ok && p_ok { Verdict::Match } else { Verdict::Mismatch };

    let name = recomputed.statistic_name;
    let explanation = match (spec.reported.statistic, spec.reported.p_value) {
        (None, None) => format!(
            "No reported value supplied; recomputing from your data gives {name} = {:.4}, p = {:.4} (shown as ground truth).",
            recomputed.statistic, recomputed.p_value
        ),
        _ => {
            let mut parts = Vec::new();
            if let Some(r) = spec.reported.statistic {
                let ok = tol.statistic_matches(r, recomputed.statistic);
                parts.push(format!(
                    "you reported {name} = {r}; recomputing gives {name} = {:.4} (Δ = {:.4}{})",
                    recomputed.statistic,
                    r - recomputed.statistic,
                    if ok { "" } else { ", beyond tolerance" }
                ));
            }
            if let Some(r) = spec.reported.p_value {
                let ok = tol.p_matches(r, recomputed.p_value);
                parts.push(format!(
                    "you reported p = {r}; recomputing gives p = {:.4} (Δ = {:.4}{})",
                    recomputed.p_value,
                    r - recomputed.p_value,
                    if ok { "" } else { ", beyond tolerance" }
                ));
            }
            format!(
                "{} → {}. Tolerance: {}.",
                parts.join("; "),
                match verdict {
                    Verdict::Match => "MATCH",
                    Verdict::Mismatch => "MISMATCH",
                },
                tol.describe()
            )
        }
    };

    Ok(VerifiedResult {
        verdict,
        tier: CertaintyTier::MathematicallyCertain,
        recomputed,
        reported: spec.reported.clone(),
        statistic_delta,
        p_delta,
        tolerance: tol,
        explanation,
    })
}

/// Assemble the full two-lane report. The verified lane recomputes+compares (or
/// records an honest recompute error); the advisory lane maps validate.rs's
/// deterministic flags. The lane disclosure is always attached.
pub fn verify_analysis(
    spec: &AnalysisSpec,
    headers: &[String],
    rows: &[Vec<String>],
    validity: Option<&StatsValidityReport>,
) -> VerificationReport {
    let (verified, recompute_error) = match verify(spec, headers, rows) {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e.to_string())),
    };
    VerificationReport {
        verified,
        recompute_error,
        advisory: validity.map(advisory_notes).unwrap_or_default(),
        disclosure: LANE_DISCLOSURE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;
    use crate::validate::validate;

    fn table(headers: &[&str], rows: &[&[&str]]) -> (Vec<String>, Vec<Vec<String>>) {
        (
            headers.iter().map(|s| s.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect(),
        )
    }

    fn spec(kind: TestKind, roles: ColumnRoles, stat: Option<f64>, p: Option<f64>) -> AnalysisSpec {
        AnalysisSpec {
            test_kind: kind,
            roles,
            reported: ReportedStatistic { statistic: stat, p_value: p, coefficient_index: None },
            tolerance: Tolerance::default(),
        }
    }

    // ---- analysis-spec drives the right test on the right columns ----

    #[test]
    fn spec_recomputes_the_right_test_on_the_right_columns() {
        // wide: two independent sample columns → Student t-test (known case:
        // A=[1..5], B=[2,4,6,8,10] → t = -1.8974, df = 8).
        let (h, r) = table(
            &["A", "B", "note"],
            &[
                &["1", "2", "x"],
                &["2", "4", "x"],
                &["3", "6", "x"],
                &["4", "8", "x"],
                &["5", "10", "x"],
            ],
        );
        let s = spec(
            TestKind::TTestStudent,
            ColumnRoles::Samples { columns: vec!["A".into(), "B".into()] },
            None,
            None,
        );
        let rec = recompute(&s, &h, &r).unwrap();
        assert_eq!(rec.statistic_name, "t");
        assert!((rec.statistic - -1.8973665961).abs() < 1e-6, "t = {}", rec.statistic);
        assert!(rec.detail.contains("df = 8"));
    }

    #[test]
    fn grouped_long_format_splits_by_label() {
        // long format: value split by group → 2 groups A/B, same as above.
        let (h, r) = table(
            &["score", "arm"],
            &[
                &["1", "A"], &["2", "A"], &["3", "A"], &["4", "A"], &["5", "A"],
                &["2", "B"], &["4", "B"], &["6", "B"], &["8", "B"], &["10", "B"],
            ],
        );
        let s = spec(
            TestKind::TTestStudent,
            ColumnRoles::Grouped { value: "score".into(), group: "arm".into() },
            None,
            None,
        );
        let rec = recompute(&s, &h, &r).unwrap();
        assert!((rec.statistic - -1.8973665961).abs() < 1e-6, "t = {}", rec.statistic);
    }

    // ---- MATCH ----

    #[test]
    fn correctly_reported_value_is_a_match() {
        // Pearson on x/y perfectly correlated → r = 1.0; report r = 1.00, p = 0.
        let (h, r) = table(
            &["x", "y"],
            &[&["1", "2"], &["2", "4"], &["3", "6"], &["4", "8"], &["5", "10"]],
        );
        let s = spec(
            TestKind::Pearson,
            ColumnRoles::Paired { x: "x".into(), y: "y".into() },
            Some(1.00),
            Some(0.0),
        );
        let v = verify(&s, &h, &r).unwrap();
        assert_eq!(v.verdict, Verdict::Match);
        assert_eq!(v.tier, CertaintyTier::MathematicallyCertain);
        assert!((v.recomputed.statistic - 1.0).abs() < 1e-9); // ground truth always shown
    }

    #[test]
    fn rounded_two_decimal_report_still_matches_full_precision() {
        // ANOVA F = 27.0 exactly; a user who wrote F = 27.00 and p = 0.001
        // (rounded from 0.0010267) still MATCHES within tolerance.
        let (h, r) = table(
            &["y", "g"],
            &[
                &["1", "a"], &["2", "a"], &["3", "a"],
                &["4", "b"], &["5", "b"], &["6", "b"],
                &["7", "c"], &["8", "c"], &["9", "c"],
            ],
        );
        let s = spec(
            TestKind::Anova,
            ColumnRoles::Grouped { value: "y".into(), group: "g".into() },
            Some(27.00),
            Some(0.001),
        );
        let v = verify(&s, &h, &r).unwrap();
        assert_eq!(v.verdict, Verdict::Match, "{}", v.explanation);
    }

    // ---- MISMATCH (the honest catch) ----

    #[test]
    fn wrong_reported_statistic_is_caught_with_both_values() {
        // true t ≈ -1.897; the user claims t = 2.41 → MISMATCH, both shown.
        let (h, r) = table(
            &["A", "B"],
            &[&["1", "2"], &["2", "4"], &["3", "6"], &["4", "8"], &["5", "10"]],
        );
        let s = spec(
            TestKind::TTestStudent,
            ColumnRoles::Samples { columns: vec!["A".into(), "B".into()] },
            Some(2.41),
            None,
        );
        let v = verify(&s, &h, &r).unwrap();
        assert_eq!(v.verdict, Verdict::Mismatch);
        assert_eq!(v.reported.statistic, Some(2.41)); // reported echoed
        assert!((v.recomputed.statistic - -1.8973665961).abs() < 1e-6); // recomputed shown
        assert!(v.explanation.contains("2.41") && v.explanation.contains("MISMATCH"));
        assert!(v.statistic_delta.unwrap().abs() > 4.0);
    }

    #[test]
    fn wrong_reported_pvalue_is_caught() {
        // Pearson r=1 → p=0; user claims p = 0.40 → MISMATCH on p.
        let (h, r) = table(
            &["x", "y"],
            &[&["1", "2"], &["2", "4"], &["3", "6"], &["4", "8"], &["5", "10"]],
        );
        let s = spec(
            TestKind::Pearson,
            ColumnRoles::Paired { x: "x".into(), y: "y".into() },
            None,
            Some(0.40),
        );
        let v = verify(&s, &h, &r).unwrap();
        assert_eq!(v.verdict, Verdict::Mismatch);
        assert!(v.p_delta.unwrap().abs() > 0.3);
    }

    // ---- OLS coefficient selection + verdict ----

    #[test]
    fn ols_slope_verified() {
        // y=[2,4,5,4,5] on x=[1..5] → slope 0.6. Report β = 0.60 → MATCH.
        let (h, r) = table(
            &["y", "x"],
            &[&["2", "1"], &["4", "2"], &["5", "3"], &["4", "4"], &["5", "5"]],
        );
        let s = spec(
            TestKind::Ols,
            ColumnRoles::Regression { outcome: "y".into(), predictors: vec!["x".into()] },
            Some(0.60),
            None,
        );
        let v = verify(&s, &h, &r).unwrap();
        assert_eq!(v.verdict, Verdict::Match);
        assert!((v.recomputed.statistic - 0.6).abs() < 1e-9);
        assert!(v.recomputed.detail.contains("R²"));
    }

    // ---- chi-square contingency ----

    #[test]
    fn chi_square_from_contingency_columns() {
        // rows = category levels, columns = observed counts. [[10,20],[30,40]]
        // → χ² ≈ 0.7937, p ≈ 0.3729. Report χ² = 0.79 → MATCH.
        let (h, r) = table(&["left", "right"], &[&["10", "20"], &["30", "40"]]);
        let s = spec(
            TestKind::ChiSquare,
            ColumnRoles::Contingency { count_columns: vec!["left".into(), "right".into()] },
            Some(0.79),
            Some(0.37),
        );
        let v = verify(&s, &h, &r).unwrap();
        assert_eq!(v.verdict, Verdict::Match, "{}", v.explanation);
        assert!((v.recomputed.statistic - 0.79365).abs() < 1e-3);
    }

    // ---- honest recompute errors (no silent fallback) ----

    #[test]
    fn incompatible_roles_are_an_honest_error() {
        let (h, r) = table(&["x", "y"], &[&["1", "2"], &["3", "4"], &["5", "6"]]);
        // correlation asked for, but Samples roles given → error, no verdict.
        let s = spec(
            TestKind::Pearson,
            ColumnRoles::Samples { columns: vec!["x".into(), "y".into()] },
            None,
            None,
        );
        assert!(recompute(&s, &h, &r).is_err());
        let report = verify_analysis(&s, &h, &r, None);
        assert!(report.verified.is_none());
        assert!(report.recompute_error.is_some());
    }

    #[test]
    fn non_numeric_data_is_an_honest_error_never_silent_zero() {
        let (h, r) = table(&["A", "B"], &[&["1", "2"], &["oops", "4"], &["3", "6"]]);
        let s = spec(
            TestKind::TTestStudent,
            ColumnRoles::Samples { columns: vec!["A".into(), "B".into()] },
            None,
            None,
        );
        assert!(verify(&s, &h, &r).is_err());
    }

    #[test]
    fn missing_cells_drop_rows_and_are_counted() {
        // one incomplete pair → dropped, not misaligned.
        let (h, r) = table(
            &["x", "y"],
            &[&["1", "2"], &["2", ""], &["3", "6"], &["4", "8"], &["5", "10"]],
        );
        let s = spec(
            TestKind::Pearson,
            ColumnRoles::Paired { x: "x".into(), y: "y".into() },
            None,
            None,
        );
        let rec = recompute(&s, &h, &r).unwrap();
        assert_eq!(rec.rows_dropped, 1);
    }

    // ---- the typed verified-vs-advisory split (structural) ----

    #[test]
    fn advisory_notes_reuse_validate_rules_and_carry_the_disclaimer() {
        // a manuscript claim that fires validate.rs rules (missing effect size,
        // missing CI, overclaim).
        let doc = "A Study\n\nResults\nThis proves the drug works (p = 0.01).";
        let report = validate(&extract_from_text(doc));
        assert!(!report.passed, "expected validate.rs to flag this claim");
        let notes = advisory_notes(&report);
        assert_eq!(notes.len(), report.flags.len());
        for n in &notes {
            // every advisory note carries the required, non-empty disclaimer
            assert!(!n.disclaimer.is_empty());
            assert_eq!(n.disclaimer, ADVISORY_DISCLAIMER);
            // and reuses the validate.rs rule/severity verbatim
            assert!(RuleId::ALL.contains(&n.rule));
        }
    }

    #[test]
    fn advisory_note_cannot_be_constructed_as_verified() {
        // STRUCTURAL PROOF. An AdvisoryNote has no verdict and no certainty tier
        // — the MathematicallyCertain tier exists ONLY on VerifiedResult, and
        // there is no From/into/constructor bridging the two types. The only way
        // to obtain a VerifiedResult is verify()/recompute() over real data.
        let doc = "A Study\n\nResults\nThis proves it (p = 0.01).";
        let report = validate(&extract_from_text(doc));
        let notes = advisory_notes(&report);
        let note = &notes[0];

        // The note serializes with NO verified-lane keys: no "verdict", no
        // "tier", no "recomputed". (If someone added such a field or a From
        // impl, this and the type system would break — the lanes stay disjoint.)
        let json = serde_json::to_value(note).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("verdict"));
        assert!(!obj.contains_key("tier"));
        assert!(!obj.contains_key("recomputed"));
        // What it DOES carry: the advice disclaimer.
        assert_eq!(obj.get("disclaimer").unwrap(), ADVISORY_DISCLAIMER);

        // Conversely a VerifiedResult DOES carry the MathematicallyCertain tier.
        let (h, r) = table(&["x", "y"], &[&["1", "2"], &["2", "4"], &["3", "6"]]);
        let v = verify(
            &spec(TestKind::Pearson, ColumnRoles::Paired { x: "x".into(), y: "y".into() }, None, None),
            &h,
            &r,
        )
        .unwrap();
        assert_eq!(v.tier, CertaintyTier::MathematicallyCertain);
    }

    // ---- the un-strippable disclosure ----

    #[test]
    fn report_always_carries_the_lane_disclosure() {
        let (h, r) = table(&["x", "y"], &[&["1", "2"], &["2", "4"], &["3", "6"]]);
        let s = spec(TestKind::Pearson, ColumnRoles::Paired { x: "x".into(), y: "y".into() }, None, None);
        // with advisory
        let doc = "A Study\n\nResults\nThis proves it (p = 0.01).";
        let validity = validate(&extract_from_text(doc));
        let report = verify_analysis(&s, &h, &r, Some(&validity));
        assert!(!report.disclosure.is_empty());
        assert_eq!(report.disclosure, LANE_DISCLOSURE);
        assert!(report.disclosure.contains("NOT verifications"));
        assert!(!report.advisory.is_empty());
        assert!(report.verified.is_some());
        // and even with no advisory, the disclosure is still there
        let bare = verify_analysis(&s, &h, &r, None);
        assert!(!bare.disclosure.is_empty());
    }

    // ---- pre-fill from extraction ----

    #[test]
    fn reported_prefills_pvalue_from_claims_but_not_the_statistic() {
        let doc = "A Study\n\nResults\nThe effect held (p = 0.03, Cohen's d = 0.5, 95% CI: 0.1 to 0.9).";
        let claims = extract_from_text(doc).statistics;
        let reported = ReportedStatistic::from_claims(&claims);
        assert_eq!(reported.p_value, Some(0.03)); // p pre-filled
        assert_eq!(reported.statistic, None); // extraction can't capture t/F/r values
    }

    // ---- determinism ----

    #[test]
    fn same_spec_and_data_give_the_same_verdict() {
        let (h, r) = table(
            &["A", "B"],
            &[&["1", "2"], &["2", "5"], &["3", "6"], &["4", "8"], &["7", "9"]],
        );
        let s = spec(
            TestKind::TTestWelch,
            ColumnRoles::Samples { columns: vec!["A".into(), "B".into()] },
            Some(1.23),
            Some(0.30),
        );
        assert_eq!(verify(&s, &h, &r).unwrap(), verify(&s, &h, &r).unwrap());
    }
}
