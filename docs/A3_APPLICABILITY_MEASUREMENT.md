# A3 applicability — measurement, 22 Sep 2026

**Measurement only. Nothing was implemented, no checklist behaviour changed, no
model was added.** The question: for the requirements in the bundled seed, how
often can an applicability condition be extracted deterministically AND
evaluated as MET / NOT_MET / UNKNOWN from what `ResearchState` already holds,
with no LLM.

Every count below was predicted before it was measured. The predictions are in
the table; three of them were wrong in ways that changed the conclusion.

## Prediction vs result

| # | predicted | measured | verdict |
|---|---|---|---|
| P1 | 60 of 213 rows state a condition | **52** (40 unique spans) | close |
| P1a | reporting_standard 45/70 | **44/70** | right |
| P1b | data_policy 5/17 | **0/17** — all its conditionals are content, not scope | **wrong** |
| P1c | limit kinds 5/75 | **0/75** — they scope by article type, in metadata | **wrong** |
| P3 | 42 of 60 recoverable by rule | **25 of 52** by the blind rule (48%) | **wrong, optimistic** |
| P5 | 8–12 false positives | **8** | right |
| P6 | 0 of 6 signals exist as reliable booleans | **0 of 6** | right |
| P8 | unknown rate ≥ 60% of conditional rows | **100%** of study-scope rows | right |
| P9 | 4 R PAPER rows should change (8, 10, 11, 13) | **4 rows — but {6, 8, 10, 13}** | right count, wrong set |

P3 is the one that matters. A rule written *before* reading the corpus caught
under half of what is there, and a rule written *after* reading it caught 81% —
so the 48% is the honest estimate of writing this rule blind, and the 81% is
fitted and should not be quoted as a forecast.

---

## PART 1 — EXTRACTION, over all 213 seed requirement rows

213 rows carry only **135 unique spans**, so every count is given in rows with
the unique-span count beside it. All 133 distinct spans were read by hand; the
labels below are that reading, not the rule's output.

### 1. How many rows state an applicability condition

| | rows | unique spans |
|---|---:|---:|
| **A — study-scope condition stated in the span** (design / population / topic / custom code) | **52** | 40 |
| A− weak or implicit (object implies it, states no condition) | 3 | 3 |
| **N — conditional language that is NOT applicability** | 17 | 16 |
| B — scoped by article type, carried in the `article_type` field | 80 | — |
| noise / mis-scoped rows found while reading | 7 | 7 |

**By kind.** The condition lives almost entirely in one kind:

| kind | A rows / total |
|---|---|
| reporting_standard | **44 / 70** |
| section_required | **8 / 34** |
| data_policy | 0 / 17 |
| figure_limit, word_limit, abstract_limit, reference_limit, reference_style | 0 / 92 |

**By journal:** frontiers-public-health 13, plos-one 12, nature-medicine 10,
plos-medicine 9, bmj 6, j-health-psychology 1, statistics-in-medicine 1,
bmc-public-health 0, lancet 0.

The shape of the finding: **a reporting-standard sentence almost always names
the design it applies to, and a statement requirement usually does not.** That
is the opposite of where A3 hurts — the rows that produce R PAPER's false
failures are `section_required`, the kind least likely to carry its condition.

### 2. Classification of the 52

| class | rows | example |
|---|---:|---|
| study design | 38 | "For observational studies, use the STROBE checklist" |
| population (human / animal) | 9 | "If the study made use of human or animal subjects and/or tissue" |
| topic / method | 3 | "Studies reporting biomarkers in association with clinical outcomes" |
| custom code | 1 | "If custom code was used in the study" |
| article type (stated in the span) | 1 | "Clinical Trial articles should have the following format" |

Article-type scoping is real but lives in the `article_type` **field** on 80
rows, not in the sentence. That distinction decides the fix: a field needs no
extraction at all.

### 3. How many are recoverable by a deterministic rule

