//! Validated generation — what replaces GBNF grammars under Candle.
//!
//! §9.1 resolved the runtime to Candle, which has no grammar-constrained
//! decoding, and the spec's ARCHITECTURE OVERRIDE note records that the
//! replacement is **strict output validation plus one retry**. This module is
//! that replacement.
//!
//! ```text
//! build_prompt ──► generate ──► strip preamble ──► serde parse ──► validate
//!                                                       │             │
//!                                              ANY failure ──────────┘
//!                                                       ▼
//!                                     ONE retry, prompt + concrete errors
//!                                                       ▼
//!                                     still failing → TaskError::ValidationFailed
//!                                                      (carries BOTH raw outputs)
//! ```
//!
//! # Never a silently repaired result
//!
//! The harness does not patch, coerce, or fill a model's output. It re-asks once
//! with the specific errors named, and if that fails it returns a typed error
//! carrying both raw strings so a human can see exactly what the model said. A
//! repaired result would be indistinguishable from a correct one, which is the
//! failure mode this whole project is organised against.
//!
//! # The chunk-id rule is enforced generically
//!
//! The spec's core evidence rule — *"the model must echo it back, never invent
//! it"* — is not left to each task. [`TaskContext`] carries the chunk ids that
//! were actually put in the `<evidence>` block, and
//! [`TaskContext::require_known_chunk`] is available to every validator. A task
//! that references a chunk it was never given fails validation.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use gaply_core::GaplyError;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::ai::model_manager::{ModelManager, TokenSink};

/// One evidence chunk as it was handed to the model.
///
/// The header format is the spec's, identical everywhere:
/// `[c1 | p.8 | Results] text…`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceChunk {
    /// The id the model must echo back. Never invented.
    pub chunk_id: String,
    /// Recorded page, or `None` for a source with no pagination.
    pub page: Option<u32>,
    pub section: Option<String>,
    pub text: String,
}

impl EvidenceChunk {
    /// Render one line exactly as the spec specifies.
    ///
    /// `[c1 | p.8 | Results] text` — a missing page renders `p.?` and a missing
    /// section renders `-`, so the three-field shape never varies and the model
    /// never has to parse two different layouts.
    pub fn render(&self) -> String {
        let page = self.page.map(|p| format!("p.{p}")).unwrap_or_else(|| "p.?".to_string());
        let section = self.section.clone().unwrap_or_else(|| "-".to_string());
        format!("[{} | {} | {}] {}", self.chunk_id, page, section, self.text)
    }
}

/// What was actually sent to the model, so validators can check against it.
#[derive(Debug, Clone, Default)]
pub struct TaskContext {
    chunks: BTreeMap<String, EvidenceChunk>,
}

impl TaskContext {
    pub fn new(chunks: Vec<EvidenceChunk>) -> Self {
        Self { chunks: chunks.into_iter().map(|c| (c.chunk_id.clone(), c)).collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn chunk_ids(&self) -> Vec<&str> {
        self.chunks.keys().map(|s| s.as_str()).collect()
    }

    pub fn get(&self, chunk_id: &str) -> Option<&EvidenceChunk> {
        self.chunks.get(chunk_id)
    }

    /// THE spec's core rule, enforced generically.
    ///
    /// A chunk id the model produced but was never given is an invented
    /// citation — the exact failure the id-echo rule exists to prevent — so it
    /// is a validation error naming both the offender and what was available.
    pub fn require_known_chunk(&self, chunk_id: &str, field: &str) -> Result<(), ValidationError> {
        if self.chunks.contains_key(chunk_id) {
            return Ok(());
        }
        Err(ValidationError {
            field: field.to_string(),
            problem: format!(
                "references chunk_id {chunk_id:?}, which was not in the evidence provided \
                 (given: {})",
                if self.chunks.is_empty() {
                    "none".to_string()
                } else {
                    self.chunk_ids().join(", ")
                }
            ),
        })
    }

    /// The `<evidence>` block, in the spec's format.
    pub fn render_evidence(&self) -> String {
        let body =
            self.chunks.values().map(|c| c.render()).collect::<Vec<_>>().join("\n");
        format!("<evidence>\n{body}\n</evidence>")
    }
}

/// A concrete, quotable reason an output was rejected. Concrete because the
/// retry prompt names these back to the model — "invalid output" teaches it
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    pub field: String,
    pub problem: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.problem)
    }
}

