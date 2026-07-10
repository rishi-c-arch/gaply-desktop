//! SLM-1 (AI-detection) real runtime: a candle-backed [`PerplexityModel`].
//!
//! This is the project's FIRST real model runtime. Like `ReqwestFetcher`
//! implements gaply-core's `HttpFetcher` from the app crate, `CandlePerplexityModel`
//! implements gaply-core's `PerplexityModel` from the app crate — so gaply-core
//! stays ML-free/sync/portable and `cargo test -p gaply_core` links no ML/TLS.
//!
//! # What it does
//!
//! Loads a LOCAL quantized Qwen2 GGUF via candle (no `hf-hub` — local paths
//! only, so no native-tls/OpenSSL) plus its Hugging Face `tokenizer.json`
//! (candle does not read the tokenizer out of the GGUF), and computes real
//! per-token teacher-forced surprisal in bits for the AI-detection signal.
//!
//! # Why the incremental decode loop
//!
//! `candle_transformers::models::quantized_qwen2::ModelWeights::forward()`
//! NARROWS TO THE LAST TOKEN before the head — it returns the distribution over
//! the *next* token only, not logits for every position. So teacher-forced
//! perplexity can't be read from one batched forward. We instead step token by
//! token using the model's KV cache: at step `i` we feed the PREVIOUS token at
//! offset `i`; the returned last-token logits are `P(token_i | prefix)`, and we
//! take `-log2 P(token_i)`. Seeding the prefix with the model's BOS conditions
//! token 0. This is O(n) single-token forwards per window — the cost the
//! standalone perf probe measures before we decide on integration.
//!
//! # Sync + threading
//!
//! `PerplexityModel`'s methods are `&self`, but `forward()` needs `&mut` (it
//! mutates the KV cache), so the model sits behind a `Mutex`. The trait is
//! synchronous; the app is expected to call it under `spawn_blocking`.

use std::path::Path;
use std::sync::Mutex;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use tokenizers::Tokenizer;

use crate::models::quantized_qwen2_lowmem::ModelWeights;

use gaply_core::ai_detect::PerplexityModel;

/// Attention/window budget. 512 (down from 1024) bounds both the O(n) decode
/// loop's CPU cost and the KV-cache growth (~112KB/token for Qwen2.5-7B, plus
/// an equal transient during the per-step `Tensor::cat`) — part of the 8GB-RAM
/// fit alongside the quantized-embedding change in `quantized_qwen2_lowmem`.
/// Stride stays at half the window, preserving the 50% overlap the 1024/512
/// configuration had.
const CONTEXT_TOKENS: usize = 512;
const STRIDE: usize = 256;

/// Qwen2 uses `<|endoftext|>` as its BOS/EOS marker. Fallback id if the
/// tokenizer lookup ever fails (Qwen2 vocab places it at 151643).
const QWEN2_EOT_FALLBACK: u32 = 151643;

/// A real [`PerplexityModel`] backed by a local quantized Qwen2 GGUF via candle.
pub struct CandlePerplexityModel {
    inner: Mutex<ModelWeights>,
    device: Device,
    tokenizer: Tokenizer,
    bos: u32,
    name: String,
}

impl CandlePerplexityModel {
    /// Load from LOCAL paths only (no network): a quantized Qwen2 `*.gguf` and
    /// its Hugging Face `tokenizer.json`. CPU device.
    pub fn from_paths(model_gguf: &Path, tokenizer_json: &Path) -> Result<Self, String> {
        let device = Device::Cpu;

        let mut file = std::fs::File::open(model_gguf)
            .map_err(|e| format!("open gguf {}: {e}", model_gguf.display()))?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| format!("read gguf {}: {e}", model_gguf.display()))?;
        let model = ModelWeights::from_gguf(content, &mut file, &device)
            .map_err(|e| format!("from_gguf: {e}"))?;

        let tokenizer = Tokenizer::from_file(tokenizer_json)
            .map_err(|e| format!("load tokenizer {}: {e}", tokenizer_json.display()))?;
        let bos = tokenizer.token_to_id("<|endoftext|>").unwrap_or(QWEN2_EOT_FALLBACK);

