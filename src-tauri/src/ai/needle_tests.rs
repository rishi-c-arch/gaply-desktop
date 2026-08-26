//! Real-model retrieval tests. Env-gated and `#[ignore]`d.
//!
//! These are the tests that prove semantic retrieval EXISTS rather than that
//! the pipeline is wired. They need the real ~130 MB model, so they never run
//! in CI; the mocked-embedder equivalents in
//! `gaply_core::ai_engine::retrieval` cover the pipeline shape there.
//!
//! Run with:
//! ```text
//! GAPLY_TEST_EMBED_MODEL=1 cargo test -p app needle -- --ignored --nocapture
//! ```
//! The model must already be installed under `$GAPLY_TEST_MODEL_DIR` (or the
//! default app data dir). These tests never download.

#![cfg(test)]

use std::path::PathBuf;

use gaply_core::ai_engine::{embeddings as core_emb, retrieval};
use gaply_core::Database;

use crate::ai::{embedding_space, embeddings::EmbeddingEngine, model_install};

/// The planted sentence. Deliberately technical and self-contained.
const NEEDLE: &str =
    "Anthropogenic eutrophication altered the composition of phytoplankton assemblages in the reservoir.";

/// Shares NO content word with the needle: nutrient≠eutrophication,
/// enrichment≠anthropogenic, algal≠phytoplankton, communities≠assemblages.
/// Matching it lexically is impossible, so a hit can only come from the vectors.
const PARAPHRASE_QUERY: &str = "effects of nutrient enrichment on algal communities";

const EXACT_QUERY: &str = "eutrophication phytoplankton assemblages reservoir";

fn enabled() -> bool {
    std::env::var("GAPLY_TEST_EMBED_MODEL").is_ok()
}

fn model_root() -> PathBuf {
    std::env::var("GAPLY_TEST_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("gaply-test-models"))
}

/// 499 filler chunks on assorted scientific topics + the needle at a known page.
fn corpus() -> Vec<String> {
    let topics = [
        "Soil respiration rates were measured monthly across three replicate plots.",
        "The catalyst showed selectivity above ninety percent under mild conditions.",
        "Participants completed a standardised questionnaire at baseline and follow-up.",
        "Compressive strength increased with curing time up to twenty-eight days.",
        "Gene expression was quantified by reverse transcription polymerase chain reaction.",
        "The finite element mesh was refined until displacement converged.",
        "Sea surface temperature anomalies correlated with the oscillation index.",
        "Survey responses were coded independently by two researchers.",
        "The alloy microstructure was examined by scanning electron microscopy.",
        "Market volatility was modelled using a generalised autoregressive framework.",
    ];
    let mut out: Vec<String> = (0..499)
        .map(|i| format!("Section {i}. {} Observation {i} followed the stated protocol.", topics[i % topics.len()]))
        .collect();
    out.push(NEEDLE.to_string());
    out
}

fn build_corpus_db(engine: &EmbeddingEngine) -> (Database, i64) {
    use gaply_core::ai_engine::{registry, store};
    use gaply_core::chunk::PagedChunk;

    let db = Database::in_memory().unwrap();
    // Everything below goes through gaply-core's PUBLIC DAO — the app crate
    // cannot execute SQL (Database::conn is pub(crate)), which is exactly the
    // boundary this test should be exercising rather than bypassing.
    let doc = store::create_document(&db, "Needle Corpus", "/tmp/needle.pdf", "needle-sum").unwrap();
    registry::register_model(
        &db,
        registry::ModelRow {
            id: crate::ai::EMBED_MODEL_ID.to_string(),
            kind: "embedding".into(),
            display_name: "BGE Small EN v1.5".into(),
            file_path: model_install::model_dir(&model_root()).display().to_string(),
            sha256: None,
            dim: Some(crate::ai::EMBED_DIM as i64),
            quant: None,
        },
    )
    .unwrap();

    let texts = corpus();
    let mut offset = 0usize;
    let chunks: Vec<PagedChunk> = texts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let c = PagedChunk {
                seq: i as i64,
                content: t.clone(),
                token_estimate: t.split_whitespace().count(),
                page: Some((i as u32 / 3) + 1),
                section: Some("Results".to_string()),
                char_start: offset,
                char_end: offset + t.len(),
            };
            offset += t.len() + 2;
            c
        })
        .collect();
    let outcome = store::index_chunks(&db, doc, &chunks).unwrap();
    assert_eq!(outcome.inserted, texts.len(), "corpus did not index cleanly");

    let pending = core_emb::chunks_missing_embeddings(&db, None, crate::ai::EMBED_MODEL_ID).unwrap();
    let t0 = std::time::Instant::now();
    let vectors = engine
        .embed_documents(&pending.iter().map(|p| p.content.clone()).collect::<Vec<_>>())
        .unwrap();
    let embed_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!(
        "embedded {} chunks in {:.0} ms  ({:.1} ms/chunk, pooling {:?})",
        pending.len(),
        embed_ms,
        embed_ms / pending.len() as f64,
        crate::ai::EMBED_POOLING
    );
    let rows: Vec<(i64, Vec<f32>)> = pending.iter().map(|p| p.chunk_id).zip(vectors).collect();
    core_emb::put_embeddings(&db, &embedding_space(), &rows).unwrap();
    (db, doc)
}

