//! **The Research OS's shared contracts: the typed envelope an operation, its
//! validation, its run, its result and its manifest travel in (blueprint §8,
//! §10.5).**
//!
//! Pure: no file, network, process, clock or model; `std`, `serde`,
//! `serde_json` and `sha2` only (`tests/contracts_are_pure.rs`). Domain-free:
//! nothing here names a type of the verifier or of any capability. What
//! differs per capability is a type parameter: the operation's parameters
//! `P`, its result `T`, its closed codes `C`.
//!
//! The wire, for every type here:
//! * fields are camelCase, and an unknown field is refused;
//! * enum values are snake_case words; codes are namespaced words
//!   ([`codes`]);
//! * `schemaVersion` is 1, and any other number is refused;
//! * no field can hold a path, a file name or text from data: ids are
//!   numbers or closed words, a content hash is 64 lowercase hex digits, a
//!   capability id must match a closed grammar with no digit, slash or
//!   space in it, and a diagnostic's parameters are integers or closed words.
//!
//! Left out on purpose: an `ArtifactRef` (Phase 2 stores nothing); the
//! blueprint's assumptions, variants, units, resource policy and
//! confirmation refs (Phase 4 adds what the Level 1 wrap needs); and any
//! free-form map.

pub mod codes;

use std::collections::BTreeMap;

use serde::de::{Deserializer, Error as _};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use self::codes::{closed, RunCode};

/* ------------------------------------------------------------------------ *
 *  Versions and ids
 * ------------------------------------------------------------------------ */

/// The contracts' schema version, which is 1 throughout Phase 2. Written as
/// the number 1; any other number is refused on the way in, so a payload of
/// a later schema is never read as this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SchemaVersion;

impl SchemaVersion {
    pub const NUMBER: u32 = 1;
}

impl Serialize for SchemaVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(Self::NUMBER)
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match u32::deserialize(d)? {
            Self::NUMBER => Ok(SchemaVersion),
            _ => Err(D::Error::custom("unsupported schema version")),
        }
    }
}

/// A domain workspace. Phase 3 adds Compute and Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceId {
    Statistics,
}

impl WorkspaceId {
    pub const ALL: &'static [WorkspaceId] = &[WorkspaceId::Statistics];

    /// The first word of this workspace's capability ids and codes.
    pub fn namespace(self) -> &'static str {
        match self {
            WorkspaceId::Statistics => "stats",
        }
    }
}

/// A capability's id, `stats.describe`: two to four dot-separated words of
/// lowercase ASCII letters and underscores, each 1 to 32 long, the first of
/// them a workspace's namespace. Checked when built and on the way in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn parse(id: &str) -> Option<CapabilityId> {
        let words: Vec<&str> = id.split('.').collect();
        let well_formed = (2..=4).contains(&words.len())
            && words.iter().all(|w| {
                (1..=32).contains(&w.len())
                    && w.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            })
            && WorkspaceId::ALL.iter().any(|ws| ws.namespace() == words[0]);
        well_formed.then(|| CapabilityId(id.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The workspace whose namespace the id starts with.
    pub fn workspace(&self) -> WorkspaceId {
        let first = self.0.split('.').next().unwrap_or_default();
        *WorkspaceId::ALL
            .iter()
            .find(|ws| ws.namespace() == first)
            .expect("checked by parse")
    }
}

impl Serialize for CapabilityId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = String::deserialize(d)?;
        CapabilityId::parse(&id).ok_or_else(|| D::Error::custom("not a capability id"))
    }
}

/// One declared user intent. A number, never a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationId(pub u64);

/// One execution of an operation. A number, never a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub u64);

