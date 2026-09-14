//! Gaply core: portable business logic with no Tauri dependency.
//!
//! Rule: anything that could run in a CLI, a server, or a different shell
//! lives here. The Tauri layer only adapts these functions to IPC.

pub mod ai_detect;
pub mod ai_engine;
pub mod ai_features;
pub mod ai_signals;
pub mod app_check;
pub mod audit_report;
pub mod cache;
pub mod chat_agent;
pub mod agent_graph;
pub mod consent;
pub mod research_state;
pub mod chunk;
pub mod citation_links;
pub mod citation_library;
pub mod config;
pub mod consistency;
pub mod page_locate;
pub mod db;
pub mod embed;
pub mod epistemic;
pub mod equation;
pub mod equation_report;
pub mod error;
pub mod escalation;
pub mod evidence;
pub mod evidence_store;
pub mod extract;
pub mod gap_finder_agent;
pub mod journal_corpus;
pub mod journal_expect;
pub mod journal_extract;
pub mod journal_standards;
pub mod journal_fingerprint;
pub mod journal_store;
pub mod memory;
pub mod import_guard;
pub mod migrations;
pub mod notes;
pub mod orchestrator;
pub mod perplexity;
pub mod oa_fetch;
pub mod plagiarism;
pub mod source_ref;
pub mod staged_sources;
pub mod plagiarism_exact;
pub mod plagiarism_library;
pub mod projects;
pub mod rag;
pub mod ratelimit;
pub mod report;
pub mod quote_clean;
pub mod report_compose;
pub mod report_model;
pub mod report_html;
pub mod report_pdf;
pub mod refverify;
pub mod reviewer_agent;
pub mod reviewer_harness;
pub mod sanitize;
pub mod scientific_model;
pub mod secrets;
pub mod stage1_norms;
pub mod stats_chat;
pub mod stats_verdict;
pub mod stats_verify;
pub mod swarm;
pub mod validate;
pub mod verify_agent;
pub mod vocabulary;
pub mod vector;

pub use config::AppConfig;
pub use db::Database;
pub use error::GaplyError;

/// Version of the core crate, independent of the shell wrapping it.
pub fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Current Unix time in seconds.
pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
