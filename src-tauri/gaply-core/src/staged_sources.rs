//! The audit's own copy of a manuscript's reference list, scoped to one job.
//!
//! # WHY NOT `citation_library` (§11 D132)
//!
//! An author-year reference list is 35 entries on a real paper, including
//! malformed ones and organisational authors ("P4H Network"), and
//! `citation_library` is shared by every feature. Writing them there would put
//! junk rows in the user's own collection — hard to un-import, for works they
//! never asked to collect. **They asked for their manuscript to be checked.**
//!
//! Staging keeps the audit's needs separate from the user's library. "Promote to
//! library" can be an explicit action later if anyone wants one; it is not a side
//! effect of running an audit.
//!
//! # ONE WORK, ONE ROW
//!
//! A staged source and a library citation can name the same work, and must not
//! fetch twice or show twice. **The library row wins and only what is missing is
//! staged**, deduped on [`normalise_doi`]. The library wins because it may
//! already carry a linked, indexed, embedded PDF and a title the user curated,
//! and because `citation_documents` links are keyed to `citation_library.id` —
//! so preferring it keeps resolution on ONE path instead of teaching every
//! consumer about two.
//!
//! **No title-based dedupe.** Deciding that two differently-spelled entries are
//! one work by title similarity is the same judgement as deciding a title lookup
//! found the right paper, and that is gated behind its own measurement rather
//! than smuggled in here as a convenience.

use rusqlite::{params, OptionalExtension};

use crate::ai_engine::audit_prepass::AuthorYearEntry;
use crate::db::Database;
use crate::GaplyError;

/// One staged reference, as the fetch and the report need it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedSource {
    pub id: i64,
    pub surname: String,
    pub year: Option<i32>,
    pub title: Option<String>,
    pub doi: Option<String>,
    /// The document this source was fetched into, once it has been.
    pub document_id: Option<i64>,
    /// How the link was made — same vocabulary as `citation_documents`.
    pub matched_by: Option<String>,
}

/// What staging did, so the caller can report it rather than guess.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageOutcome {
    pub staged: usize,
    /// Skipped because `citation_library` already holds this DOI — the library
    /// path owns it.
    pub already_in_library: usize,
    /// Skipped because the same DOI was already staged for this job.
    pub duplicate_doi: usize,
}

/// A DOI reduced to its comparable form: lowercased, prefix stripped, trailing
/// punctuation trimmed.
///
/// Stored rather than computed per query so the UNIQUE index can use it. The
/// prefixes are the three shapes reference lists actually print.
pub fn normalise_doi(doi: &str) -> Option<String> {
    let t = doi.trim().to_lowercase();
    let t = t
        .strip_prefix("https://doi.org/")
        .or_else(|| t.strip_prefix("http://doi.org/"))
        .or_else(|| t.strip_prefix("doi:"))
        .unwrap_or(&t)
        .trim()
        .trim_end_matches(['.', ',', ';', ')']);
    (!t.is_empty() && t.starts_with("10.")).then(|| t.to_string())
}

