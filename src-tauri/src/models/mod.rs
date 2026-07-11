//! App-crate model runtimes.
//!
//! These implement gaply-core's model traits (e.g. `PerplexityModel`) using
//! real ML backends. They live in the APP CRATE — never in gaply-core — so the
//! portable core stays sync, network-free and ML-free (the `ReqwestFetcher`
//! precedent): `cargo test -p gaply_core` pulls none of candle/tokenizers, and
//! Windows CI's core-test job stays fast and green.

pub mod candle_perplexity;
pub mod ollama_verify;
pub mod proxy_client;
pub mod quantized_qwen2_lowmem;

use std::path::{Path, PathBuf};

use gaply_core::ai_detect::{HeuristicModel, PerplexityModel};
use gaply_core::verify_agent::{MockProxyClient, ProxyClient};

use crate::models::candle_perplexity::CandlePerplexityModel;
use crate::models::ollama_verify::OllamaVerifyClient;
use crate::models::proxy_client::ProxyReqwestClient;

/// Default SLM-2 endpoint/model when the env overrides are unset.
const DEFAULT_SLM2_ENDPOINT: &str = "http://127.0.0.1:11434";
const DEFAULT_SLM2_MODEL: &str = "qwen3:4b";

/// Home dir for the conventional `~/gaply-models/...` layout the
/// `perplexity_probe` uses. Unix `HOME`, Windows `USERPROFILE`.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// First `*.gguf` in `dir`, if any. Lets the tier-appropriate model file
/// (Q3_K_M on 8GB, Q4_K_M on 16GB) be picked up without hardcoding a quant.
fn first_gguf(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("gguf"))
}

/// Resolve the SLM-1 GGUF + tokenizer paths. Explicit overrides
/// `GAPLY_SLM1_GGUF` / `GAPLY_SLM1_TOKENIZER` win; otherwise the conventional
/// `~/gaply-models/slm1/*.gguf` and `~/gaply-models/slm1-adapter/tokenizer.json`.
/// Returns `None` if paths can't be formed (no home dir, or no GGUF present).
fn slm1_paths() -> Option<(PathBuf, PathBuf)> {
    let gguf = match std::env::var_os("GAPLY_SLM1_GGUF") {
        Some(p) => PathBuf::from(p),
        None => first_gguf(&home_dir()?.join("gaply-models").join("slm1"))?,
    };
    let tokenizer = match std::env::var_os("GAPLY_SLM1_TOKENIZER") {
        Some(p) => PathBuf::from(p),
        None => home_dir()?
            .join("gaply-models")
            .join("slm1-adapter")
            .join("tokenizer.json"),
    };
    Some((gguf, tokenizer))
}

/// The SLM-1 perplexity model for the AI-detection lane: the real candle
/// `CandlePerplexityModel` when its GGUF + tokenizer are present and load,
/// otherwise the interim [`HeuristicModel`]. NEVER fails — a user who hasn't
/// downloaded the model must still get a working pipeline (both types implement
/// [`PerplexityModel`], so the choice is a clean either/or at the trait
/// boundary). Callers get a boxed trait object and don't know which ran.
pub fn perplexity_model() -> Box<dyn PerplexityModel> {
    match slm1_paths() {
        Some((gguf, tokenizer)) if gguf.exists() && tokenizer.exists() => {
            match CandlePerplexityModel::from_paths(&gguf, &tokenizer) {
                Ok(m) => {
                    tracing::info!(gguf = %gguf.display(), "SLM-1: loaded candle perplexity model");
                    Box::new(m)
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "SLM-1: candle model present but failed to load; falling back to HeuristicModel"
                    );
                    Box::new(HeuristicModel::gpt2_like())
                }
            }
        }
        _ => {
            tracing::warn!(
                "SLM-1: model files absent (set GAPLY_SLM1_GGUF/GAPLY_SLM1_TOKENIZER or populate \
                 ~/gaply-models/slm1); using interim HeuristicModel"
            );
            Box::new(HeuristicModel::gpt2_like())
        }
    }
}

