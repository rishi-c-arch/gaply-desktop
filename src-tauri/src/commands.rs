//! Thin IPC adapters. Every command is a one-line delegation into
//! `gaply_core` — no business logic lives here, so the Tauri layer can be
//! swapped out without touching the core.

use serde::{Deserialize, Serialize};
use tauri::State;

use gaply_core::ai_detect::{self, AiDetectionReport, HeuristicModel};
use gaply_core::db::{DbHealth, DbInitReport, MigrationReport};
use gaply_core::extract::citations::Reference;
use gaply_core::extract::{self, docparse, ExtractionResult};
use gaply_core::plagiarism::{PlagiarismReport, PlagiarismSession};
use gaply_core::projects::{self, Project};
use gaply_core::rag::{self, RagHit};
use gaply_core::refverify::ReferenceVerification;
use gaply_core::secrets;
use gaply_core::validate::{self, StatsValidityReport};
use gaply_core::{now_epoch, GaplyError};

use crate::http_fetcher::RefVerifier;
use crate::models::proxy_client::ProxyReqwestClient;
use crate::state::AppState;

/// Reference input from the Citation Manager (refverifyBridge). Only bibliographic
/// metadata — never manuscript text.
#[derive(Debug, Deserialize)]
pub struct ReferenceInput {
    pub raw: String,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub core_version: String,
    pub db_ok: bool,
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn create_project(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<Project, GaplyError> {
    projects::create_project(state.store.as_ref(), &name, description.as_deref().unwrap_or(""))
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, GaplyError> {
    projects::list_projects(state.store.as_ref())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn get_project(state: State<'_, AppState>, id: i64) -> Result<Project, GaplyError> {
    projects::get_project(state.store.as_ref(), id)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn health_check(state: State<'_, AppState>) -> Result<HealthReport, GaplyError> {
    let db_ok = state.store.ping().is_ok();
    Ok(HealthReport { core_version: gaply_core::core_version().to_string(), db_ok })
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn db_init(state: State<'_, AppState>) -> Result<DbInitReport, GaplyError> {
    state.db.init_report()
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn db_migrate(state: State<'_, AppState>) -> Result<MigrationReport, GaplyError> {
    state.db.migrate()
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn db_health(state: State<'_, AppState>) -> Result<DbHealth, GaplyError> {
    state.db.health()
}

/// Parse a manuscript file (PDF/DOCX/TXT), extract its structure and claims,
/// store the result, and return the typed `ExtractionResult`. Pure offline:
/// the file is read from the local disk and never leaves the machine.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn extract_manuscript(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
) -> Result<ExtractionResult, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let result = extract::extract_from_text(&text);
    let manuscript_title = title
        .or_else(|| result.title.clone())
        .unwrap_or_else(|| "Untitled manuscript".to_string());
    let manuscript_id = state.db.create_manuscript(&manuscript_title, "", "")?;
    extract::persist::store_extraction(&state.db, manuscript_id, &result)?;
    Ok(result)
}

/// Full offline analysis pipeline: parse → extract → deterministically
/// validate. Stores the extraction and the validation findings (severity
/// CRITICAL/MAJOR), and returns the validity report. No network, no LLM.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn validate_manuscript(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
) -> Result<StatsValidityReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let extraction = extract::extract_from_text(&text);
    let manuscript_title = title
        .or_else(|| extraction.title.clone())
        .unwrap_or_else(|| "Untitled manuscript".to_string());
    let manuscript_id = state.db.create_manuscript(&manuscript_title, "", "")?;
    let store = extract::persist::store_extraction(&state.db, manuscript_id, &extraction)?;
    let report = validate::validate(&extraction);
    validate::store_validation(&state.db, manuscript_id, Some(store.extraction_id), &report)?;
    Ok(report)
}

/// Offline AI-text detection: parse → extract → per-section perplexity /
/// burstiness scoring. Returns a report that is an explicit statistical
/// signal (never proof — see the mandatory disclaimer field). No network,
/// no LLM call. Uses the interim heuristic scorer until GPT-2 is wired.
/// Offline plagiarism / semantic-similarity check. Parses the manuscript,
/// embeds its chunks into a PER-SESSION ISOLATED store (never the shared
/// corpus), and compares against the shared corpus and against itself.
/// Returns matched spans with similarity scores and source provenance.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn check_plagiarism(
    state: State<'_, AppState>,
    path: String,
) -> Result<PlagiarismReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let mut session = PlagiarismSession::new()?;
    session.ingest_manuscript(state.embedder.as_ref(), &text)?;
    session.report(&state.db, None)
}

// --- Secret handling ---------------------------------------------------------
// API keys live only in the OS keychain, accessed by the Rust core. The
// frontend may store, check, or delete a key the user typed, but there is NO
// command to read a raw key back — external calls that need it are made from
// the core, never from React.

/// Fetch a compiled report by id (the `report_id` from AnalysisEvent::Finished).
/// Returns the REAL cached compile_report output — the report viewer routes to
/// this instead of the sample fixture. NotFound if it expired / never existed.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn get_report(state: State<'_, AppState>, report_id: String) -> Result<serde_json::Value, GaplyError> {
    let key = format!("report:{report_id}");
    match state.db.cache_get(&key, now_epoch())? {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| GaplyError::Internal(format!("parse cached report: {e}"))),
        None => Err(GaplyError::NotFound { entity: "report", id: report_id }),
    }
}

