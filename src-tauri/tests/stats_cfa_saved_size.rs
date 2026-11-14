//! **A saved confirmatory factor analysis's bytes against the read limit
//! (Research OS, Phase 9C-3, decision 781), on `stats_factor_saved_size.rs`'s
//! two tests.** `cfa_cases.rs`'s size test bounds `CfaOutcome` alone; a save
//! stores more. `project_commands::run_result` reads back the spec, the result
//! bundle (the result, its diagnostics, its limitations and the manifest) and the
//! manifest a second time, and refuses the sum above `RESULT_READ_LIMIT_BYTES`.
//!
//! A real run of 128 columns on 16 correlated factors (120 covariance pairs), the
//! widest model the spec allows, goes through the run manager; its copy is taken
//! with `Runs::copy_for_save` without saving it, and the three byte strings a save
//! stores are measured. The result is measured as run, and as an upper bound: the
//! same shape with every `Stat` at the longest not-computable form a `Stat` takes
//! (56 bytes, `distribution_out_of_range`, against a double's widest, 34), every
//! integer at its widest, every exclusion line there can be and every note.
//!
//! The pinned lengths have the manifest's elapsed time, platform and run id set to
//! fixed values first, since those differ run to run and machine to machine; the
//! result as run is measured, not pinned, as its last digits may differ by
//! platform.

mod project_support;

use serde_json::{json, Value};

