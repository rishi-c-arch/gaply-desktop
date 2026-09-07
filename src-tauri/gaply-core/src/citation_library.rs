//! Citation Manager — the LOCAL-FIRST reference library (Set 4).
//!
//! Local sqlite is the SOURCE OF TRUTH: add/list/search/tag work fully
//! offline with zero connectivity and zero sign-in. The existing Supabase
//! sync is an OPTIONAL layer the frontend drives on top (push/pull when
//! signed-in + online), tracked honestly via `sync_status` — local data is
//! never lost to a failed sync.
//!
//! Placement note (flagged at review): CRUD lives in gaply-core because
//! [`Database::conn`] is deliberately crate-private — this module is the
//! exact [`crate::projects`] precedent (portable, deterministic SQL; no
//! network, no ML, no LLM). The Tauri commands are thin app-crate wrappers.
//!
//! Anti-hallucination carry-through: the search columns (title/authors/year)
//! are DERIVED from the stored verified CSL-JSON at write time — parsed,
//! never invented; absent fields stay empty.

use rusqlite::params;
use serde::Serialize;

use crate::{now_epoch, Database, GaplyError};

/// Honest sync markers. The frontend sets 'synced'/'pending' after its
/// optional Supabase push; every local mutation resets to 'local_only'.
pub const SYNC_STATUSES: &[&str] = &["local_only", "pending", "synced"];

