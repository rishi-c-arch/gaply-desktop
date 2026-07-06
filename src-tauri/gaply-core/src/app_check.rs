//! App-attestation (Firebase App Check, custom-provider path) for desktop.
//!
//! # Why
//!
//! Once the proxy service exists, only the genuine Gaply app should be able to
//! call it — otherwise the Claude/OpenAI budget behind the proxy is open to
//! billing fraud and bot abuse. App Check natively attests iOS/Android/web;
//! **desktop must use a CUSTOM provider**. This module builds that path,
//! standalone and testable, ready to wire into the proxy later.
//!
//! # Model
//!
//! The client (Rust core) mints a short-lived token attesting "I am Gaply",
//! and the server verifies it and rejects anything without a valid token.
//! Tokens are JWT-like (`base64url(header).base64url(claims).base64url(sig)`)
//! signed with **HMAC-SHA256** over a shared secret. The secret is stored in
//! the OS keychain via [`crate::secrets`] (following the Prompt 10 vault
//! pattern), never in the bundle.
//!
//! **Limited-use tokens** carry a unique `jti`; the verifier consumes each
//! `jti` once, so a replayed limited-use token is rejected — the groundwork
//! for full replay protection.
//!
//! ## Honest limitations (read before shipping)
//!
//! - A shared HMAC secret on the client is only as strong as the client's
//!   ability to keep it secret; desktop has no hardware attestation, so this
//!   is inherently weaker than mobile Play Integrity / App Attest. Keep the
//!   secret in the keychain and rotate it.
//! - In production this HMAC scheme is meant to be swapped for **real Firebase
//!   App Check tokens** (Google-signed JWTs verified via the Firebase Admin
//!   SDK / Google public keys). [`AppCheckVerifier`] is the seam where that
//!   swap happens; the proxy-facing API (`verify`) stays the same.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::GaplyError;

type HmacSha256 = Hmac<Sha256>;

/// Header the client attaches and the proxy reads (matches Firebase's name).
pub const HEADER_NAME: &str = "X-Firebase-AppCheck";
/// Application identity asserted in every token.
pub const DEFAULT_APP_ID: &str = "ai.gaply.app";
/// Keychain entry name for the App Check signing secret (see [`crate::secrets`]).
pub const SIGNING_KEY_SECRET: &str = "app_check_signing_key";
/// Default token lifetime (Firebase App Check tokens are ~30 min).
pub const DEFAULT_TTL_SECS: i64 = 1800;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    pub app_id: String,
    pub iat: i64,
    pub exp: i64,
    /// Unique token id — the unit of replay protection for limited-use tokens.
    pub jti: String,
    pub limited_use: bool,
}

/// Result of a successful verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedToken {
    pub app_id: String,
    pub jti: String,
    pub limited_use: bool,
    /// True when accepted via a registered debug token (dev/CI bypass).
    pub debug: bool,
    pub exp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("missing app check token")]
    Missing,
    #[error("malformed app check token")]
    Malformed,
    #[error("invalid signature")]
    BadSignature,
    #[error("token expired")]
    Expired,
    #[error("limited-use token replayed")]
    Replayed,
    #[error("token asserts the wrong app id")]
    WrongApp,
}

impl VerifyError {
    /// Stable machine code (for logs / the proxy's 401 body).
    pub fn code(&self) -> &'static str {
        match self {
            VerifyError::Missing => "missing",
            VerifyError::Malformed => "malformed",
            VerifyError::BadSignature => "bad_signature",
            VerifyError::Expired => "expired",
            VerifyError::Replayed => "replayed",
            VerifyError::WrongApp => "wrong_app",
        }
    }
}

fn now_nanos() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

/// Process-unique token id. Uniqueness (not unpredictability) is what replay
/// protection needs: a replayed token carries the same `jti` and is rejected.
fn new_jti(app_id: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut h = Sha256::new();
    h.update(app_id.as_bytes());
    h.update(now_nanos().to_le_bytes());
    h.update(n.to_le_bytes());
    hex24(&h.finalize())
}

