//! Shared by the confirmatory factor analysis's command tests (Research OS,
//! Phase 9C-1, decision 749), generated from `factor_support`: an app over
//! Tauri's mock runtime with the `stats_desc_*` commands (the method opens and
//! closes its dataset with them, and the summary is another kind a wrong-kind
//! test needs), the ANOVA's, the rank tests', the related rank tests', the
//! linear regression's and the factor analysis's status, result, cancel and
//! export (five more kinds), `stats_prep_make` (a score, for the refusals only a
//! prepared table can reach), the six `stats_cfa_*` commands, and the
//! `workspace_*` commands, so a test
//! calls them through IPC with the webview's argument names and reads the JSON
//! the webview reads. Folders, tables and the IPC call itself are
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
use app_lib::stats_anova_commands as anova;
use app_lib::stats_desc_commands as desc;
use app_lib::stats_cfa_commands as cfa;
use app_lib::stats_factor_commands as factor;
use app_lib::stats_linear_regression_commands as regression;
use app_lib::stats_prep_commands as prep;
use app_lib::stats_ranks_commands as ranks;
use app_lib::stats_related_ranks_commands as related;
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
    call(&webview, "workspace_activate", json!({ "id": "statistics" }))
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
            desc::stats_desc_forget_run,
            prep::stats_prep_make,
            anova::stats_anova_status,
            anova::stats_anova_result,
            anova::stats_anova_cancel,
            anova::stats_anova_export,
            ranks::stats_ranks_status,
            ranks::stats_ranks_result,
            ranks::stats_ranks_cancel,
            ranks::stats_ranks_export,
            related::stats_related_ranks_status,
            related::stats_related_ranks_result,
            related::stats_related_ranks_cancel,
            related::stats_related_ranks_export,
            regression::stats_linear_regression_status,
            regression::stats_linear_regression_result,
            regression::stats_linear_regression_cancel,
            regression::stats_linear_regression_export,
            factor::stats_factor_status,
            factor::stats_factor_result,
            factor::stats_factor_cancel,
            factor::stats_factor_export,
            cfa::stats_cfa_validate,
            cfa::stats_cfa_run,
            cfa::stats_cfa_status,
            cfa::stats_cfa_result,
            cfa::stats_cfa_cancel,
            cfa::stats_cfa_export,
            ws::workspace_state,
            ws::workspace_activate,
            ws::workspace_deactivate,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let n = WINDOWS.fetch_add(1, Ordering::SeqCst);
    let webview = WebviewWindowBuilder::new(&app, format!("l{n}"), Default::default())
        .build()
        .expect("webview");
    (app, webview)
}

/// A spec for `stats.analyze.cfa` on the opened dataset, its parameters as a
/// screen would send them: each factor's columns by alias.
pub fn cspec(open: &Value, operation: u64, indicators: &[&[&str]], correlated: bool, missing_codes: &[f64]) -> Value {
    json!({
        "schemaVersion": 1,
        "operationId": operation,
        "workspaceId": "statistics",
        "capabilityId": "stats.analyze.cfa",
        "capabilityVersion": 1,
        "inputs": [open["inputRef"]],
        "parameters": {
            "indicators": indicators,
            "correlatedFactors": correlated,
            "missingCodes": missing_codes,
        },
        "randomSeed": null,
        "confirmationRefs": [],
    })
}

/// Polls `stats_cfa_status` until the run is terminal, for at most 30 s.
pub fn until_terminal_cfa(webview: &WebviewWindow<MockRuntime>, run_id: u64) -> Value {
    for _ in 0..3000 {
        let s = call(webview, "stats_cfa_status", json!({ "runId": run_id })).expect("status");
        if s["terminal"] == true {
            return s;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {run_id} did not end within 30 s");
}
