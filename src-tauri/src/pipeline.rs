//! Phase 3, Stage 2 — the ONE full-analysis command.
//!
//! `run_full_analysis` chains the real local agents exactly as the golden
//! `report/tests.rs` does, but as a PRODUCTION caller: extract → validate → ai
//! → plagiarism → rag → verify → debate → compile. Per-stage progress streams
//! to the frontend over a typed `tauri::ipc::Channel<AnalysisEvent>`; the
//! compiled report is cached under `report:{id}` for the report viewer (Stage 4).
//!
//! Honesty:
//! * The five local agents (extraction, validation, ai, plagiarism, rag) run
//!   for real (interim HeuristicModel/HashEmbedder stay until the SLM lands).
//! * The verify stage does REAL refverify HTTP (CrossRef/OpenAlex/… via the
//!   Stage-1 ReqwestFetcher) for reference existence/retraction. The cloud
//!   hallucination verdict goes through `verify_citations` as a real caller,
//!   but its `ProxyClient` is the mock until you deploy the proxy — so those
//!   verdicts come back UNKNOWN rather than fabricated.
//! * On ANY stage error we emit `Failed` (surfaced as a toast) and stop — never
//!   a fabricated completion.
//!
//! Threading: the whole pipeline is synchronous CPU + blocking IO, so it runs
//! inside one `tokio::task::spawn_blocking`. Nothing holds a lock across an
//! `.await`, so the std-guard-not-Send hazard never arises (a
//! `tokio::sync::Mutex` would be the tool if it did).

use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use gaply_core::embed::Embedder;
use gaply_core::extract::citations::Reference;
use gaply_core::refverify::ReferenceVerification;
use gaply_core::reviewer_agent::LaneExamination;
use gaply_core::extract::ExtractionResult;
use gaply_core::plagiarism::PlagiarismReport;
use gaply_core::report::{build_checklist, compile_report, PublishReadyReport};
use gaply_core::swarm::{
    adapters, run_debate, DebateConfig, PrecomputedAgent, RevisingVerificationAgent, SwarmAgent,
};
use gaply_core::verify_agent::{verify_citations, MockProxyClient};
use gaply_core::research_state::ResearchState;
use gaply_core::{ai_detect, extract, now_epoch, plagiarism, rag, validate, Database, GaplyError};

use crate::http_fetcher::RefVerifier;
use crate::state::AppState;

/// Report cache TTL: 30 days. The viewer reads it back by `report_id`.
const REPORT_TTL_SECS: i64 = 30 * 24 * 3600;

/// Cache key for a compiled report — ONE definition, so writer and readers
/// cannot drift apart.
///
/// **`v2` retires every report compiled before the PDF paragraph reflow**
/// (`docparse::reflow_pdf_text`). That fix changes `Location.paragraph` values
/// and the parsed reference count, both of which are user-visible. With a
/// 30-day TTL and an unversioned key, the same manuscript could be served a
/// pre-fix and a post-fix report for a month — two different answers, one of
/// them wrong, with nothing to tell them apart. Bumping the key makes stale
/// entries unreachable instead.
///
/// # `CACHED_REPORT_SCHEMA_VERSION` is part of the key
///
/// A cached report carries a serialized `Vec<EvidenceRecord>` under `evidence`,
/// and `escalation.rs:72` deserializes it with `unwrap_or_default()`. **A cached
/// report written by an older binary whose `EvidenceRecord` shape has since
/// changed therefore deserializes to an EMPTY vector**, the early return fires,
/// the aggregator sees zero findings, and the run yields `Accept` at 0.92 — a
/// silent wrong answer with no error and no log line (ARCHITECTURE_TRACE
/// §25.10). Cache entries survive rebuilds, so an upgrade is exactly when this
/// fires.
///
/// Including the version makes a stale entry MISS rather than mis-parse: the
/// report is recomputed from the manuscript, which is always correct and merely
/// slower. This closes the demonstrable path; it does not close the class — a
/// genuinely malformed record still deserializes to empty.
///
/// **RELEASE CONSTRAINT: any schema evolution that affects cached-report
/// compatibility must require an `CACHED_REPORT_SCHEMA_VERSION` bump.** Compatibility,
/// not modification — an optional field with a serde default leaves cached
/// reports readable and needs no bump. Requiring one for every change would
/// train reflexive bumping, which is how versions stop meaning anything.
/// **`pub`, not `pub(crate)`.** Two release-gate EXAMPLES hand-built
/// `report:v2:{id}` and drifted when the schema version entered the key —
/// silently, because nothing that runs in CI compiles examples. A key builder
/// whose whole purpose is "ONE definition so writer and readers cannot drift
/// apart" cannot be unreachable to half its readers.
pub fn report_cache_key(report_id: &str) -> String {
    format!("report:v2:e{}:{report_id}", gaply_core::evidence::CACHED_REPORT_SCHEMA_VERSION)
}
/// The six agent lanes the frontend renders. Debate + compile happen after,
/// under the "synthesis" pseudo-stage.
const LANE_TOTAL: usize = 6;

/// Below this the stylometry signals cannot be computed — MTLD needs ~100
/// tokens and the sentence-length CV needs several sentences, so no ELIGIBLE
/// AI-detection finding is possible and the lane examined nothing.
const MIN_STYLOMETRY_WORDS: usize = 100;

/// Current Gregorian year from the epoch clock, for `compile_report`'s
/// reference-recency count. Uses the mean Gregorian year (365.2425 days) rather
/// than a calendar library — gaply-core pulls in no date crate, and the only
/// consumer is a "how many references are older than N years" COUNT, where being
/// a day off at a New Year boundary changes nothing.
fn current_year() -> i32 {
    const SECS_PER_MEAN_YEAR: i64 = 31_556_952; // 365.2425 * 86_400
    (1970 + now_epoch() / SECS_PER_MEAN_YEAR) as i32
}

/// Semantic query used to retrieve ingested journal guidelines for the
/// checklist. Broad enough to surface the common requirement families
/// (length, abstract, disclosures, reference style).
const GUIDELINE_QUERY: &str =
    "author guidelines submission requirements word limit structured abstract \
     conflict of interest declaration reference style";

/// Typed progress events streamed to the webview over an IPC Channel.
/// `type` is the discriminant the frontend matches on.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AnalysisEvent {
    StageStarted { stage: String, index: usize, total: usize },
    StageProgress { stage: String, pct: u8 },
    /// A parsed section, streamed so the UI's outline populates as extraction
    /// yields headings.
    Section { stage: String, index: usize, total: usize, title: String },
    StageCompleted { stage: String, summary: String },
    Finished { report_id: String },
    Failed { stage: String, message: String },
}

/// **Whether this run may touch the network at all.**
///
/// # Why this is a parameter and not a `localStorage` read
///
/// Consent lived in seven screens as a `localStorage` boolean and was missing
/// in the eighth (`src/screens/analysis/bridge.ts`). Adding it to the eighth
/// would have made the screens agree and left the boundary where it was: a
/// preference the backend cannot see, stored where the user can edit it, on the
/// wrong side of the IPC call it is supposed to gate.
///
/// So the decision is made in the frontend and ENFORCED here. A `bool` would
/// have done the job and been one `!` away from meaning the opposite at every
/// call site; a two-variant enum cannot be misread, and `Denied` is a word that
/// appears in the refusal the user sees.
///
/// This governs the VERIFICATION lane's reference lookups — the only outbound
/// calls the general analysis pipeline makes. The other five lanes are local by
/// construction and are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkConsent {
    /// The user's cloud toggle for this suite is on.
    Granted,
    /// It is off. Reference lookups do not happen, and the lane SAYS so.
    Denied,
}

impl NetworkConsent {
    /// From the frontend's gate. Named rather than `From<bool>` so the call
    /// site reads as a consent decision, not a cast.
    pub fn from_allowed(allowed: bool) -> Self {
        if allowed {
            Self::Granted
        } else {
            Self::Denied
        }
    }
    pub fn is_granted(self) -> bool {
        matches!(self, Self::Granted)
    }
}

