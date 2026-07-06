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
}

impl AppState {
    pub fn new(
        config: AppConfig,
        db: Arc<Database>,
        store: Arc<dyn ProjectStore>,
        embedder: Arc<dyn Embedder>,
    ) -> Self {
        Self { config, db, store, embedder }
    }
}
