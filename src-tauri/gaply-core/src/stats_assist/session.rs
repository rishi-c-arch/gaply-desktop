//! **One run's helper state: previews, grants, budgets, and the send gate.**
//!
//! In memory only, never persisted. The flow is the prompt's:
//!
//! 1. [`HelperSession::preview`] builds the exact body for an outbound struct
//!    and returns it with its SHA-256 and its categories in words, under a new
//!    preview id. Changing a tick means building a new outbound, so a new
//!    preview.
//! 2. The user clicks Send on that preview: [`HelperSession::send_clicked`]
//!    makes (or widens) the [`ConsentGrant`] for that task and those
//!    categories, then sends.
//! 3. Every send, clicked or not, passes [`HelperSession::authorize`]: the AI
//!    help setting allows the task, a grant for this run and task covers the
//!    preview's categories, the body about to go out has the preview's hash,
//!    and the task's call budget is not spent.
//!
//! A grant lasts for the run unless revoked; turning AI help off revokes
//! every grant.

use std::collections::BTreeMap;

use super::consent::{AiHelpSetting, CategorySet, ConsentGrant, HelperRunId};
use super::outbound::{request_body, Outbound};
use super::response::{read_response, read_usage, validate_explanation, ModelText, Rejection};
use super::spec::{HelperTask, ModelId};
use super::transport::{HelperTransport, OutgoingRequest, TransportError, TransportReply};
use crate::stats_l1::dataset::sha256_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PreviewId(pub u32);

/// What the user sees before anything is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub id: PreviewId,
    pub task: HelperTask,
    /// The exact request body.
    pub body: String,
    pub sha256: String,
    pub categories: Vec<&'static str>,
}

struct StoredPreview {
    task: HelperTask,
    body: String,
    sha256: String,
    categories: CategorySet,
    outbound: Outbound,
}

/// Why a preview or a send was refused. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendRefusal {
    AiHelpOff,
    /// AI help is on, the web context switch is off.
    WebContextOff,
    UnknownPreview,
    /// No grant for this task in this run (never given, or revoked).
    NoGrant,
    /// The grant does not cover every category this preview carries.
    GrantDoesNotCover,
    /// The body about to go out is not the previewed one.
    BodyChanged,
    BudgetSpent { max: u32 },
    Transport(TransportError),
}

/// What a helper diagnostic records, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperDiagnostic {
    pub task: &'static str,
    pub prompt_version: u32,
    pub schema_version: u32,
    pub model: String,
    pub request_id: Option<String>,
    pub latency_ms: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub status: &'static str,
}

/// A request sent and its reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub task: HelperTask,
    pub preview: PreviewId,
    pub reply: TransportReply,
    pub diagnostic: HelperDiagnostic,
}

/// A send that passed the gate, with its call already counted. Only
/// [`HelperSession::prepare_send`] makes one; its bytes are the previewed
/// bytes. Not `Clone`: one prepared send is one call.
pub struct PreparedSend {
    task: HelperTask,
    preview: PreviewId,
    body: Vec<u8>,
    model: String,
}

impl std::fmt::Debug for PreparedSend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the body.
        f.debug_struct("PreparedSend").field("task", &self.task).field("preview", &self.preview).finish()
    }
}

impl PreparedSend {
    pub fn task(&self) -> HelperTask {
        self.task
    }

    /// Hand the previewed bytes to `transport`, and record the diagnostic.
    pub fn transmit(self, transport: &dyn HelperTransport) -> Result<Sent, SendRefusal> {
        let task = self.task;
        let spec = task.spec();
        let result = transport.send(&OutgoingRequest::new(task, &self.body, spec.timeout_secs));
        let reply = result.as_ref().ok();
        let status = match &result {
            Ok(_) => "ok",
            Err(TransportError::Cancelled) => "cancelled",
            Err(_) => "transport_error",
        };
        let usage = reply.map(|r| read_usage(&r.body)).unwrap_or_default();
        let diagnostic = HelperDiagnostic {
            task: task.id(),
            prompt_version: spec.prompt_version,
            schema_version: spec.schema_version,
            model: self.model,
            request_id: reply.and_then(|r| r.request_id.clone()),
            latency_ms: reply.map_or(0, |r| r.latency_ms),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            status,
        };
        tracing::info!(
            task = diagnostic.task,
            prompt_version = diagnostic.prompt_version,
            schema_version = diagnostic.schema_version,
            model = %diagnostic.model,
            request_id = ?diagnostic.request_id,
            latency_ms = diagnostic.latency_ms,
            input_tokens = ?diagnostic.input_tokens,
            output_tokens = ?diagnostic.output_tokens,
            status = diagnostic.status,
            "stats_assist: helper call"
        );
        result.map(|reply| Sent { task, preview: self.preview, reply, diagnostic }).map_err(SendRefusal::Transport)
    }
}

