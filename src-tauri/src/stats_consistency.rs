//! The Level 0 statistics-consistency path for one manuscript, WITHOUT Tauri.
//!
//! The same kind of seam as `commands::run_publishready_measured`: a plain
//! function an example binary can call, so an instrument measures the path a
//! command will take rather than a reassembly of its parts.
//!
//! In order: `import_guard::preflight`, the existing parse
//! (`extract::extract_path`), `stats_l0::result::read_results`, then every
//! method in the registry (`stats_l0::outcome::check_reading`). No network, no
//! model, and no database write: preflight only reads this machine's stored
//! import rate.

use gaply_core::extract;
use gaply_core::import_guard;
use gaply_core::equation::rational::Rational;
use gaply_core::stats_l0::method::{
    CheckNote, Computed, ConcernReason, CouldNotCheckReason, L0Verdict, NotApplicableReason, Provenance,
};
use gaply_core::stats_l0::number::{NumberNotRead, Relation};
use gaply_core::stats_l0::outcome::{check_reading, CheckOutcome, ClaimOutcome, StatsConsistencyOutcome};
use gaply_core::stats_l0::result::{read_results, PValue, Reason, TableStructure};
use gaply_core::stats_l0::{df_matches_n, estimate_in_interval, p_consistency};
use gaply_core::{Database, GaplyError};

/// Level 0 for the manuscript at `path`.
///
/// The preflight's REFUSALS are enforced here, as `ai_link_source_document`
/// enforces them: a refused file is `Err(Validation)` with the preflight's own
/// summary. Its confirmation is not: it asks about indexing time, and Level 0
/// indexes nothing, so a file preflight would only ask about runs.
///
/// A `.docx` is read with its declared tables; anything else as
/// `TableStructure::NotAvailable`, since a PDF or plain text has no table
/// objects.
#[doc(hidden)]
#[tracing::instrument(skip(db, path))]
pub fn run_stats_consistency_measured(
    db: &Database,
    path: &std::path::Path,
) -> Result<StatsConsistencyOutcome, GaplyError> {
    let pre = import_guard::preflight(db, path, gaply_core::now_epoch())?;
    if pre.is_refused() {
        return Err(GaplyError::Validation(pre.summary));
    }
    let ex = extract::extract_path(path)?;
    let is_docx = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("docx"));
    let tables = if is_docx { TableStructure::Declared } else { TableStructure::NotAvailable };
    let reading = read_results(&ex, tables);
    Ok(check_reading(&ex, reading))
}

/// The Tauri command: Level 0 for the manuscript at `path`, as the view the
/// Stats Check screen renders. Read-only on the database (preflight reads the
/// stored import rate), no network, no model.
///
/// A file preflight refuses is `Err(Validation)` carrying the refusal's own
/// message, which the screen shows as it is.
#[tauri::command]
#[tracing::instrument(skip(state, path))]
pub async fn run_stats_consistency(
    state: tauri::State<'_, crate::state::AppState>,
    path: String,
) -> Result<StatsConsistencyView, GaplyError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let outcome = run_stats_consistency_measured(&db, std::path::Path::new(&path))?;
        stats_consistency_view(&outcome)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("stats consistency task panicked: {e}")))?
}

/* ------------------------------------------------------------------------ *
 *  The view: what crosses to the webview, and nothing else.
 * ------------------------------------------------------------------------ */

// `StatsConsistencyOutcome` is deliberately not `Serialize`: it holds every
// printed unit. This view is the one serialisable form, built by one function
// (`stats_consistency_view`), and it lives in the APP crate so that no
// `gaply_core` code can name it.
// `the_view_is_named_only_by_the_files_that_send_it_to_the_webview` pins that no
// file that talks to the proxy names it. It carries manuscript text: each
// result's printed unit and its printed p.
//
// No ratio, percentage or score anywhere in it. Counts only.

/// Level 0 for one manuscript, as the Stats Check screen renders it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsConsistencyView {
    pub claims_read: usize,
    pub verdict_counts: L0CountsView,
    /// Every attempt not read, summed over `not_read`.
    pub not_read_count: usize,
    /// One row per reason, in the order each reason first occurs.
    pub not_read: Vec<L0NotReadView>,
    pub alpha: L0AlphaView,
    /// Every result read, in document order. The screen orders by verdict.
    pub results: Vec<L0ResultView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0CountsView {
    pub inconsistent: usize,
    pub concern: usize,
    pub could_not_check: usize,
    pub consistent: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0NotReadView {
    pub code: &'static str,
    pub text: String,
    pub count: usize,
}

/// The threshold `p_consistency` judged against.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0AlphaView {
    /// Each value as a decimal, APA style (`.05`).
    pub values: Vec<String>,
    /// No threshold is stated, so .05 was assumed.
    pub assumed: bool,
    /// Set when `assumed`, or when the paper states more than one.
    pub note: Option<String>,
}

