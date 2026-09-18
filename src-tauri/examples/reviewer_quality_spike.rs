//! SPIKE — reviewer-quality evaluation prep, not production code, safe to delete after use.
//!
//! Purpose: dump the REAL escalation payload our pipeline builds for a given
//! manuscript, so it can be sent to a real LLM (OpenAI, by hand, separately) to
//! judge whether the model produces trustworthy grounded per-finding verdicts on
//! the ACTUAL structured shape — BEFORE any proxy/enclave deployment. Answers the
//! tracked "reviewer-quality spike (Q12)" question with real inputs.
//!
//! What it does (LOCAL ONLY — no network, no proxy, no API keys):
//!   1. Runs the EXISTING local pipeline (the same call run_publishready uses:
//!      extraction -> validation -> AI-detection -> plagiarism -> RAG ->
//!      verification -> compile_report), producing real EvidenceRecords.
//!   2. Routes them with the REAL DefaultRoutingPolicy (Box 1).
//!   3. Builds the escalation payload with the REAL build_escalation_payload
//!      (Box 2) — the actual production shape, not a hand-simulation.
//!   4. Prints a plain-English GROUND-TRUTH summary (every local finding, its
//!      severity/confidence/routing) to compare the LLM reply against.
//!   5. Prints the production-routed escalation payload + an ALL-FINDINGS payload
//!      (every finding, bypassing routing) as pretty JSON.
//!   6. OPTIONAL: if OPENAI_API_KEY is set, sends the ALL-FINDINGS payload to the
//!      OpenAI chat-completions API and prints the live AI reviewer response, so
//!      one command runs ground-truth -> payload -> AI verdict end to end. With
//!      NO key it just prints a note and skips the call (nothing breaks).
//!      OPENAI_MODEL overrides the default fast/cheap model (gpt-4o-mini).
//!      The OpenAI call uses a real, user-supplied key and spends real credits.
//!
//! Usage:
//!   cargo run --example reviewer_quality_spike -- <path-to-manuscript>
//!   OPENAI_API_KEY=sk-... cargo run --example reviewer_quality_spike -- paper.pdf
//!   OPENAI_API_KEY=sk-... OPENAI_MODEL=gpt-4o cargo run --example reviewer_quality_spike -- paper.pdf
//!
//! Defaults for a standalone spike (noted again at the end of output): in-memory
//! DB, HashEmbedder, NO journal target and NO supplementary files (escalation
//! uses neither). AI-detection uses the interim heuristic if the candle SLM-1
//! model is absent; verification degrades if Ollama is off — so which findings
//! reach escalation reflects those (possibly degraded) local confidences. Run it
//! on the machine where you normally run the desktop app for realistic signals.

use std::collections::HashSet;
use std::sync::Arc;

use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::escalation::build_escalation_payload;
use gaply_core::evidence::EvidenceRecord;
use gaply_core::orchestrator::{route_evidence, DefaultRoutingPolicy};
use gaply_core::{now_epoch, Database, GaplyError};

use app_lib::pipeline::{run_pipeline_measured, AnalysisEvent};

