//! Online reference-verification connectors (CrossRef, OpenAlex, Retraction
//! Watch, Unpaywall, Semantic Scholar) — all free / no API key for basic use.
//!
//! For each [`Reference`](crate::extract::citations::Reference) the Extraction
//! Agent produced, we can: confirm it exists (CrossRef/OpenAlex), check
//! retraction status (Retraction Watch / OpenAlex `is_retracted`), find
//! open-access copies (Unpaywall), and enrich metadata (Semantic Scholar).
//!
//! POISONING DEFENSE (same posture as [`crate::rag`] / [`crate::sanitize`]):
//! every byte fetched from the web is UNTRUSTED. Free-text fields (titles,
//! abstracts, retraction notes) are wrapped in [`UntrustedText`], which carries
//! [`Provenance`] and only yields prompt-safe text through [`UntrustedText::llm_safe`]
//! — injection-flagged content is withheld, never passed to a downstream LLM.
//! Responses are cached with a TTL in the shared `cache` table, and each
//! external API has its own token-bucket [`RateLimiter`](crate::ratelimit).
//!
//! NETWORK SEAM: `gaply_core` stays dependency-light and portable, so it does
//! not bundle an HTTP client. Connectors take an injected [`HttpFetcher`]; tests
//! use [`MockHttpFetcher`] (no real network). A production build supplies a real
//! fetcher (e.g. reqwest/ureq, or one that routes egress through the Tailscale
//! proxy) — the same "swappable trait, real impl pending" pattern as
//! `Embedder`/`HashEmbedder` and `PerplexityModel`/`HeuristicModel`.

use std::sync::Mutex;

use serde::ser::{SerializeStruct, Serializer};
use sha2::{Digest, Sha256};

use crate::extract::citations::Reference;
use crate::ratelimit::RateLimiter;
use crate::{Database, GaplyError};

// --- TTLs (seconds) ---------------------------------------------------------
const TTL_EXISTENCE: i64 = 30 * 86_400;
const TTL_RETRACTION: i64 = 7 * 86_400; // status can change -> shorter
const TTL_OA: i64 = 7 * 86_400;
const TTL_ENRICH: i64 = 14 * 86_400;

const CROSSREF_UA: &str = "gaply/1.0 (reference verification; mailto set at deploy)";

/// Placeholder substituted for any fetched free-text that trips the injection
/// scanner, so attacker-controlled instructions can never reach a prompt.
pub const REDACTED_INJECTION: &str =
    "[redacted: untrusted web content flagged as possible prompt injection]";

// ============================================================================
// HTTP seam
// ============================================================================

/// A simple HTTP GET request the connectors want to make.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self { url: url.into(), headers: Vec::new() }
    }
    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// Injected HTTP transport. Real deployments provide a networked implementation;
/// tests provide [`MockHttpFetcher`]. Must be Send + Sync so it can be shared.
pub trait HttpFetcher: Send + Sync {
    fn get(&self, req: &HttpRequest) -> Result<HttpResponse, GaplyError>;
}

/// In-memory fetcher for tests — matches a URL substring to a canned response
/// and records every call so tests can assert e.g. "cache hit -> no HTTP call".
pub struct MockHttpFetcher {
    routes: Vec<(String, u16, String)>,
    default_status: u16,
    calls: Mutex<Vec<String>>,
}

impl MockHttpFetcher {
    pub fn new() -> Self {
        Self { routes: Vec::new(), default_status: 404, calls: Mutex::new(Vec::new()) }
    }
    /// Route any request whose URL contains `url_substr` to (status, body).
    pub fn route(mut self, url_substr: &str, status: u16, body: &str) -> Self {
        self.routes.push((url_substr.to_string(), status, body.to_string()));
        self
    }
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Default for MockHttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFetcher for MockHttpFetcher {
    fn get(&self, req: &HttpRequest) -> Result<HttpResponse, GaplyError> {
        self.calls.lock().unwrap().push(req.url.clone());
        for (sub, status, body) in &self.routes {
            if req.url.contains(sub) {
                return Ok(HttpResponse { status: *status, body: body.clone() });
            }
        }
        Ok(HttpResponse { status: self.default_status, body: String::new() })
    }
}

// ============================================================================
// Provenance + untrusted content (poisoning defense)
// ============================================================================

/// Where a piece of fetched data came from — attached to everything so results
/// are traceable and cache-vs-live is explicit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Provenance {
    pub source: String,
    pub url: String,
    pub fetched_at: i64,
    pub checksum: String, // sha256 of the raw response body
    pub from_cache: bool,
}

