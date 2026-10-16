//! **`ReportedResult`: one complete reported test, read on positive evidence.**
//!
//! The design calls this a Claim; `Claim` and `StatClaim` already name other
//! types in this crate.
//!
//! # Why a second reader exists beside `extract::stats`
//!
//! `extract::stats` keeps each match's raw text but parses every number to
//! `f64` and drops the printed string, keeps degrees of freedom only for F, and
//! never links a statistic to its own p-value: each item is a separate
//! `StatClaim` that merely shares a paragraph `Location`. Level 0 needs the
//! complete unit, so this reads it. `extract::stats` is left untouched: `Stat`
//! derives `Serialize` and feeds `validate.rs`, PublishReady and cached reports.
//!
//! # What it shares with extraction, so there is one answer to each question
//!
//! - **Which paragraphs.** It walks the `sections` an [`ExtractionResult`]
//!   already holds, built by `sections::split_document`, skipping References
//!   exactly as `extract_from_text_with` does, and gives each result the same
//!   [`Location::in_section`].
//! - **Where the p-values are.** It finds them with `stats::regexes().p_value`,
//!   the pattern that produces `Stat::PValue`, and [`coupling_violations`]
//!   checks every accepted p against a `Stat::PValue` at the same location.
//! - **Which p-values are thresholds.** The stated alpha is read HERE, by
//!   [`ALPHA_WORDING`]: a closed list of alpha phrases with the value beside
//!   them, never a value inside the span of a parsed result. The evaluation
//!   found that `Stat::SignificanceThreshold` (a marker phrase anywhere in the
//!   sentence) took a result's own p as the paper's alpha when the two shared
//!   a sentence. Extraction's classification is still used for one thing: a
//!   p it called a threshold is not counted as `PWithoutStatistic`.
//! - **Which names are effect sizes.** [`stats::EFFECT_SIZE_ALTERNATION`].
//!
//! Every number goes through [`ReportedNumber`]; the regexes here only locate
//! the printed token.
//!
//! # Acceptance
//!
//! A unit is accepted only when the statistic, its degrees of freedom (none for
//! z) and its p-value appear in that order, joined by `, ` separators, with
//! nothing between them but recognised items: `MSE = x`, ONE effect size, ONE
//! confidence interval. Anything else between them, a `;`, a sentence end or a
//! closing parenthesis ends the unit, and a statistic whose unit ends before a
//! p is not read. A statistic and a p-value that merely share a paragraph are
//! never paired.
//!
//! For t and chi-square only, the df may instead be printed as a labelled
//! item straight after the value and straight before the p:
//! `χ² = 12.34, df = 2, p < .001` (also `d.f. = 2`, `df=2`, `DF = 2`). The labelled df
//! is read exactly as a bracketed one. A labelled df on any other family
//! (`F = 4.2, df = 2, 57`), or beside a bracketed df, is
//! `MalformedDegreesOfFreedom`. A labelled df after the p is not part of the
//! unit, so `χ² = 12.34, p < .001, df = 2` stays `MissingDegreesOfFreedom`.
//!
//! After the p, the same unit may carry an effect size and an interval it did
//! not already have (`p < .001, η² = .245`), one of each in all, under the same
//! rules. Anything else there simply ends the unit.
//!
//! An accepted interval records the item it follows directly
//! ([`CiFollows`]): only one straight after an effect size can be read as that
//! effect size's interval. Each result also records whether its sentence says
//! partial, semi-partial or controlling, for `df_matches_n`.
//!
//! Each attempted unit is counted once, with its first reason. A rejected
//! attempt claims the first p-value after it in the same sentence unless
//! another statistic is printed between them, and that p is not counted again
//! as `PWithoutStatistic`.

use std::sync::OnceLock;

use regex::Regex;

use super::number::{NumberNotRead, ReportedNumber, Relation};
use crate::extract::stats::{self, Stat};
use crate::extract::{ExtractionResult, Location, SectionKind};

/// The test a result reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    T,
    F,
    ChiSquare,
    R,
    Z,
}

/// The p-value as printed: a number with its relation, or `ns`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PValue {
    Reported(ReportedNumber),
    NotSignificant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSize {
    /// The measure as printed (`d`, `η2`, `Cohen's d`), trailing `=` trimmed.
    pub name: String,
    pub value: ReportedNumber,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfidenceInterval {
    pub level: ReportedNumber,
    pub low: ReportedNumber,
    pub high: ReportedNumber,
    /// The item printed straight before the interval in the unit.
    pub follows: CiFollows,
}

/// What a confidence interval follows directly inside its unit. Only an
/// interval straight after an effect size can be taken to be that effect
/// size's interval; one anywhere else may belong to an estimate that is not
/// printed (a mean difference).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiFollows {
    Statistic,
    Mse,
    EffectSize,
    PValue,
}

/// Where the paragraph came from.
///
/// Three states, because a PDF cannot answer the question. `TableCell` and
/// `Prose` are known only for a format whose file declares its tables (a
/// .docx read by `extract_path`): the paragraph is or is not inside a cell of
/// `ExtractionResult::doc_tables`. A PDF or plain text has no table objects,
/// so its table rows arrive flattened into paragraphs and every paragraph is
/// `Unknown`, never `Prose`.
///
/// Known limit: a .docx whose table parse failed reaches here with an empty
/// `doc_tables` (`extract_path` turns the failure into an empty vector), so its
/// paragraphs read as `Prose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Prose,
    TableCell,
    Unknown,
}

/// Whether the document's format declares its tables, which decides whether
/// [`Source`] can be known. The caller knows the format; an
/// [`ExtractionResult`] does not record it, and an empty `doc_tables` is the
/// same value for a PDF and for a .docx with no tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStructure {
    /// A .docx: `doc_tables` holds every table the file declares (possibly none).
    Declared,
    /// A PDF or plain text: tables, if any, are flattened into paragraphs.
    NotAvailable,
}

/// One complete reported result. Deliberately NOT `Serialize`: `raw` and the
/// effect-size name are manuscript text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedResult {
    /// Position in document order, from 0.
    pub id: usize,
    pub location: Location,
    /// The printed unit, `t(58) = 2.31, p = .024`.
    pub raw: String,
    pub family: Family,
    pub statistic: ReportedNumber,
    /// Zero (z), one (t, r, chi-square) or two (F) values. An integer df is a
    /// count; a fractional one (Welch's 28.4) is measured.
    pub df: Vec<ReportedNumber>,
    /// Only when printed inside the unit, as in `χ2(1, N = 226)`.
    pub n: Option<ReportedNumber>,
    pub p: PValue,
    pub effect_size: Option<EffectSize>,
    pub ci: Option<ConfidenceInterval>,
    /// The sentence holding the unit says partial, semi-partial or
    /// controlling, anywhere in it.
    pub sentence_says_partial: bool,
    pub source: Source,
}

/// Why something was not read as a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// A statistic whose sentence holds no p-value after it.
    StatisticWithoutP,
    /// A p-value (as `Stat::PValue` finds them) that no unit contains.
    PWithoutStatistic,
    /// `F(1, 15) = 19.9 and 5.16`: two values in one clause.
    TwoResultsInOneClause,
    /// A p follows in the same sentence, but the unit was broken before it by
    /// something that is not a recognised item. Near is not linked.
    NotLinked,
    /// t, F, r or chi-square with no `(df)`, and t or chi-square with no
    /// labelled `df =` between the value and the p either.
    MissingDegreesOfFreedom,
    /// The wrong number of df for the family, an empty part, an `N =` outside
    /// a chi-square, a df on z, or a labelled `df =` on a family other than t
    /// and chi-square or beside a bracketed df.
    MalformedDegreesOfFreedom,
    /// A second effect size or a second interval in one unit.
    RepeatedItem,
    /// A number in the unit that [`ReportedNumber`] refused.
    Number(NumberNotRead),
}

