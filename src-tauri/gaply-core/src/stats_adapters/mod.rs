//! **The verifier's façades (Research OS, Phase 4A): each Statistics
//! capability's one door, checked against the registry, around the existing
//! function, which does not move.**
//!
//! Pure: no file, network, clock, model or log (`tests/contracts_are_pure.rs`
//! scans this directory). Each façade takes an `OperationSpec`, asks
//! [`Registry::require_runnable`] (a registered id, the capability's version,
//! a lifecycle this build may run), checks the confirmation where the
//! capability needs one, then calls the existing function and returns its
//! existing output, unchanged (decision 50):
//!
//! * [`check`] (`stats.check`): `stats_l0::result::read_results`, then
//!   `stats_l0::outcome::check_reading`. Takes no confirmation.
//! * [`verify`] (`stats.verify.<method_id>`, one generic façade keyed by
//!   method): `stats_l1::reproduce::reproduce`. Each spec names one method
//!   and carries, as `confirmation_refs`, the confirmation tokens of that
//!   method's bindings. Whether a token is live is still the gate's answer,
//!   inside `reproduce`: a stale or revoked token gives the claim today's
//!   `token_stale`, and a claim with no token is skipped, as today.
//! * [`describe_validate`] and [`describe_until`] (`stats.describe`):
//!   `stats_desc::validate` and `stats_desc::describe_until`. A registry
//!   refusal is worded with the existing `stats.desc.unknown_capability` or
//!   `stats.desc.capability_version_mismatch`, so the describe wire keeps
//!   its codes.
//! * [`ttest_validate`] and [`ttest_run`] (`stats.analyze.independent_t`,
//!   Phase 4B-1): `stats_ttest::validate` and `stats_ttest::run`, the same
//!   way, a registry refusal worded `stats.ttest.unknown_capability` or
//!   `stats.ttest.capability_version_mismatch`.
//!
//! The Level 0 and Level 1 specs carry no parameters (`()`) and no
//! `InputRef`: their inputs are the manuscript's extraction and Level 1's
//! run, held by the app and bound by the token. Their `ResultBundle` and
//! manifest wait for Phase 5's provenance (decision 50).

use std::ops::ControlFlow;

use crate::contracts::capability::{Build, CapabilityCode, ConfirmationId};
use crate::contracts::{
    CapabilityId, Diagnostic, OperationId, OperationSpec, ParamKey, ParamValue, ResourceEstimate,
    SchemaVersion, ValidationReport, WorkspaceId,
};
use crate::extract::ExtractionResult;
use crate::stats_desc::{self, DescCode, DescribeParams, DescribeResult, Halt};
use crate::stats_l0::outcome::{check_reading, StatsConsistencyOutcome};
use crate::stats_l0::result::{read_results, TableStructure};
use crate::stats_l1::catalogue::L1MethodId;
use crate::stats_l1::claim::RevisionHash;
use crate::stats_l1::dataset::{Dataset, L1IngestRefusal};
use crate::stats_l1::reproduce::{reproduce, ReproductionOutcome};
use crate::stats_l1::run_state::L1RunState;
use crate::stats_l1::token::ConfirmationToken;
use crate::stats_registry::{Entry, Kind, Registry, CHECK_ID, VERIFY_PREFIX};
use crate::stats_ttest::{self, TTestCode, TTestParams, TTestResult};

type Refusal = Diagnostic<CapabilityCode>;

fn refused(code: CapabilityCode) -> Refusal {
    Diagnostic::error(code)
}

/// The registry's check, and the spec's workspace against the capability's.
fn runnable<'r, P>(
    registry: &'r Registry,
    spec: &OperationSpec<P>,
    build: Build,
) -> Result<&'r Entry, Refusal> {
    let entry = registry.require_runnable(&spec.capability_id, spec.capability_version, build)?;
    if spec.workspace_id != entry.descriptor.workspace_id {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    Ok(entry)
}

/// A spec of a capability that takes no confirmation must carry none.
fn no_confirmation<P>(spec: &OperationSpec<P>) -> Result<(), Refusal> {
    if spec.confirmation_refs.is_empty() {
        Ok(())
    } else {
        Err(refused(CapabilityCode::ConfirmationUnmatched))
    }
}

