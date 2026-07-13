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
}
