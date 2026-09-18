# PublishReady, as built — a trace from the code

**Date of this trace: 18 Sep 2026. Tree: `~/dev/gaply-react-frontend`, branch `main`, HEAD `8e2e4da`.**

This document describes what PublishReady *is* in the committed code. It was
written by reading the code and running a small number of read-only
measurements; the architecture document was consulted only at the end, to record
where it and the code disagree (§7).

**One change was made after the trace, and only one:** the journal-key
divergence §4.5 found was repaired and guarded the same day. Every other section
describes the tree as it stands at `8e2e4da`.

## 0. How to read the evidence tags

Every claim carries one of two tags and a file:line.

* **[read]** — established by reading the named source. A `[read]` claim is a
  claim about what the code *says*, which is not the same as a claim about what
  runs. Where it matters, the reachability is stated separately and tagged.
* **[ran]** — established by executing something in this session. The three
  measurements run were: the `gaply_core` journal-seed tests; a structured
  comparison of two committed data files; and directory listings of the model
  paths the resolver consults.

Two things this trace does **not** establish, and does not claim:

1. **No PublishReady run was executed.** The app was not launched. Every
   statement about runtime behaviour is derived from the call graph, not from a
   run. Where a conclusion depends on a resolver reading the filesystem, the
   filesystem was listed [ran] and the resolver's rules were read [read].
2. **macOS only.** Nothing here was checked on Windows.

---

# 1. THE FULL PATH

A user picks a manuscript and a journal and clicks **Run PublishReady review**.
This is every hop.

## 1.1 The screen, before the button

`src/screens/publishready/PublishReadyPage.tsx` [read].

| # | What happens | Where |
|---|---|---|
| 1 | The screen asks the entitlement service whether this account may use `publishready`. Five states — `signed_out`, `offline_unverified`, `not_entitled`, `checking`, entitled — and four of them return a card instead of the form. | `PublishReadyPage.tsx:89`, `:269`, `:285`, `:301`, `:343` |
| 2 | On mount, `reviewerLetterAvailability()` asks the backend one question: is the configured proxy the loopback default? The answer is rendered **before** the run as "Before you run: …". | `PublishReadyPage.tsx:117-130`, `:435-442` |
| 3 | On mount, `journal_profiles` is invoked and the rows are filtered to `ingested && requirement_count > 0`. | `PublishReadyPage.tsx:139-153` |
| 4 | The journal search box merges the profiled rows with the bundled Scopus directory (`JOURNALS`), normalising names so a journal in both appears once, profiled row winning. | `PublishReadyPage.tsx:155-195` |
| 5 | Picking a row sets `{name, quartile, key}`. `key` is present **only** for a profiled row; a Scopus pick has `key: undefined`. The row visibly says which it is. | `PublishReadyPage.tsx:465-510` |
| 6 | The guidelines URL field is **not** prefilled, deliberately — the bundled directory carries journal websites, not guideline pages. | `PublishReadyPage.tsx:466-473` |

The entitlement gate is presentation only. The code says so in the file:
*"THE REAL gate is server-side at the proxy"* (`PublishReadyPage.tsx:84-88`)
[read]. §3.4 below records what that actually means today.

## 1.2 `run()` — the frontend half

`PublishReadyPage.tsx:219-264` [read].

1. `mayUseCloud('publishready')` — a `localStorage` read. False → the run never
   starts, with a message pointing at Settings (`:223-226`). Consent is
   **opt-out**: an absent or unreadable key means every suite is allowed
   (`src/screens/settings/settingsStore.ts:39-51`) [read].
2. If a guidelines URL was typed, `invoke('ingest_guidelines', {journalUrl,
   guidelinesUrl})` runs **first**, best-effort. A failure sets a note and does
   not block (`:238-247`).
3. `invoke('run_publishready', {path, journalName, journalQuartile, userToken,
   guidelinesUrl, journalKey})`
   (`src/screens/publishready/publishReadyBridge.ts:177-190`).

**`supplementary_paths` is never passed.** The Rust command accepts it
(`src-tauri/src/commands.rs:704`) but the bridge's `invoke` object has no such
key (`publishReadyBridge.ts:177-190`), so in production
`supplementary_paths == None` and the payload's supplementary section is always
`{present: false, note: "no supplementary data provided"}`
(`src-tauri/gaply-core/src/reviewer_agent.rs:326-328`) [read].

## 1.3 `run_publishready` — the Tauri command

`src-tauri/src/commands.rs:699-751` [read]. It clones three `Arc` handles out of
`AppState`, runs everything inside one `spawn_blocking`, then lifts the rendered
PDF bytes out of the returned value into a bounded in-memory queue
(`:740-749`; queue capped at `MAX_CACHED_PDFS = 4`, `src-tauri/src/state.rs:76`).

The whole body is `run_publishready_measured`
(`src-tauri/src/commands.rs:780-1082`). Its steps, in execution order:

| Step | What | Where |
|---|---|---|
| A | sha256 the manuscript file for the comparison record | `commands.rs:796` |
| B | run the six-lane pipeline | `commands.rs:812-833` |
| C | pull `report_id` out of the emitted `Finished` event | `commands.rs:835-842` |
| D | read the compiled report back out of the cache and parse it **typed** | `commands.rs:843-870` |
| E | parse supplementary files (none, in production — see §1.2) | `commands.rs:875-885` |
| F | targeted escalation: route + persist evidence, attempt per-finding cloud adjudication | `commands.rs:897-914` |
| G | build the reviewer payload + the `SentIds` grounding set | `commands.rs:917-919` |
| H | Box-4 **shadow** synthesis (deterministic verdict + optional cloud narrative) | `commands.rs:927-958` |
| I | render the PDF, using the **shadow's** deterministic recommendation | `commands.rs:973-981` |
| J | the **wholesale** cloud reviewer call + harness gate | `commands.rs:986-1013` |
| K | write the shadow-vs-wholesale comparison record to `~/Library/Application Support/ai.gaply.app/` | `commands.rs:1019-1062`, sink at `src-tauri/src/lib.rs:82` |
| L | return `PublishReadyOutcome` | `commands.rs:1065-1081` |

Note the ordering of I and J: **the PDF is rendered before the cloud reviewer is
called, and from a different quantity.** The PDF's "Recommendation" line comes
from `shadow_outcome.aggregation.verdict.recommendation()` — the local
deterministic aggregator (`commands.rs:973-974`). The screen's reviewer letter
comes from `reviewer`, the cloud call (`commands.rs:988`). §2.4 and §5.4 below.

## 1.4 The pipeline — six lanes, fixed order

`src-tauri/src/pipeline.rs::run_pipeline_inner` (`:391-813`) [read]. The graph is
loaded and validated first (`:420`), but **the lanes still execute in a
hardcoded sequence**; the graph is a declaration, pinned against the sequence by
a test (`src-tauri/gaply-core/src/agent_graph.rs:1136-1158`) [read].

| Lane | Order | What it reads | What it does | Line |
|---|---|---|---|---|
| parse | 0 | the file path | `docparse::parse_path` → whole text | `pipeline.rs:423-426` |
| **extraction** | 1 | the text | sections / statistics / citations / references / tables; persisted; headings streamed to the UI | `pipeline.rs:429-477` |
| **validation** | 2 | `extraction.statistics` + surrounding paragraphs | the deterministic rule set | `pipeline.rs:481-489` |
| **ai** | 3 | `extraction.sections` | per-section perplexity + burstiness with whatever model the memory gate allows | `pipeline.rs:498-506` |
| **plagiarism** | 4 | the text + the shared corpus | per-session isolated store; corpus comparison only | `pipeline.rs:509-519` |
| **rag** | 5 | the corpus | one fixed semantic query, `"reporting standards guidelines"`, top-5 | `pipeline.rs:523-531` |
| **verification** | 6 | `extraction.references` | real HTTP to CrossRef/OpenAlex/… per reference, then one model call for verdicts | `pipeline.rs:537-631` |

