//! **bmj: 120 pages fetched, 18 classified as guideline content, 0 requirements.**
//!
//! §11 D172 recorded this as a finding sitting inside a phase about something
//! else, and noted that a journal the crawler REACHES and extracts nothing from
//! is a different failure from one it cannot reach. This reads the 18 pages and
//! asks which it is: does the extractor miss real guidance, or is there none?
//!
//! Prints ROWS — the obligation-shaped sentences on each page — not counts,
//! because a count of zero is what sent anyone here in the first place.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

/// Words that make a sentence look like a requirement to a human reader.
const OBLIGATION: &[&str] =
    &["must ", "should ", "required", "may not", "no more than", "up to ", "maximum", "limit"];

fn main() {
    let entry = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.bmj.com/about-bmj/resources-authors".into());
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(2.0, 2.0), &budget, &entry).expect("crawl");
    println!("crawl: fetched={} guideline={} stopped={:?}\n", o.fetched, o.guideline, o.stopped_by);

    let (mut pages, mut with_reqs, mut obligation_sentences) = (0usize, 0usize, 0usize);
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        pages += 1;
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else {
            println!("  [{pages}] FETCH FAILED  {}", p.url);
            continue;
        };
        let blocks = html_to_blocks(&r.body);
        let reqs = extract_requirements(&blocks);
        if !reqs.is_empty() {
            with_reqs += 1;
        }
        let chars: usize = blocks.iter().map(|b| b.text.chars().count()).sum();

        let mut hits: Vec<String> = Vec::new();
        for blk in &blocks {
            for s in blk.text.split(". ") {
                let l = s.to_lowercase();
                if OBLIGATION.iter().any(|w| l.contains(w)) && s.trim().len() > 30 {
                    hits.push(s.trim().chars().take(130).collect());
                }
            }
        }
        obligation_sentences += hits.len();
        println!(
            "  [{pages:>2}] blocks={:<4} chars={:<7} reqs={:<3} oblig={:<4} {}",
            blocks.len(),
            chars,
            reqs.len(),
            hits.len(),
            p.url
        );
        for h in hits.iter().take(2) {
            println!("        | {h}");
        }
    }

    println!("\n================ bmj EXTRACTION AUDIT ================");
    println!("  guideline pages           {pages}");
    println!("  pages yielding >=1 req    {with_reqs}");
    println!("  obligation-shaped sentences across all pages  {obligation_sentences}");
    println!("\n  Many obligations + zero requirements -> the EXTRACTOR is the finding.");
    println!("  Zero of both -> the CLASSIFIER is admitting pages with no guidance.");
    println!("======================================================");
}
