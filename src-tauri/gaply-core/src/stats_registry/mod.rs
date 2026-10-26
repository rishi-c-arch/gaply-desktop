//! **The Statistics workspace's capability table (Research OS, Phase 4A):
//! what the verifier and the descriptive summary are, as capabilities, and
//! the one place that says whether a spec may run.**
//!
//! Pure: no file, network, clock, model or log (`tests/contracts_are_pure.rs`
//! scans this directory). Static and trusted: the table is built from Rust
//! constants only. No AI output, file or setting can add a capability, and a
//! [`StatsDescriptor`] has no `Deserialize` (decision 47).
//!
//! The capabilities (decision 47):
//! * `stats.check`: Level 0, the free Stats Check. Stable.
//! * `stats.verify.<method_id>`: one per method of Level 1's catalogue,
//!   DERIVED from `MethodCatalogue::ENTRIES`, so the catalogue stays the one
//!   source. A method `L1Registry` implements is Beta; one that is only in
//!   the catalogue is Planned. Today all seven are implemented.
//! * `stats.describe`: the descriptive summary, Beta, with Phase 2A's id and
//!   version (`stats_desc::CAPABILITY_ID`, `CAPABILITY_VERSION`).
//! * `stats.analyze.independent_t`: the independent-samples t-test as an
//!   analysis of the user's own data, Beta (Phase 4B-1, decision 61), with
//!   `stats_ttest`'s id and version.
//! * `stats.analyze.one_sample_t` and `stats.analyze.paired_t`: the one-sample
//!   and paired t-tests on the user's own data (Phase 8D-1, decision 253),
//!   with `stats_onesample`'s ids and versions. Beta since Phase 8D-2a
//!   (decision 265): their commands, runs and projects exist, and their
//!   screens are 8D-2b's.
//!
//! `run_stats_preview` and `run_stats_verify` are the legacy lane, not
//! capabilities: they are not here.
//!
//! [`require_runnable`] is the check every façade makes first
//! (`stats_adapters`): the id must be in the table, the version must be the
//! capability's, and its lifecycle must be runnable in this build
//! ([`Lifecycle::runnable_in`]). Each refusal is a closed
//! `capability.*` code that never names the id it was given.
//!
//! The resource profiles are literals here, because this module may not read
//! the app's constants; an app-crate test pins each one to the constant it
//! copies (`supplementary::MAX_ROWS`, `MAX_COLS`, `MAX_TOTAL_CHARS`,
//! `runs::RUN_CEILING_MS`), and a `gaply_core` test pins Level 0's input cap
//! to `import_guard::MAX_BYTES`, so neither side can drift.

use crate::contracts::capability::{
    Build, Cancel, CapabilityCode, CapabilityDescriptor, DocId, ExportKind, FileFormat, Lifecycle,
    OfflineBehavior, RamClass, ResourceProfile, SuiteId,
};
use crate::contracts::codes::closed;
use crate::contracts::workspace::{self, WorkspaceDescriptor};
use crate::contracts::{CapabilityId, Diagnostic, ParamKey, ParamValue, WorkspaceId};
use crate::stats_desc::{CAPABILITY_ID, CAPABILITY_VERSION};
use crate::stats_l1::catalogue::{L1MethodId, MethodCatalogue};
use crate::stats_l1::compute::L1Registry;
use crate::stats_onesample::{ONE_SAMPLE_ID, ONE_SAMPLE_VERSION, PAIRED_ID, PAIRED_VERSION};
use crate::stats_ttest::{CAPABILITY_ID as TTEST_ID, CAPABILITY_VERSION as TTEST_VERSION};

/// The free Stats Check's id.
pub const CHECK_ID: &str = "stats.check";
/// Every Level 1 capability's id starts with this.
pub const VERIFY_PREFIX: &str = "stats.verify.";

