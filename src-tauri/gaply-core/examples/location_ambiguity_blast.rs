//! **How much of the pipeline resolves a Location to the WRONG section?**
//!
//! `paragraph_at` takes the FIRST section of a kind. The producer knew which
//! section it was in — `extract_from_text_with` builds the `Location` inside a
//! loop over sections — and the address cannot carry it. So every consumer
//! guesses, including `validate`'s five Tier-0 rules, which override every
//! model in the system.
use gaply_core::extract::{self, docparse, Location};
use gaply_core::extract::stats::Stat;
use std::collections::BTreeMap;

/// The verbatim text a claim was matched from — every variant carries `raw`.
fn raw_of(s: &Stat) -> String {
    match s {
        Stat::PValue { raw, .. }
        | Stat::ConfidenceInterval { raw, .. }
        | Stat::SampleSize { raw, .. }
        | Stat::Test { raw, .. }
        | Stat::TestStatistic { raw, .. }
        | Stat::EffectSize { raw, .. } => raw.clone(),
        other => format!("{other:?}"),
    }
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut claims, mut claims_amb, mut claims_wrong) = (0usize, 0usize, 0usize);
    let (mut tables, mut tables_amb, mut tables_wrong) = (0usize, 0usize, 0usize);
    let (mut cites, mut cites_amb, mut cites_wrong) = (0usize, 0usize, 0usize);

    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let mut count: BTreeMap<String, usize> = BTreeMap::new();
        for s in &ex.sections {
            *count.entry(format!("{:?}", s.kind)).or_default() += 1;
        }
        let ambiguous =
            |loc: &Location| count.get(&format!("{:?}", loc.section)).copied().unwrap_or(0) > 1;
        let elsewhere = |loc: &Location, want: &str| -> bool {
            ex.sections
                .iter()
                .filter(|s| format!("{:?}", s.kind) == format!("{:?}", loc.section))
                .any(|s| s.paragraphs.get(loc.paragraph).map(|q| q.contains(want)).unwrap_or(false))
        };

        for c in &ex.statistics {
            claims += 1;
            if !ambiguous(&c.location) { continue; }
            claims_amb += 1;
            let got = extract::paragraph_at(&ex, &c.location).unwrap_or("");
            let want = raw_of(&c.stat);
            if !got.contains(want.as_str()) && elsewhere(&c.location, &want) { claims_wrong += 1; }
        }
        for t in &ex.tables {
            tables += 1;
            if !ambiguous(&t.location) { continue; }
            tables_amb += 1;
            let got = extract::paragraph_at(&ex, &t.location).unwrap_or("");
            if !got.contains(&t.label) && elsewhere(&t.location, &t.label) { tables_wrong += 1; }
        }
        for c in &ex.citations {
            cites += 1;
            if !ambiguous(&c.location) { continue; }
            cites_amb += 1;
            let got = extract::paragraph_at(&ex, &c.location).unwrap_or("");
            // `Citation::raw` is NOT always a verbatim slice. In a grouped
            // parenthetical — "(Ramachandra et al. 2015; Ramachandra et al.
            // 2016)" — the parser reconstructs the second citation's `raw` with
            // parentheses the text never had, so a containment test on `raw`
            // reports a resolution failure that did not happen. That cost one
            // false "STILL WRONG" row here. Compare on the author token, which
            // IS verbatim.
            let want: String = c
                .raw
                .trim()
                .trim_matches(|ch: char| !ch.is_alphanumeric())
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if want.is_empty() { continue; }
            if !got.contains(want.as_str()) && elsewhere(&c.location, &want) {
                cites_wrong += 1;
                println!("  STILL WRONG [{}] {:?} idx={:?} para={}",
                         std::path::Path::new(p).file_name().unwrap().to_string_lossy(),
                         c.raw, c.location.section_index, c.location.paragraph);
            }
        }
    }

    println!("                       total   ambiguous   RESOLVES WRONG");
    println!("  statistical claims  {claims:>6}  {claims_amb:>10}  {claims_wrong:>15}");
    println!("  tables              {tables:>6}  {tables_amb:>10}  {tables_wrong:>15}");
    println!("  citations           {cites:>6}  {cites_amb:>10}  {cites_wrong:>15}");
    println!("\n  'ambiguous'      = the Location's kind occurs more than once");
    println!("  'resolves wrong' = paragraph_at returns a paragraph NOT containing the");
    println!("                     item, while some other section of that kind does.");
}
