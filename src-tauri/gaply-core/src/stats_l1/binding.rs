//! **Bindings: a proposal is a suggestion; a confirmed binding is the one
//! thing a computation may run on.**
//!
//! [`BindingProposal`] and [`ConfirmedBinding`] are different types on
//! purpose. A proposal has an id and records its source: the local proposer,
//! an AI suggestion (task id and request id), or the user. An edit to a
//! proposal is a new proposal whose source is `Manual`, "edited from" the
//! original. Nothing turns a proposal into a confirmed binding except
//! [`confirm`], which [`super::run_state::L1RunState::confirm`] calls with a
//! proposal id it holds; `ConfirmedBinding`'s fields are private, so no struct
//! literal anywhere else can make one.
//!
//! `ResultBinding` was the name Step 1 proposed; `BindingProposal` and
//! `ConfirmedBinding` say which of the two states a value is in, which is the
//! distinction this module exists for. `Binding`, `Bindings` and
//! `BindingSource` are taken by the equation modules.
//!
//! # Validation
//!
//! [`confirm`] rejects, in this order: a stale fingerprint; a claim whose
//! revision has moved; a method not in the catalogue; any transformation; a
//! paired method bound to an outcome-and-group pair; an independent method
//! bound to two outcome columns; a role the method has no slot for; a missing
//! role; an unknown column; one column in two roles; a column of the wrong
//! type; a filter on a column that is not a group, or naming a level the
//! column does not have; a selected level the column does not have after
//! filtering; an extra level left unselected; the wrong number of groups
//! after level selection; and for OLS a missing or misdirected target.
//!
//! # The counts pass
//!
//! Confirming makes one pass over the data and records counts only: rows
//! read, rows the include-filters removed, rows excluded for a missing value
//! in a bound column (listwise, on the bound columns only), and the n that
//! would be used. It also flags, and never recodes, values in bound numeric
//! columns that look like missing-value codes.
//!
//! # Whole-number columns (evaluation fixes, part C, B-1)
//!
//! A bound numeric column whose every cell is a whole number may hold exact
//! counts or scale points, or values rounded to whole numbers on deposit
//! (milliseconds, percentages, ages). The data cannot say which, so the
//! binding carries the user's answer for each such column:
//! [`WholeNumbers::RoundedOrNotSure`] unless the draft names the column in
//! [`BindingDraft::exact_whole_numbers`]. The answer is part of the digest a
//! token is bound to, so changing it is a new binding; an AI suggestion
//! never carries one ([`super::run_state::L1RunState::add_proposal`] clears
//! it), and no outbound struct has a field for it.

use super::catalogue::{L1MethodId, LevelRule, MethodCatalogue, RoleName, SlotKind, VariantChoice};
use super::claim::{ClaimId, L1Claim, RevisionHash};
use super::dataset::{sha256_hex, Cell, ColumnAlias, ColumnType, Dataset};

/// A proposal's id in the run. Stable: an edit is a NEW id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProposalId(pub u32);

/// Why the local proposer suggested a binding. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalReason {
    /// Header tokens overlap the claim's sentence.
    HeaderMatchesSentence,
    /// No header matched; these are the first columns of the right types.
    FirstEligibleColumns,
}

/// Where a proposal came from.
#[derive(Clone, PartialEq, Eq)]
pub enum ProposalSource {
    Local {
        reason: ProposalReason,
        /// The header tokens that matched the sentence. Local only.
        matched_tokens: Vec<String>,
    },
    AiSuggestion {
        task_id: String,
        request_id: String,
    },
    Manual {
        /// The source this proposal was edited from, if it was an edit.
        edited_from: Option<Box<ProposalSource>>,
    },
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for ProposalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalSource::Local { reason, matched_tokens } => {
                f.debug_struct("Local").field("reason", reason).field("matched_tokens", &matched_tokens.len()).finish()
            }
            ProposalSource::AiSuggestion { task_id, request_id } => {
                f.debug_struct("AiSuggestion").field("task_id", task_id).field("request_id", request_id).finish()
            }
            ProposalSource::Manual { edited_from } => f.debug_struct("Manual").field("edited_from", edited_from).finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleBinding {
    pub role: RoleName,
    pub columns: Vec<ColumnAlias>,
}

/// The levels of a group role that enter the comparison. When absent, every
/// level present after the include-filters enters.
#[derive(Clone, PartialEq, Eq)]
pub struct LevelSelection {
    pub role: RoleName,
    pub levels: Vec<String>,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for LevelSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelSelection").field("role", &self.role).field("levels", &self.levels.len()).finish_non_exhaustive()
    }
}

/// "`column` is one of these levels": the only filter in Phase 2.
#[derive(Clone, PartialEq, Eq)]
pub struct IncludeFilter {
    pub column: ColumnAlias,
    pub levels: Vec<String>,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for IncludeFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IncludeFilter").field("column", &self.column).field("levels", &self.levels.len()).finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlsTarget {
    OverallF,
    Coefficient { predictor: ColumnAlias },
}

/// The user's answer for one bound whole-number column (part C, B-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WholeNumbers {
    /// "Rounded, or not sure": the default. Each cell may be off by up to
    /// half a unit, through the same range as a decimal column.
    RoundedOrNotSure,
    /// "Exact whole numbers": counts or points on a rating scale. No range.
    Exact,
}

