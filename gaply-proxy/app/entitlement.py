"""Server-side entitlement gate — THE REAL GATE for paid work (Set 8).

The client's locked/upgrade UI is PRESENTATION ONLY; a tampered client can
skip it. This module is where a non-entitled request actually dies: the proxy
verifies entitlement BEFORE the paid Claude call, and CONSUMING a use happens
here (server-side, after a successful call) — never as a client-side number.

Pricing-agnostic by design: nothing in this module (or anywhere in the app)
knows a price. The only question ever asked is "does this user have an
available use / active plan?" — prices live in the payment provider and
server config, changeable without a code change.

HONEST STUB STATUS: the proxy is not yet deployed, so enforcement defaults to
OFF (``entitlement_required=False``) and current behavior is unchanged. A
production deploy MUST:

  1. set ``GAPLY_ENTITLEMENT_REQUIRED=true`` (fail-closed: with no checker
     injected the proxy then 503s rather than serving unmetered paid work),
  2. inject a real ``EntitlementChecker`` into ``create_app`` — the intended
     implementation verifies the Supabase user JWT from the
     ``X-Gaply-User-Token`` header (signature + expiry, service-role key on
     the server only), reads the user's subscription/uses row, and
     atomically decrements a use on ``consume``.

The client cannot tamper with any of this: it only transports its own JWT.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

# The signed-in user's Supabase JWT, attached by the Rust client. This is the
# USER'S OWN credential (never an app secret) — the server verifies it.
USER_TOKEN_HEADER = "X-Gaply-User-Token"


@dataclass
class EntitlementResult:
    entitled: bool
    # 'no_active_plan' | 'no_uses_remaining' | 'invalid_user' | ...
    reason: str = ""
    # Uses remaining AFTER this request, when the plan is metered. None for
    # unmetered/active-plan entitlements. Informational only.
    remaining: int | None = None


class EntitlementChecker(Protocol):
    """The server-side source of truth. Implementations own the storage."""

    def check(self, user_token: str) -> EntitlementResult:
        """Does the holder of this (verified) user token have an available
        use / active plan? MUST verify the token itself — never trust it."""
        ...

    def consume(self, user_token: str) -> None:
        """Record one consumed use for this user — atomically, server-side.
        Called only after the paid work succeeded."""
        ...
