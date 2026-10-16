//! **The Level 1 commands (Phase 2, Step 6). Rust owns every decision; the
//! webview asks and displays.**
//!
//! Each command is a thin `#[tauri::command]` over a plain function in this
//! file that takes the run registry ([`L1Runs`]), the AI context
//! ([`StatsAi`]) and the database, so the tests drive exactly the path a
//! command takes, without Tauri.
//!
//! # What reaches the network
//!
//! Only [`helper_send`] (and only with a live grant, which the core session
//! checks) and [`key_test`]. Opening files, editing claims, previewing,
//! confirming and computing make no request.
//!
//! # Run state
//!
//! Each run is keyed by a run id, held in memory, bounded to [`MAX_RUNS`]
//! (the oldest is dropped), and dropped by [`close`] or when the app exits.
//! It holds the manuscript's extraction, the claims, the dataset the run state
//! confirms against, proposals, previews, grants, token handles, the last
//! outcome and usage. Compute reads the data file again; nothing is persisted.
//!
//! # Errors
//!
//! Every failure is a `GaplyError::Validation` whose message is a closed code
//! in JSON (`{"code":"ai_help_off"}`, `{"code":"budget_spent","max":20}`),
//! never an underlying message: those can hold a path or a file name. A
//! failing helper command leaves every other command working.
//!
//! # Handles
//!
//! Preview and proposal handles carry their run id, so one from another run
//! is refused rather than matched against this run's counters. A token handle
//! is the token itself (64 hex characters, unguessable); a forged one is
//! refused before the gate is asked.
//!
//! Tracing skips every argument: no path, file name, header, sentence or body.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gaply_core::extract::{self, ExtractionResult, SectionKind};
use gaply_core::import_guard;
use gaply_core::stats_assist::citation::{host_is_allowed, https_host, parse_web_context, CitationDrop, RetrievedAt};
use gaply_core::stats_assist::consent::{DataCategory, DoiScan, HelperRunId, OptInRefusal, OptIns};
use gaply_core::stats_assist::outbound::{
    build_binding_suggestion, build_claim_method, build_result_explanation, build_web_context, role_from_name, role_name, BuildRefusal,
    Outbound,
};
use gaply_core::stats_assist::query::{web_query, ContextTopic};
use gaply_core::stats_assist::response::{
    read_response, validate_binding_suggestion, validate_claim_method, validate_explanation, Rejection as ReplyRejection,
};
use gaply_core::stats_assist::session::{HelperSession, PreparedSend, PreviewId, SendRefusal, Sent};
use gaply_core::stats_assist::spec::{allowed_web_domains, HelperTask};
use gaply_core::stats_assist::transport::{TransportError, WireGate};
use gaply_core::stats_l0::input::DocumentContext;
use gaply_core::stats_l0::method::L0Verdict;
use gaply_core::stats_l0::number::{NumberNotRead, Relation};
use gaply_core::stats_l0::outcome::check_reading;
use gaply_core::stats_l0::result::{read_results, Family, PValue, TableStructure};
use gaply_core::stats_l1::binding::{
    BindingDraft, BindingProposal, OlsTarget, ProposalId, ProposalReason, ProposalSource, Rejection, WholeNumberAnswer, WholeNumbers,
};
use gaply_core::stats_l1::catalogue::{
    ContinuityVariant, L1MethodId, LevelRule, MethodCatalogue, RoleName, SlotKind, SpearmanPVariant, TVariant, Variant, VariantChoice,
};
use gaply_core::stats_l1::claim::{claims_from_reading, ClaimId, ClaimLocation, L1Claim, RevisionHash, UserClaim, UserP};
use gaply_core::stats_l1::codes::{self, Param, WireCode};
use gaply_core::stats_l1::dataset::{ColumnAlias, ColumnIssue, ColumnType, Dataset, DatasetIssue, LevelBucket, Separator};
use gaply_core::stats_l1::reproduce::ReproductionOutcome;
use gaply_core::stats_l1::run_state::{ConfirmError, EditError, L1RunState, ProposalError};
use gaply_core::stats_l1::token::ConfirmationToken;
use gaply_core::{Database, GaplyError};

use crate::stats_ai::{test_key, AiSettings, StatsAi, TaskUsage, UsageByTask, PROVIDER};
use crate::stats_l1_ingest::load_dataset;
use crate::stats_reproduction::{reproduction_view, run_stats_reproduction_measured, ReproductionView};

/// The most runs held at once; the oldest is dropped first.
pub const MAX_RUNS: usize = 4;

/* ------------------------------------------------------------------------ *
 *  Closed reason codes
 * ------------------------------------------------------------------------ */

fn w(code: &'static str) -> WireCode {
    WireCode { code, params: BTreeMap::new() }
}

fn with_num(mut c: WireCode, k: &'static str, v: u64) -> WireCode {
    c.params.insert(k, Param::Number(v));
    c
}

fn with_code(mut c: WireCode, k: &'static str, inner: WireCode) -> WireCode {
    let p = if inner.params.is_empty() { Param::Word(inner.code) } else { Param::Code(Box::new(inner)) };
    c.params.insert(k, p);
    c
}

/// A refusal: a closed code in JSON, nothing else.
fn refuse(c: WireCode) -> GaplyError {
    tracing::info!(code = c.code, "stats_l1: refused");
    GaplyError::Validation(serde_json::to_string(&c).expect("a code serialises"))
}

fn transport_code(e: TransportError) -> WireCode {
    match e {
        TransportError::NoCredential => w("no_key"),
        TransportError::Offline => w("offline"),
        TransportError::Timeout => w("timeout"),
        TransportError::Http { status } => with_num(w("http"), "status", u64::from(status)),
        TransportError::Cancelled => w("cancelled"),
        TransportError::Redirect { status } => with_num(w("redirect_refused"), "status", u64::from(status)),
        TransportError::Gated(g) => gate_code(g),
        TransportError::Failed => w("transport_failed"),
    }
}

/// The wire gate's reason, as the same top-level code a refused send gives.
fn gate_code(g: WireGate) -> WireCode {
    match g {
        WireGate::AiHelpOff => w("ai_help_off"),
        WireGate::WebContextOff => w("web_context_off"),
        WireGate::NoGrant => w("no_grant"),
        WireGate::GrantDoesNotCover => w("grant_does_not_cover"),
        WireGate::BodyChanged => w("body_changed"),
        WireGate::BudgetSpent { max } => with_num(w("budget_spent"), "max", u64::from(max)),
        WireGate::Cancelled => w("cancelled"),
        WireGate::RunClosed => w("unknown_run"),
    }
}

fn send_refusal_code(r: SendRefusal) -> WireCode {
    match r {
        SendRefusal::AiHelpOff => w("ai_help_off"),
        SendRefusal::WebContextOff => w("web_context_off"),
        SendRefusal::UnknownPreview => w("unknown_preview"),
        SendRefusal::NoGrant => w("no_grant"),
        SendRefusal::GrantDoesNotCover => w("grant_does_not_cover"),
        SendRefusal::BodyChanged => w("body_changed"),
        SendRefusal::BudgetSpent { max } => with_num(w("budget_spent"), "max", u64::from(max)),
        // A gated attempt, a cancel and a redirect are top-level codes: the
        // screen says what stopped the call, not that the network failed. A
        // cancel reads the same whether the gate or the retry wait saw it.
        SendRefusal::Transport(e @ (TransportError::Gated(_) | TransportError::Cancelled | TransportError::Redirect { .. })) => {
            transport_code(e)
        }
        SendRefusal::Transport(e) => with_code(w("transport"), "error", transport_code(e)),
    }
}

fn reply_rejection_code(r: ReplyRejection) -> WireCode {
    let inner = match r {
        ReplyRejection::MalformedJson => w("malformed_json"),
        ReplyRejection::Incomplete => w("incomplete"),
        ReplyRejection::Refused => w("refused"),
        ReplyRejection::Empty => w("empty"),
        ReplyRejection::SeveralTextParts => w("several_text_parts"),
        ReplyRejection::WrongShape => w("wrong_shape"),
        ReplyRejection::UnknownMethod => w("unknown_method"),
        ReplyRejection::VariantNotInMethod => w("variant_not_in_method"),
        ReplyRejection::UnknownRole => w("unknown_role"),
        ReplyRejection::MissingRole => w("missing_role"),
        ReplyRejection::RoleRepeated => w("role_repeated"),
        ReplyRejection::UnknownAlias => w("unknown_alias"),
        ReplyRejection::AliasInTwoRoles => w("alias_in_two_roles"),
        ReplyRejection::LevelCountInvalid => w("level_count_invalid"),
        ReplyRejection::ReliedOnUnsent => w("relied_on_unsent"),
        ReplyRejection::TextTooLong { chars, max } => with_num(with_num(w("text_too_long"), "chars", chars as u64), "max", max as u64),
        ReplyRejection::NumberNotInInput => w("number_not_in_input"),
        ReplyRejection::WrongVerdict => w("wrong_verdict"),
        ReplyRejection::NonAsciiDigit => w("non_ascii_digit"),
        ReplyRejection::NumberWord => w("number_word"),
        ReplyRejection::SignDisagrees => w("sign_disagrees"),
        ReplyRejection::MisconductWord => w("misconduct_word"),
    };
    with_code(w("reply_rejected"), "reason", inner)
}

