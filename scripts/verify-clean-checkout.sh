#!/usr/bin/env bash
# Build and test what is COMMITTED, not what is in the working tree.
#
# WHY THIS EXISTS
#
# `main` did not compile for weeks and every test run was green. Two files that
# committed code referenced — `gaply-core/src/scientific_model.rs` (declared by
# df4eef3) and `sentences_in` in `extract/sentence.rs` (called by audit_prepass
# and chunk.rs) — existed only in the working tree. Local `cargo test
# --workspace` compiles the tree, so it saw them; a fresh clone would not have.
# The tests were real. The tree they ran in was not the tree that ships.
#
# The only clean-checkout check in the repo is the Windows workflow, and it is
# `workflow_dispatch:` — manual dispatch only, no push or PR trigger — so in
# practice it never ran. Nothing was watching.
#
# WHAT IT DOES
#
# Checks out a ref into a throwaway detached worktree, builds and tests
# gaply-core there, and deletes it. The worktree shares the object store, so it
# costs a checkout rather than a clone.
#
#   scripts/verify-clean-checkout.sh              # HEAD
#   scripts/verify-clean-checkout.sh main
#   KEEP=1 scripts/verify-clean-checkout.sh       # leave the worktree for poking
#
# gaply-core is the crate this guards: it is the portable, dependency-light one,
# and it must build from source alone. The APP crate is checked too, but only
# when the bundled model resource its build script requires is available — that
# file is ~400 MB and correctly gitignored, so its absence is an environment
# fact rather than a code defect, and the script says which case it hit instead
# of failing ambiguously.
set -euo pipefail

REF="${1:-HEAD}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# NEITHER OF THESE MAY LIVE IN $TMPDIR (§11 D79).
#
# macOS periodically cleans /var/folders/**/T, and it does so PARTIALLY: cargo's
# fingerprints survive while a build script's generated output does not. The
# result is a guard that fails with
#
#   error: couldn't read .../libsqlite3-sys-*/out/bindgen.rs: No such file
#
# which reads as a code defect in a dependency and is nothing of the kind. That
# is the third time in one session that temp-cleaning produced a false signal
# here — after ~/gaply-dev.log and the eval logs — and it is the worst of the
# three, because a GUARD that looks flaky gets ignored, and a guard that gets
# ignored is not a guard.
#
# $HOME is the same reasoning CLAUDE.md already applies to the dev log: keep the
# thing you need after something went wrong outside the directories that are
# cleaned exactly when something goes wrong.
CACHE_ROOT="${GAPLY_CLEAN_ROOT:-$HOME/.cache/gaply}"
mkdir -p "$CACHE_ROOT"
WORKTREE="$(mktemp -d "$CACHE_ROOT/clean-XXXXXX")"
TARGET_DIR="${GAPLY_CLEAN_TARGET:-$CACHE_ROOT/clean-target}"

cleanup() {
  if [[ -n "${KEEP:-}" ]]; then
    echo "KEEP set — worktree left at $WORKTREE"
    return
  fi
  git -C "$REPO_ROOT" worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
  rm -rf "$WORKTREE"
}
trap cleanup EXIT

SHA="$(git -C "$REPO_ROOT" rev-parse --short "$REF")"
echo "==> verifying the COMMITTED tree at $REF ($SHA)"

# Detached, so the ref can be one that is already checked out here.
rm -rf "$WORKTREE"
git -C "$REPO_ROOT" worktree add --detach "$WORKTREE" "$REF" >/dev/null

# Anything the working tree has and the commit does not is exactly what this
# guard exists to find, so report it rather than silently proceeding.
UNTRACKED="$(git -C "$REPO_ROOT" ls-files --others --exclude-standard -- 'src-tauri/**/*.rs' | head -20)"
if [[ -n "$UNTRACKED" ]]; then
  echo "    note: these .rs files are UNCOMMITTED in your working tree —"
  echo "          if the build below fails, one of them is probably why:"
  echo "$UNTRACKED" | sed 's/^/            /'
fi

cd "$WORKTREE/src-tauri"

echo "==> cargo check -p gaply_core --all-targets"
CARGO_TARGET_DIR="$TARGET_DIR" cargo check -p gaply_core --all-targets

echo "==> cargo test -p gaply_core"
CARGO_TARGET_DIR="$TARGET_DIR" cargo test -p gaply_core

# The app crate's build script requires a bundled model that is not in git.
BUNDLED="$REPO_ROOT/src-tauri/bundled-models"
if [[ -d "$BUNDLED" ]] && compgen -G "$BUNDLED/*.gguf" >/dev/null; then
  echo "==> cargo check --workspace --all-targets (bundled models linked)"
  # Link the FILES, not the directory: `bundled-models/` is itself tracked (it
  # holds a README), so symlinking the directory onto an existing one nests the
  # link inside it and the build script still reports the resource missing.
  # Every untracked file in there, not just the .gguf: tauri.conf.json also
  # lists `bundled-models/tokenizer.json`, which is likewise gitignored.
  mkdir -p "$WORKTREE/src-tauri/bundled-models"
  for f in "$BUNDLED"/*; do
    dest="$WORKTREE/src-tauri/bundled-models/$(basename "$f")"
    [[ -e "$dest" ]] || ln -sf "$f" "$dest"
  done
  CARGO_TARGET_DIR="$TARGET_DIR" cargo check --workspace --all-targets
else
  echo "==> SKIPPED the app crate: src-tauri/bundled-models/*.gguf is absent."
  echo "    That resource is gitignored by design (~400 MB), so this is an"
  echo "    environment fact, not a code defect. gaply-core — the crate that"
  echo "    must build from source alone — was checked above."
fi

echo "==> clean checkout at $SHA is GOOD"
