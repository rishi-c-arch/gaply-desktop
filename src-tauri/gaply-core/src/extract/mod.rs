//! Extraction Agent: parse a manuscript into structured sections and pull
//! out statistical claims, in-text citations, and reference-list entries.
//!
//! Pure, offline Rust — the parsers (`pdf-extract`, `zip`+`quick-xml`) read
//! local bytes only; nothing here makes a network call. Everything operates
//! on plaintext, so the format adapters in [`docparse`] are the only
//! format-specific code.

pub mod citations;
pub mod claims;
pub mod datasets;
pub mod docparse;
pub mod methods;
pub mod omml;
pub mod persist;
pub mod sections;
pub mod sentence;
pub mod stats;
pub mod variables;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::scientific_model::ScientificExtraction;

pub use citations::{Citation, CitationStyle, Reference};
pub use sections::{Section, SectionKind};
pub use stats::{Stat, StatClaim};

/// A location within the parsed document: which section, and which paragraph
/// (0-based) inside that section.
///
/// # `section_index` exists because A KIND IS NOT A KEY — §11 D169
///
/// `SectionKind` was the whole address, and a document may hold several
/// sections of one kind. Measured over 20 real manuscripts: **10 of 20 have at
/// least one repeated kind**, and the repeats are chapter structure rather than
/// a classifier artefact — `"2.0 Introduction"`, `"3.0 Introduction"`,
/// `"4.0 Introduction"` are three real chapter introductions (41 of 43 repeated
/// sections carry DIFFERENT headings).
///
/// With only the kind, [`paragraph_at`] resolved by `find`, taking the FIRST
/// section of that kind, and **283 of 869 statistical claims resolved to a
/// paragraph that does not contain them** — inputs to `validate.rs`'s five
/// Tier-0 rules, which `swarm.rs` treats as `hard_constraint`, never voted on,
/// always overriding every model.
///
/// **The producer always knew.** `extract_from_text_with` builds every
/// `Location` inside a loop over sections; the index was discarded at
/// construction and guessed at read. This field carries it.
///
/// # Why `Option`, and not a bare `usize` with `serde(default)`
///
/// A `usize` default is `0`, which means "the first section" — so every report
/// cached before this field existed would deserialize to a CONFIDENT WRONG
/// answer, and a new one: previously the ambiguity was only in `find`'s
/// tie-break, now it would be stamped into the data. `Option` keeps the legacy
/// path explicit: `None` means "written before the index existed", and
/// [`paragraph_at`] falls back to the old `find` behaviour for exactly those,
/// and only those.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Location {
    pub section: SectionKind,
    pub paragraph: usize,
    /// Index into [`ExtractionResult::sections`] — the section this location was
    /// BUILT from. `None` only for data written before this field existed.
    ///
    /// Declared LAST so the derived `Ord` still orders by `(section, paragraph)`
    /// first; `validate.rs` keys `BTreeSet`/`BTreeMap` on `Location` and that
    /// ordering is load-bearing for its deterministic output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_index: Option<usize>,
}

impl Location {
    /// The producer's constructor: this location was built while walking
    /// `sections[index]`, so the index is known and recorded.
    pub fn in_section(section: SectionKind, index: usize, paragraph: usize) -> Self {
        Self { section, paragraph, section_index: Some(index) }
    }

    /// **The legacy/ambiguous constructor: kind only, no index.**
    ///
    /// [`paragraph_at`] will resolve this by `find`, taking the FIRST section of
    /// the kind — the pre-§11-D169 behaviour, which is wrong whenever the kind
    /// repeats. Use [`Location::in_section`] anywhere the index is available.
    /// This exists for test fixtures and for decoding data written before
    /// `section_index` existed.
    pub fn by_kind(section: SectionKind, paragraph: usize) -> Self {
        Self { section, paragraph, section_index: None }
    }

    /// True when this address cannot distinguish between sections of its kind.
    pub fn is_ambiguous(&self) -> bool {
        self.section_index.is_none()
    }
}

