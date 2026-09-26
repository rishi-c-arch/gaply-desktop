# PublishReady — what a user can reach at HEAD

**HEAD:** `c9feb09e0da15edef803b8d9aa73ae8cb5cf0a45` (main, 27 Sep 2026).
**Baseline:** `docs/PUBLISHREADY_AUDIT.md`, written against `9703e77`. HEAD is
**19 commits after the audit commit** (`f931560`), 20 after `9703e77`
(`git rev-list --count`). The brief said eight; the measured count is above.
**Mode:** measurement only. No product change; this file is the only output.

**Provenance tags.** `[src]` = source inspection at HEAD. `[probe]` = a live
run at HEAD, named. `[stored]` = a figure read from a record, not re-measured.

**The chain.** A capability is REACHABLE when a screen under `/app/*` calls
`invoke('<cmd>')`, `<cmd>` is in `generate_handler!` (`src-tauri/src/lib.rs`,
**97 commands** `[src]`, 95 at the audit), and the command reaches `gaply_core`
or the app crate's pipeline. Census `[src]`: 81 of 97 registered commands have
a reference in non-test TypeScript; the 16 without are `ai_generate_test`,
`ai_link_citations`, `create_project`, `db_health`, `db_init`, `db_migrate`,
`delete_secret`, `extract_manuscript`, `generate_handler`, `get_project`,
`has_secret`, `health_check`, `list_projects`, `rag_search`, `secrets`,
`store_secret`. Four commands are defined and not registered
(`ai_embed_document`, `ai_index_document`, `ai_index_status`,
`ai_semantic_search`).

---

## 1. What changed since the audit

Code that moved (`git diff --stat 9703e77..HEAD`, excluding docs and tests):
`consistency.rs`, `audit_prepass.rs`, `journal_store.rs`, `guidelines.rs`,
`journal_crawl.rs`, `lib.rs`, `models/mod.rs`, `ai-eval.rs`, the seed JSON, and
one vitest file. No screen, no command body, no pipeline lane and no report
composer changed. So every reachability verdict in the audit's list C stands
unless named below.

| audit item | at `9703e77` | at HEAD | how |
|---|---|---|---|
| D1 Lancet checklist is one Elsevier-wide row; D2 SIM `[NOT MET] word limit: 250` from a licence FAQ; §5's 8 mis-bound rows | live, 5 of 8 reach users | **Gone from the seed.** `journal_ownership_probe --all` over the bundled seed: requirements **205 rows, 0 foreign** (was 213 / 8); bindings **47, 0 foreign** (50 / 3); expectations **176, 0 foreign** (258 / 80). A startup reconcile (`lib.rs:121`, `remove_rows_journals_do_not_own`) deletes the same rows from an existing database. §11 D225 | `[probe]` |
| — consequence | Lancet had 2 requirement rows | **Lancet has 0 requirement rows, 0 bindings, 0 expectations; Nature Communications 0 / 0 / 0; BMC Public Health 2 host-only rows.** Picking either of the first two gives the four structural rows and nothing journal-specific | `[probe]` |
| D3 Wiley/SAGE URL stored under SIM / J Health Psychology (host-only fallback) | source argument | **Fixed.** §11 D226 reproduced it live (Journal of Advanced Nursing → `statistics-in-medicine`, 4 rows) and removed the host-equality fallback for multi-journal hosts | `[stored: D226]`; the fix is in `journal_crawl.rs` `[src]` |
| B22 / D24 no CI runs vitest or the app crate | true | **False now.** `frontend-build.yml` runs vitest; `app-tests.yml` runs `cargo test -p app --features devtools` on ubuntu (488 tests, run `36251563731`). §11 D227–D229. The GRRB gate is still a per-clone hook | `[src]`, CI run |
| D23 `ai_eval_cli` red locally | red | Unchanged locally; green in CI because CI builds with `--features devtools` (CLAUDE.md, D210 still open) | `[src]` |
| thesis-audit consistency on IJAS | 48 false structural findings (D231, measured after the audit) | **0 false structural, 0 false cosmetic** after D231 + D232 | `[stored: D231, D232]` |
| D17 cached reports keep pre-D214 labels | `get_report` never re-derives | Confirmed and sharpened: `get_report` calls `enrich_report_labels`, which "only inserts keys" (`vocabulary.rs:413`), so a label already stored is kept | `[src]` |
| expectation rows removed | 80 foreign | 258 → 176 is **82** removed; 2 more than D225's foreign count. Not investigated | `[probe]` vs `[stored: D225]` |

Everything else in the audit's lists A–D is unchanged by inspection of the diff.

---

## 2. Capability by capability

