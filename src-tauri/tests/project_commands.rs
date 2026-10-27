//! **The project commands, run (Research OS, Phase 5B-2; decisions 118 to
//! 127).** Every test works in a temporary folder given as the root, with
//! small caps, and calls the commands' plain functions with a window's
//! workspace session, as the Tauri wrappers do.
//!
//! * The workspace check: each gated command refuses with
//!   `workspace.inactive` while Statistics is inactive and writes nothing;
//!   each exempt command works with no workspace.
//! * Saving: a describe run and a t-test run, and a failed t-test with its
//!   code, saved, reopened and read back equal to what was saved; every run
//!   that may not be saved, and every name that may not be used, refused with
//!   its code and nothing written; a cap refused before anything is written;
//!   data that is no longer held refused.
//! * Deleting: each refusal of a confirmation, then a delete after which
//!   nothing under the root names the project; an unreadable project deleted.
//! * Races, with barriers and many repetitions: a deactivation, a delete and
//!   an open beside a Save; two Saves into one project; two first Saves that
//!   together would pass the all-projects cap; a Save racing its run's
//!   eviction.
//! * The exports through the commands, and every response's shape against
//!   its JSON fixture in `tests/fixtures/projects/`.
//!
//! The store-level fault injections (a crash during a first Save, the
//! process-wide lock under a paused Save) are in `src/project_store.rs`'s own
//! tests, where the fault points are compiled in.

mod project_support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use serde_json::{json, Value};

use app_lib::project_commands::{
    self as pc, DeleteConfirmationView, ProjectClosedView, ProjectCurrentView, ProjectDeletedView,
    ProjectExportView, ProjectListEntryView, ProjectOpenView, ProjectRefusalView, ProjectSizesView,
    RerunProgress, RerunStartedView, RerunView, RunResultView, SaveTarget, SavedRunView,
    StoredArtifactView, StoredRunView, CONFIRMATION_LIFETIME_MS,
};
use app_lib::runs::{Point, RunKind};
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{RunId, RunState};
use gaply_core::research_project::{ArtifactKind, Caps, DataFormat, ProjectCode, RerunComparison};

use project_support::*;

fn new_project(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

fn into_open(id: &str) -> SaveTarget {
    SaveTarget::OpenProject {
        project_id: id.to_string(),
    }
}

/// The names under `dir`, sorted, without exactly what the locks folder may
/// hold (Phase 5B-3a, decisions 132 and 144): `projects/.locks` itself, a real
/// owner-only folder, and in it only empty owner-only lock files named
/// `<id>.lock` for a project that is still listed. Anything else in the locks
/// folder fails here; anything else anywhere is returned, so the caller's own
/// rule still sees it.
fn names_without_the_locks(w: &World, dir: &Path) -> Vec<String> {
    let locks = w.root().join("projects").join(".locks");
    let live: BTreeSet<String> = pc::list(&w.root(), &w.ws)
        .unwrap()
        .into_iter()
        .map(|e| format!("{}.lock", e.project_id))
        .collect();
    let mut skip: Vec<PathBuf> = Vec::new();
    if let Ok(m) = std::fs::symlink_metadata(&locks) {
        assert!(
            m.is_dir() && !m.file_type().is_symlink(),
            "the locks folder is a real folder"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                m.permissions().mode() & 0o777,
                0o700,
                "the locks folder is owner-only"
            );
        }
        skip.push(locks.clone());
        for e in std::fs::read_dir(&locks).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let m = std::fs::symlink_metadata(e.path()).unwrap();
            assert!(
                live.contains(&name),
                "the locks folder holds {name}, which is no live project's lock file"
            );
            assert!(
                m.file_type().is_file() && m.len() == 0,
                "{name} is an empty file"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    m.permissions().mode() & 0o777,
                    0o600,
                    "{name} is owner-only"
                );
            }
            skip.push(e.path());
        }
    }
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if skip.contains(&p) {
                continue;
            }
            out.push(e.file_name().to_string_lossy().into_owned());
            if e.file_type().unwrap().is_dir() {
                stack.push(p);
            }
        }
    }
    out.sort();
    out
}

/// A world with one project holding one describe run: (world, project id).
fn with_a_project(tag: &str) -> (World, String) {
    let w = World::new(tag);
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    let saved = pc::save_run(&w.root(), &w.ws, run, new_project("Chapter 3")).expect("saved");
    (w, saved.project_id)
}

/* ---- the workspace check ---------------------------------------------------- */

#[test]
fn each_gated_command_refuses_while_statistics_is_inactive_and_writes_nothing() {
    let (w, id) = with_a_project("gated");
    w.ws.deactivate().expect("Statistics is left");
    assert_eq!(
        w.ws.projects().open_id(),
        None,
        "leaving closed the project"
    );
    // After the close, which checkpoints the project's journal.
    let before = files(&w.root());
    let r = w.root();
    let refusals: Vec<(&str, gaply_core::GaplyError)> = vec![
        (
            "save",
            pc::save_run(&r, &w.ws, 1, new_project("x")).unwrap_err(),
        ),
        (
            "save into",
            pc::save_run(&r, &w.ws, 1, into_open(&id)).unwrap_err(),
        ),
        ("open", pc::open(&r, &w.ws, &id).unwrap_err()),
        ("close", pc::close(&r, &w.ws).unwrap_err()),
        ("runs", pc::runs(&r, &w.ws, &id).unwrap_err()),
        ("result", pc::run_result(&r, &w.ws, &id, 1).unwrap_err()),
        ("report", pc::report(&r, &w.ws, &id).unwrap_err()),
        ("provenance", pc::provenance(&r, &w.ws, &id).unwrap_err()),
        ("rerun", pc::rerun(&r, &w.ws, 1).unwrap_err()),
    ];
    for (name, e) in &refusals {
        assert_eq!(code(e), "workspace.inactive", "{name}");
    }
    assert_eq!(refusals.len(), 9);
    assert_eq!(files(&w.root()), before, "nothing was written");
}

