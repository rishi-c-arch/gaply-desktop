//! Integration test: invoke real commands through the Tauri IPC harness
//! (mock runtime, no window server needed) with the DB mocked out.

use std::sync::{Arc, Mutex};

use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::WebviewWindowBuilder;

use app_lib::commands;
use app_lib::state::AppState;
use gaply_core::db::ProjectStore;
use gaply_core::projects::{NewProject, Project};
use gaply_core::{AppConfig, Database, GaplyError};

/// DB mock — no SQLite, just a Vec behind a Mutex.
#[derive(Default)]
struct MockStore {
    projects: Mutex<Vec<Project>>,
}

impl ProjectStore for MockStore {
    fn insert_project(&self, new: &NewProject) -> Result<Project, GaplyError> {
        let mut projects = self.projects.lock().unwrap();
        let project = Project {
            id: projects.len() as i64 + 1,
            name: new.name.clone(),
            description: new.description.clone(),
            created_at: new.created_at,
        };
        projects.push(project.clone());
        Ok(project)
    }

    fn list_projects(&self) -> Result<Vec<Project>, GaplyError> {
        Ok(self.projects.lock().unwrap().clone())
    }

    fn get_project(&self, id: i64) -> Result<Option<Project>, GaplyError> {
        Ok(self.projects.lock().unwrap().iter().find(|p| p.id == id).cloned())
    }

    fn ping(&self) -> Result<(), GaplyError> {
        Ok(())
    }
}

fn test_app() -> (tauri::App<tauri::test::MockRuntime>, Arc<MockStore>) {
    let store = Arc::new(MockStore::default());
    // embedded in-memory DB for the db_* commands; project commands use the mock
    let db = Arc::new(Database::in_memory().expect("in-memory db"));
    let state = AppState::new(AppConfig::new("/tmp/unused-in-tests.db"), db, store.clone());
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::create_project,
            commands::list_projects,
            commands::get_project,
            commands::health_check,
            commands::db_init,
            commands::db_migrate,
            commands::db_health,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build mock app");
    (app, store)
}

fn invoke(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    body: serde_json::Value,
) -> Result<InvokeResponseBody, serde_json::Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            // must be the platform-local origin: on macOS/Linux the webview
            // serves from tauri://localhost; a non-local origin is ACL-denied
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map_err(|e| e)
}

#[test]
fn create_project_roundtrip_through_ipc() {
    let (app, store) = test_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create test webview");

    let response = invoke(
        &webview,
        "create_project",
        serde_json::json!({ "name": "  Citation Atlas ", "description": "graph of citations" }),
    )
    .expect("create_project should succeed");

    let created: Project =
        response.deserialize().expect("response should deserialize into Project");

    // Core validation ran: name arrived trimmed.
    assert_eq!(created.name, "Citation Atlas");
    assert_eq!(created.description, "graph of citations");
    assert_eq!(created.id, 1);

    // The mocked store actually received the row.
    assert_eq!(store.projects.lock().unwrap().len(), 1);

    // And list_projects sees it through IPC too.
    let listed: Vec<Project> = invoke(&webview, "list_projects", serde_json::json!({}))
        .expect("list_projects should succeed")
        .deserialize()
        .expect("response should deserialize into Vec<Project>");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Citation Atlas");
}

#[test]
fn db_commands_report_migrated_healthy_database() {
    let (app, _store) = test_app();
    let webview = WebviewWindowBuilder::new(&app, "db", Default::default())
        .build()
        .expect("failed to create test webview");

    let health: serde_json::Value = invoke(&webview, "db_health", serde_json::json!({}))
        .expect("db_health should succeed")
        .deserialize()
        .unwrap();
    assert_eq!(health["ok"], true, "health: {health}");
    assert_eq!(health["pending_migrations"], 0);
    assert!(health["vec_version"].is_string(), "sqlite-vec missing: {health}");

    // migrations already ran at open — db_migrate must be a no-op
    let migrate: serde_json::Value = invoke(&webview, "db_migrate", serde_json::json!({}))
        .expect("db_migrate should succeed")
        .deserialize()
        .unwrap();
    assert_eq!(migrate["applied"].as_array().unwrap().len(), 0);

    let init: serde_json::Value = invoke(&webview, "db_init", serde_json::json!({}))
        .expect("db_init should succeed")
        .deserialize()
        .unwrap();
    assert_eq!(init["path"], ":memory:");
}

#[test]
fn validation_error_crosses_ipc_with_stable_shape() {
    let (app, _store) = test_app();
    let webview = WebviewWindowBuilder::new(&app, "err", Default::default())
        .build()
        .expect("failed to create test webview");

    let err = invoke(&webview, "create_project", serde_json::json!({ "name": "   " }))
        .expect_err("empty name must be rejected");

    assert_eq!(err["code"], "validation");
    assert_eq!(err["message"], "project name must not be empty");
}
