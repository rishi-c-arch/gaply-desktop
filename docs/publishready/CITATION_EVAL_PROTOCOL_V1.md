# Citation Extraction Evaluation — Protocol v1

**Status:** FROZEN. Written and hashed **before** any ground truth was counted and before any extractor output was inspected.

**Amendment 1 — 2026-08-03, before any extractor output was seen.** §7 (Decision Matrix D2) and §8 (error categories) were added after the ground truth was frozen but **before** the extractor was run or its output inspected. Both are decision rules, not ground truth, so the ground-truth hash is unaffected. The protocol was re-hashed on amendment.

**Purpose.** Establish whether `extract_in_text` achieves coverage sufficient to wire the uncited-reference finding (`report.rs::uncited_reference_findings`, built and deliberately unwired at `b8312b0`). ONTOLOGY §4.9 requires this number to exist before that finding may emit, and requires the sufficiency threshold to be stated **as a number, before the measurement**.

**Governing rules:** ONTOLOGY §4.6 (deterministic findings), §4.9 (Join Invariant and its guard), §5 (evaluation protocol).

---

## 1. Sufficiency threshold

### POLICY DECISION P1 — the harm threshold

> **Maximum acceptable expected false accusations: 0.10 per manuscript.**

A "false accusation" is one reference reported as never cited that was in fact cited.

**Rationale.** The finding is normally silent, so a false positive converts a correct report into a wholly wrong one with no true positive beside it to dilute the error; and the action it invites is destructive — deleting a reference the author did cite.

**This is a POLICY CHOICE about acceptable harm, not a derived quantity.** The rationale supports the number; it does not compute it. It is recorded as a labelled decision so it cannot later be replaced while implying the mathematics demanded the new value.

### Derivation

The finding is wrong about a reference exactly when **every** citation of that reference was missed by the extractor. So:

```
expected false accusations  =  N_eval × (1 − coverage)

  N_eval    = references that survive the §4.6 guards (dated, surname-shaped,
              unambiguous key) and are therefore eligible to be called uncited
  coverage  = fraction of those references for which at least one of their
              in-text citations was extracted
```

**Why this is exact.** Coverage is measured over **references**, not citation occurrences. A reference counts as covered if at least one genuine in-text citation to it is extracted. Under this definition `(1 − coverage)` is exactly the per-reference failure rate, so `N_eval × (1 − coverage)` is the expected number of wrongly accused references. **No independence assumption is involved.** Were coverage defined per-citation, a reference cited *k* times would require `m^k` and an independence assumption the data does not support.

Setting the left side to 0.10 and solving:

| `N_eval` | required coverage |
|---|---|
| 15 | ≥ 0.9933 |
| 16 | ≥ 0.9938 |
| 20 | ≥ 0.9950 |

> **Threshold: per-reference citation coverage ≥ 0.993.**

At this manuscript's size (`N_eval` measured at 15 in the run that produced precision 0.00), **0.993 permits zero missed references.** That is a deliberate and uncomfortable consequence of the arithmetic, stated here rather than discovered later: one miss out of 15 gives coverage 0.933 and 1.0 expected false accusations — a wrong accusation in essentially every report.

### Why 0.10 expected false accusations, and not more

Two properties of this finding, together:

1. **Its normal correct output is silence.** Most submitted manuscripts cite every reference they list. A false positive therefore does not degrade a mostly-right report — it converts a *correct, silent* report into a *wholly wrong* accusation. There is no true positive alongside it to dilute the error.
2. **The accusation is specific and destructive.** It names an entry and invites the author to delete a reference they did cite. Compare the claim-extraction spike, which tolerated precision 0.85 (~2 bogus findings among 12 on the primary screen): those were additive opinions, and a reader could discount them. This is an instruction to change the bibliography.

### Why not five points lower

| coverage | expected false accusations at `N_eval` = 15 | reading |
|---|---|---|
| 0.993 | 0.10 | one wrong accusation per ~10 manuscripts |
| 0.99 | 0.15 | one per ~7 manuscripts |
| **0.943** | **0.86** | **a wrong accusation in nearly every manuscript** |

Five points below the threshold is not a slightly worse product; it is a finding that is wrong almost every time it speaks. That is the answer to "why not lower", and it is the reason the bar cannot be softened without changing the finding's design instead.

### Acknowledged in advance

