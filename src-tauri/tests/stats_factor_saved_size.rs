//! **A saved factor analysis's bytes against the read limit (Research OS, Phase
//! 9B-2, decision 714; m3 of the review of 9B-1).** `factor_cases.rs`'s size test
//! bounds `FactorOutcome` alone; a save stores more. `project_commands::run_result`
//! reads back the spec, the result bundle (the result, its diagnostics, its
//! limitations and the manifest) and the manifest a second time, and refuses the
//! sum above `RESULT_READ_LIMIT_BYTES` (project_commands.rs, `run_result`).
//!
//! A real 128-item principal components run, with no rotation, goes through the run
//! manager; its copy is taken with `Runs::copy_for_save` without saving it, and the
//! three byte strings a save stores (`request_for`) are measured. The bundle's rest
//! is its length less the result's own bytes. To that rest, the spec and the
//! manifest the widest `FactorOutcome` is added, built as `factor_cases.rs`'s
//! `widest()` builds it, and the total must be under the limit.
//!
//! The pinned lengths have the manifest's elapsed time, platform and run id set to
//! fixed values first, since those differ run to run and machine to machine; the
//! result's own floats are measured, not pinned, as their last digit may differ by
//! platform.

mod project_support;

use serde_json::{json, Value};

use app_lib::project_commands::RESULT_READ_LIMIT_BYTES;
use gaply_core::contracts::{Exclusion, OperationId, ParamKey, ParamValue, RunId, RunState};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_factor::{
    Communality, Determinant, ExtractedFactors, Extraction, FactorCode as C, FactorOutcome, FactorParams,
    FactorSums, ItemAdequacy, ItemDescriptive, RotatedFactors, Rotation, SamplingAdequacy, Sphericity, Stat,
    MAX_ITEMS,
};
use gaply_core::stats_l1::dataset::ColumnAlias;

use project_support::*;

