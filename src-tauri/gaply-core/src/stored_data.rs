//! **What Gaply's older features keep on this computer, counted (Research
//! OS, Phase 5D-1a; decisions 109, 205 to 207), and deleted one category at a
//! time (Phase 5D-1b; decisions 210 and 213 to 217).**
//!
//! The Stored data section in Settings shows nine categories, each with how
//! many items it holds and how many bytes, and a Delete for each. The summary
//! only reads: it runs `SELECT`s, inside one deferred transaction so the nine
//! counts come from one snapshot, and it reads file sizes from metadata, never
//! a file's contents.
//!
//! The view carries counts and bytes only: no title, name, path, URL or any
//! value of a counted row or file.
//!
//! **The nine categories** (`~/gaply-corpus/os/phase5/stored_data_map.md`
//! has each one's writers, readers and foreign keys):
//!
//! 1. Manuscript analyses: `manuscripts`, with the `extractions` and
//!    `findings` that cascade from it.
//! 2. Gap Finder papers: `documents` whose `source_type` is
//!    `research_paper` and whose `source_url` starts `gapfinder://`, their
//!    `chunks`, and the `embeddings` rows whose source is one of those
//!    chunks. `embeddings` is a `vec0` virtual table and holds no foreign key,
//!    so no cascade reaches it: its rows are found by `source_id`.
//! 3. Thesis audits: `ai_jobs`, with the `ai_job_items` and
//!    `audit_staged_sources` that cascade from it, and nothing else.
//! 4. Cached reports: `cache` rows whose key starts `report:` and that have
//!    not expired.
//! 5. Expired cache rows: every `cache` row, of any key, with
//!    `created_at + ttl_seconds <= now`. An expired `report:` row counts here,
//!    never in 4.
//! 6. The comparison log: the one file `box4_comparisons.jsonl`.
//! 7. Downloaded open-access papers: the PDFs and `-abstract.txt` files in
//!    `oa_papers/`.
//! 8. Temporary exports: the AI Check and PublishReady export files in the
//!    temp folder, by the names their commands give them.
//! 9. Unused note images: a file in `note-images/` whose
//!    `gaply-image://<name>` appears in no note, asked through
//!    [`crate::notes::search_notes`], the same search the Note Creator's own
//!    clean-up uses, and that has not changed in the last 10 minutes
//!    ([`RECENT_IMAGE_SECS`], decisions 210 and 217): an image is written at
//!    paste, before its note is saved, so a new one looks unused for a moment.
//!    The count and the delete leave such an image out alike.
//!
//! **Bytes.** For rows, the sum of `length(CAST(col AS BLOB))` over the TEXT
//! and BLOB columns of the rows counted: bytes, never characters. For files,
//! their sizes from metadata. Only regular files directly in a counted folder
//! count; a symlink, a counted folder's own or an entry's, is skipped and
//! never followed.
//!
//! **The deletes (decision 213).** [`Database::stored_data_delete`] deletes
//! one category and returns its counts before and after:
//!
//! * 1 to 4 each run in one `IMMEDIATE` transaction from their root, counted
//!   inside it, so the count before is what the delete removed: `manuscripts`
//!   (cascading to `extractions` and `findings`); the Gap Finder papers'
//!   `embeddings` rows by rowid, then their `documents` (cascading to
//!   `chunks` and any `citation_documents` link, and setting a staged
//!   source's `document_id` NULL), refused as [`DeleteError::PapersInUse`]
//!   while an `ai_chunks`, `ai_evidence_cards` or `ai_jobs` row references one
//!   of them, since those cascades would reach outside the category
//!   (decision 214); `ai_jobs` (cascading to `ai_job_items` and
//!   `audit_staged_sources`); the unexpired `report:` rows of `cache`.
//! * 5 is [`Database::cache_evict_stale`]'s one statement, reused.
//! * 6 to 9 remove regular files one by one. Just before each removal the
//!   file is asked again: still a regular file, its folder still a real
//!   folder, and for a note image, still used by no note and not changed in
//!   the last 10 minutes. A link is never followed and never removed. If one
//!   removal fails, the delete stops and says how many it removed.
//!
//! After 1 to 5 the delete runs under `PRAGMA secure_delete=ON`, so the text
//! it removes is zeroed in the pages it changes (the indexes' included), and
//! the caller runs [`Database::stored_data_compact`]: `PRAGMA
//! wal_checkpoint(TRUNCATE)`, retried once if busy, which flushes those zeroed
//! pages into `gaply.db` and truncates the `-wal`. Its outcome is
//! [`Compaction::Done`] only when the checkpoint's busy column is 0;
//! otherwise the rows stay deleted and the `-wal` holds them until a later
//! checkpoint — the one at each open among them — finishes. The file does not
//! shrink; SQLite reuses the freed space (5E-fix-1, decision 224). No
//! overwrite pass (decision 104).
//!
//! The caller passes the app data folder, the temp folder and the time, so a
//! test never touches the real ones.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::ToSql;
use rusqlite::{Connection, ErrorCode, TransactionBehavior};
use serde::Serialize;

use crate::db::Database;
use crate::error::GaplyError;

/// The nine categories, in the order the section shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredCategory {
    ManuscriptAnalyses,
    GapFinderPapers,
    ThesisAudits,
    CachedReports,
    ExpiredCache,
    ComparisonLog,
    OpenAccessPapers,
    TemporaryExports,
    OrphanNoteImages,
}

impl StoredCategory {
    pub const ALL: [StoredCategory; 9] = [
        StoredCategory::ManuscriptAnalyses,
        StoredCategory::GapFinderPapers,
        StoredCategory::ThesisAudits,
        StoredCategory::CachedReports,
        StoredCategory::ExpiredCache,
        StoredCategory::ComparisonLog,
        StoredCategory::OpenAccessPapers,
        StoredCategory::TemporaryExports,
        StoredCategory::OrphanNoteImages,
    ];

    /// The category's name on the wire, as serde writes it.
    pub fn wire(self) -> &'static str {
        match self {
            StoredCategory::ManuscriptAnalyses => "manuscript_analyses",
            StoredCategory::GapFinderPapers => "gap_finder_papers",
            StoredCategory::ThesisAudits => "thesis_audits",
            StoredCategory::CachedReports => "cached_reports",
            StoredCategory::ExpiredCache => "expired_cache",
            StoredCategory::ComparisonLog => "comparison_log",
            StoredCategory::OpenAccessPapers => "open_access_papers",
            StoredCategory::TemporaryExports => "temporary_exports",
            StoredCategory::OrphanNoteImages => "orphan_note_images",
        }
    }

    /// The category a wire name names, if any.
    pub fn parse(s: &str) -> Option<StoredCategory> {
        StoredCategory::ALL.into_iter().find(|c| c.wire() == s)
    }

    /// Categories 1 to 5 live in `gaply.db`; a delete of one is followed by
    /// [`Database::stored_data_compact`]. 6 to 9 are files.
    pub fn in_database(self) -> bool {
        matches!(
            self,
            StoredCategory::ManuscriptAnalyses
                | StoredCategory::GapFinderPapers
                | StoredCategory::ThesisAudits
                | StoredCategory::CachedReports
                | StoredCategory::ExpiredCache
        )
    }
}

/// One category's count: how many items, and how many bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    pub category: StoredCategory,
    pub items: u64,
    pub bytes: u64,
}

/// The nine counts, in [`StoredCategory::ALL`]'s order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredDataSummary {
    pub categories: Vec<CategoryCount>,
}

impl StoredDataSummary {
    /// The count of `category`.
    pub fn get(&self, category: StoredCategory) -> CategoryCount {
        self.categories
            .iter()
            .copied()
            .find(|c| c.category == category)
            .unwrap_or(CategoryCount {
                category,
                items: 0,
                bytes: 0,
            })
    }
}

/// What one delete did: the category's counts just before and just after,
/// and how many items it removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deletion {
    pub category: StoredCategory,
    pub before: CategoryCount,
    pub after: CategoryCount,
    pub deleted: u64,
    /// Categories 6 to 9: a removal failed after `deleted` files were
    /// removed, and the delete stopped there.
    pub removal_failed: bool,
}

/// Why a delete did not run.
#[derive(Debug)]
pub enum DeleteError {
    /// Category 2: an `ai_chunks`, `ai_evidence_cards` or `ai_jobs` row
    /// references one of its documents (decision 214). Nothing was deleted.
    PapersInUse,
    /// Anything else. In categories 1 to 4 the transaction rolled back; in 6
    /// to 9 this comes only before the first removal.
    Failed(GaplyError),
}

impl From<GaplyError> for DeleteError {
    fn from(e: GaplyError) -> Self {
        DeleteError::Failed(e)
    }
}

impl From<rusqlite::Error> for DeleteError {
    fn from(e: rusqlite::Error) -> Self {
        DeleteError::Failed(e.into())
    }
}

impl From<std::io::Error> for DeleteError {
    fn from(e: std::io::Error) -> Self {
        DeleteError::Failed(e.into())
    }
}

/// The checkpoint after a delete of categories 1 to 5 (decision 213, as
/// 5E-fix-1's decision 224 amends it: the checkpoint only, no VACUUM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compaction {
    /// The checkpoint finished with its busy column 0 (decision 224).
    Done,
    /// The rows stay deleted; the checkpoint did not finish.
    NotDone(CompactionCode),
}

/// Why a checkpoint did not finish, as a closed code (decision 224: the
/// VACUUM codes are gone with the VACUUM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionCode {
    CheckpointBusy,
    CheckpointFailed,
}

impl CompactionCode {
    pub fn wire(self) -> &'static str {
        match self {
            CompactionCode::CheckpointBusy => "stored_data.checkpoint_busy",
            CompactionCode::CheckpointFailed => "stored_data.checkpoint_failed",
        }
    }
}

/// The comparison log's file in the app data folder (`harness_log.rs`'s
/// `HARNESS_LOG_FILE`).
pub const COMPARISON_LOG_FILE: &str = "box4_comparisons.jsonl";
/// The open-access papers' folder in the app data folder (`oa_fetch::oa_dir`).
pub const OA_PAPERS_DIR: &str = "oa_papers";
/// The two kinds of file the open-access fetch writes there.
pub const OA_PAPER_SUFFIXES: &[&str] = &[".pdf", "-abstract.txt"];
/// The note images' folder in the app data folder (`noteImages.ts`'s
/// `IMAGE_SUBDIR`).
pub const NOTE_IMAGES_DIR: &str = "note-images";
/// How a note refers to one of them (`noteImages.ts`'s `IMAGE_REF_PREFIX`).
pub const NOTE_IMAGE_REF_PREFIX: &str = "gaply-image://";
/// A note image changed less than this many seconds ago is never counted or
/// deleted as unused (decisions 210 and 217). A file whose time cannot be
/// read, or lies in the future, counts as just changed.
pub const RECENT_IMAGE_SECS: i64 = 10 * 60;
/// The temp folder's export files, as (prefix, suffix): the names
/// `export_report` and `export_publishready_pdf` write and sweep.
pub const TEMP_EXPORTS: &[(&str, &str)] =
    &[("gaply-aicheck-", ".html"), ("gaply-publishready-", ".pdf")];
/// A Gap Finder paper's `source_url` (`paper_corpus.rs`).
pub const GAP_FINDER_URL_PREFIX: &str = "gapfinder://";
/// A cached report's key (`pipeline::report_cache_key`).
pub const REPORT_KEY_PREFIX: &str = "report:";
/// The `source_type` of an `embeddings` row whose `source_id` is a chunk
/// (`rag.rs`'s ingest).
pub const EMBEDDING_SOURCE_CHUNK: &str = "chunk";

