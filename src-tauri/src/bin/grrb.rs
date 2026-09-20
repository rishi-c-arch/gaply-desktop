//! **The Gaply Research Reliability Benchmark runner.** Phase 2b, §6c.1-6c.2.
//!
//! Loads `evals/grrb/*.jsonl`, runs each case against the Tier 0 engine its
//! family names, and writes a report to `evals/reports/`.
//!
//! # No target values
//!
//! §6c.2: *"Targets are set per agent from its first measured baseline, and the
//! record says what the baseline was. A design document that prints example
//! percentages is printing numbers that describe nothing."* This program prints
//! what it measured and nothing it did not.
//!
//! # A column that does not apply prints WHY, never a number
//!
//! Most of §6c.2's matrix is written for a model. Tier 0 emits no probability,
//! spends no tokens and calls no proxy, so calibration, cost and context
//! efficiency have no value here — and a zero in those columns would read as a
//! measurement. They print `n/a` with the reason instead.
//!
//! # Unrunnable is a third outcome, not a failure
//!
//! A case whose `engine` is `none` has no Tier 0 check to ask: §11 D191's pair
//! does not exist, §11 D166's lane is declined, and a retraction verdict needs a
//! registry this run cannot reach. Counting those as failures would report a
//! product defect where the truth is an absent instrument, so they are counted
//! and named separately and excluded from every rate.

use std::collections::BTreeMap;
use std::time::Instant;

use gaply_core::extract::docparse::PagedBlock;
use gaply_core::journal_extract::GuidelineBlock;
use serde::{Deserialize, Serialize};

const FAMILIES: [&str; 6] = [
    "mathematical",
    "statistical",
    "manuscript_consistency",
    "literature",
    "journal",
    "adversarial",
];

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    family: String,
    stratum: String,
    #[serde(default)]
    engine: Option<String>,
    provenance: String,
    note: String,
    input: Input,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Input {
    Text { text: String },
    Lines { lines: Vec<String> },
    Blocks { blocks: Vec<BlockIn> },
}

#[derive(Debug, Deserialize)]
struct BlockIn {
    #[serde(default)]
    heading: Option<String>,
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    page: Option<u32>,
    text: String,
}

#[derive(Debug, Deserialize)]
struct Expected {
    finding: bool,
    #[serde(default)]
    codes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    family: String,
    stratum: String,
    expected_finding: bool,
    /// `None` when no engine could be asked.
    got_finding: Option<bool>,
    outcome: &'static str,
    deterministic: Option<bool>,
    micros: u128,
}

#[derive(Debug, Serialize)]
struct FamilyReport {
    family: String,
    tp: u32,
    fp: u32,
    fn_: u32,
    tn: u32,
    unrunnable: u32,
    accuracy_pct: Option<f64>,
    false_positive_rate_pct: Option<f64>,
    false_negative_rate_pct: Option<f64>,
    /// Integer percent: `eval_strata` returns whole points, and rounding a
    /// weighted estimate to one decimal would imply a precision the sample sizes
    /// here do not support.
    weighted_accuracy_pct: Option<u32>,
    weighting: String,
}

#[derive(Debug, Serialize)]
struct Report {
    head_sha: String,
    generated_at_epoch: u64,
    engine: &'static str,
    total_cases: usize,
    runnable: usize,
    unrunnable: usize,
    families: Vec<FamilyReport>,
    not_applicable: BTreeMap<String, String>,
    determinism_all_stable: bool,
    total_micros: u128,
    cases: Vec<CaseResult>,
}

