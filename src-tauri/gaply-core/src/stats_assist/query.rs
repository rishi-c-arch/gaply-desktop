//! **The web query: built locally from templates, editable by the user, and
//! scanned locally before any preview.**
//!
//! The template is the method's name, the variant's words when one is stated,
//! and one topic from a closed list. The user may edit it. The edit is
//! scanned, and the preview is blocked with a closed reason when it adds:
//!
//! - a column header or a level label (code or value label) of 4 or more
//!   characters, as a whole word, case-folded; or
//! - a run of 6 words copied from a manuscript sentence.
//!
//! The template's own text is not scanned: a header made only of template
//! words, or a 6-word run made only of template words, does not block. So the
//! unedited template never blocks, whatever the data or the manuscript say.
//!
//! The only way to get a [`CheckedQuery`], which the `web_context` builder
//! requires, is [`check_query`].

use std::collections::HashSet;

use crate::extract::sentence::sentences_in;
use crate::extract::ExtractionResult;
use crate::stats_l1::catalogue::{contains_word, ContinuityVariant, L1MethodId, SpearmanPVariant, TVariant, Variant};
use crate::stats_l1::dataset::SchemaSummary;

/// A header or label shorter than this is not scanned for.
pub const MIN_SCANNED_CHARS: usize = 4;
/// The length of a copied word run that blocks.
pub const COPIED_RUN_WORDS: usize = 6;
/// The longest query, in characters.
pub const MAX_QUERY_CHARS: usize = 300;

/// The closed topic list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTopic {
    Assumptions,
    ReportingGuidance,
    EffectSizeReporting,
    ChoosingAVariant,
}

impl ContextTopic {
    pub const ALL: [ContextTopic; 4] =
        [ContextTopic::Assumptions, ContextTopic::ReportingGuidance, ContextTopic::EffectSizeReporting, ContextTopic::ChoosingAVariant];
}

fn method_words(m: L1MethodId) -> &'static str {
    match m {
        L1MethodId::IndependentT => "independent samples t-test",
        L1MethodId::PairedT => "paired t-test",
        L1MethodId::OneWayAnova => "one-way ANOVA",
        L1MethodId::Pearson => "Pearson correlation",
        L1MethodId::Spearman => "Spearman rank correlation",
        L1MethodId::ChiSquareIndependence => "chi-square test of independence",
        L1MethodId::Ols => "linear regression",
    }
}

fn variant_words(v: Variant) -> Option<&'static str> {
    match v {
        Variant::T(TVariant::Student) => Some("Student's pooled variance"),
        Variant::T(TVariant::Welch) => Some("Welch's unequal variances"),
        Variant::Continuity(ContinuityVariant::Yes) => Some("with Yates continuity correction"),
        Variant::Continuity(ContinuityVariant::No) => Some("without continuity correction"),
        Variant::SpearmanP(SpearmanPVariant::TApproximation) => Some("t approximation p-value"),
        Variant::SpearmanP(SpearmanPVariant::Exact) => Some("exact p-value"),
        _ => None,
    }
}

fn topic_words(topic: ContextTopic, v: Variant) -> &'static str {
    match topic {
        ContextTopic::Assumptions => "assumptions",
        ContextTopic::ReportingGuidance => "reporting guidelines",
        ContextTopic::EffectSizeReporting => "effect size reporting",
        ContextTopic::ChoosingAVariant => match v {
            Variant::T(_) => "choosing Student or Welch",
            Variant::Continuity(_) => "when to use a continuity correction",
            Variant::SpearmanP(_) => "exact or approximate p-value",
            Variant::None => "choosing the right test",
        },
    }
}

/// The template for a method, its variant and a topic. Built only from the
/// words above: never from the schema, headers, rows, results or manuscript.
pub fn template_query(method: L1MethodId, variant: Variant, topic: ContextTopic) -> String {
    let mut parts = vec![method_words(method)];
    if topic != ContextTopic::ChoosingAVariant {
        parts.extend(variant_words(variant));
    }
    parts.push(topic_words(topic, variant));
    parts.join(" ")
}

/// Why an edited query was blocked. Closed; the blocking text is never
/// echoed, since it may be a header or a manuscript sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryBlocked {
    Empty,
    TooLong { chars: usize, max: usize },
    /// The edit names a column header (4+ characters) as a whole word.
    ContainsColumnHeader,
    /// The edit names a level code or value label (4+ characters) as a whole
    /// word.
    ContainsLevelLabel,
    /// The edit copies a run of `words` words from a manuscript sentence.
    CopiesManuscriptWords { words: usize },
}

