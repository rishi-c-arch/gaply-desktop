//! **Does the journal checklist conclude a compliance failure from a MEDIATED
//! absence?** Measured against the manuscript, not argued.
//!
//! `checklist_from_requirements` decides "data availability statement" by
//! scanning `extraction.sections[heading]`. §4.5 [v8]'s absence rule classes
//! `StateField::Sections` as Mediated. This prints both sides for one real
//! manuscript: the headings the classifier produced, and where the phrase
//! actually occurs in the text.

use gaply_core::extract::{self, docparse};

fn main() {
    for p in std::env::args().skip(1) {
        let text = docparse::parse_path(std::path::Path::new(&p)).expect("parse");
        let ex = extract::extract_from_text(&text);
        println!("=== {} ===", short(&p));
        println!("  headings the classifier produced ({}):", ex.sections.len());
        for s in &ex.sections {
            println!("    {:?}  {:?}", s.kind, s.heading);
        }
        for needle in ["data availability", "ethic", "competing interest", "funding"] {
            let lower = text.to_lowercase();
            let in_heading =
                ex.sections.iter().any(|s| s.heading.to_lowercase().contains(needle));
            let at = lower.find(needle);
            println!(
                "\n  {needle:?}\n    in a section HEADING : {in_heading}\n    in the FULL TEXT    : {}",
                at.is_some()
            );
            if let Some(i) = at {
                let start = i.saturating_sub(90);
                let end = (i + 190).min(text.len());
                let slice: String = text
                    .char_indices()
                    .filter(|(j, _)| *j >= start && *j < end)
                    .map(|(_, c)| c)
                    .collect();
                println!("    the manuscript's own words: …{}…", slice.replace('\n', " "));
            }
        }
        println!();
    }
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
