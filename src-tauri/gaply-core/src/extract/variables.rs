//! Deterministic scientific-variable extraction.
//!
//! Stage 1 (implemented): rule-based variable detection using section awareness,
//! statistical language, study-design cues, and unit/measurement keywords.
//!
//! Variables are NOT raw noun phrases. A variable is a measured or manipulated
//! scientific entity that plays a role in the study design. One sentence may
//! introduce several variables; one variable may be mentioned many times.
//!
//! Stage 2 (planned): optional local-SLM refinement of candidate variables.
//! Stage 3 (planned): optional cloud refinement with bounded metadata only.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::extract::{ExtractionResult, Location, SectionKind, sentence};
use crate::scientific_model::{
    ClaimId, SourceSpan, StatRef, Variable, VariableId, VariableRole, VariableSource,
};

/// Maximum length of a variable name stored in the model. Longer phrases are
/// truncated; the full mention remains reachable through `source_span`.
const MAX_NAME_LEN: usize = 120;

/// A detected role cue plus its metadata.
struct RoleCue {
    role: VariableRole,
    confidence: f64,
    /// Regex containing capture groups. Only the indices listed in `name_groups`
    /// are treated as variable names; other groups may hold role phrases or
    /// alternatives that did not match.
    re: Regex,
    /// Which capture groups contain variable names.
    name_groups: Vec<usize>,
}

fn role_cues() -> &'static Vec<RoleCue> {
    static CUES: OnceLock<Vec<RoleCue>> = OnceLock::new();
    CUES.get_or_init(|| {
        vec![
            // Explicit role assignment: "X was the independent variable"
            RoleCue {
                role: VariableRole::Independent,
                confidence: 0.95,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were|is|are|served as|acted as|used as)\s+(?:the\s+)?(independent variable|predictor variable|predictor)").unwrap(),
                name_groups: vec![1],
            },
            // "The independent variable was X"
            RoleCue {
                role: VariableRole::Independent,
                confidence: 0.95,
                re: Regex::new(r"(?i)(?:the\s+)?(independent variable|predictor variable|predictor)\s+(?:was|were|is|are)\s+([A-Za-z][A-Za-z0-9\s\-]{2,40})").unwrap(),
                name_groups: vec![2],
            },
            // "Y was the dependent variable / outcome variable"
            RoleCue {
                role: VariableRole::Dependent,
                confidence: 0.95,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were|is|are|served as|acted as)\s+(?:the\s+)?(dependent variable|outcome variable|outcome|response variable)").unwrap(),
                name_groups: vec![1],
            },
            RoleCue {
                role: VariableRole::Dependent,
                confidence: 0.95,
                re: Regex::new(r"(?i)(?:the\s+)?(dependent variable|outcome variable|outcome|response variable)\s+(?:was|were|is|are)\s+([A-Za-z][A-Za-z0-9\s\-]{2,40})").unwrap(),
                name_groups: vec![2],
            },
            // Outcome (often without "variable")
            RoleCue {
                role: VariableRole::Outcome,
                confidence: 0.96,
                re: Regex::new(r"(?i)(?:primary|main|key\s+)?outcome\s+(?:variable\s+)?(?:was|were|is|are)\s+([A-Za-z][A-Za-z0-9\s\-]{2,40})").unwrap(),
                name_groups: vec![1],
            },
            RoleCue {
                role: VariableRole::Outcome,
                confidence: 0.9,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were|is|are)\s+(?:the\s+)?outcome").unwrap(),
                name_groups: vec![1],
            },
            // Exposure
            RoleCue {
                role: VariableRole::Exposure,
                confidence: 0.9,
                re: Regex::new(r"(?i)(?:the\s+)?exposure\s+(?:was|were|is|are)\s+([A-Za-z][A-Za-z0-9\s\-]{2,40})|exposure\s+to\s+([A-Za-z][A-Za-z0-9\s\-]{2,40})").unwrap(),
                name_groups: vec![1, 2],
            },
            // Control / covariate
            RoleCue {
                role: VariableRole::Covariate,
                confidence: 0.9,
                re: Regex::new(r"(?i)controlled for\s+([A-Za-z][A-Za-z0-9\s\-]{2,40})(?:\s+and\s+([A-Za-z][A-Za-z0-9\s\-]{2,40}))?").unwrap(),
                name_groups: vec![1, 2],
            },
            RoleCue {
                role: VariableRole::Covariate,
                confidence: 0.9,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were)\s+(?:a\s+)?(?:continuous\s+)?covariate").unwrap(),
                name_groups: vec![1],
            },
            RoleCue {
                role: VariableRole::Covariate,
                confidence: 0.85,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were)\s+included\s+as\s+(?:a\s+)?(?:continuous\s+)?covariate").unwrap(),
                name_groups: vec![1],
            },
            RoleCue {
                role: VariableRole::Control,
                confidence: 0.85,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were)\s+(?:a\s+)?control variable").unwrap(),
                name_groups: vec![1],
            },
            // Mediator / moderator
            RoleCue {
                role: VariableRole::Mediator,
                confidence: 0.85,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were|acted as)\s+(?:a\s+)?mediator").unwrap(),
                name_groups: vec![1],
            },
            RoleCue {
                role: VariableRole::Moderator,
                confidence: 0.85,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+(?:was|were|acted as)\s+(?:a\s+)?moderator").unwrap(),
                name_groups: vec![1],
            },
            // Regression language: "X predicted Y", "the effect of X on Y"
            RoleCue {
                role: VariableRole::Predictor,
                confidence: 0.8,
                re: Regex::new(r"(?i)([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+predicted\s+[A-Za-z]").unwrap(),
                name_groups: vec![1],
            },
            RoleCue {
                role: VariableRole::Predictor,
                confidence: 0.8,
                re: Regex::new(r"(?i)effect of\s+([A-Za-z][A-Za-z0-9\s\-]{2,40})\s+on\s+[A-Za-z]").unwrap(),
                name_groups: vec![1],
            },
        ]
    })
}

