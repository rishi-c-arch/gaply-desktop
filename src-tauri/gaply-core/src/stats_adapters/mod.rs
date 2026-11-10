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
//! * [`one_sample_validate`] and [`one_sample_run`]
//!   (`stats.analyze.one_sample_t`), [`paired_validate`] and [`paired_run`]
//!   (`stats.analyze.paired_t`), Phase 8D-1: `stats_onesample`'s functions,
//!   the same way, a registry refusal worded `stats.onesample.unknown_capability`
//!   or `stats.onesample.capability_version_mismatch`. Planned in 8D-1, both
//!   are Beta since 8D-2a (decision 265), so the real registry admits them.
//! * [`anova_admit`], [`anova_validate`] and [`anova_run`]
//!   (`stats.analyze.one_way_anova`, Phase 8G-2, decision 318):
//!   `stats_anova::validate` and `stats_anova::run`, the same way, a registry
//!   refusal worded `stats.anova.unknown_capability` or
//!   `stats.anova.capability_version_mismatch`. Planned in 8G, Beta since
//!   8G-2's Part 3 (decision 333), so the real registry admits it.
//! * [`corr_admit`], [`corr_validate`] and [`corr_run`]
//!   (`stats.analyze.correlations`, Phase 8H-2, decision 356):
//!   `stats_corr::validate` and `stats_corr::run`, the same way, a registry
//!   refusal worded `stats.corr.unknown_capability` or
//!   `stats.corr.capability_version_mismatch`. Planned in 8H, Beta since
//!   8H-2's Part 1 (decision 351), so the real registry admits it.
//! * [`alpha_admit`], [`alpha_validate`] and [`alpha_run`]
//!   (`stats.analyze.cronbach_alpha`, Phase 8I-2, decision 381):
//!   `stats_alpha::validate` and `stats_alpha::run`, the same way, a registry
//!   refusal worded `stats.alpha.unknown_capability` or
//!   `stats.alpha.capability_version_mismatch`. Planned in 8I, Beta since
//!   8I-2's Part 2 (decision 377), so the real registry admits it.
//! * [`xtab_admit`], [`xtab_validate`] and [`xtab_run`]
//!   (`stats.analyze.crosstabs`, Phase 8X-1, decision 578):
//!   `stats_xtab::validate` and `stats_xtab::run`, the same way, a registry
//!   refusal worded `stats.xtab.unknown_capability` or
//!   `stats.xtab.capability_version_mismatch`. Planned in 8X-1's Part 2, Beta
//!   since its Part 3 (decision 580), so the real registry admits it.
//! * [`ranks_admit`], [`ranks_validate`] and [`ranks_run`]
//!   (`stats.analyze.independent_ranks`, Phase 8Y-1, decision 605):
//!   `stats_ranks::validate` and `stats_ranks::run`, the same way, a registry
//!   refusal worded `stats.ranks.unknown_capability` or
//!   `stats.ranks.capability_version_mismatch`. Planned in 8Y-1's Part 2, so
//!   the real registry refuses it until Part 3 makes it Beta.
//! * [`prep_runnable`], [`prep_admit`], [`prep_validate`], [`prep_build`],
//!   [`prep_original`] and [`prep_csv`] (`stats.prepare.scale_score`, Phase
//!   8M-3, decision 425): `stats_prep::validate`, `build`, `original` and
//!   `export::prepared_csv`, the same way, for a recipe rather than a spec. A
//!   registry refusal is worded `stats.prep.unknown_capability` or
//!   `stats.prep.capability_version_mismatch`. Beta since 8M-2 (decision 416),
//!   so the real registry admits it.
//!   Since 8M-4 (decisions 433 and 434), [`prep_recipe_bytes`],
//!   [`prep_recipe_from_bytes`] and [`prep_input_hash`], for a project's Save
//!   and rerun: `stats_prep`'s, unchanged.
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
use crate::stats_alpha::{self, AlphaCode, AlphaOutcome, AlphaParams};
use crate::stats_anova::{self, AnovaCode, AnovaOutcome, AnovaParams};
use crate::stats_corr::{self, CorrCode, CorrOutcome, CorrParams};
use crate::stats_desc::{self, DescCode, DescribeParams, DescribeResult, Halt};
use crate::stats_l0::outcome::{check_reading, StatsConsistencyOutcome};
use crate::stats_l0::result::{read_results, ResultReading, TableStructure};
use crate::stats_l1::catalogue::L1MethodId;
use crate::stats_l1::claim::RevisionHash;
use crate::stats_l1::dataset::{DataFingerprint, Dataset, L1IngestRefusal};
use crate::stats_l1::reproduce::{reproduce, ReproductionOutcome};
use crate::stats_l1::run_state::L1RunState;
use crate::stats_l1::token::ConfirmationToken;
use crate::stats_onesample::{
    self, OneSampleCode, OneSampleParams, OneSampleResult, PairedParams, PairedResult,
};
use crate::stats_prep::{self, PrepCode, PrepRefusal, Prepared, Recipe};
use crate::stats_ranks::{self, IndependentRanksOutcome, IndependentRanksParams, RanksCode};
use crate::stats_registry::{Entry, Kind, Registry, CHECK_ID, VERIFY_PREFIX};
use crate::stats_ttest::{self, TTestCode, TTestParams, TTestResult};
use crate::stats_xtab::{self, CrosstabsOutcome, CrosstabsParams, XtabCode};

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
        preparation: None,
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
    admit_check(registry, spec, build)?;
    Ok(check_reading(ex, read_results(ex, tables)))
}

/// `stats.check` on a reading the caller already made from `ex` with
/// `read_results`: the same registry check, then Level 0 on that reading, so
/// a caller that needs the reading itself (Level 1's open, for the document
/// context and the claims) does not read the results twice (the Phase 4
/// evaluation's F5). `read_results` is deterministic, so the outcome equals
/// [`check`]'s for the same extraction and table structure.
pub fn check_with_reading(
    spec: &OperationSpec<()>,
    build: Build,
    ex: &ExtractionResult,
    reading: ResultReading,
) -> Result<StatsConsistencyOutcome, Refusal> {
    admit_check(&Registry::statistics(), spec, build)?;
    Ok(check_reading(ex, reading))
}

