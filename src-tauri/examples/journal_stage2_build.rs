//! **Stage 2 — build and STORE fingerprints for ten journals.**
//!
//! §7/§12 record "ten journal fingerprints needed, one exists". One did not
//! exist: every Phase-3 measurement ran through `Database::in_memory()` inside
//! a probe, so nothing was ever persisted (§11 D170). This writes to a real
//! on-disk database and reports what is in it afterwards.
//!
//! **It never touches the live app database.** The path is an argument with no
//! default that points anywhere near `~/Library/Application Support`; applying
//! migrations to a user's real database is a separate decision.
//!
//! Live network. Run:
//!   cargo run -p app --release --example journal_stage2_build -- /tmp/stage2.db
use app_lib::guidelines::{html_to_blocks, html_title};
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use app_lib::journal_openalex::{recent_papers, source_id_for_issn};
use gaply_core::journal_corpus::{derive_conventions, CorpusBounds};
use gaply_core::journal_expect::{extract_expectations, is_reviewer_guidance};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::journal_fingerprint::{fingerprint_for, FingerprintProvenance};
use gaply_core::journal_standards::bindings_from;
use gaply_core::journal_store::{
    store_conventions, store_expectations, store_fingerprint_provenance, store_requirements,
    store_standard_bindings,
};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::Database;
use sha2::{Digest, Sha256};

/// The same ten as `journal_crawl_probe.rs`, plus the ISSN the convention
/// profile needs. **The ISSNs are unverified input** — the run reports whether
/// OpenAlex resolved each one, so a wrong ISSN shows as a resolution failure
/// rather than as a journal with no conventions.
const JOURNALS: &[(&str, &str, &str)] = &[
    ("plos-one", "https://journals.plos.org/plosone/s/submission-guidelines", "1932-6203"),
    ("plos-medicine", "https://journals.plos.org/plosmedicine/s/submission-guidelines", "1549-1676"),
    ("nature-medicine", "https://www.nature.com/nm/for-authors", "1546-170X"),
    ("nature-communications", "https://www.nature.com/ncomms/submission-guidelines", "2041-1723"),
    ("bmj", "https://www.bmj.com/about-bmj/resources-authors", "1756-1833"),
    ("lancet", "https://www.thelancet.com/lancet/information-for-authors", "1474-547X"),
    ("statistics-in-medicine", "https://onlinelibrary.wiley.com/page/journal/10970258/homepage/forauthors.html", "1097-0258"),
    ("frontiers-public-health", "https://www.frontiersin.org/journals/public-health/for-authors/author-guidelines", "2296-2565"),
    ("j-health-psychology", "https://journals.sagepub.com/author-instructions/JHP", "1461-7277"),
    ("bmc-public-health", "https://bmcpublichealth.biomedcentral.com/submission-guidelines", "1471-2458"),
];

/// One quarter, per §7's "once per journal per quarter".
const REFETCH_AFTER_SECS: i64 = 90 * 24 * 60 * 60;

struct Row {
    key: &'static str,
    pages: usize,
    guideline_pages: usize,
    requirements: usize,
    conflicted: usize,
    conventions: usize,
    bindings: usize,
    expectations: usize,
    issn_resolved: bool,
    papers: usize,
    provenance: bool,
}

