//! Thread-safe token-bucket rate limiter.
//!
//! Each client (keyed by an arbitrary string id) gets a bucket with a
//! `capacity` (max burst) and a `refill_rate` (tokens added per second =
//! sustained allowed throughput). Each request tries to consume one token;
//! the result says whether it was allowed and, if not, how long until enough
//! tokens refill.
//!
//! Standalone utility with **no new dependencies** — concurrency is via
//! `std::sync::Mutex`, keeping the whole per-client operation atomic so
//! concurrent callers can never double-count. Inactive buckets are swept
//! (opportunistically on access, or on demand via [`RateLimiter::sweep_idle`])
//! so the map stays bounded over a long-running session.
//!
//! Intended for the future proxy's per-user throttling and for budgeting any
//! local external-API calls.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimitResult {
    /// Whether a token was consumed.
    pub allowed: bool,
    /// When rejected, how long until one token refills; `ZERO` when allowed.
    pub retry_after: Duration,
    /// Tokens left in the bucket after this call.
    pub remaining: f64,
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
    last_active: Instant,
}

impl Bucket {
    fn new_full(capacity: f64, now: Instant) -> Self {
        Self { tokens: capacity, last_refill: now, last_active: now }
    }

    /// Add tokens for elapsed time since the last refill, capped at capacity.
    fn refill(&mut self, now: Instant, capacity: f64, rate: f64) {
        let elapsed = now.saturating_duration_since(self.last_refill).as_secs_f64();
        if elapsed > 0.0 {
            self.tokens = (self.tokens + elapsed * rate).min(capacity);
            self.last_refill = now;
        }
    }
}

#[derive(Debug)]
struct Inner {
    buckets: HashMap<String, Bucket>,
    last_sweep: Instant,
}

#[derive(Debug)]
pub struct RateLimiter {
    capacity: f64,
    refill_rate: f64,
    idle_window: Duration,
    sweep_interval: Duration,
    inner: Mutex<Inner>,
}

