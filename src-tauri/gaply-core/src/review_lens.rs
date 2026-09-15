//! **Reviewer lenses — §4.5, and the correction that section needed.**
//!
//! §4.5 specified a lens as a perspective over SPECIALIST OUTPUT:
//! `reads: Vec<SpecialistId>`. Measured, that is the one layer that is empty.
//! Of §4.2's eleven specialists, four ship and seven are declined — five
//! against the scientific layer (§11 D165) and one against the absent
//! analysis-file upload path. The layers underneath are populated on every run.
//!
//! **So a lens reads the research state and the fingerprint directly**, and
//! [`LensSource`] is the widened `reads`. The correction is recorded in §4.5 of
//! `docs/publishready-premium-architecture.md`, with the measurement behind it,
//! the way §6b.1's gating item and §3.4's discovery rule were recorded.
//!
//! # THE EVIDENCE POLICY IS NOW THE ONLY THING BETWEEN A LENS AND D165
//!
//! A lens reading `ResearchState` directly is doing what a specialist would
//! have done, without a specialist's `applies_to` and without a specialist
//! author between it and the manuscript. §11 D165 declined the scientific layer
//! because it produced plausible objects from prose it had misread; a lens with
//! the same reach and no gate would be the same failure one tier up.
//!
//! **§11 D157 is the precedent, and it is the ratio to aim at.** The equation
//! binder also reads a lower layer directly — the manuscript's own symbols —
//! and across nine documents produced **2 bindings, 76 refusals, 1 finding**.
//! It refuses by default and binds only where the document settles the
//! question, because one manuscript declares `N` with five values and a
//! document-wide symbol table would bind the wrong one and then disprove
//! correct arithmetic.
//!
//! # WHAT AN ABSENT FIELD MEANS, PER FIELD
//!
//! [`FieldReach`] is the rule, and it is declared per field rather than per
//! lens so the same field cannot mean two things in two reports:
//!
//! * [`FieldReach::Exhaustive`] — every word of the manuscript was searched.
//!   `docparse` produces the text and nothing classifies it, so a miss can only
//!   be a lexicon miss, which the finding's `uncertainty` must state.
//!   *"No data availability statement found"* IS a finding.
//! * [`FieldReach::Mediated`] — a projection the extractor built, which can omit
//!   what the manuscript contains. *"No methods section extracted"* is NOT a
//!   finding: the extractor is the reason, and the manuscript may be fine.
//!
//! **Measured over the six real manuscripts** (`examples/absence_scan.rs`), and
//! the two kinds do not behave alike:
//!
//! | mediated field | empty in | though the content is there |
//! |---|---:|---|
//! | `Conclusion` section | **5 of 6** | all six conclude; three spellings are recognised |
//! | `Results` / `Discussion` | 3 of 6 each | |
//! | `references` | 2 of 6 | those two carry 15 and 165 in-text citations |
//! | `title` | 2 of 6 are not titles | a Turnitin cover page; an AI-detector banner |
//!
//! | exhaustive field | found in | adjudication |
//! |---|---:|---|
//! | data-availability phrases | 1 of 6 | correct |
//! | ethics phrases | 1 of 6 | correct |
//! | randomisation phrases | 1 of 6 | correct |
//!
//! A lens allowed to conclude from a mediated absence would have reported five
//! of six manuscripts as having no conclusion. [`AbsenceRejected`] is returned,
//! never dropped, for the reason `specialist::run` already returns its own.
//!
//! # FOUR LENSES, AND TWO DECLINED FOR DIFFERENT REASONS
//!
//! Methodology, Statistics, Novelty & literature, Reporting & ethics ship.
//!
//! **Journal fit is declined because its layer is empty**: `derive_conventions`
//! and `PublishedPaper` exist, the OpenAlex fetch that fills them is in the app
//! crate, no `journal_papers` table exists, and the only in-core caller is a
//! test with twenty fabricated papers. `RequirementKind` has nine variants and
//! none is scope.
//!
//! **General is declined because the absence rule forbids it**: *clarity and
//! structure* and *title/abstract/keywords* are read from section presence and
//! `title`, both mediated and both measured wrong above. Its third criterion,
//! *overstated conclusions*, has a real input and moves to Methodology, where
//! §4.4 already puts "is causal language earned" at Tier 2.
//!
//! # A LENS STILL COMPUTES NOTHING AN EXTRACTOR SHOULD
//!
//! Reading a field is not the same as deriving one. A lens may ask whether a
//! phrase occurs and what a `year` is; it does not build a parallel extractor.
//! The line is [`FieldReach`]: everything a lens reads is a field
//! `ExtractionResult` or `JournalFingerprint` already carries, or a search over
//! text they already produced.
//!
//! # THE RISK TABLE'S THIRD COLUMN IS NOT A SEVERITY
//!
//! §4.5 describes `severity_policy` as *"the risk table: fatal / fixable /
//! minor per criterion"*. The document's third column is *Low Risk (Minor
//! Issue)* and **every cell in it describes the GOOD case**:
//!
//! > Methodology Quality … *"Robust design with adequate power and thorough
//! > explanation; appropriate analysis."*
//! > Journal Compliance … *"Fully compliant with instructions; well-labelled,
//! > styled manuscript."*
//!
//! Read as specified, each criterion carries a *minor concern* whose text
//! describes excellence. [`RiskBand::compliant`] is named for what it holds and
//! feeds [`ReviewerReport::strengths`].
//!
//! §8 gives a four-level vocabulary for the same table — *Blocking · Major ·
//! Minor · Informational*. [`ReviewSeverity`] is §8's, because the editor layer
//! (§5.7, Phase 5) consumes it and two vocabularies for one concept is the
//! §11 D129 shape.

use serde::{Deserialize, Serialize};

use crate::agent_graph::EvidencePolicy;
use crate::extract::{ExtractionResult, Location};
use crate::novelty::{NoveltyAssessment, NoveltyStatus};
use crate::report::{ChecklistItem, StandardEvaluation};
use crate::specialist::SpecialistReport;
use crate::validate::StatsValidityReport;

// ---------------------------------------------------------------------------
// Sources
// ---------------------------------------------------------------------------

/// Every producer a lens can read, shipped or not.
///
/// §4.5 types this as `Vec<SpecialistId>`. The set below is wider than the
/// `Specialist` trait's implementors on purpose: `validation_maths` is a lane,
/// and the two checklist evaluators emit [`ChecklistItem`] rather than
/// `specialist::Finding`. A lens reads all of them, and pretending otherwise
/// would make the three shipping nodes that are not `Specialist` impls
/// invisible to every lens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    FrequentistStats,
    MlMethodology,
    ClaimEvidenceStrength,
    NoveltyClaims,
    ValidationMaths,
    ReportingStandards,
    JournalRequirements,
    ComparableCorpus,
    BayesianAnalysis,
    QualitativeMethods,
    SurveyPsychometric,
    LabWetBench,
    ComputationalConsistency,
    ReferenceCurrency,
    LiteratureCoverage,
    ClarityStructure,
}

/// Whether a source exists, and if not, what is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    /// Built and producing rows on the real corpus.
    Shipping,
    /// Declined against a named empty layer. Never "not yet built".
    Declined,
    /// The type exists; nothing fills it in this crate.
    NoDataPath,
    /// Nothing produces this at all.
    NotBuilt,
}

impl SourceId {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceId::FrequentistStats => "frequentist_stats",
            SourceId::MlMethodology => "ml_methodology",
            SourceId::ClaimEvidenceStrength => "claim_evidence_strength",
            SourceId::NoveltyClaims => "novelty_claims",
            SourceId::ValidationMaths => "validation_maths",
            SourceId::ReportingStandards => "reporting_standards",
            SourceId::JournalRequirements => "journal_requirements",
            SourceId::ComparableCorpus => "comparable_corpus",
            SourceId::BayesianAnalysis => "bayesian_analysis",
            SourceId::QualitativeMethods => "qualitative_methods",
            SourceId::SurveyPsychometric => "survey_psychometric",
            SourceId::LabWetBench => "lab_wet_bench",
            SourceId::ComputationalConsistency => "computational_consistency",
            SourceId::ReferenceCurrency => "reference_currency",
            SourceId::LiteratureCoverage => "literature_coverage",
            SourceId::ClarityStructure => "clarity_structure",
        }
    }

    /// **The measurement, as of 15 Sep 2026.** TOTAL, with no wildcard arm: a
    /// new source cannot compile until someone decides whether it exists.
    pub fn state(self) -> SourceState {
        match self {
            SourceId::FrequentistStats
            | SourceId::MlMethodology
            | SourceId::ClaimEvidenceStrength
            | SourceId::ValidationMaths
            | SourceId::ReportingStandards
            | SourceId::JournalRequirements => SourceState::Shipping,
            // §11 D166: 12 candidate sentences over 20 real manuscripts,
            // 2 of them real novelty claims — 0.1 per manuscript — and §4.6's
            // own phrasings in none. Not a retrieval problem: the manuscripts
            // do not contain the sentences the pipeline reads.
            SourceId::NoveltyClaims => SourceState::Declined,
            // §11 D165: the scientific layer measured 5.9% precision against a
            // 50% no-skill baseline and was switched back off.
            SourceId::BayesianAnalysis
            | SourceId::QualitativeMethods
            | SourceId::SurveyPsychometric
            | SourceId::LabWetBench => SourceState::Declined,
            // No upload path accepts an analysis file; the parser and the
            // record type both exist.
            SourceId::ComputationalConsistency => SourceState::Declined,
            // `PublishedPaper` and `derive_conventions` exist; the fetch that
            // fills them is in the app crate and no table stores the result.
            SourceId::ComparableCorpus => SourceState::NoDataPath,
            SourceId::ReferenceCurrency
            | SourceId::LiteratureCoverage
            | SourceId::ClarityStructure => SourceState::NotBuilt,
        }
    }

    /// Why, in one sentence a reader can act on.
    pub fn note(self) -> &'static str {
        match self.state() {
            SourceState::Shipping => "built, and producing rows on the six real manuscripts",
            SourceState::Declined => match self {
                SourceId::NoveltyClaims =>
                    "declined (§11 D166): a 31-cue scan over 20 real manuscripts found 12 \
                     candidate sentences and 2 real novelty claims — 0.1 per manuscript — \
                     and §4.6's own phrasings appear in none of them. The instrument is \
                     kept; no lens consumes it",
                SourceId::ComputationalConsistency =>
                    "declined: no upload path accepts an analysis file. The SPSS parser and \
                     the `AnalysisRecord` type already exist",
                _ => "declined against the scientific layer (§11 D165): 5.9% precision \
                      against a 50% no-skill baseline",
            },
            SourceState::NoDataPath =>
                "the type and the derivation exist in `journal_corpus`; the OpenAlex fetch \
                 that fills them is in the app crate, no `journal_papers` table exists, and \
                 the only in-core caller is a test with twenty fabricated papers",
            SourceState::NotBuilt => match self {
                SourceId::ReferenceCurrency =>
                    "not built. Every `Reference` carries a `year`, so this is cheap — but a \
                     lens computing it would be an extractor wearing a lens's name",
                SourceId::LiteratureCoverage =>
                    "not built. Needs the field's literature, which is the comparable \
                     corpus's problem one layer down",
                _ => "not built. Section structure is extracted; nothing judges it",
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Research-state and fingerprint fields, and what their absence means
// ---------------------------------------------------------------------------

/// **What a lens searched, and therefore what an empty result means.**
///
/// Declared per FIELD rather than per lens, so the same field cannot mean two
/// things in two reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldReach {
    /// Every word of the manuscript. A miss is a lexicon miss, and the finding
    /// must say so. **An absence here is about the MANUSCRIPT.**
    Exhaustive,
    /// A projection the extractor built, which can omit what the manuscript
    /// contains. **An absence here is about the EXTRACTOR**, and no finding may
    /// rest on it.
    Mediated,
}

/// A field of the research state or the journal fingerprint that a lens reads.
///
/// TOTAL with no wildcard on [`Self::reach`]: a new field cannot compile until
/// somebody decides whether its absence is a fact about the manuscript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateField {
    /// The concatenated manuscript text, searched for a phrase.
    FullText,
    /// `ExtractionResult::statistics` — regex extraction over the full text.
    Statistics,
    /// `ExtractionResult::references` — a parsed bibliography.
    References,
    /// The `year` of references that WERE parsed. Reasoning over present values.
    ReferenceYears,
    /// `ExtractionResult::sections` — a heading classifier.
    Sections,
    /// `ExtractionResult::title` — measured to be a Turnitin cover page or an
    /// AI-detector banner on 2 of 6 real manuscripts.
    Title,
    /// `ExtractionResult::tables`.
    Tables,
    /// The journal's stored requirements, each with its four-way status.
    JournalRequirements,
    /// Which reporting standards the journal binds, and to which designs.
    JournalStandardBindings,
}

impl StateField {
    pub fn as_str(self) -> &'static str {
        match self {
            StateField::FullText => "full_text",
            StateField::Statistics => "statistics",
            StateField::References => "references",
            StateField::ReferenceYears => "reference_years",
            StateField::Sections => "sections",
            StateField::Title => "title",
            StateField::Tables => "tables",
            StateField::JournalRequirements => "journal_requirements",
            StateField::JournalStandardBindings => "journal_standard_bindings",
        }
    }

    /// **The rule. TOTAL, no wildcard.**
    pub fn reach(self) -> FieldReach {
        match self {
            // `docparse` produces the text; nothing classifies it. Searching it
            // for a phrase reaches every word the manuscript contains.
            StateField::FullText => FieldReach::Exhaustive,
            // Present values only — these say nothing by being absent, and no
            // rule below asks them to.
            StateField::ReferenceYears => FieldReach::Exhaustive,
            // Everything else is a projection that can omit what is there.
            // Measured: the Conclusion heading is missed on 5 of 6 real
            // manuscripts, `references` is empty on 2 of 6 that carry 15 and 165
            // in-text citations, and `title` is not a title on 2 of 6.
            StateField::Statistics
            | StateField::References
            | StateField::Sections
            | StateField::Title
            | StateField::Tables => FieldReach::Mediated,
            // A requirement absent from the store can mean the journal does not
            // state it OR that the crawl never reached the page. The fingerprint
            // carries a four-way status for exactly this, and `UNAVAILABLE` is
            // the honest answer rather than a silent absence.
            StateField::JournalRequirements | StateField::JournalStandardBindings => {
                FieldReach::Mediated
            }
        }
    }

    /// Why, for a reader who will not read the measurement.
    pub fn absence_note(self) -> &'static str {
        match self {
            StateField::FullText =>
                "is every word of the manuscript as `docparse` produced it, so a miss can \
                 only be a phrasing this lexicon does not know",
            // **Not a lexicon and not an absence.** This field reasons over the
            // years of references that WERE parsed and concludes nothing from
            // any being missing; the denominator guard (`MINIMUM_DATED`) is
            // what stops it speaking when there are too few. It is Exhaustive
            // because it never asks an absence question at all — and the probe
            // printing the full-text note here is what made that worth saying.
            StateField::ReferenceYears =>
                "carries only the reference years that WERE parsed, so nothing is concluded \
                 from a reference having none; below ten dated references it says nothing",
            _ => match self.reach() {
            FieldReach::Exhaustive => {
                "is searched in full, so a miss can only be a phrasing this lexicon does \
                 not know"
            }
            FieldReach::Mediated => match self {
                StateField::Sections =>
                    "comes from a heading classifier: measured, the Conclusion heading is \
                     missed on 5 of 6 real manuscripts that all conclude",
                StateField::References =>
                    "comes from a bibliography parser that returns nothing on 2 of 6 real \
                     manuscripts carrying 15 and 165 in-text citations",
                StateField::Title =>
                    "is a Turnitin cover page or an AI-detector banner on 2 of 6 real \
                     manuscripts",
                StateField::Statistics =>
                    "omits any statistic in a format the extractor does not know, which is \
                     indistinguishable from one that is not there",
                StateField::Tables =>
                    "omits any table the converter flattened, which is indistinguishable \
                     from one the paper does not have",
                _ =>
                    "may be missing because the journal does not state it OR because the \
                     crawl never reached the page; the fingerprint's four-way status is \
                     where that distinction lives",
            },
            },
        }
    }
}

/// **§4.5's `reads`, widened.** A lens sees specialist reports, research-state
/// fields and fingerprint fields — see the module header for why the first alone
/// was the defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensSource {
    Specialist(SourceId),
    State(StateField),
}

impl LensSource {
    pub fn as_str(self) -> &'static str {
        match self {
            LensSource::Specialist(s) => s.as_str(),
            LensSource::State(f) => f.as_str(),
        }
    }

    /// Whether this source exists.
    ///
    /// **A research-state field always exists** — that is the whole point of
    /// the §4.5 correction: `ExtractionResult` is populated on every run. What
    /// varies is what may be concluded from it, and [`StateField::reach`] is
    /// where that lives, not here.
    pub fn state(self) -> SourceState {
        match self {
            LensSource::Specialist(s) => s.state(),
            LensSource::State(_) => SourceState::Shipping,
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            LensSource::Specialist(s) => s.note(),
            LensSource::State(f) => f.absence_note(),
        }
    }

    /// The specialist behind this source, if it is one.
    pub fn specialist(self) -> Option<SourceId> {
        match self {
            LensSource::Specialist(s) => Some(s),
            LensSource::State(_) => None,
        }
    }

    /// The research-state field behind this source, if it is one.
    pub fn field(self) -> Option<StateField> {
        match self {
            LensSource::State(f) => Some(f),
            LensSource::Specialist(_) => None,
        }
    }
}

