//! THROWAWAY — scan EVERY ingested guideline chunk for the checklist detectors.
use gaply_core::embed::HashEmbedder;
use gaply_core::{Database, GaplyError};

const GUIDELINE_QUERY: &str =
    "author guidelines submission requirements word limit structured abstract \
     conflict of interest declaration reference style";

fn main() -> Result<(), GaplyError> {
    let url = std::env::args().nth(1).expect("usage: chunk_scan <url>");
    let db = Database::in_memory()?;
    let emb = HashEmbedder;
    let ing = app_lib::guidelines::GuidelinesIngestor::new()?;
    let rep = ing.ingest(&db, &emb, None, Some(&url), Default::default());
    println!("ingested: {} ({})", rep.any_ingested, rep.note);

    // Pull EVERY chunk: a huge k with no filter, then keep journal_guideline.
    let all = gaply_core::rag::search(&db, &emb, "a", 500, Some("journal_guideline"))?;
    println!("total chunks scanned: {}\n", all.len());

    // Which chunks the CURRENT top-5 checklist query retrieves.
    let top5 = gaply_core::rag::search(&db, &emb, GUIDELINE_QUERY, 5, Some("journal_guideline"))?;
    let top5_ids: Vec<i64> = top5.iter().map(|h| h.chunk_id).collect();
    println!("top-5 chunk_ids: {top5_ids:?}\n");

    let exact: &[(&str, &[&str])] = &[
        ("word limit (extract_word_limit)", &["word limit", "words maximum", "maximum of", "word count"]),
        ("\"structured abstract\"", &["structured abstract"]),
        ("\"conflict\"", &["conflict"]),
        ("\"vancouver\" / \"numbered\"", &["vancouver", "numbered"]),
    ];
    let alt: &[(&str, &[&str])] = &[
        ("word limit", &["maximum length", "no more than", "length limit", "characters", "word", "length"]),
        ("structured abstract", &["abstract"]),
        ("conflict", &["competing interest", "competing interests", "disclosure", "declaration"]),
        ("reference style", &["reference style", "citation style", "reference list", "citing", "references are"]),
    ];

    for (i, (label, needles)) in exact.iter().enumerate() {
        let mut hit_ids = Vec::new();
        for h in &all {
            let c = h.content.to_lowercase();
            if needles.iter().any(|n| c.contains(n)) { hit_ids.push(h.chunk_id); }
        }
        let in_top5: Vec<i64> = hit_ids.iter().copied().filter(|id| top5_ids.contains(id)).collect();
        // alternate wording
        let (alabel, aneedles) = alt[i];
        let mut alt_ids = Vec::new();
        for h in &all {
            let c = h.content.to_lowercase();
            if aneedles.iter().any(|n| c.contains(n)) { alt_ids.push(h.chunk_id); }
        }
        println!("DETECTOR: {label}");
        println!("  exact present in chunks: {:?}", hit_ids);
        println!("  retrieved by top-5     : {:?}", in_top5);
        println!("  alt wording ({alabel}) in chunks: {:?}", alt_ids.iter().take(12).collect::<Vec<_>>());
        // show a sample of alternate-wording text
        if hit_ids.is_empty() {
            if let Some(h) = all.iter().find(|h| alt_ids.contains(&h.chunk_id)) {
                let c = h.content.to_lowercase();
                for n in aneedles {
                    if let Some(p) = c.find(n) {
                        let s = p.saturating_sub(70);
                        let e = (p + 130).min(c.len());
                        println!("  sample [{}]: …{}…", n, &h.content[s..e].replace('\n', " "));
                        break;
                    }
                }
            }
        }
        println!();
    }
    Ok(())
}
