# PublishReady — audit of the record against the code at HEAD

**HEAD audited:** `9703e778cc134abc089db462a70a0ecb435c50a4` (main, 24 Sep 2026).
**Scope:** `docs/AI_ENGINE_PLAN.md` §11 D184–D224, `docs/PROBLEM_DOSSIER.md`,
`docs/EVIDENCE_LEDGER.md`.
**Mode:** measurement only. Nothing in the application was changed; this file is
the only output. Every defect below is reported, none is repaired.

## How to read this

**Provenance tags, on every figure and every verdict:**

| tag | meaning |
|---|---|
| `[src@HEAD]` | source inspection at 9703e77 |
| `[probe@HEAD]` | a live run at 9703e77 during this audit (test, example, or a probe built in a throwaway worktree at HEAD) |
| `[stored:X]` | read from a stored record or run (`plan Dxxx`, a commit message, a committed TSV) and NOT re-measured |

A stored figure and a live figure are never merged into one number. Where both
exist for the same quantity they are shown side by side and named as different
runs.

**"Guard fires"** means the test ran green at HEAD AND either a deletion test
went red (predicted in writing first, run with `--no-fail-fast`) or a specific
source argument shows its assertion is false in the broken world. Which of the
two is stated each time.

**"Reachable"** means a chain exists at HEAD from a screen (`src/` TS) through
`invoke('<cmd>')`, a command registered in `generate_handler!` (`lib.rs:129-265`,
95 commands), into `gaply_core`.

**Method.** Seven parallel read/probe passes (D184–D190, D191–D194, D195–D205,
D206–D213, D214–D224, the dossier plus reachability, the ledger plus seed rows
plus test coverage), each constrained to never write to the live tree; probes and
deletion tests ran in detached worktrees at HEAD that were removed afterwards.
Two cross-pass disagreements were resolved by re-reading source (noted where
they occur). Deletion tests actually run: 6 (listed in §7). Two further attempts
were refused by the permission classifier before anything was applied; those
guards are judged by source argument and say so.

---

## 0. The suite at HEAD is RED, and the first measurement of it said green

```
cargo test --workspace --no-fail-fast            [probe@HEAD, 16:50]
CARGO_EXIT=0  targets=26  passed=1942  failed=0  ignored=9      <- contaminated

same target, target/debug/ai-eval moved aside    [probe@HEAD, 17:20]
cargo test -p app --test ai_eval_cli --no-fail-fast
CARGO_EXIT=101  3 passed; 8 failed   (8 x "Os { code: 2, kind: NotFound }")
```

`tests/ai_eval_cli.rs` spawns `env!("CARGO_BIN_EXE_ai-eval")`; `Cargo.toml:52-55`
gates that bin behind `required-features = ["devtools"]` and the test is not
gated. Any `target/debug/ai-eval` left by an earlier `--features devtools` build
makes the target pass. One existed in the live tree during the first run, so
**the honest workspace result at HEAD is 1 failed target (`ai_eval_cli`,
8 failures)**. This is §11 D210 §1 exactly, still open, re-found by D219 and
D220, and it caught this audit too. The stale binary was restored afterwards to
leave the tree as found; it still masks the failure for any run on this machine.

The frontend suite: `npx vitest run --config vitest.config.ts` → exit 0,
**75 files / 970 tests** `[probe@HEAD]`. No CI workflow runs it (§6).

CLAUDE.md's "1302 tests, 9 ignored" is stale by ~640 `[probe@HEAD]`.

---

## 1. Per-record: §11 D184–D224

Columns: **claim** · **true at HEAD** · **later record contradicts it silently?** ·
**guard (exists / fires / how)** · **fix reachable by a user?**

### D184 — hardcoded clock in the harness
- **Claim:** `now` in the stage-2 harness is a real clock; the seed carries a real snapshot time.
- **At HEAD:** TRUE. `src-tauri/examples/journal_stage2_build.rs:132` `SystemTime::now()` `[src@HEAD]`. Qualification: the seed now holds TWO timestamps. Fingerprints are 1789728313 (18 Sep); 15 requirements + 4 bindings are 1790093868 (22 Sep, restamped by 59f792c). The picker's date is older than 19 rows it describes (conservative direction) `[src@HEAD, seed JSON]`.
- **Contradiction:** none.
- **Guard:** none (dev harness).
- **Reachable:** n/a (dev harness).

### D185 — rate-limited journal shown as healthy; `carries_guidance`
- **Claim:** coverage column + pass-2 revisit + RATE-LIMITED line; `carries_guidance` refuses `/content/by/` and document extensions.
- **At HEAD:** TRUE (`journal_stage2_build.rs:79,146-157,301-309`; `journal_crawl.rs:223-230`) `[src@HEAD]`. The named limit (a query-string `.pdf` slips through) is still open. The crawl figures (185 vs 213, one-hour crawl) are `[stored:plan D185]`.
- **Contradiction:** none.
- **Guard:** `journal_crawl::tests::an_article_listing_and_a_binary_attachment_carry_no_guidance` green `[probe@HEAD]`; fires by source argument (5 positive controls; removing the branch makes a `no` URL pass).
- **Reachable:** crawl path only (offline).

### D186 — the ten profiled journals ship bundled; a bundled row cannot pass for a fetched one
- **Claim:** seed via `include_str!`; `origin` CHECK in ('crawled','bundled'); provenance shown in the picker; "a later crawl REPLACES the row and the origin with it, so the distinction survives exactly as long as it is true".
- **At HEAD:** the first three are TRUE (`journal_store.rs:970,1042,385-388`; `migrations.rs:1083-1085`; picker `PublishReadyPage.tsx:563-569`) `[src@HEAD]`. **The last is FALSE.** The paste path (`guidelines.rs:655-660` → `store_requirements`, which APPENDS, `journal_store.rs:82`; then `:409-424` → `store_fingerprint_provenance(origin "crawled")`) keys only to the ten seeded journals. Probe: Nature Medicine BEFORE 37 requirements, origin=bundled → AFTER 38 requirements, **37 still from the bundled seed**, origin=crawled, fetched_at=now; the picker then says "fetched on this device, <today>" for all of them `[probe@HEAD seedpaste_1790249493.log]`.
- **Contradiction:** yes, silently. D192/D211 added the paste path and assert "no new provenance state is introduced" (`guidelines.rs:396-407`), without considering a seeded journal.
- **Guard:** `a_named_journal_turns_a_pasted_page_into_requirements_with_their_spans` asserts `origin == "crawled"` on an EMPTY db (`guidelines.rs:852`). It cannot see mixing. NOT GUARDING.
- **Reachable:** yes. PublishReady guidelines field → `ingest_guidelines` → picker.
- **Also:** the seed load is not transactional (`journal_store.rs:1036-1109`, fingerprint first, no tx). A mid-load failure permanently half-seeds a journal marked `bundled` `[src@HEAD, not probed]`. The picker's "N requirements" counts `fp.requirements` (37 for NM) while the seed test pins `requirements_for` (39). The pin is on a number no user sees.

### D187 — entitlement gate had no offline grace
- **Claim:** fifth state `unverifiable_no_account`; PublishReady proceeds with a notice, 4 other premium screens block; 19 `/app` routes, 5 gated; `run_full_analysis` passes `journal_key: None`.
- **At HEAD:** TRUE (`entitlement.ts:63,91-101,123-130`; `PublishReadyPage.tsx:469-492`; blocks at `CopilotPage.tsx:91`, `GapFinderPage.tsx:118`, `StatsVerifierPage.tsx:121`, `JournalVerifyPage.tsx:82`; `App.tsx:644-669`) `[src@HEAD]`. "Cannot leak paid work" is a deployment assumption (the loopback proxy URL is a runtime env var), not a code property; the proxy's JWT enforcement was not inspected.
- **Contradiction:** none; D187 is not cited again.
- **Guard:** `gating.vitest.tsx:138-178` green `[probe@HEAD]`; fires by source argument (collapsing the branch turns the second expect into `signed_out`).
- **Reachable:** yes.

