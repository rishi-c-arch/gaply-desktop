"""Fail-closed launch entrypoint: `python -m app`.

This is the authoritative launch path — it decides the bind host and hands it to
uvicorn, so the address we actually bind is the address the safety net checked.
Prefer this over `uvicorn app.main:app --host ...`, where a `--host 0.0.0.0`
passed on the command line would bypass the env-based check.

Order of operations:
  1. Load settings from the environment.
  2. Enforce the private/tailnet bind safety net (refuse to start on a public
     interface, unless the documented break-glass override is set).
  3. Launch uvicorn bound to exactly that verified host/port.
"""

from __future__ import annotations

import sys

from .bind_guard import InsecureBindError, enforce_private_bind
from .config import settings_from_env


def main() -> int:
    settings = settings_from_env()
    try:
        result = enforce_private_bind(
            settings.bind_host, allow_public=settings.allow_public_bind
        )
    except InsecureBindError as exc:
        print(f"[gaply-proxy] {exc}", file=sys.stderr)
        return 2

    if not result.allowed:
        # allow_public override is active — bind is public but explicitly waived.
        print(
            "[gaply-proxy] WARNING: public-bind safety net OVERRIDDEN "
            f"(GAPLY_ALLOW_PUBLIC_BIND). Binding to a {result.kind} address "
            f"{settings.bind_host!r} — the service may be reachable from the "
            "public internet. This is almost never what you want.",
            file=sys.stderr,
        )
    else:
        print(
            f"[gaply-proxy] bind check OK: {settings.bind_host} "
            f"({result.kind} — {result.reason})",
            file=sys.stderr,
        )

    import uvicorn  # imported here so the bind check runs before anything binds

    uvicorn.run(
        "app.main:app",
        host=settings.bind_host,
        port=settings.bind_port,
        # No proxy headers / forwarded-allow beyond loopback: Tailscale Serve
        # terminates TLS and forwards over the tailnet to this local port.
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
