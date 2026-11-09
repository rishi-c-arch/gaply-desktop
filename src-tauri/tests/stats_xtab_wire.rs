//! **Crosstabs' wire contract, from the Rust side (Research OS, Phase 8X-1,
//! decision 580), in the pattern of `stats_anova_wire.rs`.** Each view
//! serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which 8X-2's screen types will be tested against. Each fixture
//! fills every field, and between them every variant of every enum the
//! result carries: a value and a code, a computed test and one not
//! computable, a 2 x 2 table's continuity correction and Fisher's test and a
//! larger table's nulls. The values are a wire shape, not a computation.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type), so the fixtures are what a screen
//! will be sent.
//!
//! Also: `xtabCodes.json`, the code list a screen's wording will be keyed
//! by, is exactly `XtabCode::ALL`, `RunCode::ALL`, `WorkspaceCode::ALL` and
//! `CapabilityCode::ALL`, sorted; and crosstabs runs under the run manager's
//! ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_XTAB_WIRE` set, read, and then
//! pinned.

mod desc_support;
mod xtab_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_xtab_commands::{XtabResultView, XtabRunStateView, XtabValidationView};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_xtab::{
    capability_id, ChiOutcome, ChiTest, CrosstabsOutcome, FisherTest, Frequency, LevelLabel, Stat,
    XtabCode,
};
use gaply_core::Database;
use xtab_support::{app, until_terminal_xtab, xspec};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if std::env::var_os("GAPLY_WRITE_XTAB_WIRE").is_some() {
        std::fs::write(fixture(name), &got).unwrap();
    }
    let want = std::fs::read_to_string(fixture(name))
        .unwrap_or_else(|e| panic!("{name}: {e}; the view is:\n{got}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OUTPUT: &str = "6ad1bf5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5c5b";

fn suspect_warning() -> Diagnostic<XtabCode> {
    Diagnostic::warning(XtabCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(2))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 3)
}

fn exclusion(code: XtabCode, column: i64, count: u64) -> Exclusion<XtabCode> {
    Exclusion {
        code,
        count,
        params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(column))]),
    }
}

fn level(level: u32, label: &str) -> LevelLabel {
    LevelLabel {
        level,
        label: label.into(),
    }
}

fn frequency(row: u32, column: u32, observed: u64, expected: f64) -> Frequency {
    Frequency {
        row,
        column,
        observed,
        expected,
        row_proportion: 0.625,
        column_proportion: 0.5,
        total_proportion: 0.25,
        residual: observed as f64 - expected,
        pearson_residual: 0.25,
        adjusted_residual: -1.5,
    }
}

/// A 2 x 2 table: every field filled, the continuity correction and
/// Fisher's test present, a note, and every exclusion.
fn outcome() -> CrosstabsOutcome {
    CrosstabsOutcome {
        row_levels: vec![level(0, "no"), level(1, "yes")],
        column_levels: vec![level(0, "a"), level(1, "b")],
        frequencies: vec![
            frequency(0, 0, 5, 4.5),
            frequency(0, 1, 3, 3.5),
            frequency(1, 0, 4, 4.5),
            frequency(1, 1, 4, 3.5),
        ],
        row_totals: vec![8, 8],
        column_totals: vec![9, 7],
        n: 16,
        pearson: ChiOutcome::Computed(ChiTest {
            statistic: 0.254,
            df: 1.0,
            p: 0.614,
        }),
        likelihood_ratio: ChiOutcome::Computed(ChiTest {
            statistic: 0.255,
            df: 1.0,
            p: 0.613,
        }),
        continuity_corrected: Some(ChiOutcome::Computed(ChiTest {
            statistic: 0.0,
            df: 1.0,
            p: 1.0,
        })),
        fisher: Some(FisherTest {
            two_sided: 1.0,
            less: 0.807,
            greater: 0.5,
        }),
        phi: Stat::Value(0.126),
        cramers_v: Stat::Value(0.126),
        contingency_coefficient: Stat::Value(0.125),
        expected_below_five: 4,
        cell_count: 4,
        smallest_expected: 3.5,
        exclusions: vec![
            exclusion(XtabCode::MissingRow, 1, 2),
            exclusion(XtabCode::MissingCode, 1, 1),
            exclusion(XtabCode::MissingColumn, 2, 1),
            exclusion(XtabCode::MissingCode, 2, 3),
        ],
        notes: vec![XtabCode::ExpectedBelowFive],
        limitations: vec![
            XtabCode::NoLayers,
            XtabCode::FisherTwoByTwoOnly,
            XtabCode::NoFurtherNominalMeasures,
            XtabCode::NoWeights,
            XtabCode::CodePointOrder,
        ],
    }
}

/// A larger table's shape: no continuity correction or Fisher's test, and
/// a test and the measures not computable.
fn other_outcome() -> CrosstabsOutcome {
    CrosstabsOutcome {
        row_levels: vec![level(0, "a"), level(1, "b"), level(2, "c")],
        continuity_corrected: None,
        fisher: None,
        pearson: ChiOutcome::NotComputable(XtabCode::ArithmeticRefused),
        phi: Stat::NotComputable(XtabCode::ArithmeticRefused),
        cramers_v: Stat::NotComputable(XtabCode::ArithmeticRefused),
        contingency_coefficient: Stat::NotComputable(XtabCode::ArithmeticRefused),
        expected_below_five: 0,
        exclusions: Vec::new(),
        notes: Vec::new(),
        ..outcome()
    }
}

