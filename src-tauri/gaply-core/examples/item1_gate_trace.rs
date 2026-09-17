//! **Item 1: where does `claim_evidence_strength` stop, and why?**
//!
//! The specialist has four gates in series: `applies_to` (a design is read at
//! all), `admits_only_association` (observational non-empty AND experimental
//! EMPTY), `limiting_span` present (set only by the observational loop), and a
//! causal sentence in a concluding section. This prints which gate each
//! manuscript dies at, so a decline rests on the stage that stops it rather than
//! on a total of zero.
use gaply_core::extract::{self, docparse};
use gaply_core::specialist::claim_strength::{read_design, ClaimStrengthSpecialist};
use gaply_core::specialist::{self, Specialist, SpecialistInput};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut stop: std::collections::BTreeMap<&str, usize> = Default::default();
    println!("{:<44} {:<22} {}", "manuscript", "stopped at", "obs / exp");
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let d = read_design(&ex);
        let input = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let sp = ClaimStrengthSpecialist;

        // The label must name WHICH decline, or the probe reports 18 identical
        // rows for four different reasons — the uniform-result tell, self-inflicted.
        let where_ = if let Some(reason) = sp.applies_to(&input) {
            if reason.contains("no study design") {
                "declined: no design"
            } else if reason.contains("experimental design (") {
                "declined: experimental"
            } else if reason.contains("cannot tell which") {
                "declined: both designs"
            } else {
                "declined: no span"
            }
        } else {
            let r = specialist::run(&sp, &input);
            if r.admitted.is_empty() { "4 no causal sentence" } else { "FIRED" }
        };
        *stop.entry(where_).or_default() += 1;
        println!(
            "{:<44} {:<22} {:?} / {:?}",
            name.chars().take(44).collect::<String>(),
            where_,
            d.observational,
            d.experimental
        );
    }
    println!("\n  where the check stops, over {} manuscripts:", paths.len());
    for (k, v) in &stop {
        println!("    {k:<24} {v}");
    }
}
