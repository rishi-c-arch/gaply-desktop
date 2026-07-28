"""Gaply proxy — a tiny FastAPI service.

Only job: receive a STRUCTURED SUMMARY (JSON, never raw manuscript text) from
the Rust core, verify the App Check token and rate limit, add the server-side
Claude key, forward to Claude Sonnet, return the response. Nothing is persisted.

Security layers, in order:
  1. rate limiter — FastAPI middleware, keyed by a client identifier (429).
  2. App Check    — dependency on /verify; a valid token is required before any
                    Claude call (401 otherwise).
  3. validator    — structured summaries only; raw-prose payloads are rejected
                    (422) before forwarding.
/health is exempt from all of the above (no auth, liveness only).
"""

from __future__ import annotations

from typing import Any

from fastapi import Depends, FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse

from .app_check import HEADER_NAME, AppCheckVerifier, VerifyError, VerifiedToken
from .bind_guard import BindClassification, InsecureBindError, enforce_private_bind
from .claude_client import AnthropicClaudeClient, LlmProvider
from .openai_client import OpenAIClient
from .config import Settings, settings_from_env
from .entitlement import (
    IDENTITY_REASONS,
    USER_TOKEN_HEADER,
    EntitlementChecker,
    build_supabase_checker,
)
from .rate_limit import TokenBucketRateLimiter
from .validation import ValidationError, validate_structured

EXEMPT_PATHS = {"/health"}


def check_bind(settings: Settings) -> BindClassification:
    """Startup safety net: refuse to run if the configured bind host is public.

    Raises InsecureBindError (fail closed) unless the break-glass override is
    set. Called at app-creation time so `uvicorn app.main:app` with a public
    GAPLY_BIND_HOST fails immediately, and again by the __main__ entrypoint,
    which is the authoritative launch path (it hands the host to uvicorn).
    """
    return enforce_private_bind(settings.bind_host, allow_public=settings.allow_public_bind)


def _enforce_public_exposure_guards(
    settings: Settings, entitlement_checker: EntitlementChecker | None
) -> None:
    """Boot-time fail-safe (Set 8, step 4). A PUBLICLY EXPOSED proxy
    (``allow_public_bind`` accepted) MUST have entitlement fully wired — refuse
    to start otherwise, so an open paid endpoint can never come up by
    misconfiguration. Private/loopback binds keep the request-time 503 for the
    enabled-but-unconfigured case and are exempt here (dev convenience, no
    financial exposure)."""
    if not settings.allow_public_bind:
        return
    problems = []
    if not settings.entitlement_required:
        problems.append("GAPLY_ENTITLEMENT_REQUIRED must be true")
    if entitlement_checker is None:
        problems.append("a real EntitlementChecker must be configured (SUPABASE_URL + DATABASE_URL)")
    if not settings.app_check_signing_key:
        problems.append("APP_CHECK_SIGNING_KEY must be set")
    if settings.app_check_debug_tokens:
        problems.append("APP_CHECK_DEBUG_TOKENS must be empty")
    if problems:
        raise RuntimeError(
            "refusing to start with public exposure — " + "; ".join(problems)
        )


