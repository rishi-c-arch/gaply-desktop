# PublishReady at HEAD — re-measured, 2 Oct 2026

**HEAD:** `cb65e32` (main, 2 Oct 2026, D268).
**Baseline:** `docs/PUBLISHREADY_AT_HEAD_2026-10-01.md`, measured at `76b112a`.
HEAD is **12 commits** past it (`git rev-list --count 76b112a..HEAD`): D259–D268.
**Mode:** measurement only. No product change. This file is the only committed
output.

**Same method, same journals, same corpus.** It uses the same probe
(`zz_publishready_at_head`) with the same startup order: seed, then D225, D238,
D240 and D239. `lib.rs` still runs exactly that sequence, and all four
reconciles remove 0 on a fresh seed. The run is `run_pipeline_measured` (the
body of `run_publishready`), a release build, `GAPLY_DISABLE_DEEP=1`, with
consent denied. The six corpus hashes match the baseline's (`859880647c45`,
`effbb86c2495`, `de1e322a95f1`, `134956ff25ef`, `af1f36ff2c81`,
`77e91296924b`). The seed now loads **106 journals, 643 requirements, 146
bindings**, against 666 requirements at the baseline. The 23 rows were removed
by D261, D262 and D265.

**Known-good row.** R PAPER → ESWA gives `concern`, 9 findings (3 / 3 / 3),
identical to the baseline. Two runs produced identical output.

**Provenance tags.** `[probe]` means a live run at HEAD (§6). `[src]` means
source inspection at HEAD. `[hand]` means a hand-read of the manuscript text,
by one reader.

---

## 1. The six manuscripts at their picked journal `[probe]`

| manuscript | journal | verdict | override flipped it? | findings (Major / minor / info) | vs 1 Oct |
|---|---|---|---|---|---|
| R PAPER | Expert Systems with Applications | **concern** | **yes** | 9 (3 / 3 / 3) | same |
| IJAS | PLOS ONE | pass | no | 6 (1 / 2 / 3) | same |
| Health Economics | Value in Health | **concern** | **yes** | 10 (3 / 4 / 3) | same |
| chapter3 | Environmental Pollution | pass | no | 5 (0 / 2 / 3) | same |
| Lake Chapter 1 | Science of the Total Environment | pass | no | 5 (0 / 2 / 3) | same |
| final-L | Water Research | **concern** | **yes** | 9 (3 / 3 / 3) | same counts; "missing confidence interval" now raised at 19 places (D268, §3 item 5) |

**Verdicts and finding counts did not move.** The 12 commits since the
baseline changed the checklist, not the debate or the findings.

### Checklist

"Undecided" means `unevaluable: true`, which the screen now draws as `–` and
the PDF prints as "not decided" (`ReportViewerPage.tsx:488`,
`exportPdf.ts:30`) `[src]`.

| manuscript | structural P / F | journal rows | journal P / F / undecided | 1 Oct |
|---|---|---|---|---|
| R PAPER | 4 / 0 | 2 | 1 / 1 / 0 | 2: 1 / 1 / 0 |
| IJAS | 4 / 0 | 5 | 1 / 3 / **1** | 5: 0 / 5 / 0 |
| Health Economics | 4 / 0 | 2 | 1 / 1 / 0 | 3: 1 / 2 / 0 |
| chapter3 | 1 / **3** | 3 | 0 / 1 / 2 | 4: 0 / 2 / 2 |
| Lake Chapter 1 | 0 / 4 | 4 | 0 / 2 / 2 | 4: 0 / 3 / 1 |
| final-L | 4 / 0 | 3 | 0 / 2 / 1 | 3: 0 / 2 / 1 |
| **total** | | **19** | **3 / 10 / 6** | **21** |

What moved, and why:

| row | 1 Oct | now | record |
|---|---|---|---|
| IJAS abstract limit 300 | ✗, "588 words" | **✓, 248 words** | D260 |
| IJAS informed consent | ✗ | **undecided** | D264 |
| Value in Health "word limit: 120" (Highlights) | ✗ | **gone** | D261 |
| chapter3 "required section: Abstract" | ✓ (a "3.10 Summary") | **✗, "Abstract section missing"** | D268 |
| chapter3 abstract-limit row | present | **gone**, no abstract to count | D268 |
| Lake informed consent | ✗ | **undecided** | D264 |
| final-L abstract count | 9,764 words | **799 words** | D268 |
| chapter3 / final-L depends-on-type row | drawn ✓ | **drawn `–`, "not decided"** | D259 |

Specialists are unchanged from the baseline. R PAPER and IJAS each fire
`parametric_test_assumptions_unstated` (shown). Health Economics fires
`multiple_comparisons_uncorrected`, which is dropped (D233).
`claim_evidence_strength` declines on five of the six. Lanes are also
unchanged: `verification_examined` and `plagiarism_examined` are false on all
six, and Lake additionally has `validation_examined` and
`extraction_examined` false.

---

## 2. Decided versus decided-and-correct

The 1 Oct figure is **17 of 21 decided, and 10 of 21 decided and correct**.

| manuscript → journal | journal rows | decided | undecided | decided **and correct** `[hand]` |
|---|---|---|---|---|
| R PAPER → ESWA | 2 | 2 | 0 | **2**. Abstract 181 words (hand: 182 tokens including the `Abstract-` label) ✓. No competing-interests sentence anywhere in the text ✗ |
| IJAS → PLOS ONE | 5 | 4 | 1 | **4**. Abstract 248 ✓ (D260's hand count). There is no data-availability, ethics or competing-interests sentence anywhere. The only matches for the statement words are a "Funding Source" line and a co-author placeholder |
| Health Economics → Value in Health | 2 | 2 | 0 | **2**. Abstract 287 > 250 ✗ (D260). "Data Availability: Available from the corresponding author on reasonable request." ✓ |
| chapter3 → Environmental Pollution | 3 | 1 | 2 | **1**. No competing-interests sentence ✗ |
| Lake → STOTEN | 4 | 2 | 2 | **2**. No competing-interests sentence ✗. Ethics ✗ on the exemption clause (see below) |
| final-L → Water Research | 3 | 2 | 1 | **2**. Abstract over 250 ✗; the direction is right and the count includes a footer (§3 item 4). No competing-interests sentence ✗ |
| **total** | **19** | **13 (68%)** | **6** | **13 of 19 (68%)** |

**Every decided journal row is correct on this sample.** The baseline was 10 of
21 (48%).

**Two calls differ from the 1 Oct reader, and the reasons are stated so the
comparison stays honest.** On the 1 Oct reading both would be wrong, and the
figure would be **11 of 19**.

* **Lake, ethics ✗,** on *"If a study was granted exemption or did not require
  ethics approval, a statement detailing this should be included"*. 1 Oct
  counted it wrong as a conditional requirement. **D264 recorded Rishi's
  decision (2 Oct) that the exemption-clause rows keep failing**: Elsevier asks
  every submission to say something, "none needed" included. Counted correct by
  that decision.
* **IJAS, ethics ✗,** on PLOS ONE's *"If the study made use of human or
  animal subjects and/or tissue, you must provide an ethics statement."* The
  study uses silkworm haemolymph, which is animal tissue, so the condition the
  sentence states is met. D264's rule fails a human-or-animal row only on
  animal evidence, and it found some. Whether PLOS ONE expects an ethics
  statement for an invertebrate in practice is a judgement the sentence does
  not make.

**What the 6 undecided rows are.**

* **Three informed-consent rows** (IJAS, chapter3, Lake), on human-scoped
  sentences, where no human participants are shown. That is right.
* **One Short-Communication-only word limit** (Lake). That is right.
* **Two depends-on-type rows** (chapter3 at 10,733 words against
  10,000 / 3,000 / 8,000 / 4,000, and final-L at 117,337 against 3,000 /
  8,000). **Both exceed every stated limit**, so a FAIL holds whatever the
  article type. D259 chose not to decide that, and it is not wrong; §3 item 6
  covers it.

### Across all 106 journals `[probe zz_verdict_control]`

This is a different instrument from the baseline's census. The baseline ran
`run_pipeline_measured` × 106. This runs `build_checklist` × 106 over the same
startup-reconciled seed. That is the function the pipeline calls for the
checklist, but the two figures are not from one probe.

| | 1 Oct (a paper with an abstract) | now (R PAPER) |
|---|---|---|
| journal rows | 284 (2.7 per journal) | 281 (2.7 per journal) |
| undecided | 56 (20%) | **74 (26%)** |
| decided | 228 | 207 |
| competing interests (decided) | 94 | 94 |
| abstract limit (decided) | 74 | 85 |
| journals with no journal row | 3 | 3 |

**The rise in decided abstract rows (74 → 85) was not traced to a record.**
D262's removal of the registry 500s, which made 13 rows "more than one stated",
is the likely cause, but it is a prediction, not a measurement.

**The undecided count rises by 18, not by the 27 consent rows D264 made
undecided,** because other undecided rows left at the same time. A paper
without human participants gets 27 undecided consent rows and 3 undecided
ethics rows. Health Economics states consent and gets 44 undecided rows (16%),
30 fewer than R PAPER.

---

## 3. What a user still sees that is wrong, ranked as before

Ranked by exposure (how many users meet it) times how plainly wrong it reads.
Baseline numbering is given in brackets.

**Fixed since 1 Oct, and confirmed on these runs:**

* [1] A depends-on-type row drawn ✓ is now `–` / "not decided" (D259).
* [2] IJAS's abstract is 248 words, ✓ (D260). The Capsule's 30 (D261) and the
  13-journal registry 500 (D262) are gone. final-L's 9,764 is now 799 (D268).
* [3] The Value in Health Highlights "120" is gone (D261), and so is
  Fertility and Sterility's "3 authors" (D261).
* [4] Informed consent on non-human studies is undecided (D264). Lake's ethics
  ✗ stands by decision (§2).
* [5] Single-source rows show the journal's sentence (D263,
  `ReportViewerPage.tsx:526`).

