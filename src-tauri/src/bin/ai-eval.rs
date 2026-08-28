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
use app_lib::ai::evidence::{assemble, Assembled, EVIDENCE_BUDGET_TOKENS};
use app_lib::ai::tasks::citation_need::{CitationNeedInput, CitationNeedTask, PromptVariant};
use app_lib::ai::tasks::citation_support::CitationSupportTask;
use gaply_core::ai_engine::{embeddings as core_emb, registry, store};
use gaply_core::chunk::PagedChunk;
use gaply_core::Database;
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
    /// Style/length deviations the output was accepted WITH. Reported, never
    /// dropped — that is what separates this from a silent repair.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    advisories: Vec<String>,
    elapsed_ms: u64,
    tokens: usize,
    /// Latency breakdown (item 3): prefill is the single forward over the whole
    /// prompt, decode is every subsequent single-token forward. Kept apart
    /// because they scale with different things and are fixed by different
    /// means — a prefill-dominated profile argues for Metal, a decode-dominated
    /// one argues for a smaller model.
    prompt_tokens: usize,
    prefill_ms: u64,
    decode_ms: u64,
    decode_tokens_per_sec: f64,
    /// Evidence assembly, for the evidence-grounded tasks. `dropped` is what the
    /// budget removed: scope item 1 requires it to be RECORDED, not merely
    /// printed, because a case that scored badly with chunks dropped and one
    /// that scored badly with everything sent are different findings.
    #[serde(skip_serializing_if = "Option::is_none")]
    chunks_sent: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chunks_dropped: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence_words: Option<usize>,
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
    /// Share of ALL cases accepted with at least one advisory.
    advisory_rate: f64,
    mean_latency_ms: f64,
    tokens_per_second: f64,
    /// Aggregate latency split across every case.
    mean_prompt_tokens: f64,
    mean_prefill_ms: f64,
    mean_decode_ms: f64,
    /// Share of total model time spent in prefill. THE number item 3 exists to
    /// produce: it decides whether acceleration or a smaller model is the lever.
    prefill_share: f64,
    mean_decode_tokens_per_sec: f64,
    /// How often each answer was given, regardless of the label. A model that
    /// answers the same thing every time can post respectable accuracy on an
    /// unbalanced set while having learned nothing — accuracy alone hides that.
    answer_distribution: serde_json::Value,
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
    // Default the seed file to the TASK. A fixed default meant
    // `--task citation_support` silently scored the citation_need seeds and
    // printed a full, plausible, entirely meaningless report — the failure mode
    // that looks like a result.
    let cases_path = arg("--cases")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("evals/{task_name}.jsonl")));
    let date = arg("--date").unwrap_or_else(|| "unknown".to_string());
    let variant = arg("--prompt")
        .map(|v| {
            PromptVariant::parse(&v).unwrap_or_else(|| {
                eprintln!("unknown --prompt {v:?}; expected v1 or v2");
                std::process::exit(2)
            })
        })
        .unwrap_or(PromptVariant::V2);
    let out_dir = arg("--out").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("evals/reports"));


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
    if task_name == "citation_need" {
        println!("prompt  : {}", variant.version());
    }
    println!("n_ctx   : {TASK_N_CTX}");
    println!("cases   : {} from {}", cases.len(), cases_path.display());
    println!();

    if task_name == "citation_support" {
        return run_citation_support(cases, cases_path, date, out_dir, model_id, manager).await;
    }

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
        let task = CitationNeedTask::with_variant(input, variant);
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
                    advisories: run.advisories.iter().map(|a| a.to_string()).collect(),
                    elapsed_ms,
                    tokens: run.tokens,
                    prompt_tokens: run.timings.prompt_tokens,
                    prefill_ms: run.timings.prefill_ms,
                    decode_ms: run.timings.decode_ms,
                    decode_tokens_per_sec: if run.timings.decode_ms == 0 {
                        0.0
                    } else {
                        run.timings.tokens as f64 / (run.timings.decode_ms as f64 / 1000.0)
                    },
                    // citation_need sends no evidence block.
                    chunks_sent: None,
                    chunks_dropped: None,
                    evidence_words: None,
                }
            }
            Err(TaskError::ValidationFailed { errors, primary, first_raw, retry_raw, timings }) => {
                println!("  FAIL {}  {:>5}ms  fatal ({primary:?} attempt kept): {}", case.id, elapsed_ms,
                    errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "));
                CaseResult {
                    id: case.id.clone(),
                    outcome: Outcome::ValidationFailed,
                    // BOTH raw outputs — the report must show what the model
                    // actually said, not a summary of why it was rejected.
                    raw: Some(format!(
                        "--- attempt 1 ---\n{first_raw}\n--- retry ---\n{retry_raw}\n--- primary: {primary:?} ---"
                    )),
                    scored: None,
                    retried: true,
                    advisories: Vec::new(),
                    elapsed_ms,
                    // A failed run costs real model time — and the failures are
                    // the SLOWEST cases, so zeroing them would flatter every
                    // latency average in exactly the wrong direction.
                    tokens: timings.tokens,
                    prompt_tokens: timings.prompt_tokens,
                    prefill_ms: timings.prefill_ms,
                    decode_ms: timings.decode_ms,
                    decode_tokens_per_sec: if timings.decode_ms == 0 {
                        0.0
                    } else {
                        timings.tokens as f64 / (timings.decode_ms as f64 / 1000.0)
                    },
                    // citation_need sends no evidence block.
                    chunks_sent: None,
                    chunks_dropped: None,
                    evidence_words: None,
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
                    advisories: Vec::new(),
                    elapsed_ms,
                    tokens: 0,
                    prompt_tokens: 0,
                    prefill_ms: 0,
                    decode_ms: 0,
                    decode_tokens_per_sec: 0.0,
                    chunks_sent: None,
                    chunks_dropped: None,
                    evidence_words: None,
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
    let advisory_rate =
        results.iter().filter(|r| !r.advisories.is_empty()).count() as f64 / total;
    let sum = |f: fn(&CaseResult) -> f64| results.iter().map(f).sum::<f64>();
    let total_prefill = sum(|r| r.prefill_ms as f64);
    let total_decode = sum(|r| r.decode_ms as f64);
    let model_ms = total_prefill + total_decode;
    let mean_prompt_tokens = sum(|r| r.prompt_tokens as f64) / total;
    let mean_prefill_ms = total_prefill / total;
    let mean_decode_ms = total_decode / total;
    let prefill_share = if model_ms == 0.0 { 0.0 } else { total_prefill / model_ms };
    let dtps: Vec<f64> = results
        .iter()
        .filter(|r| r.decode_tokens_per_sec > 0.0)
        .map(|r| r.decode_tokens_per_sec)
        .collect();
    let mean_decode_tokens_per_sec = mean(&dtps).unwrap_or(0.0);

    // Answer distribution over the cases that produced output.
    let mut needs_true = 0usize;
    let mut types: std::collections::BTreeMap<String, usize> = Default::default();
    let mut sevs: std::collections::BTreeMap<String, usize> = Default::default();
    for r in results.iter().filter(|r| r.outcome == Outcome::Ok) {
        if let Some(got) = r.scored.as_ref().and_then(|s| s.get("got")) {
            if got.get("needs_citation").and_then(|v| v.as_bool()) == Some(true) {
                needs_true += 1;
            }
            if let Some(t) = got.get("sentence_type").and_then(|v| v.as_str()) {
                *types.entry(t.to_string()).or_default() += 1;
            }
            if let Some(v) = got.get("severity").and_then(|v| v.as_str()) {
                *sevs.entry(v.to_string()).or_default() += 1;
            }
        }
    }
    let answer_distribution = serde_json::json!({
        "needsCitationTrue": needs_true,
        "validOutputs": valid_count,
        "sentenceType": types,
        "severity": sevs,
    });

    let report = Report {
        task: task_name.clone(),
        model_id,
        prompt_version: variant.version().to_string(),
        n_ctx: TASK_N_CTX,
        date,
        cases_file: cases_path.display().to_string(),
        total_cases: results.len(),
        needs_citation_accuracy,
        sentence_type_agreement,
        severity_agreement,
        validation_failure_rate,
        retry_rate,
        advisory_rate,
        mean_latency_ms: total_ms as f64 / total,
        mean_prompt_tokens,
        mean_prefill_ms,
        mean_decode_ms,
        prefill_share,
        mean_decode_tokens_per_sec,
        answer_distribution: answer_distribution.clone(),
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
    println!("advisory rate           : {:.0}%", report.advisory_rate * 100.0);
    println!("mean latency            : {:.0} ms", report.mean_latency_ms);
    println!("tokens/sec (overall)    : {:.1}", report.tokens_per_second);
    println!();
    println!("mean prompt tokens      : {:.0}", report.mean_prompt_tokens);
    println!("mean prefill            : {:.0} ms", report.mean_prefill_ms);
    println!("mean decode             : {:.0} ms", report.mean_decode_ms);
    println!("prefill share of model  : {:.0}%", report.prefill_share * 100.0);
    println!("decode tokens/sec       : {:.1}", report.mean_decode_tokens_per_sec);
    println!();
    println!("answer distribution     : {}", serde_json::to_string(&answer_distribution)?);

    std::fs::create_dir_all(&out_dir)?;
    let name = format!("{task_name}-{}-{}.json", report.prompt_version, report.date);
    let path = Path::new(&out_dir).join(name);
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
    println!("\nreport written to {}", path.display());
    Ok(())
}

/* ===================== citation_support ================================== *
 * Builds a fixture corpus, retrieves per claim, and scores verdict agreement
 * plus whether the PLANTED passage was actually cited. That second column is
 * the one that matters: a right verdict citing the wrong passage is a right
 * answer for the wrong reason, and would look identical in an accuracy score. */

/// Index one fixture file and embed it in a mocked 3-dim space.
///
/// CI-safe by default: the mocked embedder scores lexical overlap with the
/// claim, which is enough to exercise the pipeline shape. With
/// GAPLY_TEST_EMBED_MODEL the real embedding engine is used instead.
fn build_fixture(db: &Database, name: &str, dir: &Path) -> Result<i64, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(dir.join(name))?;
    let doc = store::create_document(db, name, &dir.join(name).display().to_string(), name)?;
    registry::register_model(
        db,
        registry::ModelRow {
            id: "eval-emb".into(),
            kind: "embedding".into(),
            display_name: "eval".into(),
            file_path: "/x".into(),
            sha256: None,
            dim: Some(EVAL_DIM as i64),
            quant: None,
        },
    )?;
    // Paragraph-per-chunk keeps the planted passages intact and separable.
    let chunks: Vec<PagedChunk> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| p.split_whitespace().count() >= 8)
        .enumerate()
        .map(|(i, p)| PagedChunk {
            seq: i as i64,
            content: p.replace('\n', " "),
            token_estimate: p.split_whitespace().count(),
            page: Some((i as u32 / 3) + 1),
            section: None,
            char_start: 0,
            char_end: p.len(),
        })
        .collect();
    store::index_chunks(db, doc, &chunks)?;
    Ok(doc)
}

const EVAL_DIM: usize = 64;

/// Deterministic bag-of-words vector — lexical overlap stands in for semantic
/// similarity so CI needs no model. Reported as such in the eval output.
fn mock_vector(text: &str) -> Vec<f32> {
    let mut v = vec![0.0f32; EVAL_DIM];
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        if w.len() < 4 {
            continue;
        }
        let mut h: u64 = 1469598103934665603;
        for b in w.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        v[(h as usize) % EVAL_DIM] += 1.0;
    }
    core_emb::l2_normalize(&mut v);
    v
}

