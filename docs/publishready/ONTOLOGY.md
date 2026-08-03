# PublishReady Editorial Review Ontology v1.0

**Status:** Draft for review
**Basis:** code traced at commit `36ea74d`; verification pass at `36ea74d`; B0 resolved in `83f192c`
**Governing principle:** *PublishReady must never be more confident than its evidence — and neither may this document.*

Every claim is backed by `file:line` or explicitly marked **[INFERENCE]**. A capability that could not be verified in code is **Not Verified**, never assumed.

This document **extends** the shipped Evidence Model (`gaply-core/src/evidence.rs`, `report.rs`). It does not define a parallel system. Where the existing model already answers a question, it is cited rather than restated.

**Terminology.** The analysis units are **Review Engines**, not "agents" — most are, and will remain, deterministic Rust (graph algorithms, rule engines, statistical validators); only a subset use a language model. "Agent" implies autonomous LLM conversation, which this architecture deliberately rejects. The rename applies to **prose only**: code identifiers (`AgentKind`, `SwarmAgent`, `Finding.agent`, the `agent:` provenance prefix, the `*_agent.rs` filenames) are quoted verbatim throughout, because this document's `file:line` correspondence is its entire value. See ARCHITECTURE_TRACE.md §11 for the design intent behind the term.

---

## 1. Editorial Dimensions

| # | Dimension | The reviewer question it answers |
|---|---|---|
| D1 | Novelty | Does this add something not already known? |
| D2 | Significance | If true, does it matter to the field? |
| D3 | Methodology | Is the design capable of supporting the claims? |
| D4 | Statistics | Are the analyses correct and correctly reported? |
| D5 | Internal Consistency | Do abstract, results, and conclusions agree? |
| D6 | Literature Coverage | Is relevant prior work engaged with? |
| D7 | Citation Accuracy | Do cited works exist and say what is claimed? |
| D8 | Writing Quality | Can a competent reader follow it? |
| D9 | Figures/Tables | Are visual artifacts complete and self-describing? |
| D10 | Journal Fit | Is this the right venue? |
| D11 | Ethics | Are approvals, consent, and conflicts declared? |
| D12 | Reproducibility | Could someone repeat this? |
| D13 | Journal Compliance | Does it satisfy the target journal's stated rules? |
| D14 | Text Reuse | Does this manuscript reuse text, from itself or from other sources? |

---

## 2. Evidence Ontology

`ConfidenceKind` variants are reused verbatim from `evidence.rs:45-57`; `RoutingHint` from `evidence.rs:62-74`. Both are assigned **per-engine** by total functions (`evidence.rs:118-127`, `:131-140`).

| Dim | Evidence source | ConfidenceKind | Verification method | Explainable? | Known failure modes |
|---|---|---|---|---|---|
| D1 | *none* | — | — | — | Unanswerable today (§4) |
| D2 | *none* | — | — | — | Same |
| D3 | `validate.rs:25-31` (2 of 5 rules) | `Deterministic` | Regex over extracted stats | Yes — rule id (`report.rs:240`) | Misses non-standard phrasing |
| D4 | `validate.rs:25-31`; `stats_verify.rs` (unwired) | `Deterministic` | Rule match; recompute vs `statrs` | Yes | Recompute needs user-supplied `AnalysisSpec` + table |
| D5 | *none* | — | — | — | No cross-section comparison exists |
| D6 | `refverify.rs:578-584`, `:545-559` | `RealNative` | HTTP to CrossRef/OpenAlex/S2 | Yes — `Provenance` per source | Fetched but never aggregated |
| D7 | `refverify.rs:862-872` + `verify_agent.rs:230` | `RealNative` | Registry match → gated LLM verdict | Yes — `evidence:` refs | UNKNOWN when registries silent |
| D8 | `ai_features.rs:149-175` via `report.rs` | `DeliberatelyCoarse` | Stylometric thresholds | Yes — `signal:`/`evidence:` | Proficiency-correlated, not quality |
| D9 | `extract/mod.rs:31-36` | `NoSignal` | Caption regex count | Yes — counts in provenance | **Tables only; no figure detection** |
| D10 | *none in PublishReady*; `journal_registry.rs:83-84` exists unwired | — | — | — | Reviewer asked for fit with no fit evidence |
| D11 | `report.rs:522` (COI substring, conditional) | n/a (checklist) | Literal substring | Yes — guideline URL | Fires only if guideline says "conflict" |
| D12 | *none* | — | — | — | No data-availability detection |
| D13 | `report.rs:443-556` | n/a (checklist) | Deterministic keyword/number | Yes — `guideline_source` (`report.rs:134-136`) | Silent absence on phrasing variance |
| D14 | **self:** `plagiarism.rs:155`; **external:** none — Effectively Unavailable (§9.2) | `WiredReal` (`evidence.rs:122`) | Cosine over `HashEmbedder` chunks; session-local for the self arm | Yes — `similarity:` / `match_type:` / `source:` provenance | Bag-of-words, not semantic (B1); external arm has no eligible input (§9.2); top-k saturation (§9.3) |

### 2.1 Where the existing Evidence Model is insufficient

Stated as gaps, not designed around.

**Gap E1 — `ConfidenceKind` is per-engine with no per-finding override.** `confidence_kind()` (`evidence.rs:118-127`) is a total function of `AgentKind`. There is no way to express *"deterministic arithmetic produced by the Verification lane."* Hit concretely in commit `364e106`: the citation-recency finding is deterministic counting, but `Verification` would label it `RealNative`, so it was attributed to `Extraction`/`NoSignal` — the honest choice available, not the accurate one.

**Gap E2 — `CertaintyTier` (`report.rs:56-80`) has no tier for "deterministic but not a hard constraint."** `MathematicallyCertain` is the Maths engine's hard-constraint verdict and sorts first (`report.rs:73-79`). A deterministic table count is certain but must not outrank a statistical rule failure.

**Gap E3 — `Finding` carries no dimension field.** `Finding` (`report.rs:112-127`) carries `agent`, not dimension. Dimension is inferable only from the `signal:` provenance convention introduced in `364e106`. **[INFERENCE]** a dimension tag is the minimal extension; not designed here.

---

## 3. Capability Matrix

Tier: **1** deterministic · **2** manuscript-internal · **3** literature-grounded · **4** editorial synthesis.

Status is one of *Verified Present · Partially Wired · Built, Unwired · Computed, Discarded · Missing · Not Verified*, plus **Effectively Unavailable** — the capability's execution path is reachable, but the required evidence can never be produced in the traced architecture because no production path creates eligible inputs. The capability therefore cannot answer its editorial question despite executing normally. This classification describes the capability, not the user interface — the interface may still present results generated by the execution path. *(§9.2 is the worked example.)*

It separates from the others by a single discriminator each: **Dormant** — execution path never reached; **Configuration-dependent** — becomes available by changing deployment or config; **Partially operational** — some required evidence genuinely produced; **Missing** — implementation absent.