The verification lane's consent refusal sits **above** `RefVerifier::new()`, and
is unconditional on whether any references were found (`pipeline.rs:558-576`)
[read] — a refused lane returns an empty report and a sentence saying the check
was not made, and the run continues.

## 1.5 Synthesis — debate, checklist, compile

`pipeline.rs:634-731` [read].

1. Five agents are wrapped as `PrecomputedAgent` — their opinions cannot change
   (`:655-663`).
2. The sixth, verification, is a `RevisingVerificationAgent` **when a proxy
   object exists**, else a sixth `PrecomputedAgent` (`:665-697`).
3. `run_debate` runs up to `MAX_ROUNDS = 3` mesh rounds
   (`src-tauri/gaply-core/src/swarm.rs:271-335`, `:40`).
4. `build_checklist(db, extraction, text, guidelines_url, journal_key)`
   (`pipeline.rs:704-709`).
5. `compile_report(...)` with the debate outcome, validation, verification,
   plagiarism, extraction, the current year, the checklist, the reference
   registry and the manuscript's lines (`pipeline.rs:720-730`).
6. `unload_slm2()` unconditionally, on the error path too (`pipeline.rs:739`).
7. The report is serialised and cached under
   `report:v2:e{CACHED_REPORT_SCHEMA_VERSION}:{id}` for 30 days
   (`pipeline.rs:89-91`, `:753`).
8. `LaneExamination` is computed — five booleans that say which lanes examined
   anything at all (`pipeline.rs:762-784`).
9. `ResearchState::from_extraction` and two specialists are run and returned
   (`pipeline.rs:791-801`). Neither reaches a screen — §3.5.

## 1.6 The surfaces

| Surface | Reads | Where |
|---|---|---|
| Report viewer, 7 tabs | `outcome.report` | `PublishReadyPage.tsx:39`, `:405-411` |
| Overview / Statistics / Citations / AI Risk / Plagiarism tabs | findings filtered by `agent` | `src/screens/report/reportTypes.ts:165-179` |
| Checklist tab | `report.checklist` | `src/screens/report/ReportViewerPage.tsx:202-203` |
| Reviewer Letter tab | `outcome.reviewer` + `outcome.declined` | `PublishReadyPage.tsx:410`, `ReviewerLetterPanel.tsx:34-268` |
| "Export summary PDF" | `result.report.findings`, rendered in the browser | `PublishReadyPage.tsx:365-375` |
| "Open full report" | `invoke('export_publishready_pdf')` → the Rust-rendered bytes from step I | `PublishReadyPage.tsx:380-399`, `commands.rs:2658-2699` |
| Research Copilot dock | `{report, ragSnippets: [], citations: []}` | `PublishReadyPage.tsx:51-72`, `:413` |

Two consequences of the tab mapping that a reader should know [read]:

* Findings whose `agent` is `extraction` — the reference-recency family — have
  **no tab of their own**; they appear only under Overview
  (`reportTypes.ts:165-179`).
* Equation findings are emitted with `agent: AgentKind::ValidationMaths`
  (`src-tauri/gaply-core/src/equation_report.rs:131`, `:161`), so they land in
  the **Statistics** tab.

---

# 2. THE MODELS

There are four model positions in the product. Only two of them can run during a
PublishReady review, and on a stock install **neither of the two local ones is
the model the architecture document names**.

## 2.1 The bundled SLM — Qwen2.5-0.5B-Instruct, Q4_K_M

| | |
|---|---|
| **What it is** | `Qwen2.5-0.5B-Instruct-Q4_K_M.gguf`, 397,808,192 bytes, plus an 11,422,166-byte `tokenizer.json` [ran: `ls -la src-tauri/bundled-models/`] |
| **Where it lives, in the repo** | `src-tauri/bundled-models/` |
| **Where it lives, in the installed app** | `<resource_dir>/models/stage1-lm/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf` and `<resource_dir>/models/slm1-adapter/tokenizer.json` [read: `src-tauri/tauri.conf.json:74-78`] |
| **How it is found** | env `GAPLY_STAGE1_LM_GGUF` → `~/gaply-models/stage1-lm/*.gguf` → the bundled path [read: `src-tauri/src/models/mod.rs:68-76`, `:132-136`] |
| **Runs on** | this machine, CPU (candle) |
| **Invoked by PublishReady?** | **No.** |

Two distinct roles, neither of them in this pipeline [read]:

1. **AI Check's Stage-1 scorer** — `crate::models::stage1_lm_model()`, called
   from `src-tauri/src/aicheck.rs:285`. AI Check is a different screen.
2. **The AI engine's generative backend** —
   `BundledGenerativeLoader::resolve()` resolves through the *same*
   `stage1_lm_paths()` (`src-tauri/src/ai/generative.rs:471`), and is the model
   behind `ai_generate_test`, `ai_citation_need`, `ai_citation_support` and the
   batch audit jobs (`src-tauri/src/commands.rs:1340`, `:1403`, `:1498`,
   `:3327`). None of those is on the PublishReady path.

## 2.2 The AI-detection model — a three-tier ladder that lands on tier three

The PublishReady AI lane calls `crate::models::perplexity_model_with_kind()`
(`pipeline.rs:502`), which runs `select_deep_model()`
(`models/mod.rs:954-958`, `:671-697`) [read].

`select_deep_model` composes two decisions:

* **Structural** — `deep_tier(total_ram, force, disable, full_present,
  mini_present)` (`models/mod.rs:484-523`). The 7B needs ≥ 15 GiB
  (`DEEP_PASS_MIN_RAM_BYTES`, `:444`) and is unreachable below that except via
  `GAPLY_FORCE_DEEP=full`.
* **Per-run** — `plan_deep_load` adds a free-memory courtesy check requiring
  1.5× the model's resident estimate (`:622-654`, `:427-432`).

The two candidate models are:

| Tier | Model | Resolved from | Bundled? |
|---|---|---|---|
| `Full7B` | Qwen2.5-7B | `GAPLY_SLM1_GGUF` → `~/gaply-models/slm1/*.gguf` → bundled | **no** (`models/mod.rs:106-111` states it) |
| `Mini` | Qwen2.5-1.5B | `GAPLY_SLM1_MINI_GGUF` → `~/gaply-models/slm1-mini/*.gguf` → bundled | **no** (`:117-122`) |

**Measured [ran]:** `src-tauri/bundled-models/` contains exactly one `.gguf`,
the 0.5B, and `tauri.conf.json` maps it to `models/stage1-lm/` only — there is no
`models/slm1/` or `models/slm1-mini/` in the bundle. `~/gaply-models/` on this
machine contains exactly `slm1-adapter/tokenizer.json` and
`stage1-lm/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf`, and `find ~/gaply-models -type l`
prints nothing.

So on a fresh install **and on this development machine**, `slm1_present()` and
`slm1_mini_present()` are both false (`models/mod.rs:526-533`), `deep_tier`
returns `HeuristicOnly` (`:519-522`), `plan_deep_load` returns
`Skip(DeepKind::Absent)` (`:645-652`), and `model_for_selection` falls back to
`HeuristicModel::gpt2_like()` (`:964-970`).

