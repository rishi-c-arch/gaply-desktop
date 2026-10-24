//! **The privacy matrix with a rerun (Research OS, Phase 5C-1a; decisions 186
//! and 190), in the pattern of `stats_desc_privacy_matrix.rs`.** Its seven
//! sentinels, and a project named by an eighth:
//!
//! * a header, `HDR_SENTINEL_q81`, of the selected numeric column;
//! * a level label, `LVL_SENTINEL_k44`, of the selected categorical column;
//! * the file name `FILE_SENTINEL_d17.csv`, in the folder `PATH_SENTINEL_w93`;
//! * `7123.4567`, in a numeric column the user does not select;
//! * `61.8129`, in the selected numeric column (no statistic can be it: the
//!   describe matrix's own data, checked there against R);
//! * `TXT_SENTINEL_m02`, a text cell in a column the user does not select;
//! * the project's name, `TITLESENTp22`.
//!
//! The flow, through the commands' plain functions: open the file, run a
//! summary, save it into a new project, run it again, let the save at the
//! end land, read the project back (`project_current`, `project_runs`, each
//! run's result) and render both exports, then delete the project.
//!
//! * In the app's data folder, the sentinels of the data appear only inside
//!   `projects/<id>/artifacts/`, and the name only in `project.db`, where
//!   `project.title` is; the file's name and its folder appear in no file.
//!   Every other file of the folder (a migrated `gaply.db`, which the test
//!   makes first) is byte for byte as it was.
//! * No tracing line, from any thread (the rerun's save at the end runs on
//!   its worker), holds any sentinel.
//! * No response holds one, but a run's result read back (the header and the
//!   label, as 5B allows) and the report's title (the name).
//! * After the project is deleted, no file in the folder holds any.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod desc_support;
mod l1_support;
mod project_support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use app_lib::project_commands::{self as pc, RerunProgress, SaveTarget};
use app_lib::stats_desc_commands as desc;
use desc_support::{delimited, C};
use gaply_core::contracts::OperationId;
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;
use project_support::World;

const HDR: &str = "HDR_SENTINEL_q81";
const LVL: &str = "LVL_SENTINEL_k44";
const FILE: &str = "FILE_SENTINEL_d17";
const PATH: &str = "PATH_SENTINEL_w93";
const UNSEL_NUM: &str = "7123.4567";
const SEL_NUM: &str = "61.8129";
const TXT: &str = "TXT_SENTINEL_m02";
const TITLE: &str = "TITLESENTp22";

const SENTINELS: [&str; 7] = [HDR, LVL, FILE, PATH, UNSEL_NUM, SEL_NUM, TXT];

