//! Gaply core: portable business logic with no Tauri dependency.
//!
//! Rule: anything that could run in a CLI, a server, or a different shell
//! lives here. The Tauri layer only adapts these functions to IPC.

pub mod ai_detect;
pub mod app_check;
pub mod cache;
pub mod chunk;
pub mod config;
pub mod db;
pub mod embed;
pub mod error;
pub mod extract;
pub mod memory;
pub mod migrations;
pub mod perplexity;
pub mod plagiarism;
pub mod projects;
pub mod rag;
pub mod ratelimit;
pub mod refverify;
pub mod sanitize;
pub mod secrets;
pub mod validate;
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
