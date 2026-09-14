//! **Fetching the comparable corpus from OpenAlex.**
//!
//! Public metadata about published papers. **No user manuscript is involved and
//! none can be** — the request carries a journal id and a date, and
//! [`PublishedPaper`] has no field that could hold one.
//!
//! Lives in the app crate for the same reason `ReqwestFetcher` does: gaply-core
//! stays network-free, and the derivation
//! ([`gaply_core::journal_corpus::derive_conventions`]) is pure so it can be
//! tested without a socket.
//!
//! # Two things about the request that are not incidental
//!
//! **`mailto` in the User-Agent.** OpenAlex asks for it and routes identified
//! traffic to a faster pool; it is also the honest thing for a tool making
//! bulk requests of a free public service.
//!
//! **`from_publication_date`, and it is the only date there is.** OpenAlex
//! carries no acceptance date, so the corpus is *recently published* and the
//! output says so — `journal_corpus`'s tests pin that "accepted" never appears.

use gaply_core::journal_corpus::PublishedPaper;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::GaplyError;

const API: &str = "https://api.openalex.org";

/// Reconstruct an abstract from OpenAlex's inverted index.
///
/// The API gives `{"word": [positions]}` and never the string. A reconstruction
/// that dropped the positions would produce a bag of words that still READS as
/// an abstract, which is worse than none.
fn abstract_from_inverted(v: &serde_json::Value) -> String {
    let Some(map) = v.as_object() else { return String::new() };
    let mut slots: Vec<(u64, &str)> = Vec::new();
    for (word, positions) in map {
        let Some(arr) = positions.as_array() else { continue };
        for p in arr {
            if let Some(i) = p.as_u64() {
                slots.push((i, word.as_str()));
            }
        }
    }
    slots.sort_by_key(|(i, _)| *i);
    slots.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" ")
}

/// Resolve a journal to its OpenAlex source id by ISSN.
pub fn source_id_for_issn(fetcher: &dyn HttpFetcher, issn: &str) -> Result<String, GaplyError> {
    let resp = fetcher.get(&HttpRequest::get(&format!("{API}/sources/issn:{issn}")))?;
    if resp.status != 200 {
        return Err(GaplyError::Validation(format!("openalex sources: http {}", resp.status)));
    }
    let v: serde_json::Value = serde_json::from_str(&resp.body)
        .map_err(|e| GaplyError::Internal(format!("openalex sources json: {e}")))?;
    v.get("id")
        .and_then(|i| i.as_str())
        .and_then(|i| i.rsplit('/').next())
        .map(str::to_string)
        .ok_or_else(|| GaplyError::Validation(format!("no OpenAlex source for ISSN {issn}")))
}

/// Fetch recently published papers for a source, newest first, capped at
/// `maximum`.
pub fn recent_papers(
    fetcher: &dyn HttpFetcher,
    source_id: &str,
    from_date: &str,
    maximum: usize,
) -> Result<Vec<PublishedPaper>, GaplyError> {
    let per_page = maximum.min(200).max(1);
    let url = format!(
        "{API}/works?filter=primary_location.source.id:{source_id},from_publication_date:{from_date}\
         &sort=publication_date:desc&per-page={per_page}"
    );
    let resp = fetcher.get(&HttpRequest::get(&url))?;
    if resp.status != 200 {
        return Err(GaplyError::Validation(format!("openalex works: http {}", resp.status)));
    }
    let v: serde_json::Value = serde_json::from_str(&resp.body)
        .map_err(|e| GaplyError::Internal(format!("openalex works json: {e}")))?;
    let results = v.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    Ok(results.iter().map(parse_work).take(maximum).collect())
}

fn parse_work(w: &serde_json::Value) -> PublishedPaper {
    let biblio = w.get("biblio");
    let s = |k: &str| w.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let b = |k: &str| {
        biblio
            .and_then(|x| x.get(k))
            .and_then(|x| x.as_str())
            .filter(|x| !x.is_empty())
            .map(str::to_string)
    };
    PublishedPaper {
        id: s("id"),
        title: w.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_string(),
        abstract_text: w
            .get("abstract_inverted_index")
            .map(abstract_from_inverted)
            .unwrap_or_default(),
        publication_date: s("publication_date"),
        work_type: s("type"),
        first_page: b("first_page"),
        last_page: b("last_page"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::refverify::MockHttpFetcher;

    /// The inverted index must be reassembled IN ORDER. Dropping the positions
    /// gives a bag of words that still reads as an abstract, which is worse
    /// than having none.
    #[test]
    fn an_abstract_is_reconstructed_in_position_order() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"interval":[6],"confidence":[5],"was":[2],"The":[0],"1.8":[3],"ratio":[1],"95%":[4]}"#,
        )
        .unwrap();
        assert_eq!(abstract_from_inverted(&v), "The ratio was 1.8 95% confidence interval");
        assert_eq!(abstract_from_inverted(&serde_json::Value::Null), "");
    }

    #[test]
    fn a_work_without_an_abstract_parses_to_an_empty_one_not_a_failure() {
        // 37% of Nature Medicine's recent records have no abstract.
        let w: serde_json::Value = serde_json::from_str(
            r#"{"id":"https://openalex.org/W1","title":"T","publication_date":"2025-06-01",
                "type":"article","biblio":{"first_page":"1776","last_page":"1783"}}"#,
        )
        .unwrap();
        let p = parse_work(&w);
        assert_eq!(p.abstract_text, "");
        assert_eq!(p.page_count(), Some(8));
    }

    /// An article identifier survives parsing as the string it is, so
    /// `page_count` can refuse it rather than a parser inventing a number.
    #[test]
    fn an_article_identifier_page_is_carried_through_unparsed() {
        let w: serde_json::Value = serde_json::from_str(
            r#"{"id":"https://openalex.org/W2","title":"T","publication_date":"2025-06-01",
                "type":"article","biblio":{"first_page":"e0319586","last_page":"e0319586"}}"#,
        )
        .unwrap();
        let p = parse_work(&w);
        assert_eq!(p.first_page.as_deref(), Some("e0319586"));
        assert_eq!(p.page_count(), None);
    }

    #[test]
    fn a_non_200_is_an_honest_error_not_an_empty_corpus() {
        let f = MockHttpFetcher::new().route("openalex.org", 503, "");
        assert!(recent_papers(&f, "S1", "2025-01-01", 10).is_err());
        assert!(source_id_for_issn(&f, "1234-5678").is_err());
    }

    #[test]
    fn the_source_id_is_the_bare_identifier_not_the_url() {
        let f = MockHttpFetcher::new()
            .route("sources/issn", 200, r#"{"id":"https://openalex.org/S202381698"}"#);
        assert_eq!(source_id_for_issn(&f, "1932-6203").unwrap(), "S202381698");
    }
}
