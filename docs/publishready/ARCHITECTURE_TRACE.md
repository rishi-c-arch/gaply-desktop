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
