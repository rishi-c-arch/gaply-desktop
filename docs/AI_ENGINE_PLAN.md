# Citation Intelligence — AI Engine Plan

**Status:** plan only. No implementation code exists or is authorised by this document.
**Grounded in:** the repo at `bcae851`, read directly — `src-tauri/src/`, `src-tauri/gaply-core/src/`,
`gaply-core/src/migrations.rs`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.

> **`docs/AI_ENGINE_SPEC.md` does not exist in this repository.** No file of that name is present
> anywhere in the tree. This plan is therefore grounded in the codebase plus the requirements given
> in the task itself. Wherever the task's requirements contradict what the code actually does, the
> code wins and the contradiction is called out in §9 as an open decision. If the spec exists
> elsewhere, this plan must be re-checked against it before any code is written.

---

## 1. Hard rules

These are load-bearing. Everything below is subordinate to them.

**R1 — The deterministic citation path has ZERO dependency on this module.**
CSL/citeproc formatting, DOI parsing and resolution, dedupe, import, export and the local citation
library must continue to work with `src-tauri/src/ai/` and `gaply-core/src/ai_engine/` deleted
entirely. Concretely: nothing under `src/screens/citations/` may import from an AI bridge, and
`gaply_core::citation_library` may not reference any new AI module. The dependency arrow points one
way only — AI *reads* citation data, citation code never reads AI data.
*Enforcement:* a test that asserts the citation modules' import graph contains no AI module, plus
the deletion drill in §8.

**R2 — No network calls anywhere in the AI module tree.**
No `reqwest`, no `hf-hub`, no sockets, no `localhost`. Models load from local paths only.
*Note:* `src/models/ollama_verify.rs` already talks to `http://127.0.0.1:11434`. That is an existing
module and stays where it is; the AI engine must not call it or depend on it.
*Enforcement:* §8's grep-based test, mirroring how `gaply-core` already keeps itself network-free.

**R3 — The SLM never generates citation strings, DOIs, years, or metadata values.**
The generative model may only *select*, *classify*, *rank*, *summarise* and *point at* text that
already exists in a stored chunk. Every generative output must be anchored to a `chunk_id` that the
caller can re-read. A value that appears in an AI output but not in the chunk it cites is a bug, and
§5's `validate` module exists to catch exactly that before the row is written.

---

## 2. What already exists — do not rebuild it

This is the most important section of the plan. A substantial part of the proposed AI layer is
already implemented in `gaply-core`, and the plan below reuses rather than duplicates it.

| Proposed | Already exists | Verdict |
|---|---|---|
| chunking | `gaply-core/src/chunk.rs` — `chunk_text`, 512 tokens / 64 overlap | **Reuse as-is** |
| embeddings abstraction | `gaply-core/src/embed.rs` — `Embedder` trait | **Reuse the trait; replace the impl** |
| vector search | `gaply-core/src/vector.rs` — sqlite-vec `vec0`, 384-dim, KNN | **Reuse** |
| RAG ingestion | `gaply-core/src/rag.rs` — provenance, sanitize, quarantine, chunk, embed | **Reuse the pipeline shape** |
| evidence persistence | `gaply-core/src/evidence_store.rs` — two-phase write, provenance columns | **Follow as the precedent** |
| doc parsing | `gaply-core/src/extract/docparse.rs` | **Reuse** |
| model runtime seam | `src/models/` — candle, `PerplexityModel` trait impl in the app crate | **Follow as the precedent** |

Two facts about the existing pieces materially shape the plan:

- **`embed.rs`'s default `HashEmbedder` is a placeholder, not a model.** It is a deterministic
  384-dim feature-hashing bag-of-words encoder, and its own doc comment says so: *"a drop-in
  placeholder until a real all-MiniLM-L6-v2 runtime (ONNX) is wired in"*. Semantic retrieval does
  not currently exist — it is lexical hashing wearing a vector interface. The "embedding model
  resident from startup" requirement is therefore **new work**, not a change to existing behaviour.