fn embed_fixture(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let space = core_emb::EmbeddingSpace::new("eval-emb", "mock-v1");
    let pending = core_emb::chunks_missing_embeddings(db, None, "eval-emb")?;
    let rows: Vec<(i64, Vec<f32>)> =
        pending.iter().map(|p| (p.chunk_id, mock_vector(&p.content))).collect();
    core_emb::put_embeddings(db, &space, &rows)?;
    Ok(())
}

async fn run_citation_support(
    cases: Vec<EvalCase>,
    cases_path: PathBuf,
    date: String,
    out_dir: PathBuf,
    model_id: String,
    manager: ModelManager,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_dir = cases_path.parent().unwrap_or(Path::new(".")).join("fixtures");
    let db = Database::in_memory()?;
    println!("prompt  : citation_support-v1");
    println!("evidence: budget {EVIDENCE_BUDGET_TOKENS} tokens, MOCKED lexical embedder");
    println!();

    let mut docs: std::collections::HashMap<String, i64> = Default::default();
    let mut results = Vec::new();

    for case in &cases {
        let fixture = case.input["fixture"].as_str().unwrap_or("__empty__").to_string();
        let claim = case.input["claim"].as_str().unwrap_or_default().to_string();
        let source = case.input["cited_source"].as_str().unwrap_or_default().to_string();

        let doc = if fixture == "__empty__" {
            // An indexed-but-empty document: the NoEvidence path.
            *docs.entry(fixture.clone()).or_insert_with(|| {
                store::create_document(&db, "empty", "/tmp/empty", "empty-doc").unwrap()
            })
        } else if let Some(d) = docs.get(&fixture) {
            *d
        } else {
            let d = build_fixture(&db, &fixture, &fixtures_dir)?;
            embed_fixture(&db)?;
            docs.insert(fixture.clone(), d);
            d
        };

        let started = std::time::Instant::now();
        let assembled = assemble(&db, doc, &claim, &mock_vector(&claim), EVIDENCE_BUDGET_TOKENS)?;
        let bundle = match assembled {
            Assembled::NoEvidence { reason } => {
                let expected_none =
                    case.expected.get("outcome").and_then(|v| v.as_str()) == Some("noEvidence");
                println!(
                    "  {} {}  NoEvidence (no model run): {reason}",
                    if expected_none { "PASS" } else { "MISS" },
                    case.id
                );
                results.push(CaseResult {
                    id: case.id.clone(),
                    outcome: if expected_none { Outcome::Ok } else { Outcome::Error },
                    raw: Some(format!("NoEvidence: {reason}")),
                    scored: Some(serde_json::json!({ "noEvidence": true, "expectedNoEvidence": expected_none })),
                    retried: false,
                    advisories: Vec::new(),
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    tokens: 0,
                    prompt_tokens: 0,
                    prefill_ms: 0,
                    decode_ms: 0,
                    decode_tokens_per_sec: 0.0,
                    chunks_sent: Some(0),
                    chunks_dropped: Some(0),
                    evidence_words: Some(0),
                });
                continue;
            }
            Assembled::Ready(b) => b,
        };

        let task = CitationSupportTask {
            claim: claim.clone(),
            cited_source: source,
            evidence: bundle.rendered.clone(),
        };
        let r = run_task(&manager, &task, &bundle.ctx, Arc::new(AtomicBool::new(false)), None).await;
        let elapsed_ms = started.elapsed().as_millis() as u64;

        results.push(match r {
            Ok(run) => {
                let got = serde_json::to_value(&run.output)?;
                let want = case.expected.get("verdict").and_then(|v| v.as_str());
                let verdict_ok = want.map(|w| got["verdict"] == w);
                // Did it cite the PLANTED passage? A right verdict citing the
                // wrong passage is a right answer for the wrong reason.
                let planted = case.expected.get("planted_chunk_contains").and_then(|v| v.as_str());
                let cited_planted = planted.map(|needle| {
                    run.output.supporting_chunks.iter().any(|sc| {
                        bundle
                            .ctx
                            .get(&sc.chunk_id)
                            .map(|c| c.text.contains(needle))
                            .unwrap_or(false)
                    })
                });
                println!(
                    "  {} {}  {:>5}ms  verdict={} sent={} dropped={}{}",
                    if verdict_ok == Some(true) { "PASS" } else { "MISS" },
                    case.id,
                    elapsed_ms,
                    got["verdict"],
                    bundle.chunks_sent,
                    bundle.chunks_dropped,
                    if run.retried { " (retried)" } else { "" }
                );
                CaseResult {
                    id: case.id.clone(),
                    outcome: Outcome::Ok,
                    raw: Some(serde_json::to_string(&run.output)?),
                    scored: Some(serde_json::json!({
                        "verdict": verdict_ok,
                        "citedPlantedChunk": cited_planted,
                        "got": got,
                        "expected": case.expected,
                    })),
                    retried: run.retried,
                    advisories: run.advisories.iter().map(|a| a.to_string()).collect(),
                    elapsed_ms,
                    tokens: run.timings.tokens,
                    prompt_tokens: run.timings.prompt_tokens,
                    prefill_ms: run.timings.prefill_ms,
                    decode_ms: run.timings.decode_ms,
                    decode_tokens_per_sec: 0.0,
                    chunks_sent: Some(bundle.chunks_sent),
                    chunks_dropped: Some(bundle.chunks_dropped),
                    evidence_words: Some(bundle.words_estimated),
                }
            }
            Err(TaskError::ValidationFailed { errors, primary, first_raw, retry_raw, timings }) => {
                println!(
                    "  FAIL {}  {:>5}ms  sent={} dropped={} fatal ({primary:?} kept): {}",
                    case.id,
                    elapsed_ms,
                    bundle.chunks_sent,
                    bundle.chunks_dropped,
                    errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
                );
                CaseResult {
                    id: case.id.clone(),
                    outcome: Outcome::ValidationFailed,
                    raw: Some(format!(
                        "--- attempt 1 ---\n{first_raw}\n--- retry ---\n{retry_raw}\n--- primary: {primary:?} ---"
                    )),
                    scored: None,
                    retried: true,
                    advisories: Vec::new(),
                    elapsed_ms,
                    tokens: timings.tokens,
                    prompt_tokens: timings.prompt_tokens,
                    prefill_ms: timings.prefill_ms,
                    decode_ms: timings.decode_ms,
                    decode_tokens_per_sec: 0.0,
                    chunks_sent: Some(bundle.chunks_sent),
                    chunks_dropped: Some(bundle.chunks_dropped),
                    evidence_words: Some(bundle.words_estimated),
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
                    advisories: Vec::new(),
                    elapsed_ms,
                    tokens: 0,
                    prompt_tokens: 0,
                    prefill_ms: 0,
                    decode_ms: 0,
                    decode_tokens_per_sec: 0.0,
                    chunks_sent: Some(bundle.chunks_sent),
                    chunks_dropped: Some(bundle.chunks_dropped),
                    evidence_words: Some(bundle.words_estimated),
                }
            }
        });
    }

    let total = results.len().max(1) as f64;
    let ok = results.iter().filter(|r| r.outcome == Outcome::Ok).count();
    let agree = |field: &str| -> Option<f64> {
        let xs: Vec<f64> = results
            .iter()
            .filter_map(|r| r.scored.as_ref()?.get(field)?.as_bool())
            .map(|b| if b { 1.0 } else { 0.0 })
            .collect();
        mean(&xs)
    };
    let pct = |v: Option<f64>| v.map(|x| format!("{:.0}%", x * 100.0)).unwrap_or("n/a".into());
    let report = serde_json::json!({
        "task": "citation_support",
        "modelId": model_id,
        "promptVersion": "citation_support-v1",
        "nCtx": app_lib::ai::generative::TASK_N_CTX,
        "evidenceBudgetTokens": EVIDENCE_BUDGET_TOKENS,
        "embedder": "mock-lexical-v1",
        "date": date,
        "casesFile": cases_path.display().to_string(),
        "totalCases": results.len(),
        "validOutputs": ok,
        "verdictAgreement": agree("verdict"),
        "citedPlantedChunk": agree("citedPlantedChunk"),
        "validationFailureRate": results.iter().filter(|r| r.outcome == Outcome::ValidationFailed).count() as f64 / total,
        "retryRate": results.iter().filter(|r| r.retried).count() as f64 / total,
        "advisoryRate": results.iter().filter(|r| !r.advisories.is_empty()).count() as f64 / total,
        "meanLatencyMs": results.iter().map(|r| r.elapsed_ms as f64).sum::<f64>() / total,
        "meanPromptTokens": results.iter().map(|r| r.prompt_tokens as f64).sum::<f64>() / total,
        "meanPrefillMs": results.iter().map(|r| r.prefill_ms as f64).sum::<f64>() / total,
        "meanDecodeMs": results.iter().map(|r| r.decode_ms as f64).sum::<f64>() / total,
        "results": results,
    });

    println!();
    println!("valid outputs           : {ok}/{}", results.len());
    println!("verdict agreement       : {}", pct(agree("verdict")));
    println!("cited the PLANTED chunk : {}", pct(agree("citedPlantedChunk")));
    println!("validation failure rate : {:.0}%", report["validationFailureRate"].as_f64().unwrap_or(0.0) * 100.0);
    println!("retry rate              : {:.0}%", report["retryRate"].as_f64().unwrap_or(0.0) * 100.0);
    println!("advisory rate           : {:.0}%", report["advisoryRate"].as_f64().unwrap_or(0.0) * 100.0);
    println!("mean latency            : {:.0} ms", report["meanLatencyMs"].as_f64().unwrap_or(0.0));
    println!("mean prompt tokens      : {:.0}", report["meanPromptTokens"].as_f64().unwrap_or(0.0));
    println!("mean prefill            : {:.0} ms", report["meanPrefillMs"].as_f64().unwrap_or(0.0));

    std::fs::create_dir_all(&out_dir)?;
    let path = out_dir.join(format!("citation_support-v1-{date}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
    println!("\nreport written to {}", path.display());
    Ok(())
}
