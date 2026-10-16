//! **Every response is validated, even with strict mode on, and a valid one
//! is only a suggestion or text.**
//!
//! [`read_response`] takes the Responses API body and returns the one
//! `output_text` part, its `url_citation` annotations, and the usage counts,
//! or a closed [`Rejection`] (malformed JSON, refusal, incomplete, empty).
//! The three structured tasks then parse that text into their output type
//! with unknown keys denied, and check it against what was sent.
//!
//! What comes out is [`ClaimMethodSuggestion`], [`BindingSuggestion`] or an
//! explanation as [`ModelText`]. None of them is, holds or can become a
//! confirmed binding, a token, a verdict, a variant decision or a computed
//! value: a suggested variant is a name, not a `Variant`, and a suggested
//! binding is aliases and counts, not a `BindingDraft`. Turning a suggestion
//! into a proposal is the run state's job, and confirming it the user's.

use serde::Deserialize;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use super::outbound::{role_from_name, BindingSuggestionOutbound, ClaimMethodOutbound, Outbound, ResultExplanationOutbound};
use super::spec::variant_names;
use crate::stats_l1::catalogue::{L1MethodId, LevelRule, MethodCatalogue, RoleName, SlotKind};
use crate::stats_l0::negation::{negation_of, Negation};
use crate::stats_l1::dataset::ColumnAlias;

/// The longest reason a suggestion may give, in characters.
pub const MAX_REASON_CHARS: usize = 300;
/// The longest explanation, in characters.
pub const MAX_EXPLANATION_CHARS: usize = 900;

/// Text written by a model. Shown as plain text, marked as model text, never
/// rendered as HTML or markdown, never acted on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelText(String);

impl ModelText {
    pub(crate) fn new(s: &str) -> ModelText {
        ModelText(s.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a response was not used. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    MalformedJson,
    /// The response was cut short (status not `completed`).
    Incomplete,
    Refused,
    Empty,
    /// More than one text part: indexes are per part, so it is not read.
    SeveralTextParts,
    /// The task's JSON did not have exactly the schema's keys.
    WrongShape,
    UnknownMethod,
    VariantNotInMethod,
    UnknownRole,
    MissingRole,
    RoleRepeated,
    UnknownAlias,
    AliasInTwoRoles,
    /// A level count on a role that is not a group, or one the role's rule
    /// cannot accept.
    LevelCountInvalid,
    /// A relied-on field that was not sent.
    ReliedOnUnsent,
    TextTooLong { chars: usize, max: usize },
    /// The explanation states a number that is not in its input.
    NumberNotInInput,
    /// The explanation states a verdict other than the engine's.
    WrongVerdict,
    /// A digit outside ASCII 0-9: another script's digit, a full-width
    /// digit, a superscript or a vulgar fraction.
    NonAsciiDigit,
    /// A number written in words (three and up, "point" between number
    /// words, half, third, quarter, percent).
    NumberWord,
    /// "positive" or "negative" beside the statistic, against its sign.
    SignDisagrees,
    /// A word about misconduct: no verdict says the paper is wrong.
    MisconductWord,
}

/// A raw `url_citation` annotation, untrusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCitation {
    pub url: Option<String>,
    pub title: Option<String>,
    pub start_index: Option<i64>,
    pub end_index: Option<i64>,
}

/// Usage counts, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// The model's answer, read from the envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOutput {
    pub text: String,
    pub citations: Vec<RawCitation>,
    pub usage: Usage,
}

/// Usage counts alone, for a diagnostic, from any body (zero on anything
/// unreadable).
pub fn read_usage(body: &[u8]) -> Usage {
    let Ok(v) = serde_json::from_slice::<Value>(body) else { return Usage::default() };
    Usage { input_tokens: v["usage"]["input_tokens"].as_u64(), output_tokens: v["usage"]["output_tokens"].as_u64() }
}

