"""Server-side entitlement gate — THE REAL GATE for paid work (Set 8).

The client's locked/upgrade UI is PRESENTATION ONLY; a tampered client can
skip it. This module is where a non-entitled request actually dies: the proxy
verifies the caller's identity AND entitlement BEFORE the paid model call, and
CONSUMING a use happens here (server-side, after a successful call) — never as
a client-side number.

Four steps (see ``SupabaseEntitlementChecker``):
  1. VERIFY IDENTITY — the Supabase user JWT (``X-Gaply-User-Token``) is ES256,
     verified against the project's public JWKS (signature + exp + aud + iss).
     No shared secret lives on the server. Invalid/expired -> not entitled with
     an identity reason (the HTTP layer maps those to 401).
  2. CHECK ENTITLEMENT — read ``subscriptions`` (active premium?) and
     ``usage_counters`` (uses left this period?) via the service-privileged DB
     connection. Not entitled -> 403 with a precise reason.
  3. CONSUME ATOMICALLY — a single conditional UPSERT (``count = count + 1
     WHERE count < limit RETURNING count``). Two concurrent requests on a single
     remaining use CANNOT both get a row — proven by a real race test.
  4. FAIL SAFELY — boot-time guards in ``app.main`` refuse to start a publicly
     exposed proxy unless the gate is fully wired.

Pricing-agnostic: nothing here knows a price. The only question asked is "does
this user have an available use / active plan?" — limits are env config.
"""

from __future__ import annotations

import asyncio
import logging
import uuid
from dataclasses import dataclass
from datetime import date, datetime, timezone
from typing import Protocol

logger = logging.getLogger("gaply.entitlement")

# The signed-in user's Supabase JWT, attached by the Rust client. This is the
# USER'S OWN credential (never an app secret) — the server verifies it.
USER_TOKEN_HEADER = "X-Gaply-User-Token"

# Denial reasons. Identity reasons map to HTTP 401 (who are you?); the rest map
# to 403 (you're known, but not entitled). The mapping lives in app.main.
REASON_TOKEN_MISSING = "token_missing"
REASON_TOKEN_INVALID = "token_invalid"
REASON_TOKEN_EXPIRED = "token_expired"
REASON_NO_PLAN = "no_active_plan"
REASON_NO_USES = "no_uses_remaining"
IDENTITY_REASONS = frozenset({REASON_TOKEN_MISSING, REASON_TOKEN_INVALID, REASON_TOKEN_EXPIRED})

# The atomic consume — ONE statement, so concurrent consumes can't both win.
# The INSERT branch (first use this period) is unconditionally 1; the ON CONFLICT
# branch increments only WHILE under the limit. A row returned == this request
# consumed; no row == the use was already taken (lost the race) or the limit was
# hit. Reused verbatim by the Postgres path and mirrored by the race tests.
CONSUME_SQL = """
INSERT INTO usage_counters (user_id, feature, count, period_start)
VALUES ($1, $2, 1, $3)
ON CONFLICT (user_id, feature, period_start)
DO UPDATE SET count = usage_counters.count + 1
WHERE usage_counters.count < $4
RETURNING count
"""


@dataclass
class EntitlementResult:
    entitled: bool
    # One of the REASON_* constants when not entitled.
    reason: str = ""
    # Uses remaining AFTER this request, when metered. None for unmetered.
    remaining: int | None = None


@dataclass
class TierLimits:
    free: int
    premium: int

    def for_tier(self, tier: str) -> int:
        return self.premium if tier == "premium" else self.free


class EntitlementChecker(Protocol):
    """The server-side source of truth. Async: implementations do real I/O."""

    async def check(self, user_token: str) -> EntitlementResult:
        """Verify the token AND report entitlement. MUST verify the token
        itself — never trust it."""
        ...

    async def consume(self, user_token: str) -> None:
        """Record one consumed use, atomically. Called only after success."""
        ...


class JwtError(Exception):
    """Identity failure. ``code`` in {missing, expired, invalid}."""

    def __init__(self, code: str) -> None:
        super().__init__(code)
        self.code = code


class JwtVerifier:
    """Verifies a Supabase ES256 user JWT against the project's public JWKS.

    Only public keys are used — no secret on the server. Tests inject
    ``public_key`` directly (a local EC key) to avoid any network.
    """

    def __init__(
        self,
        *,
        issuer: str,
        audience: str = "authenticated",
        jwks_url: str | None = None,
        public_key: object | None = None,
        algorithms: tuple[str, ...] = ("ES256",),
        leeway: int = 0,
    ) -> None:
        self._issuer = issuer
        self._audience = audience
        self._algorithms = list(algorithms)
        self._leeway = leeway
        self._public_key = public_key
        self._jwk_client = None
        if public_key is None:
            if not jwks_url:
                raise ValueError("JwtVerifier requires jwks_url or public_key")
            from jwt import PyJWKClient

            self._jwk_client = PyJWKClient(jwks_url)

    def verify(self, token: str) -> dict:
        """Return the validated claims, or raise JwtError."""
        import jwt

        if not token:
            raise JwtError("missing")
        try:
            if self._public_key is not None:
                key = self._public_key
            else:
                key = self._jwk_client.get_signing_key_from_jwt(token).key
            return jwt.decode(
                token,
                key,
                algorithms=self._algorithms,
                audience=self._audience,
                issuer=self._issuer,
                leeway=self._leeway,
                options={"require": ["exp", "sub", "aud"]},
            )
        except jwt.ExpiredSignatureError:
            raise JwtError("expired")
        except Exception:
            # Bad signature, wrong aud/iss, malformed, JWKS lookup failure —
            # all collapse to one opaque "invalid" (no oracle for attackers).
            raise JwtError("invalid")