Two rules, both pure regex over the stored sentence:

| | fired | true positive | false positive | missed | precision | recall |
|---|---:|---:|---:|---:|---:|---:|
| **v1, written blind** | 33 | 25 | 8 | 27 | 76% | **48%** |
| v2, written after reading the corpus | 45 | 42 | 3 | 10 | 93% | 81% |

v2 requires two halves — a scope frame (`For …`, `Studies reporting …`,
`If …`, `in the case of …`) governing a **study-class noun** — and suppresses
matches inside acronym expansions. Its 81% is fitted to the same 133 spans it
was written from and is an upper bound, not a forecast.

**Ten examples, easiest first, with the last three being the hard ones:**

1. *"For observational studies , use the STROBE checklist and any appropriate extension STROBE extensions."* (bmj) — frame + class, trivially recoverable.
2. *"Systematic reviews and meta-analyses must follow the PRISMA guidelines."* (nature-medicine) — subject + modal.
3. *"Observational studies (cohort, case-control or cross-sectional designs) must be reported according to the STROBE statement ."* (nature-medicine) — parenthetical between subject and modal; v1 missed it, v2 allows up to four intervening words.
4. *"For studies of diagnostic accuracy , use the STARD checklist and flowchart."* (bmj) — "studies **of** X" rather than "X studies"; v1 missed it purely on word order.
5. *"If custom code was used in the study, a separate Code Availability Statement must also be provided."* (nature-medicine) — the cleanest conditional in the seed.
6. *"If the study made use of human or animal subjects and/or tissue, you must provide an ethics statement."* (plos-one) — two populations disjoined in one condition; this is the row that would fix R PAPER's ethics failure for PLOS.
7. *"For research involving human participants, informed consent must have been obtained or the reason for lack of consent explained…"* (plos-medicine) — condition plus an escape clause that is **not** a second condition.
8. **Hard.** *"Authors must provide sufficient information … in accordance with the ARRIVE (Animal Research Reporting of In Vivo Experiment) guidelines , the REFLECT statement for livestock reporting, and the SAGER guidelines ."* (statistics-in-medicine) — states **no** condition, and v1 fired on it because the *acronym expansion* contains "Animal Research". A population word inside a standard's own name is not a scope.
9. **Hard.** *"The ethics statement must also confirm that informed consent was obtained from all recipients and/or donors of cells or tissues, **where necessary**, and describe the conditions of donation of materials for research, such as human embryos or gametes."* (nature-medicine) — the condition is `where necessary`, floating mid-sentence, attached to nothing a parser can bind. **Both rules miss it**, and it is the row the dossier itself quotes as proof the condition is in the text.
10. **Hardest.** *"…all fast track submissions must include the following: Complete manuscript files, including disclosure of competing interests, funding statement, a data availability statement and **in the case of studies involved custom code**, a code availabiity statement…"* (nature-medicine) — one sentence, four requirements, one conditional clause that governs only the fourth. Three seed rows quote this span; the condition belongs to exactly one of them.

Example 10 is the general failure: **scope attachment**. Finding a condition in
a sentence is easy; deciding *which requirement it governs* is the part a regex
cannot do, and it is where 5 of the 8 v1 false positives came from.

### 4. Rows with no stated condition that clearly need one

Counting `section_required` and `data_policy` rows for ethics, consent or code
availability whose span states no condition: **7 rows** (bmj 1, nature-medicine
2, plos-medicine 1, plos-one 3). The
sharpest pair, both nature-medicine, both `code availability statement`:

- *"If custom code was used in the study, a separate Code Availability Statement must also be provided."* — conditional.
- *"Code availability statements should be provided as a separate section after the data availability statement but before the references."* — unconditional, and **this is the span R PAPER's row 12 was judged against.**

The same requirement, two spans, one carrying the condition and one not, and the
checklist picked the one without it. Likewise row 8's *"informed consent was
obtained from all human research participants"* presupposes human participants
without ever stating a condition.

