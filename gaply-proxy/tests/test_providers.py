"""Part 2 — OpenAI as a second provider, alongside Claude.

Proves: the OpenAI client maps the structured payload + normalizes the response
envelope; provider selection is SERVER-SIDE config (not a client field, so the
desktop stays provider-blind); and BOTH providers go through the SAME App Check
+ entitlement gate — no separate auth path for OpenAI.
"""

from __future__ import annotations

import asyncio
import json

import httpx
import pytest

from app.app_check import DEFAULT_APP_ID, HEADER_NAME
from app.config import Settings
from app.entitlement import USER_TOKEN_HEADER, EntitlementResult
from app.main import create_app
from app.openai_client import OpenAIClient
from fastapi.testclient import TestClient

from .conftest import DEBUG_TOKEN, GOOD_SUMMARY, TEST_KEY, StubClaude, mint


class StubProvider:
    """Records calls; returns a tagged envelope so we can tell providers apart."""

    def __init__(self, tag: str) -> None:
        self.tag = tag
        self.calls: list = []

    async def complete(self, payload):
        self.calls.append(payload)
        return {"model": f"stub-{self.tag}", "stop_reason": "stop", "text": "ok"}


class FakeChecker:
    def __init__(self, entitled: bool, reason: str = "") -> None:
        self.result = EntitlementResult(entitled=entitled, reason=reason)
        self.consumed: list = []

    async def check(self, user_token: str) -> EntitlementResult:
        return self.result

    async def consume(self, user_token: str) -> None:
        self.consumed.append(user_token)


def _app(*, claude=None, providers=None, entitlement_checker=None, **settings_kw):
    settings = Settings(
        app_check_signing_key=TEST_KEY.decode("ascii"),
        app_check_app_id=DEFAULT_APP_ID,
        app_check_debug_tokens=(DEBUG_TOKEN,),
        rate_limit_capacity=100.0,
        rate_limit_refill_rate=1e-9,
        **settings_kw,
    )
    app = create_app(
        settings=settings,
        claude_client=claude,
        entitlement_checker=entitlement_checker,
        providers=providers,
    )
    return TestClient(app)


def _post(client, body=None, extra_headers=None):
    headers = {HEADER_NAME: mint(ttl=3600, limited_use=True)}
    headers.update(extra_headers or {})
    return client.post("/verify", json=body or GOOD_SUMMARY, headers=headers)


# --- 1. OpenAI client: payload mapping + envelope normalization (no network) --

def test_openai_client_maps_payload_and_normalizes_envelope():
    captured = {}

    def handler(request: httpx.Request) -> httpx.Response:
        captured["auth"] = request.headers.get("authorization")
        captured["body"] = json.loads(request.content)
        return httpx.Response(
            200,
            json={
                "model": "gpt-4o-mini-2024",
                "choices": [{"finish_reason": "stop", "message": {"content": "REVIEWED"}}],
            },
        )

    async def go():
        async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as hc:
            oc = OpenAIClient("sk-test-key", "gpt-4o-mini", http_client=hc)
            return await oc.complete(
                {"summary": {"title": "sleep"}, "instruction": "check stats", "max_tokens": 256}
            )

    res = asyncio.run(go())
    # Normalized to the SAME envelope shape Claude returns.
    assert res == {"model": "gpt-4o-mini-2024", "stop_reason": "stop", "text": "REVIEWED"}
    # Key travels only in the Authorization header, from the arg (never hardcoded).
    assert captured["auth"] == "Bearer sk-test-key"
    assert captured["body"]["model"] == "gpt-4o-mini"
    assert captured["body"]["max_tokens"] == 256
    user_msg = next(m for m in captured["body"]["messages"] if m["role"] == "user")["content"]
    assert '"title": "sleep"' in user_msg and "check stats" in user_msg


# --- 2. Provider selection is SERVER-SIDE config -----------------------------

def test_default_provider_is_claude():
    claude = StubClaude()
    openai = StubProvider("openai")
    client = _app(claude=claude, providers={"openai": openai})  # llm_provider defaults to claude
    resp = _post(client)
    assert resp.status_code == 200
    assert len(claude.calls) == 1 and openai.calls == []


def test_config_selects_openai():
    claude = StubClaude()
    openai = StubProvider("openai")
    client = _app(claude=claude, providers={"openai": openai}, llm_provider="openai")
    resp = _post(client)
    assert resp.status_code == 200
    assert resp.json()["result"]["model"] == "stub-openai"
    assert openai.calls and claude.calls == []


def test_openai_not_configured_is_503():
    # Provider selected but no key and nothing injected -> honest 503.
    client = _app(claude=StubClaude(), llm_provider="openai")
    resp = _post(client)
    assert resp.status_code == 503
    assert resp.json()["detail"]["error"] == "openai_not_configured"


# --- 3. Provider-blindness: a client-sent field can NEVER pick the provider ---

def test_client_provider_field_is_ignored():
    claude = StubClaude()
    openai = StubProvider("openai")
    # Config says claude; a tampered client tries to force openai via the body.
    client = _app(claude=claude, providers={"openai": openai}, llm_provider="claude")
    body = dict(GOOD_SUMMARY, provider="openai")  # client-supplied — must be ignored
    resp = _post(client, body=body)
    assert resp.status_code == 200
    assert len(claude.calls) == 1 and openai.calls == [], "config wins; client field ignored"


# --- 4. BOTH providers ride the SAME entitlement gate ------------------------

def test_openai_call_blocked_by_entitlement_gate():
    openai = StubProvider("openai")
    checker = FakeChecker(entitled=False, reason="no_uses_remaining")
    client = _app(
        providers={"openai": openai},
        llm_provider="openai",
        entitlement_required=True,
        entitlement_checker=checker,
    )
    resp = _post(client, extra_headers={USER_TOKEN_HEADER: "user-jwt"})
    assert resp.status_code == 403
    assert resp.json()["detail"]["error"] == "not_entitled"
    # Same gate as Claude: the paid OpenAI call never happened.
    assert openai.calls == [] and checker.consumed == []


def test_openai_call_consumes_one_use_after_success():
    openai = StubProvider("openai")
    checker = FakeChecker(entitled=True)
    client = _app(
        providers={"openai": openai},
        llm_provider="openai",
        entitlement_required=True,
        entitlement_checker=checker,
    )
    resp = _post(client, extra_headers={USER_TOKEN_HEADER: "user-jwt"})
    assert resp.status_code == 200
    assert len(openai.calls) == 1
    assert checker.consumed == ["user-jwt"], "one use consumed server-side, same as Claude"
