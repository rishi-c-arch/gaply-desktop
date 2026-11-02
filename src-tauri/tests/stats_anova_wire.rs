//! **The one-way ANOVA's wire contract, from the Rust side (Research OS,
//! Phase 8G-2, decision 318), in the pattern of `stats_onesample_wire.rs`.**
//! Each view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline), which the screen's types are tested against. Each fixture fills
//! every field, and between them every variant of every enum the result
//! carries: a value and a code, a p and a p below the floor, a computed pair
//! and one not computable, a computed test and one not computable. The
//! values are a wire shape, not a computation.
//!
//! The live views: a real run through IPC gives views with the layout of the
//! fixtures (every key and JSON type), so the fixtures are what the screen
//! will be sent.
//!
//! Also: `anovaCodes.json`, the code list the screen's wording is keyed by,
//! is exactly `AnovaCode::ALL`, `RunCode::ALL`, `WorkspaceCode::ALL` and
//! `CapabilityCode::ALL`, sorted; the post hoc cap is the group limit the
//! screen already reads (`ttestLimits.json`); and the ANOVA runs under the
//! run manager's ceiling.
//!
//! The fixtures are written with `GAPLY_WRITE_ANOVA_WIRE` set, read, and
//! then pinned.

mod anova_support;
mod desc_support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use anova_support::{app, aspec, until_terminal_anova};
use app_lib::stats_anova_commands::{AnovaResultView, AnovaRunStateView, AnovaValidationView};
use desc_support::{call, delimited, TempDir, C};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_anova::{
    capability_id, AnovaCode, AnovaOutcome, AnovaTable, FOutcome, FTest, GroupRow, Pair,
    PairOutcome, PairStats, PostHoc, PostHocP, PostHocPairs, PostHocTable, Stat, Table,
    POST_HOC_MAX_LEVELS,
};
use gaply_core::Database;

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn fixture(name: &str) -> PathBuf {
    screens().join("fixtures").join(name)
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if std::env::var_os("GAPLY_WRITE_ANOVA_WIRE").is_some() {
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
const OUTPUT: &str = "5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a";

fn suspect_warning() -> Diagnostic<AnovaCode> {
    Diagnostic::warning(AnovaCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 2)
}

fn group(level: u32, label: &str, n: u64, mean: f64) -> GroupRow {
    GroupRow {
        level,
        label: label.into(),
        n,
        mean,
        sd: 1.25,
        se: 0.5,
        ci_lower: mean - 1.0,
        ci_upper: mean + 1.0,
        min: mean - 2.0,
        max: mean + 2.0,
    }
}

fn stats(difference: f64, p: PostHocP) -> PairStats {
    PairStats {
        difference,
        se: 0.75,
        statistic: 4.5,
        df: 9.5,
        p,
        ci_lower: difference - 1.5,
        ci_upper: difference + 1.5,
    }
}

/// Every field filled, and every enum's variants between its fields.
fn outcome() -> AnovaOutcome {
    AnovaOutcome {
        groups: vec![
            group(0, "ctrl", 4, 2.5),
            group(2, "dose_hi", 4, 11.5),
            group(3, "dose_lo", 5, 5.75),
        ],
        table: Table::Computed(AnovaTable {
            ss_between: 163.5,
            ss_within: 39.25,
            ss_total: 202.75,
            df_between: 2.0,
            df_within: 10.0,
            ms_between: 81.75,
            ms_within: 3.925,
            f: Stat::Value(20.8),
            p: Stat::Value(0.000271),
        }),
        welch: FOutcome::NotComputable(AnovaCode::WelchVarianceZero),
        levene_mean: FOutcome::Computed(FTest {
            f: 6.5,
            df1: 2.0,
            df2: 10.0,
            p: 0.0155,
        }),
        levene_median: FOutcome::NotComputable(AnovaCode::LeveneDeviationsEqual),
        eta_squared: Stat::Value(0.806),
        omega_squared: Stat::NotComputable(AnovaCode::WithinVarianceZero),
        post_hoc: vec![
            PostHocTable {
                test: PostHoc::Tukey,
                pairs: PostHocPairs::Computed(vec![
                    Pair {
                        first: 0,
                        second: 2,
                        outcome: PairOutcome::Computed(stats(-9.0, PostHocP::Below(1e-10))),
                    },
                    Pair {
                        first: 0,
                        second: 3,
                        outcome: PairOutcome::Computed(stats(-3.25, PostHocP::Value(0.032))),
                    },
                ]),
            },
            PostHocTable {
                test: PostHoc::GamesHowell,
                pairs: PostHocPairs::Computed(vec![Pair {
                    first: 2,
                    second: 3,
                    outcome: PairOutcome::NotComputable(AnovaCode::PairBothConstant),
                }]),
            },
            PostHocTable {
                test: PostHoc::Bonferroni,
                pairs: PostHocPairs::NotComputable(AnovaCode::WithinVarianceZero),
            },
        ],
        exclusions: vec![Exclusion {
            code: AnovaCode::OtherLevel,
            count: 4,
            params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(2))]),
        }],
        notes: vec![AnovaCode::PBelowFloor, AnovaCode::OmegaSquaredNegative],
        limitations: vec![
            AnovaCode::NormalityNotAssessed,
            AnovaCode::EffectSizesHaveNoInterval,
            AnovaCode::NoContrasts,
        ],
    }
}

