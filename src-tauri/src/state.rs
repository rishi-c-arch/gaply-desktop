use std::sync::Arc;

use gaply_core::db::ProjectStore;
use gaply_core::{AppConfig, Database};

/// Shared application state, managed by Tauri and injected into commands.
///
/// `store` is the trait-object view used by domain commands (mockable in
/// tests); `db` is the embedded database handle used by db_* admin commands.
/// In production both point at the same `Database`.
pub struct AppState {
    pub config: AppConfig,
    pub db: Arc<Database>,
    pub store: Arc<dyn ProjectStore>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Arc<Database>, store: Arc<dyn ProjectStore>) -> Self {
        Self { config, db, store }
    }
}