#[derive(Debug, Clone, Serialize)]
pub struct StoredReference {
    pub id: String,
    /// The verified CSL-JSON (Set 2's shape), verbatim.
    pub csl_json: String,
    pub doi: Option<String>,
    /// Search columns — derived from csl_json, never invented.
    pub title: String,
    pub authors: String,
    pub year: Option<i64>,
    pub tags: Vec<String>,
    /// Scope B: persisted verification + retraction facts (EXTERNAL — from
    /// refverify/CrossRef/Retraction Watch — NOT derived from csl_json). These
    /// survive a reload so a retracted paper never reads clean after restart and
    /// a verified entry stays verified.
    pub retracted: bool,
    pub source: Option<String>,
    pub verify_provenance: Vec<String>,
    pub verify_outcome: Option<String>,
    pub verified_at: Option<i64>,
    /// §11 D102. The OUTCOME of a retraction check — 'clear' / 'check_failed',
    /// NULL when one was never attempted. `retracted` above is the separate
    /// confirmed-retraction fact and outranks this.
    pub retraction_outcome: Option<String>,
    /// When that outcome was established. A persisted 'clear' is a claim with
    /// an invisible expiry without it.
    pub retraction_checked_at: Option<i64>,
    pub sync_status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The external verification/retraction facts written alongside a reference
/// (Scope B). Default = a fresh, unverified, not-retracted entry — what every
/// pre-Scope-B call site wants, so those pass `&Default::default()`.
#[derive(Debug, Clone, Default)]
pub struct VerificationWrite {
    pub retracted: bool,
    pub source: Option<String>,
    pub verify_provenance: Vec<String>,
    pub verify_outcome: Option<String>,
    pub verified_at: Option<i64>,
    /// §11 D102. 'clear' | 'check_failed' | None (never attempted).
    pub retraction_outcome: Option<String>,
    pub retraction_checked_at: Option<i64>,
}

fn row_to_reference(row: &rusqlite::Row) -> rusqlite::Result<StoredReference> {
    let tags_json: String = row.get(6)?;
    let prov_json: Option<String> = row.get(12)?;
    Ok(StoredReference {
        id: row.get(0)?,
        csl_json: row.get(1)?,
        doi: row.get(2)?,
        title: row.get(3)?,
        authors: row.get(4)?,
        year: row.get(5)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        sync_status: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        // Scope B columns (10..14). NULL/absent → not-retracted, unverified.
        retracted: row.get::<_, i64>(10)? != 0,
        source: row.get(11)?,
        verify_provenance: prov_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
        verify_outcome: row.get(13)?,
        verified_at: row.get(14)?,
        // Migration 19 columns (15..16). NULL → never checked.
        retraction_outcome: row.get(15)?,
        retraction_checked_at: row.get(16)?,
    })
}

const COLS: &str =
    "id, csl_json, doi, title, authors, year, tags, sync_status, created_at, updated_at, \
     retracted, source, verify_provenance, verify_outcome, verified_at, \
     retraction_outcome, retraction_checked_at";

/// Derive the search columns from the verified CSL-JSON. Absent → empty/None
/// (parsed, never invented).
fn search_columns(csl_json: &str) -> (String, String, Option<i64>, Option<String>) {
    let v: serde_json::Value = serde_json::from_str(csl_json).unwrap_or(serde_json::Value::Null);
    let title = v["title"].as_str().unwrap_or("").to_string();
    let authors = v["author"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let family = a["family"].as_str()?;
                    Some(match a["given"].as_str() {
                        Some(g) => format!("{family}, {g}"),
                        None => family.to_string(),
                    })
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();
    let year = v["issued"]["date-parts"][0][0]
        .as_i64()
        .or_else(|| v["issued"]["year"].as_i64());
    let doi = v["DOI"].as_str().map(str::to_string);
    (title, authors, year, doi)
}

/// Add or update a reference. Any mutation honestly resets `sync_status` to
/// 'local_only' — the optional sync layer re-marks it after a real push.
pub fn upsert(
    db: &Database,
    id: &str,
    csl_json: &str,
    doi: Option<&str>,
    tags: &[String],
    verify: &VerificationWrite,
) -> Result<StoredReference, GaplyError> {
    if id.trim().is_empty() {
        return Err(GaplyError::Validation("reference id must not be empty".into()));
    }
    serde_json::from_str::<serde_json::Value>(csl_json)
        .map_err(|e| GaplyError::Validation(format!("csl_json is not valid JSON: {e}")))?;
    let (title, authors, year, doi_from_json) = search_columns(csl_json);
    let doi = doi.map(str::to_string).or(doi_from_json);
    let tags_json = serde_json::to_string(tags)
        .map_err(|e| GaplyError::Internal(format!("tags serialize: {e}")))?;
    // Provenance persists as a JSON array string; empty → "[]".
    let prov_json = serde_json::to_string(&verify.verify_provenance)
        .map_err(|e| GaplyError::Internal(format!("provenance serialize: {e}")))?;
    let now = now_epoch();
    db.conn()?.execute(
        "INSERT INTO citation_library
             (id, csl_json, doi, title, authors, year, tags, sync_status, created_at, updated_at,
              retracted, source, verify_provenance, verify_outcome, verified_at,
              retraction_outcome, retraction_checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'local_only', ?8, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(id) DO UPDATE SET
             csl_json = excluded.csl_json,
             doi = excluded.doi,
             title = excluded.title,
             authors = excluded.authors,
             year = excluded.year,
             tags = excluded.tags,
             sync_status = 'local_only',
             updated_at = excluded.updated_at,
             retracted = excluded.retracted,
             source = excluded.source,
             verify_provenance = excluded.verify_provenance,
             verify_outcome = excluded.verify_outcome,
             verified_at = excluded.verified_at,
             retraction_outcome = excluded.retraction_outcome,
             retraction_checked_at = excluded.retraction_checked_at",
        params![
            id, csl_json, doi, title, authors, year, tags_json, now,
            verify.retracted as i64, verify.source, prov_json, verify.verify_outcome, verify.verified_at,
            verify.retraction_outcome, verify.retraction_checked_at
        ],
    )?;
    get(db, id)?.ok_or_else(|| GaplyError::Internal("upsert lost the row".into()))
}

pub fn get(db: &Database, id: &str) -> Result<Option<StoredReference>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt =
        conn.prepare(&format!("SELECT {COLS} FROM citation_library WHERE id = ?1"))?;
    let mut rows = stmt.query_map(params![id], row_to_reference)?;
    Ok(rows.next().transpose()?)
}

/// Everything in the library, newest first. Fully local.
pub fn list(db: &Database) -> Result<Vec<StoredReference>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt =
        conn.prepare(&format!("SELECT {COLS} FROM citation_library ORDER BY updated_at DESC"))?;
    let rows = stmt.query_map([], row_to_reference)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Deterministic local search over title / authors / DOI / year / tags.
/// `tag` additionally filters to references carrying that exact tag.
pub fn search(
    db: &Database,
    query: &str,
    tag: Option<&str>,
) -> Result<Vec<StoredReference>, GaplyError> {
    let q = query.trim();
    let like = format!("%{}%", q.replace('%', "").replace('_', ""));
    // exact-tag containment via the JSON-array encoding: ["a","b"]
    let tag_like = tag.map(|t| format!("%\"{}\"%", t.replace('"', "")));
    let conn = db.conn()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM citation_library
         WHERE (?1 = '' OR title LIKE ?2 COLLATE NOCASE
                OR authors LIKE ?2 COLLATE NOCASE
                OR IFNULL(doi, '') LIKE ?2
                OR CAST(IFNULL(year, '') AS TEXT) LIKE ?2
                OR tags LIKE ?2 COLLATE NOCASE)
           AND (?3 IS NULL OR tags LIKE ?3)
         ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map(params![q, like, tag_like], row_to_reference)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Replace a reference's tags (add/remove are set operations on the caller's
/// side; storage takes the full list). Resets sync_status honestly.
pub fn set_tags(db: &Database, id: &str, tags: &[String]) -> Result<StoredReference, GaplyError> {
    let tags_json = serde_json::to_string(tags)
        .map_err(|e| GaplyError::Internal(format!("tags serialize: {e}")))?;
    let n = db.conn()?.execute(
        "UPDATE citation_library
         SET tags = ?2, sync_status = 'local_only', updated_at = ?3
         WHERE id = ?1",
        params![id, tags_json, now_epoch()],
    )?;
    if n == 0 {
        return Err(GaplyError::NotFound { entity: "citation", id: id.to_string() });
    }
    get(db, id)?.ok_or_else(|| GaplyError::Internal("set_tags lost the row".into()))
}

pub fn delete(db: &Database, id: &str) -> Result<(), GaplyError> {
    db.conn()?.execute("DELETE FROM citation_library WHERE id = ?1", params![id])?;
    Ok(())
}

/// The optional sync layer's honest marker — only 'local_only' | 'pending' |
/// 'synced' are accepted.
pub fn set_sync_status(db: &Database, id: &str, status: &str) -> Result<(), GaplyError> {
    if !SYNC_STATUSES.contains(&status) {
        return Err(GaplyError::Validation(format!(
            "unknown sync_status {status:?} (expected local_only, pending or synced)"
        )));
    }
    let n = db.conn()?.execute(
        "UPDATE citation_library SET sync_status = ?2 WHERE id = ?1",
        params![id, status],
    )?;
    if n == 0 {
        return Err(GaplyError::NotFound { entity: "citation", id: id.to_string() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATSON_CSL: &str = r#"{"id":"wc1953","type":"article-journal",
        "title":"Molecular Structure of Nucleic Acids",
        "author":[{"family":"Watson","given":"J. D."},{"family":"Crick","given":"F. H. C."}],
        "issued":{"date-parts":[[1953]]},
        "DOI":"10.1038/171737a0","container-title":"Nature",
        "volume":"171","page":"737-738"}"#;

    const SPARSE_CSL: &str = r#"{"id":"sp1","type":"article",
        "title":"A Sparse Preprint","author":[{"family":"Lovelace","given":"Ada"}],
        "issued":{"date-parts":[[2024]]}}"#;

    fn db() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn migration_applies_and_the_table_exists() {
        let d = db();
        // in_memory() runs migrations; prove the table by using it.
        assert!(list(&d).unwrap().is_empty());
        let conn_check = upsert(&d, "wc1953", WATSON_CSL, None, &[], &Default::default());
        assert!(conn_check.is_ok());
    }

    #[test]
    fn add_list_search_tag_fully_local() {
        let d = db();
        upsert(&d, "wc1953", WATSON_CSL, None, &["dna".into()], &Default::default()).unwrap();
        upsert(&d, "sp1", SPARSE_CSL, None, &[], &Default::default()).unwrap();

        // list
        let all = list(&d).unwrap();
        assert_eq!(all.len(), 2);

        // search columns DERIVED from the verified CSL-JSON
        let w = get(&d, "wc1953").unwrap().unwrap();
        assert_eq!(w.title, "Molecular Structure of Nucleic Acids");
        assert!(w.authors.contains("Watson, J. D."));
        assert_eq!(w.year, Some(1953));
        assert_eq!(w.doi.as_deref(), Some("10.1038/171737a0"), "DOI derived from csl_json");

        // search by title / author / year / DOI / tag
        assert_eq!(search(&d, "nucleic", None).unwrap().len(), 1);
        assert_eq!(search(&d, "lovelace", None).unwrap().len(), 1);
        assert_eq!(search(&d, "1953", None).unwrap().len(), 1);
        assert_eq!(search(&d, "10.1038/171737a0", None).unwrap().len(), 1);
        assert_eq!(search(&d, "dna", None).unwrap().len(), 1);
        assert_eq!(search(&d, "", Some("dna")).unwrap().len(), 1, "tag filter");
        assert_eq!(search(&d, "zebrafish", None).unwrap().len(), 0);

        // tag management
        let tagged = set_tags(&d, "sp1", &["preprint".into(), "toread".into()]).unwrap();
        assert_eq!(tagged.tags, vec!["preprint", "toread"]);
        assert_eq!(search(&d, "", Some("toread")).unwrap().len(), 1);
        let untagged = set_tags(&d, "sp1", &[]).unwrap();
        assert!(untagged.tags.is_empty());
    }

    #[test]
    fn sparse_csl_derives_empty_search_columns_never_invented() {
        let d = db();
        let r = upsert(&d, "sp1", SPARSE_CSL, None, &[], &Default::default()).unwrap();
        assert_eq!(r.doi, None, "no DOI in the JSON → no DOI column, not a guess");
        assert_eq!(r.year, Some(2024));
    }

    #[test]
    fn mutations_reset_sync_status_and_markers_are_honest() {
        let d = db();
        let r = upsert(&d, "wc1953", WATSON_CSL, None, &[], &Default::default()).unwrap();
        assert_eq!(r.sync_status, "local_only", "new refs are honestly local-only");

        set_sync_status(&d, "wc1953", "synced").unwrap();
        assert_eq!(get(&d, "wc1953").unwrap().unwrap().sync_status, "synced");

        // an edit makes 'synced' a lie — the row resets to local_only
        upsert(&d, "wc1953", WATSON_CSL, None, &["edited".into()], &Default::default()).unwrap();
        assert_eq!(get(&d, "wc1953").unwrap().unwrap().sync_status, "local_only");

        set_sync_status(&d, "wc1953", "pending").unwrap();
        set_tags(&d, "wc1953", &[]).unwrap();
        assert_eq!(get(&d, "wc1953").unwrap().unwrap().sync_status, "local_only");

        assert!(set_sync_status(&d, "wc1953", "faked").is_err(), "unknown status refused");
        assert!(set_sync_status(&d, "missing", "synced").is_err());
    }

    #[test]
    fn delete_and_invalid_input_paths() {
        let d = db();
        upsert(&d, "wc1953", WATSON_CSL, None, &[], &Default::default()).unwrap();
        delete(&d, "wc1953").unwrap();
        assert!(list(&d).unwrap().is_empty());
        assert!(upsert(&d, " ", WATSON_CSL, None, &[], &Default::default()).is_err());
        assert!(upsert(&d, "x", "not json", None, &[], &Default::default()).is_err());
        assert!(set_tags(&d, "missing", &[]).is_err());
    }

    #[test]
    fn scope_b_verification_and_retraction_round_trip() {
        let d = db();
        let verify = VerificationWrite {
            retracted: true,
            source: Some("doi".into()),
            verify_provenance: vec!["crossref:https://api.crossref.org/works/x".into()],
            verify_outcome: None,
            verified_at: Some(1234),
            retraction_outcome: None,
            retraction_checked_at: None,
        };
        upsert(&d, "wc1953", WATSON_CSL, None, &[], &verify).unwrap();

        // The facts survive the write → read round-trip, via both get() and list().
        let r = get(&d, "wc1953").unwrap().unwrap();
        assert!(r.retracted, "retracted MUST survive — a retracted paper cannot read clean");
        assert_eq!(r.source.as_deref(), Some("doi"));
        assert_eq!(r.verify_provenance, vec!["crossref:https://api.crossref.org/works/x".to_string()]);
        assert_eq!(r.verify_outcome, None);
        assert_eq!(r.verified_at, Some(1234));

        let from_list = list(&d).unwrap().into_iter().find(|x| x.id == "wc1953").unwrap();
        assert!(from_list.retracted);
        assert_eq!(from_list.verify_provenance.len(), 1);

        // A default (unverified, not-retracted) upsert reads back honest.
        upsert(&d, "sp1", SPARSE_CSL, None, &[], &Default::default()).unwrap();
        let plain = get(&d, "sp1").unwrap().unwrap();
        assert!(!plain.retracted);
        assert!(plain.verify_provenance.is_empty());
        assert_eq!((plain.verify_outcome, plain.verified_at), (None, None));
        // Never checked reads as never checked — NOT as clear (§11 D102).
        assert_eq!((plain.retraction_outcome, plain.retraction_checked_at), (None, None));
    }

    /// §11 D102. Before this, a checked-and-clean entry reverted to "not
    /// checked for retraction" on every restart: `retracted` was durable but
    /// the OUTCOME was not.
    #[test]
    fn retraction_outcome_survives_the_round_trip() {
        let d = db();
        let checked = VerificationWrite {
            retraction_outcome: Some("clear".into()),
            retraction_checked_at: Some(1_757_030_400),
            ..Default::default()
        };
        upsert(&d, "wc1953", WATSON_CSL, None, &[], &checked).unwrap();
        let r = get(&d, "wc1953").unwrap().unwrap();
        assert_eq!(r.retraction_outcome.as_deref(), Some("clear"));
        assert_eq!(r.retraction_checked_at, Some(1_757_030_400));
        assert!(!r.retracted, "a clear check is not a retraction");

        // list() reads the same columns — the page loads through it.
        let from_list = list(&d).unwrap().into_iter().find(|x| x.id == "wc1953").unwrap();
        assert_eq!(from_list.retraction_outcome.as_deref(), Some("clear"));
        assert_eq!(from_list.retraction_checked_at, Some(1_757_030_400));

        // A failed check is a THIRD state, distinct from clear and from never.
        upsert(
            &d,
            "wc1953",
            WATSON_CSL,
            None,
            &[],
            &VerificationWrite {
                retraction_outcome: Some("check_failed".into()),
                retraction_checked_at: Some(1_757_116_800),
                ..Default::default()
            },
        )
        .unwrap();
        let r = get(&d, "wc1953").unwrap().unwrap();
        assert_eq!(r.retraction_outcome.as_deref(), Some("check_failed"));

        // The vocabulary is held at the SCHEMA: a typo cannot become a fourth
        // silent state that renders as neither checked nor unchecked.
        let bad = upsert(
            &d,
            "wc1953",
            WATSON_CSL,
            None,
            &[],
            &VerificationWrite {
                retraction_outcome: Some("cleared".into()),
                ..Default::default()
            },
        );
        assert!(bad.is_err(), "an unknown retraction outcome was accepted");

        // And that refusal did not corrupt the row it failed on.
        let after = get(&d, "wc1953").unwrap().unwrap();
        assert_eq!(after.retraction_outcome.as_deref(), Some("check_failed"));
    }
}
