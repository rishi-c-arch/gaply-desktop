//! **`src/contracts/` and `src/stats_desc/` are pure, the contracts name no
//! domain, and only the types listed here may derive `Serialize`.**
//!
//! The Research OS's guard (Phase 2A), in the shape of
//! `stats_l1_is_llm_free_and_offline.rs`: a scanned directory, a forbidden
//! list, an import allowlist, a self-check, and a deletion test recorded in
//! the Phase 2A report. Six checks:
//!
//! 1. No route to a file, the network, a process, the environment, a
//!    thread, the clock, a database, Tauri, an async runtime, a model, the
//!    proxy, stdout or a log, in either directory.
//! 2. `use` roots: `std`, `serde`, `serde_json` and `sha2` from outside the
//!    crate. Inside it, `contracts` may name only itself; `stats_desc` may
//!    name `crate::contracts`, `crate::stats_l1::dataset`, and exactly
//!    `crate::stats_verify::{mean, sample_variance, RangeFault}` (the stable
//!    two-pass mean of part C and the variance over its range-checked sum of
//!    squares) and `crate::stats_l1::binding::suspect_codes_in` (the rule
//!    that uses `MISSING_CODE_CANDIDATES`). Every `crate::` path anywhere in
//!    the code is held to the same list, so an inline path cannot route
//!    around a `use` line.
//! 3. Layering: `contracts` names no module of the verifier or a capability
//!    and no type they declare. The type list is read from their source,
//!    not typed here.
//! 4. `Serialize`: in `stats_desc`, exactly the types listed below derive
//!    it; those that hold a header or a level label are listed apart; none
//!    of them holds a cell, the dataset or the tally. In `contracts`, no
//!    field holds a `String` except the two checked newtypes.
//! 5. A type in `stats_desc` with a `String` field does not derive `Debug`
//!    and prints counts through its own `impl Debug`.
//! 6. Every struct that derives `Deserialize` refuses unknown fields (or is
//!    a transparent newtype), and every named-field struct on the wire is
//!    camelCase.
//!
//! Phase 4A adds `src/stats_registry/` and `src/stats_adapters/` to checks 1,
//! 2 and 6, each with its own import list, and both to the modules
//! `contracts` must never name (check 3):
//! * `stats_registry` may name `crate::contracts`, and exactly
//!   `crate::stats_l1::catalogue::{L1MethodId, MethodCatalogue}`,
//!   `crate::stats_l1::compute::L1Registry` and
//!   `crate::stats_desc::{CAPABILITY_ID, CAPABILITY_VERSION}`;
//! * `stats_adapters` may name `crate::contracts`, `crate::stats_registry`,
//!   `crate::stats_l0`, `crate::stats_l1`, `crate::stats_desc` (decision 46),
//!   and of `crate::extract` exactly `ExtractionResult` (Level 0's input),
//!   `SectionKind` and `extract_from_text` (its unit tests). Inline, only those
//!   three `crate::extract` paths are allowed, so no inline path can reach
//!   `extract`'s file readers.
//!
//! Phase 4B-1 adds `src/stats_ttest/` to every check, with its own lists
//! (decision 62), and to the modules `contracts` must never name:
//! * `stats_ttest` may name `crate::contracts`, and exactly
//!   `crate::stats_verify::{mean, sample_variance, t_test_student,
//!   t_test_welch, RangeFault}` (the `se` field on their result is the
//!   standard-error item, which needs no import; `RangeFault` keeps overflow
//!   and underflow apart, decision 70), `crate::stats_l0::pvalue::t_two_tailed`,
//!   `crate::stats_l1::binding::suspect_codes_in`, `crate::stats_l1::dataset`,
//!   `crate::stats_desc::{input_ref, Alias}` and
//!   `crate::stats_desc::export::text` (one formula-injection rule);
//! * in `stats_ttest`, exactly the types listed below derive `Serialize`;
//!   `GroupSummary` and `TTestResult` hold level labels, and `Split` (the
//!   values) and `Ready` (the checked columns and levels) derive nothing;
//! * `stats_registry` may also name `stats_ttest`'s id and version, and
//!   `stats_adapters` may name `crate::stats_ttest`.
//!
//! Phase 5B-1 adds `src/research_project/`, the pure project model
//! (decisions 99 to 112, ADR-009), to checks 1, 2, 3 and 6: no route to a
//! file, a database, the clock or a log (its I/O lives in the app crate's
//! `project_store`, the one module allowed to write); `use` roots `std`,
//! `serde`, `serde_json`, `sha2`, and inside the crate `crate::contracts`
//! only; and `contracts` never names it or a type it declares. Its property
//! test is in `tests/research_project_properties.rs`, since this scan reads
//! test code too.
//!
//! Phase 8E adds `src/stats_dist/`, the methods' distribution functions
//! (decision 275), to checks 1, 2 and 3: no route to a file, the network, a
//! process, the clock, a model or a log; from outside the crate `std` and
//! `statrs` (its `erfc` and `Normal`), the roots passed to the import scan
//! for that directory alone, since adding statrs to `EXTERNAL_ROOTS` would
//! let every scanned module import it; inside the crate exactly
//! `crate::stats_l0::pvalue::{t_two_tailed as l0_t_two_tailed, f_upper as
//! l0_f_upper}`, Level 0's two tails under aliases because its own functions
//! have the same names (the list holds the alias text, as `REGISTRY_ITEMS`
//! does); and `contracts` never names it. `stats_ttest`'s p and quantile
//! come from `crate::stats_dist` now (`t_quantile_975`, `MEASURED_MAX_DF as
//! QUANTILE_MAX_DF`, `t_two_tailed`), and `stats_onesample` takes
//! `two_sided_p` from `stats_ttest` and names no `stats_l0` item.
//!
//! Phase 8G-2 adds `src/stats_anova/`, the one-way ANOVA (decisions 312
//! and 326), to every check, with its own lists: inside the crate
//! `crate::contracts`, `crate::stats_desc::{input_ref, Alias}`,
//! `crate::stats_desc::export::text` (its export's one formula-injection
//! rule), `crate::stats_dist`'s six items, `crate::stats_l1::binding::
//! suspect_codes_in`, `crate::stats_l1::dataset`, `crate::stats_ttest`'s five
//! and exactly `crate::stats_verify::{mean, one_way_anova, sample_variance,
//! RangeFault}`; exactly the types listed below derive `Serialize`, of which
//! `GroupRow` and `AnovaOutcome` hold level labels; `Split` (the values) and
//! `Ready` (the checked columns and levels) derive nothing; `contracts` never
//! names it; and `stats_adapters` may name `crate::stats_anova`.
//!
//! Phase 8H adds `src/stats_corr/`, the correlations (decisions 339 and
//! 343), with the module, not after it: inside the crate `crate::contracts`,
//! `crate::stats_desc::{input_ref, Alias}`, exactly `crate::stats_dist::
//! {normal_quantile_975, normal_two_tailed, t_two_tailed}`,
//! `crate::stats_l1::binding::suspect_codes_in`, `crate::stats_l1::dataset`,
//! `crate::stats_ttest::{column, non_finite_cells}` and exactly
//! `crate::stats_verify::{mean, pearson_clamped, sample_variance, spearman,
//! CorrelationResult, RangeFault}`; exactly the types listed below derive
//! `Serialize`, and no type of the module holds a `String`; `Rows` and
//! `Entry` (the values) derive nothing; and `contracts` never names it.
//!
//! Phase 8I adds `src/stats_alpha/`, Cronbach's alpha (decisions 368 and
//! 372), with the module, not after it: inside the crate `crate::contracts`,
//! `crate::stats_desc::{input_ref, Alias}`, exactly `crate::stats_dist::
//! f_quantile_upper`, `crate::stats_l1::binding::suspect_codes_in`,
//! `crate::stats_l1::compute::RHO_ONE_ULPS`, `crate::stats_l1::dataset`,
//! `crate::stats_ttest::{column, non_finite_cells}` and exactly
//! `crate::stats_verify::{mean, pearson_clamped, sample_variance,
//! RangeFault}`; exactly the types listed below derive `Serialize`, and no
//! type of the module holds a `String`; `Rows` and `Entry` (the values)
//! derive nothing; `contracts` never names it; and `stats_registry` may name
//! its id and version. Phase 8I-2 (decision 378) reads its two files, the
//! export's with them, which may name exactly `crate::stats_desc::export::
//! text`; and `stats_adapters` may name it, for its façade.
//!
//! Phase 8M-2 (decisions 417 and 418): `stats_desc` names `stats_prep`'s hash-level
//! function and versions, and holds the shared checks every method names by
//! exact name: `preparation_holds`, `score_code_count`,
//! `suspect_codes_unless_score` and, in the t-test and the ANOVA,
//! `is_prepared_column`. So the five other method modules no longer name
//! `crate::stats_l1::binding`.
//!
//! Phase 8M adds `src/stats_prep/`, scale scores (decision 408), with the
//! module, not after it: inside the crate `crate::contracts`, exactly
//! `crate::research_project::is_invisible_format`, exactly
//! `crate::stats_alpha::{reverse, outside_scale_count, row_total, ScaleEnds}`,
//! `crate::stats_desc::{Alias}`, `crate::stats_desc::export::{number, text}`,
//! `crate::stats_l1::dataset`, `crate::stats_ttest::{column,
//! non_finite_cells}` and exactly `crate::stats_verify::mean`. Exactly the
//! types listed below derive `Serialize`; one of them, `ScaleScoreStep`, holds
//! text, the name the user typed (ADR-011 Part 4), so it prints counts through
//! its own `Debug`, as `stats_desc`'s text-holding types do; no other type of
//! the module holds a `String`, and none names a cell or the dataset but
//! `Prepared`, which derives nothing. `contracts` never names it, and
//! `stats_registry` may name its id and version.
//!
//! What it cannot see: a route through a helper in an allowed module (a
//! function of `stats_l1::dataset` that read a file would not be caught
//! here; `stats_l1_is_llm_free_and_offline.rs` guards that directory), and
//! a macro from another crate that expands to I/O.

