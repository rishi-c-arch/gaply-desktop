"""Bind-address safety net.

The proxy must be INVISIBLE to the public internet: it may bind only to
localhost or to a private/tailnet interface, never to a public one. This module
classifies a prospective bind host and refuses anything that would expose the
service publicly — most importantly ``0.0.0.0`` / ``::`` ("all interfaces"),
which is the classic misconfiguration that silently opens a public port.

Allowed:
  * loopback            — 127.0.0.0/8, ::1, "localhost"
  * Tailscale tailnet   — 100.64.0.0/10 (CGNAT range Tailscale assigns 100.x IPs
                          from) and Tailscale's ULA IPv6 (fd7a:115c:a1e0::/48,
                          covered by the generic private/ULA check)
  * other RFC1918/ULA   — 10/8, 172.16/12, 192.168/16, fc00::/7 (a private LAN
                          interface is still not the public internet)

Rejected:
  * 0.0.0.0 / ::        — binds ALL interfaces, including public ones
  * any global/public IP
  * a bare hostname that isn't "localhost" (it could resolve to a public IP; we
    refuse rather than guess)

This is a defence-in-depth safety net, not the primary control. The primary
control is *how* the host is exposed — via Tailscale Serve over the tailnet with
zero open public ports (see README). This check just guarantees the process
can't accidentally bind somewhere public if the deploy is misconfigured.
"""

from __future__ import annotations

import ipaddress
from dataclasses import dataclass

# The 100.64.0.0/10 shared-address (CGNAT) range. Tailscale hands out its 100.x
# node addresses from here. We test it explicitly because whether Python's
# ``ipaddress.is_private`` includes this range depends on the interpreter
# version (added to the private list in 3.13), and we want version-independent
# behaviour.
TAILSCALE_CGNAT = ipaddress.ip_network("100.64.0.0/10")

_LOCALHOST_NAMES = {"localhost", "localhost.", "ip6-localhost"}


class InsecureBindError(RuntimeError):
    """Raised when the proxy is about to bind to a public/unsafe interface."""


@dataclass(frozen=True)
class BindClassification:
    host: str
    allowed: bool
    kind: str  # loopback | tailscale | private | unspecified | public | hostname
    reason: str

    @property
    def is_tailnet(self) -> bool:
        return self.kind == "tailscale"


def classify_bind(host: str) -> BindClassification:
    """Classify a prospective uvicorn bind host. Never raises."""
    h = (host or "").strip()
    # Strip an IPv6 zone id (e.g. "fe80::1%en0") before parsing.
    parse_target = h.split("%", 1)[0]

    if h.lower() in _LOCALHOST_NAMES:
        return BindClassification(h, True, "loopback", "localhost name")

    try:
        ip = ipaddress.ip_address(parse_target)
    except ValueError:
        # Not an IP literal and not "localhost". A hostname could resolve to a
        # public address, so we refuse rather than silently trust DNS.
        return BindClassification(
            h, False, "hostname",
            "not an IP literal or 'localhost'; refuse to guess what it resolves to",
        )

    # 0.0.0.0 / :: — "all interfaces". This is the dangerous default; check it
    # BEFORE is_private, because Python classifies 0.0.0.0/8 as private.
    if ip.is_unspecified:
        return BindClassification(
            h, False, "unspecified",
            "binds ALL interfaces (would expose the service publicly)",
        )

    if ip.is_loopback:
        return BindClassification(h, True, "loopback", "loopback interface")

    if ip.version == 4 and ip in TAILSCALE_CGNAT:
        return BindClassification(h, True, "tailscale", "Tailscale tailnet address (100.64.0.0/10)")

    if ip.is_private:
        return BindClassification(h, True, "private", "private (RFC1918 / ULA) address")

    return BindClassification(h, False, "public", "public/globally-routable address")


def enforce_private_bind(host: str, *, allow_public: bool = False) -> BindClassification:
    """Verify ``host`` is safe to bind; raise ``InsecureBindError`` if not.

    ``allow_public=True`` is the documented break-glass escape hatch (set only
    via the GAPLY_ALLOW_PUBLIC_BIND env token). When enabled, a public bind is
    permitted but still returned as ``allowed=False`` so the caller can log a
    loud warning.
    """
    result = classify_bind(host)
    if result.allowed or allow_public:
        return result
    raise InsecureBindError(
        f"refusing to start: bind host {host!r} is a {result.kind} address "
        f"({result.reason}). The proxy may bind only to localhost or a "
        f"private/tailnet (100.64.0.0/10) interface. Set GAPLY_BIND_HOST to a "
        f"127.0.0.1 or 100.x tailnet address, or expose it via Tailscale Serve. "
        f"(Break-glass override: GAPLY_ALLOW_PUBLIC_BIND=i-accept-public-exposure.)"
    )