        Ok(Self {
            inner: Mutex::new(model),
            device,
            tokenizer,
            bos,
            name: "candle-qwen2-gguf (cpu)".to_string(),
        })
    }
}

impl PerplexityModel for CandlePerplexityModel {
    fn name(&self) -> &str {
        &self.name
    }

    fn context_tokens(&self) -> usize {
        CONTEXT_TOKENS
    }

    fn stride(&self) -> usize {
        STRIDE
    }

    /// Subword tokens as their vocabulary PIECE STRINGS (e.g. "ĠThe"), so the
    /// windower's units match the model's units. `surprisals()` maps each piece
    /// string back to its exact id via the vocab — lossless, since we round-trip
    /// through the tokenizer's own strings, not decoded text.
    fn tokenize(&self, text: &str) -> Vec<String> {
        match self.tokenizer.encode(text, false) {
            Ok(enc) => enc.get_tokens().to_vec(),
            Err(_) => Vec::new(),
        }
    }

    /// Per-token surprisal in bits (`len == tokens.len()`), teacher-forced. See
    /// the module docs for why this is an incremental KV-cache decode loop.
    fn surprisals(&self, tokens: &[String]) -> Vec<f32> {
        if tokens.is_empty() {
            return Vec::new();
        }

        // Piece string -> exact vocab id. A miss (shouldn't happen for real
        // vocab pieces) falls back to BOS so length is preserved.
        let ids: Vec<u32> = tokens
            .iter()
            .map(|t| self.tokenizer.token_to_id(t).unwrap_or(self.bos))
            .collect();

        let mut model = self.inner.lock().expect("perplexity model mutex poisoned");

        // Fresh sequence: drop any KV state from a previous window. (forward()
        // also resets when index_pos == 0, but we clear explicitly for clarity.)
        model.clear_kv_cache();

        let mut out = vec![0f32; ids.len()];
        // Prefix starts at BOS so token 0 gets real conditioning: P(t0 | <bos>).
        let mut prev = self.bos;

        for (i, &tok) in ids.iter().enumerate() {
            // Feed the PREVIOUS token at offset i; last-token logits = P(t_i | prefix).
            let input = match Tensor::new(&[prev], &self.device).and_then(|t| t.reshape((1, 1))) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("perplexity: tensor build failed at {i}: {e}");
                    return out;
                }
            };
            let logits = match model.forward(&input, i) {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("perplexity: forward failed at {i}: {e}");
                    return out;
                }
            };
            // logits: [1, vocab] -> log-softmax over vocab -> [vocab].
            let bits = match logit_surprisal_bits(&logits, tok) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("perplexity: logprob read failed at {i}: {e}");
                    return out;
                }
            };
            out[i] = bits;
            prev = tok;
        }

        out
    }
}

/// Convert last-token logits `[1, vocab]` (or `[vocab]`) into the surprisal in
/// BITS of `token_id`: `-log2 P`. candle's `log_softmax` is natural log, so we
/// divide out `ln 2`.
fn logit_surprisal_bits(logits: &Tensor, token_id: u32) -> Result<f32, String> {
    let logits = logits.to_dtype(candle_core::DType::F32).map_err(|e| e.to_string())?;
    // Collapse a leading batch dim if present so we index a 1-D [vocab] tensor.
    let logits = if logits.rank() == 2 {
        logits.squeeze(0).map_err(|e| e.to_string())?
    } else {
        logits
    };
    let logprobs =
        candle_nn::ops::log_softmax(&logits, candle_core::D::Minus1).map_err(|e| e.to_string())?;
    let lp: f32 = logprobs
        .get(token_id as usize)
        .map_err(|e| e.to_string())?
        .to_scalar::<f32>()
        .map_err(|e| e.to_string())?;
    Ok((-lp) / std::f32::consts::LN_2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructing from a bogus path must error cleanly (no panic). We do NOT
    /// load a real 5+ GB GGUF in unit tests — the runtime is exercised by the
    /// standalone `perplexity_probe` example against a real local model.
    #[test]
    fn missing_model_errors_cleanly() {
        let r = CandlePerplexityModel::from_paths(
            Path::new("/nonexistent/model.gguf"),
            Path::new("/nonexistent/tokenizer.json"),
        );
        assert!(r.is_err());
    }
}