use std::path::{Path, PathBuf};

const CONTRACTS: &str = "src/contracts";
const STATS_DESC: &str = "src/stats_desc";
const STATS_REGISTRY: &str = "src/stats_registry";
const STATS_ADAPTERS: &str = "src/stats_adapters";
const STATS_TTEST: &str = "src/stats_ttest";
const STATS_ONESAMPLE: &str = "src/stats_onesample";
const STATS_DIST: &str = "src/stats_dist";
const STATS_ANOVA: &str = "src/stats_anova";
const STATS_XTAB: &str = "src/stats_xtab";
const STATS_RANKS: &str = "src/stats_ranks";
const STATS_RELATED_RANKS: &str = "src/stats_related_ranks";
const STATS_CORR: &str = "src/stats_corr";
const STATS_ALPHA: &str = "src/stats_alpha";
const STATS_PREP: &str = "src/stats_prep";
const RESEARCH_PROJECT: &str = "src/research_project";

const FORBIDDEN: &[(&str, &str)] = &[
    ("std::fs", "the filesystem"),
    ("File::", "the filesystem"),
    ("OpenOptions", "the filesystem"),
    ("std::path", "a path"),
    ("PathBuf", "a path"),
    ("std::io", "I/O"),
    ("std::net", "the network"),
    ("reqwest", "an HTTP client"),
    ("ureq", "an HTTP client"),
    ("hyper", "an HTTP client"),
    ("std::process", "a process"),
    ("Command::", "a process"),
    ("std::env", "the environment"),
    ("std::thread", "a thread"),
    ("std::time", "the clock"),
    ("SystemTime", "the clock"),
    ("Instant", "the clock"),
    ("tokio", "an async runtime"),
    ("tauri", "the app shell"),
    ("rusqlite", "the database"),
    ("Connection", "the database"),
    ("proxy", "the cloud proxy"),
    ("Proxy", "the cloud proxy"),
    ("stats_assist", "the helper layer"),
    ("candle", "a model runtime"),
    ("tokenizers", "a model runtime"),
    ("Generative", "a generative model"),
    ("Embedder", "an embedding model"),
    ("llm", "a language model"),
    ("print!", "stdout"),
    ("println!", "stdout"),
    ("dbg!", "stderr"),
    ("tracing", "a log"),
    ("log::", "a log"),
    ("unsafe", "unchecked code"),
];

const EXTERNAL_ROOTS: &[&str] = &["std", "serde", "serde_json", "sha2"];

const CONTRACTS_CRATE_PATHS: &[&str] = &["crate::contracts"];
/// What the pure project model may name inside the crate (Phase 5B-1).
const RESEARCH_PROJECT_CRATE_PATHS: &[&str] = &["crate::contracts"];
const STATS_DESC_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_l1::dataset",
    "crate::stats_verify",
    "crate::stats_l1::binding",
    "crate::stats_prep",
];

const STATS_REGISTRY_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_l1::catalogue",
    "crate::stats_l1::compute",
    "crate::stats_desc",
    "crate::stats_ttest",
    "crate::stats_onesample",
    "crate::stats_anova",
    "crate::stats_corr",
    "crate::stats_alpha",
    "crate::stats_prep",
    "crate::stats_xtab",
    "crate::stats_ranks",
    "crate::stats_related_ranks",
];
/// What `stats_registry` may import from the catalogue, Level 1's registry
/// and the descriptive summary, by exact name. The one-way ANOVA's id and
/// version since Phase 8G (decision 321), the correlations' since 8H
/// (decision 344).
const REGISTRY_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_l1::catalogue",
        &["L1MethodId", "MethodCatalogue"],
    ),
    ("crate::stats_l1::compute", &["L1Registry"]),
    (
        "crate::stats_desc",
        &["CAPABILITY_ID", "CAPABILITY_VERSION"],
    ),
    (
        "crate::stats_ttest",
        &[
            "CAPABILITY_ID as TTEST_ID",
            "CAPABILITY_VERSION as TTEST_VERSION",
        ],
    ),
    (
        "crate::stats_onesample",
        &[
            "ONE_SAMPLE_ID",
            "ONE_SAMPLE_VERSION",
            "PAIRED_ID",
            "PAIRED_VERSION",
        ],
    ),
    (
        "crate::stats_anova",
        &[
            "CAPABILITY_ID as ANOVA_ID",
            "CAPABILITY_VERSION as ANOVA_VERSION",
        ],
    ),
    (
        "crate::stats_corr",
        &[
            "CAPABILITY_ID as CORR_ID",
            "CAPABILITY_VERSION as CORR_VERSION",
        ],
    ),
    // Phase 8I (decision 372): Cronbach's alpha's id and version.
    (
        "crate::stats_alpha",
        &[
            "CAPABILITY_ID as ALPHA_ID",
            "CAPABILITY_VERSION as ALPHA_VERSION",
        ],
    ),
    // Phase 8M (decisions 408 and 411): scale scores' id and version.
    (
        "crate::stats_prep",
        &[
            "CAPABILITY_ID as PREP_ID",
            "CAPABILITY_VERSION as PREP_VERSION",
        ],
    ),
    // Phase 8X-1 (decision 578): crosstabs' id and version.
    (
        "crate::stats_xtab",
        &[
            "CAPABILITY_ID as XTAB_ID",
            "CAPABILITY_VERSION as XTAB_VERSION",
        ],
    ),
    // Phase 8Y-1 (decision 605): the rank tests' id and version.
    (
        "crate::stats_ranks",
        &[
            "CAPABILITY_ID as RANKS_ID",
            "CAPABILITY_VERSION as RANKS_VERSION",
        ],
    ),
    // Phase 8Z-1 (decision 639): the related rank tests' id and version.
    (
        "crate::stats_related_ranks",
        &[
            "CAPABILITY_ID as RELATED_RANKS_ID",
            "CAPABILITY_VERSION as RELATED_RANKS_VERSION",
        ],
    ),
];
const STATS_ADAPTERS_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_registry",
    "crate::stats_l0",
    "crate::stats_l1",
    "crate::stats_desc",
    "crate::stats_ttest",
    "crate::stats_onesample",
    "crate::stats_anova",
    "crate::stats_corr",
    "crate::stats_alpha",
    "crate::stats_prep",
    "crate::stats_xtab",
    "crate::stats_ranks",
    "crate::stats_related_ranks",
    "crate::extract",
];
/// The one module outside decision 46's list `stats_adapters` reads, by
/// exact name.
const ADAPTER_ITEMS: &[(&str, &[&str])] = &[(
    "crate::extract",
    &["ExtractionResult", "SectionKind", "extract_from_text"],
)];
/// Inline `crate::` paths in `stats_adapters`: `crate::extract` only by
/// those exact names.
const STATS_ADAPTERS_INLINE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_registry",
    "crate::stats_l0",
    "crate::stats_l1",
    "crate::stats_desc",
    "crate::stats_ttest",
    "crate::stats_onesample",
    "crate::stats_anova",
    "crate::stats_corr",
    "crate::stats_alpha",
    "crate::stats_prep",
    "crate::stats_xtab",
    "crate::stats_ranks",
    "crate::stats_related_ranks",
    "crate::extract::ExtractionResult",
    "crate::extract::SectionKind",
    "crate::extract::extract_from_text",
];

/// The verifier's items `stats_desc` may import, by exact name.
const VERIFIER_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &["mean", "sample_variance", "RangeFault"],
    ),
    (
        "crate::stats_l1::binding",
        &["suspect_codes_in", "SuspectMissingCode"],
    ),
    // Phase 8M-2 (decision 417): the preparation check's hash and versions.
    (
        "crate::stats_prep",
        &[
            "prepared_input_hash",
            "CAPABILITY_VERSION as PREP_VERSION",
            "ENGINE_VERSION as PREP_ENGINE_VERSION",
        ],
    ),
];

/// What `stats_ttest` may name inside the crate. The export's path comes
/// before the descriptive summary's, so it is matched first.
const STATS_TTEST_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_dist",
    "crate::stats_l1::dataset",
    "crate::stats_verify",
];
/// The items `stats_ttest` may import, by exact name (decision 62).
const TTEST_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &[
            "mean",
            "sample_variance",
            "t_test_student",
            "t_test_welch",
            "RangeFault",
        ],
    ),
    (
        "crate::stats_dist",
        &[
            "t_quantile_975",
            "MEASURED_MAX_DF as QUANTILE_MAX_DF",
            "t_two_tailed",
        ],
    ),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "is_prepared_column",
            "preparation_holds",
            "score_code_count",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_onesample` may name inside the crate (Phase 8D-1). The
/// export's path comes before the descriptive summary's, so it is matched
/// first.
const STATS_ONESAMPLE_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_l1::dataset",
    "crate::stats_verify",
    "crate::stats_ttest",
];
/// The items `stats_onesample` may import, by exact name (decision 254):
/// only what it imports.
const ONESAMPLE_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &[
            "mean",
            "sample_variance",
            "t_test_one_sample",
            "t_test_paired",
            "RangeFault",
        ],
    ),
    (
        "crate::stats_ttest",
        &[
            "column",
            "essentially_constant",
            "interval_95",
            "non_finite_cells",
            "two_sided_p",
        ],
    ),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "preparation_holds",
            "score_code_count",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_anova` may name inside the crate (Phase 8G-2, decision 326).