/// One result read, with its verdict and the check the verdict rests on.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0ResultView {
    pub id: usize,
    pub location: gaply_core::extract::Location,
    /// The printed unit, exactly as printed.
    pub raw: String,
    /// The reported p's digits exactly as printed (`.0001`); `None` for `ns`.
    pub reported_p: Option<String>,
    /// `=`, `<`, `≤`, `>` or `≥`; `None` for `ns`.
    pub reported_p_relation: Option<&'static str>,
    /// What `p_consistency` recomputed from the statistic and df, before its
    /// engine margin. `None` when the inputs define no p-value.
    pub recomputed_p: Option<L0PRange>,
    pub verdict: L0VerdictView,
    /// The method whose answer the verdict rests on. Never absent: a result
    /// with no method run cannot reach the view (`stats_consistency_view`
    /// errors instead).
    pub check: L0MethodCheckView,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0PRange {
    pub lo: f64,
    pub hi: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0MethodCheckView {
    pub method_id: &'static str,
    pub method_name: &'static str,
    pub engine_version: &'static str,
    /// The publication the method follows, or that it is Gaply's own arithmetic.
    pub provenance: String,
}

/// A code the screen can key on, and the words a reader is shown.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0Words {
    pub code: &'static str,
    pub text: String,
}

/// One reason a result could not be checked, and the method it came from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L0ReasonView {
    pub method_name: &'static str,
    pub code: &'static str,
    pub text: String,
}

/// The closed verdict set. An `Inconsistent` never says which number is wrong.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum L0VerdictView {
    /// `conclusion_changes`: `None` when it was not assessed.
    Inconsistent { conclusion_changes: Option<bool> },
    Concern { reason: L0Words },
    /// Every reason, one per method, in registry order.
    CouldNotCheck { reasons: Vec<L0ReasonView> },
    /// `note`: set when the verdict rests on a reading the paper states, or on
    /// a display floor. Always shown with its text.
    Consistent { note: Option<L0Words> },
}

/// Build the view from the outcome. The one place it is built.
///
/// Errors only if a claim has no method that ran, which `p_consistency`
/// (applicable to every family the reader reads) rules out; pinned by
/// `every_claim_has_a_check_the_verdict_rests_on`.
pub fn stats_consistency_view(o: &StatsConsistencyOutcome) -> Result<StatsConsistencyView, GaplyError> {
    let not_read: Vec<L0NotReadView> = o
        .not_read_by_reason
        .iter()
        .map(|(reason, count)| {
            let (code, text) = not_read_words(*reason);
            L0NotReadView { code, text, count: *count }
        })
        .collect();
    let not_read_count = not_read.iter().map(|r| r.count).sum();

    debug_assert_eq!(o.alpha_used.len(), o.alpha_printed.len());
    let values: Vec<String> =
        o.alpha_used.iter().zip(&o.alpha_printed).map(|(value, printed)| alpha_words(*value, printed)).collect();
    let note = if o.alpha_defaulted {
        Some("The paper states no significance threshold, so .05 was assumed.".to_string())
    } else if values.len() > 1 {
        Some(format!(
            "The paper states more than one significance threshold ({}). Whether a conclusion changes was not assessed.",
            values.join(", ")
        ))
    } else {
        None
    };

    let results = o.claims.iter().map(result_view).collect::<Result<Vec<_>, _>>()?;
    let c = o.verdict_counts;
    Ok(StatsConsistencyView {
        claims_read: o.claims_read,
        verdict_counts: L0CountsView {
            inconsistent: c.inconsistent,
            concern: c.concern,
            could_not_check: c.could_not_check,
            consistent: c.consistent,
        },
        not_read_count,
        not_read,
        alpha: L0AlphaView { values, assumed: o.alpha_defaulted, note },
        results,
    })
}

