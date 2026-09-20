//! **What did the fetcher actually receive?** Status, byte count, and the head
//! of the body — the three facts a bucket count cannot carry.
//!
//! Written because the §11 D192 follow-up's known-good control came back
//! `Ingested { chunks: 1 }` on a page the seeded crawl got 39 requirements from.
//! One chunk on a real guidelines page is the throttle-stub signature CLAUDE.md
//! records twice (19 BMJ URLs at exactly 276 characters; seven at exactly
//! 12,361), and a count cannot tell a stub from a page that says nothing.
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let f = ReqwestFetcher::new().expect("client");
    for url in std::env::args().skip(1) {
        match f.get(&HttpRequest::get(&url)) {
            Ok(r) => {
                let text = app_lib::guidelines::html_to_text_public(&r.body);
                println!("\n{url}");
                println!("  status={} html_bytes={} text_chars={}", r.status, r.body.len(), text.len());
                if let Ok(needle) = std::env::var("GAPLY_PEEK_NEEDLE") {
                    let low = text.to_lowercase();
                    let mut from = 0usize;
                    let mut n = 0;
                    while let Some(rel) = low[from..].find(&needle.to_lowercase()) {
                        let at = from + rel;
                        let lo = at.saturating_sub(120);
                        let hi = (at + 160).min(text.len());
                        let lo = (0..=lo).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
                        let hi = (hi..=text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(text.len());
                        println!("  ...{}...", text[lo..hi].replace('\n', " "));
                        from = at + needle.len();
                        n += 1;
                        if n >= 6 { break }
                    }
                    if n == 0 { println!("  needle {needle:?} not present in the text"); }
                } else {
                    let head: String = text.chars().take(320).collect();
                    println!("  text head: {}", head.replace('\n', " "));
                }
            }
            Err(e) => println!("\n{url}\n  ERROR {e}"),
        }
    }
}
