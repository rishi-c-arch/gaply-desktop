//! **Step 8b, rule 1: the precision range, checked against draws, and timed.**
//!
//! The range is a first-order worst case, `b = Σ |∂S/∂x_i| δ_i`. It is not a
//! proof: a draw can move a statistic further through curvature. So, on
//! every R golden row (`golden.csv`, written by `/usr/local/bin/Rscript
//! make_l1_fixtures.R`) and on section 4's finding 2 pair, 1,000 seeded draws
//! move every cell of every numeric role uniformly within ±δ of its column,
//! recompute through the confirmed-binding path, and record the largest
//! `|S_draw − S| / b`. A ratio above 1 means the range must be widened by
//! that factor; the test fails then and prints it.
//!
//! The timing test prints how long `compute` (which builds the range) takes
//! on the largest golden data file and on a synthetic 5,000-row file per
//! method:
//!
//! ```text
//! cargo test -p gaply_core --release --test l1_precision -- --nocapture --test-threads 1
//! ```

use std::collections::BTreeMap;
use std::time::Instant;

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, OlsTarget, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, MethodCatalogue, RoleName, VariantChoice};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::compute::{compute, ComputedResult, ComputedVariant, OlsTerm, Precision};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_l1::precision;
use gaply_core::stats_l1::run_state::L1RunState;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_l1");
const DRAWS: usize = 1000;

/* ---- a seeded generator ----------------------------------------------------- */

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform on [-1, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/* ---- the golden rows ---------------------------------------------------------- */

struct Golden {
    case: String,
    data: String,
    method: String,
    roles: String,
    variant: String,
}

fn golden() -> Vec<Golden> {
    let text = std::fs::read_to_string(format!("{DIR}/golden.csv")).expect("golden.csv");
    assert!(text.lines().any(|l| l.starts_with('#') && l.contains("/usr/local/bin/Rscript")), "golden.csv names its generator");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("case,"))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Golden { case: f[0].into(), data: f[1].into(), method: f[2].into(), roles: f[3].into(), variant: f[4].into() }
        })
        .collect()
}

