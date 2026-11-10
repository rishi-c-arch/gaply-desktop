//! **The related rank tests' wire contract, from the Rust side (Research OS,
//! Phase 8Z-1, decision 641), in the pattern of `stats_related_ranks_wire.rs`.** Each
//! view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which 8Z-2's screen types will be tested against. Each fixture
//! fills every field, and between them every variant of every enum the result
//! carries: a value and a code, the Wilcoxon test with its exact p and the
//! Friedman test with a value not computable. The values are a wire shape, not
//! a computation.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type), so the fixtures are what a screen
//! will be sent.
//!
//! Also: `relatedRanksCodes.json`, the code list a screen's wording will be
//! keyed by, is exactly `RelatedRanksCode::ALL`, `RunCode::ALL`, `WorkspaceCode::ALL` and
//! `CapabilityCode::ALL`, sorted; and the related rank tests run under the run
//! manager's ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_RELATED_RANKS_WIRE` set, read, and then
//! pinned.

mod desc_support;
mod related_ranks_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_related_ranks_commands::{
    RelatedRanksResultView, RelatedRanksRunStateView, RelatedRanksValidationView,
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
use gaply_core::stats_related_ranks::{
    capability_id, Friedman, FriedmanColumn, RelatedColumn, RelatedRanksCode, RelatedRanksOutcome,
    RelatedTest, SignRanks, Stat, Wilcoxon,
};
use gaply_core::Database;
use related_ranks_support::{app, rrspec, until_terminal_related};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if std::env::var_os("GAPLY_WRITE_RELATED_RANKS_WIRE").is_some() {
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

fn suspect_warning() -> Diagnostic<RelatedRanksCode> {
    Diagnostic::warning(RelatedRanksCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 3)
}

fn exclusion(code: RelatedRanksCode, column: i64, count: u64) -> Exclusion<RelatedRanksCode> {
    Exclusion {
        code,
        count,
        params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(column))]),
    }
}

fn a(c: u32) -> Alias {
    Alias(ColumnAlias(c))
}

fn limitations() -> Vec<RelatedRanksCode> {
    vec![
        RelatedRanksCode::NoPairwiseComparisons,
        RelatedRanksCode::TwoSidedOnly,
        RelatedRanksCode::NoLocationEstimate,
        RelatedRanksCode::ExactWithoutTiesOrZerosOnly,
        RelatedRanksCode::PrattNotOffered,
        RelatedRanksCode::NoFurtherRelatedTests,
        RelatedRanksCode::NoWeights,
        RelatedRanksCode::ColumnOrder,
    ]
}

/// Two columns: the Wilcoxon test with its exact p, a note, and both
/// exclusions.
fn outcome() -> RelatedRanksOutcome {
    RelatedRanksOutcome {
        columns: vec![
            RelatedColumn { column: a(1), median: 2.5 },
            RelatedColumn { column: a(2), median: 4.75 },
        ],
        rows_used: 6,
        test: RelatedTest::Wilcoxon(Wilcoxon {
            first: a(1),
            second: a(2),
            n: 6,
            zero_differences: 0,
            tie_runs: 0,
            negative: SignRanks { count: 5, rank_sum: 20.0, mean_rank: Some(4.0) },
            positive: SignRanks { count: 1, rank_sum: 1.0, mean_rank: Some(1.0) },
            v: 1.0,
            t_minus: 20.0,
            z: Stat::Value(-1.992),
            p: Stat::Value(0.0464),
            exact_p: Some(0.0625),
            rank_biserial: Stat::Value(-0.904),
            median_difference: Stat::Value(-2.25),
        }),
        exclusions: vec![
            exclusion(RelatedRanksCode::MissingValue, 2, 2),
            exclusion(RelatedRanksCode::MissingCode, 1, 3),
        ],
        notes: vec![RelatedRanksCode::RowsBelowFive],
        limitations: limitations(),
    }
}

/// Three columns: the Friedman test with every row's values tied, so Q, its p
/// and Kendall's W are not computable.
fn other_outcome() -> RelatedRanksOutcome {
    let column = |c, rank_sum| FriedmanColumn { column: a(c), rank_sum, mean_rank: rank_sum / 3.0 };
    RelatedRanksOutcome {
        columns: vec![
            RelatedColumn { column: a(1), median: 7.0 },
            RelatedColumn { column: a(2), median: 7.0 },
            RelatedColumn { column: a(3), median: 7.0 },
        ],
        rows_used: 3,
        test: RelatedTest::Friedman(Friedman {
            columns: vec![column(1, 6.0), column(2, 6.0), column(3, 6.0)],
            q: Stat::NotComputable(RelatedRanksCode::AllValuesTied),
            df: 2.0,
            p: Stat::NotComputable(RelatedRanksCode::AllValuesTied),
            kendalls_w: Stat::NotComputable(RelatedRanksCode::AllValuesTied),
        }),
        exclusions: Vec::new(),
        notes: vec![RelatedRanksCode::RowsBelowFive],
        limitations: limitations(),
    }
}

