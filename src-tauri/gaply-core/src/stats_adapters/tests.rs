//! The façades' refusals with forged specs, against the real table and
//! against test-only tables for the states the real one has no capability
//! in; and the AI capability check. Unit tests, because a test-only table can
//! only be built inside this crate.

use super::*;
use crate::contracts::capability::Lifecycle;
use crate::extract::extract_from_text;
use crate::extract::SectionKind;
use crate::stats_desc::{ColumnRequest, TreatAs};
use crate::stats_l0::number::Relation;
use crate::stats_l0::result::Family;
use crate::stats_l1::binding::{BindingDraft, ProposalSource};
use crate::stats_l1::catalogue::{MethodCatalogue, RoleName, VariantChoice};
use crate::stats_l1::claim::{ClaimId, ClaimLocation, UserClaim, UserP};
use crate::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef};
use crate::stats_l1::reproduce::{L1CouldNotCheck, L1Verdict};
use crate::stats_l1::run_state::GateRejection;

fn real() -> Registry {
    Registry::statistics()
}

fn forged(id: &str, version: u32) -> OperationSpec<()> {
    let mut s = check_spec();
    s.capability_id = CapabilityId::parse(id).expect("well formed");
    s.capability_version = version;
    s
}

fn code<T: std::fmt::Debug>(r: Result<T, Refusal>) -> CapabilityCode {
    r.expect_err("refused").code
}

/* ---- stats.check ------------------------------------------------------ */

const MANUSCRIPT: &str = "A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045.\n";

#[test]
fn check_refuses_an_unknown_id_a_wrong_version_a_planned_capability_and_a_confirmation() {
    let ex = extract_from_text(MANUSCRIPT);
    let t = TableStructure::NotAvailable;
    let b = Build::Devtools;
    assert_eq!(
        code(check_in(&real(), &forged("stats.checks", 1), b, &ex, t)),
        CapabilityCode::UnknownCapability
    );
    assert_eq!(
        code(check_in(&real(), &forged("stats.check", 2), b, &ex, t)),
        CapabilityCode::VersionMismatch
    );
    let planned = real().with_lifecycle("stats.check", Lifecycle::Planned);
    assert_eq!(
        code(check_in(&planned, &check_spec(), b, &ex, t)),
        CapabilityCode::NotRunnable
    );
    // Another capability's spec is not this façade's.
    assert_eq!(
        code(check_in(&real(), &forged("stats.describe", 1), b, &ex, t)),
        CapabilityCode::UnknownCapability
    );
    let mut with_ref = check_spec();
    with_ref.confirmation_refs = vec![ConfirmationId::parse(&"a".repeat(64)).unwrap()];
    assert_eq!(
        code(check_in(&real(), &with_ref, b, &ex, t)),
        CapabilityCode::ConfirmationUnmatched
    );
    // And the real spec runs, giving exactly the direct call's outcome.
    let direct = check_reading(&ex, read_results(&ex, t));
    assert_eq!(
        check_in(&real(), &check_spec(), Build::Standard, &ex, t).unwrap(),
        direct
    );
    assert_eq!(direct.claims_read, 1, "the fixture reaches Level 0");
}

/* ---- stats.verify ----------------------------------------------------- */

/// One Pearson claim on synthetic data, confirmed: the state, the pair and
/// the dataset's bytes.
fn pearson_run() -> (
    L1RunState,
    Vec<(RevisionHash, ConfirmationToken)>,
    ClaimId,
    Dataset,
) {
    let headers = vec!["x".to_string(), "y".to_string()];
    let rows: Vec<Vec<String>> = (1..=12)
        .map(|i| {
            vec![
                format!("{i}.{:02}", (i * 37) % 100),
                format!("{}.{:02}", i * 2 + i % 3, (i * 53) % 100),
            ]
        })
        .collect();
    let bytes = b"x,y\nsynthetic pearson rows\n";
    let dataset = || {
        build_dataset(
            RawTable::from_rows(headers.clone(), rows.clone()),
            DataFingerprint::of_bytes(bytes, "unit csv", TableRef::FirstTable),
        )
        .expect("dataset")
    };
    let mut state = L1RunState::new();
    state.load_dataset(dataset());
    let claim = state
        .add_claim(&UserClaim {
            family: Family::R,
            statistic: ".95",
            df: vec!["10"],
            n: None,
            p: UserP::Reported {
                raw: ".001",
                relation: Relation::Lt,
            },
            location: ClaimLocation {
                section_index: 0,
                section: SectionKind::Results,
                paragraph: 0,
                sentence: 0,
            },
            sentence: None,
        })
        .expect("claim");
    let method = L1MethodId::Pearson;
    let draft = BindingDraft::new(
        method,
        VariantChoice::not_stated(MethodCatalogue::entry(method).variant_kind),
    )
    .role(RoleName::X, &[ColumnAlias(1)])
    .role(RoleName::Y, &[ColumnAlias(2)]);
    let proposal = state
        .add_proposal(claim, draft, ProposalSource::Manual { edited_from: None })
        .expect("proposal");
    let token = state.confirm(proposal).expect("confirmed").1;
    let revision = state.claim(claim).expect("claim").revision.clone();
    (state, vec![(revision, token)], claim, dataset())
}