| Capability | Status | Evidence | Quality | Eng. risk | Tier |
|---|---|---|---|---|---|
| Document parsing | Verified Present | `extract/docparse.rs` | Unmeasured | Low | 1 |
| IMRaD section split | Verified Present | `sections.rs:30-43`, `:54-62` | 22 phrases; handles numbering/colon/case | Low | 1 |
| Statistical-claim extraction | Verified Present | `extract/stats.rs:13-18` | 4 types only | Low | 1 |
| Citation/reference parsing | Verified Present | `citations.rs:39-45` | Unmeasured | Low | 1 |
| Table detection | Verified Present | `extract/mod.rs:31-36` | Caption regex | Low | 1 |
| Figure detection | **Missing** | grep: zero | — | Low | 1 |
| 5-rule statistical validator | Verified Present | `validate.rs:25-31` | Deterministic | Low | 1 |
| Statistical recompute engine | Partially Wired | `stats_verify.rs`; only `commands.rs:1454` | Real (`statrs`) | Low | 1 |
| Citation existence/retraction | Verified Present | `refverify.rs:545-575` | Real registries | Med | 3 |
| Citation LLM adjudication | Verified Present | `verify_agent.rs:230` | Harness-gated | Med | 3 |
| Citation currency aggregate | Verified Present | `report.rs` recency block (`364e106`) | Local `Reference.year` | Low | 1 |
| Plagiarism — self-match (cosine) | Verified Present | `plagiarism.rs:155` (`self_plagiarism`, session-local KNN — needs no external corpus); `pipeline.rs:236-237` | D14; **see §4.3 + B1**; refuses "plagiarism" per §4.2 | Low | 2 |
| Plagiarism — external corpus (cosine) | **Effectively Unavailable** | §9.2; `plagiarism.rs:256` | D14; no eligible input exists (§9.2) | Low | 2 |
| Plagiarism — exact/winnowing | **Built, Unwired** | `plagiarism_exact.rs`; only `commands.rs:212` | Real Jaccard | Low | 1 |
| Stylometric writing signals | Verified Present | `ai_signals.rs:333-367` → `report.rs` | Coarse by design | Low | 2 |
| Grammar/readability | **Missing** | grep: zero | — | Med | 2 |
| AI-authorship signal | Verified Present | `ai_detect.rs:432`, `:264-273` | Self-declared low confidence | Low | 2 |
| Guideline fetch + checklist | Verified Present | `guidelines.rs`; `report.rs:443-556` | 4 checks, substring-triggered | Low | 1 |
| Journal registry card (scope) | **Built, Unwired** | `journal_registry.rs:62-97` | Registry-grounded | Low | 3 |
| Journal-fit reasoning | **Built, Unwired** | `gap_finder_agent.rs:1041`, `:1091` | Gated | Med | 3 |
| Cloud reviewer synthesis | Verified Present | `reviewer_agent.rs:318-391` | Gated; ≤12 findings (`:53`) | Med | 4 |
| Deterministic reviewer verdict | **Computed, Discarded** | `commands.rs:677`; unread by UI | Real | Low | 4 |
| Targeted escalation | Partially Wired | `escalation.rs:9-17` | Degrades honestly | High | 4 |
| Evidence Store | Verified Present | `evidence_store.rs` | Real | Low | 1 |
| Provenance UI trail | Verified Present | `ReportViewerPage.tsx:184-187` | Renders full list | Low | 1 |
| Topic/keyword representation | **Missing** | `extract/mod.rs:40-47` | — | High | 3 |
| Internal-consistency check | **Missing** | grep: zero | — | Med | 2 |
| Ethics / data-availability | **Missing** | grep: `NONE` | — | Low | 1 |
| Real semantic embedder | **Missing** | `embed.rs:33`; the impl wired into the traced path (`lib.rs:76`) | — | Med | 2 |

Execution-path verified: grep for `plagiarism_exact|stats_verdict|journal_registry|gap_finder|journal_verify` across `commands.rs:505-680` (the whole `run_publishready` body) returns **0**.

---

## 4. Editorial Decision Rules

### 4.1 The general rule

> **A conclusion may be stated only if the artifact it derives from is present in the evidence actually supplied to the concluding component. If absent, emit the honest empty state — never a plausible value.**

**Instance 1 — removal over caveat.** `novelty_score` / `journal_fit_score` were deleted (`364e106`): `build_review_payload` (`reviewer_agent.rs:318-391`) carries no topic, so nothing could ground them, and `parse_score` (`:395`) could not check them.

**Instance 2 — self-suppression.** `grounded_text` (`reviewer_agent.rs:486-509`) returns `String::new()` and emits `potential_hallucination` when `evidence_ref` is absent.

**Corollary:** where caveat and removal are both available, prefer removal for *numbers*. A caveated number still anchors; a caveated sentence does not.

### 4.2 Per-conclusion requirements

| Conclusion | Minimum evidence | On insufficiency | Must REFUSE to say |
|---|---|---|---|
| "Statistically invalid" | A fired `RuleId` | Silence | Any stats verdict without rule or recompute |
| "Citation refuted" | Registry evidence + gated verdict | `Unknown` | "Fabricated" from a null registry answer |
| "Plagiarised" | A `MatchSpan` ≥ threshold | Overlap signal only | The word *plagiarism* as determination (`plagiarism.rs:37`) |
| "AI-generated" | — | Signal only | Authorship determination |
| "Novel" / "Not novel" | Topic + literature comparison — neither exists | **Emit nothing** | Any novelty claim, scored or prose |
| "Fits this journal" | Topic + `scope` — topic missing | **Emit nothing** | Any fit score or verdict |
| "Publishable" | Aggregate of above | `Unknown` | `reject` with no grounded issue (GATE 2) |

### 4.3 Accuracy corrections — B0 · **RESOLVED in `83f192c`**

Same category as the removed novelty/fit scores. The historical record is preserved below: these strings **were shipping** when this ontology was first written.

Resolved issues remain in the ontology because the historical reasoning is part of the architectural record; deleting them would erase the evidence that motivated the rule.

**Evidence for the correction.** `MatchSpan.similarity` is cosine over `HashEmbedder` (`embed.rs:33-53`), a 384-dim FNV feature-hashing bag-of-words encoder, and the `Embedder` implementation wired into the traced PublishReady execution path (`lib.rs:76` → `pipeline.rs:236-237`). It is word-order-blind (identical word multisets in different order score 1.0) and has no semantic capability (synonyms are invisible). Every label below claimed a property that cosine cannot establish.

| Original (unsupported) | file:line | Evidence actually available | Corrected wording |
|---|---|---|---|
| `"internal duplication (self-plagiarism)"` | `report.rs:42`; `adapters.ts:29` | `MatchSource::SelfManuscript` knows *where*, never *whether reuse was illegitimate* — contradicted `ISOLATION_NOTE` (`plagiarism.rs:42`), which the user never sees (§9.2) | `"internal duplication (same manuscript)"` |
| `"verbatim"` | `report.rs:44`; `adapters.ts:30` | cosine ≥0.98; cannot distinguish reordering | `"near-identical wording"` |
| `"near-verbatim"` | `report.rs:46`; `adapters.ts:31` | cosine ≥0.85, word-order-blind | `"high word overlap"` |
| `"paraphrase"` | `report.rs:48`; `adapters.ts:32` | cosine <0.85 over bag-of-words | `"partial lexical overlap"` |
| `"{label} — {N}% similarity"` | `report.rs:319`; `adapters.ts:49` | lexical cosine | `"{label} — {N}% word overlap"` |
| `"Similarity is a semantic signal…"` | `plagiarism.rs:37-39` | no semantic model exists | `"a lexical-overlap signal"` |

`"paraphrase"` was the clearest case — the inverse of the engine's capability: a genuine paraphrase produces *low* word overlap, scores below `DEFAULT_THRESHOLD = 0.80` (`plagiarism.rs:31`), and is never reported at all.

**Classification: Accuracy Correction → Resolved (`83f192c`).** Both sides are now pinned by tests that fail if either drifts (`match_type_label_wording_is_supported_by_the_algorithm` in `report/tests.rs`; `matchTypeLabel wording` in `checks/plagiarism.vitest.tsx`), and both carry a comment explaining the reasoning so the wording is not "improved" back.

### 4.4 Standing rule — correct the claim, do not caveat it