#[test]
fn each_exempt_command_works_with_no_workspace_active() {
    let (w, id) = with_a_project("exempt");
    w.ws.deactivate().expect("Statistics is left");
    let listed = pc::list(&w.root(), &w.ws).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].project_id, id);
    assert_eq!(listed[0].name.as_deref(), Some("Chapter 3"));
    assert_eq!(listed[0].run_count, Some(1));
    assert!(listed[0].saved_at.is_some() && listed[0].bytes > 0);
    let sizes = pc::sizes(&w.root(), &w.ws).expect("sizes");
    assert_eq!(sizes.project_count, 1);
    assert_eq!(sizes.project_cap_bytes, SMALL.project);
    assert_eq!(sizes.all_projects_cap_bytes, SMALL.all_projects);
    assert!(sizes.total_bytes >= listed[0].bytes);
    let c = pc::prepare_delete(&w.root(), &w.ws, &id).expect("prepare");
    assert_eq!(c.bytes, listed[0].bytes);
    pc::delete(&w.root(), &w.ws, &id, Some(&c.confirmation)).expect("delete");
    assert!(pc::list(&w.root(), &w.ws).unwrap().is_empty());
}

/* ---- saving and reading back ------------------------------------------------ */

#[test]
fn a_describe_and_a_ttest_run_save_reopen_and_read_back_equal() {
    let w = World::new("save");
    let slot = w.open("data.csv", CSV);
    let d = w.describe(slot);
    let t = w.ttest(slot);
    let e = w.engine();
    let (d_held, t_held) = (
        e.runs.result(RunId(d)).unwrap(),
        e.runs.ttest_result(RunId(t)).unwrap(),
    );
    let d_bundle = (
        serde_json::to_string(&d_held).unwrap(),
        serde_json::to_string(&d_held.manifest).unwrap(),
    );
    let t_bundle = (
        serde_json::to_string(&t_held).unwrap(),
        serde_json::to_string(&t_held.manifest).unwrap(),
    );

    let first = pc::save_run(&w.root(), &w.ws, d, new_project("Chapter 3")).expect("first");
    assert!(first.created);
    assert_eq!(first.stored_run_id, 1);
    assert_eq!(
        w.ws.projects().open_id().map(|i| i.as_str().to_string()),
        Some(first.project_id.clone()),
        "the project saved into is open"
    );
    let second = pc::save_run(&w.root(), &w.ws, t, into_open(&first.project_id)).expect("second");
    assert!(!second.created);
    assert_eq!(second.stored_run_id, 2);

    pc::close(&w.root(), &w.ws).expect("close");
    let opened = pc::open(&w.root(), &w.ws, &first.project_id).expect("open");
    assert_eq!(opened.name, "Chapter 3");
    assert_eq!(opened.run_count, 2);
    let runs = pc::runs(&w.root(), &w.ws, &first.project_id).expect("runs");
    assert_eq!(runs.len(), 2);
    assert!(runs
        .iter()
        .all(|r| r.state == RunState::Succeeded && r.end_code.is_none()));
    assert_eq!(runs[0].capability_id, "stats.describe");
    assert_eq!(runs[1].capability_id, "stats.analyze.independent_t");

    for (stored, (bundle, manifest)) in [(1u64, &d_bundle), (2, &t_bundle)] {
        let back = pc::run_result(&w.root(), &w.ws, &first.project_id, stored).expect("read");
        assert_eq!(back.state, RunState::Succeeded);
        assert_eq!(
            back.bundle_json.as_ref(),
            Some(bundle),
            "the bundle reads back byte for byte"
        );
        assert_eq!(
            back.manifest_json.as_ref(),
            Some(manifest),
            "and its manifest"
        );
        assert!(back.failure_json.is_none());
        assert_eq!(back.data_file.kind, ArtifactKind::DataFile);
        assert_eq!(back.data_file.format, DataFormat::Csv);
        assert_eq!(back.data_file.bytes, CSV.len() as u64);
        let spec: Value = serde_json::from_str(&back.spec_json).unwrap();
        assert_eq!(
            spec["inputs"][0]["contentHash"],
            json!(back.data_file.sha256)
        );
        let m: Value = serde_json::from_str(manifest).unwrap();
        assert_eq!(m["inputs"][0], json!(back.data_file.sha256));
    }
    // One data file, saved by two runs: one artifact.
    let data_files = files(&w.root())
        .keys()
        .filter(|k| k.contains("artifacts"))
        .count();
    assert_eq!(
        data_files,
        1 + 2 + 2 + 2,
        "one data file, two specs, two bundles, two manifests"
    );
}

#[test]
fn a_failed_ttest_saves_with_its_code_and_reads_back_the_same() {
    let w = World::new("failed");
    let slot = w.open("constant.csv", CONSTANT);
    let t = w.ttest(slot);
    assert_eq!(w.wait(t), RunState::Failed);
    let snap = w.engine().runs.ttest_snapshot(RunId(t)).unwrap();
    let failure = serde_json::to_value(snap.diagnostics.last().unwrap()).unwrap();
    assert_eq!(failure["code"], "stats.ttest.both_groups_constant");

    let saved = pc::save_run(&w.root(), &w.ws, t, new_project("Failed one")).expect("saved");
    pc::close(&w.root(), &w.ws).unwrap();
    pc::open(&w.root(), &w.ws, &saved.project_id).unwrap();
    let back = pc::run_result(&w.root(), &w.ws, &saved.project_id, 1).expect("read");
    assert_eq!(back.state, RunState::Failed);
    assert!(back.bundle_json.is_none() && back.manifest_json.is_none());
    let rec: Value = serde_json::from_str(&back.failure_json.expect("its failure record")).unwrap();
    assert_eq!(
        rec["failure"], failure,
        "the code and its parameters, the same"
    );
    assert_eq!(rec["status"], "failed");
}

