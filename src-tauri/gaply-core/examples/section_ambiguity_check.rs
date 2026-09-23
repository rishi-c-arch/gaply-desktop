//! Is the 177-vs-236 gap the extractor, or the probe's SECTION LOOKUP?
//!
//! Every probe here resolves a table's location with
//! `ex.sections.iter().find(|s| s.kind == tb.location.section)` — and `find`
//! returns the FIRST section of that kind. A thesis has many Results sections.
use gaply_core::extract::{self, docparse};
use std::collections::BTreeMap;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut docs_repeated, mut tables_in_repeated) = (0usize, 0usize);
    let mut total_tables = 0usize;
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for s in &ex.sections {
            *kinds.entry(format!("{:?}", s.kind)).or_default() += 1;
        }
        let repeated: Vec<_> = kinds.iter().filter(|(_, n)| **n > 1).collect();
        total_tables += ex.table_mentions.len();
        if repeated.is_empty() {
            continue;
        }
        docs_repeated += 1;
        // How many tables sit in a kind that occurs more than once?
        let affected = ex
            .table_mentions
            .iter()
            .filter(|t| kinds.get(&format!("{:?}", t.location.section)).copied().unwrap_or(0) > 1)
            .count();
        tables_in_repeated += affected;
        if affected > 0 {
            println!("{name}");
            println!("  repeated section kinds: {repeated:?}");
            println!("  tables whose kind is repeated: {affected} of {}", ex.table_mentions.len());
        }
    }
    println!("\ndocuments with repeated section kinds   {docs_repeated}");
    println!("tables sitting in a repeated kind       {tables_in_repeated} of {total_tables}");
    println!("\n  A probe using `find(kind)` reads the FIRST such section for every one");
    println!("  of those tables — the wrong paragraphs, at an index from another section.");
}