> **PublishReady must never be more confident than its evidence — in scores, labels, or wording.**
>
> **When a claim exceeds its evidence, correct the claim.** Do not caveat it, do not soften it, do not add a disclaimer beside it. Remove it, or replace it with the strongest thing the evidence genuinely supports.

Four worked precedents illustrate the rule across different classes of user-visible claim. Read them together — the rule is easier to apply from precedent than from principle:

1. **A fabricated quantitative score** — `novelty_score` / `journal_fit_score`, deleted from `reviewer_agent.rs` (instruction at `:77-91`, gate at `:409+`, evaluation struct at `:176+`). The payload (`:318-391`) carried no topic, so nothing could ground them and `parse_score` (`:395`) could not check them. A caveat was rejected because a caveated number still anchors.
2. **Misleading error wording** — the 401 message at `models/proxy_client.rs:271` said `app_check_failed` unconditionally; a 401 has two causes, so it now names both. The claim was narrowed to what the status code actually establishes.
3. **Overstated terminology** — the plagiarism band labels in §4.3 (`report.rs:44-57`, `adapters.ts:30-41`): `"verbatim"` / `"near-verbatim"` / `"paraphrase"` named textual properties a word-order-blind cosine cannot establish, replaced with wording it supports.
4. **An asserted determination** — `"internal duplication (self-plagiarism)"` (`report.rs:42`, `adapters.ts:29`) declared reuse illegitimate, contradicting `ISOLATION_NOTE` (`plagiarism.rs:42`). `MatchSource::SelfManuscript` establishes *where* a match is, never *whether* the reuse was improper. Now `"(same manuscript)"`. *(Two corrections since first written: the two are in different files — `match_type_label` is `report.rs:65` — and the note never reaches the PublishReady report at all (§9.2), so the label shipped with no counterweight the user could see. Both strengthen the case for the correction.)*

The fourth was found outside the commissioned scope, by noticing the same pattern in adjacent code while correcting the third — which is the usual way this class of defect surfaces.

**The rule runs in both directions, and only one of them announces itself.** A fifth precedent is the inverse of the four above: the `narrative_cite` fix (`1693ca6`) was recorded as having *"zero measured effect"*, which was accurate on the evidence then available. Later measurement found an indirect effect the original evaluation had not scoped — it unlocks first-author matching, worth four evaluable references (CITATION_EVAL_RESULT_V1.md §4).

Every other correction in this document found a claim that was **too strong**. This one found a claim that was **too weak**. Understatement is the harder direction to catch: an overclaim eventually contradicts something and prompts a look, while an understatement simply sits there and nothing goes wrong. The correction discipline applies to both, but only overclaims generate the pressure that triggers it — so understatements have to be looked for deliberately, usually when a later measurement widens the scope of an earlier one.

The self-suppressing counterpart is already in the code: `grounded_text` (`reviewer_agent.rs:486-509`) returns empty and flags `potential_hallucination` rather than emitting an ungrounded claim. New editorial fields should adopt that pattern rather than `parse_score`'s.

### 4.5 Standing rule — extraction produces no judgements

The Manuscript Understanding Layer produces structured evidence only. It MUST NOT express editorial judgement, quality assessment, recommendation, or opinion. `sentence 142 → Claim → Causal → reports p = 0.03` is extraction; *"weak claim"*, *"poor methodology"*, *"novel contribution"* are Review Engine decisions.

§4.1 governs *how strongly* a claim about the manuscript may be stated. This rule governs *which component may make one at all*.

It is what makes extraction accuracy and review quality independently measurable. Extraction is scored against a human-annotated ground truth — a claim either is at sentence 142 or it is not. Review quality is scored against editorial agreement: a different question, a different ground truth, a different error bar. Collapsed into one component, neither is measurable — a disputed output could be a mis-extraction or a defensible-but-unpopular judgement, and no experiment separates them.

Design intent recorded in ARCHITECTURE_TRACE.md §11.1; not yet implemented.

### 4.6 Standing rule — deterministic findings must survive three questions

Before any deterministic finding is built, it must answer:

1. **Is the evidence deterministic?**
2. **Is the MAPPING from evidence to finding deterministic?**
3. **Can ambiguity occur?**

If ambiguity exists: **refuse the finding, or downgrade its certainty. Never resolve ambiguity heuristically when silence is more truthful.**

§4.4 governs a claim that exceeds its evidence. This rule governs the step before: a finding whose *derivation* is not as certain as its inputs. Question 2 is the one that gets skipped — deterministic inputs do not make a deterministic finding if the join between them is inferred rather than parsed.

Design intent recorded in ARCHITECTURE_TRACE.md §11; applied first to the uncited-reference finding (§11.4 item 1), which it stopped.

### 4.7 Standing rule — measured bottlenecks outrank architectural elegance

**Runtime architecture is expanded only when profiling identifies a specific constraint that existing structure cannot address.**

Though it is stated for runtime, the rule generalises: it governs any structure built ahead of the evidence that it is needed. It explains three decisions already taken, in both directions:

- **The `docparse` PDF reflow was built** because measurement showed user-visible false output — 81 `Reference` rows for 20 references, page furniture parsed as references, `Location.paragraph` meaning "line index" in the provenance inspector and the exported PDF. A specific constraint, measured, that no existing structure addressed.
- **KV-cache reuse in `PerplexityModel` is worth pursuing** because measurement quantified the redundancy: 3.40×, 358.5 scored token positions where 105.3 would do (ARCHITECTURE_TRACE §12.2). Not elegance — a number.
- **The `KnowledgeExtractor` trait, the capability registry (§11.3) and the Review Engine scheduler (§12.3) were all deferred** because no instances existed to constrain them. A seam designed from zero implementations encodes guesses; from one, it encodes that one's accidents.

The failure mode this prevents is the inverse of §4.4's. There, a claim outran its evidence; here, a structure outruns its need — and unneeded structure is harder to remove than a wrong sentence, because code acquires callers.

### 4.8 Standing rule — memory optimization order

When a memory or throughput constraint is measured, work the list in order:

1. **Reduce computation.**
2. **Reuse computation.**
3. **Reduce memory movement.**
4. **Reduce memory footprint.**
5. **Raise hardware requirements** — only when 1–4 are exhausted.

Two worked examples, both of which genuinely fit:

- **Rule 2 — KV-cache reuse.** The claim-extraction spike re-scores the same prompt prefix once per candidate label: 358.5 scored token positions per sentence where 105.3 would do, a measured **3.40×** redundancy (ARCHITECTURE_TRACE §12.2). The computation is not wasteful in itself — it is simply performed five times instead of once. Reuse, not reduction.
- **Rule 4 — `unload_slm2`.** The Ollama model is explicitly evicted after the verify lane (`models/mod.rs:1008`, called at `pipeline.rs:308`), so its footprint is not held while the next lane runs. Nothing is computed less or reused; the peak is lowered.

The ordering matters because the later steps are the ones that get reached for first — footprint work is visible and feels like progress, while the computation being reduced or reused is often invisible until measured.

### 4.9 Standing rule — the Join Invariant

**A deterministic finding may only be emitted if every required operand has been positively identified.** *Missing* and *unmatched* are distinct states: an operand that was not successfully extracted must propagate to `NotEvaluated`, never to a negative conclusion. **Question 3 of §4.6 must be asked of BOTH sides of a join** — refusing ambiguity on one side does not make the join safe.

**The sharpening, which measurement forced.** For a finding of the form *"X is ABSENT from Y"*, the operand cannot be positively identified — **the operand IS the absence.** Such a finding requires instead positive evidence that **the extractor achieved MEASURED COVERAGE SUFFICIENT FOR THE INTENDED FINDING**: a coverage claim about the extractor, not a claim about an operand, and a strictly higher bar. Absent an established recall figure, *"none found"* means *"none found, and our own recall is unknown"*, which is not grounds for a negative finding.