fn main() {
    let path = std::env::args().nth(1).expect(
        "usage: journal_stage2_build <db-path>   (NOT the live app database)",
    );
    assert!(
        !path.contains("Application Support"),
        "refusing to write to the live app database — that is a separate decision"
    );
    let db = Database::open(std::path::Path::new(&path)).expect("open db");
    let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
    let cfg: serde_json::Value =
        serde_json::from_str(include_str!("../config/journal-crawl.json")).unwrap();
    let bounds = CorpusBounds::from_json(&cfg["corpus_bounds"].to_string()).expect("bounds");
    let f = ReqwestFetcher::new().unwrap();
    let limiter = RateLimiter::new(2.0, 2.0);
    let now = 1_789_200_000i64;

    let mut rows: Vec<Row> = Vec::new();

    for (key, entry, issn) in JOURNALS {
        eprintln!("--- {key}");
        let mut row = Row {
            key,
            pages: 0,
            guideline_pages: 0,
            requirements: 0,
            conflicted: 0,
            conventions: 0,
            bindings: 0,
            expectations: 0,
            issn_resolved: false,
            papers: 0,
            provenance: false,
        };

        // --- guidelines: requirements, bindings, expectations ---------------
        let mut hasher = Sha256::new();
        let mut sources = 0usize;
        match crawl(&f, &limiter, &budget, entry) {
            Err(e) => eprintln!("    crawl failed: {e}"),
            Ok(o) => {
                row.pages = o.fetched;
                row.guideline_pages = o.guideline;
                for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
                    let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
                    let blocks = html_to_blocks(&r.body);
                    let reqs = extract_requirements(&blocks);
                    if !reqs.is_empty() {
                        let out = store_requirements(&db, key, &p.url, &reqs, now).expect("store");
                        row.requirements += out.inserted;
                        sources += 1;
                        hasher.update(p.url.as_bytes());
                        for q in &reqs {
                            hasher.update(q.value.as_bytes());
                        }
                    }
                    let b = bindings_from(&reqs);
                    if !b.is_empty() {
                        row.bindings +=
                            store_standard_bindings(&db, key, &p.url, &b, now).expect("bind");
                    }
                    // Expectations come only from REVIEWER guidance. The gate
                    // over-fires (§11 D163) and is used here anyway, because the
                    // alternative is filing author instructions as expectations
                    // — §7's separation lost at the first step.
                    let title = html_title(&r.body);
                    if is_reviewer_guidance(&p.url, &title, &r.body) {
                        let ex = extract_expectations(&blocks);
                        if !ex.is_empty() {
                            row.expectations +=
                                store_expectations(&db, key, &p.url, &ex, now).expect("expect");
                        }
                    }
                }
            }
        }

        // --- conventions from the comparable corpus -------------------------
        match source_id_for_issn(&f, issn) {
            Err(e) => eprintln!("    ISSN {issn} did not resolve: {e}"),
            Ok(src) => {
                row.issn_resolved = true;
                match recent_papers(&f, &src, "2025-03-01", bounds.maximum) {
                    Err(e) => eprintln!("    recent_papers failed: {e}"),
                    Ok(papers) => {
                        row.papers = papers.len();
                        let convs = derive_conventions(&papers, &bounds);
                        row.conventions =
                            store_conventions(&db, key, "stage2", &convs, now).expect("conv");
                    }
                }
            }
        }

        // --- provenance: the row that says this journal WAS crawled ---------
        if sources > 0 {
            store_fingerprint_provenance(
                &db,
                &FingerprintProvenance {
                    journal_key: key.to_string(),
                    version: 1,
                    content_hash: format!("{:x}", hasher.finalize()),
                    fetched_at: now,
                    refetch_after: now + REFETCH_AFTER_SECS,
                    source_count: sources as i64,
                    quarantined_at: None,
                    quarantine_reason: None,
                },
            )
            .expect("provenance");
        }

        // Read back through the REAL reader, not through the counters above.
        let fp = fingerprint_for(&db, key).expect("fingerprint");
        row.conflicted = fp.conflicts.iter().map(|c| c.values.len()).sum();
        row.provenance = fp.provenance.is_some();
        rows.push(row);
    }

    println!("\n================== STAGE 2: TEN JOURNAL FINGERPRINTS ==================");
    println!(
        "{:<24} {:>5} {:>5} {:>5} {:>6} {:>5} {:>5} {:>5} {:>5}",
        "journal", "pages", "guid", "reqs", "confl", "conv", "bind", "expct", "prov"
    );
    for r in &rows {
        println!(
            "{:<24} {:>5} {:>5} {:>5} {:>6} {:>5} {:>5} {:>5} {:>5}",
            r.key,
            r.pages,
            r.guideline_pages,
            r.requirements,
            r.conflicted,
            r.conventions,
            r.bindings,
            r.expectations,
            if r.provenance { "yes" } else { "NO" }
        );
    }

    let n = rows.len();
    let reqs: usize = rows.iter().map(|r| r.requirements).sum();
    let confl: usize = rows.iter().map(|r| r.conflicted).sum();
    let with_reqs = rows.iter().filter(|r| r.requirements > 0).count();
    let with_conf = rows.iter().filter(|r| r.conflicted > 0).count();
    let with_prov = rows.iter().filter(|r| r.provenance).count();
    println!("\n  journals attempted              {n}");
    println!("  with >=1 stored requirement     {with_reqs}");
    println!("  with provenance (were crawled)  {with_prov}");
    println!("  without provenance              {}", n - with_prov);
    println!("  ISSN resolved                   {}", rows.iter().filter(|r| r.issn_resolved).count());
    println!("\n  requirements total              {reqs}");
    println!("  conventions total               {}", rows.iter().map(|r| r.conventions).sum::<usize>());
    println!("  bindings total                  {}", rows.iter().map(|r| r.bindings).sum::<usize>());
    println!("  expectations total              {}", rows.iter().map(|r| r.expectations).sum::<usize>());
    println!("\n  CONFLICTED rows                 {confl}");
    println!("  journals with >=1 conflict      {with_conf} of {n}");
    if reqs > 0 {
        println!("  conflict rate (rows)            {:.2}%", 100.0 * confl as f64 / reqs as f64);
    }
    println!("=======================================================================");
}