Legend: **REACHABLE** — a user triggers it and sees its output. **PARTIAL** — it
runs and the output is dropped or not rendered. **BUILT, UNREACHABLE** — code
exists, no path reaches it; the last column names the one missing piece.
**NOT BUILT.**

The PublishReady run is `PublishReadyPage` → `TauriPublishReadyBridge.run` →
`invoke('run_publishready')` → `run_publishready_measured` → `run_pipeline_measured`
(`src-tauri/src/pipeline.rs`) → `compile_report` → `ReportViewerPage` with tabs
`Overview, Statistics, Citations, AI Risk, Plagiarism, Checklist, Reviewer Letter`
(`PublishReadyPage.tsx:39`, `PR_TABS`) `[src]`.

| capability | verdict | chain / evidence | what is missing |
|---|---|---|---|
| **Checklist** | **REACHABLE** | `build_checklist(&db, …, journal_key)` (`pipeline.rs:714`) → `report.checklist` → Checklist tab. Structural rows always; a journal's own rows only for a profiled pick with rows: **7 of 10** at HEAD (above). Live, no journal: 4 rows, all PASS on R PAPER `[probe what_a_user_sees]`. A single-source row still shows no span on screen (`ReportViewerPage.tsx:519`, `sources.length > 1`) `[src]` | for the span: a render. For non-profiled journals: a crawl (D4, unchanged) |
| **Statistics** | **REACHABLE** | `validate::validate` (`pipeline.rs:491`) → Statistics tab. Live on R PAPER: 2 Major (`missing effect size`, `missing confidence interval`), labelled "not detected by an automated check" `[probe]`. Also `/app/check/stats` → `validate_manuscript` | — (D5/D6 overclaim defects unchanged) |
| **Citations** | **REACHABLE, degrades to "not checked"** | `run_publishready_measured` hard-codes `NetworkConsent::Granted` (`commands.rs:851`); the screen gates on `mayUseCloud('publishready')` (`PublishReadyPage.tsx:249`). Existence via `RefVerifier` (CrossRef etc.). Verdicts via `models::verify_proxy`: **cloud proxy only if the App Check key is in the keychain and `/health` answers; else local Ollama SLM-2; else a `MockProxyClient` returning no verdicts → every citation UNKNOWN** (`models/mod.rs:1088-1103`, `pipeline.rs:588-596`) `[src]`. Live probe ran consent-denied: `verification_examined: false` `[probe]` | a configured proxy or Ollama (deployment, not code) |
| **AI risk** | **REACHABLE** | `ai_detect::detect_extraction` at the memory-gated tier (`pipeline.rs:512`) → AI Risk tab. Live: 2 minor + "AiDetection: concern" `[probe]`. Copy defects D14 unchanged (`ai_detect.rs` not in the diff) | — |
| **Plagiarism** | **REACHABLE** | `PlagiarismSession` against the user's library (`pipeline.rs:519-521`) → Plagiarism tab; also `/app/check/plagiarism`. With an empty library `plagiarism_examined: false` `[probe]` | — |
| **Reviewer letter (wholesale)** | **REACHABLE with a proxy; honest "unavailable" without** | `build_review_payload` → `ProxyReqwestClient::from_env()` reachable → `/verify` → `gate_reviewer_response` (`commands.rs:1009-1042`); `reviewer_letter_availability` pre-flights. Live, no proxy: `available: false`, body "deep reasoning requires cloud analysis — unavailable offline" `[probe]` | a deployed proxy + App Check key (deployment) |
| **Reviewer letter (Box 4 shadow synthesis)** | **PARTIAL** | Runs every time (`run_shadow_synthesis`, `commands.rs:954`), compared against the wholesale letter, written to `box4_comparisons.jsonl` (`harness_log.rs:81`), returned as `shadow_reviewer` in the IPC value — and `adaptOutcome` never reads it `[src]` | a decision (Stage 2 "switch") then a render |
| **Four reviewer lenses** (`study_design`, `reproducibility_data_availability`, `ethical_compliance`, `overstated_conclusions`, `review_lens.rs:1210-1269`) | **BUILT, UNREACHABLE** | only caller is `red_team::run_pipeline`, which no production code calls `[src grep]` | a caller in `run_pipeline_inner` |
| **Editor rubric** `editor::decide` (`editor.rs:166`) | **BUILT, UNREACHABLE** | takes `&[ReviewerReport]`, which only the lenses produce; same sole caller `[src]` | a caller (after the lenses) |
| **Exports — expert layer** `exports::annotate / letter / audit_trail` (`exports.rs:132,230,270`) | **BUILT, UNREACHABLE** | zero callers `[src grep]` | a caller and a render |
| **Exports — report PDFs** | **REACHABLE** | TS summary PDF `downloadReportPdf` (`PublishReadyPage.tsx:407`); Rust full PDF `export_publishready_pdf` by `runId` (`:424`); thesis-audit `ai_job_export_report` `[src]` | — (D18 "confidence 100%" in the TS PDF unchanged) |
| **Chat (Copilot)** | **REACHABLE with a proxy** | `CopilotDock` on the result screen and `/app/copilot` → `run_copilot_chat` → `chat_agent` (pure; the proxy is the one hop) `[src]` | a deployed proxy |
| **Chat scope** (Explain / Evidence modes, `chat_scope.rs`) | **BUILT, UNREACHABLE** | `context_from` / `answer` have no production caller; only `declined.rs` tests reference the module `[src grep]` | a command |
| **Chat: corrections / challenge** | **NOT BUILT, declared** | `DECLINED_LANES` (`declined.rs:66,74`) rendered under "What Gaply does not assess" `[probe]` | a decision |
| **Novelty** | **module BUILT, UNREACHABLE by decision; the card REACHABLE** | `novelty.rs` header: "DECLINED — §11 D166 … the INSTRUMENT that measured the decline". The card renders the cloud reviewer's grounded `novelty_assessment` or a `DeclineNote` (`ReviewerLetterPanel.tsx:165-175`) `[src]`; live: the decline text `[probe]` | a decision (recorded as declined) |
| **Specialists** (`frequentist_stats`, `claim_evidence_strength`) | **PARTIAL** | run on every PublishReady run (`pipeline.rs:811-816`, `analysis: None`); `PipelineResult.specialists` is read by one test (`:1423`) and by nothing in `compile_report` or the outcome `[src]`. **Live: `frequentist_stats` FIRED a Major on BOTH probe manuscripts** — R PAPER `parametric_test_assumptions_unstated` (Methods ¶125), IJAS the same code (Methods ¶5); `claim_evidence_strength` DECLINED on both with a reason `[probe wired_specialist_states]`. Neither reaches a screen | a decision (they are kept additive to protect the golden capture), then a compile path and a render |
| **Analysis upload** | **REACHABLE as an upload; PARTIAL as a check** | `ingestAnalysis` → `analysis_ingest` (`PublishReadyPage.tsx:232`); the summary and per-file rows render (`:596-603`). Parses `.sps`/`.jnl` only; `.py/.R/.ipynb/.csv` stored, not parsed (`analysis_ingest.rs:121`, unchanged). **The record never reaches the run:** `analysis: None` (`pipeline.rs:812`); the D191 cross-check that would use it is dormant with its logistic→linear defect `[src]` | a caller passing the record, after the D191 defects are fixed |
| **Consistency checks** | **REACHABLE — in the thesis audit only** | `check_consistency` is called only in `thesis_audit.rs:209,770` (preview + job) `[src]`, rendered at `ThesisAuditScreen.tsx:817` via `/app/check/citations`. **Not called by the PublishReady pipeline.** D231/D232 changed its output on IJAS (48 → 0 false structural; the Miranda cosmetic gone) | for PublishReady: a call in `run_pipeline_inner` and a tab |
| **Journal fingerprint UI** (`JournalFingerprintView`, `ChecklistDisplay`, `JournalPicker`) | **BUILT, UNREACHABLE** | no screen mounts them; `journal_fingerprint` has a bridge method (`journalFingerprintBridge.ts:37`) with no caller; `journal_profiles` IS used for the picker `[src]` | a mount |
| **Guidelines paste** | **REACHABLE for the 10 profiled journals** | `ingest_guidelines` → `extract_requirements` gated on `key_for_url`; any other journal stores 0 (D4) `[src]`; Wiley/SAGE now key correctly (D226) | a crawl config per journal |
| **Debate / swarm** | **REACHABLE** | `report.debate` composed in `compile_report`; rendered in the Overview `[src]` | — |
| **RAG lane** | **PARTIAL by design** | runs; summary "no guideline matches (corpus not seeded yet)" (`pipeline.rs` lane 5) `[src]` | a seeded corpus |
| **Memory** `memory::record_episode` | **BUILT, UNREACHABLE** | zero callers `[src]` | a caller (or deletion) |
| **`run_premium_gate`** | **BUILT, UNREACHABLE** | tests only (`release_gate.rs:223`) `[src]` | a decision (no tier check exists) |

