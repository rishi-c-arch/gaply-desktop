//! Deterministic manuscript consistency checks (§11 D94).
//!
//! **NOT an AI feature.** No model, no embedder, no network — arithmetic over
//! text the parser already produced. It must work for a user who has never
//! downloaded a model, which is why it lives here and not under `ai_engine`.
//!
//! It runs BEFORE an audit is queued. The reason is one finding: a reference
//! list that is off by one silently invalidates most of the audit's
//! resolutions, so every `citation_support` verdict would be checking a claim
//! against the wrong paper. Reporting that at the END means the run was wasted
//! and the reader cannot tell which findings survived.
//!
//! # Every finding names its evidence
//!
//! The marker, the entry, the numbers. "Possible inconsistency detected" is not
//! a finding — it tells a researcher to go and re-derive what we already knew.
use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::ai_engine::audit_prepass::{BibEntry, MarkerStyle, PrepassReport};
use crate::extract::docparse::PagedBlock;

/// Does this finding make the audit's OUTPUT untrustworthy, or is it a defect
/// in the paper that leaves every verdict intact?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Resolution may be wrong — the user must acknowledge before auditing.
    Structural,
    /// Worth fixing, gates nothing.
    Cosmetic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyFinding {
    /// Stable identifier, for tests and for the UI to group by.
    pub kind: String,
    pub severity: Severity,
    /// Names the evidence. Never a category.
    pub message: String,
    /// What the reader should do, when it is not obvious from the message.
    pub action: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyReport {
    pub findings: Vec<ConsistencyFinding>,
    pub structural: usize,
    pub cosmetic: usize,
}

impl ConsistencyReport {
    fn push(&mut self, kind: &'static str, severity: Severity, message: String, action: Option<String>) {
        match severity {
            Severity::Structural => self.structural += 1,
            Severity::Cosmetic => self.cosmetic += 1,
        }
        self.findings.push(ConsistencyFinding { kind: kind.to_string(), severity, message, action });
    }
    /// Does the user have to acknowledge something before auditing?
    pub fn blocks_audit(&self) -> bool {
        self.structural > 0
    }
}

/// Minimum words for a block to be considered a "paragraph" worth comparing.
/// Below this, a repeat is a heading or a table cell, not duplicated prose.
const MIN_PARAGRAPH_WORDS: usize = 25;

fn caption_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "Table II.", "Fig. 3:", "Figure 12 —" at the start of a block.
    RE.get_or_init(|| {
        // The number must be FOLLOWED BY punctuation or the end of the line.
        //
        // Without that, "Table 2 presents mediation pathway coefficients." — a
        // cross-reference in prose — was read as a caption, and a paper that
        // discusses each of its tables was reported as numbering every one of
        // them twice. A caption is "Table 2." or "Table 2:"; prose is
        // "Table 2 presents".
        Regex::new(r"(?i)^\s*(table|fig\.?|figure)\s+([ivxlcdm]+|\d{1,3})\s*([.:—–-]|$)")
            .expect("caption regex")
    })
}

fn figref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // A reference to a figure INSIDE prose: "in Fig. 6", "see Figure 2".
    RE.get_or_init(|| Regex::new(r"(?i)\b(fig\.?|figure)\s+(\d{1,3})\b").expect("figref regex"))
}

fn section_letter_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "A. Dataset Acquisition", "B. Deep Learning…"
    RE.get_or_init(|| Regex::new(r"^\s*([A-H])\.\s+(\S.*)$").expect("section letter regex"))
}

// §11 D132. The author-year reference parser MOVED to `audit_prepass`, beside
// `BibEntry` and the DOI regex it now shares, and is imported rather than
// redefined — §11 D129 is what a second definition of "what this list says"
// costs.
use crate::ai_engine::audit_prepass::{
    entry_is_malformed, parse_author_year_entries, AuthorYearEntry,
};


/// Corroboration for the message, not for the decision: the shape that says
/// "this is the tail of the entry above".
fn looks_like_page_range(raw: &str) -> bool {
    let t = raw.trim_start().to_lowercase();
    t.starts_with("pp.") || t.starts_with("p.") || t.starts_with("vol.") || t.starts_with("no.")
}

/// DEFECT 6. Was `chars().take(n)`, which produced "Health financing for
/// universal …" in a shipped report: a title cut mid-word cannot be recognised
/// or searched for. Delegates to the ONE trim rule the reports share.
fn snippet(s: &str, n: usize) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    crate::report_compose::trim_at_word(&t, n)
}

/// A sentence quoted WHOLE: whitespace runs collapsed (PDF text carries
/// doubled spaces), nothing dropped.
///
/// For a quote whose job is to carry the evidence. `snippet(s, 70)` cut the
/// two sentences of a metric contradiction before either number (on `R PAPER`,
/// 0.945 sits at character 95 and 0.545 at 321), so the reader could not check
/// the claim without opening the manuscript. A stored finding is never
/// shortened; a surface that needs a bound cuts at a sentence boundary when it
/// renders.
fn whole(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The leading surname of one work inside a co-citation.
fn co_cite_lead_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-Z][\p{L}'’\-]+)").expect("co-cite lead regex"))
}

fn year_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:1[6-9]|20)\d{2}").expect("year regex"))
}

/// Levenshtein distance, capped at 1 — the only distance this needs.
///
/// `Kutzins` vs `Kutzin` is a possessive or a typo, not a missing reference,
/// and calling it an orphan is the false "this citation doesn't exist" that
/// costs a researcher more than a miss.
/// Strip a trailing possessive: "weiner’s" is Weiner, not a different author.
fn depossess(s: &str) -> String {
    for suffix in ["’s", "'s", "s’", "s'"] {
        if let Some(base) = s.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    s.to_string()
}

/// An all-capitals short token is an acronym, not a surname.
///
/// `(RBV; Barney, 1991)` introduces an abbreviation; reading `RBV` as an author
/// and reporting it missing is a false "this citation doesn't exist".
fn looks_like_acronym(raw: &str, surname: &str) -> bool {
    let upper: String = surname.to_uppercase();
    surname.chars().count() <= 5 && raw.contains(&upper)
}

fn within_one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (long, short) = if a.len() >= b.len() { (&a, &b) } else { (&b, &a) };
    if long.len() - short.len() > 1 {
        return false;
    }
    let mut i = 0usize;
    let mut j = 0usize;
    let mut slack = 1usize;
    while i < long.len() && j < short.len() {
        if long[i] == short[j] {
            i += 1;
            j += 1;
            continue;
        }
        if slack == 0 {
            return false;
        }
        slack -= 1;
        if long.len() == short.len() {
            i += 1;
            j += 1;
        } else {
            i += 1;
        }
    }
    // Whatever is left must fit in the remaining slack.
    (long.len() - i) + (short.len() - j) <= slack
}

/// A work boundary inside a co-citation: any `;`, or a `,` that follows a year.
///
/// A comma alone is not one: `(Smith, Jones, and Lee, 2019)` is one work.
/// Measured on the six-manuscript corpus (§11 D232), every single-work citation
/// has one year, so a comma after a year is where one work ends.
fn work_separator_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:1[6-9]|20)\d{2}[a-z]?\s*,|;").expect("work separator regex"))
}

/// Every (surname, year) an in-text marker refers to.
///
/// `markers_in` keeps only the FIRST work of `(A et al., 2015; B & C, 2024)`,
/// so the co-cited work would read as never cited. The raw text is split to
/// recover it: on `;`, and on a comma after a year (§11 D232).
///
/// In the comma form the marker's own year is the LAST work's: IJAS's
/// `(Trivedy et al. 1993, …, Mamatha et al. 2006)` is Trivedy with 2006. So the
/// lead takes the year printed in its own segment, and the marker's year only
/// when that segment prints none.
fn marker_works(raw: &str, lead: Option<&str>, year: Option<i32>) -> Vec<(String, Option<i32>)> {
    let first_year = |part: &str| year_re().find(part).and_then(|m| m.as_str().parse::<i32>().ok());
    let inner = raw.trim_start_matches('(').trim_end_matches(')');
    // (segment, whether a comma rather than a `;` opened it)
    let mut parts: Vec<(&str, bool)> = Vec::new();
    let (mut from, mut by_comma) = (0usize, false);
    for m in work_separator_re().find_iter(inner) {
        let cut = m.end() - 1; // the separator itself, `;` or `,`, one byte
        parts.push((&inner[from..cut], by_comma));
        by_comma = &inner[cut..m.end()] == ",";
        from = m.end();
    }
    parts.push((&inner[from..], by_comma));

    let mut out = Vec::new();
    if let Some(l) = lead {
        let own = if parts.len() > 1 { first_year(parts[0].0) } else { None };
        out.push((l.to_string(), own.or(year)));
    }
    for (part, by_comma) in parts.into_iter().skip(1) {
        let part = part.trim();
        let Some(c) = co_cite_lead_re().captures(part) else {
            continue;
        };
        let y = first_year(part);
        // After a comma, a segment is a work only if it prints a year: that is
        // what keeps `(Smith, 2019, Table 2)` from citing "Table".
        if by_comma && y.is_none() {
            continue;
        }
        out.push((c[1].to_lowercase(), y));
    }
    out
}