/// Write the bundled sample manuscript to a real temp file and return its
/// absolute path, so the one-click "sample scan" runs through the REAL pipeline
/// (the old `__bundled_sample__` sentinel pointed at no file). Text is embedded
/// in the binary, so this works in dev and in a packaged build.
#[tauri::command]
#[tracing::instrument]
pub fn sample_manuscript_path() -> Result<String, GaplyError> {
    const SAMPLE: &str = include_str!("../resources/sample_manuscript.txt");
    let path = std::env::temp_dir().join("gaply-sample-manuscript.txt");
    std::fs::write(&path, SAMPLE)?;
    Ok(path.to_string_lossy().to_string())
}

/// Store an API key (or any secret) the user entered, in the OS keychain.
#[tauri::command]
#[tracing::instrument(skip(value))]
pub fn store_secret(name: String, value: String) -> Result<(), GaplyError> {
    secrets::store_secret(&name, &value)
}

/// Whether a named secret exists (presence only — never the value).
#[tauri::command]
#[tracing::instrument]
pub fn has_secret(name: String) -> Result<bool, GaplyError> {
    secrets::has_secret(&name)
}

/// Delete a named secret from the OS keychain.
#[tauri::command]
#[tracing::instrument]
pub fn delete_secret(name: String) -> Result<(), GaplyError> {
    secrets::delete_secret(&name)
}

#[tauri::command]
#[tracing::instrument]
pub fn detect_ai(path: String) -> Result<AiDetectionReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let extraction = extract::extract_from_text(&text);
    let model = HeuristicModel::gpt2_like();
    Ok(ai_detect::detect_extraction(&model, &extraction))
}

/// Verify one reference against the live connectors (CrossRef / OpenAlex /
/// Retraction Watch / Unpaywall / Semantic Scholar) — real HTTP via rustls,
/// cache-first and rate-limited. The Citation Manager (refverifyBridge) calls
/// this; it was previously MISSING, so verify/enrich silently did nothing.
///
/// Async + spawn_blocking: the HTTP client is blocking, so it must not run on
/// the async runtime thread. Returns the structured ReferenceVerification
/// (metadata only — no manuscript text is involved).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn verify_reference(
    state: State<'_, AppState>,
    reference: ReferenceInput,
) -> Result<ReferenceVerification, GaplyError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let verifier = RefVerifier::new()?;
        let reference = Reference {
            raw: reference.raw,
            authors: String::new(),
            year: reference.year,
            title: reference.title,
            doi: reference.doi,
        };
        verifier.verify(&db, &reference, now_epoch())
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("verify_reference task panicked: {e}")))?
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn rag_search(
    state: State<'_, AppState>,
    query: String,
    top_k: Option<usize>,
    source_filter: Option<String>,
) -> Result<Vec<RagHit>, GaplyError> {
    rag::search(
        &state.db,
        state.embedder.as_ref(),
        &query,
        top_k.unwrap_or(8).min(64),
        source_filter.as_deref(),
    )
}

/// Ingest a target journal's page and/or author-guidelines URL into the
/// `journal_guideline` corpus (PublishReady). Rate-limited fetch, all fetched
/// text treated as untrusted (sanitized/quarantined). Never fails the job:
/// unreachable/non-HTML/injected sources come back as Unavailable/Quarantined
/// with an honest note, and the checklist simply stays bare — no fabrication.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn ingest_guidelines(
    state: State<'_, AppState>,
    journal_url: Option<String>,
    guidelines_url: Option<String>,
) -> Result<crate::guidelines::GuidelinesReport, GaplyError> {
    let ingestor = crate::guidelines::GuidelinesIngestor::new()?;
    Ok(ingestor.ingest(
        &state.db,
        state.embedder.as_ref(),
        journal_url.as_deref(),
        guidelines_url.as_deref(),
    ))
}

