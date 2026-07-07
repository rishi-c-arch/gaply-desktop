"""Trusted Execution Environment (TEE) attestation — AWS Nitro Enclaves.

WHY: even with the proxy hidden on a tailnet (bind_guard + Tailscale), the *host*
still sees the Claude API key and the structured summaries in cleartext. A Nitro
Enclave removes the host from the trust boundary: sensitive processing (holding
the API key, making the outbound Claude call) runs in a VM whose memory the
parent instance cannot read, reachable only over VSOCK. Before the proxy trusts
that enclave, it verifies an **attestation document** proving *which code* is
running inside.

Flow (see deploy/enclave/README.md for the full picture):
  1. proxy (parent) generates a random `nonce` and asks the enclave to attest.
  2. enclave calls the Nitro Security Module (NSM) -> returns a COSE_Sign1
     attestation document containing its PCR measurements, a cert chain to the
     AWS Nitro root CA, the nonce echoed back, and a timestamp.
  3. proxy verifies: cert chain -> AWS root, COSE signature, PCR0 == expected
     enclave image measurement (code identity), nonce match (freshness), and
     timestamp within skew. Only then does it forward the structured summary in.
  4. enclave holds the API key, calls Claude, returns the result over VSOCK.

WHAT IS REAL HERE vs WHAT NEEDS AWS:
  * Real & unit-tested now: the *verification policy* — required fields, root-of-
    trust pinning, PCR (code-identity) match, nonce/freshness, and signature
    check — plus ACCEPT/REJECT behaviour against tampered documents.
  * Mocked: the cryptography. A real Nitro doc is CBOR/COSE_Sign1 signed ES384
    (ECDSA P-384) by a leaf cert chaining to AWS's published Nitro root CA. Here
    the signature is HMAC-SHA256 over a canonical payload and the "root" is a
    pinned fingerprint string, so the policy can be exercised without AWS. The
    `SignatureCheck` seam is where the real COSE + x509 verification drops in
    (aws-nitro-enclaves-nsm-api / `cose` / `cryptography`).  Building the enclave
    image, obtaining a genuine attestation, and confirming the AWS root chain all
    require a real AWS account with a Nitro-Enclave-capable instance.
"""

from __future__ import annotations

import hashlib
import hmac
import json
from dataclasses import dataclass, field
from typing import Callable, Mapping

# AWS publishes a single, stable Nitro Enclaves Root CA. In production the leaf
# cert in the attestation document must chain to THIS root (by its SHA-384
# fingerprint). Here it is a pinned marker the mock cabundle must present.
AWS_NITRO_ROOT_CA_FINGERPRINT = "aws-nitro-enclaves-root-g1"

# Attestation documents are short-lived; reject anything older than this (or
# timestamped in the future beyond a small clock skew).
DEFAULT_MAX_AGE_SECS = 300
DEFAULT_CLOCK_SKEW_SECS = 60

# PCR indices that matter for a Nitro enclave (each is a 48-byte SHA-384 value):
#   PCR0 — hash of the enclave image (EIF): the code identity we pin.
#   PCR1 — linux kernel + bootstrap; PCR2 — application; PCR8 — signing cert.
PCR0 = 0