/// Stage a manuscript's author-year entries for one job.
///
/// Idempotent: re-staging the same job adds nothing, because the table is unique
/// on `(job_id, raw)` and on `(job_id, doi_norm)`.
pub fn stage_entries(
    db: &Database,
    job_id: i64,
    entries: &[AuthorYearEntry],
    now: i64,
) -> Result<StageOutcome, GaplyError> {
    let conn = db.conn()?;
    let mut out = StageOutcome::default();

    // THE LIBRARY'S DOIs, NORMALISED THROUGH THE SAME FUNCTION.
    //
    // The first version compared a bare DOI against `LOWER(TRIM(doi))` in SQL
    // and missed every library row storing the `https://doi.org/` form — which
    // is most of them. The fix is not a cleverer `LIKE`: it is to run ONE
    // normaliser over both sides. A second definition of "the same DOI", written
    // in SQL because SQL was nearer, is §11 D129's defect in miniature.
    let known: std::collections::HashSet<String> = {
        let mut stmt =
            conn.prepare("SELECT doi FROM citation_library WHERE doi IS NOT NULL AND doi <> ''")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.iter().filter_map(|d| normalise_doi(d)).collect()
    };

    for e in entries {
        let norm = e.doi.as_deref().and_then(normalise_doi);
        if let Some(n) = norm.as_deref() {
            // RULE 1: the library owns it.
            if known.contains(n) {
                out.already_in_library += 1;
                continue;
            }
        }
        // RULE 2: unique per job. `INSERT OR IGNORE` leans on the two UNIQUE
        // constraints rather than re-querying, so a concurrent stage cannot
        // produce a duplicate between the check and the write.
        let changed = conn.execute(
            "INSERT OR IGNORE INTO audit_staged_sources
                 (job_id, surname, year, title, doi, doi_norm, raw, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![job_id, e.surname, e.year, e.title, e.doi, norm, e.raw, now],
        )?;
        if changed == 1 {
            out.staged += 1;
        } else if norm.is_some() {
            out.duplicate_doi += 1;
        }
    }
    Ok(out)
}

/// Every staged source for a job, in insertion order.
pub fn list_for_job(db: &Database, job_id: i64) -> Result<Vec<StagedSource>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, surname, year, title, doi, document_id, matched_by
         FROM audit_staged_sources WHERE job_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map(params![job_id], |r| {
            Ok(StagedSource {
                id: r.get(0)?,
                surname: r.get(1)?,
                year: r.get(2)?,
                title: r.get(3)?,
                doi: r.get(4)?,
                document_id: r.get(5)?,
                matched_by: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The staged sources that can be fetched without a network guess: those with a
/// DOI and no document yet.
pub fn fetchable_by_doi(db: &Database, job_id: i64) -> Result<Vec<StagedSource>, GaplyError> {
    Ok(list_for_job(db, job_id)?
        .into_iter()
        .filter(|s| s.doi.is_some() && s.document_id.is_none())
        .collect())
}

/// Is this staged source's document ready to be checked against?
///
/// The SAME question `citation_links::checkable_document_for_citation` answers
/// for a library citation, and deliberately the same shape of answer: a document
/// with chunks that carry embeddings. A link alone is not checkability — it says
/// which file backs the source, not that the file can be searched.
pub fn checkable_document(db: &Database, staged_id: i64) -> Result<Option<i64>, GaplyError> {
    let conn = db.conn()?;
    let doc: Option<i64> = conn
        .query_row(
            "SELECT document_id FROM audit_staged_sources WHERE id = ?1",
            params![staged_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let Some(doc) = doc else { return Ok(None) };
    let ready: bool = conn.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM ai_chunks c
             JOIN ai_chunk_embeddings e ON e.chunk_id = c.id
             WHERE c.document_id = ?1
         )",
        params![doc],
        |r| r.get(0),
    )?;
    Ok(ready.then_some(doc))
}

/// Record that a staged source was fetched into a document.
///
/// `matched_by` mirrors `citation_documents`' vocabulary, so "how was this link
/// made" means the same thing on both sides — 'doi' here, and 'title' only once
/// the title path has passed its own gate.
pub fn link_document(
    db: &Database,
    staged_id: i64,
    document_id: i64,
    matched_by: &str,
) -> Result<(), GaplyError> {
    debug_assert!(matches!(matched_by, "doi" | "title" | "manual"));
    let conn = db.conn()?;
    conn.execute(
        "UPDATE audit_staged_sources SET document_id = ?2, matched_by = ?3 WHERE id = ?1",
        params![staged_id, document_id, matched_by],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_with_job() -> (Database, i64) {
        let db = Database::in_memory().unwrap();
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO ai_jobs (kind, status, total_items, prompt_version, created_at)
             VALUES ('thesis_audit', 'queued', 0, 'v1', 1)",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        drop(conn);
        (db, id)
    }

    fn entry(surname: &str, year: i32, doi: Option<&str>, raw: &str) -> AuthorYearEntry {
        AuthorYearEntry {
            surname: surname.to_string(),
            year: Some(year),
            doi: doi.map(str::to_string),
            title: Some(format!("A work by {surname}")),
            raw: raw.to_string(),
        }
    }

    #[test]
    fn normalising_a_doi_strips_every_shape_a_reference_list_prints() {
        for s in [
            "https://doi.org/10.1186/s12913-015-1132-5",
            "http://doi.org/10.1186/s12913-015-1132-5",
            "doi:10.1186/s12913-015-1132-5",
            "10.1186/S12913-015-1132-5.",
            "  10.1186/s12913-015-1132-5,  ",
        ] {
            assert_eq!(
                normalise_doi(s).as_deref(),
                Some("10.1186/s12913-015-1132-5"),
                "{s:?}"
            );
        }
        // Not a DOI at all.
        assert_eq!(normalise_doi("World Health Organization"), None);
        assert_eq!(normalise_doi(""), None);
    }

    /// RULE 1 — §11 D132. The library owns a work it already holds: staging a
    /// second copy would fetch twice and show twice, and the library row may
    /// already carry an indexed PDF and a curated title.
    #[test]
    fn an_entry_already_in_the_library_is_not_staged() {
        let (db, job) = db_with_job();
        db.conn()
            .unwrap()
            .execute(
                "INSERT INTO citation_library (id, csl_json, doi, title, authors, year, tags, sync_status, created_at, updated_at)
                 VALUES ('lib-1', '{}', 'https://doi.org/10.1/ALREADY', 'Held', 'A', 2020, '[]', 'local_only', 1, 1)",
                [],
            )
            .unwrap();
        // The library stores it prefixed and upper-cased; the entry prints it
        // bare and lower-cased. Matching must survive both.
        let out = stage_entries(
            &db,
            job,
            &[
                entry("held", 2020, Some("10.1/already"), "Held, A. (2020). Held."),
                entry("fresh", 2021, Some("10.1/fresh"), "Fresh, B. (2021). Fresh."),
            ],
            1,
        )
        .unwrap();
        assert_eq!(out.staged, 1, "the fresh entry must be staged");
        assert_eq!(out.already_in_library, 1, "the held entry must be skipped");
        let staged = list_for_job(&db, job).unwrap();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].surname, "fresh");
    }

    /// RULE 2 — a reference list that prints the same work twice stages it once.
    #[test]
    fn the_same_doi_is_staged_once_per_job() {
        let (db, job) = db_with_job();
        let out = stage_entries(
            &db,
            job,
            &[
                entry("a", 2020, Some("10.1/x"), "A, A. (2020). First printing."),
                entry("a", 2020, Some("https://doi.org/10.1/X"), "A, A. (2020). Second printing."),
            ],
            1,
        )
        .unwrap();
        assert_eq!(out.staged, 1);
        assert_eq!(out.duplicate_doi, 1);
        assert_eq!(list_for_job(&db, job).unwrap().len(), 1);
    }

    /// DOI-less entries are staged and must NOT collide with each other — they
    /// are deduped on `raw`, because nothing yet establishes that two of them are
    /// the same work. Deciding that by title is the judgement §11 D132 gates.
    #[test]
    fn entries_without_a_doi_are_staged_separately() {
        let (db, job) = db_with_job();
        let out = stage_entries(
            &db,
            job,
            &[
                entry("cashin", 2017, None, "Cashin, C. (2017). Aligning PFM. WHO."),
                entry("p4h", 2024, None, "P4H Network. (2024). Matrix for Oman. P4H."),
            ],
            1,
        )
        .unwrap();
        assert_eq!(out.staged, 2, "two DOI-less works collapsed into one");
        assert_eq!(out.duplicate_doi, 0);
    }

    /// Re-running an audit must not grow the table.
    #[test]
    fn staging_the_same_job_twice_is_idempotent() {
        let (db, job) = db_with_job();
        let e = [entry("a", 2020, Some("10.1/x"), "A, A. (2020). Title.")];
        stage_entries(&db, job, &e, 1).unwrap();
        let second = stage_entries(&db, job, &e, 2).unwrap();
        assert_eq!(second.staged, 0);
        assert_eq!(list_for_job(&db, job).unwrap().len(), 1);
    }

    /// NEVER `citation_library`. The whole point of the table.
    #[test]
    fn staging_never_writes_to_the_users_library() {
        let (db, job) = db_with_job();
        stage_entries(
            &db,
            job,
            &[entry("a", 2020, Some("10.1/x"), "A, A. (2020). Title.")],
            1,
        )
        .unwrap();
        let n: i64 = db
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM citation_library", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "staging put a row in the user's library");
    }

    #[test]
    fn only_doi_bearing_unfetched_sources_are_fetchable() {
        let (db, job) = db_with_job();
        stage_entries(
            &db,
            job,
            &[
                entry("withdoi", 2020, Some("10.1/x"), "WithDoi, A. (2020). T."),
                entry("nodoi", 2021, None, "NoDoi, B. (2021). T."),
            ],
            1,
        )
        .unwrap();
        assert_eq!(fetchable_by_doi(&db, job).unwrap().len(), 1);

        // Once fetched, it drops out of the fetchable set.
        let staged = list_for_job(&db, job).unwrap();
        let with_doi = staged.iter().find(|s| s.doi.is_some()).unwrap();
        db.conn()
            .unwrap()
            .execute(
                "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
                 VALUES ('paper', 'T', 'u', 1, 'ck', 'ingested', 1)",
                [],
            )
            .unwrap();
        let doc = db.conn().unwrap().last_insert_rowid();
        link_document(&db, with_doi.id, doc, "doi").unwrap();
        assert!(fetchable_by_doi(&db, job).unwrap().is_empty());
        let after = list_for_job(&db, job).unwrap();
        let linked = after.iter().find(|s| s.id == with_doi.id).unwrap();
        assert_eq!(linked.document_id, Some(doc));
        assert_eq!(linked.matched_by.as_deref(), Some("doi"));
    }

    /// Scoped to the manuscript: deleting the job takes its staged rows.
    #[test]
    fn staged_rows_are_scoped_to_the_job_and_cascade_with_it() {
        let (db, job) = db_with_job();
        stage_entries(&db, job, &[entry("a", 2020, Some("10.1/x"), "A, A. (2020). T.")], 1)
            .unwrap();
        let conn = db.conn().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        conn.execute("DELETE FROM ai_jobs WHERE id = ?1", params![job]).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_staged_sources", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "staged rows outlived their job");
    }
}
