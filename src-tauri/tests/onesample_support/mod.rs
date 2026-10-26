//! Shared by the one-sample and paired t-tests' command tests (Research OS,
//! Phase 8D-2a, decision 263): an app over Tauri's mock runtime with the
//! `stats_desc_*` commands (both tests open and close their dataset with
//! them, and the summary is the other kind a wrong-kind test needs), the
//! t-test's run and status (the third kind), the twelve
//! `stats_onesample_*` and `stats_paired_*` commands, and the `workspace_*`
//! commands, so a test calls them through IPC with the webview's argument
//! names and reads the JSON the webview reads. Folders, tables and the IPC
//! call itself are `desc_support`'s, unchanged.
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
use app_lib::stats_onesample_commands as one;
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
            one::stats_onesample_validate,
            one::stats_onesample_run,
            one::stats_onesample_status,
            one::stats_onesample_result,
            one::stats_onesample_cancel,
            one::stats_onesample_export,
            one::stats_paired_validate,
            one::stats_paired_run,
            one::stats_paired_status,
            one::stats_paired_result,
            one::stats_paired_cancel,
            one::stats_paired_export,
            ws::workspace_state,
            ws::workspace_activate,
            ws::workspace_deactivate,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let n = WINDOWS.fetch_add(1, Ordering::SeqCst);
    let webview = WebviewWindowBuilder::new(&app, format!("o{n}"), Default::default())
        .build()
        .expect("webview");
    (app, webview)
}

/// A spec for `stats.analyze.one_sample_t` on the opened dataset.
pub fn ospec(
    open: &Value,
    operation: u64,
    outcome: &str,
    test_value: Option<f64>,
    missing_codes: &[f64],
) -> Value {
    json!({
        "schemaVersion": 1,
        "operationId": operation,
        "workspaceId": "statistics",
        "capabilityId": "stats.analyze.one_sample_t",
        "capabilityVersion": 1,
        "inputs": [open["inputRef"]],
        "parameters": {
            "outcome": outcome,
            "testValue": test_value,
            "missingCodes": missing_codes,
        },
        "randomSeed": null,
        "confirmationRefs": [],
    })
}

/// A spec for `stats.analyze.paired_t` on the opened dataset.
pub fn pspec(
    open: &Value,
    operation: u64,
    first: &str,
    second: &str,
    missing_codes: &[f64],
) -> Value {
    json!({
        "schemaVersion": 1,
        "operationId": operation,
        "workspaceId": "statistics",
        "capabilityId": "stats.analyze.paired_t",
        "capabilityVersion": 1,
        "inputs": [open["inputRef"]],
        "parameters": {
            "first": first,
            "second": second,
            "missingCodes": missing_codes,
        },
        "randomSeed": null,
        "confirmationRefs": [],
    })
}

/// Polls `{prefix}_status` (`stats_onesample` or `stats_paired`) until the
/// run is terminal, for at most 10 s.
pub fn until_terminal_of(webview: &WebviewWindow<MockRuntime>, prefix: &str, run_id: u64) -> Value {
    let cmd = format!("{prefix}_status");
    for _ in 0..1000 {
        let s = call(webview, &cmd, json!({ "runId": run_id })).expect("status");
        if s["terminal"] == true {
            return s;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {run_id} did not end within 10 s");
}
