//! **Would restricting `read_design` to Methods/Abstract fix the false positives?**
//!
//! Every experimental false positive measured this session silences a Tier-2
//! check or mis-binds a standard. A section restriction is the cheapest
//! candidate fix — but only if the bad markers live OUTSIDE Methods. This prints
//! the section each experimental marker was actually found in, so the fix is
//! chosen on where they are rather than on where they feel like they should be.
use gaply_core::extract::{self, docparse};
use gaply_core::extract::SectionKind;
use gaply_core::specialist::claim_strength::{EXPERIMENTAL_MARKERS, OBSERVATIONAL_MARKERS};

fn main() {
    // Only the manuscripts where an experimental read does damage.
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let mut rows: Vec<(String, String)> = Vec::new();
        for section in ex.sections.iter().filter(|s| s.kind != SectionKind::References) {
            for para in &section.paragraphs {
                let lower = para.to_lowercase();
                for (list, tag) in
                    [(EXPERIMENTAL_MARKERS, "exp"), (OBSERVATIONAL_MARKERS, "obs")]
                {
                    for m in list.iter().filter(|m| lower.contains(**m)) {
                        let k = format!("[{tag}] {:?}", section.kind);
                        if !rows.iter().any(|(a, b)| a == *m && *b == k) {
                            rows.push(((*m).to_string(), k));
                        }
                    }
                }
            }
        }
        if rows.is_empty() {
            continue;
        }
        println!("\n######## {name}");
        for (m, k) in rows {
            println!("    {m:<18} in {k}");
        }
    }
}
