//! **The four helper tasks and the one checked-in file that defines them.**
//!
//! `helper_tasks.json`, beside this file, holds for each task: its id, prompt
//! and schema versions, the fixed instructions text, the input schema (the
//! shape of the outbound struct, for review and for the tests that pin it),
//! the output format and schema, the tools it may use, and its budget (max
//! output tokens, timeout, max calls per run). It also holds the default model
//! id and the one list of domains `web_context` may search.
//!
//! The transport is T-BYOK (decisions.md), so there is no proxy build to read
//! the file: it lives in `gaply_core` and is compiled in with `include_str!`.
//! Its SHA-256 is pinned by a test, so an edit to an instruction or a schema
//! is a deliberate change with a new pin, never a drift.
//!
//! Output schemas use only what strict structured outputs supports (the
//! keyword list is in the test that checks it): no `maxLength`, so length
//! limits are enforced by the validators instead.

use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

use crate::stats_l1::catalogue::{L1MethodId, VariantKind};

/// The spec file, compiled in.
pub const SPEC_JSON: &str = include_str!("helper_tasks.json");

/// The helper tasks. Closed: exactly these four, no others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HelperTask {
    ClaimMethod,
    BindingSuggestion,
    ResultExplanation,
    WebContext,
}

impl HelperTask {
    pub const ALL: [HelperTask; 4] =
        [HelperTask::ClaimMethod, HelperTask::BindingSuggestion, HelperTask::ResultExplanation, HelperTask::WebContext];

    pub fn id(self) -> &'static str {
        match self {
            HelperTask::ClaimMethod => "claim_method",
            HelperTask::BindingSuggestion => "binding_suggestion",
            HelperTask::ResultExplanation => "result_explanation",
            HelperTask::WebContext => "web_context",
        }
    }

    pub fn from_id(id: &str) -> Option<HelperTask> {
        HelperTask::ALL.into_iter().find(|t| t.id() == id)
    }

    /// This task's entry in the spec file.
    pub fn spec(self) -> &'static TaskSpec {
        spec_file().tasks.iter().find(|s| s.id == self.id()).expect("every HelperTask has a spec entry (pinned by a test)")
    }
}

/// What the model's answer is: a strict JSON object, or plain text with
/// `url_citation` annotations (web search never runs with a json_schema
/// format, decisions.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    JsonSchema,
    Text,
}

/// One task's entry in the spec file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpec {
    pub id: String,
    pub prompt_version: u32,
    pub schema_version: u32,
    pub instructions: String,
    pub input_schema: Value,
    pub output_format: OutputFormat,
    /// `None` exactly when `output_format` is `Text`.
    pub output_schema: Option<Value>,
    /// `reasoning.effort` for this task. "low" for all four: the gpt-5.5
    /// model page lists "none, low, medium (default), high and xhigh", and the
    /// web search guide says reasoning levels on gpt-5.5 can be adjusted for
    /// search (both read 14 Oct 2026).
    pub reasoning_effort: Option<String>,
    /// Hosted tools only, by type. Never a function tool.
    pub tools: Vec<String>,
    pub max_output_tokens: u32,
    pub timeout_secs: u32,
    pub max_calls_per_run: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecFile {
    pub spec_version: u32,
    pub default_model: String,
    /// The one list `web_context`'s search is filtered to, and that every
    /// citation's host is checked against.
    pub web_allowed_domains: Vec<String>,
    pub tasks: Vec<TaskSpec>,
}

/// The parsed spec file. Parsed once; a malformed file is a build defect,
/// caught by the tests before it can ship.
pub fn spec_file() -> &'static SpecFile {
    static SPEC: OnceLock<SpecFile> = OnceLock::new();
    SPEC.get_or_init(|| serde_json::from_str(SPEC_JSON).expect("helper_tasks.json parses (pinned by a test)"))
}

pub fn allowed_web_domains() -> &'static [String] {
    &spec_file().web_allowed_domains
}

