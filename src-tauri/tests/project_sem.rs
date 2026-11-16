//! **Structural equation models in a project (Research OS, Phase 9C-5, decisions 816 and 829).**
//! Through the commands' plain bodies, as their Tauri wrappers call them, over a root and a
//! workspace session the test makes, on `project_cfa.rs`'s pattern: Wheaton's Model B's shape
//! on six columns of 300 rows with a row holding a code ticked as missing; Wheaton's Model C's
//! shape on the same data; Example 7's shape (a loop of two columns, four exogenous columns
//! with their six covariances and the loop's error covariance) on a second table; and a column
//! on its own equal to the sum of two others in the model with a path from it, a failed run.
//! Saved into one project; the project closed and opened; each run read back, the three
//! results run again and the rerun `Identical`, and the report's lines as parameter_line
//! writes them: each Factor, Path, Covariance and Held equal line in order, and the codes'
//! count. Every file is synthetic, seeded and made here, in a folder the test creates and
//! removes; no window reaches the method until 9C-5e, so `World::sem` starts each run.
//!
//! Each model was run on the Mac before this test was written (16 Nov 2026, macOS aarch64,
//! ~/gaply-corpus/os/phase9/9c5c/p2_find_project_sem_1794807835.log): Model B attempt 1 in 12
//! iterations, df 5; Model C attempt 1 in 15, df 8; both with the coded row left out (300 of
//! 301 rows) and no note; Example 7's shape attempt 1 in 14, df 2, the note `nonrecursive`;
//! the fourth failed `sample_covariance_singular` at column 7. Each df by hand: 21 moments
//! less 16 parameters (Model B), less 13 with C's three equalities (Model C), less 19 (Example
//! 7's six paths, seven covariances and six variances). A first fourth model, two factors of
//! two columns with a path from C7 to the first, was refused `model_not_identified` before it
//! ran, and so could not be saved; it was replaced by the one below.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::project_store::{ArtifactInput, ProjectRoot, RunToSave, SaveRequest};
use app_lib::runs::RunKind;
use gaply_core::contracts::{CapabilityId, ContentHash, OperationSpec, RunState};
use gaply_core::research_project::{ArtifactHash, ArtifactKind, DataFormat, RerunComparison, UsedRole};
use gaply_core::stats_sem::{self, SemParams};
use serde_json::{json, Value};

use project_support::*;

/// A seeded stream of standard normals: xorshift64*, each normal the sum of twelve uniforms
/// less 6, so no platform function is used and every platform draws the same numbers.
struct Draws(u64);

impl Draws {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        (0..12).map(|_| self.uniform()).sum::<f64>() - 6.0
    }
}

/// Three decimals, as a file would hold them.
fn cell(x: f64) -> String {
    format!("{:.3}", x)
}

/// Wheaton's shape, seed 20261116, 300 rows: ses (factor 3) on C5 and C6; alien67 (factor 1)
/// on C1 and C2, from ses; alien71 (factor 2) on C3 and C4, from alien67 and ses; C1's and
/// C3's errors correlated; the powles loadings equal and the anomia and powles error variances
/// pairwise equal, as Model C holds them. A seventh column, C7, is C5 + C6 exactly. A 301st
/// row holds -99 in C1, which a run that ticks -99 leaves out.
fn wheaton_data() -> Vec<u8> {
    let mut r = Draws(20261116);
    let mut s = String::from("anomia67,powles67,anomia71,powles71,educatio,sei,ses_sum\n");
    for _ in 0..300 {
        let ses = r.normal();
        let alien67 = -0.6 * ses + 0.8 * r.normal();
        let alien71 = 0.6 * alien67 - 0.2 * ses + 0.7 * r.normal();
        let shared = 0.4 * r.normal();
        let c1 = alien67 + 0.6 * r.normal() + shared;
        let c2 = 0.9 * alien67 + 0.6 * r.normal();
        let c3 = alien71 + 0.6 * r.normal() + shared;
        let c4 = 0.9 * alien71 + 0.6 * r.normal();
        let c5 = ses + 0.5 * r.normal();
        let c6 = 0.8 * ses + 0.6 * r.normal();
        let row = [c1, c2, c3, c4, c5, c6].map(cell);
        let sum = row[4].parse::<f64>().unwrap() + row[5].parse::<f64>().unwrap();
        s.push_str(&format!("{},{}\n", row.join(","), cell(sum)));
    }
    s.push_str("-99,0.5,0.5,0.5,0.5,0.5,1.000\n");
    s.into_bytes()
}

