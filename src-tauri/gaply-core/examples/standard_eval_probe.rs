//! **A reporting standard evaluated against a real manuscript, row by row.**
//!
//! Prints every item's verdict with the paragraph it was decided from, whole.
//! The three statuses are deliberately distinguishable: `Met` carries its
//! evidence, `NotFound` means the engine looked and found nothing, and
//! `Unevaluable` means it could not look — rendering the third as the second
//! would invent a compliance failure.
//!
//! The coverage line is printed BEFORE the rows, because "3 of 4 met" means
//! nothing without "checks 4 of CONSORT's 25 published items" beside it.
//!
//! ```text
//! cargo run --release --example standard_eval_probe -- <manuscript> [STANDARD...]
//! ```

use gaply_core::extract::{self, docparse};
use gaply_core::journal_standards::Standard;
use gaply_core::report::{evaluate, ItemStatus};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: standard_eval_probe <manuscript> [STANDARD...]");
        std::process::exit(2);
    }
    let path = args.remove(0);
    let standards: Vec<Standard> = if args.is_empty() {
        vec![Standard::Consort, Standard::Prisma, Standard::Strobe, Standard::Arrive, Standard::Tripod]
    } else {
        args.iter().filter_map(|a| Standard::parse(a)).collect()
    };

    let text = docparse::parse_path(std::path::Path::new(&path)).expect("parse");
    let ex = extract::extract_from_text(&text);
    println!(
        "manuscript: {}\n  sections={} statistics={} references={}\n",
        std::path::Path::new(&path).file_name().unwrap().to_string_lossy(),
        ex.sections.len(),
        ex.statistics.len(),
        ex.references.len()
    );

    let mut any_met = false;
    for s in standards {
        let e = evaluate(s, &ex, &text);
        println!(
            "=== {} — {} | met {} · not found {} · unevaluable {}",
            s.as_str(),
            e.coverage_phrase(),
            e.met(),
            e.not_found(),
            e.verdicts.len() - e.met() - e.not_found()
        );
        for v in &e.verdicts {
            let mark = match v.status {
                ItemStatus::Met => "MET        ",
                ItemStatus::NotFound => "NOT FOUND  ",
                ItemStatus::Unevaluable => "UNEVALUABLE",
            };
            println!("  {mark} {:<4} {}", v.item, v.requirement);
            println!("              reads {:?} — {}", v.reads, v.detail);
            if let Some(span) = &v.evidence_span {
                println!("              span: {span}");
            }
            if v.status == ItemStatus::Met {
                any_met = true;
            }
        }
        println!();
    }

    // The known-good rule: a manuscript with 90 statistics and an Abstract must
    // meet something. A table of all-NOT-FOUND is the instrument, not the paper.
    assert!(any_met, "no item met on a manuscript with {} statistics", ex.statistics.len());
}