/// SHA-256 of exact content, as 64 lowercase hex digits. A hash of the
/// user's data or results: shown on the user's own screen and written into
/// an export the user saves, never into a log or an outbound struct, so its
/// `Debug` does not print it.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn of_bytes(bytes: &[u8]) -> ContentHash {
        use sha2::{Digest, Sha256};
        ContentHash(
            Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        )
    }

    /// Exactly 64 lowercase hex digits, or nothing.
    pub fn from_hex(hex: &str) -> Option<ContentHash> {
        (hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| ContentHash(hex.to_string()))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ContentHash(..)")
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(d)?;
        ContentHash::from_hex(&hex).ok_or_else(|| D::Error::custom("not a SHA-256 hex digest"))
    }
}

/* ------------------------------------------------------------------------ *
 *  The operation
 * ------------------------------------------------------------------------ */

/// What an input is to the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRole {
    /// A rectangular table of rows and columns.
    Table,
}

/// An input, by content and shape. Never a path: the app resolves the
/// content from its own in-memory slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputRef {
    pub content_hash: ContentHash,
    pub rows: u64,
    pub columns: u64,
    pub semantic_role: SemanticRole,
}

/// A declared user intent (blueprint §8.2), typed by its parameters. Not
/// proof that it is valid: it is validated before any run is queued.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationSpec<P> {
    pub schema_version: SchemaVersion,
    pub operation_id: OperationId,
    pub workspace_id: WorkspaceId,
    pub capability_id: CapabilityId,
    pub capability_version: u32,
    pub inputs: Vec<InputRef>,
    pub parameters: P,
    pub random_seed: Option<u64>,
}

/* ------------------------------------------------------------------------ *
 *  Diagnostics and validation
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

/// The name of a diagnostic's parameter. Closed, like the codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKey {
    /// A column, by its 1-based position.
    Column,
    Count,
    Index,
    Value,
    Limit,
    Expected,
    Found,
    From,
    Event,
}

/// A diagnostic's parameter: an integer or a closed word. Never text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue<C> {
    Int(i64),
    Code(C),
    State(RunState),
    Event(RunEvent),
}

pub type Params<C> = BTreeMap<ParamKey, ParamValue<C>>;

/// One finding, as a closed code with its parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic<C> {
    pub code: C,
    pub params: Params<C>,
    pub severity: Severity,
}

impl<C> Diagnostic<C> {
    pub fn new(severity: Severity, code: C) -> Diagnostic<C> {
        Diagnostic {
            code,
            params: BTreeMap::new(),
            severity,
        }
    }
    pub fn error(code: C) -> Diagnostic<C> {
        Diagnostic::new(Severity::Error, code)
    }
    pub fn warning(code: C) -> Diagnostic<C> {
        Diagnostic::new(Severity::Warning, code)
    }
    pub fn note(code: C) -> Diagnostic<C> {
        Diagnostic::new(Severity::Note, code)
    }
    pub fn with(mut self, key: ParamKey, value: ParamValue<C>) -> Diagnostic<C> {
        self.params.insert(key, value);
        self
    }
    pub fn int(self, key: ParamKey, value: i64) -> Diagnostic<C> {
        self.with(key, ParamValue::Int(value))
    }
}

/// The outcome of validating a spec (blueprint §8.4). `NeedsUserInput` is a
/// question for the user, not a failed scientific result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Valid,
    ValidWithWarnings,
    NeedsUserInput,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceEstimate {
    pub rows: u64,
    pub columns: u64,
    pub cells: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationReport<C> {
    pub schema_version: SchemaVersion,
    pub status: ValidationStatus,
    pub errors: Vec<Diagnostic<C>>,
    pub warnings: Vec<Diagnostic<C>>,
    /// What the user must answer before the spec can run, as closed codes.
    pub missing_information: Vec<Diagnostic<C>>,
    pub estimated_resources: ResourceEstimate,
}

impl<C> ValidationReport<C> {
    /// The status follows from what was found, most serious first:
    /// anything unsupported, then any error, then any open question, then
    /// any warning. The unsupported findings are reported among the errors.
    pub fn new(
        unsupported: Vec<Diagnostic<C>>,
        errors: Vec<Diagnostic<C>>,
        missing_information: Vec<Diagnostic<C>>,
        warnings: Vec<Diagnostic<C>>,
        estimated_resources: ResourceEstimate,
    ) -> ValidationReport<C> {
        let status = if !unsupported.is_empty() {
            ValidationStatus::Unsupported
        } else if !errors.is_empty() {
            ValidationStatus::Invalid
        } else if !missing_information.is_empty() {
            ValidationStatus::NeedsUserInput
        } else if !warnings.is_empty() {
            ValidationStatus::ValidWithWarnings
        } else {
            ValidationStatus::Valid
        };
        let mut all_errors = unsupported;
        all_errors.extend(errors);
        ValidationReport {
            schema_version: SchemaVersion,
            status,
            errors: all_errors,
            warnings,
            missing_information,
            estimated_resources,
        }
    }

    /// Valid, with or without warnings.
    pub fn may_run(&self) -> bool {
        matches!(
            self.status,
            ValidationStatus::Valid | ValidationStatus::ValidWithWarnings
        )
    }
}

/* ------------------------------------------------------------------------ *
 *  The run
 * ------------------------------------------------------------------------ */

