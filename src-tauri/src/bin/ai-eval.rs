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

use app_lib::ai::generative::{BundledGenerativeLoader, InstalledGenerativeLoader, TASK_N_CTX};
use app_lib::ai::model_manager::{BackendLoader, ModelManager};
use app_lib::ai::task::{run_task, Attempt, TaskContext, TaskError};
use app_lib::ai::evidence::{assemble, Assembled, EVIDENCE_BUDGET_TOKENS};
use app_lib::ai::tasks::citation_need::{CitationNeedInput, CitationNeedTask, PromptVariant};
use app_lib::ai::tasks::citation_support::{CitationSupportTask, CitationSupportV2Task};
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
    /// Why each attempt stopped, in attempt order (§11 D27). `maxTokens` here
    /// means the reply was CUT OFF — the case failed because the model ran out
    /// of room, which is a different finding from a model that said something
    /// wrong, and the two were indistinguishable in every Phase 6 report.
    stop_reasons: Vec<String>,
    /// The primary attempt's fatal errors, verbatim. Stored rather than only
    /// printed so a failure CATEGORY (chunk_id format, page mismatch, truncation)
    /// is countable from the report without re-reading stdout.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fatal_errors: Vec<String>,
    /// §11 D28. Substantive evidence/claim mismatches found by running the
    /// faithfulness checker on a REJECTED but schema-shaped output.
    ///
    /// Reported separately from accepted faithfulness and folded into no score.
    /// Its presence here NEVER changes `outcome` — this field exists precisely
    /// because the information was previously lost behind a format error.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostic_faithfulness: Vec<String>,
    /// Derived from `stop_reasons`, so truncation is countable without
    /// re-deriving it in every consumer. NEVER used to accept an output.
    truncated: bool,
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
    /// WHICH COMPILER built this binary and WHICH OS ran it (§11 D61).
    ///
    /// A 2.65x CPU prefill regression against D34 had to be attributed between
    /// the OS and the toolchain, and neither was recorded. Excluding the
    /// toolchain took forensics on `~/.rustup` directory mtimes — circumstantial
    /// evidence that stops working the moment anyone runs `rustup update`. Two
    /// fields turn that into a lookup. Timing cells whose `rustc` or `os`
    /// differ are not comparable, the same rule `loadContext` already carries.
    rustc: String,
    os: String,
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
    /// How many DISTINCT `severity` values the valid outputs carried (§11 D85).
    ///
    /// `severity_agreement` above is scored against three-valued labels, so
    /// when this is 1 the model graded everything the same and the percentage
    /// is a property of the LABEL distribution, not of the model's judgement —
    /// it cannot move for any quality-related reason. v4 collapsed to `high`
    /// 37/37; v3 on the same model and corpus was `{high: 20, low: 23}`, so the
    /// collapse is a fact about the PROMPT and worth keeping visible rather
    /// than deleting the metric over.
    ///
    /// Computed from the distribution the report already builds, so it turns
    /// itself off the moment a prompt grades again.
    severity_distinct_values: usize,
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

/// `StopReason` -> the string that lands in the report, and whether ANY attempt
/// was cut off at the ceiling (§11 D27).
///
/// A truncated reply is unparseable and therefore already a failure; this
/// records WHY it failed. It never makes an output valid.
fn stops(reasons: &[app_lib::ai::generative::StopReason]) -> (Vec<String>, bool) {
    use app_lib::ai::generative::StopReason;
    let names: Vec<String> = reasons
        .iter()
        .map(|r| match r {
            StopReason::EndOfTurn => "endOfTurn".to_string(),
            StopReason::MaxTokens => "maxTokens".to_string(),
            StopReason::Cancelled => "cancelled".to_string(),
        })
        .collect();
    let truncated = reasons.iter().any(|r| matches!(r, StopReason::MaxTokens));
    (names, truncated)
}

/// sha256 of the running binary (§11 D27/D29 reporting).
///
/// A bake-off cell is only comparable to another cell run by the SAME binary.
/// Phase 6 learned this the hard way: `ai-eval.rs` was edited mid-matrix and
/// only the mtime showed it. Recording the hash IN the report makes a mixed
/// series self-evident instead of a thing to reconstruct from timestamps.
fn self_hash() -> Option<String> {
    use sha2::{Digest, Sha256};
    let exe = std::env::current_exe().ok()?;
    let bytes = std::fs::read(exe).ok()?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Some(format!("{:x}", h.finalize()))
}

/// Decode rate for one case: generated tokens over decode time.
///
/// §11 D31. This was hardcoded `0.0` at all four citation_support sites while
/// citation_need computed it inline, so the support arm reported a decode rate
/// of zero in every cell ever produced. Two of those sites are genuinely
/// zero-generation paths (NoEvidence, generation error) where 0.0 was right by
/// accident; the accepted and validation-failed sites were discarding real
/// measurements.
///
/// One function, one guard, used by BOTH arms — the duplicated inline
/// expression is what let the two arms drift apart in the first place.
fn decode_tps(tokens: usize, decode_ms: u64) -> f64 {
    if decode_ms == 0 {
        0.0
    } else {
        tokens as f64 / (decode_ms as f64 / 1000.0)
    }
}

/// 1-minute load average, or `None` where the platform will not say.
///
/// §11 D33. Phase 6a's 3B cells were measured while clippy and the test suite
/// ran on the same machine: prefill cost 95.3 ms per prompt token there and
/// 34.4 ms in Phase 6b on near-identical prompts. Nothing in the engine can
/// cause that. The numbers fed a model decision before anyone noticed.
///
/// Recording load makes the contamination visible IN the report instead of
/// reconstructable from what else was running an hour ago.
#[cfg(target_os = "macos")]
fn load_avg_1m() -> Option<f64> {
    let mut avg = [0.0f64; 3];
    // SAFETY: getloadavg writes at most `nelem` doubles into the buffer, and
    // the buffer holds 3.
    let n = unsafe { libc::getloadavg(avg.as_mut_ptr(), 3) };
    if n >= 1 {
        Some(avg[0])
    } else {
        None
    }
}

