//! **How many Scopus journals can Gaply actually read guidance from? (§11 D192 follow-up)**
//!
//! The bundled directory carries WEBSITES — 258 of 258, and 0 author-guideline
//! URLs — so a sample cannot be fetched the way `pasted_url_probe` fetches a
//! pasted URL. Each journal needs its guidance page DISCOVERED from its homepage
//! first, and a discovery failure must stay distinguishable from a page that
//! loads empty. Those are different findings and only one of them is what a
//! search-capable provider would fix.
//!
//! Four buckets, plus the split the run itself forced:
//!
//! | bucket | meaning |
//! |---|---|
//! | `extracted` | requirements stored, with the count |
//! | `reached_no_guidance` | fetched 200, classified as navigation or yielding nothing |
//! | `fetch_failed` | a guidance URL was found and could not be read |
//! | `quarantined_by_gaply` | fetched and refused by the injection scanner |
//! | `link_offhost_not_followed` | a guidance link exists, one host away |
//! | `no_link_found` | homepage read, named no guidance |
//! | `homepage_unreadable` | the homepage itself could not be read |
//!
//! **The last one is not in the original four and is not a rounding detail.** A
//! homepage that refuses us produces no link, so folding it into `no_link_found`
//! would report a refused fetch as a journal that publishes no guidance — the
//! same conflation the four buckets exist to prevent, one level up.
//!
//! Not curl. `ReqwestFetcher` throughout: a curl result is a fact about curl's
//! TLS fingerprint (CLAUDE.md).

use app_lib::guidelines::{self, JournalIdentity};
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{discover_guidelines_links, LinkConfidence};
use gaply_core::embed::HashEmbedder;
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::Database;
use url::Url;

/// **The fetcher exposes status and body, never the final URL**, and reqwest
/// follows redirects silently. `www.journals.elsevier.com` was retired and
/// redirects to sciencedirect, so resolving relative links against the REQUESTED
/// host would point every candidate at a host the body never mentions, and the
/// same-host rule would then drop all of them — reporting a live journal as
/// "names no guidance". The page's own canonical link is the honest base.
/// **The fetcher exposes status and body, never the final URL**, and reqwest
/// follows redirects silently, so relative links must resolve against the host
/// the BODY belongs to rather than the one that was requested.
///
/// **Parsed from the canonical tag itself, not from a window around it.** The
/// first version searched +/-300 characters for any `href="`, and on Elsevier
/// that returned `rss.sciencedirect.com` — a base URL that is an RSS host, which
/// is absurd on its face and dropped every candidate through the same-host rule.
/// A wrong base is indistinguishable in the counts from a journal that publishes
/// nothing, which is why it is worth parsing properly.
fn effective_base(requested: &Url, body: &str) -> Url {
    let lower = body.to_lowercase();
    for (open, marker, attr) in [
        ("<link", "canonical", "href"),
        ("<meta", "og:url", "content"),
    ] {
        let mut i = 0usize;
        while let Some(rel) = lower[i..].find(open) {
            let start = i + rel;
            let Some(close) = lower[start..].find('>').map(|k| start + k) else { break };
            let tag_lower = &lower[start..close];
            i = close + 1;
            if !tag_lower.contains(marker) {
                continue;
            }
            // Value read from the ORIGINAL body: lowercasing a URL path can
            // change what it addresses.
            let tag = &body[start..close];
            let Some(a) = tag_lower.find(attr) else { continue };
            let after = &tag[a..];
            let Some(q1) = after.find(['"', '\'']) else { continue };
            let quote = after.as_bytes()[q1] as char;
            let rest = &after[q1 + 1..];
            let Some(q2) = rest.find(quote) else { continue };
            if let Ok(u) = requested.join(rest[..q2].trim()) {
                if u.host_str().is_some() {
                    return u;
                }
            }
        }
    }
    requested.clone()
}

