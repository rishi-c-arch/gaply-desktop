//! **What is in the checklist a researcher reads?**
//!
//! Measured through the product path, `run_publishready` passed no
//! `guidelines_url`, produced FOUR rows — "required section: Abstract/Methods/
//! Results/References found", all PASS. That is the degenerate case: the real
//! frontend calls `ingest_guidelines` first. This asks what the SAME manuscript
//! gets with each of six really-ingested journals, so "the checklist is thin"
//! is a measurement rather than an impression.
use gaply_core::extract::{self, docparse};
use gaply_core::report::build_checklist;
use gaply_core::Database;

fn main() {
    let mut args = std::env::args().skip(1);
    let db_path = args.next().expect("db path");
    let manuscript = args.next().expect("manuscript path");
    let db = Database::open(std::path::Path::new(&db_path)).expect("open db");
    let text = docparse::parse_path(std::path::Path::new(&manuscript)).expect("parse");
    let ex = extract::extract_from_text(&text);

    let urls: Vec<Option<String>> = vec![
        None,
        Some("https://www.nature.com/nm".into()),
        Some("https://www.bmj.com".into()),
        Some("https://bmcpublichealth.biomedcentral.com".into()),
        Some("https://journals.plos.org/plosone/s/submission-guidelines".into()),
        Some("https://journals.plos.org/plosmedicine/s/submission-guidelines".into()),
    ];
    for u in &urls {
        let items = build_checklist(&db, &ex, &text, u.as_deref()).expect("checklist");
        let pass = items.iter().filter(|i| i.passed).count();
        let uneval = items.iter().filter(|i| i.unevaluable).count();
        println!(
            "\n######## {}   {} items ({pass} pass, {} fail, {uneval} unevaluable)",
            u.as_deref().unwrap_or("<no journal selected>"),
            items.len(),
            items.len() - pass - uneval
        );
        for i in items.iter().take(14) {
            let mark = if i.unevaluable { "????" } else if i.passed { "PASS" } else { "FAIL" };
            println!("  [{mark}] {}", i.requirement.chars().take(88).collect::<String>());
            println!("         detail      : {}", i.detail.chars().take(120).collect::<String>());
            if let Some(s) = &i.source_span {
                println!("         journal said: {}", s.chars().take(96).collect::<String>());
            }
        }
        if items.len() > 14 {
            println!("  … {} more", items.len() - 14);
        }
    }
}
