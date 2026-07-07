"""Enclave-backed Claude client — forwards summaries into a Nitro Enclave.

This is a `ClaudeClient` (same Protocol as AnthropicClaudeClient) that, instead
of calling Claude from the proxy, forwards the structured summary over VSOCK to
an enclave that holds the API key and makes the outbound call. Before trusting
the enclave it performs an attestation handshake and verifies the document
(see enclave.py). Result: the parent host never sees the API key.

WHAT'S REAL vs WHAT NEEDS AWS:
  * Real & unit-tested: the handshake sequencing (fresh nonce -> attest ->
    verify -> forward) and that a failed attestation blocks forwarding.
  * Needs AWS: the actual VSOCK transport (`socket.AF_VSOCK`, Linux + Nitro
    only) and a genuine attestation from the NSM. `VsockTransport` documents the
    real socket usage; tests use `MockVsockTransport`.
"""

from __future__ import annotations

import asyncio
import json
import secrets
import time
from typing import Any, Callable, Protocol

from .enclave import AttestationDocument, AttestationVerifier, VerifiedAttestation


class Transport(Protocol):
    """Synchronous request/response to the enclave (one JSON object each way)."""

    def send(self, message: dict) -> dict: ...


def _now_ms() -> int:
    return int(time.time() * 1000)


class EnclaveClaudeClient:
    """ClaudeClient that attests, then forwards the summary into the enclave.

    Re-attests at most every `reattest_interval_ms` (an enclave's identity can't
    change without a restart, which would change PCR0 and fail the next attest).
    """

    def __init__(
        self,
        transport: Transport,
        verifier: AttestationVerifier,
        *,
        reattest_interval_ms: int = 300_000,
        nonce_source: Callable[[], str] = lambda: secrets.token_hex(16),
        now_ms: Callable[[], int] = _now_ms,
    ) -> None:
        self._transport = transport
        self._verifier = verifier
        self._reattest_interval_ms = reattest_interval_ms
        self._nonce_source = nonce_source
        self._now_ms = now_ms
        self._verified_at_ms: int | None = None
        self._verified: VerifiedAttestation | None = None

    def _attest(self) -> VerifiedAttestation:
        """Challenge the enclave and verify its attestation document."""
        nonce = self._nonce_source()
        resp = self._transport.send({"op": "attest", "nonce": nonce})
        doc = AttestationDocument.from_mapping(resp)
        verified = self._verifier.verify(doc, nonce=nonce, now_ms=self._now_ms())
        self._verified = verified
        self._verified_at_ms = self._now_ms()
        return verified

    def _ensure_attested(self) -> VerifiedAttestation:
        fresh = (
            self._verified is not None
            and self._verified_at_ms is not None
            and (self._now_ms() - self._verified_at_ms) < self._reattest_interval_ms
        )
        if fresh:
            return self._verified  # type: ignore[return-value]
        return self._attest()

    def _complete_sync(self, payload: Any) -> dict:
        # Attestation MUST pass before any summary leaves the proxy.
        self._ensure_attested()
        resp = self._transport.send({"op": "complete", "summary": payload})
        if "result" not in resp:
            raise RuntimeError(f"enclave returned no result: {resp!r}")
        return resp["result"]

    async def complete(self, payload: Any) -> dict:
        # Transport is blocking (a socket); keep the event loop free.
        return await asyncio.to_thread(self._complete_sync, payload)


class MockVsockTransport:
    """In-process stand-in for the enclave, for tests. Dispatches `op` to a stub
    that produces (attestation doc | claude result). No real socket."""

    def __init__(self, enclave_stub: Callable[[dict], dict]) -> None:
        self._stub = enclave_stub
        self.calls: list[dict] = []

    def send(self, message: dict) -> dict:
        self.calls.append(message)
        return self._stub(message)


class VsockTransport:
    """Real VSOCK transport to the enclave (Linux + Nitro only).

    Length-prefixed JSON over an AF_VSOCK stream. `AF_VSOCK` only exists on Linux
    with the Nitro hypervisor, so the symbol is resolved lazily — importing this
    module on macOS/CI is fine; only `send()` touches the socket. `cid` is the
    enclave's VSOCK context id (from `nitro-cli describe-enclaves`), `port` the
    agreed listening port inside the enclave.
    """

    def __init__(self, cid: int, port: int, *, timeout_secs: float = 30.0) -> None:
        self._cid = cid
        self._port = port
        self._timeout = timeout_secs

    def send(self, message: dict) -> dict:  # pragma: no cover - needs Nitro host
        import socket  # local import; AF_VSOCK is Linux/Nitro-only

        af_vsock = getattr(socket, "AF_VSOCK", None)
        if af_vsock is None:
            raise RuntimeError(
                "AF_VSOCK unavailable: VsockTransport requires a Linux Nitro host. "
                "Use MockVsockTransport off-Nitro."
            )
        with socket.socket(af_vsock, socket.SOCK_STREAM) as s:
            s.settimeout(self._timeout)
            s.connect((self._cid, self._port))
            body = json.dumps(message).encode("utf-8")
            s.sendall(len(body).to_bytes(4, "big") + body)
            header = _recv_exact(s, 4)
            length = int.from_bytes(header, "big")
            return json.loads(_recv_exact(s, length).decode("utf-8"))


def _recv_exact(sock, n: int) -> bytes:  # pragma: no cover - needs Nitro host
    chunks = bytearray()
    while len(chunks) < n:
        chunk = sock.recv(n - len(chunks))
        if not chunk:
            raise RuntimeError("enclave closed the VSOCK connection early")
        chunks.extend(chunk)
    return bytes(chunks)
