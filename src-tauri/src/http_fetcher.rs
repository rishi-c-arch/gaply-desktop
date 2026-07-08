//! Real HTTP transport for the refverify connectors (Phase 3, Stage 1).
//!
//! gaply-core defines the synchronous `HttpFetcher` trait and ships only
//! `MockHttpFetcher` (tests). This is the PRODUCTION implementation, kept in the
//! app crate so gaply-core stays network-free (and `cargo test -p gaply_core`
//! pulls no TLS stack — protecting the green Windows CI).
//!
//! Rate limiting and caching are already applied by `refverify::cached_fetch`
//! BEFORE this fetcher is ever called (it consumes a token from the per-API
//! `RateLimiter` first), so this type is pure transport: one GET, map the
//! response. TLS is rustls (no OpenSSL).

use gaply_core::extract::citations::Reference;
use gaply_core::refverify::{
    verify_reference, ApiRateLimiters, HttpFetcher, HttpRequest, HttpResponse, ReferenceVerification,
    VerifyContext,
};
use gaply_core::{Database, GaplyError};

/// Networked `HttpFetcher` backed by a shared `reqwest::blocking::Client`
/// (connection pool reused across calls). Cloneable — clones share the pool.
#[derive(Clone)]
pub struct ReqwestFetcher {
    client: reqwest::blocking::Client,
}

impl ReqwestFetcher {
    pub fn new() -> Result<Self, GaplyError> {
        let client = reqwest::blocking::Client::builder()
            // A descriptive UA is good manners for CrossRef/Unpaywall etc.
            .user_agent(concat!("Gaply/", env!("CARGO_PKG_VERSION"), " (research-integrity)"))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| GaplyError::Internal(format!("http client build failed: {e}")))?;
        Ok(Self { client })
    }
}

impl HttpFetcher for ReqwestFetcher {
    fn get(&self, req: &HttpRequest) -> Result<HttpResponse, GaplyError> {
        let mut rb = self.client.get(&req.url);
        for (k, v) in &req.headers {
            rb = rb.header(k.as_str(), v.as_str());
        }
        let resp = rb
            .send()
            .map_err(|e| GaplyError::Internal(format!("http GET {} failed: {e}", req.url)))?;
        let status = resp.status().as_u16();
        // Read the body regardless of status — connectors branch on `status`.
        let body = resp
            .text()
            .map_err(|e| GaplyError::Internal(format!("http body read failed: {e}")))?;
        Ok(HttpResponse { status, body })
    }
}

/// Reference verifier: owns a real HTTP fetcher + the per-API rate limiters,
/// so both the `verify_reference` command and the pipeline's verify stage share
/// one construction. Caching + rate limiting happen inside
/// `refverify::verify_reference` (via `cached_fetch`); this just provides the
/// context. Blocking IO — call from a blocking context (spawn_blocking).
pub struct RefVerifier {
    fetcher: ReqwestFetcher,
    limiters: ApiRateLimiters,
}

impl RefVerifier {
    pub fn new() -> Result<Self, GaplyError> {
        Ok(Self { fetcher: ReqwestFetcher::new()?, limiters: ApiRateLimiters::with_polite_defaults() })
    }

    /// Verify one reference against the live connectors (cache-first, rate-
    /// limited), returning the structured `ReferenceVerification`.
    pub fn verify(
        &self,
        db: &Database,
        reference: &Reference,
        now: i64,
    ) -> Result<ReferenceVerification, GaplyError> {
        let ctx = VerifyContext {
            db,
            http: &self.fetcher,
            limiters: &self.limiters,
            contact_email: None,
        };
        verify_reference(&ctx, reference, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_client() {
        // Constructing the real fetcher must not panic or require network.
        assert!(ReqwestFetcher::new().is_ok());
    }

    #[test]
    fn builds_a_verifier() {
        assert!(RefVerifier::new().is_ok());
    }
}
