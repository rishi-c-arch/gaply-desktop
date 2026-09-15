//! Deterministic scientific-dataset extraction.
//!
//! Stage 1 (implemented): rule-based detection of dataset names, sample sizes,
//! sources, repositories, and DOIs from Methods, Results, and Appendix sections.
//!
//! Stage 2 (planned): optional local-SLM refinement.
//! Stage 3 (planned): optional cloud refinement with bounded metadata only.

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::extract::{ExtractionResult, Location, SectionKind, sentence};
use crate::scientific_model::{
    ClaimId, DataSource, Dataset, DatasetId, MethodId, SourceSpan, VariableId,
};

/// "data were obtained from X" — the generic dataset-source phrasing. A const so
/// the pattern text sits beside its sibling and neither can drift into a
/// per-call `Regex::new`.
const DATA_FROM_PATTERN: &str = r"(?i)(?:data|dataset|corpus|database)\s+(?:were|was)\s+(?:obtained|collected|downloaded|extracted|gathered)\s+(?:from|via)\s+([A-Za-z0-9\s\-]{3,60})";

/// Institution / clinical dataset mentions.
const CLINICAL_PATTERN: &str = r"(?i)(?:data|dataset|records)\s+(?:from|at)\s+(?:the\s+)?([A-Za-z][A-Za-z0-9\s,\.\-]{3,80}\s+(?:Hospital|University|Clinic|Center|Centre|Institute|Registry))";

/// Maximum length of stored dataset text fields.
const MAX_FIELD_LEN: usize = 200;

/// Well-known public datasets and benchmarks. Names are normalized on output.
fn known_datasets() -> &'static Vec<(Regex, &'static str)> {
    static LIST: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    LIST.get_or_init(|| {
        raw_known_datasets()
            .iter()
            .map(|(pattern, normalized)| {
                let escaped = regex::escape(pattern);
                (
                    Regex::new(&format!(r"(?i)\b{escaped}\b"))
                        .expect("known-dataset pattern must compile"),
                    *normalized,
                )
            })
            .collect()
    })
}

/// The raw patterns. Split out of [`known_datasets`] so the names stay readable
/// and [`known_dataset_names`] has something to build from.
fn raw_known_datasets() -> &'static Vec<(&'static str, &'static str)> {
    static LIST: OnceLock<Vec<(&str, &str)>> = OnceLock::new();
    LIST.get_or_init(|| {
        vec![
            ("MNIST", "MNIST"),
            ("ImageNet", "ImageNet"),
            ("CIFAR-10", "CIFAR-10"),
            ("CIFAR-100", "CIFAR-100"),
            ("COCO", "COCO"),
            ("Open Images", "Open Images"),
            ("WikiText", "WikiText"),
            ("GLUE", "GLUE"),
            ("SQuAD", "SQuAD"),
            ("UK Biobank", "UK Biobank"),
            ("NHANES", "NHANES"),
            ("BRFSS", "BRFSS"),
            ("SEER", "SEER"),
            ("MIMIC-III", "MIMIC-III"),
            ("MIMIC-IV", "MIMIC-IV"),
            ("PubMed", "PubMed"),
            ("arXiv", "arXiv"),
            ("Wikipedia", "Wikipedia"),
            ("Common Crawl", "Common Crawl"),
            ("GitHub", "GitHub"),
            ("Stack Overflow", "Stack Overflow"),
        ]
    })
}

/// Section kinds that may contain dataset descriptions.
fn dataset_sections() -> &'static [SectionKind] {
    static SECTIONS: OnceLock<[SectionKind; 4]> = OnceLock::new();
    SECTIONS.get_or_init(|| {
        [SectionKind::Abstract, SectionKind::Methods, SectionKind::Results, SectionKind::Other]
    })
}

