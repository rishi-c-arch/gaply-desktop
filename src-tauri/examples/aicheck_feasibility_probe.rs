//! AI Check Set-2 feasibility reading — MEASUREMENT ONLY, not the full
//! 500-page probe (that's a later set). Runs the REAL SLM-1 candle model
//! once over a ~75-page synthetic document through the new passage-level
//! analysis, reporting wall-clock + peak RSS — the rough per-page cost
//! before Set 3 adds the SLM-2 per-passage classifier.
//!
//!   cargo run --release --example aicheck_feasibility_probe
//! (SLM-1 GGUF at ~/gaply-models/slm1; falls back to HeuristicModel with an
//! honest note if absent.)

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gaply_core::ai_detect;

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
    // ~75 pages: a page ≈ 2,500 chars of prose → ~190k chars, mixed AI-like
    // (common-word) and human-like (varied) paragraphs so flagging happens.
    let ai_para = "The results show that the model is able to process the data in a way that \
is both fast and easy. The system can handle many tasks at the same time. It is designed to \
be simple and clear for the user. The method works well for all of the tasks in the study. ";
    let human_para = "Frankly, the contraption wheezed like an asthmatic accordion, belching \
improbable cerulean plumes. Nobody anticipated theatrical flair from a glorified toaster; \
yet there it sputtered, magnificent, absurd, and quietly triumphant against expectation. ";
    let mut text = String::new();
    for i in 0..380 {
        text.push_str(if i % 3 == 0 { human_para } else { ai_para });
        if i % 5 == 4 {
            text.push_str("\n\n");
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

    let baseline = own_rss_mb();
    let model = app_lib::models::perplexity_model();
    println!("model: {} (baseline RSS {baseline:.0} MB)", model.name());

    let t0 = Instant::now();
    let out = ai_detect::analyze_passages_text(&*model, &text);
    let secs = t0.elapsed().as_secs_f64();
    drop(model);

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let peak = *peak.lock().unwrap();

    println!("\n========== AI CHECK FEASIBILITY READING ==========");
    println!("passages flagged        : {}", out.passages.len());
    println!(
        "ai_signal_proportion    : {:.1}% of text shows AI-associated signals",
        out.ai_signal_proportion * 100.0
    );
    println!("wall-clock (SLM-1 pass) : {secs:.1}s for ~{pages} pages ({:.2}s/page)", secs / pages.max(1) as f64);
    println!("peak RSS                : {peak:.0} MB (baseline {baseline:.0} MB)");
    println!("physical RAM            : 8192 MB");
    println!("==================================================");
}
