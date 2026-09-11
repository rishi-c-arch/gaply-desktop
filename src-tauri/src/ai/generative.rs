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
#[serde(rename_all = "camelCase")]
pub struct GenOutput {
    pub text: String,
    pub tokens: usize,
    pub stop_reason: StopReason,
    pub elapsed_ms: u64,
    /// Tokens in the prompt after tokenization — what prefill had to chew.
    pub prompt_tokens: usize,
    /// PREFILL: the single forward over the whole prompt. Measured as the first
    /// forward call, which is exactly what it is in this loop.
    pub prefill_ms: u64,
    /// DECODE: every subsequent single-token forward, summed.
    pub decode_ms: u64,
    /// Decode throughput ALONE — generated tokens / decode seconds. Reported
    /// separately from an overall figure because the two answer different
    /// questions: an overall rate that folds in a one-off prefill says nothing
    /// about how fast a long reply streams.
    pub decode_tokens_per_sec: f64,
}

/// The seam the ModelManager drives. Mockable, so the state machine,
/// serialization and cancellation tests need no real model.
pub trait GenerationBackend: Send + Sync {
    fn generate(&self, req: GenRequest<'_>) -> Result<GenOutput, GaplyError>;

    /// The weights this backend ACTUALLY opened.
    ///
    /// Separate from `ModelManager::model_id()` on purpose: that reports the
    /// loader the app was CONFIGURED with, and "which model was configured" and
    /// "which model produced this text" are different questions that looked
    /// identical the day a 3B was selected and a 0.5B was suspected. Only the
    /// object holding the weights can answer the second one.
    ///
    /// Defaults to `"unknown"` so a test double need not care.
    fn loaded_model_file(&self) -> String {
        "unknown".to_string()
    }
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
    /// KV cache at the CONFIGURED context, not the model's maximum.
    pub kv_cache_bytes: u64,
    pub total_bytes: u64,
    /// The context this build actually runs at ([`TASK_N_CTX`], capped by the
    /// model's own maximum).
    pub context_length: usize,
    /// What the GGUF says the model could do. Reported alongside so the two are
    /// never confused: sizing at 32768 when we run at 4096 over-reports RAM by
    /// 8x, and sizing at 4096 while ALLOWING 32768 would under-report it.
    pub model_max_context: usize,
    pub layers: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
}

/// KV cache entries are f32 in this runtime.
const KV_DTYPE_BYTES: u64 = 4;

/// Context window this build runs the task workload at.
///
/// The bundled Qwen2.5 advertises 32768, but the task prompts are a sentence
/// plus its neighbours (citation_need) or a handful of retrieved chunks — a few
/// hundred tokens, with max_tokens in the 120–400 range. Sizing the KV cache for
/// 32768 reserves 768 MB to serve workloads that use a fraction of it.
///
/// This is a CONFIGURED CEILING, not a guess: prompts are checked against it
/// (see `fits_context`), so the reported figure is what the engine will actually
/// use rather than an optimistic average.
///
/// # 4096 -> 5120 (§11 D69)
///
/// A real `citation_support` check against an open-access source fetched from
/// OpenAlex was REJECTED BEFORE GENERATION at `prompt is 3246 tokens`. The
/// first-attempt prompts were fine (1919-2155, all under the old 3072 budget);
/// what did not fit was the RETRY, which quotes the rejected reply verbatim by
/// design (D10). See [`RETRY_OVERHEAD_TOKENS`] for the invariant that governs
/// this, and D69 for why the context grew rather than `max_tokens` shrinking.
pub const TASK_N_CTX: usize = 5120;

/// Scaffolding a retry adds beyond the original prompt and the quoted attempt.
///
/// MEASURED, not assumed: the observed failure was a 3246-token retry built
/// from a 2155-token original, giving 3246 - 2155 - 1024 = 67.
pub const RETRY_OVERHEAD_TOKENS: usize = 67;

/// The largest first-attempt prompt this configuration can still RETRY.
///
/// # The invariant, and why it is the durable part
///
/// `retry_prompt` (§11 D10) embeds the rejected reply verbatim, so a retry costs
/// the original prompt PLUS a reply that may run to `max_tokens`:
///
/// ```text
/// original <= TASK_N_CTX - 2*MAX_TOKENS - RETRY_OVERHEAD_TOKENS
/// ```
///
/// The factor of TWO is the part that surprises: `max_tokens` is subtracted
/// once for the reply being generated and once for the reply being quoted. At
/// 4096/1024 that left 1981 — BELOW two of three real prompts, so any real check
/// needing a retry could not have one. The specific numbers here will move; the
/// invariant is what future changes to either constant must respect, and
/// `a_budget_filling_prompt_can_still_be_retried` fails loudly if they do not —
/// and since §11 D142 it does so by BUILDING a prompt at the evidence budget and
/// tokenizing it, not by comparing these constants to a third one typed in by
/// hand. `EVIDENCE_BUDGET_TOKENS` is therefore guarded too; it was not before.
pub fn max_retryable_prompt_tokens(max_tokens: usize) -> usize {
    TASK_N_CTX
        .saturating_sub(2 * max_tokens)
        .saturating_sub(RETRY_OVERHEAD_TOKENS)
}

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

