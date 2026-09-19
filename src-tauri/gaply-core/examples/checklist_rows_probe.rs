//! **What rows does the checklist actually put on screen, and what span does
//! each one quote?**
//!
//! Drives the real `load_bundled_seed` + `build_checklist` against a real
//! manuscript. Counting rows in the seed JSON answers a different question:
//! `requirements_for` is `ORDER BY id DESC` and `checklist_from_requirements`
//! takes the FIRST match per statement needle, so the row a researcher reads is
//! the LAST one the crawl stored — which is not the one a reader of the file
//! would predict.

use std::path::Path;

use gaply_core::{extract, journal_store, report, Database};

fn main() {
    let mut args = std::env::args().skip(1);
    let key = args.next().unwrap_or_else(|| "nature-medicine".to_string());
    let path = args.next().unwrap_or_else(|| {
        eprintln!("usage: checklist_rows_probe <journal_key> <manuscript>");
        std::process::exit(2);
    });

    let db = Database::in_memory().expect("db");
    let seeded = journal_store::load_bundled_seed(&db).expect("seed");
    eprintln!("seeded {} journals, {} requirements", seeded.seeded.len(), seeded.requirements);

    let text = extract::docparse::parse_path(Path::new(&path)).expect("parse");
    let ex = extract::extract_from_text(&text);
    let items = report::build_checklist(&db, &ex, &text, None, Some(&key)).expect("checklist");

    println!("\n=== {} rows for {key} on {} ===\n", items.len(), base(&path));
    for (i, it) in items.iter().enumerate() {
        println!(
            "{:>2}. [{}] {}",
            i + 1,
            if it.unevaluable { "UNEVALUABLE" } else if it.passed { "  met  " } else { "NOT MET" },
            it.requirement
        );
        println!("     detail:       {}", one_line(&it.detail, 150));
        println!("     article_type: {:?}", it.article_type);
        println!("     checked:      {:?}", it.checked_field);
        match &it.source_span {
            Some(s) => println!("     SPAN 1/{}:      {}", it.also_from.len() + 1, one_line(s, 170)),
            None => println!("     SPAN:         (none — structural check)"),
        }
        for (k, src) in it.also_from.iter().enumerate() {
            println!(
                "     SPAN {}/{}:      [{}] {}",
                k + 2,
                it.also_from.len() + 1,
                src.article_type.as_deref().unwrap_or("any type"),
                one_line(&src.source_span, 150)
            );
        }
        println!();
    }
}

fn one_line(s: &str, n: usize) -> String {
    let j = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if j.chars().count() <= n { j } else { j.chars().take(n).collect::<String>() + " …" }
}
fn base(p: &str) -> String {
    Path::new(p).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}
