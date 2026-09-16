//! Where do `lancet`'s 85 "guideline" pages actually live?
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::ratelimit::RateLimiter;
use std::collections::BTreeMap;

fn host_of(u: &str) -> String {
    u.split("://").nth(1).unwrap_or(u).split('/').next().unwrap_or(u).to_string()
}

fn main() {
    let entry = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.thelancet.com/lancet/information-for-authors".into());
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(2.0, 2.0), &budget, &entry).expect("crawl");

    let mut by_host: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // pages, requirements
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let reqs = p.body.as_deref().map(|b| extract_requirements(&html_to_blocks(b)).len()).unwrap_or(0);
        let e = by_host.entry(host_of(&p.url)).or_default();
        e.0 += 1;
        e.1 += reqs;
    }
    println!("entry host: {}\n", host_of(&entry));
    println!("{:<40} {:>7} {:>7}", "host", "guid", "reqs");
    for (h, (pages, reqs)) in &by_host {
        println!("{h:<40} {pages:>7} {reqs:>7}");
    }
    let total: usize = by_host.values().map(|v| v.0).sum();
    let on_journal: usize =
        by_host.iter().filter(|(h, _)| h.contains("thelancet")).map(|(_, v)| v.0).sum();
    println!("\n  guideline pages           {total}");
    println!("  on the JOURNAL's host     {on_journal}");
    println!("  on another host           {}", total - on_journal);
}