#[test]
fn runs_that_may_not_be_saved_are_refused_with_their_codes_and_nothing_is_written() {
    // Cancelled, while queued.
    let gate = Gate::new();
    let w = World::with(
        "refused",
        SMALL,
        Some(gated_runs(&gate, Point::BeforeStart)),
    );
    let slot = w.open("a.csv", CSV);
    let queued = w.start_describe(slot);
    gate.wait_reached();
    let before = files(&w.root());
    let e = pc::save_run(&w.root(), &w.ws, queued, new_project("p")).unwrap_err();
    assert_eq!(code(&e), "project.run_not_finished");
    w.engine().runs.cancel(RunId(queued)).unwrap();
    gate.release();
    assert_eq!(w.wait(queued), RunState::Cancelled);
    let e = pc::save_run(&w.root(), &w.ws, queued, new_project("p")).unwrap_err();
    assert_eq!(code(&e), "project.run_cancelled");
    // Unknown.
    let e = pc::save_run(&w.root(), &w.ws, 999, new_project("p")).unwrap_err();
    assert_eq!(code(&e), "project.run_unknown");
    assert_eq!(files(&w.root()), before, "nothing written");

    // Evicted: nine later runs push it out of the eight held.
    let w = World::new("evicted");
    let slot = w.open("a.csv", CSV);
    let first = w.describe(slot);
    for _ in 0..8 {
        w.describe(slot);
    }
    let before = files(&w.root());
    let e = pc::save_run(&w.root(), &w.ws, first, new_project("p")).unwrap_err();
    assert_eq!(code(&e), "project.run_evicted");
    assert_eq!(files(&w.root()), before);

    // Saved twice into one project.
    let last = w.describe(slot);
    let saved = pc::save_run(&w.root(), &w.ws, last, new_project("p")).unwrap();
    let before = files(&w.root());
    let e = pc::save_run(&w.root(), &w.ws, last, into_open(&saved.project_id)).unwrap_err();
    assert_eq!(code(&e), "project.run_already_saved");
    assert_eq!(files(&w.root()), before, "nothing written");
    let runs = pc::runs(&w.root(), &w.ws, &saved.project_id).unwrap();
    assert_eq!(runs.len(), 1);
}

#[test]
fn a_run_whose_data_is_no_longer_held_is_refused_and_nothing_is_written() {
    let w = World::new("data");
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    // As the screen does when another file is opened: the old slot closed.
    assert!(app_lib::stats_desc_commands::close(
        &w.engine().datasets,
        slot
    ));
    w.open("b.csv", CONSTANT);
    let e = pc::save_run(&w.root(), &w.ws, run, new_project("p")).unwrap_err();
    assert_eq!(code(&e), "project.data_unavailable");

    // Or evicted from the four slots held.
    let slot = w.open("c.csv", CSV);
    let run = w.describe(slot);
    for n in 0..4 {
        w.open(&format!("d{n}.csv"), CONSTANT);
    }
    let e = pc::save_run(&w.root(), &w.ws, run, new_project("p")).unwrap_err();
    assert_eq!(code(&e), "project.data_unavailable");
    assert!(
        pc::list(&w.root(), &w.ws).unwrap().is_empty(),
        "no project made"
    );
    assert!(
        names(&w.root()).iter().all(|n| n == "projects"),
        "{:?}",
        names(&w.root())
    );
}

#[test]
fn names_are_checked_and_markdown_and_html_come_out_escaped() {
    let w = World::new("names");
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    for bad in [
        String::new(),
        "   ".to_string(),
        "é".repeat(101),
        "a\u{7}b".to_string(),
        "evil\u{202E}txt".to_string(),
        "zero\u{200B}width".to_string(),
    ] {
        let e = pc::save_run(&w.root(), &w.ws, run, new_project(&bad)).unwrap_err();
        assert_eq!(code(&e), "project.bad_title", "{bad:?}");
    }
    assert!(pc::list(&w.root(), &w.ws).unwrap().is_empty());
    // 100 characters, trimmed.
    let hundred = format!("  {}  ", "é".repeat(100));
    let saved = pc::save_run(&w.root(), &w.ws, run, new_project(&hundred)).expect("100 is allowed");
    let opened = pc::open(&w.root(), &w.ws, &saved.project_id).unwrap();
    assert_eq!(opened.name, "é".repeat(100), "trimmed");

    let run = w.describe(slot);
    let html = "# <script>alert(1)</script> *bold* [x](javascript:y) & `c`";
    let saved = pc::save_run(&w.root(), &w.ws, run, new_project(html)).unwrap();
    let report = pc::report(&w.root(), &w.ws, &saved.project_id).unwrap();
    let text = String::from_utf8(report.bytes).unwrap();
    let first = text.lines().next().unwrap();
    assert_eq!(
        first,
        "# \\# &lt;script&gt;alert\\(1\\)&lt;\\/script&gt; \\*bold\\* \\[x\\]\\(javascript\\:y\\) &amp; \\`c\\`"
    );
    assert!(!text.contains("<script>") && !text.contains(html));
}

#[test]
fn a_save_that_would_pass_a_cap_is_refused_before_anything_is_written() {
    let tiny = Caps {
        project: 70 * 1024,
        all_projects: 10 * 1024 * 1024,
        db_allowance: 64 * 1024,
    };
    let w = World::with("cap", tiny, None);
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    assert!(pc::list(&w.root(), &w.ws).unwrap().is_empty());
    let before = names_without_the_locks(&w, &w.root());
    let e = pc::save_run(&w.root(), &w.ws, run, new_project("p")).unwrap_err();
    let r = refusal(&e);
    assert_eq!(r["code"], "project.project_cap");
    assert!(r["needed"].as_u64().unwrap() > r["limit"].as_u64().unwrap());
    assert_eq!(r["limit"], json!(tiny.project));
    assert_eq!(
        names_without_the_locks(&w, &w.root()),
        before,
        "no project, no staging folder"
    );
}

/* ---- deleting --------------------------------------------------------------- */

#[test]
fn delete_refuses_every_wrong_confirmation_then_removes_everything() {
    let (w, id) = with_a_project("delete");
    let slot = w.open("b.csv", CSV);
    let run = w.describe(slot);
    let other = pc::save_run(&w.root(), &w.ws, run, new_project("Other"))
        .unwrap()
        .project_id;
    let r = w.root();

    let e = pc::delete(&r, &w.ws, &id, None).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_missing");
    let e = pc::delete(&r, &w.ws, &id, Some("")).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_missing");
    let e = pc::delete(&r, &w.ws, &id, Some(&"a".repeat(64))).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_unknown");

    // Replaced by a newer one.
    let old = pc::prepare_delete(&r, &w.ws, &id).unwrap();
    let new = pc::prepare_delete(&r, &w.ws, &id).unwrap();
    let e = pc::delete(&r, &w.ws, &id, Some(&old.confirmation)).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_replaced");
    // For another project: refused, and used up.
    let e = pc::delete(&r, &w.ws, &other, Some(&new.confirmation)).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_other_project");
    let e = pc::delete(&r, &w.ws, &id, Some(&new.confirmation)).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_used");
    // Expired.
    let late = pc::prepare_delete(&r, &w.ws, &id).unwrap();
    assert_eq!(late.expires_in_ms, CONFIRMATION_LIFETIME_MS);
    w.clock.advance(CONFIRMATION_LIFETIME_MS);
    let e = pc::delete(&r, &w.ws, &id, Some(&late.confirmation)).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_expired");
    assert_eq!(pc::list(&r, &w.ws).unwrap().len(), 2, "nothing was deleted");

    // A good one, just in time, for the open project: closed first.
    w.ws.projects();
    pc::open(&r, &w.ws, &id).unwrap();
    let good = pc::prepare_delete(&r, &w.ws, &id).unwrap();
    w.clock.advance(CONFIRMATION_LIFETIME_MS - 1);
    let done = pc::delete(&r, &w.ws, &id, Some(&good.confirmation)).expect("deleted");
    assert!(done.closed);
    assert_eq!(w.ws.projects().open_id(), None);
    let e = pc::delete(&r, &w.ws, &id, Some(&good.confirmation)).unwrap_err();
    assert_eq!(code(&e), "project.confirmation_used", "used once");
    assert!(
        names(&r).iter().all(|n| !n.contains(&id)),
        "nothing names it: {:?}",
        names(&r)
    );
    assert_eq!(pc::list(&r, &w.ws).unwrap().len(), 1);
}