/// The async command. Extracts `Arc` handles from managed state (cheap, Send)
/// then runs the sync pipeline off the async runtime via spawn_blocking.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn run_full_analysis(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
    // The Analysis screen's `mayUseCloud('citation_verification')`. NOT
    // defaulted: an absent argument would deserialize to `false` on some shapes
    // and `true` on others depending on who wrote the caller, and a consent flag
    // that has a default has a way of being forgotten into the permissive one.
    allow_network: bool,
    on_event: Channel<AnalysisEvent>,
) -> Result<(), GaplyError> {
    let db = state.db.clone();
    let embedder = state.embedder.clone();
    let consent = NetworkConsent::from_allowed(allow_network);
    // `user_token: None` — this command has no signed-in-user parameter, so the
    // cloud verification tier stays unauthenticated here and degrades to local
    // Ollama/mock exactly as before. PublishReady (`run_publishready`) is the
    // path that has a token and threads it. Giving this command a token needs a
    // frontend change (`src/screens/analysis/bridge.ts` invokes it without one)
    // and is deliberately out of scope for this fix.
    // guidelines_url: None — run_full_analysis is the general analysis command and
    // has no journal-guidelines input; the checklist stays structural-only, which
    // is the honest empty case. PublishReady is the path that carries one.
    tokio::task::spawn_blocking(move || {
        run_pipeline(db, embedder, path, title, None, None, consent, on_event)
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("analysis task panicked: {e}")))?
}

/// Run a lane: emit StageStarted, execute `f`, emit StageCompleted or Failed.
/// On failure the error propagates so the whole pipeline stops.
fn lane<T>(
    emit: &dyn Fn(AnalysisEvent),
    stage: &str,
    index: usize,
    f: impl FnOnce() -> Result<(T, String), GaplyError>,
) -> Result<T, GaplyError> {
    emit(AnalysisEvent::StageStarted { stage: stage.into(), index, total: LANE_TOTAL });
    match f() {
        Ok((value, summary)) => {
            emit(AnalysisEvent::StageCompleted { stage: stage.into(), summary });
            Ok(value)
        }
        Err(e) => {
            emit(AnalysisEvent::Failed { stage: stage.into(), message: e.to_string() });
            Err(e)
        }
    }
}

/// Everything the pipeline produced, including the two sources that were
/// previously LOCALS DROPPED AT THE RETURN (ARCHITECTURE_TRACE §31.2).
///
/// `run_pipeline_inner` already widened its return once — from `()` to
/// `LaneExamination` (§26 PR-4) — and this is the same move for the same
/// reason: the data exists, in scope, at the moment of return, and the only
/// thing standing between it and its consumer is the return type.
///
/// Rejected alternatives, recorded so they are not re-proposed: a callback
/// inverts control for what is simply a return value; a global adds shared
/// mutable state to an otherwise pure pipeline; re-reading from the database
/// can disagree with what this run actually computed.
///
/// # NOT `Serialize`, ON PURPOSE — see ONTOLOGY §4.22
///
/// `text` is the ENTIRE manuscript and `extraction.sections` carries its prose.
/// This type therefore carries exactly what must never leave the machine. The
/// absence of a `Serialize` derive is the guarantee: no connector, telemetry
/// hook, export path or debug dump can serialize it by reaching for a derive
/// that happens to exist. Adding one is a deliberate architectural decision,
/// which is the point.
pub struct PipelineResult {
    pub report_id: String,
    pub report: PublishReadyReport,
    /// Which lanes examined anything — §26 PR-4's verdict-eligibility input.
    pub lanes: LaneExamination,
    /// Title, tables, references, statistics, and the section prose the
    /// Reported/Missing block and nearby-text lookup are built from.
    pub extraction: ExtractionResult,
    /// Similarity regions plus `corpus_chunks_available`, without which
    /// "no matches" cannot be told apart from "nothing to compare against".
    pub plagiarism: PlagiarismReport,
    /// The whole manuscript. Held for nearby-text lookup and NEVER copied
    /// wholesale into `LocalReportModel` — only extracted snippets are.
    pub text: String,
    /// **The machine-readable study** (§3.1), derived from `extraction`.
    ///
    /// Purely additive: nothing in the report path reads it, and the golden
    /// test `the_report_is_byte_identical_with_a_research_state_derived` pins
    /// that adding it changed no output. It is here so Phase 2's harness has
    /// something to route over, and so the derivation runs on every real run
    /// rather than being written and never exercised.
    ///
    /// Contains NO prose — see `gaply_core::research_state`.
    pub research_state: ResearchState,
}

/// Channel wrapper: forward emitted events to the IPC channel (closed channel
/// ignored — the frontend navigated away).
fn run_pipeline(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    user_token: Option<String>,
    guidelines_url: Option<String>,
    consent: NetworkConsent,
    ch: Channel<AnalysisEvent>,
) -> Result<(), GaplyError> {
    // The lane state is for the PublishReady verdict path; the general analysis
    // command has no aggregator to feed, so it is discarded here deliberately.
    run_pipeline_inner(db, embedder, path, title, user_token, guidelines_url, consent, &|ev| {
        let _ = ch.send(ev);
    })
    .map(|_result| ())
}

/// Measurement-only entry point (Set 4 8GB memory proof): drives the REAL
/// pipeline with a caller-supplied event sink so an example can instrument
/// per-stage memory. Not used by the app; `#[doc(hidden)]`, no behavior change.
#[doc(hidden)]
pub fn run_pipeline_measured(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    user_token: Option<String>,
    guidelines_url: Option<String>,
    consent: NetworkConsent,
    emit: &dyn Fn(AnalysisEvent),
) -> Result<PipelineResult, GaplyError> {
    run_pipeline_inner(db, embedder, path, title, user_token, guidelines_url, consent, emit)
}

