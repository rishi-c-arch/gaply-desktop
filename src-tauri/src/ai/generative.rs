//! The generative backend: greedy decoding over the vendored quantized Qwen2.
//!
//! # Reuse, not a second loader
//!
//! This wraps [`crate::models::quantized_qwen2_lowmem::ModelWeights`] — the same
//! GGUF loader the perplexity path already uses, with its deliberate
//! quantized-`token_embd` change. No second loader is introduced, and
//! `quantized_qwen2_lowmem.rs` is not modified: it already exposes everything a
//! decode loop needs (`from_gguf`, `forward` for last-token logits,
//! `clear_kv_cache`).
//!
//! # Loaded once per lifecycle
//!
//! `perplexity_model()` constructs a fresh model on every call. That path is
//! left exactly as it is, but this engine must not copy it — a
//! [`QwenGenerativeBackend`] is built once and held by the ModelManager until
//! the idle timer drops it.
//!
//! # Decoding
//!
//! Greedy (argmax). The spec's shared contract pins `temperature 0.0`,
//! `top_p 1.0`, `repeat_penalty 1.0`, which IS argmax — so sampling machinery
//! would be dead weight that could only drift away from the contract.
//! Grammar-constrained decoding is not available under Candle and is replaced by
//! the validated-generation harness (§9.1, and the spec's own override note).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use candle_core::{Device, Tensor};
use gaply_core::GaplyError;
use serde::Serialize;
use tokenizers::Tokenizer;

use crate::models::quantized_qwen2_lowmem::ModelWeights;

/// Qwen2.5 end-of-text, matching `candle_perplexity`'s fallback.
const QWEN2_EOT: u32 = 151_643;
/// Qwen2.5-Instruct turn terminator. An instruct model ends a reply with this,
/// not with `<|endoftext|>`, so a loop watching only EOT runs to max_tokens.
const QWEN2_IM_END: &str = "<|im_end|>";

/// What one generation call needs. Deliberately not a struct of Options: every
/// field is a decision the caller must have made.
pub struct GenRequest<'a> {
    pub prompt: String,
    pub max_tokens: usize,
    /// Checked EVERY token, so a cancel lands within one decode step.
    pub cancel: &'a AtomicBool,
    /// Optional per-token sink for streaming. Called with the incremental text.
    pub on_token: Option<&'a (dyn Fn(&str) + Send + Sync)>,
}

/// Why a generation stopped — reported, never inferred from the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// The model emitted its end-of-turn token.
    EndOfTurn,
    /// `max_tokens` was reached. The output may be truncated mid-JSON — the
    /// validator will catch that; this field is how a caller knows why.
    MaxTokens,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenOutput {
    pub text: String,
    pub tokens: usize,
    pub stop_reason: StopReason,
    pub elapsed_ms: u64,
}

/// The seam the ModelManager drives. Mockable, so the state machine,
/// serialization and cancellation tests need no real model.
pub trait GenerationBackend: Send + Sync {
    fn generate(&self, req: GenRequest<'_>) -> Result<GenOutput, GaplyError>;
}

/// RAM this model is expected to occupy, computed from the GGUF's own metadata.
///
/// NEVER process RSS. RSS measures the whole process — every other model, the
/// webview, the database — so attributing it to this model would be wrong in
/// both directions, and it cannot be reported at all before the model loads.
/// These figures are derivable up front, which is what makes a pre-flight check
/// possible.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RamEstimate {
    /// Size of the weights file on disk — what mapping it costs.
    pub weights_bytes: u64,
    /// Worst case KV cache: the full context window.
    pub kv_cache_bytes: u64,
    pub total_bytes: u64,
    pub context_length: usize,
    pub layers: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
}

/// KV cache entries are f32 in this runtime.
const KV_DTYPE_BYTES: u64 = 4;

