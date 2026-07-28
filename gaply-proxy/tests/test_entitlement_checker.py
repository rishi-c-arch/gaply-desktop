"""Set 8 — the REAL entitlement checker: identity verification (ES256/JWKS),
the 401-vs-403 distinction, boot-time fail-safe guards, and a portable
concurrency proof for the atomic-consume statement shape.

The DB-backed steps (entitlement read, atomic consume against real Postgres)
live in test_entitlement_checker_db.py. Here we prove everything that does NOT
need Postgres, plus a SQLite structural proxy for the race.
"""

from __future__ import annotations

import asyncio
import time
import uuid

import jwt
import pytest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ec

from app.config import Settings
from app.entitlement import (
    REASON_TOKEN_EXPIRED,
    REASON_TOKEN_INVALID,
    REASON_TOKEN_MISSING,
    JwtError,
    JwtVerifier,
    SupabaseEntitlementChecker,
    TierLimits,
)

ISSUER = "https://proj.supabase.co/auth/v1"
AUD = "authenticated"


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


def _token(priv_pem, *, sub=None, exp_delta=3600, aud=AUD, iss=ISSUER):
    now = int(time.time())
    claims = {
        "sub": sub or str(uuid.uuid4()),
        "aud": aud,
        "iss": iss,
        "iat": now,
        "exp": now + exp_delta,
    }
    return jwt.encode(claims, priv_pem, algorithm="ES256")


# --- JwtVerifier (ES256, public-key injected — no network) -------------------

def test_jwt_valid_token_verifies():
    priv, pub = _keypair()
    v = JwtVerifier(issuer=ISSUER, audience=AUD, public_key=pub)
    sub = str(uuid.uuid4())
    claims = v.verify(_token(priv, sub=sub))
    assert claims["sub"] == sub


def test_jwt_missing_token():
    _, pub = _keypair()
    v = JwtVerifier(issuer=ISSUER, audience=AUD, public_key=pub)
    with pytest.raises(JwtError) as e:
        v.verify("")
    assert e.value.code == "missing"


def test_jwt_fake_token_wrong_key():
    priv_a, _ = _keypair()
    _, pub_b = _keypair()  # verify against a DIFFERENT key
    v = JwtVerifier(issuer=ISSUER, audience=AUD, public_key=pub_b)
    with pytest.raises(JwtError) as e:
        v.verify(_token(priv_a))
    assert e.value.code == "invalid"


def test_jwt_expired_token():
    priv, pub = _keypair()
    v = JwtVerifier(issuer=ISSUER, audience=AUD, public_key=pub)
    with pytest.raises(JwtError) as e:
        v.verify(_token(priv, exp_delta=-10))  # already expired
    assert e.value.code == "expired"


def test_jwt_wrong_audience_is_invalid():
    priv, pub = _keypair()
    v = JwtVerifier(issuer=ISSUER, audience=AUD, public_key=pub)
    with pytest.raises(JwtError) as e:
        v.verify(_token(priv, aud="other-service"))
    assert e.value.code == "invalid"


# --- checker.check(): identity failures return BEFORE any DB work ------------
# (dsn is never touched on these paths, so no Postgres is needed.)

def _checker(pub):
    return SupabaseEntitlementChecker(
        dsn="postgresql://unused",
        verifier=JwtVerifier(issuer=ISSUER, audience=AUD, public_key=pub),
        limits=TierLimits(free=0, premium=20),
    )


def test_check_no_token_is_token_missing():
    _, pub = _keypair()
    res = asyncio.run(_checker(pub).check(""))
    assert res.entitled is False and res.reason == REASON_TOKEN_MISSING


def test_check_fake_token_is_token_invalid():
    priv_a, _ = _keypair()
    _, pub_b = _keypair()
    res = asyncio.run(_checker(pub_b).check(_token(priv_a)))
    assert res.entitled is False and res.reason == REASON_TOKEN_INVALID


def test_check_expired_token_is_token_expired():
    priv, pub = _keypair()
    res = asyncio.run(_checker(pub).check(_token(priv, exp_delta=-10)))
    assert res.entitled is False and res.reason == REASON_TOKEN_EXPIRED


# --- boot-time fail-safe guards (step 4) -------------------------------------

