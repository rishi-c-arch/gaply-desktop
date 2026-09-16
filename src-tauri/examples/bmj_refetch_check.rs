//! **Does the CRAWL see content that the RE-FETCH does not?**
//!
//! The Stage-2 runner crawls a host, then re-fetches every guideline page to
//! extract from it — doubling the requests per host. If the host throttles, the
//! second pass gets stubs and the extraction sees nothing, while the crawl's own
//! record says the page was full of text.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let entry = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.bmj.com/about-bmj/resources-authors".into());
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(2.0, 2.0), &budget, &entry).expect("crawl");
    println!("crawl: fetched={} guideline={}\n", o.fetched, o.guideline);
    println!("{:>10}  {:>10}  {:>8}  url", "crawl_chars", "refetch_chars", "blocks");
    let (mut same, mut shrunk) = (0usize, 0usize);
    for p in o.pages.iter().filter(|p| p.verdict == "guideline").take(8) {
        let (rc, blocks) = match f.get(&HttpRequest::get(&p.url)) {
            Ok(r) => {
                let b = html_to_blocks(&r.body);
                (r.body.chars().count(), b.len())
            }
            Err(_) => (0, 0),
        };
        if rc + 2000 < p.chars {
            shrunk += 1;
        } else {
            same += 1;
        }
        println!("{:>10}  {:>10}  {:>8}  {}", p.chars, rc, blocks, p.url);
    }
    println!("\n  pages whose RE-FETCH is much smaller than the crawl saw: {shrunk}");
    println!("  pages that came back the same size:                      {same}");
    println!("\n  If the crawl saw content and the re-fetch did not, the host is");
    println!("  throttling the SECOND pass and the extractor never sees the page.");
}