use app_lib::project_commands::RESULT_READ_LIMIT_BYTES;
use gaply_core::contracts::{Exclusion, OperationId, ParamKey, ParamValue, RunId, RunState};
use gaply_core::stats_cfa::{
    CfaCode as C, CfaErrorVariance, CfaFactorCovariance, CfaFactorVariance, CfaFit, CfaLoading, CfaModelFit,
    CfaOutcome, CfaParams, CfaSmc, Stat, MAX_FACTORS, MAX_ITEMS,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;

use project_support::*;

/// The pinned lengths, measured on macOS aarch64, 14 Nov 2026, in a run with
/// every pin at 0 (~/gaply-corpus/os/phase9/9c3/logs/p0_size_pins0_1794666413.log): the run's saved
/// bytes 88,701; the upper bound with the run's spec, rest and manifest 169,550;
/// with the widest spec, rest and manifest 336,701, 3,857,603 under the read
/// limit.
const SPEC_BYTES: usize = 1195;
const MANIFEST_BYTES: usize = 479;
const BUNDLE_REST_BYTES: usize = 1270;
const WIDEST_BYTES: usize = 166606;
const WIDE_SPEC_BYTES: usize = 26471;
const WIDE_MANIFEST_BYTES: usize = 22973;
const WIDE_BUNDLE_REST_BYTES: usize = 120651;

/// The longest not-computable `Stat` and the widest value `Stat`, as serde_json
/// writes them.
const WIDEST_NOT_COMPUTABLE: Stat = Stat::NotComputable(C::DistributionOutOfRange);
const WIDEST_VALUE: Stat = Stat::Value(-1.2345678901234567e-123);

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// `cfa_cases.rs`'s `structured`, written as a CSV: K factors over n rows and p
/// columns, every value an integer; column j measures factor (j − 1) mod K.
fn structured_csv(n: u64, p: u64, k: u64) -> String {
    let mut out: String = (1..=p).map(|j| format!("x{j}")).collect::<Vec<_>>().join(",");
    out.push('\n');
    for i in 1..=n {
        let row: Vec<String> = (1..=p)
            .map(|j| {
                let f = (j - 1) % k;
                let score = ((i * (7907 + 104729 * f)) % 1013) as f64 - 506.0;
                let shared = ((i * 2654435761) % 1009) as f64 - 504.0;
                let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as f64 - 498.0;
                format!("{}", (2.0 + (j % 3) as f64) * (score + shared) + noise)
            })
            .collect();
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

/// The factors of `structured_csv`: factor f's columns f, f + k, f + 2k, …,
/// correlated.
fn widest_params() -> CfaParams {
    let (p, k) = (MAX_ITEMS as u32, MAX_FACTORS as u32);
    CfaParams {
        indicators: (1..=k).map(|f| (f..=p).step_by(k as usize).map(a).collect()).collect(),
        correlated_factors: true,
        missing_codes: vec![],
    }
}

/// A fit row with every `Stat` `x` and its integers at their widest.
fn row(x: Stat) -> CfaModelFit {
    CfaModelFit {
        npar: u64::MAX,
        cmin: x,
        df: u64::MAX,
        p: x,
        cmin_df: x,
        rmr: x,
        gfi: x,
        agfi: x,
        pgfi: x,
        nfi: x,
        rfi: x,
        ifi: x,
        tli: x,
        cfi: x,
        pratio: x,
        pnfi: x,
        pcfi: x,
        ncp: x,
        ncp_lo90: x,
        ncp_hi90: x,
        fmin: x,
        f0: x,
        f0_lo90: x,
        f0_hi90: x,
        rmsea: x,
        rmsea_lo90: x,
        rmsea_hi90: x,
        pclose: x,
        aic: x,
        bcc: x,
        bic: x,
        caic: x,
        ecvi: x,
        ecvi_lo90: x,
        ecvi_hi90: x,
        mecvi: x,
        hoelter05: x,
        hoelter01: x,
    }
}

/// The upper bound of a result of the run's shape (128 columns, 16 factors, 120
/// pairs): every `Stat` the longest not-computable form, every integer its
/// widest, every exclusion line there can be (missing_value and missing_code
/// under each column, since a row is counted once under its first missing
/// column), the three notes, and the run's limitations.
fn widest(params: &CfaParams, limitations: Vec<C>) -> CfaOutcome {
    let x = WIDEST_NOT_COMPUTABLE;
    let items = params.items();
    let k = params.indicators.len() as u32;
    let factor_of = |i: &Alias| params.indicators.iter().position(|f| f.contains(i)).unwrap() as u32 + 1;
    let pairs: Vec<[u32; 2]> = (1..=k).flat_map(|f| (f + 1..=k).map(move |g| [f, g])).collect();
    CfaOutcome {
        items: items.clone(),
        rows_used: u64::MAX,
        indicators: params.indicators.clone(),
        loadings: items
            .iter()
            .map(|i| CfaLoading { factor: factor_of(i), item: *i, estimate: x, se: x, cr: x, p: x, standardized: x })
            .collect(),
        factor_variances: (1..=k).map(|f| CfaFactorVariance { factor: f, estimate: x, se: x, cr: x, p: x }).collect(),
        factor_covariances: pairs
            .iter()
            .map(|fg| CfaFactorCovariance { factors: *fg, estimate: x, se: x, cr: x, p: x, correlation: x })
            .collect(),
        error_variances: items.iter().map(|i| CfaErrorVariance { item: *i, estimate: x, se: x, cr: x, p: x }).collect(),
        smc: items.iter().map(|i| CfaSmc { item: *i, value: x }).collect(),
        fit: CfaFit { model: row(x), saturated: row(x), independence: row(x) },
        iterations: u64::MAX,
        exclusions: exclusions(),
        notes: vec![C::NegativeVariance, C::FactorCovarianceNotPositiveDefinite, C::DfZero],
        limitations,
    }
}

/// Every exclusion line there can be: missing_value and missing_code under each
/// of the 128 columns, each count the widest.
fn exclusions() -> Vec<Exclusion<C>> {
    (1..=MAX_ITEMS as i64)
        .flat_map(|j| {
            [C::MissingValue, C::MissingCode].map(|code| Exclusion {
                code,
                count: u64::MAX,
                params: std::collections::BTreeMap::from([(ParamKey::Column, ParamValue::Int(j))]),
            })
        })
        .collect()
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

/// The widest run: 128 columns on 16 correlated factors, 600 rows. Returns the
/// world (kept alive), the run's limitations and its copy for a save.
fn widest_run(name: &str) -> (World, Vec<C>, app_lib::runs::RunCopy, CfaOutcome) {
    let w = World::new(name);
    let slot = w.open("wide.csv", structured_csv(600, MAX_ITEMS as u64, MAX_FACTORS as u64).as_bytes());
    let (limitations, copy, result) = {
        let e = w.engine();
        let held = e.datasets.get(slot).expect("the slot");
        let mut spec = gaply_core::stats_cfa::spec(OperationId(17), held.dataset(), widest_params());
        spec.preparation = held.preparation();
        let run = app_lib::stats_cfa_commands::run(&e.runs, &e.datasets, slot, spec).expect("the run starts").run_id;
        let start = std::time::Instant::now();
        while !e.runs.cfa_snapshot(RunId(run)).expect("held").record.state.is_terminal() {
            assert!(start.elapsed() < std::time::Duration::from_secs(60), "the run never ended");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(e.runs.cfa_snapshot(RunId(run)).unwrap().record.state, RunState::Succeeded);
        let result = e.runs.cfa_result(RunId(run)).unwrap().primary_outputs;
        (result.limitations.clone(), e.runs.copy_for_save(RunId(run)).expect("a copy"), result)
    };
    (w, limitations, copy, result)
}

#[test]
fn a_saved_cfa_runs_bytes_with_the_widest_result_fit_the_read_limit() {
    // The two forms the bound rests on, measured.
    assert_eq!(serde_json::to_vec(&WIDEST_NOT_COMPUTABLE).unwrap().len(), 56);
    assert_eq!(serde_json::to_vec(&WIDEST_VALUE).unwrap().len(), 34);
    let (_w, limitations, copy, result) = widest_run("cfa-size");
    assert_eq!((result.items.len(), result.indicators.len(), result.factor_covariances.len()), (128, 16, 120));
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
    println!(
        "as run: saved {saved} B, limit {RESULT_READ_LIMIT_BYTES} B, margin {} B",
        RESULT_READ_LIMIT_BYTES as i64 - saved as i64
    );
    assert!(saved < RESULT_READ_LIMIT_BYTES, "the real run's saved bytes {saved}");
    let widest = serde_json::to_vec(&widest(&widest_params(), limitations)).unwrap().len();
    assert!(widest >= result_len, "the bound {widest} B is below the run's result {result_len} B");
    let total = (spec_len + rest + manifest_len + widest) as u64;
    println!(
        "upper bound: widest result {widest} B, total {total} B, limit {RESULT_READ_LIMIT_BYTES} B, margin {} B ({:.3} MiB)",
        RESULT_READ_LIMIT_BYTES as i64 - total as i64,
        (RESULT_READ_LIMIT_BYTES as i64 - total as i64) as f64 / 1048576.0
    );
    assert!(total < RESULT_READ_LIMIT_BYTES, "{total} B");
    assert_eq!(
        (spec_len, manifest_len, rest, widest),
        (SPEC_BYTES, MANIFEST_BYTES, BUNDLE_REST_BYTES, WIDEST_BYTES),
        "the pinned lengths"
    );
}

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

/// Decision 781: the other test's spec, manifest and bundle rest come from a run
/// with no codes, exclusions or warnings, so its total is not an upper bound.
/// Here each is the widest a save can hold: 128 columns; 1,000 missing codes at
/// the widest JSON a double takes (no method caps the codes), none a candidate; a
/// preparation; 256 exclusion lines (missing_value and missing_code under each
/// column, the most there can be, since a row is counted once under its first
/// missing column), each count the widest; and 768 suspect_missing_code warnings,
/// each column times the six candidate codes (a ticked code is never warned of,
/// and the window offers only those six). With the widest result, the total is
/// under the read limit.
#[test]
fn the_widest_spec_manifest_and_warnings_with_the_widest_result_fit_the_read_limit() {
    use gaply_core::contracts::{ContentHash, Diagnostic, Preparation, Severity};
    use gaply_core::stats_l1::binding::MISSING_CODE_CANDIDATES;
    let (_w, limitations, copy, _) = widest_run("cfa-size-wide");
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
    let exclusions = exclusions();
    assert_eq!(exclusions.len(), 256);
    let mut manifest: Value = serde_json::from_slice(&copy.manifest_json.expect("manifest")).unwrap();
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
    let widest = serde_json::to_vec(&widest(&widest_params(), limitations)).unwrap().len();
    let total = (spec_len + rest + manifest_len + widest) as u64;
    println!(
        "widest: result {widest} B, total {total} B, limit {RESULT_READ_LIMIT_BYTES} B, margin {} B ({:.3} MiB)",
        RESULT_READ_LIMIT_BYTES as i64 - total as i64,
        (RESULT_READ_LIMIT_BYTES as i64 - total as i64) as f64 / 1048576.0
    );
    assert!(total < RESULT_READ_LIMIT_BYTES, "{total} B");
    assert_eq!(
        (spec_len, manifest_len, rest),
        (WIDE_SPEC_BYTES, WIDE_MANIFEST_BYTES, WIDE_BUNDLE_REST_BYTES),
        "the pinned lengths"
    );
}
