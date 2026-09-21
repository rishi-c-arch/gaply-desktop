//! **SLM-2 measured on its own task** (§11 D201): AI-Check passage
//! classification, base versus base+LoRA, over a committed three-class set.
//!
//! §11 D199 found SLM-2 is a LoRA r=32 over `Qwen3-4B-Thinking-2507` and that
//! nothing had ever run it — `models/mod.rs:972` declines the lane on a Set-4
//! probe that names `qwen3:4b`, the STOCK base, because no LoRA path existed.
//! Ollama's `ADAPTER` directive is that path for a probe, so the tuned model
//! can now be asked the question the untuned one was declined on.
//!
//! It drives the PRODUCT's own path, not a convenient stand-in: the envelope is
//! `gaply_core::ai_detect::classify_payload`, the client is the shipped
//! `OllamaVerifyClient` as `ClassifyClient` (same system prompt, `think:false`,
//! the schema as Ollama's `format`, `temperature 0`, `num_ctx 16384`,
//! `keep_alive:0`), so a row here is what the app would have got.
//!
//! Two of `gate_classification`'s rules are mirrored here rather than called —
//! it is private — and they are the two that decide a row: the category must be
//! one of the three schema strings, and `quote` must be a VERBATIM substring of
//! the passage. Mirrored, so if that gate changes this probe's baseline drifts;
//! it is named here so the next person knows to re-check it.
//!
//!   cargo run --release --example slm2_classify_probe -- <set.json> <ollama-tag> <out.json> [limit]
use std::io::Write;
use std::time::Instant;

use gaply_core::ai_detect::{classify_payload, ClassifyClient};
use serde_json::{json, Value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let set_path = args.get(1).expect("usage: <set.json> <ollama-tag> <out.json> [limit]");
    let tag = args.get(2).expect("ollama model tag");
    let out_path = args.get(3).expect("output path");
    let limit: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);

    let endpoint = std::env::var("GAPLY_SLM2_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let client = app_lib::models::ollama_verify::OllamaVerifyClient::with_endpoint(&endpoint, tag)
        .expect("ollama client");
    assert!(client.reachable(), "ollama not reachable at {endpoint}");

    let set: Value =
        serde_json::from_str(&std::fs::read_to_string(set_path).expect("set")).expect("set json");
    let cases = set["cases"].as_array().expect("cases").clone();

    let mut rows: Vec<Value> = Vec::new();
    for case in cases.iter().take(limit) {
        let id = case["id"].as_str().unwrap_or("?");
        let truth = case["truth"].as_str().unwrap_or("?");
        let text = case["text"].as_str().expect("text");
        let t0 = Instant::now();
        let res = client.classify(&classify_payload(text));
        let secs = t0.elapsed().as_secs_f64();

        let row = match res {
            Ok(v) => {
                // gate rule 1: the category must be one of the three schema strings.
                let cat = v["category"].as_str().unwrap_or_default().to_string();
                let on_schema =
                    matches!(cat.as_str(), "ai_generated" | "ai_paraphrased" | "unclear");
                // gate rule 2: the quote must be verbatim from the passage.
                let quote = v["quote"].as_str().unwrap_or_default().trim().to_string();
                let verbatim = !quote.is_empty() && text.contains(quote.as_str());
                json!({
                    "id": id, "truth": truth, "secs": (secs * 10.0).round() / 10.0,
                    "outcome": if on_schema { "classified" } else { "off_schema" },
                    "category": cat,
                    "strength": v["strength"].as_str().unwrap_or_default(),
                    "quote_verbatim": verbatim,
                    "quote": quote.chars().take(160).collect::<String>(),
                    "rationale": v["rationale"].as_str().unwrap_or_default()
                        .chars().take(400).collect::<String>(),
                })
            }
            Err(e) => json!({
                "id": id, "truth": truth, "secs": (secs * 10.0).round() / 10.0,
                "outcome": "no_answer", "category": "", "error": e.to_string(),
            }),
        };
        println!(
            "{id} {truth:11} {:>6.1}s {:10} {}",
            row["secs"].as_f64().unwrap_or(0.0),
            row["outcome"].as_str().unwrap_or(""),
            row["category"].as_str().unwrap_or("")
        );
        // Flush and checkpoint after EVERY row. Rust block-buffers a redirected
        // stdout, so without this a run in progress and a run that produced
        // nothing are the same empty file — and a long run that is interrupted
        // loses every row it had already paid for.
        let _ = std::io::stdout().flush();
        rows.push(row);
        let _ = std::fs::write(
            format!("{out_path}.partial"),
            serde_json::to_string_pretty(&json!({"rows": rows})).unwrap_or_default(),
        );
    }

    let doc = json!({
        "_comment": format!(
            "Raw SLM-2 AI-Check classification rows (§11 D201). Ollama tag {tag:?} at {endpoint}, \
             driven through the shipped OllamaVerifyClient as ClassifyClient with \
             gaply_core::ai_detect::classify_payload — the product's own envelope, system prompt, \
             think:false, output_schema as Ollama's format, temperature 0, num_ctx 16384, \
             keep_alive:0 (so every row includes a model load). Base is the stock \
             Qwen3-4B-Thinking-2507 Q4_K_M from rishibrucelee/gaply-slm-models; tuned is that same \
             GGUF blob plus the repo's slm2-adapter LoRA r=32 via Ollama's ADAPTER directive. \
             Set: {set_path}."),
        "model_tag": tag, "endpoint": endpoint, "set": set_path,
        "rows": rows,
    });
    std::fs::write(out_path, serde_json::to_string_pretty(&doc).expect("ser")).expect("write");
    println!("wrote {out_path} ({} rows)", doc["rows"].as_array().unwrap().len());
}
