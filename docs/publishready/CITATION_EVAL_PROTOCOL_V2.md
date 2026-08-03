# Citation Extraction Evaluation — Protocol v2

Supersedes [Protocol v1](./CITATION_EVAL_PROTOCOL_V1.md) for the reference-side dimension only. **Everything in v1 that v2 does not restate remains in force**, including policy decision P1, the citation-side threshold 0.993, decision rule D1, decision matrix D2, all seven exclusion rules, and the ground truth (`CITATION_GROUND_TRUTH_V1.md`, sha256 `18dda4291f7bcc3cfb73c6c9d2b1f8a3d1c9b2c4ded41153016365ae08203068`).

---

## 1. Reference-side coverage — a PLANNING CRITERION, not preregistered evidence

> **Reference-side coverage target: ≥ 0.95 of bibliography entries become evaluable rows.**
>
> **This is a planning criterion. It is NOT evidence of P1's class and must never be cited as though it were.**

### Why it is weaker than P1, stated plainly

P1 was set **before** the quantity it bounds was observed. This threshold is set **after** observing 0.50 on one manuscript, and after establishing that every loss on that manuscript was fixable. Under ONTOLOGY §4.9's guard, a number chosen with that knowledge cannot claim the blindness that makes a preregistered threshold evidential.

**Knowing the losses are fixable makes it worse, not better.** A threshold set while the projected value was unknown would merely be uninformed. This one was chosen knowing the projection reached 26/26 — so it is shaped by the answer, which is further from P1's blindness rather than closer. The number below is therefore a target to plan against, not a bar whose satisfaction constitutes evidence.

### Why the two stronger options were unavailable

**Option 1 — derive from an external consequence.** The derivable question exists: the finding reports *"N of M references were checked; K appear uncited"*, and at N ≪ M a user may read *"0 uncited"* as reassurance while much of the bibliography went unexamined. That is a **silence** harm, distinct from P1's **destructive** harm, and it would need its own derivation rather than a transplanted number. But answering it requires evidence about **how users read a two-number disclosure**, and no such evidence exists. Deriving a number from an assumption about reader psychology and presenting it as external justification would be the failure §4.9's guard exists to prevent.

**Option 2 — adopt an external parser-quality benchmark.** No independently established benchmark for bibliography-entry segmentation was found.

**Option 1 becomes available** the moment there is any evidence about how the disclosure is read. Until then this criterion stands, labelled.

### What the criterion is for

Planning and regression detection: a build that drops below 0.95 has regressed and should be investigated. It does **not** license wiring, and it does not combine with D2's "Sufficient" column as evidence.

---

## 2. D2 under v2

Decision matrix D2 (v1 §7) is unchanged. The reference-side column is answered **"planning criterion met / not met"**, never **"Sufficient"**, because sufficiency in D2's sense requires a preregistered threshold and this is not one.

Consequently **v2 can still only confirm sufficiency on the citation-side dimension.** D1 continues to govern: the finding is not wired on any single-manuscript protocol under any outcome.

---

## 3. Join correctness

Still **no stated threshold**, for the same reason and with the same status as reference-side under v1: any number would now be set after observing 26/26. Reported as an observation only.

---

## 4. Freeze

| | |
|---|---|
| Protocol version | **v2** |
| Frozen | 2026-08-03 |
| Supersedes | v1 §1 reference-side only |
| Inherits unchanged | P1, citation-side 0.993, D1, D2, E1–E7, the v1 ground truth |
| Standing caveat | v1 §5 applies in full — one manuscript, one citation style, one field, counted by the implementer |
