//! Which rule excluded each of the 237? Existing data, decomposed — this is
//! input to the PREDICTION, not a measurement of the fixed extractor.
use gaply_core::extract::{self, docparse};

fn is_prose(s: &str) -> bool {
    let t = s.trim();
    t.chars().count() > 90 && t.split_whitespace().count() > 14
}
fn starts_table(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("Table ") || t.starts_with("TABLE ")
}
/// Does the caption end with a bare page number? The contents-page tell.
fn ends_with_page_number(cap: Option<&str>) -> bool {
    let Some(c) = cap else { return false };
    c.split_whitespace()
        .last()
        .map(|w| w.len() <= 4 && w.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut total, mut next_is_table, mut thin_run, mut real) = (0, 0, 0, 0);
    let mut page_num_caption = 0;
    let mut page_num_among_real = 0;
    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        for tb in &ex.tables {
            total += 1;
            if ends_with_page_number(tb.caption.as_deref()) {
                page_num_caption += 1;
            }
            let Some(sec) = ex.sections.iter().find(|s| s.kind == tb.location.section) else {
                continue;
            };
            let after: Vec<&String> =
                sec.paragraphs.iter().skip(tb.location.paragraph + 1).collect();
            if after.first().map(|s| starts_table(s)).unwrap_or(false) {
                next_is_table += 1;
                continue;
            }
            let mut cells = 0usize;
            for para in &after {
                if starts_table(para) || is_prose(para) {
                    break;
                }
                cells += 1;
                if cells > 400 {
                    break;
                }
            }
            if cells < 6 {
                thin_run += 1;
                continue;
            }
            real += 1;
            if ends_with_page_number(tb.caption.as_deref()) {
                page_num_among_real += 1;
            }
        }
    }
    println!("total detections          {total}");
    println!("  excluded: next para is another 'Table N'   {next_is_table}");
    println!("  excluded: cell run < 6                     {thin_run}");
    println!("  kept as real                               {real}");
    println!();
    println!("caption ends in a bare page number  {page_num_caption} of {total}");
    println!("  ...among the REAL ones            {page_num_among_real} of {real}");
}