**What actually scores the manuscript, therefore, is a word-frequency table.**
`HeuristicModel` assigns 0.5 bits to punctuation, 2.0 bits to any of ~150
hardcoded common words, and `3.5 + 0.55*(len-3)` clamped to `[3.5, 9.0]` to
everything else (`src-tauri/gaply-core/src/ai_detect.rs:109-122`, `:126-181`).
It is context-free — the "perplexity" it reports is a function of word length
and a stop-word list.

The code is honest about this at the surface. `deep_kind` travels with the model
(`models/mod.rs:954`), reaches the report through the swarm adapter
(`swarm.rs:406`), and `tier_provenance(Some(DeepKind::Absent))` renders
*"Scored by a WORD-FREQUENCY PROXY, not a language model — no deep model ran on
this device. THE LANE HAS NO MEASURED ACCURACY…"*
(`ai_detect.rs:433-437`) [read].

And its findings are capped: any finding whose `ClaimKind` is `AuthorshipSignal`
is forced to `Info` regardless of proposed severity
(`src-tauri/gaply-core/src/report.rs:458-463`), and the AI-detection round-table
opinion is exactly that claim (`report.rs:415-424`).

## 2.3 The embedder — two of them, and PublishReady gets the non-neural one

| | Pipeline embedder | AI-engine embedder |
|---|---|---|
| What | `HashEmbedder` — FNV-1a feature hashing into 384 dims, L2-normalised | `bge-small-en-v1.5`, candle BERT, 384 dims, CLS pooling |
| Where | `src-tauri/gaply-core/src/embed.rs:31-53` | `src-tauri/src/ai/embeddings.rs:50-77`, `src-tauri/src/ai/mod.rs:52-95` |
| Constructed | `src-tauri/src/lib.rs:121` → `AppState.embedder` | `src-tauri/src/state.rs:86` → `AppState.ai_embed` |
| Used by PublishReady | **yes** — plagiarism ingestion (`pipeline.rs:511`), RAG search (`pipeline.rs:524`), guideline ingestion (`commands.rs:487`) | **no** |
| Needs an install | no | yes; `NotInstalled` is a normal state and the app boots without it (`ai/embeddings.rs:58-62`) |

This is the single most consequential thing in §2. Everything PublishReady calls
"semantic" — the plagiarism similarity score, the RAG retrieval — is cosine
similarity over a **bag-of-words hash**, not embeddings. `report.rs` says so at
the point of rendering: the plagiarism finding title reads *"word overlap"*, not
*"similarity"* (`report.rs:800-803`) [read].

## 2.4 The cloud reviewer — Claude Sonnet 5, via gaply-proxy

The desktop side is `ProxyReqwestClient` (`src-tauri/src/models/proxy_client.rs`)
[read]:

* Endpoint: `GAPLY_PROXY_URL`, defaulting to `http://127.0.0.1:8080` (`:41`,
  `:160-165`). The loopback default is what
  `reviewer_letter_availability` detects to say "unavailable" before a run
  (`commands.rs:674-689`).
* Credentials: a **fresh single-use App Check token per request**, signed with a
  key read from the OS keychain; an absent key is `Err` and routes to the local
  tier (`proxy_client.rs:201-209`, `:305`). Plus the user's Supabase JWT in a
  separate header (`:311-313`).
* Liveness: `GET /health`, retried only on timeout/502/503/504, worst case ~35 s
  (`:51-55`, `:241-293`).
* Call: `POST /verify`, 180 s timeout (`:59`, `:307`). Response envelope
  `{"result": {"model", "stop_reason", "text"}}`, where `text` is parsed as JSON
  (`:323-339`).

The proxy side (`gaply-proxy/app/`) [read]:

* `POST /verify` → App Check dependency → entitlement dependency → **hard
  validator** → provider → consume one use (`main.py:240-268`).
* The validator rejects any payload over 8000 total string-leaf characters, any
  single field over 2000 characters, or any field over 400 characters with more
  than 8 sentences (`validation.py:14-17`, `:39-75`).
* The provider forwards **only** `payload["summary"]` and
  `payload["instruction"]` — nothing else on the payload reaches the model
  (`claude_client.py:54-56`).
* Default model: **`claude-sonnet-5`** (`config.py:13`, `:76`), `max_tokens`
  1024 by default (`claude_client.py:37`). An OpenAI provider exists and is
  selected only by `GAPLY_LLM_PROVIDER=openai` (`config.py:17`, `:77`).

**What the reviewer receives and returns.** `build_review_payload`
(`src-tauri/gaply-core/src/reviewer_agent.rs:542-614`) sends: the journal name
and quartile, the debate's two-value verdict string, up to
`MAX_FINDINGS = 12` findings (id, agent, tier, severity, title, confidence,
structured provenance only), up to `MAX_CHECKLIST = 20` checklist rows
(id, requirement, passed), the supplementary block, and a count of findings
dropped by the bound. `gate_reviewer_response` (`:634-705`) then requires a
parseable `recommendation` and a `publication_probability` in 0..=100, drops any
issue citing an id that was not sent, empties `novelty_assessment` and
`journal_fit_note` unless each cites a sent id, drops ungrounded `alternatives`,
and downgrades a `reject` with zero grounded issues to `Unknown`.

**When it does not run** — no App Check key, unreachable proxy, HTTP error, or a
gate failure — `ReviewerEvaluation::unavailable_offline()` is returned:
`Unknown`, `publication_probability: None`, empty everything, one warning, and
`available: false` (`reviewer_agent.rs:235-247`; selected at
`commands.rs:988-1012`).

## 2.5 The two local fallbacks for citation verdicts

`verify_proxy(user_token)` picks one of three implementations of the same
`ProxyClient` trait, in order (`models/mod.rs:1061-1106`) [read]:

| Tier | Implementation | Condition |
|---|---|---|
| 1 | `ProxyReqwestClient` (Claude Sonnet 5, via the proxy) | App Check key present **and** `/health` answers |
| 2 | `OllamaVerifyClient` | `http://127.0.0.1:11434` reachable; model tag `qwen3:4b` (`models/mod.rs:26-27`) |
| 3 | `MockProxyClient::returning({"verdicts": []})` | otherwise — every citation comes back `Unknown` |

So **two models can produce citation verdicts, and which one fires is decided by
reachability, not by configuration**: the cloud wins when it answers, local
Ollama when it does not, and an empty-verdict stub when neither is there. The
report distinguishes the outcome honestly — the collapsed "could not be checked"
finding says the service was unavailable and that this is *not* a finding about
the references (`report.rs:722-731`).

`unload_slm2()` is called after the debate to guarantee `qwen3:4b` is out of
Ollama's memory before any subsequent candle load (`pipeline.rs:733-739`,
`models/mod.rs:1031-1045`).

## 2.6 Models named in the code and never invoked on this path

| Named | Where | Status |
|---|---|---|
| `slm1_model()` — Qwen2.5-7B | `models/mod.rs:171-196` | reachable only through `select_deep_model`; the file is not bundled and not present here [ran] |
| `slm1_mini_model()` — Qwen2.5-1.5B | `models/mod.rs:205-234` | same |
| `aicheck_classifier()` — SLM-2 over Ollama for AI-Check classification | `models/mod.rs:982-997` | its own doc says the shipped AI-Check flow passes `None` unconditionally (`:977-981`) |
| `bge-small-en-v1.5` | `src/ai/embeddings.rs` | real, resident, and never on the PublishReady path |

---

# 3. THE AGENTS

## 3.1 What an agent is, concretely

