//! **The Research OS's project model: ids, hashes, artifact references, runs
//! as stored, the provenance graph and the closed refusal codes (Phase 5B-1,
//! decisions 99 to 112; ADR-009, ADR-004 §004b).**
//!
//! Pure, beside `contracts`: no file, network, process, clock, database or
//! log; `std`, `serde` (through [`closed!`]) and `crate::contracts` only
//! (`tests/contracts_are_pure.rs`). The one module that does I/O with these
//! types is the app crate's `project_store`, the only Research OS module
//! allowed to write (`src-tauri/tests/stats_desc_writes_nothing.rs`).
//!
//! Named `research_project`, not `project`, because the dormant
//! `crate::projects` (the old `projects` table and its three seam commands)
//! stays as it is until 5D (decision 110) and this module never uses it.
//!
//! What a stored row may hold, and what it may not: ids, hashes, a
//! capability id and version, a state, closed codes, sizes and times. The
//! one user-typed value is the [`ProjectTitle`], which lives in its own
//! field of `project.db`. Anything that names a column, a level or a value
//! lives only inside an artifact, referred to by its [`ArtifactHash`]. A
//! path or a file name is never a field of anything here (decision 101).
//!
//! An id and a hash can only be made by their parsers, which refuse anything
//! else with a closed code, so a path joined from either cannot leave its
//! folder: 32 or 64 lowercase hex characters hold no `/`, `\`, `.` or drive
//! letter.

mod graph;
mod render;

pub use self::graph::{ProvEdge, ProvGraph, ProvNode};
pub use self::render::{
    escape_markdown, iso_utc, render_prov_json, render_report_markdown, ColumnRole, ExportCode,
    ExportCount, ExportManifest, ExportParameter, ExportRun, ProjectExportInput, TestVariant,
    PROV_FILE_NAME, PROV_NAMESPACE, REPORT_FILE_NAME, REPORT_PREAMBLE,
};

use crate::contracts::codes::{closed, Closed};
use crate::contracts::{CapabilityId, ContentHash, RunState, WorkspaceId};

/// `project.db`'s schema version, kept in `PRAGMA user_version`.
pub const SCHEMA_VERSION: u32 = 1;

/// The most one project may hold on disk: every file in its folder
/// (decision 107).
pub const PROJECT_CAP_BYTES: u64 = 100 * 1024 * 1024;

/// The most all projects together may hold, tombstones included
/// (decision 107).
pub const ALL_PROJECTS_CAP_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// What a Save adds to its own count for the database's growth, since the
/// rows and the write-ahead log grow by an amount the store cannot know
/// before it writes.
pub const DB_GROWTH_ALLOWANCE_BYTES: u64 = 1024 * 1024;

/// The longest title, in characters (decision 121).
pub const TITLE_MAX_CHARS: usize = 100;

/// How many hex characters of a hash a log line may carry.
pub const HASH_PREFIX_LEN: usize = 8;

