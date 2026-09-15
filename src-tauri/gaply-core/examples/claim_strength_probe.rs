//! **Claim–evidence strength and the causal-overclaim check, row by row.**
//!
//! Prints the design reading first — both term lists and the limiting sentence
//! whole — because every verdict below rests on it, and a verdict whose input is
//! not shown cannot be refuted.
//!
//! ```text
//! cargo run --release --example claim_strength_probe -- <manuscript>...
//! ```

use gaply_core::extract::{self, docparse};
use gaply_core::specialist::claim_strength::{assess_claims, read_design, ClaimStrengthSpecialist};
use gaply_core::specialist::{run, SpecialistInput};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: claim_strength_probe <manuscript>...");
        std::process::exit(2);
    }
    for p in &args {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else {
            println!("=== {} === PARSE FAILED\n", short(p));
            continue;
        };
        let ex = extract::extract_from_text(&text);
        let d = read_design(&ex);
        println!("=== {} ===", short(p));
        println!("  observational: {:?}", d.observational);
        println!("  experimental : {:?}", d.experimental);
        println!("  admits only association: {}", d.admits_only_association());
        if let Some(s) = &d.limiting_span {
            println!("  DESIGN SENTENCE ({:?}):\n    {s}", d.limiting_location);
        }
        let claims = assess_claims(&ex);
        println!("  {} causal sentence(s) in Abstract/Discussion/Conclusion", claims.len());
        for c in &claims {
            println!(
                "  [{:?} p{}] {}  ({} statistic(s) co-located)",
                c.location.section,
                c.location.paragraph,
                c.strength.as_str(),
                c.statistics_co_located
            );
            println!("      {}", c.claim_span);
        }
        let input = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let r = run(&ClaimStrengthSpecialist, &input);
        match &r.not_applicable {
            Some(reason) => println!("  SPECIALIST: NOT APPLICABLE — {reason}"),
            None => println!(
                "  SPECIALIST: {} admitted, {} rejected",
                r.admitted.len(),
                r.rejected.len()
            ),
        }
        for f in &r.admitted {
            println!("    - {} [{:?}]\n      {}", f.code, f.severity, f.summary);
            println!("      {}", f.span.clone().unwrap_or_default().replace('\n', "\n      "));
            println!("      unknown: {}", f.uncertainty.clone().unwrap_or_default());
        }
        println!();
    }
}

fn short(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
