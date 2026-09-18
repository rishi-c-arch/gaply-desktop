//! Guideline-pipeline diagnostic — end to end, against a REAL URL.
//!
//! Not a test: it needs the network and a live page, so it cannot run in a
//! suite. Tracked anyway, for the same reason `read_report.rs` is — it performs
//! a check nothing else does, and a `git clean` should not be able to remove it.
//!
//! # STEP 5 is the point
//!
//! Steps 1-4 answer "was the page fetched, stored and retrievable". **Those all
//! passed for the BMJ homepage in run 20, which produced ZERO guideline-derived
//! checklist items** (ARCHITECTURE_TRACE §18.6.2). Ingestion reported success, a
//! plausible chunk count, and non-empty retrieval — and the content was useless.
//!
//! STEP 5 runs `build_checklist` and asks whether any item carries
//! `guideline_source: Some(_)`. **That is the only check that distinguishes
//! FETCHED from USABLE**, and it is the check both the `strip_block` boundary
//! defect and the homepage-prefill defect got wrong: each passed steps 1-4.
//!
//! Usage: `cargo run --example guideline_probe -- <guidelines-url>`
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
    println!("\n=== STEP 5: CHECKLIST — the check that actually matters ===");
    println!("  (fetched != usable; only a guideline_source: Some(_) proves the content was used)");
    let manuscript = "Title: A Study\n\nAbstract\nWe did things.\n\nMethods\nWe measured with a \
        conflict of interest declaration.\n\nResults\np = 0.02.\n\nReferences\n1. A. Author (2020).";
    let extraction = gaply_core::extract::extract_from_text(manuscript);
    let checklist = gaply_core::report::build_checklist(&db, &extraction, manuscript, Some(&url), None)?;
    let derived: Vec<_> = checklist.iter().filter(|c| c.guideline_source.is_some()).collect();
    println!("  checklist items: {} ({} guideline-derived)", checklist.len(), derived.len());
    for c in &checklist {
        let src = if c.guideline_source.is_some() { "GUIDELINE" } else { "structural" };
        println!("   [{src:10}] {} — passed={}", c.requirement, c.passed);
    }
    println!("\n  VERDICT: {}", if derived.is_empty() {
        "NO guideline-derived items — this URL ingests but is NOT USABLE"
    } else {
        "usable — guideline-derived items present"
    });
    Ok(())
}
