import pytest

from app.app_check import DEFAULT_APP_ID, AppCheckVerifier, VerifyError, mint_token

KEY = b"unit-test-app-check-key-0123456789"


def verifier(**kw) -> AppCheckVerifier:
    return AppCheckVerifier(KEY, DEFAULT_APP_ID, **kw)


def test_valid_token_verifies():
    token = mint_token(KEY, now=1000, ttl=3600, limited_use=False)
    result = verifier().verify(token, now=1000)
    assert result.app_id == DEFAULT_APP_ID
    assert not result.debug


def test_missing_and_malformed():
    v = verifier()
    with pytest.raises(VerifyError) as e:
        v.verify("")
    assert e.value.code == "missing"
    with pytest.raises(VerifyError) as e:
        v.verify("only.two")
    assert e.value.code == "malformed"


def test_bad_signature_is_rejected():
    token = mint_token(b"a-different-secret", now=1000, ttl=3600)
    with pytest.raises(VerifyError) as e:
        verifier().verify(token, now=1000)
    assert e.value.code == "bad_signature"


def test_wrong_app_id():
    token = mint_token(KEY, app_id="evil.app", now=1000, ttl=3600)
    with pytest.raises(VerifyError) as e:
        verifier().verify(token, now=1000)
    assert e.value.code == "wrong_app"


def test_expired_token():
    token = mint_token(KEY, now=1000, ttl=60)  # exp = 1060
    with pytest.raises(VerifyError) as e:
        verifier().verify(token, now=2000)
    assert e.value.code == "expired"


def test_limited_use_replay_rejected():
    v = verifier()
    token = mint_token(KEY, now=1000, ttl=3600, limited_use=True, jti="once")
    assert v.verify(token, now=1000).limited_use
    with pytest.raises(VerifyError) as e:
        v.verify(token, now=1000)
    assert e.value.code == "replayed"


def test_standard_token_is_reusable():
    v = verifier()
    token = mint_token(KEY, now=1000, ttl=3600, limited_use=False)
    assert v.verify(token, now=1000)
    assert v.verify(token, now=1000)  # reusable until exp


def test_debug_token_passes():
    v = verifier(debug_tokens=["DEBUG-xyz"])
    result = v.verify("DEBUG-xyz")
    assert result.debug
    with pytest.raises(VerifyError):
        v.verify("DEBUG-nope")
