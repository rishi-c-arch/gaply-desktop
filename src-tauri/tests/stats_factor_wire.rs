//! **The factor analysis's wire contract, from the Rust side (Research OS, Phase
//! 9B-1, decision 704), in the pattern of `stats_linear_regression_wire.rs`.**
//! Each view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which 9B-2's screen types will be tested against. Between them the
//! fixtures fill every field, and every variant of every enum the result carries:
//! a value and a not-computable code, both rotations' fields. Both result
//! fixtures are real results of `compute` on synthetic data; the views around
//! them and the manifest are a wire shape. The Other fixture is a run with
//! not-computable values and a note: three exactly uncorrelated items and their
//! sum, so the correlations are singular and the second and third
//! eigenvalues tie.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type).
//!
//! Also: `factorCodes.json` is exactly `FactorCode::ALL`, `RunCode::ALL`,
//! `WorkspaceCode::ALL` and `CapabilityCode::ALL`, sorted; `factorLimits.json`'s
//! item limit is the core's `MAX_ITEMS`; and the factor analysis runs under the
//! run manager's ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_FACTOR_WIRE` set, read, and then
//! pinned.

mod desc_support;
mod factor_support;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_factor_commands::{FactorResultView, FactorRunStateView, FactorValidationView};
use desc_support::{call, delimited, TempDir, C};
use factor_support::{app, fspec, until_terminal_factor};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Os, ParamKey, ParamValue, Platform, Reproducibility, ResourceEstimate,
    RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_factor::{capability_id, Extraction, FactorCode, FactorOutcome, FactorParams, Rotation};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn writing() -> bool {
    std::env::var_os("GAPLY_WRITE_FACTOR_WIRE").is_some()
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if writing() {
        std::fs::write(fixture(name), &got).unwrap();
    }
    let want = std::fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}; the view is:\n{got}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

/// Decision 704: the screen's item limit is the core's one constant, written with
/// the other fixtures and compared on every run.
#[test]
fn the_screens_item_limit_is_the_cores() {
    let want = json!({ "maxItems": gaply_core::stats_factor::MAX_ITEMS });
    let path = screens().join("fixtures/factorLimits.json");
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
    }
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("factorLimits.json")).unwrap();
    assert_eq!(file, want, "factorLimits.json and stats_factor::MAX_ITEMS differ");
    assert_eq!(gaply_core::stats_factor::MAX_ITEMS, 128, "decision 700");
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OUTPUT: &str = "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b";

fn suspect_warning() -> Diagnostic<FactorCode> {
    Diagnostic::warning(FactorCode::SuspectMissingCode)
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
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "factor wire", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn params(p: u32, extraction: Extraction, factors: u32, rotation: Rotation, codes: Vec<f64>) -> FactorParams {
    FactorParams {
        items: (1..=p).map(a).collect(),
        extraction,
        factors: Some(factors),
        rotation,
        iterations: 25,
        convergence: 0.001,
        missing_codes: codes,
    }
}

/// Six items on two factors over 40 rows (an integer formula) and a row left out
/// (missing in C2): principal axis, promax, every value computed.
fn outcome() -> FactorOutcome {
    let mut rows: Vec<Vec<String>> = (1..=40u64)
        .map(|i| {
            (1..=6u64)
                .map(|j| {
                    let f = (j - 1) % 2;
                    let score = ((i * 7907 + f * 15485863) % 1013) as i64 - 506;
                    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
                    (2 * score + noise).to_string()
                })
                .collect()
        })
        .collect();
    rows.push(vec!["1".into(), String::new(), "3".into(), "4".into(), "5".into(), "6".into()]);
    let d = dataset(&["a", "b", "c", "d", "e", "f"], &rows);
    let r = gaply_core::stats_factor::compute(&d, &params(6, Extraction::PrincipalAxis, 2, Rotation::Promax, vec![])).expect("computes");
    assert!(!serde_json::to_string(&r).unwrap().contains("not_computable"), "every value computed");
    r
}

/// Three columns of a Hadamard matrix of order 8 and the sum of all three,
/// with a row left out by a code and one by a missing value: principal
/// components, two factors, varimax. The correlations are singular (the sum), so
/// the determinant, KMO, the MSAs and Bartlett's test are `correlations_singular`;
/// the eigenvalues are 2, 1, 1 and 0, so the second and third tie and the note
/// is given; every item loads on the first factor, so varimax runs.
fn other_outcome() -> FactorOutcome {
    let had = [[1, 1, 1], [-1, 1, 1], [1, -1, 1], [-1, -1, 1], [1, 1, -1], [-1, 1, -1], [1, -1, -1], [-1, -1, -1]];
    let mut rows: Vec<Vec<String>> = had.iter().map(|h| vec![h[0].to_string(), h[1].to_string(), h[2].to_string(), (h[0] + h[1] + h[2]).to_string()]).collect();
    rows.push(vec!["-9".into(), "1".into(), "1".into(), "0".into()]);
    rows.push(vec!["1".into(), String::new(), "1".into(), "0".into()]);
    let d = dataset(&["h1", "h2", "h3", "s"], &rows);
    gaply_core::stats_factor::compute(&d, &params(4, Extraction::PrincipalComponents, 2, Rotation::Varimax, vec![-9.0])).expect("computes")
}

fn manifest() -> RunManifest<FactorCode> {
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

fn views() -> (FactorValidationView, FactorRunStateView, FactorResultView) {
    (
        FactorValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![
                Diagnostic::error(FactorCode::ItemRepeated).int(ParamKey::Column, 1),
                Diagnostic::error(FactorCode::TooFewRows).int(ParamKey::Count, 2).int(ParamKey::Limit, 3),
            ],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::error(FactorCode::NoCompleteRows)],
            estimated_resources: ResourceEstimate { rows: 41, columns: 6, cells: 246 },
        },
        FactorRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 22,
            steps_total: 24,
            diagnostics: vec![suspect_warning()],
        },
        FactorResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![suspect_warning(), Diagnostic::note(FactorCode::NoMlOrOtherExtraction)],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("factorValidationView.json", &v);
    assert_bytes("factorRunStateView.json", &s);
    assert_bytes("factorResultView.json", &r);
    let other = FactorResultView { result: other_outcome(), ..views().2 };
    assert_bytes("factorResultViewOther.json", &other);
    // Every variant of the result is in a fixture: a screen is tested on each
    // shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "stats.factor.correlations_singular",
        "stats.factor.eigenvalues_not_distinct",
        "stats.factor.missing_value",
        "stats.factor.missing_code",
        "\"principal_axis\"",
        "\"principal_components\"",
        "\"promax\"",
        "\"varimax\"",
        "\"structure\":[[",
        "\"factorCorrelations\":[[",
        "\"transformation\":[[",
        "\"sums\":[{",
    ] {
        assert!(text.contains(shape), "{shape} missing from the fixtures");
    }
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
/// with a value (a nullable field: the manifest's seed, a rotation's fields).
fn covered(want: &BTreeSet<String>, path: &str) -> bool {
    want.contains(path)
        || path.strip_suffix(":null").is_some_and(|at| want.iter().any(|w| w.starts_with(at) && w[at.len()..].starts_with(['.', '[', ':'])))
}

