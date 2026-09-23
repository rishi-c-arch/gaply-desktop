# PublishReady — Problem Dossier

**Purpose.** A precise statement of every known defect, written so that
research-backed solutions can be proposed against it. **It proposes no fixes.**

**Reference manuscript.** `R PAPER .docx` — *"FIREFLY-CROW SEARCH OPTIMIZED
BIDIRECTIONAL LSTM FOR EMOTION DETECTION IN SOCIAL MEDIA TEXT"*, a CS/NLP paper,
target journal **Nature Medicine (Q1)**. Its stored analysis is
**run 30** (`report:v2:e3:30`, 20,116 bytes, 2026-09-22 08:20:03, manuscript 30),
read from `~/Library/Application Support/ai.gaply.app/gaply.db`. Every symptom
below is quoted from that stored report unless stated otherwise.

**Evidence tags.** `[ran]` = I executed it this session and the output is quoted.
`[read]` = I read the code and cite file:line. `[doc]` = asserted by a document
(CLAUDE.md, a D-record) and not independently re-measured.

**Predictions.** Where I predicted before measuring, both prediction and result
are recorded, including the two I got wrong.

**Note on A1.** A1 was diagnosed AND fixed earlier in this same session
(`0425f4b`, local, unpushed). It is retained because the dossier is meant to be
complete; its status is marked.

---

## Ranked table

Impact is to a researcher judging whether to trust the tool. "Blocks expert
reviewer" means the stated goal — a letter comparable to a Q1 peer reviewer —
cannot be reached while it stands.

| # | Problem | Class | Impact | Confidence | Blocks expert reviewer |
|---|---|---|---|---|---|
| **A2** | "Results section missing" on a paper that has one | extraction | **5** | measured | yes |
| **C1** | Scientific layer below no-skill at every model tier | missing input / task definition | **5** | measured | **yes — central** |
| **A3** | Consent/ethics "not met" on a paper with no human subjects | heuristic/lexicon + missing input | **5** | measured | yes |
| **B3** | The letter never sees the manuscript | privacy boundary / missing input | **5** | measured | **yes — central** |
| **A6** | AI-detection "concern" driven by the bibliography | heuristic/lexicon | **4** | measured | no |
| **A4** | Effect-size/CI flagged on a significance *criterion* | extraction | **4** | partly | no |
| **A8** | 25 of 25 citations unchecked | wiring/configuration | **4** | partly | yes |
| **B1** | "Reviewer unavailable" in the app | wiring/configuration | **4** | partly | yes |
| **A5** | Journal quotes cut mid-word at 400 chars | extraction | 3 | measured | no |
| **A7** | MTLD 85 and 10-year recency are hardcoded, not field-aware | heuristic/lexicon | 3 | measured | no |
| **C4** | SLM-1/SLM-2 unusable; training provenance undocumented | model capability | 3 | measured | partly |
| **C6** | Only SPSS analysis files parse | extraction | 3 | read-only | no |
| **A1** | Paragraph index disagreement (**fixed `0425f4b`**) | presentation | 3 | measured | no |
| **C3** | agent_graph 9 nodes vs a 40–50 agent design | wiring | 3 | read-only | yes |
| **B2** | Four reviewer lenses with no caller | wiring | 2 | partly | partly |
| **C5** | Novelty 0.1 claims/paper | measurement/instrument | 2 | doc | no |
| **D3** | User-facing behaviour guarded only locally | measurement/instrument | 2 | measured | no |
| **D2** | commands.rs error wiring untested | measurement/instrument | 2 | measured | no |
| **B4** | Temperature unset; probability wanders 40–60 | configuration | 1 | measured | no |
| **D4** | `tauri build` fails at bundle_dmg.sh | environment (transient) | 2 | **characterized** — step known, trigger unknown (§11 D209) | no |
| **D5** | DMG ad-hoc signed, not notarized | configuration | 2 | measured | no (blocks shipping) |
| **D6** | DMG wrapper not byte-reproducible | reproducible build | 1 | measured | no |
| **D1** | 8000-char boundary vs whole-paper review | privacy boundary | **5** | measured | **yes — central** |

**Four problems are load-bearing for the stated goal and are research-level, not
engineering-level: C1, B3, D1, A3.** They are the subject of the final section.

---

## A. REPORT ACCURACY (run 30)

### A1 — Paragraph index disagreement *(FIXED this session, `0425f4b`, unpushed)*

**1. Symptom.** Run 30's provenance, verbatim `[ran]`:
`"location:Abstract paragraph 0"`, `"location:Methods paragraph 125"`. The
exported PDF prints `"Abstract, paragraph 1"` and `"paragraph 126"` for the same
findings. No document has a paragraph 0.

**2. Reproduction `[ran]`.**
`sqlite3 gaply.db "select value from cache where key='report:v2:e3:30'"` →
`findings[0].provenance` contains `location:Abstract paragraph 0`.

**3. Mechanism `[read]`, every surface that prints a location:**

| site | surface | base |
|---|---|---|
| `gaply-core/src/report.rs:656` | provenance → Inspector | **0** |
| `src/screens/checks/adapters.ts:149` | Inspector (frontend mirror) | **0** |
| `src/report_build.rs:79` | report model → PDF | 1 |
| `gaply-core/src/validate.rs:349` `¶{n+1}` | findings table | 1 |
| `gaply-core/src/extract/persist.rs:21` `¶{n+1}` | findings table | 1 |
| `ai_engine/audit_prepass.rs:1089` → `audit_report.rs:239` | citation-audit PDF | 1 |

`Location.paragraph` is a 0-based index into `Section::paragraphs`; five of six
surfaces added 1, the provenance line did not.

**4. Root cause class.** Presentation.

**5. Measured vs unknown.** Fully measured. No unknowns.

**6. Blast radius.** Any surface rendering `Finding.provenance` verbatim, and the
reviewer payload — `build_review_payload` forwards structured provenance to the
model, so the model also saw "paragraph 0".

**7. Acceptance test.** One `Location` rendered through both surface formatters
yields the same integer, and that integer is `index + 1`. **Negative control:**
making both surfaces 0-based must FAIL — an agreement-only assertion passes that.

**8. Constraints.** The structured `Location.paragraph` must stay 0-based (it
indexes a `Vec`). The golden capture `report.golden.json` pins the provenance
string byte-for-byte.

**9. Prior attempts.** None. §11 D169 fixed *resolution* (`section_index`), not
*display base*.

### A2 — "Results section missing" on a paper that has a Results section

**1. Symptom.** Checklist row 3 `[ran]`: `[NOT MET] required section: Results —
"Results section missing"`. The manuscript contains
`'IV. EXPERIMENTAL SETUP AND RESULTS'`.

**2. Reproduction `[ran]`.** Extraction over `R PAPER .docx` yields 8 sections
and no `Results`:

```
[0] Other          6 paras  heading=''
[1] Abstract       2 paras  heading='Abstract'
[2] Introduction  14 paras  heading='Introduction'
[3] Methods       66 paras  heading='Methodology'
[4] Methods      126 paras  heading='Method'
[5] Discussion     3 paras  heading='DISCUSSION'
[6] Conclusion     4 paras  heading='VI.CONCLUSION'
[7] References    25 paras  heading='References'
```

