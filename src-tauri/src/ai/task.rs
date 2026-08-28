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

use crate::ai::generative::StopReason;
use crate::ai::model_manager::{ModelManager, TokenSink};

/// One evidence chunk as it was handed to the model.
///
/// The header format is identical everywhere:
/// `[CHUNK_ID=c1 PAGE=8 SECTION=Results] text…`
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
    /// Render one line. SPEC OVERRIDE — §11 D26.
    ///
    /// `[CHUNK_ID=c1 PAGE=8 SECTION=Results] text` — a missing page renders
    /// `PAGE=?` and a missing section `SECTION=-`, so the three-field shape
    /// never varies and the model never has to parse two different layouts.
    ///
    /// THE KEYS ARE LOAD-BEARING. The spec's `[c1 | p.8 | Results]` marked the
    /// id by POSITION alone, and a 3B model was measured echoing the entire
    /// bracket back as its `chunk_id` — `"c13 | p.5 | -"` — on four of six
    /// seeds under both prompt variants. Reading a composite display string as
    /// one opaque identifier is a defensible reading of that format; being
    /// first is not a label. `CHUNK_ID=` gives the id a name to be copied by.
    ///
    /// This is presentation only. The validator still requires the BARE id and
    /// a composite remains fatal (`require_known_chunk`) — accepting the
    /// display form as an alias would destroy the D18 grounding guarantee,
    /// which is the one thing the identifier check actually buys.
    pub fn render(&self) -> String {
        let page = self.page.map(|p| p.to_string()).unwrap_or_else(|| "?".to_string());
        let section = self.section.clone().unwrap_or_else(|| "-".to_string());
        format!("[CHUNK_ID={} PAGE={} SECTION={}] {}", self.chunk_id, page, section, self.text)
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
        Err(ValidationError::fatal(
            field.to_string(),
            format!(
                "references chunk_id {chunk_id:?}, which was not in the evidence provided \
                 (given: {})",
                if self.chunks.is_empty() {
                    "none".to_string()
                } else {
                    self.chunk_ids().join(", ")
                }
            ),
        ))
    }

    /// The `<evidence>` block, in the spec's format.
    pub fn render_evidence(&self) -> String {
        let body =
            self.chunks.values().map(|c| c.render()).collect::<Vec<_>>().join("\n");
        format!("<evidence>\n{body}\n</evidence>")
    }
}

/// How much a deviation costs (plan §9.11).
///
/// The distinction is NOT how confident the validator is — it is what a wrong
/// answer does to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    /// Grounding and safety: an invented chunk id, a page that does not match,
    /// a reference-shaped query, a schema violation, two fields that contradict
    /// each other. These make an output actively MISLEADING, so they earn a
    /// retry and then a hard failure.
    Fatal,
    /// Style and length. These make an output UNTIDY. The output is accepted
    /// and the deviation travels with it — discarding a correct classification
    /// because its rationale ran to 27 words was the engine preferring nothing
    /// over something slightly long.
    Advisory,
}

/// A concrete, quotable reason an output was rejected or flagged. Concrete
/// because the retry prompt names these back to the model — "invalid output"
/// teaches it nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    pub field: String,
    pub problem: String,
    pub tier: Tier,
}

impl ValidationError {
    /// Grounding/safety: retry, then fail.
    pub fn fatal(field: impl Into<String>, problem: impl Into<String>) -> Self {
        Self { field: field.into(), problem: problem.into(), tier: Tier::Fatal }
    }
    /// Style/length: accepted, reported, never silently dropped.
    pub fn advisory(field: impl Into<String>, problem: impl Into<String>) -> Self {
        Self { field: field.into(), problem: problem.into(), tier: Tier::Advisory }
    }
    pub fn is_fatal(&self) -> bool {
        self.tier == Tier::Fatal
    }
}

/// Split a validator's output into the two tiers.
fn partition(errors: Vec<ValidationError>) -> (Vec<ValidationError>, Vec<ValidationError>) {
    errors.into_iter().partition(|e| e.is_fatal())
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.problem)
    }
}

/// Which generation an error refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Attempt {
    First,
    Retry,
}

