//! **A saved structural equation model's bytes against the read limit (Research OS, Phase
//! 9C-5, decisions 816 and 829), on `stats_cfa_saved_size.rs`'s two tests.** `sem_cases.rs`'s
//! size test bounds `SemOutcome` alone; a save stores more. `project_commands::run_result`
//! reads back the spec, the result bundle (the result, its diagnostics, its limitations and
//! the manifest) and the manifest a second time, and refuses the sum above
//! `RESULT_READ_LIMIT_BYTES`.
//!
//! The widest result 9C-4 measured is built where 9C-4 built it (`sem_cases.rs`'s
//! `the_widest_result_is_under_2_mib`, decision 800): 128 columns, 16 factors of five columns
//! and 48 columns no factor holds; 64 paths in one loop through the 64 structural variables, so
//! 4,032 effects; 256 covariances; the same splitmix64 draws over 400 rows, written as a CSV
//! with every double in its shortest round-trip form. It goes through the run manager; its copy
//! is taken with `Runs::copy_for_save` without saving it, and the three byte strings a save
//! stores are measured. The result is measured as run, and as an upper bound: its JSON with
//! every `Stat` at the longest form a `Stat` takes (measured over every `SemCode` and the
//! widest double), every count at its widest, every exclusion line there can be, every note,
//! and each variable's unit exponent at its longest, −537 (decision 846).
//!
//! The pinned lengths have the manifest's elapsed time, platform and run id set to fixed
//! values first, since those differ run to run and machine to machine; the result as run is
//! measured, not pinned, as its last digits may differ by platform.

mod project_support;

use serde_json::{json, Value};

use app_lib::project_commands::RESULT_READ_LIMIT_BYTES;
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{Exclusion, OperationId, ParamKey, ParamValue, RunId, RunState};
use gaply_core::stats_sem::{SemCode as C, SemOutcome, SemParams, Stat, MAX_COLUMNS, MAX_COVARIANCES, MAX_EQUAL_MEMBERS, MAX_PATHS};

use project_support::*;

/// The pinned lengths, measured on macOS aarch64, 17 Nov 2026, with the result's unitExponents
/// (Research OS, Phase 9C-5f, decision 846; ~/gaply-corpus/os/phase9/9c5f/logs/
/// size_measure_1794867650.log): the run's result 1,331,290 bytes, and its saved bytes 1,348,531;
/// the upper bound, each unit widened to −537, with the run's spec, rest and manifest 2,384,522
/// (15,130 + 479 + 1,632 + 2,367,281); with the wide spec, rest and manifest 2,559,352 (48,085 +
/// 22,973 + 121,013 + 2,367,281), 1,634,952 under the read limit. Before the units (16 Nov 2026,
/// 9c5c/fix_app_1794810079.log) the widest result was 2,366,543 and the margin 1,635,690.
const SPEC_BYTES: usize = 15130;
const MANIFEST_BYTES: usize = 479;
const BUNDLE_REST_BYTES: usize = 1632;
const WIDEST_BYTES: usize = 2367281;
const WIDE_SPEC_BYTES: usize = 48085;
const WIDE_MANIFEST_BYTES: usize = 22973;
const WIDE_BUNDLE_REST_BYTES: usize = 121013;

/// The widest value `Stat`, as serde_json writes it: a double at its widest, 24 characters.
const WIDEST_VALUE: Stat = Stat::Value(-1.2345678901234567e-123);

/// The longest JSON a `Stat` takes: the widest double's, or a not-computable form's, the
/// longest over every `SemCode`.
fn widest_stat() -> Value {
    let mut best = serde_json::to_value(WIDEST_VALUE).unwrap();
    for code in C::ALL {
        let v = serde_json::to_value(Stat::NotComputable(*code)).unwrap();
        if v.to_string().len() > best.to_string().len() {
            best = v;
        }
    }
    best
}

