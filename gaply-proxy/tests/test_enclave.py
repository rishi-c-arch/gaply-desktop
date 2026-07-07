"""Nitro Enclave attestation tests (mocked crypto).

VERIFIED HERE (offline): the attestation *policy* — a well-formed, correctly
"signed" document with the expected PCR0, a chain to the pinned AWS root, a
matching nonce, and a fresh timestamp is ACCEPTED; tampering with any of those
is REJECTED with a specific code. The signature is mock HMAC (see enclave.py).

NOT verifiable here: real CBOR/COSE_Sign1 ES384 decoding, the genuine AWS Nitro
root-CA chain, building the enclave image (EIF -> PCR0), and a real VSOCK
attestation — all require an AWS account with a Nitro-Enclave-capable instance.
"""

import asyncio

import pytest

from app.enclave import (
    AWS_NITRO_ROOT_CA_FINGERPRINT,
    AttestationDocument,
    AttestationError,
    AttestationVerifier,
    ExpectedEnclave,
    hmac_signature_check,
    mock_sign,
)
from app.enclave_client import EnclaveClaudeClient, MockVsockTransport

ENCLAVE_KEY = b"mock-nsm-signing-key-0123456789abcdef"
EXPECTED_PCR0 = "a" * 96  # 48-byte SHA-384 as hex
NOW_MS = 1_760_000_000_000
NONCE = "cafebabecafebabe"


def make_doc(key=ENCLAVE_KEY, **overrides) -> AttestationDocument:
    """A valid, correctly-signed mock attestation document (before overrides)."""
    fields = {
        "module_id": "i-0abc123def-enc0123456789",
        "timestamp_ms": NOW_MS,
        "digest": "SHA384",
        "pcrs": {0: EXPECTED_PCR0, 1: "b" * 96, 2: "c" * 96, 8: "d" * 96},
        "cabundle": ["intermediate-ca-marker", AWS_NITRO_ROOT_CA_FINGERPRINT],
        "certificate": "leaf-cert-marker",
        "public_key": "enclave-pubkey",
        "user_data": "",
        "nonce": NONCE,
    }
    fields.update(overrides)
    fields["signature"] = mock_sign(fields, key)  # sign AFTER overrides
    return AttestationDocument.from_mapping(fields)


def verifier(**kw) -> AttestationVerifier:
    expected = ExpectedEnclave(pcr0=EXPECTED_PCR0)
    return AttestationVerifier(expected, hmac_signature_check(ENCLAVE_KEY), **kw)


# --- ACCEPT -----------------------------------------------------------------

def test_valid_attestation_is_accepted():
    v = verifier()
    result = v.verify(make_doc(), nonce=NONCE, now_ms=NOW_MS)
    assert result.pcr0 == EXPECTED_PCR0
    assert result.module_id.startswith("i-")
    assert result.public_key == "enclave-pubkey"


def test_accepts_within_max_age_window():
    v = verifier(max_age_secs=300)
    # 299s old -> still fresh
    v.verify(make_doc(), nonce=NONCE, now_ms=NOW_MS + 299_000)


# --- REJECT: tampering with each protected property -------------------------

