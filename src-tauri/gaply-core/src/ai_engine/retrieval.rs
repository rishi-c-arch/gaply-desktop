//! Retrieval over `ai_chunks`: FTS5 lexical prefilter, then cosine rerank.
//!
//! Pure and local. The caller supplies the query VECTOR — producing it needs a
//! model, and models live in the app crate.
//!
//! # Shape, and its known ceiling
//!
//! ```text
//! query ──► FTS5 MATCH ──► top CANDIDATE_LIMIT ids ──► load vectors (one space)
//!                                                            │
//!                                        cosine in Rust ─────┴──► top k
//! ```
//!
//! This is a STRICT PREFILTER, so a chunk the lexical index misses cannot be
//! recovered by vector similarity. That ceiling is deliberate for this phase and
//! recorded in the plan (§9.6, §11 D7); the union shape removes it later. The
//! fallback below softens the worst case: when FTS returns fewer than `k` hits —
//! a short, rare, or heavily paraphrased query, which is exactly when lexical
//! matching is weakest — the search falls back to cosine over every vector in
//! scope. Brute force is acceptable at current scale.

use rusqlite::params;
use serde::Serialize;

use crate::ai_engine::embeddings::{self, EmbeddingSpace};
use crate::db::Database;
use crate::error::GaplyError;

/// How many lexical candidates to rerank. Wide enough that the vector stage
/// does real work, narrow enough that loading the vectors stays cheap.
pub const CANDIDATE_LIMIT: usize = 200;

/// How much of a chunk to return as a preview.
const SNIPPET_CHARS: usize = 320;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchHit {
    pub chunk_id: i64,
    pub document_id: i64,
    /// Recorded page, or `None` when the source had none. Never inferred.
    pub page: Option<u32>,
    pub section: Option<String>,
    pub snippet: String,
    pub score: f32,
}

/// Which path produced the hits — reported, not inferred by the caller.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum SearchPath {
    /// FTS5 supplied the candidates and cosine reranked them.
    LexicalPrefilter,
    /// FTS5 returned fewer than `k`; cosine ran over every vector in scope.
    FullScan,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub path: SearchPath,
    pub space: EmbeddingSpace,
    /// How many candidates the vector stage actually scored.
    pub scored: usize,
}

fn snippet_of(content: &str) -> String {
    if content.chars().count() <= SNIPPET_CHARS {
        return content.to_string();
    }
    let cut: String = content.chars().take(SNIPPET_CHARS).collect();
    format!("{}…", cut.trim_end())
}

/// Escape a user query for FTS5 MATCH.
///
/// FTS5 has its own query syntax, so raw user text can be a syntax error (an
/// unbalanced quote) or, worse, a silently different query (`NOT`, `*`, `:`).
/// Every token is quoted and the tokens are OR-ed, which makes the prefilter a
/// recall net rather than a conjunction — the cosine stage does the ranking, so
/// the prefilter's job is only to not miss things.
fn to_fts_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" OR "))
    }
}

