//! **What a generalised "required statement" rule WOULD yield — measured, not
//! estimated, and not yet built into the extractor.**
//!
//! `journal_extract` already has one rule of this shape: a named statement
//! ("data availability") plus a modal produces a `DataPolicy` requirement. Four
//! of the nine extractor-gap pages found by reading Nature Medicine's barren
//! set carry the identical shape with a different name — competing interests,
//! funding, author contributions, AI use. This scans the real crawl for all of
//! them and prints the spans, so the proposal carries a count rather than an
//! expectation.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

/// Statement names, each the thing a manuscript must CONTAIN.
const STATEMENTS: &[(&str, &str)] = &[
    ("competing interest", "competing interests statement"),
    ("conflict of interest", "competing interests statement"),
    ("funding statement", "funding statement"),
    ("author contribution", "author contributions statement"),
    ("code availability", "code availability statement"),
    ("ethics statement", "ethics statement"),
    ("ethics approval", "ethics approval statement"),
    ("informed consent", "informed consent statement"),
];
const MODALS: &[&str] = &["must", "required", "should", "are expected to"];

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_statement_scan <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    let mut hits = 0usize;
    let mut pages = 0usize;
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let mut page_hit = false;
        for b in html_to_blocks(&r.body) {
            for raw in b.text.split(['.', '?', '!']) {
                let s = raw.trim();
                if s.len() < 25 {
                    continue;
                }
                let lower = s.to_lowercase();
                if !MODALS.iter().any(|m| lower.contains(m)) {
                    continue;
                }
                for (needle, name) in STATEMENTS {
                    if lower.contains(needle) {
                        hits += 1;
                        page_hit = true;
                        let q: String = s.chars().take(190).collect();
                        println!("  {name}\n      span: {q}\n      from: {}", p.url);
                        break;
                    }
                }
            }
        }
        if page_hit {
            pages += 1;
        }
    }
    println!("\nWOULD YIELD: {hits} requirements across {pages} pages");
}