impl QueryBlocked {
    /// What the user is told.
    pub fn words(self) -> &'static str {
        match self {
            QueryBlocked::Empty => "The query is empty.",
            QueryBlocked::TooLong { .. } => "The query is longer than 300 characters.",
            QueryBlocked::ContainsColumnHeader => "The query contains one of your column headers. Remove it: headers stay on this device.",
            QueryBlocked::ContainsLevelLabel => "The query contains a group or level name from your data. Remove it: data values stay on this device.",
            QueryBlocked::CopiesManuscriptWords { .. } => {
                "The query copies six or more words in a row from your manuscript. Rephrase it: manuscript text stays on this device."
            }
        }
    }
}

/// What the edit is scanned against. Local only.
pub struct QueryScan<'a> {
    headers: Vec<&'a str>,
    labels: Vec<&'a str>,
    runs: HashSet<String>,
}

/// Lower-case alphanumeric words.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

impl<'a> QueryScan<'a> {
    /// Headers and level labels from the schema; sentences from every
    /// paragraph of the manuscript, plus any extra sentences (a user-entered
    /// claim's sentence).
    pub fn new(schema: &'a SchemaSummary, manuscript: Option<&ExtractionResult>, extra_sentences: &[&str]) -> QueryScan<'a> {
        let headers = schema.columns.iter().map(|c| c.header.as_str()).filter(|h| h.trim().chars().count() >= MIN_SCANNED_CHARS).collect();
        let mut labels: Vec<&str> = Vec::new();
        for c in &schema.columns {
            if let Some(levels) = &c.levels {
                for l in &levels.levels {
                    labels.push(l.code.as_str());
                    if let Some(label) = &l.label {
                        labels.push(label.as_str());
                    }
                }
            }
        }
        labels.retain(|l| l.trim().chars().count() >= MIN_SCANNED_CHARS);
        let mut runs = HashSet::new();
        let mut add = |sentence: &str| {
            let w = words(sentence);
            for win in w.windows(COPIED_RUN_WORDS) {
                runs.insert(win.join(" "));
            }
        };
        if let Some(ex) = manuscript {
            for s in &ex.sections {
                for p in &s.paragraphs {
                    for sentence in sentences_in(p) {
                        add(sentence);
                    }
                }
            }
        }
        for s in extra_sentences {
            add(s);
        }
        QueryScan { headers, labels, runs }
    }
}

/// A query the scan let through. Only [`check_query`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedQuery(String);

impl CheckedQuery {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Scan `edited` against `scan`, ignoring what `template` already says.
pub fn check_query(template: &str, edited: &str, scan: &QueryScan<'_>) -> Result<CheckedQuery, QueryBlocked> {
    let q = edited.trim();
    if q.is_empty() {
        return Err(QueryBlocked::Empty);
    }
    let chars = q.chars().count();
    if chars > MAX_QUERY_CHARS {
        return Err(QueryBlocked::TooLong { chars, max: MAX_QUERY_CHARS });
    }
    let template_words: HashSet<String> = words(template).into_iter().collect();
    let only_template = |text: &str| words(text).iter().all(|w| template_words.contains(w));

    if scan.headers.iter().any(|h| !only_template(h) && contains_word(q, h.trim())) {
        return Err(QueryBlocked::ContainsColumnHeader);
    }
    if scan.labels.iter().any(|l| !only_template(l) && contains_word(q, l.trim())) {
        return Err(QueryBlocked::ContainsLevelLabel);
    }
    let qw = words(q);
    for win in qw.windows(COPIED_RUN_WORDS) {
        if win.iter().all(|w| template_words.contains(w)) {
            continue;
        }
        if scan.runs.contains(&win.join(" ")) {
            return Err(QueryBlocked::CopiesManuscriptWords { words: COPIED_RUN_WORDS });
        }
    }
    Ok(CheckedQuery(q.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_name_the_method_the_variant_and_the_topic() {
        assert_eq!(
            template_query(L1MethodId::OneWayAnova, Variant::None, ContextTopic::Assumptions),
            "one-way ANOVA assumptions"
        );
        assert_eq!(
            template_query(L1MethodId::IndependentT, Variant::T(TVariant::Welch), ContextTopic::EffectSizeReporting),
            "independent samples t-test Welch's unequal variances effect size reporting"
        );
        assert_eq!(
            template_query(L1MethodId::IndependentT, Variant::T(TVariant::NotStated), ContextTopic::ChoosingAVariant),
            "independent samples t-test choosing Student or Welch"
        );
        for m in L1MethodId::ALL {
            for t in ContextTopic::ALL {
                let q = template_query(m, Variant::None, t);
                assert!(q.chars().count() <= MAX_QUERY_CHARS && !q.is_empty());
            }
        }
    }
}
