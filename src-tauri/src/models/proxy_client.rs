//! Cloud verification runtime: a remote [`ProxyClient`] that calls the
//! Tailscale-hidden gaply-proxy `/verify` endpoint (App Check → rate limit →
//! structured-summary validator → Claude Sonnet).
//!
//! This is the PublishReady keystone: the first `ProxyClient` that reaches the
//! cloud. Like `OllamaVerifyClient` (local) and `MockProxyClient` (test), it
//! implements the SAME `gaply_core::verify_agent::ProxyClient` trait, so
//! `verify_citations` and every caller are unchanged — `verify_proxy()` simply
//! prefers this tier when the proxy is reachable and App Check is provisioned.
//!
//! # Trust boundary discipline
//!
//! - App Check: each request carries a FRESH limited-use token minted by
//!   `gaply_core::app_check::proxy_auth_header` (pure crypto in the core; the
//!   signing secret lives ONLY in the OS keychain). Limited-use tokens are
//!   single-use — the proxy's replay guard consumes each `jti` — so a token is
//!   never reused across requests.
//! - The payload is the same structured evidence bundle the mock/Ollama tiers
//!   send (short structured fields, no raw manuscript text), which the proxy's
//!   validator requires (else 422).
//! - The Claude API key never touches the client; it lives only on the proxy.
//! - Plain HTTP to a loopback/tailnet address; reqwest is pinned to rustls, so
//!   no OpenSSL enters the graph even when the tailnet URL is HTTPS.
//!
//! # Sync + threading
//!
//! `verify` is `&self` and synchronous (blocking reqwest); callers use
//! `spawn_blocking`, same as the other tiers.

use std::time::Duration;

use serde_json::Value;

use gaply_core::app_check::{proxy_auth_header, TokenSigner};
use gaply_core::verify_agent::ProxyClient;
use gaply_core::GaplyError;

/// Default proxy endpoint. Overridden by `GAPLY_PROXY_URL` (tailnet MagicDNS
/// name in production; loopback for dev). Loopback default so an unset env
/// never points off-machine by surprise.
const DEFAULT_PROXY_URL: &str = "http://127.0.0.1:8080";

/// Health-probe timeout — short, so an undeployed proxy fails fast and
/// `verify_proxy()` falls through to the local Ollama tier.
const HEALTH_TIMEOUT_SECS: u64 = 2;

/// Inference-call timeout. Cloud reasoning can be slow (adaptive thinking), so
/// a generous ceiling — still bounded.
const REQUEST_TIMEOUT_SECS: u64 = 180;

/// Header carrying the signed-in user's Supabase JWT (Set 8). The USER'S OWN
/// credential, never an app secret: the proxy verifies it server-side to run
/// THE REAL entitlement gate before paid work. Optional until the proxy is
/// deployed with enforcement on.
const USER_TOKEN_HEADER: &str = "X-Gaply-User-Token";

/// A remote [`ProxyClient`] backed by the gaply-proxy `/verify` endpoint.
pub struct ProxyReqwestClient {
    base_url: String,
    signer: TokenSigner,
    client: reqwest::blocking::Client,
    /// Signed-in user's JWT, forwarded for server-side entitlement (Set 8).
    user_token: Option<String>,
}

