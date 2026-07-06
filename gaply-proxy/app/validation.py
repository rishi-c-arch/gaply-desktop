"""Hard validator: structured summaries only, never raw manuscript text.

A heuristic safety net (not a proof) that rejects payloads whose total text is
too long, whose individual fields are long enough to be raw prose, or whose
fields read like multi-sentence paragraphs rather than short summary points.
"""

from __future__ import annotations

import re
from typing import Any, Iterator

# Defaults — a structured summary is short and field-shaped, not a manuscript.
MAX_TOTAL_CHARS = 8000
MAX_FIELD_CHARS = 2000
MAX_SENTENCES_PER_FIELD = 8
PROSE_MIN_FIELD_CHARS = 400  # sentence heuristic only kicks in past this length

_SENTENCE = re.compile(r"[.!?]+(?:\s|$)")


class ValidationError(Exception):
    def __init__(self, reason: str) -> None:
        self.reason = reason
        super().__init__(reason)


def _string_leaves(obj: Any) -> Iterator[str]:
    if isinstance(obj, str):
        yield obj
    elif isinstance(obj, dict):
        for value in obj.values():
            yield from _string_leaves(value)
    elif isinstance(obj, list):
        for item in obj:
            yield from _string_leaves(item)


def validate_structured(
    payload: Any,
    *,
    max_total: int = MAX_TOTAL_CHARS,
    max_field: int = MAX_FIELD_CHARS,
    max_sentences: int = MAX_SENTENCES_PER_FIELD,
) -> int:
    """Validate that ``payload`` is a structured summary. Returns total chars.

    Raises ``ValidationError`` on anything that looks like raw prose / a raw
    manuscript rather than a structured summary object.
    """
    if not isinstance(payload, dict):
        raise ValidationError(
            "payload must be a structured JSON object, not raw text"
        )

    strings = list(_string_leaves(payload))
    total = sum(len(s) for s in strings)
    if total > max_total:
        raise ValidationError(
            f"total text content is {total} chars (limit {max_total}); "
            "send a structured summary, not raw manuscript text"
        )

    for s in strings:
        if len(s) > max_field:
            raise ValidationError(
                f"a field of {len(s)} chars looks like raw prose; "
                "structured summaries use short fields, not manuscript paragraphs"
            )
        sentences = len(_SENTENCE.findall(s))
        if sentences > max_sentences and len(s) > PROSE_MIN_FIELD_CHARS:
            raise ValidationError(
                f"a field with {sentences} sentences reads like raw prose, "
                "not a structured summary point"
            )

    return total
