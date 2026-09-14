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
//!
//! # WHAT THE USER-AGENT CLAIMS, AND WHY IT DOES NOT PRETEND TO BE A BROWSER
//!
//! `Gaply/<version> (research-integrity; +https://gaply.in)` — a bare,
//! honest identification: what the tool is, and where to read about it. It does
//! NOT carry the `Mozilla/5.0 (compatible; …)` wrapper that most well-behaved
//! crawlers use, and it does not impersonate Chrome.
//!
//! **That was a measured choice, not a principled one taken on faith.** Journal
//! guideline pages sit behind Cloudflare-style filters, and the old client — a
//! `Gaply/…` UA and nothing else — was refused by five of the nine largest
//! academic publishers. Measured 14 Sep 2026, same URLs, same session:
//!
//! | publisher | UA only (old client) | UA + the header set below |
//! |---|---|---|
//! | PLOS, Springer/nature.com, Frontiers, BioMed Central | 200 | 200 |
//! | **Elsevier (Lancet), Wiley, Taylor & Francis, SAGE** | **403** | **200** |
//! | BMJ | 403 / 200, intermittently | 200 |
//!
//! Three UA strings were then compared against the full header set — the bare
//! `Gaply/…`, a `Mozilla/5.0 (compatible; Gaply/…)` wrapper, and a real Chrome
//! string. **All three returned 200 from all nine.** The filters are not
//! reading the UA; so the honest string is kept, because impersonation would
//! have bought exactly nothing.
//!
//! # IT IS THE COMPLETENESS OF THE HEADER SET, NOT ANY ONE HEADER
//!
//! Ablated against the four publishers that refused the old client. `Accept`
//! alone: 403. `Accept-Language` too: 403. A cookie store as well: 403.
//! `Accept-Encoding` alone, `Sec-Fetch-*` alone, `Upgrade-Insecure-Requests`
//! alone: 403, 403, 403. **All of them together: 200.** These filters
//! fingerprint the shape of the whole request, so there is no smaller subset to
//! ship and no single line to remove later without silently losing publishers.
//!
//! The cookie store earns its place separately: Wiley answers a cookie-less
//! request with a 200 page whose title is *"Error - Cookies Turned Off"* — a
//! success status carrying no content, which is the failure mode
//! `guidelines::looks_like_guideline_page` exists to catch.
//!
//! **Why this matters beyond politeness:** without it, a crawl over ten
//! journals measures `ReqwestFetcher` rather than measuring journals.

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

/// The honest identification. See the module header for why it is not dressed
/// up as a browser.
/// OpenAlex asks bulk callers for a contact address and routes identified
/// traffic to a faster pool. It is also the honest thing for a tool making
/// repeated requests of a free public service.
pub const CONTACT: &str = "mailto:rishirajsharma8055@gmail.com";

pub const USER_AGENT: &str = concat!(
    "Gaply/",
    env!("CARGO_PKG_VERSION"),
    " (research-integrity; +https://gaply.in; mailto:rishirajsharma8055@gmail.com)"
);

/// The header set a publisher's filter expects on a document navigation.
///
/// Shipped as one list because it only works as one list — see the ablation in
/// the module header. `Accept-Encoding` is handled by reqwest's `gzip`/`brotli`
/// features rather than set here, so the client can actually decode what it
/// asks for.
const NAVIGATION_HEADERS: &[(&str, &str)] = &[
    ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    ("Accept-Language", "en-GB,en;q=0.9"),
    ("Sec-Fetch-Dest", "document"),
    ("Sec-Fetch-Mode", "navigate"),
    ("Sec-Fetch-Site", "none"),
    ("Upgrade-Insecure-Requests", "1"),
];