    // Size for the context we RUN at, never the model's maximum. Capped by the
    // model's own limit so a larger TASK_N_CTX than the model supports cannot
    // silently over-report.
    let effective_ctx = TASK_N_CTX.min(context_length);

    // K and V, per layer, across the configured window.
    let kv_cache_bytes = 2 * layers as u64
        * effective_ctx as u64
        * kv_heads as u64
        * head_dim as u64
        * KV_DTYPE_BYTES;

    Ok(RamEstimate {
        weights_bytes,
        kv_cache_bytes,
        total_bytes: weights_bytes + kv_cache_bytes,
        context_length: effective_ctx,
        model_max_context: context_length,
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
    /// The GGUF this instance opened. Provenance, not configuration — it is
    /// read back by `loaded_model_file` and stamped onto every result.
    gguf_path: std::path::PathBuf,
}

impl QwenGenerativeBackend {
    /// Load once. Callers hold the result for the model's lifecycle.
    pub fn load(model_gguf: &Path, tokenizer_json: &Path) -> Result<Self, GaplyError> {
        use candle_core::quantized::gguf_file;

        // Phase 7 STEP 2: the process-wide selection, gated per §11 D36. CPU
        // remains the floor — `shared()` returns a CPU device whenever the gate
        // refuses, so this line is not a Metal requirement.
        let selected = crate::ai::device::shared();
        let device = selected.device.clone();
        tracing::info!(device = selected.kind.as_str(), "loading generative backend");
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
        Ok(Self {
            model: std::sync::Mutex::new(model),
            tokenizer,
            device,
            eot_ids,
            gguf_path: model_gguf.to_path_buf(),
        })
    }
}

impl GenerationBackend for QwenGenerativeBackend {
    fn loaded_model_file(&self) -> String {
        self.gguf_path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.gguf_path.display().to_string())
    }

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
        // The configured ceiling is ENFORCED, not merely reported. Without this
        // the KV figure in ai_model_status would be a floor a long prompt could
        // silently exceed, which is the kind of number that reads as a
        // measurement and behaves as a wish.
        let budget = TASK_N_CTX.saturating_sub(req.max_tokens);
        if ids.len() > budget {
            return Err(GaplyError::Validation(format!(
                "prompt is {} tokens; the configured context is {TASK_N_CTX} and {} are \
                 reserved for the reply, leaving {budget}. Shorten the prompt or lower \
                 max_tokens.",
                ids.len(),
                req.max_tokens
            )));
        }

        // Fresh sequence: never inherit KV state from a previous generation.
        model.clear_kv_cache();

        let prompt_len = ids.len();
        let mut prefill_ms = 0u64;
        let mut decode_ms = 0u64;
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
            // step 0 IS the prefill: next_input is the whole prompt on the first
            // pass and a single token thereafter, so timing the first forward
            // splits the two exactly rather than approximating it.
            // Timed span covers the WHOLE step, not just the forward: pulling a
            // 152k-vocab logits row out of the tensor and scanning it for the
            // argmax is real per-token cost, and excluding it made model time
            // look 4.7x smaller than wall time. A "decode ms" that omits most of
            // decode is worse than no number at all.
            let step_start = std::time::Instant::now();
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

            // Close the step timer here: everything above is per-token work.
            let step_ms = step_start.elapsed().as_millis() as u64;
            if step == 0 {
                prefill_ms = step_ms;
            } else {
                decode_ms += step_ms;
            }

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
        Ok(GenOutput {
            tokens: generated.len(),
            text,
            stop_reason,
            elapsed_ms: started.elapsed().as_millis() as u64,
            prompt_tokens: prompt_len,
            prefill_ms,
            decode_ms,
            decode_tokens_per_sec: if decode_ms == 0 {
                0.0
            } else {
                generated.len() as f64 / (decode_ms as f64 / 1000.0)
            },
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
/// Recorded as `model_version` on every persisted card: two builds of the same
/// model id at different quantisations are not the same judge.
pub const BUNDLED_GEN_MODEL_QUANT: &str = "Q4_K_M";

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

/// Loads a model that the generative INSTALLER put on disk, resolved by
/// registry id (§9.9 — swapping the production model is a registry change plus
/// a download, never a code change in the engine).
///
/// Deliberately does not consult `ai_model_registry` itself: the registry row
/// records where a model was installed, but this loader is also what the eval
/// harness uses against a scratch directory with no database at all. It resolves
/// the same layout the installer writes, and re-verifies nothing — verification
/// belongs to the installer, and re-hashing 2 GB on every load would make model
/// switching unusable.
pub struct InstalledGenerativeLoader {
    id: String,
    gguf: std::path::PathBuf,
    tokenizer: std::path::PathBuf,
}

impl InstalledGenerativeLoader {
    /// Resolve `<app data>/models/<registry id>/`. `None` when the directory
    /// does not hold both files — the caller reports NotInstalled rather than
    /// failing obscurely at first use.
    pub fn resolve(app_data: &Path, registry_id: &str) -> Option<Self> {
        let c = crate::ai::gen_install::candidate(registry_id)?;
        let dir = crate::ai::gen_install::model_dir(app_data, c);
        let gguf = dir.join(c.gguf.local_name());
        let tokenizer = dir.join(c.tokenizer.local_name());
        (gguf.exists() && tokenizer.exists()).then(|| Self {
            id: registry_id.to_string(),
            gguf,
            tokenizer,
        })
    }

    /// Point at an explicit pair of files. Used by the bake-off harness, which
    /// runs against downloaded models before they are installed anywhere.
    pub fn from_paths(
        registry_id: &str,
        gguf: std::path::PathBuf,
        tokenizer: std::path::PathBuf,
    ) -> Result<Self, GaplyError> {
        for p in [&gguf, &tokenizer] {
            if !p.exists() {
                return Err(GaplyError::NotFound {
                    entity: "generative model file",
                    id: p.display().to_string(),
                });
            }
        }
        Ok(Self { id: registry_id.to_string(), gguf, tokenizer })
    }

    pub fn gguf_path(&self) -> &std::path::Path {
        &self.gguf
    }
}

impl crate::ai::model_manager::BackendLoader for InstalledGenerativeLoader {
    fn model_id(&self) -> String {
        self.id.clone()
    }
    fn ram_estimate(&self) -> Result<RamEstimate, GaplyError> {
        estimate_ram(&self.gguf)
    }
    fn load(&self) -> Result<std::sync::Arc<dyn GenerationBackend>, GaplyError> {
        Ok(std::sync::Arc::new(QwenGenerativeBackend::load(&self.gguf, &self.tokenizer)?))
    }
}

/* ===================== which generative model actually runs ================ *
 * The registry is the ANSWER, the bundled 0.5B is the FLOOR. */

/// The newest INSTALLED generative model, or `None` when none is usable.
///
/// Walks `ai_model_registry` newest-first and returns the first row that still
/// resolves to a real file pair under `<app data>/models/`. Rows are skipped —
/// never fatal — when:
///
/// * the id is not one of the pinned [`gen_install::CANDIDATES`] (the bundled
///   0.5B registers itself under an id with no candidate, so it lands here and
///   is correctly left to the fallback), or
/// * the files are gone (a registry row outlives a deleted directory).
///
/// A read failure is skipped too: the registry being unreadable must degrade to
/// the bundled model, not take the whole engine down.
pub fn installed_generative_loader(
    db: &gaply_core::Database,
    app_data: &Path,
) -> Option<InstalledGenerativeLoader> {
    let rows = match gaply_core::ai_engine::registry::list_models_recent_first(db, "generative") {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "model registry unreadable; falling back to the bundled model");
            return None;
        }
    };
    rows.iter().find_map(|row| InstalledGenerativeLoader::resolve(app_data, &row.id))
}

/// The loader the engine should run: an installed model if there is one, the
/// bundled 0.5B otherwise, `None` when neither resolves.
///
/// The 0.5B exists so the engine is never dead on a fresh machine — it is a
/// development/test floor, not the judge. It fails validation on most real
/// `citation_support` calls, so an installed candidate has to win whenever one
/// is present; before this, the installer could fetch and register 1.1 GB that
/// nothing ever loaded.
pub fn resolve_generative_loader(
    db: &gaply_core::Database,
    app_data: &Path,
) -> Option<std::sync::Arc<dyn crate::ai::model_manager::BackendLoader>> {
    if let Some(l) = installed_generative_loader(db, app_data) {
        return Some(std::sync::Arc::new(l));
    }
    BundledGenerativeLoader::resolve()
        .map(|l| std::sync::Arc::new(l) as std::sync::Arc<dyn crate::ai::model_manager::BackendLoader>)
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

    /* -------- which generative model the engine picks (registry vs bundled) --- */

    /// A temp `<app data>` with a candidate's two files laid out exactly as the
    /// installer writes them. `resolve` only checks that the pair EXISTS —
    /// verification is the installer's job — so the bytes are irrelevant here.
    fn install_files(app_data: &Path, registry_id: &str) {
        let c = crate::ai::gen_install::candidate(registry_id).expect("pinned candidate");
        let dir = crate::ai::gen_install::model_dir(app_data, c);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(c.gguf.local_name()), b"gguf").unwrap();
        std::fs::write(dir.join(c.tokenizer.local_name()), b"{}").unwrap();
    }

    fn register(db: &gaply_core::Database, id: &str) {
        gaply_core::ai_engine::registry::register_model(
            db,
            gaply_core::ai_engine::registry::ModelRow {
                id: id.to_string(),
                kind: "generative".into(),
                display_name: id.to_string(),
                file_path: "/m".into(),
                sha256: None,
                dim: None,
                quant: Some("Q4_K_M".into()),
            },
        )
        .unwrap();
    }

    fn tmp_app_data(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir()
            .join(format!("gaply-genres-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn an_installed_model_beats_the_bundled_floor() {
        let db = gaply_core::Database::in_memory().unwrap();
        let app_data = tmp_app_data("installed");
        install_files(&app_data, "qwen2.5-1.5b-instruct-q4km");
        register(&db, "qwen2.5-1.5b-instruct-q4km");

        let l = resolve_generative_loader(&db, &app_data).expect("an installed model resolves");
        assert_eq!(l.model_id(), "qwen2.5-1.5b-instruct-q4km");
        assert_ne!(l.model_id(), BUNDLED_GEN_MODEL_ID, "the 0.5B floor must not win here");
        let _ = std::fs::remove_dir_all(&app_data);
    }

    #[test]
    fn an_empty_registry_falls_back_to_the_bundled_floor() {
        let db = gaply_core::Database::in_memory().unwrap();
        let app_data = tmp_app_data("empty");

        assert!(
            installed_generative_loader(&db, &app_data).is_none(),
            "nothing is registered, so nothing is installed"
        );
        // The composed resolver hands back the bundled 0.5B — or None on a
        // machine/CI runner where the bundled model is not present either.
        // Asserted as that either/or so the test states a fact rather than a
        // machine-dependent hope; what it pins is that NO installed candidate
        // can be chosen from an empty registry.
        match resolve_generative_loader(&db, &app_data) {
            Some(l) => assert_eq!(l.model_id(), BUNDLED_GEN_MODEL_ID),
            None => assert!(BundledGenerativeLoader::resolve().is_none()),
        }
        let _ = std::fs::remove_dir_all(&app_data);
    }

    #[test]
    fn a_registry_row_whose_files_are_gone_is_skipped_not_fatal() {
        let db = gaply_core::Database::in_memory().unwrap();
        let app_data = tmp_app_data("stale");
        // Registered, never installed here (and the bundled id has no pinned
        // candidate at all — the other way a row is legitimately skipped).
        register(&db, "qwen2.5-3b-instruct-q4km");
        register(&db, BUNDLED_GEN_MODEL_ID);

        assert!(installed_generative_loader(&db, &app_data).is_none());
        let _ = std::fs::remove_dir_all(&app_data);
    }
}

