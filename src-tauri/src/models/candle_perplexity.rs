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
//! # Model tiers (RAM-driven)
//!
//! - **8GB machines: Qwen2.5-7B-Instruct Q3_K_M** (~3.8GB file). Verified on
//!   an 8GB M1 Air: 5.45GB peak footprint (~2.5GB headroom), ~100ms/token.
//! - **16GB machines: Qwen2.5-7B-Instruct Q4_K_M** (~4.4GB file). Does NOT fit
//!   8GB: candle's aarch64 kernel caches a repacked copy of every Q4K tensor
//!   (~+3.1GB → ~7.5GB base) and the process gets jetsam-killed on real
//!   windows (observed at 399 tokens, twice).
//!
//! Q3 tracks Q4 per-token surprisal at r = 0.975 (rank-preserving), but the
//! ABSOLUTE calibration differs by ~+0.13 bits mean — any detection threshold
//! must be calibrated PER TIER, never shared across quant levels.
//!
//! # Batched scoring (default) vs the reference decode loop
//!
//! Teacher-forced surprisal needs logits at EVERY position. Upstream
//! `forward()` narrows to the last token, so the original implementation
//! stepped token by token through the KV cache — one full weight-streaming
//! forward per token (~2.6 s/token for 7B Q4 on CPU). That loop survives as
//! [`CandlePerplexityModel::surprisals_reference`], the ground truth for
//! `perplexity_probe --compare`.
//!
//! The trait's `surprisals()` now feeds the window `[bos, t0..t_{n-2}]`
//! through the vendored model's `forward_all()` in `PREFILL_CHUNK`-token
//! prefill chunks against the KV cache — logits row `i` is
//! `P(token_i | prefix)`, identical conditioning to the loop (BOS seeds token
//! 0). Both chunkings (prefill and `SCORE_CHUNK` log-softmax rows) bound the
//! transient memory; see the const docs — on aarch64, candle's repacked-Q4K
//! kernel cache leaves only a few hundred MB of headroom on 8GB machines.
//! The two paths agree to float tolerance, not bitwise: the f32 attention
//! GEMMs accumulate in shape-dependent order, so drift grows with position
//! (position 0 is exact) and concentrates on high-entropy tokens.
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

/// Tokens fed per `forward_all()` call. Prefilling the window in chunks
/// against the KV cache is mathematically identical to one full-window pass
/// (`build_causal_mask` handles `index_pos > 0` prefill), but it bounds the
/// logits tensor to `PREFILL_CHUNK × vocab` (~78MB) instead of
/// `window × vocab` (~311MB at 512). That margin is NOT optional: on aarch64
/// candle also caches a repacked copy of every Q4K tensor on first matmul
/// (~+3.1GB for this model, on top of the ~4.4GB of QTensors), so the 8GB
/// machines this must fit leave only a few hundred MB for activations —
/// a full-window logits tensor was observed to OOM at 399 tokens.
const PREFILL_CHUNK: usize = 128;

/// Rows of a chunk's `[PREFILL_CHUNK, vocab]` logits scored per
/// log-softmax/gather round. Bounds softmax intermediates to
/// ~3 × 64 × 152k × 4B ≈ 117MB.
const SCORE_CHUNK: usize = 64;

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

/// Default display name when a caller doesn't specify a tier identity.
const DEFAULT_MODEL_NAME: &str = "candle-qwen2-gguf (cpu)";

impl CandlePerplexityModel {
    /// Load from LOCAL paths only (no network): a quantized Qwen2 `*.gguf` and
    /// its Hugging Face `tokenizer.json`. CPU device. Uses the default display
    /// name; multi-tier callers (SLM-1 vs SLM-1-MINI) should use
    /// [`CandlePerplexityModel::from_paths_named`] so the report can say WHICH
    /// model verified.
    pub fn from_paths(model_gguf: &Path, tokenizer_json: &Path) -> Result<Self, String> {
        Self::from_paths_named(model_gguf, tokenizer_json, DEFAULT_MODEL_NAME)
    }

    /// Like [`from_paths`](CandlePerplexityModel::from_paths) but with an
    /// explicit display `name` — the tier identity that surfaces in the
    /// AI-Check report's model line (e.g. the compact 1.5B vs the full 7B). The
    /// name is the ONLY difference; loading is identical (same Qwen2 loader,
    /// same shared tokenizer).
    pub fn from_paths_named(
        model_gguf: &Path,
        tokenizer_json: &Path,
        name: &str,
    ) -> Result<Self, String> {
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
            name: name.to_string(),
        })
    }
}

