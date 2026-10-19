//! **`project_current` (Research OS, Phase 5B-3b; decision 148).** The id of
//! the project open in this copy of the app, or none; nothing else. Exempt
//! from the workspace check, so a screen can ask it whatever is active.
//!
//! It follows Rust's own session, never a screen's memory: none before any
//! Open or Save; the id after a first Save and after an Open; none after
//! Close, after leaving Statistics and after the open project is deleted.

mod project_support;

use serde_json::{json, Value};

use app_lib::project_commands::{self as pc, SaveTarget};
use gaply_core::contracts::WorkspaceId;

use project_support::*;

fn new_project(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

fn current(w: &World) -> Option<String> {
    pc::current(&w.ws)
        .expect("project_current never refuses")
        .project_id
}

#[test]
fn none_before_any_open_or_save_with_or_without_a_workspace() {
    let w = World::inactive("current-inactive", SMALL, None);
    assert_eq!(current(&w), None, "no workspace, nothing open");
    let w = World::new("current-fresh");
    assert_eq!(current(&w), None, "Statistics active, nothing open");
}

#[test]
fn it_follows_save_open_close_leave_and_delete() {
    let w = World::new("current-follows");
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    let a = pc::save_run(&w.root(), &w.ws, run, new_project("A"))
        .expect("saved")
        .project_id;
    assert_eq!(current(&w), Some(a.clone()), "after a first Save");

    pc::close(&w.root(), &w.ws).expect("closed");
    assert_eq!(current(&w), None, "after Close");

    pc::open(&w.root(), &w.ws, &a).expect("opened");
    assert_eq!(current(&w), Some(a.clone()), "after Open");

    // A second project: opening it replaces the first.
    let run = w.describe(slot);
    let b = pc::save_run(&w.root(), &w.ws, run, new_project("B"))
        .expect("saved")
        .project_id;
    assert_ne!(a, b);
    assert_eq!(current(&w), Some(b.clone()), "the Save's project is open");
    pc::open(&w.root(), &w.ws, &a).expect("opened");
    assert_eq!(current(&w), Some(a.clone()), "Open replaced it");

    w.ws.deactivate().expect("Statistics is left");
    assert_eq!(current(&w), None, "after leaving Statistics");

    w.ws.activate(WorkspaceId::Statistics)
        .expect("Statistics activates");
    assert_eq!(current(&w), None, "activating opens nothing");
    pc::open(&w.root(), &w.ws, &a).expect("opened");
    assert_eq!(current(&w), Some(a.clone()));
    let c = pc::prepare_delete(&w.root(), &w.ws, &a).expect("prepared");
    let gone = pc::delete(&w.root(), &w.ws, &a, Some(&c.confirmation)).expect("deleted");
    assert!(gone.closed, "it was open, and was closed first");
    assert_eq!(current(&w), None, "after the open project is deleted");

    // Deleting a project that is not open leaves the open one open. Leaving
    // Statistics dropped the dataset, so the file is opened again.
    let slot = w.open("a.csv", CSV);
    pc::open(&w.root(), &w.ws, &b).expect("opened");
    let run = w.describe(slot);
    let c3 = pc::save_run(&w.root(), &w.ws, run, new_project("C"))
        .expect("saved")
        .project_id;
    pc::open(&w.root(), &w.ws, &b).expect("opened");
    let c = pc::prepare_delete(&w.root(), &w.ws, &c3).expect("prepared");
    pc::delete(&w.root(), &w.ws, &c3, Some(&c.confirmation)).expect("deleted");
    assert_eq!(current(&w), Some(b), "another project's delete");
}

#[test]
fn it_answers_an_id_and_nothing_else() {
    let w = World::new("current-shape");
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Sentinel ZQXJ name"))
        .expect("saved")
        .project_id;
    let open: Value = serde_json::to_value(pc::current(&w.ws).unwrap()).unwrap();
    assert_eq!(open, json!({ "projectId": id }), "the id, and no name");
    pc::close(&w.root(), &w.ws).unwrap();
    let none: Value = serde_json::to_value(pc::current(&w.ws).unwrap()).unwrap();
    assert_eq!(none, json!({ "projectId": null }));
}