- **No model is resident today.** `src/models/mod.rs::perplexity_model()` constructs a fresh model
  on every call, so a multi-GB GGUF is loaded per run. There is no refcount, no idle timer, no state
  machine. The `ModelManager` in §6 has no existing implementation to follow — it is a genuine
  addition, and an improvement on current behaviour.

---

## 3. Crate split — the constraint that decides module layout

The task proposes a single tree at `src-tauri/src/ai/{engine, model_manager, embeddings, retrieval,
tasks, jobs, validate}`. That cannot be built as one tree, because of two hard facts in the repo:

1. **`Database::conn` is `pub(crate)`** (`gaply-core/src/db.rs:118`). The app crate cannot execute
   SQL at all. `citation_library.rs` documents this explicitly as the reason its CRUD lives in
   gaply-core. Every new table's DAO must therefore live in **gaply-core**.
2. **`gaply-core` has no `tokio` and no ML dependencies**, deliberately, so that
   `cargo test -p gaply_core` links neither and Windows CI stays fast. `candle`, `tokenizers` and
   `tokio` are app-crate-only, by the same rule that put `ReqwestFetcher` there.

So the module tree splits along the existing seam: **pure/sync/SQL in gaply-core, async/ML in the
app crate**, joined by traits.

### 3.1 gaply-core — pure, sync, no ML, no network

```
src-tauri/gaply-core/src/ai_engine/
  mod.rs            AiEngine facade; re-exports; module docs carry R1–R3
  store.rs          DAO for every table in §4. Owns all SQL. Precedent: citation_library.rs
  retrieval.rs      Query planning + result assembly over vector.rs KNN + ai_chunks.
                    Pure ranking/fusion; takes &dyn Embedder, never a model.
  evidence.rs       EvidenceCard domain type + the mapping to/from evidence_cards rows.
                    Precedent: evidence.rs vs evidence_store.rs (domain vs persistence).
  validate.rs       R3 ENFORCEMENT. verify_grounded(card, chunk) -> Result<(), Ungrounded>.
                    Rejects any DOI/year/citation-string in an output absent from its chunk.
                    Pure string/regex work; fully unit-testable with no model.
  jobs.rs           Job + JobItem state machine and its SQL. Pure state transitions.
  prompts.rs        Versioned prompt templates as &'static str + PROMPT_VERSION consts.
                    In core so prompt_version is testable without loading a model.
  tasks/
    mod.rs          The AiTask trait: prompt building + output parsing per task.
    screen.rs       relevance screening of a retrieved chunk against a query
    summarize.rs    extractive summary anchored to chunk spans
    contradict.rs   claim-vs-source contradiction detection
```

`ai_engine` must not be referenced from `citation_library.rs` (R1).

### 3.2 app crate — async, ML, orchestration

```
src-tauri/src/ai/
  mod.rs            Wiring; the only place tokio and candle meet gaply-core
  model_manager.rs  ModelManager (§6). Owns load/unload, refcount, idle timer
  inference.rs      The SINGLE inference task + mpsc channel (§7)
  embeddings.rs     A real Embedder impl (candle or ONNX) — implements
                    gaply_core::embed::Embedder from the app crate.
                    Precedent: models/candle_perplexity.rs implements PerplexityModel
  engine.rs         AiEngineHandle: the façade Tauri commands hold. Enqueues, never blocks
  jobs.rs           Job runner: drains ai_jobs, calls inference, writes results via
                    gaply_core::ai_engine::store
```

Commands go in `src/commands.rs` alongside the existing 59, matching the file's conventions.
No new command file — the repo keeps them in one place.

---

## 4. SQLite schema additions

**Append-only. No existing table is modified.** Existing schema is at migration **v13**
(`evidence_claim_kind`), so these are **v14 and v15**. Migrations carry `up`/`down` pairs, run in a
transaction, and are tracked in `schema_migrations` — follow `migrations.rs` exactly.

