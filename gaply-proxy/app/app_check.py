"""App Check verification (Firebase custom-provider path), Python port.

This MUST interoperate with the Rust core's ``TokenSigner`` (gaply_core
``app_check.rs``): the token is a JWT-like ``base64url(header).base64url(claims)
.base64url(sig)`` signed with HMAC-SHA256 over a shared secret. A token minted
by the Rust client verifies here byte-for-byte, because verification recomputes
the HMAC over the received base64 strings (no re-serialization of claims).

Limited-use tokens carry a unique ``jti`` that the verifier consumes once, so a
replayed limited-use token is rejected — the replay-protection groundwork.
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
import time
from dataclasses import dataclass
from threading import Lock
from typing import Iterable

# Header the Rust client attaches (matches Firebase). Lowercased for lookup.
HEADER_NAME = "x-firebase-appcheck"
DEFAULT_APP_ID = "ai.gaply.app"
DEFAULT_TTL_SECS = 1800


def _b64url_encode(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii")


def _b64url_decode(s: str) -> bytes:
    return base64.urlsafe_b64decode(s + "=" * (-len(s) % 4))


class VerifyError(Exception):
    """Raised when a token fails verification. ``code`` is a stable slug."""

    def __init__(self, code: str) -> None:
        self.code = code
        super().__init__(code)


@dataclass
class VerifiedToken:
    app_id: str
    jti: str
    limited_use: bool
    debug: bool
    exp: int


class _ReplayGuard:
    """Consumes each limited-use ``jti`` exactly once; drops expired entries."""

    def __init__(self) -> None:
        self._seen: dict[str, int] = {}
        self._lock = Lock()

    def consume(self, jti: str, exp: int, now: int) -> bool:
        with self._lock:
            # opportunistic cleanup so the map stays bounded
            self._seen = {k: v for k, v in self._seen.items() if v > now}
            if jti in self._seen:
                return False
            self._seen[jti] = exp
            return True


class AppCheckVerifier:
    def __init__(
        self,
        key: bytes,
        app_id: str = DEFAULT_APP_ID,
        debug_tokens: Iterable[str] = (),
    ) -> None:
        self.key = key
        self.app_id = app_id
        self.debug_tokens = set(debug_tokens)
        self._replay = _ReplayGuard()

    def verify(self, token: str, now: int | None = None) -> VerifiedToken:
        if now is None:
            now = int(time.time())
        if not token:
            raise VerifyError("missing")

        if token in self.debug_tokens:
            return VerifiedToken(self.app_id, "", False, True, now)

        parts = token.split(".")
        if len(parts) != 3:
            raise VerifyError("malformed")

        signing_input = f"{parts[0]}.{parts[1]}".encode("ascii")
        try:
            sig = _b64url_decode(parts[2])
        except Exception:  # noqa: BLE001 - any decode failure is malformed
            raise VerifyError("malformed")

        # signature FIRST, so a forged/tampered token never consumes a jti
        expected = hmac.new(self.key, signing_input, hashlib.sha256).digest()
        if not hmac.compare_digest(expected, sig):  # constant-time
            raise VerifyError("bad_signature")

        try:
            claims = json.loads(_b64url_decode(parts[1]))
        except Exception:  # noqa: BLE001
            raise VerifyError("malformed")

        if claims.get("app_id") != self.app_id:
            raise VerifyError("wrong_app")
        exp = int(claims.get("exp", 0))
        if exp <= now:
            raise VerifyError("expired")
        if claims.get("limited_use"):
            if not self._replay.consume(str(claims.get("jti", "")), exp, now):
                raise VerifyError("replayed")

        return VerifiedToken(
            app_id=str(claims["app_id"]),
            jti=str(claims.get("jti", "")),
            limited_use=bool(claims.get("limited_use")),
            debug=False,
            exp=exp,
        )


def mint_token(
    key: bytes,
    app_id: str = DEFAULT_APP_ID,
    ttl: int = DEFAULT_TTL_SECS,
    limited_use: bool = False,
    now: int | None = None,
    jti: str | None = None,
) -> str:
    """Mint a token in the same format as the Rust client.

    Provided for dev/CI and tests — in production the client tokens are minted
    by the Rust core, not here.
    """
    if now is None:
        now = int(time.time())
    if jti is None:
        jti = f"jti-{now}-{time.perf_counter_ns()}"
    header = b'{"alg":"HS256","typ":"JWT"}'
    claims = {
        "app_id": app_id,
        "iat": now,
        "exp": now + ttl,
        "jti": jti,
        "limited_use": limited_use,
    }
    claims_bytes = json.dumps(claims, separators=(",", ":")).encode("utf-8")
    signing_input = f"{_b64url_encode(header)}.{_b64url_encode(claims_bytes)}"
    sig = hmac.new(key, signing_input.encode("ascii"), hashlib.sha256).digest()
    return f"{signing_input}.{_b64url_encode(sig)}"
