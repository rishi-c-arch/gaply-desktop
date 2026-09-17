//! **Is the design `read_design` reports actually the manuscript's design?**
//!
//! A design gate selects which reporting standard a manuscript is judged
//! against. A WRONG design therefore produces confidently wrong compliance
//! failures — §11 D157's shape one level up, where the checker rather than the
//! manuscript is the source of the finding. So every design that would drive
//! the gate is read with its sentence before the gate is built.
use gaply_core::extract::{self, docparse};
use gaply_core::specialist::claim_strength::read_design;

/// The only two concepts that join a journal binding today.
const JOINING: &[&str] = &["cross-sectional", "cross sectional", "randomised", "randomized"];

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let d = read_design(&ex);
        let mut all = d.observational.clone();
        all.extend(d.experimental.clone());
        let joining: Vec<&String> =
            all.iter().filter(|a| JOINING.iter().any(|j| a.as_str() == *j)).collect();
        if joining.is_empty() {
            continue;
        }
        println!("\n######## {name}");
        println!("  designs read : {all:?}");
        println!("  joining      : {joining:?}");
        // The sentence each joining term sits in — the thing a reader checks.
        let lower = text.to_lowercase();
        for j in &joining {
            let mut from = 0usize;
            let mut shown = 0;
            while let Some(at) = lower[from..].find(j.as_str()) {
                let a = from + at;
                let s = lower[..a].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
                let e = lower[a..].find(['.', '\n']).map(|i| a + i).unwrap_or(lower.len());
                println!("    {j:<18} :: {}", lower[s..e].trim().chars().take(400).collect::<String>());
                from = a + j.len();
                shown += 1;
                if shown >= 2 {
                    break;
                }
            }
        }
        if let Some(sp) = &d.limiting_span {
            println!("    limiting_span     :: {}", sp.chars().take(400).collect::<String>());
        }
    }
}
