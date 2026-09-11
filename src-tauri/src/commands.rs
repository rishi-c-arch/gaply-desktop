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
/// no LLM call.
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
    citation_id: Option<String>,
) -> Result<i64, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let title = title.unwrap_or_else(|| title_from_path(&path));
    // M2 Set 2A: citation_id is the OPTIONAL soft anchor → citation_library.id.
    // Frontend callers that omit it (all of them until Set 2B) deserialize to
    // None → stored NULL → unchanged behavior.
    plagiarism_library::add_paper(
        &state.db,
        &title,
        &text,
        &path,
        citation_id.as_deref(),
        &ExactConfig::default(),
    )
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
///
/// # DELIBERATELY UNTYPED, unlike `run_publishready`'s parse
///
/// This hands the frontend the report it already renders, so a typed parse would
/// add a failure mode to a path that has none and buy nothing — the wire is JSON
/// either way. `run_publishready` types its parse because the value FEEDS THE
/// AGGREGATOR, where a silent fallback becomes a wrong verdict identity (§31.7).
///
/// The asymmetry is the boundary: TYPE WHAT YOU COMPUTE FROM, pass through what
/// you only forward.
///
/// It does ENRICH, which is not the same as computing from: `enrich_report_labels`
/// ADDS presentation keys and reads none of the report's values into a decision.
/// That is the COMPOSER's job under §4.19 — the engine emits data, the boundary
/// attaches the words.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn get_report(state: State<'_, AppState>, report_id: String) -> Result<serde_json::Value, GaplyError> {
    let key = crate::pipeline::report_cache_key(&report_id);
    match state.db.cache_get(&key, now_epoch())? {
        Some(json) => {
            let mut report: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| GaplyError::Internal(format!("parse cached report: {e}")))?;
            // COMPOSER, not engine (ONTOLOGY §4.19): the cached report carries no
            // user-facing labels, and they are attached HERE — so a wording edit
            // applies to reports cached before it, and the engine's output stays
            // free of presentation.
            gaply_core::vocabulary::enrich_report_labels(&mut report);
            Ok(report)
        }
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
#[tracing::instrument(skip(state, on_event))]
pub async fn run_aicheck(
    state: State<'_, AppState>,
    path: String,
    // The NETWORK citation-verification opt-in (Set C2). `None`/`false` (the
    // default) keeps AI Check fully on-device; `true` is the AND of the user's
    // explicit AI-Check opt-in and the global cloud gate, decided by the caller.
    verify_citations: Option<bool>,
    // Progress/terminal events streamed during the run (pipeline.rs precedent).
    on_event: tauri::ipc::Channel<crate::aicheck::AiCheckEvent>,
) -> Result<AiCheckResult, GaplyError> {
    use crate::aicheck::AiCheckEvent;
    let db = state.db.clone();
    let cancel = state.aicheck_cancel.clone();
    // RESET the cancel token per run — a cancelled run must not poison the next.
    cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    let verify_citations = verify_citations.unwrap_or(false);
    tokio::task::spawn_blocking(move || {
        // Closed-channel-ignored: the frontend may have navigated away.
        let emit = move |ev: AiCheckEvent| {
            let _ = on_event.send(ev);
        };
        emit(AiCheckEvent::Extract);
        let text = docparse::parse_path(std::path::Path::new(&path))?;
        let extraction = extract::extract_from_text(&text);
        let should_cancel = || cancel.load(std::sync::atomic::Ordering::SeqCst);
        let mut analysis =
            match crate::aicheck::run_aicheck_flow(&extraction, &emit, &should_cancel) {
                Ok(a) => a,
                Err(c) => {
                    // Honest terminal Cancelled event; the promise rejects with a
                    // distinct code so the UI shows a benign stop, not an error.
                    let stage = match c.stage {
                        gaply_core::ai_detect::AnalysisStage::Stage1 => "stage-1 language model",
                        gaply_core::ai_detect::AnalysisStage::DeepVerify => "deep verification",
                    };
                    emit(AiCheckEvent::Cancelled { stage: stage.into(), done: c.done, total: c.total });
                    return Err(GaplyError::Cancelled);
                }
            };
        // C2: merge the citation-verification lane (metadata-only, cache-first).
        crate::aicheck::apply_citation_verification(
            &mut analysis, &extraction, &db, now_epoch(), verify_citations,
        );
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
        emit(AiCheckEvent::Report);
        Ok(AiCheckResult { analysis, sections })
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("aicheck task panicked: {e}")))?
}

