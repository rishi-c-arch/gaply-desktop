//! **The span-by-span pass on the shipped rule.** Not a projection from a
//! scan — this runs `extract_requirements` exactly as the pipeline does and
//! prints every `SectionRequired` row it produces, with its span and source,
//! so each can be judged true or false by reading the journal's own sentence.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_expect::is_reviewer_guidance;
use gaply_core::journal_extract::{extract_requirements, RequirementKind};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_statement_audit <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    let (mut rows, mut reviewer_rows, mut newly_fertile) = (0usize, 0usize, 0usize);
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let blocks = html_to_blocks(&r.body);
        let reqs = extract_requirements(&blocks);
        let stmts: Vec<_> =
            reqs.iter().filter(|r| r.kind == RequirementKind::SectionRequired).collect();
        let title = app_lib::guidelines::html_title(&r.body);
        let body: String = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join(" ");
        let reviewer = is_reviewer_guidance(&p.url, &title, &body);
        // Every admitted page, so `is_reviewer_guidance` can be judged against
        // the whole crawl rather than only where it would suppress a row.
        println!("REVIEWER={:<5} {}", reviewer, p.url);
        if stmts.is_empty() {
            continue;
        }
        if reqs.len() == stmts.len() {
            newly_fertile += 1;
        }
        println!(
            "\n--- {} {}",
            p.url,
            if reviewer { "  [REVIEWER GUIDANCE]" } else { "" }
        );
        for r in stmts {
            rows += 1;
            if reviewer {
                reviewer_rows += 1;
            }
            println!("   {}\n      span: {}", r.value, r.source_span);
        }
    }
    println!("\n{rows} SectionRequired rows; {reviewer_rows} on reviewer pages; \
              {newly_fertile} pages fertile ONLY because of this rule");
}
