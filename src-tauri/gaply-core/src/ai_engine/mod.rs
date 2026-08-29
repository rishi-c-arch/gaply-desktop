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
//! # Scope so far
//!
//! **Phase 1** (migration v14): page-aware chunk persistence — [`store`].
//! **Phase 2** (migration v15): vector storage and similarity
//! ([`embeddings`]) plus FTS5-prefiltered retrieval ([`retrieval`]).
//!
//! This module stores and compares vectors; it never PRODUCES one. Producing a
//! vector needs a model, models need candle, and candle lives in the app crate
//! — the same seam that keeps `cargo test -p gaply_core` free of TLS and ML.
//!
//! The jobs and evidence-card DAOs ship with the phases that write those
//! tables, rather than landing now as untested speculative code.

pub mod cards;
pub mod audit_prepass;
pub mod embeddings;
pub mod jobs;
pub mod registry;
pub mod retrieval;
pub mod store;