/// Every field in the other variant from [`outcome`]'s, so the two result
/// fixtures together hold each field both ways.
fn other_outcome() -> AnovaOutcome {
    AnovaOutcome {
        groups: vec![group(0, "a", 3, 1.0), group(1, "b", 3, 2.0)],
        table: Table::NotComputable(AnovaCode::OutcomeConstant),
        welch: FOutcome::Computed(FTest {
            f: 3.5,
            df1: 1.0,
            df2: 3.75,
            p: 0.14,
        }),
        levene_mean: FOutcome::NotComputable(AnovaCode::OutcomeConstant),
        levene_median: FOutcome::Computed(FTest {
            f: 0.5,
            df1: 1.0,
            df2: 4.0,
            p: 0.52,
        }),
        eta_squared: Stat::NotComputable(AnovaCode::OutcomeConstant),
        omega_squared: Stat::Value(-0.05),
        post_hoc: vec![
            PostHocTable {
                test: PostHoc::Tukey,
                pairs: PostHocPairs::NotComputable(AnovaCode::OutcomeConstant),
            },
            PostHocTable {
                test: PostHoc::GamesHowell,
                pairs: PostHocPairs::Computed(vec![Pair {
                    first: 0,
                    second: 1,
                    outcome: PairOutcome::Computed(stats(-1.0, PostHocP::Value(0.21))),
                }]),
            },
            PostHocTable {
                test: PostHoc::Bonferroni,
                pairs: PostHocPairs::Computed(vec![
                    Pair {
                        first: 0,
                        second: 1,
                        outcome: PairOutcome::Computed(stats(-1.0, PostHocP::Below(1e-10))),
                    },
                    Pair {
                        first: 0,
                        second: 1,
                        outcome: PairOutcome::NotComputable(AnovaCode::Overflow),
                    },
                ]),
            },
        ],
        exclusions: vec![],
        notes: vec![],
        limitations: vec![AnovaCode::NormalityNotAssessed],
    }
}

fn manifest() -> RunManifest<AnovaCode> {
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: 1,
        engine_version: 1,
        inputs: vec![hash(INPUT)],
        parameters_hash: hash(PARAMS),
        seed: Some(7),
        rows_read: 17,
        rows_used: 13,
        exclusions: vec![Exclusion {
            code: AnovaCode::OtherLevel,
            count: 4,
            params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(2))]),
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

