//! **Release-gate instrument — TRACKED, not a debugging script.**
//!
//! Reads the compiled PublishReady report as an author would, so the
//! COMPOSITION of findings can be reviewed rather than their individual
//! correctness (ONTOLOGY §4.11).
//!
//! # Why this is tracked
//!
//! It has found three production defects that the full test suite did not, with
//! every component metric green:
//!
//! * `registry_year_mismatch` producing five false accusations — an entity
//!   resolution failure invisible to component tests;
//! * a self-match regression from a preprocessing change (6 -> 10 spans) that
//!   passed 683 tests;
//! * 28 of 48 findings being identical restatements — a presentation failure
//!   with no incorrect component anywhere.
//!
//! A file `git clean` removes is not part of a release process.
//!
//! # What it does NOT cover
//!
//! This is the FAST DETERMINISTIC harness: it drives `run_pipeline_measured`,
//! not the Tauri command path. It does not exercise the reviewer payload, shadow
//! synthesis, escalation, or UI aggregation, and it only exercises guideline
//! ingestion because that was wired in explicitly after a gap audit drew a wrong
//! conclusion from its absence. A second instrument covering the full command
//! path is proposed separately — the two have different purposes and neither
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