There are **three different things** the codebase calls an agent, and they are
not interchangeable.

1. **A pipeline lane.** A block inside `run_pipeline_inner` wrapped by the
   `lane()` helper, which emits a start event, runs a closure, and emits a
   completion or failure event (`pipeline.rs:211-228`). This is what actually
   executes.
2. **A debate participant.** An implementation of `trait SwarmAgent`
   (`swarm.rs:97-105`) with `kind()`, `opine()` and an optional `revise()`. Five
   of the six are `PrecomputedAgent` — a frozen `Opinion` (`swarm.rs:137-154`).
3. **A node in the declared graph.** A row in
   `src-tauri/gaply-core/data/agent_graph.json`, validated at first use and
   `panic!`ing if it does not validate (`agent_graph.rs:1088-1099`). Nothing
   dispatches on it; it is the declaration of record, and the module says so
   (`agent_graph.rs:156-161`).

## 3.2 The six lanes

Trust tiers and clusters below are read from the graph file, which is the
declaration; the "runs?" column is read from `run_pipeline_inner`.

| Agent | Graph trust tier | Model | Reads | Deterministic? | Debate role | Runs in production? |
|---|---|---|---|---|---|---|
| `extraction` | `deterministic` | `none` | `manuscript` | yes | precomputed, always `pass` at 0.9 | **yes** — `pipeline.rs:429` |
| `validation_maths` | `deterministic` | `none` | `research_state` | yes | **hard constraint** — never voted on, always overrides | **yes** — `pipeline.rs:481` |
| `ai_detection` | `evidence_reasoning` | `{local: perplexity}` | `research_state` | no (statistical) | precomputed, calibration k = 0.6 | **yes** — `pipeline.rs:498` |
| `plagiarism` | `deterministic` | `none` | `research_state`, `external_evidence` | no (similarity) | precomputed, k = 0.8; **`gate_passed: false` when the corpus is empty** | **yes** — `pipeline.rs:509` |
| `rag` | `rule` | `none` | `research_state`, `journal` | yes | precomputed, k = 0.8 | **yes** — `pipeline.rs:523` |
| `verification` | `evidence_reasoning` | `cloud` | `research_state`, `external_evidence`; `requires_consent: external_evidence` | no (LLM verdicts) | **the only reviser**, k = 0.6 | **yes** — `pipeline.rs:537` |

Graph rows: `data/agent_graph.json:4-92`. Calibration factors:
`swarm.rs:67-76`. Hard constraint: `swarm.rs:371-384` (the adapter sets
`hard_constraint: true`) and `swarm.rs:202-223` (consensus honours it,
recording `overridden_by_constraint` when the soft majority disagreed).
Plagiarism's gate: `swarm.rs:424-446`. Verification's gate: an opinion is
rejected if **any** verdict carries gate flags (`swarm.rs:466-467`).

**Three properties the graph asserts and tests enforce** [read]:
`verification` is the only `model: cloud` agent and the only `may_revise: true`
agent (`agent_graph.rs:1183-1202`); `validation_maths` is the only
`hard_constraint` and it is `deterministic` (`agent_graph.rs:1206-1208`).

**The debate, precisely** (`swarm.rs:271-335`): every agent opines; opinions
failing their own gate are rejected before the round-table; up to three mesh
rounds in which each agent sees every other opinion; a round with zero revisions
terminates early as converged; then a confidence-weighted vote over the non-hard
opinions, with any hard constraint overriding the result at
`combined_confidence: 1.0`. A revision cannot change its own `agent`,
`hard_constraint` or `gate_passed` — those are copied back from the original
(`swarm.rs:314-317`).

**The only revision that can happen** is verification reconsidering its verdicts
when a peer disagrees, at most once per debate, by making a *second* proxy call
through `reconsider_citations` (`swarm/revising.rs:76-124`). If that call fails,
the prior gated verdicts are held (`:118-122`).

## 3.3 The three specialists

`frequentist_stats`, `ml_methodology` and `claim_evidence_strength` are declared
in the graph (`data/agent_graph.json:93-180`) and returned by
`specialist::shipped()` (`gaply-core/src/specialist/mod.rs:213-219`) [read].

**Two of the three execute in production, and their output reaches nothing.**
`pipeline.rs:794-801` filters `shipped()` to `frequentist_stats` and
`claim_evidence_strength`, runs both, and puts the reports in
`PipelineResult.specialists`. `ml_methodology` is deliberately withheld there,
with the reason in the field's doc comment (`pipeline.rs:305-322`).

`PipelineResult.specialists` has **no production consumer**: the only references
in the tree are the struct field itself and a pipeline test
[ran: grep over `src/`, `gaply-core/src/`, `examples/`]. `report_build.rs:270`
constructs an unrelated value with `specialists: Vec::new()`. The field's own doc
comment states this (`pipeline.rs:324-331`).

Both run with `analysis: None` (`pipeline.rs:795`), so any specialist logic
gated on an `AnalysisRecord` — the SPSS parser's output — never fires.

## 3.4 Named, built, and with no production caller

This is the distinction the question asks about. Each row was checked by grep
over `src-tauri/src/`, `src-tauri/gaply-core/src/`, `src-tauri/examples/` and
`src/` [ran].

| Thing | Where | Callers found |
|---|---|---|
| `review_lens` — the reviewer-lens layer | `gaply-core/src/review_lens.rs` | `examples/lens_review_probe.rs` only. No Tauri command builds a `LensInput`. |
| `novelty` | `gaply-core/src/novelty.rs` | `review_lens.rs` (itself uncalled) and two examples. The module header opens **"DECLINED — §11 D166"**. |
| `chat_scope` — the four chat modes | `gaply-core/src/chat_scope.rs` | `red_team.rs` tests and an example. The shipped Copilot uses `chat_agent` with a frontend-built context instead. |
| `analysis::spss` — the SPSS syntax parser | `gaply-core/src/analysis/spss.rs` | `red_team.rs`, `specialist/frequentist.rs` tests. Nothing on the pipeline path; `analysis: None` at `pipeline.rs:795`. |
| `run_premium_gate` — the premium release gate | `src-tauri/src/release_gate.rs:223` | **only its own tests** (`:843`, `:919`, `:940`, `:1002`). |
| `journal_crawl::crawl` — the live journal crawler | `src-tauri/src/journal_crawl.rs` | 20+ files under `examples/`, and **no Tauri command**. `commands.rs:543` reads only the *config* from the same module. |
| `report::uncited_reference_findings` | `gaply-core/src/report.rs:1510` | none — marked `#[allow(dead_code)]` at `:1607` with the reason. |
| `pipeline::GUIDELINE_QUERY` | `src-tauri/src/pipeline.rs:114` | none; the RAG lane uses a different literal at `:524`. |
| `PipelineResult.research_state` | `pipeline.rs:281`, built at `:791` | nothing in the report path reads it; the field says so. |
| `PublishReadyOutcome.shadow_reviewer` | `commands.rs:626`, `:1080` | serialised to the frontend and **never read** — `adaptOutcome` reads `o.reviewer` only (`publishReadyBridge.ts:96-136`), and grep for `shadowReviewer`/`shadow_reviewer` under `src/` returns nothing [ran]. |

**The premium gate matters most of the three "wrong five times this week"
candidates.** There is no tier check anywhere on the run path: `run_publishready`
takes a `user_token` and forwards it, and the *proxy* is where entitlement is
decided (`main.py:199-239`, `:266-267`). The desktop entitlement state gates
*access to the screen* and nothing else, which the screen itself now says in a
comment where a `premium` badge used to be (`PublishReadyPage.tsx:355-364`).

