//! `grim_mean`, `grim_percent` and `percent_matches_count` against R.
//!
//! - `fixtures/stats_l0/grim_fixtures.csv`: `scrutiny::grim` at its defaults,
//!   written by `make_grim_fixtures.R` with `/usr/local/bin/Rscript`.
//! - `fixtures/stats_l0/percent_count_fixtures.csv`: `scrutiny::reround` of
//!   100 · count / denominator, written by `make_percent_count_fixtures.R`.
//!
//! Every row is run with the integer-valued flag set. A row where scrutiny says
//! FALSE must be `Inconsistent`, and one where it says TRUE must be
//! `Consistent`, with one named exception: GRIM is `CouldNotCheck(Uninformative)`
//! when every value at the printed precision is reachable, and scrutiny's
//! `grim` returns TRUE there because every value IS reachable. Below N = 100
//! that happens at N = 80 and N = 96 only (`stats_l0::grim` module docs); the
//! test asserts the uninformative rows are exactly those, all TRUE in R, and
//! prints how many there are:
//!
//! ```text
//! cargo test -p gaply_core --test l0_grim_reference -- --nocapture
//! ```

use std::collections::BTreeSet;

use gaply_core::stats_l0::grim::{GrimMean, GrimPercent};
use gaply_core::stats_l0::method::{Applicability, CouldNotCheckReason, L0Verdict, VerificationMethod};
use gaply_core::stats_l0::number::ReportedNumber;
use gaply_core::stats_l0::percent_matches_count::PercentMatchesCount;
use gaply_core::stats_l0::summary::{CountPercentInput, Granularity, GrimInput, SummaryInput};

/// Header lines and data rows of a fixture, after checking the column line.
fn load(name: &str, columns: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let path = format!("{}/tests/fixtures/stats_l0/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let (mut header, mut rows, mut seen) = (Vec::new(), Vec::new(), false);
    for line in text.lines() {
        if let Some(h) = line.strip_prefix('#') {
            header.push(h.trim().to_string());
        } else if !seen {
            assert_eq!(line, columns, "{name}: column line");
            seen = true;
        } else {
            rows.push(line.split(',').map(str::to_string).collect::<Vec<_>>());
        }
    }
    let width = columns.split(',').count();
    assert!(rows.iter().all(|r| r.len() == width), "{name}: a row has the wrong width");
    (header, rows)
}

fn assert_made_by_r(name: &str, header: &[String]) {
    let all = header.join("\n");
    assert!(all.contains("/usr/local/bin/Rscript"), "{name}: not written by /usr/local/bin/Rscript");
    assert!(all.contains("scrutiny 0."), "{name}: header names no scrutiny version");
    assert!(all.contains("R version"), "{name}: header names no R version");
}

fn flag(s: &str) -> bool {
    match s {
        "TRUE" => true,
        "FALSE" => false,
        other => panic!("not a logical: {other:?}"),
    }
}

#[test]
fn grim_agrees_with_scrutiny_on_every_row() {
    let (header, rows) = load("grim_fixtures.csv", "x,n,items,percent,consistent");
    assert_made_by_r("grim_fixtures.csv", &header);
    assert!(rows.len() >= 4500, "only {} rows", rows.len());

    let mut disagreements = Vec::new();
    let mut uninformative_units = BTreeSet::new();
    let (mut uninformative, mut consistent, mut inconsistent) = (0, 0, 0);
    let mut percent_rows = 0;
    for row in &rows {
        let (x, n, items, percent, r_says) = (&row[0], &row[1], &row[2], flag(&row[3]), flag(&row[4]));
        let items: u32 = items.parse().unwrap();
        let g = GrimInput {
            value: ReportedNumber::measured(x).unwrap(),
            n: ReportedNumber::count(n).unwrap(),
            items,
            granularity: Granularity::IntegerValued,
        };
        let (input, method): (_, &dyn VerificationMethod<SummaryInput>) = if percent {
            percent_rows += 1;
            (SummaryInput::Percent(g), &GrimPercent)
        } else {
            (SummaryInput::Mean(g), &GrimMean)
        };
        assert_eq!(method.applicability(&input), Applicability::Applicable, "{row:?}");
        let outcome = method.run(&input).outcome;
        match (outcome, r_says) {
            (L0Verdict::Consistent(None), true) => consistent += 1,
            (L0Verdict::Inconsistent { conclusion_changes: None }, false) => inconsistent += 1,
            (L0Verdict::CouldNotCheck(CouldNotCheckReason::Uninformative), true) => {
                uninformative += 1;
                uninformative_units.insert(n.parse::<u32>().unwrap() * items);
            }
            _ => disagreements.push(format!("{row:?} -> {outcome:?}")),
        }
    }
    println!(
        "grim rows={} (percent {percent_rows}) consistent={consistent} inconsistent={inconsistent} \
         uninformative={uninformative} at N={uninformative_units:?} disagreements={}",
        rows.len(),
        disagreements.len()
    );
    assert!(disagreements.is_empty(), "{} rows disagree:\n{}", disagreements.len(), disagreements.join("\n"));
    assert_eq!(consistent + inconsistent + uninformative, rows.len());
    // Both verdicts must be exercised, or agreement means nothing.
    assert!(consistent > 500 && inconsistent > 500, "{consistent} / {inconsistent}");
    assert_eq!(uninformative_units, BTreeSet::from([80, 96]));
}

#[test]
fn percent_matches_count_agrees_with_scrutiny_on_every_row() {
    let (header, rows) = load("percent_count_fixtures.csv", "count,denominator,percent,consistent");
    assert_made_by_r("percent_count_fixtures.csv", &header);
    assert!(rows.len() >= 1000, "only {} rows", rows.len());
    let mut disagreements = Vec::new();
    let (mut yes, mut no) = (0, 0);
    for row in &rows {
        let input = SummaryInput::CountWithPercent(CountPercentInput {
            count: ReportedNumber::count(&row[0]).unwrap(),
            denominator: ReportedNumber::count(&row[1]).unwrap(),
            percent: ReportedNumber::measured(&row[2]).unwrap(),
        });
        assert_eq!(PercentMatchesCount.applicability(&input), Applicability::Applicable);
        match (PercentMatchesCount.run(&input).outcome, flag(&row[3])) {
            (L0Verdict::Consistent(None), true) => yes += 1,
            (L0Verdict::Inconsistent { conclusion_changes: None }, false) => no += 1,
            (v, _) => disagreements.push(format!("{row:?} -> {v:?}")),
        }
    }
    println!("percent_count rows={} consistent={yes} inconsistent={no} disagreements={}", rows.len(), disagreements.len());
    assert!(disagreements.is_empty(), "{} rows disagree:\n{}", disagreements.len(), disagreements.join("\n"));
    assert!(yes > 200 && no > 200, "{yes} / {no}");
}
