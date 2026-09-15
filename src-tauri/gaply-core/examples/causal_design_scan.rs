//! **Does the causal-overclaim check have an input?** Measured before it is built.
//!
//! §12's Phase-4 table declines claim–evidence strength against the empty
//! claim↔analysis links. The causal-overclaim check the reviewer document names
//! needs two different things — a causal sentence and the design that limits it
//! — and both are manuscript text. This prints what is actually there.

use gaply_core::extract::{self, docparse, SectionKind};

// STRICT: the directional verbs of the first list ("increased", "reduced",
// "improved", "enhanced", "produced", "driven by") were measured on the corpus
// and describe direction, not causation — "Chloride increased toward the inlet
// stations" is a description. They are gone.
const CAUSAL: &[&str] = &[
    "causes", "caused", "causing", "cause of", "causal effect", "causally",
    "leads to", "led to", "lead to", "results in", "resulted in",
    "induces", "induced", "gives rise to", "brings about", "attributable to",
    "responsible for", "the result of",
];
// Sentences where the manuscript LIMITS itself. Never a finding — this is the
// author doing the right thing, and flagging it is the opposite of the check.
const DISCLAIMERS: &[&str] = &[
    "no causal", "not causal", "cannot establish caus", "can not establish caus",
    "does not imply caus", "do not imply caus", "cannot be inferred",
    "causality cannot", "no causal direction", "mere association",
    "to causal inference", "precludes causal", "limits causal",
];
// Idioms that contain a causal phrase and are not causal claims. Measured:
// "achieving state-of-the-art results in terms of 96.42% accuracy".
const NOT_CAUSAL_IDIOMS: &[&str] = &["results in terms of", "results in table", "results in figure"];
const OBSERVATIONAL: &[&str] = &[
    "cross-sectional", "cross sectional", "observational", "correlational",
    "survey", "questionnaire", "retrospective", "cohort", "case-control",
    "case control", "secondary data", "self-reported", "association", "associated with",
    "correlation", "correlated",
];
// "pre-test"/"post-test" were in the first list and matched a cross-sectional
// SURVEY paper, where they name questionnaire piloting rather than a design.
const EXPERIMENTAL: &[&str] = &[
    "randomised", "randomized", "randomly assigned", "randomly allocated",
    "controlled trial", "control group", "treatment group", "intervention group",
    "placebo", "double-blind", "single-blind", "blinded", "experimental group",
    "manipulated",
];

fn main() {
    for p in std::env::args().skip(1) {
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else {
            println!("=== {} === PARSE FAILED", short(&p));
            continue;
        };
        let ex = extract::extract_from_text(&text);
        let lower = text.to_lowercase();
        let obs: Vec<&&str> = OBSERVATIONAL.iter().filter(|t| lower.contains(**t)).collect();
        let exp: Vec<&&str> = EXPERIMENTAL.iter().filter(|t| lower.contains(**t)).collect();
        println!("=== {} ===", short(&p));
        println!("  observational markers: {obs:?}");
        println!("  experimental markers : {exp:?}");

        // Causal sentences in the conclusion-bearing sections only.
        let mut n = 0;
        for s in ex.sections.iter().filter(|s| {
            matches!(s.kind, SectionKind::Abstract | SectionKind::Discussion | SectionKind::Conclusion)
        }) {
            for para in &s.paragraphs {
                for sent in extract::sentence::sentences_in(para) {
                    let sl = sent.to_lowercase();
                    if DISCLAIMERS.iter().any(|d| sl.contains(*d)) {
                        println!("  [{:?}] DISCLAIMER (never a finding)\n      {}", s.kind, sent.trim());
                        continue;
                    }
                    if NOT_CAUSAL_IDIOMS.iter().any(|d| sl.contains(*d)) {
                        println!("  [{:?}] IDIOM, not causal\n      {}", s.kind, sent.trim());
                        continue;
                    }
                    if let Some(c) = CAUSAL.iter().find(|c| sl.contains(**c)) {
                        n += 1;
                        println!("  [{:?}] causal={c:?}\n      {}", s.kind, sent.trim());
                    }
                }
            }
        }
        println!("  causal sentences in Abstract/Discussion/Conclusion: {n}\n");
    }
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
