//! **ambers' findings as closed codes (Research OS, Phase 8I, decision
//! 365).** ambers 0.4.6 lists what a self-contradicting `.sav` declares in
//! English sentences that can name a variable (`sav_ambers_warnings.rs`
//! pins them). Gaply maps each to a code with numbers
//! (`supplementary::sav_findings_of`) and keeps nothing of the text:
//!
//! * each of `sav_ambers_warnings.rs`'s files, read by ambers with no row
//!   limit, maps to its codes, and every sentence ambers' source writes that
//!   no file reaches (the bias and very-long-string ones) maps to its own;
//! * Gaply's own reads set a row limit, under which ambers drops its rows
//!   finding, so `header_rows_mismatch` is derived from the header and the
//!   rows read, and never when the read stopped at the limit;
//! * a sentence no pattern matches is `unrecognised_finding`, counted;
//! * a sentinel variable name inside ambers' text reaches no surface: not
//!   the open's view, not the PublishReady payload, not a `Debug`.
//!
//! A test that returns at once unless `GAPLY_WRITE_CHECKLIST_SAV=1` writes
//! the real-app checklist's file (a header of 0 cases, 20 rows behind it)
//! into `~/gaply-corpus/os/phase8/8i/`.

mod desc_support;
mod sav_support;

use app_lib::runs::DatasetSlots;
use app_lib::stats_desc_commands::open_with;
use app_lib::supplementary::{parse_supplementary_bytes_with, sav_findings_of, IngestLimits};
use desc_support::TempDir;
use gaply_core::contracts::codes::Closed;
use gaply_core::sav_findings::{SavFinding, SavFindingCode as C, SavParam as P};
use sav_support::{fixes, non_positive_case_size, type_field_minus_2, Spec};

fn f(code: C, params: &[(P, f64)]) -> SavFinding {
    SavFinding::new(code, params)
}

/// ambers' own findings for `bytes`, read with no row limit, or its error.
fn ambers_warnings(bytes: &[u8]) -> Result<Vec<String>, String> {
    ambers::read_sav_from_reader(std::io::Cursor::new(bytes.to_vec()))
        .map(|(_, meta)| meta.warnings)
        .map_err(|e| e.to_string())
}

/// `sav_ambers_warnings.rs`'s files, with what each maps to; `None` for the
/// four ambers refuses, which Gaply refuses as malformed and maps to no code.
fn want() -> Vec<(String, Vec<u8>, Option<Vec<SavFinding>>)> {
    let rows = |declared: f64, read: f64| {
        vec![f(
            C::HeaderRowsMismatch,
            &[(P::Declared, declared), (P::Read, read)],
        )]
    };
    let size = |declared: f64| {
        vec![f(
            C::CaseSizeMismatch,
            &[(P::Declared, declared), (P::Used, 2.0)],
        )]
    };
    let missing = vec![f(C::CaseSizeMissing, &[(P::Used, 2.0)])];
    let mut v: Vec<(String, Vec<u8>, Option<Vec<SavFinding>>)> = Vec::new();
    for (name, bytes, _, _) in fixes() {
        let w = match name {
            "zero_cases_uncompressed" | "zero_cases_bytecode" => Some(rows(0.0, 5.0)),
            "case_size_larger_uncompressed" | "case_size_larger_bytecode" => Some(size(3.0)),
            "case_size_smaller_uncompressed" | "case_size_smaller_bytecode" => Some(size(1.0)),
            "date_out_of_range" => Some(vec![f(C::DatesOutOfRange, &[(P::Count, 2.0)])]),
            "code_page" => Some(Vec::new()),
            "duplicate_names"
            | "type_field_256"
            | "label_count_oversized"
            | "info_record_oversized" => None,
            other => panic!("a file this test does not know: {other}"),
        };
        v.push((name.to_string(), bytes, w));
    }
    v.push((
        "type_field_minus_2".into(),
        type_field_minus_2(),
        Some(vec![f(C::TypeContinuation, &[(P::Record, 2.0)])]),
    ));
    for (n, b, _) in non_positive_case_size() {
        v.push((n, b, Some(missing.clone())));
    }
    for name in ["analysis.sav", "l1_weighted.sav"] {
        let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        v.push((
            format!("fixture {name}"),
            std::fs::read(p).unwrap(),
            Some(Vec::new()),
        ));
    }
    v
}