/// Untrusted free-text fetched from the web. It CANNOT be constructed without
/// [`Provenance`], and the only prompt-safe accessor ([`Self::llm_safe`])
/// withholds anything the injection scanner flags. Raw text is available for UI
/// display only, clearly labeled untrusted.
#[derive(Debug, Clone)]
pub struct UntrustedText {
    provenance: Provenance,
    raw: String,
    cleaned: String,
    injection_flags: Vec<String>,
}

impl UntrustedText {
    /// Wrap fetched text, tagging provenance and running the injection scanner.
    pub fn new(raw: impl Into<String>, provenance: Provenance) -> Self {
        let raw = raw.into();
        let (cleaned, injection_flags) = crate::sanitize::sanitize(&raw);
        Self { provenance, raw, cleaned, injection_flags }
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
    pub fn injection_flags(&self) -> &[String] {
        &self.injection_flags
    }
    pub fn is_suspicious(&self) -> bool {
        !self.injection_flags.is_empty()
    }
    /// Raw text for UI display ONLY — never place this in an LLM prompt.
    pub fn display_raw(&self) -> &str {
        &self.raw
    }
    /// The ONLY text safe to embed in a downstream LLM prompt: normalized, and
    /// fully withheld (redacted) if any injection pattern was detected.
    pub fn llm_safe(&self) -> String {
        if self.is_suspicious() {
            REDACTED_INJECTION.to_string()
        } else {
            self.cleaned.clone()
        }
    }
}

// Serialize outward (e.g. to the UI) in a way that makes trust explicit and
// never presents raw untrusted text as prompt-safe.
impl serde::Serialize for UntrustedText {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("UntrustedText", 5)?;
        st.serialize_field("safe_text", &self.llm_safe())?;
        st.serialize_field("raw_untrusted", &self.raw)?;
        st.serialize_field("suspicious", &self.is_suspicious())?;
        st.serialize_field("injection_flags", &self.injection_flags)?;
        st.serialize_field("provenance", &self.provenance)?;
        st.end()
    }
}

// ============================================================================
// Per-API rate limiters
// ============================================================================

/// One token bucket per external API, sized to each service's polite limits.
pub struct ApiRateLimiters {
    crossref: RateLimiter,
    openalex: RateLimiter,
    retraction_watch: RateLimiter,
    unpaywall: RateLimiter,
    semantic_scholar: RateLimiter,
}

impl ApiRateLimiters {
    /// Conservative defaults that stay well within each service's public limits.
    pub fn with_polite_defaults() -> Self {
        Self {
            crossref: RateLimiter::new(20.0, 5.0),
            openalex: RateLimiter::new(10.0, 8.0),
            retraction_watch: RateLimiter::new(5.0, 1.0),
            unpaywall: RateLimiter::new(10.0, 5.0),
            semantic_scholar: RateLimiter::new(1.0, 0.34), // ~1 req / 3s unauth
        }
    }
    /// Uniform buckets — handy for tests that want to exhaust a source.
    pub fn uniform(capacity: f64, refill_rate: f64) -> Self {
        Self {
            crossref: RateLimiter::new(capacity, refill_rate),
            openalex: RateLimiter::new(capacity, refill_rate),
            retraction_watch: RateLimiter::new(capacity, refill_rate),
            unpaywall: RateLimiter::new(capacity, refill_rate),
            semantic_scholar: RateLimiter::new(capacity, refill_rate),
        }
    }
    fn get(&self, source: &str) -> &RateLimiter {
        match source {
            "crossref" => &self.crossref,
            "openalex" => &self.openalex,
            "retraction_watch" => &self.retraction_watch,
            "unpaywall" => &self.unpaywall,
            _ => &self.semantic_scholar,
        }
    }
}