#[test]
fn an_unreadable_project_is_listed_without_a_name_and_can_still_be_deleted() {
    let (w, id) = with_a_project("unreadable");
    pc::close(&w.root(), &w.ws).unwrap();
    let db = w.root().join("projects").join(&id).join("project.db");
    for side in ["project.db-wal", "project.db-shm"] {
        let _ = std::fs::remove_file(db.with_file_name(side));
    }
    std::fs::write(&db, vec![0x5a; 4096]).unwrap();
    let listed = pc::list(&w.root(), &w.ws).unwrap();
    assert_eq!(listed[0].name, None);
    assert_eq!(listed[0].refused, Some(ProjectCode::DbUnreadable));
    assert!(listed[0].bytes > 0);
    let e = pc::open(&w.root(), &w.ws, &id).unwrap_err();
    assert_eq!(code(&e), "project.db_unreadable");
    let c = pc::prepare_delete(&w.root(), &w.ws, &id).unwrap();
    pc::delete(&w.root(), &w.ws, &id, Some(&c.confirmation)).expect("deleted");
    assert!(names(&w.root()).iter().all(|n| !n.contains(&id)));
}

/* ---- races -------------------------------------------------------------------- */

const ROUNDS: usize = 24;

#[test]
fn a_deactivation_during_a_save_leaves_the_save_whole_or_not_at_all() {
    let w = World::new("race-deactivate");
    let slot_bytes = CSV.to_vec();
    let mut saved_rounds = 0;
    for round in 0..ROUNDS {
        if w.ws.require_statistics().is_err() {
            w.ws.activate(gaply_core::contracts::WorkspaceId::Statistics)
                .unwrap();
        }
        let slot = w.open(&format!("r{round}.csv"), &slot_bytes);
        let run = w.describe(slot);
        let gate = Arc::new(Barrier::new(2));
        let (ws, root, g) = (w.ws.clone(), w.root(), gate.clone());
        let saver = std::thread::spawn(move || {
            g.wait();
            pc::save_run(&root, &ws, run, new_project(&format!("p{round}")))
        });
        gate.wait();
        let left = w.ws.deactivate();
        let saved = saver.join().unwrap();
        assert!(left.is_ok());
        assert_eq!(
            w.ws.projects().open_id(),
            None,
            "leaving closed whatever was saved into"
        );
        match saved {
            Ok(v) => {
                saved_rounds += 1;
                let listed = pc::list(&w.root(), &w.ws).unwrap();
                let e = listed
                    .iter()
                    .find(|e| e.project_id == v.project_id)
                    .expect("listed");
                assert_eq!(e.run_count, Some(1), "whole");
            }
            Err(e) => assert_eq!(code(&e), "workspace.inactive"),
        }
        let listed = pc::list(&w.root(), &w.ws).unwrap();
        assert_eq!(listed.len(), saved_rounds, "no project without its run");
        assert!(listed
            .iter()
            .all(|e| e.refused.is_none() && e.run_count == Some(1)));
        assert!(
            names_without_the_locks(&w, &w.root().join("projects"))
                .iter()
                .all(|n| !n.starts_with('.')),
            "no leftover"
        );
    }
}

#[test]
fn a_delete_during_a_save_into_the_same_project_never_leaves_half_of_either() {
    for round in 0..ROUNDS {
        let (w, id) = with_a_project(&format!("race-delete-{round}"));
        let slot = w.open("b.csv", CSV);
        let run = w.describe(slot);
        let c = pc::prepare_delete(&w.root(), &w.ws, &id).unwrap();
        let gate = Arc::new(Barrier::new(2));
        let (ws, root, g, pid) = (w.ws.clone(), w.root(), gate.clone(), id.clone());
        let saver = std::thread::spawn(move || {
            g.wait();
            pc::save_run(&root, &ws, run, into_open(&pid))
        });
        gate.wait();
        let deleted = pc::delete(&w.root(), &w.ws, &id, Some(&c.confirmation));
        let saved = saver.join().unwrap();
        deleted.expect("the delete goes through, before or after the save");
        if let Err(e) = saved {
            assert_eq!(code(&e), "project.not_open", "the delete closed it first");
        }
        assert!(
            names(&w.root()).iter().all(|n| !n.contains(&id)),
            "nothing of it left"
        );
        assert_eq!(w.ws.projects().open_id(), None);
    }
}

#[test]
fn a_delete_during_an_open_leaves_no_open_handle_on_a_deleted_project() {
    for round in 0..ROUNDS {
        let (w, id) = with_a_project(&format!("race-open-{round}"));
        pc::close(&w.root(), &w.ws).unwrap();
        let c = pc::prepare_delete(&w.root(), &w.ws, &id).unwrap();
        let gate = Arc::new(Barrier::new(2));
        let (ws, root, g, pid) = (w.ws.clone(), w.root(), gate.clone(), id.clone());
        let opener = std::thread::spawn(move || {
            g.wait();
            pc::open(&root, &ws, &pid)
        });
        gate.wait();
        let deleted = pc::delete(&w.root(), &w.ws, &id, Some(&c.confirmation));
        let opened = opener.join().unwrap();
        deleted.expect("deleted, closing the project first if the open won");
        if let Err(e) = opened {
            assert_eq!(code(&e), "project.not_found");
        }
        assert_eq!(w.ws.projects().open_id(), None);
        assert!(names(&w.root()).iter().all(|n| !n.contains(&id)));
    }
}

