//! **The structural equation model's stored checker: its records (Research OS, Phase 9C-5e,
//! decision 840).** Each record is a real SEM run on synthetic data, started through
//! `stats_sem_commands::run` (World::sem), saved through `project_support`'s World into a
//! project in a folder the test creates and removes, and read back as a stored result through
//! `pc::run_result`, as `project_sem.rs` does. `storedSem.vitest.ts` reads the committed bytes
//! and holds the checker to them: loopPivot.json for C1 (a loop whose natural units differ from
//! Rust's), small.json and smallLoop.json for C2 (data scaled by 1e-10, without and with a loop).
//!
//! With `GAPLY_WRITE_SEM_CHECKER_RECORDS` set, each record is written to
//! `CARGO_MANIFEST_DIR/../src/screens/statistics/projects/fixtures/semChecker/`. With it unset,
//! each is compared with the committed file as `project_commands.rs`'s assert_stored_fixture
//! does: the layout after `steady`'s normalisation, not the bytes, since a value's last digit may
//! differ by platform. `layout`, `stored_layout`, `steady` and `fixtures` are copied from
//! `project_commands.rs` (private to that test file); `fixtures` names this folder instead.
//!
//! Data come from a seeded xorshift64* with normals as the sum of twelve uniforms less 6, so no
//! platform function is used; every cell is written in exponent notation with every digit
//! (`{:e}`), never rounded to three decimals. Nothing here prints a cell.

mod project_support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use app_lib::project_commands::{self as pc, RunResultView, SaveTarget};
use gaply_core::contracts::RunState;
use gaply_core::research_project::Caps;
use gaply_core::stats_sem::SemParams;
use serde_json::{json, Value};

use project_support::*;

/// A seeded stream of standard normals: xorshift64*, each normal the sum of twelve uniforms
/// less 6 (project_sem.rs's Draws).
struct Draws(u64);

impl Draws {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        (0..12).map(|_| self.uniform()).sum::<f64>() - 6.0
    }

    /// Uniform on [lo, hi).
    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }

    /// −1 or 1.
    fn sign(&mut self) -> f64 {
        if self.uniform() < 0.5 {
            -1.0
        } else {
            1.0
        }
    }

    /// An integer from lo to hi (the hunt's `int`).
    fn int(&mut self, lo: usize, hi: usize) -> usize {
        lo + ((self.uniform() * (hi - lo + 1) as f64) as usize).min(hi - lo)
    }

    fn chance(&mut self, p: f64) -> bool {
        self.uniform() < p
    }

    /// 10^e times a mantissa in [1, 2), e from lo to hi, by multiplication only (the hunt's
    /// `scale`).
    fn scale(&mut self, lo: i32, hi: i32) -> f64 {
        let e = lo + self.int(0, (hi - lo) as usize) as i32;
        let mut t = 1.0;
        for _ in 0..e.unsigned_abs() {
            t *= 10.0;
        }
        self.between(1.0, 2.0) * if e < 0 { 1.0 / t } else { t }
    }
}

/// A CSV with headers v1..vp, each cell in exponent notation with every digit.
fn csv(rows: &[Vec<f64>]) -> Vec<u8> {
    let p = rows[0].len();
    let mut s = (1..=p).map(|j| format!("v{j}")).collect::<Vec<_>>().join(",");
    s.push('\n');
    for r in rows {
        s.push_str(&r.iter().map(|x| format!("{x:e}")).collect::<Vec<_>>().join(","));
        s.push('\n');
    }
    s.into_bytes()
}

fn col(c: u32) -> Value {
    json!({ "column": format!("C{c}") })
}

/// Decision 838's shape (Example 7's, as the review ran it): the paths C2 → C1, C3 → C1,
/// C1 → C2 and C4 → C2 (a loop of two columns, each with an instrument), the covariances C3
/// with C5 and C5 with C6.
fn loop_spec() -> Value {
    let paths: Vec<Value> = [(1, 2), (1, 3), (2, 1), (2, 4)].iter().map(|(t, f)| json!({ "to": col(*t), "from": col(*f) })).collect();
    let covs: Vec<Value> = [(3, 5), (5, 6)].iter().map(|(a, b)| json!({ "a": col(*a), "b": col(*b) })).collect();
    json!({ "indicators": [], "paths": paths, "covariances": covs, "equal": [], "missingCodes": [] })
}

