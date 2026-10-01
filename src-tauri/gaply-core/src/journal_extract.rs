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
    // "papers" beside "articles" (§11 D239): BMJ writes "Analysis papers should
    // be 2000 words", the same construction with the other noun.
    let at = [
        "articles are", "articles must", "articles should", "articles have",
        "papers are", "papers must", "papers should", "papers have",
    ]
    .iter()
    .filter_map(|m| lower.find(m))
    .min()?;
    let head = sentence[..at].trim();
    if head.is_empty() || head.len() > 60 {
        return None;
    }
    // Only the trailing clause: "For submissions, Data Reports" -> "Data Reports".
    let clause = head.rsplit([':', ';', '.']).next().unwrap_or(head).trim();
    // **A list NAME is one type (§11 D239).** "Curriculum, Instruction, and
    // Pedagogy" split on the comma left "and Pedagogy", which fails the
    // capitalisation test, so the limit lost its type. When every word of the
    // clause is capitalised apart from the connective, the commas are part of
    // the name; otherwise the comma still separates a lead-in from the type.
    let is_list_name = clause.contains(',')
        && clause
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|w| !w.is_empty() && !matches!(*w, "and" | "&"))
            .all(|w| w.chars().next().is_some_and(|c| c.is_uppercase()));
    let head = if is_list_name { clause } else { clause.rsplit(',').next().unwrap_or(clause).trim() };
    let words: Vec<&str> = head.split_whitespace().collect();
    let content = words.iter().filter(|w| !matches!(**w, "and" | "&")).count();
    if content == 0 || content > 4 {
        return None;
    }
    if !words
        .iter()
        .all(|w| matches!(*w, "and" | "&") || w.chars().next().is_some_and(|c| c.is_uppercase()))
    {
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
    // "Double anonymized peer review", "Single-blind peer review", "Peer
    // review": the peer-review PROCESS, not the article type Review. §11 D244.
    // Elsevier's guide template carries the first under every journal that
    // uses it, so this one heading typed a competing-interests row as a Review
    // requirement on 8 of 20 stage-1 journals. Measured over every heading on
    // 56 guide pages and in the seed: 8 headings end in "peer review", all of
    // them the process; the 4 that keep "Review" (Review, Mini Review, Book
    // Reviews, New Media Reviews) never follow "peer".
    if words.len() >= 2 && words[words.len() - 2] == "peer" {
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
        if c == '.' && inside_latin_abbreviation(text, i) {
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

/// Is the period at `i` one of the periods of "e.g." or "i.e."? §11 D242.
///
/// **Measured on the Journal of Coordination Chemistry's guide.** *"Please
/// supply a short biographical note for each author. This could be adapted
/// from your departmental website … and should be relatively brief (e.g. no
/// more than 200 words)."* Split at both periods of "e.g.", the last sentence
/// reached the extractor as *"no more than 200 words)."*: a fragment that no
/// longer said the 200 words were a biographical note's, or that they were an
/// example. Under the heading "Review Articles" it was stored as a 200-word
/// limit on review articles.
fn inside_latin_abbreviation(text: &str, i: usize) -> bool {
    let start = text[..i]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace() || *c == '(')
        .map_or(0, |(p, c)| p + c.len_utf8());
    let token: String = text[start..].chars().take(4).collect::<String>().to_lowercase();
    if matches!(token.as_str(), "e.g." | "i.e.") && i < start + 4 {
        return true;
    }
    // "eg." without its first period (§11 D256): Archives PM&R writes "(eg.
    // CONSORT, PRISMA, etc.)", which was cut after "eg." into the fragment
    // "CONSORT, PRISMA, etc.". Two occurrences in the seed and on 61 guide pages.
    token.starts_with("eg.") && i == start + 2 && token.chars().nth(3).is_none_or(char::is_whitespace)
}

/// Digits immediately after a lead phrase. A long gap means the number belongs
/// to something else — "up to the limit described in section 4" is not a limit
/// of four.
///
/// **A RANGE YIELDS ITS UPPER BOUND. §11 D243.** *"Please provide an abstract of
/// 150 to 250 words"* stores 250. The extractor keeps one value, and it keeps
/// the one whose breach is a violation, which is [`binding_limit`]'s rule: a
/// 240-word abstract complies and a 260-word one does not. The lower bound is
/// in the span for any reader who wants it.
fn number_after(s: &str, at: usize) -> Option<String> {
    let rest = &s[at..];
    let start = rest.find(|c: char| c.is_ascii_digit())?;
    if rest[..start].chars().filter(|c| !c.is_whitespace()).count() > 2 {
        return None;
    }
    let (digits, len) = digits_at(&rest[start..]);
    if digits.is_empty() {
        return None;
    }
    let after = rest[start + len..].trim_start();
    let upper = ["to ", "-", "–", "—"]
        .iter()
        .find_map(|sep| after.strip_prefix(sep))
        .map(|r| r.trim_start())
        .filter(|r| r.starts_with(|c: char| c.is_ascii_digit()))
        .map(|r| digits_at(r).0);
    match (upper, digits.parse::<u64>()) {
        (Some(u), Ok(lo)) if u.parse::<u64>().is_ok_and(|u| u > lo) => Some(u),
        _ => Some(digits),
    }
}

/// The number at the start of `s`, thousands separators dropped, and how many
/// bytes it spans.
///
/// **A separator followed by a space is still a separator. §11 D253.** *"Published
/// articles normally have fewer than 11, 000 words"* (Acta Materialia) was read
/// as 11. So `", "` or `". "` then EXACTLY three digits, then a space and a
/// word, continues the number. The trailing-word condition keeps a list of
/// numbers apart: over every sentence in the seed and on 57 guide pages the
/// pattern occurs three times, and the other two (*"Ion Processes, 142,
/// 209-240"*, *"the forms 30, 300, 3000"*) end in a digit or comma. The `". "`
/// form never occurs, and the sentence splitter cuts at it before this is
/// reached; the splitter is deliberately not changed without a measured case.
fn digits_at(s: &str) -> (String, usize) {
    let len: usize = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .map(char::len_utf8)
        .sum();
    // A trailing comma is punctuation, not a separator: "250, 10 references".
    let mut len = s[..len].trim_end_matches(',').len();
    let b = s.as_bytes();
    let group_len = s[..len].rsplit(',').next().map_or(0, str::len);
    if (1..=3).contains(&group_len)
        && b.len() >= len + 6
        && matches!(b[len], b',' | b'.')
        && b[len + 1] == b' '
        && b[len + 2..len + 5].iter().all(u8::is_ascii_digit)
        && b[len + 5] == b' '
        && b.get(len + 6).is_some_and(u8::is_ascii_alphabetic)
    {
        len += 5;
    }
    (s[..len].chars().filter(|c| c.is_ascii_digit()).collect(), len)
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

/// **Is this a limit on the TITLE? §11 D250.** There is no title-limit kind, so
/// such a row is refused, as D245 refuses a page count.
///
/// *"Other Manuscript titles should run to no more than 20 words in length"*
/// (Am J Medicine) and *"the most effective titles are no more than 10–12 words
/// and should readily give readers an overall view of the paper's significance"*
/// (J Hepatology) were stored as manuscript WORD limits. The unit is right; what
/// let them through is [`is_about_the_manuscript`], which accepts a part word
/// ("Manuscript", "paper's") anywhere in the sentence. So this reads the SUBJECT:
/// "title(s)" among the last four words of the clause before the limit phrase,
/// and not "title page". Measured over every word and abstract limit sentence in
/// the seed and on 41 guide pages: 9 mention a title, this refuses exactly the 6
/// title lengths, and keeps *"(including title and author information)"* and
/// *"excluding title page"*.
fn limit_is_on_the_title(lower: &str, lead_at: usize) -> bool {
    let clause = lower[..lead_at].rsplit([',', ';', ':', '.', '(', ')']).next().unwrap_or("");
    let words: Vec<&str> = clause
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let tail = &words[words.len().saturating_sub(4)..];
    if tail.iter().any(|w| matches!(*w, "title" | "titles")) && !tail.contains(&"page") {
        return true;
    }
    // **A limit that opens a bracket describes what precedes it. §11 D254.**
    // *"Title The full title and subtitle of the article (no more than 25
    // words)"* (Value in Health): the clause before the phrase is empty, so the
    // subject is the clause before the bracket. Measured over every bracketed
    // limit in the seed, on 57 guide pages and on the 80-journal batch: 17, and
    // only this one has a title before its bracket.
    let before = &lower[..lead_at];
    let Some(open) = before.rfind('(') else { return false };
    if before[open + 1..].chars().any(|c| c.is_alphanumeric() || c == ')') {
        return false;
    }
    let outer = before[..open].rsplit([',', ';', ':', '.', ')']).next().unwrap_or("");
    let outer_words: Vec<&str> =
        outer.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).collect();
    let titled = outer_words.iter().any(|w| matches!(*w, "title" | "titles"));
    let title_page = outer_words.windows(2).any(|w| matches!(w[0], "title" | "titles") && w[1] == "page");
    titled && !title_page
}

/// **Is this a limit on a part that is not the manuscript? §11 D261.** The
/// same question as [`limit_is_on_the_title`], with two more subjects, read
/// from the whole clause before the limit phrase. The clause runs through an
/// open bracket, so a bracketed limit is read against what the bracket
/// describes:
///
/// * *"The capsule is a summary of the abstract of 30 words or less"*
///   (Fertility and Sterility): the capsule's length, not the abstract's;
/// * *"Provide 3 highlight statements (a combined total of no more than 120
///   words)"* (Value in Health): the highlights', not the manuscript's.
///
/// Measured over the 222 seed limit rows and 61 saved guide pages. Sentences
/// that name highlights AFTER a real limit keep it (Water Research's 3,000,
/// Environmental Pollution's and Lingua's abstracts). Deliberately only these
/// two subjects; the other non-manuscript subjects measured (take-home
/// message, trial-registry abstract, case summaries, biography, impact
/// statement) are recorded in D261 and not decided here.
///
/// **A RUN-IN LABEL is not a subject.** A bold "Highlights" opening a paragraph
/// reaches the sentence glued on: *"Highlights The main text of the manuscript
/// must not exceed 6000 words"* (J Hepatology's shape, pinned in
/// `guidelines.rs`). So a subject word that is capitalised and followed directly
/// by another capitalised word ("Highlights The", "Capsule The", "SEO
/// Highlights Provide") is read as a label and skipped. Needs the sentence's
/// own case, so it takes the original beside the lowercased text; where
/// lowercasing changed the character count, the case is unavailable and every
/// occurrence counts.
fn limit_is_on_a_part_that_is_not_the_manuscript(sentence: &str, lower: &str, lead_at: usize) -> bool {
    let n = lower[..lead_at].chars().count();
    let prefix: String = if sentence.chars().count() == lower.chars().count() {
        sentence.chars().take(n).collect()
    } else {
        lower[..lead_at].to_string()
    };
    let clause = prefix.rsplit(['.', ';', ':', ',']).next().unwrap_or("");
    let words: Vec<&str> =
        clause.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).collect();
    let capital = |w: &str| w.starts_with(|c: char| c.is_ascii_uppercase());
    words.iter().enumerate().any(|(i, w)| {
        let subject = matches!(w.to_ascii_lowercase().as_str(), "capsule" | "highlight" | "highlights");
        let label = capital(w) && words.get(i + 1).is_some_and(|next| capital(next));
        subject && !label
    })
}

/// **Would the extractor refuse this stored LIMIT row today? §11 D261.** For
/// [`crate::journal_store::remove_misread_rows`]: true when the row's value is
/// read from its span only by leads whose unit has no kind (a count of authors)
/// or whose subject is not the manuscript. A stored row and a fresh crawl of the
/// same sentence then agree.
pub fn stored_limit_is_not_on_the_manuscript(span: &str, value: &str) -> bool {
    let lower = span.to_lowercase();
    let mut refused = Vec::new();
    for lead in LIMIT_LEADS {
        for (lead_at, _) in lower.match_indices(lead) {
            let at = lead_at + lead.len();
            if number_after(&lower, at).as_deref() != Some(value) {
                continue;
            }
            refused.push(
                limit_kind(&lower, at).is_none()
                    || limit_is_on_a_part_that_is_not_the_manuscript(span, &lower, lead_at),
            );
        }
    }
    !refused.is_empty() && refused.iter().all(|r| *r)
}

fn is_about_the_manuscript(sentence_lower: &str, article_type: &Option<String>) -> bool {
    article_type.is_some() || MANUSCRIPT_PARTS.iter().any(|p| sentence_lower.contains(p))
}

/// `"abstract of"` states a limit with no lead phrase at all: *"structured
/// abstract of 300 words"*, *"abstract of 150-200 words"*, *"abstract of maximum
/// 200 words"* (the last needs its own entry, because "maximum" is too long a
/// gap for [`number_after`]). Measured on the 16 guide pages, where Taylor &
/// Francis's per-type blocks and Springer's "Please provide an abstract of 150
/// to 250 words" use only this form: 21 stated limits, none extracted. §11 D243.
const LIMIT_LEADS: &[&str] = &[
    "up to", "no more than", "maximum of", "a maximum", "not exceed", "limited to",
    "must not exceed", "should not exceed", "fewer than", "at most",
    "abstract of", "abstract of maximum",
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
///
/// **The FIRST unit after the number decides, and a unit with no kind refuses
/// the row. §11 D245.** This scanned the whole 60-character window for any unit,
/// in a fixed order with "words" first, so a unit belonging to a LATER number
/// could claim this one:
///
/// * *"Manuscripts should normally not exceed 15 printed journal pages (around
///   10,000 words)"* (Int J Production Economics) was stored as a 15-WORD limit
///   and failed every manuscript. The unit of 15 is "pages", which has no kind,
///   and the "words" belongs to the approximation in brackets.
/// * *"up to 10 references and a maximum of 2 figures"* (Int J Cardiology) was
///   stored as a figure limit of 10, because "figures" outranked "references".
///
/// There is no page-limit kind (a page count depends on a layout Gaply does not
/// have), so a page limit is refused rather than converted.
fn limit_kind(lower: &str, number_at: usize) -> Option<RequirementKind> {
    let window: String = lower[number_at..].chars().take(60).collect();
    // Whole words only: "keywords" contains "words" (§11 D242).
    for token in window.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()) {
        match token {
            "words" => return Some(RequirementKind::WordLimit),
            // `Display items – up to 6 items` puts the qualifier BEFORE the
            // number and the bare noun after it, so the window that follows
            // says only "items". Measured on nature.com/nm/content.
            "figures" | "tables" | "items" => return Some(RequirementKind::FigureLimit),
            "references" => return Some(RequirementKind::ReferenceLimit),
            // Units with no kind: the number is not a limit Gaply can check.
            "pages" | "page" | "characters" | "lines" | "keywords" => return None,
            // A count of PEOPLE (§11 D261). *"limited to 3 authors, 400 words"*
            // (Fertility and Sterility) read past "authors" to "words" and
            // stored 3 as a word limit.
            "author" | "authors" => return None,
            _ => {}
        }
    }
    None
}

/// Phrases that introduce an EXTENSION of a limit rather than the limit.
const EXTENSION_PHRASES: &[&str] =
    &["can be increased", "may be increased", "can be extended", "may be extended"];

/// **Where a sentence starts describing how a limit may be EXTENDED. §11 D240.**
///
/// *"The word limit can be increased for each additional article in the Topic,
/// up to a maximum of 5,000 words for 50 articles or more."* (Frontiers,
/// Editorial). 5,000 is the ceiling for Research Topics of fifty or more
/// articles; the Editorial limit itself is not in the sentence at all. Stored as
/// the limit, it passed almost every editorial that was over the real one. A
/// number AFTER this position is an extension and is not read as a limit; a
/// number before it still is ("up to 1,000 words, which can be increased…").
/// Measured on the seed: the phrases occur in exactly those five limit spans.
pub fn extension_clause_at(lower: &str) -> Option<usize> {
    EXTENSION_PHRASES.iter().filter_map(|p| lower.find(p)).min()
}

/// What introduces an example rather than a rule. See
/// [`standard_named_as_example`] for why "such as" is not one.
const EXAMPLE_CUES: &[&str] = &["for example", "e.g.", "e.g", "eg.", "for instance"];

/// **Is the limit whose lead starts at `lead_at` given as an EXAMPLE? §11 D242.** The
/// same cue as [`standard_named_as_example`], directly before the lead phrase:
/// *"should be relatively brief (e.g. no more than 200 words)"* illustrates
/// "brief" and states no limit.
fn limit_named_as_example(lower: &str, lead_at: usize) -> bool {
    let before = lower[..lead_at].trim_end().trim_end_matches([',', ':']).trim_end();
    EXAMPLE_CUES.iter().any(|cue| before.ends_with(cue))
}

/// **Is every mention of `standard` in this sentence given as an EXAMPLE?
/// §11 D240.**
///
/// *"Materials and Methods (including flow diagram when applicable, for example
/// the CONSORT FLOW DIAGRAM…"* names CONSORT as an example of a flow diagram,
/// not as a standard the article must follow. The cue is "for example", "e.g."
/// or "for instance" DIRECTLY before the name (an article allowed between).
///
/// **"such as" is deliberately NOT a cue.** PLOS writes *"must adhere to the
/// relevant reporting guidelines for their study design, such as CONSORT for
/// randomized controlled trials"*, which is a requirement: the obligation is on
/// the guidelines, and the standard is one of them. Measured on the seed: this
/// predicate is true for the four Frontiers rows and for no other of the 67
/// reporting-standard rows, including BMJ's "(for example, for cluster RCTs…)",
/// where the cue comes AFTER the name.
pub fn standard_named_as_example(sentence: &str, standard: &str) -> bool {
    let mut any = false;
    for (i, _) in sentence.match_indices(standard) {
        any = true;
        let before = sentence[..i].trim_end().to_lowercase();
        let before = before
            .strip_suffix(" the")
            .or_else(|| before.strip_suffix(" a"))
            .or_else(|| before.strip_suffix(" an"))
            .unwrap_or(&before)
            .trim_end()
            .trim_end_matches(',')
            .trim_end();
        let framed = EXAMPLE_CUES.iter().any(|cue| before.ends_with(cue));
        if !framed {
            return false;
        }
    }
    any
}

/// **The article type a stored row carries, from its heading and its sentence
/// alone. §11 D239.**
///
/// One function, so the extractor and the reconcile of stored rows
/// ([`crate::journal_store::reclassify_article_types`]) cannot disagree: both
/// have exactly `(source_heading, source_span, kind)`, and D239 measured that
/// the stored type is a function of those three on 203 of 203 rows.
///
/// # Why LIMITS read differently
///
/// D238's hand-read found the largest class of wrong rows was a limit stated for
/// one article type stored with none. Measured per row:
///
/// * **the heading names the type, and the type is not in [`ARTICLE_TYPES`]**
///   (FAIR² Data, Analysis, Policy Forum, Guidelines and Guidance): read by
///   [`heading_as_article_type`];
/// * **the heading names a longer type than the list** ("Mini Review" matched
///   the list's "review"): the whole heading is the type.
///
/// Precedence for a limit: a heading the list covers WHOLE (D172's heading
/// first, pinned by `a_heading_type_outranks_a_sentence_type`), then the
/// sentence, then the whole heading, then a partial list match, then a plural
/// subject. An unlisted heading is the weaker statement: measured, putting it
/// before the sentence re-typed 12 correctly typed rows from the sentence's
/// form to the heading's ("Data Reports" to "Data Report");
/// * **the sentence names it in a shape the pattern missed** ("Analysis papers
///   should", "Curriculum, Instruction, and Pedagogy articles", "Letters
///   unrelated to…"): [`article_type_in_sentence`] and [`plural_type_subject`].
///
/// Other kinds keep the D172/D173 order, heading first: a statement required
/// under "Clinical Research" is not scoped to an article type called that, and
/// the whole-heading rule applies to limits only for exactly that reason.
pub fn article_type_for(heading: &str, sentence: &str, kind: RequirementKind) -> Option<String> {
    let is_limit = matches!(
        kind,
        RequirementKind::WordLimit
            | RequirementKind::AbstractLimit
            | RequirementKind::FigureLimit
            | RequirementKind::ReferenceLimit
    );
    if is_limit {
        // Heading first, as D172 settled (the more deliberate statement), but a
        // heading the list only PARTLY covers ("Mini Review" -> "review") gives
        // way to the whole heading when the heading has the shape of a type.
        let listed = article_type_of(heading);
        let covers_whole = listed.as_deref().is_some_and(|t| {
            let h = heading.trim().to_lowercase();
            let t = t.to_lowercase();
            h == t || h == format!("{t}s") || h == format!("{t}es")
        });
        if covers_whole {
            return listed;
        }
        // A sentence that names its type outranks a heading the list does not
        // carry: "Data Reports articles are…" under "Data Report" keeps the
        // sentence's form, which is what every stored row already carries.
        article_type_in_sentence(sentence)
            .or_else(|| heading_as_article_type(heading))
            .or(listed)
            .or_else(|| plural_type_subject(sentence))
            .or_else(|| type_named_in_sentence(sentence))
    } else {
        article_type_of(heading).or_else(|| article_type_in_sentence(sentence))
    }
}

/// Head nouns that make a short label the NAME of an article type.
const TYPE_HEAD_NOUNS: &[&str] = &[
    "article", "paper", "review", "report", "communication", "letter", "commentary",
    "editorial", "perspective", "opinion", "correspondence", "debate",
];

/// **May an inline label type the text under it? §11 D249.**
///
/// Publishers flatten article-type sections into labels: `<i>Expert
/// Opinion</i><br/>` (J Hepatology), `<b>Invited Reviews</b><br/>` (Int J
/// Cardiology), `<em>Original Article</em><br>` (J Internal Medicine).
/// `html_to_blocks` splits on such a label only when this says it names a type.
///
/// **Stricter than a heading, measured.** Of 78 such labels on four guide
/// pages, the heading rules would type 31, and about 15 of those are not
/// article types: Highlights, Abbreviations, Funding, Results, Proofs, Offprints,
/// Early View, Definitions, Introduction, Conclusions, Acknowledgements,
/// "Cover letter", "Editorial and peer review", "Clinical trials". So a label
/// must ALSO end in an article-type noun ([`TYPE_HEAD_NOUNS`], plural allowed),
/// and "peer review" and "cover letter" do not count (§11 D244, D173's residue).
pub fn label_names_article_type(label: &str) -> bool {
    let l = label.trim().trim_end_matches(':').trim();
    let words: Vec<String> = l.split_whitespace().map(|w| w.to_lowercase()).collect();
    let Some(last) = words.last() else { return false };
    let singular = last
        .strip_suffix("ies")
        .map(|s| format!("{s}y"))
        .or_else(|| last.strip_suffix('s').map(str::to_string))
        .unwrap_or_else(|| last.clone());
    if !TYPE_HEAD_NOUNS.contains(&singular.as_str()) && !TYPE_HEAD_NOUNS.contains(&last.as_str()) {
        return false;
    }
    if words.len() >= 2 && matches!(words[words.len() - 2].as_str(), "peer" | "cover") {
        return false;
    }
    let listed_whole = article_type_of(l).is_some_and(|t| {
        let (h, t) = (l.to_lowercase(), t.to_lowercase());
        h == t || h == format!("{t}s") || h == format!("{t}es")
    });
    listed_whole || heading_as_article_type(l).is_some()
}

/// **A listed type named anywhere in the limit's own sentence, in any case.
/// §11 D251.** The LAST fallback for a limit: every heading rule and the
/// sentence patterns above come first, so a heading that names a type wins.
///
/// D239's sentence readers need a capitalised type in a fixed shape
/// (`"<Type> articles should"`, a plural type opening the clause). Guides also
/// write *"review articles should preferably not exceed 8,000 words"* (J
/// Pragmatics), *"…and review articles should be up to 10,000 words"* (Renewable
/// Energy), *"The length of a Letter to the Editor should not exceed 800 words"*
/// and *"a maximum of 8 tables and/or figures per original article"* (J
/// Hepatology). Measured over every untyped limit sentence in the seed and on 41
/// guide pages, this fires on those four and on three more (*"The commentary
/// articles…"*, Research Policy's *"• Research Articles - … up to 8-10,000
/// words"*, Int J Cardiology's *"Original articles Text in these articles…"*),
/// all seven typed correctly, and on no seed row.
///
/// Bare "article" and "review" are not read: *"Articles should not exceed…"*
/// means every article of the section (see [`plural_type_subject`]), and
/// "review" hides in "peer review". "cover letter" is not a Letter (D173's
/// residue). A sentence naming two different types is not guessed.
fn type_named_in_sentence(sentence: &str) -> Option<String> {
    let lower = sentence.to_lowercase();
    let mut phrases: Vec<&str> = ARTICLE_TYPES
        .iter()
        .copied()
        .filter(|t| !matches!(*t, "article" | "review"))
        .chain(["original article"])
        .collect();
    phrases.sort_by_key(|t| std::cmp::Reverse(t.len()));
    let mut taken: Vec<(usize, usize)> = Vec::new();
    let mut found: Vec<&str> = Vec::new();
    for t in phrases {
        for (at, _) in lower.match_indices(t) {
            let mut end = at + t.len();
            if lower[end..].starts_with("es") {
                end += 2;
            } else if lower[end..].starts_with('s') {
                end += 1;
            }
            let bounded = !lower[..at].chars().next_back().is_some_and(char::is_alphanumeric)
                && !lower[end..].chars().next().is_some_and(char::is_alphanumeric);
            if !bounded || taken.iter().any(|&(s, e)| at < e && s < end) {
                continue;
            }
            if lower[..at].trim_end().ends_with("cover") || lower[..at].trim_end().ends_with("peer") {
                continue;
            }
            taken.push((at, end));
            if !found.contains(&t) {
                found.push(t);
            }
        }
    }
    if found.len() != 1 {
        return None;
    }
    Some(
        found[0]
            .split(' ')
            .map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// **May a RUN-IN label type the text after it? §11 D257.** Returns the type
/// name to use (the label without its colon and bracketed qualifier), or `None`.
///
/// Archives PM&R opens each type's paragraph with a bold run-in label and no
/// line break: `<p><b>Original Research:</b> … Manuscripts should be limited to
/// 3000 words…`. D249's labels need a line break, so all five limits fell under
/// "Types of papers" untyped. Run-in labels are also used for fields inside a
/// section ("Format Guidelines:", "Word Count:"), so admission is measured.
///
/// **Measured over 72 distinct run-in labels on 67 saved guide pages:** D249's
/// test ([`label_names_article_type`]) on the label with its bracket stripped
/// admits 9, all genuine types (Brief Reports, Commentaries (by Invitation),
/// Review Articles (Meta-Analyses), Special Communications, Good Practices
/// Reports…). It refuses "Original Research", because "research" is no type
/// noun, so a label that IS a listed type whole is admitted too: 10 admitted, all
/// genuine; "Format Guidelines", "Word Count", "Number of Pages", "Conflict of
/// Interest" and the other 58 refused. This bypass is for run-in labels only:
/// applied to D249's line-break labels it would admit "Clinical trials", a
/// policy section there.
pub fn run_in_label_names_article_type(label: &str) -> Option<String> {
    let mut l = label.trim().trim_end_matches(':').trim();
    if l.ends_with(')') {
        if let Some(i) = l.rfind('(') {
            l = l[..i].trim_end();
        }
    }
    if l.is_empty() {
        return None;
    }
    let listed_whole = article_type_of(l).is_some_and(|t| {
        let (h, t) = (l.to_lowercase(), t.to_lowercase());
        h == t || h == format!("{t}s") || h == format!("{t}es")
    });
    (label_names_article_type(l) || listed_whole).then(|| l.to_string())
}

/// Words a heading uses for a PART of a manuscript or of a guidelines page. A
/// heading containing one is about that part, not an article type: "Abstract",
/// "Structured abstract", "Author Guidelines", "9 – Extended data figures".
const NOT_A_TYPE_HEADING_WORDS: &[&str] = &[
    "abstract", "abstracts", "title", "keywords", "text", "reference", "references",
    "figure", "figures", "table", "tables", "author", "authors", "manuscript",
    "submission", "format", "formatting", "types", "style",
    // §11 D255: sections of a manuscript and of a guide page. Measured on the
    // 80-journal Elsevier batch, "Results", "Conclusion" and "Article
    // Structure" were each typed as an article type, and T&F's "Word Limits"
    // twice. Over 989 rows in the seed, on 57 guide pages and in the batch,
    // these words change exactly those 5 rows and no seed row. The alternative,
    // requiring every type heading to end in an article-type noun, refused ~69
    // real types (26 of them Clinical Trial) to fix ~10, 32 of them seed rows.
    "results", "result", "conclusion", "conclusions", "discussion", "introduction",
    "structure", "acknowledgements", "acknowledgments", "abbreviations", "highlights",
    "funding", "limits",
];

/// **A heading that IS an article type's name, for a limit under it. §11 D239.**
///
/// The shape, not a vocabulary: one to four words, every word capitalised (the
/// connectives "and" and "&" excepted), not opening with a gerund or a generic
/// lead ("All"), no preposition, and no word that names a part of a manuscript
/// or a page ([`NOT_A_TYPE_HEADING_WORDS`]). Measured on every untyped limit
/// row in the seed: it binds FAIR² Data, Analysis, Policy Forum and Guidelines
/// and Guidance, and leaves "Abstract", "Structured abstract", "Author
/// Guidelines", "Article types", "All rapid responses", "9 – Extended data
/// figures" and "Preparing an Analysis article" unbound.
fn heading_as_article_type(heading: &str) -> Option<String> {
    let h = heading.trim();
    if !h.chars().next().is_some_and(|c| c.is_alphabetic() && c.is_uppercase()) {
        return None;
    }
    let words: Vec<&str> = h
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    let content: Vec<&str> = words.iter().copied().filter(|w| !matches!(*w, "and" | "&")).collect();
    if content.is_empty() || content.len() > 4 {
        return None;
    }
    if !content.iter().all(|w| w.chars().next().is_some_and(|c| c.is_uppercase())) {
        return None;
    }
    let lower: Vec<String> = content.iter().map(|w| w.to_lowercase()).collect();
    if lower[0].ends_with("ing") && lower.len() > 1 {
        return None;
    }
    if NOT_AN_ARTICLE_TYPE.contains(&lower[0].as_str()) {
        return None;
    }
    if lower.iter().any(|w| {
        NOT_A_TYPE_HEADING_WORDS.contains(&w.as_str())
            || matches!(w.as_str(), "for" | "of" | "in" | "on" | "with" | "about" | "to")
    }) {
        return None;
    }
    Some(h.to_string())
}

/// **A listed type, plural, as the SUBJECT of the clause. §11 D239.**
///
/// *"Word/reference count: Letters unrelated to a specific article should not
/// exceed 500 words"* scopes the limit to Letters, and neither the heading
/// ("Article types") nor the `"<Type> articles are"` shape says so. Read from
/// the last `:` onwards, the clause opens with a capitalised plural of a type in
/// [`ARTICLE_TYPES`]. **"Articles" itself is excluded**: *"Articles should not
/// exceed 3000 words"* under a type heading means the articles of that section,
/// and reading it as the type "Article" would move the limit to the wrong type.
fn plural_type_subject(sentence: &str) -> Option<String> {
    let clause = sentence.rsplit(':').next().unwrap_or(sentence).trim();
    let first = clause.split_whitespace().next()?;
    if !first.chars().next().is_some_and(|c| c.is_uppercase()) {
        return None;
    }
    let lower = first.to_lowercase();
    ARTICLE_TYPES
        .iter()
        .filter(|t| !t.contains(' ') && **t != "article")
        .find(|t| lower == format!("{t}s") || lower == format!("{t}es"))
        .map(|t| {
            let mut c = t.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
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
            // The type a LIMIT row carries (§11 D239). Deliberately separate from
            // `article_type` above, which still decides ADMISSION
            // (`is_about_the_manuscript`): a better type must not widen what a
            // crawl admits, and that widening cannot be measured on the seed.
            let limit_type = article_type_for(&block.heading, sentence, RequirementKind::WordLimit);

            // ONE SENTENCE, ONE LIMIT PER KIND — see `binding_limit`.
            let mut limits: Vec<(RequirementKind, String)> = Vec::new();
            // §11 D240: a number after "the limit can be increased…" is how far
            // it may be EXTENDED under a condition, not the limit.
            let extension_from = extension_clause_at(&lower);
            for lead in LIMIT_LEADS {
                let mut from = 0usize;
                while let Some(i) = lower[from..].find(lead) {
                    let at = from + i + lead.len();
                    let lead_at = from + i;
                    from = at;
                    if extension_from.is_some_and(|p| at > p) {
                        continue;
                    }
                    if limit_named_as_example(&lower, lead_at) {
                        continue;
                    }
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
                    // A title's length is not the manuscript's (§11 D250).
                    if matches!(kind, RequirementKind::WordLimit | RequirementKind::AbstractLimit)
                        && limit_is_on_the_title(&lower, lead_at)
                    {
                        continue;
                    }
                    // Nor a capsule's or the highlights' (§11 D261).
                    if matches!(kind, RequirementKind::WordLimit | RequirementKind::AbstractLimit)
                        && limit_is_on_a_part_that_is_not_the_manuscript(sentence, &lower, lead_at)
                    {
                        continue;
                    }
                    limits.push((kind, value));
                }
            }
            for (kind, value) in binding_limit(limits) {
                push(&mut out, kind, value, &limit_type, block, sentence);
            }

            // A style NAME is not a style STATEMENT — §11 D171/D172.
            for (needle, label) in REFERENCE_STYLES {
                if contains_word(&lower, needle) && states_a_reference_style(&lower) {
                    push(&mut out, RequirementKind::ReferenceStyle, (*label).to_string(),
                         &article_type, block, sentence);
                }
            }

            for s in STANDARDS {
                if sentence.contains(s) && !standard_named_as_example(sentence, s) {
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
        // WHOLE. `sentences()` above already yields one complete sentence, so
        // there is nothing here to bound — and a bound here was not a display
        // choice, it was DATA LOSS: the source text is not retained
        // (`journal_guidelines` holds the fetched page only while a crawl is in
        // flight), so a clipped span could never be repaired afterwards.
        //
        // Measured 22 Sep 2026 on the shipped Nature Medicine profile: two
        // checklist rows stored EXACTLY 400 characters and ended mid-word at
        // "…a completed copy of the Nature Research Por". A researcher reading
        // that cannot check the requirement against the journal's words, which
        // is the only thing a span is for. `report_compose.rs:600` already says
        // the composer prints spans "WHOLE, never clipped"; it was telling the
        // truth about itself and inheriting a truncated input.
        source_span: span.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- §11 D239: a limit keeps the article type its text states --------
    //
    // Every fixture is the bundled seed's own (heading, span), verbatim.

    fn lim(heading: &str, span: &str) -> Option<String> {
        article_type_for(heading, span, RequirementKind::FigureLimit)
    }

    /// The heading names a type the list does not carry.
    #[test]
    fn a_limit_under_a_heading_that_names_an_unlisted_type_carries_it() {
        let fair = "These are capped at 12,000 words and may include up to 15 figures or tables, ensuring recognition and long-term visibility.";
        assert_eq!(lim("FAIR² Data", fair).as_deref(), Some("FAIR² Data"));
        assert_eq!(lim("Analysis", "Display items – up to 6 items (figures and/or tables).").as_deref(), Some("Analysis"));
        let plos = "Articles should not exceed 3000 words and may cite up to 30 references.";
        assert_eq!(lim("Guidelines and Guidance", plos).as_deref(), Some("Guidelines and Guidance"));
        assert_eq!(
            lim("Policy Forum", "Articles should not exceed 2000 words and may cite up to 30 references.").as_deref(),
            Some("Policy Forum"),
            "\"Articles should\" under a type heading is that section's articles, not the type Article"
        );
    }

    /// The sentence names the type in a shape the old pattern missed.
    #[test]
    fn a_limit_whose_sentence_names_its_type_carries_it() {
        assert_eq!(
            lim("Preparing an Analysis article", "Analysis papers should be 2000 words with 20 references and up to 3 non-text items (box, figure, or table).").as_deref(),
            Some("Analysis")
        );
        let cip = "Curriculum, Instruction, and Pedagogy articles are peer-reviewed, have a maximum word count of 5,000 and may contain no more than 5 Figures/Tables.";
        assert_eq!(lim("Curriculum, Instruction, and Pedagogy", cip).as_deref(), Some("Curriculum, Instruction, and Pedagogy"));
        let letters = "Word/reference count: Letters unrelated to a specific article should not exceed 500 words or have more than 3 references.";
        assert_eq!(
            article_type_for("Article types", letters, RequirementKind::WordLimit).as_deref(),
            Some("Letter")
        );
    }

    /// "Mini Review" is not "Review": the list's suffix match lost the modifier.
    #[test]
    fn a_mini_review_limit_is_not_filed_under_review() {
        let span = "They offer a succinct and clear summary of the topic, allowing readers to get up-to-date on new developments and/or emerging concepts, as well as discuss the following: Different schools of thought or controversies Current research gaps Potential future developments in the field Mini Reviews articles are peer-reviewed, have a maximum word count of 3,000 and may contain no more than 2 Figures/Tables.";
        let got = lim("Mini Review", span);
        assert!(got.as_deref().is_some_and(|t| t.starts_with("Mini Review")), "{got:?}");
    }

    /// **Negative controls: a limit that correctly has no type keeps none**, and
    /// a non-limit row keeps the heading-first rule.
    #[test]
    fn headings_that_name_a_part_or_a_page_bind_no_type() {
        for (heading, span) in [
            ("Abstract", "The Abstract should: Describe the main objective(s) of the study … Not exceed 300 words"),
            ("Structured abstract", "Abstracts should be 250- 300 words long: you may need up to 400 words, however, for a CONSORT or PRISMA style abstract."),
            ("Author Guidelines", "Enter an abstract of up to 250 words for all articles [except book reviews]."),
            ("9 – Extended data figures", "A maximum of 10 Extended Data display figures is permitted."),
            ("Article types", "1,800 words, maximum of 40 references Practice Pointer These are practical, often problem-based articles."),
            ("All rapid responses", "The word limit for rapid responses is 600 words (excluding references) and they should have no more than 10 references."),
        ] {
            assert_eq!(lim(heading, span), None, "{heading:?} must stay unbound");
        }
        // A statement required under a section heading is not scoped to a type
        // named after the section: the whole-heading rule is for limits only.
        assert_eq!(
            article_type_for(
                "Clinical Research",
                "Any relevant funding should be declared in a separate funding statement.",
                RequirementKind::SectionRequired
            ),
            None
        );
    }

    // ---- §11 D240: an extension is not the limit; an example is not a rule --

    #[test]
    fn a_limit_that_can_be_increased_is_not_read_from_its_extension() {
        let fr = "The word limit can be increased for each additional article in the Topic, up to a maximum of 5,000 words for 50 articles or more.";
        let got = extract_requirements(&[b("Editorial", fr)]);
        assert!(!got.iter().any(|r| r.kind == RequirementKind::WordLimit), "{got:#?}");
        // A number BEFORE the extension clause is still the limit.
        let got = extract_requirements(&[b(
            "Editorial",
            "Editorials are up to 1,000 words, which can be increased to 2,000 words for large Topics.",
        )]);
        let w: Vec<_> = got.iter().filter(|r| r.kind == RequirementKind::WordLimit).collect();
        assert_eq!(w.len(), 1, "{got:#?}");
        assert_eq!(w[0].value, "1000");
    }

    #[test]
    fn a_standard_named_as_an_example_is_not_a_requirement() {
        let fr = "Clinical Trial articles should have the following format: Abstract (please include the clinical trial registry number) Introduction Materials and Methods (including flow diagram when applicable, for example the CONSORT FLOW DIAGRAM- http://www.";
        assert!(!extract_requirements(&[b("Clinical Trial", fr)])
            .iter()
            .any(|r| r.kind == RequirementKind::ReportingStandard));
        // Negative controls, verbatim from the seed: "such as" is a requirement,
        // and a "for example" AFTER the name does not touch it.
        for span in [
            "Clinical trial reports must adhere to the relevant reporting guidelines for their study design, such as CONSORT for randomized controlled trials, TREND for non-randomized trials, and other specialized guidelines as appropriate.",
            "For a clinical trials , use the CONSORT checklist and also include a structured abstract that follows the CONSORT extension for abstract checklist, the CONSORT flowchart and, where applicable, the appropriate CONSORT extension statements (for example, for cluster RCTs, pragmatic trials, etc.",
        ] {
            assert!(
                extract_requirements(&[b("Reporting guidelines", span)])
                    .iter()
                    .any(|r| r.kind == RequirementKind::ReportingStandard && r.value == "CONSORT"),
                "{span:?}"
            );
        }
    }

    // ---- §11 D242: a keyword count, and a biographical note's example -------
    //
    // Fixtures are verbatim from Annals of Medicine's and the Journal of
    // Coordination Chemistry's Taylor & Francis guides.

    #[test]
    fn a_keyword_count_is_not_a_word_limit() {
        for heading in ["Research Article", "Review Article", "Clinical Trial"] {
            let got = extract_requirements(&[b(heading, "Should contain no more than 6 keywords .")]);
            assert!(got.is_empty(), "{heading}: {got:#?}");
        }
        // The same sentence shape with "words" as a word is still a limit.
        let got = extract_requirements(&[b("Research Article", "Should contain no more than 6000 words .")]);
        assert_eq!(got.len(), 1, "{got:#?}");
        assert_eq!((got[0].kind, got[0].value.as_str()), (RequirementKind::WordLimit, "6000"));
    }

    #[test]
    fn a_biographical_notes_example_length_is_not_an_article_limit() {
        let jcc = "Biographical note (for Review Articles only). Please supply a short biographical note for each author. This could be adapted from your departmental website or academic networking profile and should be relatively brief (e.g. no more than 200 words).";
        let got = extract_requirements(&[b("Review Articles", jcc)]);
        assert!(!got.iter().any(|r| r.kind == RequirementKind::WordLimit), "{got:#?}");
    }

    #[test]
    fn e_g_and_i_e_do_not_end_a_sentence() {
        let s = sentences("It should be brief (e.g. no more than 200 words). Use one style, i.e. APA.");
        assert_eq!(
            s,
            vec!["It should be brief (e.g. no more than 200 words).", "Use one style, i.e. APA."]
        );
    }

    // ---- §11 D243: "abstract of N words", with no lead phrase -----------------
    //
    // Fixtures are verbatim from the Springer, Taylor & Francis and Elsevier
    // guides of the 16-page yield measurement.

    fn abstract_limits(heading: &str, span: &str) -> Vec<(String, Option<String>)> {
        extract_requirements(&[b(heading, span)])
            .into_iter()
            .filter(|r| matches!(r.kind, RequirementKind::WordLimit | RequirementKind::AbstractLimit))
            .map(|r| {
                assert_eq!(r.kind, RequirementKind::AbstractLimit, "{r:#?}");
                (r.value, r.article_type)
            })
            .collect()
    }

    #[test]
    fn an_abstract_of_n_words_is_an_abstract_limit() {
        let one = |v: &str, t: Option<&str>| vec![(v.to_string(), t.map(String::from))];
        assert_eq!(abstract_limits("Abstract", "Please provide an abstract of 150 to 250 words."), one("250", None));
        assert_eq!(
            abstract_limits("Research Article", "Should contain an unstructured abstract of 150-200 words."),
            one("200", Some("Research Article"))
        );
        assert_eq!(
            abstract_limits("Clinical Trial", "Should contain a structured abstract of 300 words."),
            one("300", Some("Clinical Trial"))
        );
        assert_eq!(
            abstract_limits("Review Article", "Should contain an unstructured abstract of maximum 200 words."),
            one("200", Some("Review Article"))
        );
        // A word limit and an abstract limit in one run-on sentence stay two
        // kinds (Taylor & Francis's per-type blocks read exactly like this).
        let ijpr = "figure captions (as a list) Should be no more than 12,000 words, inclusive of: Abstract Tables References Figure or table captions Should contain an unstructured abstract of maximum 200 words.";
        let got = extract_requirements(&[b("Research Article", ijpr)]);
        let got: Vec<_> = got.iter().map(|r| (r.kind, r.value.as_str())).collect();
        assert_eq!(got, vec![(RequirementKind::WordLimit, "12000"), (RequirementKind::AbstractLimit, "200")]);
    }

    #[test]
    fn a_range_yields_its_upper_bound() {
        assert_eq!(number_after("abstract of 150 to 250 words", 11), Some("250".into()));
        assert_eq!(number_after("abstract of 150-200 words", 11), Some("200".into()));
        assert_eq!(number_after("abstract of 150 – 200 words", 11), Some("200".into()));
        assert_eq!(number_after("up to 4,000 to 6,000 words", 5), Some("6000".into()));
        // Not a range: a list, and a second number that is not larger.
        assert_eq!(number_after("up to 250, 10 references", 5), Some("250".into()));
        assert_eq!(number_after("up to 250 - 3 figures", 5), Some("250".into()));
    }

    #[test]
    fn abstract_of_without_a_number_or_after_a_lead_reads_nothing_new() {
        // No number: the abstract OF something.
        assert!(abstract_limits(
            "Clinical trials",
            "Please report the study ID number and the website where the clinical trial is registered at the end of the abstract of the article."
        )
        .is_empty());
        // "abstract of up to N" was always read, by "up to", and is still one row.
        assert_eq!(
            abstract_limits("Abstract", "Please provide an abstract of up to 250 words."),
            vec![("250".to_string(), None)]
        );
    }

    // ---- §11 D244: "peer review" is the process, not the article type -------

    #[test]
    fn a_peer_review_heading_is_not_the_article_type_review() {
        // Verbatim from Elsevier's guide template (Journal of Historical Geography).
        let span = "Declaration of competing interests (when a separate declaration of interest file is not submitted) Corresponding author address (full address is required) and email address The anonymized manuscript should contain the main body of your paper, including references and tables.";
        let got = extract_requirements(&[b("Double anonymized peer review", span)]);
        assert_eq!(got.len(), 1, "{got:#?}");
        assert_eq!(got[0].article_type, None, "{got:#?}");
        for h in ["Peer review", "Single-blind peer review", "Peer Review and Ethics", "Research data and peer review"] {
            assert_eq!(article_type_of(h), None, "{h}");
        }
        // The headings that ARE the type keep it.
        for h in ["Review", "Mini Review", "Book Reviews", "New Media Reviews"] {
            assert_eq!(article_type_of(h).as_deref(), Some("Review"), "{h}");
        }
    }

    // ---- §11 D245: the first unit after the number decides its kind --------

    fn limits(span: &str) -> Vec<(RequirementKind, String)> {
        let mut v: Vec<_> = extract_requirements(&[b("Article types", span)])
            .into_iter()
            .filter(|r| r.kind.as_str().ends_with("_limit"))
            .map(|r| (r.kind, r.value))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn the_first_unit_after_a_number_is_its_unit() {
        // Verbatim, Int J Production Economics: 15 PAGES, not 15 words.
        assert_eq!(limits("Manuscripts should normally not exceed 15 printed journal pages (around 10,000 words)."), vec![]);
        // Verbatim, Int J Pavement Engineering: pages, not figures.
        assert_eq!(limits("A typical paper for this journal should be no more than 25 pages, inclusive of: Abstract Tables References Figure or table captions Footnotes Endnotes"), vec![]);
        // Verbatim, Int J Cardiology: 10 is the reference count, 2 the figures.
        let mut want = vec![
            (RequirementKind::WordLimit, "1500".to_string()),
            (RequirementKind::ReferenceLimit, "10".to_string()),
            (RequirementKind::FigureLimit, "2".to_string()),
        ];
        want.sort();
        assert_eq!(limits("Each article should consist of a maximum of 1500 words, up to 10 references and a maximum of 2 figures."), want);
    }

    // ---- §11 D249: which inline labels may type a section --------------------

    #[test]
    fn only_a_label_naming_an_article_type_is_admitted() {
        // All from the label scan of four guide pages.
        for l in ["Commentary", "Invited Reviews", "Expert Opinion", "Original Article", "Brief Report",
                  "Editorials", "Short communication", "Patient Perspectives", "Consensus and Position Papers", "Debate"] {
            assert!(label_names_article_type(l), "{l}");
        }
        for l in ["Highlights", "Abbreviations", "Funding", "Results", "Proofs", "Offprints", "Early View",
                  "Definitions", "Introduction", "Conclusions", "Acknowledgements", "Cover letter",
                  "Editorial and peer review", "Clinical trials", "Accelerated Publication Fee", "Title page"] {
            assert!(!label_names_article_type(l), "{l}");
        }
    }

    // ---- §11 D250: a title's length is not a word limit ---------------------

    #[test]
    fn a_title_length_is_not_a_word_limit() {
        let words = |h: &str, span: &str| -> Vec<String> {
            extract_requirements(&[b(h, span)])
                .into_iter()
                .filter(|r| r.kind == RequirementKind::WordLimit)
                .map(|r| r.value)
                .collect()
        };
        // Verbatim, Am J Medicine and J Hepatology.
        assert!(words("Article types", "Other Manuscript titles should run to no more than 20 words in length.").is_empty());
        assert!(words("Article structure", "As a general guideline, the most effective titles are no more than 10–12 words and should readily give readers an overall view of the paper's significance.").is_empty());
        // Negative controls, verbatim: a title named inside the counted text.
        assert_eq!(words("Commentary", "The commentary articles should be no more than 1000 words in length (including title and author information)."), vec!["1000"]);
        assert_eq!(words("Research article", "If including an experimental section: up to 4,500 words, including figures and tables and excluding title page, abstract and keywords."), vec!["4500"]);
    }

    // ---- §11 D261: a limit on a capsule, highlights or authors ---------------

    /// Three seed rows stored as manuscript limits, verbatim: F&S's Capsule
    /// (abstract 30), F&S's "3 authors" (word limit 3), Value in Health's
    /// highlights (word limit 120). The controls are the real limits on the same
    /// pages and the sentences that name highlights or keywords AFTER a real
    /// limit, verbatim from the seed.
    #[test]
    fn a_limit_on_a_capsule_highlights_or_authors_is_not_a_manuscript_limit() {
        let limits = |h: &str, span: &str| -> Vec<(String, String)> {
            extract_requirements(&[b(h, span)])
                .into_iter()
                .filter(|r| matches!(r.kind, RequirementKind::WordLimit | RequirementKind::AbstractLimit))
                .map(|r| (r.kind.as_str().to_string(), r.value))
                .collect()
        };
        let one = |k: &str, v: &str| vec![(k.to_string(), v.to_string())];
        assert_eq!(limits("Submission", "Capsule The capsule is a summary of the abstract of 30 words or less."), vec![]);
        assert_eq!(limits("Letters to the Editors", "Letters to the Editors are limited to 3 authors, 400 words (not counting the title page or references), and 1 to 4 references."), vec![]);
        assert_eq!(limits("Highlights", "and Making Your Article Visible with SEO Highlights Provide 3 highlight statements (a combined total of no more than 120 words) that capture the paper's contribution to the field."), vec![]);

        // Controls: the real limits survive.
        assert_eq!(limits("Submission", "Submissions are limited to 650 words, up to a total of two tables and/or figures and a maximum of 5 references."), one("word_limit", "650"));
        assert_eq!(limits("Short Communications", "Submissions are usually limited to 3000 words accompanied by no more than two illustrations (figures or tables), plus a short abstract and up to three highlights."), one("word_limit", "3000"));
        assert_eq!(limits("Abstract", "The abstract (up to 300 words), highlights and conclusions of papers in this journal must contain clear and concise statements."), one("abstract_limit", "300"));
        // A run-in "Highlights" label glued to a real limit is not its subject.
        assert_eq!(limits("Special sections", "Highlights The main text of the manuscript must not exceed 6000 words."), one("word_limit", "6000"));
        assert_eq!(limits("Abstract", "As well as an abstract (of no more than 200 words) and a maximum of 6 keywords, authors must provide highlights, namely 3 to 5 bullet points (85 characters maximum, including spaces, per bullet point)."), one("abstract_limit", "200"));
    }

    // ---- §11 D251: a listed type named anywhere in the limit's sentence -------

    #[test]
    fn a_type_named_in_the_limits_own_sentence_types_it() {
        let ty = |h: &str, span: &str| -> Vec<Option<String>> {
            extract_requirements(&[b(h, span)])
                .into_iter()
                .filter(|r| r.kind.as_str().ends_with("_limit"))
                .map(|r| r.article_type)
                .collect()
        };
        let one = |t: &str| vec![Some(t.to_string())];
        // Verbatim, the four target sentences.
        assert_eq!(ty("Article types", "review articles should preferably not exceed 8,000 words."), one("Review Article"));
        assert_eq!(ty("Article types", "Paper length: as a guide, original papers should be between 4000 and 6000 words (excluding table/figure captions and references), and review articles should be up to 10,000 words."), one("Review Article"));
        assert_eq!(ty("Letters to the Editor", "The length of a Letter to the Editor should not exceed 800 words and may be subject to further editing by the Editors."), one("Letter"));
        assert_eq!(ty("Article structure", "There is a maximum of 8 tables and/or figures per original article."), one("Original Article"));
        // The heading wins when it names a type.
        assert_eq!(ty("Review Articles", "Unlike research articles, the manuscript should not exceed 5000 words."), one("Review Article"));
        // Bare "Articles" means every article of the section: no type.
        assert_eq!(ty("Article types", "Articles should not exceed 3000 words."), vec![None]);
        // Two different types in one sentence are not guessed.
        assert_eq!(ty("Article types", "Unlike review articles, letters to the editor must not exceed 500 words."), vec![None]);
        // A cover letter is not a Letter.
        assert_eq!(ty("Submission", "The manuscript and its cover letter together should not exceed 6000 words."), vec![None]);
    }

    // ---- §11 D253: a separator followed by a space is still a separator ------

    #[test]
    fn a_spaced_thousands_separator_is_read_whole() {
        let words = |span: &str| -> Vec<String> {
            extract_requirements(&[b("Length of papers", span)])
                .into_iter()
                .filter(|r| r.kind == RequirementKind::WordLimit)
                .map(|r| r.value)
                .collect()
        };
        // Verbatim, Acta Materialia.
        assert_eq!(words("Published articles normally have fewer than 11, 000 words and 12 figures in the main text."), vec!["11000"]);
        // Negative controls: a comma before a list or another count is punctuation.
        assert_eq!(number_after("up to 250, 10 references", 5), Some("250".into()));
        assert_eq!(number_after("up to 30, 300, 3000 words", 5), Some("30".into()));
        assert_eq!(number_after("up to 4,000 words", 5), Some("4000".into()));
    }

    // ---- §11 D254: a title limit inside a bracket ----------------------------

    #[test]
    fn a_title_limit_inside_a_bracket_is_not_a_word_limit() {
        let words = |heading: &str, span: &str| -> Vec<String> {
            extract_requirements(&[b(heading, span)])
                .into_iter()
                .filter(|r| r.kind == RequirementKind::WordLimit)
                .map(|r| r.value)
                .collect()
        };
        // Verbatim, Value in Health (a cover-letter component list).
        assert!(words("II. Manuscript Specifications and Submission", "In addition, the cover letter should include the following specific components: Components Description Title The full title and subtitle of the article (no more than 25 words) Description/Interest to Readers A brief description of the article.").is_empty());
        // Negative controls, verbatim: a bracketed limit on the manuscript text.
        assert_eq!(words("Commentary", "The commentary need not follow a structured format, should be limited to 1-2 typed pages (maximum of 1,000 words), and may include up to 10 citations, as well as a figure or table."), vec!["1000"]);
        assert_eq!(words("References", "Letters must be short (a maximum 800 words) and include only key references (5 maximum) and one figure if necessary."), vec!["800"]);
    }

    // ---- §11 D255: a manuscript or page section is not an article type ------

    #[test]
    fn a_section_heading_is_not_an_article_type() {
        let ty = |h: &str, span: &str| -> Vec<Option<String>> {
            extract_requirements(&[b(h, span)])
                .into_iter()
                .filter(|r| r.kind.as_str().ends_with("_limit"))
                .map(|r| r.article_type)
                .collect()
        };
        // Verbatim, from the Elsevier batch and T&F.
        assert_eq!(ty("Results", "Archives aims to publish no more than 5 figures per manuscript so restrict tables and figures to those needed to explain arguments and to assess their support."), vec![None]);
        assert_eq!(ty("Article Structure", "Figures (up to 8 total figures/tables combined) Number consecutively."), vec![None]);
        assert_eq!(ty("Conclusion", "Length of papers Published articles normally have fewer than 11, 000 words and 12 figures in the main text."), vec![None]);
        assert_eq!(ty("Word Limits", "A typical paper for this journal should be no more than 5000 words."), vec![None]);
        // Headings that ARE types keep them (all seed headings).
        assert_eq!(ty("FAIR² Data", "These are capped at 12,000 words and may include up to 15 figures or tables."), vec![Some("FAIR² Data".into())]);
        assert_eq!(ty("Policy Forum", "Articles should not exceed 2000 words and may cite up to 30 references."), vec![Some("Policy Forum".into()); 2]);
    }

    // ---- §11 D256: "eg." is "e.g." -------------------------------------------

    #[test]
    fn eg_without_its_first_period_is_an_abbreviation_and_an_example_cue() {
        // Verbatim, Archives PM&R.
        let span = "Authors should make sure the key elements from the Reporting Guideline (eg. CONSORT, PRISMA, etc.) they followed for their manuscript are included in the abstract as well as the body of the paper.";
        assert!(sentences(span)[0].contains("(eg. CONSORT"), "{:?}", sentences(span));
        let got = extract_requirements(&[b("Abstract", span)]);
        assert!(!got.iter().any(|r| r.value == "CONSORT"), "{got:#?}");
    }

    // ---- §11 D257: which run-in labels may type a paragraph ------------------

    #[test]
    fn only_a_run_in_label_naming_an_article_type_is_admitted() {
        let got = |l: &str| run_in_label_names_article_type(l);
        assert_eq!(got("Original Research:").as_deref(), Some("Original Research"));
        assert_eq!(got("Review Articles (Meta-Analyses):").as_deref(), Some("Review Articles"));
        assert_eq!(got("Commentaries (by Invitation)").as_deref(), Some("Commentaries"));
        for l in ["Format Guidelines", "Word Count", "Number of Pages", "Conflict of Interest", "File format", "Use of AI"] {
            assert_eq!(got(l), None, "{l}");
        }
    }

    /// **A stored span is the journal's complete sentence, or it is not evidence.**
    ///
    /// The extractor used to store `span.chars().take(400)`. Measured 22 Sep
    /// 2026 on the shipped Nature Medicine profile, two checklist rows were
    /// EXACTLY 400 characters and ended mid-word:
    ///
    /// ```text
    /// "…all fast track submissions must include … a completed copy of the Nature Research Por"
    /// ```
    ///
    /// The cut was unrecoverable: the fetched page is not retained, so nothing
    /// downstream could restore the sentence. This pins the whole sentence AND
    /// the property a reader actually depends on — that the span does not stop
    /// mid-word — because a future cap at a different number would pass a
    /// length-only assertion.
    #[test]
    fn a_stored_span_is_a_whole_sentence_and_never_stops_mid_word() {
        // A real guideline sentence shape, deliberately longer than the old cap.
        let long = "All fast track submissions must include a cover letter explaining the \
                    urgency of the work, a completed reporting summary, a data availability \
                    statement, a code availability statement where custom code was used, a \
                    competing interests declaration covering every listed author, and a \
                    completed copy of the Nature Research Portfolio reporting summary, which \
                    the editors use to check that the study has been described in enough \
                    detail for an independent group to repeat it without contacting the \
                    authors for further information.";
        assert!(long.chars().count() > 400, "fixture must exceed the removed cap");
        let blocks = vec![b("Fast track", long)];
        let reqs = extract_requirements(&blocks);
        assert!(!reqs.is_empty(), "the fixture must produce at least one requirement");

        for r in &reqs {
            let span = r.source_span.trim_end();
            assert!(!span.is_empty(), "a span-less row cannot be refuted: {r:?}");
            // 1. No artificial truncation marker.
            for marker in ["\u{2026}", "...", "[truncated]"] {
                assert!(
                    !span.ends_with(marker),
                    "span carries a truncation marker: {span:?}"
                );
            }
            // 2. Never stops mid-word. A complete sentence ends in terminal
            //    punctuation; the old 400-char cut ended on a letter.
            let last = span.chars().last().unwrap();
            assert!(
                !last.is_alphanumeric(),
                "span stops mid-word, which is what the 400-char cap did: …{:?}",
                span.chars().rev().take(40).collect::<String>().chars().rev().collect::<String>()
            );
            // 3. The cap is gone, not merely raised.
            assert_ne!(span.chars().count(), 400, "a 400-char span is the old cap: {span:?}");
        }
        // 4. The sentence survived WHOLE — the tail the cap used to remove.
        assert!(
            reqs.iter().any(|r| r.source_span.contains("Nature Research Portfolio")),
            "the text beyond the old cap is missing: {:?}",
            reqs.iter().map(|r| r.source_span.chars().count()).collect::<Vec<_>>()
        );
    }

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
            // §11 D239: a heading the list covers only in part is its own full
            // name for a LIMIT. D173 pinned "Mini Review" -> "Review", which is
            // the defect D238 found: Frontiers states different limits for
            // Mini Review and for Review.
            ("Study Protocol", "Study Protocol"),
            ("Mini Review", "Mini Review"),
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