/// Why a task failed, with everything needed to diagnose it.
#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "camelCase")]
pub enum TaskError {
    /// Both attempts failed. Carries BOTH raw outputs — never a summary,
    /// because the raw text is the only evidence of what actually happened.
    ValidationFailed {
        /// The PRIMARY attempt's fatal errors — see `primary`.
        errors: Vec<ValidationError>,
        /// Which attempt these errors describe. When the retry came back worse
        /// (unparseable where the first parsed, or more fatal errors), the
        /// FIRST attempt is primary: the engine stops throwing away the better
        /// of two bad answers. It is still a failure; the caller decides.
        primary: Attempt,
        first_raw: String,
        retry_raw: String,
        /// Timings for BOTH attempts. A failed run costs real time, and
        /// excluding it from a latency average understates what the user waits
        /// — the failures are the SLOWEST cases, so dropping them flatters the
        /// number in exactly the wrong direction.
        timings: RunTimings,
        /// Why each attempt stopped, in attempt order (§11 D27). On THIS path
        /// it is the answer to the question a failed run always raises: did
        /// the model say something wrong, or did it simply run out of room?
        stop_reasons: Vec<StopReason>,
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
    fn prompt_version(&self) -> &'static str;
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

/// Build the corrective retry prompt.
///
/// SHAPE MATTERS, measured (plan §11 D10). The first version appended only the
/// error lines, and ENDED on them:
///
/// ```text
/// - reason: 25 words or less
/// - severity: high|medium|low
/// ```
///
/// A 0.5B continued that bullet pattern instead of returning to JSON, so a
/// retry meant to rescue a near-miss produced no JSON at all in 5 of 8 seed
/// cases. Three changes, all aimed at the same thing — the LAST thing the model
/// reads must be the output contract, not a list:
///
/// 1. the rejected attempt is quoted back, so the task is CORRECT THIS, not
///    "produce an answer again";
/// 2. the errors are numbered prose, not bullets that invite continuation;
/// 3. the suffix ends with an explicit instruction to emit only the corrected
///    JSON object, beginning with `{`.
///
/// `original_attempt` is passed verbatim, including whatever malformed text the
/// model produced — showing it its own output is the point, and sanitising it
/// would hide the very thing being corrected.
fn retry_prompt(original: &str, original_attempt: &str, errors: &[ValidationError]) -> String {
    let listed = errors
        .iter()
        .enumerate()
        .map(|(i, e)| format!("{}. {e}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{original}\n\nYour previous reply was rejected.\n\n\
         Your previous reply was:\n{original_attempt}\n\n\
         It was rejected for these reasons:\n{listed}\n\n\
         Correct the reply above so that every reason is resolved. Keep every field \
         that was already correct.\n\
         Output ONLY the corrected JSON object and nothing else. \
         Begin your reply with {{\n"
    )
}

/// Prompt size and the prefill/decode split, summed across attempts.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTimings {
    pub prompt_tokens: usize,
    pub prefill_ms: u64,
    pub decode_ms: u64,
    pub tokens: usize,
}

/// What one attempt produced.
struct Attempted<O> {
    /// `None` when the reply could not be parsed at all.
    output: Option<O>,
    fatal: Vec<ValidationError>,
    advisory: Vec<ValidationError>,
}

impl<O> Attempted<O> {
    /// Is this attempt strictly worse than `other`?
    ///
    /// Two orderings, in priority: an unparseable reply is worse than a
    /// parseable one, and among parseable replies more fatal errors is worse.
    /// Advisories deliberately do not enter the comparison — they never block
    /// acceptance, so they cannot make one attempt preferable to another.
    fn is_worse_than(&self, other: &Self) -> bool {
        match (self.output.is_some(), other.output.is_some()) {
            (false, true) => true,
            (true, false) => false,
            _ => self.fatal.len() > other.fatal.len(),
        }
    }
}

/// Parse + validate one raw reply, keeping the tiers apart.
fn check<T: AiTask>(raw: &str, ctx: &TaskContext) -> Attempted<T::Output> {
    let Some(json) = extract_json(raw) else {
        return Attempted {
            output: None,
            fatal: vec![ValidationError::fatal(
                "<output>",
                "no JSON object or array found in the reply",
            )],
            advisory: Vec::new(),
        };
    };
    let parsed: T::Output = match serde_json::from_str(json) {
        Ok(p) => p,
        Err(e) => {
            return Attempted {
                output: None,
                fatal: vec![ValidationError::fatal(
                    "<output>",
                    format!("not valid JSON: {e}"),
                )],
                advisory: Vec::new(),
            }
        }
    };
    let (fatal, advisory) = match T::validate(&parsed, ctx) {
        Ok(()) => (Vec::new(), Vec::new()),
        Err(errors) => partition(errors),
    };
    Attempted { output: Some(parsed), fatal, advisory }
}

#[derive(Debug, Serialize)]
pub struct TaskRun<T> {
    pub output: T,
    /// Style/length deviations the output was ACCEPTED with. Never dropped:
    /// these are returned to the caller and belong in `provenance_json` on
    /// persistence, so a stored row can always say how it deviated.
    pub advisories: Vec<ValidationError>,
    pub timings: RunTimings,
    /// True when the first attempt failed and the retry succeeded. Surfaced
    /// rather than hidden — a task that usually needs a retry is a prompt that
    /// needs fixing, and that is only visible if it is reported.
    pub retried: bool,
    pub prompt_version: &'static str,
    pub model_id: String,
    pub tokens: usize,
    pub elapsed_ms: u64,
    /// Why each generation stopped, in attempt order — one entry for a
    /// first-attempt success, two when the retry ran (§11 D27).
    ///
    /// `StopReason` has always existed on `GenOutput` and has always been
    /// discarded here. A reply cut off at `max_tokens` is unparseable, and
    /// unparseable scored identically to wrong, so the single most common
    /// failure on the small models was invisible in every report. Carrying it
    /// makes truncation a COUNTED category rather than something rediscovered
    /// by reading raw text.
    pub stop_reasons: Vec<StopReason>,
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

    let a1 = check::<T>(&first.text, ctx);
    let t1 = RunTimings {
        prompt_tokens: first.prompt_tokens,
        prefill_ms: first.prefill_ms,
        decode_ms: first.decode_ms,
        tokens: first.tokens,
    };

    // ADVISORY-ONLY IS ACCEPTED. No retry: a rationale two words long or a
    // four-word query is untidy, not misleading, and the retry has been
    // measured to destroy otherwise-good answers (plan §9.11, D10).
    if a1.fatal.is_empty() {
        if let Some(output) = a1.output {
            if !a1.advisory.is_empty() {
                tracing::debug!(advisories = ?a1.advisory, "accepted with advisories");
            }
            return Ok(TaskRun {
                output,
                advisories: a1.advisory,
                retried: false,
                prompt_version: task.prompt_version(),
                model_id: manager.model_id(),
                tokens: first.tokens,
                elapsed_ms: first.elapsed_ms,
                timings: t1,
                stop_reasons: vec![first.stop_reason],
            });
        }
    }

    tracing::debug!(errors = ?a1.fatal, "fatal validation errors; retrying once");

    // Only the FATAL errors go into the corrective prompt. Asking the model to
    // fix advisories it was already forgiven would spend a whole generation on
    // tidiness and risk losing the answer.
    let second = manager
        .generate(
            retry_prompt(&prompt, &first.text, &a1.fatal),
            T::max_tokens(),
            cancel,
            on_token,
        )
        .await
        .map_err(|e| match e {
            GaplyError::Cancelled => TaskError::Cancelled,
            other => TaskError::Generation { message: other.to_string() },
        })?;

    let a2 = check::<T>(&second.text, ctx);
    let combined = RunTimings {
        prompt_tokens: t1.prompt_tokens + second.prompt_tokens,
        prefill_ms: t1.prefill_ms + second.prefill_ms,
        decode_ms: t1.decode_ms + second.decode_ms,
        tokens: t1.tokens + second.tokens,
    };

    if a2.fatal.is_empty() {
        if let Some(output) = a2.output {
            return Ok(TaskRun {
                output,
                advisories: a2.advisory,
                retried: true,
                prompt_version: task.prompt_version(),
                model_id: manager.model_id(),
                tokens: combined.tokens,
                elapsed_ms: first.elapsed_ms + second.elapsed_ms,
                timings: combined,
                stop_reasons: vec![first.stop_reason, second.stop_reason],
            });
        }
    }

    // KEEP-BETTER-ATTEMPT. Both failed; report whichever is less bad as the
    // primary artifact, with ITS errors. Nothing is repaired and this is still
    // an error — the engine just stops discarding the better of two bad answers.
    let retry_is_worse = a2.is_worse_than(&a1);
    let (primary, errors) = if retry_is_worse {
        (Attempt::First, a1.fatal)
    } else {
        (Attempt::Retry, a2.fatal)
    };
    Err(TaskError::ValidationFailed {
        errors,
        primary,
        first_raw: first.text,
        retry_raw: second.text,
        timings: combined,
        stop_reasons: vec![first.stop_reason, second.stop_reason],
    })
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

    fn prompt_version(&self) -> &'static str {
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
            errors.push(ValidationError::fatal("echo", "must not be empty"));
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
    fn evidence_renders_in_the_labelled_format() {
        let c = ctx();
        let rendered = c.render_evidence();
        assert!(rendered.starts_with("<evidence>\n"), "{rendered}");
        assert!(rendered.ends_with("\n</evidence>"), "{rendered}");
        // §11 D26's labelled header, replacing the spec's `[c1 | p.8 | Results]`
        assert!(
            rendered.contains("[CHUNK_ID=c1 PAGE=8 SECTION=Results] Organic management"),
            "header format drifted: {rendered}"
        );
        // an unpaginated chunk keeps the three-field shape rather than dropping a field
        assert!(rendered.contains("[CHUNK_ID=c2 PAGE=? SECTION=-] No significant"), "{rendered}");
    }

    /// GOLDEN — §11 D26. The whole point of the rendering change: a reader
    /// scanning for the id finds `CHUNK_ID=c1` and nothing else that looks like
    /// an identifier. The old format made `c1 | p.8 | Results` look like one
    /// composite id, and a 3B model copied exactly that back.
    #[test]
    fn the_rendered_header_makes_the_bare_chunk_id_unambiguous() {
        let c = ctx();
        let rendered = c.render_evidence();

        // 1. the id appears exactly once per chunk, immediately after its key
        assert!(rendered.contains("CHUNK_ID=c1 "), "{rendered}");
        assert!(rendered.contains("CHUNK_ID=c2 "), "{rendered}");

        // 2. the composite string the 3B actually emitted cannot be read off
        //    the rendering any more — the pipe-separated display form is gone
        assert!(!rendered.contains(" | "), "the composite display form is back: {rendered}");
        assert!(!rendered.contains("p.8"), "the bare `p.N` form is back: {rendered}");

        // 3. every id in the rendering is exactly the id the context knows, so
        //    copying what follows `CHUNK_ID=` up to whitespace always yields a
        //    valid bare id
        for id in c.chunk_ids() {
            let key = format!("CHUNK_ID={id} ");
            assert!(rendered.contains(&key), "{id} not rendered with its key: {rendered}");
        }

        // 4. and the value between the key and the next field is the id ALONE
        for line in rendered.lines().filter(|l| l.starts_with("[CHUNK_ID=")) {
            let after = &line["[CHUNK_ID=".len()..];
            let id: String = after.chars().take_while(|c| !c.is_whitespace()).collect();
            assert!(
                c.get(&id).is_some(),
                "scraping CHUNK_ID= up to whitespace yielded {id:?}, which is not a known chunk"
            );
        }
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
            ValidationError::fatal("echo", "must not be empty"),
            ValidationError::fatal("chunk_id", "references chunk_id \"c9\""),
        ];
        let p = retry_prompt("ORIGINAL", "{\"echo\":\"\"}", &errors);
        assert!(p.starts_with("ORIGINAL"), "the retry must keep the original prompt");
        // the rejected attempt is quoted back, so the task is CORRECT THIS
        assert!(p.contains("{\"echo\":\"\"}"), "the retry must show the model its own output: {p}");
        // numbered prose, not bullets that invite continuation (§11 D10)
        assert!(p.contains("1. echo: must not be empty"), "{p}");
        assert!(p.contains("2. chunk_id: references chunk_id \"c9\""), "{p}");
        assert!(!p.contains("\n- "), "bullets invite the model to continue the list: {p}");
        // and the LAST thing it reads is the output contract
        let tail = p.trim_end();
        assert!(
            tail.ends_with("Begin your reply with {"),
            "the retry must END on the output contract, not on the error list: {tail:?}"
        );
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
        stop: StopReason,
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
            Ok(GenOutput {
                tokens: 1,
                text,
                stop_reason: self.stop,
                elapsed_ms: 1,
                prompt_tokens: 0,
                prefill_ms: 0,
                decode_ms: 0,
                decode_tokens_per_sec: 0.0,
            })
        }
    }