/// Flip the AI Check cancel token — the running `run_aicheck` (if any) polls it
/// per-passage and stops promptly with an honest Cancelled state. Cheap + sync.
#[tauri::command]
pub fn cancel_aicheck(state: State<'_, AppState>) {
    state.aicheck_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Pre-flight memory status for the AI Check page (re-checkable — the user closes
/// apps, re-checks, and sees it improve). TRUTHFUL about the tier THIS machine
/// can run (never implies freeing memory unlocks the 7B on an 8GB box).
#[tauri::command]
pub fn aicheck_memory_status() -> crate::aicheck::AiCheckMemoryStatus {
    crate::aicheck::memory_status(
        crate::models::free_memory_bytes(),
        crate::models::total_physical_ram_bytes(),
        crate::models::deep_tier_from_env(),
        crate::models::stage1_lm_present(),
    )
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
pub async fn ingest_guidelines(
    state: State<'_, AppState>,
    journal_url: Option<String>,
    guidelines_url: Option<String>,
) -> Result<crate::guidelines::GuidelinesReport, GaplyError> {
    // async + spawn_blocking (mirrors build_gapfinder_corpus / run_full_analysis):
    // the guideline page fetch is blocking HTTP, so run off the event-loop thread
    // (this was the one command deferred from H1). Extract the Arc handles first
    // (State<'_> isn't Send). Logic unchanged.
    let db = state.db.clone();
    let embedder = state.embedder.clone();
    tokio::task::spawn_blocking(move || {
        let ingestor = crate::guidelines::GuidelinesIngestor::new()?;
        Ok(ingestor.ingest(
            &db,
            embedder.as_ref(),
            journal_url.as_deref(),
            guidelines_url.as_deref(),
        ))
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("ingest guidelines task panicked: {e}")))?
}

/// PublishReady result: the local report, the (cloud) reviewer evaluation, and
/// the exact structured payload sent to the proxy (exposed so the UI/tests can
/// prove it carries no manuscript text).
#[derive(Debug, Serialize)]
pub struct PublishReadyOutcome {
    pub report: serde_json::Value,
    pub reviewer: gaply_core::reviewer_agent::ReviewerEvaluation,
    pub proxy_payload: serde_json::Value,
    /// Stable identity for the whole run (== manuscript_id/report_id). Threaded
    /// to the Evidence Store + escalation so Chat can query this run later.
    pub run_id: String,
    /// Box 4 (Stage 1, SHADOW): the reviewer letter synthesized from the
    /// Evidence Store's per-finding verdicts, produced ALONGSIDE `reviewer` for
    /// comparison. `None` if assembly failed. NOT authoritative — the wholesale
    /// `reviewer` above is still primary until Stage 2/3 (switch, then remove
    /// the wholesale path), which are separate future decisions gated on the
    /// proxy escalate endpoint + reviewer-quality spike landing.
    pub shadow_reviewer: Option<gaply_core::reviewer_agent::ReviewerEvaluation>,
    /// The rendered PDF, produced during the run because the model it needs
    /// cannot be rebuilt afterwards. NOT serialized to the frontend — see the
    /// `#[serde(skip)]`; it is lifted into `AppState` by `run_publishready`
    /// and fetched by `export_publishready_pdf`.
    #[serde(skip)]
    pub pdf_bytes: Vec<u8>,
}

/// PublishReady: run the existing 6-lane pipeline (UNMODIFIED), then layer a
/// single-pass cloud reviewer evaluation on top. The reviewer is CLOUD-ONLY —
/// it uses the remote proxy directly, never the local Ollama/mock chain (a
/// local model is not an acceptable substitute for deep reviewer reasoning).
/// When the proxy is not live the reviewer letter is marked "unavailable
/// offline"; the rest of the report (all local) is still returned.
#[tauri::command]
#[tracing::instrument(skip(state, user_token))]
pub async fn run_publishready(
    state: State<'_, AppState>,
    path: String,
    journal_name: String,
    journal_quartile: String,
    supplementary_paths: Option<Vec<String>>,
    user_token: Option<String>,
    // The guideline document the user asked for. Threaded so the checklist scopes
    // to THAT journal — identity propagated, never re-derived from corpus state.
    guidelines_url: Option<String>,
) -> Result<PublishReadyOutcome, GaplyError> {
    // async + spawn_blocking (mirrors run_full_analysis): the 6-lane pipeline is
    // CPU-heavy and the reviewer does blocking keychain/HTTP, so run off the
    // event-loop thread. Extract the Arc handles first (State<'_> isn't Send).
    // Logic unchanged.
    let db = state.db.clone();
    let embedder = state.embedder.clone();
    let pdfs = state.report_pdfs.clone();
    let mut outcome = tokio::task::spawn_blocking(move || {
        run_publishready_measured(
            db,
            embedder,
            path,
            journal_name,
            journal_quartile,
            supplementary_paths,
            user_token,
            guidelines_url,
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("publishready task panicked: {e}")))??;

    // Lift the rendered bytes into session memory and drop them from the value
    // that crosses the IPC boundary — the frontend never receives a PDF it did
    // not ask for, and `#[serde(skip)]` means it could not anyway.
    let bytes = std::mem::take(&mut outcome.pdf_bytes);
    if !bytes.is_empty() {
        if let Ok(mut q) = pdfs.lock() {
            q.retain(|(id, _)| id != &outcome.run_id);
            q.push_back((outcome.run_id.clone(), bytes));
            while q.len() > crate::state::MAX_CACHED_PDFS {
                q.pop_front();
            }
        }
    }
    Ok(outcome)
}

/// The whole PublishReady command path, WITHOUT Tauri.
///
/// # Why this exists — ARCHITECTURE_TRACE §32
///
/// `run_publishready` is a `#[tauri::command]` taking `State<'_, AppState>`,
/// which an example binary cannot construct. That made **the entire command
/// path unreachable outside the app** — and with it the Box 4 comparison
/// record, so `release_gate`'s COMPARISON and PERSISTENCE invariants could only
/// ever report SKIPPED. Two of the gate's six invariants had never once been
/// evaluated, and `ship_ready` was false by construction.
///
/// **This is exactly the seam `pipeline::run_pipeline_measured` already is**, for
/// exactly the same reason. It is not a new pattern; the command path simply
/// never got one.
///
/// **Nothing here needs a proxy, an entitlement or a signed-in user.** The
/// reviewer degrades to `ReviewerEvaluation::unavailable_offline()` on an
/// unreachable proxy, a 401 or a 403, and the Box 4 harness block is
/// UNCONDITIONAL (see its comment below). Both `findings_sent` counts are
/// `DeterministicLocal` — the count of what WOULD be sent, never read from a
/// response — so they are `Observed` offline, as run 23's 403-throughout
/// capture shows.
///
/// `#[doc(hidden)]` on the measured entry point mirrors `run_pipeline_measured`:
/// it is a seam for instruments, not app surface.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn run_publishready_measured(
    db: std::sync::Arc<gaply_core::Database>,
    embedder: std::sync::Arc<dyn gaply_core::embed::Embedder>,
    path: String,
    journal_name: String,
    journal_quartile: String,
    supplementary_paths: Option<Vec<String>>,
    user_token: Option<String>,
    guidelines_url: Option<String>,
) -> Result<PublishReadyOutcome, GaplyError> {
    // Content-addressed manuscript identity for the Box 4 comparison record.
    // `run_id` is a local DB row id and cannot link records across machines.
    let manuscript_sha256 = crate::harness_log::manuscript_sha256(std::path::Path::new(&path));
    use gaply_core::reviewer_agent::{self, ReviewerEvaluation, TargetJournal};
    use gaply_core::verify_agent::ProxyClient;

    // 1) Run the existing pipeline (composes on top; the 6 lanes are untouched).
    let events = std::cell::RefCell::new(Vec::new());
    let emit = |e: crate::pipeline::AnalysisEvent| events.borrow_mut().push(e);
    // `user_token` threaded so the pipeline's cloud VERIFICATION tier can pass
    // the proxy's entitlement gate. Without it that tier reached `/verify`
    // with App Check but no user credential and was rejected 401
    // user_token_missing, so every citation degraded to UNKNOWN.
    // The pipeline now returns its extraction and plagiarism outputs too
    // (§31.2's two unreachable sources). This path consumes `lanes` only;
    // the local report model that consumes the rest is built by the PDF
    // path, and the cached-report re-read below is deliberately left alone
    // because it is what exercises PR-2's typed parse.
    let pipeline_out = crate::pipeline::run_pipeline_measured(
        db.clone(),
        embedder.clone(),
        path,
        None,
        user_token.clone(),
        guidelines_url.clone(),
        &emit,
    )?;
    let lanes = pipeline_out.lanes;

    let report_id = events
        .into_inner()
        .into_iter()
        .find_map(|e| match e {
            crate::pipeline::AnalysisEvent::Finished { report_id } => Some(report_id),
            _ => None,
        })
        .ok_or_else(|| GaplyError::Internal("pipeline produced no report".into()))?;
    let json = db
        .cache_get(&crate::pipeline::report_cache_key(&report_id), gaply_core::now_epoch())?
        .ok_or_else(|| GaplyError::Internal("compiled report not found".into()))?;
    // The escalation path still reads `report["evidence"]` as JSON — PR-2
    // already made that path WITHHOLD rather than default, so it is correct
    // as it stands and is deliberately unchanged.
    let report_json: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| GaplyError::Internal(format!("parse report: {e}")))?;
    // TYPED. Strict parsing replaces four silent `unwrap_or` fallbacks — one
    // of which let a malformed title become "" INSIDE BOTH DIGESTS (§31.7),
    // producing a stable but wrong identity instead of a failure.
    //
    // The message states the OBSERVATION and the REMEDY and names no cause.
    // That is not vagueness: putting the schema version in the cache key
    // (`report_cache_key`) made STALENESS UNREACHABLE — a stale-shape report
    // MISSES the key rather than mis-parsing — so a parse failure on a
    // matching key can no longer be explained by a version mismatch, and
    // naming one would be a guess. Re-running is the remedy either way.
    let report: gaply_core::report::PublishReadyReport = serde_json::from_str(&json)
        .map_err(|e| {
            tracing::warn!(run_id = %report_id, error = %e, "cached report failed to parse");
            GaplyError::Internal(
                "This saved report could not be read. Please run the analysis again to \
                 generate a new report. If it keeps happening, that may indicate a bug — \
                 email helloresearcher@gaply.in."
                    .to_string(),
            )
        })?;

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

    // Box 2 (ADDITIVE): targeted escalation — route this run's evidence,
    // persist it, and ATTEMPT per-finding cloud adjudication. HONEST BOUNDARY:
    // escalation CALLS degrade to "unavailable" until gaply-proxy implements a
    // task:"escalate_findings" endpoint (server-side, out of scope) AND the
    // reviewer-quality spike passes; this ships routing/persistence/idempotency
    // infra, NOT working per-finding verdicts. The wholesale reviewer below is
    // UNCHANGED. run_id == report_id == manuscript_id (one id for the whole run).
    // The escalation summary was previously LOGGED AND DROPPED — the fifth
    // instance of the projection pattern (§22.4), in exactly the place §26
    // predicted the fix would go. `withheld` now crosses into the verdict.
    let escalation_withheld: Option<gaply_core::reviewer_agent::VerdictWithheld>;
    {
        let esc_policy = gaply_core::orchestrator::DefaultRoutingPolicy::default();
        let esc_proxy = ProxyReqwestClient::from_env()
            .map(|c| c.with_user_token(user_token.clone()))
            .ok()
            .filter(|c| c.reachable());
        escalation_withheld = crate::escalation::run_targeted_escalation(
            &db,
            esc_proxy.as_ref().map(|c| c as &dyn ProxyClient),
            &report_id,
            &report_json,
            &esc_policy,
            gaply_core::now_epoch(),
        )
        .withheld;
        tracing::info!(run_id = %report_id, ?escalation_withheld, "targeted escalation (degrades until proxy escalate endpoint + reviewer-quality spike)");
    }

    // 3) Build the validator-compliant, privacy-guarded reviewer payload.
    let journal = TargetJournal { name: journal_name, quartile: journal_quartile };
    let (proxy_payload, sent_ids) =
        reviewer_agent::build_review_payload(&report, &journal, &supp_values, &report_id);

    // Box 4 (Stage 1, ADDITIVE SHADOW): synthesize a reviewer letter from the
    // Evidence Store's per-finding verdicts, mirroring Box 2's additive wiring.
    // The deterministic verdict is computed locally; the narrative is a cloud
    // call that degrades honestly. Produced for comparison — the WHOLESALE
    // reviewer below is still authoritative. Stage 2 (switch) / Stage 3
    // (remove wholesale) are separate future decisions.
    let (shadow_outcome, shadow_elapsed) = {
        let shadow_proxy = ProxyReqwestClient::from_env()
            .map(|c| c.with_user_token(user_token.clone()))
            .ok()
            .filter(|c| c.reachable());
        let started = std::time::Instant::now();
        let outcome = match crate::reviewer_synthesis::run_shadow_synthesis(
            &db,
            shadow_proxy.as_ref().map(|c| c as &dyn ProxyClient),
            &report_id,
            &report,
            &journal,
            &supp_values,
            escalation_withheld,
            lanes,
        ) {
            Ok(o) => {
                tracing::info!(
                    run_id = %report_id,
                    recommendation = ?o.letter.recommendation,
                    narrative_available = o.narrative_available,
                    "box4 shadow synthesis (NOT primary; wholesale reviewer still authoritative)"
                );
                Some(o)
            }
            Err(e) => {
                tracing::warn!(run_id = %report_id, error = %e, "box4 shadow synthesis failed; skipped");
                None
            }
        };
        (outcome, started.elapsed())
    };

    // RENDER, AFTER THE AGGREGATION — the report exists to state the
    // RECOMMENDATION, and it is not computed until here (§52.1). Rendering
    // earlier is what let the PDF read "Overall assessment: pass" on a run the
    // log recorded as MajorRevision: the composer was printing the debate's
    // consensus answer, which is a different quantity wearing that label.
    //
    // `LocalReportModel` carries manuscript prose and is deliberately not
    // `Serialize` (§4.22), so it cannot be rebuilt later from anything
    // persisted. The DETERMINISTIC recommendation is used — computed locally
    // from the severity breakdown, always available, and the one §27.1 and §30
    // treat as the deterministic verdict. `None` when none was produced, which
    // `VerdictWithheld` makes a real state and the cover must not render as
    // approval.
    let recommendation =
        shadow_outcome.as_ref().and_then(|o| o.aggregation.verdict.recommendation());
    let pdf_bytes = gaply_core::report_pdf::render_pdf(&gaply_core::report_compose::compose(
        &pipeline_out.report_model(
            Some(journal.name.clone()),
            guidelines_url.clone(),
            recommendation,
        ),
    ));

    // 3) Reviewer: cloud only, honest offline degradation. The user's JWT
    //    rides along so the proxy can run THE REAL entitlement gate + consume
    //    a use server-side (Set 8; enforcement joins the deployed proxy).
    let mut proxy_meta: Option<gaply_core::reviewer_harness::ProxyMeta> = None;
    let wholesale_started = std::time::Instant::now();
    let reviewer = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
        Ok(client) if client.reachable() => match client.verify_with_envelope(&proxy_payload) {
            Ok((resp, env)) => {
                // Surface model/stop_reason for the harness (tokens/latency
                // are still absent from the envelope → they stay Unavailable).
                proxy_meta = Some(gaply_core::reviewer_harness::ProxyMeta {
                    model: env.model,
                    stop_reason: env.stop_reason,
                    ..Default::default()
                });
                match reviewer_agent::gate_reviewer_response(&resp, &sent_ids) {
                    Ok(ev) => ev,
                    Err(e) => {
                        tracing::warn!(error = %e, "reviewer gate failed; marking unavailable");
                        ReviewerEvaluation::unavailable_offline()
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "reviewer cloud call failed; marking unavailable");
                ReviewerEvaluation::unavailable_offline()
            }
        },
        _ => ReviewerEvaluation::unavailable_offline(),
    };
    let wholesale_elapsed = wholesale_started.elapsed();

    // Box 4 shadow-comparison harness (Stage 1, LOG-ONLY): compare the shadow
    // synthesis against the wholesale reviewer. Every metric carries its
    // provenance by construction; proxy/LLM metrics render `unavailable` until
    // the live proxy + escalate endpoint land. Drives NO production behavior.
    {
        use gaply_core::reviewer_harness::{
            build_comparison_report, HarnessInputs, HarnessTiming, ShadowInputs,
        };
        // UNCONDITIONAL. Previously this whole block sat inside
        // `if let Some(outcome) = &shadow_outcome`, so a run where the shadow
        // synthesis produced nothing left NO record — indistinguishable from a
        // broken sink. Typed absence at the artifact level: the record is
        // always written, and when the shadow side is missing every shadow
        // metric says so with a reason (ONTOLOGY §4.12).
        let report = build_comparison_report(&HarnessInputs {
            run_id: &report_id,
            manuscript_sha256: &manuscript_sha256,
            recorded_at: gaply_core::now_epoch(),
            shadow: shadow_outcome.as_ref().map(|o| ShadowInputs {
                withheld: o.withheld,
                letter: &o.letter,
                breakdown: &o.aggregation.breakdown,
                findings_sent: o.findings_sent,
                narrative_available: o.narrative_available,
            }),
            wholesale: &reviewer,
            wholesale_findings_sent: sent_ids.findings.len(),
            // Both digests over the SAME `proxy_payload` Value that was
            // handed to `verify_with_envelope` above — not a rebuilt or
            // equivalent object, so `summary_digest` corresponds to the
            // reviewer's actual input representation.
            findings_projection_digest: &reviewer_agent::findings_projection_digest(
                &proxy_payload,
            ),
            summary_digest: &reviewer_agent::summary_digest(&proxy_payload),
            summary_format_version: reviewer_agent::SUMMARY_FORMAT_VERSION,
            journal_name: Some(journal.name.as_str()),
            guidelines_url: guidelines_url.as_deref(),
            timing: HarnessTiming {
                shadow: Some(shadow_elapsed),
                wholesale: Some(wholesale_elapsed),
            },
            proxy_meta, // model/stop_reason when /verify ran; tokens/latency still Tier 2c
        });
        tracing::info!(run_id = %report_id, "box4 shadow-comparison report:\n{}", report.to_markdown());
        // Leave a record. Best-effort: a diagnostic never fails the analysis.
        crate::harness_log::append(&report);
    }

    let shadow_reviewer = shadow_outcome.map(|o| o.letter);
    Ok(PublishReadyOutcome {
        pdf_bytes,
        // The IPC field stays JSON: the frontend renders it and does not need
        // the Rust type. Serializing the TYPED value guarantees it is exactly
        // what parsed, rather than the separately-parsed `report_json`.
        report: {
            let mut v = serde_json::to_value(&report).unwrap_or(report_json);
            gaply_core::vocabulary::enrich_report_labels(&mut v);
            v
        },
        reviewer,
        proxy_payload,
        run_id: report_id,
        shadow_reviewer,
    })
}

/// Citation Manager (Set 2): resolve VERIFIED citation metadata from a paper
/// file, a DOI, or a title. Deterministic + free: NO LLM, NO proxy — the
/// registry (CrossRef) is the only truth source, every field verified or
/// honestly absent, honest Unverified with a manual-entry fallback otherwise.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn resolve_citation_metadata(
    state: State<'_, AppState>,
    path: Option<String>,
    doi: Option<String>,
    title: Option<String>,
) -> Result<crate::citation_resolver::CitationResolve, GaplyError> {
    // async + spawn_blocking (mirrors verify_reference): the CrossRef fetch is a
    // blocking HTTP call, so it must run off the event-loop thread. Extract the
    // Arc<Database> handle first (State<'_> isn't Send). Logic unchanged.
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        crate::citation_resolver::resolve_with_live_fetcher(
            &db,
            path.as_deref(),
            doi.as_deref(),
            title.as_deref(),
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("resolve citation task panicked: {e}")))?
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
    // Scope B: the persisted verification/retraction facts. All optional so an
    // omitting caller defaults to a not-retracted, unverified entry.
    retracted: Option<bool>,
    source: Option<String>,
    verify_provenance: Option<Vec<String>>,
    verify_outcome: Option<String>,
    verified_at: Option<i64>,
    // §11 D102: the outcome of a retraction check, and when it was established.
    // Absent → never checked, which is NOT the same as clear.
    retraction_outcome: Option<String>,
    retraction_checked_at: Option<i64>,
) -> Result<gaply_core::citation_library::StoredReference, GaplyError> {
    let verify = gaply_core::citation_library::VerificationWrite {
        retracted: retracted.unwrap_or(false),
        source,
        verify_provenance: verify_provenance.unwrap_or_default(),
        verify_outcome,
        verified_at,
        retraction_outcome,
        retraction_checked_at,
    };
    gaply_core::citation_library::upsert(
        &state.db,
        &id,
        &csl_json.to_string(),
        doi.as_deref(),
        &tags.unwrap_or_default(),
        &verify,
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

// ---------------------------------------------------------------------------
// Citation Intelligence — AI engine, Phase 1 (docs/AI_ENGINE_PLAN.md)
// ---------------------------------------------------------------------------
//
// Thin IPC wrappers over `gaply_core::ai_engine::store`, following the
// citation_lib_* precedent: no logic lives here. Phase 1 writes `ai_chunks`
// ONLY — no embeddings, no model, and the existing `embeddings` table is not
// touched.

/// What one indexing pass did, reported honestly.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiIndexReport {
    pub document_id: i64,
    pub outcome: gaply_core::ai_engine::store::IndexOutcome,
    pub status: gaply_core::ai_engine::store::IndexStatus,
    /// True when the source yielded real page boundaries. False means every
    /// chunk is honestly page-less — NOT that pages were guessed.
    pub pages_detected: bool,
}

/// Parse a document, chunk it page-aware, and store the chunks.
///
/// `path` is where to READ the document; `document_id` is the `documents` row it
/// belongs to. The path is accepted explicitly because the v6 `documents` table
/// has no local-path column (plan §11 D5); when omitted it falls back to the
/// row's `source_url`, and if neither names a readable file the command fails
/// rather than silently indexing nothing.
///
/// Async + spawn_blocking: parsing a PDF is blocking work and must not run on
/// the async runtime thread (the `verify_reference` precedent). Re-running is
/// safe — `index_chunks` is idempotent by database constraint.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ai_index_document(
    state: State<'_, AppState>,
    document_id: i64,
    path: Option<String>,
) -> Result<AiIndexReport, GaplyError> {
    use gaply_core::ai_engine::store;
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let source = match path {
            Some(p) if !p.trim().is_empty() => p,
            _ => store::document_source(&db, document_id)?,
        };
        // The same guard the link and fetch paths use. This command is the
        // generic index entry point, so leaving it unguarded would be a way
        // around the gate rather than an exception to it.
        let file = std::path::Path::new(&source);
        let pre = gaply_core::import_guard::preflight(&db, file, gaply_core::now_epoch())?;
        if pre.is_refused() {
            return Err(GaplyError::Validation(pre.summary));
        }
        let blocks = gaply_core::extract::docparse::parse_path_paged(file)?;
        let chunks = gaply_core::chunk::chunk_paged_default(&blocks);
        let outcome = store::index_chunks(&db, document_id, &chunks)?;
        let status = store::index_status(&db, document_id)?;
        Ok(AiIndexReport {
            document_id,
            pages_detected: status.with_page > 0,
            outcome,
            status,
        })
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("ai_index_document task panicked: {e}")))?
}