/// Read the Responses API envelope. Other output items (the search call,
/// reasoning) are ignored; a refusal anywhere rejects.
pub fn read_response(body: &[u8]) -> Result<ModelOutput, Rejection> {
    let v: Value = serde_json::from_slice(body).map_err(|_| Rejection::MalformedJson)?;
    let Some(obj) = v.as_object() else { return Err(Rejection::MalformedJson) };
    match obj.get("status").and_then(Value::as_str) {
        None | Some("completed") => {}
        Some(_) => return Err(Rejection::Incomplete),
    }
    let Some(output) = obj.get("output").and_then(Value::as_array) else { return Err(Rejection::Empty) };
    let mut texts: Vec<&Value> = Vec::new();
    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        for part in item.get("content").and_then(Value::as_array).into_iter().flatten() {
            match part.get("type").and_then(Value::as_str) {
                Some("refusal") => return Err(Rejection::Refused),
                Some("output_text") => texts.push(part),
                _ => {}
            }
        }
    }
    let part = match texts[..] {
        [] => return Err(Rejection::Empty),
        [p] => p,
        _ => return Err(Rejection::SeveralTextParts),
    };
    let text = part.get("text").and_then(Value::as_str).ok_or(Rejection::MalformedJson)?;
    if text.trim().is_empty() {
        return Err(Rejection::Empty);
    }
    let citations = part
        .get("annotations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|a| a.get("type").and_then(Value::as_str) == Some("url_citation"))
        .map(|a| RawCitation {
            url: a.get("url").and_then(Value::as_str).map(str::to_string),
            title: a.get("title").and_then(Value::as_str).map(str::to_string),
            start_index: a.get("start_index").and_then(Value::as_i64),
            end_index: a.get("end_index").and_then(Value::as_i64),
        })
        .collect();
    Ok(ModelOutput { text: text.to_string(), citations, usage: read_usage(body) })
}

fn bounded(text: &str, max: usize) -> Result<ModelText, Rejection> {
    let t = text.trim();
    if t.is_empty() {
        return Err(Rejection::Empty);
    }
    let chars = t.chars().count();
    if chars > max {
        return Err(Rejection::TextTooLong { chars, max });
    }
    Ok(ModelText::new(t))
}

fn relied_on(names: &[String], sent: &Outbound) -> Result<Vec<&'static str>, Rejection> {
    let allowed = sent.sent_fields();
    names.iter().map(|n| allowed.iter().find(|a| **a == n.as_str()).copied().ok_or(Rejection::ReliedOnUnsent)).collect()
}

/* ------------------------------------------------------------------------ *
 *  claim_method
 * ------------------------------------------------------------------------ */

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimMethodWire {
    method_id: String,
    variant: String,
    reason: String,
    relied_on: Vec<String>,
}

/// A suggestion, for the user to accept, edit or ignore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimMethodSuggestion {
    /// `None` when the model said `none`.
    pub method: Option<L1MethodId>,
    /// A hint only: a name from the method's list, or `None` for not stated.
    /// Whether a variant is STATED is decided by Step 2's word list and the
    /// user's tick, never by this.
    pub variant_hint: Option<&'static str>,
    pub reason: ModelText,
    pub relied_on: Vec<&'static str>,
}

pub fn validate_claim_method(out: &ModelOutput, sent: &ClaimMethodOutbound) -> Result<ClaimMethodSuggestion, Rejection> {
    let w: ClaimMethodWire = serde_json::from_str(&out.text).map_err(|_| Rejection::WrongShape)?;
    let method = match w.method_id.as_str() {
        "none" => None,
        id => Some(L1MethodId::from_id(id).ok_or(Rejection::UnknownMethod)?),
    };
    let variant_hint = match (method, w.variant.as_str()) {
        (_, "not_stated") => None,
        (None, _) => return Err(Rejection::VariantNotInMethod),
        (Some(m), v) => {
            let names = variant_names(MethodCatalogue::entry(m).variant_kind);
            Some(*names.iter().find(|n| **n == v).ok_or(Rejection::VariantNotInMethod)?)
        }
    };
    let reason = bounded(&w.reason, MAX_REASON_CHARS)?;
    let relied_on = relied_on(&w.relied_on, &Outbound::ClaimMethod(sent.clone()))?;
    Ok(ClaimMethodSuggestion { method, variant_hint, reason, relied_on })
}

