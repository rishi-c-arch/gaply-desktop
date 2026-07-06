import pytest

from app.validation import ValidationError, validate_structured

STRUCTURED = {
    "summary": {
        "title": "Effect of sleep",
        "findings": ["n = 96", "p < 0.001"],
        "tests": [{"name": "t-test"}, {"name": "ANOVA"}],
    },
    "instruction": "Validate the stats.",
}

# a raw-manuscript-like blob: one long field of many sentences of prose
RAW_PROSE = {
    "summary": {
        "text": (
            "We examined whether extended sleep improves working-memory performance. "
            "In a randomized controlled trial participants who slept nine hours "
            "outperformed a control group. Prior work has been mixed. "
            "Working memory is central to reasoning. Earlier studies suggested a link "
            "between sleep and cognition, but effect sizes varied widely. "
            "Ninety-six healthy adults were randomized into two groups. "
            "We used an independent t-test to compare group means. "
            "A one-way ANOVA assessed dose response across three sleep durations. "
            "The extended-sleep group scored higher on the n-back task. "
            "A secondary analysis showed a dose effect. These findings replicate prior work. "
            "Our results support a causal role for sleep in working memory. "
            "The effect was robust across analyses."
        )
    }
}


def test_structured_summary_passes():
    total = validate_structured(STRUCTURED)
    assert total > 0


def test_raw_prose_field_rejected():
    with pytest.raises(ValidationError):
        validate_structured(RAW_PROSE)


def test_non_object_payload_rejected():
    with pytest.raises(ValidationError):
        validate_structured("just a raw string")


def test_total_length_cap():
    payload = {"summary": {"points": ["x" * 500 for _ in range(30)]}}  # 15000 chars
    with pytest.raises(ValidationError):
        validate_structured(payload, max_total=8000)


def test_single_long_field_rejected():
    payload = {"summary": {"blob": "y" * 3000}}
    with pytest.raises(ValidationError):
        validate_structured(payload, max_field=2000)
