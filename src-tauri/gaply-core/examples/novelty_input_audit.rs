//! **Does the novelty pipeline have an input at all?** The measurement that
//! decides whether §4.6 is a feature or a design assumption.
//!
//! Four questions, each answered over the six real manuscripts:
//!
//! 1. How many claims does the scientific layer produce, and how many land in
//!    `ClaimCategory::NoveltyClaim`?
//! 2. How many manuscripts contain the LITERAL phrasings §4.6 names —
//!    *"first to demonstrate"*, *"no prior study has"*?
//! 3. How many candidate sentences does a cue scan find per paper?
//! 4. Of those candidates, how many are section HEADINGS rather than prose?

use gaply_core::extract::{self, docparse};
use gaply_core::scientific_model::ClaimCategory;
use std::collections::BTreeMap;

/// The phrasings §4.6 itself names, verbatim from the architecture document:
/// *"first to demonstrate X", "no prior study has…"*.
const SECTION_4_6_PHRASINGS: &[&str] =
    &["first to demonstrate", "no prior study has", "no prior study", "first to show"];

/// Every cue the extractor knows, for the candidate count.
const CUES: &[&str] = &[
    "to the best of our knowledge", "to our knowledge", "to the authors' knowledge",
    "for the first time", "this is the first", "the first study", "the first report",
    "the first to", "first attempt to", "no prior study", "no previous study",
    "no prior work", "no previous work", "has not been reported", "have not been reported",
    "has not previously been", "have not previously been", "has never been",
    "have never been", "remains unexplored", "remain unexplored", "little is known",
    "fills a gap", "fill this gap", "address this gap", "novel approach", "novel method",
    "novel framework", "novel finding", "unprecedented", "hitherto",
];

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let n_docs = paths.len();
    let mut by_cat: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_claims = 0usize;
    let mut docs_with_4_6_phrasing = 0usize;
    let (mut cand_total, mut cand_headings) = (0usize, 0usize);
    let mut sentences_total = 0usize;

    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else {
            println!("{:<46} PARSE FAILED", short(p));
            continue;
        };
        let ex = extract::extract_from_text_with(&text, extract::ExtractOptions::with_scientific());
        let claims = ex.scientific.as_ref().map(|s| s.claims.clone()).unwrap_or_default();
        total_claims += claims.len();
        let mut novelty_here = 0usize;
        for c in &claims {
            let k = format!("{:?}", c.category);
            *by_cat.entry(k).or_default() += 1;
            if c.category == ClaimCategory::NoveltyClaim {
                novelty_here += 1;
            }
        }

        let lower = text.to_lowercase();
        let hits_4_6: Vec<&&str> =
            SECTION_4_6_PHRASINGS.iter().filter(|x| lower.contains(**x)).collect();
        if !hits_4_6.is_empty() {
            docs_with_4_6_phrasing += 1;
        }

        // Candidate sentences: every cue match, in prose OR in a heading.
        let (mut here, mut here_head) = (0usize, 0usize);
        for s in &ex.sections {
            // **Skip References, as `novelty::extract_claims` does.** Without
            // this the audit counted 13 candidates where the extractor admitted
            // 12, and the extra was a CITED PAPER'S TITLE — "Nanoemulsion: A
            // novel approach for nose to brain drug delivery." A bibliography
            // entry is a claim somebody else made.
            if s.kind == extract::SectionKind::References {
                continue;
            }
            let hl = s.heading.to_lowercase();
            if CUES.iter().any(|c| hl.contains(*c)) {
                here += 1;
                here_head += 1;
            }
            for para in &s.paragraphs {
                for sent in extract::sentence::sentences_in(para) {
                    sentences_total += 1;
                    let sl = sent.to_lowercase();
                    if CUES.iter().any(|c| sl.contains(*c)) {
                        here += 1;
                        // A paragraph that IS a heading: short, no terminator.
                        let t = sent.trim();
                        let heading_shaped = t.split_whitespace().count() <= 8
                            && !t.ends_with(['.', '!', '?'])
                            && para.trim() == t;
                        if heading_shaped {
                            here_head += 1;
                        }
                    }
                }
            }
        }
        cand_total += here;
        cand_headings += here_head;

        println!(
            "{:<46} claims={:<4} novelty={:<3} §4.6-phrasing={:<20} candidates={} (headings {})",
            short(p),
            claims.len(),
            novelty_here,
            if hits_4_6.is_empty() { "none".to_string() } else { format!("{hits_4_6:?}") },
            here,
            here_head
        );
    }

    println!("\n=== 1. CLAIMS BY CATEGORY across {n_docs} manuscript(s) ===");
    for (k, v) in &by_cat {
        println!("  {k:<22} {v}");
    }
    println!("  {:<22} {total_claims}", "TOTAL");
    let novelty = by_cat.get("NoveltyClaim").copied().unwrap_or(0);
    println!("  novelty category: {novelty} of {total_claims}");

    println!("\n=== 2. §4.6's LITERAL PHRASINGS ===");
    println!("  present in {docs_with_4_6_phrasing} of {n_docs} manuscript(s)");

    println!("\n=== 3/4. CANDIDATE SENTENCES ===");
    println!("  {cand_total} candidate(s) over {n_docs} manuscript(s)");
    println!("  = {:.1} per paper", cand_total as f64 / n_docs.max(1) as f64);
    println!("  of which heading-shaped: {cand_headings} of {cand_total}");
    println!("  (total sentences scanned: {sentences_total})");
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
