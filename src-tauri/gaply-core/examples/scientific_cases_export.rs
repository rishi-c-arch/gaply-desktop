//! **Export the D165 adjudication pool as JSONL, for Phase B.**
//!
//! §11 D165 measured precision over the extractor's OWN output: 152 `Method`
//! objects, adjudicated one at a time. That denominator is defined by the thing
//! being measured, so a second extractor cannot be compared to it — it would emit
//! a different number of objects and the two precisions would answer different
//! questions. CLAUDE.md's denominator entry is the general form.
//!
//! This exports FIXED INPUTS instead: one row per paragraph, carrying the full
//! paragraph text (never a prefix — a truncated span is not a span), what the
//! regex extractor said about it, and whether the no-skill heuristic would have
//! guessed it. Ground truth is added by hand afterwards; nothing here labels.
//!
//! ```text
//! cargo run --release -p gaply_core --example scientific_cases_export -- <manuscript>... > pool.jsonl
//! ```
use gaply_core::extract::{self, docparse, paragraph_at};
use gaply_core::scientific_model::SourceSpan;

fn main() {
    for p in std::env::args().skip(1) {
        let path = std::path::Path::new(&p);
        let Ok(t) = docparse::parse_path(path) else { continue };
        let doc = path.file_name().unwrap().to_string_lossy().to_string();
        let r = extract::extract_from_text_with(&t, extract::ExtractOptions::with_scientific());

        // What the REGEX chose: paragraph text -> the fields it asserted.
        let mut flagged: Vec<(String, String, String, String)> = Vec::new();
        if let Some(s) = &r.scientific {
            for m in &s.methods {
                if let Some(SourceSpan::Point(loc)) = m.source_spans.first() {
                    if let Some(par) = paragraph_at(&r, loc) {
                        flagged.push((
                            par.to_string(),
                            format!("{:?}", m.design),
                            format!("{:?}", m.sampling.size),
                            format!("{:?}", m.software),
                        ));
                    }
                }
            }
        }

        // What the NO-SKILL heuristic would guess: the first paragraph of each
        // section whose heading names methods. This is D165's 50% row, and it is
        // the bar the entry says decided the decline.
        let mut noskill: Vec<String> = Vec::new();
        for sec in &r.sections {
            let h = sec.heading.to_lowercase();
            if h.contains("method") || h.contains("materials") {
                if let Some(first) = sec.paragraphs.first() {
                    noskill.push(first.trim().to_string());
                }
            }
        }

        let mut seen = std::collections::BTreeSet::new();
        for (text, design, n, sw) in &flagged {
            let key = text.trim();
            if key.is_empty() || !seen.insert(key.to_string()) {
                continue;
            }
            println!(
                "{}",
                serde_json::json!({
                    "doc": doc, "text": key,
                    "regex_flagged": true, "regex_design": design,
                    "regex_n": n, "regex_software": sw,
                    "noskill_guess": noskill.iter().any(|x| x == key),
                })
            );
        }
        for text in &noskill {
            let key = text.trim();
            if key.is_empty() || !seen.insert(key.to_string()) {
                continue;
            }
            println!(
                "{}",
                serde_json::json!({
                    "doc": doc, "text": key,
                    "regex_flagged": false, "regex_design": "",
                    "regex_n": "", "regex_software": "",
                    "noskill_guess": true,
                })
            );
        }
    }
}
