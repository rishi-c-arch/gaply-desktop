//! Set-9 8GB memory-safety proof of the FULL PublishReady flow — MEASUREMENT
//! ONLY, no product behavior. Extends the Set-4 `mem_probe` pattern to the
//! complete `run_publishready` sequence:
//!
//!   guidelines ingest (real URL fixture, tiny local HTTP server)
//!   → 6-lane pipeline (stage 3 candle SLM-1 in-process; stage 6 Ollama SLM-2
//!     in a separate process; report synthesis)
//!   → supplementary parsing (the NEW memory element: the real sample.xlsx
//!     fixture + a worst-case ~9.5MB CSV just under the 10MB cap)
//!   → reviewer payload build + honest offline reviewer (proxy not deployed)
//!
//! run TWICE back-to-back (the proven OOM scenario). A background sampler
//! records every 250ms: this process's RSS (candle lives here), Ollama's
//! resident model bytes (qwen3 lives there), and the current flow stage — so
//! we can answer, with numbers: per-stage peaks, candle-released-before-
//! stage-6, no candle+qwen3 coexistence, WHERE supplementary parsing lands
//! relative to the model stages, qwen3 unloaded between runs, and overall
//! peak vs the 8192MB physical ceiling.
//!
//!   cargo run --release --example publishready_mem_probe
//! (Ollama must be running with qwen3:4b; SLM-1 GGUF at ~/gaply-models/slm1.)

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::reviewer_agent::{self, ReviewerEvaluation, TargetJournal};
use gaply_core::Database;

use app_lib::pipeline::{run_pipeline_measured, AnalysisEvent};
use app_lib::supplementary::parse_supplementary;

/// The real extraction fixture (has an IMRaD body + a References section with
/// DOIs, so stage 3 loads candle SLM-1 and stage 6 exercises SLM-2).
const MANUSCRIPT_FIXTURE: &str = "gaply-core/tests/fixtures/manuscript.txt";
/// The real supplementary xlsx fixture (Set 5).
const XLSX_FIXTURE: &str = "tests/fixtures/sample.xlsx";

const GUIDELINES_HTML: &str = "<html><head><title>Journal of Rest — Author Guidelines</title></head>\
<body><h1>Author Guidelines</h1><p>Manuscripts must not exceed 3000 words. A structured abstract \
is required. Methods must pre-specify the statistical tests and report effect sizes with 95% \
confidence intervals. References use the Vancouver style. Declare all conflicts of interest and \
data availability.</p></body></html>";

fn own_rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid]).output();
    out.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

fn ollama_resident_mb() -> f64 {
    let out = Command::new("curl")
        .args(["-s", "--max-time", "3", "http://127.0.0.1:11434/api/ps"])
        .output();
    let body = match out.ok().and_then(|o| String::from_utf8(o.stdout).ok()) {
        Some(b) => b,
        None => return 0.0,
    };
    let v: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return 0.0,
    };
    v["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|m| m["size_vram"].as_f64().or_else(|| m["size"].as_f64()).unwrap_or(0.0))
                .sum::<f64>()
                / (1024.0 * 1024.0)
        })
        .unwrap_or(0.0)
}

/// Tiny local HTTP fixture server: serves the guidelines HTML on every GET.
/// Returns the bound URL; runs until the process exits (daemon thread).
fn start_guidelines_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf); // drain the request line + headers
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                GUIDELINES_HTML.len(),
                GUIDELINES_HTML
            );
            let _ = s.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}/author-guidelines")
}

/// Worst-case PARSEABLE supplementary CSV. The caps proved themselves during
/// this probe's development: a >5000-row file is REJECTED (row cap), and a
/// dense 4.8MB file is REJECTED (2MB total-extracted-chars cap) — neither is
/// ever resident. The maximal input that PARSES pushes both remaining bounds
/// at once: a ~9.9MB file (just under the 10MB cap) of huge cells that CLAMP
/// to 512 chars, keeping ~1.95MB (just under the 2MB kept cap). CSV parsing
/// is streaming, so the raw file is never fully resident either.
fn write_big_csv() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("gaply_set9_big_supp.csv");
    let mut f = std::fs::File::create(&path).expect("create big csv");
    writeln!(f, "note").unwrap();
    let huge = "x".repeat(2600); // clamps to 512 kept
    for _ in 0..3800 {
        writeln!(f, "{huge}").unwrap();
    }
    path
}

#[derive(Clone)]
struct Sample {
    t_ms: u128,
    run: u64,
    stage: String,
    app_rss: f64,
    ollama: f64,
}

