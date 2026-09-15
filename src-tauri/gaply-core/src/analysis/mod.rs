//! **The analysis record — what the researcher's own analysis files say was run.**
//!
//! §1: *"Per-type parsers produce a normalised analysis record: which tests were
//! run, on what variables, with what parameters, producing what outputs. That
//! record is what the methodological cluster reasons over — not the raw
//! script."* §4.3 names it as [`crate::agent_graph::Artifact::AnalysisRecord`],
//! a SUPPLIED artifact: absent means the researcher uploaded nothing, and the
//! agent is not invoked.
//!
//! # READ-ONLY. THE CODE IS NEVER EXECUTED, AND THAT IS A GUARD, NOT A COMMENT
//!
//! §11 and §12's Phase 8 rest on the absence of process execution from this
//! product: *"this reintroduces process execution into a product whose security
//! posture rests on its absence"*. A parser is exactly where that would erode
//! first — running the script is the obvious way to learn what it did.
//!
//! `tests/analysis_never_executes.rs` is the guard. It scans this directory for
//! any route to a subprocess, an interpreter, a dynamic library or the network,
//! and pins the import list. The standing lesson is that a purity claim in a doc
//! comment is not a guard: `validate.rs`, `stats_verify.rs` and
//! `stats_verdict.rs` each state one, with zero tests, and have held only
//! because nobody tried.
//!
//! # WHAT THE CORPUS SAYS, MEASURED 15 Sep 2026 — AND IT IS WHY ONLY SPSS SHIPS
//!
//! The parsers this phase asked for were *Python, R and SPSS output*. Measured
//! before writing any of them:
//!
//! | | found |
//! |---|---|
//! | analysis files in the researcher corpus (`~/Desktop`, `~/Documents`, `~/Downloads`, depth 3–4) | **0** — no `.R`, `.py`, `.sps`, `.sav`, `.ipynb`, `.csv`, `.xlsx`, against 29 `.docx` |
//! | the same, by NAME across all of `$HOME` to depth 5 | **0** — the five `.ipynb` found are three empty checkpoints (0 cells) and two VS Code extension samples |
//! | the same, by CONTENT (`scipy.stats`, `statsmodels`, `sklearn.model_selection`) | **0** — the only two matches are `stats_verify.rs` in the dead `~/Desktop` checkout and this phase's own prompts document, both of which merely name the libraries |
//! | upload paths in the app that accept one | **0** — every picker is `pdf, docx, txt, md` |
//! | real analysis artefacts anywhere on the machine | **1** — SPSS's own journal, 12.7 KB, 5 `FACTOR` procedures from a real session |
//!
//! So: **SPSS syntax is the only one of the three with a real input to be
//! measured against**, and it ships. Python and R do not, because a parser whose
//! only inputs are fixtures its author wrote inherits its author's premise —
//! the failure this project has recorded three times, most recently in
//! `journal_expect::is_reviewer_guidance`, which passed four hand-written tests
//! and returned `true` for 20 of 36 real pages. The cost of waiting is zero:
//! with no upload path, no parser of any language has an input.
//!
//! **The `.py` and `.R` files that ARE on this machine are IBM's bundled SPSS
//! extension source** (461 and 17 of them). They are plotting and dialogue
//! plumbing, not anyone's analysis. Measuring an R parser on them would say the
//! parser parses R, not that it recovers an analysis record — the
//! boundary-drawn-too-wide error of §11 D160, where a crawl of PLOS ONE reached
//! PLOS Genetics.

pub mod spss;

use serde::{Deserialize, Serialize};

/// Version of the persisted analysis-record shape.
pub const ANALYSIS_RECORD_SCHEMA_VERSION: u32 = 1;

/// The language a source file is written in. Closed on purpose: a new one is a
/// new parser, and [`AnalysisLanguage::is_parsed`] forces the decision about
/// whether it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisLanguage {
    /// SPSS command syntax — a `.sps` file, or the journal SPSS writes itself.
    SpssSyntax,
    R,
    Python,
}

impl AnalysisLanguage {
    /// **Does a parser for this language exist?** See the module header for why
    /// two of three are `false` — it is a corpus decision, not a difficulty one.
    pub fn is_parsed(self) -> bool {
        match self {
            AnalysisLanguage::SpssSyntax => true,
            AnalysisLanguage::R | AnalysisLanguage::Python => false,
        }
    }
}

