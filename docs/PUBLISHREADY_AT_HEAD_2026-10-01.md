# PublishReady at HEAD — what a user gets, measured

**HEAD:** `76b112a` (main, 1 Oct 2026). The last code commit is `3db49ac`, D258;
`76b112a` changes only CLAUDE.md.
**Baseline:** `docs/PUBLISHREADY_REACHABILITY.md`, written at `c9feb09`. HEAD is
**37 commits** past it (`git rev-list --count c9feb09..HEAD`).
**Mode:** measurement only. No product change. This file is the only committed
output.

**This file reports only what CHANGED since that audit, and what it did not
cover.** Everything the audit states and this file does not repeat still
stands. The evidence: **no file under `src/` (the frontend) changed** since
`c9feb09` (`git log c9feb09..HEAD -- src/` is empty), so every render verdict
in it carries over. And none of `review_lens.rs`, `editor.rs`, `exports.rs`,
`chat_scope.rs`, `novelty.rs` or `commands.rs` changed. `red_team.rs` gained one
`&[]` argument in a test.

**Provenance tags.** `[src]` means source inspection at HEAD. `[probe]` means a
live run at HEAD, named in §6. `[stored]` means a figure read from a decision
record and not re-measured.

---

## 0. What changed in code since the audit `[src]`

`git diff --stat c9feb09..HEAD`, excluding docs. The 21 files are
`journal-seed.json` (+8475), `journal_extract.rs`, `journal_store.rs`,
`journal_standards.rs`, `guidelines.rs`, `journal-crawl.json`, `lib.rs`
(startup reconciles), `report.rs` and its tests, `pipeline.rs`, the equation
engine (`check.rs`, `graph.rs`, `linear.rs`, `equation_report.rs`),
`epistemic.rs`, `extract/mod.rs`, `vocabulary.rs`,
`specialist/frequentist.rs`, the golden fixture, and one probe.

What a user can notice from those changes:

| change | record | at `c9feb09` | at HEAD |
|---|---|---|---|
| seeded journals | D248–D258 | 30 profiled (the audit counted 10) | **106**: 666 requirements, 146 bindings `[probe]` |
| `frequentist_stats`'s `parametric_test_assumptions_unstated` | D233 | computed, never shown | **on the Statistics tab** as a Major with its sentence. Fired on IJAS and R PAPER `[probe]` |
| abstract limit with two stated values | D246 | picked the newest row | **undecided** (`–`) |
| article-type re-typing and misread-row removal | D239–D257 | — | run at startup; on a fresh seed they remove nothing `[probe]` |
| equation parser | D237 | — | partial lines refused, not dropped |

---

## 1. The real pipeline on the six manuscripts, each with a plausible journal `[probe]`

**Setup.** An in-memory database seeded and reconciled in the order `lib.rs`
startup uses: `load_bundled_seed`, then D225, D238, D240 and D239. The seed load
reports 106 journals, 666 requirements and 146 bindings, and all four
reconciles remove 0. Then `run_pipeline_measured`, which is the body of
`run_publishready`, runs with the journal key the picker passes. Release build,
`GAPLY_DISABLE_DEEP=1`, consent denied. Corpus hashes match D232's
(`859880647c45`, `effbb86c2495`, `de1e322a95f1`, `134956ff25ef`,
`af1f36ff2c81`, `77e91296924b`).

**Known-good row.** R PAPER gives `concern` with 9 findings, the D233/D235
recorded values. Two runs produced identical output.

**Two of the six are thesis chapters (chapter3, Lake), and final-L is a
381-page thesis.** No journal is a plausible target for them as they stand.
They were run against the journal their subject matter fits, and the results
should be read with that in mind.

| manuscript | journal picked | verdict | override flipped it? | findings (Major / minor / info) |
|---|---|---|---|---|
| R PAPER (BiLSTM, emotion detection) | Expert Systems with Applications | **concern** | **yes** | 9 (3 / 3 / 3) |
| IJAS (silkworm haemolymph) | PLOS ONE | pass | no | 6 (1 / 2 / 3) |
| Health Economics (Oman, mediation) | Value in Health | **concern** | **yes** | 10 (3 / 4 / 3) |
| chapter3 (lake limnology methods) | Environmental Pollution | pass | no | 5 (0 / 2 / 3) |
| Lake Chapter 1 (introduction only) | Science of the Total Environment | pass | no | 5 (0 / 2 / 3) |
| final-L (thesis) | Water Research | **concern** | **yes** | 9 (3 / 3 / 3) |

**Checklist** (structural rows + journal rows). "Undecided" includes the
article-type word-limit row, whose text says it is undecided and which renders
as ✓ (§3 item 1).

