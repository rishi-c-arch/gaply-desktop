//! Fetching an open-access full text for a citation — the bytes half.
//!
//! [`gaply_core::oa_fetch`] decides WHAT may be fetched from a DOI; this module
//! performs the download and turns the result into something a citation check
//! can retrieve from. It lives in the app crate for the same reason every other
//! transport does: `gaply-core` is network-free, and `cargo test -p gaply_core`
//! links no TLS stack.
//!
//! # The third permitted network operation (plan §1, R4)
//!
//! R4 says nothing leaves the machine, with named exceptions. This is the third
//! and last: the embedding-model download, the generative-model download, and
//! now this. It qualifies on the same terms the other two do —
//!
//! * **ID-only outbound.** The request carries a DOI. Not the manuscript, not
//!   the sentence being checked, not a title, not the user. A DOI identifies a
//!   published work, not the person holding it.
//! * **Explicit user action.** There is no scheduler, no prefetch and no
//!   "while you were away". A person presses a button naming what it will do.
//! * **Never at startup.** Asserted, not asserted-to: see the startup test at
//!   the foot of this file, which mirrors `gen_startup_tests`.
//!
//! # What lands on disk, and why it is a file rather than a blob
//!
//! A fetched PDF is written under `<app data>/oa_papers/` before it is indexed,
//! and the document row points at that path. That is not incidental storage:
//! the Document row's viewer, "Open document" and "Reveal in Finder" all read a
//! real path, and `ai_document_source` reports whether the file is still there.
//! A blob in the database would have made every one of those surfaces lie.
//!
//! An abstract lands the same way, as a `.txt`, for exactly the same reason — a
//! document whose file does not exist renders as "file not found — it was moved
//! or renamed since indexing", which would be a false statement about a
//! document that was never on disk in the first place.

use std::path::{Path, PathBuf};

use gaply_core::ai_engine::store;
use gaply_core::extract::citations::Reference;
use gaply_core::oa_fetch::{resolve, OaResolution, OaSource};
use gaply_core::refverify::{ApiRateLimiters, HttpFetcher, VerifyContext};
use gaply_core::{Database, GaplyError};
use serde::Serialize;

use crate::paper_corpus::{PaperFetch, MAX_FETCH_BYTES};

/// What happened to ONE source. Reported per-citation because a batch of twelve
/// will not have one answer, and collapsing twelve outcomes into "8 succeeded"
/// throws away the four sentences the user actually needs to read.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum FetchOutcome {
    /// Full text downloaded, indexed, embedded and linked.
    Fetched {
        document_id: i64,
        chunks_indexed: usize,
        chunks_embedded: usize,
        /// Whether a support check can actually run against it now.
        checkable: bool,
        source: OaSource,
        license: Option<String>,
    },
    /// No full text, but an abstract was stored — and flagged as one.
    AbstractOnly {
        document_id: i64,
        chunks_indexed: usize,
        chunks_embedded: usize,
        checkable: bool,
        source: OaSource,
        /// The firewall found and redacted injection markers in the abstract.
        injection_flagged: bool,
    },
    /// The record exists and says there is no free copy.
    Paywalled { detail: String },
    /// Nothing free and nothing to summarise — including "no DOI" and "no index
    /// has heard of this DOI", which are honest reasons, not failures.
    NoOaCopy { detail: String },
    /// A limiter refused. Retryable, and NOT evidence of absence.
    RateLimited { retry_after_secs: u64 },
    /// The copy WAS found and downloaded, and the import guard declined it —
    /// too large, or a scan with no text layer. A distinct arm from `failed`
    /// because nothing went wrong: the fetch worked and the file is the
    /// problem, which is a different thing for the user to do something about.
    NotImportable { detail: String },
    /// The lookup or the download itself failed. The only arm that is a fault.
    Failed { detail: String },
    /// Already linked to an indexed document — nothing was fetched, and no
    /// request was made. Reported rather than silently skipped: "I did nothing
    /// because you already have it" is a different sentence from "I did it".
    AlreadyLinked { document_id: i64 },
}

/// One citation's identity, as the fetch needs it. Assembled by the caller from
/// `citation_library` so this module never queries for presentation data.
#[derive(Debug, Clone)]
pub struct FetchTarget {
    pub citation_id: String,
    pub doi: Option<String>,
    pub title: Option<String>,
}