**Still wrong, ranked:**

1. **[7] `pass` above a Major.** IJAS reads `pass` with "No statistical
   assumption check detected: ANOVA" (Major) on the same page. Unchanged.
   `[probe]`
2. **[6] The D235 override forces `concern`.** R PAPER, Health Economics and
   final-L read `concern` with `overridden_by_constraint: true` and combined
   confidence 1.0. Unchanged. **Together with item 1, the verdict line is now
   the most visible wrong thing on the page**, because the checklist rows below
   it are, on this sample, correct where decided. `[probe]`
3. **[10] Lake Chapter 1: `extraction_examined: false`, yet the report shows
   "Extraction: pass".** Unchanged. `[probe]`
4. **final-L's abstract count includes a Turnitin footer.** 799 words, of which
   the last 70 are *"232 Page 50 of 401 - Integrity Submission Submission ID
   trn:oid:::3117:616484955 …"*. The verdict direction holds (the real abstract
   is about 729 > 250). It was recorded and not fixed (D268): one file, an
   artefact of the export. **New since 1 Oct only in that the 9,764 masked
   it.** `[probe]`
5. **final-L: 12 missing-confidence-interval flags are no longer raised.** They
   sit on real Chapter 4 results that the splitter absorbs into
   "3.10 Summary", because those sub-headings are not detected. They were
   flagged at the baseline only because that section was typed Abstract. The
   Major still shows ("raised at 19 places"), so **nothing on screen reads
   wrong**: the omission is invisible. Recorded as D268's section-boundary
   residual. `[probe]`
6. **[1, remainder] A depends-on-type row whose count exceeds every stated
   limit is undecided, not ✗.** chapter3 (10,733 against a maximum of 10,000)
   and final-L (117,337 against 8,000). Every type fails, so `–` is weaker than
   the evidence supports. It is not false. D259 declined this as a separate
   rule. `[probe]`
