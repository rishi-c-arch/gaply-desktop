//! **Requirement extraction from guideline pages — deterministic first.**
//!
//! Prompt 5 item 2: *"Extraction is deterministic-first: word limits, section
//! names, reference styles, reporting-standard names, ethics-statement
//! requirements are pattern matches. What patterns cannot reach goes to the
//! local 3B and carries confidence: judged. Measure the split per journal."*
//!
//! **This module is the deterministic half, and only that.** No model, no
//! network, no I/O — text in, requirements out. What it cannot reach it does
//! not guess at; the model half is a later change and is not smuggled in here
//! as a heuristic wearing a pattern's clothes.
//!
//! # Every requirement quotes the sentence it came from
//!
//! §3.4: *"every extracted requirement records its `source_document`,
//! `source_heading` and `source_span` — the exact sentence — not just a URL."*
//! The schema enforces it (`migrations` v23: a `verified` row without a span is
//! unstorable), and this module is the producer that has to satisfy it.
//!
//! # Article-type binding comes from the HEADING, and it has to
//!
//! Nature Medicine's `/nm/content` states every limit it has under an
//! article-type heading:
//!
//! ```text
//! Article
//!   Main text – up to 4,000 words, excluding abstract, Methods, references…
//!   Abstract – up to 150 words, unreferenced.
//! Brief Communication
//!   Main text – up to 2,000 words, including abstract.
//! ```
//!
//! The numbers are identical in shape and mean different things. Extracting
//! them without their heading produces a journal that requires 4,000 words and
//! 2,000 words at once — which is not a conflict to report, it is a parse that
//! threw away the distinguishing fact.
//!
//! A heading that names no article type leaves `article_type` ABSENT rather
//! than defaulting to "the journal" — §6b.3's refusal-over-inference rule in a
//! second place.
//!
//! **[MEASURED — Nature Medicine, 14 Sep 2026.]** 120 pages crawled, 51
//! classified as guideline content, and of those:
//!
//! | | |
//! |---|---:|
//! | pages yielding ≥ 1 requirement by pattern | **12** |
//! | pages yielding ZERO | **39** |
//! | requirements extracted | **68** |
//! | of which bound to an article type | **25** |
//!
//! By kind: 23 word limits, 15 reporting standards, 11 figure limits, 11 data
//! policies, 4 abstract limits, 3 reference limits, 1 reference style.
//!
//! **The 39 are mostly the MODEL's future work, not the gate's mistakes**, and
//! that distinction is the whole point of measuring rather than guessing. Of
//! the first 20 listed, **15 sit under the journal's own `/submission-guidelines/`
//! or `/editorial-policies/` paths** — authorship criteria, competing
//! interests, image integrity, ORCID — real author requirements stated as
//! prose. Only five are not the journal's guidance at all (the homepage, a
//! news index, a paid editing service).
//!
//! That is the number `guidelines::classify_page`'s precision should be
//! revisited on, and on this evidence tuning it would have cost real pages to
//! remove little noise.
//!
//! **A known gap, visible rather than silent.** `Analysis` and `Resource` are
//! Nature article types and are not in [`ARTICLE_TYPES`], so their 4,000-word
//! limits come out with `article_type: None` while `source_heading` reads
//! "Analysis". The binding is ABSENT and the heading is recorded beside it, so
//! a reader can see what was not bound — unlike the crawl lexicon, whose
//! failure produced nothing at all. Extending the list is a data change; the
//! requirement is not lost meanwhile.

use serde::{Deserialize, Serialize};

/// A run of guideline text under one heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuidelineBlock {
    /// The nearest enclosing heading, as written. Empty when the page has none.
    pub heading: String,
    pub text: String,
}

/// Mirrors the `kind` CHECK on `journal_requirements` — the vocabulary lives at
/// the schema (§11 D110) and this enum must not drift from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementKind {
    WordLimit,
    AbstractLimit,
    SectionRequired,
    ReferenceStyle,
    ReferenceLimit,
    DataPolicy,
    ReportingStandard,
    FigureLimit,
    Other,
}

