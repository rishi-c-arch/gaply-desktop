"""Set 8 — the entitlement checker against REAL Postgres.

These are the literal-path integration tests: entitlement reads and the atomic
consume (incl. the concurrent-consume RACE) run against an actual Postgres via
asyncpg. If ``DATABASE_URL`` is set we use it; otherwise we spin up a throwaway
local Postgres cluster (initdb/pg_ctl). If neither is available the module
SKIPS — the portable structural proxy in test_entitlement_checker.py still runs.
"""

from __future__ import annotations

import asyncio
import os
import shutil
import socket
import subprocess
import time
import uuid
from datetime import date, datetime, timezone

import jwt
import pytest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ec

from app.entitlement import (
    CONSUME_SQL,
    REASON_NO_PLAN,
    REASON_NO_USES,
    JwtVerifier,
    SupabaseEntitlementChecker,
    TierLimits,
)

ISSUER = "https://proj.supabase.co/auth/v1"
AUD = "authenticated"
FEATURE = "publishready"
FIXED_NOW = datetime(2026, 7, 15, tzinfo=timezone.utc)
PERIOD = date(2026, 7, 1)


# --- test JWT helpers (local EC key; no network) -----------------------------

def _keypair():
    priv = ec.generate_private_key(ec.SECP256R1())
    priv_pem = priv.private_bytes(
        serialization.Encoding.PEM,
        serialization.PrivateFormat.PKCS8,
        serialization.NoEncryption(),
    )
    pub_pem = priv.public_key().public_bytes(
        serialization.Encoding.PEM,
        serialization.PublicFormat.SubjectPublicKeyInfo,
    )
    return priv_pem, pub_pem


PRIV_PEM, PUB_PEM = _keypair()


def _token(sub, exp_delta=3600):
    now = int(time.time())
    return jwt.encode(
        {"sub": sub, "aud": AUD, "iss": ISSUER, "iat": now, "exp": now + exp_delta},
        PRIV_PEM,
        algorithm="ES256",
    )


def _checker(dsn, *, free=0, premium=3):
    return SupabaseEntitlementChecker(
        dsn=dsn,
        verifier=JwtVerifier(issuer=ISSUER, audience=AUD, public_key=PUB_PEM),
        limits=TierLimits(free=free, premium=premium),
        feature=FEATURE,
        now_fn=lambda: FIXED_NOW,
    )


def _free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def _bin(name):
    return shutil.which(name) or (
        f"/opt/homebrew/bin/{name}" if os.path.exists(f"/opt/homebrew/bin/{name}") else None
    )


SCHEMA = [
    """CREATE TABLE subscriptions (
        user_id uuid primary key,
        tier text not null default 'free',
        status text not null default 'active',
        current_period_end timestamptz
    )""",
    """CREATE TABLE usage_counters (
        user_id uuid not null,
        feature text not null,
        count int not null default 0,
        period_start date not null,
        primary key (user_id, feature, period_start)
    )""",
]


@pytest.fixture(scope="module")
def pg_dsn(tmp_path_factory):
    env_url = os.environ.get("DATABASE_URL")
    if env_url:
        _apply_schema_via_asyncpg(env_url)
        yield env_url
        return

    initdb, pg_ctl, psql = _bin("initdb"), _bin("pg_ctl"), _bin("psql")
    if not (initdb and pg_ctl and psql):
        pytest.skip("no DATABASE_URL and no local postgres (initdb/pg_ctl/psql) available")

    datadir = str(tmp_path_factory.mktemp("pgdata"))
    subprocess.run(
        [initdb, "-D", datadir, "-U", "postgres", "--auth=trust", "-E", "UTF8"],
        check=True,
        capture_output=True,
    )
    port = _free_port()
    logfile = os.path.join(datadir, "server.log")
    subprocess.run(
        [pg_ctl, "-D", datadir, "-o", f"-p {port} -c listen_addresses=127.0.0.1", "-l", logfile, "-w", "start"],
        check=True,
        capture_output=True,
    )
    dsn = f"postgresql://postgres@127.0.0.1:{port}/postgres"
    try:
        for stmt in SCHEMA:
            subprocess.run(
                [psql, "-h", "127.0.0.1", "-p", str(port), "-U", "postgres", "-d", "postgres",
                 "-v", "ON_ERROR_STOP=1", "-c", stmt],
                check=True,
                capture_output=True,
            )
        yield dsn
    finally:
        subprocess.run([pg_ctl, "-D", datadir, "-m", "immediate", "-w", "stop"], capture_output=True)