/// The synchronous pipeline. Every stage is a real production call. `emit` is
/// abstracted (Fn) so tests can drive the full pipeline with a collector.
///
/// `user_token` is the signed-in user's JWT for the cloud verification tier's
/// entitlement gate. `None` = unauthenticated: that tier is skipped or refused
/// and the lane degrades to local Ollama/mock (honest UNKNOWN verdicts), never a
/// failure.
fn run_pipeline_inner(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    user_token: Option<String>,
    // The guideline document the user asked for. Threaded from the frontend so
    // identity is PROPAGATED, not reconstructed from corpus state.
    guidelines_url: Option<String>,
    // Whether the verification lane may make its reference lookups. Threaded in
    // rather than read from a global for the same reason as `guidelines_url`:
    // the decision belongs to the caller and must arrive with the call.
    consent: NetworkConsent,
    emit: &dyn Fn(AnalysisEvent),
) -> Result<PipelineResult, GaplyError> {
    // **THE GRAPH DECIDES WHAT RUNS (§4.3).**
    //
    // Validated at first use — a graph that does not validate panics rather
    // than running unvalidated agents, which is the thing the validator exists
    // to prevent. The lanes below still execute in a fixed sequence; what the
    // graph supplies today is (a) the declaration of record, pinned against
    // that sequence by `the_graphs_order_matches_the_pipelines_lane_order`, and
    // (b) the derivable-artifact decision immediately below.
    let graph = gaply_core::agent_graph::shipped_graph();

    // Parse once, up front (part of the extraction lane's work).
    let text = lane(emit, "extraction", 1, || {
        let text = extract::docparse::parse_path(std::path::Path::new(&path))?;
        Ok((text, String::new()))
    })?;

    // 1) Extraction — real parse → sections/claims/citations, persisted.
    let extraction = {
        // **The scientific layer is a DEPENDENCY, not a flag.**
        //
        // Derived only when some scheduled agent declares
        // `requires: [ScientificExtraction]`. No shipped agent does yet, so it
        // is not derived and nothing pays for it — measured at 2.8-150 ms per
        // manuscript (`examples/scientific_cost_probe.rs`), which is affordable
        // but pointless while nothing reads the result.
        //
        // Deferred rather than passed as `ExtractOptions::scientific`: which
        // agents are scheduled can depend on what extraction found, so the
        // decision cannot be made before extraction runs, and re-running with
        // the flag set would pay the base cost twice. The four passes take an
        // already-built `ExtractionResult`, so they compose after the fact.
        let extraction = if graph.requires_scientific_extraction() {
            extract::extract_from_text_with(&text, extract::ExtractOptions::with_scientific())
        } else {
            extract::extract_from_text(&text)
        };
        let manuscript_title = title
            .clone()
            .or_else(|| extraction.title.clone())
            .unwrap_or_else(|| "Untitled manuscript".to_string());
        let manuscript_id = db.create_manuscript(&manuscript_title, "", "")?;
        extract::persist::store_extraction(&db, manuscript_id, &extraction)?;
        // Stream section headings so the outline panel populates.
        let total_sections = extraction.sections.len();
        for (i, s) in extraction.sections.iter().enumerate() {
            let title = if s.heading.is_empty() { format!("{:?}", s.kind) } else { s.heading.clone() };
            emit(AnalysisEvent::Section {
                stage: "extraction".into(),
                index: i + 1,
                total: total_sections,
                title,
            });
        }
        emit(
            AnalysisEvent::StageCompleted {
                stage: "extraction".into(),
                summary: format!(
                    "{} section(s), {} statistical claim(s), {} reference(s)",
                    extraction.sections.len(),
                    extraction.statistics.len(),
                    extraction.references.len()
                ),
            },
        );
        (extraction, manuscript_id)
    };
    let (extraction, manuscript_id) = extraction;

    // 2) Validation — deterministic 5-rule validator (hard constraint).
    let validation = lane(emit, "validation", 2, || {
        let report = validate::validate(&extraction);
        let summary = if report.passed {
            "no statistical rule violations".to_string()
        } else {
            format!("{} rule violation(s)", report.flags.len())
        };
        Ok((report, summary))
    })?;

    // 3) AI check — real candle perplexity/burstiness at whichever tier THIS
    // machine may safely run, decided by the SHARED memory-safety gate
    // (`models::select_deep_model` → `plan_deep_load`): the 7B only at ≥16GB,
    // else the compact 1.5B, else the interim heuristic. Same gate AI Check
    // uses — this lane used to bypass it and load the 7B unconditionally, which
    // is a multi-hour uncapped run on 8GB. The model is scoped INSIDE this lane
    // so it is dropped before the verification stage (one-at-a-time on 8GB).
    let ai = lane(emit, "ai", 3, || {
        // The tier travels WITH the model. Before this, `perplexity_model()`
        // threw the selection away and the report could not say which tier had
        // scored the text — see `models::perplexity_model_with_kind`.
        let (model, deep_kind) = crate::models::perplexity_model_with_kind();
        let report = ai_detect::detect_extraction(&*model, deep_kind, &extraction);
        let summary = format!("signal: {:?} (model: {})", report.signal, model.name());
        Ok((report, summary))
    })?;

    // 4) Plagiarism — per-session isolated store; self/internal duplication.
    let plag = lane(emit, "plagiarism", 4, || {
        let mut session = plagiarism::PlagiarismSession::new()?;
        session.ingest_manuscript(embedder.as_ref(), &text)?;
        let report = session.report(&db, None)?;
        let summary = format!(
            "{} corpus match(es), {} self-match(es)",
            report.corpus_matches.len(),
            report.self_matches.len()
        );
        Ok((report, summary))
    })?;

    // 5) RAG — semantic search over the (currently empty) corpus. Honest: with
    // no seeded guidelines it returns nothing; that lands until you seed it.
    let hits = lane(emit, "rag", 5, || {
        let hits = rag::search(&db, embedder.as_ref(), "reporting standards guidelines", 5, None)?;
        let summary = if hits.is_empty() {
            "no guideline matches (corpus not seeded yet)".to_string()
        } else {
            format!("{} guideline match(es)", hits.len())
        };
        Ok((hits, summary))
    })?;

    // 6) Verification — REAL refverify HTTP per reference (existence/retraction);
    // hallucination verdict via verify_citations routed to the local SLM-2
    // (Ollama) when reachable, else honest UNKNOWNs — never fabricated, never
    // fatal (see `crate::models::verify_proxy`).
    let verification = lane(emit, "verification", 6, || {
        let mut items: Vec<(Reference, ReferenceVerification)> = Vec::new();
        let refs = &extraction.references;
        // BLOCKER 3 — the refusal, in Rust, before anything is constructed.
        //
        // UNCONDITIONAL ON `refs` (§11 D154). This first shipped as
        // `!refs.is_empty() && !consent.is_granted()`, which let a manuscript
        // with no parseable references skip the check and reach `verify_proxy`
        // — client, keychain, probe — with consent denied. Whether the payload
        // turns out to be empty is decided AFTER the boundary, not at it.
        //
        // Deliberately ABOVE `RefVerifier::new()` rather than inside the loop:
        // the check has to sit where no network object exists yet, so a future
        // edit that moves work around cannot leave a fetcher built and a guard
        // further down. There is nothing here to "skip past".
        //
        // The run CONTINUES. A refused lane is not a failed lane — the other
        // five are local and their findings are unaffected — and the summary
        // says which it was, because "0 references checked" and "we did not
        // check your references" are different sentences and only one of them
        // is true here.
        if !consent.is_granted() {
            crate::models::unload_slm2();
            let report = verify_citations(
                &MockProxyClient::returning(serde_json::json!({ "verdicts": [] })),
                &[],
            )?;
            let summary = if refs.is_empty() {
                "no references extracted; no reference checks made — cloud access is off for \
                 citation verification (Settings → Sync & Privacy)"
                    .to_string()
            } else {
                format!(
                    "{} reference(s) not checked — cloud access is off for citation verification \
                     (Settings → Sync & Privacy)",
                    refs.len()
                )
            };
            return Ok(((report, Vec::new(), None), summary));
        }
        if !refs.is_empty() {
            let verifier = RefVerifier::new()?;
            let now = now_epoch();
            let total = refs.len();
            for (i, r) in refs.iter().enumerate() {
                match verifier.verify(&db, r, now) {
                    Ok(rv) => items.push((r.clone(), rv)),
                    Err(e) => tracing::warn!(reference = %r.raw, error = %e, "refverify failed for a reference"),
                }
                let pct = (((i + 1) * 100) / total) as u8;
                emit(AnalysisEvent::StageProgress { stage: "verification".into(), pct });
            }
        }
        // Route to local SLM-2 if reachable, else honest UNKNOWNs. The belt: if
        // a reachable Ollama errors mid-call (model not pulled, timeout, bad
        // reply), degrade to the mock's empty verdicts rather than fail the run.
        let proxy = crate::models::verify_proxy(user_token.clone());
        let report = match verify_citations(&*proxy, &items) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "SLM-2: verification call failed; recording UNKNOWN verdicts");
                let mock = MockProxyClient::returning(serde_json::json!({ "verdicts": [] }));
                verify_citations(&mock, &items)?
            }
        };
        // NOTE: `unload_slm2()` used to be HERE. It moved to after the debate,
        // because the debate is where `RevisingVerificationAgent` may make its
        // SECOND proxy call — unloading first would either force a reload
        // mid-debate or leave the reconsideration talking to nothing. The guard
        // is unchanged in what it guarantees (SLM-2 out of memory before the
        // pipeline returns, so a subsequent run's candle SLM-1 load is safe);
        // only the point at which it fires moved past the last user of SLM-2.
        let definite = report
            .verdicts
            .iter()
            .filter(|v| v.verdict != gaply_core::verify_agent::Verdict::Unknown)
            .count();
        let summary = format!(
            "{} reference(s) checked via public APIs; {} definite verdict(s)",
            items.len(),
            definite
        );
        // Registry evidence retained for the LOCAL side. `matched_year` and
        // `citation_count` were already transmitted to the cloud reviewer inside
        // the evidence bundle (`verify_agent.rs:176`, `:206`); dropping `items`
        // here made them unavailable to any local deterministic finding — a
        // LOCALITY problem, not unused computation (ARCHITECTURE_TRACE §2).
        // `items` leaves the lane instead of being consumed here: the
        // reconsideration needs the same (reference, evidence) pairs the first
        // call was built from, so the revision bundle — and the harness gate
        // over it — is identical to the original. The registry is built from
        // them after the debate, once the reviser hands them back.
        Ok(((report, items, Some(proxy)), summary))
    })?;
    let (verification, verify_items, verify_proxy) = verification;

    // --- synthesis: round-table debate → compiled report --------------------
    let report = (|| -> Result<gaply_core::report::PublishReadyReport, GaplyError> {
        // **§5.1 — the verification participant REVISES.**
        //
        // It was a `PrecomputedAgent`, like the other five. `SwarmAgent::revise`
        // defaults to `None`, so every debate converged in round one with
        // `revised_agents: []` — measured empty in 22 of 22 stored reports. The
        // mesh round-table was a vote, and every claim about it was a `doc`
        // claim.
        //
        // The other five stay precomputed ON PURPOSE (`revising.rs`'s design
        // boundary): their outputs are MEASUREMENTS — deterministic rules,
        // parses, similarity scores, perplexity statistics — and a validator
        // that changed its answer under peer pressure would be broken, not
        // collaborative. Verification's verdicts are LLM-derived judgements, so
        // reconsideration is meaningful there and only there.
        //
        // Boxed as `&mut` (see the blanket impl on `SwarmAgent`) so this
        // function keeps ownership and can read the REVISED report back out.
        // `compile_report` builds the per-citation findings from that report:
        // passing the pre-debate copy would render findings that contradict the
        // `revised_agents` summary printed beside them.
        let precomputed_five = || -> Vec<Box<dyn SwarmAgent>> {
            vec![
                Box::new(PrecomputedAgent::new(adapters::from_extraction(&extraction))),
                Box::new(PrecomputedAgent::new(adapters::from_validation(&validation))),
                Box::new(PrecomputedAgent::new(adapters::from_ai_detection(&ai))),
                Box::new(PrecomputedAgent::new(adapters::from_plagiarism(&plag))),
                Box::new(PrecomputedAgent::new(adapters::from_rag_hits(&hits))),
            ]
        };

        let (outcome, verification, registry) = match verify_proxy.as_ref() {
            Some(proxy) => {
                let mut reviser =
                    RevisingVerificationAgent::new(&**proxy, verify_items, verification);
                // Inner scope so the `&mut reviser` borrow ends before
                // `into_parts` consumes it.
                let outcome = {
                    let mut agents: Vec<Box<dyn SwarmAgent + '_>> = precomputed_five()
                        .into_iter()
                        .chain(std::iter::once(
                            Box::new(&mut reviser) as Box<dyn SwarmAgent + '_>
                        ))
                        .collect();
                    run_debate(&mut agents, &DebateConfig::default())?
                };
                let (items, revised) = reviser.into_parts();
                let registry: Vec<ReferenceVerification> =
                    items.into_iter().map(|(_, rv)| rv).collect();
                (outcome, revised, registry)
            }
            None => {
                // Consent refused: no proxy was built, so there is nothing to
                // reconsider WITH. The lane already returned an empty report
                // and no items; the debate runs with a precomputed participant
                // exactly as before, and `revised_agents` is legitimately empty.
                let mut agents: Vec<Box<dyn SwarmAgent>> = precomputed_five();
                agents.push(Box::new(PrecomputedAgent::new(
                    adapters::from_verification_report(&verification),
                )));
                let outcome = run_debate(&mut agents, &DebateConfig::default())?;
                (outcome, verification, Vec::new())
            }
        };
        // Build the checklist from the ingested journal_guideline corpus. With
        // no guidelines ingested, build_checklist still returns the always-on
        // structural checks (section presence) — never fabricated guideline
        // items. It is a DB/embedder query (no model load), so the one-at-a-time
        // model lifecycle is unaffected. Degrade to empty on a query error
        // rather than fail the whole analysis.
        let checklist = build_checklist(&db, &extraction, &text, guidelines_url.as_deref())
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "build_checklist failed; empty checklist");
                Vec::new()
            });
        // `extraction` unlocks the three extraction-derived finding families
        // (stylometry, tables, reference recency) — pure, no model, no network.
        // `current_year` is injected rather than read inside the core so
        // compile_report stays deterministic (the `now_epoch()` convention).
        Ok(compile_report(
            &outcome,
            &validation,
            Some(&verification),
            Some(&plag),
            Some(&extraction),
            current_year(),
            checklist,
            &registry,
        ))
    })();

    // 8GB OOM guard, moved here from inside the verification lane: SLM-2 must be
    // out of memory before any NEXT analysis loads candle SLM-1. It fires after
    // the debate because the debate is where `RevisingVerificationAgent` makes
    // its second proxy call — unloading before that would force a reload
    // mid-debate. Unconditional and outside the `match` so it runs on the error
    // path too: a failed synthesis must not leave the model resident.
    crate::models::unload_slm2();

    let report = match report {
        Ok(r) => r,
        Err(e) => {
            emit(AnalysisEvent::Failed { stage: "synthesis".into(), message: e.to_string() });
            return Err(e);
        }
    };

    // Persist the compiled report so the viewer can load the REAL output.
    let report_id = manuscript_id.to_string();
    let json = serde_json::to_string(&report)
        .map_err(|e| GaplyError::Internal(format!("serialize report: {e}")))?;
    db.cache_put(&report_cache_key(&report_id), &json, REPORT_TTL_SECS, now_epoch())?;

    emit(AnalysisEvent::Finished { report_id: report_id.clone() });

    // §26 PR-4's criterion, applied at the ONLY place the inputs are in scope.
    // (a) `Rag` is absent — it produces `ProcessState` only, so it could never
    // have contributed to a verdict and its silence says nothing.
    // (b) Each flag asks whether the INPUT to eligible-claim production was
    // present, never whether the lane produced output.
    let lanes = LaneExamination {
        // The whole lane is gated on `!refs.is_empty()` AND on consent. A lane
        // that refused examined NOTHING, and §26 PR-4 asks exactly that
        // question — so a refusal must not read as "ran and found nothing",
        // which is the shape that would let an unexamined manuscript earn a
        // clean verdict.
        verification_examined: !extraction.references.is_empty() && consent.is_granted(),
        // `validate()` iterates `result.statistics`; empty in, no flags out.
        validation_examined: !extraction.statistics.is_empty(),
        // Nothing to compare against, and too few chunks for self-overlap.
        plagiarism_examined: plag.corpus_chunks_available > 0 || plag.chunk_count >= 2,
        // Below the stylometry gates no eligible finding is possible.
        ai_detection_examined: text.split_whitespace().count() >= MIN_STYLOMETRY_WORDS,
        // Its eligible outputs are table-caption and reference-recency findings.
        extraction_examined: !extraction.tables.is_empty() || !extraction.references.is_empty(),
    };

    // The two sources §31.2 found unreachable now leave the function instead of
    // being dropped at its closing brace.
    // Derived, not re-extracted: a pure projection of what extraction already
    // returned. "Do not change what extraction does; change where its output
    // lands" — this is the landing site.
    let research_state = ResearchState::from_extraction(&extraction);

    Ok(PipelineResult {
        report_id,
        report,
        lanes,
        extraction,
        plagiarism: plag,
        text,
        research_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache key must change when the evidence schema does, so a report
    /// written by an older binary MISSES rather than deserializing to an empty
    /// evidence vector — which `escalation.rs:72`'s `unwrap_or_default()` would
    /// turn into `Accept` at 0.92 (§25.10).
    #[test]
    fn the_cache_key_carries_the_evidence_schema_version() {
        let key = report_cache_key("42");
        assert!(key.starts_with("report:v2:"), "the v2 prefix is retained: {key}");
        assert!(key.ends_with(":42"), "the report id is the last segment: {key}");
        assert!(
            key.contains(&format!("e{}", gaply_core::evidence::CACHED_REPORT_SCHEMA_VERSION)),
            "the evidence schema version must be IN the key, or a stale cached report \
             is read back with a mismatched EvidenceRecord shape: {key}"
        );
        // The property that matters is DISCRIMINATION: two schema versions must
        // not collide on one key. Asserted by construction against the current
        // format, so the test fails if the version is ever dropped from it.
        let same_id_other_version = format!(
            "report:v2:e{}:42",
            gaply_core::evidence::CACHED_REPORT_SCHEMA_VERSION + 1
        );
        assert_ne!(key, same_id_other_version, "a version change must change the key");
        // Same version, different manuscript -> still distinct.
        assert_ne!(key, report_cache_key("43"));
    }

    /// A key written under the OLD (unversioned) format must not be readable
    /// under the new one: the entry misses and the report is recomputed, which
    /// is the whole mechanism.
    #[test]
    fn pre_versioning_keys_no_longer_match() {
        assert_ne!(report_cache_key("42"), "report:v2:42");
    }

    #[test]
    fn analysis_event_serializes_with_type_tag() {
        let ev = AnalysisEvent::StageStarted { stage: "extraction".into(), index: 1, total: 6 };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["type"], "stageStarted");
        assert_eq!(v["stage"], "extraction");
        assert_eq!(v["total"], 6);

        let done = AnalysisEvent::Finished { report_id: "42".into() };
        let v2 = serde_json::to_value(&done).unwrap();
        assert_eq!(v2["type"], "finished");
        assert_eq!(v2["reportId"], "42");

        let fail = AnalysisEvent::Failed { stage: "verification".into(), message: "boom".into() };
        let v3 = serde_json::to_value(&fail).unwrap();
        assert_eq!(v3["type"], "failed");
        assert_eq!(v3["message"], "boom");
    }

    // A small but real IMRaD manuscript with a proper stat claim and NO
    // References section — so the verify stage makes no network call, keeping
    // this test hermetic while still exercising every lane end-to-end.
    const MANUSCRIPT: &str = "\
Title: Sleep and Memory Consolidation in Adults

Abstract
We examined whether a night of sleep improves memory consolidation in adults.

Introduction
Prior work suggests that sleep supports the consolidation of declarative memory.

Methods
We recruited 48 participants and analysed recall with a paired t-test.

Results
Sleep significantly improved recall (t(47) = 3.2, p = 0.002, d = 0.46).

Discussion
The results are consistent with a consolidation account of sleep.

Conclusion
A night of sleep improved memory consolidation in this sample.
";

    /// A manuscript WITH a References section, so the verification lane has
    /// something to look up. Used only by the consent tests below, which assert
    /// that nothing is looked up.
    const MANUSCRIPT_WITH_REFS: &str = "\
Title: Sleep and Memory Consolidation in Adults

Abstract
We examined whether a night of sleep improves memory consolidation in adults.

Introduction
Prior work suggests that sleep supports the consolidation of declarative memory.

Methods
We recruited 48 participants and analysed recall with a paired t-test.

Results
Sleep significantly improved recall (t(47) = 3.2, p = 0.002, d = 0.46).

Discussion
The results are consistent with a consolidation account of sleep.

References
Walker M P and Stickgold R. 2006. Sleep, memory, and plasticity. Annual Review of Psychology 57: 139-166. https://doi.org/10.1146/annurev.psych.56.091103.070307

Diekelmann S and Born J. 2010. The memory function of sleep. Nature Reviews Neuroscience 11: 114-126. https://doi.org/10.1038/nrn2762
";

    /// **BLOCKER 3 — the verification lane refuses in Rust when consent is
    /// absent.**
    ///
    /// # Why this is a Rust test and not a frontend one
    ///
    /// The gate existed in seven screens as a `localStorage` read and was
    /// missing in the eighth (`src/screens/analysis/bridge.ts`). Adding it there
    /// would have made eight screens agree — and left the boundary exactly where
    /// it was, which is to say nowhere: `localStorage` is a preference the
    /// backend cannot see and a user can edit. **This test is the boundary.** It
    /// calls the pipeline directly, with no frontend in the picture, and asserts
    /// the lookups do not happen.
    ///
    /// # Hermetic BY THE THING IT ASSERTS
    ///
    /// `MANUSCRIPT_WITH_REFS` carries two real DOIs. With `allow_network:
    /// false` nothing resolves them, so this test makes no network call — and if
    /// the refusal ever regresses, the test does not merely fail, it starts
    /// hitting CrossRef from the suite. That is the honest shape: the assertion
    /// and the hermeticity are the same property.
    #[test]
    fn the_verification_lane_makes_no_lookups_when_network_consent_is_absent() {
        use std::cell::RefCell;
        force_heuristic();
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);
        let path = std::env::temp_dir()
            .join(format!("gaply_consent_off_{}.txt", std::process::id()));
        std::fs::write(&path, MANUSCRIPT_WITH_REFS).expect("write temp manuscript");

        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        let res = run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("consent-off".into()),
            None,
            None,
            NetworkConsent::Denied,
            &emit,
        );
        let _ = std::fs::remove_file(&path);
        let out = res.expect("the run must COMPLETE — refusing a lane is not failing it");

        // The references were extracted: the lane had something to look up and
        // declined, rather than there being nothing to do.
        assert!(
            out.extraction.references.len() >= 2,
            "fixture must yield references to refuse; got {}",
            out.extraction.references.len()
        );

        let summary = events
            .into_inner()
            .into_iter()
            .find_map(|e| match e {
                AnalysisEvent::StageCompleted { stage, summary } if stage == "verification" => {
                    Some(summary)
                }
                _ => None,
            })
            .expect("the verification lane must complete");

        assert!(
            !summary.contains("checked via public APIs"),
            "the lane reported live lookups with consent absent: {summary:?}"
        );
        assert!(
            summary.contains("cloud access is off"),
            "the lane must SAY it refused, not silently report zero: {summary:?}"
        );
        assert!(
            !out.lanes.verification_examined,
            "a refused lane examined nothing and must not count as having run"
        );
    }

    /// **THE HOLE THE FIRST GUARD LEFT.**
    ///
    /// The refusal was written as `!refs.is_empty() && !consent.is_granted()`,
    /// so a manuscript whose references do not parse — which is most `.docx`
    /// chapters — SKIPPED the consent check entirely and fell through to
    /// `verify_proxy`: a real client, the OS keychain, a reachability probe,
    /// with consent denied.
    ///
    /// It was not found by a test. It was found by a measurement run hanging at
    /// 0.0% CPU, and `sample` putting the top frame in
    /// `app_check::TokenSigner::from_keychain` — the §11 D124 modal, reached on
    /// a path that had just been "fixed" to never reach the network. The
    /// original guard passed every test because every fixture that exercised it
    /// had references.
    ///
    /// **A consent check conditioned on having something to send is not a
    /// consent check.** Whether the payload turns out to be empty is decided
    /// after the boundary, not at it.
    #[test]
    fn a_manuscript_with_no_references_still_refuses_when_consent_is_absent() {
        use std::cell::RefCell;
        force_heuristic();
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);
        let path = std::env::temp_dir()
            .join(format!("gaply_norefs_{}.txt", std::process::id()));
        // MANUSCRIPT has no References section at all.
        std::fs::write(&path, MANUSCRIPT).expect("write temp manuscript");

        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        let res = run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("no-refs".into()),
            None,
            None,
            NetworkConsent::Denied,
            &emit,
        );
        let _ = std::fs::remove_file(&path);
        let out = res.expect("the run must complete");
        assert!(out.extraction.references.is_empty(), "fixture precondition: no references");

        let summary = events
            .into_inner()
            .into_iter()
            .find_map(|e| match e {
                AnalysisEvent::StageCompleted { stage, summary } if stage == "verification" => {
                    Some(summary)
                }
                _ => None,
            })
            .expect("the verification lane must complete");
        assert!(
            summary.contains("cloud access is off"),
            "an empty reference list must still take the refusal path, not fall through to the \
             proxy: {summary:?}"
        );
        assert!(!out.lanes.verification_examined);
    }

    /// **THE PHASE 1 PART B DELIVERABLE — the report is byte-identical.**
    ///
    /// `ResearchState` is meant to change WHERE extraction's output lands, not
    /// what the product says. The way that claim fails is not dramatically: a
    /// derivation that mutated a shared structure, or a field order that moved a
    /// serialised key, would shift the report by a byte nobody looks at until a
    /// cached report stops parsing (§11 D53/D103/D109 — five drifts, all
    /// silent).
    ///
    /// # THE FIRST VERSION OF THIS TEST COULD NOT FAIL
    ///
    /// It serialised `out.report` twice, with the derivation between the two
    /// calls, and asserted the bytes matched. `ResearchState::from_extraction`
    /// takes `&ExtractionResult`, so it *cannot* mutate the report — the two
    /// sides were the same immutable value and the assertion was satisfied by
    /// the type system before the test ran. A green result meant nothing, which
    /// is the shape this project keeps finding (the lint gate, the XML
    /// containment assertions).
    ///
    /// **So the comparison is against bytes produced by DIFFERENT CODE.**
    /// `tests/fixtures/report.golden.json` was captured from a throwaway
    /// worktree at `e5eb963` — the commit immediately before `ResearchState`
    /// existed — by running the same pipeline over
    /// `tests/fixtures/golden_manuscript.txt`. "Before and after" is then two
    /// compilers apart, which is what it has to mean for the claim to be worth
    /// anything.
    ///
    /// If this fails, the derivation is not additive. Regenerating the fixture
    /// to make it pass is the wrong move unless the report is MEANT to change,
    /// in which case say so in the commit and record why.
    #[test]
    fn the_report_is_byte_identical_to_the_pre_research_state_capture() {
        use std::cell::RefCell;
        force_heuristic();
        let golden = include_str!("../tests/fixtures/report.golden.json");
        let manuscript = include_str!("../tests/fixtures/golden_manuscript.txt");

        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);
        let path = std::env::temp_dir().join(format!("gaply_golden_{}.txt", std::process::id()));
        std::fs::write(&path, manuscript).expect("write temp manuscript");

        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        let out = run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("golden".into()),
            None,
            None,
            NetworkConsent::Denied,
            &emit,
        )
        .expect("pipeline completes");
        let _ = std::fs::remove_file(&path);

        let now = serde_json::to_string(&out.report).expect("report serialises");
        assert_eq!(
            now,
            golden.trim_end(),
            "the report changed. ResearchState is supposed to be a projection of extraction \
             that nothing in the report path reads — if this differs, it is not."
        );

        // And the state is real, so the comparison above is not passing because
        // the derivation quietly did nothing.
        assert!(out.research_state.hash_matches());
        assert_eq!(out.research_state.references.len(), out.extraction.references.len());
        assert!(!out.research_state.structure.is_empty());
    }

    /// The privacy-class property from §3.1, asserted rather than assumed: the
    /// research state is "structured, gate-safe" while the manuscript layer is
    /// premium-consented. A state carrying prose would quietly move text into a
    /// layer that is allowed to travel.
    #[test]
    fn the_research_state_carries_no_manuscript_prose() {
        use std::cell::RefCell;
        force_heuristic();
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);
        let path = std::env::temp_dir().join(format!("gaply_noprose_{}.txt", std::process::id()));
        std::fs::write(&path, MANUSCRIPT_WITH_REFS).expect("write temp manuscript");
        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        let out = run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("no-prose".into()),
            None,
            None,
            NetworkConsent::Denied,
            &emit,
        )
        .expect("pipeline completes");
        let _ = std::fs::remove_file(&path);

        let json = serde_json::to_string(&out.research_state).expect("state serialises");
        // A distinctive sentence from the body, present in the manuscript and in
        // no heading, caption, statistic or reference.
        let body_sentence = "Prior work suggests that sleep supports the consolidation";
        assert!(
            out.text.contains(body_sentence),
            "fixture precondition: the sentence must be in the manuscript"
        );
        assert!(
            !json.contains(body_sentence),
            "manuscript prose reached the research state, which is the gate-safe layer"
        );
    }

    #[test]
    fn full_pipeline_runs_all_six_lanes_and_produces_a_real_report() {
        use std::cell::RefCell;

        // Lane completion + report shape don't depend on which perplexity model
        // ran, so take the heuristic and keep the suite off multi-GB GGUF loads.
        force_heuristic();
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);

        let path = std::env::temp_dir().join(format!("gaply_pipeline_e2e_{}.txt", std::process::id()));
        std::fs::write(&path, MANUSCRIPT).expect("write temp manuscript");

        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);

        let res = run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("E2E manuscript".into()),
            None, // unauthenticated: keeps the verify lane off the cloud tier
            None, // no guidelines requested -> structural-only checklist
            NetworkConsent::Granted, // the fixtures carry no References section
            &emit,
        );
        let _ = std::fs::remove_file(&path);
        res.expect("pipeline should complete without error");

        let ev = events.into_inner();

        // Every one of the six lanes must complete.
        let completed: Vec<String> = ev
            .iter()
            .filter_map(|e| match e {
                AnalysisEvent::StageCompleted { stage, .. } => Some(stage.clone()),
                _ => None,
            })
            .collect();
        for stage in ["extraction", "validation", "ai", "plagiarism", "rag", "verification"] {
            assert!(
                completed.iter().any(|s| s == stage),
                "lane '{stage}' did not complete; completed = {completed:?}"
            );
        }

        // No fabricated failure, and a real Finished report id.
        assert!(
            !ev.iter().any(|e| matches!(e, AnalysisEvent::Failed { .. })),
            "unexpected Failed event: {ev:?}"
        );
        let report_id = ev
            .iter()
            .find_map(|e| match e {
                AnalysisEvent::Finished { report_id } => Some(report_id.clone()),
                _ => None,
            })
            .expect("a Finished event with a report_id");

        // The REAL compiled report was cached and deserializes with content.
        let json = db
            .cache_get(&report_cache_key(&report_id), now_epoch())
            .unwrap()
            .expect("report cached under report:{id}");
        let report: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(report["verdict"].is_string(), "report has a verdict");
        assert!(
            report["disclaimer"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
            "report has a non-empty disclaimer"
        );
        assert!(report["findings"].is_array(), "report has a findings array");

        // The pipeline must actually PASS the extraction to compile_report — the
        // core tests cover how the extraction-derived findings are built, but only
        // this asserts the call site is live. `signal:` provenance is unique to
        // them, and the fixture has a reference list, so recency always fires.
        let findings = report["findings"].as_array().unwrap();
        let derived: Vec<&serde_json::Value> = findings
            .iter()
            .filter(|f| {
                f["provenance"]
                    .as_array()
                    .map(|ps| ps.iter().any(|p| p.as_str().is_some_and(|s| s.starts_with("signal:"))))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            !derived.is_empty(),
            "no extraction-derived finding reached the report — the compile_report call site is \
             not passing the extraction; findings = {findings:?}"
        );
        // Their EvidenceRecords must exist 1:1 alongside, same as every other
        // finding (the pairing is by construction, this proves it survives here).
        assert_eq!(
            report["evidence"].as_array().map(|e| e.len()),
            Some(findings.len()),
            "evidence must stay 1:1 with findings after the new families are added"
        );
    }

    /// Force the interim heuristic perplexity model (no candle load) so pipeline
    /// runs in the test suite stay fast and don't stack multi-GB model loads in
    /// parallel on small machines. Harmless: no assertion here depends on which
    /// perplexity model ran.
    ///
    /// This drives the PRODUCT's own gate (`models::deep_tier` → HeuristicOnly)
    /// rather than the old trick of pointing `GAPLY_SLM1_GGUF` at a nonexistent
    /// file. That trick only ever hid the 7B — it left the compact 1.5B free to
    /// load — and it worked by defeating a file-existence check rather than by
    /// expressing an intent. Deliberately NOT unset afterwards: every pipeline
    /// test in this module wants the heuristic, so a leak across the shared test
    /// process is benign (and `set_var` is process-global regardless).
    fn force_heuristic() {
        std::env::set_var("GAPLY_DISABLE_DEEP", "1");
    }

    /// A REALISTIC-length manuscript: ~4,000 words across a full IMRaD body,
    /// versus the ~120-word `MANUSCRIPT` above.
    ///
    /// Length is the point. The 8GB memory proof
    /// (`examples/publishready_mem_probe.rs`) exercised this pipeline against a
    /// 260-word fixture, and at that length even an ungated 7B finishes in
    /// seconds — which is precisely why the AI lane's uncapped, gate-bypassing
    /// model load survived review. The AI lane's cost scales with TOKEN COUNT
    /// (~2x the manuscript's tokens, at window 512 / stride 256), so only a
    /// realistic length makes the wrong tier observable.
    ///
    /// Synthetic and repetitive by construction — the paragraphs rotate through
    /// four templates with a varying index — which is fine here: no assertion
    /// depends on the prose being novel, only on there being a lot of it. NO
    /// References section, so the verification lane makes no network call and
    /// the test stays hermetic (same reason as `MANUSCRIPT`).
    fn realistic_manuscript() -> String {
        const PARAS: [&str; 4] = [
            "Participants in cohort {i} completed the full assessment battery under standardised \
             laboratory conditions, with sessions scheduled at a consistent time of day to limit \
             circadian confounding. Each session opened with a short practice block that was \
             discarded before analysis. Trained assistants, blind to group allocation, \
             administered every instrument and recorded responses on paper forms that were later \
             double-entered by two independent coders. Discrepancies between coders were resolved \
             by consensus with a third rater. We logged room temperature, ambient noise, and \
             interruptions for each session, and treated any session with a documented \
             interruption as a candidate for sensitivity analysis rather than excluding it \
             outright.",
            "Analytic decisions for block {i} were fixed before the data were unblinded and \
             recorded in a dated internal protocol. We specified the primary contrast, the \
             covariate set, and the handling of missing observations in advance, and we report \
             every deviation from that plan. Continuous measures were screened for implausible \
             values against instrument-specific ranges, and flagged records were checked against \
             the original paper forms rather than silently corrected. Where a value could not be \
             verified against source documentation we treated it as missing. No observation was \
             removed on the basis of its effect on the primary estimate.",
            "The pattern observed in subgroup {i} was smaller than the effect reported in earlier \
             work, and the discrepancy deserves a plainer explanation than measurement noise. Our \
             sample skewed younger and more educated than the populations those studies \
             recruited, which plausibly compresses the range of the outcome. The instruments also \
             differ: what we scored as a single composite was previously reported as three \
             correlated subscales, and composites tend to attenuate contrasts that live in one \
             component. We therefore read our estimate as compatible with the earlier literature \
             rather than contradicting it, while acknowledging that neither reading is settled by \
             these data.",
            "Several limitations bound how far the result for stratum {i} should travel. \
             Recruitment ran through a single institution, so selection effects operating at the \
             point of referral are not addressed by our design. The follow-up window closed \
             before the outcome would be expected to stabilise in a minority of participants, and \
             those cases are necessarily represented by their last available measurement. The \
             analysis further assumes that dropout is unrelated to the outcome after conditioning \
             on the covariate set, an assumption we can probe but not verify. We report the \
             complete-case and imputed estimates side by side so readers can weigh both.",
        ];

        let mut m = String::from(
            "Title: Sleep Restriction and Declarative Memory Consolidation in Young Adults\n\n",
        );
        m.push_str(
            "Abstract\nWe examined whether a single night of restricted sleep degrades overnight \
             consolidation of declarative memory in healthy young adults, and whether any effect \
             survives adjustment for baseline encoding strength. Across four testing waves we \
             measured cued recall before and after a sleep opportunity, varying only the length \
             of that opportunity between arms.\n\n",
        );
        let mut i = 0usize;
        for heading in ["Introduction", "Methods", "Results", "Discussion"] {
            m.push_str(heading);
            m.push('\n');
            if heading == "Results" {
                m.push_str(
                    "Restricted sleep reduced overnight recall relative to the control arm \
                     (t(47) = 3.2, p = 0.002, d = 0.46). The adjusted contrast was unchanged in \
                     direction and magnitude (t(45) = 3.0, p = 0.004).\n\n",
                );
            }
            for _ in 0..10 {
                i += 1;
                m.push_str(&PARAS[i % PARAS.len()].replace("{i}", &i.to_string()));
                m.push_str("\n\n");
            }
        }
        m.push_str(
            "Conclusion\nA single night of restricted sleep measurably reduced overnight \
             declarative consolidation in this sample, with the caveats above.\n",
        );
        m
    }

    /// THE regression guard. The pipeline's AI lane must obey the SHARED
    /// memory-safety gate (`models::select_deep_model`), on a manuscript long
    /// enough for the choice to matter.
    ///
    /// Before the fix this lane called `slm1_model()` through an ungated
    /// `perplexity_model()`, so it loaded the full 7B on ANY machine with the
    /// GGUF on disk — bypassing both the >=16GB structural gate and the RAM
    /// courtesy check, and turning this fixture into a multi-hour run on 8GB.
    /// `GAPLY_DISABLE_DEEP=1` is the product's own "no deep model" decision; it
    /// had NO effect on this lane before, because the lane never consulted the
    /// gate that reads it. Asserting on the model NAME (not on wall-clock) keeps
    /// this deterministic on a loaded CI box.
    #[test]
    fn ai_lane_honours_the_shared_memory_gate_on_a_realistic_manuscript() {
        use std::cell::RefCell;

        force_heuristic();
        let text = realistic_manuscript();
        let words = text.split_whitespace().count();
        assert!(
            words > 3000,
            "fixture must stay realistic-length or it stops guarding anything; got {words} words"
        );

        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);
        let path = std::env::temp_dir()
            .join(format!("gaply_ai_gate_{}_{}.txt", std::process::id(), now_epoch()));
        std::fs::write(&path, &text).expect("write temp manuscript");

        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        let res = run_pipeline_inner(
            db,
            embedder,
            path.to_string_lossy().to_string(),
            Some("AI gate regression".into()),
            None, // unauthenticated: keeps the verify lane off the cloud tier
            None, // no guidelines requested -> structural-only checklist
            NetworkConsent::Granted, // the fixtures carry no References section
            &emit,
        );
        let _ = std::fs::remove_file(&path);
        res.expect("pipeline should complete without error");

        let summary = events
            .into_inner()
            .into_iter()
            .find_map(|e| match e {
                AnalysisEvent::StageCompleted { stage, summary } if stage == "ai" => Some(summary),
                _ => None,
            })
            .expect("the ai lane must complete and report which model ran");

        // The lane reports `model.name()`. The gate said "no deep model", so the
        // interim heuristic must be what scored the document.
        assert!(
            summary.contains("heuristic frequency proxy"),
            "gate said HeuristicOnly, so the heuristic must have scored it; got {summary:?}"
        );
        // And explicitly NOT either candle tier — the exact bypass that regressed.
        for forbidden in ["candle", "Qwen"] {
            assert!(
                !summary.contains(forbidden),
                "a gated-off run must not load a candle model; got {summary:?}"
            );
        }
    }

    /// END TO END, through the artifact: a REAL pipeline run projected into the
    /// engine model, composed, rendered, and read back out of the PDF bytes.
    ///
    /// The unit tests in `report_pdf` prove the render path on a synthetic
    /// fixture. **This one proves the THREADING** — that `ExtractionResult` and
    /// `PlagiarismReport` actually reach the page, which is the whole point of
    /// widening the pipeline's return type (§31.23). It fails if either source
    /// goes back to being dropped at the return.
    #[test]
    fn a_real_run_reaches_the_pdf_through_extraction_and_plagiarism() {
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);
        let path = std::env::temp_dir()
            .join(format!("gaply_pdf_e2e_{}_{}.txt", std::process::id(), now_epoch()));
        std::fs::write(&path, MANUSCRIPT).expect("write manuscript");
        let emit = |_e: AnalysisEvent| {};
        let result = run_pipeline_inner(
            db,
            embedder,
            path.to_string_lossy().to_string(),
            Some("pdf e2e".into()),
            None,
            None,
            NetworkConsent::Granted, // MANUSCRIPT has no References section
            &emit,
        );
        let _ = std::fs::remove_file(&path);
        let result = result.expect("pipeline should complete");

        // The two previously unreachable sources, asserted BEFORE projection so
        // a failure names which link broke.
        assert!(!result.extraction.sections.is_empty(), "extraction did not reach the caller");
        assert!(result.plagiarism.chunk_count > 0, "plagiarism did not reach the caller");
        assert!(!result.text.is_empty(), "manuscript text did not reach the caller");

        let model = result.into_report_model(Some("Journal of Testing".into()), None, None);
        let sections = model.manuscript.section_count;
        let stats = model.manuscript.statistics.len();
        assert!(sections > 0 && stats > 0, "model lost extraction facts");

        let bytes = gaply_core::report_pdf::render_pdf(&gaply_core::report_compose::compose(&model));
        let text: String = pdf_extract::extract_text_from_mem(&bytes)
            .expect("our own PDF must be readable")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        // Facts that exist ONLY because extraction was threaded through.
        assert!(text.contains("JournalofTesting"), "journal name missing from the report");
        assert!(text.contains(&format!("{sections}sections")), "section count missing");
        assert!(text.contains("Statisticsreported"), "the statistics block is absent");
        // And the plagiarism-derived distinction §26 PR-4 exists to preserve.
        assert!(
            text.contains("Textsimilarity"),
            "the similarity section is absent, so corpus_chunks_available never arrived"
        );
    }

    fn run_and_get_report(db: &Arc<Database>, embedder: Arc<dyn Embedder>) -> serde_json::Value {
        run_and_get_report_with(db, embedder, None)
    }

    /// `guidelines_url` is now an INPUT to the pipeline, not something the
    /// checklist re-derives from corpus state — so a test that ingests
    /// guidelines must pass the document it ingested.
    fn run_and_get_report_with(
        db: &Arc<Database>,
        embedder: Arc<dyn Embedder>,
        guidelines_url: Option<String>,
    ) -> serde_json::Value {
        let path = std::env::temp_dir()
            .join(format!("gaply_ck_e2e_{}_{}.txt", std::process::id(), now_epoch()));
        std::fs::write(&path, MANUSCRIPT).expect("write manuscript");
        let events: std::cell::RefCell<Vec<AnalysisEvent>> = std::cell::RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("checklist e2e".into()),
            None, // unauthenticated: keeps the verify lane off the cloud tier
            guidelines_url,
            NetworkConsent::Granted, // MANUSCRIPT has no References section
            &emit,
        )
        .expect("pipeline completes");
        let _ = std::fs::remove_file(&path);
        let report_id = events
            .into_inner()
            .iter()
            .find_map(|e| match e {
                AnalysisEvent::Finished { report_id } => Some(report_id.clone()),
                _ => None,
            })
            .expect("a Finished report id");
        let json = db
            .cache_get(&report_cache_key(&report_id), now_epoch())
            .unwrap()
            .expect("report cached");
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn ingested_guidelines_appear_in_the_pipeline_checklist() {
        force_heuristic();
        let db = Arc::new(Database::in_memory().unwrap());
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);

        // Ingest a journal guideline into the corpus (as guidelines.rs would).
        let doc = gaply_core::rag::RawDocument {
            source_type: gaply_core::rag::SourceType::JournalGuideline,
            title: "Author Guidelines".into(),
            source_url: "http://journal.test/guidelines".into(),
            fetched_at: now_epoch(),
            content: "Manuscripts must not exceed 3000 words. A structured abstract is required. \
                      Authors must include a conflict of interest declaration. References must \
                      follow a numbered Vancouver style."
                .into(),
        };
        gaply_core::rag::ingest_document(&db, embedder.as_ref(), &doc).unwrap();

        let report = run_and_get_report_with(
            &db,
            embedder,
            Some("http://journal.test/guidelines".into()),
        );
        let checklist = report["checklist"].as_array().expect("checklist array");
        assert!(!checklist.is_empty(), "checklist should be populated");
        assert!(
            checklist.iter().any(|c| !c["guideline_source"].is_null()),
            "expected a guideline-derived checklist item (guideline_source set), got {checklist:?}"
        );
    }

    #[test]
    fn no_guidelines_yields_structural_only_checklist_no_crash() {
        force_heuristic();
        let db = Arc::new(Database::in_memory().unwrap());
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);

        // No guidelines ingested → pipeline still completes; checklist has only
        // always-on structural checks, none guideline-derived (no fabrication).
        let report = run_and_get_report(&db, embedder);
        let checklist = report["checklist"].as_array().expect("checklist array");
        assert!(
            checklist.iter().all(|c| c["guideline_source"].is_null()),
            "no guidelines → no guideline-derived items, got {checklist:?}"
        );
    }
}