/// The pinned lengths, measured on macOS aarch64, 12 Nov 2026
/// (~/gaply-corpus/os/phase9/9b2/logs/p0_app_red_1794456016.log, a run with every
/// pin at 0). The spec and the bundle's rest are both 1,241 bytes by coincidence;
/// the rest is checked a second way in the test. The widest results were 9B-1's
/// measurement of `widest()` (2,982,566 and 2,950,990 bytes) with three notes;
/// decision 717's fourth, `"stats.factor.extraction_eigenvalue_negative"` and its
/// comma, adds 46 bytes (rotation_not_converged and one_factor_not_rotated are
/// both 22 characters).
const SPEC_BYTES: usize = 1241;
const MANIFEST_BYTES: usize = 482;
const BUNDLE_REST_BYTES: usize = 1241;
const WIDEST_PC_BYTES: usize = 2982612;
const WIDEST_PA_BYTES: usize = 2951036;

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// factor_cases.rs's `structured`: K factors over n rows and p items, every value
/// an integer.
fn structured_csv(n: u64, p: u64, k: u64) -> String {
    let mut out: String = (1..=p).map(|j| format!("x{j}")).collect::<Vec<_>>().join(",");
    out.push('\n');
    for i in 1..=n {
        let row: Vec<String> = (1..=p)
            .map(|j| {
                let f = (j - 1) % k;
                let score = ((i * 7907 + f * 15485863) % 1013) as f64 - 506.0;
                let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as f64 - 498.0;
                format!("{}", 2.0 * score + noise)
            })
            .collect();
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

/// The twin of `gaply-core/tests/factor_cases.rs`'s `widest()`, copied: a helper
/// cannot be shared between a gaply-core test and an app test. The largest result
/// of a shape the spec allows: 128 items, `m` factors and promax, every number the
/// widest JSON a double takes and every item with two exclusion lines.
fn widest(m: usize, extraction: Extraction, limitations: Vec<C>) -> FactorOutcome {
    let p = MAX_ITEMS;
    let x = Stat::Value(-1.2345678901234567e-123);
    let c = Stat::NotComputable(C::CorrelationsSingular);
    let square = |rows: usize| vec![vec![x; m]; rows];
    let sums = |k: usize| vec![FactorSums { total: x, percent: x, cumulative: x }; k];
    let items: Vec<Alias> = (1..=p as u32).map(a).collect();
    FactorOutcome {
        items: items.clone(),
        rows_used: u64::MAX,
        descriptives: items.iter().map(|i| ItemDescriptive { item: *i, mean: x, sd: x }).collect(),
        determinant: Determinant { value: c, log: c },
        kmo: SamplingAdequacy { overall: c, items: items.iter().map(|i| ItemAdequacy { item: *i, msa: c }).collect() },
        bartlett: Sphericity { chi_square: c, df: u64::MAX, p: c },
        eigenvalues: sums(p),
        extraction: ExtractedFactors {
            method: extraction,
            factors: m as u64,
            iterations: u64::MAX,
            communalities: items.iter().map(|i| Communality { item: *i, initial: x, extraction: x }).collect(),
            sums: sums(m),
            loadings: square(p),
        },
        rotation: Some(RotatedFactors {
            method: Rotation::Promax,
            iterations: u64::MAX,
            loadings: square(p),
            transformation: Some(square(m)),
            sums: Some(sums(m)),
            structure: Some(square(p)),
            factor_correlations: Some(square(m)),
        }),
        exclusions: items
            .iter()
            .flat_map(|i| {
                [C::MissingValue, C::MissingCode].map(|code| Exclusion {
                    code,
                    count: u64::MAX,
                    params: std::collections::BTreeMap::from([(ParamKey::Column, ParamValue::Int(i64::from(i.0 .0)))]),
                })
            })
            .collect(),
        // The most notes one result can hold (decision 717): at most one of
        // one_factor_not_rotated, rotation_not_converged and rotation_singular,
        // the longest of them, with the three that can join it.
        notes: vec![C::CommunalityAboveOne, C::ExtractionEigenvalueNegative, C::RotationNotConverged, C::EigenvaluesNotDistinct],
        limitations,
    }
}

/// The JSON's length with the manifest's elapsed time, platform and the bundle's
/// run id fixed. Re-serialising a parsed value keeps every length: keys may be
/// reordered, nothing else changes.
fn normalised_len(bytes: &[u8]) -> usize {
    let mut v: Value = serde_json::from_slice(bytes).expect("JSON");
    let fix = |m: &mut Value| {
        m["elapsedMs"] = json!(0);
        m["platform"] = json!({ "os": "macos", "arch": "aarch64" });
    };
    if v.get("manifest").is_some() {
        fix(&mut v["manifest"]);
        v["runId"] = json!(1);
    } else {
        fix(&mut v);
    }
    serde_json::to_vec(&v).unwrap().len()
}

#[test]
fn a_saved_factor_runs_bytes_with_the_widest_result_fit_the_read_limit() {
    let w = World::new("factor-size");
    let slot = w.open("wide.csv", structured_csv(400, MAX_ITEMS as u64, 8).as_bytes());
    let e = w.engine();
    let held = e.datasets.get(slot).expect("the slot");
    let mut spec = gaply_core::stats_factor::spec(
        OperationId(16),
        held.dataset(),
        FactorParams {
            items: (1..=MAX_ITEMS as u32).map(a).collect(),
            extraction: Extraction::PrincipalComponents,
            factors: None,
            rotation: Rotation::None,
            iterations: 25,
            convergence: 0.001,
            missing_codes: vec![],
        },
    );
    spec.preparation = held.preparation();
    let run = app_lib::stats_factor_commands::run(&e.runs, &e.datasets, slot, spec).expect("the run starts").run_id;
    let start = std::time::Instant::now();
    while !e.runs.factor_snapshot(RunId(run)).expect("held").record.state.is_terminal() {
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "the run never ended");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(e.runs.factor_snapshot(RunId(run)).unwrap().record.state, RunState::Succeeded);
    let limitations = e.runs.factor_result(RunId(run)).unwrap().primary_outputs.limitations;
    let copy = e.runs.copy_for_save(RunId(run)).expect("a copy");
    let (spec_json, bundle_json, manifest_json) =
        (copy.spec_json.expect("spec"), copy.bundle_json.expect("bundle"), copy.manifest_json.expect("manifest"));
    let bundle: Value = serde_json::from_slice(&bundle_json).unwrap();
    let result_len = serde_json::to_vec(&bundle["primaryOutputs"]).unwrap().len();
    let rest = normalised_len(&bundle_json) - result_len;
    // A second route to the rest: the bundle with its result replaced by `0`, one
    // byte, so the two subtractions cannot share a mistake.
    let mut without = bundle.clone();
    without["primaryOutputs"] = json!(0);
    assert_eq!(normalised_len(&serde_json::to_vec(&without).unwrap()) - 1, rest, "the bundle's rest, two ways");
    let (spec_len, manifest_len) = (spec_json.len(), normalised_len(&manifest_json));
    println!(
        "spec {spec_len} B, bundle {} B (its result {result_len} B, the rest {rest} B), manifest {manifest_len} B",
        bundle_json.len()
    );
    let saved = (spec_json.len() + bundle_json.len() + manifest_json.len()) as u64;
    assert!(saved < RESULT_READ_LIMIT_BYTES, "the real run's saved bytes {saved}");
    let mut widest_lens = Vec::new();
    for (m, extraction) in [(128, Extraction::PrincipalComponents), (127, Extraction::PrincipalAxis)] {
        let widest = serde_json::to_vec(&widest(m, extraction, limitations.clone())).unwrap().len();
        let total = (spec_len + rest + manifest_len + widest) as u64;
        println!(
            "{m} factors: widest result {widest} B, total {total} B, limit {RESULT_READ_LIMIT_BYTES} B, margin {} B ({:.3} MiB)",
            RESULT_READ_LIMIT_BYTES as i64 - total as i64,
            (RESULT_READ_LIMIT_BYTES as i64 - total as i64) as f64 / 1048576.0
        );
        assert!(total < RESULT_READ_LIMIT_BYTES, "{m} factors: {total} B");
        widest_lens.push(widest);
    }
    assert_eq!(
        (spec_len, manifest_len, rest, widest_lens[0], widest_lens[1]),
        (SPEC_BYTES, MANIFEST_BYTES, BUNDLE_REST_BYTES, WIDEST_PC_BYTES, WIDEST_PA_BYTES),
        "the pinned lengths"
    );
}

/// The pinned lengths of the widest spec, bundle rest and manifest (decision
/// 728, m3 of the review of 9B-2), measured on macOS aarch64, 13 Nov 2026, in a
/// run with every pin at 0 (~/gaply-corpus/os/phase9/9b3/logs/p0_green_1794520972.log):
/// with the widest 128-component result, a total of 3,156,567 bytes, 1,037,737
/// under the read limit.
const WIDE_SPEC_BYTES: usize = 26517;
const WIDE_MANIFEST_BYTES: usize = 23744;
const WIDE_BUNDLE_REST_BYTES: usize = 123694;

/// `n` distinct doubles, each written by serde_json at the widest a double's JSON
/// takes (24 characters, `-1.2345678901234567e-123`), none of them a candidate
/// code.
fn widest_codes(n: usize) -> Vec<f64> {
    let base = (-1.2345678901234567e-123f64).to_bits();
    let mut out = Vec::new();
    let mut k = 0u64;
    while out.len() < n {
        let x = f64::from_bits(base + k);
        k += 1;
        if serde_json::to_string(&x).unwrap().len() == 24 {
            out.push(x);
        }
    }
    out
}

/// Decision 728 (m3 of the review of 9B-2): the other test's spec, manifest and
/// bundle rest come from a run with no codes, exclusions or warnings, so its total
/// is not an upper bound. Here each is the widest a save can hold: 128 items; 1,000
/// missing codes at the widest JSON a double takes (no method caps the codes; a
/// later step's question), none a candidate; a preparation; 256 exclusion lines
/// (missing_value and missing_code under each item, the most there can be, since a
/// row is counted once under its first item), each count the widest; and 768
/// suspect_missing_code warnings, each item times the six candidate codes (a
/// ticked code is never warned of, and the window offers only those six). With the
/// widest result, the total is under the read limit.
#[test]
fn the_widest_spec_manifest_and_warnings_with_the_widest_result_fit_the_read_limit() {
    use gaply_core::contracts::{ContentHash, Diagnostic, Preparation, Severity};
    use gaply_core::stats_l1::binding::MISSING_CODE_CANDIDATES;
    let w = World::new("factor-size-wide");
    let slot = w.open("wide.csv", structured_csv(400, MAX_ITEMS as u64, 8).as_bytes());
    let e = w.engine();
    let held = e.datasets.get(slot).expect("the slot");
    let spec = gaply_core::stats_factor::spec(
        OperationId(16),
        held.dataset(),
        FactorParams {
            items: (1..=MAX_ITEMS as u32).map(a).collect(),
            extraction: Extraction::PrincipalComponents,
            factors: None,
            rotation: Rotation::None,
            iterations: 25,
            convergence: 0.001,
            missing_codes: vec![],
        },
    );
    let run = app_lib::stats_factor_commands::run(&e.runs, &e.datasets, slot, spec).expect("the run starts").run_id;
    let start = std::time::Instant::now();
    while !e.runs.factor_snapshot(RunId(run)).expect("held").record.state.is_terminal() {
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "the run never ended");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(e.runs.factor_snapshot(RunId(run)).unwrap().record.state, RunState::Succeeded);
    let limitations = e.runs.factor_result(RunId(run)).unwrap().primary_outputs.limitations;
    let copy = e.runs.copy_for_save(RunId(run)).expect("a copy");
    let preparation = serde_json::to_value(Preparation {
        source_hash: ContentHash::of_bytes(b"source"),
        recipe_hash: ContentHash::of_bytes(b"recipe"),
        source_columns: u32::MAX,
        capability_version: u32::MAX,
        engine_version: u32::MAX,
    })
    .unwrap();
    let codes = widest_codes(1000);
    assert!(codes.iter().all(|c| !MISSING_CODE_CANDIDATES.iter().any(|k| f64::from(*k) == *c)));
    let mut spec: Value = serde_json::from_slice(&copy.spec_json.expect("spec")).unwrap();
    spec["parameters"]["missingCodes"] = json!(codes);
    spec["preparation"] = preparation.clone();
    assert!(spec.get("randomSeed").is_some(), "the spec's seed key");
    spec["randomSeed"] = json!(u64::MAX);
    let exclusions: Vec<Exclusion<C>> = (1..=MAX_ITEMS as i64)
        .flat_map(|j| {
            [C::MissingValue, C::MissingCode].map(|code| Exclusion {
                code,
                count: u64::MAX,
                params: std::collections::BTreeMap::from([(ParamKey::Column, ParamValue::Int(j))]),
            })
        })
        .collect();
    assert_eq!(exclusions.len(), 256);
    let mut manifest: Value = serde_json::from_slice(&copy.manifest_json.expect("manifest")).unwrap();
    // The preparation is skipped when absent, so a prepared slot's run adds it.
    for key in ["exclusions", "rowsRead", "rowsUsed", "seed"] {
        assert!(manifest.get(key).is_some(), "the manifest's key {key}");
    }
    manifest["exclusions"] = serde_json::to_value(&exclusions).unwrap();
    manifest["rowsRead"] = json!(u64::MAX);
    manifest["rowsUsed"] = json!(u64::MAX);
    manifest["seed"] = json!(u64::MAX);
    manifest["preparation"] = preparation;
    let warnings: Vec<Diagnostic<C>> = (1..=MAX_ITEMS as i64)
        .flat_map(|j| {
            MISSING_CODE_CANDIDATES.map(|k| {
                let mut d = Diagnostic::new(Severity::Warning, C::SuspectMissingCode);
                d.params.insert(ParamKey::Column, ParamValue::Int(j));
                d.params.insert(ParamKey::Value, ParamValue::Int(i64::from(k)));
                d.params.insert(ParamKey::Count, ParamValue::Int(i64::MAX));
                d
            })
        })
        .collect();
    assert_eq!(warnings.len(), 768);
    let mut bundle: Value = serde_json::from_slice(&copy.bundle_json.expect("bundle")).unwrap();
    bundle["primaryOutputs"] = json!(0);
    bundle["diagnostics"] = serde_json::to_value(&warnings).unwrap();
    bundle["manifest"] = manifest.clone();
    let spec_len = serde_json::to_vec(&spec).unwrap().len();
    let manifest_len = normalised_len(&serde_json::to_vec(&manifest).unwrap());
    let rest = normalised_len(&serde_json::to_vec(&bundle).unwrap()) - 1;
    println!("wide spec {spec_len} B, wide bundle rest {rest} B, wide manifest {manifest_len} B");
    for (m, extraction) in [(128, Extraction::PrincipalComponents), (127, Extraction::PrincipalAxis)] {
        let widest = serde_json::to_vec(&widest(m, extraction, limitations.clone())).unwrap().len();
        let total = (spec_len + rest + manifest_len + widest) as u64;
        println!(
            "{m} factors: widest result {widest} B, total {total} B, limit {RESULT_READ_LIMIT_BYTES} B, margin {} B ({:.3} MiB)",
            RESULT_READ_LIMIT_BYTES as i64 - total as i64,
            (RESULT_READ_LIMIT_BYTES as i64 - total as i64) as f64 / 1048576.0
        );
        assert!(total < RESULT_READ_LIMIT_BYTES, "{m} factors: {total} B");
    }
    assert_eq!(
        (spec_len, manifest_len, rest),
        (WIDE_SPEC_BYTES, WIDE_MANIFEST_BYTES, WIDE_BUNDLE_REST_BYTES),
        "the pinned lengths"
    );
}
