//! Set-2 Gap Finder memory demonstration — MEASUREMENT ONLY. Builds a
//! worst-case paper corpus (MAX_PAPERS files, one near the 20MB cap, plus an
//! over-cap file that must be rejected via metadata alone) while sampling this
//! process's RSS at 50ms. Proves N-paper ingestion is text-processing-scale —
//! nowhere near the 8GB cliff — and that oversize input rejects honestly.
//!
//!   cargo run --release --example gapfinder_corpus_probe

use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gaply_core::embed::HashEmbedder;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::Database;

use app_lib::paper_corpus::{
    build_corpus, digests_measure, PaperFetch, MAX_PAPERS,
};
use gaply_core::GaplyError;

fn own_rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid]).output();
    out.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

struct NoFetch;
impl PaperFetch for NoFetch {
    fn get_capped(&self, _url: &str, _cap: usize) -> Result<(u16, Vec<u8>), GaplyError> {
        Ok((404, Vec::new()))
    }
}

fn write_paper(name: &str, megabytes: usize) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("gaply_s2_probe_{name}.txt"));
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, "Probe Paper {name}: A Study of Sleep and Memory\n\nAbstract\nWe study things carefully with n = 96 and p = 0.002 across waves.\n").unwrap();
    let para = "This is a long paragraph of study prose repeated to reach the target size. \
It discusses sleep, memory, methods, results and statistics such as t(95) = 3.1. "
        .repeat(64); // ~10KB
    for _ in 0..(megabytes * 100) {
        f.write_all(para.as_bytes()).unwrap();
        f.write_all(b"\n\n").unwrap();
    }
    p
}

fn main() {
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
                thread::sleep(Duration::from_millis(50));
            }
        })
    };

    let baseline = own_rss_mb();
    println!("baseline RSS: {baseline:.0} MB");

    // TRUE worst case within caps: MAX_PAPERS files, each just under the
    // 20MB cap (~152MB of paper text total) through parse → digest → RAG
    // ingest, sequentially.
    let paths: Vec<String> = (0..MAX_PAPERS)
        .map(|i| write_paper(&format!("near{i}"), 19).to_string_lossy().into_owned())
        .collect();

    let db = Arc::new(Database::in_memory().unwrap());
    let report = build_corpus(&db, &HashEmbedder, "probe", &paths, &[], &NoFetch, &RateLimiter::new(100.0, 100.0))
        .expect("corpus builds");
    let (total, max_field) = digests_measure(&report.digests);
    println!(
        "corpus: {} digest(s); digests measure {total} chars total, {max_field} max field",
        report.digests.len()
    );

    // Over-cap file: must reject on metadata, without reading it.
    let big = write_paper("overcap", 25);
    let report2 = build_corpus(
        &db, &HashEmbedder, "probe2",
        &[big.to_string_lossy().into_owned()], &[],
        &NoFetch, &RateLimiter::new(100.0, 100.0),
    )
    .expect("report with honest rejection");
    println!("over-cap file: {:?}", report2.results[0]);

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_file(big);

    let peak = *peak.lock().unwrap();
    println!("PEAK RSS during worst-case corpus build: {peak:.0} MB (baseline {baseline:.0} MB, delta {:.0} MB)", peak - baseline);
    println!("physical RAM: 8192 MB");
}
