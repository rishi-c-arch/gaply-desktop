//! **The one-sample and paired t-tests' wire contract, from the Rust side
//! (Research OS, Phase 8D-2a, decision 263), in the pattern of
//! `stats_ttest_wire.rs`.** Each view serialises to exactly the bytes of its
//! fixture in `src/screens/statistics/fixtures/` (pretty JSON, two-space
//! indent, a final newline). The TypeScript types that will read them are
//! 8D-2b's (decision 262); until then these fixtures are pinned from this
//! side only.
//!
//! Each fixture fills every field and has non-empty arrays. The values are a
//! wire shape, not a computation.
//!
//! Also: `onesampleCodes.json`, the code list the two screens' wording will
//! be keyed by, is exactly `OneSampleCode::ALL`, `RunCode::ALL`,
//! `WorkspaceCode::ALL` and `CapabilityCode::ALL`, sorted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use app_lib::stats_onesample_commands::{
    OneSampleResultView, OneSampleRunStateView, OneSampleValidationView, PairedResultView,
};
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ResourceEstimate, RunManifest, RunState, SchemaVersion, ValidationStatus,
};
use gaply_core::stats_onesample::{
    one_sample_capability_id, paired_capability_id, ColumnStats, OneSampleCode, OneSampleResult,
    PairedResult,
};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    let want = std::fs::read_to_string(screens().join("fixtures").join(name))
        .unwrap_or_else(|e| panic!("{name}: {e}; the view is:\n{got}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARAMS: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OUTPUT: &str = "5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a";

fn suspect_warning() -> Diagnostic<OneSampleCode> {
    Diagnostic::warning(OneSampleCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 2)
}

fn manifest(
    capability_id: gaply_core::contracts::CapabilityId,
    exclusion: Exclusion<OneSampleCode>,
) -> RunManifest<OneSampleCode> {
    RunManifest {
        schema_version: SchemaVersion,
        capability_id,
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
    }
}

#[test]
fn each_view_serialises_to_exactly_its_fixtures_bytes() {
    assert_bytes(
        "onesampleValidationView.json",
        &OneSampleValidationView {
            status: ValidationStatus::Invalid,
            errors: vec![Diagnostic::error(OneSampleCode::TooFewValues).int(ParamKey::Count, 1)],
            warnings: vec![suspect_warning()],
            missing_information: vec![Diagnostic::note(OneSampleCode::TestValueNotChosen)],
            estimated_resources: ResourceEstimate {
                rows: 10,
                columns: 1,
                cells: 10,
            },
        },
    );
    assert_bytes(
        "onesampleRunStateView.json",
        &OneSampleRunStateView {
            run_id: 5,
            state: RunState::Running,
            terminal: false,
            elapsed_ms: 12,
            cancel_requested: true,
            diagnostics: vec![suspect_warning()],
        },
    );
    let one_excluded = Exclusion {
        code: OneSampleCode::MissingValue,
        count: 1,
        params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(1))]),
    };
    assert_bytes(
        "onesampleResultView.json",
        &OneSampleResultView {
            run_id: 5,
            status: RunState::Succeeded,
            result: OneSampleResult {
                n: 9,
                mean: 63.125,
                sd: 29.25,
                test_value: 50.5,
                t: 1.5,
                df: 8.0,
                p: 0.125,
                mean_difference: 12.625,
                standard_error: 9.75,
                confidence_level: 0.95,
                ci_lower: -9.875,
                ci_upper: 35.125,
                cohens_d: 0.4375,
                exclusions: vec![one_excluded.clone()],
                limitations: vec![
                    OneSampleCode::ValuesNormalityNotAssessed,
                    OneSampleCode::CohensDHasNoInterval,
                ],
            },
            diagnostics: vec![suspect_warning()],
            manifest: manifest(one_sample_capability_id(), one_excluded),
        },
    );
    let pair_excluded = Exclusion {
        code: OneSampleCode::MissingSecond,
        count: 1,
        params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(2))]),
    };
    assert_bytes(
        "pairedResultView.json",
        &PairedResultView {
            run_id: 6,
            status: RunState::Succeeded,
            result: PairedResult {
                n_pairs: 9,
                first: ColumnStats {
                    mean: 63.125,
                    sd: 29.25,
                },
                second: ColumnStats {
                    mean: 33.0625,
                    sd: 22.75,
                },
                t: 2.5,
                df: 8.0,
                p: 0.0375,
                mean_difference: 30.0625,
                sd_differences: 36.0,
                standard_error: 12.0,
                confidence_level: 0.95,
                ci_lower: 2.375,
                ci_upper: 57.75,
                cohens_dz: 0.8125,
                exclusions: vec![pair_excluded.clone()],
                limitations: vec![
                    OneSampleCode::DifferencesNormalityNotAssessed,
                    OneSampleCode::CohensDHasNoInterval,
                ],
            },
            diagnostics: vec![suspect_warning()],
            manifest: manifest(paired_capability_id(), pair_excluded),
        },
    );
}

/// A sorted list of wire codes.
fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_onesample_code_list_is_exactly_the_closed_sets() {
    let mut rust: Vec<String> = OneSampleCode::ALL
        .iter()
        .map(|c| c.wire().to_string())
        .collect();
    assert_eq!(
        rust.len(),
        29,
        "the two tests' 29 codes (the two essentially-constant codes, decision 259)"
    );
    rust.extend(RunCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let listed: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(screens().join("onesampleCodes.json"))
            .expect("onesampleCodes.json"),
    )
    .expect("a list of words");
    assert_eq!(
        listed,
        sorted(rust),
        "onesampleCodes.json and the Rust closed sets differ"
    );
}

/// The two tests run under the same manager as every other kind, so under
/// the same ceiling.
#[test]
fn both_run_under_the_run_managers_ceiling() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    for id in [one_sample_capability_id(), paired_capability_id()] {
        let entry = registry
            .entries()
            .iter()
            .find(|e| e.descriptor.id == id)
            .expect("registered");
        assert_eq!(
            entry.descriptor.resource_profile.ceiling_ms,
            Some(app_lib::runs::RUN_CEILING_MS),
            "{}",
            id.as_str()
        );
    }
}