---

# 4. THE JOURNAL LAYER

**Both.** The product reads journal pages at runtime *and* ships a snapshot, and
the two are different mechanisms serving different parts of the checklist.

## 4.1 The bundled seed — what ships

`src-tauri/gaply-core/data/journal-seed.json`, compiled into the binary at
`journal_store.rs:970` and loaded at app startup (`src-tauri/src/lib.rs:107-119`)
[read].

Measured contents [ran, `python3` over the file]:

```
fingerprints  10      requirements 213      bindings 50
conventions   50      expectations 258      run_started_at 1789728313
kinds: reporting_standard 70, figure_limit 40, section_required 34,
       word_limit 18, data_policy 17, reference_style 17, abstract_limit 11,
       reference_limit 6
status: verified 191, conflicted 22
```

The seed is skipped per journal when a fingerprint row already exists, and
seeded rows are stamped `origin = 'bundled'`
(`journal_store.rs:1037-1052`). Three tests pin this
[ran: `cargo test -p gaply_core journal_store::seed_tests`, `CARGO_EXIT=0`,
`targets=11 passed=3 failed=0`].

## 4.2 The runtime fetch — one, and only when the user types a URL

`ingest_guidelines` (`commands.rs:472-494`) → `GuidelinesIngestor::ingest`
(`src-tauri/src/guidelines.rs`) [read]. It is called from `run()` only when the
guidelines field is non-empty (`PublishReadyPage.tsx:238-247`). It:

* fetches through the rate-limited `ReqwestFetcher`;
* classifies the page as `Interstitial` / `Navigation` / `Guideline` using
  obligation-language and requirement-evidence counts rather than a length
  threshold (`guidelines.rs:58-120`);
* wraps every extracted string in `UntrustedText` and runs
  `rag::ingest_document`, which sanitises and quarantines again;
* degrades honestly — unreachable, non-HTML or injection-flagged pages yield
  `Unavailable`/`Quarantined` with a note, and never block the run
  (`guidelines.rs:14-20`).

**There is no other runtime journal fetch.** `journal_crawl::crawl` — the
multi-page crawler with a 120-page budget and depth 3
(`src-tauri/config/journal-crawl.json`) — is registered as no Tauri command and
is reached only from `examples/` [ran, §3.4]. It is the tool that *built* the
seed, not something the product runs.

## 4.3 What the checklist is made of

`build_checklist` (`gaply-core/src/report.rs:1712-1760`) [read]:

1. `checklist_from_guidelines` always emits **four structural rows** — Abstract,
   Methods, Results, References present or missing — from the extraction's own
   section list, with `guideline_source: None` (`report.rs:2358-2380`).
2. If a guidelines URL was ingested, up to three more keyword rows can come from
   the fetched chunks: structured abstract, conflict-of-interest declaration,
   numbered reference style (`report.rs:2432`, `:2475`, `:2505`). The word-limit
   detector on this path is disabled (`report.rs:2400`).
3. **If `journal_key` is present and that key has stored requirements, the
   keyword rows are dropped and replaced** by
   `design_independent(checklist_from_requirements(...))`
   (`report.rs:1744-1757`). The structural four survive, because they are
   identified by `guideline_source: None`.

`checklist_from_requirements` (`report.rs:1919-2220`) emits, per stored
requirement kind: a word-limit row (or, where the journal states several by
article type, one row that reports them and judges nothing, `:1952-1976`); an
abstract-limit row, suppressed when the extractor found no abstract because
absence is not a failure (`:1989-2002`); a data-availability row; and one row per
distinct required statement, searched over the body with references stripped and
with synonym lists (`:2033-2066`, `:1774-1793`).

**Reporting-standard rows cannot be produced today.** `build_checklist` passes
`bindings: &[]` (`report.rs:1753`) and then filters with `design_independent`,
which drops anything whose `checked_field` starts with
`journal_requirements.reporting_standard` (`report.rs:2338-2347`) — so the 70
`reporting_standard` rows in the seed contribute nothing to a checklist. The
reason is recorded at length at `report.rs:2309-2330`: the study-design gate that
would decide which standard applies was itself declined (§11 D177), and shipping
the rows unhedged put an ARRIVE animal-study item against an employer survey.

**Only four of the eight stored requirement kinds are consumed at all.**
`checklist_from_requirements` matches `WordLimit`, `AbstractLimit`, `DataPolicy`
and `SectionRequired`, plus `ReportingStandard` which is then filtered out
(`report.rs:1931`, `:1980`, `:2013`, `:2040`, `:2254`) [ran: grep for
`RequirementKind::` over `report.rs`]. The seed's 40 `figure_limit`, 17
`reference_style` and 6 `reference_limit` rows are stored, readable, and reach no
checklist row.

## 4.4 What a user gets for a journal that is not one of the ten

A Scopus-directory pick carries `key: undefined`
(`PublishReadyPage.tsx:474`), so `journal_key` is `None` and step 3 above never
runs. The checklist is the four structural rows, plus up to three keyword rows if
the user pasted a guidelines URL that ingested successfully. The picker says this
in the row before the run: *"Not crawled — structural checks only"*
(`PublishReadyPage.tsx:503-509`).

## 4.5 The key divergence — found in this trace, fixed 18 Sep 2026

**This section described a live defect when it was written. It was repaired the
same day; what follows is the record, because the repair is the reason the
numbers elsewhere in this document changed.**

### The defect

The runtime enumerated journals from `src-tauri/config/journal-crawl.json` —
`journal_profiles` builds a `CrawlBudget` from that file and calls
`fingerprint_for(&db, &j.key)` for each entry (`commands.rs:543-551`). The seed
wrote rows under the keys in `journal-seed.json`. Five of the ten differed:

| Journal | old config key | seed key |
|---|---|---|
| Nature Communications | `nature-comms` | `nature-communications` |
| Statistics in Medicine | `stat-med` | `statistics-in-medicine` |
| Frontiers in Public Health | `front-pubhealth` | `frontiers-public-health` |
| J. Health Psychology | `jhp-sage` | `j-health-psychology` |
| BMC Public Health | `bmc-pubhealth` | `bmc-public-health` |

Lookups are exact (`journal_store.rs:426-430`, `:460-465`) and no alias existed
anywhere [ran]. So those five journals returned `provenance: None` and zero
requirements, the picker's `ingested && requirement_count > 0` filter
(`PublishReadyPage.tsx:144`) removed them, and **83 stored requirement rows were
unreachable from any surface** — 68 of them Frontiers in Public Health.

Neither file's own tests could see it: `journal_store::seed_tests` asserts the
seed writes what the seed says (`journal_store.rs:1122-1128`), which is true and
says nothing about the config. Nothing read both files.

### The repair

