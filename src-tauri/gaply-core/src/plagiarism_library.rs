//! The durable "my papers" library (Plagiarism Check Set 3) — a persistent,
//! user-curated store of the user's OWN papers that the deterministic
//! exact-match check ([`plagiarism_exact`](crate::plagiarism_exact)) compares
//! an upload against.
//!
//! This replaces the Gap-Finder-corpus stopgap: the library is a dedicated,
//! lifecycle-managed local table (migration `plagiarism_library`), NOT
//! session-tagged and NOT mixed with journal/RAG documents. Each paper stores
//! its extracted `full_text` plus its Set-2 winnowing fingerprints, computed
//! ONCE at add time and reused on every check (no re-fingerprinting per run).
//!
//! CRUD lives in gaply-core because [`Database::conn`](crate::Database) is
//! `pub(crate)` — the same reason `citation_library`/`projects` CRUD lives
//! here. Fully local + deterministic: sqlite + Set-2 fingerprinting, no LLM,
//! no proxy, no network. The docparse step (path → text) is done by the app
//! command, mirroring `check_plagiarism`; the core takes the text.

use rusqlite::params;
use serde::Serialize;

use crate::plagiarism_exact::{
    self, ExactConfig, ExactPlagiarismReport, PreparedCompareDoc,
};
use crate::{now_epoch, Database, GaplyError};

/// A library entry as shown in a list (no full text / fingerprints).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LibraryPaper {
    pub id: i64,
    pub title: String,
    pub added_at: i64,
    pub source_label: String,
}