/// Example 7's shape, seed 20261117, 300 rows: C1 from C2 and C3, C2 from C1, C4, C5 and C6
/// (a loop of two columns), C3 to C6 correlated through a shared draw, and the loop's two
/// errors correlated.
fn example_7_data() -> Vec<u8> {
    let mut r = Draws(20261117);
    let mut s = String::from("academic,attract,gpa,height,weight,rating\n");
    for _ in 0..300 {
        let common = r.normal();
        let x: [f64; 4] = [0.5, 0.6, 0.4, 0.3].map(|l| l * common + r.normal());
        let shared = 0.5 * r.normal();
        let (e1, e2) = (r.normal() + shared, r.normal() + shared);
        // y1 = 0.3 y2 + 0.5 x3 + e1; y2 = 0.2 y1 + 0.4 x4 − 0.3 x5 + 0.2 x6 + e2, solved.
        let (a, b) = (0.5 * x[0] + e1, 0.4 * x[1] - 0.3 * x[2] + 0.2 * x[3] + e2);
        let det = 1.0 - 0.3 * 0.2;
        let y1 = (a + 0.3 * b) / det;
        let y2 = (b + 0.2 * a) / det;
        s.push_str(&[y1, y2, x[0], x[1], x[2], x[3]].map(cell).join(","));
        s.push('\n');
    }
    s.into_bytes()
}

fn params(v: Value) -> SemParams {
    serde_json::from_value(v).expect("a SEM spec")
}

const WHEATON_PATHS: &str = r#"[{"to":{"factor":1},"from":{"factor":3}},{"to":{"factor":2},"from":{"factor":1}},{"to":{"factor":2},"from":{"factor":3}}]"#;

fn model_b() -> SemParams {
    params(json!({
        "indicators": [["C1", "C2"], ["C3", "C4"], ["C5", "C6"]],
        "paths": serde_json::from_str::<Value>(WHEATON_PATHS).unwrap(),
        "covariances": [{ "a": { "column": "C1" }, "b": { "column": "C3" } }],
        "equal": [],
        "missingCodes": [-99.0],
    }))
}

fn model_c() -> SemParams {
    params(json!({
        "indicators": [["C1", "C2"], ["C3", "C4"], ["C5", "C6"]],
        "paths": serde_json::from_str::<Value>(WHEATON_PATHS).unwrap(),
        "covariances": [{ "a": { "column": "C1" }, "b": { "column": "C3" } }],
        "equal": [
            [{ "loading": { "factor": 1, "column": "C2" } }, { "loading": { "factor": 2, "column": "C4" } }],
            [{ "variance": { "variable": { "column": "C1" } } }, { "variance": { "variable": { "column": "C3" } } }],
            [{ "variance": { "variable": { "column": "C2" } } }, { "variance": { "variable": { "column": "C4" } } }]
        ],
        "missingCodes": [-99.0],
    }))
}

fn example_7() -> SemParams {
    let col = |c: u32| json!({ "column": format!("C{c}") });
    let paths: Vec<Value> = [(1, 2), (1, 3), (2, 1), (2, 4), (2, 5), (2, 6)].iter().map(|(t, f)| json!({ "to": col(*t), "from": col(*f) })).collect();
    let covs: Vec<Value> = [(1, 2), (3, 4), (3, 5), (3, 6), (4, 5), (4, 6), (5, 6)].iter().map(|(a, b)| json!({ "a": col(*a), "b": col(*b) })).collect();
    params(json!({ "indicators": [], "paths": paths, "covariances": covs, "equal": [], "missingCodes": [] }))
}

