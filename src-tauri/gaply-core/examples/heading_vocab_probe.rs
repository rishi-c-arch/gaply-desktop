//! **What headings does the corpus actually write, against the 24 phrases the
//! classifier knows?**
//!
//! `detect_heading` requires an EXACT match against `classify_heading`'s 24
//! phrases after stripping numbering and a trailing colon. Extending that list
//! from memory is the lexicon problem again, so this asks the corpus first.
//!
//! It reports every heading-SHAPED line — short, title-ish, not a sentence —
//! and whether the classifier claims it. The unmatched column is the real
//! vocabulary gap; the matched column is the control that says the detector
//! works at all.

use std::collections::BTreeMap;
use std::path::Path;

use gaply_core::extract::{docparse, sections};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: heading_vocab_probe <file>...");
        std::process::exit(2);
    }

    let mut matched: BTreeMap<String, usize> = BTreeMap::new();
    let mut unmatched: BTreeMap<String, usize> = BTreeMap::new();
    let mut files = 0usize;

    for p in &paths {
        let Ok(text) = docparse::parse_path(Path::new(p)) else { continue };
        files += 1;
        for line in text.lines() {
            let t = line.trim();
            // The same shape gate `detect_heading` applies, so the two columns
            // are comparable: anything here is a line the detector LOOKED at.
            if t.is_empty() || t.split_whitespace().count() > 5 {
                continue;
            }
            if !looks_like_a_heading(t) {
                continue;
            }
            let key = normalise(t);
            if key.is_empty() {
                continue;
            }
            match sections::detect_heading_for_probe(t) {
                Some((kind, _)) => *matched.entry(format!("{kind:?} <- {key}")).or_default() += 1,
                None => *unmatched.entry(key).or_default() += 1,
            }
        }
    }

    println!("files parsed: {files}\n");
    println!("=== MATCHED ({} distinct) — the control ===", matched.len());
    for (k, n) in &matched {
        println!("  {n:>4}  {k}");
    }
    println!("\n=== UNMATCHED heading-shaped lines ({} distinct) ===", unmatched.len());
    let mut rows: Vec<(&String, &usize)> = unmatched.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (k, n) in rows.iter().take(120) {
        println!("  {n:>4}  {k}");
    }

    assert!(!matched.is_empty(), "no heading matched anywhere — the detector, not the corpus");
}

/// Title-ish: begins with a letter or a number, is not a full sentence, and is
/// not obviously body text. Deliberately permissive — this is a survey, and a
/// gate tuned to the answer would only confirm itself.
fn looks_like_a_heading(t: &str) -> bool {
    let first = t.chars().find(|c| c.is_alphanumeric());
    if !first.is_some_and(|c| c.is_alphanumeric()) {
        return false;
    }
    if t.ends_with('.') && t.split_whitespace().count() > 3 {
        return false;
    }
    // A line that is mostly digits is a table row or a page number.
    let alpha = t.chars().filter(|c| c.is_alphabetic()).count();
    alpha >= 4 && t.chars().any(|c| c.is_alphabetic())
}

/// Strip leading numbering and a trailing colon, lowercase — the same
/// normalisation `detect_heading` performs, so the keys are comparable.
fn normalise(t: &str) -> String {
    let t = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')' || c == ' ');
    t.trim().trim_end_matches(':').trim().to_lowercase()
}
