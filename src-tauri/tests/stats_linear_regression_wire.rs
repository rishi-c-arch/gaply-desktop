//! **The linear regression's wire contract, from the Rust side (Research OS,
//! Phase 9A-1, decision 674), in the pattern of `stats_related_ranks_wire.rs`.**
//! Each view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which the screen's types are tested against (linearRegressionTypes.vitest.ts,
//! Phase 9A-2, decision 684). Each fixture
//! fills every field, and between them every variant of every enum the result
//! carries: a value and a not-computable code. Both result fixtures are real
//! results of `compute` on synthetic data (the review of 9A-1, finding 3), so a
//! stored checker that tests the result's identities accepts them; the views
//! around them and the manifest are a wire shape.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type), so the fixtures are what a screen will be
//! sent.
//!
//! Also: `linearRegressionCodes.json`, the code list a screen's wording will be
//! keyed by, is exactly `LinearRegressionCode::ALL`, `RunCode::ALL`,
//! `WorkspaceCode::ALL` and `CapabilityCode::ALL`, sorted; the screen's predictor
//! limit is the core's; and the linear regression runs under the run manager's
//! ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_LINEAR_REGRESSION_WIRE` set, read,
//! and then pinned.

mod desc_support;
mod linear_regression_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_linear_regression_commands::{
    LinearRegressionResultView, LinearRegressionRunStateView, LinearRegressionValidationView,
};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_linear_regression::{capability_id, LinearRegressionCode, LinearRegressionOutcome};
use gaply_core::Database;
use linear_regression_support::{app, lrspec, until_terminal_regression};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn writing() -> bool {
    std::env::var_os("GAPLY_WRITE_LINEAR_REGRESSION_WIRE").is_some()
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if writing() {
        std::fs::write(fixture(name), &got).unwrap();
    }
    let want = std::fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}; the view is:\n{got}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

/// Decision 674: the screen's predictor limit is the core's one constant, written
/// with the other fixtures and compared on every run, as `relatedRanksLimits.json`
/// is for the related rank tests'.
#[test]
fn the_screens_predictor_limit_is_the_cores() {
    let want = json!({ "maxPredictors": gaply_core::stats_linear_regression::MAX_PREDICTORS });
    let path = screens().join("fixtures/linearRegressionLimits.json");
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
    }
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("linearRegressionLimits.json")).unwrap();
    assert_eq!(file, want, "linearRegressionLimits.json and stats_linear_regression::MAX_PREDICTORS differ");
    assert_eq!(gaply_core::stats_linear_regression::MAX_PREDICTORS, 50, "decision 669");
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OUTPUT: &str = "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b";

fn suspect_warning() -> Diagnostic<LinearRegressionCode> {
    Diagnostic::warning(LinearRegressionCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 3)
}

fn exclusion(code: LinearRegressionCode, column: i64, count: u64) -> Exclusion<LinearRegressionCode> {
    Exclusion {
        code,
        count,
        params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(column))]),
    }
}

fn a(c: u32) -> Alias {
    Alias(ColumnAlias(c))
}

fn dataset(headers: &[&str], rows: &[Vec<&str>]) -> gaply_core::stats_l1::dataset::Dataset {
    use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        ),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "linear regression wire", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// Three predictors, every value computed, a row left out (missing in C2): a real
/// result of `stats_linear_regression::compute` on synthetic data, so every
/// identity a stored checker may test holds in it (the review of 9A-1, finding 3).
fn outcome() -> LinearRegressionOutcome {
    let d = dataset(
        &["y", "x1", "x2", "x3"],
        &[
            vec!["7.1", "1.2", "0.5", "10"],
            vec!["8.4", "2.5", "-1.25", "12"],
            vec!["9.9", "3.1", "2", "9.5"],
            vec!["12.2", "4.8", "0.75", "14"],
            vec!["11.6", "5", "-0.5", "11"],
            vec!["13.9", "6.3", "1.5", "13.5"],
            vec!["16.3", "7.7", "-2", "15"],
            vec!["14.1", "8.1", "0.25", "10.5"],
            vec!["17.8", "9.4", "1", "16"],
            vec!["19.5", "10.2", "-0.75", "12.5"],
            vec!["5", "", "1", "2"],
        ],
    );
    let r = gaply_core::stats_linear_regression::compute(
        &d,
        &gaply_core::stats_linear_regression::LinearRegressionParams { outcome: a(1), predictors: vec![a(2), a(3), a(4)], missing_codes: vec![] },
    )
    .expect("computes");
    assert!(!serde_json::to_string(&r).unwrap().contains("not_computable"), "every value computed");
    r
}