/// Data of `loop_spec`'s shape, `n` rows at `seed`: gains of either sign from 0.3 to 1.3 on the
/// loop (its determinant 1 − b12 b21 kept at least 0.1 from 0), instruments from 0.5 to 2, error
/// SDs from 0.05 to 1, so a loop column's error variance and its total variance often sit in
/// different powers of two; each column then scaled by 10^e times a mantissa in [1, 2), e from −3
/// to 3, or by `scale` times a mantissa when `scale` is given.
fn loop_data(seed: u64, n: usize, scale: Option<f64>) -> Vec<u8> {
    let mut r = Draws(20_261_116 + seed);
    let (b12, b21) = loop {
        let (x, y) = (r.sign() * r.between(0.3, 1.3), r.sign() * r.between(0.3, 1.3));
        if (1.0 - x * y).abs() >= 0.1 {
            break (x, y);
        }
    };
    let (b13, b24) = (r.sign() * r.between(0.5, 2.0), r.sign() * r.between(0.5, 2.0));
    let (s1, s2) = (r.between(0.05, 1.0), r.between(0.05, 1.0));
    let scales: Vec<f64> = (0..6)
        .map(|_| {
            let mantissa = r.between(1.0, 2.0);
            match scale {
                Some(s) => s * mantissa,
                None => {
                    let e = (r.uniform() * 7.0) as i32 - 3;
                    let mut ten = 1.0;
                    for _ in 0..e.unsigned_abs() {
                        ten *= 10.0;
                    }
                    mantissa * if e < 0 { 1.0 / ten } else { ten }
                }
            }
        })
        .collect();
    let det = 1.0 - b12 * b21;
    let rows: Vec<Vec<f64>> = (0..n)
        .map(|_| {
            let z5 = r.normal();
            let x3 = 0.5 * z5 + r.normal();
            let x4 = r.normal();
            let x6 = 0.5 * z5 + r.normal();
            let (e1, e2) = (s1 * r.normal(), s2 * r.normal());
            // y1 = b12 y2 + b13 x3 + e1; y2 = b21 y1 + b24 x4 + e2, solved.
            let (a, b) = (b13 * x3 + e1, b24 * x4 + e2);
            let y1 = (a + b12 * b) / det;
            let y2 = (b + b21 * a) / det;
            [y1, y2, x3, x4, z5, x6].iter().zip(&scales).map(|(x, s)| x * s).collect()
        })
        .collect();
    csv(&rows)
}

/// The first seed at which 2480982e's checker refuses loopPivot.json's record (decision 840's
/// search, 16 Nov 2026, macOS aarch64, ~/gaply-corpus/os/phase9/9c5e/search/verdicts_1_200.tsv):
/// of seeds 1 to 200, all 200 succeeded with the note nonrecursive alone, and 2480982e's checker
/// refused 26, 82, 137 and 154.
const LOOP_PIVOT_SEED: u64 = 26;

/// The review's sem0028 spec (decision 840): one factor on C1 and C2, the paths C3, C4 and C5 →
/// factor 1, the covariances C3 with C5 and C4 with C5.
fn small_spec() -> Value {
    let paths: Vec<Value> = [3, 4, 5].iter().map(|c| json!({ "to": { "factor": 1 }, "from": col(*c) })).collect();
    let covs: Vec<Value> = [(3, 5), (4, 5)].iter().map(|(a, b)| json!({ "a": col(*a), "b": col(*b) })).collect();
    json!({ "indicators": [["C1", "C2"]], "paths": paths, "covariances": covs, "equal": [], "missingCodes": [] })
}

/// Data of `small_spec`'s shape, 500 rows, every column scaled by 1e-10 times a mantissa in
/// [1, 2): C3 and C4 independent, C5 from both, the factor from all three, C1 and C2 its
/// indicators.
fn small_data(seed: u64) -> Vec<u8> {
    let mut r = Draws(20_261_116 + seed);
    let scales: Vec<f64> = (0..5).map(|_| 1e-10 * r.between(1.0, 2.0)).collect();
    let rows: Vec<Vec<f64>> = (0..500)
        .map(|_| {
            let (x3, x4) = (r.normal(), r.normal());
            let x5 = 0.6 * x3 + 0.6 * x4 + r.normal();
            let f = 0.5 * x3 + 0.4 * x4 - 0.3 * x5 + 0.8 * r.normal();
            let (c1, c2) = (f + 0.6 * r.normal(), 0.8 * f + 0.6 * r.normal());
            [c1, c2, x3, x4, x5].iter().zip(&scales).map(|(x, s)| x * s).collect()
        })
        .collect();
    csv(&rows)
}

/* ---- the hunt's cancel family (decision 843) ------------------------------- */