/// A finding the absence rule refused, and why. **Returned, never dropped** —
/// a gate that silently swallows what it rejects is indistinguishable from a
/// lens that found nothing, which is the failure this project has recorded
/// four times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbsenceRejected {
    pub criterion: String,
    pub code: String,
    pub field: StateField,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// The reviewer document's Risk Assessment Table
// ---------------------------------------------------------------------------

/// The six rows of `docs/reviewer-criteria.md`'s Risk Assessment Table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskRow {
    NoveltyOriginality,
    MethodologyQuality,
    ClarityStructure,
    DataEthics,
    LiteratureCitations,
    JournalCompliance,
}

/// One row of that table, **verbatim**.
pub struct RiskBand {
    pub row: RiskRow,
    pub label: &'static str,
    /// *High Risk (Fatal Flaws)*.
    pub fatal: &'static str,
    /// *Medium Risk (Fixable)*.
    pub fixable: &'static str,
    /// *Low Risk (Minor Issue)* — **which describes the GOOD case**. Named
    /// `compliant` rather than `minor` because reading the column label as a
    /// severity produces a minor concern whose text is a compliment.
    pub compliant: &'static str,
}

impl RiskRow {
    pub fn band(self) -> &'static RiskBand {
        RISK_TABLE.iter().find(|b| b.row == self).expect("every row is in the table")
    }
}

/// Verbatim from `docs/reviewer-criteria.md`, *Risk Assessment Table for Common
/// Criteria*. Quoted rather than paraphrased so a reader can diff it against
/// the document.
pub static RISK_TABLE: &[RiskBand] = &[
    RiskBand {
        row: RiskRow::NoveltyOriginality,
        label: "Novelty/Originality",
        fatal: "Just repeats known results; obvious question; local interest only.",
        fixable: "Adds some new angle but under-emphasized; novelty unclear to reader.",
        compliant: "Presents a clearly new finding, well-motivated hypothesis.",
    },
    RiskBand {
        row: RiskRow::MethodologyQuality,
        label: "Methodology Quality",
        fatal: "Flawed design (no control, biased sampling, confounders); inappropriate \
                stats (no replication, wrong test).",
        fixable: "Some methodological limitations (small N, some missing controls) or not \
                  fully described.",
        compliant: "Robust design with adequate power and thorough explanation; appropriate \
                    analysis.",
    },
    RiskBand {
        row: RiskRow::ClarityStructure,
        label: "Clarity & Structure",
        fatal: "Manuscript is disorganized or incomprehensible; missing sections (e.g. no \
                abstract or unclear intro).",
        fixable: "Minor language issues or a few unclear paragraphs; some reformatting \
                  needed (e.g. reorder sections).",
        compliant: "Well-structured, logically flowing text; minimal language edits needed.",
    },
    RiskBand {
        row: RiskRow::DataEthics,
        label: "Data & Ethics",
        fatal: "Missing ethics approval when required; no raw data or hiding key results; \
                plagiarism.",
        fixable: "Limited data reporting (e.g. only summaries); Data-available-on-request \
                  (problematic); small ethical oversight (e.g. consent form detail omitted).",
        compliant: "All data deposited publicly; full ethical disclosure; reproducible \
                    methods.",
    },
    RiskBand {
        row: RiskRow::LiteratureCitations,
        label: "Literature & Citations",
        fatal: "Ignores key literature; high self-citation/coercive citations; plagiarism \
                detected.",
        fixable: "Minor omissions of related studies; references slightly outdated; some \
                  formatting errors in references.",
        compliant: "Comprehensive, up-to-date citations; follows reference style perfectly.",
    },
    RiskBand {
        row: RiskRow::JournalCompliance,
        label: "Journal Compliance",
        fatal: "Not following guidelines: wrong format, no cover letter, huge overshoot of \
                limits, mandatory statements missing.",
        fixable: "Small format issues (e.g. incorrect reference style); word count slightly \
                  over; table/figure renumbering needed.",
        compliant: "Fully compliant with instructions; well-labelled, styled manuscript.",
    },
];

/// §8's four levels. *"Precedence is deterministic: a blocking finding is
/// blocking regardless of surrounding strengths; the editor cannot demote it."*
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSeverity {
    Blocking,
    Major,
    Minor,
    Informational,
}

impl ReviewSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            ReviewSeverity::Blocking => "BLOCKING",
            ReviewSeverity::Major => "MAJOR",
            ReviewSeverity::Minor => "MINOR",
            ReviewSeverity::Informational => "INFORMATIONAL",
        }
    }
}


// ---------------------------------------------------------------------------
// The checks a lens makes over the research state directly
// ---------------------------------------------------------------------------

/// One phrase-presence check over the full text.
///
/// **Every one of these reads [`StateField::FullText`]**, which is the only
/// research-state field an absence may be concluded from. A check wanting a
/// mediated field would be rejected by [`admit_absence`], and the two that
/// tried are recorded in the module header.
pub struct StateCheck {
    pub code: &'static str,
    /// Phrases whose presence satisfies the check, lowercased.
    pub phrases: &'static [&'static str],
    /// What the report says when NONE of them occurs.
    pub absent_summary: &'static str,
    /// The limit of the lexicon, carried into every finding's uncertainty.
    pub uncertainty: &'static str,
    pub field: StateField,
}

/// **Data availability.** The reviewer document's *Reproducibility & Data
/// Availability*: *"Weak: … 'data available on request' statements"*.
///
/// Measured on the six manuscripts: found in exactly 1, the health-economics
/// paper, which genuinely carries one. The other five genuinely do not.
pub static DATA_AVAILABILITY_CHECK: StateCheck = StateCheck {
    code: "no_data_availability_statement",
    phrases: &[
        "data availability",
        "data are available",
        "data is available",
        "data will be made available",
        "available on request",
        "available upon request",
        "deposited in",
        "accession number",
        "supplementary data",
        "underlying data",
        "zenodo",
        "dryad",
        "figshare",
        "osf.io",
    ],
    absent_summary: "No data availability statement was found anywhere in the manuscript. \
                     The reviewer document puts this in the Data & Ethics row's fatal cell \
                     (\"no raw data or hiding key results\") and most journals now require a \
                     statement even when the answer is that data cannot be shared.",
    uncertainty: "Searched the whole manuscript for 14 phrasings. A statement worded in none \
                  of them reads here as absent, which is a lexicon limit rather than a \
                  finding about the manuscript. The check does not judge whether a statement \
                  that IS present is adequate.",
    field: StateField::FullText,
};

/// **Ethics approval.** *"Weak: No mention of ethics, or retroactive approval;
/// missing consent forms for human studies."*
///
/// **This check declines unless the manuscript shows it involved humans or
/// animals**, and that gate is the whole difference between a finding and a
/// nuisance: a limnology chapter or a text-classification paper needs no ethics
/// statement, and flagging one would be the ML specialist firing on a silkworm
/// paper (§4.2).
pub static ETHICS_CHECK: StateCheck = StateCheck {
    code: "no_ethics_statement_in_a_study_with_subjects",
    phrases: &[
        "ethics approval",
        "ethical approval",
        "ethics committee",
        "ethical clearance",
        "institutional review board",
        "institutional ethics",
        "iacuc",
        "informed consent",
        "consent to participate",
        "declaration of helsinki",
        "ethics statement",
        "arrive guidelines",
        "animal welfare",
        "ethical considerations",
    ],
    absent_summary: "The manuscript describes work with human participants or animals and no \
                     ethics approval, consent or clearance statement was found anywhere in \
                     it. The reviewer document's Data & Ethics fatal cell names \"missing \
                     ethics approval when required\".",
    uncertainty: "Searched the whole manuscript for 14 phrasings, and decided the study \
                  involves subjects from its own words. Both halves are lexicons: a study \
                  that is exempt, or that states its approval in wording none of these \
                  match, reads here as missing one.",
    field: StateField::FullText,
};

/// **Phrases showing that THIS study had human or animal subjects.**
///
/// Every entry carries a verb or a qualifier, and that is not tidiness. The
/// first version was a bare noun list — `participants`, `animals`, `survey of`,
/// `zebrafish` — and on `final final L.pdf` it produced a **BLOCKING** ethics
/// finding, the highest severity this system emits, on four false matches:
///
/// | matched | the manuscript's actual words |
/// |---|---|
/// | `participants` | *"urban wetlands are active **participants** in their surrounding environment"* — a metaphor |
/// | `survey of` | *"The field **survey of** Section 4.1 established that the three lakes carry a heavy nutrient load"* — a survey of lakes |
/// | `animals` | *"an almost exclusive inhabitant of the gut of warm-blooded **animals**"* — E. coli's habitat, in a cited sentence |
/// | `zebrafish` | *"127 Publication Lawrence, C.. 'The husbandry of **zebrafish**…'"* — a row of the Turnitin similarity report bound into the PDF |
///
/// That manuscript does contain a zebrafish assay, so a stricter gate may still
/// fire on it — but it would fire for a reason, and the finding above rested on
/// a metaphor, a lake survey, a bacterium's habitat and somebody else's
/// bibliography. A false BLOCKING finding is the worst output this system can
/// produce, and the lexicon that produced it was one word wide.
const SUBJECTS_PRESENT: &[&str] = &[
    // Human subjects, with the conduct that makes them this study's.
    "participants were",
    "participants gave",
    "participants provided",
    "participants completed",
    "we recruited",
    "were recruited",
    "respondents were",
    "patients were",
    "patients who",
    "volunteers were",
    "human participants",
    "study participants",
    "human subjects",
    "interviews were conducted",
    "questionnaire was administered",
    "questionnaires were administered",
    // Animal subjects, likewise.
    "animals were",
    "animal experiments",
    "animal study",
    "laboratory animals",
    "mice were",
    "rats were",
    "rabbits were",
    "zebrafish were",
    "in vivo experiment",
    "clinical trial",
    "were sacrificed",
    "were euthanised",
    "were euthanized",
    "were administered",
];

/// The checks the methodology lens makes over the state.
pub static METHODOLOGY_STATE_CHECKS: &[&StateCheck] =
    &[&DATA_AVAILABILITY_CHECK, &ETHICS_CHECK];

/// **Reference currency**, from the years of references that WERE parsed.
///
/// §4.5 listed *currency* as a criterion and the old `reads: Vec<SpecialistId>`
/// had nothing to offer it. `Reference::year` has been there all along.
///
/// Reasons over PRESENT values only: the fraction is of references that carry a
/// year, and the count of those is stated so a reader can see the denominator
/// (§11 D161 — a fraction is a claim and its denominator is the half nobody
/// checks).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCurrency {
    /// References the parser returned.
    pub total: usize,
    /// Of those, the ones carrying a parsable year — the denominator.
    pub dated: usize,
    /// Of `dated`, those published in the last ten years.
    pub recent: usize,
    pub oldest: Option<i32>,
    pub newest: Option<i32>,
}

/// The number of years within which a reference counts as recent.
///
/// The reviewer document, verbatim: *"Strong: Majority of references from the
/// past decade"*; *"Weak: Reliance on outdated sources (10+ years old)"*.
pub const RECENT_WINDOW_YEARS: i32 = 10;

/// Compute currency against a supplied "now" year. **Injected, never
/// `SystemTime`** — a deterministic lens with a clock in it is not
/// reproducible, and this crate's norm is injected time.
pub fn reference_currency(
    refs: &[crate::extract::citations::Reference],
    this_year: i32,
) -> ReferenceCurrency {
    let years: Vec<i32> = refs.iter().filter_map(|r| r.year).collect();
    ReferenceCurrency {
        total: refs.len(),
        dated: years.len(),
        recent: years.iter().filter(|y| this_year - **y <= RECENT_WINDOW_YEARS).count(),
        oldest: years.iter().copied().min(),
        newest: years.iter().copied().max(),
    }
}

impl ReferenceCurrency {
    /// **Is there enough to say anything?** Fewer than ten dated references is
    /// a denominator too small to carry a majority claim, and the honest answer
    /// is to say nothing rather than to report "0 of 3 are recent".
    pub const MINIMUM_DATED: usize = 10;

    /// `Some(true)` when a majority are outside the window; `None` when the
    /// denominator cannot support the claim.
    pub fn majority_outdated(&self) -> Option<bool> {
        (self.dated >= Self::MINIMUM_DATED).then(|| self.recent * 2 < self.dated)
    }
}

// ---------------------------------------------------------------------------
// Severity policy — a finding code to a severity, with the table cell quoted
// ---------------------------------------------------------------------------

/// Which column of the risk table a finding sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Fatal,
    Fixable,
}

/// One rule of §4.5's `severity_policy`.
///
/// The `risk_cell` is what makes the severity auditable: a reader can read the
/// document's own sentence beside the level assigned, rather than being handed
/// a number somebody chose.
pub struct SeverityRule {
    /// A `specialist::Finding::code`, a `validate::RuleId` name, or a
    /// [`NoveltyStatus`] name.
    pub code: &'static str,
    pub severity: ReviewSeverity,
    pub row: RiskRow,
    pub level: RiskLevel,
    /// Why this code sits in that cell rather than the neighbouring one.
    pub why: &'static str,
}

/// **THE BLOCKING TIER HAS FOUR CODES AND TWO CANNOT FIRE.**
///
/// Stated here, at the table, so a reader counting `Blocking` entries does not
/// take four as the reachable count:
///
/// | code | reachable |
/// |---|---|
/// | `TestGroupMismatch` | **yes** — `validate.rs` |
/// | `SmallSampleCausalClaim` | **yes** — `validate.rs` |
/// | `test_claimed_but_absent_from_analysis` | **no** — needs an uploaded `AnalysisRecord`; no picker accepts an analysis file |
/// | `PRIOR_WORK_EXISTS` | **no** — the novelty pipeline is declined (§11 D166) |
///
/// Measured over 20 real manuscripts the tier fired **twice**, both
/// `SmallSampleCausalClaim`, on 2 of 20 papers. The two unreachable codes have
/// different reopening conditions — an upload path for the first, D166's corpus
/// measurement for the second — so they are kept rather than deleted, the same
/// way the declined lenses are kept.
///
/// `editor::PRECEDENCE_NOTE` carries this onto every rubric, because
/// `Blocking: 0` read as "nothing fatal is present" is the failure it prevents.
pub const REACHABLE_BLOCKING_CODES: &[&str] =
    &["TestGroupMismatch", "SmallSampleCausalClaim"];

