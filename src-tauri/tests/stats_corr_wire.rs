//! **The correlations' wire contract, from the Rust side (Research OS,
//! Phase 8H-2, decision 356), in the pattern of `stats_anova_wire.rs`.**
//! Each view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which the screen's types are tested against. Between them the
//! fixtures fill every field and every variant of every enum the result
//! carries: a value and a code, a finite t and a perfect one, t and z, a
//! computed interval and one not computable, a computed pair and one not
//! computable, an exclusion naming its column and one naming none. The
//! values are a wire shape, not a computation.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type), so the fixtures are what the screen
//! will be sent.
//!
//! Also: `corrCodes.json`, the code list the screen's wording is keyed by,
//! is exactly `CorrCode::ALL`, `RunCode::ALL`, `WorkspaceCode::ALL` and
//! `CapabilityCode::ALL`, sorted; the budget the screen states is the core's;
//! and the correlations run under the run manager's ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_CORR_WIRE` set, read, and then
//! pinned.

mod corr_support;
mod desc_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::stats_corr_commands::{CorrResultView, CorrRunStateView, CorrValidationView};
use corr_support::{app, cspec, until_terminal_corr};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_corr::{
    capability_id, Bounded, Coefficient, CoefficientTable, CorrCode, CorrOutcome, Descriptive,
    Interval, Pair, PairOutcome, PairStats, Stat, TStat, TestStatistic,
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
    if std::env::var_os("GAPLY_WRITE_CORR_WIRE").is_some() {
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

fn suspect_warning() -> Diagnostic<CorrCode> {
    Diagnostic::warning(CorrCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(3))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 2)
}

fn column(n: i64) -> BTreeMap<ParamKey, ParamValue<CorrCode>> {
    BTreeMap::from([(ParamKey::Column, ParamValue::Int(n))])
}

fn t_pair(first: u32, second: u32, n: u64, r: f64, t: Bounded, interval: Interval) -> Pair {
    Pair {
        first: c(first),
        second: c(second),
        n,
        outcome: PairOutcome::Computed(PairStats {
            estimate: r,
            test: TestStatistic::T(TStat {
                t,
                df: (n - 2) as f64,
            }),
            p: if matches!(t, Bounded::Infinite(_)) {
                0.0
            } else {
                0.0123
            },
            interval,
        }),
    }
}

/// Every field filled, and the enums' variants between this and
/// [`other_outcome`].
fn outcome() -> CorrOutcome {
    CorrOutcome {
        descriptives: vec![
            Descriptive {
                variable: c(1),
                n: 9,
                mean: Stat::Value(4.25),
                sd: Stat::Value(1.5),
            },
            Descriptive {
                variable: c(3),
                n: 8,
                mean: Stat::Value(-2.5),
                sd: Stat::Value(0.75),
            },
            Descriptive {
                variable: c(4),
                n: 1,
                mean: Stat::Value(6.0),
                sd: Stat::NotComputable(CorrCode::TooFewValues),
            },
        ],
        coefficients: vec![
            CoefficientTable {
                coefficient: Coefficient::Pearson,
                pairs: vec![
                    t_pair(
                        1,
                        3,
                        8,
                        0.61,
                        Bounded::Value(1.886),
                        Interval::Computed {
                            lower: -0.18,
                            upper: 0.92,
                        },
                    ),
                    Pair {
                        first: c(1),
                        second: c(4),
                        n: 1,
                        outcome: PairOutcome::NotComputable(CorrCode::PairTooFewRows),
                    },
                    Pair {
                        first: c(3),
                        second: c(4),
                        n: 1,
                        outcome: PairOutcome::NotComputable(CorrCode::PairTooFewRows),
                    },
                ],
            },
            CoefficientTable {
                coefficient: Coefficient::Spearman,
                pairs: vec![t_pair(
                    1,
                    3,
                    8,
                    1.0,
                    Bounded::Infinite(CorrCode::PerfectCoefficient),
                    Interval::Computed {
                        lower: 1.0,
                        upper: 1.0,
                    },
                )],
            },
            CoefficientTable {
                coefficient: Coefficient::Kendall,
                pairs: vec![Pair {
                    first: c(1),
                    second: c(3),
                    n: 8,
                    outcome: PairOutcome::Computed(PairStats {
                        estimate: 0.5,
                        test: TestStatistic::Z(1.732),
                        p: 0.0833,
                        interval: Interval::Computed {
                            lower: -0.25,
                            upper: 0.875,
                        },
                    }),
                }],
            },
        ],
        rows_used: 9,
        exclusions: vec![
            Exclusion {
                code: CorrCode::MissingValue,
                count: 2,
                params: column(1),
            },
            Exclusion {
                code: CorrCode::MissingCode,
                count: 1,
                params: column(3),
            },
        ],
        limitations: vec![
            CorrCode::PearsonAssumesNormality,
            CorrCode::TwoTailedNoPartial,
        ],
    }
}

/// The other variants: a mean not computable, an interval not computable,
/// a pair constant, and a pairwise exclusion that names no column.
fn other_outcome() -> CorrOutcome {
    CorrOutcome {
        descriptives: vec![
            Descriptive {
                variable: c(2),
                n: 4,
                mean: Stat::Value(3.0),
                sd: Stat::Value(0.5),
            },
            Descriptive {
                variable: c(5),
                n: 0,
                mean: Stat::NotComputable(CorrCode::NoValues),
                sd: Stat::NotComputable(CorrCode::TooFewValues),
            },
        ],
        coefficients: vec![CoefficientTable {
            coefficient: Coefficient::Kendall,
            pairs: vec![Pair {
                first: c(2),
                second: c(5),
                n: 4,
                outcome: PairOutcome::Computed(PairStats {
                    estimate: -1.0,
                    test: TestStatistic::Z(-2.04),
                    p: 0.0415,
                    interval: Interval::NotComputable(CorrCode::IntervalTooFewRows),
                }),
            }],
        }],
        rows_used: 4,
        exclusions: vec![Exclusion {
            code: CorrCode::FewerThanTwoValues,
            count: 3,
            params: BTreeMap::new(),
        }],
        limitations: vec![CorrCode::TwoTailedNoPartial],
    }
}

fn manifest() -> RunManifest<CorrCode> {
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
            code: CorrCode::MissingValue,
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

fn views() -> (CorrValidationView, CorrRunStateView, CorrResultView) {
    (
        CorrValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![
                Diagnostic::error(CorrCode::VariableHoldsText)
                    .with(ParamKey::Column, ParamValue::Int(4))
                    .int(ParamKey::Count, 10),
                Diagnostic::error(CorrCode::OverBudget)
                    .int(ParamKey::Count, 780)
                    .int(ParamKey::Found, 100000),
            ],
            warnings: vec![suspect_warning()],
            missing_information: vec![],
            estimated_resources: ResourceEstimate {
                rows: 12,
                columns: 3,
                cells: 36,
            },
        },
        CorrRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 4,
            steps_total: 10,
            diagnostics: vec![suspect_warning()],
        },
        CorrResultView {
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
    assert_bytes("corrValidationView.json", &v);
    assert_bytes("corrRunStateView.json", &s);
    assert_bytes("corrResultView.json", &r);
    let other = CorrResultView {
        result: other_outcome(),
        ..views().2
    };
    assert_bytes("corrResultViewOther.json", &other);
    // Every variant is in one fixture or the other: the screen is tested on
    // each shape it can be sent.
    let text = serde_json::to_string(&r).unwrap() + &serde_json::to_string(&other).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "\"computed\":",
        "\"infinite\":\"stats.corr.perfect_coefficient\"",
        "\"t\":{\"t\":",
        "\"z\":",
        "\"pearson\"",
        "\"spearman\"",
        "\"kendall\"",
        "\"params\":{}",
        "\"params\":{\"column\":1}",
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
    let dir = TempDir::new("corr-wire");
    let headers = ["a", "b", "c", "note"];
    // c is on a 1 to 7 scale with 99 twice: a candidate missing code.
    let rows: Vec<Vec<C>> = [
        ("1.5", "2", "3"),
        ("2.25", "", "5"),
        ("3.75", "4.5", "99"),
        ("4", "3", "2"),
        ("", "6", "7"),
        ("6.125", "5.5", "6"),
        ("7", "8", "99"),
        ("9.5", "7.25", "1"),
        ("8", "9", "4"),
        ("10.5", "10", "5"),
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
        "stats_corr_validate",
        json!({ "slot": o["slot"], "spec": cspec(&o, 1, &["C3", "C4"], &["pearson"], "pairwise", &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["warnings"][0]["code"], "stats.corr.suspect_missing_code");
    let want = layout_of(&fixture_json("corrValidationView.json"));
    for path in layout_of(&v) {
        assert!(
            want.contains(&path),
            "the live validation has {path}, the fixture does not"
        );
    }
    // A run with every coefficient, listwise and pairwise; the code is not
    // ticked, so it warns on the run too.
    let mut got = BTreeSet::new();
    for (op, missing) in [(2, "listwise"), (3, "pairwise")] {
        let run = call(
            &w,
            "stats_corr_run",
            json!({ "slot": o["slot"], "spec": cspec(&o, op, &["C1", "C2", "C3"], &["pearson", "spearman", "kendall"], missing, &[]) }),
        )
        .unwrap()["runId"]
            .as_u64()
            .unwrap();
        let s = until_terminal_corr(&w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        assert_eq!(
            layout_of(&s),
            layout_of(&fixture_json("corrRunStateView.json")),
            "the live state view and its fixture"
        );
        let r = call(&w, "stats_corr_result", json!({ "runId": run })).unwrap();
        got.extend(layout_of(&r));
    }
    let mut want = layout_of(&fixture_json("corrResultView.json"));
    want.extend(layout_of(&fixture_json("corrResultViewOther.json")));
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
fn the_corr_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("corrCodes.json");
    let mut rust: Vec<String> = CorrCode::ALL.iter().map(|c| c.wire().to_string()).collect();
    assert_eq!(rust.len(), 37, "the correlations' 37 codes");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if std::env::var_os("GAPLY_WRITE_CORR_WIRE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("corrCodes.json")).unwrap();
    assert_eq!(
        sorted(listed),
        rust,
        "corrCodes.json and the Rust closed sets differ"
    );
}

/// The budget the screen states (`corrLimits.json`) is the core's own
/// (decisions 342 and 359).
#[test]
fn the_budget_the_screen_reads_is_the_cores() {
    let file: Value = serde_json::from_str(
        &std::fs::read_to_string(screens().join("corrLimits.json")).expect("corrLimits.json"),
    )
    .unwrap();
    assert_eq!(file.as_object().map(|o| o.len()), Some(1), "{file}");
    assert_eq!(
        file["budgetSeconds"].as_f64(),
        Some(gaply_core::stats_corr::BUDGET_SECONDS)
    );
}

#[test]
fn the_correlations_run_under_the_run_managers_ceiling() {
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