impl CandlePerplexityModel {
    /// REFERENCE implementation: per-token surprisal via the incremental
    /// KV-cache decode loop (one forward per token — O(n) single-token
    /// forwards, ~4.4GB of weights streamed per token, so slow). Kept
    /// permanently as the ground truth the batched path is verified against
    /// (`perplexity_probe --compare`); not used by the `PerplexityModel` trait.
    pub fn surprisals_reference(&self, tokens: &[String]) -> Vec<f32> {
        if tokens.is_empty() {
            return Vec::new();
        }

        let ids = self.token_ids(tokens);
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

    /// Piece string -> exact vocab id. A miss (shouldn't happen for real
    /// vocab pieces) falls back to BOS so length is preserved.
    fn token_ids(&self, tokens: &[String]) -> Vec<u32> {
        tokens
            .iter()
            .map(|t| self.tokenizer.token_to_id(t).unwrap_or(self.bos))
            .collect()
    }

    /// Batched teacher-forced surprisals: the window is prefilled through
    /// `forward_all()` in `PREFILL_CHUNK`-token chunks against the KV cache,
    /// and each chunk's logits are scored (log-softmax + gather) and dropped
    /// before the next chunk runs — see the const docs for why both chunkings
    /// are load-bearing on 8GB machines. `ids` must be non-empty.
    fn surprisals_batched(&self, ids: &[u32]) -> Result<Vec<f32>, String> {
        let n = ids.len();

        // Same sequence the reference loop feeds one token at a time:
        // [bos, t0, .., t_{n-2}] — logits row i is P(t_i | prefix).
        let mut input_ids = Vec::with_capacity(n);
        input_ids.push(self.bos);
        input_ids.extend_from_slice(&ids[..n - 1]);

        let mut model = self.inner.lock().expect("perplexity model mutex poisoned");
        // Fresh sequence: drop any KV state from a previous window.
        model.clear_kv_cache();

        let mut out = Vec::with_capacity(n);
        for (chunk_idx, chunk) in input_ids.chunks(PREFILL_CHUNK).enumerate() {
            let pos = chunk_idx * PREFILL_CHUNK;
            let input = Tensor::new(chunk, &self.device)
                .and_then(|t| t.reshape((1, chunk.len())))
                .map_err(|e| format!("input build at {pos}: {e}"))?;
            let logits = model
                .forward_all(&input, pos)
                .map_err(|e| format!("forward_all at {pos}: {e}"))?;
            let logits = logits
                .squeeze(0)
                .and_then(|l| l.to_dtype(candle_core::DType::F32))
                .map_err(|e| format!("logits reshape at {pos}: {e}"))?; // [chunk, vocab]

            // Logits row j of this chunk predicts ids[pos + j].
            let targets_all = &ids[pos..(pos + chunk.len()).min(n)];
            for (c, targets) in targets_all.chunks(SCORE_CHUNK).enumerate() {
                let rows = logits
                    .narrow(0, c * SCORE_CHUNK, targets.len())
                    .map_err(|e| format!("narrow chunk {pos}+{c}: {e}"))?;
                let logprobs = candle_nn::ops::log_softmax(&rows, candle_core::D::Minus1)
                    .map_err(|e| format!("log_softmax chunk {pos}+{c}: {e}"))?;
                let target_ids = Tensor::new(targets, &self.device)
                    .and_then(|t| t.reshape((targets.len(), 1)))
                    .map_err(|e| format!("targets chunk {pos}+{c}: {e}"))?;
                let picked: Vec<f32> = logprobs
                    .gather(&target_ids, 1)
                    .and_then(|g| g.squeeze(1))
                    .and_then(|g| g.to_vec1::<f32>())
                    .map_err(|e| format!("gather chunk {pos}+{c}: {e}"))?;
                out.extend(picked.into_iter().map(|lp| -lp / std::f32::consts::LN_2));
            }
        }
        Ok(out)
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

    /// Per-token surprisal in bits (`len == tokens.len()`), teacher-forced.
    /// Batched: one `forward_all()` per window. On failure this returns zeros
    /// with a warning (same shape contract as the reference loop) rather than
    /// silently falling back to the ~2.6s/token reference path.
    fn surprisals(&self, tokens: &[String]) -> Vec<f32> {
        if tokens.is_empty() {
            return Vec::new();
        }
        let ids = self.token_ids(tokens);
        match self.surprisals_batched(&ids) {
            Ok(bits) => bits,
            Err(e) => {
                tracing::warn!("perplexity: batched scoring failed: {e}");
                vec![0f32; tokens.len()]
            }
        }
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

    /// The named ctor shares the loader with `from_paths` — a bad path must
    /// still error cleanly (no panic). The name only surfaces after a
    /// successful load, which unit tests don't do (no real GGUF).
    #[test]
    fn from_paths_named_missing_model_errors_cleanly() {
        let r = CandlePerplexityModel::from_paths_named(
            Path::new("/nonexistent/model.gguf"),
            Path::new("/nonexistent/tokenizer.json"),
            "Qwen2.5-1.5B (compact, on-device)",
        );
        assert!(r.is_err());
    }
}