/// A perfect fit from data (x = 0, 3, 4 and y = 2, 11, 14), with a row left out by
/// a code: `perfect_fit` wherever decision 669 puts it, and the note. The other two
/// codes, `distribution_out_of_range` and `overflow`, have the same wire shape
/// (`{"not_computable": code}`); the first is not reachable through a dataset
/// (df at most 99,998, measured to 1e7), the second only at extreme scales.
fn other_outcome() -> LinearRegressionOutcome {
    let d = dataset(&["x", "y"], &[vec!["0", "2"], vec!["3", "11"], vec!["4", "14"], vec!["-9", "5"], vec!["-9", "7"]]);
    gaply_core::stats_linear_regression::compute(
        &d,
        &gaply_core::stats_linear_regression::LinearRegressionParams { outcome: a(2), predictors: vec![a(1)], missing_codes: vec![-9.0] },
    )
    .expect("computes")
}

fn manifest() -> RunManifest<LinearRegressionCode> {
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: 1,
        engine_version: 1,
        inputs: vec![hash(INPUT)],
        parameters_hash: hash(PARAMS),
        seed: Some(7),
        rows_read: 21,
        rows_used: 10,
        exclusions: outcome().exclusions,
        platform: Platform { os: Os::Macos, arch: Arch::Aarch64 },
        elapsed_ms: 3,
        output_hash: hash(OUTPUT),
        reproducibility: Reproducibility::Bitwise,
        preparation: None,
    }
}

fn views() -> (LinearRegressionValidationView, LinearRegressionRunStateView, LinearRegressionResultView) {
    (
        LinearRegressionValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![
                Diagnostic::error(LinearRegressionCode::ColumnRepeated).int(ParamKey::Column, 1),
                Diagnostic::error(LinearRegressionCode::TooFewRows).int(ParamKey::Count, 3).int(ParamKey::Limit, 4),
            ],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::error(LinearRegressionCode::NoCompleteRows)],
            estimated_resources: ResourceEstimate { rows: 21, columns: 4, cells: 84 },
        },
        LinearRegressionRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 0,
            steps_total: 1,
            diagnostics: vec![suspect_warning()],
        },
        LinearRegressionResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![suspect_warning(), Diagnostic::note(LinearRegressionCode::EnterOnly)],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("linearRegressionValidationView.json", &v);
    assert_bytes("linearRegressionRunStateView.json", &s);
    assert_bytes("linearRegressionResultView.json", &r);
    let other = LinearRegressionResultView { result: other_outcome(), ..views().2 };
    assert_bytes("linearRegressionResultViewOther.json", &other);
    // Every variant of the result is in a fixture: a screen is tested on each
    // shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "stats.linear_regression.perfect_fit",
        "stats.linear_regression.essentially_perfect_fit",
        "stats.linear_regression.missing_value",
        "stats.linear_regression.missing_code",
    ] {
        assert!(text.contains(shape), "{shape} missing from the fixtures");
    }
}

/* ---- the live views ---------------------------------------------------------- */

/// A JSON value's layout: every leaf path with its JSON type, an array's
/// elements under `[]`.
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

