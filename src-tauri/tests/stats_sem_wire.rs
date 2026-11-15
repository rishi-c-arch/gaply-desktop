//! **The structural equation models' wire contract, from the Rust side (Research
//! OS, Phase 9C-4, decision 803), in the pattern of `stats_cfa_wire.rs`.** Each
//! view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which the screen's types will be tested against in 9C-5. Between
//! them the fixtures fill every field and every variant of every enum the result
//! carries: a value and a not-computable code, a loading and a path, a variance
//! and an error variance, a factor and a column as a variable, effects given.
//! Both result fixtures are real results of `compute` on synthetic data; the views
//! around them and the manifest are a wire shape. The Other fixture is one run
//! with a loop and a Heywood case: a loop between two columns, each with a column
//! of its own pointing into it, beside one factor of three columns whose first
//! column's error variance is below 0 (`cfa_cases.rs`'s construction), with a row
//! left out by each reason; so its notes are `negative_variance` and
//! `nonrecursive`.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type).
//!
//! Also: `semCodes.json` is exactly `SemCode::ALL`, `RunCode::ALL`,
//! `WorkspaceCode::ALL` and `CapabilityCode::ALL`, sorted; `semLimits.json` holds
//! the core's limits; and the structural equation models run under the run
//! manager's ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_SEM_WIRE` set, read, and then
//! pinned.

mod desc_support;
mod sem_support;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_sem_commands::{SemResultView, SemRunStateView, SemValidationView};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Os, ParamKey, ParamValue, Platform, Reproducibility, ResourceEstimate,
    RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_sem::{capability_id, SemCode, SemOutcome, SemParams};
use gaply_core::Database;
use sem_support::{app, factors, sspec, until_terminal_sem};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn writing() -> bool {
    std::env::var_os("GAPLY_WRITE_SEM_WIRE").is_some()
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if writing() {
        std::fs::write(fixture(name), &got).unwrap();
    }
    let want = std::fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}; the view is:\n{got}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

/// Decision 803: the screen's limits are the core's constants, written with the
/// other fixtures and compared on every run.
#[test]
fn the_screens_limits_are_the_cores() {
    use gaply_core::stats_sem as s;
    let want = json!({
        "minColumns": s::MIN_COLUMNS,
        "maxColumns": s::MAX_COLUMNS,
        "maxFactors": s::MAX_FACTORS,
        "minFactorItems": s::MIN_FACTOR_ITEMS,
        "maxPaths": s::MAX_PATHS,
        "maxCovariances": s::MAX_COVARIANCES,
        "maxEqualMembers": s::MAX_EQUAL_MEMBERS,
    });
    let path = screens().join("fixtures/semLimits.json");
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
    }
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("semLimits.json")).unwrap();
    assert_eq!(file, want, "semLimits.json and stats_sem's limits differ");
    assert_eq!(
        (s::MIN_COLUMNS, s::MAX_COLUMNS, s::MAX_FACTORS, s::MIN_FACTOR_ITEMS, s::MAX_PATHS, s::MAX_COVARIANCES, s::MAX_EQUAL_MEMBERS),
        (2, 128, 16, 2, 64, 256, 128),
        "decision 797"
    );
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OUTPUT: &str = "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b";

fn suspect_warning() -> Diagnostic<SemCode> {
    Diagnostic::warning(SemCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 3)
}

