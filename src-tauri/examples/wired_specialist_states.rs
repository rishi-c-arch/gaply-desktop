//! **What the wired specialists actually produce, through the REAL pipeline.**
//!
//! `examples/item1_gate_trace.rs` calls `specialist::run` directly. This calls
//! `run_pipeline_measured`, which is `run_pipeline_inner` — the same function
//! the Tauri command drives — so the numbers reported are the pipeline's and not
//! a probe's approximation of it. That distinction is CLAUDE.md's
//! static-trace rule: establish which mechanism is in play before concluding
//! from either.
//!
//! Prints all three states per manuscript: FIRED with the finding, its span and
//! its severity; DECLINED with the sentence; and the specialist that ran and
//! found nothing.
use app_lib::pipeline::{run_pipeline_measured, NetworkConsent};
use gaply_core::db::Database;
use gaply_core::embed::{Embedder, HashEmbedder};
use std::sync::Arc;

fn main() {
    // Keep the deep AI tier out: this probe is about the specialist stage, and
    // a 7B load per manuscript would dominate the run without changing it.
    std::env::set_var("GAPLY_DISABLE_DEEP", "1");

    let (mut fired, mut declined, mut silent) = (0usize, 0usize, 0usize);
    for path in std::env::args().skip(1) {
        let name =
            std::path::Path::new(&path).file_name().unwrap().to_string_lossy().to_string();
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
        let out = match run_pipeline_measured(
            db,
            embedder,
            path.clone(),
            None,
            None,
            None,
            // no journal picker in this probe
            None,
            NetworkConsent::Denied,
            &|_ev| {},
        ) {
            Ok(o) => o,
            Err(e) => {
                println!("\n######## {name}\n  PIPELINE ERROR: {e:?}");
                continue;
            }
        };

        println!("\n######## {name}");
        for r in &out.specialists {
            if let Some(reason) = &r.not_applicable {
                declined += 1;
                println!("  {} -> DECLINED", r.specialist);
                println!("     reason: {reason}");
            } else if r.admitted.is_empty() {
                silent += 1;
                println!("  {} -> RAN, no findings", r.specialist);
            } else {
                fired += 1;
                println!("  {} -> FIRED ({} findings)", r.specialist, r.admitted.len());
                for f in &r.admitted {
                    println!("     severity : {:?}", f.severity);
                    println!("     code     : {}", f.code);
                    println!("     location : {:?}", f.location);
                    println!("     span     :\n{}", f.span.as_deref().unwrap_or("<none>"));
                }
            }
        }
    }
    println!("\n  across the corpus: FIRED {fired}  DECLINED {declined}  RAN-EMPTY {silent}");
}
