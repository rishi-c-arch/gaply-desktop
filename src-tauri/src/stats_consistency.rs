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
use gaply_core::stats_l0::outcome::{check_reading, StatsConsistencyOutcome};
use gaply_core::stats_l0::result::{read_results, TableStructure};
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

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::stats_l0::method::L0Verdict;

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

    #[test]
    fn a_missing_file_is_an_error_not_an_empty_outcome() {
        let db = Database::in_memory().unwrap();
        let missing = std::env::temp_dir().join("gaply-l0-seam-does-not-exist.txt");
        assert!(run_stats_consistency_measured(&db, &missing).is_err());
    }
}