/// Per-source result, carrying the citation it belongs to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReport {
    pub citation_id: String,
    /// The title as the library holds it — so a batch report reads as a list of
    /// papers rather than a list of opaque ids.
    pub title: Option<String>,
    #[serde(flatten)]
    pub outcome: FetchOutcome,
}

/// Where fetched sources live. One directory, created on demand.
pub fn oa_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("oa_papers")
}

/// A DOI reduced to a safe file stem. Every non-alphanumeric byte becomes `-`,
/// so a DOI's slashes and dots cannot walk out of `oa_papers/`.
fn doi_stem(doi: &str) -> String {
    doi.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// Everything the fetch needs that touches the outside world. Grouped so the
/// per-citation loop takes one borrow rather than five.
pub struct FetchDeps<'a> {
    pub db: &'a Database,
    /// JSON transport for the two resolvers (text).
    pub http: &'a dyn HttpFetcher,
    /// Bytes transport for the PDF itself.
    pub bytes: &'a dyn PaperFetch,
    pub limiters: &'a ApiRateLimiters,
    /// Unpaywall requires one; `None` skips Unpaywall rather than sending a
    /// request it will refuse.
    pub contact_email: Option<&'a str>,
    pub app_data_dir: &'a Path,
}

/// Embed a document's pending chunks, returning how many were written.
///
/// Takes the embedding closure rather than the engine so tests can drive the
/// whole path — including "indexed but not embedded", which is the state that
/// decides whether a check can run — without a model on disk.
pub type EmbedFn<'a> = &'a dyn Fn(&[String]) -> Result<Vec<Vec<f32>>, GaplyError>;

/// Index a document's text, embed it, and link it to the citation by DOI.
///
/// Shared by both storing paths (full text and abstract) because the difference
/// between them is what was written to disk, not what happens afterwards.
fn index_embed_and_link(
    deps: &FetchDeps,
    citation_id: &str,
    document_id: i64,
    blocks: &[gaply_core::extract::docparse::PagedBlock],
    embed: EmbedFn,
) -> Result<(usize, usize, bool), GaplyError> {
    let chunks = gaply_core::chunk::chunk_paged_default(blocks);
    let indexed = store::index_chunks(deps.db, document_id, &chunks)?.inserted;

    let space = crate::ai::embedding_space();
    let pending = gaply_core::ai_engine::embeddings::chunks_missing_embeddings(
        deps.db,
        Some(document_id),
        &space.model_id,
    )?;
    let mut embedded = 0usize;
    for batch in pending.chunks(crate::ai::EMBED_BATCH_SIZE) {
        let texts: Vec<String> = batch.iter().map(|p| p.content.clone()).collect();
        let vectors = embed(&texts)?;
        let rows: Vec<(i64, Vec<f32>)> =
            batch.iter().map(|p| p.chunk_id).zip(vectors).collect();
        gaply_core::ai_engine::embeddings::put_embeddings(deps.db, &space, &rows)?;
        embedded += rows.len();
    }

    // Linked even when embedding fell short: the link is a true statement about
    // which file backs this citation either way, and `checkable` — asked of the
    // store, not inferred here — is what says whether a check can run.
    gaply_core::citation_links::link_by_doi(deps.db, citation_id, document_id)?;
    let checkable =
        gaply_core::citation_links::checkable_document_for_citation(deps.db, citation_id)?
            .is_some();
    Ok((indexed, embedded, checkable))
}

/// Fetch, store and link one citation's open-access full text.
///
/// Never panics on a per-source problem: every failure mode is one of
/// [`FetchOutcome`]'s arms, so a batch of twelve reports twelve answers rather
/// than stopping at the first paper whose publisher returned a 403.
pub fn fetch_one(deps: &FetchDeps, target: &FetchTarget, now: i64, embed: EmbedFn) -> FetchReport {
    let outcome = fetch_one_inner(deps, target, now, embed)
        .unwrap_or_else(|e| FetchOutcome::Failed { detail: e.to_string() });
    FetchReport {
        citation_id: target.citation_id.clone(),
        title: target.title.clone(),
        outcome,
    }
}