closed! {
    /// What the screens say about a Statistics capability, as codes their
    /// wording table turns into words (decision 46). Phase 4B's method
    /// selector is the first screen to show them.
    pub enum StatsWording in "stats.capability." {
        CheckTitle => "check.title",
        CheckDescription => "check.description",
        VerifyIndependentTTitle => "verify.independent_t.title",
        VerifyPairedTTitle => "verify.paired_t.title",
        VerifyOneWayAnovaTitle => "verify.one_way_anova.title",
        VerifyPearsonTitle => "verify.pearson.title",
        VerifySpearmanTitle => "verify.spearman.title",
        VerifyChiSquareIndependenceTitle => "verify.chi_square_independence.title",
        VerifyOlsTitle => "verify.ols.title",
        VerifyDescription => "verify.description",
        DescribeTitle => "describe.title",
        DescribeDescription => "describe.description",
        /// p-values come from statrs 0.18.0, MIT (`stats_l0::pvalue`).
        LicenseStatrsMit => "license.statrs_mit",
        /// `MethodRegistry::SUMMARY` (GRIM, percent and count) never runs
        /// on a manuscript: Level 0 cannot read its inputs from one.
        LimitCheckSummaryMethodsNotOnManuscripts => "limit.check.summary_methods_not_on_manuscripts",
        /// Table cells are read as tables only from a `.docx`; a PDF or
        /// plain text has no table objects.
        LimitCheckTablesFromDocxOnly => "limit.check.tables_from_docx_only",
        /// A CSV or TSV must be UTF-8: Latin-1 and UTF-16 are refused as
        /// malformed (evaluation of Phases 2 and 3, F10).
        LimitTableTextFilesUtf8Only => "limit.table.text_files_utf8_only",
        /// 2 MiB of cell text binds before the row and column caps on a wide
        /// numeric table (ADR-010 §1).
        LimitTableCellTextCap => "limit.table.cell_text_cap",
        /// Levels follow Level 1's reading of cells (decision 17).
        LimitDescribeLevelsFollowLevel1 => "limit.describe.levels_follow_level_1",
        /// Cells are left out, never rows, so rows used equals rows read
        /// (evaluation F6).
        LimitDescribeRowsUsedEqualsRowsRead => "limit.describe.rows_used_equals_rows_read",
        AnalyzeIndependentTTitle => "analyze.independent_t.title",
        AnalyzeIndependentTDescription => "analyze.independent_t.description",
        /// The group's levels follow Level 1's reading of cells (decision 17),
        /// in code-point order, and are named by index (decision 65).
        LimitAnalyzeLevelsFollowLevel1 => "limit.analyze.levels_follow_level_1",
        /// Normality is not assessed (decision 64).
        LimitAnalyzeNormalityNotAssessed => "limit.analyze.normality_not_assessed",
        /// Cohen's d has no confidence interval (decision 63).
        LimitAnalyzeCohensDHasNoInterval => "limit.analyze.cohens_d_has_no_interval",
        AnalyzeOneSampleTTitle => "analyze.one_sample_t.title",
        AnalyzeOneSampleTDescription => "analyze.one_sample_t.description",
        AnalyzePairedTTitle => "analyze.paired_t.title",
        AnalyzePairedTDescription => "analyze.paired_t.description",
        /// The values' normality is not assessed (decision 256).
        LimitAnalyzeValuesNormalityNotAssessed => "limit.analyze.values_normality_not_assessed",
        /// The differences' normality is not assessed (decision 256).
        LimitAnalyzeDifferencesNormalityNotAssessed => "limit.analyze.differences_normality_not_assessed",
    }
}

/// A Statistics capability's descriptor.
pub type StatsDescriptor = CapabilityDescriptor<StatsWording>;

/// Which façade serves a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Check,
    Verify(L1MethodId),
    Describe,
    /// `stats.analyze.independent_t` (Phase 4B-1).
    AnalyzeIndependentT,
    /// `stats.analyze.one_sample_t` (Phase 8D-1).
    AnalyzeOneSampleT,
    /// `stats.analyze.paired_t` (Phase 8D-1).
    AnalyzePairedT,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub descriptor: StatsDescriptor,
}