`config/journal-crawl.json:86-97` now carries the seed's spelling for all ten.
Aligning rather than aliasing, because the key is read at three call sites
(`journal_profiles`, `run_publishready`'s `journal_key`, `journal_fingerprint`)
and a second mapping is a second thing to keep correct. Nothing was orphaned:
the only writers of `journal_fingerprints` are `load_bundled_seed` and
`store_fingerprint_provenance`, and the sole caller of the latter outside core's
own tests is `examples/journal_stage2_build.rs`, which uses the seed spellings
[ran] — no shipped path ever wrote a config-spelled key.

`gaply-core/tests/journal_keys_match_the_seed.rs` pins it: both files are read
with `include_str!`, both key sets are asserted non-empty before being compared,
and the failure message names the offending keys on each side. It lives in
`gaply_core` because both CI workflows run `cargo test -p gaply_core` and nothing
else (`clean-checkout.yml:100`, `windows-build-check.yml:125`), so an app-crate
guard would pass under a local `--workspace` run and never run remotely.

Deletion-tested in both directions [ran]: reverting one key to `front-pubhealth`
turns it red naming both spellings; renaming the `profiled_journals` array turns
it red with *"this guard is now blind"* rather than passing vacuously.

### What the picker shows now — measured, not derived

Measured by seeding an in-memory `Database` with `load_bundled_seed` and calling
`fingerprint_for` for each config key, which is exactly what `journal_profiles`
does minus the Tauri wrapper [ran]:

| Journal | requirements | conflicted facts | picker |
|---|---:|---:|---|
| PLOS ONE | 38 | 0 | shown |
| Nature Medicine | 37 | 1 | shown |
| Frontiers in Public Health | 56 | 2 | **shown (was hidden)** |
| PLOS Medicine | 31 | 1 | shown |
| The BMJ | 16 | 1 | shown |
| Statistics in Medicine | 7 | 0 | **shown (was hidden)** |
| The Lancet | 2 | 0 | shown |
| J. Health Psychology | 2 | 2 | **shown (was hidden)** |
| BMC Public Health | 2 | 0 | **shown (was hidden)** |
| Nature Communications | 0 | 0 | hidden — `source_count: 0`, its crawl fetched nothing |

**Nine, not ten**, and Nature Communications is filtered on its own merits
(`ingested: true`, zero requirements) rather than on a key mismatch.

**A correction to this document's earlier numbers.** An earlier draft of this
section quoted per-journal counts of 38 / 33 / 39 / 18 / 2 and 68 / 7 / 6 / 2.
Those are raw rows in `journal-seed.json`. `journal_profiles` reports
`fp.requirements.len()`, and `fingerprint_for` moves conflicted rows out of
`requirements` into `conflicts` (`journal_fingerprint.rs:189-206`), so the
displayed figures are the table above. Both quantities are real and they answer
different questions; only the second is what a user sees.

The reconciliation: 124 requirements were reachable before the fix, 191 after.
Of the 83 previously-stranded rows, 67 return as requirement rows and the
remaining 16 group into 4 conflicted facts (12 rows → 2 for Frontiers, 4 rows →
2 for J. Health Psychology). 67 + 16 = 83, and 124 + 67 = 191.

# 5. WHAT IT ACTUALLY EVALUATES

## 5.1 Every check that can produce a finding today

Findings are built in `compile_report` (`gaply-core/src/report.rs:550-1020`)
[read]. Each row below names the constructor.

| # | Check | Severity | Determinism | Reads | Where |
|---|---|---|---|---|---|
| 1 | test-vs-group-count mismatch | **Critical** | deterministic | `extraction.statistics` + paragraph | `validate.rs:70`, emitted `report.rs:588-617` |
| 2 | p-value overclaiming | **Major** | deterministic | same | `validate.rs:71-74` |
| 3 | missing effect size | **Major** | deterministic | same | `validate.rs:71-74` |
| 4 | missing confidence interval | **Major** | deterministic | same | `validate.rs:71-74` |
| 5 | equation arithmetic disagrees | **Major** (`Detected`/`Contradicted`) or **Minor** (`RequiresAuthorConfirmation`) | deterministic, exact rational, no I/O | the manuscript's own lines | `equation_report.rs:60-66`, `:73`; run at `report.rs:836-838`, built at `:1033-1059` |
| 6 | dimensional/unit mismatch across an equation | **Major** | deterministic | same | `equation_report.rs:145-162` |
| 7 | citation refuted by the literature | **Major** | model-backed (Claude or Ollama), harness-gated | the reference's structured metadata + registry evidence | `report.rs:653-660` |
| 8 | citation supported by the literature | **Info** | same | same | `report.rs:661-667` |
| 9 | N of M citations could not be checked (collapsed into one row) | **Minor**, `ClaimKind::ProcessState` | describes Gaply's own execution | the verdict list | `report.rs:701-760` |
| 10 | citation counts resolved for N of M references | **Info** | deterministic over the registry | Semantic Scholar counts | `report.rs:1420-1443` |
| 11 | corpus text overlap, one per match | **Major** at/above threshold, else **Minor** | `HashEmbedder` cosine | manuscript chunks vs the shared corpus | `report.rs:769-815` |
| 12 | sentence-length variation below the human norm | **Minor** | deterministic stylometry | `extraction` | `report.rs:1176-1199`, ctor `:1125-1154` |
| 13 | lexical diversity (MTLD) below the norm | **Minor** | deterministic | `extraction` | `report.rs:1202-1216` |
| 14 | repeated-3-gram rate above the norm | **Minor** | deterministic | `extraction` | `report.rs:1218-1230` |
| 15 | citation density low / moderate | **Minor** | deterministic | `extraction` | `report.rs:1268-1285` |
| 16 | citation density could not be measured | **Info** | deterministic | `extraction` | `report.rs:1248-1265` |
| 17 | mixed in-text citation styles | **Minor** | deterministic | `extraction` | `report.rs:1292-1305` |
| 18 | malformed DOIs among the references | **Minor** | deterministic | `extraction` | `report.rs:1309-1322` |
| 19 | references older than 10 years, by share | **Minor** above 50 %, else **Info** | deterministic | `extraction.references[].year` | `report.rs:1098-1102`, `:1618-1676` |
| 20 | the AI-detection round-table opinion | **capped at Info** | the word-frequency proxy in practice (§2.2) | section text | `report.rs:842-892`, cap at `:458-463` |
| 21 | the extraction and RAG round-table opinions | **Info** (`pass`) or capped | deterministic | lane outputs | `report.rs:842-892` |

Plus the checklist, which is not a finding but is what a researcher reads next:
four structural rows always; the journal's own word limit, abstract limit, data
availability and required statements when the journal is profiled; up to three
keyword rows from an ingested guidelines page (§4.3).

**Two mechanical notes on the finding list** [read]. Findings identical in
`(agent, title, detail)` are merged into one row before sorting, with every
location preserved in `also_at` (`report.rs:929-968`) — this is why a manuscript
with eight missing-effect-size flags shows one row saying so. And the sort is
severity → certainty tier → confidence → agent name (`report.rs:975-988`), with
`f{N}` ids assigned after the sort (`:992-994`), which is the id scheme the proxy
payload reuses.

## 5.2 Checks that exist in the file and cannot fire

| Check | Why | Where |
|---|---|---|
| **table captions / completeness** | `table_findings` opens with `if true { return Vec::new(); }` under the comment "THE WITHDRAWAL" | `report.rs:1353-1357` |
| **uncited references** | complete, and never called; `#[allow(dead_code)]` with the reason | `report.rs:1510`, `:1607` |
| **self-plagiarism (internal duplication)** | `report()` hardcodes `self_matches: Vec::new()`; the function still exists and is still tested | `plagiarism.rs:281` |
| **small sample with causal claim** (rule 5) | removed from `RuleId::ALL`, and it deliberately emits **no `RuleOutcome`** either, so it cannot report a pass | `validate.rs:50-65`, `:330-336` |
| **reporting-standard checklist rows** | `bindings: &[]` plus the `design_independent` filter | `report.rs:1753`, `:2338-2347` |

## 5.3 The declined lanes, with their records

`DECLINED_LANES` is a `&'static [DeclinedLane]` returned on **every** outcome —
it is a property of the build, not of the manuscript
(`gaply-core/src/declined.rs:42-80`, attached at `commands.rs:1067`) [read]. It
is rendered in two places: in the slot where a lane's output would have been, and
as a "What Gaply does not assess" card (`ReviewerLetterPanel.tsx:19-32`,
`:155-159`, `:177-188`).

| Lane | Record | The measurement in the reason |
|---|---|---|
| Novelty | **§11 D166** | 2 self-contribution claims across 20 manuscripts — 0.1 per paper |
| Table arithmetic | **§11 D167** | 2 of 20 manuscripts had a totals row; the obvious check was wrong on both |
| Structured scientific claims | **§11 D165** | the extractor scored 5.9 % against a 50 % no-skill baseline |
| Chat: suggested corrections | **§10** | nothing in Gaply produces a correction |
| Chat: challenging a finding | **§10** | no re-analysis path exists |

Three further declines are recorded in code but **not** in this table, so a user
never meets them: self-plagiarism (§11 D179, `plagiarism.rs:250-281`), rule 5
(§11 D178, `validate.rs:55-64`) and table captions (`report.rs:1354`).

## 5.4 The two verdicts, and which is which

PublishReady produces **two different judgements** and shows them in different
places [read].

| | Deterministic aggregation | Cloud reviewer letter |
|---|---|---|
| Computed by | `aggregate_reviewer_verdict` (`reviewer_agent.rs:1167-1261`) | Claude Sonnet 5, then `gate_reviewer_response` (`:634-705`) |
| Inputs | the Evidence Store's per-finding severities for this run | the 12-finding / 20-row structured summary |
| Rule | any Critical → `Reject`; any Major → `MajorRevision`; ≥ 3 Minor → `MinorRevision`; else `Accept` (`:1205-1213`, threshold `:808`) | the model's own answer, then gated |
| Probability | a four-value lookup: 0.05 / 0.30 / 0.70 / 0.92 (`:815-818`) | whatever the model returned, 0–100, parsed strictly (`:649`) |
| Withheld when | evidence uninterpretable, nothing examined, or a stage undelivered (`:971-981`, `:1230-1254`) | never — it is `available: false` instead |
| Surfaces as | the **Recommendation** line on the Rust-rendered PDF (`commands.rs:973-981`, `report_compose.rs:364-394`) | the **Reviewer Letter tab** (`PublishReadyPage.tsx:410`) |
| Reaches the frontend? | **no** — carried as `shadow_reviewer` and never read (§3.4) | yes, as `reviewer` |

So the on-screen recommendation and the exported-PDF recommendation come from
two different engines and can differ. The code is explicit that the shadow is not
authoritative yet (`commands.rs:620-625`, `reviewer_synthesis.rs:21-26`), and
that the switch is a separate future decision.

`ReviewerLetterPanel` renders **no probability gauge**, and says why in a comment
at the point where one used to be: a four-value lookup drawn as a percentage ring
is an implication beyond the evidence (`ReviewerLetterPanel.tsx:111-118`).

---

# 6. THE CONTEXT FLOW

## 6.1 What never leaves

* **The manuscript file.** `run_publishready` takes a *path*
  (`commands.rs:701`); the bytes are read locally by `docparse::parse_path`
  (`pipeline.rs:424`).
* **`PipelineResult`.** The struct holds `text` — the entire manuscript — and
  has no `Serialize` derive, deliberately, so no connector or debug dump can
  serialise it (`pipeline.rs:243-251`).
* **`LocalReportModel`.** Carries `nearby_text` quotations and similarity
  excerpts; not `Serialize`, which is why the PDF has to be rendered during the
  run and is held in memory rather than persisted (`report_build.rs:106-110`,
  `state.rs:25-42`).
* **`Finding.detail`.** The field holding manuscript excerpts is never read into
  the reviewer payload (`reviewer_agent.rs:568`) or the chat payload
  (`src/screens/copilot/chatContext.ts:53`).

## 6.2 `llm_safe` — the one sanitiser

`UntrustedText` cannot be constructed without `Provenance`
(`gaply-core/src/refverify.rs:176-194`). Construction runs
`sanitize::sanitize`, which strips zero-width and control characters **first**
and then scans the normalised text (`sanitize.rs:1-8`). The denylist is
`INJECTION_SUBSTRINGS` (21 English patterns, `sanitize.rs:11-33`),
`LINE_START_MARKERS` (four, matched only at line start, `:37`), and
`INJECTION_SUBSTRINGS_NON_ENGLISH` (Spanish, Portuguese, French, German,
Italian, Chinese, Japanese, Korean, Russian, Arabic, Hindi, `:50-88`).

`llm_safe()` is the **only** prompt-safe accessor, and it is all-or-nothing: any
flag redacts the entire string (`refverify.rs:211-217`). `display_raw()` exists
for UI display and its doc says never to put it in a prompt (`:205-208`).

## 6.3 The reviewer payload, field by field

`build_review_payload` (`reviewer_agent.rs:542-614`) [read]. **Every string goes
through `safe()`** — `llm_safe` then clamp to 400 chars — including strings
Gaply wrote itself, and the function's doc explains why: `equation_report`'s
titles embed the manuscript's own equation line, so "which fields are untrusted"
is a property of every finding constructor in the crate and cannot be settled by
reading one file (`reviewer_agent.rs:261-297`, ctor at `equation_report.rs:132`).

```
{ task: "publishready_review",
  run_id: safe(...),                 // metering; server dedups
  instruction: REVIEWER_INSTRUCTION, // reviewer_agent.rs:78-92
  summary: {
    journal:          { name: safe(...), quartile: safe(...) },
    overall_verdict:  safe(report.verdict),        // "pass" | "concern"
    findings_omitted: <count dropped by MAX_FINDINGS>,
    findings: [ up to 12 × { id, agent, tier, severity,
                             title: safe(...),      // title ONLY, never detail
                             confidence,
                             evidence: [ structured provenance only ] } ],
    checklist: [ up to 20 × { id, requirement: safe(...), passed } ],
    supplementary: { present: false, note: "no supplementary data provided" },
  } }
```

The evidence array is filtered by `is_structured_provenance` before `safe()`
(`reviewer_agent.rs:557-562`) — only `rule:`, `evidence:`, `swarm:`, `agent:`,
`gate:`, `similarity:`, `match_type:`, `source:`, `signal:` prefixes survive.

The proxy then forwards **only** `summary` and `instruction`
(`gaply-proxy/app/claude_client.py:54-56`), after the 8000-character validator
(`validation.py:14-17`).

## 6.4 The citation-verification payload

`bundle_citation` (`gaply-core/src/verify_agent.rs:150-227`) [read]. Per
citation: the *claimed* structured bibliographic fields (`authors`, `year`,
`title`, `doi`) and an evidence array. `Reference.raw` — the verbatim
reference-list line — is never serialised (`:217`). Every fetched string goes
through `llm_safe()`; abstracts are excluded entirely as the largest injection
surface (`:208-212`).

Note precisely: `reference.authors`, `reference.year`, `reference.title` and
`reference.doi` **are manuscript-derived** and are sent as-is — they are
structured bibliographic fields, not `llm_safe`'d. The `llm_safe` treatment is
applied to the *fetched* side (`matched_title`, `matched_authors`, retraction
reasons, venue).

The payload is budgeted client-side at 7600 characters against the proxy's 8000,
counted by a function that mirrors the server's `_string_leaves` exactly
(`verify_agent.rs:276-296`).

## 6.5 The other outbound paths

| Path | Where to | Carries | Gate |
|---|---|---|---|
| Reference verification | `api.crossref.org`, `api.openalex.org`, `api.labs.crossref.org`, `api.unpaywall.org`, `api.semanticscholar.org` | reference DOIs and titles from the manuscript, as query strings, plus a fixed contact address `mailto:rishirajsharma8055@gmail.com` in the User-Agent and in the Unpaywall `email` parameter | `NetworkConsent` (`pipeline.rs:558`) |
| Guidelines ingestion | the URL the user typed | nothing of the manuscript — an outbound GET | the same `mayUseCloud('publishready')` check, before the run starts |
| Reviewer letter | gaply-proxy `/verify` → Claude Sonnet 5 | §6.3 | consent + App Check + user JWT + proxy reachability |
| Citation verdicts | proxy, or local Ollama, or nothing | §6.4 | same |
| Escalation | proxy `/verify` with a `task:"escalate_findings"` payload | per-finding evidence records | degrades to "unavailable" — the endpoint does not exist server-side (`escalation.rs:24-32`) |
| Research Copilot | proxy `/verify` | the user's question, finding **titles** (not details), structured provenance, and citation metadata | `mayUseCloud('research_copilot')` |

Endpoints: `gaply-core/src/refverify.rs:468`, `:474`, `:749`, `:754`, `:989`,
`:1050`, `:1092`. Contact string and User-Agent:
`src-tauri/src/http_fetcher.rs:73-78`.

Copilot payload: the frontend maps `summary: f.title` and omits `detail`
(`src/screens/copilot/chatContext.ts:49-55`); the Rust side re-applies the
discipline anyway — `safe_clamp` on every summary, RAG text and citation title,
structured provenance only, bounded to 12 findings / 4 RAG / 8 citations
(`gaply-core/src/chat_agent.rs:323-395`, `:62-66`, `:291-305`). A ghostwriting
request is refused by local code before the proxy is even probed
(`commands.rs:3014-3018`).

## 6.6 Consent, stated exactly

`mayUseCloud` reads `localStorage['gaply.settings.privacy']`
(`settingsStore.ts:39-67`) [read]. It is **opt-out**: a missing key means every
suite is allowed (`:39-44`). The PublishReady gate is a frontend check
(`PublishReadyPage.tsx:223`); the Rust side records that it *inherits* the
frontend's word by passing `NetworkConsent::Granted` unconditionally, with a
comment saying exactly that and that it is still a preference rather than a
boundary (`commands.rs:820-830`). The general analysis command
(`run_full_analysis`) takes `allow_network` as a required parameter instead
(`pipeline.rs:186`), which is the shape PublishReady does not yet have.

---

# 7. WHERE THE ARCHITECTURE DOCUMENT AND THE CODE DISAGREE

**The code wins in every row below.** Both are named.

| # | `docs/publishready-premium-architecture.md` says | The code does | Verdict |
|---|---|---|---|
| 1 | §6 table, line 926: *"Cloud judgement — **OpenAI via proxy**"*; line 928 the same for chat | The proxy's default provider is `claude` and its default model is **`claude-sonnet-5`** (`gaply-proxy/app/config.py:13`, `:17`, `:76-77`). OpenAI is a non-default alternative selected by `GAPLY_LLM_PROVIDER` | **Code wins.** Claude Sonnet 5 unless an operator sets the env var |
| 2 | §6 line 925: *"Local judgement — Qwen2.5-3B (on demand)"* | No 3B exists anywhere in the resolver. The tiers are 7B / 1.5B / heuristic (`models/mod.rs:448-456`), plus the bundled **0.5B** used by AI Check and the AI engine | **Code wins.** There is no 3B |
| 3 | §6 line 927: *"Deep research on the journal — OpenAI with search, via proxy"* | No search-enabled call exists. The journal layer is a bundled snapshot plus one optional single-page fetch (§4) | **Code wins.** Not built |
| 4 | §1.1 line 54: a `ResearchProfile` router that activates 25–60 specialists per paper | No `ResearchProfile` in the tree [ran]. `specialist::shipped()` returns **three**, and `run_pipeline_inner` runs **two** (`pipeline.rs:794-801`) | **Code wins.** Two specialists, and their output reaches no surface |
| 5 | §1.1 line 60: the SPSS parser feeds "the methodological cluster" | `analysis::spss` has no caller outside tests; `SpecialistInput.analysis` is `None` at the only production call site (`pipeline.rs:795`) | **Code wins.** Unwired |
| 6 | §4.5: reviewer lenses as a product layer | `review_lens` is referenced only by an example (§3.4) | **Code wins.** No caller |
| 7 | §4.6: novelty, significance and claim strength as three outputs | `novelty.rs` opens *"DECLINED — §11 D166"*; the decline is surfaced to users via `DECLINED_LANES` | **Code wins.** Declined on measurement |
| 8 | §3.1: the Manuscript layer's privacy class as *"who may read it"* | `agent_graph`'s consent rule is about **egress**, not reading — any agent may read any layer; a cloud agent needs a covering scope to transmit one. The doc's reading would have required premium consent to parse a file the user just opened. CLAUDE.md records this correction | **Code wins**, and this one is already recorded |
| 9 | §6 line 934 (a v5 correction, and the one row where the doc is *right*): the perplexity ladder is `Full7B / Mini / HeuristicOnly` and a model table omitting it describes half the models | Confirmed: `models/mod.rs:446-456`. On a stock install the ladder resolves to `HeuristicOnly` (§2.2) | **Agreement**, with the measured resolution added |

One more disagreement is internal to the code rather than against the doc, and
belongs here because a reader of either will be misled: §4.5 of this document —
the two-verdict split. The architecture document treats "the recommendation" as
one thing. In the code it is two, computed by different engines, surfaced in
different places (§5.4).

---

# 8. THE SHORT ANSWER

PublishReady is a **six-lane local analysis pipeline with one cloud call on top**.

* Five of the six lanes are local and four of those are deterministic. The one
  statistical lane — AI detection — resolves to a word-frequency table on any
  machine without a manually-installed 7B or 1.5B GGUF, which includes every
  stock install and this development machine [ran].
* The "semantic" machinery — plagiarism similarity, RAG retrieval — runs on a
  feature-hashing bag-of-words encoder, not embeddings (§2.3). The real BGE
  embedder exists in the app and is not on this path.
* The round-table debate is real code that runs on every analysis, but five of
  six participants cannot change their answer by construction, so it converges in
  round one unless the verification agent reconsiders — which needs a reachable
  proxy (§3.2).
* The single deterministic hard constraint — the statistics validator — is four
  rules, not five (§5.2).
* The cloud reviewer is Claude Sonnet 5 behind an App Check + entitlement +
  prose-validator proxy, receives at most 12 finding titles and 20 checklist
  rows, and every claim it makes must cite an id it was given or is dropped
  (§2.4, §6.3).
* There is no tier enforcement anywhere on the run path; entitlement gates the
  screen and the proxy meters the call (§3.4).
* The journal layer is a 213-requirement snapshot compiled into the binary, plus
  one optional single-page fetch. The live crawler is a development tool with no
  Tauri command. Five of the ten seeded journals were unreachable from the picker
  because two committed files spelled their keys differently; fixed and guarded
  on 18 Sep 2026, taking the picker from five journals and 124 reachable
  requirements to nine and 191 (§4.5).
* The manuscript never leaves the device. What leaves is: structured finding
  titles and checklist rows to the proxy; reference DOIs and titles to five
  public scholarly APIs; and a GET to whatever guidelines URL the user typed.
