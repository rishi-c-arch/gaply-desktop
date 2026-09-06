//! Import size guidance — a SOFT gate on how much work an import is about to be.
//!
//! # Why this is guidance and not a cap
//!
//! Indexing a long document is slow, not wrong. The thing a user cannot see
//! before they commit is the *time*: a 900-page thesis is minutes of embedding
//! on a laptop CPU, and a progress bar that starts after the decision is made
//! is information arriving too late to act on. So this module answers one
//! question — "how long is this going to take on THIS machine?" — and then gets
//! out of the way.
//!
//! Three tiers, and they are deliberately not the same kind of thing:
//!
//! | tier | threshold | what it does |
//! |---|---|---|
//! | inform | any | states pages and an estimate |
//! | confirm | > [`CONFIRM_ABOVE_PAGES`] | asks first; the user may always say yes |
//! | refuse | > [`MAX_PAGES`] or > [`MAX_BYTES`] | declines, and says why |
//!
//! The ceiling exists because beyond it the operation stops being "slow" and
//! starts being "the app appears to have hung for an hour", which no estimate
//! makes acceptable. It is NOT an accuracy judgement — see the module note
//! below, and plan §11 D57.
//!
//! # There is no accuracy-motivated page cap, and there must not be one
//!
//! A tempting-but-wrong rule is "documents over N pages give worse answers, so
//! cap them". That would be false here, and acting on it would remove a
//! capability for no gain. What reaches the model is never the document: it is
//! the top-ranked chunks that retrieval selected for one claim, assembled
//! against a fixed token budget (`EVIDENCE_BUDGET_TOKENS`). A 40-page paper and
//! a 900-page thesis both arrive as the same handful of passages, because the
//! budget — not the source — is what bounds the model's input.
//!
//! Length changes *retrieval difficulty*, which is a ranking problem measured
//! by whether the right chunk is in the top-k, and is not improved by refusing
//! the document. It is improved by better ranking. So the only honest reason to
//! decline a long document is TIME, and time is exactly what these thresholds
//! are denominated in.
//!
//! # The estimate is measured, not asserted
//!
//! [`SEED_SECONDS_PER_PAGE`] is a starting guess for a release build. Every
//! completed import replaces it with what this machine actually did
//! ([`record_rate`]), so the second estimate a user sees is about their own
//! hardware. The rate lives in the TTL cache on purpose: losing it is harmless
//! — the seed is the fallback, and a stale rate from a machine whose conditions
//! have changed is worth forgetting.

use serde::Serialize;

use crate::db::Database;
use crate::extract::docparse::DocumentShape;
use crate::GaplyError;

/// Above this many pages, ask before starting.
pub const CONFIRM_ABOVE_PAGES: u32 = 150;
/// Above this, decline. Not an accuracy limit — see the module docs.
pub const MAX_PAGES: u32 = 1_500;
/// Above this, decline without even parsing.
pub const MAX_BYTES: u64 = 100 * 1024 * 1024;
/// The release-build starting guess, replaced by measurement.
pub const SEED_SECONDS_PER_PAGE: f64 = 0.5;

const RATE_KEY: &str = "import:seconds_per_page";
/// 90 days. Long enough to be useful across sessions, short enough that a rate
/// measured on a machine in a very different state eventually lapses.
const RATE_TTL_SECS: i64 = 90 * 24 * 60 * 60;

/// Where the estimate's number came from. Shown, because "about 4 minutes"
/// based on one prior import and based on a compiled-in guess are different
/// claims and a reader is entitled to tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EstimateBasis {
    /// No import has completed yet; [`SEED_SECONDS_PER_PAGE`] is in use.
    Seeded,
    /// Averaged over this many completed imports on this machine.
    Measured { samples: u32 },
}

/// What the import may do.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "verdict", rename_all = "camelCase")]
pub enum PreflightVerdict {
    /// Proceed. The estimate is still shown; small is not the same as silent.
    Ok,
    /// Ask first. The user may always say yes — this is a speed bump, not a
    /// gate that can be failed.
    ConfirmationRequired { reason: String },
    /// Decline, with the reason and the limit that was passed.
    Refused { reason: String },
}

