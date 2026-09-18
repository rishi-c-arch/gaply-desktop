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
        db.clone(), embedder.clone(), path.clone(), None, None, guidelines.clone(),
        // A pre-ship instrument run deliberately: the operator asked for the run.
        // no journal picker in this probe
        None,
        app_lib::pipeline::NetworkConsent::Granted,
        &emit,
    )?;
    let report_id = events.into_inner().into_iter().find_map(|e| match e {
        app_lib::pipeline::AnalysisEvent::Finished { report_id } => Some(report_id),
        _ => None,
    }).ok_or_else(|| GaplyError::Internal("no report".into()))?;

    // Was `format!("report:v2:{report_id}")` — which drifted when the evidence
    // schema version entered the key, so this lookup returned None and the
    // release gate panicked. Use the one definition.
    let json = db
        .cache_get(&app_lib::pipeline::report_cache_key(&report_id), now_epoch())?
        .expect("report");
    let report: serde_json::Value = serde_json::from_str(&json).unwrap();
    // `build_review_payload` is typed since §26 PR-3; the JSON copy above is
    // still used below to read `detail` strings the payload must NOT contain.
    let report_typed: gaply_core::report::PublishReadyReport =
        serde_json::from_str(&json).expect("compiled report parses");

    // The REAL payload builder — the stage read_report.rs never reaches.
    let journal = gaply_core::reviewer_agent::TargetJournal {
        name: "PLOS ONE".into(), quartile: "Q1".into(),
    };
    let (payload, _sent) =
        gaply_core::reviewer_agent::build_review_payload(&report_typed, &journal, &[], &report_id);

    // Every `detail` from the compiled report — the strings that must NOT cross.
    let details: Vec<String> = report["findings"].as_array().cloned().unwrap_or_default()
        .iter().filter_map(|f| f["detail"].as_str().map(str::to_string)).collect();

    // COMPARISON and PERSISTENCE need the FULL command path, which produces the
    // Box 4 comparison record. Before §32 this runner passed literal `None`, so
    // both could only ever report SKIPPED and `ship_ready` was false by
    // construction. `run_publishready_measured` is the seam that makes the
    // command path reachable outside Tauri; `harness_log::set_dir` is the second
    // half, because `append` is a silent no-op with no directory configured.
    //
    // NO PROXY IS REQUIRED. The reviewer degrades to unavailable_offline and the
    // harness block is unconditional; both findings_sent counts are
    // DeterministicLocal, so they are Observed offline.
    let sink = std::env::temp_dir().join(format!("gaply_gate_{}", std::process::id()));
    std::fs::create_dir_all(&sink).expect("create comparison sink dir");
    app_lib::harness_log::set_dir(sink.clone());
    let full_path_err = app_lib::commands::run_publishready_measured(
        db.clone(),
        embedder.clone(),
        path.clone(),
        "PLOS ONE".to_string(),
        "Q1".to_string(),
        None,
        None,
        guidelines.clone(),
        // No journal picker in the release gate.
        None,
    )
    .err();
    if let Some(e) = &full_path_err {
        println!("[full-path] run_publishready_measured failed: {e}");
    }
    let sink_file = sink.join("box4_comparisons.jsonl");
    let lines: Option<Vec<String>> = std::fs::read_to_string(&sink_file)
        .ok()
        .map(|c| c.lines().map(str::to_string).collect());
    let record: Option<serde_json::Value> = lines
        .as_ref()
        .and_then(|l| l.iter().rev().find(|s| !s.trim().is_empty()))
        .and_then(|s| serde_json::from_str(s).ok());

    // The FREE route's enumeration, shared with the all-unavailable fixture.
    // This runner builds a free-tier payload (`build_review_payload` sends
    // structured summaries only), so the free gate is the one that applies; the
    // premium gate has no runner until Phase 1 gives it a payload to run on.
    let gate = run_free_gate(&payload, &details, record.as_ref(), lines.as_deref());

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
    // TWO VALUES, not one. "Did anything fail?" and "was everything checked?"
    // are independent questions and one boolean cannot answer both (§32).
    println!("no_failures: {}", gate.no_failures());
    println!("coverage:    {}", gate.coverage());
    let _ = std::fs::remove_dir_all(&sink);
    Ok(())
}
