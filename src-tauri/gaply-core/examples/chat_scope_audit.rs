//! **Can §10's four modes be answered from the verdict layer?** Measured
//! against a real stored run before any chat wiring is built.
//!
//! §10's rule: *"Every number comes from a query. Counts, locators, severities
//! are retrieved, never generated. The model phrases; it does not compute."*
//! So for each mode, for each finding, the question is whether the ANSWER is
//! already a field of what the run stored — not whether a model could produce
//! something plausible.
//!
//! The four modes: **explain** (why was this flagged), **evidence** (show me),
//! **correction** (what would fix it — mechanical findings only), **challenge**
//! (I disagree → decision ledger → §5.5 re-runs the affected subgraph).

use gaply_core::editor::{decide, EditorInput};
use gaply_core::exports::{annotate, audit_trail, letter};
use gaply_core::extract::{self, docparse};
use gaply_core::journal_standards::Standard;
use gaply_core::report::{evaluate, StandardEvaluation};
use gaply_core::review_lens::{lenses, review, LensInput, ReviewerReport};
use gaply_core::specialist::{self, SpecialistInput};

/// Which stored field answers a mode, or what is missing.
struct Verdict {
    answerable: bool,
    by: &'static str,
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut ge, mut gv, mut gc, mut gch, mut gn) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut missing_trail: Vec<String> = Vec::new();
    let mut missing_span: Vec<String> = Vec::new();
    for path in &paths {
    let Ok(text) = docparse::parse_path(std::path::Path::new(path)) else { continue };
    let ex = extract::extract_from_text(&text);

    let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
    let specs: Vec<_> =
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
        specialists: Some(&specs),
        validity: Some(&validity),
        standards: Some(&standards),
        checklist: None,
        novelty: None,
    };
    let reports: Vec<ReviewerReport> = lenses().iter().map(|l| review(l, &input)).collect();
    let rubric = decide(&EditorInput { reports: &reports, journal: None });
    let ann = annotate(&ex, &reports);
    let trail = audit_trail(&reports, &rubric, &ann);
    let _letter = letter(&reports, &rubric);

    let concerns: Vec<_> = reports
        .iter()
        .flat_map(|r| r.major_concerns.iter().chain(r.minor_concerns.iter()))
        .collect();

    println!("MANUSCRIPT: {}", short(path));
    println!("  stored run: {} finding(s), {} annotation(s), audit trail v{}",
        concerns.len(), ann.annotations.len(), trail.schema_version);

    let (mut e_ok, mut v_ok, mut c_ok, mut ch_ok) = (0, 0, 0, 0);
    for c in &concerns {
        // EXPLAIN — "why was this flagged". Needs a summary, a severity with a
        // reason, and the trail that produced it.
        let explain = Verdict {
            answerable: !c.summary.is_empty()
                && c.trail.iter().any(|t| t.stage == "severity")
                && c.uncertainty.is_some(),
            by: "summary + trail[severity] + uncertainty",
        };
        // EVIDENCE — "show me". Needs the manuscript's own words, stored.
        let evidence = Verdict {
            answerable: !c.spans.is_empty(),
            by: "spans (whole, stored)",
        };
        // CORRECTION — "what would fix it", mechanical findings ONLY. §9 lists
        // a `suggested edit` for mechanical findings; nothing stores one.
        // `required_revision` is the reviewer DOCUMENT's generic action text,
        // not a fix for THIS manuscript, so it does not answer the question.
        let correction = Verdict { answerable: false, by: "nothing stores a suggested edit" };
        // CHALLENGE — needs a Decision row and §5.5's re-run of the affected
        // subgraph.
        let challenge = Verdict { answerable: false, by: "no decision ledger, no graph-driven re-run" };

        if explain.answerable { e_ok += 1; }
        if evidence.answerable { v_ok += 1; }
        if correction.answerable { c_ok += 1; }
        if challenge.answerable { ch_ok += 1; }

        if !explain.answerable { missing_trail.push(c.code.clone()); }
        if !evidence.answerable { missing_span.push(c.code.clone()); }
        let _ = (explain.by, evidence.by, correction.by, challenge.by);
    }
    gn += n_here(&concerns);
    ge += e_ok; gv += v_ok; gc += c_ok; gch += ch_ok;
    }
    let (n, e_ok, v_ok, c_ok, ch_ok) = (gn, ge, gv, gc, gch);

    println!("\n=== ANSWERABLE FROM THE VERDICT LAYER, over {n} finding(s) ===");
    println!("  EXPLAIN     {e_ok:>3} of {n}   summary + trail[severity] + uncertainty");
    println!("  EVIDENCE    {v_ok:>3} of {n}   spans, stored whole");
    println!("  CORRECTION  {c_ok:>3} of {n}   nothing stores a suggested edit (§9 lists one; it was not built)");
    println!("  CHALLENGE   {ch_ok:>3} of {n}   no Decision row, and §5.5's re-run needs the graph to DRIVE the lanes (§12.1 item 2)");

    println!(
        "\n  What a challenge would need — a `Decision` type, a decisions table, a \
         suggested-edit field, revision history — is checked by grep over the tree, not \
         asserted here. A probe that hardcodes its own answer is not a measurement."
    );
    if !missing_trail.is_empty() {
        println!("\n  findings with no severity trail or no uncertainty: {missing_trail:?}");
    }
    if !missing_span.is_empty() {
        println!("  findings with no span: {missing_span:?}");
    }
}

fn n_here(c: &[&gaply_core::review_lens::Concern]) -> usize { c.len() }
fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
