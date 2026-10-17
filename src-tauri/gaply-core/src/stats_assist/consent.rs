//! **Data categories, opt-ins, the AI help setting and consent grants.**
//!
//! Each task sends one base category and may send opt-in categories, each off
//! by default:
//!
//! | task | base | opt-ins |
//! |---|---|---|
//! | `claim_method` | the result's typed fields | the claim's sentence; a Methods passage |
//! | `binding_suggestion` | the aliased schema | original column headers; the claim's sentence |
//! | `result_explanation` | the finished result's enumerated fields | none |
//! | `web_context` | the search query | the paper's DOI (a DOI only) |
//!
//! An opt-in reaches a builder only as an [`Allowed`] value, and the only way
//! to make one is through [`OptIns`], which hands it out only when the user
//! ticked that category for that task. So a header, the sentence, the passage
//! or the DOI cannot enter a request by a builder argument unless the matching
//! category is selected: the type requires it.
//!
//! A [`ConsentGrant`] is made by the session when the user sends a preview,
//! and lasts for the run unless revoked. It is never persisted.
//!
//! **Evaluation fixes, part B.** The DOI-or-title opt-in became DOI only: a
//! title is free text, and free text can carry anything a scanner cannot
//! know. [`OptIns::doi`] accepts only `10.NNNN/…` (ASCII, at most 200
//! characters, after trimming a leading `https://doi.org/` or `doi:`), and
//! blocks it when, after folding both sides (NFKC, case-folded, zero-width
//! and other format characters removed), any header, level label, text cell
//! or the file name of 4 or more characters appears inside it. The claim
//! sentence opt-in carries only a sentence that is in the manuscript.

use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;

use super::spec::HelperTask;
use crate::extract::sentence::sentences_in;
use crate::extract::{ExtractionResult, SectionKind};
use crate::stats_l1::claim::L1Claim;
use crate::stats_l1::dataset::{Cell, Dataset, SchemaSummary};

/// The longest Methods passage a user may select, in characters.
pub const MAX_PASSAGE_CHARS: usize = 1_000;
/// Each original header is cut to this many characters.
pub const MAX_HEADER_CHARS: usize = 64;
/// The longest DOI a user may type, in characters, after trimming.
pub const MAX_DOI_CHARS: usize = 200;
/// A value shorter than this is not looked for inside a DOI.
pub const MIN_SCANNED_CHARS: usize = 4;
/// The longest claim sentence that may be sent, in characters.
pub const MAX_SENTENCE_CHARS: usize = 2_000;

/// What a request may carry. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DataCategory {
    /// `claim_method`'s base: statistic symbol, printed statistic, df and p,
    /// tails note, stated alpha, method words from the closed list, and the
    /// catalogue.
    ResultFields,
    /// `binding_suggestion`'s base: the method, its role slots, and the
    /// aliased schema (alias, type, has-missing, level bucket).
    AliasedSchema,
    /// `result_explanation`'s base: the finished Level 1 result's enumerated
    /// fields.
    FinishedResult,
    /// `web_context`'s base: the query.
    SearchQuery,
    /// Opt-in: original column headers, each cut to 64 characters.
    ColumnHeaders,
    /// Opt-in: the sentence that holds the result.
    ClaimSentence,
    /// Opt-in: one passage of at most 1,000 characters from the Methods.
    MethodsPassage,
    /// Opt-in: the paper's DOI, normalised; a DOI only.
    PaperDoi,
}