fn refs(pairs: &[(RevisionHash, ConfirmationToken)]) -> Vec<ConfirmationId> {
    pairs
        .iter()
        .map(|(_, t)| ConfirmationId::parse(t.as_str()).expect("a token is 64 hex"))
        .collect()
}

#[test]
fn verify_refuses_forged_specs_and_runs_the_real_one_unchanged() {
    let (state, pairs, _, data) = pearson_run();
    let b = Build::Standard;
    let read = || Ok(data.clone());
    let good = verify_spec(L1MethodId::Pearson, refs(&pairs));

    let mut unknown = good.clone();
    unknown.capability_id = CapabilityId::parse("stats.verify.kruskal").unwrap();
    assert_eq!(
        code(verify_in(&real(), &[unknown], b, &state, &pairs, read)),
        CapabilityCode::UnknownCapability
    );

    let mut version = good.clone();
    version.capability_version = 2;
    assert_eq!(
        code(verify_in(&real(), &[version], b, &state, &pairs, read)),
        CapabilityCode::VersionMismatch
    );

    let planned = real().with_lifecycle("stats.verify.pearson", Lifecycle::Planned);
    assert_eq!(
        code(verify_in(
            &planned,
            &[good.clone()],
            Build::Devtools,
            &state,
            &pairs,
            read
        )),
        CapabilityCode::NotRunnable
    );

    // Missing: a spec with no confirmation, and a live token no spec carries.
    let bare = verify_spec(L1MethodId::Pearson, Vec::new());
    assert_eq!(
        code(verify_in(&real(), &[bare], b, &state, &pairs, read)),
        CapabilityCode::ConfirmationMissing
    );
    assert_eq!(
        code(verify_in(&real(), &[], b, &state, &pairs, read)),
        CapabilityCode::ConfirmationMissing
    );
    // The token under another method's spec is not carried by its own.
    let elsewhere = verify_spec(L1MethodId::Spearman, refs(&pairs));
    assert_eq!(
        code(verify_in(&real(), &[elsewhere], b, &state, &pairs, read)),
        CapabilityCode::ConfirmationMissing
    );
    // A confirmation the computation does not use.
    let stray = verify_spec(
        L1MethodId::Pearson,
        vec![ConfirmationId::parse(&"b".repeat(64)).unwrap()],
    );
    assert_eq!(
        code(verify_in(
            &real(),
            &[good.clone(), stray],
            b,
            &state,
            &pairs,
            read
        )),
        CapabilityCode::ConfirmationUnmatched
    );
    // Another capability's spec.
    assert_eq!(
        code(verify_in(&real(), &[check_spec()], b, &state, &pairs, read)),
        CapabilityCode::UnknownCapability
    );

    let direct = reproduce(&state, &pairs, read);
    let wrapped = verify_in(&real(), &[good], b, &state, &pairs, read).expect("runs");
    assert_eq!(wrapped, direct);
    assert_eq!(
        direct.counts.reproduced + direct.counts.not_reproduced,
        1,
        "the claim was computed: {:?}",
        direct.claims[0].verdict
    );
}