/// Something not read, and why. Carries no manuscript text.
///
/// Each attempted unit is counted ONCE, with its first reason; a p-value that
/// a rejected unit reached is not counted again as `PWithoutStatistic`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotRead {
    pub location: Location,
    pub reason: Reason,
    /// Byte range in the paragraph at `location`: the attempted unit, through
    /// the p-value it reached if any, or the lone p-value. `None` for a
    /// threshold whose number was refused (extraction keeps no offset for it).
    pub span: Option<std::ops::Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultReading {
    pub results: Vec<ReportedResult>,
    pub not_read: Vec<NotRead>,
    /// The document's distinct stated significance thresholds, read by
    /// [`ALPHA_WORDING`] through [`ReportedNumber::p_value`] (and
    /// [`ReportedNumber::divided_by`] for a fraction), in document order.
    pub thresholds: Vec<ReportedNumber>,
}

struct Patterns {
    head: Regex,
    sep: Regex,
    ns: Regex,
    mse: Regex,
    ci: Regex,
    effect: Regex,
    effect_tail: Regex,
    folded: Regex,
    n_part: Regex,
    df_label: Regex,
    partial: Regex,
    p_at: Regex,
    /// Alpha wording, then the value: `α = .05`, `alpha of .05`, `significance
    /// level of .05`, `criterion of .05`, `significance was set at p < .05`.
    alpha_before: Regex,
    /// The value, then the wording: `p < .05 was considered significant`.
    alpha_after: Regex,
    /// A further value in the same statement: `…p < .05, and at p < .01 for…`.
    alpha_more: Regex,
}

/// The closed list of wordings that state a significance threshold. Group 1
/// is the operator when a `p` precedes the value, group 2 the value, group 3 a
/// divisor when the threshold is printed as a fraction (`.05/6`).
///
/// "alpha" and "criterion" take a value only through a connector written
/// straight after them (`α = .05`, `alpha of .05`, `alpha level was .05`,
/// `alpha was set at .05`, `criterion of .05`): "alpha" is also an EEG band
/// and a power, and "criterion" a training target. The significance-level
/// phrases allow a few words before their connector ("the significance level
/// for all tests was set at .05"). A value above .2 is never a threshold (a
/// reliability coefficient is the known collision), a sentence naming
/// Cronbach is skipped outright, and a candidate the wording does not take
/// is not a number at all: never an unread p.
pub const ALPHA_WORDING: [&str; 3] = [
    concat!(
        r"(?i)(?:\b(?:α|alpha)(?:\s+(?:level|value|criterion))?\s*",
        r"(?:was\s+set\s+(?:at|to)|were\s+set\s+(?:at|to)|is\s+set\s+(?:at|to)|set\s+(?:at|to)|was|were|is|=|:|of|at)",
        r"|\b(?:significance\s+(?:level|threshold|criterion)|level\s+of\s+significance",
        r"|criterion\s+(?:for|of)\s+(?:statistical\s+)?significance",
        // "The Bonferroni-corrected threshold was .05/6" (part C, m-3).
        r"|(?:corrected|adjusted)\s+(?:significance\s+)?(?:threshold|alpha|α|level))[^\d;.]{0,30}?",
        r"(?:was\s+set\s+(?:at|to)|were\s+set\s+(?:at|to)|is\s+set\s+(?:at|to)|set\s+(?:at|to)|was|were|is|=|:|of|at)",
        r"|\bsignificance\s+was\s+(?:set|determined|defined|fixed)(?:\s+(?:at|to|as))?",
        r"|\bcriterion\s+of",
        r"|\b(?:statistically\s+)?significan(?:t|ce)\s+(?:at|when|if|as))",
        r"\s*(?:\bp\s*(?:-?\s*values?)?\s*(<=|<|≤|=)\s*)?",
        r"(\d*\.\d+|\d+(?:\.\d+)?)(?:\s*/\s*(\d+))?",
    ),
    concat!(
        // "below .01" and "less than .01" read as `<` (part C, m-3).
        r"(?i)\bp\s*(?:-?\s*values?)?\s*(<=|<|≤|below|less\s+than|under)\s*",
        r"(\d*\.\d+|\d+(?:\.\d+)?)(?:\s*/\s*(\d+))?",
        r"\s+(?:was|were|is|are)\s+(?:considered|regarded|taken|deemed|defined|treated)(?:\s+as)?\s+(?:statistically\s+)?significant",
    ),
    // A continuation, matched only where the previous value ended; a short
    // scope may come between ("p < .05 for main effects and p < .01 for
    // interactions", part C, m-3).
    concat!(
        r"(?i)^(?:\s+for\s+[^\d;.,]{1,40}?)?\s*,?\s*(?:and|or)\s+(?:at\s+)?",
        r"(?:\bp\s*(?:-?\s*values?)?\s*(<=|<|≤|=)\s*)?",
        r"(\d*\.\d+|\d+(?:\.\d+)?)(?:\s*/\s*(\d+))?",
    ),
];

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // The statistic symbol, not inside a word, an optional `(df)`, then `=`.
        // t, r, z lowercase as printed; F uppercase; Z either. `T` (Wilcoxon) and
        // `R` (multiple correlation) are different statistics and are not read.
        head: Regex::new(
            r"(?:^|[^\p{L}\p{N}_'’])(t|F|r|z|Z|χ ?2|χ²|chi2|chi²)\s*(?:\(([^()]*)\))?\s*=",
        )
        .unwrap(),
        // Every pattern below is anchored: see `at`.
        sep: Regex::new(r"^\s*,\s*").unwrap(),
        // `n.s.` keeps its own final dot; bare `ns` does not take a full stop.
        ns: Regex::new(r"^(?i:n\.s\.|ns\b)").unwrap(),
        mse: Regex::new(r"^MS[Ee]\s*=").unwrap(),
        ci: Regex::new(
            r"^(?i)(\d{2}(?:\.\d+)?)\s*%\s*(?:CI|confidence\s+interval)\s*[:=]?\s*[\[(]\s*([^\s,;\[\]()]+)\s*[,;]\s*([^\s,;\[\]()]+)\s*[\])]",
        )
        .unwrap(),
        effect: Regex::new(&format!("^(?i:{})", stats::EFFECT_SIZE_ALTERNATION)).unwrap(),
        effect_tail: Regex::new(r"^(?i)\s*(?:=|:|of)?\s*").unwrap(),
        folded: Regex::new(r"^\s*(?:,\s*|\s(?:and|&)\s+)[-\u{2212}]?\.?\d").unwrap(),
        n_part: Regex::new(r"^[Nn]\s*=\s*(.*)$").unwrap(),
        // `df =`, `df=`, `d.f. =`, `d. f. =`, in any case (`DF =`).
        df_label: Regex::new(r"^(?i)d(?:f|\.\s*f\.)\s*=").unwrap(),
        // `stats::regexes().p_value`, anchored, for the same reason.
        p_at: Regex::new(&format!("^(?:{})", stats::regexes().p_value.as_str())).unwrap(),
        // `semi-partial` contains `partial` after a word boundary.
        partial: Regex::new(r"(?i)\b(?:partial|controlling)\b").unwrap(),
        alpha_before: Regex::new(ALPHA_WORDING[0]).unwrap(),
        alpha_after: Regex::new(ALPHA_WORDING[1]).unwrap(),
        alpha_more: Regex::new(ALPHA_WORDING[2]).unwrap(),
    })
}

/// A match of `re` at exactly `pos`: `re` is anchored with `^` and run on
/// `s[pos..]`, so a miss costs the length of the item, not of the paragraph.
/// (`find_at(s, pos)` with a later filter searched to the end of the paragraph
/// on every miss, which the Phase 1 evaluation measured as quadratic in the
/// results per paragraph.) Returns the absolute end and the matched text.
/// Every pattern given to this function is compiled with a leading `^`, and
/// none starts with `\b`, so running it on the slice changes nothing but cost.
fn at<'h>(re: &Regex, s: &'h str, pos: usize) -> Option<(usize, &'h str)> {
    re.find(&s[pos..]).map(|m| (pos + m.end(), m.as_str()))
}

/// The captures of an anchored `re` at exactly `pos`, as absolute `(start,
/// end)` of the whole match and of each numbered group.
fn captures_at<'h>(re: &Regex, s: &'h str, pos: usize) -> Option<(usize, Vec<Option<(usize, usize)>>)> {
    let c = re.captures(&s[pos..])?;
    let whole = c.get(0).expect("group 0");
    let groups = (1..c.len()).map(|i| c.get(i).map(|g| (pos + g.start(), pos + g.end()))).collect();
    Some((pos + whole.end(), groups))
}