closed! {
    /// A run's state: every state of blueprint §8.5. `AwaitingConfirmation`
    /// and `Invalidated` have no producer until Phase 4 wraps Level 1; they
    /// are here so the contract does not change then.
    pub enum RunState in "" {
        Draft => "draft",
        Validating => "validating",
        NeedsUserInput => "needs_user_input",
        AwaitingConfirmation => "awaiting_confirmation",
        Refused => "refused",
        Validated => "validated",
        Invalidated => "invalidated",
        Queued => "queued",
        Running => "running",
        Succeeded => "succeeded",
        Failed => "failed",
        Cancelled => "cancelled",
        TimedOut => "timed_out",
        ResourceLimit => "resource_limit",
        NumericalFailure => "numerical_failure",
    }
}

closed! {
    /// What happened to a run. Each is an edge in [`transition`]'s table
    /// from one or more states.
    pub enum RunEvent in "" {
        BeginValidation => "begin_validation",
        RequestInput => "request_input",
        Refuse => "refuse",
        PassValidation => "pass_validation",
        RequireConfirmation => "require_confirmation",
        Enqueue => "enqueue",
        Confirm => "confirm",
        Invalidate => "invalidate",
        Cancel => "cancel",
        Start => "start",
        Succeed => "succeed",
        Fail => "fail",
        TimeOut => "time_out",
        ExceedResources => "exceed_resources",
        FailNumerically => "fail_numerically",
    }
}

impl RunState {
    /// No edge leaves it. `NeedsUserInput` is one: the user's answer
    /// changes the spec, and a changed spec is a new run (blueprint §8.5
    /// draws no edge out of it).
    pub fn is_terminal(self) -> bool {
        use RunState as S;
        match self {
            S::NeedsUserInput
            | S::Refused
            | S::Invalidated
            | S::Succeeded
            | S::Failed
            | S::Cancelled
            | S::TimedOut
            | S::ResourceLimit
            | S::NumericalFailure => true,
            S::Draft
            | S::Validating
            | S::AwaitingConfirmation
            | S::Validated
            | S::Queued
            | S::Running => false,
        }
    }
}