**What a user with no proxy and no Ollama gets from a PublishReady run on R PAPER**
`[probe what_a_user_sees, consent denied, in-memory db]`: verdict `concern`,
`combined_confidence 1.0`, 8 findings (2 Major from statistics, 3 minor, 3 info),
4 structural checklist rows all PASS, reviewer letter unavailable, five declined
lanes listed, lanes `verification_examined: false, plagiarism_examined: false`.

---

## 3. What a researcher notices first, with estimates

Ordered by how soon a user meets it. "Confident" means the change is confined
to files I read and the shape is known; "guess" means the size depends on a
measurement or decision not yet made.

| # | what they meet | estimate | confidence |
|---|---|---|---|
| 1 | Reviewer letter "unavailable offline"; Copilot refuses; citations all UNKNOWN — on any machine without the proxy | deployment, not code: a proxy URL, the App Check key in the keychain, the JWT gate live | confident that it is not a code change; the deployment effort is unknown to me |
| 2 | A Major statistics finding the pipeline computed on their paper (`parametric_test_assumptions_unstated`, fired on 2 of 2 probe manuscripts) is not on any tab | 1 day to compile specialist reports into `report.findings` + re-pin the golden capture; **plus the decision** whether they join the free-tier report | guess: the day is confident, the decision is not mine |
| 3 | Picking The Lancet or Nature Communications gives the same 4 structural rows as picking nothing; 7 of 10 profiled journals have journal rows | a crawl per journal that fetches the journal's OWN pages; D4 for every non-profiled journal | guess: 1–3 days per publisher host, unmeasured |
| 4 | A checklist row says `[NOT MET]` with no quote to check it against (single-source rows) | 0.5 day: render `sources[0].source_span` when `sources.length === 1` + a vitest | confident |
| 5 | Stats Check clean pass reads "mathematically certain, 100%" even with no statistics (D5); the overclaim rule ignores the p-value (D6) | 0.5 day for the label; 1–2 days for the rule (needs the D208 lexicon measurement first) | label confident; rule a guess |
| 6 | Structural problems the thesis audit finds (orphan citations, unreadable entries, metric contradictions) never appear in the PublishReady report | 1–2 days: call `check_consistency` in `run_pipeline_inner`, map findings, add a tab, re-pin the capture | moderate: the check exists and is tested; the mapping to `Finding` is new |
| 7 | An uploaded SPSS file is counted and then ignored; an R/Python/CSV file is stored and unparsed | 1 day to pass the record (`analysis: Some`); **but** D191's defects fire first (29 of 31 false on the one pairing), so 3–5 days including fixing them and re-measuring | guess |
| 8 | Nothing shows the AI-check copy is false ("only deep-verified passages are sent to the classification model"; none are) | 0.5 day: two strings + the drifted snapshot | confident |
| 9 | The four reviewer lenses, the editor rubric and the annotated-manuscript / letter / audit-trail exports exist and no user sees them | 1 day to call them from the pipeline; 2–3 days to render; **quality unmeasured** on real manuscripts, and D166/D167 show what that measurement tends to find | guess |
| 10 | Chat cannot say "explain this finding" from stored fields (Explain/Evidence are 137/137 answerable per `chat_scope`'s own audit) | 1–2 days: a command over `chat_scope::answer` + a dock mode | guess |
| 11 | The journal fingerprint view (built, tested) is not mounted | 0.5–1 day to mount behind the picker; the 82 expectation rows and 47 bindings it would show are now all own-journal | moderate |
| 12 | The shadow reviewer letter is computed and discarded | a decision (Stage 2), then 0.5 day | not an estimate I can make |

---

## 4. Limitations

* **Probes ran consent-denied, with an in-memory database and no journal key.**
  That is the path `what_a_user_sees` documents as the common configuration; it
  means the verification lane, the plagiarism library and the seeded journal
  rows were not exercised live in this pass. The seed itself was measured by
  `journal_ownership_probe`, which reads the JSON, not a database.
* **No frontend was driven.** Every "renders at line N" is source inspection.
  The audit's §6 (no e2e, every frontend test behind a mocked bridge) is
  unchanged.
* **No proxy, no Ollama, no model call.** Reviewer, Copilot and cloud citation
  verdicts are classified from source.
* **Two manuscripts** for the specialist probe (R PAPER, IJAS). "Fired on 2 of
  2" is not a rate.
* The 82-vs-80 expectation-row difference is reported, not explained.
* Whether `shadow_reviewer` survives serialisation into the IPC value was read
  from the struct, not observed on the wire.

## 5. Probes run

| probe | input | output |
|---|---|---|
| `examples/journal_ownership_probe --all` (release) | bundled seed at HEAD | 205 / 47 / 176 rows; 0 foreign in each table; per-journal table in §1 |
| `examples/what_a_user_sees` (release, `GAPLY_DISABLE_DEEP=1`) | `R PAPER .docx` (`effbb86c2495…`) | §2's summary paragraph |
| `examples/wired_specialist_states` (release) | `R PAPER .docx`, IJAS PDF (`859880647c45…`) | FIRED 2, DECLINED 2, RAN-EMPTY 0 |

All three examples were already committed; nothing was added to the tree for
this report.