**Guard on "sufficient".** *Completeness* would be unsatisfiable, and an unsatisfiable rule is one that gets routed around rather than met. *Sufficiency* is satisfiable and forces the right question — sufficient for **what** — but it introduces a judgement where the absolute had none. So: **the sufficiency threshold must be stated as a number BEFORE the measurement, not chosen after**, on the same discipline as §5's evaluation protocol. Otherwise "sufficient" is defined by whatever the measurement happens to return, and the rule becomes a formality that ratifies any result.

#### The measurement that produced this rule

The uncited-reference finding (`report.rs::uncited_reference_findings`, built and deliberately unwired at `b8312b0`) was run on a real manuscript after `docparse::reflow_pdf_text` had corrected bibliography parsing:

> **6 findings, 6 false positives, precision 0.00.**

Every entry it named was cited — `Göncü and Parlak (2011)`, `Gordon and Burford (1984)`, `Kamimura and Kiuchi (1998)`, `Rahmathulla and Suresh (2012)`, `Srivastava and Upadhyay (2015)`, `Miranda et al. 2002` — each verifiable in the body text.

**The reference side was guarded correctly.** All four parse fragments became `NotEvaluated`: two undated (`"Central Silk Board, Bangalore."`, `"Analytical Chemistry 31(3): 426–28."`) and two carrying a DOI URL where the surname belongs. §4.6 worked exactly as designed on the side it was applied to.

**The citation side had no guard**, and that is the whole lesson. Two defects in `extract_in_text` mean a cited work can be absent from `ex.citations`:

- **`narrative_cite` (`extract/stats.rs:84`)** matches `and`/`&` but never consumes the surname that follows, so `"Gordon and Burford (1984)"` is extracted with authors `"Burford"` — the second author.
- **`paren_group` (`extract/stats.rs:88`)** splits only on `;`, so `"(Trivedy et al. 1993, Kamimura and Kiuchi 1998, Miranda et al. 2002, Mamatha et al. 2006)"` collapses to **one** citation.

**Zero true positives were observed.** The run therefore does not establish whether this manuscript contains any genuinely uncited reference — only that the finding was wrong six times out of six. The fixture test (`an_uncited_reference_is_reported_with_both_numbers`) is the *only* evidence the finding can fire at all.

#### Coverage and join correctness are separate failure modes

**Satisfying one says nothing about the other.** Coverage asks *did we find everything?* Join correctness asks *did we match the right things to each other?* A join can be perfect over incomplete coverage, and complete coverage can be joined entirely wrongly.

**Worked example — `registry_year_mismatch`, approved on a coverage argument and deleted on identity.** It compared a manuscript's `Reference.year` against a registry-resolved `matched_year`, and was approved on the argument that §4.9 does not bind because it fires only where **both** operands exist. That argument is true and irrelevant: §4.9 as written governs ABSENCE, and this finding failed on JOIN CORRECTNESS.

Measured on a real manuscript, 6 reported mismatches:

- **5 resolved to a different work.** `refverify` queries CrossRef with `query.bibliographic` and `rows=1`, which returns a single best guess with **no confidence gate and no threshold**. One resolved to a different entry *in the same bibliography*.
- **1 was correct** — DOI-matched, identical title and authors, with a genuine print/online year divergence that is not an author error either.

The comparison code was correct throughout. Every operand was positively identified. The finding still produced five false accusations, because *positively identified* was checked on the **local** side and assumed on the **registry** side.

**The layer had already been named.** `CITATION_EVAL_PROTOCOL_V1` §7 separates reference-side coverage, citation-side coverage and join correctness into a decision matrix precisely because they do not substitute for one another — and that matrix was written four commits before this finding shipped. The rule was available and was not applied.

So: when a finding joins two sources, ask §4.6 question 3 of **each side independently**, and ask separately whether the join itself is trustworthy. An identity resolved by fuzzy search is not an operand that has been positively identified.

#### Scope

The invariant is not about citations. It governs **any extractor that joins independently derived evidence**, and every planned deterministic finding in ARCHITECTURE_TRACE §11.4 has this shape: figures referenced but absent, tables never referenced, ethics statements, funding disclosures, reporting-guideline items. Each asks whether one extracted set covers another, and each will be wrong in exactly this way unless the covering set's recall is known.

§11.5 established that positional correspondence is not identity. This rule establishes the companion: **one-sided refusal is not safety.**

### 4.10 Standing rule — component correctness does not compose

**Component correctness is necessary but not sufficient for report correctness. Composing individually correct components CAN introduce failure modes — such as entity resolution, prioritization, duplication, and presentation — that are not detectable through isolated component measurements alone.**

Every measurement discipline in this document evaluates a component against its own contract. None of them evaluates what the user is handed.

### 4.11 RELEASE GATE — end-to-end author review

> **After any substantial report-generation workstream, perform an end-to-end author review of the assembled report.**
>
> This is a gate, not a recommendation. It complements unit tests, mutation tests, protocol measurements and blast-radius measurements because it is the only stage that evaluates the **composition** of findings rather than their individual correctness.

Read the report as an author would: every finding, in order, as a document — not as test output.

**Evidence for the gate — two DISTINCT failure modes, not one repeated.**

**Case 1 — a false-accusation finding that component-level reasoning did not expose.** With **683 tests green** (`gaply_core` 541, `app` 142), one real manuscript surfaced three defects:

| Defect | Composition failure |
|---|---|
| `registry_year_mismatch` produced 5 false accusations | **entity resolution** — each operand valid, the join untrustworthy |
| 28 of 48 findings identical, plus a 29th restating them | **duplication and presentation** — each finding individually correct |
| An 81% "internal duplication" quoting an AI-detection tool's cover page | **document boundary** — extraction, matching and labelling all correct |

None of the three is a component defect. Each component satisfied its contract; the report did not.

**Case 2 — a regression introduced by a preprocessing change that passed every component test.** Excluding front matter from the text-reuse check removed the cover-page match and took self-matches from 6 to **10**, again with **683 tests green**.

The two cases fail differently. The first is a finding that was wrong about the manuscript; the second is a preprocessing change that was correct in itself and wrong in composition. Neither is reachable from a component contract. The gate is therefore a standing check, and it must run **after** a fix as well as before one.

### 4.12 Standing rule — typed absence, at every layer

**Every durable artifact must distinguish negative evidence from missing evidence. Typed absence is preferable to silent absence, because silence forces a later reader to guess why nothing was recorded.**

This is the same principle at **three layers**, and only the first was ever explicit:

| Layer | How it appears | Status when found |
|---|---|---|
| **FIELD** | `Metric<T>`'s `Observed` / `Unavailable { source, requires }` split (`reviewer_harness.rs:58-63`) — a missing value carries the reason it is missing | Applied deliberately from the start |
| **REPORT** | §9.2's empty-corpus problem — zero plagiarism findings rendered as silence, which a user reads as *"nothing found"* rather than *"nothing could be searched"* | Found by tracing; still open |
| **ARTIFACT** | The Box 4 comparison record was written only when the shadow synthesis produced an outcome, so a run without one left **no file** — indistinguishable from a sink that never worked | Found while planning a baseline capture; fixed |

The artifact case is the most dangerous of the three because it destroys the evidence of its own failure. A field that says `Unavailable` still tells you the run happened. A report that renders nothing at least renders. **A missing file says nothing at all**, and the reader cannot tell an honest negative from a broken instrument — which is precisely the state a baseline would have been captured in.

