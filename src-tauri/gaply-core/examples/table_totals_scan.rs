//! **RT4 step 1 — is there an INPUT for a table-total check?**
//!
//! §12 records RT4 as an unchecked gap and names its requirement: a corpus of
//! real tables with known totals. Before any check is written, this asks the
//! prior question — whether the extractor can supply the numbers at all.
//!
//! `TableRef { label, caption, location }` carries no cells, so the rows would
//! have to be recovered from the paragraph text at `location`. This walks the
//! paragraphs FOLLOWING each detected caption and reports, per manuscript:
//! tables detected, how many are followed by numeric-looking rows, and how many
//! have a TOTALS row. Rows are printed with their text, not counted.
use gaply_core::extract::{self, docparse};

/// A row is "numeric" if it carries at least two numbers — one number is a
/// sentence mentioning a figure, not a table row.
fn numbers_in(line: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in line.chars() {
        if c.is_ascii_digit() || c == '.' || (c == '-' && cur.is_empty()) {
            cur.push(c);
        } else {
            if let Ok(v) = cur.trim_end_matches('.').parse::<f64>() {
                out.push(v);
            }
            cur.clear();
        }
    }
    if let Ok(v) = cur.trim_end_matches('.').parse::<f64>() {
        out.push(v);
    }
    out
}

/// Does this line announce a total? The vocabulary a table actually uses.
fn is_totals_row(line: &str) -> bool {
    let l = line.trim().to_lowercase();
    ["total", "sum", "overall", "all ", "grand total", "n ="]
        .iter()
        .any(|k| l.starts_with(k))
}

/// How far past a caption to look for the table body.
const LOOKAHEAD: usize = 25;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut docs, mut parsed) = (0usize, 0usize);
    let (mut tables, mut with_rows, mut with_totals) = (0usize, 0usize, 0usize);
    let mut docs_with_tables = 0usize;

    println!("{:<52} {:>7} {:>7} {:>7}", "manuscript", "tables", "+rows", "+total");
    for p in &paths {
        docs += 1;
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else {
            println!("{:<52} {:>7}", name, "PARSE-FAIL");
            continue;
        };
        parsed += 1;
        let ex = extract::extract_from_text(&text);
        let (mut t, mut r, mut tot) = (0usize, 0usize, 0usize);

        for tb in &ex.table_mentions {
            t += 1;
            // §11 D169: resolve by the PRODUCER'S index. `find(kind)` returned
            // the first section of the kind, which is what made D168's 414 /
            // 237 / 177 split a measurement through a broken instrument.
            let sec = tb.location.section_index.and_then(|i| ex.sections.get(i));
            let Some(sec) = sec else { continue };
            let start = tb.location.paragraph + 1;
            let body: Vec<&String> =
                sec.paragraphs.iter().skip(start).take(LOOKAHEAD).collect();

            let numeric: Vec<&&String> =
                body.iter().filter(|l| numbers_in(l.as_str()).len() >= 2).collect();
            if !numeric.is_empty() {
                r += 1;
            }
            let totals: Vec<&&String> = numeric
                .iter()
                .filter(|l| is_totals_row(l.as_str()))
                .copied()
                .collect();
            if !totals.is_empty() {
                tot += 1;
                // PRINT THE ROW, not the count — §11 D163's rule.
                println!("  [{}] {} :: {}", name, tb.label, totals[0].trim());
            }
        }
        tables += t;
        with_rows += r;
        with_totals += tot;
        if t > 0 {
            docs_with_tables += 1;
        }
        println!("{:<52} {:>7} {:>7} {:>7}", name, t, r, tot);
    }

    println!("\n==============================================================");
    println!("  manuscripts                 {docs}  (parsed {parsed})");
    println!("  manuscripts with >=1 table  {docs_with_tables}");
    println!("  tables detected             {tables}");
    println!("  followed by numeric rows    {with_rows}");
    println!("  with a TOTALS row           {with_totals}");
    println!("==============================================================");
    if with_totals == 0 {
        println!("\n  NO INPUT. A table-total check has nothing to read.");
    }
}
