//! **Is a matched phrase negated in its clause?** One rule, shared by every
//! reader that takes a paper's words as a statement (Phase 2 evaluation
//! fixes, part A): Level 0's one-tailed and multiple-testing words, and Level
//! 1's variant words. Each caller maps the three answers in its own safe
//! direction.
//!
//! **The clause.** It runs back from the phrase and on from it to the
//! nearest of:
//! - `.` `;` `:` `?` `!` (a `.` between two digits is a decimal point, not a
//!   boundary);
//! - a comma followed by a conjunction or relative word (`and`, `but`, `or`,
//!   `nor`, `while`, `whereas`, `although`, `though`, `yet`, `so`, `because`,
//!   `since`, `which`, `who`, `whom`, `whose`, `where`, `when`) or by a new
//!   subject (`we`, `i`, `they`, `he`, `she`, `it`, `this`, `these`, `those`,
//!   `there`, `our`, `their`, `all`, `each`).
//!
//! **Words.** Runs of letters, digits and apostrophes (`'`, `’`); a hyphen or
//! any other mark separates words. A negator is `not`, `no`, `never`,
//! `without`, `neither`, `nor`, `cannot`, any word ending in `n't` or
//! `n’t`, `than` after `rather`, and `of` after `instead`.
//!
//! **The answers.**
//! - **Clearly negated**: a negator among the three words before the phrase
//!   (but "with and without" and "with or without" are unclear); or, anywhere
//!   after the phrase in its clause, a negator that negates one of the listed
//!   verbs (use, apply, perform, conduct, make, assume, adjust, correct,
//!   employ, in any tense), the verb within the two words after the negator:
//!   "one-tailed tests were not used", "a Bonferroni correction, as planned,
//!   was not applied" (review follow-up, item 1).
//! - **Unclear**: any other negator earlier in the clause; after the phrase,
//!   any other negator among its first four words with no comma between
//!   ("Bonferroni correction did not change the results"), or one straight
//!   after an auxiliary further on (`did not`, `weren't`). Unclear is the safe
//!   answer: a correction stays adjusted, a test is not stated one-tailed, a
//!   variant is not stated.
//! - **Not negated**: anything else. A bare negator after the phrase and
//!   past a comma negates something else: "tests were one-tailed, not
//!   two-tailed" states one-tailed. After the phrase, `rather than` and
//!   `instead of` never count: they negate what follows them.

/// The three answers. Callers map them; none is a verdict by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Negation {
    Clearly,
    Unclear,
    Not,
}

/// Is `text[start..end]`, a matched phrase, negated in its clause? `start`
/// and `end` are byte offsets on character boundaries.
pub fn negation_of(text: &str, start: usize, end: usize) -> Negation {
    let (from, to) = clause_around(text, start, end);
    let before = words(&text[from..start]);
    let after_text = &text[end..to];
    let after = words(after_text);

    // Before the phrase: the three nearest words, then the rest of the
    // clause. "with and without" and "with or without" name both cases.
    let nearest = before.len().saturating_sub(3);
    let mut unclear = false;
    for i in nearest..before.len() {
        if is_negator(&before, i) {
            if with_and_without(&before, i) {
                unclear = true;
            } else {
                return Negation::Clearly;
            }
        }
    }
    unclear |= (0..nearest).any(|i| is_negator(&before, i));

    // After the phrase: a negated listed verb anywhere in the clause is
    // clear; another negator in the predicate, or after an auxiliary, is
    // unclear; a bare negator past a comma negates something else.
    let first_comma = after_text.find(',').unwrap_or(after_text.len());
    for (i, w) in after.iter().enumerate() {
        // "rather than" and "instead of" negate what FOLLOWS them.
        if !is_negator(&after, i) || matches!(w.text.as_str(), "than" | "of") {
            continue;
        }
        if negates_listed_verb(&after, i) {
            return Negation::Clearly;
        }
        let aux_before = i > 0 && AUXILIARIES.contains(&after[i - 1].text.as_str());
        if (i < 4 && w.at < first_comma) || aux_before || is_aux_contraction(&w.text) {
            unclear = true;
        }
    }
    if unclear {
        Negation::Unclear
    } else {
        Negation::Not
    }
}