impl RequirementKind {
    /// **Is a journal allowed only ONE value of this kind per article type?**
    ///
    /// This decides what counts as a conflict, and getting it wrong the first
    /// time produced a spectacular false positive: Nature Medicine's six
    /// reporting standards — CONSORT for trials, PRISMA for systematic reviews,
    /// STROBE for observational studies, STARD for biomarkers, TRIPOD for
    /// prediction models, ARRIVE for animal work — were stored as ONE
    /// CONFLICTED FACT, as though the journal could not make up its mind.
    ///
    /// The spans said otherwise in plain English: *"Observational studies …
    /// must be reported according to the STROBE"*, *"Systematic reviews and
    /// meta-analyses must follow the PRISMA guidelines."* A journal binds many
    /// standards, each to a design, and a second one does not contradict the
    /// first. The same is true of data policies.
    ///
    /// A word limit is different: one article type has one. Two different
    /// values for it IS a disagreement, and that is the case Prompt 5's
    /// conflict rule is for.
    pub fn is_single_valued(&self) -> bool {
        match self {
            RequirementKind::WordLimit
            | RequirementKind::AbstractLimit
            | RequirementKind::FigureLimit
            | RequirementKind::ReferenceLimit
            | RequirementKind::ReferenceStyle => true,
            // Many per journal, by design.
            RequirementKind::ReportingStandard
            | RequirementKind::DataPolicy
            | RequirementKind::SectionRequired
            | RequirementKind::Other => false,
        }
    }