fn fetch_one_inner(
    deps: &FetchDeps,
    target: &FetchTarget,
    now: i64,
    embed: EmbedFn,
) -> Result<FetchOutcome, GaplyError> {
    // Already have a usable copy? Then there is nothing to ask anyone. This is
    // the cheapest possible privacy win: the request that is never made.
    if let Some((document_id, _)) =
        gaply_core::citation_links::checkable_document_for_citation(deps.db, &target.citation_id)?
    {
        return Ok(FetchOutcome::AlreadyLinked { document_id });
    }

    let reference = Reference {
        raw: String::new(),
        authors: String::new(),
        year: None,
        title: target.title.clone(),
        doi: target.doi.clone(),
    };
    let ctx = VerifyContext {
        db: deps.db,
        http: deps.http,
        limiters: deps.limiters,
        contact_email: deps.contact_email,
    };

    match resolve(&ctx, &reference, now)? {
        OaResolution::Paywalled { detail } => Ok(FetchOutcome::Paywalled { detail }),
        OaResolution::NoOaCopy { detail } => Ok(FetchOutcome::NoOaCopy { detail }),
        OaResolution::RateLimited { retry_after_secs } => {
            Ok(FetchOutcome::RateLimited { retry_after_secs })
        }
        OaResolution::Unavailable { detail } => Ok(FetchOutcome::Failed { detail }),

        OaResolution::FullText { pdf_url, license, source } => {
            let (status, bytes) = deps.bytes.get_capped(&pdf_url, MAX_FETCH_BYTES)?;
            if status != 200 {
                return Ok(FetchOutcome::Failed {
                    detail: format!("the open-access copy returned http {status}"),
                });
            }
            // Magic bytes, not the URL's extension and not the server's
            // Content-Type: a `.pdf` URL that answers with an HTML paywall
            // interstitial is a real and common shape, and indexing that page
            // would store a publisher's login form as the paper's evidence.
            if !bytes.starts_with(b"%PDF-") {
                return Ok(FetchOutcome::NoOaCopy {
                    detail: "the open-access link did not return a PDF (most likely a landing or \
                             login page), so nothing was stored"
                        .to_string(),
                });
            }

            let doi = target.doi.clone().unwrap_or_default();
            let dir = oa_dir(deps.app_data_dir);
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{}.pdf", doi_stem(&doi)));
            std::fs::write(&path, &bytes)?;

            // The SAME guard the manual link uses, on the same file, before a
            // `documents` row exists. A fetched PDF is not exempt from being a
            // 900-page scan, and a batch must not silently spend an hour on one.
            //
            // The confirm tier is reported rather than blocking: the user chose
            // N sources in one press, and stopping the batch to ask about the
            // fourth would strand the other eight. `notImportable` names it so
            // they can link it deliberately.
            let pre = gaply_core::import_guard::preflight(deps.db, &path, now)?;
            if pre.is_refused() || pre.needs_confirmation() {
                let _ = std::fs::remove_file(&path);
                return Ok(FetchOutcome::NotImportable { detail: pre.summary });
            }
            let started = std::time::Instant::now();

            let blocks = gaply_core::extract::docparse::parse_path_paged(&path)?;
            let title = target
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("Open-access copy of {doi}"));
            // Keyed on the DOI, so re-fetching the same work finds the same row
            // rather than growing a duplicate on every press.
            let checksum = format!("oa-{}", store::content_hash(&doi));
            let document_id =
                store::create_document(deps.db, &title, &path.display().to_string(), &checksum)?;

            let (chunks_indexed, chunks_embedded, checkable) =
                index_embed_and_link(deps, &target.citation_id, document_id, &blocks, embed)?;
            if chunks_embedded == chunks_indexed {
                let _ = gaply_core::import_guard::record_rate(
                    deps.db,
                    pre.page_equivalents,
                    started.elapsed().as_secs_f64(),
                    now,
                );
            }
            Ok(FetchOutcome::Fetched {
                document_id,
                chunks_indexed,
                chunks_embedded,
                checkable,
                source,
                license,
            })
        }

        OaResolution::AbstractOnly { safe_text, injection_flagged, source } => {
            let doi = target.doi.clone().unwrap_or_default();
            let dir = oa_dir(deps.app_data_dir);
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{}-abstract.txt", doi_stem(&doi)));
            // `safe_text` is what the resolver returned — already `llm_safe()`.
            // The raw abstract never reaches this module, so there is no way for
            // an un-firewalled string to be the thing that gets written.
            std::fs::write(&path, safe_text.as_bytes())?;

            let title = target
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("Abstract of {doi}"));
            let checksum = format!("oa-abstract-{}", store::content_hash(&doi));
            let document_id = store::create_abstract_document(
                deps.db,
                &title,
                &path.display().to_string(),
                &checksum,
            )?;

            let blocks = vec![gaply_core::extract::docparse::PagedBlock {
                page: Some(1),
                text: safe_text.clone(),
            }];
            let (chunks_indexed, chunks_embedded, checkable) =
                index_embed_and_link(deps, &target.citation_id, document_id, &blocks, embed)?;
            Ok(FetchOutcome::AbstractOnly {
                document_id,
                chunks_indexed,
                chunks_embedded,
                checkable,
                source,
                injection_flagged,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::refverify::MockHttpFetcher;
    use std::sync::Mutex;

    /// Bytes fetcher for tests: URL substring → (status, bytes).
    struct MockBytes {
        routes: Vec<(String, u16, Vec<u8>)>,
        calls: Mutex<Vec<String>>,
    }
    impl MockBytes {
        fn new() -> Self {
            Self { routes: Vec::new(), calls: Mutex::new(Vec::new()) }
        }
        fn route(mut self, m: &str, status: u16, body: Vec<u8>) -> Self {
            self.routes.push((m.to_string(), status, body));
            self
        }
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }
    impl PaperFetch for MockBytes {
        fn get_capped(&self, url: &str, _cap: usize) -> Result<(u16, Vec<u8>), GaplyError> {
            self.calls.lock().unwrap().push(url.to_string());
            for (m, s, b) in &self.routes {
                if url.contains(m.as_str()) {
                    return Ok((*s, b.clone()));
                }
            }
            Ok((404, Vec::new()))
        }
    }

    /// A deterministic stand-in for the embedding engine: 384 dims, content-
    /// derived so two different chunks get two different vectors. The point of
    /// these tests is the FETCH path, not the model.
    fn fake_embed(texts: &[String]) -> Result<Vec<Vec<f32>>, GaplyError> {
        Ok(texts
            .iter()
            .map(|t| {
                let seed = t.len() as f32;
                (0..384).map(|i| ((i as f32 + seed) % 7.0) / 7.0).collect()
            })
            .collect())
    }

    /// Seed a citation through the library's own public API — no raw SQL, so
    /// the test exercises the same rows the product writes.
    fn add_citation(db: &Database, id: &str, doi: Option<&str>, title: &str) {
        let csl = serde_json::json!({
            "title": title,
            "author": [{"family": "Naidu", "given": "Raji T"}],
            "issued": {"date-parts": [[2023]]},
        })
        .to_string();
        gaply_core::citation_library::upsert(
            db,
            id,
            &csl,
            doi,
            &[],
            &gaply_core::citation_library::VerificationWrite::default(),
        )
        .unwrap();
    }

    fn db_with_citation(doi: Option<&str>) -> Database {
        let db = Database::in_memory().unwrap();
        // `ai_chunk_embeddings.model_id` is a FOREIGN KEY into the registry, so
        // a vector cannot be written for a model the app has not installed.
        // Registering the same space the fetch will use is what the running app
        // has done by this point.
        let space = crate::ai::embedding_space();
        gaply_core::ai_engine::registry::register_model(
            &db,
            gaply_core::ai_engine::registry::ModelRow {
                id: space.model_id.clone(),
                kind: "embedding".to_string(),
                display_name: "test embedding space".to_string(),
                file_path: "/dev/null".to_string(),
                sha256: None,
                dim: Some(384),
                quant: None,
            },
        )
        .unwrap();
        add_citation(&db, "c1", doi, "Incidence of needlestick injury");
        db
    }

    fn target() -> FetchTarget {
        FetchTarget {
            citation_id: "c1".to_string(),
            doi: Some("10.4103/ijmr.ijmr_892_23".to_string()),
            title: Some("Incidence of needlestick injury".to_string()),
        }
    }

    /// A tiny but genuinely parseable PDF, built by the same helper the
    /// docparse tests use, so "it parsed" means the real parser accepted it.
    fn real_pdf_bytes() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/gaply-core/tests/fixtures/sample_text.pdf"
        ))
        .expect("the docparse PDF fixture must exist")
    }

    const UNPAYWALL_PDF: &str = r#"{"is_oa": true, "best_oa_location": {"url_for_pdf": "https://oa.example/paper.pdf", "license": "cc-by"}}"#;
    const UNPAYWALL_CLOSED: &str = r#"{"is_oa": false, "best_oa_location": {}}"#;
    const OPENALEX_CLOSED: &str =
        r#"{"id": "https://openalex.org/W1", "open_access": {"is_oa": false}, "best_oa_location": {}}"#;
    const OPENALEX_ABSTRACT: &str = r#"{
        "id": "https://openalex.org/W1",
        "open_access": {"is_oa": true},
        "best_oa_location": {"pdf_url": null},
        "abstract_inverted_index": {"Needlestick": [0], "injury": [1], "incidence": [2], "was": [3], "measured": [4]}
    }"#;

    /// A scratch app-data dir, removed on drop. Same shape as
    /// `citation_resolver`'s test helper — no new dev-dependency for it.
    struct Harness {
        dir: PathBuf,
    }
    impl Harness {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("gaply_oa_{}_{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }
    }
    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn fetched_full_text_is_indexed_embedded_and_linked_by_doi() {
        let h = Harness::new("fulltext");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let http = MockHttpFetcher::new().route("unpaywall", 200, UNPAYWALL_PDF);
        let bytes = MockBytes::new().route("paper.pdf", 200, real_pdf_bytes());
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        match report.outcome {
            FetchOutcome::Fetched { document_id, chunks_indexed, chunks_embedded, checkable, source, .. } => {
                assert!(chunks_indexed > 0, "nothing was indexed");
                assert_eq!(chunks_embedded, chunks_indexed, "not every chunk was embedded");
                assert!(checkable, "a fully embedded source is not checkable");
                assert_eq!(source, OaSource::Unpaywall);
                // The link is recorded as an IDENTIFIER match, which is what
                // the fetch actually had.
                assert_eq!(
                    gaply_core::citation_links::documents_for_citation(&db, "c1").unwrap(),
                    vec![(document_id, gaply_core::citation_links::MatchedBy::Doi)]
                );
                // It is a full text, not an abstract.
                assert!(!store::is_abstract_only(&db, document_id).unwrap());
            }
            other => panic!("expected Fetched, got {other:?}"),
        }
        // The file is on disk where the viewer can read it.
        assert!(oa_dir(&h.dir).exists());
        assert_eq!(std::fs::read_dir(oa_dir(&h.dir)).unwrap().count(), 1);
    }

    #[test]
    fn an_html_body_at_a_pdf_url_is_no_oa_copy_and_stores_nothing() {
        // The publisher answered a `.pdf` URL with a login interstitial. Storing
        // that page would put a paywall notice into evidence for the paper.
        let h = Harness::new("html_body");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let http = MockHttpFetcher::new().route("unpaywall", 200, UNPAYWALL_PDF);
        let bytes =
            MockBytes::new().route("paper.pdf", 200, b"<html><body>Sign in</body></html>".to_vec());
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        assert!(matches!(report.outcome, FetchOutcome::NoOaCopy { .. }), "{:?}", report.outcome);
        assert!(
            gaply_core::citation_links::documents_for_citation(&db, "c1").unwrap().is_empty(),
            "a non-PDF body still produced a link"
        );
        assert!(!oa_dir(&h.dir).exists(), "a non-PDF body still wrote a file");
    }

    #[test]
    fn paywalled_is_reported_and_never_downloaded() {
        let h = Harness::new("paywalled");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let http = MockHttpFetcher::new()
            .route("unpaywall", 200, UNPAYWALL_CLOSED)
            .route("openalex", 200, OPENALEX_CLOSED);
        let bytes = MockBytes::new();
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        assert!(matches!(report.outcome, FetchOutcome::Paywalled { .. }), "{:?}", report.outcome);
        assert!(bytes.calls().is_empty(), "a paywalled source was still downloaded");
    }

    #[test]
    fn no_oa_copy_when_no_index_knows_the_doi() {
        let h = Harness::new("no_oa");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let http = MockHttpFetcher::new(); // 404s everywhere
        let bytes = MockBytes::new();
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        assert!(matches!(report.outcome, FetchOutcome::NoOaCopy { .. }), "{:?}", report.outcome);
        assert!(bytes.calls().is_empty());
    }

    #[test]
    fn abstract_only_is_stored_flagged_and_linked() {
        let h = Harness::new("abstract_only");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let http = MockHttpFetcher::new()
            .route("unpaywall", 200, UNPAYWALL_CLOSED)
            .route("openalex", 200, OPENALEX_ABSTRACT);
        let bytes = MockBytes::new();
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        match report.outcome {
            FetchOutcome::AbstractOnly { document_id, chunks_indexed, checkable, .. } => {
                assert!(chunks_indexed > 0);
                assert!(checkable);
                // THE flag. Without it nothing downstream can tell a 250-word
                // summary from the paper, and the verdict cap has nothing to read.
                assert!(
                    store::is_abstract_only(&db, document_id).unwrap(),
                    "an abstract was stored as if it were a full text"
                );
                assert_eq!(
                    gaply_core::citation_links::documents_for_citation(&db, "c1").unwrap(),
                    vec![(document_id, gaply_core::citation_links::MatchedBy::Doi)]
                );
            }
            other => panic!("expected AbstractOnly, got {other:?}"),
        }
        assert!(bytes.calls().is_empty(), "the abstract path downloaded something");
    }

    #[test]
    fn a_fetched_scan_with_no_text_layer_is_not_importable_and_stores_nothing() {
        // The fetch worked; the file is a scan. That is not a failure, and it
        // must not leave a documents row, a link, or the downloaded file behind.
        let h = Harness::new("scanned_fetch");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let scanned = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/gaply-core/tests/fixtures/scanned_no_text.pdf"
        ))
        .unwrap();
        let http = MockHttpFetcher::new().route("unpaywall", 200, UNPAYWALL_PDF);
        let bytes = MockBytes::new().route("paper.pdf", 200, scanned);
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        match &report.outcome {
            FetchOutcome::NotImportable { detail } => {
                assert!(detail.contains("no extractable text"), "{detail}");
                assert!(detail.contains("OCR"), "{detail}");
            }
            other => panic!("expected NotImportable, got {other:?}"),
        }
        assert!(
            gaply_core::citation_links::documents_for_citation(&db, "c1").unwrap().is_empty(),
            "a scanned fetch still produced a link"
        );
        // The downloaded file is cleaned up: nothing references it.
        let left = std::fs::read_dir(oa_dir(&h.dir)).map(|d| d.count()).unwrap_or(0);
        assert_eq!(left, 0, "a rejected download was left on disk");
    }

    #[test]
    fn a_completed_fetch_teaches_the_estimator_this_machines_rate() {
        // The next import's estimate is about THIS machine because this one
        // recorded what it actually did.
        let h = Harness::new("rate_learn");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        assert_eq!(
            gaply_core::import_guard::current_rate(&db, 1).1,
            gaply_core::import_guard::EstimateBasis::Seeded
        );

        let http = MockHttpFetcher::new().route("unpaywall", 200, UNPAYWALL_PDF);
        let bytes = MockBytes::new().route("paper.pdf", 200, real_pdf_bytes());
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };
        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        assert!(matches!(report.outcome, FetchOutcome::Fetched { .. }), "{:?}", report.outcome);
        assert!(
            matches!(
                gaply_core::import_guard::current_rate(&db, 1).1,
                gaply_core::import_guard::EstimateBasis::Measured { .. }
            ),
            "a completed fetch did not record a rate"
        );
    }

    #[test]
    fn a_citation_with_no_doi_makes_no_request_at_all() {
        let h = Harness::new("no_doi");
        let db = db_with_citation(None);
        let http = MockHttpFetcher::new();
        let bytes = MockBytes::new();
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let t = FetchTarget { citation_id: "c1".to_string(), doi: None, title: Some("x".into()) };
        let report = fetch_one(&deps, &t, 1, &fake_embed);
        assert!(matches!(report.outcome, FetchOutcome::NoOaCopy { .. }), "{:?}", report.outcome);
        assert_eq!(http.call_count(), 0, "a DOI-less citation reached the network");
        assert!(bytes.calls().is_empty());
    }

    #[test]
    fn an_already_linked_citation_is_reported_and_never_fetched_again() {
        let h = Harness::new("already_linked");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let http = MockHttpFetcher::new().route("unpaywall", 200, UNPAYWALL_PDF);
        let bytes = MockBytes::new().route("paper.pdf", 200, real_pdf_bytes());
        let lim = ApiRateLimiters::default();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        let first = fetch_one(&deps, &target(), 1, &fake_embed);
        assert!(matches!(first.outcome, FetchOutcome::Fetched { .. }), "{:?}", first.outcome);
        let calls_after_first = bytes.calls().len();

        let second = fetch_one(&deps, &target(), 1, &fake_embed);
        assert!(
            matches!(second.outcome, FetchOutcome::AlreadyLinked { .. }),
            "{:?}",
            second.outcome
        );
        assert_eq!(
            bytes.calls().len(),
            calls_after_first,
            "a second press re-downloaded a paper we already had"
        );
    }

    #[test]
    fn a_rate_limited_lookup_is_retryable_not_absence() {
        let h = Harness::new("rate_limited");
        let db = db_with_citation(Some("10.4103/ijmr.ijmr_892_23"));
        let http = MockHttpFetcher::new().route("unpaywall", 200, UNPAYWALL_PDF);
        let bytes = MockBytes::new().route("paper.pdf", 200, real_pdf_bytes());
        let lim = ApiRateLimiters::uniform(1.0, 1e-9);
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &lim,
            contact_email: Some("ci@gaply.test"),
            app_data_dir: &h.dir,
        };

        // Spend the one token on a different citation, then try ours.
        add_citation(&db, "c2", Some("10.9/other"), "Other");
        let other = FetchTarget {
            citation_id: "c2".to_string(),
            doi: Some("10.9/other".to_string()),
            title: Some("Other".to_string()),
        };
        let _ = fetch_one(&deps, &other, 1, &fake_embed);

        let report = fetch_one(&deps, &target(), 1, &fake_embed);
        assert!(
            matches!(report.outcome, FetchOutcome::RateLimited { .. }),
            "{:?}",
            report.outcome
        );
    }

    #[test]
    fn a_doi_cannot_write_outside_the_oa_directory() {
        // A DOI is third-party text that becomes a filename. Slashes and dots
        // are stripped to dashes, so a traversal-shaped DOI stays inside.
        assert_eq!(doi_stem("../../etc/passwd"), "------etc-passwd");
        assert_eq!(doi_stem("10.4103/ijmr.ijmr_892_23"), "10-4103-ijmr-ijmr-892-23");
        for hostile in ["../x", "a/../../b", "..", "/abs/path"] {
            let stem = doi_stem(hostile);
            assert!(!stem.contains('/'), "{stem} still contains a separator");
            assert!(!stem.contains('.'), "{stem} still contains a dot");
        }
    }

    /// The startup guarantee, mirroring `gen_startup_tests`: this operation is
    /// user-triggered, and no module that runs while the app is coming up may
    /// so much as name it.
    #[test]
    fn startup_performs_no_open_access_fetch_and_no_network() {
        for (name, src) in [
            ("state.rs", include_str!("state.rs")),
            ("logging.rs", include_str!("logging.rs")),
        ] {
            let body = src.split("mod tests").next().unwrap();
            for forbidden in [
                "reqwest",
                "TcpStream",
                "http://",
                "https://",
                "oa_fetch::fetch",
                "unpaywall",
                "openalex",
            ] {
                assert!(
                    !body.contains(forbidden),
                    "{name} references {forbidden:?} — startup must never fetch anything"
                );
            }
        }

        // And the operation itself is reachable only through an explicit
        // command: nothing in this module schedules, retries or prefetches.
        let me = include_str!("oa_fetch.rs");
        let body = me.split("mod tests").next().unwrap();
        for forbidden in ["spawn(", "thread::", "interval", "sleep(", "tokio::time"] {
            assert!(
                !body.contains(forbidden),
                "oa_fetch.rs contains {forbidden:?} — the fetch must be a user action, \
                 never something that happens on its own"
            );
        }
    }
}