#[test]
fn a_stale_or_revoked_token_stays_refused_with_todays_code() {
    // Revoked: the binding withdrawn. The façade lets `reproduce` refuse it,
    // so the outcome is the direct call's: `token_stale`, revoked.
    let (mut state, pairs, _, data) = pearson_run();
    assert!(state.withdraw(&pairs[0].1));
    let spec = verify_spec(L1MethodId::Pearson, refs(&pairs));
    let read = || Ok(data.clone());
    let wrapped = verify_in(
        &real(),
        &[spec.clone()],
        Build::Standard,
        &state,
        &pairs,
        read,
    )
    .expect("runs");
    assert_eq!(wrapped, reproduce(&state, &pairs, read));
    assert!(matches!(
        wrapped.claims[0].verdict,
        L1Verdict::CouldNotCheck(L1CouldNotCheck::TokenStale(GateRejection::Revoked(_)))
    ));
    // With no spec at all, the same.
    assert_eq!(
        verify_in(&real(), &[], Build::Standard, &state, &pairs, read).unwrap(),
        wrapped
    );

    // Stale: the claim edited after the confirmation.
    let (mut state, pairs, claim, data) = pearson_run();
    state
        .edit_claim(
            claim,
            &UserClaim {
                family: Family::R,
                statistic: ".90",
                df: vec!["10"],
                n: None,
                p: UserP::Reported {
                    raw: ".001",
                    relation: Relation::Lt,
                },
                location: ClaimLocation {
                    section_index: 0,
                    section: SectionKind::Results,
                    paragraph: 0,
                    sentence: 0,
                },
                sentence: None,
            },
        )
        .expect("edited");
    let read = || Ok(data.clone());
    let spec = verify_spec(L1MethodId::Pearson, refs(&pairs));
    let wrapped = verify_in(&real(), &[spec], Build::Standard, &state, &pairs, read).expect("runs");
    assert_eq!(wrapped, reproduce(&state, &pairs, read));
    assert!(matches!(
        wrapped.claims[0].verdict,
        L1Verdict::CouldNotCheck(L1CouldNotCheck::TokenStale(_))
    ));
}

/* ---- stats.describe --------------------------------------------------- */

fn describe_spec(data: &Dataset, id: &str, version: u32) -> OperationSpec<DescribeParams> {
    let mut s = stats_desc::spec(
        OperationId(7),
        data,
        DescribeParams {
            columns: vec![ColumnRequest {
                alias: stats_desc::Alias(ColumnAlias(1)),
                treat_as: TreatAs::Numeric,
            }],
            missing_codes: Vec::new(),
        },
    );
    s.capability_id = CapabilityId::parse(id).unwrap();
    s.capability_version = version;
    s
}

#[test]
fn describe_words_a_registry_refusal_with_its_own_codes() {
    let (_, _, _, data) = pearson_run();
    let b = Build::Standard;
    let codes =
        |r: &ValidationReport<DescCode>| r.errors.iter().map(|d| d.code).collect::<Vec<_>>();

    let unknown = describe_validate_in(
        &real(),
        &describe_spec(&data, "stats.describes", 1),
        b,
        &data,
    );
    assert_eq!(codes(&unknown), vec![DescCode::UnknownCapability]);
    let version = describe_validate_in(
        &real(),
        &describe_spec(&data, "stats.describe", 3),
        b,
        &data,
    );
    assert_eq!(codes(&version), vec![DescCode::CapabilityVersionMismatch]);
    assert_eq!(
        version.errors[0].params.get(&ParamKey::Expected),
        Some(&ParamValue::Int(1))
    );
    assert_eq!(
        version.errors[0].params.get(&ParamKey::Found),
        Some(&ParamValue::Int(3))
    );
    let planned = real().with_lifecycle("stats.describe", Lifecycle::Planned);
    let p = describe_validate_in(
        &planned,
        &describe_spec(&data, "stats.describe", 1),
        b,
        &data,
    );
    assert_eq!(codes(&p), vec![DescCode::UnknownCapability]);
    assert!(!p.may_run());
    let run = describe_until_in(
        &planned,
        &describe_spec(&data, "stats.describe", 1),
        b,
        &data,
        |_| ControlFlow::<()>::Continue(()),
    );
    assert_eq!(
        run.unwrap_err(),
        Halt::Refused(Diagnostic::error(DescCode::UnknownCapability))
    );

    // The real spec: exactly the direct calls.
    let good = describe_spec(&data, "stats.describe", 1);
    assert_eq!(
        describe_validate_in(&real(), &good, b, &data),
        stats_desc::validate(&data, &good)
    );
    assert_eq!(
        describe_until_in(
            &real(),
            &good,
            b,
            &data,
            |_| ControlFlow::<()>::Continue(())
        )
        .unwrap(),
        stats_desc::describe(&data, &good.parameters).unwrap()
    );
}

/* ---- AI --------------------------------------------------------------- */

#[test]
fn a_method_an_ai_answer_names_must_be_a_runnable_capability() {
    for m in L1MethodId::ALL {
        assert!(
            method_is_runnable_in(&real(), m, Build::Standard),
            "{} is Beta",
            m.id()
        );
    }
    // In the catalogue, not implemented: Planned, never runnable.
    let planned = real().with_lifecycle("stats.verify.ols", Lifecycle::Planned);
    for b in [Build::Standard, Build::Devtools] {
        assert!(!method_is_runnable_in(&planned, L1MethodId::Ols, b));
        assert!(method_is_runnable_in(&planned, L1MethodId::Pearson, b));
    }
}