/// The columns whose bytes are counted, by table: every column that holds
/// text or bytes (`every_counted_column_is_every_column_that_holds_text_or_bytes`
/// checks each list against the table's own schema).
const COUNTED_COLUMNS: &[(&str, &[&str])] = &[
    (
        "manuscripts",
        &["title", "authors", "abstract", "file_path", "status"],
    ),
    ("extractions", &["kind", "content"]),
    ("findings", &["severity", "category", "message"]),
    (
        "documents",
        &[
            "source_type",
            "title",
            "source_url",
            "checksum",
            "status",
            "quarantine_reason",
        ],
    ),
    ("chunks", &["content"]),
    ("embeddings", &["embedding", "source_type"]),
    (
        "ai_jobs",
        &[
            "kind",
            "status",
            "model_id",
            "prompt_version",
            "error",
            "summary_json",
            "source_path",
        ],
    ),
    (
        "ai_job_items",
        &[
            "kind",
            "sentence",
            "payload_json",
            "status",
            "result_json",
            "error",
        ],
    ),
    (
        "audit_staged_sources",
        &["surname", "title", "doi", "doi_norm", "raw", "matched_by"],
    ),
    ("cache", &["key", "value"]),
];

/// `table`'s counted columns.
fn columns(table: &str) -> &'static [&'static str] {
    COUNTED_COLUMNS
        .iter()
        .find(|(t, _)| *t == table)
        .map(|(_, c)| *c)
        .unwrap_or(&[])
}

/// `SELECT COUNT(*), <bytes>` over `table`'s rows that `filter` keeps,
/// where a row's bytes are `length(CAST(col AS BLOB))` summed over its
/// counted columns, a NULL counting 0.
fn count_sql(table: &str, filter: &str) -> String {
    let row_bytes = columns(table)
        .iter()
        .map(|c| format!("COALESCE(length(CAST(\"{c}\" AS BLOB)), 0)"))
        .collect::<Vec<_>>()
        .join(" + ");
    format!("SELECT COUNT(*), COALESCE(SUM({row_bytes}), 0) FROM \"{table}\" WHERE {filter}")
}

/// A Gap Finder paper's `documents` row: ?1 is its source type, ?2 the URL
/// prefix.
const GAP_FINDER_DOCUMENT: &str = "source_type = ?1 AND substr(source_url, 1, length(?2)) = ?2";

/// A row whose `document_id` is a Gap Finder paper, with
/// [`GAP_FINDER_DOCUMENT`]'s parameters.
fn in_gap_documents() -> String {
    format!("document_id IN (SELECT id FROM documents WHERE {GAP_FINDER_DOCUMENT})")
}

/// An unexpired cached report: ?1 is the key prefix, ?2 the time.
const CACHED_REPORT: &str = "substr(\"key\", 1, length(?1)) = ?1 AND created_at + ttl_seconds > ?2";

