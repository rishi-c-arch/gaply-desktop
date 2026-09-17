//! **The QUIET error: is the gate excluding papers that DO have an applicable standard?**
//!
//! `design_span_check` asks whether a JOINING read is correct. This asks the
//! opposite question of the manuscripts that do NOT join: does the paper
//! genuinely have a design one of the 13 bound standards covers, missed because
//! the two vocabularies share no substring?
//!
//! Prints, for every non-joining manuscript: every marker `read_design`
//! returned, the SENTENCE each was keyed on (computed here — `read_design`
//! stores `limiting_span` only for observational markers, so experimental reads
//! carry no span at all), and the title/opening so a reader can judge what the
//! paper actually is rather than infer it from the marker.
use gaply_core::extract::{self, docparse};
use gaply_core::specialist::claim_strength::read_design;

/// The design names the 10 stored fingerprints bind standards to.
const BOUND_DESIGNS: &[&str] = &[
    "clinical trial", "systematic review", "trial protocol", "randomised trial",
    "observational study", "meta-analysis", "diagnostic accuracy study",
    "animal study", "prediction model study", "cross-sectional study",
    "cohort study", "economic evaluation", "case-control study",
];

fn joins(marker: &str) -> Option<&'static str> {
    BOUND_DESIGNS
        .iter()
        .find(|b| b.contains(marker) || marker.contains(**b))
        .copied()
}

fn sentence_for<'a>(lower: &'a str, needle: &str) -> Option<&'a str> {
    let at = lower.find(needle)?;
    let s = lower[..at].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
    let e = lower[at..].find(['.', '\n']).map(|i| at + i).unwrap_or(lower.len());
    Some(lower[s..e].trim())
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut no_design, mut non_joining) = (0usize, 0usize);
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let d = read_design(&ex);
        let mut all = d.observational.clone();
        all.extend(d.experimental.clone());
        if all.iter().any(|m| joins(m).is_some()) {
            continue; // handled by design_span_check
        }
        non_joining += 1;
        if all.is_empty() {
            no_design += 1;
        }
        println!("\n######## {name}");
        println!("  designs read : {all:?}   {}", if all.is_empty() { "<- NO DESIGN READ" } else { "" });
        let lower = text.to_lowercase();
        for m in &all {
            let kind = if d.observational.contains(m) { "obs" } else { "exp" };
            match sentence_for(&lower, m) {
                Some(s) => println!("    [{kind}] {m:<18} :: {}", s.chars().take(300).collect::<String>()),
                None => println!("    [{kind}] {m:<18} :: <no sentence — matched across a break>"),
            }
        }
        // What the paper actually IS, so the judgement is not made from the marker.
        let opening: String = ex
            .sections
            .iter()
            .flat_map(|s| s.paragraphs.iter())
            .find(|p| p.trim().len() > 120)
            .map(|p| p.chars().take(380).collect())
            .unwrap_or_default();
        println!("    OPENING            :: {opening}");
    }
    println!("\n  non-joining manuscripts    {non_joining} of {}", paths.len());
    println!("  of those, NO design read   {no_design}");
}
