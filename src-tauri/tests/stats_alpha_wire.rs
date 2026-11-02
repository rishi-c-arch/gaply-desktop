//! **Cronbach's alpha's wire contract, from the Rust side (Research OS,
//! Phase 8I-2, decision 381), in the pattern of `stats_corr_wire.rs`.**
//! Each view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which the screen's types are tested against. Between them the
//! fixtures fill every field and every variant of every enum the result
//! carries: a value and a code, a computed interval and one not computable,
//! a reversed item and one not, a note and none, an exclusion of each code.
//! The values are a wire shape, not a computation.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type), so the fixtures are what the screen
//! will be sent.
//!
//! Also: `alphaCodes.json`, the code list the screen's wording is keyed by,
//! is exactly `AlphaCode::ALL`, `RunCode::ALL`, `WorkspaceCode::ALL` and
//! `CapabilityCode::ALL`, sorted; and alpha runs under the run manager's
//! ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_ALPHA_WIRE` set, read, and then
//! pinned.

mod alpha_support;
mod desc_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use alpha_support::{app, aspec, until_terminal_alpha};
use app_lib::stats_alpha_commands::{AlphaResultView, AlphaRunStateView, AlphaValidationView};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_alpha::{
    capability_id, AlphaCode, AlphaOutcome, InterItem, Interval, ItemStats, Stat,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if std::env::var_os("GAPLY_WRITE_ALPHA_WIRE").is_some() {
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
const OUTPUT: &str = "9a3f0c5be2d14e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f";

fn c(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn suspect_warning() -> Diagnostic<AlphaCode> {
    Diagnostic::warning(AlphaCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(3))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 2)
}

fn column(n: i64) -> BTreeMap<ParamKey, ParamValue<AlphaCode>> {
    BTreeMap::from([(ParamKey::Column, ParamValue::Int(n))])
}

fn item(n: u32, reversed: bool, mean: f64) -> ItemStats {
    ItemStats {
        item: c(n),
        reversed,
        mean: Stat::Value(mean),
        sd: Stat::Value(1.25),
        scale_mean_if_deleted: Stat::Value(7.5),
        scale_variance_if_deleted: Stat::Value(4.75),
        corrected_item_total: Stat::Value(0.625),
        alpha_if_deleted: Stat::Value(0.75),
    }
}

const LIMITATIONS: [AlphaCode; 2] = [AlphaCode::AssumesOneDimension, AlphaCode::NoOmega];

/// Every field filled, and the enums' variants between this and
/// [`other_outcome`].
fn outcome() -> AlphaOutcome {
    AlphaOutcome {
        k: 3,
        rows_used: 9,
        alpha: Stat::Value(0.8125),
        interval: Interval::Computed {
            lower: 0.5,
            upper: 0.9375,
        },
        standardized: Stat::Value(0.8),
        inter_item: InterItem {
            mean: Stat::Value(0.5),
            min: Stat::Value(0.25),
            max: Stat::Value(0.75),
        },
        items: vec![
            item(1, false, 3.5),
            item(2, true, 2.25),
            item(4, false, 4.0),
        ],
        exclusions: vec![
            Exclusion {
                code: AlphaCode::MissingValue,
                count: 2,
                params: column(1),
            },
            Exclusion {
                code: AlphaCode::MissingCode,
                count: 1,
                params: column(4),
            },
        ],
        notes: vec![],
        limitations: LIMITATIONS.to_vec(),
    }
}

/// The other shapes: what is not computable, a negative alpha's note.
fn other_outcome() -> AlphaOutcome {
    let nc = Stat::NotComputable(AlphaCode::ItemConstant);
    AlphaOutcome {
        k: 2,
        rows_used: 5,
        alpha: Stat::Value(-0.5),
        interval: Interval::NotComputable(AlphaCode::DistributionOutOfRange),
        standardized: nc,
        inter_item: InterItem {
            mean: nc,
            min: nc,
            max: nc,
        },
        items: vec![
            ItemStats {
                corrected_item_total: nc,
                alpha_if_deleted: Stat::NotComputable(AlphaCode::TwoItems),
                sd: Stat::Value(0.0),
                ..item(1, false, 3.0)
            },
            ItemStats {
                alpha_if_deleted: Stat::NotComputable(AlphaCode::TwoItems),
                ..item(2, false, 2.5)
            },
        ],
        exclusions: vec![],
        notes: vec![AlphaCode::NegativeAlpha],
        limitations: LIMITATIONS.to_vec(),
    }
}

fn manifest() -> RunManifest<AlphaCode> {
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: 1,
        engine_version: 1,
        inputs: vec![hash(INPUT)],
        parameters_hash: hash(PARAMS),
        seed: Some(7),
        rows_read: 12,
        rows_used: 9,
        exclusions: vec![Exclusion {
            code: AlphaCode::MissingValue,
            count: 2,
            params: column(1),
        }],
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

fn views() -> (AlphaValidationView, AlphaRunStateView, AlphaResultView) {
    (
        AlphaValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![
                Diagnostic::error(AlphaCode::ReversedOutsideScale)
                    .with(ParamKey::Column, ParamValue::Int(2))
                    .int(ParamKey::Count, 3),
                Diagnostic::error(AlphaCode::ScaleMissing),
            ],
            warnings: vec![suspect_warning()],
            missing_information: vec![],
            estimated_resources: ResourceEstimate {
                rows: 12,
                columns: 3,
                cells: 36,
            },
        },
        AlphaRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 4,
            steps_total: 7,
            diagnostics: vec![suspect_warning()],
        },
        AlphaResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![suspect_warning()],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("alphaValidationView.json", &v);
    assert_bytes("alphaRunStateView.json", &s);
    assert_bytes("alphaResultView.json", &r);
    let other = AlphaResultView {
        result: other_outcome(),
        ..views().2
    };
    assert_bytes("alphaResultViewOther.json", &other);
    // Every variant is in one fixture or the other: the screen is tested on
    // each shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "\"computed\":",
        "\"reversed\":true",
        "\"reversed\":false",
        "\"notes\":[]",
        "\"notes\":[\"stats.alpha.negative_alpha\"]",
        "\"code\":\"stats.alpha.missing_value\"",
        "\"code\":\"stats.alpha.missing_code\"",
        "\"exclusions\":[]",
    ] {
        assert!(text.contains(shape), "{shape} missing from the fixtures");
    }
    assert!(
        serde_json::to_string(&v).unwrap().contains("\"params\":{}"),
        "a diagnostic with no parameter"
    );
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
    let dir = TempDir::new("alpha-wire");
    let headers = ["a", "b", "c", "note"];
    // c is a five-point item with 99 twice: a candidate missing code.
    let rows: Vec<Vec<C>> = [
        ("1", "5", "2"),
        ("2", "", "3"),
        ("3", "3", "99"),
        ("4", "2", "4"),
        ("", "1", "5"),
        ("2", "4", "2"),
        ("3", "3", "99"),
        ("5", "1", "5"),
        ("4", "2", "4"),
        ("1", "5", "1"),
    ]
    .iter()
    .map(|(a, b, x)| {
        let cell = |s: &'static str| if s.is_empty() { C::Empty } else { C::N(s) };
        vec![cell(a), cell(b), cell(x), C::T("t")]
    })
    .collect();
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
    // A refused validation: the text column, and the candidate code warns.
    let v = call(
        &w,
        "stats_alpha_validate",
        json!({ "slot": o["slot"], "spec": aspec(&o, 1, &["C3", "C4"], &[], None, &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["warnings"][0]["code"], "stats.alpha.suspect_missing_code");
    let want = layout_of(&fixture_json("alphaValidationView.json"));
    for path in layout_of(&v) {
        assert!(
            want.contains(&path),
            "the live validation has {path}, the fixture does not"
        );
    }
    // A run with b reversed and one without; the code is not ticked, so it
    // warns on each run (c is never reversed, so its 99 is no refusal).
    let mut got = BTreeSet::new();
    for (op, reversed, scale, codes) in [
        (2, &["C2"][..], Some((1.0, 5.0)), &[][..]),
        (3, &[][..], None, &[][..]),
    ] {
        let run = call(
            &w,
            "stats_alpha_run",
            json!({ "slot": o["slot"], "spec": aspec(&o, op, &["C1", "C2", "C3"], reversed, scale, codes) }),
        )
        .unwrap()["runId"]
            .as_u64()
            .unwrap();
        let s = until_terminal_alpha(&w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        assert_eq!(
            layout_of(&s),
            layout_of(&fixture_json("alphaRunStateView.json")),
            "the live state view and its fixture"
        );
        let r = call(&w, "stats_alpha_result", json!({ "runId": run })).unwrap();
        got.extend(layout_of(&r));
    }
    let mut want = layout_of(&fixture_json("alphaResultView.json"));
    want.extend(layout_of(&fixture_json("alphaResultViewOther.json")));
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
fn the_alpha_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("alphaCodes.json");
    let mut rust: Vec<String> = AlphaCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(rust.len(), 39, "alpha's 39 codes");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if std::env::var_os("GAPLY_WRITE_ALPHA_WIRE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("alphaCodes.json")).unwrap();
    assert_eq!(
        sorted(listed),
        rust,
        "alphaCodes.json and the Rust closed sets differ"
    );
}

#[test]
fn cronbach_alpha_runs_under_the_run_managers_ceiling() {
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
