//! **DECLINED — §11 D166. This module is the INSTRUMENT that measured the
//! decline, not a shipping feature.**
//!
//! A 31-cue scan over 20 real research manuscripts — 37,557 sentences — found
//! **12 candidate sentences, of which 2 are claims a paper makes about its own
//! contribution**: 0.1 per manuscript. §4.6's own example phrasings,
//! *"first to demonstrate X"* and *"no prior study has…"*, appear in **0 of the
//! 20**. Both survivors come back `UNVERIFIED`.
//!
//! Nothing here is consumed by a lens. `SourceId::NoveltyClaims` is `Declined`,
//! and the code is kept for the reason D165 kept the scientific layer: deleting
//! the instrument would make the measurement unrepeatable. The reopening
//! condition is in D166 and its cheap half comes first — measure whether
//! authors write these sentences before rebuilding the extractor that would
//! find them.
//!
//! The one output that survives needs no retrieval: where an author asserts
//! unrestricted priority, [`NoveltyAssessment::states_no_scope`] says the claim
//! names no population, setting or task. That is a phrasing observation and is
//! explicitly not evidence that the claim is false.
//!
//! ---
//!
//! **Novelty, claim by claim, against retrieved prior work (§4.6).**
//!
//! §4.6: *"Extract every novelty claim from the research state … For each,
//! retrieve the closest prior work through the external-evidence layer.
//! Compare. Output per claim: the claim, the nearest prior work with what it
//! showed and under what conditions, and a status … Never a number."*
//!
//! # §4.6 IS WRONG ABOUT WHERE THE CLAIMS COME FROM, AND THAT IS WHY THIS NODE
//! CAN SHIP AT ALL
//!
//! §12's Phase-4 table lists *novelty, claim-by-claim* as **DECLINED**, its
//! input given as *"novelty claims from `&[Claim]`"* — the scientific layer,
//! declined on measurement (§11 D165). Read that way this node has nothing to
//! read and could not be built.
//!
//! But a novelty claim is not a scientific-layer object. It is **a sentence in
//! the manuscript containing one of a small set of phrases** — *"to the best of
//! our knowledge"*, *"the first study to"*, *"has not been reported"* — and the
//! manuscript is the free tier's own layer. Routing it through
//! [`crate::extract::claims`] would inherit that extractor's two defects for
//! nothing in return: it slices `sentence[cue_start..next_cue]`, so 89% of its
//! output is a sentence fragment, and 109 of 123 claims carry no subject. A
//! novelty claim whose span is *"first to demonstrate"* cannot be searched for
//! and cannot be refuted by a reader.
//!
//! So this module reads sentences and keeps them **whole**. Measured against
//! [`crate::extract::claims`] on the six real manuscripts, the comparison is in
//! `examples/novelty_probe.rs`.
//!
//! # `NOVEL_AS_STATED` IS UNREACHABLE HERE, AND THAT IS THE HONEST STATE
//!
//! Retrieval finding nothing is a fact about the index, not about the field.
//! One OpenAlex title search returning no match is exactly what a genuinely
//! novel claim looks like AND exactly what a badly-phrased query looks like,
//! and nothing deterministic separates them. Emitting `NOVEL_AS_STATED` from an
//! empty result set would be the project's oldest failure — an instrument
//! reporting a property of itself as a property of the world.
//!
//! [`NoveltyStatus::NovelAsStated`] therefore exists in the type, because it is
//! §4.6's wire contract and a later Tier-3 judge will produce it, and **no path
//! in this module returns it**. `novel_as_stated_is_never_returned_by_retrieval`
//! pins that, and names the condition that would change it.

use serde::{Deserialize, Serialize};

use crate::extract::{ExtractionResult, Location, SectionKind};

/// §4.6's four statuses. **Never a number** — there is no novelty score here
/// and no field one could be written into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoveltyStatus {
    /// Retrieval found prior work and a judge read it and disagreed with none
    /// of the claim. **Not reachable from retrieval alone** — see the module
    /// header.
    NovelAsStated,
    /// Prior work matches the claim as phrased, and the manuscript's own scope
    /// is narrower than the claim admits. Both spans are carried.
    NoveltyNarrowerThanStated,
    /// A retrieved work carries every distinctive term of the claim.
    PriorWorkExists,
    /// Retrieval found nothing decisive. **The default, and the honest one.**
    Unverified,
}

impl NoveltyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            NoveltyStatus::NovelAsStated => "NOVEL_AS_STATED",
            NoveltyStatus::NoveltyNarrowerThanStated => "NOVELTY_NARROWER_THAN_STATED",
            NoveltyStatus::PriorWorkExists => "PRIOR_WORK_EXISTS",
            NoveltyStatus::Unverified => "UNVERIFIED",
        }
    }
}

/// One novelty claim, as the manuscript states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoveltyClaim {
    /// **The whole sentence.** Not the fragment after the cue — a span exists to
    /// be checked, and a clipped one cannot be.
    pub sentence: String,
    /// The phrase that identified it, verbatim as it appears.
    pub cue: String,
    pub location: Location,
    /// Content terms the retrieval searches on, cue words and stopwords removed.
    pub terms: Vec<String>,
}

impl NoveltyClaim {
    /// **The span, whole.** Named so a caller reaching for the sentence gets
    /// the un-clipped one: a truncated span is not a span, and the clipping
    /// that cost this project a finding happened at the printing site.
    pub fn claim_span(&self) -> &str {
        &self.sentence
    }
}