def test_rejects_tampered_pcr0_code_identity():
    # An attacker runs DIFFERENT code inside the enclave -> PCR0 differs.
    doc = make_doc(pcrs={0: "f" * 96, 1: "b" * 96, 2: "c" * 96, 8: "d" * 96})
    with pytest.raises(AttestationError) as e:
        verifier().verify(doc, nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "pcr_mismatch"


def test_rejects_tampered_signature():
    # Flip a byte of the signature: the document was altered / forged.
    doc = make_doc()
    bad = AttestationDocument.from_mapping(
        {**_as_map(doc), "signature": ("0" if doc.signature[0] != "0" else "1") + doc.signature[1:]}
    )
    with pytest.raises(AttestationError) as e:
        verifier().verify(bad, nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "bad_signature"


def test_rejects_tampered_payload_breaks_signature():
    # Change module_id WITHOUT re-signing (attacker can't sign): signature fails.
    good = make_doc()
    forged = AttestationDocument.from_mapping({**_as_map(good), "module_id": "i-evil-enc"})
    with pytest.raises(AttestationError) as e:
        verifier().verify(forged, nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "bad_signature"


def test_rejects_untrusted_root():
    # cabundle does not terminate at the pinned AWS Nitro root.
    doc = make_doc(cabundle=["intermediate", "some-other-root"])
    with pytest.raises(AttestationError) as e:
        verifier().verify(doc, nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "untrusted_root"


def test_rejects_signed_by_wrong_key():
    # Document is internally consistent but signed by a non-NSM key.
    doc = make_doc(key=b"attacker-key-not-the-real-nsm-000")
    with pytest.raises(AttestationError) as e:
        verifier().verify(doc, nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "bad_signature"


def test_rejects_replayed_or_wrong_nonce():
    doc = make_doc(nonce="0000000000000000")  # not the nonce we challenged with
    with pytest.raises(AttestationError) as e:
        verifier().verify(doc, nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "nonce_mismatch"


def test_rejects_stale_attestation():
    doc = make_doc()  # timestamp NOW_MS
    with pytest.raises(AttestationError) as e:
        verifier(max_age_secs=300).verify(doc, nonce=NONCE, now_ms=NOW_MS + 600_000)
    assert e.value.code == "stale"


def test_rejects_future_timestamp():
    doc = make_doc(timestamp_ms=NOW_MS + 10_000_000)
    with pytest.raises(AttestationError) as e:
        verifier().verify(doc, nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "stale"


def test_rejects_malformed_document():
    with pytest.raises(AttestationError) as e:
        AttestationDocument.from_mapping({"module_id": "i-x"})  # missing fields
    assert e.value.code == "malformed"


def test_rejects_pinned_pcr1_mismatch_when_expected():
    expected = ExpectedEnclave(pcr0=EXPECTED_PCR0, pcr1="9" * 96)
    v = AttestationVerifier(expected, hmac_signature_check(ENCLAVE_KEY))
    with pytest.raises(AttestationError) as e:
        v.verify(make_doc(), nonce=NONCE, now_ms=NOW_MS)
    assert e.value.code == "pcr_mismatch"


# --- Enclave client handshake (attest -> verify -> forward) -----------------

def _enclave_stub(message):
    """A fake enclave: attests on demand, returns a Claude result on complete."""
    if message["op"] == "attest":
        doc = make_doc(nonce=message["nonce"])
        return _as_map(doc)
    if message["op"] == "complete":
        return {"result": {"model": "enclave-sonnet", "text": "reviewed-in-enclave"}}
    raise AssertionError(message)


def test_enclave_client_attests_then_forwards():
    transport = MockVsockTransport(_enclave_stub)
    client = EnclaveClaudeClient(transport, verifier(), now_ms=lambda: NOW_MS)
    out = asyncio.run(client.complete({"summary": {"title": "x"}}))
    assert out["text"] == "reviewed-in-enclave"
    # It attested BEFORE forwarding: first call is attest, second is complete.
    assert [c["op"] for c in transport.calls] == ["attest", "complete"]


def test_enclave_client_blocks_forward_when_attestation_fails():
    # Enclave presents a doc with the wrong PCR0 -> verification fails -> the
    # summary is NEVER forwarded.
    def bad_stub(message):
        if message["op"] == "attest":
            return _as_map(make_doc(nonce=message["nonce"], pcrs={0: "f" * 96, 1: "b" * 96, 2: "c" * 96, 8: "d" * 96}))
        return {"result": {"text": "should-not-happen"}}

    transport = MockVsockTransport(bad_stub)
    client = EnclaveClaudeClient(transport, verifier(), now_ms=lambda: NOW_MS)
    with pytest.raises(AttestationError):
        asyncio.run(client.complete({"summary": {}}))
    assert [c["op"] for c in transport.calls] == ["attest"]  # no "complete"


def _as_map(doc: AttestationDocument) -> dict:
    return {
        "module_id": doc.module_id,
        "timestamp_ms": doc.timestamp_ms,
        "digest": doc.digest,
        "pcrs": dict(doc.pcrs),
        "cabundle": list(doc.cabundle),
        "certificate": doc.certificate,
        "signature": doc.signature,
        "public_key": doc.public_key,
        "user_data": doc.user_data,
        "nonce": doc.nonce,
    }
