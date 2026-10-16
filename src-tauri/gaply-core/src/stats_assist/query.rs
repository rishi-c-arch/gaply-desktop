//! **The web query: built only from closed parts.**
//!
//! The query is the method's name, the variant's words when one is chosen,
//! and one topic from a closed list, through the templates below. Nothing
//! else can enter it: there is no free-text edit (evaluation fixes, part B).
//! A scanner cannot know every cell, header or sentence a user might type,
//! so the query stopped being typed. The screen shows it as read-only text
//! with pickers for the three parts.
//!
//! The only way to get a [`WebQuery`], which the `web_context` builder
//! requires, is [`web_query`].

use crate::stats_l1::catalogue::{ContinuityVariant, L1MethodId, SpearmanPVariant, TVariant, Variant};

/// The longest query, in characters (every template is shorter).
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

/// A query built from closed parts. Only [`web_query`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebQuery(String);

impl WebQuery {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The query for a method, a variant (or none) and a topic: the template, and
/// nothing else.
pub fn web_query(method: L1MethodId, variant: Variant, topic: ContextTopic) -> WebQuery {
    WebQuery(template_query(method, variant, topic))
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
                let q = web_query(m, Variant::None, t);
                assert!(q.as_str().chars().count() <= MAX_QUERY_CHARS && !q.as_str().is_empty());
            }
        }
    }
}