fn data(name: &str) -> (Vec<u8>, RawTable) {
    let bytes = std::fs::read(format!("{DIR}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).unwrap();
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let rows = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    (bytes, RawTable::from_rows(headers, rows))
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
        other => panic!("{other}"),
    }
}

fn numeric_role(r: RoleName) -> bool {
    matches!(r, RoleName::Outcome | RoleName::First | RoleName::Second | RoleName::X | RoleName::Y | RoleName::Predictors)
}

fn alias(headers: &[String], name: &str) -> ColumnAlias {
    ColumnAlias(headers.iter().position(|h| h == name).unwrap_or_else(|| panic!("no column {name}")) as u32 + 1)
}

fn wanted(v: &str, headers: &[String]) -> ComputedVariant {
    match v {
        "student" => ComputedVariant::StudentPooled,
        "welch" => ComputedVariant::Welch,
        "paired" => ComputedVariant::Paired,
        "oneway" => ComputedVariant::OneWay,
        "pearson" => ComputedVariant::Pearson,
        "spearman_t" => ComputedVariant::SpearmanTApproximation,
        "chi_uncorrected" => ComputedVariant::ChiSquareUncorrected,
        "chi_yates_r" => ComputedVariant::ChiSquareYatesR,
        "ols_f" => ComputedVariant::OlsOverallF,
        "ols_coef:(Intercept)" => ComputedVariant::OlsCoefficient(OlsTerm::Intercept),
        other => ComputedVariant::OlsCoefficient(OlsTerm::Predictor(alias(headers, other.strip_prefix("ols_coef:").unwrap()))),
    }
}

fn dataset(bytes: &[u8], raw: RawTable) -> Dataset {
    build_dataset(raw, DataFingerprint::of_bytes(bytes, "precision test", TableRef::FirstTable)).expect("dataset")
}

/// Every result for `draft` on `raw`, through claim, confirmation, token and
/// compute.
fn results(bytes: &[u8], raw: &RawTable, draft: BindingDraft) -> Vec<ComputedResult> {
    let mut s = L1RunState::new();
    s.load_dataset(dataset(bytes, raw.clone()));
    let claim = s
        .add_claim(&UserClaim {
            family: Family::T,
            statistic: "1.00",
            df: vec!["10"],
            n: None,
            p: UserP::Reported { raw: ".05", relation: Relation::Eq },
            location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
            sentence: None,
        })
        .unwrap();
    let pid = s.add_proposal(claim, draft, ProposalSource::Manual { edited_from: None }).unwrap();
    let token = s.confirm(pid).expect("confirmed").1;
    compute(&s, &token, claim, &dataset(bytes, raw.clone())).expect("computed").results
}

fn draft_of(g: &Golden, headers: &[String]) -> (BindingDraft, Vec<usize>) {
    let method = L1MethodId::from_id(&g.method).unwrap();
    let mut d = BindingDraft::new(method, VariantChoice::not_stated(MethodCatalogue::entry(method).variant_kind));
    let mut numeric_cols = Vec::new();
    for pair in g.roles.split(';') {
        let (r, cols) = pair.split_once('=').unwrap();
        let cols: Vec<ColumnAlias> = cols.split('|').map(|c| alias(headers, c)).collect();
        if numeric_role(role(r)) {
            numeric_cols.extend(cols.iter().map(|c| c.0 as usize - 1));
        }
        d = d.role(role(r), &cols);
    }
    if method == L1MethodId::Ols {
        d = d.target(OlsTarget::OverallF);
    }
    (d, numeric_cols)
}

/// The column's stored uncertainty, as the engine reads it.
fn delta_of(raw: &RawTable, col: usize) -> f64 {
    precision::half_unit(precision::column_decimals(raw.rows.iter().filter_map(|r| r.get(col)?.trim().parse::<f64>().ok())))
}

/// `raw` with every cell of `cols` moved uniformly within ± its δ.
fn jittered(raw: &RawTable, cols: &[(usize, f64)], rng: &mut SplitMix) -> RawTable {
    let mut out = raw.clone();
    for row in &mut out.rows {
        for &(c, delta) in cols {
            if let Some(v) = row.get(c).and_then(|s| s.trim().parse::<f64>().ok()) {
                row[c] = format!("{}", v + rng.unit() * delta);
            }
        }
    }
    out
}

#[derive(Default)]
struct Worst {
    stat: f64,
    df: f64,
    p_outside: usize,
    draws: usize,
}

/// Draws against one result's range; returns the worst ratios.
fn check(name: &str, base: &ComputedResult, draws: impl Iterator<Item = ComputedResult>, worst: &mut Worst) {
    match base.precision {
        Precision::Exact => {
            for d in draws {
                worst.draws += 1;
                // Whole-number columns are not moved, and Spearman ranks that
                // cannot reorder do not change: the value is the value.
                assert!((d.value - base.value).abs() <= 1e-9 * base.value.abs().max(1.0), "{name}: exact, yet a draw moved it");
            }
        }
        Precision::Unbounded { .. } => {}
        Precision::Range { statistic: (lo, hi), df, p: (plo, phi), .. } => {
            // From the value itself: r's range is clipped to [-1, 1].
            let s = base.value;
            let b = (hi - s).max(s - lo);
            for d in draws {
                worst.draws += 1;
                if b > 0.0 {
                    worst.stat = worst.stat.max((d.value - s).abs() / b);
                }
                if let Some((dlo, dhi)) = df {
                    let bdf = (dhi - dlo) / 2.0;
                    if bdf > 0.0 {
                        worst.df = worst.df.max((d.df1 - base.df1).abs() / bdf);
                    }
                }
                let slack = 1e-12;
                if d.p < plo - slack || d.p > phi + slack {
                    worst.p_outside += 1;
                }
            }
        }
    }
}

#[test]
fn the_precision_range_contains_1000_seeded_draws_on_every_golden_row_and_finding_2() {
    let rows = golden();
    assert!(rows.len() >= 43, "only {} golden rows", rows.len());
    let mut per_method: BTreeMap<String, Worst> = BTreeMap::new();
    let mut ranged = 0;
    for (k, g) in rows.iter().enumerate() {
        let (bytes, raw) = data(&g.data);
        let (draft, cols) = draft_of(g, &raw.headers);
        let want = wanted(&g.variant, &raw.headers);
        let base = *results(&bytes, &raw, draft.clone()).iter().find(|r| r.variant == want).unwrap();
        if matches!(base.precision, Precision::Range { .. }) {
            ranged += 1;
        }
        let deltas: Vec<(usize, f64)> = cols.iter().map(|&c| (c, delta_of(&raw, c))).collect();
        let mut rng = SplitMix(20_261_015 + k as u64);
        let draws = (0..DRAWS).map(|_| {
            let j = jittered(&raw, &deltas, &mut rng);
            *results(&bytes, &j, draft.clone()).iter().find(|r| r.variant == want).unwrap()
        });
        check(&g.case, &base, draws, per_method.entry(g.method.clone()).or_default());
    }

    // Finding 2: twelve pairs stored to 2 decimals (section 4).
    let pre = ["0.8", "1.16", "1.28", "1.21", "1.09", "1.04", "1.04", "1.14", "0.96", "1.07", "1.14", "1.0"];
    let post = ["0.94", "1.28", "1.57", "1.3", "1.08", "1.04", "1.09", "1.3", "0.97", "1.17", "1.41", "0.74"];
    let raw = RawTable::from_rows(vec!["pre".into(), "post".into()], pre.iter().zip(post).map(|(a, b)| vec![a.to_string(), b.to_string()]).collect());
    let draft = BindingDraft::new(L1MethodId::PairedT, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::PairedT).variant_kind))
        .role(RoleName::First, &[ColumnAlias(1)])
        .role(RoleName::Second, &[ColumnAlias(2)]);
    let base = results(b"finding 2", &raw, draft.clone())[0];
    assert!(matches!(base.precision, Precision::Range { decimals: 2, .. }), "{:?}", base.precision);
    let mut rng = SplitMix(2);
    let deltas = [(0, 0.005), (1, 0.005)];
    let draws = (0..DRAWS).map(|_| results(b"finding 2", &jittered(&raw, &deltas, &mut rng), draft.clone())[0]);
    check("finding 2", &base, draws, per_method.entry("finding_2_paired".into()).or_default());

    println!("method                    draws   worst |ΔS|/b   worst |Δdf|/b_df   p outside range");
    let mut factor: f64 = 1.0;
    for (m, w) in &per_method {
        println!("{m:24} {:>7}   {:>12.4}   {:>16.4}   {:>15}", w.draws, w.stat, w.df, w.p_outside);
        factor = factor.max(w.stat).max(w.df);
    }
    println!("golden rows with a range: {ranged} of {}", rows.len());
    assert!(ranged >= 20, "too few rows carry a range for the check to mean anything: {ranged}");
    assert!(factor <= 1.0, "the first-order range is too narrow: widen it by a factor of {factor:.4}");
    assert_eq!(per_method.values().map(|w| w.p_outside).sum::<usize>(), 0, "a draw's p fell outside the p range");
}

