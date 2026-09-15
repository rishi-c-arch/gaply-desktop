//! **What the scientific layer ACTUALLY YIELDS on real manuscripts.**
//!
//! §12.1 item 3 is about to be closed by a one-line graph change: the first
//! Phase-4 specialist declares `requires: [ScientificExtraction]`, so the
//! harness derives the layer. The cost was measured and fixed
//! (`examples/scientific_cost_probe.rs`, 30.4 s -> 196 ms). **What has never
//! been measured is what the layer RETURNS** — the cost probe printed counts
//! (`3/3/2/0`) and nothing else.
//!
//! Counts are what the journal phase got wrong four times in one afternoon: a
//! `word_limit = 12000` that was a translation price list, `1 conflicted fact`
//! that was six reporting standards. So this probe prints ROWS WITH THEIR
//! SPANS, whole and unclipped, and picks the row the pipeline itself would
//! surface — the FIRST of each kind in document order, which is what a
//! specialist taking `&[Claim]` reads first.
//!
//! Usage:
//! ```text
//! cargo run --release --example scientific_yield_probe -- <file>...
//! ```

use std::path::Path;

use gaply_core::extract::{self, docparse, paragraph_at, ExtractionResult, Location};
use gaply_core::scientific_model::{ScientificExtraction, SourceSpan};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: scientific_yield_probe <file>...");
        std::process::exit(2);
    }

    let mut totals = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut any_claim = false;

    for p in &paths {
        let text = match docparse::parse_path(Path::new(p)) {
            Ok(t) => t,
            Err(e) => {
                println!("\n=== {} ===\n  PARSE FAILED: {e}", short(p));
                continue;
            }
        };
        let r = extract::extract_from_text_with(&text, extract::ExtractOptions::with_scientific());
        let sci = match &r.scientific {
            Some(s) => s.clone(),
            None => {
                println!("\n=== {} ===\n  NO SCIENTIFIC LAYER — with_scientific() did not populate it", short(p));
                continue;
            }
        };
        println!("\n=== {} ===", short(p));
        println!(
            "  sections={} stats={} refs={} | claims={} variables={} methods={} datasets={} questions={} hypotheses={} contributions={} limitations={}",
            r.sections.len(),
            r.statistics.len(),
            r.references.len(),
            sci.claims.len(),
            sci.variables.len(),
            sci.methods.len(),
            sci.datasets.len(),
            sci.questions.len(),
            sci.hypotheses.len(),
            sci.contributions.len(),
            sci.limitations.len(),
        );
        totals.0 += sci.claims.len();
        totals.1 += sci.variables.len();
        totals.2 += sci.methods.len();
        totals.3 += sci.datasets.len();
        totals.4 += sci.questions.len();
        totals.5 += sci.hypotheses.len();
        totals.6 += sci.contributions.len();
        totals.7 += sci.limitations.len();
        if !sci.claims.is_empty() {
            any_claim = true;
        }

        print_rows(&r, &sci);
        audit(&r, &sci);
    }

    println!(
        "\nTOTAL claims={} variables={} methods={} datasets={} questions={} hypotheses={} contributions={} limitations={} over {} file(s)",
        totals.0, totals.1, totals.2, totals.3, totals.4, totals.5, totals.6, totals.7, paths.len()
    );

    // The known-good rule: a uniformly empty table is the signature of a broken
    // instrument, not of six empty manuscripts. `claims` returned non-zero on
    // all six in the cost probe, so zero here means this probe is wrong.
    assert!(
        any_claim,
        "not one claim across {} file(s) — the cost probe found claims on all six, \
         so this is the probe failing, not the corpus",
        paths.len()
    );
}

