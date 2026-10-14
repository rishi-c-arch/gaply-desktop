//! **The transport seam: a trait with no HTTP in it, and a mock that records
//! every body.**
//!
//! Step 6 implements [`HelperTransport`] in the app crate over the Responses
//! API with the user's own key (T-BYOK), in the `ProxyClient` pattern. Here
//! there is no client, no key and no network. A transport is handed an
//! [`OutgoingRequest`], which only the session can make, and only after the
//! send gate has passed: there is no way to call a transport with a body that
//! was not previewed and granted.

use std::collections::VecDeque;
use std::sync::Mutex;

use super::spec::HelperTask;

/// A request that passed the gate. Private fields: only the session makes
/// one.
#[derive(Debug)]
pub struct OutgoingRequest<'a> {
    task: HelperTask,
    body: &'a [u8],
    timeout_secs: u32,
}

impl<'a> OutgoingRequest<'a> {
    pub(crate) fn new(task: HelperTask, body: &'a [u8], timeout_secs: u32) -> OutgoingRequest<'a> {
        OutgoingRequest { task, body, timeout_secs }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportReply {
    pub body: Vec<u8>,
    /// The provider's request id, for diagnostics.
    pub request_id: Option<String>,
    /// Measured by the transport (the core has no clock).
    pub latency_ms: u64,
}

/// Why nothing came back. Closed codes with numeric parameters only: never a
/// provider message, a header or a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    NoCredential,
    Offline,
    Timeout,
    Http { status: u16 },
    Failed,
}

pub trait HelperTransport: Send + Sync {
    fn send(&self, request: &OutgoingRequest<'_>) -> Result<TransportReply, TransportError>;
}

/// A transport that never touches the network: it records every body it is
/// handed and returns queued replies in order (a `Failed` error once the
/// queue is empty).
#[derive(Debug, Default)]
pub struct MockTransport {
    replies: Mutex<VecDeque<Result<TransportReply, TransportError>>>,
    sent: Mutex<Vec<(HelperTask, Vec<u8>)>>,
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
        self.sent.lock().unwrap().push((request.task(), request.body().to_vec()));
        self.replies.lock().unwrap().pop_front().unwrap_or(Err(TransportError::Failed))
    }
}
