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

/// Phrases that look like an article type in the `"<X> articles are"` pattern
/// but name no type. Without these, *"All articles are peer reviewed"* binds
/// every requirement on the page to a type called "All".
const NOT_AN_ARTICLE_TYPE: &[&str] = &[
    "all", "these", "those", "our", "such", "the", "both",
    "some", "most", "other", "your", "any", "research",
];

/// The article type a SENTENCE names, from the shape `"<Type> articles are…"`.
///
/// # Why a pattern and not a longer vocabulary — §11 D172
///
/// `article_type_of` reads the block HEADING, and over ten stored fingerprints
/// that left **41 of 43 conflicted rows UNBOUND**, of which 14 sat in sentences
/// that name their type outright:
///
/// > *"Conceptual Analysis articles are peer-reviewed, have a maximum word count
/// > of 8,000 words and may contain no more than 10…"*
/// > *"Data Reports articles are peer-reviewed, have a maximum word count of
/// > 3,000 and may contain no more than 2 Figures/Tables"*
///
/// Unbound, those Frontiers article types collapse into one bucket and the
/// conflict rule reads them as the journal contradicting itself five ways.
///
/// **The obvious fix is to add `"conceptual analysis"`, `"data report"`,
/// `"community case study"`… to [`ARTICLE_TYPES`]. That is fitting a list to the
/// rows it was measured on** — the objection that keeps `is_reviewer_guidance`
/// untouched at 5/20 — and it would still miss the next journal's vocabulary.
/// The sentence SHAPE generalises: a guideline that scopes a limit to a type
/// says so in a fixed construction, and the construction is what this reads.
///
/// Title Case is required because article types are capitalised in guidelines;
/// [`NOT_AN_ARTICLE_TYPE`] covers the capitalised-but-generic leads.
fn article_type_in_sentence(sentence: &str) -> Option<String> {
    let lower = sentence.to_lowercase();
    let at = ["articles are", "articles must", "articles should", "articles have"]
        .iter()
        .filter_map(|m| lower.find(m))
        .min()?;
    let head = sentence[..at].trim();
    if head.is_empty() || head.len() > 60 {
        return None;
    }
    // Only the trailing clause: "For submissions, Data Reports" -> "Data Reports".
    let head = head.rsplit([',', ':', ';', '.']).next().unwrap_or(head).trim();
    let words: Vec<&str> = head.split_whitespace().collect();
    if words.is_empty() || words.len() > 4 {
        return None;
    }
    if !words.iter().all(|w| w.chars().next().is_some_and(|c| c.is_uppercase())) {
        return None;
    }
    let joined = words.join(" ");
    if NOT_AN_ARTICLE_TYPE.contains(&joined.to_lowercase().as_str()) {
        return None;
    }
    Some(joined)
}

/// Does this heading clause NAME an article type, rather than merely contain
/// one? — §11 D173.
///
/// # Measured: 15 of 94 bound rows were bound by a heading that names no type
///
/// `article_type_of` substring-matched the heading, so over the ten stored
/// fingerprints:
///
/// ```text
/// "Article types"                                 -> Article        2 rows
/// "Licenses for Subscription Articles"            -> Article        1
/// "Clinical trial transparency"                   -> Clinical Trial 1
/// "Registering clinical trials"                   -> Clinical Trial 1
/// "Reviewing Study Protocols"                     -> Protocol       2
/// "Availability and peer review of computer code" -> Review         2
/// "Mandates Data Sharing and Peer Reviews Data"   -> Review         1
/// "Writing the review"                            -> Review         4
/// ```
///
/// **This is the `Harvard` defect one layer up** (§11 D172): a name matched
/// without asking what the text is about, in the heading rather than the
/// sentence. A section about *reviewing* protocols is not a requirement for
/// Protocol articles, and binding it to one scopes a rule to the wrong papers.
///
/// # The rule, and the one case it knowingly keeps
///
/// A clause names a type when it ENDS with that type — the type is the head of
/// the noun phrase — with no preposition and no leading gerund. Clauses are
/// split on `" and "` so *"Systematic reviews and meta-analyses"* still binds
/// through its first half; without that split it would be lost.
///
/// **`"Cover letter"` still binds to `Letter` (1 row).** It is a compound whose
/// modifier changes the referent, and excluding it needs a stop-word list — a
/// vocabulary fitted to one row of the corpus it was measured on. Recorded as
/// residue instead.
fn heading_names_the_type(clause: &str, ty: &str) -> bool {
    let c = clause.trim();
    if c.is_empty() {
        return false;
    }
    let words: Vec<&str> = c.split_whitespace().collect();
    if words.len() > 6 {
        return false;
    }
    // "Registering clinical trials", "Writing the review", "Reviewing Study
    // Protocols" — a gerund makes the heading about an ACTIVITY, not a type.
    if words[0].ends_with("ing") && words.len() > 1 {
        return false;
    }
    // "Licenses FOR Subscription Articles", "peer review OF computer code".
    if words.iter().any(|w| matches!(*w, "for" | "of" | "in" | "on" | "with" | "about" | "to")) {
        return false;
    }
    // The type must be the HEAD: the clause ends with it, bare or pluralised.
    c == ty || c.ends_with(ty) || c.ends_with(&format!("{ty}s")) || c.ends_with(&format!("{ty}es"))
}