fn opt_in_code(r: OptInRefusal) -> WireCode {
    match r {
        OptInRefusal::NotAnOptInFor { .. } => w("not_an_opt_in_for_this_task"),
        OptInRefusal::PassageTooLong { chars, max } => with_num(with_num(w("passage_too_long"), "chars", chars as u64), "max", max as u64),
        OptInRefusal::PassageNotInMethods => w("passage_not_in_methods"),
        OptInRefusal::DoiMalformed => w("doi_malformed"),
        OptInRefusal::DoiContainsData => w("doi_contains_data"),
        OptInRefusal::SentenceNotInManuscript => w("sentence_not_in_manuscript"),
        OptInRefusal::Empty => w("opt_in_empty"),
        OptInRefusal::NoSentence => w("no_sentence"),
        OptInRefusal::SentenceTooLong { chars, max } => with_num(with_num(w("sentence_too_long"), "chars", chars as u64), "max", max as u64),
    }
}

fn build_code(b: BuildRefusal) -> WireCode {
    match b {
        BuildRefusal::OptInForAnotherTask { .. } => w("opt_in_for_another_task"),
    }
}

fn role_code(r: RoleName) -> WireCode {
    w(role_name(r))
}

fn column_type_word(t: ColumnType) -> &'static str {
    match t {
        ColumnType::Numeric => "numeric",
        ColumnType::Integer => "integer",
        ColumnType::Categorical => "categorical",
        ColumnType::Text => "text",
        ColumnType::Date => "date",
        ColumnType::Empty => "empty",
    }
}

fn level_rule_code(r: LevelRule) -> WireCode {
    match r {
        LevelRule::Exactly(k) => with_num(w("exactly"), "levels", k as u64),
        LevelRule::AtLeast(k) => with_num(w("at_least"), "levels", k as u64),
    }
}

fn binding_rejection_code(r: Rejection) -> WireCode {
    let col = |c: WireCode, a: ColumnAlias| with_num(c, "column", u64::from(a.0));
    let inner = match r {
        Rejection::StaleFingerprint => w("stale_fingerprint"),
        Rejection::ClaimRevisionChanged => w("claim_revision_changed"),
        Rejection::MethodNotInCatalogue => w("method_not_in_catalogue"),
        Rejection::TransformationRequested => w("transformation_requested"),
        Rejection::PairedMethodBoundToOutcomeAndGroup => w("paired_method_bound_to_outcome_and_group"),
        Rejection::IndependentMethodBoundToTwoOutcomes => w("independent_method_bound_to_two_outcomes"),
        Rejection::RoleNotInMethod(r) => with_code(w("role_not_in_method"), "role", role_code(r)),
        Rejection::MissingRole(r) => with_code(w("missing_role"), "role", role_code(r)),
        Rejection::RoleHasTooManyColumns(r) => with_code(w("role_has_too_many_columns"), "role", role_code(r)),
        Rejection::UnknownColumn(a) => col(w("unknown_column"), a),
        Rejection::ColumnInTwoRoles(a) => col(w("column_in_two_roles"), a),
        Rejection::WrongType { role, column, found } => {
            with_code(with_code(col(w("wrong_type"), column), "role", role_code(role)), "found", w(column_type_word(found)))
        }
        Rejection::FilterOnNonGroupColumn(a) => col(w("filter_on_non_group_column"), a),
        Rejection::FilterLevelNotInColumn(a) => col(w("filter_level_not_in_column"), a),
        Rejection::LevelNotInColumn { role } => with_code(w("level_not_in_column"), "role", role_code(role)),
        Rejection::UnselectedLevel { role, unselected } => {
            with_num(with_code(w("unselected_level"), "role", role_code(role)), "unselected", unselected as u64)
        }
        Rejection::WrongGroupCount { role, selected, expected } => with_code(
            with_num(with_code(w("wrong_group_count"), "role", role_code(role)), "selected", selected as u64),
            "expected",
            level_rule_code(expected),
        ),
        Rejection::TargetMissing => w("target_missing"),
        Rejection::TargetNotAPredictor => w("target_not_a_predictor"),
        Rejection::TargetNotApplicable => w("target_not_applicable"),
        Rejection::WholeNumberAnswerNotApplicable(a) => col(w("whole_number_answer_not_applicable"), a),
    };
    with_code(w("binding_rejected"), "reason", inner)
}

fn confirm_code(e: ConfirmError) -> WireCode {
    match e {
        ConfirmError::UnknownProposal => w("unknown_proposal"),
        ConfirmError::NoDataset => w("no_dataset"),
        ConfirmError::UnknownClaim => w("unknown_claim"),
        ConfirmError::Rejected(r) => binding_rejection_code(r),
    }
}

fn proposal_code(e: ProposalError) -> WireCode {
    match e {
        ProposalError::NoDataset => w("no_dataset"),
        ProposalError::UnknownClaim(_) => w("unknown_claim"),
        ProposalError::UnknownProposal(_) => w("unknown_proposal"),
        ProposalError::MethodNotInCatalogue => w("method_not_in_catalogue"),
    }
}

fn number_not_read_code(n: NumberNotRead) -> WireCode {
    let inner = match n {
        NumberNotRead::Empty => w("empty"),
        NumberNotRead::Comma => w("comma"),
        NumberNotRead::Percent => w("percent"),
        NumberNotRead::ScientificNotation => w("scientific_notation"),
        NumberNotRead::Range => w("range"),
        NumberNotRead::InternalSpace => w("internal_space"),
        NumberNotRead::Malformed => w("malformed"),
        NumberNotRead::TooManyDigits => w("too_many_digits"),
        NumberNotRead::DecimalInCount => w("decimal_in_count"),
        NumberNotRead::NegativeCount => w("negative_count"),
        NumberNotRead::PValueOutOfRange => w("p_value_out_of_range"),
    };
    with_code(w("number_not_read"), "reason", inner)
}

fn edit_code(e: EditError) -> WireCode {
    match e {
        EditError::UnknownClaim => w("unknown_claim"),
        EditError::Number(n) => number_not_read_code(n),
    }
}

fn citation_drop_word(d: CitationDrop) -> &'static str {
    match d {
        CitationDrop::Malformed => "malformed",
        CitationDrop::NotHttps => "not_https",
        CitationDrop::BadHost => "bad_host",
        CitationDrop::DomainNotAllowed => "domain_not_allowed",
        CitationDrop::RangeOutsideText => "range_outside_text",
        CitationDrop::RangeDoesNotNameSource => "range_does_not_name_source",
        CitationDrop::RangeSpansSentences => "range_spans_sentences",
    }
}

fn l0_verdict_code(v: &L0Verdict) -> WireCode {
    match v {
        L0Verdict::Consistent(None) => w("consistent"),
        L0Verdict::Consistent(Some(n)) => with_code(w("consistent"), "note", codes::check_note(*n)),
        L0Verdict::Inconsistent { conclusion_changes } => with_code(
            w("inconsistent"),
            "conclusion_changes",
            w(match conclusion_changes {
                Some(true) => "yes",
                Some(false) => "no",
                None => "not_assessed",
            }),
        ),
        L0Verdict::Concern(r) => with_code(w("concern"), "reason", codes::concern_reason(*r)),
        L0Verdict::CouldNotCheck(r) => with_code(w("could_not_check"), "reason", codes::l0_could_not_check_reason(*r)),
    }
}

fn column_issue_code(i: ColumnIssue) -> WireCode {
    match i {
        ColumnIssue::EmptyHeader => w("empty_header"),
        ColumnIssue::DuplicateHeader { first } => with_num(w("duplicate_header"), "first", u64::from(first.0)),
        ColumnIssue::AllMissing => w("all_missing"),
        ColumnIssue::MixedTypes { numbers, texts } => with_num(with_num(w("mixed_types"), "numbers", numbers as u64), "texts", texts as u64),
        ColumnIssue::NonFinite { cells } => with_num(w("non_finite"), "cells", cells as u64),
        ColumnIssue::CategoryExplosion { distinct } => with_num(w("category_explosion"), "distinct", distinct as u64),
        ColumnIssue::RaggedRows { short_rows } => with_num(w("ragged_rows"), "short_rows", short_rows as u64),
        ColumnIssue::UserMissingApplied { cells } => with_num(w("user_missing_applied"), "cells", cells as u64),
    }
}

