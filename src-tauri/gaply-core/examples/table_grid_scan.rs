//! **RT4 steps 1–3 — measure the input before writing any check.**
//!
//! Three questions in order, because each makes the next meaningful:
//!
//! 1. Of the tables the extractor detects, how many are REAL tables rather than
//!    list-of-tables front matter? `detect_table` matches any paragraph opening
//!    "Table N", and a thesis contents page is a run of those.
//! 2. Of the real ones, how many have a recoverable GRID? docparse flattens a
//!    .docx table to ONE CELL PER PARAGRAPH, so the column count has to be
//!    inferred and the cell count must divide by it.
//! 3. Of those, how many carry a sum a check could test — an explicit totals
//!    row, or a percentage column that should reach 100?
//!
//! Every stage prints rows, not just counts.
use gaply_core::extract::{self, docparse};

/// A cell that is only a number (with optional %, commas, sign).
fn as_number(s: &str) -> Option<f64> {
    let t = s.trim().trim_end_matches('%').replace(',', "");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok()
}

/// Prose, not a cell: long, and full of spaces.
fn is_prose(s: &str) -> bool {
    let t = s.trim();
    t.chars().count() > 90 && t.split_whitespace().count() > 14
}

fn starts_table(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("Table ") || t.starts_with("TABLE ")
}

/// First cell announces a total.
fn is_total_label(s: &str) -> bool {
    let l = s.trim().to_lowercase();
    let l = l.trim_end_matches(':').trim();
    l == "total" || l == "totals" || l == "sum" || l == "overall" || l == "grand total" || l == "all"
}

struct Grid {
    cols: usize,
    rows: Vec<Vec<String>>,
}