#[test]
fn a_live_run_gives_views_laid_out_as_the_fixtures() {
    let dir = TempDir::new("factor-wire");
    let headers = ["a", "b", "c", "d"];
    let cells: Vec<Vec<String>> = (1..=20u64)
        .map(|i| (1..=4u64).map(|j| (((i * 7907 + ((j - 1) % 2) * 15485863) % 1013) as i64 * 2 + ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64).to_string()).collect())
        .collect();
    let rows: Vec<Vec<C>> = cells.iter().map(|r| r.iter().map(|c| C::N(c.as_str())).collect()).collect();
    let data = dir.write("data.csv", &delimited(&headers, &rows, ",", "\n"));
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (_app, w) = app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    let o = call(&w, "stats_desc_open", json!({ "path": data.to_string_lossy() })).unwrap();
    let v = call(&w, "stats_factor_validate", json!({ "slot": o["slot"], "spec": fspec(&o, 1, &["C1", "C1", "C2"], "principal_components", json!(null), "none", &[]) })).unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("factorValidationView.json"));
    for path in layout_of(&v) {
        assert!(want.contains(&path), "the live validation has {path}, the fixture does not");
    }
    let mut want = layout_of(&fixture_json("factorResultView.json"));
    want.extend(layout_of(&fixture_json("factorResultViewOther.json")));
    let state = layout_of(&fixture_json("factorRunStateView.json"));
    let mut reached = BTreeSet::new();
    for (op, extraction, factors, rotation) in [(2, "principal_components", json!(2), "promax"), (3, "principal_components", json!(2), "varimax"), (4, "principal_components", json!(1), "none")] {
        let run = call(&w, "stats_factor_run", json!({ "slot": o["slot"], "spec": fspec(&o, op, &["C1", "C2", "C3", "C4"], extraction, factors, rotation, &[]) }))
            .unwrap()["runId"]
            .as_u64()
            .unwrap();
        let s = until_terminal_factor(&w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        for path in layout_of(&s) {
            assert!(state.contains(&path), "the live state view has {path}, the fixture does not");
        }
        let r = call(&w, "stats_factor_result", json!({ "runId": run })).unwrap();
        for path in layout_of(&r) {
            assert!(covered(&want, &path), "the live result ({rotation}) has {path}, the fixtures do not");
            reached.insert(path);
        }
    }
    // Not vacuous: the live results reach into every part of the result.
    for path in [
        ".result.extraction.loadings[][].value:number",
        ".result.rotation.structure[][].value:number",
        ".result.rotation.transformation[][].value:number",
        ".result.rotation:null",
        ".result.kmo.items[].msa.value:number",
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
fn the_factor_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("factorCodes.json");
    let mut rust: Vec<String> = FactorCode::ALL.iter().map(|c| c.wire().to_string()).collect();
    assert_eq!(rust.len(), 48, "decision 700's codes: 39, and nine limitations");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&path).expect("factorCodes.json")).unwrap();
    assert_eq!(sorted(listed), rust, "factorCodes.json and the Rust closed sets differ");
}

#[test]
fn the_factor_analysis_runs_under_the_run_managers_ceiling() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    let entry = registry.entries().iter().find(|e| e.descriptor.id == capability_id()).expect("registered");
    assert_eq!(entry.descriptor.resource_profile.ceiling_ms, Some(app_lib::runs::RUN_CEILING_MS));
}