7. **IJAS's data-availability ✗ quotes a sentence about code.** The row's
   quote is *"If code cannot be shared due to legal or ethical reasons then
   authors should state this in the Data Availability Statement…"*. The verdict
   is right (PLOS ONE requires the statement, and the manuscript has none), but
   the sentence shown is not the requirement, so a reader checking the quote
   would doubt a correct row. `[probe]`
8. **[8] Health Economics' `multiple_comparisons_uncorrected`** is computed
   and withheld (D233). That is still the right call. There is no user-visible
   defect. `[probe]`
9. **[9] Preacher.** Health Economics' thesis audit still lists both Preacher
   & Hayes rows (2004, 2008) as "Listed but never cited", and both are false.
   Thesis audit only. Counts at HEAD, R PAPER / IJAS / HE / chapter3 / Lake /
   final-L: **8 / 2 / 14 / 18 / 0 / 95**. The same six counts as 1 Oct, which
   listed them as 14 / 2 / 8 / 18 / 0 / 95; the earlier order is not recorded
   with the numbers. `[probe]`

**Latent, not visible.** The reader-facing strings with 22-space runs are still
in `unbound_standard_findings`, now at `report.rs:2714` and `:2723`.
`design_independent` and `bindings: &[]` still withhold them (D177), and
neither appeared in any of the six runs. `[src]` + `[probe]`

---

## 4. The plain answer

**The journal layer's rows are now right where they decide.** That is 13 of 13
on this sample, against 10 of 17 on 1 Oct. Each of the four failure kinds the
baseline named was addressed:

* the section boundary (D260, D268)
* rows that were not the requirement (D261, D262, D265)
* conditions applied without their condition (D264)
* the depends-on-type ✓ (D259)

The cost is fewer decided rows: 13 of 19 against 17 of 21, and 26% undecided
across 106 journals against 20%. **Every row that left the decided column was
a wrong one.**

What a user now meets first is the **verdict line, not the checklist**: `pass`
over a Major on IJAS, and an overridden `concern` on three of six.

---

## 5. Limitations

* **Consent denied, in-memory database, no proxy, no Ollama**, as at the
  baseline. Verification, the plagiarism library, the reviewer letter and
  Copilot were not exercised.
* **No frontend was driven.** `–` / "not decided" and the single-source quote
  are from `ReportViewerPage.tsx:488,526` and `exportPdf.ts:30`, read and not
  clicked.
* **One reader, no second**, for the "correct" column. The two ethics calls in
  §2 are judgements, stated with the 1 Oct alternative (11 of 19).
* **The competing-interests ✗ rows are counted correct**, as on 1 Oct: none of
  the five manuscripts that fail it has such a statement. Whether Elsevier's
  *"The declarations tool should always be completed"* asks for a statement in
  the manuscript, or for a form at submission, was not re-examined. That
  sentence is the source on most of the 94 journals.
* **The 106-journal figures use a different probe** from the baseline's census
  (§2). They are comparable in what they count, not in how they were produced.
* **final-L's "real abstract ≈ 729"** is 799 minus the 70-word footer
  paragraph. The footer was not hand-bounded inside the paragraph.

## 6. Probes run

| probe | input | output |
|---|---|---|
| `examples/zz_publishready_at_head.rs` (release, **untracked**) | the six manuscripts at their journals (§1), seeded and reconciled as at startup | §1–§2; full report JSON per manuscript |
| `examples/zz_verdict_control.rs` (release, **untracked**) | each of the six texts (`parse_path`) × 106 journals through `build_checklist`, startup-reconciled seed | §2's 106-journal table |
| `examples/zz_thesis_consistency_now.rs` (release, **untracked**) | the six manuscripts through `preview_thesis_audit` | 8 / 2 / 14 / 18 / 0 / 95; both Preacher rows |

The probes are left untracked beside the other `zz_*` examples, per the brief
("commit the report alone").