/// Where the hunt's case cancel1506 begins: the state of its generator (seed 20261117, the cancel
/// family alone, 3,000 cases, δ from 1e-9 to 1e-6), printed by the hunt's probe at that case
/// (~/gaply-corpus/os/phase9/9c5e/hunt/trace_cancel1506_1794847430.log). The draws run on from
/// case to case, so this state, not the seed, is where cancel1506's own draws begin.
const CANCEL1506_STATE: u64 = 546_337_911_576_572_692;

/// (I − C)⁻¹ by Gauss–Jordan with partial pivoting, or None where a pivot is below 1e-6 (the
/// hunt's generator, which solves each loop's data exactly).
fn inverse_i_minus(c: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = c.len();
    let mut w: Vec<Vec<f64>> = (0..n).map(|i| (0..2 * n).map(|j| if j < n { (if i == j { 1.0 } else { 0.0 }) - c[i][j] } else if j - n == i { 1.0 } else { 0.0 }).collect()).collect();
    for k in 0..n {
        let best = (k..n).max_by(|a, b| w[*a][k].abs().partial_cmp(&w[*b][k].abs()).unwrap()).unwrap();
        if !(w[best][k].abs() > 1e-6) {
            return None;
        }
        w.swap(k, best);
        let p = w[k][k];
        for j in 0..2 * n {
            w[k][j] /= p;
        }
        for r in 0..n {
            if r != k {
                let f = w[r][k];
                if f != 0.0 {
                    for j in 0..2 * n {
                        w[r][j] -= f * w[k][j];
                    }
                }
            }
        }
    }
    Some(w.into_iter().map(|row| row[n..].to_vec()).collect())
}

/// One case of the hunt's cancel family from `state`, its δ from `lo` to `hi`: a loop of two
/// columns C1 ⇄ C2, each with an instrument (C3 → C1, C4 → C2), a third instrument C5 → C1, and a
/// path C3 → C2 of −b21 a (1 + δ), so that C3's total on C2, (c + b21 a) / (1 − b12 b21), nearly
/// cancels; covariances among C3 to C5 at chance 0.3; each column's scale 1 or 10^e times a
/// mantissa (e from −3 to 3). Draws are taken in the hunt's order (its ring_case, Gen::spec and
/// Gen::data for these arguments). The data and the spec.
fn cancel_case(state: u64, lo: f64, hi: f64) -> (Vec<u8>, Value) {
    let mut r = Draws(state);
    let rows = [200, 500, 1000, 2000][r.int(0, 3)];
    let mixed = if r.chance(0.5) { 1.0 } else { 0.0 };
    let tiny = r.chance(0.2);
    // Structural variables 0 and 1 are the loop (C1, C2), 2 to 4 the instruments (C3 to C5).
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..2 {
        let ring = r.sign() * r.between(0.3, 1.3);
        edges.push(((i + 1) % 2, i, ring));
        let instrument = r.sign() * r.between(0.5, 2.0);
        edges.push((i, 2 + i, instrument));
    }
    let a = edges.iter().find(|(t, f, _)| *t == 0 && *f == 2).unwrap().2;
    let b21 = edges.iter().find(|(t, f, _)| *t == 1 && *f == 0).unwrap().2;
    let delta = r.sign() * r.between(lo, hi);
    edges.push((1, 2, -b21 * a * (1.0 + delta)));
    let third = r.sign() * r.between(0.5, 2.0);
    edges.push((0, 4, third));
    let sd: Vec<f64> = (0..5).map(|v| if v < 2 && tiny { r.between(1e-3, 1e-2) } else if v < 2 { r.between(0.2, 1.0) } else { 1.0 }).collect();
    let mut covs: Vec<(u32, u32)> = Vec::new();
    for i in 0..3u32 {
        for j in i + 1..3 {
            if r.chance(0.3) {
                covs.push((3 + i, 3 + j));
            }
        }
    }
    let scales: Vec<f64> = (0..5).map(|_| if r.chance(mixed) { r.scale(-3, 3) } else { 1.0 }).collect();
    let mut c = vec![vec![0.0; 5]; 5];
    for (t, f, b) in &edges {
        c[*t][*f] += b;
    }
    let m = inverse_i_minus(&c).expect("the hunt solved this case's loop");
    let data: Vec<Vec<f64>> = (0..rows)
        .map(|_| {
            let e: Vec<f64> = (0..5).map(|v| sd[v] * r.normal()).collect();
            let x: Vec<f64> = (0..5).map(|i| (0..5).map(|j| m[i][j] * e[j]).sum()).collect();
            x.iter().zip(&scales).map(|(x, s)| x * s).collect()
        })
        .collect();
    let mut paths: Vec<(u32, u32)> = edges.iter().map(|(t, f, _)| (*t as u32 + 1, *f as u32 + 1)).collect();
    paths.sort();
    paths.dedup();
    let spec = json!({
        "indicators": [],
        "paths": paths.iter().map(|(t, f)| json!({ "to": col(*t), "from": col(*f) })).collect::<Vec<_>>(),
        "covariances": covs.iter().map(|(a, b)| json!({ "a": col(*a), "b": col(*b) })).collect::<Vec<_>>(),
        "equal": [],
        "missingCodes": [],
    });
    (csv(&data), spec)
}

