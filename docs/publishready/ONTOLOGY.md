# PublishReady Editorial Review Ontology v1.0

**Status:** Draft for review
**Basis:** code traced at commit `36ea74d`; verification pass at `36ea74d`; B0 resolved in `83f192c`
**Governing principle:** *PublishReady must never be more confident than its evidence — and neither may this document.*

Every claim is backed by `file:line` or explicitly marked **[INFERENCE]**. A capability that could not be verified in code is **Not Verified**, never assumed.

This document **extends** the shipped Evidence Model (`gaply-core/src/evidence.rs`, `report.rs`). It does not define a parallel system. Where the existing model already answers a question, it is cited rather than restated.

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

---

## 2. Evidence Ontology

`ConfidenceKind` variants are reused verbatim from `evidence.rs:45-57`; `RoutingHint` from `evidence.rs:62-74`. Both are assigned **per-agent** by total functions (`evidence.rs:118-127`, `:131-140`).

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

### 2.1 Where the existing Evidence Model is insufficient

Stated as gaps, not designed around.

**Gap E1 — `ConfidenceKind` is per-agent with no per-finding override.** `confidence_kind()` (`evidence.rs:118-127`) is a total function of `AgentKind`. There is no way to express *"deterministic arithmetic produced by the Verification lane."* Hit concretely in commit `364e106`: the citation-recency finding is deterministic counting, but `Verification` would label it `RealNative`, so it was attributed to `Extraction`/`NoSignal` — the honest choice available, not the accurate one.

**Gap E2 — `CertaintyTier` (`report.rs:56-80`) has no tier for "deterministic but not a hard constraint."** `MathematicallyCertain` is the Maths agent's hard-constraint verdict and sorts first (`report.rs:73-79`). A deterministic table count is certain but must not outrank a statistical rule failure.

**Gap E3 — `Finding` carries no dimension field.** `Finding` (`report.rs:112-127`) carries `agent`, not dimension. Dimension is inferable only from the `signal:` provenance convention introduced in `364e106`. **[INFERENCE]** a dimension tag is the minimal extension; not designed here.

---

## 3. Capability Matrix

Tier: **1** deterministic · **2** manuscript-internal · **3** literature-grounded · **4** editorial synthesis.

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
| Plagiarism — lexical cosine | Verified Present | `plagiarism.rs`; `pipeline.rs:236-237` | **See §4.3 + B1** | Low | 2 |
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
| `"internal duplication (self-plagiarism)"` | `report.rs:42`; `adapters.ts:29` | `MatchSource::SelfManuscript` knows *where*, never *whether reuse was illegitimate* — contradicted `ISOLATION_NOTE` three lines below | `"internal duplication (same manuscript)"` |
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
4. **An asserted determination** — `"internal duplication (self-plagiarism)"` (`report.rs:42`, `adapters.ts:29`) declared reuse illegitimate, contradicting `ISOLATION_NOTE` three lines below in the same file. `MatchSource::SelfManuscript` establishes *where* a match is, never *whether* the reuse was improper. Now `"(same manuscript)"`.

The fourth was found outside the commissioned scope, by noticing the same pattern in adjacent code while correcting the third — which is the usual way this class of defect surfaces.

The self-suppressing counterpart is already in the code: `grounded_text` (`reviewer_agent.rs:486-509`) returns empty and flags `potential_hallucination` rather than emitting an ungrounded claim. New editorial fields should adopt that pattern rather than `parse_score`'s.

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
| `plagiarism.rs` | Lexical-cosine lane | embed, chunk | report | — | Verified (B0 resolved; B1 open) |
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

### Coverage confidence: **Medium**

The PublishReady pipeline is traced end-to-end at `file:line` and every module is classified. Not High because three subsystem internals remain untraced: `PlagiarismSession::{ingest_manuscript, report}`, `rag::search`, and `RefVerifier::verify`'s connector orchestration — each underwrites a dimension-quality claim.

---

## 10. Confidence & Limitations

**HIGH confidence (traced, `file:line`-backed):** Evidence Model structure and its three gaps; the full PublishReady lane sequence, verified by execution path; which capabilities are wired vs. built-but-unwired; the missing-signal families confirmed by zero-result greps; the decision-rule precedents; the provenance UI trail.

**UNVERIFIED, and why:** extraction accuracy on real manuscripts (no in-repo benchmark); `HashEmbedder` false-positive rate (unmeasured); the three subsystem internals above (not read); whether the deployed proxy revision matches this tree (no Render access).

**Engineering (bounded, estimable):** B1, B2, B4, B6, B7; Tier 1–2. *(B0 resolved in `83f192c`.)*
**Research (open, unbounded):** B3; Tier 3. D2 recommended for permanent exclusion.

**To raise coverage to High:** trace `PlagiarismSession::report` — it determines whether §4.3 is the whole problem or whether chunking/KNN adds further distortion. Then `rag::search` and `RefVerifier::verify`.

**These documents are intended to evolve alongside the implementation.** New capabilities should update the ontology before — or at least in the same change as — the implementation, so the evidence model and roadmap remain synchronized.

### Self-audit

- *Did every capability claim originate from traced code?* **Yes** — each cites `file:line` or is Missing on a zero-result grep.
- *Did every dependency originate from traced code or get marked inference?* **Yes** — Gap E3's remedy and §7.1 item 3 are the only **[INFERENCE]** marks.
- *Did any roadmap recommendation depend on an unverified assumption?* **One.** B6 assumes wiring `plagiarism_exact` improves review quality; its existence is verified, its practical superiority is unmeasured. Flagged, not asserted.
- *Any section more confident than its evidence?* **§6.1 kill criteria** are proposed thresholds, not derived ones — judgement, labelled as such.