pub struct HelperSession {
    run: HelperRunId,
    setting: AiHelpSetting,
    model: ModelId,
    previews: BTreeMap<PreviewId, StoredPreview>,
    grants: BTreeMap<HelperTask, ConsentGrant>,
    calls: BTreeMap<HelperTask, u32>,
    next: u32,
}

impl std::fmt::Debug for HelperSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Counts only: a preview body is never printed.
        f.debug_struct("HelperSession")
            .field("run", &self.run)
            .field("setting", &self.setting)
            .field("previews", &self.previews.len())
            .field("grants", &self.grants.len())
            .finish()
    }
}

impl HelperSession {
    pub fn new(run: HelperRunId, setting: AiHelpSetting, model: ModelId) -> HelperSession {
        HelperSession { run, setting, model, previews: BTreeMap::new(), grants: BTreeMap::new(), calls: BTreeMap::new(), next: 1 }
    }

    pub fn run(&self) -> HelperRunId {
        self.run
    }

    pub fn setting(&self) -> AiHelpSetting {
        self.setting
    }

    /// Change the setting. Turning AI help off revokes every grant; turning
    /// web context off revokes its grant.
    pub fn set_setting(&mut self, setting: AiHelpSetting) {
        self.setting = setting;
        self.grants.retain(|task, _| setting.allows(*task));
    }

    pub fn set_model(&mut self, model: ModelId) {
        self.model = model;
    }

    fn allowed(&self, task: HelperTask) -> Result<(), SendRefusal> {
        if !self.setting.ai_help {
            return Err(SendRefusal::AiHelpOff);
        }
        if !self.setting.allows(task) {
            return Err(SendRefusal::WebContextOff);
        }
        Ok(())
    }

    /// Build the exact request for `outbound`, under a new preview id.
    pub fn preview(&mut self, outbound: Outbound) -> Result<Preview, SendRefusal> {
        let task = outbound.task();
        self.allowed(task)?;
        let body = request_body(&outbound, &self.model);
        let sha256 = sha256_hex(body.as_bytes());
        let categories = outbound.categories();
        let id = PreviewId(self.next);
        self.next += 1;
        let preview = Preview { id, task, body: body.clone(), sha256: sha256.clone(), categories: categories.words() };
        self.previews.insert(id, StoredPreview { task, body, sha256, categories, outbound });
        tracing::debug!(task = task.id(), preview = id.0, "stats_assist: preview built");
        Ok(preview)
    }

    /// The outbound a preview was built from, for validating its reply.
    pub fn outbound(&self, id: PreviewId) -> Option<&Outbound> {
        self.previews.get(&id).map(|p| &p.outbound)
    }

    pub fn grant(&self, task: HelperTask) -> Option<&ConsentGrant> {
        self.grants.get(&task)
    }

    /// The user clicked Send on preview `id`: grant its task and categories
    /// for the run.
    pub fn grant_from_preview(&mut self, id: PreviewId) -> Result<(), SendRefusal> {
        let p = self.previews.get(&id).ok_or(SendRefusal::UnknownPreview)?;
        self.allowed(p.task)?;
        let (task, cats) = (p.task, p.categories.clone());
        match self.grants.get_mut(&task) {
            Some(g) => g.widen(&cats),
            None => {
                self.grants.insert(task, ConsentGrant::new(self.run, task, cats));
            }
        }
        tracing::debug!(task = task.id(), "stats_assist: consent granted");
        Ok(())
    }

    /// Withdraw consent for `task`. True if there was a grant.
    pub fn revoke(&mut self, task: HelperTask) -> bool {
        self.grants.remove(&task).is_some()
    }

