//! **The local proposer: deterministic binding suggestions from column types,
//! level buckets and header words. No network, no model, no fuzzy matching.**
//!
//! For a claim it picks the methods its family allows (a `t` is an independent
//! or a paired t, an `F` an ANOVA or an OLS overall F, and so on, decided by
//! the closed [`METHOD_WORDS`] list over the claim's sentence), fills each
//! role slot from the columns of the right type, and ranks candidate columns
//! by how many case-folded header tokens the claim's sentence shares. At most
//! [`MAX_PROPOSALS`] per claim, each with a closed reason code and the tokens
//! it matched. It never confirms anything.

use super::binding::{BindingDraft, OlsTarget, ProposalReason, ProposalSource, RoleBinding};
use super::catalogue::{contains_word, L1MethodId, LevelRule, MethodCatalogue, SlotKind, VariantChoice, METHOD_WORDS};
use super::claim::L1Claim;
use super::dataset::{ColumnAlias, ColumnSummary, LevelBucket, SchemaSummary};
use crate::stats_l0::result::Family;

pub const MAX_PROPOSALS: usize = 3;

/// A draft and the source that explains it. The run state gives it an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProposal {
    pub binding: BindingDraft,
    pub source: ProposalSource,
}

/// Lower-case alphanumeric runs of two or more characters, numbers excluded.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ch.is_alphanumeric() {
            cur.extend(ch.to_lowercase());
        } else if !cur.is_empty() {
            if cur.chars().count() >= 2 && !cur.chars().all(|c| c.is_ascii_digit()) {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The header tokens a column shares with the sentence.
fn overlap(column: &ColumnSummary, sentence_tokens: &[String]) -> Vec<String> {
    tokens(&column.header).into_iter().filter(|t| sentence_tokens.binary_search(t).is_ok()).collect()
}

/// Which methods a family's claim can be: the sentence's words decide between
/// the alternatives, and the first listed is the default.
fn methods_for(family: Family, sentence: &str) -> Vec<L1MethodId> {
    let says = |m: L1MethodId| METHOD_WORDS.iter().any(|(w, id)| *id == m && contains_word(sentence, w));
    match family {
        Family::T => {
            if says(L1MethodId::PairedT) && !says(L1MethodId::IndependentT) {
                vec![L1MethodId::PairedT, L1MethodId::IndependentT]
            } else {
                vec![L1MethodId::IndependentT, L1MethodId::PairedT]
            }
        }
        Family::F => {
            if says(L1MethodId::Ols) && !says(L1MethodId::OneWayAnova) {
                vec![L1MethodId::Ols, L1MethodId::OneWayAnova]
            } else {
                vec![L1MethodId::OneWayAnova, L1MethodId::Ols]
            }
        }
        Family::ChiSquare => vec![L1MethodId::ChiSquareIndependence],
        Family::R => {
            if says(L1MethodId::Spearman) && !says(L1MethodId::Pearson) {
                vec![L1MethodId::Spearman, L1MethodId::Pearson]
            } else {
                vec![L1MethodId::Pearson, L1MethodId::Spearman]
            }
        }
        Family::Z => Vec::new(),
    }
}

/// Columns eligible for a slot, best match first, then file order.
fn ranked(schema: &SchemaSummary, kind: SlotKind, sentence_tokens: &[String]) -> Vec<(ColumnAlias, Vec<String>)> {
    let eligible = |c: &ColumnSummary| match kind {
        SlotKind::Numeric | SlotKind::NumericList { .. } => c.is_numeric(),
        SlotKind::Group { levels } => {
            c.is_group()
                && match levels {
                    LevelRule::Exactly(2) => c.level_bucket == Some(LevelBucket::Two),
                    LevelRule::Exactly(k) => c.distinct == Some(k),
                    LevelRule::AtLeast(_) => c.level_bucket.is_some(),
                }
        }
    };
    let mut out: Vec<(ColumnAlias, Vec<String>)> =
        schema.columns.iter().filter(|c| eligible(c)).map(|c| (c.alias, overlap(c, sentence_tokens))).collect();
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    out
}

/// Up to [`MAX_PROPOSALS`] drafts for `claim` against `schema`, best first.
pub fn propose_local(schema: &SchemaSummary, claim: &L1Claim) -> Vec<LocalProposal> {
    let sentence_tokens = tokens(&claim.sentence);
    let mut out: Vec<LocalProposal> = Vec::new();

    for method in methods_for(claim.result.family, &claim.sentence) {
        let entry = MethodCatalogue::entry(method);
        // Per slot, the ranked candidates.
        let candidates: Vec<Vec<(ColumnAlias, Vec<String>)>> =
            entry.slots.iter().map(|s| ranked(schema, s.kind, &sentence_tokens)).collect();
        if candidates.iter().any(Vec::is_empty) {
            continue;
        }
        // Alternatives: the best for every slot, then the runner-up in each
        // slot in turn, columns never repeated within a draft.
        let mut variants: Vec<Vec<usize>> = vec![vec![0; entry.slots.len()]];
        for (i, c) in candidates.iter().enumerate() {
            if c.len() > 1 {
                let mut v = vec![0; entry.slots.len()];
                v[i] = 1;
                variants.push(v);
            }
        }
        for picks in variants {
            if out.len() >= MAX_PROPOSALS {
                break;
            }
            let mut draft = BindingDraft::new(method, VariantChoice::from_wording(entry.variant_kind, &claim.sentence, None));
            let mut used: Vec<ColumnAlias> = Vec::new();
            let mut matched: Vec<String> = Vec::new();
            let mut complete = true;
            for (slot, (cands, &pick)) in entry.slots.iter().zip(candidates.iter().zip(picks.iter())) {
                // Skip a candidate already used by an earlier slot.
                let chosen = cands.iter().enumerate().filter(|(_, (a, _))| !used.contains(a)).nth(pick).or_else(|| {
                    cands.iter().enumerate().find(|(_, (a, _))| !used.contains(a))
                });
                let Some((_, (alias, toks))) = chosen else {
                    complete = false;
                    break;
                };
                used.push(*alias);
                matched.extend(toks.iter().cloned());
                draft.roles.push(RoleBinding { role: slot.role, columns: vec![*alias] });
            }
            if !complete {
                continue;
            }
            if entry.needs_target {
                draft.target = Some(OlsTarget::OverallF);
            }
            matched.sort_unstable();
            matched.dedup();
            let reason = if matched.is_empty() { ProposalReason::FirstEligibleColumns } else { ProposalReason::HeaderMatchesSentence };
            let candidate = LocalProposal { binding: draft, source: ProposalSource::Local { reason, matched_tokens: matched } };
            if !out.iter().any(|p| p.binding == candidate.binding) {
                out.push(candidate);
            }
        }
        if out.len() >= MAX_PROPOSALS {
            break;
        }
    }
    out.truncate(MAX_PROPOSALS);
    tracing::debug!(proposals = out.len(), "stats_l1: local proposals");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::SectionKind;
    use crate::stats_l0::number::Relation;
    use crate::stats_l1::claim::{ClaimId, ClaimLocation, ClaimOrigin, UserClaim, UserP};
    use crate::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};

    fn schema(headers: &[&str], rows: &[&[&str]]) -> SchemaSummary {
        let raw = RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        );
        build_dataset(raw, DataFingerprint::of_bytes(b"b", "p", TableRef::FirstTable)).unwrap().schema
    }

    fn claim(family: Family, sentence: &str) -> L1Claim {
        L1Claim::user_entered(
            ClaimId(0),
            &UserClaim {
                family,
                statistic: "2.10",
                df: vec!["28"],
                n: None,
                p: UserP::Reported { raw: ".04", relation: Relation::Eq },
                location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
                sentence: Some(sentence),
            },
            ClaimOrigin::UserAdded,
        )
        .unwrap()
    }

    #[test]
    fn header_overlap_with_the_sentence_ranks_the_outcome_and_the_group() {
        let s = schema(
            &["id", "reaction_time", "accuracy", "condition", "site"],
            &[&["1", "410", "0.9", "control", "a"], &["2", "420", "0.8", "treatment", "b"], &["3", "430", "0.7", "control", "c"]],
        );
        let c = claim(Family::T, "Reaction time was longer in the treatment condition, t(28) = 2.10, p = .04.");
        let p = propose_local(&s, &c);
        assert!(!p.is_empty() && p.len() <= MAX_PROPOSALS, "{p:?}");
        let first = &p[0];
        assert_eq!(first.binding.method_id, "independent_t");
        assert_eq!(first.binding.roles[0].columns, vec![ColumnAlias(2)], "reaction_time shares two tokens");
        assert_eq!(first.binding.roles[1].columns, vec![ColumnAlias(4)], "condition has two levels and a matching token");
        match &first.source {
            ProposalSource::Local { reason, matched_tokens } => {
                assert_eq!(*reason, ProposalReason::HeaderMatchesSentence);
                assert_eq!(matched_tokens, &["condition", "reaction", "time"]);
            }
            other => panic!("{other:?}"),
        }
        // Every proposal is a different binding.
        for (i, a) in p.iter().enumerate() {
            for b in &p[i + 1..] {
                assert_ne!(a.binding, b.binding);
            }
        }
    }

    #[test]
    fn with_no_matching_header_the_reason_says_so_and_the_first_eligible_columns_are_used() {
        let s = schema(&["x", "g"], &[&["1", "a"], &["2", "b"]]);
        let c = claim(Family::T, "Scores rose, t(28) = 2.10, p = .04.");
        let p = propose_local(&s, &c);
        assert_eq!(p.len(), 1);
        assert!(matches!(p[0].source, ProposalSource::Local { reason: ProposalReason::FirstEligibleColumns, .. }));
        assert_eq!(p[0].binding.roles[0].columns, vec![ColumnAlias(1)]);
    }

    #[test]
    fn the_sentence_words_choose_between_paired_and_independent_and_f_between_anova_and_ols() {
        let s = schema(&["pre", "post", "group"], &[&["1", "2", "a"], &["2", "3", "b"], &["3", "4", "a"]]);
        let paired = propose_local(&s, &claim(Family::T, "A paired t-test, t(28) = 2.10, p = .04."));
        assert_eq!(paired[0].binding.method_id, "paired_t");
        let indep = propose_local(&s, &claim(Family::T, "t(28) = 2.10, p = .04."));
        assert_eq!(indep[0].binding.method_id, "independent_t");
        let ols = propose_local(&s, &claim(Family::F, "The regression was significant, F(2, 27) = 4.1, p = .03."));
        assert_eq!(ols[0].binding.method_id, "ols");
        assert_eq!(ols[0].binding.target, Some(OlsTarget::OverallF));
        let anova = propose_local(&s, &claim(Family::F, "F(2, 27) = 4.1, p = .03."));
        assert_eq!(anova[0].binding.method_id, "one_way_anova");
    }

    #[test]
    fn a_z_claim_and_a_schema_with_no_eligible_columns_get_no_proposals() {
        let s = schema(&["note"], &[&["a"], &["b"]]);
        assert!(propose_local(&s, &claim(Family::T, "t(28) = 2.10, p = .04.")).is_empty());
        let s = schema(&["x", "g"], &[&["1", "a"], &["2", "b"]]);
        assert!(propose_local(&s, &claim(Family::Z, "z = 2.10, p = .04.")).is_empty());
    }

    #[test]
    fn a_stated_variant_in_the_sentence_reaches_the_draft_and_nothing_else_does() {
        let s = schema(&["x", "g"], &[&["1", "a"], &["2", "b"]]);
        let p = propose_local(&s, &claim(Family::T, "Welch's t(28) = 2.10, p = .04."));
        assert!(p[0].binding.variant.variant().is_stated());
        let p = propose_local(&s, &claim(Family::T, "t(28) = 2.10, p = .04."));
        assert!(!p[0].binding.variant.variant().is_stated());
    }

    #[test]
    fn tokens_are_case_folded_two_plus_characters_and_never_numbers() {
        assert_eq!(tokens("Reaction_Time 2024 t p=.04 x"), vec!["reaction".to_string(), "time".to_string()]);
    }
}
