#!/usr/bin/env bash
# GRRB pre-commit gate — Phase 2b, §6c.3.
#
# Runs the benchmark and classifies the diff against the committed baseline when
# a staged change touches code the benchmark scores. PROMOTE passes; HOLD and
# ROLLBACK block the commit and print why.
#
# PROMPT 4 SAID "the agents/ or harness/ paths, the way the --workspace gate
# already guards those paths". NEITHER EXISTS. There is no pre-commit hook in
# this repo (.git/hooks holds only samples), and there are no `agents/` or
# `harness/` directories — the agent code is gaply-core/src/{specialist,swarm}/,
# *_agent.rs and agent_graph.rs, and the harness is src-tauri/src/ai/ and
# src/bin/. The list below is derived from what the benchmark actually calls,
# which is the only defensible reading: a change to a scored engine must be
# gated whatever directory it lives in.
#
# Skip deliberately with GRRB_SKIP=1 or `git commit --no-verify`.
set -u

[ "${GRRB_SKIP:-0}" = "1" ] && exit 0

REPO="$(git rev-parse --show-toplevel)"
BASELINE="$REPO/src-tauri/evals/reports/grrb-baseline.json"

# The engines the benchmark scores, plus the agent and harness code Prompt 4
# named. Anything here changes what a run measures.
WATCHED='^src-tauri/(gaply-core/src/(equation/|validate\.rs|consistency\.rs|sanitize\.rs|journal_extract\.rs|extract/citations\.rs|specialist/|swarm/|swarm\.rs|agent_graph\.rs|[a-z_]*_agent\.rs)|src/ai/|src/bin/|evals/grrb/)'

STAGED="$(git diff --cached --name-only --diff-filter=ACMR)"
HITS="$(printf '%s\n' "$STAGED" | grep -E "$WATCHED" || true)"
[ -z "$HITS" ] && exit 0

echo "GRRB gate: staged changes touch scored code —"
printf '  %s\n' $HITS

if [ ! -f "$BASELINE" ]; then
  echo "  no committed baseline at ${BASELINE#"$REPO"/} — nothing to compare against."
  echo "  Generate one with: (cd src-tauri && cargo run --features devtools --bin grrb -- evals/reports/grrb-baseline.json)"
  exit 1
fi

cd "$REPO/src-tauri" || exit 1
BUILD_LOG="$(mktemp)"
# The benchmark binaries are DEV-ONLY (`required-features = ["devtools"]`), so
# they are not built into a production bundle. Development is unchanged; the
# feature flag is the only difference, and what the benchmark measures is not
# touched. See the [[bin]] block in src-tauri/Cargo.toml.
if ! cargo build --quiet --features devtools --bin grrb --bin grrb-gate > "$BUILD_LOG" 2>&1; then
  echo "  the benchmark does not build — the gate cannot judge a tree that fails to compile:"
  sed 's/^/    /' "$BUILD_LOG" | head -20
  exit 1
fi

CAND="$(mktemp -t grrb-candidate).json"
if ! ./target/debug/grrb "$CAND" > /dev/null 2>&1; then
  echo "  the benchmark run failed. That is itself a regression."
  exit 1
fi

./target/debug/grrb-gate "$BASELINE" "$CAND"
VERDICT=$?
# 0 PROMOTE and 5 HOLD-at-baseline both pass: neither is a regression. Only a
# score that got WORSE blocks a commit (1 ROLLBACK, 4 HOLD-with-regression), and
# 3 BLOCKED means the case set moved so no comparison was possible.
case "$VERDICT" in
  0|5) exit 0 ;;
esac
if [ "$VERDICT" -ne 0 ]; then
  echo
  echo "  Commit blocked. Either fix the regression, or — if the change is"
  echo "  deliberate and the new numbers are the new truth — re-baseline with"
  echo "  (cd src-tauri && cargo run --features devtools --bin grrb -- evals/reports/grrb-baseline.json)"
  echo "  and say so in the commit message. GRRB_SKIP=1 skips this gate."
fi
exit "$VERDICT"
