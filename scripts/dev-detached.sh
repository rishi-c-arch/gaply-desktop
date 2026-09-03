#!/usr/bin/env bash
# Launch the Gaply dev app so it OUTLIVES the terminal (or agent session) that
# started it.
#
# WHY THIS EXISTS
#
# A citation audit is ~20 minutes of model time. Four separate runs were lost
# mid-audit when the session that launched the app ended and took it down, each
# time throwing away work that cannot be resumed cheaply.
#
# The naive `npm run tauri dev &` dies with its shell. `nohup ... &` survives a
# SIGHUP but stays in the shell's process GROUP, so anything that kills the
# group — which is how most harnesses and terminal apps clean up — still reaches
# it. This double-forks and calls setsid, so the process ends up owned by
# launchd (PPID 1) in its own session, with no controlling terminal and no
# relationship to the shell that started it.
#
# The log goes to $HOME, deliberately NOT to a temp or session directory: those
# get cleaned at exactly the moment you most want the log, which is after
# something disappeared.
#
#   scripts/dev-detached.sh            # release (correct for anything with a model)
#   scripts/dev-detached.sh --debug    # debug build, if you truly want it
#   tail -f ~/gaply-dev.log            # watch it
#   scripts/dev-detached.sh --stop     # stop it
#
# Release is the DEFAULT here on purpose. `tauri dev` without `--release` is
# ~68x slower on candle/BGE paths (measured; see CLAUDE.md), which turns an
# 11-second index into something that looks like a hang.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="${GAPLY_DEV_LOG:-$HOME/gaply-dev.log}"
PIDFILE="$HOME/.gaply-dev.pid"

if [[ "${1:-}" == "--stop" ]]; then
  if [[ -f "$PIDFILE" ]] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    # Kill the whole session we created, not a pattern match: `pkill -f "tauri
    # dev"` is how you also kill someone else's unrelated run.
    pgid="$(ps -o pgid= -p "$(cat "$PIDFILE")" | tr -d ' ')"
    kill -TERM -- "-$pgid" 2>/dev/null || kill -TERM "$(cat "$PIDFILE")" 2>/dev/null || true
    echo "stopped (pgid $pgid)"
  else
    echo "not running (no live pid in $PIDFILE)"
  fi
  rm -f "$PIDFILE"
  exit 0
fi

if [[ -f "$PIDFILE" ]] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
  echo "already running as pid $(cat "$PIDFILE") — $LOG"
  exit 0
fi

PROFILE_ARGS=(-- --release)
[[ "${1:-}" == "--debug" ]] && PROFILE_ARGS=()

REPO="$REPO" LOG="$LOG" PIDFILE="$PIDFILE" \
ARGS="${PROFILE_ARGS[*]-}" python3 <<'PY'
import os, sys

repo, log, pidfile = os.environ["REPO"], os.environ["LOG"], os.environ["PIDFILE"]
args = [a for a in os.environ.get("ARGS", "").split() if a]

# First fork: the parent returns to the shell immediately, so the child is
# orphaned and reparented to launchd rather than to this script.
if os.fork() > 0:
    os._exit(0)
# New session: no controlling terminal, and a process group of our own, so a
# group-directed kill aimed at the launching shell cannot reach us.
os.setsid()
# Second fork: the session leader exits, leaving a process that can never
# reacquire a controlling terminal even by accident.
if os.fork() > 0:
    os._exit(0)

os.chdir(repo)
fd = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
os.dup2(fd, 1)
os.dup2(fd, 2)
devnull = os.open(os.devnull, os.O_RDONLY)
os.dup2(devnull, 0)

with open(pidfile, "w") as f:
    f.write(str(os.getpid()))

os.execvp("npm", ["npm", "run", "tauri", "dev", *args])
PY

sleep 2
if [[ -f "$PIDFILE" ]]; then
  pid="$(cat "$PIDFILE")"
  echo "launched pid $pid (parent $(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ' || echo '?'))"
  echo "log: $LOG"
else
  echo "launch did not record a pid — check $LOG"
fi