### D188 — the code stops choosing a requirement row
- **Claim:** `also_from` carries every source; conditions are not built; "nothing reads `article_type`"; `unevaluable` is latent; "the code stops choosing".
- **At HEAD:**
  - `also_from`: TRUE (`report.rs:223-231,2167-2240`).
  - **"Nothing reads `article_type`" / "latent": FALSE.** Commit `ab217fe` (23 Sep) made article-type-scoped rows UNEVALUABLE (`report.rs:1853-1883`). On R PAPER, 4 of 10 journals now emit UNEVALUABLE rows (NM 2, BMC 1, Frontiers 1) `[probe@HEAD rows_*.log]`.
  - **"The code stops choosing": PARTLY FALSE.** The abstract-limit row still uses `.find()` in storage order (`report.rs:2123`). On NM it shows only the Brief-Communication-scoped span and drops the Article-scoped row `[probe@HEAD rows_nature-medicine.log]`. Its comment "not derived from journal_requirements" (`report.rs:2080,2128`) is false.
  - New since ab217fe: `scoped()` reads only the FIRST source's `article_type`, so storage order can now decide a VERDICT. No live instance found `[src@HEAD]`.
- **Contradiction:** yes, silently. ab217fe has no §11 record; stale "latent" comments remain at `ReportViewerPage.tsx:481-486`, `report_compose.rs:829-838` and `checklist_line.vitest.ts:62-64`.
- **Guard:** `report.vitest.tsx` "an UNDECIDABLE row is not drawn as a failure" green. Nothing guards the storage-order `.find()`.
- **Reachable:** yes, with a profiled journal.

### D189 — the product denied an abstract it quoted
- **Claim:** M1 (run-in) fixed; **M2 (PDF interior `Abstract-`) left open**; M3 declined, and "IV. Experimental Setup and Results … stays unrecognised"; M4 regex `(?:[.)]\s*|\s+)`.
- **At HEAD:** M1 TRUE (`sections.rs:172-183,240-245,286-289`). **M2 STILL OPEN:** R PAPER.pdf "MISSED run-in: 236 words on line 0 of 55"; the .docx known-good is FOUND `[probe@HEAD absprobe]`. **M3 second half FALSE:** the A2 compound-heading fix (`sections.rs:80-104`) now recognises it. M4 TRUE (`extract/stats.rs:248`). "49 → 51 lines" is `[stored:plan D189]`; HEAD gives 55 `[probe@HEAD]` (different runs, not comparable).
- **Contradiction:** silently, by the A2 fix (1b75e36), which has no §11 record.
- **Guard:** the M1/M4 tests are green; `numeral_letters_are_not_eaten_off_the_front_of_headings` is discriminating by source argument (`"Methods".is_some()` is false under the trap). No guard for M2 (open).
- **Reachable:** yes (extraction runs in every pipeline).

### D190 — checklist evidence on screen and in both exporters
- **Claim:** both exporters print status, span and `also_from`; the mirror pins both; "the checklist's evidence was on screen"; `unevaluable` is latent.
- **At HEAD:**
  - Both exporters: TRUE. Rust `report_compose.rs:592-617` via `export_publishready_pdf`; TS `exportPdf.ts:27-45` `[src@HEAD]`.
  - **"On screen": FALSE for single-source rows.** `ReportViewerPage.tsx:519` renders spans only when `sources.length > 1`. A one-source row shows requirement, detail and a bare "source" badge, with no span `[src@HEAD, re-read in reconciliation]`. So SIM's `[NOT MET] word limit: 250` appears on screen with nothing a reader could refute it by. `ChecklistDisplay.tsx`, which does render the span, is mounted by no screen.
  - Latent: FALSE since ab217fe (see D188).
- **Contradiction:** silently, by ab217fe.
- **Guard:** the `checklist_mirror::*` tests and `checklist_line.vitest.ts` are green. **Partial:** the fixture sets the primary `article_type: None`, and TS `itemFromMirror` hard-codes `article_type: null, guideline_source: null`, so neither the primary scope nor any URL can diverge detectably. Neither exporter prints either one `[src@HEAD]`.
- **Reachable:** PDFs yes; the span on screen no (single-source).

### D191 — the analysis cross-check fired on 29 of 31
- **Claim:** `tests_absent_from_the_record` exists and is unwired; the upload path is an instrument only; three defects.
- **At HEAD:**
  - Unwired: TRUE. The only production input is `pipeline.rs:812 analysis: None`.
  - Upload: reachable (`PublishReadyPage.tsx:232` → `analysis_ingest`); its result is not passed to the run.
  - Defect 1 (logistic regression → `LinearRegression`, `frequentist.rs:346`): STILL PRESENT and dormant.
  - Defect 2: probably present (no fit-index exclusion). Defect 3: not verified.
  - "29 of 31" is `[stored:plan D191]` and not reproducible: `statistics.jnl` is gone `[probe@HEAD find]`.
  - Comment `frequentist.rs:327-329` ("no upload path accepts an analysis file") is stale.
- **Contradiction:** none.
- **Guard:** `a_test_named_in_the_paper_and_absent_from_the_upload_is_found` exists. Nothing pins the check as UNWIRED: `Some(record)` at `pipeline.rs:812` would ship it with no red test.
- **Reachable:** the check, no; the upload, yes.

### D192 — the deterministic extractor that had no caller
- **Claim:** `extract_requirements` now runs on the pasted-URL path; "a journal the user names has no curated key, so one is minted from the name" (BMC Medicine → 2 stored); "unwiring the extractor reddens the guard"; nine public functions.
- **At HEAD:**
  - Production caller: TRUE, but only for the 10 profiled journals (`guidelines.rs:655`).
  - **Name-minting: FALSE.** Since D211, `key_for_url` is the only key source. Probe: BMC Medicine `journal_key=None requirements_stored=0`; known-good Nature Medicine → 3 stored `[probe@HEAD a2_probe_bmc / a2_probe_nm]`. `JournalIdentity::resolve` and `key_for_named_journal` have no production caller. Stale comments: `guidelines.rs:349-353`, `commands.rs:486-490`, `PublishReadyPage.tsx:264-270`.
  - **"Unwiring reddens the guard": FALSE** (deletion test below).
  - Nine vs ten public fns: `load_bundled_seed` (`journal_store.rs:996`) sits after the file's first `#[cfg(test)]` (:483), where the guard stops reading.
  - `store_conventions` / `store_standard_bindings` / `store_expectations`: still example-only.
- **Contradiction:** YES, silently. D211 withdraws D192's headline benefit; nothing after plan line 14830 cites D192; `PROBLEM_DOSSIER.md:494` still quotes it as current.
- **Guard:** `journal_producers_have_callers.rs` is green but **does not guard its named defect**. **DT-A2-2:** the production `extract_requirements` call was replaced with an empty Vec; red predicted, **GREEN** got. `src/bin/grrb.rs:200` (a devtools benchmark bin added 20 Sep, a day after the guard) is counted as production. **DT-A2-1:** removing `lib.rs:108` (the only `load_bundled_seed` caller); green predicted as a blind spot, GREEN got. *(One pass had called this guard "sound" on source reading; the two deletion tests override that.)* The app-crate unit test would still go red, but CI does not run the app crate.
- **Reachable:** 10 profiled journals yes; any other journal no.

### D193 — 6 of 20 became 13
- **Claim:** three fixes; 6 → 13 of 20; 46 requirements.
- **At HEAD:**
  - Fix 1 (quarantine note, `guidelines.rs:455-459,514-516`): TRUE and reachable.
  - Fix 2 (`RELATIVE_PRONOUNS`, `sanitize.rs:167,184`): TRUE and reachable.
  - **Fix 3 (`&amp;` decode, `journal_crawl.rs:566-585`): true in code, UNREACHABLE.** `links()` is called only by `crawl()` and `discover_guidelines_links()`, neither with a production caller.
  - "13 of 20" is `[stored:plan D193]`. Its instrument (`scopus_discovery_probe.rs`) keys by name, which stores 0 for non-profiled journals since D211 (inference from source plus the BMC probe; the sample was not re-run).