#[test]
fn a_binding_on_whole_number_columns_only_has_a_zero_range() {
    let raw = RawTable::from_rows(
        vec!["pre".into(), "post".into()],
        [(3, 5), (4, 7), (5, 6), (6, 8), (2, 2), (7, 9)].iter().map(|(a, b)| vec![a.to_string(), b.to_string()]).collect(),
    );
    let draft = BindingDraft::new(L1MethodId::PairedT, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::PairedT).variant_kind))
        .role(RoleName::First, &[ColumnAlias(1)])
        .role(RoleName::Second, &[ColumnAlias(2)]);
    let r = results(b"integers", &raw, draft)[0];
    assert_eq!(r.precision, Precision::Exact);
}

/* ---- timings ------------------------------------------------------------------ */

fn synthetic(rows: usize) -> RawTable {
    let mut rng = SplitMix(5000);
    let headers = ["y", "g2", "g3", "x1", "x2", "x3", "pre", "post"].iter().map(|s| s.to_string()).collect();
    let body = (0..rows)
        .map(|i| {
            let x1 = 5.0 + rng.unit() * 3.0;
            let x2 = rng.unit();
            let x3 = 10.0 + rng.unit() * 4.0;
            let y = 2.0 + 0.5 * x1 + 0.6 * x2 - 0.1 * x3 + rng.unit();
            let pre = 20.0 + rng.unit() * 4.0;
            vec![
                format!("{y:.2}"),
                format!("{}", 1 + i % 2),
                format!("{}", 1 + i % 3),
                format!("{x1:.2}"),
                format!("{x2:.2}"),
                format!("{x3:.2}"),
                format!("{pre:.2}"),
                format!("{:.2}", pre + 1.0 + rng.unit()),
            ]
        })
        .collect();
    RawTable::from_rows(headers, body)
}