/// Lexical candidate ids, best-matching first.
fn lexical_candidates(
    db: &Database,
    query: &str,
    document_id: Option<i64>,
    limit: usize,
) -> Result<Vec<i64>, GaplyError> {
    let Some(match_expr) = to_fts_query(query) else {
        return Ok(Vec::new());
    };
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT c.id
         FROM ai_chunks_fts f
         JOIN ai_chunks c ON c.id = f.rowid
         WHERE ai_chunks_fts MATCH ?1
           AND (?2 IS NULL OR c.document_id = ?2)
         ORDER BY bm25(ai_chunks_fts)
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![match_expr, document_id, limit as i64], |r| r.get::<_, i64>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Hydrate scored ids into hits, preserving the score order.
fn hydrate(
    db: &Database,
    scored: &[(i64, f32)],
) -> Result<Vec<SearchHit>, GaplyError> {
    if scored.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = scored.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let conn = db.conn()?;
    let sql = format!(
        "SELECT id, document_id, page, section, content FROM ai_chunks WHERE id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids: Vec<i64> = scored.iter().map(|(id, _)| *id).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut by_id = std::collections::HashMap::new();
    for row in rows {
        let (id, doc, page, section, content) = row?;
        by_id.insert(id, (doc, page, section, content));
    }
    Ok(scored
        .iter()
        .filter_map(|(id, score)| {
            by_id.get(id).map(|(doc, page, section, content)| SearchHit {
                chunk_id: *id,
                document_id: *doc,
                page: page.map(|p| p as u32),
                section: section.clone(),
                snippet: snippet_of(content),
                score: *score,
            })
        })
        .collect())
}

/// Rank `candidates` against `query_vector` and keep the best `k`.
fn rank(candidates: Vec<(i64, Vec<f32>)>, query_vector: &[f32], k: usize) -> Vec<(i64, f32)> {
    let mut scored: Vec<(i64, f32)> = candidates
        .into_iter()
        .map(|(id, v)| (id, embeddings::cosine(query_vector, &v)))
        .collect();
    // Deterministic: ties break on chunk_id so the same corpus and query always
    // produce the same ordering.
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    scored.truncate(k);
    scored
}

/// Semantic search: FTS prefilter, cosine rerank, top `k`.
///
/// `query_vector` must come from the SAME space as the stored vectors; the
/// space is resolved here and returned in the result so a caller cannot report
/// scores without saying which representation produced them. A library holding
/// more than one space is a typed [`GaplyError::Conflict`], never a silent
/// comparison across incompatible vectors.
pub fn semantic_search(
    db: &Database,
    query_text: &str,
    query_vector: &[f32],
    document_id: Option<i64>,
    k: usize,
) -> Result<SearchResult, GaplyError> {
    let space = embeddings::resolve_single_space(db)?;
    let candidates = lexical_candidates(db, query_text, document_id, CANDIDATE_LIMIT)?;

    // Fall back when the lexical net is thinner than what we owe the caller —
    // precisely the paraphrased-query case the vector stage exists to serve.
    let (vectors, path) = if candidates.len() < k {
        (embeddings::load_all_vectors(db, document_id, &space)?, SearchPath::FullScan)
    } else {
        (embeddings::load_vectors(db, &candidates, &space)?, SearchPath::LexicalPrefilter)
    };

    let scored_count = vectors.len();
    let ranked = rank(vectors, query_vector, k);
    Ok(SearchResult { hits: hydrate(db, &ranked)?, path, space, scored: scored_count })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_engine::embeddings::EmbeddingSpace;

    fn space() -> EmbeddingSpace {
        EmbeddingSpace::new("test-model", "p1")
    }

    fn setup(n_chunks: usize) -> (Database, i64) {
        let db = Database::in_memory().unwrap();
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
             VALUES ('paper', 'Doc', '', 1, 'sum-r', 'ingested', 1)",
            [],
        )
        .unwrap();
        let doc = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO ai_model_registry (id, kind, display_name, file_path, dim, registered_at)
             VALUES ('test-model', 'embedding', 'T', '/tmp/t', 3, 1)",
            [],
        )
        .unwrap();
        for i in 0..n_chunks {
            conn.execute(
                "INSERT INTO ai_chunks (document_id, page, section, char_start, char_end, content, token_estimate, content_hash, created_at)
                 VALUES (?1, ?2, 'Results', 0, 10, ?3, 5, ?4, 1)",
                params![doc, (i as i64 % 7) + 1, format!("filler chunk number {i} about unrelated matters"), format!("h{i}")],
            )
            .unwrap();
        }
        drop(conn);
        (db, doc)
    }

    fn embed_all(db: &Database, vec_for: impl Fn(&str) -> Vec<f32>) {
        let pending = embeddings::chunks_missing_embeddings(db, None, "test-model").unwrap();
        let rows: Vec<(i64, Vec<f32>)> =
            pending.iter().map(|p| (p.chunk_id, vec_for(&p.content))).collect();
        embeddings::put_embeddings(db, &space(), &rows).unwrap();
    }

    #[test]
    fn mixing_spaces_is_a_typed_error_not_silent_garbage() {
        let (db, _doc) = setup(3);
        embed_all(&db, |_| vec![1.0, 0.0, 0.0]);
        // a second space appears (a re-embed under new preprocessing)
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO ai_model_registry (id, kind, display_name, file_path, dim, registered_at)
             VALUES ('other-model', 'embedding', 'O', '/tmp/o', 3, 1)",
            [],
        )
        .unwrap();
        drop(conn);
        let pending: Vec<i64> = embeddings::chunks_missing_embeddings(&db, None, "other-model")
            .unwrap()
            .iter()
            .map(|p| p.chunk_id)
            .collect();
        embeddings::put_embeddings(
            &db,
            &EmbeddingSpace::new("other-model", "p1"),
            &[(pending[0], vec![0.0, 1.0, 0.0])],
        )
        .unwrap();

        let err = semantic_search(&db, "filler", &[1.0, 0.0, 0.0], None, 3).unwrap_err();
        assert_eq!(err.code(), "conflict", "expected a typed conflict, got {err}");
        assert!(err.to_string().contains("not comparable"), "{err}");

        // The SAME model with a different preprocessing_version is also a mix.
        let db2 = setup(2).0;
        embed_all(&db2, |_| vec![1.0, 0.0, 0.0]);
        let ids: Vec<i64> = embeddings::chunks_missing_embeddings(&db2, None, "nope")
            .unwrap()
            .iter()
            .map(|p| p.chunk_id)
            .collect();
        embeddings::put_embeddings(&db2, &EmbeddingSpace::new("test-model", "p2"), &[(ids[0], vec![0.0, 0.0, 1.0])])
            .unwrap();
        let err2 = semantic_search(&db2, "filler", &[1.0, 0.0, 0.0], None, 2).unwrap_err();
        assert_eq!(err2.code(), "conflict", "same model, different preprocessing must also refuse");
    }

    #[test]
    fn no_embeddings_at_all_is_a_typed_not_found() {
        let (db, _doc) = setup(2);
        let err = semantic_search(&db, "filler", &[1.0, 0.0, 0.0], None, 2).unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn needle_test_a_exact_match_ranks_first_in_a_500_chunk_corpus() {
        // Pipeline shape with a MOCKED embedder: the planted chunk gets a
        // distinctive vector, filler gets a different one. This is the CI-safe
        // half; the real-model paraphrase test is env-gated in the app crate.
        let (db, doc) = setup(499);
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO ai_chunks (document_id, page, section, char_start, char_end, content, token_estimate, content_hash, created_at)
             VALUES (?1, 42, 'Discussion', 0, 90, ?2, 12, 'needle', 1)",
            params![doc, "Anthropogenic eutrophication altered the composition of phytoplankton assemblages in the reservoir."],
        )
        .unwrap();
        let needle_id = conn.last_insert_rowid();
        drop(conn);

        embed_all(&db, |content| {
            if content.contains("eutrophication") { vec![1.0, 0.0, 0.0] } else { vec![0.0, 1.0, 0.0] }
        });

        let res = semantic_search(&db, "eutrophication phytoplankton", &[1.0, 0.0, 0.0], None, 5).unwrap();
        assert_eq!(res.hits[0].chunk_id, needle_id, "the needle did not rank first");
        assert!((res.hits[0].score - 1.0).abs() < 1e-5);
        assert_eq!(res.hits[0].page, Some(42), "page provenance lost through retrieval");
        assert_eq!(res.hits[0].section.as_deref(), Some("Discussion"));
        assert_eq!(res.space, space());
    }

    #[test]
    fn a_query_no_lexical_index_matches_falls_back_to_a_full_scan() {
        let (db, _doc) = setup(20);
        embed_all(&db, |content| {
            if content.contains("number 7 ") { vec![1.0, 0.0, 0.0] } else { vec![0.0, 1.0, 0.0] }
        });
        // "zzzz" appears in no chunk, so FTS yields nothing and the vector stage
        // must still answer — this is the paraphrase case in miniature.
        let res = semantic_search(&db, "zzzz", &[1.0, 0.0, 0.0], None, 3).unwrap();
        assert_eq!(res.path, SearchPath::FullScan);
        assert_eq!(res.hits.len(), 3);
        assert_eq!(res.scored, 20, "full scan should have scored every vector in scope");
    }

    #[test]
    fn document_scope_excludes_other_documents() {
        let (db, doc) = setup(5);
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
             VALUES ('paper', 'Other', '', 1, 'sum-o', 'ingested', 1)",
            [],
        )
        .unwrap();
        let other = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content, token_estimate, content_hash, created_at)
             VALUES (?1, 1, 0, 10, 'filler chunk number 999 about unrelated matters', 5, 'h999', 1)",
            params![other],
        )
        .unwrap();
        drop(conn);
        embed_all(&db, |_| vec![1.0, 0.0, 0.0]);

        let scoped = semantic_search(&db, "filler", &[1.0, 0.0, 0.0], Some(doc), 50).unwrap();
        assert!(scoped.hits.iter().all(|h| h.document_id == doc), "scope leaked another document");
        let unscoped = semantic_search(&db, "filler", &[1.0, 0.0, 0.0], None, 50).unwrap();
        assert!(unscoped.hits.len() > scoped.hits.len());
    }

    #[test]
    fn an_fts_syntax_hostile_query_does_not_error() {
        // Raw FTS5 syntax in user text must not become a query error or, worse,
        // a silently different query.
        let (db, _doc) = setup(5);
        embed_all(&db, |_| vec![1.0, 0.0, 0.0]);
        for q in ["\"unbalanced", "NOT filler", "chunk*", "a:b", "()", "  "] {
            let r = semantic_search(&db, q, &[1.0, 0.0, 0.0], None, 3);
            assert!(r.is_ok(), "query {q:?} errored: {:?}", r.err());
        }
    }

    #[test]
    fn resumability_only_pending_chunks_are_returned() {
        let (db, _doc) = setup(10);
        assert_eq!(embeddings::chunks_missing_embeddings(&db, None, "test-model").unwrap().len(), 10);
        // embed half
        let pending = embeddings::chunks_missing_embeddings(&db, None, "test-model").unwrap();
        let half: Vec<(i64, Vec<f32>)> =
            pending.iter().take(5).map(|p| (p.chunk_id, vec![1.0, 0.0, 0.0])).collect();
        embeddings::put_embeddings(&db, &space(), &half).unwrap();
        // a cancelled run leaves exactly the rest pending — no cursor to desync
        assert_eq!(embeddings::chunks_missing_embeddings(&db, None, "test-model").unwrap().len(), 5);
    }

    #[test]
    #[ignore = "benchmark; run with --ignored"]
    fn bench_search_over_10k_chunks() {
        let (db, _doc) = setup(10_000);
        embed_all(&db, |c| {
            let n = c.len() as f32;
            vec![(n % 7.0) / 7.0, (n % 11.0) / 11.0, (n % 13.0) / 13.0]
        });
        let t0 = std::time::Instant::now();
        let res = semantic_search(&db, "filler chunk unrelated", &[0.3, 0.5, 0.2], None, 10).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        println!("search over 10k chunks: {ms:.1} ms  (path {:?}, scored {})", res.path, res.scored);

        let t1 = std::time::Instant::now();
        let res2 = semantic_search(&db, "zzzznomatch", &[0.3, 0.5, 0.2], None, 10).unwrap();
        let ms2 = t1.elapsed().as_secs_f64() * 1000.0;
        println!("full-scan fallback over 10k chunks: {ms2:.1} ms  (scored {})", res2.scored);
    }
}
