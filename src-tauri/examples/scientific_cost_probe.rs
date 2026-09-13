//! **What the opt-in scientific layer costs, measured.**
//!
//! `ExtractOptions::scientific` is off everywhere. Its own comment says why:
//! it made every caller *"pay for four extra passes over the manuscript to
//! populate a field none of them reads"*. **That cost has never been measured**,
//! and Phase 2 cannot decide who turns it on without a number — 200 ms and
//! 20 s are different decisions.
//!
//! This times `extract_from_text` (base) against `extract_from_text_with(…,
//! with_scientific())` on REAL manuscripts, not fixtures. Parsing is done once
//! per file and excluded: it is a cost both arms pay, and including it would
//! dilute the delta this probe exists to isolate.
//!
//! # THE BASELINE, measured 13 Sep 2026 (release, median of 5, parse excluded)
//!
//! Six real manuscripts. Base extraction 95 ms total; with the scientific layer
//! **30,474 ms — a 320x slowdown**, 0.8–14.5 s per manuscript.
//!
//! ```text
//! manuscript              sentences   claims  variables    methods   datasets   found c/v/m/d
//! IJAS haemolymph.pdf            64     0.7ms     78.7ms    268.9ms    447.7ms   3/3/2/0
//! R PAPER .docx                 247     0.5ms    175.6ms   1723.5ms    913.1ms   5/1/30/0
//! Revised Health Econ.docx      305     1.2ms    284.7ms   2299.9ms   1214.2ms   12/2/27/0
//! final final L.pdf            1390    14.7ms   2097.9ms   8248.6ms   4292.1ms   82/12/79/0
//! Lake Chapter 1.docx           105     1.4ms    182.5ms    729.1ms      0.0ms   15/3/4/0
//! chapter3 .docx                629     1.9ms    530.6ms   3958.7ms   3097.5ms   6/2/10/0
//! ```
//!
//! **`methods` dominates 5 of 6 (52–80%); `datasets` is 29–41% and returns ZERO
//! on all six. `claims` is free.**
//!
//! ## It is linear in SENTENCES, not in matches or chars
//!
//! `methods` costs **4.2–7.5 ms per sentence across a 22x range of inputs**
//! while match counts vary 2 -> 79. Lake Chapter 1 (4 methods) and R PAPER
//! (30 methods) are 6.94 and 6.98 ms/sentence — indistinguishable. A constant
//! per-sentence cost that ignores its own results is work done BEFORE matching.
//!
//! Chars are the wrong denominator and misled the first reading of this data:
//! `chapter3` (74k chars, 629 sentences) costs 5.4x `Lake Chapter 1` (62k chars,
//! 105 sentences).
//!
//! ## The cause: regexes compiled per sentence
//!
//! `datasets.rs:222-224` and `methods.rs:387-390` / `401-404` build a `Regex`
//! inside a loop over a pattern list, on every sentence. The LISTS are
//! `OnceLock`-cached; the compiled regexes are not. ~45 compilations per
//! sentence in `methods`, ~22 in `datasets`, plus ~10 one-off in
//! `methods.rs:346-425`. `claims.rs` and `variables.rs` cache correctly and cost
//! 0% and 6–20%.
//!
//! The arithmetic closes: R PAPER, 247 sentences x 67 compiles = 16,549 at
//! ~150us ~= 2.48 s, against 2.64 s measured for those two passes.
//!
//! Secondary, not dominant: `methods.rs:447` runs
//! `result.statistics.iter().position(...)` inside a loop over
//! `result.statistics`, recovering an index it already has.
//!
//! ## What this predicts
//!
//! Caching the regexes should leave only matching. `variables.rs` is the
//! calibration point at 1.3–1.9 ms/sentence, so the six-manuscript total should
//! fall from 30.4 s to roughly 4–6 s; deleting the empty `datasets` pass takes
//! another ~30%. **Predictions, not results — re-run this probe after the fix
//! rather than quoting them.**
//!
//! Usage: `cargo run --release --example scientific_cost_probe -- <file>…`
//!
//! **Release build, deliberately.** The extractors are regex-heavy and a debug
//! build would report a number nobody experiences (CLAUDE.md's standing
//! `tauri dev` lesson, applied to a probe).
//!
//! # The known-good row
//!
//! `--known-good <path>:<expected_references>` names one file whose answer is
//! already known, and the probe **checks it first and exits non-zero if it is
//! wrong**. A batch of clean results is the signature of a broken harness, and
//! the only thing that can tell you is a row whose answer you knew in advance.

use std::time::{Duration, Instant};

use gaply_core::extract::{self, claims, datasets, docparse, methods, variables, ExtractOptions};

/// Runs per arm. The median is reported, so one scheduler hiccup cannot move
/// the headline; min is printed too because it is the cleanest floor.
const RUNS: usize = 5;

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn time_it(runs: usize, mut f: impl FnMut()) -> (Duration, Duration) {
    f(); // warm: first call pays lazy regex compilation, which is per-process
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        f();
        samples.push(t.elapsed());
    }
    let min = *samples.iter().min().unwrap();
    (median(samples), min)
}