/// `sem_cases.rs`'s widest model's data, by its generator, as a CSV.
fn widest_csv() -> String {
    let n = 400usize;
    let mut g = 12345u64;
    let mut normal = || {
        let mut s = 0.0;
        for _ in 0..12 {
            g = g.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = g;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            s += ((z ^ (z >> 31)) >> 11) as f64 / 9_007_199_254_740_992.0;
        }
        s - 6.0
    };
    let zeta: Vec<Vec<f64>> = (0..64).map(|_| (0..n).map(|_| normal()).collect()).collect();
    let mut eta = zeta.clone();
    for _ in 0..60 {
        eta = (0..64).map(|i| (0..n).map(|r| 0.3 * eta[(i + 63) % 64][r] + zeta[i][r]).collect()).collect();
    }
    let mut cols: Vec<Vec<f64>> = Vec::new();
    for f in &eta[..16] {
        for _ in 0..5 {
            cols.push(f.iter().map(|x| x + 0.7 * normal()).collect());
        }
    }
    cols.extend(eta[16..].iter().cloned());
    let mut s: String = (1..=cols.len()).map(|j| format!("v{j}")).collect::<Vec<_>>().join(",");
    s.push('\n');
    for r in 0..n {
        s.push_str(&cols.iter().map(|c| format!("{}", c[r])).collect::<Vec<_>>().join(","));
        s.push('\n');
    }
    s
}