/* ------------------------------------------------------------------------ *
 *  binding_suggestion
 * ------------------------------------------------------------------------ */

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleWire {
    role: String,
    alias: String,
    levels_to_select: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingWire {
    roles: Vec<RoleWire>,
    reason: String,
    relied_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedRole {
    pub role: RoleName,
    /// `None` for `cannot_tell`.
    pub column: Option<ColumnAlias>,
    /// For a group role: how many levels to select. Never which.
    pub levels_to_select: Option<u32>,
}

/// Aliases and counts for the user to review. Not a draft, not a binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingSuggestion {
    pub method: L1MethodId,
    pub roles: Vec<SuggestedRole>,
    pub reason: ModelText,
    pub relied_on: Vec<&'static str>,
}

fn parse_alias(s: &str) -> Option<ColumnAlias> {
    let digits = s.strip_prefix('C')?;
    if digits.is_empty() || digits.len() > 3 || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(ColumnAlias)
}

pub fn validate_binding_suggestion(out: &ModelOutput, sent: &BindingSuggestionOutbound) -> Result<BindingSuggestion, Rejection> {
    let w: BindingWire = serde_json::from_str(&out.text).map_err(|_| Rejection::WrongShape)?;
    let method = L1MethodId::from_id(sent.method_id).ok_or(Rejection::UnknownMethod)?;
    let entry = MethodCatalogue::entry(method);
    let mut roles: Vec<SuggestedRole> = Vec::new();
    for r in &w.roles {
        let role = role_from_name(&r.role).ok_or(Rejection::UnknownRole)?;
        let slot = entry.slot(role).ok_or(Rejection::UnknownRole)?;
        let column = match r.alias.as_str() {
            "cannot_tell" => None,
            a => {
                let alias = parse_alias(a).ok_or(Rejection::UnknownAlias)?;
                let in_schema = sent.schema.columns.iter().find(|c| c.alias == a).ok_or(Rejection::UnknownAlias)?;
                if let (SlotKind::Group { levels }, Some(n)) = (slot.kind, r.levels_to_select) {
                    let bucket_ok = match in_schema.levels {
                        Some("two") => n == 2,
                        Some("three_to_ten") => (2..=10).contains(&n),
                        Some(_) => n >= 2,
                        None => false,
                    };
                    if !bucket_ok {
                        return Err(Rejection::LevelCountInvalid);
                    }
                    let _ = levels; // the rule itself is checked below
                }
                Some(alias)
            }
        };
        let levels_to_select = match (slot.kind, r.levels_to_select) {
            (SlotKind::Group { levels }, Some(n)) => {
                let n = u32::try_from(n).map_err(|_| Rejection::LevelCountInvalid)?;
                let ok = match levels {
                    LevelRule::Exactly(k) => n as usize == k,
                    LevelRule::AtLeast(k) => n as usize >= k,
                };
                if !ok {
                    return Err(Rejection::LevelCountInvalid);
                }
                Some(n)
            }
            (SlotKind::Group { .. }, None) => None,
            (_, Some(_)) => return Err(Rejection::LevelCountInvalid),
            (_, None) => None,
        };
        let repeatable = matches!(slot.kind, SlotKind::NumericList { .. });
        if !repeatable && roles.iter().any(|x| x.role == role) {
            return Err(Rejection::RoleRepeated);
        }
        if let Some(c) = column {
            if roles.iter().any(|x| x.column == Some(c)) {
                return Err(Rejection::AliasInTwoRoles);
            }
        }
        roles.push(SuggestedRole { role, column, levels_to_select });
    }
    if entry.slots.iter().any(|s| !roles.iter().any(|r| r.role == s.role)) {
        return Err(Rejection::MissingRole);
    }
    let reason = bounded(&w.reason, MAX_REASON_CHARS)?;
    let relied_on = relied_on(&w.relied_on, &Outbound::BindingSuggestion(sent.clone()))?;
    Ok(BindingSuggestion { method, roles, reason, relied_on })
}

/* ------------------------------------------------------------------------ *
 *  result_explanation
 * ------------------------------------------------------------------------ */

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplanationWire {
    explanation: String,
}

/// A number as written: its value and how many decimals it printed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrittenNumber {
    pub value: f64,
    pub decimals: u32,
}