This is a very high bar, and **regex-based extraction may not clear it.** Failing this threshold is a legitimate and useful outcome: it would mean the uncited-reference finding cannot be wired on the current extractor, and that the choice is between a materially different extraction approach and abandoning the finding. Recording that expectation now prevents the threshold being renegotiated when the number arrives.

---

## 2. How recall is computed

### Primary metric — decision-relevant

```
coverage = |{ R ∈ EvaluatedRefs : ≥1 ground-truth citation of R was extracted }|
           ─────────────────────────────────────────────────────────────────────
                                |EvaluatedRefs|
```

`EvaluatedRefs` = references for which `classify_reference_use` returns `Cited` or `Uncited` (i.e. not `NotEvaluated`). This is the only metric the threshold applies to, because it is the only one that maps to the consequence.

### Secondary metrics — diagnostic, not gating

```
key-level recall        = |extracted keys ∩ GT keys| / |GT keys|
                          key = (author surname, year)

occurrence-level recall = matched GT occurrences / total non-ambiguous GT occurrences
```

Reported for error analysis. **They do not gate anything.** A high key-level recall with a failed coverage figure still fails.

### What counts as a match

- **Occurrence match.** An extracted `Citation` matches a ground-truth occurrence iff **the years are equal** AND **the surname sets intersect** (extracted surnames per `citations.rs::surnames`; ground-truth surnames as recorded by hand). Each extracted `Citation` may satisfy at most one ground-truth occurrence; matching proceeds greedily in document order.
- **Key match.** As above, ignoring position.
- **Reference coverage.** A reference `R` is covered iff at least one ground-truth occurrence attributed to `R` was matched.
- Surname comparison is lowercased. Year must be exact — a ±1 year is a miss, not a match.

---

## 3. Exclusion rules — fixed in advance

These exist so that recall cannot be improved after the fact by shrinking the denominator instead of improving the extractor.

### E1 — Reference-list entries are not citations
Only in-text mentions in the body count. The bibliography itself is out of the denominator.

### E2 — Ambiguous, counted but NOT scored
Recorded in their own bucket, excluded from both numerator and denominator, and reported as an ambiguity rate:

- **E2a** A name-and-year that could be either a citation or ordinary prose (an author self-mention, an organisation, a product name with a date).
- **E2b** A mention whose year is illegible, absent, or damaged in the PDF text layer.
- **E2c** A mention inside a table or figure caption where the PDF's extraction order makes attribution to a sentence unclear.
- **E2d** A name with no year within the same sentence.
- **E2e** A mention where the hand-counter cannot determine, from the PDF alone, which bibliography entry it refers to.

### E3 — Out of scope for v1
- Numeric and superscript citation styles (`[1]`, `¹`). This manuscript is author-year; numeric requires reference **numbers** parsed from entries, which `parse_reference` does not do (ARCHITECTURE_TRACE §11.4). A separate protocol.
- Footnote-style citations.
- Citations in supplementary material not present in the PDF.

### E4 — Duplicate mentions
The same `(surname, year)` repeated within a single sentence counts **once**.

### E5 — Non-manuscript pages
The test PDF is wrapped in an AI-detection report (cover pages, guidelines, score legend) and carries running headers and page footers. None of it is manuscript content; none of it enters the ground truth.

### E6 — The denominator is frozen
Once the ground-truth file is hashed, **no item may be reclassified.** If an item is later judged mis-annotated, that is a **protocol defect**: it is recorded as such, and the measurement is re-run in full under Protocol v2. It is never silently patched, and the v1 number is never quietly restated.

