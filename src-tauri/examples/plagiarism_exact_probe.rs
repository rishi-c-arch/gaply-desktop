//! Set-2 timing reading (MEASUREMENT ONLY) for the deterministic exact-match
//! plagiarism core. Builds ~500 pages of text with real recycled passages,
//! runs SELF mode + LIBRARY mode against a few comparison docs, and reports
//! wall-clock + peak RSS. The investigation predicts near-linear (winnowing +
//! postings-index candidate generation) → seconds, well under 8GB.
//!
//!   cargo run --release --example plagiarism_exact_probe

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gaply_core::plagiarism_exact::{analyze_exact, CompareDoc, ExactConfig};

fn own_rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

/// Distinct-per-index prose so most of the document is unique, with a recycled
/// paragraph sprinkled every ~20 paragraphs (real self-plagiarism to extend).
fn build_document(paras: usize) -> String {
    let recycled = "The mitochondrial membrane potential collapses during the early phase of \
        apoptosis and triggers the release of cytochrome c into the cytosol, activating the \
        caspase cascade that dismantles the cell in an orderly and irreversible fashion. ";
    let mut s = String::with_capacity(paras * 400);
    for i in 0..paras {
        if i % 20 == 7 {
            s.push_str(recycled); // recurs → self-match
        } else {
            for j in 0..14 {
                s.push_str(&format!("token{i}x{j} "));
            }
        }
        s.push_str(&format!("Paragraph {i} closes with a unique sentence numbered {i}.\n\n"));
    }
    s
}

fn build_library(docs: usize, shared: &str) -> Vec<CompareDoc> {
    (0..docs)
        .map(|d| {
            let mut t = String::new();
            for i in 0..1500 {
                t.push_str(&format!("libdoc{d}word{i} "));
            }
            // one comparison doc shares a paragraph with the source (a real
            // cross-document match to find + extend).
            if d == 0 {
                t.push_str(shared);
            }
            CompareDoc { reference: format!("Library paper {d}"), text: t }
        })
        .collect()
}

fn main() {
    // ~500 pages: ~5000 paragraphs of mostly-unique prose ≈ 250k+ words.
    let paras = 5000;
    let document = build_document(paras);
    let shared = "In the shared section both papers contain the identical sentence about \
        telomere attrition accelerating replicative senescence in cultured human fibroblasts.";
    let library = build_library(4, shared);
    let mut doc2 = document.clone();
    doc2.push_str(shared); // ensure the source also contains the shared library paragraph

    let words = doc2.split_whitespace().count();
    let pages = doc2.chars().count() / 2500;
    println!(
        "document: {} chars (~{pages} pages, {words} words) + {} library docs",
        doc2.chars().count(),
        library.len()
    );

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
                thread::sleep(Duration::from_millis(50));
            }
        })
    };

    let t = Instant::now();
    let report = analyze_exact(&doc2, &library, &ExactConfig::default());
    let secs = t.elapsed().as_secs_f64();

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let peak = *peak.lock().unwrap();

    let all_pairs = report.stats.source_fingerprints * report.stats.comparison_fingerprints;
    println!("\n========== EXACT-MATCH TIMING READING ==========");
    println!("pages (approx)          : ~{pages}");
    println!("wall-clock (self+lib)   : {secs:.3}s");
    println!("peak RSS                : {peak:.0} MB (8192 MB physical)");
    println!("self matches            : {}", report.self_matches.len());
    println!("library matches         : {}", report.library_matches.len());
    println!("duplication ratio       : {:.4}", report.duplication_ratio);
    println!(
        "source / comparison fp  : {} / {}",
        report.stats.source_fingerprints, report.stats.comparison_fingerprints
    );
    println!(
        "candidate seeds          : {}  (all-pairs would be {}, ratio 1/{})",
        report.stats.candidate_seeds,
        all_pairs,
        if report.stats.candidate_seeds == 0 { all_pairs } else { all_pairs / report.stats.candidate_seeds.max(1) }
    );
    println!("================================================");
}
