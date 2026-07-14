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
///
/// `citation_id` (M2 Set 2A) is the OPTIONAL soft anchor → `citation_library.id`
/// — the reliable link that lets Note Creator's side-by-side resolve by a shared
/// id instead of only by title. `None` stores NULL (no association), which is the
/// default until Set 2B wires the add-time picker; existing callers pass `None`
/// and behave exactly as before. NOT unique: two uploads may share one citation.
pub fn add_paper(
    db: &Database,
    title: &str,
    full_text: &str,
    source_label: &str,
    citation_id: Option<&str>,
    config: &ExactConfig,
) -> Result<i64, GaplyError> {
    let fingerprints = plagiarism_exact::fingerprint_document(full_text, config);
    let fps_json = serde_json::to_string(&fingerprints)
        .map_err(|e| GaplyError::Internal(format!("serialize fingerprints: {e}")))?;
    let conn = db.conn()?;
    conn.execute(
        "INSERT INTO plagiarism_library (title, full_text, fingerprints, source_label, citation_id, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![title, full_text, fps_json, source_label, citation_id, now_epoch()],
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

/// Read-only accessor: the stored `full_text` of a library paper matched by
/// title, or `None`. ADDITIVE — for Note Creator's optional side-by-side reading
/// (Set 6); the library's own add/list/remove/compare behavior is unchanged.
/// Deterministic, local, no model, no network.
///
/// HONEST, SAFE match (M2 Set 1): the title comparison is NORMALIZED (trim +
/// case-insensitive, mirroring the frontend badge) and AMBIGUITY-GUARDED — it
/// returns text ONLY when EXACTLY ONE library paper matches. Two same-title
/// papers → `None` (NEVER the wrong one — the old `ORDER BY added_at DESC LIMIT 1`
/// silently returned whichever was added last). Zero → `None`. (Fully reliable
/// linking by a shared id is the deferred Set 2; here the side-by-side just
/// degrades honestly rather than showing the wrong paper's text.)
pub fn full_text_by_title(db: &Database, title: &str) -> Result<Option<String>, GaplyError> {
    let needle = title.trim();
    if needle.is_empty() {
        return Ok(None);
    }
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT full_text FROM plagiarism_library WHERE lower(trim(title)) = lower(trim(?1))",
    )?;
    let mut rows = stmt.query_map(params![needle], |r| r.get::<_, String>(0))?;
    // Exactly-one guard: a first match with no second is the only case that
    // yields text; anything else (zero, or 2+ ambiguous) → None, never a guess.
    let first = match rows.next() {
        Some(r) => r?,
        None => return Ok(None),
    };
    if rows.next().is_some() {
        return Ok(None);
    }
    Ok(Some(first))
}

/// Read-only accessor: the stored `full_text` of a library paper matched by its
/// `citation_id` soft anchor (M2 Set 2A), or `None`. The RELIABLE side-by-side
/// path — an exact id match, not a fuzzy title match. ADDITIVE; nothing in the
/// live read chain calls this yet (Set 2C wires the id-first resolution into
/// `note_paper_fulltext`). Deterministic, local, no model, no network.
///
/// SAME "never the wrong paper" FLOOR as [`full_text_by_title`]: `citation_id`
/// is NOT unique (a user may associate two uploads with one citation), so this
/// is AMBIGUITY-GUARDED — text ONLY when EXACTLY ONE row carries the id. Zero →
/// `None`; 2+ → `None` (never a guess). An empty id → `None` (NULL rows and
/// free-typed notes with no anchor never match here — they use the title path).
pub fn full_text_by_citation_id(
    db: &Database,
    citation_id: &str,
) -> Result<Option<String>, GaplyError> {
    let needle = citation_id.trim();
    if needle.is_empty() {
        return Ok(None);
    }
    let conn = db.conn()?;
    let mut stmt =
        conn.prepare("SELECT full_text FROM plagiarism_library WHERE citation_id = ?1")?;
    let mut rows = stmt.query_map(params![needle], |r| r.get::<_, String>(0))?;
    // Exactly-one guard, mirroring full_text_by_title: a first match with no
    // second yields text; zero or 2+ → None, never a guess.
    let first = match rows.next() {
        Some(r) => r?,
        None => return Ok(None),
    };
    if rows.next().is_some() {
        return Ok(None);
    }
    Ok(Some(first))
}

/// M2 Set 2C — the side-by-side READ resolution chain for a note: resolve the
/// paper's stored `full_text` by the RELIABLE id first, then Set-1's honest title
/// fallback, else `None`. This is where the citation↔plagiarism link (2A column +
/// 2B capture) finally pays off in the live read.
///
/// Order (every branch guarded — NO branch returns a wrong/unverified paper):
///   1. ID (reliable): `paper_id` present & non-empty → [`full_text_by_citation_id`]
///      (exactly-one-guarded, 2A). `Some` → done.
///   2. TITLE (Set-1's honest fallback): no id, OR the id read returned `None`
///      (no link / backfill gap / old data) → [`full_text_by_title`]
///      (exactly-one-guarded). Free-typed notes (`paper_id` empty/None) land here,
///      exactly as before 2C.
///   3. `None`: neither matched — honest empty-state.
///
/// Pure sequencing: it does NOT change either underlying read (both stay
/// ambiguity-guarded), so the "never the wrong paper" floor holds on both paths.
pub fn full_text_for_note(
    db: &Database,
    title: &str,
    paper_id: Option<&str>,
) -> Result<Option<String>, GaplyError> {
    if let Some(pid) = paper_id {
        if !pid.trim().is_empty() {
            if let Some(text) = full_text_by_citation_id(db, pid)? {
                return Ok(Some(text));
            }
        }
    }
    full_text_by_title(db, title)
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
        let id = add_paper(&db, "Sleep & Memory (2021)", &format!("Intro. {RECYCLED} End."), "/papers/sleep.pdf", None, &cfg)
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
    fn full_text_by_title_reads_the_stored_text_additively() {
        let db = db();
        let cfg = ExactConfig::default();
        let body = format!("Intro. {RECYCLED} End.");
        add_paper(&db, "Sleep & Memory (2021)", &body, "/papers/sleep.pdf", None, &cfg).unwrap();

        // returns the stored full_text for a matching title
        assert_eq!(full_text_by_title(&db, "Sleep & Memory (2021)").unwrap().as_deref(), Some(body.as_str()));
        // M2 Set 1 — CASE/WHITESPACE-INSENSITIVE single match returns the RIGHT text
        // (fixes the false-negative from the old case-sensitive read).
        assert_eq!(full_text_by_title(&db, "  sleep & MEMORY (2021)  ").unwrap().as_deref(), Some(body.as_str()));
        // None for an unknown title (graceful — no side-by-side)
        assert_eq!(full_text_by_title(&db, "Not In Library").unwrap(), None);
        // empty/whitespace title → None
        assert_eq!(full_text_by_title(&db, "   ").unwrap(), None);
        // read-only: the library is unchanged after the read
        assert_eq!(list_papers(&db).unwrap().len(), 1);
    }

    #[test]
    fn full_text_by_title_never_returns_the_wrong_paper_on_a_title_collision() {
        // THE FLOOR (M2 Set 1, bug c): two papers share a (normalized) title →
        // full_text_by_title must return None, NEVER the most-recently-added one.
        let db = db();
        let cfg = ExactConfig::default();
        add_paper(&db, "Common Title", "FIRST paper body", "/a.pdf", None, &cfg).unwrap();
        add_paper(&db, "common title", "SECOND paper body (added later)", "/b.pdf", None, &cfg).unwrap();

        // ambiguous (2 case-insensitive matches) → None: we never guess which one
        assert_eq!(full_text_by_title(&db, "Common Title").unwrap(), None);
        assert_eq!(full_text_by_title(&db, "common title").unwrap(), None);
        // both rows still present — read-only, nothing dropped
        assert_eq!(list_papers(&db).unwrap().len(), 2);
    }

    // --- M2 Set 2A: the citation_id write-path + the id-keyed read ---

    fn stored_citation_id(db: &Database, row_id: i64) -> Option<String> {
        db.conn()
            .unwrap()
            .query_row(
                "SELECT citation_id FROM plagiarism_library WHERE id = ?1",
                params![row_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap()
    }

    #[test]
    fn add_paper_stores_citation_id_when_given_and_null_when_none() {
        let db = db();
        let cfg = ExactConfig::default();
        // with an id → persisted verbatim
        let linked = add_paper(&db, "Linked", "body one", "/one.pdf", Some("cit-uuid-1"), &cfg).unwrap();
        assert_eq!(stored_citation_id(&db, linked).as_deref(), Some("cit-uuid-1"));
        // without → NULL (the unchanged, default behavior existing callers keep)
        let unlinked = add_paper(&db, "Unlinked", "body two", "/two.pdf", None, &cfg).unwrap();
        assert_eq!(stored_citation_id(&db, unlinked), None);
    }

    #[test]
    fn full_text_by_citation_id_reads_the_text_for_an_exact_single_match() {
        let db = db();
        let cfg = ExactConfig::default();
        add_paper(&db, "Sleep & Memory", "the stored body", "/s.pdf", Some("cit-A"), &cfg).unwrap();

        // exact id match → its full_text
        assert_eq!(full_text_by_citation_id(&db, "cit-A").unwrap().as_deref(), Some("the stored body"));
        // an unknown id → None (graceful)
        assert_eq!(full_text_by_citation_id(&db, "cit-UNKNOWN").unwrap(), None);
        // empty/whitespace id → None (NULL rows + free-typed notes never match here)
        assert_eq!(full_text_by_citation_id(&db, "   ").unwrap(), None);
        // read-only
        assert_eq!(list_papers(&db).unwrap().len(), 1);
    }

    #[test]
    fn full_text_by_citation_id_never_returns_the_wrong_paper_when_an_id_is_shared() {
        // THE FLOOR on the ID path (M2 Set 2A): citation_id is NOT unique — a user
        // may associate two uploads with one citation. Two rows share the id →
        // full_text_by_citation_id must return None, NEVER a guess.
        let db = db();
        let cfg = ExactConfig::default();
        add_paper(&db, "Version one", "FIRST body", "/v1.pdf", Some("cit-shared"), &cfg).unwrap();
        add_paper(&db, "Version two", "SECOND body", "/v2.pdf", Some("cit-shared"), &cfg).unwrap();

        // ambiguous (2 matches) → None, mirroring the title-collision floor
        assert_eq!(full_text_by_citation_id(&db, "cit-shared").unwrap(), None);
        // both rows still present — read-only
        assert_eq!(list_papers(&db).unwrap().len(), 2);
    }

    #[test]
    fn a_null_citation_id_row_never_matches_the_id_read() {
        // A row added with None (NULL citation_id) must be invisible to the id
        // read — it is only reachable via the title path (Set-1). Guards against
        // an empty/NULL id ever colliding.
        let db = db();
        let cfg = ExactConfig::default();
        add_paper(&db, "No Anchor", "orphan body", "/n.pdf", None, &cfg).unwrap();
        assert_eq!(full_text_by_citation_id(&db, "").unwrap(), None);
        // and the title path STILL reads it (Set-1 untouched)
        assert_eq!(full_text_by_title(&db, "No Anchor").unwrap().as_deref(), Some("orphan body"));
    }

    // --- M2 Set 2C: the id-first / title-fallback / None resolution chain ---

    #[test]
    fn resolve_id_match_wins_even_when_the_title_differs() {
        // The reliable path: a note whose paper_id links to a plagiarism row (via
        // citation_id) resolves by ID — even if the note's title ≠ the row's title.
        let db = db();
        let cfg = ExactConfig::default();
        add_paper(&db, "Row Title (in library)", "the id-linked body", "/p.pdf", Some("cit-A"), &cfg).unwrap();

        // note title deliberately DIFFERENT from the row title → id still resolves it
        let got = full_text_for_note(&db, "A Totally Different Note Title", Some("cit-A")).unwrap();
        assert_eq!(got.as_deref(), Some("the id-linked body"));
    }

    #[test]
    fn resolve_falls_back_to_title_when_no_id_or_the_id_is_unlinked() {
        let db = db();
        let cfg = ExactConfig::default();
        // a title-matching row that has NO citation link (added with None)
        add_paper(&db, "Shared Title", "the title-matched body", "/t.pdf", None, &cfg).unwrap();

        // (a) free-typed note: paper_id = "" → skip id read → Set-1 title fallback
        assert_eq!(
            full_text_for_note(&db, "Shared Title", Some("")).unwrap().as_deref(),
            Some("the title-matched body")
        );
        // (b) no paper_id at all → title fallback
        assert_eq!(
            full_text_for_note(&db, "Shared Title", None).unwrap().as_deref(),
            Some("the title-matched body")
        );
        // (c) paper_id present but NOT linked to any row (backfill gap / old data)
        //     → id read None → title fallback still finds it
        assert_eq!(
            full_text_for_note(&db, "Shared Title", Some("cit-UNLINKED")).unwrap().as_deref(),
            Some("the title-matched body")
        );
    }

    #[test]
    fn resolve_returns_none_when_neither_id_nor_title_matches() {
        let db = db();
        let cfg = ExactConfig::default();
        add_paper(&db, "Some Library Paper", "body", "/x.pdf", Some("cit-X"), &cfg).unwrap();

        // wrong id AND wrong title → honest None, never a guess
        assert_eq!(full_text_for_note(&db, "Unknown Note Title", Some("cit-NOPE")).unwrap(), None);
    }

    #[test]
    fn resolve_never_returns_the_wrong_paper_on_either_path_the_floor() {
        // THE FLOOR (M2), proven on BOTH paths of the chain:
        let db = db();
        let cfg = ExactConfig::default();

        // id-path collision: two rows share citation_id 'cit-dup' (different titles,
        // NEITHER matching the note title) → id read guarded → None → title read
        // finds nothing → overall None (the id collision NEVER leaks a paper).
        add_paper(&db, "Dup A", "A body", "/a.pdf", Some("cit-dup"), &cfg).unwrap();
        add_paper(&db, "Dup B", "B body", "/b.pdf", Some("cit-dup"), &cfg).unwrap();
        assert_eq!(full_text_for_note(&db, "A Note Title Matching Neither", Some("cit-dup")).unwrap(), None);

        // title-path collision (Set-1 floor): two rows share a normalized title,
        // note has no id → title read guarded → None (never the last-added one).
        add_paper(&db, "Twin", "first twin", "/1.pdf", None, &cfg).unwrap();
        add_paper(&db, "twin", "second twin", "/2.pdf", None, &cfg).unwrap();
        assert_eq!(full_text_for_note(&db, "Twin", None).unwrap(), None);
    }

    #[test]
    fn stored_fingerprints_are_persisted_and_reused_not_recomputed() {
        let db = db();
        let cfg = ExactConfig::default();
        let text = format!("Body. {RECYCLED} tail words to pad the paper out nicely.");
        add_paper(&db, "Paper", &text, "", None, &cfg).unwrap();

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
        add_paper(&db, "Prior study (2019)", &paper, "/lib/prior.pdf", None, &cfg).unwrap();

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
        add_paper(&db, "Existing", &format!("{RECYCLED} filler tail."), "", None, &cfg).unwrap();
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
