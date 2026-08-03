//! **Pre-ship release gate runner.** See `app_lib::release_gate` for the
//! invariants and ARCHITECTURE_TRACE §14 for why two instruments exist.
//!
//! Usage: release_gate <manuscript> [guidelines-url]
use app_lib::release_gate::*;
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::{now_epoch, Database, GaplyError};
use std::sync::Arc;

fn main() -> Result<(), GaplyError> {
    let path = std::env::args().nth(1).expect("usage: release_gate <manuscript> [guidelines-url]");
    let guidelines = std::env::args().nth(2);
    let db = Arc::new(Database::in_memory()?);
    let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);

    if let Some(u) = guidelines.clone() {
        let ing = app_lib::guidelines::GuidelinesIngestor::new()?;
        let r = ing.ingest(&db, embedder.as_ref(), None, Some(&u));
        println!("[guidelines] {} ({})", r.any_ingested, r.note);
    }

    let events = std::cell::RefCell::new(Vec::new());
    let emit = |e: app_lib::pipeline::AnalysisEvent| events.borrow_mut().push(e);
    app_lib::pipeline::run_pipeline_measured(
        db.clone(), embedder, path, None, None, guidelines, &emit,
    )?;
    let report_id = events.into_inner().into_iter().find_map(|e| match e {
        app_lib::pipeline::AnalysisEvent::Finished { report_id } => Some(report_id),
        _ => None,
    }).ok_or_else(|| GaplyError::Internal("no report".into()))?;

    let json = db.cache_get(&format!("report:v2:{report_id}"), now_epoch())?.expect("report");
    let report: serde_json::Value = serde_json::from_str(&json).unwrap();

    // The REAL payload builder — the stage read_report.rs never reaches.
    let journal = gaply_core::reviewer_agent::TargetJournal {
        name: "PLOS ONE".into(), quartile: "Q1".into(),
    };
    let (payload, _sent) =
        gaply_core::reviewer_agent::build_review_payload(&report, &journal, &[], &report_id);

    // Every `detail` from the compiled report — the strings that must NOT cross.
    let details: Vec<String> = report["findings"].as_array().cloned().unwrap_or_default()
        .iter().filter_map(|f| f["detail"].as_str().map(str::to_string)).collect();

    // Proxy availability decides which invariants can run at all.
    let proxy_available = app_lib::models::proxy_client::ProxyReqwestClient::from_env().is_ok();

    let mut gate = GateReport::default();
    gate.record(PRIVACY, check_privacy(&payload, &details));
    gate.record(PROVENANCE, check_provenance(&payload));
    gate.record(SELECTION, check_selection(&payload));
    gate.record(COMPARISON, check_comparison(None));
    gate.record(PERSISTENCE, check_persistence(None));
    let liveness = check_liveness(&gate, proxy_available);
    gate.record(LIVENESS, liveness);

    println!("\n=== RELEASE GATE ===");
    println!("report findings: {}  payload findings: {}  detail strings checked: {}",
        report["findings"].as_array().map(|a| a.len()).unwrap_or(0),
        payload["summary"]["findings"].as_array().map(|a| a.len()).unwrap_or(0),
        details.len());
    for (name, o) in gate.results() {
        match o {
            GateOutcome::Pass => println!("  PASS     {name}"),
            GateOutcome::Fail { detail, .. } => println!("  FAIL     {name} — {detail}"),
            GateOutcome::Skipped { reason } => println!("  SKIPPED  {name} — {reason}"),
        }
    }
    println!("\n{}", gate.summary());
    println!("ship_ready: {}", gate.ship_ready());
    Ok(())
}