/// Every number in `text`: digits with an optional decimal part (`.05`,
/// `0.05`, `1,234`, `57`). A digit run glued to a letter (`C3`, `x2`) still
/// counts, so a number cannot be slipped past the check inside a word. Signs
/// are not read here (see [`scan_numbers`]).
pub fn numbers_in(text: &str) -> Vec<WrittenNumber> {
    scan_numbers(text).into_iter().map(|n| n.written).collect()
}

/// A number with where it sits and whether a minus sign leads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Located {
    written: WrittenNumber,
    start: usize,
    end: usize,
    negative: bool,
}

fn scan_numbers(text: &str) -> Vec<Located> {
    let b: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i].1;
        let starts = c.is_ascii_digit() || (c == '.' && b.get(i + 1).is_some_and(|x| x.1.is_ascii_digit()));
        if !starts {
            i += 1;
            continue;
        }
        let mut s = String::new();
        let mut j = i;
        while j < b.len() {
            let c = b[j].1;
            if c.is_ascii_digit() {
                s.push(c);
            } else if c == ','
                && b.get(j + 1..j + 4).is_some_and(|w| w.iter().all(|x| x.1.is_ascii_digit()))
                && !b.get(j + 4).is_some_and(|x| x.1.is_ascii_digit())
                && !s.contains('.')
            {
                // A thousands separator.
            } else if c == '.' && !s.contains('.') && b.get(j + 1).is_some_and(|x| x.1.is_ascii_digit()) {
                s.push('.');
            } else {
                break;
            }
            j += 1;
        }
        let decimals = s.split_once('.').map_or(0, |(_, d)| d.len() as u32);
        let norm = if s.starts_with('.') { format!("0{s}") } else { s };
        // A minus: `-` or `−` right before the number, itself not after a
        // letter or digit (so `t-test`, `1-tailed` and `4.43-4.45` carry none).
        let negative = i > 0
            && matches!(b[i - 1].1, '-' | '\u{2212}')
            && (i < 2 || !b[i - 2].1.is_alphanumeric());
        if let Ok(value) = norm.parse::<f64>() {
            let start = b[i].0;
            let end = b.get(j).map_or(text.len(), |x| x.0);
            out.push(Located { written: WrittenNumber { value, decimals }, start, end, negative });
        }
        i = j.max(i + 1);
    }
    out
}

/// The always-allowed numbers, for a number attached to no quantity.
const ALWAYS_ALLOWED: [f64; 5] = [0.0, 1.0, 2.0, 100.0, 95.0];

/// Number words that may not appear: an explanation writes digits, so a
/// number in words could state a value the check cannot see.
const NUMBER_WORDS: &[&str] = &[
    "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen",
    "sixteen", "seventeen", "eighteen", "nineteen", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    "hundred", "hundreds", "thousand", "thousands", "million", "millions", "billion", "half", "halves", "third", "thirds",
    "quarter", "quarters", "percent", "percentage",
];
/// Allowed alone; never on either side of "point".
const SMALL_NUMBER_WORDS: &[&str] = &["zero", "one", "two"];

/// Words no explanation may use: no verdict says the paper is wrong.
const MISCONDUCT_STEMS: &[&str] = &[
    "fraud", "fabricat", "falsif", "manipulat", "misconduct", "phack", "cheat", "dishonest", "dredg", "plagiar", "deceiv", "decept",
];

/// The quantity a number can be attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quantity {
    Statistic,
    Df,
    P,
    N,
    Alpha,
}

fn quantity_of(word: &str) -> Option<Quantity> {
    Some(match word {
        "t" | "f" | "r" | "rho" | "ρ" | "chi" | "χ" | "statistic" | "statistics" | "correlation" | "coefficient" => Quantity::Statistic,
        "df" | "degrees" | "freedom" => Quantity::Df,
        "p" | "pvalue" => Quantity::P,
        "n" | "rows" | "row" | "pairs" | "pair" | "participants" | "observations" | "cases" | "subjects" | "sample" => Quantity::N,
        "alpha" | "α" | "threshold" | "significance" => Quantity::Alpha,
        _ => return None,
    })
}

