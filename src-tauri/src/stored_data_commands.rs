//! **The Stored data section's commands (Research OS, Phases 5D-1a and
//! 5D-1b; decisions 109, 205 to 207, 210, 213 and 214).**
//!
//! `stored_data_summary` counts what Gaply's older features keep on this
//! computer: nine categories, each with how many items and how many bytes.
//! `stored_data_prepare_delete` issues a confirmation for one category, and
//! `stored_data_delete` redeems it and deletes that category. The counting
//! and the deleting are `gaply_core::stored_data`'s. This module resolves the
//! folders, the app data folder resolved at start (`AppState::app_data_dir`)
//! and the OS temp folder, and calls it under `spawn_blocking`. It takes only
//! a category's name and a confirmation from the webview.
//!
//! **A delete (decision 213)** needs a confirmation issued for its category
//! just before, by decision 124's rule ([`crate::confirmation`]), held in
//! [`StoredDataSession`], apart from the projects' own. It is refused, each
//! refusal a closed code, while work that writes into its category is going
//! ([`RunningWork`], decision 214): the six commands that write take a guard
//! first, and the delete holds the count while it runs, so none starts
//! midway. After categories 1 to 5 it runs the checkpoint that flushes the
//! delete's zeroed pages into the file (5E-fix-1, decision 224: no VACUUM),
//! and its view says whether it finished.
//!
//! **No workspace check (decisions 49, 120 and 206):** seeing and deleting
//! what is stored never needs a workspace (`tests/workspace_guards.rs`'s
//! `STORED_DATA_EXEMPT`).
//!
//! **A failure is one closed code**, `stored_data.unavailable`, whatever went
//! wrong: an SQLite or file-system message can quote a path, and the section
//! shows one fixed sentence anyway. The cause is logged by its kind only.
//! Logs carry closed codes and counts, never a title, name, path or URL.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use gaply_core::contracts::capability::ConfirmationId;
use gaply_core::stored_data::{
    CategoryCount, Compaction, DeleteError, Deletion, StoredCategory, StoredDataSummary,
};
use gaply_core::{now_epoch, Database, GaplyError};
use serde::Serialize;
use tauri::State;

use crate::confirmation::{
    Confirmations, Monotonic, NoRandomness, Refused, CONFIRMATION_LIFETIME_MS,
};
use crate::runs::Clock;
use crate::state::AppState;

/// The one error this command answers with.
pub const UNAVAILABLE: &str = "stored_data.unavailable";

fn unavailable() -> GaplyError {
    GaplyError::Internal(UNAVAILABLE.into())
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/* ------------------------------------------------------------------------ *
 *  Refusals (decisions 213 and 214)
 * ------------------------------------------------------------------------ */

/// Why a delete was not done, each a closed code with its own sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredDataRefusal {
    UnknownCategory,
    ConfirmationMissing,
    ConfirmationUnknown,
    ConfirmationUsed,
    ConfirmationReplaced,
    ConfirmationExpired,
    ConfirmationOtherCategory,
    /// Categories 1 and 4, while a full analysis, a PublishReady run,
    /// `validate_manuscript` or `extract_manuscript` is going.
    AnalysisRunning,
    /// Category 2, while a Gap Finder corpus build is going.
    GapFinderRunning,
    /// Category 2, always, while a citation check's rows reference one of
    /// its documents.
    PapersInUse,
    /// Category 3, while the jobs map holds any job, a paused one included.
    JobRunning,
    /// Category 7, while an open-access fetch is going.
    FetchRunning,
}

impl StoredDataRefusal {
    pub const ALL: [StoredDataRefusal; 12] = [
        StoredDataRefusal::UnknownCategory,
        StoredDataRefusal::ConfirmationMissing,
        StoredDataRefusal::ConfirmationUnknown,
        StoredDataRefusal::ConfirmationUsed,
        StoredDataRefusal::ConfirmationReplaced,
        StoredDataRefusal::ConfirmationExpired,
        StoredDataRefusal::ConfirmationOtherCategory,
        StoredDataRefusal::AnalysisRunning,
        StoredDataRefusal::GapFinderRunning,
        StoredDataRefusal::PapersInUse,
        StoredDataRefusal::JobRunning,
        StoredDataRefusal::FetchRunning,
    ];

    pub fn wire(self) -> &'static str {
        match self {
            StoredDataRefusal::UnknownCategory => "stored_data.unknown_category",
            StoredDataRefusal::ConfirmationMissing => "stored_data.confirmation_missing",
            StoredDataRefusal::ConfirmationUnknown => "stored_data.confirmation_unknown",
            StoredDataRefusal::ConfirmationUsed => "stored_data.confirmation_used",
            StoredDataRefusal::ConfirmationReplaced => "stored_data.confirmation_replaced",
            StoredDataRefusal::ConfirmationExpired => "stored_data.confirmation_expired",
            StoredDataRefusal::ConfirmationOtherCategory => {
                "stored_data.confirmation_other_category"
            }
            StoredDataRefusal::AnalysisRunning => "stored_data.analysis_running",
            StoredDataRefusal::GapFinderRunning => "stored_data.gap_finder_running",
            StoredDataRefusal::PapersInUse => "stored_data.papers_in_use",
            StoredDataRefusal::JobRunning => "stored_data.job_running",
            StoredDataRefusal::FetchRunning => "stored_data.fetch_running",
        }
    }
}

/// A refusal, as the webview reads it: `{"code":"stored_data.job_running"}`,
/// the shape of a project refusal.
fn refuse(r: StoredDataRefusal) -> GaplyError {
    tracing::info!(code = r.wire(), "stored data: refused");
    GaplyError::Validation(serde_json::json!({ "code": r.wire() }).to_string())
}