/// Non-macOS: say nothing rather than report a zero that reads as "idle".
#[cfg(not(target_os = "macos"))]
fn load_avg_1m() -> Option<f64> {
    None
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


/// Build the bake-off comparison document from report JSONs already on disk.
///
/// Separate from the runs on purpose: the matrix takes hours, and a formatting
/// mistake in the summary must never be a reason to run it again. Every number
/// here is READ from a report — nothing is recomputed, so the document cannot
/// disagree with the reports it cites.
fn write_bakeoff(prefix: &str, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut reports: Vec<(String, serde_json::Value)> = Vec::new();
    for e in std::fs::read_dir(out_dir)? {
        let path = e?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if !name.ends_with(".json") || !name.contains(prefix) {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        reports.push((name, v));
    }
    if reports.is_empty() {
        return Err(format!("no reports matching {prefix:?} in {}", out_dir.display()).into());
    }
    reports.sort_by(|a, b| a.0.cmp(&b.0));

    let g = |v: &serde_json::Value, k: &str| -> String {
        match v.get(k) {
            Some(serde_json::Value::Number(n)) => {
                let f = n.as_f64().unwrap_or(0.0);
                if f.fract() == 0.0 { format!("{f:.0}") } else { format!("{f:.2}") }
            }
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Null) | None => "—".into(),
            Some(other) => other.to_string(),
        }
    };
    let pctf = |v: &serde_json::Value, k: &str| -> String {
        v.get(k)
            .and_then(|x| x.as_f64())
            .map(|f| format!("{:.0}%", f * 100.0))
            .unwrap_or_else(|| "—".into())
    };

    let mut md = String::new();
    md.push_str("# Generative model bake-off\n\n");
    md.push_str(
        "Every figure is read from the report JSONs in this directory; nothing is recomputed here.\n\
         Runs were SEQUENTIAL on one machine — concurrent runs would contend for CPU and make every\n\
         latency column meaningless.\n\n\
         Scope limit (§11 D21): these are the Qwen2.5 sizes the vendored qwen2 loader can load. A\n\
         winner here is the best of THESE, not the best model available.\n\n",
    );

    md.push_str("## All cells\n\n");
    md.push_str(
        "| model | task / prompt | valid | fatal | retry | advisory | mean latency | prefill | prompt tok | load | RAM |\n\
         |---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for (_, r) in &reports {
        md.push_str(&format!(
            "| {} | {} | {}/{} | {} | {} | {} | {} ms | {} ms | {} | {} ms | {} |\n",
            g(r, "modelId"),
            g(r, "promptVersion"),
            g(r, "validOutputs"),
            g(r, "totalCases"),
            pctf(r, "validationFailureRate"),
            pctf(r, "retryRate"),
            pctf(r, "advisoryRate"),
            g(r, "meanLatencyMs"),
            g(r, "meanPrefillMs"),
            g(r, "meanPromptTokens"),
            g(r, "modelLoadMs"),
            g(r, "ramTotalMb"),
        ));
    }

    md.push_str("\n## citation_need — accuracy against the labels\n\n");
    md.push_str("| model | needs_citation acc | sentence_type | severity | answer distribution (collapse check) |\n|---|---|---|---|---|\n");
    for (_, r) in reports.iter().filter(|(_, r)| g(r, "task") == "citation_need") {
        md.push_str(&format!(
            "| {} | {} | {} | {} | `{}` |\n",
            g(r, "modelId"),
            pctf(r, "needsCitationAccuracy"),
            pctf(r, "sentenceTypeAgreement"),
            // §11 D85. A collapsed column next to a varying one, compared as
            // if they were the same kind of number, is exactly what this table
            // invites — so it says which is which.
            match r.get("severityDistinctValues").and_then(|v| v.as_u64()) {
                Some(1) => format!("{} (degenerate)", pctf(r, "severityAgreement")),
                _ => pctf(r, "severityAgreement"),
            },
            r.get("answerDistribution").map(|v| v.to_string()).unwrap_or_default(),
        ));
    }

    md.push_str("\n## citation_support — grounding and faithfulness\n\n");
    md.push_str("| model | prompt | verdict agree | cited planted chunk | FAITHFULNESS violations | cases |\n|---|---|---|---|---|---|\n");
    for (_, r) in reports.iter().filter(|(_, r)| g(r, "task") == "citation_support") {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} of {} | {} |\n",
            g(r, "modelId"),
            g(r, "promptVersion"),
            pctf(r, "verdictAgreement"),
            pctf(r, "citedPlantedChunk"),
            g(r, "faithfulnessViolations"),
            g(r, "faithfulnessCheckedCases"),
            r.get("faithfulnessViolationCases").map(|v| v.to_string()).unwrap_or_default(),
        ));
    }

    md.push_str("\n## Retry-collapse check\n\n");
    md.push_str(
        "Phase 5 measured every 0.5B retry collapsing to `{\"answer\": \"Corrected.\"}`. \
         Whether that survives at larger sizes is a property of the retry PROMPT, not of the task.\n\n",
    );
    md.push_str("| model | prompt | retries that produced `answer: Corrected.` | retries total |\n|---|---|---|---|\n");
    for (_, r) in &reports {
        let mut collapsed = 0usize;
        let mut retried = 0usize;
        if let Some(rs) = r.get("results").and_then(|v| v.as_array()) {
            for c in rs {
                if c.get("retried").and_then(|v| v.as_bool()).unwrap_or(false) {
                    retried += 1;
                }
                let raw = c.get("raw").and_then(|v| v.as_str()).unwrap_or("");
                // Only the RETRY half of the recorded raw text.
                if let Some(after) = raw.split("--- retry ---").nth(1) {
                    if after.contains("\"answer\"") {
                        collapsed += 1;
                    }
                }
            }
        }
        md.push_str(&format!(
            "| {} | {} | {collapsed} | {retried} |\n",
            g(r, "modelId"),
            g(r, "promptVersion")
        ));
    }

    md.push_str("\n## Sources\n\n");
    for (n, _) in &reports {
        md.push_str(&format!("- `{n}`\n"));
    }

    let path = out_dir.join(format!("bakeoff-{prefix}.md"));
    std::fs::write(&path, md)?;
    println!("bake-off comparison written to {}", path.display());
    Ok(())
}

/// The OS this cell ran on. `uname -r` is the kernel (Darwin) version, which is
/// what distinguishes a macOS major release for our purposes and needs no crate.
fn os_version() -> String {
    let kernel = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    #[cfg(target_os = "macos")]
    {
        let product = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        return format!("macOS {product} (Darwin {kernel})");
    }
    #[cfg(not(target_os = "macos"))]
    format!("{} (kernel {kernel})", std::env::consts::OS)
}