impl RateLimiter {
    /// `capacity` = max burst size (tokens); `refill_rate` = tokens per second
    /// of sustained throughput. Both must be positive.
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        assert!(capacity > 0.0, "capacity must be > 0");
        assert!(refill_rate > 0.0, "refill_rate must be > 0");
        let idle = Duration::from_secs(600);
        Self {
            capacity,
            refill_rate,
            idle_window: idle,
            sweep_interval: idle,
            inner: Mutex::new(Inner { buckets: HashMap::new(), last_sweep: Instant::now() }),
        }
    }

    /// Set how long a client may be idle before its bucket is eligible for
    /// cleanup. Also becomes the opportunistic auto-sweep interval.
    pub fn with_idle_window(mut self, window: Duration) -> Self {
        self.idle_window = window;
        self.sweep_interval = window;
        self
    }

    /// Attempt to consume one token for `client_id`.
    pub fn try_consume(&self, client_id: &str) -> RateLimitResult {
        self.try_consume_at(client_id, Instant::now())
    }

    /// Remove buckets idle longer than the configured window. Returns the
    /// number removed. Call periodically from a long-running host if you want
    /// cleanup independent of request traffic.
    pub fn sweep_idle(&self) -> usize {
        self.sweep_idle_at(Instant::now())
    }

    /// Number of live buckets (introspection / tests).
    pub fn bucket_count(&self) -> usize {
        self.inner.lock().unwrap().buckets.len()
    }

    fn try_consume_at(&self, client_id: &str, now: Instant) -> RateLimitResult {
        let mut inner = self.inner.lock().unwrap();

        let result = {
            let bucket = inner
                .buckets
                .entry(client_id.to_string())
                .or_insert_with(|| Bucket::new_full(self.capacity, now));
            bucket.refill(now, self.capacity, self.refill_rate);
            bucket.last_active = now;

            if bucket.tokens >= 1.0 {
                bucket.tokens -= 1.0;
                RateLimitResult { allowed: true, retry_after: Duration::ZERO, remaining: bucket.tokens }
            } else {
                let needed = 1.0 - bucket.tokens;
                RateLimitResult {
                    allowed: false,
                    retry_after: Duration::from_secs_f64(needed / self.refill_rate),
                    remaining: bucket.tokens,
                }
            }
        };

        // Opportunistic cleanup so the map stays bounded without a caller
        // remembering to sweep. The just-touched client can't be evicted
        // (its last_active == now).
        if now.saturating_duration_since(inner.last_sweep) >= self.sweep_interval {
            let window = self.idle_window;
            inner.buckets.retain(|_, b| now.saturating_duration_since(b.last_active) <= window);
            inner.last_sweep = now;
        }

        result
    }

    fn sweep_idle_at(&self, now: Instant) -> usize {
        let mut inner = self.inner.lock().unwrap();
        let before = inner.buckets.len();
        let window = self.idle_window;
        inner.buckets.retain(|_, b| now.saturating_duration_since(b.last_active) <= window);
        inner.last_sweep = now;
        before - inner.buckets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn refill_accumulates_over_time_capped_at_capacity() {
        let rl = RateLimiter::new(10.0, 2.0); // 10 burst, 2 tokens/sec
        let t0 = Instant::now();

        // drain all 10
        for _ in 0..10 {
            assert!(rl.try_consume_at("a", t0).allowed);
        }
        // 11th rejected; needs 0.5s for 1 token at 2/sec
        let r = rl.try_consume_at("a", t0);
        assert!(!r.allowed);
        assert!((r.retry_after.as_secs_f64() - 0.5).abs() < 1e-9, "retry {:?}", r.retry_after);

        // after 1s → exactly 2 tokens accrued
        let r = rl.try_consume_at("a", t0 + Duration::from_secs(1));
        assert!(r.allowed);
        assert!((r.remaining - 1.0).abs() < 1e-9, "remaining {}", r.remaining);

        // after a long wait, tokens are capped at capacity (10), not 2*100
        let t = t0 + Duration::from_secs(1) + Duration::from_secs(100);
        for _ in 0..10 {
            assert!(rl.try_consume_at("a", t).allowed);
        }
        assert!(!rl.try_consume_at("a", t).allowed, "capacity cap not enforced");
    }

    #[test]
    fn burst_succeeds_up_to_capacity_then_rejects() {
        let rl = RateLimiter::new(5.0, 1.0);
        let t = Instant::now();
        let allowed = (0..8).filter(|_| rl.try_consume_at("c", t).allowed).count();
        assert_eq!(allowed, 5, "burst should allow exactly capacity");
    }

    #[test]
    fn requests_succeed_again_after_refill() {
        let rl = RateLimiter::new(1.0, 1.0);
        let t = Instant::now();
        assert!(rl.try_consume_at("k", t).allowed);
        assert!(!rl.try_consume_at("k", t).allowed);
        // one second later a token has refilled
        assert!(rl.try_consume_at("k", t + Duration::from_secs(1)).allowed);
    }

    #[test]
    fn clients_have_independent_buckets() {
        let rl = RateLimiter::new(1.0, 1.0);
        let t = Instant::now();
        assert!(rl.try_consume_at("x", t).allowed);
        assert!(rl.try_consume_at("y", t).allowed);
        assert!(!rl.try_consume_at("x", t).allowed);
    }

    #[test]
    fn inactive_buckets_are_removed_after_window() {
        let rl = RateLimiter::new(5.0, 1.0).with_idle_window(Duration::from_secs(10));
        let t0 = Instant::now();
        rl.try_consume_at("A", t0);
        rl.try_consume_at("B", t0);
        assert_eq!(rl.bucket_count(), 2);

        // A stays active; B goes quiet
        rl.try_consume_at("A", t0 + Duration::from_secs(8));

        // sweep at t0+15s: B idle 15s (> 10) removed, A idle 7s (<= 10) kept
        let removed = rl.sweep_idle_at(t0 + Duration::from_secs(15));
        assert_eq!(removed, 1);
        assert_eq!(rl.bucket_count(), 1);
    }

    #[test]
    fn opportunistic_sweep_bounds_the_map() {
        let rl = RateLimiter::new(1.0, 1.0).with_idle_window(Duration::from_secs(1));
        let t0 = Instant::now();
        for i in 0..50 {
            rl.try_consume_at(&format!("c{i}"), t0);
        }
        assert_eq!(rl.bucket_count(), 50);

        // a request well past the sweep interval evicts all idle buckets,
        // leaving only the newcomer — no manual sweep needed
        rl.try_consume_at("newcomer", t0 + Duration::from_secs(10));
        assert_eq!(rl.bucket_count(), 1);
    }

    #[test]
    fn concurrent_access_never_double_counts() {
        // negligible refill so the only tokens available are the initial 200
        let rl = Arc::new(RateLimiter::new(200.0, 1e-9));
        let allowed = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let rl = Arc::clone(&rl);
                let allowed = Arc::clone(&allowed);
                thread::spawn(move || {
                    for _ in 0..100 {
                        if rl.try_consume("shared").allowed {
                            allowed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        // 800 attempts, capacity 200: exactly 200 allowed — no double-count
        // (would exceed 200) and no lost increments (would fall short)
        assert_eq!(allowed.load(Ordering::Relaxed), 200);
    }
}
