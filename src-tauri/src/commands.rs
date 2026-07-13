//! Thin IPC adapters. Every command is a one-line delegation into
//! `gaply_core` — no business logic lives here, so the Tauri layer can be
//! swapped out without touching the core.

use serde::{Deserialize, Serialize};
use tauri::State;

use gaply_core::ai_detect::{self, AiDetectionReport, ClassifiedAnalysis, HeuristicModel};
use gaply_core::db::{DbHealth, DbInitReport, MigrationReport};
use gaply_core::extract::citations::Reference;
use gaply_core::extract::{self, docparse, ExtractionResult};
use gaply_core::plagiarism::{PlagiarismReport, PlagiarismSession};
use gaply_core::plagiarism_exact::{ExactConfig, ExactPlagiarismReport};
use gaply_core::plagiarism_library::{self, LibraryPaper};
use gaply_core::projects::{self, Project};
use gaply_core::rag::{self, RagHit};
use gaply_core::refverify::ReferenceVerification;
use gaply_core::secrets;
use gaply_core::stats_verdict::{self, AnalysisSpec, VerificationReport as StatsVerificationReport};
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

// --- Plagiarism: the DETERMINISTIC exact-match lane + "my papers" library ----
// A separate lane from `check_plagiarism` (embedding "similar meaning") above:
// deterministic verbatim matching against the user's durable, curated paper
// library. Fully local — path-only over IPC, no network, no model.

/// Derive a human title from a file path (stem), for library entries.
fn title_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Untitled paper".to_string())
}

/// Add a paper to the durable "my papers" library: docparse → extract text →
/// compute Set-2 fingerprints once → store. Explicit user action; the upload
/// under a plagiarism check is NEVER auto-added.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn add_to_plagiarism_library(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
) -> Result<i64, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let title = title.unwrap_or_else(|| title_from_path(&path));
    plagiarism_library::add_paper(&state.db, &title, &text, &path, &ExactConfig::default())
}

/// List the user's paper library (metadata only — never the stored text).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn list_plagiarism_library(state: State<'_, AppState>) -> Result<Vec<LibraryPaper>, GaplyError> {
    plagiarism_library::list_papers(&state.db)
}

/// Remove a paper from the library by id.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn remove_from_plagiarism_library(state: State<'_, AppState>, id: i64) -> Result<bool, GaplyError> {
    plagiarism_library::remove_paper(&state.db, id)
}

/// Deterministic exact-match plagiarism check: compare an upload against the
/// user's "my papers" library (LIBRARY mode, using each paper's precomputed
/// fingerprints) AND against itself (self-plagiarism). The upload stays
/// session-isolated — never added to the library.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn check_plagiarism_exact(
    state: State<'_, AppState>,
    path: String,
) -> Result<ExactPlagiarismReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    plagiarism_library::compare_against_library(&state.db, &text, &ExactConfig::default())
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

/// One analyzed section, exactly as the AI-Check flow saw it. `text` is the
/// SAME `paragraphs.join(" ")` string the passage char-offsets index into —
/// the UI slices `text[start_char..end_char]` to render in-document
/// highlights without re-deriving (and possibly mismatching) the join.
#[derive(Debug, Serialize)]
pub struct AiCheckSection {
    pub kind: gaply_core::extract::SectionKind,
    pub heading: String,
    pub text: String,
}

/// The `run_aicheck` response: the two-way analysis plus the analyzed section
/// texts for in-document highlighting.
#[derive(Debug, Serialize)]
pub struct AiCheckResult {
    pub analysis: ClassifiedAnalysis,
    pub sections: Vec<AiCheckSection>,
}