#[test]
fn each_file_maps_to_its_codes() {
    sav_support::require_ambers_0_4_5_or_later("these files");
    let mut wrong = Vec::new();
    let all = want();
    assert_eq!(all.len(), 21, "sav_ambers_warnings.rs's 21 files");
    for (name, bytes, w) in &all {
        match (ambers_warnings(bytes), w) {
            (Ok(texts), Some(w)) => {
                let got = sav_findings_of(&texts, None);
                if &got != w {
                    wrong.push(format!("{name}: {got:?}, want {w:?}"));
                }
            }
            (Err(_), None) => {}
            (got, w) => wrong.push(format!("{name}: {:?}, want {w:?}", got.map(|t| t.len()))),
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// The sentences ambers 0.4.6's source writes that no file above reaches,
/// as its `format!` calls write them (scanner.rs:105, dictionary.rs:232,
/// :240, :325), with a name in each where ambers puts one.
#[test]
fn the_sentences_no_file_reaches_map_to_their_codes() {
    let texts: Vec<String> = vec![
        "compression bias in the header is 99.5 instead of the standard 100; compressed integer values may be shifted by 0.5 (SPSS reads such a file assuming 100)".into(),
        "zsav trailer declares a compression bias of 7 instead of the standard 100".into(),
        "very long string record declares width 100 for \"LONGA\"; ignored (valid range 256-32767)".into(),
        "very long string record declares width 1000 for \"LONGB\" needing 4 segments but only 2 variables remain; ignored".into(),
        "variable record 0 (\"FIRST\") declares type -257; treated as a continuation slot".into(),
    ];
    assert_eq!(
        sav_findings_of(&texts, None),
        vec![
            f(C::TypeContinuation, &[(P::Record, 1.0)]),
            f(C::CompressionBias, &[(P::Bias, 99.5)]),
            f(C::CompressionBias, &[(P::Bias, 7.0)]),
            f(C::VeryLongStringIgnored, &[(P::Count, 2.0)]),
        ]
    );
}

#[test]
fn a_sentence_no_pattern_matches_is_counted_as_unrecognised() {
    let texts: Vec<String> = vec![
        "a sentence a later ambers might write".into(),
        "header declares 3 values per row but the variable list defines 2; used 2".into(),
        "header declares many rows but 5 rows were read".into(),
    ];
    assert_eq!(
        sav_findings_of(&texts, None),
        vec![
            f(C::CaseSizeMismatch, &[(P::Declared, 3.0), (P::Used, 2.0)]),
            f(C::UnrecognisedFinding, &[(P::Count, 2.0)]),
        ]
    );
}

/// Under Gaply's own limit ambers gives no rows finding, so the code is
/// derived; a read that stopped at the limit says nothing of the header.
#[test]
fn the_header_rows_mismatch_is_derived_on_a_limited_read() {
    let mut five = Spec::numbers(&["x", "y"], &sav_support::five_rows());
    five.cases = Some(0);
    let bytes = five.bytes();
    let ev = parse_supplementary_bytes_with(&bytes, "sav", IngestLimits::METHODS).unwrap();
    assert_eq!(
        ev.sav_findings,
        vec![f(
            C::HeaderRowsMismatch,
            &[(P::Declared, 0.0), (P::Read, 5.0)]
        )]
    );
    // The header's own count, when it is the rows read: nothing.
    five.cases = Some(5);
    let ev = parse_supplementary_bytes_with(&five.bytes(), "sav", IngestLimits::METHODS).unwrap();
    assert_eq!(ev.sav_findings, Vec::new());
    // A header of 9 with 5 rows behind it: 9 and 5.
    five.cases = Some(9);
    let ev = parse_supplementary_bytes_with(&five.bytes(), "sav", IngestLimits::METHODS).unwrap();
    assert_eq!(
        ev.sav_findings,
        vec![f(
            C::HeaderRowsMismatch,
            &[(P::Declared, 9.0), (P::Read, 5.0)]
        )]
    );
    // A read that stopped at the limit (3 rows allowed, 4 read): nothing.
    let small = IngestLimits {
        max_rows: 3,
        ..IngestLimits::METHODS
    };
    let ev = parse_supplementary_bytes_with(&bytes, "sav", small).unwrap();
    assert!(ev.rows_truncated, "the read stopped at the limit");
    assert_eq!(ev.sav_findings, Vec::new());
    // The dates ambers nulls while reading are in the limited read too.
    let dates = fixes()
        .into_iter()
        .find(|x| x.0 == "date_out_of_range")
        .unwrap()
        .1;
    let ev = parse_supplementary_bytes_with(&dates, "sav", IngestLimits::METHODS).unwrap();
    assert_eq!(
        ev.sav_findings,
        vec![f(C::DatesOutOfRange, &[(P::Count, 2.0)])]
    );
}

/// The methods' open carries the codes to the screen's view, as codes and
/// numbers, and a view with none has no such key.
#[test]
fn the_open_shows_the_codes_beside_the_profile() {
    let dir = TempDir::new("sav_findings_open");
    let mut s = Spec::numbers(&["x", "y"], &sav_support::five_rows());
    s.cases = Some(0);
    let p = dir.write("zero.sav", &s.bytes());
    let v = open_with(&DatasetSlots::default(), &p, None, IngestLimits::METHODS).unwrap();
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(
        json["fileFindings"],
        serde_json::json!([{"code": "sav.header_rows_mismatch", "params": {"declared": 0.0, "read": 5.0}}])
    );
    s.cases = Some(5);
    let p = dir.write("five.sav", &s.bytes());
    let v = open_with(&DatasetSlots::default(), &p, None, IngestLimits::METHODS).unwrap();
    let json = serde_json::to_value(&v).unwrap();
    assert!(json.get("fileFindings").is_none(), "{json}");
}

/// A variable named by a sentinel inside ambers' text (its record's type
/// -2): the code and its record number reach the view, the name and the
/// text reach nothing. The record holds no column, so the name is nowhere
/// else in the file's view.
#[test]
fn a_name_inside_ambers_text_reaches_no_surface() {
    const SENTINEL: &str = "QZSENTNL";
    let mut s = Spec::numbers(
        &["x", SENTINEL, "z"],
        &[
            vec![1.0, 9.0, 2.0],
            vec![3.0, 9.0, 4.0],
            vec![5.0, 9.0, 7.0],
        ],
    );
    s.vars[1].type_field = -2;
    let bytes = s.bytes();
    // Positive control: ambers' text names it.
    let texts = ambers_warnings(&bytes).unwrap();
    assert!(texts.iter().any(|t| t.contains(SENTINEL)), "{texts:?}");
    let dir = TempDir::new("sav_findings_privacy");
    let p = dir.write("named.sav", &bytes);
    let view = open_with(&DatasetSlots::default(), &p, None, IngestLimits::METHODS).unwrap();
    let view_json = serde_json::to_string(&view).unwrap();
    assert!(view_json.contains("sav.type_continuation"), "{view_json}");
    let ev = parse_supplementary_bytes_with(&bytes, "sav", IngestLimits::SUPPLEMENTARY).unwrap();
    assert_eq!(
        ev.sav_findings,
        vec![f(C::TypeContinuation, &[(P::Record, 2.0)])]
    );
    let surfaces = [
        ("the open's view", view_json),
        (
            "the PublishReady payload",
            serde_json::to_string(&ev).unwrap(),
        ),
        ("the evidence's Debug", format!("{ev:?}")),
        ("the findings' Debug", format!("{:?}", ev.sav_findings)),
    ];
    for (name, text) in &surfaces {
        for needle in [
            SENTINEL,
            "continuation slot",
            "declares type",
            "variable record",
        ] {
            assert!(!text.contains(needle), "{needle} reached {name}: {text}");
        }
    }
    // Every word on any surface is one of the closed set.
    for c in C::ALL {
        assert!(c.wire().starts_with("sav."));
    }
}

/// The real-app checklist's file (decision 365, realapp_8i.md): a header of
/// 0 cases with 20 rows of three numeric items behind it. Returns at once
/// unless `GAPLY_WRITE_CHECKLIST_SAV=1`.
#[test]
fn write_the_checklists_sav() {
    if std::env::var("GAPLY_WRITE_CHECKLIST_SAV").as_deref() != Ok("1") {
        return;
    }
    let rows: Vec<Vec<f64>> = (0..20)
        .map(|i| {
            let i = i as f64;
            vec![
                1.0 + (i % 5.0),
                1.0 + ((i * 3.0) % 5.0),
                1.0 + ((i * 2.0 + 1.0) % 5.0),
            ]
        })
        .collect();
    let mut s = Spec::numbers(&["item1", "item2", "item3"], &rows);
    s.cases = Some(0);
    let dir =
        std::path::Path::new(&std::env::var("HOME").unwrap()).join("gaply-corpus/os/phase8/8i");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("HT8I_zero_cases.sav");
    std::fs::write(&path, s.bytes()).unwrap();
    let ev = parse_supplementary_bytes_with(&s.bytes(), "sav", IngestLimits::METHODS).unwrap();
    assert_eq!(
        ev.sav_findings,
        vec![f(
            C::HeaderRowsMismatch,
            &[(P::Declared, 0.0), (P::Read, 20.0)]
        )]
    );
    println!("WROTE {} ({} bytes)", path.display(), s.bytes().len());
}