The compound heading survives as **body text**: `sec[3] Methods para 59:
'IV. EXPERIMENTAL SETUP AND RESULTS'`.

**3. Mechanism `[read]`, end to end.** `extract/sections.rs:54 detect_heading`
requires ≤5 words, strips leading numbering, then `classify_heading` matches an
**exact phrase**: `"results" | "findings" | "results and discussion"`.
`"experimental setup and results"` is 4 words (passes the length gate) and is not
in the list, so it is not a heading; the splitter keeps accumulating into the
open section. Result: a **126-paragraph "Methods" section** containing the
paper's entire results, tables and figures. The checklist then reports Results
absent. *Steps run: extraction and the heading search. Step read:
`classify_heading`.*

**4. Root cause class.** Extraction.

**5. Measured vs unknown.**
*Measured:* **3 of 6 corpus manuscripts have no detected Results section**
(`Lake Chapter 1`, `chapter3`, `R PAPER`) `[ran]`.
*Unknown:* the population frequency of compound headings in real submissions, and
how many distinct compound forms exist. **Experiment:** harvest heading lines
from a 100+ manuscript corpus, cluster the ones containing a section keyword but
failing exact match, and report coverage of any candidate rule.

**6. Blast radius.** Everything keyed on `SectionKind`: the checklist's
required-section rows; §4 statistical rules that are section-scoped; the no-skill
baseline in D196–D207 (defined as "first paragraph of each Methods section");
A4's anchoring (the mega-section is why "Methods paragraph 125" exists); the
reviewer payload's section labels; and A6 (results tables filed as Methods).

**7. Acceptance test.** For a manuscript whose only results heading is compound,
a `Results` section exists, its paragraph count is plausible, and the preceding
Methods section shrinks correspondingly. **Negative control:** a paper with a
sentence *beginning* "Results show that…" as body prose must NOT gain a section —
the exact-match rule exists to prevent that, and D-record §11 D188 refuted three
requirement-matching rules for the same reason.

**8. Constraints.** §11 D188 (three refuted matching rules); the `detect_heading`
trap recorded in §11 D189 — `[.)]?\s*` lets `[IVXLCM]+` eat leading
numeral-letters, so any regex change must keep `detect_heading("Methods")`
returning `Some`. `gaply-core/tests/` heading tests and the golden capture would
move.