/// Caps wide enough for a 2,000-row file of full-precision cells.
const CAPS: Caps = Caps { project: 64 * 1024 * 1024, all_projects: 256 * 1024 * 1024, ..SMALL };

/// One run of `spec` on `data`, saved into a new project and read back as a stored result.
fn stored(tag: &str, data: &[u8], spec: Value) -> RunResultView {
    let w = World::with(tag, CAPS, None);
    let slot = w.open(&format!("{tag}.csv"), data);
    let params: SemParams = serde_json::from_value(spec).expect("a SEM spec");
    let run = w.sem(slot, params);
    let state = w.wait_new_kind(run);
    assert!(state == RunState::Succeeded || state == RunState::Failed, "{tag}: the run ended {state:?}");
    let id = pc::save_run(&w.root(), &w.ws, run, SaveTarget::NewProject { name: tag.to_string() }).expect("saves").project_id;
    pc::run_result(&w.root(), &w.ws, &id, 1).expect("read back")
}

/// A stored result's primary outputs.
fn outputs(view: &RunResultView) -> Value {
    assert_eq!(view.state, RunState::Succeeded);
    let bundle: Value = serde_json::from_str(view.bundle_json.as_deref().expect("a result")).unwrap();
    bundle["primaryOutputs"].clone()
}

fn value(stat: &Value) -> f64 {
    stat["value"].as_f64().expect("a value, not a code")
}

/* ---- copied from project_commands.rs (:871, :1139-1174), private there ------ */

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics/projects/fixtures/semChecker")
}

/// Every (path, JSON type) in `v`, arrays read through. The layout of a value
/// without the values themselves.
fn layout(v: &Value, at: &str, out: &mut BTreeSet<String>) {
    let kind = match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    out.insert(format!("{at}: {kind}"));
    match v {
        Value::Array(items) => items.iter().for_each(|x| layout(x, &format!("{at}[]"), out)),
        Value::Object(m) => m.iter().for_each(|(k, x)| layout(x, &format!("{at}.{k}"), out)),
        _ => {}
    }
}

/// A run read back, as JSON, with each stored text parsed in place.
fn stored_layout(mut v: Value) -> BTreeSet<String> {
    for k in ["specJson", "bundleJson", "manifestJson", "failureJson"] {
        if let Some(text) = v[k].as_str() {
            v[k] = serde_json::from_str(text).expect("stored JSON");
        }
    }
    let mut out = BTreeSet::new();
    layout(&v, "", &mut out);
    out
}

/// The elapsed time and the platform fixed, so a regenerated fixture does not
/// change with the machine that wrote it.
fn steady(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("\"elapsedMs\":") {
        let (head, tail) = rest.split_at(i + "\"elapsedMs\":".len());
        out.push_str(head);
        let digits = tail.chars().take_while(|c| c.is_ascii_digit()).count();
        out.push('0');
        rest = &tail[digits..];
    }
    out.push_str(rest);
    for (os, arch) in [("macos", "aarch64"), ("macos", "x86_64"), ("linux", "x86_64"), ("linux", "aarch64"), ("windows", "x86_64"), ("windows", "aarch64")] {
        out = out.replace(&format!("\"platform\":{{\"os\":\"{os}\",\"arch\":\"{arch}\"}}"), "\"platform\":{\"os\":\"macos\",\"arch\":\"aarch64\"}");
    }
    out
}

/// Writes the record with `GAPLY_WRITE_SEM_CHECKER_RECORDS` set; compares its layout with the
/// committed file either way (assert_stored_fixture, project_commands.rs:1207-1228).
fn assert_record(name: &str, view: &RunResultView) {
    let mut got = serde_json::to_value(view).unwrap();
    for k in ["specJson", "bundleJson", "manifestJson", "failureJson"] {
        if let Some(text) = got[k].as_str() {
            got[k] = Value::String(steady(text));
        }
    }
    let p = fixtures().join(name);
    if std::env::var_os("GAPLY_WRITE_SEM_CHECKER_RECORDS").is_some() {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
    }
    let want: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{name}: {e}"))).unwrap();
    assert_eq!(stored_layout(got), stored_layout(want), "{name}: the stored result and its record differ in layout");
}