/// One input value of a quantity: its value, its sign, and the fewest
/// decimals a written number must show to state it (the printed number's).
#[derive(Debug, Clone, Copy)]
struct InputValue {
    value: f64,
    min_decimals: u32,
}

/// The decimals a printed number shows (`.004` → 3, `17` → 0).
fn printed_decimals(s: &str) -> u32 {
    numbers_in(s).first().map_or(0, |w| w.decimals)
}

/// Every input value, by quantity.
fn input_values(sent: &ResultExplanationOutbound) -> Vec<(Quantity, InputValue)> {
    let mut out = Vec::new();
    for n in &sent.numbers {
        let q = match n.kind.code {
            "statistic" => Quantity::Statistic,
            "df" => Quantity::Df,
            "p" => Quantity::P,
            "n" => Quantity::N,
            _ => continue,
        };
        let d = printed_decimals(&n.printed);
        if let Some(w) = numbers_in(&n.printed).first() {
            out.push((q, InputValue { value: w.value, min_decimals: d }));
        }
        out.push((q, InputValue { value: n.computed, min_decimals: d }));
    }
    if let Some(n) = sent.n_used {
        out.push((Quantity::N, InputValue { value: n as f64, min_decimals: 0 }));
    }
    for e in &sent.exclusions {
        out.push((Quantity::N, InputValue { value: e.rows as f64, min_decimals: 0 }));
    }
    for a in &sent.stated_alpha {
        for w in numbers_in(a) {
            out.push((Quantity::Alpha, InputValue { value: w.value, min_decimals: w.decimals }));
        }
    }
    out
}

/// A written number states `v` when `v`, rounded to the written decimals, is
/// the written value, the sign agrees, and enough decimals were written.
fn states(w: &Located, v: &InputValue) -> bool {
    if w.negative && v.value >= 0.0 {
        return false;
    }
    w.written.decimals >= v.min_decimals && matches_at_precision(w.written, v.value)
}

/// A written number matches an allowed one when the allowed value, at the
/// written precision, is the written value.
fn matches_at_precision(w: WrittenNumber, allowed: f64) -> bool {
    let half = 0.5 * 10f64.powi(-(w.decimals as i32));
    (allowed.abs() - w.value).abs() <= half + 1e-9 * w.value.max(1.0)
}

/// A word token: lower-cased, with its byte range. Hyphens and underscores
/// split words; `p-value` reads as `p` then `value`, so `pvalue` is joined.
#[derive(Debug, Clone)]
struct Word {
    text: String,
    start: usize,
    end: usize,
}

fn words_of(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut cur: Option<(usize, String)> = None;
    for (i, c) in text.char_indices() {
        if c.is_alphabetic() || c == '\'' || c == '\u{2019}' {
            cur.get_or_insert_with(|| (i, String::new())).1.extend(c.to_lowercase());
        } else if let Some((s, t)) = cur.take() {
            out.push(Word { text: t, start: s, end: i });
        }
    }
    if let Some((s, t)) = cur {
        out.push(Word { text: t, start: s, end: text.len() });
    }
    // `p-value`, `p value` → `pvalue`.
    let mut joined: Vec<Word> = Vec::new();
    for w in out {
        if w.text == "value" && joined.last().is_some_and(|p| p.text == "p") {
            let p = joined.pop().unwrap();
            joined.push(Word { text: "pvalue".into(), start: p.start, end: w.end });
        } else {
            joined.push(w);
        }
    }
    joined
}

