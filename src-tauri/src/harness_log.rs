//! Durable sink for the Box 4 shadow-comparison record.
//!
//! # Why this exists
//!
//! `build_comparison_report` has always produced a complete, correctly typed
//! record — and emitted it to `tracing::info!`, which `logging.rs:9-13`
//! configures with no writer, no file appender and no rolling log. The record
//! therefore reached the running process's stdout and vanished with it.
//!
//! That gap was mistaken twice for "the measurement is one live run away". It is
//! not: the run would work and would produce **nothing that accumulates**. An
//! instrument that emits to stdout is an instrument without a record
//! (ARCHITECTURE_TRACE §12.3.2).
//!
//! This module makes a run able to LEAVE a record. It does not make the run, and
//! it changes nothing a user sees.
//!
//! # What it is for
//!
//! **Regression detection**, which log-only cannot do — detecting that agreement
//! changed requires a baseline. Specifically, it exists so the pre-promotion
//! baseline can be captured BEFORE the deterministic Box 4 verdict is promoted
//! (Group 3), while the current behaviour is still current.
//!
//! Not telemetry: the file stays on the user's machine. Records carry no
//! manuscript text, but they do carry recommendations and finding breakdowns
//! about an unpublished paper, so sending them anywhere is a separate
//! privacy-boundary decision and is deliberately not taken here.
//!
//! # Injection
//!
//! `gaply-core` stays Tauri-free, so the directory is injected from the app layer
//! at startup — the `set_bundled_models_dir` precedent (`models/mod.rs:55`).
//! Unset (tests, headless), appending is a silent no-op.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use gaply_core::reviewer_harness::ShadowComparisonReport;

/// File name under the injected directory. JSON Lines: append-only, one record
/// per line, readable without parsing the whole file — the format a baseline
/// wants, since records accumulate and are never rewritten.
const HARNESS_LOG_FILE: &str = "box4_comparisons.jsonl";

static HARNESS_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Record where comparison records are appended. Call once from the Tauri
/// `setup` with the app data dir. Idempotent (later calls no-op).
pub fn set_dir(dir: PathBuf) {
    let _ = HARNESS_LOG_DIR.set(dir);
}

fn log_path() -> Option<PathBuf> {
    HARNESS_LOG_DIR.get().map(|d| d.join(HARNESS_LOG_FILE))
}

/// Append one comparison record.
///
/// Best-effort by design: a failure to record a diagnostic must never fail an
/// analysis the user asked for, so every error is logged and swallowed. Returns
/// `true` when a line was written, which is what the tests assert on.
pub fn append(report: &ShadowComparisonReport) -> bool {
    match log_path() {
        Some(p) => append_to(&p, report),
        None => false, // not configured (tests, headless) — silent no-op
    }
}

/// The write itself, against an explicit path.
///
/// Split out from [`append`] because `HARNESS_LOG_DIR` is a process-wide
/// `OnceLock`: a test that set it could never unset it, so routing through
/// `append` alone would leave the only behaviour that matters — a record
/// actually landing on disk — untested.
fn append_to(path: &std::path::Path, report: &ShadowComparisonReport) -> bool {
    let line = match report.to_jsonl_line() {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, "box4: could not serialize comparison record");
            return false;
        }
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = %e, dir = %parent.display(), "box4: could not create record dir");
            return false;
        }
    }
    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => match writeln!(f, "{line}") {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(error = %e, "box4: could not append comparison record");
                false
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "box4: could not open record file");
            false
        }
    }
}

