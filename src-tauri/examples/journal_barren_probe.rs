//! **Read the pages that were admitted and yielded nothing.**
//!
//! `journal_extract_probe` produced the count — Nature Medicine: 51 pages
//! classified `guideline`, 12 yielding a requirement, 39 yielding none. A count
//! cannot say WHICH of three things a barren page is, and the three have
//! opposite fixes:
//!
//! 1. real guidance the EXTRACTOR missed — fix extraction, not the gate
//! 2. not guidance at all — a gate error
//! 3. guidance that states no extractable requirement — neither; correct
//!
//! So this dumps each barren page's title, its headings, and **every
//! obligation-shaped sentence it contains**, because that last one is the
//! discriminator between (1) and (3): a page full of "must" sentences that
//! yielded nothing is an extractor gap; a page with none is genuinely
//! requirement-free. Classification is done by READING the output, not by this
//! program — a heuristic written here would be the same fitting-to-the-read
//! mistake the gate was told not to make.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

/// Words that make a sentence an obligation rather than a description.
const MODALS: &[&str] = &[
    " must ", " must:", " should ", " should:", " required ", " require ", " requires ",
    " shall ", " may not ", " cannot ", " need to ", " are expected to ", " is expected to ",
    " please ", " do not ", " ensure ",
];

fn sentences(text: &str) -> Vec<String> {
    // Deliberately crude: this is a reading aid, not a pipeline stage.
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if ch == '.' || ch == '?' || ch == '!' {
            let t = cur.trim().to_string();
            if t.len() > 20 {
                out.push(t);
            }
            cur.clear();
        }
    }
    let t = cur.trim().to_string();
    if t.len() > 20 {
        out.push(t);
    }
    out
}

fn main() {
    let entry = std::env::args().nth(1).expect("usage: journal_barren_probe <entry-url>");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let f = ReqwestFetcher::new().unwrap();
    let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, &entry).unwrap();

    let mut barren = 0usize;
    let mut fertile = 0usize;

    for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
        let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
        let blocks = html_to_blocks(&r.body);
        let reqs = extract_requirements(&blocks);
        if !reqs.is_empty() {
            fertile += 1;
            continue;
        }
        barren += 1;

        let title = app_lib::guidelines::html_title(&r.body);
        let body: String = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join(" ");
        let chars = body.chars().count();

        println!("\n================================================================");
        println!("[{barren:02}] {}", p.url);
        println!("     title   : {title}");
        println!("     blocks  : {}  chars: {chars}", blocks.len());
        println!("     headings:");
        for b in blocks.iter().take(40) {
            if !b.heading.trim().is_empty() {
                println!("        - {}", b.heading.trim());
            }
        }

        let mut hits = 0usize;
        println!("     obligation-shaped sentences:");
        for s in sentences(&body) {
            let padded = format!(" {} ", s.to_lowercase());
            if MODALS.iter().any(|m| padded.contains(m)) {
                hits += 1;
                if hits <= 8 {
                    let s: String = s.chars().take(260).collect();
                    println!("        * {s}");
                }
            }
        }
        if hits == 0 {
            println!("        (none)");
        } else if hits > 8 {
            println!("        … and {} more", hits - 8);
        }
        println!("     MODAL-SENTENCE COUNT: {hits}");
    }

    println!("\n================================================================");
    println!("guideline pages: {} fertile, {} barren", fertile, barren);
}
