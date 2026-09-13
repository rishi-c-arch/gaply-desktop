//! Set-4 8GB memory-safety proof — MEASUREMENT ONLY, no product behavior.
//!
//! Drives the REAL pipeline (via `pipeline::run_pipeline_measured`) twice
//! back-to-back on a small manuscript that HAS a References section, so stage 3
//! loads candle SLM-1 (in-process RSS) and stage 6 loads Ollama SLM-2 (separate
//! process). A background sampler records, every 250ms:
//!   - this process's RSS (candle) via `ps`
//!   - Ollama's resident model size via `GET /api/ps` (0 when unloaded)
//!   - the current pipeline stage
//! so we can answer: does candle RSS release before stage 6, do the two models
//! ever coexist, does `keep_alive:0` unload qwen3:4b between runs, overall peak.
//!
//!   cargo run --release --example mem_probe
//! (Ollama must be running with qwen3:4b; SLM-1 GGUF present at ~/gaply-models.)

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::Database;

use app_lib::pipeline::{run_pipeline_measured, AnalysisEvent};

const MANUSCRIPT: &str = "\
Title: Sleep Duration and Memory Consolidation

Abstract
We tested whether a night of sleep improves consolidation of learned material.

Introduction
Sleep is believed to support consolidation of declarative memory (Watson & Crick, 1953).

Methods
We recruited 48 adults and compared recall with a paired t-test.

Results
The sleep group recalled more items (t(47) = 3.2, p = 0.002, d = 0.46).

Discussion
The findings are consistent with an active-consolidation account.

Conclusion
A night of sleep improved memory consolidation in this sample.

References
Watson, J.D., & Crick, F.H. (1953). Molecular Structure of Nucleic Acids. Nature, 171, 737-738. https://doi.org/10.1038/171737a0
";

/// This process's resident set size in MB (candle lives here).
fn own_rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid]).output();
    out.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