def _apply_schema_via_asyncpg(dsn):
    async def go():
        import asyncpg

        conn = await asyncpg.connect(dsn)
        try:
            for stmt in SCHEMA:
                await conn.execute(stmt.replace("CREATE TABLE", "CREATE TABLE IF NOT EXISTS"))
        finally:
            await conn.close()

    asyncio.run(go())


async def _seed(dsn, user_id, *, tier, status="active", period_end=None, count=None):
    import asyncpg

    conn = await asyncpg.connect(dsn)
    try:
        await conn.execute(
            "INSERT INTO subscriptions(user_id,tier,status,current_period_end) "
            "VALUES($1,$2,$3,$4) ON CONFLICT (user_id) DO UPDATE "
            "SET tier=$2,status=$3,current_period_end=$4",
            user_id, tier, status, period_end,
        )
        if count is not None:
            await conn.execute(
                "INSERT INTO usage_counters(user_id,feature,count,period_start) "
                "VALUES($1,$2,$3,$4) ON CONFLICT (user_id,feature,period_start) "
                "DO UPDATE SET count=$3",
                user_id, FEATURE, count, PERIOD,
            )
    finally:
        await conn.close()


async def _count(dsn, user_id):
    import asyncpg

    conn = await asyncpg.connect(dsn)
    try:
        return await conn.fetchval(
            "SELECT count FROM usage_counters WHERE user_id=$1 AND feature=$2 AND period_start=$3",
            user_id, FEATURE, PERIOD,
        )
    finally:
        await conn.close()


# --- 1. valid token, but not entitled ----------------------------------------

def test_free_user_no_plan_is_403_reason(pg_dsn):
    uid = uuid.uuid4()

    async def scenario():
        await _seed(pg_dsn, uid, tier="free", count=0)
        checker = _checker(pg_dsn)  # free limit = 0
        try:
            res = await checker.check(_token(str(uid)))
            assert res.entitled is False
            assert res.reason == REASON_NO_PLAN
        finally:
            await checker.aclose()

    asyncio.run(scenario())


def test_premium_user_zero_uses_remaining(pg_dsn):
    uid = uuid.uuid4()

    async def scenario():
        # premium, but already at the limit (count == premium limit)
        await _seed(pg_dsn, uid, tier="premium", count=3)
        checker = _checker(pg_dsn, premium=3)
        try:
            res = await checker.check(_token(str(uid)))
            assert res.entitled is False
            assert res.reason == REASON_NO_USES
        finally:
            await checker.aclose()

    asyncio.run(scenario())


# --- 2. valid token, entitled, consumed EXACTLY once --------------------------

def test_premium_user_with_uses_consumes_exactly_once(pg_dsn):
    uid = uuid.uuid4()

    async def scenario():
        await _seed(pg_dsn, uid, tier="premium", count=0)
        checker = _checker(pg_dsn, premium=3)
        try:
            token = _token(str(uid))
            res = await checker.check(token)
            assert res.entitled is True and res.remaining == 2  # 3 - 0 - 1
            await checker.consume(token)
            assert await _count(pg_dsn, uid) == 1, "exactly one use consumed"
            # a second check still shows entitlement with one fewer use
            res2 = await checker.check(token)
            assert res2.entitled is True and res2.remaining == 1
        finally:
            await checker.aclose()

    asyncio.run(scenario())


# --- 3. the REAL Postgres concurrent-consume race -----------------------------

def test_concurrent_consume_real_postgres(pg_dsn):
    """Two concurrent consumes against a SINGLE remaining use, executed against
    real Postgres via two separate pooled connections. Exactly one may win; the
    counter must never exceed the limit."""
    uid = uuid.uuid4()
    LIMIT = 1

    async def scenario():
        import asyncpg

        # single remaining use: limit 1, existing count 0
        await _seed(pg_dsn, uid, tier="premium", count=0)
        pool = await asyncpg.create_pool(pg_dsn, min_size=2, max_size=4)
        try:
            async def one_consume():
                async with pool.acquire() as conn:
                    row = await conn.fetchrow(CONSUME_SQL, uid, FEATURE, PERIOD, LIMIT)
                    return row is not None

            # fire both concurrently on the same loop, real DB serializes them
            wins = await asyncio.gather(one_consume(), one_consume())
            assert sum(1 for w in wins if w) == 1, f"exactly one consume must win; got {wins}"

            final = await pool.fetchval(
                "SELECT count FROM usage_counters WHERE user_id=$1 AND feature=$2 AND period_start=$3",
                uid, FEATURE, PERIOD,
            )
            assert final == LIMIT, "count must not exceed the limit (no double-consume)"
        finally:
            await pool.close()

    asyncio.run(scenario())