/// What is currently indexed for a document. Sync — a single indexed read.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn ai_index_status(
    state: State<'_, AppState>,
    document_id: i64,
) -> Result<gaply_core::ai_engine::store::IndexStatus, GaplyError> {
    gaply_core::ai_engine::store::index_status(&state.db, document_id)
}

// --- Citation Intelligence, Phase 3: the generic inference engine ----------
//
// Generic infrastructure only. NONE of the spec's eight task prompts live here;
// they are a later phase and consume this engine.

/// Everything the UI can honestly say about the models.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelStatus {
    /// Embedding engine (resident once loaded).
    pub embedding: crate::ai::embeddings::EngineState,
    /// Generative model lifecycle state.
    pub generative: crate::ai::model_manager::GenState,
    pub generative_model_id: String,
    /// In-flight generations.
    pub in_flight: usize,
    /// Computed from GGUF metadata — NEVER process RSS. `None` when no
    /// generative model resolves.
    pub generative_ram: Option<crate::ai::generative::RamEstimate>,
    /// §11 D44. Which device inference actually runs on, and why it is not the
    /// fast one when it is not. Phase 7 built the selector and wired it to
    /// nothing, so the UI had no honest way to say CPU or Metal.
    pub active_device: &'static str,
    pub device_fallback_reason: Option<String>,
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn ai_model_status(state: State<'_, AppState>) -> AiModelStatus {
    // Cheap: the gate is a class lookup and Metal device creation is not
    // attempted unless it passes (§11 D36).
    let device = crate::ai::device::select();
    AiModelStatus {
        embedding: state.ai_embed.state(),
        generative: state.ai_gen.state(),
        generative_model_id: state.ai_gen.model_id(),
        in_flight: state.ai_gen.in_flight(),
        // Reading GGUF metadata does NOT load the model.
        generative_ram: state.ai_gen.ram_estimate().ok(),
        active_device: device.kind.as_str(),
        device_fallback_reason: device.fallback_reason,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
// `rename_all` renames VARIANTS ONLY — struct-variant FIELDS keep their Rust
// snake_case unless `rename_all_fields` says otherwise. Without it this enum
// serialized `max_tokens`, `total_bytes`, `queued_behind` … while every
// TypeScript reader asked for the camelCase spelling and silently got
// `undefined`. Pinned by `event_wire_shape` tests.
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AiGenerateEvent {
    /// One incremental piece of decoded text.
    Token { text: String },
}

/// Dev-only end-to-end proof of the generation path: prompt -> generate ->
/// extract -> parse -> validate -> (one retry) -> typed result.
///
/// Registers the bundled generative model on first use (idempotent). Streams
/// tokens over the existing Channel pattern; cancellable via ai_generate_cancel.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn ai_generate_test(
    state: State<'_, AppState>,
    phrase: String,
    on_event: tauri::ipc::Channel<AiGenerateEvent>,
) -> Result<serde_json::Value, GaplyError> {
    use crate::ai::task::{run_task, EchoTask, EvidenceChunk, TaskContext};

    // Register on first use. Idempotent upsert, and it keeps the recorded path
    // honest if the resolver ever picks a different file.
    if let Some(loader) = crate::ai::generative::BundledGenerativeLoader::resolve() {
        crate::ai::generative::register_generative(&state.db, &loader)?;
    }

    let cancel_token = state.ai_gen_cancel.issue();
    let cancel = cancel_token.flag();

    let ctx = TaskContext::new(vec![EvidenceChunk {
        chunk_id: "c1".into(),
        page: Some(1),
        section: Some("Results".into()),
        text: "This is a development evidence chunk.".into(),
    }]);
    let task = EchoTask { phrase, evidence: ctx.render_evidence() };

    let sink: crate::ai::model_manager::TokenSink =
        std::sync::Arc::new(move |t: &str| {
            // Closed channel ignored: the user may have navigated away.
            let _ = on_event.send(AiGenerateEvent::Token { text: t.to_string() });
        });

    let run = run_task(&*state.ai_gen, &task, &ctx, cancel, Some(sink))
        .await
        .map_err(GaplyError::from)?;

    Ok(serde_json::json!({
        "output": run.output,
        "retried": run.retried,
        "promptVersion": run.prompt_version,
        "modelId": run.model_id,
        "loadedModelFile": state.ai_gen.loaded_model_file(),
        "tokens": run.tokens,
        "elapsedMs": run.elapsed_ms,
    }))
}

#[tauri::command]
pub fn ai_generate_cancel(state: State<'_, AppState>) {
    // Stops exactly the runs alive right now. A request that arrives after this
    // starts clean — cancellation is per-run, never a shared flag the next
    // request would have to clear (and, clearing it, revive this one).
    let stopped = state.ai_gen_cancel.cancel_all();
    tracing::info!(stopped, "generation cancel requested");
}

/// Spec Prompt 3 — does this sentence need a citation?
///
/// Thin wrapper over run_task: the task owns its prompt and validator, the
/// engine owns the lifecycle and the retry. NO PERSISTENCE (plan §11 D9) —
/// citation_need carries no evidence, cites no chunk and quotes nothing, so
/// writing it to ai_evidence_cards would create rows with a NULL chunk_id and
/// an empty quote, which is the shape the grounding check exists to prevent.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ai_citation_need(
    state: State<'_, AppState>,
    sentence: String,
    preceding_sentence: Option<String>,
    following_sentence: Option<String>,
    section: String,
) -> Result<serde_json::Value, GaplyError> {
    use crate::ai::task::{run_task, TaskContext};
    use crate::ai::tasks::citation_need::{CitationNeedInput, CitationNeedTask};

    // §11 D39 — take priority over any running batch job. Acquired BEFORE the
    // inference gate is touched, so the window is never narrower than the
    // request; released on drop, including on an early `?`.
    let _priority = state.ai_interactive.acquire();
    if let Some(loader) = crate::ai::generative::BundledGenerativeLoader::resolve() {
        crate::ai::generative::register_generative(&state.db, &loader)?;
    }
    let cancel_token = state.ai_gen_cancel.issue();
    let cancel = cancel_token.flag();

    let task = CitationNeedTask::new(CitationNeedInput {
        sentence,
        preceding_sentence: preceding_sentence.unwrap_or_default(),
        following_sentence: following_sentence.unwrap_or_default(),
        section,
    });
    // No evidence for this task; the empty context still runs the generic
    // chunk-id check, which simply has nothing to check.
    let ctx = TaskContext::default();

    let run = match run_task(&*state.ai_gen, &task, &ctx, cancel, None).await {
        Ok(r) => r,
        // Same contract as ai_citation_support: a rejected model answer is an
        // outcome the UI can explain, not an error it has to render raw.
        Err(e @ crate::ai::task::TaskError::ValidationFailed { .. }) => {
            return Ok(serde_json::json!({
                "outcome": "validationFailed",
                "reason": e.to_string(),
                // Same reasoning as the support arm: the raw attempts and stop
                // reasons are the evidence, and a Display string is a summary.
                "detail": e,
                "modelId": state.ai_gen.model_id(),
                "loadedModelFile": state.ai_gen.loaded_model_file(),
            }))
        }
        Err(other) => return Err(GaplyError::from(other)),
    };

    Ok(serde_json::json!({
        "output": run.output,
        "retried": run.retried,
        "promptVersion": run.prompt_version,
        "modelId": run.model_id,
        "loadedModelFile": state.ai_gen.loaded_model_file(),
        "tokens": run.tokens,
        "elapsedMs": run.elapsed_ms,
    }))
}

#[derive(Debug, Clone, serde::Serialize)]
// `rename_all` renames VARIANTS ONLY — struct-variant FIELDS keep their Rust
// snake_case unless `rename_all_fields` says otherwise. Without it this enum
// serialized `max_tokens`, `total_bytes`, `queued_behind` … while every
// TypeScript reader asked for the camelCase spelling and silently got
// `undefined`. Pinned by `event_wire_shape` tests.
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AiSupportEvent {
    Retrieving,
    /// How much evidence survived the budget — emitted before generation so a
    /// UI can say "judging 6 of 11 passages" rather than showing a bare spinner.
    Retrieved { chunks_sent: usize, chunks_dropped: usize },
    /// `queued_behind` is how many other generations were already alive when
    /// this one asked for the model. The engine runs ONE at a time, so a
    /// non-zero value is the difference between "slow" and "waiting", and a UI
    /// that cannot tell them apart shows a spinner that means nothing.
    Generating { queued_behind: usize },
    /// A liveness heartbeat while decoding: `tokens` of `max_tokens` produced.
    /// Emitted every [`DECODE_HEARTBEAT_TOKENS`] tokens, not per token — the
    /// point is proof of progress, not a token stream.
    Decoding { tokens: usize, max_tokens: usize },
    Validating,
}

/// How often the decode heartbeat fires. Small enough that a stalled run is
/// obvious within seconds of real progress, large enough not to flood the IPC
/// channel on a fast machine.
const DECODE_HEARTBEAT_TOKENS: usize = 8;