fn hex24(bytes: &[u8]) -> String {
    bytes.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

fn hmac_sign(key: &[u8], data: &[u8]) -> Result<Vec<u8>, GaplyError> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .map_err(|e| GaplyError::Internal(format!("app check hmac key: {e}")))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

/// Constant-time signature check (via HMAC `verify_slice`).
fn hmac_verify(key: &[u8], data: &[u8], sig: &[u8]) -> bool {
    match <HmacSha256 as Mac>::new_from_slice(key) {
        Ok(mut mac) => {
            mac.update(data);
            mac.verify_slice(sig).is_ok()
        }
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Client — token generation (runs in the Rust core)
// ---------------------------------------------------------------------------

pub struct TokenSigner {
    key: Vec<u8>,
    app_id: String,
}

impl TokenSigner {
    pub fn new(key: impl Into<Vec<u8>>, app_id: impl Into<String>) -> Self {
        Self { key: key.into(), app_id: app_id.into() }
    }

    /// Load the signing secret from the OS keychain (Prompt 10 vault pattern).
    pub fn from_keychain() -> Result<Self, GaplyError> {
        let key = crate::secrets::get_secret(SIGNING_KEY_SECRET)?;
        Ok(Self::new(key.into_bytes(), DEFAULT_APP_ID))
    }

    /// Mint a fresh token. `limited_use` marks it single-use (replay-protected).
    pub fn mint(&self, ttl_secs: i64, limited_use: bool) -> Result<String, GaplyError> {
        self.mint_at(crate::now_epoch(), ttl_secs, limited_use, new_jti(&self.app_id))
    }

    /// Deterministic minting (test hook / explicit control over time + jti).
    pub fn mint_at(
        &self,
        now: i64,
        ttl_secs: i64,
        limited_use: bool,
        jti: impl Into<String>,
    ) -> Result<String, GaplyError> {
        let header = br#"{"alg":"HS256","typ":"JWT"}"#;
        let claims = Claims {
            app_id: self.app_id.clone(),
            iat: now,
            exp: now + ttl_secs,
            jti: jti.into(),
            limited_use,
        };
        let claims_json = serde_json::to_vec(&claims)
            .map_err(|e| GaplyError::Internal(format!("app check claims: {e}")))?;
        let signing_input =
            format!("{}.{}", URL_SAFE_NO_PAD.encode(header), URL_SAFE_NO_PAD.encode(&claims_json));
        let sig = hmac_sign(&self.key, signing_input.as_bytes())?;
        Ok(format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(sig)))
    }
}

/// Store the App Check signing secret in the OS keychain.
pub fn store_signing_key(key: &str) -> Result<(), GaplyError> {
    crate::secrets::store_secret(SIGNING_KEY_SECRET, key)
}

/// The `(header, value)` pair to attach to every proxy request. Uses a
/// **limited-use** token by default (replay-protected), per the requirement to
/// enable limited-use tokens now.
pub fn proxy_auth_header(signer: &TokenSigner) -> Result<(&'static str, String), GaplyError> {
    Ok((HEADER_NAME, signer.mint(DEFAULT_TTL_SECS, true)?))
}

// ---------------------------------------------------------------------------
// Replay guard — consumes each limited-use jti exactly once
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ReplayGuard {
    // jti -> exp; entries are dropped once expired
    seen: Mutex<HashMap<String, i64>>,
}

impl ReplayGuard {
    /// Returns true if `jti` is newly consumed, false if it was already used.
    fn consume(&self, jti: &str, exp: i64, now: i64) -> bool {
        let mut seen = self.seen.lock().unwrap();
        seen.retain(|_, e| *e > now); // opportunistic cleanup so the map stays bounded
        if seen.contains_key(jti) {
            return false;
        }
        seen.insert(jti.to_string(), exp);
        true
    }
}

// ---------------------------------------------------------------------------
// Server — token verification (reusable; the proxy will call this)
// ---------------------------------------------------------------------------

pub struct AppCheckVerifier {
    key: Vec<u8>,
    app_id: String,
    debug_tokens: HashSet<String>,
    replay: ReplayGuard,
}

impl AppCheckVerifier {
    pub fn new(key: impl Into<Vec<u8>>, app_id: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            app_id: app_id.into(),
            debug_tokens: HashSet::new(),
            replay: ReplayGuard::default(),
        }
    }

    /// Register a debug token that always passes (dev/CI bypass — mirrors
    /// Firebase App Check debug tokens). Never register one in production.
    pub fn with_debug_token(mut self, token: impl Into<String>) -> Self {
        self.debug_tokens.insert(token.into());
        self
    }

    pub fn verify(&self, token: &str) -> Result<VerifiedToken, VerifyError> {
        self.verify_at(token, crate::now_epoch())
    }

    pub fn verify_at(&self, token: &str, now: i64) -> Result<VerifiedToken, VerifyError> {
        if token.is_empty() {
            return Err(VerifyError::Missing);
        }
        if self.debug_tokens.contains(token) {
            return Ok(VerifiedToken {
                app_id: self.app_id.clone(),
                jti: String::new(),
                limited_use: false,
                debug: true,
                exp: now,
            });
        }

        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(VerifyError::Malformed);
        }
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let sig = URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| VerifyError::Malformed)?;
        // signature FIRST, so a forged/tampered token can never consume a jti
        if !hmac_verify(&self.key, signing_input.as_bytes(), &sig) {
            return Err(VerifyError::BadSignature);
        }
        let payload = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| VerifyError::Malformed)?;
        let claims: Claims =
            serde_json::from_slice(&payload).map_err(|_| VerifyError::Malformed)?;

        if claims.app_id != self.app_id {
            return Err(VerifyError::WrongApp);
        }
        if claims.exp <= now {
            return Err(VerifyError::Expired);
        }
        if claims.limited_use && !self.replay.consume(&claims.jti, claims.exp, now) {
            return Err(VerifyError::Replayed);
        }

        Ok(VerifiedToken {
            app_id: claims.app_id,
            jti: claims.jti,
            limited_use: claims.limited_use,
            debug: false,
            exp: claims.exp,
        })
    }
}

