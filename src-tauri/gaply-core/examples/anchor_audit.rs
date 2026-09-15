//! **What fraction of findings can be anchored into the manuscript?**
//!
//! Measured before the marked-up view is built. §9 [v5] already corrected the
//! inherited 97.6%: that was 80 of 82 sentences on ONE paper, PDF only, in the
//! thesis-audit lane over audit items. This measures PublishReady findings, on
//! 20 manuscripts, `.docx` and `.pdf`.
//!
//! **Anchoring is not one question but four**, because a `Location` that
//! resolves can still be wrong. `Location::paragraph` is an index WITHIN a
//! section, and `extract::paragraph_at` resolves a repeated `SectionKind` to
//! the FIRST section of that kind (§12.1). So:
//!
//! | tier | meaning |
//! |---|---|
//! | NONE | the finding carries no `Location` at all |
//! | UNRESOLVED | it has one and `paragraph_at` returns nothing |
//! | AMBIGUOUS | it resolves, but the manuscript has more than one section of that kind — the anchor may silently be the wrong one |
//! | ANCHORED | it resolves and the section kind occurs exactly once |
//!
//! Only the last is a locator a researcher can be shown.

use gaply_core::extract::{self, docparse, Location, SectionKind};
use gaply_core::journal_standards::Standard;
use gaply_core::report::{evaluate, StandardEvaluation};
use gaply_core::review_lens::{lenses, review, LensInput};
use gaply_core::specialist::{self, SpecialistInput};
use std::collections::BTreeMap;

#[derive(Default)]
struct Tally {
    none: usize,
    unresolved: usize,
    ambiguous: usize,
    anchored: usize,
}

impl Tally {
    fn total(&self) -> usize {
        self.none + self.unresolved + self.ambiguous + self.anchored
    }
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut all = Tally::default();
    let mut by_code: BTreeMap<String, Tally> = BTreeMap::new();
    let mut kind_repeats: BTreeMap<String, usize> = BTreeMap::new();
    let mut docs_pdf = Tally::default();
    let mut docs_docx = Tally::default();

    println!(
        "{:<44} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "manuscript", "finds", "NONE", "UNRES", "AMBIG", "ANCH"
    );
    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);

        // How many sections of each kind — the ambiguity denominator.
        let mut counts: BTreeMap<SectionKind, usize> = BTreeMap::new();
        for s in &ex.sections {
            *counts.entry(s.kind).or_default() += 1;
        }
        for (k, n) in &counts {
            if *n > 1 {
                *kind_repeats.entry(format!("{k:?}")).or_default() += 1;
            }
        }

        let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let reports: Vec<_> =
            specialist::shipped().iter().map(|s| specialist::run(s.as_ref(), &sin)).collect();
        let validity = gaply_core::validate::validate(&ex);
        let standards: Vec<StandardEvaluation> = [
            Standard::Consort, Standard::Prisma, Standard::Strobe,
            Standard::Arrive, Standard::Tripod,
        ]
        .iter()
        .map(|s| evaluate(*s, &ex, &text))
        .collect();
        let input = LensInput {
            extraction: &ex,
            full_text: Some(&text),
            this_year: 2026,
            specialists: Some(&reports),
            validity: Some(&validity),
            standards: Some(&standards),
            checklist: None,
            novelty: None,
        };

        let mut here = Tally::default();
        for l in lenses() {
            let r = review(&l, &input);
            for c in r.major_concerns.iter().chain(r.minor_concerns.iter()) {
                let t = by_code.entry(c.code.clone()).or_default();
                // A concern is anchored if its BEST location is. One concern can
                // carry 42; the marked-up view needs each of them, but the
                // question "can this finding be shown in the manuscript" turns
                // on whether any of them can.
                let verdict = classify(&ex, &counts, &c.locations);
                match verdict {
                    0 => { here.none += 1; t.none += 1; }
                    1 => { here.unresolved += 1; t.unresolved += 1; }
                    2 => { here.ambiguous += 1; t.ambiguous += 1; }
                    _ => { here.anchored += 1; t.anchored += 1; }
                }
            }
        }
        println!(
            "{:<44} {:>6} {:>6} {:>6} {:>6} {:>6}",
            short(p), here.total(), here.none, here.unresolved, here.ambiguous, here.anchored
        );
        let bucket = if p.to_lowercase().ends_with(".pdf") { &mut docs_pdf } else { &mut docs_docx };
        bucket.none += here.none;
        bucket.unresolved += here.unresolved;
        bucket.ambiguous += here.ambiguous;
        bucket.anchored += here.anchored;
        all.none += here.none;
        all.unresolved += here.unresolved;
        all.ambiguous += here.ambiguous;
        all.anchored += here.anchored;
    }

    let pc = |n: usize, d: usize| if d == 0 { 0.0 } else { n as f64 * 100.0 / d as f64 };
    let t = all.total();
    println!("\n=== ANCHORING over {t} finding(s) in {} manuscript(s) ===", paths.len());
    println!("  NONE        {:>4}  ({:.1}%)  no Location at all", all.none, pc(all.none, t));
    println!("  UNRESOLVED  {:>4}  ({:.1}%)  has one, resolves to nothing", all.unresolved, pc(all.unresolved, t));
    println!("  AMBIGUOUS   {:>4}  ({:.1}%)  resolves, but >1 section of that kind", all.ambiguous, pc(all.ambiguous, t));
    println!("  ANCHORED    {:>4}  ({:.1}%)  resolves and the kind is unique", all.anchored, pc(all.anchored, t));
    println!("\n  SHOWABLE IN THE MANUSCRIPT: {} of {t} ({:.1}%)", all.anchored, pc(all.anchored, t));

    println!("\n=== BY FORMAT ===");
    for (name, b) in [("pdf", &docs_pdf), ("docx", &docs_docx)] {
        println!(
            "  {name:<5} {:>4} finding(s): NONE {} · UNRES {} · AMBIG {} · ANCHORED {} ({:.1}%)",
            b.total(), b.none, b.unresolved, b.ambiguous, b.anchored, pc(b.anchored, b.total())
        );
    }

    println!("\n=== BY CODE ===");
    for (c, b) in &by_code {
        println!(
            "  {:>4} total  NONE {:>3} · UNRES {:>2} · AMBIG {:>2} · ANCHORED {:>3} ({:>5.1}%)  {c}",
            b.total(), b.none, b.unresolved, b.ambiguous, b.anchored, pc(b.anchored, b.total())
        );
    }

    println!("\n=== WHY AMBIGUOUS HAPPENS: section kinds occurring >1x ===");
    for (k, n) in &kind_repeats {
        println!("  {k:<14} repeated in {n} of {} manuscript(s)", paths.len());
    }
}

/// 0 = none, 1 = unresolved, 2 = ambiguous, 3 = anchored.
fn classify(
    ex: &extract::ExtractionResult,
    counts: &BTreeMap<SectionKind, usize>,
    locs: &[Location],
) -> u8 {
    if locs.is_empty() {
        return 0;
    }
    let mut best = 1u8;
    for l in locs {
        if extract::paragraph_at(ex, l).is_none() {
            continue;
        }
        let repeated = counts.get(&l.section).copied().unwrap_or(0) > 1;
        best = best.max(if repeated { 2 } else { 3 });
    }
    best
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
