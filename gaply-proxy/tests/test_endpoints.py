"""End-to-end endpoint tests through the FastAPI app (App Check + rate limit +
validation + a stubbed Claude forward)."""

from app.app_check import HEADER_NAME

from .conftest import DEBUG_TOKEN, GOOD_SUMMARY, make_client, mint

RAW_PROSE_PAYLOAD = {
    "summary": {
        "text": (
            "We examined whether extended sleep improves working memory. "
            "In a randomized trial participants who slept nine hours did better. "
            "Prior work has been mixed. Working memory is central to reasoning. "
            "Earlier studies suggested a link between sleep and cognition. "
            "Ninety-six adults were randomized into two groups. "
            "We used an independent t-test to compare means. "
            "A one-way ANOVA assessed dose response. "
            "The extended-sleep group scored higher. A secondary analysis showed a dose effect. "
            "These findings replicate prior work. Our results support a causal role for sleep."
        )
    }
}


def test_health_needs_no_auth():
    client, _ = make_client()
    resp = client.get("/health")
    assert resp.status_code == 200
    assert resp.json() == {"status": "ok"}


def test_request_without_app_check_token_is_rejected():
    client, stub = make_client()
    resp = client.post("/verify", json=GOOD_SUMMARY)  # no token header
    assert resp.status_code == 401
    assert resp.json()["detail"]["error"] == "app_check_failed"
    assert stub.calls == []  # Claude never called


def test_structured_summary_with_valid_token_passes():
    client, stub = make_client()
    token = mint(ttl=3600, limited_use=True)
    resp = client.post("/verify", json=GOOD_SUMMARY, headers={HEADER_NAME: token})
    assert resp.status_code == 200
    assert resp.json()["result"]["text"] == "reviewed"
    assert len(stub.calls) == 1  # forwarded exactly once


def test_raw_prose_payload_is_rejected_before_forwarding():
    client, stub = make_client()
    token = mint(ttl=3600, limited_use=True)
    resp = client.post("/verify", json=RAW_PROSE_PAYLOAD, headers={HEADER_NAME: token})
    assert resp.status_code == 422
    assert resp.json()["detail"]["error"] == "validation_failed"
    assert stub.calls == []  # rejected before Claude


def test_rate_limit_exhaustion_returns_429():
    # tiny bucket + negligible refill; debug token so all requests authenticate
    client, _ = make_client(capacity=2, refill_rate=1e-9)
    headers = {HEADER_NAME: DEBUG_TOKEN, "x-client-id": "flooder"}
    codes = [
        client.post("/verify", json=GOOD_SUMMARY, headers=headers).status_code
        for _ in range(3)
    ]
    assert codes[0] == 200
    assert codes[1] == 200
    assert codes[2] == 429  # bucket exhausted
