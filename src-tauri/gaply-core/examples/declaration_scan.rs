//! What do variable declarations actually LOOK like in these manuscripts?
//! Measured before a binding predicate is written, not after.
use gaply_core::extract::docparse;
use std::path::Path;

fn main() {
    for a in std::env::args().skip(1) {
        let p = Path::new(&a);
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or(&a);
        let bytes = std::fs::read(p).expect("read");
        let text = if p.extension().map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false) {
            docparse::parse_docx(&bytes)
        } else {
            docparse::parse_pdf_bytes(&bytes)
        }
        .expect("parse");
        println!("########## {name}");
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() || t.len() > 400 {
                continue;
            }
            // A declaration-shaped line: short, contains `=` or " is " or
            // "where", and starts with something name-like.
            let lower = t.to_lowercase();
            let declarative = t.contains('=')
                || lower.starts_with("where")
                || lower.contains(" is the ")
                || lower.contains(" denotes ");
            if !declarative {
                continue;
            }
            let head: String = t.chars().take(3).collect();
            let looks_like_a_decl = lower.starts_with("where")
                || head.chars().next().is_some_and(|c| c.is_alphabetic())
                    && t.chars().take(24).any(|c| c == '=');
            if looks_like_a_decl {
                println!("  {t}");
            }
        }
    }
}
