# PublishReady Premium at HEAD — what of the design exists, measured

**HEAD:** `5e06f7d` (main, 3 Oct 2026).
**Design:** `docs/publishready-premium-architecture.md` (1,779 lines, in the
repo). **Prior audit:** `docs/PUBLISHREADY_REACHABILITY.md`, at `c9feb09`.
**Mode:** measurement only. No fixes, no implementation. This file is the only
committed output.

**Provenance tags, on every verdict.**
* `[src]` means source inspection at HEAD.
* `[probe]` means a live run at HEAD (§6).
* `[stored]` means a figure read from a decision record and not re-measured.

**The chain.** A capability is REACHABLE when three links hold:
1. a screen under `/app/*` invokes a command;
2. the command is registered in `generate_handler!` (`src-tauri/src/lib.rs:183`);
3. the command reaches the code.

Code under `#[cfg(test)]`, `examples/` or `src/bin/` is not production.

**Verdicts.**
* **NOT BUILT.**
* **BUILT, UNREACHABLE**, which names the one missing piece.
* **PARTIAL**, which means it runs and its output is dropped.
* **REACHABLE.**

---

## 1. ResearchState and the evidence graph

| capability | verdict | evidence |
|---|---|---|
| `ResearchState` | **PARTIAL** `[src]` | Built on every run at `pipeline.rs:823` (`from_extraction`). That is **after** `compile_report` (`:743`) has run and the report is cached (`:777`). Stored in `PipelineResult.research_state` (`:836`). **No production code reads any field.** Every read is in the `pipeline.rs` tests (`:1464`–`:1576`). `PipelineResult` is not `Serialize`. `run_publishready_measured` reads only `lanes` and `report_model`, and `report_model` never touches it. No frontend file names it |
| `science` layer inside it | **always `None`** `[src]` | No agent `requires` `scientific_extraction` (`agent_graph.json`), so `requires_scientific_extraction()` is false and `pipeline.rs:443` takes the plain path. This is pinned by `a_real_pipeline_run_does_not_derive_the_declined_scientific_layer`. The layer is declined (§11 D165). `research_state.rs:41-46` says it is `Some` on a real run, which is stale |
| `EvidenceGraph` (`research_state.rs:183`) | **PARTIAL** `[src]` | Built as `ResearchState.evidence` on every run. With `science: None`, only `CitesReference` and `StatisticCoLocatedWithTable` edges can exist, and the claim-edge branch (`:365`) never runs. `of_kind` and `.edges` are read only in tests and in `examples/claim_evidence_audit.rs` |

**Does anything read them? No** `[src]`. The missing piece for both is **a
reader in the report path**.

## 2. The agent graph and `AgentSpec`

| question | answer |
|---|---|
| agents declared | **9** `[src]`: `agent_graph.json`, pinned by a test at `agent_graph.rs:1111`. They are extraction, validation_maths, ai_detection, plagiarism, rag, verification, frequentist_stats, ml_methodology and claim_evidence_strength |
| agents that run | **8** `[src]`. Six lanes are hard-coded in `run_pipeline_inner` (`pipeline.rs:423`–`:546`). Two specialists run at `:647-654`. **`ml_methodology` is withheld** at `pipeline.rs:305-322`: its applicability gate admitted a histology paper |
| is the graph used at runtime? | **Validated, not dispatched** `[src]`. `shipped_graph()` is called once (`pipeline.rs:420`). Its only effect is the always-false `requires_scientific_extraction()`. `execution_order()` (`agent_graph.rs:596`) has **no production caller**. The file says so itself: *"This is DECLARATION, not dispatch"* (`agent_graph.rs:158`) |
| graph-driven execution | **NOT BUILT** `[src]`. `run_pipeline_inner` is a fixed sequence |
| is any agent revising? | **One, conditionally** `[src]` `[probe]`. Five debate participants are `PrecomputedAgent`s, and their `revise` is the trait default `None` (`swarm.rs:102`). Only `RevisingVerificationAgent` (`pipeline.rs:690`, `swarm/revising.rs:76-123`) can revise. It does so once, when a peer's answer contradicts its own, by asking the proxy client again. So 3 rounds can never happen. **Live (§11): `rounds_run: 1`, `revised_agents: []`** |
| specialists | `frequentist_stats` is **REACHABLE for one code**: only `parametric_test_assumptions_unstated` passes `WIRED_SPECIALIST_CODE` (`report.rs:1146`, `:1203`). Its other codes are dropped, which is **PARTIAL**. `claim_evidence_strength` **runs, and all its output is dropped (PARTIAL)**. `ml_methodology` is **BUILT, UNREACHABLE (a decision)** `[src]` |