fn dataset_issue_code(i: DatasetIssue) -> WireCode {
    match i {
        DatasetIssue::WeightVariableDeclared { column } => with_num(w("weight_variable_declared"), "column", u64::from(column.0)),
        DatasetIssue::WeightVariableUnknown => w("weight_variable_unknown"),
        DatasetIssue::SourceMetadataNotKnown => w("source_metadata_not_known"),
        DatasetIssue::OverlongRows { rows } => with_num(w("overlong_rows"), "rows", rows as u64),
        DatasetIssue::LooksSeparatedBy { separator } => with_code(
            w("looks_separated_by"),
            "separator",
            w(match separator {
                Separator::Semicolon => "semicolon",
                Separator::Tab => "tab",
            }),
        ),
    }
}

/// The shared code's word (`stats_l1::codes::variant`), so a proposal and a
/// finished result name a variant the same way.
fn variant_word(v: Variant) -> &'static str {
    codes::variant(v).code
}

fn variant_from_word(kind_of: L1MethodId, word: &str) -> Option<Variant> {
    let kind = MethodCatalogue::entry(kind_of).variant_kind;
    [
        Variant::T(TVariant::Student),
        Variant::T(TVariant::Welch),
        Variant::Continuity(ContinuityVariant::Yes),
        Variant::Continuity(ContinuityVariant::No),
        Variant::SpearmanP(SpearmanPVariant::TApproximation),
        Variant::SpearmanP(SpearmanPVariant::Exact),
    ]
    .into_iter()
    .find(|v| v.kind() == kind && variant_word(*v) == word)
}

fn section_word(k: SectionKind) -> &'static str {
    match k {
        SectionKind::Abstract => "abstract",
        SectionKind::Introduction => "introduction",
        SectionKind::Methods => "methods",
        SectionKind::Results => "results",
        SectionKind::Discussion => "discussion",
        SectionKind::Conclusion => "conclusion",
        SectionKind::References => "references",
        SectionKind::Other => "other",
    }
}

fn family_word(f: Family) -> &'static str {
    match f {
        Family::T => "t",
        Family::F => "F",
        Family::ChiSquare => "chi_square",
        Family::R => "r",
        Family::Z => "z",
    }
}

fn family_from_word(s: &str) -> Option<Family> {
    [Family::T, Family::F, Family::ChiSquare, Family::R, Family::Z].into_iter().find(|f| family_word(*f) == s)
}

fn relation_word(r: Relation) -> &'static str {
    match r {
        Relation::Eq => "=",
        Relation::Lt => "<",
        Relation::Le => "<=",
        Relation::Gt => ">",
        Relation::Ge => ">=",
    }
}

fn relation_from_word(s: &str) -> Option<Relation> {
    [Relation::Eq, Relation::Lt, Relation::Le, Relation::Gt, Relation::Ge].into_iter().find(|r| relation_word(*r) == s)
}

fn level_bucket_word(b: LevelBucket) -> &'static str {
    match b {
        LevelBucket::Two => "two",
        LevelBucket::ThreeToTen => "three_to_ten",
        LevelBucket::MoreThanTen => "more_than_ten",
    }
}

fn category_from_word(s: &str) -> Option<DataCategory> {
    match s {
        "column_headers" => Some(DataCategory::ColumnHeaders),
        "claim_sentence" => Some(DataCategory::ClaimSentence),
        "methods_passage" => Some(DataCategory::MethodsPassage),
        "paper_doi" => Some(DataCategory::PaperDoi),
        _ => None,
    }
}

fn topic_from_word(s: &str) -> Option<ContextTopic> {
    match s {
        "assumptions" => Some(ContextTopic::Assumptions),
        "reporting_guidance" => Some(ContextTopic::ReportingGuidance),
        "effect_size_reporting" => Some(ContextTopic::EffectSizeReporting),
        "choosing_a_variant" => Some(ContextTopic::ChoosingAVariant),
        _ => None,
    }
}

/* ------------------------------------------------------------------------ *
 *  The registry
 * ------------------------------------------------------------------------ */

/// What a preview was built for, to act on its reply.
struct PreviewMeta {
    claim: ClaimId,
    query_sent: Option<String>,
}

/// One run. Never `Serialize`; never `Debug` (it holds the manuscript and
/// the rows).
pub struct L1Run {
    data_path: PathBuf,
    ex: ExtractionResult,
    ctx: DocumentContext,
    state: L1RunState,
    level0: HashMap<ClaimId, WireCode>,
    helper: HelperSession,
    previews: HashMap<PreviewId, PreviewMeta>,
    /// Token handle (the token's own 64 hex characters) to what it was
    /// issued for.
    tokens: HashMap<String, (ClaimId, RevisionHash, ConfirmationToken)>,
    reproduction: Option<ReproductionOutcome>,
    usage: UsageByTask,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct L1Runs {
    next: AtomicU64,
    runs: Mutex<VecDeque<(u64, Arc<Mutex<L1Run>>)>>,
}

impl L1Runs {
    fn insert(&self, make: impl FnOnce(u64) -> L1Run) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let run = Arc::new(Mutex::new(make(id)));
        let mut runs = self.runs.lock().unwrap();
        runs.push_back((id, run));
        while runs.len() > MAX_RUNS {
            runs.pop_front();
        }
        id
    }

    fn get(&self, id: u64) -> Result<Arc<Mutex<L1Run>>, GaplyError> {
        self.runs.lock().unwrap().iter().find(|(r, _)| *r == id).map(|(_, run)| run.clone()).ok_or_else(|| refuse(w("unknown_run")))
    }

    /// Drop a run. True if it was held.
    pub fn close(&self, id: u64) -> bool {
        let mut runs = self.runs.lock().unwrap();
        let before = runs.len();
        runs.retain(|(r, _)| *r != id);
        runs.len() != before
    }