/// A spec built by the app for a Level 0 or Level 1 run: no parameters, no
/// input reference, no seed. The operation id is 0: these runs are not run
/// records (the run manager numbers only `stats.describe`'s).
fn app_spec(id: &str, version: u32, confirmation_refs: Vec<ConfirmationId>) -> OperationSpec<()> {
    OperationSpec {
        schema_version: SchemaVersion,
        operation_id: OperationId(0),
        workspace_id: WorkspaceId::Statistics,
        capability_id: CapabilityId::parse(id).expect("a well-formed Statistics capability id"),
        capability_version: version,
        inputs: Vec::new(),
        parameters: (),
        random_seed: None,
        confirmation_refs,
    }
}

/* ------------------------------------------------------------------------ *
 *  stats.check
 * ------------------------------------------------------------------------ */

/// The spec the Stats Check's command builds.
pub fn check_spec() -> OperationSpec<()> {
    let version = Registry::statistics()
        .entries()
        .iter()
        .find(|e| e.kind == Kind::Check)
        .map(|e| e.descriptor.version)
        .expect("stats.check is registered");
    app_spec(CHECK_ID, version, Vec::new())
}

/// `stats.check`: Level 0 on one manuscript's extraction.
pub fn check(
    spec: &OperationSpec<()>,
    build: Build,
    ex: &ExtractionResult,
    tables: TableStructure,
) -> Result<StatsConsistencyOutcome, Refusal> {
    check_in(&Registry::statistics(), spec, build, ex, tables)
}

pub(crate) fn check_in(
    registry: &Registry,
    spec: &OperationSpec<()>,
    build: Build,
    ex: &ExtractionResult,
    tables: TableStructure,
) -> Result<StatsConsistencyOutcome, Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::Check {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)?;
    Ok(check_reading(ex, read_results(ex, tables)))
}

/* ------------------------------------------------------------------------ *
 *  stats.verify.<method_id>
 * ------------------------------------------------------------------------ */

/// The spec for one Level 1 method, carrying its bindings' confirmations.
pub fn verify_spec(
    method: L1MethodId,
    confirmation_refs: Vec<ConfirmationId>,
) -> OperationSpec<()> {
    let id = format!("{VERIFY_PREFIX}{}", method.id());
    let version = Registry::statistics()
        .entries()
        .iter()
        .find(|e| e.kind == Kind::Verify(method))
        .map(|e| e.descriptor.version)
        .expect("every Level 1 method is registered");
    app_spec(&id, version, confirmation_refs)
}

fn same_token(r: &ConfirmationId, t: &ConfirmationToken) -> bool {
    r.as_str() == t.as_str()
}

/// `stats.verify`: Level 1's reproduction of every claim in `state`, behind
/// the registry. Each spec must be a runnable Level 1 capability carrying at
/// least one confirmation, each of them one of `pairs`' tokens; and every
/// token that would open the gate must be carried by the spec of its
/// binding's method. Then `reproduce` runs exactly as before, and its
/// outcome is returned unchanged.
pub fn verify(
    specs: &[OperationSpec<()>],
    build: Build,
    state: &L1RunState,
    pairs: &[(RevisionHash, ConfirmationToken)],
    read: impl FnOnce() -> Result<Dataset, L1IngestRefusal>,
) -> Result<ReproductionOutcome, Refusal> {
    verify_in(&Registry::statistics(), specs, build, state, pairs, read)
}

pub(crate) fn verify_in(
    registry: &Registry,
    specs: &[OperationSpec<()>],
    build: Build,
    state: &L1RunState,
    pairs: &[(RevisionHash, ConfirmationToken)],
    read: impl FnOnce() -> Result<Dataset, L1IngestRefusal>,
) -> Result<ReproductionOutcome, Refusal> {
    let mut carried: Vec<(L1MethodId, &[ConfirmationId])> = Vec::with_capacity(specs.len());
    for spec in specs {
        let entry = runnable(registry, spec, build)?;
        let Kind::Verify(method) = entry.kind else {
            return Err(refused(CapabilityCode::UnknownCapability));
        };
        if spec.confirmation_refs.is_empty() {
            return Err(refused(CapabilityCode::ConfirmationMissing));
        }
        for r in &spec.confirmation_refs {
            if !pairs.iter().any(|(_, t)| same_token(r, t)) {
                return Err(refused(CapabilityCode::ConfirmationUnmatched));
            }
        }
        carried.push((method, &spec.confirmation_refs));
    }
    // The claims `reproduce` would compute, found as it finds them: the
    // first pair naming the claim's revision, whose token opens the gate.
    for claim in state.claims() {
        let Some((_, token)) = pairs.iter().find(|(rev, _)| *rev == claim.revision) else {
            continue;
        };
        let Ok(binding) = state.gate(token, claim.id) else {
            continue;
        };
        let covered = carried
            .iter()
            .any(|(m, refs)| *m == binding.method() && refs.iter().any(|r| same_token(r, token)));
        if !covered {
            return Err(refused(CapabilityCode::ConfirmationMissing));
        }
    }
    Ok(reproduce(state, pairs, read))
}