impl DataCategory {
    /// What the user is told the request carries.
    pub fn words(self) -> &'static str {
        match self {
            DataCategory::ResultFields => {
                "The result's printed statistic, df and p, the tails note and stated alpha, method words found by Gaply's fixed list, and Gaply's method list"
            }
            DataCategory::AliasedSchema => {
                "The method and its roles, and each column's alias (C1, C2, …), type, whether it has missing values and how many levels it has (bucketed). No headers, labels or values"
            }
            DataCategory::FinishedResult => {
                "The finished check of this result: method and variant, verdict, the printed and computed numbers with their intervals and statuses, n used, exclusion counts, and note codes"
            }
            DataCategory::SearchQuery => "The search query shown",
            DataCategory::ColumnHeaders => "Your original column headers, each cut to 64 characters",
            DataCategory::ClaimSentence => "The sentence in your manuscript that reports this result",
            DataCategory::MethodsPassage => "The Methods passage you selected (at most 1,000 characters)",
            DataCategory::PaperDoi => "The paper's DOI you typed (a DOI only, checked to hold nothing from your data)",
        }
    }
}

/// The category every request of `task` carries.
pub fn base_category(task: HelperTask) -> DataCategory {
    match task {
        HelperTask::ClaimMethod => DataCategory::ResultFields,
        HelperTask::BindingSuggestion => DataCategory::AliasedSchema,
        HelperTask::ResultExplanation => DataCategory::FinishedResult,
        HelperTask::WebContext => DataCategory::SearchQuery,
    }
}

/// The categories a user may opt in to for `task`.
pub fn opt_in_categories(task: HelperTask) -> &'static [DataCategory] {
    match task {
        HelperTask::ClaimMethod => &[DataCategory::ClaimSentence, DataCategory::MethodsPassage],
        HelperTask::BindingSuggestion => &[DataCategory::ColumnHeaders, DataCategory::ClaimSentence],
        HelperTask::ResultExplanation => &[],
        HelperTask::WebContext => &[DataCategory::PaperDoi],
    }
}

/// A set of categories, in a fixed order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategorySet(BTreeSet<DataCategory>);

impl CategorySet {
    pub fn of(cats: impl IntoIterator<Item = DataCategory>) -> CategorySet {
        CategorySet(cats.into_iter().collect())
    }
    pub fn contains(&self, c: DataCategory) -> bool {
        self.0.contains(&c)
    }
    pub fn covers(&self, other: &CategorySet) -> bool {
        other.0.is_subset(&self.0)
    }
    pub fn iter(&self) -> impl Iterator<Item = DataCategory> + '_ {
        self.0.iter().copied()
    }
    pub fn words(&self) -> Vec<&'static str> {
        self.iter().map(DataCategory::words).collect()
    }
    fn extend(&mut self, other: &CategorySet) {
        self.0.extend(other.0.iter().copied());
    }
}