    pub fn len(&self) -> usize {
        self.runs.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/* ------------------------------------------------------------------------ *
 *  Views: what crosses to the webview. Headers and level labels appear here
 *  for display on this device; this module never sends a view anywhere else.
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L1Handle {
    pub run: u64,
    pub id: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1OpenView {
    pub run: u64,
    pub claims: Vec<L1ClaimView>,
    /// Results Level 0 read that could not be located in a sentence.
    pub not_located: usize,
    pub schema: L1SchemaView,
    /// The local proposer's suggestions, per claim.
    pub proposals: Vec<L1ProposalView>,
    /// The method catalogue, so the screen offers exactly its methods, slots
    /// and variants rather than a copy that could drift.
    pub catalogue: Vec<L1MethodView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1MethodView {
    pub method: &'static str,
    pub slots: Vec<L1SlotView>,
    /// The stated variants a user may tick; empty for a method without one.
    pub variants: Vec<&'static str>,
    /// OLS: the overall F or one predictor's coefficient must be chosen.
    pub needs_target: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1SlotView {
    pub role: &'static str,
    /// `numeric`, `group` or `numeric_list`.
    pub kind: &'static str,
    /// A group slot's level rule (`exactly` or `at_least`, with `levels`).
    pub levels: Option<WireCode>,
    /// A list slot's minimum number of columns.
    pub min: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1ClaimView {
    pub claim_id: usize,
    pub revision: String,
    pub section_index: usize,
    pub section: &'static str,
    pub paragraph: usize,
    pub sentence_index: usize,
    /// The sentence, for display on this device.
    pub sentence: String,
    pub family: &'static str,
    pub statistic: String,
    pub df: Vec<String>,
    pub n: Option<String>,
    pub p: String,
    /// Level 0's verdict on the printed numbers; `None` for a claim the user
    /// added or edited.
    pub level0: Option<WireCode>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1SchemaView {
    pub rows: usize,
    pub columns: Vec<L1ColumnView>,
    pub issues: Vec<WireCode>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1ColumnView {
    pub alias: String,
    /// The original header, for display on this device.
    pub header: String,
    pub column_type: &'static str,
    pub has_missing: bool,
    pub level_bucket: Option<&'static str>,
    /// Level codes and labels, for display on this device.
    pub levels: Vec<L1LevelView>,
    /// Including the missing-code flags (`user_missing_applied`).
    pub issues: Vec<WireCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1LevelView {
    pub code: String,
    pub label: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1ProposalView {
    pub proposal: L1Handle,
    pub claim_id: usize,
    pub method: String,
    pub roles: Vec<L1RoleView>,
    pub levels: Vec<L1LevelSelectionView>,
    pub variant: &'static str,
    /// Who set the variant: `stated_in_paper` only under Step 2's word-list
    /// rule, `user_ticked`, or `not_stated`.
    pub variant_set_by: WireCode,
    /// `local`, `ai_suggestion` or `manual`.
    pub source: &'static str,
    /// The local proposer's closed reason, when local.
    pub reason: Option<&'static str>,
    /// The bound whole-number columns and the answer this proposal carries
    /// for each (evaluation fixes, part C, B-1): the Confirm step asks about
    /// exactly these. Display on this device only; never in a request.
    pub whole_numbers: Vec<L1WholeNumberView>,
}

/// One bound whole-number column and its answer: `rounded` ("Rounded, or not
/// sure", the default) or `exact` ("Exact whole numbers").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1WholeNumberView {
    pub alias: String,
    pub answer: &'static str,
}

/// The wire word of an answer.
pub fn whole_number_word(a: WholeNumbers) -> &'static str {
    match a {
        WholeNumbers::RoundedOrNotSure => "rounded",
        WholeNumbers::Exact => "exact",
    }
}

pub fn whole_number_views(answers: &[WholeNumberAnswer]) -> Vec<L1WholeNumberView> {
    answers.iter().map(|w| L1WholeNumberView { alias: w.column.name(), answer: whole_number_word(w.answer) }).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1RoleView {
    pub role: &'static str,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1LevelSelectionView {
    pub role: &'static str,
    pub levels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1RevisionView {
    pub claim_id: usize,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1PreviewView {
    pub preview: L1Handle,
    pub task: &'static str,
    /// The exact request body.
    pub body: String,
    pub sha256: String,
    pub categories: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1SourceView {
    pub url: String,
    pub title: String,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1ContextItemView {
    /// Model text: shown as plain text.
    pub sentence: String,
    pub sources: Vec<L1SourceView>,
    pub retrieved_at: u64,
    pub query_sent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1SuggestedRoleView {
    pub role: &'static str,
    pub alias: Option<String>,
    pub levels_to_select: Option<u32>,
}

/// What a helper call returned. Every string a model wrote is in a field
/// named for it and is shown as plain text.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum L1HelperResultView {
    ClaimMethod {
        proposal: Option<L1Handle>,
        method: Option<&'static str>,
        variant_hint: Option<&'static str>,
        model_reason: String,
        relied_on: Vec<&'static str>,
    },
    BindingSuggestion {
        proposal: L1Handle,
        roles: Vec<L1SuggestedRoleView>,
        model_reason: String,
        relied_on: Vec<&'static str>,
    },
    Explanation {
        model_text: String,
    },
    Context {
        items: Vec<L1ContextItemView>,
        uncited_sentences: usize,
        dropped_citations: Vec<&'static str>,
        empty_message: Option<&'static str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1ConfirmView {
    pub token: String,
    pub claim_id: usize,
    pub rows_read: usize,
    pub rows_filtered_out: usize,
    pub rows_excluded_missing: usize,
    pub n_used: usize,
    pub suspect_missing_codes: Vec<L1SuspectCodeView>,
    /// The whole-number answers the binding was confirmed with.
    pub whole_numbers: Vec<L1WholeNumberView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1SuspectCodeView {
    pub alias: String,
    pub code: i32,
    pub cells: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1UsageView {
    pub tasks: Vec<L1TaskUsageView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1TaskUsageView {
    pub task: &'static str,
    /// Every request that left the device for this task: each attempt,
    /// retries and failed calls included.
    pub requests: u32,
    /// What the replies reported; a failed call reports none.
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiKeyStatusView {
    pub saved: bool,
    pub provider: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiKeyTestView {
    pub result: String,
}

/* ------------------------------------------------------------------------ *
 *  Inputs
 * ------------------------------------------------------------------------ */

/// A claim the user types or edits: printed strings, never floats.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L1ClaimInput {
    /// `t`, `F`, `chi_square`, `r` or `z`.
    pub family: String,
    pub statistic: String,
    pub df: Vec<String>,
    pub n: Option<String>,
    /// `None` for a p printed only as `ns`.
    pub p: Option<String>,
    /// `=`, `<`, `<=`, `>` or `>=`; `=` when absent.
    pub p_relation: Option<String>,
    pub sentence: Option<String>,
}

/// One helper task's input. Each variant carries only what its task needs.
///
/// Unknown fields are refused (evaluation fixes, part B): the web query has
/// no free-text edit and the DOI field holds a DOI only, so an `editedQuery`
/// or a `paper` field from an older screen never reaches a request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "task", rename_all = "snake_case", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum HelperTaskInput {
    ClaimMethod { claim_id: usize, methods_passage: Option<String> },
    BindingSuggestion { claim_id: usize, method_id: String },
    ResultExplanation { claim_id: usize },
    /// The query's three closed parts: a method, a variant word or none
    /// (`None` names no variant), and a topic; and an optional DOI.
    WebContext {
        claim_id: usize,
        method_id: String,
        #[serde(default)]
        variant: Option<String>,
        topic: String,
        #[serde(default)]
        doi: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleEdit {
    pub role: String,
    pub aliases: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelEdit {
    pub role: String,
    pub levels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterEdit {
    pub alias: u32,
    pub levels: Vec<String>,
}

/// The user's edits to a proposal before confirming. Recorded as Manual,
/// edited from the proposal's source.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingEdits {
    pub method_id: String,
    pub roles: Vec<RoleEdit>,
    pub levels: Vec<LevelEdit>,
    pub filters: Vec<FilterEdit>,
    /// The variant the user ticks as "the paper states this"; `None` leaves
    /// it to the paper's own words (Step 2's rule).
    pub variant_ticked: Option<String>,
    /// `overall_f`, or a predictor's alias number.
    pub ols_overall_f: Option<bool>,
    pub ols_predictor: Option<u32>,
    /// Bound whole-number columns the user says hold EXACT whole numbers
    /// (part C, B-1); every other one is rounded, or not sure. Absent means
    /// none, the default.
    #[serde(default)]
    pub exact_whole_numbers: Vec<u32>,
}

/* ------------------------------------------------------------------------ *
 *  Building views
 * ------------------------------------------------------------------------ */

fn claim_view(c: &L1Claim, level0: Option<&WireCode>) -> L1ClaimView {
    let r = &c.result;
    L1ClaimView {
        claim_id: c.id.0,
        revision: c.revision.as_str().to_string(),
        section_index: c.location.section_index,
        section: section_word(c.location.section),
        paragraph: c.location.paragraph,
        sentence_index: c.location.sentence,
        sentence: c.sentence.clone(),
        family: family_word(r.family),
        statistic: r.statistic.raw().to_string(),
        df: r.df.iter().map(|d| d.raw().to_string()).collect(),
        n: r.n.as_ref().map(|n| n.raw().to_string()),
        p: match &r.p {
            PValue::Reported(n) => format!("{} {}", relation_word(n.relation()), n.raw()),
            PValue::NotSignificant => "ns".to_string(),
        },
        level0: level0.cloned(),
    }
}

fn source_word(s: &ProposalSource) -> &'static str {
    match s {
        ProposalSource::Local { .. } => "local",
        ProposalSource::AiSuggestion { .. } => "ai_suggestion",
        ProposalSource::Manual { .. } => "manual",
    }
}

fn proposal_view(run: u64, p: &BindingProposal, dataset: Option<&Dataset>) -> L1ProposalView {
    let b = &p.binding;
    let variant = b.variant.variant();
    L1ProposalView {
        proposal: L1Handle { run, id: p.id.0 },
        claim_id: p.claim.0,
        method: b.method_id.clone(),
        roles: b.roles.iter().map(|r| L1RoleView { role: role_name(r.role), aliases: r.columns.iter().map(|a| a.name()).collect() }).collect(),
        levels: b.selected_levels.iter().map(|l| L1LevelSelectionView { role: role_name(l.role), levels: l.levels.clone() }).collect(),
        variant: variant_word(variant),
        variant_set_by: codes::variant_set_by(b.variant.set_by()),
        source: source_word(&p.source),
        reason: match &p.source {
            ProposalSource::Local { reason: ProposalReason::HeaderMatchesSentence, .. } => Some("header_matches_sentence"),
            ProposalSource::Local { reason: ProposalReason::FirstEligibleColumns, .. } => Some("first_eligible_columns"),
            ProposalSource::AiSuggestion { .. } | ProposalSource::Manual { .. } => None,
        },
        whole_numbers: dataset.map(|d| whole_number_views(&b.whole_number_answers(d))).unwrap_or_default(),
    }
}

fn catalogue_view() -> Vec<L1MethodView> {
    const STATED: [Variant; 6] = [
        Variant::T(TVariant::Student),
        Variant::T(TVariant::Welch),
        Variant::Continuity(ContinuityVariant::Yes),
        Variant::Continuity(ContinuityVariant::No),
        Variant::SpearmanP(SpearmanPVariant::TApproximation),
        Variant::SpearmanP(SpearmanPVariant::Exact),
    ];
    MethodCatalogue::ENTRIES
        .iter()
        .map(|e| L1MethodView {
            method: e.id.id(),
            slots: e
                .slots
                .iter()
                .map(|s| match s.kind {
                    SlotKind::Numeric => L1SlotView { role: role_name(s.role), kind: "numeric", levels: None, min: None },
                    SlotKind::Group { levels } => L1SlotView { role: role_name(s.role), kind: "group", levels: Some(level_rule_code(levels)), min: None },
                    SlotKind::NumericList { min } => L1SlotView { role: role_name(s.role), kind: "numeric_list", levels: None, min: Some(min) },
                })
                .collect(),
            variants: STATED.iter().filter(|v| v.kind() == e.variant_kind).map(|v| variant_word(*v)).collect(),
            needs_target: e.needs_target,
        })
        .collect()
}

fn schema_view(state: &L1RunState) -> L1SchemaView {
    let Some(ds) = state.dataset() else { return L1SchemaView { rows: 0, columns: Vec::new(), issues: Vec::new() } };
    let s = &ds.schema;
    L1SchemaView {
        rows: s.rows,
        columns: s
            .columns
            .iter()
            .map(|c| L1ColumnView {
                alias: c.alias.name(),
                header: c.header.clone(),
                column_type: column_type_word(c.column_type),
                has_missing: c.has_missing,
                level_bucket: c.level_bucket.map(level_bucket_word),
                levels: c
                    .levels
                    .as_ref()
                    .map(|l| l.levels.iter().map(|x| L1LevelView { code: x.code.clone(), label: x.label.clone(), count: x.count }).collect())
                    .unwrap_or_default(),
                issues: c.issues.iter().map(|i| column_issue_code(*i)).collect(),
            })
            .collect(),
        issues: s.issues.iter().map(|i| dataset_issue_code(*i)).collect(),
    }
}

fn usage_view(u: &UsageByTask) -> L1UsageView {
    L1UsageView {
        tasks: HelperTask::ALL
            .iter()
            .map(|t| {
                let x = u.get(t).copied().unwrap_or_default();
                L1TaskUsageView { task: t.id(), requests: x.requests, input_tokens: x.input_tokens, output_tokens: x.output_tokens }
            })
            .collect(),
    }
}

/* ------------------------------------------------------------------------ *
 *  The commands, without Tauri
 * ------------------------------------------------------------------------ */

fn lock(run: &Arc<Mutex<L1Run>>) -> std::sync::MutexGuard<'_, L1Run> {
    run.lock().unwrap_or_else(|p| p.into_inner())
}

/// Open a manuscript and a data file: Level 0 on the manuscript, the
/// dataset typed, claims located, local proposals made. No network.
#[tracing::instrument(skip_all)]
pub fn open(db: &Database, runs: &L1Runs, ai: &StatsAi, manuscript: &Path, data: &Path) -> Result<L1OpenView, GaplyError> {
    let pre = import_guard::preflight(db, manuscript, gaply_core::now_epoch()).map_err(|_| refuse(w("manuscript_unreadable")))?;
    if pre.is_refused() {
        return Err(refuse(w("manuscript_refused")));
    }
    // Unreachable today: preflight parses the file with the same parser and
    // stops a file that does not parse. Kept, under its own code, in case
    // that changes; `a_document_that_does_not_parse_is_stopped_by_the_preflight`
    // pins the premise.
    let ex = extract::extract_path(manuscript).map_err(|_| refuse(w("manuscript_not_extracted")))?;
    let is_docx = manuscript.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("docx"));
    let reading = read_results(&ex, if is_docx { TableStructure::Declared } else { TableStructure::NotAvailable });
    let ctx = DocumentContext::read(&ex, &reading);
    let (claims, not_located) = claims_from_reading(&ex, &reading);
    let level0_outcome = check_reading(&ex, reading);
    let dataset = load_dataset(data).map_err(|r| refuse(with_code(w("data_refused"), "reason", codes::l1_ingest_refusal(r))))?;