fn manifest() -> RunManifest<XtabCode> {
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: 1,
        engine_version: 1,
        inputs: vec![hash(INPUT)],
        parameters_hash: hash(PARAMS),
        seed: Some(7),
        rows_read: 22,
        rows_used: 16,
        exclusions: outcome().exclusions,
        platform: Platform {
            os: Os::Macos,
            arch: Arch::Aarch64,
        },
        elapsed_ms: 3,
        output_hash: hash(OUTPUT),
        reproducibility: Reproducibility::Bitwise,
        preparation: None,
    }
}

fn views() -> (XtabValidationView, XtabRunStateView, XtabResultView) {
    (
        XtabValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![Diagnostic::error(XtabCode::TooManyLevels)
                .with(ParamKey::Column, ParamValue::Int(1))
                .int(ParamKey::Count, 51)
                .int(ParamKey::Limit, 50)],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::error(XtabCode::SameColumn)],
            estimated_resources: ResourceEstimate {
                rows: 22,
                columns: 2,
                cells: 44,
            },
        },
        XtabRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 0,
            steps_total: 1,
            diagnostics: vec![suspect_warning()],
        },
        XtabResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![suspect_warning(), Diagnostic::note(XtabCode::NoLayers)],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("xtabValidationView.json", &v);
    assert_bytes("xtabRunStateView.json", &s);
    assert_bytes("xtabResultView.json", &r);
    let other = XtabResultView {
        result: other_outcome(),
        ..views().2
    };
    assert_bytes("xtabResultViewOther.json", &other);
    // Every variant of the result is in a fixture: a screen is tested on
    // each shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "\"computed\":",
        "\"continuityCorrected\":null",
        "\"fisher\":null",
        "\"twoSided\":",
        "stats.xtab.expected_below_five",
        "stats.xtab.missing_code",
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
    let dir = TempDir::new("xtab-wire");
    let headers = ["r", "c", "w"];
    let mut rows: Vec<Vec<C>> = Vec::new();
    for (r, c, n) in [("no", "a", 5), ("no", "b", 3), ("yes", "a", 4), ("yes", "b", 4)] {
        for _ in 0..n {
            rows.push(vec![C::T(r), C::T(c), C::N("1")]);
        }
    }
    rows.push(vec![C::Empty, C::T("a"), C::N("2")]);
    rows.push(vec![C::T("no"), C::Empty, C::N("3")]);
    let data = dir.write("data.csv", &delimited(&headers, &rows, ",", "\n"));
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (_app, w) = app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .unwrap();
    // A refused validation: one column twice.
    let v = call(
        &w,
        "stats_xtab_validate",
        json!({ "slot": o["slot"], "spec": xspec(&o, 1, "C1", "C1", &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("xtabValidationView.json"));
    for path in layout_of(&v) {
        assert!(
            want.contains(&path),
            "the live validation has {path}, the fixture does not"
        );
    }
    let run = call(
        &w,
        "stats_xtab_run",
        json!({ "slot": o["slot"], "spec": xspec(&o, 2, "C1", "C2", &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    let s = until_terminal_xtab(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    let want = layout_of(&fixture_json("xtabRunStateView.json"));
    for path in layout_of(&s) {
        assert!(
            want.contains(&path),
            "the live state view has {path}, the fixture does not"
        );
    }
    let r = call(&w, "stats_xtab_result", json!({ "runId": run })).unwrap();
    let mut want = layout_of(&fixture_json("xtabResultView.json"));
    want.extend(layout_of(&fixture_json("xtabResultViewOther.json")));
    let got = layout_of(&r);
    for path in &got {
        // A nullable field (the manifest's seed) is null on a live run and
        // filled in the fixture: the same path, with a value.
        let nullable = path.strip_suffix(":null").is_some_and(|at| {
            want.iter()
                .any(|w| w.rsplit_once(':').map(|(p, _)| p) == Some(at))
        });
        assert!(
            want.contains(path) || nullable,
            "the live result has {path}, the fixture does not"
        );
    }
    assert!(
        got.len() > 40,
        "the layout reads into the result: {}",
        got.len()
    );
}

/* ---- the code list and the ceiling --------------------------------------------- */

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_xtab_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("xtabCodes.json");
    let mut rust: Vec<String> = XtabCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(rust.len(), 26, "crosstabs' 26 codes");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if std::env::var_os("GAPLY_WRITE_XTAB_WIRE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("xtabCodes.json")).unwrap();
    assert_eq!(
        sorted(listed),
        rust,
        "xtabCodes.json and the Rust closed sets differ"
    );
}

#[test]
fn crosstabs_runs_under_the_run_managers_ceiling() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    let entry = registry
        .entries()
        .iter()
        .find(|e| e.descriptor.id == capability_id())
        .expect("registered");
    assert_eq!(
        entry.descriptor.resource_profile.ceiling_ms,
        Some(app_lib::runs::RUN_CEILING_MS)
    );
}