/* ------------------------------------------------------------------------ *
 *  Running work (decision 214)
 * ------------------------------------------------------------------------ */

/// Work that writes into a Stored data category. Category 3's work is the
/// thesis audits' own jobs map (`AppState::ai_jobs`), not a counter here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    /// `run_full_analysis`, `run_publishready`, `validate_manuscript`,
    /// `extract_manuscript`: they write categories 1 and 4.
    Analysis,
    /// `build_gapfinder_corpus`: category 2.
    GapFinder,
    /// `citation_fetch_oa`: category 7.
    OpenAccessFetch,
}

impl Work {
    fn index(self) -> usize {
        match self {
            Work::Analysis => 0,
            Work::GapFinder => 1,
            Work::OpenAccessFetch => 2,
        }
    }

    fn refusal(self) -> StoredDataRefusal {
        match self {
            Work::Analysis => StoredDataRefusal::AnalysisRunning,
            Work::GapFinder => StoredDataRefusal::GapFinderRunning,
            Work::OpenAccessFetch => StoredDataRefusal::FetchRunning,
        }
    }
}

/// The work that writes into `category`, if any counter holds it.
fn work_for(category: StoredCategory) -> Option<Work> {
    match category {
        StoredCategory::ManuscriptAnalyses | StoredCategory::CachedReports => Some(Work::Analysis),
        StoredCategory::GapFinderPapers => Some(Work::GapFinder),
        StoredCategory::OpenAccessPapers => Some(Work::OpenAccessFetch),
        _ => None,
    }
}

/// How many of each kind of [`Work`] are going in this copy of the app.
/// In memory only.
#[derive(Debug, Default)]
pub struct RunningWork {
    going: Mutex<[usize; 3]>,
}

/// Held while one piece of work runs; dropping it ends that work's count.
#[must_use = "the work counts as going only while this is held"]
pub struct Going {
    work: Work,
    of: Arc<RunningWork>,
}

impl RunningWork {
    /// Count `work` as going until the guard drops. Waits only while a
    /// delete of a category it writes into is running.
    pub fn enter(self: &Arc<Self>, work: Work) -> Going {
        lock(&self.going)[work.index()] += 1;
        Going {
            work,
            of: Arc::clone(self),
        }
    }

    /// How many of `work` are going now.
    pub fn going(&self, work: Work) -> usize {
        lock(&self.going)[work.index()]
    }
}

impl Drop for Going {
    fn drop(&mut self) {
        let mut going = lock(&self.of.going);
        let n = &mut going[self.work.index()];
        *n = n.saturating_sub(1);
    }
}

/// Runs `f` only while nothing that writes into `category` is going, and
/// holds that state until `f` returns: no counted work starts, and no job
/// is registered, while a delete runs (decision 214).
pub fn without_work<V, T>(
    category: StoredCategory,
    running: &RunningWork,
    jobs: &Mutex<HashMap<i64, V>>,
    f: impl FnOnce() -> T,
) -> Result<T, StoredDataRefusal> {
    let counted = match work_for(category) {
        Some(work) => {
            let going = lock(&running.going);
            if going[work.index()] > 0 {
                return Err(work.refusal());
            }
            Some(going)
        }
        None => None,
    };
    // The whole category, or none of it: no job is skipped (decision 214).
    let registered = if category == StoredCategory::ThesisAudits {
        let map = lock(jobs);
        if !map.is_empty() {
            return Err(StoredDataRefusal::JobRunning);
        }
        Some(map)
    } else {
        None
    };
    let out = f();
    drop(registered);
    drop(counted);
    Ok(out)
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

/* ------------------------------------------------------------------------ *
 *  The deletes (decision 213)
 * ------------------------------------------------------------------------ */

/// The Stored data section's delete confirmations: its own book, so a
/// confirmation issued here never voids a project's, nor a project's one
/// here (decision 124's rule, each with its own instance). In memory only.
pub struct StoredDataSession {
    clock: Arc<dyn Clock>,
    book: Mutex<Confirmations<StoredCategory>>,
}

impl Default for StoredDataSession {
    fn default() -> Self {
        StoredDataSession::with(Arc::new(Monotonic(Instant::now())))
    }
}

impl StoredDataSession {
    /// A session with its clock given: tests pass one they move.
    pub fn with(clock: Arc<dyn Clock>) -> Self {
        StoredDataSession {
            clock,
            book: Mutex::new(Confirmations::default()),
        }
    }

    /// A new confirmation for `category`; the one before it, for any
    /// category, is void from now on.
    pub(crate) fn issue(&self, category: StoredCategory) -> Result<ConfirmationId, GaplyError> {
        let now = self.clock.now_ms();
        lock(&self.book)
            .issue(category, now)
            .map_err(|NoRandomness| unavailable())
    }

    /// Uses the confirmation for deleting `category`.
    pub(crate) fn redeem(
        &self,
        category: StoredCategory,
        given: Option<&str>,
    ) -> Result<(), StoredDataRefusal> {
        let now = self.clock.now_ms();
        lock(&self.book)
            .redeem(&category, given, now)
            .map_err(|r| match r {
                Refused::Missing => StoredDataRefusal::ConfirmationMissing,
                Refused::Unknown => StoredDataRefusal::ConfirmationUnknown,
                Refused::Used => StoredDataRefusal::ConfirmationUsed,
                Refused::Replaced => StoredDataRefusal::ConfirmationReplaced,
                Refused::Expired => StoredDataRefusal::ConfirmationExpired,
                Refused::OtherSubject => StoredDataRefusal::ConfirmationOtherCategory,
            })
    }
}

/// A delete confirmation and the category's count now: counts only.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConfirmationView {
    pub confirmation: String,
    pub category: StoredCategory,
    pub items: u64,
    pub bytes: u64,
    pub expires_in_ms: u64,
}