The rule generalises past this codebase's current artifacts: any log, export, cache entry or telemetry record that is written *conditionally on success* has this defect. If the condition can fail, the record must still be written and must say the condition failed.

---

## 5. Evaluation Protocol

**Standing three-question rule.** No capability is accepted unless all three are answered *before* implementation:

1. **What evidence supports this conclusion?** — name the producing module and the `EvidenceRecord` it emits.
2. **Can that evidence be shown to the user?** — must survive `is_structured_provenance` (`evidence.rs:38-40`) to reach the reviewer, and must render in the provenance trail (`ReportViewerPage.tsx:184-187`).
3. **How will correctness be measured?** — name a benchmark and an expected failure mode.

| Proposed capability | Validating benchmark | Expected failure modes |
|---|---|---|
| Real semantic embedder | Paraphrase pairs (PAWS/MRPC-style); recall at fixed FP rate vs. current | Over-flags topical similarity as reuse |
| Figure detection | Hand-labelled captions across 20 PDFs | Multi-column layouts; captions inside images |
| Ethics/data-availability | 30 papers with/without statements | Phrasing variance; false absence |
| Internal consistency | Papers with known abstract/results mismatch | Requires claim extraction first |
| Topic extraction | Human-assigned field labels, top-k agreement | Interdisciplinary papers |

---

## 6. Roadmap by Evidence ROI

**Tier 0 — accuracy corrections (no new capability)**
0. ~~Correct the plagiarism strings in §4.3 to match the evidence.~~ **Done — `83f192c`.**

**Tier 1 — deterministic, bounded**
1. Wire `plagiarism_exact` into the pipeline (built, unwired)
2. Ethics / data-availability / funding statement detection
3. Figure detection to parity with `TableRef`
4. Aggregate `Enrichment.citation_count` (fetched at `refverify.rs:846`, discarded)
5. Surface `shadow_reviewer` or stop computing it (`commands.rs:677`)

**Tier 2 — manuscript-internal**
6. Real embedder behind `Embedder` (`embed.rs:12-18`)
7. Readability / grammar signal
8. Extend heading vocabulary (Limitations, Data Availability, Ethics, Funding, Statistical Analysis)
9. Internal-consistency checks (depends on claim extraction)

**Tier 3 — literature-grounded · research risk, see §6.1**
10. Topic representation · 11. Literature coverage · 12. Journal fit · 13. Novelty

**Tier 4 — editorial synthesis**
14. Promote `aggregate_reviewer_verdict` to authoritative (Stage 2)
15. Escalation endpoint + Q12 spike (`escalation.rs:9-17`)

### 6.1 Tier 3 — Research Risk

Not engineering tasks with estimable cost.

- **Novelty (D1)** — requires establishing *absence* from the literature. Unbounded: no corpus is complete. *Spike:* 20 papers with known novelty verdicts from reviewer reports; measure whether any retrieval proxy correlates. **Kill criterion: no correlation → do not build.**
- **Significance (D2)** — field-relative value judgement with no ground truth short of citation counts years later. **Recommend permanent exclusion.**
- **Literature coverage (D6)** — needs topic-keyed search plus a notion of "should have cited." *Spike:* 10 papers, compare retrieved candidates against actual reference lists; measure precision of "missing" claims. **Kill criterion: precision < 0.5.**
- **Journal fit (D10)** — least risky Tier 3 item: `scope` exists (`journal_registry.rs:83-84`) and a gated fit lane exists (`gap_finder_agent.rs:1091`). Still needs a manuscript topic that does not breach the privacy invariant. *Spike:* derive topic from headings + title + statistics inventory only (no prose); test on 15 known-fit/known-misfit pairs.

---

## 7. Separation Rule — Evidence vs. Editorial Interpretation

**Rule.** Evidence is *what was measured*; interpretation is *what a reviewer concludes*. Every interpretation must cite at least one evidence id; an interpretation that cannot must not render.

**Already shipping, enforced three ways:**
- `Finding.provenance` is non-empty by contract (`report.rs:124-126`); every finding is built through `paired()` → `at_source` (`report.rs:187`), so `ConfidenceKind`/`RoutingHint`/`limitations` are *derived*, never hand-set.
- The boundary filter (`evidence.rs:38-40`) admits only the nine structured prefixes (`evidence.rs:25-35`).
- `ReviewerIssue.finding_ref` must be in the sent set or the issue is dropped and flagged (`reviewer_agent.rs` GATE 1).

### 7.1 Standing rule — Evidence ID on every user-visible statement

**Rule.** Every user-visible statement should carry a traceable Evidence ID linking it to the evidence and code path that produced it:

```
Finding → Evidence ID (f{N}) → generating file:line → ConfidenceKind → reviewer interpretation
```

This **formalizes and extends the provenance trail already shipping**, not a new mechanism.

**What already satisfies it** (verified):
- The trail exists and is user-facing: `"Provenance · click to see why"` (`ReportViewerPage.tsx:184`), rendering the full provenance list in an inspector (`:185-187`), a `provenance[0] (+N)` summary per finding (`:373-374`), and inclusion in the exported PDF (`exportPdf.ts:32`).
- Stable ids exist server-side: `EvidenceRecord.id = f{N}`, assigned post-sort (`report.rs:406-408`), 1:1 with findings by construction.
- `ConfidenceKind` is already derived per record (`evidence.rs:195`).

**What would need to change** (verified gaps, `ReportViewerPage.tsx:360-378`):
1. **The `f{N}` id is not rendered.** The UI shows title, `certainty_label`, `detail`, and `provenance[0]`. The stable id exists but never reaches the user, so a statement cannot be cited back.
2. **`ConfidenceKind` never reaches the UI.** The report renders `CertaintyTier` (`certainty_label`) only; `report["evidence"]` is not read by the frontend.
3. **No code-path pointer.** Provenance names the rule/agent/signal (`rule:`, `agent:`, `signal:`) but not the generating `file:line`. **[INFERENCE]** adding a `source:` value carrying a stable module identifier would satisfy this within the existing prefix set (`source:` is already permitted, `evidence.rs:33`) — no new prefix required.

---

## 8. Dependency Graph

### 8.1 Upstream capabilities

| Upstream capability | Downstream dims | Leverage | Research risk | Tier | Status |
|---|---|---|---|---|---|
| Document parsing | all 13 | High | Low | 1 | Verified — `docparse.rs` |
| Section splitting | D3,D4,D5,D8,D9,D12,D13 (7) | High | Low | 1 | Verified — `sections.rs:30-43` |
| Statistical extraction | D3,D4,D5 (3) | Med | Low | 1 | Verified — `stats.rs:13-18` |
| Reference parsing | D6,D7,D13 (3) | Med | Low | 1 | Verified — `citations.rs:39-45` |
| Semantic embedding | D6,D13 + plagiarism (3) | High | Med | 2 | **B1** |
| Topic representation | D1,D2,D6,D10 (4) | High | High | 3 | **Missing** |
| Claim extraction | D1,D5,D6 (3) | High | Med | 2 | **Missing** |
| Registry connectors | D6,D7 (2) | Med | Low | 3 | Verified — `refverify.rs` |
| Guideline corpus | D13,D11 (2) | Med | Low | 1 | Verified — `guidelines.rs` |
| Evidence Store | synthesis | Med | Low | 1 | Verified — `evidence_store.rs` |
| Escalation endpoint | Tier 4 | Med | High (server) | 4 | **Missing** — `escalation.rs:9-17` |

### 8.2 Architectural Bottlenecks — ranked by the code

