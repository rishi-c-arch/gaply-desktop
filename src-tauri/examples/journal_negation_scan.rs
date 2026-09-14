//! Does a real prohibition carrying the word "statement" exist in the corpus?
//!
//! The negation guard in `states_a_required_statement` was written against a
//! sentence the statement-word guard already rejected, so its test passed for
//! the wrong reason and the guard was unexercised. This asks the corpus
//! directly: print every sentence that names a required-statement, contains
//! the word "statement" or "declaration", AND negates its modal. If the corpus
//! has none, the guard is speculative and that is worth knowing.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

const NAMES: &[&str] = &[
    "competing interest", "conflict of interest", "funding statement",
    "author contribution", "code availability", "data availability",
    "ethics statement", "ethics approval", "informed consent",
];
const MODALS: &[&str] = &["must", "required", "should", "need to", "are expected to"];
const NEG: &[&str] = &[" not ", " never ", "n't ", " cannot "];

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_negation_scan <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    let mut hits = 0usize;
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        for b in html_to_blocks(&r.body) {
            for raw in b.text.split(['.', '?', '!']) {
                let s = raw.trim();
                if s.len() < 25 {
                    continue;
                }
                let l = s.to_lowercase();
                if !NAMES.iter().any(|n| l.contains(n)) {
                    continue;
                }
                if !(l.contains("statement") || l.contains("declaration")) {
                    continue;
                }
                let Some((at, len)) =
                    MODALS.iter().filter_map(|m| l.find(m).map(|i| (i, m.len()))).min()
                else {
                    continue;
                };
                let lo = at.saturating_sub(16);
                let hi = (at + len + 24).min(l.len());
                let lo = (0..=lo).rev().find(|i| l.is_char_boundary(*i)).unwrap_or(0);
                let hi = (hi..=l.len()).find(|i| l.is_char_boundary(*i)).unwrap_or(l.len());
                if NEG.iter().any(|n| l[lo..hi].contains(n)) {
                    hits += 1;
                    println!("  NEGATED  {}\n      window: {:?}\n      from: {}", s, &l[lo..hi], p.url);
                }
            }
        }
    }
    println!("\nreal prohibitions that reach the negation guard: {hits}");
}
