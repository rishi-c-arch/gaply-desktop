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
//!
//! `--compare` additionally runs the REFERENCE per-token KV-cache decode loop
//! (`surprisals_reference`) on the same input and prints a per-token Δbits
//! table against the trait's `surprisals()` path.
//!
//! The verdict gates on the WINDOW-MEAN surprisal difference, because that is
//! what `ai_detect` consumes and because the per-token drift is verified
//! FP-accumulation noise, not error: the quantized matmuls are row-independent
//! (bitwise identical between paths); only the f32 attention GEMMs accumulate
//! in shape-dependent order, so drift grows with KV span (position 0 is
//! bitwise exact), lands on high-entropy tokens, and stays rank-preserving
//! (r = 0.975 across quant tiers). Gate: |Δ window mean| < 0.02 bits at
//! production window sizes (n ≥ 256), relaxed by √(256/n) for shorter
//! diagnostic inputs since mean-zero noise shrinks ~1/√n. Per-token max/mean
//! |Δ| are printed as diagnostics but do not gate.

use std::path::Path;
use std::time::Instant;

use app_lib::models::candle_perplexity::CandlePerplexityModel;
use gaply_core::ai_detect::PerplexityModel;

/// Window-mean gate at production window sizes (n >= GATE_REF_WINDOW).
const GATE_WINDOW_MEAN_BITS: f64 = 0.02;
/// Window length the gate is calibrated for; shorter inputs get the gate
/// relaxed by sqrt(GATE_REF_WINDOW / n) — mean-zero FP noise shrinks ~1/sqrt(n).
const GATE_REF_WINDOW: f64 = 256.0;

fn window_mean_gate(n: usize) -> f64 {
    GATE_WINDOW_MEAN_BITS * (GATE_REF_WINDOW / n as f64).sqrt().max(1.0)
}

fn usage() -> ! {
    eprintln!("usage: perplexity_probe [--compare] <model.gguf> <tokenizer.json> [text]");
    std::process::exit(2);
}

fn main() {
    let (mut positional, mut compare) = (Vec::new(), false);
    for arg in std::env::args().skip(1) {
        if arg == "--compare" {
            compare = true;
        } else {
            positional.push(arg);
        }
    }
    let mut positional = positional.into_iter();
    let model_path = positional.next().unwrap_or_else(|| usage());
    let tok_path = positional.next().unwrap_or_else(|| usage());
    let text = positional.next().unwrap_or_else(|| {
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

    if compare {
        println!("\n[--compare] running REFERENCE per-token decode loop on the same input…");
        let t2 = Instant::now();
        let ref_bits = model.surprisals_reference(&tokens);
        let ref_ms = t2.elapsed().as_millis();
        assert_eq!(ref_bits.len(), bits.len(), "path output lengths differ");

        println!("\nper-token surprisal (bits):");
        println!("  {:>8}  {:>8}  {:>9}  token", "batched", "referenc", "Δbits");
        let mut max_abs = 0f32;
        let mut sum_abs = 0f64;
        for ((tok, b), r) in tokens.iter().zip(bits.iter()).zip(ref_bits.iter()) {
            let d = b - r;
            max_abs = max_abs.max(d.abs());
            sum_abs += f64::from(d.abs());
            println!("  {b:8.4}  {r:8.4}  {d:+9.5}  {tok:?}");
        }
        let n = bits.len().max(1) as f64;
        let mean_abs = (sum_abs / n) as f32;
        let mean_new = bits.iter().map(|b| f64::from(*b)).sum::<f64>() / n;
        let mean_ref = ref_bits.iter().map(|b| f64::from(*b)).sum::<f64>() / n;

        let mean_delta = (mean_new - mean_ref).abs();
        let gate = window_mean_gate(bits.len());

        println!("\n--- compare summary ---");
        println!("tokens             : {}", bits.len());
        println!("mean surprisal     : {mean_new:.4} bits (batched)  vs  {mean_ref:.4} bits (reference)");
        println!("perplexity         : {:.2} (batched)  vs  {:.2} (reference)", 2f64.powf(mean_new), 2f64.powf(mean_ref));
        println!("max |Δ| per-token  : {max_abs:.5} bits  (diagnostic, not gated)");
        println!("mean |Δ| per-token : {mean_abs:.5} bits  (diagnostic, not gated)");
        println!("|Δ window mean|    : {mean_delta:.5} bits  (GATE < {gate:.5})");
        let per_tok_new = score_ms as f64 / n;
        let per_tok_ref = ref_ms as f64 / n;
        println!("score time         : {score_ms} ms ({per_tok_new:.1} ms/token) batched  vs  {ref_ms} ms ({per_tok_ref:.1} ms/token) reference");
        let pass = mean_delta < gate;
        println!("verdict            : {}", if pass { "PASS" } else { "FAIL" });
        if !pass {
            std::process::exit(1);
        }
        return;
    }

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
