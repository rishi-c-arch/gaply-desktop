//! **Which research-state fields can an ABSENCE be concluded from?**
//!
//! Measured before the lens rule is written. A lens reading `ResearchState`
//! directly does what a specialist would have done, so the only thing between
//! it and fabrication is what it may conclude from an empty field.
//!
//! Two kinds of field:
//!
//! * **Exhaustive** — every word of the manuscript was searched. A miss can only
//!   be a lexicon miss.
//! * **Mediated** — a projection the extractor built, which can omit what the
//!   manuscript contains. A miss can be the extractor's.
//!
//! This prints, per manuscript, what each field holds — so the rule rests on how
//! often a mediated field is empty on real input rather than on an argument.

use gaply_core::extract::{self, docparse, SectionKind};

const KINDS: &[SectionKind] = &[
    SectionKind::Abstract,
    SectionKind::Introduction,
    SectionKind::Methods,
    SectionKind::Results,
    SectionKind::Discussion,
    SectionKind::Conclusion,
    SectionKind::References,
];

/// Phrases a data-availability statement is made of. Searched over the FULL
/// TEXT, not over a section.
const DATA_AVAILABILITY: &[&str] = &[
    "data availability",
    "data are available",
    "data is available",
    "data will be made available",
    "available on request",
    "available upon request",
    "deposited in",
    "supplementary data",
    "underlying data",
];
const ETHICS: &[&str] = &[
    "ethics approval",
    "ethical approval",
    "ethics committee",
    "institutional review board",
    "irb",
    "iacuc",
    "informed consent",
    "declaration of helsinki",
    "ethical clearance",
    "consent to participate",
];
const RANDOMISATION: &[&str] =
    &["randomis", "randomiz", "randomly assigned", "randomly allocated", "blinded", "blinding"];

fn main() {
    println!(
        "{:<42} {:>6} {:>6} {:>6} {:>6} | sections extracted",
        "manuscript", "stats", "refs", "cites", "tables"
    );
    let mut missing_kind = vec![0usize; KINDS.len()];
    let mut no_title = 0usize;
    let mut junk_title = 0usize;
    let mut n = 0usize;

    let paths: Vec<String> = std::env::args().skip(1).collect();
    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else {
            println!("{:<42} PARSE FAILED", short(p));
            continue;
        };
        n += 1;
        let ex = extract::extract_from_text(&text);
        let present: Vec<&str> = KINDS
            .iter()
            .enumerate()
            .filter_map(|(i, k)| {
                let here = ex.sections.iter().any(|s| s.kind == *k && !s.paragraphs.is_empty());
                if !here {
                    missing_kind[i] += 1;
                }
                here.then(|| kind_name(*k))
            })
            .collect();
        println!(
            "{:<42} {:>6} {:>6} {:>6} {:>6} | {}",
            short(p),
            ex.statistics.len(),
            ex.references.len(),
            ex.citations.len(),
            ex.table_mentions.len(),
            present.join(" ")
        );
        match &ex.title {
            None => {
                no_title += 1;
                println!("      title: NONE");
            }
            Some(t) => {
                let junk = t.len() > 200 || t.to_lowercase().contains("submission id");
                if junk {
                    junk_title += 1;
                }
                println!(
                    "      title: {}{}",
                    if junk { "NOT A TITLE -> " } else { "" },
                    t.chars().take(110).collect::<String>()
                );
            }
        }
        let lower = text.to_lowercase();
        for (name, list) in
            [("data-availability", DATA_AVAILABILITY), ("ethics", ETHICS), ("randomisation", RANDOMISATION)]
        {
            let hits: Vec<&&str> = list.iter().filter(|t| lower.contains(**t)).collect();
            println!(
                "      full-text {name}: {}",
                if hits.is_empty() { "NONE of the phrases".to_string() } else { format!("{hits:?}") }
            );
        }
        println!();
    }

    println!("=== MEDIATED FIELDS: how often EMPTY across {n} real manuscript(s) ===");
    for (i, k) in KINDS.iter().enumerate() {
        println!(
            "  section {:<12} missing in {} of {n}",
            kind_name(*k),
            missing_kind[i]
        );
    }
    println!("  title absent          {no_title} of {n}");
    println!("  title present but NOT a title  {junk_title} of {n}");
}

fn kind_name(k: SectionKind) -> &'static str {
    match k {
        SectionKind::Abstract => "Abstract",
        SectionKind::Introduction => "Introduction",
        SectionKind::Methods => "Methods",
        SectionKind::Results => "Results",
        SectionKind::Discussion => "Discussion",
        SectionKind::Conclusion => "Conclusion",
        SectionKind::References => "References",
        SectionKind::Other => "Other",
    }
}

fn short(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
