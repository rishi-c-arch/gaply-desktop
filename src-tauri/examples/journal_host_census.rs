//! **Where do a journal's "guideline" pages actually live?** — §11 D174, D175.
//!
//! One command, and it has now explained two of the three journals whose
//! sources-to-requirements ratio looked wrong:
//!
//! * `lancet` — **84 of 85** guideline pages were `elsevier.com` corporate
//!   marketing. The crawl left the journal and spent its budget there.
//! * `statistics-in-medicine` — the INVERSE: `authorservices.wiley.com` holds
//!   12 pages and 17 requirements while the journal's own host holds 23 pages
//!   and 1. Off-host is the defect for one publisher and the guidance for the
//!   other, which is why no rule about hosts settles it.
//!
//! Usage: `cargo run -p app --release --example journal_host_census -- <entry-url> [journal-host-substring]`
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
    let entry = std::env::args().nth(1).expect(
        "usage: journal_host_census <entry-url> [journal-host-substring]",
    );
    // Which host counts as "the journal's". Defaults to the entry's own host,
    // so the split is journal-versus-elsewhere without a hardcoded publisher.
    let journal_host = std::env::args().nth(2).unwrap_or_else(|| host_of(&entry));
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
    println!("\n--- pages on the journal's own host ({journal_host}) ---");
    let mut shown = 0;
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        if !host_of(&p.url).contains(&journal_host) { continue; }
        let reqs = p.body.as_deref().map(|b| extract_requirements(&html_to_blocks(b)).len()).unwrap_or(0);
        if shown < 14 { println!("  reqs={reqs:<3} chars={:<7} {}", p.chars, p.url); shown += 1; }
    }

    let total: usize = by_host.values().map(|v| v.0).sum();
    let on_journal: usize =
        by_host.iter().filter(|(h, _)| h.contains(&journal_host)).map(|(_, v)| v.0).sum();
    println!("\n  guideline pages           {total}");
    println!("  on the JOURNAL's host     {on_journal}");
    println!("  on another host           {}", total - on_journal);
}