#[cfg(test)]
mod revision_pin {
    //! **§5.1 — production must be able to converge PAST round one.**
    //!
    //! Before this, the six debate participants were all `PrecomputedAgent` and
    //! `SwarmAgent::revise` defaults to `None`, so every run ended in round one
    //! with `revised_agents: []` — measured empty in 22 of 22 stored reports.
    //! The mesh round-table was a vote.
    //!
    //! # WHY TWO TESTS AND NOT ONE END-TO-END TEST
    //!
    //! The honest instrument would drive `run_pipeline_inner` and assert
    //! `rounds_run > 1`. **It cannot be written hermetically.** Revision needs a
    //! verification report with real citation ids, which needs `items`, which
    //! the lane only fills by calling CrossRef/OpenAlex per reference — real
    //! network, which this repo forbids in tests. Seeding the refverify cache to
    //! fake it would mean asserting on a path no user takes.
    //!
    //! So the property is split, and each half says what it cannot see:
    //!
    //! 1. [`the_debate_revises_when_a_peer_disagrees`] proves the MECHANISM
    //!    against the pipeline's own five precomputed peers. It cannot prove the
    //!    pipeline uses it.
    //! 2. [`the_pipeline_wires_the_reviser_into_the_verification_slot`] proves
    //!    the WIRING by reading the source. It cannot prove the mechanism works.
    //!
    //! Together they cover the regression; apart, either would pass while the
    //! debate was still a vote.

