"""Bind-address safety-net tests.

VERIFIED HERE (pure logic, no network): that classify_bind / enforce_private_bind
ACCEPT localhost + private/tailnet addresses and REJECT 0.0.0.0 and public IPs.
What this CANNOT verify offline: that a real Tailscale tailnet actually routes
traffic and that `tailscale serve` publishes with HTTPS + zero public ports —
that needs a live tailnet (see README "What is / isn't verified").
"""

import pytest

from app.bind_guard import (
    InsecureBindError,
    classify_bind,
    enforce_private_bind,
)

# --- ACCEPTED: loopback + private/tailnet interfaces -------------------------

ACCEPTED = [
    ("localhost", "loopback"),
    ("127.0.0.1", "loopback"),
    ("127.0.0.5", "loopback"),
    ("::1", "loopback"),
    ("100.64.0.1", "tailscale"),      # bottom of the CGNAT range
    ("100.115.92.3", "tailscale"),    # a typical Tailscale 100.x node IP
    ("100.127.255.254", "tailscale"), # near the top of 100.64.0.0/10
    ("10.0.0.5", "private"),          # RFC1918
    ("192.168.1.10", "private"),      # RFC1918
    ("172.16.0.9", "private"),        # RFC1918
    ("fd7a:115c:a1e0::1", "private"), # Tailscale ULA IPv6 (fc00::/7)
]


@pytest.mark.parametrize("host,kind", ACCEPTED)
def test_accepts_localhost_and_private_tailnet(host, kind):
    c = classify_bind(host)
    assert c.allowed, f"{host} should be allowed"
    assert c.kind == kind
    # enforce must NOT raise for an accepted address.
    assert enforce_private_bind(host).allowed


def test_tailnet_flagged_as_tailnet():
    assert classify_bind("100.115.92.3").is_tailnet
    assert not classify_bind("127.0.0.1").is_tailnet


# --- REJECTED: 0.0.0.0 / :: and public addresses ----------------------------

REJECTED = [
    ("0.0.0.0", "unspecified"),   # binds ALL interfaces — the classic mistake
    ("::", "unspecified"),        # IPv6 all-interfaces
    ("8.8.8.8", "public"),        # public IPv4
    ("1.2.3.4", "public"),        # public IPv4
    ("93.184.216.34", "public"),  # a real globally-routable IPv4
    ("2606:4700:4700::1111", "public"),  # public IPv6
    ("example.com", "hostname"),  # a name that could resolve public -> refuse
]


@pytest.mark.parametrize("host,kind", REJECTED)
def test_rejects_public_and_all_interfaces(host, kind):
    c = classify_bind(host)
    assert not c.allowed, f"{host} should be rejected"
    assert c.kind == kind
    with pytest.raises(InsecureBindError):
        enforce_private_bind(host)


def test_zero_bind_is_rejected_even_though_python_calls_it_private():
    # ipaddress marks 0.0.0.0/8 as private; the guard must still reject 0.0.0.0
    # because it means "all interfaces" (public exposure). Regression guard.
    c = classify_bind("0.0.0.0")
    assert not c.allowed
    assert c.kind == "unspecified"


# --- break-glass override ----------------------------------------------------

def test_allow_public_override_permits_public_but_reports_not_allowed():
    # With the documented escape hatch, a public bind does NOT raise, but the
    # classification still reports allowed=False so the caller can warn loudly.
    result = enforce_private_bind("0.0.0.0", allow_public=True)
    assert result.kind == "unspecified"
    assert result.allowed is False  # still classified as unsafe


def test_override_does_not_change_private_classification():
    result = enforce_private_bind("127.0.0.1", allow_public=True)
    assert result.allowed is True


# --- integration with the app startup check ---------------------------------

def test_create_app_refuses_public_bind():
    from app.config import Settings
    from app.main import create_app

    with pytest.raises(InsecureBindError):
        create_app(Settings(app_check_signing_key="k", bind_host="0.0.0.0"))


def test_create_app_accepts_tailnet_bind():
    from app.config import Settings
    from app.main import create_app

    app = create_app(Settings(app_check_signing_key="k", bind_host="100.115.92.3"))
    assert app is not None
