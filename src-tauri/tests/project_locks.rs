//! **Two copies of the app and one project (Research OS, Phase 5B-3a;
//! decision 132).** An open project holds its lock file,
//! `projects/.locks/<id>.lock`, shared; a delete takes it exclusive and holds
//! it until the folder is gone; recovery at start takes it exclusive before
//! it finishes a tombstone or clears a staging folder.
//!
//! **What stands in for a second copy.** A second, separate `File` handle on
//! the same lock file, in this process: `flock` (macOS, Linux) and
//! `LockFileEx` (Windows) treat two handles as two holders, even in one
//! process, which is what lets these tests run without a second app.
//!
//! **What this cannot test**, and why: a real second process (none is
//! started here; the two-handle stand-in rests on that property of `flock`
//! and `LockFileEx`); a copy that crashes while holding the lock (the system
//! releases it when the process ends: inferred from the documentation, not
//! measured; a dropped handle is the nearest stand-in, below); and Windows,
//! where these tests compile in CI but the app crate's test binaries do not
//! run (`windows-build-check.yml`, decision 114).

mod project_support;

use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};

use app_lib::project_commands::{self as pc, SaveTarget};
use app_lib::project_store::ProjectRoot;
use gaply_core::research_project::{ProjectCode, ProjectId};

use project_support::*;

const LOCKS: &str = ".locks";

fn projects(w: &World) -> PathBuf {
    w.root().join("projects")
}

fn lock_path(w: &World, id: &str) -> PathBuf {
    projects(w).join(LOCKS).join(format!("{id}.lock"))
}

/// Another copy's handle on the project's lock file.
fn other_copy(p: &Path) -> File {
    File::options()
        .read(true)
        .write(true)
        .open(p)
        .expect("the lock file exists")
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// A world with a project made by its first Save, open in this copy.
fn saved(tag: &str) -> (World, String) {
    let w = World::new(tag);
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    let s = pc::save_run(
        &w.root(),
        &w.ws,
        run,
        SaveTarget::NewProject {
            name: "Chapter 3".into(),
        },
    )
    .expect("saved");
    (w, s.project_id)
}

#[test]
fn an_open_project_holds_its_lock_shared_until_it_closes() {
    let (w, id) = saved("held");
    let p = lock_path(&w, &id);
    assert_eq!(fs::metadata(&p).expect("a lock file").len(), 0, "empty");
    let other = other_copy(&p);
    assert!(
        matches!(other.try_lock(), Err(TryLockError::WouldBlock)),
        "while open here, no other copy can take it exclusive"
    );
    other
        .try_lock_shared()
        .expect("another copy may open it too");
    other.unlock().unwrap();
    pc::close(&w.root(), &w.ws).expect("closed");
    other.try_lock().expect("closing released this copy's lock");
}

#[test]
fn a_lock_is_released_when_its_holder_is_dropped() {
    let w = World::new("dropped");
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let h = store.create("p").unwrap();
    let id = h.id().as_str().to_string();
    let other = other_copy(&lock_path(&w, &id));
    assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));
    drop(h);
    other.try_lock().expect("dropping the handle released it");
}

#[test]
fn a_delete_is_refused_while_another_copy_has_the_project_open_and_closes_it_here() {
    let (w, id) = saved("open-elsewhere");
    let p = lock_path(&w, &id);
    let other = other_copy(&p);
    other.try_lock_shared().expect("the other copy opens it");
    let c = pc::prepare_delete(&w.root(), &w.ws, &id).expect("prepare");
    let e = pc::delete(&w.root(), &w.ws, &id, Some(&c.confirmation)).unwrap_err();
    assert_eq!(code(&e), "project.open_elsewhere");
    assert_eq!(
        w.ws.projects().open_id(),
        None,
        "a refused delete still left the project closed in this copy"
    );
    assert!(projects(&w).join(&id).is_dir(), "nothing was removed");
    assert!(p.is_file(), "the lock file stays");
    assert_eq!(pc::list(&w.root(), &w.ws).unwrap().len(), 1);

    drop(other);
    let c = pc::prepare_delete(&w.root(), &w.ws, &id).expect("prepare again");
    pc::delete(&w.root(), &w.ws, &id, Some(&c.confirmation)).expect("deleted");
    assert_eq!(names(&projects(&w)), [LOCKS], "the folder is gone");
    assert!(
        names(&projects(&w).join(LOCKS)).is_empty(),
        "and then its lock file"
    );
}

#[test]
fn an_open_is_refused_while_another_copy_is_deleting() {
    let (w, id) = saved("being-deleted");
    pc::close(&w.root(), &w.ws).unwrap();
    let other = other_copy(&lock_path(&w, &id));
    other.try_lock().expect("the other copy's delete holds it");
    let e = pc::open(&w.root(), &w.ws, &id).unwrap_err();
    assert_eq!(code(&e), "project.being_deleted");
    assert_eq!(w.ws.projects().open_id(), None);
    drop(other);
    pc::open(&w.root(), &w.ws, &id).expect("opens once the other copy let go");
}