### 4.1 One forced rename

> **`chunks` already exists** — created by migration v6 (`rag_documents`), with
> `document_id → documents(id)`, `seq`, `content`, `token_count`. The proposed `chunks` table would
> collide outright. It is renamed **`ai_chunks`** below. For consistency the remaining new tables
> keep the names given in the task, since none of them collide. `evidence_cards` sits next to the
> existing `evidence` table (v12) — different table, deliberately different name, and §9 asks
> whether they should be one thing.

### 4.2 Provenance contract

Every row that stores AI output carries all five, non-null except where stated:
`chunk_id`, `page`, `model_id`, `prompt_version`, `created_at`.
`page` is nullable **only** where the source document genuinely has no pagination (a plain-text
ingest); the ingestion path must record it wherever the parser knows it. `model_id` is a foreign key
into `model_registry`, never a free string, so an output can always be traced to the exact weights.

### 4.3 Migration v14 — corpus

```sql
CREATE TABLE documents_ai (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    citation_id   TEXT,                    -- soft link to citation_library.id; NOT a FK (R1)
    source_path   TEXT NOT NULL DEFAULT '',
    title         TEXT NOT NULL DEFAULT '',
    checksum      TEXT NOT NULL UNIQUE,    -- sha256; dedupe + re-ingest detection
    page_count    INTEGER,
    status        TEXT NOT NULL,           -- pending | ingested | quarantined | failed
    quarantine_reason TEXT,
    created_at    INTEGER NOT NULL
);
CREATE INDEX idx_documents_ai_citation ON documents_ai(citation_id);

CREATE TABLE ai_chunks (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id   INTEGER NOT NULL REFERENCES documents_ai(id) ON DELETE CASCADE,
    seq           INTEGER NOT NULL,
    page          INTEGER,                 -- NULL only for unpaginated sources
    char_start    INTEGER NOT NULL,        -- span back into the source text, for R3 grounding
    char_end      INTEGER NOT NULL,
    content       TEXT NOT NULL,
    token_count   INTEGER NOT NULL,
    created_at    INTEGER NOT NULL,
    UNIQUE (document_id, seq)
);
CREATE INDEX idx_ai_chunks_document ON ai_chunks(document_id);

CREATE VIRTUAL TABLE chunk_embeddings USING vec0(
    embedding FLOAT[384],
    +chunk_id INTEGER,
    +model_id TEXT
);

CREATE TABLE model_registry (
    id            TEXT PRIMARY KEY,        -- e.g. "qwen2.5-0.5b-instruct-q4km"
    kind          TEXT NOT NULL,           -- embedding | generative
    display_name  TEXT NOT NULL,
    file_path     TEXT NOT NULL,
    sha256        TEXT,
    dim           INTEGER,                 -- embedding models only
    quant         TEXT,
    registered_at INTEGER NOT NULL
);
```

`citation_id` is deliberately **not** a foreign key. A FK would make the citation table depend on
the AI corpus for referential integrity and would let a cascade delete AI rows out from under it —
both violate R1. A dangling `citation_id` is a normal, handled state.

`chunk_embeddings` carries `model_id` so a re-embed under a new model is detectable and old vectors
are never silently mixed with new ones at query time.

### 4.4 Migration v15 — jobs and outputs

