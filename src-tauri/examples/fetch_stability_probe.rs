//! **Are the 403s intermittent, rate-limiting, or a fingerprint?**
//!
//! They were 200s this morning and are 403s now, from the same fetcher. The
//! three explanations have different consequences:
//!
//! * **intermittent / rate-limited** — every journal count in Phase 3 is a
//!   SAMPLE, and a count taken at a different hour would differ;
//! * **a fingerprint on something that changed** — the counts are reproducible
//!   but the fetcher is now wrong for those publishers;
//! * **genuinely varying** — the counts carry an error bar nobody has stated.
//!
//! A retry loop is not the answer to any of them until it is known which. A
//! retry against a fingerprint just spends the budget.
//!
//! Variables separated: TIME (rounds spaced apart), COOKIE STATE (a fresh
//! client per request vs one reused across the run), and a WARM-UP (the site
//! root before the target, which is how a browser arrives).
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

const TARGETS: &[(&str, &str, &str)] = &[
    ("BMJ", "https://www.bmj.com", "https://www.bmj.com/about-bmj/resources-authors"),
    ("SAGE", "https://journals.sagepub.com", "https://journals.sagepub.com/author-instructions/JHP"),
    ("Wiley", "https://onlinelibrary.wiley.com", "https://onlinelibrary.wiley.com/page/journal/10970258/homepage/forauthors.html"),
    // Controls: these answered 200 all day.
    ("PLOS (control)", "https://journals.plos.org", "https://journals.plos.org/plosone/s/submission-guidelines"),
    ("Nature (control)", "https://www.nature.com", "https://www.nature.com/nm/content"),
];

fn code(f: &ReqwestFetcher, url: &str) -> u16 {
    f.get(&HttpRequest::get(url)).map(|r| r.status).unwrap_or(0)
}

fn main() {
    let rounds: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let gap = std::time::Duration::from_secs(45);
    // One client reused for the whole run: its cookie jar accumulates.
    let shared = ReqwestFetcher::new().expect("client");

    println!("{:<18} {:>6} {:>8} {:>8} {:>10}", "TARGET", "round", "fresh", "shared", "warmed");
    for r in 0..rounds {
        for (name, root, target) in TARGETS {
            // (a) a brand-new client every time — no cookie history at all
            let fresh = ReqwestFetcher::new().expect("client");
            let a = code(&fresh, target);
            // (b) the client that has been running all along
            let b = code(&shared, target);
            // (c) a new client that visits the root first, as a browser does
            let warm = ReqwestFetcher::new().expect("client");
            let _ = code(&warm, root);
            let c = code(&warm, target);
            println!("{name:<18} {r:>6} {a:>8} {b:>8} {c:>10}");
        }
        if r + 1 < rounds {
            std::thread::sleep(gap);
        }
    }
}
