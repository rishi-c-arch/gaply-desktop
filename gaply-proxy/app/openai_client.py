"""OpenAI forwarder — the SECOND provider, alongside AnthropicClaudeClient.

Implements the SAME ``LlmProvider`` contract (structured summary in ->
``{model, stop_reason, text}`` out) and follows the SAME safe key pattern as
the Anthropic client: ``OPENAI_API_KEY`` from the environment, never hardcoded,
never committed. Uses httpx (already a dependency) so no new SDK is pulled in.

Provider selection is entirely SERVER-SIDE (see ``get_provider`` in main.py):
the desktop app sends the same opaque structured payload regardless and never
knows which provider handled it.
"""

from __future__ import annotations

import json
from typing import Any

OPENAI_URL = "https://api.openai.com/v1/chat/completions"

_DEFAULT_SYSTEM = (
    "You review structured research summaries. Input is JSON (never a raw "
    "manuscript). Return concise, structured findings."
)


class OpenAIClient:
    """Real forwarder to OpenAI chat-completions. httpx is imported lazily so
    importing this module never requires a live network."""

    def __init__(
        self,
        api_key: str,
        model: str = "gpt-4o-mini",
        max_tokens: int = 1024,
        system: str | None = None,
        *,
        http_client: Any | None = None,  # tests inject an httpx.AsyncClient(MockTransport)
        timeout: float = 180.0,
    ) -> None:
        self._api_key = api_key
        self.model = model
        self.max_tokens = max_tokens
        self.system = system or _DEFAULT_SYSTEM
        self._http_client = http_client
        self._timeout = timeout

    async def complete(self, payload: dict[str, Any]) -> dict[str, Any]:
        import httpx

        requested = payload.get("max_tokens") or self.max_tokens
        # Same structured-only forwarding as the Claude client: summary +
        # instruction, never raw manuscript text.
        content = json.dumps(
            {"summary": payload.get("summary"), "instruction": payload.get("instruction")}
        )
        body = {
            "model": self.model,
            "max_tokens": int(requested),
            "messages": [
                {"role": "system", "content": self.system},
                {"role": "user", "content": content},
            ],
        }
        headers = {"Authorization": f"Bearer {self._api_key}"}

        client = self._http_client or httpx.AsyncClient(timeout=self._timeout)
        try:
            resp = await client.post(OPENAI_URL, headers=headers, json=body)
            resp.raise_for_status()
            data = resp.json()
        finally:
            if self._http_client is None:
                await client.aclose()

        choice = (data.get("choices") or [{}])[0]
        # Normalize to the SAME envelope the Claude client returns.
        return {
            "model": data.get("model", self.model),
            "stop_reason": choice.get("finish_reason"),
            "text": (choice.get("message") or {}).get("content") or "",
        }