    /// The send gate, for the bytes about to go out.
    pub fn authorize(&self, id: PreviewId, body: &[u8]) -> Result<HelperTask, SendRefusal> {
        let p = self.previews.get(&id).ok_or(SendRefusal::UnknownPreview)?;
        self.allowed(p.task)?;
        let grant = self.grants.get(&p.task).ok_or(SendRefusal::NoGrant)?;
        if !grant.covers(self.run, p.task, &p.categories) {
            return Err(SendRefusal::GrantDoesNotCover);
        }
        if sha256_hex(body) != p.sha256 {
            return Err(SendRefusal::BodyChanged);
        }
        let max = p.task.spec().max_calls_per_run;
        if self.calls.get(&p.task).copied().unwrap_or(0) >= max {
            return Err(SendRefusal::BudgetSpent { max });
        }
        Ok(p.task)
    }

    /// Send preview `id`'s stored body.
    pub fn send(&mut self, id: PreviewId, transport: &dyn HelperTransport) -> Result<Sent, SendRefusal> {
        let body = self.previews.get(&id).ok_or(SendRefusal::UnknownPreview)?.body.clone();
        self.send_bytes(id, body.as_bytes(), transport)
    }

    /// Send `body`, which must be preview `id`'s body byte for byte.
    pub fn send_bytes(&mut self, id: PreviewId, body: &[u8], transport: &dyn HelperTransport) -> Result<Sent, SendRefusal> {
        self.prepare_send_bytes(id, body)?.transmit(transport)
    }

    /// The gate for preview `id`'s stored body, and the call counted: what a
    /// caller holding this session behind a lock takes OUT of the lock, so a
    /// slow network call never blocks the run's other commands. Every check
    /// [`Self::send`] makes is made here.
    pub fn prepare_send(&mut self, id: PreviewId) -> Result<PreparedSend, SendRefusal> {
        let body = self.previews.get(&id).ok_or(SendRefusal::UnknownPreview)?.body.clone();
        self.prepare_send_bytes(id, body.as_bytes())
    }

    fn prepare_send_bytes(&mut self, id: PreviewId, body: &[u8]) -> Result<PreparedSend, SendRefusal> {
        let task = match self.authorize(id, body) {
            Ok(t) => t,
            Err(e) => {
                tracing::info!(task = self.previews.get(&id).map(|p| p.task.id()), status = ?e, "stats_assist: send refused");
                return Err(e);
            }
        };
        *self.calls.entry(task).or_insert(0) += 1;
        Ok(PreparedSend { task, preview: id, body: body.to_vec(), model: self.model.as_str().to_string() })
    }
    /// The user clicked Send: grant, then send.
    pub fn send_clicked(&mut self, id: PreviewId, transport: &dyn HelperTransport) -> Result<Sent, SendRefusal> {
        self.grant_from_preview(id)?;
        self.send(id, transport)
    }

    /// `result_explanation` end to end, for a preview already granted:
    /// send, validate, retry once on a rejected reply, then give up with a
    /// closed reason. The numbers on screen never come from here.
    pub fn explain(&mut self, id: PreviewId, transport: &dyn HelperTransport) -> ExplanationOutcome {
        let Some(Outbound::ResultExplanation(sent)) = self.outbound(id).cloned() else {
            return ExplanationOutcome::NotShown(ExplanationGap::NotAnExplanationPreview);
        };
        let mut last = ExplanationGap::Rejected(Rejection::Empty);
        for _attempt in 0..2 {
            match self.send(id, transport) {
                Err(e) => return ExplanationOutcome::NotShown(ExplanationGap::Refused(e)),
                Ok(s) => match read_response(&s.reply.body).and_then(|o| validate_explanation(&o, &sent)) {
                    Ok(text) => return ExplanationOutcome::Shown(text),
                    Err(r) => {
                        tracing::info!(task = "result_explanation", rejection = ?r, "stats_assist: reply rejected");
                        last = ExplanationGap::Rejected(r);
                    }
                },
            }
        }
        ExplanationOutcome::NotShown(last)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplanationOutcome {
    Shown(ModelText),
    NotShown(ExplanationGap),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplanationGap {
    NotAnExplanationPreview,
    Refused(SendRefusal),
    /// Both attempts were rejected; the last rejection.
    Rejected(Rejection),
}
