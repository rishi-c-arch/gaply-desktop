//! **The comparable corpus, and what a convention can honestly be derived
//! from.**
//!
//! §7 describes a convention profile of five metrics — *"median length, typical
//! section set, figure count, whether methods precede or follow results,
//! statistical reporting style"* — derived from recently published papers.
//!
//! # MEASURED FIRST (14 Sep 2026), and three of the five are not there
//!
//! 100 recent records each from PLOS ONE and Nature Medicine via OpenAlex:
//!
//! | field | PLOS ONE | Nature Medicine |
//! |---|---:|---:|
//! | title, publication_date, type | 100% | 100% |
//! | abstract | 100% | **63%** |
//! | `biblio` first/last page | 100% | 100% |
//! | **page count derivable** | **0%** | 100% |
//! | **section structure** | **0%** | **0%** |
//! | **figure count** | **0%** | **0%** |
//! | `has_fulltext == true` | 40/50 | **22/50** |
//!
//! **`section_set`, `figure_count` and `methods_position` are not in OpenAlex
//! at all.** They need the full text parsed, which is available for 74% of
//! PLOS ONE papers and 36% of Nature Medicine's — and costs one PDF fetch and
//! parse per paper. Until that exists they are [`ConventionStatus::Unavailable`],
//! which is a real answer (§3.4: *"searched, nothing authoritative found"*) and
//! the schema enforces it: `migrations` v23 refuses an `inferred` row with
//! `n = 0`.
//!
//! # `length` IS A PAGE COUNT, AND §7 IMPLIES A WORD COUNT
//!
//! §7's example is *"Median recent paper is 4,620 words."* What the source
//! supplies is `biblio.first_page`/`last_page`, and only where a journal uses
//! real page ranges:
//!
//! ```text
//! Nature Medicine  first_page "1776"      last_page "1783"      → 8 pages
//! PLOS ONE         first_page "e0319586"  last_page "e0319586"  → an ARTICLE ID
//! ```
//!
//! A page count is not a word count, and presenting one as the other is §11
//! D161's failure — *what a number counts* — with a third surface. So the
//! metric records its unit in `detail`, and a journal whose `first_page` is an
//! article identifier gets `Unavailable` rather than a length of 1.
//!
//! # "Recently published", never "accepted"
//!
//! Acceptance dates are not in the source. `publication_date` is what OpenAlex
//! has, and naming it anything else would claim knowledge of a date nobody
//! supplied. Pinned by a test over the rendered output.

use serde::{Deserialize, Serialize};

/// One published paper, as the public record supplies it. **Nothing here comes
/// from any user's manuscript** — Prompt 5's constraint, and the reason this
/// type has no field that could carry one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPaper {
    pub id: String,
    pub title: String,
    /// Reconstructed from OpenAlex's inverted index. Empty when absent — 37% of
    /// Nature Medicine's recent records have none.
    pub abstract_text: String,
    /// As published. NEVER an acceptance date; see the module header.
    pub publication_date: String,
    /// OpenAlex `type`: `article`, `review`, …
    pub work_type: String,
    pub first_page: Option<String>,
    pub last_page: Option<String>,
}