/// The author-year equivalents of the numbered path's structural guarantees
/// (§11 D96). Silent — and says so — when the list is numbered instead.
fn check_author_year(out: &mut ConsistencyReport, blocks: &[PagedBlock], report: &PrepassReport) {
    if !report.bibliography.is_empty() {
        return; // numbered paper: the numeric checks own it
    }
    let (entries, malformed) = parse_author_year_entries(blocks);
    if entries.is_empty() && malformed.is_empty() {
        return; // no reference list found at all — not this check's business
    }

    for raw in &malformed {
        out.push(
            "reference-entry-unparseable",
            Severity::Structural,
            format!(
                "A reference entry could not be read as an author and a year, so no citation can \
                 resolve against it: “{}”",
                snippet(raw, 100)
            ),
            Some("Give it an author surname and a (year), or remove it.".to_string()),
        );
    }

    // Every work every marker refers to.
    let mut cited: BTreeMap<String, Vec<(Option<i32>, String)>> = BTreeMap::new();
    for p in &report.planned {
        for m in &p.markers {
            if m.style != MarkerStyle::AuthorYear {
                continue;
            }
            for (surname, year) in marker_works(&m.raw, m.lead_author.as_deref(), m.year) {
                cited.entry(depossess(&surname)).or_default().push((year, m.raw.clone()));
            }
        }
    }

    let mut matched_entries: std::collections::BTreeSet<String> = Default::default();
    for (surname, uses) in &cited {
        let exact: Vec<&AuthorYearEntry> = entries.iter().filter(|e| &e.surname == surname).collect();
        if !exact.is_empty() {
            matched_entries.insert(surname.clone());
            // A year check is meaningful ONLY when the marker names one year:
            // "(Dubai 2013, Abu Dhabi 2006)" pairs one work's author with
            // another's year, and comparing them says nothing.
            for (year, raw) in uses {
                let (Some(y), true) = (year, year_re().find_iter(raw).count() == 1) else { continue };
                if exact.iter().any(|e| e.year == Some(*y)) {
                    continue;
                }
                let listed: Vec<String> =
                    exact.iter().filter_map(|e| e.year).map(|y| y.to_string()).collect();
                out.push(
                    "marker-year-mismatch",
                    Severity::Structural,
                    format!(
                        "“{}” cites {y}, but the reference list has {} under that name with {}.",
                        snippet(raw, 60),
                        if exact.len() == 1 { "the entry" } else { "entries" },
                        if listed.is_empty() { "no year".to_string() } else { listed.join(" and ") }
                    ),
                    Some("Correct the year, or cite the edition you mean.".to_string()),
                );
            }
            continue;
        }

        // NEAR match: matched, and said to be near rather than asserted exact.
        if let Some(near) = entries.iter().find(|e| within_one_edit(&e.surname, surname)) {
            matched_entries.insert(near.surname.clone());
            out.push(
                "uncertain-reference-match",
                Severity::Cosmetic,
                format!(
                    "“{}” was matched to the entry beginning “{}”. The surnames differ by one \
                     character, so this may be a typo rather than a different work.",
                    snippet(&uses[0].1, 50),
                    snippet(&near.raw, 50)
                ),
                Some(
                    "Check which spelling is right, and make the marker and the reference entry \
                     agree."
                        .to_string(),
                ),
            );
            continue;
        }

        // Appears in SOME entry, just not as its leading surname: a third
        // author cited alone, or a word inside an organisation's name. Matched
        // loosely and SAID to be loose — never asserted missing.
        if let Some(e) = entries.iter().find(|e| {
            e.raw.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '-').any(|w| w == surname)
        }) {
            matched_entries.insert(e.surname.clone());
            out.push(
                "uncertain-reference-match",
                Severity::Cosmetic,
                format!(
                    "“{}” names “{surname}”, which appears in the entry beginning “{}” but is not \
                     the name it is listed under. It may be a co-author cited alone.",
                    snippet(&uses[0].1, 50),
                    snippet(&e.raw, 50)
                ),
                // §11 D131 is why this is not cosmetic in its consequences: a
                // mis-split name cost a FETCHABLE source its check. The action
                // is the one that makes the marker resolve.
                Some(
                    "Check this is the work you meant, and cite it by the name its entry is \
                     listed under."
                        .to_string(),
                ),
            );
            continue;
        }

        // An acronym introduced in the text is not a missing reference.
        if uses.iter().any(|(_, raw)| looks_like_acronym(raw, surname)) {
            continue;
        }

        out.push(
            "orphan-author-year-marker",
            Severity::Structural,
            format!(
                "“{}” cites a work the reference list does not contain: no entry begins with \
                 “{surname}”. It is cited {} time{}.",
                snippet(&uses[0].1, 60),
                uses.len(),
                if uses.len() == 1 { "" } else { "s" }
            ),
            Some("Add the reference, or correct the citation.".to_string()),
        );
    }

    for e in &entries {
        if matched_entries.contains(&e.surname) {
            continue;
        }
        out.push(
            "reference-never-cited",
            Severity::Cosmetic,
            format!("Listed but never cited in the text: “{}”", snippet(&e.raw, 90)),
            Some("Cite it, or remove it from the list.".to_string()),
        );
    }
}

/* ------------------ same metric, same subject (§11 D97) ------------------ */

/// Metrics whose NAME is unambiguous only in capitals.
///
/// `or` is the English conjunction — "internationally owned or subsidiary 37"
/// is not an odds ratio — and lower-casing it produced a value on every second
/// sentence of a real paper.
const UPPER_METRICS: &[&str] = &["MCC", "AUC", "OR", "R²", "R2"];
/// Metrics safe to match case-insensitively.
const WORD_METRICS: &[&str] = &[
    "macro-F1", "F1-score", "F1 score", "F1", "accuracy", "precision", "recall",
    "kappa", "cross-entropy", "sensitivity", "specificity",
];

fn subject_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // NOTE the absence of a trailing \b. `C′` has no word boundary after the
    // prime, so a trailing \b makes the regex backtrack to `Path C` — which
    // collapses the TOTAL and DIRECT effects of a mediation model into one
    // subject and manufactures a contradiction in a paper that has none.
    RE.get_or_init(|| {
        Regex::new(
            r"\b(Path\s+[A-Z]['’′]?|SemEval[-\s]?\d{4}|SemEval|ISEAR|GoEmotions|HEFCSO|[A-Z][A-Za-z]*-?(?:BiLSTM|LSTM|BERT|SVM|CNN))",
        )
        .expect("subject regex")
    })
}

/// One reading of a metric: its value, what it was about, and where it was said.
struct MetricUse {
    metric: String,
    value: f64,
    subjects: Vec<String>,
    sentence: String,
}

/// The subject key. The prime SURVIVES — `path c` and `path c'` are different
/// quantities.
fn subject_key(raw: &str) -> String {
    raw.to_lowercase()
        .replace(['’', '′'], "'")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '\'')
        .collect()
}

fn subjects_of(sentence: &str) -> Vec<String> {
    let mut v: Vec<String> =
        subject_re().find_iter(sentence).map(|m| subject_key(m.as_str())).collect();
    v.sort();
    v.dedup();
    v
}

/// The value a metric mention carries, or `None` when the text does not give one.
fn value_near(sentence: &str, at: usize, end: usize) -> Option<f64> {
    let after_raw: String = sentence.chars().skip(end).take(40).collect();
    let before_raw: String = {
        let start = at.saturating_sub(18);
        sentence.get(start..at).unwrap_or("").to_string()
    };
    // A confidence LEVEL is not the metric's value: "ROC AUC with 95% CI".
    let strip_ci = |s: &str| {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"(?:9\d|100)\s*%\s*CI").expect("ci regex"))
            .replace_all(s, " ")
            .to_string()
    };
    let after = strip_ci(&after_raw);
    let before = strip_ci(&before_raw);

    // A p-value is not a metric value.
    static P_RE: OnceLock<Regex> = OnceLock::new();
    let p_re = P_RE.get_or_init(|| Regex::new(r"\bp\s*[<>=]\s*$").expect("p regex"));
    if p_re.is_match(before.trim_end()) {
        return None;
    }

    // "MCC of 0.945", "AUC = 0.88", "F1-score: 0.9"
    static AFTER_RE: OnceLock<Regex> = OnceLock::new();
    let a_re = AFTER_RE.get_or_init(|| {
        Regex::new(r"^\s*(?:score\s+)?(?:of|=|was|is|:|reaches|reached)?\s*(\d+(?:\.\d+)?)")
            .expect("after regex")
    });
    if let Some(c) = a_re.captures(&after) {
        return c[1].parse().ok();
    }
    // "96.42% accuracy" — the number comes first.
    static BEFORE_RE: OnceLock<Regex> = OnceLock::new();
    let b_re = BEFORE_RE
        .get_or_init(|| Regex::new(r"(\d+(?:\.\d+)?)\s*%?\s*$").expect("before regex"));
    b_re.captures(&before).and_then(|c| c[1].parse().ok())
}

fn canonical_metric(name: &str) -> String {
    let l = name.to_lowercase().replace(['-', ' '], "");
    match l.as_str() {
        "f1score" | "macrof1" | "f1" => "F1".to_string(),
        "r2" | "r²" => "R²".to_string(),
        other => {
            if UPPER_METRICS.iter().any(|m| m.eq_ignore_ascii_case(name)) {
                name.to_uppercase()
            } else {
                other.to_string()
            }
        }
    }
}

fn collect_metric_uses(blocks: &[PagedBlock]) -> Vec<MetricUse> {
    let mut out = Vec::new();
    for b in blocks {
        // The repo's own splitter, NOT `split('.')` — a naive split cuts
        // "MCC of 0.945" into "MCC of 0" and "945", which reads the metric's
        // value as zero and loses the subject to the next fragment. That is
        // why the first Rust port missed a contradiction the prototype caught.
        for sentence in crate::extract::sentence::sentences_in(&b.text) {
            let sentence = sentence.trim();
            if sentence.is_empty() {
                continue;
            }
            let subjects = subjects_of(sentence);
            // Capitals-only metrics.
            for m in UPPER_METRICS {
                let mut from = 0usize;
                while let Some(rel) = sentence[from..].find(m) {
                    let at = from + rel;
                    let end = at + m.len();
                    let boundary_l = at == 0
                        || !sentence[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric());
                    let boundary_r = !sentence[end..].chars().next().is_some_and(|c| c.is_alphanumeric());
                    if boundary_l && boundary_r {
                        if let Some(v) = value_near(sentence, at, end) {
                            out.push(MetricUse {
                                metric: canonical_metric(m),
                                value: v,
                                subjects: subjects.clone(),
                                sentence: sentence.to_string(),
                            });
                        }
                    }
                    from = end;
                }
            }
            // Case-insensitive metrics.
            let lower = sentence.to_lowercase();
            for m in WORD_METRICS {
                let needle = m.to_lowercase();
                let mut from = 0usize;
                while let Some(rel) = lower[from..].find(&needle) {
                    let at = from + rel;
                    let end = at + needle.len();
                    let boundary_r = !lower[end..].chars().next().is_some_and(|c| c.is_alphanumeric());
                    if boundary_r {
                        if let Some(v) = value_near(sentence, at, end) {
                            out.push(MetricUse {
                                metric: canonical_metric(m),
                                value: v,
                                subjects: subjects.clone(),
                                sentence: sentence.to_string(),
                            });
                        }
                    }
                    from = end;
                }
            }
        }
    }
    out
}