/// Spec Prompt 2 — does the cited paper actually say this?
///
/// Retrieval (Phase 2) -> evidence budget -> run_task (Phase 3/4c) -> persist.
/// A VALID result is written to ai_evidence_cards with its join rows; a fatal
/// failure and a NoEvidence outcome persist NOTHING.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn ai_citation_support(
    state: State<'_, AppState>,
    claim: String,
    document_id: i64,
    cited_source: Option<String>,
    on_event: tauri::ipc::Channel<AiSupportEvent>,
) -> Result<serde_json::Value, GaplyError> {
    use crate::ai::evidence::{assemble, Assembled, EVIDENCE_BUDGET_TOKENS};
    use crate::ai::task::run_task;
    use crate::ai::tasks::citation_support::{CitationSupportTask, Verdict};
    use gaply_core::ai_engine::cards;

    // A CLAIM, not a passage. The first real-manuscript failure was two 3B
    // generations — minutes — spent on 2,163 characters of the cited paper's
    // own Methods section, pasted into the claim box. The model did what it was
    // told: it decomposed a whole section into claim_elements and produced 67
    // lines of JSON that ran out of room mid-array.
    //
    // Refused BEFORE the engine is touched, and as an OUTCOME rather than an
    // error, because it is a true and actionable answer rather than a fault.
    // The bound is generous — a long academic sentence is ~400 characters and
    // the measured real claim was 141 (33 tokens) — so this only catches input
    // that was never a claim.
    use crate::ai::tasks::citation_support::{claim_is_a_passage, MAX_CLAIM_CHARS};
    if let Some(chars) = claim_is_a_passage(&claim) {
        return Ok(serde_json::json!({
            "outcome": "claimTooLong",
            "reason": format!(
                "That is {} characters — a passage, not a claim. This check asks whether ONE \
                 sentence from your writing is supported by the cited source, so paste the \
                 sentence that cites it, not a block of the source itself.",
                chars
            ),
            "claimChars": chars,
            "maxClaimChars": MAX_CLAIM_CHARS,
            "documentId": document_id,
            "persisted": false,
        }));
    }


    // §11 D39 — take priority over any running batch job. Acquired BEFORE the
    // inference gate is touched, so the window is never narrower than the
    // request; released on drop, including on an early `?`.
    let _priority = state.ai_interactive.acquire();
    if let Some(loader) = crate::ai::generative::BundledGenerativeLoader::resolve() {
        crate::ai::generative::register_generative(&state.db, &loader)?;
    }
    let cancel_token = state.ai_gen_cancel.issue();
    let cancel = cancel_token.flag();

    let db = state.db.clone();
    let slot = state.ai_embed.clone();
    let manager = state.ai_gen.clone();

    let _ = on_event.send(AiSupportEvent::Retrieving);
    let claim_for_query = claim.clone();
    let bundle = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let qv = slot.with(|e| e.embed_query(&claim_for_query))?;
            assemble(&db, document_id, &claim_for_query, &qv, EVIDENCE_BUDGET_TOKENS)
        })
        .await
        .map_err(|e| GaplyError::Internal(format!("evidence task panicked: {e}")))??
    };

    let bundle = match bundle {
        // D15: no evidence means no generation and no persistence. This is NOT
        // the spec's insufficient_evidence, which is a judgement about evidence
        // that was shown.
        Assembled::NoEvidence { reason } => {
            return Ok(serde_json::json!({
                "outcome": "noEvidence",
                "reason": reason,
                "documentId": document_id,
                "persisted": false,
            }))
        }
        Assembled::Ready(b) => b,
    };
    let _ = on_event.send(AiSupportEvent::Retrieved {
        chunks_sent: bundle.chunks_sent,
        chunks_dropped: bundle.chunks_dropped,
    });

    // Snapshot before `bundle` is partly moved into the task below.
    const EXAMINED_SHOWN: usize = 3;
    let examined: Vec<serde_json::Value> = bundle
        .examined
        .iter()
        .take(EXAMINED_SHOWN)
        .map(|c| {
            serde_json::json!({
                "chunkId": c.chunk_id,
                "page": c.page,
                "text": c.text,
            })
        })
        .collect();

    let task = CitationSupportTask {
        claim: claim.clone(),
        cited_source: cited_source.unwrap_or_else(|| format!("document {document_id}")),
        evidence: bundle.rendered.clone(),
    };
    // in_flight() counts leases already taken, i.e. the runs this one must wait
    // for. Read BEFORE run_task, which is what takes ours.
    let _ = on_event.send(AiSupportEvent::Generating { queued_behind: manager.in_flight() });

    // A heartbeat, not a token stream: the panel needs to prove the machine is
    // working, and on an unoptimised build a single check is minutes of silence.
    let max_tokens = <CitationSupportTask as crate::ai::task::AiTask>::max_tokens();
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let beat = on_event.clone();
    let beat_seen = seen.clone();
    let sink: crate::ai::model_manager::TokenSink = std::sync::Arc::new(move |_t: &str| {
        let n = beat_seen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if n % DECODE_HEARTBEAT_TOKENS == 0 {
            // Closed channel ignored: the user may have navigated away.
            let _ = beat.send(AiSupportEvent::Decoding { tokens: n, max_tokens });
        }
    });

    let run = match run_task(&*manager, &task, &bundle.ctx, cancel, Some(sink)).await {
        Ok(r) => r,
        // The model's answer failed Gaply's grounding checks twice. Nothing was
        // persisted, nothing is broken, and this is one of the command's three
        // documented non-success OUTCOMES — the same class as NoEvidence above.
        // Returning it as a transport error instead meant the UI's honest
        // wording for it could never fire: the panel has always had a
        // `validationFailed` branch, and the command has never sent one.
        Err(e @ crate::ai::task::TaskError::ValidationFailed { .. }) => {
            // EVERYTHING the error carries, not just its Display string.
            //
            // `TaskError::ValidationFailed` was built to hold both raw outputs
            // and both stop reasons — its own doc comment calls `stop_reasons`
            // "the answer to the question a failed run always raises: did the
            // model say something wrong, or did it simply run out of room?" —
            // and this arm used to discard all of it and send `e.to_string()`.
            // The result was a failure nobody could diagnose: the first real
            // manuscript run hit "EOF while parsing a list at line 67 column 5"
            // and neither the log nor the payload could say whether the reply
            // was truncated, how long it was, or what it actually said.
            //
            // Logged at WARN as well as returned: a validation failure is rare,
            // it is the case a developer most needs the transcript for, and the
            // UI keeps only what it renders.
            tracing::warn!(
                error = ?e,
                document_id,
                "citation_support failed validation twice — raw attempts follow"
            );
            return Ok(serde_json::json!({
                "outcome": "validationFailed",
                "reason": e.to_string(),
                // The typed detail: errors, primary, firstRaw, retryRaw,
                // timings and stopReasons.
                "detail": e,
                "documentId": document_id,
                "persisted": false,
                // WHICH MODEL SAID THIS. A failure without it sends the reader
                // to the registry to infer an answer, and the registry records
                // what was CONFIGURED, not what ran.
                "modelId": manager.model_id(),
                "loadedModelFile": manager.loaded_model_file(),
            }))
        }
        // Cancellation and a genuine generation fault stay errors: one is the
        // user's own request coming back, the other really is a fault.
        Err(other) => return Err(GaplyError::from(other)),
    };
    let _ = on_event.send(AiSupportEvent::Validating);

    // Is this document the PAPER, or an abstract of it (v18)? Asked of the
    // store rather than inferred from the text, because a 250-word document and
    // a 250-word paper are indistinguishable by content and only the fetch that
    // wrote the row knows which it produced.
    let abstract_only = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            gaply_core::ai_engine::store::is_abstract_only(&db, document_id)
        })
        .await
        .map_err(|e| GaplyError::Internal(format!("abstract-only lookup panicked: {e}")))??
    };
    // THE CAP. An abstract can support a claim, but it cannot establish one:
    // the subgroup, the limitation and the number in the table are all outside
    // it, and `strong` asserts a completeness the evidence never had.
    //
    // Applied ON TOP of the model's answer rather than by rewriting it. The
    // validator's own invariants tie `partial` to a non-null `suggested_rewrite`
    // (§11 D32's family of rules), so editing `verdict` in place would produce
    // an output object that fails the very checks it just passed. What is
    // recorded instead is both facts: what the model said about the evidence it
    // saw, and what Gaply is prepared to stand behind given what that evidence
    // WAS.
    let capped_verdict = if abstract_only && run.output.verdict == Verdict::Strong {
        Some(Verdict::Partial)
    } else {
        None
    };
    let effective_verdict = capped_verdict.unwrap_or(run.output.verdict);

    // Persist. The validator has already proved every chunk_id was sent and
    // every page matches the store, so the join rows below cannot point at
    // anything the model invented.
    let advisories: Vec<String> = run.advisories.iter().map(|a| a.to_string()).collect();
    let provenance = serde_json::json!({
        "advisories": advisories,
        "abstractOnly": abstract_only,
        "rawVerdict": run.output.verdict,
        "verdictCapped": capped_verdict.is_some(),
        "chunksSent": bundle.chunks_sent,
        "chunksDropped": bundle.chunks_dropped,
        "evidenceWords": bundle.words_estimated,
        "evidenceTokensImplied": bundle.tokens_implied,
        "retrievalPath": bundle.retrieval_path,
        "supportingChunks": run.output.supporting_chunks,
    });
    let primary = run
        .output
        .supporting_chunks
        .first()
        .and_then(|c| c.chunk_id.trim_start_matches('c').parse::<i64>().ok());
    let refs: Vec<(i64, cards::ChunkRole)> = run
        .output
        .supporting_chunks
        .iter()
        .filter_map(|c| c.chunk_id.trim_start_matches('c').parse::<i64>().ok())
        .map(|id| (id, cards::ChunkRole::Supporting))
        .collect();

    let card = cards::NewEvidenceCard {
        document_id,
        chunk_id: primary,
        page: run.output.supporting_chunks.first().and_then(|c| c.page),
        claim: claim.clone(),
        evidence_text: run.output.explanation.clone(),
        evidence_type: match run.output.verdict {
            Verdict::Contradicts => "contradict".to_string(),
            Verdict::InsufficientEvidence => "context".to_string(),
            _ => "support".to_string(),
        },
        // Serialize the enum rather than hand-writing the five strings twice —
        // a second mapping is a second thing to drift from the spec.
        //
        // The EFFECTIVE verdict, not the raw one: the card is what the app
        // stands behind, and a stored `strong` that every surface renders as
        // `partial` is a store that disagrees with its own UI.
        verdict: serde_json::to_value(effective_verdict)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string)),
        confidence: Some(run.output.confidence),
        model_id: run.model_id.clone(),
        model_version: crate::ai::generative::BUNDLED_GEN_MODEL_QUANT.to_string(),
        prompt_version: run.prompt_version.to_string(),
        provenance_json: provenance.to_string(),
    };
    let card_id = tokio::task::spawn_blocking(move || cards::insert_card(&db, &card, &refs))
        .await
        .map_err(|e| GaplyError::Internal(format!("persist task panicked: {e}")))??;

    Ok(serde_json::json!({
        "outcome": "ok",
        "cardId": card_id,
        "persisted": true,
        "output": run.output,
        // WHAT THE SOURCE WAS. A check run against an abstract is a real check
        // and a useful one, but the reader has to be told which it was — an
        // unlabelled verdict invites them to believe the paper was read.
        "abstractOnly": abstract_only,
        "checkedAgainstLabel": if abstract_only {
            Some("checked against abstract only")
        } else {
            None
        },
        // The verdict the app stands behind, and — when they differ — the one
        // the model gave, so the cap is visible rather than silent.
        "effectiveVerdict": effective_verdict,
        "verdictCapped": capped_verdict.is_some(),
        "advisories": advisories,
        "retried": run.retried,
        "promptVersion": run.prompt_version,
        "modelId": run.model_id,
        "chunksSent": bundle.chunks_sent,
        "chunksDropped": bundle.chunks_dropped,
        "elapsedMs": run.elapsed_ms,
        "loadedModelFile": manager.loaded_model_file(),
        // WHY IT TOOK THAT LONG. The measured decode rate, and how much memory
        // the machine had left at the moment the run ended. The bake-off got
        // 6.7 tok/s from this model on this hardware; a run at a fifth of that
        // is not the model being slow, it is the machine being squeezed — and
        // the reader cannot tell those apart from a wall-clock number alone.
        // Sampled AFTER the run: mid-run is when it mattered, and this is the
        // closest cheap moment to it.
        "decodeTokensPerSec": if run.timings.decode_ms > 0 {
            Some(run.timings.tokens as f64 / (run.timings.decode_ms as f64 / 1000.0))
        } else {
            None
        },
        "decodeMs": run.timings.decode_ms,
        "prefillMs": run.timings.prefill_ms,
        "promptTokens": run.timings.prompt_tokens,
        "freeMemoryBytes": crate::models::free_memory_bytes(),
        "totalMemoryBytes": crate::models::total_physical_ram_bytes(),
        // WHAT WAS EXAMINED, best match first. A verdict of
        // insufficient_evidence cites nothing by design, and without these the
        // UI can only refuse to show it — which throws away a true and useful
        // answer ("I looked at these and none of them support you"). Capped:
        // this is the top of the ranking, not the whole block the model read,
        // and `chunksSent` says how many that was.
        "examinedPassages": examined,
    }))
}

// --- Citation Intelligence, Phase 2: embeddings + semantic search ----------
//
// R4: the ONLY network operation in the AI layer is ai_model_install, and it
// runs only from explicit user action. Nothing here sends telemetry, queries,
// document content or embeddings anywhere.

/// Install the pinned embedding model: download what is missing, verify every
/// file's sha256, register, then load. Streams progress; cancellable.
///
/// OFFLINE INSTALL: if the files are already in the app data dir with matching
/// hashes, this registers and loads with NO network call at all.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn ai_model_install(
    state: State<'_, AppState>,
    on_event: tauri::ipc::Channel<crate::ai::model_install::InstallEvent>,
) -> Result<crate::ai::model_install::InstallReport, GaplyError> {
    let db = state.db.clone();
    let dir = state.app_data_dir.clone();
    let slot = state.ai_embed.clone();
    let cancel = state.ai_install_cancel.clone();
    cancel.store(false, std::sync::atomic::Ordering::SeqCst); // never poison the next run
    let report = tokio::task::spawn_blocking(move || {
        let emit = move |ev| {
            let _ = on_event.send(ev); // the user may have navigated away
        };
        let r = crate::ai::model_install::install(&db, &dir, &cancel, &emit)?;
        slot.reload(&dir); // load what we just verified — still no network
        Ok::<_, GaplyError>(r)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("ai_model_install task panicked: {e}")))??;
    Ok(report)
}

/// Install a PINNED generative candidate, by registry id.
///
/// Same contract as `ai_model_install`: explicit user action only, never at
/// startup, cancellable, and an offline install (files already present with
/// matching hashes) registers with no network call at all.
///
/// Unlike the embedding install this can be a multi-GB download, so it is
/// RESUMABLE — cancelling and re-running continues from where it stopped.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn ai_model_install_generative(
    state: State<'_, AppState>,
    model_id: String,
    on_event: tauri::ipc::Channel<crate::ai::gen_install::GenInstallEvent>,
) -> Result<crate::ai::gen_install::GenInstallReport, GaplyError> {
    // Resolve BEFORE spawning: an unknown id is a caller error and should come
    // back immediately, naming what is actually on offer, rather than failing
    // inside a background task.
    let candidate = crate::ai::gen_install::candidate(&model_id).ok_or_else(|| {
        GaplyError::Validation(format!(
            "unknown generative model {model_id:?}. Available: {}",
            crate::ai::gen_install::CANDIDATES
                .iter()
                .map(|c| c.registry_id)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    let db = state.db.clone();
    let dir = state.app_data_dir.clone();
    let cancel = state.ai_install_cancel.clone();
    cancel.store(false, std::sync::atomic::Ordering::SeqCst); // never poison the next run
    let report = tokio::task::spawn_blocking(move || {
        let emit = move |ev| {
            let _ = on_event.send(ev); // the user may have navigated away
        };
        crate::ai::gen_install::install(&db, &dir, candidate, &cancel, &emit)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("ai_model_install_generative task panicked: {e}")))??;
    Ok(report)
}

/// What generative candidates exist, and which are already installed.
///
/// Read-only and network-free: it answers from the pinned table and the
/// filesystem, so a UI can offer a choice without touching the network.
#[tauri::command]
pub fn ai_generative_candidates(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, GaplyError> {
    Ok(crate::ai::gen_install::CANDIDATES
        .iter()
        .map(|c| {
            let dir = crate::ai::gen_install::model_dir(&state.app_data_dir, c);
            serde_json::json!({
                "modelId": c.registry_id,
                "displayName": c.display_name,
                "quant": c.quant,
                "paramsB": c.params_b,
                "needs16gb": c.needs_16gb,
                "totalBytes": c.total_bytes(),
                "installed": crate::ai::gen_install::all_files_verified(&dir, c),
            })
        })
        .collect())
}

#[tauri::command]
pub fn ai_model_install_cancel(state: State<'_, AppState>) {
    state.ai_install_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[derive(Debug, Clone, serde::Serialize)]
// `rename_all` renames VARIANTS ONLY — struct-variant FIELDS keep their Rust
// snake_case unless `rename_all_fields` says otherwise. Without it this enum
// serialized `max_tokens`, `total_bytes`, `queued_behind` … while every
// TypeScript reader asked for the camelCase spelling and silently got
// `undefined`. Pinned by `event_wire_shape` tests.
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AiEmbedEvent {
    Started { pending: usize },
    Progress { done: usize, total: usize },
    Cancelled { done: usize, total: usize },
    Done { embedded: usize },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AiEmbedReport {
    pub document_id: Option<i64>,
    pub embedded: usize,
    pub remaining: usize,
    pub cancelled: bool,
}

/// Embed a document's un-embedded chunks. EXPLICIT, NEVER AUTOMATIC — importing
/// or indexing a PDF does not trigger this; only a user action does.
///
/// Resumable by construction: the pending set is computed from the absence of a
/// vector, so a cancelled run leaves exactly the un-embedded rows and the next
/// run picks up there. There is no progress cursor to fall out of step.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn ai_embed_document(
    state: State<'_, AppState>,
    document_id: Option<i64>,
    on_event: tauri::ipc::Channel<AiEmbedEvent>,
) -> Result<AiEmbedReport, GaplyError> {
    use gaply_core::ai_engine::embeddings as core_emb;
    let db = state.db.clone();
    let slot = state.ai_embed.clone();
    let cancel = state.ai_embed_cancel.clone();
    cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    tokio::task::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        let space = crate::ai::embedding_space();
        let pending = core_emb::chunks_missing_embeddings(&db, document_id, &space.model_id)?;
        let total = pending.len();
        let _ = on_event.send(AiEmbedEvent::Started { pending: total });

        let mut done = 0usize;
        for batch in pending.chunks(crate::ai::EMBED_BATCH_SIZE) {
            if cancel.load(Ordering::SeqCst) {
                let _ = on_event.send(AiEmbedEvent::Cancelled { done, total });
                let remaining =
                    core_emb::chunks_missing_embeddings(&db, document_id, &space.model_id)?.len();
                return Ok(AiEmbedReport { document_id, embedded: done, remaining, cancelled: true });
            }
            let texts: Vec<String> = batch.iter().map(|p| p.content.clone()).collect();
            let vectors = slot.with(|e| e.embed_documents(&texts))?;
            let rows: Vec<(i64, Vec<f32>)> =
                batch.iter().map(|p| p.chunk_id).zip(vectors).collect();
            core_emb::put_embeddings(&db, &space, &rows)?;
            done += rows.len();
            let _ = on_event.send(AiEmbedEvent::Progress { done, total });
        }
        let _ = on_event.send(AiEmbedEvent::Done { embedded: done });
        Ok(AiEmbedReport { document_id, embedded: done, remaining: 0, cancelled: false })
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("ai_embed_document task panicked: {e}")))?
}

/// What an import is about to cost, BEFORE anything is spent on it.
///
/// Called by every surface that is about to index a file, so the user learns
/// the price while the decision is still theirs. Cheap relative to what it
/// guards: it parses, but it does not chunk, write a `documents` row or touch
/// the embedding engine — and embedding is the part that costs two orders of
/// magnitude more (a debug/release measurement that is now in CLAUDE.md).
///
/// A scanned PDF is refused HERE, with the OCR advice, which is the whole
/// reason the check moved up: finding out after a progress bar has run is
/// finding out too late to have saved anything.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ai_import_preflight(
    state: State<'_, AppState>,
    path: String,
) -> Result<gaply_core::import_guard::ImportPreflight, GaplyError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        gaply_core::import_guard::preflight(
            &db,
            std::path::Path::new(&path),
            gaply_core::now_epoch(),
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("import preflight panicked: {e}")))?
}

/// Stages of linking a source file to a citation.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LinkSourceEvent {
    Parsing,
    Indexed { chunks: usize },
    Embedding { done: usize, total: usize },
    Linked { document_id: i64 },
}