/* ------------------------------------------------------------------------ *
 *  Opt-ins
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptInRefusal {
    /// The category is not an opt-in for this task.
    NotAnOptInFor { task: HelperTask, category: DataCategory },
    /// The passage is longer than the limit.
    PassageTooLong { chars: usize, max: usize },
    /// The passage is not text from a Methods section of this manuscript.
    PassageNotInMethods,
    /// The text is not a DOI (`10.NNNN/…`, ASCII, at most 200 characters).
    DoiMalformed,
    /// The DOI holds a header, a level label, a text cell or the file name.
    DoiContainsData,
    /// The claim's sentence is not a sentence of the manuscript (a sentence
    /// typed with an added claim).
    SentenceNotInManuscript,
    /// Empty text where text was opted in.
    Empty,
    /// The claim has no sentence to send.
    NoSentence,
    /// The sentence is longer than the limit.
    SentenceTooLong { chars: usize, max: usize },
}

/// The user's ticks for one task. Built from a list of categories, each of
/// which must be an opt-in for that task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptIns {
    task: HelperTask,
    selected: CategorySet,
}

/// An opt-in value: only [`OptIns`] can make one, and only for a ticked
/// category. It remembers the task it was ticked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowed<T> {
    task: HelperTask,
    value: T,
}

impl<T> Allowed<T> {
    pub fn task(&self) -> HelperTask {
        self.task
    }
    pub(crate) fn value(&self) -> &T {
        &self.value
    }
}

/// Original headers by alias, each cut to [`MAX_HEADER_CHARS`].
#[derive(Clone, PartialEq, Eq)]
pub struct ColumnHeaders(pub(crate) Vec<(String, String)>);

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for ColumnHeaders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColumnHeaders").field("headers", &self.0.len()).finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClaimSentence(pub(crate) String);

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for ClaimSentence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaimSentence").field("chars", &self.0.chars().count()).finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MethodsPassage(pub(crate) String);

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for MethodsPassage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MethodsPassage").field("chars", &self.0.chars().count()).finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Doi(pub(crate) String);

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for Doi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Doi").field("chars", &self.0.chars().count()).finish_non_exhaustive()
    }
}

/// Fold text for the DOI scan: NFKC, case-folded, zero-width and other
/// format characters removed.
pub fn fold(text: &str) -> String {
    text.nfkc().filter(|c| !is_format_char(*c)).flat_map(char::to_lowercase).collect()
}

/// Zero-width and other invisible format characters (Unicode Cf and the
/// joiners): never part of a value a user can see.
fn is_format_char(c: char) -> bool {
    matches!(c,
        '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{206F}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}')
}

/// What a DOI is scanned against: every header, level code and label, text
/// cell and the file name, folded, of 4 or more characters. Local only.
pub struct DoiScan {
    values: Vec<String>,
}

impl DoiScan {
    pub fn new(dataset: &Dataset, file_name: Option<&str>) -> DoiScan {
        let mut values: Vec<String> = Vec::new();
        for c in &dataset.schema.columns {
            values.push(c.header.clone());
            if let Some(levels) = &c.levels {
                for l in &levels.levels {
                    values.push(l.code.clone());
                    values.extend(l.label.clone());
                }
            }
            if let Some(cells) = dataset.column_cells(c.alias) {
                for cell in cells {
                    if let Cell::Text(t) = cell {
                        values.push(t.clone());
                    }
                }
            }
        }
        if let Some(f) = file_name {
            values.push(f.to_string());
            if let Some((stem, _)) = f.rsplit_once('.') {
                values.push(stem.to_string());
            }
        }
        let mut values: Vec<String> = values.iter().map(|v| fold(v.trim())).filter(|v| v.chars().count() >= MIN_SCANNED_CHARS).collect();
        values.sort();
        values.dedup();
        DoiScan { values }
    }
}

/// The DOI in `typed`, normalised, if it is one: a leading `https://doi.org/`
/// or `doi:` and surrounding spaces trimmed, ASCII only, `^10\.\d{4,9}/\S+$`,
/// at most [`MAX_DOI_CHARS`].
pub fn normalise_doi(typed: &str) -> Option<String> {
    let t = typed.trim();
    let lower = t.to_ascii_lowercase();
    let t = ["https://doi.org/", "doi:"].iter().find(|p| lower.starts_with(*p)).map_or(t, |p| t[p.len()..].trim_start());
    if t.is_empty() || !t.is_ascii() || t.chars().count() > MAX_DOI_CHARS {
        return None;
    }
    let rest = t.strip_prefix("10.")?;
    let (registrant, suffix) = rest.split_once('/')?;
    let ok = (4..=9).contains(&registrant.len())
        && registrant.bytes().all(|b| b.is_ascii_digit())
        && !suffix.is_empty()
        && !suffix.chars().any(char::is_whitespace);
    ok.then(|| t.to_string())
}

impl OptIns {
    /// No opt-in: the default.
    pub fn none(task: HelperTask) -> OptIns {
        OptIns { task, selected: CategorySet::default() }
    }

    pub fn new(task: HelperTask, ticked: &[DataCategory]) -> Result<OptIns, OptInRefusal> {
        for &c in ticked {
            if !opt_in_categories(task).contains(&c) {
                return Err(OptInRefusal::NotAnOptInFor { task, category: c });
            }
        }
        Ok(OptIns { task, selected: CategorySet::of(ticked.iter().copied()) })
    }

    pub fn task(&self) -> HelperTask {
        self.task
    }

    pub fn is_ticked(&self, c: DataCategory) -> bool {
        self.selected.contains(c)
    }

    fn allow<T>(&self, c: DataCategory, value: T) -> Option<Allowed<T>> {
        self.is_ticked(c).then_some(Allowed { task: self.task, value })
    }

    /// The headers, by alias, cut to 64 characters, if ticked.
    pub fn column_headers(&self, schema: &SchemaSummary) -> Option<Allowed<ColumnHeaders>> {
        if !self.is_ticked(DataCategory::ColumnHeaders) {
            return None;
        }
        let headers =
            schema.columns.iter().map(|c| (c.alias.name(), c.header.chars().take(MAX_HEADER_CHARS).collect())).collect();
        self.allow(DataCategory::ColumnHeaders, ColumnHeaders(headers))
    }

    /// The claim's own sentence, if ticked: a sentence of a paragraph of
    /// `ex`, the manuscript, and nothing else (a sentence typed with an added
    /// claim is not the manuscript's).
    pub fn claim_sentence(&self, claim: &L1Claim, ex: &ExtractionResult) -> Result<Option<Allowed<ClaimSentence>>, OptInRefusal> {
        if !self.is_ticked(DataCategory::ClaimSentence) {
            return Ok(None);
        }
        let s = claim.sentence.trim();
        if s.is_empty() {
            return Err(OptInRefusal::NoSentence);
        }
        let in_manuscript =
            ex.sections.iter().any(|sec| sec.paragraphs.iter().any(|p| sentences_in(p).into_iter().any(|x| x.trim() == s)));
        if !in_manuscript {
            return Err(OptInRefusal::SentenceNotInManuscript);
        }
        let chars = s.chars().count();
        if chars > MAX_SENTENCE_CHARS {
            return Err(OptInRefusal::SentenceTooLong { chars, max: MAX_SENTENCE_CHARS });
        }
        Ok(self.allow(DataCategory::ClaimSentence, ClaimSentence(s.to_string())))
    }

    /// A passage the user selected, if ticked: at most 1,000 characters, and
    /// text that occurs inside one paragraph of a Methods section of `ex`.
    pub fn methods_passage(&self, ex: &ExtractionResult, passage: &str) -> Result<Option<Allowed<MethodsPassage>>, OptInRefusal> {
        if !self.is_ticked(DataCategory::MethodsPassage) {
            return Ok(None);
        }
        let p = passage.trim();
        if p.is_empty() {
            return Err(OptInRefusal::Empty);
        }
        let chars = p.chars().count();
        if chars > MAX_PASSAGE_CHARS {
            return Err(OptInRefusal::PassageTooLong { chars, max: MAX_PASSAGE_CHARS });
        }
        let in_methods = ex
            .sections
            .iter()
            .filter(|s| s.kind == SectionKind::Methods)
            .any(|s| s.paragraphs.iter().any(|para| para.contains(p)));
        if !in_methods {
            return Err(OptInRefusal::PassageNotInMethods);
        }
        Ok(self.allow(DataCategory::MethodsPassage, MethodsPassage(p.to_string())))
    }

    /// The DOI, if ticked: normalised by [`normalise_doi`], and blocked when
    /// any value of `scan` appears inside it once both are folded.
    pub fn doi(&self, typed: &str, scan: &DoiScan) -> Result<Option<Allowed<Doi>>, OptInRefusal> {
        if !self.is_ticked(DataCategory::PaperDoi) {
            return Ok(None);
        }
        if typed.trim().is_empty() {
            return Err(OptInRefusal::Empty);
        }
        let doi = normalise_doi(typed).ok_or(OptInRefusal::DoiMalformed)?;
        let folded = fold(&doi);
        if scan.values.iter().any(|v| folded.contains(v.as_str())) {
            return Err(OptInRefusal::DoiContainsData);
        }
        Ok(self.allow(DataCategory::PaperDoi, Doi(doi)))
    }
}

/* ------------------------------------------------------------------------ *
 *  The setting and grants
 * ------------------------------------------------------------------------ */