fn main() {
    let mut paths: Vec<String> = Vec::new();
    let mut known_good: Option<(String, usize)> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--known-good" {
            let spec = args.next().expect("--known-good <path>:<expected_refs>");
            let (p, n) = spec.rsplit_once(':').expect("--known-good <path>:<expected_refs>");
            known_good = Some((p.to_string(), n.parse().expect("expected_refs must be a number")));
            paths.push(p.to_string());
        } else {
            paths.push(a);
        }
    }
    assert!(!paths.is_empty(), "usage: scientific_cost_probe [--known-good <path>:<n>] <file>…");

    // ---- THE KNOWN-GOOD ROW, CHECKED FIRST -------------------------------
    if let Some((path, expected)) = &known_good {
        let text = docparse::parse_path(std::path::Path::new(path)).expect("known-good must parse");
        let ex = extract::extract_from_text(&text);
        let got = ex.references.len();
        println!("known-good: {} -> {got} reference(s), expected {expected}", short(path));
        if got != *expected {
            eprintln!(
                "KNOWN-GOOD FAILED: {} yielded {got} references, expected {expected}.\n\
                 The harness is wrong, so every row below is meaningless. Not reporting them."
            , short(path));
            std::process::exit(2);
        }
        println!("known-good OK — the harness reproduces a value measured independently.\n");
    } else {
        eprintln!("REFUSING TO RUN: no --known-good row.\n\
                   A batch with no case whose answer is already known cannot tell a working \
                   harness from a broken one (CLAUDE.md).");
        std::process::exit(2);
    }

    println!(
        "{:<34} {:>9} {:>10} {:>10} {:>9} {:>7}  {}",
        "manuscript", "chars", "base ms", "sci ms", "delta ms", "delta%", "claims/vars/methods/datasets"
    );

    let mut total_base = Duration::ZERO;
    let mut total_sci = Duration::ZERO;

    for p in &paths {
        let text = match docparse::parse_path(std::path::Path::new(p)) {
            Ok(t) => t,
            Err(e) => {
                println!("{:<34} PARSE FAILED: {e}", short(p));
                continue;
            }
        };
        let (base, base_min) = time_it(RUNS, || {
            std::hint::black_box(extract::extract_from_text(std::hint::black_box(&text)));
        });
        let (sci, sci_min) = time_it(RUNS, || {
            std::hint::black_box(extract::extract_from_text_with(
                std::hint::black_box(&text),
                ExtractOptions::with_scientific(),
            ));
        });

        // ---- PER-PASS BREAKDOWN ------------------------------------------
        // The four passes are CHAINED — each consumes the previous outputs —
        // so they are timed in sequence rather than independently. That is the
        // real shape of the cost: `methods` cannot be run without `claims` and
        // `variables` first, so "what would removing methods save" is answered
        // by its own segment, not by a standalone run.
        let base_ex = extract::extract_from_text(&text);
        let t = Instant::now();
        let c = claims::extract_claims(&base_ex);
        let t_claims = t.elapsed();
        let t = Instant::now();
        let v = variables::extract_variables(&base_ex, &c);
        let t_vars = t.elapsed();
        let t = Instant::now();
        let m = methods::extract_methods(&base_ex, &c, &v);
        let t_methods = t.elapsed();
        let t = Instant::now();
        let d = datasets::extract_datasets(&base_ex, &c, &v, &m);
        let t_datasets = t.elapsed();
        let ms = |x: Duration| x.as_secs_f64() * 1000.0;
        let sum = ms(t_claims) + ms(t_vars) + ms(t_methods) + ms(t_datasets);
        let pass_pct = |x: Duration| if sum > 0.0 { ms(x) / sum * 100.0 } else { f64::NAN };
        eprintln!(
            "  PASSES {:<30} claims {:>8.1}ms ({:>4.0}%)  vars {:>8.1}ms ({:>4.0}%)  \
             methods {:>9.1}ms ({:>4.0}%)  datasets {:>8.1}ms ({:>4.0}%)  \
             | n= {}/{}/{}/{}  sentences={}",
            short(p),
            ms(t_claims), pass_pct(t_claims),
            ms(t_vars), pass_pct(t_vars),
            ms(t_methods), pass_pct(t_methods),
            ms(t_datasets), pass_pct(t_datasets),
            c.len(), v.len(), m.len(), d.len(),
            base_ex.sections.iter().map(|s| s.paragraphs.len()).sum::<usize>(),
        );

        let ex = extract::extract_from_text_with(&text, ExtractOptions::with_scientific());
        let counts = match &ex.scientific {
            Some(s) => format!(
                "{}/{}/{}/{}",
                s.claims.len(),
                s.variables.len(),
                s.methods.len(),
                s.datasets.len()
            ),
            // The layer produces NOTHING on this paper. That is a result, not a
            // failure — and it is the column that decides whether the cost buys
            // anything at all.
            None => "none".to_string(),
        };

        let delta = sci.saturating_sub(base);
        let pct = if base.as_secs_f64() > 0.0 {
            delta.as_secs_f64() / base.as_secs_f64() * 100.0
        } else {
            f64::NAN
        };
        println!(
            "{:<34} {:>9} {:>10.1} {:>10.1} {:>9.1} {:>6.0}%  {}",
            short(p),
            text.len(),
            base.as_secs_f64() * 1000.0,
            sci.as_secs_f64() * 1000.0,
            delta.as_secs_f64() * 1000.0,
            pct,
            counts
        );
        eprintln!(
            "  (min: base {:.1} ms, sci {:.1} ms)",
            base_min.as_secs_f64() * 1000.0,
            sci_min.as_secs_f64() * 1000.0
        );
        total_base += base;
        total_sci += sci;
    }

    println!(
        "\nTOTAL base {:.1} ms   sci {:.1} ms   added {:.1} ms over {} manuscript(s), median of {RUNS} runs",
        total_base.as_secs_f64() * 1000.0,
        total_sci.as_secs_f64() * 1000.0,
        total_sci.saturating_sub(total_base).as_secs_f64() * 1000.0,
        paths.len()
    );
}

fn short(p: &str) -> String {
    let base = std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string());
    base.chars().take(33).collect()
}
