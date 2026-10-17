//! Findings from Phase 2, Step 8, section 4 (real papers with open data).
//! Written failing in Step 8; passes since Step 8b's rule 1 (data
//! precision), and now pins the exact verdict.
//!
//! **A deposited data file rounded coarser than the result is sensitive to
//! gives a NotReproduced that the data cannot support.** Seen on two PLOS ONE
//! papers: 0303785, claim 12, printed t(19) = 3.43, file gives 3.4247; and
//! 0106616, claims 4, 6, 8 and 9, cells stored to 2 decimals. Jittering every
//! bound cell within half its last stored digit moves the computed statistic
//! across the printed value in all five, so the file's own precision cannot
//! tell a wrong paper from a rounded deposit. Before Step 8b, Level 1 treated
//! each cell as exact and returned NotReproduced.
//!
//! The fixture is synthetic: twelve pairs drawn in Python (seed 7), the
//! printed t and p computed from the exact values, and the file holding the
//! same values rounded to 2 decimals, as a deposit would.
//!
//! Step 8b decided what it returns: CouldNotCheck with `data_precision`, 2
//! decimals.

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::pvalue;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, VariantChoice, VariantKind};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_l1::reproduce::{reproduce, L1CouldNotCheck, L1Verdict};
use gaply_core::stats_l1::run_state::L1RunState;

/// The exact values' paired t (computed in Python, seed 7; re-derived below).
const EXACT_PRE: [f64; 12] = [0.8032, 1.1579, 1.2822, 1.2148, 1.0863, 1.0356, 1.0381, 1.1438, 0.956, 1.0694, 1.1432, 1.0002];
const EXACT_POST: [f64; 12] = [0.9449, 1.2758, 1.5735, 1.3038, 1.085, 1.0409, 1.0865, 1.3047, 0.9656, 1.1657, 1.4137, 0.7424];
/// The deposited file: the same values as Python's `round(x, 2)` wrote them.
const DEPOSITED_PRE: [&str; 12] = ["0.8", "1.16", "1.28", "1.21", "1.09", "1.04", "1.04", "1.14", "0.96", "1.07", "1.14", "1.0"];
const DEPOSITED_POST: [&str; 12] = ["0.94", "1.28", "1.57", "1.3", "1.08", "1.04", "1.09", "1.3", "0.97", "1.17", "1.41", "0.74"];

fn paired_t(a: &[f64], b: &[f64]) -> f64 {
    let d: Vec<f64> = a.iter().zip(b).map(|(x, y)| y - x).collect();
    let n = d.len() as f64;
    let m = d.iter().sum::<f64>() / n;
    let sd = (d.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
    m / (sd / n.sqrt())
}

fn deposited() -> Dataset {
    let rows: Vec<Vec<String>> =
        DEPOSITED_PRE.iter().zip(DEPOSITED_POST).map(|(a, b)| vec![a.to_string(), b.to_string()]).collect();
    let raw = RawTable::from_rows(vec!["pre".into(), "post".into()], rows);
    build_dataset(raw, DataFingerprint::of_bytes(b"deposited 2dp", "in-memory", TableRef::FirstTable)).expect("dataset")
}

#[test]
fn a_mismatch_the_deposited_cells_rounding_can_explain_is_could_not_check_data_precision() {
    let t = paired_t(&EXACT_PRE, &EXACT_POST);
    let p = pvalue::t_two_tailed(t, 11.0).unwrap();
    let (t_printed, p_printed) = (format!("{:.2}", t.abs()), format!("{:.3}", p).trim_start_matches('0').to_string());
    assert_eq!(t_printed, "1.96", "the exact values print t = 1.96");
    // The deposited cells give 1.92: the premise of the finding.
    let rounded_pre: Vec<f64> = DEPOSITED_PRE.iter().map(|v| v.parse().unwrap()).collect();
    let rounded_post: Vec<f64> = DEPOSITED_POST.iter().map(|v| v.parse().unwrap()).collect();
    assert_eq!(format!("{:.2}", paired_t(&rounded_pre, &rounded_post).abs()), "1.92");

    let mut s = L1RunState::new();
    s.load_dataset(deposited());
    let claim = s
        .add_claim(&UserClaim {
            family: Family::T,
            statistic: &t_printed,
            df: vec!["11"],
            n: None,
            p: UserP::Reported { raw: &p_printed, relation: Relation::Eq },
            location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
            sentence: None,
        })
        .unwrap();
    let draft = BindingDraft::new(L1MethodId::PairedT, VariantChoice::not_stated(VariantKind::None))
        .role(RoleName::First, &[ColumnAlias(1)])
        .role(RoleName::Second, &[ColumnAlias(2)]);
    let pid = s.add_proposal(claim, draft, ProposalSource::Manual { edited_from: None }).unwrap();
    let token = s.confirm(pid).unwrap().1;
    let out = reproduce(&s, &[(s.claim(claim).unwrap().revision.clone(), token)], || Ok(deposited()));
    let v = &out.claims[0].verdict;
    assert_eq!(
        *v,
        L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals: 2 }),
        "the printed t(11) = {t_printed}, p = {p_printed} is reachable by rounding the deposited 2-decimal cells"
    );
}
