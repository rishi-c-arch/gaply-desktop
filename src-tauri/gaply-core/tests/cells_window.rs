//! **The window of cells the Data View reads (Research OS, Phase 8Q, decisions
//! 458 and 464; ADR-012 Part 3).**
//!
//! * Windows of every shape over a small table equal the prepared CSV's fields,
//!   cell by cell, so the grid and the file the user saves cannot disagree.
//! * The clamps at each edge, a window wholly past the end, and each cap.
//! * One number rule: `export::number`'s body holds the only float format in
//!   `stats_desc/` and `stats_prep/export.rs`, by a source scan with comments
//!   blanked.

use std::path::{Path, PathBuf};

use gaply_core::contracts::CapabilityId;
use gaply_core::stats_desc::cells::{
    cells_window, MAX_WINDOW_COLUMNS, MAX_WINDOW_ROWS, MAX_WINDOW_TEXT_BYTES,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_prep::{
    build, export::prepared_csv, MissingRule, Prepared, Recipe, RecipeStep, ScaleScoreStep,
    Statistic, CAPABILITY_ID, CAPABILITY_VERSION, ENGINE_VERSION, RECIPE_SCHEMA_VERSION,
};

fn dataset(headers: &[String], rows: Vec<Vec<String>>) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.to_vec(), rows),
        DataFingerprint::of_bytes(b"cells window", "cells window parser", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Numbers in every form the printer has, missing cells, and text a CSV field
/// guards and quotes, with a score appended so the table is a prepared one.
fn prepared() -> Prepared {
    let headers = strs(&["q1", "q2", "note", "plain"]);
    let rows = vec![
        strs(&["1.50", "2", "=SUM(A1)", "alpha"]),
        strs(&["-0", "", "-minus", ""]),
        strs(&["1e300", "4", "say \"hi\"", "gamma"]),
        strs(&["", "NA", "'quoted", "delta"]),
        strs(&["0.1", "5", "", "with, comma"]),
        strs(&["12345678901234567890", "1", "+plus", "@at"]),
    ];
    let table = dataset(&headers, rows);
    let step = ScaleScoreStep {
        alias: Alias(ColumnAlias(5)),
        name: "total".to_string(),
        items: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(2))],
        reversed: Vec::new(),
        scale: None,
        statistic: Some(Statistic::Sum),
        rule: Some(MissingRule::EveryItem),
        missing_codes: Vec::new(),
    };
    let recipe = Recipe {
        schema_version: RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(CAPABILITY_ID).unwrap(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        source_columns: 4,
        steps: vec![RecipeStep::ScaleScore(step)],
    };
    build(&table, &recipe, 5_000_000).expect("prepared")
}

/// One CSV field, as a reader takes it: `None` for an empty field, the number
/// as it stands, a quoted field unquoted with its one guard apostrophe off.
fn parse_csv(bytes: &[u8]) -> Vec<Vec<Option<String>>> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut rows = Vec::new();
    for line in text.split("\r\n").filter(|l| !l.is_empty()) {
        let mut fields = Vec::new();
        let mut chars = line.chars().peekable();
        loop {
            let mut field = String::new();
            let mut quoted = false;
            if chars.peek() == Some(&'"') {
                quoted = true;
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '"' {
                        if chars.peek() == Some(&'"') {
                            chars.next();
                            field.push('"');
                        } else {
                            break;
                        }
                    } else {
                        field.push(c);
                    }
                }
            } else {
                while let Some(&c) = chars.peek() {
                    if c == ',' {
                        break;
                    }
                    field.push(c);
                    chars.next();
                }
            }
            if quoted {
                let unguarded = field.strip_prefix('\'').unwrap_or(&field).to_string();
                fields.push(Some(unguarded));
            } else if field.is_empty() {
                fields.push(None);
            } else {
                fields.push(Some(field));
            }
            if chars.next().is_none() {
                break;
            }
        }
        rows.push(fields);
    }
    rows
}