/// §11 D85. The warning that must ride along with `severityAgreement`.
///
/// `severity` is scored against three-valued labels. When every valid output
/// carried the SAME value there is no variance to score, so the percentage is a
/// property of how many labels happen to say that value — it cannot move for
/// any reason to do with the model's judgement, and a bare "36%" reads as a
/// measurement of exactly the thing it cannot measure.
///
/// Keyed on the distribution the report already builds rather than on a
/// hardcoded value, so it turns itself off if a prompt ever grades again. v3 on
/// this model and corpus produced `{high: 20, low: 23}`; v4 produced
/// `{high: 37}`. The collapse is a fact about the PROMPT, which is why the
/// metric is annotated rather than deleted.
fn degenerate_severity_note(sevs: &std::collections::BTreeMap<String, usize>) -> Option<String> {
    let (only, _) = sevs.iter().next().filter(|_| sevs.len() == 1)?;
    Some(format!(
        "  [DEGENERATE: every valid output said \"{only}\" — no variance, so this \
         cannot measure judgement]"
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Summarise-only mode: builds the comparison document from reports already
    // on disk, without running a model.
    if let Some(prefix) = arg("--bakeoff") {
        let out = arg("--out").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("evals/reports"));
        return write_bakeoff(&prefix, &out);
    }

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

    // Argument validation belongs together and BEFORE any policy refusal: a
    // mistyped flag is the user's slip and should be named first, while a
    // missing one is a decision this harness is making for them (§11 D107).
    let support_variant = match arg("--support-variant").as_deref() {
        None | Some("v1") => SupportVariant::V1,
        Some("v2") => SupportVariant::V2,
        Some("v1c") => SupportVariant::V1ChunkBound,
        Some(o) => return Err(format!("unknown --support-variant {o:?}; use v1, v2 or v1c").into()),
    };


    let text = std::fs::read_to_string(&cases_path)
        .map_err(|e| format!("reading {}: {e}", cases_path.display()))?;
    let cases: Vec<EvalCase> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    // --model selects a generative model by REGISTRY ID (§9.9: swapping the
    // model is a registry change plus a download, never a code change). Absent,
    // it stays on the bundled 0.5B so every existing invocation behaves as it
    // did.
    let (loader, model_id): (Arc<dyn BackendLoader>, String) = match arg("--model") {
        None => {
            let l = BundledGenerativeLoader::resolve()
                .ok_or("no generative model resolves — set GAPLY_TEST_GEN_MODEL")?;
            let id = l.model_id();
            (Arc::new(l), id)
        }
        Some(id) => {
            // An unknown id must fail HERE, naming what exists. Falling back to
            // the bundled model would silently report 0.5B numbers under
            // another model's name — the worst possible outcome for a bake-off.
            if app_lib::ai::gen_install::candidate(&id).is_none() {
                return Err(format!(
                    "unknown --model {id:?}. Pinned candidates: {}",
                    app_lib::ai::gen_install::CANDIDATES
                        .iter()
                        .map(|c| c.registry_id)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .into());
            }
            // --model-dir points at a directory holding the GGUF + tokenizer,
            // for running candidates that are downloaded but not installed into
            // the app data dir.
            let dir = arg("--model-dir")
                .map(PathBuf::from)
                .ok_or("--model requires --model-dir <dir containing the gguf and tokenizer.json>")?;
            let c = app_lib::ai::gen_install::candidate(&id).expect("checked above");
            let l = InstalledGenerativeLoader::from_paths(
                &id,
                dir.join(c.gguf.local_name()),
                dir.join(c.tokenizer.local_name()),
            )?;
            (Arc::new(l), id)
        }
    };

    // §11 D29 stamped a mocked run `embedderIsReal: false`; §11 D107 makes it
    // REFUSE. The stamp is honest and was still walked past: a bare
    // `--task citation_support` ran the 0.5B against lexical mock retrieval and
    // printed a complete, plausible report, and the header warning did not stop
    // it being read as a result. A report nobody should quote should not be
    // produced by default.
    //
    // Checked BEFORE the model loads, so a refusal costs no time — and
    // `--smoke` still runs it, because a pipeline check is a real use that just
    // has to be asked for by name.
    if task_name == "citation_support" && !std::env::args().any(|a| a == "--smoke") {
        let mut missing = Vec::new();
        if arg("--model").is_none() {
            missing.push("  --model <id> --model-dir <dir>   (absent: the BUNDLED 0.5B runs)");
        }
        if arg("--embedder-dir").is_none() {
            missing.push("  --embedder-dir <dir>            (absent: retrieval is lexical mock)");
        }
        if !missing.is_empty() {
            eprintln!("citation_support needs the real model AND the real embedder,");
            eprintln!("or the run measures nothing (§11 D107). Missing:\n");
            for m in &missing {
                eprintln!("{m}");
            }
            eprintln!("\nAdd --smoke to run it anyway as a pipeline check. That report is");
            eprintln!("stamped embedderIsReal:false and must not be quoted as a result.");
            std::process::exit(2);
        }
    }

    let manager = ModelManager::new(loader);
    let ram = manager.ram_estimate().ok();
    let ram_mb = ram.as_ref().map(|r| r.total_bytes / (1024 * 1024));

    // Load time is a real cost of switching models and belongs in the report:
    // a model that scores well but takes 40 s to become usable is a different
    // product decision from one that loads in 4 s.
    //
    // The manager loads LAZILY, so this forces and times a real load. Timing
    // `ModelManager::new` instead would report ~0 ms for every model — a number
    // that reads as a measurement and is actually nothing.
    let load_started = std::time::Instant::now();
    let load_ms = match manager.acquire().await {
        Ok(lease) => {
            let ms = load_started.elapsed().as_millis() as u64;
            drop(lease);
            ms
        }
        Err(e) => return Err(format!("loading {model_id}: {e}").into()),
    };

    println!("task    : {task_name}");
    println!("model   : {model_id}");
    if let Some(r) = &ram {
        println!("ram     : {} MB estimated", r.total_bytes / (1024 * 1024));
    }
    println!("load    : {load_ms} ms");
    if task_name == "citation_need" {
        println!("prompt  : {}", variant.version());
    }
    println!("n_ctx   : {TASK_N_CTX}");
    println!("cases   : {} from {}", cases.len(), cases_path.display());
    println!();

    if task_name == "citation_support" {
        let embedder = match arg("--embedder-dir") {
            Some(dir) => {
                let dir = PathBuf::from(dir);
                let engine = app_lib::ai::embeddings::EmbeddingEngine::load_verified(&dir)
                    .map_err(|e| format!("loading the embedder from {}: {e}", dir.display()))?;
                EvalEmbedder::Real(Box::new(engine))
            }
            None => EvalEmbedder::Mock,
        };
        return run_citation_support(
            cases, cases_path, date, out_dir, model_id, manager, support_variant, load_ms,
            ram_mb, embedder,
        )
        .await;
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
                let (stop_reasons, truncated) = stops(&run.stop_reasons);
                CaseResult {
                    id: case.id.clone(),
                    diagnostic_faithfulness: Vec::new(),
                    fatal_errors: Vec::new(),
                    stop_reasons,
                    truncated,
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
                    decode_tokens_per_sec: decode_tps(run.timings.tokens, run.timings.decode_ms),
                    // citation_need sends no evidence block.
                    chunks_sent: None,
                    chunks_dropped: None,
                    evidence_words: None,
                }
            }
            Err(TaskError::ValidationFailed { errors, primary, first_raw, retry_raw, timings, stop_reasons }) => {
                println!("  FAIL {}  {:>5}ms  fatal ({primary:?} attempt kept): {}", case.id, elapsed_ms,
                    errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "));
                let (stop_reasons, truncated) = stops(&stop_reasons);
                CaseResult {
                    id: case.id.clone(),
                    diagnostic_faithfulness: Vec::new(),
                    fatal_errors: errors.iter().map(|e| e.to_string()).collect(),
                    stop_reasons,
                    truncated,
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
                    decode_tokens_per_sec: decode_tps(timings.tokens, timings.decode_ms),
                    // citation_need sends no evidence block.
                    chunks_sent: None,
                    chunks_dropped: None,
                    evidence_words: None,
                }
            }
            Err(e) => {
                println!("  ERR  {}  {e}", case.id);
                let (stop_reasons, truncated) = (Vec::new(), false);
                CaseResult {
                    id: case.id.clone(),
                    diagnostic_faithfulness: Vec::new(),
                    fatal_errors: Vec::new(),
                    stop_reasons,
                    truncated,
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
                    decode_tokens_per_sec: decode_tps(0, 0),
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
        rustc: env!("GAPLY_BUILD_RUSTC").to_string(),
        os: os_version(),
        model_id,
        prompt_version: variant.version().to_string(),
        n_ctx: TASK_N_CTX,
        date,
        cases_file: cases_path.display().to_string(),
        total_cases: results.len(),
        needs_citation_accuracy,
        sentence_type_agreement,
        severity_agreement,
        severity_distinct_values: sevs.len(),
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
    // §11 D85. Never print this percentage bare when the field has no variance.
    println!(
        "severity agreement      : {}{}",
        pct(report.severity_agreement),
        degenerate_severity_note(&sevs).unwrap_or_default()
    );
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

/// Which embedder assembles the evidence (§11 D29).
///
/// The Phase 6 support cells all ran `Mock`, and retrieval feeds the prompt, so
/// the whole arm measured the models against evidence a real install would
/// never have selected. `Real` is now mandatory for a bake-off; `Mock` stays
/// for CI, which must not need a 134 MB download.
enum EvalEmbedder {
    Mock,
    Real(Box<app_lib::ai::embeddings::EmbeddingEngine>),
}

impl EvalEmbedder {
    fn model_id(&self) -> &'static str {
        match self {
            EvalEmbedder::Mock => "mock-lexical-v1",
            EvalEmbedder::Real(_) => app_lib::ai::EMBED_MODEL_ID,
        }
    }
    fn preprocessing_version(&self) -> &'static str {
        match self {
            EvalEmbedder::Mock => "mock-v1",
            EvalEmbedder::Real(_) => app_lib::ai::PREPROCESSING_VERSION,
        }
    }
    fn dim(&self) -> usize {
        match self {
            EvalEmbedder::Mock => EVAL_DIM,
            EvalEmbedder::Real(_) => app_lib::ai::EMBED_DIM,
        }
    }
    fn space(&self) -> core_emb::EmbeddingSpace {
        core_emb::EmbeddingSpace::new(self.model_id(), self.preprocessing_version())
    }
    fn is_real(&self) -> bool {
        matches!(self, EvalEmbedder::Real(_))
    }
    /// Passage side — no prefix, per the model card.
    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        Ok(match self {
            EvalEmbedder::Mock => texts.iter().map(|t| mock_vector(t)).collect(),
            EvalEmbedder::Real(e) => e.embed_documents(texts)?,
        })
    }
    /// Query side — the real engine applies EMBED_QUERY_PREFIX. Using the
    /// document path for a query would be off-distribution for a model trained
    /// with an asymmetric prefix, which is exactly the kind of silent mismatch
    /// PREPROCESSING_VERSION exists to make visible.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        Ok(match self {
            EvalEmbedder::Mock => mock_vector(text),
            EvalEmbedder::Real(e) => e.embed_query(text)?,
        })
    }
}