/// The printed number token starting at `from` (after optional spaces): the
/// run of characters a number, or a malformed one, can be printed with.
/// Trailing `.,;:` are sentence punctuation and are left out, so `p = .024.`
/// gives `.024`. Returns the token and the byte offset just past it.
fn scan_token(s: &str, from: usize) -> Option<(&str, usize)> {
    let rest = &s[from..];
    let start = from + (rest.len() - rest.trim_start().len());
    let mut end = start;
    for (i, c) in s[start..].char_indices() {
        let ok = c.is_ascii_digit()
            || matches!(c, '.' | ',' | 'e' | 'E' | '×' | '%' | '-' | '\u{2212}' | '\u{2013}' | '+' | '^');
        if !ok {
            break;
        }
        end = start + i + c.len_utf8();
    }
    let tok = s[start..end].trim_end_matches(['.', ',', ';', ':']);
    if tok.is_empty() {
        None
    } else {
        Some((tok, start + tok.len()))
    }
}

fn relation_of(op: &str) -> Relation {
    match op {
        "<" => Relation::Lt,
        "<=" | "≤" => Relation::Le,
        ">" => Relation::Gt,
        ">=" | "≥" => Relation::Ge,
        // The alpha wording's "below .01", "less than .01", "under .01".
        w if ["below", "less", "under"].iter().any(|x| w.to_ascii_lowercase().starts_with(x)) => Relation::Lt,
        _ => Relation::Eq,
    }
}

fn family_of(symbol: &str) -> Family {
    match symbol {
        "t" => Family::T,
        "F" => Family::F,
        "r" => Family::R,
        "z" | "Z" => Family::Z,
        _ => Family::ChiSquare,
    }
}

/// A df value: a count when printed as an integer, measured when it carries a
/// decimal point (Welch's 28.4).
fn df_number(tok: &str) -> Result<ReportedNumber, Reason> {
    match ReportedNumber::count(tok) {
        Err(NumberNotRead::DecimalInCount) => ReportedNumber::measured(tok).map_err(Reason::Number),
        other => other.map_err(Reason::Number),
    }
}

fn parse_df(
    family: Family,
    inner: Option<&str>,
    labelled: Option<&str>,
) -> Result<(Vec<ReportedNumber>, Option<ReportedNumber>), Reason> {
    // `scan_unit` admits a labelled df only for t and chi-square with no `(df)`.
    if let Some(tok) = labelled {
        return Ok((vec![df_number(tok)?], None));
    }
    let Some(inner) = inner else {
        return if family == Family::Z { Ok((Vec::new(), None)) } else { Err(Reason::MissingDegreesOfFreedom) };
    };
    if family == Family::Z {
        return Err(Reason::MalformedDegreesOfFreedom);
    }
    let mut df = Vec::new();
    let mut n = None;
    for part in inner.split(',').map(str::trim) {
        if part.is_empty() {
            return Err(Reason::MalformedDegreesOfFreedom);
        }
        if let Some(c) = patterns().n_part.captures(part) {
            if family != Family::ChiSquare || n.is_some() {
                return Err(Reason::MalformedDegreesOfFreedom);
            }
            n = Some(ReportedNumber::count(c[1].trim()).map_err(Reason::Number)?);
        } else {
            df.push(df_number(part)?);
        }
    }
    let want = if family == Family::F { 2 } else { 1 };
    if df.len() != want {
        return Err(Reason::MalformedDegreesOfFreedom);
    }
    Ok((df, n))
}

/// The printed tokens of one unit, before any of them is read as a number.
struct Unit<'h> {
    stat_start: usize,
    end: usize,
    family: Family,
    df_inner: Option<&'h str>,
    /// The df token of `, df = 2`, printed between the value and the p.
    df_labelled: Option<&'h str>,
    value: &'h str,
    p: Option<(Relation, &'h str)>, // None = ns
    effect: Option<(&'h str, &'h str)>,
    ci: Option<(&'h str, &'h str, &'h str, CiFollows)>,
    mse: Option<&'h str>,
    /// The item most recently accepted, for [`CiFollows`].
    last: CiFollows,
}

enum Scan<'h> {
    Complete(Unit<'h>),
    /// Rejected. `end` is where scanning for the next statistic resumes;
    /// `p` is the p-value this attempt reached, if any, which is therefore not
    /// counted again on its own.
    Rejected { reason: Reason, end: usize, p: Option<(usize, usize)> },
}

/// The first p-value after `from` in the sentence holding `stat_start`, if no
/// other statistic is printed between them. That p belongs to this attempt;
/// with a statistic in between it belongs to that one.
fn claimed_p(cache: &ScanCache, s: &str, stat_start: usize, from: usize) -> Option<(usize, usize)> {
    let sentence = cache.sentence(s, stat_start);
    let sent_start = sentence.as_ptr() as usize - s.as_ptr() as usize;
    let sent_end = sent_start + sentence.len();
    let (m_start, m_end) = cache.next_p(s, sent_end, from)?;
    let between = patterns().head.find_at(&s[..m_start], from).is_some();
    (!between).then_some((m_start, m_end))
}

/// One paragraph's memo for the two searches that made a long clause
/// quadratic: 70,000 statistics with no p in one clause took 58.5 s, because
/// every unit found its sentence again and searched to the sentence's end
/// for a p (phase 2 evaluation fixes, part C, m-6). Both answers are reused
/// only where they are provably the same:
///
/// - a sentence's bounds are the same for every position inside them
///   ([`crate::extract::sentence::sentence_bounds`]);
/// - the leftmost p found searching from `f0` within one sentence is also the
///   leftmost from any later start up to it (the haystack is the same, and
///   no p starts between the two starts), and when there is none from `f0`
///   there is none from later.
#[derive(Default)]
struct ScanCache {
    sentence: std::cell::Cell<Option<(usize, usize)>>,
    next_p: std::cell::Cell<Option<(usize, usize, Option<(usize, usize)>)>>,
}

impl ScanCache {
    fn sentence<'h>(&self, s: &'h str, at: usize) -> &'h str {
        if let Some((a, b)) = self.sentence.get() {
            if a <= at && at < b && s.is_char_boundary(at) {
                return s[a..b].trim();
            }
        }
        match crate::extract::sentence::sentence_bounds(s, at) {
            Some((a, b)) => {
                self.sentence.set(Some((a, b)));
                s[a..b].trim()
            }
            None => s.trim(),
        }
    }

    /// The first p at or after `from` in `s[..sent_end]`.
    fn next_p(&self, s: &str, sent_end: usize, from: usize) -> Option<(usize, usize)> {
        if let Some((end, f0, found)) = self.next_p.get() {
            if end == sent_end && from >= f0 && found.map_or(true, |(start, _)| start >= from) {
                return found;
            }
        }
        let found = stats::regexes().p_value.find_at(&s[..sent_end], from).map(|m| (m.start(), m.end()));
        self.next_p.set(Some((sent_end, from, found)));
        found
    }
}

/// The items allowed straight after the p: one effect size and one interval
/// in all, as between the statistic and the p. Anything else ends the unit
/// there. Returns the new end, or the rejection for a repeated item.
fn trailing_items<'h>(s: &'h str, unit: &mut Unit<'h>, mut pos: usize) -> Result<usize, (Reason, usize)> {
    let pt = patterns();
    loop {
        let Some((item, _)) = at(&pt.sep, s, pos) else { return Ok(pos) };
        if let Some((end, g)) = captures_at(&pt.ci, s, item) {
            if unit.ci.is_some() {
                return Err((Reason::RepeatedItem, end));
            }
            let text = |i: usize| g[i].map(|(a, b)| &s[a..b]).expect("ci group");
            unit.ci = Some((text(0), text(1), text(2), unit.last));
            pos = end;
            continue;
        }
        if let Some((m_end, name)) = at(&pt.effect, s, item) {
            let tail = at(&pt.effect_tail, s, m_end).map_or(m_end, |(t, _)| t);
            let Some((tok, end)) = scan_token(s, tail) else { return Ok(pos) };
            if unit.effect.is_some() {
                return Err((Reason::RepeatedItem, end));
            }
            unit.effect = Some((name, tok));
            unit.last = CiFollows::EffectSize;
            pos = end;
            continue;
        }
        return Ok(pos);
    }
}