/// Phrases that mark a sentence as claiming novelty. Matched case-insensitively
/// on word boundaries.
///
/// **Not regexes**: every one of these is a literal phrase, and a literal
/// `contains` over a lowercased sentence is both faster and impossible to get
/// wrong in the way `claims.rs`'s cue slicing was. The list is the lexicon; the
/// failure direction is a missed claim, never a fabricated one.
const NOVELTY_CUES: &[&str] = &[
    "to the best of our knowledge",
    "to our knowledge",
    "to the authors' knowledge",
    "for the first time",
    "this is the first",
    "the first study",
    "the first report",
    "the first to",
    "first attempt to",
    "no prior study",
    "no previous study",
    "no prior work",
    "no previous work",
    "has not been reported",
    "have not been reported",
    "has not previously been",
    "have not previously been",
    "has never been",
    "have never been",
    "remains unexplored",
    "remain unexplored",
    "little is known",
    "fills a gap",
    "fill this gap",
    "address this gap",
    "novel approach",
    "novel method",
    "novel framework",
    "novel finding",
    "unprecedented",
    "hitherto",
];

/// Words carrying no retrievable content. Cue words are stripped separately.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "been", "being", "best", "but", "by", "can",
    "could", "did", "do", "does", "for", "from", "had", "has", "have", "he", "her", "here", "his",
    "how", "i", "if", "in", "into", "is", "it", "its", "may", "might", "more", "most", "must",
    "no", "not", "of", "on", "one", "only", "or", "other", "our", "out", "over", "own", "same",
    "she", "should", "so", "some", "such", "than", "that", "the", "their", "them", "then",
    "there", "these", "they", "this", "those", "through", "to", "under", "up", "was", "we",
    "were", "what", "when", "where", "which", "while", "who", "why", "will", "with", "would",
    "you", "your", "study", "studies", "paper", "work", "research", "paper's", "paper,",
    "report", "reported", "first", "novel", "knowledge", "authors", "author", "paper.",
    "present", "presents", "presented", "using", "used", "use", "also", "however", "thus",
    "therefore", "both", "each", "between", "among", "well", "very", "much", "many",
    // **Measured, not guessed.** Every term below appeared in a retrieval query
    // built from a real claim in this corpus and contributed nothing: the
    // calibration run (`examples/novelty_query_calibrate.rs`) returned
    // groundwater ecosystems and Deleuze for a limnology claim, because twelve
    // generic terms diluted the two that carried the topic.
    "established", "establishes", "clear", "clearly", "documented", "documents",
    "exhibited", "exhibits", "identifiable", "identified", "unaware", "previous",
    "previously", "making", "makes", "made", "key", "aim", "aims", "aimed",
    "found", "finding", "findings", "results", "result", "shows", "shown", "show",
    "demonstrate", "demonstrates", "demonstrated", "investigate", "investigated",
    "examine", "examined", "attempt", "attempts", "conducted", "carried",
];

/// How many terms a retrieval query may carry.
///
/// **Measured, not chosen.** OpenAlex's `search` is relevance-ranked over
/// title, abstract and full text, so a long query dilutes rather than narrows.
/// On the corpus's two real claims: ten terms returned `NotFound` for one and
/// ten unrelated works for the other; four returned topically-correct works for
/// both (`examples/novelty_query_calibrate.rs`). Six is that measurement plus
/// the acronyms, which are added on top because they are the terms that carry
/// the claim and the first version dropped every one of them.
pub const MAX_QUERY_TERMS: usize = 6;

/// **Cues that are self-referential on their own.** *"To the best of OUR
/// knowledge"* is a statement about the authors; it needs no further marker.
const SELF_REFERENTIAL_CUES: &[&str] =
    &["to the best of our knowledge", "to our knowledge", "to the authors' knowledge"];

/// **Markers that a sentence is about THIS study rather than about the world.**
///
/// Measured over 20 real manuscripts: the first version of this extractor
/// admitted 12 claims and **only about 3 were claims about the paper at all**.
/// The rest were `unprecedented` used as an ordinary intensifier —
///
/// * *"China's accession to the WTO … subjected Indian small industries to
///   unprecedented competitive pressure."*
/// * *"E-commerce and social media marketing offer unprecedented opportunities
///   for reaching global markets."*
/// * *"It underwent unprecedented growth during the COVID-19 phase."*
///
/// — and one *"for the first time"* about the SUBJECTS rather than the finding:
/// *"many new registrations represent young entrepreneurs entering the craft
/// sector for the first time."*
///
/// None of those is a novelty claim, and every one of them would have been
/// carried to a retrieval query and a reviewer report. A novelty claim asserts
/// that THIS WORK is new, so the sentence has to say whose work it is.
const SELF_REFERENCE: &[&str] = &[
    "this study",
    "this paper",
    "this work",
    "this research",
    "this thesis",
    "this article",
    "this investigation",
    "this analysis",
    // **Unambiguous self-reference.** "This is the first study to X" is the
    // commonest real phrasing of a novelty claim and carries no other marker.
    // "the first study to" on its own is deliberately NOT here: it also fits
    // "the first study to examine X was Smith (1990)", which is a claim about
    // somebody else.
    "this is the first",
    "we are the first",
    "the present study",
    "the present work",
    "the present research",
    "the current study",
    "the study",
    "our study",
    "our work",
    "our research",
    "our findings",
    "our results",
    "our analysis",
    "our approach",
    "our method",
    "our model",
    "we report",
    "we show",
    "we demonstrate",
    "we present",
    "we found",
    "we establish",
    "we propose",
    "the authors",
];

/// **A thesis originality declaration is not a novelty claim**, and it matched
/// the cue list exactly once in 20 manuscripts:
///
/// > *"This work has not previously been produced or submitted for
/// > consideration by another candidate for the award of the Ph.D."*
///
/// It carries `this work`, so the self-reference rule admits it. It is a
/// statement about SUBMISSION rather than about findings, and retrieving prior
/// work for it is meaningless.
const DECLARATION_PHRASES: &[&str] = &[
    "submitted for consideration",
    "for the award of",
    "in partial fulfil",
    "in partial fulfill",
    "another candidate",
    "degree of doctor",
    "declare that this",
    "is my own work",
    "has not been submitted",
    "plagiarism",
];

