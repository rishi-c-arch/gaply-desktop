//! **The method catalogue: every Level 1 method id, its role slots and its
//! variants, fixed at compile time.**
//!
//! A binding is validated against this list at confirm time. Step 3's
//! registry (methods with an implementation and fixtures) is a subset of it,
//! checked again at compute time: a method may be in the catalogue, so that a
//! user can bind it, before it can be computed.
//!
//! `MethodCatalogue` because `MethodRegistry`, `Method` and `MethodId` are
//! taken; `L1MethodId` beside `Level 0`'s string ids.
//!
//! # Role slots
//!
//! "Numeric" means a numeric or integer column. "Group" means a categorical
//! column, or an integer column with at most
//! [`super::dataset::GROUP_LEVEL_LIMIT`] distinct values. A group slot carries
//! the rule on how many levels enter the comparison after level selection.
//!
//! # Variants
//!
//! Where software differs, the binding records which variant the paper
//! states: Student or Welch for an independent t; continuity correction or
//! not for a 2×2 chi-square; the t approximation or an exact p for Spearman.
//! A variant counts as STATED only when the closed phrase list below finds it,
//! not negated by the shared rule ([`crate::stats_l0::negation`]), in the
//! claim's sentence or in a Methods passage the user selected, or when the
//! user ticks "the paper states this". Anything else, an AI suggestion
//! included, stays not stated. [`VariantChoice`]'s private fields and its three
//! constructors are what enforce that.

use crate::stats_l0::negation::{negation_of, Negation};

/// Every Level 1 method. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum L1MethodId {
    /// Student's or Welch's t on an outcome split by a two-level group.
    IndependentT,
    /// Two numeric columns, one row per subject.
    PairedT,
    OneWayAnova,
    Pearson,
    Spearman,
    ChiSquareIndependence,
    /// Ordinary least squares with numeric predictors: the overall F, or one
    /// named predictor's coefficient.
    Ols,
}

impl L1MethodId {
    pub const ALL: [L1MethodId; 7] = [
        L1MethodId::IndependentT,
        L1MethodId::PairedT,
        L1MethodId::OneWayAnova,
        L1MethodId::Pearson,
        L1MethodId::Spearman,
        L1MethodId::ChiSquareIndependence,
        L1MethodId::Ols,
    ];

    /// The string id a binding names the method by.
    pub fn id(self) -> &'static str {
        match self {
            L1MethodId::IndependentT => "independent_t",
            L1MethodId::PairedT => "paired_t",
            L1MethodId::OneWayAnova => "one_way_anova",
            L1MethodId::Pearson => "pearson",
            L1MethodId::Spearman => "spearman",
            L1MethodId::ChiSquareIndependence => "chi_square_independence",
            L1MethodId::Ols => "ols",
        }
    }

    /// The method's version, part of what a token is bound to. Every method
    /// is at version 1 until one changes.
    pub fn version(self) -> &'static str {
        "1"
    }

    pub fn from_id(id: &str) -> Option<L1MethodId> {
        L1MethodId::ALL.iter().copied().find(|m| m.id() == id)
    }

    pub fn is_paired(self) -> bool {
        matches!(self, L1MethodId::PairedT)
    }
}

/// The roles a method's columns play. Shared across methods so that a
/// proposal can name a role its method has no slot for, and be rejected for
/// exactly that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RoleName {
    Outcome,
    Group,
    /// Paired t: the first measurement.
    First,
    /// Paired t: the second measurement.
    Second,
    /// A correlation's two columns.
    X,
    Y,
    /// Chi-square: the two categorical variables.
    RowVariable,
    ColumnVariable,
    /// OLS: one or more numeric predictors.
    Predictors,
}

/// How many levels a group slot admits after level selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelRule {
    Exactly(usize),
    AtLeast(usize),
}