class SupabaseEntitlementChecker:
    """Real gate: ES256 JWT verify (JWKS) + subscriptions/usage_counters reads
    over asyncpg + the atomic conditional consume."""

    def __init__(
        self,
        *,
        dsn: str,
        verifier: JwtVerifier,
        limits: TierLimits,
        feature: str = "publishready",
        now_fn=None,
    ) -> None:
        self._dsn = dsn
        self._verifier = verifier
        self._limits = limits
        self._feature = feature
        self._now = now_fn or (lambda: datetime.now(timezone.utc))
        self._pool = None
        self._pool_lock = asyncio.Lock()

    async def _get_pool(self):
        if self._pool is None:
            async with self._pool_lock:
                if self._pool is None:
                    import asyncpg

                    self._pool = await asyncpg.create_pool(self._dsn, min_size=1, max_size=10)
        return self._pool

    async def aclose(self) -> None:
        """Close the connection pool (graceful shutdown; also used by tests)."""
        if self._pool is not None:
            await self._pool.close()
            self._pool = None

    def _verified_user_id(self, token: str) -> uuid.UUID:
        """Verify the JWT and return the user's uuid (``sub``). Raises JwtError
        on identity failure or a non-uuid subject."""
        claims = self._verifier.verify(token)
        sub = claims.get("sub")
        if not sub:
            raise JwtError("invalid")
        try:
            return uuid.UUID(str(sub))
        except ValueError:
            raise JwtError("invalid")

    def _period(self) -> date:
        now = self._now()
        return date(now.year, now.month, 1)

    async def _effective_tier(self, conn, user_id: uuid.UUID) -> str:
        row = await conn.fetchrow(
            "SELECT tier, status, current_period_end FROM subscriptions WHERE user_id = $1",
            user_id,
        )
        if row is None:
            return "free"
        active = row["status"] == "active" and (
            row["current_period_end"] is None or row["current_period_end"] > self._now()
        )
        return "premium" if (row["tier"] == "premium" and active) else "free"

    async def check(self, user_token: str) -> EntitlementResult:
        # Step 1 — identity. Returns BEFORE any DB work on failure.
        try:
            user_id = self._verified_user_id(user_token)
        except JwtError as exc:
            reason = {
                "missing": REASON_TOKEN_MISSING,
                "expired": REASON_TOKEN_EXPIRED,
            }.get(exc.code, REASON_TOKEN_INVALID)
            return EntitlementResult(entitled=False, reason=reason)

        # Step 2 — entitlement.
        pool = await self._get_pool()
        async with pool.acquire() as conn:
            tier = await self._effective_tier(conn, user_id)
            limit = self._limits.for_tier(tier)
            if limit <= 0:
                return EntitlementResult(entitled=False, reason=REASON_NO_PLAN)
            count = (
                await conn.fetchval(
                    "SELECT count FROM usage_counters "
                    "WHERE user_id = $1 AND feature = $2 AND period_start = $3",
                    user_id,
                    self._feature,
                    self._period(),
                )
                or 0
            )
            if count >= limit:
                return EntitlementResult(entitled=False, reason=REASON_NO_USES, remaining=0)
            return EntitlementResult(entitled=True, remaining=limit - count - 1)

    async def consume(self, user_token: str) -> None:
        # Step 3 — atomic consume, after the paid work already succeeded. The
        # token was validated in check(); re-verify defensively (cheap).
        user_id = self._verified_user_id(user_token)
        pool = await self._get_pool()
        async with pool.acquire() as conn:
            tier = await self._effective_tier(conn, user_id)
            limit = self._limits.for_tier(tier)
            row = await conn.fetchrow(CONSUME_SQL, user_id, self._feature, self._period(), limit)
            if row is None:
                # Lost a concurrency race for the last use, or the plan changed
                # between check and consume. The paid work already ran; we simply
                # could not record a use. Never over-counts. Log, don't crash.
                logger.warning(
                    "entitlement consume: no use available at consume time "
                    "(race or plan change), user=%s",
                    user_id,
                )


def build_supabase_checker(settings) -> SupabaseEntitlementChecker:
    """Construct the real checker from Settings (production wiring). JWKS URL
    and issuer are derived from SUPABASE_URL."""
    if not settings.supabase_url:
        raise ValueError("SUPABASE_URL is required to build the entitlement checker")
    if not settings.database_url:
        raise ValueError("DATABASE_URL is required to build the entitlement checker")
    verifier = JwtVerifier(
        issuer=f"{settings.supabase_url}/auth/v1",
        audience=settings.jwt_audience,
        jwks_url=f"{settings.supabase_url}/auth/v1/.well-known/jwks.json",
    )
    limits = TierLimits(
        free=settings.publishready_limit_free,
        premium=settings.publishready_limit_premium,
    )
    return SupabaseEntitlementChecker(
        dsn=settings.database_url,
        verifier=verifier,
        limits=limits,
        feature=settings.entitlement_feature,
    )
