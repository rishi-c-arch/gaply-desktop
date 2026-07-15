//! Measure a model's ABSOLUTE human-academic perplexity norm over the committed
//! calibration corpus — the provenance behind an entry in
//! `gaply-core/calibration/stage1_norms.json`.
//!
//! For each `calibration/human_academic/*.txt` fixture it computes the SAME
//! log-space document perplexity the flow uses for the Stage-1 / deep-verifier
//! signal (`document_perplexity(strided_surprisals(model, tokens, ctx, stride))`
//! over the whole text — context-preserving, matching `stage1_score`'s doc_ppl),
//! then prints n / median / mean / p10 / p90 / min / max ready to paste into the
//! norms table. Same metric for the 0.5B, the 1.5B, and (later) the 7B, so the
//! per-model norms are apples-to-apples with what `analyze_tiered` places.
//!
//! Run (release REQUIRED — a debug candle build is ~40x slower):
//!
//!   cargo run --release --example stage1_norms_probe -- \
//!       /path/to/model.gguf /path/to/tokenizer.json \
//!       src-tauri/gaply-core/calibration/human_academic

use std::path::Path;
use std::time::Instant;

use app_lib::models::candle_perplexity::CandlePerplexityModel;
use gaply_core::ai_detect::{document_perplexity, strided_surprisals, PerplexityModel};

fn usage() -> ! {
    eprintln!("usage: stage1_norms_probe <model.gguf> <tokenizer.json> <corpus_dir>");
    std::process::exit(2);
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    // Nearest-rank on a sorted ascending slice.
    let rank = (p / 100.0 * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let model_path = args.next().unwrap_or_else(|| usage());
    let tok_path = args.next().unwrap_or_else(|| usage());
    let corpus_dir = args.next().unwrap_or_else(|| usage());

    let t0 = Instant::now();
    let model = match CandlePerplexityModel::from_paths(Path::new(&model_path), Path::new(&tok_path)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("LOAD FAILED: {e}");
            std::process::exit(1);
        }
    };
    println!("model     : {} (loaded in {} ms)", model.name(), t0.elapsed().as_millis());
    println!("corpus    : {corpus_dir}\n");

    let mut entries: Vec<_> = std::fs::read_dir(&corpus_dir)
        .unwrap_or_else(|e| {
            eprintln!("cannot read corpus dir {corpus_dir}: {e}");
            std::process::exit(1);
        })
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("txt"))
        .collect();
    entries.sort();

    let mut ppls: Vec<f64> = Vec::new();
    for path in &entries {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skip {}: {e}", path.display());
                continue;
            }
        };
        let toks = model.tokenize(&text);
        if toks.len() < 3 {
            eprintln!("skip {} (too short: {} tokens)", path.display(), toks.len());
            continue;
        }
        let t1 = Instant::now();
        let surp = strided_surprisals(&model, &toks, model.context_tokens(), model.stride());
        let ppl = document_perplexity(&surp);
        ppls.push(ppl);
        println!(
            "  {:>7.2}  ({:>4} tok, {:>5} ms)  {}",
            ppl,
            toks.len(),
            t1.elapsed().as_millis(),
            path.file_name().and_then(|s| s.to_str()).unwrap_or("?")
        );
    }

    if ppls.is_empty() {
        eprintln!("no fixtures scored");
        std::process::exit(1);
    }
    ppls.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = ppls.len();
    let mean = ppls.iter().sum::<f64>() / n as f64;
    let median = percentile(&ppls, 50.0);
    let r1 = |x: f64| (x * 10.0).round() / 10.0;

    println!("\n--- norm ({n} fixtures) ---");
    println!("  \"n\": {n},");
    println!("  \"median\": {},", r1(median));
    println!("  \"mean\": {},", r1(mean));
    println!("  \"p10\": {},", r1(percentile(&ppls, 10.0)));
    println!("  \"p90\": {},", r1(percentile(&ppls, 90.0)));
    println!("  \"min\": {},", r1(ppls[0]));
    println!("  \"max\": {}", r1(ppls[n - 1]));
}
