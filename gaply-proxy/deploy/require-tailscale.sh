#!/usr/bin/env bash
# Fail closed if Tailscale isn't up and connected. Used as the systemd
# ExecStartPre gate: if this exits non-zero, uvicorn never starts, so the proxy
# can't come up on a host that isn't on the tailnet.
#
# Checks, in order:
#   1. the `tailscale` CLI exists
#   2. the daemon reports BackendState == "Running" (i.e. `tailscale up` done)
#   3. this node actually has a 100.x tailnet IP assigned
#
# Exit codes: 0 = tailnet ready; non-zero = not ready (systemd aborts start).
set -euo pipefail

if ! command -v tailscale >/dev/null 2>&1; then
  echo "require-tailscale: 'tailscale' CLI not found — install Tailscale first." >&2
  exit 1
fi

# Prefer JSON status (stable/parseable). Fall back to plain text if jq is absent.
if command -v jq >/dev/null 2>&1; then
  state="$(tailscale status --json 2>/dev/null | jq -r '.BackendState // "Unknown"')"
  self_ip="$(tailscale status --json 2>/dev/null | jq -r '.Self.TailscaleIPs[0] // empty')"
else
  # `tailscale ip -4` prints this node's tailnet IPv4 if connected.
  state="$([ -n "$(tailscale ip -4 2>/dev/null || true)" ] && echo Running || echo Unknown)"
  self_ip="$(tailscale ip -4 2>/dev/null | head -n1 || true)"
fi

if [ "$state" != "Running" ]; then
  echo "require-tailscale: tailnet not up (BackendState='$state'). Run 'tailscale up'." >&2
  exit 2
fi

if [ -z "${self_ip:-}" ]; then
  echo "require-tailscale: no tailnet IP assigned to this node yet." >&2
  exit 3
fi

echo "require-tailscale: OK — tailnet up, node IP ${self_ip}."
