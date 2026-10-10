//! `estimate_in_interval` and `df_matches_n` against their hand-derived golden
//! cases, `fixtures/stats_l0/estimate_interval_fixtures.csv` and
//! `fixtures/stats_l0/df_n_fixtures.csv`. Each case's derivation is the comment
//! line above it in the file.

use gaply_core::extract::{extract_from_text, Location, SectionKind};
use gaply_core::stats_l0::df_matches_n::DfMatchesN;
use gaply_core::stats_l0::estimate_in_interval::EstimateInInterval;
use gaply_core::stats_l0::input::{DocumentContext, L0Input};
use gaply_core::stats_l0::method::{Applicability, L0Verdict, VerificationMethod};
use gaply_core::stats_l0::number::{ReportedNumber, Relation};
use gaply_core::stats_l0::result::{read_results, Family, PValue, ReportedResult, Source, TableStructure};

/// A CSV line with double-quoted fields that hold no quote characters.
fn fields(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
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

fn rows(name: &str, columns: &str) -> Vec<Vec<String>> {
    let path = format!("{}/tests/fixtures/stats_l0/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some(columns), "{name}: column line");
    let width = columns.split(',').count();
    lines
        .map(|l| {
            let f = fields(l);
            assert_eq!(f.len(), width, "{name}: {l:?}");
            f
        })
        .collect()
}

/// The verdict or the applicability, spelled as the fixture spells it.
fn spelled(a: Applicability, run: impl FnOnce() -> L0Verdict) -> String {
    match a {
        Applicability::NotApplicable(why) => format!("NotApplicable({why:?})"),
        Applicability::Applicable => match run() {
            L0Verdict::Consistent(None) => "Consistent".into(),
            L0Verdict::Inconsistent { conclusion_changes: None } => "Inconsistent".into(),
            L0Verdict::Concern(why) => format!("Concern({why:?})"),
            other => format!("{other:?}"),
        },
    }
}

#[test]
fn estimate_in_interval_matches_every_golden_case() {
    let cases = rows("estimate_interval_fixtures.csv", "case,expected,sentence");
    assert!(cases.len() >= 12, "{} cases", cases.len());
    for c in &cases {
        let ex = extract_from_text(&format!("A Title\n\nResults\n{}\n", c[2]));
        let reading = read_results(&ex, TableStructure::NotAvailable);
        assert_eq!(reading.results.len(), 1, "{}: {:?}", c[0], reading.not_read);
        let ctx = DocumentContext::read(&ex, &reading);
        let input = L0Input { result: &reading.results[0], context: &ctx };
        let got = spelled(EstimateInInterval.applicability(&input), || EstimateInInterval.run(&input).outcome);
        assert_eq!(got, c[1], "case {}", c[0]);
    }
}

#[test]
fn df_matches_n_matches_every_golden_case() {
    let cases = rows("df_n_fixtures.csv", "case,family,df,n,partial,expected");
    assert!(cases.len() >= 10, "{} cases", cases.len());
    let ctx = DocumentContext { thresholds: Vec::new(), one_tailed_at: None, correction_at: None };
    for c in &cases {
        let family = match c[1].as_str() {
            "R" => Family::R,
            "T" => Family::T,
            "F" => Family::F,
            "ChiSquare" => Family::ChiSquare,
            other => panic!("family {other:?}"),
        };
        let df = c[2]
            .split(';')
            .map(|d| ReportedNumber::count(d).or_else(|_| ReportedNumber::measured(d)).unwrap())
            .collect();
        let result = ReportedResult {
            id: 0,
            location: Location::in_section(SectionKind::Results, 1, 0),
            raw: String::new(),
            family,
            statistic: ReportedNumber::measured(".31").unwrap(),
            df,
            n: (!c[3].is_empty()).then(|| ReportedNumber::count(&c[3]).unwrap()),
            p: PValue::Reported(ReportedNumber::p_value(".03", Relation::Eq).unwrap()),
            effect_size: None,
            ci: None,
            sentence_says_partial: c[4] == "true",
            source: Source::Unknown,
        };
        let input = L0Input { result: &result, context: &ctx };
        let got = spelled(DfMatchesN.applicability(&input), || DfMatchesN.run(&input).outcome);
        assert_eq!(got, c[5], "case {}", c[0]);
    }
}
