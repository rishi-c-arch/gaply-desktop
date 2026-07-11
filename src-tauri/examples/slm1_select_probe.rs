//! Set-2 fallback proof — which PerplexityModel does `perplexity_model()` pick?
//!
//! Prints the selected model's name so the missing-model fallback can be
//! verified both ways without running the whole pipeline:
//!   present  → cargo run --release --example slm1_select_probe
//!   absent   → GAPLY_SLM1_GGUF=/nope.gguf cargo run --release --example slm1_select_probe
//! The first should report the candle model; the second must fall back to the
//! interim HeuristicModel (never panic, never fail).

use app_lib::models::perplexity_model;

fn main() {
    let model = perplexity_model();
    println!("selected PerplexityModel: {}", model.name());
}
