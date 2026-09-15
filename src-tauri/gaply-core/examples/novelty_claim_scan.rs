//! **Novelty claims on the real corpus, whole-sentence, with the comparison
//! against `claims.rs` that justifies not routing through it.**
//!
//! Rows, not totals. Every claim prints its whole sentence and its retrieval
//! terms, because the terms are what a search would be built from and a bad
//! term list is invisible in a count.
//!
//! ```text
//! cargo run --release --example novelty_claim_scan -- <manuscript>...
//! ```

use gaply_core::extract::{self, docparse};
use gaply_core::novelty;
use gaply_core::scientific_model::ClaimCategory;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: novelty_claim_scan <manuscript>...");
        std::process::exit(2);
    }

    let mut whole = 0usize;
    let mut legacy = 0usize;
    let mut legacy_fragments = 0usize;

    for p in &args {
        let text = match docparse::parse_path(std::path::Path::new(p)) {
            Ok(t) => t,
            Err(e) => {
                println!("=== {} ===\n  PARSE FAILED: {e}\n", short(p));
                continue;
            }
        };
        let ex = extract::extract_from_text_with(&text, extract::ExtractOptions::with_scientific());
        let claims = novelty::extract_claims(&ex);

        // What `claims.rs` produces for the same category, for comparison.
        let legacy_claims: Vec<_> = ex
            .scientific
            .as_ref()
            .map(|s| {
                s.claims
                    .iter()
                    .filter(|c| c.category == ClaimCategory::NoveltyClaim)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        println!("=== {} ===", short(p));
        println!(
            "  novelty::extract_claims -> {}   claims.rs NoveltyClaim -> {}",
            claims.len(),
            legacy_claims.len()
        );
        for c in &claims {
            println!("  [{}] cue={:?}", fmt_loc(&c.location), c.cue);
            println!("      {}", c.sentence);
            println!("      terms: {:?}", c.terms);
        }
        for l in &legacy_claims {
            // A fragment is a statement that does not start with a capital or
            // does not end in a terminator — the shape §4.2 measured at 89%.
            let s = l.statement.trim();
            let frag = !s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                || !s.ends_with(['.', '!', '?']);
            if frag {
                legacy_fragments += 1;
            }
            println!("      claims.rs{}: {}", if frag { " FRAGMENT" } else { "" }, s);
        }
        println!();
        whole += claims.len();
        legacy += legacy_claims.len();
    }

    println!(
        "TOTAL novelty::extract_claims={whole}  claims.rs={legacy} ({legacy_fragments} fragments)"
    );
}

fn fmt_loc(l: &extract::Location) -> String {
    format!("{:?} p{}", l.section, l.paragraph)
}

fn short(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}
