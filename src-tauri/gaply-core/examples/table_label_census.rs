//! Independent check on the post-fix count: does each document's set of table
//! LABELS look like a real table series? Uses no location lookup at all.
use gaply_core::extract::{self, docparse};
use std::collections::BTreeMap;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut grand = 0usize;
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        if ex.table_mentions.is_empty() { continue; }
        grand += ex.table_mentions.len();
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for t in &ex.table_mentions {
            *counts.entry(t.label.as_str()).or_default() += 1;
        }
        let distinct = counts.len();
        let dupes: usize = counts.values().filter(|n| **n > 1).count();
        let max_rep = counts.values().copied().max().unwrap_or(0);
        println!(
            "{:<52} detections {:>4}  distinct labels {:>4}  labels seen >1x {:>3}  max {:>3}",
            name, ex.table_mentions.len(), distinct, dupes, max_rep
        );
    }
    println!("\ngrand total detections {grand}");
}
