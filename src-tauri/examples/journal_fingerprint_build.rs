//! Crawl a journal, extract its requirements, and STORE them — then paste the
//! counts Prompt 5 asks for: per journal, per kind, per source document.
//!
//! The point of running this rather than trusting the unit tests: the tables
//! were added empty, and §3.4's largest correction was a count of a table
//! nothing writes to. This is the end-to-end evidence that they are not that.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::journal_store::{requirements_for, store_requirements};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::Database;
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let key = args.next().expect("usage: journal_fingerprint_build <journal-key> <entry-url>");
    let entry = args.next().expect("entry url");

    let db = Database::in_memory().expect("db");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();
    println!("crawl: fetched={} guideline={} stopped={:?}\n", o.fetched, o.guideline, o.stopped_by);

    let mut per_doc: BTreeMap<String, usize> = BTreeMap::new();
    let (mut inserted, mut conflicts) = (0usize, 0usize);
    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let reqs = extract_requirements(&html_to_blocks(&r.body));
        if reqs.is_empty() {
            continue;
        }
        let out = store_requirements(&db, &key, &p.url, &reqs, 1).expect("store");
        inserted += out.inserted;
        conflicts += out.new_conflicts;
        if out.inserted > 0 {
            per_doc.insert(p.url.clone(), out.inserted);
        }
    }

    let stored = requirements_for(&db, &key).unwrap();
    let mut per_kind: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &stored {
        *per_kind.entry(r.kind.as_str()).or_default() += 1;
    }

    println!("=== {key} ===");
    println!("rows in journal_requirements : {}", stored.len());
    println!("  inserted this run          : {inserted}");
    println!("  distinct conflicts found   : {conflicts}");
    println!("  by kind                    : {per_kind:?}");
    println!("  bound to an article type   : {}", stored.iter().filter(|r| r.article_type.is_some()).count());

    println!("\n--- per source document ---");
    for (url, n) in &per_doc {
        println!("  {n:>3}  {url}");
    }

    let conflicted: Vec<_> = stored.iter().filter(|r| r.status == "conflicted").collect();
    println!("\n--- CONFLICTED facts ({}) — both values, neither chosen ---", conflicted.len());
    let mut groups: BTreeMap<&str, Vec<&gaply_core::journal_store::StoredRequirement>> =
        BTreeMap::new();
    for r in &conflicted {
        groups.entry(r.conflict_id.as_deref().unwrap_or("?")).or_default().push(r);
    }
    for (id, rows) in groups.iter().take(4) {
        println!("  conflict {id}");
        for r in rows {
            println!("    {:<18} = {:<8} type={:<20}", r.kind.as_str(), r.value,
                     r.article_type.as_deref().unwrap_or("(not stated)"));
            println!("        span: {}", r.source_span.chars().take(110).collect::<String>());
            println!("        from: {}", r.source_url);
        }
    }

    println!("\n--- one full row, checkable against the live page ---");
    if let Some(r) = stored.iter().find(|r| r.kind.as_str() == "word_limit") {
        println!("  kind          : {}", r.kind.as_str());
        println!("  value         : {}", r.value);
        println!("  article_type  : {:?}", r.article_type);
        println!("  status        : {}", r.status);
        println!("  source_url    : {}", r.source_url);
        println!("  source_heading: {}", r.source_heading);
        println!("  source_span   : {}", r.source_span);
    }
}