| manuscript | rows | structural P/F | journal rows | journal P / F / undecided |
|---|---|---|---|---|
| R PAPER | 6 | 4 / 0 | 2 | 1 / 1 / 0 |
| IJAS | 9 | 4 / 0 | 5 | 0 / 5 / 0 |
| Health Economics | 7 | 4 / 0 | 3 | 1 / 2 / 0 |
| chapter3 | 8 | 2 / 2 | 4 | 0 / 2 / **2** (one flagged `–`, one drawn ✓) |
| Lake Chapter 1 | 8 | 0 / 4 | 4 | 0 / 3 / 1 (flagged `–`) |
| final-L | 7 | 4 / 0 | 3 | 0 / 2 / **1** (drawn ✓) |

**Lanes.** All six ran with `verification_examined: false` (consent denied
here; in the app, with no proxy, every citation verdict is UNKNOWN, per the
audit) and `plagiarism_examined: false` (empty library). Lake Chapter 1
additionally has `validation_examined: false` and
`extraction_examined: false`. Its report still carries an "Extraction: pass"
info row. Specialists:

| manuscript | `frequentist_stats` | `claim_evidence_strength` |
|---|---|---|
| R PAPER | FIRED `parametric_test_assumptions_unstated` → **shown** | DECLINED (no recognised design) |
| IJAS | FIRED `parametric_test_assumptions_unstated` → **shown** | DECLINED (experimental design) |
| Health Economics | FIRED `multiple_comparisons_uncorrected` → **dropped** (D233) | ran, nothing |
| chapter3 | ran, nothing | DECLINED (experimental design) |
| Lake | ran, nothing | DECLINED (no recognised design) |
| final-L | ran, nothing | DECLINED (experimental design) |

The `DECLINED_LANES` list ("What Gaply does not assess") is unchanged.
`declined.rs` is not in the diff.

---

## 2. The audit's BUILT-BUT-UNREACHABLE items `[src]`

| item | reached since `c9feb09`? |
|---|---|
| Four reviewer lenses | **No.** `review_lens::` is called from no production path. The callers are `exports.rs`, `chat_scope.rs` and `red_team::run_pipeline`, which are themselves unreachable, plus tests. `review_lens.rs` is unchanged |
| Editor rubric `editor::decide` | **No.** Same sole caller (`red_team`). `editor.rs` is unchanged |
| Exports `annotate` / `letter` / `audit_trail` | **No.** Zero callers. `exports.rs` is unchanged |
| Chat scope (Explain / Evidence) | **No.** Referenced only from `lib.rs` (the module declaration), `red_team.rs`, `declined.rs` and itself |
| Journal fingerprint UI (`JournalFingerprintView`, `ChecklistDisplay`, `JournalPicker`) | **No.** No file under `src/` changed. `PublishReadyPage.tsx:27` imports the bridge for `journal_profiles`, and the `journal_fingerprint` method still has no caller |
| Novelty | **No.** `novelty.rs` is unchanged. Its only non-test references are in `review_lens.rs` (unreachable) |

Not on the list, also unchanged: `memory::record_episode` has zero production
callers, and `run_premium_gate` has none either.
**`check_consistency`** is still called only by the thesis audit
(`thesis_audit.rs:209,770`) and `bin/grrb.rs`, never by the PublishReady
pipeline.

**New since the audit, and the opposite of unreachable:** one specialist code
(D233) now reaches the Statistics tab. The bindings and reporting-standard rows
remain computed and never shown (next section).

---

## 3. What a user sees that is still wrong or empty, ranked by how likely a researcher is to notice

Ranked by exposure (how many users meet it) times how plainly wrong it reads.

1. **A word-limit row reads ✓ when the manuscript exceeds EVERY stated limit.**
   `report.rs:2221` builds "word limit depends on article type" with
   `passed: true` and `unevaluable: false`. The renderer draws `✓`
   (`ReportViewerPage.tsx:487`), and the PDF prints "met"
   (`exportPdf.ts:30`). Live: chapter3 against Environmental Pollution,
   *"10000 (Review Article); 3000 (Short Communication); 8000 (Full
   Research). Your manuscript has 10733 words"* → ✓. final-L against Water
   Research, 117,337 words against 3000 and 8000 → ✓. **It reaches 23 of 106
   journals** `[probe census]`, for every manuscript. This is the
   three-state-at-the-wire defect from CLAUDE.md, built after that rule was
   written. The fix the row's own text implies is `unevaluable: true`, plus
   a decided FAIL when the count exceeds every stated value. `[probe]` + `[src]`