/// The lifecycle of the capabilities that are not Level 1 methods
/// (decision 47). A static table, next to the two rules for Level 1.
const LIFECYCLES: &[(&str, Lifecycle)] = &[
    (CHECK_ID, Lifecycle::Stable),
    (CAPABILITY_ID, Lifecycle::Beta),
    (TTEST_ID, Lifecycle::Beta),
    // Beta since 8D-2a (decision 265).
    (ONE_SAMPLE_ID, Lifecycle::Beta),
    (PAIRED_ID, Lifecycle::Beta),
];
/// A Level 1 method `L1Registry` implements.
const VERIFY_IMPLEMENTED: Lifecycle = Lifecycle::Beta;
/// A Level 1 method that is in the catalogue only.
const VERIFY_PLANNED: Lifecycle = Lifecycle::Planned;

/// What every table capability reads (`stats_l1_ingest::TABLE_EXTENSIONS`,
/// pinned by an app test).
const TABLE_FORMATS: &[FileFormat] = &[
    FileFormat::Sav,
    FileFormat::Mat,
    FileFormat::Xlsx,
    FileFormat::Xls,
    FileFormat::Xlsb,
    FileFormat::Ods,
    FileFormat::Xlsm,
    FileFormat::Csv,
    FileFormat::Tsv,
];
/// What Level 0 reads a manuscript from (`extract::docparse::parse_path`).
const MANUSCRIPT_FORMATS: &[FileFormat] = &[
    FileFormat::Pdf,
    FileFormat::Docx,
    FileFormat::Txt,
    FileFormat::Md,
];

/// The table ingest's caps: `supplementary::MAX_ROWS`, `MAX_COLS` and
/// `MAX_TOTAL_CHARS` (cell text, UTF-8 bytes after clamping), copied.
const TABLE_MAX_ROWS: u64 = 5_000;
const TABLE_MAX_COLUMNS: u64 = 128;
const TABLE_MAX_CELL_TEXT_BYTES: u64 = 2_097_152;
/// `runs::RUN_CEILING_MS`, copied.
const DESCRIBE_CEILING_MS: u64 = 30_000;
/// `import_guard::MAX_BYTES`: the manuscript file's bytes, copied.
const MANUSCRIPT_MAX_BYTES: u64 = 104_857_600;

fn id(word: &str) -> CapabilityId {
    CapabilityId::parse(word).expect("a well-formed Statistics capability id")
}

fn lifecycle_of(word: &str) -> Lifecycle {
    LIFECYCLES
        .iter()
        .find(|(w, _)| *w == word)
        .map(|(_, l)| *l)
        .expect("every capability that is not a Level 1 method has a lifecycle")
}

fn verify_title(m: L1MethodId) -> StatsWording {
    match m {
        L1MethodId::IndependentT => StatsWording::VerifyIndependentTTitle,
        L1MethodId::PairedT => StatsWording::VerifyPairedTTitle,
        L1MethodId::OneWayAnova => StatsWording::VerifyOneWayAnovaTitle,
        L1MethodId::Pearson => StatsWording::VerifyPearsonTitle,
        L1MethodId::Spearman => StatsWording::VerifySpearmanTitle,
        L1MethodId::ChiSquareIndependence => StatsWording::VerifyChiSquareIndependenceTitle,
        L1MethodId::Ols => StatsWording::VerifyOlsTitle,
    }
}

/// A method's version as a number. Every method's is "1" today; a version
/// that is not a number is a build error caught by every registry test.
fn method_version(m: L1MethodId) -> u32 {
    m.version()
        .parse()
        .expect("a Level 1 method version is a whole number")
}

