//! **What Gaply's older features keep on this computer, counted (Research
//! OS, Phase 5D-1a; decisions 109, 205 to 207).**
//!
//! The Stored data section in Settings shows nine categories, each with how
//! many items it holds and how many bytes. This module counts them, and it
//! only reads: it runs `SELECT`s, inside one deferred transaction so the nine
//! counts come from one snapshot, and it reads file sizes from metadata, never
//! a file's contents. The deletes are 5D-1b's.
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
//! 9. Orphan note images: a file in `note-images/` whose
//!    `gaply-image://<name>` appears in no note, asked through
//!    [`crate::notes::search_notes`], the same search the Note Creator's own
//!    clean-up uses.
//!
//! **Bytes.** For rows, the sum of `length(CAST(col AS BLOB))` over the TEXT
//! and BLOB columns of the rows counted: bytes, never characters. For files,
//! their sizes from metadata. Only regular files directly in a counted folder
//! count; a symlink, a counted folder's own or an entry's, is skipped and
//! never followed.
//!
//! The caller passes the app data folder, the temp folder and the time, so a
//! test never touches the real ones.

use std::path::Path;

use rusqlite::types::ToSql;
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

fn non_negative(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// (items, bytes) of one query from [`count_sql`].
fn count(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<(u64, u64), GaplyError> {
    let (items, bytes): (i64, i64) = conn.query_row(sql, params, |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok((non_negative(items), non_negative(bytes)))
}

/// The regular files directly in `dir` whose names `keep` accepts, as
/// (name, bytes). A missing folder has none. A symlink or a subfolder in it
/// is skipped, never followed; sizes come from metadata, never contents.
fn regular_files(
    dir: &Path,
    keep: impl Fn(&str) -> bool,
) -> Result<Vec<(String, u64)>, GaplyError> {
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
            Ok(m) => out.push((name, m.len())),
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
) -> Result<Vec<(String, u64)>, GaplyError> {
    let dir = root.join(name);
    match std::fs::symlink_metadata(&dir) {
        Ok(m) if m.is_dir() => regular_files(&dir, keep),
        Ok(_) => Ok(Vec::new()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

fn total(files: &[(String, u64)]) -> (u64, u64) {
    (files.len() as u64, files.iter().map(|(_, b)| b).sum())
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
        let comparison_log = match std::fs::symlink_metadata(app_data_dir.join(COMPARISON_LOG_FILE))
        {
            Ok(m) if m.is_file() => (1, m.len()),
            Ok(_) => (0, 0),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (0, 0),
            Err(e) => return Err(e.into()),
        };
        let oa_papers = total(&files_in_subfolder(app_data_dir, OA_PAPERS_DIR, |n| {
            OA_PAPER_SUFFIXES.iter().any(|s| n.ends_with(s))
        })?);
        let temp_exports = total(&regular_files(temp_dir, |n| {
            TEMP_EXPORTS
                .iter()
                .any(|(prefix, suffix)| n.starts_with(prefix) && n.ends_with(suffix))
        })?);
        let mut orphans = Vec::new();
        for (name, bytes) in files_in_subfolder(app_data_dir, NOTE_IMAGES_DIR, |_| true)? {
            let reference = format!("{NOTE_IMAGE_REF_PREFIX}{name}");
            if crate::notes::search_notes(self, &reference, None)?.is_empty() {
                orphans.push((name, bytes));
            }
        }
        let orphan_note_images = total(&orphans);

        // The rows, from one snapshot. The transaction only reads, and is
        // rolled back when it drops.
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let research_paper = crate::rag::SourceType::ResearchPaper.as_str();
        let gap: [&dyn ToSql; 2] = [&research_paper, &GAP_FINDER_URL_PREFIX];
        let in_gap_documents =
            format!("document_id IN (SELECT id FROM documents WHERE {GAP_FINDER_DOCUMENT})");

        let (manuscripts, m_bytes) = count(&tx, &count_sql("manuscripts", "1"), &[])?;
        let (_, e_bytes) = count(
            &tx,
            &count_sql(
                "extractions",
                "manuscript_id IN (SELECT id FROM manuscripts)",
            ),
            &[],
        )?;
        let (_, f_bytes) = count(
            &tx,
            &count_sql("findings", "manuscript_id IN (SELECT id FROM manuscripts)"),
            &[],
        )?;

        let (papers, d_bytes) = count(&tx, &count_sql("documents", GAP_FINDER_DOCUMENT), &gap)?;
        let (_, c_bytes) = count(&tx, &count_sql("chunks", &in_gap_documents), &gap)?;
        let (_, v_bytes) = count(
            &tx,
            &count_sql(
                "embeddings",
                &format!("source_type = ?3 AND source_id IN (SELECT id FROM chunks WHERE {in_gap_documents})"),
            ),
            &[&research_paper, &GAP_FINDER_URL_PREFIX, &EMBEDDING_SOURCE_CHUNK],
        )?;

        let (jobs, j_bytes) = count(&tx, &count_sql("ai_jobs", "1"), &[])?;
        let (_, i_bytes) = count(
            &tx,
            &count_sql("ai_job_items", "job_id IN (SELECT id FROM ai_jobs)"),
            &[],
        )?;
        let (_, s_bytes) = count(
            &tx,
            &count_sql("audit_staged_sources", "job_id IN (SELECT id FROM ai_jobs)"),
            &[],
        )?;

        let cached_reports = count(
            &tx,
            &count_sql(
                "cache",
                "substr(\"key\", 1, length(?1)) = ?1 AND created_at + ttl_seconds > ?2",
            ),
            &[&REPORT_KEY_PREFIX, &now],
        )?;
        let expired_cache = count(
            &tx,
            &count_sql("cache", "created_at + ttl_seconds <= ?1"),
            &[&now],
        )?;
        drop(tx);

        let counts = [
            (manuscripts, m_bytes + e_bytes + f_bytes),
            (papers, d_bytes + c_bytes + v_bytes),
            (jobs, j_bytes + i_bytes + s_bytes),
            cached_reports,
            expired_cache,
            comparison_log,
            oa_papers,
            temp_exports,
            orphan_note_images,
        ];
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

        // 9. Note images: aaa and ccc are used, bbb is not.
        s.write("app/note-images/aaa.png", &[9u8; 50]);
        s.write("app/note-images/bbb.png", &[9u8; 60]);
        s.write("app/note-images/ccc.png", &[9u8; 70]);
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

    /// Every file under `root` by path, with its SHA-256, and every folder.
    fn snapshot(root: &Path) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                let rel = p.strip_prefix(root).unwrap().display().to_string();
                let t = e.file_type().unwrap();
                if t.is_dir() {
                    out.insert(rel, "dir".into());
                    stack.push(p);
                } else if t.is_symlink() {
                    out.insert(
                        rel,
                        format!("link {}", std::fs::read_link(&p).unwrap().display()),
                    );
                } else {
                    let bytes = std::fs::read(&p).unwrap();
                    out.insert(rel, format!("{:x}", Sha256::digest(&bytes)));
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
        let before = snapshot(&s.root);
        assert!(before.contains_key("app/gaply.db"), "{before:?}");
        assert!(
            !before.contains_key("app/gaply.db-wal"),
            "every connection was closed: {before:?}"
        );

        let db = s.open();
        let got = s.summary(&db, NOW);
        drop(db);
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
}
