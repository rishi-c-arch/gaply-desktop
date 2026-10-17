//! **The transport seam: a trait with no HTTP in it, and a mock that records
//! every body.**
//!
//! Step 6 implements [`HelperTransport`] in the app crate over the Responses
//! API with the user's own key (T-BYOK), in the `ProxyClient` pattern. Here
//! there is no client, no key and no network. A transport is handed an
//! [`OutgoingRequest`], which only the session can make, and only after the
//! send gate has passed: there is no way to call a transport with a body that
//! was not previewed and granted.
//!
//! **The wire gate (evaluation fixes, part B).** A request also carries the
//! caller's gate, which [`OutgoingRequest::pass_gate`] runs. The session runs
//! it before handing the request over, and a transport runs it again
//! immediately before EVERY attempt it puts on the wire, a retry included. A
//! failed gate sends nothing: [`TransportError::Gated`] with a closed reason.

use std::collections::VecDeque;
use std::sync::Mutex;

use super::spec::HelperTask;

/// Why the wire gate stopped an attempt. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireGate {
    AiHelpOff,
    WebContextOff,
    /// No grant for this task in this run (never given, or revoked).
    NoGrant,
    GrantDoesNotCover,
    /// The bytes are not the previewed ones.
    BodyChanged,
    BudgetSpent { max: u32 },
    Cancelled,
    /// The run is no longer open.
    RunClosed,
}

/// The check run immediately before each attempt goes out.
pub type Gate<'a> = &'a (dyn Fn() -> Result<(), WireGate> + Sync);

/// A request that passed the gate. Private fields: only the session makes
/// one.
pub struct OutgoingRequest<'a> {
    task: HelperTask,
    body: &'a [u8],
    timeout_secs: u32,
    gate: Option<Gate<'a>>,
}

impl std::fmt::Debug for OutgoingRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the body.
        f.debug_struct("OutgoingRequest").field("task", &self.task).field("bytes", &self.body.len()).finish()
    }
}

impl<'a> OutgoingRequest<'a> {
    pub(crate) fn new(task: HelperTask, body: &'a [u8], timeout_secs: u32) -> OutgoingRequest<'a> {
        OutgoingRequest { task, body, timeout_secs, gate: None }
    }
    pub(crate) fn with_gate(task: HelperTask, body: &'a [u8], timeout_secs: u32, gate: Gate<'a>) -> OutgoingRequest<'a> {
        OutgoingRequest { task, body, timeout_secs, gate: Some(gate) }
    }
    /// The caller's wire gate. A transport calls this immediately before
    /// every attempt, and puts nothing on the wire when it fails.
    pub fn pass_gate(&self) -> Result<(), TransportError> {
        match self.gate {
            Some(g) => g().map_err(TransportError::Gated),
            None => Ok(()),
        }
    }
    pub fn task(&self) -> HelperTask {
        self.task
    }
    /// The exact bytes previewed.
    pub fn body(&self) -> &'a [u8] {
        self.body
    }
    pub fn timeout_secs(&self) -> u32 {
        self.timeout_secs
    }
}

/// What came back. The body is the provider's response, untrusted data.
#[derive(Clone, PartialEq, Eq)]
pub struct TransportReply {
    pub body: Vec<u8>,
    /// The provider's request id, for diagnostics.
    pub request_id: Option<String>,
    /// Measured by the transport (the core has no clock).
    pub latency_ms: u64,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for TransportReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransportReply").field("bytes", &self.body.len()).field("request_id", &self.request_id).field("latency_ms", &self.latency_ms).finish_non_exhaustive()
    }
}

/// Why nothing came back. Closed codes with numeric parameters only: never a
/// provider message, a header or a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    NoCredential,
    Offline,
    Timeout,
    Http { status: u16 },
    /// The run cancelled the call; any reply is discarded.
    Cancelled,
    /// The server answered with a redirect (3xx). Redirects are never
    /// followed and never retried: nothing goes to the Location.
    Redirect { status: u16 },
    /// The wire gate stopped the attempt before anything was sent.
    Gated(WireGate),
    Failed,
}

pub trait HelperTransport: Send + Sync {
    fn send(&self, request: &OutgoingRequest<'_>) -> Result<TransportReply, TransportError>;

    /// How many requests this transport has put on the wire so far: every
    /// attempt, a retry and a failed one included. No default: a transport
    /// must count what it sends, so the usage line can show it.
    fn requests_sent(&self) -> u32;
}

/// A transport that never touches the network: it records every body it is
/// handed and returns queued replies in order (a `Failed` error once the
/// queue is empty).
#[derive(Default)]
pub struct MockTransport {
    replies: Mutex<VecDeque<Result<TransportReply, TransportError>>>,
    sent: Mutex<Vec<(HelperTask, Vec<u8>)>>,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockTransport").field("replies", &self.replies.lock().map(|q| q.len()).ok()).field("sent", &self.sent.lock().map(|v| v.len()).ok()).finish_non_exhaustive()
    }
}

impl MockTransport {
    pub fn new() -> MockTransport {
        MockTransport::default()
    }

    /// Queue a reply whose body is `body`.
    pub fn reply(self, body: &str) -> MockTransport {
        let n = self.replies.lock().unwrap().len() + 1;
        self.replies.lock().unwrap().push_back(Ok(TransportReply {
            body: body.as_bytes().to_vec(),
            request_id: Some(format!("req_mock_{n}")),
            latency_ms: 5,
        }));
        self
    }

    pub fn fail(self, e: TransportError) -> MockTransport {
        self.replies.lock().unwrap().push_back(Err(e));
        self
    }

    /// Every body handed to this transport, in order.
    pub fn sent(&self) -> Vec<(HelperTask, Vec<u8>)> {
        self.sent.lock().unwrap().clone()
    }

    pub fn sent_count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }
}

impl HelperTransport for MockTransport {
    fn send(&self, request: &OutgoingRequest<'_>) -> Result<TransportReply, TransportError> {
        request.pass_gate()?;
        self.sent.lock().unwrap().push((request.task(), request.body().to_vec()));
        self.replies.lock().unwrap().pop_front().unwrap_or(Err(TransportError::Failed))
    }

    fn requests_sent(&self) -> u32 {
        self.sent_count() as u32
    }
}