fn dataset(headers: &[&str], rows: &[Vec<String>]) -> gaply_core::stats_l1::dataset::Dataset {
    use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "sem wire", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// cfa_r_golden.rs's integer formula.
fn item(i: u64, j: u64) -> i64 {
    let f = (j - 1) % 2;
    let score = ((i * (7907 + 104729 * f)) % 1013) as i64 - 506;
    let shared = ((i * 2654435761) % 1009) as i64 - 504;
    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
    (2 + (j % 3) as i64) * (score + shared) + noise
}

fn params(v: Value) -> SemParams {
    serde_json::from_value(v).unwrap()
}

/// Six columns on two factors over 40 rows and a row left out (missing in C2): C1,
/// C3, C5 and C2, C4, C6, the second regressed on the first.
fn outcome() -> SemOutcome {
    let mut rows: Vec<Vec<String>> = (1..=40u64).map(|i| (1..=6u64).map(|j| item(i, j).to_string()).collect()).collect();
    rows.push(vec!["1".into(), String::new(), "3".into(), "4".into(), "5".into(), "6".into()]);
    let d = dataset(&["a", "b", "c", "d", "e", "f"], &rows);
    let p = params(json!({
        "indicators": [["C1", "C3", "C5"], ["C2", "C4", "C6"]],
        "paths": [{ "to": { "factor": 2 }, "from": { "factor": 1 } }],
        "covariances": [], "equal": [], "missingCodes": [],
    }));
    gaply_core::stats_sem::compute(&d, &p).expect("computes")
}

/// A loop between C1 and C2, C3 pointing into C1 and C4 into C2, the two causes
/// covarying; beside it one factor of C5, C6 and C7, x1 = u + w / 10, x2 = u + v,
/// x3 = u − v (cfa_cases.rs's Heywood case), covarying with C3 and with a second factor of
/// C8, C9 and C10. 60 rows, a row left out by the code
/// −9 and one by a missing value.
fn other_outcome() -> SemOutcome {
    let wave = |i: u64, m: u64| ((i * m) % 1013) as f64 - 506.0;
    let mut rows: Vec<Vec<String>> = (1..=60u64)
        .map(|i| {
            let (c3, c4) = (wave(i, 6007), wave(i, 9001));
            let (e1, e2) = (0.5 * wave(i, 3011), 0.5 * wave(i, 4001));
            // C1 = 0.4 C2 + 0.6 C3 + e1, C2 = 0.3 C1 + 0.5 C4 + e2, solved.
            let r1 = 0.6 * c3 + e1;
            let r2 = 0.5 * c4 + e2;
            let c1 = (r1 + 0.4 * r2) / (1.0 - 0.12);
            let c2 = 0.3 * c1 + r2;
            let (u, v, w) = (wave(i, 7907), 0.894 * wave(i, 104729), wave(i, 15485863));
            let t = 0.6 * u + wave(i, 2003);
            [c1, c2, c3, c4, u + 0.1 * w, u + v, u - v, t + 0.3 * wave(i, 5003), 0.8 * t + 0.3 * wave(i, 6011), 1.2 * t + 0.3 * wave(i, 7001)]
                .iter()
                .map(|x| format!("{x:?}"))
                .collect()
        })
        .collect();
    let mut coded = vec!["1".to_string(); 10];
    coded[0] = "-9".into();
    rows.push(coded);
    let mut holed = vec!["1".to_string(); 10];
    holed[1] = String::new();
    rows.push(holed);
    let d = dataset(&["a", "b", "c", "d", "x1", "x2", "x3", "y1", "y2", "y3"], &rows);
    let p = params(json!({
        "indicators": [["C5", "C6", "C7"], ["C8", "C9", "C10"]],
        "paths": [
            { "to": { "column": "C1" }, "from": { "column": "C2" } },
            { "to": { "column": "C1" }, "from": { "column": "C3" } },
            { "to": { "column": "C2" }, "from": { "column": "C1" } },
            { "to": { "column": "C2" }, "from": { "column": "C4" } },
        ],
        "covariances": [
            { "a": { "factor": 1 }, "b": { "factor": 2 } },
            { "a": { "factor": 1 }, "b": { "column": "C3" } },
            { "a": { "column": "C3" }, "b": { "column": "C4" } },
        ],
        "equal": [], "missingCodes": [-9.0],
    }));
    gaply_core::stats_sem::compute(&d, &p).expect("computes")
}

fn manifest() -> RunManifest<SemCode> {
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: 1,
        engine_version: 1,
        inputs: vec![hash(INPUT)],
        parameters_hash: hash(PARAMS),
        seed: Some(7),
        rows_read: 41,
        rows_used: 40,
        exclusions: outcome().exclusions,
        platform: Platform { os: Os::Macos, arch: Arch::Aarch64 },
        elapsed_ms: 3,
        output_hash: hash(OUTPUT),
        reproducibility: Reproducibility::Bitwise,
        preparation: None,
    }
}

