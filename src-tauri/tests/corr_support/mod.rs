//! Shared by the correlations' command tests (Research OS, Phase 8H-2,
//! decision 356): an app over Tauri's mock runtime with the `stats_desc_*`
//! commands (the correlations open and close their dataset with them, and
//! the summary is another kind a wrong-kind test needs), the t-test's and the
//! ANOVA's run and status (two more kinds), the six `stats_corr_*` commands,
//! and the `workspace_*` commands, so a test calls them through IPC with the
//! webview's argument names and reads the JSON the webview reads. Folders,
//! tables and the IPC call itself are `desc_support`'s, unchanged.
#![allow(dead_code)]

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{WebviewWindow, WebviewWindowBuilder};

use app_lib::runs::Runs;
use app_lib::state::AppState;
use app_lib::stats_anova_commands as anova;
use app_lib::stats_corr_commands as corr;
use app_lib::stats_desc_commands as desc;
use app_lib::stats_ttest_commands as ttest;
use app_lib::workspace::{self as ws, HealthCheck, RunsFactory, WorkspaceSession};
use gaply_core::{AppConfig, Database};

use crate::desc_support::call;

static WINDOWS: AtomicUsize = AtomicUsize::new(0);

/// An app over the mock runtime, its state built by `AppState::new` on
/// `app_data` and `db`, with `runs` as the Statistics run manager when given,
/// and the Statistics workspace activated through `workspace_activate`.
pub fn app(
    app_data: &Path,
    db: Arc<Database>,
    runs: Option<Arc<Runs>>,
) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let factory: RunsFactory =
        Arc::new(move || runs.clone().unwrap_or_else(|| Arc::new(Runs::default())));
    let (app, webview) = app_with(app_data, db, factory, Arc::new(ws::readiness));
    call(
        &webview,
        "workspace_activate",
        json!({ "id": "statistics" }),
    )
    .expect("Statistics activates");
    (app, webview)
}

/// [`app`] with no workspace active.
pub fn app_with(
    app_data: &Path,
    db: Arc<Database>,
    runs: RunsFactory,
    health: HealthCheck,
) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let mut state = AppState::new(
        AppConfig::new(app_data.join("gaply.db").to_string_lossy().as_ref()),
        db.clone(),
        db,
        Arc::new(gaply_core::embed::HashEmbedder),
        app_data.to_path_buf(),
    );
    state.workspace = Arc::new(WorkspaceSession::with_hooks(health, runs));
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            desc::stats_desc_open,
            desc::stats_desc_run,
            desc::stats_desc_status,
            desc::stats_desc_result,
            desc::stats_desc_cancel,
            desc::stats_desc_export,
            desc::stats_desc_close,
            ttest::stats_ttest_run,
            ttest::stats_ttest_status,
            ttest::stats_ttest_result,
            ttest::stats_ttest_cancel,
            ttest::stats_ttest_export,
            anova::stats_anova_run,
            anova::stats_anova_status,
            anova::stats_anova_result,
            anova::stats_anova_cancel,
            anova::stats_anova_export,
            corr::stats_corr_validate,
            corr::stats_corr_run,
            corr::stats_corr_status,
            corr::stats_corr_result,
            corr::stats_corr_cancel,
            corr::stats_corr_export,
            ws::workspace_state,
            ws::workspace_activate,
            ws::workspace_deactivate,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let n = WINDOWS.fetch_add(1, Ordering::SeqCst);
    let webview = WebviewWindowBuilder::new(&app, format!("r{n}"), Default::default())
        .build()
        .expect("webview");
    (app, webview)
}

/// A spec for `stats.analyze.correlations` on the opened dataset, its
/// parameters as the screen sends them: variables in column order,
/// coefficients in their order, codes ascending (decision 338).
pub fn cspec(
    open: &Value,
    operation: u64,
    variables: &[&str],
    coefficients: &[&str],
    missing: &str,
    missing_codes: &[f64],
) -> Value {
    json!({
        "schemaVersion": 1,
        "operationId": operation,
        "workspaceId": "statistics",
        "capabilityId": "stats.analyze.correlations",
        "capabilityVersion": 1,
        "inputs": [open["inputRef"]],
        "parameters": {
            "variables": variables,
            "coefficients": coefficients,
            "missing": missing,
            "missingCodes": missing_codes,
        },
        "randomSeed": null,
        "confirmationRefs": [],
    })
}

/// Polls `stats_corr_status` until the run is terminal, for at most 30 s.
pub fn until_terminal_corr(webview: &WebviewWindow<MockRuntime>, run_id: u64) -> Value {
    for _ in 0..3000 {
        let s = call(webview, "stats_corr_status", json!({ "runId": run_id })).expect("status");
        if s["terminal"] == true {
            return s;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {run_id} did not end within 30 s");
}
