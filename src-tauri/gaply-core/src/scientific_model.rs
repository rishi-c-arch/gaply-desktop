//! Scientific domain model for PublishReady v2.
//!
//! This module defines the objects that power scientific reasoning:
//! claims, hypotheses, variables, methods, datasets, contributions, limitations,
//! and research questions. It is intentionally provider-blind: the objects can be
//! produced by deterministic rules, local SLMs, or optional cloud refinement.
//!
//! # Privacy contract
//!
//! Objects distinguish three classes of data:
//!
//! * **Local-only data** — `source_spans` and sentence indices, which reference
//!   paragraphs inside the local `ExtractionResult`. These are never sent to a
//!   cloud model.
//! * **Shareable metadata** — concise `statement` strings, categories, roles,
//!   designs, and relationships. These can be included in bounded reviewer
//!   payloads.
//! * **Derived metadata** — `verification_status`, `confidence`, `reviewer_notes`,
//!   and `novelty_links`. These are produced by downstream reasoning and can be
//!   recomputed or sent as summary.
//!
//! Raw manuscript paragraphs are never stored here. Original text is reachable
//! via `SourceSpan` through the existing `extract::paragraph_at` resolver.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::extract::{Location, SectionKind};

/// Version of the persisted scientific-model schema.
pub const SCIENTIFIC_MODEL_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// IDs and references
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClaimId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HypothesisId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariableId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MethodId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DatasetId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContributionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LimitationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuestionId(pub String);

/// Reference to a PublishReady finding id (e.g. `"f3"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FindingRef(pub String);

/// Reference to an in-text citation by index into `ExtractionResult.citations`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CitationRef(pub usize);

/// Reference to an extracted statistic by index into `ExtractionResult.statistics`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StatRef {
    pub index: usize,
    pub location: Location,
}

/// A span of paragraphs in the extraction result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub section: SectionKind,
    pub start_paragraph: usize,
    /// Inclusive end paragraph.
    pub end_paragraph: usize,
}

/// Where in the manuscript an object is grounded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceSpan {
    Point(Location),
    Range(Span),
}

// ---------------------------------------------------------------------------
// Claim — the only object implemented in Milestone A, Step 3
// ---------------------------------------------------------------------------

/// Rhetorical role of a claim inside the manuscript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimCategory {
    MainClaim,
    SupportingClaim,
    ResultClaim,
    ConclusionClaim,
    BackgroundClaim,
    MethodClaim,
    HypothesisClaim,
    NegativeClaim,
    ComparativeClaim,
    NoveltyClaim,
    FutureWorkClaim,
    LimitationClaim,
}

/// Logical relationship asserted by the claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimNature {
    Causal,
    Associational,
    Descriptive,
    Comparative,
    Generalization,
    Methodological,
    /// Escape hatch so the taxonomy can grow without a schema bump.
    Other(String),
}

/// Where the claim came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimSource {
    Deterministic,
    LocalSlm,
    Cloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimVerificationStatus {
    Unverified,
    Verified,
    Refuted,
    Uncertain,
}

/// Where a variable came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableSource {
    Deterministic,
    LocalSlm,
    Cloud,
}

/// Where a method came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodSource {
    Deterministic,
    LocalSlm,
    Cloud,
}

/// A scientific assertion made by the manuscript.
///
/// A claim is NOT a sentence. One sentence may contain zero, one, or many
/// claims. One claim may also be summarized across multiple sentences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificClaim {
    pub id: ClaimId,
    pub category: ClaimCategory,
    /// The claim text as it appears (or is summarized) in the manuscript.
    pub statement: String,
    /// A normalized reformulation, produced by future SLM/cloud stages.
    pub normalized: Option<String>,
    pub nature: ClaimNature,
    pub source_span: SourceSpan,
    pub sentence_indices: Vec<usize>,
    pub confidence: f64,
    pub source: ClaimSource,
    /// Findings that support, refute, or contextualize this claim.
    pub evidence: Vec<FindingRef>,
    pub statistics: Vec<StatRef>,
    pub citations: Vec<CitationRef>,
    pub variables: Vec<VariableId>,
    pub methods: Vec<MethodId>,
    pub datasets: Vec<DatasetId>,
    pub verification_status: ClaimVerificationStatus,
    pub reviewer_notes: Vec<String>,
    pub novelty_links: Vec<String>,
}

