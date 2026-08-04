# PublishReady — Traced Architecture

**Basis:** repository at `6da523b`. Complements [ONTOLOGY.md](./ONTOLOGY.md) (evidence and capabilities); this document covers **control flow, data flow, and structure** — what executes, in what order, and where data goes.

**Standing rules applied:** every claim carries `file:line` or is marked **[INFERENCE]**; any value claimed to reach a user is traced *forward to its render site* (ONTOLOGY §9.4); each section separates FACTS / ANALYSIS / OPEN QUESTIONS. Where verification was not possible, the text says **Not Verified** rather than assuming.

**Scope note.** Sections 1–10 are traced fact. Section 11 is **design intent** — decisions recorded before implementation. Nothing in §11 describes shipping behaviour, and the coverage rating in §10 does not extend to it.

**Terminology.** The six analysis units are **Review Engines**, not "agents". Most are, and will remain, deterministic Rust — graph algorithms, rule engines, statistical validators; only a subset use a language model. "Agent" implies autonomous LLM conversation, which this architecture deliberately rejects.

The rename applies to **prose only**. Code identifiers are quoted as they exist and are never renamed in this document — `AgentKind`, `SwarmAgent`, `Finding.agent`, the `agent:` provenance prefix, and the `*_agent.rs` filenames appear verbatim, because the `file:line` correspondence is this document's entire value. Renaming them in prose while the code says otherwise would break exactly the producer-to-consumer traceability the standing rules exist to protect. If the rename ever reaches the code, it is a separate, mechanical change made against a green suite.

---

## 1. Execution Spine

### FACTS

`run_publishready` (`commands.rs:495`) is `async` but wraps its entire body in **one** `tokio::task::spawn_blocking`. Everything inside is **strictly sequential** — no `join`, no nested spawn, no concurrency (verified: the only `await` is the outer one on the blocking handle).

| # | Call | Returns | Consumed by | On failure |
|---|---|---|---|---|
| 1 | `run_pipeline_measured` (`:516`) | `()`, events via closure | `report_id` from the `Finished` event | **`?` — aborts the run** |
| 2 | `db.cache_get("report:{id}")` (`:~538`) | report JSON | steps 3–6 | `?` — aborts |
| 3 | `run_targeted_escalation` (`:559`) | `EscalationSummary` | `tracing::info!` only | Degrades — never fails |
| 4 | `build_review_payload` (`:~584`) | `(Value, SentIds)` | steps 5–6 + returned | Pure, cannot fail |
| 5 | `run_shadow_synthesis` (`:~633`), timed | `ShadowOutcome` | `shadow_reviewer` field | Degrades to `None` |
| 6 | `verify_with_envelope` (`:629`), timed | `(Value, ProxyEnvelope)` | `reviewer` | Degrades to `unavailable_offline` |
| 7 | `PublishReadyOutcome` (`:678`) | — | frontend | — |

**Exclusive resources.** The candle model is scoped *inside* the AI lane so it drops before verification (`pipeline.rs:200-210`); `unload_slm2` (`pipeline.rs:~290`) explicitly evicts the Ollama model after the verify lane — the one-at-a-time discipline for 8 GB machines.

**Wall-clock shape [INFERENCE, derived from the traced structure].** Dominated by three serial costs: AI-lane model load + inference; one refverify HTTP round per reference (`pipeline.rs:272`, a sequential loop); and **two sequential cloud round-trips** (shadow narrative, then wholesale reviewer). Steps 5 and 6 are independent but run one after the other.

### ANALYSIS

The spine is a single blocking thread with two cloud calls in series. Only failures in steps 1–2 abort; 3–6 all degrade. That asymmetry is deliberate and correct — local analysis is the product, cloud reasoning is additive.

### OPEN QUESTIONS

- Steps 5 and 6 have no data dependency. Whether serialising them is intentional (metering? rate limiting?) is **Not Verified**.

---

## 2. The Six Lanes

### FACTS

`run_pipeline_inner` (`pipeline.rs:161`) runs each lane through `lane()` (`:99`), which emits `StageStarted` / `StageCompleted` / `Failed`. **Any lane returning `Err` aborts the whole pipeline** (`?` at each call site).

| Lane | Input | Output | → `compile_report`? | → UI? |
|---|---|---|---|---|
| 1 Extraction | parsed text | `ExtractionResult` | **Yes** (`Some(&extraction)`) | via findings |
| 2 Validation | extraction | `StatsValidityReport` | Yes | Critical findings |
| 3 AI check | extraction | `AiDetectionReport` | Only via swarm `Opinion` | one soft finding |
| 4 Plagiarism | text, embedder, db | `PlagiarismReport` | Yes (`Some(&plag)`) | per-match findings only |
| 5 RAG | db, embedder | `Vec<RagHit>` | Only via `Opinion` | one soft finding |
| 6 Verification | references | `VerificationReport` | Yes | per-verdict findings |

**Computed-then-discarded — the list has changed since first drawn:**

| Signal | Status |
|---|---|
| `DocumentFeatures` | **Resolved** — wired in `364e106` |
| `TableRef` | **Resolved** — wired in `364e106` |
| `matched_year` / `citation_count` (`refverify.rs:661,712,846`) | **Still discarded** — recency uses local `Reference.year` *(**corrected below — this row was wrong**)* |
| `shadow_reviewer` (`commands.rs:677`) | **Still discarded** — zero frontend references *(**corrected below — understated**)* |
| `PlagiarismReport.note` (ISOLATION_NOTE) | **Newly identified** — `compile_report` never reads `.note` |
| `from_plagiarism` explanation (`swarm.rs:379-401`) | **Newly identified** — soft loop skips the Plagiarism engine; feeds the debate vote only |

Net: two resolved, two persisting, two newly found — four current.

#### Corrections to this table

Audited row by row. `DocumentFeatures`, `TableRef`, `PlagiarismReport.note` and the `from_plagiarism` explanation were re-verified and are accurate as stated (`.note` is read nowhere outside tests; the soft loop skips `AgentKind::Plagiarism` at `report.rs:382`). Two rows were not.

**`matched_year` / `citation_count` — "still discarded" is WRONG.** Both are inserted into the evidence bundle sent to the proxy (`verify_agent.rs:176`, `:206`), and the verification instruction (`:234`) explicitly tells the model to compare `matched_year` against the citation. The accurate statement is:

> **computed, transmitted to the cloud, and discarded locally.**

**Why the wording matters architecturally, not just factually.** "Unused computation" and "computation consumed remotely but unavailable to later local stages" are different problems with different fixes. The first is waste — the fix is to consume it or delete it. The second is a *locality* problem: the data has a consumer, but the consumer is across the privacy boundary, so no local deterministic finding can be built on it and nothing offline can use it. The fix is to retain it on the local side as well, which is a signature change (`pipeline.rs:280` builds `items: Vec<(Reference, ReferenceVerification)>` as a lane-local variable and drops it; `VerificationReport` carries only `verdicts` and `warnings`, `verify_agent.rs:124-128`). Describing it as waste would have pointed at the wrong repair.

**`shadow_reviewer` — "still discarded" is UNDERSTATED**, and in a way that matters for the product decision. `ShadowOutcome` has four fields (`reviewer_synthesis.rs:111-119`) in three different states:

| Field | State |
|---|---|
| `letter` | reaches the frontend in `PublishReadyOutcome`, **read by nothing** |
| `aggregation.breakdown`, `findings_sent`, `narrative_available` | **consumed locally** by `reviewer_harness::build_comparison_report` (`commands.rs:659-670`) |

So a shadow-vs-wholesale **divergence harness already exists**, log-only, explicitly *"Drives NO production behavior."* Recording the whole signal as "discarded" hid a built comparison path — and hid that the agreement measurement is one live run away rather than unbuilt.

### ANALYSIS

Lanes 3 and 5 are structurally weaker than 1/2/4/6: their reports never reach `compile_report`, only a one-line `Opinion`. AI-detection's per-section detail and RAG's per-hit provenance exist in memory and are collapsed to a sentence.

### OPEN QUESTIONS

- Whether `AiDetectionReport.sections` was ever intended to surface per-section — **Not Verified**.

---

## 3. Data Flow: Manuscript → Screen

### FACTS

One plagiarism finding, end to end:

```
file bytes
 → docparse::parse_path                              pipeline.rs:157
 → extract_from_text → ExtractionResult              pipeline.rs:163
 → PlagiarismSession::{new, ingest_manuscript, report}   pipeline.rs:237-240
     ↳ chunk_default → embed(HashEmbedder) → isolated store   plagiarism.rs:146
 → MatchSpan { manuscript_chunk_seq, manuscript_excerpt, similarity, source }
 → compile_report per-match loop                     report.rs:294-330
     ↳ title  = match_type_label + "% word overlap"           report.rs:65, :319
     ↳ detail = "“{manuscript_excerpt}” matches {source}"     ← CONTAINS MANUSCRIPT TEXT
     ↳ provenance = [similarity:, match_type:, source:]
     ↳ paired() → EvidenceRecord::at_source          report.rs:187 → evidence.rs:181
 → PublishReadyReport { findings, evidence, checklist, debate, disclaimer }
 → serde_json → db.cache_put("report:{id}", 30d TTL) pipeline.rs:357
 → PublishReadyOutcome                               commands.rs:678
 → adaptOutcome                                      publishReadyBridge.ts:72
 → ReportViewerPage.tsx    :361 title · :366 detail · :373 provenance[0]
```

**Where data is dropped or reshaped:**

| Boundary | What happens | Evidence |
|---|---|---|
| Proxy payload | `detail` **deliberately never read** — it carries `manuscript_excerpt` | `reviewer_agent.rs:345` |
| Proxy payload | provenance filtered to nine structured prefixes | `evidence.rs:25-40` |
| Proxy payload | capped at `MAX_FINDINGS = 12`, `MAX_CHECKLIST = 20` | `reviewer_agent.rs:53-55` |
| Frontend adapter | `proxy_payload` reshaped (flattens `summary`) | `publishReadyBridge.ts:106` |
| Frontend adapter | `shadow_reviewer` **not read at all** | zero references in `src/` |
| UI | renders title, `certainty_label`, `detail`, `provenance[0] (+N)`; **not** the `f{N}` id, **not** `ConfidenceKind` | `ReportViewerPage.tsx:360-378` |
| UI inspector | renders the full provenance list | `:184-187` |

### ANALYSIS