/// The "AI help" setting, enforced here and not in the webview. Both switches
/// are off by default; web context is its own switch inside AI help.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AiHelpSetting {
    pub ai_help: bool,
    pub web_context: bool,
}

impl AiHelpSetting {
    pub fn allows(self, task: HelperTask) -> bool {
        match task {
            HelperTask::WebContext => self.ai_help && self.web_context,
            _ => self.ai_help,
        }
    }
}

/// A run's id, given by the caller. Grants never outlive it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HelperRunId(pub u64);

/// Consent for one task and its categories in one run. Made only by the
/// session when the user sends a preview. Fields private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentGrant {
    run: HelperRunId,
    task: HelperTask,
    categories: CategorySet,
}

impl ConsentGrant {
    pub(crate) fn new(run: HelperRunId, task: HelperTask, categories: CategorySet) -> ConsentGrant {
        ConsentGrant { run, task, categories }
    }
    pub(crate) fn widen(&mut self, more: &CategorySet) {
        self.categories.extend(more);
    }
    pub fn run(&self) -> HelperRunId {
        self.run
    }
    pub fn task(&self) -> HelperTask {
        self.task
    }
    pub fn categories(&self) -> &CategorySet {
        &self.categories
    }
    /// The grant authorises a request of `task` in `run` carrying `cats`.
    pub fn covers(&self, run: HelperRunId, task: HelperTask, cats: &CategorySet) -> bool {
        self.run == run && self.task == task && self.categories.covers(cats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_ins_are_per_task_and_off_by_default() {
        assert!(OptIns::new(HelperTask::ResultExplanation, &[DataCategory::ClaimSentence]).is_err());
        assert_eq!(
            OptIns::new(HelperTask::ClaimMethod, &[DataCategory::ColumnHeaders]),
            Err(OptInRefusal::NotAnOptInFor { task: HelperTask::ClaimMethod, category: DataCategory::ColumnHeaders })
        );
        let o = OptIns::none(HelperTask::BindingSuggestion);
        assert!(!o.is_ticked(DataCategory::ColumnHeaders));
        assert!(normalise_doi("https://doi.org/10.1000/x1").is_some());
        for t in HelperTask::ALL {
            assert!(!opt_in_categories(t).contains(&base_category(t)));
        }
    }

    #[test]
    fn the_setting_is_off_by_default_and_web_needs_both_switches() {
        let s = AiHelpSetting::default();
        assert!(HelperTask::ALL.iter().all(|t| !s.allows(*t)));
        let s = AiHelpSetting { ai_help: true, web_context: false };
        assert!(s.allows(HelperTask::ClaimMethod) && !s.allows(HelperTask::WebContext));
        let s = AiHelpSetting { ai_help: false, web_context: true };
        assert!(!s.allows(HelperTask::WebContext));
    }

    #[test]
    fn a_grant_covers_only_its_run_task_and_categories() {
        let g = ConsentGrant::new(HelperRunId(1), HelperTask::BindingSuggestion, CategorySet::of([DataCategory::AliasedSchema]));
        let base = CategorySet::of([DataCategory::AliasedSchema]);
        assert!(g.covers(HelperRunId(1), HelperTask::BindingSuggestion, &base));
        assert!(!g.covers(HelperRunId(2), HelperTask::BindingSuggestion, &base));
        assert!(!g.covers(HelperRunId(1), HelperTask::ClaimMethod, &base));
        let more = CategorySet::of([DataCategory::AliasedSchema, DataCategory::ColumnHeaders]);
        assert!(!g.covers(HelperRunId(1), HelperTask::BindingSuggestion, &more));
    }
}
