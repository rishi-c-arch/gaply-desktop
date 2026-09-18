//! **One confirmed false PASS is grounds to check its siblings.**
//!
//! `checklist_from_requirements` finds every required statement the same way:
//! `synonyms_for(needle)` then `statement_in_text`, matched ANYWHERE in the
//! manuscript with no requirement that the match be a declaration. Funding
//! passed on *"Both regulatory and financial support are required
//! simultaneously for a change…"* — discussion prose. This prints the sentence
//! each statement kind matches, per manuscript, so the siblings are read rather
//! than assumed correct.
use gaply_core::extract::docparse;

const KINDS: &[(&str, &[&str])] = &[
    ("competing interest", &["competing interest", "conflict of interest", "conflicts of interest", "declaration of interest", "disclosure statement"]),
    ("funding", &["funding", "financial support", "grant support", "supported by a grant"]),
    ("data availability", &["data availability", "data availability statement", "availability of data"]),
    ("code availability", &["code availability", "code is available", "software availability"]),
    ("ethics", &["ethics", "ethical approval", "ethics approval", "institutional review board", "ethics committee", "ethical clearance"]),
    ("informed consent", &["informed consent", "consent was obtained", "consent to participate"]),
    ("author contribution", &["author contribution", "authors' contribution", "credit author"]),
];

fn statement_in_text(lower: &str, original: &str, names: &[&str]) -> Option<String> {
    for (at, _) in names.iter().flat_map(|n| lower.match_indices(n)) {
        let start = original[..at].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
        let rest = &original[at..];
        let end = rest.find(['.', '\n']).map(|i| at + i + 1).unwrap_or(original.len());
        let sentence = original[start..end].trim();
        if at - start >= 40 { continue }
        let t = sentence.trim_end();
        if t.split_whitespace().last().is_some_and(|w| w.len() <= 4 && w.chars().all(|c| c.is_ascii_digit())) { continue }
        return Some(sentence.to_string());
    }
    None
}

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let lower = text.to_lowercase();
        let mut rows = Vec::new();
        for (kind, names) in KINDS {
            if let Some(s) = statement_in_text(&lower, &text, names) {
                rows.push((*kind, s.chars().take(112).collect::<String>()));
            }
        }
        if rows.is_empty() { continue }
        println!("\n######## {name}");
        for (k, s) in rows {
            println!("  {k:<20} :: {s}");
        }
    }
}
