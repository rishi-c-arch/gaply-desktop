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

/// Reachability probe schedule: `(per-attempt timeout, backoff-after)` steps.
/// Attempt 1 keeps the original short 2s timeout so a genuinely-down or
/// undeployed proxy still fails fast; later attempts escalate the timeout and
/// space out with backoff so a slow-to-wake remote host (e.g. a free-tier PaaS
/// cold start) is tolerated. Worst-case wall time ≈ 2+1+10+2+20 = 35s, and ONLY
/// when a host accepts the connection but then stalls (or a fronting gateway
/// returns a "waking" 5xx) — a connection refusal or DNS failure short-circuits
/// on the first attempt (see `reachable_with_schedule`).
const PROBE_SCHEDULE: &[(Duration, Duration)] = &[
    (Duration::from_secs(2), Duration::from_secs(1)),
    (Duration::from_secs(10), Duration::from_secs(2)),
    (Duration::from_secs(20), Duration::from_secs(0)),
];

/// Inference-call timeout. Cloud reasoning can be slow (adaptive thinking), so
/// a generous ceiling — still bounded.
const REQUEST_TIMEOUT_SECS: u64 = 180;

/// Header carrying the signed-in user's Supabase JWT (Set 8). The USER'S OWN
/// credential, never an app secret: the proxy verifies it server-side to run
/// THE REAL entitlement gate before paid work. Optional until the proxy is
/// deployed with enforcement on.
const USER_TOKEN_HEADER: &str = "X-Gaply-User-Token";

/// **Is the configured proxy a real deployment, or the loopback default?**
///
/// # Why this is a STRING check and not a probe
///
/// The reviewer letter is the one part of PublishReady that needs the proxy, and
/// when the proxy is not deployed the user learned that only AFTER a full
/// analysis run — minutes of local work, then a panel saying deep reasoning was
/// unavailable. Saying it up front needs an answer in milliseconds.
///
/// Two things make a probe the wrong instrument here:
///
/// * [`ProxyReqwestClient::reachable`] walks [`PROBE_SCHEDULE`] and can take
///   **35 seconds** before it concludes, which is not a pre-flight check.
/// * [`ProxyReqwestClient::from_env`] reads the App Check signing key from the
///   OS keychain, and on macOS `get_password()` can raise an interactive
///   "allow access?" panel and BLOCK until somebody clicks it (§11 D124). A
///   pre-flight check that can hang the app behind a system dialog is worse
///   than no pre-flight check.
///
/// So this answers the cheap, certain half: an unset `GAPLY_PROXY_URL` leaves
/// [`DEFAULT_PROXY_URL`], which is loopback, and **nothing is listening on this
/// machine** — no amount of network would change that. A configured remote URL
/// may still be down; that is what the run's own honest degradation is for, and
/// this deliberately does not claim otherwise.
pub fn configured_proxy_url() -> String {
    std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| DEFAULT_PROXY_URL.to_string())
}

/// True when `url`'s host is a loopback address or `localhost`.
///
/// Parsed off the authority rather than substring-matched: `http://127.0.0.1:8080`
/// and `http://localhost/` are loopback, while a hostname that merely CONTAINS
/// one of those strings (`https://localhost.example.com`, `https://not-127.0.0.1.example.com`)
/// is a real remote host and must not be reported as undeployed.
pub fn is_loopback_url(url: &str) -> bool {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    // Authority ends at the first '/', '?' or '#'.
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    // Drop userinfo, then the port. IPv6 literals are bracketed.
    let hostport = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let host = if let Some(rest) = hostport.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        hostport.split(':').next().unwrap_or("")
    };
    let host = host.trim().to_ascii_lowercase();
    if host == "localhost" || host == "::1" {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => false,
    }
}

/// A remote [`ProxyClient`] backed by the gaply-proxy `/verify` endpoint.
pub struct ProxyReqwestClient {
    base_url: String,
    signer: TokenSigner,
    client: reqwest::blocking::Client,
    /// Signed-in user's JWT, forwarded for server-side entitlement (Set 8).
    user_token: Option<String>,
}