/// What a delete did: the category's counts before and after, how many
/// items it removed, and its outcome, `done` or `not_done` with its closed
/// code. Counts, closed codes and values only.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedView {
    pub category: StoredCategory,
    pub before: CategoryCount,
    pub after: CategoryCount,
    pub deleted: u64,
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'static str>,
    /// A partial delete only (decision 230): its checkpoint's outcome,
    /// `done` or the checkpoint's closed code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<&'static str>,
}

/// A file category whose delete stopped at a failed removal.
pub const REMOVE_FAILED: &str = "stored_data.remove_failed";

/// Category 1's delete, stopped after a batch after the first failed: the
/// earlier batches stay deleted (decision 230).
pub const DELETE_PARTIAL: &str = "stored_data.delete_partial";

/// A delete's view: for categories 1 to 5 its compaction decides the
/// outcome, and for 6 to 9 whether every removal succeeded. A partial
/// delete is not done whatever its checkpoint did, and carries that in
/// `checkpoint` (decision 230).
fn view(d: Deletion, compaction: Option<Compaction>) -> DeletedView {
    let checkpoint_code = match compaction {
        Some(Compaction::NotDone(code)) => Some(code.wire()),
        _ => None,
    };
    let (code, checkpoint) = if d.partial {
        (
            Some(DELETE_PARTIAL),
            Some(checkpoint_code.unwrap_or("done")),
        )
    } else {
        let code = match compaction {
            Some(_) => checkpoint_code,
            None => d.removal_failed.then_some(REMOVE_FAILED),
        };
        (code, None)
    };
    DeletedView {
        category: d.category,
        before: d.before,
        after: d.after,
        deleted: d.deleted,
        outcome: if code.is_none() { "done" } else { "not_done" },
        code,
        checkpoint,
    }
}

fn category_of(wire: &str) -> Result<StoredCategory, GaplyError> {
    StoredCategory::parse(wire).ok_or_else(|| refuse(StoredDataRefusal::UnknownCategory))
}

/// The work of `stored_data_prepare_delete`: a confirmation for
/// `category`, with its count now.
pub fn prepare_delete(
    db: &Database,
    session: &StoredDataSession,
    app_data_dir: &Path,
    temp_dir: &Path,
    category: &str,
) -> Result<DeleteConfirmationView, GaplyError> {
    let category = category_of(category)?;
    let count = summary(db, app_data_dir, temp_dir)?.get(category);
    let confirmation = session.issue(category)?;
    tracing::info!(
        category = category.wire(),
        "stored data: delete confirmation issued"
    );
    Ok(DeleteConfirmationView {
        confirmation: confirmation.as_str().to_string(),
        category,
        items: count.items,
        bytes: count.bytes,
        expires_in_ms: CONFIRMATION_LIFETIME_MS,
    })
}

/// What a delete reads besides the database: the confirmations, the work
/// going, the thesis audits' jobs map and the two folders.
pub struct DeleteContext<'a> {
    pub session: &'a StoredDataSession,
    pub running: &'a RunningWork,
    pub jobs: &'a Mutex<HashMap<i64, crate::ai::job_runner::JobControl>>,
    pub app_data_dir: &'a Path,
    pub temp_dir: &'a Path,
}