#[test]
fn two_saves_at_once_into_one_project_both_land_and_one_run_saves_once() {
    for round in 0..ROUNDS {
        let (w, id) = with_a_project(&format!("race-two-{round}"));
        let slot = w.open("b.csv", CSV);
        let a = w.describe(slot);
        let b = w.describe(slot);
        let gate = Arc::new(Barrier::new(3));
        let spawn = |run: u64| {
            let (ws, root, g, pid) = (w.ws.clone(), w.root(), gate.clone(), id.clone());
            std::thread::spawn(move || {
                g.wait();
                pc::save_run(&root, &ws, run, into_open(&pid))
            })
        };
        let (ta, tb, tb2) = (spawn(a), spawn(b), spawn(b));
        let (ra, rb, rb2) = (ta.join().unwrap(), tb.join().unwrap(), tb2.join().unwrap());
        ra.expect("a lands");
        let b_saves = [&rb, &rb2].iter().filter(|r| r.is_ok()).count();
        assert_eq!(b_saves, 1, "the same run saves once");
        let refused = if rb.is_err() {
            rb.unwrap_err()
        } else {
            rb2.unwrap_err()
        };
        assert_eq!(code(&refused), "project.run_already_saved");
        let runs = pc::runs(&w.root(), &w.ws, &id).unwrap();
        assert_eq!(runs.len(), 3);
        let ids: Vec<u64> = runs.iter().map(|r| r.stored_run_id).collect();
        assert_eq!(ids, [1, 2, 3]);
    }
}

#[test]
fn two_first_saves_that_together_would_pass_the_all_projects_cap_cannot_both_land() {
    for round in 0..ROUNDS {
        // Room for one project of one run, not two.
        let probe = World::new(&format!("cap-probe-{round}"));
        let slot = probe.open("a.csv", CSV);
        let run = probe.describe(slot);
        pc::save_run(&probe.root(), &probe.ws, run, new_project("one")).unwrap();
        let one = pc::sizes(&probe.root(), &probe.ws).unwrap().total_bytes;
        drop(probe);
        // A first Save checks its staging folder, smaller than a finished
        // project, so one fits; the second also counts the first, so it does
        // not.
        let caps = Caps {
            project: 10 * 1024 * 1024,
            all_projects: one + SMALL.db_allowance + one / 4,
            db_allowance: SMALL.db_allowance,
        };
        let w = World::with(&format!("race-cap-{round}"), caps, None);
        let slot = w.open("a.csv", CSV);
        let a = w.describe(slot);
        let b = w.describe(slot);
        let gate = Arc::new(Barrier::new(3));
        let spawn = |run: u64, name: &'static str| {
            let (ws, root, g) = (w.ws.clone(), w.root(), gate.clone());
            std::thread::spawn(move || {
                g.wait();
                pc::save_run(&root, &ws, run, new_project(name))
            })
        };
        let (ta, tb) = (spawn(a, "A"), spawn(b, "B"));
        gate.wait();
        let results = [ta.join().unwrap(), tb.join().unwrap()];
        let ok = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok, 1, "exactly one fits");
        let refused = results.iter().find_map(|r| r.as_ref().err()).unwrap();
        assert_eq!(code(refused), "project.all_projects_cap");
        let total = pc::sizes(&w.root(), &w.ws).unwrap().total_bytes;
        assert!(
            total <= caps.all_projects,
            "{total} > {}",
            caps.all_projects
        );
        assert_eq!(pc::list(&w.root(), &w.ws).unwrap().len(), 1);
    }
}

#[test]
fn a_save_racing_its_runs_eviction_is_whole_or_refused() {
    for round in 0..ROUNDS {
        let w = World::new(&format!("race-evict-{round}"));
        let slot = w.open("a.csv", CSV);
        let target = w.describe(slot);
        for _ in 0..7 {
            w.describe(slot);
        }
        let gate = Arc::new(Barrier::new(2));
        let (ws, root, g) = (w.ws.clone(), w.root(), gate.clone());
        let saver = std::thread::spawn(move || {
            g.wait();
            pc::save_run(&root, &ws, target, new_project("p"))
        });
        gate.wait();
        w.describe(slot); // the ninth: evicts the oldest finished run
        match saver.join().unwrap() {
            Ok(v) => {
                let e = pc::list(&w.root(), &w.ws).unwrap();
                assert_eq!(e.len(), 1);
                assert_eq!(e[0].project_id, v.project_id);
                assert_eq!(e[0].run_count, Some(1));
            }
            Err(e) => {
                assert_eq!(code(&e), "project.run_evicted");
                assert!(pc::list(&w.root(), &w.ws).unwrap().is_empty());
            }
        }
    }
}

/* ---- the exports ------------------------------------------------------------ */

