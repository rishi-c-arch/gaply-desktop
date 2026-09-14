//! Second candidate signal for a content-LISTING page: where do its links go?
//!
//! The prose-per-heading measurement failed to separate listings from guidance
//! (169/226/255 for the three listings, interleaved with real guidance at
//! 166/209/228/250). This tests the other candidate: a listing page's links are
//! overwhelmingly to ARTICLES, which the crawler already recognises via
//! NEVER_FOLLOW. Guidance pages link to other guidance.
//!
//! Same discipline as the first: if the classes overlap, no rule gets built.
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

const ARTICLEY: &[&str] = &["/article/", "/articles/", "article?id=", "/doi/", "/full/"];

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_linkshape_probe <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    println!("{:>6} {:>6} {:>7}  {}", "links", "artic", "pct", "url");
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let mut total = 0usize;
        let mut arty = 0usize;
        for part in r.body.split("href=").skip(1) {
            let q = part.trim_start();
            let Some(delim) = q.chars().next() else { continue };
            if delim != '"' && delim != '\'' {
                continue;
            }
            let Some(href) = q[1..].split(delim).next() else { continue };
            if href.is_empty() || href.starts_with('#') {
                continue;
            }
            total += 1;
            let h = href.to_lowercase();
            if ARTICLEY.iter().any(|a| h.contains(a)) {
                arty += 1;
            }
        }
        let pct = if total == 0 { 0 } else { 100 * arty / total };
        println!("{:>6} {:>6} {:>6}%  {}", total, arty, pct, p.url);
    }
}
