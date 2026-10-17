//! **The descriptive summary's wire contract, from the Rust side (Research
//! OS, Phase 2B).** Each view the `stats_desc_*` commands return serialises
//! to exactly the bytes of its fixture in `src/screens/statistics/fixtures/`
//! (pretty JSON, two-space indent, a final newline). The same fixtures are
//! deep-compared on the TypeScript side with copies typed `satisfies` the
//! view's type (`describeTypes.vitest.ts`), so a renamed or missing key fails
//! `tsc`, and a changed value or shape fails one side or the other.
//!
//! Each fixture fills every optional field and has non-empty arrays, so no
//! key can hide behind a `null` or a `[]`. The values are a wire shape, not
//! a computation: a numeric column carries a level list here only so the
//! field is filled.
//!
//! Also: `describeCodes.json`, the code list the screen's wording is keyed
//! by, is exactly `DescCode::ALL` and `RunCode::ALL`, sorted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use app_lib::stats_desc_commands::{
    DatasetView, ExportView, ResultBundleView, RunStartedView, RunStateView, ValidationReportView,
};
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, InputRef, Os, ParamKey, ParamValue, Platform,
    Reproducibility, ResourceEstimate, RunManifest, RunState, SchemaVersion, SemanticRole,
    ValidationStatus,
};
use gaply_core::stats_desc::{
    Alias, CategoricalSummary, ColumnKind, ColumnProfile, ColumnResult, DatasetProfile, DescCode,
    LevelCount, NumericSummary, Stat, SuspectCode,
};
use gaply_core::stats_l1::dataset::ColumnAlias;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let want = std::fs::read_to_string(fixtures().join("fixtures").join(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OUTPUT: &str = "5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a";

fn level(label: &str, count: u64) -> LevelCount {
    LevelCount {
        label: label.into(),
        count,
    }
}

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn suspect_warning() -> Diagnostic<DescCode> {
    Diagnostic::warning(DescCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(2))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 2)
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    assert_bytes(
        "datasetView.json",
        &DatasetView {
            slot: 3,
            display_name: "survey.csv".into(),
            profile: DatasetProfile {
                rows: 9,
                columns: vec![
                    ColumnProfile {
                        alias: alias(1),
                        header: "group".into(),
                        kind: ColumnKind::Categorical,
                        n: 9,
                        missing: 0,
                        distinct: 2,
                        levels: Some(vec![level("a", 5), level("b", 4)]),
                        suspect_missing_codes: vec![SuspectCode {
                            code: -99,
                            cells: 1,
                        }],
                        non_finite: 0,
                    },
                    ColumnProfile {
                        alias: alias(2),
                        header: "dose".into(),
                        kind: ColumnKind::Numeric,
                        n: 8,
                        missing: 1,
                        distinct: 3,
                        levels: Some(vec![level("1", 4)]),
                        suspect_missing_codes: vec![SuspectCode { code: 99, cells: 2 }],
                        non_finite: 1,
                    },
                ],
            },
            input_ref: InputRef {
                content_hash: hash(INPUT),
                rows: 9,
                columns: 2,
                semantic_role: SemanticRole::Table,
            },
        },
    );
    assert_bytes(
        "validationReportView.json",
        &ValidationReportView {
            status: ValidationStatus::Invalid,
            errors: vec![Diagnostic::error(DescCode::UnknownColumn).int(ParamKey::Column, 9)],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::note(DescCode::NoColumnsSelected)],
            estimated_resources: ResourceEstimate {
                rows: 9,
                columns: 2,
                cells: 18,
            },
        },
    );
    assert_bytes("runStartedView.json", &RunStartedView { run_id: 4 });
    assert_bytes(
        "runStateView.json",
        &RunStateView {
            run_id: 4,
            state: RunState::Running,
            terminal: false,
            columns_done: 1,
            columns_total: 2,
            elapsed_ms: 12,
            cancel_requested: true,
            diagnostics: vec![suspect_warning()],
        },
    );
    assert_bytes(
        "resultBundleView.json",
        &ResultBundleView {
            run_id: 4,
            status: RunState::Succeeded,
            columns: vec![
                ColumnResult::Numeric(NumericSummary {
                    alias: alias(2),
                    header: "dose".into(),
                    n: 8,
                    missing: 1,
                    mean: Stat::Value(52.5),
                    sd: Stat::NotComputable(DescCode::Overflow),
                    min: Stat::Value(10.5),
                    q1: Stat::Value(31.0),
                    median: Stat::Value(50.0),
                    q3: Stat::Value(70.75),
                    max: Stat::Value(95.5),
                }),
                ColumnResult::Categorical(CategoricalSummary {
                    alias: alias(1),
                    header: "group".into(),
                    n: 9,
                    missing: 0,
                    levels: vec![level("a", 5), level("b", 4)],
                }),
            ],
            diagnostics: vec![suspect_warning()],
            manifest: RunManifest {
                schema_version: SchemaVersion,
                capability_id: gaply_core::stats_desc::capability_id(),
                capability_version: 1,
                engine_version: 1,
                inputs: vec![hash(INPUT)],
                parameters_hash: hash(PARAMS),
                seed: Some(7),
                rows_read: 9,
                rows_used: 9,
                exclusions: vec![Exclusion {
                    code: DescCode::MissingCell,
                    count: 1,
                    params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(2))]),
                }],
                platform: Platform {
                    os: Os::Macos,
                    arch: Arch::Aarch64,
                },
                elapsed_ms: 3,
                output_hash: hash(OUTPUT),
                reproducibility: Reproducibility::Bitwise,
            },
        },
    );
    let bytes = b"\"a\"\r\n".to_vec();
    let sha256 = ContentHash::of_bytes(&bytes).as_hex().to_string();
    assert_bytes("exportView.json", &ExportView { bytes, sha256 });
}

/// Evaluation of Phases 2 and 3, F11: the screen words the run ceiling from
/// `describeLimits.json`, so that file is exactly the run manager's ceiling,
/// and a change to one cannot leave the other's sentence false.
#[test]
fn the_screens_run_ceiling_is_the_run_managers() {
    let file: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("describeLimits.json"))
            .expect("describeLimits.json"),
    )
    .unwrap();
    assert_eq!(
        file,
        serde_json::json!({ "runCeilingMs": app_lib::runs::RUN_CEILING_MS }),
        "describeLimits.json and runs::RUN_CEILING_MS differ"
    );
}

#[test]
fn the_screens_code_list_is_exactly_the_closed_sets() {
    let file: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("describeCodes.json")).unwrap(),
    )
    .unwrap();
    let mut rust: Vec<String> = DescCode::ALL.iter().map(|c| c.wire().to_string()).collect();
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.sort();
    assert_eq!(
        file, rust,
        "describeCodes.json and the Rust closed sets differ"
    );
}
