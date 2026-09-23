//! **`<w:tbl>` objects per manuscript, against the caption sightings.**
//!
//! Two numbers that were the same number before §11 D212: `tables` is what the
//! caption regex saw, `doc_tables` is what the file declares. A `.docx` with
//! roman-numeral captions shows the gap; a PDF shows 0 structures because the
//! format carries none, which is not the same as having no tables.
use gaply_core::extract::{self, docparse};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    println!(
        "{:<46} {:>9} {:>9} {:>8} {:>7}",
        "manuscript", "<w:tbl>", "captions", "cells", "cols"
    );
    for p in &paths {
        let path = std::path::Path::new(p);
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let tables = docparse::parse_path_tables(path).unwrap_or_default();
        let Ok(text) = docparse::parse_path(path) else {
            println!("{name:<46} (unreadable)");
            continue;
        };
        let ex = extract::extract_from_text(&text);
        let cells: usize = tables.iter().map(|t| t.cells()).sum();
        let cols: Vec<String> = tables.iter().map(|t| t.columns().to_string()).collect();
        println!(
            "{:<46} {:>9} {:>9} {:>8} {:>7}",
            name,
            tables.len(),
            ex.tables.len(),
            cells,
            cols.join(",")
        );
        for (i, t) in tables.iter().enumerate() {
            let header = t.rows.first().map(|r| r.join(" | ")).unwrap_or_default();
            println!("      [{}] {} rows x {} cols  header: {}", i + 1, t.rows.len(), t.columns(), {
                let h: String = header.chars().take(78).collect();
                h
            });
        }
    }
}