/// C7 is C5 + C6, all three in the model: one factor on C1, C2 and C3, with paths to it from
/// C5, C6 and C7, three columns on their own, and their three covariances.
fn singular() -> SemParams {
    params(json!({
        "indicators": [["C1", "C2", "C3"]],
        "paths": [
            { "to": { "factor": 1 }, "from": { "column": "C5" } },
            { "to": { "factor": 1 }, "from": { "column": "C6" } },
            { "to": { "factor": 1 }, "from": { "column": "C7" } }
        ],
        "covariances": [
            { "a": { "column": "C5" }, "b": { "column": "C6" } },
            { "a": { "column": "C5" }, "b": { "column": "C7" } },
            { "a": { "column": "C6" }, "b": { "column": "C7" } }
        ],
        "equal": [],
        "missingCodes": [-99.0],
    }))
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

/// The report's Settings block of each run, in stored-run order: the lines `parameter_line`
/// writes, read from the project's report itself (decision 829).
fn settings_blocks(report: &str) -> Vec<Vec<String>> {
    report
        .split("\n### Settings\n\n")
        .skip(1)
        .map(|rest| {
            let block = rest.split("\n### ").next().unwrap();
            block.lines().filter(|l| l.starts_with("- ")).map(str::to_string).collect()
        })
        .collect()
}

/// The lines of a block that only the factor-based methods and the SEM write, and the codes'
/// count: what decision 816 checks in order.
fn model_lines(block: &[String]) -> Vec<&str> {
    block
        .iter()
        .map(String::as_str)
        .filter(|l| {
            ["- Factor ", "- Path: ", "- Covariance: ", "- Held equal, ", "- Missing-value codes: "].iter().any(|p| l.starts_with(p))
        })
        .collect()
}

const MODEL_B: [&str; 8] = [
    "- Factor 1: columns 1, 2, by position",
    "- Factor 2: columns 3, 4, by position",
    "- Factor 3: columns 5, 6, by position",
    "- Path: factor 3 → factor 1",
    "- Path: factor 1 → factor 2",
    "- Path: factor 3 → factor 2",
    "- Covariance: column 1 ↔ column 3",
    "- Missing-value codes: 1 (their values are in the settings fingerprint)",
];

const MODEL_C: [&str; 11] = [
    "- Factor 1: columns 1, 2, by position",
    "- Factor 2: columns 3, 4, by position",
    "- Factor 3: columns 5, 6, by position",
    "- Path: factor 3 → factor 1",
    "- Path: factor 1 → factor 2",
    "- Path: factor 3 → factor 2",
    "- Covariance: column 1 ↔ column 3",
    "- Held equal, group 1 (loadings): factor 1 → column 2; factor 2 → column 4",
    "- Held equal, group 2 (variances): column 1; column 3",
    "- Held equal, group 3 (variances): column 2; column 4",
    "- Missing-value codes: 1 (their values are in the settings fingerprint)",
];

const EXAMPLE_7: [&str; 14] = [
    "- Path: column 2 → column 1",
    "- Path: column 3 → column 1",
    "- Path: column 1 → column 2",
    "- Path: column 4 → column 2",
    "- Path: column 5 → column 2",
    "- Path: column 6 → column 2",
    "- Covariance: column 1 ↔ column 2",
    "- Covariance: column 3 ↔ column 4",
    "- Covariance: column 3 ↔ column 5",
    "- Covariance: column 3 ↔ column 6",
    "- Covariance: column 4 ↔ column 5",
    "- Covariance: column 4 ↔ column 6",
    "- Covariance: column 5 ↔ column 6",
    "- Missing-value codes: 0 (their values are in the settings fingerprint)",
];

const SINGULAR: [&str; 8] = [
    "- Factor 1: columns 1, 2, 3, by position",
    "- Path: column 5 → factor 1",
    "- Path: column 6 → factor 1",
    "- Path: column 7 → factor 1",
    "- Covariance: column 5 ↔ column 6",
    "- Covariance: column 5 ↔ column 7",
    "- Covariance: column 6 ↔ column 7",
    "- Missing-value codes: 1 (their values are in the settings fingerprint)",
];

#[test]
fn structural_equation_models_are_saved_read_back_run_again_identical_and_reported() {
    let w = World::new("9c5c-project");
    let wheaton = w.open("wheaton.csv", &wheaton_data());
    let felson = w.open("felson.csv", &example_7_data());
    let b = w.sem(wheaton, model_b());
    let c = w.sem(wheaton, model_c());
    let loop7 = w.sem(felson, example_7());
    let sum = w.sem(wheaton, singular());
    for run in [b, c, loop7] {
        assert_eq!(w.wait_new_kind(run), RunState::Succeeded);
    }
    // The run's own refusal is recorded as failed.
    assert_eq!(w.wait_new_kind(sum), RunState::Failed);

    let id = pc::save_run(&w.root(), &w.ws, b, new_project("SEM")).expect("Model B saves").project_id;
    pc::save_run(&w.root(), &w.ws, c, into_open(&id)).expect("Model C saves");
    pc::save_run(&w.root(), &w.ws, loop7, into_open(&id)).expect("Example 7's shape saves");
    pc::save_run(&w.root(), &w.ws, sum, into_open(&id)).expect("the failed run saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    // Read back: each stored result is the model it was, with FIND FIRST's outcome.
    for (n, iterations, df, rows) in [(1, 12, 5, 300), (2, 15, 8, 300), (3, 14, 2, 300)] {
        let o = outputs(&w, &id, n);
        assert_eq!((&o["attempt"], &o["iterations"], &o["fit"]["model"]["df"], &o["rowsUsed"]), (&json!(1), &json!(iterations), &json!(df), &json!(rows)), "run {n}");
    }
    for n in [1, 2] {
        let o = outputs(&w, &id, n);
        assert_eq!(o["notes"], json!([]), "run {n}");
        assert_eq!(o["exclusions"], json!([{ "code": "stats.sem.missing_code", "count": 1, "params": { "column": 1 } }]), "run {n}: the coded row left out");
        assert_eq!(o["indicators"], json!([["C1", "C2"], ["C3", "C4"], ["C5", "C6"]]), "run {n}");
    }
    let third = outputs(&w, &id, 3);
    assert_eq!(third["notes"], json!(["stats.sem.nonrecursive"]), "the loop");
    assert_eq!(third["exclusions"], json!([]));
    assert_eq!(third["indicators"], json!([]));
    let back = pc::run_result(&w.root(), &w.ws, &id, 4).expect("read back");
    assert_eq!(back.state, RunState::Failed);
    assert!(back.bundle_json.is_none() && back.manifest_json.is_none());
    let failure: Value = serde_json::from_str(back.failure_json.as_deref().expect("its failure")).unwrap();
    assert_eq!(failure["failure"]["code"], "stats.sem.sample_covariance_singular");
    assert_eq!(failure["failure"]["params"]["column"], 7, "the sum, named by its alias number");

    for (stored, new) in [(1, 5), (2, 6), (3, 7)] {
        let started = pc::rerun(&w.root(), &w.ws, stored).expect("the rerun starts");
        assert_eq!(started.kind, RunKind::Sem);
        let r = settled(&w);
        assert_eq!((r.status, r.stored_run_id), (RerunProgress::Saved, Some(new)));
        assert_eq!(comparison(&w, &id, new), Some(RerunComparison::Identical), "run {stored} again");
    }
    // The failed run is not run again, by rerun_open's check 4 (decision 833, R3).
    let e = pc::rerun(&w.root(), &w.ws, 4).expect_err("a failed run is not run again");
    assert_eq!(code(&e), "project.rerun_not_succeeded");

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes).expect("UTF-8");
    for n in [1, 2, 3] {
        let sentence = format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    let blocks = settings_blocks(&r);
    assert_eq!(blocks.len(), 7, "seven runs' settings:\n{r}");
    let expected: [&[&str]; 7] = [&MODEL_B, &MODEL_C, &EXAMPLE_7, &SINGULAR, &MODEL_B, &MODEL_C, &EXAMPLE_7];
    for (n, (block, want)) in blocks.iter().zip(expected).enumerate() {
        assert_eq!(model_lines(block), want, "run {}'s lines, in order:\n{r}", n + 1);
        // And nothing else but the shape and the seed (the review of this step, finding 5: a
        // stray line of another method's, "- Factors correlated", say, would pass the filter).
        let others: Vec<&String> = block
            .iter()
            .filter(|l| !want.contains(&l.as_str()) && !l.starts_with("- Data file as read: ") && l.as_str() != "- Random seed: none")
            .collect();
        assert!(others.is_empty(), "run {}: lines no SEM writes: {others:?}", n + 1);
    }
    // No header reaches the report (a rough check, as project_cfa.rs's; the privacy matrix
    // checks headers with sentinels).
    for text in ["anomia67", "ses_sum", "academic", "rating"] {
        assert!(!r.contains(text), "{text} in the report");
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

/// Saves a Succeeded SEM row through the store, on a second handle, as `project_rerun.rs`'s
/// `store_a_row` does (that helper is its file's own): the CSV `data` and the spec's bytes, with
/// a test bundle and manifest. Its stored id.
fn store_a_row(w: &World, id: &str, data: &[u8], spec: &[u8]) -> u64 {
    let hash = |b: &[u8]| ArtifactHash::parse(ContentHash::of_bytes(b).as_hex()).unwrap();
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let mut h = store.open(id).unwrap();
    let (bundle, manifest) = (b"{\"test\":\"bundle\"}", b"{\"test\":\"manifest\"}");
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse(stats_sem::CAPABILITY_ID).unwrap(),
            capability_version: stats_sem::CAPABILITY_VERSION,
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
/// fits. Model B is saved; then two rows on the same data file: its own spec unchanged (the
/// control, which reruns), and the same spec with its third path's end moved from factor 3 to
/// C9, a column the seven-column file does not have, refused `project.rerun_refused` by the
/// SEM's `valid_on`. Made to pass every spec, `valid_on` lets the second start.
#[test]
fn a_rerun_refuses_a_spec_whose_path_end_the_data_file_does_not_have() {
    let w = World::new("9c5d-sem-refused");
    let bytes = wheaton_data();
    let slot = w.open("wheaton.csv", &bytes);
    let b = w.sem(slot, model_b());
    assert_eq!(w.wait_new_kind(b), RunState::Succeeded);
    let id = pc::save_run(&w.root(), &w.ws, b, new_project("SEM")).expect("Model B saves").project_id;
    let spec_json = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap().spec_json;
    let spec: OperationSpec<SemParams> = serde_json::from_str(&spec_json).unwrap();
    let canonical = serde_json::to_vec(&spec).unwrap();
    assert_eq!(canonical, spec_json.as_bytes(), "the premise: a Save stores canonical bytes");
    let header = String::from_utf8(bytes.clone()).unwrap().lines().next().unwrap().split(',').count();
    assert_eq!(header, 7, "the premise: the file has seven columns, so no C9");
    let mut moved = serde_json::to_value(&spec).unwrap();
    assert_eq!(moved["parameters"]["paths"][2], json!({ "to": { "factor": 2 }, "from": { "factor": 3 } }), "the premise: the third path");
    moved["parameters"]["paths"][2]["from"] = json!({ "column": "C9" });
    let moved: OperationSpec<SemParams> = serde_json::from_value(moved).unwrap();
    let moved = serde_json::to_vec(&moved).unwrap();

    // The control: the unchanged spec on a row stored the same way reruns.
    let control = store_a_row(&w, &id, &bytes, &canonical);
    pc::rerun(&w.root(), &w.ws, control).expect("the control reruns");
    assert_eq!(settled(&w).status, RerunProgress::Saved, "the control's rerun");

    let refused = store_a_row(&w, &id, &bytes, &moved);
    let e = pc::rerun(&w.root(), &w.ws, refused).expect_err("a path end the file does not have");
    assert_eq!(code(&e), "project.rerun_refused");
    assert_eq!(w.ws.projects().rerun_going(), None, "nothing is going");
}
