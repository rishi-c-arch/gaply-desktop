//! **The crawl already counted requirements. The runner re-fetches and loses them.**
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let entry = std::env::args().nth(1).expect("entry url");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(2.0, 2.0), &budget, &entry).expect("crawl");
    let during: usize = o.pages.iter().filter(|p| p.verdict == "guideline").map(|p| p.requirements).sum();
    println!("crawl: fetched={} guideline={}", o.fetched, o.guideline);
    println!("requirements the CRAWL counted, in-flight:  {during}");

    let mut after = 0usize;
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        if let Ok(r) = f.get(&HttpRequest::get(&p.url)) {
            after += extract_requirements(&html_to_blocks(&r.body)).len();
        }
    }
    println!("requirements a RE-FETCH pass recovers:      {after}");
    println!("\n  lost to the second fetch: {}", during.saturating_sub(after));
}
