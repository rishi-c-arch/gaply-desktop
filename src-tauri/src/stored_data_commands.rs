//! **The Stored data section's one command (Research OS, Phase 5D-1a;
//! decisions 109, 205 to 207).**
//!
//! `stored_data_summary` counts what Gaply's older features keep on this
//! computer: nine categories, each with how many items and how many bytes.
//! The counting is `gaply_core::stored_data`'s, and it only reads. This module
//! resolves the folders, the app data folder resolved at start
//! (`AppState::app_data_dir`) and the OS temp folder, and calls it under
//! `spawn_blocking`. It takes nothing from the webview.
//!
//! **No workspace check (decisions 49, 120 and 206):** seeing what is stored
//! never needs a workspace (`tests/workspace_guards.rs`'s
//! `STORED_DATA_EXEMPT`).
//!
//! **A failure is one closed code**, `stored_data.unavailable`, whatever went
//! wrong: an SQLite or file-system message can quote a path, and the section
//! shows one fixed sentence anyway. The cause is logged by its kind only.

use std::path::Path;

use gaply_core::stored_data::StoredDataSummary;
use gaply_core::{now_epoch, Database, GaplyError};
use tauri::State;

use crate::state::AppState;

/// The one error this command answers with.
pub const UNAVAILABLE: &str = "stored_data.unavailable";

fn unavailable() -> GaplyError {
    GaplyError::Internal(UNAVAILABLE.into())
}

/// The work: count, and turn any failure into [`UNAVAILABLE`].
pub fn summary(
    db: &Database,
    app_data_dir: &Path,
    temp_dir: &Path,
) -> Result<StoredDataSummary, GaplyError> {
    db.stored_data_summary(app_data_dir, temp_dir, now_epoch())
        .map_err(|e| {
            tracing::warn!(error_kind = e.code(), "stored data summary failed");
            unavailable()
        })
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stored_data_summary(
    state: State<'_, AppState>,
) -> Result<StoredDataSummary, GaplyError> {
    let db = state.db.clone();
    let app_data_dir = state.app_data_dir.clone();
    let temp_dir = std::env::temp_dir();
    tokio::task::spawn_blocking(move || summary(&db, &app_data_dir, &temp_dir))
        .await
        .map_err(|_| unavailable())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::stored_data::{
        StoredCategory, COMPARISON_LOG_FILE, OA_PAPERS_DIR, TEMP_EXPORTS,
    };
    use std::path::PathBuf;

    /// A folder this test makes and removes.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let p =
                std::env::temp_dir().join(format!("gaply-5d1a-cmd-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(p.join("app")).unwrap();
            std::fs::create_dir_all(p.join("tmp")).unwrap();
            Scratch(p)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_work_counts_nine_categories_in_the_given_folders() {
        let s = Scratch::new("nine");
        let db = Database::open(&s.0.join("app/gaply.db")).unwrap();
        std::fs::write(s.0.join("app").join(COMPARISON_LOG_FILE), b"{}\n").unwrap();
        let got = summary(&db, &s.0.join("app"), &s.0.join("tmp")).unwrap();
        assert_eq!(got.categories.len(), 9);
        let log = got.get(StoredCategory::ComparisonLog);
        assert_eq!((log.items, log.bytes), (1, 3));
    }

    /// A folder it cannot read fails the whole summary, as one fixed code:
    /// no message from below, which could name a path, reaches the webview.
    #[cfg(unix)]
    #[test]
    fn a_failure_is_one_closed_code_with_no_path() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("fail");
        let db = Database::open(&s.0.join("app/gaply.db")).unwrap();
        let locked = s.0.join("app").join(OA_PAPERS_DIR);
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let got = summary(&db, &s.0.join("app"), &s.0.join("tmp"));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        let e = got.expect_err("an unreadable folder is a failure, not a zero");
        let wire = serde_json::to_string(&e).unwrap();
        assert_eq!(
            wire,
            r#"{"code":"internal","message":"internal error: stored_data.unavailable"}"#
        );
        assert!(!wire.contains(s.0.to_str().unwrap()));
    }

    /// The names the counting shares with the code that writes each file.
    #[test]
    fn the_counted_names_are_the_names_the_writers_use() {
        let app = Path::new("APP");
        assert_eq!(crate::oa_fetch::oa_dir(app), app.join(OA_PAPERS_DIR));
        let harness = include_str!("harness_log.rs");
        assert!(
            harness.contains(&format!(
                "const HARNESS_LOG_FILE: &str = \"{COMPARISON_LOG_FILE}\";"
            )),
            "harness_log.rs names another file"
        );
        let commands = include_str!("commands.rs");
        for (prefix, suffix) in TEMP_EXPORTS {
            assert!(
                commands.contains(&format!(
                    "name.starts_with(\"{prefix}\") && name.ends_with(\"{suffix}\")"
                )),
                "no export sweeps {prefix}*{suffix}"
            );
            assert!(
                commands.contains(&format!("format!(\"{prefix}")),
                "no export writes {prefix}…"
            );
        }
    }
}
