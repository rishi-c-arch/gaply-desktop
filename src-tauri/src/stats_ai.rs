//! **The AI side of Level 1, T-BYOK: the user's own OpenAI key, the
//! Responses API transport, and the "AI help" setting.** (Phase 2, Step 6.)
//!
//! - [`CredentialStore`]: where the key lives. [`OsCredentialStore`] is the OS
//!   keychain through `gaply_core::secrets` (service `ai.gaply.app`, entry
//!   [`OPENAI_KEY_ENTRY`]); [`MemoryCredentialStore`] is what every test uses.
//!   The key goes from the store into the `Authorization` header and nowhere
//!   else: it is never returned by a command, never logged, never written
//!   anywhere but the OS store. [`ApiKey`] has no `Serialize` and a redacted
//!   `Debug`.
//! - [`ResponsesTransport`]: `gaply_core::stats_assist::transport::HelperTransport`
//!   over blocking reqwest (the client the app crate already uses), `POST
//!   {base}/responses`. Per-task timeout from the spec; one retry with backoff
//!   on a timeout, a 429 or a 5xx only; a per-run cancel flag checked before
//!   each attempt, during the backoff and after the reply. A blocking request
//!   already in flight cannot be aborted: it ends at its timeout, and its
//!   reply is discarded as `Cancelled`.
//! - [`AiSettings`]: AI help and web context, both off by default, and the
//!   model id. The app's other local settings live in the webview's
//!   `localStorage`, which Rust cannot enforce, so this is a small JSON file in
//!   the app data directory beside the store plugin's files, read and written
//!   only by Rust.
//!
//! Nothing here sees a path, a cell, a header or manuscript text: the
//! transport is handed the previewed bytes and nothing else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gaply_core::stats_assist::consent::AiHelpSetting;
use gaply_core::stats_assist::spec::{HelperTask, ModelId};
use gaply_core::stats_assist::transport::{HelperTransport, OutgoingRequest, TransportError, TransportReply};

/// The keychain entry for the user's OpenAI key, under service `ai.gaply.app`.
pub const OPENAI_KEY_ENTRY: &str = "openai_api_key";
/// The provider name a key status reports.
pub const PROVIDER: &str = "openai";
/// The Responses API base. Tests point a transport at a local capture server
/// instead; nothing reads this from the environment.
pub const OPENAI_BASE: &str = "https://api.openai.com/v1";
/// The backoff before the one retry.
pub const RETRY_BACKOFF: Duration = Duration::from_secs(2);
/// The key test's timeout.
pub const KEY_TEST_TIMEOUT: Duration = Duration::from_secs(20);
/// The longest key the store accepts, in characters.
pub const MAX_KEY_CHARS: usize = 512;

/* ------------------------------------------------------------------------ *
 *  The key
 * ------------------------------------------------------------------------ */

/// The user's key. No `Serialize`, no `Display`; `Debug` prints no character
/// of it.
pub struct ApiKey(String);

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey(…)")
    }
}

impl ApiKey {
    fn bearer(&self) -> Result<reqwest::header::HeaderValue, TransportError> {
        let mut v = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.0)).map_err(|_| TransportError::NoCredential)?;
        v.set_sensitive(true);
        Ok(v)
    }
}

/// Why the store refused. Closed; never the store's own message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    /// Empty, too long, or holding a space or a control character.
    Malformed,
    /// The OS store could not be reached.
    Unavailable,
}

impl CredentialError {
    pub fn code(self) -> &'static str {
        match self {
            CredentialError::Malformed => "key_malformed",
            CredentialError::Unavailable => "key_store_unavailable",
        }
    }
}

/// A key as typed, checked: trimmed, 1 to 512 visible ASCII characters. No
/// prefix is required (a test key does not start with `sk-`).
pub fn checked_key(typed: &str) -> Result<String, CredentialError> {
    let k = typed.trim();
    let ok = !k.is_empty() && k.chars().count() <= MAX_KEY_CHARS && k.chars().all(|c| c.is_ascii_graphic());
    ok.then(|| k.to_string()).ok_or(CredentialError::Malformed)
}

