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

11. **Validation is two-tier, and a worse retry never replaces a better first attempt.
   RESOLVED (2026-08-28). This changes `run_task`'s contract.** Phase 4b measured that every first
   attempt was well-formed JSON and every failure was a *rule* violation on an otherwise valid
   object, after which the retry destroyed it (D10). Two changes follow from that:

   **(a) `ValidationError` carries a tier.** `Fatal` keeps the existing behaviour — one retry, then
   `ValidationFailed`. `Advisory` is new: the output is **ACCEPTED**, no retry, and the deviations
   travel with it in `TaskRun::advisories`, to be written into `provenance_json` on persistence.

   The line between the tiers is *what a wrong answer costs*. Fatal covers grounding and safety —
   an invented or unsent `chunk_id`, a page mismatch, a reference-shaped or URL-bearing
   `search_query`, a schema or enum violation, a logical inconsistency between fields. Those make an
   output actively misleading, and a misleading citation is the worst thing this product can emit.
   Advisory covers style and length — a `reason` over the word limit, a `search_query` outside 6–12
   words. Those make an output untidy. Discarding a correct classification because its rationale ran
   to 27 words instead of 25 was the engine preferring nothing over something slightly long.

   **THIS IS NOT SILENT REPAIR.** Nothing is modified, truncated or rewritten. The output is passed
   through exactly as the model produced it, and every advisory is reported to the caller and
   persisted. The difference between this and a silent repair is that a reader can still see what
   the model actually said and how it deviated.

   **(b) Keep-better-attempt.** When a Fatal failure triggers the retry and the retry is WORSE —
   unparseable when the first parsed, or carrying more fatal errors — `ValidationFailed` names the
   FIRST attempt as primary and reports its errors. Both raw outputs are still carried. It remains
   an error; the caller decides. The engine simply stops throwing away the better of two bad
   answers.

   **Tiering lives in the validator, never in the prompt.** The spec's prompt text is unchanged and
   still states every rule with equal force — the model is asked for the same thing. What changed is
   only what the engine does when the model misses. A validator maps spec rules to tiers, and each
   task enumerates that mapping as consts beside its validator so the choice is reviewable rather
   than buried in control flow.

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

### D10 — UPDATED with Phase 4b data: the retry destroys recoverable near-misses

**Phase 4 finding (kept for the record).** The corrective suffix appended only error lines and
ENDED on them, so a 0.5B continued the bullet pattern instead of returning to JSON. 5 of 8 seed
cases produced no JSON at all on retry.