/// One uploaded file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisSource {
    /// The file's own name. **Never its path** — a path is user-identifying and
    /// this record is meant to be able to cross the §3.1 boundary under
    /// `AnalysisCode` consent.
    pub file_name: String,
    pub language: AnalysisLanguage,
    pub lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProcedureId(pub String);

/// **What kind of analysis a command performs.**
///
/// The variants are the question a methodological specialist asks — *was a
/// regression run, was a reliability coefficient computed* — not SPSS's command
/// vocabulary. [`ProcedureKind::Unrecognised`] carries the keyword verbatim so
/// an unknown command is COUNTED AND NAMED rather than dropped: a parser that
/// silently discards what it does not understand reports a clean record of a
/// file it mostly failed to read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcedureKind {
    Descriptives,
    Frequencies,
    Correlation,
    CrossTabulation,
    TTest,
    OneWayAnova,
    GeneralLinearModel,
    LinearRegression,
    LogisticRegression,
    FactorAnalysis,
    ReliabilityAnalysis,
    NonparametricTest,
    ClusterAnalysis,
    SurvivalAnalysis,
    /// Reading, saving, recoding, filtering — everything that moves data around
    /// without testing anything. Kept because *what was recoded before the test*
    /// is a methodological question, not noise.
    DataManagement,
    /// A command the parser recognised as a command and not as a procedure.
    Unrecognised(String),
}

impl ProcedureKind {
    /// Does this procedure produce a statistical result a claim could rest on?
    pub fn is_statistical(&self) -> bool {
        !matches!(self, ProcedureKind::DataManagement | ProcedureKind::Unrecognised(_))
    }
}

/// One subcommand: `/ROTATION VARIMAX` -> `("ROTATION", "VARIMAX")`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subcommand {
    pub name: String,
    /// The subcommand's arguments as written, whitespace-normalised. Empty when
    /// the subcommand is a bare flag.
    pub value: String,
}

/// One command recovered from an analysis file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Procedure {
    pub id: ProcedureId,
    /// Index into [`AnalysisRecord::sources`].
    pub source_index: usize,
    pub kind: ProcedureKind,
    /// The command keyword as written — `FACTOR`, `GET DATA`, `T-TEST`.
    pub keyword: String,
    /// **THE SPAN. The command verbatim, whole.**
    ///
    /// A record that carries only "a factor analysis was run" has to be
    /// trusted; one that carries the twelve lines of `FACTOR` can be refuted by
    /// anyone in a glance. Stored unclipped — a truncated span is not a span,
    /// and clipping at display time is the same defect from the reader's side.
    pub raw: String,
    /// 1-based line in the source file where the command starts.
    pub line: usize,
    /// Variables the command names, in order of appearance, deduplicated.
    pub variables: Vec<String>,
    pub subcommands: Vec<Subcommand>,
}

impl Procedure {
    /// The value of a named subcommand, case-insensitively.
    pub fn subcommand(&self, name: &str) -> Option<&str> {
        self.subcommands
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .map(|s| s.value.as_str())
    }
}

/// **The normalised record. What was run, on what, with what parameters.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRecord {
    pub schema_version: u32,
    pub sources: Vec<AnalysisSource>,
    pub procedures: Vec<Procedure>,
    /// Lines the parser could not assign to any command. **Reported, not
    /// discarded**: a record built from a file the parser half-read is worse
    /// than no record, and this is the number that says which one you have.
    pub unparsed_lines: usize,
}

impl AnalysisRecord {
    pub fn empty() -> Self {
        Self {
            schema_version: ANALYSIS_RECORD_SCHEMA_VERSION,
            sources: Vec::new(),
            procedures: Vec::new(),
            unparsed_lines: 0,
        }
    }

    /// Procedures that actually test something.
    pub fn statistical(&self) -> impl Iterator<Item = &Procedure> {
        self.procedures.iter().filter(|p| p.kind.is_statistical())
    }

    /// Every distinct statistical procedure kind the record contains.
    pub fn kinds(&self) -> Vec<ProcedureKind> {
        let mut out: Vec<ProcedureKind> = Vec::new();
        for p in self.statistical() {
            if !out.contains(&p.kind) {
                out.push(p.kind.clone());
            }
        }
        out
    }
}
