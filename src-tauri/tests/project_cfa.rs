//! **The confirmatory factor analysis in a project (Research OS, Phase 9C-3, decision 784).**
//! Through the commands' plain bodies, as their Tauri wrappers call them, over a root and a
//! workspace session the test makes, on `project_factor.rs`'s pattern: two correlated factors
//! of three columns with a row left out by a ticked code, one factor of four columns, three
//! uncorrelated factors of three columns, and one factor of two columns and their sum, a failed
//! run; saved; the project closed and opened; each run read back, the three results run again
//! and the rerun `Identical`, and the report's lines: each factor in order, whether the factors
//! are correlated where there are two or more, and the codes' count. Every file is synthetic
//! and in a folder the test creates and removes.
//!
//! Each model was run on the Mac before this test was written (14 Nov 2026,
//! ~/gaply-corpus/os/phase9/9c3/logs/p2_probe_models_*.log): 10, 11 and 2 iterations, df 8,
//! 2 and 27, the first and third with the coded row left out; the fourth failed
//! `sample_covariance_singular` at column 10.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::project_store::{ArtifactInput, ProjectRoot, RunToSave, SaveRequest};
use app_lib::runs::RunKind;
use gaply_core::contracts::{CapabilityId, ContentHash, OperationSpec, RunState};
use gaply_core::research_project::{ArtifactHash, ArtifactKind, DataFormat, RerunComparison, UsedRole};
use gaply_core::stats_cfa::{self, CfaParams};
use serde_json::{json, Value};

use project_support::*;

/// `cfa_cases.rs`'s structured formula: nine columns on three factors over 60 rows (column j
/// on factor (j − 1) mod 3), a tenth column `sum` equal to the first and the fourth, and a 61st
/// row holding -99 in the first column (5 elsewhere), which a run that ticks -99 leaves out.
fn data() -> Vec<u8> {
    let mut s = String::from("x1,x2,x3,x4,x5,x6,x7,x8,x9,sum\n");
    for i in 1..=60u64 {
        let row: Vec<i64> = (1..=9u64)
            .map(|j| {
                let f = (j - 1) % 3;
                let score = ((i * (7907 + 104729 * f)) % 1013) as i64 - 506;
                let shared = ((i * 2654435761) % 1009) as i64 - 504;
                let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
                (2 + (j % 3) as i64) * (score + shared) + noise
            })
            .collect();
        let line: Vec<String> = row.iter().map(i64::to_string).chain([(row[0] + row[3]).to_string()]).collect();
        s.push_str(&line.join(","));
        s.push('\n');
    }
    s.push_str("-99,5,5,5,5,5,5,5,5,5\n");
    s.into_bytes()
}

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

