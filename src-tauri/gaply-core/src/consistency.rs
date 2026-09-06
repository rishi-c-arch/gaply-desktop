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

/// A reference entry that does not look like a reference.
///
/// THE OFF-BY-ONE DETECTOR. A page-range continuation numbered as its own entry
/// — the defect that made `[7]` resolve to an SVM paper — has neither an author
/// nor a year, because it is the tail of the entry above it.
fn entry_is_malformed(e: &BibEntry) -> bool {
    // NO AUTHOR is the discriminator, on its own. A reference begins with who
    // wrote it; a continuation does not.
    //
    // The first rule tried was "no author AND no year", and it MISSED the real
    // defect: `[6] pp. 436-465, 2013.` carries a year, because a page range
    // ends with one. Requiring both let the very entry this check exists for
    // through. The year is no help here — entry [5] of the same paper is a
    // genuine reference whose year did not parse.
    e.lead_author.is_none()
}

/// Corroboration for the message, not for the decision: the shape that says
/// "this is the tail of the entry above".
fn looks_like_page_range(raw: &str) -> bool {
    let t = raw.trim_start().to_lowercase();
    t.starts_with("pp.") || t.starts_with("p.") || t.starts_with("vol.") || t.starts_with("no.")
}

fn snippet(s: &str, n: usize) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= n {
        return t;
    }
    let cut: String = t.chars().take(n).collect();
    format!("{cut}…")
}

/// One entry from an author-year reference list (§11 D96).
#[derive(Debug, Clone)]
struct AuthorYearEntry {
    /// Lower-cased leading surname, as `markers_in` reports a marker's.
    surname: String,
    year: Option<i32>,
    raw: String,
}

/// The leading surname of one work inside a co-citation.
fn co_cite_lead_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-Z][\p{L}'’\-]+)").expect("co-cite lead regex"))
}

fn ay_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "Alkenbrack, S., Hanson, K., & Lindelow, M. (2015). Title…"
    // "AlRuthia, Y., Aldallal, S., … et al. (2025). Title…"
    RE.get_or_init(|| {
        // Two shapes, both valid APA:
        //   "Alkenbrack, S., Hanson, K., & Lindelow, M. (2015)."  personal
        //   "P4H Network. (2024)."                                 organisation
        // Requiring the comma reported both organisational entries in a real
        // paper as unreadable, which is the check calling correct APA wrong.
        // Digits belong in a name: "P4H Network" is an organisation, and
        // requiring a letter after the capital called it unreadable.
        Regex::new(r"^\s*(?P<surname>[A-Z][\p{L}\p{N}'’\-]*)[^()]{0,300}?\((?P<year>(?:1[6-9]|20)\d{2})[a-z]?\)")
            .expect("author-year entry regex")
    })
}

fn year_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:1[6-9]|20)\d{2}").expect("year regex"))
}

/// The reference list, when it is author-year rather than numbered.
///
/// Everything after the references heading; each block is one entry. Returns
/// `(parsed, malformed_raws)` — an entry that yields no surname+year is not
/// discarded, it is REPORTED, because markers cannot resolve against it.
fn parse_author_year_entries(blocks: &[PagedBlock]) -> (Vec<AuthorYearEntry>, Vec<String>) {
    let start = blocks
        .iter()
        .position(|b| crate::ai_engine::audit_prepass::is_references_heading(&b.text));
    let Some(start) = start else { return (Vec::new(), Vec::new()) };

    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for b in blocks.iter().skip(start + 1) {
        let t = b.text.trim();
        // Too short to be a reference: a page number, a running header.
        if t.split_whitespace().count() < 5 {
            continue;
        }
        match ay_entry_re().captures(t) {
            Some(c) => ok.push(AuthorYearEntry {
                surname: c["surname"].to_lowercase(),
                year: c["year"].parse().ok(),
                raw: t.to_string(),
            }),
            None => bad.push(t.to_string()),
        }
    }
    (ok, bad)
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

/// Every (surname, year) an in-text marker refers to.
///
/// `markers_in` keeps only the FIRST work of `(A et al., 2015; B & C, 2024)`,
/// so the co-cited work would read as never cited. The raw text is split on
/// `;` to recover it.
fn marker_works(raw: &str, lead: Option<&str>, year: Option<i32>) -> Vec<(String, Option<i32>)> {
    let mut out = Vec::new();
    if let Some(l) = lead {
        out.push((l.to_string(), year));
    }
    if !raw.contains(';') {
        return out;
    }
    let inner = raw.trim_start_matches('(').trim_end_matches(')');
    for part in inner.split(';').skip(1) {
        let part = part.trim();
        let Some(c) = co_cite_lead_re().captures(part) else {
            continue;
        };
        let y = year_re().find(part).and_then(|m| m.as_str().parse::<i32>().ok());
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
                    "“{}” was matched to the entry beginning “{}” — the surnames differ by one \
                     character, so this may be a typo rather than a different work.",
                    snippet(&uses[0].1, 50),
                    snippet(&near.raw, 50)
                ),
                None,
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
                None,
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
                "“{}” cites a work the reference list does not contain — no entry begins with \
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
                "[{}] does not look like a reference entry — it names no author{}: “{}”. \
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
                    hits.entry(*n).or_default().push(snippet(&p.sentence, 60));
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
            None,
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
                "[{n}] is cited {count} time{} but the reference list has no entry {n} — it runs \
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
                Some("Renumber one of them — a cross-reference cannot say which you mean.".to_string()),
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
                None,
            );
        }
    }
}

/// (6) `A, A, B, C` — a lettered run that does not advance by one.
fn check_section_letters(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let mut run: Vec<(char, String)> = Vec::new();
    for b in blocks {
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
                "Subsection “{}. {}” follows “{}. {}” — the letters do not advance by one.",
                next, w[1].1, prev, w[0].1
            ),
            None,
        );
    }
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
                    "A paragraph appears {} times, at blocks {}: “{}”",
                    at.len(),
                    at.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" and "),
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

    #[test]
    fn a_paragraph_printed_twice_is_reported_with_both_locations() {
        let para = "A major drawback of this framework is that it has been restricted to single label \
                    problems and does not yet handle the multi label case that real corpora present.";
        let r = run(&[para, "An unrelated paragraph that says something else entirely about the data.", para]);
        let m = message(&r, "repeated-paragraph");
        assert!(m.contains("2 times"), "{m}");
        assert!(m.contains("0 and 2"), "must name where: {m}");
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
