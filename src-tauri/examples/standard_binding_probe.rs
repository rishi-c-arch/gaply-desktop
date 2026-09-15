//! **Which reporting standards does a real journal actually BIND?**
//!
//! The question behind a finding the product now makes: if a researcher
//! submitting a randomised trial sees no CONSORT evaluation, is that because
//! Gaply cannot evaluate CONSORT, or because this journal never stated a CONSORT
//! requirement on the pages we read? Those are different messages and only one
//! of them is checkable by the researcher.
//!
//! Crawl → extract → bind, through `ReqwestFetcher` (never curl: §11's entry on
//! TLS fingerprints). Prints every binding with the journal's own sentence, and
//! every standard MENTIONED but not bound, because a standard named is not a
//! standard bound.
//!
//! ```text
//! cargo run --release --example standard_binding_probe -- <entry-url>
//! ```

use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::{extract_requirements, RequirementKind};
use gaply_core::journal_standards::{bindings_from, Standard};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let entry = std::env::args().nth(1).expect("usage: standard_binding_probe <entry-url>");

    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(2.0, 2.0), &budget, &entry).unwrap();
    let guides: Vec<_> = o.pages.iter().filter(|p| p.verdict == "guideline").collect();
    println!("crawled {} page(s), {} classified guideline\n", o.pages.len(), guides.len());

    let mut bindings = Vec::new();
    let mut mentioned: Vec<(Standard, String, String)> = Vec::new();
    for p in &guides {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let reqs = extract_requirements(&html_to_blocks(&r.body));
        for req in reqs.iter().filter(|r| r.kind == RequirementKind::ReportingStandard) {
            if let Some(s) = Standard::parse(&req.value) {
                mentioned.push((s, p.url.clone(), req.source_span.clone()));
            }
        }
        bindings.extend(bindings_from(&reqs));
    }
    bindings.sort();
    bindings.dedup();

    println!("=== BOUND ({}) — standard, design, and the journal's sentence ===", bindings.len());
    for b in &bindings {
        println!("\n  {} -> {}", b.standard.as_str(), b.design);
        println!("    span: {}", b.source_span);
    }

    let bound: Vec<Standard> = bindings.iter().map(|b| b.standard).collect();
    println!("\n=== MENTIONED BUT NOT BOUND ===");
    let mut shown: Vec<Standard> = Vec::new();
    for (s, url, span) in &mentioned {
        if bound.contains(s) || shown.contains(s) {
            continue;
        }
        shown.push(*s);
        println!("\n  {} — named, no design stated", s.as_str());
        println!("    {url}");
        println!("    span: {span}");
    }

    println!("\n=== NEVER SEEN ON ANY PAGE ===");
    for s in [
        Standard::Consort,
        Standard::Prisma,
        Standard::Strobe,
        Standard::Arrive,
        Standard::Tripod,
        Standard::Cheers,
        Standard::Spirit,
        Standard::Stard,
    ] {
        if !bound.contains(&s) && !mentioned.iter().any(|(m, _, _)| *m == s) {
            println!("  {}", s.as_str());
        }
    }
}