/// Why a task failed, with everything needed to diagnose it.
#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "camelCase")]
pub enum TaskError {
    /// Both attempts failed. Carries BOTH raw outputs — never a summary,
    /// because the raw text is the only evidence of what actually happened.
    ValidationFailed {
        errors: Vec<ValidationError>,
        first_raw: String,
        retry_raw: String,
    },
    Generation { message: String },
    Cancelled,
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::ValidationFailed { errors, .. } => write!(
                f,
                "the model's output failed validation twice: {}",
                errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
            ),
            TaskError::Generation { message } => write!(f, "generation failed: {message}"),
            TaskError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl From<TaskError> for GaplyError {
    fn from(e: TaskError) -> Self {
        match e {
            TaskError::Cancelled => GaplyError::Cancelled,
            other => GaplyError::Validation(other.to_string()),
        }
    }
}

/// One SLM task: how to ask, and what counts as an acceptable answer.
pub trait AiTask {
    type Output: DeserializeOwned + Serialize;

    /// Versioned so every stored row can say which prompt produced it.
    fn prompt_version() -> &'static str;
    fn build_prompt(&self) -> String;
    fn max_tokens() -> usize;
    /// Every reason the output is unacceptable, not just the first — the retry
    /// names all of them, so one round trip can fix several problems.
    fn validate(out: &Self::Output, ctx: &TaskContext) -> Result<(), Vec<ValidationError>>;
}

/// Extract the JSON object/array from a model reply.
///
/// The spec says "raw JSON only, no markdown, no code fences" — but that is an
/// instruction to the model, not a guarantee, so the harness strips what models
/// actually emit: fenced blocks and prose either side. It NEVER edits the JSON
/// itself; if what is inside the braces is malformed, that is a validation
/// failure and gets a retry, not a repair.
pub fn extract_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    // ```json … ``` or ``` … ```
    let unfenced = if let Some(rest) = trimmed.strip_prefix("```") {
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        rest.rsplit_once("```").map(|(inner, _)| inner).unwrap_or(rest)
    } else {
        trimmed
    };
    let s = unfenced.trim();
    // First balanced-looking span from the first opener to its last closer.
    let start = s.find(['{', '['])?;
    let opener = s.as_bytes()[start];
    let closer = if opener == b'{' { '}' } else { ']' };
    let end = s.rfind(closer)?;
    if end <= start {
        return None;
    }
    Some(&s[start..=end])
}

const RETRY_PREFIX: &str =
    "\n\nYour previous reply was rejected. Fix EXACTLY these problems and output \
     raw JSON only — no prose, no code fences:\n";

/// Build the corrective retry prompt: the original, plus the concrete errors.
fn retry_prompt(original: &str, errors: &[ValidationError]) -> String {
    let listed =
        errors.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n");
    format!("{original}{RETRY_PREFIX}{listed}\n")
}

/// Parse + validate one raw reply.
fn check<T: AiTask>(raw: &str, ctx: &TaskContext) -> Result<T::Output, Vec<ValidationError>> {
    let Some(json) = extract_json(raw) else {
        return Err(vec![ValidationError {
            field: "<output>".into(),
            problem: "no JSON object or array found in the reply".into(),
        }]);
    };
    let parsed: T::Output = serde_json::from_str(json).map_err(|e| {
        vec![ValidationError { field: "<output>".into(), problem: format!("not valid JSON: {e}") }]
    })?;
    T::validate(&parsed, ctx)?;
    Ok(parsed)
}