    use super::*;
    use gaply_core::extract::citations::Reference;
    use gaply_core::refverify::{ExistenceCheck, Provenance, ReferenceVerification};
    use gaply_core::swarm::{AgentKind, ANSWER_CONCERN};
    use gaply_core::verify_agent::{verify_citations, MockProxyClient};

    fn reference() -> Reference {
        Reference {
            raw: "Doe, J. (2022). A paper. https://doi.org/10.1/abc".into(),
            authors: "Doe, J.".into(),
            year: Some(2022),
            title: Some("A paper".into()),
            doi: Some("10.1/abc".into()),
        }
    }

    fn items() -> Vec<(Reference, ReferenceVerification)> {
        let mut rv = ReferenceVerification {
            reference_raw: "Doe, J. (2022). A paper.".into(),
            exists: None,
            retraction: None,
            open_access: None,
            enrichment: None,
            provenance: Vec::new(),
            warnings: Vec::new(),
        };
        let prov = Provenance {
            source: "crossref".into(),
            url: "https://api.crossref.org/works/10.1/abc".into(),
            fetched_at: 0,
            checksum: String::new(),
            from_cache: false,
        };
        rv.exists = Some(ExistenceCheck {
            source: "crossref",
            found: true,
            doi: Some("10.1/abc".into()),
            title: None,
            matched_authors: None,
            matched_year: Some(2022),
            is_retracted_hint: None,
            provenance: prov.clone(),
        });
        rv.provenance.push(prov);
        vec![(reference(), rv)]
    }

