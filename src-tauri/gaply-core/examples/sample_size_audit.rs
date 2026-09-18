//! **Where does `n = 3` come from, and does the manuscript say otherwise?**
//!
//! `validate.rs` rule 5 raises a CRITICAL finding — "small sample with causal
//! claim" — from any `Stat::SampleSize { n } if n < 10` sharing a paragraph with
//! causal language. The severity means a user is told their study is fatally
//! underpowered, so the number had better be the study's.
//!
//! Prints EVERY sample size the extractor found, with the sentence it was read
//! from, smallest first — so a wrong read is visible rather than inferred.
use gaply_core::extract::stats::Stat;
use gaply_core::extract::{self, docparse, paragraph_at};

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let mut rows: Vec<(i64, String, String)> = Vec::new();
        for c in &ex.statistics {
            if let Stat::SampleSize { n, .. } = &c.stat {
                let para = paragraph_at(&ex, &c.location).unwrap_or("<unresolved>").to_string();
                // The SENTENCE containing the number, not the paragraph.
                let needle = format!("{n}");
                let sent = para
                    .split(['.', '\n'])
                    .find(|s| s.contains(&needle))
                    .unwrap_or(&para)
                    .trim()
                    .chars()
                    .take(260)
                    .collect::<String>();
                rows.push((*n, format!("{:?}", c.location), sent));
            }
        }
        rows.sort_by_key(|(n, _, _)| *n);
        println!("\n######## {name}   ({} sample sizes read)", rows.len());
        for (n, loc, sent) in rows.iter().take(14) {
            let mark = if *n < 10 { "  <-- TRIPS RULE 5 (Critical)" } else { "" };
            println!("  n = {n:<7}{mark}\n     at  {loc}\n     ::  {sent}");
        }
        if rows.len() > 14 {
            println!("  … {} more", rows.len() - 14);
        }
    }
}