/// §4.5's `severity_policy`, as data.
///
/// **Every shipped finding code appears here**, pinned by
/// `every_shipped_finding_code_has_a_severity_rule`. An unmapped code would
/// take [`DEFAULT_SEVERITY`] and be indistinguishable in the report from one
/// somebody decided — which is how a new check quietly acquires an authority
/// nobody granted it.
pub static SEVERITY_POLICY: &[SeverityRule] = &[
    // ---- frequentist_stats
    SeverityRule {
        code: "multiple_comparisons_uncorrected",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "the table's fatal cell names \"inappropriate stats (no replication, wrong \
              test)\", and an uncorrected family of tests is that — but it is correctable by \
              re-analysis without new data, which is why it is MAJOR and not BLOCKING",
    },
    SeverityRule {
        code: "parametric_test_assumptions_unstated",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "\"not fully described\" — the check cannot tell an unperformed test from an \
              unreported one, and the fixable cell is the one that covers both",
    },
    SeverityRule {
        code: "p_value_reported_as_exactly_zero",
        severity: ReviewSeverity::Minor,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "a reporting convention, corrected by writing p < 0.001",
    },
    SeverityRule {
        code: "marginal_significance_language",
        severity: ReviewSeverity::Minor,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "the document's \"Statistical Issues\": \"Over-claiming results … is frowned \
              upon\". A wording change, not a re-analysis",
    },
    SeverityRule {
        code: "test_claimed_but_absent_from_analysis",
        severity: ReviewSeverity::Blocking,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "the manuscript reports a test the uploaded analysis does not contain. Nothing \
              in a revision fixes a result that was not produced, so the editor cannot \
              demote it",
    },
    // ---- ml_methodology
    SeverityRule {
        code: "resampling_not_stated_as_post_split",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "leakage invalidates the reported performance — the fatal cell's \"flawed \
              design\". MAJOR rather than BLOCKING because the check reads the description \
              and not the code: the split may have come first and gone unstated",
    },
    SeverityRule {
        code: "no_heldout_evaluation_named",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "\"flawed design\" — a performance number with no held-out set measures fit, \
              not generalisation",
    },
    SeverityRule {
        code: "no_baseline_comparison",
        severity: ReviewSeverity::Major,
        row: RiskRow::NoveltyOriginality,
        level: RiskLevel::Fixable,
        why: "the novelty row's fixable cell: \"Adds some new angle but under-emphasized\". \
              Without a baseline the advance cannot be sized",
    },
    SeverityRule {
        code: "no_variance_across_runs",
        severity: ReviewSeverity::Minor,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "\"not fully described\" — re-running with seeds is a revision, not a redesign",
    },
    // ---- claim_evidence_strength
    SeverityRule {
        code: "causal_claim_from_associational_design",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "the document's \"Overstated Conclusions\": \"Claims must be justified by the \
              data.\" The fix is language, not data, so MAJOR — but the reviewer document \
              lists over-claiming among the commonest rejection causes",
    },
    // ---- validation_maths (Tier 0/1, hard constraint)
    SeverityRule {
        code: "TestGroupMismatch",
        severity: ReviewSeverity::Blocking,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "\"wrong test\", deterministically established. §4.4 puts Tier 0 above every \
              model in the system, so nothing downstream may demote it",
    },
    SeverityRule {
        code: "SmallSampleCausalClaim",
        severity: ReviewSeverity::Blocking,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "the document's own example of a fatal overclaim: \"claiming a treatment \
              'eliminates disease' based on limited animal data would be rejected\"",
    },
    SeverityRule {
        code: "PValueOverclaim",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fatal,
        why: "\"inappropriate stats\" — p-values reported alone is the weak case the \
              document names verbatim",
    },
    SeverityRule {
        code: "MissingEffectSize",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "\"not fully described\". The document: \"reports effect sizes or confidence \
              intervals, not just p-values\"",
    },
    SeverityRule {
        code: "MissingConfidenceInterval",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "as above — recomputable from what the manuscript already reports",
    },
    // ---- research-state checks (§4.5 [v8])
    SeverityRule {
        code: "no_data_availability_statement",
        severity: ReviewSeverity::Major,
        row: RiskRow::DataEthics,
        level: RiskLevel::Fatal,
        why: "the table's fatal cell names \"no raw data or hiding key results\". MAJOR \
              rather than BLOCKING because the check reads a 14-phrase lexicon over the \
              full text, and §8's Blocking is undemotable — an undemotable verdict on a \
              lexicon is the wrong pairing",
    },
    SeverityRule {
        code: "no_ethics_statement_in_a_study_with_subjects",
        severity: ReviewSeverity::Major,
        row: RiskRow::DataEthics,
        level: RiskLevel::Fatal,
        why: "\"Missing ethics approval when required\" is the table's fatal cell and §8 \
              names it as a blocking example verbatim. **It is MAJOR here, and the \
              downgrade was measured rather than reasoned.** It shipped as BLOCKING on the \
              argument that a doubly-gated finding is strong enough to be undemotable; the \
              first live run then surfaced it on `final final L.pdf`, where the subject \
              gate had fired on a metaphor, a survey of lakes, a description of E. coli's \
              habitat, and a row of the Turnitin report bound into the PDF. Both halves of \
              the gate are lexicons, and §8's Blocking cannot be demoted by the editor — an \
              undemotable verdict on a lexicon is the wrong pairing, the same call already \
              made for the reporting-standard codes. It goes back to BLOCKING when the \
              gate is a measured classifier rather than a phrase list",
    },
    SeverityRule {
        code: "reference_list_is_mostly_outdated",
        severity: ReviewSeverity::Minor,
        row: RiskRow::LiteratureCitations,
        level: RiskLevel::Fixable,
        why: "\"references slightly outdated\" — the table's fixable cell, and the fix is \
              a literature search rather than new work. It stays MINOR because a ten-year \
              window is the reviewer document's and not every field's",
    },
    // ---- reporting_standards
    //
    // **All six are MAJOR, and none is BLOCKING, deliberately.** The risk
    // table's Clarity & Structure row calls "missing sections (e.g. no abstract
    // or unclear intro)" a FATAL flaw, and §8 defines Blocking as "a VERIFIED
    // journal requirement unmet" which "the editor cannot demote". These
    // verdicts rest on a heading classifier and a statistic extractor, not on a
    // verified requirement: a `NotFound` here can mean the manuscript omitted
    // the item OR that the heading vocabulary missed it. An undemotable verdict
    // on a heuristic is the wrong pairing, and the condition for upgrading any
    // of them is the same one §11 D128 sets: a measured precision.
    SeverityRule {
        code: "AbstractPresent",
        severity: ReviewSeverity::Major,
        row: RiskRow::ClarityStructure,
        level: RiskLevel::Fatal,
        why: "\"missing sections (e.g. no abstract or unclear intro)\" — fatal in the \
              table, held at MAJOR because the evidence is a heading classifier",
    },
    SeverityRule {
        code: "ResultsSectionPresent",
        severity: ReviewSeverity::Major,
        row: RiskRow::ClarityStructure,
        level: RiskLevel::Fatal,
        why: "as above: the table's fatal cell names missing sections, and the evidence is \
              a heading classifier rather than a verified requirement",
    },
    SeverityRule {
        code: "SampleSizeReported",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "\"Some methodological limitations (small N …) or not fully described\" — the \
              standard asks how the sample size was determined and nothing states it",
    },
    SeverityRule {
        code: "StatisticalTestReported",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "\"not fully described\" — the standard's item asks for the statistical \
              methods and the extractor found no named test",
    },
    SeverityRule {
        code: "EffectSizeReported",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "the document: \"reports effect sizes or confidence intervals, not just \
              p-values\". Recomputable from what is already reported",
    },
    SeverityRule {
        code: "ConfidenceIntervalReported",
        severity: ReviewSeverity::Major,
        row: RiskRow::MethodologyQuality,
        level: RiskLevel::Fixable,
        why: "as above — the precision of an estimate, absent",
    },
    // ---- novelty
    SeverityRule {
        code: "PRIOR_WORK_EXISTS",
        severity: ReviewSeverity::Blocking,
        row: RiskRow::NoveltyOriginality,
        level: RiskLevel::Fatal,
        why: "\"Just repeats known results\" — and where the matching work is in the \
              manuscript's OWN bibliography, the contradiction is on the page",
    },
    SeverityRule {
        code: "novelty_claim_states_no_scope",
        severity: ReviewSeverity::Minor,
        row: RiskRow::NoveltyOriginality,
        level: RiskLevel::Fixable,
        why: "\"novelty unclear to reader\". **A PHRASING observation, not a novelty \
              verdict**: the claim asserts unrestricted priority and names no scope, which \
              is repairable by stating the scope the study has. It says nothing about \
              whether the claim is true, which is why it is MINOR and separate from \
              NOVELTY_NARROWER_THAN_STATED",
    },
    SeverityRule {
        code: "NOVELTY_NARROWER_THAN_STATED",
        severity: ReviewSeverity::Major,
        row: RiskRow::NoveltyOriginality,
        level: RiskLevel::Fixable,
        why: "\"novelty unclear to reader\" — the claim is repairable by stating the scope \
              the study actually has",
    },
];

/// What an unmapped code gets. **Major, deliberately**: an unmapped code is an
/// unreviewed one, and defaulting it to `Minor` would bury a check nobody has
/// looked at under one somebody decided was small.
pub const DEFAULT_SEVERITY: ReviewSeverity = ReviewSeverity::Major;

/// The rule for a code, if there is one.
pub fn severity_rule(code: &str) -> Option<&'static SeverityRule> {
    SEVERITY_POLICY.iter().find(|r| r.code == code)
}

// ---------------------------------------------------------------------------
// Criteria — strong / weak / action as `docs/reviewer-criteria.md` states them
// ---------------------------------------------------------------------------

/// §4.5's `Criterion`, *"from the reviewer-criteria document, each with strong /
/// weak / action as that document states them"*.
///
/// The three texts are **verbatim** from that document's *Manuscript-Level
/// Criteria*, *Methodological Rigor*, *Literature and Citation Practices*,
/// *Presentation and Writing Quality* and *Journal-Specific Compliance*
/// sections. They are quoted rather than paraphrased so a reader can diff them,
/// and so the `action` text can go into [`ReviewerReport::required_revisions`]
/// as the document's own words rather than as invented advice.
pub struct Criterion {
    pub id: &'static str,
    pub name: &'static str,
    /// The document's "Strong:" sentence.
    pub strong: &'static str,
    /// The document's "Weak:" sentence.
    pub weak: &'static str,
    /// The document's "Action:" sentence. This becomes a required revision.
    pub action: &'static str,
    /// The risk-table row this criterion is scored against.
    pub row: RiskRow,
    /// Which producers decide it — specialist reports AND research-state or
    /// fingerprint fields. See the module header for why the first alone was
    /// §4.5's defect.
    pub sources: &'static [LensSource],
    /// **The finding codes this criterion claims.** A third list beside
    /// `sources`, and it earns its place: `reporting_standards` feeds four
    /// criteria and `journal_requirements` feeds five, so routing by source
    /// alone would file one finding under two headings in one report.
    ///
    /// EMPTY IS MEANINGFUL AND IS NOT A GAP BY ITSELF. Several criteria are
    /// decided by the journal's own requirement rows, which §4.5 puts in the
    /// report's *journal-specific compliance* section rather than under a
    /// criterion; they carry no codes, and their [`CriterionOutcome`] says
    /// where their evidence went rather than leaving them looking unchecked.
    pub codes: &'static [&'static str],
}

/// §4.5's six lens ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensId {
    Methodology,
    Statistics,
    NoveltyLiterature,
    ReportingEthics,
    /// **Declared and declined.** Its layer is empty — see [`DECLINED_LENSES`].
    JournalFit,
    /// **Declared and declined.** The absence rule forbids it — see
    /// [`DECLINED_LENSES`].
    General,
}

impl LensId {
    pub fn as_str(self) -> &'static str {
        match self {
            LensId::Methodology => "methodology",
            LensId::Statistics => "statistics",
            LensId::NoveltyLiterature => "novelty_literature",
            LensId::JournalFit => "journal_fit",
            LensId::ReportingEthics => "reporting_ethics",
            LensId::General => "general",
        }
    }
}

/// §4.5's struct, with `criteria` and `severity_policy` as shared statics
/// rather than owned vectors — the policy is one table for the whole system and
/// a per-lens copy would be six things to keep in step.
pub struct ReviewLens {
    pub id: LensId,
    pub criteria: &'static [Criterion],
    /// §4.5's `reads: Vec<SpecialistId>`. Derived from the criteria rather than
    /// declared beside them: two lists of the same thing drift the first time
    /// only one is edited (§11 D129).
    pub evidence_policy: EvidencePolicy,
    /// Whether this lens reads the `JournalFingerprint`.
    pub journal_context: bool,
}

impl ReviewLens {
    /// §4.5's `reads`, computed from the criteria so it cannot disagree with
    /// them.
    pub fn reads(&self) -> Vec<LensSource> {
        let mut out: Vec<LensSource> = Vec::new();
        for c in self.criteria {
            for s in c.sources {
                if !out.contains(s) {
                    out.push(*s);
                }
            }
        }
        out.sort();
        out
    }
}

/// Shorthand so the criterion tables below stay readable.
const fn spec(s: SourceId) -> LensSource {
    LensSource::Specialist(s)
}
const fn state(f: StateField) -> LensSource {
    LensSource::State(f)
}

static METHODOLOGY_CRITERIA: &[Criterion] = &[
    Criterion {
        id: "study_design",
        name: "Study Design",
        strong: "Clear rationale and controls (e.g. control group, randomisation, blinding \
                 if needed); methods follow best practice or gold-standard techniques.",
        weak: "Flawed or missing controls; exploratory design without enough replications; \
               outdated methods when superior ones exist.",
        action: "Preempt criticisms by using robust design (power calculations, pilot \
                 data). Detail procedures thoroughly (randomisation, validation of tools). \
                 If some ideal control wasn't used, acknowledge and justify it in Discussion.",
        row: RiskRow::MethodologyQuality,
        sources: &[spec(SourceId::MlMethodology), spec(SourceId::ReportingStandards), spec(SourceId::LabWetBench)],
        codes: &["no_heldout_evaluation_named", "resampling_not_stated_as_post_split", "no_variance_across_runs", "no_baseline_comparison"],
    },
    Criterion {
        id: "reproducibility_data_availability",
        name: "Reproducibility & Data Availability",
        strong: "All data deposited in a repository (with DOI) and/or included as \
                 Supplementary files; code and parameters provided; method details \
                 sufficient for replication.",
        weak: "Manuscript only gives summary statistics; \"data available on request\" \
               statements; missing key parameters or poorly documented procedures.",
        action: "Prepare a detailed Data Availability statement following journal policy. \
                 Upload datasets (and code) to public archives (e.g. GenBank for sequences, \
                 Dryad/Zenodo for datasets, GitHub for code) with links. In Methods, ensure \
                 materials (cell lines, antibodies, software versions) are fully specified.",
        row: RiskRow::DataEthics,
        sources: &[
            state(StateField::FullText),
            spec(SourceId::JournalRequirements),
            spec(SourceId::ComputationalConsistency),
        ],
        codes: &["no_data_availability_statement"],
    },
    Criterion {
        id: "ethical_compliance",
        name: "Ethical Compliance",
        strong: "Documentation of ethics approval numbers and adherence to guidelines \
                 (Declaration of Helsinki, ARRIVE etc.).",
        weak: "No mention of ethics, or retroactive approval; missing consent forms for \
               human studies.",
        action: "Obtain and cite all necessary ethics permissions in Methods. If the study \
                 uses archival data or anonymised records, state this. Explicitly state \
                 that participants gave informed consent if applicable. Follow any \
                 field-specific reporting checklists (e.g. CONSORT, PRISMA) and mention them.",
        row: RiskRow::DataEthics,
        sources: &[
            state(StateField::FullText),
            spec(SourceId::ReportingStandards),
            spec(SourceId::JournalRequirements),
        ],
        codes: &["no_ethics_statement_in_a_study_with_subjects"],
    },
    // **Moved here from the General lens, which is declined.** §4.4 already
    // puts "is causal language earned" at Tier 2 under methodological
    // soundness, and `ClaimStrengthSpecialist::cluster` is
    // `MethodologicalSoundness`. It was in General only because the reviewer
    // document lists "Overstated Conclusions" among presentation criticisms.
    Criterion {
        id: "overstated_conclusions",
        name: "Overstated Conclusions",
        strong: "Reviewers will demand that conclusions are conservative and proportional.",
        weak: "For instance, claiming a treatment \"eliminates disease\" based on limited \
               animal data would be rejected.",
        action: "Use cautious language (\"suggests\", \"may\") if the evidence is \
                 preliminary.",
        row: RiskRow::MethodologyQuality,
        sources: &[spec(SourceId::ClaimEvidenceStrength), spec(SourceId::ValidationMaths)],
        codes: &["causal_claim_from_associational_design", "SmallSampleCausalClaim"],
    },
];

static STATISTICS_CRITERIA: &[Criterion] = &[
    Criterion {
        id: "statistical_analysis",
        name: "Statistical Analysis",
        strong: "Uses appropriate tests (e.g. t-test, ANOVA, regression) with checks \
                 (normality, variance) and reports effect sizes or confidence intervals, \
                 not just p-values.",
        weak: "P-values reported alone; misuse of tests (e.g. multiple t-tests instead of \
               ANOVA) or lack of correction for multiple comparisons.",
        action: "Consult a statistician if unsure. Include a statistics section detailing \
                 methods and thresholds. Report actual values (means, SDs, exact p-values) \
                 and indicate software or packages used.",
        row: RiskRow::MethodologyQuality,
        sources: &[
            spec(SourceId::FrequentistStats),
            spec(SourceId::ValidationMaths),
            spec(SourceId::ReportingStandards),
        ],
        codes: &["multiple_comparisons_uncorrected", "parametric_test_assumptions_unstated", "p_value_reported_as_exactly_zero", "test_claimed_but_absent_from_analysis", "TestGroupMismatch", "MissingEffectSize", "MissingConfidenceInterval"],
    },
    Criterion {
        id: "selective_reporting",
        name: "Selective Reporting",
        strong: "Reviewers expect all relevant data presented (avoiding selective \
                 reporting).",
        weak: "Over-claiming results or neglecting negative/inconclusive data is frowned \
               upon.",
        action: "Don't hide data variability—present confidence intervals and include \
                 non-significant findings in text or Supplement.",
        row: RiskRow::MethodologyQuality,
        sources: &[spec(SourceId::FrequentistStats), spec(SourceId::ValidationMaths)],
        codes: &["marginal_significance_language", "PValueOverclaim"],
    },
];

static NOVELTY_CRITERIA: &[Criterion] = &[
    Criterion {
        id: "novelty_significance",
        name: "Novelty & Significance",
        strong: "Demonstrates a gap in knowledge and addresses it (e.g. first report of a \
                 phenomenon, new mechanism).",
        weak: "Confirms known results or adds only incremental detail (\"me-too\" study).",
        action: "State the novel hypothesis/aim in the Introduction; cite very recent \
                 papers to show uniqueness. Emphasize in title/abstract how the work \
                 changes current understanding.",
        row: RiskRow::NoveltyOriginality,
        sources: &[spec(SourceId::NoveltyClaims)],
        codes: &[
            "PRIOR_WORK_EXISTS",
            "NOVELTY_NARROWER_THAN_STATED",
            "novelty_claim_states_no_scope",
        ],
    },
    Criterion {
        id: "literature_coverage",
        name: "Coverage of Relevant Literature",
        strong: "Comprehensive review of recent studies (last 3–5 years especially) and \
                 foundational works.",
        weak: "Ignoring recent publications or major groups; citing only older or \
               peripheral papers.",
        action: "Conduct thorough searches (PubMed, Scopus, Google Scholar, and specialized \
                 databases) to identify all major related studies. Add references to recent \
                 high-profile papers. Include any studies with conflicting results and \
                 discuss these.",
        row: RiskRow::LiteratureCitations,
        sources: &[spec(SourceId::LiteratureCoverage)],
        codes: &[],
    },
    Criterion {
        id: "up_to_date_references",
        name: "Up-to-Date References",
        strong: "Majority of references from the past decade, including latest articles or \
                 preprints if allowed.",
        weak: "Reliance on outdated sources (10+ years old) or irrelevant citations; missed \
               citing key recent advances.",
        action: "Double-check each section: do not say \"no one has studied this\" if \
                 someone did so recently. Use recent literature review articles and \
                 reference mining to ensure nothing is overlooked. Update the reference \
                 list after initial draft submission.",
        row: RiskRow::LiteratureCitations,
        sources: &[state(StateField::ReferenceYears)],
        codes: &["reference_list_is_mostly_outdated"],
    },
];