pub trait CredentialStore: Send + Sync {
    fn has_key(&self) -> Result<bool, CredentialError>;
    /// The key, for the `Authorization` header only.
    fn load_key(&self) -> Result<Option<ApiKey>, CredentialError>;
    fn save_key(&self, typed: &str) -> Result<(), CredentialError>;
    fn remove_key(&self) -> Result<(), CredentialError>;
}

/// The OS keychain, through `gaply_core::secrets` (keyring 3.6.3).
pub struct OsCredentialStore;

impl CredentialStore for OsCredentialStore {
    fn has_key(&self) -> Result<bool, CredentialError> {
        gaply_core::secrets::has_secret(OPENAI_KEY_ENTRY).map_err(|_| CredentialError::Unavailable)
    }
    fn load_key(&self) -> Result<Option<ApiKey>, CredentialError> {
        match gaply_core::secrets::get_secret(OPENAI_KEY_ENTRY) {
            Ok(k) => Ok(Some(ApiKey(k))),
            Err(gaply_core::GaplyError::NotFound { .. }) => Ok(None),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }
    fn save_key(&self, typed: &str) -> Result<(), CredentialError> {
        let k = checked_key(typed)?;
        gaply_core::secrets::store_secret(OPENAI_KEY_ENTRY, &k).map_err(|_| CredentialError::Unavailable)
    }
    fn remove_key(&self) -> Result<(), CredentialError> {
        match gaply_core::secrets::delete_secret(OPENAI_KEY_ENTRY) {
            Ok(()) | Err(gaply_core::GaplyError::NotFound { .. }) => Ok(()),
            Err(_) => Err(CredentialError::Unavailable),
        }
    }
}

/// An in-memory store: every test uses this, never the OS keychain.
#[derive(Default)]
pub struct MemoryCredentialStore(Mutex<Option<String>>);

impl CredentialStore for MemoryCredentialStore {
    fn has_key(&self) -> Result<bool, CredentialError> {
        Ok(self.0.lock().unwrap().is_some())
    }
    fn load_key(&self) -> Result<Option<ApiKey>, CredentialError> {
        Ok(self.0.lock().unwrap().clone().map(ApiKey))
    }
    fn save_key(&self, typed: &str) -> Result<(), CredentialError> {
        *self.0.lock().unwrap() = Some(checked_key(typed)?);
        Ok(())
    }
    fn remove_key(&self) -> Result<(), CredentialError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

/* ------------------------------------------------------------------------ *
 *  The transport
 * ------------------------------------------------------------------------ */

/// Each attempt's timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestTimeout {
    /// The task's own, from the spec (`timeout_secs`): what ships.
    FromSpec,
    /// A fixed value, for a test that needs a stall to end quickly.
    Fixed(Duration),
}

pub struct ResponsesTransport {
    client: reqwest::blocking::Client,
    base: String,
    store: Arc<dyn CredentialStore>,
    cancel: Arc<AtomicBool>,
    backoff: Duration,
    timeout: RequestTimeout,
    /// Requests put on the wire, for the usage line.
    requests: AtomicU32,
}

impl ResponsesTransport {
    pub fn new(
        base: &str,
        store: Arc<dyn CredentialStore>,
        cancel: Arc<AtomicBool>,
        backoff: Duration,
        timeout: RequestTimeout,
    ) -> Result<Self, TransportError> {
        // No global timeout: each request carries its own. Redirects are never
        // followed (evaluation fixes, part B): a 307/308 would re-send the
        // previewed body to another host.
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| TransportError::Failed)?;
        Ok(ResponsesTransport { client, base: base.trim_end_matches('/').to_string(), store, cancel, backoff, timeout, requests: AtomicU32::new(0) })
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Wait `d`, in short slices, giving up early on a cancel.
    fn backoff(&self) -> Result<(), TransportError> {
        let until = Instant::now() + self.backoff;
        while Instant::now() < until {
            if self.cancelled() {
                return Err(TransportError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(50).min(until.saturating_duration_since(Instant::now())));
        }
        Ok(())
    }
}

enum Attempt {
    Done(TransportReply),
    Retryable(TransportError),
    Final(TransportError),
}

impl HelperTransport for ResponsesTransport {
    fn send(&self, request: &OutgoingRequest<'_>) -> Result<TransportReply, TransportError> {
        let key = self.store.load_key().map_err(|_| TransportError::NoCredential)?.ok_or(TransportError::NoCredential)?;
        let bearer = key.bearer()?;
        let url = format!("{}/responses", self.base);
        let started = Instant::now();
        let mut last = TransportError::Failed;
        for attempt in 0..2 {
            if attempt == 1 {
                self.backoff()?;
            }
            // The wire gate, immediately before every attempt: AI help, the
            // grant, the hash, the budget and cancel, read where the commands
            // write them. A failed gate sends nothing.
            request.pass_gate()?;
            if self.cancelled() {
                return Err(TransportError::Cancelled);
            }
            let sent = self
                .client
                .post(&url)
                .header(reqwest::header::AUTHORIZATION, bearer.clone())
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .timeout(match self.timeout {
                    RequestTimeout::FromSpec => Duration::from_secs(u64::from(request.timeout_secs())),
                    RequestTimeout::Fixed(d) => d,
                })
                .body(request.body().to_vec())
                .send();
            // Counted unless the connection itself failed: then no byte of
            // the body left the device.
            if !matches!(&sent, Err(e) if e.is_connect()) {
                self.requests.fetch_add(1, Ordering::SeqCst);
            }
            let outcome = match sent {
                Err(e) if e.is_timeout() => Attempt::Retryable(TransportError::Timeout),
                Err(e) if e.is_connect() => Attempt::Final(TransportError::Offline),
                Err(_) => Attempt::Final(TransportError::Failed),
                Ok(resp) => {
                    let status = resp.status();
                    let request_id = resp.headers().get("x-request-id").and_then(|v| v.to_str().ok()).map(str::to_string);
                    if status.is_success() {
                        match resp.bytes() {
                            Ok(b) => Attempt::Done(TransportReply {
                                body: b.to_vec(),
                                request_id,
                                latency_ms: started.elapsed().as_millis() as u64,
                            }),
                            Err(e) if e.is_timeout() => Attempt::Retryable(TransportError::Timeout),
                            Err(_) => Attempt::Final(TransportError::Failed),
                        }
                    } else if status.is_redirection() {
                        // Never followed, never retried: nothing goes to the
                        // Location.
                        Attempt::Final(TransportError::Redirect { status: status.as_u16() })
                    } else if status.as_u16() == 429 || status.is_server_error() {
                        Attempt::Retryable(TransportError::Http { status: status.as_u16() })
                    } else {
                        Attempt::Final(TransportError::Http { status: status.as_u16() })
                    }
                }
            };
            match outcome {
                Attempt::Done(reply) => {
                    // A reply that arrives after a cancel is discarded.
                    return if self.cancelled() { Err(TransportError::Cancelled) } else { Ok(reply) };
                }
                Attempt::Final(e) => return Err(e),
                Attempt::Retryable(e) => {
                    tracing::info!(task = request.task().id(), attempt, error = ?e, "stats_ai: retryable failure");
                    last = e;
                }
            }
        }
        Err(last)
    }

