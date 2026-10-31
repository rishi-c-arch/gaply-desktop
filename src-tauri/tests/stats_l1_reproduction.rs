//! **The Phase 2 exit test, through the seam, with no helper and no network.**
//!
//! A synthetic manuscript reporting six results, one per method family, and a
//! synthetic CSV that reproduces all six (`gaply-core/tests/fixtures/stats_l1/
//! make_l1_exit_fixture.R`, written by `/usr/local/bin/Rscript`: every printed
//! number is R's, APA style). The path is the one a user's run takes: Level 0
//! reads the manuscript, Level 1 turns its results into claims, the data file
//! is loaded, each claim's binding is confirmed, and
//! `run_stats_reproduction_measured` reads the file again and decides.
//!
//! Then one printed statistic is moved beyond its rounding interval: that
//! claim alone becomes NotReproduced and the other five outcomes are
//! byte-identical. And one p is moved across alpha: `conclusion_changes` is
//! true.
//!
//! Also the guard: `ReproductionOutcome` and `ReproductionView` carry no
//! field named score, percent, probability, confidence or grade.

mod sav_support;

use std::path::{Path, PathBuf};

use app_lib::stats_l1_ingest::load_dataset;
use app_lib::stats_reproduction::{reproduction_view, run_stats_reproduction_measured, ReproductionView};
use gaply_core::extract::extract_from_text;
use gaply_core::stats_l0::input::DocumentContext;
use gaply_core::stats_l0::result::{read_results, TableStructure};
use gaply_core::stats_l1::binding::{BindingDraft, OlsTarget, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, MethodCatalogue, RoleName, VariantChoice};
use gaply_core::stats_l1::claim::{claims_from_reading, ClaimId, RevisionHash};
use gaply_core::stats_l1::compute::ComputedVariant;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_l1::reproduce::{L1Note, L1Verdict, ReproductionOutcome};
use gaply_core::stats_l1::run_state::L1RunState;
use gaply_core::stats_l1::token::ConfirmationToken;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/gaply-core/tests/fixtures/stats_l1");

/// A fixture file without its '#' provenance lines.
fn fixture(name: &str) -> String {
    let text = std::fs::read_to_string(format!("{FIXTURES}/{name}")).expect("fixture");
    assert!(text.lines().any(|l| l.starts_with('#') && l.contains("/usr/local/bin/Rscript")), "{name} names its generator");
    text.lines().filter(|l| !l.starts_with('#')).collect::<Vec<_>>().join("\n") + "\n"
}

/// The CSV as the user's data file: a file of each run's own. The tests run
/// in parallel, and a shared file rewritten during another run's read is
/// refused by the ingest as `FileChangedWhileReading` (measured: it was).
fn data_file() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static RUN: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("gaply_l1_exit_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("exit_data_{}.csv", RUN.fetch_add(1, Ordering::SeqCst)));
    std::fs::write(&path, fixture("exit_data.csv")).expect("write csv");
    path
}

/// Columns, 1-based: y_t g2 pre post y_a g3 x_r y_r a2 b2 x1 x2 y_o.
fn binding(i: usize) -> BindingDraft {
    let c = |n: u32| ColumnAlias(n);
    let draft = |m: L1MethodId| BindingDraft::new(m, VariantChoice::not_stated(MethodCatalogue::entry(m).variant_kind));
    match i {
        0 => draft(L1MethodId::IndependentT).role(RoleName::Outcome, &[c(1)]).role(RoleName::Group, &[c(2)]),
        1 => draft(L1MethodId::PairedT).role(RoleName::First, &[c(3)]).role(RoleName::Second, &[c(4)]),
        2 => draft(L1MethodId::OneWayAnova).role(RoleName::Outcome, &[c(5)]).role(RoleName::Group, &[c(6)]),
        3 => draft(L1MethodId::Pearson).role(RoleName::X, &[c(7)]).role(RoleName::Y, &[c(8)]),
        4 => draft(L1MethodId::ChiSquareIndependence).role(RoleName::RowVariable, &[c(9)]).role(RoleName::ColumnVariable, &[c(10)]),
        5 => draft(L1MethodId::Ols).role(RoleName::Outcome, &[c(13)]).role(RoleName::Predictors, &[c(11), c(12)]).target(OlsTarget::OverallF),
        _ => unreachable!(),
    }
}

