//! Journal Verification — the combined orchestration (Set 4).
//!
//! THIN: it calls the two tested lanes and joins their results — no new engine
//! logic. The grounded registry facts ([`crate::journal_registry`], LLM-free)
//! and the self-reported site summary ([`crate::journal_site_summary`], the ONE
//! model use) are combined into one result. The page is fetched ONCE (a resolved
//! link is served from cache) and threaded to both. The registry verification
//! NEVER depends on the LLM lane — if the proxy/site is unavailable, the facts
//! still come back and the site summary degrades honestly. NO verdict.

use serde::Serialize;

use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::HttpFetcher;
use gaply_core::verify_agent::ProxyClient;
use gaply_core::{Database, GaplyError};

use crate::journal_registry::{
    fetch_site_body, resolve_link_issn, resolve_name, verify_journal, JournalMatch, JournalVerification,
};
use crate::journal_site_summary::{summarize_site, SiteSummary};

/// The combined result: grounded registry facts + labeled self-reported site
/// summary + resolution/disambiguation state. Evidence + signals, NOT a verdict.
#[derive(Debug, Clone, Serialize)]
pub struct JournalVerificationResult {
    /// What the user entered (echoed).
    pub input: String,
    /// "name" | "link" | "issn".
    pub input_kind: &'static str,
    /// Multiple name matches → the user disambiguates; registry NOT run yet
    /// (never silently picked). Empty otherwise.
    pub disambiguation: Vec<JournalMatch>,
    /// True when we couldn't locate the journal at all (no ISSN resolved) — the
    /// honest not-found alert (a warning sign, but not proof of predatory).
    pub not_found: bool,
    /// Grounded registry facts (Set 2) — present once resolved to one ISSN.
    pub registry: Option<JournalVerification>,
    /// Self-reported site details (Set 3), labeled — present when a link (and
    /// the proxy) were available; honestly degraded otherwise.
    pub site_summary: Option<SiteSummary>,
    /// Honest orchestration notes (couldn't resolve, no site URL for a name, …).
    pub notes: Vec<String>,
}

const NEED_LINK_NOTE: &str =
    "Provide the journal's website link to also check its self-reported details (APC, timeline, contact).";