/// Link a local file to a citation as its source: create the document row,
/// index it, embed it, and record the link as MANUAL.
///
/// # Why one command and not four
///
/// The four steps already exist separately (`ai_index_document`,
/// `ai_embed_document`, `citation_links::link_manually`) and a UI could call
/// them in order — but a citation whose document is created and indexed and NOT
/// embedded is exactly the `unverifiable` state the user was trying to leave,
/// and a half-linked source is worse than an unlinked one because it looks
/// done. This composes them so the outcome is all-or-nothing from the user's
/// side, and streams the stages because embedding a thesis is not instant.
///
/// `matched_by = 'manual'` is the point: the user asserted this link, so it
/// outranks the DOI and title heuristics and is never silently re-derived.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn ai_link_source_document(
    state: State<'_, AppState>,
    citation_id: String,
    path: String,
    title: Option<String>,
    // The user's answer to a preflight that asked. `None`/`false` is not an
    // error — it is only refused when the preflight actually requires one.
    confirmed: Option<bool>,
    on_event: tauri::ipc::Channel<LinkSourceEvent>,
) -> Result<serde_json::Value, GaplyError> {
    use gaply_core::ai_engine::embeddings as core_emb;
    use gaply_core::ai_engine::store;

    let db = state.db.clone();
    let slot = state.ai_embed.clone();
    let cancel = state.ai_embed_cancel.clone();
    cancel.store(false, std::sync::atomic::Ordering::SeqCst);

    tokio::task::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        let file = std::path::PathBuf::from(&path);
        if !file.is_file() {
            return Err(GaplyError::NotFound {
                entity: "source file",
                id: file.display().to_string(),
            });
        }

        // 0. Preflight. Enforced HERE, not only in the UI: a size gate that
        //    lives in the frontend is a suggestion, and this one exists to stop
        //    the machine spending twenty minutes on something nobody chose. It
        //    also refuses a scanned PDF with the OCR advice before a `documents`
        //    row exists — which is the whole point of moving the check up.
        let now = gaply_core::now_epoch();
        let pre = gaply_core::import_guard::preflight(&db, &file, now)?;
        if pre.is_refused() {
            return Err(GaplyError::Validation(pre.summary.clone()));
        }
        if pre.needs_confirmation() && !confirmed.unwrap_or(false) {
            // A distinct code so the UI can offer the choice rather than
            // rendering this as a failure — the user may always say yes.
            return Ok(serde_json::json!({
                "outcome": "confirmationRequired",
                "preflight": pre,
            }));
        }
        let started = std::time::Instant::now();

        // 1. Parse. Blocking, and the first thing that can honestly fail — an
        //    unreadable file must not leave a documents row behind.
        let _ = on_event.send(LinkSourceEvent::Parsing);
        let blocks = gaply_core::extract::docparse::parse_path_paged(&file)?;

        let display_title = title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
            file.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled source").to_string()
        });
        // Path-derived, so re-linking the same file finds the same row rather
        // than growing a duplicate every time.
        let checksum = format!("manual-{}", store::content_hash(&file.display().to_string()));
        let document_id =
            store::create_document(&db, &display_title, &file.display().to_string(), &checksum)?;

        // 2. Index. Idempotent by database constraint, so a re-run is safe.
        let chunks = gaply_core::chunk::chunk_paged_default(&blocks);
        let outcome = store::index_chunks(&db, document_id, &chunks)?;
        let _ = on_event.send(LinkSourceEvent::Indexed { chunks: outcome.inserted });

        // 3. Embed. Retrieval needs the vectors — an indexed but unembedded
        //    source is still unverifiable (audit_prepass says so explicitly).
        let space = crate::ai::embedding_space();
        let pending = core_emb::chunks_missing_embeddings(&db, Some(document_id), &space.model_id)?;
        let total = pending.len();
        let mut done = 0usize;
        for batch in pending.chunks(crate::ai::EMBED_BATCH_SIZE) {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            let texts: Vec<String> = batch.iter().map(|p| p.content.clone()).collect();
            let vectors = slot.with(|e| e.embed_documents(&texts))?;
            let rows: Vec<(i64, Vec<f32>)> = batch.iter().map(|p| p.chunk_id).zip(vectors).collect();
            core_emb::put_embeddings(&db, &space, &rows)?;
            done += rows.len();
            let _ = on_event.send(LinkSourceEvent::Embedding { done, total });
        }

        // 4. Link. Recorded even when embedding was cancelled part-way: the
        //    link is true either way, and `checkable_document_for_citation`
        //    decides separately whether there are enough vectors to check
        //    against. Claiming the link failed would be the lie.
        gaply_core::citation_links::link_manually(&db, &citation_id, document_id)?;
        let _ = on_event.send(LinkSourceEvent::Linked { document_id });

        let checkable =
            gaply_core::citation_links::checkable_document_for_citation(&db, &citation_id)?
                .is_some();

        // What this machine ACTUALLY did, folded into the next estimate. Only
        // on a complete run: a cancelled embed did less work than the pages
        // imply and would teach the estimator that the machine is fast.
        if total == done {
            let _ = gaply_core::import_guard::record_rate(
                &db,
                pre.page_equivalents,
                started.elapsed().as_secs_f64(),
                now,
            );
        }

        Ok(serde_json::json!({
            "outcome": "ok",
            "pages": pre.pages,
            "elapsedSeconds": started.elapsed().as_secs_f64(),
            "documentId": document_id,
            "title": display_title,
            "chunksIndexed": outcome.inserted,
            "chunksEmbedded": done,
            "chunksPending": total.saturating_sub(done),
            // The only question the caller actually cares about: can a support
            // check run against this source now?
            "checkable": checkable,
        }))
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("link source task panicked: {e}")))?
}

/// Progress through a batch of open-access fetches.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum OaFetchEvent {
    /// How many sources the batch will attempt, before anything is requested.
    Started { total: usize },
    /// About to look one source up. Named so a person watching a batch of
    /// twelve can see WHICH paper is being asked about, not just a count.
    Fetching { index: usize, total: usize, title: Option<String> },
    /// One source finished, with its own outcome.
    Done { index: usize, total: usize, report: crate::oa_fetch::FetchReport },
    /// What the CURRENT source is doing (§11 D105). Fetching a 37-page paper
    /// spends most of its time between `Fetching` and `Done`; without this the
    /// surface has nothing to say for the whole of it.
    Phase { index: usize, total: usize, phase: crate::oa_fetch::FetchPhase },
}

/// Fetch open-access full text for one or more citations.
///
/// # The third permitted network operation (plan §1, R4)
///
/// Outbound: a DOI, to Unpaywall and OpenAlex. Nothing else — not the
/// manuscript, not the claim being checked, not the title, not the user. It
/// runs only from this command, and this command runs only when a person
/// presses a button that says what it will do. There is no scheduler and no
/// prefetch; `oa_fetch`'s own startup test asserts as much.
///
/// # One command for both surfaces
///
/// "Fetch open-access PDF" for one source and "Fetch available PDFs for these N
/// sources" are the same operation over a list of one or a list of N. A second
/// command would be a second place for the outcome vocabulary to drift, and the
/// outcomes are the whole point: a batch reports twelve answers, not a count.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn citation_fetch_oa(
    state: State<'_, AppState>,
    subjects: Vec<crate::oa_fetch::FetchSubject>,
    on_event: tauri::ipc::Channel<OaFetchEvent>,
) -> Result<Vec<crate::oa_fetch::FetchReport>, GaplyError> {
    use crate::oa_fetch::{fetch_one, FetchDeps, FetchSubject, FetchTarget};

    // WHERE DID IT STOP? (§11 D137)
    //
    // This command had NO tracing event inside its span, so a press that failed
    // before any network work left the log exactly as a press that never
    // happened — the same blind spot §11 D136 fixed one command over, and the
    // reason a live failure could not be located. Every stage boundary below now
    // says it was reached.
    tracing::info!(subjects = subjects.len(), "oa fetch requested");
    if subjects.is_empty() {
        return Ok(Vec::new());
    }

    let db = state.db.clone();
    let slot = state.ai_embed.clone();
    let app_data_dir = state.app_data_dir.clone();

    tokio::task::spawn_blocking(move || {
        // Read each source's identity from OUR OWN STORE first — the library for
        // a citation, `audit_staged_sources` for a staged reference. A DOI the
        // caller handed us is a DOI nobody checked; the frontend passes an id,
        // never bibliographic data (§11 D132).
        //
        // ONE command for both, deliberately: "fetch this one" and "fetch these
        // twelve" were already the same operation over a list, and a second
        // command would be a second place for the outcome vocabulary to drift.
        // A second command per SUBJECT KIND would be the same mistake twice.
        let targets: Vec<FetchTarget> = subjects
            .iter()
            .map(|subject| match subject {
                FetchSubject::Citation { citation_id } => {
                    let stored = gaply_core::citation_library::get(&db, citation_id)?;
                    Ok(FetchTarget {
                        subject: subject.clone(),
                        doi: stored.as_ref().and_then(|r| r.doi.clone()),
                        title: stored
                            .as_ref()
                            .map(|r| r.title.clone())
                            .filter(|t| !t.trim().is_empty()),
                    })
                }
                FetchSubject::Staged { staged_id } => {
                    let stored = gaply_core::staged_sources::get(&db, *staged_id)?;
                    Ok(FetchTarget {
                        subject: subject.clone(),
                        doi: stored.as_ref().and_then(|r| r.doi.clone()),
                        title: stored.as_ref().and_then(|r| r.title.clone()),
                    })
                }
            })
            .collect::<Result<Vec<_>, GaplyError>>()
            .inspect_err(|e| tracing::warn!(error = %e, "oa fetch: target build failed"))?;
        tracing::info!(
            targets = targets.len(),
            with_doi = targets.iter().filter(|t| t.doi.is_some()).count(),
            "oa fetch targets built"
        );

        // Which source the phase updates belong to. `FetchDeps` is built once,
        // outside the loop, so the index the sink reports has to be read at call
        // time rather than captured.
        let current = std::cell::Cell::new(0usize);
        let total_for_phase = targets.len();
        let on_phase = |phase: crate::oa_fetch::FetchPhase| {
            let _ = on_event.send(OaFetchEvent::Phase {
                index: current.get(),
                total: total_for_phase,
                phase,
            });
        };

        // Both can fail, and both failed INVISIBLY before this: a `?` here ends
        // the command with an error the caller may or may not render.
        let http = crate::http_fetcher::ReqwestFetcher::new()
            .inspect_err(|e| tracing::warn!(error = %e, "oa fetch: http client build failed"))?;
        let bytes = crate::paper_corpus::ReqwestPaperFetcher::new()
            .inspect_err(|e| tracing::warn!(error = %e, "oa fetch: paper fetcher build failed"))?;
        tracing::info!("oa fetch deps ready");
        let limiters = gaply_core::refverify::ApiRateLimiters::with_polite_defaults();
        let deps = FetchDeps {
            db: &db,
            http: &http,
            bytes: &bytes,
            limiters: &limiters,
            // No contact email is configured in the product today, so Unpaywall
            // is skipped rather than sent a request it is certain to refuse.
            // OpenAlex answers the same question without one.
            contact_email: None,
            app_data_dir: &app_data_dir,
            on_phase: &on_phase,
        };
        let embed = |texts: &[String]| slot.with(|e| e.embed_documents(texts));

        let total = targets.len();
        let _ = on_event.send(OaFetchEvent::Started { total });
        let now = gaply_core::now_epoch();
        let mut reports = Vec::with_capacity(total);
        for (i, t) in targets.iter().enumerate() {
            current.set(i);
            let _ = on_event.send(OaFetchEvent::Fetching {
                index: i,
                total,
                title: t.title.clone(),
            });
            // Never `?`: one publisher's 403 must not end a batch of twelve.
            // Every failure mode is already an OUTCOME.
            let report = fetch_one(&deps, t, now, &embed);
            // One line per source, by OUTCOME KIND — 26 lines is the point: a
            // batch that answers "paywalled" 26 times is a different fact from
            // one that never ran, and only the log can tell them apart after the
            // window is closed.
            tracing::info!(
                index = i,
                total,
                subject = ?t.subject,
                outcome = report.outcome.kind(),
                "oa fetch source done"
            );
            let _ = on_event.send(OaFetchEvent::Done {
                index: i,
                total,
                report: report.clone(),
            });
            reports.push(report);
        }
        Ok(reports)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("open-access fetch task panicked: {e}")))?
}

