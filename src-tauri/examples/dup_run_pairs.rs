//! **Can a grouped duplication row say "A repeats B", or only "A is repeated"?**
//!
//! 1879 self-match rows across the corpus collapse to 126 contiguous SOURCE
//! runs. That alone supports *"this passage appears more than once"*. The
//! stronger claim — *"§3.4 and §5.1 are the same text"* — needs the TARGETS to
//! cluster too, and `MatchSource::SelfManuscript` carries `other_chunk_seq`, so
//! it is measurable rather than assumable.
//!
//! For each source run, prints how many contiguous target runs its partners
//! form. 1 means a clean pair; many means the passage is scattered and the
//! stronger sentence would be false.
use app_lib::pipeline::{run_pipeline_measured, NetworkConsent};
use gaply_core::db::Database;
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::plagiarism::MatchSource;
use std::sync::Arc;

fn runs_of(mut v: Vec<i64>) -> Vec<(i64, i64)> {
    v.sort_unstable();
    v.dedup();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for s in v {
        match out.last_mut() {
            Some(last) if s == last.1 + 1 => last.1 = s,
            _ => out.push((s, s)),
        }
    }
    out
}

fn main() {
    std::env::set_var("GAPLY_DISABLE_DEEP", "1");
    let (mut clean, mut total) = (0usize, 0usize);
    for path in std::env::args().skip(1) {
        let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().to_string();
        let db = Arc::new(Database::in_memory().expect("db"));
        let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
        let Ok(out) = run_pipeline_measured(
            db, embedder, path.clone(), None, None, None, NetworkConsent::Denied, &|_e| {},
        ) else { continue };
        let sm = &out.plagiarism.self_matches;
        if sm.is_empty() {
            continue;
        }
        let src_runs = runs_of(sm.iter().map(|m| m.manuscript_chunk_seq).collect());
        println!("\n######## {name}  ({} rows, {} source runs)", sm.len(), src_runs.len());
        for (a, b) in &src_runs {
            let partners: Vec<i64> = sm
                .iter()
                .filter(|m| m.manuscript_chunk_seq >= *a && m.manuscript_chunk_seq <= *b)
                .filter_map(|m| match &m.source {
                    MatchSource::SelfManuscript { other_chunk_seq, .. } => Some(*other_chunk_seq),
                    _ => None,
                })
                .collect();
            let tgt = runs_of(partners);
            total += 1;
            if tgt.len() == 1 {
                clean += 1;
            }
            let shown: Vec<String> =
                tgt.iter().take(4).map(|(x, y)| format!("{x}-{y}")).collect();
            println!(
                "  chunks {a:>3}-{b:<3} ({:>2} wide) -> {} target run(s): {}{}",
                b - a + 1,
                tgt.len(),
                shown.join(", "),
                if tgt.len() > 4 { " …" } else { "" }
            );
        }
    }
    println!("\n  source runs with EXACTLY ONE target run: {clean} of {total}");
}