// ---------------------------------------------------------------------------
// Other scientific objects — defined for the container, not extracted yet
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    Primary,
    Secondary,
    Exploratory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchQuestion {
    pub id: QuestionId,
    pub kind: QuestionKind,
    pub text: String,
    pub domain: Option<String>,
    pub hypotheses: Vec<HypothesisId>,
    pub claims: Vec<ClaimId>,
    pub source_spans: Vec<SourceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisDirection {
    NonDirectional,
    Directional(EffectDirection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectDirection {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisResult {
    Supported,
    Rejected,
    Inconclusive,
    NotTested,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub question_id: Option<QuestionId>,
    pub statement: String,
    pub direction: HypothesisDirection,
    pub variables: Vec<VariableId>,
    pub method: Option<MethodId>,
    pub test_statistics: Vec<StatRef>,
    pub result: HypothesisResult,
    pub evidence: Vec<FindingRef>,
    pub source_spans: Vec<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableRole {
    Independent,
    Dependent,
    Outcome,
    Control,
    Confounder,
    Mediator,
    Moderator,
    Exposure,
    Predictor,
    Covariate,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementLevel {
    Nominal,
    Ordinal,
    Interval,
    Ratio,
    Dichotomous,
    Count,
    Continuous,
    Categorical,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub id: VariableId,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub role: VariableRole,
    pub measurement: MeasurementLevel,
    pub units: Option<String>,
    pub operationalization: Option<String>,
    pub mentions: Vec<SourceSpan>,
    pub sentence_indices: Vec<usize>,
    pub statistics: Vec<StatRef>,
    pub associated_claims: Vec<ClaimId>,
    pub associated_methods: Vec<MethodId>,
    pub associated_datasets: Vec<DatasetId>,
    pub confidence: f64,
    pub source: VariableSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyDesign {
    Experimental,
    QuasiExperimental,
    Observational,
    CrossSectional,
    Longitudinal,
    CaseStudy,
    CaseControl,
    Cohort,
    Survey,
    ClinicalTrial,
    Qualitative,
    MixedMethods,
    SystematicReview,
    MetaAnalysis,
    Simulation,
    MachineLearningPipeline,
    ComputationalExperiment,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplingDescription {
    pub method: Option<String>,
    pub frame: Option<String>,
    pub size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    Primary,
    Secondary,
    PublicDataset(String),
    Proprietary,
    Simulated,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Method {
    pub id: MethodId,
    pub name: Option<String>,
    pub design: StudyDesign,
    pub experimental_design: Option<String>,
    pub sampling: SamplingDescription,
    pub intervention: Option<String>,
    pub comparator: Option<String>,
    pub randomization: Option<String>,
    pub blinding: Option<String>,
    pub procedure_summary: Option<String>,
    pub materials: Vec<String>,
    pub software: Vec<String>,
    pub libraries: Vec<String>,
    pub algorithms: Vec<String>,
    pub evaluation_metrics: Vec<String>,
    pub protocol_references: Vec<String>,
    pub statistical_tests: Vec<StatisticalTestRef>,
    pub associated_variables: Vec<VariableId>,
    pub associated_claims: Vec<ClaimId>,
    pub associated_datasets: Vec<DatasetId>,
    pub source_spans: Vec<SourceSpan>,
    pub confidence: f64,
    pub source: MethodSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticalTestRef {
    pub name: String,
    pub stat: Option<StatRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub id: DatasetId,
    pub name: Option<String>,
    pub normalized_name: Option<String>,
    pub sample_size: Option<usize>,
    pub sample_description: Option<String>,
    pub population: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub recruitment: Option<String>,
    pub time_period: Option<String>,
    pub source: DataSource,
    pub institution: Option<String>,
    pub version: Option<String>,
    pub doi: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub inclusion_criteria: Option<String>,
    pub exclusion_criteria: Option<String>,
    pub variables: Vec<VariableId>,
    pub associated_claims: Vec<ClaimId>,
    pub associated_methods: Vec<MethodId>,
    pub missing_data_note: Option<String>,
    pub source_spans: Vec<SourceSpan>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionKind {
    Theoretical,
    Empirical,
    Methodological,
    Practical,
    Replication,
    Extension,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    pub id: ContributionId,
    pub kind: ContributionKind,
    pub statement: String,
    pub claims: Vec<ClaimId>,
    pub evidence: Vec<FindingRef>,
    pub source_spans: Vec<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitationScope {
    Sample,
    Design,
    Measurement,
    Generalizability,
    Analysis,
    Funding,
    ConflictOfInterest,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limitation {
    pub id: LimitationId,
    pub statement: String,
    pub scope: LimitationScope,
    pub mitigation: Option<String>,
    pub impact: Option<String>,
    pub affected_claims: Vec<ClaimId>,
    pub source_spans: Vec<SourceSpan>,
}

// ---------------------------------------------------------------------------
// Container
// ---------------------------------------------------------------------------

/// The serializable scientific payload stored inside `ExtractionResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificExtraction {
    pub schema_version: u32,
    pub questions: Vec<ResearchQuestion>,
    pub hypotheses: Vec<Hypothesis>,
    pub variables: Vec<Variable>,
    pub methods: Vec<Method>,
    pub datasets: Vec<Dataset>,
    pub claims: Vec<ScientificClaim>,
    pub contributions: Vec<Contribution>,
    pub limitations: Vec<Limitation>,
}

impl Default for ScientificExtraction {
    fn default() -> Self {
        Self {
            schema_version: SCIENTIFIC_MODEL_SCHEMA_VERSION,
            questions: Vec::new(),
            hypotheses: Vec::new(),
            variables: Vec::new(),
            methods: Vec::new(),
            datasets: Vec::new(),
            claims: Vec::new(),
            contributions: Vec::new(),
            limitations: Vec::new(),
        }
    }
}

/// Cloud-safe, bounded summary for reviewer payloads.
///
/// Contains no `SourceSpan` data and no paragraph text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificModelSummary {
    pub question_count: usize,
    pub hypothesis_count: usize,
    pub variable_count: usize,
    pub primary_claim_count: usize,
    pub claims_by_category: HashMap<String, usize>,
    pub limitations_count: usize,
    pub contribution_kinds: HashMap<String, usize>,
    pub primary_question: Option<String>,
}