fn settled(w: &World) -> RerunView {
    let start = Instant::now();
    loop {
        if let Some(r) = pc::current(&w.ws).unwrap().rerun {
            if r.status != RerunProgress::Going {
                return r;
            }
        }
        assert!(start.elapsed() < Duration::from_secs(30), "the rerun never settled");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn comparison(w: &World, id: &str, n: u64) -> Option<RerunComparison> {
    pc::runs(&w.root(), &w.ws, id)
        .unwrap()
        .into_iter()
        .find(|r| r.stored_run_id == n)
        .expect("the stored run")
        .comparison
}

/// A stored result's primary outputs.
fn outputs(w: &World, id: &str, n: u64) -> Value {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    assert_eq!(back.state, RunState::Succeeded, "run {n}");
    let bundle: Value = serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    bundle["primaryOutputs"].clone()
}

#[test]
fn confirmatory_factor_analyses_are_saved_read_back_run_again_identical_and_reported() {
    let w = World::new("9c3-project");
    let slot = w.open("cfa.csv", &data());
    let two = w.cfa(slot, &[&[1, 4, 7], &[2, 5, 8]], true, vec![-99.0]);
    let one = w.cfa(slot, &[&[2, 3, 6, 9]], true, vec![]);
    let three = w.cfa(slot, &[&[1, 4, 7], &[2, 5, 8], &[3, 6, 9]], false, vec![-99.0]);
    let singular = w.cfa(slot, &[&[1, 4, 10]], true, vec![-99.0]);
    for run in [two, one, three] {
        assert_eq!(w.wait_new_kind(run), RunState::Succeeded);
    }
    // A run's own refusal is recorded as failed.
    assert_eq!(w.wait_new_kind(singular), RunState::Failed);

    let id = pc::save_run(&w.root(), &w.ws, two, new_project("CFA"))
        .expect("the two-factor run saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, one, into_open(&id)).expect("the one-factor run saves");
    pc::save_run(&w.root(), &w.ws, three, into_open(&id)).expect("the three-factor run saves");
    pc::save_run(&w.root(), &w.ws, singular, into_open(&id)).expect("the failed run saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    // Read back: each stored result is the analysis it was.
    let first = outputs(&w, &id, 1);
    assert_eq!(first["indicators"], serde_json::json!([["C1", "C4", "C7"], ["C2", "C5", "C8"]]));
    assert_eq!(first["factorCovariances"].as_array().unwrap().len(), 1);
    assert_eq!(first["fit"]["model"]["df"], 8);
    assert_eq!(first["rowsUsed"], 60);
    assert_eq!(first["exclusions"][0]["code"], "stats.cfa.missing_code");
    assert_eq!(first["exclusions"][0]["params"]["column"], 1);
    let second = outputs(&w, &id, 2);
    assert_eq!(second["indicators"], serde_json::json!([["C2", "C3", "C6", "C9"]]));
    assert_eq!(second["fit"]["model"]["df"], 2);
    assert_eq!(second["rowsUsed"], 61, "no code ticked, so the 61st row is used");
    let third = outputs(&w, &id, 3);
    assert_eq!(third["indicators"].as_array().unwrap().len(), 3);
    assert_eq!(third["factorCovariances"], serde_json::json!([]), "uncorrelated");
    assert_eq!(third["fit"]["model"]["df"], 27);
    let back = pc::run_result(&w.root(), &w.ws, &id, 4).expect("read back");
    assert_eq!(back.state, RunState::Failed);
    assert!(back.bundle_json.is_none() && back.manifest_json.is_none());
    let failure: Value = serde_json::from_str(back.failure_json.as_deref().expect("its failure")).unwrap();
    assert_eq!(failure["failure"]["code"], "stats.cfa.sample_covariance_singular");
    assert_eq!(failure["failure"]["params"]["column"], 10, "the sum, named by its position");

    for (stored, new) in [(1, 5), (2, 6), (3, 7)] {
        let started = pc::rerun(&w.root(), &w.ws, stored).expect("the rerun starts");
        assert_eq!(started.kind, RunKind::Cfa);
        let r = settled(&w);
        assert_eq!((r.status, r.stored_run_id), (RerunProgress::Saved, Some(new)));
        assert_eq!(comparison(&w, &id, new), Some(RerunComparison::Identical), "run {stored} again");
    }

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes).expect("UTF-8");
    for n in [1, 2, 3] {
        let sentence = format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for (line, times) in [
        ("- Factor 1: columns 1, 4, 7, by position", 4),
        ("- Factor 2: columns 2, 5, 8, by position", 4),
        ("- Factor 3: columns 3, 6, 9, by position", 2),
        ("- Factor 1: columns 2, 3, 6, 9, by position", 2),
        ("- Factor 1: columns 1, 4, 10, by position", 1),
        ("- Factors correlated: yes", 2),
        ("- Factors correlated: no", 2),
        ("- Missing-value codes: 1", 5),
        ("- Missing-value codes: 0", 2),
    ] {
        assert_eq!(r.matches(line).count(), times, "{line}:\n{r}");
    }
    // Each run's Factor lines come in order: the two-factor run's Factor 1 before its Factor 2.
    let a = r.find("- Factor 1: columns 1, 4, 7, by position").unwrap();
    let b = r.find("- Factor 2: columns 2, 5, 8, by position").unwrap();
    assert!(a < b, "the factors in order");
    // No header reaches the report (a rough check, as project_factor.rs's; the privacy matrix
    // checks headers with sentinels).
    for text in ["sum", "x1"] {
        assert!(!r.contains(&format!(" {text}\n")) && !r.contains(&format!(": {text}")), "{text} in the report");
    }
}

fn artifact(kind: ArtifactKind, format: DataFormat, bytes: &[u8]) -> ArtifactInput {
    ArtifactInput {
        kind,
        format,
        schema_version: 1,
        expected: ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).unwrap(),
        bytes: bytes.to_vec(),
    }
}

/// Saves a Succeeded CFA row through the store, on a second handle, as `project_rerun.rs`'s
/// `store_a_row` does (that helper is its file's own): the CSV `data` and the spec's bytes, with
/// a test bundle and manifest. Its stored id.
fn store_a_row(w: &World, id: &str, data: &[u8], spec: &[u8]) -> u64 {
    let hash = |b: &[u8]| ArtifactHash::parse(ContentHash::of_bytes(b).as_hex()).unwrap();
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let mut h = store.open(id).unwrap();
    let (bundle, manifest) = (b"{\"test\":\"bundle\"}", b"{\"test\":\"manifest\"}");
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse(stats_cfa::CAPABILITY_ID).unwrap(),
            capability_version: stats_cfa::CAPABILITY_VERSION,
            state: RunState::Succeeded,
            started_at: 1_792_396_340,
            ended_at: 1_792_396_341,
            used: vec![(hash(data), UsedRole::Data), (hash(spec), UsedRole::Spec)],
            generated: vec![hash(bundle), hash(manifest)],
            derived: Vec::new(),
        },
        artifacts: vec![
            artifact(ArtifactKind::DataFile, DataFormat::Csv, data),
            artifact(ArtifactKind::OperationSpec, DataFormat::Json, spec),
            artifact(ArtifactKind::ResultBundle, DataFormat::Json, bundle),
            artifact(ArtifactKind::RunManifest, DataFormat::Json, manifest),
        ],
    };
    h.save(req).expect("a test row").0
}

/// Decision 833 (R2; decision 832's finding 2): a rerun refuses a spec its data file no longer
/// fits. The two-factor run is saved; then two rows on the same data file: its own spec
/// unchanged (the control, which reruns), and the same spec with its first factor's last item
/// moved from C7 to C11, a column the ten-column file does not have, refused
/// `project.rerun_refused` by the CFA's `valid_on`. Made to pass every spec, `valid_on` lets the
/// second start.
#[test]
fn a_rerun_refuses_a_spec_whose_item_the_data_file_does_not_have() {
    let w = World::new("9c5d-cfa-refused");
    let bytes = data();
    let slot = w.open("cfa.csv", &bytes);
    let two = w.cfa(slot, &[&[1, 4, 7], &[2, 5, 8]], true, vec![-99.0]);
    assert_eq!(w.wait_new_kind(two), RunState::Succeeded);
    let id = pc::save_run(&w.root(), &w.ws, two, new_project("CFA")).expect("the run saves").project_id;
    let spec_json = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap().spec_json;
    let spec: OperationSpec<CfaParams> = serde_json::from_str(&spec_json).unwrap();
    let canonical = serde_json::to_vec(&spec).unwrap();
    assert_eq!(canonical, spec_json.as_bytes(), "the premise: a Save stores canonical bytes");
    let header = String::from_utf8(bytes.clone()).unwrap().lines().next().unwrap().split(',').count();
    assert_eq!(header, 10, "the premise: the file has ten columns, so no C11");
    let mut moved = serde_json::to_value(&spec).unwrap();
    assert_eq!(moved["parameters"]["indicators"][0], json!(["C1", "C4", "C7"]), "the premise: the first factor");
    moved["parameters"]["indicators"][0][2] = json!("C11");
    let moved: OperationSpec<CfaParams> = serde_json::from_value(moved).unwrap();
    let moved = serde_json::to_vec(&moved).unwrap();

    // The control: the unchanged spec on a row stored the same way reruns.
    let control = store_a_row(&w, &id, &bytes, &canonical);
    pc::rerun(&w.root(), &w.ws, control).expect("the control reruns");
    assert_eq!(settled(&w).status, RerunProgress::Saved, "the control's rerun");

    let refused = store_a_row(&w, &id, &bytes, &moved);
    let e = pc::rerun(&w.root(), &w.ws, refused).expect_err("an item the file does not have");
    assert_eq!(code(&e), "project.rerun_refused");
    assert_eq!(w.ws.projects().rerun_going(), None, "nothing is going");
}