/// Cues for measurement level expressed in the same sentence as a variable.
fn measurement_cues() -> &'static Vec<(MeasurementLevel, Regex)> {
    static CUES: OnceLock<Vec<(MeasurementLevel, Regex)>> = OnceLock::new();
    CUES.get_or_init(|| {
        vec![
            (MeasurementLevel::Continuous, Regex::new(r"(?i)\bcontinuous\b").unwrap()),
            (MeasurementLevel::Categorical, Regex::new(r"(?i)\bcategorical\b").unwrap()),
            (MeasurementLevel::Dichotomous, Regex::new(r"(?i)\b(dichotomous|binary)\b").unwrap()),
            (MeasurementLevel::Ordinal, Regex::new(r"(?i)\bordinal\b").unwrap()),
            (MeasurementLevel::Nominal, Regex::new(r"(?i)\bnominal\b").unwrap()),
            (MeasurementLevel::Count, Regex::new(r"(?i)\bcount\b").unwrap()),
            (MeasurementLevel::Ratio, Regex::new(r"(?i)\bratio\b").unwrap()),
            (MeasurementLevel::Interval, Regex::new(r"(?i)\binterval\b").unwrap()),
        ]
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MeasurementLevel {
    Nominal,
    Ordinal,
    Interval,
    Ratio,
    Dichotomous,
    Count,
    Continuous,
    Categorical,
}

impl MeasurementLevel {
    fn from_keyword(text: &str) -> Option<Self> {
        for (level, re) in measurement_cues() {
            if re.is_match(text) {
                return Some(level.clone());
            }
        }
        None
    }
}

/// A variable mention before merging.
#[derive(Debug, Clone)]
struct RawVariable {
    name: String,
    role: VariableRole,
    confidence: f64,
    loc: Location,
    sentence_index: usize,
    units: Option<String>,
    measurement: Option<MeasurementLevel>,
    operationalization: Option<String>,
}

/// Extract variables from a fully populated `ExtractionResult` and the claims
/// already extracted from it.
///
/// This is Stage 1: deterministic, local, no AI. It never fails the pipeline;
/// on any internal error it returns whatever variables it managed to collect.
pub fn extract_variables(
    result: &ExtractionResult,
    claims: &[crate::scientific_model::ScientificClaim],
) -> Vec<Variable> {
    let mut raw: Vec<RawVariable> = Vec::new();

    for (sec_idx, section) in result.sections.iter().enumerate() {
        if section.kind == SectionKind::References {
            continue;
        }
        for (p_idx, paragraph) in section.paragraphs.iter().enumerate() {
            let loc = Location::in_section(section.kind, sec_idx, p_idx);
            let sentences = sentence::sentences_in(paragraph);
            for (s_idx, sentence_text) in sentences.iter().enumerate() {
                for rv in find_variables_in_sentence(sentence_text, loc.clone(), s_idx) {
                    raw.push(rv);
                }
            }
        }
    }

    // Merge mentions that share the same canonical name, keeping the highest-
    // confidence role. Preserve all mentions, sentence indices, and aliases.
    let mut by_key: HashMap<String, VariableAccumulator> = HashMap::new();
    for rv in raw {
        let key = canonical_key(&rv.name);
        let acc = by_key.entry(key).or_insert_with(|| VariableAccumulator {
            canonical_name: rv.name.clone(),
            role: rv.role.clone(),
            confidence: rv.confidence,
            aliases: Vec::new(),
            mentions: Vec::new(),
            sentence_indices: Vec::new(),
            units: rv.units.clone(),
            measurement: rv.measurement.clone(),
            operationalization: rv.operationalization.clone(),
        });
        acc.add_mention(rv);
    }

    // Build final variables with deterministic IDs and associations.
    let mut variables: Vec<Variable> = Vec::new();
    let stat_index = index_stats_by_location(result);

    for (idx, (key, mut acc)) in by_key.into_iter().enumerate() {
        let id = VariableId(format!("var-{}", idx + 1));
        acc.aliases.retain(|a| *a != key && !a.is_empty());
        acc.aliases.sort();
        acc.aliases.dedup();

        acc.mentions.sort_by(|a, b| match (a, b) {
            (SourceSpan::Point(al), SourceSpan::Point(bl)) => al.cmp(bl),
            _ => std::cmp::Ordering::Equal,
        });
        acc.mentions.dedup();

        acc.sentence_indices.sort_unstable();
        acc.sentence_indices.dedup();

        let stats = associated_stats(&stat_index, &acc.canonical_name, &acc.aliases, result);

        let mut associated_claims: Vec<ClaimId> = claims
            .iter()
            .filter(|c| name_appears_in_claim(&acc.canonical_name, &acc.aliases, c))
            .map(|c| c.id.clone())
            .collect();
        associated_claims.sort_by(|a, b| a.0.cmp(&b.0));
        associated_claims.dedup();

        variables.push(Variable {
            id,
            canonical_name: acc.canonical_name,
            aliases: acc.aliases,
            role: acc.role,
            measurement: match acc.measurement {
                Some(m) => match m {
                    MeasurementLevel::Nominal => crate::scientific_model::MeasurementLevel::Nominal,
                    MeasurementLevel::Ordinal => crate::scientific_model::MeasurementLevel::Ordinal,
                    MeasurementLevel::Interval => crate::scientific_model::MeasurementLevel::Interval,
                    MeasurementLevel::Ratio => crate::scientific_model::MeasurementLevel::Ratio,
                    MeasurementLevel::Dichotomous => crate::scientific_model::MeasurementLevel::Dichotomous,
                    MeasurementLevel::Count => crate::scientific_model::MeasurementLevel::Count,
                    MeasurementLevel::Continuous => crate::scientific_model::MeasurementLevel::Continuous,
                    MeasurementLevel::Categorical => crate::scientific_model::MeasurementLevel::Categorical,
                },
                None => crate::scientific_model::MeasurementLevel::Other("unknown".into()),
            },
            units: acc.units,
            operationalization: acc.operationalization,
            mentions: acc.mentions,
            sentence_indices: acc.sentence_indices,
            statistics: stats,
            associated_claims,
            associated_methods: Vec::new(),
            associated_datasets: Vec::new(),
            confidence: acc.confidence,
            source: VariableSource::Deterministic,
        });
    }

    variables.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    variables
}

#[derive(Debug, Clone)]
struct VariableAccumulator {
    canonical_name: String,
    role: VariableRole,
    confidence: f64,
    aliases: Vec<String>,
    mentions: Vec<SourceSpan>,
    sentence_indices: Vec<usize>,
    units: Option<String>,
    measurement: Option<MeasurementLevel>,
    operationalization: Option<String>,
}

impl VariableAccumulator {
    fn add_mention(&mut self, rv: RawVariable) {
        let alias = canonical_key(&rv.name);
        if alias != canonical_key(&self.canonical_name) && !self.aliases.contains(&alias) {
            self.aliases.push(alias);
        }
        self.mentions.push(SourceSpan::Point(rv.loc));
        self.sentence_indices.push(rv.sentence_index);
        if rv.confidence > self.confidence {
            self.role = rv.role;
            self.confidence = rv.confidence;
        }
        if self.units.is_none() {
            self.units = rv.units;
        }
        if self.measurement.is_none() {
            self.measurement = rv.measurement;
        }
        if self.operationalization.is_none() {
            self.operationalization = rv.operationalization;
        }
    }
}

/// Remove short parenthetical units/clauses so role regexes can match across
/// them (e.g. "age (years) was" becomes "age  was"). Offsets are not used for
/// name extraction, only the captured text, so this is safe.
fn strip_units_for_matching(sentence: &str) -> String {
    let re = var_pat3_re();
    re.replace_all(sentence, " ").to_string()
}

fn find_variables_in_sentence(
    sentence: &str,
    loc: Location,
    sentence_index: usize,
) -> Vec<RawVariable> {
    let mut out = Vec::new();
    let units = extract_units(sentence);
    let measurement = MeasurementLevel::from_keyword(sentence);
    let operationalization = extract_operationalization(sentence);
    let stripped = strip_units_for_matching(sentence);

    for cue in role_cues() {
        for caps in cue.re.captures_iter(&stripped) {
            for &group_idx in &cue.name_groups {
                let Some(m) = caps.get(group_idx) else { continue };
                let name = clean_name(m.as_str());
                if name.is_empty() {
                    continue;
                }
                out.push(RawVariable {
                    name,
                    role: cue.role.clone(),
                    confidence: cue.confidence,
                    loc: loc.clone(),
                    sentence_index,
                    units: units.clone(),
                    measurement: measurement.clone(),
                    operationalization: operationalization.clone(),
                });
            }
        }
    }

    out
}

/// Normalize a captured variable name.
fn clean_name(text: &str) -> String {
    let trimmed = text.trim().trim_end_matches(|c: char| c.is_ascii_punctuation());
    // Drop leading articles.
    let lower = trimmed.to_lowercase();
    let without_article = lower
        .strip_prefix("the ")
        .or_else(|| lower.strip_prefix("a "))
        .or_else(|| lower.strip_prefix("an "))
        .unwrap_or(&lower);
    let name = without_article.trim().to_string();
    if name.len() > MAX_NAME_LEN {
        name[..MAX_NAME_LEN].to_string()
    } else {
        name
    }
}

/// Canonical key used to merge aliases.
fn canonical_key(name: &str) -> String {
    name.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract a unit from parentheses near a measurement, e.g. "age (years)".
fn extract_units(sentence: &str) -> Option<String> {
    let re = var_pat1_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty() && !looks_like_citation(s))
}

fn looks_like_citation(text: &str) -> bool {
    let t = text.trim();
    // Citation-like parentheticals: "Smith et al., 2023", "n = 120"
    var_pat2_re().is_match(t)
}

/// Extract a short operationalization phrase from the sentence, if present.
fn extract_operationalization(sentence: &str) -> Option<String> {
    let re = var_pat4_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

fn index_stats_by_location(result: &ExtractionResult) -> HashMap<(SectionKind, usize), Vec<usize>> {
    let mut map: HashMap<(SectionKind, usize), Vec<usize>> = HashMap::new();
    for (i, stat) in result.statistics.iter().enumerate() {
        map.entry((stat.location.section, stat.location.paragraph))
            .or_default()
            .push(i);
    }
    map
}

fn associated_stats(
    index: &HashMap<(SectionKind, usize), Vec<usize>>,
    canonical_name: &str,
    aliases: &[String],
    result: &ExtractionResult,
) -> Vec<StatRef> {
    let mut out = Vec::new();
    for ((_section, _paragraph), indices) in index {
        for &i in indices {
            let raw = result.statistics[i].stat.raw().to_lowercase();
            let name_lower = canonical_name.to_lowercase();
            if raw.contains(&name_lower)
                || aliases.iter().any(|a| raw.contains(&a.to_lowercase()))
            {
                out.push(StatRef {
                    index: i,
                    location: result.statistics[i].location.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.index.cmp(&b.index));
    out.dedup_by(|a, b| a.index == b.index && a.location == b.location);
    out
}

fn name_appears_in_claim(
    canonical_name: &str,
    aliases: &[String],
    claim: &crate::scientific_model::ScientificClaim,
) -> bool {
    let text = claim.statement.to_lowercase();
    let name_lower = canonical_name.to_lowercase();
    if text.contains(&name_lower) {
        return true;
    }
    aliases.iter().any(|a| text.contains(&a.to_lowercase()))
}


// --- cached regexes (were compiled per sentence; see tests/regex_compilation_guard.rs) ---

fn var_pat1_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\(([A-Za-z0-9°%\-/·\.\s]{1,30})\)").expect("var_pat1_re must compile"))
}

fn var_pat2_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^[A-Z][a-z]+\s+et\s+al\.?|^\d{4}$|^n\s*[=\s]").expect("var_pat2_re must compile"))
}

fn var_pat3_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\([^)]{1,40}\)").expect("var_pat3_re must compile"))
}

fn var_pat4_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:measured|assessed|operationalized|quantified)\s+(?:as|by|using|with)\s+(.{3,80}?)(?:\.|,|;|$)").expect("var_pat4_re must compile"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;

    #[test]
    fn no_variables_in_generic_text() {
        let text = "The weather was pleasant. The conference took place in June.";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.is_empty(), "generic prose must yield no variables");
    }

    #[test]
    fn detects_independent_and_dependent_variables() {
        let text = "\nA Study\n\nMethods\nSleep duration was the independent variable and memory performance was the dependent variable.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.iter().any(|v| v.role == VariableRole::Independent && v.canonical_name.contains("sleep duration")));
        assert!(vars.iter().any(|v| v.role == VariableRole::Dependent && v.canonical_name.contains("memory performance")));
    }

    #[test]
    fn extracts_units_and_measurement_level() {
        let text = "\nA Study\n\nMethods\nParticipant age (years) was a continuous covariate.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        let age = vars.iter().find(|v| v.canonical_name.contains("age"));
        assert!(age.is_some(), "expected age variable; got {:?}", vars);
        let age = age.unwrap();
        assert_eq!(age.units.as_deref(), Some("years"));
        assert!(matches!(age.measurement, crate::scientific_model::MeasurementLevel::Continuous));
    }

    #[test]
    fn controlled_for_yields_covariate() {
        let text = "\nA Study\n\nMethods\nWe controlled for sex and education level.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.iter().any(|v| v.role == VariableRole::Covariate && v.canonical_name.contains("sex")));
        assert!(vars.iter().any(|v| v.role == VariableRole::Covariate && v.canonical_name.contains("education")));
    }

    #[test]
    fn predictor_language_detects_predictor_and_outcome() {
        let text = "\nA Study\n\nMethods\nSleep quality predicted cognitive function in older adults.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.iter().any(|v| v.role == VariableRole::Predictor && v.canonical_name.contains("sleep quality")));
    }

    #[test]
    fn exposure_detected() {
        let text = "\nA Study\n\nMethods\nThe exposure was moderate physical activity.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.iter().any(|v| v.role == VariableRole::Exposure && v.canonical_name.contains("physical activity")));
    }

    #[test]
    fn variables_associate_with_claims() {
        let text = "\nA Study\n\nAbstract\nThis study demonstrates that sleep duration improves memory consolidation in older adults.\n\nMethods\nSleep duration was the independent variable.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        let sleep = vars.iter().find(|v| v.canonical_name.contains("sleep duration"));
        assert!(sleep.is_some());
        assert!(!sleep.unwrap().associated_claims.is_empty(), "sleep duration should link to the main claim");
    }

    #[test]
    fn duplicate_names_are_merged() {
        let text = "\nA Study\n\nMethods\nAge (years) was a continuous covariate. Age was included as a covariate.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        let age_count = vars.iter().filter(|v| v.canonical_name.contains("age")).count();
        assert_eq!(age_count, 1, "age should be merged into a single variable");
        let age = vars.iter().find(|v| v.canonical_name.contains("age")).unwrap();
        assert_eq!(age.sentence_indices.len(), 2, "age should have two sentence mentions");
    }

    #[test]
    fn rct_style_paper() {
        let text = "\nA Study\n\nMethods\nParticipants were randomized to a treatment group. The independent variable was treatment condition (active vs. placebo). The dependent variable was symptom severity, measured on a 0-10 scale. We controlled for age, sex, and baseline severity.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.iter().any(|v| v.role == VariableRole::Independent));
        assert!(vars.iter().any(|v| v.role == VariableRole::Dependent));
        assert!(vars.iter().any(|v| v.role == VariableRole::Covariate));
    }

    #[test]
    fn observational_study_paper() {
        let text = "\nA Study\n\nMethods\nThis observational study examined whether air pollution exposure predicts lung function decline. The exposure was fine particulate matter. The outcome was forced expiratory volume.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.iter().any(|v| v.role == VariableRole::Exposure));
        assert!(vars.iter().any(|v| v.role == VariableRole::Outcome));
    }

    #[test]
    fn no_variable_paper() {
        let text = "\nA Study\n\nDiscussion\nThis is a commentary on current trends.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        assert!(vars.is_empty());
    }

    #[test]
    fn variable_ids_are_unique() {
        let text = "\nA Study\n\nMethods\nX was the independent variable. Y was the dependent variable. Z was a covariate.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = extract_variables(&ex, &claims);
        let ids: std::collections::HashSet<_> = vars.iter().map(|v| &v.id.0).collect();
        assert_eq!(ids.len(), vars.len(), "variable ids must be unique");
    }
}
