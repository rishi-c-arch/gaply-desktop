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

> **REFRAMED — see §15.2.1. The defect is a CONTRACT gap, not a size problem, and one reading of the proxy's message below was later made and is wrong.**

**FACT.** `POST /verify` returned **422**: *"total text content is 16255 chars (limit 8000)"*.

`validate_structured` (`gaply-proxy/app/validation.py:39-61`) walks **every string leaf anywhere in the payload** and sums their lengths, so `MAX_TOTAL_CHARS = 8000` is a **global budget across the whole request** — not per-field, not per-citation. The separate `max_field` prose check did **not** fire.

`verify_citations` has **no cap** — no `.take(N)` anywhere in the assembly. **Payload size grows approximately linearly with reference count and is unbounded; this 28-reference manuscript already exceeds the limit by roughly a factor of two.** One data point supports that and nothing broader.

**Classification: payload-size bug.** Not a privacy-boundary violation — `Reference.raw` is deliberately excluded (`verify_agent.rs:147-149`), abstracts are excluded by design (`:210-211`), all fetched text passes `llm_safe()`, and the `max_field` prose check did not fire. The proxy's phrase *"send a structured summary, not raw manuscript text"* is **its own validation vocabulary, not a finding about what was sent** — reading it as evidence of a breach is the §4.14 error.

**This corrects a recorded diagnosis.** All-UNKNOWN citation verdicts were attributed to Ollama being unavailable. This run **reached the cloud proxy and was rejected on size** — a different cause with a different fix.

**Note for §14:** `release_gate.rs`'s PRIVACY invariant covers `build_review_payload` only. **`verify_citations` has never been asserted** — no size check, no privacy check. That coverage gap is real independent of this defect's class.

### 15.2.1 REFRAME — a bounded endpoint, an unbounded client

**The 422 recurred at 16,658 characters on run 24.**

> **`verify_citations` builds ONE EVIDENCE BUNDLE PER REFERENCE with no cap, growing linearly with reference count, while `build_review_payload` caps at `MAX_FINDINGS` and clamps every field. The endpoint expects a bounded structured summary; the client sends an unbounded one.**

**That makes the fix architectural, not "truncate at 8000".** A truncation would satisfy the validator while leaving the asymmetry — one payload builder disciplined, its sibling not — and the next schema change would reopen it at a different size.

#### AND A RECORDED ERROR: §4.14, made against a warning already written here

This section's own text says the proxy's phrase *"send a structured summary, not raw manuscript text"* is **its own validation vocabulary, not a finding about what was sent**, and that reading it as evidence of a breach is the §4.14 error.

**That error was then made anyway**, in an analysis of run 24, which concluded the client was transmitting raw manuscript text.

**It is not supported, and it was already ruled out:**

* `Reference.raw` is deliberately excluded (`verify_agent.rs:147-149`);
* abstracts are excluded by design (`:210-211`);
* the proxy's **separate `max_field` prose check — which would fire on a manuscript paragraph — did not fire**;
* the 16,658 characters are **provenance URLs and per-entry sha256 checksums**, one per evidence entry, none of which the model reads.

**Recorded because the warning was written down in this very section and the error was made regardless.** A rule at §21's level 1 does not stop the person who wrote it. The conclusion — that the fix is architectural — survives on the contract gap alone and needs no claim about manuscript text.

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

**What such a run must assert: the three outcomes, not `ship_ready`.** `ship_ready()` requires zero fails **and** zero skips, so a proxy-free run reports *3 PASS, 3 SKIPPED, `ship_ready` false* — honest, and wrong to gate on.

> **CORRECTED BY §32, on the gate's first actual execution.** Two errors here, and the conclusion above survives both while its reasoning does not. **(a) The numbers were predicted, not observed:** the real figure is **4 PASS, 2 SKIPPED** — LIVENESS *passes* rather than skipping, because it is a meta-invariant over the report and needs no proxy. **(b) The attribution is wrong (§4.14):** COMPARISON and PERSISTENCE do not skip because the proxy is absent. They skip because **the runner passes literal `None`** and drives `run_pipeline_measured` rather than `run_publishready`, so no comparison record can exist **on any run, with or without a proxy.** *"A proxy-free run cannot answer `ship_ready`"* is therefore too weak: **this runner cannot answer it, ever.** `ship_ready` answers *"may we ship?"*, which a proxy-free run cannot answer, **and a job that is always red gets suppressed or worked around.** `counts()` already returns all three numbers together, so no new machinery is needed, and the Skipped results stay recorded as typed absence per §4.12 rather than being read as failure.

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

> ## ⚠ ATTRIBUTION CORRECTED — see §23.4.1
>
> **Every "run 22" in this section is wrong. The measurement is runs 20/21, the
> Thermosensitive Nanoemulsion paper.** The defect, the exclusion table and the
> conclusion all stand; only the run they are attributed to was wrong.

**This is not a design finding. It changed the recommendation.**

Two of the run's four `Minor` findings are statements about **Gaply's** execution, not the manuscript:

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

### 23.4.1 CORRECTION — the measurement is runs 20/21, not run 22

**WRONG.** The finding list this section uses — *"AiDetection: concern"*, *"lexical diversity deviates from the academic reference"*, *"35 of 35 citation(s) could not be checked"* — is **`report:v2:21`**: the **Thermosensitive Nanoemulsion** paper, **8 findings** (1 Major, 4 Minor, 3 Info).

**Run 22's report is `report:v2:22`: 19 findings, 12 Major, and none of those titles.** Its Majors are five deterministic statistical-rule failures and six internal-duplication spans.

| Run | Manuscript | Report |
|---|---|---|
| 20, 21 | `78c0ae…` Thermosensitive Nanoemulsion | 8 findings — 1 Major, 4 Minor, 3 Info |
| **22** | **`859880…` Bombyx (frozen)** | **19 findings — 12 Major, 4 Minor, 3 Info** |
| 23 | `859880…` Bombyx (frozen) | 20 findings — 12 Major, 4 Minor, 4 Info |

**So §23.4's four-row exclusion table and PR-3's predicted `MajorRevision → Accept` both describe runs 20/21 while attributed to run 22.**

#### WHAT SURVIVES — the conclusion, not the provenance

**The defect is real.** Process claims **did** change a recommendation on the Thermosensitive paper: f4 *"35 of 35 citation(s) could not be checked"* and f5 *"Verification output rejected by its internal gate"* were two of four Minors, and excluding them plus the authorship signal moved that run from `MinorRevision` to `Accept`.

> **This corrects the evidence's PROVENANCE, not the conclusion. PR-3's fix still stands, and run 23 confirms the resolver behaves correctly on real data.**

Stated explicitly because a future reader who finds a corrected measurement will otherwise wonder whether the fix it justified survived. It does.

#### THE THIRD IDENTITY ERROR — and why nothing caught it

| | Error | Caught by |
|---|---|---|
| §17 | a record frozen against the wrong manuscript | **`manuscript_sha256`** |
| §19 | a journal recorded against a mismatched guidelines URL | **`journal_name`** |
| **§23.4** | **a measurement attributed to the wrong run in PROSE** | **nothing** |

**Prose carries no identity field.** The first two were caught because the artifact recorded its own subject and a reader could compare. **A sentence saying "run 22's findings" carries no checkable claim about which run produced them** — so nothing could catch this except re-running the analysis on the frozen manuscript, which is what run 23 did.

**That is why it survived into a design document and a test fixture.** §21's instrumentation-maturity framing applies with a sharper edge: this was **level 1** knowledge — a claim in prose — and level 1's failure mode is not "a contributor forgets the rule" but "a contributor writes something unfalsifiable and it is believed".

**The general lesson, and it is narrow:** a measurement quoted in prose should name the artifact it came from — `report:v2:21`, not "run 22" — because an artifact key is checkable and a run number in a sentence is not.

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

**A SECOND divergence, added by §31.22 — the PDF report drops `MAX_FINDINGS`.** The reviewer payload forwards at most 12 findings because the proxy caps a request at 8000 characters; the PDF has no such budget and shows every one. **Contract 1 stands unchanged — the payload is not modified by that decision** — but *"the report"* and *"what the model saw"* stop being interchangeable phrases, and this document has used them interchangeably. A run with 20 findings shows 20 on the page and sent 12 to the reviewer.

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

---

## 27. Run 23 — a deterministic-side-only capture

**NOT A BASELINE, and preserved as `box4_comparisons.run23.jsonl`** alongside runs 20–22.

**The wholesale side did not run.** `wholesale_path_available: false`, so `wholesale_recommendation`, `recommendation_agreement` and every downstream comparison metric are `Unavailable { requires_live_proxy }`. Harness step 5 fails, and there is no comparison to freeze.

### 27.1 What it establishes

**The resolver works on real data.** 20 report findings, **6 excluded, 14 counted**, reconciling exactly:

| | |
|---|---|
| **Excluded** | f12 `authorship_signal` Major; f15, f16 `process_state` Minor; f17, f18, f19 `process_state` Info |
| **Counted** | 11 Major (f1–f11), 2 Minor, 1 Info |

**The deterministic verdict on the frozen manuscript is `MajorRevision` at 0.30, and it is correct.** The 11 surviving Majors are five deterministic statistical-rule failures — three missing effect sizes, two missing confidence intervals — and six internal-duplication spans at 81–86% word overlap. **Every one is a `ManuscriptDefect`. Nothing in that set is a process claim or an authorship signal.**

**Schema 4 works end to end** — record written, `verdict_withheld` correctly `Unavailable { available_now }` (nothing was withheld), both sent-counts at 12, both digests populated, `journal_name` and `guidelines_url` recorded.

### 27.2 What it does not establish

**Nothing about agreement, promotion impact, or the wholesale reviewer.** The comparison never happened.

**The §19.2 journal/guidelines divergence persists:** `journal_name` is `"PLOS Medicine"` while `guidelines_url` is PLOS ONE's. Recorded, not validated — decision 4.

### 27.3 The guideline check, replaced

**"29 chunks imported" is invalid after first ingestion.** `ingest_document` dedups by checksum, so re-running the same URL returns `Skipped` and imports **zero** chunks — a correct outcome that the old check reads as failure.

**The replacement, for every future capture:**

1. **the guideline document exists** for that `source_url` in `documents`;
2. **retrieval succeeds** — `rag::chunks_for_source` returns a non-empty set;
3. **the checklist carries at least one item with `guideline_source: Some(_)`.**

**(3) is the actual check** — it is the only one that proves the content was usable rather than merely present. (1) and (2) exist to localise a failure when (3) fails.

---

## 28. A harness gap — `requires_live_proxy` is overloaded across four causes

**Found by run 23. Not implemented; recorded as the design.**

`wholesale_recommendation` carried `requires: requires_live_proxy`, which reads as *the proxy was not live*. **The proxy log shows five 403 Forbidden responses — it was live and refused on entitlement.** Run 21 was genuinely unreachable and produced the **same** value.

> **§4.14 inside the instrument built to prevent it: attributed absence with the wrong attribution, in the artifact whose entire purpose is making absence attributable.**

### 28.1 Where the distinction exists, and where it is destroyed

**It exists at the HTTP boundary.** `verify_with_envelope` (`proxy_client.rs:215-218`) branches on `!status.is_success()`, and the proxy's 403 carries `{"error":"not_entitled","reason":"no_uses_remaining"}` (`main.py:236`). A transport failure instead produces `"proxy POST … failed"` from `.send()`'s error arm. **`map_error_status` has no 403 arm**, so the reason survives only inside a formatted string — not matchable by a caller.

**It is destroyed at `commands.rs:659-662`**, where every failure collapses into `ReviewerEvaluation::unavailable_offline()`, which carries no cause. The harness then sees only `available == false`.

### 28.2 FOUR causes, one value

| Cause | Remedy | Run |
|---|---|---|
| proxy not configured / no signer (`commands.rs:663`) | configure the client | — |
| `/health` unreachable | bring the service up | **21** |
| **403 `not_entitled`** | **reset or raise the quota** | **23** |
| **our own gate rejected the response** | fix the payload or the model contract | — |

### 28.3 The fourth cause breaks the enum's PREMISE

`MetricAvailability`'s doc says it records *"the condition under which it WOULD be observed"*.

> **For gate rejection there is no such condition. The proxy answered, a value WAS observed, and our own gate refused it. No external state would make it appear.**

**Filing it under `requires_live_proxy` does not merely misattribute the cause — it attributes OUR failure to a third party.** That is why the fourth case needs different *treatment*, not just a different variant: the other three name a missing external condition, and this one names a decision we made.

### 28.4 Three variants, and the third must locate the fault correctly

* **`RequiresEntitlement`** — a genuine availability condition; remedy is a quota reset or a raised limit.
* **`ProxyUnconfigured`** — likewise; remedy is client configuration, not service health.
* **`SelfRejected`** — the gate refused a response we received. **Any phrasing implying the service failed would repeat the error being fixed.**

### 28.5 §26's caution does NOT apply, and a future reader must not think it does

