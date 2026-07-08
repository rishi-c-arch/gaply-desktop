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

use gaply_core::refverify::{HttpFetcher, HttpRequest, HttpResponse};
use gaply_core::GaplyError;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_client() {
        // Constructing the real fetcher must not panic or require network.
        assert!(ReqwestFetcher::new().is_ok());
    }
}