/// Same metric, same subject, two values (§11 D97).
fn check_metric_agreement(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let uses = collect_metric_uses(blocks);
    let mut by_metric: BTreeMap<String, Vec<&MetricUse>> = BTreeMap::new();
    for u in &uses {
        by_metric.entry(u.metric.clone()).or_default().push(u);
    }

    for (metric, list) in by_metric {
        // Numeric comparison: 95 and 95.00 are one value.
        let distinct: Vec<f64> = {
            let mut v: Vec<f64> = list.iter().map(|u| u.value).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
            v
        };
        if distinct.len() < 2 {
            continue;
        }
        let mut asserted = false;
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                let (a, b) = (list[i], list[j]);
                if (a.value - b.value).abs() < f64::EPSILON {
                    continue;
                }
                // ONE SENTENCE REPORTING TWO VALUES IS A COMPARISON, not a
                // contradiction: "GoEmotions … reached only 46% macro-F1 …
                // substantially lower than the 95% of the present study" names
                // both figures on purpose, and the subject scan attributes both
                // to GoEmotions. A contradiction needs two separate statements.
                if a.sentence == b.sentence {
                    continue;
                }
                let shared: Vec<&String> =
                    a.subjects.iter().filter(|s| b.subjects.contains(s)).collect();
                if shared.is_empty() {
                    continue;
                }
                asserted = true;
                out.push(
                    "metric-contradiction",
                    Severity::Structural,
                    format!(
                        "{metric} is reported as {} and as {} for the same subject ({}). \
                         “{}” versus “{}”",
                        a.value,
                        b.value,
                        shared.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                        whole(&a.sentence),
                        whole(&b.sentence)
                    ),
                    Some("One of them is wrong. Check which, and correct it everywhere.".to_string()),
                );
            }
        }
        if asserted {
            continue;
        }
        // Values differ and no shared subject was established anywhere. ONE
        // finding for the metric — three values make three pairs, and a reader
        // needs to know one thing rather than three.
        if list.iter().all(|u| u.subjects.is_empty()) {
            out.push(
                "metric-values-unattributed",
                Severity::Cosmetic,
                format!(
                    "{metric} appears with {} different values ({}), and Gaply could not \
                     establish whether they describe the same thing. They may be different \
                     datasets, models or subgroups.",
                    distinct.len(),
                    distinct
                        .iter()
                        .map(|v| format!("{v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Some(
                    "Check whether these describe the same thing. If they do, one of them is \
                     wrong; if they do not, say which is which."
                        .to_string(),
                ),
            );
        }
    }
}

/// Run every check. Pure: same inputs, same findings, no clock and no I/O.
pub fn check_consistency(blocks: &[PagedBlock], report: &PrepassReport) -> ConsistencyReport {
    let mut out = ConsistencyReport::default();
    check_reference_list(&mut out, report);
    check_orphan_markers(&mut out, report);
    check_captions(&mut out, blocks);
    check_section_letters(&mut out, blocks);
    check_repeated_paragraphs(&mut out, blocks);
    check_mixed_styles(&mut out, report);
    check_author_year(&mut out, blocks, report);
    check_metric_agreement(&mut out, blocks);
    out
}

/// (1) Entries that are not references, and (3) markers that resolve to one.
fn check_reference_list(out: &mut ConsistencyReport, report: &PrepassReport) {
    let bad: Vec<&BibEntry> = report.bibliography.values().filter(|e| entry_is_malformed(e)).collect();
    for e in &bad {
        let above = report.bibliography.keys().filter(|n| **n > e.number).count();
        out.push(
            "reference-entry-malformed",
            Severity::Structural,
            format!(
                "[{}] does not look like a reference entry, because it names no author{}: “{}”. \
                 If it is a continuation of [{}], then the {} entries numbered above it are each \
                 shifted by one, and every marker pointing at them resolves to the wrong paper.",
                e.number,
                if looks_like_page_range(&e.raw) { ", and reads as a page range" } else { "" },
                snippet(&e.raw, 90),
                e.number.saturating_sub(1),
                above
            ),
            Some(
                "Renumber the reference list so this text joins the entry above it, then re-run. \
                 Auditing before you do will check claims against the wrong sources."
                    .to_string(),
            ),
        );
    }

    let bad_numbers: Vec<u32> = bad.iter().map(|e| e.number).collect();
    if bad_numbers.is_empty() {
        return;
    }
    // (3) Which markers actually land on one of those entries.
    let mut hits: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for p in &report.planned {
        for m in &p.markers {
            for n in &m.numbers {
                if bad_numbers.contains(n) {
                    // Whole: the marker this finding is about can sit anywhere
                    // in the sentence, and a cut hid it (`whole`'s doc).
                    hits.entry(*n).or_default().push(whole(&p.sentence));
                }
            }
        }
    }
    for (n, sentences) in hits {
        out.push(
            "marker-resolves-to-malformed-entry",
            Severity::Structural,
            format!(
                "{} sentence{} [{n}], which is not a usable reference entry. First: “{}”",
                sentences.len(),
                if sentences.len() == 1 { " cites" } else { "s cite" },
                sentences.first().map(String::as_str).unwrap_or("")
            ),
            // §11 D148. This carried no action, and the report renders a
            // missing action as "No action needed." — beside a severity of
            // "Affects the audit", which told the reader to ignore the finding
            // that says their citations may resolve to the wrong paper.
            //
            // The action is real and is the same one the malformed entry needs:
            // this finding IS that one's consequence.
            Some(format!(
                "Fix reference entry [{n}] first (reported above). Until it reads as a reference, \
                 this sentence's citation cannot be checked."
            )),
        );
    }
}

/// (2) `[n]` with no entry `n`.
fn check_orphan_markers(out: &mut ConsistencyReport, report: &PrepassReport) {
    if report.bibliography.is_empty() {
        return; // author-year paper, or no references heading — not an orphan
    }
    let lo = report.bibliography.keys().min().copied().unwrap_or(0);
    let hi = report.bibliography.keys().max().copied().unwrap_or(0);
    let mut orphans: BTreeMap<u32, usize> = BTreeMap::new();
    for p in &report.planned {
        for m in &p.markers {
            for n in &m.numbers {
                if !report.bibliography.contains_key(n) {
                    *orphans.entry(*n).or_default() += 1;
                }
            }
        }
    }
    for (n, count) in orphans {
        out.push(
            "orphan-marker",
            Severity::Structural,
            format!(
                "[{n}] is cited {count} time{} but the reference list has no entry {n}. It runs \
                 from [{lo}] to [{hi}].",
                if count == 1 { "" } else { "s" }
            ),
            Some("Add the missing reference, or correct the marker.".to_string()),
        );
    }
}

/// (4) duplicate table/figure numbers and (5) figures cited but never captioned.
fn check_captions(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut figure_labels: Vec<String> = Vec::new();
    for b in blocks {
        if let Some(c) = caption_re().captures(&b.text) {
            let kind = c.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
            let kind = if kind.starts_with("fig") { "Figure" } else { "Table" };
            let num = c.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
            let key = format!("{kind} {num}");
            if kind == "Figure" {
                figure_labels.push(num.clone());
            }
            seen.entry(key).or_default().push(snippet(&b.text, 70));
        }
    }
    for (label, caps) in &seen {
        if caps.len() > 1 {
            out.push(
                "duplicate-caption-number",
                Severity::Cosmetic,
                format!(
                    "{label} is used {} times. First: “{}”. Also: “{}”",
                    caps.len(),
                    caps[0],
                    caps[1]
                ),
                Some("Renumber one of them. A cross-reference cannot say which you mean.".to_string()),
            );
        }
    }

    // A figure referred to in prose that no caption defines.
    let mut referenced: BTreeMap<String, usize> = BTreeMap::new();
    for b in blocks {
        if caption_re().is_match(&b.text) {
            continue; // a caption is a definition, not a reference
        }
        for c in figref_re().captures_iter(&b.text) {
            if let Some(n) = c.get(2) {
                *referenced.entry(n.as_str().to_string()).or_default() += 1;
            }
        }
    }
    for (n, count) in referenced {
        if !figure_labels.contains(&n) {
            out.push(
                "figure-referenced-but-absent",
                Severity::Cosmetic,
                format!(
                    "Figure {n} is referred to {count} time{} but no caption defines it. \
                     Captions found: {}",
                    if count == 1 { "" } else { "s" },
                    if figure_labels.is_empty() {
                        "none".to_string()
                    } else {
                        figure_labels.join(", ")
                    }
                ),
                // §11 D150. This carried no action, so the report printed "No
                // action needed." beside it. A figure a reviewer is told to
                // look at and cannot find is a real defect, and one of the
                // cheapest to fix.
                Some("Add the caption, or remove the reference.".to_string()),
            );
        }
    }
}

