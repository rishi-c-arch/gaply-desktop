//! **Which REAL author-guidance pages does the injection scanner refuse, and why?**
//!
//! §11 D193 measured six of twenty sampled journals quarantined, every one a live
//! `guide-for-authors` page. `sanitize.rs` already documents one false positive of
//! this family — PLOS ONE's *"supporting figures ... do not follow the same
//! requirements as tables and figures in the main body"* — and says an injection
//! "names the INSTRUCTIONS it wants ignored". This is the second witness that
//! entry asked for, taken across a corpus rather than a fixture.
//!
//! The verdict comes from the REAL predicate (`sanitize::scan_injections`). The
//! windows printed afterwards are diagnosis of the page, not a second copy of the
//! rule — a parallel predicate would be the thing CLAUDE.md forbids.
use app_lib::guidelines::html_to_text_public;
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::sanitize::scan_injections;

/// The phrases `refuses_instructions` keys on, used ONLY to show the reader which
/// sentence is involved once the real predicate has already fired.
const PHRASES: &[&str] =
    &["do not follow", "don't follow", "do not obey", "do not comply with"];

fn main() {
    let list = std::env::args().nth(1).expect("usage: injection_fp_probe <tsv>");
    let raw = std::fs::read_to_string(&list).expect("corpus");
    let f = ReqwestFetcher::new().expect("client");

    let (mut ok, mut flagged, mut unreadable) = (0, 0, 0);
    let mut sentences: Vec<String> = Vec::new();

    for line in raw.lines() {
        let mut c = line.split('\t');
        let (name, url, src) = (
            c.next().unwrap_or("?"),
            c.next().unwrap_or(""),
            c.next().unwrap_or(""),
        );
        if url.is_empty() {
            continue;
        }
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let body = match f.get(&HttpRequest::get(url)) {
            Ok(r) if r.status == 200 => r.body,
            Ok(r) => {
                unreadable += 1;
                println!("[{src}] {name}\n  UNREADABLE http {}", r.status);
                continue;
            }
            Err(e) => {
                unreadable += 1;
                println!("[{src}] {name}\n  UNREADABLE {e}");
                continue;
            }
        };
        let text = html_to_text_public(&body);
        let hits = scan_injections(&text);
        if hits.is_empty() {
            ok += 1;
            println!("[{src}] {name}\n  clean  ({} chars)", text.len());
            continue;
        }
        flagged += 1;
        println!("[{src}] {name}\n  FLAGGED {hits:?}  ({} chars)  {url}", text.len());
        if hits.iter().any(|h| h.contains("do not follow")) {
            let low = text.to_lowercase();
            for p in PHRASES {
                let mut from = 0usize;
                while let Some(rel) = low[from..].find(p) {
                    let at = from + rel;
                    let lo = (0..=at.saturating_sub(90)).next_back().unwrap_or(0);
                    let hi = (at + 150).min(text.len());
                    let lo = (0..=lo).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
                    let hi = (hi..=text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(text.len());
                    let w = text[lo..hi].replace('\n', " ");
                    println!("    [{p}] ...{w}...");
                    sentences.push(format!("{name}: {w}"));
                    from = at + p.len();
                }
            }
        }
    }
    println!("\n=== {ok} clean | {flagged} flagged | {unreadable} unreadable ===");
    println!("\n=== every sentence that reached the predicate ===");
    for s in &sentences {
        println!("  {s}");
    }
}