/// The whole path for `manuscript`, with every claim bound and confirmed.
fn run(manuscript: &str) -> ReproductionOutcome {
    run_reading(manuscript, None)
}

/// [`run`], with the reproduction reading `read` again instead of the CSV
/// when one is given.
fn run_reading(manuscript: &str, read: Option<&Path>) -> ReproductionOutcome {
    let path = data_file();
    run_with(manuscript, &path, read.unwrap_or(&path))
}

/// [`run`], with the claims bound on `load` and the reproduction reading
/// `read` again.
fn run_with(manuscript: &str, load: &Path, read: &Path) -> ReproductionOutcome {
    let ex = extract_from_text(manuscript);
    let reading = read_results(&ex, TableStructure::NotAvailable);
    assert_eq!(reading.results.len(), 6, "Level 0 reads all six results: {:?}", reading.not_read.iter().map(|n| n.reason).collect::<Vec<_>>());
    let context = DocumentContext::read(&ex, &reading);
    let (claims, not_located) = claims_from_reading(&ex, &reading);
    assert_eq!(not_located, 0);

    let mut s = L1RunState::new();
    s.load_dataset(load_dataset(load).expect("the data file loads"));
    s.set_claims(claims);
    s.set_context(context);
    let mut pairs: Vec<(RevisionHash, ConfirmationToken)> = Vec::new();
    for i in 0..6 {
        let id = s.add_proposal(ClaimId(i), binding(i), ProposalSource::Manual { edited_from: None }).expect("proposal");
        let (_, token) = s.confirm(id).unwrap_or_else(|e| panic!("claim {i}: {e:?}"));
        pairs.push((s.claim(ClaimId(i)).unwrap().revision.clone(), token));
    }
    run_stats_reproduction_measured(&s, read, &pairs)
}