/// The paragraph text at `loc`, or `None` if the location does not resolve.
///
/// # It must mirror the PRODUCER, and it does
///
/// `paragraph` is an index into ONE SECTION's `paragraphs`, not a document-wide
/// count — `extract_from_text` builds it with `section.paragraphs.iter()
/// .enumerate()`, so the counter restarts at every section. This function is the
/// inverse of that loop and of nothing else.
///
/// # ONE definition, producer AND consumer
///
/// This was `validate::paragraph`, private, and it is *the text the five rules
/// evaluate* — `max_group_count`, the overclaim scan, the effect-size scan and
/// the causal scan all read it. Promoting it means the rule that raises a flag
/// and the report that quotes the flag read the SAME string by construction, so
/// a quotation can never disagree with the finding it illustrates. §34.3's
/// one-predicate-producer-and-checker shape, applied again.
///
/// # A REPEATED `SectionKind` USED TO resolve to the FIRST section — §11 D169
///
/// `split_document` emits one section per recognised heading, so a document with
/// two headings that classify alike yields two sections of one kind, and a
/// `Location` carrying only the kind is not a unique address. This resolved by
/// `find`, taking the first.
///
/// **The paragraph that used to sit here said that was deliberate, and it was
/// RIGHT about faithfulness — which is exactly why the defect survived.** Its
/// argument was that `validate.rs` resolves a `Location` the same way, so a
/// repeated kind already makes the rule evaluate the wrong paragraph, and
/// matching that behaviour makes the quotation faithful: the report shows the
/// text the engine read. Every word of that is true. It is also why nothing
/// looked wrong — **a wrong finding was displayed beside the wrong paragraph
/// that produced it, and the two agreed.** An internally consistent artefact,
/// wrong at the premise, is the §14 v6 pattern at the scale of a single
/// finding.
///
/// Measured before changing it: **283 of 869 statistical claims** across 20 real
/// manuscripts resolved to a paragraph not containing them (also 79 of 414
/// tables and 584 of 2920 citations). [`Location::section_index`] now carries
/// what the producer always knew, and the `find` path above is reached only by
/// data written before that field existed.
///
/// > **The reporting layer deliberately preserves the existing `Location`
/// > semantics and makes any repeated-`SectionKind` ambiguity VISIBLE in the
/// > rendered report rather than silently masking it. The ambiguity originates
/// > in the EXTRACTION MODEL, not in the reporting layer.**
pub fn paragraph_at<'a>(result: &'a ExtractionResult, loc: &Location) -> Option<&'a str> {
    // THE INDEX, when the producer recorded one (§11 D169). Exact, and the kind
    // is verified rather than trusted: an index into a DIFFERENT extraction
    // would otherwise resolve silently to whatever sits at that position.
    if let Some(i) = loc.section_index {
        return result
            .sections
            .get(i)
            .filter(|s| s.kind == loc.section)
            .and_then(|s| s.paragraphs.get(loc.paragraph))
            .map(String::as_str);
    }
    // LEGACY ONLY — data written before `section_index` existed. Takes the first
    // section of the kind, which is wrong whenever the kind repeats; that is the
    // behaviour D169 measured and replaced, kept here so stored reports resolve
    // exactly as they did when they were written.
    result
        .sections
        .iter()
        .find(|s| s.kind == loc.section)
        .and_then(|s| s.paragraphs.get(loc.paragraph))
        .map(String::as_str)
}

/// **Where in the manuscript a given line of text sits.**
///
/// The inverse of [`paragraph_at`]: given text the engine worked over, find the
/// `Location` that would resolve back to it.
///
/// # Why this exists — §12.1's equation GAP
///
/// Equation findings shipped with `location: None`, marked GAP at their
/// construction site: the engine works over LINES and the OMML reader carries a
/// `.docx` paragraph index, while [`Location::paragraph`] is an index WITHIN a
/// section. The two do not compose, so a researcher got the equation quoted
/// verbatim and no anchor into their manuscript.
///
/// **Threading the OMML index through was never the cheap fix.** Every
/// equation finding already carries its `source_line` — the manuscript's own
/// text — and a text search over the paragraphs the extractor produced answers
/// the same question without a second index to keep in step. This is the third
/// time this shape has been the answer in this crate: required statements and
/// the ethics subject sentence both moved from a structural lookup to a text
/// search, for the same reason.
///
/// Matching is on the TRIMMED line, and a line short enough to appear in many
/// paragraphs is refused rather than anchored to the first — an anchor that is
/// probably wrong is worse than none, which is §9 [v5]'s own argument for
/// refusing `.docx` page reconstruction.
pub fn locate_line(result: &ExtractionResult, line: &str) -> Option<Location> {
    let needle = line.trim();
    // Below this, a "line" is a fragment that could sit anywhere. Measured on
    // the corpus: equation source lines run 8–120 characters, and the short end
    // is things like `n = N/(1+Ne²)`.
    if needle.chars().count() < MIN_LOCATABLE_LINE {
        return None;
    }
    let mut found: Option<Location> = None;
    for section in &result.sections {
        for (i, para) in section.paragraphs.iter().enumerate() {
            if para.contains(needle) {
                if found.is_some() {
                    // The same text in two places: anchoring to the first would
                    // be the `paragraph_at` ambiguity defect, made here instead
                    // of inherited.
                    return None;
                }
                found = Some(Location { section: section.kind, paragraph: i, section_index: None });
            }
        }
    }
    found
}