static JOURNAL_FIT_CRITERIA: &[Criterion] = &[
    Criterion {
        id: "scope_fit",
        name: "Scope/Fit",
        strong: "A research question of broad relevance (not merely local interest).",
        weak: "Study of only narrow/regional scope or on a trendy topic outside the \
               journal's focus.",
        action: "Choose journals carefully (use pre-submission enquiries or journal finder \
                 tools) and mention fit in the cover letter.",
        row: RiskRow::JournalCompliance,
        sources: &[spec(SourceId::ComparableCorpus)],
        codes: &[],
    },
    Criterion {
        id: "comparable_position",
        name: "Position against the comparable corpus",
        strong: "Even strong science must fit the journal's scope and audience. Evidence: \
                 Submission cover letter and content clearly address the journal's aims.",
        weak: "Topic outside journal scope, too narrow or too general for target field.",
        action: "Check author guidelines/scope statement; revise title/abstract to match \
                 field language; consider more suitable journal if out of scope.",
        row: RiskRow::JournalCompliance,
        sources: &[spec(SourceId::ComparableCorpus)],
        codes: &[],
    },
];

static REPORTING_ETHICS_CRITERIA: &[Criterion] = &[
    Criterion {
        id: "mandated_standards_met",
        name: "Reporting standards mandated by the journal",
        strong: "State compliance with reporting standards (e.g. data sharing policy, \
                 clinical trial registration number if clinical trials).",
        weak: "Omitting mandatory forms or failing to declare a conflict (including funding \
               or affiliations).",
        action: "Review the journal's ethics checklist. Complete forms (for example, ICMJE \
                 author form) if required. Declare all funding sources and personal or \
                 institutional conflicts.",
        row: RiskRow::JournalCompliance,
        sources: &[spec(SourceId::ReportingStandards)],
        codes: &["AbstractPresent", "ResultsSectionPresent", "SampleSizeReported", "StatisticalTestReported", "EffectSizeReported", "ConfidenceIntervalReported"],
    },
    Criterion {
        id: "formatting_and_style",
        name: "Formatting and Style",
        strong: "Submissions that match journal layout (sections in correct order, required \
                 sections present).",
        weak: "Wrong document class (e.g. double columns instead of single), missing \
               abstract or keywords, or neglecting required section headers.",
        action: "Download and use the journal's template or sample paper. Before \
                 submission, use the submission checklist (if provided) and confirm each \
                 requirement (format, structure, word limit) is met.",
        row: RiskRow::JournalCompliance,
        sources: &[spec(SourceId::JournalRequirements)],
        codes: &[],
    },
    Criterion {
        id: "word_page_limits",
        name: "Word/Page Limits",
        strong: "Comfortable compliance (some journals reject or return without review if \
                 exceeded).",
        weak: "Significantly overlength text or too many/oversized figures (even if \
               important).",
        action: "Trim unnecessary filler; focus writing tightly on the main story. Move \
                 extensive literature background or ancillary results to the Supplement if \
                 needed. Check that titles and abstracts also meet length rules.",
        row: RiskRow::JournalCompliance,
        sources: &[spec(SourceId::JournalRequirements)],
        codes: &[],
    },
    Criterion {
        id: "reference_style",
        name: "Reference Style",
        strong: "All references correctly formatted and numbered if needed.",
        weak: "Mixed styles, missing DOI/PMID (if required), inconsistent author listings.",
        action: "Use reference manager tools (EndNote, Zotero) with the journal's style \
                 file. Proofread the final bibliography and fix any anomalies (special \
                 characters, et al usage).",
        row: RiskRow::JournalCompliance,
        sources: &[spec(SourceId::JournalRequirements)],
        codes: &[],
    },
    Criterion {
        id: "ethics_and_disclosures",
        name: "Ethics and Disclosures",
        strong: "Clear statements about IRB approval (with reference number), informed \
                 consent, and disclosure of any commercial interests or funding sources.",
        weak: "Omitting mandatory forms or failing to declare a conflict (including funding \
               or affiliations).",
        action: "Review the journal's ethics checklist. Complete forms (for example, ICMJE \
                 author form) if required. Declare all funding sources and personal or \
                 institutional conflicts.",
        row: RiskRow::DataEthics,
        sources: &[spec(SourceId::JournalRequirements), spec(SourceId::ReportingStandards)],
        codes: &[],
    },
];

/// **The General lens is DECLINED and its criteria are kept here, unused, so
/// nobody rebuilds it without meeting the reason.**
///
/// *Clarity & Structure* and *Title/Abstract/Keywords* are read from
/// [`StateField::Sections`] and [`StateField::Title`], both `Mediated`: the
/// Conclusion heading is missed on 5 of 6 real manuscripts and the title is not
/// a title on 2 of 6. Every finding this lens could make is one
/// [`admit_absence`] rejects. Judging prose quality is a Tier 3/4 reading and
/// belongs to the synthesis cluster.
///
/// Its third criterion, *Overstated Conclusions*, had a real input and moved to
/// [`METHODOLOGY_CRITERIA`].
static DECLINED_GENERAL_CRITERIA: &[Criterion] = &[
    Criterion {
        id: "clarity_structure",
        name: "Clarity & Structure",
        strong: "Clear, well-signed structure; major points easy to identify.",
        weak: "Rambling or disordered writing, missing sections or unclear logic.",
        action: "Outline the paper before writing. Write a precise abstract/summary that \
                 states the problem, methods, results, and implications. Use subheadings \
                 and ensure each paragraph transitions smoothly.",
        row: RiskRow::ClarityStructure,
        sources: &[state(StateField::Sections)],
        codes: &[],
    },
    Criterion {
        id: "title_abstract_keywords",
        name: "Title/Abstract/Keywords",
        strong: "Title highlights the main finding; abstract is standalone and clear.",
        weak: "Title is vague or over-claims; abstract omits key data or is too general.",
        action: "Review guidelines on title/abstract length. Use compelling language for \
                 the key result (avoid clichés like \"novel\" without specifics) and \
                 include 4–6 effective keywords (often from the introduction). Check the \
                 abstract against journal criteria (word count, structure).",
        row: RiskRow::ClarityStructure,
        sources: &[state(StateField::Title)],
        codes: &[],
    },
];

/// The six lenses of §4.5.
///
/// `evidence_policy` is uniform and strict: a reviewer report is the most
/// quotable artefact this system produces, so every concern in it carries the
/// span it rests on and what it could not determine. The specialists already
/// enforce that at their own gate; repeating it here is the §34.3
/// producer-and-checker shape, not a second rule.
/// **The four lenses that ship.**
///
/// §4.5 names six. Journal fit and General are declared in [`DECLINED_LENSES`]
/// with the reason each is declined, rather than omitted — §12's table shape: a
/// node left out of the list gets rebuilt, a node listed as declined against a
/// named reason does not.
///
/// `evidence_policy` is uniform and strict: a reviewer report is the most
/// quotable artefact this system produces, so every concern carries the span it
/// rests on and what it could not determine.
pub fn lenses() -> Vec<ReviewLens> {
    let policy = || EvidencePolicy {
        min_sources: 1,
        permitted_sources: vec![
            crate::agent_graph::EvidenceSource::ManuscriptSpan,
            crate::agent_graph::EvidenceSource::JournalRequirement,
            crate::agent_graph::EvidenceSource::ExternalWork,
        ],
        citation_required: true,
        uncertainty_required: true,
    };
    vec![
        ReviewLens {
            id: LensId::Methodology,
            criteria: METHODOLOGY_CRITERIA,
            evidence_policy: policy(),
            journal_context: false,
        },
        ReviewLens {
            id: LensId::Statistics,
            criteria: STATISTICS_CRITERIA,
            evidence_policy: policy(),
            journal_context: false,
        },
        ReviewLens {
            id: LensId::NoveltyLiterature,
            criteria: NOVELTY_CRITERIA,
            evidence_policy: policy(),
            journal_context: false,
        },
        ReviewLens {
            id: LensId::ReportingEthics,
            criteria: REPORTING_ETHICS_CRITERIA,
            evidence_policy: policy(),
            journal_context: true,
        },
    ]
}

/// Why each of §4.5's other two lenses is not in [`lenses`].
pub struct DeclinedLens {
    pub id: LensId,
    /// Which criteria it would have carried, kept so the decline is checkable.
    pub criteria: &'static [Criterion],
    pub reason: &'static str,
}

/// **The two declined lenses, and they are declined for different reasons.**
pub static DECLINED_LENSES: &[DeclinedLens] = &[
    DeclinedLens {
        id: LensId::JournalFit,
        criteria: JOURNAL_FIT_CRITERIA,
        reason: "THE LAYER IS EMPTY. Both criteria read the comparable corpus. \
                 `journal_corpus::derive_conventions` and `PublishedPaper` exist; the \
                 OpenAlex fetch that fills them is in the APP crate, where `gaply_core` \
                 cannot reach it; no `journal_papers` table exists in `migrations.rs`; and \
                 the only in-core caller of `derive_conventions` is a test constructing \
                 twenty fabricated papers. A journal's scope is not extracted either — \
                 `RequirementKind` has nine variants and none is scope. A corpus job would \
                 unblock this lens without changing anything here.",
    },
    DeclinedLens {
        id: LensId::General,
        criteria: DECLINED_GENERAL_CRITERIA,
        reason: "THE ABSENCE RULE FORBIDS IT. Its two remaining criteria read \
                 `StateField::Sections` and `StateField::Title`, both `Mediated`: measured \
                 over six real manuscripts, the Conclusion heading is missed on 5 of 6 that \
                 all conclude, and the title is a Turnitin cover page or an AI-detector \
                 banner on 2 of 6. Every finding this lens could make rests on an absence \
                 that is about the extractor. Judging whether prose is \"rambling or \
                 disordered\" is a Tier 3/4 reading and belongs to the synthesis cluster. \
                 Its third criterion, Overstated Conclusions, had a real input and moved to \
                 the Methodology lens. Unlike journal fit, NO data path would unblock this \
                 one: it needs a judge, not a corpus.",
    },
];

// ---------------------------------------------------------------------------
// The measurement: which lenses have inputs
// ---------------------------------------------------------------------------

/// Whether a lens has anything to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensState {
    /// Every criterion has at least one shipping source.
    Runs,
    /// Some criteria have a shipping source and some do not. The report names
    /// the unread ones rather than omitting them.
    RunsPartial,
    /// No criterion has a shipping source. The lens is declared and does not
    /// run.
    NoInputs,
}

/// One lens's inputs, measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LensAvailability {
    pub lens: LensId,
    pub state: LensState,
    /// Criteria with at least one shipping source.
    pub sourced_criteria: Vec<String>,
    /// Criteria with none, each with the reason its sources are absent.
    pub unsourced_criteria: Vec<(String, String)>,
    /// §4.5's `reads`, widened, with each source's state.
    pub reads: Vec<(LensSource, SourceState)>,
}