fn result_view(claim: &ClaimOutcome) -> Result<L0ResultView, GaplyError> {
    let r = &claim.result;
    let (reported_p, reported_p_relation) = match &r.p {
        PValue::Reported(n) => (Some(n.raw().to_string()), Some(relation_symbol(n.relation()))),
        PValue::NotSignificant => (None, None),
    };
    let ran = || {
        claim.checks.iter().filter_map(|c| match c {
            CheckOutcome::Ran(m) => Some(m),
            CheckOutcome::NotApplicable { .. } => None,
        })
    };
    let recomputed_p = ran().find(|m| m.method_id == p_consistency::ID).and_then(|m| {
        m.computed.iter().find_map(|(name, v)| match (name, v) {
            (&"recomputed_p", Computed::Range { lo, hi }) => Some(L0PRange { lo: *lo, hi: *hi }),
            _ => None,
        })
    });

    // The check the verdict rests on: the first, in registry order, whose own
    // outcome is the claim's verdict; for CouldNotCheck, the first that ran.
    let decisive = ran()
        .find(|m| match (m.outcome, claim.verdict) {
            (L0Verdict::Inconsistent { .. }, L0Verdict::Inconsistent { .. }) => true,
            (L0Verdict::Concern(a), L0Verdict::Concern(b)) => a == b,
            (L0Verdict::Consistent(a), L0Verdict::Consistent(b)) => a == b,
            _ => false,
        })
        .or_else(|| ran().next())
        .ok_or_else(|| GaplyError::Internal(format!("Level 0 result {} reached the view with no method run", r.id)))?;

    let verdict = match claim.verdict {
        L0Verdict::Inconsistent { conclusion_changes } => L0VerdictView::Inconsistent { conclusion_changes },
        L0Verdict::Concern(reason) => {
            let (code, text) = concern_words(reason);
            L0VerdictView::Concern { reason: L0Words { code, text } }
        }
        L0Verdict::Consistent(note) => L0VerdictView::Consistent {
            note: note.map(|n| {
                let (code, text) = note_words(n, reported_p.as_deref());
                L0Words { code, text }
            }),
        },
        L0Verdict::CouldNotCheck(_) => {
            // The reasons that apply to this result: each method that RAN and
            // declined, in registry order. A method that did not apply is not
            // a reason the result could not be checked; those are listed only
            // when no method ran at all, so the card is never empty.
            let words = |c: &CheckOutcome, reason: CouldNotCheckReason| {
                let (code, text) = could_not_check_words(reason);
                L0ReasonView { method_name: method_name(c.method_id()), code, text }
            };
            let mut reasons: Vec<L0ReasonView> = claim
                .checks
                .iter()
                .filter_map(|c| match c {
                    CheckOutcome::Ran(m) => match m.outcome {
                        L0Verdict::CouldNotCheck(r) => Some(words(c, r)),
                        _ => None,
                    },
                    CheckOutcome::NotApplicable { .. } => None,
                })
                .collect();
            if reasons.is_empty() {
                reasons = claim
                    .checks
                    .iter()
                    .filter_map(|c| match c {
                        CheckOutcome::NotApplicable { reason, .. } => {
                            Some(words(c, CouldNotCheckReason::NotApplicable(*reason)))
                        }
                        CheckOutcome::Ran(_) => None,
                    })
                    .collect();
            }
            L0VerdictView::CouldNotCheck { reasons }
        }
    };

    Ok(L0ResultView {
        id: r.id,
        location: r.location.clone(),
        raw: r.raw.clone(),
        reported_p,
        reported_p_relation,
        recomputed_p,
        verdict,
        check: L0MethodCheckView {
            method_id: decisive.method_id,
            method_name: method_name(decisive.method_id),
            engine_version: decisive.engine_version,
            provenance: match decisive.provenance {
                Provenance::Published { reference } => reference.to_string(),
                Provenance::GaplyArithmetic => "Gaply's own arithmetic".to_string(),
            },
        },
    })
}

/// The alpha as the card shows it: `.05` for a plain value, and a fraction
/// as the paper wrote it followed by its decimal, `.05/6 (= .0083)`, so the
/// reader sees both the adjustment and the number the conclusion was judged
/// against. A non-terminating decimal is shown to four places, rounded.
fn alpha_words(value: Rational, printed: &str) -> String {
    if printed.contains('/') {
        let decimal = if value.terminates() { apa_decimal(value) } else { apa_decimal_to(value, 4) };
        format!("{printed} (= {decimal})")
    } else {
        apa_decimal(value)
    }
}

/// `.0083` for 1/120: `decimals` places, rounded, without the leading zero.
fn apa_decimal_to(r: Rational, decimals: u32) -> String {
    let s = r.to_decimal_string(decimals);
    s.strip_prefix("0.").map(|rest| format!(".{rest}")).unwrap_or(s)
}

/// `.05`, not `0.05`, as APA prints a value that cannot exceed 1.
fn apa_decimal(r: Rational) -> String {
    let s = r.to_display_string();
    s.strip_prefix("0.").map(|rest| format!(".{rest}")).unwrap_or(s)
}

fn relation_symbol(r: Relation) -> &'static str {
    match r {
        Relation::Eq => "=",
        Relation::Lt => "<",
        Relation::Le => "≤",
        Relation::Gt => ">",
        Relation::Ge => "≥",
    }
}

/* ------------------------------------------------------------------------ *
 *  The words. One exhaustive match per enum: a new variant does not compile
 *  until it has text.
 * ------------------------------------------------------------------------ */

/// A registered method's name. Not a `match` the compiler can close, since ids
/// are strings; `every_registered_method_has_a_name` closes it over the
/// registry instead.
fn method_name(id: &str) -> &'static str {
    match id {
        p_consistency::ID => "p-value recomputed from the test statistic and its degrees of freedom",
        estimate_in_interval::ID => "Effect size inside its confidence interval",
        df_matches_n::ID => "Degrees of freedom match the sample size",
        _ => "Unnamed method",
    }
}

fn note_words(n: CheckNote, reported_p: Option<&str>) -> (&'static str, String) {
    match n {
        CheckNote::OneTailedStated => {
            ("one_tailed_stated", "Consistent as a one-tailed test, which the paper states.".to_string())
        }
        CheckNote::FloorPrintedAsEquality => (
            "floor_printed_as_equality",
            format!(
                "Printed as p = {}; the exact p is smaller still. APA style is p < .001.",
                reported_p.unwrap_or("the printed value")
            ),
        ),
    }
}

fn concern_words(c: ConcernReason) -> (&'static str, String) {
    let (code, text) = match c {
        ConcernReason::OneTailedUnstated => (
            "one_tailed_unstated",
            "These numbers agree only if the test was one-tailed, and the paper does not say it was.",
        ),
        ConcernReason::FTwoTailed => (
            "f_two_tailed",
            "These numbers agree only if the p was taken from both tails of the F distribution. An F test in an ANOVA or regression uses the upper tail only.",
        ),
        ConcernReason::EstimateDirection => (
            "estimate_direction",
            "The effect size lies outside its confidence interval, but would lie inside it with its sign reversed (or, for a ratio, inverted). The direction may be printed inconsistently.",
        ),
    };
    (code, text.to_string())
}

