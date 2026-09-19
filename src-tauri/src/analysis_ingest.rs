//! **The analysis upload, as an INSTRUMENT. §11 D191.**
//!
//! §1 of the architecture calls supporting analysis "the differentiator", and
//! `[v7]` records that the app accepts none of it: every picker is
//! `pdf, docx, txt, md`, so the analysis layer has had no input since it was
//! designed. This is that input.
//!
//! # It accepts formats it cannot parse, on purpose
//!
//! `.py`, `.R` and `.ipynb` are accepted and NOT parsed. That is not an
//! oversight, it is the measurement: **the count of what arrives is what decides
//! whether those parsers are worth building**, and today that question has one
//! data point — the single real `.py` on the development machine imports
//! `numpy`, `scipy.integrate` and `scipy.special` and contains ZERO of the
//! procedures `ProcedureKind` enumerates. Writing a Python parser now would mean
//! inventing the fixtures, which is §11 D160.
//!
//! **A file uploaded and silently ignored is worse than one refused**, so every
//! file comes back with what was done with it, and the caller renders that
//! verbatim: *"3 files: 1 parsed (82 commands, 5 statistical), 2 stored and not
//! parsed."*
//!
//! # What this deliberately does NOT do
//!
//! It does not feed `frequentist::tests_absent_from_the_record`. That check is
//! built, is the only one in the product that compares what a manuscript CLAIMS
//! against what its author RAN, and **stays unwired**: on the one pairing
//! available it fired on 29 of 31 claimed tests, and three rows showed three
//! independent defects (§11 D191). Shipping the upload and the check together
//! would hand a researcher eleven Major findings on a twelve-test paper.
//!
//! The upload is what creates the first REAL pair — a manuscript and the
//! analysis that produced it, from the same study — which is the condition that
//! reopens the check.

use serde::Serialize;

use gaply_core::analysis::{spss, AnalysisRecord};
use gaply_core::GaplyError;

/// Cap per file. Analysis scripts are small; a multi-hundred-MB file is a
/// mistake, and metadata is checked before any content is read.
const MAX_ANALYSIS_BYTES: u64 = 8 * 1024 * 1024;

/// What happened to ONE uploaded file. Never a bare success/failure: the point
/// of the instrument is that "parsed", "stored but no parser exists" and "could
/// not be read" are three different answers and a user must be able to tell them
/// apart.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum FileOutcome {
    /// A parser exists for this format and it ran.
    Parsed { commands: usize, statistical: usize, unparsed_lines: usize },
    /// Accepted and held, with no parser for this format YET. The reason is
    /// shown to the user, because "we kept your file and did nothing with it" is
    /// the sentence that has to be said out loud.
    StoredNotParsed { reason: String },
    /// Could not be read at all.
    Rejected { reason: String },
}

/// One row of the honest line.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisFileReport {
    pub file_name: String,
    pub extension: String,
    pub size_bytes: u64,
    #[serde(flatten)]
    pub outcome: FileOutcome,
}

/// What one upload did, in a form a screen can render without deciding anything.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisIngestReport {
    pub files: Vec<AnalysisFileReport>,
    pub parsed: usize,
    pub stored_not_parsed: usize,
    pub rejected: usize,
    /// Totals across every file a parser ran on.
    pub commands: usize,
    pub statistical: usize,
    /// The sentence the screen shows. Built here so the two surfaces that could
    /// render it cannot word it differently.
    pub summary: String,
}

/// Extensions the picker offers. `.sps` and `.jnl` have a parser; the rest are
/// accepted so that what arrives can be COUNTED.
pub const ACCEPTED_EXTENSIONS: &[&str] = &["sps", "jnl", "py", "R", "r", "ipynb", "csv"];