**9. Prior attempts.** §11 D168 (`detect_table`'s body-follows fix) was REVERTED.
§11 D189 fixed two of four heading mechanisms, declined one, left one open.

### A3 — Consent, ethics and informed-consent rows "not met" on a paper with no human participants

**1. Symptom `[ran]`.** Rows 8, 10, 11, 13:
`[NOT MET] informed consent statement — "none of 1 phrasings (consent) was found
anywhere in the manuscript"`; `[NOT MET] ethics statement — "none of 1 phrasings
(ethic) was found"`. R PAPER is a deep-learning study on public social-media
text. There are no human participants, no donors and no recipients.

**2. Reproduction `[ran]`.** The 13-row checklist in run 30; four rows are
human-subjects requirements.

> **CORRECTION (§11 D210): the row list above is wrong in two places.** Row 11
> (competing interests) is a CORRECT failure — its span is mis-attached, but the
> requirement is unconditional on two other Nature Medicine pages — and row 12
> (code availability) is correct too, since R PAPER used custom code. **Row 6
> (abstract limit, scoped to Brief Communication) is wrong and was missing from
> the list.** The set is {6, 8, 10, 13}. Rows 6 and 13 are fixed by §11 D209;
> 8 and 10 are not, and `docs/A3_APPLICABILITY_MEASUREMENT.md` part 8 measures
> why the evidence for them does not exist.

**3. Mechanism `[read]`.** The checklist is a **substring search over the whole
manuscript for a phrasing list**, per its own `detail` strings ("none of N
phrasings (…) was found anywhere in the manuscript"). The requirement rows come
from the journal profile (Nature Medicine), which states them because the journal
publishes clinical research. **Nothing between the journal's requirement and the
manuscript asks whether the requirement applies.** *Read, not run: I did not
trace the requirement-selection path to its source module.*

**4. Root cause class.** Heuristic/lexicon **and** missing input — the missing
input is study design.

**5. Measured vs unknown.**
*Measured:* the checklist's decision procedure is phrase presence/absence, from
its own emitted detail `[ran]`.
*Unknown:* where applicability could come from. Candidates: (a) study-design
classification of the manuscript; (b) an article-type declaration by the user;
(c) journal requirement metadata marking rows conditional. **Experiment:** label
study design for the 6-manuscript corpus by hand, then measure how many of the 13
rows change status under each candidate; and separately, measure whether journal
guideline text states the condition explicitly often enough to be extracted
(row 10's own span says *"informed consent was obtained from all recipients
and/or donors of human material"* — the condition is IN the text and unused).

**6. Blast radius.** Every checklist row for every journal; the reviewer letter,
which receives the checklist (`MAX_CHECKLIST = 20`) and can cite a false FAIL as
grounding; §11 D181's class of defect (a false FAIL telling an author to add a
declaration).

**7. Acceptance test.** On a paper with no human subjects, the four
human-subjects rows are not reported as compliance failures, and the run states
*why* they were not evaluated. **Negative control:** on a clinical manuscript
that genuinely omits a consent statement, the row must still be NOT MET — a fix
that simply suppresses the rows fails this.

**8. Constraints.** The three-state contract (`passed` / `unevaluable`) recorded
in CLAUDE.md: a missing value must not render as FAIL, and `unevaluable` shipped
WITH the evaluator so no stored report can contain an undecidable item.
§11 D183: the checklist is the journal's own requirements — a fix must not invent
requirements or silently drop the journal's.

**9. Prior attempts.** §11 D181 corrected a false FAIL for a declaration the
paper *did* contain (a detection failure). This is the complementary case —
detection is correct and **applicability** is absent. Not previously attempted.

### A4 — Effect size / CI flagged on the Abstract, quote does not show the p-value

**1. Symptom `[ran]`.** Finding 1: *"statistical rule failed: missing effect size
(raised at 2 places)"*, anchored `abstract ¶0`, provenance
`['rule:MissingEffectSize (MAJOR)', 'location:Abstract paragraph 0',
'location:Methods paragraph 125']`. Finding 2: missing confidence interval, same
anchor.

**2. Reproduction `[ran]`.** Abstract ¶0 is **1333 characters**. Its only
p-value is at **offset 1250**:
`"We conducted ablation studies, statistical significance (p < 0.05), and an
analysis of the interp…"`.

**3. Mechanism.** Two candidate mechanisms, and the measurement separates them
only partly.
(a) **Offset:** the p-value sits in the last 6% of a 1333-char paragraph, so any
quote shorter than ~1250 chars omits the evidence — the reader sees a claim about
a p-value beside prose containing none `[ran]`.
(b) **Kind:** `(p < 0.05)` after *"statistical significance"* is a **significance
criterion**, not a reported result. `validate.rs` has criterion handling and a
test named `a_declared_criterion_produces_no_finding`, and another asserting
*"exactly one MissingEffectSize — for the result, not the criterion"* `[read]`.
So the rule is *supposed* to exclude this shape.

**4. Root cause class.** Extraction (claim classification), with a presentation
component.

**5. Measured vs unknown.**
*Measured:* the paragraph length, the p-value offset, the criterion phrasing, and
that criterion-exclusion logic exists `[ran]`/`[read]`.
*Unknown, and this is the crux:* whether the extractor classified **this**
instance as criterion or result, and what the PDF actually quotes.
**Experiment:** dump `ReportedStatistic { kind, reported, location, criterion }`
for R PAPER and read the row for Abstract ¶0; separately render the PDF locator
for that finding and measure quote length against offset 1250.

**6. Blast radius.** Every statistical rule in §4 Tier 0 — these are HARD
CONSTRAINTS that override eight agents agreeing (`swarm.rs` `hard_constraint`),
so a misclassified criterion cannot be outvoted. Also the reviewer letter, which
receives findings severity-ordered and would cite a Tier-0 finding first.

**7. Acceptance test.** A paragraph whose only p-mention is a declared
significance threshold produces no MissingEffectSize/MissingCI finding; and any
finding that IS produced is displayed with a quotation containing the statistic
it is about. **Negative control:** a paragraph reporting `p = 0.03` with no
effect size must still be flagged.

**8. Constraints.** Tier-0 determinism: `validate.rs` claims "no model, no proxy,
no network, no I/O" in its header and that claim is currently **unguarded** (only
`equation/` has a purity test). §11 D191: the analysis cross-check fired on 29 of
31 claimed tests, and three rows showed three different defects — a warning that
this area's failures are not single-cause.

**9. Prior attempts.** §11 D191 (analysis cross-check defects). The criterion
distinction was added deliberately with tests; this is either a gap in that
detector or a display defect, and the dossier does not yet say which.

### A5 — Journal quotes end mid-word

**1. Symptom `[ran]`.** Checklist rows 7 and 11 end:
`"…a completed copy of the Nature Research Por"`.

**2. Reproduction `[ran]`.** Stored span lengths in run 30:

```
row  7: 400 chars  ends_mid_word=True   tail='ompleted copy of the Nature Research Por'
row 11: 400 chars  ends_mid_word=True   tail='ompleted copy of the Nature Research Por'
row  8: 124 chars  ends_mid_word=False
row 10: 246 chars  ends_mid_word=False
```

Every truncated span is **exactly 400 characters**.

**3. Mechanism `[read]`.** `gaply-core/src/journal_extract.rs:746`
`source_span: span.chars().take(400).collect()`, and the same cap at `:959`. The
cut is applied at **extraction time and stored truncated** — the full sentence is
not retained anywhere downstream.

**4. Root cause class.** Extraction.

**5. Prediction vs result.** I predicted the spans were stored whole and the cut
was presentational. **Wrong** — the cap is in storage `[ran]`.

**6. Blast radius.** Every journal requirement row on every journal profile; the
reviewer payload (`build_supplementary` / checklist spans go to the model, so the
model reads truncated evidence); the exported PDF and HTML.

**7. Acceptance test.** A requirement's evidence renders as a complete sentence,
and a reader can verify the requirement from it alone. **Negative control:** the
payload must still satisfy the proxy validator's 2000-char field and 8-sentence
limits — an untruncated span must not push a row past them (measured headroom:
worst observed total 6049 of 8000).

**8. Constraints.** The proxy validator limits (`MAX_FIELD_CHARS = 2000`,
`MAX_SENTENCES_PER_FIELD = 8`) are a privacy boundary, not a formatting choice;
`gaply-core/tests/grrb_context_pool_is_the_fixed_set.rs` reads them from
`gaply-proxy/app/validation.py` and goes red if they move.

**9. Prior attempts.** CLAUDE.md records the same class twice: "A TRUNCATED SPAN
IS NOT A SPAN", where a checklist data-availability span clipped at 150 chars by
a *probe* made a satisfied requirement look unsupported. The lesson was recorded;
the 400-char cap in the product was not found at that time.

### A6 — AI-detection "concern" is driven by the bibliography

**1. Symptom `[ran]`.** Finding 8: *"AiDetection: concern"*, detail *"signal
LeansAiLike (mean perplexity 8.2, burstiness 4.2)"*.

**2. Reproduction `[ran]`.** Two measurements.

*(a) The labelled corpus — 10 known-human abstracts, `evals/detector/aicheck30.json`:*

```
HUMAN rows: 10   flagged LeansAiLike: 0   FP rate: 0%
perplexity range 12.6 – 19.9, all Inconclusive
```

*(b) R PAPER, whole document vs per section:*

```
WHOLE DOCUMENT  21064 chars   ppl  7.9  LeansAiLike
  Abstract       1518 chars   ppl 11.0  LeansAiLike
  Introduction   4411 chars   ppl 12.8  Inconclusive
  Methods(66)    3870 chars   ppl 12.5  Inconclusive
  Methods(126)   4192 chars   ppl  7.5  LeansAiLike
  Discussion     1616 chars   ppl 13.8  Inconclusive
  Conclusion     1246 chars   ppl  8.1  LeansAiLike
  References     3404 chars   ppl  5.5  LeansAiLike
```

**3. Prediction vs result.** I predicted a **60–90% false-positive rate** on the
human abstracts, reasoning that academic prose is formulaic. **Measured 0%.**
The prediction was wrong and the wrongness is informative: the detector is not
broadly miscalibrated on prose. Its prose sections (12.5–13.8) sit in the same
band as the 10 human abstracts (12.6–19.9).

**4. Mechanism `[ran]` + `[read]`.** `AI_LIKE_PPL = 12.0` (`ai_detect.rs:339`).
**Non-prose content scores far below it:** the reference list at 5.5, and the
126-paragraph section containing the results tables at 7.5. The document-level
mean (7.9) is therefore set by the bibliography and tables, not by authored
prose, and crosses the threshold. The thresholds' own comment says they are
*"NOT calibrated against real GPT-2 output"* `[read]`.

**5. Root cause class.** Heuristic/lexicon — specifically, **scoring non-prose as
prose**.

**6. Measured vs unknown.**
*Measured:* per-section perplexities and the 0% FP rate on prose.
*Unknown:* how much of the effect is the reference list alone versus tables, and
whether excluding non-prose regions leaves a usable signal at all.
**Experiment:** recompute the document signal over prose-only regions
(References and table-like blocks removed) across the 6-manuscript corpus, and
re-measure the flag rate; then re-run the 30-row labelled set to check the
GENERATED/PARAPHRASED recall does not collapse.

**7. Blast radius.** The AI-Check lane end to end; §11 D203 (the three-way lane
is UNEVALUABLE because its prompt presupposes the verdict); the swarm, which
receives AiDetection as an opinion; and A2, because the compound-heading failure
is why 126 paragraphs of tables are labelled "Methods" rather than Results.

**8. Acceptance test.** A manuscript whose prose is human and whose bibliography
is long is not reported as AI-like, while the 10 GENERATED rows in the labelled
set remain detected. **Negative control:** an AI-generated manuscript with an
equally long bibliography must still be flagged — a fix that merely raises the
threshold fails this.

**9. Constraints.** The mandatory disclaimer must remain (`ai_detect.rs`, never
empty). §11 D200: SLM-1's adapter does not beat its own base, so a deep model is
not currently an available substitute. §11 D203 declined the three-way lane.

**10. Prior attempts.** §11 D196/D200/D201/D203 all measured model-based
alternatives to this proxy and none shipped.

### A7 — MTLD 196 vs reference 85, and "18 of 24 references older than 10 years"

**1. Symptom `[ran]`.** *"lexical diversity deviates from the academic
reference — MTLD 196, deviating 111 from a human-academic reference of 85"*, and
*"18 of 24 dated reference(s) are older than 10 years"*.

**2. Reproduction `[ran]`.** Provenance:
`evidence:mtld=196;deviation=111;reference=85` and
`evidence:references=25;dated=24;undated=1;older_than=10y;count=18`.

**3. Mechanism `[read]`.** `REFERENCE_RECENCY_YEARS: i32 = 10`
(`report.rs:1143`) — a single global constant, documented as *"a conventional
review horizon… a COUNT for a human to weigh"*. The MTLD reference of 85 is
likewise a fixed scalar formatted into the message at `report.rs:1252`. Neither
is parameterised by field, article type, or journal.

**4. Root cause class.** Heuristic/lexicon.

**5. Measured vs unknown.**
*Measured:* both values are constants and the run's numbers `[ran]`/`[read]`.
*Unknown:* what the correct reference distribution is per field. A deep-learning
paper legitimately cites LSTM (1997) and Adam (2014); a clinical trial paper
would not. **Experiment:** compute MTLD and citation-age distributions over a
field-stratified corpus (CS/NLP, clinical, ecology, economics) and report whether
85/10 falls inside or outside each field's interquartile range. Until then the
deviation is not evidence of a defect in the manuscript.

**6. Blast radius.** Both are `minor` findings entering the reviewer payload and
the swarm; MTLD also feeds the AI-detection narrative.

**7. Acceptance test.** For a manuscript in a field whose norms differ from the
global constant, the row either does not fire or states the comparison class it
used. **Negative control:** a genuinely thin bibliography (say 2 dated refs, both
20 years old) must still surface.

**8. Constraints.** ONTOLOGY §4.20 PRESENTATION class — a number displayed with
more precision or authority than its derivation supports is the defect
`ReviewerLetterPanel` removed the probability gauge for.

**9. Prior attempts.** None recorded for field-awareness.

### A8 — "25 of 25 citation(s) could not be checked"

**1. Symptom `[ran]`.** Finding 6, with provenance
`agent:verification (service unavailable — no verdict attempted)` and detail
*"25 of 25 citation(s) were not checked against the literature, so nothing is
claimed about them either way"*.

**2. Reproduction `[ran]`.** The finding as stored in run 30.

**3. Mechanism `[read]`.** Verification is the **only** agent that reaches the
network, exclusively through the proxy (`ProxyClient` seam, CLAUDE.md agent
table). Its evidence comes from `refverify.rs` connectors (CrossRef, OpenAlex,
Retraction Watch, Unpaywall, Semantic Scholar). The wording *"service
unavailable — no verdict attempted"* is the honest degraded path: it declines
rather than guessing. *Read, not run: I did not execute a verification pass.*

**4. Root cause class.** Wiring/configuration.

**5. Measured vs unknown.**
*Measured:* that the lane declined and recorded why `[ran]`.
*Unknown:* **which** service was unavailable and why — proxy unreachable, App
Check key mismatch, entitlement, or an upstream connector. This matters because
until `be77cd8` the proxy collapsed five distinct upstream failures into one
opaque 500 (see D2), so the run-30 log could not have distinguished them anyway.
**Experiment:** run the pipeline with the proxy up and a matching App Check key,
capture the per-citation outcome, and separately measure how many of the 25 refs
resolve against CrossRef/OpenAlex **without** the proxy — those connectors are
public APIs and the question of what could run locally is empirical.

**6. Blast radius.** The citation-audit PDF; the reviewer letter (a "could not be
checked" finding is one of the 8 it receives); §11 D192's journal layer, which
shares the fetch path.

**7. Acceptance test.** With connectivity, a majority of well-formed references
receive a verdict, and each unchecked one names *which* lookup failed.
**Negative control:** with the network down, the lane must still decline rather
than assert — the current honest behaviour must not regress into a guess.

**8. Constraints.** CLAUDE.md: never check a fetch with `curl` — a curl result is
a fact about curl. Any reachability claim must go through `ReqwestFetcher`.
`UntrustedText`/`llm_safe()` must remain the only path for fetched text.

**9. Prior attempts.** §11 D192/D193 addressed the journal layer's fetch
reachability (6 of 20 became 13); the citation path was not part of that.

### A9 — Other lines a Q1 reviewer would call wrong or misleading

Read as a reviewer, run 30 contains these further defects `[ran]`:

1. **"required section: Results — Results section missing"** — false (A2).
2. **Four human-subjects rows reported as compliance failures** (A3).
3. **"AiDetection: concern"** presented beside a disclaimer, but the verdict
   itself is driven by the bibliography (A6).
4. **"lexical diversity deviates from the academic reference"** framed as a
   finding when it is a comparison to an unsourced global constant (A7).
5. **Row 5, "word limit depends on article type"** — detail reads
   *"the journal states 6 word limits — 4000 (Perspective); 4000 (Article);
   4000 (type not stated); 2000 (Brief Com…"*. "4000 (type not stated)" is a
   requirement the extractor could not attribute, printed as though it were a
   distinct article type. A reader cannot act on it.
6. **"18 of 24 dated reference(s) are older than 10 years"** on a paper whose
   field's foundational work is old (A7).
7. **The verdict is `concern` with `combined_confidence: 1.0`** — confidence 1.0
   is carried by the Tier-0 statistical findings, two of which are A4's
   candidates for misclassification. A reviewer reading "1.0" would take the
   verdict as certain.

**Correct and useful in run 30, for contrast:** row 6, *"abstract has 202 words
against a limit of 150"*, with the journal's own span quoted. This is the shape
the rest should take — a specific, checkable, sourced claim.

---

## B. REVIEWER LETTER

### B1 — "Reviewer unavailable, unavailable offline" in the app

**1. Symptom `[read]`.** `ReviewerEvaluation::unavailable_offline()` sets
`body: "deep reasoning requires cloud analysis — unavailable offline"` and
`warnings: ["reviewer unavailable: cloud proxy not reachable"]`.
`ReviewerLetterPanel.tsx:74` renders the body at `data-testid="pr-unavailable"`.

**2. Reproduction.** Run 30 is the evidence that the cloud path was down at
08:20: its verification lane recorded *"service unavailable — no verdict
attempted"* `[ran]`. *I did not launch the desktop app this session, so the
screen text is `[read]`, not `[ran]`.*

**3. Mechanism `[read]`, every gate in order — `src/commands.rs:1004-1027`:**

| # | gate | failure ⇒ |
|---|---|---|
| 1 | `ProxyReqwestClient::from_env()` — URL from `GAPLY_PROXY_URL`, **signing key from the macOS keychain** (`proxy_client.rs:198` → `TokenSigner::from_keychain`, item `app_check_signing_key`, service `ai.gaply.app`) | `_ =>` arm |
| 2 | `client.reachable()` — GET `/health` | `_ =>` arm |
| 3 | `verify_with_envelope(&proxy_payload)` — App Check header, rate limit, structured validator, provider | `Err` arm |
| 4 | `gate_reviewer_response(&resp, &sent_ids)` — schema + grounding gate | `Err` arm |

Until `be77cd8` all four produced the same sentence, of which only gate 2's is
true. `1a07b40` additionally removed a fifth de-facto gate: a missing or
out-of-range `publication_probability` used to fail gate 4 outright.

**4. Root cause class.** Wiring/configuration.

**5. Measured vs unknown.**
*Measured:* the gate sequence and that all four collapsed into one message
`[read]`; that the proxy answers `/health` when running `[ran]`.
*Unknown, and this is the actionable gap:* **which gate fails on this machine.**
The proxy is started from a shell with a dev key (`grrb-local-devkey` earlier
this session), while the app reads its key from the **keychain** — if those
differ, gate 3 fails with 401 and the user is told "not reachable".
**Experiment:** run the pipeline through `run_publishready_measured` with the
proxy up and print which gate returned first; then compare the keychain item
against the proxy's `APP_CHECK_SIGNING_KEY`. Neither needs the OpenAI key.

**6. Blast radius.** Every cloud lane: verification (A8), the reviewer letter,
escalation, gap-finder, chat, and the Box-4 shadow synthesis.

**7. Acceptance test.** With the proxy up and keys matching, a letter is
produced; with each gate broken in turn, the screen names *that* gate.
**Negative control:** with the proxy genuinely down, the message must still be
"not reachable" — it is the one case where the old wording was right.

**8. Constraints.** `GAPLY_SKIP_KEYCHAIN` and `cfg!(test)` deliberately force the
no-key path because `get_password()` can block on an interactive ACL panel
(`proxy_client.rs:160-197`). Any diagnostic must not make the app prompt.

**9. Prior attempts.** `be77cd8` categorised proxy-side failures;
`f48514c` removed a fence-induced false failure at gate 4. Neither identifies
which gate fails on a given machine.

### B2 — Four reviewer lenses with no app caller

**1. Symptom `[read]`.** `gaply-core/src/review_lens.rs:1521 lenses()` returns
four `ReviewLens` values (`LensId::Methodology` and three siblings). The
codebase documents its own gap at `src/pipeline.rs:326`: *"Its only consumer in
the tree is `review_lens::collect`, and `review_lens` is referenced nowhere in
the app crate or the frontend."*

**2. Reproduction `[ran]`.** `grep -rln review_lens src/ ../src/` returns only
`src/pipeline.rs`, and the hit is that comment.

**3. Mechanism `[read]`.** Each lens carries `criteria` and an `EvidencePolicy`
requiring `min_sources: 1`, `citation_required: true`,
`uncertainty_required: true`, over `ManuscriptSpan | JournalRequirement |
ExternalWork`. So the lenses are *designed* to emit cited, uncertainty-qualified
judgements — the shape a reviewer letter needs — and nothing calls them.
*Read only: I did not execute `review_lens::review`.*

**4. Root cause class.** Wiring.

**5. Measured vs unknown.**
*Measured:* the absence of a caller `[ran]`, the policy `[read]`.
*Unknown:* whether the lenses produce useful output on a real manuscript, and
what `ManuscriptSpan` evidence they would require — which collides with D1, since
spans are exactly what the proxy boundary limits. **Experiment:** run
`examples/lens_review_probe.rs` over the 6-manuscript corpus and count, per lens,
how many criteria receive a grounded judgement versus decline.

**6. Blast radius.** The reviewer letter's quality ceiling; C2's question of which
agents could run today.

**7. Acceptance test.** For a real manuscript, each lens either emits a judgement
with a citation and an uncertainty, or declines with a reason — and the letter
shows what it produced. **Negative control:** a lens with no admissible evidence
must decline rather than emit an uncited judgement.

**8. Constraints.** `EvidencePolicy` is the gate; §11 D166/D167's pattern — a lane
that cannot ground its claims is declined rather than shipped thin.

**9. Prior attempts.** None recorded; this is built-and-unwired, not tried-and-failed.

### B3 — The letter sees 8 findings and a summary, not the manuscript

**1. Symptom.** Run 30 produced 9 findings; `build_review_payload` caps at
`MAX_FINDINGS = 12` and `MAX_CHECKLIST = 20` `[read]`.

**2. Reproduction `[ran]`.** Earlier this session, on R PAPER: the reviewer
payload was **3841 bytes** carrying **8 findings**, and
`summary.overall_verdict = "concern"`.

**3. Mechanism `[read]`.** `reviewer_agent.rs:542 build_review_payload` sends, per
finding: `id, agent, tier, severity, title, confidence, evidence` — where
`evidence` is **structured provenance only** and the comment at the `title` line
reads *"title only — `detail` is deliberately never read (privacy)"*. Checklist
rows carry `requirement` + `passed`. The proxy then forwards **only `summary` and
`instruction`** (`openai_client.py`, `claude_client.py:54`) `[ran]`.

**4. What an expert reviewer uses that the letter never receives.** Stated
precisely, because this is the research question:

| a reviewer reads | the letter gets |
|---|---|
| the full text, in order | 8 finding titles |
| the actual claims and their hedging | nothing |
| figures, tables, captions | nothing |
| the equations and whether they follow | nothing |
| the reference list and whether it supports the claims | a count |
| the relationship between Methods and Results | a verdict string |
| what the field already knows | nothing (novelty declined, §11 D166) |
| the data, where available | nothing |
| prior work by these authors | nothing |

**5. Root cause class.** Privacy boundary **and** missing input — they are the
same constraint seen from two sides.

**6. Measured vs unknown.**
*Measured:* the payload's exact contents and size `[ran]`; the forwarder's
projection `[ran]`.
*Unknown:* how much of the gap is *necessary*. **Experiment:** hold the
boundary fixed and measure how far letter quality moves as the payload is
enriched **within** it — e.g. adding section-level structure, claim spans already
admissible under the 2000-char field limit, and the equation graph. That
measures the boundary's real cost rather than assuming it.

**7. Blast radius.** The entire premium proposition; C1 and C2.

**8. Acceptance test.** Two manuscripts with *different* substantive problems
must produce letters that differ in their substance, not only in their finding
titles. **Negative control:** a manuscript with the same 8 findings but sound
underlying science must not receive the same letter as one with unsound science —
if it does, the letter is a rendering of the findings and not a review.

**9. Constraints.** §11 D1's validator boundary (below); the payload must never
carry `detail`; `llm_safe()` governs any untrusted text.

**10. Prior attempts.** §11 D205/D207 tested whether *document context* changes
model behaviour on a related task: it does not (D207). That is evidence the gap
is not closed by adding local context alone.

### B4 — Temperature unset; `publication_probability` wanders 40–60

**1. Symptom `[doc]`/`[ran]`.** §11 D206: across five runs on one manuscript the
value came back **40, 40, 50, 50, 40**; the pre-fix baseline gave 40, 40, 40, 60.

**2. Reproduction `[ran]`, §11 D206's baseline.** Recommendation
`MajorRevision` 5/5; concern set `f1`–`f5` identical 5/5; order varied in 2 of 5.

**3. Mechanism `[ran]`.** `gaply-proxy/app/openai_client.py` builds
`{model, max_tokens, messages}` with **no `temperature`**, so the provider default
of 1.0 applies — confirmed by sentinel capture through the real client.

**4. Confirmation that nothing user-visible depends on it `[ran]`, all surfaces:**

| surface | result |
|---|---|
| `ReviewerLetterPanel.tsx:111` | explicit **NO GAUGE**, citing ONTOLOGY §4.20 |
| any other React component | none reads `publicationProbability` |
| `synthesize.ts:78` writes "publication probability: X%" into prose | reached **only** from `makePublishReadyMock` |
| `report_pdf` / `report_html` / `report_compose` / `report_model` / `exports` / `audit_export` / `audit_report` | **0 occurrences of "probability"** |
| letter `body` prose, 5 real runs | `pct_in_body` false 5/5, `prob_word_in_body` false 5/5 |

**5. Root cause class.** Configuration.

**6. Measured vs unknown.** Measured. *Unknown:* whether a different manuscript
whose findings sit near a verdict boundary would flip the recommendation — the
five-run stability was observed on one manuscript with a `concern` verdict.
**Experiment:** repeat the five-run protocol on a manuscript whose finding mix
sits between two bands.

**7. Blast radius.** Every cloud lane shares `ProxyClient`, so the same
non-determinism applies to verification, escalation, chat and gap-finder — none
of which has been measured for stability.

**8. Acceptance test.** Recommendation and concern set stable across N runs on
the same payload. **Negative control:** setting a temperature must not degrade
letter quality on the same payload — quality must be measured, not assumed.

**9. Constraints.** §11 D204's OPEN item: setting a temperature invalidates every
cloud number in §11 D196/D197/D204/D207. `claude_client.py` omits temperature
deliberately (Sonnet 5 adaptive thinking) — a different case.

**10. Prior attempts.** §11 D206 measured before fixing, deliberately, and the
measurement **overturned** the expectation that temperature was the defect.

---

## C. THE AGENT / DEEP-EVALUATION GAP

### C1 — The scientific layer is below no-skill at every model tier

**1. The task, stated exactly `[doc]`, from the benchmark's own ground-truth
note:** *"YES iff the paragraph states something the authors DID in this study (a
procedure, instrument, sampling scheme, design choice, dataset, or analysis
performed). NO for headings, captions, bare formulae, table cells, titles,
keywords, author statements, page furniture, background about the field,
commentary on interpretation, and other people's work."*

**2. The input each model received `[doc]`/`[ran]`:**

| tier | input | precision |
|---|---|---|
| stock Qwen2.5-0.5B (D196) | paragraph alone | 12.9 / 16.5% |
| gpt-4o-mini (D197) | paragraph alone | 37.3 / 35.7% |
| gpt-4o (D204) | paragraph alone | 33.3 / 38.5% |
| gpt-4o + framing (D205, withdrawn) | paragraph alone (context **dropped**) | 42.7 / 49.7% |
| **gpt-4o + real context (D207)** | paragraph **+ heading + both neighbours** | **42.9 / 48.0%** |
| no-skill: first paragraph of each Methods section | position only | **50.0%** |

**3. The common failure signature `[doc]`.** *Every* model that can answer has
**100% recall, positive separation, low precision** — they accept every genuine
paragraph and far too much else. §11 D197: *"separating the classes is not the
same as being right about them."*

**4. What the task is actually asking that no paragraph-level input can answer.**
The label depends on the paragraph's **role in the document's argument** — whether
this sentence is the authors reporting their own procedure, or restating someone
else's, or describing what the field does. That is a discourse-level relation,
not a property of the text in the paragraph. D204 named it as position; **D207
measured position and it changed nothing (+0.3 / −1.7)**, so position is
necessary-at-most and demonstrably not sufficient. The open question is in the
final section.

**5. Root cause class.** Missing input — but the missing input is *not* position,
which is the finding D207 contributes.

**6. Measured vs unknown.**
*Measured:* five conditions, three model tiers, four orders of magnitude of
parameters, all below a one-line heuristic.
*Unknown:* whether the ceiling is the task or the labels. 141 of 160 labels have
a single author (§11 D198 added a second only to the 19 disputed).
**Experiment:** double-adjudicate a random 40 of the 141 and measure
inter-annotator agreement. If human agreement is itself ~70%, the 50% ceiling is
partly definitional and the task needs restating before any method is tried.

**7. Blast radius.** Every specialist that would consume method objects (C2);
§11 D165's decline; the novelty lane (C5); and B3, since a letter cannot discuss
what was done if the layer cannot identify it.

**8. Acceptance test.** The combined rule clears 50% precision on **manuscripts
never used by D196–D207**, with two-author labels, with the rule fixed in
advance. **Negative control:** it must not be tuned on the existing 160 — that set
has now been used six times.

**9. Constraints.** §11 D205's reopening condition is binding and explicit.
CLAUDE.md: *"a prior that has missed by 30–50 points at every tier of a task is
not evidence about the next tier"* — **and I broke that rule again in D207,
predicting 45–57% and 52–64% and measuring 42.9% and 48.0%. Four over-estimates,
all the same direction, all assuming the model would exploit available
information.**

**10. Prior attempts.** D196, D197, D204, D205 (withdrawn), D207. Five measured
conditions; none cleared the line.

### C2 — Which intended agents depend on C1

`agent_graph.json` ships **9 agents** `[ran]`:

```
extraction              ingestion                deterministic
validation_maths        methodological_soundness deterministic
ai_detection            integrity                local perplexity
plagiarism              integrity                deterministic
rag                     journal_fit              rule
verification            integrity                cloud
frequentist_stats       methodological_soundness rule
ml_methodology          methodological_soundness rule
claim_evidence_strength methodological_soundness evidence_reasoning
```

**Depend on C1 (need "what the authors did" as an object):** `ml_methodology`,
`claim_evidence_strength`, and any future design/reproducibility agent. Evidence:
§11 D177 records `ml_methodology` running on 4 of 20 manuscripts and
`claim_evidence_strength` on 1 of 20 — both gated by applicability they cannot
establish `[doc]`.

**Do not depend on C1 — run on inputs that already exist:** `extraction`,
`validation_maths` (Tier 0), `plagiarism`, `rag`, `frequentist_stats`,
`verification` (needs connectivity, not C1), `ai_detection` (needs A6's fix, not C1).

**Could run today on existing inputs but do not reach a user:** the four
review lenses (B2) — they consume spans and journal requirements, both of which
exist. **This is the largest immediately-available capability in the tree.**

*Unknown:* the "40–50 intended agents" figure is from the premium architecture
document; I did not enumerate it against the graph. **Experiment:** diff the
architecture's agent list against `agent_graph.json` and classify each as
shipped / declined-with-D-number / unbuilt.

### C3 — agent_graph.json: design vs existing vs declined

*Measured `[ran]`:* 9 nodes, listed above. *Read `[read]`:* clusters are
`ingestion`, `integrity`, `methodological_soundness`, `journal_fit`.

*Declined with D-numbers `[doc]`:* the **scientific layer** (§11 D165, evidence
now five conditions deep through D207); **novelty** (§11 D166, 12 candidate
sentences in 20 manuscripts, 2 real); **table totals** (§11 D167, 2 checkable
tables in 20 manuscripts, all four failures the checker's); **AI-Check three-way
lane** (§11 D203, UNEVALUABLE — the prompt presupposes the verdict).

*Unknown:* the v4 design's full node list. Same experiment as C2.

**Note on the record itself `[ran]`: there is no D202.** The sequence runs
D201 → D203. Either an entry was withdrawn without a tombstone or the number was
skipped. For a log whose whole discipline is citable provenance, a silent gap is
the dangling-pointer defect `tests/decision_records.rs` exists to catch.

### C4 — SLM-1 and SLM-2

**SLM-1 `[doc]`.** §11 D199: it exists, is a **LoRA adapter over Qwen2.5-7B**, had
never been measured, and **cannot be run by the product** as shipped. §11 D200: it
is an **AI-text detector**, and **its adapter does not beat its own base**.

**SLM-2 `[doc]`.** §11 D201: measured on its own task — **the base carries no
signal**, the adapter is **unusable on the shipped path**, and **the prompt
presupposes its verdict**. §11 D203: correcting the prompt makes it worse.

**Training provenance — what is undocumented.** This is the dossier's answer to
"include training provenance": **the records state what the adapters *are* and how
they *score*, and do not state what they were trained on, against what objective,
with what held-out split, or by whom.** §11 D201's own wording — *"no heldout
evaluation named"* — appears as a finding code. **Nothing in the repository
documents the training corpus or objective for either adapter.**

*Unknown, and it is the blocking unknown:* whether either adapter was trained on
data overlapping the evaluation sets. **Experiment:** recover the training
manifest from wherever the adapters were produced; if it cannot be recovered,
treat both as unprovenanced artefacts and measure contamination directly by
scoring them on manuscripts with known post-training dates.

**Acceptance test.** An adapter beats its own base on a held-out set whose
provenance is documented. **Negative control:** it must not beat the base on a set
drawn from its own training distribution — that result would be uninformative.

### C5 — Novelty: 0.1 claims per paper

**Symptom `[doc]`.** §11 D166: **12 candidate sentences across 20 manuscripts, 2
of them real.** The lane is DECLINED.

**Detection problem or corpus property?** The record supports *both* readings and
does not separate them: 12 candidates in 20 papers is a low **detector** yield,
and 2 real in 20 is a low **base rate**. Papers do routinely state novelty
("to our knowledge, the first…"), so a 0.1/paper true rate is surprising and
suggests detection, not scarcity.

*Unknown:* the true base rate. **Experiment:** hand-annotate novelty claims in 20
manuscripts without running the detector, then compare counts. If humans find
~1 per paper, it is detection; if they find ~0.1, it is the corpus.

**Constraint.** §11 D166 declined the lane on 20 manuscripts of evidence, and
CLAUDE.md records that the *screen* showed `—` for it, which reads as "Gaply
looked and found nothing".

### C6 — Analysis upload: **the stated premise is inverted**

**The problem as posed was "only SPSS parses". The measurement says the
opposite `[read]`, `src/supplementary.rs:105-110`:**

```
supported: xlsx, xls, xlsb, ods, xlsm, csv, tsv, txt, text, md, log
DEFERRED:  SPSS (.sav) and MATLAB (.mat)          — supplementary.rs:28
```

**So SPSS is one of the two formats that do NOT parse.**

**Root cause class.** Extraction.

*Unknown:* what fraction of real users' analysis files would parse. **Experiment:**
this is the one question here that cannot be answered from the repository — it
needs a sample of what researchers actually upload. A proxy: the format
distribution in a public repository of supplementary statistical files (Dryad,
OSF) stratified by field. Note that SPSS `.sav` is dominant in psychology,
education and much of medicine, so the deferral is likely to bite hardest in
exactly the fields Nature Medicine serves.

**Acceptance test.** A `.sav` file's variables and values are read, and its
statistics cross-check against the manuscript. **Negative control:** a corrupt or
password-protected file must decline with a reason, not produce empty rows.

---

## D. INFRASTRUCTURE AND TRUST

### D1 — What the boundary allows, and what a whole-paper reviewer needs

**Measured `[ran]`, `gaply-proxy/app/validation.py`:**

```
MAX_TOTAL_CHARS          8000     across every string leaf of the payload
MAX_FIELD_CHARS          2000     per field
MAX_SENTENCES_PER_FIELD     8     when a field exceeds 400 chars
```

Plus the forwarder: **only `summary` and `instruction` reach the provider**
(`openai_client.py`, `claude_client.py:54`) `[ran]`.

R PAPER is **21,064 characters** of extracted text `[ran]`. The boundary permits
**38%** of one short paper, and that budget must also carry the instruction.

**What a whole-paper reviewer would require:** the full text (here 21k chars,
commonly 40–60k), plus figures and tables. That is **5–30× the total budget** and
**10–30× the per-field cap**.

**Root cause class.** Privacy boundary.

*Measured vs unknown:* the limits and the gap are measured. *Unknown:* whether
useful review is possible **within** the boundary — see B3's experiment, which is
the same one.

**Acceptance test.** Whatever is sent, the validator refuses raw manuscript prose
and the refusal is visible. **Negative control:** a payload that merely nests
prose deeper must still be refused — `_string_leaves` recurses, and a fix that
evaded it by restructuring would be a boundary regression.

**Constraint.** This is the architecture's central promise ("the manuscript stays
on the device"). Any proposal that relaxes it is a product decision, not an
engineering one.

### D2 — commands.rs error wiring untested

**Measured `[ran]`.** `be77cd8` categorised proxy failures and
`unavailable_reason_for` / `unavailable(reason)` are pinned by tests in
`gaply_core`. **That they are *called* at the three collapse points in
`commands.rs:1004-1027` is guarded by nothing** — it needs a full pipeline run.

**Root cause class.** Measurement/instrument.

**Acceptance test.** With each gate failed in turn, the letter body carries that
gate's sentence. **Negative control:** a test that constructs the letter directly
does not satisfy this — it must go through the command path.

### D3 — User-facing behaviour guarded only locally

**Measured `[ran]`** by running CI's exact command and grepping for test names:
both workflows run `cargo test -p gaply_core`; the only app-crate line is
`cargo test -p app --test commands_test --no-run`, which compiles one unrelated
target without executing it.

**Guarded only by a developer's `cargo test --workspace`:**

| behaviour | test | crate |
|---|---|---|
| the fence stripper is actually *called* on the response path | `a_reply_wrapped_in_a_markdown_fence_is_still_read` | app |
| the golden report stays byte-identical | `the_report_is_byte_identical_to_the_pre_research_state_capture` | app |
| D-number citations resolve | `tests/decision_records.rs` | app |
| the reviewer error wiring (D2) | *none* | — |

**Root cause class.** Measurement/instrument.

*Note:* `f114830` moved the fence stripper's **behaviour** into `gaply_core` so it
runs on every push on Linux and Windows; its **wiring** remains app-crate.

### D4 — `npm run tauri build` fails at `bundle_dmg.sh`; cause **unknown**

> **STATUS, superseding the heading: the failing STEP is now known and the
> TRIGGER is not — see §11 D209 of `docs/AI_ENGINE_PLAN.md`.** The step is
> `hdiutil_detach_retry` (exit 16, EBUSY, ~6s of patience); the DMG builds and
> passes every acceptance check; it failed once and has since succeeded five
> times unmodified. Everything below is the original record, kept as written.

**Measured `[ran]`.** Three consecutive builds, `BUILD_EXIT=1`, identical:

```
Bundling gaply_0.1.0_aarch64.dmg
 Running bundle_dmg.sh
failed to bundle project: error running bundle_dmg.sh
```

**Two hypotheses tested and both REFUTED `[ran]`:** stale mounted volumes
(detached all three — failed identically) and leaked 593 MB `rw.*.dmg` scratch
images in the source folder (removed — failed identically).

**The script itself is not broken `[ran]`:** run directly with tauri's documented
arguments it exits **0** in 6.5 s and produces the DMG. `hdiutil` works
(test image created). Disk had 7.1 GiB free. The script ends `exit 0`, so the
`internet-enable` warning is not the cause.

**Real stderr: NOT CAPTURED.** `tauri` discards the script's output, and the
script is regenerated on every build so a logging wrapper is overwritten.
`tauri-bundler` is not available as local source (the npm CLI ships a prebuilt
binary), so the exact arguments cannot be read. **Experiment:** run the build
under `fs_usage`/`dtruss` filtered to the bundler pid, or interpose a `PATH` shim
for `hdiutil` that logs its argv, and diff those arguments against the working
manual invocation.

**Root cause class.** Configuration (provisional — the class itself is unknown).

### D5 — DMG ad-hoc signed, not notarized

**Measured `[ran]`** on the artefact built at `abcb87d`:

```
path    src-tauri/target/release/bundle/dmg/gaply_0.1.0_aarch64.dmg
size    493,555,255 bytes   sha256 8c272b3a6934b55e...
DMG     code object is not signed at all      spctl: rejected
.app    Signature=adhoc, TeamIdentifier=not set, flags=0x10002(adhoc,runtime)
        codesign --verify: satisfies its Designated Requirement
        spctl: rejected
build log: "skipping app notarization, no APPLE_ID & APPLE_PASSWORD & APPLE_TEAM_ID…"
```

`signingIdentity` is present in `tauri.conf.json` but the build signed with `-`.
**The artefact runs on this machine and is refused by Gatekeeper anywhere else.**

**Root cause class.** Configuration.

**Acceptance test.** `spctl -a -t open` accepts the DMG on a machine that has
never seen the source tree. **Negative control:** an unsigned build must still be
*refused* — a fix that disables Gatekeeper locally proves nothing.

### D6 — the DMG wrapper is not byte-reproducible; the app inside it is

**Measured `[ran]`** across three consecutive unmodified `npm run tauri build`
runs at `bf583eb`:

```
DMG                      492,946,212  /  492,946,204  /  492,946,199 bytes
Contents/MacOS/app       80a10058e12ae57c369055ded83cc45d450aa011c194c8c0fe192a82d99e7fb6
                         identical in all three
```

**The product is reproducible and its packaging is not.** The variation is in the
image wrapper — HFS+ timestamps, allocation and compression layout — not in
anything a user runs. So a sha256 of the DMG identifies a BUILD, and only the
inner binary's hash identifies the SOFTWARE. Quoting a DMG hash as though it
pinned the product is the mistake this entry exists to prevent.

**Why it is recorded separately from §11 D209** and not folded into it: D209 is
an intermittent failure whose trigger is unknown, and this is a deterministic
property measured three times out of three. Mixing a characterized-but-unexplained
failure with a fully-measured one would let the weaker evidence borrow the
stronger entry's confidence.

**Root cause class.** Reproducible build (not investigated).

**Acceptance test.** Two builds from the same commit, on the same machine, with
no source change, produce byte-identical DMGs. **Negative control:** a build with
a one-character source change must still differ — a "fix" that makes every DMG
identical regardless of input proves nothing.

---

---

## Questions the solution must answer

For the four research-level problems. These are stated as questions a published
method would have to address, not as tasks.

### C1 — "What did these authors do?"

1. **Is the task well-posed?** Two readers agreed on 17 of 19 disputed paragraphs
   (§11 D198), but 141 of 160 labels have one author. *What is human
   inter-annotator agreement on this label, and does it bound the achievable
   precision below the 50% no-skill line?* If humans agree at ~70%, no method
   clears 50% precision *as the task is currently defined*, and the contribution
   would be a better task definition.
2. **What unit is the label actually about?** Every result has 100% recall and low
   precision at paragraph granularity, and restoring document position changed
   nothing (§11 D207). *Is the decidable unit the sentence, the clause, or a
   discourse relation spanning paragraphs?*
3. **What evidence distinguishes "what these authors did" from "what is done in
   this field"?** Both are procedure descriptions in the past tense. *Is the
   distinguishing signal tense/voice, citation adjacency, or section discourse
   role — and is it present in the text at all?*
4. **Why does scale not help?** Four orders of magnitude of parameters span
   12.9%–48.0%. *Is this a knowledge problem (which models have) or a grounding
   problem (which they do not)?*

### C4 — Adapters with no provenance

1. *What were SLM-1 and SLM-2 trained on, against what objective, with what
   held-out split?* Nothing in the repository answers this.
2. *Is the evaluation set contaminated by the training set?* Until answered,
   neither a positive nor a negative result is interpretable.
3. *Under what conditions should a LoRA adapter beat its base on a
   classification task?* §11 D200 measured it not doing so; the useful question
   is whether the adapter was trained for a different objective than the one it
   is being scored on.

### B3 — Review without the text

1. *What is the smallest representation of a manuscript sufficient for a review
   that a domain expert would call substantive?* Measured within the existing
   8000-char boundary, this is answerable experimentally and nobody has asked it.
2. *Which reviewer judgements are derivable from structured findings alone, and
   which provably are not?* A letter that only re-renders findings is honest;
   one that implies more is not.
3. *Can a local model do the reading and send only structured conclusions?* This
   is the architecture's implicit premise and is untested — §11 D200/D201 measured
   the available local models on other tasks and both failed.

### A3 — Applicability

1. *How is "this requirement does not apply to this manuscript" established from
   the manuscript itself?* Study-design classification is the obvious route and
   depends on C1.
2. *Is the condition already in the journal's text?* Row 10's own span reads
   *"informed consent was obtained from all recipients and/or donors of human
   material"* — the condition is present and unused. *What fraction of
   conditional requirements state their condition extractably?* This is
   measurable today on the 10 profiled journals and is the cheapest question in
   this dossier.
3. *What should a checklist say about a requirement it cannot evaluate?* The
   three-state contract exists (`passed` / `unevaluable`); the open question is
   who decides, and on what evidence.
