#!/usr/bin/env bash
# One-time: copy the deck from Downloads into public so the site can embed it.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${1:-$HOME/Downloads/The Ethical Researcher's Guide to AI (1).pptx}"
DEST="$ROOT/public/guides/ethical-researcher-guide-ai-2026.pptx"
mkdir -p "$ROOT/public/guides"
if [[ ! -f "$SRC" ]]; then
  echo "Source not found: $SRC"
  echo "Usage: $0 [path-to-pptx]"
  exit 1
fi
cp "$SRC" "$DEST"
ls -la "$DEST"