/// sha256 of a manuscript file, lowercase hex.
///
/// `run_id` is `manuscript_id.to_string()` — a local DB row id, meaningless
/// across machines, and the same id on two machines denotes different
/// manuscripts. This is the identity two records can actually be compared on,
/// and it is the same hash the frozen evaluation artifacts use.
pub fn manuscript_sha256(path: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut h = Sha256::new();
            h.update(&bytes);
            format!("{:x}", h.finalize())
        }
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "box4: could not hash manuscript");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_a_missing_file_is_empty_not_a_panic() {
        assert_eq!(manuscript_sha256(std::path::Path::new("/no/such/file")), "");
    }

    #[test]
    fn the_hash_is_stable_and_content_addressed() {
        let dir = std::env::temp_dir().join(format!("gaply_h_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        std::fs::write(&a, b"same bytes").unwrap();
        std::fs::write(&b, b"same bytes").unwrap();
        let ha = manuscript_sha256(&a);
        assert_eq!(ha.len(), 64, "lowercase hex sha256");
        assert_eq!(ha, manuscript_sha256(&b), "identity is content, not path");
        assert_eq!(ha, manuscript_sha256(&a), "stable across calls");
        std::fs::write(&b, b"other bytes").unwrap();
        assert_ne!(ha, manuscript_sha256(&b));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn a_report(run: &str, sha: &str) -> ShadowComparisonReport {
        use gaply_core::reviewer_agent::ReviewerEvaluation;
        use gaply_core::reviewer_harness::{build_comparison_report, HarnessInputs, HarnessTiming};
        let ev = ReviewerEvaluation::unavailable_offline();
        build_comparison_report(&HarnessInputs {
            run_id: run,
            manuscript_sha256: sha,
            recorded_at: 7,
            shadow: &ev,
            shadow_breakdown: &Default::default(),
            shadow_findings_sent: 0,
            shadow_narrative_available: false,
            wholesale: &ev,
            timing: HarnessTiming::default(),
            proxy_meta: None,
        })
    }

    #[test]
    fn records_accumulate_one_json_object_per_line() {
        // THE point of the module: a run must leave a record, and records must
        // accumulate rather than overwrite.
        let dir = std::env::temp_dir().join(format!("gaply_hl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("acc.jsonl");
        let _ = std::fs::remove_file(&path);

        assert!(append_to(&path, &a_report("r1", "aaa")));
        assert!(append_to(&path, &a_report("r2", "bbb")));

        let body = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2, "appended, not overwritten");
        for l in &lines {
            let v: serde_json::Value = serde_json::from_str(l).expect("each line is one JSON object");
            assert!(v["manuscript_sha256"].is_string(), "identity is in the record");
            assert_eq!(v["recorded_at"], 7, "timestamp is in the record, not just the log line");
            assert!(v["recommendation_agreement"].is_object(), "the comparison metric is recorded");
        }
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[0]).unwrap()["manuscript_sha256"],
            "aaa"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unwritable_path_is_reported_not_panicked() {
        assert!(!append_to(std::path::Path::new("/no/such/dir/x.jsonl"), &a_report("r", "s")));
    }

    #[test]
    fn appending_without_a_configured_dir_is_a_silent_no_op() {
        // The OnceLock is process-wide and unset in tests, so this exercises the
        // headless path: a diagnostic must never fail the analysis.
        use gaply_core::reviewer_agent::ReviewerEvaluation;
        use gaply_core::reviewer_harness::{build_comparison_report, HarnessInputs, HarnessTiming};
        let ev = ReviewerEvaluation::unavailable_offline();
        let report = build_comparison_report(&HarnessInputs {
            run_id: "r1",
            manuscript_sha256: "abc",
            recorded_at: 1,
            shadow: &ev,
            shadow_breakdown: &Default::default(),
            shadow_findings_sent: 0,
            shadow_narrative_available: false,
            wholesale: &ev,
            timing: HarnessTiming::default(),
            proxy_meta: None,
        });
        assert_eq!(report.manuscript_sha256, "abc", "identity is carried into the record");
        assert_eq!(report.recorded_at, 1);
        assert!(report.to_jsonl_line().is_ok(), "the record must serialize");
        assert!(!append(&report), "no dir configured -> no write, no panic");
    }
}