/// `sem_cases.rs`'s widest model's spec, as it builds it.
fn widest_params() -> SemParams {
    let indicators: Vec<String> = (0..16).map(|f| format!("[{}]", (1..=5).map(|j| format!("\"C{}\"", 5 * f + j)).collect::<Vec<_>>().join(","))).collect();
    let var = |i: usize| if i < 16 { format!(r#"{{"factor":{}}}"#, i + 1) } else { format!(r#"{{"column":"C{}"}}"#, 80 + i - 15) };
    let paths: Vec<String> = (0..64).map(|to| format!(r#"{{"to":{},"from":{}}}"#, var(to), var((to + 63) % 64))).collect();
    let mut pairs = Vec::new();
    for gap in [5u32, 10, 15, 20] {
        for a in 1..=80 - gap {
            pairs.push((a, a + gap));
        }
    }
    pairs.sort_unstable();
    pairs.truncate(256);
    let covs: Vec<String> = pairs.iter().map(|(a, b)| format!(r#"{{"a":{{"column":"C{a}"}},"b":{{"column":"C{b}"}}}}"#)).collect();
    let json = format!(r#"{{"indicators":[{}],"paths":[{}],"covariances":[{}],"equal":[],"missingCodes":[]}}"#, indicators.join(","), paths.join(","), covs.join(","));
    let p: SemParams = serde_json::from_str(&json).unwrap();
    assert_eq!((p.paths.len(), p.covariances.len(), p.columns().len()), (MAX_PATHS, MAX_COVARIANCES, MAX_COLUMNS));
    p
}

/// Every exclusion line there can be: missing_value and missing_code under each of the 128
/// columns, each count the widest.
fn exclusions() -> Vec<Exclusion<C>> {
    (1..=MAX_COLUMNS as i64)
        .flat_map(|j| {
            [C::MissingValue, C::MissingCode].map(|code| Exclusion {
                code,
                count: u64::MAX,
                params: std::collections::BTreeMap::from([(ParamKey::Column, ParamValue::Int(j))]),
            })
        })
        .collect()
}

/// The upper bound of a result of this model's shape: its JSON with every `Stat` at its
/// longest, `rowsUsed`, `iterations`, `attempt`, `npar` and `df` at the widest integer, every
/// exclusion line and every note the method writes. Returns the bound's length and how many
/// `Stat`s it widened, so a walk that matched nothing cannot pass.
fn widest(result: &SemOutcome) -> (usize, usize) {
    fn walk(v: &mut Value, stat: &Value, n: &mut usize) {
        match v {
            Value::Object(m) => {
                if m.len() == 1 && (m.contains_key("value") || m.contains_key("not_computable")) {
                    *v = stat.clone();
                    *n += 1;
                    return;
                }
                for (k, x) in m.iter_mut() {
                    if ["rowsUsed", "iterations", "attempt", "npar", "df"].contains(&k.as_str()) && x.is_u64() {
                        *x = json!(u64::MAX);
                    } else {
                        walk(x, stat, n);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| walk(x, stat, n)),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(result).unwrap();
    let mut n = 0;
    walk(&mut v, &widest_stat(), &mut n);
    v["exclusions"] = serde_json::to_value(exclusions()).unwrap();
    v["notes"] = serde_json::to_value([C::NegativeVariance, C::CovarianceNotPositiveDefinite, C::Nonrecursive, C::DfZero]).unwrap();
    // Each variable's unit at its longest form: −537, the widest exponent in [−537, 511]
    // (Research OS, Phase 9C-5f, decision 846).
    let units = v["unitExponents"].as_array().expect("the result's units").len();
    assert_eq!(units, result.variances.len(), "one unit per variable");
    v["unitExponents"] = json!(vec![-537; units]);
    (serde_json::to_vec(&v).unwrap().len(), n)
}

/// The JSON's length with the manifest's elapsed time, platform and the bundle's run id
/// fixed. Re-serialising a parsed value keeps every length: keys may be reordered, nothing
/// else changes.
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

/// The widest run, on a still clock (`TestClock`, never advanced), so the run manager's
/// 30 s ceiling cannot stop it: an unoptimised build takes longer than that on this model
/// (measured: `TimedOut` on the default clock, 9c5c/p2_size_pins0_1794808316.log). Returns the
/// world (kept alive), the slot, its copy for a save, and its result.
fn widest_run(name: &str) -> (World, u64, app_lib::runs::RunCopy, SemOutcome) {
    let runs = std::sync::Arc::new(app_lib::runs::Runs::with_hooks(std::sync::Arc::new(TestClock::default()), None, 30_000));
    let w = World::with(name, SMALL, Some(runs));
    let slot = w.open("wide.csv", widest_csv().as_bytes());
    let (copy, result) = {
        let e = w.engine();
        let held = e.datasets.get(slot).expect("the slot");
        let mut spec = gaply_core::stats_sem::spec(OperationId(18), held.dataset(), widest_params());
        spec.preparation = held.preparation();
        drop(held);
        let run = app_lib::stats_sem_commands::run(&e.runs, &e.datasets, slot, spec).expect("the run starts").run_id;
        let start = std::time::Instant::now();
        while !e.runs.sem_snapshot(RunId(run)).expect("held").record.state.is_terminal() {
            assert!(start.elapsed() < std::time::Duration::from_secs(120), "the run never ended");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let snap = e.runs.sem_snapshot(RunId(run)).unwrap();
        assert_eq!(snap.record.state, RunState::Succeeded, "{:?}", snap.diagnostics);
        let result = e.runs.sem_result(RunId(run)).unwrap().primary_outputs;
        (e.runs.copy_for_save(RunId(run)).expect("a copy"), result)
    };
    (w, slot, copy, result)
}

#[test]
fn a_saved_sem_runs_bytes_with_the_widest_result_fit_the_read_limit() {
    // The form the bound rests on, measured.
    assert_eq!(serde_json::to_vec(&WIDEST_VALUE).unwrap().len(), 34);
    let stat = widest_stat();
    println!("the longest Stat: {} B, {stat}", stat.to_string().len());
    let (_w, _, copy, result) = widest_run("sem-size");
    assert_eq!((result.columns.len(), result.indicators.len(), result.effects.len()), (128, 16, 64 * 63));
    println!("the run: attempt {}, {} iterations", result.attempt, result.iterations);
    let (spec_json, bundle_json, manifest_json) =
        (copy.spec_json.expect("spec"), copy.bundle_json.expect("bundle"), copy.manifest_json.expect("manifest"));
    let bundle: Value = serde_json::from_slice(&bundle_json).unwrap();
    let result_len = serde_json::to_vec(&bundle["primaryOutputs"]).unwrap().len();
    let rest = normalised_len(&bundle_json) - result_len;
    // A second route to the rest: the bundle with its result replaced by `0`, one byte, so the
    // two subtractions cannot share a mistake.
    let mut without = bundle.clone();
    without["primaryOutputs"] = json!(0);
    assert_eq!(normalised_len(&serde_json::to_vec(&without).unwrap()) - 1, rest, "the bundle's rest, two ways");
    let (spec_len, manifest_len) = (spec_json.len(), normalised_len(&manifest_json));
    let saved = (spec_json.len() + bundle_json.len() + manifest_json.len()) as u64;
    println!(
        "as run: spec {spec_len} B, bundle {} B (its result {result_len} B, the rest {rest} B), manifest {manifest_len} B; saved {saved} B, limit {RESULT_READ_LIMIT_BYTES} B",
        bundle_json.len()
    );
    assert!(saved < RESULT_READ_LIMIT_BYTES, "the real run's saved bytes {saved}");
    let (widest, stats) = widest(&result);
    assert!(stats > 0, "the bound widened no Stat: its walk is inert");
    assert!(widest >= result_len, "the bound {widest} B is below the run's result {result_len} B");
    let total = (spec_len + rest + manifest_len + widest) as u64;
    println!(
        "upper bound: {stats} Stats widened, widest result {widest} B, total {total} B, margin {} B ({:.3} MiB)",
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

/// `n` distinct doubles, each written by serde_json at the widest a double's JSON takes (24
/// characters, `-1.2345678901234567e-123`), none of them a candidate code.
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

/// As the CFA's (decision 781): the other test's spec, manifest and bundle rest come from a
/// run with no codes, groups, exclusions or warnings, so its total is not an upper bound.
/// Here each is the widest a save can hold: the widest model's spec with groups held equal
/// totalling MAX_EQUAL_MEMBERS members, in groups of two of one kind (the most groups, so the
/// most brackets), the members the model's 128 longest paths and covariances as JSON, checked
/// by the method's own validation. It is wide, not the widest any spec could be: its aliases
/// are this model's, and a model whose covariances joined C100 to C128 would write one or two
/// bytes more per alias (the review of this step, finding 3; a few kilobytes against a margin
/// of over 1.5 MiB). Then 1,000 missing codes at the widest
/// JSON a double takes, none a candidate; a preparation; 256 exclusion lines, each count the
/// widest; and 768 suspect_missing_code warnings, each column times the six candidate codes.
/// With the widest result, the total is under the read limit.
#[test]
fn the_widest_spec_manifest_and_warnings_with_the_widest_result_fit_the_read_limit() {
    use gaply_core::contracts::{ContentHash, Diagnostic, Preparation, Severity};
    use gaply_core::stats_l1::binding::MISSING_CODE_CANDIDATES;
    use gaply_core::stats_sem::SemParameter;
    let (w, slot, copy, result) = widest_run("sem-size-wide");
    // Groups of two of one kind: the 128 longest of the model's paths and covariances as JSON.
    let mut params = widest_params();
    let mut members: Vec<SemParameter> = params
        .paths
        .iter()
        .map(|p| SemParameter::Path(*p))
        .chain(params.covariances.iter().map(|c| SemParameter::Covariance(*c)))
        .collect();
    members.sort_by_key(|m| (std::cmp::Reverse(serde_json::to_string(m).unwrap().len()), *m));
    members.truncate(MAX_EQUAL_MEMBERS);
    members.sort();
    assert_eq!(members.len(), MAX_EQUAL_MEMBERS);
    let (paths, covs): (Vec<SemParameter>, Vec<SemParameter>) = members.into_iter().partition(|m| matches!(m, SemParameter::Path(_)));
    assert!(paths.len() % 2 == 0 && covs.len() % 2 == 0, "pairs within a kind: {} paths, {} covariances", paths.len(), covs.len());
    println!("the held-equal members: {} paths, {} covariances", paths.len(), covs.len());
    let mut groups: Vec<Vec<SemParameter>> = paths.chunks(2).chain(covs.chunks(2)).map(|g| g.to_vec()).collect();
    groups.sort();
    params.equal = groups;
    {
        // A spec a save can hold: the method's own validation accepts it on the run's table.
        let e = w.engine();
        let held = e.datasets.get(slot).expect("the slot");
        let typed = gaply_core::stats_sem::spec(OperationId(18), held.dataset(), params.clone());
        let report = gaply_core::stats_sem::validate(held.dataset(), &typed);
        assert!(report.errors.is_empty(), "the widest spec is refused: {:?}", report.errors);
    }
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
    spec["parameters"]["equal"] = serde_json::to_value(&params.equal).unwrap();
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
    let warnings: Vec<Diagnostic<C>> = (1..=MAX_COLUMNS as i64)
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
    let (widest, _) = widest(&result);
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