fn could_not_check_words(r: CouldNotCheckReason) -> (&'static str, String) {
    let (code, text) = match r {
        CouldNotCheckReason::AdjustedP => (
            "adjusted_p",
            "The printed p is larger than the one recomputed, and the paper names a correction for multiple testing. It may be an adjusted p, which cannot be recomputed from this result alone.",
        ),
        CouldNotCheckReason::InvalidInput => {
            ("invalid_input", "The printed numbers do not define this computation, so it was not made.")
        }
        CouldNotCheckReason::Uninformative => (
            "uninformative",
            "Every value at the printed precision is possible with this sample size, so a match would say nothing.",
        ),
        CouldNotCheckReason::NotApplicable(n) => return not_applicable_words(n),
        CouldNotCheckReason::NoMethodRan => ("no_method_ran", "No check ran on this result."),
    };
    (code, text.to_string())
}

fn not_applicable_words(n: NotApplicableReason) -> (&'static str, String) {
    let (code, text) = match n {
        NotApplicableReason::OtherInputKind => ("other_input_kind", "This check does not apply to this kind of result."),
        NotApplicableReason::NoEstimateAndInterval => {
            ("no_estimate_and_interval", "The result prints no effect size with a confidence interval.")
        }
        NotApplicableReason::IntervalNotAfterEstimate => (
            "interval_not_after_estimate",
            "The confidence interval does not directly follow the effect size, so it may belong to an estimate that is not printed.",
        ),
        NotApplicableReason::DfNotFixedByN => (
            "df_not_fixed_by_n",
            "For this test the degrees of freedom are not fixed by the sample size alone.",
        ),
        NotApplicableReason::NoSampleSizeInUnit => {
            ("no_sample_size_in_unit", "No sample size is printed with the result.")
        }
        NotApplicableReason::PartialCorrelation => (
            "partial_correlation",
            "The sentence describes a partial correlation, whose degrees of freedom depend on how many variables were controlled for.",
        ),
        NotApplicableReason::GranularityUnknown => (
            "granularity_unknown",
            "Nobody has said the data are whole numbers, so this check does not apply.",
        ),
    };
    (code, text.to_string())
}

fn not_read_words(r: Reason) -> (&'static str, String) {
    let (code, text) = match r {
        Reason::StatisticWithoutP => {
            ("statistic_without_p", "A test statistic with no p-value after it in the same sentence.")
        }
        Reason::PWithoutStatistic => {
            ("p_without_statistic", "A p-value with no test statistic it could be read with.")
        }
        Reason::TwoResultsInOneClause => {
            ("two_results_in_one_clause", "Two values printed for one statistic in the same clause.")
        }
        Reason::NotLinked => (
            "not_linked",
            "A p-value follows in the sentence, but something printed between it and the statistic was not recognised.",
        ),
        Reason::MissingDegreesOfFreedom => {
            ("missing_degrees_of_freedom", "A t, F, r or chi-square with no degrees of freedom printed.")
        }
        Reason::MalformedDegreesOfFreedom => (
            "malformed_degrees_of_freedom",
            "Degrees of freedom printed in a form that does not fit this test.",
        ),
        Reason::RepeatedItem => ("repeated_item", "A second effect size or confidence interval in one result."),
        Reason::Number(n) => return number_words(n),
    };
    (code, text.to_string())
}

