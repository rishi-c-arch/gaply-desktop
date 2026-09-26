# D236 — Which evidence may override a verdict

**Measurement only.** No code changed, no verdict behaviour changed, no hard
constraint weakened or removed. This document classifies every finding that
can or cannot force the report's verdict, measures what each does today, and
describes the authority its evidence justifies. The proposed model at the end
is descriptive and **NOT implemented**.

Baseline, not rediscovered: §11 D234 (the `MathematicallyCertain` tier is a
sort key, a colour and a payload word) and D235 (the override is keyed on the
lane: `adapters::from_validation` sets `hard_constraint: true` unconditionally,
four rules can force `concern`, three of six corpus manuscripts are flipped, a
constructed well-reported paper flips on "confirmed" alone).

The framing is not "hard constraints are bad". The evidence says lane-level
authority is too coarse. A recomputed arithmetic contradiction has LESS
authority today than a word match, and the record justifies the opposite.

## Provenance

* HEAD `db9a4499a869b84c1a8e3227443bfa2b8cbf5084`, 26 Sep 2026 22:21Z, working
  tree clean under `src-tauri/` (only `public/sitemap.xml` modified).
* `[probe]` = a throwaway app-crate example, not committed, sha256
  `4bbd2cd105d612158432c6a332290c0e4354a1e734ab64ea2049c7df1f26c750`. It drives
  `pipeline::run_pipeline_measured` (the real pipeline: extraction, the five
  local lanes, the round-table debate, `compile_report`) with
  `GAPLY_DISABLE_DEEP=1`, consent denied, `HashEmbedder`, an in-memory
  database; reads `report.verdict`, `report.debate.overridden_by_constraint`
  and the findings; and re-runs `validate::validate` on the run's own
  extraction to attribute flags to rules. Swarm rows call
  `swarm::run_debate` directly with `PrecomputedAgent`s built through the same
  `Opinion` struct the adapters produce. The reviewer rows call
  `build_review_payload`, `gate_reviewer_response` and
  `aggregate_reviewer_verdict` on the probe's own report.
* `[src]` = source inspection, file and line named.
* `[rec]` = an earlier decision record, cited by number.
* Controls: case 0 is a manuscript with no statistic and no equation (expected
  `pass`, no flag); case 1 is D235's constructed paper (expected `concern`,
  overridden, `PValueOverclaim` 1). Both behaved as expected before any other
  row was read.

## Executive finding

1. **Four rules can force the verdict, and none of them recomputes anything.**
   Two are text absences scoped to one paragraph (`MissingEffectSize`,
   `MissingConfidenceInterval`); two are lexical co-occurrences
   (`PValueOverclaim`, `TestGroupMismatch`). Each alone, on a minimal
   manuscript, produced `verdict = concern` with
   `overridden_by_constraint = true` against five passing lanes `[probe]`.
2. **The one class §4.4 names as Tier 0, a recomputed arithmetic
   contradiction, forces nothing.** `Total = 2 + 2 = 5` produced a Major,
   "mathematically certain", `DETECTED` finding in a report whose verdict was
   `pass` `[probe]`. Placed beside a clean statistics pass it still read
   `pass` with the hard constraint's `pass` above it.
3. **The override runs in both directions.** A validation lane with NO flag is
   also a hard constraint. With a soft majority voting `concern` (plagiarism
   0.86, verification 0.74, AI 0.56 against 1.36 for `pass`), the clean
   validation opinion forced `pass`, `overridden_by_constraint = true`
   `[probe]`. "Nothing detected" vetoes a similarity or citation concern.
   D218, the record that would say what a clean pass claims, is reserved and
   not started `[rec]`.
4. **The runtime cannot host a second hard producer.** Two hard opinions that
   disagree return `Err("conflicting hard-constraint verdicts")` `[probe]`.
   So the equation engine cannot simply be given the flag it is described as
   having; a merge rule is a precondition of any change.
5. **A reviewer can disagree and be kept, and it changes nothing.** A reviewer
   response saying `accept` with a grounded issue disputing the absence
   finding passed the gate intact (`Accept`, one issue kept, no warnings); the
   report's verdict stayed `concern`, the payload had already sent
   `overall_verdict: "concern"` to the model, and the Box 4 aggregation over
   the same findings said `MajorRevision / 0.3` because the Major absence
   finding counts `[probe]`.
6. **`EpistemicStatus::Contradicted`, the status whose name is "established to
   be false", has no producer anywhere in the crate** `[src]`. The equation
   checker emits `Detected`, never `Contradicted`.
7. **The architecture's Tier-0 list (arithmetic, equation equivalence, unit
   checks, CI/SE/SD recomputation, table totals, N consistency) has zero
   members with a vote.** The only recomputation against data in the product,
   `stats_verdict::VerifiedResult`, lives on the Stats Verifier screen and
   never enters the pipeline `[src]`.