/// PublishReady result: the local report, the (cloud) reviewer evaluation, and
/// the exact structured payload sent to the proxy (exposed so the UI/tests can
/// prove it carries no manuscript text).
#[derive(Debug, Serialize)]
pub struct PublishReadyOutcome {
    pub report: serde_json::Value,
    pub reviewer: gaply_core::reviewer_agent::ReviewerEvaluation,
    pub proxy_payload: serde_json::Value,
}

/// PublishReady: run the existing 6-lane pipeline (UNMODIFIED), then layer a
/// single-pass cloud reviewer evaluation on top. The reviewer is CLOUD-ONLY —
/// it uses the remote proxy directly, never the local Ollama/mock chain (a
/// local model is not an acceptable substitute for deep reviewer reasoning).
/// When the proxy is not live the reviewer letter is marked "unavailable
/// offline"; the rest of the report (all local) is still returned.
#[tauri::command]
#[tracing::instrument(skip(state, user_token))]
pub fn run_publishready(
    state: State<'_, AppState>,
    path: String,
    journal_name: String,
    journal_quartile: String,
    supplementary_paths: Option<Vec<String>>,
    user_token: Option<String>,
) -> Result<PublishReadyOutcome, GaplyError> {
    use gaply_core::reviewer_agent::{self, ReviewerEvaluation, TargetJournal};
    use gaply_core::verify_agent::ProxyClient;

    // 1) Run the existing pipeline (composes on top; the 6 lanes are untouched).
    let events = std::cell::RefCell::new(Vec::new());
    let emit = |e: crate::pipeline::AnalysisEvent| events.borrow_mut().push(e);
    crate::pipeline::run_pipeline_measured(state.db.clone(), state.embedder.clone(), path, None, &emit)?;
    let report_id = events
        .into_inner()
        .into_iter()
        .find_map(|e| match e {
            crate::pipeline::AnalysisEvent::Finished { report_id } => Some(report_id),
            _ => None,
        })
        .ok_or_else(|| GaplyError::Internal("pipeline produced no report".into()))?;
    let json = state
        .db
        .cache_get(&format!("report:{report_id}"), gaply_core::now_epoch())?
        .ok_or_else(|| GaplyError::Internal("compiled report not found".into()))?;
    let report: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| GaplyError::Internal(format!("parse report: {e}")))?;

    // 2) Parse any supplementary files (memory-capped, app-crate) → JSON. The
    //    reviewer_agent llm_safe's every string; a file that fails to parse is
    //    skipped (honest), never fatal.
    let supp_values: Vec<serde_json::Value> = supplementary_paths
        .unwrap_or_default()
        .iter()
        .filter_map(|p| match crate::supplementary::parse_supplementary(std::path::Path::new(p)) {
            Ok(ev) => serde_json::to_value(&ev).ok(),
            Err(e) => {
                tracing::warn!(path = %p, error = %e, "supplementary parse failed; skipped");
                None
            }
        })
        .collect();

    // 3) Build the validator-compliant, privacy-guarded reviewer payload.
    let journal = TargetJournal { name: journal_name, quartile: journal_quartile };
    let (proxy_payload, sent_ids) = reviewer_agent::build_review_payload(&report, &journal, &supp_values);

    // 3) Reviewer: cloud only, honest offline degradation. The user's JWT
    //    rides along so the proxy can run THE REAL entitlement gate + consume
    //    a use server-side (Set 8; enforcement joins the deployed proxy).
    let reviewer = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => match client
            .verify(&proxy_payload)
            .and_then(|resp| reviewer_agent::gate_reviewer_response(&resp, &sent_ids))
        {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!(error = %e, "reviewer cloud call failed; marking unavailable");
                ReviewerEvaluation::unavailable_offline()
            }
        },
        _ => ReviewerEvaluation::unavailable_offline(),
    };

    Ok(PublishReadyOutcome { report, reviewer, proxy_payload })
}

