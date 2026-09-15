//! **Does claim–evidence strength have an input?** Measured before the report
//! shape is chosen, the way §11 D166 measured novelty.
//!
//! §4.6 scopes it to *"every major claim"*, traced *"claim → analysis → result
//! → conclusion"*. Three questions, each answered over the same 20 real
//! manuscripts:
//!
//! 1. How many claims does the extractor produce, and how many can be TRACED to
//!    a result through the evidence graph?
//! 2. What strength would each claim be assigned — how many land `UNVERIFIED`?
//! 3. For the causal-overclaim check, which needs BOTH halves: how many
//!    manuscripts state a design at all, and how many pair an association-only
//!    design with a causal conclusion?

use gaply_core::extract::{self, docparse, SectionKind};
use gaply_core::research_state::{EdgeKind, NodeRef, ResearchState};
use gaply_core::specialist::claim_strength::{read_design, CAUSAL_PHRASES};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let n = paths.len();
    let (mut claims_total, mut traced, mut whole, mut concluding) = (0usize, 0usize, 0usize, 0usize);
    let (mut d_none, mut d_obs_only, mut d_exp, mut causal_docs) = (0usize, 0usize, 0usize, 0usize);
    let mut both_halves = 0usize;
    let mut causal_sentences = 0usize;

    println!(
        "{:<46} {:>6} {:>7} {:>6} {:>6}  design",
        "manuscript", "claims", "traced", "whole", "concl"
    );
    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text_with(
            &text,
            extract::ExtractOptions::with_scientific(),
        );
        let state = ResearchState::from_extraction(&ex);
        let claims = ex.scientific.as_ref().map(|s| s.claims.clone()).unwrap_or_default();

        // TRACED = the evidence graph links this claim to a statistic. The only
        // claim->result edge the graph emits, and it is CO-LOCATION by
        // construction: Phase 1 refused to assert `Supports`.
        let linked: std::collections::BTreeSet<String> = state
            .evidence
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ClaimCoLocatedWithStatistic)
            .filter_map(|e| match &e.from {
                NodeRef::Claim { id } => Some(id.clone()),
                _ => match &e.to {
                    NodeRef::Claim { id } => Some(id.clone()),
                    _ => None,
                },
            })
            .collect();

        let (mut w, mut c) = (0usize, 0usize);
        for cl in &claims {
            claims_total += 1;
            if linked.contains(&cl.id.0) {
                traced += 1;
            }
            let t = cl.statement.trim();
            if t.chars().next().map(|x| x.is_uppercase()).unwrap_or(false)
                && t.ends_with(['.', '!', '?'])
            {
                whole += 1;
                w += 1;
                let sect = match &cl.source_span {
                    gaply_core::scientific_model::SourceSpan::Point(l) => l.section,
                    gaply_core::scientific_model::SourceSpan::Range(r) => r.section,
                };
                if matches!(
                    sect,
                    SectionKind::Abstract | SectionKind::Discussion | SectionKind::Conclusion
                ) {
                    concluding += 1;
                    c += 1;
                }
            }
        }

        let d = read_design(&ex);
        let (has_obs, has_exp) = (!d.observational.is_empty(), !d.experimental.is_empty());
        let design = match (has_obs, has_exp) {
            (false, false) => {
                d_none += 1;
                "NONE".to_string()
            }
            (true, false) => {
                d_obs_only += 1;
                format!("association-only {:?}", d.observational)
            }
            _ => {
                d_exp += 1;
                "admits causation".to_string()
            }
        };

        // Causal conclusions, counted the way the shipped check counts them.
        let mut here_causal = 0usize;
        for s in ex.sections.iter().filter(|s| {
            matches!(
                s.kind,
                SectionKind::Abstract | SectionKind::Discussion | SectionKind::Conclusion
            )
        }) {
            for para in &s.paragraphs {
                for sent in extract::sentence::sentences_in(para) {
                    let sl = sent.to_lowercase();
                    if CAUSAL_PHRASES.iter().any(|x| sl.contains(*x)) {
                        here_causal += 1;
                    }
                }
            }
        }
        causal_sentences += here_causal;
        if here_causal > 0 {
            causal_docs += 1;
        }
        if has_obs && !has_exp && here_causal > 0 {
            both_halves += 1;
        }

        println!(
            "{:<46} {:>6} {:>7} {:>6} {:>6}  {design}",
            short(p), claims.len(), linked.len(), w, c
        );
    }

    let pct = |x: usize| if claims_total == 0 { 0.0 } else { x as f64 * 100.0 / claims_total as f64 };
    println!("\n=== 1. TRACING, over {claims_total} claim(s) in {n} manuscript(s) ===");
    println!("  traced to a result (claim<->statistic edge) {traced:>5}  ({:.1}%)", pct(traced));
    println!("  whole sentences                             {whole:>5}  ({:.1}%)", pct(whole));
    println!("  whole AND in a concluding section           {concluding:>5}  ({:.1}%)", pct(concluding));

    println!("\n=== 2. STRENGTH each claim would receive ===");
    println!("  SUPPORTED / CONTRADICTED   0 — unreachable: the graph emits CO-LOCATION,");
    println!("                                 and Phase 1 refused to assert meaning");
    println!("  PARTIALLY_SUPPORTED    {traced:>5}  ({:.1}%) — a statistic sits with the claim", pct(traced));
    println!(
        "  UNVERIFIED             {:>5}  ({:.1}%) — everything else",
        claims_total - traced,
        pct(claims_total - traced)
    );

    println!("\n=== 3. THE CAUSAL-OVERCLAIM CHECK NEEDS BOTH HALVES ===");
    println!("  manuscripts stating NO design                {d_none} of {n}");
    println!("  stating an association-only design           {d_obs_only} of {n}");
    println!("  stating a design that admits causation       {d_exp} of {n}");
    println!("  with >=1 causal conclusion sentence          {causal_docs} of {n}  ({causal_sentences} sentence(s))");
    println!("  **BOTH HALVES — association-only AND causal** {both_halves} of {n}");
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
