//! Does a content-LISTING page separate from a guidance page by shape alone?
//!
//! Reading the 37 barren Nature Medicine pages found 3 listings (the journal
//! homepage, a news index, an issue table of contents) among real guidance.
//! They LOOK different — dozens of headings that are article titles, little
//! prose under each — but "looks different to me after reading them" is the
//! fitting-to-the-read mistake. So: measure prose-per-heading across every
//! admitted page and see whether the classes separate or overlap.
//!
//! If they overlap, no rule gets built.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_shape_probe <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    println!("{:>6} {:>8} {:>7} {:>9}  {:<7} {}", "blocks", "chars", "headed", "chars/hd", "yield", "url");
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let blocks = html_to_blocks(&r.body);
        let reqs = extract_requirements(&blocks);
        let headed = blocks.iter().filter(|b| !b.heading.trim().is_empty()).count();
        let chars: usize = blocks.iter().map(|b| b.text.chars().count()).sum();
        let per = if headed == 0 { 0 } else { chars / headed };
        println!(
            "{:>6} {:>8} {:>7} {:>9}  {:<7} {}",
            blocks.len(),
            chars,
            headed,
            per,
            if reqs.is_empty() { "barren" } else { "FERTILE" },
            p.url
        );
    }
}