- **Contradiction:** silently, by D211. The dossier (`:494`) still quotes it.
- **Guard:** `every_note_a_user_can_be_shown_is_clean_prose` exists and drives 7 notes, but never renders the `picked_note` sentence ("You selected a journal (X), but…", `guidelines.rs:481-485`) that every user with an unprofiled journal sees. It is app-crate, so not run in CI. The live probe rendered it once and it is clean `[probe@HEAD]`.
- **Also:** `evals/grrb/journal.jsonl:7` and plan:14919 cite D194 for the BJS/PRISMA row, which is D193's.

### D194 — the em-dash guard covers the clean half
- **Claim:** the guard scans 5 files with 0 violations; 29 reader-facing violations are outside it.
- **At HEAD:** TRUE and unchanged. The file list is the same (`audit_report.rs:1876-1884`). A lexer scan finds a **byte-identical** set of production rows at 231b2eb and at HEAD (64 = 64). Classified reader-facing: **32** `[probe@HEAD]` vs D194's 29 `[stored:plan D194]`. The difference is classification (commands.rs 8 vs 7, install errors 6 vs 5), not new code. Rows are in the working notes (not committed). The frontend has 561 lines with an em dash across 193 files; not classified; no guard.
- **Contradiction:** none.
- **Guard:** true of nothing that fails, as the record says.
- **Reachable:** the violations are all user-visible strings.

### D195 — the benchmark exists
- **Claim:** 6-family baseline; "no pre-commit hook in this repo"; consistency.rs "has no abstract-versus-results check of any kind".
- **At HEAD:**
  - Family table: reproduces **exactly** `[probe@HEAD grrb run]`. 214 cases now. `grrb-gate` vs baseline: HOLD, exit 5, all unchanged.
  - **"No pre-commit hook": stale.** A hook exists as an untracked per-clone `.git/hooks/pre-commit`; a fresh clone gets no gate. `grrb-precommit.sh:4` says HOLD blocks a commit; its own lines 64-66 let it pass.
  - **"No abstract-vs-results check of any kind": overstated.** `check_metric_agreement` (D97, `consistency.rs:537`) predates it. "No N-mismatch check" is true.
- **Contradiction:** none recorded.
- **Guard:** `grrb_cases_are_wellformed.rs` green.
- **Reachable:** dev-only.

### D196–D198 — model scoring and label corrections
- **At HEAD:**
  - Precision tables are `[stored:plan D196–D198]`.
  - D198's two corrected labels (sci-084, -092) are correct in BOTH case files, and `second_adjudication.jsonl` has 19 rows, with exactly those 2 disagreeing `[src@HEAD]`. Its `adjudicator` field is "rishi" on all 19; the first adjudicator is named nowhere, so "two authors" is on record for one.
  - **D196's reopening command `cargo run --bin grrb` exits 101** (needs `--features devtools`) `[probe@HEAD]`.
  - `grrb.rs:108-110` names the wrong former field. The column still prints `w.prec 100%` for tp=1, fn=25 `[probe@HEAD]`.
  - Proxy validator limits (D197) TRUE (`validation.py:14-16`; `config.py:27-28`).
- **Reachable:** dev-only.

### D199–D201 — SLM-1 / SLM-2
- **At HEAD:**
  - SLM-1 is the 7B and is not bundled; the app has no LoRA path; `~/gaply-models/slm1` is absent: all TRUE `[src@HEAD; probe@HEAD ls]`.
  - "Cannot currently be run by the product": TRUE.
  - But `models/mod.rs:165,176` still calls any GGUF in `~/gaply-models/slm1` "the REAL SLM-1" (the misnaming D199 corrected in docs persists in code and logs). The user-visible name is honest.
  - SLM-2's `CLASSIFY_INSTRUCTION` presupposes its verdict: TRUE (`ai_detect.rs:1759,1773-1783`).
  - Model figures are `[stored:plan D199–D201]`.
- **Guard:** `model_claims_name_their_weights.rs` and `run_artefacts_name_their_model.rs` are green. The first is satisfiable by the word "unmeasured" (answerability, not truth, as stated in D200).

### D202 — does not exist
- No heading. `git log --all -S'D202'` finds only the dossier line that notes the gap `[probe@HEAD]`. It never existed in any commit, and nothing cites it. `decision_records.rs` has no contiguity check.

### D203 — the AI-Check three-way lane is UNEVALUABLE
- **Claim:** every shipped call passes `None`; no `human` category.
- **At HEAD:** TRUE. The only production call is `aicheck.rs:377 classify_passages(None, …)`; `aicheck_classifier()` has zero callers `[src@HEAD]`. **But the screen still says things that are false:**
  - Every heuristic-only passage reads "Not classified: only deep-verified passages are sent to the classification model" (`ai_detect.rs:1688`, shown at `AiCheckReport.tsx:52`). No passage is sent to any model.
  - `PARAPHRASE_CAUTION` (`ai_detect.rs:1660`) warns about a category "from the small local model" that never runs.
- **Contradiction:** none.
- **Guard:** `classifier_lane_is_declined.rs` is green. By source argument it fires for `aicheck.rs` ONLY; a call added in any other file passes. The deletion test was refused by the classifier and not run. The AI-Check render snapshot pins fixture text for the caution that has already drifted from Rust's string.
- **Reachable:** yes (`checkBridge.ts:73` → `run_aicheck` → `aicheck.rs:377`).

### D204 — a frontier model on the fixed set
- **At HEAD:** 33.3% (A) / 38.5% (B), recall 100%, 12 excluded, **re-derived from the committed raw TSVs** against HEAD labels, with no model call: match `[probe@HEAD, from stored TSV]`. **Temperature: still unset** (`gaply-proxy/app/openai_client.py:50-58`) `[src@HEAD]`.

### D205 — HOLD / WITHDRAWN banner
- **At HEAD:**
  - **"A corrected re-run … has not been done": FALSE.** D207 ran it, and the banner has no forward pointer.
  - **"score_ctx.py reproduces D204's 33.3%/38.5% as a self-test": FALSE.** There is no self-test; with no args it crashes (IndexError); it hard-codes `/Users/rishi/dev` paths `[probe@HEAD]`.
  - Run-1 figures (42.9 / 49.0; 34.7 / 39.1 on 144 common rows) re-derived from the stored TSV: match.
  - HOLD is real in code: the scientific layer is derived only if the graph requires it; specialists get `science: None`.
  - Pointer drift: `reviewer_agent.rs:596` → 532.
- **Guard:** `proxy_forwards_only_summary_and_instruction.rs` is green; its deletion test was refused and not run.

### D206 — reviewer letter baseline
- **Claim:** `publication_probability` is shown to nobody.
- **At HEAD:** TRUE for production. No component reads `publicationProbability`; `HTMLReportGenerator.tsx:567` prints one but has no importer; `synthesize.ts:75` writes it into the body only via `makePublishReadyMock` (test-only) `[src@HEAD]`. Not enforced at runtime: `REVIEWER_INSTRUCTION` still asks the model for the number, and `gate_reviewer_response` copies `body` verbatim. The 5-run prose check is `[stored:plan D206]`. The default provider is now `claude` (`config.py:17`) and D206 measured gpt-4o.
- **Guard:** the Rust test covers only the None case; no test renders the panel with a non-null probability.
- **Reachable:** n/a (a non-display claim).

