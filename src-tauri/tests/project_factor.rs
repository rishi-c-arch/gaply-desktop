//! **The factor analysis in a project (Research OS, Phase 9B-3, decision 731).** Through
//! the commands' plain bodies, as their Tauri wrappers call them, over a root and a workspace
//! session the test makes, on `project_linear_regression.rs`'s pattern: a principal components
//! varimax run of six items under Kaiser's rule, a principal axis promax run of six items with
//! two factors and a row left out, and a principal axis run with an item that is the sum of
//! two others, a failed run; saved; the project closed and opened; each run read back, the two
//! results run again and the rerun `Identical`, and the report's lines: the items in order,
//! the extraction, the factors, the rotation, the iterations, the convergence and the codes'
//! count. Every file is synthetic and in a folder the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use gaply_core::contracts::RunState;
use gaply_core::research_project::RerunComparison;
use gaply_core::stats_factor::{Extraction, Rotation};
use serde_json::Value;

use project_support::*;

/// Six items on two factors over 40 rows (stats_factor_wire.rs's integer formula), a
/// seventh column `sum` equal to the first two, and a 41st row with no value in the
/// second item (and so none in the sum), which every run on those items leaves out.
fn data() -> Vec<u8> {
    let mut out = String::from("a,b,c,d,e,f,sum\n");
    for i in 1..=40u64 {
        let row: Vec<i64> = (1..=6u64)
            .map(|j| {
                let f = (j - 1) % 2;
                let score = ((i * 7907 + f * 15485863) % 1013) as i64 - 506;
                let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
                2 * score + noise
            })
            .collect();
        let line: Vec<String> = row.iter().map(i64::to_string).chain([(row[0] + row[1]).to_string()]).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out.push_str("1,,3,4,5,6,\n");
    out.into_bytes()
}

fn new_project(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

fn into_open(id: &str) -> SaveTarget {
    SaveTarget::OpenProject {
        project_id: id.to_string(),
    }
}

fn settled(w: &World) -> RerunView {
    let start = Instant::now();
    loop {
        if let Some(r) = pc::current(&w.ws).unwrap().rerun {
            if r.status != RerunProgress::Going {
                return r;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the rerun never settled"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn comparison(w: &World, id: &str, n: u64) -> Option<RerunComparison> {
    pc::runs(&w.root(), &w.ws, id)
        .unwrap()
        .into_iter()
        .find(|r| r.stored_run_id == n)
        .expect("the stored run")
        .comparison
}

/// A stored result's primary outputs.
fn outputs(w: &World, id: &str, n: u64) -> Value {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    assert_eq!(back.state, RunState::Succeeded, "run {n}");
    let bundle: Value = serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    bundle["primaryOutputs"].clone()
}

#[test]
fn factor_analyses_are_saved_read_back_run_again_identical_and_reported() {
    let w = World::new("9b3-project");
    let slot = w.open("factor.csv", &data());
    let six = [1, 2, 3, 4, 5, 6];
    let varimax = w.factor(slot, &six, Extraction::PrincipalComponents, None, Rotation::Varimax, vec![]);
    let promax = w.factor(slot, &six, Extraction::PrincipalAxis, Some(2), Rotation::Promax, vec![]);
    let singular = w.factor(slot, &[1, 2, 7], Extraction::PrincipalAxis, Some(1), Rotation::None, vec![]);
    assert_eq!(w.wait_new_kind(varimax), RunState::Succeeded);
    assert_eq!(w.wait_new_kind(promax), RunState::Succeeded);
    // A run's own refusal is recorded as failed.
    assert_eq!(w.wait_new_kind(singular), RunState::Failed);

    let id = pc::save_run(&w.root(), &w.ws, varimax, new_project("Factor"))
        .expect("the varimax run saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, promax, into_open(&id)).expect("the promax run saves");
    pc::save_run(&w.root(), &w.ws, singular, into_open(&id)).expect("the failed run saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    // Read back: each stored result is the analysis it was.
    let first = outputs(&w, &id, 1);
    assert_eq!(first["items"], serde_json::json!(["C1", "C2", "C3", "C4", "C5", "C6"]));
    assert_eq!(first["extraction"]["method"], "principal_components");
    assert_eq!(first["extraction"]["factors"], 2, "Kaiser's rule on two factors");
    assert_eq!(first["rotation"]["method"], "varimax");
    assert_eq!(first["rowsUsed"], 40);
    assert_eq!(first["exclusions"][0]["code"], "stats.factor.missing_value");
    assert_eq!(first["exclusions"][0]["params"]["column"], 2);
    let second = outputs(&w, &id, 2);
    assert_eq!(second["extraction"]["method"], "principal_axis");
    assert_eq!(second["rotation"]["method"], "promax");
    assert_eq!(second["rotation"]["factorCorrelations"].as_array().unwrap().len(), 2);
    let back = pc::run_result(&w.root(), &w.ws, &id, 3).expect("read back");
    assert_eq!(back.state, RunState::Failed);
    assert!(back.bundle_json.is_none() && back.manifest_json.is_none());
    let failure: Value = serde_json::from_str(back.failure_json.as_deref().expect("its failure")).unwrap();
    assert_eq!(failure["failure"]["code"], "stats.factor.correlations_singular");
    assert_eq!(failure["failure"]["params"]["column"], 7, "the sum, named by its position");

    for (stored, new) in [(1, 4), (2, 5)] {
        let started = pc::rerun(&w.root(), &w.ws, stored).expect("the rerun starts");
        assert_eq!(started.kind, RunKind::Factor);
        let r = settled(&w);
        assert_eq!((r.status, r.stored_run_id), (RerunProgress::Saved, Some(new)));
        assert_eq!(
            comparison(&w, &id, new),
            Some(RerunComparison::Identical),
            "run {stored} again"
        );
    }

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    for n in [1, 2] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for (line, times) in [
        ("- Items: columns 1, 2, 3, 4, 5, 6, by position", 4),
        ("- Items: columns 1, 2, 7, by position", 1),
        ("- Extraction: principal components", 2),
        ("- Extraction: principal axis factoring", 3),
        ("- Factors: every eigenvalue above 1", 2),
        ("- Factors: 2", 2),
        ("- Factors: 1", 1),
        ("- Rotation: varimax", 2),
        ("- Rotation: promax", 2),
        ("- Rotation: none", 1),
        ("- Maximum iterations: 25", 5),
        ("- Convergence: 0.001", 5),
        ("- Missing-value codes: 0", 5),
    ] {
        assert_eq!(r.matches(line).count(), times, "{line}:\n{r}");
    }
    // No header reaches the report (a rough check, as project_linear_regression.rs's; the
    // privacy matrix checks headers with sentinels).
    for text in ["sum"] {
        assert!(
            !r.contains(&format!(" {text}\n")) && !r.contains(&format!(": {text}")),
            "{text} in the report"
        );
    }
}
