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

## Run

```bash
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
export CLAUDE_API_KEY=sk-ant-...          # server-side only
export APP_CHECK_SIGNING_KEY=...          # shared with the Rust client
uvicorn app.main:app --port 8080
```

## Test

```bash
pip install -r requirements.txt
pytest -q
```

Tests use a stub Claude client (no network, no key) and mint App Check tokens
with a test key, so the whole suite runs offline.