/// One bound whole-number column and the answer the binding was confirmed
/// with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WholeNumberAnswer {
    pub column: ColumnAlias,
    pub answer: WholeNumbers,
}

/// Listwise on the bound columns only: the only rule in Phase 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingDataRule {
    ListwiseOnBoundColumns,
}

/// A transformation someone asked for. Phase 2 has none: any request is a
/// rejection. The variants exist so a request can be expressed and refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformationRequest {
    Log,
    Standardise,
    Centre,
    Other,
}

/// What a proposal binds, before anyone has checked it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingDraft {
    /// The method's string id, looked up in the catalogue at confirm time.
    pub method_id: String,
    pub roles: Vec<RoleBinding>,
    pub selected_levels: Vec<LevelSelection>,
    pub filters: Vec<IncludeFilter>,
    pub missing: MissingDataRule,
    pub variant: VariantChoice,
    pub target: Option<OlsTarget>,
    pub transformations: Vec<TransformationRequest>,
    /// Bound whole-number columns the user says hold EXACT whole numbers.
    /// Every other bound whole-number column is rounded, or not sure.
    pub exact_whole_numbers: Vec<ColumnAlias>,
}

impl BindingDraft {
    pub fn new(method: L1MethodId, variant: VariantChoice) -> BindingDraft {
        BindingDraft {
            method_id: method.id().to_string(),
            roles: Vec::new(),
            selected_levels: Vec::new(),
            filters: Vec::new(),
            missing: MissingDataRule::ListwiseOnBoundColumns,
            variant,
            target: None,
            transformations: Vec::new(),
            exact_whole_numbers: Vec::new(),
        }
    }
    pub fn role(mut self, role: RoleName, columns: &[ColumnAlias]) -> BindingDraft {
        self.roles.push(RoleBinding { role, columns: columns.to_vec() });
        self
    }
    pub fn levels(mut self, role: RoleName, levels: &[&str]) -> BindingDraft {
        self.selected_levels.push(LevelSelection { role, levels: levels.iter().map(|s| s.to_string()).collect() });
        self
    }
    pub fn filter(mut self, column: ColumnAlias, levels: &[&str]) -> BindingDraft {
        self.filters.push(IncludeFilter { column, levels: levels.iter().map(|s| s.to_string()).collect() });
        self
    }
    pub fn target(mut self, target: OlsTarget) -> BindingDraft {
        self.target = Some(target);
        self
    }
    /// The user's "Exact whole numbers" answer for these columns.
    pub fn exact_whole_numbers(mut self, columns: &[ColumnAlias]) -> BindingDraft {
        self.exact_whole_numbers = columns.to_vec();
        self
    }

    /// The columns this draft binds to a numeric slot that hold only whole
    /// numbers in `dataset`, each with the answer it would be confirmed
    /// with, by alias: the columns the Confirm step asks about. Empty for a
    /// method not in the catalogue. [`confirm`] records exactly these.
    pub fn whole_number_answers(&self, dataset: &Dataset) -> Vec<WholeNumberAnswer> {
        let Some(entry) = MethodCatalogue::by_id(&self.method_id) else { return Vec::new() };
        let mut out: Vec<WholeNumberAnswer> = Vec::new();
        for r in &self.roles {
            if !entry.slot(r.role).is_some_and(|slot| !matches!(slot.kind, SlotKind::Group { .. })) {
                continue;
            }
            for &c in &r.columns {
                if out.iter().any(|w| w.column == c) || !dataset.column_cells(c).is_some_and(holds_only_whole_numbers) {
                    continue;
                }
                let answer = if self.exact_whole_numbers.contains(&c) { WholeNumbers::Exact } else { WholeNumbers::RoundedOrNotSure };
                out.push(WholeNumberAnswer { column: c, answer });
            }
        }
        out.sort_by_key(|w| w.column);
        out
    }
}

/// A binding someone proposed for one claim against one dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingProposal {
    pub id: ProposalId,
    pub claim: ClaimId,
    /// The claim's revision when the proposal was made.
    pub claim_revision: RevisionHash,
    /// The dataset's fingerprint digest when the proposal was made.
    pub fingerprint_digest: String,
    pub binding: BindingDraft,
    pub source: ProposalSource,
}

/// Counts from the confirm pass. Never values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingCounts {
    pub rows_read: usize,
    pub rows_filtered_out: usize,
    pub rows_excluded_missing: usize,
    pub n_used: usize,
}

/// A value in a bound numeric column that looks like a missing-value code:
/// one of the closed set, sitting far outside the other values. The screen
/// asks; nothing here recodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuspectMissingCode {
    pub column: ColumnAlias,
    pub code: i32,
    pub cells: usize,
}

/// The closed set of codes [`SuspectMissingCode`] can name.
pub const MISSING_CODE_CANDIDATES: [i32; 6] = [-9, -99, -999, 99, 999, 9999];

