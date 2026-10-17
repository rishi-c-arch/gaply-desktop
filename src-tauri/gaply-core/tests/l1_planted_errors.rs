//! **The exit test, per method (Phase 2, Step 8): one planted error in one
//! printed number moves that claim alone to NotReproduced.**
//!
//! Every Level 1 method, over R's own numbers (`golden.csv`, written by
//! `/usr/local/bin/Rscript make_l1_fixtures.R`). One run per data file holds
//! every golden claim on that file, printed APA style, each bound and
//! confirmed. All of them reproduce. Then, one run at a time, ONE printed
//! number of ONE claim is moved well outside its rounding interval: the
//! statistic, each df, the p, and the n. That claim must be NotReproduced and
//! every other claim's outcome must be byte-identical (`Debug`) to the run
//! without the error.
//!
//! The app crate's `stats_l1_reproduction.rs` runs the same idea through the
//! file seam on the six-result manuscript; this covers the seventh method
//! (Spearman), the OLS coefficient target and the n, which that manuscript
//! does not print. Run with `--nocapture` to see the table.
//!
//! Since Step 8b (rule 1, data precision), a planted number that the stored
//! cells' rounding could still explain is not checked rather than not
//! reproduced; such cases are pinned in `EXPECTED_WITHIN_PRECISION` (only
//! Spearman's, whose 1-decimal data could reorder under rounding). One case
//! planted just inside a precision range and one just beyond it are tested
//! on their own.

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, OlsTarget, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, MethodCatalogue, RoleName, SpearmanPVariant, Variant, VariantChoice};
use gaply_core::stats_l1::claim::{ClaimLocation, RevisionHash, UserClaim, UserP};
use gaply_core::stats_l1::compute::L1Registry;
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_l1::reproduce::{reproduce, L1CouldNotCheck, L1Verdict, NotCheckedReason, NumberKind, NumberStatus, ReproductionOutcome};
use gaply_core::stats_l1::run_state::L1RunState;
use gaply_core::stats_l1::token::ConfirmationToken;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_l1");

/// One golden row, as printed in a paper.
#[derive(Clone)]
struct Printed {
    case: String,
    method: L1MethodId,
    family: Family,
    roles: Vec<(RoleName, Vec<String>)>,
    target: Option<OlsTarget>,
    variant: VariantChoice,
    statistic: String,
    df: Vec<String>,
    p: (String, Relation),
    n: String,
}

fn golden_rows(file: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(format!("{DIR}/golden.csv")).expect("golden.csv");
    assert!(text.lines().any(|l| l.starts_with('#') && l.contains("/usr/local/bin/Rscript")), "golden.csv names its generator");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("case,"))
        .map(|l| l.split(',').map(str::to_string).collect::<Vec<_>>())
        .filter(|f| f[1] == file)
        .collect()
}