    fn requests_sent(&self) -> u32 {
        self.requests.load(Ordering::SeqCst)
    }
}

/// What the key test found. Closed; each answer its own (evaluation fixes,
/// part B): "accepted" only for a 2xx from the base host with a model list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyTest {
    Accepted,
    /// AI help is off: nothing was sent.
    AiHelpOff,
    NoKey,
    /// 401.
    Rejected,
    /// 403.
    Forbidden,
    /// Any other 4xx (429 included).
    ClientError,
    /// 5xx.
    ServerError,
    /// A 3xx: never followed.
    RedirectRefused,
    /// A 2xx whose body is not a model list.
    UnexpectedReply,
    Offline,
    Timeout,
    Failed,
}

impl KeyTest {
    pub fn code(self) -> String {
        match self {
            KeyTest::Accepted => "accepted",
            KeyTest::AiHelpOff => "ai_help_off",
            KeyTest::NoKey => "no_key",
            KeyTest::Rejected => "rejected",
            KeyTest::Forbidden => "forbidden",
            KeyTest::ClientError => "client_error",
            KeyTest::ServerError => "server_error",
            KeyTest::RedirectRefused => "redirect_refused",
            KeyTest::UnexpectedReply => "unexpected_reply",
            KeyTest::Offline => "offline",
            KeyTest::Timeout => "timeout",
            KeyTest::Failed => "failed",
        }
        .into()
    }
}

/// A `GET /models` body: a JSON object with a `data` array.
fn is_model_list(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body).ok().is_some_and(|v| v.get("data").is_some_and(serde_json::Value::is_array))
}

