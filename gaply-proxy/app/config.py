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
    )