fn manifest() -> RunManifest<RelatedRanksCode> {
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

fn views() -> (RelatedRanksValidationView, RelatedRanksRunStateView, RelatedRanksResultView) {
    (
        RelatedRanksValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![
                Diagnostic::error(RelatedRanksCode::ColumnRepeated).int(ParamKey::Column, 1),
                Diagnostic::error(RelatedRanksCode::TooFewColumns).int(ParamKey::Count, 1),
            ],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::error(RelatedRanksCode::NoCompleteRows)],
            estimated_resources: ResourceEstimate {
                rows: 21,
                columns: 2,
                cells: 42,
            },
        },
        RelatedRanksRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 0,
            steps_total: 1,
            diagnostics: vec![suspect_warning()],
        },
        RelatedRanksResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: outcome(),
            diagnostics: vec![
                suspect_warning(),
                Diagnostic::note(RelatedRanksCode::NoPairwiseComparisons),
            ],
            manifest: manifest(),
        },
    )
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    let (v, s, r) = views();
    assert_bytes("relatedRanksValidationView.json", &v);
    assert_bytes("relatedRanksRunStateView.json", &s);
    assert_bytes("relatedRanksResultView.json", &r);
    let other = RelatedRanksResultView {
        result: other_outcome(),
        ..views().2
    };
    assert_bytes("relatedRanksResultViewOther.json", &other);
    // Every variant of the result is in a fixture: a screen is tested on
    // each shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "\"wilcoxon\":",
        "\"friedman\":",
        "\"exactP\":0.0625",
        "stats.related_ranks.all_values_tied",
        "stats.related_ranks.rows_below_five",
        "stats.related_ranks.missing_code",
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
    let dir = TempDir::new("related-ranks-wire");
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
    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .unwrap();
    // A refused validation: one column twice.
    let v = call(
        &w,
        "stats_related_ranks_validate",
        json!({ "slot": o["slot"], "spec": rrspec(&o, 1, &["C1", "C1"], &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("relatedRanksValidationView.json"));
    for path in layout_of(&v) {
        assert!(
            want.contains(&path),
            "the live validation has {path}, the fixture does not"
        );
    }
    let run = call(
        &w,
        "stats_related_ranks_run",
        json!({ "slot": o["slot"], "spec": rrspec(&o, 2, &["C1", "C2"], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    let s = until_terminal_related(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    let want = layout_of(&fixture_json("relatedRanksRunStateView.json"));
    for path in layout_of(&s) {
        assert!(
            want.contains(&path),
            "the live state view has {path}, the fixture does not"
        );
    }
    let r = call(&w, "stats_related_ranks_result", json!({ "runId": run })).unwrap();
    let mut want = layout_of(&fixture_json("relatedRanksResultView.json"));
    want.extend(layout_of(&fixture_json("relatedRanksResultViewOther.json")));
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
    // Not vacuous: the live result's layout reaches into the columns, the
    // test and the manifest.
    assert!(
        got.len() >= 40,
        "the layout reads into the result: {}",
        got.len()
    );
    for path in [
        ".result.columns[].column:string",
        ".result.test.wilcoxon.exactP:number",
        ".result.test.wilcoxon.negative.meanRank:number",
        ".manifest.outputHash:string",
    ] {
        assert!(got.contains(path), "{path}: {got:?}");
    }
    assert!(
        !got.iter().any(|p| p.contains("friedman")),
        "two columns: no Friedman test"
    );
    // Three columns: the Friedman shape the Other fixture carries.
    let run = call(
        &w,
        "stats_related_ranks_run",
        json!({ "slot": o["slot"], "spec": rrspec(&o, 3, &["C1", "C2", "C3"], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal_related(&w, run)["state"], "succeeded");
    let r = call(&w, "stats_related_ranks_result", json!({ "runId": run })).unwrap();
    let got = layout_of(&r);
    for path in &got {
        let nullable = path.strip_suffix(":null").is_some_and(|at| {
            want.iter()
                .any(|w| w.rsplit_once(':').map(|(p, _)| p) == Some(at))
        });
        // A computed Q is a value where the Other fixture's is a reason.
        let value_of_reason = path
            .strip_suffix(".value:number")
            .is_some_and(|at| want.contains(&format!("{at}.not_computable:string")));
        assert!(
            want.contains(path) || nullable || value_of_reason,
            "the live result has {path}, the fixture does not"
        );
    }
    assert!(got.contains(".result.test.friedman.columns[].meanRank:number"), "{got:?}");
}

/* ---- the code list and the ceiling --------------------------------------------- */

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_related_ranks_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("relatedRanksCodes.json");
    let mut rust: Vec<String> = RelatedRanksCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(rust.len(), 34, "the related rank tests' 34 codes");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if std::env::var_os("GAPLY_WRITE_RELATED_RANKS_WIRE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("relatedRanksCodes.json")).unwrap();
    assert_eq!(
        sorted(listed),
        rust,
        "relatedRanksCodes.json and the Rust closed sets differ"
    );
}

#[test]
fn the_related_rank_tests_run_under_the_run_managers_ceiling() {
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