fn main() {
    let list = std::env::args().nth(1).expect("usage: scopus_discovery_probe <sample.tsv>");
    let raw = std::fs::read_to_string(&list).expect("sample file");
    let fetcher = ReqwestFetcher::new().expect("fetcher");
    let limiter = RateLimiter::new(4.0, 0.5);

    println!("idx\tjournal\tbucket\tdetail");
    let (mut extracted, mut reached, mut failed, mut nolink, mut nohome) = (0, 0, 0, 0, 0);
    let mut offhost = 0;
    let mut quarantined = 0;
    let mut rows: Vec<String> = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f = line.split('\t');
        let (idx, name, site) = (
            f.next().unwrap_or("?").to_string(),
            f.next().unwrap_or("?").to_string(),
            f.next().unwrap_or("").to_string(),
        );
        let Ok(home_url) = Url::parse(&site) else {
            nohome += 1;
            println!("{idx}\t{name}\thomepage_unreadable\tunparseable website {site:?}");
            continue;
        };

        // 1. the homepage
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let home = match fetcher.get(&HttpRequest::get(home_url.as_str())) {
            Ok(r) if r.status == 200 => r,
            Ok(r) => {
                nohome += 1;
                println!("{idx}\t{name}\thomepage_unreadable\thttp {}", r.status);
                continue;
            }
            Err(e) => {
                nohome += 1;
                println!("{idx}\t{name}\thomepage_unreadable\tfetch failed: {e}");
                continue;
            }
        };

        // 2. discovery, against the base the PAGE declares
        let base = effective_base(&home_url, &home.body);
        let cands = discover_guidelines_links(&home.body, &base);
        // **The conservative pick is same-host; an off-host candidate is a
        // different finding and gets its own row.** Folding it into
        // `no_link_found` would report a journal whose guidance link is one host
        // away as a journal that names no guidance.
        let same_host = cands.iter().find(|c| !c.off_host);
        let best = match same_host {
            Some(b) => b,
            None => {
                if let Some(off) = cands.first() {
                    offhost += 1;
                    println!(
                        "{idx}\t{name}\tlink_offhost_not_followed\t{} :: {:?}",
                        off.url, off.anchor
                    );
                } else {
                    nolink += 1;
                    println!(
                        "{idx}\t{name}\tno_link_found\thomepage {} bytes, base {}",
                        home.body.len(),
                        base.host_str().unwrap_or("?")
                    );
                }
                continue;
            }
        };
        let tier = match best.confidence {
            LinkConfidence::Explicit => "explicit",
            LinkConfidence::Lexicon => "lexicon",
        };

        // 3. the real ingest path, exactly as a pasted URL takes it
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let db = Database::in_memory().expect("db");
        let rep = guidelines::ingest_with(
            &fetcher,
            &limiter,
            &db,
            &HashEmbedder,
            None,
            Some(&best.url),
            JournalIdentity { key: None, name: Some(&name) },
        );
        let verdict = format!("{:?}", rep.results.first());
        // **A quarantine is Gaply refusing the page, not the page being empty.**
        // Measured on Elsevier: "Requests which do not comply with the
        // instructions outlined in the form will not be considered" trips
        // `sanitize::refuses_instructions`, which wants "do not comply with"
        // within 40 chars of "instruction". That is a journal enforcing its own
        // rules — the opposite of an injection — and it is the same shape as the
        // PLOS false positive already recorded in that module. Counting it as
        // `reached_no_guidance` would attribute Gaply's refusal to the journal.
        if verdict.contains("Quarantined") {
            quarantined += 1;
            println!("{idx}\t{name}\tquarantined_by_gaply\t{tier} {} :: {verdict}", best.url);
            continue;
        }
        let reached_flag = verdict.contains("reached: true") || verdict.contains("Ingested");
        if rep.requirements_stored > 0 {
            extracted += 1;
            println!(
                "{idx}\t{name}\textracted\t{} req via {tier} {} ",
                rep.requirements_stored, best.url
            );
            let fp = gaply_core::journal_fingerprint::fingerprint_for(
                &db,
                rep.journal_key.as_deref().unwrap_or(""),
            );
            if let Ok(fp) = fp {
                for r in fp.requirements.iter().take(3) {
                    rows.push(format!(
                        "    [{}] {} = {}\n      span: {}",
                        name,
                        r.kind,
                        r.value,
                        r.source_span.chars().take(160).collect::<String>()
                    ));
                }
            }
        } else if reached_flag {
            reached += 1;
            println!("{idx}\t{name}\treached_no_guidance\t{tier} {} :: {verdict}", best.url);
        } else {
            failed += 1;
            println!("{idx}\t{name}\tfetch_failed\t{tier} {} :: {verdict}", best.url);
        }
    }

    println!("\n=== counts ===");
    println!("  extracted            {extracted}");
    println!("  reached_no_guidance  {reached}");
    println!("  fetch_failed         {failed}");
    println!("  quarantined_by_gaply  {quarantined}");
    println!("  link_offhost_not_followed {offhost}");
    println!("  no_link_found        {nolink}");
    println!("  homepage_unreadable  {nohome}");
    println!("\n=== rows, not totals ===");
    for r in rows {
        println!("{r}");
    }
}