impl Default for ApiRateLimiters {
    fn default() -> Self {
        Self::with_polite_defaults()
    }
}

// ============================================================================
// Verification context + cached fetch
// ============================================================================

/// Everything a connector needs: the DB (cache), the HTTP transport, the
/// per-API limiters, and a contact email (Unpaywall asks for one — not a secret).
pub struct VerifyContext<'a> {
    pub db: &'a Database,
    pub http: &'a dyn HttpFetcher,
    pub limiters: &'a ApiRateLimiters,
    pub contact_email: Option<&'a str>,
}

/// Result of a (possibly cached, possibly rate-limited) fetch.
enum Fetched {
    Body { body: String, provenance: Provenance },
    RateLimited { retry_after_secs: u64 },
    HttpStatus { status: u16 },
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Cache-then-rate-limit-then-fetch. On a cache hit, no network call and no
/// token is consumed. On a miss, consume a token, fetch, and cache 2xx bodies.
fn cached_fetch(
    ctx: &VerifyContext,
    source: &'static str,
    cache_key: &str,
    ttl_secs: i64,
    req: HttpRequest,
    now: i64,
) -> Result<Fetched, GaplyError> {
    if let Some(body) = ctx.db.cache_get(cache_key, now)? {
        let checksum = sha256_hex(&body);
        return Ok(Fetched::Body {
            provenance: Provenance {
                source: source.to_string(),
                url: req.url,
                fetched_at: now,
                checksum,
                from_cache: true,
            },
            body,
        });
    }

    let rl = ctx.limiters.get(source);
    let decision = rl.try_consume(source);
    if !decision.allowed {
        return Ok(Fetched::RateLimited {
            retry_after_secs: decision.retry_after.as_secs().max(1),
        });
    }

    let resp = ctx.http.get(&req)?;
    if !(200..300).contains(&resp.status) {
        return Ok(Fetched::HttpStatus { status: resp.status });
    }
    ctx.db.cache_put(cache_key, &resp.body, ttl_secs, now)?;
    let checksum = sha256_hex(&resp.body);
    Ok(Fetched::Body {
        provenance: Provenance {
            source: source.to_string(),
            url: req.url,
            fetched_at: now,
            checksum,
            from_cache: false,
        },
        body: resp.body,
    })
}

/// Outcome of a single connector call. Network/HTTP problems are non-fatal
/// outcomes (the overall verification continues), not `Err`.
#[derive(Debug)]
pub enum ConnectorOutcome<T> {
    Found(T),
    NotFound,
    RateLimited { retry_after_secs: u64 },
    Unavailable { detail: String },
}

fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn normalized_doi(reference: &Reference) -> Option<String> {
    reference.doi.as_deref().map(|d| {
        d.trim()
            .trim_start_matches("https://doi.org/")
            .trim_start_matches("http://doi.org/")
            .trim_start_matches("doi:")
            .trim()
            .to_string()
    }).filter(|d| !d.is_empty())
}

fn json(body: &str, ctx: &'static str) -> Result<serde_json::Value, GaplyError> {
    serde_json::from_str(body).map_err(|e| GaplyError::Internal(format!("{ctx} json: {e}")))
}

/// Join a CrossRef `author` array (`[{given, family}]`) into a readable author
/// string. Returns `None` if the field is absent, not an array, or yields no
/// names — never panics on a malformed body.
fn crossref_authors(work: &serde_json::Value) -> Option<String> {
    let names: Vec<String> = work["author"]
        .as_array()?
        .iter()
        .filter_map(|a| match (a["given"].as_str(), a["family"].as_str()) {
            (Some(g), Some(f)) => Some(format!("{g} {f}")),
            (None, Some(f)) => Some(f.to_string()),
            (Some(g), None) => Some(g.to_string()),
            // Organizations use a single `name` field.
            (None, None) => a["name"].as_str().map(str::to_string),
        })
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// CrossRef publication year from `date-parts` — tries the common containers
/// (`published`, `issued`, print/online) so records that only carry one are
/// still covered. `None` if none present or malformed.
fn crossref_year(work: &serde_json::Value) -> Option<i32> {
    for key in ["published", "issued", "published-print", "published-online"] {
        if let Some(y) = work[key]["date-parts"][0][0].as_i64() {
            return i32::try_from(y).ok();
        }
    }
    None
}

/// Join an OpenAlex `authorships` array (`[{author: {display_name}}]`) into a
/// readable author string. `None` if absent/empty — never panics.
fn openalex_authors(work: &serde_json::Value) -> Option<String> {
    let names: Vec<String> = work["authorships"]
        .as_array()?
        .iter()
        .filter_map(|a| a["author"]["display_name"].as_str().map(str::to_string))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

// ============================================================================
// Connector result types
// ============================================================================

#[derive(Debug, serde::Serialize)]
pub struct ExistenceCheck {
    pub source: &'static str,
    pub found: bool,
    pub doi: Option<String>,
    pub title: Option<UntrustedText>,
    /// Matched author list from the source. Web text — serialized through
    /// `llm_safe()` like `title`. Populated by the connectors (later change).
    pub matched_authors: Option<UntrustedText>,
    /// Matched publication year from the source. Numeric — no injection
    /// surface, mirrors `Reference.year`. Populated by the connectors (later).
    pub matched_year: Option<i32>,
    /// Some sources (OpenAlex) flag retraction inline.
    pub is_retracted_hint: Option<bool>,
    pub provenance: Provenance,
}

#[derive(Debug, serde::Serialize)]
pub struct RetractionCheck {
    pub retracted: bool,
    pub reasons: Vec<UntrustedText>,
    pub notice_url: Option<String>,
    pub provenance: Provenance,
}

#[derive(Debug, serde::Serialize)]
pub struct OpenAccess {
    pub is_oa: bool,
    pub best_url: Option<String>,
    pub license: Option<String>,
    pub provenance: Provenance,
}

#[derive(Debug, serde::Serialize)]
pub struct Enrichment {
    pub citation_count: Option<i64>,
    pub influential_citation_count: Option<i64>,
    pub abstract_text: Option<UntrustedText>,
    pub venue: Option<UntrustedText>,
    pub provenance: Provenance,
}

// ============================================================================
// Connectors
// ============================================================================

/// CrossRef — existence by DOI (`/works/{doi}`) or bibliographic title query.
pub fn crossref_lookup(
    ctx: &VerifyContext,
    reference: &Reference,
    now: i64,
) -> Result<ConnectorOutcome<ExistenceCheck>, GaplyError> {
    let (cache_key, url) = if let Some(doi) = normalized_doi(reference) {
        (
            format!("refverify:crossref:doi:{doi}"),
            format!("https://api.crossref.org/works/{}", pct(&doi)),
        )
    } else if let Some(title) = reference.title.as_deref().filter(|t| !t.is_empty()) {
        (
            format!("refverify:crossref:title:{}", sha256_hex(title)),
            format!("https://api.crossref.org/works?rows=1&query.bibliographic={}", pct(title)),
        )
    } else {
        return Ok(ConnectorOutcome::NotFound);
    };

    let req = HttpRequest::get(url).header("User-Agent", CROSSREF_UA);
    match cached_fetch(ctx, "crossref", &cache_key, TTL_EXISTENCE, req, now)? {
        Fetched::RateLimited { retry_after_secs } => Ok(ConnectorOutcome::RateLimited { retry_after_secs }),
        Fetched::HttpStatus { status: 404 } => Ok(ConnectorOutcome::NotFound),
        Fetched::HttpStatus { status } => Ok(ConnectorOutcome::Unavailable { detail: format!("crossref http {status}") }),
        Fetched::Body { body, provenance } => {
            let v = json(&body, "crossref")?;
            let msg = &v["message"];
            // /works/{doi} -> the work is `message`; query -> `message.items[0]`.
            let work = if msg.get("items").is_some() { &msg["items"][0] } else { msg };
            if work.is_null() {
                return Ok(ConnectorOutcome::NotFound);
            }
            let doi = work["DOI"].as_str().map(|s| s.to_string());
            let title = work["title"][0]
                .as_str()
                .map(|t| UntrustedText::new(t, provenance.clone()));
            if doi.is_none() && title.is_none() {
                return Ok(ConnectorOutcome::NotFound);
            }
            let matched_authors =
                crossref_authors(work).map(|a| UntrustedText::new(a, provenance.clone()));
            let matched_year = crossref_year(work);
            Ok(ConnectorOutcome::Found(ExistenceCheck {
                source: "crossref",
                found: true,
                doi,
                title,
                matched_authors,
                matched_year,
                is_retracted_hint: None,
                provenance,
            }))
        }
    }
}

/// OpenAlex — existence + inline `is_retracted`, by DOI or title search.
pub fn openalex_lookup(
    ctx: &VerifyContext,
    reference: &Reference,
    now: i64,
) -> Result<ConnectorOutcome<ExistenceCheck>, GaplyError> {
    let (cache_key, url) = if let Some(doi) = normalized_doi(reference) {
        (
            format!("refverify:openalex:doi:{doi}"),
            format!("https://api.openalex.org/works/doi:{}", pct(&doi)),
        )
    } else if let Some(title) = reference.title.as_deref().filter(|t| !t.is_empty()) {
        (
            format!("refverify:openalex:title:{}", sha256_hex(title)),
            format!("https://api.openalex.org/works?per-page=1&filter=title.search:{}", pct(title)),
        )
    } else {
        return Ok(ConnectorOutcome::NotFound);
    };

    let req = HttpRequest::get(url);
    match cached_fetch(ctx, "openalex", &cache_key, TTL_EXISTENCE, req, now)? {
        Fetched::RateLimited { retry_after_secs } => Ok(ConnectorOutcome::RateLimited { retry_after_secs }),
        Fetched::HttpStatus { status: 404 } => Ok(ConnectorOutcome::NotFound),
        Fetched::HttpStatus { status } => Ok(ConnectorOutcome::Unavailable { detail: format!("openalex http {status}") }),
        Fetched::Body { body, provenance } => {
            let v = json(&body, "openalex")?;
            // DOI form returns the work object; search form returns {results:[...]}.
            let work = if v.get("results").is_some() { &v["results"][0] } else { &v };
            if work.is_null() || work["id"].is_null() {
                return Ok(ConnectorOutcome::NotFound);
            }
            let doi = work["doi"].as_str().map(|s| s.to_string());
            let title = work["title"].as_str().map(|t| UntrustedText::new(t, provenance.clone()));
            let matched_authors =
                openalex_authors(work).map(|a| UntrustedText::new(a, provenance.clone()));
            let matched_year =
                work["publication_year"].as_i64().and_then(|y| i32::try_from(y).ok());
            Ok(ConnectorOutcome::Found(ExistenceCheck {
                source: "openalex",
                found: true,
                doi,
                title,
                matched_authors,
                matched_year,
                is_retracted_hint: work["is_retracted"].as_bool(),
                provenance,
            }))
        }
    }
}

/// Retraction Watch — retraction status for a DOI. Accepts the CrossRef-Labs
/// shape (`message.update-to[].type == "retraction"`) and a simple
/// `{retracted, reasons, notice_url}` shape. Requires a DOI.
pub fn retraction_watch_check(
    ctx: &VerifyContext,
    reference: &Reference,
    now: i64,
) -> Result<ConnectorOutcome<RetractionCheck>, GaplyError> {
    let Some(doi) = normalized_doi(reference) else {
        return Ok(ConnectorOutcome::NotFound);
    };
    let cache_key = format!("refverify:retraction_watch:doi:{doi}");
    let url = format!("https://api.labs.crossref.org/works/{}", pct(&doi));
    let req = HttpRequest::get(url).header("User-Agent", CROSSREF_UA);

    match cached_fetch(ctx, "retraction_watch", &cache_key, TTL_RETRACTION, req, now)? {
        Fetched::RateLimited { retry_after_secs } => Ok(ConnectorOutcome::RateLimited { retry_after_secs }),
        Fetched::HttpStatus { status: 404 } => Ok(ConnectorOutcome::NotFound),
        Fetched::HttpStatus { status } => Ok(ConnectorOutcome::Unavailable { detail: format!("retraction_watch http {status}") }),
        Fetched::Body { body, provenance } => {
            let v = json(&body, "retraction_watch")?;
            let mut retracted = v["retracted"].as_bool().unwrap_or(false);
            let mut reasons: Vec<UntrustedText> = Vec::new();
            let mut notice_url: Option<String> = None;

            // simple shape
            if let Some(arr) = v["reasons"].as_array() {
                for r in arr {
                    if let Some(s) = r.as_str() {
                        reasons.push(UntrustedText::new(s, provenance.clone()));
                    }
                }
            }
            notice_url = notice_url.or_else(|| v["notice_url"].as_str().map(|s| s.to_string()));

            // CrossRef-Labs shape: message.update-to[] with type "retraction"
            if let Some(updates) = v["message"]["update-to"].as_array() {
                for u in updates {
                    let is_retraction = u["type"].as_str().map(|t| t.eq_ignore_ascii_case("retraction")).unwrap_or(false);
                    if is_retraction {
                        retracted = true;
                        if let Some(label) = u["label"].as_str() {
                            reasons.push(UntrustedText::new(label, provenance.clone()));
                        }
                        notice_url = notice_url.or_else(|| u["DOI"].as_str().map(|d| format!("https://doi.org/{d}")));
                    }
                }
            }

            Ok(ConnectorOutcome::Found(RetractionCheck { retracted, reasons, notice_url, provenance }))
        }
    }
}

/// Unpaywall — open-access locations for a DOI. Needs a contact email (not a
/// secret; Unpaywall requires it in the query).
pub fn unpaywall_open_access(
    ctx: &VerifyContext,
    reference: &Reference,
    now: i64,
) -> Result<ConnectorOutcome<OpenAccess>, GaplyError> {
    let Some(doi) = normalized_doi(reference) else {
        return Ok(ConnectorOutcome::NotFound);
    };
    let Some(email) = ctx.contact_email.filter(|e| !e.is_empty()) else {
        return Ok(ConnectorOutcome::Unavailable { detail: "unpaywall requires a contact email".into() });
    };
    let cache_key = format!("refverify:unpaywall:doi:{doi}");
    let url = format!("https://api.unpaywall.org/v2/{}?email={}", pct(&doi), pct(email));
    let req = HttpRequest::get(url);

    match cached_fetch(ctx, "unpaywall", &cache_key, TTL_OA, req, now)? {
        Fetched::RateLimited { retry_after_secs } => Ok(ConnectorOutcome::RateLimited { retry_after_secs }),
        Fetched::HttpStatus { status: 404 } => Ok(ConnectorOutcome::NotFound),
        Fetched::HttpStatus { status } => Ok(ConnectorOutcome::Unavailable { detail: format!("unpaywall http {status}") }),
        Fetched::Body { body, provenance } => {
            let v = json(&body, "unpaywall")?;
            let is_oa = v["is_oa"].as_bool().unwrap_or(false);
            let best = &v["best_oa_location"];
            let best_url = best["url_for_pdf"].as_str().or_else(|| best["url"].as_str()).map(|s| s.to_string());
            let license = best["license"].as_str().map(|s| s.to_string());
            Ok(ConnectorOutcome::Found(OpenAccess { is_oa, best_url, license, provenance }))
        }
    }
}

/// Semantic Scholar — metadata enrichment for a DOI (citation counts, abstract).
/// The abstract is prime injection bait, so it is wrapped in [`UntrustedText`].
pub fn semantic_scholar_enrich(
    ctx: &VerifyContext,
    reference: &Reference,
    now: i64,
) -> Result<ConnectorOutcome<Enrichment>, GaplyError> {
    let Some(doi) = normalized_doi(reference) else {
        return Ok(ConnectorOutcome::NotFound);
    };
    let cache_key = format!("refverify:semantic_scholar:doi:{doi}");
    let fields = "title,abstract,venue,year,citationCount,influentialCitationCount";
    let url = format!("https://api.semanticscholar.org/graph/v1/paper/DOI:{}?fields={}", pct(&doi), pct(fields));
    let req = HttpRequest::get(url);

    match cached_fetch(ctx, "semantic_scholar", &cache_key, TTL_ENRICH, req, now)? {
        Fetched::RateLimited { retry_after_secs } => Ok(ConnectorOutcome::RateLimited { retry_after_secs }),
        Fetched::HttpStatus { status: 404 } => Ok(ConnectorOutcome::NotFound),
        Fetched::HttpStatus { status } => Ok(ConnectorOutcome::Unavailable { detail: format!("semantic_scholar http {status}") }),
        Fetched::Body { body, provenance } => {
            let v = json(&body, "semantic_scholar")?;
            if v["paperId"].is_null() && v["citationCount"].is_null() && v["abstract"].is_null() {
                return Ok(ConnectorOutcome::NotFound);
            }
            let abstract_text = v["abstract"].as_str().map(|a| UntrustedText::new(a, provenance.clone()));
            let venue = v["venue"].as_str().filter(|s| !s.is_empty()).map(|s| UntrustedText::new(s, provenance.clone()));
            Ok(ConnectorOutcome::Found(Enrichment {
                citation_count: v["citationCount"].as_i64(),
                influential_citation_count: v["influentialCitationCount"].as_i64(),
                abstract_text,
                venue,
                provenance,
            }))
        }
    }
}

// ============================================================================
// Orchestrator
// ============================================================================

/// Aggregate verification of one reference across all connectors.
#[derive(Debug, serde::Serialize)]
pub struct ReferenceVerification {
    pub reference_raw: String,
    pub exists: Option<ExistenceCheck>,
    pub retraction: Option<RetractionCheck>,
    pub open_access: Option<OpenAccess>,
    pub enrichment: Option<Enrichment>,
    /// Provenance for every source actually consulted (in call order).
    pub provenance: Vec<Provenance>,
    /// Human-readable notes: rate-limits, unavailability, injection flags.
    pub warnings: Vec<String>,
}

impl ReferenceVerification {
    fn new(reference_raw: String) -> Self {
        Self {
            reference_raw,
            exists: None,
            retraction: None,
            open_access: None,
            enrichment: None,
            provenance: Vec::new(),
            warnings: Vec::new(),
        }
    }
    /// True only if a source positively matched the reference.
    pub fn verified_exists(&self) -> bool {
        self.exists.as_ref().map_or(false, |e| e.found)
    }
    /// True if any source reports the work retracted.
    pub fn is_retracted(&self) -> bool {
        self.retraction.as_ref().map_or(false, |r| r.retracted)
            || self.exists.as_ref().and_then(|e| e.is_retracted_hint).unwrap_or(false)
    }
    fn note_untrusted(&mut self, label: &str, ut: &Option<UntrustedText>) {
        if let Some(t) = ut {
            self.provenance.push(t.provenance().clone());
            if t.is_suspicious() {
                self.warnings.push(format!("{label} flagged possible injection: {:?}", t.injection_flags()));
            }
        }
    }
}

/// Verify one reference: existence (CrossRef, falling back to OpenAlex),
/// retraction (Retraction Watch + OpenAlex hint), open access (Unpaywall), and
/// enrichment (Semantic Scholar). Never panics on connector trouble — issues are
/// recorded as warnings and the rest of the checks still run.
pub fn verify_reference(
    ctx: &VerifyContext,
    reference: &Reference,
    now: i64,
) -> Result<ReferenceVerification, GaplyError> {
    let mut report = ReferenceVerification::new(reference.raw.clone());

    // --- existence: CrossRef, then OpenAlex as a fallback -------------------
    match crossref_lookup(ctx, reference, now)? {
        ConnectorOutcome::Found(c) => {
            report.provenance.push(c.provenance.clone());
            report.note_untrusted("crossref.title", &c.title);
            report.exists = Some(c);
        }
        ConnectorOutcome::NotFound => report.warnings.push("crossref: no match".into()),
        ConnectorOutcome::RateLimited { retry_after_secs } => {
            report.warnings.push(format!("crossref: rate-limited, retry after {retry_after_secs}s"))
        }
        ConnectorOutcome::Unavailable { detail } => report.warnings.push(detail),
    }
    if !report.verified_exists() {
        match openalex_lookup(ctx, reference, now)? {
            ConnectorOutcome::Found(c) => {
                report.provenance.push(c.provenance.clone());
                report.note_untrusted("openalex.title", &c.title);
                report.exists = Some(c);
            }
            ConnectorOutcome::NotFound => report.warnings.push("openalex: no match".into()),
            ConnectorOutcome::RateLimited { retry_after_secs } => {
                report.warnings.push(format!("openalex: rate-limited, retry after {retry_after_secs}s"))
            }
            ConnectorOutcome::Unavailable { detail } => report.warnings.push(detail),
        }
    }

    // --- retraction ---------------------------------------------------------
    match retraction_watch_check(ctx, reference, now)? {
        ConnectorOutcome::Found(r) => {
            report.provenance.push(r.provenance.clone());
            for reason in &r.reasons {
                if reason.is_suspicious() {
                    report.warnings.push(format!("retraction reason flagged possible injection: {:?}", reason.injection_flags()));
                }
            }
            report.retraction = Some(r);
        }
        ConnectorOutcome::NotFound => {} // not in the retraction DB = not retracted
        ConnectorOutcome::RateLimited { retry_after_secs } => {
            report.warnings.push(format!("retraction_watch: rate-limited, retry after {retry_after_secs}s"))
        }
        ConnectorOutcome::Unavailable { detail } => report.warnings.push(detail),
    }

    // --- open access --------------------------------------------------------
    match unpaywall_open_access(ctx, reference, now)? {
        ConnectorOutcome::Found(oa) => {
            report.provenance.push(oa.provenance.clone());
            report.open_access = Some(oa);
        }
        ConnectorOutcome::NotFound => {}
        ConnectorOutcome::RateLimited { retry_after_secs } => {
            report.warnings.push(format!("unpaywall: rate-limited, retry after {retry_after_secs}s"))
        }
        ConnectorOutcome::Unavailable { detail } => report.warnings.push(detail),
    }

    // --- enrichment ---------------------------------------------------------
    match semantic_scholar_enrich(ctx, reference, now)? {
        ConnectorOutcome::Found(e) => {
            report.provenance.push(e.provenance.clone());
            report.note_untrusted("semantic_scholar.abstract", &e.abstract_text);
            report.note_untrusted("semantic_scholar.venue", &e.venue);
            report.enrichment = Some(e);
        }
        ConnectorOutcome::NotFound => {}
        ConnectorOutcome::RateLimited { retry_after_secs } => {
            report.warnings.push(format!("semantic_scholar: rate-limited, retry after {retry_after_secs}s"))
        }
        ConnectorOutcome::Unavailable { detail } => report.warnings.push(detail),
    }

    Ok(report)
}

#[cfg(test)]
mod tests;
