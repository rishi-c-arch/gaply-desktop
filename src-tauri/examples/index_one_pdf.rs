//! One-shot: index + embed + link ONE local PDF into the app's database, so the
//! Phase 9 manual verification has a real document to open.
//!
//! Deliberately a separate example rather than a hidden command: it mutates the
//! user's live library, so it is explicit, run by hand, and prints exactly what
//! it changed.
//!
//!   cargo run --release --example index_one_pdf -- "<pdf path>" "<citation title>"
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let pdf = PathBuf::from(args.next().ok_or("usage: index_one_pdf <pdf> <title>")?);
    let title = args.next().unwrap_or_else(|| {
        pdf.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string()
    });

    let db_path = dirs_next_app_data().join("gaply.db");
    println!("database : {}", db_path.display());
    println!("pdf      : {}", pdf.display());

    let db = gaply_core::Database::open(&db_path)?;

    // 1. parse + index
    let blocks = gaply_core::extract::docparse::parse_path_paged(&pdf)?;
    println!("parsed   : {} blocks", blocks.len());

    let checksum = format!("manual-{}", gaply_core::ai_engine::store::content_hash(&pdf.display().to_string()));
    let doc = gaply_core::ai_engine::store::create_document(
        &db,
        &title,
        &pdf.display().to_string(),
        &checksum,
    )?;
    println!("document : id={doc}");

    let chunks: Vec<gaply_core::chunk::PagedChunk> = blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.text.split_whitespace().count() >= 20)
        .map(|(i, b)| gaply_core::chunk::PagedChunk {
            seq: i as i64,
            content: b.text.replace('\n', " "),
            token_estimate: b.text.split_whitespace().count(),
            page: b.page,
            section: None,
            char_start: 0,
            char_end: b.text.len(),
        })
        .collect();
    let outcome = gaply_core::ai_engine::store::index_chunks(&db, doc, &chunks)?;
    println!("chunks   : {} inserted, {} duplicates", outcome.inserted, outcome.duplicates);

    // 2. embed with the REAL engine
    let emb_dir = std::env::var("GAPLY_EMBED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_home().join("gaply-models/bge-small-en-v1.5"));
    println!("embedder : {}", emb_dir.display());
    let engine = app_lib::ai::embeddings::EmbeddingEngine::load_verified(&emb_dir)?;

    gaply_core::ai_engine::registry::register_model(
        &db,
        gaply_core::ai_engine::registry::ModelRow {
            id: app_lib::ai::EMBED_MODEL_ID.into(),
            kind: "embedding".into(),
            display_name: "bge-small-en-v1.5".into(),
            file_path: emb_dir.display().to_string(),
            sha256: None,
            dim: Some(app_lib::ai::EMBED_DIM as i64),
            quant: None,
        },
    )?;
    let space = app_lib::ai::embedding_space();
    let pending = gaply_core::ai_engine::embeddings::chunks_missing_embeddings(
        &db,
        Some(doc),
        app_lib::ai::EMBED_MODEL_ID,
    )?;
    println!("embedding: {} chunks…", pending.len());
    let texts: Vec<String> = pending.iter().map(|p| p.content.clone()).collect();
    let vectors = engine.embed_documents(&texts)?;
    let rows: Vec<(i64, Vec<f32>)> =
        pending.iter().map(|p| p.chunk_id).zip(vectors).collect();
    gaply_core::ai_engine::embeddings::put_embeddings(&db, &space, &rows)?;
    println!("embedded : {} vectors", rows.len());

    // 3. a citation_library entry for THIS document, so the link that follows
    //    is truthful by construction rather than a forced title collision.
    let cite_id = format!("cite-manualindex-{}", checksum.chars().take(12).collect::<String>());
    let csl = serde_json::json!({
        "id": cite_id,
        "type": "manuscript",
        "title": title,
    })
    .to_string();
    gaply_core::citation_library::upsert(
        &db,
        &cite_id,
        &csl,
        None,
        &["phase9-manual-check".to_string()],
        &gaply_core::citation_library::VerificationWrite {
            retracted: false,
            source: Some("local-file".into()),
            verify_provenance: vec![],
            verify_outcome: None,
            verified_at: None,
        },
    )?;
    println!("citation : id={cite_id} title={title:?}");

    // 4. link
    let report = gaply_core::citation_links::link_citations(&db)?;
    println!("links    : {}", serde_json::to_string_pretty(&report)?);

    Ok(())
}

fn dirs_home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME"))
}
fn dirs_next_app_data() -> PathBuf {
    dirs_home().join("Library/Application Support/ai.gaply.app")
}
