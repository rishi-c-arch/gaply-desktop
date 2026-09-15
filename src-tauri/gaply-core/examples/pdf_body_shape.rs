//! The body rule was designed on .docx cell-per-paragraph flattening.
//! What shape does a PDF table body have?
use gaply_core::extract::{self, docparse};

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        println!("\n######## {name} — {} table(s) detected now", ex.tables.len());
        // Find raw "Table N" paragraphs regardless of whether they were admitted.
        let mut shown = 0;
        for sec in &ex.sections {
            for (i, para) in sec.paragraphs.iter().enumerate() {
                let t = para.trim();
                if !(t.starts_with("Table ") || t.starts_with("TABLE ")) { continue; }
                if shown >= 2 { break; }
                shown += 1;
                println!("\n  CAPTION PARA [{:?} p{i}]: {}", sec.kind, t.chars().take(100).collect::<String>());
                for (j, q) in sec.paragraphs.iter().skip(i + 1).take(5).enumerate() {
                    let chars = q.trim().chars().count();
                    let words = q.trim().split_whitespace().count();
                    let prose = chars > 90 && words > 14;
                    println!("    [{j}] chars={chars:<5} words={words:<4} prose={prose}  {}",
                             q.trim().chars().take(90).collect::<String>());
                }
            }
        }
    }
}