/// **Measure, for every lens, what it declares and what exists.**
///
/// Static: this is a property of the graph of sources, not of any manuscript.
/// The per-run question — *did the source that exists actually produce
/// anything* — is [`review`]'s, and the two are kept apart because "ran and
/// found nothing" and "never ran" are the distinction the journal phase's four
/// zeros turned on.
pub fn availability() -> Vec<LensAvailability> {
    lenses()
        .iter()
        .map(|l| {
            let mut sourced = Vec::new();
            let mut unsourced = Vec::new();
            for c in l.criteria {
                if c.sources.iter().any(|s| s.state() == SourceState::Shipping) {
                    sourced.push(c.name.to_string());
                } else {
                    let why = c
                        .sources
                        .iter()
                        .map(|s| format!("{} — {}", s.as_str(), s.note()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    unsourced.push((c.name.to_string(), why));
                }
            }
            let state = match (sourced.is_empty(), unsourced.is_empty()) {
                (true, _) => LensState::NoInputs,
                (false, true) => LensState::Runs,
                (false, false) => LensState::RunsPartial,
            };
            LensAvailability {
                lens: l.id,
                state,
                sourced_criteria: sourced,
                unsourced_criteria: unsourced,
                reads: l.reads().into_iter().map(|s| (s, s.state())).collect(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The reviewer report
// ---------------------------------------------------------------------------

/// One step of a concern's verification trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrailStep {
    /// What kind of step: `source`, `evidence`, `criterion`, `severity`.
    pub stage: String,
    pub detail: String,
}

/// §4.5's *"major concerns (each with its verification trail)"*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Concern {
    pub criterion: String,
    pub source: LensSource,
    pub code: String,
    pub severity: ReviewSeverity,
    pub summary: String,
    /// **What [`Self::occurrences`] counts.** §11 D161: when you print N, say
    /// what N counts. For a reporting-standard code it counts STANDARD ITEMS
    /// asking for the same manuscript property, not places in the manuscript —
    /// six standards each asking for a named statistical test is one property,
    /// asked six times.
    pub occurrence_unit: String,
    /// **How many times this check fired under this criterion.**
    ///
    /// Grouped, because a reviewer writes *"effect sizes are absent at eight
    /// places"* and not eight concerns. Ungrouped, this manuscript produced 15
    /// major concerns of which 11 were two codes repeating — a report a
    /// researcher would stop reading before reaching the one that differed.
    pub occurrences: usize,
    /// **Every manuscript sentence this rests on, each whole.** Grouping must
    /// not cost the spans: a concern that says "eight places" and quotes one of
    /// them is a count where the rows were.
    pub spans: Vec<String>,
    pub locations: Vec<Location>,
    pub trail: Vec<TrailStep>,
    /// What the underlying check could not determine. Carried up unchanged —
    /// a reviewer report that drops the uncertainty presents a heuristic as a
    /// verdict.
    pub uncertainty: Option<String>,
    /// The document's own "Action:" for this criterion.
    pub required_revision: String,
}

/// §4.5: *"an independent reviewer report in the shape a real one takes:
/// overall assessment, contribution summary, strengths, major concerns (each
/// with its verification trail), minor concerns, journal-specific compliance,
/// required revisions."*
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewerReport {
    pub lens: LensId,
    pub overall_assessment: String,
    /// **Not a summary of the contribution.**
    ///
    /// §4.5 asks for one. Summarising what a paper contributes is §4.4's Tier 4
    /// — the editorial reading — and a deterministic lens that produced one
    /// would be asserting a judgement nothing made. This field states what the
    /// lens can see of the contribution and says the judgement is absent.
    pub contribution: String,
    pub strengths: Vec<String>,
    pub major_concerns: Vec<Concern>,
    pub minor_concerns: Vec<Concern>,
    pub journal_compliance: Vec<String>,
    pub required_revisions: Vec<String>,
    /// **What happened to every criterion this lens declares** — see
    /// [`CriterionOutcome`]. An addition to §4.5's seven sections, for the same
    /// reason `Unevaluable` is a third state beside passed and failed.
    pub criteria: Vec<CriterionOutcome>,
    /// Set when the lens has no shipping source at all.
    pub not_applicable: Option<String>,
    /// **Findings the absence rule refused.** Returned, never dropped: a gate
    /// that silently swallows what it rejects is indistinguishable from a lens
    /// that found nothing. This is where the D157 ratio becomes visible —
    /// refusals beside findings, in the same report.
    pub absence_rejected: Vec<AbsenceRejected>,
    /// **Concerns the lens's own `evidence_policy` refused**, with the reason.
    ///
    /// §4.3: *"An opinion emitted without its policy satisfied is rejected at
    /// the gate with the reason recorded."* The lens carried an
    /// `evidence_policy` field and **never applied it** — declared and
    /// unenforced, which §4.3 had already recorded once as *"a policy nothing
    /// enforces is a comment"*. The consequence was visible on the first real
    /// editor run: a MAJOR concern reached the rubric's primary causes with no
    /// span, from a reporting-standard absence.
    pub policy_rejected: Vec<String>,
}

/// Everything the lenses read. One struct so adding a source is a change here
/// and not at six call sites.
///
/// **Every collection is an `Option`, and that is the load-bearing part.**
/// `Some(&[])` is *the producer ran and found nothing*; `None` is *it was never
/// run*. Flattening them into empty slices would make a lens that never ran
/// indistinguishable from a clean bill of health — the failure the journal
/// phase's four zeros recorded, and the one `SpecialistReport::not_applicable`
/// exists to prevent one layer down.
pub struct LensInput<'a> {
    pub extraction: &'a ExtractionResult,
    /// **The manuscript text as `docparse` produced it.** The one exhaustive
    /// field: nothing classifies it, so a phrase absent from it is absent from
    /// the manuscript. `None` means the caller did not supply it, and every
    /// full-text check then declines rather than concluding from nothing.
    pub full_text: Option<&'a str>,
    /// The current year, INJECTED. A deterministic lens with a clock in it is
    /// not reproducible, and this crate's norm is injected time.
    pub this_year: i32,
    pub specialists: Option<&'a [SpecialistReport]>,
    pub validity: Option<&'a StatsValidityReport>,
    pub standards: Option<&'a [StandardEvaluation]>,
    pub checklist: Option<&'a [ChecklistItem]>,
    pub novelty: Option<&'a [NoveltyAssessment]>,
}

impl LensInput<'_> {
    // `was_run` lived here and is gone with `had_subject_matter`: `executed`
    // answers both questions exactly. See the comment in `review`.
}

/// **May a finding rest on this field being EMPTY?**
///
/// The single predicate the whole §4.5 correction turns on, and the only thing
/// between a lens reading `ResearchState` directly and §11 D165. `Ok(())` to
/// admit; `Err(reason)` to refuse, and the reason is recorded in
/// [`ReviewerReport::absence_rejected`] rather than dropped.
pub fn admit_absence(field: StateField) -> Result<(), String> {
    match field.reach() {
        FieldReach::Exhaustive => Ok(()),
        FieldReach::Mediated => Err(format!(
            "this finding rests on `{}` being empty, and that field is MEDIATED: {}. An \
             absence there is a fact about the extractor, not about the manuscript.",
            field.as_str(),
            field.absence_note()
        )),
    }
}

// `self_had` and `LensInput::had_subject_matter` lived here and are gone.
// `executed` answers the same question exactly rather than by source-level
// approximation — see the comment in `review`. They were removed rather than
// left unused, because a helper nothing calls is a rule a reader will think is
// enforced.

fn trail(source: LensSource, criterion: &str, rule: Option<&SeverityRule>, code: &str) -> Vec<TrailStep> {
    let mut steps = vec![
        TrailStep {
            stage: "source".into(),
            detail: match source {
                LensSource::Specialist(x) => format!(
                    "The `{}` specialist produced the finding `{code}`. It is {}.",
                    x.as_str(),
                    x.note()
                ),
                // A research-state field is read, not run, and its note is a
                // clause about what reading it can establish — so the sentence
                // around it differs. Sharing one template printed "It is reasons
                // only over reference years that WERE parsed".
                LensSource::State(f) => format!(
                    "Read directly from the research state's `{}` field, which {}.",
                    f.as_str(),
                    f.absence_note()
                ),
            },
        },
        TrailStep {
            stage: "criterion".into(),
            detail: format!(
                "Mapped to the reviewer document's criterion \"{criterion}\"."
            ),
        },
    ];
    steps.push(match rule {
        Some(r) => TrailStep {
            stage: "severity".into(),
            detail: format!(
                "{} — the Risk Assessment Table's \"{}\" row, {} cell: \"{}\". {}",
                r.severity.as_str(),
                r.row.band().label,
                match r.level {
                    RiskLevel::Fatal => "High Risk (Fatal Flaws)",
                    RiskLevel::Fixable => "Medium Risk (Fixable)",
                },
                match r.level {
                    RiskLevel::Fatal => r.row.band().fatal,
                    RiskLevel::Fixable => r.row.band().fixable,
                },
                r.why
            ),
        },
        None => TrailStep {
            stage: "severity".into(),
            detail: format!(
                "{} by default: `{code}` has no rule in the severity policy, so nobody has \
                 decided where it sits in the risk table. The default is deliberately not \
                 the lowest level.",
                DEFAULT_SEVERITY.as_str()
            ),
        },
    });
    steps
}

/// What happened to one criterion on one manuscript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CriterionState {
    /// Its checks ran. `concerns == 0` then means the manuscript passed them.
    Evaluated,
    /// A shipping source exists and was not run on this manuscript. **Not the
    /// same as passing**, and the reason it is a separate state.
    SourceNotRun { detail: String },
    /// No shipping source at all — [`availability`]'s finding, per manuscript.
    NoShippingSource { detail: String },
    /// Decided by the journal's own requirement rows, which §4.5 puts in the
    /// *journal-specific compliance* section rather than under a criterion.
    ReportedInComplianceSection { detail: String },
}

/// One criterion's outcome. **An addition to §4.5's report shape, not part of
/// it** — §4.5 lists seven sections and none of them says what the reviewer
/// did not look at. A reviewer report that lists only its concerns reads as a
/// complete review of everything, which is the same defect as `Unevaluable`
/// rendering as passed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionOutcome {
    pub criterion: String,
    pub state: CriterionState,
    pub concerns: usize,
}

/// **Run one lens over what the specialists produced.**
///
/// Deterministic: no model, no network. Every concern carries the span its
/// source carried, the uncertainty its source stated, and a trail naming the
/// source, the criterion and the risk-table cell that set its severity.
pub fn review(lens: &ReviewLens, input: &LensInput<'_>) -> ReviewerReport {
    let mut report = ReviewerReport {
        lens: lens.id,
        overall_assessment: String::new(),
        contribution: String::new(),
        strengths: Vec::new(),
        major_concerns: Vec::new(),
        minor_concerns: Vec::new(),
        journal_compliance: Vec::new(),
        required_revisions: Vec::new(),
        criteria: Vec::new(),
        not_applicable: None,
        absence_rejected: Vec::new(),
        policy_rejected: Vec::new(),
    };

    // A lens with no shipping source at all declares itself rather than
    // producing an empty report that reads as a clean bill of health.
    if !lens
        .reads()
        .iter()
        .any(|s| s.state() == SourceState::Shipping)
    {
        let missing: Vec<String> = lens
            .reads()
            .iter()
            .map(|s| format!("{} ({})", s.as_str(), s.note()))
            .collect();
        report.not_applicable = Some(format!(
            "This lens reads only sources that do not exist, so it has nothing to review: \
             {}. An empty report from it would say the manuscript passed checks nobody ran.",
            missing.join("; ")
        ));
        report.overall_assessment = report.not_applicable.clone().unwrap();
        report.contribution =
            "Not assessed: this lens did not run.".into();
        for c in lens.criteria {
            report.criteria.push(CriterionOutcome {
                criterion: c.name.to_string(),
                state: CriterionState::NoShippingSource {
                    detail: c
                        .sources
                        .iter()
                        .map(|s| format!("{} — {}", s.as_str(), s.note()))
                        .collect::<Vec<_>>()
                        .join("; "),
                },
                concerns: 0,
            });
        }
        return report;
    }

    let Collected { findings: all, refusals, executed } = collect(input);

    for c in lens.criteria {
        let shipping: Vec<LensSource> = c
            .sources
            .iter()
            .copied()
            .filter(|s| s.state() == SourceState::Shipping)
            .collect();

        if shipping.is_empty() {
            report.criteria.push(CriterionOutcome {
                criterion: c.name.to_string(),
                state: CriterionState::NoShippingSource {
                    detail: c
                        .sources
                        .iter()
                        .map(|s| format!("{} — {}", s.as_str(), s.note()))
                        .collect::<Vec<_>>()
                        .join("; "),
                },
                concerns: 0,
            });
            continue;
        }

        if c.codes.is_empty() {
            report.criteria.push(CriterionOutcome {
                criterion: c.name.to_string(),
                state: CriterionState::ReportedInComplianceSection {
                    detail: format!(
                        "decided by the journal's own requirement rows ({}); those appear \
                         under journal-specific compliance below, not as a concern here",
                        shipping.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                    ),
                },
                concerns: 0,
            });
            continue;
        }

        // **`executed` decides this, not the sources.**
        //
        // A source-level check was an approximation and it was wrong twice: it
        // counted a declined specialist as having run, and it required subject
        // matter from sources that contribute no code to this criterion — so
        // the ethics criterion could not be evaluated without a journal crawl
        // that decides nothing about ethics.
        //
        // `executed` is exact: a code is in it only if the thing that produces
        // it ran, had something to look at, and was not turned away by its own
        // gate. Everything the old check approximated, it now answers.
        let ran: Vec<&&str> =
            c.codes.iter().filter(|x| executed.iter().any(|e| e == *x)).collect();
        let skipped: Vec<&&str> =
            c.codes.iter().filter(|x| !executed.iter().any(|e| e == *x)).collect();

        let mine: Vec<&Raw> = all
            .iter()
            .filter(|r| c.codes.contains(&r.code.as_str()) && c.sources.contains(&r.source))
            .collect();

        // One concern per CODE, carrying every occurrence and every span.
        let mut codes_seen: Vec<&str> = Vec::new();
        for r in &mine {
            if codes_seen.contains(&r.code.as_str()) {
                continue;
            }
            codes_seen.push(&r.code);
            let same: Vec<&&Raw> = mine.iter().filter(|x| x.code == r.code).collect();
            let rule = severity_rule(&r.code);
            let severity = rule.map(|x| x.severity).unwrap_or(DEFAULT_SEVERITY);
            let spans: Vec<String> = same.iter().filter_map(|x| x.span.clone()).collect();
            let concern = Concern {
                criterion: c.name.to_string(),
                source: r.source,
                code: r.code.clone(),
                severity,
                summary: if same.len() == 1 {
                    r.summary.clone()
                } else {
                    format!("{} (raised at {} places; all are quoted)", r.summary, same.len())
                },
                occurrence_unit: occurrence_unit(r.source).to_string(),
                occurrences: same.len(),
                spans: spans.clone(),
                locations: same.iter().filter_map(|x| x.location.clone()).collect(),
                trail: {
                    let mut t = trail(r.source, c.name, rule, &r.code);
                    if !spans.is_empty() {
                        t.insert(
                            1,
                            TrailStep {
                                stage: "evidence".into(),
                                detail: format!(
                                    "The manuscript's own sentence(s), {} of them: {}",
                                    spans.len(),
                                    spans.join("  ||  ")
                                ),
                            },
                        );
                    }
                    t
                },
                uncertainty: r.uncertainty.clone(),
                required_revision: c.action.to_string(),
            };
            if matches!(severity, ReviewSeverity::Blocking | ReviewSeverity::Major) {
                report.major_concerns.push(concern);
            } else {
                report.minor_concerns.push(concern);
            }
        }

        let concerns = mine.len();
        let state = if skipped.is_empty() {
            CriterionState::Evaluated
        } else {
            CriterionState::SourceNotRun {
                detail: format!(
                    "{} did not run on this manuscript — the producer declined, had nothing \
                     to examine, or its own gate turned it away — so nothing here is either \
                     a concern or a strength",
                    skipped.iter().map(|x| **x).collect::<Vec<_>>().join(", ")
                ),
            }
        };

        // **The risk table's third column, used where it belongs.** A strength
        // requires every one of this criterion's checks to have EXECUTED and
        // none to have fired. The sentence REPORTS rather than asserts: the
        // compliant cell describes the risk-table ROW, three of which cover more
        // than one criterion, so quoting it as a finding about this criterion
        // would claim more than was measured.
        if concerns == 0 && matches!(state, CriterionState::Evaluated) {
            report.strengths.push(format!(
                "{}: {} of this criterion's check(s) ran — {} — reading {}, and none fired. \
                 The Risk Assessment Table's low-risk description of the \"{}\" row is \
                 \"{}\" — this is what the checks that ran did not contradict, not a \
                 positive finding that the description holds.",
                c.name,
                ran.len(),
                ran.iter().map(|x| **x).collect::<Vec<_>>().join(", "),
                shipping.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                c.row.band().label,
                c.row.band().compliant,
            ));
        }

        report.criteria.push(CriterionOutcome {
            criterion: c.name.to_string(),
            state,
            concerns,
        });
    }

    // The journal's own requirements, in the section §4.5 gives them.
    if lens.journal_context {
        match input.checklist {
            Some(items) => {
                for i in items {
                    let verdict = if i.unevaluable {
                        "UNEVALUABLE"
                    } else if i.passed {
                        "OK"
                    } else {
                        "FLAG"
                    };
                    report.journal_compliance.push(format!(
                        "[{verdict}] {}{}",
                        i.requirement,
                        i.source_span
                            .as_deref()
                            .map(|s| format!("\n    the journal's own sentence: {s}"))
                            .unwrap_or_default()
                    ));
                }
            }
            None => report.journal_compliance.push(
                "No journal requirements were ingested for this run, so nothing here is a \
                 statement about any journal. An empty compliance section is not compliance."
                    .into(),
            ),
        }
        if let Some(evals) = input.standards {
            for e in evals {
                report
                    .journal_compliance
                    .push(format!("[{}] {}", e.standard.as_str(), e.coverage_phrase()));
            }
        }
    }

    report.required_revisions = report
        .major_concerns
        .iter()
        .chain(report.minor_concerns.iter())
        .map(|c| c.required_revision.clone())
        .fold(Vec::new(), |mut acc, r| {
            if !acc.contains(&r) {
                acc.push(r);
            }
            acc
        });

    // **The lens's own evidence policy, applied.** `citation_required` is the
    // half that bites: a concern with no span asks the reader to take it on
    // trust, and a reviewer report is the last artefact that should.
    let policy = &lens.evidence_policy;
    if policy.citation_required {
        let refuse = |v: &mut Vec<Concern>, out: &mut Vec<String>| {
            v.retain(|c| {
                if c.spans.is_empty() {
                    out.push(format!(
                        "{} / {}: refused — this lens requires every concern to quote what it \
                         rests on, and this one carried nothing",
                        c.criterion, c.code
                    ));
                    false
                } else {
                    true
                }
            });
        };
        let mut refused = Vec::new();
        refuse(&mut report.major_concerns, &mut refused);
        refuse(&mut report.minor_concerns, &mut refused);
        report.policy_rejected = refused;
    }

    for (code, field, reason) in &refusals {
        let criterion = lens
            .criteria
            .iter()
            .find(|c| c.codes.contains(&code.as_str()))
            .map(|c| c.name.to_string());
        if let Some(criterion) = criterion {
            report.absence_rejected.push(AbsenceRejected {
                criterion,
                code: code.clone(),
                field: *field,
                reason: reason.clone(),
            });
        }
    }

    report.overall_assessment = assessment(&report);
    report.contribution = contribution(lens, input);
    report
}

fn assessment(r: &ReviewerReport) -> String {
    let blocking = r.major_concerns.iter().filter(|c| c.severity == ReviewSeverity::Blocking).count();
    let major = r.major_concerns.len() - blocking;
    let unevaluated = r
        .criteria
        .iter()
        .filter(|c| !matches!(c.state, CriterionState::Evaluated))
        .count();
    format!(
        "Under this lens: {blocking} blocking, {major} major and {} minor concern(s) across \
         {} criterion/criteria that were evaluated. {unevaluated} of the {} criteria this \
         lens declares were NOT evaluated and are listed with their reasons. {} finding(s) \
         were REFUSED by the absence rule and are listed with theirs. **No recommendation \
         is made here**: Accept / Minor / Major / Reject is the editor layer's output \
         (§5.7), which weighs every lens together and is not built.",
        r.minor_concerns.len(),
        r.criteria.len() - unevaluated,
        r.criteria.len(),
        r.absence_rejected.len()
    )
}

/// §4.5 asks each lens for a *"contribution summary"*.
///
/// **It cannot produce one and does not pretend to.** Summarising what a paper
/// contributes is §4.4's Tier 4 — the editorial reading, cloud, synthesis
/// cluster. A deterministic lens emitting a sentence about the contribution
/// would be a judgement nothing made, dressed as one a reviewer made. So this
/// states what is visible and names the absence.
fn contribution(_lens: &ReviewLens, input: &LensInput<'_>) -> String {
    let title = input
        .extraction
        .title
        .as_deref()
        .map(|t| {
            let t = t.trim();
            if t.chars().count() > 120 {
                format!("{}…", t.chars().take(120).collect::<String>())
            } else {
                t.to_string()
            }
        })
        .unwrap_or_else(|| "(no title extracted)".into());
    let novelty = input
        .novelty
        .map(|n| {
            if n.is_empty() {
                "The manuscript states no novelty claim this system recognises.".to_string()
            } else {
                format!(
                    "The manuscript states {} novelty claim(s), assessed separately.",
                    n.len()
                )
            }
        })
        .unwrap_or_else(|| "Novelty claims were not extracted for this run.".to_string());
    format!(
        "Title as extracted: {title}. {novelty} **What the contribution amounts to is not \
         judged here** — that is the synthesis cluster's output (§4.4 Tier 4), and a \
         deterministic lens asserting it would be inventing a judgement."
    )
}

/// A finding flattened out of whichever producer made it.
struct Raw {
    source: LensSource,
    code: String,
    summary: String,
    span: Option<String>,
    location: Option<Location>,
    uncertainty: Option<String>,
}

type Refusal = (String, StateField, String);

/// What `collect` produced: findings, refusals, and **which codes actually
/// executed**.
///
/// The third is not bookkeeping. A check with its own gate — the ethics check
/// declines unless the manuscript shows human or animal subjects — is SKIPPED
/// on a desk study, and a skipped check produces no finding, which is
/// indistinguishable from a check that ran and was satisfied. Without this the
/// report claimed *"full ethical disclosure"* as a strength of a manuscript
/// whose ethics check never ran. Third instance of the same shape in this
/// module, each one layer further down: a declined specialist, a check with no
/// subject matter, and now a check its own gate turned away.
/// The first whole sentence of the manuscript containing any of `needles`.
///
/// Reads the extracted sections rather than the raw text so the result is a
/// SENTENCE a reader can check, not a character window cut mid-word.
fn sentence_containing(r: &ExtractionResult, needles: &[&str]) -> Option<String> {
    for section in &r.sections {
        for para in &section.paragraphs {
            for sent in crate::extract::sentence::sentences_in(para) {
                let sl = sent.to_lowercase();
                if needles.iter().any(|n| sl.contains(*n)) {
                    return Some(sent.trim().to_string());
                }
            }
        }
    }
    None
}

struct Collected {
    findings: Vec<Raw>,
    refusals: Vec<Refusal>,
    executed: Vec<String>,
}

fn collect(input: &LensInput<'_>) -> Collected {
    let mut out = Vec::new();
    let mut rejected: Vec<Refusal> = Vec::new();
    let mut executed: Vec<String> = Vec::new();

    if let Some(reports) = input.specialists {
        for r in reports {
            let source = match r.specialist.as_str() {
                "frequentist_stats" => SourceId::FrequentistStats,
                "ml_methodology" => SourceId::MlMethodology,
                "claim_evidence_strength" => SourceId::ClaimEvidenceStrength,
                _ => continue,
            };
            // A specialist that ran — `not_applicable` unset — executed all of
            // its checks, whether or not any fired.
            if r.not_applicable.is_none() {
                executed.extend(codes_of(source).iter().map(|c| c.to_string()));
            }
            for f in &r.admitted {
                out.push(Raw {
                    source: LensSource::Specialist(source),
                    code: f.code.clone(),
                    summary: f.summary.clone(),
                    span: f.span.clone(),
                    location: f.location.clone(),
                    uncertainty: f.uncertainty.clone(),
                });
            }
        }
    }

    if let Some(v) = input.validity {
        executed.extend(v.checks.iter().map(|c| format!("{:?}", c.rule)));
        for f in &v.flags {
            out.push(Raw {
                source: LensSource::Specialist(SourceId::ValidationMaths),
                code: format!("{:?}", f.rule),
                summary: f.explanation.clone(),
                // **Resolved from the location rather than left empty.** A Tier-0
                // finding overrides model consensus (§4.4); it is the LAST place
                // a reader should have to take a verdict on trust. `None` when
                // the location does not resolve — never a wrong sentence.
                span: crate::extract::paragraph_at(input.extraction, &f.location)
                    .map(str::to_string),
                location: Some(f.location.clone()),
                uncertainty: Some(
                    "A Tier-0 deterministic rule (§4.4). It overrides model consensus and \
                     cannot be demoted; what it does not do is judge whether the design \
                     that produced the number was appropriate."
                        .into(),
                ),
            });
        }
    }

    if let Some(assessments) = input.novelty {
        if !assessments.is_empty() {
            executed.push("PRIOR_WORK_EXISTS".into());
            executed.push("NOVELTY_NARROWER_THAN_STATED".into());
            executed.push("novelty_claim_states_no_scope".into());
        }
        for a in assessments {
            // **The status.** Only the two that rest on a named work reach the
            // report as a concern; `UNVERIFIED` is the common outcome and is
            // not a finding about the manuscript.
            if matches!(
                a.status,
                NoveltyStatus::PriorWorkExists | NoveltyStatus::NoveltyNarrowerThanStated
            ) {
                let work = a
                    .nearest_prior_work
                    .as_ref()
                    .map(|w| format!("\n  PRIOR WORK: {}\n  WHAT IT SHOWED: {}", w.cite(), w.what_it_showed()))
                    .unwrap_or_default();
                out.push(Raw {
                    source: LensSource::Specialist(SourceId::NoveltyClaims),
                    code: a.status.as_str().to_string(),
                    summary: format!(
                        "{}{work}",
                        a.narrowing.clone().unwrap_or_else(|| {
                            "A work carrying every content term of this novelty claim \
                             exists."
                                .into()
                        })
                    ),
                    span: Some(a.claim.sentence.clone()),
                    location: Some(a.claim.location.clone()),
                    uncertainty: Some(a.uncertainty.clone()),
                });
            }
            // **The phrasing observation, separately.** It travels whatever the
            // status is, including UNVERIFIED, because how a claim is worded is
            // a fact about the manuscript that retrieval has no bearing on.
            if let Some(note) = &a.states_no_scope {
                out.push(Raw {
                    source: LensSource::Specialist(SourceId::NoveltyClaims),
                    code: "novelty_claim_states_no_scope".into(),
                    summary: note.clone(),
                    span: Some(a.claim.sentence.clone()),
                    location: Some(a.claim.location.clone()),
                    uncertainty: Some(
                        "Read from the claim sentence alone. A scope stated in the \
                         surrounding sentences rather than in this one reads here as no \
                         scope."
                            .into(),
                    ),
                });
            }
        }
    }

    // ---- Research-state checks, each through the absence gate.
    //
    // **This is the §4.5 correction doing its work**, and it is also where a
    // lens is closest to fabricating: nothing between the manuscript and a
    // finding but the rule below.
    if let Some(text) = input.full_text {
        let lower = text.to_lowercase();
        for check in METHODOLOGY_STATE_CHECKS {
            // The ethics check declines unless the manuscript shows subjects —
            // no gate, and every desk study is flagged for lacking an ethics
            // statement it never needed.
            let subject_sentence = if check.code == ETHICS_CHECK.code {
                sentence_containing(input.extraction, SUBJECTS_PRESENT)
            } else {
                None
            };
            if check.code == ETHICS_CHECK.code && subject_sentence.is_none() {
                // Turned away by its own gate. NOT recorded as executed: a
                // manuscript that owes no ethics statement has not demonstrated
                // full ethical disclosure by not needing one.
                continue;
            }
            executed.push(check.code.to_string());
            if check.phrases.iter().any(|t| lower.contains(*t)) {
                continue;
            }
            match admit_absence(check.field) {
                Ok(()) => out.push(Raw {
                    source: LensSource::State(check.field),
                    code: check.code.to_string(),
                    summary: check.absent_summary.to_string(),
                    // **An absence has no span, and inventing one is worse than
                    // carrying none.** The finding quotes the manuscript by
                    // naming what was searched for instead.
                    // **An absence has no sentence of its own, so the finding
                    // quotes the sentence that made the check APPLY** plus what
                    // it searched for. Without the first half, the ethics
                    // finding asserted that the manuscript studies subjects and
                    // showed nothing — and the sentence it rested on was, on the
                    // first live run, a metaphor about wetlands.
                    span: Some(format!(
                        "{}Searched the whole manuscript for {} phrasings and found none of \
                         them: {}",
                        subject_sentence
                            .as_deref()
                            .map(|s| format!("WHY THIS CHECK APPLIES: {s}\n"))
                            .unwrap_or_default(),
                        check.phrases.len(),
                        check.phrases.join(", ")
                    )),
                    location: None,
                    uncertainty: Some(check.uncertainty.to_string()),
                }),
                Err(reason) => rejected.push((check.code.to_string(), check.field, reason)),
            }
        }
    }

    // ---- Reference currency, over the years of references that WERE parsed.
    {
        let c = reference_currency(&input.extraction.references, input.this_year);
        if c.majority_outdated().is_some() {
            executed.push("reference_list_is_mostly_outdated".to_string());
        }
        if let Some(true) = c.majority_outdated() {
            out.push(Raw {
                source: LensSource::State(StateField::ReferenceYears),
                code: "reference_list_is_mostly_outdated".to_string(),
                summary: format!(
                    "{} of {} dated references are from the last {} years — a minority. The \
                     reviewer document's weak case is \"Reliance on outdated sources (10+ \
                     years old)\".",
                    c.recent, c.dated, RECENT_WINDOW_YEARS
                ),
                span: Some(format!(
                    "{} reference(s) parsed, {} carrying a year, spanning {}–{}. The \
                     denominator is the dated ones, not the total.",
                    c.total,
                    c.dated,
                    c.oldest.map(|y| y.to_string()).unwrap_or_else(|| "?".into()),
                    c.newest.map(|y| y.to_string()).unwrap_or_else(|| "?".into())
                )),
                location: None,
                uncertainty: Some(format!(
                    "Reasons only over references the parser dated: {} of {} carry a year. \
                     A field where a decade-old paper is still current reads as outdated \
                     here, and the window is the reviewer document's, not this field's.",
                    c.dated, c.total
                )),
            });
        }
    }

    if let Some(evals) = input.standards {
        for e in evals {
            for v in &e.verdicts {
                if v.status != crate::report::ItemStatus::Unevaluable {
                    // The engine looked at this item, whatever it concluded.
                    let c = check_code(v);
                    if !executed.contains(&c) {
                        executed.push(c);
                    }
                }
                if v.status != crate::report::ItemStatus::NotFound {
                    continue;
                }
                out.push(Raw {
                    source: LensSource::Specialist(SourceId::ReportingStandards),
                    code: check_code(v),
                    summary: format!(
                        "{} item {}: {} — {}",
                        e.standard.as_str(),
                        v.item,
                        v.requirement,
                        v.detail
                    ),
                    // **An absence has no sentence, so it quotes what was looked
                    // for** — the same shape the full-text checks use. Without
                    // this a `NotFound` verdict reached the editor's primary
                    // causes carrying no evidence at all, while the lens's own
                    // `evidence_policy` declared `citation_required: true`.
                    span: v
                        .evidence_span
                        .clone()
                        .or_else(|| Some(format!("WHAT WAS LOOKED FOR: {}", v.detail))),
                    location: v.location.clone(),
                    uncertainty: Some(format!(
                        "The engine {}. Whether this item's verdict matches a human \
                         reviewer's is unmeasured.",
                        e.coverage_phrase()
                    )),
                });
            }
        }
    }

    executed.sort();
    executed.dedup();
    Collected { findings: out, refusals: rejected, executed }
}

/// The codes a specialist can emit. **A second list beside
/// `SHIPPED_CODES`, and it must stay in step** — pinned by
/// `every_specialist_code_is_attributed_to_its_specialist`.
fn codes_of(s: SourceId) -> &'static [&'static str] {
    match s {
        SourceId::FrequentistStats => &[
            "p_value_reported_as_exactly_zero",
            "multiple_comparisons_uncorrected",
            "parametric_test_assumptions_unstated",
            "marginal_significance_language",
            "test_claimed_but_absent_from_analysis",
        ],
        SourceId::MlMethodology => &[
            "resampling_not_stated_as_post_split",
            "no_heldout_evaluation_named",
            "no_baseline_comparison",
            "no_variance_across_runs",
        ],
        SourceId::ClaimEvidenceStrength => &["causal_claim_from_associational_design"],
        _ => &[],
    }
}

