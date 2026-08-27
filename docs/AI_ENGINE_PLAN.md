# Citation Intelligence — AI Engine Plan

**Status:** plan only. No implementation code exists or is authorised by this document.
**Grounded in:** the repo at `bcae851`, read directly — `src-tauri/src/`, `src-tauri/gaply-core/src/`,
`gaply-core/src/migrations.rs`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.

> **`docs/AI_ENGINE_SPEC.md` is in-repo and authoritative for the task layer** (§9.7). Where the
> spec's wording predates a resolved architecture decision — its llama.cpp/GGUF runtime line and its
> GBNF grammar requirement — §9 supersedes it, and an ARCHITECTURE OVERRIDE note at the head of the
> spec says so in the spec itself. Everything else in the spec stands: the eight task prompts, their
> schemas, the evidence rules, the abstention behaviour and the provenance requirements.

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

**R4 — Nothing leaves the machine.**
"No telemetry, usage statistics, document content, queries, embeddings, or error payloads are sent
to any remote service by the AI layer. The pinned model download in ai_model_install is the sole
permitted network operation."
*Consequence:* R2 is narrowed, not weakened — `ai_model_install` is the single, explicitly
user-invoked exception, it lives in the app crate, and it never runs at startup (see §9.4).

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

4. **Embedding model = bge-small-en-v1.5, 384-dim, served by candle. RESOLVED (2026-08-27).**
   BERT-family via `candle-transformers` plus the `tokenizers` crate already in the app crate — no
   second ML stack (no fastembed, no ONNX runtime), which is the same constraint that resolved §9.1.
   384 dims match the dimension already assumed throughout.
5. **The HashEmbedder corpus is NOT re-embedded. RESOLVED (2026-08-27).**
   The `embeddings` vec0 table and the plagiarism lane are untouched. The AI layer reads and writes
   ONLY `ai_chunk_embeddings`, keyed by `model_id` — two embedding spaces are never mixed, and the
   existing RAG path keeps working exactly as it does today.

8. **Phase 3's development/test generative model = the ALREADY-BUNDLED
   Qwen2.5-0.5B-Instruct-Q4_K_M. RESOLVED (2026-08-27).** It ships in `tauri.conf.json`
   `bundle.resources` (`bundled-models/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf` →
   `models/stage1-lm/…`, 397,808,192 bytes) with the shared Qwen2.5 tokenizer at
   `models/slm1-adapter/tokenizer.json`. Registering it in `ai_model_registry` as
   `kind='generative'` therefore requires **no network and no download** — it is already on disk,
   which is what makes it the right choice for building and testing the engine.
9. **The PRODUCTION generative model for the task layer is OPEN.** A larger Qwen-class model,
   downloaded on demand exactly like the embedding model, decided with eval data in the task phase.
   **The engine built in Phase 3 must therefore be model-file-agnostic: nothing may hard-code the
   0.5B path outside the registry.** The 0.5B is resolved through the existing
   `models::stage1_lm_paths()` and recorded as a registry row; every consumer reads the registry.
   Swapping the production model must be a registry change plus a download, never a code change in
   the engine.

10. **Task context is `TASK_N_CTX = 4096`, not the GGUF maximum. RESOLVED (2026-08-27).**
   The bundled Qwen2.5 advertises 32768, and Phase 3 sized the KV cache from that figure. The task
   workload does not need it: citation_need is a sentence plus its two neighbours and a section
   name, with `max_tokens: 200`; the retrieval-fed tasks are a handful of chunks. Sizing for 32768
   reserved 768 MB to serve workloads using a fraction of it.

   **Measured, bundled Qwen2.5-0.5B-Instruct-Q4_K_M:**

   | | n_ctx 32768 (before) | **n_ctx 4096 (after)** |
   |---|---|---|
   | weights | 379 MB | 379 MB |
   | KV cache | 768 MB | **96 MB** |
   | **total** | **1147 MB** | **475 MB** |

   672 MB back, and the figure now describes what the engine actually does. It is a CONFIGURED
   CEILING, not an average: `generate` rejects a prompt that would not fit alongside `max_tokens`,
   with an error naming the budget. Without that enforcement the reported number would be a floor a
   long prompt could silently exceed — a number that reads as a measurement and behaves as a wish.
   `RamEstimate` reports `model_max_context` alongside `context_length` so the configured and
   maximum values can never be mistaken for one another, and the configured value is capped by the
   model's own limit so a larger `TASK_N_CTX` cannot over-report.

   Revisit if a task genuinely needs a longer window — it is one constant, and the reported RAM
   follows it automatically.

### Still open

6. **Promote retrieval to an FTS ∪ vector union** — parallel gateways into one candidate pool,
   rather than today's FTS-prefilter-then-rerank pipeline. Revisit once the reranker phase lands,
   with eval data. The current shape is a strict prefilter, so a chunk FTS misses cannot be recovered
   by vector similarity; the union removes that ceiling at the cost of a larger candidate pool.
