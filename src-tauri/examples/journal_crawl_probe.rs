//! Crawl ten journals and report what fetch-and-classify actually found.
//!
//! The number that matters is `lexicon_misses`: pages classified as guideline
//! content that no lexicon term would have admitted. Each one is a page the
//! discovery rule in §3.4 / Prompt 5 would have skipped without saying so.
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget, StoppedBy};
use gaply_core::ratelimit::RateLimiter;

const JOURNALS: &[(&str, &str)] = &[
    ("PLOS ONE", "https://journals.plos.org/plosone/s/submission-guidelines"),
    ("PLOS Medicine", "https://journals.plos.org/plosmedicine/s/submission-guidelines"),
    ("Nature Medicine", "https://www.nature.com/nm/for-authors"),
    ("Nature Communications", "https://www.nature.com/ncomms/submission-guidelines"),
    ("The BMJ", "https://www.bmj.com/about-bmj/resources-authors"),
    ("The Lancet", "https://www.thelancet.com/lancet/information-for-authors"),
    ("Statistics in Medicine", "https://onlinelibrary.wiley.com/page/journal/10970258/homepage/forauthors.html"),
    ("Frontiers in Public Health", "https://www.frontiersin.org/journals/public-health/for-authors/author-guidelines"),
    ("J. Health Psychology (SAGE)", "https://journals.sagepub.com/author-instructions/JHP"),
    ("BMC Public Health", "https://bmcpublichealth.biomedcentral.com/submission-guidelines"),
];

fn main() {
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json"))
        .expect("config/journal-crawl.json");
    println!("budget: max_pages={} max_depth={}\n", budget.max_pages, budget.max_depth);
    let f = ReqwestFetcher::new().expect("client");
    // Polite: one request per second per host, well under any published limit.
    let limiter = RateLimiter::new(2.0, 2.0);

    let (mut tot_f, mut tot_g, mut tot_m, mut budget_bound) = (0, 0, 0, Vec::new());
    println!(
        "{:<28}{:>7}{:>11}{:>10}{:>9}{:>9}  {}",
        "JOURNAL", "FETCH", "GUIDELINE", "LEX-MISS", "NAV", "INTER", "STOPPED BY"
    );
    for (name, entry) in JOURNALS {
        match crawl(&f, &limiter, &budget, entry) {
            Ok(o) => {
                tot_f += o.fetched;
                tot_g += o.guideline;
                tot_m += o.lexicon_misses;
                if o.rate_limited {
                    println!("      (rate-limited: the crawl stopped waiting for a token)");
                }
                if o.stopped_by == StoppedBy::Budget {
                    budget_bound.push((*name, o.unvisited));
                }
                println!(
                    "{:<28}{:>7}{:>11}{:>10}{:>9}{:>9}  {}",
                    name, o.fetched, o.guideline, o.lexicon_misses, o.navigation, o.interstitial,
                    match o.stopped_by {
                        StoppedBy::Budget => format!("BUDGET ({} left)", o.unvisited),
                        StoppedBy::FrontierExhausted => "domain exhausted".into(),
                    }
                );
                for p in o.pages.iter().filter(|p| p.verdict == "guideline" && !p.lexicon_hit) {
                    println!("      lexicon-miss  d{}  {:<34} {}", p.depth,
                             p.anchor.chars().take(32).collect::<String>(), p.url);
                }
            }
            Err(e) => println!("{name:<28}  ERROR {e}"),
        }
    }
    println!("\nTOTAL fetched={tot_f}  guideline={tot_g}  lexicon-misses={tot_m}");
    if budget_bound.is_empty() {
        println!("No crawl was ended by its budget — every journal's domain was exhausted.");
    } else {
        println!("ENDED BY BUDGET (coverage NOT claimable for these):");
        for (n, left) in &budget_bound {
            println!("  {n} — {left} candidates unvisited");
        }
    }
}