/// Is this sentence a claim about THIS study?
fn is_about_this_study(sentence_lower: &str, cue: &str) -> bool {
    SELF_REFERENTIAL_CUES.contains(&cue) || SELF_REFERENCE.iter().any(|m| sentence_lower.contains(m))
}

/// Is this sentence a thesis originality declaration rather than a claim?
fn is_a_declaration(sentence_lower: &str) -> bool {
    DECLARATION_PHRASES.iter().any(|d| sentence_lower.contains(d))
}

/// Extract every novelty claim, whole-sentence.
///
/// Skips the References section: a reference list containing the words "the
/// first study" is a title, not a claim this manuscript makes.
pub fn extract_claims(result: &ExtractionResult) -> Vec<NoveltyClaim> {
    let mut out: Vec<NoveltyClaim> = Vec::new();

    for section in &result.sections {
        if section.kind == SectionKind::References {
            continue;
        }
        for (p_idx, paragraph) in section.paragraphs.iter().enumerate() {
            let loc = Location { section: section.kind, paragraph: p_idx, section_index: None };
            for sentence in crate::extract::sentence::sentences_in(paragraph) {
                let lower = sentence.to_lowercase();
                let Some(cue) = NOVELTY_CUES.iter().find(|c| lower.contains(**c)) else {
                    continue;
                };
                // **A novelty claim is about THIS study.** Without this, five of
                // twelve claims across 20 real manuscripts were `unprecedented`
                // describing a market or a period of history.
                if !is_about_this_study(&lower, cue) {
                    continue;
                }
                if is_a_declaration(&lower) {
                    continue;
                }
                let sentence = sentence.trim();
                // A sentence carrying two cues is one claim, not two. The
                // `claims.rs` shape — one object per cue, sliced between them —
                // is what produced 89% fragments.
                if out.iter().any(|c| c.sentence == sentence) {
                    continue;
                }
                out.push(NoveltyClaim {
                    sentence: sentence.to_string(),
                    cue: (*cue).to_string(),
                    location: loc.clone(),
                    terms: distinctive_terms(sentence),
                });
            }
        }
    }
    out
}

