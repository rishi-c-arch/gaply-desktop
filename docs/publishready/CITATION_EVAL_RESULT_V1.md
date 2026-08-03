# Citation Extraction Evaluation — Result, Protocol v1

| Artifact | sha256 |
|---|---|
| `CITATION_EVAL_PROTOCOL_V1.md` (amended before measurement) | `e931b9e1a373124d70e2060eb0ff037b90d9d87e1737bc1978f9575763e17e76` |
| `CITATION_GROUND_TRUTH_V1.md` | `18dda4291f7bcc3cfb73c6c9d2b1f8a3d1c9b2c4ded41153016365ae08203068` |

Measured 2026-08-03. Manuscript `IJAS Manuscript JHA Bombyx haemolymph (1).pdf`, sha256 `859880647c4579c34bc63b82c2280ab08bceac917a9547e0c839a821bc5bafd7`. **No E7 halt** — the protocol covered every judgement the measurement required.

---

## 1. The three layers

| Layer | Metric | BEFORE | AFTER | Threshold |
|---|---|---|---|---|
| **1 — reference-side** | GT entries that became evaluable rows | **13/26 = 0.5000** | **13/26 = 0.5000** | **not stated** |
| **2 — citation-side** | coverage over `EvaluatedRefs` (primary) | **12/13 = 0.9231** | **13/13 = 1.0000** | **0.993** |
| | key coverage | 24/26 = 0.9231 | 26/26 = 1.0000 | diagnostic |
| | occurrence recall | 28/31 = 0.9032 | 31/31 = 1.0000 | diagnostic |
| **3 — join correctness** | correct verdicts on evaluable rows | **12/13** | **13/13** | **not stated** |
| | finding output | **1 false accusation** | **0** | ground truth: 0 uncited |

---

## 2. Decision — D2 row 1: **REJECT WIRING**

Reference-side coverage cannot be recorded as *Sufficient*: **no threshold was stated for it**, and under ONTOLOGY §4.9's guard the observed 0.50 cannot retroactively define its own bar. D1 governs independently — the finding is not wired on Protocol v1 alone under any outcome.

**The layer that was fixed is now perfect, and it does not matter.** Half the bibliography never becomes evaluable, so the finding is blind to 13 of 26 entries. Layer 2's 1.0000 is measured over the 13 rows that survived, not over the manuscript's 26 references.

---

## 3. Error categories — mutually exclusive

| # | Category | BEFORE | AFTER |
|---|---|---|---|
| 1 | Reference-side coverage | **13** entries lost | **13** — unchanged |
| 2 | Citation-side coverage | 2 keys / 3 occurrences | **0** |
| 3 | Join correctness | 1 wrong verdict (`miranda 2002` → Uncited, truth Cited) | **0** |
| 4 | Protocol / annotation | 0 | 0 |

Entries lost to category 1: `slama 1966 · trivedy 1993 · mamatha 2006 · bizhannia 2005 · nair 2009 · etebari 2005 · moore 1954 · miller 1959 · schmidt 1980 · reitman 1957 · liu 2023 · suzuki 2023 · nair 2003`.

---

## 4. Attribution of the improvement

**`paren_group` (`extract/stats.rs:88`) accounts for the entire measured effect.** Verified by running the fix in isolation: coverage 1.0000, key coverage 26/26, occurrence recall 31/31, zero false accusations, suite green — identical to running both fixes together.

**`narrative_cite` (`extract/stats.rs:84`) had zero measured effect on this manuscript.** The defect is real — `and`/`&` is matched without consuming the surname after it, so `"Gordon and Burford (1984)"` extracts as `"Burford"` — but the matcher committed at `b8312b0` keys on *every* author surname, so `burford` matched `"Gordon R and Burford I R"` regardless. It is a correctness fix to `Citation.authors` content with **no measured outcome improvement**, and it is committed separately so the measured benefit is not attributed to both.

---

## 5. Blast radius — measured, not reasoned

| Consumer | BEFORE | AFTER | Finding changes? |
|---|---|---|---|
| citation count | 28 | 31 | — |
| `citation_density` (`ai_signals.rs:360`) | 5.443 | 6.026 | **No** — both above the 5.0 cut; no finding either way |
| `citation_style_consistency` (`ai_signals.rs:361`) | 0.607 | 0.548 | **No** — both below 0.9; the finding fires in both, only its displayed percentage moves (61% → 55%) |

**No existing finding appears or disappears.** Suite green at **530 passed, 0 failed**.

---

## 6. Caveats — verbatim from the protocol

**Measurement resolution.** At `N_eval` = 13 the achievable coverage values are 1.000, 0.923, … The threshold sits between the first two, so the single-manuscript test is pass/fail on perfection. **1.0000 is not distinguishable from 0.993 at this sample size — do not read "measured 1.0, exceeds 0.993" as clearing the bar with margin.**

**Rule of three (a planning approximation, not a proven requirement).** Zero failures in *n* trials puts the 95% upper bound on the failure rate at approximately `3/n`. A perfect 13/13 gives a 95% lower bound on coverage of roughly **0.80** — consistent with a true failure rate many times the 0.993 threshold. Demonstrating a failure rate below 0.7% with zero observed failures would require roughly **430 evaluated references**, i.e. **~25–30 manuscripts**.

**Scope.** This is coverage on **one manuscript, in one citation style, from one field, counted by one annotator who is also the implementer.** The honest statement is *"measured coverage on this manuscript under Protocol v1"* — **not** *"citation extractor recall."* A single manuscript can **fail** the threshold; it cannot **pass** it.

---

## 7. Revision sequence — evidence superseding hypotheses

Successive measurements revised the limiting factor:

1. **Assumed low-cost deterministic finding** (ARCHITECTURE_TRACE §11.4) →
2. **Paragraph reconstruction dependency** — `docparse` yielded one paragraph per physical line on PDFs; 81 `Reference` rows for 20 references →
3. **Citation-side coverage dependency** — precision 0.00, six false accusations, reference side guarded and citation side unguarded →
4. **Reference-side parsing dependency** — citation side now 1.0000; 13 of 26 bibliography entries never become evaluable.

**Each revision superseded the previous hypothesis on new measurement.** This is evidence replacing hypotheses, not an estimate degrading: at no point was a number revised downward: at each point a *different* limiting factor was identified, and the earlier one was genuinely resolved.

---

## 8. Where the limitation now sits

**26 → 20 → 13 is TWO separate losses**, with different causes, different fixes, and different correctness status:

- **26 → 20**: `parse_reference_list` (`extract/citations.rs`) merging bibliography entries.
- **20 → 13**: the §4.6 guards refusing rows — which may be **correct behaviour**, not a defect, since refusing an unidentifiable entry is what the guards exist to do.

A single "50% coverage" figure hides which of the two is limiting. They must be traced independently before either is treated as a defect.
