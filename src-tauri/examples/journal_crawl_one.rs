//! Crawl ONE journal and print every page, so a claim about a specific page
//! can be checked rather than inferred from a total.
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::ratelimit::RateLimiter;

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_crawl_one <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();
    println!("fetched={} guideline={} lex-miss={} stopped={:?} unvisited={}",
             o.fetched, o.guideline, o.lexicon_misses, o.stopped_by, o.unvisited);
    for p in &o.pages {
        println!("  {:<12} d{} lex={} {:<30} {}", p.verdict, p.depth,
                 if p.lexicon_hit { "Y" } else { "n" },
                 p.anchor.chars().take(28).collect::<String>(), p.url);
    }
}