#[test]
fn the_exports_render_through_the_commands_with_fixed_names_and_the_same_bytes() {
    let w = World::new("exports");
    let slot = w.open("data.csv", CSV);
    let d = w.describe(slot);
    let t = w.ttest(slot);
    let id = pc::save_run(&w.root(), &w.ws, d, new_project("Q3 <zebra>"))
        .unwrap()
        .project_id;
    pc::save_run(&w.root(), &w.ws, t, into_open(&id)).unwrap();
    let slot = w.open("constant.csv", CONSTANT);
    let f = w.ttest(slot);
    pc::save_run(&w.root(), &w.ws, f, into_open(&id)).unwrap();

    let report = pc::report(&w.root(), &w.ws, &id).unwrap();
    assert_eq!(report.file_name, "gaply_project_report.md");
    let text = String::from_utf8(report.bytes.clone()).unwrap();
    assert!(text.starts_with("# Q3 &lt;zebra&gt;\n"));
    assert_eq!(
        text.matches("zebra").count(),
        1,
        "the name only as the title"
    );
    for want in [
        "## Run 1: `stats.describe`, version 1",
        "## Run 2: `stats.analyze.independent_t`, version 1",
        "## Run 3: `stats.analyze.independent_t`, version 1",
        "- Status: failed",
        "- Failure code: `stats.ttest.both_groups_constant`",
        "- Column 1: outcome",
        "- Column 2: group",
        "- Variant: welch",
        "| used, data | data_file | csv |",
        "| generated | result_bundle | json |",
        "| generated | run_manifest | json |",
        "| generated | run_failure | json |",
    ] {
        assert!(text.contains(want), "missing `{want}` in:\n{text}");
    }
    assert!(!text.contains("score"), "no header");
    let again = pc::report(&w.root(), &w.ws, &id).unwrap();
    assert_eq!(
        again.bytes, report.bytes,
        "the same project, the same bytes"
    );
    assert_eq!(report.sha256, again.sha256);

    let prov = pc::provenance(&w.root(), &w.ws, &id).unwrap();
    assert_eq!(prov.file_name, "gaply_project_provenance.json");
    let v: Value = serde_json::from_slice(&prov.bytes).unwrap();
    for key in [
        "prefix",
        "entity",
        "activity",
        "agent",
        "used",
        "wasGeneratedBy",
        "wasDerivedFrom",
        "wasAssociatedWith",
    ] {
        assert!(v.get(key).is_some(), "{key}");
    }
    assert_eq!(v["activity"].as_object().unwrap().len(), 3);
    assert_eq!(v["agent"].as_object().unwrap().len(), 1);
    // Run 1 used a data file and a spec and generated two: four derivations.
    // Run 3 generated one: two.
    assert_eq!(v["wasDerivedFrom"].as_object().unwrap().len(), 4 + 4 + 2);
    assert!(!String::from_utf8_lossy(&prov.bytes).contains("zebra"));
    assert_eq!(
        pc::provenance(&w.root(), &w.ws, &id).unwrap().bytes,
        prov.bytes
    );
}

#[cfg(unix)]
#[test]
fn a_damaged_artifact_gives_its_code_and_nothing_else() {
    let (w, id) = with_a_project("damaged");
    let back = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap();
    assert!(back.bundle_json.is_some());
    let bundle_hash = {
        // The bundle's file: the one artifact holding `primaryOutputs`.
        let dir = w.root().join("projects").join(&id).join("artifacts");
        let mut found = None;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let bytes = std::fs::read(e.path()).unwrap();
            if bytes.starts_with(b"{\"schemaVersion\"")
                && bytes.windows(14).any(|w| w == b"primaryOutputs")
            {
                found = Some(e.path());
            }
        }
        found.expect("the bundle's file")
    };
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&bundle_hash, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut bytes = std::fs::read(&bundle_hash).unwrap();
    bytes[20] ^= 0x01;
    std::fs::write(&bundle_hash, &bytes).unwrap();
    let e = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap_err();
    assert_eq!(
        refusal(&e),
        json!({ "code": "project.artifact_wrong_hash" }),
        "the code and nothing else"
    );
    let e = pc::report(&w.root(), &w.ws, &id).unwrap_err();
    assert_eq!(code(&e), "project.artifact_wrong_hash");
}

#[test]
fn reading_needs_the_open_project_and_a_run_it_holds() {
    let (w, id) = with_a_project("reading");
    let e = pc::run_result(&w.root(), &w.ws, &id, 7).unwrap_err();
    assert_eq!(code(&e), "project.stored_run_unknown");
    let e = pc::runs(&w.root(), &w.ws, &"b".repeat(32)).unwrap_err();
    assert_eq!(code(&e), "project.not_open");
    let e = pc::runs(&w.root(), &w.ws, "../x").unwrap_err();
    assert_eq!(code(&e), "project.bad_project_id");
    pc::close(&w.root(), &w.ws).unwrap();
    let e = pc::report(&w.root(), &w.ws, &id).unwrap_err();
    assert_eq!(code(&e), "project.not_open");
    let e = pc::save_run(&w.root(), &w.ws, 1, into_open(&id)).unwrap_err();
    assert_eq!(code(&e), "project.not_open");
}

/* ---- the wire: every response's shape, and the code list -------------------- */

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects")
}

fn assert_fixture<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    let p = fixtures().join(name);
    if std::env::var_os("GAPLY_WRITE_PROJECT_FIXTURES").is_some() {
        std::fs::write(&p, &got).unwrap();
    }
    let want = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

const ID: &str = "0123456789abcdef0123456789abcdef";
const HASH: &str = "5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a";

