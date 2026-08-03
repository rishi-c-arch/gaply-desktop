//! **Release-gate instrument — TRACKED, not a debugging script.**
//!
//! Reads the compiled PublishReady report as an author would, so the
//! COMPOSITION of findings can be reviewed rather than their individual
//! correctness (ONTOLOGY §4.11).
//!
//! # Scope
//!
//! **ARCHITECTURE_TRACE §14 is the authoritative statement of what this covers**
//! — a stage-by-stage matrix of the production path with the risk of each
//! untested stage. Do not restate it here: an informal summary in a source file
//! becomes the version people read.
//!
//! In short: this is the FAST DETERMINISTIC harness. It drives
//! `run_pipeline_measured`, not the Tauri command path, and §14 records both the
//! three production defects it has caught and the stages it structurally cannot
//! reach. `release_gate.rs` (§14.1) is the complementary instrument; neither
//! replaces the other.
//!
//! Usage:
//!   read_report <manuscript> [selected-guidelines-url] [other-guidelines-url…]
//!
//! Extra URLs are ingested BEFORE the selected one, into the same persistent
//! corpus — the setup required to verify that a checklist reflects only the
//! journal the user chose.
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::{now_epoch, Database, GaplyError};
use std::sync::Arc;
use app_lib::pipeline::{run_pipeline_measured, AnalysisEvent};

fn main() -> Result<(), GaplyError> {
    let path = std::env::args().nth(1).expect("usage: read_report <manuscript>");
    let db = Arc::new(Database::in_memory()?);
    let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
    // Optional 2nd arg: guidelines URL. The production path ingests these BEFORE
    // the pipeline (PublishReadyPage.tsx:152); run_pipeline_measured does not.
    // arg2 = the journal the user SELECTED. arg3.. = other journals ingested
    // earlier into the same persistent corpus (the contamination setup).
    let guidelines = std::env::args().nth(2);
    let others: Vec<String> = std::env::args().skip(3).collect();
    if guidelines.is_some() || !others.is_empty() {
        let ing = app_lib::guidelines::GuidelinesIngestor::new()?;
        for url in others.iter() {
            let rep = ing.ingest(&db, embedder.as_ref(), None, Some(url));
            println!("[guidelines OTHER] {url} -> {} ({})", rep.any_ingested, rep.note);
        }
        if let Some(url) = guidelines.clone() {
            let rep = ing.ingest(&db, embedder.as_ref(), None, Some(&url));
            println!("[guidelines SELECTED] {url} -> {} ({})", rep.any_ingested, rep.note);
        }
    }
    let events = std::cell::RefCell::new(Vec::new());
    let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
    run_pipeline_measured(db.clone(), embedder, path.clone(), None, None, guidelines.clone(), &emit)?;
    let evs = events.into_inner();
    for e in &evs {
        match e {
            AnalysisEvent::StageCompleted { stage, summary, .. } => println!("[lane] {stage}: {summary}"),
            AnalysisEvent::Failed { stage, message } => println!("[lane FAILED] {stage}: {message}"),
            _ => {}
        }
    }
    let report_id = evs.into_iter().find_map(|e| match e {
        AnalysisEvent::Finished { report_id } => Some(report_id),
        _ => None,
    }).ok_or_else(|| GaplyError::Internal("no report".into()))?;
    let json = db.cache_get(&format!("report:v2:{report_id}"), now_epoch())?.expect("report cached");
    let r: serde_json::Value = serde_json::from_str(&json).unwrap();

    println!("\n================ REPORT ================");
    let findings = r["findings"].as_array().cloned().unwrap_or_default();
    println!("FINDINGS: {} total\n", findings.len());
    for (i, f) in findings.iter().enumerate() {
        println!("{:>2}. [{}] [{}] {}", i + 1,
            f["severity"].as_str().unwrap_or("?"),
            f["certainty_label"].as_str().unwrap_or("?"),
            f["title"].as_str().unwrap_or("?"));
        println!("    detail: {}", f["detail"].as_str().unwrap_or(""));
        let p = f["provenance"].as_array().cloned().unwrap_or_default();
        let first = p.first().and_then(|v| v.as_str()).unwrap_or("");
        println!("    provenance: {first}{}", if p.len() > 1 { format!(" (+{})", p.len()-1) } else { String::new() });
    }
    println!("\nCHECKLIST:");
    for c in r["checklist"].as_array().cloned().unwrap_or_default() {
        println!("  [{}] {} — {}", if c["passed"].as_bool().unwrap_or(false) { "PASS" } else { "FAIL" }, c["requirement"].as_str().unwrap_or("?"), c["detail"].as_str().unwrap_or(""));
        if let Some(src) = c["guideline_source"].as_str() { println!("        source: {src}"); }
    }
    println!("\nDEBATE: {}", serde_json::to_string(&r["debate"]).unwrap_or_default().chars().take(300).collect::<String>());
    println!("\nDISCLAIMER: {}", r["disclaimer"].as_str().unwrap_or("(none)"));
    println!("\nEVIDENCE RECORDS: {}", r["evidence"].as_array().map(|a| a.len()).unwrap_or(0));
    Ok(())
}
