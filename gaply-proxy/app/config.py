"""Configuration from environment variables (keys are NEVER hardcoded)."""

from __future__ import annotations

import os
from dataclasses import dataclass, field


@dataclass
class Settings:
    # Server-side Claude key — from env only.
    claude_api_key: str = ""
    claude_model: str = "claude-sonnet-5"
    # App Check shared HMAC secret (must match the Rust client's signing key).
    app_check_signing_key: str = ""
    app_check_app_id: str = "ai.gaply.app"
    app_check_debug_tokens: tuple[str, ...] = ()
    # Rate limiter.
    rate_limit_capacity: float = 60.0
    rate_limit_refill_rate: float = 1.0
    # Validation thresholds.
    max_total_chars: int = 8000
    max_field_chars: int = 2000
    max_sentences_per_field: int = 8
    # Bind address — must be localhost or a private/tailnet interface. Defaults
    # to loopback so the proxy is never public unless deliberately reconfigured.
    bind_host: str = "127.0.0.1"
    bind_port: int = 8080
    # Break-glass ONLY: disables the public-bind safety net. Never set in normal
    # operation — see the bind_guard module and README.
    allow_public_bind: bool = False
    # Nitro Enclave TEE path (opt-in; requires real AWS Nitro infra to function).
    # When enabled, sensitive processing + the API key live inside the enclave
    # and the proxy forwards summaries over VSOCK after verifying attestation.
    enclave_enabled: bool = False
    enclave_cid: int = 16  # enclave VSOCK context id (nitro-cli describe-enclaves)
    enclave_port: int = 5005
    enclave_expected_pcr0: str = ""  # pinned hex SHA-384 of the enclave image (EIF)
    enclave_attestation_max_age_secs: int = 300


def settings_from_env() -> Settings:
    debug = tuple(
        t.strip() for t in os.getenv("APP_CHECK_DEBUG_TOKENS", "").split(",") if t.strip()
    )
    return Settings(
        claude_api_key=os.getenv("CLAUDE_API_KEY") or os.getenv("ANTHROPIC_API_KEY", ""),
        claude_model=os.getenv("CLAUDE_MODEL", "claude-sonnet-5"),
        app_check_signing_key=os.getenv("APP_CHECK_SIGNING_KEY", ""),
        app_check_app_id=os.getenv("APP_CHECK_APP_ID", "ai.gaply.app"),
        app_check_debug_tokens=debug,
        rate_limit_capacity=float(os.getenv("RATE_LIMIT_CAPACITY", "60")),
        rate_limit_refill_rate=float(os.getenv("RATE_LIMIT_REFILL_RATE", "1")),
        max_total_chars=int(os.getenv("MAX_TOTAL_CHARS", "8000")),
        max_field_chars=int(os.getenv("MAX_FIELD_CHARS", "2000")),
        max_sentences_per_field=int(os.getenv("MAX_SENTENCES_PER_FIELD", "8")),
        bind_host=os.getenv("GAPLY_BIND_HOST", "127.0.0.1"),
        bind_port=int(os.getenv("GAPLY_BIND_PORT", "8080")),
        # Requires an explicit, unambiguous token to enable — a truthy value like
        # "1" won't do it, so it can't be flipped on by accident.
        allow_public_bind=os.getenv("GAPLY_ALLOW_PUBLIC_BIND", "") == "i-accept-public-exposure",
        enclave_enabled=os.getenv("GAPLY_ENCLAVE_ENABLED", "").lower() in {"1", "true", "yes"},
        enclave_cid=int(os.getenv("GAPLY_ENCLAVE_CID", "16")),
        enclave_port=int(os.getenv("GAPLY_ENCLAVE_PORT", "5005")),
        enclave_expected_pcr0=os.getenv("GAPLY_ENCLAVE_PCR0", ""),
        enclave_attestation_max_age_secs=int(os.getenv("GAPLY_ENCLAVE_MAX_AGE", "300")),
    )
