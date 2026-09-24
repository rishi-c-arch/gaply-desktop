//! **Which bundled journal rows come from a page the journal itself owns?**
//!
//! Runs the product's own `journal_crawl::key_for_url` — not a copy of the rule
//! — over every row of `gaply-core/data/journal-seed.json` that carries a
//! `source_url`, and prints, per journal:
//!
//! * `own`     — the URL resolves to this journal through a configured path
//!               scope (`journal_path_segments`);
//! * `host`    — the URL resolves to this journal ONLY because its host has no
//!               path scope, so host equality decided it. On a host that serves
//!               many journals this is not evidence of ownership, and the path
//!               is printed so a reader can judge it;
//! * `foreign` — the URL resolves to another journal or to none: a publisher-
//!               wide page, or another journal's.
//!
//! Every `foreign` and `host` row is printed with its heading and its WHOLE
//! span — a truncated span cannot be checked (CLAUDE.md).
//!
//! ```text
//! cargo run --example journal_ownership_probe            # requirements only
//! cargo run --example journal_ownership_probe -- --all   # + bindings, expectations
//! ```

use std::collections::BTreeMap;

use app_lib::journal_crawl::{key_for_url, CrawlBudget};

const SEED: &str = include_str!("../gaply-core/data/journal-seed.json");
const CONFIG: &str = include_str!("../config/journal-crawl.json");

#[derive(Default)]
struct Tally {
    own: usize,
    host: usize,
    foreign: usize,
}

fn classify(url: &str, key: &str, b: &CrawlBudget) -> &'static str {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default();
    match key_for_url(url, b) {
        Some(k) if k == key && b.journal_path_segments.contains_key(&host) => "own",
        Some(k) if k == key => "host",
        _ => "foreign",
    }
}

fn table(name: &str, rows: &[serde_json::Value], b: &CrawlBudget, journals: &[String]) {
    let mut tally: BTreeMap<String, Tally> = journals.iter().map(|j| (j.clone(), Tally::default())).collect();
    let mut printed = Vec::new();
    for r in rows {
        let key = r["journal_key"].as_str().unwrap_or_default();
        let url = r["source_url"].as_str().unwrap_or_default();
        let class = classify(url, key, b);
        let t = tally.entry(key.to_string()).or_default();
        match class {
            "own" => t.own += 1,
            "host" => t.host += 1,
            _ => t.foreign += 1,
        }
        if class != "own" {
            printed.push(format!(
                "  [{class}] {key} | {} = {} | resolves_to={:?}\n    url: {url}\n    heading: {}\n    span: {}",
                r["kind"].as_str().or(r["standard"].as_str()).unwrap_or("-"),
                r["value"].as_str().or(r["design"].as_str()).unwrap_or("-"),
                key_for_url(url, b),
                r["source_heading"].as_str().unwrap_or("-"),
                r["source_span"].as_str().unwrap_or("-"),
            ));
        }
    }
    let total: usize = tally.values().map(|t| t.own + t.host + t.foreign).sum();
    println!("== {name}: {} rows (read {total})", rows.len());
    println!("  {:<26} {:>5} {:>5} {:>8} {:>6}", "journal", "own", "host", "foreign", "total");
    for (j, t) in &tally {
        println!("  {:<26} {:>5} {:>5} {:>8} {:>6}", j, t.own, t.host, t.foreign, t.own + t.host + t.foreign);
    }
    let (o, h, f) = tally.values().fold((0, 0, 0), |a, t| (a.0 + t.own, a.1 + t.host, a.2 + t.foreign));
    println!("  {:<26} {:>5} {:>5} {:>8} {:>6}", "ALL", o, h, f, o + h + f);
    for p in printed {
        println!("{p}");
    }
}

fn main() {
    let all = std::env::args().any(|a| a == "--all");
    let b = CrawlBudget::from_json(CONFIG).expect("config/journal-crawl.json");
    let seed: serde_json::Value = serde_json::from_str(SEED).expect("seed JSON");
    let journals: Vec<String> = b.profiled_journals.iter().map(|j| j.key.clone()).collect();
    assert_eq!(journals.len(), 10, "expected the ten profiled journals");

    // Known-good case first: every journal's own crawl ENTRY must resolve to
    // that journal. If it does not, the instrument is wrong, not the seed.
    for j in &b.profiled_journals {
        let got = key_for_url(&j.entry, &b);
        println!("entry {:<26} -> {:?}", j.key, got);
        assert_eq!(got.as_deref(), Some(j.key.as_str()), "entry of {} does not resolve to it", j.key);
    }

    let rows = |k: &str| seed[k].as_array().cloned().unwrap_or_default();
    table("requirements", &rows("requirements"), &b, &journals);
    if all {
        table("bindings", &rows("bindings"), &b, &journals);
        table("expectations", &rows("expectations"), &b, &journals);
    }
}
