//! Citation Intelligence — the AI engine's pure, local half.
//!
//! Plan: `docs/AI_ENGINE_PLAN.md`. This module tree is the gaply-core side of
//! the split described in §3: **SQL and pure logic live here; async, ML and the
//! model runtime live in the app crate** (`src-tauri/src/ai/`). The split is not
//! stylistic — [`crate::db::Database::conn`] is `pub(crate)`, so the app crate
//! cannot execute SQL at all, exactly as [`crate::citation_library`] documents.
//!
//! # The three hard rules (plan §1)
//!
//! **R1 — the deterministic citation path has ZERO dependency on this module.**
//! CSL formatting, DOI resolution, dedupe, import, export and the local citation
//! library must keep working with this tree deleted. The dependency arrow points
//! one way: the AI layer may read citation data, citation code never reads AI
//! data. Nothing in [`crate::citation_library`] may reference `ai_engine`.
//!
//! **R2 — no network calls anywhere in this tree.** No HTTP client, no sockets,
//! no localhost. gaply-core carries no network dependency at all, which makes
//! this structural here rather than advisory.
//!
//! **R3 — the SLM never generates citation strings, DOIs, years or metadata.**
//! Model output may only select, classify, rank or point at text that already
//! exists in a stored chunk. Every AI output row records `chunk_id`, `page`,
//! `model_id`, `prompt_version` and `created_at`, and the chunks a card drew on
//! are queryable foreign keys in `ai_evidence_card_chunks` — `provenance_json`
//! may duplicate them but is never the only record.
//!
//! # Phase 1 scope
//!
//! Schema (migration v14) plus page-aware chunk persistence. Deliberately no
//! embeddings, no retrieval, no model of any kind. The DAO here therefore covers
//! `ai_chunks` only: the jobs, evidence-card and model-registry DAOs ship with
//! the phases that actually write those tables, rather than landing now as
//! untested speculative code.

pub mod store;
