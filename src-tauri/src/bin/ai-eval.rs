//! Dev eval harness — runs labelled cases through the REAL model.
//!
//! ```text
//! cargo run --bin ai-eval -- --task citation_need --cases evals/citation_need.jsonl
//! ```
//!
//! Writes a versioned report to `evals/reports/`. Every report records the
//! model id, prompt version, n_ctx and date, because a number without those is
//! not comparable to any other number — the whole point of keeping reports is
//! to see a change move a metric, and that requires knowing what changed.
//!
//! # The case schema generalises to all eight tasks
//!
//! ```jsonc
//! {
//!   "id":   "cn-seed-01",
//!   "task": "citation_need",
//!   "note": "why this case exists",          // for humans; never scored
//!   "input":    { /* task-specific: the task's INPUT block */ },
//!   "expected": { /* task-specific: only the fields worth scoring */ }
//! }
//! ```
//!
//! `input` and `expected` are opaque JSON here and are interpreted by the
//! per-task scorer. Adding a task means adding a scorer, not changing the file
//! format or this binary's reporting.
//!
//! # What it does NOT do
//!
//! It does not gate anything and it does not assert. It measures, and it writes
//! down what it measured. A 0.5B model is expected to score poorly; that is
//! information, not a failure.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use app_lib::ai::generative::{BundledGenerativeLoader, TASK_N_CTX};
use app_lib::ai::model_manager::ModelManager;
use app_lib::ai::task::{run_task, TaskContext, TaskError};
use app_lib::ai::tasks::citation_need::{CitationNeedInput, CitationNeedTask, PROMPT_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct EvalCase {
    id: String,
    task: String,
    #[serde(default)]
    note: String,
    input: serde_json::Value,
    #[serde(default)]
    expected: serde_json::Value,
}

/// One case's result. `outcome` distinguishes the three things that can happen,
/// because collapsing them would hide the most important one: a model that
/// never produces valid output scores 0% accuracy and 100% validation failure,
/// and those are different problems with different fixes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseResult {
    id: String,
    outcome: Outcome,
    /// Verbatim model output, so a report can be read without re-running.
    raw: Option<String>,
    /// Per-field agreement with the label. Absent when the case did not produce
    /// a parseable output.
    #[serde(skip_serializing_if = "Option::is_none")]
    scored: Option<serde_json::Value>,
    retried: bool,
    elapsed_ms: u64,
    tokens: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Outcome {
    /// Parsed and validated.
    Ok,
    /// Both attempts failed validation — the model could not produce a legal
    /// answer. NOT the same as answering wrongly.
    ValidationFailed,
    /// Generation itself failed (load, context, cancel).
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    task: String,
    model_id: String,
    prompt_version: String,
    n_ctx: usize,
    /// Caller-supplied so the report is reproducible without a clock in the
    /// engine; falls back to "unknown" rather than inventing a date.
    date: String,
    cases_file: String,
    total_cases: usize,
    /// Of the cases that produced a valid output, how many matched the label.
    needs_citation_accuracy: Option<f64>,
    sentence_type_agreement: Option<f64>,
    severity_agreement: Option<f64>,
    /// Of ALL cases. This is the headline number for a weak model.
    validation_failure_rate: f64,
    retry_rate: f64,
    mean_latency_ms: f64,
    tokens_per_second: f64,
    results: Vec<CaseResult>,
}

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn score_citation_need(
    expected: &serde_json::Value,
    got: &app_lib::ai::tasks::citation_need::CitationNeedOutput,
) -> serde_json::Value {
    let got_v = serde_json::to_value(got).unwrap_or(serde_json::Value::Null);
    let cmp = |field: &str| -> Option<bool> {
        let e = expected.get(field)?;
        let g = got_v.get(field)?;
        Some(e == g)
    };
    serde_json::json!({
        "needsCitation": cmp("needs_citation"),
        "sentenceType": cmp("sentence_type"),
        "severity": cmp("severity"),
        "got": got_v,
        "expected": expected,
    })
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let task_name = arg("--task").unwrap_or_else(|| "citation_need".to_string());
    let cases_path = arg("--cases")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("evals/citation_need.jsonl"));
    let date = arg("--date").unwrap_or_else(|| "unknown".to_string());
    let out_dir = arg("--out").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("evals/reports"));

    if task_name != "citation_need" {
        eprintln!("only citation_need is implemented; the other seven arrive with their phases");
        std::process::exit(2);
    }

    let text = std::fs::read_to_string(&cases_path)
        .map_err(|e| format!("reading {}: {e}", cases_path.display()))?;
    let cases: Vec<EvalCase> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    let loader = BundledGenerativeLoader::resolve()
        .ok_or("no generative model resolves — set GAPLY_TEST_GEN_MODEL")?;
    let model_id = {
        use app_lib::ai::model_manager::BackendLoader;
        loader.model_id()
    };
    let manager = ModelManager::new(Arc::new(loader));

    println!("task    : {task_name}");
    println!("model   : {model_id}");
    println!("prompt  : {PROMPT_VERSION}");
    println!("n_ctx   : {TASK_N_CTX}");
    println!("cases   : {} from {}", cases.len(), cases_path.display());
    println!();

    let mut results = Vec::new();
    for case in &cases {
        // A case file mixing tasks would score one task's output against
        // another's labels and report a confident, meaningless number.
        if case.task != task_name {
            return Err(format!(
                "case {} is task {:?} but this run is {:?}",
                case.id, case.task, task_name
            )
            .into());
        }
        let input: CitationNeedInput = serde_json::from_value(case.input.clone())
            .map_err(|e| format!("case {}: bad input: {e}", case.id))?;
        let task = CitationNeedTask::new(input);
        let ctx = TaskContext::default();
        let started = std::time::Instant::now();
        let outcome = run_task(&manager, &task, &ctx, Arc::new(AtomicBool::new(false)), None).await;
        let elapsed_ms = started.elapsed().as_millis() as u64;

        let r = match outcome {
            Ok(run) => {
                let scored = score_citation_need(&case.expected, &run.output);
                let ok = scored.get("needsCitation").and_then(|v| v.as_bool()).unwrap_or(false);
                println!(
                    "  {} {}  {:>5}ms  {}",
                    if ok { "PASS" } else { "MISS" },
                    case.id,
                    elapsed_ms,
                    if run.retried { "(retried)" } else { "" }
                );
                CaseResult {
                    id: case.id.clone(),
                    outcome: Outcome::Ok,
                    raw: Some(serde_json::to_string(&run.output)?),
                    scored: Some(scored),
                    retried: run.retried,
                    elapsed_ms,
                    tokens: run.tokens,
                }
            }
            Err(TaskError::ValidationFailed { errors, first_raw, retry_raw }) => {
                println!("  FAIL {}  {:>5}ms  validation: {}", case.id, elapsed_ms,
                    errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "));
                CaseResult {
                    id: case.id.clone(),
                    outcome: Outcome::ValidationFailed,
                    // BOTH raw outputs — the report must show what the model
                    // actually said, not a summary of why it was rejected.
                    raw: Some(format!("--- attempt 1 ---\n{first_raw}\n--- retry ---\n{retry_raw}")),
                    scored: None,
                    retried: true,
                    elapsed_ms,
                    tokens: 0,
                }
            }
            Err(e) => {
                println!("  ERR  {}  {e}", case.id);
                CaseResult {
                    id: case.id.clone(),
                    outcome: Outcome::Error,
                    raw: Some(e.to_string()),
                    scored: None,
                    retried: false,
                    elapsed_ms,
                    tokens: 0,
                }
            }
        };
        let _ = &case.note; // documentation for humans, never scored
        results.push(r);
    }

    let valid_count = results.iter().filter(|r| r.outcome == Outcome::Ok).count();
    let agree = |field: &str| -> Option<f64> {
        let xs: Vec<f64> = results
            .iter()
            .filter(|r| r.outcome == Outcome::Ok)
            .filter_map(|r| r.scored.as_ref()?.get(field)?.as_bool())
            .map(|b| if b { 1.0 } else { 0.0 })
            .collect();
        mean(&xs)
    };
    let total = results.len().max(1) as f64;
    let total_tokens: usize = results.iter().map(|r| r.tokens).sum();
    let total_ms: u64 = results.iter().map(|r| r.elapsed_ms).sum();
    let needs_citation_accuracy = agree("needsCitation");
    let sentence_type_agreement = agree("sentenceType");
    let severity_agreement = agree("severity");
    let validation_failure_rate =
        results.iter().filter(|r| r.outcome == Outcome::ValidationFailed).count() as f64 / total;
    let retry_rate = results.iter().filter(|r| r.retried).count() as f64 / total;

    let report = Report {
        task: task_name.clone(),
        model_id,
        prompt_version: PROMPT_VERSION.to_string(),
        n_ctx: TASK_N_CTX,
        date,
        cases_file: cases_path.display().to_string(),
        total_cases: results.len(),
        needs_citation_accuracy,
        sentence_type_agreement,
        severity_agreement,
        validation_failure_rate,
        retry_rate,
        mean_latency_ms: total_ms as f64 / total,
        tokens_per_second: if total_ms == 0 {
            0.0
        } else {
            total_tokens as f64 / (total_ms as f64 / 1000.0)
        },
        results,
    };

    println!();
    println!("valid outputs           : {}/{}", valid_count, report.total_cases);
    let pct = |v: Option<f64>| v.map(|x| format!("{:.0}%", x * 100.0)).unwrap_or("n/a".into());
    println!("needs_citation accuracy : {}", pct(report.needs_citation_accuracy));
    println!("sentence_type agreement : {}", pct(report.sentence_type_agreement));
    println!("severity agreement      : {}", pct(report.severity_agreement));
    println!("validation failure rate : {:.0}%", report.validation_failure_rate * 100.0);
    println!("retry rate              : {:.0}%", report.retry_rate * 100.0);
    println!("mean latency            : {:.0} ms", report.mean_latency_ms);
    println!("tokens/sec              : {:.1}", report.tokens_per_second);

    std::fs::create_dir_all(&out_dir)?;
    let name = format!("{task_name}-{}-{}.json", report.prompt_version, report.date);
    let path = Path::new(&out_dir).join(name);
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
    println!("\nreport written to {}", path.display());
    Ok(())
}