class AttestationError(Exception):
    """Attestation verification failed. `code` is a stable machine label."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(f"{code}: {detail}" if detail else code)
        self.code = code
        self.detail = detail


@dataclass(frozen=True)
class ExpectedEnclave:
    """The code identity the proxy will trust (pin these from `nitro-cli`)."""

    pcr0: str  # hex SHA-384 of the enclave image (EIF) — REQUIRED
    pcr1: str | None = None
    pcr2: str | None = None
    pcr8: str | None = None
    module_id_prefix: str = "i-"  # enclave module ids look like i-0abc...-enc...
    root_ca_fingerprint: str = AWS_NITRO_ROOT_CA_FINGERPRINT


@dataclass(frozen=True)
class AttestationDocument:
    """Decoded Nitro attestation document (fields mirror the real CBOR map).

    In production this is produced by decoding CBOR/COSE_Sign1 from the NSM; here
    tests construct it directly. `signature` stands in for the COSE_Sign1
    signature (mock: HMAC-SHA256 over the canonical payload).
    """

    module_id: str
    timestamp_ms: int
    digest: str  # always "SHA384" for Nitro
    pcrs: Mapping[int, str]  # index -> hex SHA-384
    cabundle: tuple[str, ...]  # cert chain markers; last must be the AWS root fp
    certificate: str  # leaf cert marker (mock)
    signature: str  # hex HMAC (mock) / COSE signature (prod)
    public_key: str = ""  # enclave-provided key (e.g. for KMS), optional
    user_data: str = ""
    nonce: str = ""

    def canonical_payload(self) -> bytes:
        """Deterministic bytes the signature is computed over (excludes it)."""
        body = {
            "module_id": self.module_id,
            "timestamp_ms": self.timestamp_ms,
            "digest": self.digest,
            "pcrs": {str(k): self.pcrs[k] for k in sorted(self.pcrs)},
            "cabundle": list(self.cabundle),
            "certificate": self.certificate,
            "public_key": self.public_key,
            "user_data": self.user_data,
            "nonce": self.nonce,
        }
        return json.dumps(body, separators=(",", ":"), sort_keys=True).encode("utf-8")

    @classmethod
    def from_mapping(cls, m: Mapping) -> "AttestationDocument":
        try:
            pcrs = {int(k): str(v) for k, v in m["pcrs"].items()}
            return cls(
                module_id=str(m["module_id"]),
                timestamp_ms=int(m["timestamp_ms"]),
                digest=str(m.get("digest", "SHA384")),
                pcrs=pcrs,
                cabundle=tuple(str(c) for c in m["cabundle"]),
                certificate=str(m["certificate"]),
                signature=str(m["signature"]),
                public_key=str(m.get("public_key", "")),
                user_data=str(m.get("user_data", "")),
                nonce=str(m.get("nonce", "")),
            )
        except (KeyError, TypeError, ValueError) as exc:
            raise AttestationError("malformed", f"bad attestation structure: {exc}")


@dataclass(frozen=True)
class VerifiedAttestation:
    module_id: str
    pcr0: str
    public_key: str
    timestamp_ms: int


# A SignatureCheck returns True iff the document's signature is valid. The mock
# uses HMAC; production swaps in COSE_Sign1 ES384 verification against the leaf
# cert's public key. Signature: (doc) -> bool.
SignatureCheck = Callable[[AttestationDocument], bool]


def hmac_signature_check(key: bytes) -> SignatureCheck:
    """Mock signature check: HMAC-SHA256 over the canonical payload.

    Stand-in for real COSE_Sign1/ES384 verification so the policy is testable
    offline. Constant-time compare, like the App Check verifier.
    """

    def _check(doc: AttestationDocument) -> bool:
        expected = hmac.new(key, doc.canonical_payload(), hashlib.sha256).hexdigest()
        return hmac.compare_digest(expected, doc.signature)

    return _check


def mock_sign(doc_fields: Mapping, key: bytes) -> str:
    """Produce a valid mock signature for a document (test/enclave-side helper)."""
    doc = AttestationDocument.from_mapping({**doc_fields, "signature": ""})
    return hmac.new(key, doc.canonical_payload(), hashlib.sha256).hexdigest()


class AttestationVerifier:
    """Verifies a Nitro attestation document against a pinned code identity.

    The checks, in order (each raises AttestationError with a distinct code):
      malformed -> untrusted_root -> bad_signature -> wrong_module ->
      pcr_mismatch -> nonce_mismatch -> stale.
    """

    def __init__(
        self,
        expected: ExpectedEnclave,
        signature_check: SignatureCheck,
        *,
        max_age_secs: int = DEFAULT_MAX_AGE_SECS,
        clock_skew_secs: int = DEFAULT_CLOCK_SKEW_SECS,
    ) -> None:
        self._expected = expected
        self._sig_check = signature_check
        self._max_age_secs = max_age_secs
        self._clock_skew_secs = clock_skew_secs

    def verify(
        self, doc: AttestationDocument, *, nonce: str, now_ms: int
    ) -> VerifiedAttestation:
        exp = self._expected

        # 1) Root of trust: leaf cert must chain to the AWS Nitro root CA. (Real:
        #    x509 path validation leaf -> cabundle -> pinned AWS root.)
        if not doc.cabundle or doc.cabundle[-1] != exp.root_ca_fingerprint:
            raise AttestationError("untrusted_root", "cert chain does not reach the AWS Nitro root")
        if not doc.certificate:
            raise AttestationError("untrusted_root", "missing leaf certificate")

        # 2) Signature: proves the document was produced by that leaf cert (the
        #    NSM), i.e. it wasn't forged or tampered with in transit.
        if not self._sig_check(doc):
            raise AttestationError("bad_signature", "attestation signature invalid")

        # 3) Module identity (cheap sanity check before PCRs).
        if not doc.module_id.startswith(exp.module_id_prefix):
            raise AttestationError("wrong_module", f"unexpected module_id {doc.module_id!r}")

        # 4) Code identity: PCR0 (and any other pinned PCRs) must match exactly.
        #    This is the crux — it proves the *expected code* is running inside.
        self._check_pcr(doc, PCR0, exp.pcr0)
        for idx, want in ((1, exp.pcr1), (2, exp.pcr2), (8, exp.pcr8)):
            if want is not None:
                self._check_pcr(doc, idx, want)

        # 5) Freshness against replay: the nonce we challenged with must come back.
        if not nonce or not hmac.compare_digest(doc.nonce, nonce):
            raise AttestationError("nonce_mismatch", "attestation nonce does not match challenge")

        # 6) Freshness against staleness/clock games.
        age_ms = now_ms - doc.timestamp_ms
        if age_ms > self._max_age_secs * 1000:
            raise AttestationError("stale", f"attestation is {age_ms // 1000}s old")
        if age_ms < -self._clock_skew_secs * 1000:
            raise AttestationError("stale", "attestation timestamp is in the future")

        return VerifiedAttestation(
            module_id=doc.module_id,
            pcr0=doc.pcrs[PCR0],
            public_key=doc.public_key,
            timestamp_ms=doc.timestamp_ms,
        )

    def _check_pcr(self, doc: AttestationDocument, index: int, expected_hex: str) -> None:
        got = doc.pcrs.get(index)
        if got is None:
            raise AttestationError("pcr_mismatch", f"PCR{index} absent")
        if not hmac.compare_digest(got.lower(), expected_hex.lower()):
            raise AttestationError("pcr_mismatch", f"PCR{index} does not match expected code identity")