2. **An abstract limit decided against the wrong text, both ways.**
   * IJAS against PLOS ONE: *"abstract has 588 words against a limit of 300"* →
     ✗. The real abstract is **~248 words** (hand count from `ABSTRACT` to
     `Keywords:`, `pdftotext`). The manuscript has no Introduction heading, so
     the Abstract section runs on to MATERIALS AND METHODS. **The correct
     verdict is PASS.** `[probe]` + hand-read
   * Fertility and Sterility ships *"abstract limit: 30 words"* from *"The
     capsule is a summary of the abstract of 30 words or less"*. It is a
     decided ✗ for **every** manuscript with an abstract. **Not in any
     record.** `[probe census]` + seed read
   * 13 journals carry *"abstract limit 500 (Clinical Trial)"* from *"Editors
     will not consider results to be a prior publication if … posted in the
     same clinical trials registry … brief structured abstract (fewer than 500
     words)"*. D257 counted this row twice in its 20-journal sample; **it is in
     13 journals**. Through D246 it turns 13 otherwise decided 250-word rows
     into "the journal states more than one". That is **13 of the 20**
     such undecided rows per manuscript. `[probe census]` + seed read
   * final-L: *"abstract has 9764 words"*. The count sums `ABSTRACT` (799)
     with the thesis's "3.10 Summary" sections, which are typed Abstract. The
     verdict direction holds and the number is wrong.
3. **Word limits that are not manuscript limits, decided.** Value in Health:
   *"word limit: 120 … manuscript has 6755 words"* → ✗. The source is the
   Highlights rule (*"3 highlight statements (a combined total of no more than
   120 words)"*), known to D257 and shipped at 6.9%. Fertility and Sterility
   lists *"3 (Letter)"* as a word limit, from *"limited to 3 authors, 400
   words"*. That row sits inside a depends-on-type row, so its effect is the
   ✓ in item 1. **Neither is visible as such on screen**, because of item 5.
   `[probe]`