/// Scan one unit whose statistic head ends (after `=`) at `eq_end`.
fn scan_unit<'h>(cache: &ScanCache, s: &'h str, stat_start: usize, symbol: &str, df_inner: Option<&'h str>, eq_end: usize) -> Scan<'h> {
    let pt = patterns();
    let family = family_of(symbol);
    let Some((value, value_end)) = scan_token(s, eq_end) else {
        let p = claimed_p(cache, s, stat_start, eq_end);
        let reason = if p.is_some() { Reason::NotLinked } else { Reason::StatisticWithoutP };
        return Scan::Rejected { reason, end: eq_end, p };
    };

    // `F(1, 15) = 19.9 and 5.16`: a second value straight after the first. A
    // comma followed by a confidence interval is an item, not a second value.
    if let Some((m_end, _)) = at(&pt.folded, s, value_end) {
        let after_comma = at(&pt.sep, s, value_end).map(|(e, _)| e);
        let is_ci = after_comma.is_some_and(|p| at(&pt.ci, s, p).is_some());
        if !is_ci {
            let p = claimed_p(cache, s, stat_start, value_end);
            return Scan::Rejected { reason: Reason::TwoResultsInOneClause, end: m_end, p };
        }
    }

    let mut unit = Unit {
        stat_start,
        end: value_end,
        family,
        df_inner,
        df_labelled: None,
        value,
        p: None,
        effect: None,
        ci: None,
        mse: None,
        last: CiFollows::Statistic,
    };
    let mut pos = value_end;

    // A labelled df: only as the first item after the value, and the p must
    // follow it directly (checked by `labelled_df_end` below).
    let mut labelled_df_end = None;
    if let Some((item, _)) = at(&pt.sep, s, value_end) {
        if let Some((m_end, _)) = at(&pt.df_label, s, item) {
            let allowed = matches!(family, Family::T | Family::ChiSquare) && df_inner.is_none();
            let tok = scan_token(s, m_end);
            let end = tok.map_or(m_end, |(_, e)| e);
            if !allowed {
                let p = claimed_p(cache, s, stat_start, end);
                return Scan::Rejected { reason: Reason::MalformedDegreesOfFreedom, end, p };
            }
            if let Some((tok, end)) = tok {
                unit.df_labelled = Some(tok);
                pos = end;
                labelled_df_end = Some(end);
            }
        }
    }

    loop {
        let Some((item, _)) = at(&pt.sep, s, pos) else { break };
        // The p, or `ns`, ends the required part; trailing items may follow.
        let p_end = if let Some((_, g)) = captures_at(&pt.p_at, s, item) {
            let (op_start, op_end) = g[0].expect("operator group");
            let Some((tok, end)) = scan_token(s, op_end) else { break };
            unit.p = Some((relation_of(&s[op_start..op_end]), tok));
            Some(end)
        } else {
            at(&pt.ns, s, item).map(|(e, _)| e)
        };
        if let Some(p_end) = p_end {
            unit.last = CiFollows::PValue;
            return match trailing_items(s, &mut unit, p_end) {
                Ok(end) => {
                    unit.end = end;
                    Scan::Complete(unit)
                }
                Err((reason, end)) => Scan::Rejected { reason, end, p: Some((item, p_end)) },
            };
        }
        // Nothing but the p may follow a labelled df.
        if labelled_df_end == Some(pos) {
            break;
        }
        if let Some((end, g)) = captures_at(&pt.ci, s, item) {
            if unit.ci.is_some() {
                let p = claimed_p(cache, s, stat_start, end);
                return Scan::Rejected { reason: Reason::RepeatedItem, end, p };
            }
            let text = |i: usize| g[i].map(|(a, b)| &s[a..b]).expect("ci group");
            unit.ci = Some((text(0), text(1), text(2), unit.last));
            pos = end;
            continue;
        }
        if let Some((m_end, _)) = at(&pt.mse, s, item) {
            let Some((tok, end)) = scan_token(s, m_end) else { break };
            unit.mse = Some(tok);
            unit.last = CiFollows::Mse;
            pos = end;
            continue;
        }
        if let Some((m_end, name)) = at(&pt.effect, s, item) {
            let tail = at(&pt.effect_tail, s, m_end).map_or(m_end, |(t, _)| t);
            let Some((tok, end)) = scan_token(s, tail) else { break };
            if unit.effect.is_some() {
                let p = claimed_p(cache, s, stat_start, end);
                return Scan::Rejected { reason: Reason::RepeatedItem, end, p };
            }
            unit.effect = Some((name, tok));
            unit.last = CiFollows::EffectSize;
            pos = end;
            continue;
        }
        break;
    }

    // The unit ended before a p. A p later in the same sentence, with no other
    // statistic before it, makes this NotLinked, and is this attempt's p.
    let p = claimed_p(cache, s, stat_start, value_end);
    let reason = if p.is_some() { Reason::NotLinked } else { Reason::StatisticWithoutP };
    Scan::Rejected { reason, end: value_end, p }
}

/// Read every printed number of a scanned unit. The first refusal rejects it.
fn build(cache: &ScanCache, unit: &Unit<'_>, s: &str, id: usize, location: &Location, source: Source) -> Result<ReportedResult, Reason> {
    let (df, n) = parse_df(unit.family, unit.df_inner, unit.df_labelled)?;
    let num = |t: &str| ReportedNumber::measured(t).map_err(Reason::Number);
    let statistic = num(unit.value)?;
    if let Some(m) = unit.mse {
        num(m)?;
    }
    let p = match unit.p {
        Some((rel, tok)) => PValue::Reported(ReportedNumber::p_value(tok, rel).map_err(Reason::Number)?),
        None => PValue::NotSignificant,
    };
    let effect_size = match unit.effect {
        Some((name, tok)) => Some(EffectSize {
            name: name.trim().trim_end_matches('=').trim().to_string(),
            value: num(tok)?,
        }),
        None => None,
    };
    let ci = match unit.ci {
        Some((level, lo, hi, follows)) => {
            Some(ConfidenceInterval { level: num(level)?, low: num(lo)?, high: num(hi)?, follows })
        }
        None => None,
    };
    let sentence = cache.sentence(s, unit.stat_start);
    Ok(ReportedResult {
        id,
        location: location.clone(),
        raw: s[unit.stat_start..unit.end].to_string(),
        family: unit.family,
        statistic,
        df,
        n,
        p,
        effect_size,
        ci,
        sentence_says_partial: patterns().partial.is_match(sentence),
        source,
    })
}

/// `TableCell` when `paragraph` sits inside a cell of a table the file
/// declares, `Prose` when the file declares its tables and this is not in one,
/// `Unknown` when the format declares none.
fn source_of(ex: &ExtractionResult, tables: TableStructure, paragraph: &str) -> Source {
    if tables == TableStructure::NotAvailable {
        return Source::Unknown;
    }
    let in_cell = ex
        .doc_tables
        .iter()
        .flat_map(|t| t.rows.iter().flatten())
        .any(|cell| !cell.is_empty() && cell.contains(paragraph));
    if in_cell {
        Source::TableCell
    } else {
        Source::Prose
    }
}