/// Ask the family's Tier 0 engine. `None` = nothing to ask.
fn run_case(c: &Case) -> Option<bool> {
    let engine = c.engine.as_deref().unwrap_or(match c.family.as_str() {
        "mathematical" => "equation",
        "statistical" => "validate",
        "manuscript_consistency" => "consistency",
        "literature" => "citations",
        "journal" => "journal",
        "adversarial" => "sanitize",
        _ => "none",
    });
    match (engine, &c.input) {
        ("none", _) => None,
        ("equation", Input::Lines { lines }) => {
            let graph = gaply_core::equation::graph::graph_from_lines(lines);
            let values = graph.bound_values();
            // **`is_reportable()` is the filter production uses.**
            // `check_equation` returns an outcome for EVERY equation, including
            // `Confirmed` ones — an agreement is a finding object, not a defect.
            // Counting them all made the engine fire on 8 of 8 mathematical
            // cases, correct sums included, which is the uniform-result tell
            // CLAUDE.md names as the strongest single signal in the file.
            let hit = graph.nodes.iter().any(|n| {
                gaply_core::equation::check::check_equation(&n.equation, &values)
                    .iter()
                    .any(|f| f.is_reportable())
            });
            Some(hit)
        }
        ("validate", Input::Text { text }) => {
            let ex = gaply_core::extract::extract_from_text(text);
            Some(!gaply_core::validate::validate(&ex).flags.is_empty())
        }
        ("consistency", Input::Blocks { blocks }) => {
            let pb: Vec<PagedBlock> = blocks
                .iter()
                .map(|b| PagedBlock {
                    style: b.style.clone(),
                    page: b.page,
                    text: b.text.clone(),
                })
                .collect();
            let pre = gaply_core::ai_engine::audit_prepass::prepass_blocks(&pb);
            Some(!gaply_core::consistency::check_consistency(&pb, &pre).findings.is_empty())
        }
        ("citations", Input::Text { text }) => {
            let ex = gaply_core::extract::extract_from_text(text);
            let uses = gaply_core::extract::citations::classify_reference_use(
                &ex.references,
                &ex.citations,
            );
            Some(uses.iter().any(|u| {
                matches!(u, gaply_core::extract::citations::CitationUse::Uncited)
            }))
        }
        ("journal", Input::Blocks { blocks }) => {
            let gb: Vec<GuidelineBlock> = blocks
                .iter()
                .map(|b| GuidelineBlock {
                    heading: b.heading.clone().unwrap_or_default(),
                    text: b.text.clone(),
                })
                .collect();
            Some(!gaply_core::journal_extract::extract_requirements(&gb).is_empty())
        }
        ("sanitize", Input::Text { text }) => {
            Some(!gaply_core::sanitize::scan_injections(text).is_empty())
        }
        // A case whose input shape does not match its engine is a BUG IN THE
        // CASE, and must not be silently scored as unrunnable.
        (e, _) => panic!("case {}: engine {e:?} cannot consume this input shape", c.id),
    }
}

fn pct(num: u32, den: u32) -> Option<f64> {
    (den > 0).then(|| num as f64 * 100.0 / den as f64)
}

