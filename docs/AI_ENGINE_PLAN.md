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
to any remote service by the AI layer."

There are **exactly four permitted network operations**, and this list is CLOSED — a fifth needs
its own decision record and its own argument, not an appeal to these. (It said three until §11 D152
added the fourth, under this rule; the count moves only that way, and only with an entry.)

| # | Operation | Outbound payload | Trigger |
|---|---|---|---|
| 1 | Embedding-model download (`ai_model_install`) | a pinned URL, sha256-verified | user presses Install |
| 2 | Generative-model download (`gen_install`) | a pinned URL, sha256-verified | user presses Install |
| 3 | **Open-access full-text fetch (`citation_fetch_oa`, D56)** | **a DOI**, to Unpaywall / OpenAlex, then a GET of the PDF URL those return | user presses "Fetch open-access PDF" |
| 4 | **Premium manuscript review (§11 D152)** | **the manuscript itself**, to gaply-proxy, which forwards to a cloud LLM | explicit per-manuscript consent, recorded before upload |

*Consequence:* R2 is narrowed, not weakened. All four live in the app crate, all four are
explicitly user-invoked, and none of them runs at startup (see §9.4). Operations 1-3 are pinned by a
startup test that greps the modules owning the relevant lifecycle for network identifiers:
`gen_startup_tests::startup_performs_no_generative_load_and_no_network` for 1 and 2, and
`oa_fetch::tests::startup_performs_no_open_access_fetch_and_no_network` for 3.

**#4 was added under this rule, not around it.** The list said three and said CLOSED; the premium
tier needed a fourth, so it got its own entry and its own argument in §11 D152 rather than an appeal
to the existing three — which is the procedure this paragraph specifies. **It is the only one that
breaks the ID-only invariant**, and deliberately: a DOI is a public identifier, an unpublished
manuscript is not, so #4's justification is a consent argument rather than a payload argument and is
enforced by a type and a gate (`Tier::Premium(ConsentRecord)`, `release_gate::run_premium_gate`)
rather than by the shape of what is sent. The list is closed again at four, on the same terms.

*The invariant that lets #3 exist:* **ID-only outbound.** A DOI is a public identifier for a
published work, not a fact about the person holding it — no manuscript text, no claim under check,
no title, no author and nothing about the user is in the request. Operation 3 would be forbidden the
moment it needed to send anything else, which is why the resolvers are addressed by DOI and a
citation without one makes no request at all
(`oa_fetch::tests::a_citation_without_a_doi_makes_no_request_at_all`).

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

**A wait projection must count only the items that cost model time.** Unverifiable items cost
zero, so including them in an estimate overstates the wait by hours — the same arithmetic as the
significance filter, applied to the number the user is shown before consenting. This was recorded
in a comment in `ThesisAuditScreen` beside two variables (`queued`, `modelItems`) that computed it
and fed nothing; the variables are gone and the reasoning is here instead.

The rule is still enforced, in two places, neither of them those variables:
`thesis_audit.rs` counts `would_check` from `Resolution::Checkable` alone — unverifiable and
not-examined sentences are counted separately and never reach it — and the pre-run estimate calls
`projectDuration(preview.wouldCheck, …)`. The in-run estimate uses live progress
(`progress.total - progress.completed`), which is measured rather than projected. So the constraint
above governs any FUTURE projection: seed it from a count of model work, never from a queue total.


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

#### D49 addendum — a rejected model answer is an OUTCOME, and errors must render as text

Release-mode verification produced the real failure: the 1.5B ran out of room
mid-JSON (`not valid JSON: EOF while parsing a list at line 54 column 5`),
`run_task` retried once, failed again, and the panel painted **`[object Object]`**
in red.

Two separate defects, both of the same species as the `kind`/`state` bug in D46's
neighbourhood — a contract asserted on one side and never checked against the
other.

`GaplyError` serializes as `{ code, message }`. That is a plain object, not an
`Error`, so `e instanceof Error ? e.message : String(e)` — the idiom in five AI
screens — evaluated to `String({...})`, i.e. the literal `[object Object]`. It
is now `errorText()` in `aiBridge`, which walks the shapes an invoke can reject
with and whose last resort is a sentence admitting the engine gave no reason.
`errorCode()` alongside it lets a cancellation be recognised by its code rather
than by matching message text. The test is written as a PROPERTY over nine wire
shapes, not as one example: nothing may render as `[object Object]`.

And the honest wording existed but could never fire. `CitationAiPanel` has always
had a `validationFailed` branch; `ai_citation_support` has never sent one,
because it mapped every `TaskError` through `?` into a transport error. A model
answer that fails its own grounding checks is not a transport error — nothing was
persisted, nothing is broken, and it belongs to the same class as the NoEvidence
outcome the command already returns as `Ok`. Both citation commands now return
`{ outcome: "validationFailed", reason }`, and the panel shows the reason: "ran
out of room" and "said something wrong" are different problems with different
fixes, and collapsing them into one sentence throws away the distinction
`TaskError::ValidationFailed` went to the trouble of recording.

Unrelated and CORRECT, for the record: the two
`Metal unavailable, running on CPU: MTLResidencySetDescriptor is absent` lines
are the D36 gate doing exactly its job on macOS 14.5, emitted inside the
`ai_model_status` span (twice because two surfaces asked for status). They were
logged at WARN, which sat oddly beside D44's "the device is a FACT, not a
warning" — a log that shouts about the expected state trains people to ignore
it. Demoted to INFO. The two WARNs beneath it stay warnings: Metal failing or
panicking PAST the gate is not expected anywhere.

### D50 — installing a candidate other than the pinned 1.5B needed a lane, and now has one

D48 made the choice of judge a DATA question — whichever generative model is
installed and most recently registered wins. That left an obvious hole: the
Settings button is hard-wired to `qwen2.5-1.5b-instruct-q4km`, and although
`ai_generative_candidates` is exposed, no surface renders a picker. So there was
no way, in the product, to install any other pinned candidate — and therefore no
way to exercise the preference D48 had just built.

`examples/install_candidate.rs` is that lane, deliberately as a CLI: choosing
the model the engine judges with is a deployment action, not something a user
should reach by mis-clicking. It stages files into
`<app data>/models/<registry id>/` and then hands off to
`gen_install::install`, whose OFFLINE branch re-reads every byte and checks it
against the compiled-in sha256 before writing the registry row — copying weights
into place is not installing them, and the runner never registers anything
itself. A size mismatch is refused before a two-gigabyte copy rather than after
the hash. No network: install()'s offline branch constructs no HTTP client at
all, which `gen_install`'s own test already pins.

Run against the 3B on this machine: both files verified, registered offline,
0 bytes downloaded. `estimate_ram` reports **2.41 GB** (2.10 weights + 0.30 KV
at 4096 ctx) versus the 1.5B's 1.3 GB — comfortable on the 8 GB tier, and the
candidate is flagged `needs_16gb: false`, so this is within its declared
envelope rather than a hopeful exception to it.

Why the 3B is now the judge: the 1.5B demonstrated its failure mode on the first
real manual check — `not valid JSON: EOF while parsing a list`, i.e. it ran out
of room mid-answer, twice. The 3B was 6/6 in the bake-off. The registry now
reads 3B (newest) → 0.5B bundled → 1.5B, and D48's walk takes the 3B.

### D51 — the reported error named the wrong attempt, and the app was asking an unanswerable question

Manual check: `citation_support` on the 3B failed validation twice with **"no
JSON object or array found in the reply"** — the small models' signature
failure, which the 3B never produced in the bake-off (6/6, zero retries). The
obvious reading was that the 3B had failed to load and something had silently
fallen back. It had not, and there are three separate findings underneath.

**No fallback exists, and the 3B did generate.** `ModelManager::acquire` loads
the one configured loader or returns `Err`; a failed load leaves `NotLoaded` and
propagates, so it would surface as an error, not as a validation failure. The
run reached validation, so the weights loaded. Settings' "2.2 GB" is
`estimate_ram` over the 3B's own GGUF (2.41 GB = 2.24 GiB), and the log's
primary-attempt error was `not valid JSON: missing field \`why\``, i.e.
structured JSON one field short — not prose.

**So why did the panel say "no JSON"? The keep-better-attempt rule reported the
WORSE attempt.** `check()` returned `output: None` both for "no JSON in the
reply" and for "JSON found, would not deserialize", so `is_worse_than` — written
on `output.is_some()` — could not tell them apart, fell through to comparing
fatal counts, saw one each, and declared the retry not worse. Primary became the
RETRY, and its message was surfaced. `Reached { NoJson < Unparseable < Parsed }`
now carries how far a reply got, and the comparison is an ordering over that
before it counts errors. This is not cosmetic: the message it printed is a
different model's signature failure, and it sent a live diagnosis at the wrong
model while a 3B sat loaded and had nearly answered.

**And the app was asking a question with no answer in it.** Diffed line for line,
the app and the eval harness run the SAME path — same `assemble` at the same
`EVIDENCE_BUDGET_TOKENS` (1200), same `CitationSupportTask`, same
`build_prompt` with the same ChatML template, same `MAX_TOKENS` (768, matching
the bake-off's recorded `maxTokensCeiling`), same `TASK_N_CTX` (4096), same
`promptVersion` `citation_support-v1.4`. The only difference is the INPUT. The
harness sends claims like *"Organic management increased soil invertebrate
species richness by about 31 percent."* The Citation Manager sent
`selected.csl.title` — **"Chapter 1 — Literature Review"** — as the claim, with
that same string as `cited_source`, against evidence drawn from that same
document. The RULES ask the model to decompose the claim into subject,
direction of effect, magnitude, population and condition; a title has none of
them. The task was self-referential and undecomposable, and the model wrote
prose. The bake-off never covered this because no fixture is shaped like it.

The claim is an INPUT now. The Citation Manager has no manuscript sentence to
offer, so the panel takes one and refuses to run without it — a check on an
empty or degenerate claim can only spend minutes of CPU to reject itself.

**Provenance is stamped, so "which model was this?" is not a diagnosis again.**
`GenerationBackend::loaded_model_file` reports the weights the backend actually
opened — deliberately distinct from `ModelManager::model_id`, which names the
CONFIGURED loader, because "what was selected" and "what produced this answer"
are the two questions that looked identical here. It travels on every
`citation_support` and `citation_need` payload, success AND failure, and the
panel renders it. The startup log now names the selected model too; that it did
not is why item 1 was a question at all.

### D52 — "I checked, and none of them support you" is a RESULT, not a refusal

A real 3B run against a claim the document does not support returned
`insufficient_evidence` with zero `supporting_chunks`, passed validation, and
the UI answered: *"This assessment cannot be shown: it arrived without the
evidence it was based on."* Correct model behaviour, correct engine behaviour,
and a screen that reported it as a malfunction.

D18 exists to stop AI prose borrowing credibility from evidence it does not
have. The unit enforced that by testing `evidence.length === 0` — which is not
the invariant, it is a proxy for it, and it fails on exactly the case the
engine's own validator singles out:

> `is empty for verdict '…'` — a judgement about the evidence must say WHICH
> evidence; **only insufficient_evidence may cite nothing**

So the split is already made upstream, and the UI now matches it. `MUST_CITE`
= strong / partial / weak / contradicts — every verdict that asserts something
about a PARTICULAR passage, `contradicts` included, because "this source says
the opposite" points at text just as hard as "this source supports you".
Those still get the hard refusal, and it is still enforced at runtime rather
than trusted to the type, because a finding assembled from JSON can arrive
empty whatever the type says.

`insufficient_evidence` and `no_evidence` render instead as what they are:

* the verdict, with its badge and confidence;
* **what was searched** — "Checked 12 retrieved passages from this source; none
  support the claim", or, for the D15 no-generation case, "No passages were
  retrieved… so the claim could not be checked against it" (a different fact
  with a different fix — index the document — and it must not read as a
  judgement);
* the model's explanation, positioned UNDER that account and never above it, so
  the prose is read as a comment on a search that is described rather than as a
  free-standing conclusion;
* the top retrieved passages themselves, rendered through the SAME component as
  cited evidence — openable, page-linked, quote shown.

That last point needed the backend: `EvidenceBundle` kept its chunks only in
`TaskContext`, which keys them by id and loses the ranking, so `examined` now
carries them in rank order and `ai_citation_support` returns the top 3 as
`examinedPassages`. `examined` is deliberately a separate field from `evidence`
and the two are never interchangeable: `evidence` is what a verdict rests ON,
`examined` is what it was looked FOR in. Collapsing them would let a claim of
support borrow the credibility of passages it never cited — which is D18's
original failure with extra steps.

### D53 — `rename_all` renames variants, not fields; and a slow run must explain itself

The panel read **"Writing the answer — 552 of up to undefined tokens."**

`#[serde(rename_all = "camelCase")]` on an enum renames VARIANT names. Struct-
variant FIELD names keep their Rust spelling unless `rename_all_fields` says
otherwise. Proven rather than assumed:

```
{"kind":"decoding","tokens":1,"max_tokens":2}
```

Every TypeScript reader asked for the camelCase spelling and got `undefined`.
Single-word fields (`tokens`, `bytes`, `file`) were unaffected, which is why the
damage looked partial instead of total. It was not confined to the reported
symptom:

* `Decoding.max_tokens` → "up to undefined tokens";
* `Retrieved.chunks_sent` / `chunks_dropped` → the "judging N passages" line
  could never show a number;
* `Generating.queued_behind` → `undefined > 0` is false, so the queue indicator
  built in D49 could **never** render;
* `InstallEvent.total_bytes` and `GenInstallEvent.total_bytes` / `from_bytes` →
  the install progress bar divided by `undefined`, sat at 0% for a 1.1 GB
  download, and the resume signal never fired. That is a SECOND, independent
  cause of the same symptom D49 diagnosed as the shared `installing` flag.

This is the third time a wire contract has drifted in silence (after
`kind`/`state` in D46's neighbourhood). The pattern is always the same: a
missing field does not throw in JavaScript, it renders. So the contract is now
asserted on the exact JSON in `ai::event_wire_tests`, including a general rule —
no field of any streamed event may reach the webview containing an underscore —
so a NEW field fails the test even if nobody remembers to add a case.

**And the run now accounts for itself.** The 3B decoded at ~1.5 tok/s against
the 6.7 tok/s the bake-off measured for the same model on this same machine. A
wall-clock number cannot separate "this model is slow" from "this machine had
nothing left", and on 8 GB with a 2.4 GB model those are indistinguishable from
outside — which is exactly how the previous slow run became a hunt for a
mis-selected model. The result payload now carries the MEASURED decode rate
(`decodeTokensPerSec`, from the run's own timings) and the machine's free/total
memory sampled as the run ends, and the provenance line reads them out:
*"Judged on this machine by qwen2.5-3b-instruct-q4_k_m.gguf — 1.5 tok/s decode,
0.4 GB free of 8.0 GB at the end."* Both figures are reported by the backend;
neither is estimated. `free_memory_bytes` / `total_physical_ram_bytes` already
existed for the deep-model gate, so this added no new unsafe code.

**Estimates now follow measurement.** In the citation panel the decode line
projects the remainder from the rate this run is actually achieving, not a
constant. In the thesis audit `SECONDS_PER_ITEM = 65` was labelled on screen as
"measured on this machine's CPU" — true once, on a machine with room to spare,
and off by roughly 8x under pressure. It is now explicitly the SEED figure, and
the projection switches to `observedSecondsPerItem` as soon as the run has
completed two items (one item is a sample, not a rate — and the first also pays
for loading the weights), with the label saying which of the two it is using. A
stale number wearing the word "measured" reads as a promise.

### D54 — the per-citation check is a SLICE of the thesis audit, not a second pipeline

The AI panel asked the user to type a claim. That is backwards: the manuscript
already contains the sentences that cite a source, the pre-pass already finds
them deterministically, and asking a person to retype one by hand made the
feature's own input its weakest link (§11 D51 — a citation title pasted in as a
claim is what broke the first real check).

**One planner, one predicate.** `plan_thesis_audit` and `plan_citation_audit`
are the same function under `AuditScope`: same parse, same `prepass`, same
`resolve_marker`, same `NewItem` rows, same `create_job`, same runner, same
pause/resume/cancel commands. Only the predicate differs. A fix to any of that
shared machinery reaches both, and the two can never disagree about what a
manuscript says — which they would, eventually, as two copies.

`Resolution::Unverifiable` gained `library_id: Option<String>`. Without it a
per-citation filter cannot tell "this sentence cites Smith 2019, whose PDF is
not indexed" from "this sentence cites nobody we know", and the first would
silently vanish from a view that should be showing it with a Link/Index prompt.

**The list is free; the checking is not.** `preview_citation_audit` runs the
pre-pass and marker resolution and creates NO job — §11 D40's "the pre-pass runs
no model" is what makes an honest confirmation step possible at all. The user
sees the exact sentences and the projected cost before agreeing to spend minutes
each on them. A citation nothing cites answers in the same breath, rather than
queueing an empty job to find out.

**Uncited sentences are not a source's business.** "Does this sentence need a
citation?" judges sentences that cite nothing at all, so it cannot be scoped to
one citation. The per-citation panel links to the manuscript-level audit, which
already queues exactly those items through this same planner; the manual
single-sentence mode stays as the fallback it now is.

**The manuscript is session-scoped, deliberately.** Gaply has no persistent
"current manuscript" — the audit screen picks a path each time, and the Citation
Manager's `extractedCitations` prop was declared but never passed by any caller,
so "Import from manuscript (0)" had always been dead. The panel uses the SAME
picker the audit screen uses and remembers the choice for the session. That is a
smaller claim than a stored manuscript would make, and it is one the app can
actually keep.

KNOWN, and a property of the data rather than the code: `resolve_marker` matches
a marker by lead author + year. A library entry with neither can never be the
target of a marker, so a citation whose metadata is empty previews as "0
sentences cite this source" no matter how many do. That is the honest answer for
what is recorded; the fix is the citation's metadata, not a looser match — a
title-similarity fallback is exactly how a sentence gets attributed to the wrong
source, which is the failure D18 exists to prevent.

### D55 — "Link document" is one command, and it is all-or-nothing

D45 gave the UI a read path for the citation→document link, and D54's panel
could then say honestly that a source was not checkable. Neither gave anyone a
way to FIX that, so the Document row shipped a disabled "Link document (coming
soon)" — an accurate label on a dead end.

The four steps it needs already existed separately (`ai_index_document`,
`ai_embed_document`, `citation_links::link_manually`, and the document row
itself), and a UI could have called them in order. It must not. A citation whose
document is created and indexed and NOT embedded is exactly the `unverifiable`
state the user was trying to leave, and a half-linked source is worse than an
unlinked one because it looks finished. `ai_link_source_document` composes the
four so the outcome is all-or-nothing from the caller's side, and streams the
stages (`parsing` → `indexed` → `embedding` → `linked`) because embedding a
thesis on CPU is minutes, and a spinner that says nothing is how a working step
gets mistaken for a stuck one.

`matched_by = 'manual'` is the point of the write: the user asserted this link,
so it outranks the DOI and title heuristics and is never silently re-derived.

Two states are reported separately because they are two facts: the command
returns `checkable` from `checkable_document_for_citation` — asked of the store,
not inferred — and the row refuses to say "done" when the link exists but the
vectors do not.

### D56 — fetching the open-access copy, and the third (and last) permitted network operation

D45 gave a citation a link to an indexed document; the Document row's "Link
document" gave a person a way to create one from a file they already had. Both
assume the file exists on the machine. For a great many citations it does not,
and the honest answer the panel could give was a dead end: *this source cannot
be checked, go and find the PDF yourself.*

For open-access work that is a solvable problem, and solving it is worth a rule
change — so the rule is changed EXPLICITLY, in §1's R4, rather than quietly
widened at a call site. R4 now enumerates three permitted network operations and
declares the list closed.

**What makes this one permissible is the shape of the request, not its
usefulness.** The outbound payload is a DOI. Unpaywall and OpenAlex are both
addressed as `…/{doi}`; there is no title search, no author, no manuscript text,
no sentence under check and nothing about the user. A DOI identifies a published
work — it is a fact about the literature, not about the person holding it. A
citation with no DOI therefore makes **no request at all** rather than falling
back to a title query, which would have leaked exactly the thing this invariant
exists to protect. That is asserted, not asserted-to
(`a_citation_without_a_doi_makes_no_request_at_all`,
`outbound_urls_carry_the_doi_and_nothing_else_about_the_user`).

The other two conditions are inherited from operations 1 and 2: it happens only
on an explicit press, and it never happens at startup — pinned by
`oa_fetch::tests::startup_performs_no_open_access_fetch_and_no_network`, which
greps the startup modules for network identifiers exactly as
`gen_startup_tests` does, and additionally greps THIS module for `spawn(`,
`thread::`, `interval`, `sleep(` and `tokio::time` so the fetch cannot quietly
grow a scheduler later.

**Magic bytes decide what was downloaded, not the URL and not Content-Type.** A
`.pdf` URL that answers with an HTML login interstitial is an ordinary shape on
publisher sites, and indexing that page would put a paywall notice into evidence
*for the paper*. A body that does not start with `%PDF-` is reported as
`noOaCopy` and nothing is written.

**The abstract fallback, and why it is capped rather than refused.** Some works
have no free full text but do publish an abstract, and OpenAlex carries it (as
an inverted index, which `abstract_from_inverted_index` rebuilds by position).
Checking a claim against 250 words is genuinely better than refusing to check
it — and it is also strictly less than reading the paper: a subgroup, a
limitation, or a number in a table is not decidable from a summary. So the
abstract is stored as a document *flagged* `abstract_only` (migration v18, a
defaulted additive column — everything that existed before this feature is a
full text, because nothing before it could produce anything else), and
`ai_citation_support` reads that flag and **caps the verdict at `partial`**,
returning `checkedAgainstLabel: "checked against abstract only"`.

The cap is applied ON TOP of the model's answer, not by rewriting it. The
validator ties `partial` to a non-null `suggested_rewrite`; editing `verdict` in
place would produce an output object that fails the checks it had just passed.
So the response carries both — `output.verdict` (what the model said about the
evidence it saw) and `effectiveVerdict` + `verdictCapped` (what Gaply stands
behind, given what that evidence WAS) — and the evidence card stores the
effective one, because a stored `strong` that every surface renders as `partial`
is a store that disagrees with its own UI.

**The abstract arrives already defused.** It is third-party web text and prime
injection bait, so it travels as `UntrustedText` and `gaply_core::oa_fetch`
returns only its `llm_safe()` form — there is no accessor for the raw string, so
no call site can be one refactor away from writing un-firewalled text into the
document store. `an_injected_abstract_arrives_defused_and_flagged` pins it.

**Link provenance:** `matched_by = 'doi'`. `link_by_doi` upgrades an existing
`title` link (an identifier is better evidence for the same fact) and refuses to
touch a `manual` one — a person who pointed Gaply at a file has asserted
something an automatic agreement must not silently overwrite.

**What is stored is a file, not a blob.** Both paths write into
`<app data>/oa_papers/` — the PDF as `.pdf`, the abstract as `.txt` — because
the viewer, "Open document", "Reveal in Finder" and `ai_document_source`'s
`exists` all read a real path. A blob would have made every one of those
surfaces lie. The filename is the DOI reduced to `[A-Za-z0-9]` and dashes, so a
traversal-shaped DOI cannot write outside that directory
(`a_doi_cannot_write_outside_the_oa_directory`).

**Idempotence:** the document checksum is derived from the DOI, and a citation
that already has a checkable linked document reports `alreadyLinked` without
making any request — the cheapest privacy win available is the one that is never
sent.

### D57 — import size is a GUIDANCE problem, and there is deliberately no accuracy-motivated page cap

D55 and D56 both end in the same place: parse, chunk, embed. Embedding is the
expensive step by two orders of magnitude, and until now the user learned its
cost by watching it happen. That is information arriving after the decision it
was needed for.

**Three tiers, and they are not the same kind of rule.**

| tier | threshold | behaviour |
|---|---|---|
| inform | any import | states pages and an estimate |
| confirm | > 150 pages | asks first; the user may always say yes |
| refuse | > 1,500 pages or > 100 MB | declines, and says why |

The confirm tier is a speed bump, not a gate that can be failed — every dialog
it raises has a "yes" that works. The ceiling is the only hard stop, and it
exists because past it the operation stops being "slow" and becomes "the app
appears to have hung for an hour", which no estimate makes acceptable.

**Why there is NO accuracy-motivated page cap, and why one must not be added.**

The tempting rule is "long documents give worse answers, so cap them". It is
false here. What reaches the model is never the document: it is the top-ranked
chunks retrieval selected for one claim, assembled against a fixed token budget
(`EVIDENCE_BUDGET_TOKENS`). A 40-page paper and a 900-page thesis arrive as the
same handful of passages, because the BUDGET bounds the model's input, not the
source. Length changes retrieval *difficulty* — whether the right chunk lands in
the top-k — which is a ranking problem, improved by better ranking and not at
all by refusing the document.

So the only honest reason to decline a long document is TIME, and every
threshold here is denominated in time. The refusal message says so in as many
words ("a limit on TIME, not on accuracy"), because a user told merely "too big"
reasonably concludes their thesis cannot be checked properly — which is neither
true nor what the limit means.

**The estimate is measured, not asserted.** `SEED_SECONDS_PER_PAGE = 0.5` is a
release-build starting guess; every completed import folds what this machine
actually did into a running mean (`record_rate`), so the second estimate a user
sees is about their own hardware. `EstimateBasis` is reported alongside it —
"about 4 minutes" from one prior import and from a compiled-in guess are
different claims. The rate lives in the TTL cache deliberately: losing it is
harmless (the seed is the fallback) and a rate measured under conditions that
have since changed is worth forgetting. The seed is REPLACED by the first
measurement rather than averaged with it — averaging a guess with an observation
keeps the guess alive in every later estimate.

**The scanned check moved up, and the message became one string.** A PDF with no
text layer was already refused — by `parse_path_paged`, three call sites down,
each with its own copy of the same sentence. `inspect_path` now runs the SAME
`has_extractable_text` check during preflight, so the OCR advice arrives before
a `documents` row exists or a chunk is embedded, and the three copies collapsed
into `NO_TEXT_LAYER_ADVICE` — a user who meets that refusal twice must not be
told two different things.

**Every refusal is a verdict, not an exception.** `inspect_path` reports a scan
as a `Validation` error, which `preflight` converts into
`PreflightVerdict::Refused`. One shape for "you cannot import this", because a
caller handling two refusal shapes eventually handles only one of them.

**Enforced in the backend, not only in the UI.** `ai_link_source_document` takes
`confirmed` and returns `{"outcome": "confirmationRequired", "preflight": …}`
when the guard wants an answer it has not been given; `ai_index_document` and
the OA fetch apply the same guard. A size gate that lives in the frontend is a
suggestion. The cost is one extra parse on the confirm path (preflight, then the
real parse) — cheap relative to the embedding it guards, and the ceiling means
it can never be a parse of something enormous.

**The fetch path reports rather than blocks.** A batch is one press over N
sources; stopping to ask about the fourth would strand the other eight. So an
OA-fetched file over the confirm threshold — or a fetched scan — comes back as
`notImportable`, distinct from `failed` because nothing went wrong: the fetch
worked and the file is the problem. The downloaded file is deleted, since
nothing references it.

### D58 — the two prompts the first real manuscript broke, and what each fix costs

The first end-to-end audit on a real paper (Naidu 2023, 84 sentences) failed in
two distinct ways, and they need different remedies.

**`citation_support` truncated mid-array, twice.** Both attempts died with
`EOF while parsing a list at line 67 column 5`. "Line 67" is the whole
diagnosis: the model was pretty-printing its JSON, and newlines and indentation
are generation tokens like any other. It ran out of room inside
`claim_elements`, which is the LAST thing a long reply writes. Three changes,
`citation_support-v1.4` → **`v1.5`**:

1. `COMPACT_RULE` demands single-line JSON with no whitespace. Kept OUT of
   `RULES` so the bake-off's rule set is untouched and the v1↔v2 comparison
   still means something.
2. `claim_elements` capped at 5 in the prompt. The validator already treated
   "outside 1-8" as advisory; the prompt never said a number at all.
3. `MAX_TOKENS` 768 → 1024. **The arithmetic, because the two budgets trade
   against each other:** `generative.rs` enforces
   `prompt_budget = TASK_N_CTX - max_tokens` with `TASK_N_CTX = 4096`, so 1024
   leaves **3072** for the prompt. The v1 prompt measures 2035–2775 on real
   evidence plus ~50 for the labelled header — worst case 2825 against 3072,
   fitting with 247 to spare. 1280 would NOT fit: it leaves 2816, under the
   measured worst case. Both halves are pinned by
   `the_prompt_carries_the_spec_blocks_and_puts_the_claim_last`.

**`citation_need` flagged the authors' own work.** 41 of 65 sentences came back
`needs_citation`, including hardware descriptions ("I carried out all the
experiments on an Intel Core i7-11800H CPU, 16 GB RAM"), the paper's own results
("The highest F1-scores are for joy (97.0%)") and its own comparisons
("HEFCSO-BiLSTM outperforms all eight baselines"). The model's own `reason`
sometimes said *"likely represents the author's own finding"* and it still
answered `true`.

**The rule existed and could not fire.** `citation_need`'s own-results rule is
keyed on the sentence's SECTION, and `job_runner.rs` passed
`section: String::new()` for every item — the field was never populated between
the pre-pass and the job row. Two fixes, `citation_need-v2` → **`v3`**:

1. `audit_prepass` now tracks the nearest preceding heading
   (`PlannedSentence.section`), `thesis_audit` writes it into the item's
   `payload_json`, and `job_runner` reads it back. The rule can now fire at all.
2. `RULES_V3_ADDENDUM`, placed at the END of the prompt where this model weights
   hardest, states the two rules most often got wrong: the authors' own work
   never needs a citation (with three worked examples taken verbatim from this
   failure), and **`search_query` MUST be null when `needs_citation` is false** —
   five sentences failed validation twice on exactly that, a rule the validator
   enforced and the prompt never mentioned.

Neither change is a model swap; both are the prompt saying what the engine
already required.

### D59 — what D58 did not fix, in the order it should be picked up

D58's own-work rule works: on the Naidu 2023 audit, own-results false positives
fell **18 → 2** and validation failures **5 → 3 (8% → 5%)**. A targeted probe on
ten sentences the first eval could not adjudicate found six answered correctly
*and for the right stated reason*, one ambiguous (a garbled extraction artifact)
and **two genuine misses**. Those two are the work that remains, and they are not
what D58 was about.

**1. The reason and the verdict are only weakly coupled — BOTH WAYS.**

This is the real defect, and D58 moved the bias without fixing it. Before D58 the
model wrote *"likely represents the author's own finding"* and answered
`needs_citation: true`. After D58, sentence 42 of the same paper produced:

> reason: *"the physiologically motivated sigmoid probability switching
> probability is a definition attributable to a source"* — `needs_citation: false`

The reason argues FOR a citation and the boolean says no. Same defect, inverted.
A prompt that biases the boolean is treating the symptom: the model is not
deriving the answer from its own argument, so a rule that pushes the answer one
way just relocates the error.

Two candidate fixes, and **neither should be chosen without eval data**:

- Make the schema emit `reason` FIRST and have the boolean follow it, so the
  argument is in the context window before the answer is committed to. Cheap,
  but this model may simply ignore the ordering.
- A validator rule that flags a contradiction between reason text and verdict
  (a reason containing "attributable to a source" / "requires a citation" with
  `needs_citation: false`, and the converse). Deterministic and testable, but
  keyword matching on model prose is exactly the kind of rule that looks precise
  and is not — it needs a labelled set before it earns a tier.

**2. `parse_docx` throws away the structure the file explicitly carries.**

Measured on `R PAPER .docx`: `w:pStyle` gives `Heading1`×1, `Heading2`×3,
`Heading3`×15 and `TableParagraph`×32. `parse_docx` harvests `<w:t>` text and
discards every style, so 19 tagged headings and 32 tagged table cells arrive as
undifferentiated prose and the pre-pass has to guess with an ALL-CAPS heuristic.
It guesses badly: the probe saw `"HEFCSO-BILSTM: A HYBRID"` attached to abstract
sentences and `"SEAR"` — truncated — across the whole methods run.

**One change fixes three separate things**, which is why it outranks its size:

- D58's own-work rule keys on SECTION, so on Word files it is currently reasoning
  from a garbled label.
- Table rows are flagged as needing citations; `TableParagraph` identifies them
  structurally, no heuristic required.
- It supplies the LOCATOR a `.docx` otherwise has no way to give — `Methods · ¶12`
  in place of "no page numbers".

Scope: `parse_docx` emits `(style, text)` per paragraph; the paged block carries
an optional style; `heading_of` prefers a real `HeadingN` and keeps today's
heuristic as fallback; `skip_reason` treats `TableParagraph` as a table row.
**~1–1.5 days.** Researchers audit Word manuscripts constantly, and this is what
makes that path as good as the PDF path everywhere except pagination.

*Rejected alternatives for `.docx` pagination, measured on the same file:*
`lastRenderedPageBreak` carries 3 markers for 5 internal boundaries — one of them
degenerate at character 0, the usable two at 51% and 63% where an even six-page
split needs 17/33/50/67/83% — so it would be wrong for most of the document.
Interpolating from `docProps/app.xml`'s `<Pages>6</Pages>` (accurate here, and
matching the PDF exactly) assumes uniform text density, which the paper's 31
tables break. Both manufacture numbers that LOOK checkable, which `page_label`'s
own doc comment already rules out. Converting to PDF locally is legitimate and
touches no R4 constraint — conversion is not a network operation — but needs a
real layout engine: LibreOffice headless is ~700 MB, more than twice the app, and
driving an installed Word via AppleScript/COM is per-user and platform-bound. It
belongs as a later opt-in ("Word is installed — paginate via Word?"), not a
default.

**3. The residual 5% of validation failures have a DIFFERENT cause now.**

D58 stated the `search_query`-must-be-null rule in the prompt and the failures it
was aimed at went away. What is left is not that: sentence 47 failed twice with
`missing field 'severity'`. That is schema completeness, not a rule the prompt
failed to state, so it needs its own fix and the null-rule remedy will not touch
it.

#### The constraint behind all three: eval throughput, not ideas

Every number above cost **~5 minutes per sentence** — a 65-sentence before/after
is roughly five and a half hours on an 8 GB M1 Air running the 3B through
`quantized_qwen2_lowmem`'s CPU path under swap pressure, and the ten-sentence
probe was another fifty minutes. At that rate a prompt change cannot be iterated;
it can only be committed and hoped for, which is how D58 shipped a real
improvement while leaving the coupling defect above undetected until a second
probe went looking for it.

**`citation_need` prompt work is blocked on Metal, not on ideas.** A GPU path
turns a five-hour eval into minutes and makes all three items above measurable
rather than arguable — item 1 in particular CANNOT be settled without a labelled
set run repeatedly. Sequence accordingly: Metal first if any of this is to be
done properly, and treat every prompt change made before then as provisional.

### D60 — Phase 7 STEP 2 lands: the macOS 15 gate opens, and Metal is 5.4-5.8x on prefill-bound work

D35 parked this on an OS version. **The machine is now on macOS 26.6.2 (Darwin
25.6.0)** and the blocker is gone, measured rather than assumed:

```
MTLCaptureDescriptor      (control, 10.15+): true
MTLResidencySetDescriptor (the blocker, 15+): true
metal_gate()  : Ok(())
select().kind : metal
Device::new_metal(0) OK
```

D35's other prediction also held: `candle-metal-kernels` compiles its shaders at
runtime, so the whole stack builds with **Command Line Tools only** — there is no
`metal` compiler on this machine (`xcrun -sdk macosx metal` fails) and nothing
needed one. The cold rebuild was clean in 5m30s; nothing in candle, tauri or the
toolchain broke on the macOS 14 -> 26 jump.

#### The first Metal use on a machine costs SECONDS, and it is not per-process

The first 256x256 quantized matmul took **9.25 s**; the same call in a later
process took **17.5 ms**. macOS caches the compiled shader library on disk, so
the cost is once per machine per kernel set — and it recurs for a *different*
kernel set: the BGE embedder's first Metal run paid it again (17.0 s cold vs
6.35/6.31 s warm).

That is why the device is selected **once, process-wide** (`device::shared()`),
not per engine. Two reasons, both load-bearing:

1. The compilation cost is paid at app start rather than on a user's first
   check. The running app logs `AI engine device: metal` at startup.
2. **The engines must agree.** A judge on Metal with an embedder on CPU is
   neither of the two configurations anyone wants to measure, and
   `GAPLY_FORCE_CPU` has to mean the whole process for the comparison below to
   mean anything.

#### Why both arms ran v1.5, and why D34 is context rather than a control

The correctness run was scoped as "3B v1.4". **v1.4 no longer exists** — D58
shipped `citation_support-v1.5`, changing the prompt AND `MAX_TOKENS` 768 -> 1024.
Running Metal on v1.5 against D34's v1.4 CPU number would have compared runs
differing in three ways at once and attributed the whole delta to the device.

So **both arms ran v1.5 from ONE binary, toggled only by `GAPLY_FORCE_CPU`** —
which is exactly what that env var's doc comment says it exists for. Device is
then the only variable. D34's 65.4 s is historical context, and any arithmetic
against it is arithmetic across three changes.

#### The measurement (isolated, same binary, same seeds)

| metric | CPU v1.4 (D34, macOS 14.5) | CPU v1.5 | **Metal v1.5** |
|---|---|---|---|
| mean latency | 65,393 ms | 183,635 ms | **42,530 ms** |
| mean prefill | 35,809 ms | 119,587 ms | **14,052 ms** |
| mean decode | 29,434 ms | 63,736 ms | **28,260 ms** |
| decode tok/s | 7.31 | 4.17 | **11.5** |
| prefill share | 0.548 | 0.651 | **0.330** |
| valid outputs | 6/6 | 4/6 | 5/6 |
| faithfulness violations | 1 | 2 | **0** |
| RAM | 2,295 MB | 2,295 MB | 2,295 MB |

**The clean pair is seeds 03 and 04** — no retries on either device, and prompts
byte-identical (1252/1252, 1265/1265 tokens):

| seed | CPU prefill | Metal prefill | speedup |
|---|---|---|---|
| cs-seed-03 | 117,035 ms (93.5 ms/tok) | 17,889 ms (**14.3 ms/tok**) | **5.39x** |
| cs-seed-04 | 99,972 ms (79.0 ms/tok) | 10,121 ms (**8.0 ms/tok**) | **5.80x** |

Those identical token counts are themselves a result: **the embedder retrieves
identically on both devices**, so evidence assembly is unaffected. The summary
table's mean-prompt-token gap (1638 CPU vs 1320 Metal) is **purely a retry
artifact** — a retried case sends its prompt twice — not a device difference.
Both needle tests rank the needle **#1 on Metal**, with scores byte-identical to
CPU (0.8821, 0.7669).

#### Metal is NOT bit-identical to CPU, and the divergence is reported, not averaged

Greedy decoding does not reproduce across devices, and on this seed set it
crossed the validity boundary in **both** directions:

- **cs-seed-01**: Metal failed validation (`suggested_rewrite must be null when
  the verdict is 'weak'`); CPU produced a valid `weak`.
- **cs-seed-02 / cs-seed-05**: the mirror image — CPU failed both, Metal was
  valid for both.
- **Faithfulness**: CPU violated on 03 and 04; **Metal on neither**.
- **cited-planted-chunk went the WRONG way on Metal**: 0.33 vs CPU 1.00. On six
  seeds with different retry paths that is one or two cases, so it is not a
  conclusion — but it is the one metric where Metal looks worse and it needs a
  larger seed set before anyone rules either way.

Net 5/6 valid on Metal against 4/6 on CPU. Nothing here says Metal is wrong; it
says per-device reproducibility is not a property this engine has, which is now
measured rather than assumed.

#### The gate's own test had gone vacuous, and was rewritten

`a_forced_open_gate_cannot_panic_the_caller` proved the `catch_unwind` belt by
forcing the gate open on macOS 14, where `default_probe` **genuinely panicked**.
On macOS 26 that same call builds a working Metal device and takes the happy
branch: the `else` asserting `"panicked"` is dead, and the panic-suppressing hook
is pointless. **The `Err(_)` arm became untested at exactly the moment this
machine stopped being able to reproduce the failure it was written for** — the
hazard D36 tried to avoid by writing a test that passes on both sides of the
boundary. Passing on both sides is not the same as testing on both sides.

`a_panicking_probe_yields_cpu_rather_than_unwinding` injects the panic instead of
depending on an old host OS, so the belt is exercised everywhere, forever.
`the_shared_device_is_selected_once_and_is_stable` pins the engines-agree
property. `the_real_gate_agrees_with_the_runtime` survives unchanged and still
earns its place; its `if !present` branch is now dead on this machine only.

#### OPEN: v1.5 does not clear interactive range, and decode is why

Metal steady state is **28.9 / 31.7 / 33.1 s** per check against a 10-20 s
target — 5.4x closer than CPU, still 1.5-3x short. The profile has INVERTED:
prefill share 0.548 -> **0.330**. Prefill is largely solved (93.5 -> 14.3 ms/tok);
decode improved only 7.31 -> 11.5 tok/s (**1.57x**) because decode is
memory-bandwidth-bound, which Metal barely helps. **The remaining 2-3x is an
output-token problem, not a GPU problem.**

#### OPEN AND MORE IMPORTANT: CPU prefill is ~2.4x slower than the macOS 14.5 baseline

**93.5 and 79.0 ms/tok against D34's 35.9 — already normalised per token**, so
prompt growth does not explain it. Not isolated between three candidates: the OS
jump, memory pressure (2.8 GB swap on an 8 GB machine, and the CPU arm ran
second), and background load (load avg 4.86 with Chrome open vs D34's 4.25). No
thermal or performance warning was recorded, so it is not throttling.

**CPU is the shipped floor for every user without Metal, so if this is real it
matters more than the Metal win.** It is recorded here as unresolved rather than
asserted; the controlled re-measure is CPU arm FIRST on a cold machine with
everything else closed.

### D61 — CPU prefill regressed 2.65x on macOS 26, and that makes the model choice a PER-PLATFORM question

D60 left this open. It is now closed as far as this machine can close it: the
regression is real, it is not contention, and the toolchain is excluded.

#### The measurement, controlled

Same seeds, same binary, CPU both times. Seeds 03 and 04 are the only clean
comparison points — single attempt, no retry, prompts byte-identical across
arms:

| seed | CPU contended | CPU cold + caffeinate | Metal | cold vs D34 |
|---|---|---|---|---|
| cs-seed-03 | 93.5 ms/tok | **95.2 ms/tok** | 14.3 | **2.65x** |
| cs-seed-04 | 79.0 ms/tok | **98.2 ms/tok** | 8.0 | **2.74x** |

against D34's **35.9 ms/tok** on macOS 14.5. The re-measure ran CPU FIRST on a
quiet machine: Chrome and the app closed, `caffeinate -dimsu`, load average 1.73
at start, swap **894.56 MB -> 1033.81 MB** (+139 MB, no pressure event), and no
thermal or performance warning recorded.

**It reproduced slightly WORSE, not better.** Contention, background load, memory
pressure and thermal throttling are all excluded.

Two secondary results from the same run:

- **CPU is deterministic with itself.** Seeds 02 and 05 failed validation on
  exactly the same rules in both CPU runs, faithfulness violations landed on the
  same seeds, and the summary was identical (4/6 valid, 33%, 1638 mean prompt
  tokens). So D60's CPU-vs-Metal divergence is a genuine device effect, not
  run-to-run noise.
- **A "cold machine" is not uniformly faster for this workload.** cs-seed-01
  looked 2.13x faster cold, but it was the FIRST case in the contended run and
  paid model page-in; cs-seed-04 went the other way and was 21% SLOWER cold. A
  warm page cache helps a memory-bound workload, so "cold" is not a synonym for
  "fast" here and a single seed's improvement proves nothing.

#### The toolchain is excluded by provenance, not by experiment

The obvious next step was to rebuild under the rustc that built D34. There is
nothing to rebuild against:

```
~/.rustup/toolchains/                     13 Jul 18:07   one toolchain, ever
  stable-aarch64-apple-darwin/bin/rustc   13 Jul 18:08   never modified since
~/.rustup/downloads/                      empty          no update ever fetched
rustc 1.97.0 (2d8144b78 2026-07-07)
```

D34 was measured **29 August 2026**, six weeks after that install; the Metal work
is **4 September**. One toolchain, installed once, never updated — so **D34 and
the current runs used the same byte-identical compiler**. Installing an "older"
toolchain would have compared 1.97.0 against 1.97.0 and produced a number with no
information in it. `Cargo.lock` pins `candle-core 0.11.0`, identical to D34, so
the library is excluded too.

**What remains is the OS.** The honest bound: v1.4 -> v1.5 grew the prompt
998 -> 1252 tokens, which even granting fully quadratic attention accounts for at
most ~1.25-1.57x, so **2.65x is an upper bound on the OS-attributable share, not
a clean measurement of it.** It is not zero, and it is not all of it.

**The reports now record `rustc` and `os`** (`build.rs` captures cargo's `RUSTC`,
so it is the compiler that actually built the binary rather than whatever is on
PATH at runtime). Excluding the toolchain this time required forensics on rustup
directory mtimes — circumstantial evidence that stops working the moment anyone
runs `rustup update`. Timing cells whose `rustc` or `os` differ are not
comparable, the same rule `loadContext` already carries.

### The product consequence: CPU is the floor, and the floor is now three hours

This is not a benchmarking curiosity. Measured, per citation check and for a
65-sentence audit:

| | per check | 65-sentence audit |
|---|---|---|
| **CPU** (every non-Metal user) | ~170-200 s | **~3 hours** |
| **Metal** (macOS 15+ only) | ~31 s | **~30 minutes** |

**Metal is macOS 15+ only** (D35/D36), so **every Windows user, every Linux user
and every Mac below macOS 15 is on the three-hour floor.** Phase 8 was already
shaped by long audits — crash-resume, streaming and cancellation are the feature,
not polish — but three hours is a different product from thirty minutes, and the
gap is now platform-determined rather than universal.

**So the shipped model choice is a PER-PLATFORM question, and it belongs beside
D30's Metal conditional rather than being rediscovered later.** The 3B is
defensible at ~31 s on Metal. At ~170-200 s it is hard to defend as a default,
and **the 1.5B may be the honest default without Metal** — it is already pinned,
installed and registered (`qwen2.5-1.5b-instruct-q4km`), so this is a selection
decision rather than new work.

**Not decided here, because the data does not exist yet.** The 1.5B's quality on
`citation_support` has never been measured against the 3B on the CURRENT prompt —
the bake-off cells are v1/v2.1, not v1.5 — and choosing a weaker default judge on
speed alone would trade a correctness property for a latency one without knowing
the exchange rate. **What must be measured first:** 1.5B on v1.5, CPU and Metal,
same six seeds, reporting valid-output rate, cited-planted-chunk and faithfulness
violations beside ms/tok. If the 1.5B holds grounding at materially lower latency,
per-platform defaults follow; if it does not, the honest answer is that the CPU
floor is slow and the UI must say so rather than that a weaker judge is shipped
quietly to hide it.

Sequenced against D59: the citation_need prompt work is blocked on eval
throughput, and this is the same constraint seen from the product side.

### D62 — the 1.5B does not hold grounding, so the CPU floor stays on the 3B and the UI says so

D61 named the 1.5B as the candidate default for non-Metal platforms and refused
to decide without data. The data now exists: **1.5B on `citation_support-v1.5`,
both devices, the same six seeds, isolated, CPU arm first.**

| arm | valid | fail rate | cited planted | faithfulness | outputs checkable | mean latency | prefill ms/tok |
|---|---|---|---|---|---|---|---|
| 3B CPU (cold) | 4/6 | 33% | **1.00** | 2 viol. | 2 | 198,751 ms | 85.3 |
| **3B Metal** | **5/6** | **17%** | 0.33 | **0** | 2 | **42,530 ms** | 10.6 |
| 1.5B CPU | 1/6 | **83%** | — | — | **0** | 55,054 ms | 17.9 |
| 1.5B Metal | 1/6 | **83%** | — | — | **0** | 60,248 ms | 10.8 |

Per-seed prefill on the clean seeds (03/04): 3B CPU 95.2/98.2 -> 1.5B CPU
**18.3/18.5 ms/tok**. The 1.5B IS ~5x faster on CPU prefill, as its parameter
count predicts. It does not matter.

#### That `1/6` is worse than it reads

**The one "valid" case is cs-seed-06 — the NoEvidence case that runs no model at
all.** Across all five seeds that actually invoke the model, on BOTH devices, the
1.5B produced **zero** valid outputs.

So `citedPlantedChunk` is `None` and faithfulness reads *"0 violations of 0
accepted outputs checked"*. Those are not good scores, they are **undefined** —
there were no accepted outputs to measure grounding on. Reporting "the 1.5B had
zero faithfulness violations" would be the single most misleading number
available in this whole comparison.

#### The failures are structural, and the same one every time

On CPU, **all five** broke the identical rule: *verdict is 'contradicts' but no
claim element is marked 'different'*. The model reaches for the strongest verdict
and cannot show it in the decomposition — precisely the property the validator
exists to enforce, and precisely what makes a verdict auditable.

Metal failed differently and worse: seed-01 cited **9** chunks and seed-02 cited
**12 of the 12 sent** — every chunk it was given, which is the behaviour the rule
text explicitly warns against. That is the deeper finding. **A judge that cites
everything is not selecting evidence, and unselective citation grounds nothing.**
The 3B cited 1-4 on the same seeds.

#### Decision: the CPU path stays on the 3B

Per D61's own framing — *if it does not hold grounding, we do NOT ship a weaker
judge to hide a slow floor*. This is not a marginal trade of some agreement for
5x speed; it is a failure to produce a legal grounded answer 5 times out of 5 on
both devices, and the retry cost would consume much of the speed advantage
anyway. **The 3B remains the judge on every platform, and the UI states the wait
plainly before an audit starts.**

**Two caveats, recorded because the result is being used to close a decision.**
Six seeds is a small set, and the 3B's own `citedPlantedChunk` moved 1.00 -> 0.33
between devices, which shows these metrics are noisy at n=6 — so this is not
evidence about how much better the 3B is, only that the 1.5B fails structurally.
And this tests the 1.5B **on a prompt tuned for the 3B**; a prompt written for a
smaller model is a real future option if the CPU floor becomes unacceptable, but
it is new work with its own eval, **not a default swap**.

#### The projection seed is now PER DEVICE, and the wait is stated in words

`SECONDS_PER_ITEM = 65` came from the D34 CPU baseline and, after Phase 7 STEP 2,
was wrong in both directions at once — a 65-sentence audit was quoted at "70
minutes" whether the truth was ~3.3 hours or ~34 minutes:

| device | old seed | measured (3B, v1.5) | 65-sentence audit |
|---|---|---|---|
| CPU | 65 s | **185 s** | ~3.3 hours |
| Metal | 65 s | **31 s** | ~34 minutes |

`seedSecondsPerItem(device)` selects from `ai_model_status.activeDevice`, and
**an unknown device falls back to the CPU figure** — a projection that
under-promises turns a three-hour job into an unpleasant surprise, so the honest
direction to be wrong in is the pessimistic one. `observedSecondsPerItem` still
takes over after two items, and the screen still labels which of the two is in
use; only the seed and its selection changed.

`waitAdvice()` states the consequence in words at the confirmation step, because
a bare duration reads as a progress bar that has not started: on CPU, that the
machine will be busy for that whole time, that it can be cancelled and that
finished items are kept; on Metal, that the GPU does the work. **This is the
moment the user consents to a long job, and it is written to be honest rather
than optimistic.**

These are numbers about a MACHINE and they expire. When the engine gets faster
this is one of the places that must be re-measured, not adjusted by feel.

### D63 — v1.6 states two of the three fatal rules, and the third is refused on D32's evidence

D60 named retries as the largest single decode cost (17% of Metal cases, 33% of
CPU). D58's defect shape explains them: **a rule the validator rejects on that
the prompt never states**. Every v1.5 validation failure, on both devices, was
one of three such rules.

`citation_support-v1.6` states two of them, at the END where D58 measured this
model weights hardest:

1. `suggested_rewrite` MUST be null unless the verdict is exactly `partial`.
2. `contradicts` REQUIRES at least one `claim_element` with status `different`.

#### The third rule is DELIBERATELY ABSENT, and a test stopped it being added

The obvious third — *at most 4 `supporting_chunks`* — was written, and
`the_prompt_never_mentions_the_chunk_bound` failed and caught it. **D32 measured
that exact line on this exact model**: the cap **never fired** (the 3B's cited
counts were 2, 4, 2, 3, 0) while **planted-chunk citation fell 75% -> 25%**. D34
removed it for that reason and left the test as the guard. Adding it back to save
a retry would trade the grounding property this engine exists to provide for
latency — the same trade refused for the 1.5B in D62.

What HAS changed since D32: v1.5 and v1.6 both produce 5-chunk answers on CPU,
where D32 saw the cap never fire. That is an argument for measuring the bound as
its own variant, not for quietly re-adding a line already shown to cost
grounding.

#### Result: a clear win on Metal, neutral-to-slightly-worse on CPU

| arm | valid | fail rate | retry rate | cited planted | faithfulness |
|---|---|---|---|---|---|
| v1.5 CPU (cold) | 4/6 | 33% | 33% | 1.00 | 2/2 |
| **v1.6 CPU** | 4/6 | **33%** | 33% | 0.67 | **1/2** |
| v1.5 Metal | 5/6 | 17% | 17% | 0.33 | 0/2 |
| **v1.6 Metal** | **6/6** | **0%** | **0%** | 0.50 | 0/2 |

**Metal: retries eliminated.** The single v1.5 Metal failure was cs-seed-01 on
`suggested_rewrite must be null when the verdict is 'weak'` — rule 1 fixed
exactly that case, and cited-planted moved 0.33 -> 0.50, so nothing was traded
for it. D58's defect shape, confirmed a second time.

**CPU: unchanged, and predictably so.** Both CPU failures are
`supporting_chunks: has 5 entries` — the one rule not stated. The two rules that
were stated address failures CPU does not have.

#### THE LATENCY COLUMN IS NOT REPORTED HERE, ON PURPOSE

Mean latency rose on both arms, including on the Metal arm that eliminated a
retry entirely and whose mean prompt tokens FELL (1320 -> 1142, since no case
sends its prompt twice any more). A retry-free run cannot be slower than a
retry-carrying one because of a slightly longer prompt, so that number is not
measuring what it claims to. Combined with D61's finding that this machine's CPU
throughput is unstable between runs, **validity, retry rate and cited-planted are
the only trustworthy columns at n=6.** No latency claim is made in either
direction — neither the win that would have been convenient nor an explanation
for the rise.

Cited-planted on CPU moved 1.00 -> 0.67. That is the metric D32 saw fall when
prompt text was added, so it is worth watching — but at three checkable cases it
is one case flipping, not a trend.

### D64 — the chunk bound, re-tested and refused a second time; and prompt rules COMPETE

D63 stated two of the three fatal rules and refused the third — *at most 4
`supporting_chunks`* — on D32's evidence. Two things had changed since D32, so it
was worth re-asking rather than assuming: the cap **now fires** (v1.5 and v1.6
both produce 5-chunk answers on CPU, where D32 measured cited counts of
2, 4, 2, 3, 0 and the cap never engaged), and the surrounding prompt is different.

Run as `citation_support-v1.7-chunkbound`, one cell per device, the same six
seeds, the shipped v1.6 prompt byte-unchanged.

#### It works exactly as scoped, and it is refused anyway

**CPU validity: 4/6 -> 6/6, failure rate 33% -> 0%.** Both chunk-bound
rejections (seeds 02 and 05) are fixed. That was the narrow question and the
answer is yes.

**And cited-planted fell on both devices, so it does not ship.** Grounding over
latency — the rule that refused the 1.5B in D62.

Metal, per seed:

| seed | v1.5 | v1.6 | **v1.7** |
|---|---|---|---|
| cs-seed-01 | failed | cites planted | **failed again** |
| cs-seed-03 | cites planted | cites planted | **does not** |
| aggregate | 0.33 | **0.50** | **0.00** |

**Not one accepted Metal answer cites the planted passage.**

CPU, on the three seeds checkable under all three versions (01, 03, 04):
**3/3 -> 2/3 -> 1/3**. cs-seed-04 lost the planted chunk and its cited count fell
**3 -> 2** — which is, word for word, what D32 recorded: *"cs-seed-04's cited
count fell 3 -> 2"*. Same seed, same movement, six weeks and three prompt
versions apart.

The mechanism is visible and it is not subtle: **v1.7 buys validity by making the
model cite FEWER chunks, and the chunk it drops is disproportionately the one
that carries the point.** A 6/6 run in which no answer cites the planted evidence
is worse than a 4/6 run in which the accepted ones do.

#### THE PHRASING QUESTION IS CLOSED

D32 measured three cells — no mention 75%, **stated plainly 50%**, stated with
guidance 25% — and D34 removed the guidance-laden wording. That left an obvious
hypothesis: perhaps the *guidance* did the damage, not the bound.

**v1.7 used the bare statement, with no guidance, and its test asserts neither
guidance phrase can reappear.** It still cost grounding on CPU and wiped it on
Metal. **The bound is harmful to this model regardless of how it is worded.** Do
not run this a third time hoping for a kinder phrasing; there isn't one. The
question is settled, and `the_prompt_never_mentions_the_chunk_bound` stands
UNCHANGED — a stronger guard now than before, because what it forbids has been
tested twice, under different conditions, with the same answer.

#### THE RULE BUDGET: prompt rules on this model COMPETE, they do not accumulate

The finding that generalises past this bound.

**Metal validity REGRESSED 6/6 -> 5/6 under v1.7**, because cs-seed-01 failed
again on `suggested_rewrite must be null when the verdict is 'weak'` — *the exact
rule v1.6 had just fixed*. Nothing about rule 1 changed. The only edit was adding
a third rule beneath it, and compliance with the first fell away.

So the mental model of "state one more rule, get one more rule obeyed" is wrong
for this model. **Attention to a stated rule appears to be a budget that a new
rule spends from, not a list that grows.** That is a constraint on EVERY future
prompt change, not a footnote to this one:

- A new rule's cell must re-check the rules already stated, not only the failure
  it targets. D63 would have caught nothing here by looking at its own two rules.
- "The prompt does not mention X" is a real configuration with real value, not an
  oversight waiting to be corrected. D34 was right to remove the bound, and right
  for a reason more general than the bound.
- Adding rules to fix retries has a ceiling, and v1.6 may already be at it.

#### The apparatus stays

`CitationSupportV17Task` remains as a **permanent, unreachable-from-the-app
experiment slot**: `--support-variant v1c` in the eval harness is its only
caller, `PROMPT_VERSION_V17` keeps its reports from ever being confused with a
shipped cell, and `v17_is_v16_plus_the_bound_and_nothing_else` pins that it
differs from the shipped prompt by exactly the lines under test.

**The next prompt variant should reuse this slot rather than rebuild it.** It is
what let this question be answered in one run per device without touching the
shipped prompt, and the diff test is what keeps such a cell honest — a variant
that quietly differs in two ways measures neither.

### D65 — `.docx` carries its own structure, and the pre-pass was throwing it away

D59 item 2. A Word file DECLARES what the pre-pass was guessing: `R PAPER .docx`
contains 1 `Heading1`, 3 `Heading2`, 15 `Heading3` and 32 `TableParagraph`, and
`parse_docx` harvested `<w:t>` text and discarded every `w:pStyle`. So the
pre-pass inferred headings from ALL-CAPS shape and table rows from digit
density, on a document that had already said which was which.

`parse_docx_blocks` keeps paragraph boundaries and the style beside each one.
`prepass_blocks` reads them; `prepass` remains as the `(page, text)` shim, so a
source with no style information behaves exactly as before — every existing
caller and test is unchanged. **Structure first, heuristics second: a declared
style is a fact, ALL-CAPS shape is a guess about one.**

Both arms below run on the SAME parsed blocks — the shim discards styles, which
is exactly the old behaviour — so structure awareness is the only variable.

#### Section labels: 9 guesses -> 19 declared

| before (ALL-CAPS guess) | after (`w:pStyle`) |
|---|---|
| `SEAR` | `Statistical Significance Testing` |
| `PER-EMOTION PERFORMANCE (HEFCSO-` | `Per-Emotion F1-Score Analysis` |
| `BiLSTM + SMOTE` | `Text Pre-Processing Pipeline` |
| `HEFCSO-BiLSTM` | `Proposed HEFCSO Algorithm` |
| `EXPERIMENTAL SETUP AND RESULTS` | `Experimental Configuration`, `Dataset Acquisition`, … |

Nineteen real section names in place of nine, several of which were truncated
fragments. **This is the half that matters**, because D58's own-work rule keys on
section and was reasoning from `"SEAR"` on every Word manuscript.

#### THE FILTER'S OUTPUT DID NOT CHANGE. Its EXPLANATION was wrong for 51 items.

Stated plainly so nobody later cites the table-row number as an accuracy win:

| skip reason | before | after |
|---|---|---|
| table row or figure caption | 7 | **39** |
| heading, not a claim | 5 | **24** |
| too short to be a claim | 202 | **151** |
| **planned (sent to the model)** | **84** | **84** |

**+32 tables and +19 headings match the declared counts EXACTLY** — the deltas
are not approximately right, they are the file's own structure. But 202 -> 151 is
the same 51 items: they were ALREADY being dropped, as *"too short to be a
claim"*, and are now dropped as what they actually are.

**Nothing new is excluded from the audit and no sentence changed side.**
`7 -> 39 table rows dropped` is a REPORTING correction, not a filtering
improvement — the report's "Not judged, and why" section was misdescribing a
fifth of its own skips. Anyone citing it as an accuracy gain is citing it wrong.

#### A paragraph is a locator; "no page numbers" is not

A `.docx` has no pagination until something lays it out, so 84 items reading
*"no page numbers"* told a reader where nothing was. `PlannedSentence.paragraph`
is a 1-based ordinal counted over every block — including skipped ones, since a
locator must match what the reader counts in the document — and rides in the
item payload beside `section`.

Rendered on job 7: **94 `¶N` references, zero "no page numbers"**. The attention
list reads `[needs citation] ¶8 · sentence 3`.

**`page` WINS whenever both exist** (`a_page_wins_over_a_paragraph_when_both_exist`).
Two locators for one sentence is a reader deciding which to trust, and the page
is the one they can act on. PDF behaviour is byte-unchanged.

#### AMENDMENT — the accuracy claim was MEASURED, and it is a null result

D65's section work was justified by D58: the own-work rule keys on section and
was reading `"SEAR"`. That is a claim about model behaviour, and per D64 nothing
prompt-adjacent on this model is safe to assume, so it was checked.

**30 sentences whose section label CHANGED**, run on Metal with the 3B and
`PromptVariant::V3`. Both arms share the sentences, the model, the device and the
prompt; only the section string differs.

| | BEFORE (guessed) | AFTER (declared) |
|---|---|---|
| needs_citation = true | 0 | 0 |
| needs_citation = false | 23 | 23 |
| validation failures | 7 (23%) | 7 (23%) |
| own-work answered TRUE | **0** | **0** |

**Every distribution is identical, and not one sentence changed its verdict.**
Six answers "changed" and all six are validation noise — three
`failure -> valid false` and three `valid false -> failure`, netting to zero,
which is the `missing field 'severity'` problem of D59 item 3 landing on
different sentences run to run.

**Why the effect was zero: the relabelled population is the wrong shape.** The
sentences whose labels improved are related-work claims about OTHER people's
methods — *"Recurrent network parameters have been tuned by PSO [19]"*,
*"Yang [9] formalized FA…"* — relabelled
`HEFCSO-BILSTM: A HYBRID -> Nature-Inspired Hyperparameter Optimisation`. Those
were already answered `false` correctly under the bad label, and the own-work
rule was never going to fire on them. The sentences the rule EXISTS for — the
authors' own results and hardware — live in the abstract and Results, and those
labels did NOT change: the abstract is the known `Title` gap below, and
`EXPERIMENTAL SETUP AND RESULTS` was already being guessed correctly.

**So D65 is a REPORTING and LOCATOR change, not an accuracy one** — the same
correction the table-row numbers needed, applying to the section labels too. The
labels are genuinely better; on this manuscript that improvement did not reach a
single sentence where it could change an answer.

**Caveats, because this is a null result being used to withdraw a claim:** n=30
on ONE paper, and — at the time of the first run — 23% of each arm was
uninformative from the severity failures, so roughly a quarter of the population
carried no signal either way. A manuscript where the ALL-CAPS guess mangles the
*Results* heading — rather than the related-work subheadings, as here — could
still show a real effect. This says the section signal did not matter HERE, not
that it never matters.

**RE-RUN ON THE CLEAN DENOMINATOR (after D66).** The first run was measured on a
compromised population and could not be left standing on one, so it was repeated
verbatim once the severity failures were gone:

| | BEFORE (guessed) | AFTER (declared) |
|---|---|---|
| needs_citation = true | 0 | 0 |
| needs_citation = false | 29 | 30 |
| validation failures | 1 (3%) | **0 (0%)** |
| own-work answered TRUE | **0** | **0** |

**The null HOLDS on a clean population** — uninformative cases fell 23% -> 0-3%,
and still not one sentence changed its verdict. The single "changed" answer is
again failure -> valid `false`, not a flip. **D65's amendment therefore stands,
and is now trustworthy rather than merely stated.**

#### KNOWN GAP, not a defect to chase

`HEFCSO-BILSTM: A HYBRID` still appears as the section for the abstract. **The
file declares no `Title` style for that paragraph**, so there is no structure to
read and the ALL-CAPS fallback fires and truncates at its 8-word limit. Structure
fixed the 19 paragraphs Word actually marked; it cannot fix one Word did not.
Chasing it means improving the heuristic for unstyled documents, which is a
different piece of work with its own eval — and per D64, a change near this model
is not additive and would need measuring rather than assuming.

### D66 — `severity` could not exist in a legal state, and the engine blamed the model

D59 item 3. 23% of every `citation_need` eval arm was failing validation with
`missing field 'severity'`. Treated as a model defect for as long as it was only
read through the error message.

#### Read the raw output before blaming the model

The failures are byte-identical in shape, seven times out of seven:

```json
{
  "needs_citation": false,
  "sentence_type": "author_own_result",
  "reason": "the authors' own pre-trained vectors",
  "search_query": null
}
```

with `stop_reasons: [EndOfTurn, EndOfTurn]`. **Not truncation** — the model
finished cleanly, twice. The field is **absent**, not null and not an
out-of-range variant (serde would say `invalid type: null` or `unknown variant`
for those, and the message was read closely enough to be sure).

**The correlation is total: 7/7 have `needs_citation: false`, and 7/7 have
`sentence_type: "author_own_result"`. Zero failures had `needs_citation: true`.**

A sentence that needs no citation has **no severity-of-need to report**. The
spec declared the field unconditionally required, so the model's only legal
options were to invent a grade for a need that does not exist, or to be rejected.
It chose correctly and was rejected. **These were never model failures.**

Worse: `sentence_type: "author_own_result"` means D58's own-work rule was FIRING
AND RIGHT, and the engine was throwing that exact answer away.

#### The fix is in the schema, not the prompt

`severity` becomes `Option<Severity>`, required CONDITIONALLY — mirroring
`search_query`, which the spec already treats this way in the opposite
direction. All four combinations are tested, because a conditional rule checked
only on its convenient sides is a rule nobody has checked:

| `needs_citation` | `severity` | verdict |
|---|---|---|
| true | present | ok |
| true | **absent** | **FATAL** — the grade is the information |
| false | **absent** | **LEGAL** — the fix |
| false | present | accepted silently |

**The fourth row is a reversal worth recording.** The first draft made it an
ADVISORY — "a grade for a need that does not exist is noise" — and the existing
`a_false_verdict_with_a_null_query_passes` failed. The test was right: the spec
declares `severity` present unconditionally, so a model emitting it there is
doing exactly what it was asked. D66 widens what is ACCEPTED; it must not
simultaneously start complaining about the compliant shape, and an advisory would
have penalised correct behaviour and inflated `advisoryRate`, which the bake-off
reads as a quality signal. **The prompt was not touched** (D64).

#### Measured, on the same 30 sentences

| | before | after |
|---|---|---|
| validation failures | 7 (23%) | **0 (0%)** |

And "it validates" is not the claim D66 makes, so the verdicts were measured
too, not inferred: **30/30 `needs_citation: false`, 0 failures, and exactly 7
carrying `severity` absent** — the same seven, recovered in the shape the raw
text showed. `sentence_type`: AuthorOwnResult 12, CommonKnowledge 7, Definition
4, PriorWork 6, Transition 1.

#### THE THIRD DEFECT CLASS

Three prompt/schema defects have now been diagnosed on this task, and they sit at
three different layers. Confusing them wastes runs:

- **D58 — the rule was never stated.** The validator rejected on something the
  prompt did not say. *Fix: tell the model.*
- **D64 — the rules compete.** Stating a further rule displaced compliance with
  one already stated. *Fix: state fewer; a rule budget, not a list.*
- **D66 — the field could not exist in a legal state.** The spec demanded
  information that does not exist for a legal answer. *Fix: the schema. The
  model was right.*

**The third is the one where instinct misleads hardest.** `missing field
'severity'` reads as an omission by the model, and every cheap response — a
prompt line, a default, a retry — treats it as one. **Read the raw output before
blaming the model.** Seven discarded correct answers per thirty were the cost of
not doing so earlier.

#### The measurement tax, and what it means for D65

This was not only a user-facing defect. **A quarter of every eval arm carried no
signal, and not a random quarter** — the discarded sentences were exactly the
own-work ones, which is the population D58's rule targets and precisely what the
`own-work answered TRUE` metric counts.

**So D65's amendment was measured on a weaker population than its `n=30`
implies.** Its null result read `own-work answered TRUE = 0` in BOTH arms, and
part of that zero was these answers being counted as failures rather than as the
correct `false` they were. The null is not overturned — the six changed answers
there were failure/valid churn with no verdict flips — but it was **less cleanly
measured than the amendment currently states**, and it is re-run on the clean
denominator rather than left standing on a compromised one.

### D67 — Word keeps list numbers in `numbering.xml`, so 22 of 25 references were invisible

Found while closing the OA loop, and it blocked it: `R PAPER .docx` parsed **3**
reference entries where the PDF rendered from the same file parsed **25**.

**The numbers are not in the text.** Entries [1]-[22] are a Word auto-numbered
list (`ListParagraph` + `<w:numPr>`), and Word stores their ordinals in
`word/numbering.xml`, never in the paragraph. So `<w:t>` extraction yields the
entry text with NO marker, and `parse_numbered_bibliography` — which needs a
`[n]` delimiter — found nothing. Entries [23]-[25] were typed by hand with
literal markers, which is why exactly three survived.

**This is D65's principle one step further**, and it is pre-existing rather than
a D65 regression: the `prepass` shim (styles discarded) also returns 3.

#### The naive fix is WRONG, and the document says so

"Number the `ListParagraph` run 1, 2, 3…" is the obvious repair and it breaks on
CONTINUATIONS. Block 233 is `"pp. 436-465, 2013."` — a wrapped tail of entry [5],
in its own paragraph. Counting it shifts every later ordinal by one, and a wrong
reference number is worse than a missing one: it resolves a citation to the WRONG
paper, which is the failure this engine exists to prevent.

`<w:numPr>` does not separate them either — **all 24 `ListParagraph` paragraphs
carry it, the continuation included**, so Word itself counts that wrapped line as
its own list item.

#### The guard is agreement with what Word actually renders

The PDF was produced from this docx, so **its numbering IS Word's numbering**,
and it settles the question:

```
[5] S. Mohammad and P. Turney, "Crowdsourcing a word-emotion association lexicon,"
[6] pp. 436-465, 2013.
[7] Cortes and V. Vapnik, "Support-vector networks,"
```

**The document numbers the continuation as entry 6.** That is the author's own
off-by-one, and reproducing it is CORRECT: a reader's `[6]` is whatever their
document shows, not what they meant. So positional numbering of the list run is
right after all — it reproduces Word — and the earlier objection was an objection
to a fix that guessed, not to this one.

Verified end to end: docx now parses **25** entries and every one matches the PDF,
`[6]` quirk included. GoEmotions lands at [19] and SemEval-2018 at [22] on both
paths.

#### A literal marker is ground truth AND the check

Synthesised ordinals are kept ONLY while every literal marker agrees with the
running count. `R PAPER .docx` supplies exactly that confirmation: 22 synthesised
entries, then a literal `[23]` that matches. On disagreement the synthesis is
abandoned **wholesale** — not patched, not partially trusted — and only entries
carrying their own marker survive. Four tests pin it: the auto-numbered case, the
disagreeing case, the agreeing case, and an already-marked list item that must
not be double-numbered.

### D68 — the shipped audit path bypassed D65 and D67 entirely, and both were "verified" in probes

Found while closing the OA loop, and it invalidates how two prior entries were
checked.

`prepass_manuscript` — **the audit's own entry point** — still did this:

```rust
let blocks = parse_path_paged(manuscript)?;
let paged: Vec<(Option<u32>, String)> = blocks.into_iter().map(|b| (b.page, b.text)).collect();
Ok(prepass(&paged))
```

It converted every block to a `(page, text)` pair and called the SHIM. The shim
discards `PagedBlock.style`, and with it **every declared heading and table cell
(D65) and every auto-numbered reference ordinal (D67)** — on `R PAPER .docx`, 22
of 25 references. The product never received either feature.

**Both were measured with probes that called `prepass_blocks` directly.** The
probes were correct about `prepass_blocks`; they were not measuring the product.
Two D-entries described behaviour the shipped path did not have.

#### The pattern: verifying through something other than the real path

This is the SECOND time in this work that a measurement was taken through a path
the product was not running, and the two failures rhyme:

- The real-model needle tests reported **"2 passed … finished in 0.00s"** while
  silently SKIPPING — `GAPLY_TEST_MODEL_DIR` pointed one directory too deep. A
  green that measured nothing. Caught only because 0.00s is impossible for 500
  real embeddings.
- D65/D67 measured `prepass_blocks` while the product called `prepass`. A green
  that measured the wrong thing.

Both are the same error with different faces: **the verification did not go
through the path the user's work goes through, and nothing checked that it had.**
A passing check is evidence only about the code it actually executed.

#### THE STANDING CHECK

**Any pre-pass change must be verified through the SHIPPED entry point, not the
module it edits.** `the_shipped_manuscript_path_keeps_declared_structure` does
that: it writes a real `.docx` with a styled `Heading1` and an auto-numbered
reference list — ordinals in `numbering.xml`, absent from the text, the way Word
writes them — calls `prepass_manuscript`, and asserts both the bibliography and
the section survive.

**The guard was verified against the bug rather than assumed to catch it.**
Reverting `prepass_manuscript` to the shim makes it fail with
`left: 0, right: 2` and the message *"the shipped path lost the auto-numbered
reference list — it is calling the (page, text) shim again"*. A guard that does
not fail on the defect it names is not a guard.

The `prepass` shim is deliberately KEPT: it is the honest behaviour for a source
with no style information, and every existing caller and test depends on it. The
defect was never the shim's existence — it was the entry point choosing it.

### D69 — the retry costs `max_tokens` TWICE, and the budget was validated on fixtures

The first `citation_support` check ever run against a real open-access source —
fetched from OpenAlex while closing the OA loop — was **rejected before
generation**: `prompt is 3246 tokens; the configured context is 4096`.

#### The evidence bundle was not the problem

Measured, no model runs, on all three real fetched-source checks:

| seq | evidence tok | prompt tok | over the old 3072 budget? |
|---|---|---|---|
| 25 | 1390 | 2155 | no |
| 26 | 1361 | 2078 | no |
| 33 | 1200 | 1919 | no |

**0 of 3 over.** The first attempt fits comfortably. The 3246 was the RETRY.

#### THE INVARIANT

`retry_prompt` quotes the rejected reply VERBATIM — deliberately, per §11 D10:
*"showing it its own output is the point, and sanitising it would hide the very
thing being corrected."* So a retry costs the original prompt PLUS a reply that
may run to `max_tokens`, and **`max_tokens` is subtracted twice** — once for the
reply being generated, once for the reply being quoted:

```text
original <= TASK_N_CTX - 2*MAX_TOKENS - RETRY_OVERHEAD_TOKENS
```

`RETRY_OVERHEAD_TOKENS = 67` is measured, not assumed: 3246 - 2155 - 1024 = 67.

At 4096/1024 the ceiling was **1981**, and real prompts measure 1919-2155. **Two
of three real checks could not be retried at all.** They survived only by passing
first time; the one that did not was lost outright.

**The factor of two is the durable finding.** The specific numbers will move;
`a_retryable_prompt_fits_the_configured_context` asserts the invariant against
the configured constants, so changing either one without the other fails in the
test rather than in a user's audit. Verified against the bug: reverting to 4096
makes it fail with *"a real fetched-source prompt of 2155 tokens could not be
RETRIED: ceiling is 1981"*.

#### Why the context grew rather than `max_tokens` shrinking

Three levers, and only two avoid the evidence bundle — grounding has been refused
as a trade twice already (D62, D64):

| lever | headroom | cost | risk |
|---|---|---|---|
| `MAX_TOKENS` 1024 -> 768 | 2490 | none | **re-opens D58's truncation class** |
| **`TASK_N_CTX` 4096 -> 5120** | **2982** | **+37.7 MB KV** | none behavioural |
| trim evidence | — | grounding | refused |

**Chosen: the context.** It is the only lever with no behavioural failure mode.
37.7 MB against a measured 2295 MB working set is 1.6%, on a floor whose 8 GB
proof had ~4.4 GB of margin; prompts do not grow, so prefill is unaffected (the
context is an allocation ceiling, not a length); and the 3B advertises 32768, so
`effective_ctx` clamping is not a factor.

`MAX_TOKENS = 1024` IS over-provisioned — real post-`COMPACT_RULE` outputs measure
**max 326, median 234 (n=24)**, so it is 3.1x the observed maximum. It stays
anyway. Lowering it rests on `COMPACT_RULE` continuing to hold, and D64 is
precisely the finding that a prompt-side property must not be assumed stable
while a budget depending on it is changed. **If 1024 is revisited it gets its own
measured cell.**

A fourth option — truncating the quoted attempt — was rejected: the failure a
retry most needs to show the model is truncated JSON, where the defect is at the
END, so trimming the quote would hide exactly what the retry exists to correct.

#### THE POPULATION POINT

D58 justified `MAX_TOKENS = 1024` with *"the worst measured prompt is 2825"*.
That number came from **six synthetic seeds**. Real OA sources exceed it, and the
budget those fixtures justified could not retry two of the first three real
checks ever run.

**Budgets and thresholds are validated against real fetched sources, not
fixtures.** A fixture set is chosen for coverage of BEHAVIOUR; it is not a sample
of the size distribution, and using it as one silently sets limits to fit the
test data. `a_retryable_prompt_fits_the_configured_context` therefore pins
`LARGEST_REAL_PROMPT = 2155` — a measurement from a real fetched paper — rather
than a seed figure.

### D70 — the fold table was built from synthetic examples, and the first real paper broke it

D69 in miniature, and the same lesson: **a table built from examples I wrote is a
table validated against my imagination.**

The Greek and mathematical entries were added because a reference manuscript
rendered `p <= 0.05` as `p ? 0.05` — real evidence, but from a manuscript. The
first open-access paper ever pulled through the OA path (SemEval-2018, OpenAlex)
immediately produced two characters the table did not cover:

| in the fetched PDF | rendered as | now |
|---|---|---|
| `classiﬁcation` (U+FB01) | `classi?cation` | `classification` |
| `∼1,500` (U+223C) | `?1,500` | `~1,500` |

Four `?` glyphs in the exported report; **now zero**, and the marked-character
disclosure correctly stops firing because nothing is lost any more.

**A ligature is PURELY presentational** — `ﬁ` IS `fi`, with no semantic content —
so folding it is lossless rather than an approximation, the same argument the
maths entries already rest on. A `?` in the middle of a word is the one outcome a
reader cannot interpret: it hides which word was written, in a report whose
entire purpose is showing exactly what a source says.

Added: all five common ligatures (ﬀ ﬁ ﬂ ﬃ ﬄ) plus the two archaic st-ligatures
scans produce; the dashes and hyphens WinAnsi lacks (U+2010, 2011, 2012, 2015,
2053); quotes and angle brackets it lacks (U+201B, 201F, 2039, 203A, 27E8, 27E9);
the width-only spaces PDF extraction emits constantly for justified text (U+2002
-200A, 202F, 205F -> " "; U+200B and the BOM -> nothing); and the further maths
real papers carry (∼ ∽ ≃ ≅ ∝ ≪ ≫ ⋅ × ↑ ↓ ↔ ⇒ ⇐ ∈ ∅).

**NOT added, deliberately: the en-dash, em-dash and curly quotes.** Those are
WinAnsi code points already (0x96, 0x97, 0x91-0x94) and need no fold;
`every_ligature_folds_to_its_letters` asserts `latin_base` returns `None` for
them, so nobody adds a redundant entry believing it fixes something.

**The test is built from the real strings, not new synthetic ones** —
`ligatures_and_typography_from_real_fetched_papers_survive` renders the verbatim
text of chunks c469 and c474 from the fetched SemEval-2018 PDF. Fixtures are
chosen for coverage of behaviour; they are not a sample of what real documents
contain. That is D69's population point again, in a different subsystem, found
the same way — by running the real thing end to end.

### D72 — the locator was added one payload site at a time, and missed two of four

D65 gave every audit item a paragraph locator so a Word manuscript would stop
printing "no page numbers" 84 times. It reached two of the four sites that build
an item payload, and the misses were found one real audit at a time:

- the whole-manuscript **`citation_support`** site — found when a card had no
  locator to show;
- the whole-manuscript **`unverifiable`** site — found because a real exported
  report showed **67 paragraph locators and 16 "no page numbers"**, and the 16
  were exactly the unverifiable items.

Measured on `R PAPER .docx` before the fix: `citation_need` 65/65 carried a
paragraph, `citation_support` 3/3, **`unverifiable` 0/16**. After: **16/16**.

#### Why it kept happening

The plan builds payloads in four places, each a separate `json!` literal, in two
scope branches (`AuditScope::Citation` and `AuditScope::WholeManuscript`) that
mirror each other. Adding a field means editing four literals, and a reader
checking "did D65 land?" sees the field present at whichever site they open
first. Both misses were in the WHOLE-MANUSCRIPT branch — the one a thesis audit
actually takes — while the citation-scope branch, the one easiest to reach from
the tests, was right.

**This is the third instance of the same shape in this feature** (D68 was the
first two: the shipped entry point calling the shim, and the `citedSource` site).
Editing where you are looking is not the same as editing where the product runs.

#### The fix is a guard over the OUTPUT, not a tidier input

`locator_payload()` makes the fields easy to attach, but it does not make
omission impossible — a new site can simply not call it, which is exactly how
this happened twice.

`every_planned_item_carries_a_locator` asserts the property over **every item
the plan emits**, on a `.docx` fixture where a paragraph is the only locator
available. A payload site added later is covered without anyone remembering the
test exists. It also asserts the fixture produces **at least two item kinds**, so
it cannot pass by exercising only the arm that was already correct — the precise
way the earlier tests missed this.

Verified against the bug: reverting the unverifiable site fails with
*"Unverifiable item seq 0 has no paragraph locator — a payload site was added or
edited without one"*, naming the arm.

`examples/audit_report_preview.rs` also loses its paragraph backfill, which
joined sentences to ordinals by text because the payloads did not carry them.
They do now, so the workaround measured its own join rather than the product.

### D71 — a labelling tool, and the neighbour fields the product never sends

Every `citation_need` accuracy number so far has leaned on a fixed keyword
heuristic for "the authors' own work", applied identically to both arms —
honest about DIRECTION and SIZE, and explicitly not ground truth. D59 item 1
(the reason/verdict coupling defect) cannot be settled without a labelled set,
and D59 recorded that the blocker was eval throughput. Metal removed that; what
remained missing was the labels.

`src/bin/label-cn.rs` walks a real manuscript's planned sentences — the pre-pass
filtered ones are already gone — showing each with its section and locator, and
records `needs_citation`, `sentence_type` and `severity` on single keypresses,
appending to `evals/citation_need.jsonl` in the existing schema. It resumes by
SENTENCE TEXT rather than index, so re-running after a pre-pass change does not
re-ask what was already answered, and each case is flushed immediately so
quitting never loses completed work. Default scope is UNCITED sentences: those
are what the audit routes to `citation_need`, and a sentence carrying a marker is
a `citation_support` question.

`severity` is omitted when `needs_citation` is false — §11 D66 established the
field grades a need that does not exist — so the tool does not ask.

#### The finding: the seed set measures a configuration the product does not run

`job_runner` builds `CitationNeedInput` with `preceding_sentence: String::new()`
and `following_sentence: String::new()`. **The product never shows the model a
neighbour.** `ai-eval` deserialises `CitationNeedInput` straight from the case
file, so whatever a case carries is what the model sees.

The eight existing seed cases all carry real neighbours. **So the seed set has
been measuring a richer configuration than ships** — the §11 D68 shape again, in
the eval harness rather than the pre-pass.

The tool therefore writes neighbours EMPTY by default, matching the product, with
`--with-neighbours` to opt into the richer configuration deliberately. Not
resolved here, because it is a decision rather than a defect: either
`job_runner` should pass neighbours, or the seeds should stop carrying them. What
must not happen is a labelled set built to one convention and scored under the
other.

### D73 — the seed cases sent neighbours the product never sends

D71 surfaced this and deliberately left it as a decision. Decided: **the labelled
set measures what ships.**

`job_runner` builds `CitationNeedInput` with `preceding_sentence: String::new()`
and `following_sentence: String::new()`. **The shipped engine never shows the
model a neighbouring sentence.** `ai-eval` deserialises `CitationNeedInput`
straight from the case file, so whatever a case carries is what the model sees —
and **all eight seed cases carried real neighbours.**

So every `citation_need` number ever produced was measured with context the
product does not supply. That is the §11 D68 shape again, in the eval harness
rather than the pre-pass: **a check is evidence only about the configuration it
actually ran.** The list of instances is now long enough to be a pattern rather
than a coincidence — the needle tests that silently skipped, `prepass_manuscript`
calling the shim, two missed payload sites (D72), a fold table built from
synthetic examples (D70), a token budget sized on fixtures (D69), and this.

**All eight seeds are stripped**: `preceding_sentence` and `following_sentence`
are now `""` throughout. Nothing else changed — sentence, section and every
`expected` field are byte-identical, verified field by field rather than assumed
from the diff.

#### This is a decision about the PRODUCT, not a convention the eval may assume

If neighbours are wanted, that is a change to `job_runner` — give the model the
context — **with its own measured cell**, because more context is a different
configuration and D64 established that changes near this model are not additive.
What must not happen is the eval quietly assuming a richer input than the engine
provides and reporting the result as the engine's accuracy.

`citation_need_cases_send_the_neighbours_the_product_sends` asserts the property
against the DATA, so it holds for every case added later rather than depending on
whoever writes the next one. Verified against the bug: restoring a single
neighbour fails with *"case cn-seed-01: input.preceding_sentence is … but
job_runner sends an empty string … Clear it, or change job_runner and measure
that as its own cell."* — naming both remedies, so the test forces the two sides
to move together instead of drifting apart again.

`label-cn` already writes empty neighbours by default, so labelling done from
here is consistent with the stripped seeds; `--with-neighbours` remains for
anyone deliberately measuring the other configuration, and the guard will fail if
those cases land in the shipped case file.

### D74 — a score built on DECLINED judgements, and why one manuscript is a weak instrument

A pre-labelling audit of the whole feature turned up one thing that changed the
shipping picture, and one that changed how the labelled set must be built.

#### 0 of 65 was almost right, and the report presented it as a clean bill

`citation_need` answered `needs_citation = false` for **all 65** uncited
sentences on `R PAPER .docx`, on two consecutive runs, where the pre-D58 prompt
had answered `true` for 41 of 60. The obvious reading is over-suppression from
D58's addendum. **It is not**, and the reasons say so.

Classifying all 65 stated reasons:

| | count | share |
|---|---|---|
| sound — correctly identifies the authors' own work | 28 | 43% |
| sound — other correct grounds (figure reference, definition, standard practice) | 17 | 26% |
| generic template — *"a general statement about X"* | 16 | 24% |
| **contradictory (the D59 seq-42 shape)** | **2** | **3%** |

**The coupling defect is real but RARE — 3%, not the explanation.** (A first
keyword pass reported 4; two were false positives where the reason argued
correctly against and merely contained the words "high severity". Counting by
pattern needs the patterns checked.)

What explains 0/65 is the POPULATION. Grouping the uncited sentences by section:
16 in the pre-first-heading block (title/abstract/intro), 27 across eleven
methods-and-results sections, 13 in discussion/conclusion, and **1 in a
related-work section.** The paper's literature review is properly cited, so every
claim there carries a marker and routes to `citation_support`/`unverifiable`.
What reaches `citation_need` is overwhelmingly the authors' own work, where
`false` is the right answer and D58's rule is doing its job.

#### The report claimed more than that supports

`health 97 / 100` on a manuscript containing a miscitation the audit itself
caught. Arithmetically exact — 65 of 67 judged items "came back clean" — and read
by a researcher as *almost nothing to fix*, when 65 of that 65 was the model
DECLINING to flag rather than an issue ruled out.

The score stays one number and now states its basis:

> **97/100 — 65 of 67 items were judged not to need a citation; 2 raised issues.**
> The score is that fraction and nothing else — it counts no opinion Gaply did not
> form. 65 of those are the model deciding a sentence needs no citation, **which is
> not the same as confirming the sentence is sound.** The 16 sentences Gaply could
> not check at all are excluded from the score and listed separately.

The cover's **"Sentences checked" becomes "Sentences answered"** for the same
reason: a `citation_need` item answering `false` is `done`, so "83 checked"
invited the reading that 83 sentences were meaningfully assessed.
`a_score_resting_on_declined_judgements_says_so` pins all of it, built from
job 10's real ratio.

#### THE INSTRUMENT REQUIREMENT

**A labelled set drawn from one manuscript inherits that manuscript's citation
habits.** `R PAPER .docx` is a WEAK instrument for `citation_need`: with ~1
uncited related-work sentence, it cannot exercise the case the task exists for.
Fifty cases labelled from it would be ~70% "authors' own work -> false" and would
say almost nothing about false-negative risk — the failure mode that matters,
because a missed uncited claim is silent.

**The labelled set must therefore:**

- draw from **2-3 papers**, not one;
- include **at least one with a thinly-cited literature review** — the case
  `citation_need` exists to catch;
- aim for a **balanced true/false split** rather than whatever the source papers
  happen to produce.

This is the population lesson a third time — D69 (a token budget sized on six
synthetic seeds), D70 (a fold table built from examples I wrote), and now an eval
set that would have been built from whatever one paper contained. **A corpus is
chosen for the distribution it must measure, not for being the file already on
the desk.**

### D75 — model-suggested labels, and the three provenances that are not interchangeable

`label-cn --suggest` calls `citation_need` for each sentence, shows its answer
and reason VERBATIM, and lets the labeller accept with one key or override. It
makes 50 cases achievable in an evening instead of a week.

**It also contaminates every label it touches, and the contamination is not
optional — it is the point of the feature.** So the case file records HOW each
label was produced:

| provenance | what it is | what it can score |
|---|---|---|
| `cold` | labelled without seeing the model | **the only unbiased accuracy number** |
| `suggested_overridden` | the model proposed, the human disagreed | a **lower bound** — selected for disagreement |
| `suggested_accepted` | the model proposed, the human agreed | **nothing.** The label carries the model's own answer; comparing them is circular |

**Anchoring is why this is recorded rather than trusted to memory.** A confident
proposal shifts the reader's JUDGEMENT, not merely the keystroke, so "I would
have said the same anyway" is not evidence — it is the effect being measured.
Agreement rate on suggested cases is a real number and it is NOT accuracy; it
tells you how often a labeller ratifies the engine, which is a different question
and an interesting one.

**A held-out cold set therefore remains mandatory**, and is the number any claim
about `citation_need` accuracy must rest on. `--suggest` speeds up corpus
building; it cannot speed up measurement.

#### What the tool records, and two details that matter

The model's proposal is stored beside the human answer — `needs_citation`,
`sentence_type`, `severity` and the reason verbatim — so agreement is computable
after the fact rather than tallied at labelling time, and a `differs` map gives
PER-FIELD agreement, which an aggregate rate would hide (a labeller may keep the
boolean and reject the type, which is exactly the D74 pattern where 24% of
reasons were a generic template attached to a correct answer).

Two details, both found by running it:

- **`severity` is NOT COMPARED when no citation is needed.** §11 D66 makes the
  field conditional; a model that emits one anyway is producing the
  spec-compliant shape, not disagreeing. The first version marked it a
  difference, so an ACCEPTED case reported a disagreement with itself — a false
  difference that would have corrupted the per-field rate.
- **Suggestions use EMPTY neighbours**, matching `job_runner` (§11 D73). A
  proposal generated from richer input than the product supplies is a different
  engine's opinion, and accepting it would bake that difference into the label.

`labelled_cases_record_how_they_were_produced` asserts the provenance is present
and known, and that an accepted case never records a per-field difference — if
the record and the keystroke disagree, one of them is wrong.

### D76 — the labelled set, and the answer: recall is ZERO

42 cold labels (14 from `R PAPER .docx`, 28 from a health economics paper) plus
the 8 synthetic seeds. **No `--suggest` was used**, so every label is `cold` and
scores the model without contamination (§11 D75).

**Label distribution — and why the second paper was the whole point:**

| source | true | false |
|---|---|---|
| R PAPER | 3 | 11 |
| **health economics** | **14** | **14** |
| synthetic seeds | 3 | 5 |

D74 required a second paper with a thinly-cited literature review. It produced a
clean 14/14 split where R PAPER alone was 3/11, and without it the set would have
been ~21% true and unable to measure the failure mode that matters.

#### The confusion matrix, 41 cold cases

```
                 model: yes    model: no
  you: yes            0            17
  you: no             0            24
```

**Recall 0%. False-negative rate 100%.** The model answered "no citation needed"
to EVERY cold case, including all 17 labelled as needing one.

**This is over-suppression, and it corrects D74.** That entry concluded R PAPER's
0/65 was "substantially correct for this manuscript" — true of that manuscript,
and generalised too far. A paper with real uncited claims falsifies it.

**The accuracy figures are worthless without the matrix**, which is why they are
reported together:

| source | accuracy |
|---|---|
| R PAPER (14 cold) | 78% |
| health economics (27 cold) | 48% |
| **all cold (41)** | **58%** |

**A constant "no" scores exactly 58% on this set** (24 of 41 labels are false).
The model's 58% IS that constant. R PAPER scores 78% only because 11 of its 14
labels are false — the score tracks the label distribution, not the model. On
R PAPER alone this engine would have looked 78% accurate while being incapable of
a positive answer.

#### D59 item 1, measured at last: coupling is real but is NOT the cause

**4 of 17 false negatives (23%)** state a reason that argues FOR a citation while
answering no — `cn-label-019`: *"methods and statistical tests… borrowed from
existing literature"* -> `false`; `cn-label-016`: *"a transition sentence that
sets up the need for evidence"* -> `false`.

Higher than the 3% estimated from job 10's reasons alone, and still only a
quarter. **The other 77% argue coherently AGAINST.** That is a judgement problem,
not a plumbing one, and it means fixing the reason/verdict coupling would recover
at most a quarter of the misses.

#### The mechanism: misclassification, not the boolean

`sentence_type` agreement is **37% (18/48)**, and the confusion is one-directional:

| you said -> model said | n |
|---|---|
| `empirical_claim` -> `common_knowledge` | 6 |
| `empirical_claim` -> `transition` | 3 |
| `author_own_result` -> `common_knowledge` | 3 |

The model **over-uses `common_knowledge` (14x) and `transition` (5x)** and
**under-uses `empirical_claim` (12x)**. That is how recall reaches zero: an
empirical claim relabelled common knowledge needs no citation BY DEFINITION. The
boolean is downstream of a type error. D74's "24% generic template" finding was
this in miniature — lazy wording turns out to be actual misclassification.

#### D66 held; a new failure class appeared

**4% validation-failure rate, 48/50 valid.** Neither failure is the `severity`
class D66 fixed: `cn-seed-03` omitted `search_query` on a `true` answer, and
`cn-label-040` **invented the variant `research_question`**, rejected by serde at
parse time as `FATAL_RULES` documents. Enum invention is a class worth watching.

#### What this sample supports

41 cold cases is enough for **"recall 0% vs recall 60%"** and not for
**"58% vs 62%"** — one case is 2.4 points, and variant comparison needs the true
cell above ~30. Every claim here is of the first kind.

### D77 — v2 and v3 are mirror images, and neither is judging

D76 established that v3 has **zero recall**. The obvious question is whether the
D58 own-work addendum caused it. Same 41 cold cases, same device, only the prompt
differing:

| | v3 (with addendum) | v2 (pre-D58) |
|---|---|---|
| **recall** | **0%** (0/17) | **68%** (11/16) |
| **precision** | 0% | **42%** (11 of 26 "yes" were right) |
| accuracy | 58% | 50% |
| false positives | 0 | **15** |

```
v3                          v2
        yes    no                  yes    no
you:yes   0    17         you:yes   11     5
you:no    0    24         you:no    15     9
```

**The addendum causes the zero recall — settled.** Removing it moves recall
0% -> 68%, and 41 cases comfortably supports a difference that size.

**And v2 is not a fix.** It answers "yes" to 26 of 40 and is wrong on 15 — 42%
precision, so a researcher gets three false flags for every two real ones. Its
accuracy is LOWER than v3's, because on this set a blanket "no" beats an
over-eager "yes".

**Job 7's 41-true-of-60 was a blanket YES, not sensitivity.** That is D74's trap
seen from the other side: a population where the failing behaviour happens to
look like the right one. Neither prompt discriminates; they fail in opposite
directions.

#### The mechanism is a TYPE error, and the boolean is downstream of it

v2's `sentence_type` agreement is **10/40 (25%)** — worse than v3's 37%. What v2
calls `empirical_claim`:

| actually was | n |
|---|---|
| **`author_own_result`** | **12** |
| `empirical_claim` | 7 |
| everything else | 5 |

**v2 labels the authors' own results as empirical claims** — precisely the defect
D58 was written to fix, and it DID fix it. v3 under-uses `empirical_claim` 12x;
v2 over-uses it 12x. The prompts are mirror images:

- **v2**: everything is an empirical claim -> flag it
- **v3**: everything is common knowledge or a transition -> do not

D58's 18 -> 2 improvement was real. It overshot. The underlying weakness — **the
3B cannot reliably tell an empirical claim from the authors' own result** — was
present the whole time and neither prompt addresses it.

#### What 41 cases supports, and what it does not

**Supported** (all large effects): the addendum causes the zero recall; v2
recovers recall; v2's precision is poor; both prompts misclassify types badly.

**NOT supported**: that a *narrowed* addendum lands between them. That is the
obvious next move and predicting where a middle version falls needs the true cell
above ~30; it is at 16-17.

#### The lever this points at

Both failure modes are downstream of a type error at 25-37% agreement. **A prompt
that fixes the boolean while the type is still wrong is fixing a symptom** — the
type is currently CAUSING the answer rather than explaining it. Dropping
`sentence_type` from the decision path (ask only "does this need a citation, and
why", with the type emitted afterwards as description, or not at all) tests that
directly, and tests D59 item 1 from a different angle: if the boolean improves
once it stops being derived from a misclassification, the coupling was
STRUCTURAL rather than a wording problem.

### D78 — `citation_need` ships ADVISORY; `citation_support` is the feature

Three prompt variants, the same 41 cold labelled cases, the same device:

| arm | TP | FN | FP | TN | accuracy | **recall** | **precision** | type agree |
|---|---|---|---|---|---|---|---|---|
| v3 (own-work addendum) | 0 | 17 | 0 | 24 | 58% | **0%** | — | 31% |
| v2 (pre-D58) | 11 | 5 | 15 | 9 | 50% | 68% | 42% | 25% |
| **v4 (type demoted)** | 14 | 3 | **18** | 6 | 48% | **82%** | **43%** | 29% |

#### v4 answers D59 item 1: the coupling was NOT structural

v4 emits `needs_citation` and `reason` BEFORE `sentence_type`, so the boolean is
no longer generated downstream of a classification — the hypothesis being that
D77's 25-37% type agreement was causing the answer.

**Recall moved 0% -> 82%. Precision moved 42% -> 43%.** Type agreement stayed
flat (29% vs v3's 31%). Breaking the structural dependency changed the DIRECTION
of the errors and not their rate: all three variants are thresholds on the same
undiscriminating signal, and accuracy falls monotonically as recall rises
(58% -> 50% -> 48%), which is what trading one error for another looks like.

**So the reason/verdict coupling was a symptom, not a cause.** Removing it does
not recover judgement, and no further wording is worth measuring: three variants
spanning the full behavioural range all land at <=50% accuracy, and D64 already
established that rules compete rather than accumulate on this model.

**The 3B cannot make this call.** That is the finding.

#### DECISION 1 — advisory, never a verdict

At 82% recall / 43% precision, "here are sentences worth a glance" is honest and
useful; "these need citations" is not — fewer than half are real. Wired end to
end, and if a researcher can read it as a verdict anywhere, it is not done:

- **The health score excludes it entirely.** The score now covers only claims
  checked against a real source. `advisory_suggestions_do_not_move_the_score`
  asserts fifty advisory items shift it by zero.
- **No score below `MIN_SCOREABLE` (10) checked claims.** "0/100" from n=2 is as
  misleading as the 97/100 it replaces, so the report says *"a percentage from
  that few would be noise rather than a measurement"* instead.
- **The measured precision is IN the report**, so the claim is checkable — the
  same rule the score follows: *"it flagged 82% of the sentences that genuinely
  needed a citation — but only 43% of what it flagged actually did."*
- **The section is "Worth a second look — suggestions, not findings"** and opens
  with the caveat, not the list.
- **The attention list is evidence-backed only.** A 43%-precision suggestion
  cannot appear under "most need your attention".
- **The breakdown keeps separate bars.** Summing an advisory item with a checked
  finding is the conflation the score just removed.

**And the section lists only sentences actually FLAGGED.** `needs_citation`
carries every judged uncited sentence; on the real manuscript 65 of 65 came back
"no citation needed", and the section was listing all 65 — presenting 65
non-suggestions as a review list. Filtered, the report drops from 783 rendered
lines to 296 and the section honestly reads "None found."

#### DECISION 2 — `citation_support` leads

It checks prose against a real fetched source, and this session it caught a
genuine miscitation (the paper attributes ISEAR to `[22]`, which is SemEval-2018,
a tweet corpus). That is the half worth trusting, so the structure says so:
**evidence-backed findings lead, advisory suggestions follow.**

Order: the counts -> claims checked against their source -> cited but not
checkable -> worth a second look -> not judged.

#### What 41 cold cases supports

**Supported** (large effects): v4 recovers recall; precision is ~42-43% for both
v2 and v4; no variant exceeds ~50% accuracy; type agreement is 25-31% throughout.
**NOT supported**: v4 vs v2 on precision (one case), or any ranking between them.

Re-measure `ADVISORY_RECALL_PCT` / `ADVISORY_PRECISION_PCT` before changing the
model or the prompt — they are printed to users and they expire.

### D79 — a guard that looks flaky gets ignored; and a printed number nothing enforced

Two cleanups, and a bug found while doing the second.

#### `verify-clean-checkout.sh` no longer builds in `$TMPDIR`

macOS cleans `/var/folders/**/T` periodically, and it does so PARTIALLY: cargo's
fingerprints survive while a build script's generated output does not. The guard
then fails with

```
error: couldn't read .../libsqlite3-sys-*/out/bindgen.rs: No such file
```

which reads as a defect in a dependency and is nothing of the kind.

**That is the third time in one session that temp-cleaning produced a false
signal** — after `~/gaply-dev.log` and the eval logs — and it is the worst of the
three, because **a guard that looks flaky gets ignored, and a guard that is
ignored is not a guard.** The worktree and target now live under
`$HOME/.cache/gaply` (override with `GAPLY_CLEAN_ROOT` / `GAPLY_CLEAN_TARGET`),
the same reasoning CLAUDE.md already applies to the dev log: keep the thing you
need after something went wrong out of the directories that are cleaned exactly
when something goes wrong.

#### The advisory figures were describing a prompt that did not ship

D78 decided `citation_need` ships as v4 and the report prints its measured
`82%` recall / `43%` precision so the claim is checkable. **The presentation was
wired; the engine was not.** `PROMPT_VERSION` and `CitationNeedTask::new()` still
built **v3 — measured at 0% recall** — so for one commit the report advertised
catching 82% of uncited claims while the engine answered "no citation needed" to
every one.

Fixed: v4 is the default. **And the class of error is now enforced rather than
remembered.**

`advisory_figures_match_a_real_eval_of_the_shipped_prompt` does not check that a
report EXISTS. It finds the report whose `promptVersion` matches the SHIPPED
constant, recomputes recall and precision from its per-case results against the
COLD labels only (§11 D75 — a suggested-accepted label carries the model's own
answer and cannot score it), and asserts the printed constants match within a
point.

Verified against both drift modes rather than assumed:

- constant edited 43 -> 60: *"the report tells researchers precision is 60%, but
  the eval of the SHIPPED prompt citation_need-v4 measures 43% (tp 14, fp 18)"*
- default reverted to v3: *"the report tells researchers recall is 82%, but the
  eval of the SHIPPED prompt citation_need-v3 measures 0% (tp 0, fn 17)"* —
  which is exactly the bug that had shipped

Changing the prompt, the default variant, or either number without a matching
eval run now fails in the test rather than in a researcher's report. **That is
the only way a number printed to a user stays true.**

### D80 — the audit read Gaply's own report, and nothing stopped it

An audit was started on Gaply's own exported PDF instead of the manuscript. It
ran to completion — 98 items, a health score, a full report — and the output
looks like a normal audit. Nothing in the pipeline noticed that the "manuscript"
was a Gaply report.

**This will happen to researchers.** The audit PDF sits in the same folder as the
thesis, exported minutes earlier, with a similar name. The pre-pass accepts any
PDF, and a report that audits a report is not obviously wrong on the page — it
has sentences, sections and page numbers, and the model answers every one of
them.

#### What it actually judged

The five sentences that failed validation are all report furniture, and the
footer is glued onto the heading that follows it:

```
"84 sentences were read and 79 were checked against a source or judged for
 whether they need one."
"PublishReady 3 Sentences that may need a citation These sentences carry no
 citation."
"Check the citation is PublishReady 18 Not judged The model's answer for these
 did not pass Gaply's own checks, twice."
```

The 72 items that *passed* are no better — they are the same furniture with a
verdict attached. **The failures are the visible part of an entirely meaningless
run**, which is the point: a researcher reads the 72 that "worked".

#### The guard, and why it needs more than one marker

Detection sits in `prepass_manuscript` — the single function both `plan_audit`
and `preview_citation_audit` call — so the refusal happens before a job exists
and before anything is spent, and the preview refuses too.

`compose_audit` emits a fixed skeleton, and the detector keys on that skeleton
rather than on any one phrase: the cover title `Thesis citation audit`, the
`PublishReady` page furniture, and the section headings (`At a glance`,
`The counts`, `Claims checked against their source`, `Cited, but not checkable`,
`Worth a second look`, `Not judged`).

**One marker is not enough and must not be.** A genuine manuscript may quote a
heading, and a paper *about* this tool would name it. The rule is **three
distinct markers**. The message names the markers it found, so a false positive
is arguable rather than mysterious.

Measured rather than assumed, on both a composed report and the real file:

| document | markers | refused |
|---|---|---|
| `compose_audit` output today | 7 | yes |
| **`R-PAPER-.docx-citation-audit.pdf`, the actual mis-audited file** | **5** | **yes** |
| a manuscript naming two headings in prose | 2 | no |

**The real PDF scores 5, not 7** — it predates D78, so three of today's headings
are absent and the `PublishReady` footer is present instead. That is the case for
a threshold rather than a single sentinel: the guard has to fire on reports this
version of the composer never wrote, and it does, with two markers to spare.

Refused as `GaplyError::Validation`, matching how an unsupported format is
already refused before any work.

### D81 — the model supplied `search_query`; the error message said it had not

D80's mis-run failed 5 `citation_need` items twice on

```
search_query: is required when needs_citation is true (6-12 keywords)
```

which reads as an omission by the model, and reads as D66's mirror: D66 made
`severity` conditional for the `false` branch, and the `true` branch has the same
conditional shape. The obvious next move is to widen the schema the same way.

**D66's own rule is: read the raw output before blaming the model.** Doing that
refutes the hypothesis.

#### The raw output, reproduced 5/5 on the same five sentences

```json
{
  "needs_citation": true,
  "reason": "PublishReady 3 Sentences that may need a citation These sentences carry no citation.",
  "search_query": null,
  "sentence_type": "common_knowledge",
  "severity": "high"
}
```

`stop_reasons: [EndOfTurn, EndOfTurn]`, not truncated, in all five.

**The field is PRESENT, as explicit `null`** — the value the schema's own type
line (`"search_query": string|null`) offers. The model neither omitted it nor
malformed it. It answered the schema exactly as written and was told it had not
supplied a field it did supply.

**And `reason` is a near-verbatim echo of the input sentence in all five.** The
model is not reasoning; it is parroting, because the input carries no
proposition to reason about. These are not sentences — they are report furniture
with a page footer glued to the heading below it (D80).

#### So the proposed schema fix is REFUSED, and here is why it is not D66

D66's test is *does this field carry information that exists?* For
`needs_citation: false` a severity-of-need does not exist, so requiring it forced
the model to invent or be rejected. The mirror does not hold: when a citation IS
needed, a query for it exists, and it is **the entire actionable payload of the
advisory list** — D78 ships this task as "here are sentences worth a glance", and
a suggestion with nothing to search for is not a suggestion.

`needs_citation: true` with no query is a genuinely incoherent answer, and the
FATAL is right to refuse it. Widening the schema would not recover a correct
answer as D66 did; it would launder five meaningless judgements into a list a
researcher reads. **The verdict was wrong, not the schema.**

#### The correlation says the same thing, and it is total

| population | prompt | judged | this error |
|---|---|---|---|
| Gaply's own report (job 12) | v4 | 77 | **5** |
| real manuscript (job 13) | v4 | 54 | 0 |
| 42 cold labelled + 8 seeds | v4 | 50 | **0** |

**Every occurrence in the whole job history is in the one mis-run.** The
pre-v4 jobs (2-7) failed 22 times on the OPPOSITE branch — query present when
`false` — and never once on this one. The trigger is D80's population, and D80's
guard is the fix. These two items are one defect.

#### What IS fixed here: the engine could not tell absence from null

Two changes, both about being able to diagnose this next time in a query rather
than a session:

1. **`search_query` becomes `Option<Option<String>>`.** Absent and `null` are
   different facts about what the model did — D66's diagnosis turned on exactly
   that distinction, argued from the error text alone — and the validator could
   not represent it, so its message asserted the wrong one. Both stay **FATAL**;
   only the message changes, and it now says which happened.

2. **`job_runner` keeps `first_raw` and `retry_raw` on failure.** `TaskError::
   ValidationFailed` carries both raw outputs specifically so a human can see
   what the model said, and `failed_outcome` was discarding them, storing only
   `{category, outcome}` plus the Display message. **The product path made D66's
   own rule impossible to follow** — this session had to rebuild the five inputs
   from `ai_job_items.sentence` and re-run the model to see output the engine
   had already had in hand.

#### Measured on the 41 cold cases, before and after

Neither change can move a rate — one alters a message, the other stores a
string — and "it cannot" is not a measurement, so it was run rather than
reasoned. Same binary, same device, greedy decoding:

| | before | after |
|---|---|---|
| validation failures, 50 cases | 2 (4%) | 2 (4%) |
| **this error, 42 cold cases** | **0** | **0** |
| `needs_citation` accuracy | 48% | 48% |

The two failures are unchanged and neither is the reported one: `cn-seed-07`
omits `severity` while `true`, and `cn-label-007` puts `"high"` into
`sentence_type`.

And the message was checked on the inputs it exists for — the same five
sentences, 5/5:

```
search_query: was supplied as null while needs_citation is true — the reply
asserts a citation is needed and gives nothing to search for
```

#### Left as a recommendation, not taken

**`severity` is a constant, and it is FATAL.** It came back `high` in 37 of 37
valid cold outputs and 5 of 5 here — it never varies, so it grades nothing. Its
only product consumer is `CitationAiPanel`, which already renders `'unrated'`
when it is absent; the audit report never reads it. Yet `severity` absent while
`true` is FATAL, and it discarded one of the two cold-set failures — a judgement
thrown away over a field that is read in one place, defaulted there, and
constant when present.

That is D66's shape one field over, and the evidence for it is in this section.
It is **not** changed here: it alters what ships for an unmentioned field, and
D66's own tier decision needed a reversal before it was right. It is a decision,
and the decision is not mine.

### D82 — `severity` graded nothing, and a FATAL rule enforced it

D81 left this as a recommendation because it changes what ships for a field
nobody had raised. Taken now, on the evidence D81 gathered.

#### The field carries no information, measured

| | |
|---|---|
| value in valid cold outputs | `high` **37 of 37** |
| value in the D80 repro | `high` **5 of 5** |
| read by `audit_report` | **never** |
| read by `CitationAiPanel` | yes — as `out.severity ?? 'unrated'` |

**It never varies, so it grades nothing.** Its single consumer already renders a
default for absence, and the PDF the researcher actually reads does not mention
it. Yet `severity` absent while `needs_citation` is true was **FATAL** — one
retry, then the whole judgement discarded. That cost one of the two remaining
failures on the labelled set (`cn-seed-07`), whose answer was otherwise complete
and correct.

That is D66's shape — a rule enforcing a field that cannot inform the reader —
on the field the evidence indicts rather than the one the error message pointed
at (D81).

#### ADVISORY, not silently accepted

The two tiers are declared, and the choice follows from them: FATAL is for output
that is **actively misleading**, ADVISORY for output that is **untidy**. A
missing constant misleads nobody, so it is not fatal. But it is also not
compliant — `RULES_V4` asks for `severity` and excuses it *only* when
`needs_citation` is false — so it is not nothing either.

**This is the opposite call to D66's fourth row, and deliberately.** There,
`severity` present on a `false` verdict was accepted SILENTLY because the spec
asks for the field unconditionally and the model was doing as it was told;
an advisory would have penalised correct behaviour and inflated `advisoryRate`,
which the bake-off reads as a quality signal. Here the model is *not* doing as
it was told, so counting it is exactly right — and `advisoryRate` keeps
visibility on a behaviour worth watching, given `cn-label-007` put `"high"` into
`sentence_type` in the same run. Field confusion is a thing this model does.

Accepting also removes the retry, which on this task is a whole generation that
has been observed returning an empty string.

| `needs_citation` | `severity` | before | after |
|---|---|---|---|
| true | present | ok | ok |
| true | **absent** | **FATAL** | **ADVISORY** |
| false | absent | legal (D66) | legal |
| false | present | accepted silently (D66) | accepted silently |

#### Measured on the labelled set, same binary, idle machine

| | before | after |
|---|---|---|
| validation failures, 50 cases | 2 (4%) | **1 (2%)** |
| retry rate | 4% | **2%** |
| valid outputs | 48 | **49** |
| `needs_citation` accuracy | 47.9% | **46.9%** |
| advisory rate | 36% | 38% |
| `severityAgreement` | 37.5% | 36.0% |
| `sentenceTypeAgreement` | 33.3% | 34.7% |

`cn-seed-07` is recovered — it returns a judgement instead of nothing, carrying
two advisories (absent severity, a 5-word query). The one remaining failure is
`cn-label-007`, a genuine parse error (`"high"` in `sentence_type`), which stays
fatal and should.

#### THE ACCURACY WENT DOWN, AND THAT IS THE POINT

This was predicted to rise before it was run, and the prediction was wrong in
DIRECTION, not just in size. It is recorded because the reason matters more than
the number:

```
cn-seed-07  "Plants require light to photosynthesise."
            expected needs_citation: false   (common knowledge)
            model    needs_citation: true
```

**The recovered case is a MISS.** Accuracy is unchanged in the numerator (23) and
gains one in the denominator: 23/48 -> 23/49.

So the fatal rule was not only discarding correct answers, as D66 found. **Here
it discarded a WRONG answer, and the accuracy figure was flattered by its
absence.** Every rate on this task was computed over the subset that survived
validation, and validation was silently removing cases on a criterion unrelated
to whether the model was right. `severityAgreement` falls for the same reason —
a discarded case is not scored at all, a recovered one with no grade scores as
disagreement.

**A number that improves when you delete the cases you failed to judge is not
measuring judgement.** 46.9% is the worse number and the truer one, and it is
the one D78's conclusion — that the 3B cannot make this call — already rests on.
Nothing user-facing moves: `ADVISORY_RECALL_PCT` / `ADVISORY_PRECISION_PCT` are
computed over cold labels only and `cn-seed-07` is a seed, so §11 D83's guard
still passes against the report of record.

### D83 — a test that could silently rebind the number it validates

`advisory_figures_match_a_real_eval_of_the_shipped_prompt` (D79) binds
`ADVISORY_RECALL_PCT` / `ADVISORY_PRECISION_PCT` to a real eval report. It found
that report by scanning `evals/reports` and taking **the first entry whose
`promptVersion` matches** — and `read_dir` order is filesystem order, not a
decision.

So a second report for the shipped prompt — a diagnostic run, a re-measure, a
five-case reproduction — could become the authority for two numbers printed to
researchers, without anything changing in the test or the constants. During D81
this was live: those runs were deliberately kept out of `evals/reports` for
exactly this reason, which is a workaround standing in for a fix.

**A guard that can silently change what it validates is worse than no guard**,
because it still reads as one. Now: collect every matching report, sort by
`date` (descending, ties broken by filename so the order is total), and take the
newest — with the chosen report's filename in every failure message, so the
assertion says which measurement it is speaking for.

### D84 — FOLLOW-UP, NOT DONE: `ai_index_document` has no D80 guard

D80 refuses our own report at the audit pre-pass. `ai_index_document` parses a
PDF on a different path — `parse_path_paged` straight into `ai_chunks` — and has
no such check.

**This is worse than the mis-audit it is adjacent to.** Indexing an audit report
as a source document makes it retrievable evidence, so `citation_support` can
quote Gaply's own prose back as the source a claim was checked against. D18's
whole premise is that a finding is trustworthy because the evidence beside it
came from a real document; this would satisfy the letter of that and violate its
point, and the resulting card would look exactly like a genuine one.

Not fixed here: it is a different path with a different refusal decision to make
(refuse outright, or index but exclude from retrieval), and the detector is
already in `gaply_core` and callable from both.

### D85 — `severityAgreement` scored a field with no variance, and said nothing

D82 established that `severity` is `high` in 37 of 37 valid v4 outputs. The
report went on printing

```
severity agreement      : 36%
```

as though that were a quality measurement. It is not: every valid output carried
the same value, so the number is a property of the LABELS — how many of them
happen to say `high` — and cannot move for any reason to do with the model's
judgement. **A number that cannot respond to the thing it names is worse than
absent**, because absent invites a question and 36% invites a conclusion.

#### Dropping it would be wrong, and the reports say why

The obvious fix is to delete the metric. The evidence refuses it — the field is
not inherently constant, and the collapse is a property of **v4 specifically**:

| prompt | model | severity distribution | agreement |
|---|---|---|---|
| v2 | 1.5B | `{high: 4, medium: 2}` | 50% |
| v2 | 3B | `{high: 40, low: 7}` | 35% |
| **v3** | **3B** | **`{high: 20, low: 23}`** | **25%** |
| **v4** | **3B** | **`{high: 37}`** | **38%** |

v3 graded the corpus almost evenly. **v4 collapsed it**, and that collapse is
itself a finding about v4 worth keeping visible — deleting the metric would
delete the evidence for it, and would leave nothing to notice if a future prompt
restored the variance.

#### So the metric self-reports its own degeneracy

`severityDistinctValues` goes in the report, and the agreement figure is marked
where it is printed:

```
severity agreement      : 36%  [DEGENERATE: every valid output said "high" —
                                no variance, so this cannot measure judgement]
```

The bake-off table carries the same marker, since that table is read across
models and is exactly where a collapsed column would otherwise be compared
against a varying one as if they were the same kind of number.

**The condition is computed, not asserted.** It keys on the distribution the
report already builds — one distinct value across the valid outputs — so it
turns itself off if a prompt ever grades again, and no constant in the code has
to be remembered and updated.

### D86 — the frontend suite's flaky test was never flaky; it had no headroom

A `npm run test:design` run came back 692/693. Re-running passed, and **twelve
consecutive idle runs passed**. That is the shape D79 warns about: a failure that
cannot be reproduced gets called noise, and the suite quietly stops meaning
anything.

#### Reproduced deliberately, by restoring the condition

The original failure happened while `cargo` was compiling in the background, so
the variable was LOAD, not chance. Twelve spinners on eight cores, and it failed
on the **first** run:

```
FAIL src/screens/citations/cslEngine.vitest.ts > spot-checked known-correct
     formatting > Chicago author-date: quoted title, year after authors
Error: Test timed out in 5000ms.
```

**A timeout, not an assertion.** Nothing raced and nothing was ordered wrongly —
the test simply did not finish.

#### The measurement, and why the first one misled

| condition | duration |
|---|---|
| every other assertion test in that file | 0-16 ms |
| Chicago, file run **alone** | 713 ms |
| Chicago, **full 61-file suite** (files run in parallel) | **2,871 ms** |
| vitest default `testTimeout` | 5,000 ms |

Measuring the file alone suggested a comfortable 7x margin. Running the suite as
it is actually run shows **1.7x** — and a loaded CI runner erases that easily.
`chicago-author-date.csl` is 167 KB against `nature.csl`'s 6 KB, and citeproc
compiles the whole style; the cost is real work, not a defect.

**And it is not one test.** Six others exceed 1 s in the parallel suite
(1,314 / 1,214 / 1,139 / 1,136 / 1,074 / 964 ms). The suite does real citeproc
formatting over the full bundled style repository, and the 5,000 ms default is
sized for trivial unit tests. **The mis-sized ceiling is the defect**, not the
one test that reached it first.

#### `testTimeout: 20000`, at the config, with the numbers beside it

Raised globally rather than annotating the one test that happened to lose the
race: the next-slowest are within 4x of the same wall, and patching them one
timeout at a time as each surfaces is how a suite ends up with six unexplained
magic numbers. 20 s is ~7x the measured worst case.

The cost of a higher ceiling is that a genuinely hung test takes 20 s rather than
5 s to fail. Against a 30 s suite that is a fair trade for a guard that stops
lying under load.

### D87 — D84 taken: the import paths refuse our own report too

D80 refuses a Gaply report at the AUDIT pre-pass. Three other paths create an
indexable document and none of them checked: `ai_index_document`,
`ai_link_source_document`, and the open-access fetch.

**This is the worse of the two.** An audit of a report produces nonsense about
report furniture; an INDEXED report becomes retrievable evidence, so
`citation_support` can quote Gaply's own prose back as the source a claim was
checked against. D18's premise is that a finding can be trusted because the
evidence beside it came from a real document — this satisfies that to the letter
and violates its point, and the resulting card is indistinguishable from a
genuine one.

#### Refuse, not index-but-exclude

Excluding an indexed document from retrieval is a rule sitting somewhere distant
from the reason it exists, and the next person to touch retrieval can relax it
without ever learning why. **A refusal states its reason at the point of
failure**, to the person doing the thing, at the moment they do it.

#### One gate, because three call sites is how D72 happened

The three paths already share `import_guard::preflight`, whose recorded design is
that *every one of its answers should be a verdict*. So the check goes THERE, and
no call site changes at all — they already handle `PreflightVerdict::Refused`.

Adding it to three sites one at a time is exactly the shape of D72 (a field added
per payload site, missed at two of four) and of D68 (a fix verified on a path the
product did not call). Here there is nothing to miss: a fourth import path
written tomorrow inherits the refusal by calling the gate it must call anyway.

**And it costs no extra parsing.** `inspect_path` already extracts the full text
and discards it, keeping only `text_chars`. It now returns that text to
`preflight` via `inspect_path_with_text`, so the markers are matched against
something already in memory. The policy stays in `import_guard`, which is a
policy module; `extract` keeps returning data and gains no opinion about audit
reports.

#### Tested on a REAL exported report, twice, for different reasons

- **Generated by the live pipeline** — `compose_audit` -> `render_pdf`, the exact
  two calls the export command makes — so the test cannot go stale as the report
  changes.
- **The actual mis-audited file**, committed as a fixture. It is the OLD report
  format and carries only 5 of the 8 markers (it predates D78), which is the
  case a freshly-generated PDF can never cover: **a real report this version of
  the composer never wrote, still refused.**

Plus the negative: an ordinary manuscript still imports, because a guard that
refuses everything is not a guard.

#### The threshold, measured against real documents rather than argued

D80 picked three markers by reasoning. Eleven real files off the Desktop —
including four documents *about* Gaply, which is the false-positive case the
threshold exists for — say it was right, and by a wide margin:

| document | markers | refused |
|---|---|---|
| `R PAPER .pdf` (the manuscript) | 0 | no |
| `IJAS Manuscript … haemolymph.pdf` | 0 | no |
| `chapter 1 .pdf`, `final final L.pdf` | 0 | no |
| Gaply Remediation Plan / SLM Training (design docs) | 0 | no |
| Gaply Desktop Blueprint / Frontend Build Prompts | 1 | no |
| `gaply-publishready-…pdf` (a DIFFERENT report type) | 1 | no |
| **`R-PAPER-.docx-citation-audit.pdf`** | **5** | **yes** |
| **`…-citation-audit-citation-audit.pdf`** (an audit OF an audit) | **8** | **yes** |

**Nothing scored 2, 3 or 4.** Real documents land at 0-1 and real reports at 5-8,
so the threshold sits in an empty gap rather than on a judgement call — including
for documents that discuss the tool at length and still only reach 1.

#### KNOWN LIMIT, stated rather than discovered later

`gaply-publishready-…pdf` is **also our own output** — a PublishReady report — and
it scores 1, so it is NOT refused. `GAPLY_REPORT_MARKERS` describes the *thesis
citation audit* skeleton specifically, and no other report type is covered.

Indexing a PublishReady report is the same class of problem in a milder form. It
is left alone deliberately: covering it means a second marker set bound to a
second composer, which is a decision about that report rather than about this
one, and guessing at markers for it would put untested phrases in a guard that
currently has a measured empty gap on either side of its threshold.

### D88 — the audit started before the confirmation, and the card measured the wrong thing

Two defects on one screen, found while acting on the investigation's §14.

#### The "Before you start" card was not a confirmation

`choose` calls `ai_job_start_thesis_audit`, and that command plans the job and
then calls `spawn_job_runner` — which runs items immediately. **By the time the
card asking "Start the audit?" renders, the model is already generating.**
"Not now" abandons a run that has been going for as long as the user spent
reading the card.

The frontend even asserts the opposite, in a comment written to reassure:

```
// NOTE: the backend plans AND persists the job in one call, so this is the
// point of no return for item creation — but NOT for running the model.
// Nothing is generated until `start` below.
```

`start` calls `resume_job`, which re-queues nothing on a job whose items are
already `queued` or `running`, so it *looks* like the trigger. It is not.

#### And the number it showed was the wrong grain

The card reported `queuedUnverifiable` — a count of SENTENCES. A researcher
does not have 40 unverifiable sentences; they have **four sources without a
PDF**, cited 40 times. The sentence count reads as a manuscript problem when it
is a library gap, it is unactionable (there is no per-sentence fix), and it
hides how small the actual remedy is.

#### `preview_thesis_audit`: no job, no model, per SOURCE

A real preview — pre-pass and marker resolution, no `create_job`, no runner —
mirroring `preview_citation_audit`, which already establishes that a preview
creates nothing. It returns the sentence tallies AND `sources`: one row per
distinct cited work, each with whether a check could run against it and, when
not, the deterministic reason.

The card then says what the run can actually do, before it does any of it:

> **6 of 10 cited sources have a PDF Gaply can read.** The other 4 can be
> flagged, but not verified against their source.

with the fetch and attach actions offered **there**, on the blocked sources,
rather than discovered three hours later in a section titled "Cited, but not
checkable".

### D89 — the advisory list wore the findings' clothes

§14's second item. `emit_finding` renders every item the same way: a
`Block::Badge` carrying the verdict, then the sentence. So a `citation_need`
suggestion — 43% precision, checked against nothing — arrived in the same
badge, at the same weight, as a `strong` verdict backed by a quoted passage.

The section's caveat was a paragraph at the top. **A reader who lands on item
19 never sees it**, and by then the badge is the only signal present.

- Suggestions get their own emitter: **no verdict badge**, a plain
  `suggestion` label, lighter weight.
- **The measured rate rides with each item**, not only in the intro — a reader
  meeting one suggestion in isolation still learns it is right slightly under
  half the time.
- Same on the screen: the `citation_need` group loses the `assessed` badge that
  made it look adjudicated.

No severity anywhere in the report — `ADVISORY_*` figures are about the list,
and per-item severity is a constant (§11 D82) that would read as triage.

### D90 — Manager and Audit: the CONDITION already agreed; the offer did not

§13 of the investigation claimed the two features disagree about when a
citation can be checked. **Re-reading the code, that claim was wrong** and is
corrected here: `ai_citation_document` calls
`checkable_document_for_citation`, which is the identical predicate
`plan_audit` uses. Both require a linked document with chunks AND embeddings.

The real inconsistency is what happens next:

| | Audit | Manager |
|---|---|---|
| condition | `checkable_document_for_citation` | **same** |
| when false | queues `unverifiable`, reports it grouped by source with an action | one static sentence, **no way to fix it** |

**Chosen: both refuse, with the same words and the same affordance.** Not "make
the Manager run and report unverifiable like the Audit" — because the Audit does
not run the model on those items either. It marks them deterministically and
moves on. Running a check that is known in advance to have nothing to read would
spend a minute of model time to produce a sentence the database could have
written.

So the refusal text becomes one shared constant, and the Manager gains the
fetch/attach action the Audit already offers. Same condition, same message, same
next step.

### D91 — the PDF was never the problem; the delivery was

"Exported PDFs download as .txt". Both PDF generators were checked first, on
REAL FILES rather than on bytes in memory:

| path | mechanism | the file on disk |
|---|---|---|
| Thesis audit export | Tauri `save()` + `writeFile` | `PDF document, version 1.4, 18 pages`, `com.adobe.pdf` — **correct** |
| `miniPdf` (PublishReady) | — | `PDF document, version 1.4, 1 pages` — **bytes correct** |

Both produce a valid `%PDF-1.4` … `%%EOF`. **Neither generator is broken**, and
the one export that reaches disk through the native dialog has been landing as a
real PDF all along.

The difference is the delivery. `downloadReportPdf` builds a Blob and clicks an
`<a download>` — the browser pattern — **in the desktop app**, where the webview
handles the download rather than the OS, and the name and extension it was given
are not what it saves.

**This repo already knew.** `saveTextFile` exists precisely because of it, and
says so in its own header:

> The native Tauri save dialog + writeTextFile in the desktop app; a Blob
> download in the browser. **The native dialog sidesteps the webview's
> download-scope limits.**

Text got that treatment. Binary never did, so the PDF export kept the pattern the
text export had already abandoned.

#### `saveBinaryFile`, the sibling that should have existed

Same contract as `saveTextFile` — cancel → `null`, write failure → throws,
success → the path — over `Uint8Array` instead of `String`, and used by BOTH PDF
exports so there is one binary save path rather than two.

Two things it adds that neither caller had:

- **A dialog filter.** `save({ defaultPath })` with no `filters` lets the panel
  decide what the extension means. Naming the type keeps `.pdf` attached.
- **An extension check on the returned path**, because a filter is a request to
  the OS and the returned path is the fact. Appended when missing, which is also
  the half that can be tested without driving a real dialog.

Tested on the FILE, not the bytes: the suite writes the exported PDF to a real
path and asserts `file`-level truth — the `%PDF-` header, the `%%EOF` trailer,
and a `.pdf` extension that survived the round trip.

### D92 — the annotated manuscript: highlights on the real page, PDF only

The report describes a manuscript the reader cannot see. This shows the paper as
it is, with every judged sentence highlighted in place.

#### `.docx` is refused, and the refusal is the honest half

A `.docx` is not paginated until Word paginates it: page breaks depend on the
reader's fonts, printer and margins, so **the geometry is not in the file** and
no parser can recover it. §11 D65 already substitutes paragraph ordinals for
page numbers for exactly this reason.

A reconstruction that looks subtly wrong to the person who wrote the paper is
worse than no reconstruction, so there is none. The screen says the annotated
view needs a PDF, says WHY in one line, and points at the paragraph anchors the
existing report already carries.

#### Anchoring: proven before it was built

Measured on `R PAPER .pdf` against the real pre-pass output, 82 planned
sentences:

| | first attempt | shipped approach |
|---|---|---|
| located confidently | 82.9% | **97.6% (80/82)** |
| ambiguous | 6 | **0** |
| unplaceable | 8 | 2 |

Two corrections got it there, and neither is an approximation:

- **Search every page, not the stored one.** The text is the identity; the
  stored page is a claim. This alone took 82.9% -> 90.2%.
- **Order-aware duplicates.** This paper repeats three sentences between its
  Discussion and Conclusion, and the pre-pass plans each twice. Both lists are
  in document order, so the k-th planned copy takes the k-th occurrence. That
  is exact.

**The 2 that remain are omitted, not approximated.** One is two figure captions
glued together by extraction and exists as contiguous text on no page; the other
spans a column break. Both appear in the detail list with a note saying the
sentence was judged but could not be located. A highlight on the wrong sentence
is worse than a missing one.

#### AND IT FOUND A LIVE BUG IN THE SHIPPED REPORT

Locating by text exposed that **6 of 74 locatable sentences (8%) carry the wrong
page number**, every one off by one — `pdf-extract`'s page splitting disagrees
with the real page boundaries:

```
pre-pass p.3 -> "The highest F1-scores are for joy (97.0%)…"  is on page 4
pre-pass p.4 -> "Using only FA or only CSA…"                  is on page 5
```

The audit's whole value is "go and look at this sentence", and for roughly one
in twelve it sends the reader to the wrong page. **Not fixed here** — it is a
separate defect with its own decision (correct the page assignment, or derive
pages from anchoring and demote the stored value to a hint), and this change is
scoped to the annotated view. Recorded so it is not rediscovered.

Inside the annotated view it cannot bite: anchoring never trusts the stored page.

#### Colour is never the only signal

Four statuses, and each carries **three** cues — fill colour, a left-edge
treatment, and a word in the legend and in every detail entry:

| status | fill | edge | weight |
|---|---|---|---|
| verified with evidence | green | solid | full |
| weak or contradicted | red | solid | full |
| cited but not checkable | grey | solid | full |
| **suggestion** | amber | **dashed** | **lightest** |

The last row inherits §11 D89 rather than inventing its own: a 43%-precision
suggestion is visually subordinate to an evidence-backed finding here too, and
the dashed edge keeps it distinguishable in greyscale and to a colour-blind
reader. `no_citation_needed` gets no highlight at all — it is not a finding.

### D93 — six defects in the exported report, found by looking at it

Reported from the PDF itself, not from preferences. Each was reproduced by
regenerating a real job's report and reading it, and each fix was verified the
same way.

#### The bars were a terminal's bars

```
- could not be checked ===== 16 19%
- suggestions only ================== 65 77%
- not judged = 1 1%
```

`"=".repeat(n)` inside a bullet is a chart in a monospaced terminal and a run of
punctuation anywhere else. Worse, the `{:<24}` and `{:>4}` padding that was
meant to align the count is **spaces in a proportional font**, so it aligned
nothing and the two numbers collided: `16 19%`, `1 1%`, `0 0%`.

`Block::Bar { label, value, total, tone }` replaces it. The composer supplies
the facts and each renderer draws them — the layering the `Badge` comment
already described. Three columns that cannot collide: label, a drawn track, and
the count right-aligned **by measuring its width**, never by padding.

**There were TWO ASCII-bar sites.** The verdict breakdown used the same trick
with `{:<26}`; converting only the visible one would have left the defect
alive one section further down.

#### The findings were not clipped — they were painted over

Descenders vanished on the last line before a shaded panel. The cause is one
line:

```rust
let block_top = y;   // y is the PREVIOUS element's final baseline
...
fill_rect(MARGIN_X, y, TEXT_W, block_top - y, PANEL);
```

Decoration is drawn before text *within* an element, and the comment says so —
but across elements the order is emission order, so block N+1's panel fill
covered block N's descenders, which hang below the baseline `block_top` sits on.

Fixed structurally rather than by nudging a constant: a page now accumulates
**decoration and text in separate buffers** and emits all decoration first. A
fill can no longer cover earlier text, whatever the block order.

#### And three more

- **Grammar.** "The 1 checked claim that most **need** your attention" — the
  noun agreed and the verb did not.
- **Page 1 was four-fifths white**, carrying five metadata pairs and nothing a
  reader came for. `Cover` gains an optional `headline`: the report's ANSWER,
  set large — *"80 / 100 — 8 of 10 checked claims held up"*, or *"Only 2 claims
  could be checked — too few to score"*.
- **No colour anywhere**, though `Tone` existed and the badge already used it.
  Bars and the cover headline now carry it.

Four tests, including two that hold the shape rather than the wording: one
asserts no `====` survives anywhere in a composed report, and one scans every
line for the `<digits> <digits>%` collision that shipped.

### D94 — the consistency pre-flight: deterministic, and BEFORE the audit

Twenty real defects were found in `R PAPER` by hand. **None needed a model.**
Seven of them are computable from what the pre-pass already parses, and they run
as a gate before any item is queued.

#### Why a gate and not a report section

The reference list is off by one from `[6]` — a page-range continuation numbered
as its own entry — so most in-text markers resolve one paper too far: `BiLSTM [7]`
lands on an SVM paper, `ISEAR [22]` on SemEval-2018, `GloVe [24]` on SMOTE.

**That silently invalidates most of the audit's own resolutions.** Every
`citation_support` verdict would be checking a claim against the wrong paper,
and the reader could not tell which findings survived. Reporting it in a section
at the end means three hours were spent producing verdicts nobody can use.

So it runs before queueing, and it says what it found and what it costs:

> The reference list has a structural problem — `[6]` does not look like a
> reference entry, so markers above it may resolve one entry too far. Fix this
> before auditing, or proceed knowing resolutions may be wrong.

#### NOT an AI feature

No model, no embedder, no network, and **not gated on AI being installed**. It is
arithmetic over text the parser already produced, so it must work for a user who
has never downloaded a model.

#### The seven, and what each names as evidence

Every finding carries the marker, the entry or the numbers. "Possible
inconsistency detected" is not a finding.

| check | severity | evidence it names |
|---|---|---|
| reference entry that is not a reference | **structural** | the ordinal, the entry text, what it implies for markers above it |
| orphan marker | **structural** | the marker, and the range the list actually covers |
| marker resolving to a malformed entry | **structural** | the marker, the ordinal, the entry text |
| duplicate table/figure number | cosmetic | the label and both captions |
| figure cited but never captioned | cosmetic | the reference, and the labels that do exist |
| out-of-sequence section letters | cosmetic | the run, e.g. `A, A, B, C` |
| repeated paragraph | cosmetic | both locations and the opening words |
| mixed citation styles | cosmetic | the section, its style, and the document's |

**Structural findings are acknowledged, not merely shown** — they are the ones
that make the audit's output untrustworthy. Cosmetic ones are listed and gate
nothing: a duplicate figure number is worth fixing and does not invalidate a
single verdict.

#### Deferred deliberately

- **author-year works absent from the reference list** — needs surname+year
  matching against a numeric-only list; noisier, and worth seeing the seven on
  real papers first.
- **same metric, two values** (MCC 0.945 vs 0.545) — detectable, deferred with
  the above.
- **text vs table** (joy 97.0 claimed, 94.92 tabulated) — **out of reach**: it
  needs table structure we do not keep. `parse_docx_blocks` flattens `<w:tbl>`
  into paragraphs and PDF extraction gives no cell topology, so ~5 days of table
  recovery is a prerequisite before the check is even meaningful.

#### KNOWN, and deliberately not fixed yet

**`mixed-citation-style` false-positives on a one-marker section.** On
`R PAPER .pdf` it flagged a section headed `TABLE III.` carrying a single
author-year marker. The fix is a minimum-marker threshold, and picking that
number from ONE instance is taste rather than evidence — the same mistake §11
D70 records (a fold table built from examples its author wrote, broken by the
first real paper). Left until three real manuscripts have been through it.

**Two checks are unvalidated on real input.** `section-letters-out-of-sequence`
and `repeated-paragraph` need paragraph granularity, and `pdf-extract` returns
25 blocks for a 6-page PDF with no newlines — so on a PDF they see almost
nothing. They are unit-tested and correct; they have never met a real `.docx`,
which is one block per `<w:p>` and is where they should work.

#### Run against a real `.docx`, and what it showed

`Revised Health Economics Paper FINAL (1).docx` — 313 blocks, 266 planned
sentences, 50 citation markers. **The granularity is confirmed**: a `.docx`
gives one block per paragraph where the PDF gave 25 for six pages.

It also found a false positive and killed it. The paper was reported as
numbering **Table 2 and Table 3 twice**, because it DISCUSSES each of its
tables:

```
"Table 2 presents mediation pathway coefficients. In Path A, …"   <- prose
"Table 2. Mediation Pathway Coefficients and Primary Bootstrapped …"  <- caption
```

`caption_re` matched both. A caption is `Table 2.` or `Table 2:` — the number
followed by punctuation — while a cross-reference is `Table 2 presents`. The
regex now requires the terminator. **A check that fires on a paper for
describing its own tables is worse than no check**, and this one fired twice on
the first real manuscript it met.

#### AND THE GATING CHECKS CANNOT RUN ON AN APA PAPER

This paper cites author-year — `(Alkenbrack et al., 2015)` — with a reference
list to match, so `parse_numbered_bibliography` correctly parses **zero**
entries. Every structural check keys on a NUMBERED list, so with 50 markers in
the document, **not one of them can be checked against anything.**

The result is a clean report, and a clean report on an unexaminable paper reads
exactly like a clean report on a sound one. The CLI now says so rather than
leaving it to be inferred:

> `NOTE: no NUMBERED reference list was parsed, so the structural checks
> (off-by-one entries, orphan markers) cannot run on this paper.`

**This is what the deferred author-year check is for**, and this paper is the
argument for taking it: half the field cites this way, and for all of them the
pre-flight currently has nothing to gate on.

Section letters and repeated paragraphs stayed silent here too — the paper
numbers its sections (`1. Background`, `2. Research Objectives`) rather than
lettering them, and repeats no paragraph. So both remain **unexercised**, not
validated.

### D95 — the page number is derived, and the stored one is demoted to a hint

§11 D92 measured it: **6 of 74 locatable sentences (8%) carry the wrong page**,
every one off by one. The audit's whole value is "go and look at this sentence",
and about one in twelve sent the reader to the wrong page.

#### The cause is reflow, and the true page was never lost

`parse_pdf_paged` tags every LINE with its correct page and then calls
`reflow_pdf_lines`, which merges lines into paragraphs. **A merged block keeps
one page number.** A paragraph that starts on page 3 and continues onto page 4
is one block tagged `3`, and every sentence in it inherits `3`.

So the attribution is lost at reflow, not at extraction — `extract_text_by_pages`
still has the per-page text, which is exactly what is needed to put a sentence
back on its own page. No coordinates, no pdf.js: **which page's text contains
this sentence** is a question Rust can answer from data we already parse.

#### Derived first, stored as a hint, and neither means no page

The same rule the annotated view follows, for the same reason:

| what is known | printed |
|---|---|
| the sentence appears on exactly one page (or is the k-th of k copies, in order) | **`p.4`** — derived, exact |
| not locatable, but the block carried a page | **`≈p.3`** — the stored hint, labelled |
| neither | **no page** — nothing, rather than something wrong |

A wrong page is worse than a missing one: it costs the reader a search and
teaches them the locators cannot be trusted. `≈` is not decoration — it is the
difference between "here" and "somewhere near here", and the reader is entitled
to know which they were given.

Duplicates are resolved the way §11 D92 resolves them: both lists are in
document order, so the k-th planned copy takes the k-th printed occurrence.
Exact, not a guess.

### D96 — the author-year path, designed against a real APA paper first

§11 D94 recorded the gap: `Revised Health Economics Paper` carries **50 citation
markers and zero checkable ones**, because every structural check keys on a
NUMBERED reference list. A large share of the field cites author-year, and for
all of them the pre-flight offered silence.

#### What the real markers look like, and what that forces

Dumped from that paper before a line was written. Four shapes decide the design:

```
(Alkenbrack et al., 2015; AlJohani & Bugis, 2024)   TWO works, one marker
Authority (2025)                                     a false surname
(Dubai 2013, Abu Dhabi 2006)                         lead "dubai", year 2006
Kutzins (2013)     vs entry   Kutzin, J. (2013)      near-miss surname
```

- **Multi-work markers** carry only the FIRST work through `markers_in`, so the
  second is invisible. Split on `;` and recover it, or every co-cited work reads
  as never cited.
- **`Authority (2025)`** is the tail of an organisation's name. A surname the
  list has never heard of is not automatically an orphan.
- **`(Dubai 2013, Abu Dhabi 2006)`** pairs one work's author with another's
  year. Any year comparison on it is meaningless, so a year check runs ONLY
  when the marker contains exactly one year.
- **`Kutzins` vs `Kutzin`** is a possessive or a typo, not a missing reference.

#### So uncertainty is a verdict, not a silence

> A false "this citation doesn't exist" is worse than a missed one.

Exact surname match → cited. **Near match** (edit distance 1, or a prefix within
two characters) → treated as cited AND reported separately as an uncertain
match. **No match of either kind** → orphan. The middle case is the whole
difference between this being usable and being noise.

#### Severity, by the D94 principle rather than by category

The question is the same one the numbered path asks: *does this make the audit's
output untrustworthy?*

| check | severity | why |
|---|---|---|
| marker with no entry at all | **structural** | the citation resolves to nothing |
| entry that is not author+year | **structural** | markers cannot resolve against it |
| year disagrees, one year in the marker | **structural** | it may resolve to the wrong work |
| entry never cited | cosmetic | the paper lists a work it does not use; no verdict moves |
| uncertain surname match | cosmetic | it was matched; the reader is told it was close, not exact |

#### And it says when it cannot run

The numbered checks stay silent on an APA paper and the author-year checks stay
silent on a numbered one. Each says which, because a clean report on an
unexaminable paper reads exactly like a clean report on a sound one:

> `reference style: AUTHOR-YEAR — the numbered checks (off-by-one entries,`
> `orphan [n] markers) do not apply to this paper.`

#### Measured on the paper it was designed against

`Revised Health Economics Paper` went from **50 markers and nothing checkable**
to 1 structural finding and 12 cosmetic ones. The first pass produced **8
structural**, and six of them were false:

| what fired | why it was wrong | fix |
|---|---|---|
| `P4H Network. (2024).` unreadable | organisational authors have no comma, and a digit sits inside the name | accept both APA shapes |
| `The Financial Services Authority. (2025).` unreadable | same | same |
| `Authority (2025)` orphan | the tail of an organisation's name | appears-in-an-entry -> uncertain |
| `Valletta (2011)` orphan | a third author cited alone | same |
| `Saksena and Kutzin (2019)` orphan | same | same |
| `(RBV; Barney, 1991)` orphan | an abbreviation being introduced | short all-caps is not a surname |

**Six false "this citation doesn't exist" in the first run of a check whose
whole justification is not producing them.** That is the measurement the brief
asked for, and the reason the conservative rules are rules rather than
preferences.

What survives is worth reading: `Kutzins (2013)` matched to `Kutzin, J. (2013)`
as uncertain, three co-authors cited alone flagged as uncertain rather than
missing, three listed-but-never-cited entries, and one genuine orphan.

#### STILL UNEXERCISED

`section-letters-out-of-sequence` and `repeated-paragraph` have now met two real
manuscripts and fired on neither, because neither has the defect: `R PAPER .pdf`
lacks the granularity and the health-economics paper numbers its sections and
repeats no paragraph. They remain **unexercised, not validated** — the `.docx`
that would settle them is not currently on disk.

### D97 — same metric, same subject, two values

The target: `R PAPER` reports **MCC 0.945 in the abstract and 0.545 in the
conclusion**, for the same dataset. A metric-name-plus-number scan finds it —
and finds a great deal else, so the rule was written against the real numbers in
BOTH papers before any code.

#### What a naive scan actually meets

Dumped from both manuscripts first:

| shape | why it is not a contradiction |
|---|---|
| `internationally owned **or** subsidiary 37` | the English conjunction, not an odds ratio |
| `ROC AUC with **95%** CI` | a confidence LEVEL, not the metric |
| `p < 0.001` | a p-value |
| `OR / Estimate (95% CI)` | a column header |
| `96.42% accuracy` | the number comes BEFORE the metric |
| Table II: accuracy for nine methods | same metric, nine subjects |
| `OR = 3.90` / `33.60` / `2.58` | three different mediation paths |

So: `OR` and `MCC` are matched case-SENSITIVELY (`or` is a conjunction), a
confidence level is stripped before the value is read, both number orders are
accepted, and `95` and `95.00` are compared as numbers rather than as strings.

#### The prime is load-bearing, and a regex ate it

`Path C` is the TOTAL effect and `Path C′` the DIRECT effect — different
quantities by construction, and the health-economics paper reports different
odds ratios for them, correctly.

The first prototype reported that as a contradiction. The cause was a trailing
`\b` in the subject pattern: `C′` has no word boundary after the prime, so the
regex backtracked and matched `Path C`, collapsing the two subjects into one.
**A manufactured contradiction in a real paper that had none**, from one
character of regex.

#### The discriminator

- same metric, **same confidently-extracted subject**, different values ->
  **contradiction, structural**;
- same metric, different subjects -> nothing;
- same metric, differing values, **subject not establishable** -> ONE uncertain
  finding for that metric, cosmetic, listing the values.

One per metric, not one per pair: three values produce three pairs and a reader
needs to know one thing, not three.

#### Measured on both papers

| | contradictions | uncertain |
|---|---|---|
| `R PAPER` | **MCC 0.945 vs 0.545, subject SemEval-2018** | F1 (46 vs 95 — related work vs this paper) |
| health economics | **none** | OR (2.58 / 3.90 / 33.60 — three paths, unlabelled in the table rows) |

The health-economics paper has no contradiction and is reported as having none:
its AUC is 0.88 in all five places it appears, and its three odds ratios belong
to three different paths.

#### THREE DEFECTS, EACH FOUND ONLY BY RUNNING IT

The rule was wrong three times, and every time on real input rather than in a
fixture:

1. **The prime**, above — a manufactured contradiction in a clean paper.
2. **The decimal.** The first Rust port split sentences on `'.'`, which cuts
   `MCC of 0.945` into `MCC of 0` and `945` — reading the value as zero and
   losing the subject to the next fragment. **It therefore missed the exact
   case it was written for**, while the Python prototype had caught it. Fixed by
   using the repo's own `extract::sentence::sentences_in` instead of a naive
   split.
3. **The comparison.** *"GoEmotions … reached only 46% macro-F1 … substantially
   lower than the 95% macro-F1 of the present study"* names both figures on
   purpose in ONE sentence, and the subject scan attributed both to GoEmotions.
   A contradiction needs two separate statements; a single sentence reporting
   two values is a comparison.

That is now four checks in a row — the caption regex, the author-year path, and
this — whose first run against a real manuscript was wrong. **The step that
catches them is running them on real papers before trusting them**, and it has
never once failed to find something.

### D98 — four checks in a row were wrong on their first real document

Not a defect. A pattern, recorded because it has now repeated four times without
a single exception, and because two of the four failed in the way that is
hardest to notice.

| check | first run against a real paper | what it cost |
|---|---|---|
| §11 D94 caption duplicates | fired twice on a paper for **describing its own tables** — `"Table 2 presents…"` read as a second caption | a false finding on a clean paper |
| §11 D96 author-year | **six** false orphans of eight structural findings — organisational authors, co-authors cited alone, an abbreviation read as a surname | six false *"this citation doesn't exist"* |
| §11 D97 metric agreement (a) | a trailing `\b` ate the prime in `Path C′`, collapsing the total and direct effects of a mediation model | a contradiction **manufactured in a paper that had none** |
| §11 D97 metric agreement (b) | sentences split on `'.'`, cutting `MCC of 0.945` into `MCC of 0` and `945` | **missed the exact defect it was written for** |

#### The two that matter most are the two that were silent

D97(b) and, earlier, §11 D92's anchoring at 82.9%, both **failed by finding
nothing**. A check that produces a false finding announces itself; a check that
misses its own target reports a clean paper and is indistinguishable from
success. Of the four, the two silent failures were caught only because the
number was compared against a prototype that had already worked.

#### THE RULE

**Fixtures verify the shape. Real papers verify the rule.**

A validation check is not to be trusted until it has been run against a real
document and its output read line by line. Every one of these had passing unit
tests when it was wrong — the tests were correct about the shape of the answer
and silent about whether the rule was right.

This is D69 and D70 again, one layer up: a token budget sized on fixtures, a
fold table built from self-written examples, and now four checks tuned on
inputs their author invented. The failure mode does not care which layer it is
at.

#### And it has never come back empty

Four runs, four defects, zero occasions where the real-paper step found nothing.
That is the argument for making it a step rather than a courtesy: it is not
insurance against an unlikely event.

### D99 — `label-cs`: the cold set `citation_support` never had

`citation_support` leads the report — §11 D78 calls it "the half worth trusting"
— and its entire eval is **6 synthetic seeds written in-house**, scoring 20-25%
verdict agreement. `citation_need`, the half demoted to advisory, has 42 cold
labels from real papers. That is the wrong way round, and §11 D98 is the reason
it matters: a rule validated only on inputs its author invented has been wrong
four times out of four.

#### The population was measured BEFORE the tool was written

| source | what it offers |
|---|---|
| audit jobs already in the DB | **4 distinct claims**, one of them a Gaply-report artefact from the D80 mis-run |
| `R PAPER` sentences mentioning a library source | **22** — SemEval 16, GoEmotions 5, ISEAR 4 |
| health-economics paper | **0** — its sources are not in the library |

So a tool driven by RESOLVED MARKERS would offer three cases and stop. Candidates
come from source MENTIONS instead, which is the difference between a set that
reaches useful size and one that does not.

**And 22 is a ceiling, not a yield.** Many of those sentences report the paper's
own numbers *on* SemEval rather than claims *about* it; those are
`insufficient_evidence` at best and are skipped. The honest expectation is a set
in the teens from this manuscript, and what it can support is stated at whatever
size it reaches rather than assumed.

#### Cold only, and enforced

`--suggest` is refused until 20 cold labels exist. §11 D75 records why the three
provenances are not interchangeable: a suggested-accepted label carries the
model's own answer and cannot score it, and a confident proposal moves the
labeller's judgement rather than merely their keystroke.

#### Self-contained, so the set outlives this machine

Each case is written with a FIXTURE holding the exact passages retrieved for it.
`ai-eval` indexes that fixture and retrieves from it, so the model sees the
evidence the labeller saw — the set measures JUDGEMENT over fixed evidence
rather than retrieval, which is what "measuring judgement, not coverage" means.
It also means the set is replayable on a machine that has never held the user's
library.

### D100 — GAP, NOT BUILT: the OA fetch needs a DOI, and a whole citation style has none

`gaply-core/src/oa_fetch.rs` refuses before it looks anything up:

```rust
if reference.doi.as_deref().map(str::trim).unwrap_or("").is_empty() {
    return OaResolution::NoOaCopy {
        detail: "no DOI on this citation — nothing to look up".to_string(),
    };
}
```

**The title is already there and is never used.** `oa_fetch::fetch_one` builds a
full `Reference { raw, authors, year, title, doi }` and passes it to `resolve`,
which reads only the DOI.

#### The evidence, measured

`R PAPER .pdf`, counted through the pre-pass's own bibliography parser:

| | |
|---|---|
| numbered reference entries | **25** |
| entries carrying a DOI | **0** |

That is not a defect in this paper. **IEEE-style reference lists routinely omit
DOIs**, and they are the house style across large parts of engineering and
computer science — so for that whole class of manuscript, the open-access batch
fetch reports "no DOI on this citation" for every entry and can never link a
single source. The citation the researcher most wants checked is the one the
fetch cannot even attempt.

It compounds: §11 D88's confirmation card offers "Fetch open-access copies" on
the blocked sources, and on an IEEE paper that button is guaranteed to return
nothing. The affordance is real and the outcome is empty.

#### What would close it

A title+author lookup against OpenAlex or Crossref when the DOI is absent —
both accept a bibliographic query and return a DOI, which is then the existing
path. The connectors already exist in `refverify`.

**Not built.** It needs its own decision about confidence: a title lookup can
return the wrong work, and linking a citation to the wrong paper is the failure
§11 D94's whole design fights — so it would need the same treatment as D96's
uncertain matches, where a near-match is reported as near rather than asserted.
That is a different piece of work from "use the field we already carry".

#### And it changes what a labelled set costs

§11 D99 measured `citation_support`'s labelling ceiling at ~20 candidates from
the two library sources. Extending it means indexing more of R PAPER's cited
works — GloVe (+8 candidates), Crow Search (+6), SMOTE (+5), ISEAR (+4) — and
**none of them can be reached by the OA fetch**, because none has a DOI in the
list. Each has to be added by hand, DOI first. The gap is not only a missing
convenience; it is the reason the eval set is expensive to grow.

### D101 — the fetch button that could not work, and now says so

§11 D100's compounding half, fixed. The title-lookup work it describes is
untouched; this is the honesty half.

D88's confirmation card offers **"Fetch open-access copies"** on the blocked
sources. `oa_fetch` refuses without a DOI, and an IEEE-style reference list
carries none — R PAPER has **25 entries and 0 DOIs** — so on that whole class of
manuscript the button is guaranteed to look up nothing, for every source, every
time.

A feature that cannot apply must say so. That rule already exists in the CLI —
*"no NUMBERED reference list was parsed, so the structural checks cannot run on
this paper"* (§11 D96) — and the card was the surface that had not learned it.

#### Two different facts, and only one drives the button

- **The MANUSCRIPT's reference list** carries DOIs, or does not. That explains
  *why* nothing can be fetched and is worth telling the reader.
- **The LIBRARY entry** for a blocked source carries a DOI, or does not. That is
  what `oa_fetch` actually reads, so it is what decides whether the button can
  do anything.

Both are reported: `ThesisAuditPreview` gains the manuscript's entry/DOI counts,
and `CitedSourceStatus` gains `has_doi` for the library entry. The button is
offered only when at least one blocked source has a DOI to look up.

#### What it says instead

> None of the blocked sources records a DOI, and the open-access fetch looks up
> a DOI — so it cannot resolve any of them. This manuscript's reference list has
> 25 entries and none records a DOI, which is normal for IEEE-style lists. Add
> the DOI in the Citation Manager, or attach the PDF there.

The second sentence is conditional on the manuscript's own counts, so it appears
only when they say something: a mixed list keeps the first sentence and drops the
generalisation.

**"Attach a PDF in the Citation Manager" stays.** It is the one action that still
works when there is no DOI, so the gate is on the fetch button alone, not on the
actions row. The failure to avoid was replacing a useless affordance with none.

Both directions are tested: a list where some blocked source has a DOI still
offers the fetch — and sends only the DOI-bearing ids, not all of them — while a
list with none never offers it.

### D102 — `retractionOutcome` persists, and "clear" now carries its date

The staged gap `citationTypes.ts` has been describing since Scope B, closed as
the file's own note specified: *"add a `retraction_outcome` column beside the
existing Scope B ones in gaply_core::citation_library, thread it through
citation_lib_upsert / StoredReference / storedToCitation, and the axis is
complete."*

Before this, `retracted` was durable but the OUTCOME was not, so an entry the
sweep had checked and found clean reverted to **"not checked for retraction"**
on restart. That direction was safe — under-claiming never lets a retracted
paper read clean — but it made the sweep unrepeatable in practice: every restart
re-presented a fully-checked library as fully unchecked, and the sidebar's
"N of M have not been checked" prompt fired again over work already done.

#### The second column, and why it is not scope creep

A persisted `'clear'` is a claim with an expiry nobody can see. Retractions
happen *after* a check; an entry cleared eighteen months ago and one cleared
this morning would render identically as "no retraction found". Axis C exists
precisely to stop a stale or absent fact reading as a current one, so the
outcome persists **with the time it was established**:

- `retraction_outcome` — `'clear'` / `'check_failed'`, NULL = never attempted.
  A CHECK constraint holds the vocabulary at the schema, so a typo cannot
  become a fourth silent state.
- `retraction_checked_at` — epoch seconds, stamped by the sweep.

The detail row reads **"no retraction found · checked 5 Sep 2026"**. No expiry
policy is implemented and none is implied: the date is shown so a researcher can
judge staleness themselves, which is the smallest honest thing to do. Deciding
when "clear" goes stale needs a real answer about registry lag, and inventing a
threshold here would repeat the mistake §11 D98 recorded.

`retracted` keeps its priority in `retractionState`: a confirmed retraction
outranks any outcome, so a row that is both retracted and 'clear' — which the
sweep cannot produce, but a partial write could — still reads RETRACTED.

Migration 19 is additive and defaulted, the same shape as 11 and 18: both
columns are nullable, no backfill, no rewrite, and existing rows read back
`NULL` = 'unchecked', which is exactly what they honestly are.

### D103 — the fourth serde drift, and the first to walk past D53's guard

`rename_all` on an enum renames VARIANTS, not fields. §11 D53 recorded exactly
this and built a test for it. `FetchReport` was written anyway, and shipped
broken.

Counted honestly, this is the **fourth**: D53 called itself the third (after
`kind`/`state` in D46's neighbourhood), and this is one more. What makes it
worth its own entry is not the repeat — it is that the guard D53 built to end
the class did not see this type at all.

#### The exact bytes, printed rather than assumed

```
{"citationId":"cite-x","title":"SMOTE","outcome":"fetched",
 "document_id":13,"chunks_indexed":113,"chunks_embedded":113,"checkable":true}
```

`FetchOutcome` carries `#[serde(tag = "outcome", rename_all = "camelCase")]` and
no `rename_all_fields`. `OaFetchEvent`, its streamed sibling in the same command,
has both. One type in the pair was fixed by D53's lesson and the other was not.

Four consequences, none of which threw:

* `documentId` → `undefined`, so `DocumentRow`'s `report.documentId != null` is
  false and the Document card **never leaves "No document linked"** after a
  successful fetch. The paper is on disk, indexed, embedded and linked; the
  screen says nothing happened.
* `chunksIndexed` → the success sentence reads *"indexed — 0 passages"*.
* `injectionFlagged` → permanently false, so the warning that a fetched abstract
  contained text aimed at the model **can never render**. That one is a safety
  sentence, and it was silently unreachable.
* `retryAfterSecs` → every rate-limit message says "about 60s" regardless.

Single-word fields (`checkable`, `title`, `detail`) crossed intact, which is why
the damage looked partial rather than total — the same shape as D53.

#### Why the guard missed it, which is the actual finding

`ai::event_wire_tests` asserts *"no field of any streamed event may reach the
webview containing an underscore"*, and D53 described that as a general rule so a
new field would fail even if nobody added a case. It is general over FIELDS. It
is not general over TYPES: the rule iterates a hand-written list of eight
constructed values from three enums, and a type absent from that list is not
checked by anything.

Its title scopes it further — *streamed events*. `FetchReport` is a command's
RETURN value. It crosses the identical boundary into the identical kind of
hand-written TypeScript reader, and by the file's own framing it was never in
scope. **The boundary is not "events". It is everything a `#[tauri::command]`
returns or streams.**

#### The fix is the contract test, not the attribute

Adding `rename_all_fields` to `FetchOutcome` repairs this instance and leaves
the class exactly as open as D53 left it — which is how we got here. What has to
change is the guard:

- enumerate the wire types from the command surface rather than from a list
  someone remembers to extend, so a NEW command is covered by default;
- cover return types, not only streamed events;
- keep the underscore rule as the cheap general net, and pin the exact JSON for
  the types whose fields the UI actually branches on.

#### And why neither test suite caught it

`oaFetch.vitest.tsx` builds its report fixtures by hand in camelCase, so it
verifies the TypeScript layer against a shape Rust has never produced — the
suite and the bug agree with each other. Every field in the `OaFetchReport`
interface is optional (`documentId?`), so `undefined` is legal and `tsc` had
nothing to say.

That is §11 D98's finding again, at a different boundary: a check validated only
against fixtures written by the same hand that wrote the code has now been wrong
five times out of five. Fixtures verify the shape; only the real producer
verifies the contract.

#### What was built, and what it found

`src-tauri/src/wire_contract_tests.rs` — three tests, parsing this crate's own
source with `syn` (already in the lock file transitively; a guard that exists
because a guard failed should not itself rest on regex).

**The rule is NOT "no underscore may cross".** That was tried first and is wrong
for this codebase: applied to the types reachable from the command surface it
flags **248 fields across 174 types**, because much of the app is snake_case on
BOTH sides and the two agree perfectly — `ReferenceVerification` and its
`UntrustedText { safe_text }` among them, which the frontend reads in snake_case
today and correctly. A guard reporting 248 non-bugs is a guard someone switches
off, and it would have "found" the real one by accident.

The defect is never that an underscore crossed. It is **the Rust type and the
TypeScript reader disagreeing**, and the zero-false-positive form of that is
internal coherence: *if a type DECLARES `rename_all = "camelCase"`, its fields
must actually come out camelCase.* A type asking for camelCase while emitting
`document_id` is incoherent whether or not anyone reads that field — so this
needs no exemption list, and an exemption list is the thing that just failed.

It found **six** offending enums, five of which nobody was looking for:

| type | fields | live? |
|---|---|---|
| `FetchOutcome` | `document_id`, `chunks_indexed`, `chunks_embedded`, `injection_flagged`, `retry_after_secs` | **yes — D103** |
| `EngineState` | `model_id`, `preprocessing_version` | latent |
| `GenState` | `idle_ms` | latent |
| `OaResolution` | `pdf_url`, `safe_text`, `injection_flagged`, `retry_after_secs` | not on the wire |
| `TaskError` | `first_raw`, `retry_raw`, `stop_reasons` | not on the wire |

`EngineState` and `GenState` ARE on the wire — nested in `AiModelStatus`, a
command return type — and were saved only by `EngineStateWire` being typed as
`{ state: string; [k: string]: unknown }`, so the TypeScript reads the tag and
nothing else. Latent, not harmless: one reader of `.modelId` away from D103
again. None of the six derives `Deserialize`, so no stored JSON is read back
into them and the spelling change breaks nothing.

The second test enumerates every type a `#[tauri::command]` returns or streams
from the signatures themselves and asserts each RESOLVES to a definition the
guard can see — failing closed, because failing open is how `FetchReport` went
unchecked. It immediately caught one it could not see, `StatsVerificationReport`,
which is a `use ... as ...` alias; the guard now resolves aliases rather than
allowlisting the name, since allowlisting a name it cannot see is exactly how a
guard comes to be trusted while checking nothing.

The third pins `FetchReport`'s exact JSON, because no general rule catches a
renamed tag or a dropped field. Removing `rename_all_fields` from `FetchOutcome`
fails the general rule and the pin independently — verified by reintroducing it.

`ai::event_wire_tests` keeps its exact-JSON pins and now carries a note saying it
is not the whole guard.

### D104 — Unpaywall does not close the D100 gap; measured before building

Recorded as evidence, not as a decision. §11 D100 assumed a second resolver
would reach the works OpenAlex cannot. Measured against `support@gaply.in`
before any plumbing was built, that assumption is false for the case that
motivated it.

| DOI | OpenAlex | Unpaywall |
|---|---|---|
| `10.3115/v1/D14-1162` (GloVe) | closed, no PDF | **closed, 0 OA locations** |
| `10.3115/1118783.1118785` | gold, dl.acm.org | gold, dl.acm.org |
| `10.3115/991250.991336` | gold, dl.acm.org | gold, dl.acm.org |
| `10.3115/1626269.1626275` | gold, dl.acm.org | gold, dl.acm.org |
| `10.18653/v1/2020.acl-main.372` | gold, no pdf url | gold, no pdf url |
| `10.18653/v1/S18-1001` | gold, aclweb.org | gold, aclweb.org |

**Identical in 6 of 6.** Two corrections to the premise:

* The `10.3115` prefix is **not** the invisible slice. Three older `10.3115`
  papers are gold OA in both indexes. It is the `10.3115/v1/…` sub-form — the
  2014–2016 ACL Anthology DOIs — that both indexes record as closed, even though
  `https://aclanthology.org/D14-1162.pdf` returns 200.
* Adding Unpaywall would not have fetched GloVe. The resolver is not
  half-blind here; both indexes are blind in the same place.

So GloVe's abstract-only result is CORRECT given the data both aggregators hold,
and wrong about the world. Reaching it needs a publisher-specific route (an ACL
Anthology id is derivable from the DOI suffix), which is a different decision
with a different risk — a URL heuristic that silently fetches the wrong paper is
the failure §11 D94 exists to prevent — and is NOT this entry.

What survives independently: `refverify.rs:39` still reads *"mailto set at
deploy"*, so the **Crossref polite pool is unset** on the path that actually gets
hammered (retraction sweeps, add-by-DOI). That case never depended on Unpaywall.
Held for a decision rather than built.

### D105 — the Crossref mailto that was never set, and the ninety silent seconds

Two things, both of which had been sitting in a comment or a symptom rather than
in the code.

#### The polite pool

`refverify.rs:39` read `"gaply/1.0 (reference verification; mailto set at
deploy)"`. It never was. Every Crossref request — add-by-DOI, the verify pass,
and the retraction sweep — went to the ANONYMOUS pool, whose rate limits are
lower and, more to the point, unpredictable: the pool that gets shed first under
load is the one carrying a sweep across a whole library.

The address is a **constant**, `CONTACT_EMAIL = "support@gaply.in"`, with
`GAPLY_CONTACT_EMAIL` overriding it for development and CI. It is not a
credential: it travels in plaintext on every request and is a readable string in
the shipped binary either way, so hiding it buys nothing — while a constant
cannot silently go missing the way an unset build-time variable can, which would
put us back in the anonymous pool with nothing failing to say so.

An EMPTY override yields `gaply/1.0 (reference verification)` rather than a
dangling `mailto:`. A malformed mailto reads to Crossref as abuse of the polite
pool rather than politeness, which is worse than staying anonymous.

**Unpaywall stays unset** (§11 D104 measured that it adds no coverage OpenAlex
lacks), so `email=` is still absent and that connector is still skipped. The two
are separate switches and only one is thrown here.

#### Which copy had to change, and which did not

`CROSSREF_UA` rides on exactly three requests, all to crossref.org. The
open-access fetch uses the generic `ReqwestFetcher` UA, so:

- **`DocumentRow`'s note is unchanged.** *"Sends this source's DOI to Unpaywall
  and OpenAlex — nothing else leaves your machine"* stays true of that path, and
  its test still passes. It was tempting to update it for consistency; it would
  have been a false disclosure of something that surface does not do.
- **`AiCheckPage`'s opt-in did change.** It promised *"Sends **only** citation
  details…"*, and `only` is load-bearing. It now reads *"Sends citation details
  (author, year, title, DOI) and Gaply's contact address to CrossRef/OpenAlex,
  never your manuscript."*
- **`CLOUD_SUITES`' one-liner did too**, for the same reason: it enumerates what
  the suite sends.

#### The ninety seconds

§11 D103 explained why the fetch reported nothing at the end. This is why it
reported nothing during: `citation_fetch_oa` emitted `Started` / `Fetching` /
`Done`, and everything expensive happens BETWEEN the last two. `DocumentRow`
subscribed to none of it, so a 37-page paper showed a button reading "Looking
for a free copy…" for a minute and a half — which is indistinguishable from a
stuck one, and was reported as the feature doing nothing.

`FetchPhase` now streams `resolving` → `downloading` → `indexing` →
`embedding { done, total }`. Only the last carries numbers, because it is the
only phase whose duration scales with the paper; a count on the others would be
decoration. It is emitted per embedding BATCH, so the line actually moves.

The button keeps a short label and the detail goes on its own line — the shape
the sibling `linking` flow already uses on the same card, so the two do not
present the same kind of progress two different ways.

### D106 — what a `citation_support` label can measure, and what it cannot

Six cold labels in (3 weak, 1 contradicts, 1 strong, 1 partial), and the act of
labelling produced two findings about the SET rather than about the model. Both
are recorded before the eval runs, because they bear on what any number off that
eval is allowed to mean.

#### 1. Non-claims in the candidate pool — and the filter IS already applied

The reported symptom is right: figure captions, cross-references and table
pointers were offered as candidates, and on those the retrieval returned
bibliography lines because there was nothing to match on.

The proposed cause is not. `label-cs` selects from `pre.planned`, which is the
prepass output AFTER `skip_reason` — the audit's significance filter is already
applied to candidate selection. Applying it again would change nothing, and
fixing this only in `label-cs` would have left the AUDIT queueing the same
sentences for real model work.

Measured on R PAPER, the filter catches a caption whose label LEADS
(`Table II shows…` → `TableOrFigure`) and misses one whose label TRAILS, which
is how PDF reflow usually leaves them:

```
skip_reason("Table II shows comparative performance of the proposed …") → TableOrFigure
skip_reason("Accuracy and F1-Score Comparison on SemEval-2018 Fig. 3.") → None
```

Of the 82 sentences the prepass plans for R PAPER, five end in a figure/table
label. Four are prose that legitimately cites a figure — *"The complete-flow
architecture is depicted in Fig. 1."* — and exactly one is a caption whose label
migrated to the end. The discriminator that separates them on the real data is
the token BEFORE the label: prose reaches a figure through a preposition (`in`,
`see`, `from`), a caption abuts it. So the rule is a trailing label not preceded
by a connective — which drops 1 of 5 here, the right one.

**Left unbuilt, deliberately: the pointer sentences.** Three of the four kept
above are cross-references — *"…are presented in Table III."* — and the finding
that they are "nothing a source could adjudicate" is correct. They are still
poor `citation_support` candidates. But *"As shown in Table II, the proposed
model outperforms the baselines"* is a real claim in the same syntactic shape,
and a rule that cannot tell those apart would drop evidence rather than noise.
That needs its own discriminator designed against more than one paper (§11 D98),
so it is recorded rather than guessed at.

#### 2. The claim TYPE sets the ceiling — the larger finding

All three SMOTE candidates were **method attribution** (*"our pipeline uses
SMOTE-Text class balancing"*) or the authors' **own ablation numbers**. No source
can support either: the source describes SMOTE, it does not witness that THIS
pipeline used it, and it certainly does not contain their ablation results.
`weak` was the only honest verdict on all three, and **retrieval worked perfectly
every time**. The number those cases produce is a property of the claim type, not
of the model.

The spread came entirely from **descriptive claims about a source**:

| claim | verdict | why |
|---|---|---|
| GoEmotions: "46% macro-F1 over 27 categories" | `strong` | the source states it |
| GoEmotions: "58,009 comments, 27 emotions, remapped into eight" | `partial` | the eight-category remapping is theirs, not the source's |
| SemEval: "10,983 tweets, eight Plutchik emotions" | `contradicts` | the source says 22,000+ and eleven |

So **`citation_support`'s measurable population is descriptive claims about a
cited source** — not the method-attribution citations that make up much of real
citing. A labelled set drawn from the wrong population measures nothing, and a
set drawn mostly from method attribution would report a model that agrees with
`weak` every time as accurate.

**And the verdict scale has no honest slot for it.** The five verdicts are
`strong` / `partial` / `weak` / `contradicts` / `insufficient_evidence`.
"Correctly credits a method they did use" is not weak — weak means *only
topically related*, and a correct attribution is a great deal more than topical.
It is not `strong` either, because the passages do not support the sentence as
written. The scale answers "does the evidence support this claim", and
attribution asks "is this the right work to point at", which is a different
question that needs its own verdict — or its own task — rather than being folded
into a verdict that misdescribes it.

Consequence for selection: candidates should be drawn preferentially from
descriptive claims, and the population a given eval was drawn from must be stated
alongside its accuracy, or the number is unreadable.

#### 3. The prompt asked the wrong question on a `contradicts` verdict

`label-cs` asked **"WHICH PASSAGES SUPPORT IT?"** after every verdict including
`contradicts`, where the passages being marked are the ones that REFUTE the
claim. The stored field is the same either way — the passages that settle it —
so the question now follows the verdict.

#### 4. The default eval invocation measures nothing, and says so

`ai-eval --task citation_support` with no other flags ran the **0.5B** bundled
model against a **mocked** embedder and produced a full report. The harness is
honest about it — the header prints `model : qwen2.5-0.5b-instruct-q4km` and
`*** MOCKED - not a valid bake-off (§11 D29) ***`, and the JSON carries
`embedderIsReal: false` — so this is a trap rather than a defect, and §11 D29
already built the stamp that catches it.

Recorded because the trap was walked into anyway. The numbers that run produced
(1/12 valid, 92% validation failure) describe the 0.5B against lexical mock
retrieval and say nothing about the product, which uses the 3B and bge-small.
Any `citation_support` figure quoted without the model id AND `embedderIsReal`
beside it is unreadable. A valid run needs:

```
--model qwen2.5-3b-instruct-q4km --model-dir <dir> --embedder-dir <dir>
```

#### 5. The eval, and exactly what 6 cold labels support

Run on the 3B with the real embedder (`citation_support-v1.6-2026-09-07-3b-real-embed.json`):

```
valid outputs           : 7/12        verdict agreement : 33%
validation failure rate : 42%         cited planted     : 50%
verdictDistribution     : { "weak": 6 }
```

**That last line is the result.** Every valid output was `weak` — six for six.

| case | gold | model |
|---|---|---|
| cs-seed-01 | strong | weak |
| cs-seed-02 | partial | weak |
| cs-seed-03 | weak | **weak** ✓ |
| cs-seed-04 | contradicts | weak |
| cs-seed-05 | insufficient_evidence | weak |
| cs-label-002 | weak | **weak** ✓ |
| cs-label-001/003/004/005/006 | weak, weak, contradicts, strong, partial | *validation failure* |

**The 33% is not a skill estimate — it is the prevalence of `weak` among the
scoreable cases.** A model that always answers `weak` scores exactly the same,
and on this run the model WAS that model. The eval currently cannot distinguish
`citation_support-v1.6` from a constant. Wilson 95% CI on 2/6 is **[10%, 70%]**,
which is another way of saying the same thing.

##### What the size does support

Not accuracy, and not the direction of any prompt change. It does support one
claim, because the split is clean and the failure modes are mechanical:

**Validation collapses on real manuscripts and holds on fixtures.** 5 of 5 cold
cases failed schema validation; 1 of 6 synthetic seeds did. Two reproducible
shapes, neither of them a judgement:

* `supporting_chunks` emitted as STRINGS rather than objects — the model echoes
  the evidence header back verbatim: `invalid type: string "CHUNK_ID=c14 PAGE=1
  SECTION=-", expected struct SupportingChunk`, and once with an entire 60-word
  passage inside the string;
* `missing field 'why'`, twice.

Both are the §11 D66/D81 shape — a schema the model trips over — and both are
fixable without a single further label. The fixtures never provoke them because
their evidence blocks are short and clean; the real ones carry the header format
the model then imitates.

##### What it would take to measure accuracy

The population must be **descriptive claims about a source** (§11 D106 §2), and
the model must clear validation often enough to have verdicts to score. On this
evidence the order is forced: **fix the two schema failures first**, then label —
labelling more method-attribution claims would add cases that are `weak` by
construction, scored against a model that answers `weak` regardless, and the
agreement figure would climb while nothing improved.

Also recorded: mean latency **189 s/case**, prefill 81 s, decode 6.2 tok/s, 2 of
12 truncated at the 1024 ceiling. A 12-case run took ~35 minutes.

### D107 — the citation that arrived in four shapes, and an eval that now refuses

§11 D106 §5 left the order forced: fix validation before labelling. The raw
outputs were read first, per D66's rule, and they indict the schema rather than
the model.

#### What the model actually sent

All five failures on the cold cases were ONE thing wearing three costumes. The
chunk ids were right every time (`chunkIdFormatFailures: 0`); only the wrapper
varied:

```jsonc
[{"chunk_id":"c1","page":8,"why":"…"}]        // as specified
[{"chunk_id":"c32"}]                           // cs-label-005 — no `why`
["c20","c22"]                                  // cs-label-003 — the id alone
["CHUNK_ID=c14 PAGE=1 SECTION=-"]              // cs-label-001 — the header echoed
```

Every one of these was a **serde failure**, so the analysis was discarded before
any validator ran. `cs-label-005` is the case that settles the argument: gold
`strong`, and the model's own decomposition had found *"46%"*, *"27 categories"*
and marked *"95%"* absent — the substantively correct answer — thrown away for
a missing per-chunk prose field.

#### The layer the evidence indicts

Three facts, none of them about the model's judgement:

* the consumer already tolerates it — `CitationAiPanel`, `ThesisAuditScreen` and
  `findingFromResult` all read `quote ?? why ?? ''`;
* `why`'s only existing rule is ADVISORY ("longer than 20 words"), so a 50-word
  `why` was accepted with a note while an ABSENT one destroyed the output — the
  schema was stricter about the field being missing than about it being wrong;
* the degradation preserves the meaning every time. The id is what
  `citedPlantedChunk` scores and what the card links to; `why` is commentary.

So the PARSE widens: a citation may arrive as a bare string or as an object
without `why`. **Widening the parse is not widening the rules.** The echoed
header still lands in `chunk_id` and is still FATAL there — §11 D26 ruled that
composite ids are fixed in what the model is SHOWN, never in what it may say,
and that stands. The difference is it now fails as a nameable chunk_id defect
instead of an unparseable blob. A missing `why` becomes the fourth ADVISORY,
reported so a run cannot quietly fill with citations that justify nothing.

#### The invalid-run trap, closed at the default

`ai-eval --task citation_support` with no other flags ran the **bundled 0.5B**
against a **lexical mock** embedder and printed a complete, plausible report
(1/12 valid, 92% failure). §11 D29's stamp was present and honest — the header
said `*** MOCKED ***` and the JSON said `embedderIsReal: false` — and it was
walked past anyway.

A report nobody should quote should not be produced by default. The task now
REFUSES without `--model`/`--model-dir` and `--embedder-dir`, before the model
loads so the refusal is free, naming what each omission would have silently
substituted. `--smoke` still runs it for a pipeline check, by name.

#### The re-run, and the finding it confirms

Same 12 cases, 3B, real embedder, after the parse fix:

| | before | after |
|---|---|---|
| valid outputs | 7/12 | **9/12** |
| validation failure rate | 42% | **25%** |
| chunk_id format failures | 0 | **2** |
| cited the planted chunk | 50% | 60% |
| verdict agreement | 33% | 38% |
| **verdictDistribution** | `{weak: 6}` | **`{weak: 8}`** |

`chunkIdFormatFailures` rising from 0 to 2 is the fix working, not regressing:
those two were previously unparseable blobs and are now counted as what they
are. §11 D26 holds — the echoed header is still fatal.

**Every valid verdict, in both runs, was `weak`. Fourteen for fourteen.**

| case | gold | model |
|---|---|---|
| cs-seed-01 | strong | weak |
| cs-seed-02 | partial | weak |
| cs-seed-03 | weak | weak ✓ |
| cs-seed-04 | contradicts | weak |
| cs-seed-05 | insufficient_evidence | weak |
| cs-label-002 | weak | weak ✓ |
| cs-label-003 | weak | weak ✓ |
| cs-label-005 | **strong** | weak |
| cs-label-001 / 004 / 006 | weak, contradicts, partial | *validation failed* |

**`cs-label-005` is the case that settles it.** Validation now clears, its
decomposition finds *"46%"*, *"27 categories"* and *"fine-tuned BERT"* present
and marks *"95%"* absent — the correct reading, which is the gold `strong`
case's whole content — and the verdict it attaches to that analysis is still
`weak`. The evidence is understood and the verdict does not follow from it.

The 38% agreement is 3 of 8, and all three are cases whose gold is `weak`. **A
constant `weak` predictor scores exactly 38% on this set.** Wilson 95% CI
[14%, 69%]. The output distribution has zero entropy across fourteen verdicts.

##### What this changes

This is §11 D67's `citation_need` 0/65 again, and the consequence is the same
kind: **it is not a labelling-volume problem, and more labels cannot fix it.**
`citation_support` currently leads the audit report while behaving like a
constant, so what the feature may honestly claim has to change before anything
else does. Adding cases would raise the agreement figure whenever the added
golds are `weak` — which, per §11 D106 §2, is exactly what method-attribution
candidates are — while the model got no better.

The three remaining failures are all model behaviour that D26 and D32 keep fatal
on purpose: two echoed headers, two over the 4-chunk cap (`cs-label-004` did
both). None is a schema question.

##### Unexplained, and recorded as such

A middle run — killed before it finished — had `cs-seed-01` fail with *"no JSON
object or array found"* at 298 s, where both clean runs return `weak` in ~35 s.
There is no wall-clock cap on generation, so truncation-by-timeout is NOT the
mechanism, and greedy decoding should be reproducible. It happened once, under
5–10× latency, in the run that later died. No conclusion rests on it and none
should until it recurs.

### D108 — `citation_support` is a constant, so the report stops claiming otherwise

§11 D107 measured it twice. Fourteen valid outputs across two runs, every one
`weak`. §11 D59 named the defect — the reason and the verdict only weakly
coupled — and this is that defect on the half that LED the report.

The claim changes, the way `citation_need`'s did in §11 D78: demoted to what it
demonstrably does, with the measurement stated beside it.

#### The case that distinguishes "cannot read" from "reads correctly, answers regardless"

`cs-label-005`, gold `strong`. After D107's parse fix it clears validation, and
the model's own decomposition reads:

```jsonc
"claim_elements": [
  {"element":"fine-tuned BERT",  "status":"found"},
  {"element":"average F1-score", "status":"found"},
  {"element":"over 27 categories","status":"found"},
  {"element":"46%",              "status":"found"},
  {"element":"95%",              "status":"absent"}
],
"explanation": "The evidence shows that GoEmotions fine-tuned BERT and achieved
   an average F1-score of 46% over 27 categories. However, the claim states a
   95% macro-F1 score, which is not supported by the evidence.",
"verdict": "weak"
```

Every element is marked correctly. The explanation is correct. **The verdict does
not follow from either.** Retrieval worked, comprehension worked, and the grade
was produced independently of both — which is why more labels cannot help and
why the decomposition is worth keeping while the verdict is not.

#### The arithmetic

3 of 8 agreement, and all three are cases whose gold is `weak`. **A constant
`weak` predictor scores exactly 38% on this set.** Wilson 95% CI [14%, 69%]. The
output distribution has zero entropy across fourteen verdicts.

#### What was removed, and it was more than a badge

The verdict was not only a label on each item. It was the report's entire
severity spine, through `attention_rank`:

| rendering | what it did with a constant `weak` |
|---|---|
| item badge | every item badged `weak` |
| **cover headline** | `attention_rank("weak") < 9`, so every claim "failed" → **`0 / 100 — 0 of N checked claims held up`, on every manuscript** |
| "At a glance" score | the same 0/100, badged red |
| proportional bars | 100% "checked, did not hold up" |
| attention list | every checked claim, in document order, each bullet printing `[weak]` |
| summary bars | `support: weak 8 (67%)` |

A fabricated failing grade on every paper is worse than no grade. §11 D85
settled the principle already — *a number that cannot respond to the thing it
names is worse than absent* — so `health_score`, `attention_list` and
`attention_rank` are **deleted**, not disabled, with a note in their place
saying not to rebuild either from the support verdict.

#### What replaced it

The passages, which are what measured correct. Each item now carries the source
quotes with their page, the model's reading of them (D18's rule is unchanged —
prose only where there is a quote to check it against), and the decomposition
rendered as *"46% macro-F1 — in the source"* / *"95% accuracy — NOT in the
passages read"*, explicitly marked as the model's reading rather than Gaply's
conclusion.

The section is renamed from *"Claims checked against their source"* — which told
the reader to read the passage "before the verdict" — to **"The source passages
behind each cited claim"**, and its blurb states the measurement: the grade was
identical on 14 of 14 outputs, so it carries no information and is not shown.
Ordering is document order, because sorting by a withdrawn signal would put it
back as a ranking.

`GAPLY_REPORT_MARKERS` keeps the OLD heading alongside the new one: reports
exported before this change are still Gaply reports and the self-audit guard
(§11 D80) must still recognise them.

#### What is NOT changed

The verdict is still **recorded** — in `result_json`, in `verdict_counts`, and
in the JSON export. Withdrawing it from the record would make the measurement
unrepeatable and would hide the defect rather than disclose it. It is the
REPORT that stops presenting it as a finding.

Two surfaces still render it and are NOT touched here: the on-screen
`CitationAiPanel` and the annotated manuscript's colouring (`annotationStatus`).
Both are the same defect and need the same treatment; they are recorded rather
than changed, because this entry is scoped to the report.

#### The two remaining surfaces, closed

D108 recorded that `CitationAiPanel` and the annotated manuscript still rendered
the withdrawn verdict. Both are now closed on the same terms.

**The annotated manuscript was the worse of the two.** `statusOf` mapped
`strong` to a GREEN highlight labelled *"verified with evidence"* and every
other verdict to a RED one labelled *"weak or contradicted"*. With a constant
`weak`, every `citation_support` sentence was drawn red, and the green legend
entry was unreachable — a legend naming a distinction the page could not draw,
which is worse than no legend.

The two collapse into one NEUTRAL status, `evidence` (blue, solid, full
weight): *"source passages found"*. Neither green nor red on purpose — it marks
where the evidence is and asserts nothing about whether the sentence is well
supported.

**And the highlight now depends on there being passages, not on the verdict.**
`statusOf` takes the passage count and returns `null` at zero, defaulting to
zero so a caller that forgets cannot mark a sentence it has no evidence for. A
mark reading "source passages found" over a sentence with none would be the same
lie in a new colour. A test asserts every status in the legend is reachable.

**`EvidenceCard`** badged *"✓ Supported"*, *"≈ Partially supported"*,
*"! Weak support"*, *"✕ Contradicted"* — a five-class grade, of which only
"Weak support" was ever actually shown. All four now read *"Source passages
found"* with a neutral tone and no tick or cross; the card carries the same
one-line statement the report does, and renders the decomposition beneath it.

Two distinctions are KEPT because neither is a grade:

* `no_evidence` — deterministic; the engine retrieved nothing and no model ran.
* `insufficient_evidence` — reads **"No supporting passage cited"**, not "found".
  It is the one verdict the validator lets cite nothing, so the card is showing
  what was EXAMINED rather than what was cited (§11 D52); labelling it "found"
  would be false in exactly the case it names.

The health score stays gone. If a score returns it returns on something
measured, deliberately, not as a restoration.

### D109 — the fifth wire drift, and the first hidden by a cast

The annotated manuscript could only ever draw ONE of its highlight colours, and
had shipped that way.

`ThesisAuditScreen` fills its `items` from `ai_job_results`, whose per-item wire
shape is `gaply_core::ai_engine::jobs::JobItem` — `#[serde(rename_all =
"camelCase")]` over `result_json: Option<String>`, so the field is **`resultJson`
and its value is a JSON string**. `AnnotatedManuscript` was written against the
PARSED shape that `health.flagged` carries (`FlaggedItem.result` is a real
`serde_json::Value`) and read `it.result?.output?.verdict`.

Always `undefined`. So:

| kind | `statusOf` | drawn |
|---|---|---|
| `unverifiable` | `blocked` — ignores the verdict | ✅ |
| `citation_support` | `null` | ❌ never |
| `citation_need` | `null` | ❌ never |

The view showed only grey "cited but not checkable" marks under a legend
advertising statuses the page could not produce, and its counter — *"N of M
judged sentences shown"* — computed `M` from the already-empty drawable set, so
it read as correct rather than broken.

#### What is new about this one

The four before it (§11 D46, D53, D103) were all a NAME drifting between Rust
and a hand-written TypeScript reader. This is the first where the mismatch was
**silenced at the call site by a cast**: `items={items as any}`. The types were
right on both sides and the compiler was told not to look.

`as any` is a wire-contract defeat, and it is invisible to every guard we have.

#### Why §11 D103's guard could not have caught it

Two independent reasons, and the second is the more important:

1. **The shape never enters the guard's world.** `ai_job_results` is declared
   `-> Result<serde_json::Value, GaplyError>`. The surface scan reads types off
   the signature, and `Value` is in `NOT_A_WIRE_TYPE`. **Fourteen commands
   return `serde_json::Value`** — including `ai_job_results`, `ai_job_status`,
   `ai_job_start_thesis_audit`, `ai_citation_support`, `ai_link_source_document`
   — so the app's busiest surface is entirely untyped at the boundary and
   entirely invisible to the guard.
2. **Even a fully-typed shape would not help**, because the defect is not that
   Rust emitted the wrong key. Rust emitted `resultJson` correctly. The reader
   asked for a different one, and the cast stopped anyone finding out. No
   Rust-side test can see that.

So the honest answer to "can the guard catch this shape too" is **no, not as
built** — and two things would move it:

- make the guard flag a command returning bare `serde_json::Value` as *shape not
  declared, therefore not checked*. That does not verify anything, but it turns
  fourteen silent holes into a visible list;
- a lint against `as any` on a component prop boundary. Cheaper and narrower:
  the cast is what defeated the type system here, and there is exactly one of
  them on this path.

Neither is built here.

#### The fix, and the test that had to change with it

`outputOf(item)` reads BOTH shapes — the parsed `result.output` and the
`resultJson` string — in one place, so a third call site cannot pick the wrong
field again. An unreadable `resultJson` yields `{}` and therefore no highlight,
never a mark built on a parse failure. The `as any` is gone, so the compiler now
checks the assignment.

**The test lesson is the point of the entry.** §11 D108 added *"every status in
the legend is reachable"*, which calls `statusOf` directly — it passed happily
while no support sentence could be highlighted at all. A guard over the VALUE
proves nothing about the PATH. The new tests render the component from a payload
built to the exact `ai_job_results` wire shape (camelCase keys, `resultJson` as a
string, `kind` snake_case) and assert a highlight appears, the counter is not
silently short, an empty passage list still draws nothing, and unreadable JSON
draws nothing. Reintroducing the bug fails three of them; the `statusOf` tests
stay green throughout, which is exactly how it shipped.

### D110 — a retracted source is not a model finding, and must not be filed as one

`grep -i retract` across `audit_report.rs`, `audit_export.rs` and
`thesis_audit.rs` returned **nothing**. A researcher could audit a manuscript
citing a retracted paper, read its passages quoted neutrally in the evidence
section, and never be told — while the Citation Manager held `retracted = true`
for that very entry, durably, since Scope B.

The two features had the fact and did not share it. This is the failure the
product exists to prevent.

#### Where it surfaces, and why there

**Before the run, on the confirmation card.** *"2 of your cited sources have
been RETRACTED"*, above the counts, with each work named and its citing-sentence
count. A retraction is registry-backed and settled; establishing it costs
nothing and does not need the model. Learning it after three hours of inference
inverts the cost of finding out, and the card is exactly where §11 D88 put the
other things worth knowing before spending them.

**At the TOP of the report**, above even the deterministic consistency checks.
Consistency findings are structural facts about the manuscript; a retraction is
a fact about the scholarly record, and it is the most serious thing this report
can carry.

**NOT among the model findings**, and this is the deliberate part. Filing it in
the evidence section would rank the one settled, registry-backed fact in the
report alongside — and below — output with an error rate. It is not a verdict,
it did not come from the 3B, and it does not belong in a section whose blurb
explains what a language model contributed.

#### NOT a gate, and that is also deliberate

Structural consistency findings block the run until acknowledged (§11 D94)
because they mean the audit's own resolutions cannot be trusted. Retraction is
different: **citing a retracted work is often correct.** A paper about research
integrity must cite the papers it discusses; a literature review may cite one
precisely to note its withdrawal. Blocking would be wrong. It reports loudly and
proceeds.

#### What it must never say

There is no "0 retracted" line. Absence of a confirmed retraction is not a clean
bill, because `retracted` is only true when a registry actually answered — an
entry nobody checked is UNCHECKED, and §11 D102 exists precisely to keep those
two apart. Both surfaces say so in as many words: *"Only works that were
actually checked appear here; an unchecked entry is not a clean one."*

#### The lookup was one call away

`libraryId` has been in the support item's `payload_json` since planning
(`thesis_audit.rs:525`), and `build_model` already holds `db`. Counting is over
DISTINCT works, like every other source figure on the card — five sentences
citing one retracted paper is one problem to fix, not five.

### D111 — finishing the demotion: two nav items that could never fill, one count that never varied

Two small things the §11 D108–D110 investigation turned up, both misleading as
displayed rather than wrong in code.

#### The support verdict was still on screen

`ThesisAuditScreen` rendered `health.verdictBreakdown` verbatim — *"8 support —
weak"* — while the report had stopped showing it. Same withdrawn grade, wearing
a summary. It is a constant, so the line said the same thing about every
manuscript.

The `need:` half stays: `citation_need` genuinely answers both ways and its
figures are measured (§11 D78). The `support:` half becomes a count of what was
LOCATED, with the same one-line disclosure the report and the evidence card
carry.

#### Two of four sidebar collections could never be non-empty

The nav comment ratified in August claimed *"every one is a real filter over
real state, so every nav click changes what you see."* That was wrong about half
of them:

- **From manuscript** filters `source === 'extracted'`. Nothing in the shipped
  app ever sets that value — it arrives only through the `extractedCitations`
  prop, and the single mount site (`GaplyScreens`:
  `<CitationManagerPage aiInstalled={…} />`) does not pass it. The
  *"Import from manuscript (0)"* button beside it is permanently disabled for
  the same reason.
- **Orphans** needs `computeStatus() === 'orphan'`, which needs
  `c.cited === false`. **Nothing anywhere assigns `Citation.cited`.** (The
  `'unused'` status has no branch at all — it exists in the type, the icon map
  and the label map, and is unreachable from any input.)

An empty "Orphans" does not read as *this cannot run*; it reads as *you have
none*. That is a clean bill nothing computed — §11 D85's test, failed by a nav
item.

**Both are CONDITIONAL now, not deleted.** Each appears exactly when it has
something to show, so the nav describes the library in front of you and the
entry returns on its own the day the capability lands — `manuscript` when an
analysed manuscript actually feeds `extractedCitations`, `orphans` when
something matches library entries against a manuscript's markers and sets
`cited`. The audit already does that matching; it is where the capability would
come from. The filter arms in `visible` are untouched.

### D112 — fourteen commands cross the IPC boundary untyped

Recorded as its own entry because it is a decision to take, not a defect to
patch, and §11 D109 should not be where it is buried.

`ai_job_results` is declared `-> Result<serde_json::Value, GaplyError>`. So is
every other command in this list. D103's guard reads wire types off the command
signatures, and `serde_json::Value` is in its `NOT_A_WIRE_TYPE` list — so the
app's busiest surface is invisible to it, and D109's bug lived there.

```
ai_citation_audit_preview    ai_job_recheck_items
ai_citation_audit_start      ai_job_results
ai_citation_need             ai_job_resume
ai_citation_support          ai_job_start_thesis_audit
ai_generate_test             ai_job_status
ai_job_export_report         ai_link_citations
get_report                   ai_link_source_document
```

Every AI job command is here. The TypeScript readers for these are hand-written
against shapes assembled inline with `serde_json::json!({…})`, so there is no
type on either side of the boundary to compare — only a convention nobody
checks.

#### The option considered and NOT taken

Making the guard report *"shape not declared, therefore not checked"* for each
of them. It verifies nothing; it converts fourteen silent holes into a visible
list, and then reports the same fourteen failures forever until someone does the
real work. A guard that always fails is a guard people learn to ignore — the
same reason §11 D108 rejected the 248-hit underscore rule.

The real work is giving these commands declared return types, which is a typing
decision with its own scope: some assemble their JSON from several sources, and
`ai_job_status` in particular returns a shape the frontend reads a dozen fields
from. **That deserves its own investigation, not a red guard.** Recorded here so
the list exists when it is taken up.

#### What WAS taken, from §11 D109

The narrower half: an ESLint rule, `no-any-cast-on-props`, forbidding
`as any` / `as unknown` on a JSX prop value. That is what actually defeated the
type system in D109 — both types were correct and the compiler was told not to
look.

Scoped deliberately. It does NOT flag `x!`: a non-null assertion erases
nullability, not shape, so the checker still compares every field and it cannot
produce D109's bug. Including it fired on two already-narrowed branches in
`PlagiarismCheckPage` that are fully type-checked, and a rule that reports
non-bugs is one someone switches off. Test files are exempt — with a note in the
rule that a test which casts is exercising the vocabulary rather than the path,
which is precisely how D109 stayed invisible.

It found one real cast outside the audit (`StatsVerifierReport`, a `BadgeStatus`
union that was already assignable — the cast bought nothing and only stopped the
checker confirming it), now annotated instead. Reintroducing D109's own
`items={items as any}` fails the rule.

### D113 — the importer invented a citation, and four other things it lost

A researcher's first act is importing their library. `importCitations` had 13
tests and **every fixture in them was a hand-written one-line string** —
`@article{a2020, title={Alpha Study}, ...}`. §11 D98's pattern exactly: input we
wrote, written to be easy. It passed while the splitter was cutting real entries
in half.

Corpus built to be hard (`src/screens/citations/fixtures/`), run through the
REAL importer. **Nine of nine entries survived the three main files with zero
failures** — BOM, CRLF, LaTeX accents (`Fern{\'a}ndez` → Fernández), multiline
abstracts, bare `month = oct`, nested case-protection braces, a missing trailing
newline. The defects were all in the edge files.

**Provenance, stated in the fixture README and in each file:** these are
reconstructions of what Zotero/Mendeley/EndNote emit, not exports from a real
install. In particular **default Zotero and Mendeley do not emit `@string`** —
JabRef and hand-maintained files do — so defect 3 is real but narrower than
"Zotero users".

#### 1. An `@` inside a field value split the entry — and fabricated a citation

The worst defect found in this project. `splitBibtex` was
`text.split(/(?=@\w+\s*\{)/)`, which matches anywhere, including inside a value:

- an abstract quoting BibTeX (`…write @article{foo, title={bar}}…`) was cut in
  two. The real half failed to parse and was reported; **the trailing half
  parsed CLEAN and was added to the library as a paper titled "bar"**;
- `note = {Corresponding author: nora@lab {group site}}` — `@lab {` matched
  because the pattern allowed whitespace before the brace. Both halves failed
  and the entry was lost.

A fabricated citation is worse than a wrong verdict: the researcher never sees
it arrive. It also inflated the tally — a 2-entry file reported `total=3`.

Replaced with a brace-depth scan: an `@` only starts an entry at depth 0, and a
backslash escapes the next character so `\{` in a LaTeX field does not shift it.

**The regression this caused, and the fix for it.** An unterminated entry now
swallowed the rest of the file, and a pre-existing test — *"one malformed entry
never kills the batch: good/bad/good → 2 added, 1 failed"* — caught it
immediately. Recovery cuts at the next `@type{` **at the start of a line**, which
is precisely the discriminator between a real entry (column 0 in every emitter)
and the hazards above (mid-line). So the recovery cannot reintroduce the split it
just fixed.

#### 2. `crossref` was not inherited

A child with `crossref = {parent}` and no year of its own imported with
`year: undefined`, and `computeStatus` then labelled a perfectly well-formed
reference **"malformed metadata"** — the tool calling the user's library wrong.
One level of inheritance now, plus BibTeX's real special case (a child in a
collection takes the parent's `title` as its `booktitle`). A dangling `crossref`
is left alone rather than invented.

#### 3. `@string` macros silently dropped the field

`@string` definitions were filtered out BEFORE parsing, so `journal = jml` could
never resolve: citation-js saw a bare token, dropped it, and the entry imported
looking complete with no journal and no error — losing the one field every
bibliography style prints. Macros are now collected first and expanded into the
entries.

Only a BARE identifier is substituted, and an UNDEFINED one is left alone:
`month = oct` is a built-in macro no file defines, and rewriting it to nothing
would lose data to fix a different problem.

#### 4. `and others` became an author called "others"

`author = {Nested, Nora and others}` produced a person whose family name was
literally `others`, printed as a real co-author by every style. Dropped — CSL-JSON
has no et-al marker and an invented collaborator is worse than a short list.
**Stated rather than hidden:** the entry no longer records that further authors
exist, and recovering that needs a field `Citation` does not have.

#### 5. Re-importing duplicated the VIEW, not the data

`added=0, skipped=2, review=3` on a re-import — the DOI-less entries go to
`review`, which the page applies under keep-both. The **store was already
correct**: `local.upsert` is keyed by `Citation.id`, which
`normalizeToCitation` derives deterministically from DOI, else title+year. Only
`setCitations` double-counted, prepending unconditionally — colliding React keys
were the tell, and the list silently collapsed back on reload. Fixed in the view
alone: an entry already present is replaced in place, which also keeps its
position stable.

#### What the corpus now guards

Ten cases, five of which failed before these fixes and five of which passed and
are pinned so a fix cannot quietly break them (line endings, no trailing
newline, re-import, BOM, RIS). The rule they encode: **an entry that cannot be
parsed must be REPORTED, never silently dropped — and the importer must never
produce a citation the user did not have.**

### D114 — the export button did not say what it exports

Three honesty defects in one pair of handlers, found in §11 D113's sweep.

#### The scope was silent

Every export serialises `visible` — the current collection filter and search —
not the library. Standing in "Retracted Items" and pressing **Export BibTeX**
wrote only the retracted entries, under a label that said nothing about it.

Exporting a filtered view is a legitimate thing to want, so the SCOPE stays and
the LABEL changes: **"Export BibTeX — 1 of 3"**, with a note beside the row
reading *"exports this view, not the whole library"*. Both appear only when the
view is actually narrowed — an unfiltered library shows the plain label, because
a count that never differs from the total is noise.

#### An empty export reported success, confidently

Zero visible citations wrote an empty file and toasted *"Bibliography exported"*
with tone **`certain`** — the most confident tone the vocabulary has, over a file
containing nothing. It now writes nothing and says which of the two situations
it is in, because they need different actions:

- *"Your library is empty, so there is nothing to export."*
- *"This view has no citations — the filter or search matched nothing. Clear it
  to export the library."*

Blaming a filter when the library is empty, or the library when a filter is on,
sends the reader to the wrong place.

#### Cancelling the save dialog reported success too

The same toast fired when the user pressed Cancel: `saveExportToFile` returned
and nothing checked the result.

**The check is not simply `=== null`.** `saveTextFile`'s contract returns null
for BOTH a cancelled Tauri dialog and a SUCCESSFUL browser download, so treating
null as failure would silence every browser export. Only in Tauri does null mean
cancelled, which is what `wasSaved()` encodes. A cancelled save now says nothing
at all — the honest report of an action the user chose not to take.

### D115 — triaging the unreferenced commands, and the stop button that was missing

§11's investigation reported 19 registered Tauri commands with no frontend
caller. Triaged; three of the four resulting notes are corrections to the
investigation rather than to the code.

#### The investigation had already been done, in `lib.rs`

**Twelve of the nineteen were already classified, with reasons, in the
registration list itself** — *"Reserved backend infra… Not dead"* over the
project/DB/RAG/extraction nine, and *"Reserved keychain-write seam… Kept
intentionally"* over the three secrets commands. They are left exactly as they
are.

**This is the second time this session a finding restated a decision the code
had already recorded** (the first was §11 D90, where the Manager and the Audit
turned out to use the identical condition the entry claimed differed). Both
times the comment was right and the sweep was mechanical: grep found the
absence of a caller and did not read the paragraph sitting directly above the
symbol. *Read the decision beside the code, not only the code.*

#### One claim in that comment WAS stale

It asserted each reserved command is *"exercised end-to-end through real Tauri
IPC by `tests/commands_test.rs`"*. True for seven. **`get_project` and
`health_check` appear only in that test's handler registration and are never
invoked** — they have no end-to-end coverage. Comment corrected rather than
tests added; adding coverage is a separate decision.

#### Four off the IPC boundary, functions kept

`ai_index_document`, `ai_embed_document`, `ai_index_status`,
`ai_semantic_search`. The first three are the steps `ai_link_source_document`
composes, and its own doc says why they must not be called separately: *"a
citation whose document is created and indexed and NOT embedded is exactly the
`unverifiable` state the user was trying to leave, and a half-linked source is
worse than an unlinked one because it looks done."* Exposing the pieces only
offers a way to get that wrong. `ai_semantic_search` wrapped retrieval no
surface uses; the retrieval FUNCTION is live inside the audit.

The Rust functions stay — they are the blocks the composition is built from.
Verified safe: `ai_link_source_document` calls
`gaply_core::ai_engine::embeddings` directly, not these commands.

#### `ai_embed_cancel` was not a dead command, it was a missing button

Indexing and embedding a thesis runs for minutes and streams *"Embedding 32 of
113 passages…"* the whole time. `ai_link_source_document` reads the shared
cancel flag between batches — **and the only command that SETS that flag was
unreachable from the frontend, while no surface offered the action.** A
multi-minute operation with a live progress indicator and no way to stop it.

A Stop button now sits beside that progress line. The flag is read BETWEEN
batches, so the current one finishes and the button says *"Stopping…"* rather
than "Stopped" — and a note says what survives: already-embedded passages are
kept and the source stays linked, it is just not checkable until the rest is
embedded. That is not a consolation, it is what the backend actually does:
`link_manually` runs even on a cancelled embed, because the link is true either
way, and `checkable` is computed from the vectors that exist.

**Offered for LINKING only.** The open-access fetch embeds too, but
`oa_fetch`'s loop never reads the flag, so a Stop there would be a button that
cannot succeed (§11 D101). A test asserts its absence during a fetch.

#### The orphan-detection correction — for when the Orphans nav returns

§11 D111 removed the "Orphans" collection because nothing sets `Citation.cited`,
and said it should return "when something matches library entries against a
manuscript's markers". **`ai_link_citations` is NOT that something**, which was
the natural guess.

`citation_links::link_citations` matches `citation_library` against
**`documents`** — which cited works have a readable indexed copy. It answers
"can this be checked", not "is this cited anywhere in my manuscript", and it
cannot set `cited`.

**The seam is `ai_thesis_audit_preview`, which the UI already calls.**
`preview.sources` is one entry per DISTINCT cited work, carrying `libraryId`
and `citingSentences` — computed by the pre-pass's marker resolution against
the manuscript text. A library entry appearing there with `citingSentences > 0`
is cited; one absent from a preview of the manuscript the user is working on is
an orphan of that manuscript.

The word *"of that manuscript"* is the whole difficulty, and why this is
recorded rather than built: `cited` is a property of a (citation, manuscript)
pair, and `Citation` has nowhere to put it. Storing a bare boolean would make an
entry "not an orphan" forever after one audit of one paper — a stale fact
presented as current, which is §11 D85's failure again.

### D116 — measuring the untyped surface: 2 of 14 readers were already wrong

§11 D112 listed fourteen commands returning bare `serde_json::Value` and left
the decision open. The question that settles it is not how many are untyped but
**how many already disagree with what the frontend reads** — §11 D109 was a
reader asking for a field that never existed on exactly this surface, and it
survived because nothing could check it.

Measured by sampling REAL payloads from the live database (a completed
thesis-audit job) and diffing them against every TypeScript reader.

#### The split

**Five are not really untyped.** `ai_citation_audit_preview`,
`ai_citation_audit_start`, `ai_job_start_thesis_audit`, `ai_link_citations`,
`get_report` all `serde_json::to_value(&typed_struct)` — serde attributes still
govern the names; only the signature erases it. **Nine assemble JSON by hand.**

#### The count

**2 of 14 had a reader asking for fields the payload does not carry.**

* `ai_job_results` — §11 D109, already fixed: `resultJson` (a string) read as
  `result.output`.
* `ai_job_status` — **live until this entry**, and the numbers are the argument.

Everything else verified clean against real keys: `health.*` (6 reads, all
present), `ai_link_source_document` (the `& { outcome?, preflight? }`
intersection declares exactly the two fields the retry path needs),
`ai_citation_support` (10 reads), `ai_citation_need`, `ai_job_export_report`,
`ai_job_recheck_items`. `ai_job_resume`'s return value is never read;
`ai_link_citations` has no TypeScript caller at all.

#### What `ai_job_status` was doing, on a real job

`FlaggedItem.result` is the item's stored `result_json`, parsed, verbatim — so
**the shape under `result` differs per kind**, and three kinds land in
`flagged`:

```
65 flagged items
  citation_need    46    prose at result.output.reason
  unverifiable     16    prose at result.reason
  citation_support  3    only ONE carried output.verdict
```

Every one of them got a "Show evidence" button into the support `EvidenceCard`,
which reads `output.verdict`, `output.explanation` and
`output.supporting_chunks`. So **64 of 65 drill-downs rendered "No evidence
retrieved"** — and the 46 need-items rendered an EMPTY explanation, because the
screen read `result.reason` while their prose sits one level down at
`result.output.reason`. The field existed and was never read.

#### The fix is routing, not a field name

Reading the right field would still have shown a suggestion inside an evidence
card. Each kind now renders through the surface that fits it:

* `citation_need` → §11 D89's suggestion treatment: no verdict vocabulary, its
  own hit rate stated, no route into the evidence card;
* `unverifiable` → a blocked source, from the deterministic `result.reason`,
  pointing at the fix loop below;
* `citation_support` → the evidence card, which is the only kind that has one.

**And the summary stat was the same conflation.** `flagged.length` under one
label, *"need review"*, summed 46 suggestions at ~43% precision, 16 library gaps
and 3 checked claims into a single number. Split into three, because the actions
differ.

#### The spelling split, fixed while nothing branched on it

`thesis_audit.rs` emitted `need:no_citation_required` — the only occurrence of
that spelling, against nine uses of `no_citation_needed` (the report, the
export, the annotated view). Nothing branched on either string, which is exactly
why it was worth fixing now: a one-line change today, a stored-data migration
later.

#### The test lesson, again

The existing unit tests passed throughout, while 71% of drill-downs were empty
on real data. They exercised the vocabulary; nothing exercised the PATH. The new
tests build `flagged` entries to the exact `ai_job_status` wire shape — per-kind
`result` layouts included — and assert each kind renders its own content.
Reverting to single-path routing fails two of them.

**This is the argument for typing the surface.** Two of fourteen were wrong,
both on the busiest command in the app, and neither was detectable by any guard
we have. The list in §11 D112 is the work; this is the reason to do it.

### D117 — `different` was never defined, so `contradicts` was unreachable

The disqualifying question first: **why is `different` dead?** 0 in 108 outputs
from the shipped 3B is not a preference. The answer is the prompt, and it is
narrow enough to quote in full.

#### Everything the model is told about `different`

Two places. That is the whole set, across EVERY version of this prompt in the
project's history (checked with `git log -p`):

1. `OUTPUT_SCHEMA` — `{"element": string, "status": "found|absent|different"}`.
   A bare enum token.
2. `FATAL_RULES_STATED` rule 2 — *"verdict 'contradicts' REQUIRES at least one
   claim_element with status 'different'. If nothing in the decomposition
   differs, the verdict is 'weak', not 'contradicts'."*

**`different` is never defined.** Nothing anywhere says what makes an element
`different` rather than `absent`. The single sentence that names the value ends
by telling the model the default is `weak`.

Meanwhile `weak` is pushed three times:

* SYSTEM: *"You are deliberately conservative. Partial topical overlap is NOT
  support."*
* SYSTEM: *"A source that discusses the same topic but does not report the
  claimed finding is \"weak\"."*
* the tail of the one rule that mentions `different`.

The `RULES` block defines the VERDICT `contradicts` ("evidence states the
opposite direction or a null result") but never the element STATUS that the
validator requires before that verdict is legal. The two vocabularies were never
connected.

#### It is not the model, and not the schema

The 1.5B produced `different` once — the only instance in 218 outputs — so the
token is reachable. What it produced is the more useful evidence:

```
elements: [absent  "wild bee visitation rates"]
          [found   "organic management"]
          [different "wild bee visitation rates"]     <- the SAME element, twice
verdict : contradicts        (gold: insufficient_evidence)
```

The same element appears twice with contradicting statuses, and the verdict is
wrong. That is not the vocabulary working; it reads as the model reaching for
the token the fatal rule demands in order to make its chosen verdict legal —
the rule taught it to fabricate the element post-hoc rather than to decompose.

#### The fitness verdict, stated carefully

`contradicts` is **structurally unreachable in practice**, and a citation that
says the opposite of its source is the failure this feature exists to catch. On
current evidence the task cannot report it.

But this is NOT yet a verdict on the task or the model class, and the difference
matters: **the value has never actually been asked for.** Concluding "beyond a
local model" from a prompt that never defines the term would be the §11 D74
mistake — generalising from one manuscript — in a new form. What is established
is narrower and firmer: *as posed*, the task cannot emit `contradicts`, and no
amount of model capacity fixes an undefined term.

Defining `different` is the obvious next experiment and is NOT bundled into
D117's own-work change below — two prompt edits measured together are one
uninterpretable result (§11 D64).

### D118 — the harness refuses a contended run rather than labelling one

`ai-eval` used to print `NOT declared isolated, 1m avg 2.97` and run anyway. **A
note in a header is not a control**: the report is still written, the numbers
still land in `evals/reports/`, and weeks later nobody reading the JSON knows the
machine was busy. §11 D33 already says timing figures from different load
contexts are not comparable — that is a RULE, and a rule the tool declines to
enforce is a suggestion.

A 1-minute load average above **2.0** now STOPS the run. `--isolated` is the
operator declaring the machine quiet; `--allow-contended` accepts that the
numbers are not comparable and records that fact in the report
(`allowedContended`) rather than leaving it to memory. Passing `--isolated` while
the load says otherwise is refused outright with its own message — that
combination would stamp `ranIsolated: true` onto a contended run, which is worse
than an unlabelled one.

Verified against all three flag combinations, not assumed.

### D119 — the entry shape: what the model actually emits, read from the bytes

`cs-label-004` and `006` failed with
`supporting_chunks[0].chunk_id: references chunk_id "CHUNK_ID=c11 PAGE=1 SECTION=-…"`,
which reads as a chunk-id defect. **It is not.** Reading the raw output:

```text
cs-label-006:  ["CHUNK_ID=c24 PAGE=1 SECTION=-", … six of them]
cs-label-004:  ["CHUNK_ID=c11 PAGE=1 SECTION=- We organized the SemEval-2018 …"]
```

`supporting_chunks` arrives as an array of **bare strings**. The object is never
built, and `require_known_chunk` reports the string as a bad `chunk_id`.

**The model is not confused about the id's boundary.** cs-label-006's strings
stop EXACTLY at the end of the header, before the text — it perceives the header
as a unit and knows where it ends. **A rendering change isolating or quoting the
id would have measured nothing**, and that was the change about to be built from
the error message. Reading the bytes is what stopped it — the same save as D81
and D107, and now the reliable move rather than a lucky one.

Enumeration and the missing `why` are ONE behaviour: cs-label-006 cited all six
chunks it was sent, one string each. With no justification to write there is
nothing to select on, so everything gets listed.

v1.9 acted on this with `why` first in the entry (D78's ordering finding: a field
generated first conditions what follows) and one entry shown filled in. **The
shape fix worked** — every case producing JSON emitted objects. What it cost is
D121.

### D120 — isolation is a claim about the whole run; and the guard that only watched the start

D118 made the harness refuse a contended run. It checked the load average at
START and then stopped watching. The v1.9 cell went **1.77 -> 4.31** and was
still stamped `ranIsolated: true`.

**A guard that verifies an opening condition and then looks away is the same
shape as a check that measures something other than the product** — the failure
this project has now recorded seven times.

`ranIsolated` is therefore the declaration as HONOURED, not as passed: it is
false when the machine got busy mid-run, alongside `isolationDeclared` and
`isolationHeldToEnd` so a downgrade is visible rather than silent. **Downgraded
rather than fatal**: the run has already happened and its verdicts are still
valid under greedy decoding — it is the TIMINGS that are not comparable, and
destroying the report would lose the verdicts to protect a number nobody should
have used. It fired on its first real use (v1.10, 1.74 -> 4.68).

*One correction for the record: `--isolated` was NOT failing to persist. It was
recorded under `loadContext`, and the check that reported it missing looked at
the top level. A defect was reported that did not exist; only the end-of-run gap
was real.*

### D121 — a worked example is imitated as a template, not read as an illustration

Two hazards, both measured on the cold-6, and they generalise past this feature.

**1. LITERAL LEAKAGE.** The v1.10 example was
`{"why": "reports 21 of 56 doctors, the 37.5% the claim states", "chunk_id": "c7", "page": 4}`.
On `cs-label-004` the model emitted `chunk_id: "c7"` — **the example's own id**,
against evidence containing `c11`-`c17`. Under v1.9, whose example was identical,
it invented `c1`. And `cs-label-005` produced
*"reports 46 of 58 macro-F1, the 77.9% the claim states"* — the example's
sentence frame with the numbers swapped.

**2. SIBLING-FIELD NEGLECT.** The example showed ONE `supporting_chunks` entry,
not the whole object. Three of six v1.10 failures were **`missing field
'confidence'`** — a field neither variant mentions, in an object that was
otherwise well-formed with correct shape, `why` first and real element statuses.
**Demonstrating a sub-object pulls attention to it and away from its siblings.**

**Worked examples are a HAZARD on this model class, not a default technique.**
§11 D98 already said an example must not teach to the measured case; that was
about the example's CONTENT. This is stronger and about its FORM: the model
reproduces the example's structure and values rather than generalising from
them, so an example can break fields it never mentions.

**The grounding check earned its keep.** D18's identifier guarantee caught both
invented ids — `c1` and `c7` — the first time it has caught a real fabrication in
a cell rather than in a test. A well-formed object with a plausible id that is
not in the evidence is indistinguishable from a real citation to everything
downstream.

#### The aggregate, which is the honest summary of this line of work

| cell | change | result vs baseline |
|---|---|---|
| v1.7-chunkbound | state the 4-chunk bound | **harmful** — planted-chunk 75% -> 25%, and Metal validity 6/6 -> 5/6 |
| v1.8-ownwork | own-work decomposition rule | **null** — not one of 6 cases changed |
| v1.9-entryshape | entry example + `why` first + exclusion line | **worse** — 3/6 -> 2/6 |
| v1.10-…-noexcl | v1.9 minus the exclusion line | **worst** — 3/6 -> **0/6** |

With `citation_need`'s v2/v3/v4 (D77, D78: recall 0% / 68% / 82%, all <=50%
accuracy), that is **five prompt variants across two tasks, none above baseline.**

**The prompt is not the lever on this model.** D64 established that rules compete
rather than accumulate; D78 that reordering changes the direction of errors and
not their rate; D121 that examples are imitated rather than generalised. Three
different mechanisms, one conclusion. **The whole-object variant was NOT run** —
four cells at increasing cost with no gain is enough evidence about the
technique, and a fifth would measure the same thing again.

**v1.6 stands as the shipped prompt.** v1.7, v1.8, v1.9 and v1.10 are kept in the
harness (`--support-variant v1c / v1o / v1e / v1n`) so their reports stay
reproducible; none is reachable from the app.

### D122 — a wrapped line is the document's break or ours, and only the reader can tell which

A full-paper audit produced three "suggestions" that were not sentences:

```
"RAM), Python 3.9, TensorFlow 2.10, and NLTK 3.7."
"3.3 points; SMOTE-Text costs 2.3 points."
"BiLSTM, and ATAE-LSTM across a total of six evaluation"
```

Two of the three were flagged as needing a citation. The significance filter was
blamed first and was innocent: `sentences_in` never had a boundary to get wrong,
because there is no `.!?` between `(6 GB` and `RAM)`. **The blocks arrived
already cut.** `tt.docx` records those sentences across separate `<w:p>`
paragraphs — the author pasted from a two-column layout and Word made each
visual line its own paragraph. The half that ends in a period then looks like a
complete sentence to any terminal-punctuation check.

`rejoin_wrapped_blocks` repairs them before the pre-pass loop sees them.

**The absorbed block is BLANKED, never removed.** `prepass_blocks` counts a
paragraph ordinal over every block including skipped ones, because that locator
has to match what the reader counts in their own document. Dropping a block
would shift every ordinal after it.

#### The distinction that decides where this may run

> **A break the reader can SEE belongs to the document. A break they cannot see
> belongs to us.**

In body prose a paragraph break is invisible — Word renders the wrap and the
paragraph identically — so a sentence split across two blocks is ours to repair.
In an **auto-numbered list** the same break is visible: Word puts a number in
front of it. `style_is_list` is therefore refused by `may_absorb`, and the
rejoin stops at the references heading.

**This was tested against the finding it could have deleted.** The same paper's
reference list wraps entry 5 across two paragraphs, and the audit reported it:

> "[6] does not look like a reference entry — it names no author, and reads as a
> page range: *pp. 436–465, 2013.*"

That finding was proposed for removal as an artefact of this same defect. **It is
not an artefact and it was not removed.** `<w:p>` 356 carries its own `<w:numPr>`
with `numId=27`, identical to its neighbours, so Word renders **22 numbered
entries and entry 6 really is the page range** — the reader's reference list is
genuinely broken, and D67 had already settled this on this very paper. Measured
before and after: the bibliography holds 25 entries with `[6] = "pp. 436–465,
2013."` in both. A test pins it.

#### A TRUE FINDING WAS NEARLY RETRACTED ON INFERENCE

This is the part worth remembering, because nothing about it felt like a risk.

The three prose fragments and the reference-list break have the SAME cause — a
wrapped line Word recorded as its own paragraph. From that shared cause it
followed naturally that they were the same defect, and the instruction given was
explicit and confident: confirm the `[6]` finding disappears after the rejoin,
and record prominently that the audit's highest-severity, most confident, most
actionable finding was an artefact that told a researcher to renumber a correct
reference list.

**That inference was wrong, and acting on it would have deleted a true finding
and published a false confession about the product's own accuracy.**

What stopped it was not judgement about the design. It was reading
`word/document.xml`:

```xml
<w:p><w:pPr><w:pStyle w:val="ListParagraph"/>
  <w:numPr><w:ilvl w:val="0"/><w:numId w:val="27"/></w:numPr>
  …</w:pPr>…<w:t>pp. 436–465, 2013.</w:t></w:p>
```

`<w:p>` 356 carries its OWN `<w:numPr>`, identical to 355 and 357. Word renders a
number in front of it. The reference list the reader sees really does have 22
entries with `[6] = "pp. 436–465, 2013."`, and the in-text markers run to `[24]`
against those 22. The finding was correct; so was its instruction to renumber.

Two things generalise:

- **A shared CAUSE is not a shared DEFECT.** The same wrapped line is our bug in
  prose and the document's bug in a numbered list, because the consequence to the
  reader differs — which is D67's distinction, already recorded, on this same
  paper. A plausible causal story reached the opposite conclusion from the one
  the file supported.
- **This is the session's own recurring lesson, arriving from the other
  direction.** Earlier it cost a wrong diagnosis to reason from a truncated error
  string instead of the raw output (D121's cell), and 1h52m to a monitor grepping
  for text that could never appear. Here the same discipline — read the bytes,
  not the story about them — protected a finding rather than corrected one. The
  cost of skipping it is asymmetric: a fragment that survives is noise, a
  retracted true finding is the product telling a researcher its correct answer
  was wrong.

The rejoin is therefore scoped to body prose by construction (`style_is_list`
refused in `may_absorb`, plus the references-heading stop), and
`a_wrapped_line_in_an_auto_numbered_list_is_left_alone` fails if that scope ever
widens.

#### The cascade this rule almost shipped

The first version paired "block A ends open" with "A has an unclosed `(`". On the
real document that let the caption `TABLE III. PER-EMOTION PERFORMANCE (HEFCSO-`
absorb **forty consecutive table cells** — `Emotion`, `Precision (%)`, `Joy`,
`96.55` — because the unclosed paren survived every join and re-armed the rule
each time. Those cells carry **no declared style**, so `style_is_table` never saw
them. 44 sentences vanished from the pre-pass and the count looked like a
successful cleanup.

Two fixes: the paren clause now requires the block that actually **closes** the
paren (`bt.contains(')')`), and absorption chains are capped at 3 so a rule that
stops discriminating fails small rather than eating a section. Joins on the real
document went 43 -> **3, exactly the three wrapped sentences.**

`"Std."` was also missing from `ABBREVIATIONS`, which cut
`…HEFCSO-BiLSTM, Std. BiLSTM, and ATAE-LSTM…` mid-sentence. It is the only
multi-letter abbreviation in PROSE that list was missing on this paper; the other
40-odd `Token.`-before-capital shapes are journal names inside the references
block, which never reach the prose path, or single-capital author initials, which
rule (3) already suppresses.

**A second-order effect worth recording:** with the sentence whole, `"Figure 4 is
an exemplary showpiece…"` is now correctly SKIPPED as a figure cross-reference.
The fragment had escaped that skip only because the `Figure 4` prefix sat in a
different block. Repairing the input restored a filter that was already right.

Measured on `tt.docx`: 309 -> 305 sentences, 84 -> 81 planned, bibliography
unchanged at 25.

### D123 — the number was not wrong about its sample; it was wrong about its subject

Every audit report printed:

> "on a labelled test set it flagged **82%** of the sentences that genuinely
> needed a citation — but only **43%** of what it flagged actually did"

and every suggestion carried "about 4 in 10 of these are real". Both constants
were honest measurements of a real eval (D78, D79), and a guard existed to stop
them drifting from it.

On a full-paper run the author read all 46 suggestions and found roughly **4**
defensible — nearer 1 in 10 than 4 in 10.

#### What the measurement was actually of

| where in the paper | judged | flagged | **labelled** |
|---|---|---|---|
| Title -> Proposed HEFCSO Algorithm | 26 | 15 | **14** |
| Experimental Configuration -> ACKNOWLEDGEMENT | 39 | 31 | **0** |

All 42 cold cases come from Abstract, Introduction subsections, Objectives and
early Methodology — the sections where "does this need a citation?" is a real
question. The shipped audit judges the whole document, and **two thirds of what
it flags comes from Results onward**, where the answer is structurally *no*: it
is the authors' own hardware, own split, own numbers, own ablations, and in one
case the ACKNOWLEDGEMENT paragraph.

Scored against the same labels **restricted to the audit's own selection**:
precision **25%** (tp 2, fp 6), recall 67% — n=8, too small to publish. Against
the author's read of the full report: **~9%**.

**Both constants are removed and nothing replaces them.** No re-measurement was
run: a replacement rate needs a labelled sample of the population the audit
judges, and that sample does not exist yet. The report now says what is true
without a number — that these come from a language model reading each sentence
alone, with no access to sources and no view of neighbouring sentences, and that
on a methods-and-results paper most will be the authors' own work.

#### The guard counted the labels and never asked where they came from

`advisory_figures_match_a_real_eval_of_the_shipped_prompt` required `>= 20` cold
cases and recomputed both rates from the eval report. It did both correctly and
**certified 43% on a set drawn from one paper's front third.** This is the
session's recurring shape again — D118's contended-run check, D120's isolation
guard: *a check that measures the start condition and then stops watching.*

Replaced by `a_printed_advisory_rate_needs_a_representative_set`, which asks for
**coverage, not count**: at least 10 cold cases on each side of the prior-work /
own-work split. A set of 500 Introduction sentences fails it.

Two properties of that guard matter more than the threshold:

- **It is armed by the thing it guards.** Nothing prints such a rate today, so
  there is nothing to recompute — and a guard that merely returns while disarmed
  is the shape that failed here. It reads `audit_report.rs` and arms the moment a
  `pub const ADVISORY_*` reappears under any name, printing the current coverage
  either way so the cost is known before the failure.
- **The population is DECLARED, not inferred.** The first version matched
  "result"/"discussion"/"introduction" against `input.section` and classified
  **0 of 42**, because papers name sections whatever they like — the real cases
  carry `"HEFCSO-BILSTM: A HYBRID"` and `"1.1 Universal Health Coverage and
  Employer Mandates"`. A keyword proxy standing in for the thing it cannot see is
  how the 43% got certified in the first place, so `labelling.population` is
  stated by whoever read the sentence.

### D124 — the workspace gate had stopped being able to close, and looked slow rather than broken

`cargo test --workspace` — the documented pre-commit check — could not finish
unattended on a machine that had ever provisioned the App Check signing key.
Four `pipeline::tests` sat at **0% CPU** inside
`app_check::TokenSigner::from_keychain` -> `secrets::get_secret`, indefinitely.

**It was never a regression.** It reproduced at HEAD with every local change
stashed, in the same frame. The gate had been unusable for some time, and the
failure mode is why nobody noticed: *a hung test is indistinguishable from a slow
one.* `cargo` prints "has been running for over 60 seconds" and keeps waiting,
which reads as a heavy test rather than a dead one. The tell is CPU —
ARCHITECTURE_TRACE's own diagnostic, written for the debug-vs-release model work:
**high CPU = working, 0% = wedged.**

#### What it actually was

`Entry::get_password()` on macOS does not merely look an item up. It checks the
CALLING BINARY against the keychain item's ACL, and when the binary is not on it
the OS asks for authorisation — a decision a headless `cargo test` runner can
never supply. `cargo test` rebuilds an unsigned binary with a fresh identity on
every run, so it is never on that ACL.

Measured rather than assumed, and the discriminator is the caller:

| caller | result |
|---|---|
| `/usr/bin/security` (Apple-signed, trusted for this item) | returns the value **instantly** |
| freshly built `app_lib-<hash>` test binary | **blocks at 0% CPU** in the same call |

The item exists (`ai.gaply.app / app_check_signing_key`, created 11 Jul 2026),
which is precisely why it blocked: an ABSENT item returns `NoEntry` immediately
and never prompts. **Provisioning the key is what broke the gate** — a developer
who had never run the app had a working `--workspace` and no way to know the
difference.

#### The fix is the path the design already documented

The four tests never needed a keychain. `ProxyReqwestClient::from_env` returns
`Err` when the signing key is absent, and `verify_proxy_with` already routes that
to local verification ("signing key absent; using local verification"). Under
`cfg(test)` the read is skipped and that same absent-key error is returned, so
the tests take a path the product takes in the field. `GAPLY_SKIP_KEYCHAIN`
covers what `cfg(test)` cannot reach — integration tests, a CI shell, a bisect.

**So none of them are `#[ignore]`d.** "These cannot run headless" would have been
the honest answer only if the keychain were part of what they test; it is not,
and marking them ignored would have shrunk the gate to avoid fixing it.

**NOT a production timeout.** In the shipped app the panel is answerable — there
is a window server and a signed, stable binary — and skipping the cloud tier
while the user reads the prompt would be the wrong answer.

Pinned by `building_from_env_never_touches_the_keychain_under_test`, which
asserts both properties that make the gate real: the call RETURNS (under 5s), and
it returns the absent-key error the fall-through keys on. Restore the
unconditional read and that test hangs — which is the honest failure, and it
fails at the seam rather than in whichever suite happens to run next.

Result: `cargo test --workspace` completes unattended — **1302 passed, 0 failed,
9 ignored across 13 suites**, with all five `pipeline::` tests passing. Warm wall
time **18s**.

#### A recorded number had drifted from what it describes, again

CLAUDE.md documented the same command as **"~93s, 752 tests"**. It is 1302 tests
and 18s warm — **stale by 550 tests**, and stale in the direction that flatters:
a reader budgeting 93 seconds would not question a run that took three minutes
and was actually wedged.

That is the small version of D123. The 43% precision figure was a real
measurement that stopped describing its subject; this was a real measurement that
stopped describing its command. Neither was ever wrong when written, and neither
announced that it had expired — **a number in prose has no guard.** The
difference is only in cost: one misled a researcher about their manuscript, the
other misled us about our own gate.

The figure is corrected and dated. The general lesson is D79's, arriving for the
third time in this line of work: a printed number needs something that fails when
it stops being true, and prose is not that thing.

### D125 — both sides of a comparison must be measured the same way, or it measures the instruments

D123 removed a printed rate because its labelled set covered a third of one
document. The replacement guard checked a floor — ten cold cases per half — and a
floor is a proxy. If a paper's judged sentences are ~45% prior-work / ~55%
own-work and the labelled set is 78/22, ten of each clears every floor and the
number still over-claims: it is weighted toward the half where "does this need a
citation?" is a real question and away from the half that is mostly the authors'
own work and mostly needs none.

So the guard now compares SHARES within a tolerance (10pp), and its failure
message names the population's mix, the set's mix, and **how many more own-work
cases to label** — because "you cannot print a rate" tells the next person
nothing they can act on.

#### The first design had two instruments, which is the same bug in miniature

The population's mix is summed from SECTIONS. The sample's half was, at first, a
`--population` flag declared per labelling session and written onto each case.
Those look equivalent and are not: the moment a per-sentence override exists —
and it must, because a prior-work claim can sit inside a Results section — the
sample becomes sentence-level while the population stays section-level.
**A comparison between two differently-measured sides measures the instruments,
not the thing.**

That is D123's lesson recurring one layer down, and it was caught by asking the
question the guard exists to ask: *what property does the number depend on, and
is that the property being checked?*

**Resolution: ONE instrument.** `population_mix.json` maps
`(document, section) -> prior_work | own_work`, and BOTH sides read it. A case's
half is looked up from its own `(document, section)`, never stored on the case,
so the two cannot drift. `labelling.population` was added and then removed for
exactly this reason; only `labelling.document` remains, because the section was
already recorded.

The cost is real, bounded, and symmetric: a prior-work claim inside a Results
section counts as own-work on BOTH sides. The stratum is *"sentences in sections
of this kind"*, and the population carries the same contamination as the sample
drawn from it — which is what makes the comparison valid. A per-sentence
override may still be recorded for analysis, but it may never feed the mix.

A side effect worth having: the declaration surface shrank from **42 cases to 11
sections**. Declaring a section is also the more defensible act — a labeller can
say what a section is for; asking them to re-declare it once per sentence invites
the drift the single map removes.

#### What is measured, and what is declared

- **Measured** by `label-cn` from a real pre-pass, per document, at labelling
  time: how many judged sentences each section contributes
  (`planned_total`, `sections[].planned`). Never typed by hand.
- **Declared** by the labeller, per section, via
  `label-cn <doc> --section <name> --population <half>`: which half that section
  belongs to. Never inferred from the section's NAME — matching
  "result"/"introduction" against real titles classified **0 of 42** cases,
  because papers name their sections whatever they like
  (`"HEFCSO-BILSTM: A HYBRID"`, `"1.1 Universal Health Coverage and Employer
  Mandates"`).
- Sections nobody has declared are counted as UNKNOWN and reported as a share.
  Above 25% the guard refuses to compare at all, because the population it would
  compare against is not known well enough to be a population.

`--section <substring>` was added alongside: reaching R PAPER's Results half
meant pressing through 30 already-covered front-half candidates first, and a tool
that makes the representative thing expensive gets the unrepresentative set.

### D126 — a stratified estimate reported unweighted is the same over-claim, and it passes every floor

D125 required the labelled set's MIX to match the population's. On the real
numbers that meant **44 more own-work cases** on top of 6 — because the audit's
population is 68% own-work and the set was 14% — to earn one number.

Stratification earns it from ~15 instead: sample each half deliberately, then
re-weight to the population.

```text
f_h  = N_h / n_h              (population / sample, per stratum)
TP   = Σ f_h·tp_h             FP = Σ f_h·fp_h        FN = Σ f_h·fn_h
precision = TP/(TP+FP)        recall = TP/(TP+FN)
```

`ai::eval_strata` is the whole of it — pure, no I/O, unit-tested. **The mix
requirement is dropped, because re-weighting is precisely what removes it.**
Demanding a matched mix *and* correct weights would ask the stratification to do
nothing.

#### What replaces it, and why a floor is not enough

> **A stratified estimate that is COMPUTED and then REPORTED UNWEIGHTED looks
> rigorous, keeps every stratum populated, clears every floor — and publishes
> the pooled number.**

That is D123 wearing better clothes, and a per-stratum floor cannot see it: the
strata *are* populated and the counts *are* real. So the guard recomputes the
printed figure from the per-stratum counts and the MEASURED population weights,
and asserts they match. Its failure names both, plus the pooled figure it would
have been mistaken for:

```
THE PRINTED RATE DOES NOT CARRY ITS WEIGHTS. printed 38%, recomputed 33%
from 6 own-work at 25% and 35 prior-work at 46% weighted prior_work 32%,
own_work 68% (pooled would read 44%).
```

**That message is not a mock-up — it is the guard's real output**, produced by
arming it with a placeholder constant and dropping the floor so execution
reached the assertion. An armed branch that has never run is the same latent
trap as the stub it replaced: both look like a check and neither has ever
decided anything. It was run, then both edits were reverted.

#### The run also answered a question left open by D123

Recomputed over the EXISTING 41 cold cases: **pooled 44%, weighted 33%.**

The rate this project printed for months was **43%**. So the shipped figure was,
within a point, the pooled number — and the same data re-weighted to the
population it is supposed to describe reads 11 points lower, *before* any
own-work sampling has been done. When the 15 own-work cases land the weighted
figure should fall further, because own-work is the half the model is worst on
(25% vs 46% here) and the half that carries 68% of the weight.

#### Three properties, each checked where it lives

- **Population known** — every judged sentence's section declared; above 25%
  unknown the guard refuses to compare at all.
- **Floor per stratum** — 10 cold cases each, so a per-stratum proportion is not
  noise. A floor, never a certificate.
- **Weights applied** — the printed rate recomputed from per-stratum counts and
  measured weights, and matched.

Same rule as D124 and D125, a third time: **the check has to be on the property
the number depends on, not on a proxy that is easier to count.** A floor counts
cases. A mix comparison counts sections. Only recomputation checks the
arithmetic that produced the figure a researcher reads.

#### TWO STRATA, DELIBERATELY — and what would change that

The own-work stratum is **visibly bimodal**, and not by intuition: by the
authors' own citation markers, counted per section by `label-cn --list-sections`
on the health-economics paper.

| own-work section | planned | cited BY THE AUTHORS |
|---|---|---|
| 6.3 Evidence-Tied Policy Implications | 7 | 3 (**43%**) |
| 6.1 Principal Findings and Their Interpretation | 11 | 3 (**27%**) |
| 7. Conclusion | 13 | 2 (15%) |
| 6.2 The SME Compliance Gap | 7 | 1 (14%) |
| 5.1 – 5.9, every Results subsection | 46 | **0** |
| 6.4 Limitations, Declarations, AOR | 24 | **0** |

Results prose cites nothing; Discussion cites in nearly a third of sentences.
A finer split — `own_results` / `own_interpretation` — would model that better.

**We are staying at two, and this records that as a CHOICE rather than an
oversight.** A third stratum multiplies the labelling, and — the actual reason —
*a split is easy to justify forever*. Deciding now would be modelling on
intuition, before any evidence about whether the two behave differently exists.

**The criterion for revisiting is stated in advance so it cannot be chosen after
the fact:** the first Discussion batch (6.1, 6.3) is being labelled now. If its
per-case outcomes look materially unlike the Results batch — meaningfully
different positive rate, or a precision gap of the size the prior/own split
itself shows (46% vs 25%) — that is evidence FOR splitting, and it will have been
measured rather than assumed. If they look alike, two strata were right and the
question is closed.

This is also why the first own-work batch is not enough on its own. All 24 came
from `4.3 Variables and Operationalisation` — 29 negative, 1 positive across the
whole stratum — and 4.3 sits at the ZERO-citation end of a range that runs to
43%. Weighting those up to stand for all 236 own-work sentences would say the
stratum is definitionally uncitable when a third of its Discussion is not:
**§11 D123's error one level down, inside a stratum instead of across a
document.** With `tp = 0` the stratum's precision is `0/(0+fp)` — a
false-positive rate wearing a precision label — and its recall rests on one case.
Real and worth knowing, but not a population estimate.

### D127 — the bare narrative citation is real, occurs once in 347 sentences, and the regex is NOT being changed

A sentence reached the `citation_need` queue carrying what is plainly a
citation:

> "Administrative costs per employee decline with scale, which gives a
> structural compliance advantage to larger firms, **as documented by Hadley and
> Reschovsky 2002**."

`markers_in` missed it. It handles `[7]`, `(Smith, 2019)` and the narrative
`Smith et al. (2019)`; this form has author names and a year inline with **no
parentheses at all**, so nothing anchors it.

**Measured before touching the regex, because a FALSE marker is worse than a
missed one.** The asymmetry is the whole decision: a false marker makes a
sentence count as CITED, so it leaves the queue silently and is never audited —
the manuscript loses a check and nothing says so. A missed marker costs one
extra suggestion in a list the reader is already skimming.

Across both audited manuscripts, 347 planned sentences:

| | R PAPER | health-econ |
|---|---|---|
| bare narrative citations OUTSIDE brackets that `markers_in` misses | **0** | **1** |

One occurrence in 347 sentences (0.3%), and it is the one that prompted the
question. The ~20 other author-year forms in that paper are inside parentheses
and already handled.

#### The measurement caught its own instrument first

The first pattern reported **two** hits, the second being `"Eval 2018"` — from
**Sem**`Eval 2018`, a dataset name, because the regex omitted a leading `\b` and
matched inside a word. A dataset name silently reclassified as a citation is
*exactly* the failure this rule would introduce at scale, and it appeared within
minutes of writing the probe, on the first paper tried.

That is the argument in miniature. The form is rare; the pattern that catches it
is one word-boundary away from suppressing a real sentence; and the papers most
likely to use bare narrative citations are also the ones full of
`Vision 2040`-shaped noun phrases.

**Decision: no change.** Revisit when a manuscript shows the form at a density
that matters — say >2% of judged sentences — measured the same way. The cost of
the miss is bounded and visible (one extra advisory suggestion). The cost of the
fix is unbounded and invisible (a real claim dropped from the audit).

### D128 — the advisory lane matches flagging every sentence, on the population it actually runs on

**THE SINGLE MOST USEFUL SENTENCE ANYONE HAS WRITTEN ABOUT THIS LANE, SO IT IS
STATED FIRST AND PLAINLY:**

> **Flagging EVERY sentence scores 18.0% weighted precision on this population.
> `citation_need-v4` scores 18.3%.**

A 0.3 percentage-point difference over 75 cases is noise. The lane is, at the
population level, **statistically indistinguishable from a rule that answers
"yes" to everything** — it flagged **62 of 75** judged sentences (83%). A prompt
to look that fires on four sentences in five directs attention nowhere.

This is written down because it is exactly the kind of finding that gets
forgotten and then rebuilt: the lane looks reasonable in isolation, produces
fluent per-sentence reasons, and its pooled number (24% here, 43% historically)
is low-but-plausible rather than obviously broken. **Only the comparison against
a no-skill baseline shows there is nothing there**, and nobody had computed one
in the lane's entire history.

#### The measurement

76 cold labels, stratified and population-weighted (§11 D126),
qwen2.5-3b-instruct-q4km, `citation_need-v4`, 83/84 valid outputs:

| stratum | tp | fp | fn | tn | n | N | weight | precision | recall |
|---|---|---|---|---|---|---|---|---|---|
| prior_work | 13 | 15 | 3 | 4 | 35 | 111 | 3.17x | **46%** | **81%** |
| own_work | 2 | 32 | 0 | 6 | 40 | 236 | 5.90x | 6% | — |

```text
WEIGHTED   precision 18.3%   recall 85%
POOLED     precision 24%     recall 83%
NO-SKILL   precision 18.0%   (flag every sentence)
```

**The own-work 6% is a FALSE-POSITIVE RATE WEARING A PRECISION LABEL** and must
never be quoted as precision: `tp = 2`, so the numerator is empty by
construction, and its "100% recall" rests on two cases. Its honest form is the
one that explains the 46 suggestions on R PAPER:

> **The model flagged 32 of the 38 own-work sentences that need no citation —
> 84%.**

#### What the pooled number was hiding

The two strata are not weak and weaker; they are *different*. Prior-work is a
real if modest signal — 46% precision at 81% recall. Own-work has none.

The lane's whole problem is that it runs on a population that is **68%
own-work**, where it is useless, while every previous measurement was taken
where it works. That is §11 D123 restated as a product fact rather than a
measurement error: the 43% was not a mistake about arithmetic, it was a mistake
about *which sentences the product judges*.

#### THE FIRST RUN OF THIS MEASUREMENT WAS INVALID, AND A GUARD CAUGHT IT

The own-work cases were labelled with `label-cn --with-neighbours`, which writes
`preceding_sentence` / `following_sentence` into the case. `job_runner.rs` sends
**empty strings** for both. So the 40 own-work cases were scored with context the
product never provides while the 36 prior-work cases were scored without it:
**the two strata were measured under different input configurations**, and the
weighted combination mixed them.

`citation_need_cases_send_the_neighbours_the_product_sends` (§11 D73) failed on
the first full-suite run after labelling and named the case, the field and the
remedy. Without it the headline number would have been published from a
comparison between two instruments — the D125 error committed *inside* the
measurement D125 exists to protect.

The fields were cleared (labels untouched — showing a human context while they
judge is right; shipping that context to the model is not) and the eval re-run.
Prior-work came back **byte-identical**, which is the check that the fix touched
only what it should. The first run put the model at 17.5% against an 18.2%
baseline — *below* no-skill — and that direction was an artefact: corrected, it
is 18.3% against 18.0%. **"Indistinguishable from flagging everything" survived
the correction; "marginally worse than" did not**, and the difference matters
because the first is a finding and the second would have been an overclaim in
the other direction.

#### A note on the labels, from the labelling session

Own-work yielded almost no positives: **one borderline positive across 34 new
cases.** The Discussion sections turned out to be the authors reasoning from
their own numbers — their gradient, their AOR, their barrier rates — with policy
inferences following from those.

The 27-43% author-marker density measured in 6.1/6.3 did NOT predict positives,
and the reasoning that used it to pick those sections was wrong: **cited
sentences are filtered out before reaching the labeller**, so what remains
uncited there is uncited *because it does not need citing*. Marker density
describes the sentences the queue never sees.

#### THE LANE IS RETIRED — and the reason is the distinction, not the number

**Decision: retire `citation_need`.** It rests on **18.3% against an 18.0%
no-skill baseline.**

> **It was retired not because it was WEAK but because it was INDISTINGUISHABLE
> FROM A RULE THAT FLAGS EVERYTHING.**

That distinction is the whole finding and it is the part that will get lost. A
weak signal invites improvement — a better prompt, a bigger model, a threshold.
This is not weak: there is no signal to improve, and five prompt variants across
two tasks (§11 D121) had already shown the prompt is not the lever. A feature
whose own documentation must say "it matches flagging every sentence" should not
ship, which is also why printing the weighted 18.3% beside the baseline was
rejected as an option.

What was removed, and what stands:

- **Report** — the "Worth a second look" section, its caveat paragraph, its
  proportional bar segment, and `emit_suggestion`/`emit_suggestions`. The
  section is DELETED rather than left to render "none found", which would imply
  a check that no longer happens. `"Worth a second look"` STAYS in
  `GAPLY_REPORT_MARKERS`: reports exported before today are still Gaply reports
  and the self-detection guard must still recognise them (the same reason D108's
  renamed heading is kept).
- **Screen** — the "to skim" stat, the flagged-item branch, the per-item
  suggestion label, and the `advisory` annotation status with its legend entry
  and highlight colour.
- **Export** — the `citation_need` arm. `counts_by_category` still tallies an old
  job's ROWS, because that is history and stays true; `verdict_counts` is empty,
  because it reports what is RENDERED. A reader seeing `citation_need: 2` with no
  verdicts beside it is reading the honest shape of a retired lane.
- **Planner** — no `CitationNeed` item is queued, so **no model call is spent**.
  Uncited sentences are counted as `not_examined` and the preview says so: the
  card used to add them to both the item count and the time estimate, promising
  minutes of work on a lane that no longer runs (31 minutes → 6 on the test
  fixture). A sentence nobody looked at is not a sentence that passed.
- **KEPT** — `ai/tasks/citation_need.rs`, its prompt versions, `label-cn`, the
  eval harness and the committed report, so the measurement stays reproducible.
  Exactly how v1.7–v1.10 were kept (§11 D121): present in the harness,
  unreachable from the app. `citation_support` was already the recorded primary
  feature.

Evidence and deterministic findings are untouched — the same treatment the
support verdict got in §11 D85.

#### THE PATH BACK, with its precondition, so it is available rather than forgotten

Prior-work sections are **46% precision at 81% recall**. That is a real if modest
signal, and restricting the lane to them was seriously considered.

**It is blocked on classification, not on the model.** Deciding which sections
are prior-work must be automatic — the population map that made this measurement
possible exists only because 52 sections of two papers were hand-classified, and
a shipped feature cannot ask that of a user. Measured:

| classifier | result |
|---|---|
| `extract::sections::detect_heading` (IMRaD, exact phrase) | **3 of 52 (5%)** |
| document position, best single threshold | 339 of 347 sentences (97%) — **in-sample** |

The 5% closes the first route: real headings are `"4.3 Variables and
Operationalisation"` and `"HEFCSO-BILSTM: A HYBRID"`, not `"Methods"`.

The 97% is not the second route opening. The threshold was fitted on the same two
papers it was scored against, and **the boundary already disagrees between
them** — health-econ's prior-work ends at 0.30, R PAPER's Related Work sits at
0.32. Those 5 sentences are a prior-work section classified as own-work, which
would be SKIPPED: the lane sitting out the one region where it works, while the
researcher is told their literature review was checked. "Prior work comes first"
also fails on shapes that certainly occur — Related Work at the end, review
articles, theses with interleaved literature chapters.

**THE PRECONDITION: positional classification measured OUT-OF-SAMPLE on five or
six manuscripts, before it gates what a researcher is told was checked.** Until
then the honest fallback is not classification at all but stated coverage
("checked the first 30%, through §3.5"), which asserts nothing about what those
sections are — crude, and it does not make the lane good; it makes a 46% lane
apply only where 46% is the number.

### D129 — the report contradicted itself, and the half nobody would have caught was the verification

A live audit of `R PAPER .docx` produced both of these, in one document:

> **Structural finding:** "[6] does not look like a reference entry … every
> marker pointing at them resolves to the wrong paper."

> **Unverifiable item:** "Yang [9] formalized FA" — *"[9] Whitley, A genetic
> algorithm tutorial — not in your library."* **Action:** fetch it.

A dozen such items. The report told the researcher its own numbering was
unreliable and then, lower down, named a dozen unrelated works as confident
resolutions with an action attached. `CSA introduced by Askarzadeh [10]` was
matched to *Yang, Firefly algorithm*.

#### One fact, known in one place and not the other

`consistency::check_reference_list` detects the malformed entry.
`audit_prepass::resolve_marker_with` resolves `[n]` by looking up position `n`.
**Nothing connected them**, so resolution trusted an ordinal the same report was
simultaneously calling wrong.

The predicate now lives in ONE place — `entry_is_malformed`, beside `BibEntry` in
`audit_prepass` — with `first_unreliable_entry` derived from it, and BOTH callers
read it. A second definition is exactly how the two halves drifted apart.

Entries BELOW the break still resolve and are still named: the shift only affects
the malformed entry and everything above it. At or above it, Gaply names no work
and says why.

#### THE HALF THAT MATTERED MORE, AND WAS INVISIBLE

The guard runs **before** `resolve_bib_entry`, not merely in the wording.

Had *Whitley* been in the library, indexed and embedded, the old path returned
`Checkable` — and the audit would have quoted **an unrelated paper's passages as
evidence for the claim**, with a page link, under "the source passages behind
each cited claim". The reader's only defence is the one thing the report promised
they would not need: reading the source themselves to notice it is the wrong
paper.

> **A wrong fetch instruction is visible. A wrong verification is not.**

The defect was reported as a dozen bad fetch instructions because that is what
showed on a machine whose library happened not to hold those works. On a fuller
library the same bug is silent and worse. Both are pinned:
`a_marker_above_a_malformed_entry_names_no_work` and
`an_unreliable_marker_is_never_checkable_even_when_the_entry_is_in_the_library`.

#### The general shape

Two deterministic checks over the same data, each correct alone, disagreeing in
one document because one of them did not know what the other had established.
This is the third time in this line of work that a defect lived in a SEAM rather
than in a component — §11 D122 (a shared cause with two different consequences),
§11 D125 (two sides of a comparison measured by different instruments), and now a
fact established by one check and ignored by the other.

**A finding that invalidates an input must reach everything downstream of that
input.** Where that cannot be arranged structurally, the downstream surface
must carry the doubt rather than print a confident answer.

### D130 — interval notation is not a citation, and the stronger-looking rule was the dangerous one

An audit of the health-economics paper reported two sentences as "cited, but not
checkable — numeric citation style". Neither cites anything:

> "Each item scored 1–5, standardised to **[0, 1]** by (score − 1)/4."

`[0, 1]` matched the numeric-marker regex. That is **§11 D127's asymmetry
realised**: a false marker makes the sentence count as CITED, so it leaves the
queue, is never examined, and the report asserts something false about it.
Statistical papers carry `[0, 1]`, `[0, 100]`, `[1, 5]` constantly.

Measured before fixing, across both audited papers:

| | numeric markers parsed | containing a zero | comma pairs |
|---|---|---|---|
| R PAPER (genuinely numeric-cited) | **21** | **0** | 0 |
| health-economics (author-year) | 2 | **2** | 0 |

**The rule is: a bracket containing 0 is not a marker**, and it is safe for a
STRUCTURAL reason rather than a statistical one — a numbered reference list
starts at `[1]`, so no citation marker can contain 0. It costs nothing where
numeric citations are real: not one of R PAPER's 21 contains a zero.

One detail worth keeping: the document used `[0,\u{202f}1]` with a NARROW
NO-BREAK SPACE, which `\s` matches. A rule written against `", "` would have
missed it.

#### The comma-pair rule, and the document-level rule, were both rejected

"A comma-separated pair inside one bracket is interval notation" was proposed.
`[5, 7]` is a standard multi-citation, and neither paper contains one (0 of 0) —
so there is no evidence for the rule and a real cost if it is wrong.

"A `[n]` in a paper with no numbered reference list is not a citation" looks
stronger and is worse. **It was implemented, and the planted-marker fixture
caught it in the same minute: that chapter carries numeric markers and no parsed
list, so the rule dropped ALL TWELVE.** The same happens to a real manuscript
whose reference list merely fails to parse — every numerically-cited sentence
silently becomes "not examined" and the audit quietly does nothing.

It also buys nothing measurable: the zero rule alone already takes the health
paper from 2 to 0.

> **A rule whose benefit is unmeasurable and whose failure mode is "the audit
> silently checks nothing" is the wrong trade** — which is D127's asymmetry
> pointing the other way. A missed marker costs one unexamined sentence; that
> rule costs the document.

### D131 — an author-year marker identifies a work approximately, and the report said it exactly

The same report listed **48 sources "not in your library"**. It was closer to
two-thirds of that, and several entries were not works at all.

#### One work counted as several

`reason` embedded `marker.raw`, and that string is the GROUPING KEY downstream.
So one paper arrived as three rows:

```
(AlJohani & Bugis, 2024)      (AlJohani and Bugis, 2024)      AlJohani and Bugis (2024)
```

Three spellings of Alkenbrack and three of Reka did the same. The key is now a
canonical surname-and-year, so every spelling collapses to one row.

#### An uncertain extraction printed as a certain work

The captured surname is not reliably the lead author. Measured on that paper:

| in the text | extracted |
|---|---|
| "The Financial Services Authority (2025)" | "Authority (2025)" |
| "Buchmueller, DiNardo, and Valletta (2011)" | "Valletta (2011)" |
| "Mathauer, Saksena and Kutzin (2019)" | "Saksena and Kutzin (2019)" |
| "Weiner's (2009)" | possessive kept |
| "(RBV; Barney, 1991)" | abbreviation kept |
| "(Dubai 2013, Abu Dhabi 2006)" | not a citation at all |

**`consistency` already reports this class as `uncertain-reference-match` rather
than asserting it. The unverifiable list asserted it anyway, with a fetch action
attached** — so the same report hedged in one section and instructed in another.
This is §11 D129 again, in a different pair of sections: *an uncertain resolution
must not print as a certain one.* The reason now says what it actually knows —
a surname and a year taken from the in-text marker, which may be an incomplete or
mis-split name rather than a missing source.

#### `lead_author` is a key; `lead_display` is for reading

The match key is lowercased, because `AlJohani`, `Aljohani` and `ALJOHANI` are one
author. Printing that key back at a researcher is its own small defect, so
`Marker::lead_display` carries the surname as written and is never a key.

The distinction proved itself immediately: a mechanical edit filled
`lead_display` from `lead_author`'s lowercased value, and the test asserting the
surname appears **as written** failed on the spot.

### D132 — the author-year reference list carries DOIs, and nothing read them

§11 D100 recorded the OA fetch's gap as "IEEE lists carry no DOIs, so a title
lookup is needed". That is true of R PAPER and it is NOT the whole gap. Measured
on the health-economics paper:

| | numbered entries | with DOI | author-year entries | with DOI | with title |
|---|---|---|---|---|---|
| R PAPER | 25 | 0 | 0 | 0 | 0 |
| health-economics | 0 | 0 | **35** | **26** | **35** |

**26 DOIs were sitting in the document and the audit read none of them** — not
because they were absent, but because `parse_numbered_bibliography` does not
apply to an author-year list and nothing else looked.

#### D100's framing was HALF right, and the half it missed is what made this cheap

D100 described the gap as "IEEE-style lists carry no DOIs, so a title lookup is
needed". For R PAPER that is exactly true — 25 entries, 0 DOIs, and nothing but a
title lookup will reach them.

It is not true of the other style. There the DOIs are present and the gap was a
**PARSER BLIND SPOT, not an absence** — and that distinction is the whole reason
this piece was cheap. An absence needs a network lookup, a confidence threshold,
and a wrong-paper gate. A blind spot needs a parser, which cannot link the wrong
paper at all. The expensive work was real but it was never the only work, and
D100 had bundled the two together under the expensive one.

Worth generalising: a gap stated as "the data is missing" deserves a check that
the data is actually missing. Here it was sitting in the document.

This piece is **0 → 26 fetchable by DOI** on that paper, and it supplies the 35
titles that piece 2 will need.

#### The parser was MOVED, not copied

`parse_author_year_entries` already existed in `consistency`, serving the
orphan-marker checks, which need only a surname and a year. It now lives in
`audit_prepass` beside `BibEntry` and the DOI regex it shares, carries `doi` and
`title`, and `consistency` imports it.

**§11 D129 is why that is a move.** Two definitions of "what this reference list
says" is exactly how one half of a report came to contradict the other half, in
the same module pair.

`report.bibliography` and `report.author_year_bibliography` are never both
populated: a manuscript has ONE reference list in ONE style, and parsing both and
keeping whichever is larger would invent a second source of truth about the same
text. A test pins that.

#### Staging, not the user's library

Entries are staged SCOPED TO THE MANUSCRIPT and never inserted into
`citation_library`. The reasoning, recorded because the cheaper option is the
obvious one:

- 35 entries include malformed ones and organisational authors ("P4H Network"),
  and `citation_library` is shared by every feature.
- Junk rows are hard to un-import.
- **The user never asked for those works to be in their collection.** They asked
  for their manuscript to be checked. Staging keeps the audit's needs separate
  from the user's library, and "promote to library" can be an explicit action
  later if anyone wants one.

#### ONE WORK, ONE ROW: the library wins and staging fills the gaps

A staged source and a `citation_library` row can name the same work, and must not
fetch twice or produce two rows in the report. **The library row is preferred and
only what is missing is staged**, deduped on NORMALISED DOI — lowercased,
`https://doi.org/` stripped, trailing punctuation trimmed.

The library row wins rather than the staged one because it is the user's own
asset: it may already carry a linked, indexed, embedded PDF and a title they
curated. A staged duplicate would be the poorer copy of something better, and
`citation_documents` links are keyed to `citation_library.id` anyway, so
preferring it keeps resolution on ONE path instead of teaching every consumer
about two.

Two rules, both on the normalised DOI:

1. An entry whose DOI matches a library row is **not staged** — the existing
   library path already handles it, including the fetch.
2. Staged rows are unique per `(job_id, normalised DOI)`, so a reference list that
   prints the same work twice stages it once.

**No title-based dedupe.** Deciding that two differently-spelled entries are one
work by title similarity is the same judgement as deciding a title lookup found
the right paper — so it is gated behind piece 2's measurement, not smuggled in as
a convenience here. An entry with a DOI and a library row without one stays two
rows until then, which is the honest state: nothing has established they are the
same work.

#### Only public identifiers leave the machine, and `raw` is not one

The fetch sends a RECONSTRUCTED title+author+year, never `reference.raw`. Raw is
manuscript text and can carry an author's own annotations;
`query.bibliographic` invites passing it, which would break the disclosure
silently rather than loudly. The title extractor exists partly to make that
reconstruction possible.

#### THE GATE FOR PIECE 2, STATED BEFORE PIECE 2 STARTS

Piece 2 is title-based resolution for the DOI-less remainder, and it is the only
piece that can link the WRONG paper — §11 D94's failure. Its acceptance number is
wrong-paper matches, measured on R PAPER's 25 DOI-less entries plus the health
paper's 9.

> **"0 wrong-paper matches in 34 tries" means NO FAILURE OBSERVED, not safe.**

That is the same in-sample problem as §11 D126's positional classifier: a clean
run on the two documents the rule was developed against is not evidence about the
next document. The number must be reported that way, and the acceptance bar set
against what 34 tries can actually support — not read as proof because it is a
zero.

### D133 — a fetched staged source must make a sentence checkable, and the lookup keys on the DOI

§11 D132 staged the manuscript's reference list and let the fetch reach it. That
produced **zero checkable sentences**, and would have kept producing zero however
many PDFs were downloaded: `resolve_marker_with` consulted `citation_library` and
nothing else, so a staged source whose PDF was on disk, indexed and embedded,
still resolved `Unverifiable`.

The measurement this unblocks, taken live before building:

| | |
|---|---|
| health-economics DOIs with a fetchable OA PDF | **13 of 26** |
| cited sentences those works carry | **11 of 47** (~23%) |

#### `Resolution::Checkable` is subject-shaped, and `SourceRef` MOVED DOWN

`Checkable { library_id: String, .. }` could not express "checkable via a staged
source". It is now `Checkable { via: SourceRef, .. }`.

`SourceRef` began in the app crate as `oa_fetch::FetchSubject`, for the fetch
alone. `Resolution` lives in `gaply-core`, which cannot depend on the app crate —
so the type **moved down to `gaply_core::source_ref` and `FetchSubject` became an
alias**, rather than a second copy being written upward. §11 D129 again: two
definitions of "which source is this" would drift, and the wire shape would drift
with them.

An enum rather than `library_id: Option<String>` beside `staged_id: Option<i64>`,
for the reason that pair is always wrong: it permits neither and both — two
states that cannot occur and that every consumer would have to handle or, more
likely, quietly mishandle.

The ripple was mechanical: preview grouping (where `has_doi` and `retracted` are
now ASKED of a library citation rather than assumed of every source), the planner
payload (`libraryId` kept for existing readers, `source` added as the complete
answer), the citation-scoped matcher, and `recheckItems`' matcher.

#### THE FORK: DOI-KEYED, NOT JOB-KEYED — three independent reasons

Staged rows are job-scoped, so the obvious lookup is "this job's staged sources".
It does not work, and not marginally:

1. **The preview has no job.** `preview_thesis_audit` runs before anything is
   created, so `would_check` would undercount every fetched source.
2. **The planner resolves before its job exists.** `create_job` needs the items,
   and the items come from resolution — so within one run there is no job id to
   key on.
3. **A re-run would re-download every PDF.** A fresh job means fresh staged rows
   with `document_id = NULL`, the fetch's `AlreadyLinked` short-circuit misses,
   and the work is done again.

Keying on the DOI answers all three, because it is keyed on the WORK rather than
on the run: *a fetched PDF is a fact about the paper, not about the audit that
happened to fetch it.* The same principle makes `stage_entries` inherit
`document_id` from any earlier staged row with the same `doi_norm`.

#### `Bibliography` carries BOTH lists, and that is load-bearing

The marker carries a surname and a year. The manuscript's own reference entry
carries the DOI. **Neither alone can find the file**, so resolution now receives
`Bibliography { numbered, author_year }` and `doi_for(marker)` is the bridge. It
works for numeric markers too — a numbered entry with a DOI gets the same
benefit, which R PAPER cannot use (0 DOIs) but a DOI-bearing IEEE paper would.

The test asserts BOTH directions: a fetched staged source makes its sentence
`Checkable` via `SourceRef::Staged`, **and** the same marker with only the
numbered side cannot reach the same document. The second assertion is what makes
the first mean anything — it shows the author-year list is load-bearing rather
than incidentally present.

Staged sources are checked AFTER the library, matching §11 D132's rule that the
user's curated copy wins.

#### A consequence worth stating: the first run cannot benefit

Resolution is a LOOKUP, not a fetch trigger. On the first audit of a manuscript
nothing has been fetched, so every cited sentence is unverifiable; staged sources
become checkable on a later plan, or via re-check after a fetch. That is inherent
to DOI-keying and it is why the re-check path matters more than first estimated —
without it, the only route to a checkable staged sentence is re-running the whole
audit.

#### Two notes for the record

**The framing was wrong before the work was.** This increment was scoped as
"reuse `recheckItems` with a subject-shaped argument" — a small wiring job. It is
a type moved between crates, an enum reshaped across five call sites, a new
lookup, and a cross-job inheritance rule. Saying so beat half-building it, and
the correction came from reading what `Checkable` could actually express rather
than from trusting the estimate.

**§11 D131 is lost coverage, not a cosmetic misprint.** "Saksena and Kutzin
(2019)", mis-split from "Mathauer, Saksena and Kutzin (2019)", does not merely
print the wrong name: Mathauer 2019 is `10.1186/s12939-019-1088-x`, **gold OA with
a PDF**, so the mis-split costs a FETCHABLE source and leaves its sentence
unchecked. It is the 13th of the 13 fetchable works and the only one that cannot
be reached. Same defect, strictly worse consequence than the one it was filed as.

### D134 — the button says what it is about to do, and the privacy sentence has one definition

§11 D133 made a fetched staged source resolve as checkable. This is the surface
that lets a researcher cause the fetch, and it is deliberately a BUTTON rather
than the full pre-flight card: the measurement comes before the design, same
order as everything else. A button that produces a number beats a card that
produces a design review.

```text
[ Fetch 2 sources this manuscript cites ]
Sends these 2 sources' DOIs to Unpaywall and OpenAlex — nothing else leaves your machine.
```

- **The count is from staged entries carrying a DOI and no document yet**, which
  is exactly what a DOI fetch can reach. Absent otherwise, on the standing rule
  that a button which can only fail is worse than no button (§11 D101).
- **Staging happens on plan; FETCHING DOES NOT.** An audit that silently reached
  out for a dozen PDFs because it parsed a reference list would be doing
  something nobody asked for. A test asserts `fetchOpenAccess` is not called
  after planning, and is called with `{kind: "staged", stagedId}` on the press —
  addressed as a staged subject, never as a library citation, because staging
  exists precisely so these never enter the user's collection.
- **Per-source outcomes, never a count.** "1 of 2 succeeded" hides the one the
  researcher has to do something about.

#### A SECOND COPY OF A PRIVACY CLAIM IS A SECOND THING TO KEEP TRUE

The disclosure already existed on the DocumentRow affordance and was correct
there. It is now `oaOutcome::oaFetchDisclosure(count)` — ONE definition, both
surfaces, with `count` changing only the grammar and never the claim.

It was tempting to paste the sentence. **This is the first surface on which Gaply
fetches works the user never added**, which makes it precisely the wrong sentence
to have two of: the day one copy is edited and the other is not, the product is
making two different promises about what leaves the machine, and only one of them
is being checked.

Same instinct as §11 D129 and §11 D132's shared parser, applied to a claim rather
than to a parser — and a privacy claim is the one where drift is least
acceptable.

#### Defending against a missing bridge method would have masked real wiring bugs

The screen's staged-source read threw in every test whose mock predated
`jobStagedSources`. The cheap fix is a `typeof bridge.jobStagedSources ===
'function'` guard; the right fix was to add the method to the shared mock
factory, defaulting to `async () => []` so the button is absent unless a test
opts in.

A guard there would make a genuinely unwired bridge look like a screen with
nothing to fetch — **indistinguishable from working**. That is the same call as
`label-cn` refusing an unknown flag rather than ignoring it (§11 D130's
neighbour): silence is the worst possible response to an instruction that cannot
be honoured, because it cannot be told apart from the instruction having worked.


### D135 — the re-check takes subjects, and reads the reference list back out of the staged rows

§11 D134's button fetches the manuscript's own sources. A press has to LEAD
somewhere: the sentences citing a newly-fetched source need re-judging, and
re-running the whole audit would re-judge sentences whose answers have not
changed and overwrite verdicts already read.

`recheckItems` took `citation_ids: Vec<String>`. A staged manuscript reference has
no library id, so the screen pushed `r.citationId` — `undefined` for a staged
subject — and staged sources were **silently dropped from the re-check**. It now
takes `subjects: Vec<SourceRef>`, the same union the fetch takes, so the two
cannot disagree about which sources a press was about. `nowCheckable` holds
subjects rather than ids for the same reason, deduped by `SourceRef::key` because
a `Set` of objects would not notice a repeat.

#### THE STAGED ROWS *ARE* THE REFERENCE LIST, PERSISTED

Resolution finds a staged document by the DOI its reference entry prints
(§11 D133). A re-check has a job id and **not the manuscript's path** — the
export's own comment records that a job does not carry one — so it cannot
re-parse the bibliography.

It does not need to. `surname`, `year` and `doi` are exactly what an
`AuthorYearEntry` carries, so `staged_sources::as_author_year_entries` reads the
list back out of the table and resolution runs through the SAME single path:
marker → entry → DOI → document.

The alternative was a second lookup — marker → staged row by surname+year within
the job — which would have worked and would have been the second way to find a
staged document. One more pair of things to keep agreeing, for no capability that
the first path lacks. §11 D129 is the standing reason to refuse that trade.

A side benefit worth naming: because the list is persisted, any later consumer
with a job id can resolve markers without the manuscript file. The audit's own
export has wanted that for some time.

#### What the re-check now asks

For each unverifiable item, whether its marker resolves to something checkable
AND whether that source is one of the ones just fetched. Asked of the STORE, not
taken from the fetch's report: a fetch that succeeded and an item that can now be
checked are different facts, and the narrowing stops an unrelated item that was
already checkable from being requeued.

#### THE THREE-STEP SHAPE, and what a zero would mean — WRITTEN BEFORE THE RUN

Resolution is a LOOKUP, not a fetch trigger. At plan time nothing has been
fetched, so **the first report cannot show a checkable staged source.** The loop
is:

```text
1. run the audit      -> stages the reference list; every cited sentence is unverifiable
2. press the button   -> fetches the staged sources that carry a DOI
3. press re-check     -> the sentences citing them are re-judged
```

**The measurement is step 3**, not step 1. Reading step 1 as a failure would be
misreading the design.

**AND STEP 3 SHOWING `0 requeued` DESPITE SUCCESSFUL FETCHES WOULD BE A REAL
FINDING, NOT THE EXPECTED SHAPE.** This is recorded before the run precisely so a
zero cannot be rationalised afterwards. If fetches report `fetched` and the
re-check requeues nothing, one of these is wrong and the run has found it:

- the DOI in the staged row does not normalise to the DOI resolution looks up;
- the marker's surname/year does not match the staged entry's, so `doi_for`
  returns `None` — §11 D131's mis-split extraction doing this on purpose is
  already known to cost one of the 13 fetchable works;
- the fetched document has chunks but no embeddings, so `checkable_document_for_doi`
  correctly refuses it and the UI said "fetched" about something not yet usable;
- the staged row's `document_id` was never written, i.e. the link step did not run.

Each is a different defect with a different fix, and the honest response to a zero
is to say which — not to conclude that a quarter of the paper's cited claims were
never reachable.

The predicted number is **11 of 47 cited sentences**, from 13 fetchable DOIs
(§11 D133). Anything materially below that, with fetches succeeding, is a defect
and not a disappointment.


### D136 — two blind spots that made a live failure leave no evidence, both self-inflicted

The first live run of §11 D134's button produced nothing. Diagnosis established
quickly that the staging half was correct — migration 20 applied, 35 staged rows
for job 20, 26 carrying a DOI, `list_for_job` returning exactly that against the
real database with correct camelCase — and then **stalled, because the two places
that should have recorded what happened recorded nothing.**

#### 1. A span with no event inside it writes nothing

`ai_job_staged_sources` carried `#[tracing::instrument]`, which opens a SPAN.
A span with no event inside it produces no log line, so **a successful call and a
call that never happened were identical in the log.** The diagnosis asserted the
command had not been invoked; that assertion was unfounded and was withdrawn.

It now emits an event, with `fetchable` beside the total — because that count is
what decides whether the button appears at all, and "35 rows, 0 fetchable" is a
different situation from "no rows".

#### 2. `.catch(() => setStaged([]))`, commented "the honest degradation"

It was not honest. It was SILENT. A rejected read and a manuscript with nothing to
fetch rendered the identical screen: no button, no message, no log line. So the
one failure mode the diagnosis most needed to rule out was the one that left no
trace.

The read now reports its error where the button would have been, naming the
reason through `errorText` (§11 D98's rule: never `[object Object]`). Two tests
hold the distinction open — the same screen under the two conditions, asserting
they are DISTINGUISHABLE, because that is the property that was missing rather
than either message.

#### What makes this worth an entry rather than a fix

**This is the session's own recurring lesson, committed by the person writing it
down.** §11 D130 rejected a rule whose failure mode was "the audit silently
checks nothing". `label-cn` was changed to refuse an unknown flag rather than
ignore it, on the grounds that *silence is the worst possible response to an
instruction that cannot be honoured, because it cannot be told apart from the
instruction having worked.* §11 D134 records that defending against a missing
bridge method would have masked real wiring bugs.

Then the very next surface shipped with a swallowed error and an unobservable
command — both written in the same session, hours after the notes arguing against
them. Knowing the rule is not the same as applying it at the moment the cheap
option is in front of you, and the cheap option here was four characters of
`catch`.

The standing form, for the next time: **an affordance that is absent must say
whether it is absent because there is nothing to do, or because something
failed.** Those are different facts and the reader cannot infer which.

### D137 — an error written to a state nothing renders is worse than no error handling

The first press of §11 D134's button did nothing visible. The fetch had in fact
failed before any network work — 0 documents created, app idle at 0% CPU — and
the handler did catch the error. It called `setFixNote`.

**`fixNote` renders only inside `{preview && stage === 'planned'}`** — the
pre-start card. The button is on the health card, where that card no longer
exists. So the reason was captured, stored, and displayed nowhere: no progress,
no outcomes, no error, indistinguishable from a press that did nothing.

This is §11 D136's class again — **written in the same handler, during the same
session, while fixing D136.** Three instances now: the swallowed read, the
unobservable command, and an error routed to a surface that is not mounted. The
third is the most deceptive, because the code looks correct at the call site: there
IS a catch, it DOES set a message.

The standing form is therefore narrower than D136's: **an error must be rendered
by the component that can fail, not handed to a sibling that may not be on
screen.** `stagedError` lives beside the button it describes.

#### The second invisibility in the same handler

`fetchOpenAccess` was called with no progress callback. A 26-source fetch runs for
minutes and the only sign was a disabled button, so a working run and a dead one
looked identical *on success too*. §11 D105 established exactly this for the
single-source fetch — the batch button had not learned it. Progress now renders
per source and per phase.

#### And `citation_fetch_oa` had no events inside its span

Same as §11 D136's command, one command over: `#[tracing::instrument]` alone
writes nothing. It now logs `oa fetch requested`, `targets built` (with
`with_doi`), `deps ready`, and one line per source with its outcome kind — and
each of the three `?` sites logs before propagating, so the last line printed
names the stage that failed.

That instrumentation is what produced the live result: **6 fetched, 6
abstract-only, 5 no-OA-copy, 4 failed, 1 not-importable.**

### D138 — an abstract is a real source and must not look like a full text

The measurement §11 D135 was written to protect, taken from the database rather
than the UI:

| | |
|---|---|
| unverifiable items in the job | 46 |
| **now resolve `Checkable` via a staged source** | **12** |
| backed by FULL TEXT | **5** |
| backed by an ABSTRACT only | **7** |

Not zero, so §11 D135's defect case did not occur: staged → fetched → linked →
embedded → resolves by DOI works end to end. 12 against a predicted ceiling of 11,
because abstract-only fetches also store a document and several works predicted
paywalled returned abstracts.

**Seven of the twelve rest on an abstract**, and that is the finding. The obvious
response — require full text — is wrong: an abstract legitimately carries some
claims ("GoEmotions reports 46% macro-F1" is in the abstract), so filtering would
drop seven real checks. The rule is the one this project keeps arriving at:

> **The weaker thing is allowed. It is not allowed to look like the stronger one.**

So `Resolution::Checkable` carries `abstract_only`, and the distinction travels to
the reader instead of into a filter:

- the **evidence item** says *"passages located IN THE ABSTRACT ONLY — the full
  text was not available"*, on the item, where a reader who lands on item 19 sees
  it rather than a section header they scrolled past;
- the **cover** reads *"passages quoted for 5, and for 1 from the abstract only"*
  rather than folding both into one number a reader would take as full-text
  verification;
- the **preview** counts `would_check_abstract_only` apart from `would_check`, so
  "a PDF Gaply can read" keeps meaning that;
- the **export** reads `abstractOnly` back out of the planner's payload. It had
  rebuilt `ReportItem` with the field defaulted to `false` — the distinction
  existing in the database and being dropped at the point of use, which is the
  same shape as §11 D129 and §11 D133.

### D139 — a completed audit was durable in the database and ephemeral on screen

Navigating away from a finished audit lost it. `jobId` was
`plan?.jobId ?? resumableJob?.jobId`: `plan` is local state from
`startThesisAudit` and dies with the screen, and `resumableJob` resumes
UNFINISHED work — a `done` job has nothing to resume. So the health card, the
flagged list, the staged-sources button and the re-check all became unreachable
while every byte needed to rebuild them sat in `ai_jobs`, `ai_job_items`,
`audit_staged_sources` and the job's stored consistency findings.

**It cost a real measurement.** A three-hour run completed, the fetch reported its
22 outcomes, and the Re-check press — the step §11 D135 names as THE measurement —
was gone. The number had to be recovered by querying the database directly. A job
whose results take hours to produce and seconds to lose is a defect, not a missing
nicety.

#### Built as REHYDRATION, not as a feature

`jobs::recent_jobs` plus one command; `ai_job_status` and `ai_job_results` already
served the contents. `ai_audit_jobs_recent` carries the staged counts because a
row of ids tells a reader nothing — the picker says *"#22 · done · 46/46 items ·
35 sources staged, 15 fetched"*, which is what decides whether a job is worth
reopening.

**THE TWO BUTTONS THAT WERE LOST ARE THE TWO THAT MATTER**, so both are asserted
reachable from a rehydrated job rather than only from a freshly planned one:

- the **staged fetch** needed nothing: its effect is keyed on `jobId`;
- the **re-check** needed real work. `nowCheckable` is normally filled from a
  fetch's own reports, which a reopened job does not have. It is now DERIVED FROM
  THE STORE — staged sources carrying a document, plus library ids named by
  unverifiable items. A generous candidate set is safe because `recheckItems`
  already re-asks the store and narrows ("a fetch that succeeded and an item that
  can now be checked are different facts"), so an empty requeue is a TRUE answer
  rather than a missing one.

A coupling found rather than designed: the re-check button lives inside the
"Cited, but not checkable" section, so it is unreachable when nothing is blocked.
That is honest — with nothing blocked there is nothing to re-check — but it is a
real constraint, and the first version of the test failed on it.

#### THE THREE LOSSES ARE STATED ON THE CARD, AND THAT NOTICE IS THE FEATURE WORKING

> Reopened from saved results. A job does not record which file it audited, so the
> export is named generically and the pre-run source list is unavailable. The
> original fetch's per-source reasons (paywalled, no free copy, failed) are not
> saved either — a source here reads as fetched or not fetched.

Not an apology: a reopened view that silently dropped state would be the same
shape as §11 D136, §11 D137 and §11 D138 — a distinction that exists and is
dropped at the point of use. Each loss was verified rather than assumed:

1. **No manuscript path.** See the gap below.
2. **No per-source fetch reasons.** Only the EFFECT is persisted (`document_id`
   set or not), so 4 failed, 5 no-OA-copy and 1 not-importable all read
   identically as "not fetched". The distinction existed in `FetchOutcome` and
   was never written down.
3. **No live progress**, which is inherent.

A test pins that notice, because an omission nobody states is exactly what this
week keeps producing.

#### GAP, NOT BUILT: a job does not record what it audited

`ai_jobs.document_id` is **NULL for every path-based run** — the manuscript is
opened by path, never registered as a `documents` row — so a job genuinely does
not know which file it audited. That is the root of loss (1): `manuscriptLabel`
falls back to `"manuscript"` and `previewThesisAudit(path)` cannot run.

**Recorded as its own gap because it will bite something else.** Anything that
wants to re-run, re-export under the right name, compare two audits of the same
manuscript, or answer "which paper was this?" needs it, and each will discover the
absence separately. The fix is small — persist the path, or register the
manuscript as a document — and is not part of rehydration.

#### `resumableJob` was declared and never set

`ThesisAuditPage` holds `const [resumable, setResumable] = useState(null)` and
**nothing ever calls `setResumable`**. The "Unfinished audit" card and its resume
handler have therefore been unreachable.

Discovered, not introduced — and it explains the shape of the original defect:
there was no resume path at all, rather than a resume path that failed to cover
finished jobs. Left alone deliberately: resuming an interrupted job and reopening a
finished one are different questions, and wiring the dead one while building the
other would have conflated them. Worth its own look.

### D140 — an abstract-backed failure says so, and both predictions about why were wrong

Job 23's re-check produced `12 citation support · 7 could not be judged`. The 7
are not a model quality problem in the way the number suggests: **6 of them were
checked against an abstract only**, and all 6 failed the same way —
`supporting_chunks: is empty for verdict 'weak'`. The model was shown a few
hundred words of abstract, found nothing quotable for the claim, and said so; the
validator then rejected the answer for carrying no quote.

`emit_finding` now states that, on the item, whenever `abstract_only` is set:

> Only an ABSTRACT was available for the cited work, not its full text. An
> abstract carries too little to quote for most claims, so this is a limit of
> what could be obtained rather than a judgement about the sentence.

The wording is load-bearing in one respect: it is a statement about the
**evidence obtained**, not about the sentence. A reader who sees "could not be
judged" with no explanation will attribute it to their own citation. The test
(`a_not_judged_item_says_when_only_an_abstract_was_available`) asserts the phrase
appears **exactly once** in a report carrying both an abstract-backed and a
full-text-backed item — a label that renders on every item would carry no
information, and asserting mere presence would not catch that.

#### Both hypotheses for a stronger rule failed, and the measurement is why

The proposal was to stop sending single-chunk abstracts at all, on either of two
rules. Both were tested against the real rows and neither survives:

- **Gate on `abstract_only`.** It would have cost a real check. 1 of the 7
  abstract-backed items came back with a usable passage. A rule that suppresses
  a lane producing a 1-in-7 yield is discarding evidence, not noise.
- **Gate on chunk count.** Chunk count does not predict failure — the rate is
  non-monotonic across the corpus: 1 chunk 86%, 36 chunks 0%, 61 chunks 50%,
  113 chunks 67%, 116 chunks 0%, 150 chunks 82%. There is no threshold to set.

So the label ships and the gate does not. The misleading part of `7 could not be
judged` was that it read as a verdict; that is now fixed at the point the reader
sees it, which is cheaper and more honest than suppressing the attempt. n = 7 is
too thin to justify either structural change.

#### The 150-chunk failures are NOT D69 recurring

Checked explicitly, because `prompt is 3246 tokens` in the corpus reads exactly
like the case D69 fixed, and a context overflow reappearing in practice would
mean D69's invariant test was guarding constants rather than prompts. It is not
happening, on three independent pieces of evidence:

- **The one overflow row predates the fix.** `job 9 / seq 25` is the only
  `prompt is` row in the database and its own text says *"the configured context
  is 4096"* — the pre-D69 value. Job 9: `2026-09-04 16:53:36`. The commit that
  raised `TASK_N_CTX` to 5120 (e8177d3): `2026-09-04 18:27:56`. It is the run
  that motivated D69.
- **The other 22 failures prove their retry fit.** They carry `failed validation
  twice`, i.e. `TaskError::ValidationFailed`, a variant that holds `retry_raw`.
  Reaching it requires the retry prompt to have been built AND generated. An
  over-ceiling prompt produces `TaskError::Generation` before any token. So
  every one of the 22 retries was inside 5120; the model replied with non-JSON.
- **Document chunk count cannot drive prompt size.** `assemble` requests
  `RETRIEVAL_K = 12` and trims to `evidence_budget_words(1200) = 920` words, so
  a 150-chunk and a 6-chunk document present the same top-12. The single
  unconditional admission — the first chunk, added before the budget test — is
  bounded independently: chunking caps at 512 words and 0 of 59 stored chunks
  exceed 920.

#### GAP (recorded, not fixed): the retry guard is blind to the evidence budget

The structural suspicion was right, in a different place than predicted.
`a_retryable_prompt_fits_the_configured_context` compares the ceiling
`5120 - 2*1024 - 67 = 3005` against `const LARGEST_REAL_PROMPT: usize = 2155` —
a number typed in from one measurement. It builds no prompt and tokenizes
nothing. `max_retryable_prompt_tokens` has **exactly one caller: that test**;
nothing in production consults the ceiling.

So the guard pins the two constants in its own formula and is blind to the third
input to a real prompt, `EVIDENCE_BUDGET_TOKENS`. That constant is not stable by
intent — D14's own text invites raising it ("raise it against measured prefill
once Metal lands", naming the spec's implied ~2500). Taking that invitation puts
evidence near 2500 tokens against a measured scaffold of ~765 (2155 - 1390), so
past the ceiling: D69's exact failure, reappearing in a user's audit with the
invariant test green. D14 points a future reader at spending most of the headroom.

**Arithmetic corrected by D142, which measured it.** This entry first put the
ceiling at 3009 and the headroom at 854. The ceiling is **3005**
(`5120 - 2048 - 67`), and the headroom against a real budget-filling prompt is
**779**, not 854 — 854 came from the hand-typed 2155, which is the constant the
gap is about. The estimate of where a 2500-token budget would land (~3265) was
also low: measured, it is **3754**. An entry that reasons from the number it is
criticising inherits its error.

Left as a gap rather than fixed here, to keep this cell about the abstract label.
The fix is for the guard to assemble a real budget-filling bundle, build the
prompt, tokenize it and compare THAT to the ceiling — a check on the prompt
rather than on the constants — and it needs its own cell.

### D141 — a job that cannot say what it audited

`ai_jobs.document_id` is NULL for every whole-manuscript audit ever run, and that
is correct by design: `plan_audit` takes `manuscript: &Path` and deliberately
creates no `documents` row, because auditing a file must not enter it into the
user's own corpus — the rule D132 applied to the bibliography. But it left the
job with no record of its subject at all, and two visible losses followed:

- the export fell back to the literal string `manuscript`, so a reopened job
  produced `manuscript-citation-audit.pdf`;
- the pre-run source list could not be recomputed, because — as D94 had already
  noted — the export has the manuscript's NAME and not its path.

Migration v21 adds `source_path TEXT`, `plan_audit` writes it, `JobRow` carries
it, `ai_audit_jobs_recent` returns it, and the past-audits picker prints the file
name beside each job. **Nullable and never backfilled**: the 23 existing jobs
genuinely do not record what they audited, and writing a plausible guess would
make an unknown look like a fact.

The test asserts the PERSISTED row, not the call. A path handed to `create_job`
and dropped by the INSERT would satisfy a mock and fail a user — and the column
list in that INSERT is exactly where such a thing goes wrong. It also asserts
`document_id` is still NULL, because the cheap way to make a path available would
have been to create a document row, which is the thing that must not happen.

#### The disclosure shrank to fit what is still true

D139's reopened-job note said a job "does not record which file it audited, so
the export is named generically". That sentence is now false for any job created
after this change, and a stale disclosure is its own defect — it teaches a
researcher to distrust output that is in fact fine. The note is conditional: a job
with a path says *"Audited R PAPER.docx"*, a pre-column job still says the export
is named generically, and both still state the two losses that remain (the
pre-run source list is not stored, and the original fetch's per-source reasons are
not saved). A second test pins the path-present case and asserts the
generic-naming caveat is ABSENT, because an assertion that only checks the text
appears cannot catch a caveat that has outlived its cause.

#### CONSTRAINT: the app crate cannot be tested in CI, and the bundled model is why

Recorded here because D142's guard lives in the app crate, so this is what bounds
its enforcement: **it runs locally only.** Checked rather than assumed.

`tauri-build` validates every path in `tauri.conf.json`'s `bundle.resources` at
BUILD time, not at bundle time. `bundled-models/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf`
(~400 MB) and `bundled-models/tokenizer.json` (~11 MB) are listed there and are
gitignored by design, so on a clean checkout `cargo test -p app` dies in the build
script:

```
resource path `bundled-models/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf` doesn't exist
process didn't exit successfully: build-script-build (exit status: 1)
```

Not a link error and not a failing test — it never compiles. **D124's keychain fix
is not the blocker and does not unlock this.** It did remove a different obstacle
(`GAPLY_SKIP_KEYCHAIN` names "a CI shell" explicitly), so the hang is no longer
what stands in the way; the resource validation is, and it is upstream of
everything D124 touched.

What running it would actually take, in increasing cost:

1. **Zero-byte placeholders** at both paths before `cargo test -p app`.
   `windows-build-check.yml` rejects placeholders, but for the BUNDLE, and for a
   stated reason — it "would upload a real, installable MSI/NSIS containing a
   model that cannot work … because the artifact looks legitimate". That objection
   is about a shipped artifact and does not apply to a test job that bundles and
   uploads nothing. The guard would then take its estimate path, which is the
   branch built for exactly this case.
2. **Linux webview dev packages** (webkit2gtk/libsoup/javascriptcore) for the app
   crate to link. `clean-checkout.yml` deliberately installs none of this — "no
   Node, no Tauri prerequisites" is its whole design, and it is the per-push
   guard precisely because it is cheap.
3. **A full app-crate compile including candle**, which is the expensive part.

And the Windows lane is closed regardless: the app test binary dies at load with
`STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139) before the harness runs, which is why
that workflow runs `cargo test -p gaply_core` and not `--workspace`.

So: possible, not cheap. Not done, and the constraint is written here instead so
the next person does not rediscover it — **the only enforcement of D142's guard is
the local pre-commit `cargo test --workspace` gate.** If the app crate is ever
wanted in CI, step 1 is the cheap part and step 2 is the decision.

##### The measurement nearly came out backwards

`cargo check -p app --all-targets` with the models hidden **succeeded**, and that
was one step away from being written up as proof that the model is not the
blocker. It had reused a CACHED build-script result: the `.gguf` appears in no
`rerun-if-changed`, so removing it does not invalidate the cache. Only forcing the
script to actually run showed the failure. A CI checkout has no cache, so the
cached answer was the wrong one to generalise from — and the conclusion it
supported was the opposite of the truth.

### D142 — the retry guard measured its own constants; now it measures a prompt

D140 recorded this as a gap; this is the fix.

`a_retryable_prompt_fits_the_configured_context` compared the ceiling
`TASK_N_CTX - 2*MAX_TOKENS - RETRY_OVERHEAD_TOKENS` against
`const LARGEST_REAL_PROMPT: usize = 2155` — a number typed in from one
measurement. It built no prompt and tokenized nothing, and
`max_retryable_prompt_tokens` had exactly one caller: that test. So it pinned the
two constants appearing in its own formula and was blind to the third input to a
real prompt, `EVIDENCE_BUDGET_TOKENS` — the one D14 explicitly invites moving.

`a_budget_filling_prompt_can_still_be_retried` now fills an evidence bundle to
what the budget permits, builds the prompt the model would receive, tokenizes
THAT, and asserts it against the ceiling.

**Proven, not argued.** With `EVIDENCE_BUDGET_TOKENS` temporarily set to the 2500
D14 names, the guard fails: `3754 tokens against a ceiling of 3005`. The old
guard passed that configuration silently. 3754 also exceeds `TASK_N_CTX -
max_tokens` (3072), so the FIRST attempt would not have fit either.

#### Three things the measurement changed about what I believed

- **The ceiling is 3005, not 3009.** `5120 - 2048 - 67`. D140 said 3009 and its
  headroom figure inherited the error; both are corrected there.
- **Filling to the budget in WORDS is not the worst case.** 920 words of the
  committed fixture gives a 1972-token prompt — *below* the 2155 D69 already
  measured from real OpenAlex sources. A guard built that way would have been
  measuring the fixture's token density, which is the exact failure D69 recorded
  about the synthetic seeds. The target is therefore a TOKEN count: the 920
  permitted words priced at the measured real density of 1.511 tokens/word, with
  an assertion that the synthetic prompt is **not smaller** than the largest real
  one observed. Measured result: 2226 tokens, 779 of headroom.
- **Growing by whole 512-word chunks overshot the target by ~700 tokens**, which
  would fail configurations that are actually safe. That is the opposite error and
  just as misleading, so the bundle grows in 32-word steps and stops at the
  target.

#### Where this guard runs, stated exactly — because it is NOT CI

Checked rather than assumed, and the answer is narrower than it first looked:
both workflows run `cargo test -p gaply_core` only (`windows-build-check.yml`
says so deliberately, and `clean-checkout.yml` skips the app crate because the
bundled model is gitignored). `citation_support` is an APP-crate module — it has
to be, the candle deps live there — so **this guard never runs in CI**. It runs
on the local pre-commit `cargo test --workspace` gate, which CLAUDE.md makes the
standing requirement, and it cannot be moved to `gaply_core` without moving the
task there.

Saying so is the point. A guard whose reach is overstated is the same defect class
as a guard that measures constants: the protection is believed to be somewhere it
is not. Raising `EVIDENCE_BUDGET_TOKENS` is caught by the pre-commit gate, not by
a PR check.

The CAUSE — `tauri-build` validating the gitignored bundled-model resource paths
at build time, so `cargo test -p app` cannot compile on a clean checkout at all —
and what lifting it would cost are recorded at the end of D141.

The estimate path therefore serves the no-tokenizer case that remains: a fresh
clone, or any machine without the ~400 MB bundled model, where the tokenizer is
absent and a silently-skipping guard would be the same nothing the old one was.
It prices evidence and scaffold
SEPARATELY, at 1.511 and 2.174 tokens per word: D17 measured the JSON schema block
as tokenizing far more densely than prose, and pricing scaffold words at the prose
rate under-counts it by a third. Measured, the estimate comes out at 2514 against
the tokenizer's 2226 — it **over**-states by 13%, which is the safe direction: a
tokenizer-less checkout cannot pass a prompt the real tokenizer would reject. Both
paths fail at a 2500 budget (4029 estimated, 3754 measured).

`GAPLY_FORCE_TOKEN_ESTIMATE=1` runs the estimate on a machine that HAS the
tokenizer, so the fallback branch is not the one branch nothing ever exercises. The tokenizer is also loaded once through a `OnceLock`: re-reading it
per step made this single test take 18 s against a whole-workspace suite of ~18 s.

### D143 — the audit report redesigned to a professional standard

Six defects, reported from a real generated report, fixed together because they
share a cause: the composer had no way to say "these things are alike", so every
row carried its own copy of what was common to all of them.

#### 1. A reason shared by every row is a heading, not a column value

One caveat ran to 34 words and appeared on ~20 consecutive rows. `hoist_shared_tail`
finds the longest tail every row shares, at a word boundary, and states it once
above the table. It is generic rather than matched to that one sentence, so the
next repeated caveat is handled too, and it proved that immediately: the same
mechanism now collapses six identical abstract-only explanations and seven
identical validation reasons in the "Not judged" section.

The row then needs a NAME, and the first attempt still printed the whole reason,
because an unmatched author-year citation has no parsed reference entry and the
grouping key falls back to the reason. `cited_work_label` prefers the parsed
entry, then the marker the reason quotes (`AlJohani, 2024`), then the trimmed
reason.

**A hoisted caveat must still be attachable to a row.** D140 put the abstract
sentence ON the item deliberately. Hoisting it wholesale would have undone that,
so the section states it once and each item keeps a six-word marker, and a single
abstract-backed item keeps the full sentence (there is nothing to hoist). The
test asserts all three cases.

#### 2, 3 and 5. Tables, charts, and how they are laid out

Three new `Block` variants: `Table` (caption, header, rows, per-column `Align`),
`StackedBar`, `BarChart`. The module docs record dropping `ChecklistTable`
because a renderer receiving it had to answer *what does a finding look like*.
That objection was about DOMAIN semantics and still stands; these carry none, so
the renderer decides every visual question and the composer none, exactly as
`Bar` and `Badge` already split.

Layout decisions that came from LOOKING at the output, not from the spec:

- **Column widths from content.** An even split gave a column of paragraph
  numbers the same width as a column of whole sentences. Flex columns now share
  space in proportion to what they need, and a short label column
  ("Hadley, 2002") is treated as fixed, because squeezing it wraps a name onto
  two lines and buys the long column almost nothing.
- **A table paginates per ROW and repeats its header.** The first version decided
  the re-emit before measuring the row's height, which put the repeated header
  UNDER the first row of the continuation page: worse than no header.
- **Justification is `Tw` word spacing with the last line left alone**, capped at
  2.6pt per gap; beyond that a ragged edge beats a river. Bullets are U+2022,
  hung in the margin.

#### 4. No em dashes, and the guard that first failed to notice

An em dash stands in for a colon, a comma, a parenthesis or a full stop without
saying which, and this report is read by researchers outside computing.

**The first guard passed vacuously on a file visibly carrying seven em dashes.**
It has its own entry, D144: it is the sharpest instance of this session's whole
pattern and it happened inside the fix for that pattern.

**And a fixture only reaches the strings its own data triggers.** Six dashes in
`consistency` survived the rendered-output test and appeared in the regenerated
report, because those findings are built from a real manuscript's structure. So
a second guard reads the modules that own reader-facing strings and fails on a
literal em dash in any non-comment line. Its own first version reported three
TRAILING comments as defects, which is why it now strips everything after `//`.

**Prompt strings were deliberately NOT changed.** Three em dashes live in text
sent to the model, not to the reader. D121 forbids prompt edits, and altering
one to satisfy a typographic rule would change measured behaviour to fix
something no reader sees.

#### 6. Truncation at a word boundary, and one rule for it

`"Health financing for universal …"` came from `chars().take(n)`. `trim_at_word`
keeps whole words and refuses a cut so early that only a fragment survives.
It lives in `report_compose` and BOTH callers use it: `consistency` had its own
`snippet` with the same bug, and fixing only one would have left two truncation
rules to drift apart.

Where a table cell can wrap, nothing is truncated at all: the sentences a reader
needs in order to find their own text in the manuscript are printed in full.

#### What a regenerated report can and cannot show

Consistency findings live in `ai_jobs.summary_json` and item reasons in
`ai_job_items.error`, both written at audit time. **A re-render of an old job
replays the wording that shipped with it**, which is correct and is also why the
first regenerated preview still showed the fixed strings: 7 rows of job 23 carry
an em dash in the database, and the report renders 7. The preview example takes
an optional manuscript path and recomputes the consistency findings, so the
fixed output can be seen without re-running a three-hour audit.

Measured on job 23 (R PAPER .docx): 23 pages before the shared-caveat hoist,
**18 after**.

### D144 — a guard that asserted on a byte the renderer cannot emit

The sharpest instance of this session's pattern, and it happened INSIDE the fix
for that pattern.

D142 replaced a test that compared constants with one that measures a real
prompt. Two commits later, D143's rule was "no em dash reaches the reader", and
the test written to enforce it read:

```rust
let pdf = crate::report_pdf::render_pdf(&blocks);
assert!(!pdf.windows(1).any(|w| w == [0x97]), "em dash (WinAnsi 0x97) in rendered PDF");
```

An em dash IS WinAnsi `0x97`, so this looks exactly right. It cannot fail.
`escape_pdf` writes every byte above `0x7F` as a three-digit OCTAL ESCAPE, so a
PDF this codebase produces never contains the byte `0x97` whatever its text says.
The assertion was a search for something the renderer is incapable of emitting.

**It was not caught by reasoning about the code.** It was caught by opening the
generated file:

```
$ pdftotext audit-preview.pdf - | grep -c '—'
7
$ python3 -c "print(open('audit-preview.pdf','rb').read().count(bytes([0x97])))"
0
$ python3 -c "print(open('audit-preview.pdf','rb').read().count(b'\\227'))"
7
```

Seven em dashes in the report, zero occurrences of the byte the test looked for,
seven of the escape it did not. The guard was green the entire time.

The fix asserts both forms, and the escape is the one that does the work. But the
transferable part is not the fix:

- **An encoder sits between your string and the bytes on disk.** A test that
  asserts on output has to assert on what the OUTPUT PATH actually produces, not
  on the representation the source used. The byte `0x97` was true of the
  character and false of the file.
- **A test that cannot fail looks identical to a test that passes.** Nothing in a
  green run distinguishes them, which is why the same shape has now appeared
  three times in three days: D140's guard measured the configuration, D142's
  measured typed-in constants, and this one measured an impossible byte. The only
  thing that has reliably caught it is generating the artefact and looking at it.
- **Prefer a negative control.** A guard for "X must not appear" should be run
  once against output that DOES contain X, to prove it fails. That check costs a
  minute and is the only evidence that an assertion is connected to anything.

A companion guard now reads the modules that own reader-facing strings, because a
rendered-output test only reaches the strings its own fixture data triggers: six
em dashes in `consistency` survived it, since those findings are built from a
real manuscript's structure that no hand-written model reproduces.

#### An old export and a new one will disagree, and that is correct

Worth knowing before someone files it as a bug. Consistency findings are stored
in `ai_jobs.summary_json` and item reasons in `ai_job_items.error`, both written
when the audit RAN. The report renders what is stored.

So a reader comparing a report exported last week against one exported today,
for the same job and the same finding, will see **different wording for the same
fact**: the old one says `... does not contain — no entry begins with ...`, the
new one says `... does not contain: no entry begins with ...`. Nothing was
re-judged and no finding changed. Re-rendering an old job faithfully replays the
wording that shipped with it, which is the behaviour you want from a record: a
report that silently rewrote its own history would be worse.

It is also why the first regenerated preview still showed em dashes after the fix
was correct and complete. Exactly 7 rows of job 23 carry one in the database, and
the report printed 7. `audit_report_preview` takes an optional manuscript path
and recomputes the consistency findings for exactly this reason, so the current
wording can be seen without re-running a three-hour audit.

### D145 — the report's arithmetic contradicted its own chart, and its shopping list contained things that are not works

Four defects from a page-by-page read of a real generated report (D143's
redesign). The first two are one failure; the second two are another.

#### The numbers a researcher would quote to a supervisor

"The counts" opened with: *"Gaply read 46 sentences. **39 of them cite a source
and were checked against it.**"* Five were. `checked` counts every item that
RETURNED AN ANSWER, and 34 of those answers were "I cannot reach this source", so
printing it as a checked count claimed eight times the work that happened, in the
section named for counting, one page after a chart saying 5 (11%).

The cover had the same shape from the other direction: *"5 cited claims. Source
passages quoted for 4…"* on a manuscript with 46 cited claims. Page 1 is the page
everyone reads, and it announced the successes while omitting the denominator.

One definition, used everywhere the number appears:

```
citing_total   = supported + unverifiable + failed   // every sentence citing a source
reached_source = supported + failed                  // a check was attempted
supported                                            // a passage was quoted
```

The cover reads *"5 of 46 cited claims checked against their source"*, the counts
section names all three states, and the cover metadata lost "Sentences answered:
39" — the misleading figure itself — for "Cited source reached" and "Passages
quoted".

**`checked` is not renamed and not removed.** D74 named it "answered" on purpose
and the field means what it says; the defect was printing it under a label that
claimed something else.

#### A column that never varies, and a list that said to fetch things that are not works

"Status" held `Not in your library` on all twenty rows. A value shared by every
row is a heading — D143's own rule, violated one table over — and it occupied the
width the actionable column needed. It is stated once in the caption now, and the
column is **where each work is cited** (`¶36, ¶41, ¶256`).

Five of the twenty rows were not works: `RBV, 1991` (a theory acronym),
`Authority, 2025` (from "The Financial Services Authority"), `Weiner's, 2009` (a
possessive), `Dubai, 2006` (from a co-citation), and `Kutzins, 2013` two rows
below `Kutzin, 2013`. These are D131's marker-extraction artifacts arriving in a
table whose entire purpose is to say what to go and fetch.

They are a SECOND table now, "Names that may not be separate works", with the
reason per row, and "What to do" applies only to the first group. Separated
rather than deleted: some may be real, and silently dropping rows would hide the
same uncertainty more quietly.

**The strongest signal cost nothing: the report had already said so.** The
deterministic consistency section flags `uncertain-reference-match` and
`orphan-author-year-marker` for exactly these markers, so presenting them a page
later as confident fetch targets was the report contradicting itself. The
classifier reads `m.consistency` first, then falls back to two narrow shape rules
(an all-caps 2-5 letter token; a possessive ending), both of which only ever
downgrade a row to "check this".

**Matching is on the finding's SUBJECT, as a whole word.** The first version
matched anywhere in the message and flagged `Kutzin, 2013` — a real work —
because the finding about `Kutzins (2013)` quotes the entry it was matched to and
"Kutzin" is a substring of "Kutzins". Calling a researcher's genuine citation
junk is the one thing this classifier must never do.

Measured on job 23 with the audit's stored findings: 7 of 20 names moved, and the
two least obvious (`Saksena, 2019`, `Valletta, 2011`) were checked by hand and are
correct — both are middle authors of works listed under a different first author.

#### GAP: the same manuscript yields different findings from two entry points

Noticed while measuring the above, not fixed. The consistency findings STORED by
the audit include the author-year checks and produce 7 flags. Re-running the
checker on the same file through `prepass_blocks` reports *"reference style:
NUMBERED, the author-year checks do not apply to this paper"* and produces 2.

Two paths disagreeing about one document is D129's shape, and it matters here
because the artifact classifier's best signal is whichever set of findings it is
handed. Worth its own look before anything else relies on consistency findings
being a property of the manuscript rather than of the caller.

### D146 — one manuscript, two entry points, different consistency findings

Found while measuring D145's artifact classifier, and logged before anything else
comes to rely on it.

The consistency findings the AUDIT stored for job 23 include the author-year
checks, and yield 7 flagged names. Re-running the checker over the same file
through `prepass_blocks` reports *"reference style: NUMBERED, the author-year
checks (orphan (Author, Year), uncited entries, year mismatches) do not apply to
this paper"* and yields 2.

Same manuscript, same checker, different answers, because the two paths reach it
with a differently-parsed prepass and therefore disagree about the reference
style. This is D129's shape: two callers of one piece of logic getting different
results is a defect in the logic's inputs, not a preference.

It matters beyond cosmetics. D145's classifier takes its strongest signal from
those findings, so **the quality of the "names that may not be works" split
depends on which path produced them** — 7 names separated, or 2. Anything else
that treats a consistency finding as a property of the manuscript rather than of
the caller inherits the same problem.

Not fixed here: the fix is in the prepass the two paths share, and it needs its
own measurement of which determination is right for this paper.

### D147 — the quotes were retrieval chunks, and the model's pointer was being thrown away

The report quotes source text and tells the reader that reading it IS the check.
What it quoted was the raw retrieval chunk: page 10 of a real report carried ~380
words in one block, cut mid-sentence at both ends, with a journal running header
inside the prose (`76 Health Services and Outcomes Research Methodology (2025)
25:57-84`) and PDF hyphenation debris (`regres- sion`, `mul- tiple`,
`Govern- ment`). The tool had delegated its own retrieval step to the reader.

#### What is NOT done, and why

The obvious fix is to trim each chunk to the sentences that matter. **Rejected.**
Selecting which sentences support a claim is re-deciding what supports the claim,
which is precisely the judgement this engine declines to let a model make
silently. Altering quoted evidence in a research-integrity tool is the failure the
tool exists to prevent.

The honest version is a DISPLAY change. `supporting_chunks[].why` already carries
the model's own statement of what in each chunk carries the point, and **the
export was discarding it**. It now leads the evidence, labelled as the model's,
with the chunk underneath as the context it is. No selection, no reordering, no
shortening.

A verbatim quote would be better still, and needs no new judgement: v2 asks the
model for one. But **all 46 supporting chunks in job 23 carry `why` and none
carry a quote**, because the shipped task is v1.6. Switching tasks is a separate
decision with its own measurement.

#### Every edit is visible or disclosed

* An elided run is replaced by `[...]`, never removed silently, and the note
  beside the passages says what the marker means.
* De-hyphenation cannot be marked inline without making the quote unreadable, so
  the standing disclosure states it: the passages are machine-extracted from a
  PDF, words broken across a line break have been rejoined, and a reader
  intending to rely on a quote should check it against the source.

#### The de-hyphenation rule was chosen by measurement, and my first two proposals lost

Run over the real evidence corpus (`ai_chunks`: 1,276 chunks, 131k words, 1,623
candidates):

| rule | joins | left alone | verdict |
|---|---|---|---|
| join unless the second fragment is a conjunction | 1596 | 15 | **SHIPPED** |
| also keep the hyphen when the first fragment is a prefix | 1454 | 157 | rejected |
| join only when a fragment is not a standalone word | 832 | 779 | rejected |

The **vocabulary** rule is the one I proposed and it fails on its own output: it
leaves 779 genuine breaks unfixed (`ef- fect`, `be- tween`, `How- ever`). Its
vocabulary is built from the same corpus, and the trailing fragments of broken
words (`tion`, `sion`) follow a space, so they enter the vocabulary as standalone
words and teach the rule that the break is legitimate.

The **prefix** refinement was rejected by inspecting its 142 cases: most are
ordinary words whose first syllable merely looks like a prefix, so it produced
`de-scribed`, `Pro-ceedings`, `multi-ple` and `Co-hen`. It would corrupt about
110 correct joins to rescue about 30 real compounds.

The shipped rule's 15 exclusions are every suspended hyphen in the corpus and all
are genuine (`low- and`, `inter- and`, `low- or`, `closed- and`). A 40-case random
sample of joins was correct in 38. **Known residual error, ~2%:** a genuinely
hyphenated compound that breaks at its own hyphen loses it
(`intra- annotator` -> `intraannotator`). Readable, and covered by the disclosure.

#### Three measurements nearly published the wrong answer

Worth recording, because each looked conclusive:

* `pdftotext -layout` drops the letter `f`, so the first read of the report showed
  `quoted or 4`, `indings`, `unveriiable`. That is the extractor, not the report.
* The first corpus measurement ran against `chunks` (59 rows). Evidence comes from
  `ai_chunks` (1,276 rows) via `store::chunks_by_ids`. The wrong table contained
  none of the phenomenon and would have concluded there was nothing to fix.
* Those defects appeared as `regres- sion` in the extracted text, so the rule was
  first written for `hyphen + space`. That space is inserted by `pdftotext` at a
  PDF line break; the stored text is what it is, and only reading the database
  directly settled it.

### D148 — a finding about a subsection that does not exist, and a severity that told the reader to ignore it

The two remaining defects from the design review that change what a reader DOES.
Both are in the deterministic consistency lane, which is the half of the report
that carries no error rate, so a defect here costs more than one in the model's
half.

#### A reference entry is not a subsection heading

The shipped report carried:

> Subsection "D. Demszky et al., "GoEmotions: A dataset ..." follows
> "A. Vaswani et al., "Attention is all you ...": the letters do not advance by
> one.

There is no such subsection. `section_letter_re` is `^\s*([A-H])\.\s+(\S.*)$`,
which is the right shape for "A. Dataset Acquisition" and also the exact shape of
an IEEE reference entry, because those begin with the first author's INITIAL.
Word strips the auto-number from the list, so the entry reaches the checker as
`A. Vaswani et al., "Attention is all you need"` with nothing to distinguish it.

The fix is the line the prepass already draws: `check_section_letters` now stops
at `is_references_heading`, because everything past it is a bibliography. The
prepass documents that rule for its own scan ("Stops at the references heading.
Everything past it is a bibliography"); this check is now the second caller to
respect it rather than the one that ignored it, which is the D129 lesson applied
before it cost anything further.

Measured on the real manuscript: the phantom finding is gone and the other seven
findings are unchanged, so the fix removed the false positive and nothing else.
The genuine check still fires on a genuine out-of-sequence run, and a new test
feeds it a reference list to prove the two are told apart.

**Why a false positive here is worse than a miss.** A reader who is told a
subsection is misnumbered goes looking for it. Not finding it, the reasonable
conclusion is that the tool is wrong about the manuscript, and that conclusion
costs the other six findings on the same page their credibility.

#### "Affects the audit" and "No action needed." in the same row

D143 gave the consistency table a "What to do" column, filling a missing action
with "No action needed." That is right for a cosmetic finding and a contradiction
next to "Affects the audit": one half of the row says this could change what the
rest of the report says, and the other says to ignore it. It landed on the
`marker-resolves-to-malformed-entry` finding, which is the one reporting that a
citation may resolve to the wrong paper.

Fixed in both places, because either alone would leave the hole open:

* The finding now carries its real action. It is the consequence of the malformed
  entry reported above it, so the action is that entry's: *"Fix reference entry
  [6] first (reported above). Until it reads as a reference, this sentence's
  citation cannot be checked."*
* The report no longer renders a missing action as "No action needed." for a
  STRUCTURAL finding. A severity that asserts consequences implies an action, so
  the fallback is *"Resolve this before relying on the sections below."*, and the
  cosmetic wording is untouched.

A test asserts no row can pair the two, and that a cosmetic finding keeps the
honest "No action needed.".

### D149 — the report stops talking to itself, and tells the reader where it is going

Six of the design review's remaining items, in the order they cost a reader
something. None of them is a wrong number: D145 to D148 fixed those. These are
what is left once the arithmetic is right, and the largest of them is that half
the document arrived before the part with an action attached to it.

Measured on job 23 (`R PAPER .docx`, 46 cited sentences, 5 checked, 34 blocked):
19 pages before, 20 after.

#### The footer said PublishReady on all eighteen pages

`report_pdf` drew the literal strings `"PublishReady report"` and
`"PublishReady"` into every page's furniture. That is the name of a DIFFERENT
report this renderer also draws, and it appeared on every page of a document
whose cover says "Thesis citation audit".

The renderer cannot know what it is rendering, so it stopped guessing: the cover
now carries a `running_title` and the header prints that. It is not derivable
from the two fields already there, and the attempt is exactly what produced the
defect — the audit's `title` is the document and its `subtitle` is the
manuscript, and the PublishReady report fills those the other way round. The
FOOTER now reads "Gaply", which is true of every report this renderer draws.

`the_running_header_names_the_document_the_composer_composed` asserts both
halves, the second being the negative control: the PublishReady report still
says PublishReady. A fix that deletes a name everywhere passes the first
assertion and fails the second.

**`PublishReady` is one of `GAPLY_REPORT_MARKERS`**, the list that stops Gaply
importing its own report as a source. The audit keeps six of the other markers,
so the guard is unaffected, and the marker test already exempted this string
because the composer never emitted it.

#### Identifiers that meant something to the code and nothing to the reader

Four kinds, each individually defensible, and together a report that reads as a
tool describing itself:

* `c931 · p.1` on every quoted passage. That is the retrieval store's row id.
  The passages are now numbered within their item, which is the only job the
  label had: the model's pointer line above the quote says "in passage 2" and
  the quote says "Passage 2", so the pairing D147 introduced survives.
* `¶32`, on every item of a Word manuscript, never explained. Now "paragraph
  32". `~p.5` was the same defect carrying D95's approximate-page distinction on
  a bare tilde; it is now "about page 5", which says it in words.
* "citation support" and "unverifiable" as table cells — the planner's enum
  variants. "Unverifiable" reads as a verdict on the sentence when it means
  Gaply could not obtain the cited work. Now "Checked against the cited source"
  and "Cited source could not be read", via `check_kind_label`, which falls back
  to the old spelling for an unknown kind rather than dropping the row.
* "Counts rows, not findings" in a caption: a note to whoever maintains the two
  numbers, printed to someone reading about their own manuscript.
* `JUDGED BY qwen2.5-3b-instruct-q4km` and `PROMPT VERSION citation_support-v1.6`
  as the first two rows of page 1.

**The prompt version is the one identifier that stays**, and it is the exception
that shows the rule: it is stated once, at the end, under a heading that says
what it is for ("If you need to reproduce this run exactly"). On the cover it
was the second thing a researcher read.

`no_internal_identifier_reaches_the_reader` sweeps a rich report for all of
them at once, because they leaked one at a time from four different places.

#### A page number that did not say which document it was in

Page 2 carried "quotes them below with their page" four lines above "this
manuscript ... has no page numbering, so findings are located by sentence". Both
sentences were true. The first is about the CITED SOURCES, which are PDFs; the
second is about the manuscript, which is a .docx. Neither said so, so together
they read as the report contradicting itself.

Fixed by naming the document in both: "each with the page it appears on in the
work your sentence cites", and the note now ends "The page numbers on the quoted
passages below are the cited sources' own, and are unaffected."

The same confusion was in the code. `page_label(e.page, has_pages)` answered a
question about a SOURCE passage using the MANUSCRIPT's pagination flag, so a
cited PDF whose page was not recorded printed "no page numbers" — a statement
about the wrong document. `source_page_label` is now separate and says "page not
recorded".

#### Three tellings of one measurement

"14 of 14 outputs across two runs" appeared on pages 2, 3 and 4, about forty
words each time. Every telling was accurate. By the third it reads as the report
defending itself, which is D140's defect at report scope instead of item scope.

The long form stays where the absence is ASSERTED, under the "no score" badge on
page 1. The other two state the fact and point at it: *"Gaply does not grade how
well a passage supports a sentence. "At a glance" says why."* The test asserts
the measurement appears exactly once AND that both later sections still say
there is no grade — a reader who lands on page 9 must not be left to assume one.

#### A map at the front, and an ending

Eighteen pages with no contents page, stopping on the exporter's note about
accented characters.

`Block::Contents` carries no entries. The contents IS the set of level-1
headings, so a composer that listed them again would be keeping two lists in
agreement by hand; each renderer builds it from the headings that follow. The
PDF resolves real page numbers by **laying the document out twice** — a page
number is not knowable until the document is paginated, and paginating it needs
the contents to be there. The two passes lay out identically because an entry's
page number is a right-aligned run on the entry's own line, measured into place,
advancing nothing vertically: present or absent, every element has the same
height and lands on the same page. A `debug_assert` compares the two passes'
answers.

`the_contents_page_numbers_are_the_pages_the_sections_are_on` checks each
printed number against the page that section's heading actually landed on in the
written file, by parsing the content streams. It distinguishes the heading from
the contents entry naming it by FONT, since they are the same characters. A
contents page with confident wrong numbers is worse than none.

The ending is "How this report was produced": what ran where, which model, and
the prompt version. Its closing line is conditional ("If anything appears below
this line...") because the exporter's note is conditional, and a closing line
that promises a note which is not there is the same defect one line later.

#### The half with an action attached started on page 14

Pages 4 to 13 were the 5 checked claims. "Cited, but not checkable" — the 34
sentences whose cited work is missing, every one of which carries something to
do — began on page 14, after the reader had waded through ten pages of quoted
passages.

Swapped. Measured on job 23: the works to add now start on **page 4** rather
than page 14, and the passages run 9 to 18. Adding those works is the single
thing that would make the next run of this audit say more; the passages are what
the report is FOR, and they are also reference material read one item at a time.

#### "At blocks 220 and 221", and why fixing it reads as a bug

The same defect as the identifiers above, in `consistency`: `blocks` is the
argument name of the function that found it, and the number is a 0-based index
into the parsed document. A reader cannot count to it and cannot search for it.

It is now located the way every other finding is: **"at paragraphs 221 and
222"**, or by page where the format has one, `audit_report::locator`'s rule that
a page wins. The ordinal is `index + 1` because the pre-pass counts paragraphs
1-based over EVERY block, blanked ones included, precisely so its locators match
what a reader counts in their own document. The two numberings are therefore the
same numbering, and the test asserts that directly: it takes the paragraph the
pre-pass assigns a sentence in the repeated block and requires the consistency
finding to name the same one. Drift there would have one page of the report
placing two findings about the same paragraph two numbers apart.

**This changes what a NEW run says, and deliberately not what an old one
replays.** Consistency findings are written to `ai_jobs.summary_json` at audit
time, so a re-render reproduces the wording that shipped with that job. That is
the rule recorded under "What a regenerated report can and cannot show" above,
and it is correct: an exported report is a record of what the tool said, not a
live query.

The consequence is worth stating because it looks like a bug. **Two exports of
the same job, one from before this change and one from after, differ in the
consistency section and nowhere else, and neither is stale.** Measured on job
23: its stored summary carries 13 findings and none of them is a
repeated-paragraph, while recomputing from the manuscript today yields 8,
including the repeated paragraph in the new wording. A reader comparing the two
would reasonably conclude that something had been lost. The preview example
takes an optional manuscript path for exactly this reason, and prints how many
findings it recomputed so the two sets are never silently swapped.

#### What was deliberately left

Items 10, 11, 14, 15, 16, 17 and 19 of the review are untouched, as agreed.

### D150 — three readings of the final PDF, and one of them was not the defect it looked like

Three items found by reading the shipped artifact rather than the code, which is
where the last two sessions' defects have all come from.

#### "Affects the audit" was fixed; "No action needed." on a real defect was not

§11 D148 stopped a STRUCTURAL finding rendering as "No action needed.". The
cosmetic half of that fallback was left alone and was wrong five times over:

> Figure 6 is referred to 1 time but no caption defines it. | Worth fixing |
> **No action needed.**

A figure a reviewer is told to look at and cannot find is a real defect, and one
of the cheapest to fix. Every action-less finding in `consistency` now carries
one:

| finding | action |
|---|---|
| `figure-referenced-but-absent` | Add the caption, or remove the reference. |
| `section-letters-out-of-sequence` | Renumber the subsections so the letters run in order. |
| `uncertain-reference-match` (one-character difference) | Check which spelling is right, and make the marker and the entry agree. |
| `uncertain-reference-match` (name not the one the entry is listed under) | Check this is the work you meant, and cite it by the name its entry is listed under. |
| `metric-values-unattributed` | Check whether these describe the same thing. If they do, one of them is wrong; if they do not, say which is which. |

The last two matter more than "cosmetic" suggests: §11 D131 is a mis-split name
that cost a **fetchable** source its check.

**The guard is a sweep, not five assertions.** The defect was not in any one
check — it was that a missing action had a plausible-looking default, so nothing
ever made the absence visible. `every_finding_says_what_to_do_about_it` runs a
fixture wide enough to fire eight checks and fails on any finding whose action
is `None`. The "No action needed." fallback stays for a finding that genuinely
needs nothing, and is now unreachable from the shipped check set.

Same persistence rule as §11 D149: a job audited before this keeps the actions
(and the absences) it shipped with.

#### The chart drew one population and the table under it drew another

`emit_blocked_sources` built its bar chart from `groups` — before §11 D145's
split into works-to-fetch and names-that-may-not-be-works. So a name the report
had just decided not to present as fetchable could still be ranked in the chart
of what to go and find, on the same page, from the same data.

Now computed after the split and drawn from `real`. The negative control was run
the D144 way rather than assumed: with the chart pointed back at `groups` the
new test fails, and names the bar it should not have drawn.

```
the chart ranks a name the table below it says not to fetch:
  [("Authority, 2025", 2), ("Banerjee, 2021", 1), ("Hadley, 2002", 1)]
```

**The instance that prompted this was NOT this defect, and the difference is
worth recording.** `Authority, 2025` appears in the regenerated report's chart
ranked 9th — and also in the works table beside it, because in that render it is
not classified as an artifact at all. `artifact_reason`'s first and strongest
rule is "the deterministic checks already flagged this marker", and the
recomputed findings for today's manuscript contain no `uncertain-reference-match`
for it; the other two rules look for an acronym or a possessive, and "Authority"
is neither. Job 23's PERSISTED findings DO flag it, so a replayed render files it
under "Names that may not be separate works" and a recomputed one does not.

So the chart and the table agreed in the PDF that was read. The defect they
looked like they had was real, latent, and is now fixed; the thing actually
visible on that page is that the classifier has no signal for a capitalised
ordinary word cited as a surname. **That is left deliberately.** D145's rule is
conservative on purpose — calling a researcher's genuine citation junk is the
one thing it must never do — and widening it to catch "Authority" needs a signal
that does not also catch a real author called Bishop, Church or Price.

#### Three names for one thing, fifteen lines apart

The heading said "Cited, but not checkable", the prose "sources Gaply could not
read", the chart "missing sources". Three names invite a reader to look for
three populations, and on that page one of them really was a different
population, which is what made this worth fixing rather than tidying.

One vocabulary now, built on the heading, which is also a `GAPLY_REPORT_MARKERS`
entry and could not move: the prose says "works that could not be checked", the
chart is "Which of these works block the most sentences", and the counts table
says "Cited source could not be checked", matching the page-1 chart segment
"Could not be checked". `one_state_has_one_name_throughout_the_report` fails on
"could not read" or "missing sources" reaching any composed string.

### D151 — A STANDING PROPERTY: an exported report is a record, not a live query

Named here because it has now produced three different-looking defects in two
sessions, and each was investigated as if it were its own thing.

**The property.** Consistency findings are computed once, at audit time, and
written to `ai_jobs.summary_json` (item reasons likewise to `ai_job_items.error`).
The report RENDERS what is stored. So every part of a report derived from a
consistency finding is a record of what the tool said on the day the job ran, and
a re-render after the checks change will differ from it.

That is correct behaviour, not a cache to be invalidated. An exported report is
evidence a researcher may have acted on, quoted, or sent to a supervisor; a
document that silently re-decides its own contents when the tool is upgraded is
worse than one that is out of date, because nothing on the page says which
version of the tool wrote it.

**What has actually diverged, in order of discovery:**

| what differed | where | what it looked like |
|---|---|---|
| WORDING | §11 D143 | a fixed string still appearing in a regenerated preview |
| FINDINGS | §11 D149 | job 23 stores 13, a recompute yields 8, sharing none |
| CLASSIFICATION | §11 D150 | `Authority, 2025` a suspect name on one render, a work to fetch on the other |

One cause, three symptoms. The third is the one that misleads, because the
divergence is not in the finding itself: `artifact_reason` takes its strongest
signal FROM the consistency findings, so a report derived from them inherits the
divergence one step removed, in a table that never mentions them.

**Two distinct sources, and only one is benign.**

1. *Time.* The checks themselves changed between the audit and the re-render.
   Expected, and the property above is exactly right for it.
2. *Path.* §11 D146: the audit path and the recompute path reach the checker
   with a differently-parsed prepass and disagree about the manuscript's
   reference style, so the recompute skips the author-year checks that the audit
   ran. That is a DEFECT, still open, and it is why `Authority, 2025` has no
   `uncertain-reference-match` to be classified by today.

So a difference between two renders of one job is not self-explaining: it is
either the tool having moved on, or D146. **Do not read a recomputed report as
the corrected version of a stored one** until D146 is fixed — on a numbered-style
paper the recompute runs strictly fewer checks.

**What to do with it.** `audit_report_preview` takes an optional manuscript path
and prints how many findings it recomputed, so the two sets are never silently
swapped, and the number on that line is the first thing to compare when two
exports disagree. Anything new that derives a reader-facing claim from a
consistency finding inherits this property and should say so where it is built.

### D152 — the premium tier: what leaves, under what consent, and why PRIVACY was not made conditional

**This entry exists because §1's R4 requires it.** R4 states *"Nothing leaves the
machine"* and lists **exactly three** permitted network operations, then says the
list is CLOSED: *"a fourth needs its own decision record and its own argument,
not an appeal to these."* The PublishReady Premium design (`docs/publishready-
premium-architecture.md`) sends the manuscript to a cloud model. Its own answer —
*"that is not a relaxation of the claim; it is a different tier with a different
claim"* — is an argument, but R4 asks for it to be written down here, against the
rule it departs from, rather than in the document that wants the departure.

So: this is operation **#4**, and the list is closed again behind it.

| # | Operation | Outbound payload | Trigger |
|---|---|---|---|
| 4 | **Premium manuscript review** | the manuscript text, the analysis record, and structured findings — to gaply-proxy, which forwards to a cloud LLM | user consents explicitly, per manuscript, before upload |

#### Why it cannot be an appeal to #1–#3

Operations 1 and 2 send a pinned URL. Operation 3's whole justification is the
**ID-only invariant**: a DOI is a public identifier for a published work, not a
fact about the person holding it. Premium violates that invariant directly and
deliberately — it sends the unpublished work itself. There is no reading of #3
that stretches to cover it, and an appeal to "we already make network calls"
would be the exact move R4 names.

The argument for #4 is therefore a different one, and it is a consent argument
rather than a payload argument: **the user is told, in specific terms, what will
be sent and to what class of recipient, and the sending cannot happen without a
stored record that they were told.** That is weaker than #3's invariant — no
amount of consent makes a manuscript a public identifier — and it is why the
enforcement below is in the type system and the gate rather than in a checkbox.

#### The two gates — and why `check_privacy` was NOT made tier-aware

The obvious implementation is one gate with a tier parameter, PRIVACY skipped
when the tier is premium. **That was rejected, and the reason is the whole
decision.**

> `check_privacy`'s worth is that it takes no argument. *"No manuscript text
> crosses this boundary"* is a sentence that survives only while nothing can
> weaken it. A tier-aware version is a policy wearing a gate's name.

The moment PRIVACY grows an `if tier == Premium`, the free tier's central claim
is only as strong as whoever last edited that condition — and the condition would
sit in the one function whose entire value is that it has no conditions.

So there are two gates, each unconditional (`src-tauri/src/release_gate.rs`):

| gate | invariants | guards |
|---|---|---|
| `run_free_gate` | PRIVACY + PROVENANCE, SELECTION, COMPARISON, PERSISTENCE | the free route |
| `run_premium_gate` | **TIER** + the same four structural | the premium route |

PRIVACY is absent from the premium gate not because it was relaxed there, but
because manuscript text is what that tier exists to send. TIER is its
counterpart, and it is unconditional in the same way: every payload carrying
manuscript text carries a consent id, and every consent id resolves to a stored
row whose scope covers what was sent. Both directions.

`run_all` no longer exists. There is no gate that runs "all" the invariants,
because PRIVACY and TIER are mutually exclusive by construction — a payload that
satisfies one cannot satisfy the other. A single enumeration would have needed a
tier argument to skip one of them, which is the rejected design under a different
name.

#### THE PARTITION, AND THE ONE PAYLOAD THAT FALLS OUTSIDE IT

The property asked for was *"a payload passes exactly one gate; none passes both
or neither."* **The first half holds universally. The second half is false for
exactly one shape, and that shape is the one the design exists to reject:**

| payload | free | premium |
|---|---|---|
| no manuscript text, no consent id | **PASS** | fail (TIER) |
| no manuscript text, consent id | **PASS** | fail (TIER) |
| manuscript text + resolving consent id | fail (PRIVACY) | **PASS** |
| **manuscript text, NO consent id** | fail (PRIVACY) | fail (TIER) |

The fourth row passes neither gate. **That is not a crack in the partition; it is
the safety property**, and a formulation that made it pass something would be a
worse design that satisfied a nicer sentence. It is asserted as a REQUIRED
outcome (`manuscript_text_without_consent_passes_neither_gate`) rather than
tolerated as an exception.

Row two — a clean payload carrying a consent id — needed a rule to stay on one
side: **the premium route is for payloads that carry manuscript text.** Without
it that payload passes both gates and the two routes are overlapping sets rather
than a partition. The rule has an independent reason: a payload with nothing to
consent to belongs on the route where PRIVACY can vouch for it.

#### THE DRIFT THE PARTITION ACTUALLY RESTS ON

PRIVACY and TIER are **opposite verdicts on the same question**. If each had its
own detector for "does this payload carry manuscript text", a payload could read
as clean to one and text-bearing to the other, and a payload passing both gates
would exist with every test still green — because no test would be comparing the
two detectors.

There is therefore ONE definition, `manuscript_text_in`, and both invariants call
it. This is §11 D134's rule — *"a second copy of a privacy claim is a second
thing to keep true"* — applied to a PREDICATE rather than to a sentence.
`both_gates_key_on_one_definition_of_manuscript_text` is what holds it.

#### THE TESTS WERE WRITTEN BEFORE THE GATES, AND ALL THREE BREAKS WERE RUN

A partition assertion whose first run is green proves nothing about whether it
partitions (the standing rule from the lint-gate episode). So, in order:

1. The `partition` module was written against `run_free_gate`,
   `run_premium_gate`, `TIER`, `check_tier`, `manuscript_text_in` and
   `crate::consent` — none of which existed. **13 compile errors, naming every
   missing seam.** That is the negative control.
2. Gates built; 20/20 green.
3. Each load-bearing property then broken on purpose, with the predicted failure
   stated first:

| break | prediction | result |
|---|---|---|
| remove "premium requires manuscript text" | a clean payload with a consent id passes both | **RED** — *"clean, consent: passed BOTH gates — the routes are not disjoint"* |
| give TIER its own structural-only detector | TIER and PRIVACY disagree on the same payload | **RED** — *"text: TIER diverged from the shared predicate"* |
| make PRIVACY tier-aware (*"they consented, so it is not a leak"*) | the rejected design passes the free gate on a premium payload | **RED** — both partition tests |

The second break is the one worth keeping: `no_payload_passes_both_gates` stayed
**green** under it, and only the drift guard fired. Two tests, two different
failure directions; neither subsumes the other.

#### WHAT CONSENT RECORDS, AND THE ONE FIELD THE DESIGN GOT WRONG

`src-tauri/src/consent.rs`. `Tier::Premium` holds a `ConsentRecord` by
construction, so no value of `Tier` names the premium route without one.

**The design specified `provider: Provider // OpenAI, named`. The desktop cannot
honestly record that.** `gaply-proxy/app/main.py:146-152` selects the provider
SERVER-SIDE from `GAPLY_LLM_PROVIDER`, and `:262` states the property
deliberately: *"nothing is persisted. The desktop never knows which one handled
it."* A consent record signed on this machine naming a vendor would be a claim
the machine has no way to check and no way to be told is wrong — a privacy
statement that is unfalsifiable by the party making it.

So the field is `ProviderClass::CloudLlmViaProxy` — *a cloud LLM reached
exclusively through gaply-proxy* — which is true, checkable, and the thing the
user is actually deciding about. **Naming the vendor is available and not taken:**
it requires the proxy to return which provider answered, which means giving up
provider-blindness. That trade is recorded here so it is a decision rather than a
gap, and `the_provider_is_recorded_as_a_class_reached_through_the_proxy` fails if
a vendor variant is added without revisiting it.

Two smaller departures from the design, both toward existing convention:

- **It lives in the app crate, not `gaply-core`.** The design's `Uuid` and
  `DateTime` fields are two dependencies `gaply-core` does not carry; it is
  declared *"portable, Tauri-free"* at `rust-version = "1.77.2"` with no network
  and no clock. A consent record is a fact about a NETWORK boundary, so it sits
  beside the proxy client and the gate it constrains, and uses `now_epoch()`
  seconds as `i64` with a `String` id.
- **An unknown scope bit is rejected, not masked.** A bit outside
  `ConsentScope::ALL` was written by a newer version; masking it off turns
  *"consented to something we do not understand"* into *"consented to less"*, and
  guessing downward is still guessing.

#### WHAT IS NOT DONE HERE — **CLOSED, Phase 1 Part A**

This entry originally read: *"`ConsentRecord::from_persisted` is `pub` and there
is no `consent_records` table yet… that is the one part of this weaker than the
design requires."* It is now done, and the fix corrected a decision recorded
three paragraphs above it.

- **Migration 22** creates `consent_records`: append-only, one row per manuscript
  per consent event, no UNIQUE on `manuscript_id` (a second consent is a new row
  and the old one stays true about its moment), a CHECK pinning `scope` to the
  six defined bits and `provider_class` to the known vocabulary.
- **`consent::store`** is the only public path to a `ConsentRecord`.
  `from_persisted` is now **module-private** — not `pub`, not `pub(crate)` — and
  `store::record` / `store::resolve` are its only callers. A record that exists
  is a row that exists, by visibility rather than by discipline.
- **`check_tier` takes the real resolver.** The partition tests no longer
  fabricate consents: `ConsentRecord::for_test` is `pub(crate)` to `gaply-core`
  and invisible to the app crate, so every record in those tests is written
  through the store and read back.

**AND THE MODULE MOVED TO gaply-core, correcting this entry's own reasoning.**
D152 said it lived in the app crate because the design specified `Uuid` and
`DateTime`, *"two dependencies gaply-core does not carry"*. **That reason was
void on arrival** — the implementation used `now_epoch()` and a `String` id, so
nothing was ever needed. The stated justification described a design that was not
built, and it survived a commit because nobody re-read it against the code it
justified.

What forced the move is the guarantee. `Database::conn` is `pub(crate)` to
`gaply-core`, so a store in the app crate cannot read the table — which means
`from_persisted` would have to be `pub` for the store to call it, which is
exactly the hole. **Co-location is what makes the constructor private.**
`gaply-core` already holds `app_check.rs` and `secrets.rs`, so a boundary concept
is not foreign there.

#### THE GUARD, AND WHY IT READS SOURCE

The property is a VISIBILITY fact and visibility cannot be observed at runtime: a
test that called `from_persisted` would not compile, and one that does not call
it proves nothing. So `a_consent_record_cannot_be_constructed_outside_this_module`
reads the module's own source — the instrument `wire_contract_tests.rs` and
`decision_records.rs` already use for compile-shaped properties.

It checks **two** things, because either alone leaves the guarantee gone:

1. the constructor carries no visibility modifier (`pub` opens it to the
   workspace; `pub(crate)` opens it to every other module in `gaply-core`);
2. no field is public — a public field makes `ConsentRecord { .. }` valid
   wherever the type is, which routes around a perfectly private constructor.

Both were broken on purpose and both went red:

```text
`pub fn from_persisted` — the constructor has regained visibility, so a caller
outside this module can mint a ConsentRecord the database never saw

ConsentRecord has a public field, so `ConsentRecord { .. }` bypasses the
constructor entirely
```

**The first version of this guard failed on its own search strings.**
`include_str!` pulls in the test module, and the needles appear literally in the
assertion a few lines below. It is the `pkill -f` shape — a pattern that cannot
tell its target from the thing doing the searching — so the guard is scoped to
the source preceding `#[cfg(test)]`, with an assertion that the split actually
found something, since a split that matched nothing would scan the whole file and
pass or fail for the wrong reason.

#### STILL NOT DONE

The proxy's `validate_structured` premium mode, and a premium payload builder.
Until the first exists, TIER is enforced on the desktop only, and a desktop-only
half of a two-sided boundary should be read as exactly that. Until the second
exists, `run_premium_gate` has no production runner — it is exercised only by
tests.

The proxy's `validate_structured` premium mode — the server-side half of this
boundary — is also Phase 1. Until it exists, TIER is enforced on the desktop
only, and a desktop-only half of a two-sided boundary should be read as exactly
that.

### D153 — the authorship signal is withdrawn from `major`, on every tier, because the threshold is the thing that is unmeasured

**THE SENTENCE THIS ENTRY EXISTS FOR, STATED FIRST AND PLAINLY:**

> **`classify()` takes only `(mean_ppl, burstiness)`. The boundary is
> tier-agnostic, and its own comment disclaims it. A better model behind an
> unvalidated threshold is a better-computed number on the same unvalidated
> line.**

That is why the whole lane is capped and not only the heuristic tier. The 7B
changes *which model computes perplexity*. It does not move the line, and nobody
has ever checked where the line should be.

The constants, with their own verdict attached (`ai_detect.rs:320-323`):

```rust
/// Interim, deliberately conservative thresholds for the heuristic proxy.
/// NOT calibrated against real GPT-2 output — hence the ever-present
/// disclaimer. Recalibrate when a real model is wired to the trait; the
/// SLM-1 tiers (Q3 vs Q4, ~+0.13 bits mean) need PER-TIER values here.
const AI_LIKE_PPL: f64 = 12.0;
const AI_LIKE_BURSTINESS: f64 = 8.0;
```

The second sentence is the one that had gone unread for months: a real model was
wired to the trait, and the recalibration it asks for never happened. The
per-tier values it says are needed do not exist, so `Full7B`, `Compact` and the
frequency proxy are all scored against numbers written for the proxy.

#### The measurement

**19 of 22 stored reports carry a `major` AI-detection finding** — 86% of every
run the product has ever done on this machine.

The negative control was run on the SHIPPED path (`run_pipeline_measured`, the
real six lanes), on a real human-written manuscript — a researcher's paper on
juvenile-hormone effects in *Bombyx* haemolymph:

```text
[lane] ai: signal: LeansAiLike (model: heuristic frequency proxy (fast pre-pass))
  7. [major] [AI-assessed, moderate confidence] AiDetection: concern
     detail: signal LeansAiLike (mean perplexity 8.2, burstiness 4.4); …
```

A word-frequency table called a human entomologist's paper AI-like, at the
severity that tells a researcher a reviewer would require it changed.

#### THE RE-MEASUREMENT, on six real manuscripts through the shipped pipeline

After the cap, the same harness over six of the researcher's own manuscripts
(four `.docx`, two `.pdf`):

```text
chapter3 .docx                                sev=info   AiDetection: concern
final final L.pdf                             sev=info   AiDetection: concern
IJAS Manuscript JHA Bombyx haemolymph (1).pdf sev=info   AiDetection: concern
Lake Chapter 1.docx                           sev=info   AiDetection: concern
R PAPER .docx                                 sev=info   AiDetection: concern
Revised Health Economics Paper FINAL (1).docx sev=info   AiDetection: concern
```

**6 of 6 at `info`.** The 22 stored reports are historical records written by the
old code and do not change; the number that moved is what a NEW run produces,
and on every manuscript available it is `info`.

**The second column is the one to sit with. The lane answers `concern` on 6 of
6** — every real academic manuscript it was given. That is the shape §11 D128
found fatal for `citation_need`, and it is stated here deliberately **without**
the conclusion D128 was entitled to draw:

- D128 had a **no-skill baseline** (flag everything = 18.0%) and could therefore
  say the lane was indistinguishable from it. **There is no such comparison
  here**, because there are no provenance labels for these six.
- Six manuscripts of unknown authorship provenance is not a sample, and several
  may well have had model assistance. *"It fires on everything"* and *"everything
  it was given happened to be flagged"* are different claims and only the second
  is supported.

So this is recorded as **the strongest available reason to build the labelled
set**, not as a finding that the lane is useless. The distinction is the same one
D128 insisted on, applied in the direction where the evidence is thinner — and
noting which of the two we are entitled to is the whole discipline.

#### AND THE RUN THAT PRODUCED THAT TABLE FOUND A DIFFERENT DEFECT

The first attempt at this table returned `NONE` for all six — including a
manuscript that had produced a finding minutes earlier. That is the signature of
a broken instrument, not a clean result: the loop used `timeout`, which does not
exist on this machine, and `|| true` swallowed the failure. **The known-good case
is what exposed it**, which is the argument for always having one in a batch.

The second attempt hung at 0.0% CPU on a keychain frame and exposed §11 D154.

#### THE CAP IS ON THE CLAIM, NOT ON THE PRODUCER — and that rule already existed

`AgentKind::AiDetection` produces **two different things**: the authorship
opinion (`ClaimKind::AuthorshipSignal`), and the document stylometry findings —
lexical diversity, citation density — which carry `ClaimKind::ManuscriptDefect`
at `Minor` and are documented as reviewer-relevant.

**This exact question was already decided.** `reviewer_agent::claim_is_eligible`
(`reviewer_agent.rs:1080-1084`) is keyed on the claim for editorial
admissibility, and says why in its own doc comment:

> *"This is EDITORIAL ADMISSIBILITY, deliberately keyed on the claim rather than
> on the producer. `AgentKind` means 'which subsystem produced this', which is a
> different question — and keying on it would exclude the stylometric findings
> `report.rs` documents as reviewer-relevant, since AI-detection produces both."*

So `authorship_capped` keys on `ClaimKind::AuthorshipSignal` and cites that rule
rather than restating it. **Inventing a second admissibility principle here would
be the §11 D129 shape**: two places deciding the same question, agreeing on the
day they are written, and drifting the first time only one of them is edited. A
cap keyed on the agent would also have re-decided the stylometry question *by
accident, in the opposite direction* — applying an argument about perplexity
thresholds to computations that do not use them.

#### What changed, and what stands

- **Severity** — an `AuthorshipSignal` finding is `info`, on every tier. This is
  the only behavioural change a researcher sees, besides the sentence below.
- **The finding now says which tier scored it, and that the lane has no measured
  accuracy.** `ai_detect::tier_provenance` is ONE definition covering both facts,
  because either alone misleads in a different direction: the tier alone implies
  the 7B's verdict is trustworthy; the accuracy caveat alone hides that a word
  frequency table may have produced it.
- **`AiDetectionReport::deep_kind`** — the tier now travels with the report.
  `perplexity_model()` computed the selection and threw it away one function
  later, so nothing downstream could tell a real model from the proxy except by
  string-matching the model's display NAME. `Option<DeepKind>`, where `None`
  means *not recorded* (a pre-field cached report) rather than *no deep model
  ran* — typed absence, because defaulting to `Absent` asserts something about a
  run nobody observed.
- **`detect_ai` (`commands.rs`) reports `DeepKind::Absent` as a fact about
  itself.** It hardcodes `HeuristicModel::gpt2_like()` and never consults
  `select_deep_model`, so that command is ALWAYS heuristic regardless of what the
  machine could run. That was true before and unsayable; now it is in the wire.
- **Escalation** — the authorship opinion can no longer clear
  `orchestrator.rs`'s `Critical | Major` bar, so it is never escalated. Spending
  a cloud call to ask a second model *"was this written by a model"* is asking a
  less-measured instrument to arbitrate an unmeasured one. The arm stays, with
  the consequence written down, because deleting it would make that decision
  silently rather than visibly.
- **UNCHANGED — the stylometry findings.** `Minor`, `ManuscriptDefect`, still
  reviewer-relevant. See above.
- **UNCHANGED — the lane itself.** It still runs, still scores, still reports its
  signal. Nothing is deleted and no threshold is touched: a tuned constant would
  be a *new* unvalidated line, which is the same defect with fresher numbers.
- **UNCHANGED — `claim_is_eligible`.** The claim was already ineligible for the
  recommendation, which is worth stating because it bounds what this fixes: the
  `major` was loud, not load-bearing. It changed what a researcher *believed*,
  not what the product *concluded*.

#### THE PATH BACK, with its precondition

**A labelled set. Nothing else.** Not a better prompt, not a bigger model, not a
tuned constant — until there are texts of known provenance scored by this lane,
there is no number to print beside the finding and nothing to raise it on.

The precondition is the same one §11 D128 named for `citation_need` and D126
named for the audit: a population, stratified, with the rate weighted rather than
pooled. For this lane it also needs to be **per tier** — the constants' own
comment says Q3 and Q4 differ by ~0.13 bits mean surprisal, so a single labelled
set scored on one tier does not license the other.

#### WHY THIS IS D128'S SHAPE AND NOT D128'S FINDING

`citation_need` was retired because it was *indistinguishable from a rule that
flags everything* — a measured comparison against a no-skill baseline.

**No such measurement exists here, and that is the whole point.** This lane has
not been shown to be useless; it has never been shown to be anything. The
withdrawal is not a verdict on its accuracy but a refusal to keep asserting a
severity that presupposes one. The two entries share a discipline — *a feature
whose own documentation disclaims it should not ship at a severity that
contradicts the disclaimer* — and differ in what is known: D128 measured and
withdrew; D153 withdraws **because** nothing has been measured.

### D154 — a consent check conditioned on having something to send is not a consent check

**THE RULE, STATED FIRST:**

> **Whether the payload turns out to be empty is decided AFTER the boundary, not
> at it.** A guard that asks *"is there anything to send?"* before *"am I allowed
> to send?"* has put the two questions in the wrong order, and the empty case is
> exactly where nobody looks.

#### The defect

The Analysis screen's network refusal (§11's blocker 3) shipped as:

```rust
if !refs.is_empty() && !consent.is_granted() { … refuse … }
```

A manuscript whose references do not parse — which is most `.docx` chapters —
**skipped the consent check entirely** and fell through to `verify_proxy`: a real
HTTP client, the OS keychain, a reachability probe, with consent denied. The
refusal was real and the boundary still had a hole, because the boundary was
guarded by a condition about the CARGO rather than about the PERMISSION.

#### HOW IT WAS FOUND, which is the transferable part

**No test caught it.** Every fixture that exercised the guard had references, so
the whole suite was green on a path that was open.

It was found by a batch measurement run — six real manuscripts through the
shipped pipeline — sitting at **0.0% CPU**, and `sample <pid>` putting the top
frame in `gaply_core::app_check::TokenSigner::from_keychain`: the §11 D124 modal,
on a path that had just been changed to never reach the network.

The tell was the CPU number, exactly as the standing CLAUDE.md norm says — high
CPU is working, 0.0% with a network-shaped stack is blocked on something it
should not have been doing at all. **The measurement that found it was not
looking for it**; it was re-measuring §11 D153's number and the environment
answered a different question.

#### The negative control

Restoring the old shape reproduces the wrong output, and it is the *specific*
sentence the fix's own commit message had called out as the misleading one:

```text
an empty reference list must still take the refusal path, not fall through to
the proxy: "0 reference(s) checked via public APIs; 0 definite verdict(s)"
```

That is the sharpest part of this entry. The commit that introduced the hole
argued, correctly and at length, that *"0 references checked"* and *"we did not
check your references"* are different sentences and only one of them is true —
and then left a code path that emits the wrong one. **Knowing the distinction is
not the same as having guarded it**, which is the §11 D128/D121 lesson arriving
in a third place.

`a_manuscript_with_no_references_still_refuses_when_consent_is_absent` is the pin.

---

### D155 — `local_name()` made `m:t` into `w:t`, and a ruler setting into a tab

**THE RULE, STATED FIRST:**

> **A parser that discards a namespace has not lost information, it has invented
> it.** Dropping an element leaves a hole, and a hole is visible to the next
> reader. Merging two elements because they share a local name produces text
> that parses cleanly, reads as prose, and says something the document does not
> — and nothing downstream can tell that apart from data.

#### What was fabricated, and where

`extract/docparse.rs`'s two DOCX readers matched elements with
`e.local_name()`, which strips the namespace prefix. One mistake, two victims.

**1. OMML flattened into prose.** `<m:t>` — an equation leaf — fell into the
`<w:t>` arm. The leaf text was concatenated and the structure that gives it
meaning (the fraction bar, the superscript, the radical) was thrown away,
because that structure lives in the MARKUP and never in the text. Measured
against `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` §3.8.1, Slovin's formula:

| in the manuscript | in the text stream every lane reads |
|---|---|
| `n = N/(1 + Ne²)` | `n=N1+Ne2` |
| `n = 237,000/(1 + 237,000(0.04)²) = 237,000/(1+379.2) = 237,000/380.2 = 623.36` | `n=237,0001+237,000(0.04)2=237,0001+379.2=237,000380.2=623.36` |

**`237,0001` and `237,000380.2` are numbers that are not in the manuscript.**
They are in the stream the statistic extractor, the AI-detection lane and the
plagiarism lane all consume. Three documents on this machine carry it today —
19, 19 and 39 `m:t` elements.

§6b.1 of the premium architecture had said math was *"flattened or dropped"*.
Those are different failures and the shipping one was the worse of the two; the
document did not say which, and nobody had measured.

**2. A ruler setting emitted as a tab character.** This one was not on the list
of things to look for. `<w:tab/>` inside a run is a tab; `<w:tab w:val="left"
w:pos="720"/>` inside `<w:pPr><w:tabs>` is a **tab-stop definition** — a ruler
setting that produces no text at all. The old loop fired on both, because they
are the same element name in different positions.

| document | `w:tab` total | inside `<w:tabs>` | genuine runs | tabs emitted (old) |
|---|---:|---:|---:|---:|
| `R PAPER .docx` | 21 | **21** | **0** | **21** |
| `Jitesh Agarwal .docx` | 986 | 493 | 493 | 986 |
| `Disha Correction .docx` | 284 | 145 | 139 | 284 |

**`R PAPER .docx` is the repo's primary measurement instrument** — §11 D58,
D65, D67, the 97.6% anchoring figure, the `citation_need` calibration set. It
has **zero** genuine tab runs. So, stated plainly:

> **Every measurement taken on `R PAPER .docx` before bcb3d2a was taken on a
> text stream containing 21 characters the document does not contain**, one at
> the head of each heading (`"\tRelated Work"`).

Nothing in this entry claims those measurements are wrong — a leading tab is
unlikely to move a citation-resolution count. The point is narrower and it is
the point: **the figure was never what it said it was, and no run could have
told anyone**, because a fabricated tab and a real one are the same character.

#### The fix, and the one rule in it worth transferring

Resolve namespaces (`NsReader::read_resolved_event`) rather than match prefixes
as strings — a document may bind WordprocessingML to any prefix, and a test
that matched `"w:t"` would have passed every other pin here. Stop emitting
content from inside OOXML property containers (the `…Pr` suffix is a convention
of the format, not a guess; `w:tabs` sits inside `w:pPr` and is covered by its
parent). Both readers now share one walk, so the SIBLING relationship §11 D59
established between them can no longer cost a second copy of the rules.

**An `m:oMath` becomes `EQUATION_PLACEHOLDER` (`[equation]`) when flattening
would lose structure, and keeps its text only when the element is a bare run
sequence — where flattening is provably lossless.** That predicate is what lets
`Where: N = target population` stay readable while `n=N1+Ne2` cannot occur;
dropping equations wholesale would have replaced one fabrication (`237,0001`)
with a smaller one (a dangling `= target population`).

**THE WHITELIST FAILS TOWARD THE PLACEHOLDER, AND THAT DIRECTION IS THE WHOLE
DESIGN.** The lossless set is enumerated — `oMath`, `oMathPara`, `r`, `t`,
`rPr`, `sty`, `ctrlPr`, `argPr` — and *every other OMML element, including every
element this code has never seen*, is structural. A blacklist of known
structural elements (`m:f`, `m:sSup`, `m:rad`, `m:nary`, `m:d`, `m:m`) would
have been shorter, would have passed the same tests, and would flatten the next
unfamiliar construct silently — which is the defect this entry is about,
re-armed for the next OMML feature Word emits. A guard whose unknown case is
"do the thing that broke" is not a guard.

#### The three reinstated defects, and what each caught

A green pin proves nothing about whether it gates (§11's negative-control rule,
and CLAUDE.md's "predict the failure before you claim the gate"). Each defect
was put back deliberately and the pins watched:

| reinstated | what went red | what stayed green |
|---|---|---|
| `m:t` matched as `w:t` again (no math capture) | `a_structured_equation_never_reaches_the_prose_stream_as_digits`, reproducing `n=237,0001+237,000(0.04)2` **verbatim**; `an_unknown_omml_element_is_structural_not_flattened`; the sibling-agreement pin | both tab pins, the namespace pin |
| property containers no longer suppressed | `a_tab_stop_definition_is_not_a_tab_character`; the sibling-agreement pin | every equation pin |
| namespace resolved by matching the literal `w:` prefix | `the_wordprocessing_namespace_is_matched_by_uri_not_by_prefix`, and **nothing else** | all six others |

The third row is the one that justifies its own pin existing. Prefix matching
fixes both shipped defects and passes every test written about them; only a
document that binds the namespace to another prefix distinguishes it, and that
document had to be written on purpose.

The positive controls matter as much. `a_tab_inside_a_run_is_still_a_tab`
exists because a "fix" that deleted all tabs would pass the tab-stop pin and be
wrong, and `an_inline_math_symbol_survives_because_flattening_it_is_lossless`
exists because a fix that placeholdered every equation would pass the
fabrication pin and be wrong.

#### The residue, and the one character that found the last defect

After the fix: `R PAPER .docx` is **exactly −21 characters**, matching its 21
fabricated tabs; four of the six manuscripts are byte-identical; the OMML
document's entire diff is its two structured equations (8→10 and 60→10
characters).

**That last figure was −51 before it was −48, and the three-character gap is
the transferable part of this section.** The hand-accounting predicted −50; the
measurement said −51. A one-character discrepancy in a fix about fabricated
characters is exactly the size that gets rounded away as a rounding artefact,
and chasing it instead found a third, smaller defect of the same family: a
`<w:br/>` inside `m:oMath` was being swallowed along with the equation, so the
real line break in `Where:` / `N = target population` collapsed to
`Where:N= target population`. Word whitespace inside an equation is Word
CONTENT, not OMML structure — swallowing it is the same namespace confusion
running the other way. With that corrected the diff reduced to the two
equations and nothing else, and the arithmetic closed exactly.

**So: reconcile the residue to the character, and treat a discrepancy smaller
than the effect as a finding rather than as noise.** A defect that hides inside
a rounding error is a defect that ships; this one had already shipped once.

#### What this entry does not cover

`<w:object>` OLE Equation Editor 3.0 objects: zero across all 17 real `.docx` on
this machine, so untested and untouched. DrawingML `a:t` (text boxes, charts,
SmartArt): the namespace fix means it now correctly does *not* reach the prose
stream, but no document in this corpus exercises it, so that is reasoned, not
measured.

---

### D156 — three axes that read like one word, and which of them becomes a type

**THE RULE, STATED FIRST:**

> **Before building against a name from a design document, check whether the
> code already has a name for that thing — and whether it is the SAME thing.**
> Two vocabularies for one concept start when a document's word is implemented
> beside a shipping word that already meant it. They also start the other way:
> when a document's word is *collapsed* into a shipping word that meant
> something adjacent, and the difference has nowhere left to live.

#### What was found

Building §6b's Tier-0 engine, two names from the architecture document had no
counterpart in code: `EpistemicStatus` (§9) and the five-row trust-tier table
(§4.4). The nearest shipping things were `report::CertaintyTier` and
`stats_verdict::Verdict`. The question — *do the document's names become real
types, or does the document adopt the code's?* — was decided before findings
were built rather than at findings time, because that is when the divergence
becomes expensive.

**They are three axes, not one concept with three names:**

| axis | the question it answers | where it lives |
|---|---|---|
| **trust** | how much weight does this carry, what overrides what | `report::CertaintyTier` |
| **verdict** | did a recomputation match | `stats_verdict::Verdict` |
| **epistemic status** | what is being asserted about the claim | nowhere |

#### The decision, and the measurement behind each half

**`EpistemicStatus` becomes a real type** (`gaply-core/src/epistemic.rs`). It is
a missing axis, not a synonym. `Verdict` is `Match | Mismatch` and its
binary-ness is LOAD-BEARING: `stats_verdict.rs` separates the verified and
advisory lanes by construction, and `Verdict` is carried only by the type that
also carries `CertaintyTier::MathematicallyCertain`. Adding `Unverified` to it
would put an un-decided result inside the type whose whole job is to be certain.
The two states the equation engine needs — `UNVERIFIED` and
`REQUIRES_AUTHOR_CONFIRMATION` — are precisely the two `Verdict` cannot hold,
and they are not decoration: the first is §6b.3's entire refusal discipline, and
the second is what the 21.9% case resolves to.

**§4.4's five-tier table does NOT become a type. The document adopts the code's
name.** Measured: `CertaintyTier` has **72 references across 9 Rust files** and
**crosses the IPC boundary** — `src/screens/report/reportTypes.ts` and
`src/screens/statsverifier/statsVerifierTypes.ts` both declare it, against the
serde wire names `mathematically_certain | ai_assessed_moderate |
reconsidered_after_peer_review`. It is a shipped contract with a frontend
consumer. A five-value `Tier` enum introduced beside it would be exactly the
failure this entry exists to prevent, in the same commit that named the risk.

And the precedence §4.4 is really about is already implemented twice:
`CertaintyTier::rank()` orders findings (`report.rs:821`, `orchestrator.rs`),
and `swarm::Opinion::hard_constraint` is the override — the deterministic
agent's verdicts are never voted on. §4.4's `0`–`4` numbering stays as editorial
ordering in the document, with `Tier 0 ≈ MathematicallyCertain` recorded as a
mapping rather than duplicated as a second enum.

**Why the asymmetry is right rather than inconsistent.** One name became a type
and one did not, and the test is not which document said it — it is whether the
code already has somewhere for the meaning to live. For trust it does, twice
over. For epistemic status it does not, anywhere, and collapsing it into
`CertaintyTier` would have left `UNVERIFIED` with nowhere to go, which is how a
"deterministic" engine quietly acquires a habit of guessing.

#### The decimal rule this made possible

`EpistemicStatus::RequiresAuthorConfirmation` does real work immediately.
**A manuscript's written decimals are ambiguous by construction**: `0.108` may
be the exact value used or `0.10843…` displayed to three places, and which was
meant is not recoverable from the text. The two readings give different answers
to the same arithmetic claim.

So every arithmetic claim is judged under BOTH (`DecimalReading::{AsWritten,
AsRounded}`) and the outcome reports which agree — both hold → no finding; both
fail → `DETECTED`; they disagree → a finding that SAYS SO. Picking one silently
would be the engine deciding a thing it cannot know, in the place it is easiest
to commit, and the result would look like a confident Tier-0 verdict.

One case is settled before the readings are consulted: where the reported value
is what the computed value ROUNDS TO at the precision the author displayed,
there is no discrepancy to explain. Slovin's formula in
`Corrected_Chapters_3_4_Jitesh_Agarwal.docx` computes 623.35613… and reports
`623.36`. That is display rounding and it is the negative control — it must
produce nothing.


---

### D157 — five values for one symbol, and a binder whose interesting number is its refusals

**THE RULE, STATED FIRST:**

> **A binder that binds everything produces a Tier-0 finding resting on a value
> the ENGINE chose.** Tier 0 overrides every model in the system (§4.4), so the
> number that says whether a binder is safe is not how many variables it binds.
> It is how many it refuses, and whether anyone has read the refusals.

#### The measurement that decided the design

§6b.1 says an equation's variables bind to research-state fields *"where the
binding is unambiguous"*. The obvious implementation is a document-wide symbol
table: scan for `<name> = <number>`, remember it, substitute. Before writing
one, the corpus was asked what it contains.

In `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` — **the document this engine's
negative control comes from** — the single symbol `N` is declared with five
different values:

| declaration | count | what it is |
|---|---:|---|
| `N= target population = 237,000` | 1 | the value Slovin's formula uses |
| `N = 600 \| Scale: 1=Strongly Disagree…` | 9 | table captions |
| `N = 600 usable` / `N = 600 planned` | 2 | the achieved sample |
| `N = 570` | 1 | an earlier SEM adequacy figure |
| `N = 30` | 1 | the pilot |

A document-wide table binds `N = 600` — it is twelve of the fifteen — and then
reports that `237,000/(1 + 237,000 × 0.04²) = 623.36` is wrong. **The arithmetic
is correct and the engine would have supplied the error**, wearing the one badge
in the system that overrides model consensus. This is why the predicate has
BLOCK SCOPE rather than document scope, and it is the whole argument for it.

#### The predicate — two sources, five conditions

**Source 1, preferred: unification, which reads no prose at all.** A manuscript
that writes `n = N/(1+Ne²)` and then `n = 237,000/(1+237,000(0.04)²)` has stated
the binding IN THE MATHEMATICS. Unifying the two trees yields
`N ↦ 237,000, e ↦ 0.04` and nothing else — unification either succeeds with
exactly one substitution or it fails, and it never searches. There is no
sentence to interpret, so there is nothing to misread.

**Source 2: a prose declaration, admitted only when all five hold.**

1. **Whole-line shape.** The line parses as a declaration IN ITS ENTIRETY. This
   is the condition that rejects `N = 600 | Scale: 1=Strongly Disagree…`, out of
   which the parser would otherwise happily read an `N = 600` prefix.
2. **Name left, number right.** A bare name, then a bare literal. Middle sides
   are glosses (`N = target population = 237,000`) and must never COMPUTE —
   see the third fabrication below. `N = normality of thiosulphate` declares a
   meaning, not a value, and binds nothing.
3. **In scope.** The declaration block belonging to that equation: the run of
   lines after it, through an optional `where`-style introducer, ending at the
   first line that is not a declaration.
4. **Unique within the block.** Two values bind NEITHER.
5. **A trailing parenthetical only as a CONSISTENT RESTATEMENT.**
   `e = margin of error = 0.04 (4%)` binds `0.04` because `4%` **is** `0.04`.
   That is a check, not a guess; a restatement that disagrees binds nothing.

Where the two sources disagree, neither wins and the conflict is recorded.

#### What it does, measured over nine documents

**2 bindings. 40 refusals. 1 reportable finding.**

Both bindings are `N` and `e` in the Slovin document, and they come from
UNIFICATION — the prose declaration agrees, so it is accepted silently rather
than treated as a conflict. Every one of the 40 refusals was read: all are
variables that no declaration values (`Vtitrant`, `A₆₆₃`, `Total Hardness`,
`P_G`), which is the honest state of a methods section that states formulas and
attaches no numbers to them.

**The two kinds of absence are worded differently, and that is not cosmetic.**

- *"no declaration in scope"* — there was nothing to choose between.
- *"different values are declared in the same block"* — there was, and the
  engine declined. This one keeps both candidates as evidence.

Only the second is evidence about the MANUSCRIPT. A report that spelled them the
same way would bury the interesting one in thirty-nine of the other kind.

#### THE FOUR FABRICATED FINDINGS, AND WHY THE CORPUS BEAT FIXTURES

Every one of these was a confident Tier-0 finding against a CORRECT manuscript,
produced by a working engine on real input. **Three of the four would have
passed any test written in advance**, because each needed a shape no author of
the code would think to write down.

| # | what it produced | document | cause | fix |
|---|---|---|---|---|
| 1 | `DETECTED` ×3 against table captions: *"at N = 2, usable = 3 the left is 2 and the right is 1800"* | `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` | `N = 600 usable` parsed as `N = 600 × usable` | a SPACE breaks implicit multiplication between a number and a word. `2A` is a product; `600 usable` is not. Mathematics spells a product `2x`, never `2 x` |
| 2 | `DETECTED` against the definition of R²: *"at R = 2, SSres = 3, SStot = 2.5 the left is 4 and the right is −0.2"* | `Disha Correction .docx` | `R² = 1 − (SSres/SStot)` tested as a universal identity | an equality whose two sides share NO variable is a DEFINITION, not a claim |
| 3 | the Slovin chain silently absent — nothing checked the engine's own negative control | `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` | the numeric chain begins with a name and ends with a literal, so it was read as a declaration of `n` | a declaration's middle sides must be glosses; the chain's COMPUTE |
| 4 | correct arithmetic reading as `DETECTED` once values were bound | synthetic, found by a test | a binding stored as a bare `Rational` became a zero-width interval, narrowing the rounded reading until the sides were disjoint | `BoundValues` carries the source literal's precision and writes both readings through one `insert` |

Only #4 came from a test, and only because writing the test forced the question
"what precision does a bound value have?" to be answered out loud.

#### #2 has its own shape, and it is worth naming

**A valid witness answering a question that should not have been asked.**

The engine was *right about the arithmetic*. `R² = 1 − SSres/SStot` genuinely
does not hold for every value of `R`, `SSres` and `SStot`, and `R = 2,
SSres = 3, SStot = 2.5` genuinely is a counterexample — a reader can check it by
hand and it will check out. Every component behaved correctly: the parser, the
prober, the witness, the message.

**What was wrong was the CATEGORY.** The manuscript was defining R², and the
engine evaluated it as a claim. Nothing inside the computation could have
detected that, because nothing inside the computation is about what kind of
statement it is looking at.

This is a different failure from the rest of §11's instrument entries. Those are
instruments that LIED — a swallowed exit status, a pattern matching its own
watcher, a calibration file carrying the defect it measured. This one is an
instrument telling the exact truth about the wrong question, and it is more
dangerous precisely because the evidence it offers is checkable and correct. A
reviewer handed that witness would verify it and conclude the finding stands.

The transferable form: **before asking whether a statement is true, ask what
kind of statement it is.** A checker that can only answer "true or false" will
answer it for everything it is given, including the things that are neither. And
the guard has to be structural — here, the disjoint-variables test — because a
confidence score would have been high on this one. It was not an uncertain
answer. It was a certain answer to a question nobody asked.


---

### D158 — a partial unit system manufactures dimensional findings

**THE RULE, STATED FIRST:**

> **A checker that knows SOME of a formula's units will derive a wrong
> dimension for it and report a correct formula as wrong.** Admitting a symbol
> whose companions stay unknown is not partial coverage; it is a defect that
> only appears once coverage improves. The honest state of a half-known formula
> is `UNVERIFIED`, and refusing a symbol you could plausibly guess is what keeps
> it that way.

This is §11 D157's shape in a second domain. There the engine gave a *certain
answer to a question nobody asked*; here it would give a *certain answer from
inputs it only half had*. Both wear the Tier-0 badge, which is the one thing in
the system that overrides model consensus (§4.4).

#### What the corpus carries, measured before the design

`examples/unit_scan.rs` over the six manuscripts: **14 unit annotations in three
forms** — `mg/L` ×9, `as CaCO₃` ×3, `mg/L as CaCO₃` ×2.

**Five of fourteen — 36% — carry a BASIS, which is not a unit.** `as CaCO₃` says
the quantity is expressed as the equivalent mass of calcium carbonate. Two
quantities both in `mg/L`, one on a CaCO₃ basis and one not, have identical
dimensions and cannot be added, and no exponent vector can say so. A system
built only from dimensions has two options on this corpus and both are wrong:
refuse 36% of what it meets, or drop the qualifier and treat unlike quantities
as like. So `Unit` carries the basis ALONGSIDE the dimension and the two are
compared separately.

#### The honest coverage, stated rather than buried

In `chapter3 .docx`, the units-richest document in the corpus:

| | |
|---|---:|
| units known after reading annotations and `where` clauses | **17** |
| formulas | **14** |
| dimensional checks actually performed | **1** |

The one is `Magnesium Hardness = Total Hardness − Calcium Hardness` →
`M·L⁻³ as CaCO₃`, and it needs both the basis field and unit propagation
through a definition. The other thirteen are `UNVERIFIED`.

**Across all nine documents: 8 consistent, 0 inconsistent, 23 unverified.**

#### Why the thirteen stay unverified, and why that is the right answer

One symbol. **`N` is normality throughout this corpus and the newton in SI**, so
the unit table refuses it and every formula containing it is unverifiable.

Admitting it is the obvious improvement and it is a trap. Ten of the fourteen
formulas would become *apparently* checkable while their dimensioned constants
stayed unitless — 8000, 50,000 and 35.45, each stated only in prose as
*"milliequivalent weight of O₂ × 1000"*, *"conversion factor for CaCO₃
equivalent"*, *"equivalent weight of chloride"*. Then:

```text
DO (mg/L) = (Vtitrant × N × 8000) / Vsample
            (L³   ×  N·L⁻³ ×  1  ) /  L³     =  N·L⁻³
declared:                                        M·L⁻³
```

→ a **dimensional mismatch reported on a correct formula**, because the
constant that converts equivalents to milligrams was read as dimensionless. The
engine would be arithmetically right and factually wrong, with a legible
derivation a reviewer could follow and confirm — D157's dangerous shape exactly.

So the refusal stays. `N` supplies nothing, the constants supply nothing, and
the formula is `UNVERIFIED`.

#### The limit belongs to the DOMAIN, not to the checker

This is the part §6b needs to record. **Real manuscripts state dimensioned
constants in prose**, because the reader is a chemist and the sentence
*"8000 = milliequivalent weight of O₂ × 1000"* is perfectly clear to one. There
is no markup for it, in `.docx` or anywhere else, and no parser recovers a unit
from that sentence without deciding what "milliequivalent weight" is
dimensionally — which is a chemistry judgement, not a parse.

So dimensional verification is **largely unreachable on real analytical
methods sections**, and will stay so until an author states a constant's units
machine-readably or the research record carries them. §6b.2 presents the check
as though it applies to *"every equation"*; §6b.3 has been corrected to say
where it does not, because a document that implies full coverage invites exactly
the partial-admission fix this entry rules out.

**What it IS good for, on this evidence:** derived quantities whose inputs are
themselves defined in the document — the `Magnesium Hardness` case, where every
operand traces back to a labelled definition. That is a real family and the
propagation-to-a-fixed-point exists to serve it. It is a smaller claim than §6b
made, and it is the one the measurement supports.