### E7 — Underspecification halts the measurement
If applying this protocol requires a judgement it does not cover, the measurement **stops** and the gap is reported. The ambiguity is not resolved in favour of a number. (ONTOLOGY §4.9's guard: "sufficient" must not be defined by whatever the measurement returns.)

---

## 4. Freeze

| | |
|---|---|
| Protocol version | **v1** |
| Frozen | 2026-08-03 |
| Repository | `e854e85` |
| Manuscript under evaluation | `IJAS Manuscript JHA Bombyx haemolymph (1).pdf`, sha256 `859880647c4579c34bc63b82c2280ab08bceac917a9547e0c839a821bc5bafd7` |
| Ground truth | not yet counted — to be frozen separately, after this file is hashed |
| Policy decisions | **P1** (harm threshold, §1) |
| Decision rules | **D1** (§6) |
| Extractor state at freeze | `extract_in_text` unmodified; `narrative_cite` (`extract/stats.rs:84`) and `paren_group` (`:88`) defects known but **not yet fixed** |

**Every result produced under this document must be labelled "Protocol v1".**

---

## 5. Standing caveat on any number this protocol produces

Whatever recall figure results, it is measured on **one manuscript, in one citation style, from one field, by one annotator who is also the implementer.**

The honest conclusion available from it is:

> *"Measured coverage on this manuscript under Protocol v1."*

**Not** *"citation extractor recall."* A single-manuscript figure is sufficient to **fail** the threshold — one manuscript can prove insufficiency — but it is **not** sufficient to **pass** it. Clearing 0.993 here licenses a multi-manuscript evaluation, not a wiring decision.

### Statistical reach of a single manuscript

**Rule of three (a planning approximation, not a proven requirement).** Zero failures in *n* trials puts the 95% upper bound on the failure rate at approximately `3/n`. At `n` = 15 references, a perfect 15/15 gives a 95% **lower** bound on coverage of roughly **0.80** — consistent with a true failure rate many times the 0.993 threshold.

Using the Rule of Three as a planning approximation, demonstrating a failure rate below 0.7% with zero observed failures would require roughly **430 evaluated references**, i.e. **~25–30 manuscripts** at 15–20 references each. This estimate motivates a multi-manuscript evaluation; it is not a proven requirement, and the Rule of Three is itself an approximation for the zero-failure case.

**Measurement resolution.** At `N_eval` = 15 the achievable coverage values are 1.000, 0.933, 0.867, … The threshold sits between the first two, so the single-manuscript test is **pass/fail on perfection**. 0.993 is not distinguishable from 1.0 at this sample size — **do not read "measured 1.0, exceeds 0.993" as clearing the bar with margin.**

---

## 6. DECISION RULE D1 — frozen before any data

| Observed coverage | Consequence |
|---|---|
| **Below** threshold | **Reject wiring.** |
| **Meets** threshold | Proceed **only** to a larger multi-manuscript evaluation. |

**The finding is not wired on Protocol v1 alone under any outcome.**

---

## 7. DECISION MATRIX D2 — how the three layers combine

Frozen before any data.

| Reference-side coverage | Citation-side coverage | Join correctness | Decision |
|---|---|---|---|
| Insufficient | Any | Any | **Reject wiring** |
| Sufficient | Insufficient | Any | **Reject wiring** |
| Sufficient | Sufficient | Insufficient | **Reject wiring** |
| Sufficient | Sufficient | Sufficient | Proceed **only** to multi-manuscript evaluation |

**None of the three dimensions can compensate for another.** A perfect citation-side score cannot rescue poor reference-side coverage; perfect coverage on both sides cannot rescue incorrect joins. The finding is wrong in a user-visible way if any one layer fails, and the failures do not average.

D1 (§6) still governs the final row: **the finding is not wired on Protocol v1 alone under any outcome.**

### Protocol limitation, flagged now rather than after

**Only citation-side coverage has a stated numeric threshold** — 0.993, derived from policy decision P1.

**Reference-side coverage and join correctness have NO stated thresholds.** Under ONTOLOGY §4.9's guard, a sufficiency threshold must be stated as a number *before* it is measured. If this run produces figures for those two dimensions, **those figures cannot retroactively define their own bars.** Any such number is reported as an observation only, and the corresponding row of D2 is answered "threshold not stated" rather than "sufficient".

This is a real limitation of Protocol v1, not a formality: it means v1 can **reject** wiring on any of the three layers, but can **confirm** sufficiency on only one of them.

---

## 8. Error categories — mutually exclusive

Every error lands in **exactly one** category.

| # | Category | Definition |
|---|---|---|
| 1 | **Reference-side coverage** | A bibliography entry never became an evaluable row |
| 2 | **Citation-side coverage** | No citation was extracted for an evaluable reference |
| 3 | **Join correctness** | A citation WAS extracted, but matched to the wrong reference or key |
| 4 | **Protocol / annotation** | Ambiguity or protocol limitation (E2 / E7) |

**Category 3 is the one previously collapsed into coverage.** `"Gordon and Burford (1984)"` extracting as `"Burford"` is a **join failure**, not a miss: something *was* extracted, and attached to the wrong entity. Counting it as a coverage miss would misdirect the fix toward the extractor's yield when the defect is in what it yields.

Keeping the categories exclusive is what makes the before/after comparison informative rather than a single moving percentage: a fix can improve one category and regress another, and a combined number would hide that.