### D207 — document context reaches the model and changes nothing
- **At HEAD:** per-run precisions recomputed from the stored TSV reproduce the spreads `[probe@HEAD, from stored TSV]`; the headline common-row join was not redone. **"The proxy discards the upstream detail … no diagnosis": FALSE.** `be77cd8` fixed it 19 minutes later (`gaply-proxy/app/main.py:77`), with no note in D207 and no plan record. Artefact paths lack the `src-tauri/` prefix.

### D208 — significance-criterion lexicon
- **At HEAD:** 11 `THRESHOLD_MARKERS`, unchanged (`extract/stats.rs:100-114`); none matches "statistical significance (p < 0.05)", so the R PAPER misfire stands (source argument). **"A 64-sentence labelled-by-hand corpus already available (19 + 46)": wrong twice.** 19 + 46 = 65, and the record itself later says only 6 were read. Counts are `[stored:plan D208]` (no committed probe).
- **Guard:** `a_located_finding_quotes_the_sentence_holding_its_statistic` green.

### D209 — D4 DMG failure characterised
- **At HEAD:** consistent with dossier D4. The quoted artefact (sha `b1f025dc…`) has been overwritten (now 492,984,457 bytes, 24 Sep 12:52) and cannot be checked. **D210 (plan:17190, :17204) and dossier:190 cite D209 as the fix for A3 rows 6 and 13. D209 is the DMG record; the fix is `ab217fe`, which has no §11 record.** `decision_records.rs` cannot see a wrong-target citation.

