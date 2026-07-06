from app.rate_limit import TokenBucketRateLimiter


def test_burst_then_reject():
    rl = TokenBucketRateLimiter(capacity=5, refill_rate=1)
    allowed = sum(rl.try_consume("c", now=0.0).allowed for _ in range(8))
    assert allowed == 5


def test_refill_over_time():
    rl = TokenBucketRateLimiter(capacity=2, refill_rate=2)  # 2 tokens/sec
    assert rl.try_consume("c", now=0.0).allowed
    assert rl.try_consume("c", now=0.0).allowed
    r = rl.try_consume("c", now=0.0)
    assert not r.allowed
    assert abs(r.retry_after - 0.5) < 1e-9  # 1 token at 2/sec
    # after 1s, 2 tokens refilled
    assert rl.try_consume("c", now=1.0).allowed


def test_capacity_cap():
    rl = TokenBucketRateLimiter(capacity=3, refill_rate=1)
    rl.try_consume("c", now=0.0)  # drain 1 -> 2 left
    # wait a long time: refill caps at capacity (3), not 100
    for _ in range(3):
        assert rl.try_consume("c", now=100.0).allowed
    assert not rl.try_consume("c", now=100.0).allowed


def test_independent_clients():
    rl = TokenBucketRateLimiter(capacity=1, refill_rate=1)
    assert rl.try_consume("a", now=0.0).allowed
    assert rl.try_consume("b", now=0.0).allowed
    assert not rl.try_consume("a", now=0.0).allowed


def test_idle_bucket_cleanup():
    rl = TokenBucketRateLimiter(capacity=5, refill_rate=1, idle_window=10.0)
    rl.try_consume("A", now=0.0)
    rl.try_consume("B", now=0.0)
    assert rl.bucket_count() == 2
    rl.try_consume("A", now=8.0)  # A stays active
    removed = rl.sweep_idle(now=15.0)  # B idle 15 > 10; A idle 7 kept
    assert removed == 1
    assert rl.bucket_count() == 1