```sql
CREATE TABLE ai_jobs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    kind          TEXT NOT NULL,           -- screen | summarize | contradict | embed
    status        TEXT NOT NULL,           -- queued | running | done | failed | cancelled
    total_items   INTEGER NOT NULL DEFAULT 0,
    done_items    INTEGER NOT NULL DEFAULT 0,
    model_id      TEXT REFERENCES model_registry(id),
    prompt_version TEXT NOT NULL,
    error         TEXT,
    created_at    INTEGER NOT NULL,
    started_at    INTEGER,
    finished_at   INTEGER
);
CREATE INDEX idx_ai_jobs_status ON ai_jobs(status, created_at);

CREATE TABLE ai_job_items (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id        INTEGER NOT NULL REFERENCES ai_jobs(id) ON DELETE CASCADE,
    chunk_id      INTEGER NOT NULL REFERENCES ai_chunks(id) ON DELETE CASCADE,
    status        TEXT NOT NULL,           -- queued | running | done | failed | skipped
    attempts      INTEGER NOT NULL DEFAULT 0,
    error         TEXT,
    created_at    INTEGER NOT NULL,
    finished_at   INTEGER,
    UNIQUE (job_id, chunk_id)
);
CREATE INDEX idx_ai_job_items_job ON ai_job_items(job_id, status);

CREATE TABLE evidence_cards (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id         INTEGER REFERENCES ai_jobs(id) ON DELETE SET NULL,
    chunk_id       INTEGER NOT NULL REFERENCES ai_chunks(id) ON DELETE CASCADE,
    page           INTEGER,
    kind           TEXT NOT NULL,          -- support | contradict | context
    claim          TEXT NOT NULL,          -- the claim under test (caller-supplied, not generated)
    quote          TEXT NOT NULL,          -- VERBATIM span from the chunk (R3)
    quote_start    INTEGER NOT NULL,       -- offsets into ai_chunks.content — checkable
    quote_end      INTEGER NOT NULL,
    confidence     REAL,
    model_id       TEXT NOT NULL REFERENCES model_registry(id),
    prompt_version TEXT NOT NULL,
    created_at     INTEGER NOT NULL
);
CREATE INDEX idx_evidence_cards_chunk ON evidence_cards(chunk_id);
```

`quote_start`/`quote_end` are what make R3 mechanically enforceable rather than aspirational:
`validate.rs` re-slices the chunk and asserts the stored quote is byte-identical. A model that
paraphrases fails the check and the row is never written.

---

## 5. Two-model lifetime rule

| | Embedding model | Generative model (SLM) |
|---|---|---|
| Lifetime | **Resident from startup** | **On-demand** |
| Held by | `AppState.embedder: Arc<dyn Embedder>` — *already exists* | `AppState.models: Arc<ModelManager>` — new |
| Size | ~90 MB (MiniLM-class) | 0.5–4.4 GB (GGUF) |
| Unload | never | refcount → 0, then idle timer |

The embedding side needs **no new lifecycle machinery** — `AppState` already holds an
`Arc<dyn Embedder>` constructed once in `lib.rs::setup`. The work is replacing `HashEmbedder` with a
real model in `src/ai/embeddings.rs`, keeping the trait. If the real embedder fails to load, it
falls back to `HashEmbedder` and records that in `model_registry` — never silently, because vectors
from the two are not comparable and mixing them would corrupt retrieval. That is what
`chunk_embeddings.model_id` is for.

---

## 6. ModelManager state machine

Owned by **`src/ai/model_manager.rs`**, held as `Arc<ModelManager>` in `AppState`, constructed in
`lib.rs::setup` alongside the existing `embedder`. It owns the generative model **only**.

```
                    acquire()
   ┌──────────┐ ─────────────────► ┌──────────┐
   │ NotLoaded│                    │ Loading  │
   └──────────┘ ◄───────────────── └──────────┘
        ▲         load failed           │ weights mapped
        │                               ▼
        │                          ┌──────────┐
        │                          │  Ready   │◄──── acquire() (refcount += 1)
        │                          └──────────┘
        │                            │      ▲
        │              refcount == 0 │      │ acquire() before timer fires
        │                            ▼      │
        │                          ┌──────────┐
        │                          │   Idle   │  idle_since = now
        │                          └──────────┘
        │                               │ idle_timer elapsed (default 300s)
        │                               ▼
        │                          ┌──────────┐
        └───────────────────────── │Unloading │
              weights dropped      └──────────┘
```