/// Each number's quantity: the nearest quantity word within three tokens
/// (words and numbers both count), the left one on a tie; a number inside
/// the brackets right after a statistic symbol (`t(17)`, `F(1, 50)`) is a df.
fn attachments(text: &str, nums: &[Located], words: &[Word]) -> Vec<Option<Quantity>> {
    // Tokens in order: Ok(word index) or Err(number index).
    let mut tokens: Vec<(usize, Result<usize, usize>)> = words.iter().enumerate().map(|(i, w)| (w.start, Ok(i))).collect();
    tokens.extend(nums.iter().enumerate().map(|(i, n)| (n.start, Err(i))));
    tokens.sort_by_key(|t| t.0);
    let mut out = vec![None; nums.len()];
    for (pos, tok) in tokens.iter().enumerate() {
        let Err(ni) = tok.1 else { continue };
        // A df in brackets after a statistic symbol.
        let before = &text[..nums[ni].start];
        if let Some(open) = before.rfind('(') {
            let inside = &before[open + 1..];
            if !inside.contains(')') && inside.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c.is_whitespace()) {
                let head = before[..open].trim_end();
                if let Some(last) = words.iter().rev().find(|w| w.end <= head.len()) {
                    if last.end == head.len() && matches!(last.text.as_str(), "t" | "f" | "r" | "rho" | "ρ" | "chi" | "χ" | "square") {
                        out[ni] = Some(Quantity::Df);
                        continue;
                    }
                }
            }
        }
        let quantity_at = |k: usize| match tokens[k].1 {
            Ok(wi) => quantity_of(&words[wi].text),
            Err(_) => None,
        };
        // A list: a number joined to the number before it by "and", "or" or
        // "to" (or only a comma) takes that number's quantity ("a df of 2 and
        // 183").
        if pos >= 1 {
            let mut k = pos - 1;
            if let Ok(wi) = tokens[k].1 {
                if matches!(words[wi].text.as_str(), "and" | "or" | "to") && k >= 1 {
                    k -= 1;
                }
            }
            if let Err(prev) = tokens[k].1 {
                let gap = text.get(nums[prev].end..nums[ni].start).unwrap_or("x").to_lowercase();
                let joined = gap.split(|c: char| c.is_whitespace() || c == ',').all(|w| matches!(w, "" | "and" | "or" | "to"));
                if joined {
                    if let Some(q) = out[prev] {
                        out[ni] = Some(q);
                        continue;
                    }
                }
            }
        }
        // A clause mark between two tokens ends the search in that direction:
        // "…the data, 1.58, while the df…" attaches 1.58 to nothing.
        let end_of = |k: usize| match tokens[k].1 {
            Ok(wi) => words[wi].end,
            Err(n) => nums[n].end,
        };
        let boundary = |a: usize, b: usize| text.get(end_of(a)..tokens[b].0).is_some_and(|gap| gap.contains([',', ';', ':', '.', '!', '?']));
        let (mut left_open, mut right_open) = (true, true);
        let mut found = None;
        for d in 1..=3usize {
            if left_open {
                match pos.checked_sub(d) {
                    Some(k) if !boundary(k, k + 1) => {
                        if let Some(q) = quantity_at(k) {
                            found = Some(q);
                            break;
                        }
                    }
                    _ => left_open = false,
                }
            }
            if right_open {
                let k = pos + d;
                if k < tokens.len() && !boundary(k - 1, k) {
                    if let Some(q) = quantity_at(k) {
                        found = Some(q);
                        break;
                    }
                } else {
                    right_open = false;
                }
            }
        }
        out[ni] = found;
    }
    out
}

/// The verdict a word or phrase states, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Reproduced,
    NotReproduced,
    CouldNotCheck,
}

fn class_of(verdict: &str) -> Option<Class> {
    match verdict {
        "reproduced" => Some(Class::Reproduced),
        "not_reproduced" => Some(Class::NotReproduced),
        "could_not_check" => Some(Class::CouldNotCheck),
        _ => None,
    }
}

const COULD_NOT_CHECK_PHRASES: &[&str] =
    &["could not be checked", "could not check", "couldn't be checked", "cannot be checked", "can't be checked", "unable to check", "not possible to check"];