/// (6) `A, A, B, C` — a lettered run that does not advance by one.
fn check_section_letters(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let mut run: Vec<(char, String)> = Vec::new();
    for b in blocks {
        // §11 D148. STOP AT THE REFERENCES HEADING.
        //
        // Everything past it is a bibliography, and an IEEE entry begins with
        // the first author's initial. Word strips the auto-number, so the block
        // reaches here as `A. Vaswani et al., "Attention is all you need"` and
        // matches the subsection pattern exactly. On a real manuscript that
        // produced: *Subsection "D. Demszky et al., "GoEmotions: A dataset ..."
        // follows "A. Vaswani et al., "Attention is all you ...": the letters do
        // not advance by one* — a finding about a subsection that does not
        // exist, sending the reader to look for it.
        //
        // The prepass already draws this line for exactly this reason; this
        // check is now the second caller to respect it rather than the one that
        // ignored it.
        if crate::ai_engine::audit_prepass::is_references_heading(&b.text) {
            break;
        }
        if let Some(c) = section_letter_re().captures(&b.text) {
            let ch = c.get(1).and_then(|m| m.as_str().chars().next()).unwrap_or('?');
            let title = c.get(2).map(|m| snippet(m.as_str(), 40)).unwrap_or_default();
            run.push((ch, title));
        }
    }
    for w in run.windows(2) {
        let (prev, next) = (w[0].0, w[1].0);
        // 'A' after 'C' is a NEW section's first subsection, which is correct —
        // but 'A' after 'A' is the defect itself, not a restart, so the
        // allowance requires the previous letter to be a LATER one.
        if next as u8 == prev as u8 + 1 || (next == 'A' && prev != 'A') {
            continue;
        }
        out.push(
            "section-letters-out-of-sequence",
            Severity::Cosmetic,
            format!(
                "Subsection “{}. {}” follows “{}. {}”: the letters do not advance by one.",
                next, w[1].1, prev, w[0].1
            ),
            Some("Renumber the subsections so the letters run in order.".to_string()),
        );
    }
}

/// §11 D149. WHERE A BLOCK IS, in the words the rest of the report uses.
///
/// The repeated-paragraph finding said "at blocks 220 and 221". `blocks` is
/// this function's argument name: a 0-based index into the parsed document,
/// which a reader cannot count to and cannot search for. The audit locates
/// everything else by page, or by paragraph where the format has no pages, and
/// this now says the same thing in the same vocabulary.
///
/// The ordinal is `i + 1` because the pre-pass counts paragraphs 1-based over
/// EVERY block, blanked ones included, precisely so its locators match what a
/// reader counts in their own document. The two numberings are the same
/// numbering, and `a_repeated_paragraph_is_located_the_way_every_other_finding_is`
/// pins that they stay so.
///
/// Page wins where there is one, matching `audit_report::locator`: two locators
/// for one place is a reader deciding which to trust.
fn block_locations(blocks: &[PagedBlock], at: &[usize]) -> String {
    let paged = at.iter().all(|&i| blocks.get(i).and_then(|b| b.page).is_some());
    let mut nums: Vec<String> = at
        .iter()
        .map(|&i| match blocks.get(i).and_then(|b| b.page) {
            Some(p) if paged => p.to_string(),
            _ => (i + 1).to_string(),
        })
        .collect();
    nums.dedup();
    let unit = match (paged, nums.len()) {
        (true, 1) => "page",
        (true, _) => "pages",
        (false, 1) => "paragraph",
        (false, _) => "paragraphs",
    };
    format!("{unit} {}", nums.join(" and "))
}

/// (7) The same paragraph, printed twice.
fn check_repeated_paragraphs(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let mut seen: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, b) in blocks.iter().enumerate() {
        let norm = b.text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        if norm.split_whitespace().count() < MIN_PARAGRAPH_WORDS {
            continue;
        }
        seen.entry(norm).or_default().push(i);
    }
    for (norm, at) in seen {
        if at.len() > 1 {
            out.push(
                "repeated-paragraph",
                Severity::Cosmetic,
                format!(
                    "A paragraph appears {} times, at {}: “{}”",
                    at.len(),
                    block_locations(blocks, &at),
                    snippet(&norm, 80)
                ),
                Some("Delete the duplicate, or rewrite one of them.".to_string()),
            );
        }
    }
}