/// Ollama's resident model size in MB (qwen3:4b lives in a SEPARATE process);
/// 0 when nothing is loaded. Read from `GET /api/ps` `size_vram`.
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
    let run = Arc::new(AtomicU64::new(1));
    let stage = Arc::new(Mutex::new(String::from("startup")));
    let samples = Arc::new(Mutex::new(Vec::<Sample>::new()));

    // Background sampler — 250ms cadence.
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
    let path = std::env::temp_dir().join(format!("gaply_mem_probe_{}.txt", std::process::id()));
    std::fs::write(&path, MANUSCRIPT).expect("write manuscript");

    // Records the app RSS at the instant verification (stage 6) STARTS — the
    // "candle released?" number the task asks for.
    let rss_before_stage6 = Arc::new(Mutex::new(Vec::<(u64, f64)>::new()));

    let mut run_once = |run_no: u64| {
        run.store(run_no, Ordering::Relaxed);
        let stage_for_emit = stage.clone();
        let stage_post = stage.clone();
        let rss_before_stage6 = rss_before_stage6.clone();
        let emit = move |e: AnalysisEvent| {
            let stage = &stage_for_emit;
            if let AnalysisEvent::StageStarted { stage: s, .. } = &e {
                *stage.lock().unwrap() = s.clone();
                if s == "verification" {
                    rss_before_stage6.lock().unwrap().push((run_no, own_rss_mb()));
                    println!(
                        "  [run {run_no}] stage 6 (verification) START — app_rss={:.0}MB ollama={:.0}MB",
                        own_rss_mb(),
                        ollama_resident_mb()
                    );
                }
            }
            if let AnalysisEvent::StageCompleted { stage: s, .. } = &e {
                if s == "ai" {
                    println!(
                        "  [run {run_no}] stage 3 (ai/candle) COMPLETE — app_rss={:.0}MB",
                        own_rss_mb()
                    );
                }
            }
        };
        println!("=== RUN {run_no} start (t+{}ms) ===", start.elapsed().as_millis());
        let res = run_pipeline_measured(
            db.clone(),
            embedder.clone(),
            path.to_string_lossy().to_string(),
            Some(format!("mem-probe run {run_no}")),
            None, // measurement probe: unauthenticated, so no cloud verify tier
            None,
            // A memory probe invoked by hand: the operator asked for the run.
            app_lib::pipeline::NetworkConsent::Granted,
            &emit,
        );
        *stage_post.lock().unwrap() = format!("between-runs-{run_no}");
        match res {
            Ok(_) => println!("=== RUN {run_no} OK (t+{}ms) ===", start.elapsed().as_millis()),
            Err(e) => println!("=== RUN {run_no} ERROR: {e} ==="),
        }
    };

    run_once(1);
    // Immediately snapshot Ollama: did keep_alive:0 unload qwen3:4b after run 1?
    let ollama_after_run1 = ollama_resident_mb();
    let ps_after_run1 = Command::new("curl")
        .args(["-s", "--max-time", "3", "http://127.0.0.1:11434/api/ps"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    println!(
        "\n>>> BETWEEN RUNS: ollama resident = {:.0}MB immediately after run 1's verification",
        ollama_after_run1
    );
    println!(">>> /api/ps: {}\n", ps_after_run1.trim());

    run_once(2);

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let _ = std::fs::remove_file(&path);

    // ---- analysis ----
    let all = samples.lock().unwrap().clone();
    let stat = |pred: &dyn Fn(&Sample) -> bool, field: &dyn Fn(&Sample) -> f64| -> f64 {
        all.iter().filter(|s| pred(s)).map(field).fold(0.0, f64::max)
    };
    let peak_ai_app = stat(&|s| s.stage == "ai", &|s| s.app_rss);
    let peak_verif_ollama = stat(&|s| s.stage == "verification", &|s| s.ollama);
    let overall_app = stat(&|_| true, &|s| s.app_rss);
    let overall_ollama = stat(&|_| true, &|s| s.ollama);
    let peak_coexist = all.iter().map(|s| s.app_rss + s.ollama).fold(0.0, f64::max);
    // "Overlap" = candle clearly loaded (>1500MB) AND qwen3 resident, same instant.
    let overlap: Vec<&Sample> =
        all.iter().filter(|s| s.app_rss > 1500.0 && s.ollama > 200.0).collect();

    println!("\n========== MEMORY REPORT ({} samples) ==========", all.len());
    println!("peak app RSS during stage 3 (candle)   : {peak_ai_app:.0} MB");
    for (r, v) in rss_before_stage6.lock().unwrap().iter() {
        println!("app RSS at stage 6 START (run {r})       : {v:.0} MB   <- candle released?");
    }
    println!("peak Ollama resident during stage 6    : {peak_verif_ollama:.0} MB");
    println!("overall peak app RSS (candle)          : {overall_app:.0} MB");
    println!("overall peak Ollama resident (qwen3)   : {overall_ollama:.0} MB");
    println!("peak COEXIST (app_rss + ollama, same t): {peak_coexist:.0} MB");
    println!("ollama resident between run1→run2      : {ollama_after_run1:.0} MB (0 = keep_alive:0 worked)");
    println!(
        "SIMULTANEOUS overlap samples (candle>1500MB AND qwen3>200MB): {}",
        overlap.len()
    );
    if let Some(worst) = overlap.iter().max_by(|a, b| {
        (a.app_rss + a.ollama).partial_cmp(&(b.app_rss + b.ollama)).unwrap()
    }) {
        println!(
            "  worst overlap: t+{}ms run{} stage={} app={:.0}MB ollama={:.0}MB sum={:.0}MB",
            worst.t_ms, worst.run, worst.stage, worst.app_rss, worst.ollama,
            worst.app_rss + worst.ollama
        );
    }
    println!("physical RAM on this machine           : 8192 MB");
    println!("================================================");
}
