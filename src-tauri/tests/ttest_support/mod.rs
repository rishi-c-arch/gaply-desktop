//! Shared by the t-test's command tests (Research OS, Phase 4B-2): an app
//! over Tauri's mock runtime with the `stats_desc_*` commands (the t-test
//! opens and closes its dataset with them), the six `stats_ttest_*`
//! commands, `capability_list` and the `workspace_*` commands, so a test calls
//! them through IPC with the webview's argument names and reads the JSON the
//! webview reads. Folders, tables and the IPC call itself are
//! `desc_support`'s, unchanged.
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
            desc::stats_desc_validate,
            desc::stats_desc_run,
            desc::stats_desc_status,
            desc::stats_desc_result,
            desc::stats_desc_cancel,
            desc::stats_desc_export,
            desc::stats_desc_close,
            ttest::stats_ttest_validate,
            ttest::stats_ttest_run,
            ttest::stats_ttest_status,
            ttest::stats_ttest_result,
            ttest::stats_ttest_cancel,
            ttest::stats_ttest_export,
            ttest::capability_list,
            ws::workspace_list,
            ws::workspace_state,
            ws::workspace_activate,
            ws::workspace_deactivate,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let n = WINDOWS.fetch_add(1, Ordering::SeqCst);
    let webview = WebviewWindowBuilder::new(&app, format!("t{n}"), Default::default())
        .build()
        .expect("webview");
    (app, webview)
}

/// A spec for `stats.analyze.independent_t` on the opened dataset: the
/// input as the open view gave it, columns by alias, levels by position.
pub fn tspec(
    open: &Value,
    operation: u64,
    outcome: &str,
    group: &str,
    levels: Option<[u32; 2]>,
    variant: Option<&str>,
    missing_codes: &[f64],
) -> Value {
    json!({
        "schemaVersion": 1,
        "operationId": operation,
        "workspaceId": "statistics",
        "capabilityId": "stats.analyze.independent_t",
        "capabilityVersion": 1,
        "inputs": [open["inputRef"]],
        "parameters": {
            "outcome": outcome,
            "group": group,
            "levels": levels,
            "variant": variant,
            "missingCodes": missing_codes,
        },
        "randomSeed": null,
        "confirmationRefs": [],
    })
}

/// Polls `stats_ttest_status` until the run is terminal, for at most 10 s.
pub fn until_terminal_t(webview: &WebviewWindow<MockRuntime>, run_id: u64) -> Value {
    for _ in 0..1000 {
        let s = call(webview, "stats_ttest_status", json!({ "runId": run_id })).expect("status");
        if s["terminal"] == true {
            return s;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {run_id} did not end within 10 s");
}