**Phase 4b, item 1 — retry format fixed.** The suffix now quotes the rejected attempt back, numbers
the errors as prose rather than bullets, and ENDS on the output contract ("output ONLY the corrected
JSON object ... beginning with `{`"). The bullet-continuation is gone. **The validation failure rate
did not move: 62% before, 62% after (v1).** The failure mode changed rather than disappearing — the
retry now emits a fragment such as `"..."` followed by a stray brace, or resumes from *inside* the
object, omitting the opening brace it was explicitly told to start with.

**The measurement that matters, and it reframes the whole problem.** Parsing attempt 1 separately
from the end-to-end result:

| | attempt-1 parseable | `reason` describes the TARGET sentence | end-to-end valid |
|---|---|---|---|
| **v1** | **8 / 8** | 2 / 8 | 3 / 8 |
| **v2** | **8 / 8** | **8 / 8** | **0 / 8** |

Every first attempt, under both variants, is well-formed JSON. Every failure is a *rule* violation —
almost always `search_query` outside 6-12 words — on an otherwise valid object. **The retry then
converts a recoverable near-miss into a total loss.** v2's 0/8 is not v2 being worse; it is v2
producing slightly longer text that trips the length rule more often, and the retry destroying all
of it.

**So D10's third candidate is now the supported one:** when the retry parses *worse* than the first
attempt, keep the first attempt and report the run as still-invalid with its original errors. That
is not a silent repair — nothing is patched, and the caller still receives a failure — but it stops
the engine throwing away the better of two bad answers. NOT IMPLEMENTED: it changes `run_task`'s
contract and deserves its own decision.

**v2 resolves the wrong-sentence half.** Under v1, 6 of 8 `reason` fields described *"the preceding
sentence"*; under v2, 0 of 8 do — every one describes the target. Moving the target last, under an
explicit `SENTENCE TO CLASSIFY` heading with the neighbours marked "do NOT classify these", fixed it
completely on this model. **v2 is the default.**

**The always-true collapse is NOT resolved and is not a prompt-order problem.** Under both variants
every parseable attempt answered `needs_citation: true`, `sentence_type: empirical_claim`,
`severity: high` — 8/8 under v2, including the transition sentence and the author's own result. The
0.5B is not discriminating; it is emitting the modal answer. That is a model-capacity question for
§9.9, not a prompt question, and it is why the answer distribution is now a reported column: v1's
67% accuracy came from an unbalanced set plus a constant answer, which accuracy alone hid.

### D11 — `AiTask::prompt_version` becomes an instance method

Phase 3 declared `fn prompt_version() -> &'static str` as an associated function with no `self`,
which assumed one prompt per task type. Comparing `citation_need-v1` against `-v2` on the same eval
set breaks that assumption: the version is a property of the INSTANCE, not the type.

The alternative — a second task struct per variant — would duplicate the schema, the validator and
the rules text purely to carry a different string, and the two copies would drift the moment one is
edited. One task, one validator, a variant field.

Signature changes to `fn prompt_version(&self) -> &'static str`; `run_task` calls
`task.prompt_version()`. Behaviour is unchanged for every existing caller.

### D12 — latency is PREFILL-dominated (Phase 4b, item 3)

| | v1 | v2 |
|---|---|---|
| mean prompt tokens | 682 | 926 |
| mean prefill | 11,709 ms | 17,089 ms |
| mean decode | 2,234 ms | 4,475 ms |
| **prefill share of model time** | **84%** | **79%** |
| decode tokens/sec | 34.8 | 31.3 |
| overall tokens/sec | 5.4 | 6.4 |

Phase 4's headline of "1.4 tok/s" was an artifact of dividing generated tokens by *total* wall time.
Decode alone runs at **~32-35 tok/s**, which is usable. Roughly **80% of model time is the single
prefill forward over a 700-900 token prompt.**

Two consequences for §9.9. **Metal acceleration is the high-value lever, not a smaller model** — a
prefill is one large matmul, exactly the shape a GPU helps, whereas decode is already fast enough
that halving the parameter count would buy little. And **prompt length is expensive**: v2 costs 244
more prompt tokens than v1 and 5.4 s more prefill per case, so the retry — which re-sends the whole
prompt plus the rejected attempt — roughly doubles the cost of any case that fails. Fixing the retry
is a latency win as well as a correctness one.

Timing caveat, recorded: the timed span covers the whole decode step, not only the forward. An
earlier version timed the forward alone and reported model time 4.7x below wall time, because
extracting a 152k-vocab logits row and scanning it for the argmax is real per-token cost. With the
full step timed, model time and wall time reconcile — 13.9 s vs 14.1 s for v1.

### D13 — Phase 4c outcome: two-tier validation, measured

§9.11 implemented. Same 8 seed cases, `citation_need-v2`, Qwen2.5-0.5B-Instruct-Q4_K_M, n_ctx 4096.

| | before (4b) | **after (4c)** |
|---|---|---|
| valid outputs | 0 / 8 | **6 / 8** |
| validation failure rate | 100% | **25%** |
| retry rate | 100% | **25%** |
| advisory rate | — | **75%** |
| mean latency | 21,776 ms | **14,728 ms** |
| mean prompt tokens | 926 | 519 |
| prefill share | 79% | 77% |

Latency fell 32% for exactly the reason D12 predicted: the retry re-sends the whole prompt, so
removing 6 of 8 retries removed most of the prompt tokens.

**Every advisory, on the 6 accepted cases:** five are `search_query` outside 6–12 words (2, 4, 5, 5,
14) and two are `reason` over 25 words (27, 31). Nothing else. That is the entire population of
deviations that was previously discarding whole answers.

**The 2 remaining failures are correctly fatal** and both look like this:

```json
{"needs_citation": true, "sentence_type": "empirical_claim", "severity": "high",
 "reason": "The next section turns to the methods ...", "search_query": null}
```

`needs_citation: true` with `search_query: null` is a contradiction — the model asserts a citation is
needed and simultaneously offers no way to find one — so there is no safe reading and a retry is
right. **Keep-better-attempt fired on both**, and the report names the first attempt as primary:
the retry reproduced the identical object minus its opening brace, so it was strictly worse. Before
this change, that unparseable retry would have been the only artifact reported.

**The always-true collapse REMAINS, as expected, and is now unmistakable.** All 6 accepted outputs
answered `needs_citation: true` / `empirical_claim` / `high` — including the transition sentence, the
author's own result, and the common-knowledge case.

Note how the headline accuracy MOVED THE WRONG WAY while the engine got better: 67% (4b) → 33% (4c).
That is a denominator artifact, not a regression. 4b scored 3 cases, two of which happened to carry
`true` labels; 4c scores 6, of which two do. A model that always answers `true` scores whatever
fraction of the scored set is labelled `true`. **This is precisely why the answer distribution is a
reported column** — accuracy alone would have read as a regression caused by accepting more outputs,
which is the opposite of what happened.

Capacity, not prompting, and it belongs to §9.9. The engine is no longer the limiting factor.

### D14 — `EVIDENCE_BUDGET_TOKENS = 1200` on the dev model, not the spec's implied budget

The spec's Prompt 2 sends "top_k reranked chunks from that source only" without naming a token
figure; the working assumption elsewhere has been ~2500.

**Measured reason for the smaller budget.** D12 established that this build is prefill-dominated:
~80% of model time is one forward over the prompt, and Phase 4b measured roughly **22 ms per prompt
token** on CPU. A 2500-token evidence block is therefore ~55 s of prefill **before a single output
token**, on top of the prompt's own scaffolding. At 1200 the same block costs ~26 s.

This is a DEV-MODEL constraint, not a judgement about how much evidence the task needs. It is the
first place the engine has traded answer quality for latency, so it is recorded rather than
absorbed: fewer chunks means a real chance the supporting passage is the one that got dropped, and
`chunks_dropped` is returned in the result so that possibility is visible rather than inferred.

**Raise this when Metal lands (Phase 6).** Prefill is one large matmul — exactly what GPU
acceleration helps — so the budget should be revisited against measured prefill on the accelerated
path, not left at a number chosen for a CPU.

### D15 — empty retrieval short-circuits; it is NOT `insufficient_evidence`

The spec defines `insufficient_evidence` as "retrieved chunks do not cover the claim's topic at
all" — a judgement the model makes ABOUT evidence it was shown.

When retrieval returns nothing for a document, there is no evidence to show. Sending an empty
`<evidence>` block and asking a model to judge a claim against it invites exactly the invention this
whole layer exists to prevent: the four shared rules tell it to use only what is inside that block,
and there is nothing inside it.

So zero chunks returns a typed `NoEvidence` outcome WITHOUT running the model. Nothing is generated,
nothing is persisted, and the caller is told the document had no indexed evidence — which is a
different fact from "the model read the evidence and found it off-topic", and a different fix
(index the document vs. re-examine the claim).

### D16 — page is validated against the STORE, not against what the model echoed

`TaskContext::require_known_chunk` already rejects a `chunk_id` that was never sent. Prompt 2 also
asks the model to echo a `page` per supporting chunk, which is a second thing it can get wrong while
looking right.

The page is therefore checked against the page recorded on the chunk that was actually sent, not
merely for internal consistency. A card claiming page 8 for a chunk stored on page 12 would send a
reader to the wrong page of a real PDF — the precise failure the `chunk_id`-echo rule exists to
prevent, one field over. Fatal.

Where the stored page is `None` — a source with no pagination — any page the model supplies is
unverifiable, so a non-null page is rejected rather than trusted.


### D17 — the evidence budget was in WORDS while named for tokens; corrected, with a correction to the correction

`EVIDENCE_BUDGET_TOKENS` is a model-token figure, but `assemble` trims using
`split_whitespace().count()` — words. The two are not the same, and the constant's name asserted
they were.

**First attempt at the ratio was wrong.** It was derived from the eval's `mean prompt tokens` (2198)
against the word budget (1200), giving ~0.46 words per token. That number folds the JSON schema and
rules blocks into the denominator, and punctuation-dense schema text tokenizes far more heavily than
prose. Budgeting on 0.46 would have under-filled the evidence block by roughly 40% — the opposite
error to the one the fix was introduced to remove, and equally invisible.

**Measured properly**, with the real Qwen2.5 tokenizer over the fixture paper's prose rendered as an
evidence block: **491 words → 640 tokens = 0.767 words per token.** That is the constant, and
`measure_words_per_model_token_for_evidence_text` is the `#[ignore]`d test that produces it, so the
figure can be re-derived rather than trusted if the tokenizer changes.

**This changed no eval number.** The Phase 5 fixture is 491 words in total, so a 1200-token budget
(≈920 words) never binds on it — nothing was ever dropped, `chunks_dropped` was 0 throughout, and
prompt tokens were identical before and after. The bug was real and would have bitten on a real
paper; it simply had no effect on this measurement. Recorded rather than quietly corrected, because
"the fix changed nothing" is exactly the claim that deserves the evidence attached.


## Phase 5 findings — citation_support on the 0.5B dev model

### Grounding: zero ungrounded acceptances, and what that is worth

No accepted output contained a chunk_id that was not sent, or a page that disagrees with the store.
Two things carry that claim, and they are worth different amounts:

- **By construction (strong).** The validator resolves every returned `chunk_id` against the set
  actually rendered into `<evidence>`, and every returned page against the STORE row for that chunk
  (D16) — not against the model's own header echo, which would let a fabricated pair agree with
  itself. Both are `Tier::Fatal`, so a violating output cannot be returned, and persistence is
  downstream of validation, so it cannot be written either. Pinned by two negative tests that
  construct the violation deliberately: `an_invented_chunk_id_never_reaches_the_table` and
  `a_page_that_disagrees_with_the_store_never_reaches_the_table`.
- **By observation (weak, and I will not dress it up).** The real-model smoke run accepted
  **0 of 3** outputs, so its "zero ungrounded acceptances" is vacuously true — an empty set has no
  bad members. It is not independent evidence. The eval run is slightly better: cs-seed-04 was
  killed by the page rule with *"says page 2, but chunk c2 is stored on page 1"* — the fabrication
  the rule exists for, caught on real output.

### The model is the bottleneck, not the harness

Eval, 6 seeds, `citation_support-v1`, qwen2.5-0.5b-instruct-q4km:

```
FAIL cs-seed-01  102794ms  fatal (Retry kept): <output>: not valid JSON: EOF while parsing a list
FAIL cs-seed-02   68735ms  fatal (First kept): suggested_rewrite: must be null when the verdict is 'weak'
FAIL cs-seed-03   88223ms  fatal (Retry kept): <output>: not valid JSON: invalid length 0
FAIL cs-seed-04   76938ms  fatal (First kept): supporting_chunks[1].page: says page 2, but chunk c2 is
                                              stored on page 1
FAIL cs-seed-05   92291ms  fatal (Retry kept): <output>: no JSON object or array found
PASS cs-seed-06            NoEvidence (no model run)
valid outputs 1/6 · validation failure 83% · retry 83% · advisory 0%
mean latency 71497 ms · mean prompt tokens 2198 · mean prefill 50200 ms
```

Real-model smoke (3 cases, verdict deliberately not asserted) failed all three on the **same** rule:
`suggested_rewrite: must be null when the verdict is 'weak'`. I checked the validator against the
spec before blaming the model — SPEC line 137: *"suggested_rewrite: only for 'partial' … null for
every other verdict."* The rule is correct and the model is violating it, consistently. Combined
with the always-`weak` verdict, this is the same capacity collapse as §9.9's always-true finding,
in a different output field: the 0.5B has a fixed response shape and the evidence barely moves it.

**citation_support is materially harder than citation_need** — 83% fatal vs 62%, and 71s vs 14s per
case — because the prompt carries an evidence block and the output is a nested structure with two
arrays and a cross-field conditional. This is a size problem. It is the strongest evidence yet for
resolving §9.8 (production model) upward, and it should not be read as a prompt-tuning task first.

### What did work

- **D15 short-circuit**: cs-seed-06 returned NoEvidence with the model never loaded — no invented
  judgement over an empty evidence block, and no 70s spent to produce one.
- **D16 page-vs-store**: caught real fabrication (cs-seed-04), not a synthetic one.
- **D12 keep-better-attempt**: "First kept" on seeds 02 and 04 — the retry was worse and did not
  overwrite the near-miss. The mechanism is doing its job even when the final verdict is failure.


### D18 — the grounding guarantee covers IDENTIFIERS, not PROSE

The verbatim smoke outputs make a boundary explicit that the summarised failures hid, and it must be
written down before anyone reads "zero ungrounded acceptances" as "the output is grounded".

**What is validated:** `chunk_id` (must be in the set actually sent) and `page` (must equal the
store's row). Both `Tier::Fatal`.

**What is NOT validated:** `why`, `explanation`, `suggested_rewrite` — free prose, checked only for
length (Advisory). Nothing ties them to the evidence text.

Smoke seed 3 shows why this matters. The evidence said *"No significant effect of management was
observed for earthworm abundance."* The model's `explanation` said:

> "The study shows a significant increase in earthworm abundance under organic management, which
> supports the claim that organic management significantly increased earthworm abundance."

That is the evidence's exact opposite, asserted as support. It was rejected — but **only
incidentally**, by the `suggested_rewrite`-non-null-on-`weak` rule. Had the model returned
`suggested_rewrite: null`, this output would have been ACCEPTED and PERSISTED: every chunk_id real,
every page correct, and a fabricated finding in the prose the user actually reads.

So the guarantee is exactly: *a stored card cannot cite a passage that was not retrieved, or
mislocate one that was.* It is not: *a stored card's prose is faithful to its evidence.* The second
needs entailment checking against the chunk text, which is Phase 6 work and a larger model. Until
then the prose fields are model assertions carrying provenance, not verified claims, and any UI must
present them that way.

### D19 — the asymmetric verdict rule lets `weak` through unearned

By design (§11 D13) only `strong` must be earned against `claim_elements`; a false `strong` is the
dangerous output, a false `weak` is merely unhelpful. All three smoke seeds returned `weak` with
every element marked `found` — including seed 1, where the claim genuinely was supported. The rule
is working as specified, and the model is exploiting the loose side of it. Noted, not changed:
tightening it now would convert a capacity problem into a validator problem and hide the former.

### D20 — two harness defects found while producing the Phase 5 report

1. **`--task` did not select the seed file.** `--cases` defaulted to a hardcoded
   `evals/citation_need.jsonl` regardless of `--task`, so `--task citation_support` scored the
   citation_need seeds and printed a complete, plausible, entirely meaningless report (8 cases, all
   NoEvidence, 0/8). Now defaults to `evals/{task}.jsonl`. This is the failure mode that looks like
   a result, and it would have silently invalidated any future task's first eval.
2. **`chunks_dropped` was printed but never recorded.** Scope item 1 requires the dropped count in
   the RESULT; it only reached stdout on the success path. `chunksSent` / `chunksDropped` /
   `evidenceWords` are now columns in the report JSON.

With that recorded: every citation_support case ran **sent=12, dropped=0** — 12 being `RETRIEVAL_K`,
so retrieval returned its cap and the budget dropped nothing. This is the measured confirmation of
D17's claim that the budget fix could not have moved the latency numbers on this fixture.

### The evidence budget is not the lever on prompt size

Measured: mean prompt 2198 tokens, of which the evidence block is ~640. **The schema and rules
scaffolding is roughly 1550 tokens — about 70% of every prompt**, paid on every case regardless of
how much evidence is sent. Cutting `EVIDENCE_BUDGET_TOKENS` further cannot meaningfully reduce
prefill; the scaffolding would have to shrink, or prefill has to get faster (Metal, §9.8). D14's
budget reasoning should be read with this in mind.


---

## Phase 6 — generative installer + model bake-off

### D21 — the bake-off is Qwen2.5-Instruct ONLY, because the loader is qwen2-specific

`src/models/quantized_qwen2_lowmem.rs` is a vendored qwen2 loader: it reads `token_embd.weight`,
`output_norm.weight`, `blk.N.attn_*` and the qwen2 GGUF metadata keys by name. It is not an
architecture-dispatching loader, and this phase does not add one.

*Consequence, stated so it is not rediscovered later:* **non-Qwen candidates are deferred until a
loader exists.** Llama-3.x-Instruct, Phi-3.5, Gemma-2 and Mistral are all plausible candidates at
these sizes and none of them can be evaluated here. A bake-off result of "Qwen2.5-3B is the best
model" therefore means "the best of the three Qwen2.5 sizes we can currently load", and any future
claim that it is the best model *available* would need a second loader first.

*Verified rather than assumed:* the loader already handles both tied and untied output embeddings
(`output.weight`, falling back to `token_embd.weight`), so 1.5B, 3B and 7B are all loadable within
the qwen2 family — the constraint is architecture, not size.

### D22 — the official 7B GGUF is SPLIT into two files; the optional 7B entry cannot use it as-is

`Qwen/Qwen2.5-7B-Instruct-GGUF` publishes Q4_K_M as
`qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf` + `-00002-of-00002.gguf`. The loader takes ONE
GGUF path, and multi-part GGUF assembly is not something this phase adds.

*Consequence:* the optional 7B entry is **not pinned in this phase**. Mandatory candidates (1.5B,
3B) are single-file and unaffected. If 7B is wanted after the 3B numbers, it needs either a
single-file Q4_K_M mirror pinned by revision and hash, or split-GGUF support in the loader — a
decision to take with the data, not now. Recorded because "7B optional" would otherwise look like a
switch to flip.

### D23 — the GGUF repos ship no tokenizer; it is pinned from each model's BASE repo

`Qwen/Qwen2.5-*-Instruct-GGUF` contains only GGUF files. The engine needs a `tokenizer.json`, so
each candidate pins one from its own base repo (`Qwen/Qwen2.5-1.5B-Instruct`, etc.) at a fixed
revision with its own sha256.

Pinning per-model rather than reusing the bundled 0.5B tokenizer is deliberate. The Qwen2.5 family
is *believed* to share a tokenizer, but "believed" is not a property to build a token-id mapping on:
a silent vocabulary difference would not crash, it would produce subtly wrong text. Each model
therefore carries its own verified tokenizer. If the hashes turn out identical across sizes, that is
a measured fact recorded after the fact, not an assumption relied on beforehand.


### D24 — citation_support-v2 requires a verbatim quote: the first check that ties PROSE to evidence

D18 recorded that the grounding guarantee covers identifiers only. v2 is the experiment against
that: every `supporting_chunks` entry must carry a `quote` of 5–25 words copied word-for-word from
that chunk, and the validator Fatals if it is not a substring of the text actually sent
(whitespace-normalised, nothing else).

**Why a quote and not an entailment check.** A quote is verifiable by string comparison — no second
model, no extra pass, no new failure mode of its own. It does not make `explanation` faithful and is
not claimed to. It forces the model to have located the supporting words, and it hands a reader the
exact text a verdict rests on.

**Normalisation is whitespace only, deliberately.** Folding case or punctuation would start
accepting near-misses, and a near-miss is exactly a paraphrase — the thing the check exists to
reject. Pinned by `a_paraphrased_quote_is_fatal`, which also asserts that v1 still ACCEPTS the same
output: that contrast is what the bake-off is measuring.

**The empty-string trap.** `""` is a substring of every string, so a missing-or-blank quote is
treated as missing rather than as a trivially valid one. Without that check the entire mitigation
would silently be a no-op — pinned by `an_empty_quote_is_treated_as_missing_not_as_a_valid_substring`.

`max_tokens` rises 400 → 600 for v2 only. Every cited chunk now carries up to 25 more words; leaving
it at 400 would truncate the JSON and score a formatting failure as a model failure.

**Unknown at the time of writing:** whether the extra field helps, costs latency, or simply produces
a new failure mode. Both variants run in the bake-off; the data decides.


### D25 — the faithfulness check FLAGS for human review; it never scores

The harness gains a D18 check for the seeds whose planted passage reports an ABSENCE ("No
significant effect of management was observed for earthworm abundance"; "Nitrogen leaching was not
measured"). It fires when an output cites that chunk AND its `explanation` uses language asserting
the finding exists ("increased", "supports the claim", "confirms", …).

**It is a string heuristic and is treated as one.** The count is reported, the case ids are listed,
and the verbatim output is printed — but it is deliberately excluded from every accuracy figure and
never presented as a faithfulness *rate*. A heuristic folded into a score becomes a target, and the
cheapest way to satisfy this one is to change wording rather than reasoning. Its job is to put a
human in front of the raw text.

*Known limits, stated rather than discovered later:* it cannot see a faithfulness failure that
avoids these words, it cannot judge subtle misattribution, and it does not read `why` or
`suggested_rewrite` — only `explanation`. It is a smoke alarm, not an audit.

**Seed id correction.** The Phase 6 instruction named cs-seed-03 as "the null-result seed". The
null-result passage is on **cs-seed-04** (the `contradicts` seed); cs-seed-03 is the
not-measured/`weak` seed. Rather than pick one, the check is data-driven from a `faithfulness` block
in the seed file and is enabled on BOTH — they are the two seeds whose evidence reports an absence,
which is the property the check needs.


## Phase 6a — repairing the citation_support bake-off harness

The 9-cell Phase 6 bake-off is **not** a valid support-model comparison. Four confounds were found
in its own output, all of them harness or design defects rather than model-quality findings, and
all four are fixed here before any model is chosen. The Phase 6 support cells stay on disk as the
record of the defects; they are never averaged or compared against the repaired series.

### D26 — the evidence header is LABELLED; the spec's `[c{id} | p.{page} | {section}]` is superseded

**SPEC OVERRIDE, engine-wide.** The spec pins `[c1 | p.8 | Results] text` as the evidence contract
for all eight tasks. It is superseded by an explicitly labelled form:

```
[CHUNK_ID=c1 PAGE=8 SECTION=Results] text
```

A missing page renders `PAGE=?`, a missing section `SECTION=-`, so the three-field shape still never
varies.

**Why, measured.** On the 3B, four of six seeds under BOTH prompt variants failed with
`chunk_id: "c13 | p.5 | -"` — the model echoed the whole bracket because the id's only marker was
*being first*. Position is not a label, and a bigger model reading the header as one composite
display string is a reasonable reading of the old format. The 0.5B and 1.5B never hit this because
they never got far enough; this defect was **hidden behind truncation** (D27) until a model large
enough to finish the JSON ran.

**The validator is NOT weakened.** A composite or display-form id remains fatal. The fix is on the
presentation side only: the model is shown a key it can copy, and is told in the rules that
`chunk_id` is the `CHUNK_ID=` value alone. Accepting `"c13 | p.5 | -"` as an alias for `c13` would
have destroyed the one guarantee the grounding check provides (D18).

**Applied in `task.rs::EvidenceChunk::render`**, so all eight tasks inherit it rather than each
task carrying its own header. `citation_need` is unaffected in fact as well as in principle: it has
no `<evidence>` block at all (D8), asserted by `a_prompt_never_claims_evidence_it_does_not_have`, so
its prompt is byte-identical across this change and its Phase 6 cells remain comparable and are
**not** rerun.

**Prompt versions bump: `citation_support-v1` → `v1.1`, `v2` → `v2.1`.** The rendering is part of
the prompt, so a report from either side of this change describes a different thing. The bumped
strings are what makes the two generations impossible to conflate in a report directory.

### D27 — the generation ceiling is raised from MEASURED output sizes, and truncation becomes a counted category

**Old ceilings: 400 (v1), 600 (v2, already raised once by D24).**

Measured across all 28 first attempts in the Phase 6 support cells, tokenised with the candidate's
own tokenizer:

| | at ceiling | largest COMPLETE first attempt |
|---|---|---|
| 0.5B | 10 of 10 | 286 (v1) — every v2 attempt truncated |
| 1.5B | 5 of 10 | 558 (v2) |
| 3B | 0 of 8 | 274 (v1), 209 (v2) |

**13 of 28 first attempts stopped exactly at the ceiling.** The support arm was measuring the
ceiling, not the models: a truncated reply is unparseable, and unparseable scores identically to
wrong.

**New ceilings: v1.1 = 768, v2.1 = 1024.** Derived, not guessed:

- The largest legitimately complete response observed is 558 tokens; 1024 is 1.8× that, and 768 is
  2.5× the largest complete v1 (309).
- A realistic correct response — three cited chunks, eight claim elements, a 120-word explanation —
  costs roughly 660 tokens in v2 and 420 in v1 by the schema's own limits.
- The absolute schema-legal maximum is **not** reachable and is deliberately not provisioned for:
  `supporting_chunks` is uncapped, so twelve retrieved chunks with 25-word quotes and 20-word
  rationales would need ~1400 tokens, and `TASK_N_CTX - max_tokens` would then leave less prompt
  budget than the prompts actually measure. Recorded so the gap is a known limit rather than a
  future surprise.

**Cost, stated because raising a ceiling is not free.** `generative.rs` enforces
`budget = TASK_N_CTX - max_tokens`, so every token given to the reply is taken from the prompt.
At v2.1 the prompt ceiling falls 4096−600=3496 → 4096−1024=3072; measured v2 prompts run
2035–2775 and the labelled header of D26 adds roughly 50 tokens, leaving ~250 tokens of headroom.
KV cache is sized by `TASK_N_CTX` and is unchanged. Latency rises only for replies that actually
use the room — decode is ~30 tok/s on the 0.5B and ~9 on the 3B, so a reply that grows from 400 to
768 tokens costs ~12 s more on the 0.5B and ~40 s on the 3B. That is the price of measuring the
model instead of the cap.

**Truncation stops being a one-time investigation.** `StopReason` already exists on `GenOutput`
(`EndOfTurn` / `MaxTokens` / `Cancelled`) but died inside `run_task`. It is now carried on `TaskRun`
and on the error path, per attempt, and recorded per case in every report. A model that hits the
ceiling still FAILS — the ceiling is not a repair, and nothing is accepted because it was
truncated — but the failure is now labelled with its cause forever.

### D28 — diagnostic faithfulness runs on REJECTED outputs, and can never accept one

D25 made the faithfulness check a flag for human review. Phase 6 then reported **0 violations of 2
checked in all nine cells**, and the number was worthless: `faithfulnessCheckedCases` only ever
counted accepted outputs, and the only two accepted outputs on the whole matrix were
`insufficient_evidence` and `NoEvidence` — both of which cite nothing. **The check never once
examined a citation.**

It hid a real failure. The 3B's first attempt on `cs-seed-01` under v2 returned `"verdict":
"strong"`, `"confidence": 1.0`, marked the claim element `by about 31 percent` as `found`, and
supported it with a quote containing no number at all. That output was rejected on the D26 chunk_id
defect, so the faithfulness checker never saw it.

**Two separate measures from here, never mixed:**

1. **Accepted faithfulness** — unchanged in meaning, still the D25 flag, still reported over
   accepted outputs only. This is the one that describes what the engine would have persisted.
2. **Diagnostic faithfulness** — the same checker run over outputs that PARSED and are
   schema-shaped but were REJECTED, when enough structured fields survive to check. Reported
   separately and never folded into any accuracy figure.

**A diagnostic result can never become an acceptance.** It runs after the verdict is already
`rejected`, reads a clone, writes nothing, and cannot clear a fatal error or reach persistence.
Pinned by a test that a rejected output carrying a clean faithfulness result is still rejected and
still not persisted.

### D29 — bake-off evidence is assembled with the REAL embedder; the mock is CI-only

Every Phase 6 support cell assembled its evidence with `mock-lexical-v1`, a hashed-bag-of-words
stand-in living in the harness. Retrieval feeds the prompt, so the whole support arm was measuring
the models against evidence a real install would never have selected.

The repaired series embeds the fixture corpus with the production embedder —
`bge-small-en-v1.5`, CLS pooling, `bge-v1.5-p2` (§11 D6) — and records `model_id` and
`preprocessing_version` in every report, so a report can never again be read without knowing which
retrieval produced it. The mock remains, unchanged, for CI: those tests must not need a 134 MB
download.

**If the real embedder selects different chunks than the mock did, that is recorded as a finding.**
It changes what the models were asked about, which is precisely why the Phase 6 support cells and
the repaired series can never be compared.

### Phase 6a results — the repaired citation_support series

Six cells, one binary (`d4629fd4dbafa9ea410feb7ca50a915d3081f2f4bae42823ee2c39c62ba5943e`), real
embedder throughout (`bge-small-en-v1.5` / `bge-v1.5-p2`). Reports carry the `-repaired` suffix.
**The Phase 6 support cells are not comparable to these and must never be averaged with them** —
different prompt generation (v1.1/v2.1), different ceilings, different retrieval.

| model | prompt | valid | fatal | retry | trunc | chunk_id fmt | planted cited | accepted faith. | diagnostic faith. |
|---|---|---|---|---|---|---|---|---|---|
| 0.5B | v1.1 | 1/6 | 83% | 83% | 2 | 0 | — | 0/0 | 0 |
| 1.5B | v1.1 | 2/6 | 67% | 67% | 1 | 0 | — | 0/0 | 3 |
| 3B | v1.1 | **6/6** | **0%** | **0%** | **0** | 0 | **75%** | **1 of 2** | 0 |
| 0.5B | v2.1 | 2/6 | 67% | 67% | 4 | 0 | 100% | 0/1 | 0 |
| 1.5B | v2.1 | 1/6 | 83% | 83% | 1 | 0 | — | 0/0 | 1 |
| 3B | v2.1 | 3/6 | 50% | 50% | 0 | 0 | **100%** | **1 of 1** | 0 |

**D26 is confirmed, and it was the dominant defect.** `chunkIdFormatFailures` is **0 in all six
cells**. The 3B went from 2/6 valid with four chunk_id failures to **6/6 valid with none** under
v1.1 — the same model, the same seeds, a labelled header. Phase 6's "the 3B is worse at
citation_support than the 1.5B" was an artefact of the display format.

**D27 is confirmed and partially insufficient.** The 3B never approaches either ceiling. The 0.5B
still truncates 2/6 at 768 and **4/6 at 1024** — raising the ceiling did not fix the 0.5B because
the 0.5B's failure is not length, it is runaway enumeration: it cites five, eight, twelve chunks
with a full `why` on each. `supporting_chunks` being uncapped (D27) is the actual mechanism.
Raising the ceiling further would spend context on a pathology; a chunk-count cap would be the
honest fix, and it is a schema change this phase deliberately did not make.

**D28 is confirmed, and it earned its keep twice over.**
1. *Accepted* faithfulness inspected a real citation for the first time in the project's history —
   and found a violation immediately, on the 3B, in **both** variants (cs-seed-04 under v1.1,
   cs-seed-03 under v2.1). The 3B cites the chunk that reports an ABSENCE and then writes an
   explanation asserting the finding with "higher". Nine Phase 6 cells reported "0 violations" and
   the check had examined nothing.
2. *Diagnostic* faithfulness recovered four findings from REJECTED outputs (three on 1.5B v1.1, one
   on 1.5B v2.1) that the old harness discarded entirely — including the 1.5B marking
   `by about 31 percent` as `found` while citing five chunks, none containing "31".

**D29 recorded, with no retrieval surprise.** Every cell reports `sent=12 dropped=0` under the real
embedder, as under the mock. The bake-off's retrieval was not what was broken — but it is now
recorded in every report, so this is a measurement rather than an assumption.

**A harness defect found while producing this report, in the D20 tradition.**
`decodeTokensPerSec` is hardcoded `0.0` at all four `CaseResult` sites in the citation_support arm
(the citation_need arm computes it correctly). Pre-existing, not introduced here. `tokens` and
`decodeMs` are both recorded, so decode rate is fully recoverable by arithmetic and no data was
lost — the figures in the Phase 6a report are computed that way. Fixing the field requires an
ai-eval change and therefore a new binary, which would have split this series; it is left for the
next harness commit rather than done mid-matrix.

**citation_need was NOT rerun, by verification rather than assumption.** It has no `<evidence>`
block (D8) and its prompt never calls `EvidenceChunk::render`, asserted by
`a_prompt_never_claims_evidence_it_does_not_have`. The D26 change cannot reach it, so its Phase 6
cells remain valid and the two arms are not mixed generations.


## Phase 6b — closing the three recorded defects

### D30 — MODEL DECISION: Qwen2.5-3B-Instruct-Q4_K_M, conditionally

**The production candidate is `qwen2.5-3b-instruct-q4km`.** It is the only one of the three
loadable sizes that can do citation_support at all: 6/6 valid with no retries and no truncation
under v1.1, 75–100% planted-chunk citation, against 1–2/6 for both smaller models under either
variant.

**The decision is CONDITIONAL on two things that do not yet exist, and is not final until both do:**

1. **Metal acceleration bringing citation_support latency into usable range.**

   **AMENDED (Phase 6c).** This condition originally cited 174 s (v1.1) and 212 s (v2.1). **Those
   figures are CONTAMINATED and are withdrawn** — both cells were measured while
   `cargo clippy --workspace` and the test suite ran on the same machine, after it had been doing
   continuous CPU inference for over an hour. Prefill cost 95.3 ms per prompt token there against
   34–47 ms in every later run on near-identical prompts, and nothing in the engine can cause that.

   **Provisional baselines, isolated:** **~63–74 s** for the v1 lineage and **~154–160 s** for the
   v2 lineage (Phase 6b/6c, three of the four cells declared isolated). Still not a shippable
   interaction, so the conditional STANDS and prefill is still the lever (§9.8, D12) — but the gap
   Metal must close is roughly a third of what this decision first recorded.

   **Neither figure is a controlled measurement.** Pre-Metal latency gets ONE dedicated isolated
   measurement — a quiet machine, repeated runs, load context recorded (D33) — before it decides
   anything. If Metal does not move it, this decision is reopened, not worked around.
2. **Validation on the 50-case labeled set.** Six seeds on one fixture chose this candidate. Six
   seeds cannot confirm it. Everything below is provisional on that data.

**Operating point: the v1 lineage — no mandatory quote.** Stated as v1.1 when the decision was
taken; the chunk cap (D32) moves it to **v1.2**, which is the same prompt lineage plus a bound. The
v2 quote requirement (D24) is **not rejected, it is deferred**: it halves the 3B's acceptance
(6/6 → 3/6) for ~40 s more, and both rejections it causes are verbatim-quote failures — D24's check
working, not a model regression. It is re-evaluated after the chunk cap and after Metal, when the
latency budget it spends is a different number.

**The 1.5B remains the registry's fallback for constrained machines** (`needs_16gb: false` applies
to both, but the 3B's 2295 MB estimate against the 1.5B's is the real constraint). Pending the same
labeled-set data — its 1–2/6 here is a measurement on six seeds, not an established ceiling.

**What this decision explicitly does NOT claim** (D21, restated because it is the easiest thing to
lose): "the best model" here means the best of three Qwen2.5 sizes, because the loader is
qwen2-specific. Llama-3.x, Phi-3.5, Gemma-2 and Mistral were never candidates. A claim that the 3B
is the best model *available* needs a second loader first.

### D31 — RESOLVED: `decodeTokensPerSec` was hardcoded `0.0` in the citation_support arm

Logged as a defect while producing the Phase 6a report and fixed here.

All four `CaseResult` sites in the support arm wrote a literal `0.0` while citation_need computed
the rate inline, so **every citation_support cell ever produced reported a decode rate of zero** —
including the six repaired cells. Two of the four sites (NoEvidence, generation error) are
zero-generation paths where `0.0` happened to be correct; the accepted and validation-failed sites
were discarding real measurements.

No data was lost: `tokens` and `decodeMs` are recorded per case, and the Phase 6a figures were
computed from them by hand. The fix makes that arithmetic the harness's job.

Both arms now call one `decode_tps(tokens, decode_ms)` helper. The duplicated inline expression is
what allowed the two arms to drift apart in the first place, so it is gone rather than copied a
third time.

### D32 — `supporting_chunks` is capped at 4, and the cap is FATAL

D27 recorded that `supporting_chunks` is uncapped and named it the mechanism behind the 0.5B's
truncation. Phase 6a confirmed it: raising the ceiling 400 → 768 → 1024 did not help the 0.5B,
which still truncated **4 of 6 seeds at 1024** by citing five, eight, twelve chunks with a full
`why` on each. Length was never the problem. Unbounded enumeration was.

**Four entries, Fatal above.** The cap bounds output size *by construction* rather than by giving
the pathology more room, which is what two ceiling raises amounted to.

**Fatal rather than advisory, deliberately.** A card listing a dozen chunks is not untidy — it is a
claim that twelve passages support one sentence, and a reader cannot check that. That is exactly
the class the tier split calls Fatal: it can make an unsound citation look sound. Four is enough to
carry a genuine multi-passage justification and few enough that a human can verify each one.

Stated in the prompt (`supporting_chunks: AT MOST 4 entries`), in both output schemas, and enforced
in `validate_support`, so **v2 inherits it** rather than carrying a copy. Pinned by
`four_supporting_chunks_pass` — a bound that rejected its own legal maximum would quietly be a
bound of three — `five_supporting_chunks_are_fatal`, and `the_cap_applies_to_v2_as_well`.

**PROMPT_VERSION bumps v1.1 → v1.2, v2.1 → v2.2.** The cap is stated in the prompt, so this is a
different prompt and its reports are a different generation.

**This is a SPEC ADDITION, not an override.** The spec places no ceiling on `supporting_chunks`; it
does not state one this contradicts. Recorded here because a reader comparing the spec's schema to
the implementation would otherwise find a rule with no provenance.

### Phase 6b confirmation rerun — the cap did NOT confirm clean on the 3B

Two cells, one binary (`af09a9697f2375b9bafb891b8d217358db843b5a70f3baee8e555cf2ad565dce`), real
embedder, `-capped` suffix.

| metric | v1.1 | **v1.2** | v2.1 | **v2.2** |
|---|---|---|---|---|
| valid | 6/6 | **6/6** | 3/6 | **2/6** |
| fatal rate | 0% | 0% | 50% | 67% |
| retry rate | 0% | 0% | 50% | 67% |
| advisory rate | 50% | 67% | 0% | 0% |
| truncation | 0 | 0 | 0 | 0 |
| chunk_id fmt | 0 | 0 | 0 | 0 |
| verdict agreement | 40% | 40% | 100% | 100% |
| **planted cited** | **75%** | **25%** | 100% | — |
| accepted faithfulness | 1/2 | 0/2 | 1/1 | 0/0 |
| decode tok/s | 2.74 | 6.85 | 3.77 | 5.67 |

**THE CAP NEVER FIRED.** The 3B's cited-chunk counts under v1.1 were 2, 4, 2, 3, 0 — never above
four. Under v2.1: 2, 1, 1, 1, 0. The bound is therefore **untested on the model being selected**,
and validated only against the 0.5B pathology it was designed for, which this phase did not rerun.

**Every observed change came from the PROMPT LINE, not the bound.** Decoding is greedy argmax
(temperature 0, top_p 1), so the same prompt yields the same output deterministically. Adding
`supporting_chunks: AT MOST 4 entries. Cite only the chunks that actually carry the point. Listing
every chunk you were given is not evidence.` changed the 3B's behaviour, and on this fixture it
changed it for the worse:

- **v1.2 — grounding regression.** Planted-chunk citation fell **75% → 25%** (3 of 4 → 1 of 4).
  Validity is unchanged at 6/6 and verdict agreement unchanged at 40%, so this is invisible in
  every metric except the one item 8 named primary. cs-seed-03 and cs-seed-04 both stopped citing
  the planted passage; cs-seed-04's cited count fell 3 → 2. The accepted faithfulness violation
  also disappeared — not because the model got more faithful, but because it stopped citing the
  absent-finding chunk it was previously misdescribing.
- **v2.2 — validity regression.** 3/6 → 2/6. cs-seed-03 flipped from accepted to failed: it cited
  two chunks instead of one, and the second's quote was not verbatim.

**Recommendation, not applied:** keep the bound and narrow the prompt to state ONLY the bound,
dropping the second sentence. The bound is correct by construction for the 0.5B and costs the 3B
nothing (it never binds); the editorialising guidance is what moved the 3B off the planted chunk.
That is a one-line change and a two-cell rerun, and it is a decision to take with this data rather
than silently.

### A latency measurement correction that affects D30

**The Phase 6a 3B latency figures are contaminated and must not be used.** Prefill cost per prompt
token, on near-identical prompts:

| | Phase 6a | Phase 6b |
|---|---|---|
| v1 lineage | 95.3 ms/tok | **34.4 ms/tok** |
| v2 lineage | 88.7 ms/tok | **44.9 ms/tok** |

A chunk cap cannot make prefill *per token* 2–2.8× faster. The Phase 6a 3B cells were measured
while `cargo clippy --workspace --all-targets` and the test suite were running on the same machine,
on a box that had by then been doing continuous CPU inference for over an hour. The Phase 6b cells
ran with nothing else scheduled.

**Consequence for D30:** its Metal condition was reasoned from 174 s / 212 s. The cleaner numbers
are **63 s (v1.2)** and **154 s (v2.2)**. Metal is still the right lever and the conditional stands
— 63 s is not a shippable interaction either — but the gap it must close is roughly a third of what
D30 records, and **neither figure is a controlled measurement.** Latency deserves a dedicated
isolated run before it decides anything.


## Phase 6c — the prompt line, and a guard against measuring noise

### D33 — the prompt states the bound and nothing else; load context is recorded in every report

**The prompt half of D32 is reverted to a bare statement of the bound.** The validator and schema
are unchanged — four entries, Fatal above. What is gone is the second sentence, *"Cite only the
chunks that actually carry the point. Listing every chunk you were given is not evidence."*

Decoding is greedy argmax, so prompt text is causally testable, and this sentence was measured
moving the 3B off the planted passage. The bound belongs in the validator; the prompt's job is to
state it, not to argue for it. `PROMPT_VERSION` v1.2 → **v1.3**, v2.2 → **v2.3**.

**It recovered half the loss, and half is the finding.** Planted-chunk citation on the 3B:

| | v1.1 (no bound stated) | v1.2 (bound + guidance) | v1.3 (bound only) |
|---|---|---|---|
| planted cited | **75%** (3/4) | 25% (1/4) | **50%** (2/4) |

cs-seed-03 recovered; **cs-seed-04 did not.** The residual gap is not the wording — v1.3's line is
purely factual — it is *the presence of the statement at all*. On a 3B under greedy decoding, adding
any line to the rules block reorders what the model cites. v1.3 also changed cs-seed-05, which now
cites four chunks while returning `insufficient_evidence` (legal under the current rules, and odd).

**v2 did not recover at all**: 3/6 → 2/6 → **2/6**. Every v2.3 failure is the same one, the D24
verbatim-quote check, on chunk c13 — the model paraphrases the quote it claims to be copying. That
is orthogonal to the cap and to its wording, and it is the thing to look at if the v2 lineage is
ever revived.

**Honest scope of all of this: the planted metric has a denominator of 4.** 75% / 50% / 25% are
3, 2 and 1 seeds. These are single-seed differences on one fixture, and they are reported because
they are the primary metric, not because four seeds settle anything. The 50-case labeled set (D30)
is what would.

### The load-context rule

Every citation_support report now carries `loadContext`: the 1-minute load average at cell start
and end, and a `ranIsolated` flag set by `--isolated`.

**TIMING FIGURES FROM CELLS WITH DIFFERENT LOAD CONTEXT ARE NOT COMPARABLE.** That is a rule, not a
caveat, and D30's withdrawn figures are why it exists.

Two limits, stated so the guard is not trusted further than it earns:

1. **`ranIsolated` is a DECLARATION, not a measurement.** It records that the operator scheduled
   nothing else. The load averages are the evidence for or against it.
2. **The end-of-cell load average is dominated by the cell itself.** Inference *is* the load: the
   v1.3 cell ran 1.82 → 3.66 while genuinely isolated. The START value is the useful signal —
   "was the machine quiet when this began" — and even that is weak for back-to-back cells: v2.3
   started at 3.66 because v1.3 had just finished. **By this rule's own terms, v1.3 and v2.3 are
   not strictly comparable to each other on timing.**


## Phase 6d — the untested configuration, and the end of prompt iteration

### D34 — v1.4 is the operating configuration: v1.1's prompt with the Fatal bound

**The prompt says nothing about the chunk bound. The validator still rejects a fifth chunk.**
That configuration had never been measured: D32 introduced the bound in the prompt and the
validator together, so "bound enforced but unmentioned" existed only as v1.1, which predates the
validator.

**The v1.4 prompt is byte-identical to v1.1's, verified rather than asserted.** All five
prompt-contributing constants (`SYSTEM`, `OUTPUT_SCHEMA`, `RULES`, `QUOTE_RULE`,
`OUTPUT_SCHEMA_V2`) and both `build_prompt` bodies were diffed against the tree at `69b232f`, the
last commit where v1.1 was current: identical, all seven.

Removing the statement touched **both** variants — `RULES` is shared by v1 and v2, and the
`// at most 4` annotation sat in both schemas — so v2 bumps to **v2.4** even though it is deferred
and was not run. A deferred variant carrying a stale version label is exactly the conflation the
version discipline exists to prevent.

**The prediction was exact.** Greedy decoding plus an identical prompt should reproduce v1.1's
outputs byte for byte. It did — all six seeds, `raw` identical, across two binaries built from
different source trees on different days under different load:

| seed | outcome | verdict | planted cited | raw bytes |
|---|---|---|---|---|
| cs-seed-01 | ok | weak | true | identical |
| cs-seed-02 | ok | weak | false | identical |
| cs-seed-03 | ok | weak | true | identical |
| cs-seed-04 | ok | weak | true | identical |
| cs-seed-05 | ok | insufficient_evidence | — | identical |
| cs-seed-06 | ok | (NoEvidence) | — | identical |

6/6 valid, verdict agreement 40%, **planted-chunk citation 75%**, one accepted faithfulness
violation (cs-seed-04), advisory rate 50% — every cell-level figure matching v1.1.

This closes the causal chain from 6b and 6c: **the entire v1.2/v1.3 grounding regression was the
prompt text and nothing else.** No binary difference, no retrieval difference, no nondeterminism.

**The full measured picture, one model, one fixture, greedy:**

| prompt configuration | planted-chunk citation |
|---|---|
| bound + guidance (v1.2) | 25% |
| bound stated plainly (v1.3) | 50% |
| **bound not mentioned (v1.1 / v1.4)** | **75%** |

Stating the bound cost grounding; arguing for it cost more. The validator does not need the model's
cooperation to enforce a maximum — it simply rejects — so the prompt line bought nothing and
demonstrably charged for it. Pinned by `the_prompt_never_mentions_the_chunk_bound`.

### PROMPT ITERATION ON THE 6-SEED FIXTURE IS CLOSED

The primary metric has a denominator of **four**. Every difference this phase turned on was one or
two seeds. That was tolerable for finding a defect as large as D26; it is not a basis for choosing
between configurations, and continuing would be fitting a prompt to four data points.

**No further prompt or config change to citation_support until the 50-case labeled set exists**
(D30's second condition). At that denominator a primary metric can carry a decision. Until then the
operating configuration is frozen at v1.4.

### The v2 lineage is DEFERRED, with its failure mode recorded

v2 is not rejected and not iterated on. Its measured failure is specific and unchanged across
v2.1 → v2.4: **the 3B paraphrases the quote it claims to be copying.** Every v2.3 failure was
D24's verbatim-quote check on chunk c13 — the model produces a fluent, accurate-sounding sentence
that is not a substring of the evidence.

That is not a prompt-surgery problem. It is either a capability limit at 3B or a task that wants a
stronger model, and the honest next tests are the labeled set or a larger candidate — **not more
wording changes.** D24's check is working exactly as designed; it is catching a real
paraphrase-for-quote substitution, which is precisely the failure it was built to catch.

### Pre-Metal latency baseline

**v1.4, isolated, start load 1.89: mean 65.4 s per case, prefill 35.8 s (55%), 998 prompt tokens,
0 truncations, 0 retries.** Binary `9fafab7582d484ca13aa0a0a0135094eca04be472358b5b8dbbc6eb7d12d763e`.

This is the number Metal has to move, and it supersedes every earlier figure for the operating
configuration. It remains a single isolated run on one fixture — D30's requirement of one dedicated
latency measurement still stands.


## Phase 7 — Metal feasibility: BLOCKED on this machine's OS, not on candle

### D35 — candle 0.11 implements everything this workload needs on Metal, and cannot run it on macOS 14

**Investigated in the registry sources at the pinned versions, not from documentation.**

**The capability is there.** Every operation the citation_support path needs has a real Metal
kernel in `candle-core 0.11.0`:

| op | our use | Metal path |
|---|---|---|
| quantized matmul | every attention/FFN projection | `call_quantized_matmul_mm_t` / `_mv_t` |
| quantized get_rows | `QMatMul::embedding` — the low-mem token table | `call_quantized_get_rows` |
| `softmax_last_dim` | attention | `call_last_softmax` |
| `rms_norm` | every block | `call_rms_norm` |
| `silu` | FFN | `usilu` |
| `gelu_erf` | the BGE embedder (`hidden_act: gelu`) | `ugelu_erf`, contiguous + strided |

The 3B GGUF was inspected directly rather than assumed: **435 tensors, exactly three dtypes — Q4K
(217), F32 (181), Q6K (37)** — and all three are dispatched in `quantized/metal.rs`.

**The no-C-toolchain constraint holds.** Enabling `metal` adds 21 crates. Only `pulp` has a build
script, and it is pure-Rust codegen emitting `core::arch::global_asm!` with `version_check` as its
sole build-dependency. `candle-metal-kernels` declares `build = false` and compiles its `.metal`
shaders AT RUNTIME from `include_str!`d source via `new_library_with_source`. No `cc`, no bindgen,
no offline shader compiler. The build was run and it succeeds.

### The blocker: `MTLResidencySet` is macOS 15+, and the binding PANICS

`MetalDevice::new` constructs a residency set unconditionally
(`metal_backend/mod.rs:2031`), and every buffer allocation calls `residency_set.insert`.
`MTLResidencySet` / `MTLResidencySetDescriptor` are **macOS 15.0+** API. **This machine runs macOS
14.5** (Darwin 23.5.0, Apple M1, Metal 3).

candle *intended* this to degrade: `ResidencySet` holds `raw: Option<...>`, `insert` is guarded by
`if let Some(set)`, and `newResidencySetWithDescriptor_error(...).ok()` tolerates failure. **The
guard is one line too late.** `ResidencySet::new` first calls `MTLResidencySetDescriptor::new()`,
and objc2's generated binding *panics* on a missing class:

```
panicked at objc2-metal-0.3.2/src/generated/MTLResidencySet.rs:10:1:
class MTLResidencySetDescriptor could not be found
```

Two consequences worth stating separately:

1. It is a **panic, not an `Err`**, so a `Result`-based CPU fallback cannot catch it. Any shipped
   Metal path would need `catch_unwind` or an OS-version gate *before* probing.
2. It fires at **device creation**, before any tensor work — so nothing about the rest of the
   integration was ever exercised on this machine.

### Routes, with what each actually costs

| route | viable? | cost |
|---|---|---|
| **macOS 15+** | yes — the code path is complete | an OS upgrade; not a code decision |
| **newer candle** | **no such thing** — 0.11.0 IS the latest published | — |
| **older candle (0.10.2)** | no residency sets, Q4K matmul present — **but `QTensor::embedding` does not exist at all in 0.10.2** | it is a 0.11 addition, and the low-mem loader is built on it. Reverting means `dequantize()` at load time, which is exactly what OOM-killed the 8 GB perplexity probe and the reason the vendored loader exists |
| **patch candle-metal-kernels** | yes, technically | a one-line class-availability check — and a fork to carry and re-apply on every upgrade |
| **ship Metal gated to macOS 15+** | yes | correct for other users; **zero speedup on this machine**, so it cannot be measured here |

**No Phase 7 code was landed.** The device abstraction and the Cargo change were written, compiled,
and then reverted: shipping an unmeasurable acceleration path would be worse than not having one.

**D30's Metal conditional is unaffected in principle and unresolvable in practice on this hardware.**
The pre-Metal baseline stands at **65.4 s** mean (v1.4, isolated, §11 D34), prefill 55%. Whether
Metal closes that gap remains untested — not disproven.

### D36 — Metal is a macOS 15+ feature; CPU is the floor everywhere else

The gate D35 said was missing. **No other Phase 7 work has landed** — the device abstraction is
wired to nothing yet, by decision: STEP 2 stays parked until Metal can actually be measured.

**Metal is probed ONLY behind an availability check evaluated first:**

```text
force-CPU env? ──yes──> CPU
      │no
gate: is MTLResidencySetDescriptor registered? ──no──> CPU   (the macOS 14 path)
      │yes
probe inside catch_unwind ──panic/Err──> CPU
      │Ok
     Metal
```

**Why a class lookup rather than a version parse.** `AnyClass::get` returns `Option` and cannot
panic, and it tests the *precise* thing that fails rather than a proxy for it. If Apple ships the
class in a different release than expected, or a future candle stops needing it, the gate keeps
testing the right thing without an edit. A control lookup (`MTLCaptureDescriptor`, 10.15+) runs
first so "Metal isn't loaded in this process" cannot be misreported as "your OS is too old".

**Why `catch_unwind` as well.** The gate is the correctness mechanism; the belt is for the failure
nobody has been taught yet. D35's panic came from one line *inside* candle's own
graceful-degradation path — `raw: Option<...>` was guarded, the descriptor construction above it
was not. That is a good reminder that a defence which only handles anticipated failures is not a
defence. `catch_unwind` is not used as control flow: on the expected paths it never fires.

**CPU IS THE FLOOR.** Metal is an acceleration, never a requirement. An engine that refuses to
start because the GPU is one release behind is worse than a slow engine — and on this machine
(macOS 14.5) the gate closes and everything works, slowly, exactly as it did before Phase 7.

**Tested against the real failure, not a mock of it.** `a_forced_open_gate_cannot_panic_the_caller`
forces the gate open on this macOS 14.5 machine, which makes the probe genuinely panic, and asserts
the caller receives a working CPU device. That test *proves* the belt rather than describing it.
It is written to pass on both sides of the boundary: on macOS 15+ the probe simply succeeds.

`the_real_gate_agrees_with_the_runtime` asserts the gate matches what the ObjC runtime actually
reports, rather than hardcoding today's answer — a test that starts failing the day the machine is
upgraded would be a landmine, and the property worth pinning is agreement with reality.

**The C-toolchain constraint re-verified after re-adding the dependencies:** 22 crates added, only
`pulp` carries a build script (pure-Rust `global_asm!` codegen, `version_check` its sole
build-dependency), and no crate in the set pulls `cc`, `bindgen` or `cmake`. `gaply-core` is
untouched.


## Phase 8 — the batch job system and the thesis citation audit

An audit is 50–250 sequential model calls at ~65 s each on CPU (§11 D34). That is **hours**, on a
desktop app the user can quit. Crash-resume, streaming and cancellation are not robustness polish
here — they are the feature, and everything below is shaped by that.

### D37 — `ai_job_items` is REBUILT in v16, not extended

v14 keyed every item to a chunk: `chunk_id INTEGER NOT NULL`, `UNIQUE (job_id, chunk_id)`. The
thesis audit's unit of work is a **sentence**, and a chunk holds many of them, so both constraints
block the feature outright — this is not a missing column, it is a wrong grain.

**Rebuilt rather than ALTERed, following the precedent v15 set** when it recreated
`ai_chunk_embeddings` rather than contort around SQLite's inability to add a NOT NULL column: the
table is **empty in every install that exists**. `ai_jobs` and `ai_job_items` were created in v14
and *no code has ever written to them* — Phase 8 is their first consumer, verified by grep before
the migration was written. Rebuilding an empty table costs nothing and leaves a clean shape;
carrying a nullable-chunk-plus-sentence-column hybrid would have left the wrong grain visible
forever.

What v16 adds, and why each is load-bearing:

| column | why |
|---|---|
| `seq` + `UNIQUE (job_id, seq)` | deterministic order AND idempotent item creation — re-running the planner on resume cannot duplicate rows |
| `kind` | `citation_need` / `citation_support` / `unverifiable`, so the summary is a query rather than a re-derivation |
| `sentence`, `page` | the unit of work and its provenance, kept with the item so a report needs no re-parse |
| `result_json` | results are queryable INCREMENTALLY, mid-job, which is the streaming requirement |
| `chunk_id` nullable | a `citation_need` sentence has no evidence chunk; NOT NULL would have forced a lie |

`ai_jobs` gains `summary_json` additively (ALTER) — the Thesis Health report, written once at
completion.

### D38 — resume is "reset stale running, select not-done", and it is safe because ONE runner exists

On restart, any item left `running` is reset to `queued` and re-run. That is only correct because
the single-inference invariant (§7) means there is exactly one runner: a second one would reset an
item another was actively working.

**Zero duplicate results follows from one transaction, not from care.** An item's result, its
`status='done'`, and the job's `done_items` increment are written in a SINGLE transaction. There is
no window where a result exists without the status that retires it:

- crash mid-inference → `running`, `result_json` NULL → reset, re-run, one result
- crash after commit → `done` → never selected again
- crash between the two → **cannot happen**; there is no between

The item is claimed with `UPDATE ... SET status='running' WHERE id=? AND status='queued'`, and a
zero-row update means someone else took it — the claim is the lock.

### D39 — preemption is a counter the runner reads, not semaphore fairness

The obvious implementation is to rely on `tokio::sync::Semaphore` being FIFO: an interactive
request queues, the runner's next item queues behind it, the human wins. That is *probably* true
and it is not something to build a user-visible guarantee on — it depends on the fairness of a
dependency, it is invisible in the code, and it cannot be tested without racing.

Instead: an explicit `AtomicUsize` of in-flight interactive requests. The runner checks it
**between items** and waits while it is non-zero. Consequences stated plainly:

- A job **never** yields mid-inference. Killing a half-finished generation to start another wastes
  the 65 s already spent and produces nothing — the human waits at most one item.
- The counter is incremented before the interactive request touches the gate and decremented after
  it releases, so the window is never smaller than the actual request.
- Embedding backfill sits below both by checking the same counter *and* whether a job is running.

### D40 — the pre-pass is deterministic and runs NO model, and `unverifiable` is a RESULT

Sentence segmentation (`extract::sentence`), citation-marker detection, and library matching happen
before any inference. A 250-item audit that discovers at item 200 that the manuscript was
unparseable has wasted three hours.

**`unverifiable: source not in library` is a report category, not an error.** A cited work whose
source is not indexed and embedded cannot be checked against evidence — that is a true and useful
finding about the manuscript, and recording it as a failure would both misreport the audit and
retry something that cannot succeed. It costs zero model calls.

The significance filter (skip headings, the references section, sentences under 6 words) exists to
keep the item count honest. At 65 s per item the difference between filtering and not is measured
in hours.


## Phase 9a — the in-app PDF viewer that D18's affordance requires

### D41 — D18's "verify against the source" needs a viewer that did not exist

D18 fixed the boundary: the grounding guarantee covers **identifiers**, not prose. A stored card
cannot cite a passage that was not retrieved, but nothing ties `explanation` to the evidence text —
so the UI rule that follows is that AI prose is never rendered without its evidence beside it, with
an affordance to check the source.

**Phase 9 exploration found there is no viewer to check it in.** Verified rather than assumed:

| checked | finding |
|---|---|
| `react-pdf` | used only in `src/components/seo-guides/` — marketing pages, not the app |
| `pdfjs-dist` | used only in `analysis/validateFile.ts`, to count pages during validation |
| Plagiarism / AI Check / Statistical viewers | render excerpts inline; none opens a source file |
| `@tauri-apps/plugin-opener` | one use, `openUrl` for OAuth. `openPath` hands a file to the OS default app and **takes no page argument** |
| `export_publishready_pdf` | writes to temp and hands off to the OS — the existing "show a PDF" pattern, and not an in-app viewer |
| grep `pageNumber` / `scrollToPage` / `gotoPage` | zero hits in app code |

**Premise corrected:** Phase 9 was scoped as "open the existing PDF viewer at that page". There is
no existing viewer, and the one file-opening primitive cannot target a page. Building the evidence
component on a stubbed affordance would have shipped D18's central requirement as decoration across
every AI surface at once, so the viewer becomes its own phase and the rest of Phase 9 follows it.

### D42 — the PDF is read in RUST, not by the frontend

The obvious frontend implementation is `plugin-fs` + `readFile(path)`, or Tauri's asset protocol.
Both were rejected on inspection of what they would cost:

- The fs capability grants `fs:allow-write-file`, `-exists`, `-mkdir`, `-remove` scoped to
  **`$APPDATA` only**, and does not grant `fs:allow-read-file` at all. A thesis lives in
  `~/Documents`, so reading it from the webview would need a new permission AND a scope widened to
  the user's filesystem — a large, permanent grant bought for one screen.
- The asset protocol needs the same scope widening plus a CSP change.

The backend already has filesystem access, so it reads the bytes and hands them over IPC. **No new
frontend permission, no CSP change, no asset protocol.** The narrow grant stays narrow.

*Cost, stated:* a large PDF crosses IPC once per open. That is a real cost and the reason the
viewer streams pages lazily rather than holding rendered output for a 400-page document.

### D43 — the pdf.js worker is BUNDLED, never the CDN the marketing pages use

`seo-guides/EthicalAiPdfDeck.tsx` sets
`pdfjs.GlobalWorkerOptions.workerSrc = "https://unpkg.com/pdfjs-dist@.../pdf.worker.min.mjs"`.

That is fine for a marketing page on the web and **wrong twice** for the desktop app: the CSP is
`script-src 'self'` / `worker-src 'self' blob:`, so it would be blocked; and R4 says the AI layer
makes no network calls but a viewer fetching its worker from a CDN would make the app require the
network to read a local file. The worker is bundled from the installed `pdfjs-dist` instead.

### D44 — `ai_model_status` reports the active device

Phase 7 built `ai::device::select()` and wired it to nothing, so the UI had no honest way to say
whether inference was running on CPU or Metal. Added as one additive field. On any machine below
macOS 15 it reads `cpu`, correctly, because the D36 gate closes there.

### D45 — `ai_citation_document`: the link table needs a read path for the UI

Phase 8b built `citation_documents` and `checkable_document_for_citation`, but
exposed neither to the frontend. The citation panel therefore had no way to
answer "which indexed document backs this citation?", so its support check was
mounted with `documentId = null` and could never run — the feature was reachable
in code and unreachable in the product.

One additive command, returning the linked document that is actually checkable
(indexed AND embedded) or `null`. `null` is a first-class answer: it is what
drives the panel's "this citation's source is not linked to an indexed document"
state, which is a true and actionable thing to tell someone.

### D46 — `default-run = "app"`: the eval harness made the package multi-binary

Adding `src/bin/ai-eval.rs` in Phase 4 left `cargo run` with two candidates, so
`npm run tauri dev` failed with *"could not determine which binary to run"*. The
app is the default; `ai-eval` stays explicit via `--bin ai-eval`.

### D47 — the HMR socket is a `devCsp` entry, so the shipped CSP cannot inherit it

`tauri dev` serves the frontend from `http://localhost:3000`, but the WKWebView
never picked up a frontend edit: CRA recompiled and served the new chunk, while
the window went on executing whatever it loaded at startup. Cause is the CSP —
`connect-src` named `'self'` and `wss://*.supabase.co` but no `ws://localhost:3000`,
and WebKit does not read `'self'` as covering the `ws:` scheme, so
webpack-dev-server's HMR socket was blocked. The only way to see a frontend
change was to relaunch the app (`touch src-tauri/src/lib.rs`), which turned every
UI check in a verification pass into a full rebuild.

The relaxation is DEV-ONLY BY CONSTRUCTION, not by convention. Tauri has two
separate keys: `app.security.csp` is injected into the built app, and
`app.security.devCsp` is injected during development — `csp` is used on dev only
when `devCsp` is absent (config schema). So `devCsp` is now the production CSP
plus exactly `ws://localhost:3000` on `connect-src`, and the production `csp` is
byte-for-byte unchanged. A shipped binary has no path to the dev entry.

What that leaves is a human failure — someone loosening the wrong key while
chasing a dev-server problem — so `csp_config_tests` in `lib.rs` pins it: the
production CSP contains no `ws://` and no `localhost:3000`, the two keys cover
the same directives, and `devCsp` differs from `csp` in `connect-src` alone and
by that one origin alone.

### D48 — the registry decides which generative model runs; the bundled 0.5B is the floor

`AppState` built the generative `ModelManager` from `BundledGenerativeLoader`
unconditionally, so the engine always ran the bundled 0.5B (`stage1-lm`). The
generative installer meanwhile downloads a pinned candidate into
`<app data>/models/<registry id>/`, verifies its sha256 and writes an
`ai_model_registry` row — and nothing ever loaded it. A user could install
1.1 GB, see it verified and registered, and still have every `citation_support`
call answered by the 0.5B, which fails validation on most real inputs. The
status panel was honest throughout (it reports whatever the manager resolved);
the wiring was the lie.

`generative::resolve_generative_loader` now walks the registry newest-first
(`registry::list_models_recent_first`, an addition — `list_models` keeps its id
ordering) and takes the first row that still resolves to a real file pair,
falling back to the bundled 0.5B and then to `NullLoader`.

Three properties are deliberate:

* **Recency, not size or id, is the preference.** "The model the user most
  recently chose to install" is the question a bake-off actually asks, and it
  keeps model choice a DATA change (§9.9) — installing the 3B makes it the
  judge with no code edit. Ties inside one wall-clock second fall back to id, so
  the order is total.
* **A row that no longer resolves is SKIPPED, never fatal.** A registry row
  outlives a deleted directory, and an id with no pinned candidate is normal —
  the bundled 0.5B registers itself under exactly such an id, which is why it
  falls through to the fallback rather than being "found" as installed. An
  unreadable registry degrades to the bundled model too.
* **The 0.5B stays.** It is what keeps a fresh machine's engine alive; it is a
  development/test floor, and D48 is about not letting the floor outrank a
  model the user deliberately installed.

### D49 — cancellation is per-RUN, and a long check must prove it is alive

Manual verification: "Check citation support" on a linked citation sat on
"Checking on this machine — this takes about a minute" for over six minutes,
Cancel visible, no result and no failure. Measured while it was happening:
`in_flight` was **1**, the process held ~390% CPU with candle's threads live,
and `sample` showed the unoptimised CPU kernels (`StridedIndex::next`,
`precondition_check`, non-inlined iterator adapters). Nothing was wedged. One
generation was genuinely running: up to 1024 decode steps (`MAX_TOKENS`) over a
~1200-token evidence prompt, on a 1.5B Q4 in a `cargo run` DEBUG build, plus a
possible second generation if the first fails validation. Minutes is the
correct number for that build; a minute was never going to be.

Three defects sat behind that one symptom.

**Cancellation was one shared flag that any new request cleared.** Every
generative command opened with `state.ai_gen_cancel.store(false)` — "never
poison the next run". Under the single-inference invariant that is backwards: a
request arriving while another is queued does not merely prepare itself, it
un-cancels everything else. Cancel then retry, and the retry's own
`store(false)` revives the run just stopped, which holds the one inference
permit while the retry waits behind it. `ai::cancel::CancelRegistry` replaces
it: a request takes a `CancelToken` that owns its flag and deregisters on drop,
`cancel_all()` stops exactly the runs alive at that moment, and a later request
can neither inherit nor clear someone else's cancellation.

**The backend already streamed progress and the frontend threw it away.**
`ai_citation_support` emits Retrieving → Retrieved → Generating → Validating,
and `aiBridge.citationSupport` called `channelInvoke` with no `onEvent`. So the
panel had exactly one static line for the whole run, which is why a working
check and a hung one looked identical. The channel is wired through now, the
stages are rendered, `Generating` carries `queued_behind` (the engine runs ONE
generation at a time, so "waiting" and "slow" are different facts and the UI
must not conflate them), and a `Decoding` heartbeat every 8 tokens gives
liveness during the long silent stretch. The panel counts elapsed time and NO
LONGER PREDICTS A DURATION: this runs on the user's CPU, where the honest
figure varies by more than an order of magnitude between builds and machines.

**Panel state outlived the citation it belonged to.** `CitationAiPanel` was
mounted unkeyed, so selecting another citation kept `phase`, and an in-flight
check went on saying "running" under the next citation and landed its evidence
there — evidence attributed to a source it was never read against, which in a
research-integrity tool is the worst failure in this list. It is keyed by
citation id now, and unmounting mid-run cancels the run it started, since an
orphaned generation still holds the model.

Also: a support check with no `citation_documents` row cannot produce evidence,
and the reason was hidden in a `title` tooltip on a greyed-out button. The panel
states it instead. "Check if citation is needed" stays available there — it
judges a sentence and needs no source.

**Cancel cannot be instant, and the panel now says so.** `QwenGenerativeBackend`
checks `cancel` at the top of each step, and step 0 is the WHOLE prompt in a
single `forward`. The loop's comment — "EVERY token, so cancel lands within one
decode step" — is true of decode and false of prefill: measured here, a Cancel
pressed at 3:26 into a run had still not landed 45 s later, because prefill was
in progress. Reporting "cancelling…" with no explanation would be a second
control that appears not to work, so the panel states the reason. The engine fix
is a CHUNKED PREFILL (feed the prompt in ~128-token slices, advancing
`index_pos`, checking cancel between slices, which also yields real prefill
progress); it is NOT taken here on purpose — it edits the inference loop, and
that is not a change to make in the middle of a manual verification pass. It is
the next thing to do in this area.

NOT changed, and worth naming: the dev app runs `target/debug/app`. Model work
in `tauri dev` is measured in minutes per check because of that, not because of
anything in this decision.
