//! THROWAWAY — diagnose the guideline pipeline end to end. Not a test.
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::{Database, GaplyError};

const GUIDELINE_QUERY: &str =
    "author guidelines submission requirements word limit structured abstract \
     conflict of interest declaration reference style";

fn main() -> Result<(), GaplyError> {
    let url = std::env::args().nth(1).expect("usage: guideline_probe <guidelines-url>");
    let db = Database::in_memory()?;
    let emb = HashEmbedder;

    println!("=== STEP 1: INGEST ===");
    let ing = app_lib::guidelines::GuidelinesIngestor::new()?;
    let report = ing.ingest(&db, &emb, None, Some(&url));
    println!("any_ingested: {}", report.any_ingested);
    println!("note: {}", report.note);
    for r in &report.results {
        println!("  result: {r:?}");
    }

    println!("\n=== STEP 2: PERSISTENCE ===");
    for t in ["documents", "chunks", "embeddings"] {
        println!("  {t}: {}", db.count_rows(t).unwrap_or(-1));
    }

    println!("\n=== STEP 3: RETRIEVAL (checklist path, filtered) ===");
    let hits = gaply_core::rag::search(&db, &emb, GUIDELINE_QUERY, 5, Some("journal_guideline"))?;
    println!("  hits: {}", hits.len());
    for h in &hits {
        println!("   - dist={:.3} type={} url={}", h.distance, h.source_type, h.source_url);
        println!("     text: {}", h.content.chars().take(200).collect::<String>());
    }

    println!("\n=== STEP 3b: RETRIEVAL (rag lane, unfiltered) ===");
    let hits2 = gaply_core::rag::search(&db, &emb, "reporting standards guidelines", 5, None)?;
    println!("  hits: {}", hits2.len());

    println!("\n=== STEP 4: WHAT WAS ACTUALLY STORED (first chunk of each doc) ===");
    let all = gaply_core::rag::search(&db, &emb, "the", 5, None)?;
    for h in &all {
        println!("  [{}] {} ({})", h.source_type, h.title, h.source_url);
        println!("  content_len={} head: {}", h.content.len(), h.content.chars().take(600).collect::<String>());
        println!();
    }
    Ok(())
}
