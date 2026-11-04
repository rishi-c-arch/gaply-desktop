//! **The prepared data as CSV bytes (Research OS, Phase 8M, decision 408;
//! ADR-011 Part 8).** Bytes only: the user's save dialog and the frontend's
//! existing save path write them, to the one file the user names. Nothing here
//! touches a file.
//!
//! The data as Gaply read it, with the scores added; not the user's file with
//! columns added. The header row holds the file's headers and then the names
//! the user typed, each row the file's columns in order and then the scores.
//!
//! RFC 4180 with CRLF line ends, as the methods' exports, through the
//! descriptive summary's own field writer (`stats_desc::export::text`): every
//! header, name and text cell quoted, an inner `"` doubled, and one leading
//! apostrophe on a field a spreadsheet would run. **A number never goes
//! through it**, since its guard includes `-`: a number is unquoted, in Rust's
//! shortest form that reads back to the same double. A missing cell is an
//! empty field, whatever made it missing.

use super::Prepared;
use crate::stats_desc::export::{number, text};
use crate::stats_l1::dataset::Cell;

/// The prepared table as CSV bytes (UTF-8, no byte-order mark).
pub fn prepared_csv(prepared: &Prepared) -> Vec<u8> {
    let dataset = &prepared.dataset;
    let columns = &dataset.schema.columns;
    let names = prepared.recipe.scores().map(|s| s.name.as_str());
    let headers = columns
        .iter()
        .take(dataset.file_columns())
        .map(|c| c.header.as_str())
        .chain(names);
    let mut out = String::new();
    out.push_str(&headers.map(text).collect::<Vec<_>>().join(","));
    out.push_str("\r\n");
    let cells: Vec<&[Cell]> = columns
        .iter()
        .filter_map(|c| dataset.column_cells(c.alias))
        .collect();
    let last = cells.len().saturating_sub(1);
    for r in 0..dataset.rows() {
        for (i, column) in cells.iter().enumerate() {
            match &column[r] {
                Cell::Missing => {}
                Cell::Number(v) => out.push_str(&number(*v)),
                Cell::Text(t) => out.push_str(&text(t)),
            }
            if i != last {
                out.push(',');
            }
        }
        out.push_str("\r\n");
    }
    out.into_bytes()
}