**B0 — Six plagiarism strings exceeded their evidence. RESOLVED in `83f192c`.** §4.3. Never a bottleneck in the capability sense — an honesty defect, not a missing feature. Kept in this list as the worked example of the §4.4 rule.

**B1 — `HashEmbedder` is the only embedder and is not semantic.** `embed.rs:33-53` is a 384-dim FNV feature-hashing bag-of-words encoder; `impl Embedder for` returns exactly one hit repo-wide; production wires it at `lib.rs:76`. Consumers: `plagiarism.rs`, `rag.rs`, `report.rs`. Engineering, bounded — the trait (`embed.rs:12-18`) is already the seam and `EMBEDDING_DIM` already matches MiniLM's 384.

**B6 — `plagiarism_exact` is built and unwired.** Real Jaccard over winnowed fingerprints, honest by construction; the pipeline uses the weaker engine (`pipeline.rs:236-237`). Best quality-per-effort in the matrix: the work exists.

**B7 — Four fetched-or-computed-then-discarded signals.** `Enrichment.citation_count` (`refverify.rs:846`), `ExistenceCheck.matched_year` (`:661`, `:712`), `shadow_reviewer` (`commands.rs:677`), and — until `364e106` — `DocumentFeatures`. Pure aggregation or wiring.

**B3 — No topic representation.** `ExtractionResult` (`extract/mod.rs:40-47`) carries none. Blocks D1, D2, D6, D10. `PaperDigest.summary` is verbatim abstract prose (`paper_corpus.rs:222-235`), so it cannot be reused for the user's own manuscript without breaching the privacy invariant. **Research risk: High.**

**B4 — `ConfidenceKind` cannot express per-finding provenance** (Gap E1). Every new deterministic-but-non-Validation finding hits this. Small engineering change to a shared model; needs a decision.

**B2 — Heading vocabulary gaps.** `sections.rs:30-43` covers 22 phrases and handles numbering, colons, case and plurals. Absent: Limitations, Acknowledgements, Funding, Data Availability, Ethics, Statistical Analysis, Participants. Medium priority — *downgraded from the pre-verification draft, which overstated its fragility.*

**B5 — Escalation has no server endpoint** (`escalation.rs:9-17`). Blocks Tier 4 per-finding adjudication. Server-side, outside this repo.

---

## 9. Codebase Coverage Audit

48 modules across `gaply-core/src/` and `src/`, each classified exactly once.

### Review-contributing

| Module | Purpose | Upstream | Downstream | Dims | Status |
|---|---|---|---|---|---|
| `extract/{mod,docparse,sections,stats,citations,persist}.rs` | Parse → structure | file bytes | everything | all | Verified |
| `validate.rs` | 5 deterministic rules | extract/stats | report | D3,D4 | Verified |
| `ai_detect.rs`, `ai_features.rs`, `ai_signals.rs`, `stage1_norms.rs` | Perplexity + stylometry | extract | report | D8 | Verified |
| `plagiarism.rs` | Lexical-cosine lane | embed, chunk | report | D14 | Verified (B0 resolved; B1 open) |
| `plagiarism_exact.rs`, `plagiarism_library.rs` | Deterministic overlap | — | `commands.rs:212` only | — | Built, Unwired |
| `refverify.rs`, `http_fetcher.rs` | Registry connectors | citations | verify_agent | D6,D7 | Verified |
| `verify_agent.rs`, `models/{proxy_client,ollama_verify}.rs` | Citation adjudication | refverify | report | D7 | Verified |
| `rag.rs`, `embed.rs`, `vector.rs`, `chunk.rs` | Retrieval substrate | — | plagiarism, checklist | D13,D6 | Verified (B1) |
| `guidelines.rs` | Guideline ingest | HTTP | rag → checklist | D13,D11 | Verified |
| `report.rs` | Findings + checklist | all lanes | UI, reviewer | all | Verified |
| `evidence.rs`, `evidence_store.rs`, `orchestrator.rs` | Evidence Model | report | escalation | all | Verified |
| `escalation.rs` ×2 | Targeted escalation | evidence | proxy | all | Partially Wired |
| `reviewer_agent.rs`, `reviewer_synthesis.rs`, `reviewer_harness.rs` | Editorial synthesis | report, store | UI | all | Verified |
| `swarm.rs` | Round-table consensus | lane reports | report | all | Verified |
| `stats_verify.rs`, `stats_verdict.rs`, `stats_chat.rs` | Recompute engine | user spec + table | `commands.rs:1454` | D4 | Partially Wired |
| `journal_registry.rs`, `journal_site_summary.rs`, `journal_verify.rs` | Journal facts + scope | registries | separate command | D10 | Built, Unwired |
| `gap_finder_agent.rs`, `paper_corpus.rs` | Gaps + fit reasoning | user papers | separate command | D1,D10 | Built, Unwired |
| `supplementary.rs` | Data-file parsing | files | reviewer payload | D12 | Verified |
| `pipeline.rs`, `commands.rs`, `aicheck.rs` | Orchestration | — | all | all | Verified |
| `chat_agent.rs` | Report-scoped Q&A | report | UI | — | Verified |
| `sanitize.rs`, `perplexity.rs` | Poisoning defense | untrusted text | rag | — | Verified |
| `models/{mod,candle_perplexity,quantized_qwen2_lowmem}.rs` | SLM runtimes | — | ai_detect | D8 | Verified |

### Intentionally excluded (not review-contributing)

`app_check.rs`, `secrets.rs`, `ratelimit.rs`, `cache.rs`, `db.rs`, `migrations.rs`, `config.rs`, `error.rs`, `projects.rs`, `logging.rs`, `state.rs`, `main.rs`, `lib.rs`, `bind_guard.rs`, `enclave*.rs`, `notes.rs`, `citation_library.rs`, `citation_resolver.rs`, `memory.rs` — infrastructure, security, or separate product surfaces.

### Discoveries that changed the Matrix

1. **`embed.rs` is bag-of-words** → downgraded Plagiarism and Literature-Coverage confidence; created B1 and B0. *Not the expected bottleneck.*
2. **`plagiarism_exact.rs` built and unwired** → B6, best ROI in the roadmap.
3. **`shadow_reviewer` computed and never read** (`commands.rs:677`; zero frontend refs) → fourth discarded signal.
4. **Provenance UI trail verified present** (`ReportViewerPage.tsx:184`) → §7.1 is an extension, not a new build.
5. **`classify_heading` fully traced** → B2 *downgraded* from bottleneck #2 to medium priority.
6. **`PlagiarismSession::report` traced** (the gap this section previously named) → see §9.1. No matrix change; two design questions opened.
7. **Splitting the plagiarism matrix row surfaced a missing dimension.** The merged row had listed a capability whose reviewer question the ontology never stated — §1/§2 had no plagiarism entry and this audit table showed `—` for its dims. Adding **D14 Text Reuse** closes it. The gap predates the split; merging the two arms hid it.

### 9.1 `PlagiarismSession::report` — traced, and a corrected claim

This was the trace §9 named as most valuable. It is now done, and it produced one finding, one self-correction and two open design questions.

**THE GUARD (traced).** `compare_to_corpus` (`plagiarism.rs`) keeps embedding rows whose `source_type == "chunk"` — but `"chunk"` is the *embedding-row kind*, not the RAG `SourceType`. Every RAG document lands with that kind (`rag.rs:229`, `insert_embedding("chunk", chunk_id, …)`). The `SourceType` filter is applied three lines later, inside `corpus_chunk_info`:

```
const EXCLUDED_CORPUS_SOURCE_TYPES: [&str; 2] = ["research_paper", "journal_guideline"];   // plagiarism.rs:249-256
"... WHERE c.id = ?1 AND d.status = 'ingested' AND d.source_type NOT IN (?2, ?3)"          // plagiarism.rs:271-273
```