fn data(name: &str) -> (Vec<u8>, RawTable) {
    let bytes = std::fs::read(format!("{DIR}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().expect("header").split(',').map(str::to_string).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    (bytes, RawTable::from_rows(headers, rows))
}

fn dataset(bytes: &[u8], raw: RawTable) -> Dataset {
    build_dataset(raw, DataFingerprint::of_bytes(bytes, "golden csv", TableRef::FirstTable)).expect("dataset")
}

fn role(s: &str) -> RoleName {
    match s {
        "outcome" => RoleName::Outcome,
        "group" => RoleName::Group,
        "first" => RoleName::First,
        "second" => RoleName::Second,
        "x" => RoleName::X,
        "y" => RoleName::Y,
        "row" => RoleName::RowVariable,
        "column" => RoleName::ColumnVariable,
        "predictors" => RoleName::Predictors,
        other => panic!("unknown role {other}"),
    }
}

/// APA: two decimals; r and rho without a leading zero.
fn print_statistic(v: f64, family: Family) -> String {
    let s = format!("{v:.2}");
    if family == Family::R {
        s.replacen("0.", ".", 1)
    } else {
        s
    }
}

fn print_df(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

/// APA: three decimals without a leading zero; `< .001` below it.
fn print_p(p: f64) -> (String, Relation) {
    if p < 0.001 {
        (".001".into(), Relation::Lt)
    } else {
        (format!("{p:.3}").trim_start_matches('0').to_string(), Relation::Eq)
    }
}

fn printed(file: &str, headers: &[String]) -> Vec<Printed> {
    let mut out = Vec::new();
    for f in golden_rows(file) {
        let (case, method_id, roles, variant) = (&f[0], &f[2], &f[3], &f[4]);
        // The intercept is not a target a binding can name.
        if variant == "ols_coef:(Intercept)" {
            continue;
        }
        let method = L1MethodId::from_id(method_id).expect("method");
        let num = |s: &str| s.parse::<f64>().unwrap_or_else(|_| panic!("{case}: {s}"));
        let family = match variant.as_str() {
            "student" | "welch" | "paired" => Family::T,
            "oneway" | "ols_f" => Family::F,
            "pearson" | "spearman_t" => Family::R,
            "chi_uncorrected" | "chi_yates_r" => Family::ChiSquare,
            v if v.starts_with("ols_coef:") => Family::T,
            other => panic!("{case}: variant {other}"),
        };
        let target = match variant.as_str() {
            "ols_f" => Some(OlsTarget::OverallF),
            v => v.strip_prefix("ols_coef:").map(|col| OlsTarget::Coefficient {
                predictor: ColumnAlias(headers.iter().position(|h| h == col).expect("predictor") as u32 + 1),
            }),
        };
        // Spearman's t approximation, ticked as stated (R's exact = FALSE),
        // so its p is checked rather than left to the exact p Gaply lacks.
        let choice = if variant == "spearman_t" {
            VariantChoice::user_ticked(Variant::SpearmanP(SpearmanPVariant::TApproximation))
        } else {
            VariantChoice::not_stated(MethodCatalogue::entry(method).variant_kind)
        };
        let mut df = vec![print_df(num(&f[7]))];
        if f[8] != "NA" {
            df.push(print_df(num(&f[8])));
        }
        out.push(Printed {
            case: case.clone(),
            method,
            family,
            roles: roles
                .split(';')
                .map(|pair| {
                    let (r, cols) = pair.split_once('=').expect("role=cols");
                    (role(r), cols.split('|').map(str::to_string).collect())
                })
                .collect(),
            target,
            variant: choice,
            statistic: print_statistic(num(&f[6]), family),
            df,
            p: print_p(num(&f[9])),
            n: f[10].clone(),
        });
    }
    out
}

/// Every claim bound and confirmed, then reproduced from the same bytes.
fn run(bytes: &[u8], raw: &RawTable, claims: &[Printed]) -> ReproductionOutcome {
    let headers = raw.headers.clone();
    let alias = |name: &str| ColumnAlias(headers.iter().position(|h| h == name).unwrap_or_else(|| panic!("no column {name}")) as u32 + 1);
    let mut s = L1RunState::new();
    s.load_dataset(dataset(bytes, raw.clone()));
    let mut pairs: Vec<(RevisionHash, ConfirmationToken)> = Vec::new();
    for (i, c) in claims.iter().enumerate() {
        let id = s
            .add_claim(&UserClaim {
                family: c.family,
                statistic: &c.statistic,
                df: c.df.iter().map(String::as_str).collect(),
                n: Some(&c.n),
                p: UserP::Reported { raw: &c.p.0, relation: c.p.1 },
                location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: i, sentence: 0 },
                sentence: None,
            })
            .unwrap_or_else(|e| panic!("{}: claim {e:?}", c.case));
        let mut draft = BindingDraft::new(c.method, c.variant);
        for (r, cols) in &c.roles {
            let cols: Vec<ColumnAlias> = cols.iter().map(|x| alias(x)).collect();
            draft = draft.role(*r, &cols);
        }
        if let Some(t) = c.target {
            draft = draft.target(t);
        }
        let pid = s.add_proposal(id, draft, ProposalSource::Manual { edited_from: None }).expect("proposal");
        let (_, token) = s.confirm(pid).unwrap_or_else(|e| panic!("{}: confirm {e:?}", c.case));
        pairs.push((s.claim(id).unwrap().revision.clone(), token));
    }
    reproduce(&s, &pairs, || Ok(dataset(bytes, raw.clone())))
}

/// A planted error: far outside the printed number's rounding interval.
fn plant(c: &Printed, field: &str) -> Printed {
    let mut c = c.clone();
    let shift = |s: &str, by: f64| -> f64 { s.parse::<f64>().unwrap() + by };
    match field {
        "statistic" => {
            let v: f64 = c.statistic.parse().unwrap();
            // r and rho stay inside [-1, 1].
            let moved = if c.family == Family::R { v - 0.3 * v.signum() } else { v * 1.5 + v.signum() };
            c.statistic = print_statistic(moved, c.family);
        }
        "df1" => c.df[0] = print_df(shift(&c.df[0], 3.0)),
        "df2" => c.df[1] = print_df(shift(&c.df[1], 3.0)),
        "p" => {
            c.p = match c.p.1 {
                Relation::Lt => (".300".into(), Relation::Eq),
                _ => {
                    let p: f64 = c.p.0.parse().unwrap();
                    print_p(if p < 0.5 { p + 0.3 } else { p - 0.3 })
                }
            }
        }
        "n" => c.n = (c.n.parse::<usize>().unwrap() + 5).to_string(),
        other => panic!("{other}"),
    }
    c
}

/// The data files whose golden rows cover every Level 1 method.
const FILES: [&str; 6] = ["data_t_ties.csv", "data_paired.csv", "data_anova3.csv", "data_cor_pos.csv", "data_chi_2x2.csv", "data_ols3.csv"];

#[test]
fn every_method_one_planted_error_moves_its_claim_alone_to_not_reproduced() {
    let mut methods_seen = std::collections::BTreeSet::new();
    let mut planted = 0usize;
    let mut within_precision: Vec<String> = Vec::new();
    println!("{:<28} {:<24} {:<10} verdict", "case", "method", "planted");
    for file in FILES {
        let (bytes, raw) = data(file);
        let claims = printed(file, &raw.headers);
        assert!(!claims.is_empty(), "{file}: no golden rows");
        let base = run(&bytes, &raw, &claims);
        for (c, out) in claims.iter().zip(&base.claims) {
            assert!(matches!(out.verdict, L1Verdict::Reproduced(_)), "{}: baseline {:?}", c.case, out.verdict);
            methods_seen.insert(c.method.id());
        }
        for (i, c) in claims.iter().enumerate() {
            let mut fields = vec!["statistic", "df1"];
            if c.df.len() == 2 {
                fields.push("df2");
            }
            fields.extend(["p", "n"]);
            for field in fields {
                let mut moved = claims.clone();
                moved[i] = plant(c, field);
                let out = run(&bytes, &raw, &moved);
                let verdict = &out.claims[i].verdict;
                println!("{:<28} {:<24} {:<10} {:?}", c.case, c.method.id(), field, verdict);
                // Step 8b, rule 1: a planted number that the stored cells'
                // rounding could still explain is not checked, never
                // NotReproduced. Such a case is listed and pinned below.
                let kind = match field {
                    "statistic" => NumberKind::Statistic,
                    "df1" => NumberKind::Df { index: 0 },
                    "df2" => NumberKind::Df { index: 1 },
                    "p" => NumberKind::P,
                    _ => NumberKind::Statistic,
                };
                let precision_rows = out.claims[i]
                    .candidates
                    .iter()
                    .flat_map(|cand| cand.numbers.iter())
                    .filter(|n| n.kind == kind && matches!(n.status, NumberStatus::NotChecked(NotCheckedReason::DataPrecision { .. })))
                    .count();
                if field != "n" && precision_rows > 0 {
                    let want_cnc = kind == NumberKind::Statistic;
                    assert!(
                        if want_cnc { matches!(verdict, L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { .. })) } else { matches!(verdict, L1Verdict::Reproduced(_)) },
                        "{} with its {field} planted inside the precision range: {verdict:?}",
                        c.case
                    );
                    within_precision.push(format!("{} {field}", c.case));
                } else {
                    assert!(matches!(verdict, L1Verdict::NotReproduced { .. }), "{} with its {field} planted: {verdict:?}", c.case);
                }
                for j in (0..claims.len()).filter(|j| *j != i) {
                    assert_eq!(
                        format!("{:?}", out.claims[j]),
                        format!("{:?}", base.claims[j]),
                        "{} planted in {}: {}'s outcome changed",
                        field,
                        c.case,
                        claims[j].case
                    );
                }
                planted += 1;
            }
        }
    }
    // Every registered method was planted, and enough cases ran.
    for m in L1Registry::METHODS {
        assert!(methods_seen.contains(m.method.id()), "{} was never planted", m.method.id());
    }
    assert!(planted >= 50, "only {planted} planted cases ran");
    println!("planted cases: {planted}; methods: {}", methods_seen.len());
    println!("inside the precision range (rule 1): {within_precision:?}");
    // Every case whose status Step 8b's rule 1 changed, pinned. Spearman's
    // golden data is stored to 1 decimal with gaps of 0.1 or less, so
    // rounding could reorder its ranks: its precision is unbounded and a
    // planted rho is not checked rather than not reproduced. A planted p
    // (.315 for rho .49, n 24; R's t approximation .0151, part C's band
    // reaching about .185) is ruled out by the printed rho itself, whatever
    // the data's rounding, so since part C's follow-up it is Not reproduced.
    assert_eq!(within_precision, EXPECTED_WITHIN_PRECISION, "the cases rule 1 moved");
}

/// Filled from the first run under rule 1; see the report for Step 8b.
const EXPECTED_WITHIN_PRECISION: &[&str] = &["cor_pos_spearman statistic"];

/// One planted statistic just inside the precision range and one just beyond
/// it, on the three-group ANOVA (`data_anova3.csv`, stored to 2 decimals):
/// printed to 4 decimals so the printed interval is narrower than the range.
#[test]
fn a_planted_statistic_just_inside_the_precision_range_is_could_not_check_and_just_beyond_is_not_reproduced() {
    let file = "data_anova3.csv";
    let (bytes, raw) = data(file);
    let claims = printed(file, &raw.headers);
    let i = claims.iter().position(|c| c.case == "anova_three").expect("anova_three");
    let base = run(&bytes, &raw, &claims);
    let stat = &base.claims[i].candidates[0].numbers[0];
    assert_eq!(stat.kind, NumberKind::Statistic);
    let (lo, hi) = stat.precision.expect("2-decimal cells give a precision range");
    let s = stat.computed;
    let b = hi - s;
    println!("anova_three: F = {s:.6}, precision range [{lo:.6}, {hi:.6}], b = {b:.6}");
    assert!(b > 0.001, "the range is wide enough to plant inside: b = {b}");
    for (printed, want_cnc) in [(s + b / 2.0, true), (s + b + 0.0005, false)] {
        let mut moved = claims.clone();
        moved[i].statistic = format!("{printed:.4}");
        let out = run(&bytes, &raw, &moved);
        let v = &out.claims[i].verdict;
        println!("  printed {:.4}: {v:?}", printed);
        if want_cnc {
            assert!(matches!(v, L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals: 2 })), "{v:?}");
        } else {
            assert!(matches!(v, L1Verdict::NotReproduced { .. }), "{v:?}");
        }
    }
}