/// Recover a grid from the flattened cell stream, or explain why not.
fn recover(cells: &[String]) -> Result<Grid, String> {
    if cells.len() < 6 {
        return Err(format!("only {} cell(s)", cells.len()));
    }
    // COLUMN INFERENCE. The naive "leading non-numeric cells" rule is WRONG and
    // was measured to be: a table's first DATA row opens with a row label, which
    // is also non-numeric, so the header run runs straight into it. On the
    // Revised Health Economics Table 1 that read 7 columns for a 6-column table
    // and the grid was rejected as indivisible.
    //
    // The shape that holds: header of H cells, then each row is one label plus
    // H-1 values. So the FIRST NUMBER sits at index H+1 (0-based H+1 => the
    // second cell of row 1), giving H = first_number_index - 1.
    let first_num = cells.iter().position(|c| as_number(c).is_some());
    let Some(first_num) = first_num else {
        return Err("no numeric cell at all".into());
    };
    if first_num < 2 {
        return Err("no header run (first cells are numeric)".into());
    }
    let header_len = first_num - 1;
    if header_len < 2 {
        return Err("inferred column count < 2".into());
    }
    if header_len > 12 {
        return Err(format!("header run {header_len} too long — not a grid"));
    }
    let body = &cells[header_len..];
    if body.is_empty() {
        return Err("header only, no body".into());
    }
    if body.len() % header_len != 0 {
        return Err(format!("{} body cell(s) not divisible by {header_len} column(s)", body.len()));
    }
    let mut rows = vec![cells[..header_len].to_vec()];
    for chunk in body.chunks(header_len) {
        rows.push(chunk.to_vec());
    }
    Ok(Grid { cols: header_len, rows })
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut detected, mut toc, mut real, mut gridded) = (0usize, 0usize, 0usize, 0usize);
    let (mut with_total_row, mut with_pct_col) = (0usize, 0usize);
    let mut fail_reasons: std::collections::BTreeMap<String, usize> = Default::default();

    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        for tb in &ex.table_mentions {
            detected += 1;
            // §11 D169: resolve by the PRODUCER'S index. `find(kind)` returned
            // the first section of the kind, which is what made D168's 414 /
            // 237 / 177 split a measurement through a broken instrument.
            let Some(sec) = tb.location.section_index.and_then(|i| ex.sections.get(i)) else {
                continue;
            };
            let after: Vec<&String> =
                sec.paragraphs.iter().skip(tb.location.paragraph + 1).collect();

            // (1) list-of-tables front matter: the very next paragraph is another
            // "Table N", so this caption has no body of its own.
            if after.first().map(|s| starts_table(s)).unwrap_or(false) {
                toc += 1;
                continue;
            }
            // Collect the cell run: stop at prose or the next table.
            let mut cells: Vec<String> = Vec::new();
            for para in &after {
                if starts_table(para) || is_prose(para) {
                    break;
                }
                cells.push((*para).clone());
                if cells.len() > 400 {
                    break;
                }
            }
            if cells.len() < 6 {
                toc += 1;
                continue;
            }
            real += 1;

            // (2) grid recovery
            match recover(&cells) {
                Err(why) => {
                    *fail_reasons.entry(why).or_default() += 1;
                }
                Ok(g) => {
                    gridded += 1;
                    // (3) is there a sum to check?
                    let total_row = g.rows.iter().skip(1).find(|r| is_total_label(&r[0]));
                    let mut pct_col = None;
                    for c in 1..g.cols {
                        let vals: Vec<f64> = g
                            .rows
                            .iter()
                            .skip(1)
                            .filter(|r| !is_total_label(&r[0]))
                            .filter_map(|r| as_number(&r[c]))
                            .collect();
                        if vals.len() >= 3 {
                            let s: f64 = vals.iter().sum();
                            if (s - 100.0).abs() < 2.0 {
                                pct_col = Some((c, s, vals.len()));
                            }
                        }
                    }
                    if let Some(tr) = total_row {
                        with_total_row += 1;
                        // THE WHOLE GRID, and what a naive column sum would say
                        // about every numeric column. This is the false-positive
                        // evidence §12 asked for, and it only exists as rows.
                        println!("\n  ===== [{name}] {} — {} cols =====", tb.label, g.cols);
                        for r in &g.rows {
                            println!("    {r:?}");
                        }
                        for c in 1..g.cols {
                            let body: Vec<f64> = g
                                .rows
                                .iter()
                                .skip(1)
                                .filter(|r| !is_total_label(&r[0]))
                                .filter_map(|r| as_number(&r[c]))
                                .collect();
                            let stated = as_number(&tr[c]);
                            if body.len() < 2 {
                                continue;
                            }
                            let sum: f64 = body.iter().sum();
                            let head = g.rows[0].get(c).map(|s| s.as_str()).unwrap_or("?");
                            match stated {
                                None => println!(
                                    "    col {c} '{head}': body sums to {sum:.2}; stated total \
                                     {:?} IS NOT A NUMBER — a naive check cannot even compare",
                                    tr[c]
                                ),
                                Some(st) => {
                                    let d = (sum - st).abs();
                                    let verdict = if d < 0.0001 {
                                        "EXACT"
                                    } else if d <= 0.5 {
                                        "ROUNDING"
                                    } else {
                                        "NAIVE CHECK WOULD FIRE"
                                    };
                                    println!(
                                        "    col {c} '{head}': body {sum:.2} vs stated {st:.2} \
                                         (diff {d:.2}) -> {verdict}"
                                    );
                                }
                            }
                        }
                    }
                    if let Some((c, s, n)) = pct_col {
                        with_pct_col += 1;
                        println!(
                            "  PCT COL    [{name}] {} col {} ({}) — {} rows sum to {:.2}",
                            tb.label,
                            c,
                            g.rows[0].get(c).map(|s| s.as_str()).unwrap_or("?"),
                            n,
                            s
                        );
                    }
                }
            }
        }
    }

    println!("\n================ RT4 INPUT CENSUS ================");
    println!("  tables detected by extract          {detected}");
    println!("  list-of-tables / no body            {toc}");
    println!("  REAL tables with a cell run         {real}");
    println!("  of those, grid recovered            {gridded}");
    println!("  with an explicit TOTALS row         {with_total_row}");
    println!("  with a column summing to ~100       {with_pct_col}");
    println!("\n  grid recovery failures, by reason:");
    for (why, n) in &fail_reasons {
        println!("    {n:>4}  {why}");
    }
    println!("==================================================");
}