/// The whole answer: what the file is, what it will cost, and what may happen.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreflight {
    pub pages: Option<u32>,
    pub page_equivalents: u32,
    pub bytes: u64,
    pub estimated_seconds: f64,
    pub estimate_basis: EstimateBasis,
    #[serde(flatten)]
    pub verdict: PreflightVerdict,
    /// The sentence to show. Built here so the wording is one thing, not one
    /// per surface.
    pub summary: String,
}

impl ImportPreflight {
    pub fn needs_confirmation(&self) -> bool {
        matches!(self.verdict, PreflightVerdict::ConfirmationRequired { .. })
    }
    pub fn is_refused(&self) -> bool {
        matches!(self.verdict, PreflightVerdict::Refused { .. })
    }
}

/// "about 20 seconds" / "about 4 minutes". Rounded honestly — an estimate
/// printed to the second claims a precision it does not have.
pub fn human_duration(seconds: f64) -> String {
    if seconds < 1.5 {
        return "about a second".to_string();
    }
    if seconds < 90.0 {
        return format!("about {} seconds", seconds.round() as i64);
    }
    let minutes = seconds / 60.0;
    if minutes < 90.0 {
        return format!("about {} minutes", minutes.round() as i64);
    }
    format!("about {:.1} hours", minutes / 60.0)
}

fn human_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if (bytes as f64) < MB {
        format!("{} KB", (bytes as f64 / 1024.0).round() as i64)
    } else {
        format!("{:.0} MB", bytes as f64 / MB)
    }
}

/// The decision, as a PURE function of size and rate.
///
/// Split from the file IO so every threshold is testable without a PDF on disk,
/// and so the thresholds are readable in one place rather than distributed
/// through a parsing routine.
pub fn assess(shape: &DocumentShape, seconds_per_page: f64, basis: EstimateBasis) -> ImportPreflight {
    let estimated_seconds = shape.page_equivalents as f64 * seconds_per_page;
    // How the document is described. A PDF has pages; a .docx does not, and
    // saying "12 pages" about one would be inventing a fact about the file.
    let noun = match shape.pages {
        Some(1) => "1 page".to_string(),
        Some(n) => format!("{n} pages"),
        None => format!("about {} pages' worth of text", shape.page_equivalents),
    };

    let verdict = if shape.bytes > MAX_BYTES {
        PreflightVerdict::Refused {
            reason: format!(
                "This file is {} — above the {} limit for a single import. \
                 Split it into chapters and import those, or link the part you actually cite.",
                human_size(shape.bytes),
                human_size(MAX_BYTES)
            ),
        }
    } else if shape.pages.is_some_and(|p| p > MAX_PAGES) {
        PreflightVerdict::Refused {
            reason: format!(
                "{noun} — above the {MAX_PAGES}-page limit for a single import. \
                 This is a limit on TIME, not on accuracy: indexing it would run for \
                 {}, and the app would look hung for most of it. Split it into \
                 chapters and import those.",
                human_duration(estimated_seconds)
            ),
        }
    } else if shape.page_equivalents > CONFIRM_ABOVE_PAGES {
        PreflightVerdict::ConfirmationRequired {
            reason: format!(
                "{noun} — indexing will take {} on this machine, and it runs to \
                 completion once started. Start it?",
                human_duration(estimated_seconds)
            ),
        }
    } else {
        PreflightVerdict::Ok
    };

    let summary = match &verdict {
        PreflightVerdict::Refused { reason } => reason.clone(),
        PreflightVerdict::ConfirmationRequired { reason } => reason.clone(),
        PreflightVerdict::Ok => {
            format!("{noun} — {} to index on this machine.", human_duration(estimated_seconds))
        }
    };

    ImportPreflight {
        pages: shape.pages,
        page_equivalents: shape.page_equivalents,
        bytes: shape.bytes,
        estimated_seconds,
        estimate_basis: basis,
        verdict,
        summary,
    }
}

/// This machine's measured seconds-per-page, or the seed if none is recorded.
pub fn current_rate(db: &Database, now: i64) -> (f64, EstimateBasis) {
    let stored = db.cache_get(RATE_KEY, now).ok().flatten();
    let parsed = stored
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| {
            let rate = v["secondsPerPage"].as_f64()?;
            let samples = v["samples"].as_u64()? as u32;
            // A non-finite or non-positive rate would produce a nonsense
            // estimate; treat it as no measurement rather than propagate it.
            (rate.is_finite() && rate > 0.0 && samples > 0).then_some((rate, samples))
        });
    match parsed {
        Some((rate, samples)) => (rate, EstimateBasis::Measured { samples }),
        None => (SEED_SECONDS_PER_PAGE, EstimateBasis::Seeded),
    }
}