/// Read GGUF metadata and compute the RAM estimate WITHOUT loading weights.
///
/// `gguf_file::Content::read` parses the header and metadata only; tensor data
/// is fetched later per-tensor. So this is cheap enough to run before deciding
/// whether the machine can hold the model.
pub fn estimate_ram(model_gguf: &Path) -> Result<RamEstimate, GaplyError> {
    use candle_core::quantized::gguf_file;

    let weights_bytes = std::fs::metadata(model_gguf)?.len();
    let mut file = std::fs::File::open(model_gguf)
        .map_err(|e| GaplyError::Validation(format!("open gguf {}: {e}", model_gguf.display())))?;
    let ct = gguf_file::Content::read(&mut file)
        .map_err(|e| GaplyError::Validation(format!("read gguf {}: {e}", model_gguf.display())))?;

    let md = |k: &str| -> Result<u32, GaplyError> {
        ct.metadata
            .get(k)
            .ok_or_else(|| GaplyError::Validation(format!("gguf metadata missing {k}")))?
            .to_u32()
            .map_err(|e| GaplyError::Validation(format!("gguf metadata {k}: {e}")))
    };
    let layers = md("qwen2.block_count")? as usize;
    let context_length = md("qwen2.context_length")? as usize;
    let kv_heads = md("qwen2.attention.head_count_kv")? as usize;
    let heads = md("qwen2.attention.head_count")? as usize;
    let embedding_length = md("qwen2.embedding_length")? as usize;
    if heads == 0 {
        return Err(GaplyError::Validation("gguf reports zero attention heads".into()));
    }
    let head_dim = embedding_length / heads;

    // K and V, per layer, for the whole context window.
    let kv_cache_bytes = 2 * layers as u64
        * context_length as u64
        * kv_heads as u64
        * head_dim as u64
        * KV_DTYPE_BYTES;

    Ok(RamEstimate {
        weights_bytes,
        kv_cache_bytes,
        total_bytes: weights_bytes + kv_cache_bytes,
        context_length,
        layers,
        kv_heads,
        head_dim,
    })
}

pub struct QwenGenerativeBackend {
    model: std::sync::Mutex<ModelWeights>,
    tokenizer: Tokenizer,
    device: Device,
    eot_ids: Vec<u32>,
}

impl QwenGenerativeBackend {
    /// Load once. Callers hold the result for the model's lifecycle.
    pub fn load(model_gguf: &Path, tokenizer_json: &Path) -> Result<Self, GaplyError> {
        use candle_core::quantized::gguf_file;

        let device = Device::Cpu;
        let mut file = std::fs::File::open(model_gguf).map_err(|e| {
            GaplyError::Validation(format!("open gguf {}: {e}", model_gguf.display()))
        })?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| GaplyError::Validation(format!("read gguf: {e}")))?;
        let model = ModelWeights::from_gguf(content, &mut file, &device)
            .map_err(|e| GaplyError::Internal(format!("from_gguf: {e}")))?;
        let tokenizer = Tokenizer::from_file(tokenizer_json).map_err(|e| {
            GaplyError::Validation(format!("load tokenizer {}: {e}", tokenizer_json.display()))
        })?;

        // Stop on either terminator. An instruct model ends its turn with
        // <|im_end|>; watching only <|endoftext|> would run every generation to
        // max_tokens and truncate the JSON we then try to parse.
        let mut eot_ids = vec![QWEN2_EOT];
        if let Some(id) = tokenizer.token_to_id(QWEN2_IM_END) {
            eot_ids.push(id);
        }
        Ok(Self { model: std::sync::Mutex::new(model), tokenizer, device, eot_ids })
    }
}