/// Test the saved key with a request that carries no user data: `GET
/// {base}/models`, redirects off. The caller refuses it with AI help off.
pub fn test_key(base: &str, store: &dyn CredentialStore) -> KeyTest {
    let key = match store.load_key() {
        Ok(Some(k)) => k,
        Ok(None) => return KeyTest::NoKey,
        Err(_) => return KeyTest::Failed,
    };
    let Ok(bearer) = key.bearer() else { return KeyTest::Failed };
    let Ok(client) = reqwest::blocking::Client::builder().timeout(KEY_TEST_TIMEOUT).redirect(reqwest::redirect::Policy::none()).build() else {
        return KeyTest::Failed;
    };
    match client.get(format!("{}/models", base.trim_end_matches('/'))).header(reqwest::header::AUTHORIZATION, bearer).send() {
        Ok(r) => {
            let status = r.status();
            if status.is_success() {
                match r.bytes() {
                    Ok(b) if is_model_list(&b) => KeyTest::Accepted,
                    Ok(_) => KeyTest::UnexpectedReply,
                    Err(e) if e.is_timeout() => KeyTest::Timeout,
                    Err(_) => KeyTest::Failed,
                }
            } else if status.is_redirection() {
                KeyTest::RedirectRefused
            } else if status.as_u16() == 401 {
                KeyTest::Rejected
            } else if status.as_u16() == 403 {
                KeyTest::Forbidden
            } else if status.is_client_error() {
                KeyTest::ClientError
            } else if status.is_server_error() {
                KeyTest::ServerError
            } else {
                KeyTest::UnexpectedReply
            }
        }
        Err(e) if e.is_timeout() => KeyTest::Timeout,
        Err(e) if e.is_connect() => KeyTest::Offline,
        Err(_) => KeyTest::Failed,
    }
}

/* ------------------------------------------------------------------------ *
 *  The setting
 * ------------------------------------------------------------------------ */

/// What the setting file holds. No secret.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub ai_help: bool,
    pub web_context: bool,
    pub model: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        AiSettings { ai_help: false, web_context: false, model: ModelId::default_model().as_str().to_string() }
    }
}

impl AiSettings {
    pub fn help(&self) -> AiHelpSetting {
        AiHelpSetting { ai_help: self.ai_help, web_context: self.web_context }
    }
    /// The model, or the default when the stored id is not well formed.
    pub fn model_id(&self) -> ModelId {
        ModelId::new(&self.model).unwrap_or_else(ModelId::default_model)
    }
}

pub const SETTINGS_FILE: &str = "stats_ai_settings.json";

/// The stored setting, or the default (everything off) when the file is
/// missing or unreadable: a damaged file never turns AI help on.
pub fn load_settings(path: &Path) -> AiSettings {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_settings(path: &Path, s: &AiSettings) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s).expect("settings serialise"))?;
    std::fs::rename(&tmp, path)
}

/* ------------------------------------------------------------------------ *
 *  Usage
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUsage {
    /// Requests that left the device: each attempt, retries and failed calls.
    pub requests: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub type UsageByTask = BTreeMap<HelperTask, TaskUsage>;

/* ------------------------------------------------------------------------ *
 *  The context the commands use
 * ------------------------------------------------------------------------ */

/// Makes a transport for one send, bound to its run's cancel flag.
pub type TransportFactory = Arc<dyn Fn(Arc<AtomicBool>) -> Arc<dyn HelperTransport> + Send + Sync>;