/// The verbs whose negation, after a phrase, says the phrase did not apply
/// (review follow-up, item 1), in their usual forms.
const LISTED_VERBS: &[&str] = &[
    "use",
    "uses",
    "used",
    "using",
    "apply",
    "applies",
    "applied",
    "applying",
    "perform",
    "performs",
    "performed",
    "performing",
    "conduct",
    "conducts",
    "conducted",
    "conducting",
    "make",
    "makes",
    "made",
    "making",
    "assume",
    "assumes",
    "assumed",
    "assuming",
    "adjust",
    "adjusts",
    "adjusted",
    "adjusting",
    "correct",
    "corrects",
    "corrected",
    "correcting",
    "employ",
    "employs",
    "employed",
    "employing",
];

/// The negator at `i` negates a listed verb: one is among the two words
/// after it ("was not applied", "has not been used").
fn negates_listed_verb(ws: &[Word], i: usize) -> bool {
    ws[i + 1..]
        .iter()
        .take(2)
        .any(|w| LISTED_VERBS.contains(&w.text.as_str()))
}

/// `without` at `i` is the end of "with and without" or "with or without".
fn with_and_without(ws: &[Word], i: usize) -> bool {
    ws[i].text == "without"
        && i >= 2
        && matches!(ws[i - 1].text.as_str(), "and" | "or")
        && ws[i - 2].text == "with"
}

const CONJUNCTIONS: &[&str] = &[
    "and", "but", "or", "nor", "while", "whereas", "although", "though", "yet", "so", "because",
    "since", "which", "who", "whom", "whose", "where", "when",
];
const SUBJECTS: &[&str] = &[
    "we", "i", "they", "he", "she", "it", "this", "these", "those", "there", "our", "their", "all",
    "each",
];
const AUXILIARIES: &[&str] = &[
    "is", "are", "was", "were", "be", "been", "being", "do", "does", "did", "has", "have", "had",
    "will", "would", "shall", "should", "can", "could", "may", "might", "must",
];

/// One word: its normalised text (lower case, `’` as `'`) and its byte
/// offset in the slice it came from.
struct Word {
    text: String,
    at: usize,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}'
}

fn words(s: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut cur: Option<(usize, String)> = None;
    for (i, c) in s.char_indices() {
        if is_word_char(c) {
            let ch = if c == '\u{2019}' { '\'' } else { c };
            match &mut cur {
                Some((_, w)) => w.extend(ch.to_lowercase()),
                None => cur = Some((i, ch.to_lowercase().collect())),
            }
        } else if let Some((at, text)) = cur.take() {
            out.push(Word { text, at });
        }
    }
    if let Some((at, text)) = cur {
        out.push(Word { text, at });
    }
    out
}

fn is_aux_contraction(w: &str) -> bool {
    w.ends_with("n't")
}

fn is_negator(ws: &[Word], i: usize) -> bool {
    let w = ws[i].text.as_str();
    let prev = |p: &str| i > 0 && ws[i - 1].text == p;
    matches!(
        w,
        "not" | "no" | "never" | "without" | "neither" | "nor" | "cannot"
    ) || w.ends_with("n't")
        || (w == "than" && prev("rather"))
        || (w == "of" && prev("instead"))
}

/// A `.` between two digits is a decimal point.
fn is_boundary(text: &str, i: usize, c: char) -> bool {
    match c {
        ';' | ':' | '?' | '!' => true,
        '.' => {
            let prev = text[..i].chars().next_back();
            let next = text[i + 1..].chars().next();
            !(prev.is_some_and(|p| p.is_ascii_digit()) && next.is_some_and(|n| n.is_ascii_digit()))
        }
        ',' => {
            let rest = &text[i + 1..];
            let next: String = rest
                .trim_start()
                .chars()
                .take_while(|c| is_word_char(*c))
                .flat_map(char::to_lowercase)
                .collect();
            CONJUNCTIONS.contains(&next.as_str()) || SUBJECTS.contains(&next.as_str())
        }
        _ => false,
    }
}