fn non_negative(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// (items, bytes) of one query from [`count_sql`].
fn count(conn: &Connection, sql: &str, params: &[&dyn ToSql]) -> Result<(u64, u64), GaplyError> {
    let (items, bytes): (i64, i64) = conn.query_row(sql, params, |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok((non_negative(items), non_negative(bytes)))
}

/// (items, bytes) of category 1 to 5's rows, read on `conn`. A file
/// category has no rows here and reads (0, 0).
fn row_count(
    conn: &Connection,
    category: StoredCategory,
    now: i64,
) -> Result<(u64, u64), GaplyError> {
    let research_paper = crate::rag::SourceType::ResearchPaper.as_str();
    let gap: [&dyn ToSql; 2] = [&research_paper, &GAP_FINDER_URL_PREFIX];
    match category {
        StoredCategory::ManuscriptAnalyses => {
            let (manuscripts, m_bytes) = count(conn, &count_sql("manuscripts", "1"), &[])?;
            let (_, e_bytes) = count(
                conn,
                &count_sql(
                    "extractions",
                    "manuscript_id IN (SELECT id FROM manuscripts)",
                ),
                &[],
            )?;
            let (_, f_bytes) = count(
                conn,
                &count_sql("findings", "manuscript_id IN (SELECT id FROM manuscripts)"),
                &[],
            )?;
            Ok((manuscripts, m_bytes + e_bytes + f_bytes))
        }
        StoredCategory::GapFinderPapers => {
            let (papers, d_bytes) =
                count(conn, &count_sql("documents", GAP_FINDER_DOCUMENT), &gap)?;
            let (_, c_bytes) = count(conn, &count_sql("chunks", &in_gap_documents()), &gap)?;
            let (_, v_bytes) = count(
                conn,
                &count_sql(
                    "embeddings",
                    &format!(
                        "source_type = ?3 AND source_id IN (SELECT id FROM chunks WHERE {})",
                        in_gap_documents()
                    ),
                ),
                &[
                    &research_paper,
                    &GAP_FINDER_URL_PREFIX,
                    &EMBEDDING_SOURCE_CHUNK,
                ],
            )?;
            Ok((papers, d_bytes + c_bytes + v_bytes))
        }
        StoredCategory::ThesisAudits => {
            let (jobs, j_bytes) = count(conn, &count_sql("ai_jobs", "1"), &[])?;
            let (_, i_bytes) = count(
                conn,
                &count_sql("ai_job_items", "job_id IN (SELECT id FROM ai_jobs)"),
                &[],
            )?;
            let (_, s_bytes) = count(
                conn,
                &count_sql("audit_staged_sources", "job_id IN (SELECT id FROM ai_jobs)"),
                &[],
            )?;
            Ok((jobs, j_bytes + i_bytes + s_bytes))
        }
        StoredCategory::CachedReports => count(
            conn,
            &count_sql("cache", CACHED_REPORT),
            &[&REPORT_KEY_PREFIX, &now],
        ),
        StoredCategory::ExpiredCache => count(
            conn,
            &count_sql("cache", "created_at + ttl_seconds <= ?1"),
            &[&now],
        ),
        StoredCategory::ComparisonLog
        | StoredCategory::OpenAccessPapers
        | StoredCategory::TemporaryExports
        | StoredCategory::OrphanNoteImages => Ok((0, 0)),
    }
}

/// One regular file found directly in a counted folder.
struct Found {
    name: String,
    bytes: u64,
    modified: Option<SystemTime>,
}

/// The regular files directly in `dir` whose names `keep` accepts. A missing
/// folder has none. A symlink or a subfolder in it is skipped, never
/// followed; sizes and times come from metadata, never contents.
fn regular_files(dir: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<Found>, GaplyError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !keep(&name) {
            continue;
        }
        match entry.metadata() {
            Ok(m) => out.push(Found {
                name,
                bytes: m.len(),
                modified: m.modified().ok(),
            }),
            // Removed between the listing and the read: no longer stored.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(out)
}

/// [`regular_files`] of the folder `name` inside `root`, which counts only
/// when it is a real folder: a symlink in its place is skipped, never
/// followed.
fn files_in_subfolder(
    root: &Path,
    name: &str,
    keep: impl Fn(&str) -> bool,
) -> Result<Vec<Found>, GaplyError> {
    let dir = root.join(name);
    if !is_real_dir(&dir)? {
        return Ok(Vec::new());
    }
    regular_files(&dir, keep)
}

/// Whether `dir` is a folder itself, not a link to one. Missing is no.
fn is_real_dir(dir: &Path) -> Result<bool, GaplyError> {
    match std::fs::symlink_metadata(dir) {
        Ok(m) => Ok(m.is_dir()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Whether a note image changed at `modified` is too recent to count or
/// delete as unused at `now` (decisions 210 and 217).
fn changed_recently(modified: Option<SystemTime>, now: i64) -> bool {
    let Some(modified) = modified else {
        return true;
    };
    let at = match modified.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        // Before 1970: long ago.
        Err(_) => 0,
    };
    now.saturating_sub(at) < RECENT_IMAGE_SECS
}

/// A listed file of categories 6 to 9: where it is, its name and its bytes.
struct Listed {
    path: PathBuf,
    name: String,
    bytes: u64,
}

fn total(files: &[Listed]) -> (u64, u64) {
    (files.len() as u64, files.iter().map(|f| f.bytes).sum())
}

/// The folder a file category's files sit directly in, when it is one of the
/// app data folder's own subfolders: checked again before each removal.
fn own_folder(category: StoredCategory, app_data_dir: &Path) -> Option<PathBuf> {
    match category {
        StoredCategory::OpenAccessPapers => Some(app_data_dir.join(OA_PAPERS_DIR)),
        StoredCategory::OrphanNoteImages => Some(app_data_dir.join(NOTE_IMAGES_DIR)),
        _ => None,
    }
}

/// Whether `e` is SQLite saying another connection holds what it needs.
fn is_busy(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if matches!(f.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

/// F2 (decision 224): sets `PRAGMA secure_delete=ON` on a connection for the
/// length of a delete, so the text the delete removes is zeroed in the pages
/// it changes, and restores the connection's earlier setting when it drops —
/// on every path, so the pooled connection returns to the pool unchanged.
struct SecureDelete<'c> {
    conn: &'c Connection,
    previous: i64,
}

impl<'c> SecureDelete<'c> {
    fn on(conn: &'c Connection) -> Result<SecureDelete<'c>, GaplyError> {
        let previous: i64 = conn.query_row("PRAGMA secure_delete", [], |r| r.get(0))?;
        conn.execute_batch("PRAGMA secure_delete=ON")?;
        Ok(SecureDelete { conn, previous })
    }
}

impl Drop for SecureDelete<'_> {
    fn drop(&mut self) {
        // `previous` is a 0/1/2 integer this read from the connection, so the
        // statement carries no outside value.
        let _ = self
            .conn
            .execute_batch(&format!("PRAGMA secure_delete={}", self.previous));
    }
}

/// `PRAGMA wal_checkpoint(TRUNCATE)` once, retried once if it is busy (F2,
/// decision 224). Done when the busy column is 0.
fn checkpoint_truncate(conn: &Connection) -> Compaction {
    let once = || -> rusqlite::Result<bool> {
        let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
        Ok(busy == 0)
    };
    let result = match once() {
        Ok(false) => once(),
        Err(e) if is_busy(&e) => once(),
        first => first,
    };
    match result {
        Ok(true) => Compaction::Done,
        Ok(false) => Compaction::NotDone(CompactionCode::CheckpointBusy),
        Err(e) if is_busy(&e) => Compaction::NotDone(CompactionCode::CheckpointBusy),
        Err(_) => Compaction::NotDone(CompactionCode::CheckpointFailed),
    }
}

impl Database {
    /// Count the nine categories. Reads only; see the module docs.
    pub fn stored_data_summary(
        &self,
        app_data_dir: &Path,
        temp_dir: &Path,
        now: i64,
    ) -> Result<StoredDataSummary, GaplyError> {
        // The files first: the note-image check asks the notes table
        // through `search_notes`, which takes its own connection.
        let mut files = Vec::new();
        for category in StoredCategory::ALL {
            if !category.in_database() {
                files.push(total(&self.listed_files(
                    category,
                    app_data_dir,
                    temp_dir,
                    now,
                )?));
            }
        }

        // The rows, from one snapshot. The transaction only reads, and is
        // rolled back when it drops.
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut rows = Vec::new();
        for category in StoredCategory::ALL {
            if category.in_database() {
                rows.push(row_count(&tx, category, now)?);
            }
        }
        drop(tx);

        let counts = rows.into_iter().chain(files);
        Ok(StoredDataSummary {
            categories: StoredCategory::ALL
                .iter()
                .zip(counts)
                .map(|(&category, (items, bytes))| CategoryCount {
                    category,
                    items,
                    bytes,
                })
                .collect(),
        })
    }

    /// The files of category 6 to 9 that count, as the summary counts them.
    /// A database category has none.
    fn listed_files(
        &self,
        category: StoredCategory,
        app_data_dir: &Path,
        temp_dir: &Path,
        now: i64,
    ) -> Result<Vec<Listed>, GaplyError> {
        let (dir, found) = match category {
            StoredCategory::ComparisonLog => {
                let path = app_data_dir.join(COMPARISON_LOG_FILE);
                return match std::fs::symlink_metadata(&path) {
                    Ok(m) if m.is_file() => Ok(vec![Listed {
                        path,
                        name: COMPARISON_LOG_FILE.to_string(),
                        bytes: m.len(),
                    }]),
                    Ok(_) => Ok(Vec::new()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
                    Err(e) => Err(e.into()),
                };
            }
            StoredCategory::OpenAccessPapers => (
                app_data_dir.join(OA_PAPERS_DIR),
                files_in_subfolder(app_data_dir, OA_PAPERS_DIR, |n| {
                    OA_PAPER_SUFFIXES.iter().any(|s| n.ends_with(s))
                })?,
            ),
            StoredCategory::TemporaryExports => (
                temp_dir.to_path_buf(),
                regular_files(temp_dir, |n| {
                    TEMP_EXPORTS
                        .iter()
                        .any(|(prefix, suffix)| n.starts_with(prefix) && n.ends_with(suffix))
                })?,
            ),
            StoredCategory::OrphanNoteImages => {
                let mut unused = Vec::new();
                for f in files_in_subfolder(app_data_dir, NOTE_IMAGES_DIR, |_| true)? {
                    if changed_recently(f.modified, now) {
                        continue;
                    }
                    if !self.note_image_used(&f.name)? {
                        unused.push(f);
                    }
                }
                (app_data_dir.join(NOTE_IMAGES_DIR), unused)
            }
            _ => return Ok(Vec::new()),
        };
        Ok(found
            .into_iter()
            .map(|f| Listed {
                path: dir.join(&f.name),
                name: f.name,
                bytes: f.bytes,
            })
            .collect())
    }

    /// Whether any note refers to the image `name`, asked as the Note
    /// Creator's own clean-up asks it.
    fn note_image_used(&self, name: &str) -> Result<bool, GaplyError> {
        let reference = format!("{NOTE_IMAGE_REF_PREFIX}{name}");
        Ok(!crate::notes::search_notes(self, &reference, None)?.is_empty())
    }

    /// One category's count, as the summary counts it.
    fn category_count(
        &self,
        category: StoredCategory,
        app_data_dir: &Path,
        temp_dir: &Path,
        now: i64,
    ) -> Result<CategoryCount, GaplyError> {
        let (items, bytes) = if category.in_database() {
            let conn = self.conn()?;
            row_count(&conn, category, now)?
        } else {
            total(&self.listed_files(category, app_data_dir, temp_dir, now)?)
        };
        Ok(CategoryCount {
            category,
            items,
            bytes,
        })
    }

    /// Delete one category, as the module docs set out, and return its
    /// counts before and after. Categories 1 to 5 are followed by
    /// [`Database::stored_data_compact`], which the caller runs.
    pub fn stored_data_delete(
        &self,
        category: StoredCategory,
        app_data_dir: &Path,
        temp_dir: &Path,
        now: i64,
    ) -> Result<Deletion, DeleteError> {
        // F2 (decision 224): the delete runs under `secure_delete=ON`, so the
        // text it removes is zeroed in the pages it changes; the checkpoint
        // the caller runs then flushes those pages into `gaply.db`.
        self.delete_with(category, app_data_dir, temp_dir, now, true, &|p| {
            std::fs::remove_file(p)
        })
    }

    /// [`Database::stored_data_delete`] with the file removal and whether
    /// `secure_delete` is set given, so a test can make a removal fail, or
    /// leave the deleted text in the file by running without it (F2).
    fn delete_with(
        &self,
        category: StoredCategory,
        app_data_dir: &Path,
        temp_dir: &Path,
        now: i64,
        secure: bool,
        remove: &dyn Fn(&Path) -> std::io::Result<()>,
    ) -> Result<Deletion, DeleteError> {
        let (before, deleted, removal_failed) = match category {
            StoredCategory::ManuscriptAnalyses
            | StoredCategory::GapFinderPapers
            | StoredCategory::ThesisAudits
            | StoredCategory::CachedReports => {
                let conn = self.conn()?;
                // F2 (decision 224): zero the pages this delete changes; the
                // guard restores the pooled connection's setting on drop.
                let _secure = secure.then(|| SecureDelete::on(&conn)).transpose()?;
                self.delete_rows_on(&conn, category, now)?
            }
            StoredCategory::ExpiredCache => {
                let conn = self.conn()?;
                let _secure = secure.then(|| SecureDelete::on(&conn)).transpose()?;
                let before = row_count(&conn, category, now)?;
                // F2 (decision 224): category 5 is cache_evict_stale's one
                // statement, run on this same connection so it is secure too.
                let removed = crate::cache::evict_stale_on(&conn, now)?;
                (before, removed as u64, false)
            }
            StoredCategory::ComparisonLog
            | StoredCategory::OpenAccessPapers
            | StoredCategory::TemporaryExports
            | StoredCategory::OrphanNoteImages => {
                self.delete_files(category, app_data_dir, temp_dir, now, remove)?
            }
        };
        let after = self.category_count(category, app_data_dir, temp_dir, now)?;
        Ok(Deletion {
            category,
            before: CategoryCount {
                category,
                items: before.0,
                bytes: before.1,
            },
            after,
            deleted,
            removal_failed,
        })
    }

    /// Categories 1 to 4: one `IMMEDIATE` transaction on `conn`, counted
    /// inside it. Any failure rolls it back. It runs on a given connection
    /// (not `&mut`), so a [`SecureDelete`] guard can be held on the same
    /// connection while this runs (F2, decision 224).
    fn delete_rows_on(
        &self,
        conn: &Connection,
        category: StoredCategory,
        now: i64,
    ) -> Result<((u64, u64), u64, bool), DeleteError> {
        let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        let before = row_count(&tx, category, now)?;
        let deleted = match category {
            StoredCategory::ManuscriptAnalyses => tx.execute("DELETE FROM manuscripts", [])?,
            StoredCategory::GapFinderPapers => {
                let research_paper = crate::rag::SourceType::ResearchPaper.as_str();
                let gap: [&dyn ToSql; 2] = [&research_paper, &GAP_FINDER_URL_PREFIX];
                // Decision 214: these cascades would reach a citation
                // check's rows, outside the category.
                let (in_use, _) = count(
                    &tx,
                    &format!(
                        "SELECT (SELECT COUNT(*) FROM ai_chunks WHERE {0})
                              + (SELECT COUNT(*) FROM ai_evidence_cards WHERE {0})
                              + (SELECT COUNT(*) FROM ai_jobs WHERE {0}), 0",
                        in_gap_documents()
                    ),
                    &gap,
                )?;
                if in_use > 0 {
                    return Err(DeleteError::PapersInUse);
                }
                // No cascade reaches the vec0 table, so its rows go first,
                // by rowid, while their chunks still name them.
                let rowids: Vec<i64> = {
                    let mut stmt = tx.prepare(&format!(
                        "SELECT rowid FROM embeddings
                         WHERE source_type = ?3 AND source_id IN (SELECT id FROM chunks WHERE {})",
                        in_gap_documents()
                    ))?;
                    let rows = stmt.query_map(
                        [
                            &research_paper as &dyn ToSql,
                            &GAP_FINDER_URL_PREFIX,
                            &EMBEDDING_SOURCE_CHUNK,
                        ],
                        |r| r.get(0),
                    )?;
                    rows.collect::<Result<_, _>>()?
                };
                for rowid in rowids {
                    tx.execute("DELETE FROM embeddings WHERE rowid = ?1", [rowid])?;
                }
                tx.execute(
                    &format!("DELETE FROM documents WHERE {GAP_FINDER_DOCUMENT}"),
                    &gap[..],
                )?
            }
            StoredCategory::ThesisAudits => tx.execute("DELETE FROM ai_jobs", [])?,
            StoredCategory::CachedReports => tx.execute(
                &format!("DELETE FROM cache WHERE {CACHED_REPORT}"),
                [&REPORT_KEY_PREFIX as &dyn ToSql, &now],
            )?,
            StoredCategory::ExpiredCache
            | StoredCategory::ComparisonLog
            | StoredCategory::OpenAccessPapers
            | StoredCategory::TemporaryExports
            | StoredCategory::OrphanNoteImages => 0,
        };
        tx.commit()?;
        Ok((before, deleted as u64, false))
    }

    /// Categories 6 to 9: the listed files, removed one by one, each asked
    /// again just before its removal. A failure after the listing stops the
    /// delete and is reported with the number removed, never as an error.
    fn delete_files(
        &self,
        category: StoredCategory,
        app_data_dir: &Path,
        temp_dir: &Path,
        now: i64,
        remove: &dyn Fn(&Path) -> std::io::Result<()>,
    ) -> Result<((u64, u64), u64, bool), DeleteError> {
        let listed = self.listed_files(category, app_data_dir, temp_dir, now)?;
        let before = total(&listed);
        let folder = own_folder(category, app_data_dir);
        let mut removed = 0u64;
        for file in &listed {
            match self.still_removable(category, folder.as_deref(), file, now) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(_) => return Ok((before, removed, true)),
            }
            match remove(&file.path) {
                Ok(()) => removed += 1,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Ok((before, removed, true)),
            }
        }
        Ok((before, removed, false))
    }

    /// Asked just before `file` is removed: its folder is still a real
    /// folder, it is still a regular file, and a note image is still used by
    /// no note and has not changed in the last 10 minutes.
    fn still_removable(
        &self,
        category: StoredCategory,
        folder: Option<&Path>,
        file: &Listed,
        now: i64,
    ) -> Result<bool, GaplyError> {
        if category == StoredCategory::OrphanNoteImages && self.note_image_used(&file.name)? {
            return Ok(false);
        }
        if let Some(dir) = folder {
            if !is_real_dir(dir)? {
                return Ok(false);
            }
        }
        match std::fs::symlink_metadata(&file.path) {
            Ok(m) if m.is_file() => Ok(category != StoredCategory::OrphanNoteImages
                || !changed_recently(m.modified().ok(), now)),
            Ok(_) => Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// `PRAGMA wal_checkpoint(TRUNCATE)`, retried once if busy (5E-fix-1,
    /// decision 224: no VACUUM). Done only when the checkpoint's busy column
    /// is 0; otherwise the rows stay deleted and the `-wal` holds them until a
    /// later checkpoint — the one at each open among them — finishes. The
    /// delete ran under `secure_delete=ON`, so this flushes already-zeroed
    /// pages into `gaply.db`.
    pub fn stored_data_compact(&self) -> Compaction {
        let conn = match self.conn() {
            Ok(conn) => conn,
            Err(_) => return Compaction::NotDone(CompactionCode::CheckpointFailed),
        };
        checkpoint_truncate(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sha2::{Digest, Sha256};

    use StoredCategory::*;

    /// A folder this test makes and removes, holding the app data folder
    /// (with `gaply.db` in it) and the temp folder.
    struct Scratch {
        root: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "gaply-5d1a-{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(root.join("app")).unwrap();
            std::fs::create_dir_all(root.join("tmp")).unwrap();
            Scratch { root }
        }
        fn app(&self) -> PathBuf {
            self.root.join("app")
        }
        fn tmp(&self) -> PathBuf {
            self.root.join("tmp")
        }
        fn open(&self) -> Database {
            Database::open(&self.app().join("gaply.db")).unwrap()
        }
        fn write(&self, rel: &str, bytes: &[u8]) {
            let p = self.root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }
        /// Set `rel`'s modification time to `epoch` seconds.
        fn age(&self, rel: &str, epoch: i64) {
            let f = std::fs::File::options()
                .write(true)
                .open(self.root.join(rel))
                .unwrap();
            let at = UNIX_EPOCH + std::time::Duration::from_secs(u64::try_from(epoch).unwrap());
            f.set_modified(at).unwrap();
        }
        fn summary(&self, db: &Database, now: i64) -> StoredDataSummary {
            db.stored_data_summary(&self.app(), &self.tmp(), now)
                .unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn len(xs: &[&str]) -> u64 {
        xs.iter().map(|s| s.len() as u64).sum()
    }

    fn exec(db: &Database, sql: &str, params: &[&dyn ToSql]) -> i64 {
        let conn = db.conn().unwrap();
        conn.execute(sql, params).unwrap();
        conn.last_insert_rowid()
    }

    const NOW: i64 = 1_800_000_000;
    /// A vec0 row's vector: 384 floats.
    const VECTOR_BYTES: u64 = 384 * 4;

    fn vector() -> Vec<u8> {
        (0..384u32)
            .flat_map(|i| (i as f32 / 384.0).to_le_bytes())
            .collect()
    }

    fn embed(db: &Database, source_type: &str, source_id: i64) {
        exec(
            db,
            "INSERT INTO embeddings (embedding, source_type, source_id) VALUES (?1, ?2, ?3)",
            &[&vector(), &source_type, &source_id],
        );
    }

    /// Every category's expected (items, bytes), as the fixture planted them.
    type Expected = BTreeMap<&'static str, (u64, u64)>;

    fn key(c: StoredCategory) -> &'static str {
        match c {
            ManuscriptAnalyses => "1",
            GapFinderPapers => "2",
            ThesisAudits => "3",
            CachedReports => "4",
            ExpiredCache => "5",
            ComparisonLog => "6",
            OpenAccessPapers => "7",
            TemporaryExports => "8",
            OrphanNoteImages => "9",
        }
    }

    /// Rows and files in every category, each text holding a sentinel, and
    /// beside them rows and files that look close and belong to none: a
    /// guideline document, a `paper` document, a research paper with another
    /// URL, an embedding of another source type, an unexpired non-report
    /// cache row, a `reportcard:` key, an evidence card, other files in each
    /// folder, a subfolder.
    fn plant(s: &Scratch, db: &Database) -> Expected {
        let mut want = Expected::new();

        // 1. Manuscript analyses.
        let m1 = exec(
            db,
            "INSERT INTO manuscripts (title, authors, abstract, status, created_at, updated_at)
             VALUES ('SENTINEL_SD_m1_title', 'SENTINEL_SD_m1_auth', 'SENTINEL_SD_m1_abs', 'extracted', 1, 1)",
            &[],
        );
        let e1 = exec(
            db,
            "INSERT INTO extractions (manuscript_id, kind, content, created_at)
             VALUES (?1, 'full', 'SENTINEL_SD_e1_section_text', 1)",
            &[&m1],
        );
        exec(
            db,
            "INSERT INTO findings (manuscript_id, extraction_id, severity, category, message, created_at)
             VALUES (?1, ?2, 'MAJOR', 'stats', 'SENTINEL_SD_f1_message', 1)",
            &[&m1, &e1],
        );
        exec(
            db,
            "INSERT INTO findings (manuscript_id, extraction_id, severity, category, message, created_at)
             VALUES (?1, NULL, 'MINOR', 'refs', 'SENTINEL_SD_f2_message', 1)",
            &[&m1],
        );
        exec(
            db,
            "INSERT INTO manuscripts (title, status, created_at, updated_at)
             VALUES ('SENTINEL_SD_m2_title', 'imported', 1, 1)",
            &[],
        );
        want.insert(
            "1",
            (
                2,
                len(&[
                    "SENTINEL_SD_m1_title",
                    "SENTINEL_SD_m1_auth",
                    "SENTINEL_SD_m1_abs",
                    "extracted",
                    "full",
                    "SENTINEL_SD_e1_section_text",
                    "MAJOR",
                    "stats",
                    "SENTINEL_SD_f1_message",
                    "MINOR",
                    "refs",
                    "SENTINEL_SD_f2_message",
                    "SENTINEL_SD_m2_title",
                    "imported",
                ]),
            ),
        );

        // 2. Gap Finder papers: two documents, two chunks, two embeddings.
        let doc = |source_type: &str,
                   title: &str,
                   url: &str,
                   checksum: &str,
                   status: &str,
                   q: Option<&str>| {
            exec(
                db,
                "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status,
                                        quarantine_reason, created_at)
                 VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, 1)",
                &[&source_type, &title, &url, &checksum, &status, &q],
            )
        };
        let chunk = |document: i64, seq: i64, content: &str| {
            exec(
                db,
                "INSERT INTO chunks (document_id, seq, content, token_count, created_at)
                 VALUES (?1, ?2, ?3, 3, 1)",
                &[&document, &seq, &content],
            )
        };
        let d1 = doc(
            "research_paper",
            "[s/p1] SENTINEL_SD_d1_title",
            "gapfinder://s/p1",
            "SENTINEL_SD_ck1",
            "ingested",
            None,
        );
        let c1 = chunk(d1, 0, "SENTINEL_SD_c1_text");
        let c2 = chunk(d1, 1, "SENTINEL_SD_c2_text");
        embed(db, "chunk", c1);
        embed(db, "chunk", c2);
        doc(
            "research_paper",
            "[s/p2] SENTINEL_SD_d2_title",
            "gapfinder://s/p2",
            "SENTINEL_SD_ck2",
            "quarantined",
            Some("SENTINEL_SD_reason"),
        );
        want.insert(
            "2",
            (
                2,
                len(&[
                    "research_paper",
                    "[s/p1] SENTINEL_SD_d1_title",
                    "gapfinder://s/p1",
                    "SENTINEL_SD_ck1",
                    "ingested",
                    "SENTINEL_SD_c1_text",
                    "SENTINEL_SD_c2_text",
                    "chunk",
                    "chunk",
                    "research_paper",
                    "[s/p2] SENTINEL_SD_d2_title",
                    "gapfinder://s/p2",
                    "SENTINEL_SD_ck2",
                    "quarantined",
                    "SENTINEL_SD_reason",
                ]) + 2 * VECTOR_BYTES,
            ),
        );
        // ... and the shared tables' other rows, which count nowhere.
        let guide = doc(
            "journal_guideline",
            "SENTINEL_SD_guide",
            "https://journal.example/SENTINEL_SD_guide",
            "SENTINEL_SD_ck3",
            "ingested",
            None,
        );
        let c3 = chunk(guide, 0, "SENTINEL_SD_c3_text");
        embed(db, "chunk", c3);
        let other = doc(
            "research_paper",
            "SENTINEL_SD_d5_title",
            "https://example.org/SENTINEL_SD_d5",
            "SENTINEL_SD_ck5",
            "ingested",
            None,
        );
        let c5 = chunk(other, 0, "SENTINEL_SD_c5_text");
        embed(db, "chunk", c5);
        embed(db, "manuscript_chunk", c1);
        let paper = doc(
            "paper",
            "SENTINEL_SD_paper",
            "/Users/SENTINEL_SD_user/linked.pdf",
            "SENTINEL_SD_ck4",
            "ingested",
            None,
        );
        exec(
            db,
            "INSERT INTO ai_chunks (document_id, char_start, char_end, content, token_estimate,
                                    content_hash, created_at)
             VALUES (?1, 0, 10, 'SENTINEL_SD_ai_chunk', 2, 'SENTINEL_SD_h', 1)",
            &[&paper],
        );
        exec(
            db,
            "INSERT INTO ai_model_registry (id, kind, registered_at) VALUES ('gen', 'generative', 1)",
            &[],
        );
        exec(
            db,
            "INSERT INTO ai_evidence_cards (document_id, claim, evidence_text, evidence_type, model_id,
                                            model_version, prompt_version, created_at)
             VALUES (?1, 'SENTINEL_SD_claim', 'SENTINEL_SD_evidence', 'quote', 'gen', 'q', 'p', 1)",
            &[&paper],
        );

        // 3. Thesis audits: two jobs, two items, one staged source.
        let j1 = exec(
            db,
            "INSERT INTO ai_jobs (kind, status, prompt_version, created_at, summary_json, source_path)
             VALUES ('thesis_audit', 'done', 'v1', 1, 'SENTINEL_SD_summary',
                     '/Users/SENTINEL_SD_user/thesis.docx')",
            &[],
        );
        exec(
            db,
            "INSERT INTO ai_job_items (job_id, seq, kind, sentence, payload_json, status, result_json,
                                       created_at)
             VALUES (?1, 0, 'citation_need', 'SENTINEL_SD_sentence_1', '{}', 'done',
                     'SENTINEL_SD_result', 1)",
            &[&j1],
        );
        exec(
            db,
            "INSERT INTO ai_job_items (job_id, seq, kind, sentence, status, error, created_at)
             VALUES (?1, 1, 'unverifiable', 'SENTINEL_SD_sentence_2', 'failed', 'SENTINEL_SD_err', 1)",
            &[&j1],
        );
        exec(
            db,
            "INSERT INTO audit_staged_sources (job_id, surname, year, title, doi, doi_norm, raw, created_at)
             VALUES (?1, 'SENTINEL_SD_surname', 2020, 'SENTINEL_SD_ref_title', '10.1/SENTINEL_SD',
                     '10.1/sentinel_sd', 'SENTINEL_SD_raw', 1)",
            &[&j1],
        );
        exec(
            db,
            "INSERT INTO ai_jobs (kind, status, prompt_version, created_at)
             VALUES ('citation_audit', 'running', 'v2', 1)",
            &[],
        );
        want.insert(
            "3",
            (
                2,
                len(&[
                    "thesis_audit",
                    "done",
                    "v1",
                    "SENTINEL_SD_summary",
                    "/Users/SENTINEL_SD_user/thesis.docx",
                    "citation_need",
                    "SENTINEL_SD_sentence_1",
                    "{}",
                    "done",
                    "SENTINEL_SD_result",
                    "unverifiable",
                    "SENTINEL_SD_sentence_2",
                    "{}",
                    "failed",
                    "SENTINEL_SD_err",
                    "SENTINEL_SD_surname",
                    "SENTINEL_SD_ref_title",
                    "10.1/SENTINEL_SD",
                    "10.1/sentinel_sd",
                    "SENTINEL_SD_raw",
                    "citation_audit",
                    "running",
                    "v2",
                ]),
            ),
        );

        // 4 and 5. The cache: an unexpired report counts in 4; an expired
        // report, one expiring exactly now and an expired API body in 5; an
        // unexpired API body and a `reportcard:` key nowhere.
        let cache = |k: &str, v: &str, created: i64, ttl: i64| {
            exec(
                db,
                "INSERT INTO cache (key, value, created_at, ttl_seconds) VALUES (?1, ?2, ?3, ?4)",
                &[&k, &v, &created, &ttl],
            );
        };
        cache("report:v2:e1:1", "SENTINEL_SD_report_fresh", NOW - 10, 1000);
        cache("report:v2:e1:2", "SENTINEL_SD_report_old", NOW - 2000, 1000);
        cache(
            "report:v2:e1:3",
            "SENTINEL_SD_report_edge",
            NOW - 1000,
            1000,
        );
        cache(
            "refverify:crossref:doi:SENTINEL_SD",
            "SENTINEL_SD_api_fresh",
            NOW - 10,
            1000,
        );
        cache(
            "refverify:crossref:doi:SENTINEL_SD_old",
            "SENTINEL_SD_api_old",
            NOW - 5000,
            1000,
        );
        cache(
            "reportcard:SENTINEL_SD",
            "SENTINEL_SD_not_a_report",
            NOW - 10,
            1000,
        );
        want.insert(
            "4",
            (1, len(&["report:v2:e1:1", "SENTINEL_SD_report_fresh"])),
        );
        want.insert(
            "5",
            (
                3,
                len(&[
                    "report:v2:e1:2",
                    "SENTINEL_SD_report_old",
                    "report:v2:e1:3",
                    "SENTINEL_SD_report_edge",
                    "refverify:crossref:doi:SENTINEL_SD_old",
                    "SENTINEL_SD_api_old",
                ]),
            ),
        );

        // Notes: one image is used in a body, one in a template field.
        exec(
            db,
            "INSERT INTO notes (id, note_type, title, body, created_at, updated_at)
             VALUES ('n1', 'project', 'SENTINEL_SD_note', 'see ![x](gaply-image://aaa.png)', 1, 1)",
            &[],
        );
        exec(
            db,
            "INSERT INTO notes (id, note_type, title, fields_json, created_at, updated_at)
             VALUES ('n2', 'paper', 'SENTINEL_SD_note2',
                     '{\"key_findings\":\"gaply-image://ccc.png\"}', 1, 1)",
            &[],
        );

        // 6. The comparison log.
        let log = b"{\"run\":\"SENTINEL_SD\"}\n";
        s.write("app/box4_comparisons.jsonl", log);
        want.insert("6", (1, log.len() as u64));

        // 7. Open-access papers, beside a stray file and a subfolder.
        s.write("app/oa_papers/10.1_SENTINEL_SD.pdf", &[7u8; 300]);
        s.write("app/oa_papers/10.2_SENTINEL_SD-abstract.txt", &[7u8; 40]);
        s.write("app/oa_papers/SENTINEL_SD.txt", &[7u8; 5]);
        s.write("app/oa_papers/.DS_Store", &[7u8; 6]);
        s.write("app/oa_papers/sub/10.3.pdf", &[7u8; 70]);
        want.insert("7", (2, 340));

        // 8. Temporary exports, beside files that only look like them.
        s.write("tmp/gaply-aicheck-100.html", &[8u8; 120]);
        s.write("tmp/gaply-publishready-7-100.pdf", &[8u8; 900]);
        s.write("tmp/gaply-sample-manuscript.txt", &[8u8; 11]);
        s.write("tmp/gaply-aicheck-100.htm", &[8u8; 12]);
        s.write("tmp/x-gaply-aicheck-1.html", &[8u8; 13]);
        s.write("tmp/gaply-publishready-.pdf.part", &[8u8; 14]);
        want.insert("8", (2, 1020));

        // 9. Note images, each changed an hour before NOW: aaa and ccc are
        // used, bbb is not. ddd is not used either, but it changed a minute
        // before NOW, so it counts as just pasted and is left out
        // (decision 217).
        s.write("app/note-images/aaa.png", &[9u8; 50]);
        s.write("app/note-images/bbb.png", &[9u8; 60]);
        s.write("app/note-images/ccc.png", &[9u8; 70]);
        s.write("app/note-images/ddd.png", &[9u8; 80]);
        for name in ["aaa.png", "bbb.png", "ccc.png"] {
            s.age(&format!("app/note-images/{name}"), NOW - 3600);
        }
        s.age("app/note-images/ddd.png", NOW - 60);
        want.insert("9", (1, 60));

        // A project's file, which no category counts.
        s.write("app/projects/p/project.db", &[1u8; 99]);
        want
    }

    fn assert_counts(got: &StoredDataSummary, want: &Expected) {
        let got: Expected = got
            .categories
            .iter()
            .map(|c| (key(c.category), (c.items, c.bytes)))
            .collect();
        assert_eq!(&got, want, "(items, bytes) by category");
    }

    #[test]
    fn an_empty_database_and_empty_folders_count_zero_everywhere() {
        let s = Scratch::new("empty");
        let db = s.open();
        let got = s.summary(&db, NOW);
        assert_eq!(
            got.categories
                .iter()
                .map(|c| c.category)
                .collect::<Vec<_>>(),
            StoredCategory::ALL.to_vec(),
            "nine categories, in order"
        );
        assert!(
            got.categories.iter().all(|c| c.items == 0 && c.bytes == 0),
            "{got:?}"
        );
        // The counted folders made, and still empty.
        for d in ["app/oa_papers", "app/note-images"] {
            std::fs::create_dir_all(s.root.join(d)).unwrap();
        }
        assert!(s
            .summary(&db, NOW)
            .categories
            .iter()
            .all(|c| c.items == 0 && c.bytes == 0));
    }

    #[test]
    fn each_category_counts_its_own_rows_and_files_and_no_one_elses() {
        let s = Scratch::new("fixture");
        let db = s.open();
        let want = plant(&s, &db);
        assert_counts(&s.summary(&db, NOW), &want);
    }

    #[test]
    fn a_report_row_moves_from_cached_to_expired_when_its_time_ends() {
        let s = Scratch::new("expiry");
        let db = s.open();
        db.cache_put("report:v2:e1:9", "v", 100, NOW).unwrap();
        let at = |t| {
            let got = s.summary(&db, t);
            (got.get(CachedReports).items, got.get(ExpiredCache).items)
        };
        assert_eq!(at(NOW), (1, 0));
        assert_eq!(at(NOW + 99), (1, 0));
        assert_eq!(
            at(NOW + 100),
            (0, 1),
            "created_at + ttl_seconds <= now is expired"
        );
        assert_eq!(at(NOW + 10_000), (0, 1));
    }

    #[test]
    fn a_text_with_a_multibyte_character_is_counted_in_bytes() {
        let s = Scratch::new("utf8");
        let db = s.open();
        // é is 2 bytes, — and 日 are 3 each: 3 characters, 8 bytes.
        let title = "é—日";
        assert_eq!((title.chars().count(), title.len()), (3, 8));
        exec(
            &db,
            "INSERT INTO manuscripts (title, status, created_at, updated_at) VALUES (?1, 'imported', 1, 1)",
            &[&title],
        );
        let got = s.summary(&db, NOW).get(ManuscriptAnalyses);
        assert_eq!(
            (got.items, got.bytes),
            (1, 8 + 8),
            "bytes of 'é—日' and 'imported', not 3 + 8 characters"
        );
    }

    /// A relative path keyed with '/' between its parts, never `display()`
    /// (F5: `display()` gives `\` on Windows, so a file keys differently
    /// there and a snapshot compares unequal).
    fn rel_slashes(root: &Path, p: &Path) -> String {
        p.strip_prefix(root)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// F6: after the last connection closes, SQLite truncates and removes the
    /// WAL, which can lag; wait up to 5 s for it to be gone.
    fn wait_no_wal(app: &Path) {
        let wal = app.join("gaply.db-wal");
        let start = std::time::Instant::now();
        while wal.exists() && start.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Every file under `root` by path (with '/'), with its SHA-256, and
    /// every folder. SQLite's `-wal`/`-shm` are skipped by name: they come and
    /// go with connections (F5/F6). A file that vanishes between the listing
    /// and the read is skipped, not an error (F6).
    fn snapshot(root: &Path) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                let rel = rel_slashes(root, &p);
                let t = e.file_type().unwrap();
                if t.is_dir() {
                    out.insert(rel, "dir".into());
                    stack.push(p);
                    continue;
                }
                if t.is_symlink() {
                    out.insert(
                        rel,
                        format!("link {}", std::fs::read_link(&p).unwrap().display()),
                    );
                    continue;
                }
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name == "gaply.db-wal" || name == "gaply.db-shm" {
                    continue;
                }
                match std::fs::read(&p) {
                    Ok(bytes) => {
                        out.insert(rel, format!("{:x}", Sha256::digest(&bytes)));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => panic!("{}: {e}", p.display()),
                }
            }
        }
        out
    }

    #[test]
    fn the_summary_writes_nothing() {
        let s = Scratch::new("readonly");
        let db = s.open();
        let want = plant(&s, &db);
        // Close every connection, so the WAL is checkpointed and gone.
        drop(db);
        wait_no_wal(&s.app());
        let before = snapshot(&s.root);
        assert!(before.contains_key("app/gaply.db"), "{before:?}");
        assert!(
            !before.contains_key("app/gaply.db-wal"),
            "snapshot skips the WAL: {before:?}"
        );

        let db = s.open();
        let got = s.summary(&db, NOW);
        drop(db);
        wait_no_wal(&s.app());
        let after = snapshot(&s.root);

        // Bytes first, so a write is what fails here, whatever it did to the
        // counts.
        assert_eq!(
            before, after,
            "the database and every file are byte-identical after a summary"
        );
        assert_counts(&got, &want);
    }

    #[test]
    fn the_view_holds_counts_and_bytes_and_no_text_of_what_it_counts() {
        let s = Scratch::new("sentinel");
        let db = s.open();
        plant(&s, &db);
        let got = s.summary(&db, NOW);
        // Not vacuous: every category counted something.
        for c in &got.categories {
            assert!(c.items > 0 && c.bytes > 0, "{c:?}");
        }
        let json = serde_json::to_string(&got).unwrap();
        for needle in [
            "SENTINEL",
            "sentinel",
            "gapfinder",
            "report:",
            "aaa.png",
            "bbb.png",
            "box4",
            "oa_papers",
            "gaply-aicheck",
            "/Users",
            s.root.to_str().unwrap(),
        ] {
            assert!(!json.contains(needle), "{needle} reached the view: {json}");
        }
        // Its shape: nine objects of exactly three fields.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let cats = v["categories"].as_array().unwrap();
        assert_eq!(cats.len(), 9);
        for c in cats {
            let mut keys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(keys, ["bytes", "category", "items"]);
        }
        assert_eq!(v.as_object().unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_a_counted_folder_is_skipped_and_never_followed() {
        use std::os::unix::fs::symlink;
        let s = Scratch::new("links");
        let db = s.open();
        s.write("outside/big.bin", &[1u8; 5000]);
        s.write("outside/papers/real.pdf", &[1u8; 4000]);
        let big = s.root.join("outside/big.bin");
        std::fs::create_dir_all(s.app().join("note-images")).unwrap();
        symlink(&big, s.app().join(COMPARISON_LOG_FILE)).unwrap();
        symlink(&big, s.tmp().join("gaply-aicheck-1.html")).unwrap();
        symlink(&big, s.app().join("note-images/zzz.png")).unwrap();
        // The open-access folder is itself a link to a folder holding a PDF.
        symlink(s.root.join("outside/papers"), s.app().join(OA_PAPERS_DIR)).unwrap();

        let got = s.summary(&db, NOW);
        for c in [
            ComparisonLog,
            OpenAccessPapers,
            TemporaryExports,
            OrphanNoteImages,
        ] {
            assert_eq!(
                (got.get(c).items, got.get(c).bytes),
                (0, 0),
                "{c:?} followed a link"
            );
        }

        // Control: the same files, real, are counted.
        let s2 = Scratch::new("links-control");
        let db2 = s2.open();
        s2.write("app/box4_comparisons.jsonl", &[1u8; 5000]);
        s2.write("tmp/gaply-aicheck-1.html", &[1u8; 5000]);
        s2.write("app/note-images/zzz.png", &[1u8; 5000]);
        s2.age("app/note-images/zzz.png", NOW - 3600);
        s2.write("app/oa_papers/real.pdf", &[1u8; 4000]);
        let got = s2.summary(&db2, NOW);
        for (c, bytes) in [
            (ComparisonLog, 5000),
            (OpenAccessPapers, 4000),
            (TemporaryExports, 5000),
            (OrphanNoteImages, 5000),
        ] {
            assert_eq!((got.get(c).items, got.get(c).bytes), (1, bytes), "{c:?}");
        }
    }

    #[test]
    fn the_real_writers_land_in_their_categories() {
        use crate::ai_engine::jobs::{self, ItemKind, NewItem};
        use crate::rag::{self, RawDocument, SourceType};

        let s = Scratch::new("writers");
        let db = s.open();
        let m = db.create_manuscript("SENTINEL_SD_written", "", "").unwrap();
        crate::extract::persist::store_extraction(
            &db,
            m,
            &crate::extract::extract_from_text("A result, t(10) = 2.1, p = .06."),
        )
        .unwrap();
        let paper = |source_type, title: &str, url: &str| RawDocument {
            source_type,
            title: title.into(),
            source_url: url.into(),
            fetched_at: NOW,
            content: format!(
                "{title}. A paper about sampling and inference, long enough to chunk."
            ),
        };
        let gap = rag::ingest_document(
            &db,
            &crate::embed::HashEmbedder,
            &paper(
                SourceType::ResearchPaper,
                "[s/p1] A study",
                "gapfinder://s/p1",
            ),
        )
        .unwrap();
        assert!(
            matches!(gap.status, rag::IngestStatus::Ingested { .. }),
            "{:?}",
            gap.status
        );
        rag::ingest_document(
            &db,
            &crate::embed::HashEmbedder,
            &paper(
                SourceType::JournalGuideline,
                "Guide",
                "https://journal.example/guide",
            ),
        )
        .unwrap();
        jobs::create_job(
            &db,
            "thesis_audit",
            None,
            Some("/x/m.docx"),
            "v1",
            &[NewItem {
                seq: 0,
                kind: ItemKind::CitationNeed,
                chunk_id: None,
                page: None,
                sentence: "A sentence.".into(),
                payload_json: "{}".into(),
            }],
        )
        .unwrap();
        db.cache_put(&format!("{REPORT_KEY_PREFIX}v2:e1:{m}"), "{}", 3600, NOW)
            .unwrap();

        let got = s.summary(&db, NOW);
        for c in [
            ManuscriptAnalyses,
            GapFinderPapers,
            ThesisAudits,
            CachedReports,
        ] {
            assert_eq!(got.get(c).items, 1, "{c:?}: {got:?}");
        }
        let chunks = db.count_rows("chunks").unwrap() as u64;
        assert!(chunks >= 2, "both documents were chunked");
        // The Gap Finder paper's embeddings are in its bytes: at least one
        // vector per chunk of it.
        assert!(got.get(GapFinderPapers).bytes > VECTOR_BYTES, "{got:?}");
        assert_eq!(got.get(ExpiredCache).items, 0);
    }

    #[test]
    fn every_counted_column_is_every_column_that_holds_text_or_bytes() {
        let s = Scratch::new("columns");
        let db = s.open();
        let conn = db.conn().unwrap();
        for (table, listed) in COUNTED_COLUMNS {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info(\"{table}\")"))
                .unwrap();
            let columns: Vec<(String, String)> = stmt
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            let mut holds_text_or_bytes: Vec<String> = if *table == "embeddings" {
                // vec0 reports its own column types, so the rule below cannot
                // read them: its columns are pinned by name instead, and
                // `rowid` and `source_id` are integers (migrations.rs:117-121).
                let mut all: Vec<&str> = columns.iter().map(|(n, _)| n.as_str()).collect();
                all.sort_unstable();
                assert_eq!(
                    all,
                    ["embedding", "rowid", "source_id", "source_type"],
                    "vec0's columns"
                );
                vec!["embedding".into(), "source_type".into()]
            } else {
                columns
                    .into_iter()
                    .filter(|(_, ty)| {
                        !matches!(ty.to_ascii_uppercase().as_str(), "INTEGER" | "REAL")
                    })
                    .map(|(name, _)| name)
                    .collect()
            };
            holds_text_or_bytes.sort();
            let mut listed: Vec<String> = listed.iter().map(|c| c.to_string()).collect();
            listed.sort();
            assert_eq!(listed, holds_text_or_bytes, "{table}");
        }
    }

    /// Every table a fresh database has, by name. The map
    /// (`stored_data_map.md`) classifies each; a table added later fails
    /// here until it is classified there too.
    #[test]
    fn every_table_in_a_fresh_database_is_classified() {
        let s = Scratch::new("tables");
        let db = s.open();
        let conn = db.conn().unwrap();
        let names = |sql: &str| -> Vec<String> {
            let mut stmt = conn.prepare(sql).unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        let tables = names("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name");
        let mut classified: Vec<&str> = vec![
            // 1 Manuscript analyses
            "manuscripts",
            "extractions",
            "findings",
            // 2 Gap Finder papers (shared with guideline pages), and the
            // vec0 table's shadow tables
            "documents",
            "chunks",
            "embeddings",
            "embeddings_info",
            "embeddings_chunks",
            "embeddings_rowids",
            "embeddings_vector_chunks00",
            "embeddings_auxiliary",
            // 3 Thesis audits
            "ai_jobs",
            "ai_job_items",
            "audit_staged_sources",
            // 4 and 5
            "cache",
            // has its own delete
            "notes",
            "citation_library",
            "plagiarism_library",
            // no delete in the app
            "journal_requirements",
            "journal_conventions",
            "journal_expectations",
            "journal_standard_bindings",
            "journal_fingerprints",
            "ai_model_registry",
            // no category: the findings of the map's §5, with the FTS5
            // index's shadow tables
            "ai_chunks",
            "ai_chunks_fts",
            "ai_chunks_fts_data",
            "ai_chunks_fts_idx",
            "ai_chunks_fts_docsize",
            "ai_chunks_fts_config",
            "ai_chunk_embeddings",
            "ai_evidence_cards",
            "ai_evidence_card_chunks",
            "citation_documents",
            "evidence",
            // no category: dormant, or no production writer
            "projects",
            "episodic_memory",
            "consent_records",
            "journal_guidelines",
            "retractions",
            "reference_styles",
            // not user data
            "schema_migrations",
            "sqlite_sequence",
        ];
        classified.sort_unstable();
        assert_eq!(tables, classified);
        let virtual_tables = names(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND sql LIKE 'CREATE VIRTUAL TABLE%' ORDER BY name",
        );
        assert_eq!(virtual_tables, ["ai_chunks_fts", "embeddings"]);
    }

    /* ---- the deletes (Phase 5D-1b; decisions 210, 213, 214, 217) ---------- */

    fn one(db: &Database, sql: &str, params: &[&dyn ToSql]) -> i64 {
        db.conn()
            .unwrap()
            .query_row(sql, params, |r| r.get(0))
            .unwrap()
    }

    /// Every table's rows, each row as text, sorted: the whole database.
    fn dump(db: &Database) -> BTreeMap<String, Vec<String>> {
        let conn = db.conn().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut out = BTreeMap::new();
        for t in tables {
            let mut stmt = conn.prepare(&format!("SELECT * FROM \"{t}\"")).unwrap();
            let n = stmt.column_count();
            let mut rows: Vec<String> = stmt
                .query_map([], |r| {
                    let mut cells = Vec::with_capacity(n);
                    for i in 0..n {
                        cells.push(format!("{:?}", r.get::<_, rusqlite::types::Value>(i)?));
                    }
                    Ok(cells.join("|"))
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            rows.sort();
            out.insert(t, rows);
        }
        out
    }

    /// Every file under the scratch folder but the database's own, left out
    /// by file name (F5): `gaply.db`, `gaply.db-wal`, `gaply.db-shm`.
    fn files(s: &Scratch) -> BTreeMap<String, String> {
        snapshot(&s.root)
            .into_iter()
            .filter(|(k, _)| {
                let name = k.rsplit('/').next().unwrap_or(k.as_str());
                name != "gaply.db" && name != "gaply.db-wal" && name != "gaply.db-shm"
            })
            .collect()
    }

    /// The tables a delete of `c` may change: its own (the vec0 table's
    /// shadow tables with it), and for category 2 the links that cascade.
    fn tables_of(c: StoredCategory) -> &'static [&'static str] {
        match c {
            ManuscriptAnalyses => &["manuscripts", "extractions", "findings"],
            GapFinderPapers => &[
                "documents",
                "chunks",
                "embeddings",
                "embeddings_info",
                "embeddings_chunks",
                "embeddings_rowids",
                "embeddings_vector_chunks00",
                "embeddings_auxiliary",
                "citation_documents",
            ],
            ThesisAudits => &["ai_jobs", "ai_job_items", "audit_staged_sources"],
            CachedReports | ExpiredCache => &["cache"],
            _ => &[],
        }
    }

    /// The fixture's files of `c`, which its delete removes.
    fn files_of(c: StoredCategory) -> &'static [&'static str] {
        match c {
            ComparisonLog => &["app/box4_comparisons.jsonl"],
            OpenAccessPapers => &[
                "app/oa_papers/10.1_SENTINEL_SD.pdf",
                "app/oa_papers/10.2_SENTINEL_SD-abstract.txt",
            ],
            TemporaryExports => &[
                "tmp/gaply-aicheck-100.html",
                "tmp/gaply-publishready-7-100.pdf",
            ],
            OrphanNoteImages => &["app/note-images/bbb.png"],
            _ => &[],
        }
    }

    fn gap_paper(db: &Database) -> i64 {
        one(
            db,
            "SELECT id FROM documents WHERE title = '[s/p1] SENTINEL_SD_d1_title'",
            &[],
        )
    }

    fn linked_paper(db: &Database) -> i64 {
        one(
            db,
            "SELECT id FROM documents WHERE source_type = 'paper'",
            &[],
        )
    }

    /// A library citation linked to a Gap Finder paper and to a `paper`
    /// document: `link_citations` reads every document (the map's §4).
    fn link_papers(db: &Database) {
        exec(
            db,
            "INSERT INTO citation_library (id, csl_json, title, created_at, updated_at)
             VALUES ('cit1', '{}', 'SENTINEL_SD_citation', 1, 1)",
            &[],
        );
        for doc in [gap_paper(db), linked_paper(db)] {
            exec(
                db,
                "INSERT INTO citation_documents (citation_id, document_id, matched_by, created_at)
                 VALUES ('cit1', ?1, 'title', 1)",
                &[&doc],
            );
        }
    }

    fn delete(s: &Scratch, db: &Database, c: StoredCategory) -> Deletion {
        db.stored_data_delete(c, &s.app(), &s.tmp(), NOW)
            .unwrap_or_else(|e| panic!("{c:?}: {e:?}"))
    }

    #[test]
    fn each_delete_empties_its_category_and_leaves_everything_else_as_it_was() {
        for c in StoredCategory::ALL {
            let s = Scratch::new("delete-each");
            let db = s.open();
            plant(&s, &db);
            link_papers(&db);
            let summary_before = s.summary(&db, NOW);
            let tables_before = dump(&db);
            let files_before = files(&s);
            assert!(
                summary_before.get(c).items > 0,
                "{c:?}: the fixture holds some"
            );

            let d = delete(&s, &db, c);
            assert_eq!(d.category, c);
            assert_eq!(
                d.before,
                summary_before.get(c),
                "{c:?}: before is the summary's count"
            );
            assert_eq!(d.deleted, d.before.items, "{c:?}");
            assert_eq!((d.after.items, d.after.bytes), (0, 0), "{c:?}");
            assert!(!d.removal_failed, "{c:?}");

            let summary_after = s.summary(&db, NOW);
            for other in StoredCategory::ALL {
                let got = summary_after.get(other);
                if other == c {
                    assert_eq!((got.items, got.bytes), (0, 0), "{c:?} is empty");
                } else {
                    assert_eq!(
                        got,
                        summary_before.get(other),
                        "{c:?}'s delete changed {other:?}"
                    );
                }
            }
            let tables_after = dump(&db);
            assert_eq!(
                tables_before.keys().collect::<Vec<_>>(),
                tables_after.keys().collect::<Vec<_>>()
            );
            for (t, rows) in &tables_before {
                if !tables_of(c).contains(&t.as_str()) {
                    assert_eq!(
                        Some(rows),
                        tables_after.get(t),
                        "{c:?}'s delete changed {t}"
                    );
                }
            }
            let files_after = files(&s);
            for (p, h) in &files_before {
                if files_of(c).contains(&p.as_str()) {
                    assert!(!files_after.contains_key(p), "{c:?} left {p}");
                } else {
                    assert_eq!(files_after.get(p), Some(h), "{c:?}'s delete changed {p}");
                }
            }
            assert_eq!(
                files_after.len(),
                files_before.len() - files_of(c).len(),
                "{c:?}: nothing else removed or added"
            );
            // `files` leaves the database's own files out: it is still there.
            assert!(s.app().join("gaply.db").is_file(), "{c:?} removed gaply.db");
        }
    }

    /// Each database category's sentinels: text only it holds.
    const DB_SENTINELS: &[(StoredCategory, &[&str])] = &[
        (
            ManuscriptAnalyses,
            &[
                "SENTINEL_SD_m1_title",
                "SENTINEL_SD_m1_abs",
                "SENTINEL_SD_e1_section_text",
                "SENTINEL_SD_f1_message",
                "SENTINEL_SD_m2_title",
            ],
        ),
        (
            GapFinderPapers,
            &[
                "SENTINEL_SD_d1_title",
                "SENTINEL_SD_c1_text",
                "SENTINEL_SD_c2_text",
                "SENTINEL_SD_ck2",
                "SENTINEL_SD_reason",
            ],
        ),
        (
            ThesisAudits,
            &[
                "SENTINEL_SD_summary",
                "SENTINEL_SD_sentence_1",
                "SENTINEL_SD_err",
                "SENTINEL_SD_ref_title",
                "SENTINEL_SD_raw",
            ],
        ),
        (CachedReports, &["SENTINEL_SD_report_fresh"]),
        (
            ExpiredCache,
            &[
                "SENTINEL_SD_report_old",
                "SENTINEL_SD_report_edge",
                "SENTINEL_SD_api_old",
            ],
        ),
    ];

    fn file_bytes(s: &Scratch, name: &str) -> Vec<u8> {
        match std::fs::read(s.app().join(name)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => panic!("{name}: {e}"),
        }
    }

    fn holds(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    /// Whether `gaply.db` and `gaply.db-wal` hold `needle`.
    fn in_files(s: &Scratch, needle: &[u8]) -> (bool, bool) {
        (
            holds(&file_bytes(s, "gaply.db"), needle),
            holds(&file_bytes(s, "gaply.db-wal"), needle),
        )
    }

    #[test]
    fn after_the_compaction_the_files_hold_none_of_the_deleted_text_and_all_the_rest() {
        for (c, gone) in DB_SENTINELS {
            let s = Scratch::new("compact");
            let db = s.open();
            plant(&s, &db);
            // A second pooled connection, open throughout, beside the
            // Database itself.
            let second = db.conn().unwrap();
            // Control: compacted before the delete, the files hold every
            // sentinel, so their absence below is the delete's doing.
            assert_eq!(db.stored_data_compact(), Compaction::Done);
            for (_, all) in DB_SENTINELS {
                for n in *all {
                    assert_eq!(in_files(&s, n.as_bytes()), (true, false), "control: {n}");
                }
            }

            delete(&s, &db, *c);
            assert_eq!(db.stored_data_compact(), Compaction::Done, "{c:?}");
            for n in *gone {
                assert_eq!(
                    in_files(&s, n.as_bytes()),
                    (false, false),
                    "{c:?}: {n} is still in gaply.db or its -wal"
                );
            }
            for (other, kept) in DB_SENTINELS {
                if other != c {
                    for n in *kept {
                        assert!(
                            in_files(&s, n.as_bytes()).0,
                            "{c:?}'s delete lost {other:?}'s {n}"
                        );
                    }
                }
            }
            drop(second);
        }
    }

    #[test]
    fn a_deleted_papers_vector_bytes_leave_the_files_too() {
        let s = Scratch::new("vector");
        let db = s.open();
        plant(&s, &db);
        // A vector whose bytes spell a sentinel. Each 4 bytes are ASCII, so
        // each float's exponent is below all-ones: no NaN, no infinity.
        let marker = b"SENTINEL_VEC_5d1b";
        let v: Vec<u8> = marker.iter().copied().cycle().take(384 * 4).collect();
        let d9 = exec(
            &db,
            "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
             VALUES ('research_paper', '[s/p9] p', 'gapfinder://s/p9', 1, 'ck9', 'ingested', 1)",
            &[],
        );
        let c9 = exec(
            &db,
            "INSERT INTO chunks (document_id, seq, content, token_count, created_at) VALUES (?1, 0, 'x', 1, 1)",
            &[&d9],
        );
        exec(
            &db,
            "INSERT INTO embeddings (embedding, source_type, source_id) VALUES (?1, 'chunk', ?2)",
            &[&v, &c9],
        );
        assert_eq!(db.stored_data_compact(), Compaction::Done);
        assert_eq!(
            in_files(&s, marker),
            (true, false),
            "control: the vector is in gaply.db"
        );

        delete(&s, &db, GapFinderPapers);
        assert_eq!(db.stored_data_compact(), Compaction::Done);
        assert_eq!(
            in_files(&s, marker),
            (false, false),
            "a deleted paper's vector bytes are still in gaply.db or its -wal"
        );
    }

    #[test]
    fn the_cascades_of_categories_1_2_and_3_reach_their_category_and_no_further() {
        // 1: the manuscripts take their extractions and findings.
        let s = Scratch::new("cascade-1");
        let db = s.open();
        plant(&s, &db);
        assert_eq!(db.count_rows("extractions").unwrap(), 1);
        assert_eq!(db.count_rows("findings").unwrap(), 2);
        delete(&s, &db, ManuscriptAnalyses);
        for t in ["manuscripts", "extractions", "findings"] {
            assert_eq!(db.count_rows(t).unwrap(), 0, "{t}");
        }

        // 2: the papers' embeddings by rowid, then the documents, which take
        // their chunks and their citation links; a staged source linked to
        // one keeps its row, its link set NULL.
        let s = Scratch::new("cascade-2");
        let db = s.open();
        plant(&s, &db);
        link_papers(&db);
        let gap = gap_paper(&db);
        let staged = one(&db, "SELECT id FROM audit_staged_sources", &[]);
        exec(
            &db,
            "UPDATE audit_staged_sources SET document_id = ?1, matched_by = 'doi' WHERE id = ?2",
            &[&gap, &staged],
        );
        let documents = db.count_rows("documents").unwrap();
        let d = delete(&s, &db, GapFinderPapers);
        assert_eq!(d.deleted, 2);
        assert_eq!(db.count_rows("documents").unwrap(), documents - 2);
        assert_eq!(
            one(
                &db,
                "SELECT COUNT(*) FROM chunks WHERE content IN ('SENTINEL_SD_c1_text', 'SENTINEL_SD_c2_text')",
                &[]
            ),
            0
        );
        assert_eq!(
            db.count_rows("chunks").unwrap(),
            2,
            "the guideline's and the other paper's"
        );
        let conn = db.conn().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT e.source_type, c.content FROM embeddings e
                 LEFT JOIN chunks c ON c.id = e.source_id ORDER BY e.source_type, c.content",
            )
            .unwrap();
        let left: Vec<(String, Option<String>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        drop(stmt);
        drop(conn);
        assert_eq!(
            left,
            [
                ("chunk".to_string(), Some("SENTINEL_SD_c3_text".to_string())),
                ("chunk".to_string(), Some("SENTINEL_SD_c5_text".to_string())),
                // Another source type with a deleted chunk's id: not the
                // category's, and kept.
                ("manuscript_chunk".to_string(), None),
            ]
        );
        assert_eq!(
            one(&db, "SELECT COUNT(*) FROM citation_documents", &[]),
            1,
            "only the paper document's link is left"
        );
        assert_eq!(
            one(&db, "SELECT document_id FROM citation_documents", &[]),
            linked_paper(&db)
        );
        assert_eq!(
            db.count_rows("audit_staged_sources").unwrap(),
            1,
            "the row stays"
        );
        assert_eq!(
            one(
                &db,
                "SELECT COUNT(*) FROM audit_staged_sources WHERE document_id IS NULL AND title = 'SENTINEL_SD_ref_title'",
                &[]
            ),
            1,
            "its link is set NULL (ON DELETE SET NULL)"
        );

        // 3: the jobs take their items and staged sources, and nothing else.
        let s = Scratch::new("cascade-3");
        let db = s.open();
        plant(&s, &db);
        delete(&s, &db, ThesisAudits);
        for t in ["ai_jobs", "ai_job_items", "audit_staged_sources"] {
            assert_eq!(db.count_rows(t).unwrap(), 0, "{t}");
        }
        assert_eq!(db.count_rows("ai_chunks").unwrap(), 1);
        assert_eq!(db.count_rows("ai_evidence_cards").unwrap(), 1);
    }

    #[test]
    fn category_2_is_refused_while_a_citation_check_references_one_of_its_papers() {
        for (what, sql) in [
            (
                "ai_chunks",
                "INSERT INTO ai_chunks (document_id, char_start, char_end, content, token_estimate,
                                        content_hash, created_at)
                 VALUES (?1, 0, 1, 'x', 1, 'SENTINEL_SD_h2', 1)",
            ),
            (
                "ai_evidence_cards",
                "INSERT INTO ai_evidence_cards (document_id, claim, evidence_text, evidence_type, model_id,
                                                model_version, prompt_version, created_at)
                 VALUES (?1, 'c', 'e', 'quote', 'gen', 'q', 'p', 1)",
            ),
            (
                "ai_jobs",
                "INSERT INTO ai_jobs (kind, status, prompt_version, created_at, document_id)
                 VALUES ('thesis_audit', 'done', 'v1', 1, ?1)",
            ),
        ] {
            let s = Scratch::new("in-use");
            let db = s.open();
            plant(&s, &db);
            let gap = gap_paper(&db);
            exec(&db, sql, &[&gap]);
            let before = dump(&db);
            match db.stored_data_delete(GapFinderPapers, &s.app(), &s.tmp(), NOW) {
                Err(DeleteError::PapersInUse) => {}
                other => panic!("{what}: {other:?}"),
            }
            assert_eq!(dump(&db), before, "{what}: nothing deleted");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_delete_neither_follows_nor_removes_a_symlink() {
        use std::os::unix::fs::symlink;
        let s = Scratch::new("delete-links");
        let db = s.open();
        s.write("outside/big.bin", &[1u8; 5000]);
        s.write("outside/papers/real.pdf", &[1u8; 4000]);
        let big = s.root.join("outside/big.bin");
        std::fs::create_dir_all(s.app().join("note-images")).unwrap();
        // A link in a counted folder, of each file category...
        symlink(&big, s.app().join(COMPARISON_LOG_FILE)).unwrap();
        symlink(&big, s.tmp().join("gaply-aicheck-1.html")).unwrap();
        symlink(&big, s.app().join("note-images/zzz.png")).unwrap();
        // ... and a counted folder that is itself a link.
        symlink(s.root.join("outside/papers"), s.app().join(OA_PAPERS_DIR)).unwrap();
        let before = snapshot(&s.root);

        for c in [
            ComparisonLog,
            OpenAccessPapers,
            TemporaryExports,
            OrphanNoteImages,
        ] {
            let d = delete(&s, &db, c);
            assert_eq!((d.deleted, d.removal_failed), (0, false), "{c:?}");
        }
        let after = snapshot(&s.root);
        for k in [
            "app/box4_comparisons.jsonl",
            "tmp/gaply-aicheck-1.html",
            "app/note-images/zzz.png",
            "app/oa_papers",
            "outside/big.bin",
            "outside/papers/real.pdf",
        ] {
            assert_eq!(after.get(k), before.get(k), "{k}");
            assert!(after.contains_key(k), "{k} is gone");
        }
        assert!(std::fs::symlink_metadata(s.app().join(OA_PAPERS_DIR))
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn a_note_image_changed_in_the_last_ten_minutes_is_neither_counted_nor_deleted() {
        let s = Scratch::new("recent");
        let db = s.open();
        for (name, at) in [
            ("old.png", NOW - 3600),
            // Exactly ten minutes ago: no longer recent.
            ("edge.png", NOW - RECENT_IMAGE_SECS),
            ("new.png", NOW - RECENT_IMAGE_SECS + 1),
            ("future.png", NOW + 60),
        ] {
            s.write(&format!("app/note-images/{name}"), &[9u8; 10]);
            s.age(&format!("app/note-images/{name}"), at);
        }
        let got = s.summary(&db, NOW).get(OrphanNoteImages);
        assert_eq!((got.items, got.bytes), (2, 20), "old and edge only");
        let d = delete(&s, &db, OrphanNoteImages);
        assert_eq!(d.deleted, 2);
        let mut left: Vec<String> = std::fs::read_dir(s.app().join("note-images"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["future.png", "new.png"]);
    }

    #[test]
    fn an_image_a_note_starts_using_before_its_removal_is_kept() {
        let note = |db: &Database, id: &str, body: &str| {
            exec(
                db,
                "INSERT INTO notes (id, note_type, title, body, created_at, updated_at)
                 VALUES (?1, 'project', 't', ?2, 1, 1)",
                &[&id, &body],
            );
        };

        // Between the summary and the delete.
        let s = Scratch::new("used-later");
        let db = s.open();
        s.write("app/note-images/bbb.png", &[9u8; 60]);
        s.age("app/note-images/bbb.png", NOW - 3600);
        assert_eq!(s.summary(&db, NOW).get(OrphanNoteImages).items, 1);
        note(&db, "n1", "![x](gaply-image://bbb.png)");
        let d = delete(&s, &db, OrphanNoteImages);
        assert_eq!((d.before.items, d.deleted), (0, 0));
        assert!(s.app().join("note-images/bbb.png").exists());

        // Between the delete's own listing and a removal: the first removal
        // makes both images used, so the second is asked again and kept.
        let s = Scratch::new("used-during");
        let db = s.open();
        for name in ["x.png", "y.png"] {
            s.write(&format!("app/note-images/{name}"), &[9u8; 10]);
            s.age(&format!("app/note-images/{name}"), NOW - 3600);
        }
        let first = std::cell::Cell::new(true);
        let remove = |p: &Path| {
            if first.replace(false) {
                note(&db, "n2", "gaply-image://x.png gaply-image://y.png");
            }
            std::fs::remove_file(p)
        };
        let d = db
            .delete_with(OrphanNoteImages, &s.app(), &s.tmp(), NOW, true, &remove)
            .unwrap();
        assert_eq!((d.before.items, d.deleted), (2, 1));
        let left = ["x.png", "y.png"]
            .iter()
            .filter(|n| s.app().join("note-images").join(n).exists())
            .count();
        assert_eq!(left, 1, "the image used by then is kept");
    }

    #[test]
    fn a_checkpoint_blocked_by_another_write_leaves_the_rows_deleted_and_answers_not_done() {
        let s = Scratch::new("checkpoint-write-busy");
        let db = s.open();
        plant(&s, &db);
        delete(&s, &db, CachedReports);
        let writer = db.conn().unwrap();
        writer
            .execute_batch(
                "BEGIN IMMEDIATE;
                 INSERT INTO cache (key, value, created_at, ttl_seconds) VALUES ('x', 'y', 1, 1);",
            )
            .unwrap();
        // A held write transaction keeps the TRUNCATE checkpoint off the WAL.
        assert_eq!(
            db.stored_data_compact(),
            Compaction::NotDone(CompactionCode::CheckpointBusy)
        );
        assert_eq!(
            s.summary(&db, NOW).get(CachedReports).items,
            0,
            "still deleted"
        );
        writer.execute_batch("ROLLBACK").unwrap();
        drop(writer);
        assert_eq!(
            db.stored_data_compact(),
            Compaction::Done,
            "control: unblocked"
        );
    }

    #[test]
    fn a_checkpoint_blocked_by_an_open_reader_is_finished_by_the_next_open() {
        // F2 (decision 224): a reader blocks the delete's checkpoint, the rows
        // stay deleted, and the next `Database::open` on the same file
        // finishes the checkpoint so the deleted text leaves both files. The
        // first Database and its pool stay open while the new one runs, and
        // both files are read before either is dropped.
        let s = Scratch::new("checkpoint-read-busy");
        let db = s.open();
        plant(&s, &db);
        let gone: &[&str] = &[
            "SENTINEL_SD_report_old",
            "SENTINEL_SD_report_edge",
            "SENTINEL_SD_api_old",
        ];
        delete(&s, &db, ExpiredCache);
        let reader = db.conn().unwrap();
        reader.execute_batch("BEGIN").unwrap();
        let _: i64 = reader
            .query_row("SELECT COUNT(*) FROM cache", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            db.stored_data_compact(),
            Compaction::NotDone(CompactionCode::CheckpointBusy)
        );
        assert_eq!(
            s.summary(&db, NOW).get(ExpiredCache).items,
            0,
            "still deleted"
        );
        reader.execute_batch("COMMIT").unwrap();
        drop(reader);
        // The next open's checkpoint (decision 224) finishes it, the first
        // Database still open.
        let db2 = s.open();
        for n in gone {
            assert_eq!(
                in_files(&s, n.as_bytes()),
                (false, false),
                "{n} is still in a file after the next open's checkpoint"
            );
        }
        drop(db2);
        drop(db);
    }

    #[test]
    fn without_secure_delete_the_deleted_text_stays_in_the_file() {
        // The red half of F2's secure-delete check (decision 224): with
        // `secure_delete` off, the delete frees the pages without zeroing
        // them, so the deleted text is still in `gaply.db` after the
        // checkpoint — there is no VACUUM to drop the free pages.
        let needle = b"SENTINEL_SD_report_fresh";
        // Control: secure (the shipped path) — gone from both files.
        let s = Scratch::new("secure");
        let db = s.open();
        plant(&s, &db);
        db.delete_with(CachedReports, &s.app(), &s.tmp(), NOW, true, &|p| {
            std::fs::remove_file(p)
        })
        .unwrap();
        assert_eq!(db.stored_data_compact(), Compaction::Done);
        assert_eq!(in_files(&s, needle), (false, false), "secure: gone");
        // Without secure_delete: the deleted text stays in gaply.db.
        let s2 = Scratch::new("insecure");
        let db2 = s2.open();
        plant(&s2, &db2);
        db2.delete_with(CachedReports, &s2.app(), &s2.tmp(), NOW, false, &|p| {
            std::fs::remove_file(p)
        })
        .unwrap();
        assert_eq!(db2.stored_data_compact(), Compaction::Done);
        assert!(
            in_files(&s2, needle).0,
            "without secure_delete the deleted text stays in gaply.db"
        );
    }

    #[test]
    fn a_failure_inside_the_transaction_leaves_the_category_as_it_was() {
        for (c, table) in [
            (ManuscriptAnalyses, "findings"),
            // After the embeddings went: their delete is undone too.
            (GapFinderPapers, "chunks"),
            (ThesisAudits, "audit_staged_sources"),
            (CachedReports, "cache"),
        ] {
            let s = Scratch::new("rollback");
            let db = s.open();
            plant(&s, &db);
            exec(
                &db,
                &format!(
                    "CREATE TRIGGER planted_failure BEFORE DELETE ON {table}
                     BEGIN SELECT RAISE(ABORT, 'planted'); END"
                ),
                &[],
            );
            let before = dump(&db);
            let summary = s.summary(&db, NOW);
            match db.stored_data_delete(c, &s.app(), &s.tmp(), NOW) {
                Err(DeleteError::Failed(_)) => {}
                other => panic!("{c:?}: {other:?}"),
            }
            assert_eq!(dump(&db), before, "{c:?}");
            assert_eq!(s.summary(&db, NOW), summary, "{c:?}");
        }
    }

    #[test]
    fn a_failed_removal_stops_the_delete_and_says_how_many_were_removed() {
        for c in [
            ComparisonLog,
            OpenAccessPapers,
            TemporaryExports,
            OrphanNoteImages,
        ] {
            let s = Scratch::new("remove-fails");
            let db = s.open();
            plant(&s, &db);
            // A second unused image, so category 9 has two.
            s.write("app/note-images/eee.png", &[9u8; 30]);
            s.age("app/note-images/eee.png", NOW - 3600);
            let calls = std::cell::Cell::new(0);
            // The log is one file: its first removal fails. The others have
            // two: the second fails.
            let fail_at = if c == ComparisonLog { 1 } else { 2 };
            let remove = |p: &Path| {
                calls.set(calls.get() + 1);
                if calls.get() == fail_at {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "planted",
                    ))
                } else {
                    std::fs::remove_file(p)
                }
            };
            let d = db
                .delete_with(c, &s.app(), &s.tmp(), NOW, true, &remove)
                .unwrap();
            let removed = fail_at - 1;
            assert!(d.removal_failed, "{c:?}");
            assert_eq!(d.deleted, removed, "{c:?}");
            assert_eq!(d.after.items, d.before.items - removed, "{c:?}");
            assert_eq!(calls.get(), fail_at, "{c:?}: it stopped at the failure");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_it_cannot_change_fails_the_first_removal_and_removes_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("readonly-folder");
        let db = s.open();
        plant(&s, &db);
        let dir = s.app().join(OA_PAPERS_DIR);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let got = db.stored_data_delete(OpenAccessPapers, &s.app(), &s.tmp(), NOW);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let d = got.unwrap();
        assert!(d.removal_failed);
        assert_eq!((d.before.items, d.deleted, d.after.items), (2, 0, 2));
    }

    #[test]
    fn the_wire_names_are_serdes_and_parse_back() {
        for c in StoredCategory::ALL {
            assert_eq!(
                serde_json::to_value(c).unwrap(),
                serde_json::Value::String(c.wire().into())
            );
            assert_eq!(StoredCategory::parse(c.wire()), Some(c));
        }
        assert_eq!(StoredCategory::parse("manuscripts"), None);
        let in_db: Vec<StoredCategory> = StoredCategory::ALL
            .into_iter()
            .filter(|c| c.in_database())
            .collect();
        assert_eq!(in_db, StoredCategory::ALL[..5]);
    }
}