7. **`docs/AI_ENGINE_SPEC.md`. RESOLVED (2026-08-27).** The specification is now committed in-repo
   and is authoritative for the task layer. The resolved architecture decisions in §9 take
   precedence over any obsolete runtime/decoding wording in the specification — an ARCHITECTURE
   OVERRIDE note at the head of the spec states this in the spec itself, so a reader who opens only
   that file cannot be misled. Concretely: the spec's "llama.cpp / GGUF" runtime line and its GBNF
   grammar-constrained decoding requirement are superseded by §9.1 (Candle; no llama.cpp;
   deterministic validation plus one retry). Everything else in it — the eight task prompts, their
   schemas, the evidence rules, the abstention behaviour and the provenance requirements — stands.

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

### D4 — `char_start` / `char_end` hold UTF-8 BYTE offsets

The schema column names are `char_start` / `char_end`, and they are kept. The values stored in them
are **UTF-8 byte offsets** (Rust slice indices) into the document text, not Unicode scalar counts.

Byte offsets are what make the grounding invariant mechanically checkable:
`document_text[char_start..char_end] == content`, byte-identical, is a one-line assertion in Rust and
is exactly what `validate.rs` will need to prove an evidence quote was not paraphrased. Scalar counts
would require a conversion pass on every check and would silently disagree with `content` on any
document containing non-ASCII — which, for a citation tool, is most of them.

Recorded rather than renamed because the column names are already specified and a rename buys
nothing; the semantics are documented on `PagedChunk` at the point of use.

### D5 — `ai_index_document` takes an optional `path` alongside `document_id`

Specified as `ai_index_document(document_id)`. Implemented as
`ai_index_document(document_id, path: Option<String>)`.

The `documents` table (v6) records `source_type`, `title`, `source_url` and `checksum` — it has no
local file path column, and `rag::ingest_document` takes content already in hand rather than parsing
a file. Nothing in Phase 1 creates a `documents` row for a local paper, so a `document_id` alone
does not tell the indexer what to read.

`path` is therefore accepted explicitly and falls back to the row's `source_url` when omitted. When
neither yields a readable file the command fails with an honest error rather than indexing nothing
and reporting success. The parameter disappears once an AI ingestion command owns document creation.

### D6 — RESOLVED: pooling is CLS, preprocessing version `bge-v1.5-p2`

The constants originally pinned `EMBED_POOLING = mean`. `bge-small-en-v1.5` publishes **CLS**
pooling — `1_Pooling/config.json` sets `pooling_mode_cls_token: true` /
`pooling_mode_mean_tokens: false`, and the card's reference snippet is `model_output[0][:, 0]`.
The divergence was flagged rather than silently changed, measured, and is now resolved to CLS.

**Rationale.** CLS is the configuration the model was trained and benchmarked in. One synthetic
query favouring mean by 0.03 of margin is not grounds to run off-distribution. The switch is free
now, while only test vectors exist, and would mean re-embedding user libraries later.

**Measured, release build, real model, 500-chunk corpus.** Rank 1 was the acceptance criterion,
not any particular score:

| | mean / p1 | **CLS / p2 (shipped)** |
|---|---|---|
| Needle A — exact, needle score | 0.8863 | **0.8821** |
| Needle A — best distractor | 0.5252 | **0.5893** |
| Needle A — **rank** | 1 | **1 ✓** |
| Needle B — paraphrase, needle score | 0.7258 | **0.7669** |
| Needle B — best distractor | 0.5445 | **0.6210** |
| Needle B — **rank** | 1 | **1 ✓** |

Both needles rank 1 under CLS, which is the criterion. CLS raises the whole score distribution,
needle and distractors alike, so its margins are narrower on this corpus while its absolute
similarity is higher — a difference in score calibration, not in ranking. The paraphrase query
shares no content word with the planted sentence, a premise the test asserts before searching, so
neither result can be lexical.

**Migration: nothing to purge.** `ai_chunk_embeddings` was verified to contain **0 rows** before
the switch — checked read-only against the live database at
`~/Library/Application Support/ai.gaply.app/gaply.db` (schema v15), both immediately after the
decision and again immediately before the re-run. No persistent `bge-v1.5-p1` vector ever existed:
every needle-test vector lived in an in-memory database and was discarded with it. Nothing was
deleted because there was nothing to delete, and nothing went unaccounted for.

The legacy `embeddings` vec0 table and the plagiarism corpus were not touched (§9.5).

**Pooling and `PREPROCESSING_VERSION` must always move in the same edit.** They did here: p1 → p2.
The two poolings are different vector spaces, and the stored `preprocessing_version` is the only
thing preventing one from being compared against the other — `resolve_single_space` refuses a
mixture rather than returning confident nonsense.

### D7 — retrieval is a strict FTS prefilter at this scale

Per §9.6 the eventual shape is an FTS ∪ vector union. This phase ships the prefilter pipeline
described in the task: FTS5 → top 200 candidates → cosine → top k, with a full-scope cosine fallback
when FTS returns fewer than k. Full-library brute-force cosine is acceptable at current scale and is
recorded here as a known ceiling, not an oversight: a chunk that FTS misses cannot be recovered by
vector similarity until §9.6 lands.