def _public_settings(**over):
    base = dict(
        app_check_signing_key="k",
        bind_host="0.0.0.0",
        allow_public_bind=True,  # public exposure accepted -> guards must fire
        entitlement_required=True,
    )
    base.update(over)
    return Settings(**base)


def test_boot_refuses_public_without_entitlement_required():
    from app.main import create_app

    with pytest.raises(RuntimeError, match="ENTITLEMENT_REQUIRED"):
        create_app(_public_settings(entitlement_required=False), entitlement_checker=object())


def test_boot_refuses_public_without_checker():
    from app.main import create_app

    # entitlement on, public, but no checker and no SUPABASE_URL/DATABASE_URL to
    # build one -> must refuse.
    with pytest.raises(RuntimeError, match="EntitlementChecker"):
        create_app(_public_settings(), entitlement_checker=None)


def test_boot_refuses_public_with_empty_signing_key():
    from app.main import create_app

    with pytest.raises(RuntimeError, match="APP_CHECK_SIGNING_KEY"):
        create_app(_public_settings(app_check_signing_key=""), entitlement_checker=object())


def test_boot_refuses_public_with_debug_tokens():
    from app.main import create_app

    with pytest.raises(RuntimeError, match="DEBUG"):
        create_app(
            _public_settings(app_check_debug_tokens=("DEBUG-x",)),
            entitlement_checker=object(),
        )


def test_boot_allows_public_when_fully_wired():
    from app.main import create_app

    app = create_app(_public_settings(), entitlement_checker=object())
    assert app.state.entitlement_checker is not None


def test_boot_private_bind_is_exempt_from_guards():
    # Loopback (not public) with entitlement on but no checker: must still BUILD
    # (request-time 503 remains the fail-closed), never a boot failure.
    from app.main import create_app

    app = create_app(
        Settings(app_check_signing_key="k", entitlement_required=True),
        entitlement_checker=None,
    )
    assert app.state.entitlement_checker is None


# --- SQLite STRUCTURAL PROXY for the atomic-consume race ---------------------

def test_concurrent_consume_sqlite_structural(tmp_path):
    """STRUCTURAL PROXY — NOT the literal Postgres path.

    Proves the atomic conditional-UPSERT STATEMENT SHAPE prevents double-consume
    under REAL thread concurrency, using SQLite (portable, runs anywhere). The
    production checker runs the IDENTICAL statement against Postgres — the real
    Postgres integration race test is `test_concurrent_consume_real_postgres`
    in test_entitlement_checker_db.py. Do not mistake this for having verified
    against real Postgres.
    """
    import sqlite3
    import threading

    db = str(tmp_path / "race.db")
    con = sqlite3.connect(db)
    con.execute("PRAGMA journal_mode=WAL")
    con.execute(
        "CREATE TABLE usage_counters (user_id TEXT, feature TEXT, "
        "count INT NOT NULL DEFAULT 0, period_start TEXT, "
        "PRIMARY KEY (user_id, feature, period_start))"
    )
    # A SINGLE remaining use: limit = 1, existing count = 0.
    con.execute(
        "INSERT INTO usage_counters VALUES (?,?,?,?)",
        ("u1", "publishready", 0, "2026-07-01"),
    )
    con.commit()
    con.close()

    LIMIT = 1
    sql = (
        "INSERT INTO usage_counters (user_id, feature, count, period_start) "
        "VALUES (?, ?, 1, ?) "
        "ON CONFLICT (user_id, feature, period_start) "
        "DO UPDATE SET count = count + 1 WHERE count < ? RETURNING count"
    )
    results: list = []
    barrier = threading.Barrier(2)

    def worker():
        c = sqlite3.connect(db, timeout=5)
        c.execute("PRAGMA busy_timeout=5000")
        barrier.wait()  # maximize contention: both fire at once
        row = c.execute(sql, ("u1", "publishready", "2026-07-01", LIMIT)).fetchone()
        c.commit()
        c.close()
        results.append(row)

    threads = [threading.Thread(target=worker) for _ in range(2)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    winners = [r for r in results if r is not None]
    assert len(winners) == 1, f"exactly one consume must win; got {results}"

    con = sqlite3.connect(db)
    final = con.execute(
        "SELECT count FROM usage_counters WHERE user_id='u1'"
    ).fetchone()[0]
    con.close()
    assert final == LIMIT, "count must not exceed the limit (no double-consume)"