/// Read a file and decide, in the cheap-first order that matters:
/// size (a `stat`) before parsing, and the scanned check before any indexing.
pub fn preflight(db: &Database, path: &std::path::Path, now: i64) -> Result<ImportPreflight, GaplyError> {
    let (rate, basis) = current_rate(db, now);

    // The byte ceiling FIRST, from metadata alone. Parsing a 400 MB PDF to
    // discover it is too big spends exactly the time this is meant to save.
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if bytes > MAX_BYTES {
        let shape = DocumentShape { pages: None, page_equivalents: 0, bytes, text_chars: 0 };
        return Ok(assess(&shape, rate, basis));
    }

    // `inspect_path` refuses a scanned PDF with the OCR advice, and it does so
    // before a `documents` row exists or a single chunk is embedded.
    //
    // That refusal arrives as a `Validation` error, and it is turned into a
    // VERDICT here rather than propagated. "This file cannot be imported" is
    // the exact question this function exists to answer, so every one of its
    // answers should be a verdict — a caller that had to handle two refusals in
    // two shapes would eventually handle only one of them. Non-validation
    // errors (a corrupt file, an IO fault) stay errors, because those are
    // faults rather than answers.
    match crate::extract::docparse::inspect_path_with_text(path) {
        Ok((shape, text)) => {
            // §11 D87. REFUSE ONE OF OUR OWN AUDIT REPORTS.
            //
            // D80 stops a report being AUDITED. Indexing one is worse: it
            // becomes retrievable evidence, and `citation_support` can then
            // quote Gaply's own prose back as the source a claim was checked
            // against — a card indistinguishable from a genuine one, which is
            // precisely what D18 exists to prevent.
            //
            // Refused rather than indexed-and-excluded-from-retrieval: an
            // exclusion is a rule sitting far from its reason, and whoever
            // touches retrieval next can relax it without learning why. A
            // refusal states its reason at the point of failure.
            //
            // It lives HERE, in the one gate `ai_index_document`,
            // `ai_link_source_document` and the OA fetch all already call, so
            // there is no per-site version to miss (§11 D72) and a fourth
            // import path inherits it for free.
            let markers = crate::ai_engine::audit_prepass::gaply_report_markers_in(&text);
            if markers.len() >= crate::ai_engine::audit_prepass::GAPLY_REPORT_MIN_MARKERS {
                let reason = format!(
                    "this looks like a Gaply audit report, not a source document — it contains \
                     {}. Indexing it would let Gaply quote its own report back to you as \
                     evidence for a claim. Import the paper it was written about instead.",
                    markers.iter().map(|m| format!("“{m}”")).collect::<Vec<_>>().join(", ")
                );
                return Ok(ImportPreflight {
                    pages: shape.pages,
                    page_equivalents: shape.page_equivalents,
                    bytes,
                    estimated_seconds: 0.0,
                    estimate_basis: basis,
                    verdict: PreflightVerdict::Refused { reason: reason.clone() },
                    summary: reason,
                });
            }
            Ok(assess(&shape, rate, basis))
        }
        Err(GaplyError::Validation(reason)) => Ok(ImportPreflight {
            pages: None,
            page_equivalents: 0,
            bytes,
            estimated_seconds: 0.0,
            estimate_basis: basis,
            verdict: PreflightVerdict::Refused { reason: reason.clone() },
            summary: reason,
        }),
        Err(other) => Err(other),
    }
}