/// **The two numbers that decide whether a Phase-4 specialist can run at all.**
///
/// A specialist takes `&[Claim]` and must (a) quote the manuscript back to the
/// researcher and (b) know which analysis a claim rests on. (a) needs the span
/// to RESOLVE; (b) needs the cross-links to be populated. Both are fields on
/// the type, and a field that exists is not a field that is filled.
fn audit(r: &ExtractionResult, sci: &ScientificExtraction) {
    // A `SectionKind` that occurs TWICE makes every `Location` naming it
    // ambiguous: the passes build `paragraph` by `enumerate()` PER SECTION, so
    // the counter restarts, and `paragraph_at` resolves to the FIRST section of
    // that kind. An index from the second section either fails to resolve
    // (loud) or lands on the first section's paragraph of that number (SILENT,
    // and the wrong sentence). Counting both is the point: the loud number
    // understates the damage.
    let mut kind_count: std::collections::BTreeMap<_, usize> = Default::default();
    for sec in &r.sections {
        *kind_count.entry(sec.kind).or_default() += 1;
    }
    let ambiguous_kind =
        |k: &gaply_core::extract::SectionKind| kind_count.get(k).copied().unwrap_or(0) > 1;

    let mut span_total = 0usize;
    let mut span_bad = 0usize;
    let mut span_ambiguous = 0usize;
    let mut check = |s: &SourceSpan| {
        span_total += 1;
        let kind = match s {
            SourceSpan::Point(loc) => loc.section,
            SourceSpan::Range(sp) => sp.section,
        };
        if ambiguous_kind(&kind) {
            span_ambiguous += 1;
        }
        let ok = match s {
            SourceSpan::Point(loc) => paragraph_at(r, loc).is_some(),
            SourceSpan::Range(sp) => (sp.start_paragraph..=sp.end_paragraph).all(|p| {
                paragraph_at(r, &Location { section: sp.section, paragraph: p }).is_some()
            }),
        };
        if !ok {
            span_bad += 1;
        }
    };
    for c in &sci.claims {
        check(&c.source_span);
    }
    for v in &sci.variables {
        v.mentions.iter().for_each(&mut check);
    }
    for m in &sci.methods {
        m.source_spans.iter().for_each(&mut check);
    }
    for d in &sci.datasets {
        d.source_spans.iter().for_each(&mut check);
    }

    let claims_with_var = sci.claims.iter().filter(|c| !c.variables.is_empty()).count();
    let claims_with_method = sci.claims.iter().filter(|c| !c.methods.is_empty()).count();
    let claims_with_stat = sci.claims.iter().filter(|c| !c.statistics.is_empty()).count();
    let claims_with_dataset = sci.claims.iter().filter(|c| !c.datasets.is_empty()).count();
    let vars_with_claim = sci.variables.iter().filter(|v| !v.associated_claims.is_empty()).count();
    let methods_with_claim = sci.methods.iter().filter(|m| !m.associated_claims.is_empty()).count();
    let methods_typed = sci
        .methods
        .iter()
        .filter(|m| !matches!(m.design, gaply_core::scientific_model::StudyDesign::Other(_)))
        .count();
    // A claim whose statement opens mid-sentence is a fragment: the extractor
    // cut at its trigger word and kept the tail. A specialist reasoning over
    // "higher than the untreated control." has no subject to reason about.
    let fragments = sci
        .claims
        .iter()
        .filter(|c| {
            c.statement
                .chars()
                .next()
                .is_some_and(|ch| ch.is_lowercase())
        })
        .count();

    println!(
        "  AUDIT spans {}/{} unresolved, {} ambiguous-kind | claims->var {}/{} ->method {}/{} ->stat {}/{} ->dataset {}/{} | var->claim {}/{} | method->claim {}/{} | method design typed {}/{} | claim fragments {}/{}",
        span_bad,
        span_total,
        span_ambiguous,
        claims_with_var,
        sci.claims.len(),
        claims_with_method,
        sci.claims.len(),
        claims_with_stat,
        sci.claims.len(),
        claims_with_dataset,
        sci.claims.len(),
        vars_with_claim,
        sci.variables.len(),
        methods_with_claim,
        sci.methods.len(),
        methods_typed,
        sci.methods.len(),
        fragments,
        sci.claims.len(),
    );
}

fn print_rows(r: &ExtractionResult, sci: &ScientificExtraction) {
    // FIRST of each kind in document order — the row a specialist reading
    // `&[Claim]` meets first, not a row chosen for looking good.
    for (i, c) in sci.claims.iter().take(3).enumerate() {
        println!(
            "  claim[{i}] {:?}/{:?} conf={:.2} stats={} cites={} vars={} methods={}",
            c.category,
            c.nature,
            c.confidence,
            c.statistics.len(),
            c.citations.len(),
            c.variables.len(),
            c.methods.len()
        );
        println!("    statement: {}", c.statement);
        println!("    span     : {}", span_text(r, &c.source_span));
    }
    for (i, v) in sci.variables.iter().take(3).enumerate() {
        println!(
            "  var[{i}] {:?} role={:?} level={:?} units={:?} claims={} methods={}",
            v.canonical_name,
            v.role,
            v.measurement,
            v.units,
            v.associated_claims.len(),
            v.associated_methods.len()
        );
        if let Some(s) = v.mentions.first() {
            println!("    span     : {}", span_text(r, s));
        }
    }
    for (i, m) in sci.methods.iter().take(3).enumerate() {
        println!(
            "  method[{i}] design={:?} name={:?} n={:?} software={:?} tests={:?}",
            m.design,
            m.name,
            m.sampling.size,
            m.software,
            m.statistical_tests.iter().map(|t| t.name.as_str()).collect::<Vec<_>>()
        );
        if let Some(s) = m.source_spans.first() {
            println!("    span     : {}", span_text(r, s));
        }
    }
    for (i, d) in sci.datasets.iter().take(3).enumerate() {
        println!(
            "  dataset[{i}] name={:?} n={:?} source={:?} population={:?}",
            d.name, d.sample_size, d.source, d.population
        );
        if let Some(s) = d.source_spans.first() {
            println!("    span     : {}", span_text(r, s));
        }
    }
}

/// Resolve a span to the manuscript text it quotes. **Whole, never clipped** —
/// a truncated span is not a span (§11): the journal checklist's
/// data-availability item read as unsupported boilerplate purely because the
/// probe printing it cut at 150 chars.
fn span_text(r: &ExtractionResult, s: &SourceSpan) -> String {
    match s {
        SourceSpan::Point(loc) => match paragraph_at(r, loc) {
            Some(t) => format!("[{:?} ¶{}] {t}", loc.section, loc.paragraph),
            None => format!("[{:?} ¶{}] <UNRESOLVED>", loc.section, loc.paragraph),
        },
        SourceSpan::Range(sp) => {
            let mut out = format!("[{:?} ¶{}..={}] ", sp.section, sp.start_paragraph, sp.end_paragraph);
            for p in sp.start_paragraph..=sp.end_paragraph {
                let loc = Location { section: sp.section, paragraph: p };
                match paragraph_at(r, &loc) {
                    Some(t) => {
                        out.push_str(t);
                        out.push(' ');
                    }
                    None => out.push_str("<UNRESOLVED> "),
                }
            }
            out
        }
    }
}

fn short(p: &str) -> String {
    Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.into())
}