/// The work of `stored_data_delete`: redeem the confirmation, check
/// decision 214, delete, and for categories 1 to 5 compact.
pub fn delete(
    db: &Database,
    cx: &DeleteContext<'_>,
    category: &str,
    confirmation: Option<&str>,
) -> Result<DeletedView, GaplyError> {
    let category = category_of(category)?;
    cx.session.redeem(category, confirmation).map_err(refuse)?;
    let deletion = without_work(category, cx.running, cx.jobs, || {
        db.stored_data_delete(category, cx.app_data_dir, cx.temp_dir, now_epoch())
    })
    .map_err(refuse)?
    .map_err(|e| match e {
        DeleteError::PapersInUse => refuse(StoredDataRefusal::PapersInUse),
        DeleteError::Failed(e) => {
            tracing::warn!(
                category = category.wire(),
                error_kind = e.code(),
                "stored data delete failed"
            );
            unavailable()
        }
    })?;
    // Outside the hold on running work: the rows are committed, and a
    // compaction can take a while on a large file.
    let compaction = category.in_database().then(|| db.stored_data_compact());
    let view = view(deletion, compaction);
    tracing::info!(
        category = category.wire(),
        deleted = view.deleted,
        outcome = view.outcome,
        code = view.code.unwrap_or(""),
        checkpoint = view.checkpoint.unwrap_or(""),
        "stored data: deleted"
    );
    Ok(view)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stored_data_prepare_delete(
    state: State<'_, AppState>,
    session: State<'_, Arc<StoredDataSession>>,
    category: String,
) -> Result<DeleteConfirmationView, GaplyError> {
    let db = state.db.clone();
    let session = Arc::clone(&session);
    let app_data_dir = state.app_data_dir.clone();
    let temp_dir = std::env::temp_dir();
    tokio::task::spawn_blocking(move || {
        prepare_delete(&db, &session, &app_data_dir, &temp_dir, &category)
    })
    .await
    .map_err(|_| unavailable())?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stored_data_delete(
    state: State<'_, AppState>,
    session: State<'_, Arc<StoredDataSession>>,
    category: String,
    confirmation: Option<String>,
) -> Result<DeletedView, GaplyError> {
    let db = state.db.clone();
    let session = Arc::clone(&session);
    let running = state.running.clone();
    let jobs = state.ai_jobs.clone();
    let app_data_dir = state.app_data_dir.clone();
    let temp_dir = std::env::temp_dir();
    // Off the command's thread: the delete, then the checkpoint.
    tokio::task::spawn_blocking(move || {
        let cx = DeleteContext {
            session: &session,
            running: &running,
            jobs: &jobs,
            app_data_dir: &app_data_dir,
            temp_dir: &temp_dir,
        };
        delete(&db, &cx, &category, confirmation.as_deref())
    })
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

    /* ---- running work (decision 214) --------------------------------- */

    use crate::ai::job_runner::JobControl;
    use StoredCategory::*;

    type Jobs = Mutex<HashMap<i64, JobControl>>;

    fn no_jobs() -> Jobs {
        Mutex::new(HashMap::new())
    }

    /// The refusal `category` meets now, or None.
    fn meets(
        category: StoredCategory,
        running: &RunningWork,
        jobs: &Jobs,
    ) -> Option<StoredDataRefusal> {
        without_work(category, running, jobs, || ()).err()
    }

    #[test]
    fn each_kind_of_work_refuses_the_categories_it_writes_while_it_is_going() {
        let jobs = no_jobs();
        for (work, refused, code) in [
            (
                Work::Analysis,
                &[ManuscriptAnalyses, CachedReports][..],
                StoredDataRefusal::AnalysisRunning,
            ),
            (
                Work::GapFinder,
                &[GapFinderPapers][..],
                StoredDataRefusal::GapFinderRunning,
            ),
            (
                Work::OpenAccessFetch,
                &[OpenAccessPapers][..],
                StoredDataRefusal::FetchRunning,
            ),
        ] {
            let running = Arc::new(RunningWork::default());
            let going = running.enter(work);
            assert_eq!(running.going(work), 1, "{work:?}");
            for c in StoredCategory::ALL {
                let want = refused.contains(&c).then_some(code);
                assert_eq!(meets(c, &running, &jobs), want, "{work:?} going, {c:?}");
            }
            drop(going);
            assert_eq!(running.going(work), 0, "{work:?}");
            for c in StoredCategory::ALL {
                assert_eq!(meets(c, &running, &jobs), None, "{work:?} ended, {c:?}");
            }
        }
    }

    #[test]
    fn two_pieces_of_the_same_work_each_count_until_their_own_guard_drops() {
        let jobs = no_jobs();
        let running = Arc::new(RunningWork::default());
        let first = running.enter(Work::Analysis);
        let second = running.enter(Work::Analysis);
        assert_eq!(running.going(Work::Analysis), 2);
        drop(first);
        assert_eq!(
            meets(ManuscriptAnalyses, &running, &jobs),
            Some(StoredDataRefusal::AnalysisRunning)
        );
        drop(second);
        assert_eq!(meets(ManuscriptAnalyses, &running, &jobs), None);
    }

    #[test]
    fn thesis_audits_are_refused_while_the_jobs_map_holds_any_job_a_paused_one_included() {
        let running = RunningWork::default();
        let jobs = no_jobs();
        let paused = JobControl::new(7);
        paused.pause();
        lock(&jobs).insert(7, paused);
        assert_eq!(
            meets(ThesisAudits, &running, &jobs),
            Some(StoredDataRefusal::JobRunning)
        );
        for c in StoredCategory::ALL {
            if c != ThesisAudits {
                assert_eq!(meets(c, &running, &jobs), None, "{c:?}");
            }
        }
        lock(&jobs).clear();
        assert_eq!(meets(ThesisAudits, &running, &jobs), None);
    }

    #[test]
    fn a_job_left_running_by_a_crash_and_held_by_no_runner_is_deleted() {
        let s = Scratch::new("crashed-job");
        let db = Database::open(&s.0.join("app/gaply.db")).unwrap();
        use gaply_core::ai_engine::jobs::{self, ItemKind, JobStatus, NewItem};
        let job = jobs::create_job(
            &db,
            "thesis_audit",
            None,
            None,
            "v1",
            &[NewItem {
                seq: 0,
                kind: ItemKind::CitationNeed,
                chunk_id: None,
                page: None,
                sentence: "A sentence.".into(),
                payload_json: "{}".into(),
            }],
        )
        .unwrap();
        jobs::set_job_status(&db, job, JobStatus::Running).unwrap();
        // What a crash leaves: running in the database, and no runner here.
        assert_eq!(jobs::interrupted_jobs(&db).unwrap(), [job]);
        let running = RunningWork::default();
        let d = without_work(ThesisAudits, &running, &no_jobs(), || {
            db.stored_data_delete(
                ThesisAudits,
                &s.0.join("app"),
                &s.0.join("tmp"),
                now_epoch(),
            )
        })
        .expect("no runner holds it")
        .expect("deleted");
        assert_eq!((d.before.items, d.deleted, d.after.items), (1, 1, 0));
    }

    #[test]
    fn work_that_starts_during_a_delete_waits_for_it_to_finish() {
        let running = Arc::new(RunningWork::default());
        let (tx, rx) = std::sync::mpsc::channel();
        without_work(ManuscriptAnalyses, &running, &no_jobs(), || {
            let r = Arc::clone(&running);
            std::thread::spawn(move || {
                let going = r.enter(Work::Analysis);
                tx.send(()).unwrap();
                drop(going);
            });
            assert!(
                rx.recv_timeout(std::time::Duration::from_millis(200))
                    .is_err(),
                "an analysis started while the delete held its category"
            );
        })
        .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("it started once the delete returned");
    }

    /// The commands of decision 214, each taking its guard as its first
    /// statement; and no other function in the app takes one.
    const GUARDED: &[(&str, &str, &str)] = &[
        ("commands.rs", "extract_manuscript", "Analysis"),
        ("commands.rs", "validate_manuscript", "Analysis"),
        ("commands.rs", "run_publishready", "Analysis"),
        ("pipeline.rs", "run_full_analysis", "Analysis"),
        ("commands.rs", "build_gapfinder_corpus", "GapFinder"),
        ("commands.rs", "citation_fetch_oa", "OpenAccessFetch"),
    ];

    fn source(file: &str) -> &'static str {
        match file {
            "commands.rs" => include_str!("commands.rs"),
            "pipeline.rs" => include_str!("pipeline.rs"),
            _ => panic!("{file}"),
        }
    }

    #[test]
    fn each_named_command_takes_its_guard_first() {
        let mut failures = Vec::new();
        for (file, name, work) in GUARDED {
            let src = source(file);
            let start = [format!("pub fn {name}("), format!("pub async fn {name}(")]
                .iter()
                .find_map(|sig| src.find(sig.as_str()))
                .unwrap_or_else(|| panic!("{file}: no fn {name}"));
            let open = start + src[start..].find('{').expect("a body");
            let first = src[open + 1..].trim_start();
            let want = format!("let _going = state.running.enter(Work::{work});");
            if !first.starts_with(&want) {
                failures.push(format!(
                    "{file}::{name} does not take its guard first: {:?}",
                    first.lines().next().unwrap_or("")
                ));
            }
        }
        // Every guard in these files is one of the six, so a seventh writer
        // would have to be named here; and `Work` there is this module's.
        let mut sites = 0;
        for file in ["commands.rs", "pipeline.rs"] {
            sites += source(file).matches("state.running.enter(").count();
            if !source(file).contains("\nuse crate::stored_data_commands::Work;\n") {
                failures.push(format!("{file} does not import stored_data_commands::Work"));
            }
        }
        if sites != GUARDED.len() {
            failures.push(format!("{sites} guards taken, {} named", GUARDED.len()));
        }
        assert!(
            failures.is_empty(),
            "decision 214:\n{}",
            failures.join("\n")
        );
    }

    /* ---- the commands and the confirmation (decision 213) ------------ */

    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Default)]
    struct Moved(AtomicU64);

    impl Clock for Moved {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    /// A database and folders with something in every category, each text
    /// and file name carrying a sentinel.
    struct Fixture {
        s: Scratch,
        db: Database,
        /// The Gap Finder paper's document.
        gap: i64,
    }

    const SENTINEL: &str = "SENTINEL_5D1B";

    impl Fixture {
        fn new(tag: &str) -> Self {
            use gaply_core::ai_engine::jobs::{self, ItemKind, NewItem};
            use gaply_core::rag::{self, RawDocument, SourceType};
            let s = Scratch::new(tag);
            let db = Database::open(&s.0.join("app/gaply.db")).unwrap();
            let now = now_epoch();
            db.create_manuscript(&format!("{SENTINEL}_title"), "", "")
                .unwrap();
            let gap = rag::ingest_document(
                &db,
                &gaply_core::embed::HashEmbedder,
                &RawDocument {
                    source_type: SourceType::ResearchPaper,
                    title: format!("[s/p1] {SENTINEL}_paper"),
                    source_url: "gapfinder://s/p1".into(),
                    fetched_at: now,
                    content: format!(
                        "{SENTINEL}. A paper about sampling and inference, long enough to chunk."
                    ),
                },
            )
            .unwrap()
            .document_id;
            jobs::create_job(
                &db,
                "thesis_audit",
                None,
                Some(&format!("/Users/{SENTINEL}/thesis.docx")),
                "v1",
                &[NewItem {
                    seq: 0,
                    kind: ItemKind::CitationNeed,
                    chunk_id: None,
                    page: None,
                    sentence: format!("{SENTINEL} sentence."),
                    payload_json: "{}".into(),
                }],
            )
            .unwrap();
            db.cache_put("report:v2:e1:1", &format!("{SENTINEL}_report"), 3600, now)
                .unwrap();
            db.cache_put("report:v2:e1:2", &format!("{SENTINEL}_old"), 1, now - 100)
                .unwrap();
            let write = |rel: &str| {
                let p = s.0.join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, SENTINEL).unwrap();
            };
            write(&format!("app/{COMPARISON_LOG_FILE}"));
            write(&format!("app/{OA_PAPERS_DIR}/{SENTINEL}.pdf"));
            write(&format!("app/{OA_PAPERS_DIR}/{SENTINEL}2.pdf"));
            write(&format!("tmp/gaply-aicheck-{SENTINEL}.html"));
            write(&format!("app/note-images/{SENTINEL}.png"));
            // An image changed an hour ago, so it counts as unused.
            let image = std::fs::File::options()
                .write(true)
                .open(s.0.join(format!("app/note-images/{SENTINEL}.png")))
                .unwrap();
            image
                .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600))
                .unwrap();
            Fixture { s, db, gap }
        }
        fn app(&self) -> PathBuf {
            self.s.0.join("app")
        }
        fn tmp(&self) -> PathBuf {
            self.s.0.join("tmp")
        }
        fn count(&self, c: StoredCategory) -> u64 {
            summary(&self.db, &self.app(), &self.tmp())
                .unwrap()
                .get(c)
                .items
        }
        fn prepare(&self, session: &StoredDataSession, c: StoredCategory) -> String {
            prepare_delete(&self.db, session, &self.app(), &self.tmp(), c.wire())
                .unwrap()
                .confirmation
        }
        fn delete(
            &self,
            session: &StoredDataSession,
            running: &RunningWork,
            jobs: &Jobs,
            c: StoredCategory,
            confirmation: Option<&str>,
        ) -> Result<DeletedView, GaplyError> {
            let (app, tmp) = (self.app(), self.tmp());
            let cx = DeleteContext {
                session,
                running,
                jobs,
                app_data_dir: &app,
                temp_dir: &tmp,
            };
            delete(&self.db, &cx, c.wire(), confirmation)
        }
    }

    /// The closed code a refusal carries on the wire.
    fn code_of(e: &GaplyError) -> String {
        let wire: serde_json::Value = serde_json::to_value(e).unwrap();
        let inner: serde_json::Value =
            serde_json::from_str(wire["message"].as_str().unwrap()).expect("a refusal's JSON");
        assert_eq!(
            inner.as_object().unwrap().len(),
            1,
            "the code and nothing else"
        );
        inner["code"].as_str().unwrap().to_string()
    }

    #[test]
    fn every_category_deletes_through_its_confirmation() {
        let f = Fixture::new("all-nine");
        let session = StoredDataSession::default();
        let running = RunningWork::default();
        for c in StoredCategory::ALL {
            let before = f.count(c);
            assert!(before > 0, "{c:?}: the fixture holds some");
            let confirmation = f.prepare(&session, c);
            let d = f
                .delete(&session, &running, &no_jobs(), c, Some(&confirmation))
                .unwrap();
            assert_eq!(d.category, c);
            assert_eq!(
                (d.before.items, d.deleted, d.after.items),
                (before, before, 0),
                "{c:?}"
            );
            assert_eq!((d.outcome, d.code), ("done", None), "{c:?}");
            assert_eq!(f.count(c), 0, "{c:?}");
        }
    }

    #[test]
    fn a_missing_used_replaced_expired_or_other_categorys_confirmation_deletes_nothing() {
        let f = Fixture::new("confirm");
        let clock = Arc::new(Moved::default());
        let session = StoredDataSession::with(clock.clone());
        let running = RunningWork::default();
        let jobs = no_jobs();
        let c = CachedReports;
        let refused = |given: Option<&str>, category: StoredCategory, want: StoredDataRefusal| {
            let before = f.count(category);
            let e = f
                .delete(&session, &running, &jobs, category, given)
                .unwrap_err();
            assert_eq!(code_of(&e), want.wire());
            assert_eq!(f.count(category), before, "{want:?} deleted something");
        };

        refused(None, c, StoredDataRefusal::ConfirmationMissing);
        refused(Some(""), c, StoredDataRefusal::ConfirmationMissing);
        refused(
            Some(&"0".repeat(64)),
            c,
            StoredDataRefusal::ConfirmationUnknown,
        );

        let other = f.prepare(&session, OrphanNoteImages);
        refused(
            Some(&other),
            c,
            StoredDataRefusal::ConfirmationOtherCategory,
        );
        // A mismatched one is used up too.
        refused(
            Some(&other),
            OrphanNoteImages,
            StoredDataRefusal::ConfirmationUsed,
        );
        assert_eq!(f.count(OrphanNoteImages), 1);

        let first = f.prepare(&session, c);
        let second = f.prepare(&session, c);
        assert_ne!(first, second, "random each time");
        refused(Some(&first), c, StoredDataRefusal::ConfirmationReplaced);

        let late = f.prepare(&session, c);
        clock
            .0
            .fetch_add(CONFIRMATION_LIFETIME_MS, Ordering::SeqCst);
        refused(Some(&late), c, StoredDataRefusal::ConfirmationExpired);

        let in_time = f.prepare(&session, c);
        clock
            .0
            .fetch_add(CONFIRMATION_LIFETIME_MS - 1, Ordering::SeqCst);
        let d = f
            .delete(&session, &running, &jobs, c, Some(&in_time))
            .unwrap();
        assert_eq!(d.deleted, 1, "1 ms short of five minutes");
        refused(Some(&in_time), c, StoredDataRefusal::ConfirmationUsed);

        let e = f.delete(&session, &running, &jobs, c, None);
        assert!(e.is_err());
        let e = prepare_delete(&f.db, &session, &f.app(), &f.tmp(), "manuscripts").unwrap_err();
        assert_eq!(code_of(&e), StoredDataRefusal::UnknownCategory.wire());
        let e = f
            .delete(&session, &running, &jobs, ManuscriptAnalyses, None)
            .unwrap_err();
        assert_eq!(code_of(&e), StoredDataRefusal::ConfirmationMissing.wire());
        assert_eq!(f.count(ManuscriptAnalyses), 1);
        let restarted = StoredDataSession::default();
        let kept = f.prepare(&session, ManuscriptAnalyses);
        let e = f
            .delete(&restarted, &running, &jobs, ManuscriptAnalyses, Some(&kept))
            .unwrap_err();
        assert_eq!(
            code_of(&e),
            StoredDataRefusal::ConfirmationUnknown.wire(),
            "held only in memory"
        );
    }

    #[test]
    fn a_stored_data_confirmation_leaves_a_pending_project_confirmation_good_and_the_reverse() {
        use crate::project_commands::ProjectSession;
        use gaply_core::research_project::ProjectId;
        let project = ProjectId::parse(&"a".repeat(32)).unwrap();

        // A project's first, then Stored data's: both still good.
        let projects = ProjectSession::default();
        let stored = StoredDataSession::default();
        let p = projects.issue(&project).unwrap();
        let s = stored.issue(CachedReports).unwrap();
        assert_eq!(projects.redeem(&project, Some(p.as_str())), Ok(()));
        assert_eq!(stored.redeem(CachedReports, Some(s.as_str())), Ok(()));

        // The reverse order.
        let projects = ProjectSession::default();
        let stored = StoredDataSession::default();
        let s = stored.issue(CachedReports).unwrap();
        let p = projects.issue(&project).unwrap();
        assert_eq!(stored.redeem(CachedReports, Some(s.as_str())), Ok(()));
        assert_eq!(projects.redeem(&project, Some(p.as_str())), Ok(()));

        // Neither book knows the other's.
        let projects = ProjectSession::default();
        let stored = StoredDataSession::default();
        let p = projects.issue(&project).unwrap();
        let s = stored.issue(CachedReports).unwrap();
        assert_eq!(
            stored.redeem(CachedReports, Some(p.as_str())),
            Err(StoredDataRefusal::ConfirmationUnknown)
        );
        assert_eq!(
            projects.redeem(&project, Some(s.as_str())),
            Err(gaply_core::research_project::ProjectCode::ConfirmationUnknown)
        );
    }

    #[test]
    fn each_refusal_of_decision_214_reaches_the_wire_as_its_code_and_deletes_nothing() {
        let f = Fixture::new("refusals");
        let session = StoredDataSession::default();
        let running = Arc::new(RunningWork::default());
        for (work, category, code) in [
            (
                Work::Analysis,
                ManuscriptAnalyses,
                StoredDataRefusal::AnalysisRunning,
            ),
            (
                Work::Analysis,
                CachedReports,
                StoredDataRefusal::AnalysisRunning,
            ),
            (
                Work::GapFinder,
                GapFinderPapers,
                StoredDataRefusal::GapFinderRunning,
            ),
            (
                Work::OpenAccessFetch,
                OpenAccessPapers,
                StoredDataRefusal::FetchRunning,
            ),
        ] {
            let before = f.count(category);
            let going = running.enter(work);
            let c = f.prepare(&session, category);
            let e = f
                .delete(&session, &running, &no_jobs(), category, Some(&c))
                .unwrap_err();
            assert_eq!(code_of(&e), code.wire(), "{category:?}");
            assert_eq!(f.count(category), before, "{category:?}");
            drop(going);
            let c = f.prepare(&session, category);
            let d = f
                .delete(&session, &running, &no_jobs(), category, Some(&c))
                .unwrap();
            assert_eq!(d.deleted, before, "{category:?}, once the work ended");
        }

        let jobs = no_jobs();
        lock(&jobs).insert(1, crate::ai::job_runner::JobControl::new(1));
        let c = f.prepare(&session, ThesisAudits);
        let e = f
            .delete(&session, &running, &jobs, ThesisAudits, Some(&c))
            .unwrap_err();
        assert_eq!(code_of(&e), StoredDataRefusal::JobRunning.wire());
        assert_eq!(f.count(ThesisAudits), 1);
        lock(&jobs).clear();
        let c = f.prepare(&session, ThesisAudits);
        assert_eq!(
            f.delete(&session, &running, &jobs, ThesisAudits, Some(&c))
                .unwrap()
                .deleted,
            1
        );
    }

    #[test]
    fn gap_finder_papers_are_refused_while_a_citation_checks_job_references_one() {
        use gaply_core::ai_engine::jobs;
        let f = Fixture::new("in-use");
        jobs::create_job(&f.db, "thesis_audit", Some(f.gap), None, "v1", &[]).unwrap();
        let session = StoredDataSession::default();
        let c = f.prepare(&session, GapFinderPapers);
        let e = f
            .delete(
                &session,
                &RunningWork::default(),
                &no_jobs(),
                GapFinderPapers,
                Some(&c),
            )
            .unwrap_err();
        assert_eq!(code_of(&e), StoredDataRefusal::PapersInUse.wire());
        assert_eq!(f.count(GapFinderPapers), 1);
    }

    #[test]
    fn the_views_hold_counts_closed_codes_and_values_only() {
        let f = Fixture::new("views");
        let session = StoredDataSession::default();
        for c in StoredCategory::ALL {
            let prepared = prepare_delete(&f.db, &session, &f.app(), &f.tmp(), c.wire()).unwrap();
            let p = serde_json::to_value(&prepared).unwrap();
            let mut keys: Vec<&str> = p.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(
                keys,
                ["bytes", "category", "confirmation", "expiresInMs", "items"]
            );
            let d = f
                .delete(
                    &session,
                    &RunningWork::default(),
                    &no_jobs(),
                    c,
                    Some(&prepared.confirmation),
                )
                .unwrap();
            let v = serde_json::to_value(&d).unwrap();
            let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(
                keys,
                ["after", "before", "category", "deleted", "outcome"],
                "{c:?}"
            );
            for json in [p.to_string(), v.to_string()] {
                for needle in [
                    SENTINEL,
                    "/Users",
                    "gapfinder",
                    "report:",
                    f.s.0.to_str().unwrap(),
                ] {
                    assert!(!json.contains(needle), "{needle} reached the view: {json}");
                }
            }
        }
    }

    #[test]
    fn a_partial_manuscripts_delete_answers_delete_partial_with_its_checkpoint() {
        // Decision 230: a batch after the first failed. The view says how
        // many were deleted, the new code, and the checkpoint's outcome in
        // its own field.
        use gaply_core::stored_data::{CategoryCount, CompactionCode, Deletion};
        let count = |items| CategoryCount {
            category: StoredCategory::ManuscriptAnalyses,
            items,
            bytes: items * 10,
        };
        let d = Deletion {
            category: StoredCategory::ManuscriptAnalyses,
            before: count(5),
            after: count(3),
            deleted: 2,
            removal_failed: false,
            partial: true,
        };
        for (compaction, checkpoint) in [
            (Compaction::Done, "done"),
            (
                Compaction::NotDone(CompactionCode::CheckpointBusy),
                "stored_data.checkpoint_busy",
            ),
            (
                Compaction::NotDone(CompactionCode::CheckpointFailed),
                "stored_data.checkpoint_failed",
            ),
        ] {
            let v = serde_json::to_value(view(d, Some(compaction))).unwrap();
            assert_eq!(v["outcome"], "not_done");
            assert_eq!(v["code"], DELETE_PARTIAL_WIRE);
            assert_eq!(v["checkpoint"], checkpoint);
            assert_eq!(v["deleted"], 2);
        }
        // Not partial: no checkpoint field, the outcome as before.
        let whole = Deletion {
            partial: false,
            deleted: 5,
            after: count(0),
            ..d
        };
        let v = serde_json::to_value(view(whole, Some(Compaction::Done))).unwrap();
        assert_eq!(v["outcome"], "done");
        assert!(v.get("checkpoint").is_none() && v.get("code").is_none());
    }

    /// The code as the webview reads it, spelled out here.
    const DELETE_PARTIAL_WIRE: &str = "stored_data.delete_partial";

    #[cfg(unix)]
    #[test]
    fn a_failed_removal_answers_not_done_with_how_many_were_removed() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new("not-removed");
        let session = StoredDataSession::default();
        let dir = f.app().join(OA_PAPERS_DIR);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let c = f.prepare(&session, OpenAccessPapers);
        let got = f.delete(
            &session,
            &RunningWork::default(),
            &no_jobs(),
            OpenAccessPapers,
            Some(&c),
        );
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let d = got.unwrap();
        assert_eq!((d.outcome, d.code), ("not_done", Some(REMOVE_FAILED)));
        assert_eq!((d.before.items, d.deleted, d.after.items), (2, 0, 2));
        assert_eq!(
            serde_json::to_value(&d).unwrap()["code"],
            serde_json::json!("stored_data.remove_failed")
        );
    }

    #[test]
    fn a_blocked_compaction_answers_not_done_and_the_rows_stay_deleted() {
        let f = Fixture::new("not-compacted");
        // Another connection's write transaction, held through the delete's
        // compaction: opened on the file directly, after the delete commits.
        let blocker = std::sync::Mutex::new(None::<rusqlite::Connection>);
        let running = RunningWork::default();
        let jobs = no_jobs();
        // The delete runs, then the blocker takes the write lock before the
        // compaction: the cache's own delete has committed by then.
        let d = {
            let db = &f.db;
            let path = f.app().join("gaply.db");
            let category = without_work(ExpiredCache, &running, &jobs, || {
                db.stored_data_delete(ExpiredCache, &f.app(), &f.tmp(), now_epoch())
            })
            .unwrap()
            .unwrap();
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "BEGIN IMMEDIATE; INSERT INTO cache (key, value, created_at, ttl_seconds) VALUES ('x', 'y', 1, 1);",
            )
            .unwrap();
            *lock(&blocker) = Some(conn);
            view(category, Some(db.stored_data_compact()))
        };
        assert_eq!(
            (d.outcome, d.code),
            ("not_done", Some("stored_data.checkpoint_busy"))
        );
        assert_eq!(d.deleted, 1);
        if let Some(conn) = lock(&blocker).take() {
            conn.execute_batch("ROLLBACK").unwrap();
        }
        assert_eq!(f.count(ExpiredCache), 0, "the rows stay deleted");
    }

    #[cfg(unix)]
    #[test]
    fn a_failure_answers_with_one_closed_code_and_no_path() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new("failure");
        let session = StoredDataSession::default();
        let c = f.prepare(&session, OpenAccessPapers);
        let dir = f.app().join(OA_PAPERS_DIR);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        let deleted = f.delete(
            &session,
            &RunningWork::default(),
            &no_jobs(),
            OpenAccessPapers,
            Some(&c),
        );
        let prepared = prepare_delete(&f.db, &session, &f.app(), &f.tmp(), OpenAccessPapers.wire());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        for e in [deleted.unwrap_err(), prepared.unwrap_err()] {
            let wire = serde_json::to_string(&e).unwrap();
            assert_eq!(
                wire,
                r#"{"code":"internal","message":"internal error: stored_data.unavailable"}"#
            );
        }
        assert_eq!(f.count(OpenAccessPapers), 2, "nothing removed");
    }

    #[test]
    fn every_refusal_has_its_own_code() {
        let mut codes: Vec<&str> = StoredDataRefusal::ALL.iter().map(|r| r.wire()).collect();
        let n = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), n);
        for c in &codes {
            assert!(c.starts_with("stored_data."), "{c}");
        }
        assert!(!codes.contains(&UNAVAILABLE) && !codes.contains(&REMOVE_FAILED));
    }
}