/// Extract datasets from a fully populated `ExtractionResult`.
///
/// This is Stage 1: deterministic, local, no AI. It never fails the pipeline.
pub fn extract_datasets(
    result: &ExtractionResult,
    claims: &[crate::scientific_model::ScientificClaim],
    variables: &[crate::scientific_model::Variable],
    methods: &[crate::scientific_model::Method],
) -> Vec<Dataset> {
    let mut raw: Vec<RawDataset> = Vec::new();

    for (sec_idx, section) in result.sections.iter().enumerate() {
        if section.kind == SectionKind::References || !dataset_sections().contains(&section.kind) {
            continue;
        }
        for (p_idx, paragraph) in section.paragraphs.iter().enumerate() {
            let loc = Location::in_section(section.kind, sec_idx, p_idx);
            let sentences = sentence::sentences_in(paragraph);
            for (s_idx, sentence_text) in sentences.iter().enumerate() {
                raw.extend(extract_datasets_from_sentence(
                    sentence_text, loc.clone(), s_idx, result,
                ));
            }
        }
    }

    // Merge by normalized name so the same dataset mentioned twice is one object.
    let mut by_key: HashMap<String, RawDataset> = HashMap::new();
    for rd in raw {
        let key = normalize_name(&rd.name);
        match by_key.get_mut(&key) {
            Some(existing) => existing.merge(rd),
            None => {
                by_key.insert(key, rd);
            }
        }
    }

    // Build final datasets with deterministic IDs and associations.
    let mut datasets: Vec<Dataset> = Vec::new();
    for (idx, (key, rd)) in by_key.into_iter().enumerate() {
        let id = DatasetId(format!("dataset-{}", idx + 1));
        let associated_claims = associated_claims(claims, &rd.name, &rd.sentence_texts);
        let associated_methods = associated_methods(methods, &rd.name, &rd.sentence_texts);
        let associated_variables = associated_variables(variables, &rd.sentence_texts);

        datasets.push(Dataset {
            id,
            name: Some(rd.name.clone()),
            normalized_name: Some(key),
            sample_size: rd.sample_size,
            sample_description: rd.sample_description,
            population: rd.population,
            country: rd.country,
            region: rd.region,
            recruitment: rd.recruitment,
            time_period: rd.time_period,
            source: rd.source,
            institution: rd.institution,
            version: rd.version,
            doi: rd.doi,
            repository: rd.repository,
            license: rd.license,
            inclusion_criteria: None,
            exclusion_criteria: None,
            variables: associated_variables,
            associated_claims,
            associated_methods,
            missing_data_note: None,
            source_spans: rd.source_spans,
            confidence: rd.confidence,
        });
    }

    datasets.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    datasets
}

#[derive(Debug, Clone)]
struct RawDataset {
    name: String,
    sample_size: Option<usize>,
    sample_description: Option<String>,
    population: Option<String>,
    country: Option<String>,
    region: Option<String>,
    recruitment: Option<String>,
    time_period: Option<String>,
    source: DataSource,
    institution: Option<String>,
    version: Option<String>,
    doi: Option<String>,
    repository: Option<String>,
    license: Option<String>,
    source_spans: Vec<SourceSpan>,
    sentence_texts: Vec<String>,
    confidence: f64,
}

impl RawDataset {
    fn merge(&mut self, other: RawDataset) {
        self.source_spans.extend(other.source_spans);
        self.source_spans.sort_by(|a, b| match (a, b) {
            (SourceSpan::Point(al), SourceSpan::Point(bl)) => al.cmp(bl),
            _ => std::cmp::Ordering::Equal,
        });
        self.source_spans.dedup();
        self.sentence_texts.extend(other.sentence_texts);
        if other.confidence > self.confidence {
            self.confidence = other.confidence;
            self.source = other.source;
        }
        if self.sample_size.is_none() {
            self.sample_size = other.sample_size;
        }
        if self.sample_description.is_none() {
            self.sample_description = other.sample_description;
        }
        if self.population.is_none() {
            self.population = other.population;
        }
        if self.country.is_none() {
            self.country = other.country;
        }
        if self.region.is_none() {
            self.region = other.region;
        }
        if self.recruitment.is_none() {
            self.recruitment = other.recruitment;
        }
        if self.time_period.is_none() {
            self.time_period = other.time_period;
        }
        if self.institution.is_none() {
            self.institution = other.institution;
        }
        if self.version.is_none() {
            self.version = other.version;
        }
        if self.doi.is_none() {
            self.doi = other.doi;
        }
        if self.repository.is_none() {
            self.repository = other.repository;
        }
        if self.license.is_none() {
            self.license = other.license;
        }
    }
}