    /// **THE MECHANISM.** The verification participant revises when a peer
    /// contradicts it, and the debate therefore runs past round one.
    ///
    /// The peers are `PrecomputedAgent`s built by the same `adapters::*`
    /// functions `run_pipeline_inner` uses, with one deliberately dissenting —
    /// which is exactly the trigger `revising.rs` documents (a peer whose
    /// stance contradicts ours).
    #[test]
    fn the_debate_revises_when_a_peer_disagrees() {
        let proxy = MockProxyClient::returning_sequence(vec![
            serde_json::json!({"verdicts": [{
                "citation_id": "c1", "verdict": "SUPPORTED", "confidence": 0.9,
                "rationale": "matches", "evidence_refs": ["ev-c1-0"]
            }]}),
            serde_json::json!({"verdicts": [{
                "citation_id": "c1", "verdict": "UNKNOWN", "confidence": 0.3,
                "rationale": "peer similarity finding undermines support",
                "evidence_refs": ["ev-c1-0"]
            }]}),
        ]);
        let its = items();
        let initial = verify_citations(&proxy, &its).expect("initial verification");

        let mut reviser = RevisingVerificationAgent::new(&proxy, its, initial);
        let outcome = {
            let mut agents: Vec<Box<dyn SwarmAgent + '_>> = vec![
                // A dissenting peer — the contradiction that triggers revision.
                Box::new(PrecomputedAgent::new(gaply_core::swarm::Opinion {
                    agent: AgentKind::Plagiarism,
                    answer: ANSWER_CONCERN.into(),
                    explanation: "high-similarity match".into(),
                    confidence: 0.92,
                    hard_constraint: false,
                    gate_passed: true,
                })),
                Box::new(&mut reviser),
            ];
            run_debate(&mut agents, &DebateConfig::default()).expect("debate runs")
        };