/// Research OS, Phase 8F-2 (decision 291): a `.sav` ambers 0.4.4 panicked on,
/// read again by the reproduction, gives every claim could not check, and no
/// panic: no columns; since 8F-4 (decision 306) the big-endian file
/// `SavBigEndian`, and the two little-endian files with variables, which now
/// read, `FingerprintChanged`, since they are not the CSV the claims were
/// bound on. Until 8F-4 those three were the ingest's `Malformed`.
#[test]
fn a_sav_that_crashed_ambers_gives_could_not_check_with_no_panic() {
    use gaply_core::stats_l1::compute::L1Refusal;
    use gaply_core::stats_l1::dataset::DatasetRefusal;
    use gaply_core::stats_l1::dataset::L1IngestRefusal;
    use gaply_core::stats_l1::reproduce::L1CouldNotCheck;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let dir = std::env::temp_dir().join(format!("gaply_8f2_sav_repro_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut wrong = Vec::new();
    for (name, bytes) in sav_support::crashing() {
        let p = dir.join(format!("{name}.sav"));
        std::fs::write(&p, &bytes).unwrap();
        let want = match name {
            "no_variables" => {
                L1CouldNotCheck::DataUnreadable(L1IngestRefusal::Dataset(DatasetRefusal::NoColumns))
            }
            "case_size_minus_1_big_endian" => {
                L1CouldNotCheck::DataUnreadable(L1IngestRefusal::SavBigEndian)
            }
            _ => L1CouldNotCheck::Refused(L1Refusal::FingerprintChanged),
        };
        match catch_unwind(AssertUnwindSafe(|| {
            run_reading(&fixture("exit_manuscript.txt"), Some(&p))
        })) {
            Err(_) => wrong.push(format!("{name}: panicked")),
            Ok(out) => {
                if out.claims.len() != 6
                    || !out
                        .claims
                        .iter()
                        .all(|c| c.verdict == L1Verdict::CouldNotCheck(want.clone()))
                {
                    wrong.push(format!(
                        "{name}: {:?}",
                        out.claims.iter().map(|c| &c.verdict).collect::<Vec<_>>()
                    ));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Research OS, Phase 8F-3 (decision 295): a big-endian `.sav` read again by
/// the reproduction gives every claim could not check, its refusal
/// `SavBigEndian`, uncompressed or compressed.
#[test]
fn a_big_endian_sav_gives_could_not_check_sav_big_endian() {
    use gaply_core::stats_l1::dataset::L1IngestRefusal;
    use gaply_core::stats_l1::reproduce::L1CouldNotCheck;
    let dir = std::env::temp_dir().join(format!("gaply_8f3_sav_repro_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut wrong = Vec::new();
    for (name, bytes, _) in sav_support::same_values(sav_support::Order::Big) {
        let p = dir.join(format!("{name}.sav"));
        std::fs::write(&p, &bytes).unwrap();
        let out = run_reading(&fixture("exit_manuscript.txt"), Some(&p));
        let want = L1Verdict::CouldNotCheck(L1CouldNotCheck::DataUnreadable(
            L1IngestRefusal::SavBigEndian,
        ));
        if out.claims.len() != 6 || !out.claims.iter().all(|c| c.verdict == want) {
            wrong.push(format!(
                "{name}: {:?}",
                out.claims.iter().map(|c| &c.verdict).collect::<Vec<_>>()
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn changed(from: &str, to: &str) -> String {
    let m = fixture("exit_manuscript.txt");
    assert_eq!(m.matches(from).count(), 1, "{from:?} occurs once in the fixture");
    m.replace(from, to)
}

fn kind(v: &L1Verdict) -> &'static str {
    match v {
        L1Verdict::Reproduced(_) => "Reproduced",
        L1Verdict::NotReproduced { .. } => "NotReproduced",
        L1Verdict::CouldNotCheck(_) => "CouldNotCheck",
    }
}

fn claim_json(view: &ReproductionView, i: usize) -> String {
    serde_json::to_string(&view.claims[i]).expect("serialises")
}

#[test]
fn the_exit_test_all_six_reproduce_one_changed_statistic_is_the_only_change() {
    let base = run(&fixture("exit_manuscript.txt"));
    for c in &base.claims {
        let notes = match &c.verdict {
            L1Verdict::Reproduced(n) => format!("{n:?}"),
            other => format!("{other:?}"),
        };
        println!("claim {} {:<24} {:<14} {notes}", c.claim.0, c.method.map(|m| m.id()).unwrap_or("-"), kind(&c.verdict));
    }
    assert!(base.claims.iter().all(|c| matches!(c.verdict, L1Verdict::Reproduced(_))), "{:#?}", base.claims.iter().map(|c| &c.verdict).collect::<Vec<_>>());
    assert_eq!((base.counts.reproduced, base.counts.not_reproduced, base.counts.could_not_check), (6, 0, 0));
    // R printed Student's t (var.equal = TRUE), and the outcome names it.
    let L1Verdict::Reproduced(notes) = &base.claims[0].verdict else { unreachable!() };
    assert!(notes.contains(&L1Note::VariantMatched(ComputedVariant::StudentPooled)), "{notes:?}");
    assert!(notes.contains(&L1Note::SignComparedInMagnitude), "{notes:?}");
    // R printed the uncorrected chi-square, which is a candidate when no
    // variant is stated.
    let L1Verdict::Reproduced(notes) = &base.claims[4].verdict else { unreachable!() };
    assert!(notes.contains(&L1Note::VariantMatched(ComputedVariant::ChiSquareUncorrected)), "{notes:?}");
    let base_view = reproduction_view(&base);

    // The ANOVA's F, 7.37 -> 7.57: beyond [7.365, 7.375].
    let moved = run(&changed("F(2, 37) = 7.37", "F(2, 37) = 7.57"));
    assert!(matches!(moved.claims[2].verdict, L1Verdict::NotReproduced { .. }), "{:?}", moved.claims[2].verdict);
    let view = reproduction_view(&moved);
    for i in [0, 1, 3, 4, 5] {
        assert_eq!(format!("{:?}", moved.claims[i]), format!("{:?}", base.claims[i]), "claim {i}'s outcome changed");
        assert_eq!(claim_json(&view, i), claim_json(&base_view, i), "claim {i}'s view changed");
    }
    assert_eq!((moved.counts.reproduced, moved.counts.not_reproduced), (5, 1));
}

#[test]
fn the_exit_test_a_p_moved_across_alpha_changes_the_conclusion() {
    // The paired t's p, .025 -> .250: the exact p is below .05 and the printed
    // one now above it.
    let out = run(&changed("t(39) = -2.33, p = .025", "t(39) = -2.33, p = .250"));
    assert_eq!(out.claims[1].verdict, L1Verdict::NotReproduced { conclusion_changes: Some(true) });
    assert!(out.claims.iter().enumerate().all(|(i, c)| i == 1 || matches!(c.verdict, L1Verdict::Reproduced(_))));
}

/* ------------------------------------------------------------------------ *
 *  Guard: no score, percent, probability, confidence or grade.
 * ------------------------------------------------------------------------ */

const FORBIDDEN: [&str; 5] = ["score", "percent", "probability", "confidence", "grade"];

fn keys(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, v) in m {
                out.push(k.clone());
                keys(v, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
        _ => {}
    }
}

/// Identifiers and string literals in `src`, with `//` comments removed.
fn code_words(src: &str) -> Vec<String> {
    let mut words = Vec::new();
    for line in src.lines() {
        let code = line.split("//").next().unwrap_or("");
        words.extend(code.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(str::to_lowercase));
    }
    words
}

#[test]
fn neither_the_outcome_nor_its_view_has_a_score_percent_probability_confidence_or_grade() {
    // The view, every key, over runs that reach all three verdict kinds.
    let mut all = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    // The third run prints a fractional df on the ANOVA: CouldNotCheck.
    for manuscript in [fixture("exit_manuscript.txt"), changed("F(2, 37) = 7.37", "F(2, 37) = 7.57"), changed("F(2, 37) = 7.37", "F(2, 37.5) = 7.37")] {
        let out = run(&manuscript);
        seen.extend(out.claims.iter().map(|c| kind(&c.verdict)));
        let json = serde_json::to_value(reproduction_view(&out)).expect("serialises");
        keys(&json, &mut all);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec!["CouldNotCheck", "NotReproduced", "Reproduced"], "every verdict kind reached the view");
    assert!(all.len() > 100, "only {} keys: the walk would pass vacuously", all.len());
    for k in &all {
        let lower = k.to_lowercase();
        assert!(!FORBIDDEN.iter().any(|f| lower.contains(f)), "the view has a key {k:?}");
    }
    // The types themselves, in the core module and the app view module.
    let files = [
        concat!(env!("CARGO_MANIFEST_DIR"), "/gaply-core/src/stats_l1/reproduce.rs"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/stats_reproduction.rs"),
    ];
    for f in files {
        let src = std::fs::read_to_string(f).expect("source");
        let src = src.split("#[cfg(test)]").next().unwrap_or(&src);
        let words = code_words(src);
        assert!(words.len() > 500, "{f}: only {} words scanned", words.len());
        for w in &words {
            assert!(!FORBIDDEN.iter().any(|x| w.contains(x)), "{f}: an identifier or literal {w:?}");
        }
    }
}

/* ------------------------------------------------------------------------ *
 *  Step 7: the view carries the shared closed codes, never Debug names.
 * ------------------------------------------------------------------------ */

fn strings_and_codes(v: &serde_json::Value, strings: &mut Vec<String>, codes: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                if k == "code" {
                    codes.push(x.as_str().expect("a code is a string").to_string());
                }
                strings_and_codes(x, strings, codes);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| strings_and_codes(x, strings, codes)),
        serde_json::Value::String(s) => strings.push(s.clone()),
        _ => {}
    }
}

#[test]
fn the_view_carries_closed_codes_and_provenance_without_the_p_outcome() {
    let mut strings = Vec::new();
    let mut codes = Vec::new();
    let mut provenance = 0;
    for manuscript in [fixture("exit_manuscript.txt"), changed("F(2, 37) = 7.37", "F(2, 37) = 7.57"), changed("F(2, 37) = 7.37", "F(2, 37.5) = 7.37")] {
        let json = serde_json::to_value(reproduction_view(&run(&manuscript))).expect("serialises");
        strings_and_codes(&json, &mut strings, &mut codes);
        for c in json["claims"].as_array().unwrap() {
            if let Some(p) = c["provenance"].as_object() {
                provenance += 1;
                assert!(!p.contains_key("outcome") && !p.contains_key("computed"), "{p:?}");
                assert!(p["bindingSource"]["code"].is_string() && p["variantSetBy"]["code"].is_string(), "{p:?}");
            }
        }
    }
    // A positive count first: the walk saw codes, and computed claims.
    assert!(codes.len() > 50, "only {} codes seen", codes.len());
    assert!(provenance >= 12, "only {provenance} claims carried provenance");
    for c in &codes {
        assert!(c.chars().all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '_'), "a code {c:?}");
    }
    for want in ["variant_matched", "student_pooled", "matched", "not_matched", "statistic"] {
        assert!(codes.iter().any(|c| c == want), "{want} never reached the view");
    }
    // A Debug name is CamelCase; no string in the view is.
    for s in &strings {
        let camel = s.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && s.chars().skip(1).any(|c| c.is_ascii_uppercase()) && !s.contains(' ');
        assert!(!camel, "a Debug-shaped string in the view: {s:?}");
    }
}

/// Research OS, Phase 8F-4 (decisions 306 and 307): the exit test's CSV as a
/// `.sav` (its two text columns as strings of width 8), in the shapes ambers
/// 0.4.6 reads rightly: a header of 0 cases with the rows behind it, a case
/// size larger than the variable list, and a case size of 0 and of -1,
/// uncompressed and bytecode. Bound and read again, each reproduces all six
/// claims, with the verdicts the CSV gives.
fn exit_sav(f: impl Fn(&mut sav_support::Spec)) -> Vec<u8> {
    use sav_support::{Cell, Spec, Var};
    let csv = fixture("exit_data.csv");
    let mut lines = csv.lines();
    let headers: Vec<&str> = lines.next().unwrap().split(',').collect();
    let rows: Vec<Vec<&str>> = lines
        .filter(|l| !l.is_empty())
        .map(|l| l.split(',').collect())
        .collect();
    let text = |i: usize| rows.iter().any(|r| r[i].parse::<f64>().is_err());
    let vars = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if text(i) {
                Var::text(h.as_bytes())
            } else {
                Var::number(h.as_bytes())
            }
        })
        .collect();
    let cells = rows
        .iter()
        .map(|r| {
            r.iter()
                .enumerate()
                .map(|(i, c)| {
                    if text(i) {
                        Cell::Text(c.as_bytes().to_vec())
                    } else {
                        Cell::Number(c.parse().unwrap())
                    }
                })
                .collect()
        })
        .collect();
    let mut s = Spec::new(vars, cells);
    f(&mut s);
    s.bytes()
}

fn reproduces_as_the_csv(files: Vec<(&str, Vec<u8>)>) {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let csv = run(&fixture("exit_manuscript.txt"));
    assert!(
        csv.claims.iter().all(|c| kind(&c.verdict) == "Reproduced"),
        "the CSV reproduces all six"
    );
    // A folder of each test's own: two tests share this, in parallel.
    let dir = std::env::temp_dir().join(format!(
        "gaply_8f4_sav_repro_{}_{}",
        std::process::id(),
        files[0].0
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut wrong = Vec::new();
    for (name, bytes) in files {
        let p = dir.join(format!("{name}.sav"));
        std::fs::write(&p, &bytes).unwrap();
        match catch_unwind(AssertUnwindSafe(|| {
            run_with(&fixture("exit_manuscript.txt"), &p, &p)
        })) {
            Err(_) => wrong.push(format!("{name}: panicked")),
            Ok(out) => {
                let got: Vec<_> = out.claims.iter().map(|c| &c.verdict).collect();
                let want: Vec<_> = csv.claims.iter().map(|c| &c.verdict).collect();
                if got != want {
                    wrong.push(format!("{name}: {got:?}"));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_exit_data_as_a_sav_ambers_0_4_6_reads_rightly_reproduces_as_the_csv() {
    let mut files = vec![
        ("proper", exit_sav(|_| {})),
        ("zero_cases_uncompressed", exit_sav(|s| s.cases = Some(0))),
        (
            "zero_cases_bytecode",
            exit_sav(|s| {
                s.cases = Some(0);
                s.compression = 1
            }),
        ),
        (
            "case_size_larger_uncompressed",
            exit_sav(|s| s.case_size = Some(14)),
        ),
        (
            "case_size_larger_bytecode",
            exit_sav(|s| {
                s.case_size = Some(14);
                s.compression = 1
            }),
        ),
    ];
    for (name, cs, compression) in [
        ("case_size_0", 0, 0),
        ("case_size_0_bytecode", 0, 1),
        ("case_size_minus_1", -1, 0),
        ("case_size_minus_1_bytecode", -1, 1),
    ] {
        files.push((
            name,
            exit_sav(|s| {
                s.case_size = Some(cs);
                s.compression = compression
            }),
        ));
    }
    reproduces_as_the_csv(files);
}

/// The smaller case size, never read under ambers 0.4.4.
#[test]
fn the_exit_data_as_a_sav_with_a_case_size_smaller_than_its_variables_reproduces_as_the_csv() {
    sav_support::require_ambers_0_4_5_or_later("case_size_smaller");
    reproduces_as_the_csv(vec![
        (
            "case_size_smaller_uncompressed",
            exit_sav(|s| s.case_size = Some(12)),
        ),
        (
            "case_size_smaller_bytecode",
            exit_sav(|s| {
                s.case_size = Some(12);
                s.compression = 1
            }),
        ),
    ]);
}

/// Decision 307's files ambers 0.4.6 refuses, read again by the
/// reproduction: every claim could not check, `DataUnreadable(Malformed)`.
fn reproduction_refuses(names: &[&str]) {
    use gaply_core::stats_l1::dataset::L1IngestRefusal;
    use gaply_core::stats_l1::reproduce::L1CouldNotCheck;
    let dir = std::env::temp_dir().join(format!(
        "gaply_8f4_sav_refused_{}_{}",
        std::process::id(),
        names[0]
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let all = sav_support::fixes();
    let mut wrong = Vec::new();
    for name in names {
        let (_, bytes, _, safe) = all.iter().find(|f| f.0 == *name).unwrap();
        if !safe {
            sav_support::require_ambers_0_4_5_or_later(name);
        }
        let p = dir.join(format!("{name}.sav"));
        std::fs::write(&p, bytes).unwrap();
        let out = run_reading(&fixture("exit_manuscript.txt"), Some(&p));
        let want =
            L1Verdict::CouldNotCheck(L1CouldNotCheck::DataUnreadable(L1IngestRefusal::Malformed));
        if out.claims.len() != 6 || !out.claims.iter().all(|c| c.verdict == want) {
            wrong.push(format!(
                "{name}: {:?}",
                out.claims.iter().map(|c| &c.verdict).collect::<Vec<_>>()
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_sav_ambers_0_4_6_refuses_gives_could_not_check_malformed() {
    reproduction_refuses(&["duplicate_names", "type_field_256"]);
}

/// The oversized declarations, never read under ambers 0.4.4.
#[test]
fn a_sav_declaring_sizes_beyond_itself_gives_could_not_check_malformed() {
    reproduction_refuses(&["label_count_oversized", "info_record_oversized"]);
}