## 1. The four rules that can force a verdict

Source: `validate.rs` lines 224–293 `[src]`. The label column is the
user-facing certainty label after D219 `[rec]`. "Wrong while correct" means
the detector fired on exactly what it looks for and the manuscript is
nonetheless fine.

### `TestGroupMismatch` (Critical)

* **Observes.** A `Stat::Test` whose name is `t-test` at location L, and the
  largest match of `(\d+|two…ten)[\s-]+(groups?|arms?|cohorts?|conditions?)`
  anywhere in the paragraph at L being ≥ 3. The test's statistic, its groups,
  and any correction are not read.
* **Claims.** *"A t-test compares exactly two groups, but the surrounding text
  refers to N groups. Comparing 3+ groups with pairwise t-tests inflates the
  false-positive rate; use ANOVA."*
* **Evidence.** Co-occurrence of two lexical facts in one paragraph. Nothing
  links the test to the count.
* **Kind.** Lexical interpretation. Not recomputation, not absence.
* **Wrong while correct.** Yes. *"The three cohorts were compared by one-way
  ANOVA; pairwise differences were then tested with Bonferroni-corrected
  t-tests"* fires, and describes a correct analysis. *"Two groups were compared
  with a t-test; the three conditions were pooled"* fires. D216: zero firings
  on six real manuscripts, so its precision is unmeasured; its own label since
  D219 says *"whether the test spanned those groups was not checked"* `[rec]`.
* **Veto justified?** No, on the evidence as it stands. A veto is justified
  when a firing entails the defect. Here entailment has never been measured on
  real text and the rule cannot see the two facts (which groups the test
  spanned, whether a correction was applied) that decide it. The underlying
  defect, if confirmed by a reader, is serious; the rule does not confirm it.
  Case 4 below is the first measured flip this rule has ever produced, and it
  is on a constructed sentence.

### `PValueOverclaim` (Major)

* **Observes.** A `Stat::PValue` at L (value and operator unread) and the
  first match of `proves?|proven|proved|confirms?|confirmed|conclusively|definitively`
  anywhere in the paragraph at L.
* **Claims.** *"Overclaiming language ("X") appears alongside a p-value. A
  p-value quantifies evidence against the null hypothesis; it cannot prove,
  confirm, or conclusively establish a hypothesis."*
* **Evidence.** A word in a paragraph. The sentence the word is in is not
  consulted; the p-value it is supposedly predicated of is not identified.