/// Index one fixture file and register the embedding model actually in use.
fn build_fixture(
    db: &Database,
    name: &str,
    dir: &Path,
    emb: &EvalEmbedder,
) -> Result<i64, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(dir.join(name))?;
    let doc = store::create_document(db, name, &dir.join(name).display().to_string(), name)?;
    registry::register_model(
        db,
        registry::ModelRow {
            id: emb.model_id().into(),
            kind: "embedding".into(),
            display_name: emb.model_id().into(),
            file_path: "/x".into(),
            sha256: None,
            dim: Some(emb.dim() as i64),
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

fn embed_fixture(db: &Database, emb: &EvalEmbedder) -> Result<(), Box<dyn std::error::Error>> {
    let space = emb.space();
    let pending = core_emb::chunks_missing_embeddings(db, None, emb.model_id())?;
    let texts: Vec<String> = pending.iter().map(|p| p.content.clone()).collect();
    let vectors = emb.embed_documents(&texts)?;
    let rows: Vec<(i64, Vec<f32>)> =
        pending.iter().map(|p| p.chunk_id).zip(vectors).collect();
    core_emb::put_embeddings(db, &space, &rows)?;
    Ok(())
}

/// Which citation_support prompt variant to run. v2 adds the mandatory quote
/// (§11 D24); everything else is identical, which is what makes the comparison
/// meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupportVariant {
    V1,
    V2,
    /// §11 D64 EXPERIMENT: v1.6 plus the chunk bound, stated plainly. Not a
    /// shipped configuration — reachable only from this harness.
    V1ChunkBound,
}

impl SupportVariant {
    /// DELEGATED, never duplicated. These strings name the report file and fill
    /// `promptVersion`; when they were literals here, the §11 D26 bump to
    /// v1.1/v2.1 would have left every repaired report labelled with the old
    /// generation — the exact conflation the bump exists to prevent.
    fn version(self) -> &'static str {
        match self {
            SupportVariant::V1 => app_lib::ai::tasks::citation_support::PROMPT_VERSION,
            SupportVariant::V2 => app_lib::ai::tasks::citation_support::PROMPT_VERSION_V2,
            SupportVariant::V1ChunkBound => {
                app_lib::ai::tasks::citation_support::PROMPT_VERSION_V17
            }
        }
    }

    /// The generation ceiling this variant runs at (§11 D27), read from the
    /// task rather than restated.
    fn ceiling(self) -> usize {
        use app_lib::ai::task::AiTask;
        match self {
            SupportVariant::V1 => {
                <app_lib::ai::tasks::citation_support::CitationSupportTask as AiTask>::max_tokens()
            }
            SupportVariant::V2 => {
                <app_lib::ai::tasks::citation_support::CitationSupportV2Task as AiTask>::max_tokens()
            }
            SupportVariant::V1ChunkBound => {
                <app_lib::ai::tasks::citation_support::CitationSupportV17Task as AiTask>::max_tokens()
            }
        }
    }
}

