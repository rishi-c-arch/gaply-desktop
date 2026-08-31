//! Citation Intelligence — the app-crate half of the AI engine.
//!
//! Plan: `docs/AI_ENGINE_PLAN.md`. gaply-core holds the SQL and the pure
//! scoring; everything here is the part that cannot live there — candle, the
//! tokenizer, and the one permitted network operation. Same seam as
//! `ReqwestFetcher` and `CandlePerplexityModel`: `cargo test -p gaply_core`
//! links neither TLS nor ML because of it.
//!
//! # R4 — nothing leaves the machine
//!
//! No telemetry, usage statistics, document content, queries, embeddings, or
//! error payloads are sent to any remote service by the AI layer. The pinned
//! model download in [`model_install`] is the sole permitted network operation,
//! it runs only from the explicit `ai_model_install` command, and it NEVER runs
//! at startup — startup may verify a hash and load an already-registered model,
//! nothing more. `startup_performs_no_network_call` in [`embeddings`] holds that
//! line.

pub mod cancel;
#[cfg(test)]
mod event_wire_tests;
pub mod device;
pub mod embeddings;
pub mod job_runner;
pub mod evidence;
pub mod generative;
pub mod model_manager;
pub mod task;
pub mod tasks;

#[cfg(test)]
mod support_flow_tests;

#[cfg(test)]
mod gen_startup_tests;
pub mod gen_install;
pub mod model_install;

#[cfg(test)]
mod needle_tests;

/* ============================ pinned constants ============================ *
 * Nothing here is interpreted at runtime. A vector's meaning is fixed by the
 * model AND by exactly these preprocessing choices, so all of them are pinned
 * together and versioned together by PREPROCESSING_VERSION. Changing ANY of
 * them requires bumping that string, after which gaply-core's
 * single-(model_id, preprocessing_version) rule refuses the old vectors rather
 * than silently comparing two representations. */

/// Registry id for the embedding model. Also the `model_id` on every vector.
pub const EMBED_MODEL_ID: &str = "bge-small-en-v1.5";

/// Output dimension. Matches `config.json`'s `hidden_size: 384`, verified
/// against the downloaded file rather than assumed.
pub const EMBED_DIM: usize = 384;

/// Prepended to QUERIES only. Verified verbatim against the model card, which
/// lists exactly this string for `bge-small-en-v1.5`.
pub const EMBED_QUERY_PREFIX: &str = "Represent this sentence for searching relevant passages: ";

/// Prepended to DOCUMENTS: nothing. The model card is explicit — "In all cases,
/// no instruction needs to be added to passages."
pub const EMBED_DOC_PREFIX: &str = "";

/// How token vectors collapse into one sentence vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pooling {
    /// Average over non-padding tokens.
    Mean,
    /// The `[CLS]` token's hidden state — what BGE was trained to produce.
    Cls,
}

/// PINNED POOLING — matches the model's published configuration.
///
/// `bge-small-en-v1.5` is a CLS-pooled model: `1_Pooling/config.json` sets
/// `pooling_mode_cls_token: true` / `pooling_mode_mean_tokens: false`, and the
/// card's reference snippet is `model_output[0][:, 0]`. This constant follows
/// that (plan §11 D6, RESOLVED): CLS is the configuration the model was trained
/// and benchmarked in, and running mean-pooled would be running off-distribution
/// on the strength of one synthetic query's 0.03 of extra margin.
///
/// CHANGING THIS REQUIRES BUMPING [`PREPROCESSING_VERSION`] IN THE SAME EDIT.
/// The two poolings produce different vector spaces; the stored
/// `preprocessing_version` is the only thing stopping vectors from one being
/// compared against the other, and gaply-core refuses a mixture rather than
/// returning confident nonsense.
pub const EMBED_POOLING: Pooling = Pooling::Cls;

/// Version of the whole preprocessing recipe above. Stored on every vector.
///
/// p1 → p2 when pooling moved from mean to CLS (§11 D6). No persistent p1
/// vectors ever existed, so nothing needed re-embedding.
pub const PREPROCESSING_VERSION: &str = "bge-v1.5-p2";

/// Fixed, conservative batch. No hardware profiling in this phase.
pub const EMBED_BATCH_SIZE: usize = 16;

/// BERT's positional limit for this model (`max_position_embeddings: 512`).
/// Longer inputs are truncated rather than erroring — a chunk is ~512 word
/// tokens, so this bites only on unusually dense text.
pub const EMBED_MAX_TOKENS: usize = 512;

/// The space every vector this build writes belongs to.
pub fn embedding_space() -> gaply_core::ai_engine::embeddings::EmbeddingSpace {
    gaply_core::ai_engine::embeddings::EmbeddingSpace::new(EMBED_MODEL_ID, PREPROCESSING_VERSION)
}

/* ============================== engine slot =============================== */

use std::sync::RwLock;

/// Holds the resident embedding engine and its honest state.
///
/// Load-once, never unloaded (see `embeddings.rs`). The lock exists for ONE
/// transition — `NotInstalled` → `Ready` after the user installs a model — not
/// for an eviction policy.
pub struct EmbeddingSlot {
    engine: RwLock<Option<embeddings::EmbeddingEngine>>,
    state: RwLock<embeddings::EngineState>,
}

impl EmbeddingSlot {
    /// Startup entry point. Verifies and loads if installed; otherwise records
    /// NotInstalled and lets the app boot. **Never downloads.**
    pub fn load_at_startup(app_data: &std::path::Path) -> Self {
        let (engine, state) = embeddings::EmbeddingEngine::load_if_installed(app_data);
        if let embeddings::EngineState::Corrupt { reason } = &state {
            tracing::warn!(%reason, "embedding model present but failed verification");
        }
        Self { engine: RwLock::new(engine), state: RwLock::new(state) }
    }

    pub fn state(&self) -> embeddings::EngineState {
        self.state.read().expect("slot poisoned").clone()
    }

    /// Re-verify and load after an install. Same no-network path as startup.
    pub fn reload(&self, app_data: &std::path::Path) -> embeddings::EngineState {
        let (engine, state) = embeddings::EmbeddingEngine::load_if_installed(app_data);
        *self.engine.write().expect("slot poisoned") = engine;
        *self.state.write().expect("slot poisoned") = state.clone();
        state
    }

    /// Run `f` against the loaded engine, or fail with a message that says
    /// which state blocked it — never a generic "unavailable".
    pub fn with<R>(
        &self,
        f: impl FnOnce(&embeddings::EmbeddingEngine) -> Result<R, gaply_core::GaplyError>,
    ) -> Result<R, gaply_core::GaplyError> {
        let guard = self.engine.read().expect("slot poisoned");
        match guard.as_ref() {
            Some(e) => f(e),
            None => Err(match self.state() {
                embeddings::EngineState::Corrupt { reason } => gaply_core::GaplyError::Validation(
                    format!("the embedding model failed verification ({reason}) — re-run the model install"),
                ),
                _ => gaply_core::GaplyError::Validation(
                    "no embedding model is installed — run the model install first. Every \
                     deterministic feature works without it."
                        .to_string(),
                ),
            }),
        }
    }
}