### D8 — citation_need has no `<evidence>` block, so three of the four shared rules do not apply

Spec §0 states four rules "apply to all 8" prompts:

1. only use text inside `<evidence>`; 2. every field traceable to a `chunk_id`;
3. abstain when the evidence is insufficient; 4. raw JSON only.

**Prompt 3 has no `<evidence>` block.** Its INPUT is `preceding_sentence` / `sentence` /
`following_sentence` / `section` — the sentence under test and its neighbours, not retrieved
passages. Rules 1–3 are therefore inapplicable to it and are NOT injected: telling the model it may
only use text inside a block that does not exist would be incoherent, and instructing it to echo a
`chunk_id` it was never given is precisely the invention the rule exists to prevent.

Rule 4 IS injected, and matters more here than anywhere else: §9.1 removed grammar-constrained
decoding, so "raw JSON only" is the only instruction standing between the model and a fenced or
prose-wrapped reply. The harness strips fences anyway, but the instruction reduces how often it has
to.

`TaskContext` is still constructed (empty) and `run_task` still runs the generic chunk-id check —
it simply has nothing to check, and `require_known_chunk` on an empty context reports
"given: none". The plumbing is uniform across tasks even where one task carries no evidence.

The SYSTEM / INPUT / OUTPUT SCHEMA / RULES text is otherwise **verbatim from spec Prompt 3**. The
ARCHITECTURE OVERRIDE covers runtime and decoding only; prompt text is authoritative.

### D9 — evidence-bearing tasks persist, classification tasks do not

`ai_evidence_cards` exists for outputs that point AT a source. citation_need produces no evidence,
cites no chunk, and quotes nothing — it classifies one sentence. Writing it to an evidence table
would put rows there with a NULL chunk_id and an empty quote, which is exactly the shape the
`quote_start`/`quote_end` grounding check was built to make impossible.

So this phase persists nothing: `ai_citation_need` returns its result to the caller. When a batch
job needs durable classification results, that gets its own table with its own provenance columns,
not a widened evidence table.

### D10 — OPEN, found by the Phase 4 eval: the corrective retry makes a small model worse

The first eval run of citation_need against the bundled 0.5B produced a clear and unexpected
failure pattern, recorded here because it is a defect in the Phase 3 harness design rather than in
citation_need, and because the fix should be chosen with eval data rather than guessed.

**Observed, 8 seed cases, `citation_need-v1`, Qwen2.5-0.5B-Instruct-Q4_K_M, n_ctx 4096:**

| metric | value |
|---|---|
| valid outputs | 3 / 8 |
| needs_citation accuracy (of valid) | 67% |
| sentence_type agreement | 33% |
| severity agreement | 33% |
| **validation failure rate** | **62%** |
| retry rate | 62% |
| mean latency | 17,118 ms |
| tokens/sec | 1.4 |

**The failure is in the RETRY, not the first attempt.** Attempt 1 is almost always well-formed JSON
that violates one narrow rule — usually `search_query` under the 6-word minimum
(`"land degradation, distribution"`). The retry then returns *no JSON at all*: the model echoes the
corrective error list back as bullets —

```
- reason: 25 words or less
- severity: high|medium|low
- needs_citation: true|false
```

`run_task` appends errors as `- field: problem` lines, and a 0.5B continues that pattern instead of
switching back to JSON. So a retry designed to rescue a near-miss converts it into a total loss.
**Every one of the 5 failures followed this shape.** A larger model would likely not do this, which
is exactly why it must be measured rather than assumed.

Candidate fixes, none applied yet: restate the required JSON skeleton in the retry rather than only
the errors; phrase errors as prose rather than a bulleted list; or keep the first attempt when the
retry parses worse than it did. The last is attractive but must not become a silent repair — it
would need to be reported as such.

**Two other signals from the same run, for the prompt phase:** the model classified almost
everything as `empirical_claim` / `high` / `needs_citation: true` regardless of input, and its
`reason` repeatedly describes *"the preceding sentence"* rather than the target — suggesting the
INPUT block order (preceding first) draws a small model's attention to the wrong sentence. Both are
prompt-level, not engine-level, and belong to the phase that tunes citation_need against the real
50-case set.

### D11 — `AiTask::prompt_version` becomes an instance method

Phase 3 declared `fn prompt_version() -> &'static str` as an associated function with no `self`,
which assumed one prompt per task type. Comparing `citation_need-v1` against `-v2` on the same eval
set breaks that assumption: the version is a property of the INSTANCE, not the type.

The alternative — a second task struct per variant — would duplicate the schema, the validator and
the rules text purely to carry a different string, and the two copies would drift the moment one is
edited. One task, one validator, a variant field.

Signature changes to `fn prompt_version(&self) -> &'static str`; `run_task` calls
`task.prompt_version()`. Behaviour is unchanged for every existing caller.
