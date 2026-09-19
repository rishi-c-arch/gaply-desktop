//! Statistical-claim extraction: p-values, confidence intervals, sample
//! sizes, and named statistical tests, each tagged with its location.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::extract::Location;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Stat {
    PValue { operator: String, value: f64, raw: String },
    ConfidenceInterval { level: Option<f64>, low: f64, high: f64, raw: String },
    SampleSize { n: i64, raw: String },
    Test { name: String, raw: String },
    /// A test STATISTIC with its degrees of freedom — `F(2, 57) = 4.82`.
    ///
    /// Distinct from `Test`, whose `raw` is the test NAME match ("one-way
    /// ANOVA"). The name told the report which test ran; this tells it what the
    /// test produced, which is what the Reported column needs.
    TestStatistic { name: String, value: f64, df: Vec<f64>, raw: String },
    /// A reported effect size with its VALUE — `Cohen's d = 0.42`.
    ///
    /// `validate.rs` has detected the PRESENCE of an effect size since the
    /// MissingEffectSize rule was written, but only as `is_match`, so no value
    /// was ever captured and the report could say "Missing: effect size" and
    /// never "Reported: Cohen's d = 0.42" (§31.2).
    EffectSize { name: String, value: f64, raw: String },
    /// A significance CRITERION — `p ≤ 0.05` used as a decision rule rather
    /// than as a reported result (ARCHITECTURE_TRACE §41, §42).
    ///
    /// # A SEPARATE VARIANT, not a flag on `PValue`
    ///
    /// Measured across the six `Stat` consumers: the three that DISPLAY or
    /// PERSIST a statistic match exhaustively, so a new variant breaks their
    /// build and forces a decision; the two that REASON over one carry a
    /// wildcard, so they skip it silently — which is exactly the fix. The match
    /// shapes had already encoded the display-versus-reason distinction before
    /// anyone named it.
    ///
    /// # ONE POSITIVE STATE
    ///
    /// This variant means *"established as a criterion"*. Everything remaining a
    /// `PValue` means *"NOT established as a criterion"* — which is not the same
    /// as *"established as a result"*. A distinct Result state would require a
    /// SECOND DETECTOR with independent evidence; until that capability exists,
    /// materialising one would encode certainty the system does not possess
    /// (ONTOLOGY §4.12).
    SignificanceThreshold { operator: String, value: f64, raw: String },
}

impl Stat {
    /// The verbatim text from which this statistic was extracted.
    pub fn raw(&self) -> &str {
        match self {
            Stat::PValue { raw, .. }
            | Stat::ConfidenceInterval { raw, .. }
            | Stat::SampleSize { raw, .. }
            | Stat::Test { raw, .. }
            | Stat::TestStatistic { raw, .. }
            | Stat::EffectSize { raw, .. }
            | Stat::SignificanceThreshold { raw, .. } => raw.as_str(),
        }
    }
}

/// Phrases whose presence in a p-value's SENTENCE establishes that the p-value
/// is a significance CRITERION.
///
/// # THE ONLY DEFINITION — `threshold_markers_are_owned_by_extraction` proves it
///
/// Three rules and three non-rule consumers read `Stat`. Any of them
/// re-deriving this decision from surrounding text would recreate the
/// duplication this work exists to remove, so the list lives here and the
/// classification is made ONCE, at extraction.
///
/// # CONSERVATIVE ON PURPOSE — the error directions are not symmetric
///
/// A marker that over-fires classifies a REPORTED result as a criterion, and the
/// rules then drop a real finding **silently** — an absence with no attribution
/// (§4.14). A marker that under-fires leaves today's behaviour unchanged. **The
/// harmful direction is over-inclusion**, so every entry is a phrase that states
/// a DECISION RULE by its own semantics, not merely a phrase observed near one.
///
/// Two candidates were REJECTED for that reason after being observed in the
/// corpus:
///
/// * **`"was detected"`** — *"a significant effect was detected (p = 0.03)"* is
///   an ordinary result sentence.
/// * **bare `"α"`** — *"Cronbach's α = 0.85"* is a reliability coefficient.
///
/// Rejecting them costs one of the six measured criteria (a conditional
/// *"where significant autocorrelation was detected"*). Under a one-sided design
/// that cost is coverage, never correctness.
///
/// **Three phrasings from three papers is a sample, not a vocabulary.** Recall is
/// unmeasured.
const THRESHOLD_MARKERS: &[&str] = &[
    // observed
    "significance level",
    "significance was determined at",
    "critical difference at",
    "not significant at",
    // same construction, unobserved — each states a decision rule outright
    "level of significance",
    "significance was set at",
    "significance was set to",
    "significance threshold",
    "considered significant at",
    "considered statistically significant at",
    "alpha level",
];

