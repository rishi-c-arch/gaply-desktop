//! Versioned schema migrations with up/down pairs.
//!
//! Applied versions are tracked in `schema_migrations`. Migrations run in
//! transactions; `migrate_up` is idempotent and safe to call on every start.

use rusqlite::Connection;

use crate::error::GaplyError;

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub up: &'static str,
    pub down: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "projects",
        // IF NOT EXISTS: adopts dev databases created before the migration
        // runner existed (identical schema, no schema_migrations table).
        up: "
            CREATE TABLE IF NOT EXISTS projects (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL
            );
        ",
        down: "DROP TABLE projects;",
    },
    Migration {
        version: 2,
        name: "manuscript_pipeline",
        up: "
            CREATE TABLE manuscripts (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
                title      TEXT NOT NULL,
                authors    TEXT NOT NULL DEFAULT '',
                abstract   TEXT NOT NULL DEFAULT '',
                file_path  TEXT,
                status     TEXT NOT NULL DEFAULT 'imported',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE extractions (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                manuscript_id INTEGER NOT NULL REFERENCES manuscripts(id) ON DELETE CASCADE,
                kind          TEXT NOT NULL,
                content       TEXT NOT NULL,
                created_at    INTEGER NOT NULL
            );
            CREATE TABLE findings (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                manuscript_id INTEGER NOT NULL REFERENCES manuscripts(id) ON DELETE CASCADE,
                extraction_id INTEGER REFERENCES extractions(id) ON DELETE SET NULL,
                severity      TEXT NOT NULL,
                category      TEXT NOT NULL,
                message       TEXT NOT NULL,
                created_at    INTEGER NOT NULL
            );
            CREATE INDEX idx_extractions_manuscript ON extractions(manuscript_id);
            CREATE INDEX idx_findings_manuscript ON findings(manuscript_id);
        ",
        down: "
            DROP TABLE findings;
            DROP TABLE extractions;
            DROP TABLE manuscripts;
        ",
    },
    Migration {
        version: 3,
        name: "knowledge_base",
        up: "
            CREATE TABLE journal_guidelines (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                journal_name TEXT NOT NULL UNIQUE,
                issn         TEXT,
                url          TEXT,
                content      TEXT NOT NULL,
                fetched_at   INTEGER NOT NULL
            );
            CREATE TABLE retractions (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                doi             TEXT NOT NULL UNIQUE,
                title           TEXT NOT NULL DEFAULT '',
                reason          TEXT NOT NULL DEFAULT '',
                retraction_date TEXT,
                source          TEXT NOT NULL DEFAULT '',
                created_at      INTEGER NOT NULL
            );
            -- SUPERSEDED (reserved, intentionally unused): CSL styles ship as a frontend
            -- bundle (public/csl/, ~2,856 styles) rendered by citeproc-js — no Rust reads
            -- this table. Kept, not dropped: migrations are append-only, so removal would
            -- need a new DROP migration on every existing DB for zero functional gain.
            CREATE TABLE reference_styles (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                name       TEXT NOT NULL UNIQUE,
                csl        TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
        ",
        down: "
            DROP TABLE reference_styles;
            DROP TABLE retractions;
            DROP TABLE journal_guidelines;
        ",
    },
    Migration {
        version: 4,
        name: "embeddings_vec0",
        // vec0 virtual table (sqlite-vec). 384 dims = all-MiniLM-L6-v2.
        // `+column` = auxiliary columns stored alongside each vector.
        up: "
            CREATE VIRTUAL TABLE embeddings USING vec0(
                embedding FLOAT[384],
                +source_type TEXT,
                +source_id INTEGER
            );
        ",
        down: "DROP TABLE embeddings;",
    },
    Migration {
        version: 5,
        name: "episodic_memory_and_cache",
        up: "
            CREATE TABLE episodic_memory (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                agent      TEXT NOT NULL,
                decision   TEXT NOT NULL,
                context    TEXT NOT NULL DEFAULT '',
                outcome    TEXT,
                correction TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX idx_episodic_agent_time ON episodic_memory(agent, created_at);
            CREATE TABLE cache (
                key         TEXT PRIMARY KEY,
                value       TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                ttl_seconds INTEGER NOT NULL
            );
        ",
        down: "
            DROP TABLE cache;
            DROP TABLE episodic_memory;
        ",
    },
    Migration {
        version: 6,
        name: "rag_documents",
        up: "
            CREATE TABLE documents (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type       TEXT NOT NULL,
                title             TEXT NOT NULL,
                source_url        TEXT NOT NULL DEFAULT '',
                fetched_at        INTEGER NOT NULL,
                checksum          TEXT NOT NULL UNIQUE,
                status            TEXT NOT NULL,
                quarantine_reason TEXT,
                created_at        INTEGER NOT NULL
            );
            CREATE TABLE chunks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                seq         INTEGER NOT NULL,
                content     TEXT NOT NULL,
                token_count INTEGER NOT NULL,
                created_at  INTEGER NOT NULL,
                UNIQUE (document_id, seq)
            );
            CREATE INDEX idx_chunks_document ON chunks(document_id);
        ",
        down: "
            DROP TABLE chunks;
            DROP TABLE documents;
        ",
    },
    Migration {
        version: 7,
        name: "citation_library_local",
        // Citation Manager Set 4: the LOCAL-FIRST reference library. Local
        // sqlite is the source of truth; Supabase sync is an optional layer.
        // csl_json holds the VERIFIED CSL-JSON (Set 2); title/authors/year
        // are search columns DERIVED from it at write time (deterministic,
        // never invented); tags is a JSON string array; sync_status is the
        // honest marker ('local_only' | 'pending' | 'synced').
        up: "
            CREATE TABLE citation_library (
                id          TEXT PRIMARY KEY,
                csl_json    TEXT NOT NULL,
                doi         TEXT,
                title       TEXT NOT NULL DEFAULT '',
                authors     TEXT NOT NULL DEFAULT '',
                year        INTEGER,
                tags        TEXT NOT NULL DEFAULT '[]',
                sync_status TEXT NOT NULL DEFAULT 'local_only',
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            );
            CREATE INDEX idx_citation_library_doi ON citation_library(doi);
            CREATE INDEX idx_citation_library_year ON citation_library(year);
        ",
        down: "DROP TABLE citation_library;",
    },
    Migration {
        version: 8,
        name: "plagiarism_library",
        // Plagiarism Check Set 3: the durable, user-curated "my papers" store
        // for DETERMINISTIC exact-match comparison — NOT session-tagged, NOT
        // mixed with journal/RAG docs (that shared corpus is the embedding
        // lane's stopgap). full_text is the paper's extracted text; fingerprints
        // is the Set-2 winnowing fingerprint set, serialized (computed ONCE at
        // add time, reused every check — no re-fingerprinting per run).
        up: "
            CREATE TABLE plagiarism_library (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                title        TEXT NOT NULL,
                full_text    TEXT NOT NULL,
                fingerprints TEXT NOT NULL,
                source_label TEXT NOT NULL DEFAULT '',
                added_at     INTEGER NOT NULL
            );
            CREATE INDEX idx_plagiarism_library_added ON plagiarism_library(added_at);
        ",
        down: "DROP TABLE plagiarism_library;",
    },
    Migration {
        version: 9,
        name: "notes",
        // Note Creator Set 2: the free, local-first notes store. TWO note types
        // in ONE table — 'paper' (a structured template attached to a paper) and
        // 'project' (the researcher's own ideas / hypotheses / to-dos).
        //
        // paper_id is a SOFT anchor (by convention → citation_library.id) with
        // DELIBERATELY NO foreign-key constraint: a note is the researcher's OWN
        // work and MUST survive deletion of the paper it annotates — nothing
        // cascades. paper_title is denormalized so the note stays self-sufficient
        // and still displays its paper even if the library row is gone.
        //
        // fields_json holds the optional per-paper template fields (citation,
        // research_question, methodology, key_findings, notable_quotes, …) as
        // flexible JSON — no rigid columns, so the template can evolve without a
        // migration; '{}' for project notes. tags (JSON array) and sync_status
        // (the honest 'local_only'|'pending'|'synced' marker — sync is DEFERRED,
        // unused for now) mirror citation_library.
        up: "
            CREATE TABLE notes (
                id          TEXT PRIMARY KEY,
                note_type   TEXT NOT NULL,
                paper_id    TEXT,
                paper_title TEXT NOT NULL DEFAULT '',
                title       TEXT NOT NULL DEFAULT '',
                fields_json TEXT NOT NULL DEFAULT '{}',
                body        TEXT NOT NULL DEFAULT '',
                tags        TEXT NOT NULL DEFAULT '[]',
                sync_status TEXT NOT NULL DEFAULT 'local_only',
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            );
            CREATE INDEX idx_notes_paper ON notes(paper_id);
            CREATE INDEX idx_notes_type ON notes(note_type);
        ",
        down: "DROP TABLE notes;",
    },
    Migration {
        version: 10,
        name: "plagiarism_library_citation_link",
        // M2 Set 2A: the reliable link between the plagiarism "my papers" store
        // and the citation library, so Note Creator's side-by-side can resolve by
        // a shared id (exact) instead of only by title (ambiguous).
        //
        // ADDITIVE + NULLABLE: a nullable `citation_id TEXT` (soft anchor →
        // citation_library.id, mirroring notes.paper_id — DELIBERATELY no FK, so
        // deleting the citation never cascades into the user's paper store).
        // Existing rows get citation_id = NULL (zero data loss); the read path
        // treats NULL as "no id link" and falls back to the Set-1 title match.
        // NOT unique — a user may associate two uploads with one citation, so the
        // id read is exactly-one-guarded (see full_text_by_citation_id), never a
        // guess. This is the FIRST ALTER migration (all prior are CREATE/DROP
        // TABLE); the up/down is exercised by a dedicated test below.
        up: "
            ALTER TABLE plagiarism_library ADD COLUMN citation_id TEXT;
            CREATE INDEX idx_plagiarism_library_citation ON plagiarism_library(citation_id);
        ",
        down: "
            DROP INDEX idx_plagiarism_library_citation;
            ALTER TABLE plagiarism_library DROP COLUMN citation_id;
        ",
    },
    Migration {
        version: 11,
        name: "citation_library_verification_persist",
        // Citation Manager Scope B: persist the VERIFICATION + RETRACTION facts
        // that were session-only and vanished on reload. The top bug this fixes:
        // a CrossRef+Retraction-Watch-confirmed retracted paper read CLEAN after
        // a restart because the frontend storedToCitation hardcoded retracted:false.
        // These are EXTERNAL facts (from refverify), NOT derivable from csl_json,
        // so they get their own columns.
        //
        // ADDITIVE + SAFE: every column is nullable or NOT NULL DEFAULT 0 (a
        // constant SQLite applies to existing rows). No UNIQUE, no backfill, no
        // rewrite — existing rows read back retracted=0 / unverified, identical to
        // today's behavior (zero regression). It CANNOT half-apply into corruption:
        // nothing existing is modified, and migrate_up is version-gated so a re-run
        // resumes cleanly. The DOI-normalized upsert guard is DELIBERATELY a
        // separate later migration (a UNIQUE index can abort on pre-existing
        // duplicate DOIs — a different risk needing a dedup-existing-rows step).
        up: "
            ALTER TABLE citation_library ADD COLUMN retracted INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE citation_library ADD COLUMN source TEXT;
            ALTER TABLE citation_library ADD COLUMN verify_provenance TEXT;
            ALTER TABLE citation_library ADD COLUMN verify_outcome TEXT;
            ALTER TABLE citation_library ADD COLUMN verified_at INTEGER;
        ",
        down: "
            ALTER TABLE citation_library DROP COLUMN verified_at;
            ALTER TABLE citation_library DROP COLUMN verify_outcome;
            ALTER TABLE citation_library DROP COLUMN verify_provenance;
            ALTER TABLE citation_library DROP COLUMN source;
            ALTER TABLE citation_library DROP COLUMN retracted;
        ",
    },
];

pub fn latest_version() -> i64 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

fn ensure_migrations_table(conn: &Connection) -> Result<(), GaplyError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;
    Ok(())
}

