# Gaply Research Reliability Benchmark (GRRB)

Phase 2b of `docs/publishready-premium-architecture.md` §6c.1. **It grows; it is
never finished.**

## One JSONL per family

`mathematical` · `statistical` · `manuscript_consistency` · `literature` ·
`journal` · `adversarial`

## The case record

```json
{
  "id":       "grrb-adv-001",
  "family":   "adversarial",
  "stratum":  "guideline_page",
  "provenance": "adapted:<what>, anonymised: <how>" | "hand-written" | "synthetic",
  "note":     "why this case exists, and the record it came from",
  "input":    { "kind": "text" | "blocks" | "lines", ... },
  "expected": { "finding": true, "codes": ["..."] } | { "finding": false }
}
```

**`expected.finding: false` is a first-class case, not a filler.** Four of the
five measurements this set was seeded from are about a check firing on something
correct, so an expected ABSENCE is the majority of what is worth measuring here.

## Population estimates are MEASURED or declared absent

`populations.json` carries one entry per `family/stratum`, and each entry says
**how the number was obtained**. A stratum whose population is unmeasured is
written `null` with a reason, and the runner then refuses to print a weighted
rate for it rather than substituting the pooled one — `ai/eval_strata.rs`'s
`Stratum::scale()` already returns `None` for exactly this case, and §11 D123 is
what happens when a pooled rate is printed instead.

## What is NOT here

No target values. §6c.2: *"Targets are set per agent from its first measured
baseline, and the record says what the baseline was."* The first run against the
Tier 0 engine sets the baseline and the decision record states it.
