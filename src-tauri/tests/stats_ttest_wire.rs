//! **The independent-samples t-test's wire contract, from the Rust side
//! (Research OS, Phase 4B-2), in the pattern of `stats_desc_wire.rs`.** Each
//! new view serialises to exactly the bytes of its fixture in
//! `src/screens/statistics/fixtures/` (pretty JSON, two-space indent, a final
//! newline). `ttestTypes.vitest.ts` deep-compares the same fixtures with
//! copies typed `satisfies` the TypeScript views, so a renamed or missing key
//! fails `tsc`, and a changed value or shape fails one side or the other.
//!
//! Each fixture fills every optional field and has non-empty arrays. The
//! values are a wire shape, not a computation.
//!
//! Also: `ttestCodes.json`, the code list the t-test screen's wording is
//! keyed by, is exactly `TTestCode::ALL`, `RunCode::ALL`, `WorkspaceCode::ALL`
//! and `CapabilityCode::ALL`, sorted; and `capabilityCodes.json`, the method
//! selector's, is exactly `StatsWording::ALL`, sorted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use app_lib::stats_ttest_commands::{
    CapabilityView, GroupLevel, GroupLevelsView, TTestResultView, TTestRunStateView,
    ValidationReportView,
};
use gaply_core::contracts::capability::{CapabilityCode, Lifecycle};
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_registry::StatsWording;
use gaply_core::stats_ttest::{GroupSummary, Standardizer, Stat, TTestCode, TTestResult, Variant};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let want = std::fs::read_to_string(screens().join("fixtures").join(name))
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

fn suspect_warning() -> Diagnostic<TTestCode> {
    Diagnostic::warning(TTestCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 2)
}

fn group(level: u32, label: &str, n: u64, mean: f64, sd: f64) -> GroupSummary {
    GroupSummary {
        level,
        label: label.into(),
        n,
        mean,
        sd,
    }
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    assert_bytes(
        "ttestValidationReportView.json",
        &ValidationReportView {
            status: ValidationStatus::Invalid,
            errors: vec![Diagnostic::error(TTestCode::GroupTooSmall)
                .int(ParamKey::Index, 1)
                .int(ParamKey::Count, 1)],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::note(TTestCode::VariantNotChosen)],
            estimated_resources: ResourceEstimate {
                rows: 10,
                columns: 2,
                cells: 20,
            },
        },
    );
    assert_bytes(
        "ttestRunStateView.json",
        &TTestRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            diagnostics: vec![suspect_warning()],
        },
    );
    let exclusion = Exclusion {
        code: TTestCode::MissingOutcome,
        count: 1,
        params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(1))]),
    };
    assert_bytes(
        "ttestResultView.json",
        &TTestResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: TTestResult {
                variant: Variant::Welch,
                t: 1.5,
                df: 7.25,
                p: 0.125,
                mean_difference: 30.5,
                standard_error: 16.25,
                confidence_level: 0.95,
                ci_lower: -8.5,
                ci_upper: 68.75,
                cohens_d: 1.125,
                d_standardizer: Standardizer::RootMeanVariance,
                d_standardizer_value: 26.5,
                groups: [
                    group(0, "ctrl", 5, 63.125, 29.25),
                    group(1, "trt", 4, 33.0625, 22.75),
                ],
                sd_ratio: Stat::Value(1.25),
                exclusions: vec![exclusion.clone()],
                limitations: vec![
                    TTestCode::NormalityNotAssessed,
                    TTestCode::CohensDHasNoInterval,
                ],
            },
            diagnostics: vec![suspect_warning()],
            manifest: RunManifest {
                schema_version: SchemaVersion,
                capability_id: gaply_core::stats_ttest::capability_id(),
                capability_version: 1,
                engine_version: 1,
                inputs: vec![hash(INPUT)],
                parameters_hash: hash(PARAMS),
                seed: Some(7),
                rows_read: 10,
                rows_used: 9,
                exclusions: vec![exclusion],
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
    // The follow-up's view (decision 81): a group coded 0/1.
    assert_bytes(
        "groupLevelsView.json",
        &GroupLevelsView {
            slot: 3,
            column: gaply_core::stats_desc::Alias(gaply_core::stats_l1::dataset::ColumnAlias(2)),
            levels: vec![
                GroupLevel {
                    index: 0,
                    label: "0".into(),
                    count: 6,
                },
                GroupLevel {
                    index: 1,
                    label: "1".into(),
                    count: 6,
                },
            ],
        },
    );
    assert_bytes(
        "capabilityList.json",
        &vec![CapabilityView {
            id: gaply_core::stats_ttest::capability_id(),
            version: 1,
            lifecycle: Lifecycle::Beta,
            title_code: StatsWording::AnalyzeIndependentTTitle,
            description_code: StatsWording::AnalyzeIndependentTDescription,
            license_notes: vec![StatsWording::LicenseStatrsMit],
            known_limitations: vec![
                StatsWording::LimitAnalyzeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeNormalityNotAssessed,
                StatsWording::LimitAnalyzeCohensDHasNoInterval,
            ],
            runnable: true,
        }],
    );
}

/// A sorted list of wire codes.
fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

fn listed(name: &str) -> Vec<String> {
    serde_json::from_str(&std::fs::read_to_string(screens().join(name)).unwrap())
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn the_ttest_screens_code_list_is_exactly_the_closed_sets() {
    let mut rust: Vec<String> = TTestCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(
        rust.len(),
        30,
        "the t-test's 30 codes (groups_essentially_constant, decision 259)"
    );
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    assert_eq!(
        listed("ttestCodes.json"),
        sorted(rust),
        "ttestCodes.json and the Rust closed sets differ"
    );
}

#[test]
fn the_selectors_wording_list_is_exactly_stats_wording() {
    let rust: Vec<String> = StatsWording::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(
        listed("capabilityCodes.json"),
        sorted(rust),
        "capabilityCodes.json and StatsWording differ"
    );
}

/// Decision 87: the screen's group limit is the core's one constant.
#[test]
fn the_screens_group_limit_is_the_cores() {
    let file: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(screens().join("ttestLimits.json")).expect("ttestLimits.json"),
    )
    .unwrap();
    assert_eq!(
        file,
        serde_json::json!({ "maxGroupLevels": gaply_core::stats_ttest::MAX_GROUP_LEVELS }),
        "ttestLimits.json and stats_ttest::MAX_GROUP_LEVELS differ"
    );
    assert_eq!(gaply_core::stats_ttest::MAX_GROUP_LEVELS, 50, "decision 87");
}

/// The t-test screen words the run ceiling from `describeLimits.json` too,
/// which `stats_desc_wire.rs` pins to `runs::RUN_CEILING_MS`; the t-test runs
/// under the same manager, so the same ceiling.
#[test]
fn the_ttest_runs_under_the_run_managers_ceiling() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    let entry = registry
        .entries()
        .iter()
        .find(|e| e.descriptor.id == gaply_core::stats_ttest::capability_id())
        .expect("registered");
    assert_eq!(
        entry.descriptor.resource_profile.ceiling_ms,
        Some(app_lib::runs::RUN_CEILING_MS)
    );
}