impl ProxyReqwestClient {
    /// Build from the environment: `GAPLY_PROXY_URL` (else the loopback
    /// default) and the App Check signing key from the OS keychain. Returns
    /// `Err` when the signing key is absent — that is the intended signal for
    /// `verify_proxy()` to skip the cloud tier and fall through cleanly, since
    /// without the key no acceptable token can be minted.
    pub fn from_env() -> Result<Self, GaplyError> {
        let base_url =
            std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| DEFAULT_PROXY_URL.to_string());
        let signer = TokenSigner::from_keychain()?;
        Self::new(&base_url, signer)
    }

    /// Build with an explicit endpoint + signer (tests, future tiering).
    pub fn new(base_url: &str, signer: TokenSigner) -> Result<Self, GaplyError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| GaplyError::Internal(format!("proxy client build failed: {e}")))?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            signer,
            client,
            user_token: None,
        })
    }

    /// Attach the signed-in user's JWT so the proxy can enforce entitlement
    /// server-side (empty/None → header omitted; pre-login and tests).
    pub fn with_user_token(mut self, token: Option<String>) -> Self {
        self.user_token = token.filter(|t| !t.is_empty());
        self
    }

    /// Liveness probe: `GET /health` with a short timeout. Used by
    /// `verify_proxy()` to decide whether to route to the cloud, so an
    /// undeployed proxy never breaks the run.
    pub fn reachable(&self) -> bool {
        let Ok(probe) = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .build()
        else {
            return false;
        };
        probe
            .get(format!("{}/health", self.base_url))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Like [`ProxyClient::verify`], but ALSO returns the envelope metadata
    /// (`result.model` / `result.stop_reason`) that the trait's `verify`
    /// discards. Used by the Box-4 comparison harness so `model_identifier` and
    /// `stop_reason` flip from `Unavailable` to `Observed` the moment `/verify`
    /// is live — at near-zero cost (the datum is already on the wire).
    pub fn verify_with_envelope(
        &self,
        payload: &Value,
    ) -> Result<(Value, ProxyEnvelope), GaplyError> {
        // Fresh limited-use token per request (single-use; never reuse).
        let (header, token) = proxy_auth_header(&self.signer)?;

        let url = format!("{}/verify", self.base_url);
        let mut req = self.client.post(&url).header(header, token).json(payload);
        // User entitlement credential (Set 8) — the proxy's real gate needs to
        // know WHICH user asks; App Check only proves WHICH app.
        if let Some(user) = &self.user_token {
            req = req.header(USER_TOKEN_HEADER, user);
        }
        let resp = req
            .send()
            .map_err(|e| GaplyError::Internal(format!("proxy POST {url} failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(map_error_status(status, resp));
        }

        // Success envelope: {"result": {"model", "stop_reason", "text"}}.
        // `text` is Claude's reply string, parsed to JSON (mirrors
        // OllamaVerifyClient); `model`/`stop_reason` are surfaced as metadata.
        let envelope: Value = resp
            .json()
            .map_err(|e| GaplyError::Internal(format!("proxy response body read failed: {e}")))?;
        let result = &envelope["result"];
        let text = result["text"].as_str().ok_or_else(|| {
            GaplyError::Validation("proxy response missing result.text".into())
        })?;
        let reply = serde_json::from_str(text)
            .map_err(|e| GaplyError::Validation(format!("proxy reply is not valid JSON: {e}")))?;
        let meta = ProxyEnvelope {
            model: result["model"].as_str().map(str::to_string),
            stop_reason: result["stop_reason"].as_str().map(str::to_string),
        };
        Ok((reply, meta))
    }
}

/// Envelope metadata the proxy returns alongside the reply. The trait `verify`
/// discards these; [`ProxyReqwestClient::verify_with_envelope`] surfaces them.
#[derive(Debug, Clone, Default)]
pub struct ProxyEnvelope {
    pub model: Option<String>,
    pub stop_reason: Option<String>,
}

impl ProxyClient for ProxyReqwestClient {
    fn verify(&self, payload: &Value) -> Result<Value, GaplyError> {
        // The trait contract returns the parsed reply only; the envelope
        // metadata is dropped here (callers that need it use verify_with_envelope).
        self.verify_with_envelope(payload).map(|(reply, _)| reply)
    }
}

/// Map a non-2xx proxy response to an honest, distinct [`GaplyError`]. Reads
/// `Retry-After` (429) and a truncated body for context.
fn map_error_status(
    status: reqwest::StatusCode,
    resp: reqwest::blocking::Response,
) -> GaplyError {
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = resp.text().unwrap_or_default();
    let detail = body.chars().take(300).collect::<String>();
    match status.as_u16() {
        401 => GaplyError::Internal(format!("proxy app_check_failed (401): {detail}")),
        422 => GaplyError::Validation(format!("proxy validation_failed (422): {detail}")),
        429 => {
            let ra = retry_after.map(|r| format!(", retry after {r}s")).unwrap_or_default();
            GaplyError::Conflict(format!("proxy rate_limited (429){ra}: {detail}"))
        }
        503 => GaplyError::Config(format!("proxy unavailable (503): {detail}")),
        other => GaplyError::Internal(format!("proxy returned {other}: {detail}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    use gaply_core::app_check::{AppCheckVerifier, DEFAULT_APP_ID};

    const KEY: &[u8] = b"proxy-test-signing-key-0123456789abcdef";

    #[derive(Clone, Copy)]
    enum Mode {
        Ok,
        /// 200 ONLY when the X-Gaply-User-Token header carries the expected
        /// user JWT (Set 8 entitlement plumbing), 403 otherwise.
        RequireUserToken,
        Unauthorized,
        Unprocessable,
        RateLimited,
        Unavailable,
    }

    /// Minimal HTTP/1.1 mock of the gaply-proxy: serves GET /health (200) and
    /// POST /verify per `mode`. In Ok mode it VERIFIES the App Check token with
    /// the same shared key, proving the client attached a valid one.
    struct MockProxy {
        addr: SocketAddr,
        stop: Arc<AtomicBool>,
    }

    impl MockProxy {
        fn start(mode: Mode) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = stop.clone();
            thread::spawn(move || {
                let verifier = AppCheckVerifier::new(KEY, DEFAULT_APP_ID);
                for stream in listener.incoming() {
                    if stop_thread.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(s) = stream {
                        handle(s, mode, &verifier);
                    }
                }
            });
            Self { addr, stop }
        }

        fn url(&self) -> String {
            format!("http://{}", self.addr)
        }
    }

    impl Drop for MockProxy {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(self.addr);
        }
    }

    fn read_request(stream: &mut TcpStream) -> String {
        // Read with a short timeout: once the client's request is fully sent,
        // the next read times out and we respond. Robust for small requests.
        stream.set_read_timeout(Some(Duration::from_millis(150))).ok();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    }

    fn send(stream: &mut TcpStream, code: u16, status: &str, extra_headers: &str, body: &str) {
        let resp = format!(
            "HTTP/1.1 {code} {status}\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
    }

    fn handle(mut stream: TcpStream, mode: Mode, verifier: &AppCheckVerifier) {
        let req = read_request(&mut stream);
        let first = req.lines().next().unwrap_or("");

        if first.starts_with("GET /health") {
            send(&mut stream, 200, "OK", "", r#"{"status":"ok"}"#);
            return;
        }
        if !first.starts_with("POST /verify") {
            send(&mut stream, 404, "Not Found", "", r#"{"error":"not_found"}"#);
            return;
        }

        // Extract the App Check header the client attached.
        let token = req
            .lines()
            .find_map(|l| {
                let (name, value) = l.split_once(':')?;
                name.trim().eq_ignore_ascii_case("X-Firebase-AppCheck").then(|| value.trim().to_string())
            })
            .unwrap_or_default();

        match mode {
            Mode::RequireUserToken => {
                let user = req
                    .lines()
                    .find_map(|l| {
                        let (name, value) = l.split_once(':')?;
                        name.trim()
                            .eq_ignore_ascii_case("X-Gaply-User-Token")
                            .then(|| value.trim().to_string())
                    })
                    .unwrap_or_default();
                if user == "user-jwt-123" {
                    send(
                        &mut stream,
                        200,
                        "OK",
                        "",
                        r#"{"result":{"model":"stub","stop_reason":"end_turn","text":"{\"verdicts\":[]}"}}"#,
                    );
                } else {
                    send(&mut stream, 403, "Forbidden", "", r#"{"error":"not_entitled","reason":"missing user token"}"#);
                }
            }
            Mode::Ok => {
                if verifier.verify(&token).is_ok() {
                    // result.text is Claude's reply — a JSON string.
                    send(
                        &mut stream,
                        200,
                        "OK",
                        "",
                        r#"{"result":{"model":"stub","stop_reason":"end_turn","text":"{\"verdicts\":[]}"}}"#,
                    );
                } else {
                    send(&mut stream, 401, "Unauthorized", "", r#"{"error":"app_check_failed","reason":"bad"}"#);
                }
            }
            Mode::Unauthorized => {
                send(&mut stream, 401, "Unauthorized", "", r#"{"error":"app_check_failed","reason":"bad_signature"}"#)
            }
            Mode::Unprocessable => {
                send(&mut stream, 422, "Unprocessable Entity", "", r#"{"error":"validation_failed","reason":"too_long"}"#)
            }
            Mode::RateLimited => {
                send(&mut stream, 429, "Too Many Requests", "Retry-After: 3\r\n", r#"{"error":"rate_limited","retry_after":3}"#)
            }
            Mode::Unavailable => {
                send(&mut stream, 503, "Service Unavailable", "", r#"{"error":"claude_not_configured"}"#)
            }
        }
    }

    fn client_for(proxy: &MockProxy) -> ProxyReqwestClient {
        ProxyReqwestClient::new(&proxy.url(), TokenSigner::new(KEY, DEFAULT_APP_ID)).unwrap()
    }

    #[test]
    fn health_probe_true_when_serving() {
        let proxy = MockProxy::start(Mode::Ok);
        assert!(client_for(&proxy).reachable());
    }

    #[test]
    fn reachable_false_on_dead_url() {
        // Nothing is listening here → clean false, no panic.
        let c = ProxyReqwestClient::new("http://127.0.0.1:1", TokenSigner::new(KEY, DEFAULT_APP_ID))
            .unwrap();
        assert!(!c.reachable());
    }

    #[test]
    fn verify_attaches_valid_token_and_unwraps_result_text() {
        let proxy = MockProxy::start(Mode::Ok);
        // The mock 200s ONLY if the X-Firebase-AppCheck token verifies with the
        // shared key, so success proves a valid token was attached. The result
        // envelope is unwrapped and result.text parsed into JSON.
        let out = client_for(&proxy).verify(&serde_json::json!({"summary": {}})).unwrap();
        assert_eq!(out, serde_json::json!({"verdicts": []}));
    }

    #[test]
    fn verify_with_envelope_surfaces_model_and_stop_reason() {
        // The mock envelope is {"result":{"model":"stub","stop_reason":"end_turn",...}}.
        // verify_with_envelope returns the same parsed reply AS verify, PLUS the
        // envelope metadata that verify discards.
        let proxy = MockProxy::start(Mode::Ok);
        let (reply, meta) =
            client_for(&proxy).verify_with_envelope(&serde_json::json!({"summary": {}})).unwrap();
        assert_eq!(reply, serde_json::json!({"verdicts": []}));
        assert_eq!(meta.model.as_deref(), Some("stub"));
        assert_eq!(meta.stop_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn verify_is_a_strict_projection_of_verify_with_envelope() {
        let proxy = MockProxy::start(Mode::Ok);
        let client = client_for(&proxy);
        let payload = serde_json::json!({"summary": {}});
        let plain = client.verify(&payload).unwrap();
        let (reply, _meta) = client.verify_with_envelope(&payload).unwrap();
        assert_eq!(plain, reply, "verify() must remain a strict projection of verify_with_envelope() — if this fails, the two parsing paths have drifted apart");
    }

    #[test]
    fn fresh_token_per_request_is_not_replayed() {
        // Two calls in a row must both succeed — proving each minted a fresh
        // limited-use token (a reused one would 401 as replayed).
        let proxy = MockProxy::start(Mode::Ok);
        let c = client_for(&proxy);
        assert!(c.verify(&serde_json::json!({"summary": {}})).is_ok());
        assert!(c.verify(&serde_json::json!({"summary": {}})).is_ok());
    }

    #[test]
    fn user_token_header_is_attached_when_set() {
        // The mock 200s ONLY when X-Gaply-User-Token carries the expected JWT
        // — success proves the entitlement credential crossed the wire.
        let proxy = MockProxy::start(Mode::RequireUserToken);
        let c = client_for(&proxy).with_user_token(Some("user-jwt-123".into()));
        assert!(c.verify(&serde_json::json!({"summary": {}})).is_ok());
    }

    #[test]
    fn no_user_token_means_no_header() {
        // Without a token the header is absent → the strict mock 403s, which
        // maps to an honest error (and proves nothing is silently invented).
        let proxy = MockProxy::start(Mode::RequireUserToken);
        let err = client_for(&proxy).verify(&serde_json::json!({"summary": {}})).unwrap_err();
        assert!(matches!(err, GaplyError::Internal(m) if m.contains("403")));
        // Empty string is treated as absent too (filter in with_user_token).
        let proxy2 = MockProxy::start(Mode::RequireUserToken);
        let c = client_for(&proxy2).with_user_token(Some(String::new()));
        assert!(c.verify(&serde_json::json!({"summary": {}})).is_err());
    }

    #[test]
    fn maps_401_to_internal() {
        let proxy = MockProxy::start(Mode::Unauthorized);
        let err = client_for(&proxy).verify(&serde_json::json!({"summary": {}})).unwrap_err();
        assert!(matches!(err, GaplyError::Internal(m) if m.contains("app_check_failed")));
    }

    #[test]
    fn maps_422_to_validation() {
        let proxy = MockProxy::start(Mode::Unprocessable);
        let err = client_for(&proxy).verify(&serde_json::json!({"summary": {}})).unwrap_err();
        assert!(matches!(err, GaplyError::Validation(m) if m.contains("validation_failed")));
    }

    #[test]
    fn maps_429_to_conflict_with_retry_after() {
        let proxy = MockProxy::start(Mode::RateLimited);
        let err = client_for(&proxy).verify(&serde_json::json!({"summary": {}})).unwrap_err();
        assert!(matches!(err, GaplyError::Conflict(m) if m.contains("rate_limited") && m.contains("retry after 3s")));
    }

    #[test]
    fn maps_503_to_config() {
        let proxy = MockProxy::start(Mode::Unavailable);
        let err = client_for(&proxy).verify(&serde_json::json!({"summary": {}})).unwrap_err();
        assert!(matches!(err, GaplyError::Config(m) if m.contains("unavailable")));
    }
}