/// Why a proposal was not confirmed. Aliases, roles and counts only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    StaleFingerprint,
    ClaimRevisionChanged,
    MethodNotInCatalogue,
    TransformationRequested,
    PairedMethodBoundToOutcomeAndGroup,
    IndependentMethodBoundToTwoOutcomes,
    RoleNotInMethod(RoleName),
    MissingRole(RoleName),
    RoleHasTooManyColumns(RoleName),
    UnknownColumn(ColumnAlias),
    ColumnInTwoRoles(ColumnAlias),
    WrongType { role: RoleName, column: ColumnAlias, found: ColumnType },
    FilterOnNonGroupColumn(ColumnAlias),
    FilterLevelNotInColumn(ColumnAlias),
    LevelNotInColumn { role: RoleName },
    UnselectedLevel { role: RoleName, unselected: usize },
    WrongGroupCount { role: RoleName, selected: usize, expected: LevelRule },
    TargetMissing,
    TargetNotAPredictor,
    TargetNotApplicable,
    /// "Exact whole numbers" was given for a column that is not a bound
    /// numeric column holding only whole numbers.
    WholeNumberAnswerNotApplicable(ColumnAlias),
}

/// A binding the user confirmed. Private fields; built only by [`confirm`].
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmedBinding {
    proposal: ProposalId,
    source: ProposalSource,
    claim: ClaimId,
    claim_revision: RevisionHash,
    fingerprint_digest: String,
    method: L1MethodId,
    roles: Vec<RoleBinding>,
    /// Every group role, with the levels that enter: resolved and sorted.
    levels: Vec<LevelSelection>,
    filters: Vec<IncludeFilter>,
    missing: MissingDataRule,
    variant: VariantChoice,
    target: Option<OlsTarget>,
    counts: BindingCounts,
    suspect_codes: Vec<SuspectMissingCode>,
    /// Every bound numeric column whose cells are all whole numbers, with the
    /// answer it was confirmed with, by alias.
    whole_numbers: Vec<WholeNumberAnswer>,
    digest: String,
}