pub fn current_version(conn: &Connection) -> Result<i64, GaplyError> {
    ensure_migrations_table(conn)?;
    let v = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    Ok(v)
}

/// Apply all pending migrations. Returns the names of those applied.
pub fn migrate_up(conn: &mut Connection) -> Result<Vec<&'static str>, GaplyError> {
    let current = current_version(conn)?;
    let mut applied = Vec::new();
    for m in MIGRATIONS.iter().filter(|m| m.version > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.up)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![m.version, m.name, crate::now_epoch()],
        )?;
        tx.commit()?;
        tracing::info!(version = m.version, name = m.name, "migration applied");
        applied.push(m.name);
    }
    Ok(applied)
}

/// Roll back migrations until schema version equals `target`.
/// Returns the names of migrations rolled back.
pub fn migrate_down(conn: &mut Connection, target: i64) -> Result<Vec<&'static str>, GaplyError> {
    let current = current_version(conn)?;
    let mut reverted = Vec::new();
    for m in MIGRATIONS
        .iter()
        .rev()
        .filter(|m| m.version <= current && m.version > target)
    {
        let tx = conn.transaction()?;
        tx.execute_batch(m.down)?;
        tx.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            rusqlite::params![m.version],
        )?;
        tx.commit()?;
        tracing::info!(version = m.version, name = m.name, "migration rolled back");
        reverted.push(m.name);
    }
    Ok(reverted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_connection;

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn up_from_scratch_reaches_latest_with_all_tables() {
        let mut conn = test_connection();
        let applied = migrate_up(&mut conn).unwrap();
        assert_eq!(applied.len(), MIGRATIONS.len());
        assert_eq!(current_version(&conn).unwrap(), latest_version());

        let tables = table_names(&conn);
        for expected in [
            "projects",
            "manuscripts",
            "extractions",
            "findings",
            "journal_guidelines",
            "retractions",
            "reference_styles",
            "embeddings",
            "episodic_memory",
            "cache",
            "documents",
            "chunks",
        ] {
            assert!(tables.iter().any(|t| t == expected), "missing table {expected}: {tables:?}");
        }
    }

    #[test]
    fn up_is_idempotent() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        let second = migrate_up(&mut conn).unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn down_to_zero_removes_everything() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        let reverted = migrate_down(&mut conn, 0).unwrap();
        assert_eq!(reverted.len(), MIGRATIONS.len());
        assert_eq!(current_version(&conn).unwrap(), 0);

        let tables = table_names(&conn);
        assert!(
            tables.iter().all(|t| t == "schema_migrations" || t.starts_with("sqlite_")),
            "leftover tables: {tables:?}"
        );
    }

    #[test]
    fn partial_down_keeps_earlier_versions() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        // roll back notes + plagiarism library + citations + rag + memory/cache
        // + embeddings, keep knowledge base and below
        let reverted = migrate_down(&mut conn, 3).unwrap();
        assert_eq!(
            reverted,
            vec![
                "citation_library_verification_persist",
                "plagiarism_library_citation_link",
                "notes",
                "plagiarism_library",
                "citation_library_local",
                "rag_documents",
                "episodic_memory_and_cache",
                "embeddings_vec0"
            ]
        );
        assert_eq!(current_version(&conn).unwrap(), 3);

        let tables = table_names(&conn);
        assert!(tables.iter().any(|t| t == "retractions"));
        assert!(!tables.iter().any(|t| t == "embeddings"));
        assert!(!tables.iter().any(|t| t == "cache"));
    }

    #[test]
    fn down_then_up_restores_schema() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        migrate_down(&mut conn, 0).unwrap();
        let reapplied = migrate_up(&mut conn).unwrap();
        assert_eq!(reapplied.len(), MIGRATIONS.len());
        assert!(table_names(&conn).iter().any(|t| t == "embeddings"));
    }

    // --- M2 Set 2A: the first ALTER migration (v10) ---

    fn column_names(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(1)) // col 1 = name
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    fn index_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    }

    #[test]
    fn v10_adds_nullable_citation_id_column_and_index_additively() {
        // Simulate an EXISTING db that predates v10: migrate up to 9, insert a
        // plagiarism_library row, THEN apply v10 — the row must survive with
        // citation_id = NULL (additive, no data loss).
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap(); // reaches latest, but we re-check post-state
        // roll back to v9 (peels v11 + v10) to stand with a pre-existing row
        migrate_down(&mut conn, 9).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 9);
        assert!(!column_names(&conn, "plagiarism_library").iter().any(|c| c == "citation_id"));
        conn.execute(
            "INSERT INTO plagiarism_library (title, full_text, fingerprints, source_label, added_at)
             VALUES ('Old Paper', 'body', '[]', '/old.pdf', 1)",
            [],
        )
        .unwrap();

        // re-apply: v10 adds citation_id (the subject here); v11 rides along and
        // does not touch plagiarism_library. The pre-existing row survives either way.
        let applied = migrate_up(&mut conn).unwrap();
        assert_eq!(applied, vec!["plagiarism_library_citation_link", "citation_library_verification_persist"]);
        assert_eq!(current_version(&conn).unwrap(), 11);

        // the column + index now exist; the pre-existing row is intact, NULL id
        assert!(column_names(&conn, "plagiarism_library").iter().any(|c| c == "citation_id"));
        assert!(index_exists(&conn, "idx_plagiarism_library_citation"));
        let (title, cid): (String, Option<String>) = conn
            .query_row(
                "SELECT title, citation_id FROM plagiarism_library WHERE title = 'Old Paper'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(title, "Old Paper");
        assert_eq!(cid, None, "pre-existing rows backfill to NULL, not a guessed link");
    }

    #[test]
    fn v10_down_removes_the_column_and_index_reversibly() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        assert!(column_names(&conn, "plagiarism_library").iter().any(|c| c == "citation_id"));
        assert!(index_exists(&conn, "idx_plagiarism_library_citation"));

        // peel v11 off first so this test stays isolated to v10's down.
        migrate_down(&mut conn, 10).unwrap();
        // down one more: v10's column + index are gone, the table remains
        let reverted = migrate_down(&mut conn, 9).unwrap();
        assert_eq!(reverted, vec!["plagiarism_library_citation_link"]);
        assert!(!column_names(&conn, "plagiarism_library").iter().any(|c| c == "citation_id"));
        assert!(!index_exists(&conn, "idx_plagiarism_library_citation"));
        assert!(table_names(&conn).iter().any(|t| t == "plagiarism_library"));

        // and it re-applies cleanly (idempotent up after a partial down): v10 + v11
        let reapplied = migrate_up(&mut conn).unwrap();
        assert_eq!(reapplied, vec!["plagiarism_library_citation_link", "citation_library_verification_persist"]);
        assert!(column_names(&conn, "plagiarism_library").iter().any(|c| c == "citation_id"));
    }

    #[test]
    fn v11_adds_five_verification_columns_additively_no_data_loss() {
        // Simulate an EXISTING db that predates v11: stand at v10 with a citation
        // row, THEN apply v11 — the row must survive with retracted defaulting to
        // 0 and the verification columns NULL (additive, no rewrite).
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        migrate_down(&mut conn, 10).unwrap(); // roll back the single v11 → stand at v10
        assert_eq!(current_version(&conn).unwrap(), 10);
        for c in ["retracted", "source", "verify_provenance", "verify_outcome", "verified_at"] {
            assert!(!column_names(&conn, "citation_library").iter().any(|n| n == c), "{c} should not exist yet");
        }
        conn.execute(
            "INSERT INTO citation_library (id, csl_json, doi, title, authors, year, tags, sync_status, created_at, updated_at)
             VALUES ('old', '{}', NULL, 'Old Ref', '', NULL, '[]', 'local_only', 1, 1)",
            [],
        )
        .unwrap();

        // apply v11
        let applied = migrate_up(&mut conn).unwrap();
        assert_eq!(applied, vec!["citation_library_verification_persist"]);
        assert_eq!(current_version(&conn).unwrap(), 11);
        for c in ["retracted", "source", "verify_provenance", "verify_outcome", "verified_at"] {
            assert!(column_names(&conn, "citation_library").iter().any(|n| n == c), "missing column {c}");
        }

        // the pre-existing row is intact: retracted defaulted to 0, verify cols NULL
        let (title, retracted, source, prov, outcome, vat): (String, i64, Option<String>, Option<String>, Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT title, retracted, source, verify_provenance, verify_outcome, verified_at
                 FROM citation_library WHERE id = 'old'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .unwrap();
        assert_eq!(title, "Old Ref");
        assert_eq!(retracted, 0, "existing row is not-retracted by default — no regression");
        assert_eq!((source, prov, outcome, vat), (None, None, None, None), "verify cols NULL, no guess");
    }

    #[test]
    fn v11_down_removes_the_five_columns_reversibly() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        for c in ["retracted", "source", "verify_provenance", "verify_outcome", "verified_at"] {
            assert!(column_names(&conn, "citation_library").iter().any(|n| n == c));
        }

        let reverted = migrate_down(&mut conn, 10).unwrap();
        assert_eq!(reverted, vec!["citation_library_verification_persist"]);
        for c in ["retracted", "source", "verify_provenance", "verify_outcome", "verified_at"] {
            assert!(!column_names(&conn, "citation_library").iter().any(|n| n == c), "{c} should be dropped");
        }
        assert!(table_names(&conn).iter().any(|t| t == "citation_library"), "table itself remains");

        // re-applies cleanly (idempotent up after a partial down)
        let reapplied = migrate_up(&mut conn).unwrap();
        assert_eq!(reapplied, vec!["citation_library_verification_persist"]);
    }
}