impl PublishedPaper {
    /// Pages, when the journal uses real page numbers.
    ///
    /// `None` for a journal whose `first_page` is an article identifier
    /// (`e0319586`) — which is NOT a one-page paper, and returning 1 there
    /// would be a fabricated length.
    pub fn page_count(&self) -> Option<u32> {
        let a: i64 = self.first_page.as_ref()?.parse().ok()?;
        let z: i64 = self.last_page.as_ref()?.parse().ok()?;
        (z >= a).then(|| (z - a + 1) as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConventionMetric {
    Length,
    SectionSet,
    FigureCount,
    MethodsPosition,
    StatsStyle,
}

impl ConventionMetric {
    /// The exact string the schema's CHECK accepts.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConventionMetric::Length => "length",
            ConventionMetric::SectionSet => "section_set",
            ConventionMetric::FigureCount => "figure_count",
            ConventionMetric::MethodsPosition => "methods_position",
            ConventionMetric::StatsStyle => "stats_style",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConventionStatus {
    /// Derived from the corpus, with an `n`.
    Inferred,
    /// Searched, nothing derivable. **A real answer, not a missing row.**
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedConvention {
    pub metric: ConventionMetric,
    pub median: Option<f64>,
    pub iqr_low: Option<f64>,
    pub iqr_high: Option<f64>,
    pub n: usize,
    /// What the number COUNTS, in words. A length of 10 means nothing without
    /// "pages"; §11 D161.
    pub detail: String,
    pub status: ConventionStatus,
}

impl DerivedConvention {
    fn unavailable(metric: ConventionMetric, why: &str) -> DerivedConvention {
        DerivedConvention {
            metric,
            median: None,
            iqr_low: None,
            iqr_high: None,
            n: 0,
            detail: why.to_string(),
            status: ConventionStatus::Unavailable,
        }
    }
}

/// Corpus size bounds. **In config, not code** (Prompt 5): below `minimum` a
/// convention is `Unavailable` however good the data looks.
#[derive(Debug, Clone, Deserialize)]
pub struct CorpusBounds {
    pub minimum: usize,
    pub target: usize,
    pub maximum: usize,
}

impl CorpusBounds {
    pub fn from_json(s: &str) -> Result<CorpusBounds, crate::error::GaplyError> {
        let b: CorpusBounds = serde_json::from_str(s)
            .map_err(|e| crate::error::GaplyError::Config(format!("corpus bounds: {e}")))?;
        if b.minimum == 0 || b.minimum > b.target || b.target > b.maximum {
            return Err(crate::error::GaplyError::Config(
                "corpus bounds must satisfy 0 < minimum <= target <= maximum".into(),
            ));
        }
        Ok(b)
    }
}

fn quartiles(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    let med = if n % 2 == 0 { (v[n / 2 - 1] + v[n / 2]) / 2.0 } else { v[n / 2] };
    (v[n / 4], med, v[(3 * n / 4).min(n - 1)])
}

/// Statistical reporting style, from the abstracts the corpus supplies.
///
/// **A partial proxy, and labelled as one.** §7 wants the style of the paper;
/// this sees only the abstract, which is where a journal's house style is most
/// visible and least complete. The `detail` says so, so a reader is never left
/// to assume it was the full text.
fn stats_style(papers: &[PublishedPaper]) -> DerivedConvention {
    let with_abstract: Vec<&PublishedPaper> =
        papers.iter().filter(|p| !p.abstract_text.is_empty()).collect();
    if with_abstract.is_empty() {
        return DerivedConvention::unavailable(
            ConventionMetric::StatsStyle,
            "no abstracts in the corpus",
        );
    }
    let (mut p_only, mut ci_only, mut both, mut neither) = (0usize, 0usize, 0usize, 0usize);
    for p in &with_abstract {
        let t = p.abstract_text.to_lowercase();
        let has_p = t.contains("p <") || t.contains("p<") || t.contains("p =") || t.contains("p=")
            || t.contains("p-value");
        let has_ci = t.contains("confidence interval") || t.contains(" ci ") || t.contains("95% ci")
            || t.contains("credible interval");
        match (has_p, has_ci) {
            (true, true) => both += 1,
            (true, false) => p_only += 1,
            (false, true) => ci_only += 1,
            (false, false) => neither += 1,
        }
    }
    let n = with_abstract.len();
    DerivedConvention {
        metric: ConventionMetric::StatsStyle,
        median: None,
        iqr_low: None,
        iqr_high: None,
        n,
        detail: format!(
            "of {n} recently published abstracts: {both} report both p-values and confidence \
             intervals, {p_only} p-values only, {ci_only} intervals only, {neither} neither. \
             Read from ABSTRACTS, not full text."
        ),
        status: ConventionStatus::Inferred,
    }
}

/// **Is this a research paper, for the purpose of a convention?**
///
/// Prompt 5: *"FILTER to comparable papers — same article type, same or
/// adjacent study design, within a date window, above an embedding-similarity
/// threshold."* The similarity half is analysis-time and needs a manuscript;
/// this is the half that belongs to corpus BUILD time, and without it the
/// distribution is a mixture rather than a distribution.
///
/// **Measured on Nature Medicine's 200 most recent papers:**
///
/// | | n | median pages | range |
/// |---|---:|---:|---|
/// | with an abstract | 41 | **9** | 1–16 |
/// | without an abstract | 39 | **2** | 1–17 |
///
/// The unfiltered median was 7 with an IQR of 2–11, which is not a typical
/// paper length — it is two populations averaged. A journal's Correspondence,
/// News & Views and editorials carry no abstract; its research papers do.
/// OpenAlex's own `type` cannot separate them: it reports `article` for 97 of
/// 100 recent Nature Medicine records, short-form items included.
///
/// **This is a proxy and is named as one.** A research paper without an
/// abstract is excluded, and a long editorial that has one is included. It is
/// the best separator the source supplies, and the alternative — using the
/// page count itself to decide what is a research paper — would be circular.
pub fn is_research_paper(p: &PublishedPaper) -> bool {
    !p.abstract_text.is_empty()
}

/// Derive every convention §7 names, marking as `Unavailable` the ones the
/// source cannot supply.
///
/// **All five are always returned.** A metric omitted because it could not be
/// computed is indistinguishable from one nobody asked for, and §3.4 requires
/// absence to be a status rather than a silence.
pub fn derive_conventions(papers: &[PublishedPaper], bounds: &CorpusBounds) -> Vec<DerivedConvention> {
    let mut out = Vec::new();
    // Conventions describe the journal's RESEARCH papers. See
    // `is_research_paper` for the measurement behind this line.
    let comparable: Vec<&PublishedPaper> = papers.iter().filter(|p| is_research_paper(p)).collect();

    // --- length -----------------------------------------------------------
    let pages: Vec<f64> =
        comparable.iter().filter_map(|p| p.page_count()).map(|n| n as f64).collect();
    out.push(if pages.len() < bounds.minimum {
        DerivedConvention::unavailable(
            ConventionMetric::Length,
            &format!(
                "{} of {} comparable papers ({} fetched) carry a usable page range \
                 (minimum {}). A journal that identifies articles by number rather than \
                 page — PLOS ONE's `e0319586` — supplies no length.",
                pages.len(),
                comparable.len(),
                papers.len(),
                bounds.minimum
            ),
        )
    } else {
        let n = pages.len();
        let (lo, med, hi) = quartiles(pages);
        DerivedConvention {
            metric: ConventionMetric::Length,
            median: Some(med),
            iqr_low: Some(lo),
            iqr_high: Some(hi),
            n,
            // The unit, stated. §7's example is a word count; this is not one.
            detail: format!(
                "PAGES per recently published RESEARCH paper, from the page range (n = {n} of \
                 {} fetched; items without an abstract are excluded — see `is_research_paper`). \
                 This is a page count, not a word count.",
                papers.len()
            ),
            status: ConventionStatus::Inferred,
        }
    });

    // --- the three the source does not carry ------------------------------
    for (metric, why) in [
        (ConventionMetric::SectionSet,
         "OpenAlex carries no section structure. Deriving it needs the open-access full text \
          parsed, available for 74% of PLOS ONE and 36% of Nature Medicine recent papers."),
        (ConventionMetric::FigureCount,
         "OpenAlex carries no figure count. Same full-text requirement as the section set."),
        (ConventionMetric::MethodsPosition,
         "Methods position is a property of the section order, which OpenAlex does not carry."),
    ] {
        out.push(DerivedConvention::unavailable(metric, why));
    }

    // --- statistical reporting style, from abstracts ----------------------
    let owned: Vec<PublishedPaper> = comparable.iter().map(|p| (*p).clone()).collect();
    let style = stats_style(&owned);
    out.push(if style.n < bounds.minimum {
        DerivedConvention::unavailable(
            ConventionMetric::StatsStyle,
            &format!("{} abstracts, below the corpus minimum of {}", style.n, bounds.minimum),
        )
    } else {
        style
    });

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> CorpusBounds {
        CorpusBounds { minimum: 10, target: 50, maximum: 200 }
    }

    fn paper(first: &str, last: &str, abs: &str) -> PublishedPaper {
        PublishedPaper {
            id: format!("W{first}"),
            title: "A paper".into(),
            abstract_text: abs.into(),
            publication_date: "2025-06-01".into(),
            work_type: "article".into(),
            first_page: Some(first.into()),
            last_page: Some(last.into()),
        }
    }

    /// **An article identifier is not a page range.** PLOS ONE's `biblio` is
    /// `first_page: "e0319586", last_page: "e0319586"`; reading that as a
    /// one-page paper would be a fabricated length in a field §7 renders as a
    /// journal's typical article size.
    #[test]
    fn an_article_identifier_yields_no_page_count() {
        assert_eq!(paper("e0319586", "e0319586", "").page_count(), None);
        // A real range does.
        assert_eq!(paper("1776", "1783", "").page_count(), Some(8));
        // …and a reversed one does not guess.
        assert_eq!(paper("1783", "1776", "").page_count(), None);
    }

    /// A journal that numbers articles rather than pages gets `Unavailable`
    /// for length — with the reason naming the actual obstacle.
    #[test]
    fn a_journal_without_page_ranges_has_no_length_convention() {
        let papers: Vec<PublishedPaper> =
            (0..40).map(|i| paper(&format!("e03{i:05}"), &format!("e03{i:05}"), "p < 0.05")).collect();
        let got = derive_conventions(&papers, &bounds());
        let len = got.iter().find(|c| c.metric == ConventionMetric::Length).unwrap();
        assert_eq!(len.status, ConventionStatus::Unavailable);
        assert_eq!(len.n, 0);
        assert!(len.detail.contains("page range"), "{}", len.detail);
        assert!(len.median.is_none());
    }

    /// Nature Medicine's shape: real page ranges, a derivable median — and the
    /// unit stated, because a page count is not a word count.
    #[test]
    fn page_ranges_give_a_length_whose_unit_is_named() {
        let papers: Vec<PublishedPaper> = (0..40)
            .map(|i| paper(&format!("{}", 1000 + i * 10), &format!("{}", 1000 + i * 10 + 7), "p < 0.05"))
            .collect();
        let got = derive_conventions(&papers, &bounds());
        let len = got.iter().find(|c| c.metric == ConventionMetric::Length).unwrap();
        assert_eq!(len.status, ConventionStatus::Inferred);
        assert_eq!(len.median, Some(8.0));
        assert_eq!(len.n, 40);
        assert!(len.detail.to_lowercase().contains("page"), "the unit must be named: {}", len.detail);
        assert!(len.detail.contains("not a word count"), "{}", len.detail);
    }

    /// **All five metrics are always returned.** A metric omitted because it
    /// could not be computed is indistinguishable from one nobody asked for.
    #[test]
    fn every_metric_is_reported_including_the_ones_the_source_cannot_supply() {
        let papers: Vec<PublishedPaper> =
            (0..40).map(|i| paper(&format!("{}", 100 + i), &format!("{}", 105 + i), "p < 0.05 (95% CI 1.1-2.0)")).collect();
        let got = derive_conventions(&papers, &bounds());
        let metrics: Vec<&str> = got.iter().map(|c| c.metric.as_str()).collect();
        for m in ["length", "section_set", "figure_count", "methods_position", "stats_style"] {
            assert!(metrics.contains(&m), "{m} missing: {metrics:?}");
        }
        // The three OpenAlex cannot supply say WHY, not merely that they are absent.
        for m in [ConventionMetric::SectionSet, ConventionMetric::FigureCount,
                  ConventionMetric::MethodsPosition] {
            let c = got.iter().find(|c| c.metric == m).unwrap();
            assert_eq!(c.status, ConventionStatus::Unavailable);
            assert!(c.detail.len() > 40, "a reason, not a shrug: {}", c.detail);
        }
    }

    /// Below the configured minimum, a convention is UNAVAILABLE however clean
    /// the data is. An inference from six papers is not an inference.
    /// **A mixture is not a distribution.** Measured on Nature Medicine: 41
    /// papers with an abstract have a median of 9 pages, 39 without have a
    /// median of 2, and the unfiltered median of 7 describes neither
    /// population. Without the filter, a journal's Correspondence drags its
    /// "typical paper length" down by two pages.
    #[test]
    fn short_form_items_are_excluded_from_the_length_convention() {
        let mut papers: Vec<PublishedPaper> = (0..20)
            .map(|i| paper(&format!("{}", 1000 + i * 20), &format!("{}", 1008 + i * 20),
                           "We report a randomised trial. p < 0.05"))
            .collect();
        // …and twenty two-page items with no abstract, as a journal publishes.
        papers.extend((0..20).map(|i| paper(&format!("{}", 500 + i * 5), &format!("{}", 501 + i * 5), "")));

        let got = derive_conventions(&papers, &bounds());
        let len = got.iter().find(|c| c.metric == ConventionMetric::Length).unwrap();
        assert_eq!(len.n, 20, "only the research papers count");
        assert_eq!(len.median, Some(9.0), "9 pages, not the 5.5 of the mixture");
        assert!(len.detail.contains("without an abstract are excluded"), "{}", len.detail);

        // The statistical style is read from the same filtered set.
        let style = got.iter().find(|c| c.metric == ConventionMetric::StatsStyle).unwrap();
        assert_eq!(style.n, 20);
    }

    #[test]
    fn a_corpus_below_the_minimum_yields_no_convention() {
        let papers: Vec<PublishedPaper> = (0..6)
            .map(|i| paper(&format!("{}", 100 + i * 10), &format!("{}", 107 + i * 10), "p < 0.05"))
            .collect();
        let got = derive_conventions(&papers, &bounds());
        assert!(got.iter().all(|c| c.status == ConventionStatus::Unavailable), "{got:#?}");
    }

    /// The statistical-style convention is read from ABSTRACTS and says so, so
    /// a reader never assumes it was the full text.
    #[test]
    fn the_statistical_style_convention_names_its_source() {
        let mut papers: Vec<PublishedPaper> = (0..20)
            .map(|i| paper(&format!("{}", 100 + i), &format!("{}", 105 + i),
                           "The odds ratio was 1.8 (95% CI 1.2-2.7), p < 0.001."))
            .collect();
        papers.extend((0..10).map(|i| paper(&format!("{}", 500 + i), &format!("{}", 505 + i),
                                            "Differences were significant at p = 0.03.")));
        let got = derive_conventions(&papers, &bounds());
        let s = got.iter().find(|c| c.metric == ConventionMetric::StatsStyle).unwrap();
        assert_eq!(s.status, ConventionStatus::Inferred);
        assert_eq!(s.n, 30);
        assert!(s.detail.contains("20 report both"), "{}", s.detail);
        assert!(s.detail.contains("ABSTRACTS"), "the proxy must be labelled: {}", s.detail);
    }

    /// **Prompt 5's named test: the word "accepted" must not appear in
    /// convention output.** OpenAlex supplies a publication date and nothing
    /// else; naming it an acceptance date claims a fact nobody supplied.
    #[test]
    fn the_word_accepted_never_appears_in_convention_output() {
        let papers: Vec<PublishedPaper> = (0..40)
            .map(|i| paper(&format!("{}", 100 + i), &format!("{}", 108 + i), "p < 0.05"))
            .collect();
        for c in derive_conventions(&papers, &bounds()) {
            let lower = c.detail.to_lowercase();
            assert!(!lower.contains("accepted"), "{}", c.detail);
            assert!(!lower.contains("acceptance"), "{}", c.detail);
        }
        // And the phrase that IS true appears where a reader would look.
        let len = derive_conventions(&papers, &bounds());
        assert!(len.iter().any(|c| c.detail.contains("recently published")));
    }

    #[test]
    fn corpus_bounds_come_from_config_and_must_be_ordered() {
        let b = CorpusBounds::from_json(r#"{"minimum":10,"target":50,"maximum":200}"#).unwrap();
        assert_eq!((b.minimum, b.target, b.maximum), (10, 50, 200));
        assert!(CorpusBounds::from_json(r#"{"minimum":0,"target":50,"maximum":200}"#).is_err());
        assert!(CorpusBounds::from_json(r#"{"minimum":60,"target":50,"maximum":200}"#).is_err());
    }
}
