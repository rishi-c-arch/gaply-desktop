use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::sync::Arc;

use gaply_core::db::ProjectStore;
use gaply_core::embed::Embedder;
use gaply_core::{AppConfig, Database};

/// Shared application state, managed by Tauri and injected into commands.
///
/// `store` is the trait-object view used by domain commands (mockable in
/// tests); `db` is the embedded database handle used by db_* admin commands
/// and RAG. In production both point at the same `Database`. `embedder`
/// turns text into 384-dim vectors for semantic search.
pub struct AppState {
    pub config: AppConfig,
    pub db: Arc<Database>,
    pub store: Arc<dyn ProjectStore>,
    pub embedder: Arc<dyn Embedder>,
    /// AI Check's cancel token — flipped `true` by `cancel_aicheck`, RESET to
    /// `false` at the start of each `run_aicheck` so a cancelled run never
    /// poisons the next one. Polled per-passage in the deep loop.
    pub aicheck_cancel: Arc<AtomicBool>,
    /// Rendered PublishReady PDFs, by `report_id`. IN MEMORY ONLY.
    ///
    /// # Why not the cache table — four measurements, three negative (§51)
    ///
    /// 1. **No blob store.** `cache_put(key, value: &str)` is text-only and no
    ///    migration declares a `BLOB` column.
    /// 2. **Weight is not the problem** — 4.5–37 KB per report, measured.
    /// 3. **No invalidation key.** `report_cache_key` is keyed on
    ///    `CACHED_REPORT_SCHEMA_VERSION`, not on the composer or renderer, so a
    ///    wording edit would serve a stale PDF for up to 30 days undetected.
    /// 4. **Persisting it crosses §4.22.** A rendered report contains
    ///    `nearby_text` quotations and similarity excerpts — manuscript prose
    ///    that lives only in the deliberately non-`Serialize` `LocalReportModel`
    ///    and appears in NO persisted artifact today.
    ///
    /// So the bytes are held for the session and never written. Bounded to
    /// [`MAX_CACHED_PDFS`] so a long session cannot grow without limit.
    pub report_pdfs: Arc<Mutex<VecDeque<(String, Vec<u8>)>>>,
    /// The app data directory — where the AI model lives. Held so commands can
    /// find it without a Tauri handle.
    pub app_data_dir: std::path::PathBuf,
    /// The resident embedding engine (Citation Intelligence, Phase 2). Loaded
    /// at startup IF installed and verified; otherwise NotInstalled and the app
    /// boots normally with every deterministic feature intact.
    pub ai_embed: Arc<crate::ai::EmbeddingSlot>,
    /// Cancel token for the model install (AI Check's precedent).
    pub ai_install_cancel: Arc<AtomicBool>,
    /// Cancel token for an embedding pass.
    pub ai_embed_cancel: Arc<AtomicBool>,
    /// GENERATIVE model lifecycle (Phase 3). Constructed at startup but NOT
    /// loaded — the weights are pulled in lazily by the first generation.
    pub ai_gen: Arc<crate::ai::model_manager::ModelManager>,
    /// Cancel token for a generation.
    pub ai_gen_cancel: Arc<AtomicBool>,
}

/// How many rendered reports to keep in memory. A user exports the run they
/// just did; older ones are re-rendered by re-running, which is the honest cost
/// of not persisting them.
pub const MAX_CACHED_PDFS: usize = 4;

impl AppState {
    pub fn new(
        config: AppConfig,
        db: Arc<Database>,
        store: Arc<dyn ProjectStore>,
        embedder: Arc<dyn Embedder>,
        app_data_dir: std::path::PathBuf,
    ) -> Self {
        let ai_embed = Arc::new(crate::ai::EmbeddingSlot::load_at_startup(&app_data_dir));
        tracing::info!(state = ?ai_embed.state(), "embedding engine");
        // The generative manager is CONSTRUCTED here, never loaded: resolving a
        // path and reading nothing is not a load. The first generation request
        // is the only thing that maps weights.
        let gen_manager = match crate::ai::generative::BundledGenerativeLoader::resolve() {
            Some(loader) => crate::ai::model_manager::ModelManager::new(Arc::new(loader)),
            None => {
                // No generative model resolves. Report it honestly; every
                // deterministic feature still works.
                let m = crate::ai::model_manager::ModelManager::new(Arc::new(
                    crate::ai::model_manager::NullLoader,
                ));
                m.set_not_installed();
                m
            }
        };
        tracing::info!(state = ?gen_manager.state(), "generative model manager");
        Self {
            config,
            db,
            store,
            embedder,
            aicheck_cancel: Arc::new(AtomicBool::new(false)),
            report_pdfs: Arc::new(Mutex::new(VecDeque::new())),
            app_data_dir,
            ai_embed,
            ai_install_cancel: Arc::new(AtomicBool::new(false)),
            ai_embed_cancel: Arc::new(AtomicBool::new(false)),
            ai_gen: Arc::new(gen_manager),
            ai_gen_cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}