### 5. Negative controls

**17 rows** carry conditional language that is not an applicability condition.
They fall into four kinds:

| kind | rows | example |
|---|---:|---|
| content of the statement | 6 | *"If code cannot be shared due to legal or ethical reasons then authors should state this in the Data Availability Statement"* |
| hedge | 4 | *"where possible"*, *"if necessary"*, *"for relevant journals"* |
| submission mechanics / process | 4 | *"If you are submitting a manuscript to a particular special issue…"* |
| alternative, not scope | 3 | *"If approval was not obtained, the authors must provide a detailed statement explaining why it was not needed"* |

**Result: the blind rule v1 wrongly treated 8 of them as applicability
conditions — every one of its false positives was an N row.** v2 cuts that to 2.
The residual two are the scope-attachment case (example 10), which no rule over
a single sentence can fix.

Two data-quality problems surfaced while reading, both live in the shipped seed
and both feeding the checklist:

- `plos-one` / PRISMA carries *"A PRISMA 2020-guided search was conducted in ScienceDirect, Web of Science…"* — **a manuscript's sentence**, not journal guidance. A crawler leak.
- `statistics-in-medicine` `word_limit = 250` is *"re-use … up to 250 words from their contributions without seeking permission"* — **a re-use licence quota stored as a submission limit**. This is exactly the §11 D163 class, still present.

---

## PART 2 — EVALUATION, over what the pipeline actually holds

### 6. The six signals

`ResearchState::science` is `None` on every production run: no shipped agent
declares `scientific_extraction` under `requires` (it appears only under
`optional`, for three agents), and a test pins it —
`a_real_pipeline_run_does_not_derive_the_declined_scientific_layer`
(`src-tauri/src/pipeline.rs:1433`). So the whole `StudyDesign` / `Method` /
`Dataset` layer is typed absence, not data (§11 D165).

| question | best signal that exists | live in production? | reliability |
|---|---|---|---|
| human participants | `SUBJECTS_PRESENT`, 31 verb-qualified phrases (`review_lens.rs:685`) | **no** — `review_lens` is reachable from no Tauri command | measured below: **missed a paper that had them** |
| animals | same lexicon | no | caught 1 of 2 |
| clinical trial | `StudyDesign::ClinicalTrial` (dead layer); `EXPERIMENTAL_MARKERS` (live, internal to one specialist) | partly | never reaches the checklist |
| registered trial | **nothing** — no NCT/ISRCTN/PROSPERO reader anywhere | no | absent |
| custom code | `Method::software` (dead); `ml::is_machine_learning_paper` (live) | the ML boolean is computed but the specialist is withheld | on R PAPER: **true**, 11 distinct terms |
| study design | three readers, all blocked from the checklist — `build_checklist` passes `bindings: &[]` (`report.rs:1801`) | no | — |
| article type | **nothing classifies the manuscript**; `article_type` is the journal's field only | no | absent |

The stored `ExtractionResult` for R PAPER confirms it from the artefact side:
six keys — `title, sections, statistics, citations, references, tables` — and no
population, design, ethics or article-type field of any kind.

**Reliability, measured on the four `.docx` corpus manuscripts** (the two PDFs
were not measured — no text extractor was run, and that gap is stated rather
than filled):

| manuscript | subjects lexicon | truth | verdict |
|---|---|---|---|
| R PAPER | 0 phrases | deep learning on social-media text, no subjects | correct |
| chapter3 | 1 (`were euthanised`) | zebrafish exposure experiment | correct |
| Revised Health Economics | **0 phrases** | *"Participant consent obtained prior to interview"* | **FALSE NEGATIVE** |
| Lake Chapter 1 | 0 phrases | *"The present study employed two model systems, a vertebrate and a plant"*, but this chapter runs no experiment | ambiguous |