fn extract_datasets_from_sentence(
    sentence: &str,
    loc: Location,
    _sentence_index: usize,
    result: &ExtractionResult,
) -> Vec<RawDataset> {
    let mut out = Vec::new();

    // 1. Known public datasets.
    for (re, normalized) in known_datasets() {
        if re.is_match(sentence) {
            out.push(build_dataset(
                normalized.to_string(),
                0.95,
                DataSource::PublicDataset(normalized.to_string()),
                loc.clone(),
                sentence,
                result,
            ));
        }
    }

    // 2. Generic dataset source language: "data were obtained from X".
    for caps in from_re().captures_iter(sentence) {
        if let Some(m) = caps.get(1) {
            let name = clean_name(m.as_str());
            if !name.is_empty() && !known_dataset_names().contains(&name.to_lowercase()) {
                out.push(build_dataset(
                    name,
                    0.75,
                    infer_data_source(sentence),
                    loc.clone(),
                    sentence,
                    result,
                ));
            }
        }
    }

    // 3. Institution / clinical dataset mentions.
    for caps in clinical_re().captures_iter(sentence) {
        if let Some(m) = caps.get(1) {
            let name = clean_name(m.as_str());
            if !name.is_empty() && !known_dataset_names().contains(&name.to_lowercase()) {
                out.push(build_dataset(
                    name.clone(),
                    0.8,
                    DataSource::Primary,
                    loc.clone(),
                    sentence,
                    result,
                ));
            }
        }
    }

    // Tag each raw dataset with its sentence index implicitly through source_spans.
    for rd in &mut out {
        rd.sentence_texts.push(sentence.to_string());
    }

    out
}

fn build_dataset(
    name: String,
    confidence: f64,
    source: DataSource,
    loc: Location,
    sentence: &str,
    result: &ExtractionResult,
) -> RawDataset {
    RawDataset {
        name: name.clone(),
        sample_size: extract_sample_size(sentence).or_else(|| loc_sample_size(result, &loc)),
        sample_description: extract_sample_description(sentence),
        population: extract_population(sentence),
        country: extract_country(sentence),
        region: None,
        recruitment: extract_recruitment(sentence),
        time_period: extract_time_period(sentence),
        source,
        institution: extract_institution(sentence),
        version: extract_version(sentence),
        doi: extract_doi(sentence),
        repository: extract_repository(sentence),
        license: extract_license(sentence),
        source_spans: vec![SourceSpan::Point(loc)],
        sentence_texts: Vec::new(),
        confidence,
    }
}

/// Cached. This rebuilt the whole `HashSet` on every call and is called twice
/// per sentence, so it allocated 21 lowercased strings and a set each time.
fn known_dataset_names() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        raw_known_datasets()
            .iter()
            .map(|(_, normalized)| normalized.to_lowercase())
            .collect()
    })
}

/// "data were obtained from X". Cached — was compiled on every sentence.
fn from_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(DATA_FROM_PATTERN).expect("from_re must compile")
    })
}

/// Institution / clinical dataset mentions. Cached — was compiled per sentence.
fn clinical_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(CLINICAL_PATTERN).expect("clinical_re must compile")
    })
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn clean_name(text: &str) -> String {
    let trimmed = text.trim().trim_end_matches(|c: char| c.is_ascii_punctuation());
    let lower = trimmed.to_lowercase();
    let without_articles = lower
        .strip_prefix("the ")
        .or_else(|| lower.strip_prefix("a "))
        .or_else(|| lower.strip_prefix("an "))
        .map(|s| s.to_string())
        .unwrap_or_else(|| trimmed.to_string());
    truncate(&without_articles, MAX_FIELD_LEN)
}

fn infer_data_source(sentence: &str) -> DataSource {
    let lower = sentence.to_lowercase();
    if lower.contains("simulated") || lower.contains("synthetic") {
        DataSource::Simulated
    } else if lower.contains("publicly available") || lower.contains("public dataset") {
        DataSource::PublicDataset("public".into())
    } else if lower.contains("proprietary") || lower.contains("commercial") {
        DataSource::Proprietary
    } else {
        DataSource::Primary
    }
}

fn extract_sample_size(sentence: &str) -> Option<usize> {
    let re = ds_pat1_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<usize>().ok())
}

fn loc_sample_size(result: &ExtractionResult, loc: &Location) -> Option<usize> {
    result
        .statistics
        .iter()
        .find(|s| s.location == *loc && matches!(s.stat, crate::extract::Stat::SampleSize { .. }))
        .and_then(|s| match s.stat {
            crate::extract::Stat::SampleSize { n, .. } => Some(n as usize),
            _ => None,
        })
}

fn extract_sample_description(sentence: &str) -> Option<String> {
    let re = ds_pat2_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| truncate(m.as_str().trim(), MAX_FIELD_LEN))
}

fn extract_population(sentence: &str) -> Option<String> {
    let re = ds_pat3_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| truncate(m.as_str().trim(), MAX_FIELD_LEN))
}