#[tauri::command]
pub fn ai_embed_cancel(state: State<'_, AppState>) {
    state.ai_embed_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Semantic search: embed the query (with the query prefix), FTS prefilter,
/// cosine rerank, top k.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ai_semantic_search(
    state: State<'_, AppState>,
    query: String,
    document_id: Option<i64>,
    k: Option<usize>,
) -> Result<gaply_core::ai_engine::retrieval::SearchResult, GaplyError> {
    let db = state.db.clone();
    let slot = state.ai_embed.clone();
    let k = k.unwrap_or(10).clamp(1, 100);
    tokio::task::spawn_blocking(move || {
        let qv = slot.with(|e| e.embed_query(&query))?;
        gaply_core::ai_engine::retrieval::semantic_search(&db, &query, &qv, document_id, k)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("ai_semantic_search task panicked: {e}")))?
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

/// Read-only: the stored full_text of a paper in the plagiarism "my papers"
/// library — for Note Creator's OPTIONAL side-by-side reading. Thin delegation to
/// the M2 Set 2C resolution chain: RELIABLE id match (via the note's `paper_id` →
/// citation_id) first, then Set-1's honest title fallback, else None. Both
/// underlying reads are exactly-one-guarded — never the wrong paper. `paper_id`
/// empty/None (free-typed notes) uses the title fallback exactly as before.
/// Local, deterministic, no network. None → no full text (editor shows notes alone).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn note_paper_fulltext(
    state: State<'_, AppState>,
    title: String,
    paper_id: Option<String>,
) -> Result<Option<String>, GaplyError> {
    gaply_core::plagiarism_library::full_text_for_note(&state.db, &title, paper_id.as_deref())
}

/// Diagnostic-only: record the SHAPE of an OAuth deep-link callback the app
/// received, so a "silently stuck on login" case is diagnosable from
/// terminal/Console without guesswork. REDACTED BY CONSTRUCTION — the frontend
/// passes only structure (scheme/host/path) and boolean presence + location of
/// params; the actual `code`/token VALUES never cross this boundary and are
/// never logged. INFO level, one line per callback.
#[tauri::command]
pub fn log_auth_callback(
    scheme: String,
    host: String,
    path: String,
    has_code: bool,
    has_error: bool,
    has_access_token: bool,
    has_refresh_token: bool,
    param_location: String,
) {
    tracing::info!(
        target: "app_lib::auth",
        %scheme,
        %host,
        %path,
        has_code,
        has_error,
        has_access_token,
        has_refresh_token,
        %param_location,
        "oauth deep-link callback received"
    );
}

/// Diagnostic-only companion to `log_auth_callback`: record whether the PKCE
/// code-verifier is present in the auth store at a given OAuth stage
/// ("after_signin" = write side / "before_exchange" = read side). Boolean only —
/// the verifier VALUE is never read or logged. Pins a "verifier not found"
/// failure to the write side vs the read side.
#[tauri::command]
pub fn log_auth_probe(stage: String, verifier_present: bool) {
    tracing::info!(
        target: "app_lib::auth",
        %stage,
        verifier_present,
        "oauth verifier probe"
    );
}

/// Diagnostic-only: an auth-storage write failed. Logged (not swallowed) so a
/// failed verifier/session persist is never invisible. REDACTED — logs the KIND
/// of key and the error message, never the key's value.
#[tauri::command]
pub fn log_storage_error(operation: String, key_kind: String, message: String) {
    tracing::warn!(
        target: "app_lib::auth",
        %operation,
        %key_kind,
        %message,
        "auth storage write failed"
    );
}

/// Read a citation-IMPORT file's TEXT for client-side parsing (Citation Manager,
/// Set 2b). The Citation Manager parses .bib / .ris / CSL-JSON in the webview
/// (citation-js), which needs the file's text — every other file lane hands a
/// PATH to the core, but import is client-side.
///
/// PURPOSE-SCOPED, NOT a general file reader — do NOT widen it into
/// `read_text_file`: it accepts ONLY the import extensions (.bib/.bibtex/.ris/
/// .json), returns TEXT (never bytes), and refuses files over IMPORT_MAX_BYTES
/// (the size cap enforced server-side, mirroring importCitations' entry cap). A
/// future need for a different file's text gets its OWN scoped command.
#[tauri::command]
pub fn read_import_file(path: String) -> Result<String, GaplyError> {
    const IMPORT_MAX_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB — even a huge .bib is small text
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "bib" | "bibtex" | "ris" | "json") {
        return Err(GaplyError::Validation(format!(
            "Unsupported import file type: .{ext} — Gaply imports .bib, .ris, or .json"
        )));
    }
    let meta = std::fs::metadata(p)?;
    if meta.len() > IMPORT_MAX_BYTES {
        return Err(GaplyError::Validation(format!(
            "Import file is too large ({} bytes; max {IMPORT_MAX_BYTES}). Split it and import in batches.",
            meta.len()
        )));
    }
    Ok(std::fs::read_to_string(p)?)
}

/// Export the AI Check report as a PDF the user saves themselves. Takes the
/// already-built, SELF-CONTAINED report HTML (inline CSS, no assets — produced by
/// the frontend's `buildAiCheckReportHtml`), writes it to a temp file, and opens
/// it in the DEFAULT BROWSER via the opener plugin's Rust API, where the user
/// presses ⌘P → "Save as PDF".
///
/// WHY the browser: JS `window.print()`, the framework's `webview.print()`, AND
/// ⌘P are all no-ops in wry/WKWebView (empirically proven) — the Tauri webview
/// simply has no print surface. The browser is the honest print path.
///
/// The opener plugin's Rust API (`open_path`) is called here, in trusted backend
/// code, so it is NOT bound by the webview opener scope (which forbids `file://`
/// and `open_path`); no capability change is needed. PURPOSE-SCOPED: it accepts
/// report HTML and writes+opens ONE temp file, nothing else — not a general file
/// writer. Sweeps prior `gaply-aicheck-*.html` first so temp never accumulates.
#[tauri::command]
pub fn export_report(app: tauri::AppHandle, html: String) -> Result<(), GaplyError> {
    use tauri_plugin_opener::OpenerExt;
    const MAX_HTML_BYTES: usize = 8 * 1024 * 1024; // a report is small; guard runaway input
    if html.len() > MAX_HTML_BYTES {
        return Err(GaplyError::Validation(format!(
            "Report HTML too large ({} bytes; max {MAX_HTML_BYTES}).",
            html.len()
        )));
    }
    let dir = std::env::temp_dir();
    // Sweep prior exports so the temp dir doesn't grow one stale report per click.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("gaply-aicheck-") && name.ends_with(".html") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    // Fresh file name per export (epoch) → the browser never serves a cached
    // older report from the same file:// URL.
    let file = dir.join(format!("gaply-aicheck-{}.html", now_epoch()));
    std::fs::write(&file, html.as_bytes())?;
    app.opener()
        .open_path(file.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| GaplyError::Internal(format!("failed to open report in browser: {e}")))?;
    Ok(())
}

/// Write the PublishReady PDF for `report_id` to disk and open it.
///
/// # Why the bytes come from memory rather than from a store
///
/// They were rendered during the run (§51). The model they came from carries
/// manuscript prose and is deliberately not `Serialize` (§4.22), so it cannot
/// be rebuilt from the cached JSON — that carries findings but no statistics
/// block, no similarity regions, no lane record and no quotations.
///
/// **A miss is honest, not fatal:** the report is still in the viewer, and
/// re-running produces the bytes again. The alternative — persisting rendered
/// prose — is what §4.22 exists to prevent.
///
/// Uses `tauri-plugin-opener`, already a dependency, exactly as `export_report`
/// does; `tauri-plugin-dialog` is not present and adding it is not needed to
/// deliver the file.
#[tauri::command]
#[tracing::instrument(skip(state, app))]
pub fn export_publishready_pdf(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    report_id: String,
) -> Result<String, GaplyError> {
    use tauri_plugin_opener::OpenerExt;

    let bytes = {
        let q = state
            .report_pdfs
            .lock()
            .map_err(|_| GaplyError::Internal("report cache poisoned".into()))?;
        q.iter().find(|(id, _)| id == &report_id).map(|(_, b)| b.clone())
    };
    let bytes = bytes.ok_or_else(|| {
        GaplyError::Validation(
            "This report's PDF is no longer in memory. Run the analysis again to export it."
                .into(),
        )
    })?;

    let dir = std::env::temp_dir();
    // Sweep prior exports so the temp dir does not grow one stale PDF per click.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("gaply-publishready-") && name.ends_with(".pdf") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let file = dir.join(format!("gaply-publishready-{report_id}-{}.pdf", now_epoch()));
    std::fs::write(&file, &bytes)?;
    let path = file.to_string_lossy().to_string();
    app.opener()
        .open_path(path.clone(), None::<String>)
        .map_err(|e| GaplyError::Internal(format!("failed to open the report: {e}")))?;
    Ok(path)
}

/// Research Gap Finder (Set 2): build the session's paper corpus — N uploaded
/// files + N links → bounded, llm_safe per-paper digests (stable ids p1…pN)
/// + a per-session RAG ingest. NO reasoning, NO LLM, NO model load (the
/// one-at-a-time lifecycle is untouched); link fetches are rate-limited per
/// host with capped downloads. Caps reject oversize input honestly.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn build_gapfinder_corpus(
    state: State<'_, AppState>,
    session: String,
    paths: Option<Vec<String>>,
    links: Option<Vec<String>>,
) -> Result<crate::paper_corpus::CorpusReport, GaplyError> {
    // async + spawn_blocking (mirrors run_full_analysis): link fetches + PDF
    // parsing are blocking, so run off the event-loop thread. Extract the Arc
    // handles first (State<'_> isn't Send). Logic unchanged.
    let db = state.db.clone();
    let embedder = state.embedder.clone();
    tokio::task::spawn_blocking(move || {
        let fetcher = crate::paper_corpus::ReqwestPaperFetcher::new()?;
        // Same polite per-host budget as guidelines ingestion.
        let limiter = gaply_core::ratelimit::RateLimiter::new(5.0, 1.0);
        crate::paper_corpus::build_corpus(
            &db,
            embedder.as_ref(),
            &session,
            &paths.unwrap_or_default(),
            &links.unwrap_or_default(),
            &fetcher,
            &limiter,
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("gapfinder corpus task panicked: {e}")))?
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
pub async fn run_gap_finder(
    session: String,
    corpus: serde_json::Value,
    user_token: Option<String>,
) -> Result<GapFinderOutcome, GaplyError> {
    // async + spawn_blocking (mirrors run_full_analysis / verify_reference): the
    // proxy client does blocking keychain + HTTP work, so it must run OFF the
    // event-loop thread — the window stays responsive during the call and the
    // 2s offline reachability probe. Body/logic is unchanged.
    tokio::task::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("gap finder task panicked: {e}")))?
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
pub async fn run_gapfinder_qa(
    session: String,
    corpus: serde_json::Value,
    grounded_gaps: serde_json::Value,
    constraints: serde_json::Value,
    latest_answer: String,
    user_token: Option<String>,
) -> Result<gaply_core::gap_finder_agent::QaTurn, GaplyError> {
    // async + spawn_blocking: keeps the blocking keychain/HTTP work off the
    // event-loop thread (mirrors run_full_analysis). Body/logic unchanged — the
    // firewall still short-circuits before any proxy probe.
    tokio::task::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("gapfinder qa task panicked: {e}")))?
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
pub async fn run_gapfinder_draft(
    session: String,
    corpus: serde_json::Value,
    achievable_gaps: serde_json::Value,
    constraints: serde_json::Value,
    user_note: String,
    user_token: Option<String>,
) -> Result<gaply_core::gap_finder_agent::DraftResult, GaplyError> {
    // async + spawn_blocking: blocking keychain/HTTP off the event-loop thread
    // (mirrors run_full_analysis). Body/logic unchanged.
    tokio::task::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("gapfinder draft task panicked: {e}")))?
}

/// Gap Finder (Set 6): verify a journal's facts STRICTLY from registry data
/// — OpenAlex /sources + the DOAJ API, refverify-style (rate limiter,
/// llm_safe, TTL cache, honest-unverified). NO LLM is involved in this
/// command at all: the card cannot contain a model-originated fact.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn verify_journal_registry(
    state: State<'_, AppState>,
    issn: String,
    name: Option<String>,
    local_predatory_signals: Option<Vec<String>>,
) -> Result<crate::journal_registry::JournalVerification, GaplyError> {
    // async + spawn_blocking (mirrors verify_reference): the registry APIs are
    // blocking HTTP, so run off the event-loop thread. Extract the Arc<Database>
    // first (State<'_> isn't Send). Logic unchanged.
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let fetcher = crate::http_fetcher::ReqwestFetcher::new()?;
        // Polite per-host budget, refverify-style.
        let limiter = gaply_core::ratelimit::RateLimiter::new(5.0, 1.0);
        crate::journal_registry::verify_journal(
            &db,
            &fetcher,
            &limiter,
            &issn,
            name.as_deref().unwrap_or(""),
            &local_predatory_signals.unwrap_or_default(),
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("verify journal registry task panicked: {e}")))?
}