impl LevelRule {
    pub fn admits(self, n: usize) -> bool {
        match self {
            LevelRule::Exactly(k) => n == k,
            LevelRule::AtLeast(k) => n >= k,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// Exactly one numeric or integer column.
    Numeric,
    /// Exactly one group column, with a rule on its selected levels.
    Group { levels: LevelRule },
    /// At least `min` numeric columns.
    NumericList { min: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleSlot {
    pub role: RoleName,
    pub kind: SlotKind,
}

/// Which variant family a method has, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantKind {
    None,
    T,
    Continuity,
    SpearmanP,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodEntry {
    pub id: L1MethodId,
    pub slots: &'static [RoleSlot],
    pub variant_kind: VariantKind,
    /// OLS needs a target: the overall F or one predictor's coefficient.
    pub needs_target: bool,
}

impl MethodEntry {
    pub fn slot(&self, role: RoleName) -> Option<&RoleSlot> {
        self.slots.iter().find(|s| s.role == role)
    }
}

pub struct MethodCatalogue;

const fn numeric(role: RoleName) -> RoleSlot {
    RoleSlot { role, kind: SlotKind::Numeric }
}
const fn group(role: RoleName, levels: LevelRule) -> RoleSlot {
    RoleSlot { role, kind: SlotKind::Group { levels } }
}

impl MethodCatalogue {
    pub const ENTRIES: &'static [MethodEntry] = &[
        MethodEntry {
            id: L1MethodId::IndependentT,
            slots: &[numeric(RoleName::Outcome), group(RoleName::Group, LevelRule::Exactly(2))],
            variant_kind: VariantKind::T,
            needs_target: false,
        },
        MethodEntry {
            id: L1MethodId::PairedT,
            slots: &[numeric(RoleName::First), numeric(RoleName::Second)],
            variant_kind: VariantKind::None,
            needs_target: false,
        },
        MethodEntry {
            id: L1MethodId::OneWayAnova,
            slots: &[numeric(RoleName::Outcome), group(RoleName::Group, LevelRule::AtLeast(2))],
            variant_kind: VariantKind::None,
            needs_target: false,
        },
        MethodEntry {
            id: L1MethodId::Pearson,
            slots: &[numeric(RoleName::X), numeric(RoleName::Y)],
            variant_kind: VariantKind::None,
            needs_target: false,
        },
        MethodEntry {
            id: L1MethodId::Spearman,
            slots: &[numeric(RoleName::X), numeric(RoleName::Y)],
            variant_kind: VariantKind::SpearmanP,
            needs_target: false,
        },
        MethodEntry {
            id: L1MethodId::ChiSquareIndependence,
            slots: &[
                group(RoleName::RowVariable, LevelRule::AtLeast(2)),
                group(RoleName::ColumnVariable, LevelRule::AtLeast(2)),
            ],
            variant_kind: VariantKind::Continuity,
            needs_target: false,
        },
        MethodEntry {
            id: L1MethodId::Ols,
            slots: &[
                numeric(RoleName::Outcome),
                RoleSlot { role: RoleName::Predictors, kind: SlotKind::NumericList { min: 1 } },
            ],
            variant_kind: VariantKind::None,
            needs_target: true,
        },
    ];

    pub fn entry(id: L1MethodId) -> &'static MethodEntry {
        MethodCatalogue::ENTRIES.iter().find(|e| e.id == id).expect("every L1MethodId has a catalogue entry")
    }

    /// The entry for a string id, or `None` for a method not in the catalogue.
    pub fn by_id(id: &str) -> Option<&'static MethodEntry> {
        L1MethodId::from_id(id).map(MethodCatalogue::entry)
    }
}

/* ------------------------------------------------------------------------ *
 *  Variants
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TVariant {
    Student,
    Welch,
    NotStated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuityVariant {
    Yes,
    No,
    NotStated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpearmanPVariant {
    TApproximation,
    Exact,
    NotStated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    None,
    T(TVariant),
    Continuity(ContinuityVariant),
    SpearmanP(SpearmanPVariant),
}

impl Variant {
    pub fn kind(self) -> VariantKind {
        match self {
            Variant::None => VariantKind::None,
            Variant::T(_) => VariantKind::T,
            Variant::Continuity(_) => VariantKind::Continuity,
            Variant::SpearmanP(_) => VariantKind::SpearmanP,
        }
    }

    /// The not-stated member of a kind.
    pub fn not_stated(kind: VariantKind) -> Variant {
        match kind {
            VariantKind::None => Variant::None,
            VariantKind::T => Variant::T(TVariant::NotStated),
            VariantKind::Continuity => Variant::Continuity(ContinuityVariant::NotStated),
            VariantKind::SpearmanP => Variant::SpearmanP(SpearmanPVariant::NotStated),
        }
    }

    pub fn is_stated(self) -> bool {
        !matches!(
            self,
            Variant::None
                | Variant::T(TVariant::NotStated)
                | Variant::Continuity(ContinuityVariant::NotStated)
                | Variant::SpearmanP(SpearmanPVariant::NotStated)
        )
    }

    /// For the normalised binding digest.
    pub fn canonical(self) -> String {
        format!("{self:?}")
    }
}

/// Where the paper's wording was found, for a word-list variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordingSource {
    ClaimSentence,
    MethodsPassage,
}

/// Who set the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantSetBy {
    /// Nobody: the variant is the kind's not-stated member.
    NotStated,
    /// The closed word list found `word` in the paper's text.
    WordList { source: WordingSource, word: &'static str },
    /// The user ticked "the paper states this".
    UserTicked,
}

/// A variant and who set it. Private fields: the only ways to hold a STATED
/// variant are the word list and the user's tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantChoice {
    variant: Variant,
    set_by: VariantSetBy,
}

impl VariantChoice {
    pub fn not_stated(kind: VariantKind) -> VariantChoice {
        VariantChoice { variant: Variant::not_stated(kind), set_by: VariantSetBy::NotStated }
    }

    /// The user ticked "the paper states this" for `variant`. A not-stated
    /// member ticked is simply not stated.
    pub fn user_ticked(variant: Variant) -> VariantChoice {
        if variant.is_stated() {
            VariantChoice { variant, set_by: VariantSetBy::UserTicked }
        } else {
            VariantChoice::not_stated(variant.kind())
        }
    }

    /// Read the variant from the paper's own words: the claim's sentence first,
    /// then the user-selected Methods passage. Not stated when neither names
    /// one, or when the two name different ones.
    pub fn from_wording(kind: VariantKind, claim_sentence: &str, methods_passage: Option<&str>) -> VariantChoice {
        let in_sentence = variant_in_text(kind, claim_sentence);
        let in_methods = methods_passage.and_then(|m| variant_in_text(kind, m));
        match (in_sentence, in_methods) {
            (Some((v, w)), Some((m, _))) if v != m => {
                // Two different variants named: the paper does not clearly
                // state one, and the sentence is not preferred over the
                // Methods or the other way round.
                let _ = w;
                VariantChoice::not_stated(kind)
            }
            (Some((v, word)), _) => {
                VariantChoice { variant: v, set_by: VariantSetBy::WordList { source: WordingSource::ClaimSentence, word } }
            }
            (None, Some((v, word))) => {
                VariantChoice { variant: v, set_by: VariantSetBy::WordList { source: WordingSource::MethodsPassage, word } }
            }
            (None, None) => VariantChoice::not_stated(kind),
        }
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }
    pub fn set_by(&self) -> VariantSetBy {
        self.set_by
    }
}

/// One phrase that names a variant, as words (see [`phrase_words`]): each
/// element lists the words allowed at that place, `""` makes it optional,
/// and a trailing `*` matches any word starting with what precedes it.
/// `label` is what the outcome records as the word the paper used.
struct VariantPhrase {
    label: &'static str,
    words: &'static [&'static [&'static str]],
    variant: Variant,
    /// A phrase that names one variant by negating the other ("equal
    /// variances were not assumed" states Welch). It wins over any phrase it
    /// overlaps.
    names_other_way: bool,
}

const fn phrase(label: &'static str, words: &'static [&'static [&'static str]], variant: Variant) -> VariantPhrase {
    VariantPhrase { label, words, variant, names_other_way: false }
}

const fn other_way(label: &'static str, words: &'static [&'static [&'static str]], variant: Variant) -> VariantPhrase {
    VariantPhrase { label, words, variant, names_other_way: true }
}

const WELCH: Variant = Variant::T(TVariant::Welch);
const STUDENT: Variant = Variant::T(TVariant::Student);
const UNCORRECTED: Variant = Variant::Continuity(ContinuityVariant::No);
const YATES: Variant = Variant::Continuity(ContinuityVariant::Yes);
const YATES_WORDS: &[&str] = &["yates", "yates'", "yates's"];
const VARIANCES: &[&str] = &["variances", "variance"];

/// The closed list (evaluation fixes, part A). A phrase counts only when the
/// shared negation rule finds it not negated in its clause; a clearly
/// negated phrase is dropped, and an unclear one leaves the variant not
/// stated. "Student" counts only as "Student's t" or "Student t"; "pooled"
/// only for a pooled-variance t-test.
const VARIANT_PHRASES: &[VariantPhrase] = &[
    // Independent t.
    other_way("equal variances not assumed", &[&["equal"], VARIANCES, &["were", "are", "was", "is", ""], &["not"], &["assumed"]], WELCH),
    other_way("not assuming equal variances", &[&["not", "without"], &["assuming"], &["equal"], VARIANCES], WELCH),
    phrase("welch", &[&["welch", "welch's"]], WELCH),
    phrase("unequal variances", &[&["unequal"], &["variances"]], WELCH),
    phrase("satterthwaite", &[&["satterthwaite", "satterthwaite's"]], WELCH),
    phrase("student's t", &[&["student", "student's", "students'"], &["t"]], STUDENT),
    phrase("equal variances", &[&["equal"], &["variances"]], STUDENT),
    phrase("pooled-variance t", &[&["pooled"], &["variance", "variances", ""], &["t"]], STUDENT),
    // 2x2 chi-square.
    other_way("without yates' correction", &[&["without"], &["a", "the", ""], YATES_WORDS, &["continuity", ""], &["correction", "corrections"]], UNCORRECTED),
    other_way("without continuity correction", &[&["without"], &["a", "the", ""], &["continuity"], &["correction", "corrections"]], UNCORRECTED),
    other_way("uncorrected chi-square", &[&["uncorrected"], &["chi*", "\u{3c7}*"]], UNCORRECTED),
    phrase("yates", &[YATES_WORDS], YATES),
    phrase("continuity correction", &[&["continuity"], &["correction", "corrected"]], YATES),
    // Spearman's p.
    phrase("t approximation", &[&["t"], &["approximation"]], Variant::SpearmanP(SpearmanPVariant::TApproximation)),
    phrase("exact", &[&["exact"]], Variant::SpearmanP(SpearmanPVariant::Exact)),
];

/// Words that name a METHOD rather than a variant, for the local proposer:
/// `paired` decides paired against independent t, and so on.
pub const METHOD_WORDS: &[(&str, L1MethodId)] = &[
    ("paired", L1MethodId::PairedT),
    ("repeated measures", L1MethodId::PairedT),
    ("independent", L1MethodId::IndependentT),
    ("two-sample", L1MethodId::IndependentT),
    ("unpaired", L1MethodId::IndependentT),
    ("spearman", L1MethodId::Spearman),
    ("rho", L1MethodId::Spearman),
    ("rank", L1MethodId::Spearman),
    ("pearson", L1MethodId::Pearson),
    ("anova", L1MethodId::OneWayAnova),
    ("regression", L1MethodId::Ols),
    ("ols", L1MethodId::Ols),
    ("least squares", L1MethodId::Ols),
    ("chi", L1MethodId::ChiSquareIndependence),
];

/// Case-folded, word-boundary containment: `welch` matches `Welch's` and not
/// `unwelcheable`. Boundaries are non-alphanumeric characters.
pub fn contains_word(text: &str, word: &str) -> bool {
    let hay = text.to_lowercase();
    let needle = word.to_lowercase();
    let mut from = 0;
    while let Some(i) = hay[from..].find(&needle) {
        let start = from + i;
        let end = start + needle.len();
        let before_ok = start == 0 || !hay[..start].chars().next_back().is_some_and(char::is_alphanumeric);
        let after_ok = end == hay.len() || !hay[end..].chars().next().is_some_and(char::is_alphanumeric);
        if before_ok && after_ok {
            return true;
        }
        from = start + needle.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Words for phrase matching: runs of letters, digits and apostrophes
/// (`\u{2019}` read as `'`), lower case, with their byte ranges. A hyphen
/// separates words, so "t-test" is `t`, `test`.
fn phrase_words(text: &str) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = Vec::new();
    let mut cur: Option<(String, usize)> = None;
    for (i, c) in text.char_indices() {
        if c.is_alphanumeric() || c == '\'' || c == '\u{2019}' {
            let c = if c == '\u{2019}' { '\'' } else { c };
            match &mut cur {
                Some((w, _)) => w.extend(c.to_lowercase()),
                None => cur = Some((c.to_lowercase().collect(), i)),
            }
        } else if let Some((w, at)) = cur.take() {
            out.push((w, at, i));
        }
    }
    if let Some((w, at)) = cur {
        out.push((w, at, text.len()));
    }
    out
}

fn word_fits(word: &str, allowed: &str) -> bool {
    match allowed.strip_suffix('*') {
        Some(prefix) => word.starts_with(prefix),
        None => word == allowed,
    }
}

/// Every occurrence of `p` in `words`, as byte ranges of `text`.
fn phrase_spans(p: &VariantPhrase, words: &[(String, usize, usize)]) -> Vec<(usize, usize)> {
    fn from(elems: &[&[&str]], words: &[(String, usize, usize)], at: usize) -> Option<usize> {
        let Some((first, rest)) = elems.split_first() else { return Some(at) };
        if let Some(w) = words.get(at) {
            if first.iter().any(|a| !a.is_empty() && word_fits(&w.0, a)) {
                if let Some(end) = from(rest, words, at + 1) {
                    return Some(end);
                }
            }
        }
        if first.contains(&"") {
            return from(rest, words, at);
        }
        None
    }
    (0..words.len())
        .filter_map(|i| from(p.words, words, i).filter(|&end| end > i).map(|end| (words[i].1, words[end - 1].2)))
        .collect()
}

/// The variant `text` states, by [`VARIANT_PHRASES`] and the shared negation
/// rule: phrases naming the other way first, then the rest unless they
/// overlap one. A clearly negated phrase is dropped; an unclear one, or two
/// phrases naming different variants, leaves the variant not stated.
fn variant_in_text(kind: VariantKind, text: &str) -> Option<(Variant, &'static str)> {
    let words = phrase_words(text);
    let mut found: Vec<(&VariantPhrase, usize, usize)> = Vec::new();
    for p in VARIANT_PHRASES.iter().filter(|p| p.variant.kind() == kind) {
        for (s, e) in phrase_spans(p, &words) {
            found.push((p, s, e));
        }
    }
    let specials: Vec<(usize, usize)> = found.iter().filter(|(p, _, _)| p.names_other_way).map(|&(_, s, e)| (s, e)).collect();
    found.retain(|(p, s, e)| p.names_other_way || !specials.iter().any(|&(a, b)| *s < b && a < *e));
    let mut stated: Option<(Variant, &'static str)> = None;
    for (p, s, e) in found {
        match negation_of(text, s, e) {
            Negation::Clearly => continue,
            Negation::Unclear => return None,
            Negation::Not => match stated {
                Some((v, _)) if v != p.variant => return None,
                Some(_) => {}
                None => stated = Some((p.variant, p.label)),
            },
        }
    }
    stated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_method_has_an_entry_and_ids_round_trip() {
        for m in L1MethodId::ALL {
            let e = MethodCatalogue::entry(m);
            assert_eq!(e.id, m);
            assert_eq!(L1MethodId::from_id(m.id()), Some(m));
            assert!(!e.slots.is_empty());
        }
        assert_eq!(MethodCatalogue::ENTRIES.len(), 7);
        assert!(MethodCatalogue::by_id("mann_whitney").is_none());
        assert!(MethodCatalogue::by_id("").is_none());
    }

    #[test]
    fn the_slots_are_the_ones_the_step_names() {
        let t = MethodCatalogue::entry(L1MethodId::IndependentT);
        assert_eq!(t.slot(RoleName::Group).unwrap().kind, SlotKind::Group { levels: LevelRule::Exactly(2) });
        let a = MethodCatalogue::entry(L1MethodId::OneWayAnova);
        assert_eq!(a.slot(RoleName::Group).unwrap().kind, SlotKind::Group { levels: LevelRule::AtLeast(2) });
        let o = MethodCatalogue::entry(L1MethodId::Ols);
        assert_eq!(o.slot(RoleName::Predictors).unwrap().kind, SlotKind::NumericList { min: 1 });
        assert!(o.needs_target);
        assert_eq!(MethodCatalogue::entry(L1MethodId::PairedT).slots.iter().map(|s| s.role).collect::<Vec<_>>(), vec![RoleName::First, RoleName::Second]);
    }

    #[test]
    fn a_variant_is_stated_only_by_the_word_list_or_the_tick() {
        let c = VariantChoice::from_wording(VariantKind::T, "Welch's t-test, t(27.4) = 2.1, p = .04.", None);
        assert_eq!(c.variant(), Variant::T(TVariant::Welch));
        assert_eq!(c.set_by(), VariantSetBy::WordList { source: WordingSource::ClaimSentence, word: "welch" });

        let c = VariantChoice::from_wording(VariantKind::T, "t(58) = 2.1, p = .04.", Some("We assumed equal variances."));
        assert_eq!(c.variant(), Variant::T(TVariant::Student));
        assert_eq!(c.set_by(), VariantSetBy::WordList { source: WordingSource::MethodsPassage, word: "equal variances" });

        let c = VariantChoice::from_wording(VariantKind::T, "t(58) = 2.1, p = .04.", None);
        assert_eq!(c.variant(), Variant::T(TVariant::NotStated));
        assert_eq!(c.set_by(), VariantSetBy::NotStated);

        // Two different variants named: not stated.
        let c = VariantChoice::from_wording(VariantKind::T, "Welch's test gave t = 2.", Some("Student's t-tests were used."));
        assert!(!c.variant().is_stated());

        let c = VariantChoice::user_ticked(Variant::T(TVariant::Welch));
        assert_eq!(c.set_by(), VariantSetBy::UserTicked);
        // Ticking "not stated" is not a statement.
        assert_eq!(VariantChoice::user_ticked(Variant::T(TVariant::NotStated)).set_by(), VariantSetBy::NotStated);
    }

    #[test]
    fn negated_continuity_wording_is_read_before_the_phrase_it_negates() {
        let c = VariantChoice::from_wording(VariantKind::Continuity, "χ²(1) = 4.1, p = .04, without continuity correction.", None);
        assert_eq!(c.variant(), Variant::Continuity(ContinuityVariant::No));
        let c = VariantChoice::from_wording(VariantKind::Continuity, "Yates' corrected χ²(1) = 3.2, p = .07.", None);
        assert_eq!(c.variant(), Variant::Continuity(ContinuityVariant::Yes));
        let c = VariantChoice::from_wording(VariantKind::SpearmanP, "rs = .41, p = .02 (exact).", None);
        assert_eq!(c.variant(), Variant::SpearmanP(SpearmanPVariant::Exact));
        // A word of another kind does not set this kind.
        let c = VariantChoice::from_wording(VariantKind::Continuity, "Welch's t", None);
        assert!(!c.variant().is_stated());
    }

    #[test]
    fn word_matching_respects_boundaries_and_case() {
        assert!(contains_word("Welch's t-test", "welch"));
        assert!(contains_word("WELCH", "welch"));
        assert!(!contains_word("unwelcheable", "welch"));
        assert!(!contains_word("students", "student"));
        assert!(contains_word("the students' t", "students"));
        assert!(contains_word("a t-approximation.", "t-approximation"));
        assert!(!contains_word("", "welch"));
    }
}
