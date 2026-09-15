//! **Is a missing section a missing SECTION, or a missing heading?**
//!
//! `report::evaluate`'s `AbstractPresent` and `ResultsSectionPresent` return
//! `NotFound` when the heading classifier produced no section of that kind, and
//! the reviewer lens turns `NotFound` into a MAJOR concern. §4.5 [v8] classes
//! `sections` as MEDIATED. This checks the two against the full text.

use gaply_core::extract::{self, docparse, SectionKind};

fn main() {
    let (mut n, mut abs_missing, mut abs_but_present, mut res_missing, mut res_but_present) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for p in std::env::args().skip(1) {
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        n += 1;
        let lower = text.to_lowercase();
        for (kind, words, miss, contra) in [
            (SectionKind::Abstract, ["abstract", "summary"], &mut abs_missing, &mut abs_but_present),
            (SectionKind::Results, ["results", "findings"], &mut res_missing, &mut res_but_present),
        ] {
            let classified =
                ex.sections.iter().any(|s| s.kind == kind && !s.paragraphs.is_empty());
            if classified {
                continue;
            }
            *miss += 1;
            // The word appears in the manuscript at the start of a line — i.e.
            // the content is almost certainly there and the classifier missed it.
            let looks_present = text.lines().any(|l| {
                let t = l.trim().to_lowercase();
                words.iter().any(|w| t == *w || t.starts_with(&format!("{w}:"))
                    || (t.len() < 40 && t.contains(w) && !t.ends_with('.')))
            });
            if looks_present {
                *contra += 1;
                let example = text
                    .lines()
                    .find(|l| {
                        let t = l.trim().to_lowercase();
                        words.iter().any(|w| t.len() < 40 && t.contains(w) && !t.ends_with('.'))
                    })
                    .unwrap_or("")
                    .trim();
                println!(
                    "{:<44} {kind:?} NotFound — but the text has a line: {:?}",
                    short(&p), example
                );
            } else {
                println!("{:<44} {kind:?} NotFound — and nothing in the text contradicts it", short(&p));
            }
        }
    }
    println!("\n=== over {n} manuscript(s) ===");
    println!("  Abstract NotFound: {abs_missing}, of which the text contradicts: {abs_but_present}");
    println!("  Results  NotFound: {res_missing}, of which the text contradicts: {res_but_present}");
    println!(
        "\n  {} of {} section-absence verdicts are contradicted by the manuscript itself",
        abs_but_present + res_but_present,
        abs_missing + res_missing
    );
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