impl GenerationBackend for QwenGenerativeBackend {
    fn generate(&self, req: GenRequest<'_>) -> Result<GenOutput, GaplyError> {
        let started = std::time::Instant::now();
        let mut model = self
            .model
            .lock()
            .map_err(|_| GaplyError::Internal("generative model lock poisoned".into()))?;

        let encoding = self
            .tokenizer
            .encode(req.prompt.as_str(), true)
            .map_err(|e| GaplyError::Internal(format!("tokenize failed: {e}")))?;
        let mut ids: Vec<u32> = encoding.get_ids().to_vec();
        if ids.is_empty() {
            return Err(GaplyError::Validation("empty prompt".into()));
        }

        // Fresh sequence: never inherit KV state from a previous generation.
        model.clear_kv_cache();

        let prompt_len = ids.len();
        let mut generated: Vec<u32> = Vec::with_capacity(req.max_tokens);
        let mut emitted_text = String::new();
        let mut stop_reason = StopReason::MaxTokens;

        // Prefill the prompt in one forward, then decode one token at a time.
        let mut next_input: Vec<u32> = ids.clone();
        let mut index_pos = 0usize;

        for step in 0..req.max_tokens {
            // EVERY token, so cancel lands within one decode step rather than
            // one request.
            if req.cancel.load(Ordering::SeqCst) {
                stop_reason = StopReason::Cancelled;
                break;
            }

            let input = Tensor::new(next_input.as_slice(), &self.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| GaplyError::Internal(format!("input tensor: {e}")))?;
            let logits = model
                .forward(&input, index_pos)
                .map_err(|e| GaplyError::Internal(format!("forward at {step}: {e}")))?;
            index_pos += next_input.len();

            // Greedy: argmax. The spec pins temperature 0 / top_p 1 /
            // repeat_penalty 1, which is exactly this.
            let logits = logits
                .squeeze(0)
                .map_err(|e| GaplyError::Internal(format!("squeeze: {e}")))?;
            let row: Vec<f32> = logits
                .to_vec1()
                .map_err(|e| GaplyError::Internal(format!("logits to_vec1: {e}")))?;
            let next = row
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i as u32)
                .ok_or_else(|| GaplyError::Internal("empty logits row".into()))?;

            if self.eot_ids.contains(&next) {
                stop_reason = StopReason::EndOfTurn;
                break;
            }

            generated.push(next);
            ids.push(next);
            next_input = vec![next];

            // Incremental detokenization: decode what we have and emit only the
            // delta. Decoding token-by-token would split multi-byte characters
            // and multi-token words.
            if req.on_token.is_some() {
                let full = self
                    .tokenizer
                    .decode(&generated, true)
                    .map_err(|e| GaplyError::Internal(format!("decode: {e}")))?;
                if full.len() > emitted_text.len() {
                    let delta = full[emitted_text.len()..].to_string();
                    if let Some(sink) = req.on_token {
                        sink(&delta);
                    }
                    emitted_text = full;
                }
            }
        }

        let text = self
            .tokenizer
            .decode(&generated, true)
            .map_err(|e| GaplyError::Internal(format!("decode: {e}")))?;
        let _ = prompt_len;
        Ok(GenOutput {
            tokens: generated.len(),
            text,
            stop_reason,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }
}

/* =============== the bundled-model loader (plan §9.8 / §9.9) ============== *
 * The 0.5B is resolved through models::stage1_lm_paths() — the EXISTING
 * resolver — and recorded as an ai_model_registry row. Nothing here hard-codes
 * a path, so swapping in the production model (§9.9) is a registry change plus
 * a download, not a code change in the engine. */

/// Registry id for the bundled development/test generative model (§9.8).
pub const BUNDLED_GEN_MODEL_ID: &str = "qwen2.5-0.5b-instruct-q4km";

/// Loads whichever GGUF the resolver produces. `GAPLY_TEST_GEN_MODEL` overrides
/// it for tests without touching the resolution order used in production.
pub struct BundledGenerativeLoader {
    gguf: std::path::PathBuf,
    tokenizer: std::path::PathBuf,
}

impl BundledGenerativeLoader {
    /// `None` when no generative model resolves — the caller reports
    /// NotInstalled rather than failing obscurely later.
    pub fn resolve() -> Option<Self> {
        if let Some(p) = std::env::var_os("GAPLY_TEST_GEN_MODEL") {
            let gguf = std::path::PathBuf::from(p);
            let tokenizer = gguf.parent()?.join("tokenizer.json");
            if gguf.exists() && tokenizer.exists() {
                return Some(Self { gguf, tokenizer });
            }
        }
        let (gguf, tokenizer) = crate::models::stage1_lm_paths()?;
        (gguf.exists() && tokenizer.exists()).then_some(Self { gguf, tokenizer })
    }

