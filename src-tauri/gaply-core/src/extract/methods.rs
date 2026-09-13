//! Deterministic scientific-method extraction.
//!
//! Stage 1 (implemented): rule-based detection of study design, sampling,
//! randomization, blinding, software, and statistical tests from the Methods
//! section and related paragraphs.
//!
//! Stage 2 (planned): optional local-SLM refinement.
//! Stage 3 (planned): optional cloud refinement with bounded metadata only.

use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

use crate::extract::{ExtractionResult, Location, SectionKind, sentence};
use crate::scientific_model::{
    ClaimId, Method, MethodId, MethodSource, SamplingDescription, SourceSpan, StatRef,
    StatisticalTestRef, StudyDesign, VariableId,
};

/// Maximum length of short text fields stored on a method.
const MAX_FIELD_LEN: usize = 200;

/// Known scientific software packages and programming environments.
fn software_list() -> &'static Vec<&'static str> {
    static LIST: OnceLock<Vec<&str>> = OnceLock::new();
    LIST.get_or_init(|| {
        vec![
            "R", "Python", "SPSS", "SAS", "Stata", "MATLAB", "GraphPad Prism",
            "TensorFlow", "PyTorch", "Keras", "scikit-learn", "scikitlearn",
            "JASP", " jamovi ", "jamovi", "Mplus", "HLM", "AMOS", "lisrel",
            "OpenAI Gym", "NLTK", "spaCy", "Stan", "JAGS", "BUGS",
        ]
    })
}

/// Known machine-learning / scientific libraries.
fn library_list() -> &'static Vec<&'static str> {
    static LIST: OnceLock<Vec<&str>> = OnceLock::new();
    LIST.get_or_init(|| {
        vec![
            "NumPy", "Pandas", "SciPy", "Matplotlib", "Seaborn", "Plotly",
            "OpenCV", "Hugging Face", "Transformers", "LangChain",
        ]
    })
}

/// A design cue with the study design it signals and a confidence score.
struct DesignCue {
    design: StudyDesign,
    confidence: f64,
    re: Regex,
}

