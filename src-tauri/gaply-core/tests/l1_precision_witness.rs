//! The joint evaluation's precision witness (Blocker B-2), as named
//! regressions (Phase 2 evaluation fixes, part A, section 2).
//!
//! Both files are stored to 1 decimal. Each has unrounded cells that round to
//! EXACTLY the stored file and give the printed result, found by
//! `/usr/local/bin/Rscript precision_witness.R` in
//! `tests/fixtures/stats_l1/precision_sweep/` (seed 7), which also writes the
//! two stored files. The papers are honest; only the deposit's rounding
//! differs.
//! Before this step the first-order range was too narrow for data whose
//! spread is close to the rounding step, and both gave NotReproduced (s1 with
//! `conclusion_changes: true`). They must be CouldNotCheck(data_precision).

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, VariantChoice, VariantKind};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_l1::reproduce::{reproduce, L1CouldNotCheck, L1Verdict};
use gaply_core::stats_l1::run_state::L1RunState;

fn table(headers: [&str; 2], rows: &[[&str; 2]]) -> Dataset {
    let rows = rows
        .iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows),
        DataFingerprint::of_bytes(b"witness", "in-memory", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// s1, the stored file. R on it: `summary(aov(y ~ g))` -> F = 4.200000,
/// p = 0.072338. The unrounded cells 0.954543, 1.050604, 0.962833 / 1.151116,
/// 1.138039, 1.162209 / 1.142619, 1.259555, 1.230356 round to it and give
/// F(2, 6) = 17.657419, p = 0.003063.
fn s1() -> Dataset {
    table(
        ["y", "g"],
        &[
            ["1.0", "a"],
            ["1.1", "a"],
            ["1.0", "a"],
            ["1.2", "b"],
            ["1.1", "b"],
            ["1.2", "b"],
            ["1.1", "c"],
            ["1.3", "c"],
            ["1.2", "c"],
        ],
    )
}

/// s2, the stored file. R on it: `cor.test(x, y)` -> r = 0.996069,
/// p = 0.000023. The unrounded pairs (1.047155, 1.971775), (1.065615,
/// 2.225782), (1.330081, 2.473348), (1.351998, 2.836642), (1.575724,
/// 3.245412), (1.749182, 3.265351) round to it and give r(4) = 0.956247,
/// p = 0.002830.
fn s2() -> Dataset {
    table(
        ["xa", "yb"],
        &[
            ["1.0", "2.0"],
            ["1.1", "2.2"],
            ["1.3", "2.5"],
            ["1.4", "2.8"],
            ["1.6", "3.2"],
            ["1.7", "3.3"],
        ],
    )
}

fn verdict(
    data: fn() -> Dataset,
    family: Family,
    stat: &str,
    df: Vec<&str>,
    p: &str,
    draft: BindingDraft,
) -> L1Verdict {
    let mut s = L1RunState::new();
    s.load_dataset(data());
    let claim = s
        .add_claim(&UserClaim {
            family,
            statistic: stat,
            df,
            n: None,
            p: UserP::Reported {
                raw: p,
                relation: Relation::Eq,
            },
            location: ClaimLocation {
                section_index: 0,
                section: SectionKind::Results,
                paragraph: 0,
                sentence: 0,
            },
            sentence: None,
        })
        .unwrap();
    let pid = s
        .add_proposal(claim, draft, ProposalSource::Manual { edited_from: None })
        .unwrap();
    let token = s.confirm(pid).unwrap().1;
    reproduce(
        &s,
        &[(s.claim(claim).unwrap().revision.clone(), token)],
        move || Ok(data()),
    )
    .claims
    .remove(0)
    .verdict
}

#[test]
fn the_anova_witness_is_could_not_check_data_precision() {
    let draft = BindingDraft::new(
        L1MethodId::OneWayAnova,
        VariantChoice::not_stated(VariantKind::None),
    )
    .role(RoleName::Outcome, &[ColumnAlias(1)])
    .role(RoleName::Group, &[ColumnAlias(2)])
    .levels(RoleName::Group, &["a", "b", "c"]);
    assert_eq!(
        verdict(s1, Family::F, "17.66", vec!["2", "6"], ".003", draft),
        L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals: 1 })
    );
}

#[test]
fn the_pearson_witness_is_could_not_check_data_precision() {
    let draft = BindingDraft::new(
        L1MethodId::Pearson,
        VariantChoice::not_stated(VariantKind::None),
    )
    .role(RoleName::X, &[ColumnAlias(1)])
    .role(RoleName::Y, &[ColumnAlias(2)]);
    assert_eq!(
        verdict(s2, Family::R, ".96", vec!["4"], ".003", draft),
        L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals: 1 })
    );
}