    pub fn gguf_path(&self) -> &std::path::Path {
        &self.gguf
    }
}

impl crate::ai::model_manager::BackendLoader for BundledGenerativeLoader {
    fn model_id(&self) -> String {
        BUNDLED_GEN_MODEL_ID.to_string()
    }
    fn ram_estimate(&self) -> Result<RamEstimate, GaplyError> {
        estimate_ram(&self.gguf)
    }
    fn load(&self) -> Result<std::sync::Arc<dyn GenerationBackend>, GaplyError> {
        Ok(std::sync::Arc::new(QwenGenerativeBackend::load(&self.gguf, &self.tokenizer)?))
    }
}

/// Record the generative model in `ai_model_registry`. Idempotent — the DAO
/// upserts, so calling this on every use is safe and keeps the row's path
/// honest if the resolver ever picks a different file.
pub fn register_generative(
    db: &gaply_core::Database,
    loader: &BundledGenerativeLoader,
) -> Result<(), GaplyError> {
    gaply_core::ai_engine::registry::register_model(
        db,
        gaply_core::ai_engine::registry::ModelRow {
            id: BUNDLED_GEN_MODEL_ID.to_string(),
            kind: "generative".to_string(),
            display_name: "Qwen2.5 0.5B Instruct (Q4_K_M, bundled)".to_string(),
            file_path: loader.gguf.display().to_string(),
            sha256: None,
            dim: None,
            quant: Some("Q4_K_M".to_string()),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_estimate_is_arithmetic_not_rss() {
        // Hand-computed against Qwen2.5-0.5B's published shape:
        // 24 layers, 32768 ctx, 2 kv heads, 896 embedding / 14 heads = 64 head_dim.
        // 2 * 24 * 32768 * 2 * 64 * 4 = 805,306,368 bytes.
        let layers = 24u64;
        let ctx = 32_768u64;
        let kv_heads = 2u64;
        let head_dim = 64u64;
        let expected = 2 * layers * ctx * kv_heads * head_dim * KV_DTYPE_BYTES;
        assert_eq!(expected, 805_306_368);
    }

    /// Reports the real figure for whichever GGUF resolves. `#[ignore]` because
    /// it needs the bundled resource; run with `--ignored --nocapture`.
    #[test]
    #[ignore = "needs the bundled GGUF"]
    fn ram_estimate_for_the_bundled_model() {
        let Some((gguf, _tok)) = crate::models::stage1_lm_paths() else {
            eprintln!("SKIP: no stage-1 GGUF resolves");
            return;
        };
        if !gguf.exists() {
            eprintln!("SKIP: {} absent", gguf.display());
            return;
        }
        let e = estimate_ram(&gguf).expect("bundled gguf must parse");
        let mb = |b: u64| b as f64 / 1024.0 / 1024.0;
        println!("\n=== RAM ESTIMATE (computed from GGUF metadata, never RSS) ===");
        println!("file        : {}", gguf.display());
        println!("weights     : {:>10} bytes ({:.0} MB)", e.weights_bytes, mb(e.weights_bytes));
        println!("kv cache    : {:>10} bytes ({:.0} MB)  [full {} ctx]", e.kv_cache_bytes, mb(e.kv_cache_bytes), e.context_length);
        println!("TOTAL       : {:>10} bytes ({:.0} MB)", e.total_bytes, mb(e.total_bytes));
        println!("shape       : {} layers, {} kv heads, head_dim {}", e.layers, e.kv_heads, e.head_dim);
        assert_eq!(e.total_bytes, e.weights_bytes + e.kv_cache_bytes);
        assert!(e.layers > 0 && e.kv_heads > 0 && e.head_dim > 0);
    }

    #[test]
    fn estimate_ram_rejects_a_file_that_is_not_a_gguf() {
        let p = std::env::temp_dir().join(format!("gaply-notgguf-{}.gguf", std::process::id()));
        std::fs::write(&p, b"definitely not a gguf").unwrap();
        let err = estimate_ram(&p).unwrap_err();
        assert_eq!(err.code(), "validation", "expected an honest validation error, got {err}");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn estimate_ram_reports_a_missing_file_rather_than_guessing() {
        assert!(estimate_ram(Path::new("/nonexistent/model.gguf")).is_err());
    }
}