fn table() -> Vec<u8> {
    let headers = [HDR, "group", "note", "aux"];
    let score = [
        "44.1257", "10.5", "95.5", SEL_NUM, "23.25", "50", "82", "31", "70.75",
    ];
    let group = ["a", LVL, "a", LVL, "a", LVL, "a", LVL, "a"];
    let note = ["n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9"];
    let aux = ["1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9"];
    let rows: Vec<Vec<C>> = (0..9)
        .map(|i| vec![C::N(score[i]), C::T(group[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if e.file_type().unwrap().is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Every sentinel `bytes` holds, as text, and each number also as the
/// 8-byte big-endian double SQLite stores.
fn found(bytes: &[u8]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = SENTINELS
        .iter()
        .chain([TITLE].iter())
        .copied()
        .filter(|s| contains(bytes, s.as_bytes()))
        .collect();
    for (name, n) in [(SEL_NUM, 61.8129f64), (UNSEL_NUM, 7123.4567f64)] {
        if contains(bytes, &n.to_be_bytes()) && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Every file under the app's data folder but the project's own, by its
/// path, with its SHA-256.
fn the_rest(app: &Path, project: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    files_under(app, &mut files);
    files
        .iter()
        .filter(|f| !f.starts_with(project))
        .map(|f| {
            (
                f.strip_prefix(app).unwrap().to_string_lossy().into_owned(),
                gaply_core::contracts::ContentHash::of_bytes(&std::fs::read(f).unwrap())
                    .as_hex()
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn a_rerun_puts_no_sentinel_anywhere_but_the_projects_artifacts_and_its_title() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let w = World::new("rerun-matrix");
    let app = w.root();
    std::fs::create_dir_all(&app).unwrap();
    let _db = Database::open(&app.join("gaply.db")).expect("a migrated database");
    let folder = w.dir.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join(format!("{FILE}.csv"));
    std::fs::write(&data, table()).unwrap();

    let mut responses: Vec<(String, String)> = Vec::new();
    let e = w.engine();
    let opened = desc::open(&e.datasets, &data).expect("open");
    let held = e.datasets.get(opened.slot).unwrap();
    let spec = stats_desc::spec(
        OperationId(1),
        held.dataset(),
        DescribeParams {
            columns: vec![
                ColumnRequest {
                    alias: Alias(ColumnAlias(1)),
                    treat_as: TreatAs::Numeric,
                },
                ColumnRequest {
                    alias: Alias(ColumnAlias(2)),
                    treat_as: TreatAs::Categorical,
                },
            ],
            missing_codes: vec![],
        },
    );
    drop(held);
    let run = desc::run(&e.runs, &e.datasets, opened.slot, spec)
        .unwrap()
        .run_id;
    w.wait(run);
    let saved = pc::save_run(
        &app,
        &w.ws,
        run,
        SaveTarget::NewProject {
            name: TITLE.to_string(),
        },
    )
    .expect("saved");
    let id = saved.project_id.clone();
    let project = app.join("projects").join(&id);
    let before = the_rest(&app, &project);
    assert!(before.contains_key("gaply.db"), "{before:?}");

    let started = pc::rerun(&app, &w.ws, 1).expect("the rerun starts");
    responses.push(("rerun".into(), serde_json::to_string(&started).unwrap()));
    let start = std::time::Instant::now();
    let current = loop {
        let c = pc::current(&w.ws).unwrap();
        if c.rerun
            .as_ref()
            .is_some_and(|r| r.status != RerunProgress::Going)
        {
            break c;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(current.rerun.as_ref().unwrap().status, RerunProgress::Saved);
    responses.push(("current".into(), serde_json::to_string(&current).unwrap()));
    responses.push((
        "runs".into(),
        serde_json::to_string(&pc::runs(&app, &w.ws, &id).unwrap()).unwrap(),
    ));
    let mut results = String::new();
    for n in [1, 2] {
        results.push_str(
            &serde_json::to_string(&pc::run_result(&app, &w.ws, &id, n).unwrap()).unwrap(),
        );
    }
    let report = pc::report(&app, &w.ws, &id).unwrap();
    let prov = pc::provenance(&app, &w.ws, &id).unwrap();
    let report = String::from_utf8(report.bytes).unwrap();
    let prov = String::from_utf8(prov.bytes).unwrap();
    assert!(
        report.contains("Repeats run 1."),
        "the rerun is in the report"
    );
    assert!(prov.contains("wasInformedBy"), "and in the provenance");

    // Responses: none, but a result read back (the header and the label) and
    // the report's title (the name).
    for (name, text) in &responses {
        assert_eq!(found(text.as_bytes()), Vec::<&str>::new(), "{name}");
    }
    let in_results = found(results.as_bytes());
    assert!(in_results.contains(&HDR) && in_results.contains(&LVL));
    for s in &in_results {
        assert!([HDR, LVL].contains(s), "{s} in a result read back");
    }
    assert_eq!(found(report.as_bytes()), [TITLE], "the report's title only");
    assert_eq!(report.matches(TITLE).count(), 1);
    assert_eq!(found(prov.as_bytes()), Vec::<&str>::new(), "the provenance");

    // Files: the data's sentinels only under artifacts/, the name only in
    // project.db, the rest of the folder as it was.
    pc::close(&app, &w.ws).unwrap();
    let artifacts = project.join("artifacts");
    let mut files = Vec::new();
    files_under(&app, &mut files);
    let (mut in_artifacts, mut in_db) = (Vec::new(), Vec::new());
    for f in &files {
        let hits = found(&std::fs::read(f).unwrap());
        if f.starts_with(&artifacts) {
            in_artifacts.extend(hits.iter().copied());
            assert!(!hits.contains(&TITLE), "the name in {}", f.display());
        } else if f.parent() == Some(project.as_path())
            && f.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("project.db")
        {
            in_db.extend(hits.iter().copied());
            assert!(
                hits.iter().all(|s| *s == TITLE),
                "{hits:?} in {}",
                f.display()
            );
        } else {
            assert!(hits.is_empty(), "{hits:?} in {}", f.display());
        }
        for s in [FILE, PATH] {
            assert!(!hits.contains(&s), "{s} in {}", f.display());
        }
    }
    for s in [HDR, LVL, TXT, SEL_NUM, UNSEL_NUM] {
        assert!(in_artifacts.contains(&s), "the copy of the data holds {s}");
    }
    assert!(in_db.contains(&TITLE), "the name is in project.title");
    assert_eq!(
        the_rest(&app, &project),
        before,
        "the rest of the app's folder is byte for byte as it was"
    );

    // Tracing: every line, from every thread, the worker's included.
    let traced = lines.lock().unwrap().clone();
    assert!(
        traced.iter().any(|l| l.contains("project: rerun saved")),
        "the save at the end's event was captured: {} lines",
        traced.len()
    );
    for l in &traced {
        assert_eq!(found(l.as_bytes()), Vec::<&str>::new(), "a log line: {l}");
    }

    // Deleted: no file in the folder holds any.
    pc::open(&app, &w.ws, &id).unwrap();
    let c = pc::prepare_delete(&app, &w.ws, &id).unwrap();
    pc::delete(&app, &w.ws, &id, Some(&c.confirmation)).unwrap();
    let mut left = Vec::new();
    files_under(&app, &mut left);
    for f in &left {
        assert!(
            found(&std::fs::read(f).unwrap()).is_empty(),
            "{} after the delete",
            f.display()
        );
    }
    println!(
        "lines: {}; files before the delete: {}; after: {}",
        traced.len(),
        files.len(),
        left.len()
    );
}