    let mut level0 = HashMap::new();
    for c in &claims {
        if let Some(o) = level0_outcome.claims.iter().find(|o| o.result.id == c.result.id) {
            level0.insert(c.id, l0_verdict_code(&o.verdict));
        }
    }
    let mut state = L1RunState::new();
    state.load_dataset(dataset);
    state.set_claims(claims);
    state.set_context(ctx.clone());
    let ids: Vec<ClaimId> = state.claims().iter().map(|c| c.id).collect();
    for id in ids {
        let _ = state.propose_local(id);
    }
    let settings = ai.settings();
    let id = runs.insert(|id| L1Run {
        data_path: data.to_path_buf(),
        ex,
        ctx,
        state,
        level0,
        helper: HelperSession::new(HelperRunId(id), settings.help(), settings.model_id()),
        previews: HashMap::new(),
        tokens: HashMap::new(),
        reproduction: None,
        usage: BTreeMap::new(),
        cancel: Arc::new(AtomicBool::new(false)),
    });
    let run = runs.get(id)?;
    let r = lock(&run);
    tracing::info!(run = id, claims = r.state.claims().len(), not_located, "stats_l1: opened");
    Ok(L1OpenView {
        run: id,
        claims: r.state.claims().iter().map(|c| claim_view(c, r.level0.get(&c.id))).collect(),
        not_located,
        schema: schema_view(&r.state),
        proposals: r.state.proposals().iter().map(|p| proposal_view(id, p, r.state.dataset())).collect(),
        catalogue: catalogue_view(),
    })
}

fn user_claim<'a>(input: &'a L1ClaimInput, location: ClaimLocation) -> Result<UserClaim<'a>, GaplyError> {
    let family = family_from_word(&input.family).ok_or_else(|| refuse(w("unknown_family")))?;
    let p = match &input.p {
        None => UserP::NotSignificant,
        Some(raw) => UserP::Reported {
            raw,
            relation: match input.p_relation.as_deref() {
                None => Relation::Eq,
                Some(s) => relation_from_word(s).ok_or_else(|| refuse(w("unknown_relation")))?,
            },
        },
    };
    Ok(UserClaim {
        family,
        statistic: &input.statistic,
        df: input.df.iter().map(String::as_str).collect(),
        n: input.n.as_deref(),
        p,
        location,
        sentence: input.sentence.as_deref(),
    })
}

/// Edit a claim: a new revision, and that claim's confirmation dies.
#[tracing::instrument(skip_all)]
pub fn edit_claim(runs: &L1Runs, run: u64, claim: usize, input: &L1ClaimInput) -> Result<L1RevisionView, GaplyError> {
    let r = runs.get(run)?;
    let mut r = lock(&r);
    let location = r.state.claim(ClaimId(claim)).ok_or_else(|| refuse(w("unknown_claim")))?.location;
    let uc = user_claim(input, location)?;
    let rev = r.state.edit_claim(ClaimId(claim), &uc).map_err(|e| refuse(edit_code(e)))?;
    r.level0.remove(&ClaimId(claim));
    Ok(L1RevisionView { claim_id: claim, revision: rev.as_str().to_string() })
}

/// Add a claim the reader did not find.
#[tracing::instrument(skip_all)]
pub fn add_claim(runs: &L1Runs, run: u64, input: &L1ClaimInput) -> Result<L1RevisionView, GaplyError> {
    let r = runs.get(run)?;
    let mut r = lock(&r);
    let location = ClaimLocation { section_index: 0, section: SectionKind::Other, paragraph: 0, sentence: 0 };
    let uc = user_claim(input, location)?;
    let id = r.state.add_claim(&uc).map_err(|n| refuse(number_not_read_code(n)))?;
    let rev = r.state.claim(id).expect("just added").revision.as_str().to_string();
    let _ = r.state.propose_local(id);
    Ok(L1RevisionView { claim_id: id.0, revision: rev })
}

/// The proposals now held, for the screen to refresh.
pub fn proposals(runs: &L1Runs, run: u64) -> Result<Vec<L1ProposalView>, GaplyError> {
    let r = runs.get(run)?;
    let r = lock(&r);
    Ok(r.state.proposals().iter().map(|p| proposal_view(run, p, r.state.dataset())).collect())
}

fn sync_helper(r: &mut L1Run, ai: &StatsAi) {
    let s = ai.settings();
    r.helper.set_setting(s.help());
    r.helper.set_model(s.model_id());
}

