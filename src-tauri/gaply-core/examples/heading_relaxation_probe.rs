//! **What would each candidate heading rule ADMIT that the current one does not,
//! and how much of it is prose?**
//!
//! The corpus has 3 files / 2 distinct papers with a missed abstract, out of 32.
//! That is too small a numerator to fit a rule to. What IS measurable at scale is
//! the other direction: every line in every file is a chance for a relaxed rule
//! to invent a heading, and **a false heading is worse than a missed one — it
//! splits a section where none exists**, moving every statistic after it into the
//! wrong place.
//!
//! So this prints what each rule newly admits, as ROWS. The lexicon half of each
//! rule is the REAL `detect_heading_for_probe`, never a copy.

use std::path::Path;

use gaply_core::extract::{docparse, sections};
use regex::Regex;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    // R2: numbering followed by `.` or `)` with NO space required. The current
    // regex demands `\s+`, which is why "VI.Conclusion" is never stripped.
    // `[.)]` is MANDATORY here: without it `[IVXLCM]+` eats "CLIM" from
    // "CLIMATE" and the rule would strip the front off ordinary words.
    // NOTE: the `regex` crate has no look-around, so the "followed by a
    // non-space" half is checked in code below rather than in the pattern.
    // Any production rule inherits the same constraint.
    let glued = Regex::new(r"^\s*(?:\d+(?:\.\d+)*|[IVXLCM]+)[.)]").unwrap();
    // R1: a known heading phrase, then a punctuation separator, then more text.
    let runin = Regex::new(r"^\s*([A-Za-z][A-Za-z &]{0,28}?)\s*[-–—:.]\s*(\S.*)$").unwrap();

    let (mut r1_rows, mut r2_rows) = (Vec::new(), Vec::new());
    // R3 = R1 restricted to the PREAMBLE — every line before the first heading
    // the current rule accepts. Principled rather than fitted: an abstract is
    // the first section of a paper, so a run-in abstract heading cannot appear
    // after one. It cannot rescue a heading that sits mid-document, and saying
    // so is the point.
    let mut r3_rows: Vec<(String, String, String, usize, String)> = Vec::new();
    let mut files = 0usize;
    let mut lines_seen = 0usize;

    for p in &paths {
        let Ok(text) = docparse::parse_path(Path::new(p)) else { continue };
        files += 1;
        let mut seen_heading = false;
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            lines_seen += 1;
            // Only lines the CURRENT rule rejects can be "newly admitted".
            // **THE BASELINE IS THE CURRENT RULE, AND IT IS NOW TWO FUNCTIONS.**
            // After §11 D189 shipped, a run-in heading is accepted by
            // `detect_runin_heading`, not by `detect_heading`. A probe that asks
            // only the latter reports lines as "newly admitted" that the product
            // already admits — measuring against a rule that no longer exists.
            // **THE BASELINE IS THE CURRENT RULE, AND THE CURRENT RULE IS
            // POSITION-DEPENDENT.** Two wrong baselines were tried first, and
            // both are worth naming because each fails in a different
            // direction:
            //
            //   * asking only `detect_heading` under-counts the product — it
            //     reports the run-in abstract as "newly admitted" when §11 D189
            //     already ships it;
            //   * asking `detect_runin_heading` position-INDEPENDENTLY
            //     over-counts it — all 37 R1 lines look already-admitted, when
            //     `split_document` accepts a run-in only in the preamble.
            //
            // The rule is: an ordinary heading anywhere, or a run-in heading
            // before the first heading.
            if sections::detect_heading_for_probe(t).is_some() {
                seen_heading = true;
                continue;
            }
            if !seen_heading && sections::detect_runin_heading_for_probe(t).is_some() {
                seen_heading = true;
                continue;
            }
            // --- R1: run-in heading -------------------------------------
            if let Some(c) = runin.captures(t) {
                let prefix = c.get(1).map(|m| m.as_str()).unwrap_or("");
                let rest = c.get(2).map(|m| m.as_str()).unwrap_or("");
                if let Some((kind, _)) = sections::detect_heading_for_probe(prefix) {
                    let row = (base(p), format!("{kind:?}"), prefix.to_string(),
                               rest.split_whitespace().count(), trunc(t, 110));
                    if !seen_heading {
                        r3_rows.push(row.clone());
                    }
                    r1_rows.push(row);
                }
            }
            // --- R2: numbering glued to the word -------------------------
            if let Some(m) = glued.find(t) {
                let rest = &t[m.end()..];
                // the look-ahead, in code: numbering must abut the word.
                if rest.starts_with(char::is_whitespace) || rest.is_empty() {
                    continue;
                }
                if let Some((kind, _)) = sections::detect_heading_for_probe(rest) {
                    r2_rows.push((base(p), format!("{kind:?}"), trunc(t, 90)));
                }
            }
        }
    }

    println!("files {files}, non-empty lines {lines_seen}\n");
    println!("=== R1 — run-in heading: <known phrase><separator><text> ===");
    println!("newly admitted: {}", r1_rows.len());
    for (f, k, p, w, l) in &r1_rows {
        println!("  {k:<12} prefix={p:<12} rest={w:>4}w  {f}");
        println!("      {l:?}");
    }
    println!("\n=== R3 — R1, but only BEFORE the first heading ===");
    println!("newly admitted: {}", r3_rows.len());
    for (f, k, p2, w, l) in &r3_rows {
        println!("  {k:<12} prefix={p2:<12} rest={w:>4}w  {f}");
        println!("      {l:?}");
    }
    println!("\n=== R2 — numbering glued: <number|roman>[.)]<word> ===");
    println!("newly admitted: {}", r2_rows.len());
    for (f, k, l) in &r2_rows {
        println!("  {k:<12} {f}\n      {l:?}");
    }
    assert!(lines_seen > 0, "nothing scanned — the probe, not the corpus");
}

fn base(p: &str) -> String {
    Path::new(p).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}
fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n).collect::<String>() + "…" }
}
