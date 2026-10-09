//! Where `p_consistency` departs from statcheck ON PURPOSE, one invented
//! sentence per departure.
//!
//! `tests/fixtures/stats_l0/statcheck_diff_fixtures.csv` records what statcheck
//! returns for each sentence (written by `make_statcheck_diff_fixtures.R` with
//! `/usr/local/bin/Rscript`; the statcheck version is in its header). This test
//! asserts both behaviours: the statcheck columns that show the departure, and
//! Gaply's verdict for the same sentence. If a statcheck release stops
//! departing, the first half fails and says which departure is gone.

use std::collections::BTreeMap;

use gaply_core::extract::extract_from_text;
use gaply_core::stats_l0::input::{DocumentContext, L0Input};
use gaply_core::stats_l0::method::{CheckNote, ConcernReason, CouldNotCheckReason, L0Verdict, VerificationMethod};
use gaply_core::stats_l0::p_consistency::PConsistency;
use gaply_core::stats_l0::result::{read_results, TableStructure};

/// A CSV line as R's `write.table(quote = TRUE)` writes it. Quoted fields here
/// hold no quote characters; that is asserted, not assumed.
fn fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    assert!(!quoted, "unbalanced quotes in {line:?}");
    out.push(cur);
    out
}

/// departure -> its row, keyed by the column names in the file's header.
fn load() -> (Vec<String>, BTreeMap<String, BTreeMap<String, String>>) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_l0/statcheck_diff_fixtures.csv");
    let text = std::fs::read_to_string(path).expect("statcheck_diff_fixtures.csv is present");
    let mut header = Vec::new();
    let mut cols: Option<Vec<String>> = None;
    let mut rows = BTreeMap::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix('#') {
            header.push(h.trim().to_string());
            continue;
        }
        assert_eq!(line.matches('"').count() % 2, 0, "{line:?}");
        let f = fields(line);
        match &cols {
            None => cols = Some(f),
            Some(c) => {
                assert_eq!(f.len(), c.len(), "{line:?}");
                let row: BTreeMap<String, String> = c.iter().cloned().zip(f).collect();
                assert!(rows.insert(row["departure"].clone(), row).is_none(), "one row per departure");
            }
        }
    }
    (header, rows)
}

fn gaply(sentence: &str) -> L0Verdict {
    let ex = extract_from_text(&format!("A Title\n\nResults\n{sentence}\n"));
    let reading = read_results(&ex, TableStructure::NotAvailable);
    assert_eq!(reading.results.len(), 1, "{sentence:?}: {:?}", reading.not_read);
    let ctx = DocumentContext::read(&ex, &reading);
    PConsistency.run(&L0Input { result: &reading.results[0], context: &ctx }).outcome
}

#[test]
fn each_departure_from_statcheck_is_shown_on_both_sides() {
    let (header, rows) = load();
    assert!(header.iter().any(|h| h.contains("/usr/local/bin/Rscript")), "{header:?}");
    assert!(header.iter().any(|h| h.starts_with("statcheck ")), "{header:?}");

    // (departure, statcheck columns that show it, Gaply's verdict)
    let cases: [(&str, &[(&str, &str)], L0Verdict); 7] = [
        // statcheck reads "p = .000" as p = 0 and counts it an error; Gaply
        // reads it as p < .0005, which t(28) = 4.20 (p near .00025) satisfies.
        ("p_printed_as_zero", &[("n_results", "1"), ("error", "TRUE")], L0Verdict::Consistent(None)),
        // statcheck recomputes an adjusted p as if it were raw; Gaply declines.
        (
            "adjusted_p_with_correction_named",
            &[("n_results", "1"), ("error", "TRUE"), ("decision_error", "TRUE")],
            L0Verdict::CouldNotCheck(CouldNotCheckReason::AdjustedP),
        ),
        // A p that is right one-tailed, with no word saying so: statcheck an
        // error, Gaply a Concern.
        (
            "one_tailed_unstated",
            &[("n_results", "1"), ("error", "TRUE"), ("one_tailed_in_txt", "FALSE")],
            L0Verdict::Concern(ConcernReason::OneTailedUnstated),
        ),
        // statcheck judges the decision at .05; Gaply at the stated .01, where
        // reported .02 and recomputed .068 are both non-significant.
        (
            "alpha_other_than_05",
            &[("n_results", "1"), ("error", "TRUE"), ("decision_error", "TRUE")],
            L0Verdict::Inconsistent { conclusion_changes: Some(false) },
        ),
        // statcheck extracts nothing when an effect size sits between the
        // statistic and the p; Gaply reads the unit and checks it.
        ("effect_size_between_statistic_and_p", &[("n_results", "0")], L0Verdict::Consistent(None)),
        // statcheck reads "p = 0.0001" as an exact value and counts it an
        // error; Gaply reads it as a software floor (true p near 1.4e-12).
        (
            "p_floor_printed_as_equality",
            &[("n_results", "1"), ("error", "TRUE"), ("decision_error", "FALSE")],
            L0Verdict::Consistent(Some(CheckNote::FloorPrintedAsEquality)),
        ),
        // statcheck takes every F as upper-tail; Gaply flags an F that only a
        // two-tailed variance-ratio test explains (upper .0035, two-tailed .0071).
        (
            "f_two_tailed_variance_ratio",
            &[("n_results", "1"), ("error", "TRUE"), ("decision_error", "FALSE")],
            L0Verdict::Concern(ConcernReason::FTwoTailed),
        ),
    ];

    let mut checked = 0;
    for (departure, statcheck, ours) in cases {
        let row = rows.get(departure).unwrap_or_else(|| panic!("{departure}: no fixture row"));
        for (col, want) in statcheck {
            let got = row.get(*col).unwrap_or_else(|| panic!("{departure}: statcheck column {col} is absent"));
            assert_eq!(got, want, "{departure}: statcheck's {col} no longer shows the departure");
        }
        assert_eq!(gaply(&row["sentence"]), ours, "{departure}: Gaply's verdict");
        checked += 1;
    }
    assert_eq!(checked, rows.len(), "every fixture row is a named departure");
}