/// Whether the p-value beginning at byte offset `at` in `paragraph` is a
/// significance criterion.
///
/// **The input is the SENTENCE, not the paragraph** — ARCHITECTURE_TRACE §41.12.
/// In all six measured cases the marker and its p-value share a sentence, and a
/// sentence is the atom beneath every paragraph unit, so this classification is
/// unaffected by the format-dependent paragraph sizes item 1b addresses. The
/// paragraph would also be wrong on its merits: a Methods paragraph declaring a
/// threshold and later reporting a result would classify both alike.
fn is_significance_criterion(paragraph: &str, at: usize) -> bool {
    let sentence = crate::extract::sentence::sentence_containing(paragraph, at).to_lowercase();
    THRESHOLD_MARKERS.iter().any(|m| sentence.contains(m))
}

/// The alternation naming every effect-size measure Gaply recognises.
///
/// # ONE SOURCE, two consumers
///
/// `validate::patterns().effect_size` (detection, for the MissingEffectSize
/// rule) and `regexes().effect_size_value` (extraction, with a value group) are
/// both BUILT FROM THIS. Two independently written patterns would be two
/// definitions of "what counts as an effect size", and they would drift — the
/// `matchTypeLabel` and `report_cache_key` shape (§31.24).
///
/// `effect_size_pattern_is_unchanged` pins the composed detection pattern to the
/// literal that shipped, so sharing the source did not alter which paragraphs
/// the rule fires on.
pub const EFFECT_SIZE_ALTERNATION: &str = concat!(
    r"cohen'?s\s*d|hedges'?\s*g|eta[\s-]*squared|η2|η²|partial\s+eta|",
    r"omega[\s-]*squared|cramer'?s\s*v|odds\s+ratio|hazard\s+ratio|risk\s+ratio|",
    // `cohen'?s\s*f2?` — the QUALIFIER is required. Bare `\bf2\b` was measured
    // matching formulation batch labels (F1…F4) and would equally match the
    // FDA/EMA dissolution similarity factor, written identically as `f2 = 65.2`
    // but ranging 0–100 against Cohen's f²'s 0.02–0.35. See §49.4.
    r"effect\s+size|cohen'?s\s*f2|cohen'?s\s*f²|",
    r"\bOR\s*=|\bHR\s*=|\bRR\s*=|\bd\s*=|\bg\s*=|\br\s*=|\bR2\b|R²"
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatClaim {
    #[serde(flatten)]
    pub stat: Stat,
    pub location: Location,
}

pub struct Regexes {
    pub p_value: Regex,
    pub confidence_interval: Regex,
    pub sample_size: Regex,
    pub sample_phrase: Regex,
    pub tests: Vec<(Regex, &'static str)>,
    /// `F(2, 57) = 4.82` — the statistic, not the test name.
    pub f_statistic: Regex,
    /// An effect-size measure followed by its value. Built from
    /// [`EFFECT_SIZE_ALTERNATION`].
    pub effect_size_value: Regex,
    pub heading_number: Regex,
    pub runin_heading: Regex,
    pub table_caption: Regex,
    pub doi: Regex,
    pub year_paren: Regex,
    pub year_bare: Regex,
    pub narrative_cite: Regex,
    pub paren_group: Regex,
    /// Candidate bracket group `[...]` — permissive capture of the inner text;
    /// `citations::parse_numeric_group` decides authoritatively whether it is a
    /// numeric in-text citation (digits/commas/ranges only) or noise
    /// (`[Fig. 2]`, `[95% CI]`, `[data not shown]`).
    pub bracket_numeric: Regex,
}

pub fn regexes() -> &'static Regexes {
    static RE: OnceLock<Regexes> = OnceLock::new();
    RE.get_or_init(|| {
        let test = |p: &str, name: &'static str| (Regex::new(p).unwrap(), name);
        Regexes {
            // p, optional "-value", operator, number (incl. leading-dot and sci-notation)
            p_value: Regex::new(
                r"(?i)\bp\s*(?:-?\s*values?)?\s*(<=|>=|<|>|=|≤|≥)\s*(\d*\.\d+(?:[eE][-+]?\d+)?|\d+(?:\.\d+)?)",
            )
            .unwrap(),
            confidence_interval: Regex::new(
                r"(?i)(\d{2,3})\s*%\s*(?:ci|confidence\s+interval)\s*[:=]?\s*[\[\(]?\s*(-?\d+(?:\.\d+)?)\s*(?:to|,|–|—|-)\s*(-?\d+(?:\.\d+)?)",
            )
            .unwrap(),
            // **The trailing `(\.\d+)?` exists to be REJECTED, not used.** Without
            // it `n = 0.52` matched `n = 0` and captured `0` — a sample size of
            // zero, which is not a small study but an impossible one. Measured
            // over 20 manuscripts: SIX such reads, every one a decimal severed
            // from its integer, and in the pharmaceutics manuscript `n` is the
            // Korsmeyer–Peppas RELEASE EXPONENT — *"the release exponent
            // n = 0.52 for curcumin indicated Non-Fickian anomalous…"* — a
            // variable named `n` that counts nothing. One of the six reached a
            // user as a CRITICAL finding (§11 D178).
            sample_size: Regex::new(r"(?i)\bn\s*=\s*(\d{1,3}(?:,\d{3})+|\d+)(\.\d+)?").unwrap(),
            sample_phrase: Regex::new(
                r"(?i)\bsample\s+size\s+(?:of|was|=|:)?\s*(\d{1,3}(?:,\d{3})+|\d+)(\.\d+)?",
            )
            .unwrap(),
            tests: vec![
                test(r"(?i)\b(?:independent|paired|two[- ]sample|one[- ]sample|student'?s)?\s*t[- ]tests?\b", "t-test"),
                test(r"(?i)\b(?:one|two)[- ]way\s+anova\b|\banova\b|\banalysis of variance\b", "ANOVA"),
                test(r"(?i)\bchi[- ]squared?\b|\bχ2\b|\bχ²\b", "chi-square"),
                test(r"(?i)\bmann[- ]whitney\b", "Mann-Whitney U test"),
                test(r"(?i)\bwilcoxon\b", "Wilcoxon signed-rank test"),
                test(r"(?i)\bkruskal[- ]wallis\b", "Kruskal-Wallis test"),
                test(r"(?i)\bfisher'?s?\s+exact\b", "Fisher's exact test"),
                test(r"(?i)\bpearson(?:'?s)?\s+correlation\b", "Pearson correlation"),
                test(r"(?i)\bspearman(?:'?s)?\b", "Spearman correlation"),
                test(r"(?i)\b(?:linear|logistic|multiple)\s+regression\b", "regression"),
            ],
            // Only F is captured here. t(df) and chi-square(df) share the shape
            // exactly and are deliberately out of this milestone's scope; adding
            // them is another entry in this list plus a name.
            f_statistic: Regex::new(
                r"(?i)\bF\s*\(\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*\)\s*=\s*(-?\d+(?:\.\d+)?)",
            )
            .unwrap(),
            // `=?` is optional because the alternation already embeds `=` for the
            // shorthand forms (`\bOR\s*=`), and omits it for the named ones.
            effect_size_value: Regex::new(&format!(
                r"(?i)({EFFECT_SIZE_ALTERNATION})\s*(?:=|:|of)?\s*(-?\d+(?:\.\d+)?)"
            ))
            .unwrap(),
            // **`[.)]` STANDS IN FOR THE SPACE, and must stay MANDATORY on that
            // branch. §11 D189.** The old pattern required `\s+`, so "VI.CONCLUSION"
            // was never stripped and the heading was never recognised.
            //
            // TRAP, measured: making the separator optional instead —
            // `[.)]?\s*` — lets `[IVXLCM]+` match "CLIM" in "CLIMATE" and strip
            // the front off ordinary words. The alternation below cannot: after
            // the numerals it demands either a `.`/`)` or whitespace.
            heading_number: Regex::new(r"^\s*(?:\d+(?:\.\d+)*|[IVXLCM]+)(?:[.)]\s*|\s+)")
                .unwrap(),
            // A RUN-IN heading: a phrase, a punctuation separator, then body
            // text on the SAME line ("Abstract- Emotion detection in…").
            // Letters/spaces/& only in the prefix — no digits — so numbered
            // headings keep taking the path above. §11 D189.
            runin_heading: Regex::new(r"^\s*([A-Za-z][A-Za-z &]{0,28}?)\s*[-\u{2013}\u{2014}:.]\s*(\S.*)$")
                .unwrap(),
            table_caption: Regex::new(r"(?i)^table\s+(\d+)[.:]?\s*(.*)$").unwrap(),
            doi: Regex::new(r"(?i)\b(10\.\d{4,9}/[^\s]+)").unwrap(),
            year_paren: Regex::new(r"\((19|20)\d{2}[a-z]?\)").unwrap(),
            year_bare: Regex::new(r"\b((?:19|20)\d{2})\b").unwrap(),
            // narrative: author token(s) immediately followed by (year)
            narrative_cite: Regex::new(
                r"([A-Z][\p{L}'’.\-]+(?:\s+(?:et\s+al\.?|(?:and|&)\s+[A-Z][\p{L}'’.\-]+|,\s+[A-Z][\p{L}'’.\-]+)+)?)\s+\(((?:19|20)\d{2}[a-z]?)\)",
            )
            .unwrap(),
            paren_group: Regex::new(r"\(([^()]*(?:19|20)\d{2}[^()]*)\)").unwrap(),
            bracket_numeric: Regex::new(r"\[([^\[\]]{1,80})\]").unwrap(),
        }
    })
}

fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('.') {
        format!("0.{rest}").parse().ok()
    } else if let Some(rest) = s.strip_prefix("-.") {
        format!("-0.{rest}").parse().ok()
    } else {
        s.parse().ok()
    }
}

