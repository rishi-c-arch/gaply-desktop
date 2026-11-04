//! **A window of a table's cells, for the user's own screen (Research OS, Phase
//! 8Q, decision 458; ADR-012 Part 3).** Pure: it reads what the dataset holds
//! and returns strings, and nothing stays anywhere when its caller has sent
//! them. The Data View is the first screen to show a table's rows, and this is
//! the only way to them: the whole table never crosses at once.
//!
//! A number goes through [`number`], the rule the prepared CSV's numbers go
//! through, so the grid and the file the user saves cannot print one value two
//! ways. It stays a string on the wire: serde writes an infinity as `null`,
//! which here means missing, and JS reprints a large number its own way, so the
//! screen prints what Rust wrote. Text is as read and never through the CSV's
//! `text`, since the grid is not a spreadsheet and a guard apostrophe would be a
//! character the file does not hold. Missing is `None`, whatever made it missing.
//!
//! A request is clamped and says what it holds, so there is no refusal here. A
//! window wholly past the end has no cells, and its `first_row` or `first_column`
//! is the table's size. A cap on text stops at a whole row, so a window's rows
//! are always whole; the caller reads `cells.len()`.
//!
//! [`CellWindow`] derives nothing: it holds cells, so it has no `Debug` that
//! could print one, and it is not itself what crosses to the screen (the app's
//! `CellWindowView` is).

use super::export::number;
use crate::stats_l1::dataset::{Cell, Dataset};

/// At most this many rows in one window.
pub const MAX_WINDOW_ROWS: usize = 200;
/// At most this many columns in one window.
pub const MAX_WINDOW_COLUMNS: usize = 64;
/// At most this much cell text in one window, numbers' printed bytes included,
/// counted in UTF-8 bytes.
pub const MAX_WINDOW_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// What a window holds, after clamping. `rows` and `columns` are the table's
/// whole size; `cells` holds whole rows, by row, `None` for a missing cell.
pub struct CellWindow {
    pub first_row: usize,
    pub first_column: usize,
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<Vec<Option<String>>>,
}

/// The cells from `first_row` and `first_column`, clamped to the table and to
/// the caps.
pub fn cells_window(
    dataset: &Dataset,
    first_row: usize,
    row_count: usize,
    first_column: usize,
    column_count: usize,
) -> CellWindow {
    let rows = dataset.rows();
    let columns = dataset.schema.columns.len();
    let first_row = first_row.min(rows);
    let first_column = first_column.min(columns);
    let row_end = first_row + row_count.min(MAX_WINDOW_ROWS).min(rows - first_row);
    let column_count = column_count
        .min(MAX_WINDOW_COLUMNS)
        .min(columns - first_column);

    let chosen: Vec<&[Cell]> = dataset
        .schema
        .columns
        .iter()
        .skip(first_column)
        .take(column_count)
        .filter_map(|c| dataset.column_cells(c.alias))
        .collect();

    let mut cells = Vec::new();
    if !chosen.is_empty() {
        let mut text_bytes = 0usize;
        for r in first_row..row_end {
            let row: Vec<Option<String>> = chosen
                .iter()
                .map(|c| match &c[r] {
                    Cell::Missing => None,
                    Cell::Number(v) => Some(number(*v)),
                    Cell::Text(t) => Some(t.clone()),
                })
                .collect();
            let row_bytes: usize = row.iter().flatten().map(String::len).sum();
            if text_bytes + row_bytes > MAX_WINDOW_TEXT_BYTES {
                break;
            }
            text_bytes += row_bytes;
            cells.push(row);
        }
    }
    CellWindow {
        first_row,
        first_column,
        rows,
        columns,
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };

    /// The typing pass keeps an infinity as text and a `NaN` token as missing,
    /// so a number that is not finite is reached only by putting it there. If
    /// one ever is there, it prints as a string, and never as `None`.
    #[test]
    fn a_number_that_is_not_finite_is_still_a_printed_string() {
        let mut d = build_dataset(
            RawTable::from_rows(
                vec!["a".to_string()],
                vec![
                    vec!["1".to_string()],
                    vec!["2".to_string()],
                    vec!["3".to_string()],
                ],
            ),
            DataFingerprint::of_bytes(b"cells", "cells parser", TableRef::FirstTable),
        )
        .unwrap();
        d.replace_cell_for_test(ColumnAlias(1), 0, Cell::Number(f64::INFINITY));
        d.replace_cell_for_test(ColumnAlias(1), 1, Cell::Number(f64::NEG_INFINITY));
        d.replace_cell_for_test(ColumnAlias(1), 2, Cell::Number(f64::NAN));
        let w = cells_window(&d, 0, 3, 0, 1);
        let got: Vec<Option<&str>> = w.cells.iter().map(|r| r[0].as_deref()).collect();
        assert_eq!(got, [Some("inf"), Some("-inf"), Some("NaN")]);
    }

    /// What the file's own reader does with an infinity, which is why the Data
    /// View shows it as the user typed it (`1e999` stays `1e999`).
    #[test]
    fn an_infinity_in_a_file_is_text_as_read_and_a_nan_token_is_missing() {
        let d = build_dataset(
            RawTable::from_rows(
                vec!["a".to_string()],
                vec![
                    vec!["1".to_string()],
                    vec!["1e999".to_string()],
                    vec!["NaN".to_string()],
                ],
            ),
            DataFingerprint::of_bytes(b"cells", "cells parser", TableRef::FirstTable),
        )
        .unwrap();
        let w = cells_window(&d, 0, 3, 0, 1);
        let got: Vec<Option<&str>> = w.cells.iter().map(|r| r[0].as_deref()).collect();
        assert_eq!(got, [Some("1"), Some("1e999"), None]);
    }
}
