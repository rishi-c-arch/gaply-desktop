//! **Is Nature Medicine's empty checklist the wrong URL, or an unfetchable page?**
//!
//! The stored "Author guidelines" document for `nature.com/nm` is 368 characters
//! of browser-compatibility banner. Two causes fit: the ingest was pointed at a
//! journal HOMEPAGE rather than a guidelines page, or nature.com serves a
//! JavaScript shell that no fetcher can read. They need different fixes, so they
//! are separated by measurement.
//!
//! Through `ReqwestFetcher` — the app's own client — because a curl result is a
//! fact about curl (CLAUDE.md).
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let f = ReqwestFetcher::new().expect("client");
    for url in std::env::args().skip(1) {
        let req = HttpRequest { url: url.clone(), headers: Vec::new() };
        match f.get(&req) {
            Ok(page) => {
                let text = app_lib::guidelines::html_to_text_public(&page.body);
                let head: String = text.chars().take(160).collect();
                println!(
                    "\n{:<62}\n  HTTP {}   html {:>7} chars   text {:>6} chars",
                    url,
                    page.status,
                    page.body.len(),
                    text.len()
                );
                println!("  head: {}", head.replace('\n', " "));
                // The INGEST path's own extractor, not the probe's: this is
                // what decides how much reaches the store.
                let blocks = app_lib::guidelines::html_to_blocks(&page.body);
                let btext: usize = blocks.iter().map(|b| b.text.len()).sum();
                println!("  html_to_blocks -> {} blocks, {} chars", blocks.len(), btext);
            }
            Err(e) => println!("\n{url}\n  FETCH ERROR: {e:?}"),
        }
    }
}
