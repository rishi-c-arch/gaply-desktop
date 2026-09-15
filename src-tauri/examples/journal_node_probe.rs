//! **Phase 4 node 2: the journal-requirements checklist, end to end.**
//!
//! Crawl a real journal, store its requirements, run the checklist against a
//! real manuscript, print every row with the journal's own sentence. Nothing is
//! hand-fed; every span can be checked against the live page.
//!
//! ```text
//! cargo run --release --example journal_node_probe -- <key> <entry-url> <manuscript>
//! ```

use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::journal_standards::bindings_from;
use gaply_core::journal_store::{requirements_for, store_requirements};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::report::checklist_from_requirements;
use gaply_core::Database;

fn main() {
    let mut a = std::env::args().skip(1);
    let key = a.next().expect("usage: journal_node_probe <key> <entry> <manuscript>");
    let entry = a.next().expect("entry url");
    let path = a.next().expect("manuscript path");

    let text = gaply_core::extract::docparse::parse_path(std::path::Path::new(&path)).expect("parse");
    let extraction = gaply_core::extract::extract_from_text(&text);
    let words = text.split_whitespace().count();

    let db = Database::in_memory().unwrap();
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(2.0, 2.0), &budget, &entry).unwrap();

    let mut bindings = Vec::new();
    let mut pages = 0usize;
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let reqs = extract_requirements(&html_to_blocks(&r.body));
        bindings.extend(bindings_from(&reqs));
        let _ = store_requirements(&db, &key, &p.url, &reqs, 1);
        pages += 1;
    }
    bindings.sort();
    bindings.dedup();
    let stored = requirements_for(&db, &key).unwrap();

    println!(
        "manuscript {} — {words} words, {} sections, {} statistics",
        std::path::Path::new(&path).file_name().unwrap().to_string_lossy(),
        extraction.sections.len(),
        extraction.statistics.len()
    );
    println!("journal {key}: {pages} guideline page(s), {} stored requirement(s), {} binding(s)\n",
        stored.len(), bindings.len());

    let items = checklist_from_requirements(&extraction, words, &stored, &bindings);
    println!("=== {} checklist row(s) ===", items.len());
    for (i, it) in items.iter().enumerate() {
        // THREE states, not two. An item nobody could decide must not print as
        // a flag against a manuscript that has done nothing wrong.
        let mark = if it.unevaluable {
            "UNEVAL"
        } else if it.passed {
            "OK    "
        } else {
            "FLAG  "
        };
        println!("\n[{i}] {mark} {}", it.requirement);
        println!("     {}", it.detail);
        if let Some(s) = &it.source_span {
            println!("     journal says: {s}");
        }
        if let Some(f) = &it.checked_field {
            println!("     checked: {f}");
        }
    }

    assert!(!items.is_empty(), "no rows from {pages} pages — the instrument, not the journal");
}
