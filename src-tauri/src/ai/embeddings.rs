//! The embedding engine: candle BERT + tokenizers, resident once loaded.
//!
//! Implements the production side of `gaply_core::embed::Embedder` from the app
//! crate, exactly as `CandlePerplexityModel` implements `PerplexityModel` —
//! gaply-core stays sync, ML-free and network-free.
//!
//! # Lifetime
//!
//! Resident from load, never unloaded. The weights are ~130 MB, which is small
//! enough that unloading buys nothing and an idle timer would only add a stall
//! to the next search. That is the OPPOSITE of the generative model's policy
//! (plan §6), and deliberately so — the two differ by two orders of magnitude
//! in size.
//!
//! # Startup performs NO network call
//!
//! [`EmbeddingEngine::load_if_installed`] verifies sha256 and loads, or reports
//! [`EngineState::NotInstalled`] / [`EngineState::Corrupt`]. It never downloads.
//! Acquisition happens only in `ai_model_install`, from explicit user action.
//! The app boots normally in every state — a missing model degrades semantic
//! search, and nothing else.

use std::path::Path;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use gaply_core::GaplyError;
use serde::Serialize;
use tokenizers::Tokenizer;

use crate::ai::{
    model_install, Pooling, EMBED_BATCH_SIZE, EMBED_DIM, EMBED_DOC_PREFIX, EMBED_MAX_TOKENS,
    EMBED_MODEL_ID, EMBED_POOLING, EMBED_QUERY_PREFIX, PREPROCESSING_VERSION,
};

/// What the engine can honestly say about itself.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum EngineState {
    /// No model installed. Not an error — the app runs, semantic search does not.
    NotInstalled,
    /// Files present but a hash did not match. Distinct from NotInstalled on
    /// purpose: "never installed" and "installed and now wrong" call for
    /// different user actions.
    Corrupt { reason: String },
    Ready { model_id: String, dim: usize, preprocessing_version: String },
}

pub struct EmbeddingEngine {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl EmbeddingEngine {
    /// Load if a verified install exists. **No network, ever.**
    pub fn load_if_installed(app_data: &Path) -> (Option<Self>, EngineState) {
        let dir = model_install::model_dir(app_data);
        if !dir.join("model.safetensors").is_file() {
            return (None, EngineState::NotInstalled);
        }
        if let Err(e) = model_install::verify_install(&dir) {
            return (None, EngineState::Corrupt { reason: e.to_string() });
        }
        match Self::load_verified(&dir) {
            Ok(engine) => (
                Some(engine),
                EngineState::Ready {
                    model_id: EMBED_MODEL_ID.to_string(),
                    dim: EMBED_DIM,
                    preprocessing_version: PREPROCESSING_VERSION.to_string(),
                },
            ),
            Err(e) => (None, EngineState::Corrupt { reason: e.to_string() }),
        }
    }

