//! Shared by the verifier's workspace tests (Research OS, Phase 4A): an app
//! over Tauri's mock runtime with the verifier's commands and the four
//! `workspace_*` commands, its AI context on an in-memory key store and a
//! transport that is never asked; and every operation and exempt command
//! with arguments that reach its body.
#![allow(dead_code)]

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{WebviewWindow, WebviewWindowBuilder};

use app_lib::runs::Runs;
use app_lib::state::AppState;
use app_lib::stats_ai::{MemoryCredentialStore, StatsAi, TransportFactory};
use app_lib::workspace::{self as ws, HealthCheck, RunsFactory, WorkspaceSession};
use app_lib::{stats_consistency, stats_l1_commands as l1, stats_legacy};
use gaply_core::stats_assist::transport::HelperTransport;
use gaply_core::stats_verdict::{
    AnalysisSpec, ColumnRoles, ReportedStatistic, TestKind, Tolerance,
};
use gaply_core::{AppConfig, Database};

use crate::desc_support::{call, TempDir};

pub struct World {
    pub dir: TempDir,
    pub _app: tauri::App<MockRuntime>,
    pub w: WebviewWindow<MockRuntime>,
}

pub fn world(health: HealthCheck) -> World {
    let dir = TempDir::new("verifier-ws");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let mut state = AppState::new(
        AppConfig::new(app_data.join("gaply.db").to_string_lossy().as_ref()),
        db.clone(),
        db,
        Arc::new(gaply_core::embed::HashEmbedder),
        app_data.clone(),
    );
    let runs: RunsFactory = Arc::new(|| Arc::new(Runs::default()));
    state.workspace = Arc::new(WorkspaceSession::with_hooks(health, runs));
    let never: TransportFactory =
        Arc::new(|_| -> Arc<dyn HelperTransport> { panic!("nothing is sent in this test") });
    state.stats_ai = Arc::new(StatsAi::for_tests(
        Arc::new(MemoryCredentialStore::default()),
        never,
        "http://127.0.0.1:9",
    ));
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            stats_consistency::run_stats_consistency,
            stats_legacy::run_stats_preview,
            stats_legacy::run_stats_verify,
            l1::stats_l1_open,
            l1::stats_l1_edit_claim,
            l1::stats_l1_add_claim,
            l1::stats_l1_proposals,
            l1::stats_l1_helper_preview,
            l1::stats_l1_helper_send,
            l1::stats_l1_helper_revoke,
            l1::stats_l1_helper_cancel,
            l1::stats_l1_confirm_binding,
            l1::stats_l1_draft_proposal,
            l1::stats_l1_compute,
            l1::stats_l1_close,
            l1::stats_ai_usage,
            l1::stats_ai_setting_get,
            l1::stats_ai_setting_set,
            l1::stats_ai_key_status,
            l1::stats_ai_key_save,
            l1::stats_ai_key_test,
            l1::stats_ai_key_remove,
            ws::workspace_list,
            ws::workspace_state,
            ws::workspace_activate,
            ws::workspace_deactivate,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let w = WebviewWindowBuilder::new(&app, "verifier", Default::default())
        .build()
        .expect("webview");
    World { dir, _app: app, w }
}

pub fn files(w: &World) -> (String, String) {
    let ms = w.dir.write(
        "ms.txt",
        b"A Title\n\nResults\nScores rose, t(4) = 1.20, p = .29.\n",
    );
    let data = w.dir.write(
        "data.csv",
        b"score,group\n1.5,a\n2.5,b\n3.5,a\n4.5,b\n5.5,a\n6.5,b\n",
    );
    (
        ms.to_string_lossy().into_owned(),
        data.to_string_lossy().into_owned(),
    )
}

pub fn claim() -> Value {
    json!({ "family": "t", "statistic": "1.20", "df": ["4"], "n": null, "p": ".29", "pRelation": "=", "sentence": null })
}

pub fn edits() -> Value {
    json!({ "methodId": "independent_t", "roles": [], "levels": [], "filters": [], "variantTicked": null, "olsOverallF": null, "olsPredictor": null })
}

/// Every operation command, with arguments that reach its body.
pub fn operation_calls(w: &World) -> Vec<(&'static str, Result<Value, Value>)> {
    let (ms, data) = files(w);
    operation_calls_on(w, &ms, &data)
}