/// (8) One section citing `(Smith, 2019)` in a `[n]` paper.
fn check_mixed_styles(out: &mut ConsistencyReport, report: &PrepassReport) {
    let mut numeric = 0usize;
    let mut author_year = 0usize;
    // Per section, so the finding can name where.
    let mut by_section: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for p in &report.planned {
        let section = p.section.clone().unwrap_or_else(|| "(no section)".to_string());
        let e = by_section.entry(section).or_default();
        for m in &p.markers {
            match m.style {
                MarkerStyle::Numeric => {
                    numeric += 1;
                    e.0 += 1;
                }
                MarkerStyle::AuthorYear => {
                    author_year += 1;
                    e.1 += 1;
                }
            }
        }
    }
    if numeric == 0 || author_year == 0 {
        return; // one style throughout — nothing mixed
    }
    let document_is_numeric = numeric >= author_year;
    let (doc_style, odd_style) = if document_is_numeric {
        ("numeric [n]", "author-year")
    } else {
        ("author-year", "numeric [n]")
    };
    for (section, (n, a)) in by_section {
        let (odd, same) = if document_is_numeric { (a, n) } else { (n, a) };
        if odd > 0 && odd > same {
            out.push(
                "mixed-citation-style",
                Severity::Cosmetic,
                format!(
                    "“{section}” uses {odd_style} citations ({odd} of them) while the rest of the \
                     paper uses {doc_style}."
                ),
                Some("Convert them to the document's style so every marker resolves the same way.".to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_engine::audit_prepass::prepass_blocks;

    fn blocks(texts: &[&str]) -> Vec<PagedBlock> {
        texts
            .iter()
            .map(|t| PagedBlock { page: Some(1), style: None, text: (*t).to_string() })
            .collect()
    }
    fn run(texts: &[&str]) -> ConsistencyReport {
        let b = blocks(texts);
        let r = prepass_blocks(&b);
        check_consistency(&b, &r)
    }
    fn kinds(r: &ConsistencyReport) -> Vec<String> {
        r.findings.iter().map(|f| f.kind.clone()).collect()
    }
    fn message(r: &ConsistencyReport, kind: &str) -> String {
        r.findings.iter().find(|f| f.kind == kind).map(|f| f.message.clone()).unwrap_or_default()
    }

    /// THE ONE THIS EXISTS FOR (§11 D94). Verbatim from `R PAPER .pdf`: entry
    /// [6] is a page-range continuation numbered as its own reference, so every
    /// marker above it resolves one paper too far.
    #[test]
    fn a_page_range_numbered_as_a_reference_is_structural() {
        let r = run(&[
            "Classical lexical-based approaches [5] suffer from a lack of reasoning about words.",
            "The BiLSTM architecture [7] handles long-range dependencies in social media text.",
            "References",
            "[5] S. Mohammad and P. Turney, \"Crowdsourcing a word-emotion association lexicon,\" 2013.",
            "[6] pp. 436-465, 2013.",
            "[7] C. Cortes and V. Vapnik, \"Support-vector networks,\" Mach. Learn., vol. 20, 1995.",
        ]);
        assert!(r.blocks_audit(), "an off-by-one reference list must gate the audit");
        assert!(kinds(&r).iter().any(|k| k == "reference-entry-malformed"), "{:?}", kinds(&r));

        let m = message(&r, "reference-entry-malformed");
        // NAMES THE EVIDENCE: the ordinal, the text, and the consequence.
        assert!(m.contains("[6]"), "{m}");
        assert!(m.contains("pp. 436-465"), "{m}");
        assert!(m.contains("shifted by one"), "{m}");
        assert!(m.contains("resolves to the wrong paper"), "{m}");
    }

    /// The rule that was tried FIRST and missed it: "no author AND no year".
    /// A page range ends with a year, so that let the defect through — while a
    /// genuine entry whose year fails to parse must NOT be flagged.
    #[test]
    fn absence_of_an_author_decides_it_not_absence_of_a_year() {
        let with_year = BibEntry {
            number: 6,
            raw: "pp. 436-465, 2013.".into(),
            doi: None,
            lead_author: None,
            year: Some(2013),
        };
        assert!(entry_is_malformed(&with_year), "a page range with a year must still be caught");

        let no_year = BibEntry {
            number: 5,
            raw: "S. Mohammad and P. Turney, \"Crowdsourcing a lexicon\".".into(),
            doi: None,
            lead_author: Some("mohammad".into()),
            year: None,
        };
        assert!(!entry_is_malformed(&no_year), "a real reference whose year did not parse is fine");
    }

    #[test]
    fn a_marker_landing_on_a_malformed_entry_is_named() {
        let r = run(&[
            "Emotion detection in social media text relies on lexical resources [6] for polarity.",
            "References",
            "[5] S. Mohammad, \"Crowdsourcing a word-emotion lexicon,\" 2013.",
            "[6] pp. 436-465, 2013.",
        ]);
        let m = message(&r, "marker-resolves-to-malformed-entry");
        assert!(m.contains("[6]"), "{m}");
        assert!(m.contains("cites"), "singular verb: {m}");
    }

    /// **THE QUOTE MUST SHOW THE MARKER IT IS ABOUT.** The citing sentence is
    /// verbatim from `R PAPER .docx`; the entries are this file's fixture lines.
    ///
    /// The test above cannot see truncation: `m.contains("[6]")` is satisfied by
    /// the lead-in ("1 sentence cites [6]") whatever the quote holds. The quote
    /// WAS cut at 60 characters, and `[6]` sits at character 75, so the reader
    /// saw "Classical lexical-based approaches [5] or superficial …": only `[5]`,
    /// under a finding about `[6]`. This asserts on the quote itself.
    #[test]
    fn a_marker_on_a_malformed_entry_quotes_its_sentence_whole() {
        const SENTENCE: &str = "Classical lexical-based approaches [5] or superficial learning classifiers [6] suffer from a lack of good reasoning and understanding about out-of-vocabulary words and domain shift.";
        let r = run(&[
            SENTENCE,
            "References",
            "[5] S. Mohammad and P. Turney, \"Crowdsourcing a word-emotion association lexicon,\" 2013.",
            "[6] pp. 436-465, 2013.",
            "[7] C. Cortes and V. Vapnik, \"Support-vector networks,\" Mach. Learn., vol. 20, 1995.",
        ]);
        let m = message(&r, "marker-resolves-to-malformed-entry");
        let q = quotes(&m);
        assert_eq!(q, vec![SENTENCE], "the quote must be the citing sentence, whole: {m}");
        assert!(q[0].contains("[6]"), "the quote must show the marker the finding is about: {m}");
    }

    /// **IJAS, THE CASE THIS EXISTS FOR.** Its reference list prints years bare
    /// (`Surname I. YYYY.`), and the parser read every entry as unparseable and
    /// every marker as citing a missing work: 48 structural findings, all false.
    /// Citing sentences and entries verbatim from the manuscript; the second
    /// sentence runs on past "in B." (as in *B. mori*) and is cut there.
    ///
    /// Asserts the parse POSITIVELY, twice, so an empty list cannot pass by
    /// producing no findings: the pre-pass holds four entries, and the check
    /// reports exactly one thing, that the fourth (Slama 1966, verbatim, cited
    /// nowhere in this fixture) is never cited. Only a PARSED bare-year entry
    /// can produce that finding; with no entries the check is silent.
    #[test]
    fn unbracketed_year_citations_resolve_against_their_entries() {
        let texts = ["Total protein was estimated by the Coomassie brilliant blue dye-binding method with bovine serum albumin as standard at 595 nm (Bradford 1976).", "The analogues thus behave as growth stimulants only within a narrow window, an interpretation consistent with the dose dependence reported by Kamimura and Kiuchi (1998), who found that fenoxycarb prolonged the fifth stadium and progressively prevented cocoon formation and pupation as the dose was raised, and by Göncü and Parlak (2011), who showed that fenoxycarb disturbs midgut remodelling in B.", "References", "Göncü E and Parlak O. 2011. The influence of juvenile hormone analogue, fenoxycarb on the midgut remodeling in Bombyx mori (L., 1758) (Lepidoptera: Bombycidae) during larval– pupal metamorphosis. Turkish Journal of Entomology 35(2): 179–94.", "Kamimura M and Kiuchi M. 1998. Effects of a juvenile hormone analogue, fenoxycarb, on 5th stadium larvae of the silkworm, Bombyx mori (Lepidoptera: Bombycidae). Applied Entomology and Zoology 33(2): 333–38. https://doi.org/10.1303/aez.33.333", "Bradford M M. 1976. A rapid and sensitive method for the quantitation of microgram quantities of protein utilizing the principle of protein–dye binding. Analytical Biochemistry 72: 248– 54.", "Slama K and Williams C M. 1966. Juvenile hormone V. The sensitivity of the bug, Pyrrhocoris apterus, to a hormonally active factor in American paper-pulp. Biological Bulletin 130: 235–46."];
        let b = blocks(&texts);
        let pre = prepass_blocks(&b);
        assert_eq!(pre.author_year_bibliography.len(), 4, "{:?}", pre.author_year_unreadable);
        let r = check_consistency(&b, &pre);
        let got: Vec<(&str, &str)> = r.findings.iter().map(|f| (f.kind.as_str(), f.message.as_str())).collect();
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].0, "reference-never-cited", "{got:?}");
        assert!(got[0].1.contains("Slama"), "{got:?}");
    }

    /// NEGATIVE CONTROL: a numbered list still skips the author-year check, even
    /// when an entry in it prints a bare year (R PAPER's shape; its list is
    /// numbered). The numbered parse is asserted positively.
    #[test]
    fn a_numbered_list_skips_the_author_year_check_even_with_a_bare_year_entry() {
        let texts = [
            "Total protein was estimated by the dye-binding method at 595 nm [1].",
            "References",
            "[1] Bradford M M. 1976. A rapid and sensitive method for the quantitation of microgram quantities of protein utilizing the principle of protein–dye binding. Analytical Biochemistry 72: 248– 54.",
        ];
        let b = blocks(&texts);
        let pre = prepass_blocks(&b);
        assert_eq!(pre.bibliography.len(), 1, "the numbered list was not parsed");
        assert!(pre.author_year_bibliography.is_empty());
        let r = check_consistency(&b, &pre);
        let author_year_kinds = [
            "reference-entry-unparseable",
            "orphan-author-year-marker",
            "reference-never-cited",
            "uncertain-reference-match",
            "marker-year-mismatch",
        ];
        assert!(
            !kinds(&r).iter().any(|k| author_year_kinds.contains(&k.as_str())),
            "the author-year check ran on a numbered list: {:?}",
            kinds(&r)
        );
    }

    #[test]
    fn an_orphan_marker_names_the_range_the_list_covers() {
        let r = run(&[
            "The proposed approach outperforms every published baseline on this corpus [42].",
            "References",
            "[1] T. Cover and P. Hart, \"Nearest neighbor pattern classification,\" 1967.",
            "[2] D. Whitley, \"A genetic algorithm tutorial,\" Stat. Comput., 1994.",
        ]);
        assert!(r.blocks_audit());
        let m = message(&r, "orphan-marker");
        assert!(m.contains("[42]"), "{m}");
        assert!(m.contains("[1]") && m.contains("[2]"), "must name the real range: {m}");
    }

    #[test]
    fn a_paper_with_no_numbered_list_produces_no_orphans() {
        // Author-year paper: every numeric check must stay silent rather than
        // reporting every marker as an orphan.
        let r = run(&[
            "Emotion detection has been studied widely (Smith, 2019) across many corpora.",
            "The approach follows Tang et al. (2016) in its treatment of aspect terms.",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "orphan-marker"), "{:?}", kinds(&r));
        assert!(!r.blocks_audit());
    }

    #[test]
    fn duplicate_table_numbers_name_both_captions() {
        let r = run(&[
            "TABLE II. HEFCSO HYPERPARAMETER CONFIGURATION Parameter Value Tuned By Range",
            "TABLE II. COMPARATIVE PERFORMANCE ON SEMEVAL-2018 Method Accuracy F1 MCC",
        ]);
        let m = message(&r, "duplicate-caption-number");
        assert!(m.contains("Table II"), "{m}");
        assert!(m.contains("HYPERPARAMETER"), "{m}");
        assert!(m.contains("COMPARATIVE"), "both captions must be named: {m}");
        // Cosmetic: a duplicate number invalidates no verdict.
        assert!(!r.blocks_audit());
    }

    /// FOUND ON A REAL PAPER (§11 D94). "Revised Health Economics Paper" was
    /// reported as numbering Table 2 and Table 3 twice — because it DISCUSSES
    /// each of its tables, and "Table 2 presents…" is a cross-reference in
    /// prose, not a caption. A check that fires on a paper for describing its
    /// own tables is worse than no check.
    #[test]
    fn a_prose_cross_reference_is_not_a_caption() {
        let r = run(&[
            "Table 2 presents mediation pathway coefficients. In Path A, each one-log-unit rise \
             in out-of-pocket spending was associated with a decrease in the index.",
            "Table 2. Mediation Pathway Coefficients and Primary Bootstrapped Indirect Effects",
        ]);
        assert!(
            !kinds(&r).iter().any(|k| k == "duplicate-caption-number"),
            "a sentence discussing Table 2 was counted as a second caption: {:?}",
            r.findings
        );
    }

    /// And the real duplicate is still caught — the fix must not disarm it.
    #[test]
    fn two_real_captions_with_the_same_number_are_still_caught() {
        let r = run(&[
            "Table 2. Mediation Pathway Coefficients and Primary Bootstrapped Indirect Effects",
            "Table 2: Multivariable Logistic Regression Adjusted Odds Ratios",
        ]);
        assert!(kinds(&r).iter().any(|k| k == "duplicate-caption-number"), "{:?}", r.findings);
    }

    #[test]
    fn a_figure_cited_but_never_captioned_lists_the_ones_that_exist() {
        let r = run(&[
            "The convergence behaviour of the optimiser is shown in Fig. 6 across all datasets.",
            "Fig. 1. Proposed HEFCSO-BiLSTM System Architecture",
            "Fig. 3. Accuracy and F1-Score Comparison on the benchmark",
        ]);
        let m = message(&r, "figure-referenced-but-absent");
        assert!(m.contains("Figure 6"), "{m}");
        assert!(m.contains("1, 3"), "must list the captions that DO exist: {m}");
    }

    #[test]
    fn a_captioned_figure_is_not_reported_as_absent() {
        let r = run(&[
            "The architecture is depicted in Fig. 1 and described in the following section.",
            "Fig. 1. Proposed HEFCSO-BiLSTM System Architecture",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "figure-referenced-but-absent"), "{:?}", kinds(&r));
    }

    #[test]
    fn section_letters_that_do_not_advance_are_reported() {
        let r = run(&[
            "A. Dataset Acquisition Three corpora from publicly available benchmarks were employed.",
            "A. Preprocessing Pipeline Emoji were replaced with their textual descriptions first.",
            "B. GloVe Embeddings Each token is mapped to a 200-dimensional pretrained vector.",
        ]);
        let m = message(&r, "section-letters-out-of-sequence");
        assert!(m.contains("A."), "{m}");
        assert!(m.contains("do not advance"), "{m}");
    }

    /// §11 D148. A REFERENCE ENTRY IS NOT A SUBSECTION HEADING.
    ///
    /// An IEEE entry begins with the first author's initial, and Word strips the
    /// auto-number, so `A. Vaswani et al., "Attention is all you need"` arrives
    /// as a block matching the subsection pattern exactly. On a real manuscript
    /// this produced a finding about a subsection that does not exist, which
    /// sends a reader looking for it.
    #[test]
    fn a_reference_entry_is_not_read_as_a_subsection() {
        let r = run(&[
            "A. Dataset Acquisition Three corpora from publicly available benchmarks were employed.",
            "B. GloVe Embeddings Each token is mapped to a 200-dimensional pretrained vector.",
            "References",
            "A. Vaswani et al., \u{201c}Attention is all you need,\u{201d} in NeurIPS, 2017, pp. 5998-6008.",
            "D. Demszky et al., \u{201c}GoEmotions: A dataset of fine-grained emotions,\u{201d} ACL, 2020.",
        ]);
        assert!(
            !kinds(&r).iter().any(|k| k == "section-letters-out-of-sequence"),
            "a reference list was read as subsections: {:?}",
            r.findings
        );
    }

    #[test]
    fn a_new_sections_first_subsection_is_not_out_of_sequence() {
        // C then A is a NEW section starting, which is correct.
        let r = run(&[
            "A. Dataset Acquisition Three corpora from public benchmarks were employed here.",
            "B. GloVe Embeddings Each token maps to a 200-dimensional pretrained vector space.",
            "C. Proposed Algorithm The optimiser treats each configuration as a vector.",
            "A. Ablation Results Removing the attention layer costs two accuracy points.",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "section-letters-out-of-sequence"), "{:?}", kinds(&r));
    }

    /// §11 D150. EVERY FINDING SAYS WHAT TO DO ABOUT IT.
    ///
    /// The report renders a missing action as "No action needed." for a
    /// cosmetic finding (§11 D148), which is right for a finding that genuinely
    /// needs nothing and wrong for five that did. "Figure 6 is referred to 1
    /// time but no caption defines it" told a reader to ignore a figure a
    /// reviewer would flag.
    ///
    /// Checked over the whole check set rather than one kind at a time, because
    /// the defect was not in any single check: it was that a missing action had
    /// a plausible-looking default, so nothing ever made the absence visible.
    #[test]
    fn every_finding_says_what_to_do_about_it() {
        // Wide enough to reach every check that fires on prose: an orphan
        // marker, a near-miss surname, a co-author cited alone, a figure with
        // no caption, a duplicated caption number, a repeated paragraph,
        // out-of-order subsections and a mixed citation style.
        let para = "A major drawback of this framework is that it has been restricted to single label \
                    problems and does not yet handle the multi label case that real corpora present.";
        let r = run(&[
            "A. Dataset Acquisition Three corpora from publicly available benchmarks were employed.",
            "D. Ablation Results Removing the attention layer costs two accuracy points.",
            "Accuracy reached 91.2% on the held out split, as Figure 6 shows in the appendix.",
            "Fig. 3. F1-Score Across Three Benchmark Datasets",
            "Fig. 3. Training Loss Convergence on the Emotions Dataset",
            para,
            "An unrelated paragraph that says something else entirely about the data and its shape.",
            para,
            "The approach follows Kutzins (2013) and was extended by Banerjee (2021) for our case.",
            "References",
            "Kutzin, J. (2013). Health financing for universal coverage. Bulletin of the WHO.",
        ]);
        assert!(r.findings.len() >= 4, "the fixture reached too little: {:?}", kinds(&r));
        let silent: Vec<&str> = r
            .findings
            .iter()
            .filter(|f| f.action.is_none())
            .map(|f| f.kind.as_str())
            .collect();
        assert!(
            silent.is_empty(),
            "these findings carry no action, so the report will tell the reader to ignore \
             them: {silent:?}"
        );
    }

    /// The one this was reported as (§11 D150).
    #[test]
    fn a_referenced_figure_with_no_caption_says_what_to_do() {
        let r = run(&[
            "Accuracy reached 91.2% on the held out split, as Figure 6 shows in the appendix.",
            "Fig. 3. F1-Score Across Three Benchmark Datasets",
        ]);
        let f = r
            .findings
            .iter()
            .find(|f| f.kind == "figure-referenced-but-absent")
            .expect("no finding");
        assert_eq!(f.action.as_deref(), Some("Add the caption, or remove the reference."));
    }

    #[test]
    fn a_paragraph_printed_twice_is_reported_with_both_locations() {
        let para = "A major drawback of this framework is that it has been restricted to single label \
                    problems and does not yet handle the multi label case that real corpora present.";
        let r = run(&[para, "An unrelated paragraph that says something else entirely about the data.", para]);
        let m = message(&r, "repeated-paragraph");
        assert!(m.contains("2 times"), "{m}");
        // Every block in this fixture is on page 1, so that is where they are.
        assert!(m.contains("at page 1"), "must name where: {m}");
    }

    /// §11 D149. A LOCATION A READER CAN COUNT TO.
    ///
    /// This said "at blocks 220 and 221" on a real Word manuscript: a 0-based
    /// index into the parsed document, in a report that locates everything else
    /// by paragraph. The ordinal here must be the pre-pass's, because that is
    /// the one every other finding in the report is printed with.
    #[test]
    fn a_repeated_paragraph_is_located_the_way_every_other_finding_is() {
        let para = "A major drawback of this framework is that it has been restricted to single label \
                    problems and does not yet handle the multi label case that real corpora present.";
        let texts = [para, "An unrelated paragraph that says something else entirely about the data.", para];
        // A Word manuscript: no pages, so the locator is the paragraph ordinal.
        let b: Vec<PagedBlock> = texts
            .iter()
            .map(|t| PagedBlock { page: None, style: None, text: (*t).to_string() })
            .collect();
        let pre = prepass_blocks(&b);
        let r = check_consistency(&b, &pre);
        let m = message(&r, "repeated-paragraph");
        assert!(m.contains("at paragraphs 1 and 3"), "{m}");
        assert!(!m.contains("block"), "the parser's own word reached the reader: {m}");

        // And it is the SAME number the pre-pass gives a sentence in that
        // paragraph, which is what makes the two comparable on one page of the
        // report. Drift here would have the report locating one finding at
        // paragraph 3 and another at paragraph 2 for the same place.
        let third = pre
            .planned
            .iter()
            .find(|p| p.sentence.starts_with("A major drawback"))
            .and_then(|p| p.paragraph);
        assert_eq!(third, Some(1), "the pre-pass numbers paragraphs differently: {third:?}");
    }

    #[test]
    fn a_short_repeated_line_is_not_a_repeated_paragraph() {
        // Headings and table cells repeat legitimately.
        let r = run(&["Results", "Results", "Discussion"]);
        assert!(!kinds(&r).iter().any(|x| x == "repeated-paragraph"), "{:?}", kinds(&r));
    }

    #[test]
    fn a_section_citing_in_the_minority_style_is_reported() {
        let r = run(&[
            "II. RELATED WORK",
            "B. Deep Learning Architectures The TD-LSTM was introduced by Tang et al. (2016) to model aspects.",
            "The ATAE-LSTM model was proposed by Wang et al. (2016) to incorporate aspect information.",
            "III. METHODOLOGY",
            "Recurrent network parameters have been tuned by PSO [19] and Genetic Algorithms [8].",
            "Classical lexical approaches [5] suffer from a lack of reasoning about vocabulary.",
            "The BiLSTM architecture [7] handles the long-range dependencies of informal text.",
        ]);
        let m = message(&r, "mixed-citation-style");
        assert!(m.contains("author-year"), "{m}");
        assert!(m.contains("numeric"), "{m}");
        assert!(!r.blocks_audit(), "a style mismatch invalidates no verdict");
    }

    #[test]
    fn one_style_throughout_is_never_reported_as_mixed() {
        let r = run(&[
            "Classical lexical approaches [5] suffer from a lack of reasoning about vocabulary.",
            "The BiLSTM architecture [7] handles the long-range dependencies of informal text.",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "mixed-citation-style"), "{:?}", kinds(&r));
    }

    /* ------------------- author-year (§11 D96) ------------------- */

    /// The list this paper actually has, trimmed. Two personal entries, one
    /// organisational, one with a digit in the name.
    fn ay_paper(extra: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = vec![
            "Kutzin, J. (2013). Health financing for universal coverage and health system performance.".into(),
            "Mathauer, I., Saksena, P., & Kutzin, J. (2019). Pooling arrangements in health financing systems.".into(),
            "Buchmueller, T. C., DiNardo, J., & Valletta, R. G. (2011). The effect of an employer mandate.".into(),
            "P4H Network. (2024). Oman updates compulsory health insurance policy for the private sector.".into(),
            "The Financial Services Authority. (2025). Dhamani platform statistics for the first quarter.".into(),
            "References".into(),
        ];
        v.rotate_right(1); // put "References" first
        v.extend(extra.iter().map(|s| s.to_string()));
        v
    }
    fn run_ay(body: &[&str], refs: &[&str]) -> ConsistencyReport {
        let mut all: Vec<String> = body.iter().map(|s| s.to_string()).collect();
        all.extend(ay_paper(refs));
        let owned: Vec<&str> = all.iter().map(String::as_str).collect();
        run(&owned)
    }

    /// A citation to a work that appears NOWHERE in the list. The only shape
    /// that may be asserted missing.
    #[test]
    fn a_marker_with_no_trace_in_the_list_is_an_orphan() {
        let r = run_ay(
            &["Coverage expanded rapidly across the region in the following decade (Nakamura, 2018)."],
            &[],
        );
        let m = message(&r, "orphan-author-year-marker");
        assert!(m.contains("Nakamura"), "{m}");
        assert!(r.blocks_audit(), "an orphan citation must gate");
    }

    /// A FALSE "this citation doesn't exist" is worse than a missed one, so a
    /// co-author cited alone is matched loosely and SAID to be loose.
    #[test]
    fn a_co_author_cited_alone_is_uncertain_not_an_orphan() {
        let r = run_ay(
            &["The employer mandate reduced uninsurance among affected workers (Valletta, 2011)."],
            &[],
        );
        assert!(
            !kinds(&r).iter().any(|k| k == "orphan-author-year-marker"),
            "a third author cited alone was asserted missing: {:?}",
            r.findings
        );
        let m = message(&r, "uncertain-reference-match");
        assert!(m.contains("valletta"), "{m}");
        assert!(m.contains("co-author cited alone"), "{m}");
        assert!(!r.blocks_audit(), "an uncertain match must not gate");
    }

    /// "Kutzins (2013)" against "Kutzin, J. (2013)" — a possessive or a typo.
    #[test]
    fn a_surname_one_character_out_is_uncertain_not_an_orphan() {
        let r = run_ay(&["Kutzins (2013) sets out the financing functions in detail for policymakers."], &[]);
        assert!(!kinds(&r).iter().any(|k| k == "orphan-author-year-marker"), "{:?}", r.findings);
        let m = message(&r, "uncertain-reference-match");
        assert!(m.contains("differ by one character"), "{m}");
    }

    #[test]
    fn a_possessive_marker_matches_its_entry() {
        let r = run_ay(&["Kutzin’s (2013) framework separates revenue raising from purchasing entirely."], &[]);
        assert!(!kinds(&r).iter().any(|k| k == "orphan-author-year-marker"), "{:?}", r.findings);
    }

    /// "(RBV; Barney, 1991)" introduces an abbreviation, and RBV is not an author.
    #[test]
    fn an_acronym_is_not_reported_as_a_missing_reference() {
        let r = run_ay(
            &["The resource-based view (RBV; Kutzin, 2013) explains persistent differences between firms."],
            &[],
        );
        assert!(
            !r.findings.iter().any(|f| f.message.to_lowercase().contains("rbv")),
            "an acronym was read as an author: {:?}",
            r.findings
        );
    }

    /// Organisational authors are valid APA and must not be called unreadable.
    #[test]
    fn organisational_entries_parse() {
        let r = run_ay(&["Compulsory cover was extended to the private sector in that year (P4H Network, 2024)."], &[]);
        assert!(
            !kinds(&r).iter().any(|k| k == "reference-entry-unparseable"),
            "a valid organisational entry was called unreadable: {:?}",
            r.findings
        );
    }

    #[test]
    fn an_entry_that_is_not_author_and_year_is_structural() {
        let r = run_ay(&["Coverage rose steadily over the period under review by the regulator."],
                       &["see the appendix for the full methodology and the survey instrument used"]);
        let m = message(&r, "reference-entry-unparseable");
        assert!(m.contains("appendix"), "{m}");
        assert!(r.blocks_audit());
    }

    /// A year check is meaningful only when the marker names ONE year.
    #[test]
    fn a_year_that_disagrees_is_structural() {
        let r = run_ay(&["Health financing functions were set out at the time (Kutzin, 2011)."], &[]);
        let m = message(&r, "marker-year-mismatch");
        assert!(m.contains("2011"), "{m}");
        assert!(m.contains("2013"), "must name the year the list carries: {m}");
        assert!(r.blocks_audit());
    }

    /// "(Dubai 2013, Abu Dhabi 2006)" pairs one work's author with another's
    /// year, so comparing them says nothing.
    #[test]
    fn a_marker_naming_two_years_is_not_year_checked() {
        let r = run_ay(&["Both emirates legislated early (Kutzin 2013, Abu Dhabi 2006)."], &[]);
        assert!(
            !kinds(&r).iter().any(|k| k == "marker-year-mismatch"),
            "a two-year marker was year-checked: {:?}",
            r.findings
        );
    }

    /// `markers_in` keeps only the FIRST work of a co-citation, so the second
    /// would read as never cited unless it is recovered.
    #[test]
    fn the_second_work_of_a_co_citation_counts_as_cited() {
        let r = run_ay(
            &["Pooling reduces fragmentation across schemes (Kutzin, 2013; Mathauer et al., 2019)."],
            &[],
        );
        let never: Vec<String> = r
            .findings
            .iter()
            .filter(|f| f.kind == "reference-never-cited")
            .map(|f| f.message.clone())
            .collect();
        assert!(
            !never.iter().any(|m| m.contains("Mathauer")),
            "the co-cited work was reported as never cited: {never:?}"
        );
    }

    /// Every (surname, year) the check credits for one sentence, driven from
    /// `markers_in` so the lead and year are what the parser really supplies.
    fn works_of(sentence: &str) -> Vec<(String, Option<i32>)> {
        crate::ai_engine::audit_prepass::markers_in(sentence)
            .iter()
            .flat_map(|m| marker_works(&m.raw, m.lead_author.as_deref(), m.year))
            .collect()
    }
    fn w(s: &str, y: i32) -> (String, Option<i32>) {
        (s.to_string(), Some(y))
    }

    /// §11 D232. IJAS's co-citation, verbatim with the PDF's doubled spaces.
    /// The parser's marker is Trivedy with Mamatha's 2006; each work must be
    /// credited under its own author and year.
    #[test]
    fn a_comma_co_citation_credits_each_work_its_own_year() {
        let s = "sub-lethal doses improve cocoon and post-cocoon traits (Trivedy  et al.  1993, \
                 Kamimura and Kiuchi 1998, Miranda  et al.  2002, Mamatha et al. 2006).";
        assert_eq!(
            works_of(s),
            vec![w("trivedy", 1993), w("kamimura", 1998), w("miranda", 2002), w("mamatha", 2006)]
        );
    }

    /// §11 D232. The comma form with two works, the shape in its simplest case.
    #[test]
    fn a_comma_co_citation_of_two_works_credits_both() {
        assert_eq!(works_of("as shown (Smith 2019, Jones 2020)."), vec![w("smith", 2019), w("jones", 2020)]);
    }

    /// NEGATIVE CONTROL (§11 D232): commas between authors of ONE work are not
    /// work separators. Only a comma after a year is.
    #[test]
    fn a_single_work_with_comma_separated_authors_is_one_work() {
        assert_eq!(works_of("as shown (Smith, Jones, and Lee, 2019)."), vec![w("smith", 2019)]);
    }

    /// NEGATIVE CONTROL (§11 D232): the IJAS single-work form, no comma at all.
    #[test]
    fn the_single_work_ijas_form_is_one_work() {
        assert_eq!(works_of("as shown (Kamimura and Kiuchi 1998)."), vec![w("kamimura", 1998)]);
    }

    /// NEGATIVE CONTROL (§11 D232): a page locator follows a year-comma and is
    /// not a work. The parser makes no marker of this form today (the premise,
    /// pinned), so `marker_works` is also called on it directly.
    #[test]
    fn a_page_locator_is_not_a_second_work() {
        assert!(works_of("as shown (Smith, 2019, p. 12).").is_empty());
        for raw in ["(Smith, 2019, p. 12)", "(Smith, 2019, Table 2)"] {
            assert_eq!(marker_works(raw, Some("smith"), Some(2019)), vec![w("smith", 2019)], "{raw}");
        }
    }

    /// NEGATIVE CONTROL (§11 D232): the semicolon form, which already worked.
    #[test]
    fn a_semicolon_co_citation_still_credits_each_work() {
        assert_eq!(works_of("as shown (Smith, 2019; Jones, 2020)."), vec![w("smith", 2019), w("jones", 2020)]);
    }

    /// §11 D232, end to end: IJAS's sentence and the four entries it cites,
    /// verbatim. Miranda 2002 is cited only here and was reported never cited.
    #[test]
    fn every_work_of_the_ijas_co_citation_counts_as_cited() {
        let texts = [
            "In Bombyx mori L., sub- lethal doses applied during the last instar prolong the feeding period and improve cocoon and post-cocoon traits (Trivedy  et al.  1993, Kamimura and Kiuchi 1998, Miranda  et al.  2002, Mamatha et al. 2006).",
            "References",
            "Kamimura M and Kiuchi M. 1998. Effects of a juvenile hormone analogue, fenoxycarb, on 5th stadium larvae of the silkworm, Bombyx mori (Lepidoptera: Bombycidae). Applied Entomology and Zoology 33(2): 333–38. https://doi.org/10.1303/aez.33.333",
            "Mamatha D M, Cohly H P P, Raju A H H and Rao M R. 2006. Studies on the quantitative and qualitative characters of cocoons and silk from methoprene and fenoxycarb treated Bombyx mori (L) larvae. African Journal of Biotechnology 5(15): 1422–26.",
            "Miranda J E, De Bortoli S A and Takahashi R. 2002. Development and silk production by silkworm larvae after topical application of methoprene. Scientia Agricola 59(3): 585–88. https://doi.org/10.1590/S0103-90162002000300026",
            "Trivedy K, Remadevi O K, Magadum S B and Datta R K. 1993. Effect of juvenile hormone analogue, Labomin, on the growth and economic characters of the silkworm, Bombyx mori L. Indian Journal of Sericulture 32(2): 162–68.",
        ];
        let b = blocks(&texts);
        let pre = prepass_blocks(&b);
        assert_eq!(pre.author_year_bibliography.len(), 4, "{:?}", pre.author_year_unreadable);
        let r = check_consistency(&b, &pre);
        let got: Vec<(&str, &str)> = r.findings.iter().map(|f| (f.kind.as_str(), f.message.as_str())).collect();
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn an_entry_nobody_cites_is_cosmetic() {
        let r = run_ay(&["Coverage rose steadily over the period under review by the regulator."], &[]);
        let m = message(&r, "reference-never-cited");
        assert!(m.contains("Kutzin"), "{m}");
        let f = r.findings.iter().find(|f| f.kind == "reference-never-cited").unwrap();
        assert_eq!(f.severity, Severity::Cosmetic, "an unused entry moves no verdict");
    }

    /// Each family stays out of the other's paper.
    #[test]
    fn the_author_year_checks_are_silent_on_a_numbered_paper() {
        let r = run(&[
            "Classical lexical approaches [5] suffer from a lack of reasoning about vocabulary.",
            "References",
            "[5] S. Mohammad and P. Turney, \"Crowdsourcing a word-emotion lexicon,\" 2013.",
        ]);
        for k in ["orphan-author-year-marker", "reference-never-cited", "marker-year-mismatch"] {
            assert!(!kinds(&r).iter().any(|x| x == k), "{k} fired on a numbered paper: {:?}", r.findings);
        }
    }

    #[test]
    fn within_one_edit_is_exactly_one() {
        assert!(within_one_edit("kutzin", "kutzins"));
        assert!(within_one_edit("kutzin", "kutzon"));
        assert!(within_one_edit("kutzin", "kutzin"));
        assert!(!within_one_edit("kutzin", "kutzinsky"));
        assert!(!within_one_edit("smith", "jones"));
    }

    /* ---------------- metric agreement (§11 D97) ---------------- */

    /// THE TARGET, verbatim from `R PAPER`: MCC 0.945 in the abstract, 0.545 in
    /// the conclusion, for the same dataset.
    #[test]
    fn the_same_metric_with_two_values_for_one_subject_is_a_contradiction() {
        let r = run(&[
            "In the combined data set, the results achieved 96.42% accuracy, 95.00% F1-score, \
             and an MCC of 0.945 for SemEval 2018, with 10 percentage points better than the best baseline.",
            "The paper discussed the development of HEFCSO-BiLSTM, achieving an MCC of 0.545 on \
             SemEval2018 with complete relative improvements over the baselines.",
        ]);
        let m = message(&r, "metric-contradiction");
        assert!(m.contains("0.945") && m.contains("0.545"), "{m}");
        assert!(m.contains("semeval2018"), "must name the shared subject: {m}");
        assert!(r.blocks_audit(), "a contradiction must gate the audit");
    }

    /// The quoted text between each “…” pair, in order.
    fn quotes(m: &str) -> Vec<&str> {
        m.split('“').skip(1).filter_map(|q| q.split('”').next()).collect()
    }

    /// **A QUOTE MUST CARRY THE EVIDENCE IT QUOTES.** Both sentences verbatim
    /// from `R PAPER .docx`, unshortened.
    ///
    /// The test above cannot see truncation: its sentences are shortened, and
    /// `m.contains("0.945")` is satisfied by the message's lead-in ("reported as
    /// 0.945 and as 0.545") whatever the quotes say. The quotes WERE cut at 70
    /// characters, before either number (0.945 sits at character 95, 0.545 at
    /// 321), so a reader could not check the claim without opening the
    /// manuscript. This asserts on the quotes themselves.
    #[test]
    fn a_metric_contradiction_quotes_both_sentences_whole() {
        const ABSTRACT: &str = "In the combined data set, the results achieved 96.42% accuracy, 95.00% F1-score, and an MCC of 0.945 for SemEval 2018, with 10 percentage points better than the best baseline in accuracy.";
        const CONCLUSION: &str = "The paper discussed the development of HEFCSO-BiLSTM, a new model capable of recognizing emotions from social media text, combining a hybrid-nature inspired metaheuristic hyperparameter optimization by using BiLSTM with an attention mechanism, achieving state-of-the-art results in terms of 96.42% accuracy and an MCC of 0.545 on SemEval2018, with complete relative gains across all datasets.";
        let r = run(&[ABSTRACT, CONCLUSION]);
        let m = message(&r, "metric-contradiction");
        let q = quotes(&m);
        assert_eq!(q, vec![ABSTRACT, CONCLUSION], "each quote must be its sentence, whole: {m}");
        assert!(q[0].contains("0.945") && q[1].contains("0.545"), "{m}");
    }

    /// The same conclusion sentence as `R PAPER .pdf` extracts it, with the
    /// doubled spaces of PDF text. The quote collapses whitespace runs (as the
    /// old `snippet` did) and drops nothing else.
    #[test]
    fn a_whole_quote_collapses_pdf_whitespace_and_nothing_else() {
        const ABSTRACT: &str = "In the combined data set, the results achieved 96.42% accuracy, 95.00% F1-score, and an MCC of 0.945 for SemEval 2018, with 10 percentage points better than the best baseline in accuracy.";
        const CONCLUSION_PDF: &str = "The paper discussed the development of HEFCSO-BiLSTM, a new model  capable  of  recognizing  emotions  from  social  media  text, combining  a  hybrid-nature  inspired  metaheuristic  hyperparameter optimization  by  using  BiLSTM  with  an  attention  mechanism, achieving state-of-the-art results in terms of 96.42% accuracy and an MCC of 0.545 on SemEval2018, with complete relative gains across all  datasets.";
        let r = run(&[ABSTRACT, CONCLUSION_PDF]);
        let m = message(&r, "metric-contradiction");
        let q = quotes(&m);
        let collapsed = CONCLUSION_PDF.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(q.get(1).copied(), Some(collapsed.as_str()), "{m}");
        assert!(!m.contains("  "), "a doubled space reached the message: {m}");
    }

    /// THE PRIME IS LOAD-BEARING. `Path C` is the total effect and `Path C′` the
    /// direct effect: different quantities, correctly reported with different
    /// odds ratios. A trailing `\b` in the subject pattern made the regex
    /// backtrack past the prime and manufacture a contradiction in a real paper
    /// that had none.
    #[test]
    fn path_c_and_path_c_prime_are_different_subjects() {
        let r = run(&[
            "The total association of firm size with formal provision (Path C) was OR = 3.90, \
             with a confidence interval that excludes unity for the pooled sample.",
            "The direct association of size with provision net of capacity (Path C′: OR = 2.58) \
             remains after controlling for the mediator in the same model.",
        ]);
        assert!(
            !kinds(&r).iter().any(|k| k == "metric-contradiction"),
            "the total and direct effects were called a contradiction: {:?}",
            r.findings
        );
    }

    /// One sentence naming two values is a COMPARISON. "GoEmotions reached only
    /// 46% … substantially lower than the 95% of the present study" states both
    /// on purpose, and the subject scan attributes both to GoEmotions.
    #[test]
    fn two_values_in_one_sentence_are_a_comparison_not_a_contradiction() {
        let r = run(&[
            "In contrast, GoEmotions by Demszky et al. reached only 46% macro-F1 over 27 \
             categories, which is substantially lower than the 95% macro-F1 of the present study.",
        ]);
        assert!(
            !kinds(&r).iter().any(|k| k == "metric-contradiction"),
            "a within-sentence comparison was read as a contradiction: {:?}",
            r.findings
        );
    }

    /// "or" is the English conjunction. Lower-casing the metric list put an
    /// odds ratio on every second sentence of a real paper.
    #[test]
    fn the_word_or_is_not_an_odds_ratio() {
        let r = run(&[
            "Firms were locally owned 182 (82.0%), internationally owned or subsidiary 37 (16.7%), \
             and not specified in the remaining three cases.",
            "Coverage was recorded as one for a licensed product or 0 otherwise across the sample.",
        ]);
        assert!(
            !r.findings.iter().any(|f| f.kind.starts_with("metric-")),
            "the conjunction was read as a metric: {:?}",
            r.findings
        );
    }

    /// A confidence LEVEL is not the metric's value, and a p-value is not either.
    #[test]
    fn confidence_levels_and_p_values_are_not_metric_values() {
        let r = run(&[
            "Model discrimination was assessed with ROC AUC with 95% CI as the primary criterion.",
            "The coefficient was significant at p < 0.001 across every specification tested here.",
        ]);
        assert!(
            !r.findings.iter().any(|f| f.kind.starts_with("metric-")),
            "a CI level or a p-value became a metric value: {:?}",
            r.findings
        );
    }

    /// "96.42% accuracy" puts the number FIRST, and 95 and 95.00 are one value.
    #[test]
    fn a_value_before_the_metric_is_read_and_compared_numerically() {
        let r = run(&[
            "On SemEval-2018 the model achieved 95% accuracy across the held-out evaluation split.",
            "The same SemEval-2018 configuration achieved 95.00% accuracy when the run was repeated.",
        ]);
        assert!(
            !kinds(&r).iter().any(|k| k == "metric-contradiction"),
            "95 and 95.00 were compared as text: {:?}",
            r.findings
        );
    }

    /// Values differ, no subject anywhere: say so once, not once per pair.
    #[test]
    fn unattributed_values_produce_one_uncertain_finding_per_metric() {
        let r = run(&[
            "The reported kappa was 0.71 for the first annotation round of the corpus.",
            "A later round reported a kappa of 0.83 under the revised guidelines document.",
            "A third round reported a kappa of 0.90 after the adjudication step was added.",
        ]);
        let uncertain: Vec<&ConsistencyFinding> =
            r.findings.iter().filter(|f| f.kind == "metric-values-unattributed").collect();
        assert_eq!(uncertain.len(), 1, "three values gave three findings: {:?}", r.findings);
        assert_eq!(uncertain[0].severity, Severity::Cosmetic);
        let m = &uncertain[0].message;
        assert!(m.contains("0.71") && m.contains("0.83") && m.contains("0.9"), "{m}");
        assert!(m.contains("could not establish"), "{m}");
        assert!(!r.blocks_audit(), "an unattributed spread must not gate");
    }

    /// Table II lists accuracy for nine methods — nine subjects, not nine
    /// contradictions.
    #[test]
    fn the_same_metric_for_different_subjects_is_not_a_contradiction() {
        let r = run(&[
            "The HEFCSO-BiLSTM model reached 96.42% accuracy on the combined evaluation corpus.",
            "The baseline SVM reached 88.70% accuracy on the same combined evaluation corpus.",
        ]);
        assert!(
            !kinds(&r).iter().any(|k| k == "metric-contradiction"),
            "two models' scores were called a contradiction: {:?}",
            r.findings
        );
    }

    /// A clean manuscript must produce NOTHING. A check that fires on everything
    /// is noise a researcher learns to skip.
    #[test]
    fn a_consistent_manuscript_produces_no_findings() {
        let r = run(&[
            "Emotion detection in social media text is a long-standing problem in the field [1].",
            "The BiLSTM architecture [2] handles the long-range dependencies of informal text.",
            "Fig. 1. Proposed system architecture",
            "The architecture is depicted in Fig. 1 and described in the following section.",
            "References",
            "[1] M. De Choudhury, S. Counts, and E. Horvitz, \"Mining social media,\" 2013.",
            "[2] M. Schuster and K. Paliwal, \"Bidirectional recurrent neural networks,\" 1997.",
        ]);
        assert_eq!(r.findings.len(), 0, "clean manuscript flagged: {:?}", r.findings);
        assert!(!r.blocks_audit());
    }

    /// Severity is the gate, and only resolution-invalidating findings hold it.
    #[test]
    fn only_structural_findings_gate_the_audit() {
        for kind in ["reference-entry-malformed", "orphan-marker", "marker-resolves-to-malformed-entry"] {
            let f = ConsistencyFinding {
                kind: kind.to_string(),
                severity: Severity::Structural,
                message: String::new(),
                action: None,
            };
            assert_eq!(f.severity, Severity::Structural, "{kind}");
        }
        let mut r = ConsistencyReport::default();
        r.push("duplicate-caption-number", Severity::Cosmetic, String::new(), None);
        assert!(!r.blocks_audit(), "a cosmetic finding must not gate");
        r.push("orphan-marker", Severity::Structural, String::new(), None);
        assert!(r.blocks_audit());
    }
}
