//! Crawl a journal, extract requirements, and report the split.
//!
//! Prompt 5 item 2 asks for the pattern/model split measured per journal. The
//! model half does not exist yet, so what is measurable now is the other side
//! of the same number: **guideline pages that yield ZERO requirements.** Those
//! are either pages a pattern cannot reach (the model's future work) or pages
//! that are not guidance at all (`classify_page`'s false admissions).
//!
//! That is the measurement the gate's precision should be revisited on — a
//! count rather than an impression formed from reading URLs.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::{extract_requirements, ExtractedRequirement, RequirementKind};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use std::collections::BTreeMap;

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_extract_probe <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let limiter = RateLimiter::new(4.0, 4.0);

    let o = crawl(&f, &limiter, &budget, &entry).unwrap();
    println!("crawl: fetched={} guideline={} stopped={:?}", o.fetched, o.guideline, o.stopped_by);

    let (mut with, mut without) = (0usize, 0usize);
    let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut bound = 0usize;
    let mut all: Vec<(String, ExtractedRequirement)> = Vec::new();
    let mut barren: Vec<String> = Vec::new();

    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let reqs = extract_requirements(&html_to_blocks(&r.body));
        if reqs.is_empty() {
            without += 1;
            barren.push(p.url.clone());
            continue;
        }
        with += 1;
        for req in reqs {
            *by_kind.entry(req.kind.as_str()).or_default() += 1;
            if req.article_type.is_some() {
                bound += 1;
            }
            all.push((p.url.clone(), req));
        }
    }

    println!("\nguideline pages yielding >=1 requirement : {with}");
    println!("guideline pages yielding ZERO            : {without}");
    println!("requirements extracted (by pattern)      : {}", all.len());
    println!("  of which bound to an article type      : {bound}");
    println!("  by kind: {by_kind:?}");

    println!("\n--- every word/abstract/figure/reference limit, with its span ---");
    for (url, r) in all.iter().filter(|(_, r)| {
        matches!(
            r.kind,
            RequirementKind::WordLimit
                | RequirementKind::AbstractLimit
                | RequirementKind::FigureLimit
                | RequirementKind::ReferenceLimit
        )
    }) {
        println!(
            "  {:<18} {:>7}  type={:<20} heading={:<24}\n      span: {}\n      from: {url}",
            r.kind.as_str(),
            r.value,
            r.article_type.as_deref().unwrap_or("(not stated)"),
            r.source_heading.chars().take(22).collect::<String>(),
            r.source_span.chars().take(120).collect::<String>()
        );
    }

    println!("\n--- guideline pages that yielded nothing (the number to revisit the gate on) ---");
    for u in barren.iter().take(20) {
        println!("  {u}");
    }
}
