#!/usr/bin/env bash
# Local (loopback) gaply-proxy launch. Reads secrets from ../.env (gitignored):
#   APP_CHECK_SIGNING_KEY (shared with the Rust client's keychain)
#   ANTHROPIC_API_KEY     (the real Claude key — never in any client)
# Enclave OFF for now (follow-up). Binds 127.0.0.1:8080 (bind_guard enforces private).
set -euo pipefail
cd "$(dirname "$0")/.."
set -a; source .env; set +a
export GAPLY_ENCLAVE_ENABLED=0
echo "[deploy] launching gaply-proxy on ${GAPLY_BIND_HOST:-127.0.0.1}:${GAPLY_BIND_PORT:-8080} (enclave off)"
exec python3 -m app