/// The shortest line [`locate_line`] will anchor. Below it a match says more
/// about how common the text is than about where the finding belongs.
pub const MIN_LOCATABLE_LINE: usize = 8;

/// A span of the document a rule searches — §47.1's WINDOW form, made explicit.
///
/// # Why a REGION and not a `Location`
///
/// Rule 3 asks *"is an effect size reported NEAR this p-value"*, which is a
/// question about a span. Answering it with a `Location` equality test would
/// answer a different question — *"trust whatever this `Location` currently
/// means"* — and would move rule 3 from §47.1's WINDOW family into its KEY
/// family, where a boundary change flips a `bool` with no "nearly" to absorb it.
///
/// **Today a region is exactly one paragraph, so the two are behaviourally
/// identical.** They diverge when item 1b decides what a region should be, which
/// is precisely when the coupling would otherwise have bitten. The type exists so
/// that change is a widening HERE rather than a semantic change at every caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub section: SectionKind,
    /// Inclusive paragraph range.
    pub first_paragraph: usize,
    pub last_paragraph: usize,
}

impl Region {
    /// The single paragraph at `loc` — the only region shape that exists today.
    pub fn paragraph(loc: &Location) -> Self {
        Region {
            section: loc.section,
            first_paragraph: loc.paragraph,
            last_paragraph: loc.paragraph,
        }
    }

    pub fn contains(&self, loc: &Location) -> bool {
        loc.section == self.section
            && loc.paragraph >= self.first_paragraph
            && loc.paragraph <= self.last_paragraph
    }
}

/// Locations at which an effect size was EXTRACTED — a typed `Stat::EffectSize`
/// with a value, not a text match.
///
/// # ONE definition, two consumers (§42's shape, applied a third time)
///
/// `validate.rs`'s MissingEffectSize rule and the report's Reported/Missing
/// split both ask whether an effect size accompanies a statistic. Before this
/// they asked it two different ways — the rule scanned TEXT with
/// `EFFECT_SIZE_ALTERNATION`, the report joined on typed locations — so the
/// report could list a statistic as "reported without an effect size" that the
/// rule had declined to flag. **Now both read this.**
pub fn effect_size_locations(result: &ExtractionResult) -> Vec<Location> {
    result
        .statistics
        .iter()
        .filter(|s| matches!(s.stat, stats::Stat::EffectSize { .. }))
        .map(|s| s.location.clone())
        .collect()
}

/// Whether a typed effect size falls within `region`.
///
/// # Why this replaced a text scan — ARCHITECTURE_TRACE §48, §49
///
/// The rule used to ask `EFFECT_SIZE_ALTERNATION.is_match(paragraph)`, which
/// answers *"is an effect-size TOKEN present"*. That is not the question:
/// measured on a pharmaceutical paper, `\bf2\b` (Cohen's f²) matched formulation
/// BATCH LABELS — `F1, F2, F3` — and suppressed **six** real MissingEffectSize
/// findings. A token is not a reported effect size; a typed extraction with a
/// value is.
///
/// **This is the whole of shape 3**: detection consults the same typed data
/// extraction produces, over a REGION rather than at a point.
pub fn has_effect_size_in(result: &ExtractionResult, region: &Region) -> bool {
    result
        .statistics
        .iter()
        .any(|s| matches!(s.stat, stats::Stat::EffectSize { .. }) && region.contains(&s.location))
}

/// A referenced table (e.g. "Table 1") and its caption, if the paragraph is
/// the caption itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRef {
    pub label: String,
    pub caption: Option<String>,
    pub location: Location,
}

/// The full typed result of extracting one manuscript.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub title: Option<String>,
    pub sections: Vec<Section>,
    pub statistics: Vec<StatClaim>,
    pub citations: Vec<Citation>,
    pub references: Vec<Reference>,
    pub tables: Vec<TableRef>,
    /// Optional scientific-understanding layer. Always constructed locally;
    /// cloud stages only receive bounded summaries derived from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scientific: Option<Arc<ScientificExtraction>>,
}

/// What an extraction run should compute beyond the base structure.
///
/// The Stage-1 scientific extractors are OPT-IN. They landed wired into every
/// call, which made every existing caller — AI Check, PublishReady, the audit
/// pre-pass, the paper corpus — pay for four extra passes over the manuscript
/// to populate a field none of them reads. The base extraction is on the hot
/// path of features that run over hundreds of pages; the scientific layer is
/// consumed by one that does not exist yet. Charging the first for the second
/// is the wrong default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExtractOptions {
    /// Run `claims`/`variables`/`methods`/`datasets` and populate
    /// [`ExtractionResult::scientific`].
    pub scientific: bool,
}