/// Does this output assert a finding that the planted passage says is absent?
///
/// String-level and deliberately crude (§11 D25): it flags for HUMAN REVIEW, it
/// does not score. The condition is narrow on purpose — the planted chunk must
/// actually be cited, and the explanation must use language asserting the
/// finding exists. A model that correctly says "no effect was found" uses none
/// of these words.
fn faithfulness_violation(
    out: &app_lib::ai::tasks::citation_support::CitationSupportOutput,
    ctx: &TaskContext,
    spec: &serde_json::Value,
) -> Option<String> {
    let needle = spec.get("planted_chunk_contains")?.as_str()?;
    let cited_planted = out.supporting_chunks.iter().any(|sc| {
        ctx.get(&sc.chunk_id).map(|c| c.text.contains(needle)).unwrap_or(false)
    });
    if !cited_planted {
        return None;
    }
    let expl = out.explanation.to_lowercase();
    let hit = spec
        .get("forbidden_language")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .find(|w| expl.contains(&w.to_lowercase()))?;
    Some(hit.to_string())
}

/* ------------------- DIAGNOSTIC faithfulness (§11 D28) -------------------- *
 * D25's checker only ever ran on ACCEPTED outputs, and on the whole Phase 6
 * matrix the only accepted outputs were `insufficient_evidence` and
 * `NoEvidence` — both of which cite nothing. "0 violations of 2 checked" in all
 * nine cells therefore meant the check had never examined a citation.
 *
 * Everything below runs on REJECTED outputs and is reported SEPARATELY. None of
 * it can accept anything: it takes a parsed clone, returns strings, and is
 * called after the outcome is already ValidationFailed. */

/// Recover the bare id a composite `chunk_id` was trying to name.
///
/// DIAGNOSTIC ONLY. The validator must keep rejecting `"c13 | p.5 | -"` (§11
/// D26) — that rejection is the D18 grounding guarantee. But once an output is
/// already rejected, refusing to read its intent throws away the evidence of
/// what the model actually meant, which is the information cs-seed-01 lost.
fn recover_chunk_id(raw: &str) -> &str {
    let t = raw.trim();
    let t = t.strip_prefix('[').unwrap_or(t);
    let t = t.strip_prefix("CHUNK_ID=").unwrap_or(t);
    t.split(|c: char| c.is_whitespace() || c == '|' || c == ']').next().unwrap_or(t)
}

/// Numbers that appear in a claim element, as bare digit runs.
fn numbers_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Substantive evidence/claim mismatches, as human-readable strings.
///
/// Two narrow, string-level checks, both in D25's spirit — they FLAG, they
/// never score:
///
/// 1. D25's forbidden-language check, when the seed carries a `faithfulness`
///    block (cs-seed-03 / cs-seed-04).
/// 2. A claim element marked `found` that carries a NUMBER which appears
///    nowhere in the evidence the output actually cited. This is the check that
///    exposes the 3B's cs-seed-01 failure: it marked `by about 31 percent` as
///    `found` while quoting a sentence containing no number at all. A number is
///    the one thing that cannot be paraphrased into existence, which is why it
///    is safe to check by string.
fn diagnostic_mismatches(
    out: &app_lib::ai::tasks::citation_support::CitationSupportOutput,
    ctx: &TaskContext,
    spec: Option<&serde_json::Value>,
) -> Vec<String> {
    use app_lib::ai::tasks::citation_support::ElementStatus;
    let mut hits = Vec::new();

    if let Some(spec) = spec {
        if let Some(word) = faithfulness_violation_recovered(out, ctx, spec) {
            hits.push(format!(
                "explanation asserts a finding the planted passage reports as ABSENT (matched {word:?})"
            ));
        }
    }

    // The text the output actually pointed at: cited chunks plus any quotes.
    let mut cited = String::new();
    for sc in &out.supporting_chunks {
        if let Some(c) = ctx.get(recover_chunk_id(&sc.chunk_id)) {
            cited.push_str(&c.text);
            cited.push(' ');
        }
        if let Some(q) = &sc.quote {
            cited.push_str(q);
            cited.push(' ');
        }
    }

    for el in &out.claim_elements {
        if el.status != ElementStatus::Found {
            continue;
        }
        for n in numbers_in(&el.element) {
            if !cited.contains(&n) {
                hits.push(format!(
                    "claim_element {:?} is marked found, but {:?} appears in none of the cited evidence",
                    el.element, n
                ));
            }
        }
    }
    hits
}

/// D25's check, with DIAGNOSTIC id recovery so a composite chunk_id no longer
/// hides the faithfulness question behind a format error.
fn faithfulness_violation_recovered(
    out: &app_lib::ai::tasks::citation_support::CitationSupportOutput,
    ctx: &TaskContext,
    spec: &serde_json::Value,
) -> Option<String> {
    let needle = spec.get("planted_chunk_contains")?.as_str()?;
    let cited_planted = out.supporting_chunks.iter().any(|sc| {
        ctx.get(recover_chunk_id(&sc.chunk_id)).map(|c| c.text.contains(needle)).unwrap_or(false)
    });
    if !cited_planted {
        return None;
    }
    let expl = out.explanation.to_lowercase();
    let hit = spec
        .get("forbidden_language")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .find(|w| expl.contains(&w.to_lowercase()))?;
    Some(hit.to_string())
}

/// Parse a rejected raw attempt far enough to inspect it. Returns `None` when
/// the output never became schema-shaped — a truncated reply has nothing to
/// diagnose, and saying so is more honest than reporting zero findings.
fn parse_for_diagnosis(
    raw: &str,
) -> Option<app_lib::ai::tasks::citation_support::CitationSupportOutput> {
    let json = app_lib::ai::task::extract_json(raw)?;
    serde_json::from_str(json).ok()
}