def create_app(
    settings: Settings | None = None,
    claude_client: LlmProvider | None = None,
    entitlement_checker: EntitlementChecker | None = None,
    providers: dict[str, LlmProvider] | None = None,
) -> FastAPI:
    settings = settings or settings_from_env()
    # Safety net: never come up bound to a public interface (see bind_guard).
    check_bind(settings)
    # Production wiring: build the real checker from env when enforcement is on
    # and none was injected (tests inject their own). Requires SUPABASE_URL +
    # DATABASE_URL.
    if (
        entitlement_checker is None
        and settings.entitlement_required
        and settings.database_url
    ):
        entitlement_checker = build_supabase_checker(settings)
    # Boot-time fail-safe: a public proxy must have the gate fully wired.
    _enforce_public_exposure_guards(settings, entitlement_checker)
    verifier = AppCheckVerifier(
        settings.app_check_signing_key.encode("utf-8"),
        settings.app_check_app_id,
        settings.app_check_debug_tokens,
    )
    limiter = TokenBucketRateLimiter(
        settings.rate_limit_capacity, settings.rate_limit_refill_rate
    )

    app = FastAPI(title="Gaply Proxy", version="0.1.0")
    app.state.settings = settings
    app.state.verifier = verifier
    app.state.limiter = limiter
    # Provider registry (server-side). The injected `claude_client` (tests, or a
    # provisioned enclave client) fills the "claude" slot; `providers` lets tests
    # inject others (e.g. an OpenAI stub). Lazily-built entries are cached here.
    registry: dict[str, LlmProvider] = dict(providers or {})
    if claude_client is not None:
        registry.setdefault("claude", claude_client)
    app.state.claude_client = claude_client  # back-compat handle
    app.state.providers = registry
    app.state.entitlement_checker = entitlement_checker  # None -> stub (see below)

    @app.middleware("http")
    async def rate_limit_middleware(request: Request, call_next):
        if request.url.path in EXEMPT_PATHS:
            return await call_next(request)
        client_id = request.headers.get("x-client-id") or (
            request.client.host if request.client else "unknown"
        )
        result = limiter.try_consume(client_id)
        if not result.allowed:
            return JSONResponse(
                status_code=429,
                content={"error": "rate_limited", "retry_after": round(result.retry_after, 3)},
                headers={"Retry-After": str(int(result.retry_after) + 1)},
            )
        return await call_next(request)

    def require_app_check(request: Request) -> VerifiedToken:
        token = request.headers.get(HEADER_NAME, "")
        try:
            return verifier.verify(token)
        except VerifyError as exc:
            raise HTTPException(
                status_code=401,
                detail={"error": "app_check_failed", "reason": exc.code},
            )

    def get_provider() -> LlmProvider:
        """Select the LLM provider SERVER-SIDE from config (GAPLY_LLM_PROVIDER).
        The request body never names a provider — the desktop stays
        provider-blind. Both providers reach here through the SAME App Check +
        entitlement dependencies; this only picks the forwarder."""
        name = settings.llm_provider
        cached = app.state.providers.get(name)
        if cached is not None:
            return cached
        if name == "claude":
            # TEE path: when the enclave is enabled, the API key and the Claude
            # call live INSIDE the Nitro Enclave; a real deploy injects a
            # provisioned EnclaveClaudeClient as `claude_client`. Without one we
            # fail closed rather than fall back to a host-side call that would
            # defeat the enclave's purpose.
            if settings.enclave_enabled:
                raise HTTPException(
                    status_code=503,
                    detail={
                        "error": "enclave_not_provisioned",
                        "reason": "GAPLY_ENCLAVE_ENABLED is set but no attested "
                        "EnclaveClaudeClient was injected. Provision the Nitro "
                        "Enclave (deploy/enclave) and supply it as claude_client.",
                    },
                )
            if not settings.claude_api_key:
                raise HTTPException(
                    status_code=503, detail={"error": "claude_not_configured"}
                )
            client: LlmProvider = AnthropicClaudeClient(
                settings.claude_api_key, settings.claude_model
            )
        elif name == "openai":
            if not settings.openai_api_key:
                raise HTTPException(
                    status_code=503, detail={"error": "openai_not_configured"}
                )
            client = OpenAIClient(settings.openai_api_key, settings.openai_model)
        else:
            raise HTTPException(
                status_code=503,
                detail={
                    "error": "unknown_provider",
                    "reason": f"GAPLY_LLM_PROVIDER={name!r} is not 'claude' or 'openai'",
                },
            )
        app.state.providers[name] = client
        return client

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"status": "ok"}

    async def require_entitlement(request: Request) -> str | None:
        """THE REAL entitlement gate (Set 8) — runs BEFORE any paid model work.
        Returns the user token to consume against on success, or None when
        enforcement is off (the honest pre-deployment stub state; production
        MUST flip GAPLY_ENTITLEMENT_REQUIRED=true and wire a checker).
        Enabled with no checker fails CLOSED (503) rather than skipping the
        gate. Identity failures (missing/invalid/expired token) -> 401;
        valid-but-not-entitled -> 403."""
        if not settings.entitlement_required:
            return None
        checker: EntitlementChecker | None = app.state.entitlement_checker
        if checker is None:
            raise HTTPException(
                status_code=503,
                detail={
                    "error": "entitlement_not_configured",
                    "reason": "GAPLY_ENTITLEMENT_REQUIRED is set but no "
                    "EntitlementChecker was injected into create_app.",
                },
            )
        user_token = request.headers.get(USER_TOKEN_HEADER, "")
        if not user_token:
            raise HTTPException(
                status_code=401,
                detail={"error": "user_token_missing", "reason": "sign in required"},
            )
        result = await checker.check(user_token)
        if not result.entitled:
            # Identity failure -> 401 (who are you?); otherwise -> 403 (known,
            # not entitled). Never a generic 500.
            if result.reason in IDENTITY_REASONS:
                raise HTTPException(
                    status_code=401,
                    detail={"error": "unauthenticated", "reason": result.reason},
                )
            raise HTTPException(
                status_code=403,
                detail={"error": "not_entitled", "reason": result.reason},
            )
        return user_token

    @app.post("/verify")
    async def verify_endpoint(
        request: Request,
        _token: VerifiedToken = Depends(require_app_check),
        provider: LlmProvider = Depends(get_provider),
        entitled_user: str | None = Depends(require_entitlement),
    ) -> dict[str, Any]:
        payload = await request.json()
        # Hard validator: structured summaries only, never raw manuscript text.
        try:
            validate_structured(
                payload,
                max_total=settings.max_total_chars,
                max_field=settings.max_field_chars,
                max_sentences=settings.max_sentences_per_field,
            )
        except ValidationError as exc:
            raise HTTPException(
                status_code=422,
                detail={"error": "validation_failed", "reason": exc.reason},
            )
        # Forward to the selected provider (Claude or OpenAI, server-chosen);
        # nothing is persisted. The desktop never knows which one handled it.
        result = await provider.complete(payload)
        # Consume a use ONLY after the paid work succeeded — server-side,
        # never a client-side number.
        if entitled_user is not None:
            await app.state.entitlement_checker.consume(entitled_user)
        return {"result": result}

    return app


# Module-level app for `uvicorn app.main:app`.
app = create_app()