fn load_engine() -> Option<EmbeddingEngine> {
    let root = model_root();
    let dir = model_install::model_dir(&root);
    if !model_install::all_files_verified(&dir) {
        eprintln!(
            "SKIP: no verified model at {} — install it first (these tests never download)",
            dir.display()
        );
        return None;
    }
    Some(EmbeddingEngine::load_verified(&dir).expect("verified model must load"))
}

fn needle_rank(res: &retrieval::SearchResult, needle_id: i64) -> Option<usize> {
    res.hits.iter().position(|h| h.chunk_id == needle_id).map(|p| p + 1)
}

fn needle_chunk_id(db: &Database, doc: i64) -> i64 {
    gaply_core::ai_engine::store::list_chunks(db, doc)
        .unwrap()
        .into_iter()
        .find(|c| c.content == NEEDLE)
        .expect("needle must be in the corpus")
        .id
}

#[test]
#[ignore = "needs the real embedding model; set GAPLY_TEST_EMBED_MODEL"]
fn needle_a_exact_terms_rank_first() {
    if !enabled() {
        eprintln!("SKIP: set GAPLY_TEST_EMBED_MODEL=1");
        return;
    }
    let Some(engine) = load_engine() else { return };
    let (db, doc) = build_corpus_db(&engine);
    let needle = needle_chunk_id(&db, doc);

    let qv = engine.embed_query(EXACT_QUERY).unwrap();
    let t0 = std::time::Instant::now();
    let res = retrieval::semantic_search(&db, EXACT_QUERY, &qv, None, 5).unwrap();
    let ms = t0.elapsed().as_secs_f64() * 1000.0;

    println!("\n=== NEEDLE A (exact) ===");
    println!("query: {EXACT_QUERY:?}");
    println!("search {ms:.1} ms  path {:?}  scored {}", res.path, res.scored);
    for (i, h) in res.hits.iter().enumerate() {
        println!("  {}. score {:.4} p{:?} {:?}", i + 1, h.score, h.page, &h.snippet[..70.min(h.snippet.len())]);
    }
    assert_eq!(needle_rank(&res, needle), Some(1), "the needle did not rank first");
}

#[test]
#[ignore = "needs the real embedding model; set GAPLY_TEST_EMBED_MODEL"]
fn needle_b_semantic_paraphrase_ranks_top_3() {
    if !enabled() {
        eprintln!("SKIP: set GAPLY_TEST_EMBED_MODEL=1");
        return;
    }
    let Some(engine) = load_engine() else { return };
    let (db, doc) = build_corpus_db(&engine);
    let needle = needle_chunk_id(&db, doc);

    // Prove the premise: the query and the needle share no content word, so a
    // hit cannot be lexical. If this ever fails, the test has stopped testing
    // semantics and needs a new query, not a looser threshold.
    let stop = ["of", "on", "the", "in", "effects"];
    let q_words: std::collections::HashSet<String> = PARAPHRASE_QUERY
        .split_whitespace()
        .map(|w| w.trim_matches('.').to_lowercase())
        .filter(|w| !stop.contains(&w.as_str()))
        .collect();
    let n_words: std::collections::HashSet<String> = NEEDLE
        .split_whitespace()
        .map(|w| w.trim_matches('.').to_lowercase())
        .collect();
    let shared: Vec<_> = q_words.intersection(&n_words).collect();
    assert!(shared.is_empty(), "query and needle share content words {shared:?} — not a semantic test");

    let qv = engine.embed_query(PARAPHRASE_QUERY).unwrap();
    let t0 = std::time::Instant::now();
    let res = retrieval::semantic_search(&db, PARAPHRASE_QUERY, &qv, None, 10).unwrap();
    let ms = t0.elapsed().as_secs_f64() * 1000.0;

    println!("\n=== NEEDLE B (semantic paraphrase) ===");
    println!("query:  {PARAPHRASE_QUERY:?}");
    println!("needle: {NEEDLE:?}");
    println!("shared content words: NONE — a lexical match is impossible");
    println!("search {ms:.1} ms  path {:?}  scored {}", res.path, res.scored);
    for (i, h) in res.hits.iter().take(5).enumerate() {
        println!("  {}. score {:.4} p{:?} {:?}", i + 1, h.score, h.page, &h.snippet[..70.min(h.snippet.len())]);
    }
    let rank = needle_rank(&res, needle);
    println!("needle rank: {rank:?}");
    assert!(
        matches!(rank, Some(r) if r <= 3),
        "paraphrase needle ranked {rank:?}, expected top 3 — semantic retrieval is not working"
    );
}