fn design_cues() -> &'static Vec<DesignCue> {
    static CUES: OnceLock<Vec<DesignCue>> = OnceLock::new();
    CUES.get_or_init(|| {
        vec![
            DesignCue {
                design: StudyDesign::ClinicalTrial,
                confidence: 0.98,
                re: Regex::new(r"(?i)\brandomized controlled trial\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::ClinicalTrial,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bclinical trial\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::ClinicalTrial,
                confidence: 0.88,
                re: Regex::new(r"(?i)\brandomly\s+(?:assigned|allocated)\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::ClinicalTrial,
                confidence: 0.85,
                re: Regex::new(r"(?i)\brandomized\s+(?:into|to)\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::CrossSectional,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bcross-sectional\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::Longitudinal,
                confidence: 0.95,
                re: Regex::new(r"(?i)\blongitudinal\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::Cohort,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bcohort\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::CaseControl,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bcase[- ]control\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::Survey,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bsurvey\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::Qualitative,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bqualitative\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::MixedMethods,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bmixed[- ]methods?\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::SystematicReview,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bsystematic review\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::MetaAnalysis,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bmeta[- ]analysis\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::Simulation,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bsimulation\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::MachineLearningPipeline,
                confidence: 0.92,
                re: Regex::new(r"(?i)\b(machine learning|deep learning|neural network)\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::ComputationalExperiment,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bcomputational experiment\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::Experimental,
                confidence: 0.75,
                re: Regex::new(r"(?i)\bexperimental (design|study)\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::QuasiExperimental,
                confidence: 0.85,
                re: Regex::new(r"(?i)\bquasi[- ]experimental\b").unwrap(),
            },
            DesignCue {
                design: StudyDesign::Observational,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bobservational\b").unwrap(),
            },
        ]
    })
}

/// Extract methods from a fully populated `ExtractionResult`.
///
/// This is Stage 1: deterministic, local, no AI. It never fails the pipeline.
pub fn extract_methods(
    result: &ExtractionResult,
    claims: &[crate::scientific_model::ScientificClaim],
    variables: &[crate::scientific_model::Variable],
) -> Vec<Method> {
    let mut methods: Vec<Method> = Vec::new();

    for section in &result.sections {
        if section.kind == SectionKind::References {
            continue;
        }
        for (p_idx, paragraph) in section.paragraphs.iter().enumerate() {
            let loc = Location {
                section: section.kind,
                paragraph: p_idx,
            };
            if let Some(method) = extract_method_from_paragraph(
                paragraph, loc, result, claims, variables,
            ) {
                methods.push(method);
            }
        }
    }

    methods
}

fn extract_method_from_paragraph(
    paragraph: &str,
    loc: Location,
    result: &ExtractionResult,
    claims: &[crate::scientific_model::ScientificClaim],
    variables: &[crate::scientific_model::Variable],
) -> Option<Method> {
    let sentences = sentence::sentences_in(paragraph);
    if sentences.is_empty() {
        return None;
    }

    // Detect the dominant design across all sentences in the paragraph.
    // If no explicit design cue is present but the paragraph contains strong
    // method signals (software, sampling, algorithms, metrics), fall back to a
    // generic analytical method so that pure analysis paragraphs are captured.
    let (design, confidence) = match sentences
        .iter()
        .filter_map(|s| detect_design(s))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        Some(d) => d,
        None => {
            if has_method_signals(&sentences) {
                (StudyDesign::Other("analytical".into()), 0.6)
            } else {
                return None;
            }
        }
    };

    // Aggregate method metadata from the whole paragraph.
    let mut sampling = SamplingDescription {
        method: None,
        frame: None,
        size: None,
    };
    let mut intervention: Option<String> = None;
    let mut comparator: Option<String> = None;
    let mut randomization: Option<String> = None;
    let mut blinding: Option<String> = None;
    let mut software: Vec<String> = Vec::new();
    let mut libraries: Vec<String> = Vec::new();
    let mut algorithms: Vec<String> = Vec::new();
    let mut evaluation_metrics: Vec<String> = Vec::new();

    for sentence in &sentences {
        let s = extract_sampling(sentence);
        if sampling.size.is_none() {
            sampling.size = s.size;
        }
        if sampling.method.is_none() {
            sampling.method = s.method;
        }
        if intervention.is_none() {
            intervention = extract_intervention(sentence);
        }
        if comparator.is_none() {
            comparator = extract_comparator(sentence);
        }
        if randomization.is_none() {
            randomization = extract_randomization(sentence);
        }
        if blinding.is_none() {
            blinding = extract_blinding(sentence);
        }
        software.extend(extract_software(sentence));
        libraries.extend(extract_libraries(sentence));
        algorithms.extend(extract_algorithms(sentence));
        evaluation_metrics.extend(extract_evaluation_metrics(sentence));
    }

    software.sort();
    software.dedup();
    libraries.sort();
    libraries.dedup();
    let algo_set: std::collections::HashSet<_> = algorithms.into_iter().collect();
    algorithms = algo_set.into_iter().collect();
    let metric_set: std::collections::HashSet<_> = evaluation_metrics.into_iter().collect();
    evaluation_metrics = metric_set.into_iter().collect();

    let procedure_summary = Some(truncate(paragraph.trim(), MAX_FIELD_LEN * 2));
    let stat_tests = associated_stat_tests(result, &loc, paragraph);
    let associated_variables = associated_variables(variables, paragraph);
    let associated_claims = associated_claims(claims, paragraph);

    Some(Method {
        id: MethodId(format!("method-{}-{}", loc.section as u8, loc.paragraph)),
        name: method_name_for_design(&design),
        design,
        experimental_design: None,
        sampling,
        intervention,
        comparator,
        randomization,
        blinding,
        procedure_summary,
        materials: Vec::new(),
        software,
        libraries,
        algorithms,
        evaluation_metrics,
        protocol_references: Vec::new(),
        statistical_tests: stat_tests,
        associated_variables,
        associated_claims,
        associated_datasets: Vec::new(),
        source_spans: vec![SourceSpan::Point(loc)],
        confidence,
        source: MethodSource::Deterministic,
    })
}

fn has_method_signals(sentences: &[&str]) -> bool {
    let joined = sentences.join(" ").to_lowercase();
    !extract_software(&joined).is_empty()
        || !extract_libraries(&joined).is_empty()
        || !extract_algorithms(&joined).is_empty()
        || !extract_evaluation_metrics(&joined).is_empty()
        || extract_sampling(&joined).size.is_some()
}

fn detect_design(sentence: &str) -> Option<(StudyDesign, f64)> {
    let mut best: Option<(StudyDesign, f64)> = None;
    for cue in design_cues() {
        if cue.re.is_match(sentence) {
            if best.as_ref().map(|(_, c)| *c < cue.confidence).unwrap_or(true) {
                best = Some((cue.design.clone(), cue.confidence));
            }
        }
    }
    best
}

fn method_name_for_design(design: &StudyDesign) -> Option<String> {
    let s = match design {
        StudyDesign::Experimental => "experimental study",
        StudyDesign::QuasiExperimental => "quasi-experimental study",
        StudyDesign::Observational => "observational study",
        StudyDesign::CrossSectional => "cross-sectional study",
        StudyDesign::Longitudinal => "longitudinal study",
        StudyDesign::CaseStudy => "case study",
        StudyDesign::CaseControl => "case-control study",
        StudyDesign::Cohort => "cohort study",
        StudyDesign::Survey => "survey study",
        StudyDesign::ClinicalTrial => "clinical trial",
        StudyDesign::Qualitative => "qualitative study",
        StudyDesign::MixedMethods => "mixed-methods study",
        StudyDesign::SystematicReview => "systematic review",
        StudyDesign::MetaAnalysis => "meta-analysis",
        StudyDesign::Simulation => "simulation study",
        StudyDesign::MachineLearningPipeline => "machine learning pipeline",
        StudyDesign::ComputationalExperiment => "computational experiment",
        StudyDesign::Other(_) => return None,
    };
    Some(s.to_string())
}

fn extract_sampling(sentence: &str) -> SamplingDescription {
    let re = sampling_size_re();
    let method = sampling_method_re()
        .captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());
    let size = re.captures(sentence)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<usize>().ok());
    SamplingDescription { method, frame: None, size }
}

fn extract_intervention(sentence: &str) -> Option<String> {
    let re = intervention_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| truncate(m.as_str().trim(), MAX_FIELD_LEN))
        .filter(|s| !s.is_empty())
}

fn extract_comparator(sentence: &str) -> Option<String> {
    let re = comparator_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_randomization(sentence: &str) -> Option<String> {
    let re = randomization_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_blinding(sentence: &str) -> Option<String> {
    let re = blinding_re();
    re.captures(sentence)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_software(sentence: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (re, sw) in software_res() {
        if re.is_match(sentence) {
            out.push(sw.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn extract_libraries(sentence: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (re, lib) in library_res() {
        if re.is_match(sentence) {
            out.push(lib.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn extract_algorithms(sentence: &str) -> Vec<String> {
    let re = algorithm_re();
    re.captures_iter(sentence)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn extract_evaluation_metrics(sentence: &str) -> Vec<String> {
    let re = evaluation_metric_re();
    re.captures_iter(sentence)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn associated_stat_tests(result: &ExtractionResult, loc: &Location, sentence: &str) -> Vec<StatisticalTestRef> {
    let mut out = Vec::new();
    let sentence_lower = sentence.to_lowercase();
    // `enumerate()` carries the index the old code recovered with
    // `result.statistics.iter().position(|s| s as *const _ == stat as *const _)`
    // — a linear scan over the same vector it was already iterating, to find an
    // index it already had. O(n^2) in statistics per matching sentence, and a
    // pointer-identity comparison whose correctness depended on the vector not
    // being reallocated between the two iterations.
    for (stat_index, stat) in result.statistics.iter().enumerate() {
        if stat.location.section == loc.section && stat.location.paragraph == loc.paragraph {
            let raw = stat.stat.raw().to_lowercase();
            if sentence_lower.contains(&raw) {
                out.push(StatisticalTestRef {
                    name: match stat.stat {
                        crate::extract::Stat::Test { ref name, .. } => name.clone(),
                        _ => stat.stat.raw().to_string(),
                    },
                    stat: Some(StatRef {
                        index: stat_index,
                        location: stat.location.clone(),
                    }),
                });
            }
        }
    }
    out
}

fn associated_variables(
    variables: &[crate::scientific_model::Variable],
    sentence: &str,
) -> Vec<VariableId> {
    let mut out = Vec::new();
    let sentence_lower = sentence.to_lowercase();
    for v in variables {
        let name_lower = v.canonical_name.to_lowercase();
        if sentence_lower.contains(&name_lower)
            || v.aliases.iter().any(|a| sentence_lower.contains(&a.to_lowercase()))
        {
            out.push(v.id.clone());
        }
    }
    out
}

fn associated_claims(
    claims: &[crate::scientific_model::ScientificClaim],
    sentence: &str,
) -> Vec<ClaimId> {
    let mut out = Vec::new();
    let sentence_lower = sentence.to_lowercase();
    for c in claims {
        if sentence_lower.contains(&c.statement.to_lowercase()) {
            out.push(c.id.clone());
        }
    }
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



/// Software names paired with their COMPILED regex.
///
/// `software_list()` was `OnceLock`-cached and the regex built from each entry
/// was not, so this compiled 25 patterns on every sentence. Same defect as
/// `datasets.rs`'s known-dataset loop, same fix.
fn software_res() -> &'static Vec<(Regex, &'static str)> {
    static RES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    RES.get_or_init(|| compile_word_patterns(software_list()))
}

/// Library names paired with their COMPILED regex. 10 patterns, per sentence.
fn library_res() -> &'static Vec<(Regex, &'static str)> {
    static RES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    RES.get_or_init(|| compile_word_patterns(library_list()))
}

/// Word-boundary-anchored, case-insensitive, escaped — the exact shape the two
/// loops built inline. One definition so they cannot drift apart.
fn compile_word_patterns(list: &'static [&'static str]) -> Vec<(Regex, &'static str)> {
    list.iter()
        .map(|s| {
            let escaped = regex::escape(s);
            (
                Regex::new(&format!(r"(?i)\b{escaped}\b")).expect("word pattern must compile"),
                *s,
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Cached regexes.
//
// Each of these was `Regex::new(...).unwrap()` inside a function called ONCE
// PER SENTENCE, so a 1,390-sentence manuscript paid ten compilations per
// sentence for patterns that never change. The pattern text is unchanged —
// only where it is compiled moved. Baseline and arithmetic:
// `examples/scientific_cost_probe.rs`.
// ---------------------------------------------------------------------------

fn sampling_size_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:n\s*=\s*|sample\s+(?:size\s+(?:of\s+)?|of\s+)|recruited\s+|enrolled\s+|(?:a\s+)?total\s+of\s+)(\d{1,6})(?:\s+participants|\s+subjects|\s+patients|\s+respondents|\s+students)?").expect("sampling_size_re must compile"))
}

fn sampling_method_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(convenience sample|random sample|stratified sample|cluster sample|snowball sample|purposive sample)\b").expect("sampling_method_re must compile"))
}

fn intervention_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:randomized|assigned|allocated)\s+(?:to|into)\s+([A-Za-z0-9\s\-]{3,80})(?:\s+and\s+)?(?:group|arm|condition)?").expect("intervention_re must compile"))
}

fn comparator_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(placebo|control group|standard care|usual care|waitlist control|sham)\b").expect("comparator_re must compile"))
}

fn randomization_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(randomly allocated|randomly assigned|computer[- ]generated|block randomization|stratified randomization|simple randomization)\b").expect("randomization_re must compile"))
}

fn blinding_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(double[- ]blind|single[- ]blind|triple[- ]blind|open[- ]label|observer[- ]blind|assessor[- ]blind)\b").expect("blinding_re must compile"))
}

fn algorithm_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(logistic regression|linear regression|random forest|support vector machine|SVM|k-nearest neighbor|k-NN|decision tree|gradient boosting|neural network|CNN|RNN|LSTM|transformer|BERT|GPT)\b").expect("algorithm_re must compile"))
}

fn evaluation_metric_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(accuracy|precision|recall|F1[- ]score|F1|AUROC|AUC[- ]ROC|mean squared error|RMSE|MAE|sensitivity|specificity|R[\u00b22])\b").expect("evaluation_metric_re must compile"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;

    #[test]
    fn detects_randomized_controlled_trial() {
        let text = "\nA Study\n\nMethods\nWe conducted a randomized controlled trial with 120 participants.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = crate::extract::variables::extract_variables(&ex, &claims);
        let methods = extract_methods(&ex, &claims, &vars);
        assert!(methods.iter().any(|m| matches!(m.design, StudyDesign::ClinicalTrial)));
    }

    #[test]
    fn extracts_software() {
        let text = "\nA Study\n\nMethods\nStatistical analyses were performed in R and SPSS.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = crate::extract::variables::extract_variables(&ex, &claims);
        let methods = extract_methods(&ex, &claims, &vars);
        let m = methods.iter().find(|m| !m.software.is_empty()).expect("expected software");
        assert!(m.software.iter().any(|s| s == "R"));
        assert!(m.software.iter().any(|s| s == "SPSS"));
    }

    #[test]
    fn extracts_sampling_size() {
        let text = "\nA Study\n\nMethods\nA total of 256 students were recruited via convenience sampling.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = crate::extract::variables::extract_variables(&ex, &claims);
        let methods = extract_methods(&ex, &claims, &vars);
        let m = methods.iter().find(|m| m.sampling.size.is_some()).expect("expected sample size");
        assert_eq!(m.sampling.size, Some(256));
    }

    #[test]
    fn detects_machine_learning_pipeline() {
        let text = "\nA Study\n\nMethods\nWe built a machine learning pipeline using Python, TensorFlow, and a transformer model. Accuracy and F1-score were used for evaluation.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = crate::extract::variables::extract_variables(&ex, &claims);
        let methods = extract_methods(&ex, &claims, &vars);
        let m = methods.iter().find(|m| matches!(m.design, StudyDesign::MachineLearningPipeline));
        assert!(m.is_some());
        let m = m.unwrap();
        assert!(m.software.iter().any(|s| s == "Python"));
        assert!(m.software.iter().any(|s| s == "TensorFlow"));
        assert!(m.evaluation_metrics.iter().any(|s| s.to_lowercase().contains("accuracy")));
        assert!(m.evaluation_metrics.iter().any(|s| s.to_lowercase().contains("f1")));
    }

    #[test]
    fn detects_blinding_and_comparator() {
        let text = "\nA Study\n\nMethods\nParticipants were randomly assigned to active treatment or placebo in a double-blind fashion.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = crate::extract::variables::extract_variables(&ex, &claims);
        let methods = extract_methods(&ex, &claims, &vars);
        let m = methods.iter().find(|m| matches!(m.design, StudyDesign::ClinicalTrial));
        assert!(m.is_some());
        let m = m.unwrap();
        assert!(m.blinding.as_deref().map(|s| s.to_lowercase().contains("double-blind")).unwrap_or(false));
        assert!(m.comparator.as_deref().map(|s| s.to_lowercase().contains("placebo")).unwrap_or(false));
    }

    #[test]
    fn no_methods_in_generic_text() {
        let text = "The weather was pleasant. The conference took place in June.";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = crate::extract::variables::extract_variables(&ex, &claims);
        let methods = extract_methods(&ex, &claims, &vars);
        assert!(methods.is_empty());
    }

    #[test]
    fn method_ids_are_unique() {
        let text = "\nA Study\n\nMethods\nWe used a cross-sectional survey design.\n";
        let ex = extract_from_text(text);
        let claims = crate::extract::claims::extract_claims(&ex);
        let vars = crate::extract::variables::extract_variables(&ex, &claims);
        let methods = extract_methods(&ex, &claims, &vars);
        let ids: std::collections::HashSet<_> = methods.iter().map(|m| &m.id.0).collect();
        assert_eq!(ids.len(), methods.len());
    }
}
