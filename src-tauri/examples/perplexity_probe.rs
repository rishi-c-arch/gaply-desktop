//! Standalone proof for the candle SLM-1 runtime — NOT wired into the pipeline.
//!
//! Loads a local quantized Qwen2 GGUF + tokenizer.json, runs the real
//! `PerplexityModel::surprisals()` on ONE short string, and prints the per-token
//! bits, mean perplexity, and wall-clock timing (load vs. score). This is the
//! perf/sanity gate before any integration.
//!
//! Run (release is REQUIRED — a debug candle build is orders of magnitude
//! slower and would give a misleading perf number):
//!
//!   cargo run --release --example perplexity_probe -- \
//!       /path/to/model.gguf /path/to/tokenizer.json ["optional custom text"]

use std::path::Path;
use std::time::Instant;

use app_lib::models::candle_perplexity::CandlePerplexityModel;
use gaply_core::ai_detect::PerplexityModel;

fn main() {
    let mut args = std::env::args().skip(1);
    let model_path = args.next().unwrap_or_else(|| {
        eprintln!("usage: perplexity_probe <model.gguf> <tokenizer.json> [text]");
        std::process::exit(2);
    });
    let tok_path = args.next().unwrap_or_else(|| {
        eprintln!("usage: perplexity_probe <model.gguf> <tokenizer.json> [text]");
        std::process::exit(2);
    });
    let text = args.next().unwrap_or_else(|| {
        "The mitochondria is the powerhouse of the cell. Our results show a \
         statistically significant effect. This finding warrants further study."
            .to_string()
    });

    println!("model     : {model_path}");
    println!("tokenizer : {tok_path}");
    println!("text      : {text:?}\n");

    let t0 = Instant::now();
    let model = match CandlePerplexityModel::from_paths(Path::new(&model_path), Path::new(&tok_path))
    {
        Ok(m) => m,
        Err(e) => {
            eprintln!("LOAD FAILED: {e}");
            std::process::exit(1);
        }
    };
    let load_ms = t0.elapsed().as_millis();
    println!("loaded in {load_ms} ms  (model: {})\n", model.name());

    let tokens = model.tokenize(&text);
    println!("token count: {}", tokens.len());

    let t1 = Instant::now();
    let bits = model.surprisals(&tokens);
    let score_ms = t1.elapsed().as_millis();

    println!("\nper-token surprisal (bits):");
    for (tok, b) in tokens.iter().zip(bits.iter()) {
        println!("  {b:6.2}  {tok:?}");
    }

    let n = bits.len().max(1) as f64;
    let mean_bits = bits.iter().map(|b| *b as f64).sum::<f64>() / n;
    let perplexity = 2f64.powf(mean_bits);
    let per_tok = if bits.is_empty() { 0.0 } else { score_ms as f64 / bits.len() as f64 };

    println!("\n--- summary ---");
    println!("tokens          : {}", bits.len());
    println!("mean surprisal  : {mean_bits:.3} bits");
    println!("perplexity      : {perplexity:.2}");
    println!("load time       : {load_ms} ms");
    println!("score time      : {score_ms} ms  ({per_tok:.1} ms/token, CPU)");
}
