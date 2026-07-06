"""Claude forwarder. Adds the server-side key (from env, never hardcoded) and
forwards the structured summary to Claude Sonnet via the official Anthropic SDK.

Behind the ``ClaudeClient`` protocol so tests inject a stub — no real Claude
call or network access happens in the test suite.
"""

from __future__ import annotations

import asyncio
import json
from typing import Any, Protocol


class ClaudeClient(Protocol):
    async def complete(self, payload: dict[str, Any]) -> dict[str, Any]:
        ...


class AnthropicClaudeClient:
    """Real forwarder. The Anthropic SDK is imported lazily so importing this
    module (and building the app) never requires the SDK to be present."""

    def __init__(
        self,
        api_key: str,
        model: str = "claude-sonnet-5",
        max_tokens: int = 1024,
        system: str | None = None,
    ) -> None:
        from anthropic import Anthropic  # lazy import

        self._client = Anthropic(api_key=api_key)
        self.model = model
        self.max_tokens = max_tokens
        self.system = system or (
            "You review structured research summaries. Input is JSON (never a raw "
            "manuscript). Return concise, structured findings."
        )

    async def complete(self, payload: dict[str, Any]) -> dict[str, Any]:
        requested = payload.get("max_tokens") or self.max_tokens
        # Sonnet 5: adaptive thinking is the default; do NOT pass temperature/
        # top_p/top_k (they 400 on Sonnet 5). Keep the request minimal.
        content = json.dumps(
            {"summary": payload.get("summary"), "instruction": payload.get("instruction")}
        )

        def _call() -> dict[str, Any]:
            message = self._client.messages.create(
                model=self.model,
                max_tokens=int(requested),
                system=self.system,
                messages=[{"role": "user", "content": content}],
            )
            text = "".join(
                block.text
                for block in message.content
                if getattr(block, "type", None) == "text"
            )
            return {
                "model": message.model,
                "stop_reason": message.stop_reason,
                "text": text,
            }

        # SDK call is sync; run it off the event loop.
        return await asyncio.to_thread(_call)