fn article_type_of(heading: &str) -> Option<String> {
    let h = heading.to_lowercase();
    let mut best: Option<&str> = None;
    for t in ARTICLE_TYPES {
        if h.split(" and ").any(|clause| heading_names_the_type(clause, t))
            && best.is_none_or(|b| t.len() > b.len())
        {
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

/// **A word limit is about a PART OF THE MANUSCRIPT.** Measured, and it reached
/// the database: Nature Medicine's licensing page says
///
/// > *"In the case of text-mining, individual words, concepts and quotes up to
/// > 100 words per matching sentence may be used…"*
///
/// and that was stored as a `word_limit` of 100, in conflict with the real
/// 4,000. It is a text-mining quota in a re-use licence — a real sentence on a
/// real guidance page, saying nothing about how long a submission may be.
///
/// So a numeric word limit is admitted only when its sentence names a part of
/// the manuscript, OR its heading names an article type. The second clause is
/// not slack: `nature.com/nm/content` says *"Format Length – up to 4,000
/// words."* under the heading **Perspective**, which names no manuscript part
/// and is unambiguously a limit because of where it sits.
const MANUSCRIPT_PARTS: &[&str] = &[
    "main text", "manuscript", "article", "paper", "submission", "abstract",
    "summary", "text is", "length", "body", "contribution", "letter", "report",
];

fn is_about_the_manuscript(sentence_lower: &str, article_type: &Option<String>) -> bool {
    article_type.is_some() || MANUSCRIPT_PARTS.iter().any(|p| sentence_lower.contains(p))
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

/// Words that make a sentence a statement ABOUT REFERENCING, rather than a
/// sentence that merely contains a style's name.
///
/// **Measured — §11 D172.** Over the ten stored fingerprints, `reference_style`
/// matched the bare substring and produced 20 rows, of which 6 were wrong and
/// **5 of 8 `Harvard` rows were the word inside a proper noun**: `Harvard
/// Dataverse` in a repository list (PLOS x2), *"a Fulbright postdoctoral
/// fellowship in MGH-Harvard Medical School"* (Lancet), *"Professor of
/// Biostatistics at the Harvard T.[H. Chan]"* and *"Harvard University Boston,
/// Massachusetts, USA"* (Statistics in Medicine). The sixth was
/// *"Grant details and acknowledgments are not permitted as numbered
/// references"* — a rule about what may not BE a reference, read as a
/// declaration of reference style.
///
/// Every one of those six spans lacks any of these words. Every one of the
/// fourteen correct spans has one. **A word boundary alone does not help** —
/// `Harvard` is a whole word in all five false rows — which is why the test is
/// about what the sentence is ABOUT rather than about how the name is spelled.
const STYLE_CONTEXT: &[&str] = &["style", "referencing", "citation", "citing", "cite "];

/// Does this sentence state a reference style, as opposed to naming one?
fn states_a_reference_style(lower: &str) -> bool {
    STYLE_CONTEXT.iter().any(|w| lower.contains(w))
}

/// Whole-word containment: `needle` must not sit inside a longer alphanumeric
/// token. Guards a future needle (`"ama style"`, `"apa style"`) rather than the
/// measured defect, which was a whole word already.
fn contains_word(hay: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(i) = hay[from..].find(needle) {
        let s = from + i;
        let e = s + needle.len();
        let before_ok = s == 0 || !hay[..s].chars().next_back().is_some_and(|c| c.is_alphanumeric());
        let after_ok = e >= hay.len() || !hay[e..].chars().next().is_some_and(|c| c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        from = e;
    }
    false
}

const STANDARDS: &[&str] =
    &["CONSORT", "PRISMA", "STROBE", "ARRIVE", "TRIPOD", "CHEERS", "SPIRIT", "STARD"];

/// Reduce one sentence's numeric limits to ONE PER KIND — the binding one.
///
/// # The defect, measured — §11 D171/D172
///
/// > *"PLOS Medicine prefers abstract submissions not exceed 300 words, with a
/// > maximum of 500 words allowed."*
///
/// Two `LIMIT_LEADS` fire on that sentence — `not exceed` -> 300 and
/// `maximum of` -> 500 — and both were stored as `abstract_limit`. Same journal,
/// same kind, same (absent) article type, different values: the conflict rule
/// then marked the journal as contradicting itself, over **one sentence that
/// contradicts nothing**.
///
/// # The MAXIMUM wins, and the span keeps the rest
///
/// §7 defines a requirement as *"a rule; violation is blocking"*. Exceeding 300
/// words is **not** a violation — the journal says 500 is allowed. Exceeding 500
/// is. So 500 is the requirement; 300 is a preference, and the span carries it
/// verbatim for any reader who wants it.
///
/// **Storing the lower value would flag compliant manuscripts**, which is the
/// direction this codebase refuses (§11 D167: a check that reports a correct
/// paper as wrong is worse than none).
///
/// # Why NOT a second field
///
/// A `preferred` column beside `value` models the journal more faithfully, and
/// it was rejected on the measurement: **one sentence in 200 stored
/// requirements** across ten journals. It needs a schema change, a reliable map
/// from lead phrase to force (`prefers` vs `maximum` vs `should not exceed`),
/// and a renderer that today does not exist — D128's bar, unmet. If a corpus
/// ever shows preferred/hard pairs at real density, this doc comment is where
/// the case gets made.
///
/// # Scoped to NUMERIC limits only
///
/// A sentence naming CONSORT **and** STROBE states two requirements, not a
/// dispute — `many_reporting_standards_are_not_a_journal_contradicting_itself`
/// pins that, and it is measured here too (4 such spans over the ten journals).
/// This function never sees them: it is applied to the limit loop alone.
fn binding_limit(limits: Vec<(RequirementKind, String)>) -> Vec<(RequirementKind, String)> {
    let mut best: Vec<(RequirementKind, String)> = Vec::new();
    for (kind, value) in limits {
        match best.iter_mut().find(|(k, _)| *k == kind) {
            None => best.push((kind, value)),
            Some(slot) => {
                let a = value.parse::<u64>().ok();
                let b = slot.1.parse::<u64>().ok();
                if let (Some(a), Some(b)) = (a, b) {
                    if a > b {
                        slot.1 = value;
                    }
                } else if slot.1 != value {
                    // Unparseable: keep the first and do not guess which binds.
                    // Nothing in the corpus reaches this; it exists so a future
                    // non-numeric limit fails loudly in review rather than
                    // silently picking one.
                    debug_assert!(false, "non-numeric limit values: {} vs {value}", slot.1);
                }
            }
        }
    }
    best
}

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
        let heading_type = article_type_of(&block.heading);
        for sentence in sentences(&block.text) {
            let lower = sentence.to_lowercase();
            // HEADING FIRST — a section heading scopes everything under it and
            // is the more deliberate statement. The sentence is the fallback,
            // and it is what binds the per-type limits a heading never mentions.
            let article_type = heading_type
                .clone()
                .or_else(|| article_type_in_sentence(sentence));

            // ONE SENTENCE, ONE LIMIT PER KIND — see `binding_limit`.
            let mut limits: Vec<(RequirementKind, String)> = Vec::new();
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
                    // A count of words that is not about the manuscript is not
                    // a word limit. See `MANUSCRIPT_PARTS`.
                    if matches!(kind, RequirementKind::WordLimit | RequirementKind::AbstractLimit)
                        && !is_about_the_manuscript(&lower, &article_type)
                    {
                        continue;
                    }
                    limits.push((kind, value));
                }
            }
            for (kind, value) in binding_limit(limits) {
                push(&mut out, kind, value, &article_type, block, sentence);
            }

            // A style NAME is not a style STATEMENT — §11 D171/D172.
            for (needle, label) in REFERENCE_STYLES {
                if contains_word(&lower, needle) && states_a_reference_style(&lower) {
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

            // The same shape, generalised to the other statements a journal
            // requires (§11 D163). `SectionRequired` rather than a new kind:
            // these ARE required manuscript elements, and the checklist checks
            // them the way it checks data availability — by heading presence.
            for (needle, value, _) in REQUIRED_STATEMENTS {
                if states_a_required_statement(&lower, needle) {
                    push(&mut out, RequirementKind::SectionRequired, (*value).to_string(),
                         &article_type, block, sentence);
                    break;
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// **Statements a manuscript must CONTAIN, and the heading that satisfies each.**
///
/// `DataPolicy` above is one instance of this shape — a named statement plus a
/// modal — and reading Nature Medicine's 37 admitted-but-barren pages found
/// four more of exactly it: competing interests, funding, author contributions,
/// AI use. This is that rule generalised, not nine new patterns (§11 D163).
///
/// `(needle, canonical value, heading substring the manuscript must carry)`.
const REQUIRED_STATEMENTS: &[(&str, &str, &str)] = &[
    ("competing interest", "competing interests statement", "competing interest"),
    ("conflict of interest", "competing interests statement", "competing interest"),
    ("funding statement", "funding statement", "funding"),
    ("author contribution", "author contributions statement", "author contribution"),
    ("code availability", "code availability statement", "code availability"),
    ("ethics statement", "ethics statement", "ethic"),
    ("ethics approval", "ethics approval statement", "ethic"),
    ("informed consent", "informed consent statement", "consent"),
];

/// Words that turn an obligation into a PROHIBITION.
///
/// **The defect this exists for, caught in the scan before the rule shipped.**
/// Nature Medicine's acknowledgements page says *"The section should also NOT
/// be used to declare competing interests"* — a sentence carrying a statement
/// name and a modal, meaning the exact opposite of a requirement. Storing it
/// would be §11 D155's fabrication with a new surface: a real sentence, read
/// backwards.
const NEGATIONS: &[&str] = &[" not ", " never ", "n't ", " cannot ", " no longer ", " neither "];

/// Is this sentence a requirement to INCLUDE `needle`, rather than a
/// prohibition, a link label, or a mention?
///
/// Three conditions, each earning itself against a real false positive:
///
/// 1. **A modal.** Without one the sentence is describing, not requiring.
/// 2. **The literal word "statement" or "declaration".** This is what
///    separates a requirement from a NAVIGATION LIST: `/nm/editorial-policies`
///    concatenates its link labels into *"…should read and follow these
///    policies: Authorship Acknowledgements Funding Competing interests
///    Confidentiality…"*, which carries a modal and three statement names and
///    requires none of them. It contains no "statement".
/// 3. **No negation NEXT TO THE MODAL.** See `NEGATIONS`.
///
/// **Condition 3's window is the part that had to be measured rather than
/// guessed.** The first version looked for a negation anywhere between the
/// modal and the statement name, and it refused the single strongest true
/// positive in the corpus: *"all authors … are required to include a statement
/// at the end of their published article to declare **whether or not** they
/// have any competing interests."* That `not` sits 95 characters from the
/// modal and belongs to a different clause entirely.
///
/// A prohibition negates the MODAL — *"should not be used"*, *"must not"*,
/// *"is not required"* — so the window is the modal's own neighbourhood:
/// `NEGATION_BEFORE` characters before it and `NEGATION_AFTER` after. The
/// acknowledgements prohibition puts `not` 6 characters after `should`; the
/// competing-interests requirement puts it 95 after `required`. Nothing in the
/// corpus falls between, but the numbers are stated here rather than tuned,
/// so a case that does can be argued about against a sentence.
///
/// A genuine double negative (*"must not omit a competing interests
/// statement"*) is still refused. Failing to extract a real requirement is
/// recoverable; storing one the journal never made is not.
///
/// **THIS GUARD IS PREDICTED, NOT MEASURED, and that distinction is recorded
/// here rather than left for someone to assume either way.**
/// `examples/journal_negation_scan.rs` asks the corpus directly how many real
/// sentences name a required statement, carry the word "statement", and negate
/// their modal. Across Nature Medicine's full crawl the answer is **ZERO** —
/// the one real prohibition is stopped by the "statement"/"declaration"
/// condition before it ever reaches here. So this is a guard against a
/// sentence that other journals plausibly write (*"A competing interests
/// statement is not required for Correspondence"*) and that this corpus does
/// not contain. Re-run the scan on a new publisher before concluding it is
/// dead code; do not remove it on the strength of one journal.
const NEGATION_BEFORE: usize = 16;
const NEGATION_AFTER: usize = 24;

fn states_a_required_statement(lower: &str, needle: &str) -> bool {
    if !lower.contains(needle) {
        return false;
    }
    if !(lower.contains("statement") || lower.contains("declaration")) {
        return false;
    }
    let Some((modal_at, modal_len)) = ["must", "required", "should", "are expected to", "need to"]
        .iter()
        .filter_map(|m| lower.find(m).map(|i| (i, m.len())))
        .min()
    else {
        return false;
    };
    let lo = modal_at.saturating_sub(NEGATION_BEFORE);
    let hi = (modal_at + modal_len + NEGATION_AFTER).min(lower.len());
    // Character boundaries: the haystack is lowercased UTF-8, not ASCII.
    let lo = (0..=lo).rev().find(|i| lower.is_char_boundary(*i)).unwrap_or(0);
    let hi = (hi..=lower.len()).find(|i| lower.is_char_boundary(*i)).unwrap_or(lower.len());
    !NEGATIONS.iter().any(|n| lower[lo..hi].contains(n))
}

/// The manuscript heading that satisfies a stored statement requirement.
///
/// The checklist needs this and the extractor owns the table, so the mapping
/// lives with the table rather than being restated at the far end where it
/// could drift.
pub fn heading_for_statement(value: &str) -> Option<&'static str> {
    REQUIRED_STATEMENTS.iter().find(|(_, v, _)| *v == value).map(|(_, _, h)| *h)
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

    /// **A text-mining quota in a licence is not a word limit.** Verbatim from
    /// `nature.com/nm/editorial-policies/self-archiving-and-license-to-publish`,
    /// and it reached the database as a `word_limit` of 100 conflicting with
    /// the real 4,000.
    #[test]
    fn a_word_count_that_is_not_about_the_manuscript_is_not_a_word_limit() {
        let got = extract_requirements(&[b(
            "Wholesale re-publishing is prohibited",
            "In the case of text-mining, individual words, concepts and quotes up to 100 words              per matching sentence may be used, whereas longer paragraphs of text and images              cannot.",
        )]);
        assert!(
            got.iter().all(|r| r.kind != RequirementKind::WordLimit),
            "a licensing quota became a word limit: {got:#?}"
        );
    }

    /// The second clause is not slack. `Format Length – up to 4,000 words.`
    /// names no manuscript part and IS a limit, because its heading names an
    /// article type.
    #[test]
    fn a_limit_under_an_article_type_heading_needs_no_manuscript_noun() {
        let got = extract_requirements(&[b("Perspective", "Format Length – up to 4,000 words.")]);
        assert_eq!(got.len(), 1, "{got:#?}");
        assert_eq!(got[0].kind, RequirementKind::WordLimit);
        assert_eq!(got[0].article_type.as_deref(), Some("Perspective"));

        // …and the same sentence with no heading at all still counts, because
        // "Length" is itself a manuscript part.
        assert!(!extract_requirements(&[b("", "Format Length – up to 4,000 words.")]).is_empty());
        // But a bare count under a non-article heading does not.
        assert!(extract_requirements(&[b(
            "Permissions",
            "Reuse of up to 250 words per request is permitted.",
        )])
        .iter()
        .all(|r| r.kind != RequirementKind::WordLimit));
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
    // --- the generalised required-statement rule (§11 D163) ----------------
    //
    // Every case below is a REAL sentence from Nature Medicine's crawl, and the
    // three negatives are the false positives the scan surfaced before the rule
    // shipped.

    #[test]
    fn a_required_statement_is_extracted() {
        for (sentence, expected) in [
            ("A competing interests statement is required.", "competing interests statement"),
            ("An author contributions statement is required.", "author contributions statement"),
            (
                "Any relevant funding should be declared in a separate funding statement.",
                "funding statement",
            ),
            (
                "If custom code was used in the study, a separate Code Availability Statement \
                 must also be provided.",
                "code availability statement",
            ),
            (
                "In addition to any declarations in submission systems or forms, all authors \
                 regardless of peer review model are required to include a statement at the end \
                 of their published article to declare whether or not they have any competing \
                 interests.",
                "competing interests statement",
            ),
        ] {
            let out = extract_requirements(&[b("Policy", sentence)]);
            assert!(
                out.iter().any(|r| r.kind == RequirementKind::SectionRequired
                    && r.value == expected),
                "{sentence:?} did not yield {expected:?}: {out:#?}"
            );
        }
    }

    /// **A PROHIBITION IS NOT A REQUIREMENT** (§11 D155's family).
    ///
    /// The real sentence that prompted this — Nature Medicine's *"The section
    /// should also not be used to declare competing interests"* — is in the
    /// second case below, and it is NOT what exercises the negation guard: it
    /// carries no "statement"/"declaration", so the navigation-list guard
    /// rejects it first. Removing the negation guard entirely left this test
    /// GREEN, which is how that was found.
    ///
    /// The first case is the one that reaches the guard, and it is
    /// CONSTRUCTED. See `states_a_required_statement`: zero sentences in the
    /// measured corpus get that far.
    #[test]
    fn a_prohibition_is_not_read_as_a_requirement() {
        for sentence in [
            // Reaches the negation guard — nothing earlier rejects it.
            "A competing interests statement is not required for Correspondence.",
            // Real, but stopped one guard earlier.
            "The section should also not be used to declare competing interests, including any \
             related to funding sources that may gain or lose financially through this \
             publication.",
        ] {
            let out = extract_requirements(&[b("Acknowledgements", sentence)]);
            assert!(
                out.iter().all(|r| r.kind != RequirementKind::SectionRequired),
                "a prohibition was stored as a requirement: {sentence:?} -> {out:#?}"
            );
        }
    }

    /// A page's NAVIGATION concatenated into one sentence carries a modal and
    /// three statement names and requires none of them.
    #[test]
    fn a_navigation_list_of_policy_names_is_not_a_requirement() {
        let out = extract_requirements(&[b(
            "Editorial Policies",
            "BEFORE SUBMISSION All prospective authors should read and follow these policies: \
             Authorship Acknowledgements Funding Competing interests Confidentiality \
             Corrections, Retractions and Matters Arising Research Ethics Image integrity",
        )]);
        assert!(
            out.iter().all(|r| r.kind != RequirementKind::SectionRequired),
            "a link list was stored as a requirement: {out:#?}"
        );
    }

    /// Naming a statement is not requiring one.
    #[test]
    fn a_bare_mention_of_a_statement_is_not_a_requirement() {
        let out = extract_requirements(&[b(
            "Funding",
            "Our funding statement policy page explains how declarations are displayed.",
        )]);
        assert!(
            out.iter().all(|r| r.kind != RequirementKind::SectionRequired),
            "a mention was stored as a requirement: {out:#?}"
        );
    }

    // ---------------------------------------------------------------------
    // §11 D172 — the three extractor defects Stage 2 exposed
    // ---------------------------------------------------------------------

    /// **A style NAME is not a style STATEMENT.** Every string here is a real
    /// span from the ten stored fingerprints — five `Harvard` proper nouns that
    /// were stored as a reference style, and the three that genuinely state one.
    #[test]
    fn a_style_name_inside_a_proper_noun_is_not_a_reference_style() {
        let wrong = [
            "Dryad Digital Repository Dutch national centre of expertise and repository for \
             research data (DANS) figshare Harvard Dataverse Network Kaggle",
            "After a Fulbright postdoctoral fellowship in MGH-Harvard Medical School, I joined \
             King's College London as a research fellow.",
            "Prior to joining NYU, she was Professor of Biostatistics at the Harvard T.",
            "Harvard University Boston, Massachusetts, USA",
            "Grant details and acknowledgments are not permitted as numbered references.",
        ];
        for s in wrong {
            let got = extract_requirements(&[b("h", s)]);
            assert!(
                !got.iter().any(|r| r.kind == RequirementKind::ReferenceStyle),
                "named a style from a sentence that states none: {s:?} -> {got:#?}"
            );
        }

        let right = [
            ("Many Frontiers journals use the Harvard referencing system;", "Harvard"),
            ("The journal follows the Sage Harvard reference style.", "Harvard"),
            ("All PLOS journals utilize the Vancouver reference style.", "Vancouver"),
            ("PLOS uses \u{201c}Vancouver\u{201d} style, as outlined in the ICMJE sample references.", "Vancouver"),
        ];
        for (s, want) in right {
            let got = extract_requirements(&[b("h", s)]);
            assert!(
                got.iter().any(|r| r.kind == RequirementKind::ReferenceStyle && r.value == want),
                "missed a real style statement: {s:?} -> {got:#?}"
            );
        }
    }

    /// **One sentence, one limit per kind — the binding one.**
    ///
    /// The PLOS Medicine span, verbatim. Before this, `not exceed` and
    /// `maximum of` each produced a row and the two were marked as the journal
    /// contradicting itself.
    #[test]
    fn a_preferred_and_a_hard_limit_in_one_sentence_are_one_requirement() {
        let got = extract_requirements(&[b(
            "Abstract",
            "PLOS Medicine prefers abstract submissions not exceed 300 words, with a maximum \
             of 500 words allowed.",
        )]);
        let abs: Vec<&ExtractedRequirement> =
            got.iter().filter(|r| r.kind == RequirementKind::AbstractLimit).collect();
        assert_eq!(abs.len(), 1, "one sentence, one abstract limit: {abs:#?}");
        assert_eq!(
            abs[0].value, "500",
            "the BLOCKING value wins — exceeding 300 is not a violation, the journal allows 500"
        );
        // The preference is not lost; it is in the span, which is where a
        // reader can see both numbers and neither is invented.
        assert!(abs[0].source_span.contains("300"), "{:?}", abs[0].source_span);
        assert!(abs[0].source_span.contains("500"), "{:?}", abs[0].source_span);
    }

    /// Two STANDARDS in one sentence remain two requirements — the reduction
    /// above is scoped to numeric limits and must not touch them.
    #[test]
    fn two_standards_in_one_sentence_are_still_two_requirements() {
        let got = extract_requirements(&[b(
            "Review",
            "Does the study conform to any relevant guidelines such as CONSORT, STROBE, and \
             the Fort Lauderdale agreement?",
        )]);
        let n = got.iter().filter(|r| r.kind == RequirementKind::ReportingStandard).count();
        assert_eq!(n, 2, "CONSORT and STROBE are two requirements, not a dispute: {got:#?}");
    }

    /// **A limit scoped to an article type in its own SENTENCE binds to it.**
    ///
    /// NOTE the fixture heading: `"Submission formats"`, not `"Article types"`.
    /// The obvious heading binds every row under it to a type called `Article`,
    /// because `article_type_of` substring-matches `"article"` — §11 D172's
    /// FOURTH defect, found by writing this test and deliberately not fixed
    /// here. A fixture that tripped it would have measured the heading matcher
    /// while claiming to measure the sentence binder.
    ///
    /// Frontiers states five per-type limits this way and the heading names
    /// none of them. Unbound, they collapse into one bucket and the conflict
    /// rule reads the journal as contradicting itself five ways — 19 of the 43
    /// conflicted rows over the ten fingerprints. Real spans, verbatim.
    #[test]
    fn a_type_named_in_the_sentence_binds_the_limit_to_it() {
        let cases = [
            ("Conceptual Analysis articles are peer-reviewed, have a maximum word count of \
              8,000 words and may contain no more than 10 Figures/Tables.", "Conceptual Analysis", "10"),
            ("Data Reports articles are peer-reviewed, have a maximum word count of 3,000 and \
              may contain no more than 2 Figures/Tables.", "Data Reports", "2"),
            ("Brief Research Reports articles are peer-reviewed, have a maximum word count of \
              4,000 and may contain no more than 4 Figures/Tables.", "Brief Research Reports", "4"),
        ];
        for (span, want_type, want_value) in cases {
            let got = extract_requirements(&[b("Submission formats", span)]);
            let f = got
                .iter()
                .find(|r| r.kind == RequirementKind::FigureLimit)
                .unwrap_or_else(|| panic!("no figure limit from {span:?} -> {got:#?}"));
            assert_eq!(f.value, want_value, "{span:?}");
            assert_eq!(
                f.article_type.as_deref(),
                Some(want_type),
                "the sentence names its type and the binding must use it: {got:#?}"
            );
        }
    }

    /// **The heading still wins.** A section heading scopes everything under it
    /// and is the more deliberate statement; the sentence is the fallback.
    #[test]
    fn a_heading_type_outranks_a_sentence_type() {
        let got = extract_requirements(&[b(
            "Brief Communication",
            "Review articles are peer-reviewed and may contain no more than 8 Figures/Tables.",
        )]);
        let f = got.iter().find(|r| r.kind == RequirementKind::FigureLimit).expect("a limit");
        assert_eq!(f.article_type.as_deref(), Some("Brief Communication"), "{got:#?}");
    }

    /// **A generic lead is not a type.** Without the stop-list, this binds every
    /// requirement on the page to an article type called "All".
    #[test]
    fn a_generic_lead_does_not_become_an_article_type() {
        for span in [
            "All articles are peer reviewed and may contain no more than 6 Figures/Tables.",
            "These articles are limited to 4 Figures/Tables.",
        ] {
            let got = extract_requirements(&[b("h", span)]);
            let f = got.iter().find(|r| r.kind == RequirementKind::FigureLimit).expect("a limit");
            assert_eq!(f.article_type, None, "a generic lead must stay UNBOUND: {span:?}");
        }
    }

    /// **The residue, pinned so it is not mistaken for a gap nobody noticed.**
    ///
    /// *"These are capped at 12,000 words…"* refers back to a previous sentence
    /// and names no type. 5 of the 19 Frontiers rows read like this, and they
    /// stay unbound deliberately: resolving the antecedent is an anaphora
    /// problem, and guessing one would bind a limit to the wrong type — worse
    /// than leaving it unscoped, which at least renders as "not stated".
    #[test]
    fn a_sentence_referring_back_stays_unbound_rather_than_guessing() {
        let got = extract_requirements(&[b(
            "Submission formats",
            "These are capped at 12,000 words and may include up to 15 figures or tables.",
        )]);
        let f = got.iter().find(|r| r.kind == RequirementKind::FigureLimit).expect("a limit");
        assert_eq!(f.article_type, None, "{got:#?}");
    }

    /// **A heading that CONTAINS a type name does not always NAME one** —
    /// §11 D173. Every heading here is real, from the ten stored fingerprints.
    #[test]
    fn a_heading_about_a_topic_does_not_bind_its_requirements_to_a_type() {
        let wrong = [
            "Article types",
            "Licenses for Subscription Articles",
            "Clinical trial transparency",
            "Registering clinical trials",
            "Reviewing Study Protocols",
            "Availability and peer review of computer code and algorithm",
            "Mandates Data Sharing and Peer Reviews Data",
            "Writing the review",
        ];
        for h in wrong {
            let got = extract_requirements(&[b(h, "Manuscripts must not exceed 4,000 words.")]);
            let r = got.first().expect("a word limit");
            assert_eq!(
                r.article_type, None,
                "{h:?} is a topic, not an article type — binding scopes the rule to the \
                 wrong papers: {got:#?}"
            );
        }

        let right = [
            ("Article", "Article"),
            ("Brief Communication", "Brief Communication"),
            ("Clinical Trials", "Clinical Trial"),
            ("Study Protocol", "Protocol"),
            ("Mini Review", "Review"),
            ("Systematic reviews and meta-analyses", "Systematic Review"),
            ("Matters Arising", "Matters Arising"),
            ("Perspectives", "Perspective"),
        ];
        for (h, want) in right {
            let got = extract_requirements(&[b(h, "Manuscripts must not exceed 4,000 words.")]);
            let r = got.first().expect("a word limit");
            assert_eq!(r.article_type.as_deref(), Some(want), "{h:?} names a type: {got:#?}");
        }
    }

    /// **The residue, pinned rather than special-cased.** `"Cover letter"` still
    /// binds to `Letter`: it is a compound whose modifier changes the referent,
    /// and excluding it needs a stop-word fitted to one row of the corpus it was
    /// measured on. This test exists so the state is a recorded decision, and it
    /// goes red the day someone fixes it — at which point delete it.
    #[test]
    fn cover_letter_still_binds_to_letter_and_that_is_known() {
        let got = extract_requirements(&[b("Cover letter", "Manuscripts must not exceed 500 words.")]);
        assert_eq!(got.first().expect("a limit").article_type.as_deref(), Some("Letter"));
    }

}