#[derive(Debug, Serialize)]
pub struct TaskRun<T> {
    pub output: T,
    /// True when the first attempt failed and the retry succeeded. Surfaced
    /// rather than hidden — a task that usually needs a retry is a prompt that
    /// needs fixing, and that is only visible if it is reported.
    pub retried: bool,
    pub prompt_version: &'static str,
    pub model_id: String,
    pub tokens: usize,
    pub elapsed_ms: u64,
}

/// Generate, parse, validate; on ANY failure retry ONCE with the concrete
/// errors named; on a second failure return a typed error carrying both raw
/// outputs. Never a silently repaired result.
pub async fn run_task<T: AiTask>(
    manager: &ModelManager,
    task: &T,
    ctx: &TaskContext,
    cancel: Arc<AtomicBool>,
    on_token: Option<TokenSink>,
) -> Result<TaskRun<T::Output>, TaskError> {
    let prompt = task.build_prompt();

    let first = manager
        .generate(prompt.clone(), T::max_tokens(), cancel.clone(), on_token.clone())
        .await
        .map_err(|e| match e {
            GaplyError::Cancelled => TaskError::Cancelled,
            other => TaskError::Generation { message: other.to_string() },
        })?;

    let first_errors = match check::<T>(&first.text, ctx) {
        Ok(output) => {
            return Ok(TaskRun {
                output,
                retried: false,
                prompt_version: T::prompt_version(),
                model_id: manager.model_id(),
                tokens: first.tokens,
                elapsed_ms: first.elapsed_ms,
            })
        }
        Err(errors) => errors,
    };

    tracing::debug!(errors = ?first_errors, "task output rejected; retrying once");

    let second = manager
        .generate(
            retry_prompt(&prompt, &first_errors),
            T::max_tokens(),
            cancel,
            on_token,
        )
        .await
        .map_err(|e| match e {
            GaplyError::Cancelled => TaskError::Cancelled,
            other => TaskError::Generation { message: other.to_string() },
        })?;

    match check::<T>(&second.text, ctx) {
        Ok(output) => Ok(TaskRun {
            output,
            retried: true,
            prompt_version: T::prompt_version(),
            model_id: manager.model_id(),
            tokens: first.tokens + second.tokens,
            elapsed_ms: first.elapsed_ms + second.elapsed_ms,
        }),
        Err(errors) => Err(TaskError::ValidationFailed {
            errors,
            first_raw: first.text,
            retry_raw: second.text,
        }),
    }
}

/* ------------------------------- EchoTask -------------------------------- *
 * DEV ONLY. Proves the whole path end to end — prompt, generation, extraction,
 * parse, validation including the chunk-id rule, retry — without implementing
 * any of the spec's eight task prompts, which are a later phase. */

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EchoOutput {
    pub echo: String,
    /// Must be one of the ids in the evidence, when evidence was provided.
    pub chunk_id: Option<String>,
}

pub struct EchoTask {
    pub phrase: String,
    pub evidence: String,
}

impl AiTask for EchoTask {
    type Output = EchoOutput;