        assert!(
            outcome.rounds_run > 1,
            "production converged in round one on a fixture designed to provoke revision — \
             the debate is a vote again: {outcome:?}"
        );
        assert!(
            outcome.revised_agents.contains(&AgentKind::Verification),
            "the verification agent must be recorded as having revised: {:?}",
            outcome.revised_agents
        );

        // And the REVISED report is what a caller gets back — the property
        // `compile_report` depends on, since it builds per-citation findings
        // from it and would otherwise contradict `revised_agents`.
        let (_returned_items, revised) = reviser.into_parts();
        assert!(
            revised.verdicts.iter().any(|v| v.verdict == gaply_core::verify_agent::Verdict::Unknown),
            "into_parts must hand back the RECONSIDERED report, not the original: {revised:?}"
        );
    }

    /// **THE WIRING.** The pipeline's consent-granted path constructs a
    /// `RevisingVerificationAgent`, and does NOT build the verification slot
    /// from `adapters::from_verification_report` there.
    ///
    /// A source scan, for the reason in the module header. What it cannot catch:
    /// a reviser that is constructed and then never reaches `run_debate`, or one
    /// whose revision is discarded. The mechanism test above covers neither of
    /// those either — only a hermetic end-to-end run would, and that is blocked
    /// on the network dependency described above.
    #[test]
    fn the_pipeline_wires_the_reviser_into_the_verification_slot() {
        let src = include_str!("pipeline.rs");
        let synth = src
            .split("--- synthesis: round-table debate")
            .nth(1)
            .expect("the synthesis block must exist")
            .split("#[cfg(test)]")
            .next()
            .expect("bounded by the test module");

        assert!(
            synth.contains("RevisingVerificationAgent::new("),
            "the synthesis block no longer constructs a RevisingVerificationAgent — the \
             verification participant cannot revise and every debate will converge in \
             round one"
        );
        // The precomputed verification participant is legitimate on exactly one
        // path: consent refused, where no proxy exists to reconsider with. It
        // must not appear outside that arm.
        let precomputed_verification =
            synth.matches("from_verification_report(").count();
        assert_eq!(
            precomputed_verification, 1,
            "expected exactly one precomputed verification participant (the consent-refused \
             arm); found {precomputed_verification}"
        );
    }
}