#[test]
fn every_response_serialises_to_its_fixture() {
    assert_fixture(
        "projectList.json",
        &vec![
            ProjectListEntryView {
                project_id: ID.into(),
                name: Some("Chapter 3".into()),
                bytes: 40960,
                saved_at: Some(1_792_396_351),
                run_count: Some(2),
                refused: None,
            },
            ProjectListEntryView {
                project_id: "fedcba9876543210fedcba9876543210".into(),
                name: None,
                bytes: 4096,
                saved_at: None,
                run_count: None,
                refused: Some(ProjectCode::DbUnreadable),
            },
        ],
    );
    assert_fixture(
        "projectSizes.json",
        &ProjectSizesView {
            total_bytes: 45056,
            project_count: 2,
            project_cap_bytes: 104_857_600,
            all_projects_cap_bytes: 10_737_418_240,
        },
    );
    assert_fixture(
        "projectPrepareDelete.json",
        &DeleteConfirmationView {
            project_id: ID.into(),
            confirmation: HASH.into(),
            bytes: 40960,
            expires_in_ms: 300_000,
        },
    );
    assert_fixture(
        "projectDeleted.json",
        &ProjectDeletedView {
            project_id: ID.into(),
            closed: true,
        },
    );
    assert_fixture(
        "projectSaveRun.json",
        &SavedRunView {
            project_id: ID.into(),
            stored_run_id: 1,
            created: true,
        },
    );
    assert_fixture(
        "projectOpen.json",
        &ProjectOpenView {
            project_id: ID.into(),
            name: "Chapter 3".into(),
            created_at: 1_792_396_351,
            run_count: 2,
            bytes: 40960,
        },
    );
    assert_fixture("projectClose.json", &ProjectClosedView { closed: true });
    // Phase 5B-3b (decision 148): an id, or none. Phase 5C-1a (decision
    // 187): with the last rerun, going, saved or not saved, or none.
    let rerun = |status, stored_run_id, code| {
        Some(RerunView {
            run_id: 7,
            rerun_of: 1,
            status,
            stored_run_id,
            code,
        })
    };
    assert_fixture(
        "projectCurrent.json",
        &vec![
            ProjectCurrentView {
                project_id: Some(ID.into()),
                rerun: None,
            },
            ProjectCurrentView {
                project_id: Some(ID.into()),
                rerun: rerun(RerunProgress::Going, None, None),
            },
            ProjectCurrentView {
                project_id: Some(ID.into()),
                rerun: rerun(RerunProgress::Saved, Some(3), None),
            },
            ProjectCurrentView {
                project_id: Some(ID.into()),
                rerun: rerun(RerunProgress::NotSaved, None, Some(ProjectCode::DiskFull)),
            },
            ProjectCurrentView {
                project_id: None,
                rerun: None,
            },
        ],
    );
    // Phase 5C-1a: project_rerun's answer.
    assert_fixture(
        "projectRerun.json",
        &vec![
            RerunStartedView {
                run_id: 7,
                kind: RunKind::Describe,
            },
            RerunStartedView {
                run_id: 8,
                kind: RunKind::Ttest,
            },
        ],
    );
    assert_fixture(
        "projectRuns.json",
        &vec![
            StoredRunView {
                stored_run_id: 1,
                capability_id: "stats.analyze.independent_t".into(),
                capability_version: 1,
                state: RunState::Failed,
                end_code: Some(ProjectCode::RunInterrupted),
                started_at: 1_792_396_340,
                ended_at: Some(1_792_396_341),
                saved_at: 1_792_396_351,
                agent_version: "0.1.0".into(),
                rerun_of: None,
                comparison: None,
                engine_version_differs: None,
            },
            StoredRunView {
                // Phase 5C-1a (decision 189): a rerun, compared.
                stored_run_id: 2,
                capability_id: "stats.describe".into(),
                capability_version: 1,
                state: RunState::Succeeded,
                end_code: None,
                started_at: 1_792_396_360,
                ended_at: Some(1_792_396_361),
                saved_at: 1_792_396_362,
                agent_version: "0.1.0".into(),
                rerun_of: Some(1),
                comparison: Some(RerunComparison::Identical),
                engine_version_differs: Some(false),
            },
        ],
    );
    assert_fixture(
        "projectRunResult.json",
        &RunResultView {
            stored_run_id: 1,
            capability_id: "stats.analyze.independent_t".into(),
            state: RunState::Failed,
            data_file: StoredArtifactView {
                sha256: HASH.into(),
                kind: ArtifactKind::DataFile,
                format: DataFormat::Xlsx,
                bytes: 2048,
            },
            spec_json: "{\"capabilityId\":\"stats.analyze.independent_t\"}".into(),
            // Every optional field filled, to pin the shape: a real run has a
            // bundle and a manifest, or a failure record.
            bundle_json: Some("{\"status\":\"succeeded\"}".into()),
            manifest_json: Some(format!("{{\"outputHash\":\"{HASH}\"}}")),
            failure_json: Some("{\"schemaVersion\":1,\"status\":\"failed\",\"failure\":{\"code\":\"stats.ttest.both_groups_constant\",\"params\":{},\"severity\":\"error\"}}".into()),
        },
    );
    assert_fixture(
        "projectReport.json",
        &ProjectExportView {
            file_name: "gaply_project_report.md",
            bytes: b"# T\n".to_vec(),
            sha256: HASH.into(),
        },
    );
    assert_fixture(
        "projectProvenance.json",
        &ProjectExportView {
            file_name: "gaply_project_provenance.json",
            bytes: b"{}\n".to_vec(),
            sha256: HASH.into(),
        },
    );
    assert_fixture(
        "projectRefusal.json",
        &[
            ProjectRefusalView {
                code: "project.project_cap",
                needed: Some(104_857_601),
                limit: Some(104_857_600),
            },
            ProjectRefusalView {
                code: "project.not_open",
                needed: None,
                limit: None,
            },
        ],
    );
}

/* ---- the wire: stored results, as run_result returns them (Phase 5B-3b) --- */

/// Every (path, JSON type) in `v`, arrays read through. The layout of a value
/// without the values themselves.
fn layout(v: &Value, at: &str, out: &mut BTreeSet<String>) {
    let kind = match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    out.insert(format!("{at}: {kind}"));
    match v {
        Value::Array(items) => items
            .iter()
            .for_each(|x| layout(x, &format!("{at}[]"), out)),
        Value::Object(m) => m
            .iter()
            .for_each(|(k, x)| layout(x, &format!("{at}.{k}"), out)),
        _ => {}
    }
}

/// A run read back, as JSON, with each stored text parsed in place.
fn stored_layout(mut v: Value) -> BTreeSet<String> {
    for k in ["specJson", "bundleJson", "manifestJson", "failureJson"] {
        if let Some(text) = v[k].as_str() {
            v[k] = serde_json::from_str(text).expect("stored JSON");
        }
    }
    let mut out = BTreeSet::new();
    layout(&v, "", &mut out);
    out
}

/// The elapsed time and the platform fixed, so a regenerated fixture does not
/// change with the machine that wrote it.
fn steady(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("\"elapsedMs\":") {
        let (head, tail) = rest.split_at(i + "\"elapsedMs\":".len());
        out.push_str(head);
        let digits = tail.chars().take_while(|c| c.is_ascii_digit()).count();
        out.push('0');
        rest = &tail[digits..];
    }
    out.push_str(rest);
    for (os, arch) in [
        ("macos", "aarch64"),
        ("macos", "x86_64"),
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("windows", "x86_64"),
        ("windows", "aarch64"),
    ] {
        out = out.replace(
            &format!("\"platform\":{{\"os\":\"{os}\",\"arch\":\"{arch}\"}}"),
            "\"platform\":{\"os\":\"macos\",\"arch\":\"aarch64\"}",
        );
    }
    out
}

/// Pins a stored result's layout against `tests/fixtures/projects/stored/`,
/// for the webview's shape check (`storedShape.ts`, tested against these
/// files). The layout, not the bytes: a fixture holds real values from the
/// Mac that wrote it, and a p-value's last digit or an elapsed time may differ
/// on Linux or Windows; every key, every nesting and every JSON type must be
/// the same. Written with `GAPLY_WRITE_PROJECT_FIXTURES` set, as the others.
fn assert_stored_fixture(name: &str, view: &RunResultView) {
    let mut got = serde_json::to_value(view).unwrap();
    for k in ["specJson", "bundleJson", "manifestJson", "failureJson"] {
        if let Some(text) = got[k].as_str() {
            got[k] = Value::String(steady(text));
        }
    }
    let p = fixtures().join("stored").join(name);
    if std::env::var_os("GAPLY_WRITE_PROJECT_FIXTURES").is_some() {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
    }
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{name}: {e}")),
    )
    .unwrap();
    assert_eq!(
        stored_layout(got),
        stored_layout(want),
        "{name}: the stored result and its fixture differ in layout"
    );
}

