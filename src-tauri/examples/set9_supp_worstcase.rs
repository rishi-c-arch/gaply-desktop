//! Set-9 addendum: worst-case PARSEABLE supplementary CSV, standalone RSS
//! measurement. The caps reject >10MB files and >5000-row CSVs outright, so
//! the true worst case is ~5000 heavy rows just under 10MB. Samples this
//! process's RSS at 50ms cadence across the parse and prints the peak delta.
//! MEASUREMENT ONLY.

use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use app_lib::supplementary::parse_supplementary;

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
    // Maximal parseable input: ~9.9MB file (under the 10MB cap) whose huge
    // cells clamp to 512 chars, keeping ~1.95MB (under the 2MB kept cap).
    let path = std::env::temp_dir().join("gaply_set9_worstcase.csv");
    {
        let mut f = std::fs::File::create(&path).expect("create");
        writeln!(f, "note").unwrap();
        let huge = "x".repeat(2600);
        for _ in 0..3800 {
            writeln!(f, "{huge}").unwrap();
        }
    }
    let size = std::fs::metadata(&path).unwrap().len();
    let before = own_rss_mb();
    println!("file: {:.2} MB, rss before parse: {before:.0} MB", size as f64 / (1024.0 * 1024.0));

    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(Mutex::new(before));
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

    let ev = parse_supplementary(&path).expect("worst-case csv parses");
    let after_parse = own_rss_mb();
    let kept = serde_json::to_string(&ev).map(|s| s.len()).unwrap_or(0);
    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let _ = std::fs::remove_file(&path);

    let peak = *peak.lock().unwrap();
    println!("rss after parse : {after_parse:.0} MB (evidence kept: {} KB serialized)", kept / 1024);
    println!("PEAK during parse: {peak:.0} MB  (delta over baseline: {:.0} MB)", peak - before);
}
