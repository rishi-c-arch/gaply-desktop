//! **Item 7's deliverable: a checklist run against a real journal's actual
//! requirements.**
//!
//! Crawl → extract → store → build the checklist from the stored rows. Nothing
//! is hand-fed; every item's span is the journal's own sentence and can be
//! checked against the live page.
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
    let key = a.next().expect("usage: journal_checklist_probe <key> <entry> <manuscript.txt>");
    let entry = a.next().expect("entry url");
    let path = a.next().expect("manuscript path");

    let text = gaply_core::extract::docparse::parse_path(std::path::Path::new(&path)).expect("parse");
    let extraction = gaply_core::extract::extract_from_text(&text);
    let words = text.split_whitespace().count();

    let db = Database::in_memory().unwrap();
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    let mut all_bindings = Vec::new();
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let reqs = extract_requirements(&html_to_blocks(&r.body));
        all_bindings.extend(bindings_from(&reqs));
        let _ = store_requirements(&db, &key, &p.url, &reqs, 1);
    }
    all_bindings.sort();
    all_bindings.dedup();
    let stored = requirements_for(&db, &key).unwrap();

    println!("manuscript : {path}");
    println!("  {words} words, {} sections\n", extraction.sections.len());
    println!("journal    : {key}");
    println!("  {} requirements stored, {} standard bindings\n", stored.len(), all_bindings.len());

    let checklist = checklist_from_requirements(&extraction, &text, words, &stored, &all_bindings);
    println!("=== CHECKLIST ({} items) ===", checklist.len());
    for c in &checklist {
        println!("\n  [{}] {}", if c.passed { "PASS" } else { "FAIL" }, c.requirement);
        println!("      {}", c.detail);
        if let Some(t) = &c.article_type {
            println!("      article type : {t}");
        }
        if let Some(fld) = &c.checked_field {
            println!("      checked      : {fld}");
        }
        if let Some(sp) = &c.source_span {
            // The WHOLE span. Truncating it hid the phrase that justified the
            // data-availability item and made a true finding look unsupported —
            // a span exists to be checked, and a clipped one cannot be.
            println!("      journal says : \"{sp}\"");
        }
        if let Some(u) = &c.guideline_source {
            println!("      source       : {u}");
        }
    }
}