closed! {
    /// Why the project store refused, or how it marked a row. Every refusal
    /// of the store and of this module is one of these; none carries text.
    pub enum ProjectCode in "project." {
        /// A project id that is not 32 lowercase hex characters.
        BadProjectId => "bad_project_id",
        /// A hash that is not 64 lowercase hex characters.
        BadHash => "bad_hash",
        /// A title that is empty, too long or holds a control character.
        BadTitle => "bad_title",
        /// An app version that is not 1 to 32 of `[0-9A-Za-z.+-]`.
        BadAgentVersion => "bad_agent_version",
        /// A format the artifact's kind does not take.
        BadFormat => "bad_format",
        /// The projects folder could not be resolved or is not absolute.
        RootUnavailable => "root_unavailable",
        /// No project folder has that id.
        NotFound => "not_found",
        /// A project folder, its database or its artifacts folder is a
        /// symlink; the store follows none.
        SymlinkRefused => "symlink_refused",
        /// The project folder has no `project.db`.
        DbMissing => "db_missing",
        /// `project.db` is not a database this build can read.
        DbUnreadable => "db_unreadable",
        /// `project.db` was written by a newer build.
        DbNewerSchema => "db_newer_schema",
        /// Another connection holds the project's write lock.
        Busy => "busy",
        /// The project is open in this process; close it before deleting.
        StillOpen => "still_open",
        /// Only a finished run is saved in Phase 5B.
        RunNotFinished => "run_not_finished",
        /// An edge names an artifact that is neither in the Save nor in the
        /// project.
        UnknownArtifact => "unknown_artifact",
        /// The bytes do not hash to the hash given, or did not read back as
        /// written.
        HashMismatch => "hash_mismatch",
        /// Damage: the row's file is gone.
        ArtifactMissing => "artifact_missing",
        /// Damage: the row's file is not a regular file.
        ArtifactNotRegular => "artifact_not_regular",
        /// Damage: the row's file has another size than the row's.
        ArtifactWrongSize => "artifact_wrong_size",
        /// Damage: the row's file does not hash to its name.
        ArtifactWrongHash => "artifact_wrong_hash",
        /// The edge would close a cycle in the provenance graph.
        ProvCycle => "prov_cycle",
        /// A derivation names a run that did not use its source or did not
        /// generate its artifact.
        ProvUnlinkedDerivation => "prov_unlinked_derivation",
        /// The Save would pass the one-project cap.
        ProjectCap => "project_cap",
        /// The Save or the create would pass the all-projects cap.
        AllProjectsCap => "all_projects_cap",
        /// The disk is full, or SQLite could not get a page.
        DiskFull => "disk_full",
        /// A file operation failed for another reason.
        IoFailed => "io_failed",
        /// A database operation failed for another reason.
        DbFailed => "db_failed",
        /// A run's end code: it was left unfinished, and recovery marked it
        /// failed.
        RunInterrupted => "run_interrupted",
        // Phase 5B-2: the commands' refusals (decisions 120 to 124).
        /// No project is open, or the id given is not the open project's.
        NotOpen => "not_open",
        /// No run of the registry has that id, and none ever had.
        RunUnknown => "run_unknown",
        /// The run is no longer held: evicted, or its workspace was left.
        RunEvicted => "run_evicted",
        /// The run was cancelled; a cancelled run is never saved.
        RunCancelled => "run_cancelled",
        /// The run ended in a state a project does not keep: refused,
        /// waiting on the user, timed out or over a budget.
        RunNotSaveable => "run_not_saveable",
        /// The run failed without a closed code to keep.
        RunFailedWithoutCode => "run_failed_without_code",
        /// This project already holds that run.
        RunAlreadySaved => "run_already_saved",
        /// The data file the run read is no longer held: its dataset was
        /// closed or evicted, so its bytes cannot be shown to be the same.
        DataUnavailable => "data_unavailable",
        /// The data file held does not hash to what the run read.
        DataMismatch => "data_mismatch",
        /// A delete came with no confirmation.
        ConfirmationMissing => "confirmation_missing",
        /// The confirmation is not one this session issued.
        ConfirmationUnknown => "confirmation_unknown",
        /// The confirmation was used already.
        ConfirmationUsed => "confirmation_used",
        /// A newer confirmation replaced it.
        ConfirmationReplaced => "confirmation_replaced",
        /// The confirmation is more than five minutes old.
        ConfirmationExpired => "confirmation_expired",
        /// The confirmation was issued for another project.
        ConfirmationOtherProject => "confirmation_other_project",
        /// The open project holds no run with that id.
        StoredRunUnknown => "stored_run_unknown",
        /// A run's result is larger than a read returns.
        ResultTooLarge => "result_too_large",
    }
}

/// A refusal: a closed code, and for a cap the bytes it would have needed and
/// the limit. Nothing else, so an error cannot carry a path or a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectRefusal {
    pub code: ProjectCode,
    pub needed: Option<u64>,
    pub limit: Option<u64>,
}

impl ProjectRefusal {
    pub fn over(code: ProjectCode, needed: u64, limit: u64) -> ProjectRefusal {
        ProjectRefusal {
            code,
            needed: Some(needed),
            limit: Some(limit),
        }
    }
}