fn normalize_operator(op: &str) -> String {
    match op {
        "≤" => "<=".into(),
        "≥" => ">=".into(),
        other => other.into(),
    }
}

/// Extract every statistical claim in one paragraph, tagged with `loc`.
pub fn extract(paragraph: &str, loc: &Location) -> Vec<StatClaim> {
    let re = regexes();
    let mut out = Vec::new();
    let mut push = |stat: Stat| out.push(StatClaim { stat, location: loc.clone() });

    // THE ONE PRODUCER of the criterion/result decision. Every consumer reads
    // the variant; none re-derives it.
    for c in re.p_value.captures_iter(paragraph) {
        if let Some(value) = parse_number(&c[2]) {
            let whole = c.get(0).expect("group 0 always matches");
            let operator = normalize_operator(&c[1]);
            let raw = whole.as_str().trim().to_string();
            if is_significance_criterion(paragraph, whole.start()) {
                push(Stat::SignificanceThreshold { operator, value, raw });
            } else {
                push(Stat::PValue { operator, value, raw });
            }
        }
    }

    for c in re.confidence_interval.captures_iter(paragraph) {
        match (parse_number(&c[2]), parse_number(&c[3])) {
            (Some(low), Some(high)) => push(Stat::ConfidenceInterval {
                level: parse_number(&c[1]),
                low,
                high,
                raw: c[0].trim().to_string(),
            }),
            _ => {}
        }
    }

    let parse_n = |s: &str| -> Option<i64> { s.replace(',', "").parse().ok() };
    // A count has no fractional part. `n = 0.52` is a release exponent, a
    // Cook's-distance cutoff or a proportion — never a number of participants —
    // so a captured decimal tail REJECTS the match rather than truncating it.
    for c in re.sample_size.captures_iter(paragraph).chain(re.sample_phrase.captures_iter(paragraph))
    {
        if c.get(2).is_some() {
            continue;
        }
        if let Some(n) = parse_n(&c[1]) {
            push(Stat::SampleSize { n, raw: c[0].trim().to_string() });
        }
    }

    for (re_test, name) in &re.tests {
        if let Some(m) = re_test.find(paragraph) {
            push(Stat::Test { name: (*name).to_string(), raw: m.as_str().trim().to_string() });
        }
    }

    for c in re.f_statistic.captures_iter(paragraph) {
        if let (Some(df1), Some(df2), Some(value)) =
            (parse_number(&c[1]), parse_number(&c[2]), parse_number(&c[3]))
        {
            push(Stat::TestStatistic {
                name: "F".to_string(),
                value,
                df: vec![df1, df2],
                raw: c[0].trim().to_string(),
            });
        }
    }

    for c in re.effect_size_value.captures_iter(paragraph) {
        if let Some(value) = parse_number(&c[2]) {
            push(Stat::EffectSize {
                // The matched measure as WRITTEN, trailing `=` and whitespace
                // trimmed — the alternation embeds `=` for shorthand forms.
                name: c[1].trim().trim_end_matches('=').trim().to_string(),
                value,
                raw: c[0].trim().to_string(),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{Location, SectionKind};

    fn stats_of(text: &str) -> Vec<Stat> {
        let l = Location { section: SectionKind::Results, paragraph: 0, section_index: None };
        extract(text, &l).into_iter().map(|c| c.stat).collect()
    }

    /// **A count has no fractional part, and `n` is not always a count.**
    ///
    /// Every string here is from a real manuscript. The regex had no trailing
    /// boundary, so `n = 0.52` matched `n = 0` and captured `0` — a sample size
    /// of zero, which is not a small study but an impossible one. Six such reads
    /// across 20 manuscripts, one of which reached a user as a CRITICAL
    /// "small sample with causal claim" finding (§11 D178).
    ///
    /// In the pharmaceutics cases `n` is the Korsmeyer-Peppas RELEASE EXPONENT:
    /// a variable named `n` that counts nothing at all.
    #[test]
    fn a_severed_decimal_is_not_a_sample_size() {
        for text in [
            "The release exponent n = 0.52 for curcumin indicated Non-Fickian anomalous transport.",
            "At 34 C the Power Law model resulted in n = 0.31 and k = 6842 Pa.",
            "Observations were evaluated for Cook's distance values greater than 4/n = 0.018.",
        ] {
            let sizes: Vec<i64> = stats_of(text)
                .iter()
                .filter_map(|s| match s {
                    Stat::SampleSize { n, .. } => Some(*n),
                    _ => None,
                })
                .collect();
            assert!(sizes.is_empty(), "{text:?} yielded sample size(s) {sizes:?}");
        }

        // **The negative control.** Without it this test is satisfied by the
        // extractor reading no sample size at all, which would be a worse bug
        // wearing a green tick.
        let real: Vec<i64> = stats_of("We analysed the final sample (n = 150) of enterprises.")
            .iter()
            .filter_map(|s| match s {
                Stat::SampleSize { n, .. } => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(real, vec![150], "a genuine integer sample size must still be read");
    }

    /// Item 1 — the STATISTIC, not the test name. Before this, a paragraph
    /// reporting `F(2, 57) = 4.82` produced only `Test { name: "ANOVA" }`, so
    /// the report could name the test and never what it produced.
    #[test]
    fn an_f_statistic_is_extracted_with_its_degrees_of_freedom() {
        let stats = stats_of("A one-way ANOVA was significant, F(2, 57) = 4.82, p = 0.011.");
        let f = stats
            .iter()
            .find_map(|s| match s {
                Stat::TestStatistic { name, value, df, .. } => Some((name.clone(), *value, df.clone())),
                _ => None,
            })
            .expect("F statistic must be extracted");
        assert_eq!(f.0, "F");
        assert_eq!(f.1, 4.82);
        assert_eq!(f.2, vec![2.0, 57.0]);
        // The test NAME is still extracted — the two are different facts.
        assert!(stats.iter().any(|s| matches!(s, Stat::Test { name, .. } if name == "ANOVA")));
    }

    /// Item 2 — a PRESENT effect size becomes a value. `validate.rs` has
    /// detected presence since MissingEffectSize was written, but only as
    /// `is_match`, so nothing carried the number.
    #[test]
    fn a_reported_effect_size_is_extracted_with_its_value() {
        for (text, name, value) in [
            ("The effect was moderate, Cohen's d = 0.42.", "Cohen's d", 0.42),
            ("Group differences were large (eta-squared = 0.31).", "eta-squared", 0.31),
            ("The odds ratio was substantial, OR = 2.75.", "OR", 2.75),
        ] {
            let stats = stats_of(text);
            let e = stats
                .iter()
                .find_map(|s| match s {
                    Stat::EffectSize { name, value, .. } => Some((name.clone(), *value)),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no effect size extracted from {text:?}"));
            assert_eq!(e.0.to_lowercase(), name.to_lowercase(), "measure name");
            assert_eq!(e.1, value, "measure value");
        }
    }

    /// A paragraph with no effect size must produce none — otherwise the
    /// Reported/Missing split would be meaningless in the other direction.
    #[test]
    fn a_paragraph_without_an_effect_size_yields_none() {
        let stats = stats_of("The difference was significant, p = 0.03.");
        assert!(!stats.iter().any(|s| matches!(s, Stat::EffectSize { .. })));
    }

    fn loc() -> Location {
        Location { section: SectionKind::Results, paragraph: 0, section_index: None }
    }

    fn pvalues(text: &str) -> Vec<(String, f64)> {
        extract(text, &loc())
            .into_iter()
            .filter_map(|s| match s.stat {
                Stat::PValue { operator, value, .. } => Some((operator, value)),
                _ => None,
            })
            .collect()
    }

    /// The alternation, pinned to the literal that ships.
    ///
    /// # A RELEASE ARTIFACT — appended to, never rewritten
    ///
    /// Same convention as `evidence.rs`'s compatibility fixtures: a string that
    /// has existed in production is kept, and a superseded one is retained as
    /// the evidence of a deliberate break rather than deleted. Regenerating from
    /// the live constant would assert the constant equals itself.
    ///
    /// # It moved here, and its rationale changed
    ///
    /// It lived in `validate.rs` and read *"sharing the alternation must not
    /// have changed WHICH PARAGRAPHS the MissingEffectSize rule fires on"*.
    /// **That rationale is now void: rule 3 consults the TYPED extraction and
    /// never reads this pattern** (§49 shape 3). The alternation has exactly one
    /// consumer — extraction — so the pin belongs beside it, and what it now
    /// guards is which STRINGS are extracted as an effect size.
    #[test]
    fn effect_size_pattern_is_unchanged() {
        /// SUPERSEDED at §49.4 — bare `\bf2\b` matched formulation batch labels
        /// (F1…F4) and would equally match the FDA/EMA dissolution similarity
        /// factor, written `f2 = 65.2` but ranging 0–100 against Cohen's f²'s
        /// 0.02–0.35. Kept as the evidence of the change, not as a live pin.
        const RETIRED_AT_49: &str = r"(?i)(cohen'?s\s*d|hedges'?\s*g|eta[\s-]*squared|η2|η²|partial\s+eta|omega[\s-]*squared|cramer'?s\s*v|odds\s+ratio|hazard\s+ratio|risk\s+ratio|effect\s+size|\bOR\s*=|\bHR\s*=|\bRR\s*=|\bd\s*=|\bg\s*=|\br\s*=|\bR2\b|R²|\bf2\b)";
        const SHIPPED: &str = r"(?i)(cohen'?s\s*d|hedges'?\s*g|eta[\s-]*squared|η2|η²|partial\s+eta|omega[\s-]*squared|cramer'?s\s*v|odds\s+ratio|hazard\s+ratio|risk\s+ratio|effect\s+size|cohen'?s\s*f2|cohen'?s\s*f²|\bOR\s*=|\bHR\s*=|\bRR\s*=|\bd\s*=|\bg\s*=|\br\s*=|\bR2\b|R²)";

        let composed = format!("(?i)({})", EFFECT_SIZE_ALTERNATION);
        assert_eq!(composed, SHIPPED, "the alternation changed; regenerate DELIBERATELY and append the old literal");
        assert_ne!(
            composed, RETIRED_AT_49,
            "the retired literal is live again — if that is intended, move it into SHIPPED and \
             retire the current one; do not delete it"
        );
    }

    #[test]
    fn pvalue_formats() {
        assert_eq!(pvalues("significant (p < 0.001)"), vec![("<".into(), 0.001)]);
        assert_eq!(pvalues("P = .04 overall"), vec![("=".into(), 0.04)]);
        assert_eq!(pvalues("p-value of p ≤ 0.05"), vec![("<=".into(), 0.05)]);
        assert_eq!(pvalues("p = 1.2e-5 tiny"), vec![("=".into(), 1.2e-5)]);
    }

    #[test]
    fn confidence_interval_variants() {
        let cis: Vec<_> = extract("95% CI: 1.2 to 3.4 and 99% CI [0.5, 0.9]", &loc())
            .into_iter()
            .filter_map(|s| match s.stat {
                Stat::ConfidenceInterval { level, low, high, .. } => Some((level, low, high)),
                _ => None,
            })
            .collect();
        assert_eq!(cis[0], (Some(95.0), 1.2, 3.4));
        assert_eq!(cis[1], (Some(99.0), 0.5, 0.9));
    }

    #[test]
    fn sample_sizes() {
        let ns: Vec<_> = extract("cohort (n = 1,240) and sample size of 60", &loc())
            .into_iter()
            .filter_map(|s| match s.stat {
                Stat::SampleSize { n, .. } => Some(n),
                _ => None,
            })
            .collect();
        assert!(ns.contains(&1240));
        assert!(ns.contains(&60));
    }

    #[test]
    fn test_name_canonicalization() {
        let names = |t: &str| -> Vec<String> {
            extract(t, &loc())
                .into_iter()
                .filter_map(|s| match s.stat {
                    Stat::Test { name, .. } => Some(name),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(names("a paired t-test"), vec!["t-test"]);
        assert_eq!(names("one-way ANOVA"), vec!["ANOVA"]);
        assert_eq!(names("a chi-squared test"), vec!["chi-square"]);
        assert_eq!(names("Mann-Whitney U"), vec!["Mann-Whitney U test"]);
        assert!(names("Fisher's exact test").contains(&"Fisher's exact test".to_string()));
    }

    // =======================================================================
    // §42 — the criterion/result classification
    // =======================================================================

    fn kinds(text: &str) -> Vec<String> {
        let l = Location { section: SectionKind::Methods, paragraph: 0, section_index: None };
        extract(text, &l)
            .into_iter()
            .filter_map(|c| match c.stat {
                Stat::PValue { raw, .. } => Some(format!("result:{raw}")),
                Stat::SignificanceThreshold { raw, .. } => Some(format!("criterion:{raw}")),
                _ => None,
            })
            .collect()
    }

    /// **Every threshold case measured across the corpus.** These are verbatim
    /// constructions from three real manuscripts (ARCHITECTURE_TRACE §41), not
    /// invented examples — the phrasing is the evidence.
    #[test]
    fn declared_criteria_are_classified_as_criteria() {
        for text in [
            "Treatment means were compared by the critical difference at p \u{2264} 0.05.",
            "CD (C), critical difference for the concentration; NS, not significant at p \u{2264} 0.05.",
            "Analysis was by one-way ANOVA with post-hoc Tukey's test (significance level p < 0.05).",
            "Statistical significance was determined at p < 0.05 unless otherwise stated.",
        ] {
            assert_eq!(
                kinds(text).len(),
                1,
                "fixture must yield exactly one p-value: {text:?}"
            );
            assert!(
                kinds(text)[0].starts_with("criterion:"),
                "must classify as a criterion: {text:?} -> {:?}",
                kinds(text)
            );
        }
    }

    /// **Every result case must survive.** The corpus measured 0 over-fires in
    /// 24 result paragraphs; these are the constructions that produced it,
    /// including the two that falsify the tempting shortcuts.
    #[test]
    fn reported_results_are_not_classified_as_criteria() {
        for text in [
            // the VALUE falsifier — a real result at exactly 0.05
            "Permeation differed at all time points (p < 0.0001 at 1 h; p < 0.05 at 12 h).",
            // the MARKER falsifier — "significant at" in a RESULT sentence
            "All four differences are significant at p < 0.001.",
            "One-way ANOVA (F = 1842.6, df = 8, p < 0.0001) confirmed the difference.",
            "The pre-intervention trend was flat (\u{3b2}1 = -0.092, p = 0.066).",
            "Indistinguishable from the negative control (97.4%; p > 0.05).",
        ] {
            let k = kinds(text);
            assert!(!k.is_empty(), "fixture must yield a p-value: {text:?}");
            assert!(
                k.iter().all(|s| s.starts_with("result:")),
                "must NOT classify as a criterion: {text:?} -> {k:?}"
            );
        }
    }

    /// **The classification input is the SENTENCE, not the paragraph.** A
    /// Methods paragraph that declares a threshold and then reports a result
    /// must classify them differently — the case the paragraph unit gets wrong.
    #[test]
    fn a_declaration_and_a_result_in_one_paragraph_classify_differently() {
        let text = "Statistical significance was determined at p < 0.05. \
                    The intervention improved recall (p = 0.002).";
        let k = kinds(text);
        assert_eq!(k.len(), 2, "{k:?}");
        assert!(k.iter().any(|s| s == "criterion:p < 0.05"), "{k:?}");
        assert!(k.iter().any(|s| s == "result:p = 0.002"), "{k:?}");
    }

    /// A criterion carries its operator and value like any other statistic —
    /// the classification changes what it MEANS, not what was read.
    #[test]
    fn a_criterion_preserves_the_value_as_written() {
        let l = Location { section: SectionKind::Methods, paragraph: 0, section_index: None };
        let out = extract("Significance was determined at p < 0.05.", &l);
        match &out[0].stat {
            Stat::SignificanceThreshold { operator, value, raw } => {
                assert_eq!(operator, "<");
                assert!((value - 0.05).abs() < 1e-12);
                assert_eq!(raw, "p < 0.05");
            }
            other => panic!("expected a criterion, got {other:?}"),
        }
    }

    /// **OWNERSHIP, GREP-PROVEN.** Exactly one module may decide whether a
    /// p-value is a criterion. A consumer that re-derives the decision from
    /// surrounding text recreates the duplication §41.3 exists to remove, and a
    /// doc comment saying so is §21 level 1.
    ///
    /// Checks the workspace source for the marker vocabulary outside this file.
    /// A copy-pasted phrase list and a second `THRESHOLD_MARKERS` both fail.
    #[test]
    fn threshold_markers_are_owned_by_extraction() {
        use std::path::Path;
        // gaply-core/ -> src-tauri/
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
        let owner = root.join("gaply-core/src/extract/stats.rs");

        fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().map(|n| n == "target").unwrap_or(false) {
                        continue;
                    }
                    walk(&p, out);
                } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(&root, &mut files);
        assert!(files.len() > 20, "the walk must find the workspace sources: {}", files.len());
        assert!(files.contains(&owner), "the owning file must be in the walk");

        // TWO CHECKS, at two scopes.
        //
        // The IDENTIFIER may not be named anywhere else at all — reading the
        // marker list from another module IS the duplication, test or not.
        //
        // The PHRASES are checked in PRODUCTION code only, i.e. everything
        // before a file's `#[cfg(test)]`. A test fixture containing
        // "significance was determined at" is a caller feeding text THROUGH the
        // one producer, which is the intended use; a `#[cfg(test)]` block ships
        // no behaviour. This test caught its own author twice — a doc example in
        // `sentence.rs` and a fixture in `validate.rs` — which is the evidence
        // that the scope needed stating rather than assuming.
        const IDENT: &str = "THRESHOLD_MARKERS";
        const PHRASES: &[&str] = &["critical difference at", "significance was determined at"];

        for f in &files {
            if f == &owner {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(f) else { continue };
            assert!(
                !src.contains(IDENT),
                "{} names {IDENT}. The criterion/result decision has ONE producer \
                 (extract::stats::extract); reading its vocabulary elsewhere recreates the \
                 duplication ARCHITECTURE_TRACE §41.3 exists to remove. Read the \
                 `Stat::SignificanceThreshold` variant instead.",
                f.display()
            );
            let production = src.split("#[cfg(test)]").next().unwrap_or("");
            for phrase in PHRASES {
                assert!(
                    !production.contains(phrase),
                    "{} contains {phrase:?} in PRODUCTION code. The criterion/result decision \
                     has ONE producer; a consumer re-deriving it from text recreates the \
                     duplication ARCHITECTURE_TRACE §41.3 exists to remove. Read the \
                     `Stat::SignificanceThreshold` variant instead.",
                    f.display()
                );
            }
        }
    }
}