    fn prompt_version() -> &'static str {
        "echo-v1"
    }

    fn max_tokens() -> usize {
        128
    }

    fn build_prompt(&self) -> String {
        // Qwen2.5-Instruct chat template. The four spec rules are stated in the
        // system turn even for the dev task, so the harness is exercised in the
        // shape the real tasks will use.
        format!(
            "<|im_start|>system\n\
             You output raw JSON only. No markdown, no code fences, no commentary.\n\
             You may only use text inside <evidence>. You have no other knowledge.\n\
             Every field must be traceable to a chunk_id you were given.\n\
             If the evidence is insufficient, output null for chunk_id.\n\
             <|im_end|>\n\
             <|im_start|>user\n\
             {evidence}\n\
             Echo the phrase below back in this exact JSON shape:\n\
             {{\"echo\": \"<the phrase>\", \"chunk_id\": \"<an id from the evidence, or null>\"}}\n\n\
             Phrase: {phrase}\n\
             <|im_end|>\n\
             <|im_start|>assistant\n",
            evidence = self.evidence,
            phrase = self.phrase
        )
    }

    fn validate(out: &EchoOutput, ctx: &TaskContext) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if out.echo.trim().is_empty() {
            errors.push(ValidationError {
                field: "echo".into(),
                problem: "must not be empty".into(),
            });
        }
        // The generic rule: a referenced chunk must have been provided.
        if let Some(id) = &out.chunk_id {
            if let Err(e) = ctx.require_known_chunk(id, "chunk_id") {
                errors.push(e);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> TaskContext {
        TaskContext::new(vec![
            EvidenceChunk {
                chunk_id: "c1".into(),
                page: Some(8),
                section: Some("Results".into()),
                text: "Organic management significantly increased species richness.".into(),
            },
            EvidenceChunk {
                chunk_id: "c2".into(),
                page: None,
                section: None,
                text: "No significant effect was observed for soil fauna.".into(),
            },
        ])
    }

    #[test]
    fn evidence_renders_in_the_spec_format() {
        let c = ctx();
        let rendered = c.render_evidence();
        assert!(rendered.starts_with("<evidence>\n"), "{rendered}");
        assert!(rendered.ends_with("\n</evidence>"), "{rendered}");
        // exactly the spec's `[c1 | p.8 | Results] text`
        assert!(
            rendered.contains("[c1 | p.8 | Results] Organic management"),
            "header format drifted: {rendered}"
        );
        // an unpaginated chunk keeps the three-field shape rather than dropping a field
        assert!(rendered.contains("[c2 | p.? | -] No significant"), "{rendered}");
    }

    #[test]
    fn extract_json_survives_what_models_actually_emit() {
        assert_eq!(extract_json(r#"{"a":1}"#), Some(r#"{"a":1}"#));
        assert_eq!(extract_json("```json\n{\"a\":1}\n```"), Some("{\"a\":1}"));
        assert_eq!(extract_json("```\n{\"a\":1}\n```"), Some("{\"a\":1}"));
        assert_eq!(extract_json("Sure! Here you go:\n{\"a\":1}\nHope that helps."), Some("{\"a\":1}"));
        assert_eq!(extract_json("[{\"a\":1}]"), Some("[{\"a\":1}]"));
        assert_eq!(extract_json("no json here"), None);
        assert_eq!(extract_json(""), None);
        // It must NOT repair malformed JSON — that is the retry's job, and a
        // silent repair would be indistinguishable from a correct answer.
        assert_eq!(extract_json(r#"{"a":}"#), Some(r#"{"a":}"#));
    }

    #[test]
    fn an_invented_chunk_id_is_a_validation_error_naming_what_was_available() {
        let c = ctx();
        let err = c.require_known_chunk("c99", "chunk_id").unwrap_err();
        assert_eq!(err.field, "chunk_id");
        assert!(err.problem.contains("c99"), "{err}");
        assert!(err.problem.contains("c1, c2"), "must say what WAS available: {err}");
        assert!(c.require_known_chunk("c1", "chunk_id").is_ok());
    }

    #[test]
    fn require_known_chunk_with_no_evidence_says_none_rather_than_an_empty_list() {
        let empty = TaskContext::default();
        let err = empty.require_known_chunk("c1", "chunk_id").unwrap_err();
        assert!(err.problem.contains("given: none"), "{err}");
    }

    #[test]
    fn echo_validation_rejects_an_empty_echo_and_an_invented_id_together() {
        let c = ctx();
        let out = EchoOutput { echo: "  ".into(), chunk_id: Some("c42".into()) };
        let errors = EchoTask::validate(&out, &c).unwrap_err();
        assert_eq!(errors.len(), 2, "the retry must be told about BOTH problems: {errors:?}");
        assert!(errors.iter().any(|e| e.field == "echo"));
        assert!(errors.iter().any(|e| e.field == "chunk_id"));
    }

    #[test]
    fn echo_validation_accepts_a_null_chunk_id() {
        // Abstaining is correct behaviour, not failure (spec §0 rule 3).
        let out = EchoOutput { echo: "hello".into(), chunk_id: None };
        assert!(EchoTask::validate(&out, &ctx()).is_ok());
    }

    #[test]
    fn the_retry_prompt_names_the_concrete_errors() {
        let errors = vec![
            ValidationError { field: "echo".into(), problem: "must not be empty".into() },
            ValidationError { field: "chunk_id".into(), problem: "references chunk_id \"c9\"".into() },
        ];
        let p = retry_prompt("ORIGINAL", &errors);
        assert!(p.starts_with("ORIGINAL"), "the retry must keep the original prompt");
        assert!(p.contains("- echo: must not be empty"), "{p}");
        assert!(p.contains("- chunk_id: references chunk_id \"c9\""), "{p}");
        assert!(p.contains("raw JSON only"), "{p}");
    }

    /* ---- the retry path, end to end, against a scripted mock backend ---- */

    use crate::ai::generative::{GenOutput, GenRequest, GenerationBackend, RamEstimate, StopReason};
    use crate::ai::model_manager::{BackendLoader, ModelManager};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    /// Replies with a scripted sequence, and records every prompt it saw — so a
    /// test can assert the retry prompt actually carried the errors.
    struct ScriptedBackend {
        replies: StdMutex<std::collections::VecDeque<String>>,
        prompts: Arc<StdMutex<Vec<String>>>,
        calls: Arc<AtomicUsize>,
    }

    impl GenerationBackend for ScriptedBackend {
        fn generate(&self, req: GenRequest<'_>) -> Result<GenOutput, GaplyError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.prompts.lock().unwrap().push(req.prompt.clone());
            let text = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| "SCRIPT EXHAUSTED".to_string());
            Ok(GenOutput { tokens: 1, text, stop_reason: StopReason::EndOfTurn, elapsed_ms: 1 })
        }
    }

    struct ScriptedLoader {
        replies: Vec<String>,
        prompts: Arc<StdMutex<Vec<String>>>,
        calls: Arc<AtomicUsize>,
    }

    impl BackendLoader for ScriptedLoader {
        fn model_id(&self) -> String {
            "scripted".into()
        }
        fn ram_estimate(&self) -> Result<RamEstimate, GaplyError> {
            Ok(RamEstimate {
                weights_bytes: 1,
                kv_cache_bytes: 1,
                total_bytes: 2,
                context_length: 1,
                layers: 1,
                kv_heads: 1,
                head_dim: 1,
            })
        }
        fn load(&self) -> Result<Arc<dyn GenerationBackend>, GaplyError> {
            Ok(Arc::new(ScriptedBackend {
                replies: StdMutex::new(self.replies.clone().into()),
                prompts: self.prompts.clone(),
                calls: self.calls.clone(),
            }))
        }
    }

    fn scripted(
        replies: &[&str],
    ) -> (ModelManager, Arc<StdMutex<Vec<String>>>, Arc<AtomicUsize>) {
        let prompts = Arc::new(StdMutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let m = ModelManager::new(Arc::new(ScriptedLoader {
            replies: replies.iter().map(|s| s.to_string()).collect(),
            prompts: prompts.clone(),
            calls: calls.clone(),
        }));
        (m, prompts, calls)
    }

    fn echo_task() -> (EchoTask, TaskContext) {
        let c = ctx();
        (EchoTask { phrase: "hello".into(), evidence: c.render_evidence() }, c)
    }

    #[tokio::test]
    async fn valid_first_time_means_no_retry_and_one_generation() {
        let (m, _p, calls) = scripted(&[r#"{"echo":"hello","chunk_id":"c1"}"#]);
        let (t, c) = echo_task();
        let run = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert_eq!(run.output.echo, "hello");
        assert!(!run.retried);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "retried a valid answer");
        assert_eq!(run.prompt_version, "echo-v1");
        assert_eq!(run.model_id, "scripted");
    }

    #[tokio::test]
    async fn bad_json_then_good_succeeds_on_the_retry_and_says_so() {
        let (m, prompts, calls) =
            scripted(&["not json at all", r#"{"echo":"hello","chunk_id":"c2"}"#]);
        let (t, c) = echo_task();
        let run = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert!(run.retried, "a successful retry must be REPORTED, not hidden");
        assert_eq!(run.output.chunk_id.as_deref(), Some("c2"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        // The retry prompt must name the concrete problem.
        let ps = prompts.lock().unwrap();
        assert_eq!(ps.len(), 2);
        assert!(ps[1].starts_with(&ps[0]), "the retry dropped the original prompt");
        assert!(ps[1].contains("no JSON object or array found"), "retry prompt: {}", ps[1]);
    }

    #[tokio::test]
    async fn bad_json_twice_is_a_typed_failure_carrying_both_raw_outputs() {
        let (m, _p, calls) = scripted(&["garbage one", "garbage two"]);
        let (t, c) = echo_task();
        let err = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "exactly ONE retry, no more");
        match err {
            TaskError::ValidationFailed { errors, first_raw, retry_raw } => {
                assert_eq!(first_raw, "garbage one", "the first raw output was lost");
                assert_eq!(retry_raw, "garbage two", "the retry raw output was lost");
                assert!(!errors.is_empty());
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_invented_chunk_id_fails_validation_even_though_the_json_is_perfect() {
        // THE spec rule: echo the id back, never invent it. The JSON parses
        // cleanly, so only the chunk-id check can catch this.
        let invented = r#"{"echo":"hello","chunk_id":"c999"}"#;
        let (m, _p, calls) = scripted(&[invented, invented]);
        let (t, c) = echo_task();
        let err = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        match err {
            TaskError::ValidationFailed { errors, .. } => {
                assert!(
                    errors.iter().any(|e| e.field == "chunk_id" && e.problem.contains("c999")),
                    "the invented id was not the reported reason: {errors:?}"
                );
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_fenced_reply_is_accepted_without_a_retry() {
        let (m, _p, calls) =
            scripted(&["```json\n{\"echo\":\"hello\",\"chunk_id\":null}\n```"]);
        let (t, c) = echo_task();
        let run = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert!(!run.retried, "a code fence should be stripped, not retried");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(run.output.chunk_id, None);
    }

    #[tokio::test]
    async fn cancellation_is_a_typed_cancel_not_a_validation_failure() {
        let (m, _p, _calls) = scripted(&[r#"{"echo":"hello","chunk_id":null}"#]);
        let (t, c) = echo_task();
        let cancel = Arc::new(AtomicBool::new(true)); // already cancelled
        let err = run_task(&m, &t, &c, cancel, None).await.unwrap_err();
        assert!(matches!(err, TaskError::Cancelled), "got {err:?}");
    }

    #[test]
    fn the_prompt_carries_the_four_spec_rules_and_the_evidence_block() {
        let c = ctx();
        let t = EchoTask { phrase: "hello".into(), evidence: c.render_evidence() };
        let p = t.build_prompt();
        assert!(p.contains("<evidence>") && p.contains("</evidence>"));
        assert!(p.contains("only use text inside <evidence>"));
        assert!(p.contains("traceable to a chunk_id"));
        assert!(p.contains("raw JSON only"));
        assert!(p.contains("[c1 | p.8 | Results]"), "evidence not in the spec's format");
        assert_eq!(EchoTask::prompt_version(), "echo-v1");
    }
}
