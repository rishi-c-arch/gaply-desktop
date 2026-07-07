# gaply-proxy

A tiny, standalone FastAPI service — **separate from the Tauri/Rust core**. Its
only job: receive a **structured summary** (JSON, never raw manuscript text)
from the Rust core, verify the request, attach the server-side Claude key, and
forward to **Claude Sonnet** (`claude-sonnet-5`). Nothing is persisted.

## Security layers

1. **Rate limiter** (FastAPI middleware, keyed by a client identifier) → `429`
   when exhausted. Client id = `X-Client-Id` header, falling back to the peer IP.
   Port of the Prompt 9 token-bucket limiter.
2. **App Check** (dependency on `/verify`) → `401` without a valid
   `X-Firebase-AppCheck` token. Verifies HMAC-signed tokens minted by the Rust
   core's `TokenSigner` (Prompt 12), including limited-use replay protection.
3. **Hard validator** → `422` for anything that looks like raw prose / a raw
   manuscript rather than a structured summary (length + prose heuristics).

`/health` is exempt from all three (liveness only, no auth).

## Endpoints

- `POST /verify` — body `{"summary": {...}, "instruction": "...", "max_tokens": N}`.
  Validates, forwards the structured summary to Claude Sonnet, returns
  `{"result": {"model", "stop_reason", "text"}}`. No payload is stored.
- `GET /health` — `{"status": "ok"}`, no auth.

## Private by default — invisible to the public internet (Tailscale)

This proxy is meant to be reachable **only over a Tailscale tailnet**, never from
the public internet. There are two independent layers:

1. **Bind safety net (in-process, always on).** The proxy refuses to bind to a
   public interface. On launch it classifies `GAPLY_BIND_HOST` and **fails
   closed** (exit 2) unless it is loopback, a Tailscale tailnet address
   (`100.64.0.0/10`), or another private/ULA address. `0.0.0.0` / `::`
   ("all interfaces" — the classic mistake that opens a public port) is always
   rejected. See `app/bind_guard.py`. Default bind is `127.0.0.1:8080`.

2. **Exposure via Tailscale Serve (how clients reach it).** The port is
   published onto the tailnet with automatic HTTPS and **zero open public
   ports**; who can reach it is governed by your tailnet ACLs.

### Setup on the proxy host

```bash
# 1. Install Tailscale (see https://tailscale.com/download) and bring the node up.
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up                       # authenticate this node to your tailnet
tailscale ip -4                         # -> this node's 100.x tailnet address

# 2. Run the proxy bound to loopback (the default; the safety net enforces this).
export CLAUDE_API_KEY=sk-ant-...         # server-side only
export APP_CHECK_SIGNING_KEY=...         # shared with the Rust client
python -m app                            # authoritative launch: bind check -> uvicorn

# 3. Publish it onto the tailnet over HTTPS with NO public ports opened.
sudo tailscale serve --bg 8080           # tailnet-only HTTPS -> 127.0.0.1:8080
tailscale serve status                   # shows the https://<node>.<tailnet>.ts.net URL
```

Clients (e.g. the Rust core on a tailnet node) then call
`https://<node>.<tailnet>.ts.net/verify`. Nothing is exposed publicly; access is
whatever your tailnet ACLs allow. (Use `tailscale serve` for tailnet-only; only
`tailscale funnel` would expose it publicly — which this design deliberately does
not use.)

> **Always launch with `python -m app`, not `uvicorn app.main:app --host 0.0.0.0`.**
> The `-m app` entrypoint decides the bind host from `GAPLY_BIND_HOST` and runs
> the safety net against the address it actually binds. A `--host` passed
> straight to `uvicorn` on the command line bypasses the env-based check (though
> `create_app()` still refuses a public `GAPLY_BIND_HOST` at import time).

### Fail-closed startup (systemd)

`deploy/gaply-proxy.service` + `deploy/require-tailscale.sh` ensure the proxy
**will not start unless the tailnet is up and this node has a 100.x IP**. The
`ExecStartPre=require-tailscale.sh` gate exits non-zero (aborting the start) when
`tailscale status` isn't `Running` or no tailnet IP is assigned. Install:

```bash
sudo cp deploy/require-tailscale.sh /usr/local/bin/ && sudo chmod +x /usr/local/bin/require-tailscale.sh
sudo cp deploy/gaply-proxy.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now gaply-proxy
```

### What is verified now vs. what needs a real tailnet

Honestly scoped, in the same spirit as the Windows CI check that needed real
GitHub infrastructure to confirm end-to-end:

| Concern | Status |
|---|---|
| Bind guard ACCEPTS localhost / `100.64.0.0/10` / private, REJECTS `0.0.0.0` & public IPs | **Verified now** — `tests/test_bind_guard.py` (unit) + `create_app`/`python -m app` refuse a public bind (exit 2) |
| `create_app()` won't construct with a public `GAPLY_BIND_HOST` | **Verified now** — unit test |
| systemd gate script logic (exit non-zero when tailnet down) | **Logic written & `bash -n` clean**; exercised end-to-end only on a host with `tailscaled` |
| Tailscale actually routes traffic tailnet-only; `serve` gives HTTPS + zero public ports; ACLs enforce access | **Requires a live Tailscale network** — cannot be confirmed offline (no tailnet in this repo/CI) |

## Optional hardening — Trusted Execution Environment (AWS Nitro Enclaves)

Tailscale hides the proxy from the network, but the **host still sees the API key
and the summaries**. For a stronger boundary, sensitive processing can run inside
an **AWS Nitro Enclave**: the parent instance has zero visibility into enclave
memory, the enclave holds the key and makes the Claude call, and the proxy only
trusts it after verifying a cryptographic **attestation** of the code identity
(PCR0). Communication is VSOCK-only.

- **Designed & unit-tested now:** the attestation *verification policy*
  (`app/enclave.py`) and the attest→verify→forward handshake
  (`app/enclave_client.py`), with mocked crypto — ACCEPT a valid document, REJECT
  tampered PCR0 / signature / payload / root / nonce / stale ones
  (`tests/test_enclave.py`).
- **Requires real AWS to deploy/verify:** building the enclave image (EIF→PCR0),
  a genuine NSM attestation, real COSE_Sign1/ES384 + AWS Nitro root-CA
  validation, KMS attestation-gated key release, and VSOCK transport. Full
  guide + build steps: [`deploy/enclave/README.md`](deploy/enclave/README.md).

Enable with `GAPLY_ENCLAVE_ENABLED=yes` and pin `GAPLY_ENCLAVE_PCR0`. Until a
provisioned enclave client is injected, the proxy **fails closed** (`503
enclave_not_provisioned`) rather than silently doing a host-side call. See also
`deploy/enclave/README.md` for **on-device** alternatives (Apple Secure Enclave,
future macOS pass) and why **Intel SGX** (EPC limits, side-channel history) is
not the primary recommendation.

## Environment variables (keys are never hardcoded)

| Var | Purpose |
|---|---|
| `CLAUDE_API_KEY` (or `ANTHROPIC_API_KEY`) | server-side Claude key |
| `CLAUDE_MODEL` | default `claude-sonnet-5` |
| `APP_CHECK_SIGNING_KEY` | shared HMAC secret (must match the Rust client key) |
| `APP_CHECK_APP_ID` | default `ai.gaply.app` |
| `APP_CHECK_DEBUG_TOKENS` | comma-separated dev/CI bypass tokens (never in prod) |
| `RATE_LIMIT_CAPACITY` / `RATE_LIMIT_REFILL_RATE` | bucket size / tokens per sec |
| `MAX_TOTAL_CHARS` / `MAX_FIELD_CHARS` / `MAX_SENTENCES_PER_FIELD` | validator thresholds |
| `GAPLY_BIND_HOST` / `GAPLY_BIND_PORT` | bind address (default `127.0.0.1:8080`); must be localhost or private/tailnet |
| `GAPLY_ALLOW_PUBLIC_BIND` | break-glass ONLY; must equal `i-accept-public-exposure` to disable the public-bind safety net |
| `GAPLY_ENCLAVE_ENABLED` | opt into the Nitro Enclave TEE path (`1`/`true`/`yes`); needs real AWS Nitro infra |
| `GAPLY_ENCLAVE_PCR0` | pinned hex SHA-384 of the enclave image (code identity to trust) |
| `GAPLY_ENCLAVE_CID` / `GAPLY_ENCLAVE_PORT` | enclave VSOCK context id / port (default `16` / `5005`) |
| `GAPLY_ENCLAVE_MAX_AGE` | max attestation-document age in seconds (default `300`) |

## Run

```bash
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
export CLAUDE_API_KEY=sk-ant-...          # server-side only
export APP_CHECK_SIGNING_KEY=...          # shared with the Rust client
python -m app                             # fail-closed launch (bind check -> uvicorn)
# then publish tailnet-only:  sudo tailscale serve --bg 8080
```

## Test

```bash
pip install -r requirements.txt
pytest -q
```

Tests use a stub Claude client (no network, no key) and mint App Check tokens
with a test key, so the whole suite runs offline.