#[test]
fn stored_results_have_the_layout_of_their_fixtures() {
    let w = World::new("stored");
    let slot = w.open("a.csv", CSV);
    let d = w.describe(slot);
    let t = w.ttest(slot);
    let id = pc::save_run(&w.root(), &w.ws, d, new_project("Stored"))
        .unwrap()
        .project_id;
    pc::save_run(&w.root(), &w.ws, t, into_open(&id)).unwrap();
    let constant = w.open("constant.csv", CONSTANT);
    let f = w.ttest(constant);
    assert_eq!(w.wait(f), RunState::Failed);
    pc::save_run(&w.root(), &w.ws, f, into_open(&id)).unwrap();
    for (n, name, state, capability) in [
        (1, "summary.json", RunState::Succeeded, "stats.describe"),
        (
            2,
            "ttest.json",
            RunState::Succeeded,
            "stats.analyze.independent_t",
        ),
        (
            3,
            "ttest_failed.json",
            RunState::Failed,
            "stats.analyze.independent_t",
        ),
    ] {
        let back = pc::run_result(&w.root(), &w.ws, &id, n).expect("read back");
        assert_eq!(back.state, state, "{name}");
        assert_eq!(back.capability_id, capability, "{name}");
        assert_stored_fixture(name, &back);
    }
    // The layout check is not inert: a summary is not laid out as a t-test.
    let a = stored_layout(
        serde_json::to_value(pc::run_result(&w.root(), &w.ws, &id, 1).unwrap()).unwrap(),
    );
    let b = stored_layout(
        serde_json::to_value(pc::run_result(&w.root(), &w.ws, &id, 2).unwrap()).unwrap(),
    );
    assert_ne!(a, b);
    assert!(
        a.len() > 40,
        "the layout reads into the stored JSON: {}",
        a.len()
    );
}

/// The one-sample t-test's saved results (Research OS, Phase 8D-2b, decision
/// 269): a result, and a failed run whose values are all equal, laid out as
/// their fixtures, for the webview's shape check, as the three above are.
#[test]
fn stored_one_sample_results_have_the_layout_of_their_fixtures() {
    let w = World::new("stored-one-sample");
    let slot = w.open("a.csv", CSV);
    let run = w.one_sample(slot, 1, 3.0);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Stored one-sample"))
        .unwrap()
        .project_id;
    let equal = w.open("equal.csv", b"x\n4\n4\n4\n");
    let f = w.one_sample(equal, 1, 3.0);
    assert_eq!(w.wait_new_kind(f), RunState::Failed);
    pc::save_run(&w.root(), &w.ws, f, into_open(&id)).unwrap();
    for (n, name, state) in [
        (1, "one_sample.json", RunState::Succeeded),
        (2, "one_sample_failed.json", RunState::Failed),
    ] {
        let back = pc::run_result(&w.root(), &w.ws, &id, n).expect("read back");
        assert_eq!(back.state, state, "{name}");
        assert_eq!(back.capability_id, "stats.analyze.one_sample_t", "{name}");
        assert_stored_fixture(name, &back);
    }
    let failure = pc::run_result(&w.root(), &w.ws, &id, 2)
        .unwrap()
        .failure_json
        .expect("a failure record");
    assert!(
        failure.contains("stats.onesample.values_constant"),
        "{failure}"
    );
}

/// The paired t-test's saved results (Research OS, Phase 8D-2c, decision
/// 272): a result, and a failed run whose two different columns hold equal
/// values, so every difference is 0 (`differences_constant`, a run's
/// refusal; `same_column` would need one alias twice), laid out as their
/// fixtures, as the one-sample test's are. The shared CSV has one numeric
/// column, so this opens its own two-column bytes.
#[test]
fn stored_paired_results_have_the_layout_of_their_fixtures() {
    let w = World::new("stored-paired");
    let slot = w.open("pairs.csv", b"a,b\n1.5,1.0\n2.5,2.25\n3.0,2.0\n4.5,3.75\n");
    let run = w.paired(slot, 1, 2);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Stored paired"))
        .unwrap()
        .project_id;
    let equal = w.open("equal_pairs.csv", b"a,b\n4,4\n5,5\n6,6\n");
    let f = w.paired(equal, 1, 2);
    assert_eq!(w.wait_new_kind(f), RunState::Failed);
    pc::save_run(&w.root(), &w.ws, f, into_open(&id)).unwrap();
    for (n, name, state) in [
        (1, "paired.json", RunState::Succeeded),
        (2, "paired_failed.json", RunState::Failed),
    ] {
        let back = pc::run_result(&w.root(), &w.ws, &id, n).expect("read back");
        assert_eq!(back.state, state, "{name}");
        assert_eq!(back.capability_id, "stats.analyze.paired_t", "{name}");
        assert_stored_fixture(name, &back);
    }
    let failure = pc::run_result(&w.root(), &w.ws, &id, 2)
        .unwrap()
        .failure_json
        .expect("a failure record");
    assert!(
        failure.contains("stats.onesample.differences_constant"),
        "{failure}"
    );
}

#[test]
fn the_project_code_list_is_pinned() {
    let codes: Vec<&str> = ProjectCode::ALL.iter().map(|c| c.wire()).collect();
    assert_fixture("projectCodes.json", &codes);
}

#[test]
fn a_save_target_reads_from_the_webview_shape_and_nothing_else() {
    let t: SaveTarget =
        serde_json::from_value(json!({ "into": "newProject", "name": "x" })).unwrap();
    assert!(matches!(t, SaveTarget::NewProject { name } if name == "x"));
    let t: SaveTarget =
        serde_json::from_value(json!({ "into": "openProject", "projectId": ID })).unwrap();
    assert!(matches!(t, SaveTarget::OpenProject { project_id } if project_id == ID));
    for bad in [
        json!({ "into": "newProject", "name": "x", "path": "/tmp" }),
        json!({ "into": "openProject", "project_id": ID }),
        json!({ "into": "elsewhere" }),
        json!({ "name": "x" }),
    ] {
        assert!(
            serde_json::from_value::<SaveTarget>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}