/// Journal Verification (PAID): the combined command — grounded registry facts
/// (local retrieval) + the self-reported site summary (the ONE cloud/LLM use),
/// joined into one evidence result. Thin orchestration over the tested lanes.
///
/// Honest gate placement: the user's JWT rides to the PROXY for the server-side
/// entitlement gate on the LLM site-summary (the genuine cloud work) — mirroring
/// how the Stats Verifier gates its interpretive chat. The grounded registry
/// checks are LOCAL and run regardless, so an offline/unavailable proxy degrades
/// the site summary honestly and never blocks the facts. The page is fetched
/// ONCE and threaded to both lanes.
#[tauri::command]
#[tracing::instrument(skip(state, query, issn, local_predatory_signals, user_token))]
pub async fn verify_journal_full(
    state: State<'_, AppState>,
    query: Option<String>,
    issn: Option<String>,
    local_predatory_signals: Option<Vec<String>>,
    user_token: Option<String>,
) -> Result<crate::journal_verify::JournalVerificationResult, GaplyError> {
    // async + spawn_blocking (mirrors run_full_analysis): registry HTTP + the
    // proxy keychain/HTTP are blocking, so run off the event-loop thread. Extract
    // the Arc<Database> first (State<'_> isn't Send). Logic unchanged.
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let fetcher = crate::http_fetcher::ReqwestFetcher::new()?;
        let limiter = gaply_core::ratelimit::RateLimiter::new(5.0, 1.0);
        let client = match ProxyReqwestClient::from_env().map(|c| c.with_user_token(user_token)) {
            Ok(c) if c.reachable() => Some(c),
            _ => None,
        };
        let proxy = client.as_ref().map(|c| c as &dyn gaply_core::verify_agent::ProxyClient);
        crate::journal_verify::verify_journal_full(
            &db,
            &fetcher,
            &limiter,
            proxy,
            query.as_deref(),
            issn.as_deref(),
            &local_predatory_signals.unwrap_or_default(),
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("verify journal full task panicked: {e}")))?
}

/// Gap Finder (Set 6): journal-fit reasoning over the VERIFIED card. The
/// model reasons about fit using card facts by id; the gate drops any
/// invented journal/gap and the card itself is never modified. CLOUD-ONLY,
/// honest offline; user JWT rides for entitlement.
#[tauri::command]
#[tracing::instrument(skip(corpus, achievable_gaps, journal_card, user_token))]
pub async fn run_gapfinder_fit(
    session: String,
    corpus: serde_json::Value,
    achievable_gaps: serde_json::Value,
    journal_card: serde_json::Value,
    user_token: Option<String>,
) -> Result<gaply_core::gap_finder_agent::FitResult, GaplyError> {
    // async + spawn_blocking: blocking keychain/HTTP off the event-loop thread
    // (mirrors run_full_analysis). Body/logic unchanged.
    tokio::task::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("gapfinder fit task panicked: {e}")))?
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
pub async fn run_copilot_chat(
    context: serde_json::Value,
    question: String,
    language: Option<String>,
    user_token: Option<String>,
) -> Result<gaply_core::chat_agent::ChatTurn, GaplyError> {
    // async + spawn_blocking: blocking keychain/HTTP off the event-loop thread
    // (mirrors run_full_analysis). Body/logic unchanged — the firewall still
    // refuses ghostwriting before any proxy probe.
    tokio::task::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("copilot chat task panicked: {e}")))?
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
pub async fn run_stats_chat(
    path: String,
    spec: AnalysisSpec,
    manuscript_path: Option<String>,
    question: String,
    language: Option<String>,
    user_token: Option<String>,
) -> Result<gaply_core::stats_chat::StatsChatTurn, GaplyError> {
    // async + spawn_blocking: this parses the uploaded table/manuscript (blocking
    // I/O) and does blocking keychain/HTTP for the chat — all off the event-loop
    // thread (mirrors run_full_analysis). Body/logic unchanged.
    tokio::task::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("stats chat task panicked: {e}")))?
}

/* ===================== Phase 8: batch jobs ================================ *
 * Thin wrappers, per the gaply-core split: planning, persistence and reporting
 * all live in gaply-core; the runner lives in `ai::job_runner`; nothing here
 * does work of its own beyond wiring the Channel and the control registry. */

/// Streamed after every completed item — the `ai://job/{id}/progress` payload.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressEvent {
    pub job_id: i64,
    pub completed: i64,
    pub total: i64,
    pub current_category: String,
    pub latest_item_summary: String,
}

/// Plan and start a thesis citation audit.
///
/// Returns as soon as the PLAN is durable, not when the audit finishes — it
/// runs for hours, and a command that blocked for that long would be unusable.
/// The plan itself is the immediate, useful answer: how many items, of which
/// kinds, before any time is spent.
/// The manuscript's own bytes, for the annotated view (§11 D92).
///
/// PDF ONLY, and that is the feature's boundary rather than a limitation of
/// this command: a `.docx` records no page geometry, so there is no page to
/// annotate. Guarded the same way `read_import_file` is — extension and size,
/// checked before the read — because this takes a path from the caller.
#[tauri::command]
pub async fn ai_manuscript_bytes(path: String) -> Result<tauri::ipc::Response, GaplyError> {
    // A thesis PDF is large; a 200 MB one is not a manuscript and must not be
    // pulled through the IPC boundary into a webview.
    const MAX_BYTES: u64 = 128 * 1024 * 1024;
    let bytes = tokio::task::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if ext != "pdf" {
            return Err(GaplyError::Validation(format!(
                "the annotated view needs a PDF; this manuscript is a .{ext}. A Word file does \
                 not record where its pages break, so there is no page layout to annotate."
            )));
        }
        if !p.is_file() {
            return Err(GaplyError::Validation(format!(
                "the manuscript is no longer at {} — it was moved, renamed or deleted",
                p.display()
            )));
        }
        let meta = std::fs::metadata(p)?;
        if meta.len() > MAX_BYTES {
            return Err(GaplyError::Validation(format!(
                "this PDF is {} bytes; the annotated view handles up to {MAX_BYTES}",
                meta.len()
            )));
        }
        std::fs::read(p).map_err(GaplyError::from)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("manuscript read task panicked: {e}")))??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// What an audit WOULD do — no job, no model, no side effect (§11 D88).
///
/// `ai_job_start_thesis_audit` plans AND spawns the runner, so it cannot be the
/// thing behind a "start?" card: by the time the card renders, generation has
/// begun. This is what the card asks instead.
#[tauri::command]
pub async fn ai_thesis_audit_preview(
    state: State<'_, AppState>,
    path: String,
) -> Result<gaply_core::ai_engine::thesis_audit::ThesisAuditPreview, GaplyError> {
    let db = state.db.clone();
    let manuscript = std::path::PathBuf::from(path);
    tokio::task::spawn_blocking(move || {
        gaply_core::ai_engine::thesis_audit::preview_thesis_audit(&db, &manuscript)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("audit preview panicked: {e}")))?
}

#[tauri::command]
pub async fn ai_job_start_thesis_audit(
    state: State<'_, AppState>,
    path: String,
    on_event: tauri::ipc::Channel<JobProgressEvent>,
) -> Result<serde_json::Value, GaplyError> {
    use gaply_core::ai_engine::thesis_audit;

    if let Some(loader) = crate::ai::generative::BundledGenerativeLoader::resolve() {
        crate::ai::generative::register_generative(&state.db, &loader)?;
    }

    let db = state.db.clone();
    let manuscript = std::path::PathBuf::from(path);
    let plan = tokio::task::spawn_blocking(move || {
        thesis_audit::plan_thesis_audit(
            &db,
            &manuscript,
            crate::ai::tasks::citation_support::PROMPT_VERSION,
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("audit planning panicked: {e}")))??;

    spawn_job_runner(&state, plan.job_id, on_event);
    Ok(serde_json::to_value(&plan).unwrap_or(serde_json::Value::Null))
}

/// Recent thesis audits, so a finished one can be REOPENED (§11 D139).
///
/// A completed audit is durable in the database and was ephemeral on screen: its
/// health, items, staged sources and both action buttons lived only while the
/// screen that started it stayed mounted. This is rehydration, not a new feature —
/// `ai_job_status` and `ai_job_results` already serve the contents.
///
/// Carries the staged counts because they are what the picker needs to say
/// something useful: "26 sources staged, 15 fetched" tells a reader which job is
/// worth reopening, where a row of ids does not.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ai_audit_jobs_recent(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<serde_json::Value>, GaplyError> {
    let db = state.db.clone();
    let out = tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>, GaplyError> {
        let jobs = gaply_core::ai_engine::jobs::recent_jobs(&db, "thesis_audit", limit.clamp(1, 50))?;
        let mut out = Vec::with_capacity(jobs.len());
        for j in jobs {
            let staged = gaply_core::staged_sources::list_for_job(&db, j.id)?;
            out.push(serde_json::json!({
                "jobId": j.id,
                "status": j.status.as_str(),
                "totalItems": j.total_items,
                "doneItems": j.done_items,
                "createdAt": j.created_at,
                "finishedAt": j.finished_at,
                // §11 D141. What the job audited. NULL for jobs created before
                // the column existed, and the screen must print those as unknown
                // rather than substituting a plausible name.
                "sourcePath": j.source_path,
                "stagedSources": staged.len(),
                "stagedFetched": staged.iter().filter(|s| s.document_id.is_some()).count(),
            }));
        }
        Ok(out)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("recent jobs read panicked: {e}")))??;
    tracing::info!(jobs = out.len(), "recent audit jobs read");
    Ok(out)
}

/// The reference entries this audit staged from the manuscript (§11 D134).
///
/// Read-only and deterministic: no model, no network. The button that fetches
/// them needs to say HOW MANY it is about to fetch before the researcher presses
/// it, and it cannot say that from a count alone — it needs the ids to send.
///
/// Staging happens when the audit plans; FETCHING DOES NOT. A researcher presses
/// the button. An audit that silently reached out for a dozen PDFs because it
/// parsed a reference list would be doing something nobody asked for.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ai_job_staged_sources(
    state: State<'_, AppState>,
    job_id: i64,
) -> Result<Vec<gaply_core::staged_sources::StagedSource>, GaplyError> {
    let db = state.db.clone();
    let rows = tokio::task::spawn_blocking(move || gaply_core::staged_sources::list_for_job(&db, job_id))
        .await
        .map_err(|e| GaplyError::Internal(format!("staged source read panicked: {e}")))??;
    // AN INVOCATION MUST BE VISIBLE (§11 D136).
    //
    // `#[tracing::instrument]` opens a SPAN; a span with no event inside it
    // writes nothing, so a successful call and a call that never happened looked
    // identical in the log. That cost a live diagnosis: the staged rows were
    // right, the command existed, and there was no way to tell whether the
    // screen had asked for them.
    //
    // `fetchable` is logged beside the total because it is what decides whether
    // the button appears at all — a read that returns 35 rows and 0 fetchable is
    // a different situation from one that returns nothing.
    let fetchable = rows.iter().filter(|s| s.doi.is_some() && s.document_id.is_none()).count();
    tracing::info!(job_id, staged = rows.len(), fetchable, "staged sources read");
    Ok(rows)
}

/// Register a planned job and run it in the background.
///
/// Shared by the whole-manuscript audit and the per-citation slice — they are
/// the SAME pipeline with different predicates (§11 D54), and two copies of
/// this would be two places for resume, preemption and the control registry to
/// drift apart.
fn spawn_job_runner(
    state: &State<'_, AppState>,
    job_id: i64,
    on_event: tauri::ipc::Channel<JobProgressEvent>,
) {
    let control = crate::ai::job_runner::JobControl::new(job_id);
    state.ai_jobs.lock().expect("job registry poisoned").insert(job_id, control.clone());

    // The runner outlives the command that started it by design.
    let db = state.db.clone();
    let manager = state.ai_gen.clone();
    let slot = state.ai_embed.clone();
    let priority = state.ai_interactive.clone();
    let registry = state.ai_jobs.clone();
    tokio::spawn(async move {
        let embed = move |claim: &str| slot.with(|e| e.embed_query(claim));
        let emit = move |p: crate::ai::job_runner::JobProgress| {
            let _ = on_event.send(JobProgressEvent {
                job_id: p.job_id,
                completed: p.completed,
                total: p.total,
                current_category: p.current_category,
                latest_item_summary: p.latest_item_summary,
            });
        };
        let outcome =
            crate::ai::job_runner::run_job(&db, &manager, &control, &priority, &embed, &emit).await;
        let paused = matches!(outcome, Ok(crate::ai::job_runner::RunOutcome::Paused));
        match &outcome {
            Ok(o) => tracing::info!(job_id, ?o, "job finished"),
            Err(e) => tracing::error!(job_id, %e, "job failed"),
        }
        // A finished job's control handle is dead weight; a paused one's is not.
        if !paused {
            registry.lock().expect("job registry poisoned").remove(&job_id);
        }
    });
}

/// Every manuscript sentence that cites ONE library entry — deterministically,
/// with no model and no job created.
///
/// This is what the user confirms before anything is spent. The pre-pass runs
/// no model (§11 D40), so the list costs a parse and nothing else, and a
/// citation nothing cites answers in the same breath instead of queueing an
/// empty job to find out.
#[tauri::command]
pub async fn ai_citation_audit_preview(
    state: State<'_, AppState>,
    citation_id: String,
    path: String,
) -> Result<serde_json::Value, GaplyError> {
    let db = state.db.clone();
    let manuscript = std::path::PathBuf::from(path);
    let preview = tokio::task::spawn_blocking(move || {
        gaply_core::ai_engine::thesis_audit::preview_citation_audit(&db, &manuscript, &citation_id)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("citation preview panicked: {e}")))??;
    Ok(serde_json::to_value(&preview).unwrap_or(serde_json::Value::Null))
}