- `acquire() -> Result<ModelLease>` transitions `NotLoaded|Idle → Ready` and increments the
  refcount. **`ModelLease` decrements on `Drop`**, so a cancelled or panicking task cannot leak the
  refcount — the same reason the repo uses RAII rather than manual bookkeeping elsewhere.
- `Idle → Ready` on a new acquire is the important edge: a user running two screenings a minute
  apart must not pay two multi-GB loads. The idle timer exists to release memory, not to enforce a
  cadence.
- `Loading` is a state, not a lock held across the load: concurrent `acquire()` calls await one
  `tokio::sync::Notify` rather than each starting a load.
- `Unloading` is entered only from `Idle`, and re-checks the refcount under the lock immediately
  before dropping the weights, so an `acquire()` racing the timer wins.
- Memory gating reuses what exists: `models::free_memory_bytes()`,
  `total_physical_ram_bytes()` and the tier logic already in `src/models/mod.rs`. A machine that
  cannot hold the model must fail `acquire()` honestly, exactly as `aicheck_memory_status` already
  reports today — never load and get jetsam-killed.

---

## 7. Single-inference invariant

**Exactly one long-lived tokio task owns the model context. Nothing else touches it.**

```
Tauri command ──► AiEngineHandle::enqueue(req) ──► mpsc::Sender<InferenceRequest>
                                                         │
                                                  (bounded, capacity N)
                                                         ▼
                                        ┌────────────────────────────┐
                                        │  inference task (single)   │
                                        │  spawned once in setup     │
                                        │  owns ModelLease + context │
                                        └────────────────────────────┘
                                                         │
                                        oneshot::Sender<Result<Out>> per request
```

```rust
pub struct InferenceRequest {
    pub job_id: i64,
    pub item_id: i64,
    pub prompt: String,
    pub prompt_version: &'static str,
    pub cancel: CancellationToken,
    pub reply: oneshot::Sender<Result<InferenceOutput, GaplyError>>,
}
```

- The task is spawned **once** in `lib.rs::setup` and lives for the process. It is the only holder
  of the model context, which removes the need for a mutex around inference entirely.
- The channel is **bounded**. A full queue is backpressure the command surface must report, not an
  unbounded memory sink.
- Every request carries a `tokio_util::sync::CancellationToken`. The task checks it **between
  decode steps**, so cancellation is bounded by one token's latency rather than one request's.
  A token cancelled before the request is dequeued short-circuits without loading anything.
- Cancellation is **cooperative and honest**: it stops generation, it does not un-write completed
  rows. Completed `ai_job_items` stay `done`. This matches the semantics the Citation Manager's
  verify pass already uses, and it is the right default here for the same reason — a partial
  per-item result is valid, unlike a partial document score.
- `tokio_util` is a **new dependency** (`tokio` itself is already `features = ["full"]`).

### 7.1 The command-shape contradiction

The task specifies *"Tauri commands enqueue and return immediately."* **The repo's established
pattern is the opposite**, and it is used by every long-running feature:

```ts
// src/screens/checks/checkBridge.ts:69 — the canonical shape
const ch = new Channel<AiCheckEvent>();
ch.onmessage = onEvent;
return invoke<AiCheckResult>('run_aicheck', { path, verifyCitations, onEvent: ch });
```

`run_aicheck` is `async`, streams progress over `tauri::ipc::Channel`, runs the work under
`spawn_blocking`, and **awaits the final result**. Cancellation is a separate `cancel_aicheck`
command flipping an `Arc<AtomicBool>` in `AppState`.

Both shapes are defensible; they should not both exist. §9 asks which to adopt. **This plan's
default, pending that answer, is the existing pattern** — the frontend has five bridges built
against it, it needs no polling, and matching it is cheaper than diverging. The recommended shape:

| Command | Shape | Returns |
|---|---|---|
| `ai_job_start` | async + `Channel<AiJobEvent>` | terminal `AiJobResult` |
| `ai_job_cancel` | sync | `()` — cancels the token by `job_id` |
| `ai_job_status` | sync | `AiJobStatus` — for a reopened window |
| `ai_ingest_document` | async + `Channel` | `DocumentId` |
| `ai_model_status` | sync | `ModelStatus` — state machine state + memory tier |

`ai_job_status` is what makes the jobs tables earn their place: without persisted jobs, a window
reopened mid-run has nothing to reattach to.

---

## 8. Enforcing the hard rules

| Rule | Mechanism | Where |
|---|---|---|
| R1 deletion drill | CI job: delete both AI trees, drop the modules from `lib.rs`, run the citation test suites. They must pass. | new CI step |
| R1 import graph | Test asserting no file under `src/screens/citations/` or `gaply-core/src/citation_library.rs` references an AI module | `gaply-core/tests/` |
| R2 no network | Test that greps both AI trees for `reqwest`, `hf_hub`, `TcpStream`, `http://`, `https://` and fails on a hit | `gaply-core/tests/`, mirroring the existing core-purity habit |
| R3 grounding | `validate::verify_grounded` called before **every** `evidence_cards` insert; unit tests with a paraphrasing model double | `ai_engine/validate.rs` |
| R3 no invention | Property test: for every generated card, `chunk.content[quote_start..quote_end] == quote` | `ai_engine/validate.rs` |

R2's grep test is the weaker of the three — it catches direct use, not a call through an existing
app-crate helper. The structural guarantee would be putting the AI logic in a crate with no network
dependency in its `Cargo.toml`, which is exactly why `gaply-core` is network-free today. Pushing as
much as possible into `gaply-core/src/ai_engine/` makes R2 structural rather than advisory, and is
an additional argument for the split in §3.

---

## 9. Decisions

### RESOLVED (2026-08-26)

1. **Runtime = candle. RESOLVED.** llama.cpp is not being added — the no-C-toolchain constraint is
   load-bearing, and it is what already shaped the `reqwest`/`tokenizers`/`statrs` choices.
   *Consequence:* grammar-constrained decoding is unavailable, and is replaced by strict output
   validation plus one retry, specified in a later phase.
2. **Command shape = the existing repo pattern. RESOLVED.** Async command + `tauri::ipc::Channel`
   streaming + `spawn_blocking` + `Arc<AtomicBool>` cancellation, exactly as AI Check does — five
   frontend bridges already speak this shape, so matching it costs nothing and diverging would cost
   a second convention. Supersedes the enqueue-and-return-immediately shape in §7.
3. **`ai_evidence_cards` is a separate table. RESOLVED.** It is NOT merged with the existing
   `evidence` table: that table serves the PublishReady evidence workflow, whereas AI cards are
   model-derived analytical judgments needing independent provenance, model/prompt versioning,
   confidence, validation state, and their own deletion/reprocessing lifecycle. AI cards reference
   `documents`/`ai_chunks` and must never replace canonical citation metadata.

### Still open

4. **Which embedding model?** `embed.rs` names all-MiniLM-L6-v2 (384-dim, matching both vec0 tables)
   and says ONNX. candle can serve BERT-family models and would avoid a second ML stack. Confirm the
   model and the runtime.
5. **Re-embedding existing rows.** The live `embeddings` table holds `HashEmbedder` vectors from the
   current RAG pipeline. Once a real embedder exists, those are not comparable. Leave them (they
   serve the existing RAG path) or re-embed? This plan leaves them untouched and puts AI vectors in
   `chunk_embeddings`.
6. **`docs/AI_ENGINE_SPEC.md`** — does it exist elsewhere? This plan is unvalidated against it.

---

## 10. Sequencing

Ordered so each step is independently verifiable and nothing is built on an unconfirmed decision.