### D210 — three things recorded and NOT fixed
- **At HEAD:**
  1. `ai_eval_cli`: STILL BROKEN (§0) `[probe@HEAD]`.
  2. Both mis-scoped seed rows are still live and `status: "verified"`: PLOS ONE PRISMA from `journals.plos.org/plosone/feed/atom`; SIM `word_limit 250` from a licence FAQ `[src@HEAD]`. **"The checklist consumes them" is FALSE for PRISMA** (`design_independent`, `report.rs:2519`, filters `reporting_standard`; plos-one's checklist has no PRISMA row) `[probe@HEAD a7_probe]`, and TRUE for the word limit.
  3. The A3 doc correction is in place, but its attribution is wrong (D209, above).

### D211 — a fetched page's journal is decided by its URL
- **At HEAD:**
  - `key_for_url` is the only key source: TRUE (`journal_crawl.rs:353`, `guidelines.rs:379`) and reachable.
  - **"Host equality alone … is not the rule": FALSE for 2 of 10.** `journal_scope` returns None for hosts without a path-segment config and `key_for_url` then keys on host equality (`journal_crawl.rs:364`). `onlinelibrary.wiley.com` (statistics-in-medicine) and `journals.sagepub.com` (j-health-psychology) have no entry in `config/journal-crawl.json`, so a guideline URL for ANY Wiley or SAGE journal is stored under those two `[src@HEAD; NOT run, disk]`.
  - The 8 mis-bound rows are still present (§5 / list D).
- **Guard:** the named tests are green; `a_url_resolves_to_the_journal_whose_scope_it_falls_in` uses only nature/plos/bmj and cannot see a multi-journal host missing from the config.

### D212 — a Word table is read as a grid
- **At HEAD:** TRUE (`docparse.rs:988,1018,1116`; `pipeline.rs:454`). The named tests are green. Live `doc_tables` counts match (R PAPER 5, Health Econ 5, chapter3 8, others 0) `[probe@HEAD table_chain_probe]`.

### D213 — a table sighting is not a table
- **At HEAD:**
  - **Every figure in its six-manuscript table reproduces live** `[probe@HEAD probe_run_1790249851.log]`, with R PAPER (5) as the known-good case.
  - The PDF thesis shows **"This manuscript has 15 sections, 33 tables and 212 references"**. 33 is distinct `Table N` labels opening a paragraph (the PDF fallback, `extract/mod.rs:377-386`); 106 is the thesis's List of Tables `[stored:plan D213; not re-counted]`. It is printed with no qualifier; the caveat exists only in a Rust doc comment.
  - `extraction_examined` (`pipeline.rs:800`) is still open (§ list D).
- **Guard:** the 14 named guards exist; those run are green. `extraction_examined` is deliberately unguarded.

### D214 — an absence is "not detected"
- **Claim:** absence rules labelled "not detected by an automated check"; "one source, two renderers".
- **At HEAD:** label TRUE (`vocabulary.rs:123-125`). **"Two renderers": FALSE.** Four surfaces print `certainty_label`:
  - R1: viewer row, `ReportViewerPage.tsx:413-415`
  - R2: Inspector, `:219-221`
  - R3: TS summary PDF, `exportPdf.ts:61`, present since e87f0c7 and at D214's own commit
  - R4: Rust composer PDF, `report_compose.rs:461-464`

  At HEAD all four agree for absence and pattern rules `[src@HEAD; rule strings confirmed by named tests]`. D214's list of what the tier still drives (sort, colour, hard_constraint, routing) omits that the tier goes **verbatim to both cloud models** (`reviewer_agent.rs:610`, `chatContext.ts:51`).
- **Contradiction:** D219 "corrects" the count to three, which is still short.
- **Guard:** **D214's two named guards no longer exist** (0 hits). `vocabulary.vitest.ts:36-37` still cites the vitest one (a stale pointer; TS is not scanned by `cited_tests_exist.rs`). The successor tests are green.
- **Reachable:** yes (Stats Check and PublishReady).

### D215 / D216 — overclaim and absence rules traced
- Corpus figures (5 firings, 0 entailed) are `[stored:plan D215/D216]`. Source claims are TRUE: rule 2 reads only the location of `Stat::PValue { .. }` (`validate.rs:200-202,241-256`) and has no negation handling. The live probe is in list D.

### D217 — no `validate.rs` rule labelled "mathematically certain"
- Partly superseded by D219, as recorded. "Both renderers" is FALSE (four, above). `swarm.rs:376-378` still says "mathematically certain" (D219 records it as unrendered `[stored]`).

### D218 — RESERVED, not started
- **At HEAD:** the Stats Check clean pass is titled "All deterministic statistical rules passed", labelled "mathematically certain", with detail "4 rules evaluated; none fired." (`adapters.ts:177-187`), verdict pass and `combined_confidence: 1.0` (`:190-192`). The TS PDF prints "Overall verdict: PASS · combined confidence 100%" and "Validation / Maths — mathematically certain (confidence 100%)".
- **What it claims vs what is measured:**
  - It claims certain cleanliness. D216 measured false positives only; recall is unmeasured.
  - `small_sample_causal_claim` is declined, yet the title says "All".
  - **Vacuous pass:** body "No statistics here at all." → `passed=true checks=4 flags=0` `[probe@HEAD a5_overclaim2]`.
  - The page's own disclaimer (`adapters.ts:35-37`) contradicts the row.
- **Code already implying D218's result:** the row itself (the pre-D217 state, knowingly kept); `review_lens.rs:2089-2107` awarding a strength when all checks ran and none fired (hedged); the swarm opinion text.
- **Guard: NONE.** **DT-A5-3:** the title and label were both renamed; green predicted, full vitest **970/970 GREEN**. D218's "nothing is changed until it is done" is unenforced.

### D219 — pattern labels; D217's overreach reverted
- **At HEAD:** the revert is complete for strings. Every D217 string has 0 hits except one comment (`validate.rs:241`). The labels are TRUE (`vocabulary.rs:126-133`).
- "Reaches Stats Check, clean run" is incomplete: it also reaches the TS PDF (R3).
- The renderer count "three" is short (four).

### D220 — the PDF prints each finding's own label
- **Claim:** "Since D217/D219 no finding in the Rust report is labelled 'mathematically certain'; the validation lane is that tier's only producer" (repeated in `report.rs:395-399`).
- **At HEAD: FALSE.** Equation findings (`equation_report.rs:129,160`, emitted at `report.rs:909`) carry tier AND label "mathematically certain" through the real pipeline. `pipeline::tests::an_equation_finding_reaches_the_report_with_its_tier_and_trail` asserts exactly that and is **green** `[probe@HEAD a5_eqtest]`, while its own doc calls the finding "a question, not a verdict". D214 and D217 themselves listed equation_report as a producer that keeps the label.
- **Consequence:** the disclaimer ("not that the manuscript is wrong") sits beside a "mathematically certain" finding on R1–R4.
- **"A cached report keeps the disclaimer":** TRUE and understated. `get_report` (`commands.rs:246-259`) never re-derives `certainty_label`; the cache key has been unchanged since 15 Sep with a 30-day TTL. Reports cached before c045c24 reopen with the old labels `[src@HEAD; whether any exist: not checked]`.
- **Guard:** the named tests are green; the deletion tests are `[stored:plan D220]`.
- **Reachable:** yes.

### D221 — the remaining "require correction" copies
- **At HEAD:** no user-reachable copy remains (grep hits are comments and tests only). The same claim, reworded, survives at `synthesize.ts:76-77` ("…mathematically certain findings must be corrected regardless of…"), mock-only and unreachable. So it is four copies, not three.

### D222 / D223 / D224 — report viewer
- **At HEAD:**
  - D223: TRUE. There is no `manuscriptSections` default, and all 4 production callers omit it. **DT-A5-2:** the default was restored; red predicted, **1 red** got.
  - D224: TRUE. **DT-A5-1:** `setSelectedId` was dropped; 3 red predicted, **exactly 3 red** got.
  - Both guards are user-shaped at the component level (render + click every tab), but drive `ReportViewerPage` directly, not `PublishReadyPage`, `LiveReportPage` or `CheckScreen`.
  - A latent twin of D222: `report = SAMPLE_REPORT` is still the default (`ReportViewerPage.tsx:104`). It is not reachable today, since every caller passes `report`.

---

## 2. PROBLEM_DOSSIER.md at HEAD

The dossier was written at 281aedf and edited twice since. **Five fixes landed
after it and it was not updated. Four of the five have no §11 record**
(`grep -c <sha> AI_ENGINE_PLAN.md` = 0 for 692dd62, 1b75e36, f73e49d, 1812199;
likewise ab217fe) `[src@HEAD]`.

| item | as written | at HEAD |
|---|---|---|
| A1 paragraph index | fixed, "unpushed" | Fixed (`report_model.rs:72`). "Unpushed" is stale. The frontend `+1` (`adapters.ts:172`) has **no test**; the Rust pair cannot see TS |
| A2 "Results missing" | open, impact 5 | **Fixed by 1b75e36** (`classify_heading_parts`). Tests green. No D-record |
| A3 human-subjects rows | rows 6, 13 "fixed by §11 D209" | Fixed by **ab217fe**, not D209. Rows 8 and 10 open |
| A4 effect size on a criterion | crux unknown | Display half fixed (c56afd2, `quoted_evidence_in`). Cause measured in D208 (lexicon); widening refused |
| A5 spans cut at 400 | open | **Fixed by 692dd62** (forward-only; stored rows stay truncated until re-crawl; seed restored by 59f792c) |
| A6 bibliography drives AI concern | open, impact 4 | **Partly fixed by f73e49d**. R PAPER is still LeansAiLike from table bodies `[stored:commit msg]` |
| A7 fixed constants | open | True (line drift `report.rs:1143` → 1171) |
| A8 25/25 unchecked | open | String at `report.rs:825`; which service failed is not verified |
| A9 misc | — | #1 stale (A2 fixed), #2 partly stale |
| B1 reviewer unavailable | — | Gate arms moved (`commands.rs:1023,1037,1044`); line drift only |
| B2 four lenses, no app caller | — | **TRUE** (list C) |
| B3 letter sees findings only | — | TRUE (`reviewer_agent.rs:54,56,586`) |
| B4 temperature, gauge, mock | — | TRUE |
| C1, C4, C5 | model/novelty measurements | record only |
| C2 9 agents | — | 9 nodes confirmed. **Not in the dossier:** specialists run on every analysis and their output is dropped (list C) |
| C3 no D202 | — | TRUE |
| C6 "premise inverted" | CSV/xlsx parse, SPSS deferred | **Wrong on the user's path.** The PublishReady analysis upload goes through `analysis_ingest`, which parses only `.sps`/`.jnl` (`analysis_ingest.rs:121`); CSV/R/py/ipynb are stored, not parsed. The dossier audited `supplementary.rs`, which that upload never calls. `supplementary.rs` also parses .sav/.mat since 1812199, and no UI reaches it |
| D1 proxy limits | — | TRUE (`validation.py:14-16`) |
| D2 command error wiring untested | — | TRUE |
| D3 CI runs `-p gaply_core` | — | TRUE |
| D4 DMG | → D209 | Pointer correct. Not rebuilt (disk) |
| D5 "signingIdentity present but signed with `-`" | — | **Mischaracterised.** `tauri.conf.json:72` IS `"-"`, deliberately since c44157a. The DMG is "not signed at all" `[probe@HEAD codesign]` |
| D6 not reproducible | — | Not verified |
| Q A3.2 | open question | **Answered** in `A3_APPLICABILITY_MEASUREMENT.md` (52 of 213 conditional) |

## 3. EVIDENCE_LEDGER.md at HEAD

- **None of the 19 numbered citations appears anywhere in the repo except the ledger** (`docs/`, `src/`, `src-tauri/`, CLAUDE.md, README) `[src@HEAD]`. Every "REMOVE AS JUSTIFICATION / RE-JUSTIFY / MERGE" disposition is satisfied **vacuously**: there is nothing left to stop citing. The "solutions document" and "original architecture document" the ledger audits are not in the repo, and it does not name them.
- **One live record conflict:** the ledger records arXiv `2604.20801` as a vulnerability-discovery paper in an "unrelated domain" `[stored:ledger]`. `publishready-premium-architecture.md:3,38,422,1034,1072,1745` cites the same id as "AgentFlow" and builds on its "central result" `[src@HEAD]`. One of them is wrong about this id. Not resolvable without a network fetch, which this audit did not do.

---

## 4. The four lists

### A. Claims that are no longer true

| # | record | claim | at HEAD | provenance |
|---|---|---|---|---|
| A1 | D220, `report.rs:395-399` | no Rust finding is "mathematically certain"; validation is the only producer | equation findings are, via the real pipeline; a green test asserts it | probe@HEAD |
| A2 | D214, D217, D219 | "one source, two renderers" / "both renderers agree"; D219's "three" | four label renderers (R1–R4). They agree today for rule findings; the enumeration was always short | src@HEAD |
| A3 | D214 | tier retained only for sort, colour, hard_constraint, routing | also sent verbatim to the reviewer and Copilot models | src@HEAD |
| A4 | D214 | named guards `an_absence_rule_says_not_detected…`, vitest "shows an absence rule as not detected…" | neither exists; `vocabulary.vitest.ts:36-37` still cites one | src@HEAD |
| A5 | D186 | a bundled row cannot pass for a fetched one | a paste relabels 37 bundled rows "fetched on this device, today" | probe@HEAD |
| A6 | D188, D190 | `unevaluable` latent; nothing reads `article_type` | live since ab217fe (4 of 10 journals on R PAPER); stale comments in 3 files | probe@HEAD |
| A7 | D188 | "the code stops choosing" | abstract-limit `.find()` picks by storage order; its comment is false | probe@HEAD |
| A8 | D189 | "IV. Experimental Setup and Results" stays unrecognised | recognised since 1b75e36 | src@HEAD |
| A9 | D190 | the checklist's evidence is on screen | not for single-source rows | src@HEAD |
| A10 | D192 | a named journal gets a minted key; BMC Medicine → 2 stored | key None, 0 stored | probe@HEAD |
| A11 | D192 | unwiring the extractor reddens the guard | stays green (grrb bin) | deletion test |
| A12 | D192 | nine public functions | ten; the guard sees nine | src@HEAD |
| A13 | D193, dossier:494 | 6 of 20 became 13 (as current) | its own instrument would store 0 for non-profiled journals since D211 | stored + inference |
| A14 | D195 | no pre-commit hook; HOLD blocks a commit | per-clone untracked hook; its body lets HOLD pass | src@HEAD |
| A15 | D195 | consistency.rs has no abstract-vs-results check of any kind | `check_metric_agreement` (D97) predates it | src@HEAD |
| A16 | D196 | reopen with `cargo run --bin grrb` | exit 101, needs `--features devtools` | probe@HEAD |
| A17 | D205 | corrected re-run not done | D207 did it; no forward pointer | src@HEAD |
| A18 | D205 | score_ctx.py self-tests against 33.3 / 38.5 | no self-test; crashes with no args; hard-coded paths | probe@HEAD |
| A19 | D207 | the proxy discards upstream detail | fixed by be77cd8, no record | src@HEAD |
| A20 | D208 | 64-sentence labelled corpus available | 19 + 46 = 65; not labelled (6 read) | src (arithmetic) |
| A21 | D210, dossier A3 | rows 6/13 fixed by §11 D209 | D209 is the DMG record; the fix is ab217fe (no record) | src@HEAD |
| A22 | D210 | the checklist consumes the PRISMA row | filtered by `design_independent` | probe@HEAD |
| A23 | D211 | host equality alone is not the rule | it is, for the Wiley and SAGE hosts | src@HEAD |
| A24 | D221 | three remaining copies | a fourth reworded copy in `synthesize.ts:76-77` (mock-only) | src@HEAD |
| A25 | dossier A2, A5, A6, A4, C6, D5, Q A3.2, A1 | as in §2 | as in §2 | src@HEAD |
| A26 | ledger vs architecture doc | arXiv 2604.20801 | two incompatible identities | stored + src |
| A27 | `frequentist.rs:327-329`; `guidelines.rs:349-353`; `commands.rs:486-490`; `PublishReadyPage.tsx:264-270`; `grrb.rs:108-110`; `models/mod.rs:165` | code comments | describe behaviour that has since changed | src@HEAD |
| A28 | CLAUDE.md | 1302 tests | 1942 Rust + 970 vitest | probe@HEAD |

### B. Guards that no longer guard

| # | guard | defect | how determined |
|---|---|---|---|
| B1 | `gaply-core/tests/journal_producers_have_callers.rs` | counts devtools `src/bin/grrb.rs` as production, so unwiring the real `extract_requirements` call stays green; also stops reading `journal_store.rs` at its first `#[cfg(test)]`, so `load_bundled_seed` is invisible; MODULES exclude `journal_expect` and `journal_standards` | deletion tests DT-A2-1 and DT-A2-2 (both green) |
| B2 | **none** on the Stats Check clean-pass title and label (D218) | renaming both leaves 970/970 green | DT-A5-3 |
| B3 | **none** on the TS summary PDF certainty line | reintroducing the exact D214/D220 defect in `exportPdf.ts:61` leaves 970/970 green; R3 tests assert only that a Blob is produced | DT-A5-4 |
| B4 | `guidelines::tests::a_named_journal_turns_a_pasted_page…` | runs on an empty db, so it cannot see bundled/fetched mixing | fixture read |
| B5 | checklist-line mirror (D190) | fixture fixes primary `article_type` None and TS hard-codes `guideline_source: null`; scope and URL drift undetectable | fixture read |
| B6 | `report.vitest.tsx` "box appears for several sources and not for one" | asserts absence with no positive control that the span shows elsewhere | source read |
| B7 | `seed_tests::a_fresh_database_gets_…` | pins 39 (`requirements_for`); the picker shows 37 | source + probe |
| B8 | `journal_keys_match_the_seed.rs` | checks key-set equality only; passes with 8 + 3 + 80 mis-bound seed rows | assertion read |
| B9 | `a_url_resolves_to_the_journal_whose_scope_it_falls_in` | fixtures chosen where host equality is correct; blind to Wiley/SAGE | test + config read |
| B10 | `classifier_lane_is_declined.rs` | scans `aicheck.rs` only | source (deletion test refused) |
| B11 | `every_note_a_user_can_be_shown_is_clean_prose` | never renders the `picked_note` sentence users see; app-crate, not in CI | fixture read |
| B12 | `decision_records.rs` | checks a D-number exists, not that it is the right one (the D209 mis-citation passes); app-crate, never run in CI; no contiguity check (D202) | source read |
| B13 | `supplementary::tests::unsupported_format_is_a_clear_error` | asserts .sav unsupported; passes only because the new parser's error text contains "unsupported" | source read |
| B14 | `guidelines::tests::a_crawled_journals_curated_key_beats…` | pins `JournalIdentity::resolve`, which has no production caller since D211 | caller grep |
| B15 | `vocabulary.vitest.ts:38` | pins the artifact's values only, never opens `adapters.ts`, and names a nonexistent guard | source read |
| B16 | rule-2 unit tests `validate.rs:396-407` | one case at p < 0.001; neither the p-value's value nor negation is pinned either way | source + probe |
| B17 | `journalFingerprint.vitest.tsx` | pins three components no screen mounts | mount grep |
| B18 | `model_claims_name_their_weights.rs` | satisfiable by "unmeasured" (by design, D200) | source read |
| B19 | D206 "no probability rendered" | Rust test covers only None; no frontend test with a non-null value | source read |
| B20 | `em-dash` guard (`audit_report.rs`) | covers 5 files holding 0 violations; the 32 are outside it | lexer scan |
| B21 | the GRRB gate | an untracked per-clone hook, built by no CI job; guards one machine | src@HEAD |
| B22 | **all 970 vitest tests** and **all app-crate tests** | run in no CI workflow; every frontend guard and every app-crate guard is local-only | workflow read |
| B23 | none pins D191 unwired (`pipeline.rs:812`), `extraction_examined`, or the reachability of review_lens / editor / exports / chat_scope / novelty / specialists | — | source read |

### C. Fixed, or built, but unreachable

Chain checked UI → invoke → registered command → core `[src@HEAD]`.

| item | defined | production callers | verdict |
|---|---|---|---|
| four reviewer lenses `review_lens::lenses/review` | `review_lens.rs:1521,1908` | only `red_team::run_pipeline`, which has none | core-only |
| editor rubric `editor::decide` | `editor.rs:166` | only via red_team | core-only |
| rubric/letter renderers `exports::annotate/letter/audit_trail` | `exports.rs:132,230,270` | none | core-only |
| `journal_store::store_conventions / store_standard_bindings / store_expectations` | `journal_store.rs:192,259,305` | `examples/journal_stage2_build.rs` only; users get the tables only from the bundled seed; `build_checklist` passes `bindings: &[]` (`report.rs:1825`) | core-only |
| upstream `journal_expect::extract_expectations`, `journal_standards::bindings_from` | `journal_expect.rs:109`, `journal_standards.rs:120` | none | core-only, unguarded |
| journal fingerprint UI (`JournalFingerprintView`, `ChecklistDisplay`, `JournalPicker`) + `journal_fingerprint` command | `src/screens/publishready/journal/` | no screen mounts them; `.fingerprint()` has no caller | built + tested, unmounted |
| specialists `frequentist_stats`, `claim_evidence_strength` | `pipeline.rs:811-818` | executed on every run; `PipelineResult.specialists` read only by a test (`pipeline.rs:1423`) | **executed, output dropped** |
| D191 cross-check | `frequentist.rs:255` | `analysis: None` hard-coded | unwired |
| D193 fix 3 (`&amp;` decode) and `discover_guidelines_links` | `journal_crawl.rs:467,566-585` | examples only | unreachable |
| D192 extraction for non-profiled journals | `guidelines.rs:655` | gated on `key_for_url` (10 hosts) | unreachable for all others |
| `.sav` / `.mat` parsing | `supplementary.rs:137-138,397` | TS never passes `supplementaryPaths`; the Stats Verifier picker excludes both | command-only |
| `run_premium_gate` | `release_gate.rs:223` | tests only | dead (no tier check exists anywhere) |
| novelty `extract_claims/assess/retrieve` | `novelty.rs:300,631,786` | none; the "Novelty" card renders the LLM's field | core-only (declined, D166) |
| `chat_scope::context_from/answer` | `chat_scope.rs:198,297` | Copilot does not use it | core-only |
| `memory::record_episode/…` | `memory.rs:24,40,57` | none | dead |
| registered commands with no TS caller and no "reserved" note | `ai_generate_test`, `ai_link_citations` | — | command-only |
| other zero-production `pub fn`s | 46 of 388 in `gaply_core` (e.g. `rag::ingest_corpus`, `ai_detect::analyze_tiered`, `plagiarism_exact::analyze_exact`) | none | dead; not individually reviewed |

Reachable, for contrast: `plagiarism_exact` (`/app/check/plagiarism` → `check_plagiarism_exact`); `checklist_from_requirements` **only when a profiled journal is picked**; the D203 decline; the D220–D224 fixes.

### D. Open defects, ranked by user impact

"Blocks" = stops a researcher getting a correct result from the app for their case.

| # | defect | evidence | blocks? |
|---|---|---|---|
| D1 | **The Lancet's journal checklist is one Elsevier-wide row**, sourced from "Competing interests for editors who are employees of Elsevier"; the picker advertises "2 requirements", both mis-bound | `a7_seed_checklist` probe: 5 items, 1 journal row, src elsevier.com `[probe@HEAD]` | **yes, for Lancet** |
| D2 | **Statistics in Medicine shows `[NOT MET] word limit: 250`**, from a Wiley re-use-licence FAQ; 3 of its 4 journal rows are Wiley-wide; single-source, so the screen shows no span to refute it | R PAPER probe `[NOT MET]` `[probe@HEAD rows_statistics-in-medicine.log]`; seed row `status: verified` | **yes, for SIM** |
| D3 | **Any Wiley or SAGE guideline URL is stored under SIM or J Health Psychology** (host-only fallback) | `journal_crawl.rs:364`; config lacks both hosts `[src@HEAD; NOT run]` | yes, for those users (unverified live) |
| D4 | **A pasted guidelines URL for any journal outside the 10 stores nothing**; checklist is structural only (the note is honest) | BMC Medicine 0 stored; NM control 3 `[probe@HEAD]` | yes, for journal-specific checks |
| D5 | **Stats Check clean pass: "mathematically certain", confidence 100%, on a pass that can be vacuous** (no statistics → pass); exported the same way | `a5_overclaim2` `[probe@HEAD]`; `adapters.ts:177-192`; `exportPdf.ts:50-61` | no, false assurance (D218's subject) |
| D6 | **The overclaim rule ignores the p-value's value and flags a negation**: identical Major for p = 0.001 / 0.2 / 0.9; "does not prove" flagged | `a5_overclaim_1790249284.log` `[probe@HEAD]`; `validate.rs:200-202,241-256` | no, false Major findings |
| D7 | **Equation findings "mathematically certain"** beside a disclaimer saying deterministic findings are "not that the manuscript is wrong" | green test `pipeline.rs:957` `[probe@HEAD]` | no, contradictory claim |
| D8 | **A PDF of a paper with an interior `Abstract-` loses its abstract** (D189 M2) | R PAPER.pdf MISSED `[probe@HEAD]` | yes, for such PDFs |
| D9 | **Paste on a seeded journal relabels the bundled snapshot "fetched on this device, <today>"** | seedpaste probe `[probe@HEAD]` | no, false freshness |
| D10 | **Thesis PDF: "33 tables"** against 106 in its List of Tables, with no qualifier | table_chain_probe `[probe@HEAD]`; 106 `[stored:plan D213]` | no, visible false count |
| D11 | **`extraction_examined` true on caption sightings alone**; "Table and reference checks" omitted from "What was not examined" though the table lane outputs nothing | chapter3: 0 refs, flag true `[probe@HEAD]`; `report.rs:1426-1430` | no, a check reported as run |
| D12 | **Single-source checklist rows show no journal sentence on screen** (PDFs do) | `ReportViewerPage.tsx:519` `[src@HEAD]` | partly: verdicts uncheckable on screen |
| D13 | **Mis-scoped rows now visible:** BMC "competing interests" UNEVALUABLE "for Letter articles" from a *cover letter* span; NM abstract limit shown only for Brief Communications | rows_bmc / rows_nature-medicine `[probe@HEAD]` | mild |
| D14 | **AI-Check copy is false:** "only deep-verified passages are sent to the classification model" (none are); caution about a model that never runs | `ai_detect.rs:1660,1688`; `AiCheckReport.tsx:52,302-304` `[src@HEAD]` | no, misleading |
| D15 | **PublishReady analysis upload parses only SPSS syntax**; CSV/R/py/ipynb stored, not parsed | `analysis_ingest.rs:121-140` `[src@HEAD]` | yes, for non-SPSS users of that feature |
| D16 | **Reviewer-lens / editor / export / chat layer never reaches a user**; specialists computed and discarded | list C | yes, for the expert-review goal |
| D17 | **Cached reports (≤30 d) keep pre-D214 "mathematically certain" labels** at `/app/report` | `commands.rs:246-259` `[src@HEAD]`; existence of such rows not checked | no |
| D18 | **TS PDF prints "confidence 100%" per finding** and "combined confidence 100%", which the viewer removed as uncalibrated | `exportPdf.ts:51-52,61` | no |
| D19 | **Tier `mathematically_certain` sent verbatim to reviewer and Copilot models**; Copilot's prompt asks for a disclaimer only on AI-assessed items | `reviewer_agent.rs:610`, `chatContext.ts:51`, `chatGuards.ts:17` | unknown (model output not measured) |
| D20 | **Proxy sets no temperature** (every cloud reply sampled at default) | `openai_client.py:50-58` `[src@HEAD]` | no |
| D21 | **Reviewer body prose unfiltered while the instruction still asks for `publication_probability`** | `reviewer_agent.rs:82,707` | no, latent |
| D22 | **32 reader-facing em dashes in app-crate strings**, plus unclassified frontend strings | lexer scan `[probe@HEAD]` | no |
| D23 | **`ai_eval_cli` red at HEAD**; masked by any stale `ai-eval` binary | §0 `[probe@HEAD]` | no (dev) |
| D24 | **No CI runs vitest or any app-crate test**; ai-eval/grrb unbuildable from a clean checkout without the gitignored models, and built by no CI job; GRRB gate per-clone | workflow read; clean-worktree build exit 101 `[probe@HEAD]` | no (dev) |
| D25 | **Latent:** 83 further mis-bound seed rows (3 bindings, 80 expectations, including ~25 Elsevier `/connect/` marketing posts) surface the day the fingerprint view mounts; logistic → linear regression the day D191 is wired; `scoped()` first-source verdicts; non-transactional seed load; `report = SAMPLE_REPORT` default | `[probe@HEAD seed audit; src@HEAD]` | no, latent |
| D26 | **Record hygiene:** D202 gap with no tombstone; five fixes with no §11 record (692dd62, 1b75e36, f73e49d, 1812199, ab217fe) plus be77cd8 | `git log`, grep `[probe@HEAD]` | no |

**Named items from the brief, located:**
- 8 mis-bound seed rows → D1, D2
- `extraction_examined` → D11
- PDF tables 33 of 106 → D10
- `ai_eval_cli` on a clean build → §0, D23, D24. In a clean worktree without models, `cargo build --features devtools --bin ai-eval` exits 101 at tauri-build (`bundled-models/tokenizer.json doesn't exist`); with symlinked models it builds (exit 0) `[probe@HEAD]`.
- D218's clean-pass claim → D5, B2
- the overclaim rule ignoring the p-value → D6, B16

---

## 5. The 8 mis-bound seed rows, in full

`[probe@HEAD]`: a replica of `key_for_url` over `gaply-core/data/journal-seed.json`.
It reproduces D211's count and hosts exactly. It is a copy of the rule, not the
Rust function.

| # | journal | kind | value | host | heading | reaches the screen? |
|---|---|---|---|---|---|---|
| 1 | lancet | reporting_standard | CONSORT | www.elsevier.com | Clinical trial transparency | no (filtered) |
| 2 | lancet | section_required | competing interests statement | www.elsevier.com | Competing interests for editors who are employees of Elsevier | **yes** |
| 3 | SIM | data_policy | data availability statement | authorservices.wiley.com | Data sharing | **yes** (as `also_from`) |
| 4 | SIM | data_policy | data availability statement | authorservices.wiley.com | Encourages Data Sharing | **yes** |
| 5 | SIM | reporting_standard | ARRIVE | authorservices.wiley.com | Animals in research | no (filtered) |
| 6 | SIM | reporting_standard | CONSORT | authorservices.wiley.com | Registering clinical trials | no (filtered) |
| 7 | SIM | section_required | competing interests statement | authorservices.wiley.com | How does free format work? | **yes** |
| 8 | SIM | word_limit | 250 | authorservices.wiley.com (licensing FAQ) | Licenses for Subscription Articles | **yes, as NOT MET on R PAPER** |

5 of 8 reach users. The Lancet has only these 2 requirement rows; SIM has 7, of
which 6 are these. There is no guard over binding correctness (B8).

---

## 6. User-visible surfaces never driven the way a user drives them

**Infrastructure facts** `[src@HEAD]`:
- There is no e2e, Playwright or webdriver test.
- Every frontend test injects a mocked bridge. No test crosses the real `invoke` into Rust.
- The only IPC tests (`src-tauri/tests/commands_test.rs`) register 9 commands, and **none of the 9 is invoked from TS**. So **0 of ~82 TS-invoked commands are exercised through IPC.**
- The 970-test vitest suite and the CRA jest tests run in **no CI workflow**. `App.test.tsx` still asserts the CRA template's "learn react".

Classes: **(i)** component rendered and interacted with (mocked bridge), or an export produced from a real run and read back; **(ii)** only helpers or inputs tested, or rendered from a hand-built fixture; **(iii)** no test.

| surface | class | evidence |
|---|---|---|
| **`/app/report` → LiveReportPage** (where every PublishReady result, the checklist and the certainty labels are read) | **ii / iii** | LiveReportPage is rendered by no test. ReportViewerPage is tested only on hand-written `SAMPLE_REPORT`. No test renders a real `compile_report` or stored report through the route |
| LandingPage, AuthSuccessPage, ComingSoonPage, ThesisAuditPage wrapper | **iii** | none |
| Stats Check clean-pass row | **iii** | no test renders a passing Stats Check run (B2) |
| TS summary PDF content (R3), every screen | **ii** | tests assert a Blob / download only (B3) |
| PublishReady full PDF (`export_publishready_pdf`) | ii+ | PDF content produced from a real `run_pipeline_inner` and read back (`pipeline.rs:1843-1888`); the command and its button are untested |
| Thesis-audit PDF/HTML (`ai_job_export_report`) | ii | built from constructed rows; no job run → export → read back |
| AI-Check HTML export (`export_report`) | ii | HTML-string tests; write command untested; snapshot fixture drifted from Rust |
| AI-Check paraphrase callout | ii | fixture strings, not `classify_passages(None)` output |
| Notes .tex | ii | pure functions; CI compiles goldens from `emit.ts`, not from the UI |
| Picker provenance after a paste on a seeded journal | ii | vitest mocks the profile row; Rust test uses an empty db |
| Scoped UNEVALUABLE row → either exporter | ii | unit tests whose comments still say "latent" |
| Single-source checklist row on screen | ii | nothing asserts the span is visible |
| `picked_note` ("You selected a journal (X), but…") | iii in tests | rendered only by this audit's live probe |
| Guidelines ingest with a Wiley/SAGE URL | iii | — |
| ReviewerLetterPanel with a non-null `publicationProbability` | iii | — |
| `run_publishready_measured` error wiring (dossier D2) | iii | no test caller |
| `adapters.ts:172` paragraph `+1` | iii | no vitest |
| `supplementaryPaths` argument | iii | never set by any TS caller |
| D223 / D224 viewer fixes | i at component level | render + click every tab; deletion-tested; but drive ReportViewerPage directly, not the screens that mount it |
| PublishReadyPage, StatsCheckPage, AiCheckPage, PlagiarismCheckPage, Copilot, GapFinder, StatsVerifier, JournalVerify, Citations, Notes (.docx), Settings, etc. | i (mock) | screen-level vitests exist; all behind a mocked bridge |
| JournalFingerprintView / ChecklistDisplay / JournalPicker | tested, **unmounted** | tests cover components no user sees |

---

## 7. Deletion tests run in this audit

All were run in a detached worktree at HEAD, predicted in writing first, and run with `--no-fail-fast`.

| id | break | predicted | got |
|---|---|---|---|
| DT-A2-1 | remove the only `load_bundled_seed` call (`lib.rs:108`) | green (blind spot) | **green** |
| DT-A2-2 | production `extract_requirements` call → empty Vec | red | **green** (B1) |
| DT-A5-1 | `openTab` drops `setSelectedId(firstIndexFor(t))` | 3 D224 tests red | **exactly 3 red** |
| DT-A5-2 | restore the `SAMPLE_MANUSCRIPT_SECTIONS` default | 1 D223 test red | **exactly 1 red** |
| DT-A5-3 | clean-pass title + label renamed | green (no guard) | **green, 970/970** |
| DT-A5-4 | `exportPdf.ts:61` prints "mathematically certain" for the tier | green (no guard) | **green, 970/970** |
| — | `aicheck.rs:377` → `Some(..)`; add a forwarded key in `openai_client.py` | — | **refused by the permission classifier; not run** |
| (control) | `ai_eval_cli` with the stale binary moved aside | red | **red, 8 of 11** (§0) |

## 8. What was not verified, and why

- **Model accuracy figures (D196–D207):** stored. Only D204 A/B, D205 run 1 and D207 per-run spreads were re-derived, and those from the committed TSVs, not new model calls.
- **Corpus and crawl figures** (D185, D188, D189, D208, D215, D216, D193's 20-journal sample): stored; they need crawls, corpora or files that no longer exist (`statistics.jnl`).
- **The Wiley/SAGE mis-binding (D3):** source argument only. The app-crate probe was skipped because free disk was 4.9 GiB.
- **Whether pre-D214 cached reports exist in the user's `gaply.db` (D17), and the live DB's seed rows:** only the bundled seed was measured.
- **Whether the reviewer or Copilot models ever repeat "mathematically certain" (D19):** needs the proxy.
- **106 tables in the thesis:** not re-counted. Whether its labels 99–105 are body captions or contents rows is not established.
- **The DMG (D209, dossier D4/D6):** not rebuilt (disk). The recorded sha's artefact has been overwritten.
- **Citation truth in the ledger:** no network fetch.
- **App-crate tests named by records but outside the runs above** (`the_shipped_body_carries_no_generation_cap`, the pipeline `scientific.is_none()` pin): existence confirmed, not individually run. The full workspace run at 16:50 included them green, subject to §0's caveat that the run was contaminated in one target.
- **This audit's own instrument caveat:** probes shared the live `src-tauri/target` directory to save disk. A devtools build during the audit rewrote `target/debug/ai-eval` (16:55). That is the D210 hazard, and it is why §0's first green was re-measured rather than quoted.
