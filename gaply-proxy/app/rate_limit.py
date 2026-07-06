"""Token-bucket rate limiter (Python port of gaply_core ``ratelimit.rs``).

Per-client buckets keyed by an arbitrary string id, thread-safe via a lock,
with opportunistic cleanup of idle buckets. Time is injectable (``now``) so the
refill/cleanup math is deterministically testable.
"""

from __future__ import annotations

import time
from dataclasses import dataclass
from threading import Lock


@dataclass
class RateLimitResult:
    allowed: bool
    retry_after: float  # seconds until one token refills; 0.0 when allowed
    remaining: float


class TokenBucketRateLimiter:
    def __init__(self, capacity: float, refill_rate: float, idle_window: float = 600.0) -> None:
        assert capacity > 0, "capacity must be > 0"
        assert refill_rate > 0, "refill_rate must be > 0"
        self.capacity = float(capacity)
        self.refill_rate = float(refill_rate)
        self.idle_window = float(idle_window)
        # client_id -> (tokens, last_refill, last_active)
        self._buckets: dict[str, tuple[float, float, float]] = {}
        self._lock = Lock()
        self._last_sweep: float | None = None

    def try_consume(self, client_id: str, now: float | None = None) -> RateLimitResult:
        if now is None:
            now = time.monotonic()
        with self._lock:
            tokens, last_refill, _ = self._buckets.get(client_id, (self.capacity, now, now))
            elapsed = max(0.0, now - last_refill)
            tokens = min(self.capacity, tokens + elapsed * self.refill_rate)

            if tokens >= 1.0:
                tokens -= 1.0
                result = RateLimitResult(True, 0.0, tokens)
            else:
                result = RateLimitResult(False, (1.0 - tokens) / self.refill_rate, tokens)

            self._buckets[client_id] = (tokens, now, now)

            # opportunistic sweep so the map stays bounded over a long session
            if self._last_sweep is None:
                self._last_sweep = now
            if now - self._last_sweep >= self.idle_window:
                self._buckets = {
                    k: v for k, v in self._buckets.items() if now - v[2] <= self.idle_window
                }
                self._last_sweep = now

            return result

    def sweep_idle(self, now: float | None = None) -> int:
        if now is None:
            now = time.monotonic()
        with self._lock:
            before = len(self._buckets)
            self._buckets = {
                k: v for k, v in self._buckets.items() if now - v[2] <= self.idle_window
            }
            self._last_sweep = now
            return before - len(self._buckets)

    def bucket_count(self) -> int:
        with self._lock:
            return len(self._buckets)