/// Build the exact request for one helper task. No network.
#[tracing::instrument(skip_all)]
pub fn helper_preview(runs: &L1Runs, ai: &StatsAi, run: u64, input: &HelperTaskInput, categories: &[String]) -> Result<L1PreviewView, GaplyError> {
    let r = runs.get(run)?;
    let mut r = lock(&r);
    sync_helper(&mut r, ai);
    let cats: Vec<DataCategory> = categories
        .iter()
        .map(|c| category_from_word(c).ok_or_else(|| refuse(w("unknown_category"))))
        .collect::<Result<_, _>>()?;
    let task = match input {
        HelperTaskInput::ClaimMethod { .. } => HelperTask::ClaimMethod,
        HelperTaskInput::BindingSuggestion { .. } => HelperTask::BindingSuggestion,
        HelperTaskInput::ResultExplanation { .. } => HelperTask::ResultExplanation,
        HelperTaskInput::WebContext { .. } => HelperTask::WebContext,
    };
    let opt = OptIns::new(task, &cats).map_err(|e| refuse(opt_in_code(e)))?;
    let claim_of = |r: &L1Run, id: usize| r.state.claim(ClaimId(id)).cloned().ok_or_else(|| refuse(w("unknown_claim")));
    let method_of = |id: &str| L1MethodId::from_id(id).ok_or_else(|| refuse(w("unknown_method")));
    let (outbound, claim, query_sent) = match input {
        HelperTaskInput::ClaimMethod { claim_id, methods_passage } => {
            let claim = claim_of(&r, *claim_id)?;
            let sentence = opt.claim_sentence(&claim, &r.ex).map_err(|e| refuse(opt_in_code(e)))?;
            let passage = match (opt.is_ticked(DataCategory::MethodsPassage), methods_passage) {
                (true, Some(p)) => opt.methods_passage(&r.ex, p).map_err(|e| refuse(opt_in_code(e)))?,
                (true, None) => return Err(refuse(opt_in_code(OptInRefusal::Empty))),
                (false, _) => None,
            };
            (Outbound::ClaimMethod(build_claim_method(&claim, &r.ctx, sentence, passage).map_err(|e| refuse(build_code(e)))?), claim.id, None)
        }
        HelperTaskInput::BindingSuggestion { claim_id, method_id } => {
            let claim = claim_of(&r, *claim_id)?;
            let method = method_of(method_id)?;
            let schema = &r.state.dataset().ok_or_else(|| refuse(w("no_dataset")))?.schema;
            let headers = opt.column_headers(schema);
            let sentence = opt.claim_sentence(&claim, &r.ex).map_err(|e| refuse(opt_in_code(e)))?;
            (
                Outbound::BindingSuggestion(build_binding_suggestion(method, schema, headers, sentence).map_err(|e| refuse(build_code(e)))?),
                claim.id,
                None,
            )
        }
        HelperTaskInput::ResultExplanation { claim_id } => {
            let rep = r
                .reproduction
                .as_ref()
                .and_then(|o| o.claims.iter().find(|c| c.claim == ClaimId(*claim_id)))
                .ok_or_else(|| refuse(w("no_finished_result")))?;
            (Outbound::ResultExplanation(build_result_explanation(rep, &r.ctx)), ClaimId(*claim_id), None)
        }
        HelperTaskInput::WebContext { claim_id, method_id, variant, topic, doi } => {
            let claim = claim_of(&r, *claim_id)?;
            let method = method_of(method_id)?;
            let topic = topic_from_word(topic).ok_or_else(|| refuse(w("unknown_topic")))?;
            let kind = MethodCatalogue::entry(method).variant_kind;
            // The variant is a closed word picked on the screen, or none.
            let variant = match variant {
                Some(word) => variant_from_word(method, word).ok_or_else(|| refuse(w("unknown_variant")))?,
                None => Variant::not_stated(kind),
            };
            // Built only from the three closed parts: nothing typed enters.
            let query = web_query(method, variant, topic);
            let doi = match (opt.is_ticked(DataCategory::PaperDoi), doi) {
                (true, Some(d)) => {
                    let dataset = r.state.dataset().ok_or_else(|| refuse(w("no_dataset")))?;
                    let file_name = r.data_path.file_name().and_then(|f| f.to_str());
                    let scan = DoiScan::new(dataset, file_name);
                    opt.doi(d, &scan).map_err(|e| refuse(opt_in_code(e)))?
                }
                (true, None) => return Err(refuse(opt_in_code(OptInRefusal::Empty))),
                (false, _) => None,
            };
            let query_sent = query.as_str().to_string();
            (Outbound::WebContext(build_web_context(&query, doi).map_err(|e| refuse(build_code(e)))?), claim.id, Some(query_sent))
        }
    };
    let p = r.helper.preview(outbound).map_err(|e| refuse(send_refusal_code(e)))?;
    r.previews.insert(p.id, PreviewMeta { claim, query_sent });
    tracing::info!(run, task = p.task.id(), "stats_l1: preview built");
    Ok(L1PreviewView { preview: L1Handle { run, id: p.id.0 }, task: p.task.id(), body: p.body, sha256: p.sha256, categories: p.categories })
}

/// Count what left the device, whatever came back: `requests` is the number
/// the transport put on the wire (each attempt, a retry and a failed one
/// included), and the tokens are what a reply reported.
fn record_usage(r: &mut L1Run, task: HelperTask, requests: u32, sent: Option<&Sent>) {
    let u = r.usage.entry(task).or_insert_with(TaskUsage::default);
    u.requests += requests;
    if let Some(sent) = sent {
        u.input_tokens += sent.diagnostic.input_tokens.unwrap_or(0);
        u.output_tokens += sent.diagnostic.output_tokens.unwrap_or(0);
    }
}

/// Prepare under the lock, transmit outside it, record under it again.
///
/// **The wire gate** (evaluation fixes, part B) runs immediately before the
/// request is handed over and again before every attempt the transport puts
/// on the wire, a retry included: AI help (read from `StatsAi`, which
/// `stats_ai_setting_set` writes), then under the run's lock the cancel flag
/// (`stats_l1_helper_cancel`) and the session's grant, hash and budget
/// (`stats_l1_helper_revoke`). A failed gate sends nothing.
fn transmit(run: &Arc<Mutex<L1Run>>, ai: &StatsAi, prepared: PreparedSend) -> Result<Sent, GaplyError> {
    let cancel = lock(run).cancel.clone();
    let transport = (ai.transport)(cancel);
    let task = prepared.task();
    let (preview, body_sha256) = (prepared.preview(), prepared.body_sha256());
    let gate = || -> Result<(), WireGate> {
        let help = ai.settings().help();
        if !help.allows(task) {
            return Err(if help.ai_help { WireGate::WebContextOff } else { WireGate::AiHelpOff });
        }
        let r = lock(run);
        if r.cancel.load(Ordering::SeqCst) {
            return Err(WireGate::Cancelled);
        }
        r.helper.wire_check(preview, &body_sha256)
    };
    let before = transport.requests_sent();
    let result = prepared.transmit_gated(&*transport, &gate);
    let requests = transport.requests_sent().saturating_sub(before);
    record_usage(&mut lock(run), task, requests, result.as_ref().ok());
    result.map_err(|e| refuse(send_refusal_code(e)))
}

