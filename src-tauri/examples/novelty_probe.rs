//! **§4.6's novelty pipeline over the real corpus, claim by claim.**
//!
//! Rows with spans, never a total — and never a number: the output per claim is
//! the claim, the nearest prior work, and a status.
//!
//! Retrieval goes through `ReqwestFetcher` and the `refverify` cache and rate
//! limiter, never curl (§11).
//!
//! ```text
//! cargo run --release --example novelty_probe -- <manuscript>...
//! ```

use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::extract::{self, docparse};
use gaply_core::novelty::{self, NoveltyStatus};
use gaply_core::refverify::{ApiRateLimiters, VerifyContext};
use gaply_core::Database;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: novelty_probe <manuscript>...");
        std::process::exit(2);
    }
    let dir = std::env::temp_dir().join("gaply-novelty-probe");
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let db = Database::open(&dir.join("novelty.db")).expect("db");
    let http = ReqwestFetcher::new().expect("fetcher");
    let limiters = ApiRateLimiters::with_polite_defaults();
    let ctx = VerifyContext { db: &db, http: &http, limiters: &limiters, contact_email: None };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut by_status: Vec<(NoveltyStatus, usize)> = Vec::new();

    for p in &args {
        let text = match docparse::parse_path(std::path::Path::new(p)) {
            Ok(t) => t,
            Err(e) => {
                println!("=== {} ===\n  PARSE FAILED: {e}\n", short(p));
                continue;
            }
        };
        let ex = extract::extract_from_text(&text);
        let claims = novelty::extract_claims(&ex);
        println!(
            "=== {} ===\n  {} novelty claim(s), {} reference(s) in the bibliography",
            short(p),
            claims.len(),
            ex.references.len()
        );
        for c in &claims {
            let query = novelty::retrieval_query(c);
            let retrieved = match novelty::retrieve(&ctx, c, 10, now) {
                Ok(w) => w,
                Err(e) => {
                    println!("  RETRIEVAL ERROR: {e}");
                    Vec::new()
                }
            };
            let a = novelty::assess(c, &ex.references, &retrieved);
            let slot = by_status.iter_mut().find(|(s, _)| *s == a.status);
            match slot {
                Some((_, n)) => *n += 1,
                None => by_status.push((a.status, 1)),
            }

            println!("\n  CLAIM [{:?} p{}]", c.location.section, c.location.paragraph);
            println!("    {}", c.claim_span());
            println!("  QUERY: {query:?} -> {} retrieved", retrieved.len());
            println!("  STATUS: {}   (basis: {:?})", a.status.as_str(), a.basis);
            match &a.nearest_prior_work {
                Some(w) => {
                    println!("  NEAREST PRIOR WORK ({}): {}", w.source, w.cite());
                    println!("  WHAT IT SHOWED: {}", w.what_it_showed());
                }
                None => println!("  NEAREST PRIOR WORK: none retrieved"),
            }
            if let Some(n) = &a.narrowing {
                println!("  NARROWING: {n}");
            }
            if let Some(n) = &a.states_no_scope {
                println!("  PHRASING (not a novelty verdict): {n}");
            }
            println!("  UNCERTAINTY: {}", a.uncertainty);
        }
        println!();
    }

    println!("STATUS TALLY (rows above are the evidence; this is a summary of them)");
    let total: usize = by_status.iter().map(|(_, n)| *n).sum();
    for (s, n) in &by_status {
        println!("  {:<30} {n} of {total}", s.as_str());
    }
    let unverified = by_status
        .iter()
        .find(|(s, _)| *s == NoveltyStatus::Unverified)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    if total > 0 {
        println!(
            "\n  UNVERIFIED is {unverified} of {total}. Retrieval finding nothing is not \n  \
             evidence of novelty — it is a fact about one index and one bag-of-terms query."
        );
    }
}

fn short(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}