/// The export's path comes before the descriptive summary's, so it is
/// matched first.
const STATS_ANOVA_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_dist",
    "crate::stats_l1::dataset",
    "crate::stats_ttest",
    "crate::stats_verify",
];
/// The items `stats_anova` may import, by exact name (decisions 313 and
/// 326): only what it imports.
const ANOVA_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &["mean", "one_way_anova", "sample_variance", "RangeFault"],
    ),
    (
        "crate::stats_dist",
        &[
            "f_upper",
            "studentized_range_quantile",
            "studentized_range_upper",
            "t_quantile",
            "t_two_tailed",
            "SRANGE_P_FLOOR",
        ],
    ),
    (
        "crate::stats_ttest",
        &[
            "column",
            "group_levels",
            "interval_95",
            "non_finite_cells",
            "MAX_GROUP_LEVELS",
        ],
    ),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "is_prepared_column",
            "preparation_holds",
            "score_code_count",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_xtab` may name inside the crate (Phase 8X-1, decision 577).
/// The export's path comes before the descriptive summary's, so it is
/// matched first.
const STATS_XTAB_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_dist",
    "crate::stats_l1::dataset",
    "crate::stats_ttest",
    "crate::stats_verify",
];
/// The items `stats_xtab` may import, by exact name (decisions 575 and
/// 577): only what it imports, Pearson's and Yates's statistics from
/// `stats_verify`, the one implementation.
const XTAB_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &["chi_square_2x2_yates", "chi_square_independence"],
    ),
    ("crate::stats_dist", &["chi_square_upper", "fisher_2x2"]),
    ("crate::stats_l1::dataset", &["Cell", "Dataset"]),
    (
        "crate::stats_ttest",
        &["column", "group_levels", "MAX_GROUP_LEVELS"],
    ),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "is_prepared_column",
            "preparation_holds",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_ranks` may name inside the crate (Phase 8Y-1, decision 604).
/// The export's path comes before the descriptive summary's, so it is
/// matched first.
const STATS_RANKS_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_anova",
    "crate::stats_corr",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_dist",
    "crate::stats_l1::dataset",
    "crate::stats_ttest",
    "crate::stats_verify",
];
/// The items `stats_ranks` may import, by exact name (decisions 602 and
/// 604): only what it imports. The ranks, the tie runs and the medians come
/// from their one implementation each: `stats_verify::average_ranks`
/// (Spearman's), `stats_corr::runs` and `stats_anova::median`.
const RANKS_ITEMS: &[(&str, &[&str])] = &[
    ("crate::stats_verify", &["average_ranks"]),
    ("crate::stats_corr", &["runs", "tie_cube_sum"]),
    ("crate::stats_anova", &["median"]),
    (
        "crate::stats_dist",
        &["chi_square_upper", "exact_two_sided", "normal_two_tailed", "rank_sum_exact_tails"],
    ),
    ("crate::stats_l1::dataset", &["Cell", "Dataset"]),
    (
        "crate::stats_ttest",
        &["column", "group_levels", "non_finite_cells", "MAX_GROUP_LEVELS"],
    ),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "is_prepared_column",
            "preparation_holds",
            "score_code_count",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_related_ranks` may name inside the crate (Phase 8Z-1, decision
/// 638). The export's path comes before the descriptive summary's, so it is
/// matched first.
const STATS_RELATED_RANKS_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_anova",
    "crate::stats_corr",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_dist",
    "crate::stats_l1::dataset",
    "crate::stats_ttest",
    "crate::stats_verify",
];
/// The items `stats_related_ranks` may import, by exact name: only what it
/// imports. The ranks, the tie runs and the medians come from their one
/// implementation each.
const RELATED_RANKS_ITEMS: &[(&str, &[&str])] = &[
    ("crate::stats_verify", &["average_ranks"]),
    ("crate::stats_corr", &["runs", "tie_cube_sum"]),
    ("crate::stats_anova", &["median"]),
    (
        "crate::stats_dist",
        &["chi_square_upper", "exact_two_sided", "normal_two_tailed", "signed_rank_exact_tails"],
    ),
    ("crate::stats_l1::dataset", &["Cell", "Dataset"]),
    ("crate::stats_ttest", &["column", "non_finite_cells"]),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "preparation_holds",
            "score_code_count",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_corr` may name inside the crate (Phase 8H, decision 343).
/// The export's path comes before the descriptive summary's, so it is
/// matched first; Level 1's compute only for its one constant (8H-2,
/// decisions 352 and 353).
const STATS_CORR_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_dist",
    "crate::stats_l1::compute",
    "crate::stats_l1::dataset",
    "crate::stats_ttest",
    "crate::stats_verify",
];
/// The items `stats_corr` may import, by exact name (decisions 339 and
/// 343): only what it imports.
const CORR_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &[
            "mean",
            "pearson_clamped",
            "sample_variance",
            "spearman",
            "CorrelationResult",
            "RangeFault",
        ],
    ),
    (
        "crate::stats_dist",
        &["normal_quantile_975", "normal_two_tailed", "t_two_tailed"],
    ),
    ("crate::stats_ttest", &["column", "non_finite_cells"]),
    ("crate::stats_l1::compute", &["RHO_ONE_ULPS"]),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "preparation_holds",
            "score_code_count",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_alpha` may name inside the crate (Phase 8I, decision 372);
/// Level 1's compute only for its one constant (decision 352's test). The
/// export's path comes before the descriptive summary's, so it is matched
/// first (8I-2, decision 378).
const STATS_ALPHA_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_dist",
    "crate::stats_l1::compute",
    "crate::stats_l1::dataset",
    "crate::stats_ttest",
    "crate::stats_verify",
];
/// The items `stats_alpha` may import, by exact name (decision 368): only
/// what it imports.
const ALPHA_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &["mean", "pearson_clamped", "sample_variance", "RangeFault"],
    ),
    ("crate::stats_dist", &["f_quantile_upper"]),
    ("crate::stats_ttest", &["column", "non_finite_cells"]),
    ("crate::stats_l1::compute", &["RHO_ONE_ULPS"]),
    ("crate::stats_desc::export", &["text"]),
    (
        "crate::stats_desc",
        &[
            "input_ref",
            "preparation_holds",
            "score_code_count",
            "suspect_codes_unless_score",
            "Alias",
        ],
    ),
];

/// What `stats_prep` may name inside the crate (Phase 8M, decision 408).
const STATS_PREP_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::research_project",
    "crate::stats_alpha",
    "crate::stats_desc::export",
    "crate::stats_desc",
    "crate::stats_l1::dataset",
    "crate::stats_ttest",
    "crate::stats_verify",
];
/// The items `stats_prep` may import, by exact name (decision 408): only
/// what it imports.
const PREP_ITEMS: &[(&str, &[&str])] = &[
    ("crate::research_project", &["is_invisible_format"]),
    (
        "crate::stats_alpha",
        &["reverse", "outside_scale_count", "row_total", "ScaleEnds"],
    ),
    ("crate::stats_desc::export", &["number", "text"]),
    ("crate::stats_desc", &["Alias"]),
    ("crate::stats_ttest", &["column", "non_finite_cells"]),
    ("crate::stats_verify", &["mean"]),
];

/// What `stats_dist` may name inside the crate (Phase 8E, decision 275):
/// Level 0's three tail functions only, the chi-square's added in Phase 8X-1
/// (decision 576).
const STATS_DIST_CRATE_PATHS: &[&str] = &["crate::stats_l0::pvalue"];
/// The items `stats_dist` may import, by exact text: the alias text
/// `imported_names` returns (as `REGISTRY_ITEMS`'s `CAPABILITY_ID as
/// TTEST_ID`), because its own functions have the same names.
const DIST_ITEMS: &[(&str, &[&str])] = &[(
    "crate::stats_l0::pvalue",
    &[
        "t_two_tailed as l0_t_two_tailed",
        "f_upper as l0_f_upper",
        "chi_square_upper as l0_chi_square_upper",
    ],
)];
/// `stats_dist`'s roots from outside the crate: `std`, and `statrs`, whose
/// `erfc`, `Normal` and `ln_binomial` it is built on. Passed to the import scan for that
/// directory alone: on `EXTERNAL_ROOTS`, statrs would be open to every
/// scanned module.
const DIST_EXTERNAL_ROOTS: &[&str] = &["std", "statrs"];

/// The modules `contracts` must never name.
const DOMAIN_MODULES: &[&str] = &[
    "stats_l0",
    "stats_l1",
    "stats_assist",
    "stats_desc",
    "stats_verify",
    "stats_verdict",
    "stats_chat",
    "stats_registry",
    "stats_adapters",
    "stats_ttest",
    "stats_onesample",
    "stats_dist",
    "stats_anova",
    "stats_corr",
    "stats_alpha",
    "stats_prep",
    "stats_xtab",
    "stats_ranks",
    "stats_related_ranks",
    "research_project",
];
/// Where their types are declared, read to build the type list.
const DOMAIN_SOURCES: &[&str] = &[
    "src/stats_l0",
    "src/stats_l1",
    "src/stats_assist",
    "src/stats_desc",
    "src/stats_verify.rs",
    "src/stats_registry",
    "src/stats_adapters",
    "src/stats_ttest",
    "src/stats_onesample",
    "src/stats_dist",
    "src/stats_anova",
    "src/stats_corr",
    "src/stats_alpha",
    "src/stats_prep",
    "src/stats_xtab",
    "src/stats_ranks",
    "src/stats_related_ranks",
    "src/research_project",
];

