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
    r"effect\s+size|\bOR\s*=|\bHR\s*=|\bRR\s*=|\bd\s*=|\bg\s*=|\br\s*=|\bR2\b|R²|\bf2\b"
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
            sample_size: Regex::new(r"(?i)\bn\s*=\s*(\d{1,3}(?:,\d{3})+|\d+)").unwrap(),
            sample_phrase: Regex::new(
                r"(?i)\bsample\s+size\s+(?:of|was|=|:)?\s*(\d{1,3}(?:,\d{3})+|\d+)",
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
            heading_number: Regex::new(r"^\s*(?:\d+(?:\.\d+)*|[IVXLCM]+)[.)]?\s+").unwrap(),
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

    for c in re.p_value.captures_iter(paragraph) {
        if let Some(value) = parse_number(&c[2]) {
            push(Stat::PValue {
                operator: normalize_operator(&c[1]),
                value,
                raw: c[0].trim().to_string(),
            });
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
    for c in re.sample_size.captures_iter(paragraph) {
        if let Some(n) = parse_n(&c[1]) {
            push(Stat::SampleSize { n, raw: c[0].trim().to_string() });
        }
    }
    for c in re.sample_phrase.captures_iter(paragraph) {
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
        let l = Location { section: SectionKind::Results, paragraph: 0 };
        extract(text, &l).into_iter().map(|c| c.stat).collect()
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
        Location { section: SectionKind::Results, paragraph: 0 }
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
}