/// Every verdict class `text` states. Part A's negation rule turns a
/// reproduced-class word into the not-reproduced class; "failed" or "unable"
/// near it does too. An unclear negation is read as not reproduced: a
/// rejected explanation shows nothing, which is the safe side.
fn stated_classes(text: &str) -> Vec<Class> {
    let t = text.to_lowercase().replace('_', " ");
    let words = words_of(&t);
    let mut out = Vec::new();
    for p in COULD_NOT_CHECK_PHRASES {
        if t.contains(p) {
            out.push(Class::CouldNotCheck);
        }
    }
    let near = |i: usize, set: &[&str]| -> bool {
        let lo = i.saturating_sub(3);
        let hi = (i + 2).min(words.len());
        words[lo..hi].iter().enumerate().any(|(k, w)| lo + k != i && set.iter().any(|s| w.text.starts_with(s)))
    };
    for (i, w) in words.iter().enumerate() {
        let x = w.text.as_str();
        if ["irreproduc", "unreproduc", "nonreproduc"].iter().any(|s| x.starts_with(s)) {
            out.push(Class::NotReproduced);
            continue;
        }
        let whole_result_match = (x.starts_with("match") || x.starts_with("agree"))
            && (words.get(i + 1..(i + 3).min(words.len())).is_some_and(|n| n.iter().any(|n| n.text == "data"))
                || (i >= 1 && words[i.saturating_sub(4)..i].iter().any(|p| p.text == "every" || p.text == "all")
                    && words[i.saturating_sub(4)..i].iter().any(|p| p.text.starts_with("number"))));
        let reproduced_word =
            ["reproduc", "replicat", "confirm", "verif"].iter().any(|s| x.starts_with(s)) || whole_result_match;
        // "confirm" and "verify" about the binding (the user's mapping of
        // columns) describe that step, not the result: "no binding was
        // confirmed" states no verdict. Only when the binding is what was
        // confirmed ("binding (was) confirmed", "confirmed (the) binding"):
        // a "binding" elsewhere nearby is not enough.
        let is = |k: usize, w: &str| words.get(k).is_some_and(|x| x.text == w);
        let be = |k: usize| words.get(k).is_some_and(|x| ["was", "is", "were", "been", "be"].contains(&x.text.as_str()));
        let about_binding = (x.starts_with("confirm") || x.starts_with("verif"))
            && ((i >= 1 && words[i - 1].text.starts_with("binding"))
                || (i >= 2 && words[i - 2].text.starts_with("binding") && be(i - 1))
                || words.get(i + 1).is_some_and(|x| x.text.starts_with("binding"))
                || (is(i + 1, "the") && words.get(i + 2).is_some_and(|x| x.text.starts_with("binding"))));
        if reproduced_word && !about_binding {
            let negated = negation_of(&t, w.start, w.end) != Negation::Not || near(i, &["fail", "unable"]);
            out.push(if negated { Class::NotReproduced } else { Class::Reproduced });
            continue;
        }
        if x.starts_with("discrepanc") {
            let none = i >= 1 && words[i - 1].text == "no";
            out.push(if none { Class::Reproduced } else { Class::NotReproduced });
        }
    }
    out
}

/// No digit outside ASCII 0-9, checked on the text and on its NFKC form:
/// other scripts' digits, full-width digits, superscripts and vulgar
/// fractions are all refused.
fn has_non_ascii_digit(text: &str) -> bool {
    text.chars().any(|c| c.is_numeric() && !c.is_ascii_digit())
        || text.chars().any(|c| !c.is_ascii_digit() && c.to_string().nfkc().any(|n| n.is_ascii_digit()))
}

fn has_number_word(words: &[Word]) -> bool {
    let is_num = |s: &str| NUMBER_WORDS.contains(&s) || SMALL_NUMBER_WORDS.contains(&s);
    words.iter().enumerate().any(|(i, w)| {
        let x = w.text.as_str();
        NUMBER_WORDS.contains(&x)
            || (x == "point" && i >= 1 && i + 1 < words.len() && is_num(&words[i - 1].text) && is_num(&words[i + 1].text))
            || (x == "per" && words.get(i + 1).is_some_and(|n| n.text == "cent"))
    })
}

fn has_misconduct_word(text: &str) -> bool {
    let squashed: String = text.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    MISCONDUCT_STEMS.iter().any(|s| squashed.contains(s))
}

/// "positive" or "negative" next to the statistic or correlation must agree
/// with the computed statistic's sign.
fn sign_words_disagree(words: &[Word], sent: &ResultExplanationOutbound) -> bool {
    let computed = sent.numbers.iter().find(|n| n.kind.code == "statistic").map(|n| n.computed);
    words.iter().enumerate().any(|(i, w)| {
        let positive = w.text.starts_with("positiv");
        let negative = w.text.starts_with("negativ");
        if !positive && !negative {
            return false;
        }
        let lo = i.saturating_sub(3);
        let hi = (i + 4).min(words.len());
        let about_statistic = words[lo..hi].iter().any(|x| matches!(quantity_of(&x.text), Some(Quantity::Statistic)) || x.text.starts_with("correlat"));
        if !about_statistic {
            return false;
        }
        match computed {
            Some(v) => (positive && v < 0.0) || (negative && v > 0.0),
            None => true,
        }
    })
}

