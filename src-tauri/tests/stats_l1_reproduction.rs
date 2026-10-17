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

use std::path::PathBuf;

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
    let ex = extract_from_text(manuscript);
    let reading = read_results(&ex, TableStructure::NotAvailable);
    assert_eq!(reading.results.len(), 6, "Level 0 reads all six results: {:?}", reading.not_read.iter().map(|n| n.reason).collect::<Vec<_>>());
    let context = DocumentContext::read(&ex, &reading);
    let (claims, not_located) = claims_from_reading(&ex, &reading);
    assert_eq!(not_located, 0);

    let path = data_file();
    let mut s = L1RunState::new();
    s.load_dataset(load_dataset(&path).expect("the CSV loads"));
    s.set_claims(claims);
    s.set_context(context);
    let mut pairs: Vec<(RevisionHash, ConfirmationToken)> = Vec::new();
    for i in 0..6 {
        let id = s.add_proposal(ClaimId(i), binding(i), ProposalSource::Manual { edited_from: None }).expect("proposal");
        let (_, token) = s.confirm(id).unwrap_or_else(|e| panic!("claim {i}: {e:?}"));
        pairs.push((s.claim(ClaimId(i)).unwrap().revision.clone(), token));
    }
    run_stats_reproduction_measured(&s, &path, &pairs)
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