1. Migrations v14/v15 + `ai_engine/store.rs` DAO + round-trip tests. No models involved.
2. `ai_engine/validate.rs` + `prompts.rs`. Pure, fully testable with no model.
3. `src/ai/embeddings.rs` — real `Embedder`, swap in `lib.rs::setup`, keep the fallback.
4. `ai_engine/retrieval.rs` over the new tables.
5. `src/ai/model_manager.rs` + state-machine tests using a stub model.
6. `src/ai/inference.rs` — the single task, channel, cancellation.
7. `ai_engine/tasks/*` + `src/ai/jobs.rs` runner.
8. Commands + frontend bridge, matching whichever shape §9.2 settles.
9. R1/R2/R3 enforcement tests and the CI deletion drill.

Steps 1–4 are unblocked by every question in §9 except 4. Steps 5–7 need question 1 answered.

---

## 11. Deviations from this plan, recorded before implementation

### D1 — table names carry the `ai_` prefix throughout (Phase 1)

§4 kept the task's original names where they did not collide (`documents_ai`,
`chunk_embeddings`, `evidence_cards`). Phase 1 instead prefixes **every** new table `ai_`:
`ai_chunks`, `ai_chunk_embeddings`, `ai_evidence_cards`, `ai_evidence_card_chunks`, `ai_jobs`,
`ai_job_items`, `ai_model_registry`. One prefix, one rule, no per-table exceptions to remember, and
`ai_evidence_cards` cannot be mistaken for the existing `evidence` table at a glance.

`documents_ai` from §4.3 is **not created**. Phase 1 chunks reference the **existing** `documents`
table (v6) directly, because `rag.rs` already writes provenance rows there — checksum, status,
quarantine reason — and duplicating that would mean two document registries and two ingestion
truths. AI-specific document state, if it turns out to be needed, gets its own table later rather
than a fork of this one.

### D2 — `ai_chunk_embeddings` is a real table, not a `vec0` virtual table (Phase 1)

§4.3 proposed `CREATE VIRTUAL TABLE … USING vec0`. Phase 1 creates an ordinary table with a
`vector BLOB` column plus mandatory `model_id` and `dim`. Two reasons: a `vec0` table cannot express
a foreign key to `ai_chunks(id)` or a `NOT NULL` on `model_id`, and Phase 1 writes no embeddings at
all, so committing to a vector index before the embedding model is chosen would be guessing. The KNN
path is a later phase's decision; the schema records the vectors and their model space honestly in
the meantime.

### D3 — `reflow_pdf_text` is refactored to carry a page tag per line (Phase 1)

**This is the only change to shared extraction code in Phase 1, and it is behaviour-preserving.**

`docparse::parse_path` funnels PDFs through `pdf_extract::extract_text`, which returns one flat
`String` with no page boundaries; `reflow_pdf_text` then rewrites that text — joining wrapped lines
and deleting running headers/footers — so character offsets into the raw text do not survive into
the reflowed text. Page attribution therefore cannot be recovered after the fact, and inferring a
page from text position is forbidden by the PAGE RULE.

`pdf-extract 0.7.12` does expose `extract_text_by_pages(path) -> Vec<String>`
(`src/lib.rs:2230`), so pages **are** available — the current path simply does not ask for them.

The change: `reflow_pdf_text(&str) -> String` keeps its exact signature and behaviour, and becomes a
thin wrapper over a new `reflow_pdf_lines(&[(Option<u32>, &str)]) -> Vec<(Option<u32>, String)>`
carrying the same algorithm. Every existing caller — plagiarism, AI Check, PublishReady, Gap Finder
— is untouched and its tests must stay green; that is the acceptance condition for this refactor.

*Why refactor rather than duplicate:* a second copy of the reflow rules is the exact
same-rule-in-two-places defect this codebase has been bitten by before. One algorithm, two entry
points.

*Page attribution rule:* a block is attributed to the page of the line that **started** it. A
paragraph spanning a page break belongs to the page it starts on. This is a recorded fact, never an
estimate. Non-paginated sources (DOCX, TXT, MD) yield `page = NULL` by construction.