fn main() {
    let start = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let run = Arc::new(AtomicU64::new(0));
    let stage = Arc::new(Mutex::new(String::from("startup")));
    let samples = Arc::new(Mutex::new(Vec::<Sample>::new()));
    let set_stage = |st: &Arc<Mutex<String>>, s: &str| *st.lock().unwrap() = s.to_string();

    let sampler = {
        let (stop, run, stage, samples) =
            (stop.clone(), run.clone(), stage.clone(), samples.clone());
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let s = Sample {
                    t_ms: start.elapsed().as_millis(),
                    run: run.load(Ordering::Relaxed),
                    stage: stage.lock().unwrap().clone(),
                    app_rss: own_rss_mb(),
                    ollama: ollama_resident_mb(),
                };
                samples.lock().unwrap().push(s);
                thread::sleep(Duration::from_millis(250));
            }
        })
    };

    let db = Arc::new(Database::in_memory().expect("in-memory db"));
    let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);

    // Resolve fixtures relative to src-tauri (cargo runs examples from there).
    let manuscript = std::fs::canonicalize(MANUSCRIPT_FIXTURE).expect("manuscript fixture");
    let xlsx = std::fs::canonicalize(XLSX_FIXTURE).expect("xlsx fixture");
    let big_csv = write_big_csv();
    println!(
        "fixtures: manuscript={} xlsx={}B big_csv={}B",
        manuscript.display(),
        std::fs::metadata(&xlsx).unwrap().len(),
        std::fs::metadata(&big_csv).unwrap().len()
    );

    // ---- guidelines ingest (once, like the UI's journal-pick step) --------
    set_stage(&stage, "guidelines_ingest");
    let url = start_guidelines_fixture();
    let ingestor = app_lib::guidelines::GuidelinesIngestor::new().expect("ingestor");
    let report = ingestor.ingest(&db, embedder.as_ref(), None, Some(&url));
    println!(
        "guidelines ingest: {} result(s), ingested={}, app_rss={:.0}MB",
        report.results.len(),
        report.any_ingested,
        own_rss_mb()
    );

    let rss_at_stage6_start = Arc::new(Mutex::new(Vec::<(u64, f64, f64)>::new()));

    let run_once = |run_no: u64| {
        run.store(run_no, Ordering::Relaxed);
        let stage_for_emit = stage.clone();
        let rss_at6 = rss_at_stage6_start.clone();
        let report_id = Arc::new(Mutex::new(String::new()));
        let report_id_emit = report_id.clone();
        let emit = move |e: AnalysisEvent| match &e {
            AnalysisEvent::StageStarted { stage: s, .. } => {
                *stage_for_emit.lock().unwrap() = s.clone();
                if s == "verification" {
                    let (a, o) = (own_rss_mb(), ollama_resident_mb());
                    rss_at6.lock().unwrap().push((run_no, a, o));
                    println!("  [run {run_no}] stage 6 START — app_rss={a:.0}MB ollama={o:.0}MB");
                }
            }
            AnalysisEvent::StageCompleted { stage: s, .. } => {
                if s == "ai" {
                    println!("  [run {run_no}] stage 3 (candle) COMPLETE — app_rss={:.0}MB", own_rss_mb());
                }
                if s == "verification" {
                    *stage_for_emit.lock().unwrap() = "report_synthesis".into();
                }
            }
            AnalysisEvent::Finished { report_id: id } => {
                *report_id_emit.lock().unwrap() = id.clone();
            }
            _ => {}
        };

        println!("=== RUN {run_no} start (t+{}ms) ===", start.elapsed().as_millis());
        run_pipeline_measured(
            db.clone(),
            embedder.clone(),
            manuscript.to_string_lossy().to_string(),
            Some(format!("set9 publishready run {run_no}")),
            &emit,
        )
        .expect("pipeline run");

        // ---- run_publishready's post-pipeline steps, same order ----------
        let id = report_id.lock().unwrap().clone();
        let report_json: serde_json::Value = serde_json::from_str(
            &db.cache_get(&format!("report:{id}"), gaply_core::now_epoch())
                .expect("cache read")
                .expect("compiled report present"),
        )
        .expect("report parses");

        set_stage(&stage, "supplementary_xlsx");
        let ev_xlsx = parse_supplementary(&xlsx).expect("xlsx parses");
        println!("  [run {run_no}] xlsx parsed — app_rss={:.0}MB", own_rss_mb());

        set_stage(&stage, "supplementary_bigcsv");
        let ev_csv = parse_supplementary(&big_csv).expect("big csv parses");
        println!("  [run {run_no}] 9.5MB csv parsed — app_rss={:.0}MB", own_rss_mb());

        set_stage(&stage, "reviewer_assembly");
        let journal = TargetJournal { name: "Journal of Rest".into(), quartile: "Q1".into() };
        let supp_values = vec![
            serde_json::to_value(&ev_xlsx).unwrap(),
            serde_json::to_value(&ev_csv).unwrap(),
        ];
        let (payload, _sent) = reviewer_agent::build_review_payload(&report_json, &journal, &supp_values);
        // Proxy not deployed → the reviewer is the honest offline letter.
        let reviewer = ReviewerEvaluation::unavailable_offline();
        println!(
            "  [run {run_no}] reviewer payload built ({}B wire, reviewer.available={}) — app_rss={:.0}MB",
            serde_json::to_string(&payload).map(|s| s.len()).unwrap_or(0),
            reviewer.available,
            own_rss_mb()
        );

        set_stage(&stage, &format!("between-runs-{run_no}"));
        println!("=== RUN {run_no} OK (t+{}ms) ===", start.elapsed().as_millis());
    };

    run_once(1);
    let ollama_between = ollama_resident_mb();
    println!("\n>>> BETWEEN RUNS: ollama resident = {ollama_between:.0}MB (0 = keep_alive:0 + explicit unload held)\n");
    run_once(2);

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let _ = std::fs::remove_file(&big_csv);

    // ------------------------------ analysis -------------------------------
    let all = samples.lock().unwrap().clone();
    let peak = |pred: &dyn Fn(&Sample) -> bool, field: &dyn Fn(&Sample) -> f64| -> f64 {
        all.iter().filter(|s| pred(s)).map(field).fold(0.0, f64::max)
    };
    let stages = [
        "guidelines_ingest",
        "extraction",
        "validation",
        "ai",
        "plagiarism",
        "rag",
        "verification",
        "report_synthesis",
        "supplementary_xlsx",
        "supplementary_bigcsv",
        "reviewer_assembly",
    ];

    println!("\n========== SET 9 MEMORY REPORT ({} samples) ==========", all.len());
    println!("{:<24}{:>14}{:>16}{:>14}", "stage", "peak app RSS", "peak ollama", "peak sum");
    for st in stages {
        let a = peak(&|s| s.stage == st, &|s| s.app_rss);
        let o = peak(&|s| s.stage == st, &|s| s.ollama);
        let sum = all
            .iter()
            .filter(|s| s.stage == st)
            .map(|s| s.app_rss + s.ollama)
            .fold(0.0, f64::max);
        println!("{st:<24}{a:>11.0} MB{o:>13.0} MB{sum:>11.0} MB");
    }
    for (r, a, o) in rss_at_stage6_start.lock().unwrap().iter() {
        println!("stage-6 START (run {r}): app_rss={a:.0}MB (candle released?) ollama={o:.0}MB");
    }
    let overall_app = peak(&|_| true, &|s| s.app_rss);
    let overall_ollama = peak(&|_| true, &|s| s.ollama);
    let peak_sum = all.iter().map(|s| s.app_rss + s.ollama).fold(0.0, f64::max);
    let overlap: Vec<&Sample> =
        all.iter().filter(|s| s.app_rss > 1500.0 && s.ollama > 200.0).collect();
    println!("overall peak app RSS                 : {overall_app:.0} MB");
    println!("overall peak ollama resident         : {overall_ollama:.0} MB");
    println!("peak COEXIST (app+ollama, same t)    : {peak_sum:.0} MB");
    println!("ollama resident between run1→run2    : {ollama_between:.0} MB");
    println!("coexistence samples (candle>1500 AND qwen3>200): {}", overlap.len());
    if let Some(w) = overlap
        .iter()
        .max_by(|a, b| (a.app_rss + a.ollama).partial_cmp(&(b.app_rss + b.ollama)).unwrap())
    {
        println!(
            "  worst: t+{}ms run{} stage={} app={:.0} ollama={:.0} sum={:.0}",
            w.t_ms, w.run, w.stage, w.app_rss, w.ollama, w.app_rss + w.ollama
        );
    }
    println!("physical RAM                          : 8192 MB");
    println!("margin (8192 - peak sum)              : {:.0} MB", 8192.0 - peak_sum);
    println!("=======================================================");
}