    struct ScriptedLoader {
        replies: Vec<String>,
        prompts: Arc<StdMutex<Vec<String>>>,
        calls: Arc<AtomicUsize>,
        stop: StopReason,
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
                model_max_context: 1,
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
                stop: self.stop,
            }))
        }
    }

    fn scripted(
        replies: &[&str],
    ) -> (ModelManager, Arc<StdMutex<Vec<String>>>, Arc<AtomicUsize>) {
        scripted_stopping(replies, StopReason::EndOfTurn)
    }

    /// Same, with the stop reason the backend reports — the only way to
    /// exercise the truncation path without a real model (§11 D27).
    fn scripted_stopping(
        replies: &[&str],
        stop: StopReason,
    ) -> (ModelManager, Arc<StdMutex<Vec<String>>>, Arc<AtomicUsize>) {
        let prompts = Arc::new(StdMutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let m = ModelManager::new(Arc::new(ScriptedLoader {
            replies: replies.iter().map(|s| s.to_string()).collect(),
            prompts: prompts.clone(),
            calls: calls.clone(),
            stop,
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

    /// §11 D27. Truncation must stay a FAILURE and must say so. Raising the
    /// ceiling is not a repair: a reply cut off at `max_tokens` is unparseable,
    /// and the report has to be able to distinguish "ran out of room" from
    /// "said something wrong" — those have different fixes.
    #[tokio::test]
    async fn a_truncated_generation_is_still_a_failure_and_reports_the_ceiling_as_the_cause() {
        let cut = r#"{"echo":"hello","chunk_id":"c"#; // cut off mid-string
        let (m, _p, calls) = scripted_stopping(&[cut, cut], StopReason::MaxTokens);
        let (t, c) = echo_task();
        let err = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        match err {
            TaskError::ValidationFailed { stop_reasons, .. } => assert_eq!(
                stop_reasons,
                vec![StopReason::MaxTokens, StopReason::MaxTokens],
                "the ceiling was hit twice and the report could not say so"
            ),
            other => panic!("a truncated reply must FAIL, got {other:?}"),
        }
    }

    /// The other half of D27: a run that happened to finish inside the ceiling
    /// still carries its stop reason, so "was this cell ceiling-bound?" is
    /// answerable from the report rather than by re-running.
    #[tokio::test]
    async fn an_accepted_run_still_carries_why_generation_stopped() {
        let (m, _p, _c) = scripted(&[r#"{"echo":"hello","chunk_id":"c1"}"#]);
        let (t, c) = echo_task();
        let run = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert_eq!(run.stop_reasons, vec![StopReason::EndOfTurn]);
    }

    #[tokio::test]
    async fn bad_json_twice_is_a_typed_failure_carrying_both_raw_outputs() {
        let (m, _p, calls) = scripted(&["garbage one", "garbage two"]);
        let (t, c) = echo_task();
        let err = run_task(&m, &t, &c, Arc::new(AtomicBool::new(false)), None).await.unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "exactly ONE retry, no more");
        match err {
            TaskError::ValidationFailed { errors, first_raw, retry_raw, .. } => {
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

    /* ---------------- two-tier validation (plan §9.11) -------------------- */

    /// A task whose validator emits whichever tiers the test asks for. The
    /// flags live in TIER_FLAGS rather than on the struct because `validate` is
    /// an associated function with no `self` to read them from.
    struct TieredTask;

    #[derive(Debug, serde::Deserialize, Serialize)]
    struct TieredOut {
        ok: bool,
    }

    impl AiTask for TieredTask {
        type Output = TieredOut;
        fn prompt_version(&self) -> &'static str {
            "tiered-v1"
        }
        fn max_tokens() -> usize {
            32
        }
        fn build_prompt(&self) -> String {
            "P".into()
        }
        fn validate(_out: &TieredOut, _ctx: &TaskContext) -> Result<(), Vec<ValidationError>> {
            // Reads the flags off a thread-local set by each test, so one task
            // type can exercise every tier combination.
            TIER_FLAGS.with(|f| {
                let (fatal, advisory) = *f.borrow();
                let mut errs = Vec::new();
                if fatal {
                    errs.push(ValidationError::fatal("f", "a grounding problem"));
                }
                if advisory {
                    errs.push(ValidationError::advisory("a", "is 27 words; the limit is 25"));
                }
                if errs.is_empty() {
                    Ok(())
                } else {
                    Err(errs)
                }
            })
        }
    }

    thread_local! {
        static TIER_FLAGS: std::cell::RefCell<(bool, bool)> =
            const { std::cell::RefCell::new((false, false)) };
    }

    fn tiered(fatal: bool, advisory: bool) -> TieredTask {
        TIER_FLAGS.with(|f| *f.borrow_mut() = (fatal, advisory));
        TieredTask
    }

    #[tokio::test]
    async fn advisory_only_output_is_accepted_with_the_advisories_attached() {
        let (m, _p, calls) = scripted(&[r#"{"ok":true}"#]);
        let t = tiered(false, true);
        let run = run_task(&m, &t, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None)
            .await
            .expect("advisory-only must be ACCEPTED, not retried");
        assert!(!run.retried, "an advisory triggered a retry");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "an advisory cost a second generation");
        assert_eq!(run.advisories.len(), 1, "the advisory was dropped: {:?}", run.advisories);
        assert_eq!(run.advisories[0].tier, Tier::Advisory);
        assert!(run.advisories[0].problem.contains("27 words"));
        // NOT a silent repair: the output is passed through untouched.
        assert!(run.output.ok);
    }

    #[tokio::test]
    async fn a_clean_output_carries_no_advisories() {
        let (m, _p, _c) = scripted(&[r#"{"ok":true}"#]);
        let t = tiered(false, false);
        let run = run_task(&m, &t, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None)
            .await
            .unwrap();
        assert!(run.advisories.is_empty());
    }

    #[tokio::test]
    async fn a_fatal_error_still_retries_then_fails() {
        let (m, _p, calls) = scripted(&[r#"{"ok":true}"#, r#"{"ok":true}"#]);
        let t = tiered(true, false);
        let err = run_task(&m, &t, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None)
            .await
            .unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "a fatal error must still earn exactly one retry");
        assert!(matches!(err, TaskError::ValidationFailed { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn a_mixed_output_treats_the_fatal_as_fatal() {
        // An advisory must never soften a grounding failure.
        let (m, _p, calls) = scripted(&[r#"{"ok":true}"#, r#"{"ok":true}"#]);
        let t = tiered(true, true);
        let err = run_task(&m, &t, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None)
            .await
            .unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "the fatal half did not trigger the retry");
        match err {
            TaskError::ValidationFailed { errors, .. } => {
                assert!(errors.iter().all(|e| e.is_fatal()), "advisories leaked into the failure");
                assert_eq!(errors.len(), 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn keep_better_attempt_reports_attempt_one_when_the_retry_is_unparseable() {
        // Attempt 1 parses but has a fatal error; the retry is garbage. The
        // FIRST attempt is the better artifact and must be the one reported.
        let (m, _p, _c) = scripted(&[r#"{"ok":true}"#, "not json at all"]);
        let t = tiered(true, false);
        let err = run_task(&m, &t, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None)
            .await
            .unwrap_err();
        match err {
            TaskError::ValidationFailed { primary, errors, first_raw, retry_raw, .. } => {
                assert_eq!(primary, Attempt::First, "the worse retry was reported as primary");
                // ITS errors, not the retry's parse error.
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].field, "f");
                assert!(!errors.iter().any(|e| e.problem.contains("no JSON")));
                // both raws are still carried
                assert_eq!(first_raw, r#"{"ok":true}"#);
                assert_eq!(retry_raw, "not json at all");
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn keep_better_attempt_prefers_the_retry_when_it_is_not_worse() {
        // Both attempts fail identically; there is no reason to prefer the
        // stale one, so the retry stays primary.
        let (m, _p, _c) = scripted(&["not json", "also not json"]);
        let t = tiered(true, false);
        let err = run_task(&m, &t, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None)
            .await
            .unwrap_err();
        match err {
            TaskError::ValidationFailed { primary, .. } => assert_eq!(primary, Attempt::Retry),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn a_retry_that_fixes_the_fatal_error_succeeds_and_reports_retried() {
        let (m, _p, calls) = scripted(&["not json", r#"{"ok":true}"#]);
        let t = tiered(false, false);
        let run = run_task(&m, &t, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None)
            .await
            .unwrap();
        assert!(run.retried);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
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
        assert!(p.contains("[CHUNK_ID=c1 PAGE=8 SECTION=Results]"), "evidence rendering drifted");
        assert_eq!(t.prompt_version(), "echo-v1");
    }
}