impl ConfirmedBinding {
    pub fn proposal(&self) -> ProposalId {
        self.proposal
    }
    pub fn source(&self) -> &ProposalSource {
        &self.source
    }
    pub fn claim(&self) -> ClaimId {
        self.claim
    }
    pub fn claim_revision(&self) -> &RevisionHash {
        &self.claim_revision
    }
    pub fn fingerprint_digest(&self) -> &str {
        &self.fingerprint_digest
    }
    pub fn method(&self) -> L1MethodId {
        self.method
    }
    pub fn roles(&self) -> &[RoleBinding] {
        &self.roles
    }
    pub fn columns_for(&self, role: RoleName) -> &[ColumnAlias] {
        self.roles.iter().find(|r| r.role == role).map(|r| r.columns.as_slice()).unwrap_or(&[])
    }
    pub fn levels(&self) -> &[LevelSelection] {
        &self.levels
    }
    pub fn filters(&self) -> &[IncludeFilter] {
        &self.filters
    }
    pub fn missing(&self) -> MissingDataRule {
        self.missing
    }
    pub fn variant(&self) -> VariantChoice {
        self.variant
    }
    pub fn target(&self) -> Option<OlsTarget> {
        self.target
    }
    pub fn counts(&self) -> BindingCounts {
        self.counts
    }
    pub fn suspect_codes(&self) -> &[SuspectMissingCode] {
        &self.suspect_codes
    }
    /// The bound whole-number columns and the answer for each.
    pub fn whole_numbers(&self) -> &[WholeNumberAnswer] {
        &self.whole_numbers
    }
    /// The answer for `column`, when it is a bound whole-number column.
    pub fn whole_number_answer(&self, column: ColumnAlias) -> Option<WholeNumbers> {
        self.whole_numbers.iter().find(|w| w.column == column).map(|w| w.answer)
    }
    /// SHA-256 of the normalised binding: what a token is bound to.
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/* ------------------------------------------------------------------------ *
 *  Validation and confirmation
 * ------------------------------------------------------------------------ */

/// Validate `proposal` against `dataset` and `claim`, make the counts pass,
/// and return the confirmed binding. Only the run state calls this.
pub(super) fn confirm(proposal: &BindingProposal, dataset: &Dataset, claim: &L1Claim) -> Result<ConfirmedBinding, Rejection> {
    if proposal.fingerprint_digest != dataset.schema.fingerprint.digest() {
        return Err(Rejection::StaleFingerprint);
    }
    if proposal.claim != claim.id || proposal.claim_revision != claim.revision {
        return Err(Rejection::ClaimRevisionChanged);
    }
    let draft = &proposal.binding;
    let entry = MethodCatalogue::by_id(&draft.method_id).ok_or(Rejection::MethodNotInCatalogue)?;
    if !draft.transformations.is_empty() {
        return Err(Rejection::TransformationRequested);
    }

    let has = |role: RoleName| draft.roles.iter().any(|r| r.role == role && !r.columns.is_empty());
    if entry.id.is_paired() && has(RoleName::Outcome) && has(RoleName::Group) {
        return Err(Rejection::PairedMethodBoundToOutcomeAndGroup);
    }
    if matches!(entry.id, L1MethodId::IndependentT | L1MethodId::OneWayAnova) {
        let two_outcomes = (has(RoleName::First) && has(RoleName::Second))
            || draft.roles.iter().any(|r| r.role == RoleName::Outcome && r.columns.len() > 1);
        if two_outcomes {
            return Err(Rejection::IndependentMethodBoundToTwoOutcomes);
        }
    }
    for r in &draft.roles {
        if entry.slot(r.role).is_none() {
            return Err(Rejection::RoleNotInMethod(r.role));
        }
    }

    // Every slot filled, with the right number of columns.
    let mut roles: Vec<RoleBinding> = Vec::with_capacity(entry.slots.len());
    for slot in entry.slots {
        let given = draft.roles.iter().filter(|r| r.role == slot.role).flat_map(|r| r.columns.iter().copied()).collect::<Vec<_>>();
        match slot.kind {
            SlotKind::Numeric | SlotKind::Group { .. } => {
                if given.is_empty() {
                    return Err(Rejection::MissingRole(slot.role));
                }
                if given.len() > 1 {
                    return Err(Rejection::RoleHasTooManyColumns(slot.role));
                }
            }
            SlotKind::NumericList { min } => {
                if given.len() < min {
                    return Err(Rejection::MissingRole(slot.role));
                }
            }
        }
        roles.push(RoleBinding { role: slot.role, columns: given });
    }

    // Every column known, and no column in two roles.
    let mut seen: Vec<ColumnAlias> = Vec::new();
    for r in &roles {
        for &c in &r.columns {
            if dataset.schema.column(c).is_none() {
                return Err(Rejection::UnknownColumn(c));
            }
            if seen.contains(&c) {
                return Err(Rejection::ColumnInTwoRoles(c));
            }
            seen.push(c);
        }
    }
    for f in &draft.filters {
        if dataset.schema.column(f.column).is_none() {
            return Err(Rejection::UnknownColumn(f.column));
        }
    }

    // Types.
    for (r, slot) in roles.iter().zip(entry.slots) {
        for &c in &r.columns {
            let col = dataset.schema.column(c).expect("checked above");
            let ok = match slot.kind {
                SlotKind::Numeric | SlotKind::NumericList { .. } => col.is_numeric(),
                SlotKind::Group { .. } => col.is_group(),
            };
            if !ok {
                return Err(Rejection::WrongType { role: r.role, column: c, found: col.column_type });
            }
        }
    }

    // Filters: on group columns, naming levels the column has.
    for f in &draft.filters {
        let col = dataset.schema.column(f.column).expect("checked above");
        let Some(levels) = col.levels.as_ref().filter(|_| col.is_group()) else {
            return Err(Rejection::FilterOnNonGroupColumn(f.column));
        };
        if f.levels.is_empty() || f.levels.iter().any(|l| !levels.codes().any(|c| c == l)) {
            return Err(Rejection::FilterLevelNotInColumn(f.column));
        }
    }
    let row_passes = |row: usize| -> bool { passes_filters(dataset, &draft.filters, row) };

    // Levels: the selection, when given, must name levels the column has
    // after filtering and must leave none unselected; the count must fit.
    let mut levels: Vec<LevelSelection> = Vec::new();
    for (r, slot) in roles.iter().zip(entry.slots) {
        let SlotKind::Group { levels: rule } = slot.kind else { continue };
        let column = r.columns[0];
        let cells = dataset.column_cells(column).expect("checked above");
        let mut present: Vec<String> = (0..dataset.rows()).filter(|&i| row_passes(i)).filter_map(|i| cells[i].level_code()).collect();
        present.sort_unstable();
        present.dedup();
        let selected = draft.selected_levels.iter().find(|s| s.role == r.role);
        let mut effective = match selected {
            Some(s) => {
                let mut sel = s.levels.clone();
                sel.sort_unstable();
                sel.dedup();
                if sel.iter().any(|l| !present.contains(l)) {
                    return Err(Rejection::LevelNotInColumn { role: r.role });
                }
                let unselected = present.iter().filter(|p| !sel.contains(p)).count();
                if unselected > 0 {
                    return Err(Rejection::UnselectedLevel { role: r.role, unselected });
                }
                sel
            }
            None => present,
        };
        if !rule.admits(effective.len()) {
            return Err(Rejection::WrongGroupCount { role: r.role, selected: effective.len(), expected: rule });
        }
        effective.sort_unstable();
        levels.push(LevelSelection { role: r.role, levels: effective });
    }

    // OLS target.
    match (entry.needs_target, draft.target) {
        (true, None) => return Err(Rejection::TargetMissing),
        (true, Some(OlsTarget::Coefficient { predictor })) => {
            let predictors = roles.iter().find(|r| r.role == RoleName::Predictors).map(|r| r.columns.as_slice()).unwrap_or(&[]);
            if !predictors.contains(&predictor) {
                return Err(Rejection::TargetNotAPredictor);
            }
        }
        (true, Some(OlsTarget::OverallF)) => {}
        (false, Some(_)) => return Err(Rejection::TargetNotApplicable),
        (false, None) => {}
    }

    // Whole-number columns among the numeric slots, and the user's answers.
    let whole_numbers = draft.whole_number_answers(dataset);
    if let Some(&c) = draft.exact_whole_numbers.iter().find(|c| !whole_numbers.iter().any(|w| w.column == **c)) {
        return Err(Rejection::WholeNumberAnswerNotApplicable(c));
    }

    // The counts pass.
    let bound: Vec<ColumnAlias> = roles.iter().flat_map(|r| r.columns.iter().copied()).collect();
    let mut counts = BindingCounts { rows_read: dataset.rows(), rows_filtered_out: 0, rows_excluded_missing: 0, n_used: 0 };
    for i in 0..dataset.rows() {
        if !row_passes(i) {
            counts.rows_filtered_out += 1;
            continue;
        }
        let complete = bound.iter().all(|&c| !dataset.column_cells(c).expect("checked")[i].is_missing());
        if complete {
            counts.n_used += 1;
        } else {
            counts.rows_excluded_missing += 1;
        }
    }

    // Suspect codes in the numeric-slot columns.
    let mut suspect_codes = Vec::new();
    for (r, slot) in roles.iter().zip(entry.slots) {
        if matches!(slot.kind, SlotKind::Group { .. }) {
            continue;
        }
        for &c in &r.columns {
            suspect_codes.extend(suspect_codes_in(c, dataset.column_cells(c).expect("checked")));
        }
    }

    let mut filters = draft.filters.clone();
    filters.sort_by_key(|f| f.column);
    for f in &mut filters {
        f.levels.sort_unstable();
        f.levels.dedup();
    }
    let digest = normalised_digest(entry.id, &roles, &levels, &filters, draft.missing, draft.variant, draft.target, &whole_numbers);

    tracing::debug!(
        rows_read = counts.rows_read,
        rows_filtered_out = counts.rows_filtered_out,
        rows_excluded_missing = counts.rows_excluded_missing,
        n_used = counts.n_used,
        suspect_codes = suspect_codes.len(),
        "stats_l1: binding confirmed"
    );

    Ok(ConfirmedBinding {
        proposal: proposal.id,
        source: proposal.source.clone(),
        claim: claim.id,
        claim_revision: claim.revision.clone(),
        fingerprint_digest: proposal.fingerprint_digest.clone(),
        method: entry.id,
        roles,
        levels,
        filters,
        missing: draft.missing,
        variant: draft.variant,
        target: draft.target,
        counts,
        suspect_codes,
        whole_numbers,
        digest,
    })
}

/// Every number in the column is a whole number, and there is at least one.
/// Missing and text cells are not numbers and do not count either way.
pub fn holds_only_whole_numbers(cells: &[Cell]) -> bool {
    let mut any = false;
    for c in cells {
        if let Cell::Number(v) = c {
            if !(v.is_finite() && *v == v.trunc()) {
                return false;
            }
            any = true;
        }
    }
    any
}

/// Whether `row` passes every include-filter: its cell in each filtered
/// column is one of that filter's levels. A missing cell passes no filter.
/// Shared by the confirm pass and Level 1's computation, so the rows counted
/// at confirm time are the rows computed on.
pub(super) fn passes_filters(dataset: &Dataset, filters: &[IncludeFilter], row: usize) -> bool {
    filters.iter().all(|f| {
        dataset.column_cells(f.column).and_then(|cells| cells[row].level_code()).is_some_and(|code| f.levels.contains(&code))
    })
}

/// A candidate code is flagged when it occurs and every OTHER value in the
/// column (the non-candidate ones, at least three of them) lies in a range
/// the code sits more than three widths outside of. `-99` among 1-to-7 is
/// flagged; `99` among 0-to-120 is not. The descriptive summary's profile
/// reports what this finds and never applies it (`stats_desc`, Research OS
/// Phase 2A).
pub(crate) fn suspect_codes_in(column: ColumnAlias, cells: &[Cell]) -> Vec<SuspectMissingCode> {
    let values: Vec<f64> = cells.iter().filter_map(Cell::number).collect();
    let is_candidate = |v: f64| MISSING_CODE_CANDIDATES.iter().any(|&c| v == f64::from(c));
    let others: Vec<f64> = values.iter().copied().filter(|v| !is_candidate(*v)).collect();
    if others.len() < 3 {
        return Vec::new();
    }
    let lo = others.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = others.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let width = (hi - lo).max(1.0);
    MISSING_CODE_CANDIDATES
        .iter()
        .filter_map(|&code| {
            let cells = values.iter().filter(|&&v| v == f64::from(code)).count();
            let c = f64::from(code);
            (cells > 0 && (c < lo - 3.0 * width || c > hi + 3.0 * width)).then_some(SuspectMissingCode { column, code, cells })
        })
        .collect()
}

fn normalised_digest(
    method: L1MethodId,
    roles: &[RoleBinding],
    levels: &[LevelSelection],
    filters: &[IncludeFilter],
    missing: MissingDataRule,
    variant: VariantChoice,
    target: Option<OlsTarget>,
    whole_numbers: &[WholeNumberAnswer],
) -> String {
    let mut s = format!("method={};", method.id());
    let mut rs: Vec<&RoleBinding> = roles.iter().collect();
    rs.sort_by_key(|r| r.role);
    for r in rs {
        s.push_str(&format!("{:?}:{};", r.role, r.columns.iter().map(|c| c.name()).collect::<Vec<_>>().join(",")));
    }
    let mut ls: Vec<&LevelSelection> = levels.iter().collect();
    ls.sort_by_key(|l| l.role);
    for l in ls {
        s.push_str(&format!("levels {:?}:[{}];", l.role, l.levels.join("\u{1f}")));
    }
    for f in filters {
        s.push_str(&format!("filter {}:[{}];", f.column.name(), f.levels.join("\u{1f}")));
    }
    s.push_str(&format!("missing={missing:?};variant={};target={target:?}", variant.variant().canonical()));
    for w in whole_numbers {
        s.push_str(&format!(";whole {}={:?}", w.column.name(), w.answer));
    }
    sha256_hex(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::SectionKind;
    use crate::stats_l0::number::Relation;
    use crate::stats_l0::result::Family;
    use crate::stats_l1::catalogue::{TVariant, Variant, VariantKind};
    use crate::stats_l1::claim::{ClaimLocation, ClaimOrigin, UserClaim, UserP};
    use crate::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};

    fn dataset(headers: &[&str], rows: &[&[&str]]) -> Dataset {
        let raw = RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        );
        build_dataset(raw, DataFingerprint::of_bytes(b"bytes", "p", TableRef::FirstTable)).unwrap()
    }