/// Send a preview: the AI help setting, the grant (created or reused for the
/// preview's task and categories), the hash and the budget are checked in the
/// core session; then the reply is validated and turned into a suggestion,
/// an explanation or context items. Suggestions become proposals.
#[tracing::instrument(skip_all)]
pub fn helper_send(runs: &L1Runs, ai: &StatsAi, run: u64, preview: L1Handle, now_unix: u64) -> Result<L1HelperResultView, GaplyError> {
    if preview.run != run {
        return Err(refuse(w("preview_from_another_run")));
    }
    let handle = runs.get(run)?;
    let id = PreviewId(preview.id);
    let (prepared, outbound, meta_claim, query_sent) = {
        let mut r = lock(&handle);
        sync_helper(&mut r, ai);
        let meta = r.previews.get(&id).ok_or_else(|| refuse(w("unknown_preview")))?;
        let (meta_claim, query_sent) = (meta.claim, meta.query_sent.clone());
        // The setting first, then the key: a closed code, before any grant
        // is made and before any network.
        let task = r.helper.outbound(id).map(Outbound::task).ok_or_else(|| refuse(w("unknown_preview")))?;
        let help = ai.settings().help();
        if !help.allows(task) {
            return Err(refuse(send_refusal_code(if help.ai_help { SendRefusal::WebContextOff } else { SendRefusal::AiHelpOff })));
        }
        if !ai.credentials.has_key().unwrap_or(false) {
            return Err(refuse(w("no_key")));
        }
        r.helper.grant_from_preview(id).map_err(|e| refuse(send_refusal_code(e)))?;
        let prepared = r.helper.prepare_send(id).map_err(|e| refuse(send_refusal_code(e)))?;
        let outbound = r.helper.outbound(id).cloned().ok_or_else(|| refuse(w("unknown_preview")))?;
        r.cancel.store(false, Ordering::SeqCst);
        (prepared, outbound, meta_claim, query_sent)
    };
    let sent = transmit(&handle, ai, prepared)?;
    let request_id = sent.diagnostic.request_id.clone().unwrap_or_default();
    let read = |s: &Sent| read_response(&s.reply.body);
    match &outbound {
        Outbound::ClaimMethod(o) => {
            let s = read(&sent).and_then(|out| validate_claim_method(&out, o)).map_err(|e| refuse(reply_rejection_code(e)))?;
            let proposal = match s.method {
                Some(m) => {
                    let draft = BindingDraft::new(m, VariantChoice::not_stated(MethodCatalogue::entry(m).variant_kind));
                    let source = ProposalSource::AiSuggestion { task_id: HelperTask::ClaimMethod.id().into(), request_id };
                    let pid = lock(&handle).state.add_proposal(meta_claim, draft, source).map_err(|e| refuse(proposal_code(e)))?;
                    Some(L1Handle { run, id: pid.0 })
                }
                None => None,
            };
            Ok(L1HelperResultView::ClaimMethod {
                proposal,
                method: s.method.map(L1MethodId::id),
                variant_hint: s.variant_hint,
                model_reason: s.reason.as_str().to_string(),
                relied_on: s.relied_on,
            })
        }
        Outbound::BindingSuggestion(o) => {
            let s = read(&sent).and_then(|out| validate_binding_suggestion(&out, o)).map_err(|e| refuse(reply_rejection_code(e)))?;
            let mut draft = BindingDraft::new(s.method, VariantChoice::not_stated(MethodCatalogue::entry(s.method).variant_kind));
            let mut by_role: BTreeMap<RoleName, Vec<ColumnAlias>> = BTreeMap::new();
            for role in &s.roles {
                if let Some(c) = role.column {
                    by_role.entry(role.role).or_default().push(c);
                }
            }
            for (role, cols) in &by_role {
                draft = draft.role(*role, cols);
            }
            let source = ProposalSource::AiSuggestion { task_id: HelperTask::BindingSuggestion.id().into(), request_id };
            let pid = lock(&handle).state.add_proposal(meta_claim, draft, source).map_err(|e| refuse(proposal_code(e)))?;
            Ok(L1HelperResultView::BindingSuggestion {
                proposal: L1Handle { run, id: pid.0 },
                roles: s
                    .roles
                    .iter()
                    .map(|x| L1SuggestedRoleView { role: role_name(x.role), alias: x.column.map(|c| c.name()), levels_to_select: x.levels_to_select })
                    .collect(),
                model_reason: s.reason.as_str().to_string(),
                relied_on: s.relied_on,
            })
        }
        Outbound::ResultExplanation(o) => {
            // Validate; on a rejected reply, one retry, then no explanation.
            match read(&sent).and_then(|out| validate_explanation(&out, o)) {
                Ok(text) => Ok(L1HelperResultView::Explanation { model_text: text.as_str().to_string() }),
                Err(first) => {
                    tracing::info!(run, rejection = ?first, "stats_l1: explanation rejected; one retry");
                    let prepared = lock(&handle).helper.prepare_send(id).map_err(|e| refuse(send_refusal_code(e)))?;
                    let again = transmit(&handle, ai, prepared)?;
                    let text = read(&again).and_then(|out| validate_explanation(&out, o)).map_err(|e| refuse(reply_rejection_code(e)))?;
                    Ok(L1HelperResultView::Explanation { model_text: text.as_str().to_string() })
                }
            }
        }
        Outbound::WebContext(_) => {
            let out = read(&sent).map_err(|e| refuse(reply_rejection_code(e)))?;
            let q = query_sent.unwrap_or_default();
            let ev = parse_web_context(&out, &q, RetrievedAt { unix_seconds: now_unix }, allowed_web_domains());
            Ok(L1HelperResultView::Context {
                empty_message: ev.empty_message(),
                items: ev
                    .items
                    .iter()
                    .map(|i| L1ContextItemView {
                        sentence: i.sentence.as_str().to_string(),
                        sources: i.sources.iter().map(|s| L1SourceView { url: s.url.clone(), title: s.title.clone(), domain: s.domain.clone() }).collect(),
                        retrieved_at: i.retrieved_at.unix_seconds,
                        query_sent: i.query_sent.clone(),
                    })
                    .collect(),
                uncited_sentences: ev.uncited_sentences,
                dropped_citations: ev.dropped_citations.iter().map(|d| citation_drop_word(*d)).collect(),
            })
        }
    }
}

/// Withdraw consent for a task in a run.
pub fn helper_revoke(runs: &L1Runs, run: u64, task: &str) -> Result<bool, GaplyError> {
    let task = HelperTask::from_id(task).ok_or_else(|| refuse(w("unknown_task")))?;
    let r = runs.get(run)?;
    let revoked = lock(&r).helper.revoke(task);
    Ok(revoked)
}

/// Cancel the run's helper call in flight: no retry, and its reply is
/// discarded.
pub fn helper_cancel(runs: &L1Runs, run: u64) -> Result<(), GaplyError> {
    let r = runs.get(run)?;
    let cancel = lock(&r).cancel.clone();
    cancel.store(true, Ordering::SeqCst);
    Ok(())
}

fn draft_from_edits(claim: &L1Claim, e: &BindingEdits) -> Result<BindingDraft, GaplyError> {
    let method = L1MethodId::from_id(&e.method_id).ok_or_else(|| refuse(w("unknown_method")))?;
    let kind = MethodCatalogue::entry(method).variant_kind;
    let variant = match &e.variant_ticked {
        Some(word) => VariantChoice::user_ticked(variant_from_word(method, word).ok_or_else(|| refuse(w("unknown_variant")))?),
        None => VariantChoice::from_wording(kind, &claim.sentence, None),
    };
    let mut d = BindingDraft::new(method, variant);
    for role in &e.roles {
        let name = role_from_name(&role.role).ok_or_else(|| refuse(w("unknown_role")))?;
        let cols: Vec<ColumnAlias> = role.aliases.iter().map(|a| ColumnAlias(*a)).collect();
        d = d.role(name, &cols);
    }
    for l in &e.levels {
        let name = role_from_name(&l.role).ok_or_else(|| refuse(w("unknown_role")))?;
        let levels: Vec<&str> = l.levels.iter().map(String::as_str).collect();
        d = d.levels(name, &levels);
    }
    for f in &e.filters {
        let levels: Vec<&str> = f.levels.iter().map(String::as_str).collect();
        d = d.filter(ColumnAlias(f.alias), &levels);
    }
    if e.ols_overall_f == Some(true) {
        d = d.target(OlsTarget::OverallF);
    } else if let Some(p) = e.ols_predictor {
        d = d.target(OlsTarget::Coefficient { predictor: ColumnAlias(p) });
    }
    let exact: Vec<ColumnAlias> = e.exact_whole_numbers.iter().map(|a| ColumnAlias(*a)).collect();
    Ok(d.exact_whole_numbers(&exact))
}

/// The user's binding form as a proposal: an edit of `from` (recorded as
/// Manual, edited from its source) or, with no proposal to start from (none
/// was made, or an edit to the claim dropped it), a new Manual one. Rust reads
/// the variant from the paper's words or the user's tick, as in Step 2, and
/// the view says which. Nothing is confirmed. No network.
#[tracing::instrument(skip_all)]
pub fn draft_proposal(runs: &L1Runs, run: u64, claim: usize, from: Option<L1Handle>, edits: &BindingEdits) -> Result<L1ProposalView, GaplyError> {
    let handle = runs.get(run)?;
    let mut r = lock(&handle);
    let c = r.state.claim(ClaimId(claim)).cloned().ok_or_else(|| refuse(w("unknown_claim")))?;
    let draft = draft_from_edits(&c, edits)?;
    let id = match from {
        Some(h) => {
            if h.run != run {
                return Err(refuse(w("proposal_from_another_run")));
            }
            let original = r.state.proposal(ProposalId(h.id)).ok_or_else(|| refuse(w("unknown_proposal")))?;
            if original.claim != c.id {
                return Err(refuse(w("proposal_for_another_claim")));
            }
            r.state.edit_proposal(ProposalId(h.id), draft).map_err(|e| refuse(proposal_code(e)))?
        }
        None => r.state.add_proposal(c.id, draft, ProposalSource::Manual { edited_from: None }).map_err(|e| refuse(proposal_code(e)))?,
    };
    let p = r.state.proposal(id).ok_or_else(|| refuse(w("unknown_proposal")))?;
    Ok(proposal_view(run, p, r.state.dataset()))
}

/// Confirm a proposal, with the user's edits if any. Returns the token
/// handle and the counts. No network.
#[tracing::instrument(skip_all)]
pub fn confirm_binding(runs: &L1Runs, run: u64, proposal: L1Handle, edits: Option<&BindingEdits>) -> Result<L1ConfirmView, GaplyError> {
    if proposal.run != run {
        return Err(refuse(w("proposal_from_another_run")));
    }
    let handle = runs.get(run)?;
    let mut r = lock(&handle);
    let original = r.state.proposal(ProposalId(proposal.id)).cloned().ok_or_else(|| refuse(w("unknown_proposal")))?;
    let id = match edits {
        Some(e) => {
            let claim = r.state.claim(original.claim).cloned().ok_or_else(|| refuse(w("unknown_claim")))?;
            let draft = draft_from_edits(&claim, e)?;
            r.state.edit_proposal(original.id, draft).map_err(|e| refuse(proposal_code(e)))?
        }
        None => original.id,
    };
    let (binding, token) = r.state.confirm(id).map_err(|e| refuse(confirm_code(e)))?;
    let counts = binding.counts();
    let view = L1ConfirmView {
        token: token.as_str().to_string(),
        claim_id: binding.claim().0,
        rows_read: counts.rows_read,
        rows_filtered_out: counts.rows_filtered_out,
        rows_excluded_missing: counts.rows_excluded_missing,
        n_used: counts.n_used,
        suspect_missing_codes: binding
            .suspect_codes()
            .iter()
            .map(|s| L1SuspectCodeView { alias: s.column.name(), code: s.code, cells: s.cells })
            .collect(),
        whole_numbers: whole_number_views(binding.whole_numbers()),
    };
    let entry = (binding.claim(), binding.claim_revision().clone(), token.clone());
    r.tokens.insert(token.as_str().to_string(), entry);
    tracing::info!(run, "stats_l1: binding confirmed");
    Ok(view)
}

