#!/usr/bin/env bash
# Test: the first-run script sets the documented Ollama env pair correctly.
# Runs the script in --dry-run (no installs, no pulls, no persistence) and
# asserts the CRITICAL pair is exported with the exact required values.
set -euo pipefail
cd "$(dirname "$0")/.."

out=$(bash scripts/ollama-first-run.sh --dry-run 2>&1)

fail() { echo "FAIL: $1"; echo "---- output ----"; echo "$out"; exit 1; }

echo "$out" | grep -q '^\[gaply-setup\] env: OLLAMA_FLASH_ATTENTION=1$' \
  || fail "OLLAMA_FLASH_ATTENTION=1 not set"
echo "$out" | grep -q '^\[gaply-setup\] env: OLLAMA_KV_CACHE_TYPE=q8_0$' \
  || fail "OLLAMA_KV_CACHE_TYPE=q8_0 not set"
# the pairing rationale must be surfaced to the user, not buried
echo "$out" | grep -qi 'flash attention' || fail "missing Flash Attention rationale"
# dry-run must not actually pull models
echo "$out" | grep -q 'DRY-RUN would execute: ollama pull all-minilm' \
  || fail "dry-run should plan (not run) the all-minilm pull"
if echo "$out" | grep -q 'pulling manifest'; then fail "dry-run performed a real pull"; fi
# RAM-aware guidance present (either branch)
echo "$out" | grep -qE 'Qwen(2\.5-7B|3-4B) recommended' || fail "missing RAM-aware guidance"

echo "PASS: ollama-first-run --dry-run sets OLLAMA_FLASH_ATTENTION=1 + OLLAMA_KV_CACHE_TYPE=q8_0"
