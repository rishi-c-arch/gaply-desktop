pub mod aicheck;
pub mod citation_resolver;
pub mod commands;
pub mod guidelines;
pub mod http_fetcher;
pub mod logging;
pub mod models;
pub mod journal_registry;
pub mod journal_site_summary;
pub mod journal_verify;
pub mod paper_corpus;
pub mod pipeline;
pub mod state;
pub mod supplementary;

use std::sync::Arc;

use tauri::Manager;

use gaply_core::{AppConfig, Database};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::install_panic_hook();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // single-instance MUST be registered first, and only on Windows/Linux,
    // where the deep link spawns a second process we forward to the running app.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
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
            commands::extract_manuscript,
            commands::validate_manuscript,
            commands::detect_ai,
            commands::run_aicheck,
            commands::check_plagiarism,
            commands::check_plagiarism_exact,
            commands::add_to_plagiarism_library,
            commands::list_plagiarism_library,
            commands::remove_from_plagiarism_library,
            commands::store_secret,
            commands::has_secret,
            commands::delete_secret,
            commands::verify_reference,
            commands::resolve_citation_metadata,
            commands::citation_lib_upsert,
            commands::citation_lib_list,
            commands::citation_lib_search,
            commands::citation_lib_set_tags,
            commands::citation_lib_delete,
            commands::citation_lib_set_sync_status,
            commands::note_create,
            commands::note_update,
            commands::note_get,
            commands::note_list,
            commands::note_search,
            commands::note_set_tags,
            commands::note_delete,
            commands::note_paper_fulltext,
            commands::get_report,
            commands::sample_manuscript_path,
            commands::ingest_guidelines,
            commands::run_publishready,
            commands::run_copilot_chat,
            commands::run_stats_preview,
            commands::run_stats_verify,
            commands::run_stats_chat,
            commands::build_gapfinder_corpus,
            commands::run_gap_finder,
            commands::run_gapfinder_qa,
            commands::run_gapfinder_draft,
            commands::verify_journal_registry,
            commands::verify_journal_full,
            commands::run_gapfinder_fit,
            pipeline::run_full_analysis,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "fatal: tauri runtime failed");
            panic!("error while running tauri application: {e}");
        });
}
