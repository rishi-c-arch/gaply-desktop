//! RT4 step 1b — PRINT THE ROWS. `table_totals_scan` reported 414 tables and
//! one totals row, and the one was prose starting "Total chlorophyll…". Both
//! numbers need their rows read before either is believed.
use gaply_core::extract::{self, docparse};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let show: usize = args.remove(0).parse().unwrap_or(3);
    for p in &args {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        if ex.tables.is_empty() {
            continue;
        }
        println!("\n######## {name}  ({} detected) ########", ex.tables.len());
        for tb in ex.tables.iter().take(show) {
            // §11 D169: resolve by the PRODUCER'S index. `find(kind)` returned
            // the first section of the kind, which is what made D168's 414 /
            // 237 / 177 split a measurement through a broken instrument.
            let Some(sec) = tb.location.section_index.and_then(|i| ex.sections.get(i)) else {
                continue;
            };
            println!("\n  LABEL   {}", tb.label);
            println!("  CAPTION {:?}", tb.caption.as_deref().unwrap_or("<none>"));
            println!("  -- the 30 paragraphs that follow, verbatim --");
            for (i, para) in
                sec.paragraphs.iter().skip(tb.location.paragraph + 1).take(30).enumerate()
            {
                let t: String = para.chars().take(160).collect();
                println!("   [{i}] {t}");
            }
        }
    }
}
