//! **What does a repeated `SectionKind` actually MEAN?**
//!
//! Five Introductions in a thesis is chapter structure — five real, separate
//! introductions. Two Methods in a paper may be one subsection the classifier
//! flattened. Those want different answers, so the counts decide which case
//! matters. Prints the HEADING of every section in each repeated run.
use gaply_core::extract::{self, docparse};
use std::collections::BTreeMap;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut runs_by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let (mut docs, mut docs_repeated) = (0usize, 0usize);
    let (mut identical_heading, mut different_heading) = (0usize, 0usize);
    let mut total_repeated_sections = 0usize;

    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        docs += 1;
        let ex = extract::extract_from_text(&text);
        let mut by_kind: BTreeMap<String, Vec<(usize, &str, usize)>> = BTreeMap::new();
        for (i, s) in ex.sections.iter().enumerate() {
            by_kind
                .entry(format!("{:?}", s.kind))
                .or_default()
                .push((i, s.heading.as_str(), s.paragraphs.len()));
        }
        let repeated: Vec<_> = by_kind.iter().filter(|(_, v)| v.len() > 1).collect();
        if repeated.is_empty() {
            continue;
        }
        docs_repeated += 1;
        println!("\n######## {name}");
        for (kind, secs) in repeated {
            *runs_by_kind.entry(kind.clone()).or_default() += 1;
            total_repeated_sections += secs.len();
            let all_same = secs.iter().all(|(_, h, _)| *h == secs[0].1);
            if all_same {
                identical_heading += secs.len();
            } else {
                different_heading += secs.len();
            }
            println!("  {kind} x{}  headings {}", secs.len(),
                     if all_same { "IDENTICAL" } else { "DIFFER" });
            for (i, h, n) in secs {
                println!("    [section {i:>3}] {n:>4} para  {:?}", h);
            }
        }
    }

    println!("\n================ WHAT A REPEAT MEANS ================");
    println!("  documents                       {docs}");
    println!("  with >=1 repeated kind          {docs_repeated}");
    println!("  sections inside a repeated run  {total_repeated_sections}");
    println!("    where all headings IDENTICAL  {identical_heading}");
    println!("    where headings DIFFER         {different_heading}");
    println!("  repeated runs by kind:");
    for (k, n) in &runs_by_kind {
        println!("    {k:<14} {n} document(s)");
    }
    println!("=====================================================");
}