/// Research Gap Finder (Set 2): build the session's paper corpus — N uploaded
/// files + N links → bounded, llm_safe per-paper digests (stable ids p1…pN)
/// + a per-session RAG ingest. NO reasoning, NO LLM, NO model load (the
/// one-at-a-time lifecycle is untouched); link fetches are rate-limited per
/// host with capped downloads. Caps reject oversize input honestly.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn build_gapfinder_corpus(
    state: State<'_, AppState>,
    session: String,
    paths: Option<Vec<String>>,
    links: Option<Vec<String>>,
) -> Result<crate::paper_corpus::CorpusReport, GaplyError> {
    let fetcher = crate::paper_corpus::ReqwestPaperFetcher::new()?;
    // Same polite per-host budget as guidelines ingestion.
    let limiter = gaply_core::ratelimit::RateLimiter::new(5.0, 1.0);
    crate::paper_corpus::build_corpus(
        &state.db,
        state.embedder.as_ref(),
        &session,
        &paths.unwrap_or_default(),
        &links.unwrap_or_default(),
        &fetcher,
        &limiter,
    )
}

/// Research Gap Finder (Set 3): grounded gap extraction over the session's
/// paper corpus. CLOUD-ONLY like the reviewer — the pure
/// gaply_core::gap_finder_agent builds a session-scoped, privacy-guarded
/// payload and gates the reply (grounded_gaps must cite sent paper ids;
/// ungrounded ideas land in the labeled suggestions lane, decided by CODE);
/// no local model is loaded (one-at-a-time safe). Proxy unreachable → honest
/// unavailable_offline, never faked. The user's JWT rides along for the
/// server-side entitlement gate (Set 8 seam).
#[derive(Debug, Serialize)]
pub struct GapFinderOutcome {
    pub findings: gaply_core::gap_finder_agent::GapFindings,
    /// The exact payload sent (or that would be sent) to the proxy — exposed
    /// so the UI/tests can prove no raw paper text crosses the boundary.
    pub proxy_payload: serde_json::Value,
}

#[tauri::command]
#[tracing::instrument(skip(corpus, user_token))]
pub fn run_gap_finder(
    session: String,
    corpus: serde_json::Value,
    user_token: Option<String>,
) -> Result<GapFinderOutcome, GaplyError> {
    use gaply_core::gap_finder_agent::{self, GapFindings};
    use gaply_core::verify_agent::ProxyClient;

    // Session-scoped, privacy-guarded payload (errors on session mismatch).
    let (proxy_payload, sent) = gap_finder_agent::build_gap_payload(&session, &corpus)?;

    let findings = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => match client
            .verify(&proxy_payload)
            .and_then(|resp| gap_finder_agent::gate_gap_response(&resp, &sent))
        {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(error = %e, "gap finder cloud call failed; marking unavailable");
                GapFindings::unavailable_offline()
            }
        },
        _ => GapFindings::unavailable_offline(),
    };
    Ok(GapFinderOutcome { findings, proxy_payload })
}

/// Research Copilot: one report-scoped chat turn behind the integrity
/// FIREWALL (`gaply_core::chat_agent`). CLOUD-ONLY like the reviewer — no
/// local model is ever loaded here (one-at-a-time lifecycle safe), and the
/// App Check signing key stays in the OS keychain (no secrets in the
/// frontend). `context` is the frontend's structured chat context
/// (findings/rag/citations summaries); the core re-applies the privacy +
/// llm_safe discipline before anything crosses the trust boundary. Chat
/// history is frontend state — nothing is persisted here.
#[tauri::command]
#[tracing::instrument(skip(context, question, user_token))]
pub fn run_copilot_chat(
    context: serde_json::Value,
    question: String,
    language: Option<String>,
    user_token: Option<String>,
) -> Result<gaply_core::chat_agent::ChatTurn, GaplyError> {
    use gaply_core::chat_agent;

    let language = language.unwrap_or_else(|| "en".to_string());

    // FIREWALL LAYER 1 first: a ghostwriting request is refused by local code
    // before we even probe the proxy — the model is never in the loop.
    if chat_agent::is_ghostwriting(&question) {
        return Ok(chat_agent::chat_turn(None, &context, &question, &language));
    }

    // Cloud only, honest degradation: unreachable/unprovisioned proxy → the
    // turn honestly says answers need the cloud (never a faked answer).
    // The user's JWT rides along for the proxy's server-side entitlement gate
    // (Set 8; enforcement joins the deployed proxy).
    let turn = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => {
            chat_agent::chat_turn(Some(&client), &context, &question, &language)
        }
        _ => chat_agent::chat_turn(None, &context, &question, &language),
    };
    Ok(turn)
}