    /// score, group (1/2), site (a/b/c), likert (1..7 with a -99), age (0..120 with a 99), text.
    fn typical() -> Dataset {
        dataset(
            &["score", "group", "site", "likert", "age", "text"],
            &[
                &["1.5", "1", "a", "1", "99", "x"],
                &["2.5", "2", "b", "7", "120", "y"],
                &["3.5", "1", "c", "4", "0", "z"],
                &["4.5", "2", "a", "-99", "45", "w"],
                &["", "1", "b", "3", "60", "v"],
                &["6.5", "2", "", "5", "30", "u"],
            ],
        )
    }

    fn claim() -> L1Claim {
        L1Claim::user_entered(
            ClaimId(0),
            &UserClaim {
                family: Family::T,
                statistic: "2.10",
                df: vec!["4"],
                n: None,
                p: UserP::Reported { raw: ".04", relation: Relation::Eq },
                location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
                sentence: None,
            },
            ClaimOrigin::UserAdded,
        )
        .unwrap()
    }

    fn proposal(d: &Dataset, c: &L1Claim, binding: BindingDraft) -> BindingProposal {
        BindingProposal {
            id: ProposalId(1),
            claim: c.id,
            claim_revision: c.revision.clone(),
            fingerprint_digest: d.schema.fingerprint.digest(),
            binding,
            source: ProposalSource::Manual { edited_from: None },
        }
    }