pub struct StatsAi {
    settings_path: Option<PathBuf>,
    settings: Mutex<AiSettings>,
    pub credentials: Arc<dyn CredentialStore>,
    /// The key test's base.
    pub base: String,
    pub transport: TransportFactory,
}

impl StatsAi {
    /// The shipped context: the OS keychain, the Responses API, the setting
    /// file in `app_data_dir`.
    pub fn production(app_data_dir: &Path) -> StatsAi {
        let path = app_data_dir.join(SETTINGS_FILE);
        let store: Arc<dyn CredentialStore> = Arc::new(OsCredentialStore);
        let for_transport = store.clone();
        let transport: TransportFactory = Arc::new(move |cancel| {
            match ResponsesTransport::new(OPENAI_BASE, for_transport.clone(), cancel, RETRY_BACKOFF, RequestTimeout::FromSpec) {
                Ok(t) => Arc::new(t) as Arc<dyn HelperTransport>,
                Err(_) => Arc::new(Unavailable) as Arc<dyn HelperTransport>,
            }
        });
        StatsAi {
            settings: Mutex::new(load_settings(&path)),
            settings_path: Some(path),
            credentials: store,
            base: OPENAI_BASE.to_string(),
            transport,
        }
    }

    /// A context for tests: an in-memory setting, the given store and
    /// transport, and the given key-test base.
    pub fn for_tests(store: Arc<dyn CredentialStore>, transport: TransportFactory, base: &str) -> StatsAi {
        StatsAi { settings_path: None, settings: Mutex::new(AiSettings::default()), credentials: store, base: base.to_string(), transport }
    }

    pub fn settings(&self) -> AiSettings {
        self.settings.lock().unwrap().clone()
    }

    pub fn set_settings(&self, s: AiSettings) -> Result<AiSettings, &'static str> {
        let model = ModelId::new(&s.model).ok_or("model_id_malformed")?;
        let s = AiSettings { model: model.as_str().to_string(), ..s };
        if let Some(path) = &self.settings_path {
            save_settings(path, &s).map_err(|_| "settings_not_saved")?;
        }
        *self.settings.lock().unwrap() = s.clone();
        tracing::info!(ai_help = s.ai_help, web_context = s.web_context, "stats_ai: setting changed");
        Ok(s)
    }
}

/// A transport that could not be built: every send fails, closed.
struct Unavailable;

impl HelperTransport for Unavailable {
    fn send(&self, _: &OutgoingRequest<'_>) -> Result<TransportReply, TransportError> {
        Err(TransportError::Failed)
    }

    fn requests_sent(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_checked_and_never_printed() {
        assert_eq!(checked_key("  KEY_SECRET_a1b2 \n"), Ok("KEY_SECRET_a1b2".into()));
        assert_eq!(checked_key(""), Err(CredentialError::Malformed));
        assert_eq!(checked_key("two words"), Err(CredentialError::Malformed));
        assert_eq!(checked_key("bad\u{7}"), Err(CredentialError::Malformed));
        assert_eq!(checked_key(&"k".repeat(513)), Err(CredentialError::Malformed));
        assert_eq!(format!("{:?}", ApiKey("KEY_SECRET_a1b2".into())), "ApiKey(…)");
        let h = ApiKey("KEY_SECRET_a1b2".into()).bearer().unwrap();
        assert!(h.is_sensitive());
        assert!(!format!("{h:?}").contains("KEY_SECRET"));
    }

    #[test]
    fn a_damaged_setting_file_never_turns_ai_help_on() {
        let dir = std::env::temp_dir().join(format!("gaply-stats-ai-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(SETTINGS_FILE);
        std::fs::write(&p, b"{\"aiHelp\": tru").unwrap();
        assert_eq!(load_settings(&p), AiSettings::default());
        assert!(!load_settings(&dir.join("absent.json")).ai_help);
        let s = AiSettings { ai_help: true, web_context: false, model: "gpt-5.5".into() };
        save_settings(&p, &s).unwrap();
        assert_eq!(load_settings(&p), s);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