/// The code a standard item's verdict is routed by — its CHECK, not its item
/// number. `CONSORT 7a` and `ARRIVE 2a` are the same question about the same
/// manuscript field, and routing by item number would file them under different
/// criteria.
/// What an occurrence count means, per source.
///
/// **Measured, and it is why this exists.** On 11 of 20 real manuscripts the
/// editor's top primary cause was `StatisticalTestReported x6` — the same
/// conclusion ("no named statistical test was found") reached once per bound
/// standard. Six standards asking for one property is not six problems, and
/// ordering causes by a raw occurrence count put that repetition ahead of
/// `MissingEffectSize x42`, which really is 42 places in the manuscript.
pub fn occurrence_unit(source: LensSource) -> &'static str {
    match source {
        LensSource::Specialist(SourceId::ReportingStandards) => {
            "standard items asking for the same manuscript property — not places in the \
             manuscript"
        }
        LensSource::Specialist(SourceId::JournalRequirements) => "journal requirements",
        LensSource::State(StateField::FullText) => "phrase searches over the whole manuscript",
        LensSource::State(StateField::ReferenceYears) => "reference lists",
        _ => "places in the manuscript",
    }
}

fn check_code(v: &crate::report::ItemVerdict) -> String {
    // The reads list is the check's own declaration of what it looked at.
    match v.reads {
        r if r.contains(&"statistics") && v.requirement.to_lowercase().contains("sample size") => {
            "SampleSizeReported".into()
        }
        r if r.contains(&"statistics")
            && (v.requirement.to_lowercase().contains("effect size")) =>
        {
            "EffectSizeReported".into()
        }
        r if r.contains(&"statistics")
            && v.requirement.to_lowercase().contains("confidence interval") =>
        {
            "ConfidenceIntervalReported".into()
        }
        r if r.contains(&"structure") => {
            if v.requirement.to_lowercase().contains("summary") {
                "AbstractPresent".into()
            } else {
                "ResultsSectionPresent".into()
            }
        }
        _ => "StatisticalTestReported".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specialist::{self, SpecialistInput};

    /// Every finding code any shipped producer can emit. Kept here rather than
    /// derived, because the point of the two tests below is that the lists
    /// cannot be derived from each other — if they could, they would agree by
    /// construction and prove nothing (§11's derivation entry).
    const SHIPPED_CODES: &[&str] = &[
        // frequentist_stats
        "p_value_reported_as_exactly_zero",
        "multiple_comparisons_uncorrected",
        "parametric_test_assumptions_unstated",
        "marginal_significance_language",
        "test_claimed_but_absent_from_analysis",
        // ml_methodology
        "resampling_not_stated_as_post_split",
        "no_heldout_evaluation_named",
        "no_baseline_comparison",
        "no_variance_across_runs",
        // claim_evidence_strength
        "causal_claim_from_associational_design",
        // validation_maths
        "TestGroupMismatch",
        "PValueOverclaim",
        "MissingEffectSize",
        "MissingConfidenceInterval",
        "SmallSampleCausalClaim",
        // novelty
        "PRIOR_WORK_EXISTS",
        "NOVELTY_NARROWER_THAN_STATED",
        "novelty_claim_states_no_scope",
        // research-state checks, §4.5 [v8]
        "no_data_availability_statement",
        "no_ethics_statement_in_a_study_with_subjects",
        "reference_list_is_mostly_outdated",
        // reporting_standards, routed by check rather than item number
        "AbstractPresent",
        "ResultsSectionPresent",
        "SampleSizeReported",
        "StatisticalTestReported",
        "EffectSizeReported",
        "ConfidenceIntervalReported",
    ];

    /// **A finding nobody claimed is a finding nobody sees.** Routing is by
    /// code, so a new check that no criterion lists would be collected,
    /// filtered out and never appear in any report — silently, and with no
    /// count anywhere to notice it by.
    #[test]
    fn every_shipped_finding_code_is_claimed_by_exactly_one_criterion() {
        for code in SHIPPED_CODES {
            let owners: Vec<&str> = lenses()
                .iter()
                .flat_map(|l| l.criteria.iter())
                .filter(|c| c.codes.contains(code))
                .map(|c| c.id)
                .collect();
            assert_eq!(
                owners.len(),
                1,
                "`{code}` is claimed by {owners:?} — zero means it never reaches a report, \
                 more than one means it appears twice in the same review"
            );
        }
    }

    /// **The severity policy must not default.** A code with no rule takes
    /// [`DEFAULT_SEVERITY`] and reads in the report exactly like one somebody
    /// placed in the risk table deliberately.
    #[test]
    fn every_shipped_finding_code_has_a_severity_rule() {
        let missing: Vec<&&str> =
            SHIPPED_CODES.iter().filter(|c| severity_rule(c).is_none()).collect();
        assert!(
            missing.is_empty(),
            "no rule in SEVERITY_POLICY for {missing:?} — each would be reported at {} with \
             no risk-table cell behind it",
            DEFAULT_SEVERITY.as_str()
        );
    }

    /// **The §4.5 correction, as a test.** Four lenses ship; the other two are
    /// declared declined, and for DIFFERENT reasons — one is a missing data
    /// path a corpus job would fill, the other a criterion this tier should not
    /// evaluate at all. Collapsing them into "not built" is what loses that.
    #[test]
    fn four_lenses_ship_and_the_two_declined_are_declined_for_different_reasons() {
        let ids: Vec<LensId> = lenses().iter().map(|l| l.id).collect();
        assert_eq!(
            ids,
            vec![
                LensId::Methodology,
                LensId::Statistics,
                LensId::NoveltyLiterature,
                LensId::ReportingEthics
            ]
        );
        let declined: Vec<LensId> = DECLINED_LENSES.iter().map(|d| d.id).collect();
        assert_eq!(declined, vec![LensId::JournalFit, LensId::General]);

        let fit = &DECLINED_LENSES[0];
        assert!(
            fit.criteria.iter().all(|c| c.sources.iter().all(|s| s.state() != SourceState::Shipping)),
            "journal fit is declined because its LAYER is empty"
        );
        assert!(fit.reason.contains("THE LAYER IS EMPTY"));

        let general = &DECLINED_LENSES[1];
        assert!(
            general.criteria.iter().all(|c| c
                .sources
                .iter()
                .all(|s| s.field().map(|f| f.reach() == FieldReach::Mediated).unwrap_or(false))),
            "general is declined because every field it reads is MEDIATED — a different \
             reason, and one no data path fixes"
        );
        assert!(general.reason.contains("THE ABSENCE RULE FORBIDS IT"));
        assert!(
            general.reason.contains("NO data path would unblock this one"),
            "the two declines must not read as the same kind"
        );
    }

    /// **A shipping lens has sourced criteria, and NAMES the unsourced ones.**
    ///
    /// The widened `reads` bought three criteria that had no source under
    /// §4.5's original signature — data availability, ethics disclosure and
    /// reference currency. It did not buy all of them: *Coverage of Relevant
    /// Literature* still needs the field's literature, which is the comparable
    /// corpus's problem, and the honest handling is to report it unsourced
    /// rather than to drop it from the lens so the lens looks complete.
    #[test]
    fn a_shipping_lens_has_sourced_criteria_and_names_every_unsourced_one() {
        for l in lenses() {
            let sourced = l
                .criteria
                .iter()
                .filter(|c| c.sources.iter().any(|s| s.state() == SourceState::Shipping))
                .count();
            assert!(sourced > 0, "{:?} has no sourced criterion at all", l.id);

            // Whatever is unsourced must be reachable as such, so the report can
            // print it rather than the reader assuming it was checked.
            let a = availability();
            let entry = a.iter().find(|x| x.lens == l.id).unwrap();
            assert_eq!(
                entry.sourced_criteria.len() + entry.unsourced_criteria.len(),
                l.criteria.len(),
                "{:?}: every criterion is accounted for in one list or the other",
                l.id
            );
            for (name, why) in &entry.unsourced_criteria {
                assert!(!why.is_empty(), "{:?} / {name} is unsourced with no reason", l.id);
            }
        }
    }

    /// **Nothing a shipping lens reads may rest on a mediated absence.** The
    /// gate enforces this per finding; this enforces it at declaration time, so
    /// a criterion cannot be added whose every finding would be refused.
    #[test]
    fn no_shipping_criterion_concludes_from_a_mediated_field() {
        for l in lenses() {
            for c in l.criteria {
                for f in c.sources.iter().filter_map(|s| s.field()) {
                    assert_eq!(
                        f.reach(),
                        FieldReach::Exhaustive,
                        "{:?} / {} reads `{}`, whose absence is about the extractor — {}",
                        l.id,
                        c.name,
                        f.as_str(),
                        f.absence_note()
                    );
                }
            }
        }
    }

    /// **A source that exists and did not run is not a pass.** This is the
    /// four-zeros distinction at the lens layer: `None` and `Some(&[])` must
    /// produce different reports.
    #[test]
    fn a_source_that_never_ran_is_not_reported_as_a_strength() {
        let r = crate::extract::extract_from_text(
            "Introduction\n\nA paragraph.\n\nResults\n\nThe mean was 4.2 (p = 0.03).\n",
        );
        let lenses = lenses();
        let stats = lenses.iter().find(|l| l.id == LensId::Statistics).unwrap();

        let never_ran = review(
            stats,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: None,
                validity: None,
                standards: None,
                checklist: None,
                novelty: None,
            },
        );
        assert!(
            never_ran.strengths.is_empty(),
            "nothing ran, so nothing passed: {:?}",
            never_ran.strengths
        );
        assert!(
            never_ran
                .criteria
                .iter()
                .all(|c| matches!(c.state, CriterionState::SourceNotRun { .. })),
            "{:?}",
            never_ran.criteria
        );

        let ran_clean = review(
            stats,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(&[specialist::SpecialistReport {
                    specialist: "frequentist_stats".into(),
                    ..Default::default()
                }]),
                validity: Some(&crate::validate::validate(&r)),
                standards: Some(&[]),
                checklist: None,
                novelty: None,
            },
        );
        assert!(
            !ran_clean.strengths.is_empty(),
            "the checks ran and found nothing, which IS a strength: {ran_clean:?}"
        );
    }

    /// **The risk table's third column is a strength, never a minor concern.**
    /// Read as §4.5 describes the policy — *"fatal / fixable / minor"* — the
    /// Low Risk column would become a minor concern reading "Robust design with
    /// adequate power".
    #[test]
    fn no_concern_ever_quotes_the_risk_tables_compliant_column() {
        let compliant: Vec<&str> = RISK_TABLE.iter().map(|b| b.compliant).collect();
        let r = crate::extract::extract_from_text(
            "Abstract\n\nA cross-sectional survey of 412 firms.\n\nDiscussion\n\n\
             Organisational capacity causes higher insurance provision among firms.\n",
        );
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let reports = vec![specialist::run(
            &specialist::claim_strength::ClaimStrengthSpecialist,
            &input,
        )];
        let lenses = lenses();
        let m = lenses.iter().find(|l| l.id == LensId::Methodology).unwrap();
        let report = review(
            m,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(&reports),
                validity: Some(&crate::validate::validate(&r)),
                standards: None,
                checklist: None,
                novelty: None,
            },
        );
        assert_eq!(report.major_concerns.len(), 1, "{report:?}");
        for c in report.major_concerns.iter().chain(report.minor_concerns.iter()) {
            for good in &compliant {
                assert!(
                    !c.summary.contains(good),
                    "a concern quoting the table's good-case column: {}",
                    c.summary
                );
            }
        }
    }

    /// Every concern carries the span, the uncertainty and a trail naming the
    /// risk-table cell. A reviewer report is the most quotable artefact this
    /// system makes; a concern in it that cannot be checked is the worst place
    /// for an unsupported assertion.
    #[test]
    fn every_concern_carries_its_span_its_uncertainty_and_its_trail() {
        let r = crate::extract::extract_from_text(
            "Abstract\n\nA cross-sectional survey of 412 firms.\n\nDiscussion\n\n\
             Organisational capacity causes higher insurance provision among firms.\n",
        );
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let reports = vec![specialist::run(
            &specialist::claim_strength::ClaimStrengthSpecialist,
            &input,
        )];
        let lenses = lenses();
        let m = lenses.iter().find(|l| l.id == LensId::Methodology).unwrap();
        let report = review(
            m,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(&reports),
                validity: Some(&crate::validate::validate(&r)),
                standards: None,
                checklist: None,
                novelty: None,
            },
        );
        for c in &report.major_concerns {
            assert!(!c.spans.is_empty(), "{c:?}");
            assert_eq!(c.spans.len(), c.occurrences, "every occurrence keeps its span: {c:?}");
            assert!(c.uncertainty.is_some(), "{c:?}");
            assert!(
                c.trail.iter().any(|t| t.stage == "severity" && t.detail.contains("Risk")),
                "the trail must name the risk-table cell that set the severity: {:?}",
                c.trail
            );
            assert!(!c.required_revision.is_empty(), "{c:?}");
        }
    }

    /// **A specialist that DECLINED has not run its checks.** Measured on
    /// `Revised Health Economics Paper FINAL (1).docx`: the methodology lens
    /// printed *"Study Design: Robust design with adequate power and thorough
    /// explanation; appropriate analysis"* as a STRENGTH of a cross-sectional
    /// employer survey, because the MACHINE-LEARNING specialist — which had
    /// declined, correctly, on a paper with no model in it — counted as a check
    /// that ran and found nothing.
    #[test]
    fn a_specialist_that_declined_does_not_earn_the_manuscript_a_strength() {
        let r = crate::extract::extract_from_text(
            "Abstract\n\nA cross-sectional employer survey of 222 firms.\n\nMethods\n\n\
             Logistic regression was used.\n",
        );
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let reports = vec![specialist::run(&specialist::ml::MachineLearningSpecialist, &input)];
        assert!(
            reports[0].not_applicable.is_some(),
            "precondition: this is not an ML paper"
        );
        let lenses = lenses();
        let m = lenses.iter().find(|l| l.id == LensId::Methodology).unwrap();
        let report = review(
            m,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(&reports),
                validity: None,
                // **`Some(&[])` is load-bearing in this test, not incidental.**
                // `study_design` has two shipping sources. With `standards:
                // None` the criterion is `SourceNotRun` because the STANDARDS
                // did not run, and the test passes whether or not the declined
                // ML report is counted — which is what the deletion test found
                // when it went green where red was predicted. Running the
                // standards leaves the ML specialist as the only thing that
                // could have earned the strength.
                standards: Some(&[]),
                checklist: None,
                novelty: None,
            },
        );
        assert!(
            report.strengths.is_empty(),
            "the only design check declined; nothing established a robust design: {:?}",
            report.strengths
        );
    }

    /// **One concern per code, carrying every occurrence.** Ungrouped, the
    /// statistics lens produced 15 major concerns on one manuscript of which 11
    /// were two codes repeating.
    #[test]
    fn a_code_that_fires_repeatedly_is_one_concern_carrying_every_span() {
        let r = crate::extract::extract_from_text(
            "Results\n\nGroup A differed (t = 2.1, p = 0.04).\n\nThe second test also \
             differed (t = 3.0, p = 0.01).\n\nA third differed (t = 4.0, p = 0.02).\n",
        );
        let v = crate::validate::validate(&r);
        let repeated = v
            .flags
            .iter()
            .filter(|f| f.rule == crate::validate::RuleId::MissingEffectSize)
            .count();
        assert!(repeated >= 2, "precondition: the rule fires more than once ({repeated})");

        let lenses = lenses();
        let stats = lenses.iter().find(|l| l.id == LensId::Statistics).unwrap();
        let report = review(
            stats,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(&[]),
                validity: Some(&v),
                standards: Some(&[]),
                checklist: None,
                novelty: None,
            },
        );
        let missing: Vec<&Concern> = report
            .major_concerns
            .iter()
            .filter(|c| c.code == "MissingEffectSize")
            .collect();
        assert_eq!(missing.len(), 1, "one concern, not {repeated}: {missing:?}");
        assert_eq!(missing[0].occurrences, repeated, "and it says how many");
    }

    /// **Ran with nothing to look at is not a pass.** Measured on the real
    /// manuscript: the novelty lens printed *"Presents a clearly new finding,
    /// well-motivated hypothesis"* as a strength of a paper that states no
    /// novelty claim at all.
    ///
    /// The criterion carrying that case is now `Up-to-Date References`: the
    /// novelty criterion's source was DECLINED in §11 D166, so it reports as
    /// having no shipping source rather than as having run with nothing to
    /// examine. Both are asserted below, because the two states must stay
    /// distinguishable — that is what this test is for.
    #[test]
    fn a_check_with_nothing_to_examine_yields_neither_a_concern_nor_a_strength() {
        let r = crate::extract::extract_from_text(
            "Introduction\n\nThis paper reports an employer survey.\n",
        );
        let lenses = lenses();
        let nov = lenses.iter().find(|l| l.id == LensId::NoveltyLiterature).unwrap();
        let report = review(
            nov,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: None,
                validity: None,
                standards: None,
                checklist: None,
                // The extractor RAN and found no novelty claim.
                novelty: Some(&[]),
            },
        );
        assert!(
            report.strengths.is_empty(),
            "no novelty claim was made, so none was verified: {:?}",
            report.strengths
        );
        assert!(report.major_concerns.is_empty() && report.minor_concerns.is_empty());
        // DECLINED is not the same as ran-with-nothing-to-examine, and the
        // report must say which. §11 D166 is the reason, carried in the row.
        let novelty = report
            .criteria
            .iter()
            .find(|c| c.criterion == "Novelty & Significance")
            .unwrap_or_else(|| panic!("{:?}", report.criteria));
        match &novelty.state {
            CriterionState::NoShippingSource { detail } => {
                assert!(detail.contains("D166"), "the row names the decision: {detail}")
            }
            other => panic!("a declined source is not a source that did not run: {other:?}"),
        }
        // And the criterion that DID run with nothing to examine is in the
        // other state.
        assert!(
            report.criteria.iter().any(|c| c.criterion == "Up-to-Date References"
                && matches!(c.state, CriterionState::SourceNotRun { .. })),
            "{:?}",
            report.criteria
        );
        // And the reference-currency criterion says nothing on a manuscript with
        // no parsed references — below the denominator it refuses to speak.
        assert!(
            report.major_concerns.iter().all(|c| c.code != "reference_list_is_mostly_outdated"),
            "{:?}",
            report.major_concerns
        );
    }

    /// A strength must READ as a report of what ran, not as an assertion that
    /// the risk table's good-case description holds. Three risk rows cover more
    /// than one criterion, so the description is about the row.
    #[test]
    fn a_strength_says_what_ran_rather_than_asserting_the_good_case() {
        let r = crate::extract::extract_from_text(
            "Results\n\nThe effect was large (Cohen's d = 0.8, 95% CI: 0.4-1.2, t = 2.1, \
             p = 0.04).\n",
        );
        let lenses = lenses();
        let stats = lenses.iter().find(|l| l.id == LensId::Statistics).unwrap();
        let report = review(
            stats,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(&[specialist::SpecialistReport {
                    specialist: "frequentist_stats".into(),
                    ..Default::default()
                }]),
                validity: Some(&crate::validate::validate(&r)),
                standards: Some(&[]),
                checklist: None,
                novelty: None,
            },
        );
        assert!(!report.strengths.is_empty(), "{report:?}");
        for s in &report.strengths {
            assert!(
                s.contains("check(s) ran") && s.contains("not a positive finding"),
                "a strength that asserts rather than reports: {s}"
            );
        }
    }

    fn methodology_over(text: &str) -> ReviewerReport {
        let r = crate::extract::extract_from_text(text);
        let lenses = lenses();
        let m = lenses.iter().find(|l| l.id == LensId::Methodology).unwrap();
        review(
            m,
            &LensInput {
                extraction: &r,
                full_text: Some(text),
                this_year: 2026,
                specialists: Some(&[]),
                validity: None,
                standards: None,
                checklist: None,
                novelty: None,
            },
        )
    }

    /// **The exhaustive half.** Every word of the manuscript was searched, so
    /// the absence of a data availability statement is a fact about the
    /// manuscript — and this is the finding §4.5's original `reads` could not
    /// produce, because no specialist makes it.
    #[test]
    fn an_absence_in_an_exhaustive_field_is_a_finding() {
        let report = methodology_over(
            "Abstract\n\nWe surveyed 412 firms.\n\nResults\n\nProvision was 36.5%.\n",
        );
        let f = report
            .major_concerns
            .iter()
            .find(|c| c.code == "no_data_availability_statement")
            .unwrap_or_else(|| panic!("{report:?}"));
        assert_eq!(f.source, LensSource::State(StateField::FullText));
        assert!(
            f.spans[0].contains("Searched the whole manuscript"),
            "an absence has no quotable sentence, so it quotes what it searched for: {:?}",
            f.spans
        );
        assert!(
            f.uncertainty.as_deref().unwrap().contains("lexicon limit"),
            "an absence finding must state the limit of its own lexicon"
        );
        assert!(report.absence_rejected.is_empty(), "{:?}", report.absence_rejected);
    }

    #[test]
    fn a_data_availability_statement_that_is_present_suppresses_the_finding() {
        let report = methodology_over(
            "Abstract\n\nWe surveyed 412 firms.\n\nMethods\n\nData availability: the \
             dataset is deposited in Zenodo.\n",
        );
        assert!(
            report.major_concerns.iter().all(|c| c.code != "no_data_availability_statement"),
            "{:?}",
            report.major_concerns
        );
    }

    /// **The mediated half, and the D157 ratio made visible.** A finding resting
    /// on a mediated absence is REFUSED, and the refusal is returned with its
    /// reason rather than dropped.
    #[test]
    fn an_absence_in_a_mediated_field_is_refused_with_its_reason() {
        for f in [
            StateField::Sections,
            StateField::Title,
            StateField::References,
            StateField::Statistics,
            StateField::Tables,
            StateField::JournalRequirements,
        ] {
            let err = admit_absence(f).expect_err("mediated fields are refused");
            assert!(err.contains("MEDIATED"), "{err}");
            assert!(
                err.contains("about the extractor"),
                "the refusal must say whose fact the absence is: {err}"
            );
        }
        for f in [StateField::FullText, StateField::ReferenceYears] {
            assert!(admit_absence(f).is_ok(), "{f:?}");
        }
    }

    /// **The ethics check is gated on the manuscript having subjects**, and the
    /// gate is the difference between a finding and a nuisance: a limnology
    /// chapter needs no ethics statement, and flagging one would be the ML
    /// specialist firing on a silkworm paper.
    #[test]
    fn the_ethics_check_declines_on_a_manuscript_with_no_human_or_animal_subjects() {
        let desk = methodology_over(
            "Introduction\n\nThis chapter reviews sediment chemistry in three lakes.\n\
             \n\nResults\n\nAlkalinity increased toward the inlet.\n",
        );
        assert!(
            desk.major_concerns
                .iter()
                .all(|c| c.code != "no_ethics_statement_in_a_study_with_subjects"),
            "no subjects, so no ethics statement is owed: {:?}",
            desk.major_concerns
        );

        let with_subjects = methodology_over(
            "Methods\n\nWe recruited 412 participants who completed a questionnaire.\n",
        );
        let f = with_subjects
            .major_concerns
            .iter()
            .find(|c| c.code == "no_ethics_statement_in_a_study_with_subjects")
            .unwrap_or_else(|| panic!("{with_subjects:?}"));
        // MAJOR, not BLOCKING: both halves of this check are lexicons, and §8's
        // Blocking cannot be demoted by the editor. See the severity rule.
        assert_eq!(f.severity, ReviewSeverity::Major);
    }

    /// **The four false subject matches that produced a BLOCKING finding on a
    /// real manuscript.** Each string is that manuscript's own words.
    ///
    /// This is the row the pipeline surfaced on its first live run against a
    /// real journal, which is why it was found: a probe that prints the
    /// pipeline's own first row rather than one chosen for looking good.
    #[test]
    fn a_metaphor_a_lake_survey_and_a_bibliography_are_not_research_subjects() {
        for sentence in [
            "The microclimate findings reinforce that urban wetlands are active \
             participants in their surrounding environment.",
            "The field survey of Section 4.1 established that the three lakes carry a \
             heavy and seasonally variable nutrient load.",
            "It is an almost exclusive inhabitant of the gut of warm-blooded animals, and \
             it is shed in enormous numbers in faeces.",
            "127 Publication Lawrence, C.. \"The husbandry of zebrafish (Danio rerio): A \
             review\", Aquaculture, 2007",
        ] {
            let report = methodology_over(&format!("Introduction\n\n{sentence}\n"));
            assert!(
                report
                    .major_concerns
                    .iter()
                    .all(|c| c.code != "no_ethics_statement_in_a_study_with_subjects"),
                "this manuscript has no research subjects: {sentence:?}"
            );
        }
    }

    /// **The ethics finding quotes the sentence that makes it apply.** An
    /// absence has no sentence of its own, so without this the finding asserts
    /// that the manuscript studies subjects and shows nothing — and on the
    /// first live run the sentence it rested on was a metaphor about wetlands.
    #[test]
    fn the_ethics_finding_quotes_the_sentence_that_shows_subjects() {
        // `final final L.pdf`'s own words, and the phrase that actually fires.
        let report = methodology_over(
            "Methods\n\nAfter exposure, the fish were euthanised, and the brain tissues \
             were dissected and homogenised in 0.1 M phosphate buffer.\n",
        );
        let f = report
            .major_concerns
            .iter()
            .find(|c| c.code == "no_ethics_statement_in_a_study_with_subjects")
            .unwrap_or_else(|| panic!("{report:?}"));
        assert!(
            f.spans[0].contains("WHY THIS CHECK APPLIES: After exposure, the fish were \
                                 euthanised"),
            "the finding must quote what made it apply: {:?}",
            f.spans
        );
        assert!(f.spans[0].contains("Searched the whole manuscript for"), "{:?}", f.spans);
    }

    #[test]
    fn a_stated_ethics_approval_suppresses_the_finding() {
        let report = methodology_over(
            "Methods\n\nWe recruited 412 participants. Ethical approval was granted by the \
             institutional review board and the study followed the Declaration of \
             Helsinki.\n",
        );
        assert!(
            report
                .major_concerns
                .iter()
                .all(|c| c.code != "no_ethics_statement_in_a_study_with_subjects"),
            "{:?}",
            report.major_concerns
        );
    }

    /// **A fraction is a claim and its denominator is the half nobody checks**
    /// (§11 D161). Below ten dated references the check says nothing rather
    /// than reporting "0 of 3 are recent".
    #[test]
    fn reference_currency_refuses_to_speak_below_its_denominator() {
        let refs = |years: &[i32]| -> Vec<crate::extract::citations::Reference> {
            years
                .iter()
                .map(|y| crate::extract::citations::Reference {
                    raw: String::new(),
                    authors: "A".into(),
                    year: Some(*y),
                    title: None,
                    doi: None,
                })
                .collect()
        };
        assert_eq!(reference_currency(&refs(&[1990, 1991, 1992]), 2026).majority_outdated(), None);

        let old = refs(&[1980, 1981, 1982, 1983, 1984, 1985, 1986, 1987, 1988, 1989, 2024]);
        let c = reference_currency(&old, 2026);
        assert_eq!(c.dated, 11);
        assert_eq!(c.majority_outdated(), Some(true));

        let recent = refs(&[2018, 2019, 2020, 2021, 2022, 2023, 2024, 2025, 2019, 2020, 1970]);
        assert_eq!(reference_currency(&recent, 2026).majority_outdated(), Some(false));
    }

    /// The denominator is the DATED references, not the total, and the row says
    /// which — the fraction's unit, stated where a reader sees it.
    #[test]
    fn the_currency_row_states_which_references_its_denominator_counts() {
        let mut refs: Vec<crate::extract::citations::Reference> = (0..11)
            .map(|i| crate::extract::citations::Reference {
                raw: String::new(),
                authors: "A".into(),
                year: Some(1980 + i),
                title: None,
                doi: None,
            })
            .collect();
        // Five with no parsable year — in the total, not in the denominator.
        for _ in 0..5 {
            refs.push(crate::extract::citations::Reference {
                raw: String::new(),
                authors: "B".into(),
                year: None,
                title: None,
                doi: None,
            });
        }
        let r = crate::extract::extract_from_text("Introduction\n\nA paragraph.\n");
        let mut ex = r.clone();
        ex.references = refs;
        let lenses = lenses();
        let nov = lenses.iter().find(|l| l.id == LensId::NoveltyLiterature).unwrap();
        let report = review(
            nov,
            &LensInput {
                extraction: &ex,
                full_text: Some("Introduction\n\nA paragraph.\n"),
                this_year: 2026,
                specialists: None,
                validity: None,
                standards: None,
                checklist: None,
                novelty: Some(&[]),
            },
        );
        let c = report
            .minor_concerns
            .iter()
            .find(|c| c.code == "reference_list_is_mostly_outdated")
            .unwrap_or_else(|| panic!("{report:?}"));
        assert!(c.summary.contains("of 11 dated references"), "{}", c.summary);
        assert!(c.spans[0].contains("16 reference(s) parsed, 11 carrying a year"), "{:?}", c.spans);
        assert!(
            c.spans[0].contains("denominator is the dated ones"),
            "the row must say what its denominator counts: {:?}",
            c.spans
        );
    }

    /// A full-text check with no text declines rather than concluding that
    /// every phrase is absent.
    #[test]
    fn a_full_text_check_with_no_text_supplied_concludes_nothing() {
        let r = crate::extract::extract_from_text("Methods\n\nWe recruited 412 participants.\n");
        let lenses = lenses();
        let m = lenses.iter().find(|l| l.id == LensId::Methodology).unwrap();
        let report = review(
            m,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(&[]),
                validity: None,
                standards: None,
                checklist: None,
                novelty: None,
            },
        );
        assert!(report.major_concerns.is_empty(), "{:?}", report.major_concerns);
        assert!(report.strengths.is_empty(), "and it is not a pass either: {:?}", report.strengths);
    }

    /// **A check its own gate turned away is not a pass.** The ethics check
    /// declines unless the manuscript shows human or animal subjects; on a desk
    /// study it never runs, and without this the report claimed *"All data
    /// deposited publicly; full ethical disclosure; reproducible methods"* as a
    /// strength of a manuscript whose ethics check had not executed.
    ///
    /// Third instance of the same shape, each a layer further down: a declined
    /// specialist, a check with no subject matter, a check its gate skipped.
    #[test]
    fn a_check_its_own_gate_skipped_is_not_reported_as_a_strength() {
        let desk = methodology_over(
            "Introduction\n\nThis chapter reviews sediment chemistry in three lakes.\n\n\
             Methods\n\nData availability: the dataset is deposited in Zenodo.\n",
        );
        assert!(
            desk.strengths.iter().all(|s| !s.starts_with("Ethical Compliance")),
            "the ethics check never ran on a study with no subjects: {:?}",
            desk.strengths
        );
        assert!(
            desk.criteria.iter().any(|c| c.criterion == "Ethical Compliance"
                && matches!(&c.state, CriterionState::SourceNotRun { detail }
                    if detail.contains("its own gate turned it away"))),
            "and the report says which: {:?}",
            desk.criteria
        );
        // The data-availability check DID run on the same manuscript and was
        // satisfied, so that one is a strength — the pair is the point.
        assert!(
            desk.strengths.iter().any(|s| s.starts_with("Reproducibility & Data Availability")),
            "{:?}",
            desk.strengths
        );
    }

    /// **The strength line's number must count what it says it counts**
    /// (§11 D161). It read "1 check(s) ran (full_text, journal_requirements)" —
    /// a count of CODES against a list of SOURCES.
    #[test]
    fn the_strength_line_counts_checks_and_names_them() {
        let report = methodology_over(
            "Methods\n\nWe recruited 412 participants. Ethical approval was granted by the \
             institutional review board. Data availability: deposited in Zenodo.\n",
        );
        let s = report
            .strengths
            .iter()
            .find(|s| s.starts_with("Ethical Compliance"))
            .unwrap_or_else(|| panic!("{report:?}"));
        assert!(
            s.contains("1 of this criterion's check(s) ran — \
                        no_ethics_statement_in_a_study_with_subjects — reading"),
            "the number counts checks and the checks are named: {s}"
        );
    }

    /// Codes that no `Specialist` impl emits, each with what produces it. An
    /// EXPLICIT list rather than a string heuristic: the first version filtered
    /// `SHIPPED_CODES` by prefix and silently stopped covering a code the
    /// moment one was named differently.
    const NOT_FROM_A_SPECIALIST: &[(&str, &str)] = &[
        ("TestGroupMismatch", "validate.rs"),
        ("PValueOverclaim", "validate.rs"),
        ("MissingEffectSize", "validate.rs"),
        ("MissingConfidenceInterval", "validate.rs"),
        ("SmallSampleCausalClaim", "validate.rs"),
        ("PRIOR_WORK_EXISTS", "novelty::assess"),
        ("NOVELTY_NARROWER_THAN_STATED", "novelty::assess"),
        ("novelty_claim_states_no_scope", "novelty::assess"),
        ("AbstractPresent", "report::evaluate"),
        ("ResultsSectionPresent", "report::evaluate"),
        ("SampleSizeReported", "report::evaluate"),
        ("StatisticalTestReported", "report::evaluate"),
        ("EffectSizeReported", "report::evaluate"),
        ("ConfidenceIntervalReported", "report::evaluate"),
        ("no_data_availability_statement", "a research-state check"),
        ("no_ethics_statement_in_a_study_with_subjects", "a research-state check"),
        ("reference_list_is_mostly_outdated", "a research-state check"),
    ];

    /// `codes_of` is a second list beside `SHIPPED_CODES`, and two lists of one
    /// thing drift the first time only one is edited (§11 D129). Every shipped
    /// code is either attributed to a specialist or named here as coming from
    /// something else — a code in neither list executes for nobody, and a
    /// criterion holding it can never earn a strength.
    #[test]
    fn every_specialist_code_is_attributed_to_its_specialist() {
        let attributed: Vec<&str> = [
            SourceId::FrequentistStats,
            SourceId::MlMethodology,
            SourceId::ClaimEvidenceStrength,
        ]
        .iter()
        .flat_map(|s| codes_of(*s).iter().copied())
        .collect();
        for c in &attributed {
            assert!(SHIPPED_CODES.contains(c), "`{c}` is attributed but not shipped");
        }
        for c in SHIPPED_CODES {
            let elsewhere = NOT_FROM_A_SPECIALIST.iter().any(|(n, _)| n == c);
            assert!(
                attributed.contains(c) || elsewhere,
                "`{c}` is shipped, attributed to no specialist, and not listed as coming \
                 from anything else — nothing would ever mark it executed"
            );
            assert!(
                !(attributed.contains(c) && elsewhere),
                "`{c}` is claimed by a specialist AND by something else"
            );
        }
    }

    /// **The lens's `evidence_policy` is APPLIED, not just declared.**
    ///
    /// It carried the field and never used it, which §4.3 had already recorded
    /// once as *"a policy nothing enforces is a comment"*. The consequence
    /// showed up on the first real editor run: a reporting-standard absence
    /// reached the rubric's primary causes with no span.
    #[test]
    fn a_concern_that_quotes_nothing_is_refused_by_the_lenss_own_policy() {
        let r = crate::extract::extract_from_text("Results\n\nThe mean was 4.2 (p = 0.03).\n");
        // A specialist report whose finding carries no span at all.
        let naked = specialist::SpecialistReport {
            specialist: "frequentist_stats".into(),
            admitted: vec![crate::specialist::Finding {
                specialist: "frequentist_stats".into(),
                code: "multiple_comparisons_uncorrected".into(),
                severity: crate::report::FindingSeverity::Major,
                status: crate::epistemic::EpistemicStatus::Detected,
                summary: "something".into(),
                span: None,
                location: None,
                evidence: vec![crate::agent_graph::EvidenceSource::ManuscriptSpan],
                uncertainty: Some("u".into()),
            }],
            rejected: Vec::new(),
            not_applicable: None,
        };
        let lenses = lenses();
        let stats = lenses.iter().find(|l| l.id == LensId::Statistics).unwrap();
        assert!(stats.evidence_policy.citation_required, "precondition");
        let report = review(
            stats,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: Some(std::slice::from_ref(&naked)),
                validity: None,
                standards: Some(&[]),
                checklist: None,
                novelty: None,
            },
        );
        assert!(
            report.major_concerns.iter().all(|c| c.code != "multiple_comparisons_uncorrected"),
            "{report:?}"
        );
        assert_eq!(report.policy_rejected.len(), 1, "{:?}", report.policy_rejected);
        assert!(
            report.policy_rejected[0].contains("carried nothing"),
            "and the refusal says why: {:?}",
            report.policy_rejected
        );
    }

    /// **Every concern a lens emits quotes something.** The invariant the
    /// policy exists to hold, asserted over all four lenses on one input.
    #[test]
    fn every_concern_from_every_lens_carries_at_least_one_span() {
        let text = "Abstract\n\nA cross-sectional employer survey of 222 firms.\n\n\
                    Results\n\nProvision was 36.5% (t = 2.1, p = 0.04).\n\n\
                    Discussion\n\nCapacity causes higher provision among firms.\n";
        let r = crate::extract::extract_from_text(text);
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let reports: Vec<_> =
            specialist::shipped().iter().map(|s| specialist::run(s.as_ref(), &input)).collect();
        let standards: Vec<crate::report::StandardEvaluation> = [
            crate::journal_standards::Standard::Consort,
            crate::journal_standards::Standard::Strobe,
        ]
        .iter()
        .map(|s| crate::report::evaluate(*s, &r, text))
        .collect();

        for l in lenses() {
            let report = review(
                &l,
                &LensInput {
                    extraction: &r,
                    full_text: Some(text),
                    this_year: 2026,
                    specialists: Some(&reports),
                    validity: Some(&crate::validate::validate(&r)),
                    standards: Some(&standards),
                    checklist: None,
                    novelty: None,
                },
            );
            for c in report.major_concerns.iter().chain(report.minor_concerns.iter()) {
                assert!(
                    !c.spans.is_empty(),
                    "{:?} / {} / {} quotes nothing",
                    l.id,
                    c.criterion,
                    c.code
                );
            }
            // **And nothing had to be REFUSED for lacking one.** Without this
            // the assertion above holds trivially when the policy strips a
            // span-less concern before it is counted: a deletion test on the
            // span fix went GREEN because the policy caught what the fix would
            // have let through. Two guards covering one property is defence in
            // depth; a test that cannot tell them apart is not a test of
            // either.
            assert!(
                report.policy_rejected.is_empty(),
                "{:?} produced a concern its own policy had to refuse: {:?}",
                l.id,
                report.policy_rejected
            );
        }
    }

    /// **No lens makes a recommendation.** Accept / Minor / Major / Reject is
    /// the editor layer's output (§5.7), and a lens producing one would be six
    /// recommendations with nothing weighing them.
    #[test]
    fn no_lens_recommends_an_outcome() {
        let r = crate::extract::extract_from_text("Introduction\n\nA paragraph.\n");
        for l in lenses() {
            let report = review(
                &l,
                &LensInput {
                    extraction: &r,
                    full_text: None,
                    this_year: 2026,
                    specialists: None,
                    validity: None,
                    standards: None,
                    checklist: None,
                    novelty: None,
                },
            );
            let text = format!("{} {}", report.overall_assessment, report.contribution);
            for banned in ["we recommend", "recommend acceptance", "should be accepted",
                           "should be rejected", "recommend rejection"] {
                assert!(
                    !text.to_lowercase().contains(banned),
                    "{:?} made a recommendation: {text}",
                    l.id
                );
            }
        }
    }

    /// An empty journal-compliance section is not compliance. The journal phase
    /// established that an empty crawl asserting a fact about a journal is a
    /// claim about the instrument.
    #[test]
    fn an_empty_compliance_section_says_no_journal_was_read() {
        let r = crate::extract::extract_from_text("Introduction\n\nA paragraph.\n");
        let lenses = lenses();
        let re = lenses.iter().find(|l| l.id == LensId::ReportingEthics).unwrap();
        let report = review(
            re,
            &LensInput {
                extraction: &r,
                full_text: None,
                this_year: 2026,
                specialists: None,
                validity: None,
                standards: None,
                checklist: None,
                novelty: None,
            },
        );
        assert!(
            report.journal_compliance.iter().any(|l| l.contains("not compliance")),
            "{:?}",
            report.journal_compliance
        );
    }
}