    fn t_draft() -> BindingDraft {
        BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
            .role(RoleName::Outcome, &[ColumnAlias(1)])
            .role(RoleName::Group, &[ColumnAlias(2)])
    }

    const C: fn(u32) -> ColumnAlias = ColumnAlias;

    #[test]
    fn an_integer_coded_group_and_a_likert_outcome_bind_with_counts() {
        let d = typical();
        let c = claim();
        let draft = BindingDraft::new(L1MethodId::IndependentT, VariantChoice::user_ticked(Variant::T(TVariant::Welch)))
            .role(RoleName::Outcome, &[C(4)])
            .role(RoleName::Group, &[C(2)]);
        let b = confirm(&proposal(&d, &c, draft), &d, &c).unwrap();
        assert_eq!(b.method(), L1MethodId::IndependentT);
        assert_eq!(b.counts(), BindingCounts { rows_read: 6, rows_filtered_out: 0, rows_excluded_missing: 0, n_used: 6 });
        assert_eq!(b.levels(), &[LevelSelection { role: RoleName::Group, levels: vec!["1".into(), "2".into()] }]);
        assert_eq!(b.variant().variant(), Variant::T(TVariant::Welch));
        // -99 in a 1-to-7 column is flagged, never recoded: n_used still counts it.
        assert_eq!(b.suspect_codes(), &[SuspectMissingCode { column: C(4), code: -99, cells: 1 }]);
        assert_eq!(b.digest().len(), 64);
        // 0/1 coding binds the same way.
        let d2 = dataset(&["y", "g"], &[&["1", "0"], &["2", "1"], &["3", "0"], &["4", "1"]]);
        let b2 = confirm(&proposal(&d2, &c, t_draft()), &d2, &c).unwrap();
        assert_eq!(b2.levels()[0].levels, vec!["0", "1"]);
    }

    #[test]
    fn ninety_nine_inside_an_age_range_is_not_flagged() {
        let d = typical();
        let c = claim();
        // Outcome: age (C5), 0 to 120 with a real 99 in it.
        let draft = t_draft().roles_replaced(&[(RoleName::Outcome, C(5)), (RoleName::Group, C(2))]);
        let b = confirm(&proposal(&d, &c, draft), &d, &c).unwrap();
        assert!(b.suspect_codes().is_empty(), "{:?}", b.suspect_codes());
    }

    #[test]
    fn the_counts_pass_counts_filtered_and_missing_rows_separately() {
        let d = typical();
        let c = claim();
        // Outcome `score` (one missing), group `site` filtered to a and b.
        let draft = BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
            .role(RoleName::Outcome, &[C(1)])
            .role(RoleName::Group, &[C(3)])
            .filter(C(3), &["a", "b"]);
        let b = confirm(&proposal(&d, &c, draft), &d, &c).unwrap();
        // Rows: c (filtered), "" site (filtered: missing is not one of the levels),
        // the score-missing row is site b → excluded for missing. 6 - 2 - 1 = 3.
        assert_eq!(b.counts(), BindingCounts { rows_read: 6, rows_filtered_out: 2, rows_excluded_missing: 1, n_used: 3 });
        assert_eq!(b.levels()[0].levels, vec!["a", "b"]);
    }

    fn rejects(d: &Dataset, draft: BindingDraft) -> Rejection {
        let c = claim();
        confirm(&proposal(d, &c, draft), d, &c).unwrap_err()
    }