fn number_words(n: NumberNotRead) -> (&'static str, String) {
    let (code, why) = match n {
        NumberNotRead::Empty => ("number_empty", "it was empty"),
        NumberNotRead::Comma => (
            "number_comma",
            "it contains a comma, which could be a thousands separator or a decimal comma",
        ),
        NumberNotRead::Percent => ("number_percent", "it is a percentage"),
        NumberNotRead::ScientificNotation => ("number_scientific_notation", "it is in scientific notation"),
        NumberNotRead::Range => ("number_range", "it is a range"),
        NumberNotRead::InternalSpace => ("number_internal_space", "it contains a space"),
        NumberNotRead::Malformed => ("number_malformed", "its form was not recognised"),
        NumberNotRead::TooManyDigits => ("number_too_many_digits", "it has too many digits to hold exactly"),
        NumberNotRead::DecimalInCount => ("number_decimal_in_count", "a count is printed with a decimal point"),
        NumberNotRead::NegativeCount => ("number_negative_count", "a count is printed with a minus sign"),
        NumberNotRead::PValueOutOfRange => ("number_p_out_of_range", "a p-value is printed with a sign or above 1"),
    };
    (code, format!("A number in the result could not be read exactly: {why}."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::stats_l0::registry::MethodRegistry;

    /// A scratch file under the system temp dir, removed on drop.
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn scratch(name: &str, body: &[u8]) -> Scratch {
        let p = std::env::temp_dir().join(format!("gaply-l0-seam-{}-{name}", std::process::id()));
        std::fs::write(&p, body).unwrap();
        Scratch(p)
    }

    #[test]
    fn the_seam_reads_and_judges_a_manuscript_from_its_path() {
        let f = scratch(
            "ok.txt",
            b"A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045. Scores fell, t(28) = 2.10, p = .001.\n",
        );
        let db = Database::in_memory().unwrap();
        let o = run_stats_consistency_measured(&db, &f.0).unwrap();
        assert_eq!(o.claims_read, 2);
        assert_eq!((o.verdict_counts.consistent, o.verdict_counts.inconsistent), (1, 1));
        assert!(matches!(o.claims[1].verdict, L0Verdict::Inconsistent { .. }));
    }

    /// A zip archive holding one STORED entry, built by hand: the app crate
    /// has no zip dependency, and a .docx is a zip with `word/document.xml`.
    fn stored_zip(name: &str, data: &[u8]) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut c = 0xFFFF_FFFFu32;
            for &b in bytes {
                c ^= u32::from(b);
                for _ in 0..8 {
                    c = if c & 1 == 1 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
                }
            }
            !c
        }
        let (crc, len, n) = (crc32(data), data.len() as u32, name.len() as u16);
        let mut out = Vec::new();
        let w16 = |o: &mut Vec<u8>, v: u16| o.extend_from_slice(&v.to_le_bytes());
        let w32 = |o: &mut Vec<u8>, v: u32| o.extend_from_slice(&v.to_le_bytes());
        // Local file header.
        w32(&mut out, 0x0403_4B50);
        w16(&mut out, 20);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w32(&mut out, crc);
        w32(&mut out, len);
        w32(&mut out, len);
        w16(&mut out, n);
        w16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        let cd_offset = out.len() as u32;
        // Central directory.
        w32(&mut out, 0x0201_4B50);
        w16(&mut out, 20);
        w16(&mut out, 20);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w32(&mut out, crc);
        w32(&mut out, len);
        w32(&mut out, len);
        w16(&mut out, n);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w32(&mut out, 0);
        w32(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        let cd_size = out.len() as u32 - cd_offset;
        // End of central directory.
        w32(&mut out, 0x0605_4B50);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 1);
        w16(&mut out, 1);
        w32(&mut out, cd_size);
        w32(&mut out, cd_offset);
        w16(&mut out, 0);
        out
    }

    /// A `.docx` goes through the seam with its declared tables: a unit in
    /// prose reads as `Prose`, a unit inside a table cell as `TableCell`, and
    /// both are judged. The ledger of the Phase 1 evaluation noted that no
    /// test drove the `.docx` branch (`TableStructure::Declared`) end to end.
    #[test]
    fn a_docx_goes_through_the_seam_with_its_declared_tables() {
        let body = concat!(
            "<w:p><w:r><w:t>A Title</w:t></w:r></w:p>",
            "<w:p><w:r><w:t>Results</w:t></w:r></w:p>",
            "<w:p><w:r><w:t xml:space=\"preserve\">Scores rose, t(28) = 2.10, p = .045.</w:t></w:r></w:p>",
            "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Group</w:t></w:r></w:p></w:tc>",
            "<w:tc><w:p><w:r><w:t xml:space=\"preserve\">Scores fell, t(28) = 2.10, p = .001.</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
        );
        let doc = format!(
            "<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
        );
        let f = scratch("units.docx", &stored_zip("word/document.xml", doc.as_bytes()));
        let db = Database::in_memory().unwrap();
        let o = run_stats_consistency_measured(&db, &f.0).unwrap();
        assert_eq!(o.claims_read, 2, "{:?}", o.not_read);
        let sources: Vec<gaply_core::stats_l0::result::Source> = o.claims.iter().map(|c| c.result.source).collect();
        assert_eq!(
            sources,
            vec![gaply_core::stats_l0::result::Source::Prose, gaply_core::stats_l0::result::Source::TableCell],
            "the Declared branch labels prose and table cells"
        );
        assert!(matches!(o.claims[0].verdict, L0Verdict::Consistent(None)));
        assert!(matches!(o.claims[1].verdict, L0Verdict::Inconsistent { .. }));
    }

    #[test]
    fn a_refused_file_is_a_validation_error_and_nothing_is_read() {
        // Gaply's own audit-report markers make preflight refuse the file.
        let markers = gaply_core::ai_engine::audit_prepass::GAPLY_REPORT_MARKERS.join("\n");
        let f = scratch("report.txt", markers.as_bytes());
        let db = Database::in_memory().unwrap();
        let err = run_stats_consistency_measured(&db, &f.0).unwrap_err();
        assert!(matches!(err, GaplyError::Validation(ref m) if m.contains("Gaply audit report")), "{err:?}");
    }

    /// Preflight asks about a long file because indexing it takes time. Level 0
    /// indexes nothing, so it runs without asking.
    #[test]
    fn a_file_preflight_would_only_ask_about_runs_without_confirmation() {
        let mut body = String::from("A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045.\n\n");
        let filler = "Participants completed the questionnaire in a quiet room before the session began.\n\n";
        while body.len() < 400_000 {
            body.push_str(filler);
        }
        let f = scratch("long.txt", body.as_bytes());
        let db = Database::in_memory().unwrap();
        let pre = import_guard::preflight(&db, &f.0, gaply_core::now_epoch()).unwrap();
        assert!(pre.needs_confirmation(), "the fixture must reach the confirmation branch: {pre:?}");
        let o = run_stats_consistency_measured(&db, &f.0).unwrap();
        assert_eq!(o.claims_read, 1);
        assert_eq!(o.verdict_counts.consistent, 1);
    }

    /* -------------------------------- the view -------------------------------- */

    fn view_of(text: &str) -> StatsConsistencyView {
        let ex = gaply_core::extract::extract_from_text(text);
        let reading = read_results(&ex, TableStructure::NotAvailable);
        stats_consistency_view(&check_reading(&ex, reading)).unwrap()
    }

    fn by_raw<'a>(v: &'a StatsConsistencyView, raw: &str) -> &'a L0ResultView {
        v.results.iter().find(|r| r.raw.contains(raw)).unwrap_or_else(|| {
            panic!("no result containing {raw:?}: {:?}", v.results.iter().map(|r| &r.raw).collect::<Vec<_>>())
        })
    }

    const FOUR_VERDICTS: &str = "A Title\n\nResults\n\
        Scores rose, t(28) = 2.10, p = .045. \
        Scores fell, t(28) = 2.10, p = .001. \
        Recall improved, t(28) = 8.50, p = .0001. \
        Errors dropped, t(28) = 2.10, p = .022. \
        Speed changed, r(28) = 1.50, p = .020. \
        A third, t = 3.1, p = .004.\n";

    #[test]
    fn each_verdict_reaches_the_view_with_its_words_through_the_real_reader() {
        let v = view_of(FOUR_VERDICTS);
        assert_eq!(v.claims_read, v.results.len());
        let c = v.verdict_counts;
        assert_eq!(c.inconsistent + c.concern + c.could_not_check + c.consistent, v.claims_read);

        let ok = by_raw(&v, "p = .045");
        assert_eq!(ok.verdict, L0VerdictView::Consistent { note: None });
        assert_eq!(ok.check.method_id, p_consistency::ID);
        assert_eq!(ok.reported_p.as_deref(), Some(".045"));
        assert_eq!(ok.reported_p_relation, Some("="));
        let range = ok.recomputed_p.expect("p_consistency records its range");
        assert!(range.lo <= 0.045 && 0.045 <= range.hi * 1.01, "{range:?}");

        let bad = by_raw(&v, "p = .001");
        // One threshold read (the default), so whether the conclusion changes
        // was assessed: .001 and the recomputed ~.045 are both below .05.
        assert_eq!(bad.verdict, L0VerdictView::Inconsistent { conclusion_changes: Some(false) });

        let floor = by_raw(&v, "p = .0001");
        assert_eq!(
            floor.verdict,
            L0VerdictView::Consistent {
                note: Some(L0Words {
                    code: "floor_printed_as_equality",
                    text: "Printed as p = .0001; the exact p is smaller still. APA style is p < .001.".into(),
                })
            }
        );

        let concern = by_raw(&v, "p = .022");
        assert!(
            matches!(&concern.verdict, L0VerdictView::Concern { reason } if reason.code == "one_tailed_unstated"),
            "{:?}",
            concern.verdict
        );
        assert_eq!(concern.check.method_id, p_consistency::ID);

        let cannot = by_raw(&v, "r(28) = 1.50");
        let L0VerdictView::CouldNotCheck { reasons } = &cannot.verdict else { panic!("{:?}", cannot.verdict) };
        // Only the method that ran and declined is listed; the two methods
        // that did not apply to this result are not reasons it could not be
        // checked (evaluation finding 10).
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert_eq!(reasons[0].code, "invalid_input");
        assert_eq!(reasons[0].method_name, method_name(p_consistency::ID));
        assert!(MethodRegistry::LEVEL_0.len() > 1, "the registry must hold methods that did not apply for this to mean anything");

        // The lone `t = 3.1` has no df: not read, counted, with its reason.
        assert_eq!(v.not_read_count, 1);
        assert_eq!(v.not_read[0].code, "missing_degrees_of_freedom");
        assert_eq!(v.not_read[0].count, 1);

        assert!(v.alpha.assumed);
        assert_eq!(v.alpha.values, vec![".05".to_string()]);
        assert_eq!(v.alpha.note.as_deref(), Some("The paper states no significance threshold, so .05 was assumed."));
    }

    #[test]
    fn a_stated_one_tailed_test_reads_as_the_step_states() {
        let v = view_of(
            "A Title\n\nMethods\nAll tests were one-tailed.\n\nResults\nErrors dropped, t(28) = 2.10, p = .022.\n",
        );
        assert_eq!(
            v.results[0].verdict,
            L0VerdictView::Consistent {
                note: Some(L0Words {
                    code: "one_tailed_stated",
                    text: "Consistent as a one-tailed test, which the paper states.".into()
                })
            }
        );
    }

    #[test]
    fn several_stated_thresholds_say_the_conclusion_was_not_assessed() {
        let v = view_of(
            "A Title\n\nMethods\nSignificance was set at p < .05, and at p < .01 for the secondary outcomes.\n\n\
             Results\nScores fell, t(28) = 2.10, p = .001.\n",
        );
        assert!(!v.alpha.assumed);
        assert_eq!(v.alpha.values.len(), 2, "{:?}", v.alpha);
        assert!(v.alpha.note.as_deref().is_some_and(|n| n.contains("more than one") && n.contains("not assessed")));
        assert_eq!(v.results[0].verdict, L0VerdictView::Inconsistent { conclusion_changes: None });

        let one = view_of(
            "A Title\n\nMethods\nSignificance was set at p < .01.\n\nResults\nScores rose, t(28) = 2.10, p = .045.\n",
        );
        assert_eq!((one.alpha.values.clone(), one.alpha.assumed, one.alpha.note.clone()), (vec![".01".to_string()], false, None));
    }

    /// A fractional threshold is shown as the paper wrote it, followed by its
    /// decimal, so a reader sees both the adjustment and the number the
    /// conclusion was judged against (evaluation finding 5).
    #[test]
    fn a_fractional_threshold_shows_as_the_paper_wrote_it_with_its_decimal() {
        let v = view_of(
            "A Title\n\nMethods\nSignificance was set at p < .05/6.\n\n\
             Results\nScores rose, t(28) = 2.10, p = .045.\n",
        );
        assert!(!v.alpha.assumed);
        assert_eq!(v.alpha.values, vec![".05/6 (= .0083)".to_string()], "{:?}", v.alpha);
        assert_eq!(v.alpha.note, None);
        // The wording changes nothing the methods judge: this p is consistent
        // as printed (R: .04439 to .04534), under any alpha.
        assert_eq!(v.results[0].verdict, L0VerdictView::Consistent { note: None });
    }

    /// The card's props type requires a check; this is the Rust half: a claim
    /// with no method that ran cannot become a card. `p_consistency` applies to
    /// every family the reader reads, so the error is unreachable today, and
    /// the first assertion pins that premise.
    #[test]
    fn every_claim_has_a_check_the_verdict_rests_on() {
        let ex = gaply_core::extract::extract_from_text(FOUR_VERDICTS);
        let o = check_reading(&ex, read_results(&ex, TableStructure::NotAvailable));
        assert!(o.claims_read >= 5);
        for c in &o.claims {
            assert!(
                c.checks.iter().any(|k| matches!(k, CheckOutcome::Ran(m) if m.method_id == p_consistency::ID)),
                "p_consistency did not run on {:?}",
                c.result.raw
            );
        }
        let mut broken = o.clone();
        broken.claims[0].checks = vec![CheckOutcome::NotApplicable {
            method_id: p_consistency::ID,
            reason: NotApplicableReason::OtherInputKind,
        }];
        assert!(matches!(stats_consistency_view(&broken), Err(GaplyError::Internal(_))));
    }

    #[test]
    fn every_registered_method_has_a_name() {
        let ids: Vec<&str> = MethodRegistry::LEVEL_0.iter().map(|m| m.method.id()).collect();
        assert_eq!(ids.len(), 3, "the registry scan found {ids:?}");
        for id in ids {
            assert_ne!(method_name(id), method_name("not a method"), "{id} has no name");
        }
    }

    /// Every sentence a reader can be shown, rendered and read back: a full
    /// stop at the end, no doubled space, no em dash, and a distinct code.
    #[test]
    fn every_word_a_reader_can_be_shown_is_clean_prose() {
        let na = [
            NotApplicableReason::OtherInputKind,
            NotApplicableReason::NoEstimateAndInterval,
            NotApplicableReason::IntervalNotAfterEstimate,
            NotApplicableReason::DfNotFixedByN,
            NotApplicableReason::NoSampleSizeInUnit,
            NotApplicableReason::PartialCorrelation,
            NotApplicableReason::GranularityUnknown,
        ];
        let numbers = [
            NumberNotRead::Empty,
            NumberNotRead::Comma,
            NumberNotRead::Percent,
            NumberNotRead::ScientificNotation,
            NumberNotRead::Range,
            NumberNotRead::InternalSpace,
            NumberNotRead::Malformed,
            NumberNotRead::TooManyDigits,
            NumberNotRead::DecimalInCount,
            NumberNotRead::NegativeCount,
            NumberNotRead::PValueOutOfRange,
        ];
        let mut all: Vec<(&str, String)> = vec![
            note_words(CheckNote::OneTailedStated, None),
            note_words(CheckNote::FloorPrintedAsEquality, Some(".0001")),
            concern_words(ConcernReason::OneTailedUnstated),
            concern_words(ConcernReason::FTwoTailed),
            concern_words(ConcernReason::EstimateDirection),
            could_not_check_words(CouldNotCheckReason::AdjustedP),
            could_not_check_words(CouldNotCheckReason::InvalidInput),
            could_not_check_words(CouldNotCheckReason::Uninformative),
            could_not_check_words(CouldNotCheckReason::NoMethodRan),
        ];
        all.extend(na.iter().map(|n| could_not_check_words(CouldNotCheckReason::NotApplicable(*n))));
        all.extend(
            [
                Reason::StatisticWithoutP,
                Reason::PWithoutStatistic,
                Reason::TwoResultsInOneClause,
                Reason::NotLinked,
                Reason::MissingDegreesOfFreedom,
                Reason::MalformedDegreesOfFreedom,
                Reason::RepeatedItem,
            ]
            .into_iter()
            .chain(numbers.into_iter().map(Reason::Number))
            .map(not_read_words),
        );
        let mut codes = std::collections::BTreeSet::new();
        for (code, text) in &all {
            assert!(text.ends_with('.'), "{code}: {text:?}");
            assert!(!text.contains("  ") && !text.contains('\u{2014}'), "{code}: {text:?}");
            assert!(codes.insert(*code), "duplicate code {code}");
        }
        assert_eq!(all.len(), 9 + 7 + 7 + 11);
        // Inconsistent's words live in the screen and must never blame the p.
        for (_, text) in &all {
            assert!(!text.to_lowercase().contains("p-value is wrong"), "{text}");
        }
    }

    /// Counts only: no key in the serialised view names a ratio, percentage,
    /// score or grade.
    #[test]
    fn the_view_carries_counts_and_no_score() {
        fn keys(v: &serde_json::Value, out: &mut Vec<String>) {
            match v {
                serde_json::Value::Object(m) => {
                    for (k, x) in m {
                        out.push(k.to_lowercase());
                        keys(x, out);
                    }
                }
                serde_json::Value::Array(a) => a.iter().for_each(|x| keys(x, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        keys(&serde_json::to_value(view_of(FOUR_VERDICTS)).unwrap(), &mut out);
        assert!(out.len() > 30, "the walk found {} keys", out.len());
        for k in &out {
            for banned in ["percent", "ratio", "score", "grade", "rate", "coverage"] {
                assert!(!k.contains(banned), "key {k:?}");
            }
        }
    }

    /// **No proxy payload can contain the view.** A SOURCE scan, over both
    /// crates: it constrains what code NAMES, not what bytes a payload holds at
    /// run time.
    ///
    /// The view carries manuscript text, so it must reach the webview and
    /// nothing else. Every proxy call goes through `ProxyClient::verify(&Value)`,
    /// an untyped payload, so there is no payload TYPE to check against; the
    /// guard is instead that no file which names `ProxyClient`, or the proxy's
    /// HTTP client, also names the view or this module, and that only the
    /// files listed below name either. `gaply_core` cannot name an app-crate
    /// type at all; it is scanned anyway so the guard does not rest on that.
    ///
    /// Blind spot, stated so a green run is not over-read: code that receives
    /// the view through a generic `Serialize` bound without naming it is not
    /// seen. No such code names this module today, which the allowlist pins.
    #[test]
    fn the_view_is_named_only_by_the_files_that_send_it_to_the_webview() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        walk(&root.join("src"), &mut files);
        walk(&root.join("gaply-core").join("src"), &mut files);

        // Paths relative to the manifest, '/'-separated so the allowlist
        // matches on Windows too (windows-build-check is the only instrument
        // that reaches that).
        let rel = |p: &std::path::Path| {
            p.strip_prefix(root).unwrap().components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
        };
        const ALLOWED: &[&str] = &["src/stats_consistency.rs", "src/lib.rs", "src/wire_contract_tests.rs"];
        const VIEW: &[&str] = &["StatsConsistencyView", "stats_consistency_view", "stats_consistency::"];
        const PROXY: &[&str] = &["ProxyClient", "ProxyReqwestClient", "proxy_client"];

        let mut naming_view = Vec::new();
        let mut naming_proxy = Vec::new();
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            let name = rel(f);
            if VIEW.iter().any(|n| text.contains(n)) {
                naming_view.push(name.clone());
            }
            if PROXY.iter().any(|n| text.contains(n)) {
                naming_proxy.push(name);
            }
        }
        // Positive counts: a scan whose input went empty would pass forever.
        assert!(files.len() > 100, "only {} source files found", files.len());
        assert!(naming_proxy.len() >= 5, "the proxy scan found only {naming_proxy:?}");
        assert!(naming_view.contains(&"src/stats_consistency.rs".to_string()), "{naming_view:?}");

        let outside: Vec<&String> = naming_view.iter().filter(|n| !ALLOWED.contains(&n.as_str())).collect();
        assert!(outside.is_empty(), "the Level 0 view is named outside the webview path: {outside:?}");
        // This file names the proxy identifiers in the constant above; it is
        // the one file allowed to, and it calls nothing through them.
        let both: Vec<&String> = naming_view
            .iter()
            .filter(|n| naming_proxy.contains(n) && n.as_str() != "src/stats_consistency.rs")
            .collect();
        assert!(both.is_empty(), "a file that talks to the proxy names the Level 0 view: {both:?}");
        let here = std::fs::read_to_string(root.join("src/stats_consistency.rs")).unwrap();
        let code = here.split("#[cfg(test)]").next().unwrap();
        assert!(!PROXY.iter().any(|n| code.contains(n)), "the command's own code names the proxy");
    }

    #[test]
    fn a_missing_file_is_an_error_not_an_empty_outcome() {
        let db = Database::in_memory().unwrap();
        let missing = std::env::temp_dir().join("gaply-l0-seam-does-not-exist.txt");
        assert!(run_stats_consistency_measured(&db, &missing).is_err());
    }
}
