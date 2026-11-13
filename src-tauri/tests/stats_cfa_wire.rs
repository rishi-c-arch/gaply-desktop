//! **The confirmatory factor analysis's wire contract, from the Rust side
//! (Research OS, Phase 9C-1, decision 749), in the pattern of
//! `stats_factor_wire.rs`.** Each view serialises to exactly the bytes of its
//! fixture in `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a
//! final newline), which 9C-2's screen types will be tested against. Between them
//! the fixtures fill every field and every variant of every enum the result
//! carries: a value and a not-computable code, the factors' covariances given and
//! empty. Both result fixtures are real results of `compute` on synthetic data;
//! the views around them and the manifest are a wire shape. The Other fixture is a
//! Heywood case at 0 degrees of freedom: one factor of three columns, the first
//! one's error variance below 0, so both notes and `df_zero`'s codes are given,
//! with a row left out by each reason.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type).
//!
//! Also: `cfaCodes.json` is exactly `CfaCode::ALL`, `RunCode::ALL`,
//! `WorkspaceCode::ALL` and `CapabilityCode::ALL`, sorted; `cfaLimits.json` holds
//! the core's `MAX_ITEMS` and `MAX_FACTORS`; and the confirmatory factor analysis
//! runs under the run manager's ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_CFA_WIRE` set, read, and then
//! pinned.

mod cfa_support;
mod desc_support;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_cfa_commands::{CfaResultView, CfaRunStateView, CfaValidationView};
use cfa_support::{app, cspec, until_terminal_cfa};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Os, ParamKey, ParamValue, Platform, Reproducibility, ResourceEstimate,
    RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_cfa::{capability_id, CfaCode, CfaOutcome, CfaParams};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn writing() -> bool {
    std::env::var_os("GAPLY_WRITE_CFA_WIRE").is_some()
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if writing() {
        std::fs::write(fixture(name), &got).unwrap();
    }
    let want = std::fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}; the view is:\n{got}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

/// Decision 749: the screen's limits are the core's constants, written with the
/// other fixtures and compared on every run.
#[test]
fn the_screens_limits_are_the_cores() {
    let want = json!({ "maxItems": gaply_core::stats_cfa::MAX_ITEMS, "maxFactors": gaply_core::stats_cfa::MAX_FACTORS });
    let path = screens().join("fixtures/cfaLimits.json");
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
    }
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("cfaLimits.json")).unwrap();
    assert_eq!(file, want, "cfaLimits.json and stats_cfa's limits differ");
    assert_eq!((gaply_core::stats_cfa::MAX_ITEMS, gaply_core::stats_cfa::MAX_FACTORS), (128, 16), "decision 745");
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OUTPUT: &str = "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b";

fn suspect_warning() -> Diagnostic<CfaCode> {
    Diagnostic::warning(CfaCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 3)
}

fn a(c: u32) -> Alias {
    Alias(ColumnAlias(c))
}

fn dataset(headers: &[&str], rows: &[Vec<String>]) -> gaply_core::stats_l1::dataset::Dataset {
    use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "cfa wire", TableRef::FirstTable),
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

/// Six columns on two factors over 40 rows and a row left out (missing in C2):
/// two correlated factors, C1, C3, C5 and C2, C4, C6. Every value computed but
/// the fixed loadings' standard errors, C.R.s and Ps.
fn outcome() -> CfaOutcome {
    let mut rows: Vec<Vec<String>> = (1..=40u64).map(|i| (1..=6u64).map(|j| item(i, j).to_string()).collect()).collect();
    rows.push(vec!["1".into(), String::new(), "3".into(), "4".into(), "5".into(), "6".into()]);
    let d = dataset(&["a", "b", "c", "d", "e", "f"], &rows);
    let p = CfaParams { indicators: vec![vec![a(1), a(3), a(5)], vec![a(2), a(4), a(6)]], correlated_factors: true, missing_codes: vec![] };
    let r = gaply_core::stats_cfa::compute(&d, &p).expect("computes");
    let text = serde_json::to_string(&r).unwrap();
    assert_eq!(text.matches("not_computable").count(), text.matches("stats.cfa.fixed_parameter").count(), "every value computed but the fixed loadings'");
    r
}

/// One factor of three columns, x1 = u + w / 10, x2 = u + v, x3 = u − v (as
/// cfa_cases.rs's Heywood case), 60 rows, and a row left out by a code and one by
/// a missing value: the first column's error variance is below 0 and the model
/// has 0 degrees of freedom.
fn other_outcome() -> CfaOutcome {
    let wave = |i: u64, m: u64| ((i * m) % 1013) as f64 - 506.0;
    let mut rows: Vec<Vec<String>> = (1..=60u64)
        .map(|i| {
            let (u, v, w) = (wave(i, 7907), 0.894 * wave(i, 104729), wave(i, 15485863));
            vec![format!("{:?}", u + 0.1 * w), format!("{:?}", u + v), format!("{:?}", u - v)]
        })
        .collect();
    rows.push(vec!["-9".into(), "1".into(), "1".into()]);
    rows.push(vec!["1".into(), String::new(), "1".into()]);
    let d = dataset(&["x1", "x2", "x3"], &rows);
    let p = CfaParams { indicators: vec![vec![a(1), a(2), a(3)]], correlated_factors: true, missing_codes: vec![-9.0] };
    gaply_core::stats_cfa::compute(&d, &p).expect("computes")
}