/// The one way a run changes state: the edges of blueprint §8.5, and no
/// others. A terminal state refuses every event with `already_terminal`;
/// any other missing edge is `illegal_transition`. Both name the state and
/// the event.
pub fn transition(from: RunState, event: RunEvent) -> Result<RunState, Diagnostic<RunCode>> {
    use RunEvent as E;
    use RunState as S;
    let to = match (from, event) {
        (S::Draft, E::BeginValidation) => S::Validating,
        (S::Validating, E::RequestInput) => S::NeedsUserInput,
        (S::Validating, E::Refuse) => S::Refused,
        (S::Validating, E::PassValidation) => S::Validated,
        (S::Validated, E::RequireConfirmation) => S::AwaitingConfirmation,
        (S::Validated, E::Enqueue) => S::Queued,
        (S::AwaitingConfirmation, E::Confirm) => S::Queued,
        (S::AwaitingConfirmation, E::Cancel) => S::Cancelled,
        (S::AwaitingConfirmation, E::Invalidate) => S::Invalidated,
        (S::Queued, E::Start) => S::Running,
        (S::Queued, E::Cancel) => S::Cancelled,
        (S::Queued, E::Refuse) => S::Refused,
        (S::Running, E::Succeed) => S::Succeeded,
        (S::Running, E::Fail) => S::Failed,
        (S::Running, E::Cancel) => S::Cancelled,
        (S::Running, E::TimeOut) => S::TimedOut,
        (S::Running, E::ExceedResources) => S::ResourceLimit,
        (S::Running, E::FailNumerically) => S::NumericalFailure,
        _ => {
            let code = if from.is_terminal() {
                RunCode::AlreadyTerminal
            } else {
                RunCode::IllegalTransition
            };
            return Err(Diagnostic::error(code)
                .with(ParamKey::From, ParamValue::State(from))
                .with(ParamKey::Event, ParamValue::Event(event)));
        }
    };
    Ok(to)
}

/// One run, as the run manager keeps it in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRecord {
    pub schema_version: SchemaVersion,
    pub run_id: RunId,
    pub operation_id: OperationId,
    pub state: RunState,
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
}

/* ------------------------------------------------------------------------ *
 *  The result and its manifest
 * ------------------------------------------------------------------------ */

/// What a run produced (blueprint §8.6): the capability's own typed result
/// `T` inside a common envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultBundle<T, C> {
    pub schema_version: SchemaVersion,
    pub run_id: RunId,
    pub status: RunState,
    pub primary_outputs: T,
    pub diagnostics: Vec<Diagnostic<C>>,
    pub limitations: Vec<Diagnostic<C>>,
    pub manifest: RunManifest<C>,
}

/// Cells or rows a run left out, by reason, with a count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Exclusion<C> {
    pub code: C,
    pub count: u64,
    pub params: Params<C>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    Macos,
    Windows,
    Linux,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    Aarch64,
    X86_64,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// The platform this build targets, fixed when it was compiled.
    pub fn current() -> Platform {
        let os = if cfg!(target_os = "macos") {
            Os::Macos
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            Os::Other
        };
        let arch = if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else if cfg!(target_arch = "x86_64") {
            Arch::X86_64
        } else {
            Arch::Other
        };
        Platform { os, arch }
    }
}

/// How far the manifest promises the result repeats (blueprint §10.5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Reproducibility {
    /// The same inputs, engine version and platform give the same bytes.
    Bitwise,
    /// Repeats within this relative tolerance.
    WithinTolerance {
        relative: f64,
    },
    NotReproducible,
}

/// The record of one completed run (blueprint §10.5). It holds content
/// hashes of the user's data and results, which the user's own screen shows
/// and an export the user saves contains; it never goes into a log or an
/// outbound struct (decision 12). No path, no file name, no header, no
/// label and no cell: hashes, versions and counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunManifest<C> {
    pub schema_version: SchemaVersion,
    pub capability_id: CapabilityId,
    pub capability_version: u32,
    pub engine_version: u32,
    /// Each input's content hash, in the spec's order.
    pub inputs: Vec<ContentHash>,
    pub parameters_hash: ContentHash,
    pub seed: Option<u64>,
    pub rows_read: u64,
    pub rows_used: u64,
    pub exclusions: Vec<Exclusion<C>>,
    pub platform: Platform,
    pub elapsed_ms: u64,
    pub output_hash: ContentHash,
    pub reproducibility: Reproducibility,
}
