//! **What each lens declares and what exists — §4.5 [v8].**
//!
//! Static: a property of the source graph, not of any manuscript. The version
//! of this probe that ran before the §4.5 correction pointed every lens at
//! specialist output, and found the emptiness there: seven of eleven
//! specialists declined. A lens now reads the research state and the
//! fingerprint directly, so this prints, for each field, whether an ABSENCE in
//! it may be concluded from — which is the only thing standing between a lens
//! and §11 D165.
//!
//! ```text
//! cargo run --release --example lens_input_probe
//! ```

use gaply_core::review_lens::{
    availability, lenses, FieldReach, LensState, SourceState, StateField, DECLINED_LENSES,
};

fn main() {
    println!("=== EVERY SOURCE A LENS CAN READ, AND WHETHER IT EXISTS ===\n");
    let mut seen = Vec::new();
    for l in lenses() {
        for s in l.reads() {
            if !seen.contains(&s) {
                seen.push(s);
            }
        }
    }
    seen.sort();
    for s in &seen {
        println!("  {:<28} {:?}", s.as_str(), s.state());
        println!("      {}", s.note());
    }

    println!("\n=== EVERY RESEARCH-STATE FIELD A LENS MAY READ ===");
    println!("  (an absence may be concluded from an Exhaustive field only)\n");
    for f in [
        StateField::FullText,
        StateField::ReferenceYears,
        StateField::Statistics,
        StateField::References,
        StateField::Sections,
        StateField::Title,
        StateField::Tables,
        StateField::JournalRequirements,
        StateField::JournalStandardBindings,
    ] {
        println!(
            "  {:<28} {:?}{}",
            f.as_str(),
            f.reach(),
            match (f.reach(), f) {
                (FieldReach::Exhaustive, StateField::FullText) => "  <- absence IS a finding",
                (FieldReach::Exhaustive, _) => "  <- asks no absence question",
                _ => "  <- absence is about the EXTRACTOR; no finding may rest on it",
            }
        );
        println!("      {}", f.absence_note());
    }

    println!("\n=== THE FOUR SHIPPING LENSES ===");
    for a in availability() {
        println!("\n--- {} : {:?}", a.lens.as_str(), a.state);
        println!("  reads:");
        for (s, st) in &a.reads {
            println!("    {:<28} {:?}", s.as_str(), st);
        }
        if !a.sourced_criteria.is_empty() {
            println!("  criteria WITH a shipping source ({}):", a.sourced_criteria.len());
            for c in &a.sourced_criteria {
                println!("    + {c}");
            }
        }
        if !a.unsourced_criteria.is_empty() {
            println!("  criteria WITHOUT one ({}):", a.unsourced_criteria.len());
            for (c, why) in &a.unsourced_criteria {
                println!("    - {c}\n        {why}");
            }
        }
    }

    println!("\n=== DECLINED, AND FOR DIFFERENT REASONS ===");
    for d in DECLINED_LENSES {
        println!("\n--- {} : DECLINED", d.id.as_str());
        println!("  {}", d.reason);
        for c in d.criteria {
            println!(
                "    would have evaluated: {} (reads {})",
                c.name,
                c.sources.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
    }

    println!("\n=== SUMMARY ===");
    for a in availability() {
        let verdict = match a.state {
            LensState::Runs => "RUNS",
            LensState::RunsPartial => "RUNS, PARTIAL",
            LensState::NoInputs => "CANNOT RUN — every source absent",
        };
        println!(
            "  {:<20} {:<32} {} of {} criteria sourced",
            a.lens.as_str(),
            verdict,
            a.sourced_criteria.len(),
            a.sourced_criteria.len() + a.unsourced_criteria.len()
        );
    }
    let declined = seen.iter().filter(|s| s.state() == SourceState::Declined).count();
    let shipping = seen.iter().filter(|s| s.state() == SourceState::Shipping).count();
    println!(
        "\n  {shipping} shipping source(s), {declined} declined, {} with no data path or \
         unbuilt.",
        seen.len() - shipping - declined
    );
}
