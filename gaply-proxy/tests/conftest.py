"""Shared test fixtures: a stub Claude client (no network) and a helper to
build a TestClient with test settings and mint valid App Check tokens."""

from __future__ import annotations

from typing import Any

import pytest
from fastapi.testclient import TestClient

from app import app_check
from app.config import Settings
from app.main import create_app

# Shared HMAC secret for tests (mint + verify use the same bytes).
TEST_KEY = b"proxy-test-signing-key-0123456789abcdef"
DEBUG_TOKEN = "DEBUG-allow-ci"


class StubClaude:
    """Records calls, returns a canned response — never hits the network."""

    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []

    async def complete(self, payload: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(payload)
        return {"model": "stub-sonnet", "stop_reason": "end_turn", "text": "reviewed"}


def make_client(
    *,
    capacity: float = 100.0,
    refill_rate: float = 1e-9,
    claude: StubClaude | None = None,
    entitlement_required: bool = False,
    entitlement_checker=None,
) -> tuple[TestClient, StubClaude]:
    stub = claude or StubClaude()
    settings = Settings(
        app_check_signing_key=TEST_KEY.decode("ascii"),
        app_check_app_id=app_check.DEFAULT_APP_ID,
        app_check_debug_tokens=(DEBUG_TOKEN,),
        rate_limit_capacity=capacity,
        rate_limit_refill_rate=refill_rate,
        entitlement_required=entitlement_required,
    )
    app = create_app(
        settings=settings, claude_client=stub, entitlement_checker=entitlement_checker
    )
    return TestClient(app), stub


def mint(**kwargs) -> str:
    return app_check.mint_token(TEST_KEY, **kwargs)


@pytest.fixture
def client_and_stub():
    return make_client()


# a small, well-formed structured summary used by the passing-path tests
GOOD_SUMMARY = {
    "summary": {
        "title": "Sleep and working memory",
        "sections": ["abstract", "methods", "results"],
        "key_findings": ["n = 96", "p < 0.001", "95% CI 2.1 to 4.8"],
        "stats": [{"test": "t-test"}, {"test": "ANOVA"}],
    },
    "instruction": "Check the statistics for validity.",
}