/// Orchestrate the two lanes into one combined result. `issn` (when present) is
/// the direct/post-disambiguation path — skips resolution. Otherwise `query` is
/// a name or a link.
pub fn verify_journal_full(
    db: &Database,
    fetcher: &dyn HttpFetcher,
    limiter: &RateLimiter,
    proxy: Option<&dyn ProxyClient>,
    query: Option<&str>,
    issn: Option<&str>,
    local_predatory_signals: &[String],
) -> Result<JournalVerificationResult, GaplyError> {
    // Direct ISSN (e.g. the user picked a disambiguation match).
    if let Some(issn) = issn.map(str::trim).filter(|s| !s.is_empty()) {
        let registry = verify_journal(db, fetcher, limiter, issn, "", local_predatory_signals)?;
        return Ok(JournalVerificationResult {
            input: issn.to_string(),
            input_kind: "issn",
            disambiguation: Vec::new(),
            not_found: false,
            registry: Some(registry),
            site_summary: None,
            notes: vec![NEED_LINK_NOTE.to_string()],
        });
    }

    let query = query.map(str::trim).unwrap_or("");
    if query.is_empty() {
        return Err(GaplyError::Validation("enter a journal name, ISSN, or link".into()));
    }
    let mut notes = Vec::new();

    if query.starts_with("http://") || query.starts_with("https://") {
        // ---- LINK: resolve ISSN from the page + summarize the same page ----
        let link = resolve_link_issn(db, fetcher, limiter, query)?;
        notes.extend(link.unverified.clone());
        // fetch-once: resolve_link_issn already fetched+cached the page; this is
        // a cache hit (same journal:site:{url} key), so no second network call.
        let body = fetch_site_body(db, fetcher, limiter, query);

        let registry = match link.issns.first() {
            Some(issn) => Some(verify_journal(db, fetcher, limiter, issn, "", local_predatory_signals)?),
            None => None,
        };
        // the ONE LLM use — always attempted with the already-fetched body
        let site_summary = Some(summarize_site(proxy, query, body.as_deref()));
        let not_found = registry.is_none();
        Ok(JournalVerificationResult {
            input: query.to_string(),
            input_kind: "link",
            disambiguation: Vec::new(),
            not_found,
            registry,
            site_summary,
            notes,
        })
    } else {
        // ---- NAME: resolve to ISSN(s); multiple → disambiguate, don't pick ----
        let res = resolve_name(db, fetcher, limiter, query)?;
        notes.extend(res.unverified.clone());

        if res.matches.len() > 1 {
            return Ok(JournalVerificationResult {
                input: query.to_string(),
                input_kind: "name",
                disambiguation: res.matches,
                not_found: false,
                registry: None,
                site_summary: None,
                notes,
            });
        }

        let registry = res
            .matches
            .first()
            .and_then(|m| m.issn.clone())
            .map(|issn| verify_journal(db, fetcher, limiter, &issn, query, local_predatory_signals))
            .transpose()?;
        if registry.is_some() {
            notes.push(NEED_LINK_NOTE.to_string());
        }
        let not_found = registry.is_none() && res.matches.is_empty();
        Ok(JournalVerificationResult {
            input: query.to_string(),
            input_kind: "name",
            disambiguation: Vec::new(),
            not_found,
            registry,
            // a name lookup has no site URL — the self-reported lane needs a link.
            site_summary: None,
            notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal_site_summary::SiteSummaryStatus;
    use gaply_core::refverify::MockHttpFetcher;
    use gaply_core::verify_agent::MockProxyClient;
    use serde_json::json;
    use std::sync::Arc;

    fn db() -> Arc<Database> {
        Arc::new(Database::in_memory().unwrap())
    }
    fn wide() -> RateLimiter {
        RateLimiter::new(100.0, 100.0)
    }
    fn oa_active() -> &'static str {
        r#"{"results":[{"display_name":"Journal of Sleep Research","issn_l":"1365-2869","is_in_doaj":true,
        "counts_by_year":[{"year":3000,"works_count":180}]}]}"#
    }
    const SITE: &str =
        "APC: authors pay $1200. Reviews within 3 weeks. Contact editor@journal.example.org.";
    fn site_response() -> serde_json::Value {
        json!({ "apc": "$1200", "review_timeline": "within 3 weeks",
                "guidelines_link": "not stated", "contact": "editor@journal.example.org" })
    }

    #[test]
    fn link_combines_registry_facts_and_labeled_site_summary() {
        // the SITE page carries the ISSN so link→ISSN resolves + the same body
        // feeds the summary.
        let f = MockHttpFetcher::new()
            .route("journal.example.org", 200, &format!("{SITE} ISSN 1365-2869"))
            .route("api.openalex.org", 200, oa_active())
            .route("doaj.org", 200, r#"{"total":1,"results":[{}]}"#)
            .route("eutils.ncbi.nlm.nih.gov", 200, r#"{"esearchresult":{"count":"1"}}"#);
        let proxy = MockProxyClient::returning(site_response());
        let r = verify_journal_full(&db(), &f, &wide(), Some(&proxy), Some("https://journal.example.org/about"), None, &[]).unwrap();

        assert_eq!(r.input_kind, "link");
        // grounded registry facts present
        let reg = r.registry.expect("registry facts");
        assert_eq!(reg.doaj_registered, Some(true));
        assert_eq!(reg.pubmed_indexed, Some(true));
        // labeled self-reported site summary present
        let ss = r.site_summary.expect("site summary");
        assert_eq!(ss.status, SiteSummaryStatus::Summarized);
        assert!(ss.label.contains("self-reported"));
        assert!(ss.details.iter().any(|d| d.field == "apc" && d.value.as_deref() == Some("$1200")));
        assert!(!r.not_found);
    }

    #[test]
    fn site_page_is_fetched_once_and_threaded_to_both() {
        let f = MockHttpFetcher::new()
            .route("journal.example.org", 200, &format!("{SITE} ISSN 1365-2869"))
            .route("api.openalex.org", 200, oa_active())
            .route("doaj.org", 200, r#"{"total":1,"results":[{}]}"#)
            .route("eutils.ncbi.nlm.nih.gov", 200, r#"{"esearchresult":{"count":"1"}}"#);
        let proxy = MockProxyClient::returning(site_response());
        verify_journal_full(&db(), &f, &wide(), Some(&proxy), Some("https://journal.example.org/about"), None, &[]).unwrap();
        // the SITE url appears exactly ONCE in the fetch log (resolve fetched it;
        // fetch_site_body hit the cache) — no double fetch.
        let site_calls = f.calls().iter().filter(|u| u.contains("journal.example.org/about")).count();
        assert_eq!(site_calls, 1, "site body fetched once, reused from cache");
    }

    #[test]
    fn multiple_name_matches_are_returned_for_disambiguation() {
        let body = r#"{"results":[
            {"display_name":"Advances in Science","issn_l":"1111-1111"},
            {"display_name":"Advances in Science and Tech","issn_l":"2222-2222"}
        ]}"#;
        let f = MockHttpFetcher::new().route("api.openalex.org", 200, body);
        let r = verify_journal_full(&db(), &f, &wide(), None, Some("advances in science"), None, &[]).unwrap();
        assert_eq!(r.disambiguation.len(), 2, "both returned; not silently picked");
        assert!(r.registry.is_none());
        assert!(!r.not_found);
    }

    #[test]
    fn registry_facts_survive_when_the_proxy_is_unavailable() {
        let f = MockHttpFetcher::new()
            .route("journal.example.org", 200, &format!("{SITE} ISSN 1365-2869"))
            .route("api.openalex.org", 200, oa_active())
            .route("doaj.org", 200, r#"{"total":1,"results":[{}]}"#)
            .route("eutils.ncbi.nlm.nih.gov", 200, r#"{"esearchresult":{"count":"1"}}"#);
        // proxy None (offline) — the registry facts must STILL come back
        let r = verify_journal_full(&db(), &f, &wide(), None, Some("https://journal.example.org/about"), None, &[]).unwrap();
        assert!(r.registry.is_some(), "grounded facts never depend on the LLM lane");
        assert_eq!(r.site_summary.as_ref().unwrap().status, SiteSummaryStatus::LlmUnavailable);
        assert!(r.site_summary.as_ref().unwrap().notice.as_ref().unwrap().contains("registry checks above are unaffected"));
    }

    #[test]
    fn name_no_match_is_the_honest_not_found() {
        let f = MockHttpFetcher::new().route("api.openalex.org", 200, r#"{"results":[]}"#);
        let r = verify_journal_full(&db(), &f, &wide(), None, Some("nonexistent journal zzz"), None, &[]).unwrap();
        assert!(r.not_found, "not found in the registries — the honest alert");
        assert!(r.registry.is_none());
    }

    #[test]
    fn direct_issn_path_skips_resolution() {
        let f = MockHttpFetcher::new()
            .route("api.openalex.org", 200, oa_active())
            .route("doaj.org", 200, r#"{"total":1,"results":[{}]}"#)
            .route("eutils.ncbi.nlm.nih.gov", 200, r#"{"esearchresult":{"count":"1"}}"#);
        let r = verify_journal_full(&db(), &f, &wide(), None, None, Some("1365-2869"), &[]).unwrap();
        assert_eq!(r.input_kind, "issn");
        assert!(r.registry.is_some());
        assert!(r.notes.iter().any(|n| n.contains("website link")));
    }

    #[test]
    fn empty_input_is_a_clear_error() {
        let f = MockHttpFetcher::new();
        assert!(verify_journal_full(&db(), &f, &wide(), None, Some("   "), None, &[]).is_err());
        assert!(verify_journal_full(&db(), &f, &wide(), None, None, None, &[]).is_err());
    }
}
