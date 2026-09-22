//! **Export every paragraph WITH its place in the document** — the input §11
//! D204 says the scientific-layer probe removes.
//!
//! `scientific_cases_export.rs` emits one row per paragraph and deliberately
//! carries nothing about where it sits. D204's decline rests on that: three
//! model tiers spanning four orders of magnitude all land below a one-line rule
//! that uses only position, so the missing input is position, not capacity.
//!
//! This emits the SAME paragraphs from the SAME extraction call, plus the
//! section heading that governs them, the paragraph's index inside that
//! section, and the paragraphs immediately before and after it. Joined onto the
//! fixed 160 by exact paragraph text, it turns the fixed set into a
//! with-context set without relabelling anything.
//!
//! ```text
//! cargo run --release -p gaply_core --example scientific_context_export -- <manuscript>... > ctx.jsonl
//! ```
use gaply_core::extract;

fn main() {
    for p in std::env::args().skip(1) {
        let path = std::path::Path::new(&p);
        let Ok(t) = extract::docparse::parse_path(path) else {
            eprintln!("parse failed: {p}");
            continue;
        };
        let doc = path.file_name().unwrap().to_string_lossy().to_string();
        let r = extract::extract_from_text_with(&t, extract::ExtractOptions::with_scientific());
        eprintln!("{doc}: {} sections", r.sections.len());
        for (si, sec) in r.sections.iter().enumerate() {
            for (pi, par) in sec.paragraphs.iter().enumerate() {
                println!(
                    "{}",
                    serde_json::json!({
                        "doc": doc,
                        "heading": sec.heading,
                        "kind": format!("{:?}", sec.kind),
                        "section_index": si,
                        "paragraph_index": pi,
                        "paragraphs_in_section": sec.paragraphs.len(),
                        "prev": if pi > 0 { sec.paragraphs.get(pi - 1).cloned() } else { None },
                        "next": sec.paragraphs.get(pi + 1).cloned(),
                        "text": par,
                    })
                );
            }
        }
    }
}
