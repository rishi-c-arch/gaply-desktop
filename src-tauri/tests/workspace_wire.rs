//! **The workspace wire, from the Rust side (Research OS, Phase 3).** Each
//! view and refusal the `workspace_*` commands return serialises to exactly
//! the bytes of its fixture in `src/screens/workspace/fixtures/` (pretty JSON,
//! two-space indent, a final newline). `workspaceTypes.vitest.ts` deep-compares
//! the same fixtures with copies typed `satisfies` the TypeScript views, so a
//! renamed key fails `tsc` and a changed value fails one side or the other.
//!
//! Also: `workspaceCodes.json`, the code list the screens' wording is keyed
//! by, is exactly `WorkspaceCode::ALL`, sorted.

use std::path::PathBuf;

use app_lib::workspace::{list, WorkspaceStateView};
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::workspace::{transition, WorkspaceCode, WorkspaceEvent, WorkspaceState};
use gaply_core::contracts::{Diagnostic, ParamKey, ParamValue, WorkspaceId};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/workspace")
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let want = std::fs::read_to_string(screens().join("fixtures").join(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

#[test]
fn every_workspace_view_serialises_to_its_fixture() {
    assert_bytes("workspaceList.json", &list());
    assert_bytes(
        "workspaceStateView.json",
        &WorkspaceStateView {
            state: WorkspaceState::Active(WorkspaceId::Statistics),
            active_run: Some(3),
            latest_run: Some(3),
        },
    );
    assert_bytes(
        "workspaceStateViewNone.json",
        &WorkspaceStateView {
            state: WorkspaceState::NoWorkspace,
            active_run: None,
            latest_run: None,
        },
    );
    // As `WorkspaceSession::deactivate` builds it (its content through IPC:
    // `workspace_session.rs`).
    assert_bytes(
        "refusalRunActive.json",
        &Diagnostic::error(WorkspaceCode::RunActive)
            .int(ParamKey::Run, 3)
            .with(
                ParamKey::Workspace,
                ParamValue::Workspace(WorkspaceId::Statistics),
            ),
    );
    assert_bytes(
        "refusalIllegalTransition.json",
        &transition(
            WorkspaceState::Active(WorkspaceId::Statistics),
            WorkspaceEvent::Activate(WorkspaceId::Compute),
        )
        .unwrap_err(),
    );
}

#[test]
fn the_screens_code_list_is_exactly_the_workspace_codes() {
    let json = std::fs::read_to_string(screens().join("workspaceCodes.json")).unwrap();
    let listed: Vec<String> = serde_json::from_str(&json).unwrap();
    let mut codes: Vec<String> = WorkspaceCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    codes.sort();
    assert_eq!(listed, codes);
}