/// [`operation_calls`] on the given manuscript and data file.
pub fn operation_calls_on(
    w: &World,
    ms: &str,
    data: &str,
) -> Vec<(&'static str, Result<Value, Value>)> {
    let spec = serde_json::to_value(AnalysisSpec {
        test_kind: TestKind::TTestWelch,
        roles: ColumnRoles::Grouped {
            value: "score".into(),
            group: "group".into(),
        },
        reported: ReportedStatistic::default(),
        tolerance: Tolerance::default(),
    })
    .unwrap();
    let handle = json!({ "run": 1, "id": 1 });
    vec![
        (
            "run_stats_consistency",
            call(&w.w, "run_stats_consistency", json!({ "path": ms })),
        ),
        (
            "run_stats_preview",
            call(
                &w.w,
                "run_stats_preview",
                json!({ "path": data, "manuscriptPath": null }),
            ),
        ),
        (
            "run_stats_verify",
            call(
                &w.w,
                "run_stats_verify",
                json!({ "path": data, "spec": spec, "manuscriptPath": null }),
            ),
        ),
        (
            "stats_l1_open",
            call(
                &w.w,
                "stats_l1_open",
                json!({ "manuscript": ms, "data": data }),
            ),
        ),
        (
            "stats_l1_edit_claim",
            call(
                &w.w,
                "stats_l1_edit_claim",
                json!({ "run": 1, "claimId": 0, "claim": claim() }),
            ),
        ),
        (
            "stats_l1_add_claim",
            call(
                &w.w,
                "stats_l1_add_claim",
                json!({ "run": 1, "claim": claim() }),
            ),
        ),
        (
            "stats_l1_proposals",
            call(&w.w, "stats_l1_proposals", json!({ "run": 1 })),
        ),
        (
            "stats_l1_helper_preview",
            call(
                &w.w,
                "stats_l1_helper_preview",
                json!({ "run": 1, "input": { "task": "result_explanation", "claimId": 0 }, "categories": [] }),
            ),
        ),
        (
            "stats_l1_helper_send",
            call(
                &w.w,
                "stats_l1_helper_send",
                json!({ "run": 1, "preview": handle }),
            ),
        ),
        (
            "stats_l1_confirm_binding",
            call(
                &w.w,
                "stats_l1_confirm_binding",
                json!({ "run": 1, "proposal": handle, "edits": null }),
            ),
        ),
        (
            "stats_l1_draft_proposal",
            call(
                &w.w,
                "stats_l1_draft_proposal",
                json!({ "run": 1, "claimId": 0, "from": null, "edits": edits() }),
            ),
        ),
        (
            "stats_l1_compute",
            call(&w.w, "stats_l1_compute", json!({ "run": 1, "tokens": [] })),
        ),
    ]
}

pub fn exempt_calls(w: &World) -> Vec<(&'static str, Result<Value, Value>)> {
    let setting = call(&w.w, "stats_ai_setting_get", json!({}));
    vec![
        (
            "stats_l1_close",
            call(&w.w, "stats_l1_close", json!({ "run": 1 })),
        ),
        (
            "stats_l1_helper_revoke",
            call(
                &w.w,
                "stats_l1_helper_revoke",
                json!({ "run": 1, "task": "claim_method" }),
            ),
        ),
        (
            "stats_l1_helper_cancel",
            call(&w.w, "stats_l1_helper_cancel", json!({ "run": 1 })),
        ),
        (
            "stats_ai_usage",
            call(&w.w, "stats_ai_usage", json!({ "run": 1 })),
        ),
        ("stats_ai_setting_get", setting.clone()),
        (
            "stats_ai_setting_set",
            call(
                &w.w,
                "stats_ai_setting_set",
                json!({ "setting": setting.clone().expect("a setting") }),
            ),
        ),
        (
            "stats_ai_key_status",
            call(&w.w, "stats_ai_key_status", json!({})),
        ),
        (
            "stats_ai_key_save",
            call(
                &w.w,
                "stats_ai_key_save",
                json!({ "key": "KEY_SECRET_a1b2" }),
            ),
        ),
        (
            "stats_ai_key_test",
            call(&w.w, "stats_ai_key_test", json!({})),
        ),
        (
            "stats_ai_key_remove",
            call(&w.w, "stats_ai_key_remove", json!({})),
        ),
    ]
}

pub fn is_workspace_refusal(e: &Value) -> bool {
    e["code"] == "validation"
        && e["message"]
            .as_str()
            .and_then(|m| serde_json::from_str::<Value>(m).ok())
            .is_some_and(|c| {
                c["code"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("workspace."))
            })
}