/// Read every complete result, everything not read with its reason, and the
/// document's distinct significance thresholds. `tables` says whether the
/// document's format declares its tables; see [`Source`].
pub fn read_results(ex: &ExtractionResult, tables: TableStructure) -> ResultReading {
    let pt = patterns();
    let p_re = &stats::regexes().p_value;
    let mut results = Vec::new();
    let mut not_read = Vec::new();
    let mut thresholds: Vec<ReportedNumber> = Vec::new();

    for (s_idx, section) in ex.sections.iter().enumerate() {
        // Reference lists are not scanned for statistics, as in extraction.
        if section.kind == SectionKind::References {
            continue;
        }
        for (p_idx, para) in section.paragraphs.iter().enumerate() {
            let location = Location::in_section(section.kind, s_idx, p_idx);
            let s = para.as_str();
            // Spans whose p-value a unit already accounts for.
            let mut covered: Vec<(usize, usize)> = Vec::new();
            let cache = ScanCache::default();
            // Spans of stated thresholds in this paragraph.
            let mut threshold_spans: Vec<(usize, usize)> = Vec::new();
            let mut pos = 0;
            while let Some(c) = pt.head.captures_at(s, pos) {
                let sym = c.get(1).expect("symbol group");
                let eq_end = c.get(0).expect("group 0").end();
                let df_inner = c.get(2).map(|m| m.as_str());
                match scan_unit(&cache, s, sym.start(), sym.as_str(), df_inner, eq_end) {
                    Scan::Complete(unit) => {
                        covered.push((unit.stat_start, unit.end));
                        let source = source_of(ex, tables, s);
                        match build(&cache, &unit, s, results.len(), &location, source) {
                            Ok(r) => results.push(r),
                            Err(reason) => not_read.push(NotRead {
                                location: location.clone(),
                                reason,
                                span: Some(unit.stat_start..unit.end),
                            }),
                        }
                        pos = unit.end;
                    }
                    Scan::Rejected { reason, end, p } => {
                        let span_end = p.map_or(end, |(_, pe)| pe.max(end));
                        if let Some(p) = p {
                            covered.push(p);
                        }
                        not_read.push(NotRead { location: location.clone(), reason, span: Some(sym.start()..span_end) });
                        pos = end.max(eq_end);
                    }
                }
            }

            // The stated alpha: alpha wording with its value, outside every
            // unit's span. Found here so that a threshold is never a parsed
            // result's own p, and so that a fraction (`.05/6`) is visible.
            // (start, end, operator, value, divisor) of each stated value,
            // continuations ("…, and at p < .01") included.
            let mut stated: Vec<(usize, usize, Option<&str>, &str, Option<&str>)> = Vec::new();
            for re in [&pt.alpha_before, &pt.alpha_after] {
                for c in re.captures_iter(s) {
                    let whole = c.get(0).expect("group 0");
                    let value = c.get(2).expect("value group");
                    stated.push((whole.start(), whole.end(), c.get(1).map(|m| m.as_str()), value.as_str(), c.get(3).map(|m| m.as_str())));
                    let mut end = whole.end();
                    while let Some(m) = pt.alpha_more.captures(&s[end..]) {
                        let w = m.get(0).expect("group 0");
                        let v = m.get(2).expect("value group");
                        stated.push((end + w.start(), end + w.end(), m.get(1).map(|x| x.as_str()), v.as_str(), m.get(3).map(|x| x.as_str())));
                        end += w.end();
                    }
                }
            }
            for (start, end, op, value, divisor) in stated {
                {
                    let value_at = s[start..end].find(value).map_or(start, |i| start + i);
                    if covered.iter().any(|&(a, b)| value_at >= a && value_at < b) {
                        continue;
                    }
                    let sentence = crate::extract::sentence::sentence_containing(s, start).to_lowercase();
                    if sentence.contains("cronbach") {
                        continue;
                    }
                    let relation = op.map_or(Relation::Eq, relation_of);
                    let read = ReportedNumber::p_value(value, relation).and_then(|t| match divisor {
                        Some(d) => d.parse::<i128>().ok().and_then(|d| t.divided_by(d)).ok_or(NumberNotRead::TooManyDigits),
                        None => Ok(t),
                    });
                    match read {
                        // Above .2 it is not a significance threshold.
                        Ok(t) if exact_above_one_fifth(t.value()) => {}
                        Ok(t) => {
                            threshold_spans.push((start, end));
                            if !thresholds.iter().any(|k: &ReportedNumber| k.value() == t.value()) {
                                thresholds.push(t);
                            }
                        }
                        // Not a p at all (`alpha (8–12 Hz)`): nothing to count.
                        Err(_) => {}
                    }
                }
            }

            // A p-value no unit and no threshold accounts for, unless extraction
            // classified it a threshold. Those are matched by raw text, once each.
            let mut thresholds_here: Vec<&str> = ex
                .statistics
                .iter()
                .filter(|c| c.location == location)
                .filter_map(|c| match &c.stat {
                    Stat::SignificanceThreshold { raw, .. } => Some(raw.as_str()),
                    _ => None,
                })
                .collect();
            for m in p_re.find_iter(s) {
                if covered.iter().chain(threshold_spans.iter()).any(|&(a, b)| m.start() >= a && m.start() < b) {
                    continue;
                }
                if let Some(i) = thresholds_here.iter().position(|t| *t == m.as_str().trim()) {
                    thresholds_here.remove(i);
                    continue;
                }
                not_read.push(NotRead {
                    location: location.clone(),
                    reason: Reason::PWithoutStatistic,
                    span: Some(m.start()..m.end()),
                });
            }
        }
    }

    ResultReading { results, not_read, thresholds }
}

/// `v > 1/5`, exactly. A stated significance level above .2 is not one.
fn exact_above_one_fifth(v: crate::equation::rational::Rational) -> bool {
    v.numer().checked_mul(5).is_some_and(|n| n > v.denom())
}

/// An accepted p-value with no `Stat::PValue` at the same location whose raw
/// contains the same printed p. Carries the printed p, which is manuscript
/// text; not `Serialize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CouplingViolation {
    pub result_id: usize,
    pub location: Location,
    pub printed_p: String,
}