/// `stats_desc` types that hold a column header or a level label. They may
/// derive `Serialize` because they ARE the user's result, for the user's own
/// screen and the export the user saves.
const HOLDS_HEADERS_OR_LABELS: &[&str] = &[
    "DatasetProfile",
    "ColumnProfile",
    "LevelCount",
    "DescribeResult",
    "ColumnResult",
    "NumericSummary",
    "CategoricalSummary",
];
/// `stats_desc` types that derive `Serialize` and hold no text at all.
const HOLDS_NO_TEXT: &[&str] = &[
    "DescribeParams",
    "ColumnRequest",
    "TreatAs",
    "ColumnKind",
    "SuspectCode",
    "Stat",
];
/// `stats_ttest` types that hold a level label (Phase 4B-1). They may derive
/// `Serialize` because they ARE the user's result.
const TTEST_HOLDS_LABELS: &[&str] = &["GroupSummary", "TTestResult"];
/// `stats_ttest` types that derive `Serialize` and hold no text at all.
const TTEST_HOLDS_NO_TEXT: &[&str] = &["TTestParams", "Variant", "Standardizer", "Stat"];
/// `stats_onesample` types that derive `Serialize` (Phase 8D-1). None holds
/// text: the module holds no label, header or cell.
const ONESAMPLE_SERIALISED: &[&str] = &[
    "OneSampleParams",
    "PairedParams",
    "OneSampleResult",
    "ColumnStats",
    "PairedResult",
];

/// Variants of contracts' own enums whose names a domain module also gives
/// a type (Phase 8G-2): `SemanticRole::Table` and `stats_anova::Table`. Each
/// is exempt only on the line that declares it, the trimmed line `Table,`;
/// any other use of the word in contracts is still a finding, and the
/// exemption must be used exactly once, so it cannot outlive the variant.
const CONTRACTS_VARIANTS_NAMED_LIKE_DOMAIN_TYPES: &[&str] = &["Table"];

/// `stats_anova` types that hold a level label (Phase 8G-2, decision 326):
/// a group's row, and the result that holds the rows. They may derive
/// `Serialize` because they ARE the user's result.
const ANOVA_HOLDS_LABELS: &[&str] = &["GroupRow", "AnovaOutcome"];
/// `stats_anova` types that derive `Serialize` and hold no text at all.
const ANOVA_HOLDS_NO_TEXT: &[&str] = &[
    "PostHoc",
    "AnovaParams",
    "Stat",
    "PostHocP",
    "AnovaTable",
    "Table",
    "FTest",
    "FOutcome",
    "PairStats",
    "Pair",
    "PairOutcome",
    "PostHocTable",
    "PostHocPairs",
];

/// `stats_xtab` types that hold a level label (Phase 8X-1, decision 577): a
/// level's label, and the result that holds them. They may derive
/// `Serialize` because they ARE the user's result.
const XTAB_HOLDS_LABELS: &[&str] = &["LevelLabel", "CrosstabsOutcome"];
/// `stats_xtab` types that derive `Serialize` and hold no text at all.
const XTAB_HOLDS_NO_TEXT: &[&str] = &[
    "CrosstabsParams",
    "Stat",
    "Frequency",
    "ChiTest",
    "ChiOutcome",
    "FisherTest",
];

/// `stats_ranks` types that hold a level label (Phase 8Y-1, decision 604): a
/// group, and the result that holds them. They may derive `Serialize`
/// because they ARE the user's result.
const RANKS_HOLDS_LABELS: &[&str] = &["RankGroup", "IndependentRanksOutcome"];
/// `stats_ranks` types that derive `Serialize` and hold no text at all.
const RANKS_HOLDS_NO_TEXT: &[&str] = &[
    "IndependentRanksParams",
    "Stat",
    "MannWhitney",
    "KruskalWallis",
    "RankTest",
];

/// `stats_corr` types that derive `Serialize` (Phase 8H, decision 343).
/// None holds text: the module holds no header, label or cell.
const CORR_SERIALISED: &[&str] = &[
    "Coefficient",
    "MissingHandling",
    "CorrParams",
    "Stat",
    "Bounded",
    "TStat",
    "TestStatistic",
    "Interval",
    "Descriptive",
    "PairStats",
    "PairOutcome",
    "Pair",
    "CoefficientTable",
    "CorrOutcome",
];

/// `stats_related_ranks` types that derive `Serialize` (Phase 8Z-1, decision
/// 638). None holds text: the module holds no header, label or cell.
const RELATED_RANKS_SERIALISED: &[&str] = &[
    "RelatedRanksParams",
    "Stat",
    "RelatedColumn",
    "SignRanks",
    "Wilcoxon",
    "FriedmanColumn",
    "Friedman",
    "RelatedTest",
    "RelatedRanksOutcome",
];

/// `stats_alpha` types that derive `Serialize` (Phase 8I, decision 372).
/// None holds text: the module holds no header, label or cell.
const ALPHA_SERIALISED: &[&str] = &[
    "ScaleEnds",
    "AlphaParams",
    "Stat",
    "Interval",
    "InterItem",
    "ItemStats",
    "AlphaOutcome",
];

/// `stats_prep`'s one type that holds text: the name the user typed (ADR-011
/// Part 4, decision 408), which `Serialize`s only into the recipe.
const PREP_HOLDS_TEXT: &[&str] = &["ScaleScoreStep"];
/// `stats_prep` types that derive `Serialize` and hold no text at all.
const PREP_HOLDS_NO_TEXT: &[&str] = &[
    "Statistic",
    "MissingRule",
    "RecipeStep",
    "Recipe",
    "ScoreCounts",
];

/// What a serialisable type must never hold.
const CELL_TYPES: &[&str] = &[
    "Cell",
    "Dataset",
    "RawTable",
    "Tally",
    "Split",
    "Ready",
    "Values",
    "Pairs",
    "Rows",
    "Entry",
    "SchemaSummary",
    "ColumnSummary",
];

