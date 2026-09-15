//! **What fraction of the claim extractor's claims can claim-evidence strength
//! classify?** Measured BEFORE the report shape is built around it.
//!
//! §4.6 scopes claim–evidence strength to *"every major claim"*, and §12's
//! Phase-4 table gives its input as the claim↔analysis links. This asks the
//! narrower question the declines leave: given the claims the extractor
//! actually produces, how many can this check say anything about?
//!
//! A claim is CLASSIFIABLE here only if all of:
//!   * it is a whole sentence, not a fragment (`claims.rs` slices at its cue);
//!   * it sits in a concluding section — Abstract, Discussion or Conclusion;
//!   * it asserts causation, which is what the check reasons about;
//!   * the manuscript states a design to weigh it against.
//!
//! Each is counted separately so the fraction has a cause, not just a value.

use gaply_core::extract::{self, docparse, SectionKind};
use gaply_core::specialist::claim_strength::read_design;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut total, mut whole, mut concluding, mut causal, mut with_design) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut docs_with_design = 0usize;

    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text_with(
            &text,
            extract::ExtractOptions::with_scientific(),
        );
        let design = read_design(&ex);
        let has_design = !design.observational.is_empty() || !design.experimental.is_empty();
        if has_design {
            docs_with_design += 1;
        }
        let claims = ex.scientific.as_ref().map(|s| s.claims.clone()).unwrap_or_default();

        let (mut w, mut cc, mut ca) = (0usize, 0usize, 0usize);
        for c in &claims {
            total += 1;
            let t = c.statement.trim();
            // A whole sentence: starts capitalised and ends in a terminator.
            let is_whole = t.chars().next().map(|x| x.is_uppercase()).unwrap_or(false)
                && t.ends_with(['.', '!', '?']);
            if is_whole {
                whole += 1;
                w += 1;
            }
            let section = match &c.source_span {
                gaply_core::scientific_model::SourceSpan::Point(l) => l.section,
                gaply_core::scientific_model::SourceSpan::Range(r) => r.section,
            };
            let in_concluding = matches!(
                section,
                SectionKind::Abstract | SectionKind::Discussion | SectionKind::Conclusion
            );
            if is_whole && in_concluding {
                concluding += 1;
                cc += 1;
            }
            let lower = t.to_lowercase();
            let asserts_cause = gaply_core::specialist::claim_strength::CAUSAL_PHRASES
                .iter()
                .any(|x| lower.contains(*x));
            if is_whole && in_concluding && asserts_cause {
                causal += 1;
                ca += 1;
                if has_design {
                    with_design += 1;
                }
            }
        }
        println!(
            "{:<46} claims={:<4} whole={:<4} +concluding={:<3} +causal={:<3} design={}",
            short(p), claims.len(), w, cc, ca, has_design
        );
    }

    println!("\n=== CLASSIFIABLE FRACTION of {total} claim(s) over {} manuscript(s) ===", paths.len());
    let pct = |n: usize| if total == 0 { 0.0 } else { n as f64 * 100.0 / total as f64 };
    println!("  whole sentences                 {whole:>4}  ({:.1}%)", pct(whole));
    println!("  + in a concluding section       {concluding:>4}  ({:.1}%)", pct(concluding));
    println!("  + asserting causation           {causal:>4}  ({:.1}%)", pct(causal));
    println!("  + with a design to weigh against{with_design:>4}  ({:.1}%)", pct(with_design));
    println!("\n  manuscripts stating a design: {docs_with_design} of {}", paths.len());
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