#[test]
fn a_lock_error_has_its_own_code_and_a_symlinked_locks_folder_is_refused() {
    // The lock file is a folder: it cannot be locked.
    let w = World::new("lock-error");
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let id = store.create("p").unwrap().id().as_str().to_string();
    let p = lock_path(&w, &id);
    fs::remove_file(&p).unwrap();
    fs::create_dir(&p).unwrap();
    assert_eq!(
        store.open(&id).err().map(|r| r.code),
        Some(ProjectCode::LockFailed)
    );
    assert_eq!(
        store.delete(&id).err().map(|r| r.code),
        Some(ProjectCode::LockFailed)
    );
    assert!(projects(&w).join(&id).is_dir(), "nothing was removed");

    // The locks folder is a symlink: refused, its target untouched.
    #[cfg(unix)]
    {
        let w = World::new("locks-symlink");
        let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
        let elsewhere = w.dir.0.join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, projects(&w).join(LOCKS)).unwrap();
        assert_eq!(
            store.create("p").err().map(|r| r.code),
            Some(ProjectCode::SymlinkRefused)
        );
        assert!(names(&elsewhere).is_empty(), "nothing was written there");
    }
}

#[test]
fn the_locks_folder_is_skipped_by_the_list_the_sizes_and_the_caps() {
    let (w, id) = saved("skipped");
    assert!(ProjectId::parse(LOCKS).is_err(), "never a valid project id");
    // Bytes in the locks folder (a lock file is empty; these stand in for
    // anything else found there) count toward nothing.
    let junk = projects(&w).join(LOCKS).join("junk");
    fs::write(&junk, vec![0u8; (SMALL.all_projects + 1) as usize]).unwrap();

    let list = pc::list(&w.root(), &w.ws).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].project_id, id);
    let sizes = pc::sizes(&w.root(), &w.ws).unwrap();
    assert_eq!(sizes.project_count, 1);
    assert_eq!(
        sizes.total_bytes, list[0].bytes,
        "the locks folder is not counted"
    );

    // A Save still passes the all-projects cap, which the junk alone exceeds.
    let slot = w.open("b.csv", CONSTANT);
    let run = w.ttest(slot);
    pc::save_run(
        &w.root(),
        &w.ws,
        run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("the caps skip the locks folder");
    fs::remove_file(&junk).unwrap();
}

#[test]
fn opening_a_project_writes_its_lock_file_and_nothing_else_of_its_own() {
    let w = World::new("open-writes");
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let id = store.create("p").unwrap().id().as_str().to_string();
    // Closed, and its lock file removed (no holder), so the open must make it.
    fs::remove_file(lock_path(&w, &id)).unwrap();
    let before = files(&w.root());
    let h = store.open(&id).unwrap();
    let after = files(&w.root());
    let added: Vec<&String> = after.keys().filter(|k| !before.contains_key(*k)).collect();
    let lock = format!("projects/{LOCKS}/{id}.lock");
    // SQLite's own companions of an open WAL database come and go with the
    // connection (5B-1's journal mode); they are not the app's writes.
    assert!(
        added
            .iter()
            .all(|k| **k == lock || k.ends_with("project.db-wal") || k.ends_with("project.db-shm")),
        "{added:?}"
    );
    assert!(added.iter().any(|k| **k == lock), "{added:?}");
    assert_eq!(after[&lock].0, 0, "the lock file is empty");
    for (k, v) in &before {
        if !k.contains("project.db") {
            assert_eq!(after.get(k), Some(v), "{k} is unchanged");
        }
    }
    drop(h);
}

#[test]
fn recovery_skips_a_leftover_another_copy_holds_and_finishes_it_later() {
    let w = World::new("recovery");
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let a = store.create("a").unwrap().id().as_str().to_string();
    let b = store.create("b").unwrap().id().as_str().to_string();
    drop(store);
    // A delete another copy is in the middle of, and one a crash left.
    let tomb_a = projects(&w).join(format!(".deleting-{a}"));
    let tomb_b = projects(&w).join(format!(".deleting-{b}"));
    fs::rename(projects(&w).join(&a), &tomb_a).unwrap();
    fs::rename(projects(&w).join(&b), &tomb_b).unwrap();
    // A first Save another copy is in the middle of.
    let c = "c".repeat(32);
    let staging = projects(&w).join(format!(".creating-{c}"));
    fs::create_dir(&staging).unwrap();
    fs::write(lock_path(&w, &c), b"").unwrap();

    let deleting = other_copy(&lock_path(&w, &a));
    deleting.try_lock().unwrap();
    let creating = other_copy(&lock_path(&w, &c));
    creating.try_lock_shared().unwrap();

    ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    assert!(tomb_a.is_dir(), "held by another copy: skipped");
    assert!(staging.is_dir(), "held by another copy: skipped");
    assert!(!tomb_b.exists(), "nobody held it: finished");
    assert!(!lock_path(&w, &b).exists(), "and its lock file removed");

    drop(deleting);
    drop(creating);
    ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    assert_eq!(names(&projects(&w)), [LOCKS]);
    assert!(names(&projects(&w).join(LOCKS)).is_empty());
}

#[test]
fn opening_a_project_that_is_not_there_leaves_no_lock_file() {
    let w = World::new("missing");
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let id = "d".repeat(32);
    assert_eq!(
        store.open(&id).err().map(|r| r.code),
        Some(ProjectCode::NotFound)
    );
    assert!(!lock_path(&w, &id).exists());
    assert_eq!(
        store.delete(&id).err().map(|r| r.code),
        Some(ProjectCode::NotFound)
    );
    assert!(!lock_path(&w, &id).exists());
}
