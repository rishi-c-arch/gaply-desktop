//! **Can a design gate be built? Do the two vocabularies join?**
//!
//! Stage 2 evaluates all five standards against every manuscript, producing 116
//! `NotFound` verdicts over 20 — most on standards that do not apply. The gate
//! that would fix it needs the MANUSCRIPT's design (`claim_strength::read_design`)
//! to match a JOURNAL BINDING's design (`StandardBinding.design`). Those two
//! vocabularies were built for different purposes and may not meet.
use gaply_core::extract::{self, docparse};
use gaply_core::specialist::claim_strength::read_design;
use std::collections::BTreeSet;

/// The design names the 10 stored fingerprints actually bind standards to.
const BOUND_DESIGNS: &[&str] = &[
    "clinical trial", "systematic review", "trial protocol", "randomised trial",
    "observational study", "meta-analysis", "diagnostic accuracy study",
    "animal study", "prediction model study", "cross-sectional study",
    "cohort study", "economic evaluation", "case-control study",
];

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut any_design, mut joins) = (0usize, 0usize);
    let mut vocab: BTreeSet<String> = BTreeSet::new();
    println!("{:<46} {:>26} {}", "manuscript", "design read", "joins a binding?");
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let d = read_design(&ex);
        let mut all: Vec<String> = d.observational.clone();
        all.extend(d.experimental.clone());
        for a in &all {
            vocab.insert(a.clone());
        }
        if !all.is_empty() {
            any_design += 1;
        }
        // Does ANY design the manuscript states match ANY design a journal binds?
        let matched: Vec<&String> = all
            .iter()
            .filter(|a| {
                BOUND_DESIGNS.iter().any(|b| b.contains(a.as_str()) || a.as_str().contains(b))
            })
            .collect();
        if !matched.is_empty() {
            joins += 1;
        }
        println!(
            "{:<46} {:>26} {}",
            name.chars().take(46).collect::<String>(),
            all.join(",").chars().take(26).collect::<String>(),
            if matched.is_empty() { "-" } else { "YES" }
        );
    }
    println!("\n  manuscripts stating ANY design      {any_design} of {}", paths.len());
    println!("  whose design JOINS a bound design   {joins}");
    println!("\n  the vocabulary read_design produced:");
    println!("    {:?}", vocab);
    println!("  the vocabulary journals bind to:");
    println!("    {BOUND_DESIGNS:?}");
}