* **Kind.** Lexical interpretation.
* **Wrong while correct.** Yes, measured: D216, 5 real firings, the evidence
  entails the finding in 0, and in 4 the word shared no sentence with any
  p-value (*"the Firth sensitivity analysis confirmed that all associations
  were consistent in direction"*, *"Herohalli … proved to be the most
  degraded"*) `[rec]`. Case 1 below: a paper reporting t, p, d, a CI and a
  stated normality check flips on "confirmed" in a neighbouring sentence.
* **Veto justified?** No. See §5.

### `MissingEffectSize` (Major)

* **Observes.** A `Stat::PValue` at L, and `has_effect_size_in` returning
  false for the paragraph region of L, where an effect size is a typed
  `Stat::EffectSize` with a value (§48–§49: a typed region query, not a text
  scan).
* **Claims.** *"A p-value is reported without an accompanying effect size
  (e.g. Cohen's d, eta-squared, r, odds ratio). Statistical significance does
  not convey the magnitude or practical importance of an effect."*
* **Evidence.** The absence of one typed extraction inside one paragraph.
* **Kind.** Text absence, paragraph-scoped, conditional on the extractor's
  recall of effect-size forms.
* **Wrong while correct.** Yes, by at least five independent routes: the
  effect size is in the next paragraph or a table (the region is one
  paragraph, pinned by `the_region_query_is_scoped_to_the_region_it_is_given`);
  it is written in a form the extractor does not type (a mean difference with
  units, a percentage-point change without a named measure); the p-value is
  from a diagnostic test for which no effect size is conventional (D216's F1:
  *"The Box–Tidwell test confirmed linearity … (p = 0.34)"*; a Shapiro–Wilk
  p); the "p-value" is a declared criterion read as a result (D216's A4 miss,
  still live); or the journal simply does not require one (D214: no
  requirement and no expectation row in the seed for any of the ten journals;
  the only route is CONSORT 17a, bound to trial designs at seven journals and
  already checked on the checklist with its source) `[rec]`. On final-L it
  fired 42 times, once per p-value location, on a manuscript whose statistics
  are Kruskal–Wallis and Holm-adjusted comparisons `[rec D235]`.
* **Veto justified?** No. An absence inside a paragraph is evidence for a
  reporting-completeness row, and the product already labels it *"not
  detected by an automated check"*. It cannot establish that the manuscript
  lacks the quantity, only that the detector did not find it where it looked.

### `MissingConfidenceInterval` (Major)

* **Observes.** A `Stat::PValue` at L with `L.section ∈ {Abstract, Results}`,
  and no `Stat::ConfidenceInterval` whose `Location` key equals L exactly
  (`ci_locs.contains(loc)`).
* **Claims.** *"A primary statistical claim reports a p-value but no
  confidence interval. Report a CI so readers can judge the precision and
  plausible range of the estimate…"*
* **Evidence.** Absence of a typed extraction at one location key, plus the
  structural assumption that Abstract/Results p-values are "primary claims".
* **Kind.** Text absence, location-key scoped.
* **Wrong while correct.** Yes: every route above, plus section misfiling
  (D215: final-L's results passage classified as `abstract` ¶26 `[rec]`),
  CIs reported in a table or the following paragraph, and diagnostic
  p-values (a normality test has no estimate to bound).
* **Veto justified?** No, for the same reason as `MissingEffectSize`.

**Summary of §1.** Two absences and two lexical signals. None recomputes a
number. Each has at least one measured or structurally certain way to be
correct-as-detector and wrong-as-claim. Their labels, since D214–D219,
describe the detection and not the manuscript. The override treats each as a
deterministic verdict about the manuscript.

## 2. Equation findings, by status

Source: `equation/check.rs` `check_claim` (lines 213–406), `epistemic.rs`
`Agreement::status` (lines 165–171), `equation_report.rs`, `report.rs`
`equation_findings` (lines 1117–1145) `[src]`. "Runtime" is what the report
path does; every equation finding is built AFTER `run_debate` returns and
produces no `Opinion`, so none can reach `consensus`.

| status | what is computed | recomputed from numbers? | can the source text disprove it? | architecture says | runtime does |
|---|---|---|---|---|---|
| `Confirmed` | both sides evaluate to the same exact rational, or the reported side is the computed side rounded at its displayed precision; or an identity is syntactically identical | yes (numeric) / no (identity) | n/a | a check that passed | not reported (`is_finding` false) |
| `Supported` | an identity agreed at every point sampled | no: sampled, "evidence, not a proof" | n/a | a check that passed | not reported |
| `Unverified` | a definition (sides share no variable), an unbound name, a non-computable rounded reading, or an undetermined comparison | no | n/a | §6b.3: stated as such, never guessed | not reported |
| `Detected` | numeric: both sides compute, differ as written, AND the rounded-interval readings do not overlap; symbolic: an identity differs at a witness point | **yes**, exact rational arithmetic on the manuscript's own numbers | the recomputation cannot be disproved by the text it recomputes; the **parse** can be. The finding quotes both sides verbatim and the trail, so a reader can see a misread (`×` as a name, juxtaposition, a unit annotation) in one glance. D157 recorded four fabricated Tier-0 findings, all from the checker; `defined_quantity` closed that class `[rec]`. The false-`Detected` rate after that fix is **unmeasured** | §4.4: Tier 0, "overrides everything above it"; §6b: the hard constraint that symbolic computation verifies; `report.rs:650` names `Opinion::hard_constraint` as one of its "two correct homes" | Major, `mathematically_certain`, sorted first, **no vote**. Case 5: verdict `pass`. Case 7: verdict `pass` beside a clean stats pass |
| `RequiresAuthorConfirmation` | both sides compute, differ as written, rounded intervals overlap (or the impossible `(true, false)`) | yes, both readings shown | it is a question, and the author's answer decides it | `equation_report.rs` header: "not a weaker DETECTED, it is a question"; §6b.3 | Minor, `mathematically_certain`, **no vote**. Case 6: verdict `pass`. Correct as intended |
| `Contradicted` | **nothing produces it.** Defined in `EpistemicStatus` ("Established to be false"), mapped to Major in `equation_report::severity_for`, and no constructor in `gaply-core/src` or `src` assigns it (`grep -rn "EpistemicStatus::Contradicted"`: two consumers, zero producers) | n/a | n/a | the status a veto would key on, by its own doc comment | dead |
| dimension `Inconsistent` | unit algebra over the two sides against declared units | yes (units, not values) | the finding's own detail says *"it can also mean a constant's units were never stated in the text"*, i.e. it names its false-positive mode | §4.4 "unit checks", Tier 0 | Major, `mathematically_certain`, **no vote** |

**The other recomputation in the product.** `stats_verdict::VerifiedResult`
recomputes a test statistic from the user's uploaded data table and compares it
with the reported value within a stated tolerance; `Verdict::Mismatch` is the
only "recomputed contradiction against data" Gaply can make `[src]`. It is
reached from the Stats Verifier commands (`commands.rs:3071`), renders on
`StatsVerifierReport.tsx`, and never enters `run_pipeline_inner`,
`compile_report` or the debate. Authority on the report verdict: none.

**A real contradiction through the pipeline `[probe]`.** Manuscript: the
minimal IMRaD body, Results containing one prose sentence and the line
`Total = 2 + 2 = 5`.

```
validate flags        : none
report.verdict        : pass
combined_confidence   : 1
overridden_by_constraint: false
findings[0]           : [major] tier=mathematically_certain label="mathematically certain"
                        title "Arithmetic detected: Total = 2 + 2 = 5"
                        prov  signal:equation-arithmetic
```

The reader sees `Verdict: PASS` above a Major "mathematically certain"
arithmetic mismatch. The same line with a fully reported statistic beside it
(case 7) gives the same `pass`, now with the hard-constraint `pass` from the
clean validation lane sitting above the contradiction.

## 4. Probe results (every case alone)

All rows `[probe]`, one run each, release build, identical harness. "flags" is
`validate::validate` on the run's own extraction. "override" is
`report.debate.overridden_by_constraint`, which is true only when the hard
answer displaced a DIFFERENT soft winner.

| case | Results text (in the same 5-section body) | flags by rule | verdict | override | finding that carries it |
|---|---|---|---|---|---|
| 0 clean (control) | one prose sentence | none | **pass** | false | none from validation |
| 1 D235's paper (known-good) | *"The sensitivity analysis confirmed the main result. Sleep improved recall (t(47) = 3.2, p = 0.002, d = 0.46, 95% CI [0.17, 0.75])."* | `PValueOverclaim` 1 | **concern** | **true** | Major, label *"a listed word appears in the same paragraph as a p-value; its meaning was not assessed"* |
| 2 `MissingEffectSize` alone | *"Sleep improved recall (t(47) = 3.2, p = 0.002, 95% CI [0.17, 0.75])."* | `MissingEffectSize` 1 | **concern** | **true** | Major, label *"not detected by an automated check"* |
| 3 `MissingConfidenceInterval` alone | *"Sleep improved recall (t(47) = 3.2, p = 0.002, d = 0.46)."* | `MissingConfidenceInterval` 1 | **concern** | **true** | Major, label *"not detected by an automated check"* |
| 4 `TestGroupMismatch` alone | *"We compared three groups with a t-test (p = 0.01, Cohen's d = 0.4, 95% CI: 0.1 to 0.7)."* | `TestGroupMismatch` 1 | **concern** | **true** | **Critical**, label *"… whether the test spanned those groups was not checked"* |
| 5 arithmetic contradiction | prose + `Total = 2 + 2 = 5` | none | **pass** | false | Major `DETECTED`, "mathematically certain", no vote |
| 6 author confirmation | prose + the D156 weighted-provision chain (`… = 0.084 + 0.065 + 0.046 + 0.024 = 21.9%`) | none | **pass** | false | Minor `REQUIRES_AUTHOR_CONFIRMATION`, no vote |
| 7 contradiction + clean stats | *"Sleep improved recall (p = 0.002, Cohen's d = 0.46, 95% CI: 0.17 to 0.75)."* + `Total = 2 + 2 = 5` | none | **pass** | false | the hard `pass` from a clean lane, above a Major `DETECTED` |

Every validation finding in rows 1–4 carries `tier = mathematically_certain`
and a label that says the opposite. That pairing is D214–D219's deliberate
state (the tier was kept for sort, colour and "the consensus override"); this
table is what the retained override does with it.

In rows 1–4, 6 and 7 the heuristic AI lane also voted `concern` (weight
0.56); the soft vote was still `pass` (extraction 0.86 + RAG 0.50 against
0.56), so every `true` in the override column is the constraint alone.

### The override at the swarm, both directions `[probe]`

`run_debate` over six `PrecomputedAgent`s. Weights are the rescaled values
`consensus` used.

| row | opinions | answer | combined | override |
|---|---|---|---|---|
| S1 | five lanes `pass` (0.86, 0.56, 0.66, 0.50, 0.68) + validation `concern` **hard** | **concern** | 1.000 | **true** |
| S1′ | the same, `hard_constraint` cleared | pass | 0.765 | false |
| S2 | extraction `pass` 0.86, RAG `pass` 0.50; AI `concern` 0.56, plagiarism `concern` 0.86, verification `concern` 0.74; validation `pass` **hard** | **pass** | 1.000 | **true** |
| S2′ | the same, `hard_constraint` cleared | pass | 0.522 | false |
| S3 | five `pass` + validation `pass` hard + validation `concern` hard | `Err(conflicting hard-constraint verdicts in the same debate)` | | |

S2 is the row D235 did not measure: a soft majority for `concern` (2.16
against 1.36) is displaced by a clean statistics lane. Note S2′: with the
flag cleared the vote still says `pass` at 0.522, because the validation
opinion votes at k = 1.0 with confidence 1.0, so even as a soft voter a clean
pass outweighs plagiarism and verification together. That is a calibration
observation, not this document's question, and it is recorded so the
"cleared" rows are not read as "the soft vote would have said concern".

S3 is structural: `consensus` takes `hard[0].answer` and errors if any other
hard opinion differs. Two independent hard producers with different answers
cannot coexist in one debate today.

### When a reviewer disagrees `[probe]`

Case 2's report (one `MissingEffectSize`, verdict `concern`, overridden).

* `build_review_payload` sent `summary.overall_verdict = "concern"` and
  `findings[0] = {id f1, agent validation_maths, tier mathematically_certain,
  severity major, confidence 1.0, title "statistical rule failed: missing
  effect size", evidence ["rule:MissingEffectSize (MAJOR)", "agent:validation_maths
  (deterministic)"]}`. The model sees the tier word and the verdict, and not
  the label that says "not detected".
* A response `{recommendation: accept, publication_probability: 90, issues:
  [{finding_ref: f1, severity: minor, rationale: "effect size present in
  Table 2; the finding is mistaken"}]}` passed `gate_reviewer_response`:
  `Accept`, 1 issue kept, 0 warnings. The gate checks grounding, not
  agreement, so a reviewer may dispute the constraint freely.
* `report.verdict` after the gate: `concern`. Nothing in the reviewer path
  writes it (`report.rs:1086` is the only writer, from `outcome.result.answer`).
* `aggregate_reviewer_verdict` over the same five findings, hand-mapped with
  `verified: None`: `Computed { MajorRevision, 0.3 }`. f1 counts as a Major
  `ManuscriptDefect`; f3–f5 (two process-state opinions and the AI signal)
  were excluded with reasons. So the Box 4 shadow recommendation is decided
  by the same absence finding, through severity rather than through the
  override.

Two independent surfaces, then, carry the absence rule's authority: the
debate (through `hard_constraint`) and the local recommendation (through
`FindingSeverity::Major`). Fixing the first does not touch the second.

## 3. Verdict-authority matrix

"Today" is measured or read as cited. "Justified" is what the evidence class
itself supports, with the condition that would license it. It is not a design.

| evidence class | example | authority today | authority the evidence justifies |
|---|---|---|---|
| **Recomputed contradiction from the manuscript's own numbers** | `Total = 2 + 2 = 5` → `DETECTED`; a dimension `Inconsistent` | **None on the verdict** (case 5, 7: `pass`). Major finding, sorted first, forwarded to the reviewer as `mathematically_certain`. Counts as Major in Box 4 → `MajorRevision` | **The strongest class in the product and the one §4.4 describes.** A floor on the verdict (it cannot read `pass` while one stands) is justified by the evidence pointers and the trail, which let a reader refute a misparse in one glance. Two conditions before it is licensed: the false-`Detected` rate after D157's fix measured on the corpus; and the finding shown at the verdict, so the reader sees WHICH line set the floor. A ceiling (forcing `pass`) is never justified by it |
| **Recomputed contradiction against uploaded data** | `stats_verdict::Verdict::Mismatch` (reported t ≠ recomputed t beyond tolerance) | **None**; a separate screen, never in the pipeline | Same standing as the row above within its stated tolerance, and stronger because the data is the user's. It is not on the verdict path at all, so today the question does not arise |
| **Structural requirement with a source** | a checklist item: required section absent, CONSORT 17a effect size at a CONSORT-bound journal, a word limit | **None on the debate** (`swarm.rs` never reads a checklist). Checklist rows; the mock reviewer letter's `minor_revision` on `failedChecklist` (mock path only, `makePublishReadyMock`). `review_lens` gives `Blocking` to `TestGroupMismatch`, unreachable from any screen | §4.4 Tier 1: outranks model judgement on ITS claim (tiers 2–4), not the manuscript's verdict. Journal-conditional by construction (D214 found no seeded obligation for effect sizes or CIs outside CONSORT trials), so a requirement's authority is exactly the authority of its source row and no more |
| **Text absence** | `MissingEffectSize`, `MissingConfidenceInterval`; D233's `parametric_test_assumptions_unstated` | **Two of three force `concern`** (cases 2, 3) and, with no flag, force `pass` (S2). D233's row casts no vote by explicit decision | **None at verdict level.** Evidence for a reporting row whose label already says "not detected by an automated check". Five independent ways to be correct-as-detector and wrong-as-claim (§1). It can raise a checklist item where a source obliges the quantity; it cannot establish a defect. D233's treatment is the consistent one |
| **Lexical signal** | `PValueOverclaim`, `TestGroupMismatch`; the stylometry signals; `marginal_significance_language` | **Two force `concern`** (cases 1, 4; one Critical). Stylometry: Minor findings. AI-detection: capped at `info` (D153) | **None.** A word-in-paragraph co-occurrence is a prompt to read the sentence. Measured precision for the one rule that has fired is 0–1 of 5. The codebase has already ruled on this pairing for another lexicon: `no_ethics_statement_in_a_study_with_subjects` was demoted from `Blocking` because *"an undemotable verdict on a lexicon is the wrong pairing"* (`review_lens.rs:977`) |
| **Model judgment** | verification `Refuted` (LLM-derived, harness-gated); plagiarism similarity; the cloud reviewer's recommendation | Soft vote at k 0.6–0.8; the reviewer is gated for grounding and cannot touch the verdict; AI signal `info` | A vote, never a veto, which is what the runtime does. One exception today: a clean validation pass vetoes these votes outright (S2). A model concern should be outvoted by evidence, not silenced by an absence of detection |
| **Author-confirmation question** | `REQUIRES_AUTHOR_CONFIRMATION` on the 21.9% chain | Minor finding, no vote (case 6: `pass`) | **None on the verdict.** A question forces nothing until answered. Correct today |

The matrix has one inversion and one mirror. The inversion: the top row has
no authority and rows four and five have all of it. The mirror: rows four and
five also force `pass`, which no row in the "justified" column supports.

## The exact mismatch between architecture and runtime

| the record says | where | the runtime does | measured |
|---|---|---|---|
| Tier 0 is "arithmetic, equation equivalence, N consistency, table totals, CI/SE/SD recomputation, duplicate references, unit checks", decided by "symbolic and numeric computation", and it "overrides everything above it" | architecture §4.4 | none of the listed items produces an `Opinion`; arithmetic and unit findings are folded in after `run_debate`; CI/SE/SD recomputation exists only on the Stats Verifier screen | cases 5, 6, 7 `pass` |
| "The existing `hard_constraint` flag is Tier 0 and Tier 1" | §4.4 | the flag's only producer is `from_validation`, whose four inputs D214–D219 classify as detections (two absences, two lexical). The Tier-1 producer (the checklist) has no vote | cases 1–4 |
| "any evidence-backed deterministic finding on the same claim sets the floor" | architecture line 810 | the floor is set by the LANE, for the whole manuscript, on any claim, in both directions | S1, S2 |
| "the Validation/Maths agent's deterministic verdicts are never subject to the vote" | `swarm.rs` header item 5 | `answer = if r.passed { pass } else { concern }`, `hard_constraint: true` unconditionally; so a clean pass is also a "deterministic verdict", which D218 reserves as an unmeasured claim | S2 |
| the hard-constraint verdict "already has two correct homes: `Opinion::hard_constraint` and `CertaintyTier::MathematicallyCertain`" | `report.rs:650` | the first home is never populated for equations; the second is a sort key (D234) | case 5 |
| `REQUIRES_AUTHOR_CONFIRMATION` "is a question" | `equation_report.rs` header | no vote | case 6, consistent |
| "deterministic (mathematically certain) findings must be corrected regardless of the overall recommendation" | `synthesize.ts:77`, `ReviewerLetterPanel.tsx` (D220) | the sentence is composed on the mock bridge only (`makePublishReadyMock`); on the live path the letter comes from the Rust outcome. The claim it makes is about equation findings, which the verdict ignores, and is printed beside validation findings, which the verdict obeys | `[src]` |
| `TestGroupMismatch` is `Blocking`: "wrong test, deterministically established. §4.4 puts Tier 0 above every model" | `review_lens.rs:927` | `review_lens` is reachable from no screen; the live authority is the swarm flag, where the rule has the same standing as the other three | `[src]`, D216: 0 real firings |
| `Contradicted` = "Established to be false" | `epistemic.rs:71` | no producer | `[src]` |

The one-sentence form: **the override is granted to a struct that says
"this lane ran a rule", and the record justifies it for a finding that says
"this number does not add up". Those are different things, and today the
first has the authority and the second does not.**

## 5. Is `PValueOverclaim`'s ability to force `concern` defensible?

**No.** Plainly:

* Its measured precision is 0 of 5 entailed, 1 of 5 arguable, 4 of 5
  contradicted by the sentence the word is in (D216). A veto with that record
  is a veto exercised by a word list.
* Its own label, since D219, says *"its meaning was not assessed"*. A finding
  that declares it did not assess meaning cannot at the same time be the one
  thing in the report that is not up for debate.
* It flips a fully reported manuscript alone (case 1: t, p, d, CI, and a
  stated normality check), and the user sees `Verdict: CONCERN · hard
  constraint applied` with no indication that the constraint was one word in a
  neighbouring sentence.
* The codebase's own policy for the same shape, applied elsewhere, is
  against it: a lexicon-gated finding was demoted from undemotable severity
  for exactly this reason (`review_lens.rs`, the ethics-statement row), and
  `PValueOverclaim`'s own row in that (unreachable) policy is `Major`, below
  `Blocking`. The lens layer, which nobody reaches, already grants it less
  authority than the swarm, which everyone reaches.
* On the corpus it is never the sole author of a flip (D235), which is why the
  cost has not been seen live. It is the sole author in case 1, and case 1 is
  how a careful author writes.

Whether it should remain a **finding** at Major is a separate question this
document does not decide. The word list does catch *"this proves the
treatment works (p < 0.001)"*, and a Major row with its paragraph quoted is a
reasonable prompt to read. That is a finding's job, not a verdict's.

## 6. Is "not detected in the manuscript" sufficient evidence for "force the verdict to concern"?

**No, and the two are not the same claim.** "Not detected" is a statement
about a detector's output over a region. "Concern" is a statement about the
manuscript. Between them lie five independent conditions, each of which can
fail with the detector correct:

1. **Recall.** The extractor types specific forms (`Stat::EffectSize` with a
   value; `Stat::ConfidenceInterval`). A quantity written outside those forms
   is absent to the detector and present to a reader.
2. **Scope.** One paragraph for effect sizes, one location key for CIs. A
   table, a following paragraph, or a summary sentence is out of scope.
3. **Applicability.** A diagnostic p-value (normality, linearity,
   goodness-of-fit) has no effect size and no estimate to bound. D216's F1
   is one; the absence rules fire on it.
4. **Obligation.** D214 searched the seed and found no requirement or
   expectation row for effect sizes or CIs at any of ten journals outside
   the CONSORT-bound trial designs, which the checklist already handles with
   a source.
5. **Classification.** A misfiled section (D215) moves a p-value into or out
   of "primary".

The product's own label for these rules already concedes the point: *"not
detected by an automated check"* is what it says on every such finding, and
D233's identically shaped detection was given no vote *because* "a term scan
is a statement about the text". The override is the one place where the
D214–D219 reclassification has not been applied.

And the mirror is worse than the case. "Nothing detected" forcing `pass`
(S2) silences a plagiarism or citation lane that examined something real, on
the strength of four pattern checks finding nothing, when D218 has not yet
said what a clean pass claims. If an absence cannot establish a defect, an
absence of absences cannot establish its absence.

## Proposed authority model — descriptive, NOT implemented

This describes what the evidence supports. No code implements it, no decision
adopts it, and its preconditions are unmeasured.

1. **Key the hard constraint on the evidence class of a finding, not on the
   lane.** An opinion is hard only when it is built from a recomputed
   contradiction: an equation `DETECTED`, a dimension `Inconsistent`, or a
   future `stats_verdict::Mismatch` if that ever reaches the pipeline. The
   four `validate.rs` rules do not qualify; D233's row does not qualify;
   `REQUIRES_AUTHOR_CONFIRMATION` does not qualify.
2. **A hard constraint is a floor, never a ceiling.** It can hold the verdict
   at `concern`; it cannot force `pass`. A clean validation lane votes softly
   (or not at all; see 4) and cannot veto another lane's concern.
3. **Multiple hard producers merge, they do not conflict.** Today's `Err` in
   `consensus` must become a rule (any hard `concern` floors the verdict;
   hard opinions never disagree because none says `pass`). This is a
   precondition, not a refinement: S3 shows a second producer crashes the
   debate as the code stands.
4. **The four rules become soft, and the corpus decides how soft.** As soft
   voters at k = 1.0 and confidence 1.0 they would still outweigh most lanes
   (S2′). The candidate weights and the alternative (a finding with no vote,
   as D233 chose) are a measurement to run over the twenty-manuscript corpus
   before anyone picks. Expected consequence, stated plainly: the three
   corpus flips in D235 would revert to `pass` under any soft weighting the
   five-lane vote can outvote, and whether that is the right verdict for
   final-L, R PAPER and Health Economics is the decision, not a side effect.
5. **The verdict surface names the floor.** `Verdict: CONCERN · hard
   constraint applied` becomes a pointer to the finding that set it, with the
   quoted line, so the reader can refute a misparse where they see the verdict.
6. **The reviewer payload carries the label, not only the tier.** Today the
   model sees `mathematically_certain` and the verdict, and not "not detected
   by an automated check". Unmeasured what it makes of the word (D19).

Preconditions, all unmeasured today: the false-`DETECTED` rate after D157 on
the corpus; how many equation `DETECTED` findings the six and twenty
manuscripts produce at all; the per-rule soft-vote outcome on the corpus; the
live plagiarism/verification concern rate that S2 would newly let through.

## What must remain unchanged

* The four rules' detection logic, severities, locations and labels
  (D214–D219). This is about their authority, not their content.
* `consensus`'s hard-constraint mechanism itself. The override is the design
  (§4.4); the defect is what it is keyed on.
* `REQUIRES_AUTHOR_CONFIRMATION` casting no vote, and D233's row casting no
  vote. Both are correct and are the model above applied early.
* The equation engine's purity guard (`tests/equation_is_llm_free.rs`) and
  its refusal-over-inference rules. A finding that is about to acquire
  verdict authority must keep the property that makes it trustworthy.
* The reviewer gate's independence from the verdict. A reviewer disagreeing
  is kept, not overruled; that is right.
* The golden captures (`tests/fixtures/report.golden.json`, the app crate's
  pre-research-state capture). Any change to keying is a deliberate golden
  regeneration with its diff read, never a drift.
* `overridden_by_constraint` as the report's record of displacement. It is
  the one field that made this measurable.

## What is unsafe today

1. **A well-reported paper is marked `concern` on one word** (case 1) or on
   a diagnostic p-value's missing effect size (D216 F1's shape), and the
   screen says "hard constraint applied" with nothing to point at. The
   author reads a deterministic verdict; the label under it says the
   opposite.
2. **A wrong equation sits under `Verdict: PASS`** (cases 5, 7). The reader
   is told the manuscript passed above a Major "mathematically certain"
   arithmetic mismatch, and in case 7 the pass is the hard constraint's.
3. **A clean statistics pass vetoes a similarity or citation concern** (S2).
   Not reproduced live (it needs a corpus and a proxy), but the mechanism is
   unconditional and the row is the code's own arithmetic.
4. **The cloud reviewer is sent `overall_verdict: "concern"` and a
   `mathematically_certain` tier produced by a lexical rule**, and what the
   model does with either is unmeasured.
5. **`TestGroupMismatch` is Critical, hard, and has never fired on real
   text.** Its first measured flip is case 4, constructed. An untested veto
   at the highest severity.
6. **Adding a second hard producer crashes the debate** (S3). Anyone who
   reads §4.4 and gives the equation engine the flag it is described as
   having will make every manuscript with both a flag and a clean pass return
   `Err`.
7. **Two surfaces carry the same authority independently** (the override and
   Box 4's severity count), so a fix to one leaves the other.

## What a later commit should implement

In order, each with its measurement first. None of this is done here.

1. Measure the equation engine's `DETECTED` output over the six and the
   twenty manuscripts and hand-read every row for a misparse (the D157
   method). Record the false rate. This licenses or refuses row 1 of the
   matrix.
2. Replace `consensus`'s conflict error with the floor merge (proposal 3),
   pinned by a test with two hard `concern` opinions and one with a hard
   `concern` beside a would-be hard `pass` that no longer exists.
3. Build the hard opinion from recomputed-contradiction findings, in the
   `concern` direction only, and stop `from_validation` setting
   `hard_constraint` (proposal 1, 2). Regenerate the goldens deliberately
   and read the diff: D235's three flips are the expected changes.
4. Decide the four rules' soft standing on the corpus measurement (proposal
   4) as its own record, and apply D233's no-vote treatment or a weight.
5. Put the floor-setting finding on the verdict surface and the label in the
   reviewer payload (proposals 5, 6), with a screen test that reads the
   rendered text.
6. Re-measure D235's table and this document's eight cases; every row's
   expected value is written above.

## Uncertainty, stated

* Eight constructed inputs. The corpus evidence is D235's six manuscripts
  and D216's five firings; neither is a rate.
* Every run used the heuristic AI tier and a denied verification lane. With
  live lanes the soft margins change; the override does not, because it does
  not read them.
* S2 (a clean pass vetoing a concern) was measured at the swarm, not through
  the pipeline; producing it live needs a plagiarism corpus or a proxy.
* The reviewer rows used a hand-written response and a hand-mapped Box 4
  input (`verified: None` throughout). They show the gate's and the
  aggregator's rules, not a model's behaviour.
* The model-side effect of `overall_verdict` and the tier word is
  unmeasured and needs the proxy (audit D19).
* The false-`DETECTED` rate of the equation engine after D157 is unmeasured;
  the matrix's top row is justified conditionally on it.
* `review_lens` and `synthesize.ts` were read, not driven; both are off the
  live path and are cited only as evidence of what the codebase has already
  decided about lexicon-gated authority.
* Who judged the "wrong while correct" routes in §1: Claude, from the source
  and the cited records. No statistician has reviewed them.