fn views() -> (AnovaValidationView, AnovaRunStateView, AnovaResultView) {
    (
        AnovaValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![Diagnostic::error(AnovaCode::GroupTooSmall)
                .int(ParamKey::Index, 1)
                .int(ParamKey::Count, 1)],
            warnings: vec![suspect_warning()],
            missing_information: vec![
                Diagnostic::note(AnovaCode::TooFewLevels).int(ParamKey::Count, 1)
            ],
            estimated_resources: ResourceEstimate {
                rows: 17,
                columns: 2,
                cells: 34,
            },
        },
        AnovaRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            steps_done: 20,
            steps_total: 46,
            diagnostics: vec![suspect_warning()],
        },
        AnovaResultView {
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
    assert_bytes("anovaValidationView.json", &v);
    assert_bytes("anovaRunStateView.json", &s);
    assert_bytes("anovaResultView.json", &r);
    assert_bytes(
        "anovaResultViewOther.json",
        &AnovaResultView {
            result: other_outcome(),
            ..views().2
        },
    );
    // The result's every variant is in the fixture: the screen is tested on
    // each shape it can be sent.
    let text = serde_json::to_string(&r).unwrap();
    for shape in [
        "\"value\":",
        "\"not_computable\":",
        "\"computed\":",
        "\"below\":",
        "\"tukey\"",
        "\"games_howell\"",
        "\"bonferroni\"",
    ] {
        assert!(
            text.contains(shape),
            "{shape} missing from the result fixture"
        );
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
    let dir = TempDir::new("anova-wire");
    let headers = ["y", "g"];
    let rows: Vec<Vec<C>> = [
        ("1.5", "a"),
        ("2.25", "a"),
        ("3.75", "a"),
        ("99", "a"),
        ("4.5", "b"),
        ("6.125", "b"),
        ("7", "b"),
        ("9.5", "c"),
        ("12.75", "c"),
        ("8", "c"),
        ("15.5", "c"),
        ("5", "d"),
    ]
    .iter()
    .map(|(y, g)| vec![C::N(y), C::T(g)])
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
    // A refused validation: level d has one value (group_too_small), and a
    // candidate missing code (99) warns.
    let v = call(
        &w,
        "stats_anova_validate",
        json!({ "slot": o["slot"], "spec": aspec(&o, 1, "C1", "C2", None, &["tukey"], &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    let want = layout_of(&fixture_json("anovaValidationView.json"));
    for path in layout_of(&v) {
        assert!(
            want.contains(&path),
            "the live validation has {path}, the fixture does not"
        );
    }
    // A run on levels a to c, all three tests.
    let run = call(
        &w,
        "stats_anova_run",
        json!({ "slot": o["slot"], "spec": aspec(&o, 2, "C1", "C2", Some(&[0, 1, 2]), &["tukey", "games_howell", "bonferroni"], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    let s = until_terminal_anova(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    // The candidate missing code (99) warns on the live run as on the
    // fixture, so the two layouts are equal, diagnostics included.
    assert_eq!(
        layout_of(&s),
        layout_of(&fixture_json("anovaRunStateView.json")),
        "the live state view and its fixture"
    );
    let r = call(&w, "stats_anova_result", json!({ "runId": run })).unwrap();
    let mut want = layout_of(&fixture_json("anovaResultView.json"));
    want.extend(layout_of(&fixture_json("anovaResultViewOther.json")));
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
        got.len() > 60,
        "the layout reads into the result: {}",
        got.len()
    );
}

/* ---- the code list, the cap and the ceiling ------------------------------------ */

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_anova_code_list_is_exactly_the_closed_sets() {
    let path = screens().join("anovaCodes.json");
    let mut rust: Vec<String> = AnovaCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(rust.len(), 38, "the ANOVA's 38 codes");
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    if std::env::var_os("GAPLY_WRITE_ANOVA_WIRE").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("anovaCodes.json")).unwrap();
    assert_eq!(
        sorted(listed),
        rust,
        "anovaCodes.json and the Rust closed sets differ"
    );
}

/// The post hoc cap is the group limit (decisions 317 and 322), which the
/// screen reads from `ttestLimits.json`, pinned to the core's constant.
#[test]
fn the_post_hoc_cap_is_the_group_limit_the_screen_reads() {
    let file: Value = serde_json::from_str(
        &std::fs::read_to_string(screens().join("ttestLimits.json")).expect("ttestLimits.json"),
    )
    .unwrap();
    assert_eq!(file, json!({ "maxGroupLevels": POST_HOC_MAX_LEVELS }));
    assert_eq!(
        POST_HOC_MAX_LEVELS,
        gaply_core::stats_ttest::MAX_GROUP_LEVELS
    );
}

#[test]
fn the_anova_runs_under_the_run_managers_ceiling() {
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