// ---------------------------------------------------------------------------
// TEST-ONLY mock proxy — exercises verification over real HTTP, NOT the proxy
// ---------------------------------------------------------------------------

#[cfg(test)]
mod stub {
    //! A minimal HTTP endpoint that models the FUTURE proxy for tests only.
    //! It requires a valid App Check token in the `X-Firebase-AppCheck` header
    //! (200 if valid, 401 otherwise). This is NOT the real proxy — it exists
    //! solely to test verification logic in isolation over real HTTP.

    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    use super::{AppCheckVerifier, HEADER_NAME};

    pub struct MockProxy {
        addr: SocketAddr,
        stop: Arc<AtomicBool>,
    }

    impl MockProxy {
        pub fn start(verifier: Arc<AppCheckVerifier>) -> std::io::Result<Self> {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let addr = listener.local_addr()?;
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = stop.clone();
            thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop_thread.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(s) = stream {
                        handle(s, &verifier);
                    }
                }
            });
            Ok(Self { addr, stop })
        }

        pub fn addr(&self) -> SocketAddr {
            self.addr
        }
    }

    impl Drop for MockProxy {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            // unblock the accept loop with a throwaway connection
            let _ = TcpStream::connect(self.addr);
        }
    }

    fn handle(mut stream: TcpStream, verifier: &AppCheckVerifier) {
        let mut buf = [0u8; 8192];
        let n = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(_) => return,
        };
        let req = String::from_utf8_lossy(&buf[..n]);
        let token = req
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.trim().eq_ignore_ascii_case(HEADER_NAME).then(|| value.trim().to_string())
            })
            .unwrap_or_default();

        let (code, status, reason) = match verifier.verify(&token) {
            Ok(_) => (200u16, "OK", "ok"),
            Err(e) => (401u16, "Unauthorized", e.code()),
        };
        let body = format!("{{\"reason\":\"{reason}\"}}");
        let resp = format!(
            "HTTP/1.1 {code} {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
    }

    /// Test client: send a request (optionally with a token) and return the
    /// HTTP status code.
    pub fn request_status(addr: SocketAddr, token: Option<&str>) -> u16 {
        let mut stream = TcpStream::connect(addr).expect("connect to mock proxy");
        let header = match token {
            Some(t) => format!("{HEADER_NAME}: {t}\r\n"),
            None => String::new(),
        };
        let req = format!(
            "POST /v1/proxy HTTP/1.1\r\nHost: localhost\r\n{header}Connection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).expect("write request");
        let mut resp = String::new();
        stream.read_to_string(&mut resp).expect("read response");
        resp.lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|c| c.parse().ok())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::stub::{request_status, MockProxy};
    use super::*;
    use std::sync::Arc;

    const KEY: &[u8] = b"test-app-check-shared-secret-0123456789";

    fn signer() -> TokenSigner {
        TokenSigner::new(KEY, DEFAULT_APP_ID)
    }

    // ---- required: reject a request with no valid token (via the stub) ----

    #[test]
    fn request_without_token_is_rejected() {
        let verifier = Arc::new(AppCheckVerifier::new(KEY, DEFAULT_APP_ID));
        let proxy = MockProxy::start(verifier).unwrap();
        assert_eq!(request_status(proxy.addr(), None), 401);
        // a garbage token is also rejected
        assert_eq!(request_status(proxy.addr(), Some("not-a-token")), 401);
    }

    // ---- required: a valid debug token passes (via the stub) ----

    #[test]
    fn valid_debug_token_passes() {
        let verifier =
            Arc::new(AppCheckVerifier::new(KEY, DEFAULT_APP_ID).with_debug_token("DEBUG-abc123"));
        let proxy = MockProxy::start(verifier).unwrap();
        assert_eq!(request_status(proxy.addr(), Some("DEBUG-abc123")), 200);
        // an unregistered debug-looking token still fails
        assert_eq!(request_status(proxy.addr(), Some("DEBUG-nope")), 401);
    }

    // ---- required: expired / reused limited-use tokens are rejected ----

    #[test]
    fn expired_token_is_rejected() {
        let verifier = Arc::new(AppCheckVerifier::new(KEY, DEFAULT_APP_ID));
        let proxy = MockProxy::start(verifier).unwrap();
        // mint a token that already expired (issued 1h ago, 60s TTL)
        let past = crate::now_epoch() - 3600;
        let expired = signer().mint_at(past, 60, false, "jti-expired").unwrap();
        assert_eq!(request_status(proxy.addr(), Some(&expired)), 401);
    }

    #[test]
    fn reused_limited_use_token_is_rejected() {
        let verifier = Arc::new(AppCheckVerifier::new(KEY, DEFAULT_APP_ID));
        let proxy = MockProxy::start(verifier).unwrap();
        // valid, far-future, limited-use token
        let token = signer().mint(3600, true).unwrap();
        // first use consumes it, second use is a replay
        assert_eq!(request_status(proxy.addr(), Some(&token)), 200);
        assert_eq!(request_status(proxy.addr(), Some(&token)), 401);
    }

    // ---- direct verifier unit tests (granular reasons) ----

    #[test]
    fn valid_standard_token_is_reusable_until_expiry() {
        let v = AppCheckVerifier::new(KEY, DEFAULT_APP_ID);
        let token = signer().mint(3600, false).unwrap(); // not limited-use
        assert!(v.verify(&token).is_ok());
        assert!(v.verify(&token).is_ok(), "standard tokens are reusable until exp");
    }

    #[test]
    fn tampered_payload_fails_signature() {
        let v = AppCheckVerifier::new(KEY, DEFAULT_APP_ID);
        let token = signer().mint(3600, false).unwrap();
        let mut parts: Vec<&str> = token.split('.').collect();
        // swap in a different app_id payload, keep the original signature
        let forged_payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&Claims {
                app_id: "evil.app".into(),
                iat: crate::now_epoch(),
                exp: crate::now_epoch() + 3600,
                jti: "x".into(),
                limited_use: false,
            })
            .unwrap(),
        );
        parts[1] = &forged_payload;
        let forged = parts.join(".");
        assert_eq!(v.verify(&forged).unwrap_err(), VerifyError::BadSignature);
    }

    #[test]
    fn wrong_signing_key_fails() {
        let v = AppCheckVerifier::new(b"a-completely-different-secret-key".to_vec(), DEFAULT_APP_ID);
        let token = signer().mint(3600, false).unwrap();
        assert_eq!(v.verify(&token).unwrap_err(), VerifyError::BadSignature);
    }

    #[test]
    fn wrong_app_id_is_rejected() {
        let v = AppCheckVerifier::new(KEY, "some.other.app");
        let token = signer().mint(3600, false).unwrap();
        assert_eq!(v.verify(&token).unwrap_err(), VerifyError::WrongApp);
    }

    #[test]
    fn empty_and_malformed_tokens() {
        let v = AppCheckVerifier::new(KEY, DEFAULT_APP_ID);
        assert_eq!(v.verify("").unwrap_err(), VerifyError::Missing);
        assert_eq!(v.verify("only.two").unwrap_err(), VerifyError::Malformed);
    }

    #[test]
    fn proxy_auth_header_mints_limited_use_token() {
        let (name, token) = proxy_auth_header(&signer()).unwrap();
        assert_eq!(name, HEADER_NAME);
        let v = AppCheckVerifier::new(KEY, DEFAULT_APP_ID);
        let verified = v.verify(&token).unwrap();
        assert!(verified.limited_use);
        // and it is single-use
        assert_eq!(v.verify(&token).unwrap_err(), VerifyError::Replayed);
    }
}