The false negative is the whole result. The paper says *"Participant consent
obtained prior to interview"*; the lexicon looks for *"participants were"*,
*"participants gave"*, *"interviews were conducted"*. Singular noun, nominalised
verb, and it misses. **On the two unambiguous positives the lexicon scores 1 of
2.**

### 7. Per condition type: evaluable or not

| condition type | rows it governs | deterministically evaluable today |
|---|---:|---|
| study design | 38 | **no** — every design reader is dead, internal, or filtered out |
| population (human/animal) | 9 | **one direction only** — presence is evidence, absence is a lexicon miss |
| custom code | 1 | **one direction only** — `is_machine_learning_paper` is live but withheld |
| topic / method | 3 | no |
| article type (field) | 80 | **no** — nothing classifies the manuscript; UNKNOWN is the only honest state |

**Zero of five can be decided in both directions.** Two can only ever say
"applies"; none can say "does not apply" on evidence.

### 8. Projected rates

The repo's own rule decides this. `FieldReach::Exhaustive` vs `Mediated`
(`review_lens.rs:259`) says a full-text phrase search reaches every word, so its
absence is a fact about the manuscript — but the doc comment on that very enum
adds *"a miss is a lexicon miss, and the finding must say so."*

- **False-applicable** (claiming a requirement applies when it does not): low
  and harmless. The lexicon fires on a phrase that is present; the cost is a row
  that says NEEDS REVIEW instead of being hidden.
- **False-inapplicable** (hiding a requirement that does apply): **measured at 1
  of 2 positive cases** if absence were read as NOT_APPLICABLE. This is the
  dangerous direction and it is not a projection — the health-economics paper
  is a real manuscript in the corpus that would have had its consent row
  suppressed.
- **Unknown**: **100% of the 52 study-scope rows** if the standard is "decided
  on evidence". Nothing in production can answer the design question at all.

So an applicability layer built today cannot produce NOT_APPLICABLE on evidence.
It can only move rows from a false NOT MET to UNKNOWN.

### 9. R PAPER: which rows are wrong, read from the stored run

Run 30 (`report:v2:e3:30`, 22 Sep 08:20, the run the dossier quotes): 13 rows,
**10 FAIL**, and **not one row carries `unevaluable`** — the flag is absent from
every row, which is D188's latent third state.

| row | verdict now | journal's own condition | should be |
|---|---|---|---|
| 6 abstract limit 150 | FAIL "202 words against a limit of 150" | `article_type = Brief Communication`; R PAPER is not one | **NEEDS REVIEW** |
| 8 informed consent | FAIL | *"informed consent was obtained from all human research participants"* | **NEEDS REVIEW** |
| 10 ethics statement | FAIL | *"…recipients and/or donors of cells or tissues, where necessary … human embryos or gametes"* | **NEEDS REVIEW** |
| 13 author contributions | FAIL | `article_type = Matters Arising`; R PAPER is not one | **NEEDS REVIEW** |
| 7 data availability | FAIL | span is fast-track-scoped, but nature-medicine requires a DAS of *"all original research manuscripts"* elsewhere | stays NOT MET, wrong span |
| 11 competing interests | FAIL | same fast-track span; the requirement is unconditional elsewhere | **stays NOT MET** |
| 12 code availability | FAIL | *"If custom code was used"* — R PAPER is a deep-learning study, 11 ML terms | **stays NOT MET** |
| 9 funding | FAIL | *"Any relevant funding should be declared"* — hedge, not a scope | stays NOT MET |
| 3 Results missing | FAIL | — | A2, not applicability |

**Four rows change, and the dossier's set was not quite right.** A3 names rows
8, 10, 11, 13. Measured: row 11's span is mis-attached but its requirement is
genuinely unconditional, so it must keep failing; and **row 6 — which A3 does
not mention — is wrong for exactly the same reason as row 13.**