    /// Load from a directory whose hashes have ALREADY been verified.
    pub fn load_verified(dir: &Path) -> Result<Self, GaplyError> {
        let device = Device::Cpu; // CPU-native, matching the rest of the app
        let config: Config = serde_json::from_slice(&std::fs::read(dir.join("config.json"))?)
            .map_err(|e| GaplyError::Internal(format!("model config unreadable: {e}")))?;
        if config.hidden_size != EMBED_DIM {
            return Err(GaplyError::Validation(format!(
                "model reports hidden_size {} but this build is pinned to {EMBED_DIM}",
                config.hidden_size
            )));
        }
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| GaplyError::Internal(format!("tokenizer load failed: {e}")))?;
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[dir.join("model.safetensors")], DType::F32, &device)
                .map_err(|e| GaplyError::Internal(format!("weights load failed: {e}")))?
        };
        let model = BertModel::load(vb, &config)
            .map_err(|e| GaplyError::Internal(format!("bert load failed: {e}")))?;
        Ok(Self { model, tokenizer, device })
    }

    /// Embed documents (no prefix) — the passage side of the asymmetry.
    pub fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, GaplyError> {
        self.embed_prefixed(texts, EMBED_DOC_PREFIX)
    }

    /// Embed a query (query prefix applied) — the other side of the asymmetry.
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>, GaplyError> {
        Ok(self
            .embed_prefixed(std::slice::from_ref(&text.to_string()), EMBED_QUERY_PREFIX)?
            .into_iter()
            .next()
            .expect("one in, one out"))
    }

    /// Batch entry point. Batches are fixed-size and conservative — no hardware
    /// profiling in this phase.
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, GaplyError> {
        self.embed_documents(texts)
    }

    fn embed_prefixed(&self, texts: &[String], prefix: &str) -> Result<Vec<Vec<f32>>, GaplyError> {
        let mut out = Vec::with_capacity(texts.len());
        for batch in texts.chunks(EMBED_BATCH_SIZE) {
            let prepared: Vec<String> = batch.iter().map(|t| format!("{prefix}{t}")).collect();
            out.extend(self.forward_batch(&prepared)?);
        }
        Ok(out)
    }

    fn forward_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, GaplyError> {
        let internal = |e: String| GaplyError::Internal(e);
        let mut encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| internal(format!("tokenize failed: {e}")))?;

        // BERT cannot attend past its positional table; truncate rather than
        // erroring, and truncate the MASK identically so padding logic holds.
        for e in encodings.iter_mut() {
            e.truncate(EMBED_MAX_TOKENS, 0, tokenizers::TruncationDirection::Right);
        }
        let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0).max(1);

        let mut ids = Vec::with_capacity(texts.len() * max_len);
        let mut mask = Vec::with_capacity(texts.len() * max_len);
        for e in &encodings {
            let f = e.get_ids();
            let m = e.get_attention_mask();
            ids.extend_from_slice(f);
            mask.extend_from_slice(m);
            for _ in f.len()..max_len {
                ids.push(0);
                mask.push(0);
            }
        }
        let shape = (texts.len(), max_len);
        let ids = Tensor::from_vec(ids, shape, &self.device)
            .map_err(|e| internal(format!("input tensor: {e}")))?;
        let mask_t = Tensor::from_vec(mask.clone(), shape, &self.device)
            .map_err(|e| internal(format!("mask tensor: {e}")))?;
        let type_ids = ids.zeros_like().map_err(|e| internal(format!("type ids: {e}")))?;

        let hidden = self
            .model
            .forward(&ids, &type_ids, Some(&mask_t))
            .map_err(|e| internal(format!("bert forward: {e}")))?;

        let pooled = match EMBED_POOLING {
            // Mean over NON-PADDING tokens only. Averaging the padding in would
            // make a vector depend on its batch-mates' lengths, which is a
            // correctness bug, not an approximation.
            Pooling::Mean => {
                let m = mask_t
                    .to_dtype(DType::F32)
                    .map_err(|e| internal(format!("mask dtype: {e}")))?
                    .unsqueeze(2)
                    .map_err(|e| internal(format!("mask unsqueeze: {e}")))?;
                let masked = hidden
                    .broadcast_mul(&m)
                    .map_err(|e| internal(format!("mask mul: {e}")))?;
                let summed = masked.sum(1).map_err(|e| internal(format!("sum: {e}")))?;
                let counts = m.sum(1).map_err(|e| internal(format!("count: {e}")))?;
                summed
                    .broadcast_div(&counts)
                    .map_err(|e| internal(format!("mean div: {e}")))?
            }
            // The [CLS] token, position 0 — what BGE publishes (plan §11 D6).
            Pooling::Cls => hidden
                .i((.., 0, ..))
                .map_err(|e| internal(format!("cls slice: {e}")))?,
        };

        let mut vectors: Vec<Vec<f32>> =
            pooled.to_vec2().map_err(|e| internal(format!("to_vec2: {e}")))?;
        for v in vectors.iter_mut() {
            gaply_core::ai_engine::embeddings::l2_normalize(v);
        }
        Ok(vectors)
    }
}

// `i((.., 0, ..))` needs the indexing trait in scope.
use candle_core::IndexOp;

#[cfg(test)]
mod tests {
    use super::*;

    /// The startup path must not touch the network. This asserts it
    /// STRUCTURALLY rather than by observation: `load_if_installed` is the only
    /// startup entry point, and on a directory with no model it returns
    /// NotInstalled without constructing an HTTP client. The download lives
    /// behind `model_install::install`, which nothing here calls.
    #[test]
    fn startup_performs_no_network_call() {
        let dir = std::env::temp_dir().join(format!("gaply-startup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (engine, state) = EmbeddingEngine::load_if_installed(&dir);
        assert!(engine.is_none());
        assert_eq!(state, EngineState::NotInstalled);

        // Source-level guard: the startup module must not reference the
        // downloader or any URL. If a future edit reaches for the network at
        // boot, this fails.
        let src = include_str!("embeddings.rs");
        let body = src.split("mod tests").next().unwrap();
        for forbidden in ["reqwest", "TcpStream", "http://", "model_install::install"] {
            assert!(
                !body.contains(forbidden),
                "the startup path references {forbidden:?} — startup must never acquire a model"
            );
        }
        // The downloader is reachable ONLY from the install command. If it ever
        // gains a caller inside the engine, the line above fails; this asserts
        // the other half — that the engine's own entry points are verify-only.
        assert!(body.contains("model_install::verify_install"), "startup must still verify");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_install_is_reported_as_corrupt_not_missing() {
        let dir = std::env::temp_dir().join(format!("gaply-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model_dir = model_install::model_dir(&dir);
        std::fs::create_dir_all(&model_dir).unwrap();
        for f in model_install::PINNED_FILES {
            std::fs::write(model_dir.join(f.name), b"wrong bytes").unwrap();
        }
        let (engine, state) = EmbeddingEngine::load_if_installed(&dir);
        assert!(engine.is_none(), "a corrupt model must not load");
        match state {
            EngineState::Corrupt { reason } => {
                assert!(reason.contains("failed verification"), "unhelpful reason: {reason}")
            }
            other => panic!("expected Corrupt, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn not_installed_and_corrupt_are_distinct_states() {
        // The distinction matters: one says "install it", the other says
        // "re-install it". Collapsing them would misdirect the user.
        assert_ne!(
            EngineState::NotInstalled,
            EngineState::Corrupt { reason: "x".into() }
        );
    }
}