/// Queue and run the per-citation slice. Same planner, same job rows, same
/// runner, same pause/resume/cancel commands as the thesis audit.
#[tauri::command]
pub async fn ai_citation_audit_start(
    state: State<'_, AppState>,
    citation_id: String,
    path: String,
    on_event: tauri::ipc::Channel<JobProgressEvent>,
) -> Result<serde_json::Value, GaplyError> {
    let db = state.db.clone();
    let manuscript = std::path::PathBuf::from(path);
    let plan = tokio::task::spawn_blocking(move || {
        gaply_core::ai_engine::thesis_audit::plan_citation_audit(
            &db,
            &manuscript,
            crate::ai::tasks::citation_support::PROMPT_VERSION,
            &citation_id,
        )
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("citation audit planning panicked: {e}")))??;

    spawn_job_runner(&state, plan.job_id, on_event);
    Ok(serde_json::to_value(&plan).unwrap_or(serde_json::Value::Null))
}

/// Export a finished audit as PDF or HTML.
///
/// Returns the bytes; the frontend writes them wherever the user chose. The
/// composer and both renderers live in gaply-core, so this command is a read
/// plus a render and owns no wording of its own.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn ai_job_export_report(
    state: State<'_, AppState>,
    job_id: i64,
    manuscript_name: String,
    format: String,
) -> Result<serde_json::Value, GaplyError> {
    let db = state.db.clone();
    let app_data = state.app_data_dir.clone();
    tokio::task::spawn_blocking(move || {
        // Passed in rather than read from a clock inside the composer, so a
        // report is reproducible and its tests are not time-dependent.
        let generated_on = crate::audit_export::today_label();
        // Only reached for jobs that predate model recording, and disclosed as
        // a guess on the cover when it is used.
        let installed = crate::ai::generative::resolve_generative_loader(&db, &app_data)
            .map(|l| l.model_id());
        let model = crate::audit_export::build_model_with(
            &db,
            job_id,
            &manuscript_name,
            &generated_on,
            installed.as_deref(),
        )?;
        let blocks = gaply_core::audit_report::compose_audit(&model);
        let (bytes, extension) = match format.as_str() {
            "pdf" => (gaply_core::report_pdf::render_pdf(&blocks), "pdf"),
            "html" => (
                gaply_core::report_html::render_html(&blocks, "Thesis citation audit")
                    .into_bytes(),
                "html",
            ),
            other => {
                return Err(GaplyError::Validation(format!(
                    "unknown export format {other:?} — expected \"pdf\" or \"html\""
                )))
            }
        };
        Ok(serde_json::json!({
            "bytes": bytes,
            "extension": extension,
            "suggestedName": format!(
                "{}-citation-audit.{extension}",
                manuscript_name.trim_end_matches(".pdf").replace(' ', "-")
            ),
        }))
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("report export panicked: {e}")))?
}

/// Re-queue only the items that became checkable, and run them.
///
/// The loop that turns a report with no evidence into one with evidence: the
/// user fetched the missing sources, so the sentences citing them can now be
/// checked for real. Only those items are re-queued — re-running the whole
/// audit would spend minutes re-judging sentences whose answers have not
/// changed, and would overwrite verdicts the user has already read.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn ai_job_recheck_items(
    state: State<'_, AppState>,
    job_id: i64,
    subjects: Vec<gaply_core::source_ref::SourceRef>,
    on_event: tauri::ipc::Channel<JobProgressEvent>,
) -> Result<serde_json::Value, GaplyError> {
    use gaply_core::ai_engine::{audit_prepass, jobs};

    let db = state.db.clone();
    let requeued: Vec<serde_json::Value> = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>, GaplyError> {
            let mut out = Vec::new();
            // THE MANUSCRIPT'S REFERENCE LIST, FROM THE STAGED ROWS (§11 D135).
            //
            // Resolution finds a staged document by the DOI its reference entry
            // prints, and a re-check has a job id but NOT the manuscript's path —
            // the export's own comment records that. The staged table IS that
            // list, persisted, so it is read back rather than the manuscript
            // being re-parsed or a second way to find a staged document being
            // grown beside the first.
            let staged_entries = gaply_core::staged_sources::as_author_year_entries(&db, job_id)?;
            let numbered = std::collections::BTreeMap::new();
            let bib = audit_prepass::Bibliography {
                numbered: &numbered,
                author_year: &staged_entries,
            };
            // Which unverifiable items now resolve to a checkable document? Asked
            // of the store, not assumed from the fetch's own report: a fetch that
            // succeeded and an item that can now be checked are different facts.
            let mut offset = 0i64;
            loop {
                let page = jobs::job_results(&db, job_id, offset, 200)?;
                if page.is_empty() {
                    break;
                }
                offset += page.len() as i64;
                for it in page {
                    if it.kind.as_str() != "unverifiable" {
                        continue;
                    }
                    for m in audit_prepass::markers_in(&it.sentence) {
                        // Does this marker NOW resolve to something checkable,
                        // and is it one of the sources just fetched?
                        //
                        // Asked of the STORE rather than taken from the fetch's
                        // report: a fetch that succeeded and an item that can now
                        // be checked are different facts. Narrowed to the
                        // fetched subjects so an unrelated item already checkable
                        // is not requeued.
                        let Ok(audit_prepass::Resolution::Checkable {
                            via,
                            document_id,
                            abstract_only,
                        }) =
                            audit_prepass::resolve_marker_with(&db, &m, &bib)
                        else {
                            continue;
                        };
                        if !subjects.contains(&via) {
                            continue;
                        }
                        out.push(serde_json::json!({
                            "seq": it.seq,
                            "documentId": document_id,
                            // `libraryId` stays for existing readers and is null
                            // for a staged source; `source` is the complete
                            // answer (§11 D133).
                            "libraryId": via.citation_id(),
                            "source": via,
                            // §11 D138. Travels with the requeue, so a re-checked
                            // item keeps saying which kind of source backed it.
                            "abstractOnly": abstract_only,
                        }));
                        break;
                    }
                }
            }
            Ok(out)
        })
        .await
        .map_err(|e| GaplyError::Internal(format!("recheck scan panicked: {e}")))??
    };

    if requeued.is_empty() {
        return Ok(serde_json::json!({ "requeued": 0, "items": [] }));
    }

    // One re-queue per item, because each carries its own document id.
    let db2 = db.clone();
    let items = requeued.clone();
    tokio::task::spawn_blocking(move || -> Result<(), GaplyError> {
        for it in &items {
            let seq = it["seq"].as_i64().unwrap_or(-1);
            jobs::requeue_items(
                &db2,
                job_id,
                &[seq],
                jobs::ItemKind::CitationSupport,
                &it.to_string(),
            )?;
        }
        Ok(())
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("recheck requeue panicked: {e}")))??;

    spawn_job_runner(&state, job_id, on_event);
    Ok(serde_json::json!({ "requeued": requeued.len(), "items": requeued }))
}

/// Status plus the Thesis Health report so far. Readable mid-job.
#[tauri::command]
pub async fn ai_job_status(
    state: State<'_, AppState>,
    job_id: i64,
) -> Result<serde_json::Value, GaplyError> {
    let db = state.db.clone();
    let running = state.ai_jobs.lock().expect("job registry poisoned").contains_key(&job_id);
    tokio::task::spawn_blocking(move || {
        let job = gaply_core::ai_engine::jobs::get_job(&db, job_id)?
            .ok_or_else(|| GaplyError::NotFound { entity: "job", id: job_id.to_string() })?;
        let health = gaply_core::ai_engine::thesis_audit::thesis_health(&db, job_id)?;
        Ok(serde_json::json!({
            "job": job,
            "runningInThisProcess": running,
            "health": health,
        }))
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("status task panicked: {e}")))?
}

#[tauri::command]
pub async fn ai_job_pause(state: State<'_, AppState>, job_id: i64) -> Result<bool, GaplyError> {
    let found = state
        .ai_jobs
        .lock()
        .expect("job registry poisoned")
        .get(&job_id)
        .map(|c| {
            c.pause();
            true
        })
        .unwrap_or(false);
    Ok(found)
}

/// Resume: reset anything a crash or pause left mid-flight, then run again.
#[tauri::command]
pub async fn ai_job_resume(
    state: State<'_, AppState>,
    job_id: i64,
    on_event: tauri::ipc::Channel<JobProgressEvent>,
) -> Result<serde_json::Value, GaplyError> {
    let db = state.db.clone();
    // §11 D38 — the item a crash left `running` goes back to the queue. The
    // count is reported rather than swallowed: at ~65 s an item it is the
    // honest measure of what the interruption cost.
    let requeued = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || gaply_core::ai_engine::jobs::resume_job(&db, job_id))
            .await
            .map_err(|e| GaplyError::Internal(format!("resume task panicked: {e}")))??
    };

    let control = {
        let mut reg = state.ai_jobs.lock().expect("job registry poisoned");
        let c = reg
            .entry(job_id)
            .or_insert_with(|| crate::ai::job_runner::JobControl::new(job_id))
            .clone();
        c.resume();
        c
    };

    let manager = state.ai_gen.clone();
    let slot = state.ai_embed.clone();
    let priority = state.ai_interactive.clone();
    let registry = state.ai_jobs.clone();
    tokio::spawn(async move {
        let embed = move |claim: &str| slot.with(|e| e.embed_query(claim));
        let emit = move |p: crate::ai::job_runner::JobProgress| {
            let _ = on_event.send(JobProgressEvent {
                job_id: p.job_id,
                completed: p.completed,
                total: p.total,
                current_category: p.current_category,
                latest_item_summary: p.latest_item_summary,
            });
        };
        let outcome =
            crate::ai::job_runner::run_job(&db, &manager, &control, &priority, &embed, &emit).await;
        if !matches!(outcome, Ok(crate::ai::job_runner::RunOutcome::Paused)) {
            registry.lock().expect("job registry poisoned").remove(&job_id);
        }
    });

    Ok(serde_json::json!({ "jobId": job_id, "requeuedItems": requeued }))
}

#[tauri::command]
pub async fn ai_job_cancel(state: State<'_, AppState>, job_id: i64) -> Result<bool, GaplyError> {
    let found = state
        .ai_jobs
        .lock()
        .expect("job registry poisoned")
        .get(&job_id)
        .map(|c| {
            c.cancel();
            true
        })
        .unwrap_or(false);
    Ok(found)
}

/// Results, paginated — readable while the job is still running.
#[tauri::command]
pub async fn ai_job_results(
    state: State<'_, AppState>,
    job_id: i64,
    offset: i64,
    limit: i64,
) -> Result<serde_json::Value, GaplyError> {
    let db = state.db.clone();
    // A caller asking for everything would pull 250 verbatim model outputs into
    // one IPC message; bounded here rather than trusted.
    let limit = limit.clamp(1, 200);
    tokio::task::spawn_blocking(move || {
        let items = gaply_core::ai_engine::jobs::job_results(&db, job_id, offset, limit)?;
        Ok(serde_json::json!({ "jobId": job_id, "offset": offset, "items": items }))
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("results task panicked: {e}")))?
}

/// Build or refresh `citation_library` → `documents` links (Phase 8b).
///
/// Deterministic and model-free. Run after importing citations or indexing
/// documents: it decides which cited works an audit can actually check, so the
/// report is returned rather than left silent.
#[tauri::command]
pub async fn ai_link_citations(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, GaplyError> {
    let db = state.db.clone();
    let report =
        tokio::task::spawn_blocking(move || gaply_core::citation_links::link_citations(&db))
            .await
            .map_err(|e| GaplyError::Internal(format!("link task panicked: {e}")))??;
    Ok(serde_json::to_value(&report).unwrap_or(serde_json::Value::Null))
}


/* ============== Phase 9a: serving a source document to the viewer ========= *
 * §11 D42 — the backend reads the file. The webview's fs capability is
 * $APPDATA-scoped and grants no read at all, so doing this in the frontend
 * would mean a new permission plus a scope widened to the user's whole
 * filesystem, bought for one screen. */

/// Where a document's file is, and whether it is still there.
///
/// Cheap enough to call per evidence row: an evidence row whose file has moved
/// must render as "file not found" rather than as a dead click.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSource {
    pub document_id: i64,
    pub path: String,
    pub exists: bool,
    /// Lowercased extension, so the UI knows whether the viewer can render it
    /// at all before it asks for bytes.
    pub extension: String,
}

#[tauri::command]
pub async fn ai_document_source(
    state: State<'_, AppState>,
    document_id: i64,
) -> Result<DocumentSource, GaplyError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let path = gaply_core::ai_engine::store::document_source(&db, document_id)?;
        let p = std::path::Path::new(&path);
        Ok(DocumentSource {
            document_id,
            exists: p.is_file(),
            extension: p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default(),
            path,
        })
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("document source task panicked: {e}")))?
}

/// The document's bytes, for the in-app viewer.
///
/// Returned as a raw IPC response rather than a JSON array: a 400-page thesis
/// is tens of megabytes, and base64-in-JSON would inflate it by a third and
/// cost a parse on both sides.
#[tauri::command]
pub async fn ai_document_bytes(
    state: State<'_, AppState>,
    document_id: i64,
) -> Result<tauri::ipc::Response, GaplyError> {
    let db = state.db.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        let path = gaply_core::ai_engine::store::document_source(&db, document_id)?;
        let p = std::path::Path::new(&path);
        if !p.is_file() {
            // NAMED, not a generic io error: "the file moved" is something the
            // user can act on, and the row already told them it was missing.
            return Err(GaplyError::Validation(format!(
                "the source file for this document is no longer at {path} — it was moved, \
                 renamed or deleted since it was indexed"
            )));
        }
        std::fs::read(p).map_err(GaplyError::from)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("document read task panicked: {e}")))??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Which indexed document backs this citation, if any (§11 D45).
///
/// `None` is a real answer, not a failure: it is what tells the citation panel
/// to say the source is not linked, rather than offering a check that cannot
/// run.
#[tauri::command]
pub async fn ai_citation_document(
    state: State<'_, AppState>,
    citation_id: String,
) -> Result<Option<serde_json::Value>, GaplyError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let found = gaply_core::citation_links::checkable_document_for_citation(&db, &citation_id)?;
        Ok(found.map(|(document_id, matched_by)| {
            serde_json::json!({ "documentId": document_id, "matchedBy": matched_by })
        }))
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("citation document task panicked: {e}")))?
}