fn extension_of(path: &std::path::Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// Parse what can be parsed, count everything else, and say so.
pub fn ingest(paths: &[String]) -> AnalysisIngestReport {
    let mut files = Vec::new();
    let (mut parsed, mut stored, mut rejected) = (0usize, 0usize, 0usize);
    let (mut commands, mut statistical) = (0usize, 0usize);

    for p in paths {
        let path = std::path::Path::new(p);
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = extension_of(path);
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        let outcome = if size > MAX_ANALYSIS_BYTES {
            rejected += 1;
            FileOutcome::Rejected {
                reason: format!(
                    "{size} bytes is over the {MAX_ANALYSIS_BYTES}-byte cap for an analysis file; \
                     nothing was read"
                ),
            }
        } else {
            match std::fs::read_to_string(path) {
                Err(e) => {
                    rejected += 1;
                    FileOutcome::Rejected { reason: format!("could not be read as text: {e}") }
                }
                Ok(text) => match ext.as_str() {
                    "sps" | "jnl" => {
                        let rec: AnalysisRecord = spss::parse(&name, &text);
                        parsed += 1;
                        commands += rec.procedures.len();
                        let s = rec.statistical().count();
                        statistical += s;
                        FileOutcome::Parsed {
                            commands: rec.procedures.len(),
                            statistical: s,
                            unparsed_lines: rec.unparsed_lines,
                        }
                    }
                    other => {
                        stored += 1;
                        FileOutcome::StoredNotParsed {
                            reason: format!(
                                "Gaply has no parser for .{other} yet. The file was accepted and \
                                 read, and nothing was extracted from it."
                            ),
                        }
                    }
                },
            }
        };
        files.push(AnalysisFileReport { file_name: name, extension: ext, size_bytes: size, outcome });
    }

    // THE HONEST LINE. Every clause is a count this function actually made.
    let mut parts: Vec<String> = Vec::new();
    if parsed > 0 {
        parts.push(format!("{parsed} parsed ({commands} commands, {statistical} statistical)"));
    }
    if stored > 0 {
        parts.push(format!("{stored} stored and not parsed"));
    }
    if rejected > 0 {
        parts.push(format!("{rejected} could not be read"));
    }
    let summary = if files.is_empty() {
        "No analysis files were chosen.".to_string()
    } else {
        format!(
            "{} file{}: {}.",
            files.len(),
            if files.len() == 1 { "" } else { "s" },
            parts.join(", ")
        )
    };

    // The durable half of the instrument: the counts reach the log whether or
    // not anyone reads the screen.
    tracing::info!(
        files = files.len(), parsed, stored_not_parsed = stored, rejected, commands, statistical,
        extensions = ?files.iter().map(|f| f.extension.clone()).collect::<Vec<_>>(),
        "analysis upload (D191 instrument)"
    );

    AnalysisIngestReport {
        files, parsed, stored_not_parsed: stored, rejected, commands, statistical, summary,
    }
}

/// Tauri surface. Sync: reading a few small text files needs no thread.
#[tauri::command]
#[tracing::instrument]
pub fn analysis_ingest(paths: Vec<String>) -> Result<AnalysisIngestReport, GaplyError> {
    Ok(ingest(&paths))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Hermetic on purpose, and the split is deliberate.** The REAL files this
    /// was measured against live at machine-specific paths — SPSS's journal
    /// under `~/Library/Application Support`, the one real `.py` in
    /// `~/Downloads` — so a test that read them would pass on one laptop and
    /// fail in CI. Those measurements live in `examples/analysis_record_probe.rs`
    /// and are recorded in §11 D191. What is pinned HERE is the part the
    /// instrument's honesty depends on: that three outcomes stay three, and that
    /// the sentence names each of them.
    fn write(dir: &std::path::Path, name: &str, body: &str) -> String {
        let p = dir.join(name);
        std::fs::write(&p, body).expect("write fixture");
        p.to_string_lossy().to_string()
    }

    /// **Unique PER TEST, and it has to be.** `now_epoch()` is second-resolution,
    /// so two tests starting in the same second shared one directory and the
    /// first to finish deleted the other's fixtures with `remove_dir_all`. It
    /// failed deterministically in a batch and passed when either test ran
    /// alone, which is the shape that reads as flakiness and is not.
    fn tmp(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "gaply-ingest-{label}-{}-{:?}",
            gaply_core::now_epoch(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&d).expect("mkdir");
        d
    }

    /// The SPSS shape is real syntax, not invented: one `FACTOR` with the
    /// subcommands the measured journal actually carries.
    const SPS: &str = "GET DATA /TYPE=TXT /FILE='x.csv'.\n\
FACTOR\n  /VARIABLES V17 V18 V19\n  /EXTRACTION PC\n  /ROTATION VARIMAX\n  /METHOD=CORRELATION.\n\
EXECUTE.\n";

    #[test]
    fn a_parsed_file_a_stored_one_and_the_sentence_that_names_both() {
        let d = tmp("parsed-and-stored");
        let paths = vec![
            write(&d, "analysis.sps", SPS),
            // The real .py on the dev machine is numerical mathematics with no
            // statistical call in it; this stands for that class.
            write(&d, "numerics.py", "import numpy as np\nfrom scipy.special import gamma\n"),
        ];
        let r = ingest(&paths);

        assert_eq!(r.parsed, 1);
        assert_eq!(r.stored_not_parsed, 1);
        assert_eq!(r.rejected, 0);
        assert!(r.commands >= 3, "the SPSS file yielded commands: {r:#?}");
        assert!(r.statistical >= 1, "FACTOR is a statistical procedure: {r:#?}");

        // THE LINE A USER READS. Both halves present, neither implied.
        assert!(r.summary.contains("2 files"), "{}", r.summary);
        assert!(r.summary.contains("1 parsed"), "{}", r.summary);
        assert!(r.summary.contains("stored and not parsed"), "{}", r.summary);

        // **A FILE ACCEPTED AND IGNORED MUST SAY SO IN ITS OWN ROW**, not only in
        // the total: the total is what a user skims, the row is what they act on.
        let py = r.files.iter().find(|f| f.extension == "py").expect("the .py row");
        match &py.outcome {
            FileOutcome::StoredNotParsed { reason } => {
                assert!(reason.contains(".py"), "the reason names the format: {reason}");
                assert!(
                    reason.contains("nothing was extracted"),
                    "and says plainly that nothing came of it: {reason}"
                );
            }
            other => panic!("a .py must be stored-not-parsed, not {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Three states, not two. `Rejected` and `StoredNotParsed` are different
    /// answers and collapsing them would tell a user their unreadable file was
    /// merely awaiting a parser.
    #[test]
    fn an_unreadable_file_is_rejected_not_stored() {
        let d = tmp("unreadable");
        let missing = d.join("does-not-exist.sps").to_string_lossy().to_string();
        let r = ingest(&[missing]);
        assert_eq!(r.rejected, 1);
        assert_eq!(r.stored_not_parsed, 0, "absence is not 'no parser yet'");
        assert!(r.summary.contains("could not be read"), "{}", r.summary);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The formats with no parser are ACCEPTED on purpose (§11 D191): the count
    /// of what arrives is the measurement that decides whether to build them.
    /// A list that quietly dropped them would answer the question "nobody
    /// uploads Python" by making it impossible to.
    #[test]
    fn the_unparseable_formats_are_offered_rather_than_refused() {
        for e in ["py", "R", "ipynb", "csv"] {
            assert!(
                ACCEPTED_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)),
                "{e} must be offered by the picker even with no parser"
            );
        }
        assert!(ACCEPTED_EXTENSIONS.contains(&"sps"), "and the one format with a parser");
    }

    /// Nothing chosen is not an error, and must not read as one.
    #[test]
    fn choosing_nothing_says_so() {
        let r = ingest(&[]);
        assert_eq!(r.files.len(), 0);
        assert!(r.summary.contains("No analysis files"), "{}", r.summary);
    }
}