impl From<ProjectCode> for ProjectRefusal {
    fn from(code: ProjectCode) -> ProjectRefusal {
        ProjectRefusal {
            code,
            needed: None,
            limit: None,
        }
    }
}

impl std::fmt::Display for ProjectRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code.wire())?;
        if let (Some(n), Some(l)) = (self.needed, self.limit) {
            write!(f, " (needed {n} bytes, limit {l})")?;
        }
        Ok(())
    }
}

/// Lowercase hex of `bytes`; what the store hands to the parsers below.
pub fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A project's id: 32 lowercase hex characters, from a random source. Also
/// its folder's name. Made only by [`ProjectId::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectId(String);

impl ProjectId {
    pub const LEN: usize = 32;

    pub fn parse(raw: &str) -> Result<ProjectId, ProjectCode> {
        if is_lower_hex(raw, Self::LEN) {
            Ok(ProjectId(raw.to_string()))
        } else {
            Err(ProjectCode::BadProjectId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An artifact's SHA-256, 64 lowercase hex characters; also its file's
/// name. Made only by [`ArtifactHash::parse`], whose rule is
/// [`ContentHash::from_hex`]'s. Its `Debug` prints the prefix only.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArtifactHash(ContentHash);

impl ArtifactHash {
    pub fn parse(raw: &str) -> Result<ArtifactHash, ProjectCode> {
        ContentHash::from_hex(raw)
            .map(ArtifactHash)
            .ok_or(ProjectCode::BadHash)
    }

    pub fn as_hex(&self) -> &str {
        self.0.as_hex()
    }

    /// The first [`HASH_PREFIX_LEN`] characters: the only form a log takes.
    pub fn prefix(&self) -> &str {
        &self.0.as_hex()[..HASH_PREFIX_LEN]
    }

    pub fn content_hash(&self) -> &ContentHash {
        &self.0
    }
}

impl std::fmt::Debug for ArtifactHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArtifactHash({}..)", self.prefix())
    }
}

/// The project's name, typed by the user: the one user text a row holds.
/// Its `Debug` prints its length only.
#[derive(Clone, PartialEq, Eq)]
pub struct ProjectTitle(String);

/// A character a reader cannot see that changes how text around it reads:
/// Unicode's format characters (category Cf: the bidirectional overrides and
/// isolates, zero-width spaces and joiners, the soft hyphen, the byte-order
/// mark, the tag characters), the line and paragraph separators, the
/// combining grapheme joiner and the Hangul fillers. Listed by code point,
/// from the Unicode 15.1 character database, read 19 Oct 2026; `std` has no
/// category lookup and no crate is added for one.
pub fn is_invisible_format(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x034F
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x115F..=0x1160
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0x3164
            | 0xFEFF
            | 0xFFA0
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

impl ProjectTitle {
    /// 1 to [`TITLE_MAX_CHARS`] characters, with no whitespace at either
    /// end, no control character and no invisible formatting character
    /// ([`is_invisible_format`]) (decision 121).
    pub fn parse(raw: &str) -> Result<ProjectTitle, ProjectCode> {
        let n = raw.chars().count();
        if n == 0
            || n > TITLE_MAX_CHARS
            || raw.trim() != raw
            || raw
                .chars()
                .any(|c| c.is_control() || is_invisible_format(c))
        {
            return Err(ProjectCode::BadTitle);
        }
        Ok(ProjectTitle(raw.to_string()))
    }

    /// The name as the user typed it: trimmed, then [`ProjectTitle::parse`]'s
    /// rule (decision 121).
    pub fn from_typed(raw: &str) -> Result<ProjectTitle, ProjectCode> {
        ProjectTitle::parse(raw.trim())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ProjectTitle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProjectTitle({} chars)", self.0.chars().count())
    }
}

/// The app as the provenance agent: its version, never a person's name or
/// account.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentVersion(String);

impl AgentVersion {
    pub fn parse(raw: &str) -> Result<AgentVersion, ProjectCode> {
        let ok = (1..=32).contains(&raw.len())
            && raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'));
        if ok {
            Ok(AgentVersion(raw.to_string()))
        } else {
            Err(ProjectCode::BadAgentVersion)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

closed! {
    /// What an artifact is.
    pub enum ArtifactKind in "" {
        /// The user's data file, as the ingest read it (decision 101).
        DataFile => "data_file",
        /// The run's `OperationSpec`, as canonical JSON.
        OperationSpec => "operation_spec",
        /// The run's `ResultBundle`, its manifest inside, as canonical JSON.
        ResultBundle => "result_bundle",
        /// The run's `RunManifest` on its own, as canonical JSON (Phase 5B-2).
        RunManifest => "run_manifest",
        /// A failed run's failure record: its closed code and parameters, as
        /// JSON (Phase 5B-2, decision 123).
        RunFailure => "run_failure",
    }
}

closed! {
    /// An artifact's format, a closed code (decision 101: never a file name).
    /// The data formats are the ones Level 1's ingest reads.
    pub enum DataFormat in "" {
        Csv => "csv",
        Tsv => "tsv",
        Xlsx => "xlsx",
        Xlsm => "xlsm",
        Xlsb => "xlsb",
        Xls => "xls",
        Ods => "ods",
        Sav => "sav",
        Mat => "mat",
        Json => "json",
    }
}

impl ArtifactKind {
    /// A data file is one of the ingest's formats; a spec or a bundle is
    /// JSON.
    pub fn admits(self, format: DataFormat) -> bool {
        match self {
            ArtifactKind::DataFile => format != DataFormat::Json,
            ArtifactKind::OperationSpec
            | ArtifactKind::ResultBundle
            | ArtifactKind::RunManifest
            | ArtifactKind::RunFailure => format == DataFormat::Json,
        }
    }
}

closed! {
    /// What an artifact was to the run that used it.
    pub enum UsedRole in "" {
        Data => "data",
        Spec => "spec",
    }
}

/// An artifact, as its row describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    pub hash: ArtifactHash,
    pub bytes: u64,
    pub kind: ArtifactKind,
    pub format: DataFormat,
    /// The schema version of the artifact's own format (1 for a spec or a
    /// bundle of the contracts' schema version 1).
    pub schema_version: u32,
    /// `None`, or one of the four damage codes.
    pub damage: Option<ProjectCode>,
}

/// A closed code's wire word with its type left behind, so codes of several
/// sets can sit in one list (the exports' input). Made only from a member of
/// a [`Closed`] set, so its text is always one of that set's `'static` words,
/// never user text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClosedWord(&'static str);

impl ClosedWord {
    pub fn of<C: Closed>(code: C) -> ClosedWord {
        ClosedWord(code.wire())
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// A run's id inside one project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StoredRunId(pub u64);

/// A run, as its row holds it: ids, the capability, a state, a closed code
/// and times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRun {
    pub run_id: StoredRunId,
    pub capability_id: CapabilityId,
    pub capability_version: u32,
    pub state: RunState,
    pub end_code: Option<ProjectCode>,
    pub agent: AgentVersion,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub saved_at: i64,
}

/// The project's own row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: ProjectId,
    pub title: ProjectTitle,
    pub workspace_home: WorkspaceId,
    pub created_at: i64,
    pub app_version: AgentVersion,
}

/// The word a workspace is stored as.
pub fn workspace_word(ws: WorkspaceId) -> &'static str {
    match ws {
        WorkspaceId::Statistics => "statistics",
        WorkspaceId::Compute => "compute",
        WorkspaceId::Design => "design",
    }
}

/// The workspace a stored word names, if any.
pub fn workspace_from_word(word: &str) -> Option<WorkspaceId> {
    WorkspaceId::ALL
        .iter()
        .copied()
        .find(|w| workspace_word(*w) == word)
}

/// Recovery's rule for a run row (ADR-009 §6): a run that never reached a
/// terminal state is `Failed`, interrupted. Never `Succeeded`: a run reads
/// as succeeded only if that state was committed.
pub fn recovered_state(state: RunState) -> Option<(RunState, ProjectCode)> {
    if state.is_terminal() {
        None
    } else {
        Some((RunState::Failed, ProjectCode::RunInterrupted))
    }
}

/// What the store found on disk for an artifact row, read from metadata
/// without following a symlink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Missing,
    NotRegular,
    Regular { len: u64 },
}

/// Recovery's damage rule for a row of `bytes` bytes: `None` when the file
/// is there at that size. The hash is checked whenever the file is read.
pub fn damage_of(bytes: u64, file: FileState) -> Option<ProjectCode> {
    match file {
        FileState::Missing => Some(ProjectCode::ArtifactMissing),
        FileState::NotRegular => Some(ProjectCode::ArtifactNotRegular),
        FileState::Regular { len } if len != bytes => Some(ProjectCode::ArtifactWrongSize),
        FileState::Regular { .. } => None,
    }
}

/// The store's limits, a parameter so tests use small ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub project: u64,
    pub all_projects: u64,
    pub db_allowance: u64,
}

impl Caps {
    /// Decision 107's limits.
    pub const REAL: Caps = Caps {
        project: PROJECT_CAP_BYTES,
        all_projects: ALL_PROJECTS_CAP_BYTES,
        db_allowance: DB_GROWTH_ALLOWANCE_BYTES,
    };

    /// Whether a write of `incoming` new bytes fits: the project's size on
    /// disk plus `incoming` plus the database allowance must not pass the
    /// project cap, nor the all-projects size plus the same the all-projects
    /// cap. Exactly at a cap fits; one byte over does not.
    pub fn admit(
        &self,
        project_now: u64,
        all_now: u64,
        incoming: u64,
    ) -> Result<(), ProjectRefusal> {
        let add = incoming.saturating_add(self.db_allowance);
        let project = project_now.saturating_add(add);
        if project > self.project {
            return Err(ProjectRefusal::over(
                ProjectCode::ProjectCap,
                project,
                self.project,
            ));
        }
        let all = all_now.saturating_add(add);
        if all > self.all_projects {
            return Err(ProjectRefusal::over(
                ProjectCode::AllProjectsCap,
                all,
                self.all_projects,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_are_the_pinned_list() {
        let words: Vec<&str> = ProjectCode::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(
            words,
            [
                "project.bad_project_id",
                "project.bad_hash",
                "project.bad_title",
                "project.bad_agent_version",
                "project.bad_format",
                "project.root_unavailable",
                "project.not_found",
                "project.symlink_refused",
                "project.db_missing",
                "project.db_unreadable",
                "project.db_newer_schema",
                "project.busy",
                "project.still_open",
                "project.run_not_finished",
                "project.unknown_artifact",
                "project.hash_mismatch",
                "project.artifact_missing",
                "project.artifact_not_regular",
                "project.artifact_wrong_size",
                "project.artifact_wrong_hash",
                "project.prov_cycle",
                "project.prov_unlinked_derivation",
                "project.project_cap",
                "project.all_projects_cap",
                "project.disk_full",
                "project.io_failed",
                "project.db_failed",
                "project.run_interrupted",
                "project.not_open",
                "project.run_unknown",
                "project.run_evicted",
                "project.run_cancelled",
                "project.run_not_saveable",
                "project.run_failed_without_code",
                "project.run_already_saved",
                "project.data_unavailable",
                "project.data_mismatch",
                "project.confirmation_missing",
                "project.confirmation_unknown",
                "project.confirmation_used",
                "project.confirmation_replaced",
                "project.confirmation_expired",
                "project.confirmation_other_project",
                "project.stored_run_unknown",
                "project.result_too_large",
            ]
        );
        let kinds: Vec<&str> = ArtifactKind::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(
            kinds,
            [
                "data_file",
                "operation_spec",
                "result_bundle",
                "run_manifest",
                "run_failure"
            ]
        );
        let formats: Vec<&str> = DataFormat::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(
            formats,
            ["csv", "tsv", "xlsx", "xlsm", "xlsb", "xls", "ods", "sav", "mat", "json"]
        );
        let roles: Vec<&str> = UsedRole::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(roles, ["data", "spec"]);
    }

    #[test]
    fn the_caps_are_decision_107s() {
        assert_eq!(PROJECT_CAP_BYTES, 104_857_600);
        assert_eq!(ALL_PROJECTS_CAP_BYTES, 10_737_418_240);
        assert_eq!(DB_GROWTH_ALLOWANCE_BYTES, 1_048_576);
        assert_eq!(
            Caps::REAL,
            Caps {
                project: 104_857_600,
                all_projects: 10_737_418_240,
                db_allowance: 1_048_576
            }
        );
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn a_cap_admits_exactly_its_limit_and_refuses_one_byte_more() {
        let caps = Caps {
            project: 1_000,
            all_projects: 5_000,
            db_allowance: 100,
        };
        assert_eq!(caps.admit(400, 400, 500), Ok(()));
        assert_eq!(
            caps.admit(400, 400, 501),
            Err(ProjectRefusal::over(ProjectCode::ProjectCap, 1_001, 1_000))
        );
        assert_eq!(caps.admit(0, 4_400, 500), Ok(()));
        assert_eq!(
            caps.admit(0, 4_401, 500),
            Err(ProjectRefusal::over(
                ProjectCode::AllProjectsCap,
                5_001,
                5_000
            ))
        );
        assert!(
            caps.admit(u64::MAX, 0, u64::MAX).is_err(),
            "saturates, never wraps"
        );
    }

    #[test]
    fn ids_and_hashes_come_only_from_their_parsers() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            ProjectId::parse(id).map(|p| p.as_str().to_string()),
            Ok(id.to_string())
        );
        let hash = "a".repeat(64);
        assert_eq!(
            ArtifactHash::parse(&hash).map(|h| h.prefix().to_string()),
            Ok("aaaaaaaa".into())
        );
        for bad in [
            "",
            "../",
            "../0123456789abcdef0123456789abcd",
            "/etc/passwd",
            "C:\\Windows",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "0123456789abcdef0123456789abcde/",
            "0123456789abcdef 123456789abcdef",
        ] {
            assert_eq!(
                ProjectId::parse(bad),
                Err(ProjectCode::BadProjectId),
                "{bad:?}"
            );
            assert_eq!(
                ArtifactHash::parse(bad),
                Err(ProjectCode::BadHash),
                "{bad:?}"
            );
        }
        assert_eq!(
            ArtifactHash::parse(&"A".repeat(64)),
            Err(ProjectCode::BadHash)
        );
        assert_eq!(
            ArtifactHash::parse(&"a".repeat(63)),
            Err(ProjectCode::BadHash)
        );
        assert_eq!(
            ArtifactHash::parse(&format!("../{}", "a".repeat(61))),
            Err(ProjectCode::BadHash)
        );
        assert_eq!(hex_lower(&[0x00, 0xab, 0xff]), "00abff");
        assert_eq!(
            ProjectId::parse(&hex_lower(&[7u8; 16])).map(|p| p.as_str().len()),
            Ok(32)
        );
    }

    #[test]
    fn debug_never_prints_a_title_or_a_whole_hash() {
        let t = ProjectTitle::parse("TITLE_SENTINEL").expect("title");
        assert_eq!(format!("{t:?}"), "ProjectTitle(14 chars)");
        let h = ArtifactHash::parse(&"b".repeat(64)).expect("hash");
        assert_eq!(format!("{h:?}"), "ArtifactHash(bbbbbbbb..)");
        assert_eq!(
            ProjectRefusal::over(ProjectCode::ProjectCap, 9, 8).to_string(),
            "project.project_cap (needed 9 bytes, limit 8)"
        );
    }

    #[test]
    fn titles_and_agent_versions_are_checked() {
        assert!(ProjectTitle::parse("Thesis chapter 3").is_ok());
        assert!(ProjectTitle::parse(&"é".repeat(TITLE_MAX_CHARS)).is_ok());
        for bad in ["", "   ", "a\nb", "a\u{0}b"] {
            assert_eq!(
                ProjectTitle::parse(bad),
                Err(ProjectCode::BadTitle),
                "{bad:?}"
            );
        }
        assert_eq!(
            ProjectTitle::parse(&"x".repeat(TITLE_MAX_CHARS + 1)),
            Err(ProjectCode::BadTitle)
        );
        assert!(AgentVersion::parse("0.1.0").is_ok());
        assert!(AgentVersion::parse("1.2.3-beta+4").is_ok());
        for bad in ["", "1 0", "a/b", &"9".repeat(33)] {
            assert_eq!(
                AgentVersion::parse(bad),
                Err(ProjectCode::BadAgentVersion),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_typed_name_is_trimmed_and_invisible_formatting_is_refused() {
        assert_eq!(
            ProjectTitle::from_typed("  Thesis  ").map(|t| t.as_str().to_string()),
            Ok("Thesis".into())
        );
        assert_eq!(
            ProjectTitle::parse(" Thesis"),
            Err(ProjectCode::BadTitle),
            "a stored name has no space at either end"
        );
        assert!(ProjectTitle::from_typed(&"é".repeat(100)).is_ok());
        assert_eq!(
            ProjectTitle::from_typed(&"é".repeat(101)),
            Err(ProjectCode::BadTitle),
            "counted in characters, not bytes"
        );
        for bad in [
            "",
            " \t ",
            "a\u{7}b",
            "evil\u{202E}gpj.exe",
            "a\u{2066}b",
            "zero\u{200B}width",
            "soft\u{00AD}hyphen",
            "bom\u{FEFF}",
            "tag\u{E0041}",
            "line\u{2028}sep",
        ] {
            assert_eq!(
                ProjectTitle::from_typed(bad),
                Err(ProjectCode::BadTitle),
                "{bad:?}"
            );
        }
        // Visible text in any script is a name.
        for good in [
            "Kapitel 3: Ergebnisse",
            "第三章",
            "Résumé — v2",
            "a + b = c",
        ] {
            assert!(ProjectTitle::from_typed(good).is_ok(), "{good:?}");
        }
    }

    #[test]
    fn a_closed_word_is_a_closed_sets_word() {
        assert_eq!(
            ClosedWord::of(ProjectCode::NotOpen).as_str(),
            "project.not_open"
        );
        assert_eq!(ClosedWord::of(RunState::Failed).as_str(), "failed");
    }

    #[test]
    fn an_unfinished_run_recovers_failed_and_a_finished_one_is_left() {
        for s in RunState::ALL.iter().copied() {
            match recovered_state(s) {
                Some((to, code)) => {
                    assert!(!s.is_terminal(), "{s:?}");
                    assert_eq!(to, RunState::Failed);
                    assert_eq!(code, ProjectCode::RunInterrupted);
                }
                None => assert!(s.is_terminal(), "{s:?}"),
            }
        }
        assert_eq!(
            recovered_state(RunState::Running),
            Some((RunState::Failed, ProjectCode::RunInterrupted))
        );
        assert_eq!(
            recovered_state(RunState::Queued),
            Some((RunState::Failed, ProjectCode::RunInterrupted))
        );
    }

    #[test]
    fn damage_is_missing_not_regular_or_wrong_size() {
        assert_eq!(
            damage_of(5, FileState::Missing),
            Some(ProjectCode::ArtifactMissing)
        );
        assert_eq!(
            damage_of(5, FileState::NotRegular),
            Some(ProjectCode::ArtifactNotRegular)
        );
        assert_eq!(
            damage_of(5, FileState::Regular { len: 4 }),
            Some(ProjectCode::ArtifactWrongSize)
        );
        assert_eq!(damage_of(5, FileState::Regular { len: 5 }), None);
    }

    #[test]
    fn kinds_take_their_formats_and_workspaces_round_trip() {
        assert!(ArtifactKind::DataFile.admits(DataFormat::Csv));
        assert!(!ArtifactKind::DataFile.admits(DataFormat::Json));
        assert!(ArtifactKind::ResultBundle.admits(DataFormat::Json));
        assert!(!ArtifactKind::OperationSpec.admits(DataFormat::Sav));
        for w in WorkspaceId::ALL.iter().copied() {
            assert_eq!(workspace_from_word(workspace_word(w)), Some(w));
        }
        assert_eq!(workspace_from_word("stats"), None);
    }
}