/// The classified result of a single `/health` probe. `reachable()` collapses
/// this to `bool`, but the retry loop needs the distinction: only `TimedOut`
/// and `Waking` (a slow-to-wake host) are retried; `Live`, `Erroring`, and
/// `Unreachable` are decisive and stop the loop immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeOutcome {
    /// 2xx — the proxy answered a health check. The only `reachable() == true`.
    Live,
    /// 502/503/504 from a fronting gateway — "waking up", so retry.
    Waking,
    /// Some other non-2xx — the server answered and is genuinely erroring
    /// (401/404/422/429/500…). Decisive: not asleep, so never retried.
    Erroring(reqwest::StatusCode),
    /// The per-attempt timeout elapsed — the classic cold-start signature.
    TimedOut,
    /// Connection refused / DNS failure / other transport error — nothing is
    /// listening (or it is unroutable). Decisive fast-fail, never retried.
    Unreachable,
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
        let signer = Self::signer_from_env()?;
        Self::new(&base_url, signer)
    }

    /// The App Check signing key, or the documented `Err` that routes
    /// verification to the local tier.
    ///
    /// # WHY THIS MAY REFUSE TO READ THE KEYCHAIN AT ALL (§11 D124)
    ///
    /// `get_password()` on macOS does not merely look an item up — it checks
    /// the calling binary against the item's ACL, and when the binary is not on
    /// it the OS raises an interactive "allow access?" panel and BLOCKS until
    /// somebody clicks it. `cargo test` rebuilds an unsigned binary with a new
    /// identity every time, so it is never on that ACL.
    ///
    /// The effect was that `cargo test --workspace` could not finish
    /// unattended on a developer machine that had ever provisioned the key:
    /// four `pipeline::tests` sat at **0% CPU** inside `get_secret`, forever,
    /// waiting for a panel that a headless runner never shows. Two whole
    /// workspace runs were lost to it, and the fault was invisible — a hung
    /// test looks exactly like a slow one. It reproduced at HEAD with every
    /// local change stashed, so it was never a regression; it was a gate that
    /// had quietly stopped being able to close.
    ///
    /// **A test that requires an interactive OS prompt cannot gate anything**,
    /// so under `cfg(test)` the read is skipped and the ABSENT-KEY error is
    /// returned instead. That is not a special case invented for tests: it is
    /// the path `verify_proxy_with` already documents and takes ("signing key
    /// absent; using local verification"), and the four tests are about the
    /// pipeline's lanes, not about which verification tier answers.
    ///
    /// `GAPLY_SKIP_KEYCHAIN` does the same for anything `cfg(test)` cannot
    /// reach — integration tests, a CI shell, a bisect script.
    ///
    /// NOT a production timeout. In the shipped app the panel is answerable
    /// because there is a window server and a signed, stable binary, and
    /// silently skipping the cloud tier while the user reads the prompt would
    /// be the wrong answer.
    fn signer_from_env() -> Result<TokenSigner, GaplyError> {
        if cfg!(test) || std::env::var_os("GAPLY_SKIP_KEYCHAIN").is_some() {
            return Err(GaplyError::NotFound {
                entity: "secret",
                id: gaply_core::app_check::SIGNING_KEY_SECRET.to_string(),
            });
        }
        TokenSigner::from_keychain()
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

    /// Liveness probe: `GET /health`, tolerant of a slow-to-wake remote host.
    ///
    /// PUBLIC CONTRACT UNCHANGED: `-> bool`, `true` iff the proxy answered a
    /// health check successfully. A connection refusal or DNS failure still
    /// fails fast on the first attempt (no retry) — preserving the original
    /// "undeployed proxy fails fast" behavior for a local/loopback proxy that
    /// simply isn't running. The only new behavior is that a per-attempt timeout
    /// or a gateway "waking" status (502/503/504) — the cold-start signatures —
    /// are retried per `PROBE_SCHEDULE` instead of being reported as down.
    pub fn reachable(&self) -> bool {
        matches!(
            self.reachable_with_schedule(PROBE_SCHEDULE, |d| std::thread::sleep(d)),
            ProbeOutcome::Live
        )
    }

    /// One `GET /health` attempt, classified into a `ProbeOutcome`. No retry.
    fn probe_once(&self, timeout: Duration) -> ProbeOutcome {
        let Ok(probe) = reqwest::blocking::Client::builder().timeout(timeout).build() else {
            return ProbeOutcome::Unreachable;
        };
        match probe.get(format!("{}/health", self.base_url)).send() {
            Ok(r) if r.status().is_success() => ProbeOutcome::Live,
            Ok(r) if matches!(r.status().as_u16(), 502 | 503 | 504) => ProbeOutcome::Waking,
            Ok(r) => ProbeOutcome::Erroring(r.status()),
            // `is_timeout()` is the per-attempt ceiling elapsing — the classic
            // cold-start signature. Everything else (connection refused, DNS
            // failure, other transport errors) is a hard, non-retryable miss.
            Err(e) if e.is_timeout() => ProbeOutcome::TimedOut,
            Err(_) => ProbeOutcome::Unreachable,
        }
    }

    /// Drive `probe_once` across a schedule of `(timeout, backoff)` steps,
    /// retrying ONLY the cold-start signatures (`TimedOut`, `Waking`) and
    /// exiting immediately on a decisive outcome (`Live`, `Erroring`,
    /// `Unreachable`). `sleep` is injected so tests drive the schedule with no
    /// real delay. Returns the LAST probe's outcome.
    fn reachable_with_schedule(
        &self,
        schedule: &[(Duration, Duration)],
        sleep: impl Fn(Duration),
    ) -> ProbeOutcome {
        let mut last = ProbeOutcome::Unreachable;
        for (i, &(timeout, backoff)) in schedule.iter().enumerate() {
            last = self.probe_once(timeout);
            match last {
                // Decisive: the server answered (well or badly), or nothing is
                // listening. No amount of waiting changes these — stop now.
                ProbeOutcome::Live | ProbeOutcome::Erroring(_) | ProbeOutcome::Unreachable => {
                    return last;
                }
                // Cold-start signatures — back off and try again if steps remain.
                ProbeOutcome::TimedOut | ProbeOutcome::Waking => {
                    if i + 1 < schedule.len() && !backoff.is_zero() {
                        sleep(backoff);
                    }
                }
            }
        }
        last
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
        // 401 has TWO causes and the message must not presume one: App Check
        // (wrong/absent app credential) or the user token (absent/expired
        // entitlement credential). It previously read "app_check_failed"
        // unconditionally, which sent a real `user_token_missing` 401 chasing the
        // wrong credential. `detail` carries the proxy's own error code, so the
        // actual cause is still named — by the server, not guessed here.
        401 => GaplyError::Internal(format!("proxy unauthorized (401, app_check or user_token): {detail}")),
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
    /// Loopback detection is a SECURITY-SHAPED string parse: a false "yes" tells
    /// a user with a working proxy that their reviewer letter is unavailable,
    /// and a false "no" is the silence this blocker exists to remove. The
    /// substring implementations both fail on the same cases, so they are
    /// asserted explicitly.
    #[test]
    fn loopback_detection_reads_the_host_not_the_string() {
        use super::is_loopback_url;
        for yes in [
            "http://127.0.0.1:8080",
            "http://127.0.0.1",
            "http://localhost:8080/verify",
            "https://LOCALHOST/",
            "http://[::1]:8080",
            "http://127.5.5.5:9/x?y#z",
            "http://user:pw@127.0.0.1:8080",
        ] {
            assert!(is_loopback_url(yes), "{yes} is loopback");
        }
        for no in [
            "https://gaply-proxy.tailnet.ts.net",
            "https://localhost.example.com/verify",
            "https://not-127.0.0.1.example.com",
            "https://127.0.0.1.nip.io",
            "https://example.com/localhost",
            "",
        ] {
            assert!(!is_loopback_url(no), "{no} is NOT loopback");
        }
    }

    /// The default is loopback — which is the whole reason the notice exists.
    #[test]
    fn an_unset_env_resolves_to_the_loopback_default() {
        use super::{configured_proxy_url, is_loopback_url, DEFAULT_PROXY_URL};
        assert!(is_loopback_url(DEFAULT_PROXY_URL));
        std::env::remove_var("GAPLY_PROXY_URL");
        assert_eq!(configured_proxy_url(), DEFAULT_PROXY_URL);
        assert!(is_loopback_url(&configured_proxy_url()));
    }

    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    use gaply_core::app_check::{AppCheckVerifier, DEFAULT_APP_ID};

    const KEY: &[u8] = b"proxy-test-signing-key-0123456789abcdef";

    /// THE WORKSPACE GATE MUST BE ABLE TO CLOSE UNATTENDED.
    ///
    /// `from_env` used to reach the OS keychain, and on macOS that means an ACL
    /// check against the calling binary — which `cargo test` rebuilds unsigned
    /// and unrecognised every time. The OS then raises an interactive panel and
    /// blocks forever in a runner that can never show one, at 0% CPU, which is
    /// indistinguishable from a slow test.
    ///
    /// §11 D124. This asserts the two properties that make
    /// `cargo test --workspace` finish on its own: the call RETURNS, and it returns the absent-key error
    /// that `verify_proxy_with` already routes to local verification. If the
    /// keychain read is ever restored unconditionally, this test hangs — which
    /// is the honest failure, and it fails here rather than in whichever
    /// unrelated suite happens to run next.
    #[test]
    fn building_from_env_never_touches_the_keychain_under_test() {
        let started = std::time::Instant::now();
        let err = ProxyReqwestClient::from_env().err().expect("no signing key may be found");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "from_env took {:?} — it is reaching the OS keychain",
            started.elapsed()
        );
        assert!(
            matches!(err, GaplyError::NotFound { entity: "secret", .. }),
            "expected the ABSENT-KEY error the local-tier fall-through keys on, got {err:?}"
        );
        // And the tier selector must actually take that fall-through, or the
        // pipeline tests would still be waiting on a cloud client.
        let tier = crate::models::verify_proxy_with(ProxyReqwestClient::from_env(), None);
        let _: &dyn ProxyClient = &*tier;
    }

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

    fn client_for_url(url: &str) -> ProxyReqwestClient {
        ProxyReqwestClient::new(url, TokenSigner::new(KEY, DEFAULT_APP_ID)).unwrap()
    }

    fn client_for(proxy: &MockProxy) -> ProxyReqwestClient {
        client_for_url(&proxy.url())
    }

    // Fast schedule for the retry tests: short per-attempt timeouts, zero
    // backoff. Paired with a no-op injected sleep, retries are instant, so these
    // exercise the RETRY LOGIC (classification + loop control) without real
    // delay. Production `reachable()` runs the same code path over the real
    // `PROBE_SCHEDULE` (2s/10s/20s).
    const FAST: &[(Duration, Duration)] = &[
        (Duration::from_secs(1), Duration::ZERO),
        (Duration::from_secs(1), Duration::ZERO),
        (Duration::from_secs(1), Duration::ZERO),
    ];

    /// A proxy mock that spawns a thread PER connection (so a slow attempt never
    /// blocks accepting the next) and hands the handler a 0-based attempt index,
    /// letting a test script per-attempt behavior (timeout, then 200, etc.).
    struct ScriptedProxy {
        addr: SocketAddr,
        stop: Arc<AtomicBool>,
    }

    impl ScriptedProxy {
        fn start<F>(handler: F) -> Self
        where
            F: Fn(usize, TcpStream) + Send + Sync + 'static,
        {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = stop.clone();
            let handler = Arc::new(handler);
            thread::spawn(move || {
                let mut n = 0usize;
                for stream in listener.incoming() {
                    if stop_thread.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(s) = stream {
                        let h = handler.clone();
                        let idx = n;
                        n += 1;
                        thread::spawn(move || h(idx, s));
                    }
                }
            });
            Self { addr, stop }
        }

        fn url(&self) -> String {
            format!("http://{}", self.addr)
        }
    }

    impl Drop for ScriptedProxy {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(self.addr);
        }
    }

    #[test]
    fn health_probe_true_when_serving() {
        let proxy = MockProxy::start(Mode::Ok);
        assert!(client_for(&proxy).reachable());
    }

    #[test]
    fn reachable_false_on_dead_url() {
        // Nothing is listening here → clean false, no panic. Crucially this must
        // NOT wait through the retry schedule: a refused connection is decisive
        // (Unreachable), so reachable() returns on the FIRST attempt. Even though
        // PROBE_SCHEDULE would permit ~35s of retries+backoff, this completes
        // near-instantly — the loopback "undeployed proxy fails fast" guarantee.
        let c = ProxyReqwestClient::new("http://127.0.0.1:1", TokenSigner::new(KEY, DEFAULT_APP_ID))
            .unwrap();
        let start = std::time::Instant::now();
        assert!(!c.reachable());
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(1),
            "refused connection must fast-fail (no 35s schedule wait), took {elapsed:?}"
        );
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

    /// THE REGRESSION. The client's builder attaching the header was never the
    /// problem — `verify_proxy()` (the VERIFICATION lane's factory, in
    /// `crate::models`) built the cloud client via `from_env()` and never called
    /// `with_user_token`, so verification requests arrived with App Check but no
    /// entitlement credential and the proxy answered 401 user_token_missing;
    /// every citation then degraded to UNKNOWN.
    ///
    /// PRECISE SCOPE: this covers runs where the cloud tier was SELECTED AND
    /// REACHED. It is NOT the same as a run with no proxy configured, which
    /// fails fast on connection refusal and never reaches the gate at all — that
    /// is a separate, already-understood cause with the same UNKNOWN symptom.
    ///
    /// Behavioral, not structural: the mock parses the RAW request lines and only
    /// answers 200 when `X-Gaply-User-Token` actually carries the expected JWT.
    /// Asserting through `verify_proxy_with` (rather than a hand-built client)
    /// is what makes this a guard on the FACTORY — the thing that regressed.
    /// It lives in this module because the HTTP mock does.
    #[test]
    fn verify_proxy_attaches_the_user_token_on_the_wire() {
        let proxy = MockProxy::start(Mode::RequireUserToken);
        let client = crate::models::verify_proxy_with(
            Ok(client_for(&proxy)),
            Some("user-jwt-123".into()),
        );
        assert!(
            client.verify(&serde_json::json!({"summary": {}})).is_ok(),
            "the verification factory must forward the user token; a 403 here means \
             X-Gaply-User-Token never reached the wire"
        );
    }

    /// The other half of the contract: no token in means no header out, so the
    /// strict gate refuses. This is exactly what the whole verification lane did
    /// before the fix — pinned so a silent regression to "always None" fails.
    #[test]
    fn verify_proxy_without_a_token_is_refused_by_the_entitlement_gate() {
        let proxy = MockProxy::start(Mode::RequireUserToken);
        let client = crate::models::verify_proxy_with(Ok(client_for(&proxy)), None);
        let err = client.verify(&serde_json::json!({"summary": {}})).unwrap_err();
        assert!(
            matches!(err, GaplyError::Internal(ref m) if m.contains("403")),
            "expected the gate to refuse an untokened call, got {err:?}"
        );
    }

    /// A 401 must not blame App Check when the cause may be the user token. The
    /// proxy's own error code still names the real cause via `detail`.
    #[test]
    fn unauthorized_message_names_both_credentials() {
        let proxy = MockProxy::start(Mode::Unauthorized);
        let err = client_for(&proxy).verify(&serde_json::json!({"summary": {}})).unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("401"), "status must be stated: {msg}");
        assert!(
            msg.contains("app_check") && msg.contains("user_token"),
            "a 401 must name BOTH possible credentials, not presume one: {msg}"
        );
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

    /* ------------------- reachability retry (cold start) ------------------ */

    #[test]
    fn timeout_then_retry_succeeds() {
        // Attempt 1 hangs past the 1s per-attempt timeout → TimedOut (the
        // cold-start signature); attempt 2 answers 200 → Live. The loop retried
        // the timeout and recovered, so reachable() (which is `matches!(…, Live)`)
        // returns TRUE.
        let proxy = ScriptedProxy::start(|idx, mut stream| {
            if idx == 0 {
                // Outlast attempt 1's 1s timeout, then let the client give up.
                thread::sleep(Duration::from_millis(1500));
            } else {
                let _ = read_request(&mut stream);
                send(&mut stream, 200, "OK", "", r#"{"status":"ok"}"#);
            }
        });
        let outcome = client_for_url(&proxy.url()).reachable_with_schedule(FAST, |_| {});
        assert_eq!(outcome, ProbeOutcome::Live);
        assert!(matches!(outcome, ProbeOutcome::Live), "reachable() would be true");
    }

    #[test]
    fn waking_503_then_retry_succeeds() {
        // Attempt 1 → 503 (a fronting gateway "waking up") → Waking (retry);
        // attempt 2 → 200 → Live. reachable() returns TRUE.
        let proxy = ScriptedProxy::start(|idx, mut stream| {
            let _ = read_request(&mut stream);
            if idx == 0 {
                send(&mut stream, 503, "Service Unavailable", "", r#"{"status":"waking"}"#);
            } else {
                send(&mut stream, 200, "OK", "", r#"{"status":"ok"}"#);
            }
        });
        let outcome = client_for_url(&proxy.url()).reachable_with_schedule(FAST, |_| {});
        assert_eq!(outcome, ProbeOutcome::Live);
        assert!(matches!(outcome, ProbeOutcome::Live), "reachable() would be true");
    }

    #[test]
    fn connection_refused_is_immediate_no_retry() {
        // Nothing listening → connection refused → Unreachable, DECISIVE. Even
        // with a schedule of huge timeouts AND huge REAL backoff sleeps injected,
        // the loop must exit on attempt 1 without ever retrying or sleeping.
        let c = client_for_url("http://127.0.0.1:1");
        let brutal: &[(Duration, Duration)] = &[
            (Duration::from_secs(30), Duration::from_secs(30)),
            (Duration::from_secs(30), Duration::from_secs(30)),
            (Duration::from_secs(30), Duration::ZERO),
        ];
        let start = std::time::Instant::now();
        // Real sleep injected on purpose — it must NEVER be called here.
        let outcome = c.reachable_with_schedule(brutal, |d| std::thread::sleep(d));
        let elapsed = start.elapsed();
        assert_eq!(outcome, ProbeOutcome::Unreachable);
        assert!(
            elapsed < Duration::from_secs(1),
            "refused must fast-fail with no retry/delay, took {elapsed:?}"
        );
    }

    #[test]
    fn non_2xx_is_decisive_and_not_retried() {
        // A 404 means the server answered and is genuinely erroring — NOT asleep.
        // It must classify as Erroring (carrying the status) and stop on attempt 1.
        let proxy = ScriptedProxy::start(|_idx, mut stream| {
            let _ = read_request(&mut stream);
            send(&mut stream, 404, "Not Found", "", r#"{"error":"nope"}"#);
        });
        let outcome = client_for_url(&proxy.url()).reachable_with_schedule(FAST, |_| {});
        match outcome {
            ProbeOutcome::Erroring(status) => assert_eq!(status.as_u16(), 404),
            other => panic!("expected Erroring(404), got {other:?}"),
        }
    }
}
