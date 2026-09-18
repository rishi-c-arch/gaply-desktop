//! **Is a sub-10 `n` a parse artefact or a semantic error?** and
//! **does this corpus contain a genuine small sample at all?**
//!
//! The first decides whether a guard is arithmetic; the second decides whether
//! rule 5 has any true positive to be right about. Prints RAW text around each
//! sub-10 read — enough to see a severed decimal — and every n in [1,40] with
//! its sentence, since a genuine small study would live there.
use gaply_core::extract::stats::Stat;
use gaply_core::extract::{self, docparse, paragraph_at};

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let mut printed = false;
        for c in &ex.statistics {
            let Stat::SampleSize { n, .. } = &c.stat else { continue };
            if *n >= 41 {
                continue;
            }
            let para = paragraph_at(&ex, &c.location).unwrap_or("").to_string();
            // RAW window around the literal "n = N" / "n=N", so a severed
            // decimal is visible as the character that follows.
            let hay = para.to_lowercase();
            let mut window = String::from("<not located in paragraph>");
            for pat in [format!("n = {n}"), format!("n={n}"), format!("n ={n}"), format!("n= {n}")] {
                if let Some(i) = hay.find(&pat) {
                    let a = i.saturating_sub(70);
                    let b = (i + pat.len() + 45).min(para.len());
                    window = para[a..b].replace('\n', " ");
                    break;
                }
            }
            if !printed {
                println!("\n######## {name}");
                printed = true;
            }
            println!("  n = {n:<4} :: …{window}…");
        }
    }
}