#[test]
fn compute_with_its_precision_range_is_timed_on_the_largest_fixture_and_5000_rows() {
    // The largest golden data file by rows.
    let mut largest = String::new();
    let mut most = 0;
    for g in golden() {
        let (_, raw) = data(&g.data);
        if raw.rows.len() > most {
            most = raw.rows.len();
            largest = g.data.clone();
        }
    }
    let rows = golden();
    let g = rows.iter().find(|g| g.data == largest).unwrap();
    let (bytes, raw) = data(&g.data);
    let (draft, _) = draft_of(g, &raw.headers);
    let t = Instant::now();
    results(&bytes, &raw, draft);
    println!("largest fixture {largest} ({most} rows, {}): {:?}", g.method, t.elapsed());

    let raw = synthetic(5000);
    let c = |n: u32| ColumnAlias(n);
    let methods: Vec<(&str, BindingDraft)> = vec![
        ("independent_t", BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::IndependentT).variant_kind)).role(RoleName::Outcome, &[c(1)]).role(RoleName::Group, &[c(2)])),
        ("paired_t", BindingDraft::new(L1MethodId::PairedT, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::PairedT).variant_kind)).role(RoleName::First, &[c(7)]).role(RoleName::Second, &[c(8)])),
        ("one_way_anova", BindingDraft::new(L1MethodId::OneWayAnova, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::OneWayAnova).variant_kind)).role(RoleName::Outcome, &[c(1)]).role(RoleName::Group, &[c(3)])),
        ("pearson", BindingDraft::new(L1MethodId::Pearson, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::Pearson).variant_kind)).role(RoleName::X, &[c(4)]).role(RoleName::Y, &[c(1)])),
        ("spearman", BindingDraft::new(L1MethodId::Spearman, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::Spearman).variant_kind)).role(RoleName::X, &[c(4)]).role(RoleName::Y, &[c(1)])),
        ("chi_square_independence", BindingDraft::new(L1MethodId::ChiSquareIndependence, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::ChiSquareIndependence).variant_kind)).role(RoleName::RowVariable, &[c(2)]).role(RoleName::ColumnVariable, &[c(3)])),
        ("ols", BindingDraft::new(L1MethodId::Ols, VariantChoice::not_stated(MethodCatalogue::entry(L1MethodId::Ols).variant_kind)).role(RoleName::Outcome, &[c(1)]).role(RoleName::Predictors, &[c(4), c(5), c(6)]).target(OlsTarget::OverallF)),
    ];
    for (name, draft) in methods {
        let t = Instant::now();
        let r = results(b"synthetic 5000", &raw, draft);
        println!("5,000 rows {name:24}: {:?} (precision {:?})", t.elapsed(), r[0].precision);
    }
}
