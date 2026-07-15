//! AI Check Set-3 feasibility re-measurement — MEASUREMENT ONLY. Proves the
//! two-stage strategy makes large-document analysis workable: Stage 1
//! (heuristic pre-pass, whole document) in seconds; Stage 2 (real SLM-1,
//! candidates only, default budget) in bounded minutes.
//!
//!   cargo run --release --example aicheck_tiered_probe

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gaply_core::ai_detect::{
    self, HeuristicModel, DEFAULT_MAX_DEEP_PASSAGES, DEFAULT_MAX_DEEP_TOKENS,
};
use gaply_core::extract::extract_from_text;

fn own_rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid]).output();
    out.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

fn main() {
    // ~150 pages of sectioned prose, mixed AI-like and human-like.
    let ai_para = "The results show that the model is able to process the data in a way that \
is both fast and easy. The system can handle many tasks at the same time. It is designed to \
be simple and clear for the user. The method works well for all of the tasks in the study. ";
    let human_para = "Frankly, the contraption wheezed like an asthmatic accordion, belching \
improbable cerulean plumes. Nobody anticipated theatrical flair from a glorified toaster; \
yet there it sputtered, magnificent, absurd, and quietly triumphant against expectation. ";
    let mut text = String::from("Introduction\n\n");
    for i in 0..1500 {
        text.push_str(if i % 4 == 0 { ai_para } else { human_para });
        if i % 6 == 5 {
            text.push_str("\n\n");
        }
        if i == 500 {
            text.push_str("\n\nMethods\n\n");
        }
        if i == 1000 {
            text.push_str("\n\nDiscussion\n\n");
        }
    }
    let pages = text.chars().count() / 2500;
    println!("document: {} chars (~{pages} pages)", text.chars().count());

    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(Mutex::new(own_rss_mb()));
    let sampler = {
        let (stop, peak) = (stop.clone(), peak.clone());
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let r = own_rss_mb();
                let mut p = peak.lock().unwrap();
                if r > *p {
                    *p = r;
                }
                drop(p);
                thread::sleep(Duration::from_millis(100));
            }
        })
    };

    let t_extract = Instant::now();
    let ex = extract_from_text(&text);
    println!("extraction: {:.1}s", t_extract.elapsed().as_secs_f64());

    // ---- STAGE 1 alone (the whole-document heuristic pre-pass) -----------
    let fast = HeuristicModel::default();
    let t1 = Instant::now();
    let stage1 = ai_detect::analyze_passages(&fast, &ex);
    let stage1_secs = t1.elapsed().as_secs_f64();
    println!(
        "STAGE 1 (heuristic, ALL {pages} pages): {stage1_secs:.1}s → {} candidate passage(s)",
        stage1.passages.len()
    );

    // ---- The full two-stage flow with the REAL deep model ----------------
    let deep = app_lib::models::perplexity_model();
    println!("deep model: {}", deep.name());
    let t2 = Instant::now();
    let out = ai_detect::analyze_tiered(
        &fast,
        Some(&*deep),
        &ex,
        DEFAULT_MAX_DEEP_PASSAGES,
        DEFAULT_MAX_DEEP_TOKENS,
        ai_detect::DeepKind::Full,
        None,
    );
    let total_secs = t2.elapsed().as_secs_f64();
    drop(deep); // one-at-a-time: the model drops here

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let peak = *peak.lock().unwrap();

    println!("\n========== TWO-STAGE FEASIBILITY REPORT ==========");
    println!("pages                    : ~{pages}");
    println!("stage 1 (whole doc)      : {stage1_secs:.1}s");
    println!("two-stage total          : {total_secs:.1}s ({:.1} min)", total_secs / 60.0);
    println!("coverage                 : {}", out.coverage_note);
    println!(
        "proportion               : {:.1}% of text shows AI-associated signals",
        out.ai_signal_proportion * 100.0
    );
    println!("peak RSS                 : {peak:.0} MB (8192 MB physical)");
    println!("==================================================");
}
