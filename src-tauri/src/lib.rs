pub mod commands;
pub mod logging;
pub mod state;

use std::sync::Arc;

use tauri::Manager;

use gaply_core::{AppConfig, Database};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let config = AppConfig::new(data_dir.join("gaply.db"));
            config.validate()?;
            logging::init(&config.log_filter);

            let db = Arc::new(Database::open(&config.db_path)?);
            let embedder = Arc::new(gaply_core::embed::HashEmbedder);
            app.manage(AppState::new(config, db.clone(), db, embedder));

            tracing::info!(version = env!("CARGO_PKG_VERSION"), "gaply desktop started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_project,
            commands::list_projects,
            commands::get_project,
            commands::health_check,
            commands::db_init,
            commands::db_migrate,
            commands::db_health,
            commands::rag_search,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "fatal: tauri runtime failed");
            panic!("error while running tauri application: {e}");
        });
}