/// Add a paper to the library: compute its Set-2 fingerprints ONCE and store
/// them with the extracted text. Returns the new row id.
pub fn add_paper(
    db: &Database,
    title: &str,
    full_text: &str,
    source_label: &str,
    config: &ExactConfig,
) -> Result<i64, GaplyError> {
    let fingerprints = plagiarism_exact::fingerprint_document(full_text, config);
    let fps_json = serde_json::to_string(&fingerprints)
        .map_err(|e| GaplyError::Internal(format!("serialize fingerprints: {e}")))?;
    let conn = db.conn()?;
    conn.execute(
        "INSERT INTO plagiarism_library (title, full_text, fingerprints, source_label, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![title, full_text, fps_json, source_label, now_epoch()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Enumerate the library (metadata only — never the full text).
pub fn list_papers(db: &Database) -> Result<Vec<LibraryPaper>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, added_at, source_label FROM plagiarism_library ORDER BY added_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(LibraryPaper {
            id: r.get(0)?,
            title: r.get(1)?,
            added_at: r.get(2)?,
            source_label: r.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(GaplyError::from)
}

/// Remove a paper by id. Returns whether a row was deleted.
pub fn remove_paper(db: &Database, id: i64) -> Result<bool, GaplyError> {
    let n = db
        .conn()?
        .execute("DELETE FROM plagiarism_library WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

/// Compare an upload against the WHOLE library using the deterministic
/// exact-match core with each paper's PRECOMPUTED fingerprints. The upload is
/// session-isolated — it is NEVER written to the library (adding is an explicit
/// [`add_paper`] action). Self-plagiarism within the upload is also reported.
pub fn compare_against_library(
    db: &Database,
    upload_text: &str,
    config: &ExactConfig,
) -> Result<ExactPlagiarismReport, GaplyError> {
    // Load rows, then DROP the connection before the CPU-bound match so no DB
    // lock is held across compute.
    let rows: Vec<(String, String, String)> = {
        let conn = db.conn()?;
        let mut stmt = conn.prepare(
            "SELECT title, full_text, fingerprints FROM plagiarism_library ORDER BY added_at DESC",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };

    let mut library = Vec::with_capacity(rows.len());
    for (title, text, fps_json) in rows {
        let fingerprints: Vec<(u64, usize)> = serde_json::from_str(&fps_json)
            .map_err(|e| GaplyError::Internal(format!("deserialize fingerprints for '{title}': {e}")))?;
        library.push(PreparedCompareDoc { reference: title, text, fingerprints });
    }

    Ok(plagiarism_exact::analyze_exact_prepared(upload_text, &library, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECYCLED: &str =
        "The mitochondrial membrane potential collapses during the early phase of apoptosis and \
         triggers the release of cytochrome c into the cytosol activating the caspase cascade.";

    fn db() -> Database {
        Database::in_memory().expect("in-memory db")
    }

    #[test]
    fn add_list_remove_round_trip() {
        let db = db();
        let cfg = ExactConfig::default();
        let id = add_paper(&db, "Sleep & Memory (2021)", &format!("Intro. {RECYCLED} End."), "/papers/sleep.pdf", &cfg)
            .unwrap();

        let papers = list_papers(&db).unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].id, id);
        assert_eq!(papers[0].title, "Sleep & Memory (2021)");
        assert_eq!(papers[0].source_label, "/papers/sleep.pdf");

        assert!(remove_paper(&db, id).unwrap());
        assert!(list_papers(&db).unwrap().is_empty());
        assert!(!remove_paper(&db, id).unwrap(), "removing a gone id is false");
    }

    #[test]
    fn stored_fingerprints_are_persisted_and_reused_not_recomputed() {
        let db = db();
        let cfg = ExactConfig::default();
        let text = format!("Body. {RECYCLED} tail words to pad the paper out nicely.");
        add_paper(&db, "Paper", &text, "", &cfg).unwrap();

        // the stored fingerprints must equal a fresh computation (persisted
        // faithfully, so the compare path reuses them without recomputing).
        let stored: String = db
            .conn()
            .unwrap()
            .query_row("SELECT fingerprints FROM plagiarism_library", [], |r| r.get(0))
            .unwrap();
        let stored_fps: Vec<(u64, usize)> = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored_fps, plagiarism_exact::fingerprint_document(&text, &cfg));
        assert!(!stored_fps.is_empty());
    }

    #[test]
    fn compare_finds_a_library_match_with_spans_in_both_and_names_the_scope() {
        let db = db();
        let cfg = ExactConfig::default();
        let paper = format!("Their earlier work. {RECYCLED} plus the rest of their study text.");
        add_paper(&db, "Prior study (2019)", &paper, "/lib/prior.pdf", &cfg).unwrap();

        let upload = format!("My new manuscript introduction. {RECYCLED}");
        let report = compare_against_library(&db, &upload, &cfg).unwrap();

        assert_eq!(report.library_matches.len(), 1, "{:#?}", report.library_matches);
        let m = &report.library_matches[0];
        assert_eq!(&upload[m.source.start_char..m.source.end_char], m.source.text);
        assert_eq!(&paper[m.matched.start_char..m.matched.end_char], m.matched.text);
        assert!(m.source.text.contains("cytochrome c"));
        assert_eq!(m.source_ref, "Prior study (2019)");
        // scope is honest and specific: names the actual library papers
        assert!(report.disclosure.contains("Prior study (2019)"));
        assert!(report.disclosure.contains("your 1 paper(s) in your library"));
        assert!(report.disclosure.contains("does NOT replace Turnitin"));
        // and only the one fingerprint-sharing paper was tokenized
        assert_eq!(report.stats.library_papers_examined, 1);
    }

    #[test]
    fn the_upload_is_never_auto_added_to_the_library() {
        let db = db();
        let cfg = ExactConfig::default();
        add_paper(&db, "Existing", &format!("{RECYCLED} filler tail."), "", &cfg).unwrap();
        let before = list_papers(&db).unwrap().len();

        let _ = compare_against_library(&db, &format!("Upload text. {RECYCLED}"), &cfg).unwrap();

        // checking an upload must NOT grow the library (adding is explicit)
        assert_eq!(list_papers(&db).unwrap().len(), before);
    }

    #[test]
    fn empty_library_compare_still_runs_self_mode_deterministically() {
        let db = db();
        let cfg = ExactConfig::default();
        let upload = format!("One. {RECYCLED} Two. Some unique filler. Three. {RECYCLED}");
        let a = compare_against_library(&db, &upload, &cfg).unwrap();
        let b = compare_against_library(&db, &upload, &cfg).unwrap();
        assert_eq!(a, b, "same library + upload → same report");
        assert!(a.library_matches.is_empty());
        assert_eq!(a.self_matches.len(), 1, "self-recycling still detected");
        assert!(a.disclosure.contains("your paper library is empty"));
    }
}
