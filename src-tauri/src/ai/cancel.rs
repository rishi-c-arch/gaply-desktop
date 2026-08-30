//! Per-run cancellation for the generative engine.
//!
//! # Why this is not one shared `AtomicBool`
//!
//! It used to be. Every generative command cloned `AppState::ai_gen_cancel` and
//! opened with `cancel.store(false)` — "never poison the next run". Under the
//! single-inference invariant that is a real hazard: a request that arrives
//! while another is queued or running does not just prepare itself, it
//! **un-cancels everything else**. Cancel, then retry, and the retry's opening
//! `store(false)` resurrects the run the user had just stopped — which then
//! holds the one inference permit while the new request waits behind it, with
//! the UI showing nothing but "Checking on this machine".
//!
//! So cancellation is per-RUN. A request takes a token; the token owns its own
//! flag and nobody else can clear it. `cancel_all` stops exactly the runs that
//! exist at that moment, which is the honest meaning of a Cancel button on an
//! engine that runs one generation at a time. A finished run drops its token
//! and leaves the registry, so a later cancel cannot reach back and poison it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The cancellation flag for ONE generation request.
///
/// Held for the life of the request; deregisters on drop, including on an early
/// `?`. Clone the inner `Arc<AtomicBool>` with [`CancelToken::flag`] to hand to
/// `run_task` / `ModelManager::generate`.
pub struct CancelToken {
    flag: Arc<AtomicBool>,
    registry: Arc<CancelRegistry>,
}

impl CancelToken {
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl Drop for CancelToken {
    fn drop(&mut self) {
        self.registry.forget(&self.flag);
    }
}

/// Every generation request currently alive.
#[derive(Default)]
pub struct CancelRegistry {
    live: Mutex<Vec<Arc<AtomicBool>>>,
}

impl CancelRegistry {
    /// A fresh, un-cancelled token, registered until it drops.
    pub fn issue(self: &Arc<Self>) -> CancelToken {
        let flag = Arc::new(AtomicBool::new(false));
        self.lock().push(flag.clone());
        CancelToken { flag, registry: self.clone() }
    }

    /// Cancel every run alive right now. Returns how many were signalled, which
    /// is what lets a caller say "stopped 2 checks" rather than guess.
    ///
    /// Runs that start AFTER this call are untouched — that is the point: the
    /// user's next attempt must not inherit a cancellation aimed at the last one.
    pub fn cancel_all(&self) -> usize {
        let live = self.lock();
        for f in live.iter() {
            f.store(true, Ordering::SeqCst);
        }
        live.len()
    }

    /// How many requests are alive. Used for the queue depth a UI reports.
    pub fn live(&self) -> usize {
        self.lock().len()
    }

    fn forget(&self, flag: &Arc<AtomicBool>) {
        self.lock().retain(|f| !Arc::ptr_eq(f, flag));
    }

    /// A panicking holder must not disable cancellation for the whole session,
    /// which is what unwrapping a poisoned lock would do.
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Arc<AtomicBool>>> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_starts_uncancelled_and_deregisters_on_drop() {
        let reg = Arc::new(CancelRegistry::default());
        {
            let t = reg.issue();
            assert!(!t.is_cancelled());
            assert_eq!(reg.live(), 1);
        }
        assert_eq!(reg.live(), 0, "a finished run must leave the registry");
    }

    #[test]
    fn cancel_all_stops_every_live_run_and_reports_how_many() {
        let reg = Arc::new(CancelRegistry::default());
        let a = reg.issue();
        let b = reg.issue();
        assert_eq!(reg.cancel_all(), 2);
        assert!(a.is_cancelled() && b.is_cancelled());
    }

    /// The regression this type exists for: cancel, then retry. The retry must
    /// run, and it must NOT revive the cancelled run.
    #[test]
    fn a_new_run_neither_inherits_nor_clears_an_earlier_cancellation() {
        let reg = Arc::new(CancelRegistry::default());
        let stopped = reg.issue();
        reg.cancel_all();
        assert!(stopped.is_cancelled());

        let retry = reg.issue();
        assert!(!retry.is_cancelled(), "the retry must not inherit the cancellation");
        assert!(stopped.is_cancelled(), "the retry must not resurrect the cancelled run");
    }

    #[test]
    fn cancelling_with_nothing_running_is_a_no_op_that_cannot_poison_the_next_run() {
        let reg = Arc::new(CancelRegistry::default());
        assert_eq!(reg.cancel_all(), 0);
        assert!(!reg.issue().is_cancelled());
    }
}