impl ExtractOptions {
    /// Base structure only — the historical behaviour.
    pub fn base() -> Self {
        Self { scientific: false }
    }
    /// Base structure plus the Stage-1 scientific layer.
    pub fn with_scientific() -> Self {
        Self { scientific: true }
    }
}

/// Extract structure and claims from already-parsed manuscript plaintext.
///
/// Base structure only. Callers that want the scientific layer ask for it with
/// [`extract_from_text_with`].
#[tracing::instrument(skip(text), fields(len = text.len()))]
pub fn extract_from_text(text: &str) -> ExtractionResult {
    extract_from_text_with(text, ExtractOptions::base())
}

/// [`extract_from_text`], with the optional stages named explicitly.
pub fn extract_from_text_with(text: &str, opts: ExtractOptions) -> ExtractionResult {
    let (title, secs) = sections::split_document(text);

    let mut statistics = Vec::new();
    let mut citations = Vec::new();
    let mut tables = Vec::new();
    let mut references = Vec::new();

    for (s_idx, section) in secs.iter().enumerate() {
        if section.kind == SectionKind::References {
            references = citations::parse_reference_list(section);
            // reference-list entries are parsed structurally; don't also scan
            // them as in-text citations or statistics.
            continue;
        }
        for (p_idx, paragraph) in section.paragraphs.iter().enumerate() {
            let loc = Location::in_section(section.kind, s_idx, p_idx);
            statistics.extend(stats::extract(paragraph, &loc));
            citations.extend(citations::extract_in_text(paragraph, &loc));
            if let Some(t) = detect_table(paragraph, &loc) {
                tables.push(t);
            }
        }
    }

    let mut result = ExtractionResult {
        title,
        sections: secs,
        statistics,
        citations,
        references,
        tables,
        ..Default::default()
    };

    if !opts.scientific {
        return result;
    }

    // Stage 1 deterministic scientific extraction. Best-effort: never fails the run.
    let claims = claims::extract_claims(&result);
    let variables = variables::extract_variables(&result, &claims);
    let mut methods = methods::extract_methods(&result, &claims, &variables);
    let datasets = datasets::extract_datasets(&result, &claims, &variables, &methods);

    // Cross-link datasets back into methods.
    for m in &mut methods {
        m.associated_datasets = datasets
            .iter()
            .filter(|d| d.associated_methods.iter().any(|mid| mid.0 == m.id.0))
            .map(|d| d.id.clone())
            .collect();
        m.associated_datasets.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let scientific = ScientificExtraction {
        claims,
        variables,
        methods,
        datasets,
        ..Default::default()
    };
    if !scientific.claims.is_empty()
        || !scientific.variables.is_empty()
        || !scientific.methods.is_empty()
        || !scientific.datasets.is_empty()
    {
        result.scientific = Some(Arc::new(scientific));
    }

    result
}

/// A paragraph that begins "Table N ..." is treated as that table's caption.
///
/// # TWO MEASURED DEFECTS, recorded in §11 D167 and NOT fixed here
///
/// Both were found building RT4's corpus over 20 real manuscripts. They are
/// defects in this function, independent of the lane that surfaced them.
///
/// 1. **A contents page is a run of matches.** Any paragraph opening `Table N`
///    qualifies, so a thesis list-of-tables produces one `TableRef` per entry:
///    **237 of 414 detections (57%)** across the corpus were front-matter rows
///    such as `"Table 2:Evolution of Small-Scale Industry Definition in India
///    47"`, where `47` is a page number and no body follows. Anything reading
///    `ExtractionResult::tables` as a TABLE COUNT is reading a number roughly
///    2.3x too large — `report::table_findings` does exactly that.
/// 2. **Prefix captioning swallows prose.** The caption is the remainder of the
///    matching paragraph, so *"Table 2 presents mediation pathway coefficients.
///    In Path A, each one-level increase…"* — 500 characters of Results prose —
///    became that table's caption. A caption field that can hold a paragraph is
///    not a caption.
///
/// Fixing either is extractor work needing its own before/after measurement:
/// the discriminator for (1) is whether a BODY follows, which this function
/// cannot see from one paragraph.
fn detect_table(paragraph: &str, loc: &Location) -> Option<TableRef> {
    let re = &stats::regexes().table_caption;
    let caps = re.captures(paragraph.trim())?;
    let label = format!("Table {}", &caps[1]);
    let caption = caps.get(2).map(|m| m.as_str().trim()).filter(|s| !s.is_empty());
    Some(TableRef {
        label,
        caption: caption.map(String::from),
        location: loc.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Effect of Sleep on Memory Consolidation

Jane A. Smith, John Doe

Abstract
We tested whether sleep improves recall. In a randomized trial (n = 120),
sleep-deprived participants performed worse (p < 0.001).

Methods
Participants were assigned to two groups. We used an independent t-test to
compare recall scores. A one-way ANOVA assessed dose effects (n = 120).

Results
Table 1 summarizes the outcomes. Recall improved with sleep (95% CI: 1.2 to
3.4; p = 0.03). Smith et al. (2023) reported a similar effect. Earlier work
found no effect (Jones, 2019; Brown & Lee, 2020).

Discussion
Our findings align with prior reports (Smith et al., 2023).

References
Smith, J., Adams, R., & Clark, T. (2023). Sleep and memory in adults. Journal of Sleep, 12(3), 45-67. https://doi.org/10.1234/jsleep.2023.045
Jones, P. (2019). Memory under deprivation. Cognitive Science, 5(1), 10-20.
";

    #[test]
    fn detects_all_imrad_sections() {
        let r = extract_from_text(SAMPLE);
        let kinds: Vec<_> = r.sections.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SectionKind::Abstract));
        assert!(kinds.contains(&SectionKind::Methods));
        assert!(kinds.contains(&SectionKind::Results));
        assert!(kinds.contains(&SectionKind::Discussion));
        assert!(kinds.contains(&SectionKind::References));
        assert_eq!(r.title.as_deref(), Some("Effect of Sleep on Memory Consolidation"));
    }

    #[test]
    fn extracts_pvalues_with_location() {
        let r = extract_from_text(SAMPLE);
        let pvals: Vec<_> = r
            .statistics
            .iter()
            .filter_map(|s| match &s.stat {
                Stat::PValue { value, .. } => Some((*value, s.location.section)),
                _ => None,
            })
            .collect();
        // p < 0.001 in Abstract, p = 0.03 in Results
        assert!(pvals.iter().any(|(v, sec)| (*v - 0.001).abs() < 1e-9
            && *sec == SectionKind::Abstract));
        assert!(pvals.iter().any(|(v, sec)| (*v - 0.03).abs() < 1e-9
            && *sec == SectionKind::Results));
    }

    #[test]
    fn extracts_test_names_ci_and_sample_size() {
        let r = extract_from_text(SAMPLE);
        let tests: Vec<_> = r
            .statistics
            .iter()
            .filter_map(|s| match &s.stat {
                Stat::Test { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert!(tests.iter().any(|t| t == "t-test"));
        assert!(tests.iter().any(|t| t == "ANOVA"));

        assert!(r.statistics.iter().any(|s| matches!(&s.stat,
            Stat::ConfidenceInterval { low, high, .. } if (*low - 1.2).abs() < 1e-9 && (*high - 3.4).abs() < 1e-9)));
        assert!(r.statistics.iter().any(|s| matches!(&s.stat,
            Stat::SampleSize { n, .. } if *n == 120)));
    }

    #[test]
    fn extracts_both_citation_styles() {
        let r = extract_from_text(SAMPLE);
        // narrative: "Smith et al. (2023)"
        assert!(r.citations.iter().any(|c| c.style == CitationStyle::Narrative
            && c.authors.contains("Smith")
            && c.year == Some(2023)));
        // parenthetical: "(Jones, 2019; Brown & Lee, 2020)"
        assert!(r.citations.iter().any(|c| c.style == CitationStyle::Parenthetical
            && c.authors.contains("Jones")
            && c.year == Some(2019)));
        assert!(r.citations.iter().any(|c| c.style == CitationStyle::Parenthetical
            && c.authors.contains("Brown")
            && c.year == Some(2020)));
    }

    #[test]
    fn parses_reference_list_with_doi() {
        let r = extract_from_text(SAMPLE);
        assert_eq!(r.references.len(), 2);
        let first = &r.references[0];
        assert!(first.authors.contains("Smith"));
        assert_eq!(first.year, Some(2023));
        assert_eq!(first.doi.as_deref(), Some("10.1234/jsleep.2023.045"));
        assert!(first.title.as_deref().unwrap().contains("Sleep and memory"));
        // second reference has no DOI
        assert_eq!(r.references[1].doi, None);
    }

    #[test]
    fn detects_table_reference() {
        let r = extract_from_text(SAMPLE);
        assert!(r.tables.iter().any(|t| t.label == "Table 1"));
    }
}
