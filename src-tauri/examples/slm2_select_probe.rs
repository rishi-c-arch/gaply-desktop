//! Set-3 fallback proof — which ProxyClient does `verify_proxy()` pick?
//!
//! Prints the verification backend the pipeline would route to, so the
//! missing-Ollama fallback can be verified both ways without a full run:
//!   Ollama up    → cargo run --release --example slm2_select_probe
//!   Ollama down  → GAPLY_SLM2_ENDPOINT=http://127.0.0.1:1 cargo run --release --example slm2_select_probe
//! The first should report `ollama …`; the second must fall back to `mock …`
//! (honest UNKNOWN verdicts) — never panic, never fail the pipeline.

use app_lib::models::verify_backend_label;

fn main() {
    println!("SLM-2 verification backend: {}", verify_backend_label());
}
