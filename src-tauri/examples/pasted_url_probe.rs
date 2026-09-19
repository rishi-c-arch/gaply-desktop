//! **BEFORE/AFTER for §11 D192: a journal outside the seeded ten.**
//!
//! Drives the REAL `guidelines::ingest_with` with the REAL `ReqwestFetcher`
//! against a live author-guidelines page. Not curl: a curl 200 is a fact about
//! curl's TLS fingerprint, not about what Gaply can reach (CLAUDE.md).
//!
//! BEFORE is `journal_name: None` — the path as it shipped, where the page
//! reaches the RAG corpus and the requirement extractor is never called.
//! AFTER is the same fetch with the journal named.

use app_lib::guidelines;
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::embed::HashEmbedder;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::Database;

fn main() {
    let mut a = std::env::args().skip(1);
    let url = a.next().expect("usage: pasted_url_probe <guidelines-url> <journal name>");
    let name = a.next().expect("a journal name");
    let fetcher = ReqwestFetcher::new().expect("fetcher");
    let limiter = RateLimiter::new(10.0, 2.0);

    let before = guidelines::JournalIdentity::default();
    let after = guidelines::JournalIdentity { key: None, name: Some(name.as_str()) };
    for (label, journal) in [("BEFORE (as shipped)", before), ("AFTER (D192)", after)] {
        let db = Database::in_memory().expect("db");
        let rep = guidelines::ingest_with(
            &fetcher, &limiter, &db, &HashEmbedder, None, Some(&url), journal,
        );
        println!("\n=== {label} ===");
        println!("  any_ingested={}  note={}", rep.any_ingested, rep.note);
        // WHICH failure. "unavailable" covers a refused fetch, a bot
        // interstitial served with a 200, and a page classified as navigation —
        // three different facts, and only one of them is about Gaply.
        for r in &rep.results {
            println!("  verdict: {r:?}");
        }
        println!("  journal_key={:?}", rep.journal_key);
        println!("  requirements_stored={}  duplicate={}", rep.requirements_stored, rep.requirements_duplicate);
        if let Some(k) = &rep.journal_key {
            let fp = gaply_core::journal_fingerprint::fingerprint_for(&db, k).expect("fp");
            println!("  origin={:?}", fp.provenance.as_ref().map(|p| p.origin.clone()));
            println!("  rows readable through the real reader: {}", fp.requirements.len());
            for r in fp.requirements.iter().take(8) {
                println!("    [{}] {}  (article_type={:?})", r.kind, r.value, r.article_type);
                println!("        url:  {}", r.source_url);
                println!("        span: {}", one_line(&r.source_span, 150));
            }
        }
    }
}

fn one_line(s: &str, n: usize) -> String {
    let j = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if j.chars().count() <= n { j } else { j.chars().take(n).collect::<String>() + " …" }
}