/// AI Check (Set 5): the TWO-WAY tiered analysis — human-written vs
/// AI-associated — over a manuscript path (see `crate::aicheck` for the
/// probe-driven two-way decision). Long-running — the deep pass is SLM-1 on
/// CPU — so async + spawn_blocking, like `run_full_analysis`. Never fails for
/// a missing model: degraded tiers come back as honest labels in the result.
#[tauri::command]
#[tracing::instrument]
pub async fn run_aicheck(path: String) -> Result<AiCheckResult, GaplyError> {
    tokio::task::spawn_blocking(move || {
        let text = docparse::parse_path(std::path::Path::new(&path))?;
        let extraction = extract::extract_from_text(&text);
        let analysis = crate::aicheck::run_aicheck_flow(&extraction);
        // The same join analyze_passages uses — offsets line up by construction.
        let sections = extraction
            .sections
            .iter()
            .map(|s| AiCheckSection {
                kind: s.kind,
                heading: s.heading.clone(),
                text: s.paragraphs.join(" "),
            })
            .collect();
        Ok(AiCheckResult { analysis, sections })
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("aicheck task panicked: {e}")))?
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

/// Citation Manager (Set 2): resolve VERIFIED citation metadata from a paper
/// file, a DOI, or a title. Deterministic + free: NO LLM, NO proxy — the
/// registry (CrossRef) is the only truth source, every field verified or
/// honestly absent, honest Unverified with a manual-entry fallback otherwise.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn resolve_citation_metadata(
    state: State<'_, AppState>,
    path: Option<String>,
    doi: Option<String>,
    title: Option<String>,
) -> Result<crate::citation_resolver::CitationResolve, GaplyError> {
    crate::citation_resolver::resolve_with_live_fetcher(
        &state.db,
        path.as_deref(),
        doi.as_deref(),
        title.as_deref(),
    )
}

