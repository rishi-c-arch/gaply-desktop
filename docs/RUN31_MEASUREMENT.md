# Run 31 — three measurements, plus coverage and scope

**Report only. No fixes, no implementation, no production change.** Every number
below names the run or probe that produced it. Nothing from different runs is
combined.

Provenance tags: **STORED RUN** (read from the database), **LIVE PROBE** (a
command run now), **SOURCE INSPECTION** (code read, nothing executed),
**HAND-COUNTED** (counted by hand from the file's own bytes).

---

## A. Journal identity

### A.1 How run 31 selected its journal — STORED RUN

**By picker, not by URL.** Three independent facts in the live database:

| evidence | value | provenance |
|---|---|---|
| `journal_guidelines` table | **empty** — nothing was ever crawled into it | STORED RUN |
| `journal_fingerprints` for `nature-medicine` | `origin = bundled`, `source_count = 37`, `fetched_at = 1789728313` (the seed's timestamp) | STORED RUN |
| source URLs of run 31's 9 sourced checklist rows | all `www.nature.com/nm/*`, i.e. the bundled seed | STORED RUN |

No pasted URL took part in run 31. The `journal_key` came from the picker row
and the checklist was built from the bundled seed.

### A.2 Which identifier decides journal identity — SOURCE INSPECTION

**A slug of the display name, or a curated key from a ten-row JSON table.
Nothing else.** The whole of the resolution logic is
`src-tauri/src/guidelines.rs:268-274`:

```rust
pub fn resolve(&self) -> Option<String> {
    self.key
        .map(str::to_string)
        .or_else(|| self.name.map(key_for_named_journal))
        .filter(|k| !k.is_empty())
}
```

and `key_for_named_journal` (`guidelines.rs:219-232`) lowercases the name and
replaces every non-alphanumeric run with a dash.

**There is no URL→journal resolution anywhere in the product.** A pasted URL is
fetched and parsed; the only part of it ever inspected is the host, and that is
used solely as a rate-limit bucket (`guidelines.rs:637-640`, consumed at `:516`).

**ISSN and DOI are absent from this path.** `grep -riE '\bissn\b'` over
`guidelines.rs`, `journal_store.rs`, `journal_extract.rs` and
`journal_fingerprint.rs` returns **0**. ISSN handling exists only in the
separate Journal Verify lane (`journal_registry.rs`), which never produces a
`journal_key`. No `journal_*` table has an ISSN column
(`migrations.rs:927, 952, 971, 988, 1039`).

| candidate identifier | used to decide journal identity? |
|---|---|
| ISSN | **no** |
| DOI metadata | **no** |
| journal record id (curated key) | **yes**, when the picked row is one of the ten |
| exact URL | **no** — the URL is never compared to anything |
| title match | **yes** — a slug of the display name, the fallback branch |

### A.3 The binding test — LIVE PROBE

`https://link.springer.com/journal/11418` is the **Journal of Natural
Medicines**. Fed through the real path
(`examples/pasted_url_probe`, real `ReqwestFetcher`, in-memory database), with
the name the picker would carry for a user who selected Nature Medicine:

```
$ cargo run --example pasted_url_probe -- \
    "https://link.springer.com/journal/11418" "Nature Medicine"
  journal_key=Some("nature-medicine")        # <- the Springer URL resolved here
  requirements_stored=0                      # that page is navigation, not guidance
```

The homepage yields no requirements, so nothing was contaminated. **The
guidelines page does:**

```
$ cargo run --example pasted_url_probe -- \
    "https://link.springer.com/journal/11418/submission-guidelines" "Nature Medicine"
  journal_key=Some("nature-medicine")
  requirements_stored=1     origin=Some("crawled")
  rows readable through the real reader: 1
    [section_required] competing interests statement  (article_type=NotStated)
        url:  https://link.springer.com/journal/11418/submission-guidelines
        span: The above should be summarized in a statement and placed in a
              'Declarations' section before the reference list …
```

**A Journal of Natural Medicines requirement is now a Nature Medicine checklist
row, readable through the product's own reader.**

**Is a wrong-journal binding reachable? Yes, and by the ordinary path.** The user
picks a journal by name, pastes any guidelines URL, and whatever that page states
is stored under the picked journal's key. `store_requirements(db, k, url, …)`
(`guidelines.rs:604-613`) writes the key and the URL side by side and never
compares them.

**A publisher is not a journal, and this is worse than publisher inference.** No
inference occurs at all: the URL's domain is never consulted, so
`link.springer.com`, `nature.com/nbt` and `example.com/anything` all bind
equally to whatever name is in the picker. Two related collisions, both SOURCE
INSPECTION:

* **Name-slug collisions.** `"The BMJ"` → `the-bmj` against the stored `bmj`;
  `"The Lancet"` → `the-lancet` against `lancet` (noted at `guidelines.rs:242-249`).
* **Publisher hosts inside the crawler.** `journal-crawl.json:105-111`
  allowlists publisher-wide author-services hosts whose pages are stored under
  the single journal key being crawled. The config already records the measured
  damage: `authorservices.springernature.com` contributed **9 requirements to
  Nature Medicine, all nine false** — translation pricing stored as word limits
  — and was removed (`journal-crawl.json:24-29`, §11 D163).

**Provenance overwrite** — SOURCE INSPECTION: the crawled write is
`INSERT OR REPLACE INTO journal_fingerprints … VALUES (…, 'crawled')`
(`journal_store.rs:384-387`) on a table whose primary key is `journal_key`
(`migrations.rs:1039`), so a crawled ingest replaces the `bundled` provenance
row for that journal. Not exercised by the probe — its database held no seed —
so this is read, not run.

**Classification: reproduced, cause identified.** The cause is deterministic and
named: `JournalIdentity::resolve` derives the key from the picker's key or the
display name, and no code path compares the pasted URL to the journal it was
bound to.

---

## B. Table detection

### B.1 What R PAPER actually contains — HAND-COUNTED

Read directly from `word/document.xml` inside the `.docx`:

| | value | provenance |
|---|---|---|
| `<w:tbl>` objects | **5** | HAND-COUNTED |
| `<w:tr>` rows across them | 42 | HAND-COUNTED |
| representation | **real Word table objects** — not images, not tab-separated text (0 tab characters in the document text) | HAND-COUNTED |

The five, by header row: `Dataset / Source / Samples / Emotions / Avg. Length`
(3 rows), `Parameter / Value / Tuned By` (11), `Method / Accuracy (%) / F1-Score
(%)` (10), `Emotion / Precision (%) / Recall (%) / F1-Score (%)` (10),
`Configuration / Acc (%) / F1 (%) / MCC` (8). The prompt named four; there are
five — the extra is the comparative-performance table, captioned
**"TABLE II. COMPARATIVE PERFORMANCE ON SEMEVAL-2018 TEST SET"**.

**The cell text survives.** `"Tuned By"`, `"Avg. Length"`, `"MCC"`,
`"Hyperparameter"` are all present in run 31's stored extraction text
(STORED RUN). Only the structure is lost: `docparse` ignores `<w:tbl>`, `<w:tr>`
and `<w:tc>`, so each cell arrives as an ordinary paragraph.

### B.2 The six-manuscript measurement

| Manuscript | Ground truth | Extractor | Representation | Missed | Provenance |
|---|---:|---:|---|---:|---|
| R PAPER .docx | **5** | **0** | Word table objects, roman-numeral captions | **5** | HAND-COUNTED + LIVE PROBE |
| chapter3 .docx | 8 | 8 | Word table objects, arabic captions | 0 | HAND-COUNTED + LIVE PROBE |
| Lake Chapter 1.docx | 0 | 0 | no tables | 0 | HAND-COUNTED + LIVE PROBE |
| Revised Health Economics .docx | 5 | **6** (4 distinct labels) | Word table objects, arabic captions | **−1 (over-count)** | HAND-COUNTED + LIVE PROBE |
| IJAS … haemolymph.pdf | ~3 | 3 | PDF text, no table objects exist in PDF | 0 | HAND-COUNTED (pdftotext) + LIVE PROBE |
| final final L.pdf | ~106 | 33 | PDF text | ~73 | HAND-COUNTED (pdftotext) + LIVE PROBE |

Ground truth for `.docx` is the `<w:tbl>` count, which is exact. For the two
PDFs it is the count of distinct `Table N` caption labels found by `pdftotext
-layout`, an independent reader — **approximate**, because a line beginning
"Table 5" may be a cross-reference rather than a caption. Extractor counts are
`examples/table_label_census`, which prints nothing for a zero-table document;
the two zeros above are that silence, read as zero deliberately.

### B.3 Where detection fails — SOURCE INSPECTION

**Two stages, and the count is wrong in both directions at once.**

1. **docx parsing.** `<w:tbl>`, `<w:tr>`, `<w:tc>`, `<w:tblGrid>` are matched
   nowhere in the codebase — a grep for `w:tbl` across all Rust sources returns
   **one** hit, a doc comment. The walk in
   `gaply-core/src/extract/docparse.rs:880-885` handles only `w:t`, `w:tab`,
   `w:br`, `w:cr`; everything else falls to `_ => {}`. A table is flattened to
   one paragraph per cell with no grid.
2. **Structural extraction.** `ExtractionResult.tables` is not a list of tables.
   `TableRef` (`extract/mod.rs:305-310`) is `label`, `caption`, `location` — **a
   caption sighting**. It is produced by `detect_table`
   (`extract/mod.rs:462-472`) from one regex, `extract/stats.rs:256`:

   ```rust
   table_caption: Regex::new(r"(?i)^table\s+(\d+)[.:]?\s*(.*)$")
   ```

   `(\d+)` is **arabic only**. R PAPER's captions are `TABLE II.` and
   `TABLE IV.`, and two of its five tables have no caption paragraph at all.
   Hence 0 of 5.

So for R PAPER the answer is **both stages**: the grid is lost in docx parsing,
and the count is lost in structural extraction. Neither normalisation, the
manuscript model, report aggregation nor rendering drops anything further — they
faithfully carry a number that was already wrong.

The inflation direction is already recorded in the extractor's own doc comment
(`extract/mod.rs:437-461`, §11 D167): a thesis list-of-tables produces one
`TableRef` per contents line, **178 of 414 detections (43%)** across 20
manuscripts, so the count runs ~1.75× high. That is the Health Economics
over-count above, in miniature.

### B.4 Every downstream check that depends on the table count — SOURCE INSPECTION

| consumer | file:line | what depends on it |
|---|---|---|
| **reviewer-letter lane examination** | `src-tauri/src/pipeline.rs:783` — `extraction_examined: !extraction.tables.is_empty() \|\| !extraction.references.is_empty()` | whether the letter says *"Structure checks: no tables or dated references were found"*, and — via `nothing_examined()` — whether the verdict is **withheld** |
| displayed count in the exported PDF | `report_build.rs:241` → `report_compose.rs:404-411` | the sentence *"This manuscript has N sections, N tables and N references."* |
| AI-writing stylometry | `ai_detect.rs:602-636` | table paragraphs are excluded from the prose corpus; a spurious `TableRef` deletes a real prose paragraph from scoring, a missed table leaves cell runs scored as prose |
| research-state provenance + evidence graph | `research_state.rs:238, 259, 349-359` | a displayed provenance count, and `StatisticCoLocatedWithTable` edges |
| content hash / cache key | `research_state.rs:205-207` | incremental re-analysis identity |
| review-lens absence rule | `review_lens.rs:292, 331, 375-377` | `StateField::Tables` is `Mediated`, so no finding may rest on "this paper has no tables" |
| `table_findings` | `report.rs:1396-1400` | **withdrawn** — hard `return Vec::new()`; the body is live code behind that switch |

The frontend has **no** consumer of the count; it reaches a user through the
exported PDF and through the reviewer letter's examination wording.

---

## C. Certainty tier

### C.1 The chain — SOURCE INSPECTION

| stage | what it is | file:line |
|---|---|---|
| source rule | Rule 3: for each `Stat::PValue` location, is there any typed `Stat::EffectSize` in the same paragraph region | `validate.rs:262-279` |
| | Rule 4: for each `Stat::PValue` in Abstract or Results, is there a CI at the same location | `validate.rs:281-292` |
| deterministic observation | **"no effect size was detected in this paragraph"** — a true statement about the extraction | `extract/mod.rs:294-300` |
| inferred obligation | **"an effect size is required here"** — never stated anywhere; the finding's detail asserts it in prose (*"Statistical significance does not convey the magnitude…"*) | `validate.rs:270-276` |
| certainty assignment | `tier: CertaintyTier::MathematicallyCertain` — a **literal inside the loop over every validation flag**, consulting no rule id and no evidence | `report.rs:648-649` |
| rendering | badge text from `certainty_label`; the phrase itself is `"mathematically certain"` | `vocabulary.rs:91-97`, `ReportViewerPage.tsx:368-371` |
| journal evidence | **none** | — |

A second, independent assignment exists in TypeScript: `adapters.ts:138-156`
hardcodes `tier: 'mathematically_certain'` for the Stats Check screen, because
the `validate_manuscript` command returns a report carrying no tier at all.

### C.2 The two claims are different, and only one is supported

* **"No effect size was detected"** — supported. Deterministic, reproducible,
  and true of the extraction.
* **"An effect size is required here"** — unsupported. No journal requirement,
  no reporting-standard item and no checklist row is bound to either rule in
  production.

The wiring for the second exists and is switched off. `ItemCheck::EffectSizeReported`
and `ConfidenceIntervalReported` are declared (`journal_standards.rs:193, 195`),
bound to CONSORT 17a, STROBE 16a and TRIPOD 16 (`:229, :245, :261`) and
implemented (`report.rs:2938-2947`) — but `report::evaluate` has **no caller in
`src-tauri/src`** (grep returns none), and the checklist path passes
`bindings: &[]` and then strips reporting-standard rows via `design_independent`
(`report.rs:1798`, `:2491-2501`).

### C.3 What the seed says — LIVE PROBE over the bundled seed

Searched all 213 requirements, 258 expectations, 50 bindings and 50 conventions:

| requirement | rows in the seed |
|---|---:|
| effect size (or Cohen's d, eta-squared, standardised mean difference) | **0** |
| confidence interval, as a **requirement** | **0** |
| confidence interval, as an observed **frequency** in `conventions` | 10 |

The ten CI rows are descriptive corpus statistics, not obligations — each reads
*"of N recently published abstracts: … report both p-values and confidence
intervals … Read from ABSTRACTS, not full text"*, `status: inferred`.

**For the journal run 31 actually targeted**, `nature-medicine`'s own row says:
*"of 77 recently published abstracts: 7 report both p-values and confidence
intervals, 5 p-values only, 8 intervals only, **57 neither**"*.

**No seeded journal states an effect-size or confidence-interval requirement.**
Nothing above is inferred from general academic practice.

---

## D. Checklist coverage — SOURCE INSPECTION + LIVE PROBE

Three separate questions, kept separate:

| | rule exists? | journal requirement exists in the seed? | rule bound to that requirement? |
|---|---|---|---|
| **keyword count** | **no** — `RequirementKind` has 9 variants (`WordLimit, AbstractLimit, SectionRequired, ReferenceStyle, ReferenceLimit, DataPolicy, ReportingStandard, FigureLimit, Other`, `journal_extract.rs:89-99`) and none is a keyword count; no checklist row constructs one | **no** — 0 seed rows match `keyword` | n/a |
| **graphical abstract** | **no** — `graphical abstract` / `visual abstract` appear **nowhere** in `gaply-core/src`, `src-tauri/src` or `src/` | **no** — 0 seed rows | n/a |

---

## E. Journal scope — SOURCE INSPECTION

**Not implemented.** Aims-and-scope compatibility is assessed nowhere.

* No deterministic code computes it: no scope text, no topic model, no
  comparison of manuscript subject against journal subject.
* The journal profile has no scope field. `JournalFingerprint` carries
  `journal_key, version, content_hash, fetched_at, refetch_after, source_count`
  — no aims or scope.
* A **field** exists for a sentence about it: `reviewer_agent.rs:247`
  `journal_fit_note`, filled only from a cloud reviewer model's response
  (`reviewer_agent.rs:741`), and the local path sets it to the empty string
  (`synthesize.ts:92`), which the panel renders as `—`
  (`ReviewerLetterPanel.tsx:167`).
* `PremiumFeatures.tsx:92` advertises *"Verify journal fit. AI-powered matching
  to find the best venue for your manuscript."*

So: a slot, an advert, and no assessment. Not built here.

---

## F. Provenance

| item | value |
|---|---|
| HEAD | `1438ba19f09bcf1ec164917a99c59f8acc85f1f6` |
| run identifier | manuscript id **31**, cache key `report:v2:e3:31`, 20,471 bytes, created `1790128614` = 2026-09-23 07:26:54 local |
| manuscript | `/Users/rishi/Desktop/R PAPER .docx`, 527,284 bytes, sha256 `effbb86c24958610e83b60546114ac7cd9f6cc7f5e24988f9dac19337feda87f` |
| database | `~/Library/Application Support/ai.gaply.app/gaply.db` (14,721,024 bytes) **plus its `-wal`, 490,312 bytes** |
| seed | `gaply-core/data/journal-seed.json`, sha256 `646a9246…2392`, `run_started_at = 1789728313`, 10 fingerprints / 213 requirements |
| probe database | in-memory (`Database::in_memory`) — the live database was never written |

**One instrument note, because it nearly cost the report its subject.** The
first read of the database used `?mode=ro&immutable=1`, which tells SQLite the
file cannot change and therefore **ignores the write-ahead log**. Run 31 lives
in the WAL, so that read returned run 30 as the newest run — a correct answer to
a narrower question, and indistinguishable from "run 31 does not exist". Every
figure above comes from a read without `immutable`.

---

## G. Confirmed defects

### G1 — a pasted URL binds to the picked journal with no verification

* **Observed:** `https://link.springer.com/journal/11418/submission-guidelines`
  (Journal of Natural Medicines) resolved to `journal_key = "nature-medicine"`
  and stored 1 requirement under it, `origin = "crawled"`, readable through the
  product's own reader.
* **Expected:** either the URL identifies the journal, or a URL that disagrees
  with the picked journal is refused.
* **Reproduction:** reproduced, cause identified (LIVE PROBE, twice).
* **Cause:** `JournalIdentity::resolve` (`guidelines.rs:268-274`) derives the key
  from the picker's key or a slug of the display name; the URL is never compared
  to anything. `store_requirements(db, k, url, …)` (`guidelines.rs:604-613`)
  writes both without a cross-check.
* **Affected path:** `PublishReadyPage` → `ingest_guidelines`
  (`commands.rs:502-505`) → `guidelines::ingest_with` → `journal_store` →
  `report::build_checklist`.
* **Certainty:** high — run live, end to end, with the row printed back.

### G2 — a Word table is invisible to the table count

* **Observed:** R PAPER contains 5 `<w:tbl>` objects; the extractor reports 0,
  and run 31's stored extraction has `tables: 0`.
* **Expected:** 5, or an explicit statement that tables were not extracted.
* **Reproduction:** reproduced, cause identified (HAND-COUNTED + LIVE PROBE).
* **Cause:** `<w:tbl>`/`<w:tr>`/`<w:tc>` are matched nowhere
  (`docparse.rs:880-885`), and detection is a caption regex requiring arabic
  digits (`stats.rs:256`) against captions reading `TABLE II.`.
* **Affected path:** `docparse::parse_docx` → `extract_from_text` →
  `ExtractionResult.tables` → the seven consumers in §B.4.
* **Certainty:** high.

### G3 — `extraction_examined` is driven by a count that is wrong in both directions

* **Observed:** `extraction_examined: !extraction.tables.is_empty() || …`
  (`pipeline.rs:783`) is fed by caption sightings.
* **Expected:** a lane-examination flag that reflects whether the lane examined
  anything.
* **Reproduction:** reproduced, cause identified (SOURCE INSPECTION; the
  wrong counts in §B.2 are measured).
* **Cause:** a contents-page line makes the flag true without a table
  (§11 D167's 43%); a flattened or roman-numbered table makes it false with
  five. It can flip a withheld verdict either way.
* **Certainty:** medium-high — the inputs are measured, the flip was not
  separately provoked.

### G4 — "mathematically certain" is asserted for an inferred obligation

* **Observed:** *"statistical rule failed: missing effect size"* rendered as
  **mathematically certain** in run 31.
* **Expected:** certainty about the observation ("no effect size was detected"),
  not about the obligation ("one is required here").
* **Reproduction:** reproduced, cause identified (STORED RUN + SOURCE
  INSPECTION).
* **Cause:** `report.rs:648-649` assigns the tier as a literal to every flag the
  validator emits; no seeded journal states either requirement (0 rows); the
  standards items that would bind it are unreachable in production.
* **Certainty:** high for the mechanism. Whether the tier *should* change is a
  judgment this report does not make.

### G5 — the seed carries no effect-size or CI requirement while the product implies one

* **Observed:** 0 requirement rows for either, across 10 journals; the only CI
  rows are frequencies, and Nature Medicine's says 57 of 77 abstracts report
  neither.
* **Reproduction:** reproduced, cause identified (LIVE PROBE over the seed).
* **Certainty:** high.

## H. Suspected defects

### H1 — a crawled ingest replaces a bundled journal's provenance

`INSERT OR REPLACE … VALUES (…, 'crawled')` (`journal_store.rs:384-387`) on a
`journal_key` primary key (`migrations.rs:1039`). Read, **not run** — the probe's
database held no seed, so the replacement was not observed. Certainty: medium.

### H2 — name-slug collisions can bind two journals to one key

`"The BMJ"` → `the-bmj` vs the stored `bmj`; any two titles slugifying alike
collide. Noted in the source (`guidelines.rs:242-249`); not exercised here.
Certainty: medium.

### H3 — PDF table counts are unmeasured, not merely wrong

`final final L.pdf` shows 33 detections against ~106 caption labels. The ground
truth is a caption-label count from an independent reader and may itself include
cross-references, so the gap's size is uncertain even though its direction is
not. Certainty: low on the magnitude, high on the direction.

### H4 — table cell text is scored as prose

`ai_detect.rs:583-586` states it: the extractor locates captions, not cell
paragraphs, so a table body is scored as prose by the AI-writing lane. Not
measured here. Certainty: medium, and it is the codebase's own claim.

---

**Unrelated to this report:** the `ai_eval_cli` target fails at HEAD for the
reason recorded in §11 D210 (`required-features = ["devtools"]`). It is not a
regression from anything measured here.
