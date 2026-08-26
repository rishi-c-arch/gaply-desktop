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
    Migration {
        version: 12,
        name: "evidence_store",
        // Evidence Store (Box 3): persist EVERY EvidenceRecord per PublishReady run
        // (not just escalated ones), so the Orchestrator, reviewer synthesis and
        // Chat can reference a run's full evidence later. confidence_kind and
        // routing_hint are persisted — the honesty model must survive to the store,
        // never be re-derived downstream. Escalation columns are NULL until a
        // finding is escalated (Phase 2, evidence_record_escalation).
        //
        // RUN IDENTITY INVARIANT: a PublishReady run has exactly one run_id
        // (== manuscript_id). All escalation, evidence storage, reviewer synthesis,
        // and chat interactions for that run operate on this same identity.
        // Re-running analysis creates a new run_id.
        //
        // `verified` is THREE-STATE and NULL/0 MUST NEVER be conflated:
        //   NULL       = not escalated
        //   0 (false)  = escalated + gate-rejected
        //   1 (true)   = escalated + passed
        //
        // ADDITIVE + SAFE: brand-new table, no existing data touched, version-gated
        // re-run resumes cleanly. UNIQUE(run_id, finding_id) is the store's key.
        up: "
            CREATE TABLE evidence (
                run_id                  TEXT    NOT NULL,
                finding_id              TEXT    NOT NULL,
                agent                   TEXT    NOT NULL,
                severity                TEXT    NOT NULL,
                confidence              REAL    NOT NULL,
                confidence_kind         TEXT    NOT NULL,
                routing_hint            TEXT    NOT NULL,
                provenance              TEXT    NOT NULL,
                evidence_refs           TEXT    NOT NULL,
                limitations             TEXT,
                llm_verdict             TEXT,
                llm_rationale           TEXT,
                verified                INTEGER,
                gate_flags              TEXT,
                provider                TEXT,
                provider_model          TEXT,
                evidence_schema_version INTEGER NOT NULL,
                created_at              INTEGER NOT NULL,
                UNIQUE(run_id, finding_id)
            );
            CREATE INDEX idx_evidence_run ON evidence(run_id);
        ",
        down: "DROP TABLE evidence;",
    },
    Migration {
        version: 13,
        name: "evidence_claim_kind",
        // Claim identity (ARCHITECTURE_TRACE §26): WHAT editorial statement a
        // finding makes, as distinct from WHICH subsystem produced it. Run 22
        // measured the need — two findings describing Gaply's own execution were
        // two of the four Minors that produced MinorRevision (§23.4).
        //
        // ADDITIVE + SAFE. Pre-existing rows get the sentinel '' rather than a
        // plausible-looking default: an old row's claim is genuinely UNKNOWN, and
        // writing 'manuscript_defect' into it would assert something never
        // recorded — exactly the silent-default failure §26.4 rejects for the
        // cached-report path. `assemble_reviewer_input` maps '' to None (typed
        // absence), and no such row is ever read for a live verdict, because a
        // run only ever aggregates the rows it just persisted.
        up: "ALTER TABLE evidence ADD COLUMN claim TEXT NOT NULL DEFAULT '';",
        // SQLite cannot drop a column on older versions; the table is rebuilt.
        down: "ALTER TABLE evidence DROP COLUMN claim;",
    },
    Migration {
        version: 14,
        name: "ai_engine_phase1",
        // Citation Intelligence, Phase 1 (docs/AI_ENGINE_PLAN.md §4, §11).
        //
        // PURELY ADDITIVE. No existing table is altered. Every new table takes
        // the `ai_` prefix (§11 D1): `chunks` and `documents` already exist from
        // v6 (rag_documents), so an unprefixed `chunks` would collide outright,
        // and one prefix rule beats per-table exceptions.
        //
        // Chunks reference the EXISTING `documents` table rather than a parallel
        // `documents_ai`: rag.rs already writes provenance there (checksum,
        // status, quarantine reason) and a second registry would mean two
        // ingestion truths.
        //
        // PROVENANCE IS THE POINT. Every model-derived row carries model_id,
        // model_version, prompt_version and created_at, and every chunk it drew
        // on is a QUERYABLE FOREIGN KEY in ai_evidence_card_chunks —
        // provenance_json may duplicate that, but is never the only record.
        //
        // `page` is nullable EVERYWHERE and is a RECORDED fact, never inferred.
        // NULL means the source had no reliable page boundaries (DOCX/TXT, or a
        // PDF whose pages could not be read). A page is never estimated from
        // text position.
        up: "
            CREATE TABLE ai_chunks (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id    INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                page           INTEGER,
                section        TEXT,
                char_start     INTEGER NOT NULL,
                char_end       INTEGER NOT NULL,
                content        TEXT NOT NULL,
                token_estimate INTEGER NOT NULL,
                content_hash   TEXT NOT NULL,
                created_at     INTEGER NOT NULL,
                UNIQUE (document_id, content_hash)
            );
            CREATE INDEX idx_ai_chunks_document ON ai_chunks(document_id);
            CREATE INDEX idx_ai_chunks_page ON ai_chunks(document_id, page);

            -- model_id is MANDATORY: two embedding spaces must never be
            -- silently mixable, and a vector with no recorded model is a
            -- vector nobody can safely compare.
            CREATE TABLE ai_chunk_embeddings (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id   INTEGER NOT NULL REFERENCES ai_chunks(id) ON DELETE CASCADE,
                model_id   TEXT NOT NULL REFERENCES ai_model_registry(id),
                dim        INTEGER NOT NULL,
                vector     BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE (chunk_id, model_id)
            );
            CREATE INDEX idx_ai_chunk_embeddings_chunk ON ai_chunk_embeddings(chunk_id);

            CREATE TABLE ai_model_registry (
                id            TEXT PRIMARY KEY,
                kind          TEXT NOT NULL CHECK (kind IN ('embedding','generative')),
                display_name  TEXT NOT NULL DEFAULT '',
                file_path     TEXT NOT NULL DEFAULT '',
                sha256        TEXT,
                dim           INTEGER,
                quant         TEXT,
                registered_at INTEGER NOT NULL
            );

            CREATE TABLE ai_jobs (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                kind           TEXT NOT NULL,
                status         TEXT NOT NULL CHECK (status IN ('queued','running','done','failed','cancelled')),
                document_id    INTEGER REFERENCES documents(id) ON DELETE CASCADE,
                total_items    INTEGER NOT NULL DEFAULT 0,
                done_items     INTEGER NOT NULL DEFAULT 0,
                model_id       TEXT REFERENCES ai_model_registry(id),
                prompt_version TEXT NOT NULL DEFAULT '',
                error          TEXT,
                created_at     INTEGER NOT NULL,
                started_at     INTEGER,
                finished_at    INTEGER
            );
            CREATE INDEX idx_ai_jobs_status ON ai_jobs(status, created_at);

            CREATE TABLE ai_job_items (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id      INTEGER NOT NULL REFERENCES ai_jobs(id) ON DELETE CASCADE,
                chunk_id    INTEGER NOT NULL REFERENCES ai_chunks(id) ON DELETE CASCADE,
                status      TEXT NOT NULL CHECK (status IN ('queued','running','done','failed','skipped')),
                attempts    INTEGER NOT NULL DEFAULT 0,
                error       TEXT,
                created_at  INTEGER NOT NULL,
                finished_at INTEGER,
                UNIQUE (job_id, chunk_id)
            );
            CREATE INDEX idx_ai_job_items_job ON ai_job_items(job_id, status);

            -- chunk_id is the PRIMARY chunk and is nullable: a multi-source card
            -- has no single primary, and every chunk it used lives in the join
            -- table below regardless.
            CREATE TABLE ai_evidence_cards (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id     INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                chunk_id        INTEGER REFERENCES ai_chunks(id) ON DELETE SET NULL,
                page            INTEGER,
                claim           TEXT NOT NULL,
                evidence_text   TEXT NOT NULL,
                evidence_type   TEXT NOT NULL,
                verdict         TEXT,
                confidence      REAL,
                model_id        TEXT NOT NULL REFERENCES ai_model_registry(id),
                model_version   TEXT NOT NULL,
                prompt_version  TEXT NOT NULL,
                provenance_json TEXT NOT NULL DEFAULT '{}',
                created_at      INTEGER NOT NULL
            );
            CREATE INDEX idx_ai_evidence_cards_document ON ai_evidence_cards(document_id);
            CREATE INDEX idx_ai_evidence_cards_chunk ON ai_evidence_cards(chunk_id);

            CREATE TABLE ai_evidence_card_chunks (
                card_id  INTEGER NOT NULL REFERENCES ai_evidence_cards(id) ON DELETE CASCADE,
                chunk_id INTEGER NOT NULL REFERENCES ai_chunks(id) ON DELETE CASCADE,
                role     TEXT NOT NULL CHECK (role IN ('primary','supporting','contradicting')),
                PRIMARY KEY (card_id, chunk_id, role)
            );
            CREATE INDEX idx_ai_evidence_card_chunks_chunk ON ai_evidence_card_chunks(chunk_id);
        ",
        // Reverse dependency order so the FKs unwind cleanly.
        down: "
            DROP TABLE ai_evidence_card_chunks;
            DROP TABLE ai_evidence_cards;
            DROP TABLE ai_job_items;
            DROP TABLE ai_jobs;
            DROP TABLE ai_chunk_embeddings;
            DROP TABLE ai_model_registry;
            DROP TABLE ai_chunks;
        ",
    },
    Migration {
        version: 15,
        name: "ai_engine_phase2",
        // Citation Intelligence, Phase 2: real embeddings + lexical retrieval.
        //
        // (a) preprocessing_version on ai_chunk_embeddings. A vector is only
        //     comparable to another produced by the SAME model AND the same
        //     preprocessing (prefix, pooling, normalization). model_id alone is
        //     not enough: flipping pooling from mean to CLS changes the vector
        //     space without changing the model. Retrieval requires exactly one
        //     (model_id, preprocessing_version) pair and errors on a mix, so
        //     this column is what makes that rule enforceable rather than
        //     aspirational. NOT NULL with no default — a vector that cannot say
        //     how it was produced must not be storable.
        //
        //     ai_chunk_embeddings is EMPTY at this point (Phase 1 wrote no
        //     vectors), so the table is recreated rather than ALTERed: SQLite
        //     cannot add a NOT NULL column without a default, and inventing a
        //     default would be exactly the silent-provenance failure the column
        //     exists to prevent.
        //
        // (b) FTS5 over ai_chunks(content) as the lexical prefilter, kept in
        //     sync by triggers. `content=` makes it an EXTERNAL-CONTENT index:
        //     the text lives once, in ai_chunks, and fts holds only the index.
        up: "
            DROP TABLE ai_chunk_embeddings;
            CREATE TABLE ai_chunk_embeddings (
                id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id              INTEGER NOT NULL REFERENCES ai_chunks(id) ON DELETE CASCADE,
                model_id              TEXT NOT NULL REFERENCES ai_model_registry(id),
                preprocessing_version TEXT NOT NULL,
                dim                   INTEGER NOT NULL,
                vector                BLOB NOT NULL,
                created_at            INTEGER NOT NULL,
                UNIQUE (chunk_id, model_id)
            );
            CREATE INDEX idx_ai_chunk_embeddings_chunk ON ai_chunk_embeddings(chunk_id);
            CREATE INDEX idx_ai_chunk_embeddings_space
                ON ai_chunk_embeddings(model_id, preprocessing_version);

            CREATE VIRTUAL TABLE ai_chunks_fts USING fts5(
                content,
                content='ai_chunks',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 2'
            );
            -- Triggers keep the external-content index in step with the table.
            -- The delete/update forms use the 'delete' command with the OLD
            -- text, which is how fts5 external-content indexes are unwound;
            -- omitting it corrupts the index rather than merely staling it.
            CREATE TRIGGER ai_chunks_fts_ai AFTER INSERT ON ai_chunks BEGIN
                INSERT INTO ai_chunks_fts(rowid, content) VALUES (new.id, new.content);
            END;
            CREATE TRIGGER ai_chunks_fts_ad AFTER DELETE ON ai_chunks BEGIN
                INSERT INTO ai_chunks_fts(ai_chunks_fts, rowid, content)
                VALUES ('delete', old.id, old.content);
            END;
            CREATE TRIGGER ai_chunks_fts_au AFTER UPDATE ON ai_chunks BEGIN
                INSERT INTO ai_chunks_fts(ai_chunks_fts, rowid, content)
                VALUES ('delete', old.id, old.content);
                INSERT INTO ai_chunks_fts(rowid, content) VALUES (new.id, new.content);
            END;
            -- Adopt any rows Phase 1 already indexed.
            INSERT INTO ai_chunks_fts(rowid, content) SELECT id, content FROM ai_chunks;
        ",
        down: "
            DROP TRIGGER ai_chunks_fts_au;
            DROP TRIGGER ai_chunks_fts_ad;
            DROP TRIGGER ai_chunks_fts_ai;
            DROP TABLE ai_chunks_fts;
            DROP TABLE ai_chunk_embeddings;
            CREATE TABLE ai_chunk_embeddings (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id   INTEGER NOT NULL REFERENCES ai_chunks(id) ON DELETE CASCADE,
                model_id   TEXT NOT NULL REFERENCES ai_model_registry(id),
                dim        INTEGER NOT NULL,
                vector     BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE (chunk_id, model_id)
            );
            CREATE INDEX idx_ai_chunk_embeddings_chunk ON ai_chunk_embeddings(chunk_id);
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
    use rusqlite::params;

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
                "ai_engine_phase2",
                "ai_engine_phase1",
                "evidence_claim_kind",
                "evidence_store",
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
        assert_eq!(
            applied,
            vec![
                "plagiarism_library_citation_link",
                "citation_library_verification_persist",
                "evidence_store",
                "evidence_claim_kind",
                "ai_engine_phase1",
                "ai_engine_phase2"
            ]
        );
        assert_eq!(current_version(&conn).unwrap(), 15);

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

        // and it re-applies cleanly (idempotent up after a partial down): v10 + v11 + v12
        let reapplied = migrate_up(&mut conn).unwrap();
        assert_eq!(
            reapplied,
            vec![
                "plagiarism_library_citation_link",
                "citation_library_verification_persist",
                "evidence_store",
                "evidence_claim_kind",
                "ai_engine_phase1",
                "ai_engine_phase2"
            ]
        );
        assert!(column_names(&conn, "plagiarism_library").iter().any(|c| c == "citation_id"));
    }

    #[test]
    fn v11_adds_five_verification_columns_additively_no_data_loss() {
        // Simulate an EXISTING db that predates v11: stand at v10 with a citation
        // row, THEN apply v11 — the row must survive with retracted defaulting to
        // 0 and the verification columns NULL (additive, no rewrite).
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        migrate_down(&mut conn, 10).unwrap(); // roll back v12 + v11 → stand at v10
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

        // apply v11 (+ v12 rides along; it does not touch citation_library)
        let applied = migrate_up(&mut conn).unwrap();
        assert_eq!(applied, vec!["citation_library_verification_persist", "evidence_store", "evidence_claim_kind", "ai_engine_phase1", "ai_engine_phase2"]);
        assert_eq!(current_version(&conn).unwrap(), 15);
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

        // down to v10 peels v12 (evidence_store) then v11 (the subject here).
        let reverted = migrate_down(&mut conn, 10).unwrap();
        assert_eq!(reverted, vec!["ai_engine_phase2", "ai_engine_phase1", "evidence_claim_kind", "evidence_store", "citation_library_verification_persist"]);
        for c in ["retracted", "source", "verify_provenance", "verify_outcome", "verified_at"] {
            assert!(!column_names(&conn, "citation_library").iter().any(|n| n == c), "{c} should be dropped");
        }
        assert!(table_names(&conn).iter().any(|t| t == "citation_library"), "table itself remains");

        // re-applies cleanly (idempotent up after a partial down): v11 + v12
        let reapplied = migrate_up(&mut conn).unwrap();
        assert_eq!(reapplied, vec!["citation_library_verification_persist", "evidence_store", "evidence_claim_kind", "ai_engine_phase1", "ai_engine_phase2"]);
    }

    /* ------------------------- v14: AI engine, Phase 1 --------------------- */

    const AI_TABLES: &[&str] = &[
        "ai_chunk_embeddings",
        "ai_chunks",
        "ai_evidence_card_chunks",
        "ai_evidence_cards",
        "ai_job_items",
        "ai_jobs",
        "ai_model_registry",
    ];

    #[test]
    fn v14_round_trips_and_touches_no_existing_table() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 15);
        for t in AI_TABLES {
            assert!(table_names(&conn).iter().any(|n| n == t), "missing {t}");
        }
        // The pre-v14 tables the AI layer builds on are untouched and still
        // carry their own columns (purely additive).
        assert!(table_names(&conn).iter().any(|n| n == "documents"));
        assert!(table_names(&conn).iter().any(|n| n == "chunks"));
        let v6_chunks = column_names(&conn, "chunks");
        assert!(v6_chunks.iter().any(|c| c == "token_count"), "v6 chunks was altered");
        assert!(!v6_chunks.iter().any(|c| c == "page"), "v6 chunks gained an AI column");

        // down → every ai_ table is gone, everything else survives
        let reverted = migrate_down(&mut conn, 13).unwrap();
        assert_eq!(reverted, vec!["ai_engine_phase2", "ai_engine_phase1"]);
        assert_eq!(current_version(&conn).unwrap(), 13);
        for t in AI_TABLES {
            assert!(!table_names(&conn).iter().any(|n| n == t), "{t} survived the down migration");
        }
        assert!(table_names(&conn).iter().any(|n| n == "documents"), "down migration took documents with it");
        assert!(table_names(&conn).iter().any(|n| n == "chunks"), "down migration took v6 chunks with it");

        // and re-applies cleanly
        let reapplied = migrate_up(&mut conn).unwrap();
        assert_eq!(reapplied, vec!["ai_engine_phase1", "ai_engine_phase2"]);
        for t in AI_TABLES {
            assert!(table_names(&conn).iter().any(|n| n == t), "{t} missing after re-apply");
        }
    }

    /// Seed the minimum rows an evidence card needs: a document, a chunk, a
    /// model, and a card. Returns (document_id, chunk_id, card_id).
    fn seed_card(conn: &Connection) -> (i64, i64, i64) {
        conn.execute(
            "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
             VALUES ('test', 'Doc', '', 1, 'sum-1', 'ingested', 1)",
            [],
        )
        .unwrap();
        let doc = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO ai_chunks (document_id, page, section, char_start, char_end, content, token_estimate, content_hash, created_at)
             VALUES (?1, 3, 'Methods', 0, 10, 'some text', 2, 'hash-1', 1)",
            params![doc],
        )
        .unwrap();
        let chunk = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO ai_model_registry (id, kind, display_name, file_path, registered_at)
             VALUES ('m1', 'generative', 'M', '/tmp/m', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_evidence_cards
                (document_id, chunk_id, page, claim, evidence_text, evidence_type,
                 model_id, model_version, prompt_version, created_at)
             VALUES (?1, ?2, 3, 'c', 'e', 'support', 'm1', 'v1', 'p1', 1)",
            params![doc, chunk],
        )
        .unwrap();
        (doc, chunk, conn.last_insert_rowid())
    }

    #[test]
    fn ai_evidence_card_chunks_rejects_an_invalid_role() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        let (_doc, chunk, card) = seed_card(&conn);

        // the three legal roles are accepted
        for role in ["primary", "supporting", "contradicting"] {
            conn.execute(
                "INSERT INTO ai_evidence_card_chunks (card_id, chunk_id, role) VALUES (?1, ?2, ?3)",
                params![card, chunk, role],
            )
            .unwrap_or_else(|e| panic!("legal role {role} rejected: {e}"));
        }
        // anything else is refused by the CHECK constraint
        let err = conn.execute(
            "INSERT INTO ai_evidence_card_chunks (card_id, chunk_id, role) VALUES (?1, ?2, 'refuting')",
            params![card, chunk],
        );
        assert!(err.is_err(), "an invalid role was accepted");
    }

    #[test]
    fn ai_evidence_card_chunks_rejects_an_orphan_chunk_id() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        let (_doc, _chunk, card) = seed_card(&conn);

        // A chunk_id that does not exist must be refused — chunk references are
        // real foreign keys, not advisory ids duplicated out of provenance_json.
        let err = conn.execute(
            "INSERT INTO ai_evidence_card_chunks (card_id, chunk_id, role) VALUES (?1, 999999, 'primary')",
            params![card],
        );
        assert!(err.is_err(), "an orphan chunk_id was accepted — FK not enforced");

        // and an orphan card_id likewise
        let err = conn.execute(
            "INSERT INTO ai_evidence_card_chunks (card_id, chunk_id, role) VALUES (999999, 1, 'primary')",
            [],
        );
        assert!(err.is_err(), "an orphan card_id was accepted");
    }

    /* -------------------- v15: embeddings space + FTS5 --------------------- */

    #[test]
    fn v15_adds_fts_and_the_preprocessing_column_and_round_trips() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        assert!(column_names(&conn, "ai_chunk_embeddings").iter().any(|c| c == "preprocessing_version"));
        assert!(table_names(&conn).iter().any(|t| t == "ai_chunks_fts"));

        let reverted = migrate_down(&mut conn, 14).unwrap();
        assert_eq!(reverted, vec!["ai_engine_phase2"]);
        assert!(!table_names(&conn).iter().any(|t| t == "ai_chunks_fts"), "fts survived the down");
        assert!(!column_names(&conn, "ai_chunk_embeddings").iter().any(|c| c == "preprocessing_version"));
        // v14's tables are all still there — the down is scoped to v15.
        for t in AI_TABLES {
            assert!(table_names(&conn).iter().any(|n| n == t), "{t} lost by the v15 down");
        }
        assert_eq!(migrate_up(&mut conn).unwrap(), vec!["ai_engine_phase2"]);
    }

    #[test]
    fn a_vector_cannot_be_stored_without_saying_how_it_was_produced() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        let (_doc, chunk, _card) = seed_card(&conn);
        conn.execute(
            "INSERT INTO ai_model_registry (id, kind, display_name, file_path, dim, registered_at)
             VALUES ('emb', 'embedding', 'E', '/tmp/e', 384, 1)",
            [],
        )
        .unwrap();
        // NOT NULL, no default: preprocessing_version must be stated.
        assert!(
            conn.execute(
                "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, dim, vector, created_at)
                 VALUES (?1, 'emb', 384, X'00', 1)",
                params![chunk],
            )
            .is_err(),
            "a vector with no preprocessing_version was accepted"
        );
        conn.execute(
            "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
             VALUES (?1, 'emb', 'bge-v1.5-p1', 384, X'00', 1)",
            params![chunk],
        )
        .unwrap();
    }

    #[test]
    fn fts_index_follows_inserts_updates_and_deletes() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        let (doc, _chunk, _card) = seed_card(&conn);

        let hits = |c: &Connection, q: &str| -> i64 {
            c.query_row(
                "SELECT COUNT(*) FROM ai_chunks_fts WHERE ai_chunks_fts MATCH ?1",
                params![q],
                |r| r.get(0),
            )
            .unwrap()
        };

        // INSERT → indexed
        conn.execute(
            "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content, token_estimate, content_hash, created_at)
             VALUES (?1, 1, 0, 10, 'phytoplankton assemblages shifted', 3, 'h-fts', 1)",
            params![doc],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        assert_eq!(hits(&conn, "phytoplankton"), 1, "insert not indexed");

        // UPDATE → old term gone, new term present
        conn.execute("UPDATE ai_chunks SET content = 'zooplankton grazing' WHERE id = ?1", params![id]).unwrap();
        assert_eq!(hits(&conn, "phytoplankton"), 0, "stale term still indexed after update");
        assert_eq!(hits(&conn, "zooplankton"), 1, "update not indexed");

        // DELETE → gone
        conn.execute("DELETE FROM ai_chunks WHERE id = ?1", params![id]).unwrap();
        assert_eq!(hits(&conn, "zooplankton"), 0, "delete not removed from index");
    }

    #[test]
    fn v15_adopts_rows_that_phase_1_already_indexed() {
        // A database that stood at v14 with chunks already stored must come out
        // of v15 with those chunks searchable — otherwise the index silently
        // covers only what was written after the upgrade.
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        migrate_down(&mut conn, 14).unwrap();
        let (doc, _c, _card) = seed_card(&conn);
        conn.execute(
            "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content, token_estimate, content_hash, created_at)
             VALUES (?1, 2, 0, 10, 'preexisting eutrophication text', 3, 'h-pre', 1)",
            params![doc],
        )
        .unwrap();
        migrate_up(&mut conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ai_chunks_fts WHERE ai_chunks_fts MATCH 'eutrophication'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "v15 did not adopt pre-existing ai_chunks rows");
    }

    #[test]
    fn ai_chunk_embeddings_requires_a_model_id() {
        let mut conn = test_connection();
        migrate_up(&mut conn).unwrap();
        let (_doc, chunk, _card) = seed_card(&conn);
        // NOT NULL: a vector with no recorded model space is unusable, so the
        // schema refuses it rather than storing something incomparable.
        let err = conn.execute(
            "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, dim, vector, created_at)
             VALUES (?1, NULL, 384, X'00', 1)",
            params![chunk],
        );
        assert!(err.is_err(), "a NULL model_id was accepted");
    }
}
