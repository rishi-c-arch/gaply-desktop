//! Build the convention profile for a journal from real OpenAlex records, and
//! print every metric — including the ones the source cannot supply, because a
//! metric omitted is indistinguishable from one nobody asked for.
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_openalex::{recent_papers, source_id_for_issn};
use gaply_core::journal_corpus::{derive_conventions, ConventionStatus, CorpusBounds};

fn main() {
    let issn = std::env::args().nth(1).expect("usage: journal_conventions_probe <issn> [from-date]");
    let from = std::env::args().nth(2).unwrap_or_else(|| "2025-03-01".into());
    let cfg: serde_json::Value =
        serde_json::from_str(include_str!("../config/journal-crawl.json")).unwrap();
    let bounds =
        CorpusBounds::from_json(&cfg["corpus_bounds"].to_string()).expect("corpus_bounds in config");
    println!("bounds: min={} target={} max={}", bounds.minimum, bounds.target, bounds.maximum);

    let f = ReqwestFetcher::new().unwrap();
    let src = source_id_for_issn(&f, &issn).expect("source");
    let papers = recent_papers(&f, &src, &from, bounds.maximum).expect("works");
    println!("ISSN {issn} -> {src}; {} recently published papers since {from}", papers.len());
    println!(
        "  with an abstract: {}   with a usable page range: {}",
        papers.iter().filter(|p| !p.abstract_text.is_empty()).count(),
        papers.iter().filter(|p| p.page_count().is_some()).count()
    );

    println!("\n--- conventions ---");
    for c in derive_conventions(&papers, &bounds) {
        let head = match c.status {
            ConventionStatus::Inferred => format!(
                "INFERRED   n={:<4} median={:<8}",
                c.n,
                c.median.map(|m| format!("{m}")).unwrap_or_else(|| "-".into())
            ),
            ConventionStatus::Unavailable => "UNAVAILABLE".to_string(),
        };
        println!("  {:<18} {head}", c.metric.as_str());
        if let (Some(lo), Some(hi)) = (c.iqr_low, c.iqr_high) {
            println!("                     IQR {lo}–{hi}");
        }
        println!("                     {}", c.detail);
    }
}