Stale comments, recorded and not fixed:
* `agent_graph.rs:1128` says no specialist runs.
* `pipeline.rs:285` and `:326` say `compile_report` does not read specialists.
* `report.rs:420` says every participant is precomputed.

## 3. The reviewer lenses and the editor rubric

The design (§4.5) names **6 lenses**. Its v8 note ships 4 and declines 2.

| capability | verdict | missing |
|---|---|---|
| Methodology, Statistics, Novelty & literature, Reporting & ethics lenses (`review_lens::lenses()`, `:1531`) | **BUILT, UNREACHABLE** `[src]` | **A production caller that builds a `LensInput`.** `lenses()`/`review()` are called only from `red_team::run_pipeline`, the `exports.rs` and `chat_scope.rs` tests, and examples. The app crate references `review_lens` nowhere except a comment (`pipeline.rs:326`). Its one production use is a word list (`HUMAN_SUBJECTS_PRESENT`) in the checklist |
| Journal fit, General lenses | **NOT BUILT, declined** `[src]` | `DECLINED_LENSES` (`review_lens.rs:1575`) |
| Editor rubric `editor::decide` (`editor.rs:166`) | **BUILT, UNREACHABLE** `[src]` | **A caller.** Its input is the lenses' `ReviewerReport`s, so it follows the lenses. Its posture was measured uninformative: 18 of 20 MAJOR REVISION (design §8) `[stored]` |
| Editor's disagreement-map input (§5.7) | **NOT BUILT** `[src]` | `editor.rs:14` says *"nothing produces one"* (§5.3 unbuilt) |

## 4. Novelty, significance, claim–evidence strength (§4.6)

| capability | verdict | evidence |
|---|---|---|
| Novelty (`novelty.rs`) | **BUILT, UNREACHABLE by decision** `[src]` | The header opens *"DECLINED — §11 D166. This module is the INSTRUMENT that measured the decline"*. Called only from examples. **What a user sees** on the novelty card (`ReviewerLetterPanel.tsx:165-175`) is one of two things: the cloud reviewer's own `novelty_assessment` when it is grounded in a sent finding id (proxy only), or the D166 decline sentence. Neither comes from `novelty.rs` |
| Significance (what the work would change) | **NOT BUILT** `[src]` | Every `significance` in the code is statistical. The only other use is the name of a lens criterion fed by the declined novelty source |
| Claim–evidence strength (`specialist/claim_strength.rs`) | **PARTIAL** `[src]` | Runs on every PublishReady run (`pipeline.rs:647-654`). `specialist_findings` drops every code but the one frequentist code, so its output reaches no report and no screen |

## 5. `AnalysisRecord`

| capability | verdict | evidence |
|---|---|---|
| SPSS parser (`analysis/spss.rs:34`), used for `.sps` and `.jnl` | **BUILT** `[src]` | It is the only parser. `.jnl` is SPSS's journal, not Stata/JASP |
| R / Python / ipynb / CSV parsers | **NOT BUILT** `[src]` | `analysis_ingest.rs:134-141`: *"Gaply has no parser for .{other} yet"* |
| Upload (`analysis_ingest` command) | **PARTIAL** `[src]` | Reachable from `PublishReadyPage.tsx:232`. It parses, copies counts into the summary, and **drops the record**: there is no table, no storage, and no reader |
| Record reaches a run | **BUILT, UNREACHABLE. Confirmed: `analysis: None`** `[src]` | `pipeline.rs:648`: `SpecialistInput { extraction: &extraction, science: None, analysis: None }`. It is the only `analysis:` in the file. The frontend's `run` invoke carries no analysis argument. The D191 cross-check (`frequentist.rs:255`, `:336`) is gated on `Some` and has never run on a real pair. Its own comments record 29 of 31 false on the one pairing and the logistic→linear defect (§11 D191) `[stored]` |