fn extract_country(sentence: &str) -> Option<String> {
    let re = ds_pat4_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_recruitment(sentence: &str) -> Option<String> {
    let re = ds_pat5_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_time_period(sentence: &str) -> Option<String> {
    let re = ds_pat6_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_institution(sentence: &str) -> Option<String> {
    let re = ds_pat7_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| truncate(m.as_str().trim(), MAX_FIELD_LEN))
}

fn extract_version(sentence: &str) -> Option<String> {
    let re = ds_pat8_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_doi(sentence: &str) -> Option<String> {
    let re = ds_pat9_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_repository(sentence: &str) -> Option<String> {
    let re = ds_pat10_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_license(sentence: &str) -> Option<String> {
    let re = ds_pat11_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn associated_claims(
    claims: &[crate::scientific_model::ScientificClaim],
    dataset_name: &str,
    sentence_texts: &[String],
) -> Vec<ClaimId> {
    let mut out = Vec::new();
    let name_lower = dataset_name.to_lowercase();
    for c in claims {
        let claim_lower = c.statement.to_lowercase();
        if claim_lower.contains(&name_lower) {
            out.push(c.id.clone());
            continue;
        }
        for s in sentence_texts {
            if claim_lower.contains(&s.to_lowercase()) {
                out.push(c.id.clone());
                break;
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup();
    out
}

fn associated_methods(
    methods: &[crate::scientific_model::Method],
    dataset_name: &str,
    sentence_texts: &[String],
) -> Vec<MethodId> {
    let mut out = Vec::new();
    let name_lower = dataset_name.to_lowercase();
    for m in methods {
        let mut linked = false;
        for _span in &m.source_spans {
            for s in sentence_texts {
                if s.to_lowercase().contains(&name_lower) {
                    linked = true;
                    break;
                }
            }
            if linked {
                break;
            }
        }
        if linked {
            out.push(m.id.clone());
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup();
    out
}

fn associated_variables(
    variables: &[crate::scientific_model::Variable],
    sentence_texts: &[String],
) -> Vec<VariableId> {
    let mut out = Vec::new();
    for v in variables {
        let name_lower = v.canonical_name.to_lowercase();
        for s in sentence_texts {
            if s.to_lowercase().contains(&name_lower)
                || v.aliases.iter().any(|a| s.to_lowercase().contains(&a.to_lowercase()))
            {
                out.push(v.id.clone());
                break;
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup();
    out
}

fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        match text[..max_len].rfind(|c: char| c.is_whitespace()) {
            Some(i) if i > 0 => text[..i].to_string(),
            _ => text[..max_len].to_string(),
        }
    }
}

fn ds_pat1_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:n\s*=\s*|sample\s+(?:size\s+(?:of\s+)?|of\s+)|total of\s+|included\s+)(\d{1,7})(?:\s+(?:participants|subjects|patients|records|images|samples))?").expect("ds_pat1_re must compile"))
}

fn ds_pat2_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:sample of\s+|participants were\s+|included\s+)([A-Za-z0-9\s,\-]{3,80})(?:\s+from\s+|$|\.)").expect("ds_pat2_re must compile"))
}

fn ds_pat3_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:population of\s+|sample of\s+)([A-Za-z0-9\s,\-]{3,60})(?:\s+(?:from|in|with|aged)|\.|,)").expect("ds_pat3_re must compile"))
}

fn ds_pat4_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(in the United States|in the US|in the UK|in the Netherlands|in Germany|in France|in China|in India|in Australia|in Canada|in Japan|in Brazil|in Spain|in Italy)\b").expect("ds_pat4_re must compile"))
}

fn ds_pat5_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(recruited through|recruited via|recruited from|convenience sampling|random sampling|stratified sampling|snowball sampling|purposive sampling)\b").expect("ds_pat5_re must compile"))
}

fn ds_pat6_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(between\s+\d{4}\s+and\s+\d{4}|from\s+\d{4}\s+to\s+\d{4}|from\s+January\s+\d{4}|in\s+\d{4})\b").expect("ds_pat6_re must compile"))
}

fn ds_pat7_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b([A-Za-z][A-Za-z0-9\s,\.\-]{3,80}(?:Hospital|University|Clinic|Center|Centre|Institute|Registry))\b").expect("ds_pat7_re must compile"))
}

fn ds_pat8_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(version\s+\d+(?:\.\d+)?|v\d+(?:\.\d+)?)\b").expect("ds_pat8_re must compile"))
}

fn ds_pat9_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(10\.\d{4,9}/[-._;()/:A-Za-z0-9]+)\b").expect("ds_pat9_re must compile"))
}

