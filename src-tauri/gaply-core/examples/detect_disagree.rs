//! Where does the extractor's body_follows disagree with the probe's rule?
//! The prediction was 177 and the measurement 236; this prints the gap's rows.
use gaply_core::extract::{self, docparse};

fn is_prose(s: &str) -> bool {
    let t = s.trim();
    t.chars().count() > 90 && t.split_whitespace().count() > 14
}
fn starts_table(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("Table ") || t.starts_with("TABLE ")
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let show: usize = args.remove(0).parse().unwrap_or(4);
    let mut shown = 0usize;
    let (mut admitted, mut probe_rejects) = (0usize, 0usize);
    for p in &args {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        for tb in &ex.table_mentions {
            admitted += 1;
            // §11 D169: resolve by the PRODUCER'S index. `find(kind)` returned
            // the first section of the kind, which is what made D168's 414 /
            // 237 / 177 split a measurement through a broken instrument.
            let Some(sec) = tb.location.section_index.and_then(|i| ex.sections.get(i)) else {
                continue;
            };
            let after: Vec<&String> =
                sec.paragraphs.iter().skip(tb.location.paragraph + 1).collect();
            // THE PROBE'S rule, applied to what the extractor admitted.
            let toc = after.first().map(|s| starts_table(s)).unwrap_or(false);
            let mut cells = 0usize;
            for para in &after {
                if starts_table(para) || is_prose(para) {
                    break;
                }
                cells += 1;
                if cells > 400 { break; }
            }
            if toc || cells < 6 {
                probe_rejects += 1;
                if shown < show {
                    shown += 1;
                    println!("\n--- [{name}] {} — probe rejects (toc={toc}, cells={cells})", tb.label);
                    println!("    caption: {:?}", tb.caption.as_deref().unwrap_or("<none>"));
                    for (i, q) in after.iter().take(8).enumerate() {
                        let t: String = q.chars().take(110).collect();
                        println!("    [{i}] {t}");
                    }
                }
            }
        }
    }
    println!("\nextractor admitted        {admitted}");
    println!("probe's rule rejects      {probe_rejects}");
    println!("agree                     {}", admitted - probe_rejects);
}
