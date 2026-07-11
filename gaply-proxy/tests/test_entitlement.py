"""Set 8 — server-side entitlement gate tests.

THE REAL GATE lives here at the proxy (the client's lock screen is UX only).
These tests prove: enforcement blocks non-entitled users BEFORE any paid
Claude work; a use is consumed server-side only after success; enabled-but-
unconfigured fails CLOSED; and the default (proxy not deployed) is honestly
unchanged. Prices appear nowhere — the gate only asks "entitled?".
"""

from __future__ import annotations

from app.app_check import HEADER_NAME
from app.entitlement import USER_TOKEN_HEADER, EntitlementResult

from .conftest import GOOD_SUMMARY, StubClaude, make_client, mint


class FakeChecker:
    """Scriptable server-side source of truth; records consume calls."""

    def __init__(self, entitled: bool, reason: str = "") -> None:
        self.result = EntitlementResult(entitled=entitled, reason=reason)
        self.checked: list[str] = []
        self.consumed: list[str] = []

    def check(self, user_token: str) -> EntitlementResult:
        self.checked.append(user_token)
        return self.result

    def consume(self, user_token: str) -> None:
        self.consumed.append(user_token)


class FailingClaude(StubClaude):
    """Paid work that blows up — consume must NOT happen."""

    async def complete(self, payload):
        self.calls.append(payload)
        raise RuntimeError("upstream exploded")


def post(client, extra_headers=None):
    headers = {HEADER_NAME: mint(ttl=3600, limited_use=True)}
    headers.update(extra_headers or {})
    return client.post("/verify", json=GOOD_SUMMARY, headers=headers)


def test_default_off_is_unchanged_behavior():
    # The honest pre-deployment stub: enforcement off, no user token needed.
    client, stub = make_client()
    resp = post(client)
    assert resp.status_code == 200
    assert len(stub.calls) == 1


def test_enabled_without_checker_fails_closed():
    # Misconfigured production (flag on, no checker) must refuse paid work,
    # never silently skip the gate.
    client, stub = make_client(entitlement_required=True)
    resp = post(client, {USER_TOKEN_HEADER: "user-jwt"})
    assert resp.status_code == 503
    assert resp.json()["detail"]["error"] == "entitlement_not_configured"
    assert stub.calls == [], "no paid work without a configured gate"


def test_missing_user_token_is_401():
    checker = FakeChecker(entitled=True)
    client, stub = make_client(entitlement_required=True, entitlement_checker=checker)
    resp = post(client)  # no user token header
    assert resp.status_code == 401
    assert resp.json()["detail"]["error"] == "user_token_missing"
    assert stub.calls == []
    assert checker.consumed == []


def test_not_entitled_is_403_before_any_paid_work():
    checker = FakeChecker(entitled=False, reason="no_uses_remaining")
    client, stub = make_client(entitlement_required=True, entitlement_checker=checker)
    resp = post(client, {USER_TOKEN_HEADER: "user-jwt"})
    assert resp.status_code == 403
    detail = resp.json()["detail"]
    assert detail["error"] == "not_entitled"
    assert detail["reason"] == "no_uses_remaining"
    # THE point: the model was never called and nothing was consumed.
    assert stub.calls == []
    assert checker.consumed == []


def test_entitled_proceeds_and_consumes_one_use_after_success():
    checker = FakeChecker(entitled=True)
    client, stub = make_client(entitlement_required=True, entitlement_checker=checker)
    resp = post(client, {USER_TOKEN_HEADER: "user-jwt"})
    assert resp.status_code == 200
    assert len(stub.calls) == 1
    assert checker.checked == ["user-jwt"]
    assert checker.consumed == ["user-jwt"], "exactly one use, consumed server-side"


def test_no_consumption_when_the_paid_work_fails():
    checker = FakeChecker(entitled=True)
    failing = FailingClaude()
    client, _ = make_client(
        entitlement_required=True, entitlement_checker=checker, claude=failing
    )
    try:
        resp = post(client, {USER_TOKEN_HEADER: "user-jwt"})
        assert resp.status_code >= 500
    except RuntimeError:
        pass  # TestClient may re-raise unhandled app exceptions — same proof
    assert checker.consumed == [], "a failed call must not burn a use"