fn ds_pat10_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(github\.com/[^\s]+|gitlab\.com/[^\s]+|zenodo\.org/[^\s]+|figshare\.com/[^\s]+|osf\.io/[^\s]+|huggingface\.co/[^\s]+|kaggle\.com/[^\s]+)\b").expect("ds_pat10_re must compile"))
}

fn ds_pat11_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(CC0|CC-BY|CC BY|MIT license|Apache-2\.0|GPL|ODbL|public domain)\b").expect("ds_pat11_re must compile"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;

    fn extract_all(result: &ExtractionResult) -> (Vec<crate::scientific_model::ScientificClaim>, Vec<crate::scientific_model::Variable>, Vec<crate::scientific_model::Method>) {
        let claims = crate::extract::claims::extract_claims(result);
        let variables = crate::extract::variables::extract_variables(result, &claims);
        let methods = crate::extract::methods::extract_methods(result, &claims, &variables);
        (claims, variables, methods)
    }

    #[test]
    fn detects_known_public_dataset() {
        let text = "\nA Study\n\nMethods\nWe trained on ImageNet and evaluated on CIFAR-10.\n";
        let ex = extract_from_text(text);
        let (claims, variables, methods) = extract_all(&ex);
        let datasets = extract_datasets(&ex, &claims, &variables, &methods);
        assert!(datasets.iter().any(|d| d.normalized_name.as_deref() == Some("imagenet")));
        assert!(datasets.iter().any(|d| d.normalized_name.as_deref() == Some("cifar-10")));
    }

    #[test]
    fn detects_dataset_from_source_language() {
        let text = "\nA Study\n\nMethods\nData were obtained from the National Health Interview Survey.\n";
        let ex = extract_from_text(text);
        let (claims, variables, methods) = extract_all(&ex);
        let datasets = extract_datasets(&ex, &claims, &variables, &methods);
        assert!(datasets.iter().any(|d| d.name.as_deref().map(|n| n.to_lowercase().contains("national health interview survey")).unwrap_or(false)));
    }

    #[test]
    fn extracts_sample_size_for_dataset() {
        let text = "\nA Study\n\nMethods\nWe used the UK Biobank dataset (n = 502,000).\n";
        let ex = extract_from_text(text);
        let (claims, variables, methods) = extract_all(&ex);
        let datasets = extract_datasets(&ex, &claims, &variables, &methods);
        let ukbb = datasets.iter().find(|d| d.normalized_name.as_deref() == Some("uk biobank"));
        assert!(ukbb.is_some());
        assert!(ukbb.unwrap().sample_size.is_some());
    }

    #[test]
    fn extracts_dataset_repository() {
        let text = "\nA Study\n\nMethods\nThe code and data are available at github.com/example/repo.\n";
        let ex = extract_from_text(text);
        let (claims, variables, methods) = extract_all(&ex);
        let datasets = extract_datasets(&ex, &claims, &variables, &methods);
        assert!(datasets.iter().any(|d| d.repository.as_deref().map(|r| r.contains("github.com")).unwrap_or(false)));
    }

    #[test]
    fn no_datasets_in_generic_text() {
        let text = "The weather was pleasant. The conference took place in June.";
        let ex = extract_from_text(text);
        let (claims, variables, methods) = extract_all(&ex);
        let datasets = extract_datasets(&ex, &claims, &variables, &methods);
        assert!(datasets.is_empty());
    }

    #[test]
    fn dataset_ids_are_unique() {
        let text = "\nA Study\n\nMethods\nWe used MNIST and CIFAR-10.\n";
        let ex = extract_from_text(text);
        let (claims, variables, methods) = extract_all(&ex);
        let datasets = extract_datasets(&ex, &claims, &variables, &methods);
        let ids: std::collections::HashSet<_> = datasets.iter().map(|d| &d.id.0).collect();
        assert_eq!(ids.len(), datasets.len());
    }

    #[test]
    fn duplicate_dataset_names_are_merged() {
        let text = "\nA Study\n\nMethods\nWe trained on MNIST. MNIST contains 70,000 images.\n";
        let ex = extract_from_text(text);
        let (claims, variables, methods) = extract_all(&ex);
        let datasets = extract_datasets(&ex, &claims, &variables, &methods);
        assert_eq!(datasets.iter().filter(|d| d.normalized_name.as_deref() == Some("mnist")).count(), 1);
        let mnist = datasets.iter().find(|d| d.normalized_name.as_deref() == Some("mnist")).unwrap();
        assert_eq!(mnist.source_spans.len(), 1, "same-paragraph mentions share one source span");
    }
}
