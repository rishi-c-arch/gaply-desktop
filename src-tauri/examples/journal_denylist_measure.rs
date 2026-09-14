//! **How often does the injection guard fire on real guideline pages?**
//!
//! Prompt 5 asks for multilingual denylist entries and a structural
//! imperative-mood rule. Before extending a guard it is worth knowing whether
//! it has ever fired on real input: a guard that has never triggered is not
//! proven working, it is untested against the world, and extending it would be
//! adding entries to a list nothing has exercised.
//!
//! So this runs `sanitize` over every page a crawl fetches and reports what
//! trips — by page, with the matched pattern and the surrounding text.
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_denylist_measure <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    let (mut pages, mut tripped, mut stripped_chars) = (0usize, 0usize, 0usize);
    for p in o.pages.iter() {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let blocks = app_lib::guidelines::html_to_blocks(&r.body);
        let text: String = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
        if text.trim().is_empty() {
            continue;
        }
        pages += 1;
        let (clean, hits) = gaply_core::sanitize::sanitize(&text);
        stripped_chars += text.chars().count().saturating_sub(clean.chars().count());
        if !hits.is_empty() {
            tripped += 1;
            println!("\nTRIPPED  {}  [{}]", p.url, p.verdict);
            for h in &hits {
                if let Some(at) = clean.to_lowercase().find(h.as_str()) {
                    let lo = at.saturating_sub(90);
                    let hi = (at + h.len() + 90).min(clean.len());
                    let lo = (0..=lo).rev().find(|i| clean.is_char_boundary(*i)).unwrap_or(0);
                    let hi = (hi..=clean.len()).find(|i| clean.is_char_boundary(*i)).unwrap_or(clean.len());
                    println!("   {h:?}\n      …{}…", clean[lo..hi].replace('\n', " "));
                }
            }
        }
    }
    println!("\n{tripped} of {pages} fetched pages tripped the injection guard.");
    println!("{stripped_chars} hidden/control characters stripped across the crawl.");
}