/// Level 1 for the confirmed claims: the data file is read again. No network.
#[tracing::instrument(skip_all)]
pub fn compute(runs: &L1Runs, run: u64, handles: &[String]) -> Result<ReproductionView, GaplyError> {
    let handle = runs.get(run)?;
    let mut r = lock(&handle);
    let mut pairs = Vec::with_capacity(handles.len());
    for h in handles {
        let (_, rev, token) = r.tokens.get(h).ok_or_else(|| refuse(w("unknown_token_handle")))?;
        pairs.push((rev.clone(), token.clone()));
    }
    let outcome = run_stats_reproduction_measured(&r.state, &r.data_path, &pairs);
    let view = reproduction_view(&outcome);
    r.reproduction = Some(outcome);
    tracing::info!(run, reproduced = view.counts.reproduced, not_reproduced = view.counts.not_reproduced, "stats_l1: computed");
    Ok(view)
}

pub fn usage(runs: &L1Runs, run: u64) -> Result<L1UsageView, GaplyError> {
    let r = runs.get(run)?;
    let u = lock(&r).usage.clone();
    Ok(usage_view(&u))
}

pub fn key_status(ai: &StatsAi) -> Result<AiKeyStatusView, GaplyError> {
    let saved = ai.credentials.has_key().map_err(|e| refuse(w(e.code())))?;
    Ok(AiKeyStatusView { saved, provider: PROVIDER })
}

/// Save the key. It goes in once and never comes back: the reply is the
/// status, not the key.
pub fn key_save(ai: &StatsAi, typed: &str) -> Result<AiKeyStatusView, GaplyError> {
    ai.credentials.save_key(typed).map_err(|e| refuse(w(e.code())))?;
    tracing::info!("stats_ai: key saved");
    key_status(ai)
}

pub fn key_remove(ai: &StatsAi) -> Result<AiKeyStatusView, GaplyError> {
    ai.credentials.remove_key().map_err(|e| refuse(w(e.code())))?;
    tracing::info!("stats_ai: key removed");
    key_status(ai)
}

/// Test the key with a request that carries no user data.
pub fn key_test(ai: &StatsAi) -> AiKeyTestView {
    // With AI help off this feature makes no network request at all, the
    // key test included (evaluation fixes, part B).
    if !ai.settings().ai_help {
        tracing::info!(result = "ai_help_off", "stats_ai: key test refused");
        return AiKeyTestView { result: crate::stats_ai::KeyTest::AiHelpOff.code() };
    }
    let result = test_key(&ai.base, ai.credentials.as_ref()).code();
    tracing::info!(result = %result, "stats_ai: key tested");
    AiKeyTestView { result }
}

/// A model-supplied link, rechecked: https, no userinfo or port, and a host
/// that is an allowed domain or a subdomain of one on a dot boundary.
pub fn checked_source_link(url: &str) -> Result<String, GaplyError> {
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(refuse(w("link_malformed")));
    }
    let host = https_host(url).map_err(|d| refuse(with_code(w("link_refused"), "reason", w(citation_drop_word(d)))))?;
    if !host_is_allowed(&host, allowed_web_domains()) {
        return Err(refuse(with_code(w("link_refused"), "reason", w("domain_not_allowed"))));
    }
    Ok(url.to_string())
}

/* ------------------------------------------------------------------------ *
 *  The Tauri commands
 * ------------------------------------------------------------------------ */

type St<'a> = tauri::State<'a, crate::state::AppState>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, GaplyError> + Send + 'static) -> Result<T, GaplyError> {
    tokio::task::spawn_blocking(f).await.map_err(|_| GaplyError::Internal("stats_l1 task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_open(state: St<'_>, manuscript: String, data: String) -> Result<L1OpenView, GaplyError> {
    let (db, runs, ai) = (state.db.clone(), state.stats_l1.clone(), state.stats_ai.clone());
    blocking(move || open(&db, &runs, &ai, Path::new(&manuscript), Path::new(&data))).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_edit_claim(state: St<'_>, run: u64, claim_id: usize, claim: L1ClaimInput) -> Result<L1RevisionView, GaplyError> {
    let runs = state.stats_l1.clone();
    blocking(move || edit_claim(&runs, run, claim_id, &claim)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_add_claim(state: St<'_>, run: u64, claim: L1ClaimInput) -> Result<L1RevisionView, GaplyError> {
    let runs = state.stats_l1.clone();
    blocking(move || add_claim(&runs, run, &claim)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_proposals(state: St<'_>, run: u64) -> Result<Vec<L1ProposalView>, GaplyError> {
    let runs = state.stats_l1.clone();
    blocking(move || proposals(&runs, run)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_helper_preview(
    state: St<'_>,
    run: u64,
    input: HelperTaskInput,
    categories: Vec<String>,
) -> Result<L1PreviewView, GaplyError> {
    let (runs, ai) = (state.stats_l1.clone(), state.stats_ai.clone());
    blocking(move || helper_preview(&runs, &ai, run, &input, &categories)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_helper_send(state: St<'_>, run: u64, preview: L1Handle) -> Result<L1HelperResultView, GaplyError> {
    let (runs, ai) = (state.stats_l1.clone(), state.stats_ai.clone());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    blocking(move || helper_send(&runs, &ai, run, preview, now)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_helper_revoke(state: St<'_>, run: u64, task: String) -> Result<bool, GaplyError> {
    helper_revoke(&state.stats_l1, run, &task)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_helper_cancel(state: St<'_>, run: u64) -> Result<(), GaplyError> {
    helper_cancel(&state.stats_l1, run)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_confirm_binding(
    state: St<'_>,
    run: u64,
    proposal: L1Handle,
    edits: Option<BindingEdits>,
) -> Result<L1ConfirmView, GaplyError> {
    let runs = state.stats_l1.clone();
    blocking(move || confirm_binding(&runs, run, proposal, edits.as_ref())).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_draft_proposal(
    state: St<'_>,
    run: u64,
    claim_id: usize,
    from: Option<L1Handle>,
    edits: BindingEdits,
) -> Result<L1ProposalView, GaplyError> {
    let runs = state.stats_l1.clone();
    blocking(move || draft_proposal(&runs, run, claim_id, from, &edits)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_compute(state: St<'_>, run: u64, tokens: Vec<String>) -> Result<ReproductionView, GaplyError> {
    let runs = state.stats_l1.clone();
    blocking(move || compute(&runs, run, &tokens)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_l1_close(state: St<'_>, run: u64) -> Result<bool, GaplyError> {
    Ok(state.stats_l1.close(run))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ai_usage(state: St<'_>, run: u64) -> Result<L1UsageView, GaplyError> {
    usage(&state.stats_l1, run)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ai_setting_get(state: St<'_>) -> Result<AiSettings, GaplyError> {
    Ok(state.stats_ai.settings())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ai_setting_set(state: St<'_>, setting: AiSettings) -> Result<AiSettings, GaplyError> {
    state.stats_ai.set_settings(setting).map_err(|c| refuse(w(c)))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ai_key_status(state: St<'_>) -> Result<AiKeyStatusView, GaplyError> {
    let ai = state.stats_ai.clone();
    blocking(move || key_status(&ai)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ai_key_save(state: St<'_>, key: String) -> Result<AiKeyStatusView, GaplyError> {
    let ai = state.stats_ai.clone();
    blocking(move || key_save(&ai, &key)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ai_key_test(state: St<'_>) -> Result<AiKeyTestView, GaplyError> {
    let ai = state.stats_ai.clone();
    blocking(move || Ok(key_test(&ai))).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ai_key_remove(state: St<'_>) -> Result<AiKeyStatusView, GaplyError> {
    let ai = state.stats_ai.clone();
    blocking(move || key_remove(&ai)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_open_source_link(app: tauri::AppHandle, url: String) -> Result<(), GaplyError> {
    use tauri_plugin_opener::OpenerExt;
    let url = checked_source_link(&url)?;
    app.opener().open_url(url, None::<&str>).map_err(|_| refuse(w("link_not_opened")))
}
