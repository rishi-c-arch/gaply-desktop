//! **The rank tests' wire contract, from the Rust side (Research OS, Phase
//! 8Y-1, decision 607), in the pattern of `stats_xtab_wire.rs`.** Each view
//! serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which the screen's types (`ranksTypes.vitest.ts`, 8Y-2) are tested against. Each fixture
//! fills every field, and between them every variant of every enum the
//! result carries: a value and a code, the Mann-Whitney test with its exact
//! p and the Kruskal-Wallis test with a value not computable. The values are
//! a wire shape, not a computation.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type), so the fixtures are what a screen
//! will be sent.
//!
//! Also: `ranksCodes.json`, the code list a screen's wording will be keyed
//! by, is exactly `RanksCode::ALL`, `RunCode::ALL`, `WorkspaceCode::ALL` and
//! `CapabilityCode::ALL`, sorted; and the rank tests run under the run
//! manager's ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_RANKS_WIRE` set, read, and then
//! pinned.

mod desc_support;
mod ranks_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_ranks_commands::{RanksResultView, RanksRunStateView, RanksValidationView};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_ranks::{
    capability_id, IndependentRanksOutcome, KruskalWallis, MannWhitney, RankGroup, RankTest,
    RanksCode, Stat,
};
use gaply_core::Database;
use ranks_support::{app, rspec, until_terminal_ranks};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if std::env::var_os("GAPLY_WRITE_RANKS_WIRE").is_some() {
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

fn suspect_warning() -> Diagnostic<RanksCode> {
    Diagnostic::warning(RanksCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 3)
}

fn exclusion(code: RanksCode, column: i64, count: u64) -> Exclusion<RanksCode> {
    Exclusion {
        code,
        count,
        params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(column))]),
    }
}

fn group(level: u32, label: &str, n: u64, median: f64, rank_sum: f64) -> RankGroup {
    RankGroup {
        level,
        label: label.into(),
        n,
        median,
        rank_sum,
        mean_rank: rank_sum / n as f64,
    }
}

fn limitations() -> Vec<RanksCode> {
    vec![
        RanksCode::NoPairwiseComparisons,
        RanksCode::TwoSidedOnly,
        RanksCode::NoLocationShiftEstimate,
        RanksCode::ExactWithoutTiesOnly,
        RanksCode::NoFurtherKSampleTests,
        RanksCode::NoWeights,
        RanksCode::CodePointOrder,
    ]
}

/// Two groups: the Mann-Whitney test with its exact p, a note, and every
/// exclusion.
fn outcome() -> IndependentRanksOutcome {
    IndependentRanksOutcome {
        groups: vec![group(0, "control", 4, 2.5, 13.0), group(2, "treated", 6, 6.25, 42.0)],
        n: 10,
        tie_runs: 0,
        test: RankTest::MannWhitney(MannWhitney {
            first: 0,
            second: 2,
            u1: 3.0,
            u2: 21.0,
            z: Stat::Value(-1.917),
            p: Stat::Value(0.0552),
            exact_p: Some(0.0667),
            rank_biserial: -0.75,
        }),
        exclusions: vec![
            exclusion(RanksCode::MissingGroup, 2, 2),
            exclusion(RanksCode::OtherLevel, 2, 5),
            exclusion(RanksCode::MissingOutcome, 1, 1),
            exclusion(RanksCode::MissingCode, 1, 3),
        ],
        notes: vec![RanksCode::GroupBelowFive],
        limitations: limitations(),
    }
}

/// Three groups: the Kruskal-Wallis test with every value tied, so H, its p
/// and epsilon-squared are not computable; no exact p.
fn other_outcome() -> IndependentRanksOutcome {
    IndependentRanksOutcome {
        groups: vec![
            group(0, "a", 3, 7.0, 15.0),
            group(1, "b", 3, 7.0, 15.0),
            group(2, "c", 3, 7.0, 15.0),
        ],
        n: 9,
        tie_runs: 1,
        test: RankTest::KruskalWallis(KruskalWallis {
            h: Stat::NotComputable(RanksCode::AllValuesTied),
            df: 2.0,
            p: Stat::NotComputable(RanksCode::AllValuesTied),
            epsilon_squared: Stat::NotComputable(RanksCode::AllValuesTied),
        }),
        exclusions: Vec::new(),
        notes: vec![RanksCode::GroupBelowFive],
        limitations: limitations(),
    }
}