§26 warned against `MetricAvailability` absorbing **editorial policy** — whether a claim is a publishability defect — because that would key one policy on a type meaning another (§21's `ConfidenceKind` rule).

> **These are OPERATIONAL causes of a missing value, which is exactly what the enum exists for.** The addition is on-purpose, not an exception to §26.

### 28.6 What it would take

1. `map_error_status` gains a **matchable** 403 arm — the type must be selectable by a caller, since `commands.rs` cannot branch on a string.
2. `commands.rs:641-663` stops collapsing. The cause belongs on **`ProxyMeta`**, not on `ReviewerEvaluation`: the letter is the reviewer's output, and this is a fact about the **call**. `ProxyMeta` already exists, is already `Option`, and is already threaded to `HarnessInputs`.
3. `build_comparison_report` selects on the cause rather than on `available` alone.

### 28.7 UNESTABLISHED — and narrower than it first looked

**What the client log settles.** Run 23's log names the reviewer's refusal directly:

> `reviewer cloud call failed; marking unavailable error=proxy returned 403: not_entitled`

**So the reviewer call WAS made and WAS refused.** An earlier draft of this section listed *"the reviewer was never attempted because earlier callers exhausted the quota"* as an alternative. **That situation is not merely unobserved — it is impossible**, and the list should say so:

* **no client-side short-circuit exists.** Each caller builds its own `ProxyReqwestClient::from_env()` (`commands.rs:577, 605, 642`) with no shared state and nothing recording a prior 403.
* **`reachable()` probes `/health`** (`proxy_client.rs:139-144`), which is not entitlement-gated, so a refusal elsewhere cannot make the reviewer's client look unreachable.
* **Exhaustion produces a 403 on EVERY subsequent call** rather than suppressing later ones.

**Listing a situation the evidence rules out is the shape of error §23.4 was**, which is why it is corrected here rather than after.

**What genuinely remains unestablished is narrower: WHICH of the five 403s was the reviewer's.** It does not change run 23's interpretation, because the client log names it. **So the "same gap one level out" is about per-caller attribution GENERALLY, not about this run.**

**Per-caller metering (decision 3) closes it as a side effect** — a per-feature counter makes each caller's consumption separately visible, so which caller was refused becomes readable from the server side. **That makes the metering work more valuable than its own justification suggested:** §20.2 argued it on billing semantics, and it also buys observability the harness cannot get on its own.

### 28.8 Record-interpretation note

**Adding variants is serialization-compatible** — no existing value changes meaning, so **no `schema_version` bump** under §26.4's rule.

**But a reader comparing run 21 or run 23 against any future record must know that `requires_live_proxy` was previously OVERLOADED across four causes.** The old records are not wrong about what was observed; they are imprecise about why, and nothing in them says so.

---

## 29. Run 24 — the first complete comparison, and why it is not the baseline

**Preserved as `box4_comparisons.run24.jsonl`.**

### 29.1 What it establishes — the first complete comparison

| | |
|---|---|
| `wholesale_path_available` | **true** — the first run where it is |
| `shadow_recommendation` | `major_revision` at 0.30 |
| `wholesale_recommendation` | `major_revision` at 20.0 |
| **`recommendation_agreement`** | **true** |
| `model_identifier` | `gpt-4o-mini-2024-07-18`, `stop_reason: stop` |
| Both sent-counts | 12 / 12 |
| `wholesale_grounded_issues` / `hallucination_drops` | 3 / 2 |

**The deterministic verdict was validated against a live wholesale reviewer on the frozen manuscript, and the two agreed.** That is the first time every layer of the comparison ran end to end.

**Artifact identity verified before interpretation:** `summary_digest` in the persisted record is `2d65a8c8…b858c67ea48a`, identical to the value printed in the run log — **so the metrics being read belong to the artifact on disk, not a different write.**

### 29.2 Why it is NOT the baseline — and the divergence was NOT CORRECTABLE

`journal_name` is `"PLOS Medicine"` while `guidelines_url` is PLOS ONE's. **`journal.name` reaches `summary.journal` (`reviewer_agent.rs:435`), so the wholesale model was told a target journal that does not match the guidelines the checklist was built from** — confounding exactly the half of the comparison Box 4 exists to measure.

> **Runs 22, 23 and 24 are identical because of a MECHANISM, not repeated operator error. The instruction to select PLOS ONE was impossible, three times over.**

**Two stacked defects:**

1. **The selection is IMMUTABLE once made.** `PublishReadyPage.tsx:313-339` renders the match list only while `journal` is `null`; once set, it is replaced by a static *"Target: X"* line with **no clear control and no way to re-open**. The search box remains, accepts typing, and recomputes `journalMatches` into a branch that is never rendered.
2. **PLOS ONE is ABSENT from the directory.** `scopusDirectory.json` has 258 entries and exactly one PLOS journal — **PLOS Medicine**. Search is `j.name.toLowerCase().includes(q)`, so *"plos"* or *"plos one"* can only ever return PLOS Medicine.

**Even a working reset control would not have made PLOS ONE selectable.**

### 29.3 Classification

**NOT an observability defect.** The UI honestly displays what it will send — `journal.name` is rendered and `journal.name` is transmitted, one value on one path, with no persistence and no stale copy. There is no divergence between what the operator saw and what was sent.

**It is a CORRECTABILITY defect plus a DIRECTORY COVERAGE gap.**

### 29.4 Tracked items

**IMMUTABLE JOURNAL SELECTION.** A user who picks wrongly is stuck until the component unmounts. **A real product defect** — and it **does not gate the baseline**, because a UI correctability fix cannot change a verdict. *Proposed fix: render the match list whenever `journalQuery` is non-empty, or add a clear control.*

**DIRECTORY COVERAGE.** PLOS ONE's absence invites the question of what else is missing from 258 entries. **Not surveyed — the question is recorded, not answered.**

### 29.5 The journal/guidelines pair for the next capture — VERIFIED, not assumed

`https://journals.plos.org/plosmedicine/s/submission-guidelines`, probed end to end:

| Step | Result |
|---|---|
| Ingest | `Ingested { chunks: 19 }` |
| Retrieval (`journal_guideline`-filtered) | **5 hits** |
| **Checklist** | **6 items, 2 GUIDELINE-DERIVED** — *numbered (Vancouver) reference style*, *conflict-of-interest declaration* |

> **The third check is the one that matters: `guideline_source: Some(_)` proves the content was USABLE, not merely fetched.** Ingestion succeeding and retrieval returning hits were both true for the BMJ homepage in run 20, which produced zero guideline-derived items (§18.6.2). The `strip_block` and homepage-prefill defects each passed the first two checks and failed the third.

**Pairing PLOS Medicine with PLOS Medicine's guidelines makes the next capture internally consistent, needs no code change, and is selectable in the directory.**

---

## 30. Run 25 — the pre-promotion operational baseline (n=1)

**FROZEN. Preserved as `box4_comparisons.run25.jsonl`.**

**Never "baseline" alone, never "agreement baseline"** — the label is *pre-promotion operational baseline (n=1)*, per `harness_log.rs`'s capture protocol.

**Artifact identity verified BEFORE anything was read from it:** the persisted `summary_digest` is `2d65a8c8…b858c67ea48a`, identical to the logged value.

### 30.1 The freeze condition — every part holds

| | |
|---|---|
| `manuscript_sha256` | `859880647c…` — the frozen manuscript |
| `schema_version` | **4** |
| `journal_name` | **PLOS Medicine** — **matching** `guidelines_url` |
| `guidelines_url` | `plosmedicine/s/submission-guidelines` |
| Both sent-counts | 12 / 12 |
| Both digests | present |
| `wholesale_path_available` | **true** |
| `wholesale_recommendation`, `recommendation_agreement` | both **Observed** |

### 30.2 The promotion delta

> **On this manuscript and this execution, promotion would change the recommendation presented to the user from Reject (10%) to Major Revision (30%).**

**One manuscript, one execution, against a non-deterministic counterparty. Not a rate, and not evidence the deterministic side is better or worse.**

**Run 24 on the same manuscript produced `major_revision`/20.0 with agreement TRUE; run 25 produced `reject`/10.0 with agreement FALSE.** §18.1's non-determinism, now observed on the frozen manuscript.

### 30.3 Digest coverage — investigated in order, stopped at the first failure

**STEP 1 — PASSES.** `summary_digest` hashes all of `payload["summary"]`, which contains `checklist`.

**STEP 2 — PASSES, and explains the identical digest across runs 24 and 25 despite different guideline sources.** Both produce the same six items in the same order with the same `passed` values. **The full checklists differ ONLY in `guideline_source`, and `guideline_source` is not in the payload** — `build_review_payload` emits `{id, requirement, passed}` per item. Two different source URLs yielding the same requirements produce a byte-identical payload checklist.

**STEP 3 — FAILS. Two reviewer-visible inputs sit outside the digest.** The proxy forwards only `{"summary", "instruction"}` as the user message (`openai_client.py:47-49`), plus a system message the client never sees.

* **`instruction`** — a compile-time `const`, so constant *within* a binary but not across binaries.
* **the server-side system prompt** (`_DEFAULT_SYSTEM`) — `main.py:183` constructs `OpenAIClient` with no `system=`, so it falls to a module constant in the **separately deployed** proxy. **A server-side edit changes the model's input with no client-side signal of any kind** — §24.3's shape, a value the comparison depends on that no layer owns.

**No prompt or template version field exists anywhere**; a repo-wide grep returns nothing. `model` is env-overridable via `OPENAI_MODEL`, but `model_identifier` is recorded, so that one is observable.

**STEP 4 — NOT REACHED.** §18.1's *"strong evidence, not a controlled demonstration"* **stands unchanged**: `journal_name` being identical here removes one confound and does not remove these two.

#### THE BASELINE IS VALID DESPITE STEP 3 FAILING

**These are separate properties.** The freeze condition concerns **the record's completeness and configuration coherence**; the digest gap concerns **what CROSS-RUN comparisons can claim**. **§30 is sound as a baseline.** A reader hitting "STEP 3 FAILS" should not infer the baseline is compromised — it is not.

#### THE GAP IS EVIDENTIARY, NOT NECESSARILY FACTUAL

**`instruction` is a compile-time `const`, runs 24 and 25 used the SAME BINARY** (the 01:54 build), **and the proxy was not redeployed between them.** So the input across those two runs was **probably identical**.

> **What is missing is the ability to PROVE it from the artifacts, not evidence that it differed.** A later reader could otherwise conclude the runs differed, when the likelier truth is that they did not and the record cannot show it.

**`summary_digest`'s definition is narrowed at the field itself** to say it is a summary digest, not an input identity.

### 30.4 Fix candidates — recorded, NOT built

**The system-prompt gap has an obvious remedy with no new mechanism.** The proxy already returns `model_identifier` in its envelope; **returning a hash of the system prompt alongside it** would make the invisible input observable through a channel that already exists and is already threaded to `HarnessInputs` via `ProxyMeta`.

**`instruction` could be hashed client-side** and recorded the same way — it is a `const` in scope at `build_review_payload`.

| | Cost |
|---|---|
| system-prompt hash | one field in the proxy envelope, one in `ProxyMeta`, one in the record. **No new channel.** Requires a proxy deploy |
| `instruction` hash | client-side only; three lines and a record field |

**What they would buy:** with `summary`, `instruction` and the system prompt all covered, **identical digests plus different recommendations WOULD be the controlled demonstration §18.1 wanted.**

**Not implemented. Whether the promotion decision needs that level of proof is a separate question** — §30.2's delta is already measurable without it.

---

## 31. The PDF report contract — Milestone 1

**The PDF becomes PublishReady's primary output; the desktop app becomes the engine that produces it.** `publication_probability`, the gauge and the score ring are removed. **The recommendation is the product and the evidence explains it.** Similarity percentages stay, because those are measured.

### 31.0 THE GOVERNING RULE

> ## THE REPORT MUST DESCRIBE WHAT THE ENGINE MEASURED, NOT WHAT A HUMAN NATURALLY INFERS FROM IT.

**It generalises past this document.** A field that exists is a field the engine owns; a field the engine cannot produce faithfully is omitted, not fabricated and not inferred. This is F2/F6's principle applied to presentation rather than to the verdict.

### 31.1 THE HEADLINE — similarity, and the DESIGN overclaimed

**Not the engine underdelivering.** `MatchSpan.similarity` is **cosine over whole-chunk embeddings** (`plagiarism.rs:187-204`). There is **no sub-chunk match to locate, and no offsets because there is nothing to offset** — the comparison unit *is* the chunk.

> **Turnitin shows exact matched text because it does string matching. This engine does embedding similarity over chunks.** A side-by-side layout under a similarity number implies *"these passages matched"* when the engine established *"these two chunks are similar"*.

**VOCABULARY, DECIDED.** Do **not** say *"matching passage"*, *"matched text"*, or *"N% of the words are the same"*. Say **SIMILAR TEXT REGIONS** or **SIMILAR DOCUMENT REGIONS** — the unit named is the unit the engine compared.

**What a match carries today:** `manuscript_chunk_seq`, `manuscript_excerpt`, `similarity`, and the other side's `excerpt` (both `Corpus` and `SelfManuscript`). `excerpt()` is `trim()` truncated at `EXCERPT_CHARS` with an ellipsis — **a chunk prefix, not a sentence**, and chunks are not sentence-aligned.

#### The existing wording was already corrected — do not reopen it

**`83f192c` — *"correct labels that claimed more than the algorithm measures"*.** Its rationale is recorded per replacement:

| Was | Became | Why |
|---|---|---|
| *"internal duplication (self-plagiarism)"* | *"(same manuscript)"* | asserted self-plagiarism as a finding |
| *"verbatim"* | *"near-identical wording"* | word-order-blind cosine cannot establish byte identity |
| *"near-verbatim"* | *"high word overlap"* | inherited the identity claim at a weaker threshold |
| *"paraphrase"* | *"partial lexical overlap"* | **inverted** — a real paraphrase scores BELOW threshold and is never reported |
| *"{N}% similarity"* | *"{N}% word overlap"* | *"similarity"* reads as semantic |
| *"semantic signal"* | *"lexical-overlap signal"* | asserted a semantic model not in the tree |

**Both sides carry a comment explaining why the words were chosen "so they are not 'improved' back."** That was a considered decision, made well.

**The narrower question, answered:** *"word overlap"* remains the best available description of the **dimension** — lexical, not semantic, word-order-blind. **The residual imprecision is the `%`, not the phrase:** cosine over feature-hashed term-frequency vectors is not literally the fraction of shared words. **That is a rendering question for Milestone 2**, and it points the same way as removing the probability gauge.

### 31.2 Classification

**A — READY TODAY, presentation only**

manuscript title (`Option`, so absence is honest) · target journal · assessment date · run id · **reference count** · **table count** · finding counts by tier · **excluded count and per-finding reasons** · section location · recency numbers · citation-style percentage · table caption count · checklist with the guideline/structural split.

**THE REPORTED-STATISTICS BLOCK IS A, minus the F value.** `MissingEffectSize` is emitted per p-value location (`validate.rs:256-266`), and `Flag.location` is the same `Location` carried by every `StatClaim` — so filtering `extraction.statistics` on matching `location` recovers what *was* reported. `Stat::Test { name }` gives a canonical test label and `Stat::PValue { operator, value }` renders `p < 0.05` exactly. **"Reported: One-way ANOVA, p = 0.02 · Missing: effect size" ships at zero engine cost, and it is the most actionable block in the report.**

**B — HONEST PLACEHOLDER**, with the smallest change to reach A:

| Item | Why | Smallest change |
|---|---|---|
| **F statistic** | `Test.raw` is the *test-name* match, not `F(2,27)=14.3`; no F extractor | one regex + a `Stat::TestStatistic` variant — **additive, no schema bump** |
| **nearby text** | no finding carries a manuscript snippet | a lookup: `sections[k].paragraphs[loc.paragraph]`. **No extraction change** — see §31.3 |
| **similarity excerpts** | chunk prefixes, not passages | **possibly zero code** — a vocabulary decision (§31.1) |
| **page count** | pagination is lost at parse; the engine never receives one | **stays B and OMITTED** — see §31.4 |
| **subsection** | `Location` has `section` + `paragraph`; `SectionKind` is flat | `subsection: Option<String>` on `Location` — **largest**: touches every producer and the evidence schema |

**C — NEW PRODUCT FEATURE**

**A prioritised revision checklist**, if it means remediation advice rather than a restatement of findings. A restatement is A. **Advice is a separate PR and must not enter the report work.**

### 31.3 The nearby-text boundary is STRUCTURAL, not conventional

The lookup is cheap **and it puts manuscript prose into the report.** The PDF is a **local artifact**, so that is appropriate. `build_review_payload` **crosses to the proxy**, so it is not.

> **The snippet must live in a type the payload builder CANNOT REACH — a report-assembly-layer or local-only type — so it cannot accidentally flow remote, rather than merely not doing so today.**

`build_review_payload` already drops `detail` deliberately for exactly this reason. **A convention that "we don't put snippets in the payload" is a rule someone can forget; a type that has no snippet field is one they cannot.**

### 31.4 Page count — omitted, with the substitution recorded

The frontend has `estimatePdfPageCount`, and threading it in would be easy. **It is an ESTIMATE**, so rendering *"13 pages"* is the report implying knowledge the engine lacks — §31.0 exactly.

> **Once the PDF exists, its own page count is a fact the report genuinely owns.** That is the substitution: not the manuscript's page count, the report's.

### 31.5 `exportPdf.ts` — REPLACE, not a starting point

62 lines. It emits **combined confidence as a percentage** and **per-finding confidence as a percentage** — two of the precision claims this workstream exists to remove. No cover, no grouping, no exclusions, no similarity, no journal section.

**Its only reusable asset is the `lines.push({ text, size, gray })` primitive.**

### 31.6 The severity leak is an ABSTRACTION leak, not a wording problem

**FACT.** `FindingSeverity` has **no label function in Rust** — `rank()` only (`report.rs:126-137`). Every severity string a user sees is TypeScript doing `f.severity.toUpperCase()`.

> **No presentation layer for severity exists at all. Nobody chose "Major" as user-facing language — the product has been exposing an internal enum value, uppercased.**

**So introducing *"Important issue"* is CREATING the first presentation vocabulary for severity, not rebranding an existing one.** Materially smaller and cleaner than a rename: there is no prior decision to overturn and no second site to keep in step, because the second site is `.toUpperCase()`.

Contrast `CertaintyTier`, which **does** have `label()` (`report.rs:97`) and whose wording was deliberately chosen. **Severity never got that treatment.**

### 31.7 VERIFIED — both digests are already partly EDITORIAL

**`title` is user-facing prose, and it is inside both digests.**

* `build_review_payload` emits `"title": clamp(f["title"])` per finding (`reviewer_agent.rs:520`).
* `findings_projection_digest` hashes `(id, severity, **title**)` (`:411-415`).
* `summary_digest` hashes all of `payload["summary"]`, which contains those findings.

> **A typo fix, a wording improvement or a clarity edit to any finding title changes BOTH digests today.**

**Another claim-narrower-than-assumed correction, in the same family as §18.3 and §30.3.** `summary_digest` is **already partly an EDITORIAL digest, not purely an analytical one**, and identical digests imply **identical presentation-plus-analysis**, not identical analysis.

**True since §18.7 and never stated.** It sharpens the Milestone 2 caution: moving labels onto the wire would not *make* the digest editorial — **it already is**. What changes is how often an editorial edit moves it.

### 31.8 The nine label sites, classified

**The guard that stops the vocabulary becoming "every string in the application"**, which would recreate the coupling it exists to remove.

| Site | Class |
|---|---|
| `FindingSeverity` *(no producer — TS `.toUpperCase()`)* | **ANALYTICAL** |
| `ClaimKind` → *Technical check* / *AI writing signal* | **ANALYTICAL** |
| `Recommendation` / `RECOMMENDATION_LABEL` | **ANALYTICAL** |
| `CertaintyTier::label` (`report.rs:97`) | **ANALYTICAL** — already correct, already Rust-owned |
| `match_type_label` (`report.rs:66`) + `matchTypeLabel` (`adapters.ts:38`) | **ANALYTICAL** — hand-mirrored today |
| `RuleId::label` (`validate.rs:68`) | **GENERATED** — a rule identity, consumed *inside* a title |
| **finding `title`** (12 constructors) | **GENERATED** |
| `adapters.ts` hardcoded `certainty_label` (5×) | **ANALYTICAL, currently duplicated** — the wire already carries it |
| `AGENT_LABEL` (`exportPdf.ts`), `STAGE_LABEL` (`useAnalysis.ts`) | **PURE UI** |

> **THE BOUNDARY: vocabulary owns closed concepts · producers own generated findings · presentation maps concepts and never generates evidence.**

**Carried forward, unresolved:** finding titles are **formatted sentences with interpolated data**, and the vocabulary cannot own them — **centralising them would replace typed producers with string templates, which is a regression.** And the agreed mappings still need reconciling against these sites, including `RECOMMENDATION_LABEL`'s caps.

### 31.9 ARCHITECTURAL DECISION — the report is the canonical output and Rust owns it

**Phase 4 of the project: artifact ownership.**

> **The UI becomes a VIEWER of the report, rather than the report being an EXPORT of the UI.**

**The reason is not that Rust makes better PDFs.** It is that **every milestone completed was about the report and none was about the React screen** — identity (§26), provenance (§22), evidence (§23), exclusions (§26 PR-3), guideline traceability (§18.6), the recommendation (§26 PR-4), the baseline freeze (§30), digest discipline (§18.7, §30.3). **The React screen has been the accidental owner of the thing the whole project was building.**

**What made this decidable without a trade-off:** §31.5's estimate found that **layout is the work, and it is the same work in either language**. Neither `lopdf` nor `miniPdf.ts` does layout; the current word-wrap is an approximation either way.

> **Cost is language-neutral; only ownership differs. That is the REVERSE of the usual shape, where the better architecture costs more.**

### 31.10 THREE LAYERS — the refinement that makes the rule hold

| Layer | Owns | Knows nothing about |
|---|---|---|
| **ENGINE** | `ReportModel` — data | documents, pages, sections-as-layout |
| **COMPOSER** | semantic blocks: **which sections exist and in what order**, in ONE place | pages, columns, breaks |
| **RENDERER** | PDF / HTML / DOCX / email: **lays blocks out, computes nothing** | the database, the filesystem, business logic |

**Why not two layers.** If the **engine** emits blocks it knows what a cover is — **presentation leaking backwards**. If **renderers** decide sections, three renderers diverge on the first edit — **the drift this project has now seen four times** (`matchTypeLabel`'s hand mirror, `adapters.ts`'s hardcoded tiers, `synthesize.ts:45`, `certainty_label`).

> **The composer is what the rule actually names, and collapsing it into either neighbour reintroduces the problem it solves.**

**The renderer receives ONE IMMUTABLE STRUCTURE. No SQLite, no filesystem lookups, no business logic.** That is what makes a second renderer cheap and a third one safe.

### 31.11 REFRAMING — §31 is not fundamentally about PDF generation

**It is about the primary artifact meeting the standards already imposed on the engine.**

| The engine already requires | The artifact now requires |
|---|---|
| typed data, not untyped projection | `ReportModel`, not `serde_json::Value` re-parsed at `commands.rs:546` |
| presentation vocabulary, not leaked internals | `severity_label`, not `.toUpperCase()` on the enum |
| explicit omissions, not silent gaps | a marked omission, not `[^\x20-\x7E]` deletion |
| labels only where a concept needs one | `claim_label` returning `Option`, `None` for the default |
| rendering that preserves evidence | the render path asserted, not just the analysis path |

> **The report architecture is an EXTENSION OF THE CORRECTNESS PHILOSOPHY, not a separate UI effort.**

Every item above is a rule this project already applies to the engine, applied one hop further. **PDF generation is the occasion, not the subject.**

### 31.12 The `Deserialize` gap — five derives, no deliberate omission

`PublishReadyReport`, `Finding`, `ChecklistItem`, `DebateSummary` and `CertaintyTier` are `Serialize` only. `FindingSeverity`, `AgentKind`, `ClaimKind` and `EvidenceRecord` already have `Deserialize`.

**Nothing in the tree resists it** — `Finding`'s only non-derived member is `CertaintyTier`; `ChecklistItem` is strings and bools; `DebateSummary` holds `Vec<AgentKind>`, already covered.

**No deliberate omission.** `evidence.rs:164-166` records the opposite case explicitly — *"Derives `Deserialize` (added in Box 2) so the Orchestrator can reconstruct records… this also pulled `Deserialize` onto `AgentKind`/`FindingSeverity`."* **The pattern was to add it when a consumer appeared. No consumer appeared for the report, because `commands.rs` reached for `Value` instead.**

> **Five derives would make `commands.rs:546` parse a typed report, let `build_review_payload` stop indexing `report["findings"]`, and turn §22's `overall_verdict`-reduced-to-a-string into a typed field. `ReportModel` then becomes mostly NAMING rather than mostly RECOVERY.**

**Two caveats.** `#[serde(flatten)]`/`#[serde(tag)]` interact awkwardly with `Deserialize` — not present in these types, but worth checking as they grow. And **`Deserialize` on a public type is a CONTRACT**: anything shaped like the JSON becomes a `PublishReadyReport`, so §26.4's compatibility-pin discipline extends to them. **A benefit, but a decision rather than a side effect.**

### 31.13 The base-14 limit is a MARKET problem, and the current behaviour is worse than missing glyphs

**`miniPdf.ts:29` — `.replace(/[^\x20-\x7E]/g, '')`. Not WinAnsi: ASCII printable, everything else SILENTLY DELETED.**

| Input | Renders as |
|---|---|
| `Müller` | **`Mller`** |
| `Kumar Śarmā` | **`Kumar arm`** |
| `हिन्दी` | **empty** |

Lines 23-28 first fold typographic quotes, dashes and ellipses to ASCII — deliberate and correct. **Line 29 then removes the remainder.**

> **The failure is not missing glyphs. It is silent, plausible-looking corruption** — `Mller` reads as a name, and a viewer shows no error because the characters were gone before the PDF was written. **ONTOLOGY §4.20's TEXT class, and the instance that named it.**

**Against Gaply's market this is not an edge case.** Indian academic and legal research means diacritics routinely and Devanagari in titles, and **a manuscript title is on page one of the primary artifact.**

**The fallback, in increasing cost:**

1. **Mark the omission** — a visible sentinel instead of deletion. One line. **Required regardless of what else is decided.**
2. **Extend to WinAnsi** — Latin-1 covers `Müller` and `naïve` with **no font file**, since base-14 carries WinAnsiEncoding. An encoding map. **Does nothing for Devanagari.**
3. **Embed Noto Sans / Noto Sans Devanagari** — OFL, redistributable, ~450 KB and ~250 KB unsubsetted; subsetting is a further crate.

**This changes §31's v1 estimate.** *"Direct `lopdf` + an AFM table, no font file"* holds **only if the market is Latin-1**. It is not — so **the font decision is deferrable for LAYOUT but not for CORRECTNESS**, and (1) is what makes deferring it honest.

### 31.14 The three vocabulary gaps, settled

**`ManuscriptDefect` → `claim_label` returns `Option<&'static str>`, `None`.** Typed absence rather than an invented word. **The label exists to mark the EXCEPTIONS** — a counted finding is just a finding; only excluded ones need to say why they did not count. This is what the specimen already did.

**`Critical` — the case, found before naming it.** Two rules emit it (`validate.rs:58-66`):

* `TestGroupMismatch` — *"A t-test compares exactly two groups, but the surrounding text refers to {count} groups… inflates the false-positive rate"*
* `SmallSampleCausalClaim` — *"A strong causal claim is paired with a very small sample (n = {n} < 10)… conclusions are unreliable"*

The code's own comment: *"wrong test / underpowered causal claim **invalidate the analysis**"*.

> **Both say the same thing, and it is not "more serious than Major" — it is a DIFFERENT KIND of problem. `Major` findings are things MISSING from a sound analysis; `Critical` findings are things WRONG WITH the analysis itself.** The label should be about validity, not severity.

**Unobserved across runs 22-25** (`critical: 0` every time), so whatever word is chosen ships untested against real output.

**`RECOMMENDATION_LABEL` → sentence case.** `"Major revision"`, with `text-transform` doing headers. **Caps are presentation, and the PDF needs the label in a sentence.**

### 31.15 The two instruments for §4.20 — a contract, not a plan

| | Verifies | Ships |
|---|---|---|
| **PARTIAL** | the SANITIZER'S TRANSFORMATION — what `toAscii` returns for a given input | **now** |
| **FULL** | REPRESENTATION CORRECTNESS — the artifact's rendered BYTES against known Unicode input | **with the Rust renderer** |

> ## PASSING THE FIRST DOES NOT IMPLY THE SECOND.
>
> **The sanitizer can be correct while the renderer is wrong, and the reverse.** An assertion on `Finding.title` passes while the PDF says `arm`. **A future reader must not conclude the sanitizer test covers §4.20 — it covers one function inside it.**

**Why the full instrument waits, stated rather than tracked:** asserting `miniPdf`'s bytes today would **instrument the thing being replaced.** Its home is the first PDF PR. *(Said explicitly because `StageUndelivered` nearly drifted into an open-ended wait for the same reason — §26.13.)*

**The partial ships anyway** because the Rust renderer may be weeks out and **the fix should not sit unguarded in the interval.**

### 31.16 The sanitization policy — two tiers, argued rather than assumed

**The invariant is NEVER SILENTLY DELETE. The transformation policy is a separate decision, and the honest answer differs by script.**

**TIER 1 — Latin with diacritics → TRANSLITERATED.** NFD decomposition drops the combining marks: `Müller` → `Muller`, `Kumar Śarmā` → `Kumar Sarma`.

**Why transliteration and not a mark:** it is what library catalogues do, **the reader recovers the original**, the name stays recognisable, and the meaning is unchanged. `M?ller` satisfies the invariant and is hostile to every European name in the corpus.

**TIER 2 — non-Latin → MARKED, never transliterated.** `हिन्दी` → `□□□□□□`, folded to `?` at the byte layer.

**Why marking and not transliteration:** **no ASCII form of Devanagari, CJK or Arabic preserves meaning for a reader.** IAST and ITRANS are scholarly conventions rather than rendering fallbacks, and both carry diacritics of their own — so transliterating here would produce a plausible-looking romanisation that is **§4.20's error in a new place**. A visible mark states what is true: something was here this renderer cannot show.

**The mark is kept distinct through `toAscii` and folded to `?` only in `escapePdf`**, so a manuscript that genuinely contains `?` is never confused with a character we could not render — and a test can tell them apart.

**This is not a fix for the underlying limitation.** Part of tier 1's work can be replaced by RENDERING the characters: base-14 fonts carry WinAnsiEncoding, covering Latin-1 at **zero font cost**. It is unreachable in `miniPdf` because `new Blob([string])` encodes UTF-8 and a multi-byte character would shift the xref offsets — **a technical block, not a scope decision.** It belongs with the Rust renderer (§31.13).

> **SCOPE CORRECTION (§31.17.1): "part of" is load-bearing and was absent when this paragraph was first written.** WinAnsi covers **Latin-1**, not **Latin**. Tier 1 narrows at the port; it does not lift.

### 31.17 TO THE AUTHOR OF THE RUST RENDERER — tier 1 NARROWS, it does not vanish

> **THIS SECTION REPLACES A CLAIM THAT WAS MEASURABLY FALSE.** The previous §31.17 said Latin transliteration *"does not survive the port"* and instructed the renderer's author to **DELETE** tier 1. Acting on that instruction ships `?arm?` for a name. **§4.4 — the claim is corrected, not caveated.**

#### What was true, and where it stopped being true

The old reasoning: Rust writes bytes directly → base-14 carries WinAnsiEncoding → WinAnsi covers Latin-1 → therefore transliteration is unnecessary.

**Every step is true except the conclusion's scope.** `Latin-1` is not `Latin`, and the whole error fits in that one substitution.

#### The measurement, so the claim is checkable rather than trusted

> **WinAnsiEncoding contains exactly SEVEN characters from Latin Extended-A: `Œ œ Š š Ÿ Ž ž`.**

Everything else in that block is absent — which is most of the Polish, Czech, Turkish, Hungarian, Romanian and romanized-Sanskrit alphabets.

| Author name | Base-14 WinAnsi, no tier 1 | |
|---|---|---|
| Müller, García, François, Žilina | unchanged | **intact, byte-exact** |
| **Śarmā** | `?arm?` | sentinels |
| **Łukasz** | `?ukasz` | sentinels |
| **Dvořák** | `Dvo?ák` | sentinels |
| **Öztürk Şahin** | `Öztürk ?ahin` | sentinels |
| **Ştefănescu** | `?tef?nescu` | sentinels |

#### The corrected fate — a narrowing, stated as a scope change

| | Tier 1 applies to |
|---|---|
| **`miniPdf.ts`** (UTF-8 byte path) | **everything non-ASCII** |
| **Rust renderer** (WinAnsi bytes) | **Latin beyond Latin-1** |

**Two different limits, and only the first one lifts.** Rust can *write* Latin-1 bytes, so `Müller` stops being transliterated and renders exactly — that part of the old claim holds. Base-14 still cannot *represent* Latin Extended-A, so `Śarmā` must still fold to `Sarma`.

| | Fate at the port |
|---|---|
| **TIER 1** (Latin transliteration) | **NARROWS to Latin-beyond-Latin-1. Do not delete** |
| **TIER 2** (non-Latin marked) | **survives unchanged** until a font is embedded |

#### Why folding beats marking, for names specifically

> **`?ukasz` is unusable while `Lukasz` is wrong but readable, and for a NAME recognisability is the axis that matters to its owner.**

**Latin-1 stays byte-exact and untouched; only what base-14 cannot represent at all is folded, and only to its own base letter.** This is narrower than `miniPdf`'s tier 1, which folds `ü` as well — the Rust renderer must not.

#### The error was in the design document, and the check that caught it

**The Milestone 3 design proposal asserted that the render-path instrument would prove `Śarmā` appears in the PDF bytes AS `Śarmā`** — described there as *"the assertion the sanitizer test cannot make"*.

> **That assertion is not merely untested. It is IMPOSSIBLE in base-14, and the fixture would have failed on its first run.**

**Caught by checking whether the character was encodable BEFORE building the test around it** — the same discipline, applied in the same hour, that verified `pdf-extract` decodes WinAnsi before the instrument was allowed to trust it as an oracle. **Both checks were cheap; both would have been expensive as first failures**, and one of them would have been debugged as a renderer bug when the renderer was correct.

**This is §31.20's pattern in a new medium.** There the rule was *assert the pattern matched before replacing it*; here it is **verify the fixture is representable, and the oracle is trustworthy, before either is load-bearing.**

### 31.18 The font decision — deferred TO THE RENDERER PR, with a destination

**Not "tracked".** Attached to the renderer PR, the way `StageUndelivered` was given a destination (§26.13).

**The reasoning:** today's sentinel for Devanagari is **honest and rare** — most submissions to international journals carry Latin titles, so the marked case is the exception rather than the norm. **Solving it in TypeScript means embedding a font in a renderer that is being replaced; solving it in Rust means solving it once.**

**What the renderer PR decides:** whether to embed Noto Sans / Noto Sans Devanagari (OFL, redistributable, ~450 KB and ~250 KB unsubsetted) or to keep tier 2's mark. **A bundle-size decision, not a technical one** — the technical answer is settled.

### 31.21 Milestone 2 — the design changed mid-build, and the project's own rule caused it

**The first implementation stored `severity_label` and `claim_label` on `Finding`**, following `certainty_label`'s precedent. **The compiler named twelve construction sites — and that was the signal to re-check the design rather than push through.**

> **ONTOLOGY §4.19, recorded two commits earlier, forbids it: the ENGINE emits a report that knows nothing about presentation, so a user-facing label baked into the engine's PERSISTED OUTPUT is presentation leaking backwards.**

**A rule written down caught a mistake being made by the person who wrote it, in the window where it was still cheap.** That is the best outcome available from a recorded rule, and it is worth naming as such.

**The labels are attached at the BOUNDARY instead** — `vocabulary::enrich_report_labels`, called from `get_report` and from `run_publishready`'s IPC field. That is the COMPOSER's job under the three-layer rule.

#### Both open questions DISSOLVED rather than being answered

| Question | Under the boundary design |
|---|---|
| **Does this need a `CACHED_REPORT_SCHEMA_VERSION` bump?** | **No — the labels never enter the persisted shape.** Storing them WOULD have required one: a `serde(default)` empty label is *readable and wrong* (an old cached report renders findings with no severity), which is §26.4's compatible-and-wrong case |
| **Do labels leak into `build_review_payload`?** | **Structurally impossible.** The payload is an explicit seven-key projection, and `summary_shape_is_pinned_to_the_format_version` already guards that key set. No label edit can move `summary_digest` (§31.7) |

> **A design that makes questions unnecessary is better than one that answers them.**

**A further property, unplanned:** enriching at the boundary means **a wording edit applies to reports cached before it**, because nothing stale was ever written.

#### CORRECTION — `adapters.ts`'s five strings cannot read the wire

An earlier proposal said they should *"read the wire or be deleted"*. **That was wrong.** `adapters.ts` builds **SYNTHETIC** `PublishReadyReport`s client-side from raw plagiarism and AI-detection output — **those reports never crossed a wire, so there is none to read.**

**They are a genuine second copy, exactly like `matchTypeLabel`, and PINNING is the only available option.** Both are covered by the mirror artifact.

#### The mirror pin

`src/generated/vocabulary.json` is checked in. **Rust asserts the artifact matches its functions; vitest asserts the TypeScript tables match the artifact.** Neither side can drift without one failing — the shape that would have caught `synthesize.ts:45`, and the replacement for `83f192c`'s hand-kept lockstep plus a comment asking editors not to change the words.

Regenerate with `UPDATE_VOCABULARY=1 cargo test -p gaply_core mirror`.

### 31.19 The refactor justified itself on first compile

**`report_with_sentinel` — the report fixture nine `reviewer_agent` tests share — carried `"agent": "Plagiarism"`. That is not a valid `AgentKind`; the serialized form is `"plagiarism"`.** Untyped JSON accepted it for the life of the test.

> **Data that could never come from production, sitting in the codebase's own tests, caught the moment parsing became strict — before the refactor touched a single real report.**

#### Does the test READ that field? No — and both halves of the answer matter

**Checked rather than assumed.** The nine tests assert on sentinel absence (privacy), `similarity:` provenance, supplementary bounds, injection safety, `run_id`, the summary key set, and digest behaviour. **None asserts the agent's VALUE.** `summary_shape_is_pinned_to_the_format_version` asserts that `agent` is among the keys (`:1869-1872`), not what it contains.

**So correcting the fixture changes what the test PARSES, not what it TESTS.**

**One consequence that is not inert:** the payload's `summary.findings[0].agent` now serializes as `"plagiarism"` rather than passing `"Plagiarism"` through, which changes `summary_digest`'s input. **No test pins a literal digest** — a repo-wide search for a 64-hex literal returns nothing — so nothing breaks, and the digest tests compare digests to each other rather than to a constant.

**The precise statement: inert for every assertion, not inert for the serialized bytes, and nothing depended on those bytes.**

### 31.20 A tooling pattern, recorded on its second instance

**`git checkout -- <file>` is not an undo for a mutation. It is an undo for the WORKING TREE.**

During this refactor a mutation test was reverted with `git checkout --` on the assumption that the file was *"clean apart from the mutation"*. **It was not** — it carried the typed `build_review_payload`, `review_manuscript`, and two fixtures. All were discarded. Caught by a `grep` for the typed signature, and redone; the next mutation used a backup copy.

**This is the SAME SHAPE as the `cd src-tauri` failure recorded earlier in this work:**

| | Command | Did | Was wanted |
|---|---|---|---|
| 1 | `cd src-tauri && python3 …` | `cd` failed, the patch never applied, **exit 0** | apply a patch |
| 2 | `git checkout -- file` | reverted the file to HEAD, **exit 0** | revert one edit |

> **Both succeeded at what they do while doing something other than what was wanted, and both were caught by an EFFECT CHECK rather than by an exit code.**

**Two instances is a pattern rather than two incidents.** The standing practice it argues for: **after any destructive or path-dependent command, assert the effect** — grep for the string that should be present, or `git status` for the file that should still be modified. An exit code reports whether the command ran, never whether it did the intended thing.

#### THIRD INSTANCE — and the strongest form of the argument

**One commit after recording the rule above, the same session broke it.** A `str.replace` patch adding `textTransform` to `ReviewerLetterPanel.tsx` did not match the real indentation, changed nothing, and reported *"header uppercases in CSS"*. **Caught by a failing test, not by the author.**

> **§21's level-1 limit, demonstrated on the person who wrote the rule.** A rule that lives in prose does not stop the contributor who wrote it one commit earlier — which is the whole argument for level 3, made against the strongest possible case.

#### THE MECHANICAL FIX — because the practice cannot be remembered

```python
# silent when the pattern is absent
src = src.replace(old, new)

# fails when the pattern is absent
assert old in src, f"pattern not found: {old[:60]}"
src = src.replace(old, new)
```

**`str.replace` returning the input unchanged is precisely the shape §31.20 names** — an operation succeeding at what it does while doing nothing that was wanted.

**The `assert` converts a remembered check into a failure**, which is the level-1-to-level-3 move this project has now made three times: the `SUMMARY_FORMAT_VERSION` pin, the compatibility fixtures, and this.

**Adopted for every patch from here.** Same principle in shell: **after any destructive or path-dependent command, assert the effect rather than trusting exit 0.**

### 31.22 `MAX_FINDINGS` in the PDF — NO CAP, and the position stated rather than left implicit

**The constant answers the question itself**, in `gaply-core/src/reviewer_agent.rs:54`:

```rust
/// Max findings forwarded (report is severity-ordered, so this is the top-N).
/// Keeps the payload comfortably under the proxy's 8000-char total cap.
pub const MAX_FINDINGS: usize = 12;
```

> **It exists for the proxy's character budget. A PDF has no character budget.**

**No UX argument survives contact with *"why is finding 13 missing?"*** once the technical reason is gone — and run 23 produced 20 findings, so this is measured rather than hypothetical.

#### THE POSITION: do not reintroduce a limit without an evidence-based requirement

**Not "no cap for now".** A cap is a decision to withhold findings from the person who asked for them, and it needs a reason of its own — an observed readability failure, not an assumption of one.

| Findings | Behaviour |
|---|---|
| **~20** | straightforward — a handful of pages, read start to finish |
| **~50** | acceptable with severity grouping, which the composer already emits |
| **~200** | a long appendix. Genuinely unwieldy, and still complete |

**If readability ever does become a real concern, the answer is STRUCTURE, not TRUNCATION** — a grouped appendix here, collapsible sections in an HTML renderer. Both preserve completeness; a cap destroys it, and destroys it silently at exactly the manuscript that needed the report most.

> **The report's job is to be COMPLETE. The reviewer payload's job is to FIT A BUDGET. Those are different jobs, and one constant should not have been serving both.**

**The middle option — summarising long severity groups — was rejected** because it reintroduces a composer decision about what a reader may skip, which is the class of judgement this whole workstream has been removing.

**Pinned by test:** `every_finding_reaches_the_page_regardless_of_count` renders 40 findings and asserts all 40 appear in the extracted PDF text. Mutation-verified — reintroducing a 12-cap in the composer fails it.

### 31.23 Milestone 3 — the renderer, and three corrections it forced

#### The three layers, as a contract

| Layer | Module | Question | Forbidden |
|---|---|---|---|
| **ENGINE** | `report_model.rs` | what exists? | naming a page, font, column |
| **COMPOSER** | `report_compose.rs` | how should a researcher read this? | computing any manuscript fact |
| **RENDERER** | `report_pdf.rs` | how do these become pages? | any decision — ordering, filtering, ranking, truncation |

> **The ENGINE may not name a page, the COMPOSER may not compute a fact, the RENDERER may not make a decision.**

**Each prohibition is structural, not advisory.** `compose(&LocalReportModel) -> Vec<Block>` holds no `Database` and nothing to compute a fact *from*; `render_pdf(&[Block]) -> Vec<u8>` receives no severity to re-rank and no finding it could drop. **If the composer needs a number the model lacks, the model is wrong.**

#### TWO DEVIATIONS from the approved design, both tightening it

| Proposed | Built | Why |
|---|---|---|
| `Block::FindingGroup`, `Block::ChecklistTable` | **flat blocks only** — `Cover`, `Heading`, `Paragraph`, `Bullet`, `Note`, `PageBreak` | A `FindingGroup` hands the renderer *"what does a finding look like?"* — a presentation decision made below the composer. The composer now expands groups itself |
| `render_pdf` in the app crate ("it is I/O") | **in `gaply_core`** | It is not I/O. `&[Block] -> Vec<u8>` is pure — no filesystem, clock or network. It also sits beside `pdf-extract`, which the instrument needs. The real I/O stays in the app crate |

#### The threading — PR-4's precedent, applied a second time

`run_pipeline_inner` widened from `()` to `LaneExamination` in PR-4; it now returns `PipelineResult`, carrying the two sources §31.2 found unreachable.

| Source | Contributes | Lost without it |
|---|---|---|
| `ExtractionResult` | title, table/reference counts, **statistics for the Reported/Missing block**, section prose | the report's most actionable block |
| `PlagiarismReport` | similarity regions, `corpus_chunks_available` | the similarity section, **and the ability to say "nothing to compare against" instead of "no matches"** |

**Rejected and recorded so they are not re-proposed:** a callback inverts control for what is simply a return value; a global adds shared mutable state to a pure pipeline; re-reading from the database can disagree with what the run computed.

#### The folding rule — three outcomes, two disclosures

| | Outcome | Disclosure |
|---|---|---|
| `Müller` | **INTACT** — byte-exact through WinAnsi | none |
| `Śarmā` → `Sarma` | **SIMPLIFIED** — altered and readable | `NOTE_SIMPLIFIED` |
| `हिन्दी` → `??????` | **MARKED** — absent and visible | `NOTE_MARKED` |

**One note covering the last two would blur different facts.** A reader judging whether to trust a name needs the first specifically — *"some characters could not be displayed"* gives them no reason to doubt a name sitting right there on the page, spelled wrongly.

**`NOTE_MARKED` states no count.** The marks are per code point, which is not the unit a reader counts: `हिन्दी` is six code points and three visual clusters, so any length claim asserts a character count nobody would recognise. Grapheme segmentation would fix it and needs `unicode-segmentation`, which reaches `gaply_core` through no path today — **every route to that crate in the workspace runs through the app crate.** The marks show THAT something is missing, not how much.

#### A TABLE beats NFD, on the case that motivated the rule

The obvious fold decomposes with NFD and drops combining marks. **`Ł` has no decomposition**, so NFD leaves `Łukasz` as `?ukasz` — the exact outcome the rule exists to prevent. A 60-line table maps `Ł → L` directly, needs no new dependency, and covers Latin Extended-A plus the Romanian comma-below letters. **Latin Extended Additional (Vietnamese) is a known gap** and falls through to `MARKED`.

#### CORRECTION 3 — a disclosure cannot use an unrenderable example

`NOTE_SIMPLIFIED` first read *"for example, a name written Śarmā appears here as Sarma."*

> **`Śarmā` is precisely what the renderer cannot represent. The sentence rendered as "a name written Sarma appears here as Sarma" — a note explaining an alteration, silently altered, into nonsense.**

**Caught by a `debug_assert` that the disclosure strings are themselves ASCII**, on the instrument's first run. The wording now names the CLASS of change rather than showing an instance: **showing one is impossible in the medium doing the showing.**

**This is §4.20 turned on the disclosure itself** — the mechanism meant to reveal silent alteration, silently altered. Worth naming because the failure is invisible by construction: the note still reads as a sentence.

#### The instrument, and its oracle verified first

`LocalReportModel → compose() → render_pdf() → PDF bytes → pdf-extract → assert`. **The assertion is on the extracted text, never on an intermediate structure** — an assertion on `Finding.title` passes while the PDF says `arm`.

**Before the instrument was allowed to trust `pdf-extract`, a hand-written 666-byte WinAnsi PDF was round-tripped through it:**

| Byte | Expected | Returned |
|---|---|---|
| `0xFC` | ü | `U+00FC` |
| `0xE9` | é | `U+00E9` |
| **`0x8A`** | **Š** | **`U+0160`** |
| **`0x9E`** | **ž** | **`U+017E`** |

**The last two are the ones that mattered.** They live in CP1252's `0x80`–`0x9F` block, which ISO-8859-1 defines as control characters — a reader assuming Latin-1 would have returned control codes, and the failure would have been debugged as a renderer bug while the renderer was correct.

**Mutation-verified, four mutations, four kills:** `latin_base` never folds · Latin-1 not identity-mapped · disclosures always emitted · a 12-cap reintroduced in the composer.

#### What the milestone actually cost, in corrections

**Three claims in this document were false when Milestone 3 began**, and all three were found by checking rather than by failure:

1. **§31.17** — *"tier 1 does not survive the port"*. WinAnsi covers Latin-1, not Latin.
2. **The design proposal** — the instrument would prove `Śarmā` appears as `Śarmā`. Impossible in base-14.
3. **`NOTE_SIMPLIFIED`** — a disclosure whose example the disclosure's own subject destroys.

> **Each was cheap now and expensive later, and none would have announced itself.** The first ships `?arm?` via an instruction to delete working code; the second fails on first run and reads as a renderer bug; the third renders as a grammatical sentence that means nothing.

### 31.24 The standing suite never compiled the examples — found by widening a return type

**Widening `run_pipeline_inner`'s return broke `examples/mem_probe.rs`, which is expected.** Running `cargo check --all-targets` to confirm the fix surfaced **two failures that had nothing to do with this work**:

| Example | Broken by | Since |
|---|---|---|
| `release_gate.rs` | `build_review_payload` became typed | **§26 PR-3** |
| `publishready_mem_probe.rs` | same | **§26 PR-3** |

**Verified against `HEAD` rather than assumed:** `git show HEAD:…/reviewer_agent.rs` already declares `report: &crate::report::PublishReadyReport`, and `git show HEAD:…/release_gate.rs` still passes a `serde_json::Value`. **Both were already broken before this milestone began.**

#### Why nobody noticed

> **The project's standing "full suite" is `cargo test -p gaply_core` plus `cargo test --lib`. NEITHER COMPILES EXAMPLES.** Windows CI runs `cargo test -p gaply_core`, which does not either.

**A typed refactor that the compiler was supposed to police went unpoliced in exactly the directory that holds the release-gate tooling.** §26 PR-3's whole argument was that types make a class of error impossible — and they did, for every caller the build actually looked at.

#### And underneath it, a SECOND defect the compiler could never have caught

`report_cache_key` carries the comment *"ONE definition, so writer and readers cannot drift apart."* **Three examples hand-built the key instead**, and all three had drifted:

| Site | Built | Actual |
|---|---|---|
| `release_gate.rs` | `report:v2:{id}` | `report:v2:e{N}:{id}` |
| `read_report.rs` | `report:v2:{id}` | `report:v2:e{N}:{id}` |
| `publishready_mem_probe.rs` | `report:{id}` | `report:v2:e{N}:{id}` |

**These compile perfectly.** They fail at RUNTIME, on `.expect("report")`, because the lookup misses — and they are the tools used to gate a release. The last one is two schema bumps stale.

**Cause: `report_cache_key` was `pub(crate)`,** so examples — which are separate crates — could not call the one definition even if they wanted to. **A drift-prevention mechanism unreachable to half its readers prevents nothing.** Now `pub`, and all three call it.

#### THE GENERAL LESSON, in its strongest form

> **A VERIFICATION TOOL THAT SILENTLY STOPS BEING EXECUTABLE IS NO LONGER A VERIFICATION TOOL.**

**This is stronger than §21's level-1-versus-level-2 distinction, and it has to be.** Level 1 says a check holds only while someone remembers to run it — the failure mode is a check not run, and everyone knows it was not run. **Here the failure mode is worse: a tool PRESENT IN THE TREE implies it works.** Its file is there, its name says what it guarantees, and every reader — including this document — treats its existence as coverage. **Absence would have been noticed. Silent non-execution was not.**

#### What specifically was not running

**`release_gate.rs` asserts PRIVACY, PROVENANCE and SELECTION** — it drives a real pipeline, builds the real reviewer payload, and checks that no `detail` string from the compiled report crosses the wire.

> **It has not compiled since §26 PR-3 — through the entire F2/F6 arc.**

That arc is precisely where the payload contract was designed, argued, decided (§26.10, contract 1) and implemented across four PRs. **The tool whose job was to check the payload's privacy properties was non-executable for all of it.**

**The baseline, stated precisely:** *the baseline was frozen using the harness protocol rather than the release gate, because the gate had silently become non-executable.* **§30's freeze did not depend on the gate.** That is a different claim from the freeze having been compromised by the gate's absence, and this record must not be read as the second — the harness protocol is what §30 ran, and what it measured stands.

#### THE CAUSE IS THE DURABLE PART

`report_cache_key` was **`pub(crate)`**. Examples are separate crates, so they could not call it. **Three of them hand-built the key instead, and all three drifted** — `release_gate.rs` and `read_report.rs` by one schema version, `publishready_mem_probe.rs` by two.

> **`pub(crate)` prevented legitimate reuse, duplication followed, and the duplication drifted.**

**Same family as `matchTypeLabel` and `synthesize.ts:45`, different root cause.** Those were oversights — a second copy written because nobody noticed the first. **This one was MANUFACTURED by an access-control decision:** the copies were not careless, they were the only option available to a correct author.

> **A visibility modifier on a value that examples or tests legitimately need is a DUPLICATION GENERATOR.**

**That is the checkable form.** Before narrowing visibility, ask who legitimately needs the value — and note that examples and integration tests are OUTSIDE the crate, so `pub(crate)` excludes them by construction. **A drift-prevention mechanism unreachable to half its readers prevents nothing; it converts a shared definition into a private one plus copies.**

#### And the §21 form

The instrumentation gap here is not a missing test, it is a **missing TARGET**. `--all-targets` should be part of what "full suite" means; until it is, `examples/` is a directory the compiler is never asked about. **Recorded as an observation with a named fix rather than adopted unilaterally** — CI is manual-dispatch-only (§21), so what the standing local command is remains a decision to be made, not assumed.

### 31.25 An approval error worth recording — a METHOD was approved where a PROPERTY was meant

**The character-folding decision was approved as *"(b) NFD base-letter fallback"*. The approval was not wrong about the goal**; the reasoning recorded alongside it states the goal exactly — *"`?ukasz` is unusable while `Lukasz` is wrong but readable, and for a name recognisability is the axis that matters to its owner."*

> **What was approved was an ALGORITHM. What was meant was a PROPERTY: preserve the recognisability of Latin-script names wherever possible.**

**`Ł` is where the two diverge.** It carries no combining mark and has no NFD decomposition, so pure NFD leaves it untouched and `Łukasz` still renders `?ukasz` — **the algorithm named in the approval fails the very case the approval's own reasoning used to justify it.** A lookup table satisfies the property; NFD does not.

#### The general form

> **Approving a method is weaker than approving the property the method must satisfy, because ONLY THE PROPERTY CAN BE CHECKED AGAINST A CASE.**

**A method can only be checked against itself.** *"Did we implement NFD correctly?"* has a yes/no answer that stays yes while `Łukasz` renders `?ukasz`. *"Is this name recognisable to its owner?"* fails immediately on the same input. **The property admits counter-examples; the method does not.**

**This is why the implementation is pinned by a test named for the PROPERTY** — `folding_alone_discloses_only_simplification` asserts `Lukasz`, with the comment *"the case NFD cannot handle"*. **If the table is ever replaced by "real" NFD as a simplification, that test fails.**

**Related but distinct from §4.17** (*assert the invariant, not the implementation*), which governs how a test is written once the requirement is known. **This one is upstream of that: it governs how the requirement is STATED at approval time.** A method approved as a requirement produces tests that assert the method, and §4.17's failure follows from it rather than causing it.

### 31.26 VERIFY THE PREMISE BEFORE OPTIMIZING THE SOLUTION — four instances in one milestone

**Four recommendations in Milestone 3 were CHANGED by measuring their premise.** Each had been reasoned to confidently, and each was wrong in the same direction.

| # | Premise assumed | Measured | Recommendation before → after |
|---|---|---|---|
| 1 | `unicode-segmentation` is "already in the tree" (it is in `Cargo.lock`) | **every path runs through the APP crate**; `gaply_core` has none | grapheme sentinels → **drop the length claim** |
| 2 | `pdf-extract` may not decode WinAnsi; the instrument might need another reader | round-trips `0x8A→Š`, `0x9E→ž` — CP1252, not ISO-8859-1 | evaluate readers → **use it, verified** |
| 3 | The examples gap needs `cargo check --all-targets` promoted | **plain `cargo test` already builds examples**; the gap is the `--lib` flag | add a command → **remove a flag** |
| 4 | So removing `--lib` is the cheap fix | **`cargo test` costs +108s per edit** (22 example binaries link candle/tauri); `check --all-targets` costs **+3s** | remove a flag → **`check --all-targets && test --lib`** |

#### What makes this worth recording rather than filing under "good practice"

> **In every case the measurement made the design SIMPLER. No new dependency, no different oracle, no extra command, no two-minute suite.**

**That is the asymmetry.** An unverified premise is nearly always a premise about a CONSTRAINT — *this crate is unavailable, this library is unreliable, this command does not cover that target, this fix must be slow.* **Machinery is then designed to work around the constraint.** When the constraint turns out not to exist, the machinery has no purpose left, and the simpler design was available from the start.

**The failure is invisible while it happens.** Adding a dependency, an oracle or a command all look like diligence. **Nothing about the resulting design announces that it is solving a problem that was not there** — it just looks slightly heavier than it needed to be, which is indistinguishable from thoroughness.

#### Instance 4 is the sharpest, because measuring reversed a measured conclusion

**Instance 3 was itself a measurement** — an example was deliberately broken and three commands were compared, showing `cargo test` catches it and `cargo test --lib` does not. **That measurement was correct and its recommendation was still wrong**, because it established COVERAGE without establishing COST.

> **A measurement answers the question it was asked. "Does this command catch the error?" is not "should this be the standing command?"**

**The second measurement (`--all-targets` at 3s versus `cargo test` at 133s) is what settled it.** The 108-second gap is entirely codegen and linking for 22 example binaries — work `check` skips while still type-checking every target.

#### The standing-command numbers

Measured after `touch src/lib.rs` (the realistic edit path), then twice warm:

| Command | After edit | Warm | Tests run | Examples compiled |
|---|---|---|---|---|
| `cargo test --lib` *(today)* | **24.6s** | 2.8s | 177 | **no** |
| `cargo test --tests` | 54.5s | 2.6s | 182 | **no** |
| `cargo test` | **132.7s** | 3.0s | 182 | yes |
| `cargo check --all-targets` | **3.0s** | 0.3s | 0 | yes |
| **`cargo check --all-targets && cargo test --lib`** | **27.6s** | 3.1s | 177 | **yes** |

> **CORRECTED BY §37 — the "Examples compiled" column DOES NOT SAY WHOSE.** It means the APP package's examples. Run from `src-tauri`, `cargo check --all-targets` checks the app package's targets and `gaply_core` only as a LIB DEPENDENCY — **it cannot see `gaply_core`'s tests or examples at all.** Measured: a deliberate type error in a `gaply_core` test PASSES this command.

> **~~+3 seconds, +12%, one command line — and the class of defect that hid `release_gate.rs`'s non-compilation for four PRs becomes impossible.~~ SUPERSEDED BY §37.** True for the app package, false for the workspace. **`release_gate.rs` is an app example, which is exactly why this command caught it — and why the defect looked closed when it was not.**

#### THE SHARPEST FORM OF THE ERROR

**§34.5's scope-erasure rule — *"a count must carry its scope; '3 sites' is not a fact, '3 sites in `gaply_core`' is"* — was derived PARTLY FROM THE TABLE ABOVE.**

> **The rule was correct, and its own evidence was an instance of what it warned about.** A column reading *"Examples compiled: yes"* is a count without its scope, sitting in the section that named the failure.

**It then produced a wrong Step 2 estimate TWICE, IN OPPOSITE DIRECTIONS**, from two different scope-limited commands: `-p gaply_core --all-targets` reported 5 sites, `--all-targets` reported 1, **and neither was the union, which is 6.**

**Warm cost is identical across all of them (~3s), so the only figure that discriminates is the edit path** — which is the path the command is actually run on.

**The decision rule's second branch does not fire.** The overhead is not substantial, so nothing needs to be documented as a cost to be tolerated: **a standing command avoided because it is slow is a routine that is not run**, and 3 seconds does not produce avoidance where 24 did not.

#### A SEPARATE FAILURE — the overturn was RECORDED but never REPORTED

**The table above was written into this document in the same turn the measurement ran. It was not stated in the reply.** That turn's response led with the release gate's matrix and never mentioned the timings, the +108s, or the fact that recommendation (a) — *"drop `--lib`"* — had just been overturned.

> **For two turns the RECORD held the measured answer while the CONVERSATION still carried the reasoned one.** The discrepancy surfaced only because a later commit message quoted the corrected command and the reader noticed it conflicted with a recommendation nobody had retracted.

**This is not the verify-the-premise failure; the premise was verified.** It is a reporting failure, and it is worse in one specific way: **a correction that exists only in a document nobody has re-read has not yet corrected anything.** The reader was still reasoning from the superseded recommendation, and had to reconstruct the 108-second figure independently to challenge it.

**The rule:** when a measurement OVERTURNS a recommendation already given, the reversal is the headline of that turn's report, not a row in a table written the same hour. **Writing it down is necessary and is not the same as saying it.**

## 32. The release gate's first execution since PR-3 — a measurement

**Run against `gaply-core/tests/fixtures/manuscript.txt`** (260 words, committed, so the run is reproducible and no private document was involved). **11 report findings, 11 payload findings, 11 detail strings checked.**

### 32.1 `ship_ready: false` — and it has never been capable of anything else

> **The headline is not "4 PASS, 0 FAIL, 2 SKIPPED". The headline is that `ship_ready` is FALSE BY CONSTRUCTION.**

`ship_ready()` requires `fail == 0 && skipped == 0`. **Two invariants always skip**, for reasons that have nothing to do with the run:

```rust
gate.record(COMPARISON,  check_comparison(None));   // examples/release_gate.rs:62
gate.record(PERSISTENCE, check_persistence(None));  // examples/release_gate.rs:63
```

**The arguments are literal `None`, and the runner drives `run_pipeline_measured`, not `run_publishready`** — so the comparison record those two invariants read is never produced, on any run, in any environment.

> **`ship_ready` has never returned `true` and, as the runner is written, never can.**

**The skip REASONS are honest** — *"the full command path did not run"* is exactly correct. **The verdict computed from them is not**, because it presents a permanent structural fact as a per-run outcome.

### 32.2 The project already misread this, and the misreading has a name

**§21 recorded the gate's expected output as an observation about the SYSTEM.** It was an observation about the RUNNER.

| | §21 | Measured |
|---|---|---|
| Counts | *3 PASS, 3 SKIPPED* (predicted) | **4 PASS, 2 SKIPPED** |
| Why the skips | *"a proxy-free run cannot answer"* | **the runner passes `None`; the proxy is irrelevant** |
| `ship_ready` false | a property of that run | **a property of the tool** |

**Two ontology rules apply, and they are different failures:**

**§4.20's PRESENTATION class, applied to an INSTRUMENT.** `ship_ready` is a field implying a computation that cannot occur — the same shape as `publication_probability` rendered as a percentage gauge over a four-value lookup. **A boolean named "ship_ready" asserts that shipping-readiness was evaluated.** It was not; two of its six inputs were never obtainable.

**§24's "unexamined, not unstated" shape.** **Nobody decided `ship_ready` would be permanently false.** Nobody wrote it down, argued for it, or accepted it as a cost. **And nobody examined whether it could be otherwise** — the field was read as a verdict for as long as the gate went unrun, which is to say for its entire life since PR-3.

> **An instrument can carry the same defect it was built to detect, and it is harder to see there, because the instrument is what you would normally check WITH.**

### 32.3 The coverage claim, restated

**The gate is recorded throughout this document as SIX invariants.** What it actually is:

| | Count | |
|---|---|---|
| **Real coverage** | **4** | PRIVACY, PROVENANCE, SELECTION, and LIVENESS as a meta-check |
| **Unreachable from this runner** | **2** | COMPARISON, PERSISTENCE |
| **Summary that can vary** | **0** | `ship_ready` is constant |

**LIVENESS is a meta-invariant** — it checks the REPORT, not the product, asserting that no proxy-dependent invariant claims Pass without a proxy. **It currently guards two invariants that cannot run**, so it passes in its cheapest possible configuration with nothing to catch. It is correct and it is nearly free of content on this input.

**§21 promoted the gate as level-3 self-detecting.** **That is true of the four and false of what the six-invariant framing implies.** The correction is to the coverage claim, not to the gate's value — four end-to-end invariants asserted against real bytes is a real instrument.

### 32.4 The genuinely new result — an ARGUMENT became a MEASUREMENT

**PRIVACY and PROVENANCE passing is a first measurement, not a re-confirmation.** Both ran against a system substantially changed since they last executed: F2/F6's four PRs, `build_review_payload`'s typed `&PublishReadyReport` boundary, `ClaimKind` on every `Finding`, the vocabulary, and `enrich_report_labels`.

**§31.21 ARGUED that labels could not leak into the payload**, on the grounds that the payload is an explicit seven-key projection and `summary_shape_is_pinned_to_the_format_version` guards the key set. **The reasoning was sound and it was reasoning.**

> **The gate now confirms it against SERIALIZED BYTES: no payload finding carries a `detail` field, and none of the 11 `detail` strings appears anywhere in the payload's serialization.**

**This is the move the project keeps setting up and rarely completes** — §4.17's *assert the invariant, not the implementation*, reaching the artifact rather than stopping at the argument. **It is worth naming because the argument was CORRECT.** The value was never in catching an error; it was in converting a claim that had to be trusted into one that is checked.

### 32.5 Two smaller observations

**SELECTION passed at 11 findings against a cap of 12.** **Not vacuous** — the check compared and did not fire. But **the invariant's claim is that the cap holds UNDER LOAD, and that went untested at the narrowest possible margin.** Run 23 produced **20 findings** on a real manuscript; this fixture produces 11. **The one input that would exercise the invariant is the one the reproducible fixture cannot supply.**

**LIVENESS passed with nothing to catch**, as above — recorded so that a future green LIVENESS is not read as evidence that the implicit-pass hazard was tested.

### 32.6 The fourth unchecked premise came from the TASK, not the work

**The task framing stated:** *"The runner would have to drive the full command path, which needs a proxy, entitlement, and a signed-in user — that is why it drives the pipeline instead."*

**Checked, and false.** `commands.rs:724` carries an explicit comment: **"UNCONDITIONAL. Previously this whole block sat inside `if let Some(outcome) = &shadow_outcome`"** — the Box 4 harness block runs on every call. The reviewer degrades to `ReviewerEvaluation::unavailable_offline()` on an unreachable proxy, a 401 and a 403 alike. **None of the three stated prerequisites is one.**

> **This is §31.26's fourth instance, and the first where the unchecked premise came from the person SETTING the task rather than being caught inside the work.**

#### The framing biases the answer even when the question is open

The same message asked to *"state honestly whether a runnable version is feasible at all, or whether those two invariants belong somewhere other than this runner."* **That question is genuinely open. The sentence before it was not, and it did the damage.**

**The first response proposed Option B — move the invariants elsewhere — and reasoned to it from the stated blocker.** Option B is coherent, defensible, and solves a problem that does not exist. **Asking for an honest assessment does not undo presenting the constraint as established**, because the assessment is then performed *within* the constraint rather than *on* it.

**The general form, which is the durable part:** a premise stated as fact by the requester is the hardest kind to check, because checking it reads as doubting the request rather than doing it. **That is exactly when it most needs checking** — the four instances in this milestone cost minutes each, and this one would have redirected an entire piece of work.

### 32.7 The COMPARISON caveat was withdrawn before it was recorded

**Claimed:** with no proxy the wholesale side is `unavailable_offline`, so `wholesale_findings_sent` may be `Unavailable` and `check_comparison` would return SKIPPED — "five and a half of six".

**Falsified by run 23's own record**, captured with the proxy returning 403 throughout:

```
wholesale_path_available   false | status: observed
wholesale_findings_sent    12    | source: deterministic_local | status: observed
shadow_findings_sent       12    | source: deterministic_local | status: observed
```

**And confirmed in the source rather than inferred from the capture:**

```rust
wholesale_findings_sent: Metric::observed(inp.wholesale_findings_sent, DeterministicLocal),   // unconditional
shadow_findings_sent:    sh.map(|s| Metric::observed(s.findings_sent, DeterministicLocal)),   // iff the shadow ran — local
```

> **Both counts are the number of findings that WOULD be sent, computed locally. Neither is read from a response.** `wholesale_path_available: false` and `wholesale_findings_sent: 12 (observed)` are consistent, and the pair is the clearest example in the record of why §26's Metric type distinguishes SOURCE from AVAILABILITY.

**A fifth instance of the same pattern, caught before it reached the document.**

### 32.8 Option A implemented — six of six, for the first time

**`run_publishready_measured` extracted** from `run_publishready`'s `spawn_blocking` body; the command now calls it. **Not a new pattern — `pipeline::run_pipeline_measured` is the same seam for the same reason**, and the command path simply never got one. The runner additionally calls `harness_log::set_dir`, without which `append` is a documented silent no-op.

```
  PASS  PRIVACY      PASS  COMPARISON
  PASS  PROVENANCE   PASS  PERSISTENCE
  PASS  SELECTION    PASS  LIVENESS
  6 PASS, 0 FAIL, 0 SKIPPED
  no_failures: true
  coverage:    6 of 6 invariants evaluated
```

**COMPARISON and PERSISTENCE have never reported anything before.** This is not a passing run of a known-good check; it is the first data either has produced.

### 32.9 `ship_ready` replaced by two values

`no_failures() -> bool` and `coverage() -> String`. **The reasoning lives on `GateReport`'s own docs, not only here.**

> **The defect was never that `ship_ready` was always false. It is that ONE BOOLEAN had to answer TWO INDEPENDENT QUESTIONS — "did anything fail?" and "was everything checked?"**

**Computing the boolean differently was CONSIDERED AND REJECTED as the worst option available.** Excluding skips (`fail == 0` alone) would have returned **`true`** on the very run that exposed all of this — a run where a third of the invariants never executed. **It converts a visible structural gap into a green light.**

**`GateOutcome::Skipped` already carried its reason as typed absence (§4.12); the SUMMARY was discarding it. The fix is to stop collapsing, not to collapse differently** — `coverage()` now names each skipped invariant with its reason.

#### AND `coverage()` INHERITS THE SAME DEFECT, ONE LEVEL DOWN

**The new summary reads `6 of 6 invariants evaluated`. That is true, and it overstates.**

> **LIVENESS was EVALUATED and checked nothing** — it returned Pass on its first line because `proxy_available` was true. **"Evaluated" is not "exercised", and `coverage()` counts the first while reading as the second.**

**`ship_ready` conflated "did anything fail?" with "was everything checked?". `coverage()` separates those two and then conflates "was it checked?" with "did the check have content?"** — the same collapse, one level further in, introduced by the fix for the original.

**Not repaired here, and recorded rather than left to be rediscovered**, because the repair is the same decision as LIVENESS's: a check that cannot fail on a given input is not distinguishable from one that passed, and expressing that distinction needs the gate to know when an invariant was VACUOUS. **§32.11's SELECTION observation is the same class** — it passed at 11 against a cap of 12, evaluated and barely exercised.

**The honest reading of today's output is therefore: 6 evaluated, 4 with content, 2 vacuous** (LIVENESS entirely, SELECTION at the margin). **No field says that.**

### 32.10 LIVENESS has now passed THREE TIMES, never once by doing its job

| Run | Why it passed |
|---|---|
| §21 (predicted) | expected to SKIP |
| §32 first execution | COMPARISON and PERSISTENCE were SKIPPED — nothing to catch |
| §32.8, after Option A | **`proxy_available` was TRUE, so it returned Pass on line 1** |

#### `proxy_available` measures CONFIGURATION, not reachability

```rust
let proxy_available = ProxyReqwestClient::from_env().is_ok();
```

`from_env` reads `GAPLY_PROXY_URL` (defaulting when unset) and `TokenSigner::from_keychain()`. **It succeeds when a signing key exists in the keychain. It never touches the network.** The production path uses `client.reachable()`, which probes `/health` — a different question, asked with a different call.

> **A Tailscale-hidden, completely unreachable proxy reports `proxy_available: true`** on any machine that has ever signed in. **§4.20's PRESENTATION class again, and again inside the instrument: a name asserting a measurement that was not performed.**

**The pass is DEDUCIBLE, not assumed:** `check_liveness` returns `Fail` when COMPARISON or PERSISTENCE report Pass and `proxy_available` is false. Both reported Pass and LIVENESS did not fail, therefore `proxy_available` was true.

#### And Option A introduced a LATENT SPURIOUS FAILURE — recorded, not fixed

**On a machine with no keychain entry, `proxy_available` is false.** COMPARISON and PERSISTENCE still pass, because they are `DeterministicLocal` and need no proxy. **`check_liveness` would then FAIL the run** for an implicit pass that did not occur.

> **LIVENESS's premise is now false: it classifies COMPARISON and PERSISTENCE as PROXY-DEPENDENT, and §32.7 established that neither is.**

**The invariant was correct when written** — before Option A those two could only come from a path that needed the command, and the proxy was assumed to be part of it. **Making them reachable offline falsified the classification rather than the check.** The fix is to LIVENESS's invariant list or to what `proxy_available` measures, and it is deliberately not made here.

### 32.11 Still open, recorded not fixed

**SELECTION passed at 11 findings against `MAX_FINDINGS` 12.** The check compared and did not fire, so it is not vacuous — **but the invariant's claim is that the cap holds UNDER LOAD, and that remains untested at the narrowest possible margin.** Run 23 produced **20 findings** on a real manuscript; this fixture produces 11.

**A fixture that exercises the cap is worth considering and is NOT changed here.** Two shapes, neither built: a second committed fixture long enough to exceed 12 findings, or a unit-level assertion over a synthetic 20-finding report — **which `check_selection` already has**, meaning the gap is specifically in the END-TO-END path, where a real payload is built from a real manuscript. **The second option does not close it; only the first does.**

### 32.12 The shape of the LIVENESS repair — three concepts in one value, NOT STARTED

**`proxy_available` mixes three questions that must be separated before anything is repaired.** Recording the framing, because the repair follows from it and is a DECISION rather than a typo.

| Concept | Question | What measures it today |
|---|---|---|
| **CONFIGURED** | do credentials and a URL exist? | **`from_env().is_ok()`** — what the value actually holds |
| **REACHABLE** | does the proxy respond? | **`reachable()`**, which probes `/health` — what the check NEEDS |
| **REQUIRED** | does this invariant depend on the proxy at all? | **LIVENESS's hard-coded list** `COMPARISON \| PERSISTENCE` |

#### Two separate errors, in one value

> **(1) The check reads CONFIGURED while meaning REACHABLE.** A Tailscale-hidden proxy that answers nothing reports `true` on any machine that has ever signed in, and LIVENESS disables itself. **ONTOLOGY §4.16's fifth instance, and its first inside an instrument.**

> **(2) The REQUIRED list is stale.** COMPARISON and PERSISTENCE were classified proxy-dependent; §32.7 established both counts are `DeterministicLocal` and `Observed` offline. **The classification was true when written and was falsified by Option A**, not by any change to the check.

**They compound in opposite directions, which is why the gate looks green.** Error (1) makes LIVENESS pass when it should test; error (2) means that if error (1) were fixed alone, LIVENESS would FAIL every offline run for an implicit pass that cannot occur. **Fixing either one in isolation makes the gate wrong in a new way** — which is the whole reason this is recorded as a decision and not attempted.

#### What a repair must decide, not assume

1. **Does LIVENESS take REACHABLE as its input?** If so, `reachable()` costs a network probe on every gate run, including offline ones.
2. **Is REQUIRED still a fixed list, or does it come from the record?** Every `Metric` already carries `MetricSource` and `MetricAvailability` — **the record can say whether a value needed the proxy, so the list may not need to exist at all.**
3. **What should LIVENESS assert once COMPARISON and PERSISTENCE are local?** It may have no proxy-dependent invariant left to guard, in which case it is either retired or repointed at whatever genuinely is one.

**Question 2 is the one worth answering first**: a hard-coded list of "which invariants need the proxy" is derived state duplicating what `MetricAvailability` already records — **§11.5's shape, and the reason this went stale silently.**

## 33. LIVENESS deleted — the property moved into the data model

### 33.1 The claim, stated narrowly

> **For the current six invariants, LIVENESS is redundant, because none of them consume proxy-dependent metrics.**

**Deliberately not "a redundant copy of the data model".** That overclaims: it would leave no room for a future invariant that genuinely does depend on live proxy data, and such an invariant is entirely plausible — anything reading `wholesale_publication_probability` or `recommendation_agreement` would be one.

#### The closure that makes the narrow claim safe

**If such an invariant is added later, the all-unavailable fixture catches it ON THE DAY IT IS WRITTEN, without anyone classifying it.** The check is added to `run_all`, the fixture iterates `run_all`, and a check that claims Pass on an unreadable metric fails the build immediately.

> **That is exactly the case LIVENESS did not handle.** It guarded two hard-coded NAMES, so a seventh invariant was outside it until someone remembered to classify it — and nobody would, because the list is in a different file from the check.

**The honest narrow claim and the replacement fit together**: the claim is true only of today's six, and the fixture is what makes tomorrow's seventh safe without widening the claim.

#### The consequence for any future "liveness" concept

> **Treat it as a FRESH DESIGN PROBLEM, not something preserved because it existed historically.**

**And only if the new invariant cannot derive its requirement from the availability of the metrics it actually consumes.** If it can — and every metric-reading check can, because `Metric` carries its own status — then the requirement is already in the data and a second expression of it is the duplication this section exists to remove.

### 33.2 The argument, recorded so the deletion is not re-litigated

**1. The derived REQUIRED set is EMPTY, over the inputs a check READS.**

| Check | Consults a `Metric`? |
|---|---|
| PRIVACY, PROVENANCE, SELECTION | no — the payload, built locally by the pure `build_review_payload` |
| PERSISTENCE | no — line count and parseability |
| **COMPARISON** | **yes — two, both `DeterministicLocal` and `Observed` offline** |

**No invariant resists the derivation, so there is no residual list.** A guard over an empty set is a no-op with a name.

**2. `Metric`'s shape makes "cannot read what is not there" STRUCTURAL, not conventional.**

```rust
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Metric<T> {
    Observed { value: T, source: MetricSource },
    Unavailable { source: MetricSource, requires: MetricAvailability },   // NO `value` KEY
}
```

`record["x"]["value"]` is `Null` for an unavailable metric, `as_u64()` yields `None`, and `check_comparison`'s `_` arm returns **Skipped**. **The property lives in the type's serialization at the point of use.**

**3. LIVENESS guarded two hard-coded names, not the property.** A check doing `.as_u64().unwrap_or(0)` would have passed on missing data and LIVENESS would not have noticed unless its name were in the list.

**4. Deleting it removes the latent spurious failure §32.10 introduced.** With COMPARISON and PERSISTENCE reachable offline, `check_liveness(_, false)` would have FAILED every run on a machine with no keychain entry. There is no `proxy_available` left to mis-measure.

### 33.3 The naive derivation, falsified — with its case

**Tempting and wrong:** *"an invariant is proxy-dependent iff any metric in the record is `Unavailable{RequiresLiveProxy}`."*

**Run 23's record falsifies it directly**, and `build_comparison_report` confirms the shape is structural rather than incidental to that capture:

```
wholesale_publication_probability  unavailable | requires: requires_live_proxy   (:385)
recommendation_agreement           unavailable | requires: requires_live_proxy   (:395)
wholesale_findings_sent            12 | source: deterministic_local | observed   (unconditional)
shadow_findings_sent               12 | source: deterministic_local | observed   (iff shadow ran)
```

> **An unavailable proxy metric sat directly beside an evaluable invariant. COMPARISON reads the second pair and never the first.** Deriving from what the RECORD CARRIES marks COMPARISON proxy-dependent; deriving from what the CHECK READS does not.

**This paragraph exists to stop the wrong derivation being rediscovered**, because it is the obvious one and it is available in one line of code.

### 33.4 The fixture found a real defect on its first run

**The replacement was not merely equivalent to what it replaced.** Feeding every check inputs it cannot read exposed that **PRIVACY, PROVENANCE and SELECTION all returned Pass on an unreadable payload** — `payload["summary"]["findings"].as_array().unwrap_or(&empty)` yields an empty slice, the loop does not execute, and the check reports Pass.

> **A check passing on input it never got — the exact class LIVENESS was nominally about, in three invariants LIVENESS never guarded.**

**A THIRD option was taken, beyond exclude-or-accept.** The two offered were to exclude the trivially-passing checks from the fixture or to accept the trivial pass. **Both would have recorded a known-wrong Pass as acceptable.** Instead the checks now distinguish **ABSENT from EMPTY**:

* `findings: []` — a genuinely clean manuscript. **Pass is correct**, and `an_empty_findings_array_still_passes` pins it.
* `findings` missing or not an array — **nothing was readable. Skipped**, with the reason, per §4.12.

**Mutation-verified, two mutations, two kills:** reverting PRIVACY's guard reproduces the defect; removing a check from `run_all` fails the enumeration assertion.

### 33.5 `run_all` is the structural half

**Both the runner and the fixture call `run_all`.** A seventh invariant is added in one place and is covered in both. **Today COMPARISON is the only metric-reading check, so a fixture NAMING it would assert the same thing and look equivalent — the difference appears when a seventh check arrives, which is exactly when LIVENESS failed.**

### 33.6 Coverage, and the vacuity gap that remains

> **Coverage remains COMPLETE. The vacuous meta-check was REMOVED rather than counted.**

**The point is not six becoming five.** It is that **the denominator now contains only checks that test observable behaviour** — every entry in it can fail on some input.

**The gap NARROWS WITHOUT CLOSING.** SELECTION still passes at 11 findings against a cap of 12: **evaluated as a function, untested as an invariant.** A fixture that exercises `MAX_FINDINGS` end to end remains open (§32.11) — run 23 produced 20 findings on a real manuscript, and no committed fixture produces more than 11.

### 33.7 Sequencing — §28 is independent

**No check reads `requires`.** The derivation asks only whether a value is `Observed`, which is the `status` tag; §28 refines the `requires` reason. **Independent.**

**Interaction, a preference and not a dependency:** if §28's four causes land first, the all-unavailable fixture becomes four cases rather than one, so building it after §28 avoids a rewrite. **It was built now because LIVENESS's deletion needed a replacement in the same commit**, and widening one fixture is cheaper than leaving the deletion uncovered.

### 33.8 The gate after deletion

```
  PASS  PRIVACY      PASS  COMPARISON
  PASS  PROVENANCE   PASS  PERSISTENCE
  PASS  SELECTION
  5 PASS, 0 FAIL, 0 SKIPPED
  no_failures: true
```

## 34. Milestone 4 — scoping, and an estimate wrong in both directions

**Four items that move report fields from "the engine cannot produce this faithfully" to "it can".** Unlike Milestones 1–3, these increase what the engine KNOWS rather than how it reports what it knows.

### 34.1 The measured order REVERSES §31.2's estimate

| Item | Measured | §31.2 estimated |
|---|---|---|
| **1+2** `Stat::TestStatistic` / `EffectSize` | **1 match site** (`extract/persist.rs:22`) | "F-statistic next" |
| **4** `Location.subsection` | **5 construction sites** | **"the largest of the four"** |
| **3** `Finding.location` | **25 construction sites** | **"nearby-text cheapest"** |

> **The sixth instance of §31.26's pattern, and the first where the estimate failed in BOTH DIRECTIONS AT ONCE.**

**Nearby-text was priced on its ALGORITHM and cost its PLUMBING.** *"A lookup, no extraction"* is true — the lookup is `sections[k].paragraphs[loc.paragraph]`, three lines. **But there is nothing to look up FROM until `Finding` carries a `Location`, and that is 25 construction sites.** The estimate measured the interesting part and the boring part was the whole cost.

**Subsection was priced as touching "every Location producer AND THE EVIDENCE SCHEMA".** `EvidenceRecord` has no `Location` field. **`grep Location gaply-core/src/evidence.rs` returns nothing** — one grep, and it was never run.

**The two failures are different and worth separating.** The first is a genuine estimation difficulty: plumbing is invisible when you are thinking about the algorithm. **The second is not estimation at all — it is a factual claim about the codebase, stated without checking**, and it inflated the item that turned out to be second-cheapest.

### 34.2 TWO RULES CONVERGED on `Option<Location>`

**The field is `Option<Location>`, and it is NOT chosen for compatibility.**

> **A document-level AI-detection finding has no manuscript position. Neither does a `ProcessState` finding about a lane that did not run. A required `Location` would demand a FABRICATED one.**

**§4.12 (typed absence) reaches `Option` from the domain.** **§26.4 (compatibility) reaches `Option` from the cache**, because `Option<T>` deserializes a record that lacks the key — measured, not assumed:

```
Option<T>, no serde attr : parses a record missing the key   → true
Option<T>, serde(default): parses                            → true
required T               : parses                            → false
```

**Two rules arriving at the same shape INDEPENDENTLY is evidence the shape is right rather than convenient.** Had they diverged — had the honest shape been incompatible — the honest shape would still have won and the version would have moved. **They did not diverge, and that is a fact about the design rather than a compromise made to avoid a bump.**

#### A correct prediction about the wrong design

**The task framing predicted:** *"Item 3 in particular — a required Location on Finding — is INCOMPATIBLE by the rule as stated."*

> **Correct about the REQUIRED form, and irrelevant to the form that should be built.**

**Confirmed against the pin rather than asserted, both ways.** A required `Location` makes `CACHED_REPORT_V2` fail and `previously_written_cached_reports_still_deserialize` fire; `Option<Location>` does not. **The pin covers `Finding` — `CACHED_REPORT_V2` deserializes a whole `PublishReadyReport` — so the trigger is armed and correctly aimed. This milestone simply does not pull it.**

**Worth recording because a prediction can be right and still not bear on the decision.** Checking it was still correct: the confirmation is what establishes that the pin's coverage reaches `Finding` at all, which nothing had previously demonstrated.

### 34.3 `check_provenance` — ONE PREDICATE, producer and checker

**A DIFFERENT SHAPE from §32.9's vacuity, and recorded separately for that reason.**

```rust
build_review_payload:  .filter(|p| is_structured_provenance(p))      // producer
check_provenance:      if !is_structured_provenance(s) { fail(…) }   // checker
```

> **PROVENANCE can detect a BYPASSED filter. It cannot detect a WRONG PREDICATE.**

**If `is_structured_provenance` accepted something it should not, both sides would agree and the check would pass.** The two are not independent observations of the same property — they are **one predicate consulted twice**.

**The name is where the gap shows.** *"PROVENANCE"* implies it validates the STRUCTURE of provenance. **It validates that THE FILTER RAN.** Those coincide everywhere except where the predicate is wrong — which is the only place an independent check would have been worth having.

**Not vacuity.** §32.9's checks pass because their inputs are trivial; this one passes because it is asking the producer to confirm its own definition. **A regression guard on the filter, which is genuinely useful and is not what the name claims.**

**Noted, not fixed in this milestone.**

### 34.4 Build order

**By measured cost, not by estimate:** items 1+2 together (they share `persist.rs:22` and the extraction loop, so splitting pays the overhead twice) → item 4 → item 3.

**Item 3 last for a second reason beyond cost:** it is the only item whose wrong shape trips the pin, so it should land when nothing else is in the diff to confuse the signal.

### 34.4b The scoping SEQUENCE, kept because it is reusable

**Four steps, in this order, and the order matters:**

| | Step | Why here |
|---|---|---|
| **1** | **What produces it today, and what would have to change** | Establishes the surface before anything is priced. Item 2 turned out to be *promoting an existing pattern*, not writing a new one |
| **2** | **Schema impact, CONFIRMED AGAINST THE PIN BY EXPERIMENT** | Not reasoned. A required field was added, the pin was run, it fired; `Option` was added, it did not. **The prediction and its confirmation are different acts** |
| **3** | **What the report gains, QUOTED** | *"Reported with an effect size (0) → None."* is checkable; "better statistics coverage" is not. A gain that cannot be quoted is a gain that cannot be verified afterwards |
| **4** | **Cost measured by COMPILING, not estimating** | Add the field, count the errors, revert. §31.2's estimate was wrong at both ends and this took four minutes |

**Step 2 before step 4 is deliberate.** An incompatible change is a different KIND of work regardless of its site count — it moves a version, retires fixtures, and invalidates caches. **Pricing it before knowing that produces a number attached to the wrong question.**

### 34.5 The measurement was scoped to one crate — 1 match site was really 3

**§34.1 recorded item 1+2's cost as "1 match site (`extract/persist.rs:22`)".** That figure came from `cargo check -p gaply_core --all-targets`.

**The workspace has three:**

| Site | Crate |
|---|---|
| `extract/persist.rs:22` | `gaply_core` |
| `paper_corpus.rs:253` | **app** |
| `examples/claim_extraction_spike.rs:748` | **app (example)** |

> **`-p gaply_core` answers "what breaks in the core", and the question asked was "what does this item cost".** A measurement that is correct within its scope and wrong about the question is the same failure as §31.26's instance 3 — *"does this command catch the error?"* is not *"should this be the standing command?"*

The item is still the cheapest of the four — the ordering is unchanged — but the number was wrong by 3×, and wrong in the direction that makes an item look easier than it is.

#### The rule, stated WITHOUT naming a command

> **A cost measurement must be taken over the WIDEST SCOPE THE CHANGE CAN REACH, and the result must CARRY THAT SCOPE.** "3 sites" is not a fact; "3 sites in `gaply_core`" is.

**Deliberately not "run `cargo check --all-targets`".** Commands change — this one was adopted three sections ago and the project has already reconsidered its standing suite twice. **A rule naming a command decays into a rule nobody can apply once the command is renamed**, whereas *"the widest scope the change can reach"* is answerable in any toolchain and for any kind of change, including ones with no compiler at all.

**The scope belongs WITH the number**, not in the reader's head. An unqualified count is read as total, and this one was — §34.1 recorded "1 match site" and the build found three.

#### EXTENSION — the scope is not enough. RECORD THE COMMAND.

**The rule above was applied correctly and still let a number through that nobody could check.** A design brief carried *"Location on Finding — 25 construction sites"*. The scope was implied and the figure looked measured; **the COMMAND that produced it was never written down.**

> **A number with a stated scope but no command is one NOBODY CAN FALSIFY.** "12 in `report.rs`" is checkable only if the command is there to re-run. Without it, the reader's only options are to trust the number or to re-derive it from scratch — and re-deriving is what nobody does to a figure that looks already measured.

**That is how 25 survived into a design brief and out the other side.** It was reproduced by no scope that was tried:

| Counted | Command | N |
|---|---|---|
| `Finding {` literals | `grep -n "Finding {" gaply-core/src/report.rs` | **12** |
| …plus the `evidence.rs` test helper | same over `gaply-core/src src` | **13** |
| distinct emitting **branches** | reading `compile_report`'s call graph | **18** (17 wired + 1 unwired) |
| `ChecklistItem {` sites | `grep -n "ChecklistItem {" gaply-core/src/report.rs` | 4 |

**18 EMITTING BRANCHES is the right unit**, because `stylo_finding` is ONE literal serving SEVEN branches — a literal count understates the decisions to be made by six, and a branch count is what "which sites can supply a location" was actually asking.

**This does not weaken §34.5's rule; it completes it.** The scope says what was looked at, the command says how — and only the second is re-runnable by the next reader.

### 34.6 The `effect_size_pattern_is_unchanged` pin fired on its first run — on its author

**The shared-alternation refactor was written via a Python patch script, with `\b` inside a NON-RAW Python string.** Python read it as a backspace and wrote **ten literal `0x08` bytes** into `stats.rs`. The regex still compiled — `\x08` is a valid literal character — and silently stopped matching word boundaries.

```
left:  "…|\u{8}OR\\s*=|\u{8}HR\\s*=|…|\u{8}R2\u{8}|R²|\u{8}f2\u{8})"
right: "…|\\bOR\\s*=|\\bHR\\s*=|…|\\bR2\\b|R²|\\bf2\\b)"
```

> **The pin written minutes earlier, to prove the refactor did not change the pattern, caught the refactor changing the pattern.**

**Two failures worth separating.** The defect is **§31.20's own lesson, missed by the person who recorded it.** That section is specifically about Python patch scripts silently doing the wrong thing, and its remedy — `assert old in src` — guards the MATCH but not the REPLACEMENT. **An assertion that the pattern was found says nothing about whether what you wrote in its place is what you meant.**

**The catch is the strongest available evidence for byte-identity pins.** Without it the corruption surfaced as two unrelated golden-report tests failing, which reads as *"the new extraction changed the goldens"* — a plausible and expected consequence of this very milestone. **It would have been accepted as correct.**

**Remedy adopted:** a patch script that writes regex or escape-bearing text into source uses a RAW Python string, and any such refactor carries a byte-identity pin against a checked-in literal.

### 34.7 A mutation SURVIVED, and the fix was structural

**`effect_size_present: false` — the exact pre-Milestone-4 behaviour — was hard-coded back in, and the suite stayed green.**

**Cause:** the test asserted on `effect_size_locations(&ex)` and performed its own `contains` check. **It verified the HELPER and never the WIRING**, so the model could carry `false` while the test proved the join worked.

#### The PROPERTY, not the incident

> **A test that reimplements the thing it is testing is INSENSITIVE TO THE WIRING. It passes whether or not the production path uses the code under test — so it measures the helper's correctness and reports it as the feature's.**

**Stated as a property because the incident is not the useful part.** The specific mutation (`effect_size_present: false`) is one of an unbounded family; what is checkable in review is the SHAPE — **a test that computes the expected value by the same route the production code would, rather than reading what the production code produced.**

**The diagnostic:** if the test would still pass with the production call site deleted, it is testing the helper. **`reported_statistics` exists so there is a production call site to read from.**

**Fixed by making the wiring reachable:** the statistics mapping is now `pub fn reported_statistics(&ExtractionResult) -> Vec<ReportedStatistic>`, split out of `into_report_model` so a test can assert on **what the model carries** without constructing a whole `PipelineResult`. **The mutation now fails.**

**§4.17 in a new place** — *assert the invariant, not the implementation*. Here the test asserted an implementation it had itself written, which is the same error with the test as its own producer. **Related to §34.3's one-predicate-two-consumers finding:** both are cases where checker and checked share a source, and neither can detect what they agree on.

## 35. Two families of defect — and the boundary between them

**This section names the pattern most of this record's findings belong to, AND marks where it stops.** It is one of two families, not a unified theory of everything found here — and the second is recorded immediately below precisely so the first is not stretched over it.

### 35.1 PREVENT SILENT DIVERGENCE

> **Whenever two artifacts are intended to stay aligned, REMOVE the place where they could drift, or INSTRUMENT it. A coupling that is intended but unenforced is a coupling that will break, and it will break quietly.**

**"Quietly" is the operative word.** Divergence produces no error. Both artifacts remain individually valid; only their relationship is wrong, and nothing owns a relationship.

#### The four practices are a LIFECYCLE, not four habits

| Stage | Practice | Failure it prevents |
|---|---|---|
| **DESIGN** | one definition, multiple consumers | **forked definitions** |
| **VERIFICATION** | mechanically verify the coupling | **hidden uncoupling** |
| **MEASUREMENT** | carry scope with the result | **scope inflation** |
| **IMPLEMENTATION** | investigate reuse before rewriting | **duplicate implementations** |

**They are ordered by when the divergence is cheapest to prevent.** A forked definition prevented at DESIGN costs nothing; the same fork caught at VERIFICATION costs a test; missed at MEASUREMENT it produces a wrong estimate that a decision is then made on; reached at IMPLEMENTATION it is a second copy someone must find. **Each stage is the last chance before the next one gets more expensive.**

#### Instances already in this record

| Instance | Stage that would have caught it |
|---|---|
| Projections dropping values (§22.4, six instances) | DESIGN |
| `matchTypeLabel`, `adapters.ts`'s five label strings | DESIGN → pinned at VERIFICATION |
| Three hand-built report cache keys (§31.24) | DESIGN — `pub(crate)` made reuse impossible |
| LIVENESS's REQUIRED list vs `MetricAvailability` (§33) | DESIGN — derivable, so the list should not exist |
| The tier-1 doc comment vs reality (§31.17) | VERIFICATION |
| BillingPage's *"metering isn't deployed"* | VERIFICATION |
| `Śarmā` rendered as `arm` (§4.20 TEXT) | VERIFICATION — the render-path instrument |
| `effect_size` alternation, two patterns (§34) | DESIGN — one const, byte-identity pin |
| "1 match site" that was 3 (§34.5) | MEASUREMENT |
| `validate.rs:257` nearly rewritten (§34) | IMPLEMENTATION |

**The remedy is always one of two:** remove the second copy, or instrument the gap. **Never "be careful" — that is the state every one of these was already in.**

### 35.2 THE BOUNDARY — the other family is UNSUPPORTED CLAIM

> **ONE artifact asserting more than its evidence, with NO SECOND ARTIFACT to diverge from.**

**Nothing here is out of sync.** The value is faithful, the pipeline is correct, and the defect is entirely in what the presentation claims about it.

| Instance | The value is right | The FORM claims |
|---|---|---|
| `publication_probability` in a gauge | 0.30 rendered faithfully | **calibration** |
| *"Fix these first"* | ordered by severity, correctly | **remediation order** |
| `ship_ready` | `fail == 0 && skipped == 0`, computed correctly | **that readiness was evaluated** |
| *"% similarity"* on a cosine | the cosine is correct | **semantic similarity** |
| `MAX_FINDINGS = 12` | the top 12 by severity | **that these are the twelve that matter** |

**Looking for a second copy here finds nothing, because there is none.** Applying §35.1's remedy — "remove the duplicate" — has no target, which is the diagnostic that tells the two families apart.

> **DIVERGENCE is fixed by removing the second copy or instrumenting the gap. AN UNSUPPORTED CLAIM is fixed by WEAKENING THE CLAIM or COMPUTING WHAT IT ASSERTS.**

**Both remedies are available for every unsupported claim, and the choice is a product decision.** *"Fix these first"* → *"Issues by severity"* weakens the claim; computing a real remediation order would have been the other answer. `ship_ready` → `no_failures` + `coverage` weakens; making COMPARISON reachable computed.

### 35.3 ONE CLASS SPANS BOTH — which is how we know §4.20 is grouped by symptom

**The sharper result is not that §4.20 spans both families. It is that a SINGLE CLASS does.**

> **LABELS is DIVERGENCE when a second copy exists, and an UNSUPPORTED CLAIM when none does.**

| LABELS instance | Second copy? | Family |
|---|---|---|
| `matchTypeLabel`, `adapters.ts`'s five strings | **yes** — the Rust vocabulary says the same thing | **divergence** — share a source, or pin it |
| The severity enum `.toUpperCase()`d onto the screen | **no** — nothing else claims to name it | **unsupported claim** — an internal identifier asserting it is a user-facing term |

#### Why that is stronger than "§4.20 spans both families"

**A taxonomy grouped by CAUSE cannot have a member that lands in either family.** If the classes named what went wrong, each would resolve to one repair. LABELS resolves to two, **and which one is decided by something outside the class entirely** — whether a second artifact happens to exist elsewhere in the codebase.

> **So §4.20's classes are grouped by SYMPTOM: by how the defect LOOKS in the rendered artifact, not by what produced it.**

| §4.20 class | Family |
|---|---|
| **TEXT** — characters dropped or substituted | divergence |
| **NUMBERS** — a cosine labelled *"% similarity"* | unsupported claim |
| **PRESENTATION** — a lookup drawn as a gauge | unsupported claim |
| **ORDERING** — a sort implying a ranking | unsupported claim |
| **LABELS** — an internal name reaching the screen | **either** |

**"Spans both" would have been compatible with the classes being cause-grouped** — a taxonomy can straddle two families while every member sits cleanly in one. **A member landing in either is not compatible with that**, and that is the whole force of the finding.

### 35.4 THE DECISION PROCEDURE

**Before choosing a remedy, ask ONE question:**

> **Is a SECOND ARTIFACT intended to express the same fact?**

| Answer | Family | Remedy |
|---|---|---|
| **Yes** | **Prevent Silent Divergence** | **remove the duplication, or instrument the coupling** |
| **No** | **Unsupported Claim** | **weaken the claim, or compute what it asserts** |

**The question is answerable without knowing the taxonomy**, which is the point — it is what another engineer can actually apply at the moment of repair. **And it is answerable by looking**: the second artifact either exists in the tree or it does not.

**Worked, on the two LABELS cases.** `matchTypeLabel` — does anything else name a match type? Yes, `vocabulary.rs`. → remove or pin. The uppercased severity enum — does anything else name a severity? Before Milestone 2, **nothing did**. → the remedy was to COMPUTE what the rendering asserted, which is what `severity_label` is.

### 35.5 §4.20 IS NOT RESTRUCTURED — two taxonomies, two questions

**Both are kept, because they answer different questions and are used at different moments:**

| | Question | Used |
|---|---|---|
| **§4.20's five classes** | *"What kinds of implication can this rendered artifact accidentally make?"* | **at review** — a checklist run over a diff |
| **§35's two families** | *"What kind of engineering defect produced this implication?"* | **at repair** — after something is found |

> **A checklist wants to be grouped by symptom, because symptoms are what a reviewer can see.** A repair guide wants to be grouped by cause, because causes are what determine the fix. **Collapsing them would make one of the two worse.**

**A cross-reference preserves both**, and is recorded in ONTOLOGY §4.20 beside the classes so the family question is asked where the classes are used.

## 36. The second principle — OPERATIONAL SUCCESS IS NOT EVIDENCE OF SEMANTIC CORRECTNESS

> **A thing that RAN is not a thing that DID WHAT WAS WANTED. The report of success and the effect are different facts, and only one of them was checked.**

**§31.20 recorded this on two instances and named the remedy. It now has four instances and a practice set**, which is what makes it a principle rather than a habit about `cd`.

### 36.1 The four practices

| Practice | The false signal it refuses |
|---|---|
| **Do not trust an EXIT CODE** | the command ran → *the command did the intended thing* |
| **Do not trust a COUNT without its SCOPE** | 3 sites → *3 sites everywhere* |
| **Do not trust a RULE THAT IS NOT EXECUTED** | the rule is written down → *the rule is in force* |
| **Do not trust a SUCCESSFUL COMMAND until the EFFECT it was meant to produce is VERIFIED** | success → *the effect happened* |

**The four are one refusal applied to four kinds of report.** In each case something reported success truthfully — the command really did exit 0, the count really was 3, the rule really is written, the command really succeeded — **and the report was about a different question than the one being asked.**

### 36.2 The four instances

| # | What reported success | What was actually true |
|---|---|---|
| 1 | `cd src-tauri && python3 …` — **exit 0** | `cd` failed, the patch never applied (§31.20) |
| 2 | `git checkout -- file` — **exit 0** | the file reverted to HEAD, discarding the work (§31.20) |
| 3 | `cargo check -p gaply_core` — **1 match site** | 3 in the workspace; the count was true *within its scope* (§34.5) |
| 4 | `cargo check --all-targets` — **1 error** | run from the repo root with no manifest. **Nothing was checked at all** |

**Instance 4 is today's, and it is the sharpest of the four** because the failure it fabricated was in the direction of alarm rather than of comfort. **A false green is dangerous and a false red is expensive**, and both come from the same refusal to ask what the command actually measured. It was caught by asking *"which directory was that run in?"* before reporting a regression — an effect check, applied to a diagnostic rather than to a mutation.

**A fifth is arguably `assert old in src` itself (§34.6):** the assertion succeeded, the pattern really was found, and **the replacement text was corrupt.** The guard was correct about the question it asked. That instance is recorded under §34.6 because its remedy is specific (raw strings, byte-identity pins), but it belongs to this family.

### 36.3 The ontology converged on two principles, each with a practice set

**Not a designed shape — an observed one.**

| Principle | Practice set | Failure mode |
|---|---|---|
| **PREVENT SILENT DIVERGENCE** (§35) | design · verification · measurement · implementation | two artifacts that should agree, quietly disagreeing |
| **OPERATIONAL SUCCESS IS NOT SEMANTIC CORRECTNESS** (§36) | exit codes · counts · rules · effects | a truthful report about the wrong question |

**Both were reached by accumulating instances and noticing the shape afterwards**, which is the same route §21's instrumentation-maturity section and §4.20's classes took. **The record has never yet produced a useful principle by stating one first**, and that is worth noting about the method rather than only about the principles.

**They are genuinely distinct.** Divergence needs two artifacts; this one needs only a report and a question. **But they compose at the boundary — an unexecuted rule (§36) is how a divergence (§35) stays silent**, which is why the LIVENESS list, the drifting cache keys, and the tier-1 doc comment each appear under §35 while their persistence is explained by §36.

## 37. The standing command, measured at workspace scope

**§31.26 chose `cargo check --all-targets && cargo test --lib` from measurements taken inside the app package. Both halves are blind to `gaply_core`'s own targets.**

### 37.1 Correctness, measured by injecting three faults

| Fault | `check --all-targets && test --lib` | **the routine actually run** | `check --workspace --all-targets` | `test --workspace` |
|---|---|---|---|---|
| Compile error in a **`gaply_core` test** | **MISS** | CAUGHT | CAUGHT | CAUGHT |
| Compile error in an **app example** | CAUGHT | CAUGHT | CAUGHT | CAUGHT |
| **Failing** (compiling) `gaply_core` test | **MISS** | CAUGHT | **MISS** | CAUGHT |

> **`check --workspace --all-targets` alone CANNOT replace a test run** — it executes no tests, so a failing test passes it. That was the question worth asking, and measuring it is what answered it.

> **`cargo test --workspace` alone catches all three.**

### 37.2 Cost, after a CORE edit (`gaply-core/src/lib.rs` touched — what Steps 2 and 3 modify)

| Command | After core edit | Warm | Tests |
|---|---|---|---|
| the routine actually run (three commands) | **42.2s** | 4.9s | 752 |
| `cargo test --workspace` | **93.4s** | 5.1s | 752 |
| `cargo check --workspace --all-targets` | **7.0s** | 0.3s | 0 |

### 37.3 THE ARGUMENT AGAINST KEEPING WHAT IS RUN TODAY

**The three-command routine misses nothing, and that is not a property of the commands.**

> **Its coverage is an ACCIDENT OF HABIT: it misses nothing only because three commands happen to be run and one of them happens to be `-p gaply_core`.** Drop that third command — or run the two that §31.26 actually recorded — and a broken `gaply_core` test compiles to green.

**§31.24 named exactly this: a verification tool whose correctness depends on something other than the tool stops being a verification tool.** The recorded standing command and the practised one had diverged, and the practised one was carrying the coverage.

**The 51-second premium buys eliminating the accident**, it is **paid once before a commit rather than on every edit**, and **the inner loop gets FASTER — 36.6s → 7.0s.**

### 37.4 ADOPTED

| | Command | Cost | Catches |
|---|---|---|---|
| **Inner loop** | `cargo check --workspace --all-targets` | **7.0s / 0.3s warm** | every compile error, both packages, all targets |
| **Before commit** | `cargo test --workspace` | 93.4s | everything, including failing tests |

**Recorded in `CLAUDE.md` as well as here.** A standing routine that lives only in an architecture record is at §21's level 1 — it holds while someone remembers it — and `CLAUDE.md` is the file actually read at the start of a session.

**This does not make the routine enforced.** Nothing runs it automatically; CI remains manual-dispatch-only (§21). **It makes the DEFAULT correct, which is the most a local command can do**, and it removes the case where following the written instruction gives less coverage than the habit.

## 38. Step 2 is a DOCPARSE item, and its value is navigation, not findability

**Step 2 (numbered subsections in a `Location`) is deferred and reclassified. It is not a `report.rs` item and never was.**

### 38.1 Why the rejection was right, and why narrowing was more right

The corpus available to tune it has **zero true subsections**. A rule tuned against such a corpus can only be tuned until its false positives fall silent — **it is tuned against no positive evidence at all**, which is §4.9's absence-operand problem wearing a threshold. Low recall would have been defensible; *unmeasurable precision* is not.

### 38.2 The stronger reason — Step 3 supersedes the purpose

**`nearby_text` supersedes the subsection FOR THE PURPOSE THE SUBSECTION SERVED.** "Results › Primary outcomes, paragraph 3" still requires the reader to count paragraphs. "Results, paragraph 3 — '…the quoted sentence…'" is found by searching the author's own document. **Step 3 delivers findability directly**, and once the sentence is present a numbered-only subheading is a label nobody needs.

### 38.3 Its real home, and its real value

The signal Step 2 wants does not exist in `report.rs` or in `sections.rs`. It exists in **DOCX paragraph styles and PDF font-size runs, which `docparse` currently discards** — `parse_docx` reads `w:t` text and drops `w:pStyle`; `parse_pdf` takes `pdf_extract::extract_text`, which returns no font metrics at all. A heuristic over plaintext is a reconstruction of information the format already carried and the adapter threw away.

| | |
|---|---|
| **Home** | `extract/docparse.rs` — surface style/size runs the adapters discard |
| **Value** | **navigation polish**, not findability |
| **Prerequisite** | a corpus containing real subsections; the present one has none |

**Recorded with that home and that value so it is not later re-scoped as "5 sites" in `report.rs`** — which is the number §34.5's scope-erased measurement produced, and the reason §37 exists.

## 39. Step 3 — the finding's location, and what building it revealed

**`Finding` gained `Option<Location>`; `LocalFinding::nearby_text` fills from it through the resolver the RULE uses.** The design was investigated before implementation so that implementation would decide nothing; this section records what it decided anyway, and what it found.

### 39.1 The result the design turned on

**`validate::paragraph` was not a lookup helper — it was THE TEXT THE RULES EVALUATE** (`max_group_count`, the overclaim scan, the effect-size scan, the causal scan all read it). Promoting it to `extract::paragraph_at` means:

> **The quotation is the string that PRODUCED the finding, not a re-derivation of where the finding meant.** A quotation cannot disagree with the finding it illustrates, because there is one resolver and both go through it.

§34.3's one-predicate-producer-and-checker shape, reached from a different direction.

### 39.2 The section-ordinal ambiguity — SHIPPED WITHOUT FIXING, and the argument is not "pre-existing"

`split_document` emits one section per recognised heading, so a document with two headings that classify alike (`"Abstract"` + `"Summary"`, `"Introduction"` + `"Background"`) yields two sections of one kind. `Location` is then not a unique address, and `find` takes the first.

**"It is pre-existing" is true and is the WEAK form of the argument. The strong form:**

> **Today a repeated `SectionKind` means the RULE evaluated the wrong paragraph — the finding is already wrong, silently, with nothing able to show it. After Step 3 the author sees a quotation that does not contain the problem the finding describes.**
>
> **Step 3 does not introduce the ambiguity. It is the INSTRUMENT THAT WOULD REVEAL IT.**

**Stated so a future reader cannot attribute it to the wrong layer:** *Step 3 deliberately preserves the existing `Location` semantics and makes any repeated-`SectionKind` ambiguity visible in the rendered report rather than silently masking it. **The ambiguity originates in the EXTRACTION MODEL, not in the reporting layer.***

Shipping the instrument before the fix is therefore correct ordering, not deferred debt. Fixing it first would mean fixing a defect no one can currently observe.

*(Measured on the reference manuscript: 5 sections, 5 distinct kinds — no repeat. The frequency in real manuscripts is unknown, which is why the fix waits for evidence.)*

### 39.3 A SHIPPING DEFECT found while scoping, ranked above the join hazard

**`"citation c3 REFUTED by evidence"` names a position in an internal filtered list and identifies nothing to the author.** `citation_id` is `format!("c{}", idx + 1)` over `verify_citations`'s `items` slice, which `pipeline.rs` builds by SKIPPING references whose refverify call errored.

**Two distinct problems, and they are not the same size:**

| | | |
|---|---|---|
| **The label** | **SHIPPING DEFECT** | live today, in the author's report, independent of Step 3 and of whether any location join is ever built. `c3` is not a name the author can resolve to anything. |
| The skew | latent join hazard | `items[i] != references[i]` after any refverify failure, so a future `c{N} → references[N-1] → ¶N-1` join is silently off by the number of prior failures |

**The label is the item to open.** The skew only matters if someone builds the join; the label is being read now. Recorded here rather than fixed because it is not Step 3's diff.

### 39.4 The cap was MEASURED, and the measurement changed the reasoning

Design reasoned to ~300 characters. The frozen reference manuscript (`sha256 859880647c…`) through `parse_path` → `extract_from_text`:

| body paragraphs | median | mean | p90 | max | over 300 chars |
|---|---|---|---|---|---|
| 36 | **728** | 749 | 1269 | 2131 | **32 of 36 (89%)** |

> **The reasoning was wrong about the SHAPE OF THE CHOICE, not merely the number. Truncation is the NORMAL case — no cap avoids it**, so the cap cannot be chosen to minimise how often it fires, which is what "300 is enough for one or two sentences" implicitly assumed.

What it can be chosen for is whether the reader receives a whole opening sentence — first-sentence length: median 189, **p75 270**, p90 439. Paragraphs receiving less than their first full sentence: **8 at 300, 5 at 350, still 5 at 400.** **350 is where the curve flattens**, and the earlier sizing (the bundled `sample_manuscript.txt`, median 140 chars) was a toy that would have produced a defensible-looking wrong answer.

**A second measurement made the whitespace snap load-bearing rather than decorative:** a blind `chars().take(350)` lands mid-token on 23 of the 32 truncated paragraphs and **inside a NUMBER on 2** — `p = 0.03` shown as `p = 0.0` is §4.20's TEXT class in a report about statistics.

### 39.5 What the report gains, on the real document

5 of 9 findings located; every quotation resolves; the two flattened tables are quoted at 367 and 371 characters with the numeric run cut at a whitespace boundary and marked. **Head-anchoring is vindicated by the hardest case in the corpus:** a flattened table's head is its CAPTION — *"Table 1 Main effect of juvenile hormone analogue and of its concentration on the haemolymph…"* — which is the most searchable string it contains. An anchor near the statistic would have emitted `"3.57 3.40 3.37 0.052"`.

**Two observations recorded, neither a Step 3 defect:**

1. **The statistical rules are firing on flattened TABLE content** — 4 of the 5 located findings quote a table, not prose. The quotation makes that visible for the first time. Whether a p-value inside a table should be evaluated as prose is a separate question, now askable because the evidence is on the page.
2. **The same paragraph is quoted twice** where two rules fire on it (`MissingEffectSize` + `MissingConfidenceInterval` on Results ¶3 and ¶4), so the report shows an identical 367-character block twice. The duplicate FINDINGS already existed; the quotation makes the redundancy conspicuous.

### 39.6 Where the design was wrong

**Nothing in the design had to be reversed at implementation.** Two corrections, both from measurement rather than from the code:

* the cap moved 300 → **350**, and the *reason* for having a cap changed (§39.4);
* the **frequency** claims in the design were unmeasured — "real academic paragraphs run 500–1000 chars" was a guess that happened to bracket the measured 728.

**One design claim was confirmed by running it rather than by argument**: `Option<Location>` does not fire `previously_written_cached_reports_still_deserialize`, and the required form fails it with `missing field \`location\``. **`CACHED_REPORT_SCHEMA_VERSION` is NOT bumped** — `pipeline.rs`'s rule, that a compatible change must not bump, or bumping stops meaning anything.

## 40. Two results from Step 3's first real execution

### 40.1 Head-anchoring survived a case it was NOT CHOSEN FOR

The head of the quotation was chosen because a PARAGRAPH begins at a blank line, so its first characters are a clean sentence start and the best search key the report can offer. **The reference manuscript supplied a case that was not in that argument: 4 of the 5 located findings quote a FLATTENED TABLE, which is not prose at all.**

**The decision held, for a reason the argument never used** — a flattened table's head is its CAPTION, the most identifying string it contains:

> *"Table 1 Main effect of juvenile hormone analogue and of its concentration on the haemolymph biochemical constituents of B. mori (CSR2 × CSR4)…"*

An author searching that lands on Table 1 exactly. **The anchor the design rejected — a window centred on the statistic — would have emitted `"3.57 3.40 3.37 0.052"`, which locates nothing** and cannot be searched for, because those digits recur throughout the table.

**A decision that holds on a case outside the reasoning that produced it is better evidenced than one confirmed by its own argument**, which is the distinction §36 draws between operational success and semantic correctness — here in the rare direction where the evidence is stronger than the claim.

### 40.2 THE INSTRUMENT REVEALED A FALSE-POSITIVE CLASS ON ITS FIRST RUN — in extraction, not in the rules

**The investigated hypothesis was refused on evidence.** It supposed rule 3 scans an adjacency window that column-flattening could disrupt. `validate.rs:261` is whole-paragraph CONTAINMENT — it measures no distance — and the manuscript reports **no effect size anywhere** (`EFFECT_SIZE_ALTERNATION`: 0 matches in 33,516 chars), so nothing could have been hidden.

**What the quotation exposed instead is bigger, and is not about tables.** Every p-value extracted from this manuscript is `p ≤ 0.05`, and every occurrence is a SIGNIFICANCE-THRESHOLD DECLARATION:

| Location | Text | |
|---|---|---|
| Methods ¶6 | "compared by the critical difference **at p ≤ 0.05**" | prose |
| Results ¶3 | "NS, **not significant at p ≤ 0.05**" | Table 1 legend |
| Results ¶4 | "NS, **not significant at p ≤ 0.05**" | Table 2 legend |

`Stat::PValue { operator: "<=", value: 0.05 }` is assigned to all three. Rule 3 then tells the author *"A p-value is reported without an accompanying effect size"* and rule 4 *"A primary statistical claim reports a p-value but no confidence interval"* — **about text that reports no statistical claim.**

**Table 3 produced no flag** (Results ¶7, 1247 chars, detected and captioned — its legend lacks the string). The flag follows the threshold text, not the table, which is what distinguishes a mechanism from a correlation here.

**Nothing was watching this before a quotation existed.** The findings were correct-looking sentences over correct-looking counts; only the manuscript's own words, on the page, made the mismatch visible — §35's silent-divergence family, caught by adding an observer rather than an assertion.

#### SCALE — DEMONSTRATED, NOT MEASURED

> **This class is expected to affect a large fraction of quantitative manuscripts, because significance-threshold declarations are common. The reference manuscript demonstrates the MECHANISM; measuring additional manuscripts is the next step.**

**An earlier draft of this note read *"a guaranteed false finding on nearly every paper"*, and it was corrected before it was recorded.** One manuscript demonstrates a mechanism, not a rate — **the same n=1 discipline §30 holds the promotion delta to**, where *"one manuscript, one execution … not a rate"* is stated on a result that was equally tempting to generalise.

**The overreach came from the TASK rather than from the work, which is the THIRD instance of that shape:** §32.6 (the first, and the section that named it), the adjacency-window mechanism refused above (the second), and this. **The count is offered as checkable, not as authority** — the second instance was hedged with an *"if"*, so a reader may reasonably score it differently.

#### STEP 3 MADE IT LEGIBLY WRONG, WHICH RAISES SEVERITY

**Before quotations existed a user had little way to judge whether a finding was grounded correctly.** The finding named a rule, a section and a paragraph number; verifying it meant counting paragraphs in their own document and inferring what the engine had read.

**After Step 3 they see the rule, the quoted text, and the mismatch TOGETHER, in one bullet.** The defect was always there — the instrument made it observable, which is what it was built to do. **The consequence for sequencing: the visibility ships to every user the moment Step 3 does.** The severity of the underlying defect is unchanged; its exposure is not.

#### BASELINE IMPACT — the verdict does not move, the report does

**Run 25 (§27.1): 11 Major findings counted, of which FIVE are this class** — three `MissingEffectSize`, two `MissingConfidenceInterval` — and six are internal-duplication spans at 81–86% word overlap.

| | |
|---|---|
| Major findings, run 25 | **11** → **6** with this class removed |
| Deterministic verdict | **`MajorRevision` either way** |

**One eligible Major finding is sufficient for `MajorRevision`** (`reviewer_synthesis.rs`'s resolver matrix: `Sev::Major => Recommendation::MajorRevision`), so six still yields it. **§30's promotion delta is therefore UNAFFECTED.**

> **But 5 of 9 findings would leave the report** in the extraction-only execution used for Step 3's first run, and 5 of 11 Majors in run 25's full execution. **Verdict stability and finding content are different measures of the same system**, and a reader of §30 should know which one moved. *(Two runs, two denominators — the scopes are stated because the counts are not comparable.)*

### 40.3 The window risk, opposite in sign and NOT MEASURED

Flattening makes the rule's unit BIGGER: Results ¶3 is 2131 chars and 156 decimal numbers against a 741-char median prose paragraph in the same section. *"Same paragraph ≈ same claim"* fails there, and the consequence is **suppression** — one effect size anywhere in a large table would silence every p-value flag in it. **A false NEGATIVE class, and unobservable on this corpus** (zero effect sizes), so it is recorded as a structural risk and not as a measurement.

`ExtractionResult.tables` already holds Table 1/2/3 with captions and locations. `validate.rs` cannot see it and has no notion of table structure. **The information exists; the rules do not consult it.**

## 41. Board item 1 investigated — and a load-bearing claim of the investigation was wrong

### 41.1 The corpus, and what it is not

35 real documents parsed from one machine (0 unreadable), after excluding 11 Word lock files, 4 Gaply design PDFs and 5 non-manuscripts. **Nine files contain an extractor-matched p-value; they reduce to FOUR DISTINCT PAPERS** — six of the nine are revisions or format-variants of one QI paper. 252 p-value matches across files; **47 validation flags across the four distinct papers.**

> **Every threshold-vs-result classification below was made by ONE labeller reading 57 contexts.** No inter-rater agreement was measured. That is the binding limit on every rate in this section, and it is stated first because a table of percentages reads as a measurement whatever the caveat says.

### 41.2 THE CORRECTION — "the evidence is discarded at extraction" IS FALSE

**The investigation's load-bearing claim was that `Stat::PValue.raw` holds only the eight characters `p ≤ 0.05`, so the distinguishing text is gone before any rule runs and no rule-side fix is a candidate.**

**Checked against Step 3's own work, and refuted.** `raw` is not the rule's input:

```rust
// validate.rs:260 — rule 3
if !patterns().effect_size.is_match(paragraph(result, loc)) {
```

`paragraph(result, loc)` is `extract::paragraph_at`, which returns **the whole paragraph**. `StatClaim` carries a `Location`; Step 3 promoted the resolver precisely so this string has one definition. **`validate.rs` can read "compared by the critical difference at" today.**

**Measured over the corpus, on the exact string each rule reads:**

| | flagged paragraphs | policy phrase present |
|---|---|---|
| **Threshold-derived** | **6** | **6 — 100%** |
| Result-derived | 24 | **0 — 0%** |

**`raw` being eight characters is TRUE and does not imply unavailability.** The two are independent, and the investigation collapsed them.

#### THE 0-OF-24 IS THE STRONGER HALF

> **6 of 6 shows the policy phrases are AVAILABLE. 0 of 24 shows they DO NOT OVER-FIRE — and that is the harder half, and the one any detection strategy actually rests on.**

**Availability alone would justify nothing.** A phrase list that appears in every threshold paragraph *and also* in half the result paragraphs is not a signal; it is a way of flagging most p-values. The measurement that matters is the negative one: **24 paragraphs whose p-values are genuine reported results, and not one of them carries a policy phrase.** The 6 says the evidence is reachable; the 24 says reaching for it discriminates.

### 41.3 The conclusion survives; the argument for it does not

**Extraction-side is still the right home — for a DRY reason rather than a forced one.**

> **The classification belongs to the STATISTIC, not to any rule.** Three rules (`PValueOverclaim`, `MissingEffectSize`, `MissingConfidenceInterval`) plus three non-rule consumers (`report_build::describe`, `paper_corpus.rs`, `extract/persist.rs`) all key on `Stat::PValue`. Deciding *"criterion or result"* inside each is the duplication generator this record has found repeatedly — `matchTypeLabel`, `report_cache_key`, the effect-size alternation (§31.24). **Classify once and every consumer inherits it.**

**Why the distinction is worth the paragraph it costs:** *a decision justified by a FALSE CONSTRAINT goes unexamined when the constraint is later found not to hold.* Had "the evidence is gone" stood, a future reader finding `paragraph_at` in `validate.rs` would have concluded the placement was wrong and moved it — because the recorded reason would be visibly false, and nothing would have recorded the real one. **The conclusion survived the correction; the argument for it did not, and only the argument is reusable.**

### 41.4 THE MARKER FALSIFIER — the most useful negative result

**No surface-marker regex is currently justified, and the reason is a case, not an intuition.** ILI Chapter 6:

> "…all four differences are **significant at** p < 0.001."

**A RESULT, using the same construction as IJAS's threshold legend** *"NS, not significant at p ≤ 0.05"*. The bare marker appears in both classes, so it cannot separate them.

**What separated cleanly in §41.2's 6/0 measurement was a POLICY VERB** — *"significance level"*, *"significance was determined at"*, *"critical difference at"*, *"not significant at"* (negated, in a legend), *"was detected"*. **The 0-of-24 is evidence these do not over-fire on results IN THIS CORPUS. It is NOT evidence the list is complete** — three phrasings from three papers is a sample, not a vocabulary, and the falsifier shows how narrow the margin is between a phrase that separates and one that does not.

### 41.5 The value heuristic — FOLK KNOWLEDGE, and half of it falsified

| | occurrences | criteria |
|---|---|---|
| at 0.05 | 10 | **7 (70%)** |
| away from 0.05 | 47 | **0** |

**The falsifier**, Chapter 5-6: *"p < 0.0001 at 1, 2, 4 and 6 h; p < 0.001 at 8 h; **p < 0.05 at 12 h**"* — a reported result, at 0.05, with `<`.

> **The value can rule a criterion OUT (47/47 away from 0.05 are results), never IN (70% is not a decision).** Labelled folk knowledge because that is what it is; the asymmetry is the only part the corpus supports.

**Operators:** `=` is a result 47/47 and never appears as `p = 0.05`. `≤` is a criterion 3/3 — **all three in one paper, in one author's house style, which is not a convention.** `<` is used for both.

### 41.6 The false-negative trade

Suppressing every p at 0.05 removes 8 false flags and costs 2 true ones — **4:1, measured on the corpus that produced the heuristic**, with a 30% error rate at 0.05, and the drop would be **silent**: an absence with no attribution (§4.14). The ratio is the trap, not the argument.

### 41.7 What inherits it

**THREE rules, not two.** `pvalue_locs` feeds `PValueOverclaim` (`:244`), `MissingEffectSize` (`:260`), `MissingConfidenceInterval` (`:274`) — 13 / 85 / 82 flags across the corpus. Rules 1 and 5 key on other sets and are unaffected. **Note rule 4 reads no text at all today** — it tests `is_primary(section) && !ci_locs.contains(loc)` — so it is the one rule for which the evidence is *available but unused*.

**`stats_verdict.rs:127` already guards, with a DIFFERENT rationale:** *"an `=`-reported p is a point value we can compare; inequalities are bounds, not values"*. **Bound-vs-value, not criterion-vs-result** — `p < 0.001` is a genuine result and also a bound. It is a precedent that the split is implementable, **not evidence that it is the right split**, and reading it as support would borrow a conclusion from an argument that does not reach it.

### 41.8 The rate

| Paper | Format | Flags | From a threshold |
|---|---|---|---|
| IJAS Bombyx | PDF | 5 | **5 — 100%** |
| Chapter 5-6 | DOCX | 12 | 2 |
| BMW PDSA / Cureus | DOCX | 28 | 1 |
| ILI Chapter 6 | DOCX | 2 | 0 |
| | | **47** | **8 — 17%** |

**One paper in four has an entirely false statistical-finding set.** Mechanism confirmed on three papers; **the 17% is n=4, one labeller, and is not a population figure.**

### 41.9 Items 1b and 1c — and the identity check 1b required

**1c IS ESTABLISHED REGARDLESS OF THE PAIRING.** The same QI paper yields **26 references as DOCX and 1 as PDF**. A reference count collapsing to one survives any doubt about revisions.

**1b needed an identity check, and it PASSED.** *"Cureus Manuscript (Revised).docx"* vs *"final Cureus Manuscript (Revised).pdf"*:

| | |
|---|---|
| 8-gram shingle overlap | **93.2% A→B, 92.2% B→A** |
| Abstract | identical opening, 6 paragraphs both |
| Words | 8222 / 8318 |
| Residual non-overlap | PDF hyphenation and reflow artefacts — `"hindi- english"`, `"post- intervention"`, `"heteroskedasticity- and"` |

**The doubt was worth raising and did not survive it.** §17/§19/§23.4's pattern — two artifacts assumed identical — checked for the fourth time, and this time the assumption held.

**What the pair then isolates:** 74 statistics extracted from the DOCX, 71 from the PDF — **near-identical statistics, 29 flags versus 16.** The difference is not content and not extraction of the statistics; it is the paragraph UNIT. Methods 102 paragraphs vs 17; Results 312 vs 32.

| Document | Format | Paragraphs | Median chars |
|---|---|---|---|
| IJAS Bombyx | PDF | 36 | **728** |
| Cureus | PDF | 86 | **545** |
| BMW PDSA | DOCX | 477 | **14** |
| Chapter 5-6 | DOCX | 991 | **12** |
| ILI Chapter 6 | DOCX | 2360 | **4** |

`parse_docx` writes `\n\n` at every `</w:p>` (`docparse.rs:370`), so every heading, table cell and one-line row becomes a paragraph; `reflow_pdf_text` is PDF-only. **Flagged DOCX paragraphs of 9, 12, 19, 21 and 30 characters were observed — windows in which no effect size could ever appear, so the flag is structurally guaranteed.**

> **§40.3 recorded the opposite end of this: PDF table flattening makes the window too LARGE. Both are real. They are two ends of one missing invariant — nothing in the system states what "same paragraph" is supposed to MEAN.**

#### 1b's SEVERITY IS STRUCTURAL, NOT FREQUENCY-BASED

**A flagged DOCX paragraph of 9, 12, 19, 21 or 30 characters is a window in which no effect size could fit.** `MissingEffectSize` there is not wrong *on this manuscript* — **it CANNOT BE TRUE. It is false by construction**, and no manuscript exists on which it could be right.

**That is a DIFFERENT CLASS from item 1**, and the difference should not be blurred by both being called false positives:

| | Item 1 | Item 1b |
|---|---|---|
| What is wrong | the finding's **grounding** — it points at a criterion and calls it a reported result | the finding **cannot be true** — the window admits no effect size |
| Does the advice survive? | **often yes** — this manuscript genuinely reports no effect sizes, so "no effect size" is right for the wrong reason | **no** — nothing is being measured |
| Depends on the manuscript? | yes — 1 paper in 4 was wholly affected | **no — structural** |

**And the exposure is worse than the corpus suggests. DOCX is what most authors upload**, and **the baseline never saw it: run 25 was the PDF** (`manuscript_sha256 859880647c…`, §30.1). **Every operational measurement this record holds was taken on the format with the LARGER window** — so §30's baseline, §27.1's finding decomposition and §40.2's rate are all PDF figures, and the DOCX path has never been measured end to end.

**1b has therefore graduated from hypothesis to finding.** The ordering below was decided when it had not.

### 41.10 The corpus was PRICED AND DECLINED

| | |
|---|---|
| Source | PubMed Central OA subset; this machine cannot supply it — 35 documents yielded 4 with p-values |
| Fetch + parse | ~1 hour, negligible compute |
| **Labelling** | 50 papers × ~15 p-values ≈ **750 contexts, 2–4 hours focused** |
| Second labeller | doubles it — and inter-rater agreement should be measured, not assumed |
| **Total** | **half a day to a day of human labelling** |

**DECLINED, and the reason is not cost.** The mechanism holds on three papers and the fix's location is settled without it. **The rate matters for prioritising this against other work, not for deciding whether it is real** — and paying for a number that cannot change the decision is the shape §31.26 warns about.

### 41.11 Ordering, decided on evidence grade

1. **Item 1** — it produces incorrect claims about the manuscript, the fix's location is settled, no corpus is needed to begin.
2. **Item 1c** — established, and likely an independent extraction defect.
3. **Item 1b** — decided third when it was a hypothesis; §41.9's identity check has since graduated it to a finding.

**The ordering follows the evidence grades the investigation produced rather than treating three discoveries as equally established** — which is the durable part, and it survives 1b's promotion.