/// The coupling check: one answer to "where are the p-values". Every p this
/// reader accepted must also be a `Stat::PValue` at the same `Location`.
pub fn coupling_violations(reading: &ResultReading, ex: &ExtractionResult) -> Vec<CouplingViolation> {
    reading
        .results
        .iter()
        .filter_map(|r| {
            let PValue::Reported(p) = &r.p else { return None };
            let found = ex.statistics.iter().any(|c| {
                c.location == r.location && matches!(&c.stat, Stat::PValue { raw, .. } if raw.contains(p.raw()))
            });
            (!found).then(|| CouplingViolation {
                result_id: r.id,
                location: r.location.clone(),
                printed_p: p.raw().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equation::rational::Rational;
    use crate::extract::extract_from_text;

    fn doc(body: &str) -> ExtractionResult {
        extract_from_text(&format!("A Title\n\nResults\n{body}\n"))
    }
    fn read(body: &str) -> (ResultReading, ExtractionResult) {
        let ex = doc(body);
        (read_results(&ex, TableStructure::NotAvailable), ex)
    }
    fn one(body: &str) -> ReportedResult {
        let (r, ex) = read(body);
        assert_eq!(r.results.len(), 1, "{body:?}: results {:?}, not read {:?}", r.results, r.not_read);
        assert!(coupling_violations(&r, &ex).is_empty(), "{body:?}: coupling");
        r.results.into_iter().next().unwrap()
    }
    fn reasons(body: &str) -> Vec<Reason> {
        let (r, _) = read(body);
        assert!(r.results.is_empty(), "{body:?} was read: {:?}", r.results);
        r.not_read.into_iter().map(|n| n.reason).collect()
    }
    fn q(s: &str) -> Rational {
        Rational::parse_decimal(s).unwrap().0
    }
    fn p_of(r: &ReportedResult) -> &ReportedNumber {
        match &r.p {
            PValue::Reported(p) => p,
            PValue::NotSignificant => panic!("expected a printed p"),
        }
    }

    // ---- accepted forms ----

    #[test]
    fn a_t_with_a_bound_p() {
        let r = one("The groups differed, t(37) = -4.93, p < .001.");
        assert_eq!(r.family, Family::T);
        assert_eq!(r.raw, "t(37) = -4.93, p < .001");
        // df 37 is counted: the exact point 37.
        assert_eq!(r.df.len(), 1);
        assert_eq!(r.df[0].interval().lo, q("37"));
        assert_eq!(r.df[0].interval().hi, q("37"));
        assert_eq!(r.statistic.value(), q("-4.93"));
        assert_eq!(p_of(&r).relation(), Relation::Lt);
        assert_eq!(p_of(&r).value(), q("0.001"));
        assert_eq!(r.source, Source::Unknown);
    }

    #[test]
    fn an_f_with_mse_between_statistic_and_p() {
        let r = one("Dose mattered, F(2, 70) = 4.48, MSE = 6.61, p < .02.");
        assert_eq!(r.family, Family::F);
        assert_eq!(r.raw, "F(2, 70) = 4.48, MSE = 6.61, p < .02");
        assert_eq!(r.df.iter().map(|d| d.value()).collect::<Vec<_>>(), vec![q("2"), q("70")]);
        assert_eq!(r.statistic.value(), q("4.48"));
        assert_eq!(p_of(&r).value(), q("0.02"));
    }

    #[test]
    fn a_chi_square_with_n_written_with_the_greek_letter_and_as_chi2() {
        for (body, raw) in [
            ("Uptake differed, χ2(1, N = 226) = 6.90, p < .01.", "χ2(1, N = 226) = 6.90, p < .01"),
            ("Uptake differed, chi2(1, N = 226) = 6.90, p < .01.", "chi2(1, N = 226) = 6.90, p < .01"),
        ] {
            let r = one(body);
            assert_eq!(r.family, Family::ChiSquare, "{body}");
            assert_eq!(r.raw, raw);
            assert_eq!(r.df.len(), 1);
            assert_eq!(r.df[0].value(), q("1"));
            // N = 226 is a count: exact.
            let n = r.n.as_ref().expect("n printed in the unit");
            assert_eq!(n.interval().lo, q("226"));
            assert_eq!(n.interval().hi, q("226"));
            assert_eq!(r.statistic.value(), q("6.90"));
        }
    }

    #[test]
    fn an_r_with_df() {
        let r = one("Scores correlated, r(48) = .31, p = .03.");
        assert_eq!(r.family, Family::R);
        assert_eq!(r.statistic.value(), q("0.31"));
        assert_eq!(r.df[0].value(), q("48"));
        assert_eq!(p_of(&r).relation(), Relation::Eq);
    }

    #[test]
    fn a_z_takes_no_df() {
        let r = one("The test gave z = 2.05, p = .04.");
        assert_eq!(r.family, Family::Z);
        assert!(r.df.is_empty());
        assert_eq!(r.raw, "z = 2.05, p = .04");
    }

    #[test]
    fn a_fractional_welch_df_is_measured() {
        let r = one("Variances differed, t(28.4) = 2.10, p = .045.");
        // 28.4 printed with one decimal: [28.35, 28.45], not a point.
        assert_eq!(r.df[0].interval().lo, q("28.35"));
        assert_eq!(r.df[0].interval().hi, q("28.45"));
    }

    // ---- a labelled df (Step 3b) ----

    #[test]
    fn a_chi_square_with_a_labelled_df() {
        let r = one("Uptake differed, χ² = 12.34, df = 2, p < .001.");
        assert_eq!(r.family, Family::ChiSquare);
        assert_eq!(r.raw, "χ² = 12.34, df = 2, p < .001");
        // A labelled integer df is a count, exactly as in the bracket form.
        assert_eq!(r.df.len(), 1);
        assert_eq!(r.df[0].interval().lo, q("2"));
        assert_eq!(r.df[0].interval().hi, q("2"));
        assert!(r.n.is_none());
        assert_eq!(r.statistic.value(), q("12.34"));
        assert_eq!(p_of(&r).value(), q("0.001"));
    }

    #[test]
    fn a_t_with_a_labelled_df() {
        let r = one("Groups differed, t = 2.31, df=58, p = .024.");
        assert_eq!(r.family, Family::T);
        assert_eq!(r.raw, "t = 2.31, df=58, p = .024");
        assert_eq!(r.df[0].interval().lo, q("58"));
        assert_eq!(r.df[0].interval().hi, q("58"));
        // A labelled df with decimals is measured, as a bracketed Welch df is.
        let r = one("Variances differed, t = 2.10, df = 28.4, p = .045.");
        assert_eq!(r.df[0].interval().lo, q("28.35"));
        assert_eq!(r.df[0].interval().hi, q("28.45"));
    }

    #[test]
    fn a_labelled_df_may_be_written_d_f() {
        for (body, raw) in [
            ("Uptake differed, χ2 = 6.90, d.f. = 1, p < .01.", "χ2 = 6.90, d.f. = 1, p < .01"),
            ("Uptake differed, χ2 = 6.90, d. f. = 1, p < .01.", "χ2 = 6.90, d. f. = 1, p < .01"),
        ] {
            let r = one(body);
            assert_eq!(r.raw, raw);
            assert_eq!(r.df[0].value(), q("1"));
        }
    }

    #[test]
    fn a_labelled_df_may_be_uppercase_and_be_followed_by_ns() {
        let r = one("Uptake differed, χ² = 12.34, DF = 2, p < .001.");
        assert_eq!(r.raw, "χ² = 12.34, DF = 2, p < .001");
        assert_eq!(r.df[0].value(), q("2"));
        let r = one("No difference, t = 1.20, df = 58, ns.");
        assert_eq!(r.p, PValue::NotSignificant);
        assert_eq!(r.raw, "t = 1.20, df = 58, ns");
    }

    #[test]
    fn an_f_with_a_labelled_df_pair_is_malformed() {
        assert_eq!(reasons("Dose mattered, F = 4.2, df = 2, 57, p = .02."), vec![Reason::MalformedDegreesOfFreedom]);
        // Not only the pair: the labelled form is t and chi-square only.
        assert_eq!(reasons("Dose mattered, F = 4.2, df = 2, p = .02."), vec![Reason::MalformedDegreesOfFreedom]);
        assert_eq!(reasons("Scores correlated, r = .31, df = 48, p = .03."), vec![Reason::MalformedDegreesOfFreedom]);
    }

    #[test]
    fn a_labelled_df_beside_a_bracketed_df_is_malformed() {
        assert_eq!(reasons("Groups differed, t(58) = 2.31, df = 58, p = .024."), vec![Reason::MalformedDegreesOfFreedom]);
    }

    #[test]
    fn a_labelled_df_after_the_p_does_not_complete_a_unit() {
        assert_eq!(reasons("Uptake differed, χ² = 12.34, p < .001, df = 2."), vec![Reason::MissingDegreesOfFreedom]);
        assert_eq!(reasons("Groups differed, t = 2.31, p = .024, df = 58."), vec![Reason::MissingDegreesOfFreedom]);
    }

    #[test]
    fn a_chi_square_with_neither_df_form_is_missing_its_df() {
        assert_eq!(reasons("Uptake differed, χ² = 12.34, p < .001."), vec![Reason::MissingDegreesOfFreedom]);
    }

    #[test]
    fn only_the_p_may_follow_a_labelled_df() {
        // The order is value, df, p. An item between the df and the p breaks it.
        assert_eq!(reasons("Groups differed, t = 2.31, df = 58, d = 0.60, p = .024."), vec![Reason::NotLinked]);
    }

    #[test]
    fn a_word_ending_in_df_is_not_a_labelled_df() {
        // The label is matched only where the item after the value starts. An
        // unanchored search finds the `DF =` inside `PDF =` and reads df = 2.
        for body in ["Groups differed, t = 2.31, PDF = 2, p = .024.", "Uptake differed, χ² = 12.34, PDF = 2, p < .001."] {
            let (r, _) = read(body);
            // No result, so no df was taken from `PDF = 2`.
            assert!(r.results.is_empty(), "{body:?} was read: {:?}", r.results);
            let why: Vec<Reason> = r.not_read.iter().map(|n| n.reason).collect();
            assert_eq!(why, vec![Reason::NotLinked], "{body:?}");
        }
    }

    #[test]
    fn ns_is_a_p_value() {
        let r = one("No difference, t(58) = 1.20, ns.");
        assert_eq!(r.p, PValue::NotSignificant);
        assert_eq!(r.raw, "t(58) = 1.20, ns");
        let r = one("No difference, t(58) = 1.20, n.s.");
        assert_eq!(r.p, PValue::NotSignificant);
    }

    #[test]
    fn an_effect_size_and_interval_inside_the_unit_are_kept() {
        let r = one("It worked, t(30) = 2.50, d = 0.45, 95% CI [0.10, 0.80], p = .018.");
        let e = r.effect_size.as_ref().expect("effect size");
        assert_eq!(e.name, "d");
        assert_eq!(e.value.value(), q("0.45"));
        let ci = r.ci.as_ref().expect("interval");
        assert_eq!(ci.level.value(), q("95"));
        assert_eq!(ci.low.value(), q("0.10"));
        assert_eq!(ci.high.value(), q("0.80"));
    }

    #[test]
    fn an_interval_records_the_item_it_follows() {
        for (body, follows) in [
            ("It worked, t(30) = 2.50, d = 0.45, 95% CI [0.10, 0.80], p = .018.", CiFollows::EffectSize),
            ("It worked, t(30) = 2.50, 95% CI [0.10, 0.80], d = 0.45, p = .018.", CiFollows::Statistic),
            ("It worked, F(1, 30) = 6.25, MSE = 1.20, 95% CI [0.10, 0.80], p = .018.", CiFollows::Mse),
            ("It worked, t(30) = 2.50, p = .018, 95% CI [0.10, 0.80].", CiFollows::PValue),
            ("It worked, t(30) = 2.50, p = .018, d = 0.45, 95% CI [0.10, 0.80].", CiFollows::EffectSize),
            // An effect size before the p and the interval after it: the p is between.
            ("It worked, t(30) = 2.50, d = 0.45, p = .018, 95% CI [0.10, 0.80].", CiFollows::PValue),
        ] {
            let r = one(body);
            assert_eq!(r.ci.as_ref().expect("interval").follows, follows, "{body}");
        }
    }

    #[test]
    fn the_sentence_saying_partial_or_controlling_is_recorded() {
        for (body, partial) in [
            ("Scores correlated, r(48) = .31, p = .03.", false),
            ("The partial correlation held, r(47) = .31, p = .03.", true),
            ("A semi-partial correlation held, r(47) = .31, p = .03.", true),
            ("Controlling for age, r(47) = .31, p = .03.", true),
            ("After controlling for age, scores correlated, r(47) = .31, p = .03.", true),
            // Another sentence of the same paragraph does not count.
            ("We report partial correlations below. Scores correlated, r(48) = .31, p = .03.", false),
            // Not inside a longer word.
            ("Partiality was low, r(48) = .31, p = .03.", false),
        ] {
            assert_eq!(one(body).sentence_says_partial, partial, "{body}");
        }
    }

    #[test]
    fn a_unit_is_never_split_inside_a_numeric_literal() {
        // "p = .024." ends a sentence; the full stop is punctuation, not a digit.
        let r = one("The effect held, t(58) = 2.31, p = .024. A second sentence follows.");
        assert_eq!(p_of(&r).raw(), ".024");
        assert_eq!(p_of(&r).decimals(), 3);
        assert_eq!(r.raw, "t(58) = 2.31, p = .024");
    }

    #[test]
    fn a_parenthetical_unit_is_read() {
        let r = one("Recall improved (t(30) = 2.10, p = .044) after sleep.");
        assert_eq!(r.raw, "t(30) = 2.10, p = .044");
    }

    #[test]
    fn a_statistic_letter_inside_a_word_is_not_a_statistic() {
        // Without the word-boundary guard, "Sheet(30)" reads as t(30).
        let (r, _) = read("Sheet(30) = 2.10, p = .04, and Prior(12) = .5, p = .02.");
        assert!(r.results.is_empty(), "{:?}", r.results);
    }

    // ---- rejections, one reason each ----

    #[test]
    fn a_statistic_with_no_p_is_not_read() {
        assert_eq!(reasons("The difference was t(30) = 2.10 overall."), vec![Reason::StatisticWithoutP]);
    }

    #[test]
    fn a_p_with_no_statistic_is_not_read() {
        assert_eq!(reasons("The effect was reliable (p = .03)."), vec![Reason::PWithoutStatistic]);
    }

    #[test]
    fn two_results_folded_into_one_clause_are_not_read() {
        let r = reasons("Both effects held, F(1, 15) = 19.9 and 5.16, p < .01.");
        assert_eq!(r, vec![Reason::TwoResultsInOneClause]);
    }

    #[test]
    fn a_number_reported_number_refuses_rejects_the_unit() {
        assert_eq!(
            reasons("A large effect, t(30) = 1,204, p = .04."),
            vec![Reason::Number(NumberNotRead::Comma)]
        );
    }

    #[test]
    fn a_statistic_and_p_joined_only_by_proximity_are_not_linked() {
        let r = reasons("A difference, t(30) = 2.10, which was large, p = .04.");
        assert_eq!(r, vec![Reason::NotLinked]);
    }

    #[test]
    fn a_statistic_without_its_df_is_not_read() {
        assert_eq!(reasons("A difference, t = 2.10, p = .04."), vec![Reason::MissingDegreesOfFreedom]);
    }

    #[test]
    fn a_df_of_the_wrong_shape_is_not_read() {
        assert_eq!(reasons("A difference, t(1, 30) = 2.10, p = .04."), vec![Reason::MalformedDegreesOfFreedom]);
    }

    #[test]
    fn a_second_effect_size_in_one_unit_is_not_read() {
        let r = reasons("A difference, t(30) = 2.10, d = 0.40, d = 0.50, p = .04.");
        assert_eq!(r, vec![Reason::RepeatedItem]);
    }

    #[test]
    fn a_t_and_an_unrelated_p_in_different_sentences_yield_nothing() {
        let r = reasons("Groups differed, t(30) = 2.10. In another analysis the effect held (p = .04).");
        assert_eq!(r, vec![Reason::StatisticWithoutP, Reason::PWithoutStatistic]);
    }

    #[test]
    fn a_unit_split_across_two_paragraphs_is_not_read() {
        // DOCX table cells arrive as paragraphs; a unit split across cells is two
        // paragraphs, and is not a complete unit.
        let r = reasons("t(30) = 2.10,\n\np = .04");
        assert_eq!(r, vec![Reason::StatisticWithoutP, Reason::PWithoutStatistic]);
    }

    // ---- items after the p ----

    #[test]
    fn an_effect_size_and_interval_after_the_p_belong_to_the_unit() {
        let r = one("Groups differed, F(3, 157) = 7.92, p < .001, η2 = .245, 95% CI [0.10, 0.38]. Next.");
        assert_eq!(r.raw, "F(3, 157) = 7.92, p < .001, η2 = .245, 95% CI [0.10, 0.38]");
        let e = r.effect_size.as_ref().expect("effect size after p");
        assert_eq!(e.name, "η2");
        assert_eq!(e.value.value(), q("0.245"));
        assert_eq!(r.ci.as_ref().expect("interval after p").high.value(), q("0.38"));
    }

    #[test]
    fn after_the_p_anything_else_just_ends_the_unit() {
        let r = one("Groups differed, t(30) = 2.10, p = .04, which was expected.");
        assert_eq!(r.raw, "t(30) = 2.10, p = .04");
        assert!(r.effect_size.is_none());
    }

    #[test]
    fn one_effect_size_in_all_counting_both_sides_of_the_p() {
        let r = reasons("A difference, t(30) = 2.10, d = 0.40, p = .04, d = 0.50.");
        assert_eq!(r, vec![Reason::RepeatedItem]);
    }

    // ---- each attempt counted once ----

    #[test]
    fn a_p_claimed_by_a_rejected_attempt_is_not_counted_again() {
        // A REJECTED attempt (the unit breaks before its p) still claims the p
        // later in its sentence: one entry, not NotLinked plus PWithoutStatistic.
        let (r, ex) = read("A difference, t(30) = 2.10, which was large, p = .04.");
        let why: Vec<Reason> = r.not_read.iter().map(|n| n.reason).collect();
        assert_eq!(why, vec![Reason::NotLinked]);
        assert!(r.results.is_empty());
        // The span runs from the statistic through the p it reached.
        let para = crate::extract::paragraph_at(&ex, &r.not_read[0].location).unwrap();
        let span = r.not_read[0].span.clone().expect("span");
        assert_eq!(&para[span], "t(30) = 2.10, which was large, p = .04");
    }

    #[test]
    fn a_p_after_another_statistic_belongs_to_that_statistic() {
        let (r, _) = read("Overall t(30) = 2.10 held, and F(1, 20) = 3.00, p = .04.");
        assert_eq!(r.results.len(), 1, "{:?}", r.not_read);
        assert_eq!(r.results[0].family, Family::F);
        let why: Vec<Reason> = r.not_read.iter().map(|n| n.reason).collect();
        assert_eq!(why, vec![Reason::StatisticWithoutP]);
    }

    // ---- source ----

    #[test]
    fn a_docx_paragraph_outside_every_table_is_prose() {
        let mut ex = doc("t(30) = 2.10, p = .04");
        ex.doc_tables = vec![crate::extract::docparse::DocTable { rows: vec![vec!["Group".into(), "Mean".into()]] }];
        let r = read_results(&ex, TableStructure::Declared);
        assert_eq!(r.results[0].source, Source::Prose);
    }

    #[test]
    fn a_pdf_paragraph_is_never_labelled_prose() {
        let ex = doc("t(30) = 2.10, p = .04");
        let r = read_results(&ex, TableStructure::NotAvailable);
        assert_eq!(r.results[0].source, Source::Unknown);
    }

    // ---- source (declared table cell) ----

    #[test]
    fn a_unit_inside_a_declared_table_cell_is_a_table_cell() {
        let mut ex = doc("t(30) = 2.10, p = .04");
        ex.doc_tables = vec![crate::extract::docparse::DocTable {
            rows: vec![vec!["Group".into(), "t(30) = 2.10, p = .04".into()]],
        }];
        let r = read_results(&ex, TableStructure::Declared);
        assert_eq!(r.results.len(), 1);
        assert_eq!(r.results[0].source, Source::TableCell);
    }

    // ---- thresholds ----

    #[test]
    fn stated_thresholds_are_collected_once_each_and_are_not_unread_ps() {
        let (r, _) = read(
            "The significance level was p < .05.\n\nSignificance was set at p < 0.05 for main effects.\n\n\
             The alpha level was p < .01 for follow-ups.",
        );
        let vals: Vec<Rational> = r.thresholds.iter().map(|t| t.value()).collect();
        assert_eq!(vals, vec![q("0.05"), q("0.01")]);
        assert!(r.not_read.iter().all(|n| n.reason != Reason::PWithoutStatistic), "{:?}", r.not_read);
    }

    fn thresholds_of(body: &str) -> Vec<Rational> {
        read(body).0.thresholds.iter().map(|t| t.value()).collect()
    }

    /// Evaluation finding 4: a result reported in the same sentence as a
    /// threshold phrase used to donate its own p to the paper's alpha.
    #[test]
    fn a_threshold_phrase_in_a_result_sentence_does_not_take_the_result_p() {
        let (r, _) = read("The significance level was set at p < .05, and the groups differed, t(28) = 2.10, p = .045.");
        assert_eq!(r.results.len(), 1, "{:?}", r.not_read);
        assert_eq!(r.thresholds.iter().map(|t| t.value()).collect::<Vec<_>>(), vec![q("0.05")]);
        assert!(r.not_read.is_empty(), "{:?}", r.not_read);
    }

    #[test]
    fn a_sentence_with_no_alpha_wording_states_no_threshold() {
        assert_eq!(thresholds_of("Scores rose, t(28) = 2.10, p = .045."), vec![]);
        // A lone p is not a threshold either; it is an unread p.
        let (r, _) = read("The effect was reliable (p = .03).");
        assert!(r.thresholds.is_empty());
        assert_eq!(r.not_read.iter().map(|n| n.reason).collect::<Vec<_>>(), vec![Reason::PWithoutStatistic]);
    }

    /// The closed list of alpha wordings, one sentence each.
    #[test]
    fn every_alpha_wording_is_read() {
        for body in [
            "Tests used α = .05.",
            "We used an alpha of .05.",
            "The alpha level was .05.",
            "Alpha was set at .05 for all tests.",
            "We adopted a significance level of .05.",
            "The significance threshold was .05.",
            "A criterion of .05 was used.",
            "p < .05 was considered significant.",
            "P-values < .05 were regarded as statistically significant.",
            "Significance was set at p < .05.",
            "Results were considered significant at p < .05.",
            "The level of significance was p \u{2264} .05.",
            "Statistical significance was defined as p < 0.05.",
        ] {
            assert_eq!(thresholds_of(body), vec![q("0.05")], "{body:?}");
            let (r, _) = read(body);
            assert!(r.not_read.is_empty(), "{body:?}: {:?}", r.not_read);
        }
    }

    /// Several thresholds in one statement.
    #[test]
    fn a_statement_with_several_thresholds_reads_each() {
        assert_eq!(
            thresholds_of("Significance was set at p < .05, and at p < .01 for the secondary outcomes."),
            vec![q("0.05"), q("0.01")]
        );
        assert_eq!(thresholds_of("We used an alpha of .05 or .01 for follow-ups."), vec![q("0.05"), q("0.01")]);
        let (r, _) = read("Significance was set at p < .05, and at p < .01 for the secondary outcomes.");
        assert!(r.not_read.is_empty(), "{:?}", r.not_read);
    }

    #[test]
    fn a_fractional_threshold_is_the_exact_rational() {
        assert_eq!(thresholds_of("Significance was set at p < .05/6."), vec![Rational::new(1, 120).unwrap()]);
        assert_eq!(thresholds_of("An alpha of 0.05/3 was used."), vec![Rational::new(1, 60).unwrap()]);
        let (r, _) = read("Significance was set at p < .05/6.");
        assert_eq!(r.thresholds[0].raw(), ".05/6");
        assert!(r.not_read.is_empty(), "{:?}", r.not_read);
    }

    /// "alpha" is also an EEG band and a power, and "criterion" a training
    /// target. None of these is a threshold, and none is an unread number
    /// either: a candidate the wording does not take is simply not a p.
    /// (Measured on the DEV corpus: a first draft turned "alpha (8–12 Hz)" into
    /// fifteen `PValueOutOfRange` rows in one EEG paper.)
    #[test]
    fn alpha_in_another_sense_is_neither_a_threshold_nor_an_unread_number() {
        for body in [
            "Power in the alpha (8\u{2013}12 Hz) band fell during the task.",
            "Mean alpha power = 0.12 over frontal sensors.",
            "Training continued until criterion was met (>35 rewards).",
            "The alpha values of 0.954 for attitude and 0.944 for knowledge were acceptable.",
        ] {
            let (r, _) = read(body);
            assert!(r.thresholds.is_empty(), "{body:?}: {:?}", r.thresholds.iter().map(|t| t.raw()).collect::<Vec<_>>());
            assert!(r.not_read.is_empty(), "{body:?}: {:?}", r.not_read);
        }
    }

    /// Cronbach's alpha is a reliability coefficient, not a significance level.
    #[test]
    fn a_reliability_alpha_is_not_a_threshold() {
        for body in ["Cronbach's alpha = .85 for the scale.", "The scale's alpha was .85.", "Cronbach's α = .05 was poor."] {
            assert_eq!(thresholds_of(body), vec![], "{body:?}");
        }
    }

    // ---- coupling ----

    #[test]
    fn every_accepted_p_is_a_stat_pvalue_at_the_same_location() {
        let bodies = [
            "t(37) = -4.93, p < .001.",
            "F(2, 70) = 4.48, MSE = 6.61, p < .02.",
            "χ2(1, N = 226) = 6.90, p < .01.",
            "r(48) = .31, p = .03.",
            "z = 2.05, p = .04.",
            "t(28.4) = 2.10, p = .045.",
            "t(58) = 2.31, p = .024. Next.",
        ];
        let mut checked = 0;
        for b in bodies {
            let (r, ex) = read(b);
            assert_eq!(r.results.len(), 1, "{b}");
            assert!(coupling_violations(&r, &ex).is_empty(), "{b}");
            checked += 1;
        }
        assert_eq!(checked, bodies.len());
    }

    #[test]
    fn the_coupling_check_fires_when_extraction_has_no_such_p() {
        let ex = doc("t(30) = 2.10, p = .04.");
        let r = read_results(&ex, TableStructure::NotAvailable);
        assert_eq!(r.results.len(), 1);
        // Synthetic: the same document with extraction's PValue claims removed.
        let mut stripped = ex.clone();
        stripped.statistics.retain(|c| !matches!(c.stat, Stat::PValue { .. }));
        let v = coupling_violations(&r, &stripped);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].printed_p, ".04");
        assert_eq!(v[0].result_id, 0);
    }
}