/// The registry's answer for a `stats.check` spec: runnable, the check's own
/// kind, and no confirmation.
fn admit_check(registry: &Registry, spec: &OperationSpec<()>, build: Build) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::Check {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
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

/// The registry's answer for a t-test spec, as the registry gives it: the
/// app's `stats_ttest_*` commands return a `capability.*` refusal as it is
/// (Research OS, Phase 4B-2, decision 74), before the façade below, which
/// still checks and words its own refusal for any other caller.
pub fn ttest_admit(spec: &OperationSpec<TTestParams>, build: Build) -> Result<(), Refusal> {
    ttest_entry(&Registry::statistics(), spec, build)
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
 *  stats.analyze.one_sample_t and stats.analyze.paired_t (Phase 8D-1)
 * ------------------------------------------------------------------------ */

/// A registry refusal in the two tests' own codes, as [`ttest_refusal`] does.
fn onesample_refusal(r: &Refusal) -> Diagnostic<OneSampleCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(OneSampleCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(OneSampleCode::UnknownCapability),
    }
}

/// The registry's check for a spec of `kind`, as [`ttest_entry`] does.
fn onesample_entry<P>(
    registry: &Registry,
    spec: &OperationSpec<P>,
    build: Build,
    kind: Kind,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != kind {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// A refused validation report in the two tests' codes.
fn onesample_refused_report(
    r: &Refusal,
    dataset: &Dataset,
    columns: u64,
) -> ValidationReport<OneSampleCode> {
    let rows = dataset.rows() as u64;
    let estimate = ResourceEstimate {
        rows,
        columns,
        cells: rows.saturating_mul(columns),
    };
    ValidationReport::new(
        vec![onesample_refusal(r)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        estimate,
    )
}

/// The registry's answer for a one-sample spec, as the registry gives it, as
/// [`ttest_admit`] is for the t-test.
pub fn one_sample_admit(
    spec: &OperationSpec<OneSampleParams>,
    build: Build,
) -> Result<(), Refusal> {
    onesample_entry(
        &Registry::statistics(),
        spec,
        build,
        Kind::AnalyzeOneSampleT,
    )
}

/// `stats.analyze.one_sample_t`'s validation: the registry first, then
/// `stats_onesample::one_sample_validate`, unchanged.
pub fn one_sample_validate(
    spec: &OperationSpec<OneSampleParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<OneSampleCode> {
    one_sample_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn one_sample_validate_in(
    registry: &Registry,
    spec: &OperationSpec<OneSampleParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<OneSampleCode> {
    match onesample_entry(registry, spec, build, Kind::AnalyzeOneSampleT) {
        Ok(()) => stats_onesample::one_sample_validate(dataset, spec),
        Err(r) => onesample_refused_report(&r, dataset, 1),
    }
}

/// `stats.analyze.one_sample_t`'s run: the registry again, then
/// `stats_onesample::one_sample_run`, unchanged.
pub fn one_sample_run<B>(
    spec: &OperationSpec<OneSampleParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<OneSampleResult, stats_onesample::Halt<B>> {
    one_sample_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn one_sample_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<OneSampleParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<OneSampleResult, stats_onesample::Halt<B>> {
    onesample_entry(registry, spec, build, Kind::AnalyzeOneSampleT)
        .map_err(|r| stats_onesample::Halt::Refused(onesample_refusal(&r)))?;
    stats_onesample::one_sample_run(dataset, &spec.parameters, checkpoint)
}

/// The registry's answer for a paired spec, as the registry gives it.
pub fn paired_admit(spec: &OperationSpec<PairedParams>, build: Build) -> Result<(), Refusal> {
    onesample_entry(&Registry::statistics(), spec, build, Kind::AnalyzePairedT)
}

/// `stats.analyze.paired_t`'s validation: the registry first, then
/// `stats_onesample::paired_validate`, unchanged.
pub fn paired_validate(
    spec: &OperationSpec<PairedParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<OneSampleCode> {
    paired_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn paired_validate_in(
    registry: &Registry,
    spec: &OperationSpec<PairedParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<OneSampleCode> {
    match onesample_entry(registry, spec, build, Kind::AnalyzePairedT) {
        Ok(()) => stats_onesample::paired_validate(dataset, spec),
        Err(r) => onesample_refused_report(&r, dataset, 2),
    }
}

/// `stats.analyze.paired_t`'s run: the registry again, then
/// `stats_onesample::paired_run`, unchanged.
pub fn paired_run<B>(
    spec: &OperationSpec<PairedParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<PairedResult, stats_onesample::Halt<B>> {
    paired_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn paired_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<PairedParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<PairedResult, stats_onesample::Halt<B>> {
    onesample_entry(registry, spec, build, Kind::AnalyzePairedT)
        .map_err(|r| stats_onesample::Halt::Refused(onesample_refusal(&r)))?;
    stats_onesample::paired_run(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  stats.analyze.one_way_anova (Phase 8G-2)
 * ------------------------------------------------------------------------ */

/// A registry refusal in the ANOVA's own codes, as [`ttest_refusal`] does.
fn anova_refusal(r: &Refusal) -> Diagnostic<AnovaCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(AnovaCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(AnovaCode::UnknownCapability),
    }
}

/// The registry's check for an ANOVA spec, as [`ttest_entry`] does: a
/// registered, runnable entry of the ANOVA's kind, and no confirmation.
fn anova_entry(
    registry: &Registry,
    spec: &OperationSpec<AnovaParams>,
    build: Build,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::AnalyzeOneWayAnova {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// The registry's answer for an ANOVA spec, as it gives it, for the app's
/// `stats_anova_*` commands, as [`ttest_admit`] is for the t-test.
pub fn anova_admit(spec: &OperationSpec<AnovaParams>, build: Build) -> Result<(), Refusal> {
    anova_entry(&Registry::statistics(), spec, build)
}

/// `stats.analyze.one_way_anova`'s validation: the registry first, so a spec
/// the registry refuses is `Unsupported` before anything reads the dataset;
/// then `stats_anova::validate`, unchanged.
pub fn anova_validate(
    spec: &OperationSpec<AnovaParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<AnovaCode> {
    anova_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn anova_validate_in(
    registry: &Registry,
    spec: &OperationSpec<AnovaParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<AnovaCode> {
    match anova_entry(registry, spec, build) {
        Ok(()) => stats_anova::validate(dataset, spec),
        Err(r) => {
            let rows = dataset.rows() as u64;
            let estimate = ResourceEstimate {
                rows,
                columns: 2,
                cells: rows.saturating_mul(2),
            };
            ValidationReport::new(
                vec![anova_refusal(&r)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                estimate,
            )
        }
    }
}

/// `stats.analyze.one_way_anova`'s run: the registry again, then
/// `stats_anova::run`, unchanged, with its check points.
pub fn anova_run<B>(
    spec: &OperationSpec<AnovaParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<AnovaOutcome, stats_anova::Halt<B>> {
    anova_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn anova_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<AnovaParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<AnovaOutcome, stats_anova::Halt<B>> {
    anova_entry(registry, spec, build)
        .map_err(|r| stats_anova::Halt::Refused(anova_refusal(&r)))?;
    stats_anova::run(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  stats.analyze.correlations (Phase 8H-2)
 * ------------------------------------------------------------------------ */

/// A registry refusal in the correlations' own codes, as [`anova_refusal`]
/// does.
fn corr_refusal(r: &Refusal) -> Diagnostic<CorrCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(CorrCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(CorrCode::UnknownCapability),
    }
}

/// The registry's check for a correlations spec, as [`anova_entry`] does: a
/// registered, runnable entry of the correlations' kind, and no
/// confirmation.
fn corr_entry(
    registry: &Registry,
    spec: &OperationSpec<CorrParams>,
    build: Build,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::AnalyzeCorrelations {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// The registry's answer for a correlations spec, as it gives it, for the
/// app's `stats_corr_*` commands, as [`anova_admit`] is for the ANOVA.
pub fn corr_admit(spec: &OperationSpec<CorrParams>, build: Build) -> Result<(), Refusal> {
    corr_entry(&Registry::statistics(), spec, build)
}

/// `stats.analyze.correlations`' validation: the registry first, so a spec
/// the registry refuses is `Unsupported` before anything reads the dataset;
/// then `stats_corr::validate`, unchanged, which refuses a spec over the
/// budget (decision 355).
pub fn corr_validate(
    spec: &OperationSpec<CorrParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<CorrCode> {
    corr_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn corr_validate_in(
    registry: &Registry,
    spec: &OperationSpec<CorrParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<CorrCode> {
    match corr_entry(registry, spec, build) {
        Ok(()) => stats_corr::validate(dataset, spec),
        Err(r) => {
            let rows = dataset.rows() as u64;
            let columns = spec.parameters.variables.len() as u64;
            let estimate = ResourceEstimate {
                rows,
                columns,
                cells: rows.saturating_mul(columns),
            };
            ValidationReport::new(
                vec![corr_refusal(&r)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                estimate,
            )
        }
    }
}

/// `stats.analyze.correlations`' run: the registry again, then
/// `stats_corr::run`, unchanged, with its check points.
pub fn corr_run<B>(
    spec: &OperationSpec<CorrParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CorrOutcome, stats_corr::Halt<B>> {
    corr_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn corr_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<CorrParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CorrOutcome, stats_corr::Halt<B>> {
    corr_entry(registry, spec, build).map_err(|r| stats_corr::Halt::Refused(corr_refusal(&r)))?;
    stats_corr::run(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  stats.analyze.cronbach_alpha (Phase 8I-2)
 * ------------------------------------------------------------------------ */

/// A registry refusal in alpha's own codes, as [`corr_refusal`] does.
fn alpha_refusal(r: &Refusal) -> Diagnostic<AlphaCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(AlphaCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(AlphaCode::UnknownCapability),
    }
}

/// The registry's check for an alpha spec, as [`corr_entry`] does: a
/// registered, runnable entry of alpha's kind, and no confirmation.
fn alpha_entry(
    registry: &Registry,
    spec: &OperationSpec<AlphaParams>,
    build: Build,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::AnalyzeCronbachAlpha {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// The registry's answer for an alpha spec, as it gives it, for the app's
/// `stats_alpha_*` commands, as [`corr_admit`] is for the correlations.
pub fn alpha_admit(spec: &OperationSpec<AlphaParams>, build: Build) -> Result<(), Refusal> {
    alpha_entry(&Registry::statistics(), spec, build)
}

/// `stats.analyze.cronbach_alpha`'s validation: the registry first, so a
/// spec the registry refuses is `Unsupported` before anything reads the
/// dataset; then `stats_alpha::validate`, unchanged, which refuses a
/// reversed value outside the scale and too few complete rows (decision
/// 380).
pub fn alpha_validate(
    spec: &OperationSpec<AlphaParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<AlphaCode> {
    alpha_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn alpha_validate_in(
    registry: &Registry,
    spec: &OperationSpec<AlphaParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<AlphaCode> {
    match alpha_entry(registry, spec, build) {
        Ok(()) => stats_alpha::validate(dataset, spec),
        Err(r) => {
            let rows = dataset.rows() as u64;
            let columns = spec.parameters.items.len() as u64;
            let estimate = ResourceEstimate {
                rows,
                columns,
                cells: rows.saturating_mul(columns),
            };
            ValidationReport::new(
                vec![alpha_refusal(&r)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                estimate,
            )
        }
    }
}

/// `stats.analyze.cronbach_alpha`'s run: the registry again, then
/// `stats_alpha::run`, unchanged, with its check points.
pub fn alpha_run<B>(
    spec: &OperationSpec<AlphaParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<AlphaOutcome, stats_alpha::Halt<B>> {
    alpha_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn alpha_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<AlphaParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<AlphaOutcome, stats_alpha::Halt<B>> {
    alpha_entry(registry, spec, build)
        .map_err(|r| stats_alpha::Halt::Refused(alpha_refusal(&r)))?;
    stats_alpha::run(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  stats.analyze.crosstabs (Phase 8X-1)
 * ------------------------------------------------------------------------ */

/// A registry refusal in crosstabs' own codes, as [`anova_refusal`] does.
fn xtab_refusal(r: &Refusal) -> Diagnostic<XtabCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(XtabCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(XtabCode::UnknownCapability),
    }
}

/// The registry's check for a crosstabs spec, as [`anova_entry`] does: a
/// registered, runnable entry of crosstabs' kind, and no confirmation.
fn xtab_entry(
    registry: &Registry,
    spec: &OperationSpec<CrosstabsParams>,
    build: Build,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::AnalyzeCrosstabs {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// The registry's answer for a crosstabs spec, as it gives it, for the app's
/// `stats_xtab_*` commands, as [`anova_admit`] is for the ANOVA.
pub fn xtab_admit(spec: &OperationSpec<CrosstabsParams>, build: Build) -> Result<(), Refusal> {
    xtab_entry(&Registry::statistics(), spec, build)
}

/// `stats.analyze.crosstabs`' validation: the registry first, so a spec the
/// registry refuses is `Unsupported` before anything reads the dataset; then
/// `stats_xtab::validate`, unchanged.
pub fn xtab_validate(
    spec: &OperationSpec<CrosstabsParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<XtabCode> {
    xtab_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn xtab_validate_in(
    registry: &Registry,
    spec: &OperationSpec<CrosstabsParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<XtabCode> {
    match xtab_entry(registry, spec, build) {
        Ok(()) => stats_xtab::validate(dataset, spec),
        Err(r) => {
            let rows = dataset.rows() as u64;
            let estimate = ResourceEstimate {
                rows,
                columns: 2,
                cells: rows.saturating_mul(2),
            };
            ValidationReport::new(
                vec![xtab_refusal(&r)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                estimate,
            )
        }
    }
}

/// `stats.analyze.crosstabs`' run: the registry again, then
/// `stats_xtab::run`, unchanged, with its one check point.
pub fn xtab_run<B>(
    spec: &OperationSpec<CrosstabsParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CrosstabsOutcome, stats_xtab::Halt<B>> {
    xtab_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn xtab_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<CrosstabsParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CrosstabsOutcome, stats_xtab::Halt<B>> {
    xtab_entry(registry, spec, build).map_err(|r| stats_xtab::Halt::Refused(xtab_refusal(&r)))?;
    stats_xtab::run(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  stats.analyze.independent_ranks (Phase 8Y-1)
 * ------------------------------------------------------------------------ */

/// A registry refusal in the rank tests' own codes, as [`xtab_refusal`] does.
fn ranks_refusal(r: &Refusal) -> Diagnostic<RanksCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(RanksCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(RanksCode::UnknownCapability),
    }
}

/// The registry's check for a rank tests spec, as [`xtab_entry`] does: a
/// registered, runnable entry of the rank tests' kind, and no confirmation.
fn ranks_entry(
    registry: &Registry,
    spec: &OperationSpec<IndependentRanksParams>,
    build: Build,
) -> Result<(), Refusal> {
    let entry = runnable(registry, spec, build)?;
    if entry.kind != Kind::AnalyzeIndependentRanks {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    no_confirmation(spec)
}

/// The registry's answer for a rank tests spec, as it gives it, for the
/// app's `stats_ranks_*` commands, as [`xtab_admit`] is for crosstabs.
pub fn ranks_admit(
    spec: &OperationSpec<IndependentRanksParams>,
    build: Build,
) -> Result<(), Refusal> {
    ranks_entry(&Registry::statistics(), spec, build)
}

/// `stats.analyze.independent_ranks`' validation: the registry first, so a
/// spec the registry refuses is `Unsupported` before anything reads the
/// dataset; then `stats_ranks::validate`, unchanged.
pub fn ranks_validate(
    spec: &OperationSpec<IndependentRanksParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<RanksCode> {
    ranks_validate_in(&Registry::statistics(), spec, build, dataset)
}

pub(crate) fn ranks_validate_in(
    registry: &Registry,
    spec: &OperationSpec<IndependentRanksParams>,
    build: Build,
    dataset: &Dataset,
) -> ValidationReport<RanksCode> {
    match ranks_entry(registry, spec, build) {
        Ok(()) => stats_ranks::validate(dataset, spec),
        Err(r) => {
            let rows = dataset.rows() as u64;
            let estimate = ResourceEstimate {
                rows,
                columns: 2,
                cells: rows.saturating_mul(2),
            };
            ValidationReport::new(
                vec![ranks_refusal(&r)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                estimate,
            )
        }
    }
}

/// `stats.analyze.independent_ranks`' run: the registry again, then
/// `stats_ranks::run`, unchanged, with its one check point.
pub fn ranks_run<B>(
    spec: &OperationSpec<IndependentRanksParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<IndependentRanksOutcome, stats_ranks::Halt<B>> {
    ranks_run_in(&Registry::statistics(), spec, build, dataset, checkpoint)
}

pub(crate) fn ranks_run_in<B>(
    registry: &Registry,
    spec: &OperationSpec<IndependentRanksParams>,
    build: Build,
    dataset: &Dataset,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<IndependentRanksOutcome, stats_ranks::Halt<B>> {
    ranks_entry(registry, spec, build)
        .map_err(|r| stats_ranks::Halt::Refused(ranks_refusal(&r)))?;
    stats_ranks::run(dataset, &spec.parameters, checkpoint)
}

/* ------------------------------------------------------------------------ *
 *  stats.prepare.scale_score (Phase 8M-3)
 * ------------------------------------------------------------------------ */

/// A registry refusal in scale scores' own codes, as [`alpha_refusal`] does.
fn prep_refusal(r: &Refusal) -> Diagnostic<PrepCode> {
    match r.code {
        CapabilityCode::VersionMismatch => {
            let int = |k| match r.params.get(&k) {
                Some(ParamValue::Int(n)) => *n,
                _ => 0,
            };
            Diagnostic::error(PrepCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, int(ParamKey::Expected))
                .int(ParamKey::Found, int(ParamKey::Found))
        }
        _ => Diagnostic::error(PrepCode::UnknownCapability),
    }
}

/// The registry's check for this capability's id and version: a registered,
/// runnable entry of this kind. A recipe is no spec: no workspace field and no
/// confirmation to check.
fn prep_entry(
    registry: &Registry,
    id: &CapabilityId,
    version: u32,
    build: Build,
) -> Result<(), Refusal> {
    let entry = registry.require_runnable(id, version, build)?;
    if entry.kind != Kind::PrepareScaleScore {
        return Err(refused(CapabilityCode::UnknownCapability));
    }
    Ok(())
}

/// The registry's answer for a recipe, as it gives it, for the app's
/// `stats_prep_*` commands, as [`alpha_admit`] is for alpha.
pub fn prep_admit(recipe: &Recipe, build: Build) -> Result<(), Refusal> {
    prep_entry(
        &Registry::statistics(),
        &recipe.capability_id,
        recipe.capability_version,
        build,
    )
}

/// The registry's answer for a command that takes no recipe: this build's
/// capability, at this build's version.
pub fn prep_runnable(build: Build) -> Result<(), Refusal> {
    prep_runnable_in(&Registry::statistics(), build)
}

pub(crate) fn prep_runnable_in(registry: &Registry, build: Build) -> Result<(), Refusal> {
    let id = CapabilityId::parse(stats_prep::CAPABILITY_ID)
        .expect("a well-formed Statistics capability id");
    prep_entry(registry, &id, stats_prep::CAPABILITY_VERSION, build)
}

/// `stats.prepare.scale_score`'s validation: the registry first, so a recipe
/// the registry refuses is `Unsupported` before anything reads the dataset;
/// then `stats_prep::validate`, unchanged. `cells_cap` is the caller's.
pub fn prep_validate(
    recipe: &Recipe,
    build: Build,
    dataset: &Dataset,
    cells_cap: u64,
) -> ValidationReport<PrepCode> {
    prep_validate_in(&Registry::statistics(), recipe, build, dataset, cells_cap)
}

/// The report for a recipe the registry refused.
fn prep_refused_report(
    r: &Refusal,
    recipe: &Recipe,
    dataset: &Dataset,
) -> ValidationReport<PrepCode> {
    let rows = dataset.rows() as u64;
    let columns = dataset.schema.columns.len() as u64 + recipe.steps.len() as u64;
    let estimate = ResourceEstimate {
        rows,
        columns,
        cells: rows.saturating_mul(columns),
    };
    ValidationReport::new(
        vec![prep_refusal(r)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        estimate,
    )
}

pub(crate) fn prep_validate_in(
    registry: &Registry,
    recipe: &Recipe,
    build: Build,
    dataset: &Dataset,
    cells_cap: u64,
) -> ValidationReport<PrepCode> {
    match prep_entry(
        registry,
        &recipe.capability_id,
        recipe.capability_version,
        build,
    ) {
        Ok(()) => stats_prep::validate(dataset, recipe, cells_cap),
        Err(r) => prep_refused_report(&r, recipe, dataset),
    }
}

/// `stats.prepare.scale_score`'s build: the registry again, then
/// `stats_prep::build`, unchanged.
pub fn prep_build(
    recipe: &Recipe,
    build: Build,
    dataset: &Dataset,
    cells_cap: u64,
) -> Result<Prepared, PrepRefusal> {
    prep_build_in(&Registry::statistics(), recipe, build, dataset, cells_cap)
}

pub(crate) fn prep_build_in(
    registry: &Registry,
    recipe: &Recipe,
    build: Build,
    dataset: &Dataset,
    cells_cap: u64,
) -> Result<Prepared, PrepRefusal> {
    prep_entry(
        registry,
        &recipe.capability_id,
        recipe.capability_version,
        build,
    )
    .map_err(|r| PrepRefusal::Report(prep_refused_report(&r, recipe, dataset)))?;
    stats_prep::build(dataset, recipe, cells_cap)
}

/// The file's table again, from a prepared one, unchanged from
/// `stats_prep::original`. It reads what is already held and reaches no
/// registry: only a recipe that was admitted can have made a prepared table.
pub fn prep_original(dataset: &Dataset, fingerprint: DataFingerprint) -> Dataset {
    stats_prep::original(dataset, fingerprint)
}

/// The prepared data's CSV bytes, unchanged from
/// `stats_prep::export::prepared_csv`.
pub fn prep_csv(prepared: &Prepared) -> Vec<u8> {
    stats_prep::export::prepared_csv(prepared)
}

/// A recipe's canonical bytes, unchanged from `stats_prep::recipe_bytes`: what
/// a project keeps of it, and what its hash is taken over (decision 433).
pub fn prep_recipe_bytes(recipe: &Recipe) -> Vec<u8> {
    stats_prep::recipe_bytes(recipe)
}

/// A recipe from its canonical bytes, unchanged from
/// `Recipe::from_canonical_bytes`: a project's stored recipe, read back
/// (decision 434). It reaches no registry: [`prep_validate`] does, next.
pub fn prep_recipe_from_bytes(bytes: &[u8]) -> Result<Recipe, PrepCode> {
    Recipe::from_canonical_bytes(bytes)
}

/// A prepared input's SHA-256, unchanged from `stats_prep::prepared_input_hash`:
/// what a Save compares a run's spec with (decision 433).
pub fn prep_input_hash(source_sha256_hex: &str, recipe_sha256_hex: &str) -> String {
    stats_prep::prepared_input_hash(source_sha256_hex, recipe_sha256_hex)
}

/* ------------------------------------------------------------------------ *
 *  AI: a method a model's answer names
 * ------------------------------------------------------------------------ */

/// Whether a Level 1 method that an AI task's answer names may be used: only
/// when its capability is registered in `registry` and runnable in this
/// build. The app refuses the answer otherwise, with the validator's own
/// `unknown_method` (Phase 4A, item 5). The app passes
/// `Registry::statistics()`; a test passes a table with the method Planned or
/// absent (decision 96, the Phase 4 evaluation's F1).
pub fn method_is_runnable_in(registry: &Registry, method: L1MethodId, build: Build) -> bool {
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

/// The one-sample and paired façades (Phase 8D-1): the real registry admits
/// both in both builds, since they are Beta (8D-2a, decision 265); with a
/// test-only table where they are Beta, each refuses an id of another façade, a wrong version, a
/// Planned table and a confirmation, and runs the real spec exactly as
/// `stats_onesample` does.
#[cfg(test)]
mod onesample_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };
    use crate::stats_onesample::{Halt, ONE_SAMPLE_ID, PAIRED_ID};

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [
            ("1.5", "2"),
            ("2.5", "2.25"),
            ("4", "3"),
            ("3", "3.5"),
            ("6.5", "5"),
        ]
        .iter()
        .map(|(a, b)| vec![a.to_string(), b.to_string()])
        .collect();
        build_dataset(
            RawTable::from_rows(vec!["a".into(), "b".into()], rows),
            DataFingerprint::of_bytes(b"onesample facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn one(d: &Dataset, id: &str, version: u32) -> OperationSpec<OneSampleParams> {
        let mut s = stats_onesample::one_sample_spec(
            OperationId(4),
            d,
            OneSampleParams {
                outcome: Alias(ColumnAlias(1)),
                test_value: Some(3.0),
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    fn paired(d: &Dataset, id: &str, version: u32) -> OperationSpec<PairedParams> {
        let mut s = stats_onesample::paired_spec(
            OperationId(5),
            d,
            PairedParams {
                first: Alias(ColumnAlias(1)),
                second: Alias(ColumnAlias(2)),
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    /// The real table with both Beta: a table the real one does not have.
    fn beta() -> Registry {
        Registry::statistics()
            .with_lifecycle(ONE_SAMPLE_ID, Lifecycle::Beta)
            .with_lifecycle(PAIRED_ID, Lifecycle::Beta)
    }

    fn go(_: usize) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn codes(r: &ValidationReport<OneSampleCode>) -> Vec<OneSampleCode> {
        r.errors.iter().map(|e| e.code).collect()
    }

    fn unknown() -> Halt<()> {
        Halt::Refused(Diagnostic::error(OneSampleCode::UnknownCapability))
    }

    #[test]
    fn the_real_registry_admits_both_in_both_builds_because_they_are_beta() {
        let d = data();
        let (o, p) = (one(&d, ONE_SAMPLE_ID, 1), paired(&d, PAIRED_ID, 1));
        for build in [Build::Standard, Build::Devtools] {
            for r in [one_sample_admit(&o, build), paired_admit(&p, build)] {
                r.unwrap_or_else(|r| panic!("Beta is admitted in {build:?}: {r:?}"));
            }
            assert!(one_sample_validate(&o, build, &d).may_run(), "{build:?}");
            assert!(paired_validate(&p, build, &d).may_run(), "{build:?}");
            assert_eq!(
                one_sample_run(&o, build, &d, go).ok(),
                crate::stats_onesample::one_sample_compute(&d, &o.parameters).ok(),
                "{build:?}: the module's own result"
            );
            assert_eq!(
                paired_run(&p, build, &d, go).ok(),
                crate::stats_onesample::paired_compute(&d, &p.parameters).ok(),
                "{build:?}: the module's own result"
            );
            assert!(one_sample_run(&o, build, &d, go).is_ok(), "{build:?}");
            assert!(paired_run(&p, build, &d, go).is_ok(), "{build:?}");
        }
    }

    #[test]
    fn with_both_beta_each_facade_refuses_forged_specs_and_runs_the_real_one_as_the_module_does() {
        let d = data();
        let b = Build::Devtools;
        // Another façade's id, the other test's id included.
        for id in ["stats.describe", PAIRED_ID] {
            let s = one(&d, id, 1);
            assert_eq!(
                codes(&one_sample_validate_in(&beta(), &s, b, &d)),
                [OneSampleCode::UnknownCapability],
                "{id}"
            );
            assert_eq!(
                one_sample_run_in(&beta(), &s, b, &d, go).unwrap_err(),
                unknown(),
                "{id}"
            );
        }
        for id in ["stats.describe", ONE_SAMPLE_ID] {
            let s = paired(&d, id, 1);
            assert_eq!(
                codes(&paired_validate_in(&beta(), &s, b, &d)),
                [OneSampleCode::UnknownCapability],
                "{id}"
            );
            assert_eq!(
                paired_run_in(&beta(), &s, b, &d, go).unwrap_err(),
                unknown(),
                "{id}"
            );
        }
        // A wrong version.
        let v = one_sample_validate_in(&beta(), &one(&d, ONE_SAMPLE_ID, 2), b, &d);
        assert_eq!(codes(&v), [OneSampleCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Expected),
            Some(&ParamValue::Int(1))
        );
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(2))
        );
        let v = paired_validate_in(&beta(), &paired(&d, PAIRED_ID, 2), b, &d);
        assert_eq!(codes(&v), [OneSampleCode::CapabilityVersionMismatch]);
        // A Planned table: the real one, with both set back to Planned (they
        // are Beta since 8D-2a, decision 265).
        let (good_one, good_paired) = (one(&d, ONE_SAMPLE_ID, 1), paired(&d, PAIRED_ID, 1));
        let planned = Registry::statistics()
            .with_lifecycle(ONE_SAMPLE_ID, Lifecycle::Planned)
            .with_lifecycle(PAIRED_ID, Lifecycle::Planned);
        assert_eq!(
            codes(&one_sample_validate_in(&planned, &good_one, b, &d)),
            [OneSampleCode::UnknownCapability]
        );
        assert_eq!(
            codes(&paired_validate_in(&planned, &good_paired, b, &d)),
            [OneSampleCode::UnknownCapability]
        );
        // A confirmation on a capability that takes none.
        let confirmation = vec![ConfirmationId::parse(&"cd".repeat(32)).unwrap()];
        let mut c = good_one.clone();
        c.confirmation_refs = confirmation.clone();
        assert_eq!(
            codes(&one_sample_validate_in(&beta(), &c, b, &d)),
            [OneSampleCode::UnknownCapability]
        );
        let mut c = good_paired.clone();
        c.confirmation_refs = confirmation;
        assert_eq!(
            codes(&paired_validate_in(&beta(), &c, b, &d)),
            [OneSampleCode::UnknownCapability]
        );

        // The real specs, in a table where they may run: exactly the direct calls.
        assert_eq!(
            one_sample_validate_in(&beta(), &good_one, b, &d),
            stats_onesample::one_sample_validate(&d, &good_one)
        );
        assert_eq!(
            one_sample_run_in(&beta(), &good_one, b, &d, go).unwrap(),
            stats_onesample::one_sample_compute(&d, &good_one.parameters).unwrap()
        );
        assert_eq!(
            paired_validate_in(&beta(), &good_paired, b, &d),
            stats_onesample::paired_validate(&d, &good_paired)
        );
        assert_eq!(
            paired_run_in(&beta(), &good_paired, b, &d, go).unwrap(),
            stats_onesample::paired_compute(&d, &good_paired.parameters).unwrap()
        );
        for build in [Build::Standard, Build::Devtools] {
            assert!(one_sample_validate_in(&beta(), &good_one, build, &d).may_run());
            assert!(paired_validate_in(&beta(), &good_paired, build, &d).may_run());
        }
    }
}

/// Scale scores' façade (Research OS, Phase 8M-3, decision 425): the real
/// registry admits a recipe in both builds, since it is Beta; it refuses an id
/// of another façade, a wrong version and a Planned table, each in its own
/// codes, and gives the module's own answers for a real recipe.
#[cfg(test)]
mod prep_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };
    use crate::stats_prep::{MissingRule, RecipeStep, ScaleScoreStep, Statistic};

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [("1", "2"), ("2", "4"), ("3", "5")]
            .iter()
            .map(|(a, b)| vec![a.to_string(), b.to_string()])
            .collect();
        build_dataset(
            RawTable::from_rows(vec!["a".into(), "b".into()], rows),
            DataFingerprint::of_bytes(b"prep facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn recipe() -> Recipe {
        Recipe {
            schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
            capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
            capability_version: stats_prep::CAPABILITY_VERSION,
            engine_version: stats_prep::ENGINE_VERSION,
            source_columns: 2,
            steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
                alias: Alias(ColumnAlias(3)),
                name: "total".into(),
                items: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(2))],
                reversed: Vec::new(),
                scale: None,
                statistic: Some(Statistic::Sum),
                rule: Some(MissingRule::EveryItem),
                missing_codes: Vec::new(),
            })],
        }
    }

    const CAP: u64 = 1_000;

    fn codes(r: &ValidationReport<PrepCode>) -> Vec<PrepCode> {
        r.errors.iter().map(|e| e.code).collect()
    }

    #[test]
    fn the_real_registry_admits_a_recipe_in_both_builds_because_it_is_beta() {
        let (d, r) = (data(), recipe());
        for build in [Build::Standard, Build::Devtools] {
            prep_runnable(build).unwrap_or_else(|e| panic!("{build:?}: {e:?}"));
            prep_admit(&r, build).unwrap_or_else(|e| panic!("{build:?}: {e:?}"));
            assert!(prep_validate(&r, build, &d, CAP).may_run(), "{build:?}");
            let built = prep_build(&r, build, &d, CAP).expect("it builds");
            assert_eq!(
                built.dataset,
                stats_prep::build(&d, &r, CAP).unwrap().dataset,
                "{build:?}: the module's own table"
            );
        }
    }

    #[test]
    fn the_recipes_bytes_its_parse_and_the_input_hash_are_the_modules_own() {
        let r = recipe();
        let bytes = prep_recipe_bytes(&r);
        assert_eq!(bytes, stats_prep::recipe_bytes(&r));
        assert_eq!(prep_recipe_from_bytes(&bytes).unwrap(), r);
        let mut spaced = b" ".to_vec();
        spaced.extend_from_slice(&bytes);
        assert_eq!(
            prep_recipe_from_bytes(&spaced),
            Err(PrepCode::RecipeUnreadable)
        );
        assert_eq!(
            prep_input_hash(&"a".repeat(64), &"b".repeat(64)),
            stats_prep::prepared_input_hash(&"a".repeat(64), &"b".repeat(64))
        );
    }

    #[test]
    fn the_facade_refuses_another_id_a_wrong_version_and_a_planned_table() {
        let (d, b) = (data(), Build::Devtools);
        let real = Registry::statistics();
        let mut other = recipe();
        other.capability_id = CapabilityId::parse("stats.describe").unwrap();
        assert_eq!(
            codes(&prep_validate_in(&real, &other, b, &d, CAP)),
            [PrepCode::UnknownCapability]
        );
        assert!(prep_build_in(&real, &other, b, &d, CAP).is_err());
        assert_eq!(
            prep_admit(&other, b).unwrap_err().code,
            CapabilityCode::UnknownCapability
        );

        let mut stale = recipe();
        stale.capability_version = 9;
        let v = prep_validate_in(&real, &stale, b, &d, CAP);
        assert_eq!(codes(&v), [PrepCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(9))
        );
        assert!(prep_build_in(&real, &stale, b, &d, CAP).is_err());

        let planned =
            Registry::statistics().with_lifecycle(stats_prep::CAPABILITY_ID, Lifecycle::Planned);
        for build in [Build::Standard, Build::Devtools] {
            assert_eq!(
                codes(&prep_validate_in(&planned, &recipe(), build, &d, CAP)),
                [PrepCode::UnknownCapability],
                "{build:?}"
            );
            assert!(prep_build_in(&planned, &recipe(), build, &d, CAP).is_err());
            assert!(prep_runnable_in(&planned, build).is_err(), "{build:?}");
        }
    }

    #[test]
    fn the_real_recipe_gets_exactly_the_modules_answers() {
        let (d, r, b) = (data(), recipe(), Build::Devtools);
        assert_eq!(
            prep_validate_in(&Registry::statistics(), &r, b, &d, CAP),
            stats_prep::validate(&d, &r, CAP)
        );
        let built = prep_build(&r, b, &d, CAP).unwrap();
        assert_eq!(prep_csv(&built), stats_prep::export::prepared_csv(&built));
        let fingerprint = d.schema.fingerprint.clone();
        assert_eq!(prep_original(&built.dataset, fingerprint), d);
    }
}

/// The one-way ANOVA's façade (Research OS, Phase 8G-2, decisions 318 and
/// 333): the real registry admits it in both builds, since it is Beta; it
/// refuses an id of another façade, a wrong version, a Planned table and a
/// confirmation, each in its own codes, and runs the real spec exactly as
/// `stats_anova` does.
#[cfg(test)]
mod anova_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_anova::{AnovaParams, Halt, PostHoc};
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [
            ("1.5", "a"),
            ("2.5", "a"),
            ("4", "a"),
            ("3", "b"),
            ("6.5", "b"),
            ("7", "b"),
            ("8", "c"),
            ("9.25", "c"),
            ("12", "c"),
        ]
        .iter()
        .map(|(y, g)| vec![y.to_string(), g.to_string()])
        .collect();
        build_dataset(
            RawTable::from_rows(vec!["y".into(), "g".into()], rows),
            DataFingerprint::of_bytes(b"anova facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn forged(d: &Dataset, id: &str, version: u32) -> OperationSpec<AnovaParams> {
        let mut s = stats_anova::spec(
            OperationId(6),
            d,
            AnovaParams {
                outcome: Alias(ColumnAlias(1)),
                group: Alias(ColumnAlias(2)),
                levels: None,
                post_hoc: vec![PostHoc::Tukey, PostHoc::GamesHowell],
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    fn go(_: usize) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn codes(r: &ValidationReport<AnovaCode>) -> Vec<AnovaCode> {
        r.errors.iter().map(|e| e.code).collect()
    }

    fn unknown() -> Halt<()> {
        Halt::Refused(Diagnostic::error(AnovaCode::UnknownCapability))
    }

    #[test]
    fn the_real_registry_admits_the_anova_in_both_builds_because_it_is_beta() {
        let d = data();
        let s = forged(&d, stats_anova::CAPABILITY_ID, 1);
        for build in [Build::Standard, Build::Devtools] {
            anova_admit(&s, build)
                .unwrap_or_else(|r| panic!("Beta is admitted in {build:?}: {r:?}"));
            assert!(anova_validate(&s, build, &d).may_run(), "{build:?}");
            assert_eq!(
                anova_run(&s, build, &d, go).ok(),
                stats_anova::compute(&d, &s.parameters).ok(),
                "{build:?}: the module's own result"
            );
            assert!(anova_run(&s, build, &d, go).is_ok(), "{build:?}");
        }
    }

    #[test]
    fn the_anova_facade_refuses_forged_specs_and_runs_the_real_one_as_the_module_does() {
        let d = data();
        let b = Build::Devtools;
        let real = Registry::statistics();
        // Not in the table at all, and registered ids of other façades.
        for id in [
            "stats.analyze.two_way_anova",
            "stats.describe",
            "stats.analyze.independent_t",
            "stats.verify.one_way_anova",
        ] {
            let s = forged(&d, id, 1);
            assert_eq!(
                codes(&anova_validate_in(&real, &s, b, &d)),
                [AnovaCode::UnknownCapability],
                "{id}"
            );
            assert!(!anova_validate_in(&real, &s, b, &d).may_run(), "{id}");
            assert_eq!(
                anova_run_in(&real, &s, b, &d, go).unwrap_err(),
                unknown(),
                "{id}"
            );
            assert!(anova_admit(&s, b).is_err(), "{id}");
        }
        // A wrong version, in the ANOVA's own codes with its numbers.
        let v = anova_validate_in(&real, &forged(&d, stats_anova::CAPABILITY_ID, 2), b, &d);
        assert_eq!(codes(&v), [AnovaCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Expected),
            Some(&ParamValue::Int(1))
        );
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(2))
        );
        // A Planned table: the real one, set back to Planned, as 8G had it.
        let good = forged(&d, stats_anova::CAPABILITY_ID, 1);
        let planned =
            Registry::statistics().with_lifecycle(stats_anova::CAPABILITY_ID, Lifecycle::Planned);
        for build in [Build::Standard, Build::Devtools] {
            assert_eq!(
                codes(&anova_validate_in(&planned, &good, build, &d)),
                [AnovaCode::UnknownCapability]
            );
            assert_eq!(
                anova_run_in(&planned, &good, build, &d, go).unwrap_err(),
                unknown()
            );
        }
        // A confirmation on a capability that takes none.
        let mut c = good.clone();
        c.confirmation_refs = vec![ConfirmationId::parse(&"ef".repeat(32)).unwrap()];
        assert_eq!(
            codes(&anova_validate_in(&real, &c, b, &d)),
            [AnovaCode::UnknownCapability]
        );

        // The real spec: exactly the direct calls.
        assert_eq!(
            anova_validate_in(&real, &good, b, &d),
            stats_anova::validate(&d, &good)
        );
        assert_eq!(
            anova_run_in(&real, &good, b, &d, go).unwrap(),
            stats_anova::compute(&d, &good.parameters).unwrap()
        );
    }
}

/// The correlations' façade (Research OS, Phase 8H-2, decisions 351 and
/// 356): the real registry admits it in both builds, since it is Beta; it
/// refuses an id of another façade, a wrong version, a Planned table and a
/// confirmation, each in its own codes, and runs the real spec exactly as
/// `stats_corr` does.
#[cfg(test)]
mod corr_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_corr::{Coefficient, CorrParams, Halt, MissingHandling};
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [
            ("1.5", "2", "9"),
            ("2.5", "1", "7.5"),
            ("4", "3.5", "8"),
            ("3", "5", "4"),
            ("6.5", "4", "3"),
            ("7", "8", "2.5"),
        ]
        .iter()
        .map(|(a, b, c)| vec![a.to_string(), b.to_string(), c.to_string()])
        .collect();
        build_dataset(
            RawTable::from_rows(vec!["a".into(), "b".into(), "c".into()], rows),
            DataFingerprint::of_bytes(b"corr facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn forged(d: &Dataset, id: &str, version: u32) -> OperationSpec<CorrParams> {
        let mut s = stats_corr::spec(
            OperationId(8),
            d,
            CorrParams {
                variables: (1..=3).map(|n| Alias(ColumnAlias(n))).collect(),
                coefficients: vec![
                    Coefficient::Pearson,
                    Coefficient::Spearman,
                    Coefficient::Kendall,
                ],
                missing: MissingHandling::Pairwise,
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    fn go(_: usize) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn codes(r: &ValidationReport<CorrCode>) -> Vec<CorrCode> {
        r.errors.iter().map(|e| e.code).collect()
    }

    fn unknown() -> Halt<()> {
        Halt::Refused(Diagnostic::error(CorrCode::UnknownCapability))
    }

    #[test]
    fn the_real_registry_admits_the_correlations_in_both_builds_because_they_are_beta() {
        let d = data();
        let s = forged(&d, stats_corr::CAPABILITY_ID, 1);
        for build in [Build::Standard, Build::Devtools] {
            corr_admit(&s, build)
                .unwrap_or_else(|r| panic!("Beta is admitted in {build:?}: {r:?}"));
            assert!(corr_validate(&s, build, &d).may_run(), "{build:?}");
            assert_eq!(
                corr_run(&s, build, &d, go).ok(),
                stats_corr::compute(&d, &s.parameters).ok(),
                "{build:?}: the module's own result"
            );
            assert!(corr_run(&s, build, &d, go).is_ok(), "{build:?}");
        }
    }

    #[test]
    fn the_corr_facade_refuses_forged_specs_and_runs_the_real_one_as_the_module_does() {
        let d = data();
        let b = Build::Devtools;
        let real = Registry::statistics();
        // Not in the table at all, and registered ids of other façades.
        for id in [
            "stats.analyze.partial_correlations",
            "stats.describe",
            "stats.analyze.one_way_anova",
            "stats.verify.pearson",
            "stats.verify.spearman",
        ] {
            let s = forged(&d, id, 1);
            assert_eq!(
                codes(&corr_validate_in(&real, &s, b, &d)),
                [CorrCode::UnknownCapability],
                "{id}"
            );
            assert!(!corr_validate_in(&real, &s, b, &d).may_run(), "{id}");
            assert_eq!(
                corr_run_in(&real, &s, b, &d, go).unwrap_err(),
                unknown(),
                "{id}"
            );
            assert!(corr_admit(&s, b).is_err(), "{id}");
        }
        // A wrong version, in the correlations' own codes with its numbers.
        let v = corr_validate_in(&real, &forged(&d, stats_corr::CAPABILITY_ID, 2), b, &d);
        assert_eq!(codes(&v), [CorrCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Expected),
            Some(&ParamValue::Int(1))
        );
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(2))
        );
        // A Planned table: the real one, set back to Planned, as 8H had it.
        let good = forged(&d, stats_corr::CAPABILITY_ID, 1);
        let planned =
            Registry::statistics().with_lifecycle(stats_corr::CAPABILITY_ID, Lifecycle::Planned);
        for build in [Build::Standard, Build::Devtools] {
            assert_eq!(
                codes(&corr_validate_in(&planned, &good, build, &d)),
                [CorrCode::UnknownCapability]
            );
            assert_eq!(
                corr_run_in(&planned, &good, build, &d, go).unwrap_err(),
                unknown()
            );
        }
        // A confirmation on a capability that takes none.
        let mut c = good.clone();
        c.confirmation_refs = vec![ConfirmationId::parse(&"ef".repeat(32)).unwrap()];
        assert_eq!(
            codes(&corr_validate_in(&real, &c, b, &d)),
            [CorrCode::UnknownCapability]
        );

        // The real spec: exactly the direct calls.
        assert_eq!(
            corr_validate_in(&real, &good, b, &d),
            stats_corr::validate(&d, &good)
        );
        assert_eq!(
            corr_run_in(&real, &good, b, &d, go).unwrap(),
            stats_corr::compute(&d, &good.parameters).unwrap()
        );
    }
}

/// Cronbach's alpha's façade (Research OS, Phase 8I-2, decisions 377 and
/// 381): the real registry admits it in both builds, since it is Beta; it
/// refuses an id of another façade, a wrong version, a Planned table and a
/// confirmation, each in its own codes, and runs the real spec exactly as
/// `stats_alpha` does.
#[cfg(test)]
mod alpha_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_alpha::{AlphaParams, Halt, ScaleEnds};
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [
            ("1", "5", "2"),
            ("2", "4", "2"),
            ("3", "3", "4"),
            ("4", "2", "3"),
            ("5", "1", "5"),
            ("2", "5", "1"),
        ]
        .iter()
        .map(|(a, b, c)| vec![a.to_string(), b.to_string(), c.to_string()])
        .collect();
        build_dataset(
            RawTable::from_rows(vec!["a".into(), "b".into(), "c".into()], rows),
            DataFingerprint::of_bytes(b"alpha facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn forged(d: &Dataset, id: &str, version: u32) -> OperationSpec<AlphaParams> {
        let mut s = stats_alpha::spec(
            OperationId(8),
            d,
            AlphaParams {
                items: (1..=3).map(|n| Alias(ColumnAlias(n))).collect(),
                reversed: vec![Alias(ColumnAlias(2))],
                scale: Some(ScaleEnds { min: 1.0, max: 5.0 }),
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    fn go(_: usize) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn codes(r: &ValidationReport<AlphaCode>) -> Vec<AlphaCode> {
        r.errors.iter().map(|e| e.code).collect()
    }

    fn unknown() -> Halt<()> {
        Halt::Refused(Diagnostic::error(AlphaCode::UnknownCapability))
    }

    #[test]
    fn the_real_registry_admits_cronbach_alpha_in_both_builds_because_it_is_beta() {
        let d = data();
        let s = forged(&d, stats_alpha::CAPABILITY_ID, 1);
        for build in [Build::Standard, Build::Devtools] {
            alpha_admit(&s, build)
                .unwrap_or_else(|r| panic!("Beta is admitted in {build:?}: {r:?}"));
            assert!(alpha_validate(&s, build, &d).may_run(), "{build:?}");
            assert_eq!(
                alpha_run(&s, build, &d, go).ok(),
                stats_alpha::compute(&d, &s.parameters).ok(),
                "{build:?}: the module's own result"
            );
            assert!(alpha_run(&s, build, &d, go).is_ok(), "{build:?}");
        }
    }

    #[test]
    fn the_alpha_facade_refuses_forged_specs_and_runs_the_real_one_as_the_module_does() {
        let d = data();
        let b = Build::Devtools;
        let real = Registry::statistics();
        // Not in the table at all, and registered ids of other façades.
        for id in [
            "stats.analyze.mcdonald_omega",
            "stats.describe",
            "stats.analyze.correlations",
            "stats.analyze.one_way_anova",
            "stats.verify.pearson",
        ] {
            let s = forged(&d, id, 1);
            assert_eq!(
                codes(&alpha_validate_in(&real, &s, b, &d)),
                [AlphaCode::UnknownCapability],
                "{id}"
            );
            assert!(!alpha_validate_in(&real, &s, b, &d).may_run(), "{id}");
            assert_eq!(
                alpha_run_in(&real, &s, b, &d, go).unwrap_err(),
                unknown(),
                "{id}"
            );
            assert!(alpha_admit(&s, b).is_err(), "{id}");
        }
        // A wrong version, in alpha's own codes with its numbers.
        let v = alpha_validate_in(&real, &forged(&d, stats_alpha::CAPABILITY_ID, 2), b, &d);
        assert_eq!(codes(&v), [AlphaCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Expected),
            Some(&ParamValue::Int(1))
        );
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(2))
        );
        // A Planned table: the real one, set back to Planned, as 8I had it.
        let good = forged(&d, stats_alpha::CAPABILITY_ID, 1);
        let planned =
            Registry::statistics().with_lifecycle(stats_alpha::CAPABILITY_ID, Lifecycle::Planned);
        for build in [Build::Standard, Build::Devtools] {
            assert_eq!(
                codes(&alpha_validate_in(&planned, &good, build, &d)),
                [AlphaCode::UnknownCapability]
            );
            assert_eq!(
                alpha_run_in(&planned, &good, build, &d, go).unwrap_err(),
                unknown()
            );
        }
        // A confirmation on a capability that takes none.
        let mut c = good.clone();
        c.confirmation_refs = vec![ConfirmationId::parse(&"ef".repeat(32)).unwrap()];
        assert_eq!(
            codes(&alpha_validate_in(&real, &c, b, &d)),
            [AlphaCode::UnknownCapability]
        );

        // The real spec: exactly the direct calls.
        assert_eq!(
            alpha_validate_in(&real, &good, b, &d),
            stats_alpha::validate(&d, &good)
        );
        assert_eq!(
            alpha_run_in(&real, &good, b, &d, go).unwrap(),
            stats_alpha::compute(&d, &good.parameters).unwrap()
        );
    }
}

/// Crosstabs' façade (Research OS, Phase 8X-1, decisions 578 and 580): the
/// real registry admits it in both builds, since it is Beta; it refuses an
/// id of another façade, a wrong version, a Planned table and a
/// confirmation, each in its own codes, and runs the real spec exactly as
/// `stats_xtab` does.
#[cfg(test)]
mod xtab_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };
    use crate::stats_xtab::{CrosstabsParams, Halt};

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [
            ("a", "x"),
            ("a", "x"),
            ("a", "y"),
            ("b", "y"),
            ("b", "y"),
            ("b", "x"),
            ("b", "y"),
            ("a", "x"),
        ]
        .iter()
        .map(|(r, c)| vec![r.to_string(), c.to_string()])
        .collect();
        build_dataset(
            RawTable::from_rows(vec!["r".into(), "c".into()], rows),
            DataFingerprint::of_bytes(b"xtab facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn forged(d: &Dataset, id: &str, version: u32) -> OperationSpec<CrosstabsParams> {
        let mut s = stats_xtab::spec(
            OperationId(7),
            d,
            CrosstabsParams {
                rows: Alias(ColumnAlias(1)),
                columns: Alias(ColumnAlias(2)),
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    fn go(_: usize) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn codes(r: &ValidationReport<XtabCode>) -> Vec<XtabCode> {
        r.errors.iter().map(|e| e.code).collect()
    }

    fn unknown() -> Halt<()> {
        Halt::Refused(Diagnostic::error(XtabCode::UnknownCapability))
    }

    #[test]
    fn the_real_registry_admits_crosstabs_in_both_builds_because_it_is_beta() {
        let d = data();
        let s = forged(&d, stats_xtab::CAPABILITY_ID, 1);
        for build in [Build::Standard, Build::Devtools] {
            xtab_admit(&s, build)
                .unwrap_or_else(|r| panic!("Beta is admitted in {build:?}: {r:?}"));
            assert!(xtab_validate(&s, build, &d).may_run(), "{build:?}");
            assert_eq!(
                xtab_run(&s, build, &d, go).ok(),
                stats_xtab::compute(&d, &s.parameters).ok(),
                "{build:?}: the module's own result"
            );
            assert!(xtab_run(&s, build, &d, go).is_ok(), "{build:?}");
        }
    }

    #[test]
    fn the_xtab_facade_refuses_forged_specs_and_runs_the_real_one_as_the_module_does() {
        let d = data();
        let b = Build::Devtools;
        let real = Registry::statistics();
        // Not in the table at all, and registered ids of other façades.
        for id in [
            "stats.analyze.crosstabs_layers",
            "stats.describe",
            "stats.analyze.one_way_anova",
            "stats.verify.chi_square_independence",
        ] {
            let s = forged(&d, id, 1);
            assert_eq!(
                codes(&xtab_validate_in(&real, &s, b, &d)),
                [XtabCode::UnknownCapability],
                "{id}"
            );
            assert!(!xtab_validate_in(&real, &s, b, &d).may_run(), "{id}");
            assert_eq!(xtab_run_in(&real, &s, b, &d, go).unwrap_err(), unknown(), "{id}");
            assert!(xtab_admit(&s, b).is_err(), "{id}");
        }
        // A wrong version, in crosstabs' own codes with its numbers.
        let v = xtab_validate_in(&real, &forged(&d, stats_xtab::CAPABILITY_ID, 2), b, &d);
        assert_eq!(codes(&v), [XtabCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Expected),
            Some(&ParamValue::Int(1))
        );
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(2))
        );
        // A Planned table: the real one, set back to Planned, as Part 2 had it.
        let good = forged(&d, stats_xtab::CAPABILITY_ID, 1);
        let planned =
            Registry::statistics().with_lifecycle(stats_xtab::CAPABILITY_ID, Lifecycle::Planned);
        for build in [Build::Standard, Build::Devtools] {
            assert_eq!(
                codes(&xtab_validate_in(&planned, &good, build, &d)),
                [XtabCode::UnknownCapability]
            );
            assert_eq!(
                xtab_run_in(&planned, &good, build, &d, go).unwrap_err(),
                unknown()
            );
        }
        // A confirmation on a capability that takes none.
        let mut c = good.clone();
        c.confirmation_refs = vec![ConfirmationId::parse(&"ef".repeat(32)).unwrap()];
        assert_eq!(
            codes(&xtab_validate_in(&real, &c, b, &d)),
            [XtabCode::UnknownCapability]
        );
        // The real spec: exactly the direct calls.
        assert_eq!(
            xtab_validate_in(&real, &good, b, &d),
            stats_xtab::validate(&d, &good)
        );
        assert_eq!(
            xtab_run_in(&real, &good, b, &d, go).unwrap(),
            stats_xtab::compute(&d, &good.parameters).unwrap()
        );
    }
}

#[cfg(test)]
mod ranks_facade_tests {
    use super::*;
    use crate::contracts::capability::Lifecycle;
    use crate::stats_desc::Alias;
    use crate::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };
    use crate::stats_ranks::{Halt, IndependentRanksParams};

    fn data() -> Dataset {
        let rows: Vec<Vec<String>> = [
            ("1.5", "a"),
            ("2.5", "b"),
            ("3.5", "a"),
            ("4.5", "b"),
            ("5.5", "b"),
            ("0.5", "a"),
            ("6.5", "b"),
            ("7.5", "c"),
            ("8.5", "c"),
        ]
        .iter()
        .map(|(y, g)| vec![y.to_string(), g.to_string()])
        .collect();
        build_dataset(
            RawTable::from_rows(vec!["y".into(), "g".into()], rows),
            DataFingerprint::of_bytes(b"ranks facade", "test", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    fn forged(d: &Dataset, id: &str, version: u32) -> OperationSpec<IndependentRanksParams> {
        let mut s = stats_ranks::spec(
            OperationId(7),
            d,
            IndependentRanksParams {
                outcome: Alias(ColumnAlias(1)),
                group: Alias(ColumnAlias(2)),
                levels: None,
                missing_codes: Vec::new(),
            },
        );
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s
    }

    fn go(_: usize) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn codes(r: &ValidationReport<RanksCode>) -> Vec<RanksCode> {
        r.errors.iter().map(|e| e.code).collect()
    }

    fn unknown() -> Halt<()> {
        Halt::Refused(Diagnostic::error(RanksCode::UnknownCapability))
    }

    #[test]
    fn the_real_registry_admits_the_rank_tests_in_both_builds_because_they_are_beta() {
        let d = data();
        let s = forged(&d, stats_ranks::CAPABILITY_ID, 1);
        for build in [Build::Standard, Build::Devtools] {
            ranks_admit(&s, build)
                .unwrap_or_else(|r| panic!("Beta is admitted in {build:?}: {r:?}"));
            assert!(ranks_validate(&s, build, &d).may_run(), "{build:?}");
            assert_eq!(
                ranks_run(&s, build, &d, go).ok(),
                stats_ranks::compute(&d, &s.parameters).ok(),
                "{build:?}: the module's own result"
            );
            assert!(ranks_run(&s, build, &d, go).is_ok(), "{build:?}");
        }
    }

    #[test]
    fn the_ranks_facade_refuses_forged_specs_and_runs_the_real_one_as_the_module_does() {
        let d = data();
        let b = Build::Devtools;
        let real = Registry::statistics();
        for id in [
            "stats.analyze.independent_ranks_exact",
            "stats.describe",
            "stats.analyze.one_way_anova",
            "stats.analyze.crosstabs",
        ] {
            let s = forged(&d, id, 1);
            assert_eq!(
                codes(&ranks_validate_in(&real, &s, b, &d)),
                [RanksCode::UnknownCapability],
                "{id}"
            );
            assert!(!ranks_validate_in(&real, &s, b, &d).may_run(), "{id}");
            assert_eq!(ranks_run_in(&real, &s, b, &d, go).unwrap_err(), unknown(), "{id}");
            assert!(ranks_admit(&s, b).is_err(), "{id}");
        }
        let v = ranks_validate_in(&real, &forged(&d, stats_ranks::CAPABILITY_ID, 2), b, &d);
        assert_eq!(codes(&v), [RanksCode::CapabilityVersionMismatch]);
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Expected),
            Some(&ParamValue::Int(1))
        );
        assert_eq!(
            v.errors[0].params.get(&ParamKey::Found),
            Some(&ParamValue::Int(2))
        );
        // A Planned table: the real one, set back to Planned, as Part 2 had it.
        let good = forged(&d, stats_ranks::CAPABILITY_ID, 1);
        let planned =
            Registry::statistics().with_lifecycle(stats_ranks::CAPABILITY_ID, Lifecycle::Planned);
        for build in [Build::Standard, Build::Devtools] {
            assert_eq!(
                codes(&ranks_validate_in(&planned, &good, build, &d)),
                [RanksCode::UnknownCapability]
            );
            assert_eq!(
                ranks_run_in(&planned, &good, build, &d, go).unwrap_err(),
                unknown()
            );
        }
        let mut c = good.clone();
        c.confirmation_refs = vec![ConfirmationId::parse(&"ef".repeat(32)).unwrap()];
        assert_eq!(
            codes(&ranks_validate_in(&real, &c, b, &d)),
            [RanksCode::UnknownCapability]
        );
        assert_eq!(
            ranks_validate_in(&real, &good, b, &d),
            stats_ranks::validate(&d, &good)
        );
        assert_eq!(
            ranks_run_in(&real, &good, b, &d, go).unwrap(),
            stats_ranks::compute(&d, &good.parameters).unwrap()
        );
    }
}