fn manifest() -> RunManifest<CfaCode> {
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

fn views() -> (CfaValidationView, CfaRunStateView, CfaResultView) {
    (
        CfaValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![
                Diagnostic::error(CfaCode::ItemRepeated).int(ParamKey::Column, 1),
                Diagnostic::error(CfaCode::TooFewRows).int(ParamKey::Count, 2).int(ParamKey::Limit, 4),
            ],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::error(CfaCode::NoCompleteRows)],
            estimated_resources: ResourceEstimate { rows: 41, columns: 6, cells: 246 },
        },
        CfaRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 7,
            steps_total: 9,
            diagnostics: vec![suspect_warning()],
        },
        CfaResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![suspect_warning(), Diagnostic::note(CfaCode::MlOnly)],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("cfaValidationView.json", &v);
    assert_bytes("cfaRunStateView.json", &s);
    assert_bytes("cfaResultView.json", &r);
    let other = CfaResultView { result: other_outcome(), ..views().2 };
    assert_bytes("cfaResultViewOther.json", &other);
    // Every variant of the result is in a fixture: a screen is tested on each
    // shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "stats.cfa.fixed_parameter",
        "stats.cfa.df_zero",
        "stats.cfa.negative_variance",
        "stats.cfa.missing_value",
        "stats.cfa.missing_code",
        "\"factorCovariances\":[]",
        "\"factorCovariances\":[{",
        "\"factors\":[1,2]",
    ] {
        assert!(text.contains(shape), "{shape} missing from the fixtures");
    }
    assert_eq!(other.result.notes, [CfaCode::NegativeVariance, CfaCode::DfZero], "the Other fixture's premise");
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
    let dir = TempDir::new("cfa-wire");
    let headers = ["a", "b", "c", "d", "e", "f"];
    let cells: Vec<Vec<String>> = (1..=30u64).map(|i| (1..=6u64).map(|j| item(i, j).to_string()).collect()).collect();
    let rows: Vec<Vec<C>> = cells.iter().map(|r| r.iter().map(|c| C::N(c.as_str())).collect()).collect();
    let data = dir.write("data.csv", &delimited(&headers, &rows, ",", "\n"));
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (_app, w) = app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    let o = call(&w, "stats_desc_open", json!({ "path": data.to_string_lossy() })).unwrap();
    let v = call(&w, "stats_cfa_validate", json!({ "slot": o["slot"], "spec": cspec(&o, 1, &[&["C1", "C1", "C2"]], true, &[]) })).unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("cfaValidationView.json"));
    for path in layout_of(&v) {
        assert!(want.contains(&path), "the live validation has {path}, the fixture does not");
    }
    let mut want = layout_of(&fixture_json("cfaResultView.json"));
    want.extend(layout_of(&fixture_json("cfaResultViewOther.json")));
    let state = layout_of(&fixture_json("cfaRunStateView.json"));
    let mut reached = BTreeSet::new();
    let specs = [
        cspec(&o, 2, &[&["C1", "C3", "C5"], &["C2", "C4", "C6"]], true, &[]),
        cspec(&o, 3, &[&["C1", "C3", "C5"], &["C2", "C4", "C6"]], false, &[]),
        cspec(&o, 4, &[&["C1", "C3", "C5"]], true, &[]),
    ];
    for spec in specs {
        let run = call(&w, "stats_cfa_run", json!({ "slot": o["slot"], "spec": spec })).unwrap()["runId"].as_u64().unwrap();
        let s = until_terminal_cfa(&w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        for path in layout_of(&s) {
            assert!(state.contains(&path), "the live state view has {path}, the fixture does not");
        }
        let r = call(&w, "stats_cfa_result", json!({ "runId": run })).unwrap();
        for path in layout_of(&r) {
            assert!(covered(&want, &path), "the live result has {path}, the fixtures do not");
            reached.insert(path);
        }
    }
    // Not vacuous: the live results reach into every part of the result.
    for path in [
        ".result.loadings[].estimate.value:number",
        ".result.loadings[].se.not_computable:string",
        ".result.factorCovariances[].correlation.value:number",
        ".result.errorVariances[].se.value:number",
        ".result.smc[].value.value:number",
        ".result.fit.rmseaHi90.value:number",
        ".result.fit.rmsea.not_computable:string",
        ".result.fit.df:number",
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
fn the_cfa_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("cfaCodes.json");
    let mut rust: Vec<String> = CfaCode::ALL.iter().map(|c| c.wire().to_string()).collect();
    assert_eq!(
        rust.len(),
        48,
        "decision 745's codes: 27 checks and refusals with the suspect code, 8 not computable, the factor \
         covariance's note, 2 exclusions and 10 limitations"
    );
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&path).expect("cfaCodes.json")).unwrap();
    assert_eq!(sorted(listed), rust, "cfaCodes.json and the Rust closed sets differ");
}

#[test]
fn the_cfa_runs_under_the_run_managers_ceiling() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    let entry = registry.entries().iter().find(|e| e.descriptor.id == capability_id()).expect("registered");
    assert_eq!(entry.descriptor.resource_profile.ceiling_ms, Some(app_lib::runs::RUN_CEILING_MS));
}
