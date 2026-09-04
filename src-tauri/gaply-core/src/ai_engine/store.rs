//! Persistence for `ai_chunks` (migration v14).
//!
//! Deterministic and fully local, following [`crate::citation_library`]: the
//! Tauri command is a thin app-crate wrapper, and no logic lives there.
//!
//! # Re-index idempotency
//!
//! `content_hash` is the sha256 of the chunk's text, and the table carries
//! `UNIQUE (document_id, content_hash)`. Indexing the same document twice
//! therefore inserts nothing the second time — the guarantee is a database
//! constraint, not a caller convention, so a future caller cannot lose it.
//!
//! # Pages are recorded, never inferred
//!
//! `page` is `Option<u32>` end to end and stores SQL NULL when the source had no
//! reliable page boundaries. Nothing in this module derives a page from position
//! in the text.

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::chunk::PagedChunk;
use crate::db::Database;
use crate::error::GaplyError;
use crate::now_epoch;

const COLS: &str = "id, document_id, page, section, char_start, char_end, content, \
                    token_estimate, content_hash, created_at";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StoredChunk {
    pub id: i64,
    pub document_id: i64,
    /// NULL in SQL when the source carried no page boundaries.
    pub page: Option<u32>,
    pub section: Option<String>,
    /// UTF-8 byte offsets into the document text (plan §11 D4).
    pub char_start: i64,
    pub char_end: i64,
    pub content: String,
    pub token_estimate: i64,
    pub content_hash: String,
    pub created_at: i64,
}

/// What one indexing pass actually did. `inserted + duplicates == submitted`,
/// so a caller can never report "indexed N" for a pass that stored fewer.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
pub struct IndexOutcome {
    pub submitted: usize,
    pub inserted: usize,
    /// Chunks already present under the same (document_id, content_hash).
    pub duplicates: usize,
}

/// Index-state summary for one document.
///
/// `with_page` / `without_page` are reported separately and never merged: a
/// document whose pages could not be read is a materially different thing from
/// one that was never indexed, and the Citation Support Checker needs to tell
/// them apart before it trusts a page-linked quote.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct IndexStatus {
    pub document_id: i64,
    pub chunk_count: i64,
    pub with_page: i64,
    pub without_page: i64,
    pub max_page: Option<u32>,
    pub sections: Vec<String>,
}

/// sha256 of the chunk text — the idempotency key.
pub fn content_hash(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("{:x}", h.finalize())
}

fn row_to_chunk(row: &rusqlite::Row) -> rusqlite::Result<StoredChunk> {
    Ok(StoredChunk {
        id: row.get(0)?,
        document_id: row.get(1)?,
        page: row.get::<_, Option<i64>>(2)?.map(|p| p as u32),
        section: row.get(3)?,
        char_start: row.get(4)?,
        char_end: row.get(5)?,
        content: row.get(6)?,
        token_estimate: row.get(7)?,
        content_hash: row.get(8)?,
        created_at: row.get(9)?,
    })
}

/// Persist a document's chunks. Idempotent: re-indexing inserts nothing.
///
/// The document must already exist in `documents` — the FK is enforced, so a
/// bad `document_id` is refused rather than orphaning rows.
pub fn index_chunks(
    db: &Database,
    document_id: i64,
    chunks: &[PagedChunk],
) -> Result<IndexOutcome, GaplyError> {
    let now = now_epoch();
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    let mut outcome = IndexOutcome { submitted: chunks.len(), ..Default::default() };
    {
        // ON CONFLICT DO NOTHING against UNIQUE(document_id, content_hash):
        // the re-index guarantee is the constraint, not this call site.
        let mut stmt = tx.prepare(
            "INSERT INTO ai_chunks
                 (document_id, page, section, char_start, char_end, content,
                  token_estimate, content_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (document_id, content_hash) DO NOTHING",
        )?;
        for c in chunks {
            let n = stmt.execute(params![
                document_id,
                c.page.map(i64::from),
                c.section.as_deref(),
                c.char_start as i64,
                c.char_end as i64,
                c.content,
                c.token_estimate as i64,
                content_hash(&c.content),
                now,
            ])?;
            if n == 0 {
                outcome.duplicates += 1;
            } else {
                outcome.inserted += 1;
            }
        }
    }
    tx.commit()?;
    Ok(outcome)
}

/// Every chunk of a document, in stored order.
pub fn list_chunks(db: &Database, document_id: i64) -> Result<Vec<StoredChunk>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM ai_chunks WHERE document_id = ?1 ORDER BY id"
    ))?;
    let rows = stmt.query_map(params![document_id], row_to_chunk)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Chunks on one page. `None` selects the page-less chunks specifically —
