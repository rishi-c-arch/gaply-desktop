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
| `matched_year` / `citation_count` (`refverify.rs:661,712,846`) | **Still discarded** — recency uses local `Reference.year` |
| `shadow_reviewer` (`commands.rs:677`) | **Still discarded** — zero frontend references |
| `PlagiarismReport.note` (ISOLATION_NOTE) | **Newly identified** — `compile_report` never reads `.note` |
| `from_plagiarism` explanation (`swarm.rs:379-401`) | **Newly identified** — soft loop skips the Plagiarism engine; feeds the debate vote only |

Net: two resolved, two persisting, two newly found — four current.

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