fn main() {
    let root = std::path::Path::new("evals/grrb");
    let pops: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("populations.json")).expect("populations.json"))
            .expect("populations.json is not JSON");

    let mut all: Vec<Case> = Vec::new();
    for f in FAMILIES {
        let p = root.join(format!("{f}.jsonl"));
        let src = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        for (i, line) in src.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let c: Case = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{}:{}: {e}", p.display(), i + 1));
            assert_eq!(c.family, f, "{} is filed under {f}", c.id);
            assert!(!c.provenance.trim().is_empty(), "{}: every case needs a provenance note", c.id);
            all.push(c);
        }
    }
    assert!(all.len() >= 50, "§6c.1 asks for fifty; found {}", all.len());

    let t0 = Instant::now();
    let mut results = Vec::new();
    let mut stable = true;
    for c in &all {
        let t = Instant::now();
        let got = run_case(c);
        let micros = t.elapsed().as_micros();
        // Determinism is a COLUMN (§6c.2): same input, same output.
        let det = got.map(|g| run_case(c) == Some(g));
        if det == Some(false) {
            stable = false;
        }
        let outcome = match got {
            None => "unrunnable",
            Some(g) if g == c.expected.finding && g => "tp",
            Some(g) if g == c.expected.finding => "tn",
            Some(g) if g => "fp",
            Some(_) => "fn",
        };
        results.push(CaseResult {
            id: c.id.clone(),
            family: c.family.clone(),
            stratum: c.stratum.clone(),
            expected_finding: c.expected.finding,
            got_finding: got,
            outcome,
            deterministic: det,
            micros,
        });
    }

    let mut families = Vec::new();
    for f in FAMILIES {
        let rows: Vec<&CaseResult> = results.iter().filter(|r| r.family == f).collect();
        let cnt = |o: &str| rows.iter().filter(|r| r.outcome == o).count() as u32;
        let (tp, fp, fn_, tn, un) = (cnt("tp"), cnt("fp"), cnt("fn"), cnt("tn"), cnt("unrunnable"));
        let runnable = tp + fp + fn_ + tn;

        // Weighted accuracy, per §6c.1 — and it is withheld unless EVERY stratum
        // with cases carries a measured population. §11 D123 is what a pooled
        // rate printed in a weighted slot costs.
        let mut strata: Vec<app_lib::ai::eval_strata::Stratum> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        let mut names: Vec<String> = rows.iter().map(|r| r.stratum.clone()).collect();
        names.sort();
        names.dedup();
        for s in &names {
            let p = pops.get(f).and_then(|x| x.get(s)).and_then(|x| x.get("population"));
            match p.and_then(|v| v.as_u64()) {
                Some(n) => {
                    let sr: Vec<&&CaseResult> = rows.iter().filter(|r| &r.stratum == s).collect();
                    let c2 = |o: &str| sr.iter().filter(|r| r.outcome == o).count() as u32;
                    strata.push(app_lib::ai::eval_strata::Stratum {
                        name: s.clone(),
                        population: n as usize,
                        tp: c2("tp"),
                        fp: c2("fp"),
                        fn_: c2("fn"),
                        tn: c2("tn"),
                    });
                }
                None => missing.push(s.clone()),
            }
        }
        let (weighted, why) = if missing.is_empty() && !strata.is_empty() {
            let acc = app_lib::ai::eval_strata::stratified_precision_pct(&strata);
            (acc, "weighted over every stratum".to_string())
        } else {
            (
                None,
                format!(
                    "WITHHELD — these strata have no measured population: {}. A pooled rate \
                     printed here would be the §11 D123 over-claim.",
                    if missing.is_empty() { "(no runnable strata)".into() } else { missing.join(", ") }
                ),
            )
        };

        families.push(FamilyReport {
            family: f.to_string(),
            tp,
            fp,
            fn_,
            tn,
            unrunnable: un,
            accuracy_pct: pct(tp + tn, runnable),
            false_positive_rate_pct: pct(fp, fp + tn),
            false_negative_rate_pct: pct(fn_, fn_ + tp),
            weighted_accuracy_pct: weighted,
            weighting: why,
        });
    }

    let mut na = BTreeMap::new();
    for (k, v) in [
        ("calibration_brier_ece", "n/a — Tier 0 emits no probability. §4.4 gives it authority because it is deterministic, not because it is confident."),
        ("cost", "n/a — no model is called and no proxy is reached; the number would be 0.00 and would read as a measurement."),
        ("context_efficiency_tokens_per_finding", "n/a — no context is constructed and no tokens are spent."),
        ("citation_grounding", "n/a — Tier 0 findings cite manuscript spans, not retrieved sources; there is nothing to ground against."),
        ("evidence_policy_satisfaction", "not yet implemented — the agent graph's evidence_policy applies to specialists, and no specialist runs in this baseline."),
        ("contradiction_rate_against_tier0", "n/a — this run IS Tier 0. The column becomes meaningful the first time a model-backed agent is scored."),
        ("reviewer_agreement", "the labels ARE the human comparison: every case was adjudicated by hand from a decision record. Accuracy against them is this column."),
    ] {
        na.insert(k.to_string(), v.to_string());
    }

    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let report = Report {
        head_sha: head,
        generated_at_epoch: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        engine: "tier0",
        total_cases: all.len(),
        runnable: results.iter().filter(|r| r.outcome != "unrunnable").count(),
        unrunnable: results.iter().filter(|r| r.outcome == "unrunnable").count(),
        families,
        not_applicable: na,
        determinism_all_stable: stable,
        total_micros: t0.elapsed().as_micros(),
        cases: results,
    };

    let out = std::env::args().nth(1).unwrap_or_else(|| {
        format!("evals/reports/grrb-{}.json", report.generated_at_epoch)
    });
    if let Some(d) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(d).ok();
    }
    std::fs::write(&out, serde_json::to_string_pretty(&report).expect("serialize"))
        .unwrap_or_else(|e| panic!("{out}: {e}"));

    println!("GRRB baseline — engine=tier0  head={}", report.head_sha);
    println!(
        "  {} cases  |  {} runnable  |  {} unrunnable  |  determinism stable: {}",
        report.total_cases, report.runnable, report.unrunnable, report.determinism_all_stable
    );
    println!("\n  {:<24} {:>3} {:>3} {:>3} {:>3} {:>5}  {:>9}  weighted", "family", "tp", "fp", "fn", "tn", "unrun", "accuracy");
    for f in &report.families {
        let acc = f.accuracy_pct.map(|a| format!("{a:.1}%")).unwrap_or_else(|| "-".into());
        let w = f.weighted_accuracy_pct.map(|a| format!("{a}%")).unwrap_or_else(|| "withheld".into());
        println!(
            "  {:<24} {:>3} {:>3} {:>3} {:>3} {:>5}  {:>9}  {w}",
            f.family, f.tp, f.fp, f.fn_, f.tn, f.unrunnable, acc
        );
    }
    println!("\n  report: {out}");
    println!("\n  columns that do not apply (§6c.2), with the reason:");
    for (k, v) in &report.not_applicable {
        println!("    {k}: {v}");
    }
}