#[test]
fn a_live_run_gives_views_laid_out_as_the_fixtures() {
    let dir = TempDir::new("linear-regression-wire");
    let headers = ["y", "z", "w"];
    let mut rows: Vec<Vec<C>> = Vec::new();
    for (y, z, w) in [
        ("1.5", "2", "3"),
        ("2.5", "1.75", "9"),
        ("3.5", "5", "4"),
        ("4.5", "6.5", "8"),
        ("5.5", "2", "7"),
        ("6.5", "13", "5"),
        ("7.5", "8.75", "2"),
    ] {
        rows.push(vec![C::N(y), C::N(z), C::N(w)]);
    }
    rows.push(vec![C::Empty, C::N("3"), C::N("2")]);
    rows.push(vec![C::N("8"), C::Empty, C::N("3")]);
    let data = dir.write("data.csv", &delimited(&headers, &rows, ",", "\n"));
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (_app, w) = app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    let o = call(&w, "stats_desc_open", json!({ "path": data.to_string_lossy() })).unwrap();
    // A refused validation: one column twice.
    let v = call(&w, "stats_linear_regression_validate", json!({ "slot": o["slot"], "spec": lrspec(&o, 1, "C1", &["C1"], &[]) })).unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("linearRegressionValidationView.json"));
    for path in layout_of(&v) {
        assert!(want.contains(&path), "the live validation has {path}, the fixture does not");
    }
    let run = call(&w, "stats_linear_regression_run", json!({ "slot": o["slot"], "spec": lrspec(&o, 2, "C1", &["C2", "C3"], &[]) }))
        .unwrap()["runId"]
        .as_u64()
        .unwrap();
    let s = until_terminal_regression(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    let want = layout_of(&fixture_json("linearRegressionRunStateView.json"));
    for path in layout_of(&s) {
        assert!(want.contains(&path), "the live state view has {path}, the fixture does not");
    }
    let r = call(&w, "stats_linear_regression_result", json!({ "runId": run })).unwrap();
    let mut want = layout_of(&fixture_json("linearRegressionResultView.json"));
    want.extend(layout_of(&fixture_json("linearRegressionResultViewOther.json")));
    let got = layout_of(&r);
    for path in &got {
        // A nullable field (the manifest's seed) is null on a live run and
        // filled in the fixture: the same path, with a value.
        let nullable = path
            .strip_suffix(":null")
            .is_some_and(|at| want.iter().any(|w| w.rsplit_once(':').map(|(p, _)| p) == Some(at)));
        assert!(want.contains(path) || nullable, "the live result has {path}, the fixture does not");
    }
    // Not vacuous: the live result's layout reaches into the columns, the model,
    // the coefficients and the manifest.
    assert!(got.len() >= 60, "the layout reads into the result: {}", got.len());
    for path in [
        ".result.columns[].column:string",
        ".result.predictors[].vif.value:number",
        ".result.anova.residual.df:number",
        ".result.residuals.standardizedResidual.maximum.value:number",
        ".manifest.outputHash:string",
    ] {
        assert!(got.contains(path), "{path}: {got:?}");
    }
    // A perfect fit through IPC: the Other fixture's not-computable shape, live.
    let exact = dir.write("exact.csv", &delimited(&["x", "y"], &[vec![C::N("0"), C::N("2")], vec![C::N("3"), C::N("11")], vec![C::N("4"), C::N("14")]], ",", "\n"));
    let e = call(&w, "stats_desc_open", json!({ "path": exact.to_string_lossy() })).unwrap();
    let run = call(&w, "stats_linear_regression_run", json!({ "slot": e["slot"], "spec": lrspec(&e, 3, "C2", &["C1"], &[]) }))
        .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal_regression(&w, run)["state"], "succeeded");
    let r = call(&w, "stats_linear_regression_result", json!({ "runId": run })).unwrap();
    let got = layout_of(&r);
    for path in &got {
        let nullable = path
            .strip_suffix(":null")
            .is_some_and(|at| want.iter().any(|w| w.rsplit_once(':').map(|(p, _)| p) == Some(at)));
        assert!(want.contains(path) || nullable, "the live perfect fit has {path}, the fixtures do not");
    }
    assert!(got.contains(".result.anova.f.not_computable:string"), "{got:?}");
    assert_eq!(r["result"]["anova"]["f"]["not_computable"], "stats.linear_regression.perfect_fit");
}

/* ---- the code list and the ceiling --------------------------------------------- */

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_linear_regression_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("linearRegressionCodes.json");
    let mut rust: Vec<String> = LinearRegressionCode::ALL.iter().map(|c| c.wire().to_string()).collect();
    assert_eq!(rust.len(), 38, "the linear regression's 38 codes (36, and decision 683's too_small and predicted_no_spread)");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&path).expect("linearRegressionCodes.json")).unwrap();
    assert_eq!(sorted(listed), rust, "linearRegressionCodes.json and the Rust closed sets differ");
}

#[test]
fn the_linear_regression_runs_under_the_run_managers_ceiling() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    let entry = registry.entries().iter().find(|e| e.descriptor.id == capability_id()).expect("registered");
    assert_eq!(entry.descriptor.resource_profile.ceiling_ms, Some(app_lib::runs::RUN_CEILING_MS));
}