fn check() -> Entry {
    Entry {
        kind: Kind::Check,
        descriptor: StatsDescriptor {
            id: id(CHECK_ID),
            version: 1,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::CheckTitle,
            description_code: StatsWording::CheckDescription,
            lifecycle: lifecycle_of(CHECK_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: MANUSCRIPT_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: None,
                max_columns: None,
                max_input_bytes: Some(MANUSCRIPT_MAX_BYTES),
                ceiling_ms: None,
                cancel: Cancel::None,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("every_method_has_fixtures"),
            known_limitation_codes: vec![
                StatsWording::LimitCheckSummaryMethodsNotOnManuscripts,
                StatsWording::LimitCheckTablesFromDocxOnly,
            ],
            export_kinds: Vec::new(),
            documentation_id: DocId("D269"),
        },
    }
}

fn verify(m: L1MethodId) -> Entry {
    let lifecycle = if L1Registry::get(m).is_some() {
        VERIFY_IMPLEMENTED
    } else {
        VERIFY_PLANNED
    };
    Entry {
        kind: Kind::Verify(m),
        descriptor: StatsDescriptor {
            id: id(&format!("{VERIFY_PREFIX}{}", m.id())),
            version: method_version(m),
            workspace_id: WorkspaceId::Statistics,
            title_code: verify_title(m),
            description_code: StatsWording::VerifyDescription,
            lifecycle,
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: method_version(m),
            supported_formats: TABLE_FORMATS.to_vec(),
            // Level 1's open runs Level 0 on the manuscript first.
            dependency_ids: vec![id(CHECK_ID)],
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(TABLE_MAX_ROWS),
                max_columns: Some(TABLE_MAX_COLUMNS),
                max_input_bytes: Some(TABLE_MAX_CELL_TEXT_BYTES),
                ceiling_ms: None,
                cancel: Cancel::None,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("l1_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableCellTextCap,
            ],
            export_kinds: Vec::new(),
            documentation_id: DocId("D270"),
        },
    }
}

fn describe() -> Entry {
    Entry {
        kind: Kind::Describe,
        descriptor: StatsDescriptor {
            id: id(CAPABILITY_ID),
            version: CAPABILITY_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::DescribeTitle,
            description_code: StatsWording::DescribeDescription,
            lifecycle: lifecycle_of(CAPABILITY_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: Vec::new(),
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(TABLE_MAX_ROWS),
                max_columns: Some(TABLE_MAX_COLUMNS),
                max_input_bytes: Some(TABLE_MAX_CELL_TEXT_BYTES),
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("desc_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableCellTextCap,
                StatsWording::LimitDescribeLevelsFollowLevel1,
                StatsWording::LimitDescribeRowsUsedEqualsRowsRead,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("D271"),
        },
    }
}

fn analyze_independent_t() -> Entry {
    Entry {
        kind: Kind::AnalyzeIndependentT,
        descriptor: StatsDescriptor {
            id: id(TTEST_ID),
            version: TTEST_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::AnalyzeIndependentTTitle,
            description_code: StatsWording::AnalyzeIndependentTDescription,
            lifecycle: lifecycle_of(TTEST_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            // p and the interval's quantile come from statrs's Student t
            // through `stats_l0::pvalue`.
            license_notes: vec![StatsWording::LicenseStatrsMit],
            // The descriptive summary's profile: the same ingest, and the
            // same run manager's ceiling and check point (Phase 4B-2).
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(TABLE_MAX_ROWS),
                max_columns: Some(TABLE_MAX_COLUMNS),
                max_input_bytes: Some(TABLE_MAX_CELL_TEXT_BYTES),
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("ttest_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableCellTextCap,
                StatsWording::LimitAnalyzeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeNormalityNotAssessed,
                StatsWording::LimitAnalyzeCohensDHasNoInterval,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("methods/independent_t"),
        },
    }
}

/// One of the t family's analyses that are not the independent t-test
/// (Phase 8D-1): the independent t-test's profile, formats and licence, and
/// its own normality limitation instead of the levels one, since neither
/// reads a group's levels.
fn analyze_t(
    kind: Kind,
    word: &str,
    version: u32,
    title: StatsWording,
    description: StatsWording,
    normality: StatsWording,
    documentation: &'static str,
) -> Entry {
    Entry {
        kind,
        descriptor: StatsDescriptor {
            id: id(word),
            version,
            workspace_id: WorkspaceId::Statistics,
            title_code: title,
            description_code: description,
            lifecycle: lifecycle_of(word),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(TABLE_MAX_ROWS),
                max_columns: Some(TABLE_MAX_COLUMNS),
                max_input_bytes: Some(TABLE_MAX_CELL_TEXT_BYTES),
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("onesample_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableCellTextCap,
                normality,
                StatsWording::LimitAnalyzeCohensDHasNoInterval,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId(documentation),
        },
    }
}

fn analyze_one_sample_t() -> Entry {
    analyze_t(
        Kind::AnalyzeOneSampleT,
        ONE_SAMPLE_ID,
        ONE_SAMPLE_VERSION,
        StatsWording::AnalyzeOneSampleTTitle,
        StatsWording::AnalyzeOneSampleTDescription,
        StatsWording::LimitAnalyzeValuesNormalityNotAssessed,
        "methods/one_sample_t",
    )
}

fn analyze_paired_t() -> Entry {
    analyze_t(
        Kind::AnalyzePairedT,
        PAIRED_ID,
        PAIRED_VERSION,
        StatsWording::AnalyzePairedTTitle,
        StatsWording::AnalyzePairedTDescription,
        StatsWording::LimitAnalyzeDifferencesNormalityNotAssessed,
        "methods/paired_t",
    )
}

/// A capability table. The Statistics one is [`Registry::statistics`]; a
/// different table exists only inside this crate's own unit tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    /// THE Statistics table: `stats.check`, one `stats.verify.<method_id>`
    /// per catalogue entry in its order, `stats.describe`, then
    /// `stats.analyze.independent_t`, `stats.analyze.one_sample_t` and
    /// `stats.analyze.paired_t`.
    pub fn statistics() -> Registry {
        let mut entries = vec![check()];
        entries.extend(MethodCatalogue::ENTRIES.iter().map(|e| verify(e.id)));
        entries.push(describe());
        entries.push(analyze_independent_t());
        entries.push(analyze_one_sample_t());
        entries.push(analyze_paired_t());
        Registry { entries }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Every capability id, in the table's order.
    pub fn ids(&self) -> Vec<CapabilityId> {
        self.entries
            .iter()
            .map(|e| e.descriptor.id.clone())
            .collect()
    }

    /// The entry for `id`, or `capability.unknown_capability`.
    pub fn lookup(&self, id: &CapabilityId) -> Result<&Entry, Diagnostic<CapabilityCode>> {
        self.entries
            .iter()
            .find(|e| e.descriptor.id == *id)
            .ok_or_else(|| Diagnostic::error(CapabilityCode::UnknownCapability))
    }

    /// The check before any run: a registered id, the capability's version,
    /// and a lifecycle this build may run. Refused, in that order, with
    /// `unknown_capability`, `version_mismatch {expected, found}` or
    /// `not_runnable {lifecycle}`.
    pub fn require_runnable(
        &self,
        id: &CapabilityId,
        version: u32,
        build: Build,
    ) -> Result<&Entry, Diagnostic<CapabilityCode>> {
        let entry = self.lookup(id)?;
        let d = &entry.descriptor;
        if d.version != version {
            return Err(Diagnostic::error(CapabilityCode::VersionMismatch)
                .int(ParamKey::Expected, i64::from(d.version))
                .int(ParamKey::Found, i64::from(version)));
        }
        if !d.lifecycle.runnable_in(build) {
            return Err(Diagnostic::error(CapabilityCode::NotRunnable)
                .with(ParamKey::Lifecycle, ParamValue::Lifecycle(d.lifecycle)));
        }
        Ok(entry)
    }

    /// A copy with one capability's lifecycle changed: a table for the states
    /// the real one has no capability in. For tests only: a test passes such
    /// a table to the code under test as a parameter (decision 96), and
    /// `src-tauri/tests/verifier_facade_guards.rs` fails if production source
    /// calls this. A copy can never reach the wire: a descriptor has no
    /// `Deserialize`, and the commands build their table with [`Self::statistics`].
    pub fn with_lifecycle(mut self, word: &str, lifecycle: Lifecycle) -> Registry {
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.descriptor.id.as_str() == word)
            .expect("the capability is in the table");
        e.descriptor.lifecycle = lifecycle;
        self
    }

    /// A copy without one capability: a table in which `word` is not
    /// registered. For tests only, under the same guard as
    /// [`Self::with_lifecycle`].
    pub fn without(mut self, word: &str) -> Registry {
        let before = self.entries.len();
        self.entries.retain(|e| e.descriptor.id.as_str() != word);
        assert_eq!(
            self.entries.len() + 1,
            before,
            "{word} is in the table once"
        );
        self
    }
}

/// [`Registry::lookup`] in the Statistics table.
pub fn lookup(id: &CapabilityId) -> Result<Entry, Diagnostic<CapabilityCode>> {
    Registry::statistics().lookup(id).cloned()
}

/// [`Registry::require_runnable`] in the Statistics table.
pub fn require_runnable(
    id: &CapabilityId,
    version: u32,
    build: Build,
) -> Result<Entry, Diagnostic<CapabilityCode>> {
    Registry::statistics()
        .require_runnable(id, version, build)
        .cloned()
}

/// A workspace's descriptor, with Statistics' capability ids read from this
/// table, so there is one list (decision 47). `workspace_list` serves these.
pub fn workspace_descriptor(id: WorkspaceId) -> WorkspaceDescriptor {
    let capability_ids = match id {
        WorkspaceId::Statistics => Registry::statistics().ids(),
        WorkspaceId::Compute | WorkspaceId::Design => Vec::new(),
    };
    workspace::descriptor_with(id, capability_ids)
}

/// Every workspace's descriptor, in `WorkspaceId::ALL`'s order.
pub fn workspace_descriptors() -> Vec<WorkspaceDescriptor> {
    WorkspaceId::ALL
        .iter()
        .map(|w| workspace_descriptor(*w))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_with_one_in_each_state() -> Registry {
        Registry::statistics()
            .with_lifecycle("stats.verify.pearson", Lifecycle::Planned)
            .with_lifecycle("stats.verify.spearman", Lifecycle::Experimental)
            .with_lifecycle("stats.verify.ols", Lifecycle::Deprecated)
    }

    #[test]
    fn each_lifecycle_runs_only_where_decision_48_says_in_both_builds() {
        let t = table_with_one_in_each_state();
        let states: Vec<Lifecycle> = t.entries().iter().map(|e| e.descriptor.lifecycle).collect();
        for l in [
            Lifecycle::Planned,
            Lifecycle::Experimental,
            Lifecycle::Beta,
            Lifecycle::Stable,
            Lifecycle::Deprecated,
        ] {
            assert!(states.contains(&l), "the test table has a {l:?} capability");
        }
        let expect = |l: Lifecycle, b: Build| match (l, b) {
            (Lifecycle::Planned | Lifecycle::Deprecated, _) => false,
            (Lifecycle::Experimental, Build::Standard) => false,
            (Lifecycle::Experimental, Build::Devtools) => true,
            (Lifecycle::Beta | Lifecycle::Stable, _) => true,
        };
        let mut checked = 0;
        for build in [Build::Standard, Build::Devtools] {
            for e in t.entries() {
                let d = &e.descriptor;
                let got = t.require_runnable(&d.id, d.version, build);
                if expect(d.lifecycle, build) {
                    assert!(
                        got.is_ok(),
                        "{} ({:?}) in {build:?}: {got:?}",
                        d.id.as_str(),
                        d.lifecycle
                    );
                } else {
                    let refusal = got.expect_err("refused");
                    assert_eq!(
                        refusal.code,
                        CapabilityCode::NotRunnable,
                        "{}",
                        d.id.as_str()
                    );
                    assert_eq!(
                        refusal.params.get(&ParamKey::Lifecycle),
                        Some(&ParamValue::Lifecycle(d.lifecycle))
                    );
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 2 * 12, "twelve capabilities, two builds");
    }

    /// Phase 8D-1 (decision 253), made Beta in 8D-2a (decision 265): the
    /// one-sample and paired t-tests are listed, Beta, runnable in both
    /// builds, each with its own title, description and limitations.
    #[test]
    fn the_one_sample_and_paired_t_tests_are_listed_as_beta_and_runnable() {
        let t = Registry::statistics();
        for (word, kind, title, description, normality) in [
            (
                ONE_SAMPLE_ID,
                Kind::AnalyzeOneSampleT,
                StatsWording::AnalyzeOneSampleTTitle,
                StatsWording::AnalyzeOneSampleTDescription,
                StatsWording::LimitAnalyzeValuesNormalityNotAssessed,
            ),
            (
                PAIRED_ID,
                Kind::AnalyzePairedT,
                StatsWording::AnalyzePairedTTitle,
                StatsWording::AnalyzePairedTDescription,
                StatsWording::LimitAnalyzeDifferencesNormalityNotAssessed,
            ),
        ] {
            let id = CapabilityId::parse(word).unwrap();
            let e = t.lookup(&id).expect("listed");
            let d = &e.descriptor;
            assert_eq!(e.kind, kind, "{word}");
            assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta), "{word}");
            assert_eq!(
                (d.title_code, d.description_code),
                (title, description),
                "{word}"
            );
            assert_eq!(
                d.known_limitation_codes,
                [
                    StatsWording::LimitTableTextFilesUtf8Only,
                    StatsWording::LimitTableCellTextCap,
                    normality,
                    StatsWording::LimitAnalyzeCohensDHasNoInterval,
                ],
                "{word}: no levels limitation, since neither reads levels"
            );
            for build in [Build::Standard, Build::Devtools] {
                t.require_runnable(&id, 1, build)
                    .unwrap_or_else(|r| panic!("{word}: Beta runs in {build:?}: {r:?}"));
            }
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            &ids[ids.len() - 3..],
            [TTEST_ID, ONE_SAMPLE_ID, PAIRED_ID],
            "appended after the independent t-test, one-sample first"
        );
    }

    #[test]
    fn a_planned_catalogue_method_is_never_runnable() {
        let t = Registry::statistics().with_lifecycle("stats.verify.paired_t", Lifecycle::Planned);
        let pid = CapabilityId::parse("stats.verify.paired_t").unwrap();
        for build in [Build::Standard, Build::Devtools] {
            let r = t.require_runnable(&pid, 1, build).unwrap_err();
            assert_eq!(r.code, CapabilityCode::NotRunnable);
        }
    }

    #[test]
    fn refusals_come_in_order_unknown_then_version_then_lifecycle() {
        let t = table_with_one_in_each_state();
        let unknown = CapabilityId::parse("stats.verify.kruskal").unwrap();
        assert_eq!(
            t.require_runnable(&unknown, 1, Build::Devtools)
                .unwrap_err()
                .code,
            CapabilityCode::UnknownCapability
        );
        // A Planned capability at the wrong version is a version refusal.
        let planned = CapabilityId::parse("stats.verify.pearson").unwrap();
        let v = t
            .require_runnable(&planned, 2, Build::Devtools)
            .unwrap_err();
        assert_eq!(v.code, CapabilityCode::VersionMismatch);
        assert_eq!(v.params.get(&ParamKey::Expected), Some(&ParamValue::Int(1)));
        assert_eq!(v.params.get(&ParamKey::Found), Some(&ParamValue::Int(2)));
    }
}
