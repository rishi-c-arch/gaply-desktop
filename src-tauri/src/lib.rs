pub mod ai;
mod aicheck;
pub mod citation_resolver;
pub mod commands;
pub mod escalation;
pub mod guidelines;
pub mod http_fetcher;
pub mod harness_log;
pub mod release_gate;
mod logging;
pub mod models;
pub mod journal_registry;
pub mod journal_site_summary;
pub mod journal_verify;
pub mod paper_corpus;
pub mod pipeline;
pub mod report_build;
pub mod reviewer_synthesis;
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
    // where a deep link spawns a SECOND process with the URL in argv. We focus
    // the running window AND forward any gaply:// URL to the frontend, which
    // completes the OAuth code exchange (see useDeepLinkAuth). macOS delivers
    // deep links natively via onOpenUrl and never hits this path.
    //
    // NOTE (honest): verified on macOS (which doesn't use this branch) + tests;
    // the Windows/Linux warm-start forwarding is written to the documented
    // single-instance argv pattern but is pending a hand-test on real hardware.
    // It provably replaces the prior code, which discarded argv entirely.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            use tauri::Emitter;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            if let Some(url) = argv.iter().find(|a| a.starts_with("gaply://")) {
                let _ = app.emit("deep-link-url", url.clone());
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
            // Durable sink for the Box 4 comparison record (harness_log). Without
            // this the record reaches stdout only and nothing accumulates.
            harness_log::set_dir(data_dir.clone());

            // Point model resolution at the BUNDLED models (Set 1) as the
            // last-resort source: <resource_dir>/models/{stage1-lm,slm1-adapter}.
            // A stranger with no ~/gaply-models falls through to the packaged 0.5B.
            if let Ok(res) = app.path().resource_dir() {
                crate::models::set_bundled_models_dir(res.join("models"));
            }

            let db = Arc::new(Database::open(&config.db_path)?);
            let embedder = Arc::new(gaply_core::embed::HashEmbedder);
            app.manage(AppState::new(config, db.clone(), db, embedder, data_dir.clone()));

            tracing::info!(version = env!("CARGO_PKG_VERSION"), "gaply desktop started");
            crate::models::log_model_resolution(); // proof the bundled model is reachable
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Reserved backend infra — no frontend `invoke` caller today, but kept
            // deliberately: each is exercised end-to-end through real Tauri IPC by
            // `tests/commands_test.rs` (project CRUD roundtrip, db lifecycle/health,
            // rag provenance search, extract-and-store). They back the project-scoped
            // notes concept, DB diagnostics, the RAG agent, and standalone extraction,
            // and are the seam a future settings/diagnostics UI plugs into. Not dead.
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
            commands::cancel_aicheck,
            commands::aicheck_memory_status,
            commands::check_plagiarism,
            commands::check_plagiarism_exact,
            commands::add_to_plagiarism_library,
            commands::list_plagiarism_library,
            commands::remove_from_plagiarism_library,
            // Reserved keychain-write seam — no frontend caller yet. The READ side
            // (`gaply_core::secrets::get_secret`) is live: `app_check.rs` loads the
            // attestation signing key through it. These write/presence/delete commands
            // are what an API-key-entry UI on the (currently stubbed) cloud-auth path
            // will call. Kept intentionally.
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
            commands::ai_model_install,
            commands::ai_model_install_cancel,
            commands::ai_model_status,
            commands::ai_embed_document,
            commands::ai_embed_cancel,
            commands::ai_semantic_search,
            commands::ai_index_document,
            commands::ai_index_status,
            commands::note_create,
            commands::note_update,
            commands::note_get,
            commands::note_list,
            commands::note_search,
            commands::note_set_tags,
            commands::note_delete,
            commands::note_paper_fulltext,
            commands::log_auth_callback,
            commands::log_auth_probe,
            commands::log_storage_error,
            commands::read_import_file,
            commands::export_report,
            commands::export_publishready_pdf,
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