/// Fold one completed import into this machine's rate.
///
/// A running mean over the recorded sample count, capped so an old average
/// cannot become unmovable: after [`MAX_SAMPLES`] the estimate keeps tracking
/// recent behaviour rather than converging on a machine's historical average,
/// which is the wrong thing when conditions (thermal, memory pressure, a
/// different model) have changed.
pub fn record_rate(
    db: &Database,
    page_equivalents: u32,
    elapsed_seconds: f64,
    now: i64,
) -> Result<(), GaplyError> {
    const MAX_SAMPLES: u32 = 20;
    if page_equivalents == 0 || !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
        return Ok(());
    }
    let observed = elapsed_seconds / page_equivalents as f64;
    let (prev_rate, basis) = current_rate(db, now);
    let (rate, samples) = match basis {
        // The seed is a guess, not an observation: the first real measurement
        // replaces it outright rather than being averaged with it.
        EstimateBasis::Seeded => (observed, 1),
        EstimateBasis::Measured { samples } => {
            let n = samples.min(MAX_SAMPLES) as f64;
            ((prev_rate * n + observed) / (n + 1.0), samples.saturating_add(1))
        }
    };
    let value = serde_json::json!({ "secondsPerPage": rate, "samples": samples }).to_string();
    db.cache_put(RATE_KEY, &value, RATE_TTL_SECS, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(pages: Option<u32>, page_equivalents: u32, bytes: u64) -> DocumentShape {
        DocumentShape { pages, page_equivalents, bytes, text_chars: page_equivalents as usize * 2000 }
    }

    fn seeded(s: &DocumentShape) -> ImportPreflight {
        assess(s, SEED_SECONDS_PER_PAGE, EstimateBasis::Seeded)
    }

    #[test]
    fn a_small_document_proceeds_and_still_states_the_cost() {
        // Small is not the same as silent: the estimate is the whole point, and
        // a user who is never told the cheap case cannot calibrate the dear one.
        let p = seeded(&shape(Some(38), 38, 2 * 1024 * 1024));
        assert_eq!(p.verdict, PreflightVerdict::Ok);
        assert!(!p.needs_confirmation() && !p.is_refused());
        assert_eq!(p.estimated_seconds, 19.0);
        assert!(p.summary.starts_with("38 pages — about 19 seconds"), "{}", p.summary);
    }

    #[test]
    fn exactly_at_the_confirm_threshold_still_proceeds() {
        // 150 is allowed; the rule is ABOVE 150. An off-by-one here is a
        // dialog a user did not need to see.
        let p = seeded(&shape(Some(CONFIRM_ABOVE_PAGES), CONFIRM_ABOVE_PAGES, 1024));
        assert_eq!(p.verdict, PreflightVerdict::Ok, "{}", p.summary);
    }

    #[test]
    fn above_the_confirm_threshold_asks_and_can_be_answered_yes() {
        let p = seeded(&shape(Some(151), 151, 1024));
        assert!(p.needs_confirmation(), "{:?}", p.verdict);
        // It must read as a question, not a refusal — the user may always proceed.
        assert!(p.summary.contains("Start it?"), "{}", p.summary);
        assert!(p.summary.contains("151 pages"), "{}", p.summary);
        assert!(!p.is_refused());
    }

    #[test]
    fn exactly_at_the_page_ceiling_is_allowed_and_above_it_is_refused() {
        let at = seeded(&shape(Some(MAX_PAGES), MAX_PAGES, 1024));
        assert!(at.needs_confirmation(), "the ceiling itself must be reachable: {:?}", at.verdict);

        let over = seeded(&shape(Some(MAX_PAGES + 1), MAX_PAGES + 1, 1024));
        assert!(over.is_refused(), "{:?}", over.verdict);
        // The refusal says WHY, and says it is about time rather than accuracy —
        // otherwise a user reasonably concludes their thesis is "too big to
        // check properly", which is not true and not what this limit means.
        assert!(over.summary.contains("limit on TIME, not on accuracy"), "{}", over.summary);
        assert!(over.summary.contains("Split it"), "{}", over.summary);
    }

    #[test]
    fn the_byte_ceiling_refuses_independently_of_page_count() {
        // A 200 MB, 12-page scan-with-a-text-layer is under every page rule and
        // still not something to pull through the parser.
        let p = seeded(&shape(Some(12), 12, 200 * 1024 * 1024));
        assert!(p.is_refused(), "{:?}", p.verdict);
        assert!(p.summary.contains("200 MB"), "{}", p.summary);
        assert!(p.summary.contains("100 MB"), "{}", p.summary);
    }

    #[test]
    fn exactly_at_the_byte_ceiling_is_allowed() {
        let p = seeded(&shape(Some(10), 10, MAX_BYTES));
        assert_eq!(p.verdict, PreflightVerdict::Ok, "{}", p.summary);
    }

    #[test]
    fn a_page_less_format_is_described_as_an_equivalence_never_as_pages() {
        // A .docx has no pages. Reporting "12 pages" about one states a fact
        // the file does not contain.
        let p = seeded(&shape(None, 12, 40_000));
        assert!(p.pages.is_none());
        assert!(p.summary.contains("about 12 pages' worth of text"), "{}", p.summary);
        assert!(!p.summary.contains("12 pages —"), "{}", p.summary);
    }

    #[test]
    fn the_estimate_follows_the_measured_rate_not_the_seed() {
        let s = shape(Some(100), 100, 1024);
        let seeded_p = seeded(&s);
        let measured = assess(&s, 2.0, EstimateBasis::Measured { samples: 3 });
        assert_eq!(seeded_p.estimated_seconds, 50.0);
        assert_eq!(measured.estimated_seconds, 200.0);
        assert_eq!(measured.estimate_basis, EstimateBasis::Measured { samples: 3 });
        // And a slow machine can push a document over the confirm line that a
        // fast one would have imported without asking — which is the point of
        // measuring rather than asserting.
        assert!(measured.summary.contains("about 3 minutes"), "{}", measured.summary);
    }

    #[test]
    fn durations_are_rounded_to_a_precision_the_estimate_actually_has() {
        assert_eq!(human_duration(0.4), "about a second");
        assert_eq!(human_duration(19.0), "about 19 seconds");
        assert_eq!(human_duration(200.0), "about 3 minutes");
        assert_eq!(human_duration(7200.0), "about 2.0 hours");
    }

    #[test]
    fn a_scanned_pdf_is_a_refusal_verdict_not_an_error() {
        // Every "you cannot import this" must arrive in ONE shape, or a caller
        // ends up handling the size refusal and missing the scanned one.
        let db = Database::in_memory().unwrap();
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/scanned_no_text.pdf"
        ));
        let p = preflight(&db, fixture, 100).expect("a scanned PDF is an ANSWER, not an error");
        assert!(p.is_refused(), "{:?}", p.verdict);
        assert!(p.summary.contains("no extractable text"), "{}", p.summary);
        assert!(p.summary.contains("OCR"), "the advice must say what to do: {}", p.summary);
        // And it costs nothing downstream: no estimate is implied for a file
        // that is never going to be indexed.
        assert_eq!(p.estimated_seconds, 0.0);
    }

    #[test]
    fn a_real_text_pdf_preflights_as_ok_with_an_estimate() {
        let db = Database::in_memory().unwrap();
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sample_text.pdf"
        ));
        let p = preflight(&db, fixture, 100).expect("a text PDF should preflight");
        assert_eq!(p.verdict, PreflightVerdict::Ok, "{}", p.summary);
        assert!(p.pages.is_some());
        assert!(p.estimated_seconds > 0.0);
        assert!(p.summary.contains("to index on this machine"), "{}", p.summary);
    }

    #[test]
    fn the_first_measurement_replaces_the_seed_rather_than_averaging_with_it() {
        // The seed is a guess. Averaging a guess with an observation would keep
        // the guess alive in every later estimate.
        let db = Database::in_memory().unwrap();
        assert_eq!(current_rate(&db, 100).1, EstimateBasis::Seeded);

        record_rate(&db, 10, 40.0, 100).unwrap(); // 4.0 s/page observed
        let (rate, basis) = current_rate(&db, 100);
        assert_eq!(rate, 4.0);
        assert_eq!(basis, EstimateBasis::Measured { samples: 1 });
    }

    #[test]
    fn later_measurements_are_averaged_in() {
        let db = Database::in_memory().unwrap();
        record_rate(&db, 10, 40.0, 100).unwrap(); // 4.0
        record_rate(&db, 10, 20.0, 100).unwrap(); // 2.0 → mean 3.0
        let (rate, basis) = current_rate(&db, 100);
        assert!((rate - 3.0).abs() < 1e-9, "got {rate}");
        assert_eq!(basis, EstimateBasis::Measured { samples: 2 });
    }

    #[test]
    fn a_nonsense_measurement_is_ignored_rather_than_stored() {
        let db = Database::in_memory().unwrap();
        record_rate(&db, 0, 40.0, 100).unwrap();
        record_rate(&db, 10, 0.0, 100).unwrap();
        record_rate(&db, 10, f64::NAN, 100).unwrap();
        assert_eq!(current_rate(&db, 100).1, EstimateBasis::Seeded, "a bad sample was stored");
    }

    #[test]
    fn a_lapsed_rate_falls_back_to_the_seed_rather_than_to_nothing() {
        // The rate lives in the TTL cache; losing it must be harmless.
        let db = Database::in_memory().unwrap();
        record_rate(&db, 10, 40.0, 100).unwrap();
        let (rate, basis) = current_rate(&db, 100 + RATE_TTL_SECS + 1);
        assert_eq!(rate, SEED_SECONDS_PER_PAGE);
        assert_eq!(basis, EstimateBasis::Seeded);
    }

    /// §11 D87. Our own audit report must not become indexable evidence.
    ///
    /// Generated by the LIVE pipeline — `compose_audit` then `render_pdf`, the
    /// exact two calls the export command makes — so this cannot go stale as
    /// the report format changes.
    #[test]
    fn a_freshly_exported_audit_report_is_refused_at_import() {
        let db = Database::in_memory().unwrap();
        let blocks = crate::audit_report::compose_audit(&crate::audit_report::AuditReportModel {
            manuscript_name: "R PAPER .pdf".into(),
            has_pages: true,
            generated_on: "5 September 2026".into(),
            model_id: "qwen2.5-3b-instruct-q4km".into(),
            prompt_version: "citation_support-v1.6".into(),
            total_sentences: 114,
            checked: 84,
            counts_by_category: vec![("citation_need".into(), 65)],
            verdict_counts: vec![("needs_citation".into(), 52)],
            skipped_reasons: vec![],
            supported: vec![],
            needs_citation: vec![],
            unverifiable: vec![],
            failed: vec![],
            consistency: vec![],
        });
        let pdf = crate::report_pdf::render_pdf(&blocks);

        let dir = std::env::temp_dir().join(format!("d87-fresh-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("audit.pdf");
        std::fs::write(&path, &pdf).unwrap();

        let pre = preflight(&db, &path, 0).unwrap();
        assert!(pre.is_refused(), "a freshly exported audit report was accepted: {}", pre.summary);
        assert!(pre.summary.contains("Gaply audit report"), "{}", pre.summary);
        // The refusal says WHY, not just no.
        assert!(pre.summary.contains("quote its own report back"), "{}", pre.summary);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §11 D87. The file that was ACTUALLY mis-audited, committed as a fixture.
    ///
    /// It predates D78 and carries only 5 of the 8 markers, which is the case a
    /// freshly generated PDF can never cover: a real report that THIS version of
    /// the composer never wrote, still refused. That is the whole argument for a
    /// threshold over a single sentinel (§11 D80).
    #[test]
    fn the_real_exported_report_is_refused_even_in_the_old_format() {
        let db = Database::in_memory().unwrap();
        let fixture =
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/gaply_audit_report.pdf");
        let path = std::path::Path::new(fixture);
        assert!(path.exists(), "fixture missing: {fixture}");

        let pre = preflight(&db, path, 0).unwrap();
        assert!(pre.is_refused(), "the real mis-audited report was accepted: {}", pre.summary);
        assert!(pre.summary.contains("Gaply audit report"), "{}", pre.summary);
        // It is the OLD format: fewer markers than today's composer emits, and
        // still over the threshold.
        let text = crate::extract::docparse::inspect_path_with_text(path).unwrap().1;
        let found = crate::ai_engine::audit_prepass::gaply_report_markers_in(&text);
        assert!(
            found.len() >= crate::ai_engine::audit_prepass::GAPLY_REPORT_MIN_MARKERS,
            "only {} markers: {found:?}",
            found.len()
        );
    }

    /// A guard that refuses everything is not a guard. An ordinary manuscript
    /// still imports.
    #[test]
    fn an_ordinary_document_still_imports() {
        let db = Database::in_memory().unwrap();
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample_text.pdf");
        let pre = preflight(&db, std::path::Path::new(fixture), 0).unwrap();
        assert!(!pre.is_refused(), "a normal document was refused: {}", pre.summary);
    }
}
