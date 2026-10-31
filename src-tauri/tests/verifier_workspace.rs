//! **The verifier's commands and the workspace (Research OS, Phase 4A,
//! decision 49), through IPC.** An app over Tauri's mock runtime with the
//! verifier's commands and the four `workspace_*` commands, its AI context on
//! an in-memory key store and a transport that is never asked:
//!
//! * every operation command (Level 0, the legacy lane, Level 1's nine) is
//!   refused with `workspace.inactive` while no workspace is active and while
//!   Statistics is activating (a readiness check parked mid-activation), and
//!   runs once Statistics is active: it answers a view, or a refusal of its
//!   own that is not a workspace refusal;
//! * every exempt command runs while no workspace is active: a user can
//!   always delete their key and drop a run;
//! * `workspace_list` carries the registry's capability ids;
//! * the app tells the registry whether it is a devtools build.
//!
//! No key, keychain, network request or real service: the store is
//! `MemoryCredentialStore`, the key is the sentinel `KEY_SECRET_a1b2`, the
//! key test runs with AI help off (no request), and the link is checked by
//! `checked_source_link` and refused before anything could open it.

mod desc_support;
mod verifier_support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::stats_l1_commands as l1;
use app_lib::workspace::{self as ws, Health, HealthCheck};
use desc_support::{call, refusal};
use gaply_core::contracts::capability::Build;
use verifier_support::*;

fn assert_all_inactive(calls: Vec<(&str, Result<Value, Value>)>, phase: &str) {
    assert_eq!(calls.len(), 12, "every operation command");
    for (name, r) in calls {
        let e = r.expect_err(name);
        let c = refusal(&e);
        assert_eq!(c["code"], "workspace.inactive", "{name}: {c}");
        assert_eq!(c["params"]["expected"], "statistics", "{name}");
        assert_eq!(c["params"]["from"], phase, "{name}");
    }
}

#[test]
fn every_operation_command_is_refused_until_statistics_is_active_and_runs_once_it_is() {
    // A readiness check that parks until released: the state stays Activating.
    let (asked_tx, asked_rx) = mpsc::channel::<()>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let parked: HealthCheck = Arc::new(move |_| {
        asked_tx.send(()).ok();
        go_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .ok();
        Health::Ready
    });
    let w = world(parked);

    assert_all_inactive(operation_calls(&w), "no_workspace");

    std::thread::scope(|s| {
        let webview = w.w.clone();
        let activator = s.spawn(move || {
            call(
                &webview,
                "workspace_activate",
                json!({ "id": "statistics" }),
            )
        });
        asked_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the readiness check ran");
        assert_eq!(
            call(&w.w, "workspace_state", json!({})).unwrap()["state"]["phase"],
            "activating"
        );
        assert_all_inactive(operation_calls(&w), "activating");
        go_tx.send(()).unwrap();
        activator.join().unwrap().expect("Statistics activates");
    });

    let calls = operation_calls(&w);
    for (name, r) in &calls {
        if let Err(e) = r {
            assert!(
                !is_workspace_refusal(e),
                "{name} is still refused by the workspace: {e}"
            );
        }
    }
    // The ones that reach real work answer a view.
    for name in [
        "run_stats_consistency",
        "run_stats_preview",
        "run_stats_verify",
        "stats_l1_open",
    ] {
        let r = &calls.iter().find(|(n, _)| *n == name).unwrap().1;
        assert!(r.is_ok(), "{name}: {r:?}");
    }
}

#[test]
fn every_exempt_command_runs_while_no_workspace_is_active() {
    let w = world(Arc::new(ws::readiness));
    assert_eq!(
        call(&w.w, "workspace_state", json!({})).unwrap()["state"]["phase"],
        "no_workspace"
    );
    let calls = exempt_calls(&w);
    // `stats_open_source_link` takes the Wry `AppHandle`, so it cannot be
    // registered on the mock runtime: its body's first step, the link check,
    // runs here directly, and `workspace_guards.rs` shows it has no workspace
    // check.
    assert_eq!(
        calls.len(),
        10,
        "every exempt command the mock runtime can hold"
    );
    assert!(
        l1::checked_source_link("not a link").is_err(),
        "refused before anything opens"
    );
    for (name, r) in &calls {
        if let Err(e) = r {
            assert!(!is_workspace_refusal(e), "{name} is gated: {e}");
        }
    }
    let ok = |name: &str| calls.iter().find(|(n, _)| *n == name).unwrap().1.is_ok();
    for name in [
        "stats_l1_close",
        "stats_ai_setting_get",
        "stats_ai_setting_set",
        "stats_ai_key_status",
        "stats_ai_key_save",
        "stats_ai_key_test",
        "stats_ai_key_remove",
    ] {
        assert!(
            ok(name),
            "{name} answers a view while no workspace is active"
        );
    }
    // The key went in and came out of the in-memory store; nothing else holds it.
    let status = call(&w.w, "stats_ai_key_status", json!({})).unwrap();
    assert_eq!(status["saved"], false);
}

#[test]
fn workspace_list_carries_the_registrys_capability_ids() {
    let w = world(Arc::new(ws::readiness));
    let list = call(&w.w, "workspace_list", json!({})).unwrap();
    let stats = list
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "statistics")
        .unwrap();
    let ids: Vec<&str> = stats["capabilityIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let registry: Vec<String> = gaply_core::stats_registry::Registry::statistics()
        .ids()
        .iter()
        .map(|i| i.as_str().to_string())
        .collect();
    assert_eq!(ids, registry.iter().map(String::as_str).collect::<Vec<_>>());
    // Phase 8D-1: twelve, with `stats.analyze.one_sample_t` and
    // `stats.analyze.paired_t` after `stats.analyze.independent_t`; Phase 8G:
    // thirteen, with `stats.analyze.one_way_anova` last (decision 321).
    assert_eq!(ids.len(), 13);
}

#[test]
fn the_app_tells_the_registry_whether_it_is_a_devtools_build() {
    assert_eq!(
        ws::build(),
        Build::from_devtools(cfg!(feature = "devtools"))
    );
    let flag = AtomicBool::new(cfg!(feature = "devtools"));
    let expected = if flag.load(Ordering::SeqCst) {
        Build::Devtools
    } else {
        Build::Standard
    };
    assert_eq!(ws::build(), expected);
}
