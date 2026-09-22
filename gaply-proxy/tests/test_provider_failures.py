"""**What a client receives when the upstream model provider fails.**

Measured 22 Sep 2026, before this existed: FIVE distinct causes — an OpenAI
outage, an OpenAI rate limit, a bad API key, a connect timeout, and a bug in
the proxy — all produced the identical opaque response::

    500  text/plain  "Internal Server Error"

`provider.complete()` was not wrapped, so every exception reached FastAPI's
default handler. The desktop's `map_error_status` sent that down its
``other =>`` arm, `commands.rs` discarded the text, and the screen said
*"reviewer unavailable: cloud proxy not reachable"* — which is FALSE. The proxy
was reachable; it answered. A user would check their network while the actual
cause was upstream.

These tests pin the categories and, just as importantly, pin that the reason
text never carries the payload or a credential.
"""

from __future__ import annotations

import httpx
import pytest
from fastapi.testclient import TestClient

from app import app_check
from app.app_check import HEADER_NAME
from app.config import Settings
from app.main import create_app

from .conftest import DEBUG_TOKEN, TEST_KEY

SENTINEL = "MANUSCRIPT-SENTINEL-XYZ"
API_KEY = "sk-proj-NOTAREALKEY-0123456789"
PAYLOAD = {"summary": {"finding": SENTINEL}, "instruction": "review this"}


class RaisingUpstream:
    """Stands in for the provider client when the upstream call fails."""

    def __init__(self, exc: BaseException) -> None:
        self.exc = exc

    async def complete(self, payload):  # noqa: ANN001 - provider protocol
        raise self.exc


def _client(exc: BaseException) -> TestClient:
    settings = Settings(
        app_check_signing_key=TEST_KEY.decode("ascii"),
        app_check_app_id=app_check.DEFAULT_APP_ID,
        app_check_debug_tokens=(DEBUG_TOKEN,),
        rate_limit_capacity=100.0,
        rate_limit_refill_rate=1e-9,
    )
    app = create_app(settings=settings, claude_client=RaisingUpstream(exc))
    # A real client over the wire gets the RESPONSE, not the exception.
    return TestClient(app, raise_server_exceptions=False)


def _upstream(status: int) -> httpx.HTTPStatusError:
    req = httpx.Request("POST", "https://api.openai.com/v1/chat/completions")
    return httpx.HTTPStatusError(
        f"error '{status}' for url '{req.url}' key={API_KEY}",
        request=req,
        response=httpx.Response(status),
    )


@pytest.mark.parametrize(
    ("exc", "status", "category"),
    [
        (_upstream(503), 503, "upstream_unavailable"),
        (_upstream(500), 503, "upstream_unavailable"),
        (httpx.ConnectTimeout("timed out"), 503, "upstream_unavailable"),
        (httpx.ConnectError("refused"), 503, "upstream_unavailable"),
        (_upstream(429), 502, "upstream_rejected"),
        (_upstream(401), 502, "upstream_rejected"),
        (_upstream(400), 502, "upstream_rejected"),
        (KeyError("choices"), 500, "proxy_error"),
        (ValueError("bad shape"), 500, "proxy_error"),
    ],
)
def test_a_provider_failure_is_categorised_not_collapsed_into_500(exc, status, category):
    r = _client(exc).post("/verify", json=PAYLOAD, headers={HEADER_NAME: DEBUG_TOKEN})
    assert r.status_code == status, r.text
    assert r.headers["content-type"].startswith("application/json"), r.headers
    detail = r.json()["detail"]
    assert detail["error"] == category, detail
    assert detail["reason"], "a category with no reason tells a user nothing"


def test_the_three_categories_are_distinguishable_from_one_another():
    """The defect was that they were NOT. Asserted as one table, because the
    failure mode is a uniform column across rows that should differ."""
    seen = {}
    for exc in (_upstream(503), _upstream(401), KeyError("choices")):
        r = _client(exc).post("/verify", json=PAYLOAD, headers={HEADER_NAME: DEBUG_TOKEN})
        seen[r.json()["detail"]["error"]] = r.status_code
    assert set(seen) == {"upstream_unavailable", "upstream_rejected", "proxy_error"}, seen
    assert len(set(seen.values())) == 3, f"each category needs its own status: {seen}"


@pytest.mark.parametrize(
    "exc",
    [
        _upstream(503),
        _upstream(401),
        httpx.ConnectTimeout(f"connecting with key={API_KEY}"),
        # THE proxy_error PATH NEEDS A SENSITIVE FIXTURE TOO. A deletion test
        # that switched this branch to `str(exc)` stayed GREEN, because the only
        # proxy_error fixtures were `KeyError("choices")` and `ValueError("bad
        # shape")`, whose messages carry nothing worth leaking. The test could
        # not see a leak in the branch it was meant to guard.
        KeyError(f"choices {API_KEY} {SENTINEL}"),
        ValueError(f"bad shape near {SENTINEL} with key={API_KEY}"),
    ],
)
def test_an_error_never_leaks_the_manuscript_or_a_credential(exc):
    """The reason is built from the exception TYPE and the upstream status code
    only — never `str(exc)`, which carries the request URL and, in the fixture
    above, an API key planted there on purpose."""
    r = _client(exc).post("/verify", json=PAYLOAD, headers={HEADER_NAME: DEBUG_TOKEN})
    body = r.text
    assert SENTINEL not in body, f"manuscript text leaked into an error: {body}"
    assert API_KEY not in body, f"a credential leaked into an error: {body}"
    assert "sk-" not in body, f"something key-shaped leaked into an error: {body}"
