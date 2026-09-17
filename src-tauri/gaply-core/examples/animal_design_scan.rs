//! **Does the corpus contain a design `read_design` has no vocabulary for?**
//!
//! ARRIVE is one of the eight standards `journal_standards::Standard` knows, and
//! it covers in-vivo animal research. Neither `OBSERVATIONAL_MARKERS` nor
//! `EXPERIMENTAL_MARKERS` contains a single animal term, so a manuscript whose
//! design is "we dosed live fish" reads as no design at all. This prints the
//! sentence, so an applicable-standard judgement is made on the manuscript's own
//! words rather than on the absence of a marker.
use gaply_core::extract::docparse;

const LIVE_ANIMAL: &[&str] = &[
    "zebrafish", "danio rerio", "in vivo", "in-vivo", "mice", "mouse", "rats ", "rat ",
    "rabbit", "guinea pig", "animal experiment", "animal stud", "bioassay on fish",
    "acute toxicity test", "lc50", "lethal concentration",
];

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let lower = text.to_lowercase();
        let mut hits: Vec<(&str, String)> = Vec::new();
        for t in LIVE_ANIMAL {
            let Some(at) = lower.find(t) else { continue };
            let s = lower[..at].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
            let e = lower[at..].find(['.', '\n']).map(|i| at + i).unwrap_or(lower.len());
            hits.push((t, lower[s..e].trim().chars().take(240).collect()));
        }
        if hits.is_empty() {
            continue;
        }
        println!("\n######## {name}  ({} terms)", hits.len());
        for (t, s) in hits.iter().take(4) {
            println!("    {t:<20} :: {s}");
        }
    }
}