/// The byte range of the clause around `[start, end)`.
fn clause_around(text: &str, start: usize, end: usize) -> (usize, usize) {
    let from = text[..start]
        .char_indices()
        .rev()
        .find(|&(i, c)| is_boundary(text, i, c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let to = text[end..]
        .char_indices()
        .find(|&(i, c)| is_boundary(text, end + i, c))
        .map_or(text.len(), |(i, _)| end + i);
    (from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The answer for `phrase` inside `text`, at its first occurrence
    /// (case-insensitive).
    fn ans(text: &str, phrase: &str) -> Negation {
        let lower = text.to_lowercase();
        let at = lower
            .find(&phrase.to_lowercase())
            .unwrap_or_else(|| panic!("{phrase:?} not in {text:?}"));
        negation_of(text, at, at + phrase.len())
    }

    #[test]
    fn a_negator_just_before_the_phrase_negates_it() {
        assert_eq!(
            ans("We did not use one-tailed tests.", "one-tailed"),
            Negation::Clearly
        );
        assert_eq!(
            ans("No one-tailed tests were used.", "one-tailed"),
            Negation::Clearly
        );
        assert_eq!(
            ans("Without Yates' correction, the groups differed.", "Yates"),
            Negation::Clearly
        );
        assert_eq!(
            ans(
                "We used Welch's test rather than Student's t-test.",
                "Student's t"
            ),
            Negation::Clearly
        );
        assert_eq!(
            ans(
                "We used Welch's test instead of Student's t-test.",
                "Student's t"
            ),
            Negation::Clearly
        );
        assert_eq!(
            ans("We didn't use one-tailed tests.", "one-tailed"),
            Negation::Clearly
        );
        assert_eq!(
            ans("We didn’t use one-tailed tests.", "one-tailed"),
            Negation::Clearly
        );
        assert_eq!(
            ans(
                "Neither one-tailed nor two-tailed tests were run.",
                "one-tailed"
            ),
            Negation::Clearly
        );
    }

    #[test]
    fn a_negated_predicate_negates_the_phrase() {
        assert_eq!(
            ans("One-tailed tests were not used.", "one-tailed"),
            Negation::Clearly
        );
        assert_eq!(
            ans(
                "Equal variances were not assumed; t(15) = 1.35.",
                "equal variances"
            ),
            Negation::Clearly
        );
        assert_eq!(
            ans("Equal variances weren't assumed.", "equal variances"),
            Negation::Clearly
        );
        assert_eq!(
            ans("A Bonferroni correction was not applied.", "Bonferroni"),
            Negation::Clearly
        );
    }

    #[test]
    fn a_negator_after_a_comma_negates_something_else() {
        assert_eq!(
            ans("Tests were one-tailed, not two-tailed.", "one-tailed"),
            Negation::Not
        );
        assert_eq!(
            ans(
                "After Bonferroni correction, no effect remained.",
                "Bonferroni"
            ),
            Negation::Not
        );
        assert_eq!(
            ans("Student's t-test, not Welch's, was used.", "Student's t"),
            Negation::Not
        );
        // "rather than" and "instead of" negate what follows them.
        assert_eq!(
            ans(
                "We used Welch's t-test rather than Student's t-test.",
                "Welch's"
            ),
            Negation::Not
        );
        assert_eq!(
            ans(
                "We used Welch's t-test instead of Student's t-test.",
                "Welch's"
            ),
            Negation::Not
        );
    }

    #[test]
    fn a_negator_further_back_or_further_on_is_unclear() {
        assert_eq!(
            ans(
                "No significant effects survived the Bonferroni correction.",
                "Bonferroni"
            ),
            Negation::Unclear
        );
        // Review follow-up, item 1 (refined): past a comma, a negator that
        // negates a listed verb ("assume") is clear. Was Unclear before.
        assert_eq!(
            ans(
                "Equal variances, by Levene's test, were not assumed.",
                "equal variances"
            ),
            Negation::Clearly
        );
        assert_eq!(
            ans(
                "We could not confirm that the groups had equal variances.",
                "equal variances"
            ),
            Negation::Unclear
        );
    }

    #[test]
    fn clauses_end_where_the_rule_says() {
        // A full stop, a semicolon and a colon end the clause.
        assert_eq!(
            ans(
                "Nothing was excluded. One-tailed tests were used.",
                "one-tailed"
            ),
            Negation::Not
        );
        assert_eq!(
            ans(
                "No outliers were removed; one-tailed tests were used.",
                "one-tailed"
            ),
            Negation::Not
        );
        assert_eq!(
            ans("No outliers: one-tailed tests were used.", "one-tailed"),
            Negation::Not
        );
        // A decimal point does not.
        assert_eq!(
            ans(
                "No effect at p = 0.05 used a one-tailed test.",
                "one-tailed"
            ),
            Negation::Unclear
        );
        // A comma before a conjunction or a new subject ends it.
        assert_eq!(
            ans(
                "No outliers were removed, and one-tailed tests were used.",
                "one-tailed"
            ),
            Negation::Not
        );
        assert_eq!(
            ans(
                "No outliers were removed, we used one-tailed tests.",
                "one-tailed"
            ),
            Negation::Not
        );
        assert_eq!(
            ans(
                "We used Welch's t-test, which does not assume equal variances.",
                "equal variances"
            ),
            Negation::Clearly
        );
        // A comma before anything else does not.
        assert_eq!(
            ans(
                "Although no outliers were found, one-tailed tests were used.",
                "one-tailed"
            ),
            Negation::Unclear
        );
    }

    #[test]
    fn nothing_negative_is_not_negated() {
        assert_eq!(
            ans("One-tailed tests were used.", "one-tailed"),
            Negation::Not
        );
        assert_eq!(
            ans(
                "Assuming equal variances, the groups differed.",
                "equal variances"
            ),
            Negation::Not
        );
        assert_eq!(ans("P values were FDR-corrected.", "FDR"), Negation::Not);
        // "no" inside another word is not a negator.
        assert_eq!(
            ans("Nonetheless, one-tailed tests were used.", "one-tailed"),
            Negation::Not
        );
        assert_eq!(
            ans("Notably, one-tailed tests were used.", "one-tailed"),
            Negation::Not
        );
    }
    /// Review follow-up, item 1: after the phrase, a negator is clear only
    /// when it negates one of the listed verbs (use, apply, perform,
    /// conduct, make, assume, adjust, correct, employ, in any tense).
    /// Anything else after the phrase, and "with and without" or "with or
    /// without" before it, is unclear.
    #[test]
    fn after_the_phrase_only_a_negated_listed_verb_is_clear() {
        for (text, phrase) in [
            (
                "Bonferroni correction did not change the results.",
                "Bonferroni",
            ),
            ("The FDR correction did not alter any conclusion.", "FDR"),
            (
                "One-tailed tests did not differ from two-tailed ones.",
                "one-tailed",
            ),
        ] {
            assert_eq!(ans(text, phrase), Negation::Unclear, "{text:?}");
        }
        for (text, phrase) in [
            (
                "Results were similar with and without Bonferroni correction.",
                "Bonferroni",
            ),
            ("With or without FDR correction, the effect held.", "FDR"),
        ] {
            assert_eq!(ans(text, phrase), Negation::Unclear, "{text:?}");
        }
        for (text, phrase) in [
            ("A Bonferroni correction was not applied.", "Bonferroni"),
            ("One-tailed tests were not used.", "one-tailed"),
            ("Equal variances were not assumed.", "equal variances"),
            ("Equal variances weren't assumed.", "equal variances"),
            ("The p values were not corrected.", "p values"),
            ("Holm's procedure was not employed.", "Holm"),
            ("A Tukey test was not performed.", "Tukey"),
            ("One-tailed tests were never conducted.", "one-tailed"),
            ("An FDR adjustment was not made.", "FDR"),
            ("Bonferroni was not adjusting anything here.", "Bonferroni"),
        ] {
            assert_eq!(ans(text, phrase), Negation::Clearly, "{text:?}");
        }
        // Past a comma: a negated listed verb is clear; any other negator
        // after an auxiliary is unclear; a bare negator negates something
        // else.
        assert_eq!(
            ans("One-tailed tests, however, were not used.", "one-tailed"),
            Negation::Clearly
        );
        assert_eq!(
            ans(
                "A Bonferroni correction, as planned, was not applied.",
                "Bonferroni"
            ),
            Negation::Clearly
        );
        assert_eq!(
            ans(
                "Bonferroni correction, however, did not change the results.",
                "Bonferroni"
            ),
            Negation::Unclear
        );
        assert_eq!(
            ans("Tests were one-tailed, not two-tailed.", "one-tailed"),
            Negation::Not
        );
        // Before the phrase, a nearby negator is still clear.
        assert_eq!(
            ans("No Bonferroni correction was applied.", "Bonferroni"),
            Negation::Clearly
        );
    }
}