#[test]
fn windows_of_every_shape_equal_the_prepared_csvs_fields_cell_by_cell() {
    let prepared = prepared();
    let table = parse_csv(&prepared_csv(&prepared));
    let body = &table[1..];
    let (rows, columns) = (body.len(), body[0].len());
    assert_eq!((rows, columns), (6, 5));
    // The reader takes off exactly the one apostrophe the guard put on, so a
    // text cell comes back whole, a leading apostrophe of its own included.
    assert_eq!(body[0][2].as_deref(), Some("=SUM(A1)"));
    assert_eq!(body[3][2].as_deref(), Some("'quoted"));

    let dataset = &prepared.dataset;
    let mut windows = 0;
    let mut compared = 0;
    for first_row in 0..=rows + 1 {
        for row_count in 0..=rows + 2 {
            for first_column in 0..=columns + 1 {
                for column_count in 0..=columns + 2 {
                    let w = cells_window(dataset, first_row, row_count, first_column, column_count);
                    windows += 1;
                    assert_eq!((w.rows, w.columns), (rows, columns));
                    assert_eq!(w.first_row, first_row.min(rows));
                    assert_eq!(w.first_column, first_column.min(columns));
                    let want_rows = row_count.min(rows.saturating_sub(first_row));
                    let want_columns = column_count.min(columns.saturating_sub(first_column));
                    let want_rows = if want_columns == 0 { 0 } else { want_rows };
                    assert_eq!(w.cells.len(), want_rows);
                    for (r, row) in w.cells.iter().enumerate() {
                        assert_eq!(row.len(), want_columns);
                        for (c, cell) in row.iter().enumerate() {
                            let field = &body[first_row + r][first_column + c];
                            assert_eq!(cell, field, "r{r} c{c}");
                            compared += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(windows > 500 && compared > 1000, "{windows} {compared}");
}

#[test]
fn a_cell_is_what_the_table_holds_with_no_csv_guard_and_missing_as_none() {
    let prepared = prepared();
    let w = cells_window(&prepared.dataset, 0, 6, 0, 5);
    let at = |r: usize, c: usize| w.cells[r][c].as_deref();
    assert_eq!(at(0, 0), Some("1.5"), "a number, shortest form");
    assert_eq!(at(1, 0), Some("-0"), "a negative zero keeps its sign");
    assert_eq!(at(2, 0).map(str::len), Some(301), "1e300 at all its digits");
    assert_eq!(at(5, 0), Some("12345678901234567000"), "a long number");
    assert_eq!(at(1, 1), None, "an empty cell");
    assert_eq!(at(3, 1), None, "a missing token");
    assert_eq!(at(0, 2), Some("=SUM(A1)"), "text with no guard apostrophe");
    assert_eq!(at(1, 2), Some("-minus"));
    assert_eq!(at(2, 2), Some("say \"hi\""), "text with no doubled quote");
    assert_eq!(at(3, 2), Some("'quoted"), "a leading apostrophe stays");
    assert_eq!(at(0, 4), Some("3.5"), "a score reads like any number");
    assert_eq!(at(1, 4), None, "a row not scored is empty");
}

/// `rows` rows by `columns` columns, each cell its own row and column.
fn numbered(rows: usize, columns: usize) -> Dataset {
    let headers: Vec<String> = (0..columns).map(|c| format!("h{c}")).collect();
    let body = (0..rows)
        .map(|r| {
            (0..columns)
                .map(|c| (r * 1000 + c).to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    dataset(&headers, body)
}

#[test]
fn a_window_clamps_at_the_last_row() {
    let d = numbered(10, 3);
    let w = cells_window(&d, 8, 5, 0, 1);
    assert_eq!(w.first_row, 8);
    assert_eq!(w.cells.len(), 2);
    assert_eq!(w.cells[1][0].as_deref(), Some("9000"));
}

#[test]
fn a_window_clamps_at_the_last_column() {
    let d = numbered(2, 10);
    let w = cells_window(&d, 0, 1, 8, 5);
    assert_eq!(w.first_column, 8);
    assert_eq!(w.cells[0].len(), 2);
    assert_eq!(w.cells[0][1].as_deref(), Some("9"));
}

#[test]
fn a_window_wholly_past_the_end_has_no_cells_and_says_where_the_end_is() {
    let d = numbered(10, 3);
    let past_rows = cells_window(&d, 10, 5, 0, 3);
    assert_eq!((past_rows.first_row, past_rows.cells.len()), (10, 0));
    let far_past = cells_window(&d, usize::MAX, usize::MAX, 0, 3);
    assert_eq!((far_past.first_row, far_past.cells.len()), (10, 0));
    let past_columns = cells_window(&d, 0, 5, 3, 5);
    assert_eq!(
        (past_columns.first_column, past_columns.cells.len()),
        (3, 0)
    );
    let far_columns = cells_window(&d, 0, 5, usize::MAX, usize::MAX);
    assert_eq!((far_columns.first_column, far_columns.cells.len()), (3, 0));
}

#[test]
fn the_row_cap_is_200() {
    assert_eq!(MAX_WINDOW_ROWS, 200);
    let d = numbered(205, 1);
    let w = cells_window(&d, 2, 500, 0, 1);
    assert_eq!(w.cells.len(), 200);
    assert_eq!(w.cells[199][0].as_deref(), Some("201000"));
}

#[test]
fn the_column_cap_is_64() {
    assert_eq!(MAX_WINDOW_COLUMNS, 64);
    let d = numbered(1, 70);
    let w = cells_window(&d, 0, 1, 2, 500);
    assert_eq!(w.cells[0].len(), 64);
    assert_eq!(w.cells[0][63].as_deref(), Some("65"));
}

/// 200 rows by 64 columns of 512 four-byte characters: 2,048 bytes a cell and
/// 131,072 a row, so exactly 32 rows are exactly 4 MiB.
fn heavy(extra_in_row_32: bool) -> Dataset {
    let cell = "\u{1D11E}".repeat(512);
    let headers: Vec<String> = (0..64).map(|c| format!("h{c}")).collect();
    let body = (0..200)
        .map(|r| {
            (0..64)
                .map(|c| {
                    if extra_in_row_32 && r == 31 && c == 0 {
                        format!("{cell}x")
                    } else {
                        cell.clone()
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();
    dataset(&headers, body)
}

#[test]
fn the_text_cap_is_4_mib_in_utf8_bytes_and_stops_at_a_whole_row() {
    assert_eq!(MAX_WINDOW_TEXT_BYTES, 4 * 1024 * 1024);
    let exact = cells_window(&heavy(false), 0, 200, 0, 64);
    assert_eq!(exact.cells.len(), 32, "exactly 4 MiB fits");
    assert!(exact.cells.iter().all(|r| r.len() == 64), "whole rows");

    let over = cells_window(&heavy(true), 0, 200, 0, 64);
    assert_eq!(over.cells.len(), 31, "one byte more stops a row sooner");
    let bytes: usize = over.cells.iter().flatten().flatten().map(String::len).sum();
    assert!(bytes <= MAX_WINDOW_TEXT_BYTES, "{bytes}");
}

#[test]
fn the_text_cap_counts_a_numbers_printed_bytes_too() {
    // A row of 130,972 bytes of text and one number that prints at 309. Counting the
    // text alone, 32 rows fit in 4 MiB; counting the number too, 31 do.
    let wide = "\u{1D11E}".repeat(512);
    let last = "x".repeat(3996);
    let headers: Vec<String> = (0..64).map(|c| format!("h{c}")).collect();
    let body = (0..200)
        .map(|_| {
            let mut row = vec!["1e308".to_string()];
            row.extend((0..62).map(|_| wide.clone()));
            row.push(last.clone());
            row
        })
        .collect();
    let d = dataset(&headers, body);
    let w = cells_window(&d, 0, 200, 0, 64);
    assert_eq!(w.cells[0][0].as_deref().map(str::len), Some(309));
    let text_alone: usize = w.cells[0][1..].iter().flatten().map(String::len).sum();
    assert_eq!(text_alone, 130_972);
    assert_eq!(32 * text_alone <= MAX_WINDOW_TEXT_BYTES, true);
    assert_eq!(w.cells.len(), 31);
}

/* ---- one number rule ------------------------------------------------------ */

fn core_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// `//` comments blanked, line for line, and a `//` inside a string kept.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| {
            let mut in_str = false;
            let b = l.as_bytes();
            for i in 0..b.len() {
                match b[i] {
                    b'"' if i == 0 || b[i - 1] != b'\\' => in_str = !in_str,
                    b'/' if !in_str && b.get(i + 1) == Some(&b'/') => return &l[..i],
                    _ => {}
                }
            }
            l
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn read(p: &Path) -> String {
    code_only(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
}

/// The text of `fn name`'s body, by counting braces from its signature.
fn body_of(src: &str, signature: &str) -> String {
    let start = src
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` is gone: name where it went"));
    let mut depth = 0usize;
    let mut opened = false;
    for (i, c) in src[start..].char_indices() {
        match c {
            '{' => {
                depth += 1;
                opened = true;
            }
            '}' => {
                depth -= 1;
                if opened && depth == 0 {
                    return src[start..start + i + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("`{signature}` has no closed body");
}

/// The formatting forms a float can take here, as typed: a lone placeholder
/// over one value, inline or positional, and `to_string` on the two names these
/// files give a float. A guarded text field (`"'{field}"`, `"\"{}\""`) is not a
/// lone placeholder.
fn float_formats(src: &str) -> usize {
    let mut n = 0;
    let mut rest = src;
    while let Some(at) = rest.find("format!(\"{") {
        let tail = &rest[at + "format!(\"{".len()..];
        let close = tail.find('}').unwrap_or(0);
        let inner = &tail[..close];
        let lone = tail[close..].starts_with("}\"")
            && inner
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == ':' || c == '.');
        if lone {
            n += 1;
        }
        rest = tail;
    }
    n + src.matches("v.to_string()").count()
        + src.matches("write!(").count()
        + src.matches("writeln!(").count()
}

#[test]
fn numbers_body_holds_the_only_float_format_and_the_three_writers_call_it() {
    let desc = core_src().join("stats_desc");
    let export = read(&desc.join("export.rs"));
    let number = body_of(&export, "fn number(v: f64)");
    assert_eq!(float_formats(&number), 1, "number's body: {number}");

    let mut scanned = 0;
    for entry in std::fs::read_dir(&desc).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let src = read(&path);
        let formats = float_formats(&src);
        let allowed = usize::from(path.ends_with("export.rs"));
        assert_eq!(
            formats,
            allowed,
            "{} formats a float itself",
            path.display()
        );
        scanned += 1;
    }
    assert!(
        scanned >= 3,
        "{scanned} stats_desc files: the scan is inert"
    );

    let prep = read(&core_src().join("stats_prep/export.rs"));
    assert_eq!(
        float_formats(&prep),
        0,
        "stats_prep/export.rs formats a float"
    );

    let cells = read(&desc.join("cells.rs"));
    for (what, body) in [
        ("stat_fields", body_of(&export, "fn stat_fields(")),
        ("prepared_csv", body_of(&prep, "fn prepared_csv(")),
        ("cells_window", body_of(&cells, "fn cells_window(")),
    ] {
        assert!(body.contains("number("), "{what} does not call number(");
    }
}