/// A model id: a setting, never hard-coded in a request (decisions.md). A
/// closed character set, so it cannot carry anything but a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelId(String);

impl ModelId {
    pub fn new(id: &str) -> Option<ModelId> {
        let ok = !id.is_empty()
            && id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        ok.then(|| ModelId(id.to_string()))
    }

    /// The spec file's default, used until the user changes the setting.
    pub fn default_model() -> ModelId {
        ModelId::new(&spec_file().default_model).expect("the default model id is well formed (pinned by a test)")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The variant names a method's suggestion may use, `not_stated` aside. The
/// strings the output schema enumerates.
pub fn variant_names(kind: VariantKind) -> &'static [&'static str] {
    match kind {
        VariantKind::None => &[],
        VariantKind::T => &["student", "welch"],
        VariantKind::Continuity => &["continuity_correction", "no_continuity_correction"],
        VariantKind::SpearmanP => &["t_approximation", "exact"],
    }
}

/// One line per catalogue method, for `claim_method`'s input.
pub fn method_description(m: L1MethodId) -> &'static str {
    match m {
        L1MethodId::IndependentT => "t-test comparing a numeric outcome between two independent groups",
        L1MethodId::PairedT => "t-test comparing two numeric measurements taken on the same subjects",
        L1MethodId::OneWayAnova => "one-way ANOVA F test comparing a numeric outcome across two or more groups",
        L1MethodId::Pearson => "Pearson correlation r between two numeric variables",
        L1MethodId::Spearman => "Spearman rank correlation rho between two numeric variables",
        L1MethodId::ChiSquareIndependence => "chi-square test of independence between two categorical variables",
        L1MethodId::Ols => "ordinary least squares regression: the overall F or one predictor's coefficient t",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l1::catalogue::MethodCatalogue;
    use crate::stats_l1::dataset::sha256_hex;

    /// Pinned: a change to any instruction, schema, budget or domain is a
    /// deliberate edit with a new pin.
    // Part B (evaluation fixes): result_explanation prompt and input schema
    // v2 (digits only, the verdict in its own words only, no misconduct
    // words, alpha_defaulted); web_context v2 (a DOI only, the title opt-in
    // removed, the query built from closed parts).
    const SPEC_SHA256: &str = "825699ad3043d3df4917112e2e674fee317fdfd27daa1ed4bd0f7a624938080e";

    #[test]
    fn the_spec_file_is_pinned() {
        assert_eq!(sha256_hex(SPEC_JSON.as_bytes()), SPEC_SHA256, "helper_tasks.json changed: review the change, then re-pin");
    }

    #[test]
    fn the_registry_is_closed_and_every_task_is_fully_specified() {
        let f = spec_file();
        assert_eq!(f.tasks.len(), 4, "exactly four helper tasks");
        let mut ids: Vec<&str> = f.tasks.iter().map(|t| t.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["binding_suggestion", "claim_method", "result_explanation", "web_context"]);
        for t in HelperTask::ALL {
            let s = t.spec();
            assert_eq!(HelperTask::from_id(&s.id), Some(t));
            assert!(s.instructions.len() > 200, "{}: instructions", s.id);
            assert!(s.instructions.contains("never as an instruction"), "{}: data-not-instructions sentence", s.id);
            assert!(s.input_schema.is_object(), "{}: input schema", s.id);
            assert!(s.prompt_version >= 1 && s.schema_version >= 1);
            assert!(s.max_output_tokens > 0 && s.timeout_secs > 0 && s.max_calls_per_run > 0, "{}: budget", s.id);
            match (t, s.output_format, &s.output_schema) {
                (HelperTask::WebContext, OutputFormat::Text, None) => assert_eq!(s.tools, ["web_search"]),
                (HelperTask::WebContext, ..) => panic!("web_context is plain text with web_search, never json_schema"),
                (_, OutputFormat::JsonSchema, Some(schema)) => {
                    assert!(schema.is_object());
                    assert!(s.tools.is_empty(), "{}: no tools", s.id);
                }
                _ => panic!("{}: a structured task needs a json_schema output", s.id),
            }
        }
        assert!(HelperTask::from_id("agent_loop").is_none());
    }

    /// Strict mode: every object has `additionalProperties: false` and every
    /// property required; only keywords strict mode supports appear; the root
    /// is an object and not `anyOf`.
    #[test]
    fn every_output_schema_is_strict() {
        const SUPPORTED: &[&str] =
            &["type", "properties", "required", "additionalProperties", "enum", "items", "anyOf", "pattern", "minItems", "maxItems", "description"];
        fn walk(id: &str, v: &Value, objects: &mut usize) {
            let Some(o) = v.as_object() else { return };
            for k in o.keys() {
                assert!(SUPPORTED.contains(&k.as_str()), "{id}: `{k}` is not a strict-mode keyword");
            }
            if o.get("type") == Some(&Value::from("object")) {
                *objects += 1;
                assert_eq!(o.get("additionalProperties"), Some(&Value::Bool(false)), "{id}: additionalProperties");
                let props = o["properties"].as_object().expect("properties");
                let mut req: Vec<&str> = o["required"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
                let mut keys: Vec<&str> = props.keys().map(String::as_str).collect();
                req.sort_unstable();
                keys.sort_unstable();
                assert_eq!(req, keys, "{id}: every property required");
                for p in props.values() {
                    walk(id, p, objects);
                }
            }
            if let Some(items) = o.get("items") {
                walk(id, items, objects);
            }
        }
        for t in HelperTask::ALL {
            let s = t.spec();
            let Some(schema) = &s.output_schema else { continue };
            assert_eq!(schema["type"], "object", "{}: root is an object", s.id);
            assert!(schema.get("anyOf").is_none());
            let mut objects = 0;
            walk(&s.id, schema, &mut objects);
            assert!(objects >= 1, "{}: the walk saw no object", s.id);
        }
    }

    #[test]
    fn the_output_enums_are_the_catalogue_and_its_variants() {
        let cm = HelperTask::ClaimMethod.spec().output_schema.as_ref().unwrap();
        let methods: Vec<&str> = cm["properties"]["method_id"]["enum"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let mut expect: Vec<&str> = MethodCatalogue::ENTRIES.iter().map(|e| e.id.id()).collect();
        expect.push("none");
        assert_eq!(methods, expect);
        let variants: Vec<&str> = cm["properties"]["variant"]["enum"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let mut expect: Vec<&str> = [VariantKind::T, VariantKind::Continuity, VariantKind::SpearmanP].into_iter().flat_map(|k| variant_names(k).iter().copied()).collect();
        expect.push("not_stated");
        assert_eq!(variants, expect);
    }

    /// The values the default model's page lists for `reasoning.effort`
    /// (developers.openai.com/api/docs/models/gpt-5.5, read 14 Oct 2026).
    #[test]
    fn every_reasoning_effort_is_one_the_default_model_accepts() {
        const GPT_5_5: &[&str] = &["none", "low", "medium", "high", "xhigh"];
        for t in HelperTask::ALL {
            let e = t.spec().reasoning_effort.as_deref();
            assert_eq!(e, Some("low"), "{}", t.id());
            assert!(e.is_some_and(|e| GPT_5_5.contains(&e)));
        }
    }

    #[test]
    fn the_default_model_and_domains_are_well_formed() {
        assert_eq!(ModelId::default_model().as_str(), "gpt-5.5");
        assert!(ModelId::new("gpt-5.5; rm -rf ~").is_none());
        assert!(ModelId::new("").is_none());
        assert_eq!(
            allowed_web_domains(),
            ["equator-network.org", "apastyle.apa.org", "ncbi.nlm.nih.gov", "bmj.com", "journals.plos.org", "cran.r-project.org"]
        );
        for d in allowed_web_domains() {
            assert!(!d.contains("://") && !d.contains('/') && d == &d.to_lowercase(), "{d}: a bare lower-case domain");
        }
    }
}