Row 6 against row 5 is the sharpest thing in this report. Both are limits scoped
by an article type the pipeline does not know. Row 5 says so, in the user's
words: *"which limit applies depends on the article type you are submitting,
**which this analysis does not know**"* — and passes. Row 6, one row later,
asserts a failure against a Brief Communication limit. **The honest behaviour is
already implemented, one branch away from the dishonest one.**

Row 12 is the negative control from A3's acceptance test, and it survives: the
condition is custom code, R PAPER used custom code, the row must stay NOT MET.
Any fix that hides row 12 is wrong.

---

## Conclusion: lexicon or semantics?

**A3 is two problems, and only the first is a lexicon problem.**

**Extraction (journal side) is largely deterministic.** 52 of 213 rows state a
condition; a blind regex reaches 48% of them, a corpus-informed one 81%, at 93%
precision. What defeats it is not meaning but **scope attachment** — which
requirement in a multi-requirement sentence a conditional clause governs — plus
unanchored hedges like *"where necessary"*. That is the residue, and it is
small.

**Evaluation (manuscript side) is not a lexicon problem, and it is not a
semantics problem either — it is a missing input.** There is no design
classifier, no article-type classifier, no trial-registration reader, and the
one relevant lexicon lives in code no Tauri command can reach. Adding an LLM
would not fix this: the measured failure was a paper whose consent sentence the
*lexicon* missed, and §11 D165/D207 already measured the model-based route to
study understanding at below the no-skill line. **The input does not exist, so
the only honest third state is UNKNOWN.**

## The smallest change that fixes R PAPER without hiding anything

Not implemented. Stated so it can be argued with before it is built.

**Use the `article_type` field the journal already gave us, and render the
third state that already exists.**

When a requirement carries `article_type = Some(t)` and nothing has established
the manuscript's article type — which is always, today — emit
`unevaluable: true` instead of `passed: false`, with the detail row 5 already
writes: *the journal states this for `t`; which type applies is not known here*.

That is one condition on one field, and it fixes **rows 6 and 13** outright. It
requires no new signal, no lexicon and no model, because the scope is metadata
the seed already carries on 80 rows. `ChecklistItem.unevaluable` exists
(`report.rs:280`), both renderers already draw "not decided"
(`report_compose.rs:591`), and **nothing in production sets it** — the four
sites that do are all unreachable. This would be its first producer.

Rows 8 and 10 need one more thing: a population condition extracted from the
span (examples 5–7 above are recoverable; example 9 is not). The rule is
`condition present AND no manuscript evidence either way → UNKNOWN`, never
NOT_APPLICABLE. Given the health-economics false negative, **absence of lexicon
evidence must never be allowed to conclude inapplicability** — that is the
mechanism by which a fix would hide a genuinely unmet requirement.

**Negative controls the change must pass**, both available today:

1. Row 12 (code availability, R PAPER) must stay NOT MET.
2. The health-economics manuscript must not have its consent row suppressed,
   despite the subject lexicon returning zero on it.

A third, from the dossier: on a clinical manuscript that genuinely omits a
consent statement, the row must still be NOT MET.

## Not claimed

- **Not that 52 is the true count of conditional requirements.** It is the count
  of rows whose stored span states a condition, over 9 journals in one snapshot.
- **Not that rule v2's 81% would hold.** It was written after reading the spans
  it scores; 48% is the blind figure.
- **Not measured on the two PDF manuscripts.** The subject-lexicon reliability
  figure rests on four `.docx` files, of which two carry unambiguous subjects.
- **Not that `is_machine_learning_paper` is available to the checklist.** It is
  computed, but the specialist carrying it is filtered out of the pipeline.
- **No threshold, weight or coefficient was introduced, and no product code was
  changed.**

Artefacts: the bundled seed `gaply-core/data/journal-seed.json`; stored run 30
in `~/Library/Application Support/ai.gaply.app/gaply.db`
(`cache` key `report:v2:e3:30`); the four `.docx` manuscripts named in §11 D165.