    #[test]
    fn a_missing_role_is_rejected() {
        let d = typical();
        assert_eq!(rejects(&d, BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T)).role(RoleName::Outcome, &[C(1)])), Rejection::MissingRole(RoleName::Group));
        assert_eq!(rejects(&d, BindingDraft::new(L1MethodId::Ols, VariantChoice::not_stated(VariantKind::None)).role(RoleName::Outcome, &[C(1)]).target(OlsTarget::OverallF)), Rejection::MissingRole(RoleName::Predictors));
    }

    #[test]
    fn a_column_of_the_wrong_type_is_rejected() {
        let d = typical();
        // text as outcome
        assert_eq!(rejects(&d, t_draft().roles_replaced(&[(RoleName::Outcome, C(6)), (RoleName::Group, C(2))])), Rejection::WrongType { role: RoleName::Outcome, column: C(6), found: ColumnType::Categorical });
        // a numeric non-integer column as group
        assert_eq!(rejects(&d, t_draft().roles_replaced(&[(RoleName::Outcome, C(4)), (RoleName::Group, C(1))])), Rejection::WrongType { role: RoleName::Group, column: C(1), found: ColumnType::Numeric });
    }

    impl BindingDraft {
        fn roles_replaced(mut self, roles: &[(RoleName, ColumnAlias)]) -> BindingDraft {
            self.roles = roles.iter().map(|(r, c)| RoleBinding { role: *r, columns: vec![*c] }).collect();
            self
        }
    }

    #[test]
    fn the_wrong_number_of_groups_after_level_selection_is_rejected() {
        let d = typical();
        let three = t_draft().roles_replaced(&[(RoleName::Outcome, C(1)), (RoleName::Group, C(3))]);
        assert_eq!(rejects(&d, three), Rejection::WrongGroupCount { role: RoleName::Group, selected: 3, expected: LevelRule::Exactly(2) });
        // ANOVA on one level after filtering: fewer than 2.
        let one = BindingDraft::new(L1MethodId::OneWayAnova, VariantChoice::not_stated(VariantKind::None))
            .role(RoleName::Outcome, &[C(1)])
            .role(RoleName::Group, &[C(3)])
            .filter(C(3), &["a"]);
        assert_eq!(rejects(&d, one), Rejection::WrongGroupCount { role: RoleName::Group, selected: 1, expected: LevelRule::AtLeast(2) });
    }

    #[test]
    fn an_extra_level_left_unselected_is_rejected_and_a_filter_is_how_to_drop_it() {
        let d = typical();
        let partial = t_draft().roles_replaced(&[(RoleName::Outcome, C(1)), (RoleName::Group, C(3))]).levels(RoleName::Group, &["a", "b"]);
        assert_eq!(rejects(&d, partial), Rejection::UnselectedLevel { role: RoleName::Group, unselected: 1 });
        let filtered = t_draft().roles_replaced(&[(RoleName::Outcome, C(1)), (RoleName::Group, C(3))]).levels(RoleName::Group, &["a", "b"]).filter(C(3), &["a", "b"]);
        let c = claim();
        assert!(confirm(&proposal(&d, &c, filtered), &d, &c).is_ok());
        // A selected level the column does not have.
        let ghost = t_draft().levels(RoleName::Group, &["1", "3"]);
        assert_eq!(rejects(&d, ghost), Rejection::LevelNotInColumn { role: RoleName::Group });
    }

    #[test]
    fn one_column_in_two_roles_is_rejected() {
        let d = typical();
        assert_eq!(rejects(&d, t_draft().roles_replaced(&[(RoleName::Outcome, C(4)), (RoleName::Group, C(4))])), Rejection::ColumnInTwoRoles(C(4)));
        let ols = BindingDraft::new(L1MethodId::Ols, VariantChoice::not_stated(VariantKind::None))
            .role(RoleName::Outcome, &[C(1)])
            .role(RoleName::Predictors, &[C(4), C(5), C(4)])
            .target(OlsTarget::OverallF);
        assert_eq!(rejects(&d, ols), Rejection::ColumnInTwoRoles(C(4)));
    }

    #[test]
    fn a_paired_method_bound_to_an_outcome_and_group_pair_is_rejected() {
        let d = typical();
        let draft = BindingDraft::new(L1MethodId::PairedT, VariantChoice::not_stated(VariantKind::None))
            .role(RoleName::Outcome, &[C(1)])
            .role(RoleName::Group, &[C(2)]);
        assert_eq!(rejects(&d, draft), Rejection::PairedMethodBoundToOutcomeAndGroup);
        // And bound properly it confirms.
        let ok = BindingDraft::new(L1MethodId::PairedT, VariantChoice::not_stated(VariantKind::None))
            .role(RoleName::First, &[C(1)])
            .role(RoleName::Second, &[C(4)]);
        let c = claim();
        let b = confirm(&proposal(&d, &c, ok), &d, &c).unwrap();
        assert_eq!(b.counts().n_used, 5);
    }

    #[test]
    fn an_independent_method_bound_to_two_outcome_columns_is_rejected() {
        let d = typical();
        let draft = BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
            .role(RoleName::First, &[C(1)])
            .role(RoleName::Second, &[C(4)]);
        assert_eq!(rejects(&d, draft), Rejection::IndependentMethodBoundToTwoOutcomes);
        let draft = BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
            .role(RoleName::Outcome, &[C(1), C(4)])
            .role(RoleName::Group, &[C(2)]);
        assert_eq!(rejects(&d, draft), Rejection::IndependentMethodBoundToTwoOutcomes);
        // A role the method has no slot for, outside those two cases.
        let draft = t_draft().role(RoleName::Predictors, &[C(5)]);
        assert_eq!(rejects(&d, draft), Rejection::RoleNotInMethod(RoleName::Predictors));
    }

    #[test]
    fn an_unknown_column_is_rejected() {
        let d = typical();
        assert_eq!(rejects(&d, t_draft().roles_replaced(&[(RoleName::Outcome, C(1)), (RoleName::Group, C(42))])), Rejection::UnknownColumn(C(42)));
        assert_eq!(rejects(&d, t_draft().filter(C(0), &["a"])), Rejection::UnknownColumn(C(0)));
    }

    #[test]
    fn a_stale_fingerprint_and_a_moved_claim_revision_are_rejected() {
        let d = typical();
        let c = claim();
        let mut p = proposal(&d, &c, t_draft());
        p.fingerprint_digest = "not this dataset".into();
        assert_eq!(confirm(&p, &d, &c).unwrap_err(), Rejection::StaleFingerprint);
        let mut p = proposal(&d, &c, t_draft());
        p.claim_revision = RevisionHash::synthetic(b"older");
        assert_eq!(confirm(&p, &d, &c).unwrap_err(), Rejection::ClaimRevisionChanged);
        let mut p = proposal(&d, &c, t_draft());
        p.claim = ClaimId(7);
        assert_eq!(confirm(&p, &d, &c).unwrap_err(), Rejection::ClaimRevisionChanged);
    }

    #[test]
    fn a_method_not_in_the_catalogue_and_any_transformation_are_rejected() {
        let d = typical();
        let mut draft = t_draft();
        draft.method_id = "mann_whitney".into();
        assert_eq!(rejects(&d, draft), Rejection::MethodNotInCatalogue);
        let mut draft = t_draft();
        draft.transformations.push(TransformationRequest::Log);
        assert_eq!(rejects(&d, draft), Rejection::TransformationRequested);
    }

    #[test]
    fn filters_must_name_a_group_column_and_its_levels() {
        let d = typical();
        assert_eq!(rejects(&d, t_draft().filter(C(1), &["1.5"])), Rejection::FilterOnNonGroupColumn(C(1)));
        assert_eq!(rejects(&d, t_draft().filter(C(3), &["zz"])), Rejection::FilterLevelNotInColumn(C(3)));
        assert_eq!(rejects(&d, t_draft().filter(C(3), &[])), Rejection::FilterLevelNotInColumn(C(3)));
    }

    #[test]
    fn ols_needs_a_target_that_is_one_of_its_predictors() {
        let d = typical();
        let ols = || BindingDraft::new(L1MethodId::Ols, VariantChoice::not_stated(VariantKind::None)).role(RoleName::Outcome, &[C(1)]).role(RoleName::Predictors, &[C(4), C(5)]);
        assert_eq!(rejects(&d, ols()), Rejection::TargetMissing);
        assert_eq!(rejects(&d, ols().target(OlsTarget::Coefficient { predictor: C(2) })), Rejection::TargetNotAPredictor);
        assert_eq!(rejects(&d, t_draft().target(OlsTarget::OverallF)), Rejection::TargetNotApplicable);
        let c = claim();
        let b = confirm(&proposal(&d, &c, ols().target(OlsTarget::Coefficient { predictor: C(5) })), &d, &c).unwrap();
        assert_eq!(b.target(), Some(OlsTarget::Coefficient { predictor: C(5) }));
        assert_eq!(b.counts().n_used, 5);
    }

    #[test]
    fn chi_square_binds_two_group_columns_with_optional_selection() {
        let d = typical();
        let c = claim();
        let draft = BindingDraft::new(L1MethodId::ChiSquareIndependence, VariantChoice::not_stated(VariantKind::Continuity))
            .role(RoleName::RowVariable, &[C(2)])
            .role(RoleName::ColumnVariable, &[C(3)])
            .levels(RoleName::ColumnVariable, &["a", "b", "c"]);
        let b = confirm(&proposal(&d, &c, draft), &d, &c).unwrap();
        assert_eq!(b.counts(), BindingCounts { rows_read: 6, rows_filtered_out: 0, rows_excluded_missing: 1, n_used: 5 });
        assert!(b.suspect_codes().is_empty(), "group columns are not scanned for codes");
    }

    #[test]
    fn the_digest_is_the_normalised_binding_not_its_spelling() {
        let d = typical();
        let c = claim();
        let a = confirm(&proposal(&d, &c, t_draft().filter(C(3), &["b", "a", "a"])), &d, &c).unwrap();
        let b = confirm(&proposal(&d, &c, BindingDraft { roles: t_draft().roles.into_iter().rev().collect(), ..t_draft() }.filter(C(3), &["a", "b"])), &d, &c).unwrap();
        assert_eq!(a.digest(), b.digest());
        let w = confirm(&proposal(&d, &c, BindingDraft { variant: VariantChoice::user_ticked(Variant::T(TVariant::Welch)), ..t_draft() }.filter(C(3), &["a", "b"])), &d, &c).unwrap();
        assert_ne!(a.digest(), w.digest());
    }
}
