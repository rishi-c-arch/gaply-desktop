//! **Read the duplication pairs. Do not score them.**
//!
//! The scoring is what is under suspicion, so candidates are chosen
//! STRUCTURALLY — contiguity of chunk runs — and then read. Prints BOTH sides
//! of a self-match from the pipeline's own `MatchSpan`, at full excerpt length,
//! so "these are the same passage" is a judgement a reader makes rather than a
//! number a reader trusts.
//!
//! Usage: dup_read_pairs <path> [seq_filter]
use app_lib::pipeline::{run_pipeline_measured, NetworkConsent};
use gaply_core::db::Database;
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::plagiarism::MatchSource;
use std::sync::Arc;

fn main() {
    std::env::set_var("GAPLY_DISABLE_DEEP", "1");
    let path = std::env::args().nth(1).unwrap();
    let only: Option<i64> = std::env::args().nth(2).and_then(|s| s.parse().ok());
    let limit: usize = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(4);

    let db = Arc::new(Database::in_memory().expect("db"));
    let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
    let out = run_pipeline_measured(
        db, embedder, path.clone(), None, None, None, None, NetworkConsent::Denied, &|_e| {},
    )
    .expect("pipeline");

    let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().to_string();
    println!("######## {name} — {} self-matches", out.plagiarism.self_matches.len());
    let mut shown = 0;
    for m in &out.plagiarism.self_matches {
        if let Some(s) = only {
            if m.manuscript_chunk_seq != s {
                continue;
            }
        }
        let MatchSource::SelfManuscript { other_chunk_seq, excerpt } = &m.source else { continue };
        shown += 1;
        println!("\n=== chunk {} vs chunk {} ===", m.manuscript_chunk_seq, other_chunk_seq);
        println!("A ({} chars): {}", m.manuscript_excerpt.len(), m.manuscript_excerpt);
        println!("B ({} chars): {}", excerpt.len(), excerpt);
        if shown >= limit {
            break;
        }
    }
    if shown == 0 {
        println!("(no match printed — filter matched nothing)");
    }
}