/// `IS NULL` rather than `= NULL`, so "no page recorded" is a queryable state.
pub fn chunks_on_page(
    db: &Database,
    document_id: i64,
    page: Option<u32>,
) -> Result<Vec<StoredChunk>, GaplyError> {
    let conn = db.conn()?;
    let sql = format!(
        "SELECT {COLS} FROM ai_chunks
         WHERE document_id = ?1 AND page IS ?2
         ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![document_id, page.map(i64::from)], row_to_chunk)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Full chunks for specific ids, returned in the ORDER THE CALLER ASKED FOR.
///
/// Retrieval hands back ids ranked by relevance, and that order is what decides
/// which chunks survive an evidence budget. Returning them in id order instead
/// would silently drop the wrong ones.
pub fn chunks_by_ids(db: &Database, ids: &[i64]) -> Result<Vec<StoredChunk>, GaplyError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let conn = db.conn()?;
    let sql = format!("SELECT {COLS} FROM ai_chunks WHERE id IN ({placeholders})");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), row_to_chunk)?;
    let mut by_id = std::collections::HashMap::new();
    for r in rows {
        let c = r?;
        by_id.insert(c.id, c);
    }
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

/// Summarise what is indexed for a document.
pub fn index_status(db: &Database, document_id: i64) -> Result<IndexStatus, GaplyError> {
    let conn = db.conn()?;
    let (chunk_count, with_page, without_page, max_page): (i64, i64, i64, Option<i64>) = conn
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN page IS NOT NULL THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN page IS NULL THEN 1 ELSE 0 END), 0),
                    MAX(page)
             FROM ai_chunks WHERE document_id = ?1",
            params![document_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT section FROM ai_chunks
         WHERE document_id = ?1 AND section IS NOT NULL ORDER BY section",
    )?;
    let sections = stmt
        .query_map(params![document_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(IndexStatus {
        document_id,
        chunk_count,
        with_page,
        without_page,
        max_page: max_page.map(|p| p as u32),
        sections,
    })
}

/// Create (or adopt) a `documents` row for a local file.
///
/// Phase 1 recorded that nothing created a document row for a local paper
/// (plan §11 D5); this is that missing piece. `checksum` is UNIQUE in the
/// schema, so re-ingesting the same content adopts the existing row instead of
/// failing — the same dedupe-by-provenance rule `rag::ingest_document` uses.
pub fn create_document(
    db: &Database,
    title: &str,
    source_path: &str,
    checksum: &str,
) -> Result<i64, GaplyError> {
    let conn = db.conn()?;
    conn.execute(
        "INSERT INTO documents
             (source_type, title, source_url, fetched_at, checksum, status, created_at)
         VALUES ('paper', ?1, ?2, ?3, ?4, 'ingested', ?3)
         ON CONFLICT (checksum) DO NOTHING",
        params![title, source_path, now_epoch(), checksum],
    )?;
    Ok(conn.query_row(
        "SELECT id FROM documents WHERE checksum = ?1",
        params![checksum],
        |r| r.get(0),
    )?)
}

/// Create a document that holds an ABSTRACT rather than a full text.
///
/// Same row shape as [`create_document`] plus the v18 flag, and it is a
/// separate function on purpose: the flag is the only thing standing between a
/// 250-word summary and a verdict presented as if the paper had been read, so
/// setting it must be a decision at the call site rather than an argument
/// someone can forget to pass.
pub fn create_abstract_document(
    db: &Database,
    title: &str,
    source_url: &str,
    checksum: &str,
) -> Result<i64, GaplyError> {
    let conn = db.conn()?;
    conn.execute(
        "INSERT INTO documents
             (source_type, title, source_url, fetched_at, checksum, status, created_at, abstract_only)
         VALUES ('paper', ?1, ?2, ?3, ?4, 'ingested', ?3, 1)
         ON CONFLICT (checksum) DO UPDATE SET abstract_only = 1",
        params![title, source_url, now_epoch(), checksum],
    )?;
    Ok(conn.query_row(
        "SELECT id FROM documents WHERE checksum = ?1",
        params![checksum],
        |r| r.get(0),
    )?)
}

/// Whether this document is an abstract rather than the paper (v18).
///
/// A missing document answers `false` rather than erroring: the caller asking
/// this question is deciding how much to trust a verdict, and the safe reading
/// of "I cannot find that row" is not "assume it was a full text" — but the row
/// is always present in every path that calls this (the verdict is about a
/// document that was just retrieved from), so a `false` here means exactly what
/// it says.
pub fn is_abstract_only(db: &Database, document_id: i64) -> Result<bool, GaplyError> {
    let conn = db.conn()?;
    let flag: Option<i64> = conn
        .query_row(
            "SELECT abstract_only FROM documents WHERE id = ?1",
            params![document_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(flag.unwrap_or(0) != 0)
}

/// The recorded source of a document — its `source_url`, which for a locally
/// ingested paper is its path.
///
/// An unknown id, or a row with no recorded source, is an honest error rather
/// than an empty string that would later fail as a confusing "file not found".
pub fn document_source(db: &Database, document_id: i64) -> Result<String, GaplyError> {
    let conn = db.conn()?;
    let source: Option<String> = conn
        .query_row(
            "SELECT source_url FROM documents WHERE id = ?1",
            params![document_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    match source {
        Some(s) if !s.trim().is_empty() => Ok(s),
        Some(_) => Err(GaplyError::Validation(format!(
            "document {document_id} has no recorded source path — pass one explicitly"
        ))),
        None => Err(GaplyError::NotFound { entity: "document", id: document_id.to_string() }),
    }
}

/// Drop a document's chunks so it can be re-indexed from scratch (a re-parse
/// after an extraction fix, say). Returns the number removed.
pub fn clear_chunks(db: &Database, document_id: i64) -> Result<usize, GaplyError> {
    Ok(db
        .conn()?
        .execute("DELETE FROM ai_chunks WHERE document_id = ?1", params![document_id])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::extract::docparse::PagedBlock;

    fn db_with_document() -> (Database, i64) {
        let db = Database::in_memory().expect("in-memory db");
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
             VALUES ('paper', 'Doc', '', 1, 'sum-1', 'ingested', 1)",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        drop(conn);
        (db, id)
    }

    fn blocks() -> Vec<PagedBlock> {
        vec![
            PagedBlock { page: Some(1), style: None, text: "Methods".into() },
            PagedBlock { page: Some(1), style: None, text: "We did the thing. Then we measured it.".into() },
            PagedBlock { page: Some(2), style: None, text: "The thing worked. Numbers followed.".into() },
        ]
    }

    #[test]
    fn indexing_stores_page_section_and_span() {
        let (db, doc) = db_with_document();
        let chunks = crate::chunk::chunk_paged(&blocks(), 100, 10);
        let outcome = index_chunks(&db, doc, &chunks).unwrap();
        assert_eq!(outcome.inserted, chunks.len());
        assert_eq!(outcome.duplicates, 0);

        let stored = list_chunks(&db, doc).unwrap();
        assert_eq!(stored.len(), chunks.len());
        assert_eq!(stored[0].page, Some(1));
        assert_eq!(stored[0].section.as_deref(), Some("Methods"));
        assert_eq!(stored[1].page, Some(2));
        // the span round-trips and still re-slices the document text
        let doc_text = crate::chunk::document_text(&blocks());
        for s in &stored {
            assert_eq!(&doc_text[s.char_start as usize..s.char_end as usize], s.content);
        }
    }

    #[test]
    fn re_indexing_the_same_document_creates_no_duplicates() {
        let (db, doc) = db_with_document();
        let chunks = crate::chunk::chunk_paged(&blocks(), 100, 10);

        let first = index_chunks(&db, doc, &chunks).unwrap();
        assert_eq!(first.inserted, chunks.len());
        let after_first = list_chunks(&db, doc).unwrap().len();

        // Same input again — every row collides on (document_id, content_hash).
        let second = index_chunks(&db, doc, &chunks).unwrap();
        assert_eq!(second.inserted, 0, "re-index inserted rows");
        assert_eq!(second.duplicates, chunks.len());
        assert_eq!(second.submitted, chunks.len());
        assert_eq!(list_chunks(&db, doc).unwrap().len(), after_first, "row count grew on re-index");

        // A third pass is likewise a no-op.
        index_chunks(&db, doc, &chunks).unwrap();
        assert_eq!(list_chunks(&db, doc).unwrap().len(), after_first);
    }

    #[test]
    fn identical_text_on_two_documents_is_not_a_duplicate() {
        // The uniqueness key is (document_id, content_hash): two papers sharing a
        // boilerplate sentence must both keep it.
        let (db, doc_a) = db_with_document();
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
             VALUES ('paper', 'Other', '', 1, 'sum-2', 'ingested', 1)",
            [],
        )
        .unwrap();
        let doc_b = conn.last_insert_rowid();
        drop(conn);

        let chunks = crate::chunk::chunk_paged(&blocks(), 100, 10);
        index_chunks(&db, doc_a, &chunks).unwrap();
        let b = index_chunks(&db, doc_b, &chunks).unwrap();
        assert_eq!(b.inserted, chunks.len(), "a second document's chunks were dropped as duplicates");
    }

    #[test]
    fn page_less_chunks_store_null_and_are_queryable_as_such() {
        let (db, doc) = db_with_document();
        let blocks = vec![PagedBlock { page: None, style: None, text: "No pages here. Two sentences.".into() }];
        let chunks = crate::chunk::chunk_paged(&blocks, 100, 10);
        index_chunks(&db, doc, &chunks).unwrap();

        let stored = list_chunks(&db, doc).unwrap();
        assert!(stored.iter().all(|c| c.page.is_none()), "a page was invented");

        // IS NULL, not = NULL — "no page recorded" is a state you can select.
        assert_eq!(chunks_on_page(&db, doc, None).unwrap().len(), stored.len());
        assert!(chunks_on_page(&db, doc, Some(1)).unwrap().is_empty());

        let st = index_status(&db, doc).unwrap();
        assert_eq!(st.with_page, 0);
        assert_eq!(st.without_page, stored.len() as i64);
        assert_eq!(st.max_page, None);
    }

    #[test]
    fn index_status_separates_paged_from_page_less() {
        let (db, doc) = db_with_document();
        let chunks = crate::chunk::chunk_paged(&blocks(), 100, 10);
        index_chunks(&db, doc, &chunks).unwrap();
        let st = index_status(&db, doc).unwrap();
        assert_eq!(st.chunk_count, chunks.len() as i64);
        assert_eq!(st.with_page, chunks.len() as i64);
        assert_eq!(st.without_page, 0);
        assert_eq!(st.max_page, Some(2));
        assert_eq!(st.sections, vec!["Methods".to_string()]);
    }

    #[test]
    fn an_unknown_document_id_is_refused_not_orphaned() {
        let (db, _doc) = db_with_document();
        let chunks = crate::chunk::chunk_paged(&blocks(), 100, 10);
        assert!(index_chunks(&db, 999_999, &chunks).is_err(), "orphan chunks were accepted");
    }

    #[test]
    fn clearing_allows_a_clean_re_index() {
        let (db, doc) = db_with_document();
        let chunks = crate::chunk::chunk_paged(&blocks(), 100, 10);
        index_chunks(&db, doc, &chunks).unwrap();
        let removed = clear_chunks(&db, doc).unwrap();
        assert_eq!(removed, chunks.len());
        assert!(list_chunks(&db, doc).unwrap().is_empty());
        let again = index_chunks(&db, doc, &chunks).unwrap();
        assert_eq!(again.inserted, chunks.len());
    }
}
