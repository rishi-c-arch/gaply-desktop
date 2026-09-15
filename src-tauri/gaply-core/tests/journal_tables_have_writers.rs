//! **No `journal_*` table may have a reader and no writer.**
//!
//! # The rule was already written down, and nothing enforced it
//!
//! `journal_store.rs`'s header says this module exists because *"§3.4's biggest
//! correction was that `journal_guidelines` had 0 rows after 27 manuscripts and
//! that the count was a count of a table nothing writes to"*, and that *"a
//! schema with no producer is the same artefact §3.4 spent a page correcting"*.
//!
//! **Two of the four tables that module serves were in exactly that state**
//! (§11 D170). `journal_fingerprint::fingerprint_for` queried
//! `journal_standard_bindings` and `journal_expectations`; the only INSERTs
//! anywhere were in `migrations.rs` and one unit test. `bindings_from` and
//! `extract_expectations` both ran, produced values, and every caller discarded
//! them — so a fingerprint's `standards` and `expectations` fields were
//! permanently empty and indistinguishable from "this journal has none".
//!
//! This is the `location_is_unambiguous.rs` shape: a rule stated in prose in the
//! module it governs, true when written, with nothing to keep it true. A header
//! is not a guard.
//!
//! # What it cannot catch
//!
//! 1. **A writer that is never CALLED.** `store_standard_bindings` existing does
//!    not mean a crawl invokes it. That is what the Stage-2 run measures, by
//!    counting rows in a real database rather than counting functions.
//! 2. **A writer reachable only from a test.** The scan excludes `mod tests`
//!    and `tests.rs`, so a test-only INSERT does not count as a producer — which
//!    is the whole point, since that is what `journal_standard_bindings` had.
//! 3. **A table nobody reads or writes.** Dead, not dangerous, and not this
//!    test's business.

use std::path::Path;

/// Where the schema is declared. INSERTs here create the table, they do not
/// populate it in production, so they are not producers.
const MIGRATIONS: &str = "src/migrations.rs";

const SCANNED_DIR: &str = "src";

fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has the crate moved?", dir.display());
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Production source only: everything before `#[cfg(test)]`, and never a
/// `tests.rs`. A test-only INSERT is exactly what this guard must not accept as
/// a producer.
fn production_source(path: &Path, src: &str) -> String {
    if path.file_name().is_some_and(|n| n == "tests.rs") {
        return String::new();
    }
    let cut = src.find("#[cfg(test)]").unwrap_or(src.len());
    src[..cut]
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `journal_*` table the migrations declare.
fn declared_tables(migrations: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in migrations.lines() {
        let l = line.trim();
        let Some(rest) = l.strip_prefix("CREATE TABLE ") else { continue };
        let name = rest.trim_start_matches("IF NOT EXISTS ").trim();
        let name = name.split(['(', ' ']).next().unwrap_or("").trim();
        if name.starts_with("journal_") {
            out.push(name.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn every_journal_table_that_is_read_is_also_written() {
    // Normalised path handling — `location_is_unambiguous.rs` went red on
    // Windows CI for exactly this, and this scan compares paths too.
    let files = rust_files(Path::new(SCANNED_DIR));
    assert!(!files.is_empty(), "the scan found no files — it would pass vacuously");

    let migrations_src = std::fs::read_to_string(MIGRATIONS).expect("migrations.rs");
    let tables = declared_tables(&migrations_src);
    assert!(
        tables.len() >= 4,
        "only {} journal_* tables found in migrations — the parser has drifted from the \
         schema and this guard would pass vacuously: {tables:?}",
        tables.len()
    );

    let mut sources: Vec<(String, String)> = Vec::new();
    for f in &files {
        let rel = f.to_string_lossy().replace('\\', "/");
        if rel.ends_with(MIGRATIONS) {
            continue;
        }
        let src = std::fs::read_to_string(f).expect("read");
        sources.push((rel, production_source(f, &src)));
    }

    let mut offenders = Vec::new();
    let mut rows = Vec::new();
    for table in &tables {
        let mut readers = Vec::new();
        let mut writers = Vec::new();
        for (rel, code) in &sources {
            if code.contains(&format!("FROM {table}")) {
                readers.push(rel.clone());
            }
            if code.contains(&format!("INSERT INTO {table}"))
                || code.contains(&format!("INSERT OR IGNORE INTO {table}"))
                || code.contains(&format!("INSERT OR REPLACE INTO {table}"))
            {
                writers.push(rel.clone());
            }
        }
        rows.push(format!(
            "  {table:<28} readers={} writers={}",
            readers.len(),
            writers.len()
        ));
        if !readers.is_empty() && writers.is_empty() {
            offenders.push(format!(
                "{table}: read by {} file(s) — {} — and written by NONE outside migrations",
                readers.len(),
                readers.join(", ")
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "a journal_* table is read and never written (§11 D170 — a schema with no producer \
         renders as 'this journal has none', which is indistinguishable from 'nobody ever \
         stored one').\n\nWrite the producer in `journal_store.rs` beside the other three, \
         or delete the reader.\n\n{}\n\nfull table:\n{}",
        offenders.join("\n"),
        rows.join("\n")
    );
}