pub fn validate_explanation(out: &ModelOutput, sent: &ResultExplanationOutbound) -> Result<ModelText, Rejection> {
    let w: ExplanationWire = serde_json::from_str(&out.text).map_err(|_| Rejection::WrongShape)?;
    let text = bounded(&w.explanation, MAX_EXPLANATION_CHARS)?;
    let raw = text.as_str();
    if has_non_ascii_digit(raw) {
        return Err(Rejection::NonAsciiDigit);
    }
    let norm: String = raw.nfkc().collect();
    let words = words_of(&norm);
    if has_number_word(&words) {
        return Err(Rejection::NumberWord);
    }
    if has_misconduct_word(&norm) {
        return Err(Rejection::MisconductWord);
    }
    let nums = scan_numbers(&norm);
    let attached = attachments(&norm, &nums, &words);
    let inputs = input_values(sent);
    for (n, q) in nums.iter().zip(&attached) {
        let ok = match q {
            Some(q) => inputs.iter().any(|(iq, v)| iq == q && states(n, v)),
            None => {
                (!n.negative && ALWAYS_ALLOWED.iter().any(|a| matches_at_precision(n.written, *a) && n.written.value == *a))
                    || inputs.iter().any(|(iq, v)| (*iq == Quantity::Alpha && !n.negative && states(n, v)) || states(n, &InputValue { value: v.value, min_decimals: 0 }))
            }
        };
        if !ok {
            return Err(Rejection::NumberNotInInput);
        }
    }
    if sign_words_disagree(&words, sent) {
        return Err(Rejection::SignDisagrees);
    }
    let engine = class_of(sent.verdict);
    if stated_classes(&norm).iter().any(|c| Some(*c) != engine) {
        return Err(Rejection::WrongVerdict);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_with_their_precision_even_inside_a_word() {
        let n = numbers_in("t(58) = -2.31, p = .024; n = 1,204 of C3 rows, 95% CI.");
        let v: Vec<(f64, u32)> = n.iter().map(|w| (w.value, w.decimals)).collect();
        assert_eq!(v, vec![(58.0, 0), (2.31, 2), (0.024, 3), (1204.0, 0), (3.0, 0), (95.0, 0)]);
        assert!(numbers_in("no numbers here.").is_empty());
        assert_eq!(numbers_in("ended at 12.").len(), 1);
    }

    #[test]
    fn verdict_words_are_classified_with_their_negation() {
        assert_eq!(stated_classes("The result was reproduced."), vec![Class::Reproduced]);
        assert_eq!(stated_classes("The result was not reproduced."), vec![Class::NotReproduced]);
        assert_eq!(stated_classes("The verdict is not_reproduced."), vec![Class::NotReproduced]);
        assert_eq!(stated_classes("The data did not reproduce it."), vec![Class::NotReproduced]);
        assert_eq!(stated_classes("Reproduction failed."), vec![Class::NotReproduced]);
        assert_eq!(stated_classes("This result is irreproducible."), vec![Class::NotReproduced]);
        assert_eq!(stated_classes("This could not be checked."), vec![Class::CouldNotCheck]);
        assert_eq!(stated_classes("All printed numbers match the data."), vec![Class::Reproduced]);
        assert_eq!(stated_classes("There is no discrepancy."), vec![Class::Reproduced]);
        assert!(stated_classes("The p-value matched.").is_empty());
    }

    #[test]
    fn precision_comparison_rounds_the_allowed_value() {
        let w = WrittenNumber { value: 2.31, decimals: 2 };
        assert!(matches_at_precision(w, 2.3104));
        assert!(matches_at_precision(w, -2.305));
        assert!(!matches_at_precision(w, 2.32));
        assert!(matches_at_precision(WrittenNumber { value: 58.0, decimals: 0 }, 57.6));
    }
}