/// Resolve the SLM-2 endpoint + model tag: `GAPLY_SLM2_ENDPOINT` /
/// `GAPLY_SLM2_MODEL` win, else localhost defaults.
fn slm2_endpoint_model() -> (String, String) {
    let endpoint = std::env::var("GAPLY_SLM2_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_SLM2_ENDPOINT.to_string());
    let model =
        std::env::var("GAPLY_SLM2_MODEL").unwrap_or_else(|_| DEFAULT_SLM2_MODEL.to_string());
    (endpoint, model)
}

/// Human-readable label of which verification backend [`verify_proxy`] would
/// pick right now (`ollama …` when reachable, `mock …` when not) — for probes
/// and logging; runs the SAME reachability decision as `verify_proxy`.
pub fn verify_backend_label() -> String {
    // Mirror verify_proxy's tier order: cloud → ollama → mock.
    if let Ok(client) = ProxyReqwestClient::from_env() {
        if client.reachable() {
            return "cloud proxy".to_string();
        }
    }
    let (endpoint, model) = slm2_endpoint_model();
    match OllamaVerifyClient::with_endpoint(&endpoint, &model) {
        Ok(c) if c.reachable() => format!("ollama ({endpoint}, {model})"),
        _ => format!("mock (no cloud proxy; Ollama unreachable at {endpoint})"),
    }
}

/// Explicitly unload the SLM-2 model from Ollama and verify it's gone, after
/// the verification stage. The 8GB OOM guard: guarantees qwen3:4b is out of
/// memory before a subsequent analysis loads SLM-1 (candle) — belt-and-
/// suspenders over the request's `keep_alive: 0`. No-op/harmless when Ollama
/// isn't running or the model was never loaded.
pub fn unload_slm2() {
    let (endpoint, model) = slm2_endpoint_model();
    match OllamaVerifyClient::with_endpoint(&endpoint, &model) {
        Ok(client) if client.unload() => {
            tracing::info!(%endpoint, %model, "SLM-2: model unloaded and confirmed out of memory");
        }
        Ok(_) => {
            tracing::warn!(
                %endpoint, %model,
                "SLM-2: unload requested but model still appears resident (or Ollama unreachable)"
            );
        }
        Err(e) => tracing::warn!(error = %e, "SLM-2: unload client build failed"),
    }
}

/// The verification [`ProxyClient`] for the pipeline's verify stage: the real
/// [`OllamaVerifyClient`] when Ollama is reachable, otherwise gaply-core's
/// [`MockProxyClient`] returning empty verdicts (→ every citation UNKNOWN).
/// NEVER makes the pipeline fail for a user without Ollama running — the mirror
/// of [`perplexity_model`]'s missing-model fallback.
pub fn verify_proxy() -> Box<dyn ProxyClient> {
    // Tier 1 — cloud proxy (deep reasoning). Selected ONLY when the App Check
    // signing key is provisioned (from_env → keychain; Err = skip) AND the
    // proxy answers /health. Otherwise fall through to local, cleanly.
    match ProxyReqwestClient::from_env() {
        Ok(client) if client.reachable() => {
            tracing::info!("verification: routing to cloud proxy");
            return Box::new(client);
        }
        Ok(_) => tracing::debug!("cloud proxy configured but unreachable; falling through to local"),
        Err(_) => tracing::debug!("cloud proxy signing key absent; using local verification"),
    }

    // Tier 2 — local Ollama SLM-2.
    let (endpoint, model) = slm2_endpoint_model();
    match OllamaVerifyClient::with_endpoint(&endpoint, &model) {
        Ok(client) if client.reachable() => {
            tracing::info!(%endpoint, %model, "SLM-2: routing verification to local Ollama");
            Box::new(client)
        }
        // Tier 3 — mock: honest empty verdicts (every citation UNKNOWN).
        _ => {
            tracing::warn!(
                %endpoint,
                "SLM-2: Ollama unreachable; verification falls back to UNKNOWN verdicts"
            );
            Box::new(MockProxyClient::returning(serde_json::json!({ "verdicts": [] })))
        }
    }
}
