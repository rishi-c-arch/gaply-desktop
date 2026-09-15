//! **Calibrating the retrieval query against the real claims.**
//!
//! The first attempt joined every distinctive term and returned noise for one
//! claim and `NotFound` for the other. That is the instrument, not the corpus,
//! so this prints what each candidate query construction retrieves before any
//! rule is written against it.

use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::extract::{self, docparse, SectionKind};
use gaply_core::novelty;
use gaply_core::refverify::{ApiRateLimiters, ConnectorOutcome, VerifyContext};
use gaply_core::Database;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::env::temp_dir().join("gaply-novelty-scan");
    std::fs::create_dir_all(&dir).unwrap();
    let db = Database::open(&dir.join("scan.db")).unwrap();
    let http = ReqwestFetcher::new().unwrap();
    let limiters = ApiRateLimiters::with_polite_defaults();
    let ctx = VerifyContext { db: &db, http: &http, limiters: &limiters, contact_email: None };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

    for p in &args {
        let text = docparse::parse_path(std::path::Path::new(p)).unwrap();
        let ex = extract::extract_from_text(&text);
        // Topic vocabulary: terms of the title + abstract.
        let mut topic = String::new();
        if let Some(t) = &ex.title { topic.push_str(t); topic.push(' '); }
        for s in ex.sections.iter().filter(|s| s.kind == SectionKind::Abstract) {
            for para in &s.paragraphs { topic.push_str(para); topic.push(' '); }
        }
        let topic_terms = novelty::distinctive_terms(&topic);
        println!("=== {} ===", short(p));
        println!("  title: {:?}", ex.title);
        println!("  topic terms ({}): {:?}\n", topic_terms.len(), topic_terms.iter().take(40).collect::<Vec<_>>());

        for c in &novelty::extract_claims(&ex) {
            println!("  CLAIM: {}", c.sentence);
            let shared: Vec<String> =
                c.terms.iter().filter(|t| topic_terms.contains(t)).cloned().collect();
            println!("  claim∩topic ({}): {:?}", shared.len(), shared);
            let candidates: Vec<(&str, String)> = vec![
                ("all terms", c.terms.join(" ")),
                ("claim∩topic", shared.join(" ")),
                ("first 4 terms", c.terms.iter().take(4).cloned().collect::<Vec<_>>().join(" ")),
                ("longest 4 terms", longest(&c.terms, 4).join(" ")),
            ];
            for (name, q) in candidates {
                if q.trim().is_empty() { println!("    [{name}] EMPTY QUERY"); continue; }
                match gaply_core::refverify::openalex_search(&ctx, &q, 5, now) {
                    Ok(ConnectorOutcome::Found(w)) => {
                        println!("    [{name}] q={q:?} -> {} work(s)", w.len());
                        for x in w.iter().take(5) {
                            println!("        [{}] {}", x.publication_year.map(|y|y.to_string()).unwrap_or("????".into()),
                                x.title.as_ref().map(|t| t.display_raw().to_string()).unwrap_or_default());
                        }
                    }
                    Ok(o) => println!("    [{name}] q={q:?} -> {o:?}"),
                    Err(e) => println!("    [{name}] q={q:?} -> ERROR {e}"),
                }
            }
            println!();
        }
    }
}

fn longest(terms: &[String], n: usize) -> Vec<String> {
    let mut v = terms.to_vec();
    v.sort_by_key(|t| std::cmp::Reverse(t.len()));
    v.truncate(n);
    v
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
