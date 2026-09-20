//! **What does one more hop cost, and what does it buy? (§11 D193 step 3)**
//!
//! Nature Sustainability's discovered guidance page yields 0 requirements from
//! 3,471 characters; the control, Nature Medicine, yields 0 from 3,453 while the
//! bundled seed holds 39 for it — from a multi-page crawl. So the page a homepage
//! points at is a HUB, and the requirements are one level down.
//!
//! This measures the hop instead of estimating it: pages fetched, wall-clock
//! seconds, and requirements gained, with the same `ingest_with` a pasted URL
//! takes. Cost first, build second.
use app_lib::guidelines::{self, JournalIdentity};
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::discover_guidelines_links;
use gaply_core::embed::HashEmbedder;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::Database;
use url::Url;

/// A bound, because "one more hop" without one is a crawl. Chosen before the run.
const MAX_CHILDREN: usize = 12;
const POLITE_MS: u64 = 1200;

fn main() {
    let mut args = std::env::args().skip(1);
    let hub = args.next().expect("usage: one_hop_probe <hub-url> <journal name>");
    let name = args.next().expect("a journal name");
    let f = ReqwestFetcher::new().expect("client");
    let limiter = RateLimiter::new(8.0, 1.0);
    let db = Database::in_memory().expect("db");
    let t0 = std::time::Instant::now();

    // Hop 0 — the page the homepage pointed at, exactly as today.
    let base = Url::parse(&hub).expect("url");
    let rep0 = guidelines::ingest_with(
        &f,
        &limiter,
        &db,
        &HashEmbedder,
        None,
        Some(&hub),
        JournalIdentity { key: None, name: Some(&name) },
    );
    let t_hop0 = t0.elapsed();
    println!("HOP 0  {hub}");
    println!("  requirements={}  elapsed={:?}  fetches=1", rep0.requirements_stored, t_hop0);

    // Hop 1 — the guidance links the hub itself names.
    let body = match f.get(&HttpRequest::get(&hub)) {
        Ok(r) if r.status == 200 => r.body,
        other => {
            println!("  hub unreadable on refetch: {other:?}");
            return;
        }
    };
    let kids: Vec<_> = discover_guidelines_links(&body, &base)
        .into_iter()
        .filter(|l| !l.off_host && l.url != hub)
        .take(MAX_CHILDREN)
        .collect();
    println!("\nHOP 1  {} candidate(s) named by the hub (cap {MAX_CHILDREN})", kids.len());

    let mut fetches = 1usize;
    let mut gained = 0usize;
    for k in &kids {
        std::thread::sleep(std::time::Duration::from_millis(POLITE_MS));
        let r = guidelines::ingest_with(
            &f,
            &limiter,
            &db,
            &HashEmbedder,
            None,
            Some(&k.url),
            JournalIdentity { key: None, name: Some(&name) },
        );
        fetches += 1;
        gained += r.requirements_stored;
        println!(
            "  +{:<3} {:?}  {}",
            r.requirements_stored,
            k.confidence,
            k.url.chars().take(96).collect::<String>()
        );
    }

    let total = t0.elapsed();
    println!("\n=== cost of one more hop, {name} ===");
    println!("  fetches      1 -> {fetches}");
    println!("  wall clock   {:?} -> {:?}", t_hop0, total);
    println!("  requirements {} -> {}", rep0.requirements_stored, rep0.requirements_stored + gained);
    let key = rep0.journal_key.unwrap_or_default();
    if let Ok(fp) = gaply_core::journal_fingerprint::fingerprint_for(&db, &key) {
        println!("  readable through the real reader: {}", fp.requirements.len());
        for r in fp.requirements.iter().take(6) {
            println!("    [{}] {} = {}", r.kind, format!("{:?}", r.article_type), r.value);
        }
    }
}