fn views() -> (SemValidationView, SemRunStateView, SemResultView) {
    (
        SemValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![
                Diagnostic::error(SemCode::ItemRepeated).int(ParamKey::Column, 1),
                Diagnostic::error(SemCode::EqualUnknownParameter).int(ParamKey::Index, 1).int(ParamKey::Value, 2),
                Diagnostic::error(SemCode::ModelNotIdentified).int(ParamKey::Count, 1),
            ],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::error(SemCode::NoCompleteRows)],
            estimated_resources: ResourceEstimate { rows: 41, columns: 6, cells: 246 },
        },
        SemRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 7,
            steps_total: 11,
            diagnostics: vec![suspect_warning()],
        },
        SemResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![suspect_warning(), Diagnostic::note(SemCode::MlOnly)],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("semValidationView.json", &v);
    assert_bytes("semRunStateView.json", &s);
    assert_bytes("semResultView.json", &r);
    let other = SemResultView { result: other_outcome(), ..views().2 };
    assert_bytes("semResultViewOther.json", &other);
    // Every variant of the result is in a fixture: a screen is tested on each
    // shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "stats.sem.fixed_parameter",
        "stats.sem.negative_variance",
        "stats.sem.nonrecursive",
        "stats.sem.missing_value",
        "stats.sem.missing_code",
        "\"kind\":\"loading\"",
        "\"kind\":\"path\"",
        "\"error\":true",
        "\"error\":false",
        "{\"factor\":1}",
        "{\"column\":\"C1\"}",
        "\"effects\":[{",
        "\"covariances\":[]",
        "\"covariances\":[{",
        "{\"a\":{\"factor\":1}",
    ] {
        assert!(text.contains(shape), "{shape} missing from the fixtures");
    }
    assert_eq!(other.result.notes, [SemCode::NegativeVariance, SemCode::Nonrecursive], "the Other fixture's premise");
}

/* ---- the live views ---------------------------------------------------------- */

fn layout(v: &Value, at: &str, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                layout(x, &format!("{at}.{k}"), out);
            }
        }
        Value::Array(a) => {
            for x in a {
                layout(x, &format!("{at}[]"), out);
            }
        }
        Value::Null => {
            out.insert(format!("{at}:null"));
        }
        Value::Bool(_) => {
            out.insert(format!("{at}:bool"));
        }
        Value::Number(_) => {
            out.insert(format!("{at}:number"));
        }
        Value::String(_) => {
            out.insert(format!("{at}:string"));
        }
    }
}

fn layout_of(v: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    layout(v, "", &mut out);
    out
}

fn fixture_json(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixture(name)).unwrap()).unwrap()
}

/// A live path is in the fixtures, or is null where a fixture has the same path
/// with a value (a nullable field: the manifest's seed).
fn covered(want: &BTreeSet<String>, path: &str) -> bool {
    want.contains(path)
        || path.strip_suffix(":null").is_some_and(|at| want.iter().any(|w| w.starts_with(at) && w[at.len()..].starts_with(['.', '[', ':'])))
}