/// Citation Manager (Set 4): the LOCAL-FIRST reference library. All of these
/// are fully local/offline sqlite operations — no network, no LLM, no
/// sign-in required. The frontend's optional Supabase sync layer marks
/// sync_status honestly after real pushes.
#[tauri::command]
#[tracing::instrument(skip(state, csl_json))]
pub fn citation_lib_upsert(
    state: State<'_, AppState>,
    id: String,
    csl_json: serde_json::Value,
    doi: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<gaply_core::citation_library::StoredReference, GaplyError> {
    gaply_core::citation_library::upsert(
        &state.db,
        &id,
        &csl_json.to_string(),
        doi.as_deref(),
        &tags.unwrap_or_default(),
    )
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn citation_lib_list(
    state: State<'_, AppState>,
) -> Result<Vec<gaply_core::citation_library::StoredReference>, GaplyError> {
    gaply_core::citation_library::list(&state.db)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn citation_lib_search(
    state: State<'_, AppState>,
    query: String,
    tag: Option<String>,
) -> Result<Vec<gaply_core::citation_library::StoredReference>, GaplyError> {
    gaply_core::citation_library::search(&state.db, &query, tag.as_deref())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn citation_lib_set_tags(
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<gaply_core::citation_library::StoredReference, GaplyError> {
    gaply_core::citation_library::set_tags(&state.db, &id, &tags)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn citation_lib_delete(state: State<'_, AppState>, id: String) -> Result<(), GaplyError> {
    gaply_core::citation_library::delete(&state.db, &id)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn citation_lib_set_sync_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<(), GaplyError> {
    gaply_core::citation_library::set_sync_status(&state.db, &id, &status)
}

/// Note Creator (Set 3): thin IPC wrappers over the Set 2 notes store
/// (`gaply_core::notes`). All fully local/offline sqlite — no network, no LLM,
/// no sign-in, no entitlement (Note Creator is free). No logic lives here.
///
/// Writable note fields from the frontend. `NoteInput` in core isn't
/// Deserialize (and Set 2's notes.rs stays untouched), so this app-crate struct
/// carries the wire shape and maps into it. `id` is the caller's stable uuid;
/// `fields_json` arrives as a JSON object (null/absent → the store's `{}`).
#[derive(Debug, Deserialize)]
pub struct NoteWrite {
    pub id: String,
    pub note_type: String,
    #[serde(default)]
    pub paper_id: Option<String>,
    #[serde(default)]
    pub paper_title: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub fields_json: Option<serde_json::Value>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl NoteWrite {
    fn into_input(self) -> (String, gaply_core::notes::NoteInput) {
        let fields_json = match self.fields_json {
            None | Some(serde_json::Value::Null) => String::new(), // → core stores '{}'
            Some(v) => v.to_string(),
        };
        (
            self.id,
            gaply_core::notes::NoteInput {
                note_type: self.note_type,
                paper_id: self.paper_id,
                paper_title: self.paper_title,
                title: self.title,
                fields_json,
                body: self.body,
                tags: self.tags,
            },
        )
    }
}

#[tauri::command]
#[tracing::instrument(skip(state, note))]
pub fn note_create(
    state: State<'_, AppState>,
    note: NoteWrite,
) -> Result<gaply_core::notes::Note, GaplyError> {
    let (id, input) = note.into_input();
    gaply_core::notes::upsert_note(&state.db, &id, &input)
}

#[tauri::command]
#[tracing::instrument(skip(state, note))]
pub fn note_update(
    state: State<'_, AppState>,
    note: NoteWrite,
) -> Result<gaply_core::notes::Note, GaplyError> {
    let (id, input) = note.into_input();
    gaply_core::notes::update_note(&state.db, &id, &input)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn note_get(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<gaply_core::notes::Note>, GaplyError> {
    gaply_core::notes::get_note(&state.db, &id)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn note_list(
    state: State<'_, AppState>,
    note_type: Option<String>,
    paper_id: Option<String>,
) -> Result<Vec<gaply_core::notes::Note>, GaplyError> {
    gaply_core::notes::list_notes(&state.db, note_type.as_deref(), paper_id.as_deref())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn note_search(
    state: State<'_, AppState>,
    query: String,
    tag: Option<String>,
) -> Result<Vec<gaply_core::notes::Note>, GaplyError> {
    gaply_core::notes::search_notes(&state.db, &query, tag.as_deref())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn note_set_tags(
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<gaply_core::notes::Note, GaplyError> {
    gaply_core::notes::set_tags(&state.db, &id, &tags)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn note_delete(state: State<'_, AppState>, id: String) -> Result<(), GaplyError> {
    gaply_core::notes::delete_note(&state.db, &id)
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

/// Research Gap Finder (Set 4): one achievability-Q&A turn. The structured
/// constraints ACCUMULATOR lives with the caller (frontend state — the
/// chat-history-stays-local discipline); this command hands the pure agent
/// the current snapshot. The chat firewall's pre-filter runs FIRST inside
/// qa_turn (a mid-Q&A "write my paper" pivot is refused before any client is
/// touched), so we skip even the proxy probe on a refusal. CLOUD-ONLY,
/// honest offline (constraints preserved); user JWT rides for entitlement.
#[tauri::command]
#[tracing::instrument(skip(corpus, grounded_gaps, constraints, latest_answer, user_token))]
pub fn run_gapfinder_qa(
    session: String,
    corpus: serde_json::Value,
    grounded_gaps: serde_json::Value,
    constraints: serde_json::Value,
    latest_answer: String,
    user_token: Option<String>,
) -> Result<gaply_core::gap_finder_agent::QaTurn, GaplyError> {
    use gaply_core::chat_agent;
    use gaply_core::gap_finder_agent;

    // FIREWALL first — a ghostwriting pivot never even probes the proxy.
    if chat_agent::is_ghostwriting(&latest_answer) {
        return gap_finder_agent::qa_turn(None, &session, &corpus, &grounded_gaps, &constraints, &latest_answer);
    }

    let proxy = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => Some(client),
        _ => None,
    };
    gap_finder_agent::qa_turn(
        proxy.as_ref().map(|c| c as &dyn gaply_core::verify_agent::ProxyClient),
        &session,
        &corpus,
        &grounded_gaps,
        &constraints,
        &latest_answer,
    )
}

/// Research Gap Finder (Set 5): one structured-draft turn against Set 4's
/// narrowed gaps. The pure agent enforces the prose firewall (per-item
/// detect_manuscript_prose + word cap — a written paragraph is BLOCKED,
/// never shown) and pins grounding from what was sent. The chat firewall
/// pre-filter runs FIRST here too — a prose-authoring request ("write my
/// methodology section as prose") never even probes the proxy. CLOUD-ONLY,
/// honest offline; user JWT rides for entitlement.
#[tauri::command]
#[tracing::instrument(skip(corpus, achievable_gaps, constraints, user_note, user_token))]
pub fn run_gapfinder_draft(
    session: String,
    corpus: serde_json::Value,
    achievable_gaps: serde_json::Value,
    constraints: serde_json::Value,
    user_note: String,
    user_token: Option<String>,
) -> Result<gaply_core::gap_finder_agent::DraftResult, GaplyError> {
    use gaply_core::chat_agent;
    use gaply_core::gap_finder_agent;

    if chat_agent::is_ghostwriting(&user_note) {
        return gap_finder_agent::draft_turn(None, &session, &corpus, &achievable_gaps, &constraints, &user_note);
    }
    let proxy = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => Some(client),
        _ => None,
    };
    gap_finder_agent::draft_turn(
        proxy.as_ref().map(|c| c as &dyn gaply_core::verify_agent::ProxyClient),
        &session,
        &corpus,
        &achievable_gaps,
        &constraints,
        &user_note,
    )
}

/// Gap Finder (Set 6): verify a journal's facts STRICTLY from registry data
/// — OpenAlex /sources + the DOAJ API, refverify-style (rate limiter,
/// llm_safe, TTL cache, honest-unverified). NO LLM is involved in this
/// command at all: the card cannot contain a model-originated fact.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn verify_journal_registry(
    state: State<'_, AppState>,
    issn: String,
    name: Option<String>,
    local_predatory_signals: Option<Vec<String>>,
) -> Result<crate::journal_registry::JournalVerification, GaplyError> {
    let fetcher = crate::http_fetcher::ReqwestFetcher::new()?;
    // Polite per-host budget, refverify-style.
    let limiter = gaply_core::ratelimit::RateLimiter::new(5.0, 1.0);
    crate::journal_registry::verify_journal(
        &state.db,
        &fetcher,
        &limiter,
        &issn,
        name.as_deref().unwrap_or(""),
        &local_predatory_signals.unwrap_or_default(),
    )
}

/// Gap Finder (Set 6): journal-fit reasoning over the VERIFIED card. The
/// model reasons about fit using card facts by id; the gate drops any
/// invented journal/gap and the card itself is never modified. CLOUD-ONLY,
/// honest offline; user JWT rides for entitlement.
#[tauri::command]
#[tracing::instrument(skip(corpus, achievable_gaps, journal_card, user_token))]
pub fn run_gapfinder_fit(
    session: String,
    corpus: serde_json::Value,
    achievable_gaps: serde_json::Value,
    journal_card: serde_json::Value,
    user_token: Option<String>,
) -> Result<gaply_core::gap_finder_agent::FitResult, GaplyError> {
    use gaply_core::gap_finder_agent;
    let proxy = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => Some(client),
        _ => None,
    };
    gap_finder_agent::fit_turn(
        proxy.as_ref().map(|c| c as &dyn gaply_core::verify_agent::ProxyClient),
        &session,
        &corpus,
        &achievable_gaps,
        &journal_card,
    )
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

// ============================================================================
// Statistical Analysis Verifier (premium) — thin adapters over
// `gaply_core::stats_verdict` (deterministic recompute + verdict) and
// `gaply_core::stats_chat` (the analysis-scoped interpretive chat). The
// verified math lives entirely in core; these commands only parse the uploaded
// table and delegate. No business logic here.
// ============================================================================

/// Columns of the uploaded data + an optional extraction-prefilled p-value, so
/// the frontend spec-builder can map roles and pre-fill the reported statistic.
#[derive(Debug, Serialize)]
pub struct StatsPreview {
    pub headers: Vec<String>,
    pub row_count: usize,
    /// p-value pre-filled from an optional manuscript (via extraction); the
    /// test-statistic value stays user-provided (extraction can't capture it).
    pub prefill_p_value: Option<f64>,
}

/// The first table of an uploaded CSV/spreadsheet (header row + data rows).
fn load_first_table(path: &str) -> Result<crate::supplementary::Table, GaplyError> {
    let ev = crate::supplementary::parse_supplementary(std::path::Path::new(path))?;
    ev.tables.into_iter().next().ok_or_else(|| {
        GaplyError::Validation(
            "no table found in the uploaded file — provide a CSV or spreadsheet with a header row \
             and numeric data"
                .into(),
        )
    })
}

/// Deterministic advisory input: validate.rs over an optional manuscript.
fn stats_validity(manuscript_path: &Option<String>) -> Option<StatsValidityReport> {
    let p = manuscript_path.as_ref()?;
    let text = docparse::parse_path(std::path::Path::new(p)).ok()?;
    Some(validate::validate(&extract::extract_from_text(&text)))
}

/// Pre-fill a reported p-value from an optional manuscript's extracted claims.
fn stats_prefill_p(manuscript_path: &Option<String>) -> Option<f64> {
    let p = manuscript_path.as_ref()?;
    let text = docparse::parse_path(std::path::Path::new(p)).ok()?;
    let extraction = extract::extract_from_text(&text);
    stats_verdict::ReportedStatistic::from_claims(&extraction.statistics).p_value
}

/// Preview the uploaded data's columns (and optionally pre-fill the reported
/// p-value from an attached manuscript). Local, deterministic — no network.
#[tauri::command]
#[tracing::instrument(skip(manuscript_path))]
pub fn run_stats_preview(
    path: String,
    manuscript_path: Option<String>,
) -> Result<StatsPreview, GaplyError> {
    let table = load_first_table(&path)?;
    Ok(StatsPreview {
        headers: table.headers,
        row_count: table.rows.len(),
        prefill_p_value: stats_prefill_p(&manuscript_path),
    })
}

/// Recompute the user's analysis-spec against the uploaded data and compare
/// reported vs recomputed → a [`StatsVerificationReport`]. 100% deterministic
/// and LOCAL (the Set 2 engine); no model, no proxy. The optional manuscript
/// adds the deterministic advisory lane (validate.rs).
#[tauri::command]
#[tracing::instrument(skip(spec, manuscript_path))]
pub fn run_stats_verify(
    path: String,
    spec: AnalysisSpec,
    manuscript_path: Option<String>,
) -> Result<StatsVerificationReport, GaplyError> {
    let table = load_first_table(&path)?;
    let validity = stats_validity(&manuscript_path);
    Ok(stats_verdict::verify_analysis(&spec, &table.headers, &table.rows, validity.as_ref()))
}

/// One analysis-scoped interpretive-chat turn (Set 4). The SAME deterministic
/// report is rebuilt server-side to scope the chat (the report type is
/// Serialize-only by design — never round-tripped). FIREWALL runs in local code
/// before any proxy probe; cloud-only, honest degradation; the user's JWT rides
/// along for the proxy's server-side entitlement gate.
#[tauri::command]
#[tracing::instrument(skip(spec, manuscript_path, question, user_token))]
pub fn run_stats_chat(
    path: String,
    spec: AnalysisSpec,
    manuscript_path: Option<String>,
    question: String,
    language: Option<String>,
    user_token: Option<String>,
) -> Result<gaply_core::stats_chat::StatsChatTurn, GaplyError> {
    use gaply_core::stats_chat;

    let language = language.unwrap_or_else(|| "en".to_string());
    let table = load_first_table(&path)?;
    let validity = stats_validity(&manuscript_path);
    let report = stats_verdict::verify_analysis(&spec, &table.headers, &table.rows, validity.as_ref());

    // FIREWALL layer 1 first: a ghostwriting request is refused by local code
    // before we even probe the proxy. (stats_chat's own pre-filter — including
    // the code-authoring mirror — still runs inside, so nothing slips through.)
    if gaply_core::chat_agent::is_ghostwriting(&question) {
        return Ok(stats_chat::stats_chat_turn(None, &report, &question, &language));
    }
    let turn = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => {
            stats_chat::stats_chat_turn(Some(&client), &report, &question, &language)
        }
        _ => stats_chat::stats_chat_turn(None, &report, &question, &language),
    };
    Ok(turn)
}