/* ------------------------------------------------------------------------ *
 *  stats.describe
 * ------------------------------------------------------------------------ */

/// A registry refusal in the descriptive summary's own codes:
/// `version_mismatch` keeps its `expected` and `found`; every other refusal
/// is `unknown_capability`, which is what the screen already words.
fn describe_refusal(r: &Refusal) -> Diagnostic<DescCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(DescCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(DescCode::UnknownCapability),
    }
}

fn describe_entry(
    registry: &Registry,
    spec: &OperationSpec<DescribeParams>,
    build: Build,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::Describe {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// `stats.describe`'s validation: the registry first, so a spec the
/// registry refuses is `Unsupported` before anything reads the dataset;
/// then `stats_desc::validate`, unchanged.
pub fn describe_validate(
    spec: &OperationSpec<DescribeParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<DescCode> {
    describe_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn describe_validate_in(
    registry: &Registry,
    spec: &OperationSpec<DescribeParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<DescCode> {
    match describe_entry(registry, spec, build) {
        Ok(()) => stats_desc::validate(dataset, spec),
        Err(r) => {
            let rows = dataset.rows() as u64;
            let columns = spec.parameters.columns.len() as u64;
            let estimate = ResourceEstimate {
                rows,
                columns,
                cells: rows.saturating_mul(columns),
            };
            ValidationReport::new(
                vec![describe_refusal(&r)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                estimate,
            )
        }
    }
}

/// `stats.describe`'s run: the registry again, then
/// `stats_desc::describe_until`, unchanged.
pub fn describe_until<B>(
    spec: &OperationSpec<DescribeParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<DescribeResult, Halt<B>> {
    describe_until_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn describe_until_in<B>(
    registry: &Registry,
    spec: &OperationSpec<DescribeParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<DescribeResult, Halt<B>> {
    describe_entry(registry, spec, build).map_err(|r| Halt::Refused(describe_refusal(&r)))?;
    stats_desc::describe_until(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  stats.analyze.independent_t
 * ------------------------------------------------------------------------ */

/// A registry refusal in the t-test's own codes, as [`describe_refusal`]
/// does for the descriptive summary.
fn ttest_refusal(r: &Refusal) -> Diagnostic<TTestCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(TTestCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(TTestCode::UnknownCapability),
    }
}

fn ttest_entry(
    registry: &Registry,
    spec: &OperationSpec<TTestParams>,
    build: Build,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::AnalyzeIndependentT {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// `stats.analyze.independent_t`'s validation: the registry first, so a
/// spec the registry refuses is `Unsupported` before anything reads the
/// dataset; then `stats_ttest::validate`, unchanged.
pub fn ttest_validate(
    spec: &OperationSpec<TTestParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<TTestCode> {
    ttest_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn ttest_validate_in(
    registry: &Registry,
    spec: &OperationSpec<TTestParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<TTestCode> {
    match ttest_entry(registry, spec, build) {
        Ok(()) => stats_ttest::validate(dataset, spec),
        Err(r) => {
            let rows = dataset.rows() as u64;
            let estimate = ResourceEstimate {
                rows,
                columns: 2,
                cells: rows.saturating_mul(2),
            };
            ValidationReport::new(
                vec![ttest_refusal(&r)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                estimate,
            )
        }
    }
}

/// `stats.analyze.independent_t`'s run: the registry again, then
/// `stats_ttest::run`, unchanged.
pub fn ttest_run<B>(
    spec: &OperationSpec<TTestParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<TTestResult, stats_ttest::Halt<B>> {
    ttest_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn ttest_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<TTestParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<TTestResult, stats_ttest::Halt<B>> {
    ttest_entry(registry, spec, build)
        .map_err(|r| stats_ttest::Halt::Refused(ttest_refusal(&r)))?;
    stats_ttest::run(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  AI: a method a model's answer names
 * ------------------------------------------------------------------------ */

/// Whether a Level 1 method that an AI task's answer names may be used: only
/// when its capability is runnable in this build. The app refuses the answer
/// otherwise, with the validator's own `unknown_method` (Phase 4A, item 5).
pub fn method_is_runnable(method: L1MethodId, build: Build) -> bool {
    method_is_runnable_in(&Registry::statistics(), method, build)
}

pub(crate) fn method_is_runnable_in(registry: &Registry, method: L1MethodId, build: Build) -> bool {
    registry
        .entries()
        .iter()
        .find(|e| e.kind == Kind::Verify(method))
        .is_some_and(|e| {
            registry
                .require_runnable(&e.descriptor.id, e.descriptor.version, build)
                .is_ok()
        })
}

#[cfg(test)]
mod tests;

/// The t-test's façade refuses forged specs (Phase 4B-1): an unknown id, a
/// wrong version, and a Planned capability (a test-only table), and runs the
/// real spec exactly as `stats_ttest` does.
#[cfg(test)]
mod ttest_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };
    use crate::stats_ttest::{Halt, Variant};

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [
            ("1.5", "a"),
            ("2.5", "a"),
            ("4", "a"),
            ("3", "b"),
            ("6.5", "b"),
            ("7", "b"),
        ]
        .iter()
        .map(|(y, g)| vec![y.to_string(), g.to_string()])
        .collect();
        build_dataset(
            RawTable::from_rows(vec!["y".into(), "g".into()], rows),
            DataFingerprint::of_bytes(b"facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn forged(d: &Dataset, id: &str, version: u32) -> OperationSpec<TTestParams> {
        let mut s = stats_ttest::spec(
            OperationId(3),
            d,
            TTestParams {
                outcome: Alias(ColumnAlias(1)),
                group: Alias(ColumnAlias(2)),
                levels: None,
                variant: Some(Variant::Welch),
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    fn real() -> Registry {
        Registry::statistics()
    }

    fn go(_: usize) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    #[test]
    fn the_ttest_facade_refuses_an_unknown_id_a_wrong_version_and_a_planned_capability() {
        let d = data();
        let b = Build::Devtools;
        let codes =
            |r: &ValidationReport<TTestCode>| r.errors.iter().map(|e| e.code).collect::<Vec<_>>();
        // Unknown: not in the table at all, and a registered id of another façade.
        for id in ["stats.analyze.welch_t", "stats.describe"] {
            let r = ttest_validate_in(&real(), &forged(&d, id, 1), b, &d);
            assert_eq!(codes(&r), [TTestCode::UnknownCapability], "{id}");
            assert!(!r.may_run());
            assert_eq!(
                ttest_run_in(&real(), &forged(&d, id, 1), b, &d, go).unwrap_err(),
                Halt::Refused(Diagnostic::error(TTestCode::UnknownCapability))
            );
        }
        let v = ttest_validate_in(&real(), &forged(&d, stats_ttest::CAPABILITY_ID, 2), b, &d);
        assert_eq!(codes(&v), [TTestCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Expected),
            Some(&ParamValue::Int(1))
        );
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(2))
        );
        let planned = real().with_lifecycle(stats_ttest::CAPABILITY_ID, Lifecycle::Planned);
        let good = forged(&d, stats_ttest::CAPABILITY_ID, 1);
        for build in [Build::Standard, Build::Devtools] {
            assert_eq!(
                codes(&ttest_validate_in(&planned, &good, build, &d)),
                [TTestCode::UnknownCapability]
            );
            assert_eq!(
                ttest_run_in(&planned, &good, build, &d, go).unwrap_err(),
                Halt::Refused(Diagnostic::error(TTestCode::UnknownCapability))
            );
        }
        // A confirmation on a capability that takes none.
        let mut confirmed = good.clone();
        confirmed.confirmation_refs = vec![ConfirmationId::parse(&"ab".repeat(32)).unwrap()];
        assert_eq!(
            codes(&ttest_validate_in(&real(), &confirmed, b, &d)),
            [TTestCode::UnknownCapability]
        );

        // The real spec: exactly the direct calls.
        assert_eq!(
            ttest_validate_in(&real(), &good, b, &d),
            stats_ttest::validate(&d, &good)
        );
        assert_eq!(
            ttest_run_in(&real(), &good, b, &d, go).unwrap(),
            stats_ttest::compute(&d, &good.parameters).unwrap()
        );
        assert_eq!(
            ttest_validate(&good, Build::Standard, &d),
            stats_ttest::validate(&d, &good)
        );
    }
}