#[test]
fn a_live_run_gives_views_laid_out_as_the_fixtures() {
    let dir = TempDir::new("sem-wire");
    let headers = ["a", "b", "c", "d", "e", "f"];
    let cells: Vec<Vec<String>> = (1..=30u64).map(|i| (1..=6u64).map(|j| item(i, j).to_string()).collect()).collect();
    let rows: Vec<Vec<C>> = cells.iter().map(|r| r.iter().map(|c| C::N(c.as_str())).collect()).collect();
    let data = dir.write("data.csv", &delimited(&headers, &rows, ",", "\n"));
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (_app, w) = app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    let o = call(&w, "stats_desc_open", json!({ "path": data.to_string_lossy() })).unwrap();
    let v = call(&w, "stats_sem_validate", json!({ "slot": o["slot"], "spec": sspec(&o, 1, factors(&[&["C1", "C1", "C2"]], true, &[])) })).unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("semValidationView.json"));
    for path in layout_of(&v) {
        assert!(want.contains(&path), "the live validation has {path}, the fixture does not");
    }
    let mut want = layout_of(&fixture_json("semResultView.json"));
    want.extend(layout_of(&fixture_json("semResultViewOther.json")));
    let state = layout_of(&fixture_json("semRunStateView.json"));
    let mut reached = BTreeSet::new();
    let specs = [
        sspec(&o, 2, factors(&[&["C1", "C3", "C5"], &["C2", "C4", "C6"]], true, &[])),
        sspec(&o, 3, json!({ "indicators": [["C1", "C3", "C5"], ["C2", "C4", "C6"]], "paths": [{ "to": { "factor": 2 }, "from": { "factor": 1 } }], "covariances": [], "equal": [], "missingCodes": [] })),
        sspec(&o, 4, factors(&[&["C1", "C3", "C5"], &["C2", "C4", "C6"]], false, &[])),
    ];
    for spec in specs {
        let run = call(&w, "stats_sem_run", json!({ "slot": o["slot"], "spec": spec })).unwrap()["runId"].as_u64().unwrap();
        let s = until_terminal_sem(&w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        for path in layout_of(&s) {
            assert!(state.contains(&path), "the live state view has {path}, the fixture does not");
        }
        let r = call(&w, "stats_sem_result", json!({ "runId": run })).unwrap();
        for path in layout_of(&r) {
            assert!(covered(&want, &path), "the live result has {path}, the fixtures do not");
            reached.insert(path);
        }
    }
    // Not vacuous: the live results reach into every part of the result.
    for path in [
        ".result.weights[].estimate.value:number",
        ".result.weights[].se.not_computable:string",
        ".result.weights[].kind:string",
        ".result.weights[].to.column:string",
        ".result.weights[].from.factor:number",
        ".result.covariances[].correlation.value:number",
        ".result.variances[].error:bool",
        ".result.smc[].value.value:number",
        ".result.effects[].indirect.value:number",
        ".result.fit.model.rmseaHi90.value:number",
        ".result.fit.saturated.rmsea.not_computable:string",
        ".result.fit.saturated.gfi.value:number",
        ".result.fit.independence.cfi.value:number",
        ".result.attempt:number",
        ".manifest.outputHash:string",
    ] {
        assert!(reached.contains(path), "{path}");
    }
}

/* ---- the code list and the ceiling --------------------------------------------- */

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_sem_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("semCodes.json");
    let mut rust: Vec<String> = SemCode::ALL.iter().map(|c| c.wire().to_string()).collect();
    assert_eq!(
        rust.len(),
        73,
        "decision 799's codes: 8 of the input and codes; 33 of decision 797's checks (the shape 6, the paths 6, \
         the covariances 4, the groups held equal 7, the columns 6, the identification 1, the rows 3); the suspect \
         code; 3 of the run's own refusals; 9 not computable (overflow among them, also a refusal); 2 notes; 2 \
         exclusions; and 15 limitations"
    );
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&path).expect("semCodes.json")).unwrap();
    assert_eq!(sorted(listed), rust, "semCodes.json and the Rust closed sets differ");
}

#[test]
fn the_sem_runs_under_the_run_managers_ceiling() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    let entry = registry.entries().iter().find(|e| e.descriptor.id == capability_id()).expect("registered");
    assert_eq!(entry.descriptor.resource_profile.ceiling_ms, Some(app_lib::runs::RUN_CEILING_MS));
}