fn main() -> Result<(), GaplyError> {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("SPIKE usage: cargo run --example reviewer_quality_spike -- <manuscript-path>");
            std::process::exit(2);
        }
    };

    // 1) Local pipeline — EXACTLY the path run_publishready drives (6 lanes +
    //    compile_report). In-memory DB auto-migrates; HashEmbedder for RAG/plag.
    let db = Arc::new(Database::in_memory()?);
    let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
    let events = std::cell::RefCell::new(Vec::new());
    let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
    run_pipeline_measured(
        db.clone(), embedder, path.clone(), None, None, None,
        // A spike invoked by hand: the operator asked for the run.
        // no journal picker in this probe
        None,
        app_lib::pipeline::NetworkConsent::Granted,
        &emit,
    )?;

    let report_id = events
        .into_inner()
        .into_iter()
        .find_map(|e| match e {
            AnalysisEvent::Finished { report_id } => Some(report_id),
            _ => None,
        })
        .ok_or_else(|| GaplyError::Internal("pipeline produced no report".into()))?;

    let json = db
        .cache_get(&format!("report:{report_id}"), now_epoch())?
        .ok_or_else(|| GaplyError::Internal("compiled report not found".into()))?;
    let report: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| GaplyError::Internal(format!("parse report: {e}")))?;

    // 2) Real EvidenceRecords (Box 0 emits them 1:1 with findings, in report.evidence).
    let records: Vec<EvidenceRecord> =
        serde_json::from_value(report["evidence"].clone()).unwrap_or_default();

    // 3) Route with the REAL default policy, then build the REAL escalation
    //    payload for everything routed to escalation. NOTE: production batches
    //    these (critical solo, rest by agent) into SEPARATE calls
    //    (escalation::batch_escalation, private); the spike sends them together
    //    so you see all verdicts at once — the per-finding PAYLOAD SHAPE is
    //    identical (same build_escalation_payload).
    let policy = DefaultRoutingPolicy::default();
    let routed = route_evidence(&records, &policy);
    let escalate_ids: HashSet<&str> = routed.escalate.iter().map(String::as_str).collect();
    let batch: Vec<&EvidenceRecord> =
        records.iter().filter(|r| escalate_ids.contains(r.id.as_str())).collect();
    let (payload, sent) = build_escalation_payload(&batch, &routed.context_pool);

    // 4) GROUND TRUTH — the local pipeline's own view, to score the LLM against.
    println!("================ GROUND TRUTH (local pipeline) ================");
    println!("manuscript:          {path}");
    println!("report/run id:       {report_id}");
    println!("overall verdict:     {}", report["verdict"].as_str().unwrap_or("?"));
    println!("combined confidence: {}", report["combined_confidence"]);
    println!("findings detected:   {}", records.len());
    println!(
        "routed to escalation: {} (eligible-but-over-budget: {})",
        routed.escalate.len(),
        routed.omitted_count
    );
    println!("RAG context attachments: {}", routed.context_pool.len());
    println!();
    println!("--- all local findings (id | agent | severity | confidence | routing_hint) ---");
    if records.is_empty() {
        println!("  (none — pipeline produced no evidence records for this manuscript)");
    }
    for r in &records {
        let mark = if escalate_ids.contains(r.id.as_str()) { "  <== ESCALATED" } else { "" };
        println!(
            "  {:<5} | {:<16?} | {:<9?} | {:.3} | {:?}{}",
            r.id, r.agent, r.severity, r.confidence, r.routing_hint, mark
        );
    }
    // The report's human-readable finding titles (NOT in the payload — privacy),
    // printed here as ground truth so you know what each f{N} actually is.
    if let Some(findings) = report["findings"].as_array() {
        println!();
        println!("--- finding titles (ground-truth reference; NOT sent in the payload) ---");
        for (i, f) in findings.iter().enumerate() {
            println!("  f{:<3} | {}", i + 1, f["title"].as_str().unwrap_or("?"));
        }
    }
    println!();

    // 5) The ACTUAL escalation payload — copy this into the LLM.
    println!("================ ESCALATION PAYLOAD (send this to the LLM) ================");
    if batch.is_empty() {
        println!("(EMPTY: no findings were routed to escalation for this manuscript — either the");
        println!(" clean/control case, or every finding was accepted locally / held out. In");
        println!(" production nothing would be sent to the cloud. This is itself a valid result:");
        println!(" the local pipeline was confident enough not to need the LLM here.)");
        println!();
    }
    println!("sent finding_ids:  {:?}", sent.finding_ids);
    println!("sent evidence_refs: {:?}", sent.evidence_refs);
    println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    println!();

    // 6) ALL-FINDINGS dump — the SAME real build_escalation_payload builder, but
    //    fed EVERY local finding (not just the routed-uncertain subset), so you
    //    can test the LLM's per-finding verdict against your planted issues even
    //    when the pipeline was confident enough not to route them.
    let all: Vec<&EvidenceRecord> = records.iter().collect();
    let (all_payload, all_sent) = build_escalation_payload(&all, &routed.context_pool);
    println!("================ ALL-FINDINGS PAYLOAD (bypasses production routing — every local finding sent for adjudication, for reviewer-quality testing only; NOT what a real PublishReady run sends) ================");
    println!("sent finding_ids:  {:?}", all_sent.finding_ids);
    println!("{}", serde_json::to_string_pretty(&all_payload).unwrap());
    println!();

    // 7) OPTIONAL live AI reviewer call — sends the ALL-FINDINGS payload to OpenAI
    //    so one command shows ground-truth -> payload -> AI verdict. Skipped (with
    //    a note) when OPENAI_API_KEY is unset, so the dump-only path never breaks.
    println!("================ AI REVIEWER RESPONSE ================");
    match std::env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| DEFAULT_OPENAI_MODEL.to_string());
            eprintln!("Sending payload to OpenAI (model: {model}) — this will use API credits.");
            match call_openai(&key, &model, &all_payload) {
                Ok(reply) => println!("{reply}"),
                Err(e) => println!("(OpenAI call failed: {e})"),
            }
        }
        _ => {
            println!("(skipped — set OPENAI_API_KEY to also get the live AI response.");
            println!(" Optional: OPENAI_MODEL overrides the default {DEFAULT_OPENAI_MODEL}.)");
        }
    }

    eprintln!();
    eprintln!("NOTE (spike defaults): in-memory DB; HashEmbedder; NO journal target, NO");
    eprintln!("supplementary files (escalation uses neither). AI-detection uses the interim");
    eprintln!("heuristic if the candle SLM-1 model is absent; verification degrades if Ollama");
    eprintln!("is off — so which findings reached escalation reflects those local confidences.");
    eprintln!("The escalation payload carries STRUCTURED findings only (no manuscript text).");

    Ok(())
}

/// Fast/cheap default — this is a validation spike, not the production model
/// choice. Override with OPENAI_MODEL to try a stronger model on a paper.
const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";

/// The adjudication instruction prepended to the payload for the AI reviewer.
const OPENAI_INSTRUCTION: &str = "These findings have already been decided by a local analysis pipeline. \
Explain and adjudicate each one. Do NOT invent new findings. Cite only the finding ids provided in the payload.";

/// SPIKE — POST the ALL-FINDINGS payload to the OpenAI chat-completions API and
/// return the assistant's text. A real network call with a real user-supplied
/// key; throwaway, no proxy, no app coupling. Errors are returned as strings so
/// a failed call degrades to a printed note instead of aborting the run.
fn call_openai(api_key: &str, model: &str, payload: &serde_json::Value) -> Result<String, String> {
    let content = format!("{OPENAI_INSTRUCTION}\n\n{}", serde_json::to_string_pretty(payload).unwrap());
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": content }],
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let v: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {v}"));
    }
    Ok(v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no content in response)")
        .to_string())
}