## 6. The mathematical engine (§6b)

The design names **Tier 0 only** for the engine (§6b.2, *"Every one of these is
Tier 0"*), with five checks.

| check (§6b.2) | verdict | evidence |
|---|---|---|
| 1. Symbolic equivalence | **REACHABLE** `[src]` | `equation/equiv.rs:115`, reached from `check.rs:442`. In `compile_report` it runs via `equation_findings` (`report.rs:914`, `:1117`) |
| 2. Dimensional consistency | **REACHABLE** `[src]` | `equation/units.rs:372` `check_sides` → `dimension_finding` |
| (arithmetic of the manuscript's own equations) | **REACHABLE** `[src]` `[probe]` | `check_equation` → `arithmetic_finding`. Live on 2 Oct: Health Economics *"Arithmetic requires author confirmation: Weighted provision = …"* (`PUBLISHREADY_AT_HEAD_2026-10-02.md`) |
| 3–4. Numerical substitution / formula recomputation (SE, CI, effect sizes, t, ANOVA, r, χ², OLS) | **REACHABLE outside PublishReady** `[src]` | `stats_verify.rs` / `stats_verdict::verify_analysis`, through `run_stats_verify` and `run_stats_chat` (`commands.rs:3144`, `:3171`; registered `lib.rs:303-304`). Mounted at `/app/statsverifier`. It needs an uploaded data table and a spec, **not** an `AnalysisRecord`, and **never runs in the PublishReady pipeline** |
| 5. Equation ↔ text ↔ code ↔ result consistency | **NOT BUILT** `[src]` | `BindingSource` has only `Substitution` and `Declaration` (`equation/graph.rs:84-90`), with no analysis-record source |
| `EquationGraph` | **REACHABLE (internally)** `[src]` | Built on every run (`report.rs:1121`), consumed by its own caller, not persisted |
| `validate.rs` (Tier 0 statistics rules) | **REACHABLE** `[src]` `[probe]` | Four rules (`RuleId::ALL`), with rule 5 declined (D178). In the Statistics tab. Stats Check (`validate_manuscript`) runs this and nothing from §6b |

## 7. The journal layer — what is live

| capability | verdict | evidence |
|---|---|---|
| Bundled seed | **REACHABLE** `[probe]` | **106 journals, 643 requirements, 146 bindings**. Read out of the 2 Oct DMG binary: 0 spans at exactly 400 characters |
| Startup reconciles | **REACHABLE** `[src]` `[probe]` | D225, D238, D240 and D239 run at startup (`lib.rs:108-165`). On a fresh seed they remove 0 |
| Checklist with journal rows | **REACHABLE** `[probe]` | Through `run_publishready` with the picker's key. On 2 Oct: 19 journal rows on six manuscripts, 13 decided, 13 correct on a hand-read |
| Single-source quote, undecided `–` | **REACHABLE** `[src]` | `ReportViewerPage.tsx:488`, `:526` (D259, D263) |
| Reporting-standard binding rows | **BUILT, UNREACHABLE by decision** `[src]` | `design_independent` + `bindings: &[]` (`report.rs:1926`). The design gate is declined (D177) |
| Journal picker list (`journal_profiles`) | **REACHABLE** `[src]` | `PublishReadyPage.tsx:151` |
| Journal fingerprint UI (`JournalPicker`, `JournalFingerprintView`, `ChecklistDisplay`) | **BUILT, UNREACHABLE** `[src]` | **A mount.** `JournalPicker` is referenced by no other file. `JournalFingerprintView` is used only inside it. `ChecklistDisplay` is referenced nowhere. The `journal_fingerprint` command is registered, and its bridge method has no caller |
| Guidelines paste (`ingest_guidelines`) | **REACHABLE for profiled journals** `[src]` | Keyed by `key_for_url` |
| Crawl | **not in the app** `[src]` | Production uses only `CrawlBudget` and `key_for_url` (`guidelines.rs:331`, `commands.rs:564`). The seed is a build-time snapshot |
| §8 Stage 2 comparative position | **NOT BUILT** `[src]` | A fetcher exists (`journal_openalex::recent_papers`, `:65`) with no caller outside its own module |
| §8 Stage 3 calibrated probability | **NOT BUILT, deliberately** `[src]` | The design blocks it until outcome data exists. `publication_probability` reaches no screen (`reviewer_agent.rs:664`) |

## 8. The marked-up manuscript and the four exports (§9)

The design names four exports: **annotated manuscript, reviewer letter,
readiness rubric, audit trail** (line 1276).

| export | verdict | missing |
|---|---|---|
| Annotated manuscript `exports::annotate` (`exports.rs:132`) | **BUILT, UNREACHABLE** `[src]` | **A command and a render**. Its input is lens output, so it follows the lenses. The only annotated view a user reaches is the thesis audit's `AnnotatedManuscript.tsx`, a different lane |
| Reviewer letter `exports::letter` (`:230`) | **BUILT, UNREACHABLE** `[src]` | A command and a render (after the lenses). The letter a user sees is the cloud `reviewer_agent` letter, not this |
| Readiness rubric (`editor::decide`) | **BUILT, UNREACHABLE** `[src]` | §3 |
| Audit trail `exports::audit_trail` (`:270`) | **BUILT, UNREACHABLE** `[src]` | A command and a save path |
| Suggested edits (§9) | **NOT BUILT** `[src]` | `chat_scope.rs:106-123` |

**Exports a user can reach today, none of them the four** `[src]`:
* the summary PDF (`downloadReportPdf`, `PublishReadyPage.tsx:407`);
* the full PDF (`export_publishready_pdf`, `:424`), rendered during the run:
  21,435 bytes on the live run `[probe]`;
* the AI-check HTML;
* the thesis-audit PDF/HTML;
* the notes manuscript `.docx`.

## 9. Chat (§10)

The design names four modes and ships two: **Explain, Evidence** ship;
**Correction, Challenge** are declined.

| mode | verdict | missing |
|---|---|---|
| Explain, Evidence (`chat_scope::answer`, `:297`) | **BUILT, UNREACHABLE** `[src]` | **A command**, plus the lens output it reads, plus a mode control. `chat_scope` has no production reference. The dock is free text |
| Correction | **NOT BUILT, declined** `[src]` | A suggested-edit producer (§8 above), then a decision |
| Challenge | **NOT BUILT, declined** `[src]` | A decision ledger, incremental re-analysis (§5.5), and a graph executor. `struct Decision` and `revision_history` have 0 hits |
| Copilot (`run_copilot_chat` → `chat_agent`) | **REACHABLE with a proxy only** `[src]` | `ProxyReqwestClient::from_env()` + `reachable()` (`commands.rs:3057`). With no proxy it shows `NEEDS_CLOUD_MESSAGE`: *"Answering questions needs Gaply's cloud connection, which isn't reachable right now…"* (`chat_agent.rs:188`) |

**Contradiction, recorded:** `chat_agent::CHAT_INSTRUCTION` (`chat_agent.rs:92`)
tells the model to *"suggest what to fix and why"*. That is the Correction mode
that §10 and `DECLINED_LANES` say chat cannot do `[src]`.

## 10. The cloud path

| question | answer |
|---|---|
| where OpenAI is called | **Only in the proxy** `[src]`: `gaply-proxy/app/openai_client.py:68`, POST to `api.openai.com/v1/chat/completions`, default `gpt-4o-mini`. **The proxy's default provider is Claude** (`config.py:17`, `GAPLY_LLM_PROVIDER`). OpenAI is used only when that is set to `openai`. **No Rust code calls OpenAI.** Every Rust hit is a comment or a test fixture |
| proxy routes | `GET /health`; `POST /verify` (the only model route). There is **no `/copilot` route** (`chatBridge.ts:61` throws *"not wired yet"*) `[src]` |
| what gates the app | 1. `GAPLY_PROXY_URL`, else **`http://127.0.0.1:8080`** (`proxy_client.rs:41`). 2. The App Check signing key in the keychain (`signer_from_env`, `:194`). 3. `GET /health` 2xx. 4. The user JWT (`X-Gaply-User-Token`), enforced by the proxy only if `entitlement_required` (default **False**, `config.py:43`). 5. `NetworkConsent`. `run_publishready` hard-codes `Granted` (`commands.rs:851`), and the real gate is the frontend `mayUseCloud`, a localStorage preference `[src]` |
| default proxy configured? | **No** `[src]`. No proxy URL in `tauri.conf.json` or any `.env*` file. The only remote URL is inside `#[cfg(test)]` |
| what a user sees, no proxy | **Citations:** *"25 of 25 citation(s) could not be checked … This check needs the citation verification service, which was unavailable for this run — it is not a finding about your references"* `[probe]`. **Reviewer letter:** before the run, *"The reviewer letter needs Gaply's cloud proxy, and no proxy is configured on this machine…"* (`commands.rs:700`); after it, **REVIEWER UNAVAILABLE**, *"deep reasoning requires cloud analysis — unavailable offline"* `[probe]` `[src]`. **Copilot:** `NEEDS_CLOUD_MESSAGE` `[src]` |

**And the declines are hidden in exactly this case** `[src]`.
`ReviewerLetterPanel.tsx:82` returns early when `letter.available === false`.
The *"What Gaply does not assess"* card is at `:194`, below that return. So a
user with no proxy never sees the five declined lanes, although the outcome
carries them (§11).

## 11. The SLMs

**D200** `[stored]` (`AI_ENGINE_PLAN.md:15430`): *"SLM-1 is an AI-text detector,
and its adapter does not beat its own base"*. The base scored 18/20 and the
tuned model 17/20, on 20 items. No accuracy is claimed. *"Nothing is wired into
the product."*

**D201** `[stored]` (`:15585`): *"SLM-2 measured on its own task: the base
carries no signal, the adapter is unusable on the shipped path, and the prompt
presupposes its verdict"*. It scored 9/20 on generated-versus-paraphrased,
below no-skill. The adapter took a median 329 s per item, and 5 of 10 items
returned no answer. 9 of 10 human abstracts were put in a machine category.
*"Nothing is wired into the product."*

| SLM | on a live path? |
|---|---|
| SLM-1 7B (candle, base GGUF; no LoRA path) | **Gated** `[src]`. The PublishReady AI lane uses it only with **≥ 15 GiB RAM** (`models/mod.rs:444`) and a GGUF in `GAPLY_SLM1_GGUF` or `~/gaply-models/slm1`. It is **not bundled** |
| SLM-1 mini 1.5B | **Gated** `[src]`: needs `~/gaply-models/slm1-mini`, which is not bundled |
| Stage-1 0.5B (bundled, 398 MB) | **REACHABLE in AI Check only** (`run_aicheck` → `stage1_lm_model()`), never in PublishReady `[src]` |
| SLM-2 (Ollama `qwen3:4b`) | **Fallback tier for citation verdicts**, only if the user runs Ollama locally (`models/mod.rs:1100`). `reachable()` checks `/api/version` only, not that the model is pulled `[src]`. Its AI Check classifier role has no production caller (`aicheck_classifier`) |
| D200/D201 adapters | **NOT on any path** `[stored]` `[src]`. There is no LoRA path in the runtime |

**On this machine (8 GiB, nothing in `~/gaply-models/slm1*`), the live run's AI
lane said:** *"Scored by a WORD-FREQUENCY PROXY, not a language model — no deep
model ran on this device"* `[probe]`. That is what a fresh install gets
anywhere: the bundled 0.5B is not used by the pipeline.

## 12. Other named capabilities

| capability | verdict | missing |
|---|---|---|
| Shadow reviewer synthesis (Box 4) | **PARTIAL** `[src]` `[probe]` | Computed on every run, and **no non-test frontend file reads `shadow_reviewer`**. Live it produced `recommendation: major_revision`, with warnings excluding f7–f10. A decision (Stage 2 "switch"), then a render |
| Debate / round-table | **REACHABLE** `[src]` `[probe]` | `report.debate`, rendered on the Overview |
| Premium gate `run_premium_gate` (`release_gate.rs:223`) | **BUILT, UNREACHABLE** `[src]` | Called only in tests (`release_gate.rs:843+`, after `#[cfg(test)]` at `:548`). **A decision**: no tier check exists |
| Release gate (promote / hold / rollback, §6c.3) | **BUILT, UNREACHABLE** `[src]` | Its only caller is `examples/release_gate.rs`. **A caller** (CI or command) |
| Memory `record_episode` (§6c.6) | **BUILT, UNREACHABLE** `[src]` | Called only in its test (`memory.rs:87`). **A caller** |

---

## 13. The three counts

Counted over the rows above. A design capability is one row: the four shipped
lenses are one row, and Explain/Evidence are one row.

### 1. Built and unreachable: **14**

| # | capability | the single thing missing |
|---|---|---|
| 1 | Four reviewer lenses | a production caller that builds `LensInput` |
| 2 | Editor rubric | a caller (after #1) |
| 3 | Annotated manuscript export | a command + render (after #1) |
| 4 | Reviewer-letter export | a command + render (after #1) |
| 5 | Audit-trail export | a command + save path |
| 6 | Chat Explain / Evidence | a command over `chat_scope::answer` (after #1) |
| 7 | `AnalysisRecord` into a run | a caller passing `analysis: Some` |
| 8 | Novelty module | a decision (D166) |
| 9 | Reporting-standard rows | a decision (the D177 design gate) |
| 10 | `ml_methodology` specialist | a decision (applicability gate) |
| 11 | Journal fingerprint UI | a mount |
| 12 | `run_premium_gate` | a decision (no tier exists) |
| 13 | Release gate | a caller |
| 14 | Memory `record_episode` | a caller |

### 2. Not built at all: **12**

1. significance
2. graph-driven execution
3. the editor's disagreement map
4. equation ↔ text ↔ code ↔ result consistency
5. R / Python / ipynb / CSV parsers
6. suggested edits
7. chat Correction
8. chat Challenge
9. the Journal-fit lens
10. the General lens
11. §8 Stage 2
12. §8 Stage 3

Six of these are **declined or deferred by the design itself**: Correction,
Challenge, Journal fit, General, Stage 2 and Stage 3.

**Partial (runs, output dropped): 6.**
1. `ResearchState`
2. `EvidenceGraph`
3. `claim_evidence_strength`
4. the dropped `frequentist_stats` codes
5. the analysis upload's record
6. the shadow reviewer

### 3. Of the 14 unreachable: wiring alone, or a corpus result first

**Reachable without new measurement: 4.**
* journal fingerprint UI (a mount)
* release gate (a caller)
* memory (a caller, or deletion)
* `run_premium_gate` (a product decision, not a measurement)

**Need a corpus result first: 10.**

| capability | why |
|---|---|
| Lenses, editor, the three exports, Explain/Evidence (6) | All read lens output, and **lens quality on real manuscripts is unmeasured**. The one figure that exists says the editor's posture is uninformative (18 of 20 MAJOR REVISION) `[stored]`. The anchor rate is 28 of 137 findings, 20.4% `[stored]` |
| `AnalysisRecord` | The D191 cross-check fired 29 of 31 false on its one pairing, and has a known logistic→linear defect `[stored]` |
| Novelty | D166 declined it on 20 manuscripts `[stored]` |
| Reporting-standard rows | D177 declined the design gate `[stored]` |
| `ml_methodology` | Its gate admitted a histology paper `[src]` |

---

## 14. What a premium user gets today `[probe]`

**Setup.**
* `run_publishready_measured`, the body of the `run_publishready` command,
  with `NetworkConsent::Granted` as the command sets it.
* A database seeded and reconciled in startup order: 106 / 643 / 146, all four
  reconciles 0.
* Journal key `expert-systems-with-applications`.
* Release build, the app's default model tier.
* Manuscript: `R PAPER .docx` (`effbb86c2495`).

**This machine.** 8 GiB RAM. Nothing listening on `127.0.0.1:8080` (proxy) or
`:11434` (Ollama). `verify_backend_label`:
*"mock (no cloud proxy; Ollama unreachable)"*.

**Run with `GAPLY_SKIP_KEYCHAIN=1`.** An App Check key IS in this machine's
keychain. An unsigned probe binary is not on its access list and would block on
an OS prompt. The skip takes the key-absent branch of `verify_proxy_with`,
which falls through exactly as the signed app does when `/health` fails. So
the result is the same for a user with no proxy deployed. **Elapsed: 4.3 s.**

**What "paid" means in this build** `[src]`:
* Entitlement is read from Supabase (`entitlement.ts`). PublishReady refuses
  `not_entitled` users with a teaser (`PublishReadyPage.tsx:339`).
* **This checkout has no Supabase configuration** (only `.env.example`). So the
  DMG built from it on 2 Oct reads `unverifiable_no_account`, and **no user of
  that build can be entitled**.
* In a build that has Supabase, a paying user unlocks the same run. The paid
  work is the reviewer letter, citation verdicts and Copilot, and all of it
  needs the proxy.

**Exactly what came back:**

| field | value |
|---|---|
| verdict | **`concern`**, `combined_confidence 1.0`, `overridden_by_constraint: true` |
| debate | `rounds_run 1`, `converged`, `revised_agents []` |
| findings | **10**: 3 major, 4 minor, 3 info |
| major | *statistical rule failed: missing effect size (raised at 2 places)*; *missing confidence interval*; *No statistical assumption check detected: t-test* |
| minor | *lexical diversity deviates from the academic reference*; *mixed in-text citation styles (80% dominant)*; *18 of 24 dated reference(s) are older than 10 years*; **_25 of 25 citation(s) could not be checked_** |
| info | *Extraction: pass*; *AiDetection: concern* (word-frequency proxy); *Rag: pass* |
| checklist | 6 rows. The four structural rows PASS. *abstract limit 250*: PASS, 181 words. *competing interests statement*: FAIL |
| reviewer letter | `available: false`. *"deep reasoning requires cloud analysis — unavailable offline"*. `recommendation: unknown`, 0 issues. The novelty assessment is empty |
| shadow reviewer | computed: `major_revision`, 0 issues, with *"reviewer narrative unavailable: cloud proxy not reachable"*. **Not rendered** |
| declined lanes | 5: Novelty, Table arithmetic, Structured scientific claims, Chat: suggested corrections, Chat: challenging a finding. **Not shown**, because of the early return in §10 |
| proxy payload | built (`publishready_review`, 10 findings, 6 checklist rows, journal ESWA) and **not sent**: there is no proxy |
| PDF | rendered, 21,435 bytes, available via "Open full report" |

**Compared with the consent-denied probe** (`PUBLISHREADY_AT_HEAD_2026-10-02.md`,
9 findings), the real command path adds exactly one finding: the 25-of-25
unchecked citations. Everything else is identical.

**In one line:** a paying user today gets the free tier's local report plus a
statement that their 25 citations were not checked. The reviewer letter, the
citation verdicts and Copilot need a proxy that is not deployed. None of the
§4–§10 premium capabilities appears on screen.

---

## 15. Limitations

* **Delegated reads.** Five read-only tracing passes produced most `[src]`
  rows, each citing `file:line`. I re-checked these directly at HEAD:
  * `analysis: None` (`pipeline.rs:648`, the only hit);
  * `ResearchState` built after `compile_report` and read only in tests;
  * 9 agents in the graph;
  * no `review_lens::` / `editor::` / `exports::` / `chat_scope::` /
    `novelty::` / `red_team::` reference in the app crate;
  * `WIRED_SPECIALIST_CODE`;
  * no non-test frontend reader of `shadow_reviewer`;
  * `equation_findings` in `compile_report`;
  * `stats_verdict` only in the Stats Verifier commands;
  * the D200/D201 headings and the 15 GiB floor;
  * the `ReviewerLetterPanel` early return;
  * the callers of the journal UI, the premium gate, the release gate and
    memory.

  Other line numbers are as the tracing passes reported them.
* **The count is a judgement about granularity.** Splitting the lenses into four
  rows, or Explain/Evidence into two, changes the totals. The rows are listed
  so a different granularity can be re-counted.
* **One manuscript, one journal** for §14. It shows what the path produces, not
  a rate.
* **No proxy, no Ollama, no Supabase.** The paid path's cloud outputs were
  classified from source, not observed.
* **No frontend was driven.** "Not rendered" and "not shown" are from source.

## 16. Probes run

| probe | input | output |
|---|---|---|
| `examples/zz_premium_user.rs` (release, **untracked**) | `R PAPER .docx`, `expert-systems-with-applications`, `GAPLY_SKIP_KEYCHAIN=1` | §14; full outcome JSON |
| `lsof`, `sysctl hw.memsize`, `security find-generic-password -s ai.gaply.app` (metadata only, no secret read) | this machine | §14's environment |

The probe is left untracked beside the other `zz_*` examples, per the brief.