fn manifest() -> RunManifest<RanksCode> {
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

fn views() -> (RanksValidationView, RanksRunStateView, RanksResultView) {
    (
        RanksValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![Diagnostic::error(RanksCode::GroupEmpty).int(ParamKey::Index, 1)],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::error(RanksCode::SameColumn)],
            estimated_resources: ResourceEstimate {
                rows: 21,
                columns: 2,
                cells: 42,
            },
        },
        RanksRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 0,
            steps_total: 1,
            diagnostics: vec![suspect_warning()],
        },
        RanksResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![
                suspect_warning(),
                Diagnostic::note(RanksCode::NoPairwiseComparisons),
            ],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("ranksValidationView.json", &v);
    assert_bytes("ranksRunStateView.json", &s);
    assert_bytes("ranksResultView.json", &r);
    let other = RanksResultView {
        result: other_outcome(),
        ..views().2
    };
    assert_bytes("ranksResultViewOther.json", &other);
    // Every variant of the result is in a fixture: a screen is tested on
    // each shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "\"mann_whitney\":",
        "\"kruskal_wallis\":",
        "\"exactP\":0.0667",
        "stats.ranks.all_values_tied",
        "stats.ranks.group_below_five",
        "stats.ranks.missing_code",
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
    let dir = TempDir::new("ranks-wire");
    let headers = ["y", "g", "w"];
    let mut rows: Vec<Vec<C>> = Vec::new();
    for (y, g) in [
        ("1.5", "a"),
        ("2.5", "b"),
        ("3.5", "a"),
        ("4.5", "b"),
        ("5.5", "a"),
        ("6.5", "b"),
        ("7.5", "b"),
    ] {
        rows.push(vec![C::N(y), C::T(g), C::N("1")]);
    }
    rows.push(vec![C::Empty, C::T("a"), C::N("2")]);
    rows.push(vec![C::N("8"), C::Empty, C::N("3")]);
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
        "stats_ranks_validate",
        json!({ "slot": o["slot"], "spec": rspec(&o, 1, "C1", "C1", None, &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("ranksValidationView.json"));
    for path in layout_of(&v) {
        assert!(
            want.contains(&path),
            "the live validation has {path}, the fixture does not"
        );
    }
    let run = call(
        &w,
        "stats_ranks_run",
        json!({ "slot": o["slot"], "spec": rspec(&o, 2, "C1", "C2", None, &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    let s = until_terminal_ranks(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    let want = layout_of(&fixture_json("ranksRunStateView.json"));
    for path in layout_of(&s) {
        assert!(
            want.contains(&path),
            "the live state view has {path}, the fixture does not"
        );
    }
    let r = call(&w, "stats_ranks_result", json!({ "runId": run })).unwrap();
    let mut want = layout_of(&fixture_json("ranksResultView.json"));
    want.extend(layout_of(&fixture_json("ranksResultViewOther.json")));
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
    // Not vacuous: the live result's layout reaches into the groups, the
    // test and the manifest (40 paths measured for this two-group run).
    assert!(
        got.len() >= 40,
        "the layout reads into the result: {}",
        got.len()
    );
    for path in [
        ".result.groups[].label:string",
        ".result.test.mann_whitney.exactP:number",
        ".manifest.outputHash:string",
    ] {
        assert!(got.contains(path), "{path}: {got:?}");
    }
    assert!(
        !got.iter().any(|p| p.contains("kruskal_wallis")),
        "two groups: no Kruskal-Wallis test"
    );
}

/* ---- the code list and the ceiling --------------------------------------------- */

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_ranks_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("ranksCodes.json");
    let mut rust: Vec<String> = RanksCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(rust.len(), 37, "the rank tests' 37 codes");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if std::env::var_os("GAPLY_WRITE_RANKS_WIRE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("ranksCodes.json")).unwrap();
    assert_eq!(
        sorted(listed),
        rust,
        "ranksCodes.json and the Rust closed sets differ"
    );
}

#[test]
fn the_rank_tests_run_under_the_run_managers_ceiling() {
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