    /// The exact string the schema's CHECK accepts.
    pub fn as_str(&self) -> &'static str {
        match self {
            RequirementKind::WordLimit => "word_limit",
            RequirementKind::AbstractLimit => "abstract_limit",
            RequirementKind::SectionRequired => "section_required",
            RequirementKind::ReferenceStyle => "reference_style",
            RequirementKind::ReferenceLimit => "reference_limit",
            RequirementKind::DataPolicy => "data_policy",
            RequirementKind::ReportingStandard => "reporting_standard",
            RequirementKind::FigureLimit => "figure_limit",
            RequirementKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ExtractedRequirement {
    pub kind: RequirementKind,
    /// The value as a string — `"4000"`, `"Vancouver"`, `"CONSORT"`.
    pub value: String,
    /// The article type this is bound to, when the heading names one.
    /// **`None` means "not stated", never "applies to everything".**
    pub article_type: Option<String>,
    pub source_heading: String,
    /// The exact sentence. Never empty.
    pub source_span: String,
}

/// Article types a heading may name. Longest match wins, so
/// "Brief Communication" is not read as "Communication".
const ARTICLE_TYPES: &[&str] = &[
    "brief communication", "short communication", "research article",
    "original research", "systematic review", "case report", "clinical trial",
    "matters arising", "review article", "perspective", "commentary", "editorial",
    "correspondence", "letter", "article", "review", "protocol",
];

fn article_type_of(heading: &str) -> Option<String> {
    let h = heading.to_lowercase();
    let mut best: Option<&str> = None;
    for t in ARTICLE_TYPES {
        if h.contains(t) && best.is_none_or(|b| t.len() > b.len()) {
            best = Some(t);
        }
    }
    best.map(|t| {
        t.split(' ')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// Split into sentences, keeping each one whole so it can be quoted.
///
/// Guideline pages are full of `4,000`, so a naive split on `.` fragments the
/// very spans this exists to quote. A period between two digits is not a
/// boundary.
fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, c) in text.char_indices() {
        if !matches!(c, '.' | '!' | '?' | ';' | '\n') {
            continue;
        }
        let next = text[i + c.len_utf8()..].chars().next();
        let prev = text[..i].chars().last();
        if c == '.'
            && prev.is_some_and(|p| p.is_ascii_digit())
            && next.is_some_and(|n| n.is_ascii_digit())
        {
            continue;
        }
        let end = i + c.len_utf8();
        let s = text[start..end].trim();
        if s.chars().count() > 3 {
            out.push(s);
        }
        start = end;
    }
    let tail = text[start..].trim();
    if tail.chars().count() > 3 {
        out.push(tail);
    }
    out
}

/// Digits immediately after a lead phrase. A long gap means the number belongs
/// to something else — "up to the limit described in section 4" is not a limit
/// of four.
fn number_after(s: &str, at: usize) -> Option<String> {
    let rest = &s[at..];
    let start = rest.find(|c: char| c.is_ascii_digit())?;
    if rest[..start].chars().filter(|c| !c.is_whitespace()).count() > 2 {
        return None;
    }
    let digits: String = rest[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .filter(|c| c.is_ascii_digit())
        .collect();
    (!digits.is_empty()).then_some(digits)
}

const LIMIT_LEADS: &[&str] = &[
    "up to", "no more than", "maximum of", "a maximum", "not exceed", "limited to",
    "must not exceed", "should not exceed", "fewer than", "at most",
];

const REFERENCE_STYLES: &[(&str, &str)] = &[
    ("vancouver", "Vancouver"),
    ("harvard", "Harvard"),
    ("apa style", "APA"),
    ("ama style", "AMA"),
    ("numbered reference", "numbered"),
    ("author-date", "author-date"),
];

const STANDARDS: &[&str] =
    &["CONSORT", "PRISMA", "STROBE", "ARRIVE", "TRIPOD", "CHEERS", "SPIRIT", "STARD"];

/// Which unit a limit applies to, and therefore which kind it is.
fn limit_kind(lower: &str, number_at: usize) -> Option<RequirementKind> {
    let window: String = lower[number_at..].chars().take(60).collect();
    for (unit, kind) in [
        ("words", RequirementKind::WordLimit),
        ("figures", RequirementKind::FigureLimit),
        ("tables", RequirementKind::FigureLimit),
        ("display items", RequirementKind::FigureLimit),
        // `Display items – up to 6 items` puts the qualifier BEFORE the number
        // and the bare noun after it, so the window that follows says only
        // "items". Measured on nature.com/nm/content.
        ("items", RequirementKind::FigureLimit),
        ("references", RequirementKind::ReferenceLimit),
    ] {
        if window.contains(unit) {
            return Some(kind);
        }
    }
    None
}

/// Extract every requirement a pattern can reach. **Pure.**
pub fn extract_requirements(blocks: &[GuidelineBlock]) -> Vec<ExtractedRequirement> {
    let mut out = Vec::new();
    for block in blocks {
        let article_type = article_type_of(&block.heading);
        for sentence in sentences(&block.text) {
            let lower = sentence.to_lowercase();

            for lead in LIMIT_LEADS {
                let mut from = 0usize;
                while let Some(i) = lower[from..].find(lead) {
                    let at = from + i + lead.len();
                    from = at;
                    let Some(value) = number_after(&lower, at) else { continue };
                    let Some(kind) = limit_kind(&lower, at) else { continue };
                    // An abstract limit is a word limit under a different name,
                    // and conflating them loses which the journal meant.
                    let kind = if kind == RequirementKind::WordLimit
                        && lower[..at].contains("abstract")
                    {
                        RequirementKind::AbstractLimit
                    } else {
                        kind
                    };
                    push(&mut out, kind, value, &article_type, block, sentence);
                }
            }

            for (needle, label) in REFERENCE_STYLES {
                if lower.contains(needle) {
                    push(&mut out, RequirementKind::ReferenceStyle, (*label).to_string(),
                         &article_type, block, sentence);
                }
            }

            for s in STANDARDS {
                if sentence.contains(s) {
                    push(&mut out, RequirementKind::ReportingStandard, (*s).to_string(),
                         &article_type, block, sentence);
                }
            }

            if (lower.contains("data availability") || lower.contains("data-availability"))
                && (lower.contains("must") || lower.contains("required") || lower.contains("should"))
            {
                push(&mut out, RequirementKind::DataPolicy,
                     "data availability statement required".to_string(),
                     &article_type, block, sentence);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn push(
    out: &mut Vec<ExtractedRequirement>,
    kind: RequirementKind,
    value: String,
    article_type: &Option<String>,
    block: &GuidelineBlock,
    sentence: &str,
) {
    let span = sentence.trim();
    if span.is_empty() {
        return;
    }
    out.push(ExtractedRequirement {
        kind,
        value,
        article_type: article_type.clone(),
        source_heading: block.heading.clone(),
        source_span: span.chars().take(400).collect(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(heading: &str, text: &str) -> GuidelineBlock {
        GuidelineBlock { heading: heading.into(), text: text.into() }
    }

    /// **`nature.com/nm/content`, verbatim.** The page carrying every
    /// extractable Nature Medicine requirement, and the reason the crawler was
    /// inverted. Two article types, four numbers, all identical in shape.
    #[test]
    fn the_nature_medicine_article_type_page_binds_each_limit_to_its_type() {
        let blocks = vec![
            b("Article", "Format Main text – up to 4,000 words, excluding abstract, Methods, \
                references and figure legends. Abstract – up to 150 words, unreferenced. \
                Display items – up to 6 items. References – up to 50 references."),
            b("Brief Communication", "Format Abstract – up to 150 words, unreferenced. \
                Main text – up to 2,000 words, including abstract. Display items – up to 2 items. \
                References – up to 10 references."),
        ];
        let got = extract_requirements(&blocks);

        let word_limits: Vec<(&str, Option<&str>)> = got
            .iter()
            .filter(|r| r.kind == RequirementKind::WordLimit)
            .map(|r| (r.value.as_str(), r.article_type.as_deref()))
            .collect();
        assert!(word_limits.contains(&("4000", Some("Article"))), "{word_limits:?}");
        assert!(word_limits.contains(&("2000", Some("Brief Communication"))), "{word_limits:?}");

        // **An abstract limit is not a word limit**, or the journal appears to
        // require 150 words of main text.
        let abstracts: Vec<&str> = got
            .iter()
            .filter(|r| r.kind == RequirementKind::AbstractLimit)
            .map(|r| r.value.as_str())
            .collect();
        assert_eq!(abstracts, vec!["150", "150"], "{got:#?}");
        assert!(!word_limits.iter().any(|(v, _)| *v == "150"), "{word_limits:?}");

        // Display items and references, each bound.
        assert!(got.iter().any(|r| r.kind == RequirementKind::FigureLimit
            && r.value == "6"
            && r.article_type.as_deref() == Some("Article")));
        assert!(got.iter().any(|r| r.kind == RequirementKind::ReferenceLimit
            && r.value == "10"
            && r.article_type.as_deref() == Some("Brief Communication")));

        // Every requirement quotes its sentence.
        assert!(got.iter().all(|r| !r.source_span.is_empty()));
        assert!(got
            .iter()
            .find(|r| r.value == "4000")
            .unwrap()
            .source_span
            .contains("excluding abstract"));
    }

    /// **The failure this binding exists to prevent**, stated as a test: the
    /// same two pages with the headings thrown away produce a journal that
    /// requires 4,000 and 2,000 words at once, with nothing to tell them apart.
    #[test]
    fn without_its_heading_a_limit_cannot_be_told_from_another_article_types_limit() {
        let flat = vec![b(
            "",
            "Main text – up to 4,000 words. Main text – up to 2,000 words.",
        )];
        let got = extract_requirements(&flat);
        let vals: Vec<&str> = got.iter().map(|r| r.value.as_str()).collect();
        assert_eq!(vals.len(), 2);
        // Both present, both UNBOUND — and the engine says so rather than
        // picking one or averaging them.
        assert!(got.iter().all(|r| r.article_type.is_none()), "{got:#?}");
    }

    /// A heading that names no article type leaves the binding ABSENT.
    #[test]
    fn a_heading_that_names_no_article_type_binds_nothing() {
        let got = extract_requirements(&[b(
            "Formatting your submission",
            "The main text is limited to 3,500 words.",
        )]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, RequirementKind::WordLimit);
        assert_eq!(got[0].value, "3500");
        assert_eq!(got[0].article_type, None, "absent, never 'the journal'");
    }

    #[test]
    fn the_longest_article_type_phrase_wins() {
        assert_eq!(article_type_of("Brief Communication"), Some("Brief Communication".into()));
        assert_eq!(article_type_of("Research Article"), Some("Research Article".into()));
        assert_eq!(article_type_of("Submission guidelines"), None);
    }

    /// `4,000` must not be split into two sentences, or the span it is quoted
    /// from is a fragment.
    #[test]
    fn a_thousands_separator_does_not_end_a_sentence() {
        let s = sentences("Main text – up to 4,000 words. Abstract – up to 150 words.");
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s[0].contains("4,000"));
    }

    /// **A number far from its lead phrase is not that lead's limit.**
    #[test]
    fn a_distant_number_is_not_read_as_a_limit() {
        let got = extract_requirements(&[b(
            "",
            "Manuscripts are limited to the length described in section 4 of this page.",
        )]);
        assert!(got.is_empty(), "{got:#?}");
    }

    /// PLOS ONE, verbatim: it has no length limit, and the extractor must not
    /// invent one from a sentence that says so.
    #[test]
    fn a_journal_with_no_length_limit_yields_no_word_limit() {
        let got = extract_requirements(&[b(
            "Submission Guidelines",
            "There are no explicit length restrictions for the full text of PLOS ONE submissions. \
             The abstract must not exceed 300 words.",
        )]);
        let words: Vec<&ExtractedRequirement> =
            got.iter().filter(|r| r.kind == RequirementKind::WordLimit).collect();
        assert!(words.is_empty(), "{words:#?}");
        assert!(got
            .iter()
            .any(|r| r.kind == RequirementKind::AbstractLimit && r.value == "300"));
    }

    #[test]
    fn reference_styles_and_reporting_standards_are_pattern_matches() {
        let got = extract_requirements(&[b(
            "References",
            "References must follow the Vancouver style. Randomised trials must be reported \
             according to CONSORT; systematic reviews follow PRISMA.",
        )]);
        assert!(got.iter().any(|r| r.kind == RequirementKind::ReferenceStyle
            && r.value == "Vancouver"));
        let standards: Vec<&str> = got
            .iter()
            .filter(|r| r.kind == RequirementKind::ReportingStandard)
            .map(|r| r.value.as_str())
            .collect();
        assert!(standards.contains(&"CONSORT") && standards.contains(&"PRISMA"), "{standards:?}");
    }

    /// A data-availability page that merely MENTIONS the phrase is not a
    /// requirement; one that obliges is.
    #[test]
    fn a_data_policy_needs_an_obligation_not_just_the_phrase() {
        let mention = extract_requirements(&[b("Data", "Our data availability page explains more.")]);
        assert!(mention.iter().all(|r| r.kind != RequirementKind::DataPolicy), "{mention:#?}");
        let obliged = extract_requirements(&[b(
            "Data",
            "Authors must include a data availability statement with every submission.",
        )]);
        assert!(obliged.iter().any(|r| r.kind == RequirementKind::DataPolicy));
    }

    /// The enum's strings must be exactly what the schema's CHECK accepts, or a
    /// row that passes here is rejected at insert.
    #[test]
    fn the_kind_strings_match_the_schema_check_exactly() {
        let schema = include_str!("migrations.rs");
        let check = schema
            .split("kind            TEXT NOT NULL CHECK (kind IN (")
            .nth(1)
            .expect("the journal_requirements CHECK");
        let check: String = check.chars().take(400).collect();
        for k in [
            RequirementKind::WordLimit,
            RequirementKind::AbstractLimit,
            RequirementKind::SectionRequired,
            RequirementKind::ReferenceStyle,
            RequirementKind::ReferenceLimit,
            RequirementKind::DataPolicy,
            RequirementKind::ReportingStandard,
            RequirementKind::FigureLimit,
            RequirementKind::Other,
        ] {
            assert!(
                check.contains(&format!("'{}'", k.as_str())),
                "`{}` is not in the schema CHECK",
                k.as_str()
            );
        }
    }
}
