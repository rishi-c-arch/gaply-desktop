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

use super::outbound::{role_from_name, BindingSuggestionOutbound, ClaimMethodOutbound, Outbound, ResultExplanationOutbound};
use super::spec::variant_names;
use crate::stats_l1::catalogue::{L1MethodId, LevelRule, MethodCatalogue, RoleName, SlotKind};
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
/// are dropped: values compare in magnitude, as t does.
pub fn numbers_in(text: &str) -> Vec<WrittenNumber> {
    let b: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let starts = b[i].is_ascii_digit() || (b[i] == '.' && b.get(i + 1).is_some_and(char::is_ascii_digit));
        if !starts {
            i += 1;
            continue;
        }
        let mut s = String::new();
        let mut j = i;
        while j < b.len() {
            let c = b[j];
            if c.is_ascii_digit() {
                s.push(c);
            } else if c == ',' && b.get(j + 1..j + 4).is_some_and(|w| w.iter().all(char::is_ascii_digit)) && !b.get(j + 4).is_some_and(char::is_ascii_digit) && !s.contains('.') {
                // A thousands separator.
            } else if c == '.' && !s.contains('.') && b.get(j + 1).is_some_and(char::is_ascii_digit) {
                s.push('.');
            } else {
                break;
            }
            j += 1;
        }
        let decimals = s.split_once('.').map_or(0, |(_, d)| d.len() as u32);
        let norm = if s.starts_with('.') { format!("0{s}") } else { s };
        if let Ok(value) = norm.parse::<f64>() {
            out.push(WrittenNumber { value, decimals });
        }
        i = j.max(i + 1);
    }
    out
}

/// The numbers an explanation may state: every number anywhere in its input
/// (JSON numbers, and numbers written inside its strings), plus 0, 1, 2, 100
/// and 95. The stated alpha is in the input already.
fn allowed_numbers(sent: &ResultExplanationOutbound) -> Vec<f64> {
    fn walk(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Number(n) => out.extend(n.as_f64().map(f64::abs)),
            Value::String(s) => out.extend(numbers_in(s).into_iter().map(|w| w.value)),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = vec![0.0, 1.0, 2.0, 100.0, 95.0];
    walk(&serde_json::to_value(sent).expect("serialises"), &mut out);
    out
}

/// A written number matches an allowed one when the allowed value, at the
/// written precision, is the written value.
fn matches_at_precision(w: WrittenNumber, allowed: f64) -> bool {
    let half = 0.5 * 10f64.powi(-(w.decimals as i32));
    (allowed.abs() - w.value).abs() <= half + 1e-9 * w.value.max(1.0)
}

/// The verdicts a text states, by the Level 1 words.
fn stated_verdicts(text: &str) -> Vec<&'static str> {
    let t = text.to_lowercase();
    let mut out = Vec::new();
    for p in ["could not check", "could not be checked", "couldn't be checked", "cannot be checked", "can't be checked", "unable to check"] {
        if t.contains(p) {
            out.push("could_not_check");
        }
    }
    const NEGATORS: &[&str] = &["not ", "n't ", "never ", "fail", "unable", "no longer "];
    let mut from = 0;
    while let Some(i) = t[from..].find("reproduc") {
        let at = from + i;
        let mut start = at.saturating_sub(24);
        while !t.is_char_boundary(start) {
            start -= 1;
        }
        let window = &t[start..at];
        out.push(if NEGATORS.iter().any(|n| window.contains(n)) { "not_reproduced" } else { "reproduced" });
        from = at + "reproduc".len();
    }
    out
}

pub fn validate_explanation(out: &ModelOutput, sent: &ResultExplanationOutbound) -> Result<ModelText, Rejection> {
    let w: ExplanationWire = serde_json::from_str(&out.text).map_err(|_| Rejection::WrongShape)?;
    let text = bounded(&w.explanation, MAX_EXPLANATION_CHARS)?;
    let allowed = allowed_numbers(sent);
    for n in numbers_in(text.as_str()) {
        if !allowed.iter().any(|a| matches_at_precision(n, *a)) {
            return Err(Rejection::NumberNotInInput);
        }
    }
    if stated_verdicts(text.as_str()).iter().any(|v| *v != sent.verdict) {
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
        assert_eq!(stated_verdicts("The result was reproduced."), vec!["reproduced"]);
        assert_eq!(stated_verdicts("The result was not reproduced."), vec!["not_reproduced"]);
        assert_eq!(stated_verdicts("The data did not reproduce it."), vec!["not_reproduced"]);
        assert_eq!(stated_verdicts("This could not be checked."), vec!["could_not_check"]);
        assert!(stated_verdicts("The p-value matched.").is_empty());
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