/// The only `String`-holding types in `contracts`, each checked on the way
/// in. Sorted, as the scan's list is.
const CONTRACTS_STRING_TYPES: &[&str] = &["CapabilityId", "ConfirmationId", "ContentHash"];

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    if dir.is_file() {
        return vec![dir.to_path_buf()];
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has it moved?", dir.display());
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every file of a directory, comments removed, with its path.
fn sources(dir: &str, at_least: usize) -> Vec<(String, String)> {
    let files = rust_files(Path::new(dir));
    assert!(
        files.len() >= at_least,
        "{dir}: {} files — the scan would pass vacuously",
        files.len()
    );
    files
        .iter()
        .map(|f| {
            (
                f.display().to_string(),
                code_only(&std::fs::read_to_string(f).expect("read")),
            )
        })
        .collect()
}

fn forbidden_in(path: &str, code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (needle, what) in FORBIDDEN {
        for (n, line) in code.lines().enumerate() {
            if line.contains(needle) {
                out.push(format!(
                    "{path}:{}: `{needle}` — {what}\n    {}",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    out
}

/// Every `use` statement, joined onto one line when rustfmt wrapped it.
fn use_statements(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<String> = None;
    for line in code.lines() {
        let t = line.trim();
        if let Some(acc) = open.as_mut() {
            acc.push(' ');
            acc.push_str(t);
        } else if let Some(rest) = t
            .strip_prefix("pub(crate) use ")
            .or_else(|| t.strip_prefix("pub use "))
            .or_else(|| t.strip_prefix("use "))
        {
            open = Some(rest.to_string());
        }
        if open.as_ref().is_some_and(|acc| acc.ends_with(';')) {
            out.push(
                open.take()
                    .expect("open")
                    .trim_end_matches(';')
                    .trim()
                    .to_string(),
            );
        }
    }
    out
}

/// The imported names of `use <prefix>::{a, b}` or `use <prefix>::a`.
fn imported_names(statement: &str, prefix: &str) -> Vec<String> {
    let rest = statement
        .strip_prefix(prefix)
        .unwrap_or("")
        .trim_start_matches("::");
    rest.trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `roots`: the crates allowed from outside this one, `EXTERNAL_ROOTS` for
/// every directory but `stats_dist`, which passes `DIST_EXTERNAL_ROOTS`.
fn import_violations(
    path: &str,
    code: &str,
    crate_paths: &[&str],
    items: &[(&str, &[&str])],
    roots: &[&str],
) -> Vec<String> {
    let mut out = Vec::new();
    for statement in use_statements(code) {
        if !statement.contains("::") {
            continue; // a local name re-exported, such as `pub(crate) use closed;`
        }
        let root = statement
            .split(['{', ':', ';', ' '])
            .next()
            .unwrap_or("")
            .trim();
        match root {
            "self" | "super" => {}
            "crate" => {
                let Some(allowed) = crate_paths.iter().find(|p| statement.starts_with(*p)) else {
                    out.push(format!(
                        "{path}: `use {statement}` — not an allowed crate path"
                    ));
                    continue;
                };
                if let Some((_, names)) = items.iter().find(|(p, _)| p == allowed) {
                    for name in imported_names(&statement, allowed) {
                        if !names.contains(&name.as_str()) {
                            out.push(format!("{path}: `use {statement}` — `{name}` is not on the list for {allowed}"));
                        }
                    }
                }
            }
            r if roots.contains(&r) => {}
            other => out.push(format!(
                "{path}: `use {statement}` — `{other}` is not on the import allowlist"
            )),
        }
    }
    out
}

/// Every `crate::` path in the code, `use` lines or not, must start with an
/// allowed path.
fn crate_path_violations(path: &str, code: &str, crate_paths: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let mut rest = line;
        while let Some(i) = rest.find("crate::") {
            let tail = &rest[i..];
            if !crate_paths.iter().any(|p| tail.starts_with(p)) {
                out.push(format!(
                    "{path}:{}: `{}` is not an allowed crate path",
                    n + 1,
                    tail.split([' ', ';', ',', '(']).next().unwrap_or(tail)
                ));
            }
            rest = &rest[i + "crate::".len()..];
        }
    }
    out
}

fn identifiers(code: &str) -> Vec<&str> {
    code.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect()
}

/// A `struct` or `enum` declaration with the attribute lines above it.
#[derive(Debug)]
struct Item {
    name: String,
    attrs: String,
    body: String,
}

fn items(code: &str) -> Vec<Item> {
    let lines: Vec<&str> = code.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        let Some(rest) = [
            "pub struct ",
            "pub enum ",
            "pub(crate) struct ",
            "pub(crate) enum ",
            "struct ",
            "enum ",
        ]
        .iter()
        .find_map(|p| t.strip_prefix(p)) else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let mut attrs = Vec::new();
        for above in lines[..i].iter().rev() {
            let a = above.trim();
            if a.starts_with("#[") || a.is_empty() {
                attrs.push(a);
            } else {
                break;
            }
        }
        let mut body = String::new();
        let mut depth = 0i32;
        for line in &lines[i..] {
            body.push_str(line);
            body.push('\n');
            depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            if depth <= 0 && (line.contains('}') || line.trim_end().ends_with(';')) {
                break;
            }
        }
        out.push(Item {
            name,
            attrs: attrs.join("\n"),
            body,
        });
    }
    out
}

fn derives(item: &Item, what: &str) -> bool {
    item.attrs
        .lines()
        .any(|a| a.starts_with("#[derive(") && a.contains(what))
}

/// Field-type positions of `String`: `: String`, `<String`, `(String`.
fn holds_string(body: &str) -> bool {
    [": String", "<String", "(String"]
        .iter()
        .any(|p| body.contains(p))
}

#[test]
fn contracts_and_stats_desc_reach_no_file_network_process_clock_model_or_log() {
    let mut found = Vec::new();
    for (path, code) in sources(CONTRACTS, 3)
        .iter()
        .chain(sources(STATS_DESC, 2).iter())
        .chain(sources(STATS_REGISTRY, 1).iter())
        .chain(sources(STATS_ADAPTERS, 2).iter())
        .chain(sources(STATS_TTEST, 2).iter())
        .chain(sources(STATS_ONESAMPLE, 2).iter())
        .chain(sources(STATS_DIST, 1).iter())
        .chain(sources(STATS_ANOVA, 2).iter())
        .chain(sources(STATS_CORR, 2).iter())
        .chain(sources(STATS_ALPHA, 2).iter())
        .chain(sources(STATS_PREP, 2).iter())
        .chain(sources(STATS_XTAB, 2).iter())
        .chain(sources(STATS_RANKS, 2).iter())
        .chain(sources(STATS_RELATED_RANKS, 2).iter())
        .chain(sources(RESEARCH_PROJECT, 2).iter())
    {
        found.extend(forbidden_in(path, code));
    }
    assert!(
        found.is_empty(),
        "contracts, stats_desc, stats_registry, stats_adapters, stats_ttest, stats_onesample, stats_dist, stats_anova, stats_corr, stats_alpha, stats_prep, stats_xtab, stats_ranks and research_project are pure. Found:\n{}",
        found.join("\n")
    );
}

#[test]
fn imports_are_std_serde_serde_json_sha2_and_the_named_crate_paths_only() {
    let mut found = Vec::new();
    let mut statements = 0;
    for (dir, crate_paths, verifier, inline, roots) in [
        (
            CONTRACTS,
            CONTRACTS_CRATE_PATHS,
            &[][..],
            CONTRACTS_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_DESC,
            STATS_DESC_CRATE_PATHS,
            VERIFIER_ITEMS,
            STATS_DESC_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_REGISTRY,
            STATS_REGISTRY_CRATE_PATHS,
            REGISTRY_ITEMS,
            STATS_REGISTRY_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_ADAPTERS,
            STATS_ADAPTERS_CRATE_PATHS,
            ADAPTER_ITEMS,
            STATS_ADAPTERS_INLINE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_TTEST,
            STATS_TTEST_CRATE_PATHS,
            TTEST_ITEMS,
            STATS_TTEST_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_ONESAMPLE,
            STATS_ONESAMPLE_CRATE_PATHS,
            ONESAMPLE_ITEMS,
            STATS_ONESAMPLE_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_DIST,
            STATS_DIST_CRATE_PATHS,
            DIST_ITEMS,
            STATS_DIST_CRATE_PATHS,
            DIST_EXTERNAL_ROOTS,
        ),
        (
            STATS_ANOVA,
            STATS_ANOVA_CRATE_PATHS,
            ANOVA_ITEMS,
            STATS_ANOVA_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_XTAB,
            STATS_XTAB_CRATE_PATHS,
            XTAB_ITEMS,
            STATS_XTAB_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_RANKS,
            STATS_RANKS_CRATE_PATHS,
            RANKS_ITEMS,
            STATS_RANKS_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_RELATED_RANKS,
            STATS_RELATED_RANKS_CRATE_PATHS,
            RELATED_RANKS_ITEMS,
            STATS_RELATED_RANKS_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_CORR,
            STATS_CORR_CRATE_PATHS,
            CORR_ITEMS,
            STATS_CORR_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_ALPHA,
            STATS_ALPHA_CRATE_PATHS,
            ALPHA_ITEMS,
            STATS_ALPHA_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            STATS_PREP,
            STATS_PREP_CRATE_PATHS,
            PREP_ITEMS,
            STATS_PREP_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
        (
            RESEARCH_PROJECT,
            RESEARCH_PROJECT_CRATE_PATHS,
            &[][..],
            RESEARCH_PROJECT_CRATE_PATHS,
            EXTERNAL_ROOTS,
        ),
    ] {
        for (path, code) in sources(dir, 1) {
            statements += use_statements(&code).len();
            found.extend(import_violations(
                &path,
                &code,
                crate_paths,
                verifier,
                roots,
            ));
            found.extend(crate_path_violations(&path, &code, inline));
        }
    }
    assert!(
        statements >= 8,
        "{statements} `use` statements found — the import scan is inert"
    );
    assert!(found.is_empty(), "{}", found.join("\n"));

    // stats_desc must still import each named verifier item, or the list
    // guards nothing.
    let desc: String = sources(STATS_DESC, 2).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in VERIFIER_ITEMS {
        let imported: Vec<String> = use_statements(&desc)
            .iter()
            .filter(|s| s.starts_with(prefix))
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_desc no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // So must stats_registry each item on its list.
    let reg: String = sources(STATS_REGISTRY, 1)
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    for (prefix, names) in REGISTRY_ITEMS {
        let imported: Vec<String> = use_statements(&reg)
            .iter()
            .filter(|s| s.starts_with(prefix))
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_registry no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_ttest each item on its list (decision 62).
    let ttest: String = sources(STATS_TTEST, 2)
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    for (prefix, names) in TTEST_ITEMS {
        let imported: Vec<String> = use_statements(&ttest)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_TTEST_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_ttest no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_onesample each item on its list (decision 254).
    let onesample: String = sources(STATS_ONESAMPLE, 2)
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    for (prefix, names) in ONESAMPLE_ITEMS {
        let imported: Vec<String> = use_statements(&onesample)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_ONESAMPLE_CRATE_PATHS
                        .iter()
                        .find(|p| s.starts_with(*p))
                        == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_onesample no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_dist each item on its list (decision 275), by its alias text.
    let dist: String = sources(STATS_DIST, 1).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in DIST_ITEMS {
        let imported: Vec<String> = use_statements(&dist)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_DIST_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_dist no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_anova each item on its list (decision 326).
    let anova: String = sources(STATS_ANOVA, 2)
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    for (prefix, names) in ANOVA_ITEMS {
        let imported: Vec<String> = use_statements(&anova)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_ANOVA_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_anova no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_xtab each item on its list (decision 577).
    let xtab: String = sources(STATS_XTAB, 2).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in XTAB_ITEMS {
        let imported: Vec<String> = use_statements(&xtab)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_XTAB_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_xtab no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_ranks each item on its list (decision 604).
    let ranks: String = sources(STATS_RANKS, 2).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in RANKS_ITEMS {
        let imported: Vec<String> = use_statements(&ranks)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_RANKS_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_ranks no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_related_ranks each item on its list (decision 638).
    let related: String = sources(STATS_RELATED_RANKS, 2).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in RELATED_RANKS_ITEMS {
        let imported: Vec<String> = use_statements(&related)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_RELATED_RANKS_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_related_ranks no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_corr each item on its list (decision 343).
    let corr: String = sources(STATS_CORR, 2).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in CORR_ITEMS {
        let imported: Vec<String> = use_statements(&corr)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_CORR_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_corr no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_alpha each item on its list (decision 372).
    let alpha: String = sources(STATS_ALPHA, 2)
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    for (prefix, names) in ALPHA_ITEMS {
        let imported: Vec<String> = use_statements(&alpha)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_ALPHA_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_alpha no longer imports {prefix}::{name}; update the list"
            );
        }
    }
    // And stats_prep each item on its list (decision 408).
    let prep: String = sources(STATS_PREP, 2).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in PREP_ITEMS {
        let imported: Vec<String> = use_statements(&prep)
            .iter()
            .filter(|s| {
                s.starts_with(prefix)
                    && STATS_PREP_CRATE_PATHS.iter().find(|p| s.starts_with(*p)) == Some(prefix)
            })
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_prep no longer imports {prefix}::{name}; update the list"
            );
        }
    }
}

#[test]
fn contracts_name_no_verifier_or_capability_module_or_type() {
    let contracts = sources(CONTRACTS, 2);
    let own: Vec<String> = contracts
        .iter()
        .flat_map(|(_, c)| items(c))
        .map(|i| i.name)
        .collect();
    assert!(
        own.len() >= 20,
        "{} types found in contracts — the item scan is inert",
        own.len()
    );

    let mut domain_types = Vec::new();
    for src in DOMAIN_SOURCES {
        for (_, code) in sources(src, 1) {
            for line in code.lines() {
                let t = line.trim();
                for p in [
                    "pub struct ",
                    "pub enum ",
                    "pub trait ",
                    "pub type ",
                    "pub(crate) struct ",
                    "pub(crate) enum ",
                ] {
                    if let Some(rest) = t.strip_prefix(p) {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            domain_types.push(name);
                        }
                    }
                }
            }
        }
    }
    domain_types.sort();
    domain_types.dedup();
    assert!(
        domain_types.len() >= 100,
        "{} domain types found — the type list is inert",
        domain_types.len()
    );

    let mut found = Vec::new();
    let mut variant_declarations = 0;
    for (path, code) in &contracts {
        for line in code.lines() {
            for word in identifiers(line) {
                if DOMAIN_MODULES.contains(&word) {
                    found.push(format!("{path}: names the module `{word}`"));
                }
                if domain_types.iter().any(|t| t == word) && !own.iter().any(|o| o == word) {
                    if CONTRACTS_VARIANTS_NAMED_LIKE_DOMAIN_TYPES.contains(&word)
                        && line.trim() == format!("{word},")
                    {
                        variant_declarations += 1;
                        continue;
                    }
                    found.push(format!("{path}: names the domain type `{word}`"));
                }
            }
        }
    }
    assert_eq!(
        variant_declarations,
        CONTRACTS_VARIANTS_NAMED_LIKE_DOMAIN_TYPES.len(),
        "each exempt variant is declared exactly once in contracts; update the list"
    );
    let shared: Vec<&String> = domain_types.iter().filter(|t| own.contains(t)).collect();
    println!("names declared both in contracts and in a domain module (contracts' own, so exempt): {shared:?}");
    assert!(
        found.is_empty(),
        "contracts are domain-free:\n{}",
        found.join("\n")
    );
}

#[test]
fn only_the_listed_stats_desc_types_derive_serialize_and_none_holds_a_cell() {
    let desc: Vec<Item> = sources(STATS_DESC, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = desc
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = HOLDS_HEADERS_OR_LABELS
        .iter()
        .chain(HOLDS_NO_TEXT)
        .copied()
        .collect();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_DESC} are not the listed ones"
    );

    for item in desc.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
        if HOLDS_NO_TEXT.contains(&item.name.as_str()) {
            assert!(
                !holds_string(&item.body),
                "{} is listed as holding no text but has a String field",
                item.name
            );
            for text in HOLDS_HEADERS_OR_LABELS {
                assert!(
                    !identifiers(&item.body).contains(text),
                    "{} is listed as holding no text but holds a {text}",
                    item.name
                );
            }
        }
    }
    let tally = desc
        .iter()
        .find(|i| i.name == "Tally")
        .expect("the tally exists");
    assert!(
        !tally.attrs.contains("#[derive("),
        "the tally holds the column's values and derives nothing"
    );

    let contracts: Vec<Item> = sources(CONTRACTS, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut with_strings: Vec<&str> = contracts
        .iter()
        .filter(|i| holds_string(&i.body))
        .map(|i| i.name.as_str())
        .collect();
    with_strings.sort_unstable();
    assert_eq!(
        with_strings, CONTRACTS_STRING_TYPES,
        "only the checked newtypes in contracts may hold a String"
    );
}

#[test]
fn a_stats_desc_type_holding_text_prints_counts_through_its_own_debug() {
    let files = sources(STATS_DESC, 2);
    let all: String = files.iter().map(|(_, c)| c.as_str()).collect();
    let texty: Vec<Item> = files
        .iter()
        .flat_map(|(_, c)| items(c))
        .filter(|i| holds_string(&i.body))
        .collect();
    assert!(
        texty.len() >= 5,
        "{} text-holding types found — the scan is inert",
        texty.len()
    );
    for item in &texty {
        assert!(
            !derives(item, "Debug"),
            "{} holds a header or label and derives Debug",
            item.name
        );
        let printable = all.contains(&format!("impl std::fmt::Debug for {}", item.name));
        if derives(item, "Serialize") || printable {
            assert!(
                HOLDS_HEADERS_OR_LABELS.contains(&item.name.as_str()),
                "{} holds a String and is not listed",
                item.name
            );
            assert!(printable, "{} has no counts-only Debug", item.name);
        } else {
            // The tally: neither printed nor serialised, so nothing to list.
            assert!(
                item.attrs.is_empty() || !item.attrs.contains("#[derive("),
                "{} derives something",
                item.name
            );
        }
    }
}

/// Phase 4B-1: in `stats_ttest`, exactly the listed types derive
/// `Serialize`, none names a cell, the dataset or the split, and `Split`
/// derives nothing.
#[test]
fn only_the_listed_stats_ttest_types_derive_serialize_and_none_holds_a_cell() {
    let items: Vec<Item> = sources(STATS_TTEST, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = TTEST_HOLDS_LABELS
        .iter()
        .chain(TTEST_HOLDS_NO_TEXT)
        .copied()
        .collect();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_TTEST} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
        if TTEST_HOLDS_NO_TEXT.contains(&item.name.as_str()) {
            assert!(
                !holds_string(&item.body),
                "{} is listed as holding no text but has a String field",
                item.name
            );
            for text in TTEST_HOLDS_LABELS {
                assert!(
                    !identifiers(&item.body).contains(text),
                    "{} is listed as holding no text but holds a {text}",
                    item.name
                );
            }
        }
    }
    let split = items
        .iter()
        .find(|i| i.name == "Split")
        .expect("the split exists");
    assert!(
        !split.attrs.contains("#[derive("),
        "the split holds the outcome's values and derives nothing"
    );
}

/// Phase 4B-1: a `stats_ttest` type with a `String` field does not derive
/// `Debug`, prints counts through its own, and is listed.
#[test]
fn a_stats_ttest_type_holding_text_prints_counts_through_its_own_debug() {
    let files = sources(STATS_TTEST, 2);
    let all: String = files.iter().map(|(_, c)| c.as_str()).collect();
    let texty: Vec<Item> = files
        .iter()
        .flat_map(|(_, c)| items(c))
        .filter(|i| holds_string(&i.body))
        .collect();
    assert!(
        !texty.is_empty(),
        "no text-holding type found in {STATS_TTEST} — the scan is inert"
    );
    for item in &texty {
        assert!(
            !derives(item, "Debug"),
            "{} holds a label and derives Debug",
            item.name
        );
        let printable = all.contains(&format!("impl std::fmt::Debug for {}", item.name));
        if derives(item, "Serialize") || printable {
            assert!(
                TTEST_HOLDS_LABELS.contains(&item.name.as_str()),
                "{} holds a String and is not listed",
                item.name
            );
            assert!(printable, "{} has no counts-only Debug", item.name);
        } else {
            // `Ready`, the checked columns and levels: neither printed nor
            // serialised, so it must derive nothing.
            assert!(
                !item.attrs.contains("#[derive("),
                "{} holds a String and derives something",
                item.name
            );
        }
    }
}

/// Phase 8G-2 (decision 326): in `stats_anova`, exactly the listed types
/// derive `Serialize`, none names a cell, the dataset or the split, a type
/// listed as holding no text holds none, and `Split` and `Ready` derive
/// nothing.
#[test]
fn only_the_listed_stats_anova_types_derive_serialize_and_none_holds_a_cell() {
    let items: Vec<Item> = sources(STATS_ANOVA, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = ANOVA_HOLDS_LABELS
        .iter()
        .chain(ANOVA_HOLDS_NO_TEXT)
        .copied()
        .collect();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_ANOVA} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
        if ANOVA_HOLDS_NO_TEXT.contains(&item.name.as_str()) {
            assert!(
                !holds_string(&item.body),
                "{} is listed as holding no text but has a String field",
                item.name
            );
            for text in ANOVA_HOLDS_LABELS {
                assert!(
                    !identifiers(&item.body).contains(text),
                    "{} is listed as holding no text but holds a {text}",
                    item.name
                );
            }
        }
    }
    for reading in ["Split", "Ready"] {
        let item = items
            .iter()
            .find(|i| i.name == reading)
            .unwrap_or_else(|| panic!("{reading} exists"));
        assert!(
            !item.attrs.contains("#[derive("),
            "{reading} holds the values or the labels and derives nothing"
        );
    }
}

/// Phase 8G-2 (decision 326): a `stats_anova` type with a `String` field
/// does not derive `Debug`; one that is serialised or printed is listed and
/// prints counts through its own `Debug`; any other derives nothing.
#[test]
fn a_stats_anova_type_holding_text_prints_counts_through_its_own_debug() {
    let files = sources(STATS_ANOVA, 2);
    let all: String = files.iter().map(|(_, c)| c.as_str()).collect();
    let texty: Vec<Item> = files
        .iter()
        .flat_map(|(_, c)| items(c))
        .filter(|i| holds_string(&i.body))
        .collect();
    assert!(
        texty.len() >= 2,
        "{} text-holding types found in {STATS_ANOVA} — the scan is inert",
        texty.len()
    );
    for item in &texty {
        assert!(
            !derives(item, "Debug"),
            "{} holds a label and derives Debug",
            item.name
        );
        let printable = all.contains(&format!("impl std::fmt::Debug for {}", item.name));
        if derives(item, "Serialize") || printable {
            assert!(
                ANOVA_HOLDS_LABELS.contains(&item.name.as_str()),
                "{} holds a String and is not listed",
                item.name
            );
            assert!(printable, "{} has no counts-only Debug", item.name);
        } else {
            assert!(
                !item.attrs.contains("#[derive("),
                "{} holds a String and derives something",
                item.name
            );
        }
    }
}

/// Phase 8D-1: in `stats_onesample`, exactly the listed types derive
/// `Serialize`, none names a cell, the dataset or a reading, and `Values` and
/// `Pairs`, which hold the values, derive nothing.
#[test]
fn only_the_listed_stats_onesample_types_derive_serialize_and_none_holds_a_cell() {
    let items: Vec<Item> = sources(STATS_ONESAMPLE, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = ONESAMPLE_SERIALISED.to_vec();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_ONESAMPLE} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
    }
    for reading in ["Values", "Pairs"] {
        let item = items
            .iter()
            .find(|i| i.name == reading)
            .unwrap_or_else(|| panic!("{reading} exists"));
        assert!(
            !item.attrs.contains("#[derive("),
            "{reading} holds the values and derives nothing"
        );
    }
}

/// Phase 8D-1: no `stats_onesample` type holds a `String`. The module holds no
/// header, label or cell text, so its results are numbers, closed words and
/// counts only.
#[test]
fn no_stats_onesample_type_holds_text() {
    let items: Vec<Item> = sources(STATS_ONESAMPLE, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    assert!(
        items.len() >= 8,
        "{} items found in {STATS_ONESAMPLE} — the scan is inert",
        items.len()
    );
    let texty: Vec<&str> = items
        .iter()
        .filter(|i| holds_string(&i.body))
        .map(|i| i.name.as_str())
        .collect();
    assert!(texty.is_empty(), "these hold a String: {texty:?}");
}

/// Phase 8H (decision 343): in `stats_corr`, exactly the listed types derive
/// `Serialize`, none names a cell, the dataset or the rows, and `Rows` and
/// `Entry`, which hold the values, derive nothing.
#[test]
fn only_the_listed_stats_corr_types_derive_serialize_and_none_holds_a_cell() {
    let items: Vec<Item> = sources(STATS_CORR, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = CORR_SERIALISED.to_vec();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_CORR} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
    }
    for reading in ["Rows", "Entry"] {
        let item = items
            .iter()
            .find(|i| i.name == reading)
            .unwrap_or_else(|| panic!("{reading} exists"));
        assert!(
            !item.attrs.contains("#[derive("),
            "{reading} holds the values and derives nothing"
        );
    }
}

/// Phase 8H (decision 343): no `stats_corr` type holds a `String`. Its
/// results are numbers, aliases and closed words, so its `Debug` can print
/// nothing of the user's data.
#[test]
fn no_stats_corr_type_holds_text() {
    let items: Vec<Item> = sources(STATS_CORR, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    assert!(
        items.len() >= 12,
        "{} items found in {STATS_CORR} — the scan is inert",
        items.len()
    );
    let texty: Vec<&str> = items
        .iter()
        .filter(|i| holds_string(&i.body))
        .map(|i| i.name.as_str())
        .collect();
    assert!(texty.is_empty(), "these hold a String: {texty:?}");
}

/// Phase 8Z-1 (decision 638): in `stats_related_ranks`, exactly the listed
/// types derive `Serialize`, none names a cell or the dataset, and `Rows`,
/// which holds the values, derives nothing.
#[test]
fn only_the_listed_stats_related_ranks_types_derive_serialize_and_none_holds_a_cell() {
    let items: Vec<Item> = sources(STATS_RELATED_RANKS, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    assert!(
        items.len() >= 10,
        "{} items found in {STATS_RELATED_RANKS} — the scan is inert",
        items.len()
    );
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = RELATED_RANKS_SERIALISED.to_vec();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_RELATED_RANKS} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
    }
    let rows = items.iter().find(|i| i.name == "Rows").expect("Rows exists");
    assert!(!rows.attrs.contains("#[derive("), "Rows holds the values and derives nothing");
}

/// Phase 8Z-1 (decision 638): no `stats_related_ranks` type holds a `String`.
/// Its results are numbers, aliases and closed words, so its `Debug` can print
/// nothing of the user's data.
#[test]
fn no_stats_related_ranks_type_holds_text() {
    let items: Vec<Item> = sources(STATS_RELATED_RANKS, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    assert!(
        items.len() >= 10,
        "{} items found in {STATS_RELATED_RANKS} — the scan is inert",
        items.len()
    );
    let texty: Vec<&str> = items
        .iter()
        .filter(|i| holds_string(&i.body))
        .map(|i| i.name.as_str())
        .collect();
    assert!(texty.is_empty(), "these hold a String: {texty:?}");
}

/// Phase 8I (decision 372): in `stats_alpha`, exactly the listed types
/// derive `Serialize`, none names a cell, the dataset or the rows, `Rows`
/// and `Entry`, which hold the values, derive nothing, and no type holds a
/// `String`.
#[test]
fn only_the_listed_stats_alpha_types_derive_serialize_and_none_holds_a_cell_or_text() {
    let items: Vec<Item> = sources(STATS_ALPHA, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    assert!(
        items.len() >= 12,
        "{} items found in {STATS_ALPHA} — the scan is inert",
        items.len()
    );
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = ALPHA_SERIALISED.to_vec();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_ALPHA} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
    }
    for reading in ["Rows", "Entry"] {
        let item = items
            .iter()
            .find(|i| i.name == reading)
            .unwrap_or_else(|| panic!("{reading} exists"));
        assert!(
            !item.attrs.contains("#[derive("),
            "{reading} holds the values and derives nothing"
        );
    }
    let texty: Vec<&str> = items
        .iter()
        .filter(|i| holds_string(&i.body))
        .map(|i| i.name.as_str())
        .collect();
    assert!(texty.is_empty(), "these hold a String: {texty:?}");
}

/// Phase 8M (decision 408): in `stats_prep`, exactly the listed types derive
/// `Serialize`, none names a cell or the dataset, and only `ScaleScoreStep`
/// holds a `String`, as `stats_desc`'s text-holding types do: it does not
/// derive `Debug` and prints counts through its own. `Prepared`, which holds
/// the dataset, derives nothing, and prints counts through its own too.
#[test]
fn only_the_listed_stats_prep_types_derive_serialize_and_only_the_step_holds_text() {
    let items: Vec<Item> = sources(STATS_PREP, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    assert!(
        items.len() >= 8,
        "{} items found in {STATS_PREP} — the scan is inert",
        items.len()
    );
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = PREP_HOLDS_TEXT
        .iter()
        .chain(PREP_HOLDS_NO_TEXT)
        .copied()
        .collect();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_PREP} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
    }
    let texty: Vec<&str> = items
        .iter()
        .filter(|i| holds_string(&i.body))
        .map(|i| i.name.as_str())
        .collect();
    assert_eq!(texty, PREP_HOLDS_TEXT, "these hold a String");

    let all: String = sources(STATS_PREP, 2).into_iter().map(|(_, c)| c).collect();
    for name in ["ScaleScoreStep", "Recipe", "Prepared"] {
        let item = items
            .iter()
            .find(|i| i.name == name)
            .unwrap_or_else(|| panic!("{name} exists"));
        assert!(
            !derives(item, "Debug"),
            "{name} would print its text or values through a derived Debug"
        );
        assert!(
            all.contains(&format!("impl std::fmt::Debug for {name}")),
            "{name} has no counts-only Debug"
        );
    }
    let prepared = items
        .iter()
        .find(|i| i.name == "Prepared")
        .expect("Prepared exists");
    assert!(
        !prepared.attrs.contains("#[derive("),
        "Prepared holds the dataset and derives nothing"
    );
}

#[test]
fn every_deserialised_struct_refuses_unknown_fields_and_wire_structs_are_camel_case() {
    let mut checked = 0;
    for dir in [
        CONTRACTS,
        STATS_DESC,
        STATS_REGISTRY,
        STATS_ADAPTERS,
        STATS_TTEST,
        STATS_ONESAMPLE,
        STATS_ANOVA,
        STATS_CORR,
        STATS_ALPHA,
        STATS_PREP,
        STATS_XTAB,
        STATS_RANKS,
        STATS_RELATED_RANKS,
        RESEARCH_PROJECT,
    ] {
        for (path, code) in sources(dir, 1) {
            // Every check here reads a derive from ONE line. A derive list
            // wrapped over several lines would be invisible to them, so it
            // fails here instead of passing there.
            for (n, line) in code.lines().enumerate() {
                let t = line.trim();
                assert!(
                    !t.starts_with("#[derive(") || t.ends_with(")]"),
                    "{path}:{}: a derive wrapped over lines; the guard reads one line",
                    n + 1
                );
            }
            for item in items(&code) {
                let is_struct = item.body.trim_start().starts_with("pub struct")
                    || item.body.trim_start().starts_with("struct");
                let named_fields = is_struct
                    && item
                        .body
                        .lines()
                        .next()
                        .is_some_and(|l| l.trim_end().ends_with('{'));
                if is_struct && derives(&item, "Deserialize") {
                    checked += 1;
                    assert!(
                        item.attrs.contains("deny_unknown_fields")
                            || item.attrs.contains("transparent"),
                        "{path}: {} derives Deserialize without deny_unknown_fields",
                        item.name
                    );
                }
                if named_fields && (derives(&item, "Serialize") || derives(&item, "Deserialize")) {
                    assert!(
                        item.attrs.contains("rename_all = \"camelCase\""),
                        "{path}: {} is on the wire without camelCase",
                        item.name
                    );
                }
            }
        }
    }
    assert!(
        checked >= 12,
        "{checked} deserialised structs found — the scan is inert"
    );
}

/// The predicates catch what they exist for, and a comment naming a route
/// does not trip them.
#[test]
fn the_guard_catches_each_kind_of_violation() {
    for offending in [
        "fn read(p: &str) -> String { std::fs::read_to_string(p).unwrap() }\n",
        "use std::net::TcpStream;\n",
        "fn now() -> u64 { std::time::SystemTime::now().elapsed().unwrap().as_secs() }\n",
        "fn show(x: f64) { println!(\"{x}\"); }\n",
        "fn log(x: f64) { tracing::info!(x); }\n",
        "use tauri::State;\n",
    ] {
        assert!(
            !forbidden_in("synthetic.rs", &code_only(offending)).is_empty(),
            "missed: {offending}"
        );
    }
    assert!(forbidden_in(
        "synthetic.rs",
        &code_only("//! never std::fs, never tracing\npub fn f() {}\n")
    )
    .is_empty());

    let bad_root = "use regex::Regex;\n";
    assert!(!import_violations(
        "s.rs",
        bad_root,
        STATS_DESC_CRATE_PATHS,
        VERIFIER_ITEMS,
        EXTERNAL_ROOTS
    )
    .is_empty());
    let wrapped = "use crate::stats_verify::{\n    mean,\n    t_test_welch,\n};\n";
    assert!(
        !import_violations(
            "s.rs",
            wrapped,
            STATS_DESC_CRATE_PATHS,
            VERIFIER_ITEMS,
            EXTERNAL_ROOTS
        )
        .is_empty(),
        "a wrapped use with an unlisted item"
    );
    let ok = "use crate::stats_verify::{mean, sample_variance, RangeFault};\nuse crate::stats_l1::dataset::Cell;\n";
    assert!(import_violations(
        "s.rs",
        ok,
        STATS_DESC_CRATE_PATHS,
        VERIFIER_ITEMS,
        EXTERNAL_ROOTS
    )
    .is_empty());
    let l0 = "use crate::stats_l0::pvalue::p_t;\n";
    assert!(!import_violations(
        "s.rs",
        l0,
        STATS_DESC_CRATE_PATHS,
        VERIFIER_ITEMS,
        EXTERNAL_ROOTS
    )
    .is_empty());
    let inline = "fn p() -> f64 { crate::stats_l0::pvalue::p_t(1.0, 2.0) }\n";
    assert!(
        !crate_path_violations("s.rs", inline, STATS_DESC_CRATE_PATHS).is_empty(),
        "an inline crate path"
    );

    // Phase 4A's lists refuse what they do not name.
    let reader = "use crate::extract::docparse::parse_path;\n";
    assert!(
        !import_violations(
            "a.rs",
            reader,
            STATS_ADAPTERS_CRATE_PATHS,
            ADAPTER_ITEMS,
            EXTERNAL_ROOTS
        )
        .is_empty(),
        "an extract item off the list"
    );
    let inline_reader = "fn x(p: &std::path::Path) { let _ = crate::extract::extract_path(p); }\n";
    assert!(
        !crate_path_violations("a.rs", inline_reader, STATS_ADAPTERS_INLINE_PATHS).is_empty(),
        "an inline path to extract's file reader"
    );
    let input = "use crate::extract::ExtractionResult;\n";
    assert!(import_violations(
        "a.rs",
        input,
        STATS_ADAPTERS_CRATE_PATHS,
        ADAPTER_ITEMS,
        EXTERNAL_ROOTS
    )
    .is_empty());
    let binding = "use crate::stats_l1::binding::confirm;\n";
    assert!(
        !import_violations(
            "r.rs",
            binding,
            STATS_REGISTRY_CRATE_PATHS,
            REGISTRY_ITEMS,
            EXTERNAL_ROOTS
        )
        .is_empty(),
        "the registry names the catalogue and Level 1's registry only"
    );
    let helper = "use crate::stats_assist::spec::HelperTask;\n";
    assert!(
        !forbidden_in("a.rs", &code_only(helper)).is_empty(),
        "the helper layer"
    );

    let leak = "#[derive(Debug, Clone, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct Leak {\n    pub rows: Vec<String>,\n}\n";
    let item = &items(leak)[0];
    assert!(
        item.name == "Leak"
            && derives(item, "Serialize")
            && derives(item, "Debug")
            && holds_string(&item.body)
    );
    let open = "#[derive(Deserialize)]\npub struct Open {\n    pub a: u32,\n}\n";
    assert!(!items(open)[0].attrs.contains("deny_unknown_fields"));
}

/// `stats_dist` takes the chi-square's upper tail from `stats_l0::pvalue`,
/// the one implementation (Phase 8X-1, decision 576), and never builds its
/// own: its code, comments stripped, names no `ChiSquared`.
#[test]
fn stats_dist_names_no_chi_squared_of_its_own() {
    let files = sources(STATS_DIST, 2);
    let mut seen = Vec::new();
    for (path, code) in &files {
        for (n, line) in code.lines().enumerate() {
            if line.contains("ChiSquared") {
                seen.push(format!("{path}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        seen.is_empty(),
        "stats_dist names ChiSquared; it wraps stats_l0::pvalue::chi_square_upper:\n{}",
        seen.join("\n")
    );
    let all: String = files.iter().map(|(_, c)| c.as_str()).collect();
    assert!(
        all.contains("fn chi_square_upper"),
        "the scan reads stats_dist's chi_square_upper, so it is not vacuous"
    );
}

/// Phase 8X-1 (decision 577): in `stats_xtab`, exactly the listed types
/// derive `Serialize`, none names a cell, the dataset or a reading, a type
/// listed as holding no text holds none, and `Split` and `Ready`, which hold
/// the counts and the labels, derive nothing.
#[test]
fn only_the_listed_stats_xtab_types_derive_serialize_and_none_holds_a_cell() {
    let items: Vec<Item> = sources(STATS_XTAB, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = XTAB_HOLDS_LABELS
        .iter()
        .chain(XTAB_HOLDS_NO_TEXT)
        .copied()
        .collect();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_XTAB} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
        if XTAB_HOLDS_NO_TEXT.contains(&item.name.as_str()) {
            assert!(
                !holds_string(&item.body),
                "{} is listed as holding no text but has a String field",
                item.name
            );
            for text in XTAB_HOLDS_LABELS {
                assert!(
                    !identifiers(&item.body).contains(text),
                    "{} is listed as holding no text but holds a {text}",
                    item.name
                );
            }
        }
    }
    for reading in ["Split", "Ready"] {
        let item = items
            .iter()
            .find(|i| i.name == reading)
            .unwrap_or_else(|| panic!("{reading} exists"));
        assert!(
            !item.attrs.contains("#[derive("),
            "{reading} holds the counts or the labels and derives nothing"
        );
    }
}

/// Phase 8X-1 (decision 577): a `stats_xtab` type with a `String` field does
/// not derive `Debug`; one that is serialised or printed is listed and prints
/// counts through its own `Debug`; any other derives nothing.
#[test]
fn a_stats_xtab_type_holding_text_prints_counts_through_its_own_debug() {
    let files = sources(STATS_XTAB, 2);
    let all: String = files.iter().map(|(_, c)| c.as_str()).collect();
    let texty: Vec<Item> = files
        .iter()
        .flat_map(|(_, c)| items(c))
        .filter(|i| holds_string(&i.body))
        .collect();
    assert!(
        texty.len() >= 2,
        "{} text-holding types found in {STATS_XTAB} — the scan is inert",
        texty.len()
    );
    for item in &texty {
        assert!(
            !derives(item, "Debug"),
            "{} holds a label and derives Debug",
            item.name
        );
        let printable = all.contains(&format!("impl std::fmt::Debug for {}", item.name));
        if derives(item, "Serialize") || printable {
            assert!(
                XTAB_HOLDS_LABELS.contains(&item.name.as_str()),
                "{} holds a String and is not listed",
                item.name
            );
            assert!(printable, "{} has no counts-only Debug", item.name);
        } else {
            assert!(
                !item.attrs.contains("#[derive("),
                "{} holds a String and derives something",
                item.name
            );
        }
    }
}

/// Phase 8Y-1 (decision 604): in `stats_ranks`, exactly the listed types
/// derive `Serialize`, none names a cell, the dataset or a reading, a type
/// listed as holding no text holds none, and `Split`, `Ready` and `Ranked`,
/// which hold the values, the labels and the ranks' sums, derive nothing.
#[test]
fn only_the_listed_stats_ranks_types_derive_serialize_and_none_holds_a_cell() {
    let items: Vec<Item> = sources(STATS_RANKS, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = items
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = RANKS_HOLDS_LABELS
        .iter()
        .chain(RANKS_HOLDS_NO_TEXT)
        .copied()
        .collect();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_RANKS} are not the listed ones"
    );
    for item in items.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
        if RANKS_HOLDS_NO_TEXT.contains(&item.name.as_str()) {
            assert!(
                !holds_string(&item.body),
                "{} is listed as holding no text but has a String field",
                item.name
            );
            for text in RANKS_HOLDS_LABELS {
                assert!(
                    !identifiers(&item.body).contains(text),
                    "{} is listed as holding no text but holds a {text}",
                    item.name
                );
            }
        }
    }
    for reading in ["Split", "Ready", "Ranked"] {
        let item = items
            .iter()
            .find(|i| i.name == reading)
            .unwrap_or_else(|| panic!("{reading} exists"));
        assert!(
            !item.attrs.contains("#[derive("),
            "{reading} holds the values, the labels or the ranks and derives nothing"
        );
    }
}

/// Phase 8Y-1 (decision 604): a `stats_ranks` type with a `String` field does
/// not derive `Debug`; one that is serialised or printed is listed and prints
/// counts through its own `Debug`; any other derives nothing.
#[test]
fn a_stats_ranks_type_holding_text_prints_counts_through_its_own_debug() {
    let files = sources(STATS_RANKS, 2);
    let all: String = files.iter().map(|(_, c)| c.as_str()).collect();
    let texty: Vec<Item> = files
        .iter()
        .flat_map(|(_, c)| items(c))
        .filter(|i| holds_string(&i.body))
        .collect();
    assert!(
        texty.len() >= 2,
        "{} text-holding types found in {STATS_RANKS} — the scan is inert",
        texty.len()
    );
    for item in &texty {
        assert!(
            !derives(item, "Debug"),
            "{} holds a label and derives Debug",
            item.name
        );
        let printable = all.contains(&format!("impl std::fmt::Debug for {}", item.name));
        if derives(item, "Serialize") || printable {
            assert!(
                RANKS_HOLDS_LABELS.contains(&item.name.as_str()),
                "{} holds a String and is not listed",
                item.name
            );
            assert!(printable, "{} has no counts-only Debug", item.name);
        } else {
            assert!(
                !item.attrs.contains("#[derive("),
                "{} holds a String and derives something",
                item.name
            );
        }
    }
}