4. **Conditional statements applied unconditionally.** "Informed consent
   statement" ✗ on a silkworm study (PLOS ONE's *human-subjects-research*
   page) and on two lake studies (*"For clinical studies, a statement of
   informed consent …"*). "Ethics approval statement" ✗ on Lake rests on
   *"If a study was granted exemption …"*. Informed consent is in **27 of 106**
   journals' checklists. 15 of those rows are the one clinical-studies
   sentence. A researcher outside clinical work reads it as a failure. `[probe]`
   + seed read
5. **A single-source checklist row shows no quote.** The render requires
   `sources.length > 1` (`ReportViewerPage.tsx:519`), unchanged since the
   audit. **15 of the 21 journal rows in §1 are single-source**, including
   items 2 and 3 above. The quote that would let a reader dismiss "120 words"
   as a Highlights rule in one glance exists in the payload and is not
   drawn. `[probe]` + `[src]`
6. **The D235 override forces `concern`.** It is still live and unchanged:
   R PAPER, Health Economics and final-L each read `concern` with
   `overridden_by_constraint: true` and combined confidence 1.0. The soft
   vote said `pass` in all three. Exactly D235's table. `[probe]`
7. **The inverse: `pass` above a Major.** IJAS reads `pass` with "No
   statistical assumption check detected: ANOVA" (Major) on the same page.
   That is the D233 row folded in after the debate (D234 case E, now live on
   a real manuscript). `[probe]`
8. **Dropped specialist finding.** Health Economics'
   `multiple_comparisons_uncorrected` runs and is withheld by decision (D233:
   "24 tests, 71%" where the plausible family is about 5, about 23%). It is
   computed on every run and shown nowhere. **That is the right call** while
   the count is wrong, and there is no user-visible defect. `[probe]`
9. **Preacher.** Health Economics' thesis audit still lists *"Preacher &
   Hayes (2004)"* and *"(2008)"* as "Listed but never cited". Both are false
   (D232 fix 2, not built). It is cosmetic and **thesis audit only**, not
   PublishReady. Live counts at HEAD match D232's after-state exactly: 14 / 2 /
   8 / 18 / 0 / 95. `[probe]`
10. **Lake Chapter 1:** `extraction_examined: false`, yet the report shows
    "Extraction: pass". Minor and visible. `[probe]`

**Latent, not visible:** `report.rs:2596` and `:2605` hold reader-facing
strings with 22-space runs inside the literal. This is CLAUDE.md's
heredoc-backslash shape, from `b1d795a1`. Those rows come from
`unbound_standard_findings`, and `build_checklist` passes `bindings: &[]`
through `design_independent` (D177), so none appeared in any of the 636
runs. It surfaces the day the design gate opens.

---

## 4. Decided versus undecided — whether the journal work pays off

**For the six manuscripts at their picked journal** (journal rows only, with
the depends-on-type row counted as undecided because its own text says so):

| manuscript → journal | journal rows | decided | undecided | decided **and correct** (hand-read) |
|---|---|---|---|---|
| R PAPER → ESWA | 2 | 2 (100%) | 0 | 2 |
| IJAS → PLOS ONE | 5 | 5 (100%) | 0 | **2**. Abstract wrong (item 2); consent and ethics are conditional on subjects this study does not have (item 4) |
| Health Economics → Value in Health | 3 | 3 (100%) | 0 | **2**. "120" is the Highlights limit |
| chapter3 → Environmental Pollution | 4 | 2 (50%) | 2 | **1**. Consent rests on a clinical-studies sentence |
| Lake → STOTEN | 4 | 3 (75%) | 1 | **1**. Consent and ethics-exemption are conditional |
| final-L → Water Research | 3 | 2 (67%) | 1 | 2 (abstract direction right, count wrong) |
| **total** | **21** | **17 (81%)** | **4** | **10 of 21 (48%)** |

**Across all 106 seeded journals** `[probe census, 6 × 106 runs]`, a paper
with an abstract gets **284 journal rows, 2.7 per journal**:

* **33** rows flagged undecided, plus **23** depends-on-type rows drawn ✓,
  make **56 undecided (20%)** and **228 decided (80%)**.
* Of the 228, **94 are a competing-interests row**. On 86 journals that row
  rests only on the same Elsevier sentence (*"…The declarations tool should
  always be completed"*).
* **74 are an abstract-limit row.** On 59 journals it rests only on Elsevier's
  template sentence (*"…does not exceed 250 words"*).
* **7 are a single word limit.** The other 9 word-limit rows are type-scoped
  and undecided.
* **53 are other statement checks** that look for a phrase: informed consent
  27, ethics approval 13, data availability 6, and 7 more.
* **3 journals give no journal row at all**: The Lancet, Nature Communications
  (0 seed rows) and The Journal of Nutrition (4 rows, none of a kind the
  checklist reads).

**What the seed holds and the checklist never reads** `[src]` + seed:
`reporting_standard` (153 rows) and all 146 bindings are withheld by the
declined design gate (D177). `figure_limit` (70), `reference_limit` (25) and
`reference_style` (22) have no checklist reader. **At most 396 of 666 seed
rows can reach a reader.**

**The plain answer.** For a real paper, the journal layer adds **2–5 rows**,
and most of them are decided. On the six measured, **fewer than half the
journal rows are both decided and correct**. Every wrong one is one of four
kinds:

* a section boundary (IJAS's abstract)
* a row stored from a sentence that is not that requirement (Highlights, the
  Capsule, the trial registry)
* a conditional requirement applied without its condition (informed consent)
* the depends-on-type ✓

Across 106 journals the decided share is 80%. But its two largest rows are
the same publisher template on almost every journal, so going from 30 to 106
journals added mostly the same two checks. **The number that pays off is not
the decided fraction. It is decided-and-correct, and on this sample that is
10 of 21.**

---

## 5. Limitations

* **Consent denied, in-memory database, no proxy, no Ollama.** These are the
  audit's conditions. The verification lane, plagiarism library, reviewer
  letter and Copilot were not exercised. Their verdicts carry over from the
  audit by source inspection.
* **No frontend was driven.** "Draws ✓" is from `ReportViewerPage.tsx:487`
  and `exportPdf.ts:30`, read and not clicked.
* **One reader, no second.** This applies to the "correct" column in §4 and
  to the IJAS and Health Economics abstract counts (`pdftotext`, `textutil`,
  hand-bounded).
* **The applicability calls in item 4 are judgements.** Whether PLOS ONE's
  ethics statement covers silkworms is arguable. They are counted as not
  correct because the stored sentence states a condition the manuscript does
  not meet.
* **The census used the same six manuscripts against every journal**, which
  measures row production, not 636 plausible submissions.
* **Seed rows were read only where a probe surfaced them.** The 666 were not
  re-audited. The Capsule, "3 authors" and 13-journal registry findings come
  from a scan of limit rows for non-manuscript words, which is not a complete
  pass.
* Health Economics' abstract counts as 306, against 273 by hand up to
  `Keywords:`. The keywords line is the likely difference; this was not
  traced.

## 6. Probes run

| probe | input | output |
|---|---|---|
| `examples/zz_publishready_at_head.rs` (release, **untracked**) | six manuscripts, each against its journal (§1); seeded and reconciled as at startup | §1 tables; full report JSON per manuscript |
| same, census mode | each manuscript × all 106 seed keys (636 runs, ~7 s per manuscript) | §4 census |
| `examples/zz_thesis_consistency_now.rs` (release, **untracked**) | six manuscripts through `preview_thesis_audit` | 14 / 2 / 8 / 18 / 0 / 95 findings; both Preacher rows |

Both probes are left untracked beside the other `zz_*` examples, per the brief
("commit the report alone").