/// The content terms of a claim, for retrieval.
///
/// Lowercased, punctuation-trimmed, stopwords and cue words removed, deduped,
/// order preserved. Terms shorter than four characters are dropped: a
/// three-letter token retrieves noise, and the claim's distinctiveness never
/// rests on one.
pub fn distinctive_terms(sentence: &str) -> Vec<String> {
    let mut lower = sentence.to_lowercase();
    for cue in NOVELTY_CUES {
        lower = lower.replace(cue, " ");
    }
    let mut out: Vec<String> = Vec::new();
    for raw in lower.split_whitespace() {
        let t: String =
            raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '-').to_string();
        if t.len() < 4 || t.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if STOPWORDS.contains(&t.as_str()) {
            continue;
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// **Acronyms in the claim, which are the terms that carry it.**
///
/// The first version of [`distinctive_terms`] dropped every token shorter than
/// four characters, and on a real claim — *"hybridizes FA with CSA for
/// hyperparameter optimization in BiLSTM emotion classification"* — that
/// removed `FA` and `CSA`, the two tokens naming the algorithms the claim is
/// about. An acronym is recovered from the ORIGINAL sentence, before
/// lowercasing, because its shape is the only thing that identifies it.
///
/// A run of 2–8 characters that is all-uppercase (digits allowed after the
/// first character) and is not a sentence-initial single word. `I` is excluded
/// by the length floor.
pub fn acronyms(sentence: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in sentence.split_whitespace() {
        let t: String = raw.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
        if t.len() < 2 || t.len() > 8 {
            continue;
        }
        let mut chars = t.chars();
        let first_upper = chars.next().map(|c| c.is_ascii_uppercase()).unwrap_or(false);
        let rest_ok = t.chars().skip(1).all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        let has_two_caps = t.chars().filter(|c| c.is_ascii_uppercase()).count() >= 2;
        if first_upper && rest_ok && has_two_caps && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// **The query a claim is retrieved with.**
///
/// Acronyms first — they are the most distinctive tokens and the ones the first
/// version threw away — then the longest remaining terms, up to
/// [`MAX_QUERY_TERMS`]. Length is a crude proxy for specificity and it is an
/// honest one here: it is what separated `eco-successional` and `multi-criteria`
/// from `clear` and `four` in the calibration run.
pub fn retrieval_query(claim: &NoveltyClaim) -> String {
    let mut terms: Vec<String> = acronyms(&claim.sentence);
    let mut rest: Vec<String> = claim
        .terms
        .iter()
        .filter(|t| !terms.iter().any(|a| a.to_lowercase() == **t))
        .cloned()
        .collect();
    rest.sort_by_key(|t| std::cmp::Reverse(t.len()));
    for t in rest {
        if terms.len() >= MAX_QUERY_TERMS {
            break;
        }
        terms.push(t);
    }
    terms.join(" ")
}

/// **How much of a claim a retrieved work must carry before it NARROWS it.**
///
/// A majority of the claim's distinctive terms. Derived, not chosen:
/// `examples/novelty_overlap_calibrate.rs` ran the real retrievals for both
/// novelty claims in a 20-manuscript corpus — **20 works, and the highest
/// overlap was 3 of 7 terms (43%)**, on
/// *"Advanced Hybridization and Optimization of DNNs for Medical Imaging"*
/// matching `hyperparameter, optimization, classification`. The next rows down
/// matched one generic term each: *"Video Vortex reader: responses to
/// YouTube"* matched `response`.
///
/// Anything low enough to fire on that corpus fires on the YouTube paper, which
/// is the filter-catches-everything shape this project has measured before. A
/// majority bar excludes every one of the 20 — **so this threshold has never
/// fired on real data, and that is the finding rather than a failure**:
/// retrieval on a bag of terms returns topically-adjacent work and does not
/// decide novelty. `a_majority_overlap_narrows_a_claim_and_names_the_paper` is
/// the positive control, because a bar that has never fired proves nothing
/// about whether it can.
pub const NOVELTY_OVERLAP: f64 = 0.5;

/// Cues that claim novelty WITHOUT restriction — *"the first", "for the first
/// time"*. A claim carrying one of these and no scope clause asserts priority
/// over the whole literature.
///
/// *"To the best of our knowledge"* is deliberately NOT here: it is a hedge
/// that concedes the author's own limits, and treating it as a universal claim
/// would flag the honest phrasing and let the absolute one through.
const UNIVERSAL_CUES: &[&str] = &[
    "for the first time",
    "this is the first",
    "the first study",
    "the first report",
    "the first to",
    "no prior study",
    "no previous study",
    "no prior work",
    "no previous work",
    "has never been",
    "have never been",
];

/// Prepositions that introduce the scope of a claim — the population, setting,
/// material or task it is restricted to.
///
/// **Matched as WHOLE TOKENS, never as substrings.** The first version used
/// `contains("in ")` and matched inside `rema`**`in `**`unexplored`, so a claim
/// using one of this module's own cues read as scoped and the narrowing finding
/// was suppressed. `protein`, `obtain`, `certain`, `domain` and `maintain` do
/// the same. It was found by the test written to explain why a deletion test
/// went green — not by the deletion test, and not by reading.
const SCOPE_PREPOSITIONS: &[&str] =
    &["in", "among", "within", "across", "under", "for", "on", "during", "with"];

/// **Does the claim restrict itself?**
///
/// The cue is removed first, because *"…eco-successional stages for the first
/// time"* contains `for ` and it is part of the cue.
///
/// # THE CUE-STRIPPING IS NOT CURRENTLY LOAD-BEARING, AND THE DELETION TEST IS
/// WHAT SAID SO
///
/// Removing the strip loop and predicting a red test produced a GREEN one. The
/// reason is the content-word condition below: `for the first time` puts `the`
/// after the preposition, `the` is three characters, and the length floor
/// rejects it before the stopword list is consulted. Checked across the whole
/// of [`NOVELTY_CUES`] — **no cue in the list introduces a scope-shaped phrase**,
/// so today the strip loop changes no answer.
///
/// It is kept rather than deleted because the condition that makes it redundant
/// is a property of the cue LIST, not of this function, and a cue such as
/// *"unlike earlier studies in"* would make it load-bearing the day it is
/// added. `no_novelty_cue_looks_like_a_scope_clause_on_its_own` is the test
/// that reaches that property directly and goes red on that day — which is
/// what the deletion test could not do, and the distinction worth recording:
/// a guard whose deletion is invisible needs a test on the premise that makes
/// it invisible, not a louder test on the guard.
pub fn states_its_scope(claim: &NoveltyClaim) -> bool {
    let mut rest = claim.sentence.to_lowercase();
    for cue in NOVELTY_CUES {
        rest = rest.replace(cue, " ");
    }
    let words: Vec<String> = rest
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-').to_string())
        .collect();
    words.windows(2).any(|pair| {
        SCOPE_PREPOSITIONS.contains(&pair[0].as_str())
            // A preposition introduces a scope only if a content word follows.
            && pair[1].len() >= 4
            && !STOPWORDS.contains(&pair[1].as_str())
    })
}

/// One retrieved or cited work, projected for the report.
///
/// `title` is [`crate::refverify::UntrustedText::llm_safe`] output: third-party
/// web text that a synthesis stage may later read, so an injection-flagged
/// title is withheld rather than carried, and `suspicious` says which happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorWorkRef {
    pub title: String,
    pub year: Option<i32>,
    pub doi: Option<String>,
    pub cited_by_count: Option<i64>,
    /// **What it showed, in the work's own abstract.** §4.6 asks for *"the
    /// nearest prior work with what it showed and under what conditions"*, and
    /// a row carrying only a title asks the reader to take the match on trust.
    ///
    /// `None` when OpenAlex has no abstract for the work — which is a fact
    /// about the index and is stated as such rather than left blank. Also
    /// `llm_safe`, so an injection-flagged abstract is withheld rather than
    /// carried into a reviewer report a synthesis stage may later read.
    #[serde(default)]
    pub showed: Option<String>,
    /// `openalex` for a retrieved work, `manuscript-bibliography` for one the
    /// manuscript itself cites.
    pub source: String,
    pub suspicious: bool,
}

impl PriorWorkRef {
    /// One line naming the work, for a report row. **Never truncated** — a
    /// clipped span is not a span, and the same is true of a citation.
    pub fn cite(&self) -> String {
        format!(
            "[{}] {}{}",
            self.year.map(|y| y.to_string()).unwrap_or_else(|| "n.d.".into()),
            self.title,
            self.doi.as_deref().map(|d| format!(" ({d})")).unwrap_or_default()
        )
    }

    /// Title and abstract, lowercased — what a term match is made against.
    pub fn searchable(&self) -> String {
        format!("{} {}", self.title, self.showed.clone().unwrap_or_default()).to_lowercase()
    }

    /// What it showed, or the honest statement that the index has no abstract.
    pub fn what_it_showed(&self) -> String {
        match &self.showed {
            Some(a) => a.clone(),
            None => "OpenAlex carries no abstract for this work, so what it showed is not \
                     stated here. The title and DOI are above; the claim against it has \
                     not been checked on content."
                .into(),
        }
    }
}

/// Which rule decided a status. Named, so a row says why rather than asserting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoveltyBasis {
    /// The manuscript's OWN bibliography carries a work matching the claim.
    CitedWorkMatchesClaim,
    /// A retrieved work carries every distinctive term of the claim.
    RetrievedTitleMatchesClaim,
    /// A retrieved work carries a MAJORITY of the claim's terms while the claim
    /// asserts unrestricted priority. **The paper that narrows it is named.**
    RetrievedWorkNarrowsTheClaim,
    /// Nothing decisive.
    NothingDecisive,
}

/// §4.6's per-claim output. **No number anywhere, by construction** — there is
/// no field a novelty score could be written into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoveltyAssessment {
    pub claim: NoveltyClaim,
    pub status: NoveltyStatus,
    pub basis: NoveltyBasis,
    /// The nearest prior work retrieval found, whether or not it decided
    /// anything. §4.6 asks for it on every row, and a row that shows what was
    /// looked at is refutable where a bare `UNVERIFIED` is not.
    pub nearest_prior_work: Option<PriorWorkRef>,
    /// The narrowing, when the status is `NOVELTY_NARROWER_THAN_STATED`. It
    /// names the paper that narrows the claim — see [`NOVELTY_OVERLAP`].
    pub narrowing: Option<String>,
    /// **A PHRASING observation, not a novelty verdict.**
    ///
    /// A claim saying *"for the first time"* and naming no population, setting,
    /// material or task asserts priority over the whole literature. That is
    /// worth telling an author whatever retrieval found, and it is NOT evidence
    /// that the claim is false — so it travels beside the status rather than
    /// as one. Keeping it separate is what stopped a phrasing observation being
    /// reported as `NOVELTY_NARROWER_THAN_STATED` with an unrelated paper
    /// attached as though that paper had narrowed anything.
    #[serde(default)]
    pub states_no_scope: Option<String>,
    /// **Always present.** Every rule below reasons from term overlap or from
    /// the absence of a phrase, and both need their limits stated.
    pub uncertainty: String,
}

/// **Decide one claim. Pure — no network, no clock.**
///
/// `retrieved` is what [`retrieve`] returned, in OpenAlex's relevance order;
/// `references` is the manuscript's own bibliography.
///
/// # The precedence, and why the manuscript's own bibliography is first
///
/// A manuscript that claims *"no prior study has examined X"* while citing a
/// study of X contradicts itself in its own pages. That finding needs no
/// network, cannot be wrong about the index, and a reader can check it by
/// turning to the reference list. It outranks anything retrieval says.
pub fn assess(
    claim: &NoveltyClaim,
    references: &[crate::extract::citations::Reference],
    retrieved: &[PriorWorkRef],
) -> NoveltyAssessment {
    let nearest = retrieved.first().cloned();

    // A phrasing observation, computed once and carried on every status. It is
    // about how the claim is WORDED and says nothing about whether it is true.
    let lower = claim.sentence.to_lowercase();
    let universal = UNIVERSAL_CUES.iter().find(|c| lower.contains(**c)).copied();
    let states_no_scope = match universal {
        Some(cue) if !states_its_scope(claim) => Some(format!(
            "The claim says \"{cue}\" and names no population, setting, material or task it \
             is restricted to, so as written it asserts priority over the whole literature. \
             This is an observation about the WORDING and is not evidence that the claim is \
             false — state the scope the study actually has."
        )),
        _ => None,
    };

    let finish = |status, basis, nearest, narrowing, uncertainty: String| NoveltyAssessment {
        claim: claim.clone(),
        status,
        basis,
        nearest_prior_work: nearest,
        narrowing,
        states_no_scope: states_no_scope.clone(),
        uncertainty,
    };

    // 1. The manuscript's own bibliography. A manuscript claiming "no prior
    //    study has examined X" while citing a study of X contradicts itself in
    //    its own pages: no network, no index, and a reader checks it by turning
    //    to the reference list.
    if let Some(r) = references.iter().find(|r| {
        r.title.as_deref().map(|t| carries_all_terms(t, &claim.terms)).unwrap_or(false)
    }) {
        return finish(
            NoveltyStatus::PriorWorkExists,
            NoveltyBasis::CitedWorkMatchesClaim,
            Some(PriorWorkRef {
                title: r.title.clone().unwrap_or_default(),
                year: r.year,
                doi: r.doi.clone(),
                cited_by_count: None,
                showed: None,
                source: "manuscript-bibliography".into(),
                suspicious: false,
            }),
            None,
            "Matched on the claim's content terms appearing in the cited title. A title can \
             carry the terms and address a different question, and the reference list is \
             where a reader checks that."
                .into(),
        );
    }

    // 2. A retrieved work carrying EVERY distinctive term.
    if let Some(w) = retrieved.iter().find(|w| carries_all_terms(&w.searchable(), &claim.terms)) {
        return finish(
            NoveltyStatus::PriorWorkExists,
            NoveltyBasis::RetrievedTitleMatchesClaim,
            Some(w.clone()),
            None,
            format!(
                "Every one of the claim's {} content terms appears in this work's title or \
                 abstract. A match inside a long abstract is weaker than one in a title, and \
                 this row does not distinguish them. What the work showed is quoted above; \
                 whether it anticipates THIS claim is a reading nothing here performed.",
                claim.terms.len()
            ),
        );
    }

    // 3. **A claim is NARROWED by a PAPER, not by its own phrasing.** A
    //    retrieved work carrying a majority of the claim's terms, where the
    //    claim asserts unrestricted priority.
    if universal.is_some() && states_no_scope.is_some() {
        if let Some((w, hits)) = best_overlap(retrieved, &claim.terms) {
            if !claim.terms.is_empty()
                && (hits as f64) / (claim.terms.len() as f64) >= NOVELTY_OVERLAP
            {
                return finish(
                    NoveltyStatus::NoveltyNarrowerThanStated,
                    NoveltyBasis::RetrievedWorkNarrowsTheClaim,
                    Some(w.clone()),
                    Some(format!(
                        "The claim asserts unrestricted priority, and {} already covers \
                         {hits} of its {} content terms. What that work showed: {}",
                        w.cite(),
                        claim.terms.len(),
                        w.what_it_showed()
                    )),
                    "The overlap is on content terms, not on meaning: this work is \
                     demonstrably about the same terms, and whether it anticipates the \
                     claim requires reading what it showed under what conditions. The \
                     narrowing is that the claim as written cannot stand over this work, \
                     not that the study's contribution is absent."
                        .into(),
                );
            }
        }
    }

    finish(
        NoveltyStatus::Unverified,
        NoveltyBasis::NothingDecisive,
        nearest,
        None,
        format!(
            "Retrieval returned topically-near work and nothing carrying a majority of the \
             claim's {} content terms. **This is the common outcome and it is not evidence \
             of novelty**: an empty or adjacent result set is a fact about one index and one \
             bag-of-terms query. Deciding whether any retrieved work anticipates the claim \
             requires reading what each showed and under what conditions, which is a \
             judgement this node does not make (§4.4 Tier 3).",
            claim.terms.len()
        ),
    )
}

/// The retrieved work carrying the most of `terms`, with how many.
fn best_overlap<'a>(
    retrieved: &'a [PriorWorkRef],
    terms: &[String],
) -> Option<(&'a PriorWorkRef, usize)> {
    retrieved
        .iter()
        .map(|w| {
            let hay = w.searchable();
            (w, terms.iter().filter(|t| hay.contains(t.as_str())).count())
        })
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0)
}