async fn run_citation_support(
    cases: Vec<EvalCase>,
    cases_path: PathBuf,
    date: String,
    out_dir: PathBuf,
    model_id: String,
    manager: ModelManager,
    variant: SupportVariant,
    load_ms: u64,
    ram_mb: Option<u64>,
    embedder: EvalEmbedder,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_dir = cases_path.parent().unwrap_or(Path::new(".")).join("fixtures");
    let db = Database::in_memory()?;
    let load_start = load_avg_1m();
    let ran_isolated = std::env::args().any(|a| a == "--isolated");
    let ceiling = variant.ceiling();
    println!("prompt  : {}", variant.version());
    println!("ceiling : {ceiling} max_tokens (§11 D27)");
    println!(
        "load    : 1m avg {} at start{}",
        load_start.map(|l| format!("{l:.2}")).unwrap_or_else(|| "unknown".into()),
        if ran_isolated { ", declared ISOLATED" } else { ", NOT declared isolated" }
    );
    println!(
        "evidence: budget {EVIDENCE_BUDGET_TOKENS} tokens, embedder {} ({}){}",
        embedder.model_id(),
        embedder.preprocessing_version(),
        if embedder.is_real() { "" } else { "  *** MOCKED - not a valid bake-off (§11 D29) ***" }
    );
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
            let d = build_fixture(&db, &fixture, &fixtures_dir, &embedder)?;
            embed_fixture(&db, &embedder)?;
            docs.insert(fixture.clone(), d);
            d
        };

        let started = std::time::Instant::now();
        let query_vector = embedder.embed_query(&claim)?;
        let assembled = assemble(&db, doc, &claim, &query_vector, EVIDENCE_BUDGET_TOKENS)?;
        let bundle = match assembled {
            Assembled::NoEvidence { reason } => {
                let expected_none =
                    case.expected.get("outcome").and_then(|v| v.as_str()) == Some("noEvidence");
                println!(
                    "  {} {}  NoEvidence (no model run): {reason}",
                    if expected_none { "PASS" } else { "MISS" },
                    case.id
                );
                let (stop_reasons, truncated) = (Vec::new(), false);
                results.push(CaseResult {
                    id: case.id.clone(),
                    diagnostic_faithfulness: Vec::new(),
                    fatal_errors: Vec::new(),
                    stop_reasons,
                    truncated,
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
                    decode_tokens_per_sec: decode_tps(0, 0),
                    chunks_sent: Some(0),
                    chunks_dropped: Some(0),
                    evidence_words: Some(0),
                });
                continue;
            }
            Assembled::Ready(b) => b,
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let r = match variant {
            SupportVariant::V1 => {
                let task = CitationSupportTask {
                    claim: claim.clone(),
                    cited_source: source,
                    evidence: bundle.rendered.clone(),
                };
                run_task(&manager, &task, &bundle.ctx, cancel, None).await
            }
            SupportVariant::V2 => {
                let task = CitationSupportV2Task {
                    claim: claim.clone(),
                    cited_source: source,
                    evidence: bundle.rendered.clone(),
                };
                run_task(&manager, &task, &bundle.ctx, cancel, None).await
            }
            SupportVariant::V1ChunkBound => {
                let task = app_lib::ai::tasks::citation_support::CitationSupportV17Task {
                    claim: claim.clone(),
                    cited_source: source,
                    evidence: bundle.rendered.clone(),
                };
                run_task(&manager, &task, &bundle.ctx, cancel, None).await
            }
        };
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
                let faithfulness = case
                    .expected
                    .get("faithfulness")
                    .and_then(|spec| faithfulness_violation(&run.output, &bundle.ctx, spec));
                if let Some(word) = &faithfulness {
                    println!(
                        "  !! FAITHFULNESS_VIOLATION {} — cites the planted absent-finding chunk \
                         and asserts it with {word:?}\n     explanation: {}",
                        case.id, run.output.explanation
                    );
                }
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
                let (stop_reasons, truncated) = stops(&run.stop_reasons);
                CaseResult {
                    id: case.id.clone(),
                    diagnostic_faithfulness: Vec::new(),
                    fatal_errors: Vec::new(),
                    stop_reasons,
                    truncated,
                    outcome: Outcome::Ok,
                    raw: Some(serde_json::to_string(&run.output)?),
                    scored: Some(serde_json::json!({
                        "verdict": verdict_ok,
                        // The verdict GIVEN, not just whether it matched — a
                        // distribution over these is how a collapse (every case
                        // answered the same way) becomes visible at all.
                        "verdictGiven": got["verdict"],
                        // Item 8: reported explicitly for EVERY accepted result.
                        "citedPlantedChunk": cited_planted,
                        // D18/D25: flagged for human review, never auto-scored.
                        "faithfulnessViolation": faithfulness,
                        "quotesPresent": run.output.supporting_chunks.iter()
                            .filter(|sc| sc.quote.is_some()).count(),
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
                    decode_tokens_per_sec: decode_tps(run.timings.tokens, run.timings.decode_ms),
                    chunks_sent: Some(bundle.chunks_sent),
                    chunks_dropped: Some(bundle.chunks_dropped),
                    evidence_words: Some(bundle.words_estimated),
                }
            }
            Err(TaskError::ValidationFailed { errors, primary, first_raw, retry_raw, timings, stop_reasons }) => {
                println!(
                    "  FAIL {}  {:>5}ms  sent={} dropped={} fatal ({primary:?} kept): {}",
                    case.id,
                    elapsed_ms,
                    bundle.chunks_sent,
                    bundle.chunks_dropped,
                    errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
                );
                let (stop_reasons, truncated) = stops(&stop_reasons);
                // §11 D28 — DIAGNOSTIC ONLY. Runs after the outcome is already
                // ValidationFailed, on a parsed clone, and can neither clear a fatal
                // error nor reach persistence. It exists so a rejected output stops
                // taking its faithfulness evidence down with it.
                let primary_raw = match primary {
                    Attempt::First => first_raw.as_str(),
                    Attempt::Retry => retry_raw.as_str(),
                };
                let diagnostic_faithfulness = parse_for_diagnosis(primary_raw)
                    .map(|parsed| {
                        diagnostic_mismatches(&parsed, &bundle.ctx, case.expected.get("faithfulness"))
                    })
                    .unwrap_or_default();
                if !diagnostic_faithfulness.is_empty() {
                    for m in &diagnostic_faithfulness {
                        println!("       DIAGNOSTIC (rejected output): {m}");
                    }
                }
                CaseResult {
                    id: case.id.clone(),
                    diagnostic_faithfulness,
                    fatal_errors: errors.iter().map(|e| e.to_string()).collect(),
                    stop_reasons,
                    truncated,
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
                    decode_tokens_per_sec: decode_tps(timings.tokens, timings.decode_ms),
                    chunks_sent: Some(bundle.chunks_sent),
                    chunks_dropped: Some(bundle.chunks_dropped),
                    evidence_words: Some(bundle.words_estimated),
                }
            }
            Err(e) => {
                println!("  ERR  {}  {e}", case.id);
                let (stop_reasons, truncated) = (Vec::new(), false);
                CaseResult {
                    id: case.id.clone(),
                    diagnostic_faithfulness: Vec::new(),
                    fatal_errors: Vec::new(),
                    stop_reasons,
                    truncated,
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
                    decode_tokens_per_sec: decode_tps(0, 0),
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
        let faithfulness_hits: Vec<String> = results
        .iter()
        .filter(|r| {
            r.scored
                .as_ref()
                .and_then(|s| s.get("faithfulnessViolation"))
                .map(|v| !v.is_null())
                .unwrap_or(false)
        })
        .map(|r| r.id.clone())
        .collect();
    // §11 D28 — ACCEPTED faithfulness. The honest denominator is accepted
    // outputs that CARRIED a faithfulness spec, not seeds that had one: Phase 6
    // counted the latter and so reported "2 checked" in cells where the checker
    // had inspected nothing at all.
    let accepted_ids: std::collections::HashSet<&str> =
        results.iter().filter(|r| r.outcome == Outcome::Ok).map(|r| r.id.as_str()).collect();
    let faithfulness_checked = cases
        .iter()
        .filter(|c| c.expected.get("faithfulness").is_some() && accepted_ids.contains(c.id.as_str()))
        .count();

    // §11 D28 — DIAGNOSTIC faithfulness, on REJECTED outputs. Kept in its own
    // fields and out of every rate: it describes outputs the engine threw away.
    let diagnostic_cases: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| !r.diagnostic_faithfulness.is_empty())
        .map(|r| serde_json::json!({ "id": r.id, "outcome": r.outcome, "findings": r.diagnostic_faithfulness }))
        .collect();

    let load_end = load_avg_1m();
    let binary_hash = self_hash();
    // Collapse check: a model that answers everything the same way can score
    // respectably on agreement while having learned nothing.
    let verdict_distribution: std::collections::BTreeMap<String, usize> = {
        let mut d: std::collections::BTreeMap<String, usize> = Default::default();
        for r in results.iter().filter(|r| r.outcome == Outcome::Ok) {
            if let Some(v) =
                r.scored.as_ref().and_then(|s| s.get("verdictGiven")).and_then(|v| v.as_str())
            {
                *d.entry(v.to_string()).or_default() += 1;
            }
        }
        d
    };
    let mean_decode_tps = {
        let n = results.iter().filter(|r| r.decode_tokens_per_sec > 0.0).count();
        if n == 0 {
            0.0
        } else {
            results.iter().map(|r| r.decode_tokens_per_sec).sum::<f64>() / n as f64
        }
    };
    let prefill_share = {
        let lat: f64 = results.iter().map(|r| r.elapsed_ms as f64).sum();
        if lat == 0.0 {
            0.0
        } else {
            results.iter().map(|r| r.prefill_ms as f64).sum::<f64>() / lat
        }
    };

    let report = serde_json::json!({
        "task": "citation_support",
        "modelId": model_id,
        "promptVersion": variant.version(),
        "modelLoadMs": load_ms,
        "ramTotalMb": ram_mb,
        "nCtx": app_lib::ai::generative::TASK_N_CTX,
        "evidenceBudgetTokens": EVIDENCE_BUDGET_TOKENS,
        // §11 D29. A report can never again be read without knowing which
        // retrieval produced the evidence it scores.
        "embedder": embedder.model_id(),
        "preprocessingVersion": embedder.preprocessing_version(),
        "embedderIsReal": embedder.is_real(),
        "date": date,
        "casesFile": cases_path.display().to_string(),
        "totalCases": results.len(),
        "validOutputs": ok,
        "verdictAgreement": agree("verdict"),
        "citedPlantedChunk": agree("citedPlantedChunk"),
        "validationFailureRate": results.iter().filter(|r| r.outcome == Outcome::ValidationFailed).count() as f64 / total,
        "retryRate": results.iter().filter(|r| r.retried).count() as f64 / total,
        // §11 D27. The ceiling this cell ran at, and how often it was hit.
        // COUNTED, never corrected for: a truncated reply is still a failure.
        "maxTokensCeiling": ceiling,
        "truncationCount": results.iter().filter(|r| r.truncated).count(),
        "truncationCases": results.iter().filter(|r| r.truncated).map(|r| r.id.clone()).collect::<Vec<_>>(),
        // §11 D26. The defect the labelled header exists to remove. If this is
        // not zero on the repaired series, the rendering change did not work.
        "chunkIdFormatFailures": results.iter().filter(|r| r.fatal_errors.iter().any(|e| e.contains("chunk_id"))).count(),
        "chunkIdFormatCases": results.iter().filter(|r| r.fatal_errors.iter().any(|e| e.contains("chunk_id"))).map(|r| r.id.clone()).collect::<Vec<_>>(),
        "advisoryRate": results.iter().filter(|r| !r.advisories.is_empty()).count() as f64 / total,
        "meanLatencyMs": results.iter().map(|r| r.elapsed_ms as f64).sum::<f64>() / total,
        "meanPromptTokens": results.iter().map(|r| r.prompt_tokens as f64).sum::<f64>() / total,
        "meanPrefillMs": results.iter().map(|r| r.prefill_ms as f64).sum::<f64>() / total,
        "meanDecodeMs": results.iter().map(|r| r.decode_ms as f64).sum::<f64>() / total,
        // D18/D25. Counted, listed by case id, and NEVER folded into a score:
        // a faithfulness flag is a prompt for a human to read the verbatim
        // output, not a metric to optimise.
        "faithfulnessViolations": faithfulness_hits.len(),
        "faithfulnessViolationCases": faithfulness_hits,
        "faithfulnessCheckedCases": faithfulness_checked,
        // §11 D28. Reported SEPARATELY from the accepted figures above and
        // deliberately not a rate — these describe rejected outputs.
        "diagnosticFaithfulnessFailures": diagnostic_cases.len(),
        "diagnosticFaithfulnessCases": diagnostic_cases,
        // Which binary produced this cell. Never compare two cells whose
        // hashes differ.
        "binaryHash": binary_hash,
        // WHICH COMPILER built it, and WHICH OS ran it (§11 D61).
        //
        // A 2.65x CPU prefill regression against D34 had to be attributed
        // between the OS and the toolchain, and neither was in the report. The
        // toolchain was eventually excluded by inspecting `~/.rustup` directory
        // mtimes — circumstantial evidence that stops working the moment anyone
        // runs `rustup update`. Two fields make that a lookup instead.
        "rustc": env!("GAPLY_BUILD_RUSTC"),
        "os": os_version(),
        // §11 D33. Timing figures from cells with DIFFERENT load context are
        // not comparable — that is a rule, not a caveat. `ranIsolated` is a
        // DECLARATION by the operator, not a measurement: it says nothing else
        // was scheduled, and the load averages are the evidence for or against.
        "loadContext": {
            "loadAvg1mStart": load_start,
            "loadAvg1mEnd": load_end,
            "ranIsolated": ran_isolated,
        },
        "verdictDistribution": verdict_distribution,
        "meanDecodeTokensPerSec": mean_decode_tps,
        "prefillShare": prefill_share,
        "results": results,
    });

    println!();
    println!("valid outputs           : {ok}/{}", results.len());
    println!("verdict agreement       : {}", pct(agree("verdict")));
    println!("cited the PLANTED chunk : {}", pct(agree("citedPlantedChunk")));
    println!("validation failure rate : {:.0}%", report["validationFailureRate"].as_f64().unwrap_or(0.0) * 100.0);
    println!(
        "truncated at ceiling    : {} of {} (ceiling {ceiling})",
        report["truncationCount"], results.len()
    );
    println!("chunk_id format failures: {}", report["chunkIdFormatFailures"]);
    println!(
        "load 1m avg             : {} -> {}{}",
        load_start.map(|l| format!("{l:.2}")).unwrap_or_else(|| "?".into()),
        load_end.map(|l| format!("{l:.2}")).unwrap_or_else(|| "?".into()),
        if ran_isolated { "  (declared isolated)" } else { "  (NOT isolated — timings not comparable)" }
    );
    println!("retry rate              : {:.0}%", report["retryRate"].as_f64().unwrap_or(0.0) * 100.0);
    println!("advisory rate           : {:.0}%", report["advisoryRate"].as_f64().unwrap_or(0.0) * 100.0);
    println!("mean latency            : {:.0} ms", report["meanLatencyMs"].as_f64().unwrap_or(0.0));
    println!("mean prompt tokens      : {:.0}", report["meanPromptTokens"].as_f64().unwrap_or(0.0));
    println!("mean prefill            : {:.0} ms", report["meanPrefillMs"].as_f64().unwrap_or(0.0));
    println!("model load              : {load_ms} ms");
    println!(
        "FAITHFULNESS (accepted) : {} violations of {} accepted outputs checked",
        report["faithfulnessViolations"].as_u64().unwrap_or(0),
        faithfulness_checked
    );
    println!(
        "FAITHFULNESS (rejected) : {} diagnostic finding(s) on rejected outputs",
        report["diagnosticFaithfulnessFailures"].as_u64().unwrap_or(0)
    );

    std::fs::create_dir_all(&out_dir)?;
    let path = out_dir.join(format!("{}-{date}.json", variant.version()));
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
    println!("\nreport written to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use app_lib::ai::task::{AiTask, EvidenceChunk};
    use app_lib::ai::tasks::citation_support::CitationSupportTask;

    fn ctx() -> TaskContext {
        TaskContext::new(vec![
            EvidenceChunk {
                chunk_id: "c13".into(),
                page: Some(5),
                section: None,
                text: "Organic management was associated with higher soil invertebrate species \
                       richness in this paired comparison."
                    .into(),
            },
            EvidenceChunk {
                chunk_id: "c1".into(),
                page: Some(1),
                section: Some("Results".into()),
                text: "Species richness rose 31 percent under organic management.".into(),
            },
        ])
    }

    /// DIAGNOSTIC id recovery reads intent; it never becomes an acceptance rule.
    #[test]
    fn recover_chunk_id_reads_the_intent_of_every_composite_form() {
        assert_eq!(recover_chunk_id("c13"), "c13");
        assert_eq!(recover_chunk_id("c13 | p.5 | -"), "c13");
        assert_eq!(recover_chunk_id("[CHUNK_ID=c13 PAGE=5 SECTION=-]"), "c13");
        assert_eq!(recover_chunk_id("CHUNK_ID=c13"), "c13");
        assert_eq!(recover_chunk_id("  c13  "), "c13");
    }

    #[test]
    fn numbers_are_pulled_out_of_a_claim_element() {
        assert_eq!(numbers_in("by about 31 percent"), vec!["31".to_string()]);
        assert_eq!(numbers_in("no digits here"), Vec::<String>::new());
    }

    /// §11 D28, the case that motivated it. This is the 3B's real cs-seed-01
    /// first attempt under v2: a composite chunk_id (so the engine rejects it),
    /// `by about 31 percent` marked `found`, and a quote carrying no number.
    /// Phase 6 lost this entirely behind the chunk_id error.
    #[test]
    fn the_3b_cs_seed_01_shape_is_exposed_as_a_diagnostic_mismatch() {
        let raw = r#"{
          "verdict": "strong",
          "confidence": 1.0,
          "supporting_chunks": [{
            "chunk_id": "c13 | p.5 | -", "page": 5,
            "quote": "Organic management was associated with higher soil invertebrate species richness in this paired comparison.",
            "why": "Reports the richness increase."
          }],
          "claim_elements": [
            {"element": "Organic management", "status": "found"},
            {"element": "by about 31 percent", "status": "found"}
          ],
          "explanation": "The study found a 31 percent increase, which matches the claim exactly.",
          "suggested_rewrite": null
        }"#;
        let parsed = parse_for_diagnosis(raw).expect("schema-shaped enough to diagnose");
        let hits = diagnostic_mismatches(&parsed, &ctx(), None);
        assert!(
            hits.iter().any(|h| h.contains("31") && h.contains("marked found")),
            "the 31 percent mismatch was not surfaced: {hits:?}"
        );
    }

    /// THE boundary (§11 D28). A diagnostic finding is evidence for a human; it
    /// is not a second opinion the validator has to respect. The same output
    /// that produced findings above is STILL fatally invalid, and the two
    /// judgements are computed independently.
    #[test]
    fn a_diagnostic_finding_cannot_turn_a_rejected_output_into_an_accepted_one() {
        let raw = r#"{
          "verdict": "strong", "confidence": 1.0,
          "supporting_chunks": [{"chunk_id": "c13 | p.5 | -", "page": 5, "why": "x"}],
          "claim_elements": [{"element": "by about 31 percent", "status": "found"}],
          "explanation": "e", "suggested_rewrite": null
        }"#;
        let parsed = parse_for_diagnosis(raw).unwrap();

        // the diagnostic has something to say...
        assert!(!diagnostic_mismatches(&parsed, &ctx(), None).is_empty());

        // ...and the validator is entirely unmoved by it.
        let errors = CitationSupportTask::validate(&parsed, &ctx())
            .expect_err("a composite chunk_id must stay fatal");
        assert!(
            errors.iter().any(|e| e.field.contains("chunk_id") && e.is_fatal()),
            "the composite id stopped being fatal: {errors:?}"
        );
    }

    /// A truncated reply never became schema-shaped, so there is nothing to
    /// diagnose. Saying so beats reporting zero findings as if it were clean.
    #[test]
    fn a_truncated_reply_has_nothing_to_diagnose() {
        assert!(parse_for_diagnosis(r#"{"verdict":"strong","supporting_chunks":[{"chunk_i"#).is_none());
    }

    /// §11 D85. The metric must announce when it cannot measure anything.
    ///
    /// Both directions, because a warning that fires always is as useless as one
    /// that never fires — and the whole reason the metric was kept rather than
    /// deleted is that v3 DID grade this corpus.
    #[test]
    fn severity_agreement_is_marked_degenerate_only_when_it_has_no_variance() {
        let dist = |pairs: &[(&str, usize)]| {
            pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect::<std::collections::BTreeMap<String, usize>>()
        };

        // v4 on the 3B: `high` 37/37. No variance — the percentage is a
        // property of the labels, and saying so is the point.
        let note = degenerate_severity_note(&dist(&[("high", 37)])).expect("must be marked");
        assert!(note.contains("DEGENERATE"), "{note}");
        assert!(note.contains("\"high\""), "the note must name the value: {note}");
        assert!(note.contains("cannot measure judgement"), "{note}");

        // v3 on the SAME model and corpus graded almost evenly. Nothing to warn
        // about, and a warning here would train the reader to ignore it.
        assert_eq!(degenerate_severity_note(&dist(&[("high", 20), ("low", 23)])), None);
        assert_eq!(degenerate_severity_note(&dist(&[("high", 4), ("medium", 2)])), None);

        // No valid outputs at all is not a degenerate GRADE — it is a run with
        // nothing in it, which `validOutputs` already reports.
        assert_eq!(degenerate_severity_note(&dist(&[])), None);
    }
}
