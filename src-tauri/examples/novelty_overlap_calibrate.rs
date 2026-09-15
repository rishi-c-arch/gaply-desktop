//! **What term overlap do real retrievals actually produce?**
//!
//! Run before the narrowing rule is written. A claim is NARROWED by a specific
//! paper, so the rule needs a threshold, and a threshold chosen in a design
//! document is the thing this project exists not to do.
//!
//! Prints, per claim, every retrieved work with how many of the claim's terms
//! appear in its title and in its abstract — and whether OpenAlex carries an
//! abstract at all, which decides whether "what it showed" can ever be stated.

use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::extract::{self, docparse};
use gaply_core::novelty;
use gaply_core::refverify::{ApiRateLimiters, VerifyContext};
use gaply_core::Database;

fn main() {
    let dir = std::env::temp_dir().join("gaply-novelty-cal");
    std::fs::create_dir_all(&dir).unwrap();
    let db = Database::open(&dir.join("c.db")).unwrap();
    let http = ReqwestFetcher::new().unwrap();
    let limiters = ApiRateLimiters::with_polite_defaults();
    let ctx = VerifyContext { db: &db, http: &http, limiters: &limiters, contact_email: None };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    let (mut works, mut with_abstract) = (0usize, 0usize);

    for p in std::env::args().skip(1) {
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        for c in &novelty::extract_claims(&ex) {
            println!("\n=== {} ===", short(&p));
            println!("CLAIM: {}", c.sentence);
            println!("terms ({}): {:?}", c.terms.len(), c.terms);
            println!("query: {:?}\n", novelty::retrieval_query(c));
            let retrieved = novelty::retrieve(&ctx, c, 10, now).unwrap_or_default();
            for (i, w) in retrieved.iter().enumerate() {
                works += 1;
                let tl = w.title.to_lowercase();
                let ab = w.showed.clone().unwrap_or_default().to_lowercase();
                if w.showed.is_some() {
                    with_abstract += 1;
                }
                let in_title: Vec<&String> =
                    c.terms.iter().filter(|t| tl.contains(t.as_str())).collect();
                let in_abs: Vec<&String> =
                    c.terms.iter().filter(|t| ab.contains(t.as_str())).collect();
                let either: Vec<&String> = c
                    .terms
                    .iter()
                    .filter(|t| tl.contains(t.as_str()) || ab.contains(t.as_str()))
                    .collect();
                println!(
                    "  #{:<2} title {}/{}  abstract {}/{}  either {}/{}  abstract? {}",
                    i + 1,
                    in_title.len(), c.terms.len(),
                    in_abs.len(), c.terms.len(),
                    either.len(), c.terms.len(),
                    if w.showed.is_some() { "yes" } else { "NO" }
                );
                println!("       {}", w.cite());
                if !either.is_empty() {
                    println!("       matched: {either:?}");
                }
            }
        }
    }
    println!(
        "\nOPENALEX ABSTRACT COVERAGE: {with_abstract} of {works} retrieved work(s) carry one"
    );
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