/* ---- the records ----------------------------------------------------------- */

/// C1's record: decision 838's shape at 2,000 rows, a loop, at the seed decision 840's search
/// found 2480982e's checker refuses.
#[test]
fn the_loop_pivot_record_is_a_stored_loop_of_decision_838s_shape() {
    let view = stored("loopPivot", &loop_data(LOOP_PIVOT_SEED, 2000, None), loop_spec());
    let o = outputs(&view);
    // The premises: the loop, 2,000 rows, the six columns and the four paths.
    assert_eq!(o["notes"], json!(["stats.sem.nonrecursive"]), "the loop and nothing else");
    assert_eq!(o["rowsUsed"], json!(2000));
    assert_eq!(o["columns"], json!(["C1", "C2", "C3", "C4", "C5", "C6"]));
    assert_eq!(o["weights"].as_array().unwrap().len(), 4);
    assert_record("loopPivot.json", &view);
}

/// C2's record without a loop: the review's sem0028 spec on data scaled by 1e-10, no notes,
/// every variance estimate far inside 2480982e's absolute zone of 1e-17.
#[test]
fn the_small_record_has_no_loop_and_every_variance_below_2480982es_zone() {
    let view = stored("small", &small_data(1), small_spec());
    let o = outputs(&view);
    assert_eq!(o["notes"], json!([]), "no note");
    let variances: Vec<f64> = o["variances"].as_array().unwrap().iter().map(|v| value(&v["estimate"])).collect();
    assert_eq!(variances.len(), 6, "the factor and five columns");
    assert!(variances.iter().all(|v| *v > 0.0 && *v < 1e-17), "the premise: every variance in (0, 1e-17): {} of 6", variances.iter().filter(|v| **v > 0.0 && **v < 1e-17).count());
    assert!(o["smc"].as_array().unwrap().iter().all(|s| s["value"].is_object() && s["value"].get("value").is_some()), "every SMC a value");
    assert_record("small.json", &view);
}

/// Decision 843's record: the hunt's cancel1506, a loop whose two routes from C3 to C2 nearly
/// cancel, which the total's tolerance of decision 840 refused.
#[test]
fn the_cancel_record_is_the_hunts_cancel1506() {
    let (data, spec) = cancel_case(CANCEL1506_STATE, 1e-9, 1e-6);
    let view = stored("cancel", &data, spec);
    let o = outputs(&view);
    // The premises, as the hunt recorded cancel1506 (run3_cancel_3000/runs.tsv): the loop and
    // nothing else, attempt 1, 10 iterations, df 4, 1,000 rows, five columns.
    assert_eq!(o["notes"], json!(["stats.sem.nonrecursive"]));
    assert_eq!((&o["attempt"], &o["iterations"], &o["fit"]["model"]["df"], &o["rowsUsed"]), (&json!(1), &json!(10), &json!(4), &json!(1000)));
    assert_eq!(o["columns"], json!(["C1", "C2", "C3", "C4", "C5"]));
    // And the cancelling: C3's total on C2 is small beside its direct path.
    let e = o["effects"].as_array().unwrap().iter().find(|e| e["to"] == col(2) && e["from"] == col(3)).expect("C3 on C2");
    assert!(value(&e["total"]).abs() < 0.1 * value(&e["direct"]).abs(), "the premise: C3's routes to C2 nearly cancel");
    assert_record("cancel.json", &view);
}

/// C2's record with a loop: decision 838's shape on data scaled by 1e-10.
#[test]
fn the_small_loop_record_is_a_loop_with_every_variance_below_2480982es_zone() {
    let view = stored("smallLoop", &loop_data(1, 1000, Some(1e-10)), loop_spec());
    let o = outputs(&view);
    assert_eq!(o["notes"], json!(["stats.sem.nonrecursive"]), "the loop and nothing else");
    let variances: Vec<f64> = o["variances"].as_array().unwrap().iter().map(|v| value(&v["estimate"])).collect();
    assert!(variances.iter().all(|v| *v > 0.0 && *v < 1e-17), "the premise: every variance in (0, 1e-17): {} of {}", variances.iter().filter(|v| **v > 0.0 && **v < 1e-17).count(), variances.len());
    assert_record("smallLoop.json", &view);
}