impl ReqwestFetcher {
    pub fn new() -> Result<Self, GaplyError> {
        let mut headers = reqwest::header::HeaderMap::new();
        for (k, v) in NAVIGATION_HEADERS {
            match (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                (Ok(name), Ok(value)) => {
                    headers.insert(name, value);
                }
                // A malformed constant is a programming error, not a runtime
                // condition — but it must not take the whole client down, so
                // the header is dropped and the client still builds.
                _ => tracing::error!(header = %k, "invalid default header constant"),
            }
        }
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            // Publishers set a challenge cookie on the first response and
            // expect it back. Without this, Wiley answers 200 with
            // "Error - Cookies Turned Off" and no content.
            .cookie_store(true)
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
        // BOUNDED read (M4): the body can be an ARBITRARY, user-supplied journal
        // page (journal_registry fetches it for the site lane), so cap it exactly
        // like the paper-corpus fetcher (crate::paper_corpus::MAX_FETCH_BYTES). A
        // lying/absent Content-Length can't OOM us — the take() in the helper is
        // the real guard. Over-cap → an honest Validation error, which the caller
        // (cached_registry_get) already degrades to "unverified"/"not stated".
        let declared = resp.content_length();
        let bytes = read_body_capped(resp, declared, crate::paper_corpus::MAX_FETCH_BYTES)?;
        let body = String::from_utf8_lossy(&bytes).into_owned();
        Ok(HttpResponse { status, body })
    }
}

/// Read an HTTP response body into memory with a HARD size cap, mirroring the
/// paper-corpus fetcher's `ReqwestPaperFetcher::get_capped` (same
/// `crate::paper_corpus::MAX_FETCH_BYTES` — no competing cap): a fast reject on an
/// honest Content-Length, then a `Read::take`-bounded read so a lying/absent
/// Content-Length can't OOM us. Over-cap is an honest `GaplyError::Validation`,
/// never an unbounded read.
fn read_body_capped(
    reader: impl std::io::Read,
    declared_len: Option<u64>,
    cap: usize,
) -> Result<Vec<u8>, GaplyError> {
    use std::io::Read;
    // Fast reject on an honest Content-Length; the take() below is the real guard.
    if let Some(len) = declared_len {
        if len > cap as u64 {
            return Err(GaplyError::Validation(format!(
                "response body is {len} bytes; exceeds the {cap}-byte cap"
            )));
        }
    }
    let mut buf = Vec::new();
    reader
        .take((cap + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|e| GaplyError::Internal(format!("http body read failed: {e}")))?;
    if buf.len() > cap {
        return Err(GaplyError::Validation(format!(
            "response body exceeds the {cap}-byte read cap"
        )));
    }
    Ok(buf)
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

    #[test]
    fn read_body_capped_allows_under_cap_and_rejects_over_cap() {
        use std::io::Cursor;
        // under the cap → the body is returned verbatim
        let small = vec![b'a'; 100];
        assert_eq!(read_body_capped(Cursor::new(small.clone()), Some(100), 1000).unwrap(), small);
        // exactly AT the cap → allowed
        assert_eq!(
            read_body_capped(Cursor::new(vec![b'z'; 1000]), None, 1000).unwrap().len(),
            1000
        );
        // over cap via an HONEST Content-Length → fast reject
        assert!(matches!(
            read_body_capped(Cursor::new(vec![b'x'; 10]), Some(5000), 1000),
            Err(GaplyError::Validation(_))
        ));
        // over cap via the ACTUAL body with a LYING/absent Content-Length → the
        // take() guard still rejects. THIS is the memory-exhaustion protection:
        // the read never grows unboundedly past the cap.
        assert!(matches!(
            read_body_capped(Cursor::new(vec![b'y'; 2000]), None, 1000),
            Err(GaplyError::Validation(_))
        ));
    }

    #[test]
    fn shares_the_paper_corpus_cap_no_competing_mechanism() {
        // Consistency: the journal-URL read uses the SAME cap as the paper-corpus
        // fetcher — one constant, not a second competing limit.
        assert_eq!(crate::paper_corpus::MAX_FETCH_BYTES, 20 * 1024 * 1024);
    }
}
