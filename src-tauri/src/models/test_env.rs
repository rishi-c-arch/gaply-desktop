//! Test-only: the ONE lock for every app-crate test that changes a `GAPLY_*`
//! model variable, and a guard that puts each variable back when it drops.
//!
//! The environment is process-wide and the test harness runs tests on
//! parallel threads. Two locks used to cover these tests, one in
//! `aicheck::tests` and one in `models::mini_loader_tests`, so a test under
//! one could change a variable another test was reading under the other.
//! Measured 18 Oct 2026: with both modules in one test binary, the models
//! test removed `GAPLY_STAGE1_LM_GGUF` between an AI-Check flow test setting
//! it to a nonexistent path and the flow reading it, the resolver fell back to
//! the real Stage-1 model in `~/gaply-models`, and a test meant to be
//! heuristic-only was still running candle at a 120 s alarm (the sampled
//! stack was in `CandlePerplexityModel::surprisals`). The aicheck tests on
//! their own finish in 0.05 s.
//!
//! Users today: `models::mini_loader_tests`, `aicheck::tests` (the flow
//! tests) and `pipeline::tests::force_heuristic`. A new test that changes a
//! `GAPLY_*` model variable takes [`ModelEnv::lock`] and changes it through
//! the guard (add a `remove` beside `set` if one is needed); the restore then
//! runs even when the test fails.

use std::ffi::{OsStr, OsString};
use std::sync::{Mutex, MutexGuard};

static MODEL_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Holds [`MODEL_ENV_LOCK`] and remembers each variable's value from before
/// its first change. On drop it restores them, newest first, and only then
/// releases the lock (fields drop after `Drop::drop`), so no other test sees
/// a half-restored environment.
pub(crate) struct ModelEnv {
    saved: Vec<(&'static str, Option<OsString>)>,
    _lock: MutexGuard<'static, ()>,
}

impl ModelEnv {
    /// Takes the lock. A poisoned lock (a test failed while holding it) is
    /// taken anyway: its guard has already restored the environment, and one
    /// failure should not fail every later test.
    pub(crate) fn lock() -> Self {
        let lock = MODEL_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self {
            saved: Vec::new(),
            _lock: lock,
        }
    }

    pub(crate) fn set(&mut self, name: &'static str, value: impl AsRef<OsStr>) {
        self.remember(name);
        std::env::set_var(name, value);
    }

    fn remember(&mut self, name: &'static str) {
        if !self.saved.iter().any(|(n, _)| *n == name) {
            self.saved.push((name, std::env::var_os(name)));
        }
    }
}

impl Drop for ModelEnv {
    fn drop(&mut self) {
        for (name, value) in self.saved.drain(..).rev() {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
    }
}

/// Sets a variable for the rest of the process, under the same lock, and does
/// NOT restore it. Only for a value every test wants and none unsets
/// (`GAPLY_DISABLE_DEEP=1`, `pipeline::tests::force_heuristic`). Taking the
/// lock keeps the set out of another test's guarded window, so that guard
/// cannot restore over it: once set it reads `1` outside every window, and
/// each later window saves `1` and puts `1` back.
pub(crate) fn set_for_the_process(name: &'static str, value: &str) {
    let _lock = ModelEnv::lock();
    std::env::set_var(name, value);
}

#[cfg(test)]
mod tests {
    use super::ModelEnv;
    use std::ffi::OsString;

    /// One variable starts set and one absent; a thread changes both through
    /// the guard (one twice) and then returns or panics. Returns what is
    /// there afterwards. Each test passes its own names (no product code reads
    /// them), since this takes the lock three times and two callers could
    /// interleave.
    fn after_a_test_that(
        panics: bool,
        was_set: &'static str,
        was_unset: &'static str,
    ) -> (Option<OsString>, Option<OsString>) {
        {
            let _env = ModelEnv::lock();
            std::env::set_var(was_set, "before");
            std::env::remove_var(was_unset);
        }
        let joined = std::thread::spawn(move || {
            let mut env = ModelEnv::lock();
            env.set(was_set, "during");
            env.set(was_set, "during, again");
            env.set(was_unset, "during");
            assert!(!panics, "a failing test");
        })
        .join();
        assert_eq!(joined.is_err(), panics);
        let _env = ModelEnv::lock();
        let after = (std::env::var_os(was_set), std::env::var_os(was_unset));
        std::env::remove_var(was_set);
        after
    }

    #[test]
    fn the_guard_puts_back_what_was_there_when_the_test_passes() {
        let after = after_a_test_that(
            false,
            "GAPLY_TEST_GUARD_PASS_SET",
            "GAPLY_TEST_GUARD_PASS_UNSET",
        );
        assert_eq!(after, (Some("before".into()), None));
    }

    /// The panic also poisons the lock, so this checks that the next test
    /// still gets it.
    #[test]
    fn the_guard_puts_back_what_was_there_when_the_test_fails() {
        let after = after_a_test_that(
            true,
            "GAPLY_TEST_GUARD_FAIL_SET",
            "GAPLY_TEST_GUARD_FAIL_UNSET",
        );
        assert_eq!(after, (Some("before".into()), None));
    }
}
