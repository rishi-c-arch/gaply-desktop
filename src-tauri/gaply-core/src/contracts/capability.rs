//! **Capabilities (Research OS, Phase 4A): what one executable feature of a
//! workspace is, and whether this build may run it (blueprint §7.2, §7.3,
//! §11.4, §17).**
//!
//! Pure and domain-free, like the rest of `contracts`
//! (`tests/contracts_are_pure.rs`): nothing here names a capability module.
//! What differs per workspace is the wording-code type `W`, a closed set that
//! the screen's wording table turns into words, because text a screen shows
//! is a code, never a sentence in Rust (decision 46).
//!
//! A workspace's table of capabilities is not here. It is static, trusted
//! code in that workspace's registry (`stats_registry` for Statistics).
//! [`CapabilityDescriptor`] derives `Serialize` and never `Deserialize`, so no
//! payload, file, setting or model reply can add one.
//!
//! Lifecycle is enforced, not only shown (decision 48):
//! [`Lifecycle::runnable_in`] is the one rule. Planned and Deprecated never
//! run; Experimental runs only in a build with the app's `devtools` feature;
//! Beta and Stable run.

use serde::de::{Deserializer, Error as _};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use super::codes::closed;
use super::{CapabilityId, WorkspaceId};

/* ------------------------------------------------------------------------ *
 *  The confirmation a run carries
 * ------------------------------------------------------------------------ */

/// A confirmation a capability's run needs (blueprint §8.2's
/// `confirmation_refs`), held by its id: opaque, exactly 64 lowercase hex
/// digits, the form Level 1's confirmation token takes (decision 50). It
/// never holds data. Its `Debug` prints no digit, and it never goes into a
/// log, an error or an outbound struct.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConfirmationId(String);

impl ConfirmationId {
    /// Exactly 64 lowercase hex digits, or nothing.
    pub fn parse(id: &str) -> Option<ConfirmationId> {
        (id.len() == 64
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| ConfirmationId(id.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ConfirmationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConfirmationId(..)")
    }
}

impl Serialize for ConfirmationId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ConfirmationId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = String::deserialize(d)?;
        ConfirmationId::parse(&id).ok_or_else(|| D::Error::custom("not a confirmation id"))
    }
}

/* ------------------------------------------------------------------------ *
 *  Lifecycle and the build
 * ------------------------------------------------------------------------ */

closed! {
    /// A capability's readiness (blueprint §7.3, §11.4).
    pub enum Lifecycle in "" {
        /// On the roadmap only. Never runnable.
        Planned => "planned",
        /// Internal: runnable only in a build with the `devtools` feature.
        Experimental => "experimental",
        /// Usable, with its scope and limitations documented.
        Beta => "beta",
        /// Its acceptance suite and release criteria pass.
        Stable => "stable",
        /// Old results stay readable; no new runs.
        Deprecated => "deprecated",
    }
}

/// Which kind of build asks to run something. `Devtools` is the app built
/// with its `devtools` feature; the app passes `cfg!(feature = "devtools")`
/// (pinned by an app test).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Build {
    Standard,
    Devtools,
}

impl Build {
    pub fn from_devtools(devtools: bool) -> Build {
        if devtools {
            Build::Devtools
        } else {
            Build::Standard
        }
    }
}

impl Lifecycle {
    /// THE lifecycle rule (decision 48).
    pub fn runnable_in(self, build: Build) -> bool {
        match self {
            Lifecycle::Planned | Lifecycle::Deprecated => false,
            Lifecycle::Experimental => build == Build::Devtools,
            Lifecycle::Beta | Lifecycle::Stable => true,
        }
    }
}

closed! {
    /// Why a capability request was refused, by a registry or by a façade.
    /// Never names the id it was given: a refusal does not echo its input.
    pub enum CapabilityCode in "capability." {
        /// The id is not in the registry, or not one this façade serves.
        UnknownCapability => "unknown_capability",
        /// The spec's version is not the capability's. Params `expected`,
        /// `found`.
        VersionMismatch => "version_mismatch",
        /// The capability exists but this build may not run it. Param
        /// `lifecycle`.
        NotRunnable => "not_runnable",
        /// A computation would use a confirmation the spec does not carry,
        /// or the capability needs one and the spec carries none.
        ConfirmationMissing => "confirmation_missing",
        /// The spec carries a confirmation the computation does not use, or
        /// one for a capability that needs none.
        ConfirmationUnmatched => "confirmation_unmatched",
    }
}

/* ------------------------------------------------------------------------ *
 *  The descriptor
 * ------------------------------------------------------------------------ */

/// The memory a capability is expected to fit in (blueprint §17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RamClass {
    Gb8,
    Gb16,
    Gb32,
}

/// Whether a capability's run can be stopped once it has started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cancel {
    /// The run reads a cancel flag at its check points.
    Cooperative,
    /// The run cannot be stopped once it has started: it is one call.
    None,
}

/// What a capability reads and may use while it runs (blueprint §17). A limit
/// that does not exist is `None`, never a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceProfile {
    pub ram_class: RamClass,
    /// The most data rows read, the header not counted.
    pub max_rows: Option<u64>,
    pub max_columns: Option<u64>,
    /// The most bytes of input content read; the descriptor's documentation
    /// says which bytes (for a table, the cell text in UTF-8).
    pub max_input_bytes: Option<u64>,
    /// A wall-clock ceiling on the run, in milliseconds.
    pub ceiling_ms: Option<u64>,
    pub cancel: Cancel,
    pub uses_gpu: bool,
    /// Whether the run writes temporary files.
    pub temp_disk: bool,
}

/// A file format a capability reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    Csv,
    Tsv,
    Xlsx,
    Xlsm,
    Xlsb,
    Xls,
    Ods,
    Sav,
    Mat,
    Pdf,
    Docx,
    Txt,
    Md,
}

/// What a capability can export through the user's save dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    Csv,
    Json,
}

/// Whether a run needs the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineBehavior {
    /// The run makes no network request.
    RunsOffline,
    NeedsNetwork,
}

/// The test target that validates a capability, by its name. Set only in
/// trusted code; the registry's tests check each names a target that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SuiteId(pub &'static str);

/// The decision record that documents a capability, by its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct DocId(pub &'static str);

/// One executable capability (blueprint §7.2). Texts are codes of `W`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDescriptor<W> {
    pub id: CapabilityId,
    /// What a spec's `capability_version` must be.
    pub version: u32,
    pub workspace_id: WorkspaceId,
    pub title_code: W,
    pub description_code: W,
    pub lifecycle: Lifecycle,
    pub input_schema_version: u32,
    pub output_schema_version: u32,
    pub method_schema_version: u32,
    pub supported_formats: Vec<FileFormat>,
    pub dependency_ids: Vec<CapabilityId>,
    pub license_notes: Vec<W>,
    pub resource_profile: ResourceProfile,
    pub offline_behavior: OfflineBehavior,
    pub validation_suite_id: SuiteId,
    pub known_limitation_codes: Vec<W>,
    pub export_kinds: Vec<ExportKind>,
    pub documentation_id: DocId,
}
