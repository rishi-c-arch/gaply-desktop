//! **The known-good control for the novelty retriever.**
//!
//! Every batch needs an input whose answer you already know, and this is it for
//! OpenAlex search. The control is `CONSORT statement reporting randomised
//! trials`, which must return *"CONSORT 2010 Statement: updated guidelines for
//! reporting parallel group randomised trials"* at or near rank 1:
//!
//! ```text
//! cargo run --release --example oa_query_try -- CONSORT statement reporting randomised trials
//! ```
//!
//! It earned its place immediately. The first novelty queries returned
//! groundwater ecosystems for a limnology claim and `NotFound` for an ML one,
//! and the question was whether the retriever was broken or the claims were
//! genuinely novel. This control answered it in one run — rank 1, 13,614
//! citations — which said the retriever was fine and the QUERY CONSTRUCTION was
//! the instrument at fault. Without it, "retrieval found nothing" would have
//! gone into a decision record as a result about the field.
//!
//! Through `ReqwestFetcher`, never curl (§11).
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::refverify::{ApiRateLimiters, ConnectorOutcome, VerifyContext};
use gaply_core::Database;

fn main() {
    let q: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::env::temp_dir().join("gaply-novelty-scan");
    std::fs::create_dir_all(&dir).unwrap();
    let db = Database::open(&dir.join("scan.db")).unwrap();
    let http = ReqwestFetcher::new().unwrap();
    let limiters = ApiRateLimiters::with_polite_defaults();
    let ctx = VerifyContext { db: &db, http: &http, limiters: &limiters, contact_email: None };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let query = q.join(" ");
    println!("query: {query:?}");
    match gaply_core::refverify::openalex_search(&ctx, &query, 8, now) {
        Ok(ConnectorOutcome::Found(w)) => for x in &w {
            println!("  [{}] cited={} {}", x.publication_year.map(|y|y.to_string()).unwrap_or("????".into()),
                x.cited_by_count.unwrap_or(0),
                x.title.as_ref().map(|t| t.display_raw().to_string()).unwrap_or_default());
        },
        Ok(o) => println!("  {o:?}"),
        Err(e) => println!("  ERROR {e}"),
    }
}
