#!/usr/bin/env bash
# Gaply first-run Ollama setup — FUTURE SLM deployment path.
#
# HONEST SCOPE: no real Ollama model is wired into gaply_core today. All
# ML-adjacent work ships with documented interim proxies (HeuristicModel for
# perplexity, HashEmbedder for embeddings). This script prepares the machine
# for the day real local inference lands (Qwen2.5-7B / Qwen3-4B once trained,
# all-MiniLM-L6-v2 to replace HashEmbedder, a real perplexity model to replace
# HeuristicModel). Until then it is safe to run: it only installs/configures
# Ollama and pre-pulls models; the app does not call them yet.
#
# CRITICAL env pair (why both): Ollama ships with Flash Attention OFF by
# default, and KV-cache quantization (OLLAMA_KV_CACHE_TYPE) only takes effect
# when Flash Attention is enabled. Setting q8_0 without FLASH_ATTENTION=1
# silently does nothing — always set BOTH:
#   OLLAMA_FLASH_ATTENTION=1
#   OLLAMA_KV_CACHE_TYPE=q8_0
#
# Usage:
#   ./scripts/ollama-first-run.sh            # full first-run setup
#   ./scripts/ollama-first-run.sh --dry-run  # print what WOULD happen (CI-testable)
set -euo pipefail

DRY_RUN=0
[ "${1:-}" = "--dry-run" ] && DRY_RUN=1

say() { echo "[gaply-setup] $*"; }
run() {
  if [ "$DRY_RUN" = 1 ]; then say "DRY-RUN would execute: $*"; else "$@"; fi
}

# --- the CRITICAL env pair (exported for this session; persisted below) ------
export OLLAMA_FLASH_ATTENTION=1
export OLLAMA_KV_CACHE_TYPE=q8_0
say "env: OLLAMA_FLASH_ATTENTION=${OLLAMA_FLASH_ATTENTION}"
say "env: OLLAMA_KV_CACHE_TYPE=${OLLAMA_KV_CACHE_TYPE}"
say "(KV-cache quantization only takes effect with Flash Attention enabled;"
say " Ollama ships with Flash Attention off by default — both must be set.)"

# --- 1. detect Ollama ---------------------------------------------------------
if command -v ollama >/dev/null 2>&1; then
  say "Ollama found: $(command -v ollama)"
else
  say "Ollama is NOT installed."
  case "$(uname -s)" in
    Darwin) say "Install: https://ollama.com/download/mac  (or: brew install ollama)" ;;
    Linux)  say "Install: curl -fsSL https://ollama.com/install.sh | sh" ;;
    *)      say "Install: https://ollama.com/download/windows" ;;
  esac
  if [ "$DRY_RUN" = 1 ]; then
    say "DRY-RUN: continuing as if Ollama were installed."
  else
    say "Re-run this script after installing Ollama."
    exit 0
  fi
fi

# --- 2. persist the env pair where the Ollama server will see it --------------
case "$(uname -s)" in
  Darwin)
    run launchctl setenv OLLAMA_FLASH_ATTENTION 1
    run launchctl setenv OLLAMA_KV_CACHE_TYPE q8_0
    say "persisted via launchctl setenv (restart the Ollama app to pick up)"
    ;;
  Linux)
    say "persist by adding to the ollama systemd unit (systemctl edit ollama):"
    say '  [Service]'
    say '  Environment="OLLAMA_FLASH_ATTENTION=1"'
    say '  Environment="OLLAMA_KV_CACHE_TYPE=q8_0"'
    ;;
  *)
    say "persist on Windows (PowerShell):"
    say '  [Environment]::SetEnvironmentVariable("OLLAMA_FLASH_ATTENTION","1","User")'
    say '  [Environment]::SetEnvironmentVariable("OLLAMA_KV_CACHE_TYPE","q8_0","User")'
    ;;
esac

# --- 3. RAM-aware model guidance (future SLM path) -----------------------------
free_gb=0
case "$(uname -s)" in
  Darwin) total_bytes=$(sysctl -n hw.memsize 2>/dev/null || echo 0); free_gb=$((total_bytes / 1073741824)) ;;
  Linux)  free_kb=$(awk '/MemAvailable/ {print $2}' /proc/meminfo 2>/dev/null || echo 0); free_gb=$((free_kb / 1048576)) ;;
  *)      free_gb=0 ;;
esac
say "detected ~${free_gb} GB RAM (macOS reports total; Linux reports available)"
if [ "$free_gb" -ge 16 ]; then
  say "RAM guidance: >=16 GB free -> Qwen2.5-7B recommended when SLM support lands"
  FUTURE_SLM="qwen2.5:7b"
else
  say "RAM guidance: <16 GB free -> Qwen3-4B recommended when SLM support lands"
  FUTURE_SLM="qwen3:4b"
fi

# --- 4. pull interim/support models --------------------------------------------
# all-MiniLM-L6-v2 (embeddings): the model HashEmbedder is shaped after (384-dim)
# — pulled now so the swap-in is instant when the Embedder seam gets a real impl.
run ollama pull all-minilm
# NOTE: GPT-2 (the perplexity reference model for the HeuristicModel seam) is not
# in the Ollama library; when that seam is wired for real it will ship via
# candle/gguf or a custom Ollama Modelfile — documented in docs/PACKAGING.md.
say "future SLM (NOT pulled automatically; large download): ollama pull ${FUTURE_SLM}"

say "done. Today the app runs fully offline on interim proxies; models pulled"
say "here are staged for the future SLM integration, not used by the app yet."