An excluded document returns `None`, and the caller's `else { continue }` drops it — **no `MatchSpan` is constructed.** This shipped as the M1 fix and is covered by `users_own_working_docs_never_surface_as_corpus_matches` (`plagiarism.rs:394-438`), which ingests both excluded types with text *identical* to the manuscript, asserts zero matches, then ingests a `retraction` and asserts it *does* match — proving the denylist is precise, not a blanket disable.

**THE CORRECTED CLAIM — preserved, per §4.3 doctrine.** While proposing B6, this document's author asserted:

> ~~"`compare_to_corpus` filters `m.source_type != "chunk"` (the embedding-row type) and **does not filter by RAG `SourceType`**"~~ — and concluded that PublishReady was surfacing journal guidelines as plagiarism matches.

**That was wrong.** It was true of the one line read, and false as a claim about the code path, which is the claim actually made. The trace stopped at the first filter and generalised.

Worth recording for two reasons. First, it is a worked example of this document's own standard — *distinguish traced observation from derived conclusion* (§10 self-audit) — catching its own author. Second, **the ontology itself was not wrong**: §9 listed `PlagiarismSession::report` as untraced and rated coverage Medium precisely because of it. The rating did its job; the chat claim exceeded the document's stated coverage. That is the intended failure mode — a claim that outruns the evidence gets caught by the coverage rating rather than shipping.

**DESIGN NOTE — the denylist is load-bearing, not decorative (experimentally demonstrated).** Measured `HashEmbedder` cosine between realistic manuscript prose and a realistic author-guidelines chunk, against the 0.80 threshold:

| Manuscript text | cosine |
|---|---|
| ordinary methods prose | 0.202 |
| a compliance/declarations section | 0.486 |
| a methods paragraph echoing guideline vocabulary | 0.722 |
| near-verbatim restatement of the guideline | 0.754 |

Nothing clears 0.80, but the margin is ~0.05, not an order of magnitude — and the closest cases are declarations/compliance sections, exactly the text that quotes guideline vocabulary back. Without M1 this would plausibly fire on real submissions.

**OPEN DESIGN QUESTION 1 — denylist vs allowlist default.** `plagiarism.rs:253-255` documents the choice: *"DENYLIST, not allowlist: a genuine corpus feed added later under any other `source_type` is scanned automatically."* Deliberate and defensible. But it means a new `SourceType` is opted **into** plagiarism scanning by default, and per the editorial audit below only one of the four current types arguably belongs there — so the default points the wrong way. Not urgent; needs a decision, not a patch.

**OPEN DESIGN QUESTION 2 — should `retraction` be scanned?** Currently it is (and the M1 test relies on it to prove the denylist is precise).
*For scanning:* a retraction notice is third-party published text the author did not write; verbatim overlap with one is genuinely odd and worth surfacing.
*Against:* a retraction notice is *metadata about* a paper, not the paper. Overlap most likely means the manuscript legitimately quotes or discusses the notice — which is scholarship, not reuse. `refverify`'s retraction lane (`refverify.rs:562-567`) is the component that should consume retraction data, for D7.
Unresolved. Recorded, not decided.

### 9.2 The RAG write surface, and the external-plagiarism capability state

Established by tracing **table writes**, not `ingest_document` callers — the method change that turned an inference into a fact.

**THE WRITE SURFACE IS THREE STATEMENTS, REPOSITORY-WIDE (traced).**

| Statement | Location | Reachable from |
|---|---|---|
| `INSERT INTO documents` | `rag.rs:188` | `rag::ingest_document` only |
| `INSERT INTO chunks` | `rag.rs:222` | `rag::ingest_document` only |
| `INSERT INTO embeddings` | `vector.rs:46` | `Database::insert_embedding` |

There is no other `INSERT INTO documents` in the tree. A RAG document carrying a `SourceType` can therefore only be created through `rag::ingest_document`. `insert_embedding`'s only other production caller is `plagiarism.rs:146`, which writes kind `"manuscript_chunk"` into the per-session **isolated** store, never the shared database.

**EVERY PRODUCTION `ingest_document` CALLER (traced, exhaustive).**

| Caller | SourceType | Eligible for cosine plagiarism? |
|---|---|---|
| `guidelines.rs:184` | `JournalGuideline` | **No** — excluded by `EXCLUDED_CORPUS_SOURCE_TYPES` (`plagiarism.rs:256`) |
| `paper_corpus.rs:417` | `ResearchPaper` | **No** — excluded, same list |

(`pipeline.rs:729` and `plagiarism.rs:402` are inside `#[cfg(test)]` — `pipeline.rs:363`, `plagiarism.rs:289`.)

Also checked and clear: no scheduled jobs or background tasks (`lib.rs:62` is Tauri `.setup` only, no `spawn`/`interval`/`tokio::time` outside `spawn_blocking`); no seed or import utility that writes (`read_import_file` returns a `String`); of 58 registered Tauri commands only `ingest_guidelines` and `build_gapfinder_corpus` ingest at all; migrations contain no RAG-table `INSERT` (the three are `schema_migrations` bookkeeping at `:410` and two inside migration tests); and gaply-proxy cannot reach this database — its only DB dependency is `asyncpg` against Postgres.

**CONSEQUENCE.** The two SourceTypes eligible for cosine comparison (`retraction`, `reference_style`) have **no production creation path of any kind**. The two production does create are both excluded. In the traced shipping architecture at this commit, `compare_to_corpus` can only return empty because no production code creates an eligible `SourceType`.

**CAPABILITY CLASSIFICATION — External cosine plagiarism comparison: `EFFECTIVELY UNAVAILABLE`.**

In the traced architecture at this commit the lane executes on every PublishReady run and can only return empty. Rejected alternatives, with reasons:

* **Dormant** — no. Dormant code does not run. This runs every time, consumes work, and produces a user-visible result.
* **Configuration-dependent** — no. No setting enables it. There is nothing to configure, because no ingester exists to point at a corpus.
* **Partially operational** — no. The *self*-match arm is fully operational, but it answers a different editorial question ("is text repeated **within** this manuscript"). Treating the two arms as one capability is precisely what produces the misleading output below.

**RESIDUAL UNCERTAINTIES (stated, not resolved).**

1. Scope is this repository at the time of tracing. A retraction or reference-style ingester added later changes the classification.
2. `db_migrate` / `db_init` are user-invocable (`commands.rs:74`, `:80`). Migrations were verified to contain no RAG-table `INSERT`, and any that seeded documents would have to use `rag.rs:188` — but individual migration bodies were not read line by line.
3. A pre-existing user database could hold `retraction` rows written by an older build. Historical migrations were not audited for a removed ingester. "Empty" is a statement about what **this build can produce**, not a guarantee about every database on disk.

**THIS CLASSIFICATION IS LOAD-BEARING AND FRAGILE.** Adding any ingester that creates a `retraction` or `reference_style` document flips it — and nothing in the code would flag that. `rag::ingest_document` being the sole write path is the fact this rests on; if that stops being true, this section is stale.

**OPEN ISSUE — the report is silent about text reuse when nothing matches.** *(Corrected — see the superseded claim below.)*

**TRACED BEHAVIOUR.** With zero matches the PublishReady report contains **no plagiarism output at all**:

* the per-match loop iterates `corpus_matches`/`self_matches` and emits nothing when both are empty — *a non-event is not a finding* (`report.rs:288`), pinned by `empty_plagiarism_yields_zero_findings` (`report/tests.rs`);
* the soft-opinion loop **explicitly skips `AgentKind::Plagiarism`** (`report.rs`, guard `if op.hard_constraint || op.agent == AgentKind::Verification || op.agent == AgentKind::Plagiarism { continue }`), so the `Opinion` built by `adapters::from_plagiarism` (`swarm.rs:379-401`) feeds the **debate vote only** and never becomes a `Finding`;
* `PlagiarismReport.note` carries `ISOLATION_NOTE` (producer: `plagiarism.rs:42`, set in `PlagiarismSession::report`), but `compile_report` **never reads `.note`** (consumer: absent — no `.note` reference in `report.rs`). The disclaimer is not propagated into the PublishReady report in the traced execution path.

**CONSEQUENCE (traced).** No plagiarism findings are rendered, and no capability-state statement is rendered. The current report therefore does not distinguish between:

* no reusable text found,
* no eligible external comparison corpus,
* comparison unavailable.

No fix is described here.

**SUPERSEDED CLAIM, preserved per §4.3 doctrine.** An earlier revision of this section stated:

> ~~"With an empty corpus the plagiarism lane emits one finding via `adapters::from_plagiarism` through `compile_report`'s soft-opinion loop — title `"Plagiarism: pass"`, detail `"0 corpus / N self match(es) at threshold 0.80; <ISOLATION_NOTE>"`, rendered at `ReportViewerPage.tsx:361,366`"~~ — and characterised the defect as a misleading *"pass"*.

**That was wrong.** `from_plagiarism` genuinely constructs that explanation, but nothing consumes it: the soft-opinion loop skips that engine. The trace established that a producer existed and inferred that its output reached the user, without tracing the consumer. The rendered-output quotation was reconstructed from the producer, not observed.

The corrected defect is **silence**, not a misleading status — a different problem, and one with no existing surface to attach wording to.

**Verification trigger.** Any future production ingester for `retraction` or `reference_style` invalidates this classification and requires this section and the Capability Matrix (§3) to be re-reviewed.

### 9.3 Known Precision Limits

Traced properties of the current algorithms whose practical impact is unquantified. **Not open issues** — these are not unresolved bugs but characteristics of what the code does today. They belong here so a future contributor finds them before re-deriving them, and so they are not mistaken for defects awaiting a fix.

**The corpus KNN is unfiltered; filtering happens after a global top-k.**

`knn_embeddings` (`vector.rs:71-93`) is:

```sql
SELECT rowid, distance, source_type, source_id
  FROM embeddings
 WHERE embedding MATCH ?1 AND k = ?2
 ORDER BY distance
```

There is **no `source_type` predicate**. `compare_to_corpus` requests `k = 5` and then filters in Rust — first on the embedding-row kind (`source_type != "chunk"` → skip), then on the RAG `SourceType` denylist via `corpus_chunk_info` (`plagiarism.rs:271-273`).

**Consequence.** Eligible chunks can exist and still never surface: if the shared database holds enough *excluded* chunks, the global top-5 can be occupied entirely by rows that are then dropped. Production guidelines are re-ingested on every PublishReady run (traced: `guidelines.rs:184`, from `PublishReadyPage.tsx:147-155`). If previously ingested guideline chunks are retained rather than replaced, the excluded population can grow over time, increasing the likelihood that an unfiltered top-k search returns only excluded rows. Whether re-ingestion replaces or appends has not been traced, and whether this accumulation occurs in deployed databases has not been measured.

**Classification.** Mechanism: **Traced** (`vector.rs:71-93`, `plagiarism.rs:271-273`). Impact frequency: **unmeasured** — no data exists on how often top-5 is saturated in a real database.

**Editorial consequence.** This is precisely why a signal at the eligible-input layer can support *"an external comparison was possible"* and can **never** support *"an external comparison occurred"*. The latter is a different claim requiring a different signal — whether the KNN actually returned an eligible candidate on this run — and would need its own wording. Conflating the two would be the §4.4 failure in the opposite direction from B0: understating rather than overstating, but still a claim not matched to its evidence.

### 9.4 Verification methodology — tracing producers without tracing consumers

A recurring error in this document's own construction, recorded so it is not repeated.

**The pattern.** A producer is traced — a value is constructed, a field is set, a function returns the right thing — and its arrival at the user is then *inferred*. The consumer is never traced. Because the producer is real and correct, the inference feels grounded; it is not.

**Three documented instances, all in this document:**

1. **§9.1 — `compare_to_corpus`.** The first filter (`source_type != "chunk"`) was traced and the conclusion *"does not filter by RAG `SourceType`"* drawn from it. The actual filter lives in the callee `corpus_chunk_info` (`plagiarism.rs:271-273`), three lines further on. Traced the guard, not the guard's continuation.
2. **§9.2 — `from_plagiarism`.** The `Opinion` and its explanation string were traced; the soft-opinion loop that **skips** `AgentKind::Plagiarism` was not. Producer real, consumer absent.
3. **§9.2 — `ISOLATION_NOTE`.** `PlagiarismReport.note` is set (`plagiarism.rs:42`); `compile_report` never reads it. Producer real, consumer absent.

**Contrast — the same check done correctly.** `shadow_reviewer` was classified *Computed, Discarded* only after grepping the frontend for a consumer and finding none (`commands.rs:677`; zero references in `src/`). That is the standard the three instances above failed to meet.

**STANDING VERIFICATION RULE.** Whenever a capability is claimed to reach the user, trace **both**:

* **producer → consumer**, and
* **consumer → rendered UI**.

**A producer alone is insufficient evidence that information reaches the user.** A claim about what a user sees requires a trace terminating at a render site, or it is inference and must be labelled as such.

### Coverage confidence: **Medium**

The PublishReady pipeline is traced end-to-end at `file:line` and every module is classified. Not High because two subsystem internals remain untraced: `rag::search` and `RefVerifier::verify`'s connector orchestration — each underwrites a dimension-quality claim. (`PlagiarismSession::report`, previously the third, is now traced — §9.1.)

---

## 10. Confidence & Limitations

**HIGH confidence (traced, `file:line`-backed):** Evidence Model structure and its three gaps; the full PublishReady lane sequence, verified by execution path; which capabilities are wired vs. built-but-unwired; the missing-signal families confirmed by zero-result greps; the decision-rule precedents; the provenance UI trail.

**UNVERIFIED, and why:** extraction accuracy on real manuscripts (no in-repo benchmark); `HashEmbedder` false-positive rate (unmeasured); the three subsystem internals above (not read); whether the deployed proxy revision matches this tree (no Render access).

**Engineering (bounded, estimable):** B1, B2, B4, B6, B7; Tier 1–2. *(B0 resolved in `83f192c`.)*
**Research (open, unbounded):** B3; Tier 3. D2 recommended for permanent exclusion.

**To raise coverage to High:** trace `rag::search`, then `RefVerifier::verify`'s connector orchestration. (`PlagiarismSession::report` is done — §9.1.)

**These documents are intended to evolve alongside the implementation.** New capabilities should update the ontology before — or at least in the same change as — the implementation, so the evidence model and roadmap remain synchronized.

### Self-audit

- *Did every capability claim originate from traced code?* **Yes** — each cites `file:line` or is Missing on a zero-result grep.
- *Did every dependency originate from traced code or get marked inference?* **Yes** — Gap E3's remedy and §7.1 item 3 are the only **[INFERENCE]** marks.
- *Did any roadmap recommendation depend on an unverified assumption?* **One.** B6 assumes wiring `plagiarism_exact` improves review quality; its existence is verified, its practical superiority is unmeasured. Flagged, not asserted.
- *Any section more confident than its evidence?* **§6.1 kill criteria** are proposed thresholds, not derived ones — judgement, labelled as such.
