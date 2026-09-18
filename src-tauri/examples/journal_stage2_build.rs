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
    source_count: i64,
    crawl_ran: bool,
    /// **The crawl gave up waiting for a rate-limit token. §11 D185.**
    ///
    /// `CrawlOutcome` has set this since the limiter was written, and this
    /// harness discarded it — so a STARVED journal printed exactly like an empty
    /// one: `plos-medicine  1 page, 0 reqs, prov: yes`. It has 33 requirements;
    /// it was crawled straight after `plos-one` on the same host and never got a
    /// token. `nature-communications` shows the same signature after
    /// `nature-medicine`. Both starved journals are second-on-host, which is the
    /// case `journal_crawl`'s own comment predicts by name.
    ///
    /// "We fetched nothing" and "there is nothing" are different answers, and a
    /// summary that cannot tell them apart is the instrument reporting a
    /// property of itself as a property of the world.
    rate_limited: bool,
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
    // **A HARDCODED CLOCK IN A MEASUREMENT HARNESS. §11 D184.**
    //
    // This read `1_789_200_000` — a constant. Every row this harness has ever
    // written carries it: all 213 requirements and all 10 fingerprints say
    // "fetched 2026-09-12T08:00:00Z", identical to the second across ten
    // publishers, which no real crawl produces. The 213 rows are REAL — their
    // `source_url`, `source_heading` and `source_span` come from actual pages —
    // but the time attached to them never happened.
    //
    // It cost nothing while the output stayed in `/tmp`. It became a problem the
    // moment that output was proposed as a SHIPPED seed, where a researcher
    // deciding whether to trust a nine-month-old word limit reads exactly this
    // field. A synthetic value wearing the shape of a measured one, inside the
    // instrument that produces the numbers everything else rests on — the
    // `exists()` family (§11 D172's stub that asserted its own answer).
    //
    // A real clock also makes the seed re-derivable: run it again and the
    // provenance says when, so a snapshot can be compared with the pages it came
    // from rather than taken on faith.
    //
    // **IT IS STILL ONE VALUE FOR THE WHOLE RUN, AND THAT IS DELIBERATE.** All
    // ten journals will carry this second, which trips the uniform-result tell
    // exactly as the constant did — so the difference is written down here
    // rather than left for the next reader to re-derive. The constant was false
    // about every row. This is TRUE about every row at the granularity the field
    // is for: `fetched_at` on a snapshot answers "how old is this", and a crawl
    // that takes an hour is one snapshot, not 213 independently-aged facts.
    // `refetch_after` is computed from it and means "this SNAPSHOT is stale
    // after", which is a claim about the run.
    //
    // What it must never be read as is a per-page fetch time. A row says the
    // snapshot began at this instant, not that its page was read at it. If a
    // consumer ever needs per-page timing, it has to come from the fetch itself
    // — `now` cannot be refined into it, and widening this value's meaning
    // instead would be the same move as the constant, one step smaller.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after 1970")
        .as_secs() as i64;

    let mut rows: Vec<Row> = Vec::new();

    // **TWO PASSES, and the second exists because the limiter is per-host. §11 D185.**
    //
    // A journal crawled straight after a sibling on the same host starts with an
    // empty bucket and gives up waiting: `plos-medicine` after `plos-one`
    // (journals.plos.org), `nature-communications` after `nature-medicine`
    // (www.nature.com). Both fetched ONE page and stored nothing, while holding
    // 33 requirements between them. Reordering the list would only move which
    // sibling starves; a second pass runs the starved keys once every other
    // journal has finished, by which time the bucket has refilled for free.
    //
    // Pass 2 visits ONLY journals pass 1 reported as rate-limited, so a journal
    // that genuinely has no requirements is not fetched twice to prove it.
    for pass in 1..=2u32 {
    for (key, entry, issn) in JOURNALS {
        if pass == 2 && !rows.iter().any(|r| r.key == *key && r.rate_limited) {
            continue;
        }
        if pass == 2 {
            eprintln!("--- {key} (second pass: rate-limited in pass 1)");
        } else {
            eprintln!("--- {key}");
        }
        let mut row = Row {
            rate_limited: false,
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
            source_count: 0,
            crawl_ran: false,
        };

        // --- guidelines: requirements, bindings, expectations ---------------
        let mut hasher = Sha256::new();
        // SOURCES CONSULTED, not sources that yielded something — §11 D173.
        let mut sources = 0usize;
        match crawl(&f, &limiter, &budget, entry) {
            Err(e) => eprintln!("    crawl failed: {e}"),
            Ok(o) => {
                row.crawl_ran = true;
                row.rate_limited = o.rate_limited;
                row.pages = o.fetched;
                row.guideline_pages = o.guideline;
                for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
                    // THE BODY THE CRAWL ALREADY FETCHED — §11 D174. This used
                    // to re-request every guideline URL from a host the crawl
                    // had just pulled 120 pages from. BMJ throttles that second
                    // pass and returns a 12,361-char stub for nearly every URL,
                    // so the extractor saw nothing and the journal recorded zero
                    // requirements — a defect in this runner that was read as a
                    // defect in the extractor and then in the classifier.
                    let Some(body) = p.body.as_deref() else { continue };
                    let blocks = html_to_blocks(body);
                    let reqs = extract_requirements(&blocks);
                    // A page FETCHED AND PARSED is a source, whether or not the
                    // extractor found anything in it. Counting only productive
                    // pages made a journal whose crawl succeeded and whose
                    // extraction returned nothing indistinguishable from one the
                    // crawler never reached.
                    sources += 1;
                    hasher.update(p.url.as_bytes());
                    if !reqs.is_empty() {
                        let out = store_requirements(&db, key, &p.url, &reqs, now).expect("store");
                        row.requirements += out.inserted;
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
                    let title = html_title(body);
                    if is_reviewer_guidance(&p.url, &title, body) {
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
        //
        // WRITTEN WHENEVER THE CRAWL RAN, with `source_count = 0` where that is
        // the truth (§11 D173). The three states a reader needs:
        //   no row            -> never crawled (the crawl errored or never ran)
        //   row, count = 0    -> crawled, no guideline page found
        //   row, count = N    -> crawled, N source documents consulted
        // Gating on `sources > 0` collapsed the first two, so `bmj` — 120 pages
        // fetched, 18 classified as guideline, zero requirements extracted —
        // recorded identically to `nature-communications`, which fetched one
        // page and was never really reached.
        if row.crawl_ran {
            store_fingerprint_provenance(
                &db,
                &FingerprintProvenance {
                    // A real crawl, by construction — this harness IS the crawl.
                    origin: "crawled".into(),
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
        row.source_count = fp.provenance.as_ref().map(|p| p.source_count).unwrap_or(-1);
        // Upsert: pass 2 REPLACES the starved row rather than appending a second
        // one, so the table keeps one row per journal and the summary counts
        // what the run actually ended up with.
        match rows.iter().position(|r| r.key == row.key) {
            Some(i) => rows[i] = row,
            None => rows.push(row),
        }
    }
    }

    println!("\n================== STAGE 2: TEN JOURNAL FINGERPRINTS ==================");
    println!(
        "{:<24} {:>5} {:>5} {:>5} {:>6} {:>5} {:>5} {:>5} {:>5} {:>6}  {}",
        "journal", "pages", "guid", "reqs", "confl", "conv", "bind", "expct", "prov", "srcs",
        "coverage"
    );
    for r in &rows {
        // **THREE STATES, not two. §11 D185.** "we fetched nothing", "there is
        // nothing" and "we read it all" are different answers, and a row that
        // shows only a count conflates the first two. A starved journal is the
        // one whose number must never be read as a finding about the journal.
        let coverage = if r.rate_limited {
            "RATE-LIMITED — not covered, count means nothing"
        } else if !r.crawl_ran {
            "crawl failed — not covered"
        } else if r.requirements == 0 {
            "covered; the journal states none this extractor reads"
        } else {
            "covered"
        };
        println!(
            "{:<24} {:>5} {:>5} {:>5} {:>6} {:>5} {:>5} {:>5} {:>5} {:>6}  {}",
            r.key,
            r.pages,
            r.guideline_pages,
            r.requirements,
            r.conflicted,
            r.conventions,
            r.bindings,
            r.expectations,
            if r.provenance { "yes" } else { "NO" },
            if r.provenance { r.source_count.to_string() } else { "-".into() },
            coverage
        );
    }

    let n = rows.len();
    let reqs: usize = rows.iter().map(|r| r.requirements).sum();
    let confl: usize = rows.iter().map(|r| r.conflicted).sum();
    let with_reqs = rows.iter().filter(|r| r.requirements > 0).count();
    let with_conf = rows.iter().filter(|r| r.conflicted > 0).count();
    let with_prov = rows.iter().filter(|r| r.provenance).count();
    let starved: Vec<&str> = rows.iter().filter(|r| r.rate_limited).map(|r| r.key).collect();
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
    // **The headline number is wrong if any journal was starved, and the
    // summary must say so rather than let a reader average it away.**
    if starved.is_empty() {
        println!("\n  rate-limited journals           none — every journal was covered");
    } else {
        println!("\n  RATE-LIMITED JOURNALS           {} of {n}: {}", starved.len(), starved.join(", "));
        println!("  -> their counts are NOT findings about those journals. The limiter is");
        println!("     per-host and shared, so a journal crawled straight after a sibling on");
        println!("     the same host starts with an empty bucket. Re-run those keys after the");
        println!("     others, or the totals above understate the corpus by whatever they hold.");
    }
    println!("=======================================================================");
}
