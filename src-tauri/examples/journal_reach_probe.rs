//! **Does the app's OWN fetcher reach journal guideline pages?**
//!
//! The header work in `http_fetcher.rs` was measured with curl. This measures
//! it through `ReqwestFetcher`, because a finding about curl is not a finding
//! about the app — the whole point of the change is that a crawl should measure
//! journals rather than measuring the client.
//!
//! Live network; not a test. Run:
//! `cargo run -p app --release --example journal_reach_probe`
use app_lib::guidelines::{classify_page, html_title, PageVerdict};
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

const PAGES: &[(&str, &str)] = &[
    ("PLOS", "https://journals.plos.org/plosone/s/submission-guidelines"),
    ("Springer/nature", "https://www.nature.com/nm/content"),
    ("BMJ", "https://www.bmj.com/about-bmj/resources-authors"),
    ("Elsevier (Lancet)", "https://www.thelancet.com/lancet/information-for-authors"),
    ("Wiley", "https://onlinelibrary.wiley.com/page/journal/10970258/homepage/forauthors.html"),
    ("Taylor & Francis", "https://www.tandfonline.com/action/authorSubmission?journalCode=ierj20&page=instructions"),
    ("Frontiers", "https://www.frontiersin.org/journals/public-health/for-authors/author-guidelines"),
    ("SAGE", "https://journals.sagepub.com/author-instructions/JHP"),
    ("BioMed Central", "https://bmcpublichealth.biomedcentral.com/submission-guidelines"),
    // A journal HOMEPAGE — the negative control. Four rows of exactly this
    // shape are in the live corpus marked `ingested`.
    ("(control) BMJ home", "https://www.bmj.com"),
    ("(control) nature.com/nm", "https://www.nature.com/nm"),
];

fn main() {
    let f = ReqwestFetcher::new().expect("client");
    let (mut ok, mut guideline) = (0usize, 0usize);
    println!("{:<22}{:>6}{:>9}  {}", "PUBLISHER", "HTTP", "CHARS", "GATE");
    for (name, url) in PAGES {
        match f.get(&HttpRequest::get(*url)) {
            Ok(r) => {
                if r.status == 200 {
                    ok += 1;
                }
                let text = app_lib::guidelines::html_to_text_public(&r.body);
                let v = classify_page(&html_title(&r.body), &text);
                if matches!(v, PageVerdict::Guideline { .. }) {
                    guideline += 1;
                }
                let label = match &v {
                    PageVerdict::Guideline { obligations, requirements } => {
                        format!("GUIDELINE  (ob={obligations} req={requirements})")
                    }
                    PageVerdict::Navigation { obligations, requirements } => {
                        format!("navigation (ob={obligations} req={requirements})")
                    }
                    PageVerdict::Interstitial { signature } => {
                        format!("INTERSTITIAL [{signature}]")
                    }
                };
                println!("{name:<22}{:>6}{:>9}  {label}", r.status, text.chars().count());
            }
            Err(e) => println!("{name:<22}{:>6}{:>9}  ERROR {e}", "-", "-"),
        }
    }
    println!("\n{ok}/{} returned 200;  {guideline} classified as guideline content.", PAGES.len());
}
