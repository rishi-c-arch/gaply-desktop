//! Where the mathematics in these manuscripts actually LIVES.
//!
//! The OMML survey answered "how many carry equation-editor math" (almost
//! none). This answers the question that matters for §6b: how many carry
//! mathematics AT ALL, in a form the text layer preserves.
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

        let mut hits = 0usize;
        println!("########## {name}");
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() || t.len() > 300 {
                continue;
            }
            if !t.contains('=') {
                continue;
            }
            let digits = t.chars().filter(char::is_ascii_digit).count();
            let ops = t.matches(['/', '×', '*', '^', '−', '+']).count();
            // An equation-shaped line: an equals sign, an operator, and digits,
            // but not a prose sentence that happens to contain "p = 0.03".
            if digits >= 2 && ops >= 1 {
                hits += 1;
                if hits <= 12 {
                    println!("  {t}");
                }
            }
        }
        println!("  -> {hits} equation-shaped lines\n");
    }
}