/// Every one of `terms` appears in `text`, case-insensitively.
///
/// Substring, not word-boundary: `classification` matching inside
/// `classifications` is the behaviour wanted, and a term is at least four
/// characters so an accidental substring hit is unlikely rather than impossible.
fn carries_all_terms(text: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let lower = text.to_lowercase();
    terms.iter().all(|t| lower.contains(t.as_str()))
}

/// **The network half.** Thin on purpose: the decision in [`assess`] is pure
/// and testable, and this is the part that cannot be.
///
/// Goes through [`crate::refverify::openalex_search`], which shares this
/// crate's cache, rate limiter and provenance. Never curl (§11).
pub fn retrieve(
    ctx: &crate::refverify::VerifyContext,
    claim: &NoveltyClaim,
    per_page: usize,
    now: i64,
) -> Result<Vec<PriorWorkRef>, crate::GaplyError> {
    use crate::refverify::ConnectorOutcome;
    let q = retrieval_query(claim);
    if q.trim().is_empty() {
        return Ok(Vec::new());
    }
    match crate::refverify::openalex_search(ctx, &q, per_page, now)? {
        ConnectorOutcome::Found(works) => Ok(works
            .iter()
            .map(|w| PriorWorkRef {
                title: w.title.as_ref().map(|t| t.llm_safe()).unwrap_or_default(),
                year: w.publication_year,
                doi: w.doi.clone(),
                cited_by_count: w.cited_by_count,
                showed: w.abstract_text.as_ref().map(|a| a.llm_safe()),
                source: "openalex".into(),
                suspicious: w.title.as_ref().map(|t| t.is_suspicious()).unwrap_or(false)
                    || w.abstract_text.as_ref().map(|a| a.is_suspicious()).unwrap_or(false),
            })
            .collect()),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(text: &str) -> ExtractionResult {
        crate::extract::extract_from_text(text)
    }

    /// **The whole sentence, not the tail after the cue.** This is the one
    /// property that separates this extractor from `claims.rs`, whose output is
    /// 89% fragments, so it is the property with a test.
    #[test]
    fn a_novelty_claim_carries_its_whole_sentence() {
        let r = ex("Introduction\n\nTo the best of our knowledge, this is the first study to \
                    quantify haemolymph protein turnover in Bombyx mori under thermal stress.\n");
        let claims = extract_claims(&r);
        assert_eq!(claims.len(), 1, "two cues in one sentence is one claim: {claims:?}");
        assert!(
            claims[0].sentence.starts_with("To the best of our knowledge"),
            "span starts at the cue, not before it: {:?}",
            claims[0].sentence
        );
        assert!(
            claims[0].sentence.ends_with("thermal stress."),
            "and runs to the end of the sentence: {:?}",
            claims[0].sentence
        );
    }

    /// **`unprecedented` describing the world is not a novelty claim**, and it
    /// was five of the twelve the first version admitted across 20 real
    /// manuscripts. Every string below is one of those manuscripts' own
    /// sentences.
    #[test]
    fn a_cue_describing_the_world_rather_than_the_paper_is_not_a_claim() {
        for sentence in [
            "China's accession to the WTO in 2001 and its subsequent emergence as the \
             'factory of the world' subjected Indian small industries to unprecedented \
             competitive pressure.",
            "E-commerce and social media marketing offer unprecedented opportunities for \
             reaching global markets without the intermediaries that traditionally \
             captured a large share of artisan income.",
            "It underwent unprecedented growth during the COVID-19 phase and is forecasted \
             to reach a value of USD 10.4 billion by 2025.",
            "The qualitative data suggests that many new registrations represent young \
             entrepreneurs entering the craft sector for the first time.",
            "A structured training programme that teaches artisans how to use e-commerce \
             platforms would address this gap.",
        ] {
            let r = ex(&format!("Introduction\n\n{sentence}\n"));
            assert!(
                extract_claims(&r).is_empty(),
                "this sentence is about the world, not about the paper: {sentence:?}"
            );
        }
    }

    /// **A thesis originality declaration is not a novelty claim**, and it
    /// carries `this work`, so the self-reference rule alone admits it. One real
    /// instance in 20 manuscripts, quoted.
    #[test]
    fn a_thesis_originality_declaration_is_not_a_novelty_claim() {
        let r = ex("Other\n\nThis work has not previously been produced or submitted for \
                    consideration by another candidate for the award of the Ph.D. by any \
                    other person to any other institution.\n");
        assert!(extract_claims(&r).is_empty(), "{:?}", extract_claims(&r));
    }

    /// The two that survive on the real corpus, in the manuscripts' own words.
    #[test]
    fn a_claim_about_this_study_survives_both_gates() {
        for sentence in [
            "The study established a clear experimental threshold documented multi-criteria \
             response and exhibited four identifiable eco-successional stages for the first \
             time.",
            "To the best of our knowledge, authors are unaware of any previous work that \
             hybridizes FA with CSA for hyperparameter optimization in BiLSTM emotion \
             classification, making this the key novelty of this paper.",
        ] {
            let r = ex(&format!("Introduction\n\n{sentence}\n"));
            assert_eq!(extract_claims(&r).len(), 1, "{sentence:?}");
        }
    }

    /// **The recall this costs, named rather than left to be rediscovered.**
    ///
    /// One of the ten suppressed sentences was a true claim: the author's own
    /// proposal, stated without naming whose it is. Requiring a subject is what
    /// removed the five `unprecedented` rows, and it removes this too. The
    /// trade is recorded here so nobody loosens the rule without knowing what
    /// loosening it re-admits.
    #[test]
    fn a_true_claim_with_no_stated_subject_is_the_measured_cost_of_the_rule() {
        let r = ex("Introduction\n\nPricing educational materials in small, affordable \
                    quantities similar to the shampoo sachets used in rural consumer \
                    markets is a novel approach to solve the problem of affordability.\n");
        assert!(
            extract_claims(&r).is_empty(),
            "known miss: a real claim whose sentence names no subject"
        );
    }

    #[test]
    fn a_reference_list_entry_is_not_a_claim_this_manuscript_makes() {
        let r = ex("References\n\nSmith J. The first report of thermal stress in silkworm. 2019.\n");
        assert!(extract_claims(&r).is_empty());
    }

    /// The terms are what retrieval searches on, so a cue word surviving into
    /// them would search for "first" and return the field.
    fn claim(sentence: &str) -> NoveltyClaim {
        let r = ex(&format!("Introduction\n\n{sentence}\n"));
        extract_claims(&r).pop().unwrap_or_else(|| panic!("no claim in {sentence:?}"))
    }

    fn work(title: &str) -> PriorWorkRef {
        PriorWorkRef {
            title: title.into(),
            year: Some(2021),
            doi: None,
            cited_by_count: Some(12),
            showed: Some("An abstract the index happened to carry.".into()),
            source: "openalex".into(),
            suspicious: false,
        }
    }

    /// **The one status no path returns, and the reason.**
    ///
    /// Retrieval finding nothing is a fact about the index. `NOVEL_AS_STATED`
    /// asserts a fact about the field, and nothing deterministic gets from one
    /// to the other. The variant exists because §4.6 names it and a Tier-3
    /// judge reading the nearest prior work will produce it; until then this
    /// test is where switching it on becomes deliberate.
    #[test]
    fn novel_as_stated_is_never_returned_by_retrieval() {
        let c = claim(
            "To the best of our knowledge, no comparable measurement of haemolymph \
             trehalose has been published for this species.",
        );
        for retrieved in [vec![], vec![work("Something entirely unrelated")]] {
            let a = assess(&c, &[], &retrieved);
            assert_ne!(
                a.status,
                NoveltyStatus::NovelAsStated,
                "an empty or unrelated retrieval is not evidence of novelty"
            );
        }
    }

    /// The manuscript contradicting itself outranks anything retrieval says,
    /// and needs no network to find.
    #[test]
    fn a_cited_work_matching_the_claim_outranks_retrieval() {
        let c = claim(
            "To the best of our knowledge, no previous study has examined haemolymph \
             trehalose in Bombyx mori under thermal stress.",
        );
        let cited = crate::extract::citations::Reference {
            raw: "Rao et al. 2019".into(),
            authors: "Rao".into(),
            year: Some(2019),
            title: Some(
                "Haemolymph trehalose dynamics in Bombyx mori under thermal stress".into(),
            ),
            doi: None,
        };
        let a = assess(&c, std::slice::from_ref(&cited), &[work("An unrelated paper")]);
        assert_eq!(a.status, NoveltyStatus::PriorWorkExists);
        assert_eq!(a.basis, NoveltyBasis::CitedWorkMatchesClaim);
        assert_eq!(a.nearest_prior_work.unwrap().source, "manuscript-bibliography");
    }

    /// **A claim is NARROWED by a PAPER, not by its own phrasing.**
    ///
    /// The earlier rule returned `NOVELTY_NARROWER_THAN_STATED` from the claim
    /// sentence alone and attached whatever retrieval ranked first as
    /// "context". On `final final L.pdf` that meant a limnology claim was
    /// reported as narrowed beside *"Pattern-oriented modelling: a
    /// 'multi-scope' for predictive systems ecology"* — a paper that narrows
    /// nothing. A narrowing that cannot name the work doing the narrowing is an
    /// assertion, so the status now needs a majority overlap and the row cites
    /// the paper and quotes what it showed.
    #[test]
    fn a_majority_overlap_narrows_a_claim_and_names_the_paper() {
        // `final final L.pdf`'s real claim, whole. Seven content terms.
        let c = claim(
            "The study established a clear experimental threshold documented multi-criteria \
             response and exhibited four identifiable eco-successional stages for the first \
             time.",
        );
        assert_eq!(c.terms.len(), 7, "{:?}", c.terms);
        // Carries five of the seven — a majority, not all. Matching ALL of them
        // is `PRIOR_WORK_EXISTS`, which is a different and stronger row; the
        // first version of this fixture matched all three terms of a shortened
        // claim and proved that instead.
        let covering = PriorWorkRef {
            title: "Four eco-successional stages identified in restored urban ponds".into(),
            year: Some(2019),
            doi: Some("https://doi.org/10.0000/example".into()),
            cited_by_count: Some(88),
            showed: Some(
                "We document a multi-criteria response across four successional stages in \
                 restored urban ponds."
                    .into(),
            ),
            source: "openalex".into(),
            suspicious: false,
        };
        let a = assess(&c, &[], std::slice::from_ref(&covering));
        assert_eq!(a.status, NoveltyStatus::NoveltyNarrowerThanStated, "{a:?}");
        assert_eq!(a.basis, NoveltyBasis::RetrievedWorkNarrowsTheClaim);
        let n = a.narrowing.as_deref().unwrap();
        assert!(n.contains("Four eco-successional stages identified"), "names the paper: {n}");
        assert!(n.contains("10.0000/example"), "with its DOI: {n}");
        assert!(
            n.contains("multi-criteria response across four successional stages"),
            "and what it showed: {n}"
        );
    }

    /// **The same claim with an unrelated retrieval is UNVERIFIED**, and the
    /// phrasing observation travels beside it rather than as a verdict. This is
    /// what BOTH real claims in a 20-manuscript corpus do.
    #[test]
    fn an_unqualified_claim_with_no_covering_paper_is_unverified_not_narrowed() {
        // The real claim, and a work at the overlap the CORPUS actually
        // produced: 3 of 7 terms, the highest seen across 20 real retrievals.
        // A fixture with ZERO overlap would never reach the threshold at all —
        // which is what the first version of this test did, and the deletion
        // test caught it by going green where red was predicted.
        let c = claim(
            "The study established a clear experimental threshold documented multi-criteria \
             response and exhibited four identifiable eco-successional stages for the first \
             time.",
        );
        let adjacent = PriorWorkRef {
            title: "Pattern-oriented modelling: a multi-scope for predictive systems ecology"
                .into(),
            year: Some(2011),
            doi: None,
            cited_by_count: Some(300),
            showed: Some(
                "An experimental response surface across four model structures.".into(),
            ),
            source: "openalex".into(),
            suspicious: false,
        };
        let hits = c
            .terms
            .iter()
            .filter(|t| adjacent.searchable().contains(t.as_str()))
            .count();
        assert_eq!(hits, 3, "precondition: the corpus's own maximum overlap, {:?}", c.terms);
        let a = assess(&c, &[], std::slice::from_ref(&adjacent));
        assert_eq!(a.status, NoveltyStatus::Unverified, "{a:?}");
        assert!(a.narrowing.is_none(), "nothing narrowed it: {a:?}");
        assert!(
            a.states_no_scope.is_some(),
            "the phrasing observation still travels, as an observation: {a:?}"
        );
        assert!(
            a.states_no_scope.as_deref().unwrap().contains("not evidence that the claim is false"),
            "and it says what it is not"
        );
        assert!(
            a.uncertainty.contains("not evidence of novelty"),
            "UNVERIFIED must say it is not a clean bill: {}",
            a.uncertainty
        );
    }

    /// A claim that names its own scope carries no phrasing observation.
    #[test]
    fn a_scoped_claim_carries_no_phrasing_observation() {
        let c = claim(
            "This is the first study to characterise eco-successional staging in shallow \
             urban freshwater lakes of the Deccan plateau.",
        );
        let a = assess(&c, &[], &[work("Succession in restored urban streams")]);
        assert_eq!(a.status, NoveltyStatus::Unverified, "{a:?}");
        assert!(a.states_no_scope.is_none(), "{a:?}");
    }

    /// **The hedge is not the assertion.** *"To the best of our knowledge"*
    /// concedes the author's limits; flagging it while letting *"for the first
    /// time"* through would punish the honest phrasing.
    #[test]
    fn a_hedged_claim_is_not_treated_as_a_universal_one() {
        let c = claim(
            "To the best of our knowledge this combination has not been evaluated \
             elsewhere.",
        );
        let a = assess(&c, &[], &[]);
        assert_ne!(a.status, NoveltyStatus::NoveltyNarrowerThanStated, "{a:?}");
    }

    /// The query is what retrieval is built from, and the first version of it
    /// dropped exactly the tokens that carried the claim.
    #[test]
    fn the_query_keeps_acronyms_and_caps_its_length() {
        let c = claim(
            "To the best of our knowledge, no previous work hybridizes FA with CSA for \
             hyperparameter optimization in BiLSTM emotion classification.",
        );
        let q = retrieval_query(&c);
        assert!(q.contains("FA"), "{q:?}");
        assert!(q.contains("CSA"), "{q:?}");
        assert!(
            q.split_whitespace().count() <= MAX_QUERY_TERMS + 3,
            "a long query dilutes relevance ranking: {q:?}"
        );
    }

    #[test]
    fn cue_words_and_stopwords_are_not_retrieval_terms() {
        let t = distinctive_terms(
            "To the best of our knowledge, this is the first study to quantify haemolymph \
             protein turnover in Bombyx mori.",
        );
        assert!(t.contains(&"haemolymph".to_string()), "{t:?}");
        assert!(t.contains(&"bombyx".to_string()), "{t:?}");
        assert!(!t.iter().any(|x| x == "first" || x == "study" || x == "knowledge"), "{t:?}");
    }
}