`detail` is the split point of the whole privacy design: rendered locally (correct — the user's own manuscript, on their machine) and dropped at the proxy. The `evidence[]` vector reaches the frontend in JSON but nothing reads it, so `ConfidenceKind` — the Evidence Model's central honesty device — never renders.

### OPEN QUESTIONS

- Whether `report["evidence"]` is read by any other surface (PDF export? Copilot?) — traced for `ReportViewerPage` and `exportPdf` only; other screens **Not Verified**.

---

## 4. The Proxy Boundary

### FACTS

**Within PublishReady — four crossings, confirmed exhaustive:**

| # | Builder | Call site | Gates |
|---|---|---|---|
| 1 | `verify_agent::verify_citations` payload | `verify_agent.rs:300` (via `pipeline.rs:~283`) | App Check + entitlement + harness gate |
| 2 | `build_escalation_payload` | `escalation.rs:151` | + `gate_escalation_response` |
| 3 | `build_reviewer_request` (Box 4) | `reviewer_synthesis.rs:147` | + `gate_reviewer_narrative` |
| 4 | `build_review_payload` (wholesale) | `commands.rs:629` | + `gate_reviewer_response` |

**Others exist, all outside PublishReady:** `chat_agent.rs:516`, `stats_chat.rs:349`, `gap_finder_agent.rs:340,717,1020,1219` (gaps / QA / draft / fit), `journal_site_summary.rs:238`, and `verify_agent.rs:517` (a second citation call — the reconsideration round).

**Server side** (`gaply-proxy`): `/verify` has **no task dispatch** — `payload["task"]` is never read; validation, then a single `provider.complete(payload)` (`main.py:240-268`). The provider is chosen server-side (`main.py:146-193`); the response envelope `{model, stop_reason, text}` is parsed from the live HTTP body (`openai_client.py:75-81`).

**Deliberately excluded:** finding `detail`, unstructured provenance, raw manuscript text, abstracts (`verify_agent.rs:14-16`), and `origin` paths in gap-finder digests (`gap_finder_agent.rs:44-46`).

### ANALYSIS

All four PublishReady crossings share one shape: pure builder → injected `ProxyClient` → harness gate enforcing refs ⊆ sent. That uniformity is the strongest structural property in the codebase.

### OPEN QUESTIONS

- `verify_agent.rs:517` (reconsideration) — whether it executes in the PublishReady path is **Not Verified**. It is reached via `RevisingVerificationAgent`; whether the pipeline's debate config enables revision was not traced.

---

## 5. State and Persistence

### FACTS

**Tables** (`migrations.rs`): `projects · schema_migrations · cache · chunks · citation_library · documents · episodic_memory · evidence · extractions · findings · journal_guidelines · manuscripts · notes · plagiarism_library · reference_styles · retractions`, plus the `embeddings` vec0 virtual table (`vector.rs`).

| Store | Written by | Lifetime |
|---|---|---|
| `manuscripts`, `extractions`, `findings` | `store_extraction` (`pipeline.rs:169`) | Persistent |
| `documents` / `chunks` / `embeddings` | `rag::ingest_document` **only** | Persistent, accumulates |
| `cache` (`report:{id}`) | `pipeline.rs:357`, **30-day TTL** | Persistent until TTL |
| `evidence` | `evidence_persist` (escalation Phase 1) | Persistent, per `run_id` |
| Plagiarism session store | `PlagiarismSession::new()`, in-memory | **Per-run, discarded** |

**Isolation.** User text never enters the shared corpus (`plagiarism.rs:11-19`), pinned by `user_upload_never_writes_to_shared_corpus`.

**Cross-run leakage — one genuine channel.** The RAG corpus accumulates. `ingest_guidelines` runs before **every** PublishReady run (`PublishReadyPage.tsx:147-155`), so `documents` / `chunks` / `embeddings` grow across runs. Whether re-ingestion replaces or appends is **Not Verified** (see ONTOLOGY §9.3).

### ANALYSIS

`run_id == report_id == manuscript_id` is a single identity threading the report cache, the Evidence Store and escalation — the most useful structural invariant available to future work.

### OPEN QUESTIONS

- Writers for `episodic_memory`, `retractions`, `journal_guidelines`, `reference_styles` are **Not Verified** (no production writer was found for the last two).

---

## 6. Failure and Degradation

### FACTS

| Failure | Behaviour | User sees | Distinguishes "checked, found nothing" from "could not check"? |
|---|---|---|---|
| Proxy unreachable (reviewer) | `unavailable_offline` | *"deep reasoning requires cloud analysis — unavailable offline"*, `available: false` | **Yes** |
| Proxy unreachable (verification) | Falls to Ollama → mock | Every citation `UNKNOWN` | **Partly** — `Unknown` ≠ `Refuted`, but not why |
| Proxy 500 (escalation) | `None` → `unavailable` tally | Nothing rendered | **No** |
| Proxy 401/500 (Box 4 shadow) | `narrative_available = false` | Nothing — not read by the UI | **No** |
| SLM-1 won't load / low RAM | `HeuristicModel` | `model.name()` in the lane summary | **Yes** |
| Ollama absent | Mock, empty verdicts | `UNKNOWN` verdicts | Partly |
| refverify connector timeout | `warn!`, reference skipped (`pipeline.rs:274`) | Fewer verdicts, no notice | **No** |
| Guidelines page unreachable | `GuidelineIngest::Unavailable`, honest note | UI note (`PublishReadyPage.tsx`) | **Yes** |
| Empty external corpus | Zero findings | **Nothing at all** | **No** (ONTOLOGY §9.2) |
| Zero plagiarism matches | Zero findings | **Nothing at all** | **No** |
| DB locked | `GaplyError` → lane `Failed` → run aborts | Error toast | **Yes** (fails loudly) |

### ANALYSIS

A clean pattern emerges: **degradations that swap a component announce themselves; degradations that yield an empty collection are silent.** The first kind flows through a named field (`available`, `model.name()`, `IngestStatus`); the second relies on findings, and "no findings" is indistinguishable from "nothing to find". Five of eleven rows fail the distinguishability test, and all five are empty-collection cases.

### OPEN QUESTIONS

- Whether `AnalysisEvent::Failed` reaches the PublishReady UI is **Not Verified**. Events are collected into a `RefCell` at `commands.rs:515` and only `Finished` is read — which would mean per-lane failures are invisible in PublishReady even though `run_full_analysis` streams them.

---

## 7. The Finding / Non-Finding Boundary

### FACTS

`report.rs:288` — *"Empty matches → zero findings (a non-event is not a finding)"* — is a comment on **the plagiarism per-match fan-out only**. It is not enforced anywhere globally.

**Findings describing the ANALYSIS rather than the MANUSCRIPT already exist — three classes, all shipping:**

1. **Extraction opinion** — `"Extraction: pass"` / *"extracted N statistical claim(s) and M reference(s) without parse anomalies"* (`swarm.rs:334`). Describes what the parser did.
2. **RAG opinion** — `"Rag: pass"` / *"N provenance-tagged context hit(s) retrieved"* (`swarm.rs:408`). Describes retrieval — i.e. the analysis. **Reports zero hits as a finding.**
3. **Gate-rejected opinions** — `"{Agent} output rejected by its internal gate"`, severity `Minor`, provenance `swarm:rejected-before-debate` (`report.rs`). Describes a Review Engine failing, not the manuscript.

**Surfaces currently carrying capability state to the user:** the three above, plus `ReviewerEvaluation.available` (rendered as the unavailable-offline state) and `warnings[]`. **Nothing** carries plagiarism capability state.

### ANALYSIS

The premise that a capability-state statement would be novel is **false**. Class 2 is the direct precedent: RAG emits a finding reporting *zero* retrieved hits — a non-event about the analysis, rendered today. The rule at `report.rs:288` governs per-item fan-out, not "may a finding describe the analysis".

The asymmetry is structural rather than principled: engines whose opinion survives the soft loop (Extraction, AI-detection, RAG) get an analysis-level finding; engines skipped by it (Verification, Plagiarism) do not, because the loop assumes their per-item findings cover them — which holds only when items exist.

### OPEN QUESTIONS

- Why Verification and Plagiarism were skipped (`report.rs` comment: *"covered in detail above"*) — whether the empty case was considered is **Not Verified**.

---

## 8. Architectural Seams

### FACTS

| Seam | Used? |
|---|---|
| `ProxyClient` (`verify_agent.rs:47`) | **Live** — three impls: reqwest, Ollama, Mock |
| `Embedder` (`embed.rs:12`) | **Vestigial** — one impl (`HashEmbedder`); the seam exists, unused |
| `PerplexityModel` | **Live** — candle + heuristic, gated |
| `HttpFetcher` | **Live** — reqwest + mock |
| `SwarmAgent` / `PrecomputedAgent` | **Live** — the six-engine attach point |
| `RoutingPolicy` (`orchestrator.rs`) | **Live** — `DefaultRoutingPolicy` |
| `LlmProvider` (proxy, `claude_client.py:15`) | **Live** — Claude + OpenAI |
| `create_app(providers=…)` | **Test-only** |
| `EnclaveClaudeClient` | **Dormant** — requires Nitro |

**Where a new lane attaches:** add it to `run_pipeline_inner` via `lane()`, produce a report type, add an `adapters::from_*` → `Opinion`, and add an `AgentKind` variant — which forces classification in `confidence_kind` / `routing_hint` / `limitations` (all TOTAL, no wildcard, `evidence.rs:118-158`) **or it will not compile**.

### ANALYSIS

The `AgentKind` totality is the strongest guard rail in the codebase: a seventh engine cannot be added without explicitly deciding its evidence tier. `Embedder` is the clearest under-used seam — fully abstracted, one implementation, and the B1 bottleneck sits behind it.

---

## 9. What the Architecture Makes Hard

### FACTS + ANALYSIS

| Roadmap item | Accommodates cleanly? | Structural obstacle |
|---|---|---|
| Wire `plagiarism_exact` (B6) | **Yes** | None — add a lane + `compile_report` param; precedent set by `364e106` |
| Aggregate discarded signals (B7) | **Yes** | None — the data is in scope |
| Real embedder (B1) | **Yes** | None — `Embedder` is the seam, `EMBEDDING_DIM` already 384 |
| Figure detection | **Yes** | None — mirrors `TableRef` |
| Ethics / data-availability | **Yes** | None — text search over sections |
| Capability-state reporting | **Partly** | No blocker in the Evidence Model (§7 precedent); the obstacle is that Plagiarism and Verification are *skipped* by the soft loop, so there is no emission point |
| Per-finding `ConfidenceKind` | **No** | `confidence_kind()` is a total function of `AgentKind` (`evidence.rs:118-127`); no per-finding override |
| Deterministic verdict authoritative | **Partly** | Computed but unread by the UI; needs a frontend contract change |
| Internal consistency (D5) | **No** | Requires claim extraction; `ExtractionResult` has no claims field |
| Topic / novelty / fit (D1, D2, D6, D10) | **No** | No topic representation; `PaperDigest.summary` is verbatim prose (`paper_corpus.rs:222-235`) — unusable under the privacy invariant |

### 9.1 Correction — what claim extraction would and would not unlock

**As originally written** (the two rows above, preserved unchanged): D5 was recorded as blocked on *claim extraction*, and D1/D2/D6/D10 as blocked on *topic representation*, presented as two adjacent gaps of the same kind.

**That framing was too generous to claim extraction.** It reads as though the four literature-grounded dimensions sit one research step behind D5, on the same dependency chain. They do not.

The distinction, stated precisely:

- **D5 (internal consistency) is genuinely unlocked by claim extraction.** It compares the manuscript's claims against the manuscript's own statistics. Both operands are local, the whole comparison is local, and nothing crosses the proxy. Claim extraction is the only missing piece.
- **D1 / D2 / D6 / D10 are not unlocked by claim extraction.** They compare the manuscript's claims against the *literature's* claims. That comparison requires the claim's semantic content to reach wherever the literature is. The blocker is therefore **the privacy invariant itself**, not a missing extractor.

This matters because a claim's faithful representation is a verbatim span — a claim's content *is* its wording, and the difference between *"X causes Y"* and *"X is associated with Y"* is the entire editorial question. A derived projection (`claim:kind=causal`, `claim:section=results`, `claim:has_supporting_stat=false`) is enough to support local findings and would sit naturally as a tenth `STRUCTURED_PREFIX`, with the verbatim text living in `Finding.detail` — the field the proxy builder already deliberately never reads (`reviewer_agent.rs:345`). But that projection cannot support a comparison against the literature.

**Consequence for sequencing:** succeeding at claim extraction does not move D1/D2/D6/D10 at all. Those four need a separate, explicit decision about the privacy boundary, and that decision must not be allowed to ride on a claim-extraction result. Recorded under the §4.4 doctrine — the original framing is preserved above rather than silently rewritten, because the correction is the useful artefact.
| Escalation adjudication | **No** | Server endpoint absent (`escalation.rs:9-17`) |
| Surfacing `ConfidenceKind` in the UI | **Partly** | `evidence[]` reaches the frontend; nothing reads it |

---

## 10. Coverage

**Traced fully:** the `run_publishready` spine; the six lanes; `compile_report` including all finding classes; the Evidence Model; all four PublishReady proxy crossings with their payload builders and gates; `plagiarism.rs` (both arms); the RAG write surface; `ReportViewerPage` render sites; the proxy server's `/verify` path; the migrations table list.

**Traced partially:** `refverify` connector orchestration (shapes yes, sequencing and retry no); `evidence_store` (call sites yes, SQL no); `orchestrator` routing thresholds (Verification / Plagiarism / AI-detection arms read; RAG and Extraction not); `chunk` and `vector` internals.

**Not traced:** `rag::search` internals — flagged three times and still open; `docparse` format adapters; `stats_verify` numerics; `enclave*`; the frontend beyond `ReportViewerPage`, `publishready/`, and `checks/adapters.ts`.

### Confidence: **Medium**

The same standard ONTOLOGY applies to itself. The spine, the proxy boundary and the finding pipeline are traced end to end and would support High on their own. It is held at **Medium** because `rag::search` remains untraced while §7's analysis depends on RAG's opinion being the precedent, and because two conclusions in §3 and §6 are marked Not Verified at their render sites (`AnalysisEvent::Failed`, `report["evidence"]` consumers) — precisely the producer-without-consumer gap ONTOLOGY §9.4 warns about.

**Single highest-value next trace:** `rag::search` — it closes the last named gap and underwrites §7's precedent.

---

## Status legend

These five labels are project vocabulary and govern the status columns in §§11–12. Defining them implicitly at each use is how they drift.

| Status | Meaning |
|---|---|
| **SHIPPING** | Implemented and verified by trace |
| **MEASURED OPPORTUNITY** | Profiling demonstrated a bottleneck with measurable benefit |
| **OPEN QUESTION** | Evidence shows a question worth investigating; no implementation direction established |
| **UNVERIFIED** | Some evidence exists but the execution path has not been traced |
| **UNMEASURED** | Plausible optimization with no profiling evidence |

---

## 11. Design Intent

> **Nothing in this section is implemented.** These are decisions recorded before the code exists, so that the constraints are fixed while they are still cheap to honour. The §10 coverage rating does not apply here — there is nothing yet to trace. Any statement below that later becomes code must be re-recorded in §§1–10 with a `file:line`.

### 11.1 Hard rule — the Manuscript Understanding Layer produces no judgements

The Manuscript Understanding Layer (MUL) produces **structured evidence only**. It MUST NOT express editorial judgement, quality assessment, recommendation, or opinion.

| Extraction (MUL may emit) | Judgement (MUL must not emit) |
|---|---|
| sentence 142 → Claim → Causal → reports `p = 0.03` | "weak claim" |
| Methods section absent | "poor methodology" |
| 3 causal claims, 0 supporting statistics | "novel contribution" |

**Why the rule earns its keep:** it is what makes extraction accuracy and review quality **independently measurable**. Extraction is scored against a human-annotated ground truth — a claim either is at sentence 142 or it is not. Review quality is scored against editorial agreement, which is a different question with a different ground truth and a different error bar. Collapse them into one component and neither can be measured: a disputed output could be a mis-extraction or a defensible-but-unpopular judgement, and no experiment separates the two.

This is the same discipline as ONTOLOGY §4's confidence rule, applied one layer down. That rule governs *how strongly* a claim about the manuscript may be stated; this one governs *which component is allowed to make one at all*.

**Proposed for ONTOLOGY.md** alongside the confidence rule — not yet inserted there; recorded here first.

### 11.2 Evidence ownership — every fact has exactly one owner

| Fact | Owner |
|---|---|
| text, location, section | Sentence |
| claim kind, hedge | Claim annotation |
| p-value, confidence interval, test name | Statistics annotation |
| judgement | Review Engines — and nothing else |

**No duplication.** If two Review Engines need a p-value, they read the same annotation. Fixing the extractor therefore fixes every consumer at once, and there is no second copy to drift.

This is the graph-level form of the desync-proofing the paired `ReportFinding` already applies at the finding level (`report.rs:187` → `evidence.rs:181`), where `paired()` constructs the `Finding` and its `EvidenceRecord::at_source` together so the two cannot diverge. §2 of this document is a catalogue of what happens without that discipline: six signals computed by one component and silently not consumed by another.

The rule also gives a precise statement of what a MUL bug is: a fact with two owners, or a fact with none.

### 11.3 Capability registry — design intent, do NOT build

Each Review Engine would declare, as data: its inputs, its required annotations, its `ConfidenceKind`, and what it can and cannot review. The Editor then **computes** capability state from those declarations rather than from hard-coded logic.

This generalises §9's dependency chain —

```
novelty unavailable ← no topic representation ← no claim graph ← no claim extraction
```

— from prose in a table into something machine-readable, so that "novelty is unavailable" is derived from a missing declared input rather than asserted by a human who might forget to update it.

It also solves the gap §7 identified. Capability state currently has nowhere to live except a `Finding`, which is why the empty-collection degradations in §6 are silent: there is no non-`Finding` channel for "this could not be evaluated". A registry is that channel, and it removes the pressure to express capability state as a finding about the manuscript.

**Build only when two Review Engines exist to constrain the schema.** A registry designed against one engine encodes that engine's accidents as structure; the second engine is what distinguishes the general shape from the specific one.

### 11.4 Free deterministic value — a distinct backlog tier

Findings requiring **no model, no network, no graph, and no privacy decision**, all `MathematicallyCertain`. This is a separate tier from the T1–T4 roadmap in ARCHITECTURE_MAP.md because its entry cost is arithmetic over data `ExtractionResult` already carries.

1. **Reference never cited in text** — the cheapest unbuilt real finding in the product
2. Citation with no matching bibliography entry
3. Duplicate reference
4. Figure referenced but absent
5. Table never referenced
6. Broken cross-reference
7. Missing DOI
8. Inconsistent reference style

**On (1), stated precisely** — the join key differs by citation style, and the difference matters:

- For `CitationStyle::Numeric` (`citations.rs:19`), `Citation.numbers: Vec<u32>` indexes the reference list directly, so the finding is a **pure set difference** between the union of all `numbers` and `1..=references.len()`. No matching, no heuristic, no threshold.
- For `Parenthetical` and `Narrative` (`:13-16`), `Citation` carries `authors: String` and `year: Option<i32>` while `Reference` carries `authors: String`, `year: Option<i32>`, `title`, `doi` (`:39-45`). Matching is therefore author-surname plus year, which is a comparison rule with genuine failure modes — "et al." truncation, two papers by the same authors in the same year, transliterated surnames.

So (1) is unconditionally deterministic for numeric-style manuscripts and requires a stated matching rule for author-year styles. Recording that split now prevents it being discovered as a surprise mid-implementation, and it suggests numeric-style manuscripts as the first shipping case.

Item (7) is already partly available: `Reference.doi: Option<String>` is parsed (`citations.rs:44`) and `refverify` fetches DOIs, so "missing DOI" is a `None` count over data in scope.

**Not built in the claim-extraction spike.** This tier is deliberately independent of it: nothing here needs a model, and none of it is blocked on the Layer 2 result.

**Every item in this tier has the shape ONTOLOGY §4.9 governs.** Each asks whether one extracted set covers another — references against citations, figures against figure mentions, tables against table mentions. The matcher for item (1) is built and **deliberately unwired** (`b8312b0`) because it measured **precision 0.00** on a real manuscript: the reference side was guarded, the citation side was not. None of these findings can ship until the covering extractor's recall is established, which is a coverage claim about the extractor rather than a claim about any operand. The Join Invariant exists because this tier looked cheap and was not.

**The revision sequence is recorded in [CITATION_EVAL_RESULT_V1.md](./CITATION_EVAL_RESULT_V1.md) §7.** Successive measurements moved the limiting factor four times — assumed low-cost deterministic finding → paragraph reconstruction → citation-side coverage → reference-side parsing — each revision superseding the previous hypothesis on new measurement, and each earlier factor genuinely resolved rather than downgraded. Protocol v1 ended in **REJECT WIRING** with citation-side coverage at 1.0000 and reference-side at 0.50. Read this tier as *measured-and-still-blocked*, not as cheap.

#### Correction — the numeric-style "safe subset" was not safe

**As originally written** (preserved above): item (1) was characterised as *"the cheapest unbuilt real finding in the product"*, unconditionally deterministic for numeric-style manuscripts and needing a stated matching rule only for author-year styles. That characterisation, and the sharpening that produced it, were **both wrong** — and they were wrong in the direction that would have shipped the defect.

**Traced chain.** `parse_reference_list` (`citations.rs:175-177`) maps **one paragraph → one `Reference`**, applying no sort, dedup or filter. `Reference` (`citations.rs:39-45`) carries **no number field**: `parse_reference` never reads a leading `1.` or `[N]`. A reference's position in the vector *is* its implied citation number, by assumption.

**Measured on a real PDF** (`IJAS Manuscript JHA Bombyx haemolymph`, 20 references):

```
parsed references            81      (4.05x inflation)
references with a year       28 of 81      → reconstructed: 18 of 20
index [11]  "ACADEMI.CX AI WRITING REPORT"    ← page furniture parsed as a reference
index [12]  "Page 11 of 13"                   ← page furniture parsed as a reference
```

One bibliography entry becomes up to four `Reference` rows, so every index after the first is wrong.

**The corrected reading.** Numeric-style is not the safe subset — it is the variant whose correctness depends **entirely** on bibliography index fidelity. Author-year matching does not depend on index position at all and is therefore the **more robust** of the two, the opposite of what was recorded. Sequence: fix paragraph reconstruction, add explicit reference-number parsing, then author-year, then numeric.

Recorded under the §4.4 doctrine — the original framing is preserved rather than rewritten, because the correction is the useful artefact.

### 11.5 Positional correspondence is not identity

**When two representations of the same concept are joined, at least one side must be parsed or explicitly identified. Position is not an identifier.**

This is the general form of what §4.6 question 2 catches, and it is worth stating separately from the citation case that produced it.

The uncited-reference failure was **not** numbering drift — no transformation renumbers, sorts or dedups anything. The failure is that the two sides are produced by different kinds of mechanism:

| Side | How the number is obtained | Deterministic? |
|---|---|---|
| In-text `[15]` | **parsed** from the text (`parse_numeric_group`, `citations.rs:130-159`) | Yes |
| Bibliography entry 15 | **inferred** from paragraph position | **No** |

Only the parsed half is deterministic. A join is exactly as reliable as its weaker side, and "they originate from the same document" is not evidence that they stay aligned — it is the assumption that hides the problem.

**The same split can recur silently elsewhere**, and in each case the fix is the same — parse the identifier that is already written in the text rather than counting occurrences:

- **Tables** — `TableRef.label` is parsed from `"Table 3"` (`extract/mod.rs:81-91`), but any code joining tables by *vector index* would reintroduce the defect. Already observed adjacent to this: a caption split across lines produced **4 detected tables where the manuscript has 3**.
- **Figures** — same shape, not yet built.
- **Equations** — numbered in text, would be positional if indexed.
- **Supplementary material** — labelled `S1`, `S2`; position is not identity.

The tell is a join whose key is an array index. Whenever one appears, question 2 of §4.6 applies.

---

## 12. Runtime Architecture

Split into three categories so a reader can tell shipping code from aspiration. **The categories are not interchangeable** — §12.1 is traced fact, §12.2 is one measured number with a named blocker, §12.3 is direction with no evidence behind it yet.

### 12.1 Already implemented

**A substantial part of the runtime architecture that gets proposed for this product already exists. The risk this section guards against is rebuilding it.**

| Capability | Where | Notes |
|---|---|---|
| Model memory planning | `models/mod.rs:619` `plan_deep_load`, `:668` `select_deep_model`, `:424` `fits_free_memory` | Requires **1.5× resident** (`free >= resident * 3 / 2`) so activations fit without swap-thrash |
| Residency budgets | `models/mod.rs:591` `MINI_RESIDENT_BYTES` = 1600 MiB, `:594` `FULL_7B_RESIDENT_BYTES` = 6 GiB | |
| Hardware tier selection | `models/mod.rs:481` `deep_tier`, `:441` `DEEP_PASS_MIN_RAM_BYTES` = 15 GiB | Below the threshold the 7B is unreachable — `ForceTier::Full` sets the tier but `plan_deep_load` still applies the courtesy check |
| Refusal is tested, not assumed | `models/mod.rs:845` `eight_gb_machine_never_plans_the_full_7b` | Asserts across four free-memory values that an 8 GB machine yields `LoadMini` or `Skip(SkippedLowMemory)` and **never** `LoadFull`; `:858` pins that a refusal falls back to the heuristic |
| Three on-disk quantization tiers | 7B `Q3_K_M` (3.5 GB), 1.5B `Q4_K_M` (940 MB), 0.5B `Q4_K_M` (379 MB, bundled) | Resolved by precedence env → `~/gaply-models` → bundled (`models/mod.rs:66-75`) |
| Explicit eviction | `models/mod.rs:1008` `unload_slm2`, called at `pipeline.rs:308` after the verify lane | The one-at-a-time discipline for 8 GB |
| Persistence | `db.rs:25-38` (sqlite-vec registered per connection), `vector.rs:1` (`embeddings` vec0 virtual table) | |
| Report cache | `pipeline.rs:55` `report_cache_key` → `report:v2:{id}`, TTL 30 days (`:43`) | Single definition so writer and readers cannot drift |

**Correction to one item this section was drafted with.** The quantized loader is **not memory-mapped**. `CandlePerplexityModel::from_paths_named` opens the GGUF with `std::fs::File::open` (`candle_perplexity.rs:128`), reads the container with `gguf_file::Content::read` (`:130`), and hands the **file handle** to `ModelWeights::from_gguf` (`:132`, signature at `quantized_qwen2_lowmem.rs:179` — `R: Seek + Read`). Weights are read into heap-resident `QTensor`s. Consistent with measurement: the 0.5B is a 379 MB file and the spike measured **723 MB peak RSS**.

This matters twice over. It is a factual correction, and it is load-bearing for §12.3 — see the caution there, whose original justification depended on an mmap that does not exist.

### 12.2 Measured

**Exactly one runtime item is backed by measurement:** the claim-extraction spike (Layer 2, FAILED on its viability gate at 20.0 min against a 10 min budget).

| Quantity | Measured |
|---|---|
| Mean prompt prefix | 63.3 tokens |
| Mean continuation | 8.4 tokens |
| Scored positions per sentence | **358.5** as built, **105.3** if the prefix were scored once |
| Redundancy | **3.40×** |
| Throughput | **31.2 ms per token position** (38,364 positions / 1196.2 s, 0.5B Q4 on CPU) |
| Projection | ~20 min → **~5.9 min** with the prefix scored once |

**Blocker:** `PerplexityModel` (`ai_detect.rs:63-72`) exposes five methods — `name`, `context_tokens`, `stride`, `tokenize`, `surprisals` — and **no KV-cache reuse**. Scoring N continuations against one prefix therefore re-scores the prefix N times. This is a concrete engineering blocker with a measured payoff, not a speculative optimisation.

**Sequencing constraint, from the same spike.** F2 — the classifier is degenerate (107/107 emitted as claims, `not-a-claim` chosen zero times, 100/107 collapsed to `causal`, root cause: comparing raw conditional surprisal across different continuation strings measures each string's prior, not its fit) — **must be fixed first. A faster degenerate classifier is still degenerate.** The 3.40× is worth collecting only once the thing being sped up produces a usable answer.

**~5.9 min is a projection, not a measurement.** It has not been observed and depends on a trait change that has not been designed.

### 12.3 Direction — unmeasured, record but do not build

Cache hierarchy · runtime scheduler · adaptive concurrency · per-engine resource budgets · layer offloading · memory-ownership enforcement.

**No profiling shows any of these is currently needed.** Nothing runs concurrently — `run_publishready` is one `spawn_blocking` with everything sequential inside (§1) — and one manuscript is processed at a time.

Three cautions, recorded so a future reader does not inherit them uncritically:

1. **A multi-level cache hierarchy with distinct eviction policies is datacenter-shaped.** Distinct policies earn their complexity under multi-consumer contention. This is one user, one manuscript, sequential — the same reason arXiv:2309.06180's PagedAttention was found inapplicable here.

2. **Application-managed layer offloading — do not implement without profiling that identifies a bottleneck the OS cannot address.** *The original form of this caution was: "the quantized loader already mmaps, so offloading would fight the page cache." That justification is false* — see §12.1: the loader uses `File::open` + streaming reads, and weights are heap-resident. The conclusion stands on different grounds: residency is already governed by `fits_free_memory`'s 1.5× gate and by `unload_slm2`, both of which offloading would duplicate or contradict, and no profiling exists. It also leaves a real question open that the mmap claim would have foreclosed: **whether the loader should mmap** is unexamined and may be the cheaper intervention. Measure before building either.

3. **The Review Engine scheduler and per-engine resource budgets are the capability-registry pattern again — designed from zero instances.** One Review Engine exists conceptually; none are implemented. Build when two real engines constrain the shape (§11.3 states the same precondition for the registry, and for the same reason: a design fitted to one instance encodes that instance's accidents as structure).

### 12.3.1 Work nature — "already computed" is not a work estimate

| Item | Nature |
|---|---|
| `matched_year` | **Integration** |
| `citation_count` | **Integration** |
| `shadow_reviewer` | **Product decision** |
| Box 4 verdict authoritative | **Product semantics / UI contract** |

**"Already computed" describes where the data is, not what the remaining work is.** Two of these are wiring — thread a value through a signature and build a finding. Two are decisions about what the product should *say*, and no amount of engineering answers them.

Grouping them as one backlog category — "expose already-computed signals" — is the error this table exists to prevent. It makes a product decision look like an afternoon of plumbing, which is how a decision gets made by default instead of deliberately.

#### Gap — what the UI does with the number, recorded before Group 3 starts

Traced but not previously written down; it lived only in conversation.

**`publication_probability` renders three times** in `ReviewerLetterPanel.tsx:53-60` — as a gauge `score`, an aria-label, and visible text. A severity-count aggregate is not a probability, and presenting one in a gauge implies a calibration it does not have. Substituting the deterministic verdict under the same widget and the same label changes the semantics without changing a word of the language.

**`ReviewerLetterPanel.tsx:107`** currently reads *"Suggested based on the analysis — advisory, not a recommendation to submit."* That hedge was written for an LLM output. For a deterministic aggregate it becomes wrong in the **understating** direction — §4.4 applies in both directions (see its fifth precedent).

**`Recommendation::Unknown`** (`reviewer_agent.rs:109`) means *"the gate could not trust the recommendation"*. For a deterministic aggregator that state is unreachable, so the UI branch becomes dead — or must be repurposed for "no findings to aggregate", which is a different thing wearing the same name.

**`synthesize.ts:45`** builds the same shape for the dev/test path and would drift silently.

### 12.3.2 The Box 4 comparison harness — decisions and two lessons

`reviewer_harness::build_comparison_report` (`reviewer_harness.rs:163`) already computes 21 typed metrics including **`recommendation_agreement`** (`:195`), emitted once at `commands.rs:674` via `tracing::info!`. Four decisions, recorded:

1. **Log-only stays for user-facing purposes.** No UI change. Two recommendations side by side, with no calibrated agreement rate, give a user no basis to choose between them.
2. **Extend the ARTIFACT, not the metrics.** Durable sink, manuscript hash, timestamp, stable cross-machine identity. The metric layer needs nothing — it is already correct and typed.
3. **No one-off live run.** An n=1 result would be quoted later as *"the paths agree"*, and a number outlives its caveats.
4. **Capture the baseline before Group 3.** Promoting the deterministic verdict changes what users are told; measuring that change requires a *before*, and it must be captured while the current behaviour is still current.

#### Lesson — capability complete, evidence pipeline absent

**"The harness exists" was twice treated as equivalent to "the measurement is one run away."** It is not. A live run would work, emit a complete and correct record — and produce **nothing that accumulates**. `logging.rs:9-13` configures `tracing_subscriber::fmt()` with no writer, no file appender, no rolling log, so the record goes to the process's stdout and is gone when the process exits. `run_id` is `manuscript_id.to_string()` (`pipeline.rs:376`), a local DB row id: meaningless across machines, and the same id on two machines denotes different manuscripts. There is no manuscript hash and no timestamp inside the struct.

Stated generally: **an instrument that emits to stdout is an instrument without a record.** Capability and evidence are separate milestones, and a working comparison proves only that comparison is possible — never that the things compared agree. The gap between them is persistence and identity, not logic.

#### Lesson — the non-deterministic counterparty

The wholesale reviewer is an LLM, and is therefore **the only non-deterministic thing measured in this project**. Everything else — extraction, validation, plagiarism cosine, the citation matcher, the reflow — returns the same answer on the same input, which is why Protocol v1 could treat one run on one manuscript as a complete observation of that manuscript.

Agreement cannot. A single disagreement may be genuine divergence or model variance, and the two are indistinguishable without **repeat runs on identical input**. Any future agreement protocol must carry that requirement; Protocol v1 never needed it, so it is not inheritable from the existing protocol family and must be stated explicitly.

### 12.3.3 Open investigation — what is the manuscript?

**Known defect, recorded rather than fixed.** On one real manuscript the text-reuse check reports an 81% *"internal duplication (same manuscript)"* whose quoted text is an AI-detection tool's **cover page** matching itself. Every component behaved correctly: `docparse` parsed what it was given, the plagiarism engine found a genuine internal repetition, and the label is the corrected, accurate one from `83f192c`.

**No component owns the document boundary.** `docparse` decides what is *text*. `sections` decides what is *structure*. Nothing decides what is *the manuscript*. This is the same class as footnote splicing (§B5 of the reflow work), left alone for the same reason.

**Two attempts to fix the symptom without answering the underlying question have now failed** — the paragraph rebuild and the raw-text slice, both measured, both producing 10 self-matches against a baseline of 6. Document scope, structure and downstream semantics are intertwined tightly enough that treating this as a small cleanup risks repeated regressions: scope determines chunk offsets, chunk offsets determine match spans, and match spans are what the user reads. **This needs answering as a question, not patching as a defect.**

**Four candidate rules were evaluated and all rejected:**

| Candidate | Rejected because |
|---|---|
| Drop everything before the title | The title is `split_document`'s first non-empty line, which on this PDF **is** the cover boilerplate. Circular. |
| Drop content before the first recognised IMRaD heading | Deletes genuine front matter (affiliations, keywords) on clean manuscripts, and deletes the entire document when no heading is recognised. |
| Detect known wrappers by signature | Narrow, and unbounded in maintenance — there are many such tools. |
| Exclude `SectionKind::Other` from the plagiarism corpus | **Attempted and reverted — see below.** |

#### Item 3 as a MEASURED NEGATIVE — one attempt, spent

Criteria were **frozen before any code changed**, and the revert followed that rule rather than a judgement formed after seeing the numbers.

| Criterion (preregistered) | Outcome |
|---|---|
| (a) the cover-page self-match (81%, *"0% detected as AI…"*) is absent | **PASS** |
| (b) exactly **5** self-matches remain | **FAIL — 10** |
| (c) those 5 are the same baseline spans, by span identity | **FAIL — zero overlap** |

Recorded so future work does not retrace this path: excising `SectionKind::Other` from the raw text **does** remove the cover-page match, and **does** shift every chunk boundary.

**What criterion (c) caught, and why aggregate metrics were not enough.** Here (b) failed on its own, so (c) was not strictly needed *this time*. But had the count returned 5 by coincidence, **(c) is the only clause that distinguishes "restored" from "replaced"**. Two instances now support that:

- the validation flag set held at **5** while all five reported locations moved (§6 of the reflow work);
- this run could in principle have returned the expected count while replacing every span.

**Identity checks can be necessary even when aggregate metrics are unchanged.** A count is a property of a set's size, not of its membership, and the thing users read is the membership.

**The reflow precedent held — now measured, not predicted.** It was recorded in advance that *every cleanup rule trades one class of false positive for another*, which is why footnote-stripping was excluded from the reflow. This attempt traded one false positive (the cover page) for four new ones. The prediction is now an observation.

#### The measured reason the scope exclusion failed

Excluding `Other` was attempted against three preregistered criteria: the cover-page match absent, exactly 5 self-matches remaining, and those 5 being the same baseline spans by identity. **(a) passed; (b) and (c) failed** — 10 self-matches, and not one baseline span survived.

Two implementations were measured: rebuilding the body from `Section.paragraphs` (whitespace-normalising), and slicing the raw text from the first heading (byte-identical). **Both produced exactly 10.** So the cause is not text fidelity — it is that **chunking is offset-dependent**: removing any leading span re-phases the entire chunk grid, and every downstream boundary moves.

**Consequence for the eventual fix:** "exclude a region" cannot work as an approach at all while chunk boundaries depend on absolute offsets. Either chunking is made offset-independent (anchored to sentence or paragraph starts), or the boundary is decided before chunking rather than by subtraction afterwards. That is chunk-topology work and belongs to this investigation, not to a fix for a symptom.

The criterion that caught it was **span identity, not count** — a count-only check would have read 10 vs 5 as merely "wrong number" rather than "entirely different set", and a run returning 5 different spans would have passed a count check while failing the goal.

### 12.3.4 Guideline ingestion and D13 — RESOLVED, with both root causes

Recorded because the audit that found this misclassified it, and the misclassification is instructive.

**The symptom:** every report showed *"no guideline matches (corpus not seeded yet)"*, and the checklist was four structural items regardless of which journal the user selected. The architecture audit filed this as an unrecorded product hole — *"the guideline corpus is empty"* — implying the feature was never finished.

**It was two defects in two different layers, both now fixed.**

**Root cause 1 — ingestion (`b884afb`).** `strip_block`'s `<head` open pattern also matched `<header>`, and an unterminated block drops the remainder, so a valid 188 KB page became 0 characters and was refused as *"no substantive guideline text"*. Class A, wearing a class D/E message — see ONTOLOGY §4.14. Blast radius included `paper_corpus.rs:388`, so Gap Finder paper ingestion was truncated at the first `<header>` too.

**Root cause 2 — retrieval and scoping (`ceadc29`).** `build_checklist` took a semantic top-5 filtered by `source_type` alone. Two independent failures: the exact strings the detectors match sat in PLOS chunks 1, 6, 7, 8 while the top-5 returned 3, 20, 21, 4, 12 (**zero overlap**), and `source_type` scoping matches *every* journal in a persistent corpus, so a checklist could state another journal's requirements as the target's. Replaced with `rag::chunks_for_source`, scoped by a `guidelines_url` threaded from the frontend (ONTOLOGY §4.13).

**A correction this supersedes.** B1 (bag-of-words embedder) and §9.3 (top-k saturation) were recorded as blocking D13. They are not. The exact strings were present; a semantic embedder would not have been the fix. §9.3 is the proximate mechanism, the design error is upstream of it, and **the audit's own diagnosis was wrong in a way that would have directed months of embedding work at a scoping bug.**

### 12.3.5 OPEN — `extract_word_limit` fabricates

Disabled in `ceadc29`, recorded in code at the call site, and recorded here because a code comment is not a backlog.

Fed real guideline text for the first time, it emitted *"word limit (300 words) — manuscript has 5144 words (limit 300)"* for PLOS ONE, which has **no 300-word manuscript limit**. 300 is almost certainly the **abstract** limit, scraped from an abstract-context chunk and applied to the whole manuscript.

Confident, false, and actionable — the §4.4 class, and **worse than the empty checklist it replaced, because silence misleads no one.** It was harmless only while retrieval starved it of input.

**Re-enabling requires its own chunk scan**, answering one question: is the limit recoverable in context — can a detector tell which number governs which artifact — or is the requirement simply not extractable by regex? Until that is answered, silence.

### 12.4 Memory efficiency principles — design intent

**Runtime optimization targets measured bottlenecks while preserving single-manuscript execution on 8 GB.** Techniques are adopted only after profiling demonstrates benefit; the working order when a constraint is measured is ONTOLOGY §4.8.

This is a list of **techniques**, not architectural layers, each carrying its actual status:

| Technique | Status | Evidence |
|---|---|---|
| Weight quantization | **SHIPPING** | Three tiers on disk: 7B `Q3_K_M`, 1.5B `Q4_K_M`, 0.5B `Q4_K_M` (§12.1) |
| Model selection by available memory | **SHIPPING** | `plan_deep_load` (`models/mod.rs:619`) with `fits_free_memory`'s 1.5× multiplier (`:424`) |
| Explicit model unloading | **SHIPPING** | `unload_slm2` (`models/mod.rs:1008`), called at `pipeline.rs:308` after the verify lane |
| Sequential pipeline execution | **SHIPPING** | The PublishReady pipeline is one `spawn_blocking` with everything sequential inside (§1). **This execution model is why a scheduler is unnecessary today** — there is nothing to schedule. Named for what ships: zero Review Engines exist, and calling this "Review Engine execution" would project tomorrow's vocabulary onto today's code |
| KV-cache reuse | **MEASURED OPPORTUNITY** | 3.40× redundancy, 358.5 positions where 105.3 would do (§12.2). Blocked: `PerplexityModel` (`ai_detect.rs:63-72`) exposes no cache reuse. Sequenced behind F2 |
| GGUF memory-mapping instead of heap-resident `QTensor`s | **OPEN QUESTION** | Raised by the §12.1 correction. **Success criterion: does peak RSS drop below the 723 MB measured for a 379 MB model?** A measurable question, not a plan |
| Activation reuse | **UNMEASURED** | — |
| Shared tokenizer reuse | **UNVERIFIED — needs a trace** | The tokenizer is a shared *file* across Qwen tiers (`~/gaply-models/slm1-adapter/tokenizer.json`, `models/mod.rs:80-98`), and `Tokenizer::from_file` is called inside `from_paths_named` (`candle_perplexity.rs:135`), i.e. per model instantiation. **How many instantiations occur per run across lanes has not been traced**, so whether anything is actually loaded twice is unknown. Not a known opportunity |
| Shared embedding cache | **UNMEASURED** | — |
| Lazy loading of large resources | **UNMEASURED** | — |

**Explicitly not proposed, and why.** No L1/L2/L3 cache terminology. No multi-level cache hierarchy. No CPU/GPU scheduler. No memory-ownership manager. No page-eviction manager.

Nothing measured shows any of these is needed — nothing runs concurrently and one manuscript is processed at a time (§12.3) — and ONTOLOGY §4.7 governs: runtime architecture is expanded only when profiling identifies a specific constraint that existing structure cannot address. The vocabulary matters as much as the structures: naming a cache "L2" imports an eviction-policy problem this product does not have, and the name would outlive the reasoning that introduced it.

---

## 13. UNDECIDED — what "done" means for PublishReady

**This is a product decision and no answer is proposed here.** It is recorded because the ambiguity is real, undocumented, and determines whether the next phase of work is engineering or research.

There is a tier roadmap (ARCHITECTURE_MAP T1–T4) and a dimension list (ONTOLOGY D1–D14), but **no statement of which subset constitutes a shippable product**. As a result "PublishReady is X% complete" has two defensible answers that differ by a factor of three.

### Denominator (a) — PublishReady as currently scoped: **~80%**

The checks that exist today, made correct and shipped. Remaining: the cover-page document-boundary defect (§12.3.3), `plagiarism_exact` unwired (B6), residual computed-and-discarded signals (B7), `extract_word_limit` (§12.3.5). **A planning figure.**

### Denominator (b) — the intended product, D1–D14: **~25%**

Five dimensions have no capability at all (D1, D2, D5, D10, D12). Four more are partial: D6 is fetched but never aggregated, D9 is tables without figures, D11 fires only if a guideline says "conflict", D14 has no external arm. The two flagship reviewer questions — *is this novel* and *does it matter* — are recorded as **"Unanswerable today"**. **A product figure.**

### Why the choice matters

The distance between the two is not schedule, it is **kind**. Everything in (a) is engineering. The distance to (b) runs through B3 (topic representation, *"Research, Unbounded"*) and through the privacy invariant, which §9.1 established is what actually blocks D1/D2/D6/D10 — not claim extraction.

Choosing (a) makes the next phase a finite engineering programme. Choosing (b) makes it a research programme with an unbounded component. **Nothing in this repository currently records which one is intended.**

---

## 14. Release instruments — coverage and risk

**Authoritative statement of what `read_report.rs` covers.** Its header points here; the header must not restate this informally, or the informal version becomes the one people read.

ONTOLOGY §4.11 makes an end-to-end author review a release gate. This section records **which production stages that gate actually reaches** — deliberately as coverage *and risk*, not a percentage, because the stages differ in importance and a single number hides exactly that.

### The matrix

| Production stage | `read_report.rs` exercises it? | Defects found there, and by what | Risk if untested | Recommendation |
|---|---|---|---|---|
| **Guideline ingestion** (`ingest_guidelines` → `GuidelinesIngestor`) | **Partly** — wired in only *after* a gap audit drew a wrong conclusion from its absence. Not via the command | **TWO**, both by **manual probing**, neither by the gate: `strip_block` boundary match (`b884afb`); `source_type`-only scoping (`ceadc29`) | **Highest measured.** Two defects, zero gate detections. One emptied D13, the product's clearest differentiator; the other produced *incorrect* reports | `release_gate.rs`, via the real command, with two-journal and failed-ingestion setups |
| **Manuscript hashing** (`harness_log::manuscript_sha256`) | No | None | Low — unit-tested, content-addressed | Cover incidentally |
| **6-lane pipeline** (`run_pipeline_measured`) | **Yes** — this is what it drives | Chunk-boundary regression, 6 → 10 self-matches, found **by the gate** with 683 tests green | Low — well covered | Keep in `read_report.rs` |
| **Report cache** (`report:v2:{id}`) | **Reads it directly**, bypassing `get_report` | None | Medium — the key was bumped this session; only the write path is exercised | `release_gate.rs` should read through `get_report` |
| **Targeted escalation** (`run_targeted_escalation`) | **No** | **Unknown** | **High.** It writes the `evidence` table, the shadow path's ONLY input. `shadow_findings_sent == 0` is its documented failure signature and nothing exercises it | `release_gate.rs` |
| **Reviewer payload** (`build_review_payload`) | **No** | **Unknown** | **High.** Enforces the privacy boundary — `detail` dropped, provenance filtered to nine prefixes, `MAX_FINDINGS = 12`. Never asserted end to end | `release_gate.rs` |
| **Shadow synthesis** (`run_shadow_synthesis`) | **No** | **Unknown** | **High.** Its `None` branch produced the silent-no-record defect — fixed, never exercised in production | `release_gate.rs` |
| **Wholesale reviewer** (`verify_with_envelope`) | **No** | **Unknown** | **High**, and **blocked** — needs a live authenticated proxy | `release_gate.rs`, gated on proxy availability |
| **Comparison harness + sink** (`build_comparison_report`, `harness_log::append`) | **No** | **None by the gate** | **Reduced, not closed.** The sink is now **confirmed working in the real app** (runs 20 and 21, §17/§18) — but by manual capture, not by an instrument. The stage is exercised; nothing *asserts* it | `release_gate.rs` — still needed, so the confirmation survives without a human running the app |
| **UI aggregation** (`adaptOutcome`, `ReportViewerPage`, `ReviewerLetterPanel`) | **No** | **Unknown** | **Medium–High** | **OPEN ITEM — see below.** Out of scope for a Rust instrument |

### Why the gap survived three successful gate applications

**The instrument looks effective because it works well on the thing it can see, and its track record was accumulated entirely outside its blind spot.** The stage it covers has the best defect record — three gate catches, each with every component metric green — while the stage with the worst record, guideline ingestion at two defects and zero gate detections, is the one it barely touches.

That asymmetry is self-reinforcing: every success is drawn from the covered stage, which raises confidence in the instrument as a whole, which makes the uncovered stages *less* likely to be examined. **This is the rationale for two complementary instruments rather than one more complicated one** — a single instrument grown to cover everything would still report one verdict, and the verdict would still be dominated by the stages it happens to reach.

### OPEN — UI aggregation has no instrument at all

Not merely uncovered: **out of scope for any Rust instrument**, and currently unverified by anything.

`report["evidence"]` and `shadow_reviewer` both reach the frontend and are read by nothing (§2, §3). §12.3.1's render gap — `publication_probability` drawn three times, the `:107` hedge, the unreachable `Recommendation::Unknown` branch, `synthesize.ts:45` drift — is recorded but **unverified by test**.

This needs a **frontend test**, and no such test is planned. Recording it here so that "covered by `release_gate.rs`" is never assumed of it.

### The two instruments

| | `read_report.rs` | `release_gate.rs` (proposed, §14.1) |
|---|---|---|
| Drives | `run_pipeline_measured` | the Tauri command path |
| Speed | fast, deterministic | slow, network-dependent |
| Purpose | development | pre-ship gate |
| Catches | composition defects in report assembly | command-path, privacy, persistence, liveness |

**Neither replaces the other.** Of six defects in this session's history, `read_report.rs` caught three and `release_gate.rs` would have caught the other three — a disjoint split, not an overlap.

---

## 15. The proxy response contract

Two defects traced when the proxy first went live. Both were invisible until then: every prior run degraded before reaching a real model — Ollama absent, proxy unreachable, or 401 — so neither path was ever exercised against live output.

### 15.1 Shadow narrative — the contract mismatch

**FACT, and the authoritative statement:**

> **The server does not request schema-enforced JSON, while the client requires schema-valid JSON.**

Objectively true from the trace, and it holds regardless of model behaviour:

* **Client:** `serde_json::from_str(result.text)` (`proxy_client.rs:231-232`) errors on anything non-JSON — a strict expectation.
* **Server:** the OpenAI request body (`openai_client.py:~55-64`) carries **no `response_format`, no `json_schema`, no `strict`, not even `{"type": "json_object"}`**. Greps for all four return zero matches in `openai_client.py` *and* `claude_client.py`.
* **Prompt:** the system message (`openai_client.py:20-23`) says *"Return concise, structured findings."* — it does not say "JSON", does not name a schema, and does not constrain format.

**The two requests share one response configuration.** `main.py` has no task dispatch; both traverse a single `provider.complete(payload)` that builds one body shape. Only the prompt content differs.

**Interpretation, subordinate to the fact above:** the observed asymmetry — wholesale parsed, shadow failed at column 2 — is consistent with both prompts depending on the model's formatting, one succeeding and one not. That reading is not required for the contract mismatch to be a defect, and **it rests on a single observation. No rate of non-JSON replies has been measured.**

**Also eliminated by source, without instrumentation:** the hypothesis that the two client paths expect different schemas. `verify` is a one-line delegation to `verify_with_envelope` (`proxy_client.rs:250-254`) discarding only the metadata. There is one implementation, so a client-side contract difference is impossible.

#### The fix, scoped to what was traced

**For the OpenAI path, adopting Structured Outputs with a schema matching the Rust response types would eliminate this class of formatting failures.**

The class spans providers — `claude_client.py` has no equivalent enforcement — so the architectural statement is:

> **Every provider used behind a schema-strict client must provide an equivalent schema guarantee or an explicit adaptation layer.**

`main.py:146-193` selects the provider server-side, so fixing only the OpenAI path leaves the class open on the other.

**The schema must be derived from what the client deserializes (`ReviewerEvaluation`, `CitationVerdict`), not from the prompt's description of those objects.** Otherwise a valid-schema-A reply meets an expects-schema-B parser, the failure looks like *"Structured Outputs didn't work"*, and the real cause is schema drift.

**Instrumentation is retained regardless** — log the first ~200 chars of `result.text` on inner-parse failure. Under enforcement this is a diagnostic safeguard, not the primary fix: a fallback path, a provider swap or a server regression would all reproduce this failure, and the log is what identifies which. Without it, an enforced system that fails looks identical to an unenforced one that fails.

### 15.2 `verify_citations` — unbounded payload

**FACT.** `POST /verify` returned **422**: *"total text content is 16255 chars (limit 8000)"*.

`validate_structured` (`gaply-proxy/app/validation.py:39-61`) walks **every string leaf anywhere in the payload** and sums their lengths, so `MAX_TOTAL_CHARS = 8000` is a **global budget across the whole request** — not per-field, not per-citation. The separate `max_field` prose check did **not** fire.

`verify_citations` has **no cap** — no `.take(N)` anywhere in the assembly. **Payload size grows approximately linearly with reference count and is unbounded; this 28-reference manuscript already exceeds the limit by roughly a factor of two.** One data point supports that and nothing broader.

**Classification: payload-size bug.** Not a privacy-boundary violation — `Reference.raw` is deliberately excluded (`verify_agent.rs:147-149`), abstracts are excluded by design (`:210-211`), all fetched text passes `llm_safe()`, and the `max_field` prose check did not fire. The proxy's phrase *"send a structured summary, not raw manuscript text"* is **its own validation vocabulary, not a finding about what was sent** — reading it as evidence of a breach is the §4.14 error.

**This corrects a recorded diagnosis.** All-UNKNOWN citation verdicts were attributed to Ollama being unavailable. This run **reached the cloud proxy and was rejected on size** — a different cause with a different fix.

**Note for §14:** `release_gate.rs`'s PRIVACY invariant covers `build_review_payload` only. **`verify_citations` has never been asserted** — no size check, no privacy check. That coverage gap is real independent of this defect's class.

---

## 16. Box 4 promotion — adversarial audit

**Recorded as it stood at audit time.** F1's fix is measured and green and lands in the commit immediately following this one; it is recorded below as it was found, not rewritten in past tense.

### Coverage of the audit itself

| Area | Completed? |
|---|---|
| Paths to the Box 4 verdict · unmeasured assumptions · gate-missed invariants · reconstructed identity · contract mismatches · silent divergence · single-observation assumptions · n=1 misreading | **Yes** |
| Paths never run against the live proxy | **Partly** — last run's terminal output secondhand, no process access |
| Unvalidated JSONL fields | **Structurally yes, empirically NO** — the sink has never written a record |
| **UI rendering (§12.3.1)** | **NOT TRACED** |
| **Live-proxy runtime behaviour** | **NOT TRACED** |

**The last two are untraced, not clean.** Without this statement an untraced area and a clean area read identically — the release-gate blind spot (§14) applied to the audit.

### Findings

| # | Finding | Class |
|---|---|---|
| **F1** | Every validation flag hardcoded `Critical`; any `Critical` → `Reject` at 0.05 | **A — shipping blocker** |
| **F2** | Known false positives (cover-page self-match, AI-detection `concern`) would directly drive the verdict | **A — shipping blocker** |
| **F3** | `publication_probability` is a 4-value lookup on the recommendation, rendered as a gauge | **B** |
| **F4** | Verdict computed over ALL findings; narrative explains only `MAX_FINDINGS = 12` | **B** |
| **F5** | Escalation's `task:"escalate_findings"` endpoint absent server-side (grep of `gaply-proxy/app/*.py`: zero matches) | **C** |
| **F6** | `Recommendation::Unknown` unreachable from the aggregator; empty analysis → `Accept` 0.92 | **B** |
| **F7** | No JSONL field ever validated against real data | **D** |
| **F8** | `run_id` collision on re-runs — **hypothesis CLOSED BY TRACE** | **E** |
| **F9** | `release_gate.rs` asserts nothing about the verdict it exists to gate | **D** |
| **F10** | Two open §4.16 contract mismatches (`verify_citations` bound, schema enforcement) | **B / C** |
| **F11** | n=1 misreading mitigated only by documentation | **E** |

**F8 in full, because a closed hypothesis is a result.** I suspected re-running a manuscript would reuse `run_id`, failing `evidence_persist` on a duplicate and silently zeroing the shadow input — the documented `shadow_findings_sent == 0` signature. **`create_manuscript` INSERTs a new row per run (`db.rs:170-175`), so `run_id` is unique per run.** Closed by trace, not left as a worry.

### F1 — the axis conflation, a SECOND INSTANCE of Gap E2

Not a new gap. ONTOLOGY §2.1's Gap E2 records the confusion of **certainty** with **editorial weight** on the `CertaintyTier` axis. F1 is the identical confusion on the **severity** axis, never previously recorded.

The hardcode was deliberate — `// hard constraints: every deterministic rule flag is CRITICAL` — and conflates two orthogonal claims. *"Hard constraint"* is **epistemic** (the Maths engine's verdicts are never voted on) and already has two correct homes: `Opinion::hard_constraint` and `CertaintyTier::MathematicallyCertain`. `FindingSeverity` is **editorial urgency**. The producer meant *"never voted on"*; the consumer read *"fatal"*.

The contradiction was visible in shipped output: the finding rendered `[critical]` while its own provenance line read `rule:MissingEffectSize (MAJOR)`, and `store_validation` wrote **Major** to the `findings` table for the same flag.

**Measured blast radius — composition AND order unchanged on this manuscript.** The sort is `severity → tier → confidence` (`report.rs:536-544`) and `MathematicallyCertain` ranks first *within* a band, so the validation findings led on **tier** all along. **Ordering never depended on the conflation.**

That settles the justification: the severity hardcode was **redundant and inert for its apparent purpose** — it did nothing except feed an aggregator that read it as something else.

**The exact 5 + 6 + 1 = 12 fit is coincidental.** Post-fix the Major band is larger, so the coincidence is easier to break: a manuscript with 13+ Major findings would see a real selection change in the reviewer payload.

### The `stylo_finding` near-miss

`report.rs:626` takes `dev: Deviation`, uses it for `confidence()`, and hardcodes `severity: Minor`. **Not a contradiction** — `Deviation` (`Notable` / `Moderate`) carries no severity claim, so nothing is being overridden. Recorded because it is the same shape: an available signal discarded on the severity axis.

### The baseline capture was never the last blocker

It records the **before** state. It does not establish that the **after** state is acceptable. Framing it as the final gate was wrong: promotion needs a comparison of the two verdicts across manuscripts, and `release_gate.rs` asserts nothing about the verdict at all (F9).

### The strongest argument to reject promotion, as it stood

> The deterministic verdict has never been compared against the recommendation it would replace, on any manuscript — and the only manuscript we have evidence for shows it would output "Reject, 5% publication probability" for a paper whose actual defect is that it omits effect sizes. Promotion does not swap one recommendation source for another of comparable behaviour; it replaces a moderate LLM judgement with a decision tree that rejects ordinary manuscripts, and routes every known false positive straight into the headline verdict.

F1's fix removes the `Critical → Reject` mechanism. **F2, F3, F4, F6 and F9 remain open.**

---

## 17. Box 4 capture attempt — run 20

**Outcome: a successful operational run, NOT a valid reference baseline.** Those are related and not the same thing.

### The positive result — the instrumentation is operational

This is the main outcome. End to end, on a live proxy, in the real app:

* comparison report emitted
* JSONL sink written — `<app_data_dir>/box4_comparisons.jsonl`, 2772 bytes, exactly one parseable record
* manuscript hash recorded
* payload digest recorded
* both recommendations recorded
* disagreement recorded
* persistence working

**The baseline failed for an IDENTITY reason, not an instrumentation one.** That changes the remaining work from *building the instrumentation* to *capturing the correct reference run*.

### `manuscript_sha256` is now a demonstrated capability, not a design argument

The instrumentation recorded a hash; the hash did not match the intended frozen manuscript; the freeze condition failed on that basis; and **the incorrect record was not accepted as the baseline.**

| | sha256 |
|---|---|
| Record (run 20) | `78c0aebde07c13a06e218eaa34eda23bb2205dd3d38a2a26ece540d02c6b1722` |
| Frozen manuscript, verified on disk | `859880647c4579c34bc63b82c2280ab08bceac917a9547e0c839a821bc5bafd7` |

`run_id: 20` is a local row id and would have told us nothing. Without the hash this record would have been frozen as the reference for a manuscript it does not describe.

### The freeze condition

| Condition | |
|---|---|
| Same manuscript hash | **FAIL** |
| Same pipeline configuration | Independently failing — see the guideline difference below |
| 42 → 8 persisted drop explained | See below: **invalid comparison**, not an explanation |

**Step 6 not done.** Steps 3–5 passed on their own terms.

### The 42 → 8 drop — an INVALID COMPARISON, not an explanation

**The apparent 42 → 8 anomaly dissolved because the two runs were not on the same manuscript. Since the identity precondition failed, no causal inference about the count difference is warranted.**

Recording it as "explained" would imply an inference that was not drawn.

**`findings=48` was never evidence the runs were comparable.** That log line comes from `store_extraction`'s per-statistic and per-reference rows in the `findings` table — not report findings. An investigation instruction to *"trace 48 → ? → 8"* chased a number that did not mean what it was taken to mean. Worth recording as a methodological note: **a number appearing in two logs is not a shared quantity until its producer is traced.**

### Run facts

* `wholesale_recommendation` **OBSERVED** (`minor_revision`), `recommendation_agreement` **OBSERVED** (`false`) — both gated on `wholesale.available`, so a genuine live call, not the digest standing in for one.
* `wholesale_payload_digest` **populated** (`1f942a0d…`), confirming it is not computed on the wrong side of a branch.
* `model_identifier: gpt-4o-mini-2024-07-18`, `stop_reason: stop`.

**Observed disagreement (n=1).** Shadow `major_revision` / 0.3; wholesale `minor_revision` / 70.0.

One execution, one manuscript, one wholesale realization. **Not a rate, not evidence of superiority, and it must never be quoted as either.** The single valid product observation: **had Box 4 been promoted, this user would have seen a different editorial recommendation** — the first concrete evidence of what promotion does.

**F1 confirmed live in this binary:** `critical` is all zeros in the breakdown, and `major_revision` follows from `major: {escalated_verified: 1}`. Pre-fix, the same run would have produced `Reject` at 0.05.

### Two gaps this run exposed

**Gap A — OBSERVABILITY DEFICIENCY (not a runtime defect).** *(CLOSED at `0f26670`; verified in production by run 21 — see §18.4.)* `shadow_findings_sent` is an input to `ShadowInputs`, consumed to compute `shadow_issue_coverage`, and **never serialized**. So the COMPARISON invariant cannot be evaluated from the artifact because only one side is persisted. The `wholesale_findings_sent` counterpart added earlier has no shadow twin in the output.

**Gap B — layer mismatch, not a broken gate.** `release_gate.rs` currently validates **pipeline behaviour** rather than **persisted operational artifacts**. Therefore COMPARISON and PERSISTENCE cannot become PASS from a successful Box 4 run alone. The gate is not broken; it validates a different layer than the one being authorised.

### Unresolved before the retry

**The guideline configuration.** Run 20 ingested `chunks=4`; the PLOS page previously produced 29. Either a different journal URL was used or the same page returned far less. Both are configuration differences that break comparability, and this must be settled before the reference run.

> **RESOLVED in §18.6.2** — a different URL, and not a guidelines page at all. Run 20 ingested the **BMJ homepage**. The PLOS submission-guidelines page has never been ingested by the application.

### What the retry needs

The verified frozen manuscript (`859880…`, *IJAS Manuscript JHA Bombyx haemolymph*), the intended guideline configuration, and the rebuilt app.

---

## 18. Box 4 capture attempt — run 21

**Outcome: the freeze condition failed again, and the run produced a more valuable result than the baseline would have.** `manuscript_sha256` is `78c0aebd…` — the same wrong manuscript as run 20, not the frozen `859880…`. Steps 3–5 passed; **step 6 not done.**

Both records survive and were compared field by field: run 20 at `<app_data_dir>/box4_comparisons.run20.jsonl`, run 21 at `box4_comparisons.jsonl`.

### 18.1 Wholesale non-determinism — measured, n=2

#### The confound comes first: the two runs told the reviewer different target journals

**`summary.journal.name` reaches the model, is not covered by the digest, and is persisted by nothing.** Run 20 ingested BMJ, run 21 BMC Public Health (§18.6.2), so the two runs were near-certainly conducted against different target journals — and a reviewer told *"target journal: BMJ"* versus *"target journal: BMC Public Health"* may legitimately recommend differently. **That would not be non-determinism at all.**

No artifact can rule it out. It is stated ahead of the measurement rather than after it, because a reader who meets the result first will have already formed the conclusion the confound is supposed to qualify.

What follows is therefore **strong evidence of wholesale non-determinism, not a controlled demonstration of it.** §18.7 records the prerequisite that makes the next pair conclusive.

#### The measurement

**The same manuscript, the same finding set, the same model, produced two different editorial recommendations and two different probabilities.**

| Field | Run 20 | Run 21 | |
|---|---|---|---|
| `manuscript_sha256` | `78c0aebd…` | `78c0aebd…` | same |
| `wholesale_payload_digest` | `1f942a0d9f3bac4e26c67382a416399d399a2f4a19cd4c0c4106e333bfb51da2` | *(identical, all 64 hex)* | same |
| `model_identifier` | `gpt-4o-mini-2024-07-18` | `gpt-4o-mini-2024-07-18` | same |
| `stop_reason` | `stop` | `stop` | same |
| **shadow** (deterministic) | `major_revision` / 0.3 | `major_revision` / 0.3 | **same** |
| `finding_breakdown` | *(4-band object)* | *(identical)* | **same** |
| **`wholesale_recommendation`** | **`minor_revision`** | **`major_revision`** | **DIFF** |
| **`wholesale_publication_probability`** | **70.0** | **40.0** | **DIFF** |
| **`recommendation_agreement`** | **false** | **true** | **DIFF** |
| `wholesale_grounded_issues` | 3 | 4 | DIFF |
| `wholesale_runtime_ms` | 3793 | 5001 | DIFF |

`wholesale_grounded_issues` 3 → 4 is a second, independent realization difference on the same input: the number of the model's issues that survived the grounding gate.

**The deterministic side reproduced exactly** — same recommendation, same probability, same breakdown. That is a positive result in its own right and the first cross-run confirmation that the deterministic lane is deterministic in production, not merely by construction.

#### CORRECTION — the input was *not* byte-identical, and the digest does not claim that

`payload_digest` (`reviewer_agent.rs:356-369`) hashes **only the ordered `(id, severity, title)` tuples of `summary.findings`.** It does **not** cover the per-finding `evidence` arrays, the checklist, the journal name, `overall_verdict`, `findings_omitted`, or the supplementary section — all of which are inside `summary` and all of which the model sees (`reviewer_agent.rs:427-441`).

So the digest establishes exactly this: **the finding set forwarded to the reviewer was identical in membership, severity, and order.** "Byte-identical input" is a stronger claim than the instrument supports, and it happens to be false.

Reconstructed independently from the persisted reports (`report:v2:20`, `report:v2:21`):

| Payload element | Covered by digest? | Run 20 vs run 21 |
|---|---|---|
| `findings` — id, severity, title, order | **yes** | identical (8 findings, 0 omitted) |
| `findings[].evidence` (structured provenance) | no | **ONE DIFFERENCE** — see below |
| `checklist` | no | identical (4 items, all `required section: …`, all passed) |
| `overall_verdict` | no | identical (`pass`) |
| `journal.name` / `quartile` | no | **UNKNOWN — not persisted by anything** |
| `supplementary` | no | **UNKNOWN — not persisted by anything** |

The one traced difference is in finding **f7 `Rag: pass`**, severity `info`:

```
run 20:  "swarm:round-table (1 round(s), rescaled weight 0.434)"   confidence 0.43438997237635957
run 21:  "swarm:round-table (1 round(s), rescaled weight 0.437)"   confidence 0.43656041271400425
```

`swarm:` is a structured prefix (`evidence.rs:28`), so that string **is** forwarded. This is the guideline-configuration difference of §18.6.2 propagating: a different ingested page changed the RAG lane's confidence in the third decimal, which changed one provenance string on the lowest-severity finding.

#### What survives, and what does not

**Survives.** A single-digit change to an `info`-severity provenance string is not a credible cause of `minor_revision`/70.0 → `major_revision`/40.0. On the evidence, this is model non-determinism.

**Does not survive: the run is not clean.** The `journal.name` confound stated at the head of this section is untouched by any of the above — it sits outside the digest's projection, so no amount of digest agreement bears on it.

**What would close it:** §18.7. Both halves of it, not either.

### 18.2 The claim this licenses

> **A `recommendation_agreement` value from any single run carries almost no information, because the same manuscript produced both values.**

**n=2 — two realizations, not a variance estimate.** No rate, no distribution, no confidence interval. Two runs cannot distinguish "occasionally flips" from "flips half the time", and nothing here may be quoted as a stability figure.

It bears directly on §16's promotion question. Any argument of the form *"the deterministic verdict agreed / disagreed with the reviewer on run N"* is now known to be an argument about one draw from an unmeasured distribution. §17 recorded a disagreement at n=1 with the caveat that it was one realization; run 21 shows that caveat was load-bearing — the same comparison inverted.

### 18.3 The digest earned its purpose, and revealed its boundary

It was added so *"was the model given the same input?"* would be **answerable rather than inferred**, and it answered it. No other field could have: `finding_breakdown` was identical in both runs and is a property of set *size*, not membership — its own module doc says so (`reviewer_agent.rs:344-349`).

#### FINDING — the field name asserts a guarantee the producer never made

Not an aside. This is **ONTOLOGY §4.16** with the *name* as the mismatched contract:

> `wholesale_payload_digest` reads as a digest of the payload. It is a digest of a **projection of one field** of the payload — `summary.findings`, tuple `(id, severity, title)`.

The misreading it produced is on record: this session's own conclusion, *"byte-identical input to the reviewer"*, was drawn by a consumer who assumed a guarantee the producer never offered. **The doc comment is accurate and the name is not, and the name is what gets read at the call site and in the JSONL.** `commands.rs:688` writes it, and every downstream reader meets the field with no doc comment attached.

Two properties make this the §4.16 shape rather than a cosmetic complaint:

* **It is undetectable by testing the consumer in isolation.** Every unit test asserting the digest is stable, changes with membership, and ignores order-preserving no-ops passes — and none of them can detect that a reader will over-read the field's scope.
* **It fails toward false confidence.** A too-narrow digest that agrees says "same input" when the input differed; a too-broad one would merely say "not comparable" and stop. **The failure direction is the dangerous one.**

**Recorded as a general rule for instrument design: a digest is only as good as the extent of what it covers, and its name must not imply more.** The remedy is §18.7, which changes the coverage rather than only the name — renaming alone would leave the same blind spot with a more honest label.

### 18.4 Gap A — CLOSED, verified in production

| | Run 20 (schema 1) | Run 21 (schema 2) |
|---|---|---|
| `shadow_findings_sent` | absent from the record | **8, Observed** |
| `wholesale_findings_sent` | 8, Observed | 8, Observed |
| `schema_version` | 1 | **2** |

Both sides present, both `Observed`, and they **agree at 8**.

**The COMPARISON invariant is evaluable from the persisted artifact for the first time** — verified in production, not only in unit tests. `shadow_findings_sent == 0` is the documented failure signature of the escalation path (§14); a record that omits it cannot distinguish that failure from a healthy run, which was the whole content of Gap A.

The `SCHEMA_VERSION` 1 → 2 bump is doing its job here: the meaning of the field's absence changed, and run 20's record is correctly self-identifying as one where absence means "not recorded", not "zero".

### 18.5 The verification path failed differently — do not conflate with §15.2

| | Run 20 | Run 21 |
|---|---|---|
| Client log | 422 from `POST /verify` | `cloud proxy configured but unreachable` |
| Proxy server log | 422 recorded | **no entry — the request never arrived** |
| Citation verdicts | all UNKNOWN | all UNKNOWN |

**Same user-visible outcome, different cause.** Run 20 reached the proxy and was rejected on payload size; run 21 never reached it. [INFERENCE] a Render cold start is the likely cause of run 21 — consistent with the absent server-log entry, and not established.

**§15.2 is unaffected and still open.** The unbounded `verify_citations` payload was not fixed and not reached; a second UNKNOWN run is not evidence about it either way. Recording this so the two are not merged into "the verification path is flaky" — one is a traced defect with a known fix, the other is transport.

### 18.6 Two configuration problems blocking the reference run

#### 18.6.1 The frozen manuscript — exact location

It is present and hash-verified. Runs 18–21 all analysed a **different** paper (`Thermosensitive Nanoemulsion-Based In-Situ Gel…`, manuscript rows 18–21), which is why the hash never matched.

```
/Users/rishi/Desktop/IJAS Manuscript JHA Bombyx haemolymph (1).pdf
```

Filename, verbatim, including the space before `(1)`: **`IJAS Manuscript JHA Bombyx haemolymph (1).pdf`** — directly on the Desktop, not in a subfolder. sha256 `859880647c4579c34bc63b82c2280ab08bceac917a9547e0c839a821bc5bafd7`.

#### 18.6.2 The guidelines URL — a traced UI defect, not a user error

**Determinable, and determined.** From the `documents` table:

| Run | Ingested URL | Journal | Chunks | What was actually ingested |
|---|---|---|---|---|
| 20 | `https://www.bmj.com` | BMJ | 4 | news headlines, plus a nav strip |
| 21 | `https://bmcpublichealth.biomedcentral.com` | BMC Public Health | **2** | journal-overview marketing and a nav strip |
| — | `https://journals.plos.org/plosone/s/submission-guidelines` | PLOS ONE | 29 | **never ingested by the app** |

A `chunks=2` ingest corresponds to **a journal homepage**, not a guidelines page. Chunk 0 of run 21 begins *"Skip to main content … BMC journals have moved to Springer Nature Link … Publishing model : Open access"*; chunk 1 is an article list ending in *"Journal updates — Supporting the Sustainable Development Goals"*. There is no author-guidance prose in either.

**Root cause, traced.** Picking a journal prefills the URL field from the directory:

* `PublishReadyPage.tsx:318-323` — `onClick` → `setGuidelinesUrl(j.guidelinesUrl ?? '')`
* `journalData.ts:44` — `guidelinesUrl: j.website ?? null`

**The directory field is a journal *website*, not a guidelines page.** Measured over `src/data/scopusDirectory.json`: **258 journals, 258 with a website, and 0 whose URL is an author-guidelines page.** (A keyword scan returns one hit — *Learning and Instruction* — which is the journal's name, not a path.) Every entry is a landing page: `https://www.thelancet.com`, `https://www.nejm.org`, `https://journals.plos.org/plosmedicine`.

So the prefill is wrong for every journal in the directory, and the surrounding copy asserts otherwise — *"Gaply fetches this page and cross-references your manuscript against the real guidelines"* and *"Pick a journal above to prefill its known URL"* (`PublishReadyPage.tsx:341-346`).

#### The audit's result was the SEPARATION, not the rename

**Two unrelated things shared the name `guidelinesUrl`.**

| Name | Carries | Correct? |
|---|---|---|
| `JournalRecord.guidelinesUrl` | the directory's journal **website** | **invented** — renamed to `website` |
| `PublishReadyPage` state, `publishReadyBridge` params, Rust `guidelines_url` | the URL **the user typed** | **correct** — genuinely a guidelines URL |

A rename that had not separated these would have renamed correct code. Consumers of the invented field classified into three buckets:

* **CORRECT** — `journalData.ts:127` domain search: matching a pasted URL against the journal's domain genuinely wants the website.
* **MISNAMED** — the same defect in a second place. `PublishReadyPage.tsx:322` (the prefill) and **`JournalCheckPage.tsx:261`**, which labelled the homepage *"Author guidelines"* on a live screen, uncovered by any test. Both fixed; the label now reads *"Journal website"* (§4.4 — correct the claim).
* **AMBIGUOUS** — `journal.vitest.tsx:48,61`. The fixture values look guidelines-shaped, but **nothing asserts on the field**; they exist only because the type required it. A suggestive literal is not evidence of intent, and these are **not** recorded as defects.

**Confirmed impact, from the persisted reports.** Runs 20 and 21 produced **identical 4-item checklists**, all `required section: …`, all passed — the structural fallback. **Zero journal-derived requirements in either run.** D13 was empty in both, despite ingestion reporting success both times.

**This is worse than an empty field, and that is the point.** Blank means guidelines are skipped and the checklist is visibly structural. A homepage ingests successfully, reports a plausible non-zero chunk count, and yields a checklist indistinguishable from the blank case — **a silent failure wearing the appearance of a working feature.**

#### 18.6.3 Reporting a wrong page — the durable half

Removing the prefill only helps because the field ends up empty. **It does nothing for a wrong URL the user pastes by hand**, which is the case that survives it. The fix is to say what was actually found:

**Three observations, then two interpretations:**

| | |
|---|---|
| **OBSERVATION** | the page was fetched successfully |
| | the fetched URL was *&lt;url&gt;* |
| | none of the requirements Gaply currently detects were found on it |
| **INTERPRETATION** — *"this may mean"* | the page may be a journal homepage rather than an author-guidelines page |
| | or it may contain author-guideline requirements Gaply does not yet detect |

#### Why the URL is named, and why the advice is not

**"That page" is a pronoun with no antecedent visible on the checklist tab**, and the report may be read long after the run with the input field since edited. Naming the URL completes the observation.

**What was correctly removed is the ADVICE.** An earlier draft ended *"…and use that URL instead"*, which **presumed the homepage explanation over the detector-coverage one** — the very asymmetry the wording exists to avoid. **The URL itself presumes nothing.**

#### The wording scopes the claim to detector coverage, not to the page

**Only three detectors exist** — `"structured abstract"`, `"conflict"`, `"vancouver" | "numbered"` (`report.rs:1232,1247,1260`). **A genuine author-guidelines page stating none of those produces zero hits.** Many journals require no structured abstract, and plenty use author-date rather than numbered citations. An earlier draft read *"it may be a journal homepage rather than the guidelines"* — which would have sent that user hunting for a page that does not exist.

Two properties make the replacement honest:

* **It scopes the claim to our coverage rather than the page's content.** *"Requirements that Gaply currently detects"* cannot be read as *"the page has no requirements"*, and it accommodates future detectors without becoming wrong.
* **It separates observation from interpretation.** Zero detector matches is what was measured; everything after *"this may mean"* is explicitly hypothesis. Both explanations are named and neither is privileged.

**This is §4.14 avoided rather than committed** — a correct absence, with a cause that would have misdirected the remedy.

**The deciding argument is the dependency graph, not single-source-of-truth.** The warning depends on a guideline-derived count produced by `checklist_from_guidelines` (`report.rs:1169`), which requires an `ExtractionResult` — and no such thing exists while a URL is being fetched. Once that holds, running detectors at ingest time is not *"more immediate"*; it is *"must duplicate or approximate later logic"*. The chosen approach computes the fact **once, where its inputs already exist.**

#### The design survived its own plumbing error

The proposal was to thread a guideline-derived count out of `run_pipeline_inner`. That turned out to be unnecessary. **The choice between options 1, 2 and 3 was decided by the dependency graph, which does not depend on the plumbing estimate** — so being wrong about the mechanics did not invalidate the option chosen. Worth recording as a property of the reasoning, not a lucky escape: an argument resting on a structural constraint survives an incorrect implementation forecast, where one resting on effort would not have.

**The plumbing was smaller than the design assumed.** `ChecklistItem.guideline_source` (`report.rs:167`) is already serialized and already reaches the render site — `ReportViewerPage.tsx` reads it for the existing empty state. No Rust change and no new count were needed; the report on the wire already carried the answer. What was missing was one bit the report cannot know: **whether a URL was supplied at all.**

**Two empty states, not one.** The existing message says *"No target-journal guidelines were provided"* — **false when the user did provide one and it yielded nothing**, which is exactly what a homepage produces. `ChecklistView` now distinguishes them, and the two are mutually exclusive.

`PublishReadyPage` holds the URL of the **completed run** separately from the input field, which the user may edit afterwards: the message describes the run that produced the report, not the current form state.

**`GuidelinesReport.note` is unchanged.** It keeps saying only what ingestion can honestly know — that bytes landed. The §4.14 correction is to answer the user's question *somewhere*, not to make ingestion claim knowledge it does not have.

##### Revisit condition

> **Revisit extracting a shared guideline-analysis engine when multiple consumers need guideline-only predicates independently of manuscript evaluation.**

Stated structurally rather than by detector count, because the trigger is a second independent consumer, not a third detector.

#### Confirmation for the retry: the URL must be typed in

The field starts empty (`PublishReadyPage.tsx:100`, `useState('')`) and is optional — blank runs structural checks only. But it **does not stay empty once a journal is picked**: selecting one overwrites it with that journal's homepage. So the correct sequence is **pick the journal first, then replace the prefilled URL** with the real guidelines URL:

```
https://journals.plos.org/plosone/s/submission-guidelines
```

Expected ingest: **`chunks: 29`**. Any other count means a different page was fetched and the run is not the reference run.

### 18.7 Reproducibility metadata — BOTH halves, not either

**Recorded as a decision, and as a prerequisite for the baseline retry — not only for the promotion comparison.** Without it, a repeat pair cannot distinguish model variance from a changed prompt, which is precisely the state §18.1 is in.

Two mechanisms answering two different questions. Neither substitutes for the other:

| | Persisted named fields (`journal.name`, guidelines URL, …) | Hash over the **whole** `summary` |
|---|---|---|
| Answers | **WHAT** differed | **THAT** something differed |
| Strength | names the changed input, so the difference is diagnosable | catches variation **nobody thought to persist** |
| Blind spot | silent about any input not on the list | a bare inequality — cannot say what moved |

**The concrete case for each is already in the record.**

* Run 20 vs 21: a full-`summary` hash would have said only *"not comparable"* and stopped. **Persisted fields would have named the journal** — and that is the difference between a dead end and §18.1's confound being stated up front.
* The `swarm:` weight `0.434` vs `0.437`: nobody would have listed a third-decimal RAG confidence as a reproducibility field. **Only a full hash catches it.** It was found here by hand-diffing two cached reports, which does not scale and will not happen next time.

**Versioning sensitivity is real and is what `schema_version` now exists for.** A full-`summary` hash changes value whenever the payload's *shape* changes — a new field, a reordered object — so hashes are comparable only within a schema version. Run 20 (v1) and run 21 (v2) already demonstrate the boundary the version field is there to mark: v1's *absence* of `shadow_findings_sent` means "not recorded", v2's would mean "zero". The same reading applies to a shape-sensitive hash.

**Scope:** additive to `ShadowComparisonReport` — no runtime behaviour changes, and nothing a user sees.

---

#### 18.7.1 Serialization findings — checked before implementing

Two properties, and they came out differently.

**DETERMINISTIC — holds.** `serde_json::Map` is a `BTreeMap`: the `preserve_order` feature is not enabled anywhere in the dependency graph, verified with `cargo tree -e features` rather than assumed from the manifest, since features are additive and any crate in the build could have turned it on. Key order is sorted and stable; floats use shortest-round-trip formatting, a pure function of the `f64`. **Equal inputs produce equal bytes.**

Float values differing in low bits between runs are *input* variation, not serializer nondeterminism — and catching that is the point. The `swarm:` weight `0.434` vs `0.437` (§18.1) was invisible to the findings projection and would have been invisible to any hand-written field list.

**CANONICAL ACROSS EVOLUTION — does not hold.** An added or renamed key, a changed `MAX_FINDINGS` / `MAX_CHECKLIST` bound, or a changed `clamp` length all alter the bytes while the logical review is unchanged. One risk class is structurally absent: `summary` is built by the `json!` macro from literal keys, not derived from a struct's field order, so reordering a struct cannot silently reorder the payload.

##### The design risk, answered rather than assumed

> **Could a `summary` field be added without anyone touching `schema_version`? Yes — so `schema_version` cannot be the boundary.**

`SCHEMA_VERSION` lives in `reviewer_harness.rs`; `summary` is built in `reviewer_agent.rs`. Nothing about editing the second brings the first to mind. **A convention that "you should also bump the schema" is not a guarantee**, and the failure is silent: every historical digest quietly stops meaning what it meant.

Hence a **separate `SUMMARY_FORMAT_VERSION`** on the format itself, and — because a doc comment is exactly the kind of instruction that gets missed — an enforcing test:

`summary_shape_is_pinned_to_the_format_version` pins the exact key set at every level (`summary`, `journal`, each finding, each checklist item, `supplementary`). **Adding one field fails it**, and the failure message names the constant and this section. Verified by mutation: inserting a `"mutation_probe"` key into `summary` produced

> *"the summary key set changed. `summary_digest` hashes these bytes, so this invalidates every historical digest: bump SUMMARY_FORMAT_VERSION…"*

**Two version axes, deliberately separate.** `schema_version` versions the comparison record; `summary_format_version` versions the thing being hashed. They may move independently, and both travel inside the record so a digest is never read without its interpretation key.

#### 18.7.2 What was implemented

| Field | Answers |
|---|---|
| `findings_projection_digest` | were the same **findings** sent? *(renamed from `wholesale_payload_digest`)* |
| `summary_digest` | did **anything** change, including what nobody thought to persist? |
| `summary_format_version` | within which format is `summary_digest` comparable? |
| `journal_name` | what the reviewer was told the target journal is |
| `guidelines_url` | which guidelines page scoped the checklist |

**The rename is the third instance** of a name implying more coverage than its producer delivers — after `guidelinesUrl` meaning *publisher website* (§18.6.2) and F1's hardcoded `Critical` meaning *hard constraint* (§16). **Documenting the first two would have left the misleading name at every call site, which is where the misreading happens.** So this one was renamed, not annotated.

**What `summary_digest` hashes, stated precisely** — because "the summary" could name several things here, and leaving that unstated would recreate the very defect this section fixes:

> `serde_json::to_string(&payload["summary"])`, over the **same `Value` handed to `verify_with_envelope`**, after all deterministic preprocessing and immediately before transmission. Not a reconstructed or logically equivalent object.

`reqwest`'s `.json(payload)` serializes that same `Value` with `serde_json`, and `Value` serialization is context-free — an object emits identical bytes nested or standalone. **Two matching `summary_digest`s therefore mean the same reviewer payload representation**, not merely equivalent objects that serialized differently elsewhere. Computed at `commands.rs:688`, beside the projection digest, from the payload variable that was passed to the call.

`journal_name` and `guidelines_url` are typed absence per §4.12: a new `MetricAvailability::NotSupplied` distinguishes *"the user gave none"* from every other unavailability — nothing is broken and no future capability changes it. A silent `""` could not be told apart from *supplied but empty*.

#### 18.7.3 `SCHEMA_VERSION` 2 → 3

Not because fields were added — adding a field is backward-compatible for readers. **Because what the artifact can support changed.** A consumer can now distinguish four cases that were indistinguishable at v2:

* identical finding tuples with **different** summaries
* identical summaries
* different target journals
* different guideline URLs

**Run 20 vs run 21 was exactly that failure:** matching digests were read as "identical input" when the journal differed and was never recorded. Same test as the `shadow_findings_sent` bump — the meaning of the record changed, not merely its field count.

---

## 19. Box 4 capture attempt — run 22

**Outcome: the closest run yet, and the freeze condition fails on two counts. Not frozen.**

### 19.1 What passed — the configuration problems are resolved

| Freeze condition | |
|---|---|
| `manuscript_sha256` = `859880647c…` | **PASS — the frozen manuscript, for the first time** |
| `schema_version` = 3 | **PASS** |
| Guideline ingest = 29 chunks | **PASS — the stop condition cleared** |
| `guidelines_url` = the PLOS ONE submission-guidelines page | **PASS** |
| Both sent-counts present and agreeing | **PASS — 12 and 12** |
| Both digests populated | **PASS** |
| Deterministic side complete | **PASS** |

Runs 20 and 21 failed on manuscript identity and on ingesting a journal homepage. **Neither recurs.** `findings_projection_digest` `8990774c…`, `summary_digest` `1cb0bb8d…`, `summary_format_version` 1.

### 19.2 FAILED 1 — an internally inconsistent persisted configuration

> **`journal_name` is `"PLOS Medicine"` while `guidelines_url` is PLOS ONE's.**

**The record captures a journal/guidelines mismatch that was previously unobservable. Whether this represents intentional flexibility or missing validation requires investigation.** It is **not** recorded as a defect.

**§18.7's `journal_name` made it visible on its first run.** The same class of confound made runs 20 and 21 uninterpretable with no way to see it: matching digests were read as identical input while the target journal differed and was recorded nowhere. **Second field to earn its place on first use, after `manuscript_sha256`** — which caught run 20's wrong manuscript on its own first run.

### 19.3 FAILED 2 — entitlement exhausted

`/verify` returned **403 `not_entitled` / `no_uses_remaining`** for the wholesale call. `wholesale_path_available` is `false`; `wholesale_recommendation`, `recommendation_agreement`, `model_identifier`, `stop_reason` and every downstream comparison metric are `Unavailable` with `requires_live_proxy`.

**No wholesale-dependent capture is possible under the current entitlement.** The logs establish that this run failed on exhaustion; they establish nothing about how it replenishes. §19.5 answers that from source.

### 19.4 Journal and guidelines are independent inputs, by construction

**FACTS.**

* Two separate UI fields. `journal` is set by picking from the directory (`PublishReadyPage.tsx:318-323`); `guidelinesUrl` is free text the user types (`:333-340`). Since the prefill was removed (§18.6.2) **nothing writes one from the other.**
* Both cross the boundary independently: `run_publishready` takes `journal_name`, `journal_quartile` and `guidelines_url` as three unrelated parameters (`commands.rs:498-504`).
* They are consumed by **different subsystems**. `guidelines_url` scopes the checklist corpus (`report.rs:1161-1165`); `journal` reaches only `summary.journal` in the reviewer payload (`reviewer_agent.rs:435`). **Neither ever reads the other.**
* **No consistency check exists anywhere** — not in the UI, not in the command, not in the core.

**ANALYSIS.** The independence is structural rather than incidental: the two values feed different lanes and were never joined. That is consistent with intentional flexibility — checking a manuscript against journal A's reviewer framing and journal B's written requirements is a coherent thing to want, and the fields' separation permits it.

It is equally consistent with an unnoticed gap. **The code contains no statement of intent either way**, so the question cannot be settled by reading it — which is why §19.2 records the observation and stops.

**What is certain:** *if* divergence is intentional, **nothing surfaces it.** The report shows a checklist sourced from one journal beside a reviewer letter framed for another, with no indication they differ. A user who mistyped a URL, or picked the wrong journal from a directory of 258 similarly named entries, sees exactly what a deliberate cross-check looks like.

**Three honest behaviours, not yet chosen:**

| | Behaviour | Cost |
|---|---|---|
| 1 | **Warn** — surface the divergence, proceed | Needs a reliable journal↔URL correspondence, which the directory cannot supply (0 of 258 carry a guidelines URL, §18.6.2) |
| 2 | **Validate** — refuse to proceed | Forecloses the legitimate cross-journal case on a correspondence we cannot compute |
| 3 | **Record both without comment** | What ships today, and what made run 22 diagnosable at all |

**(1) and (2) both require knowing which guidelines URL belongs to which journal — precisely the datum §18.6.2 established does not exist.** Any check would be built on a guess, which is the class of defect that section exists to close. **No behavioural change is proposed.**

### 19.5 Quota mechanics — traced

**FACTS, all from source.**

| Question | Answer |
|---|---|
| **When is entitlement checked?** | **Once per HTTP request.** `require_entitlement` is a FastAPI dependency on `/verify` (`main.py:240-244`), not a per-run gate |
| **Which endpoint decrements?** | **`/verify` — the only paid endpoint.** `main.py` exposes exactly two routes: `GET /health` and `POST /verify` |
| **What decrements a use?** | **A successful call only.** `consume()` runs *after* `provider.complete()` returns (`main.py:263-267`) |
| **Do failed requests decrement?** | **No.** A 422 raises inside `validate_structured` *before* `provider.complete` (`main.py:247-259`); a 403 raises in the dependency, before the handler body |
| **Do retries multiply consumption?** | **No.** The retry schedule is on `/health` probing only (`proxy_client.rs:167-190`); `verify_with_envelope` makes exactly one POST with no retry |
| **Only wholesale, or every caller?** | **Every caller.** One shared counter, `feature = "publishready"` (`config.py:59`). Four call sites reach `/verify` in a PublishReady run: citation verification, targeted escalation, shadow narrative, wholesale reviewer |
| **Where is the limit set?** | `PUBLISHREADY_LIMIT_PREMIUM`, default **20**; free tier **0** (`config.py:57-58`) |
| **Reset schedule?** | **Automatic, calendar month.** `_period()` returns `date(year, month, 1)` (`entitlement.py:211-213`), and the counter is keyed on it — so a new month is a new row starting at 0. **No intervention required; the next reset is 1 September 2026** |
| **Dev path that avoids production quota?** | **Two.** `GAPLY_ENTITLEMENT_REQUIRED=false` makes `require_entitlement` return `None`, and `consume` is then skipped (`main.py:207-208, 266-267`) — but it is a *server-side* setting that disables the gate for every user. The clean one is the **local Ollama path**, which never reaches the proxy |

#### The arithmetic, and the contract defect behind it

**Escalation sends one `/verify` per batch, and `batch_escalation` produces one batch per Critical record plus one per agent kind** (`escalation.rs:120-137`). A run with findings from several agents therefore issues several calls. Run 22's seven requests — 5×200, 1×422, 1×403 — consumed **5 uses**: the five successes.

That directly contradicts what the Rust code believes:

> `reviewer_agent.rs:429-431` — *"Metering: one run = one metered use (server dedups by `run_id`)"*
> `escalation.rs:140-142` — *"Stamps `run_id` on the payload so the proxy can meter at-most-once per run (server-side dedup — one run = one use; a proxy contract, out of Rust scope)."*

**`run_id` appears nowhere in the proxy.** A repository-wide grep across `gaply-proxy/` returns zero matches, and `consume(user_token)` takes only the user — there is no per-run key and no dedup.

**ONTOLOGY §4.16, fourth instance — and the first with a monetary cost.**

**"Out of Rust scope" is what let it survive.** Rust deferred to a contract it never verified; the proxy was never told such a contract existed; and the phrase closed the question on the only side that was looking. **Neither side was auditable from the other** — the Rust comment describes proxy behaviour no proxy test asserts, and the proxy implements metering no Rust test exercises. A defect that lives in the gap between two components, described by neither's tests, is invisible to both.

#### Impact, worded to the evidence

> **Under the current implementation, a single PublishReady review can consume multiple metered `/verify` calls. Run 22 generated five successful `/verify` requests. If that pattern is typical, the effective number of reviews available under a 20-use entitlement is substantially lower than the headline figure.**

**Established:** the proxy meters per successful `/verify`; multiple such calls occur per run; Rust documents a different assumption.

**NOT established:** that every review always consumes 4–6 calls. `batch_escalation` produces one batch per Critical record plus one per agent kind, so the count varies by manuscript. One run is one observation.

**The contract mismatch itself is proven by source and does not depend on the arithmetic.**

### 19.6 Retry status

**The run is otherwise correctly configured.** Only the journal selection and the entitlement stand between here and a valid baseline. **Development runs consume production entitlement** — worth knowing independently of this capture.

---

## 20. Entitlement — three distinct findings

**Nothing here is implemented.** Findings 1 and 2 need decisions that are not code's to make.

---

## 20.1 FINDING 1 — deployment contradiction (proven; needs a product decision)

> **The deployed implementation and the current customer-facing copy contradict each other. Resolving the contradiction requires either changing the entitlement configuration or changing the customer-facing copy.**

**This leads because it affects users today**, exists **regardless of which metering model is chosen**, and is observable from the deployed system rather than inferred from design. It is not a consequence of the metering question below — it would survive every option in §20.2 untouched.

**The investigation establishes the contradiction, not which resolution is preferable.**

### The two instances, with their evidence distinguished

**PREMIUM — directly observed.**

`tiers.ts:64-66` returns `cap: null, remaining: null` for a premium user, under the comment *"premium is unlimited on everything it can access"* (`:53-55`). The deployed proxy meters. **Run 22's 403 `no_uses_remaining` IS the premium case** — this is observation, not inference. A premium account was refused by the deployed system while the client model held it to be uncapped.

**FREE — inferred.**

`BillingPage.tsx:96` promises *"Citation verification and journal checks are currently unlimited on the free tier."* `publishready_limit_free = 0` (`config.py:57`), and one shared counter serves every `/verify` caller, so a signed-in free user's first citation verification **would** return 403.

**No free account has been observed being rejected.** The conclusion follows from the configuration and the code path, not from a log line. It is well-supported and it is not the same grade of evidence as the premium case.

### Neither tier can see its position

`EntitlementResult.remaining` is computed by the proxy (`entitlement.py:70-73`) and **`main.py` never returns it**. No surface anywhere displays remaining uses. **A user cannot tell how close they are to a limit they were told does not exist** — which is why both instances surface only as a refusal.

### Immediate options for the free tier

| Option | Effect | Cost |
|---|---|---|
| **Correct the copy** | aligns messaging with deployment | no OpenAI cost; users lose the "unlimited" promise |
| **Raise `limit_free` above 0** | makes the existing promise true | **every free `/verify` incurs OpenAI cost on Rishi's account** |
| **Small monthly free allowance** | consistent after updating both | predictable cost, still allows free evaluation |

---

## 20.2 FINDING 2 — architecture investigation (options a–d)

The question is not only where dedup lives but **whether "one run = one use" is the right model at all.** The four callers do different work at different pipeline stages; that may be four legitimate uses under a higher limit rather than one use needing dedup.

**Context.** `feature = "publishready"` is a **single counter for all `/verify` traffic** (`config.py:59`), so citation verification, targeted escalation, shadow synthesis and the reviewer all draw on an allowance named after only the last of them.

**On the customer-facing wording:** no surface states any number, and there is no Stripe integration in this repository. **There is no "20 uses" claim to be mismatched against** — see §20.1 for what the copy does say.

---

### (a) Proxy dedups by `run_id`

Makes the proxy honour the contract Rust already documents.

* **"A use" means** — one PublishReady review. The most intuitive of the four, and the only one matching the existing Rust comments.
* **Retry** — dedup suppresses double-charging after a failure, which is the easy half. **The hard half is not an implementation detail: `run_id` is `manuscript_id.to_string()`, a stable DB row id, so naive dedup changes what constitutes a billable review rather than merely suppressing retries.** Re-running the same manuscript ten times after revisions would cost one use, permanently. Fixing that means changing what `run_id` *is*, which is wider than the dedup.
* **Cost — highest.** `run_id` into every `/verify` payload (escalation and the reviewer send it; citation verification does not), storage keyed `(user_id, run_id)`, a retention policy defining how long a run stays "the same run", plus the billable-review decision above. New table, new migration, new expiry job.
* **Migration** — **silently increases effective value for every current subscriber against unchanged balances.** A subscriber who has consumed 15 of 20 keeps that number while future runs cost ~1 instead of ~5. A pricing change delivered as a bug fix, and not reversible without taking value back.

### (b) Rust consolidates calls

* **"A use" means** — still one `/verify`, but nearer one per run.
* **Retry** — unchanged, with a larger blast radius: one failure loses four results instead of one.
* **Cost — high, and partly impossible.** The four callers have genuine cross-stage data dependencies — citation verification needs `refverify` output, escalation needs the compiled report's evidence rows, shadow synthesis needs the escalation verdicts, the reviewer needs the finished report. **Escalation is intrinsically multi-call:** `batch_escalation` (`escalation.rs:120-137`) splits by severity and agent kind *by design*, one batch per Critical record plus one per agent. Consolidating means re-architecting the pipeline for a billing reason.
* **Migration** — none. Balances keep their meaning; runs simply cost less.

### (c) Keep per-call metering, raise the limit

* **"A use" means** — one cloud call, which **no customer would infer** from copy saying "unlimited". Honest only once the copy says so.
* **Retry** — **the cleanest of the four, because it is the one that already exists.** A 422 raises before `provider.complete` and a 403 before the handler body, so only successes ever charge.
* **Cost — lowest. One environment variable.** No new machinery, no migration, no schema change.
* **Migration** — additive: no subscriber loses anything and stored counters keep their meaning. **Describing** it needs a transition story, since a number would have to appear where "unlimited" currently does.

### (d) Meter per caller — internal calls unmetered, only the reviewer a "use"

**This leads, and the reason is specific: it makes EXISTING copy true rather than requiring new copy.** Basic's *"PublishReady simulated review"* becomes exactly what is metered, and Pro's *"unlimited online verifications"* becomes literally true once citation verification stops drawing on the counter.

* **"A use" means** — one simulated peer review.
* **Retry** — good. One metered call per run; a failed reviewer call never reaches `consume`, and internal calls cannot exhaust the allowance. **It also resolves §20.1's free-tier instance directly**, by moving citation verification off the metered feature.
* **Cost — moderate, and the machinery exists.** `entitlement_feature` is already config (`config.py:59`) and `usage_counters` is already keyed by `feature`. The work is per-feature limits in config and feature classification at the proxy. **No new table, no dedup, no retention policy.**
* **Migration** — cleanest of the four. Existing `publishready` rows keep their meaning; new features start empty. Effective value rises while the headline feature's semantics are unchanged.

> **CAVEAT, at the same weight as the recommendation.** Feature classification **must be derived by trusted server-side logic — endpoint, or validated request shape — never from a caller-supplied identifier.** A client-declared feature is a client assertion: a tampered client would claim the unmetered feature for a paid call. **This follows directly from what `require_entitlement` exists to enforce** — the module's own opening line is that the client's gating is presentation only and a tampered client can skip it. Accepting a caller's word for which counter to charge would reintroduce exactly that hole one level down.

---

### Cross-feature note — §15.2 and entitlement are coupled by the current model

**ESTABLISHED.**

1. Run 22 issued **seven** `POST /verify` requests: one 422, five 200, one 403.
2. The 422 came from `verify_citations` exceeding the proxy's 8,000-character validation limit (§15.2).
3. **A 422 exits before `consume()`**, so it does not decrement entitlement (`main.py:247-259` raises inside `validate_structured`, ahead of `provider.complete`).
4. If §15.2 is fixed, that call becomes **eligible to succeed** rather than failing at validation.

**PREDICTION, not consequence.**

> Under the current implementation, the unbounded `verify_citations` payload generates a `/verify` request that fails validation (422) before entitlement consumption. **If §15.2 is corrected without changing the entitlement model, that request is expected to become a successful metered verification.** The current metering design therefore creates a coupling between payload correctness and entitlement consumption that should be considered when sequencing the work.

That the repaired request reaches `consume()` **follows from the current design but has not been observed.** It is not stated as fact.

#### The sequencing consequence

**Under option (d) the coupling dissolves.** If citation verification is metered on its own counter, a fixed §15.2 draws from *that* counter rather than the shared `publishready` pool, and the reviewer's capacity is unaffected. **The interaction bites only under the current single-counter model.**

So this is an argument about **ORDER**, not about either fix:

| Sequence | Result |
|---|---|
| **metering first, then §15.2** | §15.2 carries no entitlement consequences |
| **§15.2 first, then metering** | the repaired call is expected to draw on the shared reviewer allowance in the interim |

**Neither fix is argued against.** Both are correct; only their order has a cost.

---

## 20.3 FINDING 3 — implementation observations

**A stale comment falsified by deployment.** `BillingPage.tsx:91-92` reads *"server-side metering isn't deployed"*. **It reflected reality when written**, the deployment changed, **nothing tied the comment to deployment state**, and **run 22 falsifies it**. Distinct in kind from the entitlement findings: this one is maintainability. A comment asserting the state of a *separately deployed system* has no mechanism that could keep it true, and no test can hold it — the same shape as §4.16, one component describing another's behaviour with nothing checking the description.

**The server-side derivation requirement**, recorded here as an implementation constraint independent of whether (d) is chosen: any per-feature metering must classify requests from trusted server-side signals, never from a caller-supplied identifier.

### Open questions

* **The deployed `PUBLISHREADY_LIMIT_PREMIUM`.** `20` is the code default (`config.py:58`); the deployed value is an environment variable this repository cannot read.
* **Typical calls per run.** Run 22 produced five successful `/verify` requests. `batch_escalation`'s count varies by manuscript, so **one run is one observation**, not a rate.
* **Whether any subscriber has been affected.** Not determinable from this repository.

---

## 21. Instrumentation maturity — an observed pattern

**Descriptive, not prescriptive.** This documents an evolution in how the project preserves engineering knowledge. It is not a rule, not a work item, and it does not prescribe future work.

### Three levels

They differ **not in permanence but in WHO MUST DETECT A VIOLATION.**

| Level | Examples | How a violation is detected |
|---|---|---|
| **1. Prose rules** | ONTOLOGY §4.4, §4.9, §4.10, §4.12, §4.14–§4.18 | **A contributor must have read and remembered the rule** |
| **2. Recorded instrumentation** | `manuscript_sha256`, `journal_name`, `findings_projection_digest`, `summary_digest` | **The system records the fact, but a human must compare or interpret it** |
| **3. Enforcing instrumentation** | the `SUMMARY_FORMAT_VERSION` pin, `release_gate.rs` | **The system detects the violation itself and blocks progress** |

### What recent work actually did

**Primarily moved knowledge from level 1 to level 2.** Two additions demonstrated their value immediately by exposing configuration problems that were previously invisible:

* `manuscript_sha256` — caught run 20 analysing the wrong manuscript (§17).
* `journal_name` — caught run 22's PLOS Medicine / PLOS ONE inconsistency (§19.2).

Both on their first run.

### Level 2's limit, shown by run 22

**Level 2 is a substantial improvement over prose because it makes hidden state observable, but it still depends on someone reviewing the output.**

Run 22 illustrates this directly. `journal_name` **faithfully recorded** the mismatch — the field did its job completely. But the inconsistency became actionable **only because someone compared the recorded fields.** Filed without review, the field would have **documented the confound rather than caught it** — which is precisely what happened to runs 20 and 21, where the same class of confound was present and no field existed to record it at all.

### Level 3 is a different class of protection

The `SUMMARY_FORMAT_VERSION` pin does not rely on a reviewer noticing a changed digest, nor on anyone remembering a documentation rule. **It fails at the point where the invariant is violated** — the edit that adds a `summary` field is the edit that turns the suite red.

> **CORRECTION (§4.4).** This section originally said the pin "lives in `gaply-core`, which CI runs (`windows-build-check.yml:117`)", and drew a level-3 axis of *automatic* versus *available*. **That line exists and that workflow does not run.** Both workflows in this repository are `workflow_dispatch` only (`windows-build-check.yml:19`, `package-release.yml:19`) — **there is no automatic CI**, nothing triggers on push, PR or schedule. **The automatic/available axis does not exist**, because no instrument here is triggered by a machine. The axis below replaces it.

#### The axis that the trace actually supports

**Whether the instrument intercepts the normal path of work** — not whether a machine or a human pulls the trigger.

| | Instruments | Why it holds |
|---|---|---|
| **Fires as a side effect of work done anyway** | the `SUMMARY_FORMAT_VERSION` pin; **`release_gate.rs`'s 13 unit tests, including all three proxy-free invariants** (PRIVACY, PROVENANCE, SELECTION) | Both are caught by an ordinary local `cargo test`. A contributor does not choose to run them; they run because the contributor was testing anyway |
| **Requires a deliberate separate act** | `examples/release_gate.rs` | Takes a manuscript path and runs only when someone chooses to |

**This places `release_gate`'s proxy-free invariants much closer to the pin than this section originally implied.** The library module and the runner are different instruments with different trigger conditions, and lumping them together as "`release_gate.rs`" obscured that. Its coverage limitation is separately recorded as §17's Gap B.

#### Why the app crate is untested in CI — a side effect, not a decision

The one workflow that runs tests runs **`cargo test -p gaply_core` alone**. The stated reason is specific (`windows-build-check.yml:107-114`): `cargo test --workspace` dies on Windows at `STATUS_ENTRYPOINT_NOT_FOUND` because **`tests/commands_test.rs`** links wry/tao.

**`release_gate` was swept up in a crate-granularity exclusion made for one integration-test target.** Its unit tests link no GUI stack. Nobody decided to exclude them.

**Open question, and it changes the shape of any fix:** whether manual-dispatch-only is deliberate. GitHub Actions minutes are metered on private repositories and Windows bills at 2×, so conserving them is a legitimate reason — **materially different from nobody having set it up.** Not answered here, and no workflow is proposed.

#### What the three proxy-free invariants would need

**Only a checked-in report JSON.** `build_review_payload` is pure, and PRIVACY, PROVENANCE and SELECTION read nothing but `payload["summary"]`:

```
report.json fixture → build_review_payload(…) → check_privacy / check_provenance / check_selection
```

**No manuscript, no pipeline, no DB, no proxy.** PRIVACY's end-to-end half needs the report's `detail` strings, which come from the same fixture.

**What such a run must assert: the three outcomes, not `ship_ready`.** `ship_ready()` requires zero fails **and** zero skips, so a proxy-free run reports *3 PASS, 3 SKIPPED, `ship_ready` false* — honest, and wrong to gate on. `ship_ready` answers *"may we ship?"*, which a proxy-free run cannot answer, **and a job that is always red gets suppressed or worked around.** `counts()` already returns all three numbers together, so no new machinery is needed, and the Skipped results stay recorded as typed absence per §4.12 rather than being read as failure.

### The observed progression

> **RULE → RECORDED EVIDENCE → ENFORCED INVARIANT**

**Instrumentation that repeatedly exposes violations of a stable invariant becomes a candidate for promotion to enforcement.**

**The criterion is the existence of an enforceable invariant, not frequency of usefulness.** A field can prove useful a hundred times and still admit no assertion — `summary_digest` is exactly that. Conversely `SUMMARY_FORMAT_VERSION` was promotable **on its first use**, because the invariant was defined before anything asserted it.

**This is an architectural observation, not a current work item, and it does not imply every recorded field should become an assertion.**

**`summary_digest` is the counter-example.** A changed digest means *"something differed"*, and **whether that is a problem depends on context** — the run 20 / run 21 pair wanted the digest to change under model non-determinism and wanted it stable under a repeat. There is no single invariant it could enforce, and **promoting it would mean inventing one to justify the promotion.**

By contrast, two candidates could each support an automated check **once the invariant is decided**:

* a known `manuscript_sha256` for a frozen baseline — the invariant already exists informally as the freeze condition;
* a journal/guidelines consistency policy **if** the project later adopts one (§19.4 records that no such policy exists today, and that the data to compute one does not either).

> ***The invariant comes first and the enforcement second.***

This states in one line what the progression above only illustrates: **the diagram explains the evolution; this sentence explains why a promotion is justified.** With the criterion stated earlier, it now says the same thing from both directions — an invariant without evidence is untested, and evidence without an invariant has nothing to assert.

The pin was promotable because `SUMMARY_FORMAT_VERSION` defined what "unchanged" means before anything asserted it.

---

## 22. Two verdicts over one body of evidence

**Investigation only.** Nothing implemented, and the residue is stated as an open question rather than a direction.

### 22.1 `overall_verdict` is an upstream resolved decision, not an aggregation input

An earlier draft of this pass classified `report["verdict"]` as "aggregation policy". **That is wrong.** The swarm has already decided by the time it exists; it is carried as a completed result, not as evidence the aggregator could weigh.

| | `report["verdict"]` | `Recommendation` |
|---|---|---|
| Vocabulary | `"pass"` / `"concern"` (`swarm.rs:314-315`) | `Accept` / `MinorRevision` / `MajorRevision` / `Reject` / `Unknown` |
| Granularity | binary | four ordered bands + a gate-only state |
| Resolves over | **six agent-level `Opinion`s**, one per lane | **N findings**, one per detected issue |
| Question | *does any agent have a concern?* | *what should an editor do?* |
| Mechanism | confidence-weighted vote, **hard-constraint override** (`swarm.rs:177-185`) | severity counting, **no weights, no precedence** (`reviewer_agent.rs:890-898`) |
| Produced at | `report.rs:563` | `reviewer_synthesis.rs:139` |

### 22.2 The granularity asymmetry — why "replace" fails structurally

**OBSERVATION.** The swarm resolves at the **agent** level: six opinions, one per lane. `from_validation` (`swarm.rs:344-355`) answers `concern` iff `!r.passed`. **A manuscript with fourteen statistical errors and one with a single error both yield exactly one `concern` from `ValidationMaths`.**

**`overall_verdict` is insensitive to how much is wrong.** The aggregator is nothing but that sensitivity — `MINOR_REVISION_THRESHOLD = 3` is a count, and quantity is its only substantive content.

**INTERPRETATION.** Replacing four ordered bands with a binary would discard the sole dimension the aggregator contributes. **"Replace" fails structurally, not by preference** — no weighting of the swarm's output recovers a quantity it never carried.

### 22.3 Run 22 already showed the divergence, and it went unremarked

**OBSERVATION.** Run 22's report carries `verdict: "pass"`. Its comparison record carries `shadow_recommendation: major_revision`. **Both in the same run, from the same evidence.** The cause is traceable: `AiDetection: concern` was a lone `Major` finding, enough for the severity tree, while the swarm's confidence-weighted vote did not carry it (AI-detection's opinion confidence is 0.6, rescaled by a 0.6 calibration factor — `swarm.rs:73`).

**The observation sat in files read closely enough to check five other fields.** §19 verified `manuscript_sha256`, `schema_version`, chunk count, `guidelines_url`, and both sent-counts against the freeze condition, and this divergence was in the same two artifacts, unnoticed. **Recorded because a missed observation in a reviewed artifact is the §21 level-2 limit demonstrated on the reviewer rather than on the instrument.**

### 22.4 The values were filtered, not dropped

**OBSERVATION, stated descriptively.** `reviewer_synthesis.rs:1-26` describes its join as taking "the compiled report's presentation fields (titles / checklist / journal / **verdict**)". **The values were not dropped accidentally; they were filtered because that module classifies `verdict` with presentation fields.**

**The classification was defensible when written.** The projection served narrative synthesis, where the swarm's verdict genuinely is presentation — a line in a letter. Nothing then required family identity or an upstream decision.

**What must be determined is whether that classification remains valid now that the aggregation contract has expanded.** If not, **the classification itself — not the projection — is the architectural decision requiring revision.** The projection is a faithful implementation of it.

### 22.5 Every path by which AI-detection reaches an editorial recommendation

**Traced before any exclusion is proposed. There are not two paths; there are five, and one of them invalidates the obvious exclusion key.**

| # | Path | Reaches | Severity |
|---|---|---|---|
| 1 | agent `Opinion` → weighted vote → `report.verdict` | `overall_verdict` | n/a (binary) |
| 2 | swarm opinion → finding (`report.rs:463-484`) | **deterministic recommendation** | **`Major` when `concern`** |
| 3 | **stylometry → `stylo_finding` (`report.rs:645-662`)** | **deterministic recommendation** | **`Minor`** — counts toward `MINOR_REVISION_THRESHOLD` |
| 4 | citation-density-unmeasurable (`report.rs:752`) | deterministic recommendation | `Info` — cannot change it |
| 5 | any of the above in the top-12 → `summary.findings` | **wholesale LLM recommendation** | n/a |

**NOT a path: escalation.** `evidence_record_escalation` (`evidence_store.rs:144-158`) writes `llm_verdict`, `llm_rationale`, `verified`, `gate_flags`, `provider`, `provider_model` — **never `severity`** — and the aggregator ignores `verified` by enforced invariant.

#### The finding this trace produced: `AgentKind::AiDetection` is overloaded

**Two semantically different producers share the label.**

* **Path 2 — the AI-authorship signal.** `ai_detect.rs:41`: *"STATISTICAL SIGNAL ONLY — NOT proof of AI authorship."* Categorically unfit (§21 Step 1).
* **Path 3 — document-level writing and citation hygiene.** `report.rs:665-673` states the scoping explicitly: *"The AI-AUTHORSHIP tells … are EXCLUDED: they answer 'was this written by a model', which is AI Check's job, not a reviewer's. What is kept answers 'is this well written and consistently cited', which is."*

**The code already declares path 3 reviewer-relevant, and it carries the same `AgentKind`.**

Run 22 contains both: `f1 major "AiDetection: concern"` (path 2) and `f2 minor "lexical diversity deviates from the academic reference"` (path 3).

**Consequence for any exclusion mechanism: excluding by `AgentKind::AiDetection` would also exclude findings the code explicitly documents as belonging in a review.** `AgentKind` remains the right *key* (§21), but it is **not sufficient as the eligibility predicate** for this family — the granularity of the unfitness is finer than the granularity of the key. This is exactly what drawing the complete graph before proposing an exclusion was for.

### 22.6 Chronology, and the absence of stated intent

| Commit | |
|---|---|
| `c83d74d` | swarm added — debate, hard-constraint override |
| `fa773f5` | report compiler wires `outcome.result.answer` into `report.verdict` |
| `015fbb7` | `reviewer_agent` added |
| **`d1c4e66`** | Box 4 Stage 1 — **`aggregate_reviewer_verdict` introduced** |

**The swarm resolution came first by a wide margin; the aggregator arrived later, in the Box 4 shadow work.**

**No producer, comment, test or document states their intended relationship.** `d1c4e66`'s message is unusually detailed about the aggregator's contract — severity mapping, zero weights, the `VerificationState` invariant, the invariance test — and **never mentions `report["verdict"]`**. ONTOLOGY does not address it. This document previously mentioned `overall_verdict` only as a payload field the digest does not cover.

### 22.7 Conclusion

**Two decisions over one body of evidence, built years apart for different consumers, with no relationship stated by any producer, comment, test or document.**

They are **not two views of one decision** — different units, different mechanisms, and they diverge in production (§22.3). They are **not one decision and one input to it** — `overall_verdict` is already resolved, and the AI signal already enters both paths, so consuming it would let the same evidence vote twice at two granularities.

**Neither "consume" nor "replace" is supported by the code.**

> **The investigation establishes that the hard-constraint precedence rule is the only element of `overall_verdict` not obviously reproduced by severity aggregation. Whether that rule should become an explicit aggregation input remains an open design question.**

**Not established:** whether the two verdicts have ever disagreed in a way that reached a user. Run 22 is the only report available, and its `shadow_reviewer` was not the authoritative letter, so that divergence was never shown to anyone.

---

## 23. Claim identity — ownership, and a new form of the projection pattern

### 23.1 The distinction is implemented procedurally and represented nowhere

**FACT.** The distinction between *"was this written by a model"* and *"is this well written"* is executed correctly and expressed by no value.

* `stylometry_findings` (`report.rs:677+`) reads `sentence_length_cv`, `mtld`/`mtld_deviation`, `ngram_repetition`, `citation_density`. It does **not** read `function_word_ratio`, `em_dash_per100`, `template_density` — exactly the three its doc names as authorship tells (`:665-673`).
* The distinction is carried by **which struct fields a function chooses not to read**, plus a doc comment.

**The same feature supports both claims.** `sentence_length_cv`'s definition reads *"Low = uniform = **AI**"* (`ai_features.rs:150`); `stylometry_findings` renders that identical field as *"uniform sentence length reads as monotonous to a reader"*. **The distinction is therefore not a property of the data and cannot be owned by the feature producer.** It is a property of the claim constructed from the feature, and it is made in two places: `from_ai_detection` (`swarm.rs:358-375`) and `stylometry_findings`.

> **The code executes it correctly, the type system cannot express it, and downstream consumers therefore cannot observe or verify it.**

**This is a NEW FORM of the projection pattern (§22.4).** Every prior case — `overall_verdict`, `confidence_kind`, `routing_hint`, `AgentKind`, `limitations` — was a value **produced, persisted, and dropped at a boundary**. This one **never became a value at all**. Separating *implementation* from *representation* is what distinguishes it: nothing was lost in transit, because nothing was ever put in transit.

**`BiasTier` is not the owner**, and assuming it would repeat the `ConfidenceKind` error. It classifies fairness risk and **cuts across** this distinction: `lexical diversity (MTLD)` is `BiasTier::Stylometric` (`ai_signals.rs:398`) and is precisely the finding the code declares reviewer-relevant.

### 23.2 `AgentKind` is NOT a fourth overloaded name

**Correcting this pass's own reasoning.** The three renames — `guidelinesUrl`, `wholesale_payload_digest`, `release_gate.rs` (module vs runner) — were each **one concept given a name claiming more than it delivered**. Renaming corrected the claim.

**`AgentKind::AiDetection` accurately denotes the producing subsystem, and both claims genuinely come from it**, from the same feature computation. **This is one producer legitimately emitting more than one claim type** — a different situation with a different remedy.

**The rule established when `ConfidenceKind` was rejected applies to `AgentKind` too:** do not key policy A on a field whose meaning is policy B. **`AgentKind` means "which subsystem produced this", which is not editorial admissibility.** It looked correct only because it is the finest-grained identity that exists — an accident of availability, not a property that makes it right.

**Which is why option (ii), splitting `AgentKind`, is rejected: it would make the producer taxonomy encode a policy**, the inverse of the three renames, which made names tell the truth. `AgentKind` survives as the key for per-lane execution state, where the question genuinely is *"did this subsystem run"*.

**Established: option (iii)** — `AgentKind` keeps denoting the producer; the claim identity is emitted by the producer alongside it, at the point the claim is constructed, where the distinction is already made and already documented.

### 23.3 The Box 4 interaction — exclusion would silently invalidate the comparison premise

**FACT.** `REVIEWER_INSTRUCTION` (`reviewer_agent.rs:77-91`) constrains grounding, format and the `reject` justification. **It says nothing about eligibility.** `build_review_payload` sends `report["findings"]` top-12 with no filter. No comment, test or document addresses whether an ineligible finding should be sent.

**Three distinct contracts, and no evidence for any:**

1. **every finding** — current behaviour, **not established as intended**;
2. **only verdict-eligible findings** — makes the two recommendations comparable, but withholds context a reviewer might legitimately use in prose;
3. **every finding together with its eligibility** — strictly more information; costs a payload field and a `SUMMARY_FORMAT_VERSION` bump.

**Under contract 1, the moment any exclusion lands the shadow and wholesale recommendations are computed over DIFFERENT EVIDENCE SETS.** §18.1's non-determinism finding compared them assuming identical input; exclusion would falsify that assumption **by construction, silently, and without changing any digest** — `findings_projection_digest` covers what was *sent*, not what was *counted*.

> **The wholesale contract must be decided before any exclusion ships.**

### 23.4 A LIVE DEFECT — the manuscript is penalised for Gaply's own failures

**This is not a design finding. It changed run 22's recommendation.**

Two of run 22's four `Minor` findings are statements about **Gaply's** execution, not the manuscript:

* **f4** — *"35 of 35 citation(s) could not be checked"* (`report.rs:394`)
* **f5** — *"Verification output rejected by its internal gate"* (`report.rs:519`)

**f4 exists because `/verify` returned 422 and then 403** — §15.2's unbounded-payload defect and §19.3's exhausted entitlement. The citation lane never ran.

> **Showing the author "we couldn't check your citations" is correct. Letting it change their recommendation is not.**

#### The measurement — observed

Run 22's findings: **1 Major, 4 Minor, 3 Info.** `MINOR_REVISION_THRESHOLD = 3`.

| Excluding | Minor count | Recommendation |
|---|---|---|
| nothing (actual) | 4 | `MajorRevision` |
| AI-authorship only (f1) | 4 | `MinorRevision` |
| process claims only (f4, f5) | 2 | `MajorRevision` |
| **both** | **2** | **`Accept`** |

#### What the measurement demonstrates — stated separately

**A recommendation change is produced by excluding findings that describe Gaply's own execution.** The conclusion follows from the four rows above; it does not replace them. With the AI-authorship signal also excluded, the same manuscript moves from `MinorRevision` to `Accept` — the two exclusions are independently insufficient and jointly decisive.

### 23.5 Eligibility is already being decided — via severity, inconsistently

**FACT.** `report.rs:~757` carries the comment:

> *"Info, not Minor: this is a statement about OUR parse, not the paper."*

**That is the eligibility decision, being made today, in the wrong field.** `Info` cannot change a recommendation, so choosing it *is* choosing ineligibility — expressed through an editorial-urgency field rather than an admissibility one.

**And it is applied inconsistently.** `:394` and `:519` are process claims by the same reasoning and carry **`Minor`**, which counts. `:380-381` states the identical principle in a second family — *"describes OUR infrastructure … does not belong in their report"* — while `:394`, twelve lines later, emits `Minor`.

> **The redesign makes an existing policy representable rather than introducing a new one.** Developers had already encoded it — in two comments, in one deliberate severity choice, and inconsistently in three others.

### 23.6 At least two axes exist

| Axis | Example | Reduces to the other? |
|---|---|---|
| **manuscript vs process** | f3 *"references older than 10 years"* vs f4 *"could not be checked"* | — |
| **authorship vs writing quality** | *"AiDetection: concern"* vs *"lexical diversity deviates"* | **No — both are manuscript claims** |

**A single `ClaimKind` enum cannot express both.** They are independent dimensions, not values of one.

**A third axis was not audited for.** The method that found these two — *asking what editorial question each constructor answers* — would find a third if one exists, and applying it exhaustively is the remaining work before any type is designed.

### 23.7 Category resolution

**The audit decided against the exceptional reading.** Claim identity is not one distinction that never became a value; **it is a dimension the `Finding` type lacks entirely.** `Finding` carries producer, severity, tier, confidence, title, detail and provenance, and nothing stating **what kind of assertion this is**.

**The remedy is therefore "the type is incomplete", not "represent this distinction."** The three-category scheme is **not** recorded: its stated condition — that AI-detection prove exceptional — was not met.

**No existing value carries claim identity.** `provenance` prefixes come closest and are partial (`signal:` in some families, `rule:`/`evidence:`/`gate:` in others) and were designed for grounding. The **constructing function** is 1:1 with claim type in every case — structural, not a value, which is §23.1's shape exactly.

---

## 24. The two reviewers answer different questions

### 24.1 The instructions say so explicitly

**DOCUMENTED CONTRACT — wholesale** (`REVIEWER_INSTRUCTION`, `reviewer_agent.rs:77-91`):

> *"You are a peer reviewer evaluating a manuscript from the STRUCTURED FINDINGS… Respond with ONLY a JSON object containing `recommendation`, `publication_probability`, `issues`, and `body`."*

**DOCUMENTED CONTRACT — shadow** (`REVIEWER_SYNTHESIS_INSTRUCTION`, `:741-746`):

> *"The findings below have **ALREADY been decided**… **Do NOT re-evaluate, re-score, re-classify, or overturn** any finding… **Do not output a recommendation or any score.**"*

**The shadow's model is forbidden to produce a recommendation.** The shadow recommendation is `aggregate_reviewer_verdict`'s severity count; the wholesale recommendation is an LLM judgement.

> **`recommendation_agreement` compares a rule engine's output against a language model's opinion.** They are not two reviewers — they are a deterministic aggregator and a peer-review simulation, asked different questions, answering in the same four-value vocabulary. **That shared vocabulary is the only thing making them look comparable.**

### 24.2 What the metric is and is not

**The measurement is right for the promotion question.** Box 4 exists to answer *"if we replace the LLM's recommendation with the aggregator's, what changes?"* — and comparing exactly those two values is the correct way to answer it, **regardless of whether the two were designed to answer the same question.**

**The NAME is wrong.** *"Agreement"* imports a normative claim the design never made: that concurrence between them is evidence of correctness. Nothing in the code or the commit history supports that.

**It cannot distinguish three causes of disagreement:**

1. the two answer **different questions** (§24.1);
2. **model noise** — the same input produced both values in runs 20 and 21 (§18.1);
3. **different evidence sets** (§24.3).

> **It is not evidence that the deterministic side is as good as the LLM.** It is evidence of what a user's headline recommendation would become.

### 24.3 The same-evidence assumption is UNEXAMINED, not unstated

| Layer | Status |
|---|---|
| **Payload construction** | **Two independent builders.** `build_review_payload` reads `report["findings"]` (severity-sorted); `build_reviewer_request` reads `input.findings` from `evidence_by_run` (`ORDER BY rowid`). **Neither references the other; nothing enforces that they select the same set** |
| **Reviewer instructions** | Silent on evidence identity |
| **Experiment design** | `d1c4e66` details the aggregator's contract at length and **never states the two paths must see identical evidence** |
| **Comparison metrics** | `recommendation_agreement` compares two recommendations with **no evidence-identity precondition** |

> **The assumption originates nowhere. It is UNEXAMINED rather than UNSTATED.**
>
> **An unstated decision has an owner who chose not to write it down; an unexamined one has no owner — so no layer can enforce or falsify it.**

By inspection the two sets probably coincide today, since evidence rows are inserted from `report["evidence"]`, built in lockstep with `report["findings"]`. **That is an emergent property of two independent code paths, asserted by nothing.**

### 24.4 FINDING — the cardinality guard repeats the count/membership error one layer up

`release_gate.rs`'s **COMPARISON** invariant checks `shadow_findings_sent == wholesale_findings_sent` — both 12 in run 22.

**§18.3 established that a count cannot prove membership.** That is exactly why `payload_digest` was renamed `findings_projection_digest`: a `SeverityByStateCounts` breakdown can be identical while the findings differ entirely.

> **The error the rename existed to correct now sits inside the instrument built to catch such things.** Two counts agreeing is not two sets matching.

**And membership is currently unverifiable on the shadow side: the shadow payload has no digest at all.** `summary_digest` and `findings_projection_digest` both cover the **wholesale** payload only (`commands.rs:688`). `build_reviewer_request`'s payload is hashed by nothing.

### 24.5 Every dependent metric

| Metric | Depends on same-evidence? | Status |
|---|---|---|
| **`recommendation_agreement`** | **Yes, totally** | §24.2 |
| **`shadow_issue_coverage`** | **Yes — it IS the denominator** | `issues.len() / findings_sent` (`:371-377`). Measures *how many of the findings we sent did the narrative discuss* — a **completeness check on one model**, filed among cross-model comparisons. Says nothing about the wholesale side |
| `shadow_grounded_issues` | No | Counts the shadow letter's issues |
| `wholesale_grounded_issues` | No | Counts the wholesale letter's issues |
| `shadow_hallucination_drops` | No | Gate warnings, shadow reply |
| `wholesale_hallucination_drops` | No | Gate warnings, wholesale reply |

**The two `grounded_issues` values are comparable only if the sets match.** Run 20's 3 vs run 21's 4 measures set difference as readily as model variance, and **neither field is labelled as carrying that precondition.**

### 24.6 A DOCUMENTED CONTRADICTION, nowhere acknowledged

`d1c4e66` states the goal is **replacement**: *"Stage 2 (switch) and Stage 3 (remove wholesale) are separate future decisions."*

**Replacement implies the two should answer the same question.** The two instructions state that they do not (§24.1).

> **That contradiction is acknowledged in no comment, test or document, and it must be resolved before promotion. It is a product decision.**

### 24.7 Process claims reach the wholesale reviewer too

Investigation A's f4 (*"35 of 35 citation(s) could not be checked"*) and f5 (*"Verification output rejected by its internal gate"*) are inside the top-12 sent to the LLM, under an instruction telling it to act as a peer reviewer and produce a recommendation.

**The model is being told about Gaply's infrastructure failures and asked to weigh them as manuscript evidence.** This is live now, not conditional on any exclusion shipping.

**Not established:** whether the two payload sets have ever diverged. Run 22 is the only artifact and it records cardinality, not membership.

---

## 25. What is Box 4 for?

**Three readings are live and mutually inconsistent:**

* **(a) REPLACE** — substitute a deterministic recommendation for the LLM's. `d1c4e66`'s stated Stage 2 / Stage 3 goal.
* **(b) CROSS-CHECK** — validate the LLM against an independent signal. What *"agreement"* connotes.
* **(c) NARRATIVE** — produce a review letter from already-decided findings. What `REVIEWER_SYNTHESIS_INSTRUCTION` actually asks for.

### 25.1 Which does each layer answer?

| Layer | Answer |
|---|---|
| **Documents** | **(a)**, unambiguously and twice. `d1c4e66`: *"Stage 2 (switch) and Stage 3 (remove wholesale) are separate future decisions."* `harness_log.rs:20-23`: *"the pre-promotion baseline… BEFORE the deterministic Box 4 verdict is promoted."* |
| **Code** | **(b)'s mechanism, (c)'s output discarded** — see §25.2 |
| **Metrics** | **predominantly (c).** Five of six measure narrative quality: `shadow_issue_coverage`, `shadow_grounded_issues`, `shadow_hallucination_drops`, `wholesale_grounded_issues`, `wholesale_hallucination_drops`. Only `recommendation_agreement` bears on (a), under a name that connotes (b) |

**Three different answers.**

### 25.2 The shadow letter is produced and discarded

**OBSERVATION.** `shadow_reviewer` is produced (`commands.rs:710-711`), serialized, crosses the IPC boundary — and is read by nothing. `adaptOutcome` reads `o.reviewer`, the **wholesale** letter (`publishReadyBridge.ts:72-101`), and the TypeScript `PublishReadyOutcome` interface (`:63-67`) **does not declare `shadow_reviewer` at all.**

**That is the §2 projection pattern at the output end of the very feature this investigation was studying.**

**INTERPRETATION, stated as consistency and not as proof:**

> **The current implementation is most consistent with (a). If Box 4 were primarily for (c), discarding the shadow letter would be difficult to explain. If it were primarily for (b), a comparison whose result reaches neither a gate nor a user would have no operational effect.**
>
> **The investigation could not establish whether the discard is deliberate or simply unwired.**

### 25.3 Why this became a product question

> **The same engineering work is prerequisite under one interpretation, counterproductive under another, and unnecessary under a third. That is why it became a product question — not because engineers disagreed, but because the same implementation has opposite values depending on what Box 4 is supposed to be.**

### 25.4 What each reading implies

| Question | (a) REPLACE | (b) CROSS-CHECK | (c) NARRATIVE |
|---|---|---|---|
| **Process claims in the aggregator?** | **Must be excluded** — §23.4 is fatal to a verdict that becomes the only verdict | **Both sides must match** — independence is the value, so identical inputs and independent methods; excluding from one alone destroys the check | n/a — no verdict |
| **Process claims in the wholesale payload?** | matters only during the transition | same as above | **They belong.** *"We couldn't check your citations"* is the honest caveat a letter should carry |
| **Identical evidence required?** | during comparison **yes**, or the measurement misinforms the decision it serves; irrelevant after the switch | **Required, and must be ENFORCED rather than assumed** (§24.3) | irrelevant — no comparison |
| **`recommendation_agreement` should be called** | what the user's recommendation **would become** — it measures a delta | *"agreement"* is right **only if** the two are independent estimates of one quantity, which §24.1 shows they are not | the metric should not exist |
| **Does the F2/F6 redesign serve it?** | **Prerequisite** | **Counterproductive** unless mirrored on the wholesale side, which nothing does | **Nearly irrelevant** — a narrative needs no verdict; the aggregator could be deleted and (c) still works |

**Under (c) the current behaviour is nearly right and only the severity is wrong. Under (a) it is a live defect. Under (b) the asymmetry is the defect.**

### 25.5 The L4 complication — two different points on the roadmap

**CURRENT PRODUCT PURPOSE** — the question being decided now: what is Box 4 for, today, in the shipping pipeline?

**TARGET ARCHITECTURE** — §11's direction: many **Review Engines** whose outputs are synthesised by an editorial layer above them (the "L4 editorial board"), with §11.3's capability registry as the non-`Finding` channel for capability state.

**These are not competitors. They sit at different points on the roadmap**, and the answer to the first does not settle the second. A future reader should not conclude that the L4 architecture contradicts today's decision, nor that today's answer was the permanent intent.

**One connection is worth recording, because §11.3 predicted §23.4 before it was observed:**

> *"Capability state currently has nowhere to live except a `Finding`… there is no non-`Finding` channel for 'this could not be evaluated'. A registry is that channel, and it removes the pressure to express capability state as a finding about the manuscript."* — §11.3

**f4 — *"35 of 35 citation(s) could not be checked"* — is exactly capability state expressed as a `Finding` about the manuscript.** The design intent recorded in §11.3 anticipated the defect §23.4 measured. That is evidence about where the eventual remedy belongs, and no evidence at all about which of (a), (b) or (c) is Box 4's purpose today.

### 25.6 A reading, offered as a reading

**The documents state (a), the code was built for (a), and the metrics accreted around (c) because narrative quality was what could be measured early.**

**If (a) is confirmed, F2/F6 is the right next work and §23.4's defect is urgent. If it is not, the next work is different.**

**This is a reading of the evidence, not a decision. The decision is not the code's to make.**

### 25.7 Not established

* **Whether the shadow letter's discard is deliberate.** The Rust returns `shadow_reviewer`; the TypeScript interface omits it. Consistent with *"not wired yet"* and with *"deliberately not shown"* — no comment distinguishes them.
* **Whether anyone intended the metric suite to be predominantly (c).** The metrics were added incrementally and each is individually reasonable.

### 25.8 Decision boundary

> **The investigations from §21–§25 have reduced the remaining uncertainty to product intent rather than implementation. No further tracing is expected to resolve it. Any subsequent engineering design — F2/F6, the aggregation contract, eligibility representation, the wholesale evidence contract, or Box 4 promotion metrics — should follow from the chosen product purpose rather than precede it.**

### 25.9 DECISIONS TAKEN — §25.8's boundary is resolved

**These are decisions, not inferences.** §25 established that the code, the documents and the metrics each answered differently, and that no further tracing would resolve it. A reading was chosen.

| # | Decision | Consequence |
|---|---|---|
| **1** | **Box 4 is for (a) REPLACE.** The deterministic verdict becomes the production recommendation **this generation**, with §11's L4 editorial board as the **stated later target**, where Box 4 becomes one reviewer inside a board | **F2/F6 is prerequisite, not optional.** §23.4's defect is urgent. §25.4's (a) column governs every downstream question |
| **2** | **Entitlement — correct the COPY, do not raise limits** | §20.1's contradiction resolves on the messaging side. `limit_free` stays 0; the "unlimited" claims are the thing that changes |
| **3** | **Metering — option (d), per-caller**, with the feature derived **SERVER-SIDE from endpoint or validated payload shape, never from a caller-declared field** | §20.2 resolved. The caveat is part of the decision, not a footnote: a client-declared feature is a client assertion, and trusting it reopens the hole `require_entitlement` exists to close |
| **4** | **Journal/guidelines — record the divergence, do not validate it** | §19.4 resolved. `journal_name` and `guidelines_url` are both recorded (§18.7); no consistency check is added, and none was proposable anyway since the data to compute one does not exist (§18.6.2) |

**Still open:** whether manual-dispatch-only CI is deliberate cost control (§21). That answer shapes any `release_gate` fix and nothing else.

#### What decision 1 settles, and what it does not

**Settles:** process claims must not influence the verdict (§25.4, (a) column); the two payloads must carry identical evidence *during the comparison period*; `recommendation_agreement` measures a **delta**, not a concurrence; and the F2/F6 aggregation redesign is the right next engineering work.

**Does not settle:** the L4 editorial board remains the target architecture, and §25.5's distinction stands — **this decision is about the current generation, not the permanent intent.** A future reader should not read (a) as ruling out the board; the board is where (a)'s deterministic verdict eventually becomes one voice among several.

### 25.10 Phase 1, item 1 — `escalation.rs:72`

`serde_json::from_value(report["evidence"]).unwrap_or_default()` turns a malformed evidence array into an empty vector; the early return at `:73-75` fires, the aggregator sees zero findings, and the verdict is **`Accept` at 0.92**. **A deserialization failure renders as a clean manuscript.**

#### On the escalation path, an empty evidence vector is not a valid successful outcome

**FACT.** All six agents always participate (`pipeline.rs:339-345`). Every opinion lands in exactly one of two lists and **both produce findings** — admitted soft opinions at `report.rs:497`, gate-rejected ones at `:513-519`. Only `ValidationMaths` is `hard_constraint`; only `Verification` and `Plagiarism` are skipped (`:470-472`). **Extraction, AiDetection and Rag therefore contribute a finding on every run.** If every opinion fails its gate, `run_debate` returns `Err` (`swarm.rs:264-266`) and no report is compiled at all.

`findings` and `evidence` are unzipped from one ordered source (`report.rs:559-560`), so they are empty together or not at all.

> **On this path, empty ⟹ malformed or absent. The fix need not separate those cases, because one of them cannot occur here.** The early return at `:73-75` reads as *"nothing to escalate"*; in practice it is *"deserialization failed"*.

#### The demonstrable trigger was schema drift on a cached report

`unwrap_or_default()` catches a missing key **and any element that fails to match `EvidenceRecord`** — a new required field, or a new variant of `AgentKind` / `ConfidenceKind` / `RoutingHint`. The outer parse is strict and fatal (`commands.rs:546`); **it is the inner, per-element parse that failed silently.**

**Cache entries survive rebuilds**, so an upgrade is exactly when this fires — and §23–§25's claim-identity work modifies `Finding`/`EvidenceRecord`, which is precisely the change that triggers it.

#### The options, and why the cache key decided between them

| | Behaviour | User sees | Needs `ExecutionState`? |
|---|---|---|---|
| **A. Propagate the error** | run returns `Err` | **an error instead of a report** | No |
| **B. Verdict withheld** | report renders, no recommendation, with a reason | findings and checklist, no verdict | **Yes** |
| **C. Degraded flag** | verdict present, flagged | a recommendation plus a caveat | Yes — and weakest; a flagged wrong number is still a number |

**The cache-key finding is not independent of the A/B choice — it determines it.** Versioning the key eliminates the only demonstrable trigger. With that gone, **A's urgency goes with it, and A's cost is real: a user whose manuscript was fine gets an error because OUR evidence did not parse — §23.4's shape, penalising the author for our failure.**

> **DECISION: cache key now, B as the first F2/F6 consumer.** One change instead of two, no interim regression, demonstrable path closed today. `unwrap_or_default()` stays untouched, and *"recommendation withheld because evidence could not be interpreted"* becomes the first concrete consumer of the execution-state model.

#### RELEASE CONSTRAINT

> **Any schema evolution that affects cached-report compatibility must require an `EVIDENCE_SCHEMA_VERSION` bump.**

**Compatibility, not modification.** An optional field with a serde default leaves cached reports readable and needs no bump. **Requiring one for every change would train reflexive bumping, which is how versions stop meaning anything.**

**This must be read where the claim-identity design is read, not only here.** That work modifies `Finding`/`EvidenceRecord`, and **shipping it before the cache is versioned would knowingly create an upgrade path where stale cached reports produce a silent `Accept` at 0.92.**

**Nothing enforces the bump.** `EVIDENCE_SCHEMA_VERSION` (`evidence.rs:20`) has no pin test; `evidence.rs:321` only asserts that a constructed record carries it. **That is §21's level-2 problem — a manual convention, not an invariant.** The `SUMMARY_FORMAT_VERSION` precedent applies directly: a test pinning `EvidenceRecord`'s serialized shape, failing with a message naming the constant and what to do. **Proposed, not built.**

#### Third instance of a contract whose premise moved

`escalation.rs:7`: *"DEGRADES HONESTLY on any failure (**never fails the run**)."*

**Correct when written** — escalation was an additive side-channel, and failing a run over a diagnostic would have been wrong. **`evidence_persist` has since become the sole populator of the aggregator's input**, so "degrade honestly" now means "silently produce `Accept` at 0.92".

**Same shape as `assemble_reviewer_input`'s projection (§22.4) and `reviewer_synthesis`'s presentation-field classification.** All three were right for the contract they implemented, and all three had their premise moved underneath them. **Revising this one is part of B's work and should be explicit rather than incidental.**

**Not established:** whether this has ever fired. `unwrap_or_default()` logs nothing, so there is no evidence either way.

---

## 26. F2/F6 — aggregation contract design

**Design only. Nothing implemented.** Every element names the traced instance that demanded it and what breaks without it. Elements no instance required are listed in §26.8 and excluded — this project has deferred four abstractions (the `KnowledgeExtractor` trait, the capability registry, the Review Engine scheduler, per-engine budgets) precisely because they were designed from zero instances.

### 26.1 The three concepts

#### PRODUCER — `AgentKind`

| | |
|---|---|
| **Status** | Exists. Flattened to `String` by `enum_text` (`evidence_store.rs:78-80,115`), never restored (`reviewer_synthesis.rs:86`) |
| **Owner** | the lane that constructed the finding |
| **Readers** | `confidence_kind` / `routing_hint` / `limitations` (`evidence.rs:118,131,144`); escalation batching (`escalation.rs:120-137`); per-lane execution state |
| **Crosses** | `Finding` → `EvidenceRecord` → DB → `ReviewerFinding` |
| **Demanded by** | §21 Step 1 — per-lane execution state must be keyed by lane |
| **Breaks without** | nothing can be keyed by family. `AgentKind` already derives `Deserialize`, so this is **restoration, not introduction** |

**Not the eligibility key** (§23.2) — `AgentKind` means *"which subsystem produced this"*, which is not editorial admissibility.

#### CLAIM — what editorial statement this is

| | |
|---|---|
| **Status** | **does not exist** (§23.7) |
| **Owner** | the **claim-construction site**, not the feature producer — §23.1, since `sentence_length_cv` supports both an authorship claim and a writing-quality claim |
| **Readers** | the eligibility resolver; presentation |
| **Demanded by** | §23.4 (measured) and §22.5 (excluding by `AgentKind` would exclude reviewer-relevant stylometry) |
| **Breaks without** | the measured defect stays, or is fixed by a key that also excludes findings `report.rs:665-673` documents as belonging in a review |

#### DISPOSITION — how the statement participates

**Not a field on `Finding`.** §23's ownership finding: *"excluded from the verdict"* is a property of the **aggregation** — no producer can state it and no per-agent constant can carry it.

| | |
|---|---|
| **Owner** | **`VerdictAggregation`**, beside `breakdown` (`reviewer_agent.rs:851-853`) |
| **Computed** | by the resolver, from CLAIM + policy, at aggregation time |
| **Crosses** | aggregator → report → UI. **Never persisted on a finding** |
| **Shape** | **eligibility AND its reason** — §4.14: *"did not affect the recommendation"* ≠ *"unimportant"*. The `Metric::Unavailable { requires }` shape |
| **Breaks without** | a user sees `AiDetection: concern` beside `Accept` with no explanation |

### 26.2 `ClaimKind` — a default and two carve-outs

**These are NOT three dimensions.** All three answer one question — *what is this claim about*: our execution, model authorship, or a manuscript flaw. **`ConfidenceKind` is what describes epistemic character; `AuthorshipSignal` describes what the claim ASSERTS.**

**What is genuinely uneven is the GRAIN.** `ProcessState` and `AuthorshipSignal` are each one narrow thing; `ManuscriptDefect` covers statistics, citations, overlap, stylometry, tables and recency. **It is a default plus two carve-outs, and the carve-outs are exactly the two traced instances.**

| Variant | Traced instance |
|---|---|
| `ProcessState` | §23.4 — f4 *"could not be checked"*, f5 *"gate rejected"*, lane opinions. **Measured behaviour change** |
| `AuthorshipSignal` | §22.5 / §21 — `ai_detect.rs:41` disclaims proof; `report.rs:497` maps it to `Major` |
| `ManuscriptDefect` | the default — everything else |

**The comment above the enum must say this**, because *"three dimensions compressed into one"* invites someone to fix a problem that does not exist, while *"a default and two carve-outs, each demanded by a traced instance"* invites them to leave it alone:

> **This enum is intentionally instance-driven rather than taxonomically complete. Future variants should only be introduced when demanded by traced evidence.**

#### Third-axis risk, costed

| Extension | Class |
|---|---|
| a fourth variant | **COMPATIBLE** — additive |
| a second optional field with `serde(default)` | **COMPATIBLE** |
| **changing a plain variant to a DATA-CARRYING one** | **INCOMPATIBLE** — the wire form goes from `"process_state"` (string) to `{"process_state": {...}}` (object). Every historical fixture fails; a bump is required |

**This is a departure from §21's conclusion**, which observed that an enum whose variants can carry data is the extensible shape. **The bet taken here is that the carve-outs stay parameterless.** It is safe for the two traced instances — *"this is a process claim"* and *"this is an authorship signal"* carry no payload — and it is **unsafe if a future carve-out needs a parameter**, at which point the correct move is a **new parameterless variant or a second field**, both compatible, rather than parameterising an existing one.

### 26.3 The aggregation input

**Enters:**

| Element | Demanded by | Breaks without |
|---|---|---|
| `severity` | existing | the recommendation |
| `claim` | §23.4 | the measured defect |
| `producer` (restored) | §21 Step 1 | execution state cannot be keyed |
| **execution state, two keys** | §21 Step 1's asymmetry | §16 F6 — empty findings still mean `Accept` at 0.92 |

**Two keys, because delivery is stage-scoped and examination is family-scoped:**

* **per-stage delivery** — did this stage produce its output? (`lane` aborts on `Err` (`pipeline.rs:134-137`), so failure is not an aggregator state; **absent delivery is**)
* **per-lane examination** — `NotRun | RanOverEmptyInput | Ran { examined }`. This is §22.1's **missing denominator**: zero references → no citation findings, today indistinguishable from "checked, clean"

**Does not enter:** `routing_hint` (workflow policy — escalation eligibility ≠ verdict eligibility), `confidence_kind` (evidence metadata; rejected as the key in §21), `limitations` (presentation metadata with **no consumer at all**), `overall_verdict` **as a value** (already resolved; the AI signal enters both paths, so consuming it double-counts at two granularities).

**Deliberately unresolved:** the hard-constraint precedence rule (§22.7). **No traced instance has required it** — no run has been observed where precedence would have changed the verdict.

### 26.4 Migration and serialization — predicted

| Change | Class | Bump? |
|---|---|---|
| `ClaimKind` type | additive | no |
| **`claim: ClaimKind` required on `Finding`/`EvidenceRecord`** | **INCOMPATIBLE — required field** | **YES** |
| DB column | schema migration | — |
| execution state on the report | new field | same bump |
| later variant / later optional field | **COMPATIBLE** | no |

#### Why required, not optional-with-default — and why that is only safe now

**An `Option<ClaimKind>` with `serde(default)` would be compatible and wrong.** A stale cached report has no `claim`, so every finding takes the default; if that default is `ManuscriptDefect`, **f4 and f5 in cached reports count toward the verdict — §23.4's defect reintroduced for cached data and invisible.** Blind spot 1's shape at the worst possible field.

> **A required field is correct, and `8af96a7` is what makes it safe:** `EVIDENCE_SCHEMA_VERSION` is in the cache key, so a bump makes stale entries **miss and recompute** rather than mis-parse. **The Phase 1 work is the precondition for the correct Phase 2 choice.** The compatibility pin (`4d57165`) will demand the bump at build time; this design predicts it.

#### THE MIGRATION INVARIANT

> **No cached report written by an older binary may silently change editorial meaning after upgrade.**

**Every migration decision is judged against it. NOTHING ENFORCES IT.**

It is precisely the compatibility pin's **semantic blind spot**: **F1's `Critical → Major` violated this invariant, parsed cleanly, and nothing fired.** The pin detects *parse* incompatibility only. **This invariant is maintained by discipline, not by instrument**, and the pin's existence must not make it look covered.

### 26.5 PR-1's specific enforcement risk — `..Default::default()`

A required field is compile-enforced at struct **literals**. **The gap is struct-update syntax:** `Finding { severity, ..Default::default() }` compiles and fills `claim` **silently with a wrong default** rather than failing. Same shape as blind spot 1 — a default silently standing in for a real value.

**AUDITED, and the gap is currently absent:** neither `Finding` nor `EvidenceRecord` derives `Default` (`report.rs:143`, `evidence.rs:100`), and a scan of every `Finding {` / `EvidenceRecord {` literal across `gaply-core/src` and `src` found **zero** struct-update occurrences.

> **PR-1's requirement: construction must FAIL TO COMPILE, not silently default. Do not derive `Default` on either type, and re-run the scan before merge.**

### 26.6 PR sequence — derived

**Four, from what must land together versus what can land apart.**

**PR-1 — identity, end to end.** `ClaimKind`; every constructor emits it (total match, compile-forced); carried `Finding` → `EvidenceRecord` → DB → **restored in `assemble_reviewer_input` together with `AgentKind`**; `EVIDENCE_SCHEMA_VERSION` bumped. **Must land together** — a field written and never read is the pattern this investigation condemns. **The verdict must not change**, pinned by a test on a fixture report, so a regression in *data* and one in *decision* stay attributable.

> **PR-1 is NOT fully behaviour-neutral.** Bumping `EVIDENCE_SCHEMA_VERSION` **invalidates every cached report**: the next run on a previously-analysed manuscript recomputes instead of serving from cache. Benign — recomputation is always correct and merely slower — but user-visible, and not to be overclaimed as "no behaviour change".

**PR-2 — evidence interpretability and the withheld verdict.** The coupling, drawn deliberately: *"recommendation withheld because evidence could not be interpreted"* needs a **run-level** state, a different widening from PR-1's per-finding one. Folding it into PR-1 would bundle a behaviour change into a behaviour-neutral PR; deferring it to PR-4 leaves `unwrap_or_default()` silently producing `Accept` at 0.92 across two PRs. **Chosen: its own PR, with the run-level state type introduced WHOLE and the states PR-4 will fill declared typed-absent (`Unavailable { requires }`).** That avoids designing a narrow channel and widening it — §4.12 applied to the design itself, and it satisfies the rule that the public contract must not widen every PR. Includes `escalation.rs:72` and the contract revision (§26.7).

**PR-3 — the eligibility resolver. Behaviour changes here.** Total `ClaimKind → eligibility`, no wildcard; the aggregator counts only eligible findings; `VerdictAggregation` gains DISPOSITION with reasons; the UI renders why a finding did not count. **Blast radius measured before merge** (§16 F1's treatment).

> **PR-3 IS BLOCKED ON A PRODUCT DECISION — §23.3's wholesale contract.** The sequence is not fully unblocked.

**PR-4 — execution state, both keys.** Threaded from lane exit; fills the states PR-2 declared absent; makes `Recommendation::Unknown` reachable, closing §16 F6.

**Why not fewer:** PR-1 and PR-3 must be separable or a verdict change cannot be attributed to the resolver rather than the data. **Why not more:** PR-1's four sites have no useful intermediate state.

#### `MAX_FINDINGS` — exclusion does not free the slots

**PR-3 excludes `ProcessState` from the verdict. The payload cap selects by the report's severity ordering, not by eligibility** (`reviewer_agent.rs:382`, `:945`). So **f4 and f5 keep 2 of the 12 slots and keep reaching the wholesale reviewer**, which §24.7 already records as live: the model is told about Gaply's infrastructure failures and asked to weigh them as manuscript evidence.

**PR-3 does not change the cap.** Doing so is the wholesale contract decision (§23.3) — filtering the payload is contract 2, sending everything with eligibility attached is contract 3, and **choosing is not PR-3's to make.** Recorded so the exclusion is not mistaken for having removed these findings from the LLM's view.

### 26.7 `escalation.rs:7`'s contract — the revision

**Today:** *"DEGRADES HONESTLY on any failure (never fails the run)."* **Correct when written** — escalation was an additive side-channel. `evidence_persist` has since become the aggregator's sole input, so "degrade honestly" now means "silently produce `Accept` at 0.92". Third instance of a contract whose premise moved (§25.10).

**Becomes:**

> **Escalation still never fails the run. What changed is that `evidence_persist` is the aggregator's sole input, so an empty or uninterpretable evidence set is a TYPED ABSENCE that must reach the verdict — not a silent zero. Honest degradation now means the run completes with the verdict WITHHELD and the reason stated, never with a verdict computed from evidence that could not be interpreted.**

Lands in PR-2, explicitly.

### 26.8 Excluded — no instance required it

| Excluded | Why |
|---|---|
| a general claim taxonomy | §26.2 — two instances underdetermine it |
| a second orthogonal axis | no instance requires it; both extensions stay compatible |
| hard-constraint precedence at the aggregator | §22.7 open; no instance has required it |
| `deny_unknown_fields` | runtime behaviour change, deliberately unbundled |
| a wholesale-payload eligibility filter | §23.3 — three contracts, none evidenced |

### 26.9 Open before PR-3

1. **The wholesale evidence contract** (§23.3). Under decision (a), leaving the payload unfiltered makes the two sides compute over **different evidence sets**, silently falsifying §18.1's premise **without changing any digest**.
2. **The shadow payload has no digest** (§24.4) — membership is unverifiable on that side, and PR-3 is exactly when the two sets could diverge.

### 26.10 The wholesale evidence contract — DECIDED: contract 1, unfiltered

**`build_review_payload` stays unfiltered. PR-3 is unblocked.**

**The reasoning.** Under decision (a), promotion replaces the wholesale recommendation with the deterministic one. During the migration period there are genuinely **two production behaviours**: the wholesale reviewer is **today's**, the deterministic aggregator is the **candidate**. Filtering the wholesale payload to match would compare the candidate against a **modified incumbent that users never experienced**, which weakens the promotion evidence.

> **The exclusion is not a confound to control for. It is the treatment.**

**Contract 3 rejected** — telling the LLM what counts changes its behaviour, so the before-state stops being the actual before-state.

#### THE COST — recorded prominently, not as a footnote

**After PR-3 the two sides compute over different evidence sets.** A recommendation delta therefore carries the **combined** effect of *different evidence* **and** *different decision mechanisms*, with no way to separate them from the record.

> **`recommendation_agreement` remains valid as a PROMOTION-IMPACT metric. It is no longer interpretable as a MODEL-QUALITY metric.**

Consistent with §24.2, which already established the measurement is right for the promotion question and the *name* imports a normative claim the design never made. **This decision removes the last reading under which the name could have been recovered.**

**The payload digest will not detect the divergence.** `findings_projection_digest` covers `summary.findings` as sent (§18.3), and **the exclusion happens after** — inside the aggregator, on a set the digest never sees. Two runs can therefore share a digest while their verdicts were computed over different evidence, and nothing in the record will say so.

### 26.11 PR-2 serialization audit — what a withheld run actually emits

**Bytes, not accessor paths.** The boundary test asserts what consumers *observe*; serialization is a different question, because `derive(Serialize)` emits every field regardless of whether an accessor was called.

| Surface | Withheld run emits | **Mechanism that makes it clean** |
|---|---|---|
| Report cache `report:v2:e2:{id}` | no probability | **The field does not exist on `PublishReadyReport`.** The aggregation is computed after the report is cached and is never written into it |
| `box4_comparisons.jsonl` | `{"requires":"verdict_withheld","source":"deterministic_local","status":"unavailable"}` | **`Metric` is a tagged enum** — the `Unavailable` variant has **no `value` field to emit** |
| Exported PDF | no probability | **The letter never reaches it** — `downloadReportPdf` takes `PublishReadyReport` only |
| Wholesale payload | no probability | `build_review_payload` reads `report[…]` only |
| **IPC → frontend (`shadow_reviewer`)** | **was `"publication_probability":0.0`** | **DEFECT — fixed in this commit** |

#### The fifth surface's guard was RUNTIME, not structural

**Nothing rendered the `0.0` only because `ReviewerLetterPanel` branches on `available` first.** That is a consumer choosing correctly, not a property of the data.

**Under decision (a) the shadow letter BECOMES the production letter**, so that branch was the only barrier between a withheld run and a rendered `0%` — a §4.4 defect of the same class as the offline copy: a number present in an artifact for a run where nothing computed one.

**Fixed structurally:** `ReviewerEvaluation.publication_probability` is `Option<f64>` with `skip_serializing_if = "Option::is_none"`, so **the key is omitted from the wire entirely** and there is no sentinel to mistake for a score. Verified by dumping the bytes:

```json
{"recommendation":"unknown","novelty_assessment":"", … ,"available":false}
```

**The pre-existing offline path emitted the same `0.0` and is fixed by the same change.** Frontend: `publicationProbability: number | null`, and the gauge renders nothing rather than `0%`.

#### A test-gap finding: §4.17 in a new form

**The first withheld test entered DOWNSTREAM of the defect.** It started from the withheld *signal* rather than from malformed evidence, so restoring `unwrap_or_default()` did not fire it — the mutation is what exposed this.

> **The right property, asserted at the wrong boundary. A test entered downstream of a defect cannot detect that defect**, and *"mutation-verified"* would otherwise have read as sufficient when it was not.

`malformed_or_absent_evidence_withholds_the_verdict` closes it at the source.

#### Why `Unknown`'s third meaning is acceptable

The letter now uses `Unknown` for offline, ungrounded-reject, **and** withheld-filler. **The decisive harm does not transfer:** the 0.05 came from the *aggregator* mapping `Unknown → PROB_REJECT`, and the letter sets its probability explicitly — now `None`, so nothing is emitted at all.

**The out-of-band field distinguishing offline from withheld is `warnings`**, carrying `"verdict withheld: <slug>"`. Both otherwise produce `Unknown` + `available: false`, and the UI branches on exactly that.

### 26.12 SEQUENCING — the baseline capture belongs AFTER PR-3

`harness_log.rs:20-23` states the sink exists so the pre-promotion baseline is captured **before the deterministic Box 4 verdict is promoted**, and promotion follows F2/F6 (decision 1).

> **A baseline taken now would measure a deterministic verdict that PR-3 is about to change — one that never ships.**

Recorded because the baseline has been described as ready-to-run for several turns, and running it prematurely would **spend entitlement on a measurement of the wrong thing** (§19.3: the allowance is ~4 attempts at current consumption, and §20.2's metering change is not built).

**The correct order: PR-3 → PR-4 → baseline capture → promotion.**

### 26.13 PR-4 — execution state, and three corrections to §26

#### The justification is a measured AMBIGUITY, not a measured defect

**Every prior PR in this arc had an observed defect behind it.** PR-1 restored a value traced to a specific loss; PR-2 closed §25.10's silent `Accept` at 0.92; PR-3 reversed a recommendation §23.4 had measured.

> **PR-4 has a demonstrated INABILITY TO TELL TWO STATES APART.** The system cannot distinguish *genuinely clean* from *nothing checked* even in principle, and emits `Accept` at 0.92 for both.

**Weaker evidence, still sufficient — and the record should say which.** No available run would be withheld by PR-4: run 22 had 35 references, extracted statistics and 7 tables, so multiple denominator lanes examined real input. **§22.1's dangerous case is UNOBSERVED, not absent.**

#### The criterion, so a seventh lane's answer is derivable

**(a) Can the lane produce ELIGIBLE claims at all?** If no, it is **not in the denominator** — its silence says nothing about the manuscript. `Rag` is excluded on this ground.
**(b) If yes, was the INPUT its eligible-claim production requires present and non-empty?** If not, it examined nothing.

**The subject is the input to eligible production, not the lane's output.** `Verification` with zero references is the clarifying case: it emits *"0 of 0 citations could not be checked"* — output, but no evidence.

| Lane | In denominator | Examined nothing when |
|---|---|---|
| Verification | yes | zero references |
| ValidationMaths | yes | zero statistical claims |
| Plagiarism | yes | no corpus **and** < 2 chunks |
| AiDetection | yes | text below the stylometry gates |
| Extraction | yes | no tables **and** no references |
| **Rag** | **no** | produces `ProcessState` only |

**`NothingExamined` fires only when EVERY lane in the denominator examined nothing.** A single starved lane is the partial case.

#### The lane state's job

> **Zero eligible findings has two causes, and only the lane state separates them: genuinely clean versus nothing checked. It is the DISAMBIGUATOR that makes `Accept` honest when emitted, not an additional signal.** It is not consulted when findings exist.

#### The partial case

Citations starved, statistics examined the whole manuscript and found nothing. **`Accept` is honest as "clean as far as we looked" and dishonest as "clean".** It does **not** withhold — that manuscript has substantial evidence, and withholding would be a worse outcome than a caveated recommendation.

**The caveat travels PR-3's channel**, because *"this lane examined nothing"* is the same shape of statement as *"this finding did not count"*: `not_examined: Vec<LaneNotExamined>` beside `excluded`, surfaced through the letter, rendered in its own card. **Visible but not decisive, deliberately.**

#### Prerequisite built here: plagiarism's corpus datum

`PlagiarismReport.corpus_chunks_available` is new. **Without it the lane's criterion collapses to "zero matches"** — the exact clean-versus-unchecked ambiguity PR-4 exists to resolve — and **a lane whose criterion is unsound must not enter the denominator.** The count applies the same `EXCLUDED_CORPUS_SOURCE_TYPES` exclusions the comparison applies, so it answers the question the criterion asks.

#### CORRECTION — `ReviewerInput` does widen

**§26's claim was narrower than it read.** Introducing the withheld-reason vocabulary whole stopped **that enum** widening; **it could not pre-provision a channel for data PR-2 did not have.** `ReviewerInput` gains `lanes`, and `VerdictAggregation` gains `not_examined`. Recorded as the correction rather than the claim.

#### The SIXTH projection instance, and its distinguishing feature

Per-lane examination is known at lane exit (`pipeline.rs:214-320`), where the input is in scope, and **lost immediately** — `lane()` returns only the lane's value, and the summary string reaches a progress event and is discarded.

**Unlike the first five, the value was never CONSTRUCTED at all.** That makes it **§23.1's shape — implemented procedurally, represented nowhere — rather than §22.4's**, where a value was produced, persisted and dropped at a boundary.

#### DEFERRED with a named destination: `StageUndelivered`

`lane()` propagates `Err` and the caller uses `?`, so a lane failure aborts the run and **no path produces `StageUndelivered`**. One stage does degrade silently: `build_checklist` (`pipeline.rs:355`) is `unwrap_or_else(|e| vec![])`, so **a DB or embedder error yields an empty checklist indistinguishable from "no guidelines supplied"** — §18.6.2's shape, one layer up.

**TRACKED ITEM: fix `build_checklist`'s silent empty, and populate `StageUndelivered` there.**

Deferred from PR-4 because bundling would make its blast radius two things at once, and this is a different defect from F6's. **But a variant that ships unconstructed is the state `Recommendation::Unknown` was in, and that was a finding (§16 F6).** So it carries a destination, not an open-ended wait.
