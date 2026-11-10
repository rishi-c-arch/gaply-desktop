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
//! * `stats.analyze.one_way_anova`: the one-way ANOVA on the user's own data
//!   (Phase 8G, decision 312), with `stats_anova`'s id and version. Planned
//!   in 8G (decisions 321 and 324); Beta since 8G-2's Part 3 (decision 333),
//!   so its commands, runs and projects run through the façades.
//! * `stats.analyze.correlations`: Pearson's r, Spearman's rho and Kendall's
//!   tau-b on the user's own data (Phase 8H, decision 336), with
//!   `stats_corr`'s id and version. Planned in 8H (decision 344); Beta since
//!   8H-2's Part 1 (decision 351), so its commands, runs and projects run
//!   through the façades.
//! * `stats.analyze.cronbach_alpha`: Cronbach's alpha on the user's own data
//!   (Phase 8I, decision 367), with `stats_alpha`'s id and version. Planned
//!   in 8I (decision 372); Beta since 8I-2's Part 2 (decision 377), so its
//!   commands, runs and projects run through the façades.
//! * `stats.prepare.scale_score`: scale scores, a new numeric column on the
//!   user's data (Phase 8M, decision 408; ADR-011), with `stats_prep`'s id and
//!   version. Planned in 8M (decision 411); Beta since 8M-2's Part 2 (decision
//!   416), so 8M-3's commands can run in both builds. It has no run kind, no
//!   ceiling and no route.
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
use crate::stats_alpha::{CAPABILITY_ID as ALPHA_ID, CAPABILITY_VERSION as ALPHA_VERSION};
use crate::stats_anova::{CAPABILITY_ID as ANOVA_ID, CAPABILITY_VERSION as ANOVA_VERSION};
use crate::stats_corr::{CAPABILITY_ID as CORR_ID, CAPABILITY_VERSION as CORR_VERSION};
use crate::stats_desc::{CAPABILITY_ID, CAPABILITY_VERSION};
use crate::stats_l1::catalogue::{L1MethodId, MethodCatalogue};
use crate::stats_l1::compute::L1Registry;
use crate::stats_onesample::{ONE_SAMPLE_ID, ONE_SAMPLE_VERSION, PAIRED_ID, PAIRED_VERSION};
use crate::stats_prep::{CAPABILITY_ID as PREP_ID, CAPABILITY_VERSION as PREP_VERSION};
use crate::stats_ranks::{CAPABILITY_ID as RANKS_ID, CAPABILITY_VERSION as RANKS_VERSION};
use crate::stats_related_ranks::{
    CAPABILITY_ID as RELATED_RANKS_ID, CAPABILITY_VERSION as RELATED_RANKS_VERSION,
};
use crate::stats_ttest::{CAPABILITY_ID as TTEST_ID, CAPABILITY_VERSION as TTEST_VERSION};
use crate::stats_xtab::{CAPABILITY_ID as XTAB_ID, CAPABILITY_VERSION as XTAB_VERSION};

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
        /// p-values and quantiles come from statrs 0.18.0, MIT
        /// (`stats_l0::pvalue` for Level 0 and Level 1; `stats_dist`, on
        /// its `erfc` and `Normal`, for the methods).
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
        /// How large a file the methods on the user's data open depends on
        /// the computer's memory, and each method's screen states it
        /// (Research OS, Phase 8F-2, decision 288).
        LimitTableLimitsByMemory => "limit.table.limits_by_memory",
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
        AnalyzeOneWayAnovaTitle => "analyze.one_way_anova.title",
        AnalyzeOneWayAnovaDescription => "analyze.one_way_anova.description",
        /// Eta and omega squared have no confidence interval (decision 319).
        LimitAnalyzeEffectSizesHaveNoInterval => "limit.analyze.effect_sizes_have_no_interval",
        /// No Brown–Forsythe test of means, no trend tests and no planned
        /// contrasts (decision 319).
        LimitAnalyzeNoRobustMeansTrendsOrContrasts => "limit.analyze.no_robust_means_trends_or_contrasts",
        AnalyzeCorrelationsTitle => "analyze.correlations.title",
        AnalyzeCorrelationsDescription => "analyze.correlations.description",
        /// Pearson's p and interval assume bivariate normality, which is not
        /// checked (decision 344).
        LimitAnalyzePearsonNormalityNotChecked => "limit.analyze.pearson_normality_not_checked",
        /// Two-tailed p values only, and no partial correlations (decision
        /// 344).
        LimitAnalyzeTwoTailedNoPartial => "limit.analyze.two_tailed_no_partial",
        AnalyzeCronbachAlphaTitle => "analyze.cronbach_alpha.title",
        AnalyzeCronbachAlphaDescription => "analyze.cronbach_alpha.description",
        /// Alpha assumes the items measure one thing, which is not checked
        /// (decision 372).
        LimitAnalyzeAlphaOneDimension => "limit.analyze.alpha_one_dimension",
        /// McDonald's omega and squared multiple correlations are not shown
        /// (decision 372).
        LimitAnalyzeNoOmegaOrSmc => "limit.analyze.no_omega_or_smc",
        AnalyzeCrosstabsTitle => "analyze.crosstabs.title",
        AnalyzeCrosstabsDescription => "analyze.crosstabs.description",
        /// Two columns, no third, layer column (decision 579).
        LimitAnalyzeCrosstabsTwoColumnsOnly => "limit.analyze.crosstabs_two_columns_only",
        /// Fisher's exact test for a 2 x 2 table only (decision 579).
        LimitAnalyzeFisherTwoByTwoOnly => "limit.analyze.fisher_two_by_two_only",
        /// No lambda, Goodman and Kruskal's tau, uncertainty coefficient,
        /// linear-by-linear test or ordered measures (decision 579).
        LimitAnalyzeNoFurtherNominalMeasures => "limit.analyze.no_further_nominal_measures",
        /// Each row counts once; no weights (decision 579).
        LimitAnalyzeNoWeights => "limit.analyze.no_weights",
        /// Levels in code-point order, which sets phi's sign and Fisher's
        /// one-sided directions (decision 579).
        LimitAnalyzeLevelsInCodePointOrder => "limit.analyze.levels_in_code_point_order",
        AnalyzeIndependentRanksTitle => "analyze.independent_ranks.title",
        AnalyzeIndependentRanksDescription => "analyze.independent_ranks.description",
        /// No Dunn's test or other pairwise comparisons after the
        /// Kruskal-Wallis test (decision 606).
        LimitAnalyzeNoPairwiseRankComparisons => "limit.analyze.no_pairwise_rank_comparisons",
        /// Each Mann-Whitney p is two-sided (decision 606).
        LimitAnalyzeRankTestsTwoSidedOnly => "limit.analyze.rank_tests_two_sided_only",
        /// No Hodges-Lehmann estimate of the shift, and no interval for it
        /// (decision 606).
        LimitAnalyzeNoLocationShiftEstimate => "limit.analyze.no_location_shift_estimate",
        /// The exact p only without ties and below 50 values a group
        /// (decision 606).
        LimitAnalyzeExactPWithoutTiesOnly => "limit.analyze.exact_p_without_ties_only",
        /// No median test and no Jonckheere-Terpstra test (decision 606).
        LimitAnalyzeNoFurtherKSampleTests => "limit.analyze.no_further_k_sample_tests",
        /// Groups in code-point order, which sets the first group and the
        /// signs of z and the rank-biserial correlation (decision 606).
        LimitAnalyzeRankGroupsInCodePointOrder => "limit.analyze.rank_groups_in_code_point_order",
        AnalyzeRelatedRanksTitle => "analyze.related_ranks.title",
        AnalyzeRelatedRanksDescription => "analyze.related_ranks.description",
        /// No pairwise comparisons after a Friedman test (decision 640).
        LimitAnalyzeNoPairwiseRelatedComparisons => "limit.analyze.no_pairwise_related_comparisons",
        /// Each Wilcoxon p is two-sided (decision 640).
        LimitAnalyzeRelatedTestsTwoSidedOnly => "limit.analyze.related_tests_two_sided_only",
        /// No Hodges-Lehmann estimate of the differences (decision 640).
        LimitAnalyzeNoDifferencesLocationEstimate => "limit.analyze.no_differences_location_estimate",
        /// The Wilcoxon exact p's rule (decision 640).
        LimitAnalyzeSignedRankExactPRule => "limit.analyze.signed_rank_exact_p_rule",
        /// A pair of equal values is left out; Pratt's method is not offered
        /// (decision 640).
        LimitAnalyzeZeroDifferencesDropped => "limit.analyze.zero_differences_dropped",
        /// No sign test, marginal homogeneity test or Cochran's Q (decision 640).
        LimitAnalyzeNoFurtherRelatedTests => "limit.analyze.no_further_related_tests",
        /// The columns in the order the user chose, which sets the differences
        /// and the signs (decision 640).
        LimitAnalyzeRelatedColumnsInChosenOrder => "limit.analyze.related_columns_in_chosen_order",
        PrepareScaleScoreTitle => "prepare.scale_score.title",
        PrepareScaleScoreDescription => "prepare.scale_score.description",
        /// A score's items are the file's columns (ADR-011 question 13).
        LimitPrepareItemsAreFileColumns => "limit.prepare.items_are_file_columns",
        /// Recoding, formulas and selecting cases are not offered yet (decision
        /// 391).
        LimitPrepareNoRecodingFormulaOrSelection => "limit.prepare.no_recoding_formula_or_selection",
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
    /// `stats.analyze.one_way_anova` (Phase 8G).
    AnalyzeOneWayAnova,
    /// `stats.analyze.correlations` (Phase 8H).
    AnalyzeCorrelations,
    /// `stats.analyze.cronbach_alpha` (Phase 8I).
    AnalyzeCronbachAlpha,
    /// `stats.analyze.crosstabs` (Phase 8X-1).
    AnalyzeCrosstabs,
    /// `stats.analyze.independent_ranks` (Phase 8Y-1).
    AnalyzeIndependentRanks,
    /// `stats.analyze.related_ranks` (Phase 8Z-1).
    AnalyzeRelatedRanks,
    /// `stats.prepare.scale_score` (Phase 8M). Not a method: no run kind.
    PrepareScaleScore,
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
    // Planned in 8G (decision 321); Beta at the start of 8G-2's Part 3
    // (decision 333), so its commands, runs and projects can run.
    (ANOVA_ID, Lifecycle::Beta),
    // Planned in 8H (decision 344); Beta at the start of 8H-2's Part 1
    // (decision 351), so its commands, runs and projects can run.
    (CORR_ID, Lifecycle::Beta),
    // Planned in 8I (decision 372); Beta at the start of 8I-2's Part 2
    // (decision 377), so its commands, runs and projects can run.
    (ALPHA_ID, Lifecycle::Beta),
    // Planned in 8X-1's Part 2 (decision 578); Beta at the start of its Part 3
    // (decision 580), so its commands and runs can run.
    (XTAB_ID, Lifecycle::Beta),
    // Planned in 8Y-1's Part 2 (decision 605); Beta at the start of its Part 3
    // (decision 607), so its commands and runs can run.
    (RANKS_ID, Lifecycle::Beta),
    // Planned in 8Z-1's Part 2 (decision 639); Beta at the start of its Part 3
    // (decision 641), so its commands and runs can run.
    (RELATED_RANKS_ID, Lifecycle::Beta),
    // Planned in 8M (decision 411); Beta at the start of 8M-2's Part 2
    // (decision 416), so 8M-3's commands can run in both builds.
    (PREP_ID, Lifecycle::Beta),
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
/// The methods on the user's data (the summary and the three t-tests) since
/// Research OS Phase 8F-2 (decision 288): the rows and columns every memory
/// tier opens, `IngestLimits::for_tier`'s, copied. Their bytes depend on the
/// computer, so their profiles give none, and their limitation says so.
/// `TABLE_MAX_*` stay, for Level 1.
const METHOD_MAX_ROWS: u64 = 100_000;
const METHOD_MAX_COLUMNS: u64 = 128;
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
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("desc_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
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
            // p and the interval's quantile come through `stats_dist`,
            // still from statrs (its Student t at df ≤ 1e4; its erfc and
            // Normal above, Phase 8E).
            license_notes: vec![StatsWording::LicenseStatrsMit],
            // The descriptive summary's profile: the same ingest, and the
            // same run manager's ceiling and check point (Phase 4B-2).
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("ttest_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
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
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("onesample_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
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

/// The one-way ANOVA on the user's data (Phase 8G, decision 312): the
/// t-tests' profile, formats and licence; the group's levels and normality
/// limitations, as the independent t-test's, and its own two.
fn analyze_one_way_anova() -> Entry {
    Entry {
        kind: Kind::AnalyzeOneWayAnova,
        descriptor: StatsDescriptor {
            id: id(ANOVA_ID),
            version: ANOVA_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::AnalyzeOneWayAnovaTitle,
            description_code: StatsWording::AnalyzeOneWayAnovaDescription,
            lifecycle: lifecycle_of(ANOVA_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("anova_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeNormalityNotAssessed,
                StatsWording::LimitAnalyzeEffectSizesHaveNoInterval,
                StatsWording::LimitAnalyzeNoRobustMeansTrendsOrContrasts,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("methods/one_way_anova"),
        },
    }
}

/// The correlations on the user's data (Phase 8H, decisions 336 and 344):
/// the other methods' profile, formats and licence; the table capabilities'
/// two limitations and its own two.
fn analyze_correlations() -> Entry {
    Entry {
        kind: Kind::AnalyzeCorrelations,
        descriptor: StatsDescriptor {
            id: id(CORR_ID),
            version: CORR_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::AnalyzeCorrelationsTitle,
            description_code: StatsWording::AnalyzeCorrelationsDescription,
            lifecycle: lifecycle_of(CORR_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("corr_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzePearsonNormalityNotChecked,
                StatsWording::LimitAnalyzeTwoTailedNoPartial,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("methods/correlations"),
        },
    }
}

/// Cronbach's alpha on the user's data (Phase 8I, decisions 367 and 372):
/// the other methods' profile, formats and licence; the table capabilities'
/// two limitations and its own two.
fn analyze_cronbach_alpha() -> Entry {
    Entry {
        kind: Kind::AnalyzeCronbachAlpha,
        descriptor: StatsDescriptor {
            id: id(ALPHA_ID),
            version: ALPHA_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::AnalyzeCronbachAlphaTitle,
            description_code: StatsWording::AnalyzeCronbachAlphaDescription,
            lifecycle: lifecycle_of(ALPHA_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("alpha_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzeAlphaOneDimension,
                StatsWording::LimitAnalyzeNoOmegaOrSmc,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("methods/cronbach_alpha"),
        },
    }
}

/// Crosstabs and chi-square on the user's data (Phase 8X-1, decisions 575,
/// 578 and 579): the other methods' profile, formats and licence; the table
/// capabilities' two limitations, the levels' reading, and its own five.
fn analyze_crosstabs() -> Entry {
    Entry {
        kind: Kind::AnalyzeCrosstabs,
        descriptor: StatsDescriptor {
            id: id(XTAB_ID),
            version: XTAB_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::AnalyzeCrosstabsTitle,
            description_code: StatsWording::AnalyzeCrosstabsDescription,
            lifecycle: lifecycle_of(XTAB_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("crosstabs_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitDescribeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeCrosstabsTwoColumnsOnly,
                StatsWording::LimitAnalyzeFisherTwoByTwoOnly,
                StatsWording::LimitAnalyzeNoFurtherNominalMeasures,
                StatsWording::LimitAnalyzeNoWeights,
                StatsWording::LimitAnalyzeLevelsInCodePointOrder,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("methods/crosstabs"),
        },
    }
}

/// The Mann-Whitney and Kruskal-Wallis tests on the user's data (Phase 8Y-1,
/// decisions 602, 605 and 606): the other methods' profile, formats and
/// licence; the table capabilities' two limitations, the levels' reading as
/// crosstabs has it, crosstabs' weights, and its own six.
fn analyze_independent_ranks() -> Entry {
    Entry {
        kind: Kind::AnalyzeIndependentRanks,
        descriptor: StatsDescriptor {
            id: id(RANKS_ID),
            version: RANKS_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::AnalyzeIndependentRanksTitle,
            description_code: StatsWording::AnalyzeIndependentRanksDescription,
            lifecycle: lifecycle_of(RANKS_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("independent_ranks_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitDescribeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeNoWeights,
                StatsWording::LimitAnalyzeNoPairwiseRankComparisons,
                StatsWording::LimitAnalyzeRankTestsTwoSidedOnly,
                StatsWording::LimitAnalyzeNoLocationShiftEstimate,
                StatsWording::LimitAnalyzeExactPWithoutTiesOnly,
                StatsWording::LimitAnalyzeNoFurtherKSampleTests,
                StatsWording::LimitAnalyzeRankGroupsInCodePointOrder,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("methods/independent_ranks"),
        },
    }
}

/// The Wilcoxon signed-rank and Friedman tests on the user's data (Phase 8Z-1,
/// decisions 636, 639 and 640): the other methods' profile, formats and licence;
/// the table capabilities' two limitations, crosstabs' weights, and its own seven.
fn analyze_related_ranks() -> Entry {
    Entry {
        kind: Kind::AnalyzeRelatedRanks,
        descriptor: StatsDescriptor {
            id: id(RELATED_RANKS_ID),
            version: RELATED_RANKS_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::AnalyzeRelatedRanksTitle,
            description_code: StatsWording::AnalyzeRelatedRanksDescription,
            lifecycle: lifecycle_of(RELATED_RANKS_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: vec![StatsWording::LicenseStatrsMit],
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: Some(DESCRIBE_CEILING_MS),
                cancel: Cancel::Cooperative,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("related_ranks_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzeNoWeights,
                StatsWording::LimitAnalyzeNoPairwiseRelatedComparisons,
                StatsWording::LimitAnalyzeRelatedTestsTwoSidedOnly,
                StatsWording::LimitAnalyzeNoDifferencesLocationEstimate,
                StatsWording::LimitAnalyzeSignedRankExactPRule,
                StatsWording::LimitAnalyzeZeroDifferencesDropped,
                StatsWording::LimitAnalyzeNoFurtherRelatedTests,
                StatsWording::LimitAnalyzeRelatedColumnsInChosenOrder,
            ],
            export_kinds: vec![ExportKind::Csv, ExportKind::Json],
            documentation_id: DocId("methods/related_ranks"),
        },
    }
}

/// Scale scores (Phase 8M, decisions 408 and 411): the methods' rows, columns,
/// formats and table limitations, with no ceiling and no cancel (ADR-011 Part
/// 12): one fast call, measured at 0.18 s at 100,000 × 128 (decision 410), never
/// a run. No licence note: it uses no statrs code.
fn prepare_scale_score() -> Entry {
    Entry {
        kind: Kind::PrepareScaleScore,
        descriptor: StatsDescriptor {
            id: id(PREP_ID),
            version: PREP_VERSION,
            workspace_id: WorkspaceId::Statistics,
            title_code: StatsWording::PrepareScaleScoreTitle,
            description_code: StatsWording::PrepareScaleScoreDescription,
            lifecycle: lifecycle_of(PREP_ID),
            input_schema_version: 1,
            output_schema_version: 1,
            method_schema_version: 1,
            supported_formats: TABLE_FORMATS.to_vec(),
            dependency_ids: Vec::new(),
            license_notes: Vec::new(),
            resource_profile: ResourceProfile {
                ram_class: RamClass::Gb8,
                max_rows: Some(METHOD_MAX_ROWS),
                max_columns: Some(METHOD_MAX_COLUMNS),
                max_input_bytes: None,
                ceiling_ms: None,
                cancel: Cancel::None,
                uses_gpu: false,
                temp_disk: false,
            },
            offline_behavior: OfflineBehavior::RunsOffline,
            validation_suite_id: SuiteId("prep_r_golden"),
            known_limitation_codes: vec![
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitPrepareItemsAreFileColumns,
                StatsWording::LimitPrepareNoRecodingFormulaOrSelection,
            ],
            export_kinds: vec![ExportKind::Csv],
            documentation_id: DocId("methods/scale_scores"),
        },
    }
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
    /// `stats.analyze.independent_t`, `stats.analyze.one_sample_t`,
    /// `stats.analyze.paired_t`, `stats.analyze.one_way_anova`,
    /// `stats.analyze.correlations`, `stats.analyze.cronbach_alpha`,
    /// `stats.analyze.crosstabs`, `stats.analyze.independent_ranks`,
    /// `stats.analyze.related_ranks` and `stats.prepare.scale_score`.
    pub fn statistics() -> Registry {
        let mut entries = vec![check()];
        entries.extend(MethodCatalogue::ENTRIES.iter().map(|e| verify(e.id)));
        entries.push(describe());
        entries.push(analyze_independent_t());
        entries.push(analyze_one_sample_t());
        entries.push(analyze_paired_t());
        entries.push(analyze_one_way_anova());
        entries.push(analyze_correlations());
        entries.push(analyze_cronbach_alpha());
        entries.push(analyze_crosstabs());
        entries.push(analyze_independent_ranks());
        entries.push(analyze_related_ranks());
        entries.push(prepare_scale_score());
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
        assert_eq!(checked, 2 * 19, "nineteen capabilities, two builds");
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
                    // Phase 8F-2 (decision 288).
                    StatsWording::LimitTableLimitsByMemory,
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
            &ids[ids.len() - 10..ids.len() - 7],
            [TTEST_ID, ONE_SAMPLE_ID, PAIRED_ID],
            "appended after the independent t-test, one-sample first"
        );
    }

    /// Research OS, Phase 8F-2 (decision 288): the methods on the user's
    /// data say their limits depend on the computer's memory, and carry the
    /// rows and columns every tier shares and no bytes; Level 1 keeps the
    /// cell text cap and its profile, unchanged.
    #[test]
    fn the_methods_state_the_limits_by_memory_and_level_1_keeps_the_cell_text_cap() {
        let (mut methods, mut verify) = (0, 0);
        for e in Registry::statistics().entries() {
            let codes = &e.descriptor.known_limitation_codes;
            let p = e.descriptor.resource_profile;
            let id = e.descriptor.id.as_str();
            match e.kind {
                Kind::Describe
                | Kind::AnalyzeIndependentT
                | Kind::AnalyzeOneSampleT
                | Kind::AnalyzePairedT
                | Kind::AnalyzeOneWayAnova
                | Kind::AnalyzeCorrelations
                | Kind::AnalyzeCronbachAlpha
                | Kind::AnalyzeCrosstabs
                | Kind::AnalyzeIndependentRanks
                | Kind::AnalyzeRelatedRanks
                | Kind::PrepareScaleScore => {
                    methods += 1;
                    assert!(
                        codes.contains(&StatsWording::LimitTableLimitsByMemory),
                        "{id}"
                    );
                    assert!(
                        !codes.contains(&StatsWording::LimitTableCellTextCap),
                        "{id}"
                    );
                    assert_eq!(
                        (p.max_rows, p.max_columns, p.max_input_bytes),
                        (Some(100_000), Some(128), None),
                        "{id}"
                    );
                }
                Kind::Verify(_) => {
                    verify += 1;
                    assert!(codes.contains(&StatsWording::LimitTableCellTextCap), "{id}");
                    assert!(
                        !codes.contains(&StatsWording::LimitTableLimitsByMemory),
                        "{id}"
                    );
                    assert_eq!(
                        (p.max_rows, p.max_columns, p.max_input_bytes),
                        (Some(5_000), Some(128), Some(2_097_152)),
                        "{id}"
                    );
                }
                Kind::Check => {
                    assert!(
                        !codes.contains(&StatsWording::LimitTableLimitsByMemory),
                        "{id}"
                    );
                }
            }
        }
        assert_eq!((methods, verify), (11, 7));
        assert_eq!((METHOD_MAX_ROWS, METHOD_MAX_COLUMNS), (100_000, 128));
    }

    /// Phase 8G (decisions 321 and 324): the one-way ANOVA is listed, with
    /// its own title, description and limitations, and its validation suite
    /// and page named. Planned in 8G and refused in both builds; Beta since
    /// 8G-2's Part 3 (decisions 325 and 333), so runnable in both. Last until
    /// 8H, now just before the correlations (decision 344).
    #[test]
    fn the_one_way_anova_is_listed_as_beta_and_runs_in_both_builds() {
        let t = Registry::statistics();
        let id = CapabilityId::parse(ANOVA_ID).unwrap();
        let e = t.lookup(&id).expect("listed");
        let d = &e.descriptor;
        assert_eq!(e.kind, Kind::AnalyzeOneWayAnova);
        assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta));
        assert_eq!(
            (d.title_code, d.description_code),
            (
                StatsWording::AnalyzeOneWayAnovaTitle,
                StatsWording::AnalyzeOneWayAnovaDescription
            )
        );
        assert_eq!(
            d.known_limitation_codes,
            [
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeNormalityNotAssessed,
                StatsWording::LimitAnalyzeEffectSizesHaveNoInterval,
                StatsWording::LimitAnalyzeNoRobustMeansTrendsOrContrasts,
            ]
        );
        assert_eq!(
            (d.validation_suite_id, d.documentation_id),
            (SuiteId("anova_r_golden"), DocId("methods/one_way_anova"))
        );
        for build in [Build::Standard, Build::Devtools] {
            t.require_runnable(&id, 1, build)
                .unwrap_or_else(|r| panic!("Beta runs in {build:?}: {r:?}"));
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            &ids[ids.len() - 7..ids.len() - 5],
            [ANOVA_ID, CORR_ID],
            "appended after the paired t-test, the correlations after it"
        );
    }

    /// Phase 8H (decision 344): the correlations are listed, with their own
    /// title, description and limitations, and their validation suite and
    /// page named. Planned in 8H and refused in both builds; Beta since
    /// 8H-2's Part 1 (decision 351), so runnable in both. Last until 8I, now
    /// just before Cronbach's alpha (decision 372).
    #[test]
    fn the_correlations_are_listed_as_beta_and_run_in_both_builds() {
        let t = Registry::statistics();
        let id = CapabilityId::parse(CORR_ID).unwrap();
        let e = t.lookup(&id).expect("listed");
        let d = &e.descriptor;
        assert_eq!(e.kind, Kind::AnalyzeCorrelations);
        assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta));
        assert_eq!(
            (d.title_code, d.description_code),
            (
                StatsWording::AnalyzeCorrelationsTitle,
                StatsWording::AnalyzeCorrelationsDescription
            )
        );
        assert_eq!(
            d.known_limitation_codes,
            [
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzePearsonNormalityNotChecked,
                StatsWording::LimitAnalyzeTwoTailedNoPartial,
            ]
        );
        assert_eq!(
            (d.validation_suite_id, d.documentation_id),
            (SuiteId("corr_r_golden"), DocId("methods/correlations"))
        );
        for build in [Build::Standard, Build::Devtools] {
            t.require_runnable(&id, 1, build)
                .unwrap_or_else(|r| panic!("Beta runs in {build:?}: {r:?}"));
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            &ids[ids.len() - 6..ids.len() - 4],
            [CORR_ID, ALPHA_ID],
            "appended after the one-way ANOVA, Cronbach's alpha after it"
        );
    }

    /// Phase 8I (decision 372): Cronbach's alpha is listed, with its own title,
    /// description and limitations, and its validation suite and page named.
    /// Planned in 8I and refused in both builds; Beta since 8I-2's Part 2
    /// (decision 377), so runnable in both. Last until 8M, just before scale
    /// scores until 8X-1, now just before crosstabs (decision 578).
    #[test]
    fn cronbach_alpha_is_listed_as_beta_and_runs_in_both_builds() {
        let t = Registry::statistics();
        let id = CapabilityId::parse(ALPHA_ID).unwrap();
        let e = t.lookup(&id).expect("listed");
        let d = &e.descriptor;
        assert_eq!(e.kind, Kind::AnalyzeCronbachAlpha);
        assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta));
        assert_eq!(
            (d.title_code, d.description_code),
            (
                StatsWording::AnalyzeCronbachAlphaTitle,
                StatsWording::AnalyzeCronbachAlphaDescription
            )
        );
        assert_eq!(
            d.known_limitation_codes,
            [
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzeAlphaOneDimension,
                StatsWording::LimitAnalyzeNoOmegaOrSmc,
            ]
        );
        assert_eq!(
            (d.validation_suite_id, d.documentation_id),
            (SuiteId("alpha_r_golden"), DocId("methods/cronbach_alpha"))
        );
        for build in [Build::Standard, Build::Devtools] {
            t.require_runnable(&id, 1, build)
                .unwrap_or_else(|r| panic!("Beta runs in {build:?}: {r:?}"));
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            &ids[ids.len() - 5..ids.len() - 3],
            [ALPHA_ID, XTAB_ID],
            "appended after the correlations, crosstabs after it (decision 578)"
        );
    }

    /// Phase 8X-1 (decision 578): crosstabs is listed after Cronbach's alpha and
    /// before scale scores, with its own title, description and limitations,
    /// the levels' reading among them, and its validation suite and page named.
    /// Planned in Part 2 and refused in both builds; Beta since Part 3
    /// (decision 580), so runnable in both.
    #[test]
    fn crosstabs_is_listed_as_beta_and_runs_in_both_builds() {
        let t = Registry::statistics();
        let id = CapabilityId::parse(XTAB_ID).unwrap();
        let e = t.lookup(&id).expect("listed");
        let d = &e.descriptor;
        assert_eq!(e.kind, Kind::AnalyzeCrosstabs);
        assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta));
        assert_eq!(
            (d.title_code, d.description_code),
            (
                StatsWording::AnalyzeCrosstabsTitle,
                StatsWording::AnalyzeCrosstabsDescription
            )
        );
        assert_eq!(
            d.known_limitation_codes,
            [
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitDescribeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeCrosstabsTwoColumnsOnly,
                StatsWording::LimitAnalyzeFisherTwoByTwoOnly,
                StatsWording::LimitAnalyzeNoFurtherNominalMeasures,
                StatsWording::LimitAnalyzeNoWeights,
                StatsWording::LimitAnalyzeLevelsInCodePointOrder,
            ]
        );
        assert!(
            !d.known_limitation_codes
                .contains(&StatsWording::LimitAnalyzeLevelsFollowLevel1),
            "the user chooses no two levels"
        );
        assert_eq!(
            (d.validation_suite_id, d.documentation_id),
            (SuiteId("crosstabs_r_golden"), DocId("methods/crosstabs"))
        );
        for build in [Build::Standard, Build::Devtools] {
            t.require_runnable(&id, 1, build)
                .unwrap_or_else(|r| panic!("Beta runs in {build:?}: {r:?}"));
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            &ids[ids.len() - 4..ids.len() - 2],
            [XTAB_ID, RANKS_ID],
            "appended after Cronbach's alpha, the rank tests after it (decision 605)"
        );
    }

    /// Phase 8Y-1 (decision 605): the rank tests are listed after crosstabs and
    /// before scale scores, with their own title, description and
    /// limitations, the levels' reading and crosstabs' weights among them,
    /// and their validation suite and page named. Planned in Part 2 and refused
    /// in both builds; Beta since Part 3 (decision 607), so runnable in both.
    #[test]
    fn the_rank_tests_are_listed_as_beta_and_run_in_both_builds() {
        let t = Registry::statistics();
        let id = CapabilityId::parse(RANKS_ID).unwrap();
        let e = t.lookup(&id).expect("listed");
        let d = &e.descriptor;
        assert_eq!(e.kind, Kind::AnalyzeIndependentRanks);
        assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta));
        assert_eq!(
            (d.title_code, d.description_code),
            (
                StatsWording::AnalyzeIndependentRanksTitle,
                StatsWording::AnalyzeIndependentRanksDescription
            )
        );
        assert_eq!(
            d.known_limitation_codes,
            [
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitDescribeLevelsFollowLevel1,
                StatsWording::LimitAnalyzeNoWeights,
                StatsWording::LimitAnalyzeNoPairwiseRankComparisons,
                StatsWording::LimitAnalyzeRankTestsTwoSidedOnly,
                StatsWording::LimitAnalyzeNoLocationShiftEstimate,
                StatsWording::LimitAnalyzeExactPWithoutTiesOnly,
                StatsWording::LimitAnalyzeNoFurtherKSampleTests,
                StatsWording::LimitAnalyzeRankGroupsInCodePointOrder,
            ]
        );
        assert!(
            !d.known_limitation_codes
                .contains(&StatsWording::LimitAnalyzeLevelsFollowLevel1),
            "the user's levels are included or left out, never two compared"
        );
        assert_eq!(
            (d.validation_suite_id, d.documentation_id),
            (
                SuiteId("independent_ranks_r_golden"),
                DocId("methods/independent_ranks")
            )
        );
        for build in [Build::Standard, Build::Devtools] {
            t.require_runnable(&id, 1, build)
                .unwrap_or_else(|r| panic!("Beta runs in {build:?}: {r:?}"));
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            &ids[ids.len() - 3..ids.len() - 1],
            [RANKS_ID, RELATED_RANKS_ID],
            "appended after crosstabs, the related rank tests after it"
        );
    }

    /// Phase 8Z-1 (decision 639): the related rank tests are listed after the
    /// independent rank tests and before scale scores, with their own title,
    /// description and limitations and their validation suite and page named.
    /// Beta since Part 3 (decision 641), so runnable in both builds.
    #[test]
    fn the_related_rank_tests_are_listed_and_named() {
        let t = Registry::statistics();
        let id = CapabilityId::parse(RELATED_RANKS_ID).unwrap();
        let e = t.lookup(&id).expect("listed");
        let d = &e.descriptor;
        assert_eq!(e.kind, Kind::AnalyzeRelatedRanks);
        assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta));
        assert_eq!(
            (d.title_code, d.description_code),
            (
                StatsWording::AnalyzeRelatedRanksTitle,
                StatsWording::AnalyzeRelatedRanksDescription
            )
        );
        assert_eq!(
            d.known_limitation_codes,
            [
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitAnalyzeNoWeights,
                StatsWording::LimitAnalyzeNoPairwiseRelatedComparisons,
                StatsWording::LimitAnalyzeRelatedTestsTwoSidedOnly,
                StatsWording::LimitAnalyzeNoDifferencesLocationEstimate,
                StatsWording::LimitAnalyzeSignedRankExactPRule,
                StatsWording::LimitAnalyzeZeroDifferencesDropped,
                StatsWording::LimitAnalyzeNoFurtherRelatedTests,
                StatsWording::LimitAnalyzeRelatedColumnsInChosenOrder,
            ]
        );
        assert_eq!(
            (d.validation_suite_id, d.documentation_id),
            (
                SuiteId("related_ranks_r_golden"),
                DocId("methods/related_ranks")
            )
        );
        for build in [Build::Standard, Build::Devtools] {
            assert!(t.require_runnable(&id, 1, build).is_ok(), "Beta runs in {build:?}");
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            &ids[ids.len() - 2..],
            [RELATED_RANKS_ID, PREP_ID],
            "appended after the rank tests, scale scores after it"
        );
    }

    /// Phase 8M (decision 411): scale scores are listed last, with their own
    /// title, description and limitations, the methods' rows and columns with
    /// no ceiling and no cancel, and their validation suite and page named.
    /// Planned in 8M and refused in both builds; Beta since 8M-2's Part 2
    /// (decision 416), so they run in both.
    #[test]
    fn scale_scores_are_listed_last_as_beta_and_run_in_both_builds() {
        let t = Registry::statistics();
        let id = CapabilityId::parse(PREP_ID).unwrap();
        let e = t.lookup(&id).expect("listed");
        let d = &e.descriptor;
        assert_eq!(e.kind, Kind::PrepareScaleScore);
        assert_eq!((d.version, d.lifecycle), (1, Lifecycle::Beta));
        assert_eq!(
            (d.title_code, d.description_code),
            (
                StatsWording::PrepareScaleScoreTitle,
                StatsWording::PrepareScaleScoreDescription
            )
        );
        assert_eq!(
            d.known_limitation_codes,
            [
                StatsWording::LimitTableTextFilesUtf8Only,
                StatsWording::LimitTableLimitsByMemory,
                StatsWording::LimitPrepareItemsAreFileColumns,
                StatsWording::LimitPrepareNoRecodingFormulaOrSelection,
            ]
        );
        assert_eq!(
            (d.validation_suite_id, d.documentation_id),
            (SuiteId("prep_r_golden"), DocId("methods/scale_scores"))
        );
        let p = d.resource_profile;
        assert_eq!(
            (p.max_rows, p.max_columns, p.max_input_bytes),
            (Some(100_000), Some(128), None)
        );
        assert_eq!((p.ceiling_ms, p.cancel), (None, Cancel::None));
        for build in [Build::Standard, Build::Devtools] {
            assert!(
                t.require_runnable(&id, 1, build).is_ok(),
                "Beta in {build:?}"
            );
        }
        let ids: Vec<String> = t.ids().iter().map(|i| i.as_str().to_string()).collect();
        assert_eq!(
            ids.last().map(String::as_str),
            Some(PREP_ID),
            "appended last"
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
