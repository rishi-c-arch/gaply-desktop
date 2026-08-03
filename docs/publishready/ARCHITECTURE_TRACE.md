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
