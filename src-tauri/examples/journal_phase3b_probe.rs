//! Two measurements before building items 4 and 5.
//!
//! **Item 4:** how many of the ten journals publish reviewer guidance at all?
//! If most do not, an empty `journal_expectations` table is the result, and the
//! corpus must not be allowed to fill the gap — an expectation derived from
//! recent papers is a CONVENTION and belongs in the other table (§7).
//!
//! **Item 5:** do journals BIND a reporting standard to a study design, or just
//! name standards? Nature Medicine's spans each named their design. If that
//! generalises, binding is mechanical; if most journals list standards without
//! saying when they apply, the binding is a judgement nobody has made.
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::journal_extract::{extract_requirements, RequirementKind};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

const JOURNALS: &[(&str, &str)] = &[
    ("PLOS ONE", "https://journals.plos.org/plosone/s/submission-guidelines"),
    ("PLOS Medicine", "https://journals.plos.org/plosmedicine/s/submission-guidelines"),
    ("Nature Medicine", "https://www.nature.com/nm/for-authors"),
    ("Nature Communications", "https://www.nature.com/ncomms/submit"),
    ("The BMJ", "https://www.bmj.com/about-bmj/resources-authors"),
    ("The Lancet", "https://www.thelancet.com/lancet/information-for-authors"),
    ("Statistics in Medicine", "https://onlinelibrary.wiley.com/page/journal/10970258/homepage/forauthors.html"),
    ("Frontiers in Public Health", "https://www.frontiersin.org/journals/public-health/for-authors/author-guidelines"),
    ("J. Health Psychology (SAGE)", "https://journals.sagepub.com/author-instructions/JHP"),
    ("BMC Public Health", "https://bmcpublichealth.biomedcentral.com/submission-guidelines"),
];

/// Anchors/paths that name guidance FOR REVIEWERS, not for authors.
const REVIEWER: &[&str] =
    &["reviewer", "referee", "peer-review", "peer review", "for-reviewers", "review process"];

/// Study designs a standard can be bound to.
const DESIGNS: &[&str] = &[
    "randomised", "randomized", "clinical trial", "trials", "systematic review",
    "meta-analys", "observational", "cohort", "case-control", "cross-sectional",
    "animal", "prediction model", "diagnostic", "economic evaluation", "qualitative",
    "biomarker", "protocol",
];

fn main() {
    let budget_src = include_str!("../config/journal-crawl.json");
    let mut budget = CrawlBudget::from_json(budget_src).unwrap();
    budget.max_pages = 60; // a measurement, not a corpus build
    let f = ReqwestFetcher::new().unwrap();
    let limiter = RateLimiter::new(4.0, 4.0);

    let (mut with_reviewer, mut bound_total, mut unbound_total) = (0usize, 0usize, 0usize);
    println!("{:<28}{:>9}{:>11}{:>10}{:>10}", "JOURNAL", "REVIEWER", "STANDARDS", "BOUND", "UNBOUND");
    for (name, entry) in JOURNALS {
        let Ok(o) = crawl(&f, &limiter, &budget, entry) else {
            println!("{name:<28}  crawl failed");
            continue;
        };
        let reviewer_pages: Vec<&str> = o
            .pages
            .iter()
            .filter(|p| p.verdict == "guideline")
            .filter(|p| {
                let hay = format!("{} {}", p.url, p.anchor).to_lowercase();
                REVIEWER.iter().any(|r| hay.contains(r))
            })
            .map(|p| p.url.as_str())
            .collect();
        if !reviewer_pages.is_empty() {
            with_reviewer += 1;
        }

        let (mut bound, mut unbound) = (0usize, 0usize);
        let mut examples: Vec<String> = Vec::new();
        for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
            let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
            for req in extract_requirements(&html_to_blocks(&r.body)) {
                if req.kind != RequirementKind::ReportingStandard {
                    continue;
                }
                let span = req.source_span.to_lowercase();
                if DESIGNS.iter().any(|d| span.contains(d)) {
                    bound += 1;
                    if examples.len() < 2 {
                        examples.push(format!("{} — {}", req.value, req.source_span.chars().take(96).collect::<String>()));
                    }
                } else {
                    unbound += 1;
                    if examples.len() < 2 {
                        examples.push(format!("{} (UNBOUND) — {}", req.value, req.source_span.chars().take(90).collect::<String>()));
                    }
                }
            }
        }
        bound_total += bound;
        unbound_total += unbound;
        println!("{name:<28}{:>9}{:>11}{:>10}{:>10}", reviewer_pages.len(), bound + unbound, bound, unbound);
        for u in reviewer_pages.iter().take(2) {
            println!("      reviewer page: {u}");
        }
        for e in &examples {
            println!("      {e}");
        }
    }
    println!(
        "\njournals publishing reviewer guidance: {with_reviewer}/{}\n\
         reporting-standard spans: {bound_total} name a study design, {unbound_total} do not",
        JOURNALS.len()
    );
}
