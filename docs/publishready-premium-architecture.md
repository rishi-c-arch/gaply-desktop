# PublishReady Premium — Architecture

*Design document, v6. Grounded in the 13 September teardown (HEAD 568efd4), AgentFlow (arXiv 2604.20801), Context Engineering (arXiv 2603.09619), MetaGPT (2308.00352), AutoGen (2308.08155), and ChatDev (2307.07924). Revised after three external reviews and against the attached reviewer-criteria document ("What Reviewers of Top-Quartile Journals Evaluate"); §14 records what was taken and what was refused.*

> **v6 — one correction, and it is to the architecture.** §3.1's "Privacy class"
> column conflated *what needs consent to leave the machine* with *who may read
> it*. Read as a read-permission it would have required premium consent to parse
> a file the user had just opened. The table now states the egress class and the
> read rule separately; §3.1 carries how it was found, which is the part worth
> keeping — a spec, an implementation and a test all agreeing and all three
> wrong.
>
> **v5 — what changed and why.** v4 was written from the teardown and never
> checked against the tree. Reading the code found nine claims about current
> behaviour that were wrong, stale, or untraceable — one of them load-bearing
> enough to have sent a whole phase at a table nothing writes to. Every
> correction below is marked **[v5]** with what was measured. §14 records that
> these came from reading the code, not from a review.
>
> **Two things are now built** (they were not when v4 said "nothing here is
> built"): the two-gate privacy boundary (§2.2) and its decision record,
> `docs/AI_ENGINE_PLAN.md` §11 D152. Everything else is still design.

> **Gaply does not ask fifty agents whether a manuscript is good. It builds a machine-readable representation of the research, gives each specialist the minimum context it needs, tests the evidence chain independently, routes disagreement to targeted re-analysis, and produces a journal-specific readiness assessment a reviewer could retrace.**

---

## 0. What this is, and what it refuses to be

**The product:** a researcher uploads a manuscript, optionally its supporting analysis (code, SPSS, MATLAB, lab notebooks, data), and names a target journal. Gaply returns the manuscript marked up as a reviewer at that journal would mark it, a structured account of what would need to change, and an honest estimate of where it stands — plus a chat that answers questions about *this* manuscript and *these* findings.

**Three things it refuses to do, each because the teardown showed what happens otherwise:**

1. **It will not print a probability it cannot calibrate.** **[v5 — corrected]** v4 said `citation_need` "shipped at an assumed accuracy and measured 0.34." The 0.34 is traceable to nothing: it appears nowhere in `AI_ENGINE_PLAN.md`. The real measurement (§11 D128) is **18.3% weighted precision against an 18.0% no-skill baseline** — and the lane is **already retired**, not shipping. D128's finding is also sharper than "it was weak": *"It was retired not because it was WEAK but because it was INDISTINGUISHABLE FROM A RULE THAT FLAGS EVERYTHING."* A weak signal invites a better prompt; this one had nothing to improve. AI-detection is the live example instead — it ships with no labelled set and flagged hand-written prose `major` in **19 of 22 stored reports** (measured). A publication-likelihood number is the highest-stakes output the product could have, and it ships as a *rubric score* until there is accept/reject ground truth to calibrate it against. See §8.

2. **It will not quietly relax the privacy claim.** The release gate is the strongest thing in the product — five invariants, an independent server-side validator, *"no manuscript text crosses the proxy boundary."* Premium sends the manuscript to a cloud model. That is not a relaxation of the claim; it is a *different tier with a different claim*, consented to before upload and enforced in Rust. See §2.

3. **It will not add agents to a harness that cannot revise.** All six current participants are `PrecomputedAgent`; `revise()` returns `None`; every run converges in one round with `revised_agents: []`. AgentFlow's central result is that *the harness*, not the headcount, moves outcomes several-fold with the model fixed. The first agent that revises in production is the prerequisite for the fortieth. See §5.

---

## 1. Inputs

| Input | Required | Format | Where it goes |
|---|---|---|---|
| Manuscript | yes | .docx, .pdf, .tex, .md | Manuscript layer (§3) |
| Supporting analysis | no | .py .R .m .sps .ipynb .csv .xlsx, lab notebook PDFs, zip | Analysis layer, parsed per type |
| | | **[v7] THE APP ACCEPTS NONE OF THESE.** Every picker is `pdf, docx, txt, md` (`AnalysisTheaterPage.tsx:105`, `AiCheckPage.tsx:368`, `GapFinderPage.tsx:175`). So the analysis layer has no input, and *"supporting analysis is the differentiator"* below describes a capability with no way in. `AnalysisRecord` and the SPSS parser now exist (`gaply-core/src/analysis/`); **the upload path does not, and building it is the next thing this differentiator needs.** | |
| Target journal | yes | pick from registry, or paste a URL to author guidelines | Journal layer |
| Field / subfield | inferred, confirmable | from manuscript + journal | routes the specialist cluster |

### 1.1 The research-type router

Before any cluster wakes, a deterministic-first classifier produces a `ResearchProfile`: study design, domain, analysis types present, data modalities, article type, and the reporting standards the journal binds to that design. Specialists activate against the profile, so a systematic review wakes PRISMA, search-strategy and risk-of-bias specialists; a wet-lab paper wakes controls, replicates, blinding and image-integrity; an ML paper wakes leakage, baselines, ablation and external validation. **Agent count is a consequence of the research, not a product requirement** — one paper may activate 25 specialists and another 60.

**Supporting analysis is the differentiator.** A reviewer who can see the code checks the statistics against what was actually run.

**[v7 — measured before writing the parsers, and only one of the three ships.]** The phase asked for Python, R and SPSS parsers. Measured first: the researcher corpus (`~/Desktop`, `~/Documents`, `~/Downloads`, depth 3–4) holds **zero** analysis files of any kind against 29 `.docx`; the only real analysis artefact on the machine is **SPSS's own journal, 12.7 KB, five `FACTOR` procedures over 37 variables** with their rotation, extraction and missing-data options. So SPSS syntax is the only one of the three with a real input, and it is the only one built — against that file, recovering **82 commands, 5 statistical, 0 unrecognised, and 5 unparsed lines that are exactly the 5 human date lines SPSS writes between sessions.** Python and R are deliberately not built: a parser whose only inputs are fixtures its author wrote inherits its author's premise, which this project has recorded three times, and with no upload path no parser of any language has an input. The `.py` and `.R` files that ARE on the machine are IBM's bundled extension source — plotting plumbing, not anyone's analysis, and measuring against them would be §11 D160's boundary-drawn-too-wide again.

**The SPSS parser's own first result was wrong, and the count is what caught it.** It reported 53 commands, **0 unrecognised** and 34 unparsed lines. Twenty-nine of those 34 were real commands: a one-word command IS its own terminator, so the token arrives as `EXECUTE.` and failed an alphabetic keyword test. The kinds table looked perfect either way. **A parser that discards what it cannot read reports a clean record of a file it mostly failed to parse**, which is why `unparsed_lines` and `Unrecognised(keyword)` are both carried and both printed. Nobody else offers this because nobody else has the analysis. Per-type parsers produce a normalised *analysis record*: which tests were run, on what variables, with what parameters, producing what outputs. That record is what the methodological cluster reasons over — not the raw script.

---

## 2. Privacy: two tiers, one Rust boundary

The teardown found consent enforced in seven screens via `localStorage`, missing in one, and nothing in Rust. That is a preference, not a boundary. Premium is built on a boundary.

### 2.1 The two contracts

| | Free | Premium |
|---|---|---|
| What leaves the machine | reference DOIs/titles (consented), structured summaries (gated) | the manuscript, the analysis record, structured findings |
| Where it goes | CrossRef, OpenAlex, gaply-proxy → structured only | gaply-proxy → OpenAI, with the manuscript |
| The claim | *"No manuscript text crosses the proxy boundary."* — unchanged, gate stays green | *"Your manuscript is sent to OpenAI for review. Here is exactly what, and why."* — stated before upload, in the report, in the file |
| Enforcement | release gate, five invariants, unchanged | a distinct Rust type that cannot be constructed without a consent record |

### 2.2 The boundary in code

```
enum Tier { Free, Premium(ConsentRecord) }

struct ConsentRecord {
    manuscript_id: Uuid,
    consented_at: DateTime,
    scope: ConsentScope,      // bitset — see below
    provider: ProviderClass,   // [v5] a CLASS, not a vendor — see below
    statement_version: u32,    // which consent text they saw
}
```

```
ConsentScope: Manuscript | AnalysisMetadata | AnalysisCode | AnalysisData
            | ExternalEvidence | JournalResearch
```

Each scope is a separate checkbox with its own statement. A researcher may send the manuscript and keep the code local; that combination is common and should be one click. The default is manuscript-only.

- **[Phase 1 — BUILT, and the v5 note below about placement is superseded]**
  `ConsentRecord`, `ConsentScope`, `Tier`, the `consent_records` table
  (migration 22, append-only) and the store now exist in **`gaply-core`**, not
  the app crate. `ConsentRecord::from_persisted` is **module-private** — not
  `pub`, not `pub(crate)` — and `store::record` / `store::resolve` are its only
  callers, so the only way to obtain a record anywhere in the workspace is to
  write a row or read one back. `check_tier` takes the real resolver; the
  partition tests can no longer fabricate a consent. §11 D152 carries the
  reasoning and the two deliberate breaks.

  The v5 bullet below said it lived in the app crate because the design's `Uuid`
  and `DateTime` were dependencies `gaply-core` does not carry. **That reason was
  void on arrival** — the implementation used `now_epoch()` and a `String` id.
  What forced the move is that `Database::conn` is `pub(crate)`, so a store in
  the app crate cannot read the table, which would have kept `from_persisted`
  public — the hole itself.
- **[v5 — corrected]** v4's field was `provider: Provider // OpenAI, named`. **The desktop cannot honestly record that.** `gaply-proxy/app/main.py:146-152` picks the provider server-side from `GAPLY_LLM_PROVIDER`, and `:262` states the property deliberately: *"nothing is persisted. The desktop never knows which one handled it."* A consent record naming a vendor would be a claim this machine can neither check nor be told is wrong — an unfalsifiable privacy statement. The field names a **class** (`CloudLlmViaProxy`) instead. Naming the vendor is available and costs provider-blindness; §11 D152 records that trade rather than leaving it a gap.
- `ConsentRecord` is persisted in a new table, one row per manuscript per consent event, never deleted. **[v5]** The type lives in the **app crate**, not `gaply-core`: the design's `Uuid`/`DateTime` are two dependencies `gaply-core` does not carry and should not acquire — it is declared *"portable, Tauri-free"* at `rust-version = "1.77.2"`, with no network and no clock. It uses `now_epoch()` `i64` and a `String` id, the conventions already in the codebase.
- The premium proxy client takes `Tier::Premium(record)` and nothing else. A free-tier call *cannot* reach the premium route because there is no constructor path from `Tier::Free` to it. This is the type system doing what `localStorage` cannot.
- **[v5 — BUILT, and not as a sixth invariant]** v4 said the release gate is "extended with a sixth invariant: TIER". It cannot be. `check_privacy` **fails** any payload containing manuscript text, and a premium payload *is* manuscript text — so a single enumeration would have had to take a tier argument and skip PRIVACY, which is a tier-aware PRIVACY under a different name.

  So there are **two gates, each unconditional**:

  | gate | invariants | guards |
  |---|---|---|
  | `run_free_gate` | PRIVACY + PROVENANCE, SELECTION, COMPARISON, PERSISTENCE | the free route |
  | `run_premium_gate` | **TIER** + the same four structural | the premium route |

  `check_privacy`'s worth is that it takes no argument — *"no manuscript text crosses this boundary"* survives only while nothing can weaken it. PRIVACY is absent from the premium gate not because it was relaxed there, but because manuscript text is what that tier exists to send. TIER is its counterpart: every payload carrying manuscript text carries a `ConsentRecord` id, and every id resolves to a stored row whose scope covers what was sent. Both directions.

  Both invariants call **one** predicate, `manuscript_text_in`. Separate detectors would let a payload read as clean to one and text-bearing to the other, and a payload passing both routes would exist with every test green.

  One payload passes **neither** gate — manuscript text with no consent record. That is the safety property, not a gap, and it is asserted as required. Full reasoning, the three deliberate breaks, and what is still missing: **§11 D152**.
- The consent statement is versioned. If the text changes, existing records do not cover the new scope and the user is asked again.

### 2.3 What premium still keeps local

Everything deterministic runs locally regardless of tier: extraction, the five statistical rules, structural checklist, reference-list consistency, the analysis-record parsers. Premium adds cloud *judgement* on top of local *computation*. The findings that carry "mathematically certain" never depended on the cloud and still don't.

### 2.4 Retention and the proxy

The proxy today persists nothing and never tells the desktop which provider answered. Premium keeps both properties. Additionally:

- The proxy forwards with the provider's zero-retention / no-training flag set, and refuses to forward if the provider's API does not offer one.
- The desktop stores the *response*, never re-sends the manuscript for the same analysis version. Re-runs on an unchanged manuscript are served from the stored response.
- The chat (§10) reads from stored findings. It never re-sends the manuscript.

---

## 3. The context fabric

The Context Engineering paper proposes five criteria for an agent's informational environment: **relevance, sufficiency, isolation, economy, provenance.** The product already does provenance well — every finding is tagged computed or judged.

**[v5 — corrected]** v4 justified the isolation argument with *"the pipeline's AI lane and AI Check use different model selectors and can disagree about the same manuscript."* **That was already fixed before v4 was written.** Both go through `select_deep_model()`: `pipeline.rs:326` → `models::perplexity_model()` → `models/mod.rs:936-938`, and `aicheck.rs:329` directly. `pipeline.rs:318-324` documents the merge and `pipeline.rs:788` pins it.

The lanes *can* still disagree — the pipeline calls whole-document `detect_extraction` while AI Check runs the tiered stage-1/deep path — but that is a different defect with a different fix, and the layer argument below should be read as resting on the general principle rather than on that example.

The fabric is six layers. Each is a distinct epistemic object with a distinct privacy class — that pairing is the test for whether something is a layer or merely a label. Each agent declares which it reads. The harness enforces it.

### 3.1 Layers

**[v6 — the "Privacy class" column meant the wrong thing, and this is a defect
in the architecture rather than a note about the code. See below the table.]**

**PRIVACY CLASSES ARE ABOUT EGRESS, NOT READING.** A layer's class says what
needs consent to **leave the machine**, never who may look at it locally. The
column is renamed accordingly, and the read rule is stated in full because the
wrong half of it is the part that was missing:

> **Any agent may READ any layer. A CLOUD agent may only SEND a layer whose
> egress class requires consent if it declares a scope that COVERS that layer —
> and a scope that exists but does not cover it is rejected.**

That last clause is the hole: `requires_consent.is_some()` waves through an
agent that declares `JournalResearch` and then sends the manuscript.

| Layer | Contents | Egress class — what consent it needs to LEAVE | Who may read it (locally: anyone) |
|---|---|---|---|
| **Manuscript** | full text, sections, figures, tables | `Manuscript` consent to leave. **No consent to read** — the free tier's extraction reads all of it locally and sends nothing | manuscript-facing specialists |
| **Analysis** | the analysis record parsed from code/SPSS/MATLAB/notebooks; raw data structure | `AnalysisCode` / `AnalysisData` to leave — code and data are more sensitive than prose | methodological specialists |
| **Journal** | ingested guidelines, scope, recent-paper corpus, reporting-standard bindings — the `JournalFingerprint` (§7) | none — public, shared across users | journal-fit and reporting specialists |
| **External evidence** | cited works, retraction records, OA full texts, public literature the research agent gathers | `ExternalEvidence` — public, cached, injection-guarded, but the SELECTION is about this manuscript | integrity and literature specialists |
| **Research state** | the machine-readable study: question, hypotheses, design, population, variables, outcomes, claims, methods, analyses, results, table/figure POINTERS, references, uncertainties | **[v7] CONDITIONAL — see below.** None while the scientific layer is absent; `Manuscript` once it is derived | every agent |
| **Verdict** | findings with epistemic status, evidence, per-agent opinions, revision history, disagreement map, consensus, decision ledger | never leaves | synthesis, chat |

#### [v7] THE RESEARCH STATE'S CLASS IS NOT A PROPERTY OF THE LAYER

**Switching the scientific layer on (Phase 4, §12.1 item 3) made "carries no prose" false**, and this is a correction to this document rather than a note about the code. `ScientificClaim::statement` is a manuscript sentence or a span of one:

```text
manuscript:       "Treated larvae showed values appreciably higher than the
                   untreated control across both seasons."
claim.statement:  "higher than the untreated control across both seasons."
```

A verbatim substring of the manuscript, inside a layer this table said may travel without `Manuscript` consent. Everything `research_state.rs` composes ITSELF is still prose-free — `SectionSummary` carries a paragraph COUNT and never a paragraph — and that guarantee does not extend to the layer it composes by `Arc`. So the egress question is per-STATE, not per-LAYER: `ResearchState::carries_manuscript_prose()`.

**The test meant to catch this was green, and green for the wrong reason.** `the_research_state_carries_no_manuscript_prose` ran while `science` was always `None` and its fixture produced **zero claims** — it could not have failed however much prose a derived layer carried. It now asserts its own precondition (claims > 0) and failed immediately when that precondition was added, which is how this was found. **A privacy assertion that cannot reach the thing it guards is not a weaker guard; it is no guard, wearing a green tick** — the same shape as the XML containment assertions in §11 that were satisfied by a document Word refused to open.

#### [v6] HOW THIS WAS FOUND — three things agreeing, and all three wrong

The original column said the Manuscript layer was *"premium, `Manuscript`
consent"*, under a heading — *"Who may read it"* — that made it a READ
permission. Read that way, **the free tier would need premium consent to parse a
file the user had just opened.**

The sequence is the finding, not a footnote:

1. **The spec was wrong** — a privacy column that conflated reading with sending.
2. **The implementation followed the spec.** `agent_graph.rs`'s first rule was
   `layer.is_premium_consented() && requires_consent.is_none()` → error, for any
   agent, cloud or not.
3. **The test pinned the implementation.**
   `reading_the_manuscript_layer_without_consent_is_rejected` asserted exactly
   that, and passed.

Three artefacts agreeing with each other, all three wrong, and no amount of
re-reading any of them would have surfaced it — each was consistent with the
other two. **What broke the loop was building a real graph and watching the
validator reject something obviously legitimate**: `extraction`, a local
deterministic pass that transmits nothing, failed to validate. The absurdity of
the rejection is what exposed the premise; the test was then corrected along
with the rule it was pinning.

This is the §11 D129 shape one level up — not two copies of a claim drifting
apart, but three copies of a claim staying perfectly in sync while all of them
are false. Agreement between a spec, its implementation and its test is evidence
that they were derived from each other, not that any of them is right.

**[Phase 1 — two corrections, both found by building it.]**

**Tables and figures appeared in TWO layers with incompatible privacy classes.**
The Manuscript row claims them as premium-consented and the Research-state row
claimed them as gate-safe; a table cannot be both, and §3.1's own test — *"a
distinct epistemic object with a distinct privacy class"* — is what the overlap
fails. The split the code forces: a `TableRef` is `{label, caption, location}`, a
POINTER with no cell data, and that is gate-safe; a table's CONTENTS are
manuscript text and are not extracted at all. The row above now says pointers.

**"the free tier already computes most of it" was wrong — it CAN and does not.**
`ExtractOptions::scientific` is opt-in and **no production caller opts in**;
`extract_from_text` passes `ExtractOptions::base()`, so `ExtractionResult.scientific`
is `None` on every real run. The extractors for claims, variables, methods and
datasets are written and tested; they are switched off, because (in the option's
own words) *"the scientific layer is consumed by one that does not exist yet"*.
`ResearchState` is that consumer, and turning the layer on is now a **Phase 2
prerequisite with a cost** — four extra passes over the manuscript for every
caller — not a detail inside Phase 4. §12 Phase 2 records the decision.

**Figures are listed and nothing extracts them.** `extract` detects tables only.
`ResearchState::figures` is present and permanently empty, with a provenance row
recording `(none — no figure extractor exists)` at count 0, because "no extractor
runs for this" and "this extractor found nothing" are different facts.

The four of these must stay distinguishable in every finding: *the manuscript says N=624; the SPSS output says valid N=600; the journal requires exclusions be reported; the specialist concludes the 24 need explaining.* Each is a different layer, and a finding that blurs them is a finding a reviewer cannot retrace.

### 3.2 Why layers rather than one prompt

- **Isolation.** A journal-fit agent has no business reading raw manuscript prose; it reads the derived structural map against the journal's scope. A statistics agent has no business reading the journal's recent papers. Declaring layers per agent makes the wrong read a type error.
- **Economy.** Each agent gets what its declared layers contain, chunked to its budget, and nothing else. The 8 GB machine constraint the teardown named (one model at a time, explicit `unload_slm2()`) becomes a scheduling problem the harness owns rather than each lane.
- **Provenance.** Every finding records which layers its producing agent read. A finding derived from the journal layer alone is a claim about the journal; one derived from manuscript + journal is a claim about fit. The report can say which.

### 3.3 Versioning

Every layer carries a version. Every finding records the versions of every layer its producer read, plus the agent-graph version and the model identifiers. *Why did Gaply's assessment change?* is then a diff, not a mystery. This is also what makes incremental re-analysis (§5.5) safe: a node re-runs when any input version it depends on has changed, and only then.

### 3.4 The journal layer — what exists, and what is actually missing

**[v5 — THE BIGGEST CORRECTION IN THIS DOCUMENT.]** v4 opened: *"`journal_guidelines` has 0 rows after 27 manuscripts. The checklist has only ever been structural."* The count is real. **It is a count of a table nothing writes to.**

Measured against the live database:

```
select count(*) from journal_guidelines;                       -> 0
select count(*) from manuscripts;                              -> 27
select source_type, count(*) from documents group by 1;        -> journal_guideline | 6
                                                                  paper             | 22
select count(*) from chunks;                                   -> 59
```

`journal_guidelines` is a migration-3 vestigial table, sibling of `reference_styles`, which `migrations.rs:93` labels *"SUPERSEDED (reserved, intentionally unused)"*. Its only mention outside `migrations.rs` is a provenance **string literal** at `guidelines.rs:166` — never a SQL write. The corpus holds **six rows**, all `status='ingested'`, 59 chunks.

**[v6 — and six rows is not six guideline pages. THE SAME SUBSTITUTION AS v4's, ONE LEVEL DOWN.]** Opened, on 14 Sep 2026:

| doc | entry URL | chars | what it actually is |
|---|---|---:|---|
| 1 | `https://www.nature.com/nm` | **368** | nature.com's no-JavaScript banner — *"You are using a browser version with limited support for CSS…"* |
| 2, 3 | `https://www.bmj.com` | 10,875 / 10,883 | the BMJ **homepage** — news headlines |
| 4 | `https://bmcpublichealth.biomedcentral.com` | 5,085 | the journal **homepage** — navigation |
| 5 | `journals.plos.org/plosone/s/submission-guidelines` | 98,882 | **real guidelines**, 29 chunks |
| 6 | `journals.plos.org/plosmedicine/s/submission-guidelines` | 64,395 | **real guidelines**, 19 chunks |

**The corpus is TWO guideline pages.** Four of the six entry URLs are journal homepages — the URLs are literally `https://www.bmj.com` and `https://www.nature.com/nm` — and one of those is 368 characters of a browser warning.

**Why they were stored as `ingested`:** the liveness check was `MIN_GUIDELINE_CHARS = 200`, a rule about LENGTH. Every way a fetch fails while returning HTTP 200 clears it — a browser banner at 368, a bot interstitial at ~226, a homepage at 10,875. Replaced (§11 D159) with a check that asks what the page IS: an exact title rule for interstitials, and obligation-plus-requirement evidence for guideline content.

v4 said *"0 rows"*; v5 corrected that to *"a count of a table nothing writes to"* and then wrote *"six ingested guideline pages"*. **That is the same error twice, in the same direction: a count of records read as a count of the thing the records are about.** The number to beat is 2.

And the pipeline v4 said did not exist is wired end to end:

| stage | where |
|---|---|
| fetch, rate-limited, untrusted | `src-tauri/src/guidelines.rs` |
| `strip_hidden` / denylist / quarantine | `gaply-core/src/sanitize.rs:43,79` |
| RAG ingest as `SourceType::JournalGuideline` | `gaply-core/src/rag.rs:29,41` |
| checklist query against that corpus | `pipeline.rs:944` (a passing test) |
| UI trigger, behind consent | `PublishReadyPage.tsx:157-163` |

**The corpus is small because the guidelines URL is optional and six people pasted one — not because the layer is unbuilt.** This is the project's own standing lesson (*"a static trace tells you what a mechanism DOES, not that it is the mechanism in play"*) in its fourth instance, and the first to reach a build plan: Phase 3's v4 deliverable was satisfiable by inserting into a table no reader reads.

**What is actually missing** is the depth, not the plumbing: one flat page per journal instead of a crawl, no requirement/convention/expectation split, no status on any fact, no article-type binding, no comparable corpus, no `JournalFingerprint` type. Everything below is therefore an **extension of a working path**, and the first task of Phase 3 is to read `guidelines.rs`, `journal_registry.rs` and `paper_corpus.rs` before writing anything:

- **Guideline ingestion is a crawl, not a scrape.** A pasted URL is an *entry point into the journal's instruction ecosystem*, not a page to read. Requirements are spread across author guidelines, article-type pages, submission checklists, formatting, ethics, data availability, reporting standards, trial registration, AI-use policy, conflict-of-interest, preprint and licensing pages, and downloadable templates or checklists. Ingestion is:

  ```
  entry URL → canonical journal → discovery of permitted linked guideline pages
            → document collection → source classification (which page is about what)
            → deep extraction → normalisation → conflict detection
            → article-type binding → JournalFingerprint
  ```

  Discovery is bounded to the journal's own domain and the publisher's author-services domain, and stops at a depth of three.

  **[v6 — THE DISCOVERY RULE IS INVERTED. v5 said "follows only links whose anchor text or path names a guideline topic"; that is now crawl ORDER, never admission.]**

  **The evidence is Nature Medicine.** Every extractable requirement it publishes — main-text limits of 4,000 / 2,000 / 1,000 words, abstract 150, references 10, display items 2, each already bound to an article type — is on ONE page: `https://www.nature.com/nm/content`. Its path is `/nm/content`; its anchor text is *"Content types"*. Neither names a guideline topic. A 30-term lexicon written by reading PLOS does not contain that phrase, and PLOS has no equivalent page to learn it from — its article types are inside the submission-guidelines page.

  The deciding argument is not that a lexicon is incomplete; every heuristic is. It is that **a lexicon's completeness is unknowable for a publisher nobody has read, and its failure is silent** — a crawl that skipped the only page with numbers on it looks exactly like a crawl that found everything.

  So discovery is **fetch-and-classify**: follow links on the journal's own domain within the depth bound, fetch the page, and let `guidelines::classify_page` decide what it is. The lexicon orders the frontier, so a missing term costs latency rather than coverage.

  **The standing evidence is a number, reported per journal: `lexicon_misses`** — pages classified `Guideline` that no lexicon term would have admitted. Every one is a page the v5 rule would have dropped without saying so.

  **Two bounds, in `config/journal-crawl.json` and not in code, because fetch-and-classify costs one request per candidate and the bounds are the whole difference between crawling a journal and crawling a publisher:** `max_pages` and `max_depth`. There is no compiled-in default — a missing or malformed config is an error, since a budget that silently falls back to a constant is a budget nobody set. **A crawl that ends on `max_pages` is reported as `StoppedBy::Budget`, and any count derived from it is a lower bound rather than a coverage claim.**

  **Links from an `Interstitial` are never followed.** A bot challenge or cookie wall returned with HTTP 200 means the request did not reach the journal; its `<a>` elements are the vendor's error furniture, and following them spends the budget on a wall. Every fetched document passes the injection guards (strip_hidden, denylist, perplexity, quarantine); they stay and are extended per §11. The output is a **source tree**, and every extracted requirement records its `source_document`, `source_heading` and `source_span` — the exact sentence — not just a URL.
- **Every journal fact carries a status**, and the product never flattens them into "the journal requires…":

  | Status | Meaning | Example |
  |---|---|---|
  | `VERIFIED` | stated in the journal's own guidelines, span recorded | *Word limit 5,000 — Author Guidelines §3.2* |
  | `INFERRED` | derived from the comparable corpus, with n | *External validation present in 18 of 25 comparable papers — not a stated requirement* |
  | `UNAVAILABLE` | searched, nothing authoritative found | *AI-use policy — no policy located on the journal's pages* |
  | `CONFLICTED` | two sources disagree | *Abstract limit 250 (Author Guidelines) vs 300 (Article Types page)* — both shown, neither chosen |

- **Comparable corpus, not "latest fifty."** For a named journal, fetch *recently published* papers' abstracts and section structures via OpenAlex (public, no manuscript involved). Publication date is what the source provides; acceptance date is not. The product says "recently published" because that is what it has, then filter to those *comparable* to the manuscript: same article type, same or adjacent study design, within a date window, above an embedding-similarity threshold. The corpus has a minimum (below which conventions are `UNAVAILABLE`), a target, and a maximum. From it derive: typical length, typical section set, methods the journal actually publishes, statistical conventions it expects. This is what makes "fit" a measured comparison rather than a model's impression.
- **Reporting-standard bindings** — the journal's own instructions say which checklist (CONSORT, PRISMA…) applies to which study type. Ingest those as structured requirements, not prose.

---

## 4. The agents

MetaGPT's contribution is that agents work when they have *roles with defined artefacts* — a PRD, a design, a task list — rather than free-form conversation. ChatDev's is that a *pipeline of phases with role pairs* beats a mesh for producing a coherent artefact. AutoGen's is that the *conversation topology* is a first-class design choice. AgentFlow's is that all of it — roles, prompts, tools, topology, coordination — should live in **one typed graph** so it can be searched and enforced.

So: not fifty agents. **Six clusters, each with a defined artefact, containing specialists that share an interface.** The count lands around forty because the specialists are per-analysis-type and per-standard, not because forty is a target.

### 4.1 The clusters

| Cluster | Reads | Produces | Model |
|---|---|---|---|
| **Ingestion** | manuscript, analysis files | derived layer: structure, claims, references, stats, analysis record | deterministic |
| **Methodological soundness** | derived + analysis record + manuscript (premium) | per-method findings: does the analysis support the claim | local rules + cloud judgement |
| **Reporting standards** | derived + journal | checklist findings against the standard the journal mandates | deterministic against ingested standard |
| **Journal fit** | derived + journal | scope match, length, structure, conventions, recent-paper comparison | local embeddings + cloud judgement |
| **Integrity** | derived + references | citation consistency, retraction, self-plagiarism, image reuse signals | deterministic + existing lanes |
| **Synthesis** | verdict layer only | the marked-up manuscript, the reviewer letter, the rubric score, the chat context | cloud |

### 4.2 Specialists inside a cluster

> **[v7] BEFORE THE LIST: four of these nodes have inputs and seven do not.**
> The seven are not unbuilt, they are **declined**, each against a named empty
> layer — the scientific layer (§11 D165, 5.9% precision against a 50% no-skill
> baseline) or the analysis record (no upload path). Phase 4's real shape is in
> §12; read it first, or this list reads as eleven things waiting to be built.
>
> **Shipping:** frequentist inference · machine learning · reporting standards ·
> journal requirements.

Methodological soundness is the one that fans out, because analysis types differ:

- Frequentist inference (t-tests, ANOVA, regression assumptions, multiple comparisons)
- Bayesian analysis
- Machine learning (train/test leakage, baseline fairness, reporting of variance)
- Qualitative methods (coding reliability, saturation claims)
- Survey / psychometric (reliability, validity, sample adequacy)
- Lab / wet-bench (controls, replicates, blinding as stated)
- Computational **consistency** (do the code's stated operations account for the numbers in the paper — not reproduction; the code is never executed, see §11)

Each is one specialist with the same interface: `fn assess(record: &AnalysisRecord, claims: &[Claim]) -> Vec<Opinion>`. Adding a specialist is adding a node with declared layers and tools. The harness routes by the analysis record's detected type, so a paper with no code never wakes the reproducibility specialist.

**[v7 — BUILT, and all three parts of that signature are wrong about the code. `gaply-core/src/specialist/`.]** Measured 15 Sep 2026 while building the first two:

1. **`record: &AnalysisRecord` makes the record MANDATORY, and it is absent on every real run.** No upload path in the app accepts an analysis file — every picker is `pdf, docx, txt, md` — and the researcher corpus contains **zero** `.R`, `.py`, `.sps`, `.sav`, `.ipynb`, `.csv` or `.xlsx` against 29 `.docx`. The record is a SUPPLIED artifact, so a specialist declaring it in `requires` is *not invoked*: **written to this signature, the frequentist specialist would never run at all.** It is `Option<&AnalysisRecord>`, and the specialist must have something to say without it.
2. **`claims: &[Claim]` is not enough, because of what the claims are.** With the layer on across the six real manuscripts: **123 claims, 109 of them (89%) sentence fragments** — *"higher than the untreated control."*, *"compared to unidirectional models."* — because `claims.rs:255` slices `sentence[cue_start..next_cue]` and keeps the tail. And the cross-links a methodological specialist would navigate are **hardcoded empty at construction** (`claims.rs:277-279`): claim→variable **0 of 123**, claim→method **0 of 123**, claim→dataset **0 of 123**. `SpecialistInput` therefore carries the whole `ExtractionResult`; the scientific layer is an enrichment beside it, not the substrate.
3. **`-> Vec<Opinion>` cannot be honoured, and should not be.** `Opinion` carries an `AgentKind` — a closed six-variant enum with **321 references** and a frontend wire contract. Adding forty specialists to it is the change §11 D156 already declined for `CertaintyTier`. Specialists emit `specialist::Finding`; a `swarm::adapters`-shaped mapping is the seam if the round-table ever needs one.

**The "harness routes by the analysis record's detected type" is also not the mechanism.** `run_pipeline_inner` still executes a hardcoded sequence (§12.1 item 2). What decides whether a specialist speaks is `Specialist::applies_to` — and it must exist, because four of the six real manuscripts are not ML papers and an ML specialist without a gate reports "no baseline comparison" on a silkworm paper, correctly and uselessly.

Reporting standards fan out per standard (CONSORT, PRISMA, STROBE, ARRIVE, CHEERS, SRQR, TRIPOD, MOOSE…). Each is a *deterministic* checklist evaluator over the derived layer, because the standards are checklists. No model.

### 4.3 The agent contract

Every agent, cluster or specialist, is a node in the typed graph with:

```
struct AgentSpec {
    id: AgentId,
    cluster: Cluster,
    reads: Vec<Layer>,               // enforced — reading outside is a build error
    tools: Vec<Tool>,                // enforced — a tool not listed cannot be called
    model: ModelRequirement,         // None | Local(tier) | Cloud(provider)
    emits: OpinionKind,
    may_revise: bool,                // §5 — the thing that is currently always false
    hard_constraint: bool,           // deterministic verdicts override soft consensus
    budget: TokenBudget,
    requires: Vec<Artifact>,         // e.g. AnalysisRecord — absent → agent is not invoked
    evidence_policy: EvidencePolicy, // minimum sources, permitted source types,
                                     //   citation required, uncertainty required
    timeout: Duration,
}
```

`evidence_policy` is what stops a model producing a sophisticated conclusion from insufficient evidence. A journal-fit specialist requires the guideline *and* the recent-paper corpus; a statistical specialist requires the analysis record *and* the claim it bears on; a literature specialist requires a peer-reviewed source. An opinion emitted without its policy satisfied is rejected at the gate with the reason recorded.

This is AgentFlow's DSL applied. The graph is data (a `.ron` or `.json` file), validated at build time: no cycle, every `reads` is a layer that exists, every cloud agent has a consent gate upstream, every `hard_constraint` agent has `model: None`.

**[BUILT — `gaply-core/src/agent_graph.rs` + `data/agent_graph.json`. Three corrections came out of building it.]**

**[v7] 0. `requires` needs THREE kinds, and the two below are not enough either.** Writing the first two specialists produced a case neither kind can express. A methodological specialist is BETTER with the analysis record — it can then check that a test the paper reports appears among the procedures actually run, which §1 calls the differentiator — and is perfectly useful without it, because the p-values and tests it reasons over are in the manuscript. Declared under `requires` the record is supplied-and-absent, so the specialist is **never invoked**; left out entirely, its `evidence_policy` may not permit `AnalysisRecordEntry`, so the one record-backed check is **rejected at the gate** the moment a record arrives. Both readings of a two-kind rule are wrong, which is what says the rule needs a third. `AgentSpec::optional` is it: an artifact that ENRICHES without gating. Scheduling ignores it; the evidence policy may name it. **The validator surfaced this by rejecting the first graph written** — the same way it rejected `extraction` and exposed the egress confusion below. A case the rule has to decide is what tests a premise.

**[v7] 0b. `evidence_policy` did not exist on `AgentSpec` until now**, though §4.3 has always listed it. Nor do `tools`, `budget`, `timeout`, `cluster` or `emits`: of the twelve fields §4.3 names, the built struct carried **five**. `cluster` and `evidence_policy` are now real, with two validator rules broken on purpose — a policy demanding sources while permitting no kind (unsatisfiable, so it DISABLES the agent rather than constraining it), and a policy permitting a source kind that arrives in an artifact the agent declared in neither list. `tools`, `budget`, `timeout` and `emits` remain unbuilt and are listed here so they are not read as shipped.

**1. `requires` needs two kinds, and §4.3 only has one.** It is defined here as *"absent → agent is not invoked"*, which is right for `AnalysisRecord` — the researcher uploaded code or did not, and absence is a fact about the submission. It is wrong for `ScientificExtraction`, which the harness can PRODUCE from the manuscript it already parsed. `Artifact::is_derivable` splits them: derivable-and-required → the harness computes it; supplied-and-absent → the agent is not invoked. Conflating them gives either an agent that never runs because nobody set a flag, or a harness trying to conjure an analysis record out of prose.

**2. "every `reads` is a layer that exists" is enforced by the TYPE, not by the validator.** `Layer` is a closed enum, so a graph file naming `"quantum_layer"` fails to DESERIALIZE and never reaches validation. What remains checkable is the degenerate case the type cannot express — an agent declaring no layers at all. A test records where the rule lives so nobody adds a redundant check or removes the type guarantee believing a check covers it.

**3. §3.1's privacy classes are about EGRESS, not about reading — and the first graph written against the other reading failed to validate.** `extraction` reads the whole Manuscript layer on the free tier and transmits nothing; requiring `Manuscript` consent to read it would mean the free tier needed premium consent to parse a file the user just opened. The rule is therefore: a LOCAL agent may read any layer; a CLOUD agent reading a layer whose content needs consent to leave must declare a scope COVERING that layer. `ConsentScopeName::covers` is that mapping, and a scope that exists but does not cover the layer is rejected — the case a bare `is_some()` check waves through.

**The validator was built before the graph, and each rule was broken separately**: a two-node cycle and a three-node cycle, a missing dependency (asserted NOT to be reported as a cycle — different fixes, different places to look), a cloud agent with no gate, a gate on a local agent, a hard-constraint agent with a local model and with a cloud model, an agent reading nothing, and a cloud agent whose scope does not cover what it reads. Each asserts the REASON, because a validator reporting `Cycle` for a missing gate would pass a bare `is_err()`. Every violation is returned, not the first, so fixing a graph is not an N-round game.

**The scientific layer is now a dependency rather than a flag.** `graph.requires_scientific_extraction()` decides whether the pipeline derives it, so a lane that reads nothing pays nothing and there is no list to maintain. It is deferred rather than passed as `ExtractOptions::scientific`, because which agents are scheduled can depend on what extraction found — the decision cannot be made before extraction runs, and re-running with the flag set would pay the base cost twice. No shipped agent declares it yet, pinned by a test that is where switching it on becomes deliberate.

**The six lanes are described by the graph, and the description is pinned against reality** — `the_graphs_order_matches_the_pipelines_lane_order` fails if the graph and `run_pipeline_inner` disagree, because a graph that describes something that does not happen is worse than no graph when the next phase routes over it. The report stayed **byte-identical** against `tests/fixtures/report.golden.json` through the migration.

**TWO THINGS THIS DID NOT DO, stated plainly because the phrase "moved onto it" does lighter work than it sounds.**

**The lanes are DESCRIBED by the graph, not DRIVEN by it.** `run_pipeline_inner` still executes its six lanes in a hardcoded sequence. What the graph supplies today is the declaration of record and the derivable-artifact decision; the order is pinned against the executor so the two cannot diverge silently, which is a guarantee about agreement, not about control. **A graph-driven executor is a separate change, and the golden test cannot cover it** — byte-identity proves the report did not move, not that the mechanism producing it is the one the graph describes. That change needs its own instrument.

**The cloud rule is enforced in two halves, and only one of them runs.** The validator checks at build time that a cloud agent DECLARES a scope covering what it reads. It cannot check that the running harness holds a `Tier::Premium(record)` whose stored scope actually covers what was sent — that is `run_premium_gate`'s job, and **`run_premium_gate` still has no production runner**. So §4.3's *"every cloud agent has a `Tier::Premium` gate upstream"* is today a static declaration plus a runtime gate that nothing in production invokes.

### 4.4 Trust tiers

Every output carries a tier. The tier decides what may override what, and the report shows it.

| Tier | What | Decided by | Overrides |
|---|---|---|---|
| **0 — Deterministic** | arithmetic, equation equivalence, N consistency, table totals, CI/SE/SD recomputation, duplicate references, unit checks | symbolic and numeric computation | everything above it |
| **1 — Rule** | CONSORT, PRISMA, STROBE, ARRIVE, TRIPOD, journal requirements | checklist evaluators over the research state | tiers 2–4 |
| **2 — Evidence reasoning** | does the evidence support the claim; is the method appropriate; is causal language earned | LLM with `evidence_policy` satisfied | tiers 3–4 |
| **3 — Expert judgement** | novelty, likely reviewer concern, contribution, fit | multiple specialists + evidence | tier 4 |
| **4 — Synthesis** | the editorial reading | synthesis cluster | nothing |

**Deterministic evidence outranks model consensus.** Eight agents agreeing an equation is correct is not evidence about the equation. The existing `hard_constraint` flag is Tier 0 and Tier 1; this table is the general form.

**[v8 — THE BLOCKING TIER HAS FOUR CODES AND TWO OF THEM CANNOT FIRE. Read four as the count and the tier looks twice as wide as it is.]**

`review_lens::SEVERITY_POLICY` maps four finding codes to `Blocking`:

| code | reachable? |
|---|---|
| `TestGroupMismatch` (`validate.rs`) | **yes** |
| `SmallSampleCausalClaim` (`validate.rs`) | **yes** |
| `test_claimed_but_absent_from_analysis` | **no** — it compares the paper's reported tests against an uploaded `AnalysisRecord`, and no picker accepts an analysis file. The parser and the record type both exist; the upload path is the missing half |
| `PRIOR_WORK_EXISTS` | **no** — the novelty pipeline is declined (§11 D166) |

**So the tier that outranks everything rests on two deterministic statistical
rules.** Measured across 20 real manuscripts it fired **twice**, both
`SmallSampleCausalClaim`, on 2 of 20 papers. `Blocking: 0` therefore means those
two rules did not fire — **not that nothing fatal is present**, and the rubric
prints that sentence every time rather than leaving the reader to infer it.

This is the same treatment the declined lenses get and for the same reason: a
count that includes unreachable members overstates coverage, and the two
unreachable codes have different reopening conditions — an upload path for the
first, and D166's corpus measurement for the second.

### 4.5 Reviewer lenses

Specialists produce opinions. Reviewers produce *reports*. The attached reviewer-criteria document describes what a Q1 reviewer actually evaluates, and it does not map one-to-one onto specialists — a methodology reviewer reads the statistics specialist's output *and* the design specialist's *and* the ethics evaluator's, and forms a view.

So: not six more agents. A `ReviewLens` is a perspective applied *over* specialist outputs:

```
struct ReviewLens {
    id: LensId,                      // methodology | statistics | novelty_literature
                                     //   | journal_fit | reporting_ethics | general
    criteria: Vec<Criterion>,        // from the reviewer-criteria document, each with
                                     //   strong / weak / action as that document states them
    reads: Vec<SpecialistId>,        // which specialist outputs this lens sees
    evidence_policy: EvidencePolicy,
    severity_policy: SeverityPolicy, // the risk table: fatal / fixable / minor per criterion
    journal_context: bool,           // whether this lens reads the JournalFingerprint
}
```

The six lenses and their criteria, taken from the document:

| Lens | Evaluates |
|---|---|
| **Methodology** | study design, controls, sampling, randomisation, blinding, reproducibility & data availability, ethical compliance |
| **Statistics** | test choice, assumptions verified, effect sizes and CIs reported, multiple-comparison correction, power, selective reporting |
| **Novelty & literature** | what is claimed as new, against retrieved prior work; coverage, currency, contradictory evidence addressed, self-citation proportion |
| **Journal fit** | scope match, comparable-corpus position, audience relevance |
| **Reporting & ethics** | mandated standards met, declarations present, formatting, word limits, reference style, supplementary use |
| **General** | title/abstract/keywords accuracy, clarity and structure, overstated conclusions, presentation quality |

Each lens produces an independent **reviewer report** in the shape a real one takes: overall assessment, contribution summary, strengths, major concerns (each with its verification trail), minor concerns, journal-specific compliance, required revisions. Lenses run in parallel and do not see each other's reports until the disagreement map (§5.3).

**[v8 — `reads: Vec<SpecialistId>` IS THE DEFECT, AND IT IS THIS SECTION'S, NOT THE IMPLEMENTATION'S. `gaply-core/src/review_lens.rs`.]**

This is the same shape as §6b.1's gating item and §3.4's discovery rule: a
correction to what this document specified, arrived at by measuring, not a note
about how the code happened to be written.

**The specification points the lenses at the one layer that is empty.** Measured
15 Sep 2026 against §12's Phase-4 table: of eleven specialists, **four ship and
seven are declined** — five against the scientific layer (§11 D165, 5.9%
precision against a 50% no-skill baseline) and one against the absent
analysis-file upload path. Built to the signature above, the six lenses read
**7 shipping sources, 2 declined, 4 unbuilt**, and the arithmetic came out:

| lens | criteria with a shipping SPECIALIST |
|---|---|
| reporting & ethics | 5 of 5 |
| methodology | 3 of 3, every one on a partial source |
| statistics | 2 of 2 |
| novelty & literature | 1 of 3 |
| general | 1 of 3 |
| journal fit | **0 of 2** |

**And the layers UNDERNEATH are populated.** `ResearchState` carries sections,
statistics, citations, references and tables on every run; the
`JournalFingerprint` carries 41 stored requirements and 15 bindings from a real
Nature Medicine crawl. The emptiness is at the specialist tier only. A lens
restricted to specialist output is starved beside a full larder.

**So the correction: a lens reads the research state and the fingerprint
DIRECTLY.** `reads: Vec<SpecialistId>` becomes `reads: Vec<LensSource>`, where a
source is a specialist report, a research-state field, or a fingerprint field.
Three criteria the reviewer document names were reported as unsourced under the
old rule and are decidable under the new one without a single new extractor:
reference currency (every `Reference` carries a `year`), ethics disclosure and
data availability (both are phrases in the full text).

**WHAT THIS COSTS, AND WHY D157 IS THE PRECEDENT.** A lens reading the research
state directly is doing what a specialist would have done, and it has no
`applies_to` and no specialist author standing between it and the manuscript.
The `evidence_policy` is therefore the *only* thing between a lens and exactly
the fabrication §11 D165 declined the scientific layer over.

D157 is the precedent because it is the same problem solved once already. The
equation binder reads a lower layer directly — the manuscript's own symbols —
and across nine documents produced **2 bindings, 76 refusals, 1 finding**. It
refuses by default and binds only where the document itself settles the
question, because one manuscript declares `N` with five different values and a
document-wide symbol table would bind the wrong one and then disprove correct
arithmetic. A lens over the research state needs the same ratio and the same
default.

**The rule that gives it one: every field a lens reads declares what its ABSENCE
means, and the policy rejects a finding that rests on the wrong kind.**

* **Exhaustive** — the lens searched every word of the manuscript. `docparse`
  produces the text and nothing classifies it, so a miss can only be a lexicon
  miss, which the finding's `uncertainty` must state. *"No data availability
  statement found"* is a finding.
* **Mediated** — the lens searched a projection the extractor built, and the
  projection can omit what the manuscript contains. *"No methods section
  extracted"* is **not** a finding: the extractor is the reason, and the
  manuscript may be perfectly fine.

**Measured over the six real manuscripts, and the two kinds do not behave alike
at all** (`examples/absence_scan.rs`):

| mediated field | empty in | and the content is plainly there |
|---|---:|---|
| `Conclusion` section | **5 of 6** | every one of them concludes; `classify_heading` accepts three spellings |
| `Results` section | 3 of 6 | |
| `Discussion` section | 3 of 6 | |
| `references` | 2 of 6 | both of those carry 15 and 165 in-text citations |
| `title` | 2 of 6 are not titles | a Turnitin cover page; an AI-detector banner |

| exhaustive field | found in | adjudication |
|---|---:|---|
| data-availability phrases | 1 of 6 | correct — only the health-economics paper has one |
| ethics phrases | 1 of 6 | correct — *"ethical approval"*, *"declaration of helsinki"* |
| randomisation phrases | 1 of 6 | correct — the randomised-block silkworm experiment |

A lens allowed to conclude from a mediated absence would have reported **five of
six manuscripts as having no conclusion**. Restricted to exhaustive fields it
got all three of its absence questions right on all six. The rejection is
returned with its reason, never dropped, for the reason `specialist::run`
already returns its own.

**FOUR LENSES SHIP, AND TWO ARE DECLINED FOR DIFFERENT REASONS.**

| lens | state |
|---|---|
| **Methodology** | SHIPS — design terms, ethics and data-availability statements, all exhaustive |
| **Statistics** | SHIPS — `statistics`, the frequentist specialist, `validate.rs` |
| **Novelty & literature** | SHIPS — novelty claims, and reference currency from `Reference::year` |
| **Reporting & ethics** | SHIPS — standards, and the journal's own requirement rows |
| **Journal fit** | **DECLINED — the layer is empty.** `derive_conventions` and `PublishedPaper` exist, the OpenAlex fetch that fills them is in the APP crate where `gaply_core` cannot reach it, no `journal_papers` table exists in `migrations.rs`, and the only in-core caller is a test with twenty fabricated papers. Scope is not extracted either: `RequirementKind` has nine variants and none is scope. |
| **General** | **DECLINED — the rule above forbids it.** Its criteria are *clarity and structure* and *title/abstract/keywords*, and both are read from mediated fields: section presence (the Conclusion heading missed 5 of 6) and `title` (not a title in 2 of 6). Every finding it could make is one the absence rule rejects. Its third criterion, *overstated conclusions*, has a real input and moves to Methodology, where §4.4 already puts "is causal language earned" at Tier 2. Prose quality is a Tier 3/4 judgement and belongs to the synthesis cluster, not to a deterministic lens. |

The two declines are not the same kind and the table says which is which: one is
a missing data path that a corpus job would fill, the other is a criterion this
tier should not evaluate at all.

**Two smaller corrections from the same build.**

`severity_policy` is described above as *"the risk table: fatal / fixable /
minor per criterion"*. The reviewer document's third column is *Low Risk (Minor
Issue)* and **every one of its six cells describes the GOOD case** — *"Robust
design with adequate power"*, *"Fully compliant with instructions"*. Read as
specified, each criterion carries a minor concern whose text is a compliment.
It feeds `strengths`. The severity vocabulary is §8's four levels, not three.

*"contribution summary"* cannot be produced here. Summarising what a paper
contributes is §4.4's Tier 4. The field states what is visible and names the
absence rather than asserting a judgement nothing made.

### 4.6 Novelty, significance and claim strength — three separate outputs

The reviewer-criteria document puts novelty and significance first. The architecture makes them explicit pipelines rather than a judgement score, and keeps them apart because a paper can be highly novel and low significance or the reverse.

**Novelty, claim by claim.** Extract every novelty claim from the research state (*"first to demonstrate X", "no prior study has…"*). For each, retrieve the closest prior work through the external-evidence layer. Compare. Output per claim: the claim, the nearest prior work with what it showed and under what conditions, and a status — `NOVEL_AS_STATED`, `NOVELTY_NARROWER_THAN_STATED` (with the narrowing), `PRIOR_WORK_EXISTS` (named), `UNVERIFIED` (retrieval found nothing decisive). *"This is the first study to demonstrate X" → Study A (2024) demonstrated X under condition Y → novelty claim requires revision.* Never a number.

**[v8 — DECLINED. §11 D166. The paragraph above assumes a kind of sentence this corpus does not contain.]**

It reads as though novelty claims are a normal feature of a manuscript —
*"extract EVERY novelty claim"*, with a worked example in the present tense.
Measured, they are rare and, where present, hedged.

**A 31-cue scan over 20 real research manuscripts — 37,557 sentences — found 12
candidate sentences. Two are claims the paper makes about its own contribution.
That is 0.1 per manuscript.** The cue list is far wider than the two phrasings
this section names, including `unprecedented`, `novel approach` and
`little is known`.

| | 20 manuscripts |
|---|---:|
| candidate sentences | **12** |
| per manuscript | **0.6** |
| surviving adjudication as claims about THIS study | **2** |
| manuscripts containing one | **2 of 20** |
| `ClaimCategory::NoveltyClaim`, a second independent extractor | **2 of 528 claims** |
| containing *"first to demonstrate"* or *"no prior study has"* | **0 of 20** |

Ten of the twelve are not novelty claims: five are `unprecedented` describing a
market or a period of history, one is *"for the first time"* about the study's
SUBJECTS, one is a thesis originality declaration, one is a recommendation.

**And the two survivors are both `UNVERIFIED`** — retrieval returned 20 works
with a highest term overlap of 3 of 7, on generic terms. So the worked example
above, *"Study A (2024) demonstrated X under condition Y"*, did not occur once in
20 manuscripts. **The worked example is a form of sentence that is not in this
corpus**: `"first to demonstrate"` and `"no prior study has"` appear in none of
them.

**What this section can honestly claim** is narrower and needs no retrieval:
where an author DOES assert unrestricted priority, say so as a PHRASING
observation — the claim names no population, setting or task, and therefore
asserts priority over the whole literature — and do not present that as a finding
about whether the claim is true. That is `novelty_claim_states_no_scope`, a minor
concern under the Novelty & Literature lens.

D166 has the corpus, the instrument, the row-by-row adjudication, and a
reopening condition whose two halves are ordered: **measure whether authors write
these sentences before rebuilding the extractor that would find them.**

**The rest of this section is unaffected.** *Significance* was always Tier 3 and
cloud, and is untouched. *Claim–evidence strength* reads concluding sentences and
a design field, not the novelty category, and ships — see below.

**Significance, separately.** What the contribution changes if true: field-level, practical, theoretical. Tier 3, cloud judgement, evidence-policy requires the comparable corpus so the judgement is relative to what the journal publishes.

**Claim–evidence strength, on every major claim.** The evidence graph traced claim → analysis → result → conclusion, classified `SUPPORTED · PARTIALLY_SUPPORTED · UNSUPPORTED · CONTRADICTED · UNVERIFIED`. The specific check the reviewer document names: **causal overclaim** — the study supports *associated with*, the conclusion says *causes*. That is a Tier 2 finding with the sentence and the design that limits it both cited.

**[v8 — SHIPS, and only because it does NOT read the claims. Measured before the report shape was chosen.]**

*"On every major claim"* names the claim extractor's output as the input. Asked
what fraction of that output this check can classify
(`examples/claim_classifiable_audit.rs`), the answer is none:

| | six manuscripts | twenty manuscripts |
|---|---:|---:|
| claims produced | 123 | 528 |
| whole sentences, not fragments | 10 (8.1%) | 47 (8.9%) |
| + in a concluding section | 2 (1.6%) | 11 (2.1%) |
| + asserting causation | **0** | **0** |
| + with a design to weigh against | **0** | **0** |

**Scoped as this section writes it, the check classifies 0 of 528.** The 8–9%
whole-sentence figure is `claims.rs` slicing from its cue to the next one and
keeping the tail — the same defect §4.2 measured at 89% fragments.

**What ships reads concluding SENTENCES and the design field**, both from
`ExtractionResult`, and that input is real: **17 of 20 manuscripts state a design
this check recognises**, and the corpus yields 6 causal sentences across 20
papers. On it: 2 association-only designs, 0 findings, and every zero
adjudicated as correct — the cross-sectional papers make no causal claim, and
`Revised Health Economics Paper FINAL (1).docx` writes *"Cross-sectional design:
No causal direction can be established."*

So the same sentence that ends D165 applies here: declining the claims layer
costs this check nothing, because it was never scoped to it. `SUPPORTED` and
`CONTRADICTED` remain unreachable for the separate reason in the module header —
Phase 1 built the evidence graph to emit co-location and refused to assert
meaning, and reading co-location as support would reverse that silently.

**THE TRACING, MEASURED** (`examples/claim_evidence_audit.rs`, 20 manuscripts,
528 claims). §4.6 asks for claim → analysis → result → conclusion. The evidence
graph emits exactly one claim→result edge, `ClaimCoLocatedWithStatistic`:

| | claims | share |
|---|---:|---:|
| traced to a result | **65** | 12.3% |
| `SUPPORTED` / `CONTRADICTED` | **0** | unreachable — the edge is co-location |
| `PARTIALLY_SUPPORTED` | 65 | 12.3% |
| **`UNVERIFIED`** | **463** | **87.7%** |

**A per-claim report over this input is 463 rows saying "we could not check
this."** That is the honest state and it is NOT a reason to loosen the tracing:
the 87.7% is what an extractor that sees position and not meaning can say, and
the way to move it is a real claim↔analysis link, not a weaker bar for calling
something traced. Recorded here so the number is the thing anyone proposing to
improve it has to beat.

So the shipped output is **the causal-overclaim finding only**, and the per-claim
distribution stays a measurement rather than a report section.

**THE CAUSAL-OVERCLAIM CHECK NEEDS BOTH HALVES, AND BOTH ARE COMMON — THE
CONJUNCTION IS NOT:**

| over 20 manuscripts | |
|---|---:|
| state a design that admits causation | 15 |
| state an association-only design | 2 |
| state no design this check recognises | 3 |
| make ≥1 causal conclusion (12 raw phrase matches, **6** after the disclaimer, idiom and `X-induced` guards) | 5 |
| **both halves — association-only AND a causal conclusion** | **0** |

**This is why the check ships where novelty was declined, and the difference is
the shape of the scarcity.** Novelty's input was itself rare — 0.1 real claims
per manuscript (§11 D166). Here both inputs are ordinary: 17 of 20 manuscripts
state a design and 5 make causal conclusions. What does not occur in these 20 is
the *pairing*, and a check that stays silent because the defect is absent is
working. The two association-only papers make no causal claim at all, and one
says so outright — *"Cross-sectional design: No causal direction can be
established."*

**What that costs, stated plainly: the check has never fired on real input.** Its
precision on this corpus is not 4/4 or 0/4, it is undefined, and only a
constructed positive control shows it can fire at all. That is a weaker claim
than the frequentist specialist's and is reported as such.

---

## 5. The harness

The current `run_debate` is a real mesh round-table with a sound design: initial opinions, internal-gate filter, up to three rounds of visible revision, confidence-weighted consensus, hard-constraint override. It has never run past round one because nothing can revise.

### 5.1 Fix the existing thing first

`RevisingVerificationAgent` exists, is tested, and is constructed only in tests. Before any new agent:

1. Wire it into `pipeline.rs` in place of the precomputed verification participant.
2. Run the golden manuscripts. Confirm `rounds_run > 1` and `revised_agents` non-empty on at least one.
3. Pin it: a test that fails if the production pipeline ever again converges in round one on a manuscript where the fixture agent is designed to revise.

Until that test is green, the debate is a vote and every architectural claim about it is a `doc` claim.

**[BUILT — and the "run the golden manuscripts" step could not be done as
written.]** `pipeline.rs` now puts a `RevisingVerificationAgent` in the
verification slot on the consent-granted path; the other five stay precomputed
deliberately (`revising.rs`'s design boundary: their outputs are MEASUREMENTS,
and a validator that changed its answer under peer pressure would be broken, not
collaborative).

Three things the wiring needed that §5.1 does not mention:

- **The proxy has to outlive the lane.** It was built inside the verification
  lane's closure and dropped there. It now leaves the lane, and `unload_slm2()`
  moved to *after* the debate — the reconsideration is a SECOND proxy call, so
  unloading before it would force a mid-debate reload.
- **The revised report has to come back.** `compile_report` builds the
  per-citation findings from the verification report, so a caller holding the
  pre-debate copy would render findings that contradict the `revised_agents`
  summary printed beside them. `SwarmAgent` gained a blanket impl for `&mut T`
  so the pipeline keeps ownership, and the agent gained `into_parts()`.
- **Step 2 — "run the golden manuscripts, confirm `rounds_run > 1`" — is not
  hermetically possible.** Revision needs a verification report with real
  citation ids, which needs `items`, which the lane only fills by calling
  CrossRef/OpenAlex per reference. So the pin is SPLIT: one test proves the
  mechanism against the pipeline's own five precomputed peers, one proves the
  wiring by reading the source. Each says in its own docs what it cannot see.
  Both were broken on purpose; the mechanism break reproduces exactly
  `rounds_run: 1, revised_agents: []`.

### 5.2 Evidence outranks consensus

Eight agents saying *minor* and two saying *major* is not a majority decision if a deterministic SPSS mismatch says *major*. The existing `hard_constraint` override already encodes this for the statistical validator; it generalises. Consensus weighting applies only among judgement opinions; any evidence-backed deterministic finding on the same claim sets the floor.

### 5.3 Disagreement-driven rounds

The current mesh has every agent reconsider everything each round. Instead:

```
independent opinions → disagreement map → targeted re-analysis → adjudication → verdict
```

The disagreement map identifies which claims have opinions that differ by more than a threshold, and *only those* go to a second round, with the disagreeing agents shown each other's evidence. An adjudicator — a synthesis-cluster agent with the full verdict layer — resolves what remains. Fewer tokens, and the revision that happens is the revision that matters.

### 5.4 Orchestration shape

ChatDev's phases over MetaGPT's artefacts, with AutoGen's topology made explicit:

```
Phase 1  INGEST      ingestion cluster, deterministic, always local
Phase 2  ASSESS      methodological + reporting + integrity + fit, in parallel
                     (each cluster's specialists in parallel within it)
Phase 3  DEBATE      the existing mesh round-table, now with revising agents
Phase 4  SYNTHESISE  synthesis cluster over the verdict layer
Phase 5  RENDER      marked-up manuscript, letter, rubric, chat context
```

- Phase 2 is where the fan-out is. Clusters are independent; specialists within a cluster are independent. This is the parallelism the current sequential six-lane pipeline lacks.
- Phase 3 is the current `run_debate`, unchanged in design. What changes is that specialists are `may_revise: true` where a revision is meaningful (a methodological specialist seeing the reproducibility specialist's finding that the code doesn't produce the paper's number *should* revise its confidence).
- The hard-constraint override stays exactly as it is. A deterministic checklist failure is not up for debate.

### 5.5 Incremental re-analysis

The graph is a dependency engine. When a researcher records a decision (§10) or edits the manuscript, the harness computes which research-state fields changed, which agents read those fields, and re-runs only that subgraph. `N = 624 → 600` re-runs statistics, tables, results and the assessments that depend on them — not journal fit, not integrity. Every node's `input_hash / context_hash / model_version / output_hash` is what makes this decidable.

### 5.6 Failure and degradation

The teardown praised this and it stays: every lane degrades, nothing aborts. Extended:

- A cloud agent that cannot reach the proxy emits `Opinion::Unavailable(reason)` — a first-class value the synthesis phase renders as *"not assessed: [reason]"*, never as absence.
- A specialist whose declared layer is empty (no analysis record) is not invoked and does not appear.
- The existing `"Verification output rejected by its internal gate"` finding moves to a *harness log* the chat can surface if asked, and out of the findings list. **[v5 — half of this is already done, and the half that remains is the smaller one.]** v4 said it is "shown to users as though it were about their paper." It is shown — in **20 of 22** stored reports, as a `Minor` — but it no longer counts toward anything: `ClaimKind::ProcessState` exists and `reviewer_agent.rs:1096` excludes it from the recommendation, recording an `ExcludedFinding` with the reason. So the remaining work is purely surfacing, not verdict influence, and the same is true of AI-detection's `AuthorshipSignal` (`:1092`).

### 5.7 The editor layer

Reviewers and editors are different roles with different outputs, and the reviewer-criteria document's workflow — editorial screening → peer review → editorial decision — is the shape. The synthesis cluster becomes an explicit **editor**:

```
reviewer reports (§4.5) → disagreement map → targeted re-analysis → adjudication
    → EDITOR: scope · severity precedence · priority → editorial posture
```

The editor reads all reviewer reports, the disagreement map, the journal fingerprint, and Tier 0/1 findings. It applies **severity precedence** from the risk table — a fatal flaw (missing required ethics approval; an arithmetic contradiction in the primary result; a design with no control) is blocking regardless of how many minor strengths surround it — and produces the editorial posture (§8). A reviewer says *"I am concerned that…"*; the editor says *"given these concerns and this journal's scope, major revision, for these three reasons."*

The editor is Tier 4 and overrides nothing below it. It orders and frames; it does not re-decide a Tier 0 finding.

**[v8 — BUILT, and §5.7 names four inputs of which the editor reads three. `gaply-core/src/editor.rs`.]**

| §5.7 says it reads | measured |
|---|---|
| all reviewer reports | **real** — four lenses (§4.5 [v8]) |
| the journal fingerprint | **real** — from a crawl |
| Tier 0/1 findings | **real** — `validate.rs`, the equation engine |
| **the disagreement map** | **nothing produces one** |

**The disagreement map does not exist, and this is a correction to this section
rather than a note about the code.** §5.3 specifies it — *"identifies which
claims have opinions that differ by more than a threshold, and only those go to
a second round"* — and §5.3 is unbuilt: the round-table has never run past round
one, because until `RevisingVerificationAgent` shipped nothing could revise, and
nothing since has produced a map. `EditorInput` therefore has no field for it,
and every rubric names it in `not_read` so a reader is not left assuming the
editor weighed a disagreement it never saw.

**Severity precedence does not need it, and saying why matters.** Precedence is
a function of the severities already attached to concerns: the most severe tier
with any member decides, and nothing below it softens the result. That is
computable from the four reports alone, and `one_blocking_finding_outranks_a_hundred_major_ones`
is the test. **So the editor's ordering job is whole.**

**WHAT IS LOST IS NOT THE ORDERING — IT IS THE STEP BEFORE IT.** §5.7's own
pipeline is *"reviewer reports → disagreement map → targeted re-analysis →
adjudication → EDITOR"*. The map exists to decide **what gets re-analysed**:
without it there is no targeted second round, so every concern reaching the
editor is a first-pass concern that no second agent has examined. Concretely:

* **Nothing is adjudicated.** Two lenses reaching opposite conclusions about the
  same sentence would both arrive, and the editor would count both. It cannot
  today, because the four lenses partition the finding codes — `every_shipped_finding_code_is_claimed_by_exactly_one_criterion`
  enforces exactly one owner per code — so **disagreement is currently
  impossible by construction, not resolved**. That is why its absence costs
  nothing yet, and it is also the reason the absence is easy to miss.
* **Nothing is re-examined.** §5.3's economy — *"the revision that happens is the
  revision that matters"* — buys nothing when no revision happens at all.

The map becomes load-bearing the moment two lenses can speak to one code, which
is the moment the code-ownership test above has to be relaxed. Until then the
editor reads three of four inputs and says so.

### 5.8 Model scheduling

The 8 GB constraint is real. The harness owns it:

- Local model loads are serialised. Phase 2 specialists that need the local model queue for it; those that are deterministic or cloud run alongside.
- Cloud calls are batched per cluster where the provider allows, to reduce round-trips.
- `unload_slm2()` and the scoped model drop stay; they become harness policy rather than per-lane discipline.

---

## 6. Models — which does what

The teardown corrected the vocabulary: there are no bespoke models, no adapters, no training pipeline. "SLM-1" and "SLM-2" are stock Qwen with prompt engineering. Premium keeps that honest and uses each where it measured well.

| Role | Model | Justification |
|---|---|---|
| Deterministic computation | none | rules, checklists, embeddings, consistency |
| Local classification, low stakes | Qwen2.5-0.5B (bundled) | fast pre-pass, already measured for AI Check |
| Local judgement | Qwen2.5-3B (on demand) | *only* where its measured accuracy is stated in the report beside the finding |
| Cloud judgement | OpenAI via proxy | methodological assessment, fit narrative, synthesis, letter |
| Deep research on the journal | OpenAI with search, via proxy | **journal layer only** — recent papers, scope, conventions. Never the manuscript. |
| Chat | OpenAI via proxy, verdict layer only | §10 |

**Deep research is pointed at the journal, not the manuscript.** That is the design move that reconciles "use OpenAI's deep research" with "data privacy is important." The manuscript goes to OpenAI once, for review, under consent. The *research* — what does this journal publish, what do its reviewers expect, what changed in its guidelines — is about public information and can be as deep as the budget allows without touching the manuscript at all.

**The 3B stays measured or stays out.** **[v5 — corrected]** v4 wrote *"its shipped tasks measured 0.34 and 0.20–0.40."* Neither number is traceable to any record. The figure that exists is `citation_need`'s **18.3% weighted against an 18.0% no-skill baseline** (§11 D128) — and that lane is retired, so it is not a shipped task. The honest statement is that **the 3B's shipped accuracy on PublishReady tasks is unmeasured**, which is a stronger reason for the rule than a wrong number would have been. Premium does not put a 3B verdict in front of a researcher without a number beside it; where cloud judgement is available, the 3B is a pre-filter, not a verdict.

**[v5]** Note also that the table above names the 0.5B and the 3B, and those are the *generative* tiers. The perplexity path the AI-detection lane uses is a different ladder — `DeepTier::{Full7B, Mini, HeuristicOnly}` (Qwen2.5-7B / 1.5B / frequency proxy, `models/mod.rs:446-456`) — and it is the one producing the `major` findings §0.1 names. A model table that omits it describes half the models actually running.

---

## 6b. The mathematical verification engine

**LLMs reason about mathematics. Symbolic computation verifies it.** This is a hard constraint, not a preference. No agent is the final authority on whether two expressions are equivalent, whether a reported statistic follows from its inputs, or whether units balance.

### 6b.1 The `EquationGraph`

The evidence graph (claim ↔ analysis ↔ result ↔ table ↔ source) is extended with mathematical provenance:

```
Method → Equation { variables, parameters, assumptions, units }
       → Analysis record { inputs actually used }
       → Numerical result
       → Table cell / figure / manuscript statement
```

**[v5 — corrected: there is no upstream for this yet.]** v4 said equations are extracted "from LaTeX, OMML (the .docx exporter already round-trips it), MathML and PDF." The OMML claim is wrong in direction and in location:

- `src/screens/notes/manuscriptOmml.ts:327` `texToOmml()` is a **frontend TypeScript generator**, LaTeX → OMML, one direction, for the Notes writer's export. It is not a round trip.
- There is **no OMML reader anywhere**. `gaply-core/src/extract/docparse.rs` extracts `<w:t>` text only — no `m:oMath` handling. Math in an uploaded `.docx` is flattened or dropped before anything sees it.
- The code that does exist sits on the side §11 declares must never do extraction.

So equation extraction is **net-new work on the ingest path**, and it is the gating item for this whole subsystem: the `EquationGraph` has nothing to build from until the ingest path can read equations. Equations, once extracted, are canonicalised and stored as nodes with their variables bound to research-state fields where the binding is unambiguous.

**[v6 — measured 14 Sep 2026, and it moves the gating item. The corpus does not write its mathematics in OMML.]** `gaply-core/examples/equation_survey.rs` and `textmath_scan.rs` run the real `docparse` over the six manuscripts:

| | OMML (`m:oMath`) | equation-shaped lines surviving `docparse` |
|---|---:|---:|
| chapter3 .docx | 0 | 16 |
| final final L.pdf | — | 3 |
| IJAS … haemolymph.pdf | — | 0 |
| Lake Chapter 1.docx | 0 | 0 |
| R PAPER .docx | 0 | 0 |
| Revised Health Economics FINAL .docx | 0 | 2 |
| **total** | **0** | **21** |

**21 equation-shaped lines reach the parser intact; zero lines of OMML exist in any of the six.** No `w:object`, no `EQ` field, no `word/embeddings/` either, across all 17 real `.docx` on the machine. What researchers in this corpus actually type is linear text, units included — `DO (mg/L) = (Vtitrant × N × 8000) / Vsample`, `Chlorophyll a (mg/L) = (12.7 × A₆₆₃) − (2.69 × A₆₄₅)`, `Weighted provision = (0.108 × 0.78) + … = 21.9%`.

So **the gating item is a LINEAR-TEXT equation parser**, not an OMML reader. It is the only thing that unlocks any equation in the named corpus, and it is what the dimensional check needs: the units are written in the prose (`mg/L`) and are not in the OMML at all.

**The OMML reader is a smaller, later piece, and its priority is a CORRECTNESS argument rather than a coverage one.** It covers 3 of 17 documents (~18%) — see the fix recorded below.

**[v6 — the "flattened or dropped" above was half right, and the wrong half was shipping.]** Measured, not read: `parse_docx` matched elements by `local_name()`, which discards the namespace, so `<m:t>` fell into the `<w:t>` arm. Math was **flattened, never dropped** — and flattening is the worse failure. `n = N/(1+Ne²)` came out `n=N1+Ne2`; `n = 237,000/(1+379.2) = 623.36` came out `n=237,0001+379.2=623.36`. **`237,0001` is a number that is not in the manuscript**, in the text stream the statistic extractor, the AI-detection lane and the plagiarism lane all read. A dropped equation is a hole; this was fabricated data shaped like data.

The same collapse had a second victim nobody was looking for. `<w:tab/>` in a run is a tab; `<w:tab w:val="left" w:pos="720"/>` inside `<w:pPr><w:tabs>` is a **ruler setting** that produces no text. The old loop fired on both. `R PAPER .docx` — the instrument most of §11's numbers were taken on — has **21 tab-stop definitions and zero genuine tab runs**, and the parser emitted exactly 21 tabs, each at the head of a heading with no tab (`"\tRelated Work"`). `Jitesh Agarwal .docx`: 493 of 986 fabricated. `Disha Correction .docx`: 145 of 284.

Fixed by resolving namespaces (`NsReader::read_resolved_event`) instead of matching prefixes as strings, and by refusing to emit content from inside OOXML property containers. An `m:oMath` now becomes `docparse::EQUATION_PLACEHOLDER` (`[equation]`) when flattening would lose structure, and keeps its text only when the element is a bare run sequence, where flattening is provably lossless — so `Where: N = target population` stays readable while `n=N1+Ne2` cannot occur. Seven pins guard it, and each was **confirmed to go red** against a deliberate reinstatement of its own defect: the fabricated-digit pin reproduces `n=237,0001+237,000(0.04)2` verbatim, and the namespace pin is caught by nothing else. After the fix, `R PAPER` is exactly −21 characters and every other line of the six is byte-identical.

### 6b.2 What it checks, deterministically

- **Symbolic equivalence** — the same quantity written two ways in two places is recognised as the same; a sign or denominator that differs is flagged.
- **Dimensional consistency** — units on both sides of every equation, and on every variable that carries them.
- **Numerical substitution** — where the analysis record supplies inputs, recompute the reported statistic and compare. *Manuscript reports t = 3.43. Analysis record gives mean 24.8, μ₀ 20, s 8.4, n 36; computed t = 3.43.* → `CONFIRMED`. *Analysis record reports t = 2.91.* → `DETECTED — numerical inconsistency, difference 0.52, requires author confirmation`.
- **Formula validation** — standard statistics (SE, CI, effect sizes, p-values where the test statistic and df are present) recomputed from reported inputs.
- **Equation ↔ text ↔ code ↔ result consistency** — the same quantity across all four, with any disagreement located.

Every one of these is Tier 0. The output is a finding with `EpistemicStatus`, evidence pointers to both sides of the disagreement, and a reviewer verification trail. An LLM may *explain* the finding in the report. It does not decide it.

**[v5 — two of these five are already built, and v4 did not say so.]** `gaply-core/src/stats_verify.rs` is a deterministic recompute engine with no model, no proxy, no network and no I/O: Welch's and Student's *t*, one-way ANOVA, Pearson and Spearman, χ² of independence, and OLS with standard errors — every result validated against scipy/R reference values in its own test module. That is **numerical substitution** and **formula validation** already shipping. Rebuilding them would waste the work and, worse, create a second numeric authority for the same quantities. What is genuinely new here is **symbolic equivalence**, **dimensional consistency**, and the **equation extraction** that feeds all of it.

### 6b.3 What it does not do

**[v6 — the dimensional check is largely unreachable on real manuscripts, and §6b.2 implies otherwise.]** §6b.2 asks for *"units on both sides of every equation"*. Measured (`examples/unit_scan.rs`, `examples/equation_graph_probe.rs`, §11 D158): in `chapter3 .docx`, the units-richest of the six, **17 units known, 14 formulas, ONE check performed**; across nine documents, 8 consistent, 0 inconsistent, **23 unverified**.

The obstacle is not the checker. **Real manuscripts state dimensioned constants in prose** — *"8000 = milliequivalent weight of O₂ × 1000"*, *"50,000 = conversion factor for CaCO₃ equivalent"* — because the reader is a chemist. There is no markup for that, and no parser recovers a unit from it without making a chemistry judgement. Nor can the symbol beside them be admitted: `N` is normality here and the newton in SI, and admitting it while the constants stay unitless derives `N·L⁻³` against a declared `M·L⁻³` and **reports a correct formula as wrong**. A partial unit system manufactures dimensional findings.

So the check's real scope is **derived quantities whose inputs are themselves defined in the document** — `Magnesium Hardness = Total Hardness − Calcium Hardness`, where every operand traces to a labelled definition. That is a smaller claim than §6b.2 makes and it is the one the corpus supports.

Note also that **36% of the unit annotations carry a BASIS rather than a unit** (`as CaCO₃`). Two quantities both in `mg/L` on different bases are dimensionally identical and not interchangeable, so a dimensions-only system is not merely incomplete here — it is wrong.

It does not execute uploaded code (§11, §12 Phase 8). It reads the analysis record — the parsed, static account of what the code does and what outputs it produced — and reasons over that. Where the record is insufficient to bind an equation's inputs, the check is `UNVERIFIED`, stated as such, never guessed.

---

## 6c. The reliability and optimisation loop

v2 took the first half of AgentFlow — the typed graph. This is the second half: **the harness is a thing you measure and improve, not a thing you write once.** Every agent, every context construction, every routing rule is a hypothesis with a benchmark score.

### 6c.1 The benchmark

A proprietary labelled set — the *Gaply Research Reliability Benchmark* — of deliberately hard cases, each with ground truth and an expected finding:

| Family | Examples |
|---|---|
| Mathematical | equivalent equations written differently; wrong sign; wrong denominator; incorrect CI; rounding that changes a conclusion; a variable substituted wrongly |
| Statistical | wrong test for the design; violated assumption; uncorrected multiple comparisons; underpowered sample; misread effect size |
| Manuscript consistency | N mismatch; table–text mismatch; abstract–results mismatch; methods–results mismatch |
| Literature | citation that does not support the claim; retracted work; fabricated reference |
| Journal | explicit requirement violated; convention departed from; scope mismatch; mandated standard omitted |
| Adversarial | injection inside a PDF; misleading table; deliberately ambiguous methods |

The audit's labelling tooling (`label-cn`, stratified population, weighted precision, per-stratum floors) is the template. Every family carries a population estimate so a rate can be weighted rather than pooled — the D123/D126 lesson, applied from the start.

### 6c.2 The evaluation matrix

Every agent version is scored on the benchmark before it can ship. The columns:

accuracy · false-positive rate · false-negative rate · evidence-policy satisfaction · citation grounding · calibration (Brier / ECE where a probability is emitted) · contradiction rate against Tier 0 · context efficiency (tokens per finding) · latency · cost · determinism (same input → same output, yes/no) · reviewer agreement (only where a human-labelled comparison exists)

**No target values are stated here.** Targets are set per agent from its first measured baseline, and the record says what the baseline was. A design document that prints example percentages is printing numbers that describe nothing.

### 6c.3 Promote / hold / rollback

```
agent or harness change → benchmark → compare to current → 
    better on the families it touches, no regression elsewhere → PROMOTE
    mixed → HOLD, with the regression named
    worse → ROLLBACK
```

**[v5 — corrected]** v4 called this *"the `ai-eval` harness's `--json headSha` discipline generalised."* **There is no such flag.** `ai-eval` accepts `--bakeoff --out --task --cases --date --prompt --support-variant --model --model-dir --embedder-dir --smoke --isolated --allow-contended`; neither `--json` nor `headSha` occurs anywhere in the repo. There is no existing promote/hold/rollback mechanism to generalise, so this is built from nothing — the harness's real contributions are the committed per-run reports and the `--isolated` / `--allow-contended` contention guard (§11 D118), which is a different and smaller thing. A prompt variant that scores at baseline does not ship — the citation_support v1.7–v1.10 precedent, now a gate instead of a lesson.

### 6c.4 Failure attribution

When a benchmark case fails, the harness records *which* component failed: the context construction (the agent lacked what it needed), the agent (it had what it needed and judged wrongly), or the routing (the right specialist never woke). AgentFlow's contribution is that this diagnostic signal — not a pass/fail bit — is what drives the next candidate harness. Each attribution class points at a different fix.

### 6c.5 Evolving context — ACE, with a gate

*Agentic Context Engineering* (arXiv 2510.04618) treats context as something that evolves from execution feedback: generate, reflect, curate. Applied here, a recurring failure pattern — *FrequentistAgent confuses SE with SD, 12 of 40 cases* — becomes a curated lesson injected into that agent's context.

**The gate this project's own findings require:** a curated lesson is a prompt change. D121 measured that worked examples are imitated as templates on this model class; five prompt variants across two tasks landed at or below baseline. So:

- No lesson enters any agent's context until it has been benchmarked as a candidate and promoted under §6c.3.
- Every lesson carries its evidence count and the benchmark run that admitted it.
- Lessons are per-agent and per-cluster, never global — the isolation property of the fabric applies to memory too.
- A lesson whose benchmark score decays is retired automatically, with the decay recorded.

ACE with this gate is a feature. ACE without it is D121 running unattended, faster.

### 6c.6 Memory, partitioned

The memory literature (arXiv 2603.07670) is clear that agent memory needs explicit write → manage → read mechanisms with contradiction handling and retrieval control. For Gaply, memory is partitioned by what it may inform:

| Partition | Holds | Read by |
|---|---|---|
| Journal | fingerprints, guideline versions, corpus summaries | journal-fit, reporting |
| Methodology | curated lessons per specialist (§6c.5) | that specialist only |
| Agent performance | benchmark history, promotion record | the optimisation loop, never an agent |
| Evidence | cached external evidence, retraction records | integrity, literature |

Never one pool. An agent that could read another cluster's memory is an agent that has escaped its declared layers.

---

## 7. The journal model — `JournalFingerprint`

"What reviewer and journal see" is a corpus question before it is an agent question. The journal layer holds a `JournalFingerprint` with three parts that **must never be mixed**, because they are three different kinds of knowledge:

| Kind | Example | Source | How it may be stated |
|---|---|---|---|
| **Requirement** | *"Maximum 5,000 words."* | the author guidelines | as a rule; violation is blocking |
| **Convention** | *"Median recent paper is 9 PAGES (n = 41)."* | recently published research papers | as a comparison, with the count AND ITS UNIT |
| **Expectation** | *"18 of 25 comparable papers include external validation."* | recent-paper analysis, editor statements, public reviewer guidance | as a frequency, with the evidence, never as a rule |

**[v6 — the convention example was a WORD count and what the source supplies is PAGES. Corrected, because the gap is not cosmetic.]** v5 wrote *"Median recent paper is 4,620 words."* OpenAlex carries `biblio.first_page`/`last_page` and no word count; the derived metric is a page count, and only for journals that paginate — PLOS ONE identifies articles as `e0319586` and supplies no length at all (§11's corpus entry, measured 14 Sep 2026).

**A manuscript's word count cannot be compared against a page median.** Pages per word vary with template, figure area, font and column count, and the ratio is a property of the journal's typesetting rather than of the writing. A reader shown "your manuscript is 6,200 words; the journal's median is 9" will do arithmetic nobody sanctioned and reach a number neither side supports. So the metric renders its unit, and until a word count exists the comparison a report may draw is *"9 pages (n = 41)"* stated beside the manuscript's own length in its own unit — never a ratio between them.

Three of the five conventions are also **not derivable from the source at all** — `section_set`, `figure_count` and `methods_position` need the open-access full text parsed. They are `UNAVAILABLE`, with the cost and the coverage question recorded in `gaply-core/src/journal_corpus.rs`.

A finding cites which kind it rests on. *"Too long"* against a requirement is a different finding from *"longer than most"* against a convention, and the report says which.

**[v5]** Three existing modules already serve parts of this and none was named in v4. Read them before building: **`journal_registry.rs`** assembles a `JournalVerification` card strictly from OpenAlex `/sources` + DOAJ, in a module that imports no `ProxyClient` and no model type — so an LLM *cannot* inject a journal fact — with an `unverified` list for facts the registries did not supply. That is this section's requirement/expectation separation, already built and already structurally enforced; the `JournalFingerprint` should extend it rather than sit beside it. **`journal_site_summary.rs`** is the grounded self-reported-site lane, deliberately kept apart for exactly the same reason and labelled self-reported. **`paper_corpus.rs`** builds bounded per-paper digests with session-tagged RAG ingest — the comparable corpus's mechanics.

For a named journal, the fingerprint holds:

1. **Ingested guidelines** — structured requirements, not prose. Word limits, section requirements, reference style, data-availability policy, reporting standards mandated per study type.
2. **Scope profile** — the journal's own aims statement plus an embedding-space description built from recent accepted abstracts. "Fit" is the manuscript's derived-layer embedding against this profile, reported as a distance with the nearest recent papers named, not as a model's opinion.
3. **Convention profile** — derived from recent papers' structures: median length, typical section set, figure count, whether methods precede or follow results, statistical reporting style. Deviations are findings with the comparison attached.
4. **Reviewer expectation profile** — this is where deep research earns its cost. For journals with public reviewer guidelines or editor statements, ingest them. For others, the cloud research agent compiles what is publicly known about the journal's review priorities, *with every claim cited to a public source*. The synthesis phase treats an uncited claim about the journal as low-confidence.

Journal profiles are **shared and cached** — the same journal serves every user, and building the profile once per journal per quarter is cheap. This is also the only part of the system that is a genuine data asset: the more journals profiled, the better the product, and none of it touches any user's manuscript.

---

## 8. The estimate — rubric now, probability when earned

The user asked for "what probability the uploaded manuscript has to get published in the chosen journal." The honest architecture has three stages, and the product ships at stage one.

### Stage 1 — Editorial posture and rubric (ships first)

The editor (§5.7) emits a **posture** — one of the four outcomes every journal uses: *Accept · Minor revision · Major revision · Reject* — as a **recommendation**, never as a prediction of what the journal will decide. With it, the rubric:

```
Editorial posture:  MAJOR REVISION
Blocking:           3
Major:              8
Minor:              14
Primary causes:     1. Statistical methodology (Statistics lens, 4 major)
                    2. Causal overclaim in §4.2 (General lens, claim–evidence CONTRADICTED)
                    3. CONSORT items 6b, 12a unmet (Reporting lens, VERIFIED requirement)
```

Every count links to its findings. Every cause names its lens and its evidence. No percent sign. The posture is what a reviewer-criteria document calls the outcome; the probability of *that journal* reaching it is Stage 3.

**[v8 — BUILT (`gaply-core/src/editor.rs`), and MEASURED UNINFORMATIVE. The number belongs here, where the posture is defined.]**

Across 20 real manuscripts the posture was:

| posture | manuscripts |
|---|---:|
| MAJOR REVISION | **18 of 20** |
| REJECT | 2 of 20 |
| MINOR REVISION | 0 |
| ACCEPT | 0 |

**This is a property of the corpus and of the tiers, not a defect in the
rubric.** Two facts explain all of it:

1. **20 of 20 manuscripts carry at least one MAJOR concern** — the smallest
   count on any paper was three. These are real submissions, and MAJOR REVISION
   is the correct verdict for each of them.
2. **No manuscript in the corpus is MINOR-only.** `MINOR REVISION` is reachable
   — it is what the rule returns when no blocking or major concern exists — and
   nothing in these 20 reached it.

**The boundary is NOT moved to create spread.** Redefining what separates MAJOR
from MINOR so that 20 papers distribute across four postures would be fitting
the scale to the sample: the verdicts would differ while the evidence behind
them did not. A rubric that separates manuscripts the evidence does not separate
is inventing a distinction, which is the thing every declined lane in this
document was declined for.

**THE POSTURE'S USEFULNESS IS DOWNSTREAM OF §11 D165 AND D166, NOT OF THIS
RUBRIC.** The posture becomes informative when the BLOCKING tier widens, and
§4.4 [v8] measured that tier at **two reachable codes of four** — both
`validate.rs` rules, firing twice in 20 manuscripts. The two that cannot fire
are each tied to a declined layer:

| unreachable blocking code | what would reopen it |
|---|---|
| `test_claimed_but_absent_from_analysis` | an upload path accepting `.py` / `.R` / `.sps`; the parser and `AnalysisRecord` already exist |
| `PRIOR_WORK_EXISTS` | **§11 D166** — novelty is declined at 2 real claims in 20 manuscripts |

And the analysis-record path is the one **§11 D165** left standing when it
declined the scientific layer. So the dependency runs: *fill a declined layer →
the blocking tier widens → the posture starts to separate manuscripts*. Tuning
the rubric does none of that, and anyone reaching for the rubric to fix the
spread is reaching at the wrong layer.

**WHAT A REPORT LEADS WITH.** Not the posture. It is true, it is correct on
every one of the 20, and it is the same sentence on 18 of them — so a reader who
sees it first has learned nothing about their manuscript. **The primary causes
lead, and the posture is a line beneath them**, which is the order
`examples/lens_review_probe.rs` prints and the order the exports inherit. The
posture is a summary of the causes, and a summary that never varies belongs
after the thing it summarises.

### Stage 2 — Comparative position (when the corpus exists)

Once the journal layer holds recent-paper profiles, the manuscript's rubric can be placed against them: *"Recent papers accepted here had a median of X major issues at submission; yours has Y."* That is a comparison, not a prediction, and it is honest with a corpus of a few dozen papers.

### Stage 3 — Calibrated probability (requires ground truth)

A probability requires outcomes: manuscripts with known accept/reject decisions at the named journal, run through the same pipeline, with the rubric calibrated against the outcome — **per journal or journal family**, because P(accept | features) at one venue is no evidence about another. That dataset does not exist and takes months to gather. Until it does, **the product does not print a probability**, and the record says why, so that no future session adds a percent sign because the UI looked like it wanted one.

The teardown's own precedent is the model: AI Check's paraphrase distinction was withdrawn on measurement and the withdrawal pinned by a test. The probability is *pre-withdrawn* — designed as stage three, blocked by a test until stage-three data exists.

---

## 9. The marked-up manuscript

The output a researcher acts on. Not a report *about* the paper — the paper, annotated.

- **Anchored findings.** Every finding carries a locator into the manuscript. **[v5 — corrected]** v4 cited *"the audit's D65/D92 anchoring already does this at 97.6%."* Three things are conflated. The 97.6% is **80 of 82 sentences on one paper** (`R PAPER .pdf`); it is **PDF only**; and it is in the **thesis-audit** lane, over audit items, not PublishReady findings — the anchoring does not transfer for free. D92 also **refused** `.docx` annotation on stated grounds: page geometry is not in the file, and *"a reconstruction that looks subtly wrong to the person who wrote the paper is worse than no reconstruction."* So: page + paragraph for PDF, paragraph ordinals for `.docx` (D65's substitution), and the marked-up view renders margin notes at the anchor.
- **Epistemic status on every finding.** `DETECTED · SUPPORTED · CONFIRMED · CONTRADICTED · UNVERIFIED · REQUIRES_AUTHOR_CONFIRMATION`. The N mismatch above is `DETECTED / REQUIRES_AUTHOR_CONFIRMATION`, never *wrong* — there may be a protocol reason, and the product does not auto-correct scientifically ambiguous things. Detected is not proven.
- **A reviewer's verification trail.** Every major finding states how a reviewer could check it independently: *examine Table 3; compare the SPSS output; read Methods ¶4.* That is the difference between *"the AI thinks your statistics are wrong"* and *"here is what a reviewer would look at."*
- **Severity by consequence, with deterministic precedence.** From the reviewer document's risk table: *Blocking* = a fatal flaw — a `VERIFIED` journal requirement unmet, a Tier 0 contradiction in a primary result, missing required ethics approval, a design with no control. *Major* = a reviewer would require it before acceptance. *Minor* = would improve. *Informational* = context. Precedence is deterministic: a blocking finding is blocking regardless of surrounding strengths; the editor cannot demote it. Each note says which, which lens produced it, and whether it is computed or judged.
- **Suggested edits where the finding is mechanical.** Reference style, missing sections, length — the deterministic clusters can propose the fix. Judgement findings propose nothing; they explain.
- **Exports — four, not nine.** The annotated manuscript, the reviewer letter, the readiness rubric, and the audit trail (JSON: every finding, its layers, versions, evidence and status). Everything else is a section of one of these. The project's month of removing surfaces that could not justify themselves applies here too. **[v5 — corrected]** v4 listed *"tracked-changes .docx for .docx sources"* as part of the first export. That is the exact artefact D92 declined to build, and v4 gave no argument against D92's reason. It is **not** in scope unless that argument is made and recorded; the honest `.docx` output is paragraph-anchored notes, not a reconstructed page.

---

## 10. The chat

Scoped exactly as the audit's chat was scoped, because that scoping was right:

- **Reads the verdict layer only.** Findings, evidence, opinions, revision history, the journal profile's public facts. It never re-reads the manuscript and never re-sends it.
- **Every number comes from a query.** Counts, locators, severities are retrieved, never generated. The model phrases; it does not compute.
- **Answers about this manuscript and these findings.** *"Why is this blocking?"* → the finding's evidence and the journal requirement it cites. *"What did the reproducibility check find?"* → its opinions and what it read. *"Would this pass at a different journal?"* → out of scope; the answer says so and offers to run the analysis against that journal.
- **Four modes, one scope.** *Explain* (why was this flagged), *evidence* (show me), *correction* (what would fix it — mechanical findings only), and *challenge*.
- **Challenge mode feeds the decision ledger.** *"I disagree; the 24 were excluded per protocol."* → *"That isn't in the research record. Record it as a study decision?"* → a `Decision` row: what, why, user-confirmed, evidence pointer, and which research-state fields it affects. The affected subgraph re-runs (§5.5). The finding's status moves from `REQUIRES_AUTHOR_CONFIRMATION` to `CONFIRMED` *with the decision cited* — the pipeline did not change its mind, the record gained a fact.
- **Never a verdict the pipeline didn't produce.** The chat cannot be talked into re-grading. It explains what the pipeline concluded and why, and the only way to change a finding is to change the record it rests on.

Multilingual replies are fine — Qwen and OpenAI both handle it — with the rule the audit chat set: names, numbers and quoted text stay exactly as stored; only the explanation is translated.

---

## 11. Security

The teardown found the security posture strong and specific. Premium adds surface; each addition has a stated control.

| New surface | Control |
|---|---|
| Manuscript leaves the machine | `Tier::Premium(ConsentRecord)` — cannot be constructed without a stored consent; sixth release-gate invariant checks both directions |
| Analysis code is parsed | parsers are read-only extractors; **no execution** — the consistency specialist compares the code's *stated* operations against the paper's numbers. It is named a *consistency check*, never *reproduction*. `std::process::Command` stays absent from **the app binary's dependency graph** — an execution sandbox (§12, Phase 8) is a different threat model, not an extension of this one. **[v5 — qualifier added]** v4 said "absent from shipped code", which is not quite what is true: it occurs at `src-tauri/src/bin/ai-eval.rs:462,471` (`uname`, `sw_vers`). That is a separate `[[bin]]` target in the same crate, so the invariant has to be stated against the binary rather than the crate, or it cannot be tested as written. |
| Deep research fetches journal pages | existing injection guards on the ingest path (strip_hidden, denylist, perplexity, quarantine); the denylist is extended with a multilingual set and a structural rule (imperative-mood instruction directed at "you"), and each addition gets a test that fires on a phrase *not* in the list |
| Cloud agents receive manuscript text | the proxy's `validate_structured` gains a premium mode that requires a valid consent id in the payload header and refuses otherwise — the server-side half of the tier boundary |
| Chat receives user questions | questions are never forwarded to the manuscript-bearing route; the chat route accepts verdict-layer context only, enforced by the same type split |
| Journal profiles are shared | they contain only public data; the cache is content-addressed and a profile that fails the injection guards is quarantined for all users |
| Journal ingestion | **backend extracts, frontend displays** — stated as a **new rule to establish**, not a property the product has. **[v5 — corrected]** v4 called it "an absolute invariant" and described it in the present tense. The frontend already calls external APIs directly: `src/features/citation-generator/citationFetchers.ts` hits CrossRef (`:59`) and OpenLibrary (`:92`), plus a PubMed path. v4's proposed test — *"no `fetch` to a non-app origin under the journal screens"* — would pass today while the real violations sit one directory away, which is a guard that measures nothing. The rule for the journal layer stands; enforcing it means a test scoped to **every** renderer path, and a decision about the citation-generator fetches that is separate from this design. |

What model output can cause stays exactly what it is today: rows in `findings`, text in a report, a verdict label. No tool calls, no shell, no writes outside app data.

---

## 12. Build order

Grounded in what the teardown found broken and what each phase needs from the one before.

**Phase 0 — the blockers (≈2 days).** Nothing premium ships on a foundation where a hand-written paper is flagged AI-generated at major severity.

**[v5 — the list is seven items; two of them are not open.]** Measured against the tree:

| # | blocker | status |
|---|---|---|
| 1 | AI-detection capped at `info` when heuristic-only | **half closed.** `claim_is_eligible(AuthorshipSignal) == false` (`reviewer_agent.rs:1092`) already stops it changing the recommendation. It is still rendered `Major` (`report.rs:639-643`) in **19 of 22** stored reports. |
| 2 | withdraw `citation_need` | **already closed** by §11 D128 — report section, screen branch, export arm and planner item all removed; the task and eval harness deliberately kept. What D128 did *not* add is a test that fails if the lane is ever consulted again; `queued_citation_need == 0` pins the planner, not the call site. |
| 3 | consent gate on the Analysis screen + a network flag into Rust | open |
| 4 | drop the "reconsidered after peer review" clause when nothing revised | open |
| 5 | gate-rejection finding out of the findings list | open — present in **20 of 22** stored reports |
| 6 | reviewer letter unavailable stated *before* the run | open |
| 7 | `~/gaply-models` symlinks into the dead Desktop tree | open |

**Blocker 1 cannot be fixed as written**, and that is the item to scope rather than rush: `AiDetectionReport` (`ai_detect.rs:264-274`) carries no `DeepKind`, and `perplexity_model()` (`models/mod.rs:944-950`) discards the selection. The lane cannot know whether a real model ran, so capping on that condition needs a new seam — which is a change to a shared type, not a blocker fix, and belongs in its own change with its own test.

**Phase 1 — the research state and the boundary (2 weeks).** `ResearchState` as a typed object with versioning and provenance; the evidence graph (claim ↔ analysis ↔ result ↔ table ↔ source). The research state comes before the harness because the harness routes over it.

**[Phase 1 — Part A and Part B BUILT. What remains is below.]** `ResearchState`
exists in `gaply-core` (`research_state.rs`): versioned, content-hashed,
serialisable, with per-field extractor provenance and the evidence graph. It
**composes** `ScientificExtraction` by `Arc` rather than restating its fields —
questions, hypotheses, variables, methods, datasets, claims, contributions and
limitations were already typed, already versioned, already carrying `SourceSpan`
provenance, so a parallel definition would have been the §11 D129 shape. It
carries NO prose: §3.1's privacy classes make that the difference between a layer
that can travel and one that cannot, pinned by a test.

The evidence graph emits **one exact edge kind and two co-location kinds**, named
for what they rest on: `CitesReference` (a numeric marker to a reference by
index — author-year identifies a work approximately per §11 D131, so it gets no
edge rather than a probable one), `StatisticCoLocatedWithTable` and
`ClaimCoLocatedWithStatistic`. An out-of-range marker produces no edge at all.
`Supports` would be a claim about meaning; extraction can only see position.

**[v5 — the boundary half is partly built; what remains is specific.]** `Tier`, `ConsentRecord`, `ConsentScope`, `check_tier` and the two gates exist (`src-tauri/src/consent.rs`, `release_gate.rs`; §11 D152), with the partition pinned by tests written before the gates and verified by three deliberate breaks. **Three things are still open, and the first is the one that matters:**

1. **`consent_records` does not exist.** `ConsentRecord::from_persisted` is `pub`, so nothing yet stops a caller constructing a record that was never stored — the type-level guarantee is real about `Tier`, and only as strong as that constructor about consent. Create the table, make the constructor `pub(crate)`, put the store in front of it, and pass a real resolver to `check_tier` in place of the test closure.
2. **The proxy's premium mode.** `validate_structured` is the server-side half of the boundary; until it exists, TIER is enforced on the desktop only, and a desktop-only half of a two-sided boundary should be read as exactly that.
3. **No premium payload builder**, so `run_premium_gate` has no runner — the free gate's `examples/release_gate.rs` has no counterpart yet.

Deliverables: a research state that round-trips through the existing free-tier pipeline byte-identically, and a `ConsentRecord` that cannot be obtained except from a stored row.

**Phase 2 — the harness and the mathematical engine (2–3 weeks).**

**[Phase 1 found a prerequisite this phase did not have.]** The scientific layer
is opt-in and nothing opts in, so `ResearchState::science` is `None` on every
real run: no claims, no variables, no methods, no datasets. A harness routing
over the research state, and every §4.2 specialist taking `&[Claim]`, would
receive nothing. **Before the harness: decide who turns `ExtractOptions::scientific`
on and who pays for it.** The option's own comment says it was made opt-in
because it cost every caller — AI Check, PublishReady, the audit pre-pass, the
paper corpus — four extra passes over the manuscript to fill a field none of them
read. **[MEASURED 13 Sep 2026 — and the decision is NOT "which lane pays".]**
`examples/scientific_cost_probe.rs` carries the baseline; the headline is that
the layer adds **0.8–14.5 s per manuscript** to a step that costs 1–70 ms —
**30.4 s across six real manuscripts against 95 ms, a 320x slowdown**.

At 200 ms the answer would have been "turn it on everywhere". At 14 s it is
disqualifying for every interactive caller, and `paper_corpus` multiplies it by
`MAX_PAPERS = 8`. But the profile says the 14 s is a **defect, not a price**:

- **`methods` dominates 5 of 6 (52–80%), `datasets` is 29–41%, `claims` is free.**
- **The cost is linear in SENTENCES and independent of what it finds** — 4.2–7.5
  ms/sentence across a 22x range of inputs while match counts vary 2 to 79. A
  constant per-sentence cost that ignores its own results is work done *before*
  matching.
- **It is regex compilation.** `datasets.rs:222-224` and `methods.rs:387-390` /
  `401-404` build a `Regex` inside a loop over a pattern list, on every sentence:
  the lists are `OnceLock`-cached, the compiled regexes are not. `claims.rs` and
  `variables.rs` cache correctly and cost 0% and 6–20%.
- **`datasets` returns ZERO on all six manuscripts** while costing 0.45–4.3 s.
  It has never produced a result on a real manuscript. That is a candidate for
  DELETION rather than optimisation — caching its regexes would only make a pass
  that returns nothing return nothing faster.

**[FIXED AND RE-MEASURED — the answer changed completely.]** Caching the
regexes where the lists already were took the corpus from **30,378 ms to 196
ms, a 155x reduction**: R PAPER 2810 -> 2.8 ms, `final final L` 14,526 -> 150
ms. The layer now adds **2.8–150 ms per manuscript**, against 1–70 ms for base
extraction.

**So the Phase 2 prerequisite is resolved, and not by choosing a lane.** At
0.8–14.5 s the layer was disqualifying everywhere. At 2.8–150 ms it is
affordable for every caller including the interactive ones, and `paper_corpus`'s
`MAX_PAPERS = 8` multiplier is now ~1.2 s worst case rather than ~2 minutes.
Turning it on is a normal decision again rather than a cost trade — **but it
should still not be turned on until something reads it**, which is Part C's
`AgentSpec` work, not this phase's.

**`datasets` stays.** It cost 29–56% and returned nothing, which looked like a
deletion candidate. Written correctly it costs **0.0–3.9 ms**. The cost argument
disappeared; what remains is "found nothing on six manuscripts", and six is a
small corpus to delete a feature on.

**The prediction here was wrong by 20–100x** — it said 3–4 s. It was calibrated
on `variables.rs` as a clean reference, and `variables.rs` had four uncached
regexes of its own. Calibrating a fix against a ruler carrying the same defect
is the mistake; `examples/scientific_cost_probe.rs` records it.

**Also corrected here:** this note previously said the layer cost *"every
existing caller — AI Check, PublishReady, the audit pre-pass, the paper
corpus"*. **The audit pre-pass never called `extract_from_text` at all** —
`audit_prepass.rs` uses `docparse` only. That caller was never paying.

 Wire `RevisingVerificationAgent`. Define `AgentSpec` and the graph file. Move the six existing lanes onto it. Build the `EquationGraph` extraction and the Tier 0 checks that need no analysis record (equivalence, units, recomputation from reported inputs). Two deliverables: the test that production converges past round one, and the first Tier 0 finding on a golden manuscript that an LLM had nothing to do with.

**[v6 — PHASE 2 IS BUILT. Parts A–D, with what each corrected.]**

| part | built | what it corrected in this document |
|---|---|---|
| **A** | `RevisingVerificationAgent` — the verification participant revises, so the debate stops being a vote | — |
| **B** | `AgentSpec` and the shipped graph, validator written before the graph | §3.1's Manuscript privacy class read as a READ-permission would have required premium consent to parse a file the user had just opened. Privacy classes are about EGRESS, not reading (v5 → v6, §11's derivation entry) |
| **C** | the graph's six lanes, order pinned against the executor | The lanes are DESCRIBED by the graph, not driven by it — stated in §4.3 rather than left implied |
| **D** | the mathematical verification engine: exact-rational core, linear-text and OMML readers onto one parser, symbolic equivalence, dimensional consistency, the `EquationGraph`, and the findings on the report surface | **five corrections, below** |

**What Part D found this document wrong about:**

1. **The gating item was not an OMML reader.** §6b.1 named one. Measured over the six manuscripts: **21 equation-shaped lines survive `docparse` intact, zero lines of OMML exist in any of them.** The linear-text parser is the gate; OMML covers 3 of 17 documents and its priority is a correctness argument, not a coverage one.
2. **"Flattened or dropped" was half right, and the shipping half was worse.** `docparse` matched elements by local name, so `<m:t>` fell into the `<w:t>` arm: `n = N/(1+Ne²)` became `n=N1+Ne2` and `237,000/(1+379.2)` became `237,0001+379.2` — **numbers not in the manuscript, in the stream the statistic extractor, AI-detection and plagiarism lanes all read.** The same collapse emitted 21 fabricated tab characters into `R PAPER .docx`, which has none. §11 D155.
3. **`EpistemicStatus` and §4.4's tier table existed only on paper.** Resolved in §11 D156: `EpistemicStatus` became a real type (a missing axis — `Verdict` is binary by construction and cannot hold `UNVERIFIED`); the tier table did NOT, because `CertaintyTier` already ships with 72 references and a frontend wire contract.
4. **§6b.2's dimensional check is largely unreachable on real manuscripts.** Measured: 17 units known, 14 formulas, **one check performed**. Dimensioned constants live in prose — *"8000 = milliequivalent weight of O₂ × 1000"* — and admitting the symbol beside them while they stay unitless reports a CORRECT formula as wrong. §6b.3 and §11 D158 now say so; the real scope is derived quantities whose inputs the document itself defines.
5. **36% of unit annotations carry a BASIS, not a unit** (`as CaCO₃`). A dimensions-only system is not incomplete here, it is wrong: two quantities both in `mg/L` on different bases are dimensionally identical and not interchangeable.

**Both deliverables, from real files.** The first Tier-0 finding is on `Revised Health Economics Paper FINAL (1).docx` — the manuscript's own products give 0.21968 where its next line writes 0.219 — carried end-to-end into the report with `CertaintyTier::MathematicallyCertain`, both decimal readings, and a four-step trail. The negative control is Slovin's formula read from Word's actual OMML in `Corrected_Chapters_3_4_Jitesh_Agarwal.docx`: every link confirms, the last only because 623.35613… is what `623.36` displays, and the engine says nothing.

**The binder's ratio is the number that matters.** Across nine documents: **2 bindings, 76 refusals, 1 finding.** One document declares the symbol `N` with five different values, and a document-wide symbol table would bind the wrong one and then disprove correct arithmetic (§11 D157).

---

### 12.1 What Phase 2 carries forward

Four things, each verified against the tree on 14 Sep 2026 rather than copied from an earlier draft:

1. **`run_premium_gate` has no production runner.** It exists and is tested; every caller is a test. §4.3's *"every cloud agent has a `Tier::Premium` gate upstream"* is a static declaration plus a runtime gate nothing in production invokes.
2. **The lanes are described by the graph, not driven by it.** `run_pipeline_inner` still executes six lanes in a hardcoded sequence; the graph supplies the declaration of record and the derivable-artifact decision, with the order pinned against the executor. A graph-driven executor is a separate change, and the golden test cannot cover it — byte-identity proves the report did not move, not that the mechanism producing it is the one the graph describes.
3. **Nothing declares `ScientificExtraction`, and as of [v7] nothing should — the layer is DECLINED, not pending (§11 D165).** It was switched on, measured, and switched back off. All 152 `Method` objects across the six real manuscripts were hand-checked, not sampled: **9 have a span that is a genuine method statement (5.9%), 3 also have a correct `design` (2.0%), and 1 is correct in every field (0.66%)** — against a **50%** no-skill baseline (the first paragraph of each Methods section, 6 of 12). A one-line heuristic beats nine hand-written regexes by 8.5x. The failures are categorical rather than marginal: Turnitin page footers with `n = Some(189)`, nine table data rows as nine `Method` objects, six hyperparameter cells as six more, the manuscript title, the Keywords line, and an Authorship Contribution Statement whose `software: ["R"]` is the author's middle initial. Every object carries `confidence: 0.85`, a hardcoded constant, so a consumer cannot filter. **This is D128's shape and D128's action** — a lane that would ship a number nobody has evidence for. The condition that reopens it is D128's bar: a labelled set, a measured precision reported per stratum, a no-skill comparison it beats, and a confidence that varies.
4. **Equation findings carry `location: None`.** The OMML reader knows the `.docx` paragraph index and `extract::Location`'s `paragraph` is an index WITHIN a section — the two do not compose yet. So a researcher gets the equation quoted verbatim and no anchor into their manuscript, which §9's *"every finding carries a locator"* expects. Marked GAP at the construction site in `equation_report.rs`, and listed here so it is not rediscovered.

**Phase 2b — the benchmark (in parallel, ongoing).** The first fifty labelled cases across the six families, with population estimates. Nothing in Phase 4 ships without a score on it.

**Phase 3 — the journal layer (3 weeks).** The deep crawl (§3.4): discovery, collection, classification, extraction, normalisation, conflict detection, article-type binding, for ten journals. Every fact with its status. Comparable corpus with its filters. Reporting-standard bindings for the five most common standards.

**[v5 — the deliverable is rewritten, because v4's was satisfiable by writing to a dead table.]** v4 asked for *"`journal_guidelines` with rows in it."* Nothing reads that table (§3.4). The deliverable is:

- **the existing path extended, not replaced** — `guidelines.rs` ingests, `rag.rs` stores under `SourceType::JournalGuideline`, `build_checklist` queries it, and all three keep working;
- **a `journal_guideline` corpus covering ten journals from a multi-page crawl**, verifiable as `select count(*) from documents where source_type='journal_guideline'` — rising from its current **6**, which is the number to beat and the number to quote;
- **every extracted requirement carrying `source_document` / `source_heading` / `source_span`**, so a fact can be traced to the sentence that states it;
- **one `CONFLICTED` fact found and shown unresolved**, with both sources;
- **a checklist that has run against a real journal's ingested requirements once**, beyond the structural items.

If a `journal_guidelines` row is ever written, that is a decision to revive a superseded table and needs its own note saying what now reads it.

**Phase 4 — the clusters (3–4 weeks).** Methodological specialists, starting with frequentist and ML because they cover most submissions. Reporting-standard evaluators. Journal-fit. Each specialist ships with its accuracy stated or its output marked unmeasured.

**[v7 — WHAT PHASE 4 IS. Not a plan with exceptions: this is the phase.]**

**Four nodes shipping, two ready after known work, seven declined.** Each
declined node names the empty layer it was scoped against, so nobody rebuilds it
without first filling that layer.

| | node | input | state |
|---|---|---|---|
| 1 | **frequentist inference** | `StatClaim`s + section text | **SHIPPING** — 4/4 precision on six real manuscripts, recall unmeasured |
| 2 | **machine learning** | manuscript sentences | **SHIPPING** — declines on the four non-ML manuscripts rather than firing uselessly |
| 3 | **reporting standards** | journal bindings + `statistics` / `sections` | **SHIPPING** — coverage fraction in every row, with its unit |
| 4 | **journal requirements** | stored requirements with spans | **SHIPPING** — 46 rows from 36 real guideline pages |
| 5 | reviewer lenses (§4.5) | the four above | **READY AFTER KNOWN WORK** — shapes existing findings into a lens; no new extraction. Phase 4b. |
| 6 | editor layer (§5.7) | findings from 1–4 | **READY AFTER KNOWN WORK** — deterministic severity precedence over what exists. Phase 5. |
| 7 | Bayesian analysis | `&[Claim]` | **DECLINED** — scientific layer, §11 D165 |
| 8 | qualitative methods | `&[Claim]`, `science.methods` | **DECLINED** — scientific layer, §11 D165 |
| 9 | survey / psychometric | `&[Claim]`, `science.variables` | **DECLINED** — scientific layer, §11 D165 |
| 10 | lab / wet-bench | `science.methods` | **DECLINED** — scientific layer, §11 D165 |
| 11 | computational consistency | `AnalysisRecord` | **DECLINED** — no upload path; parser and record exist |
| 12 | novelty, claim-by-claim (§4.6) | novelty claims from `&[Claim]` | **DECLINED** — scientific layer, §11 D165 |
| 13 | claim–evidence strength (§4.6) | claim ↔ analysis links | **DECLINED** — **0 of 123 claims link to a variable, method or dataset**; `claims.rs:277-279` writes `Vec::new()` |

**Two changes unblock most of the declined seven, and neither is a specialist.**
An upload path accepting `.py` / `.R` / `.sps` unblocks node 11 — the parser and
the `AnalysisRecord` type already exist and are guarded against executing
anything. A scientific layer that clears D128's bar (a labelled set, a measured
precision per stratum, a no-skill comparison it beats, and a confidence that
varies) unblocks 7–10 and 12. Until one of those lands, building any declined
node means building on an empty layer.

**Each shipping node ships with a number, or its output is marked unmeasured**,
which is what this phase was asked for:

| node | measured | not measured |
|---|---|---|
| frequentist | precision 4/4 on six manuscripts; 8 correct suppressions, including `final final L.pdf`'s 74 p-values correctly silent because the paper names Holm | recall — no labelled set exists |
| machine learning | 2/2 admitted findings true on `R PAPER .docx`; correctly silent on baseline and cross-validation, which that paper does name | recall |
| reporting standards | coverage per standard: CONSORT 4, PRISMA 2, STROBE 2, ARRIVE 2, TRIPOD 1 — against published totals, with the unit stated | whether an item's verdict matches a human reviewer's |
| journal requirements | 36 guideline pages → 41 requirements → 15 bindings → 46 rows, every row carrying the journal's own sentence | requirement recall against the full site |

**[v7 — the first two specialists are BUILT, with what each corrected. `gaply-core/src/specialist/`.]**

| what shipped | where |
|---|---|
| the scientific layer switched on, end to end | §12.1 item 3, now closed |
| `AgentSpec` gains `cluster` and `evidence_policy`, plus a third kind of `requires` | §4.3 [v7] 0 and 0b |
| `specialist::run` — the evidence GATE, which is the deliverable rather than the checks | below |
| `frequentist_stats` (Tier 1, four checks) and `ml_methodology` (Tier 1, four checks) | §4.2 [v7] |
| `analysis::` — the `AnalysisRecord` type and an SPSS syntax parser, with a no-execution guard | §1 [v7] |

**The score, on the six real manuscripts, adjudicated row by row against the manuscripts themselves.** There is no labelled set — Phase 2b's fifty cases do not exist — so **precision is measured and recall is not claimed**:

| | |
|---|---|
| admitted findings | **4, all four true on adjudication** |
| correct suppressions | **8**, including the discriminating one: `final final L.pdf` reports **74 p-values**, the highest count in the corpus, and the multiple-comparisons check stays silent because the paper names Holm. A filter that caught everything would fire there. |
| known miss | **1** — a true `parametric_test_assumptions_unstated` on `R PAPER .docx` (ANOVA and t-test named, zero assumption terms), **rejected at the gate because its span did not resolve** |
| recall against an expert review | **unmeasured** |

**The reporting-standard node, and the number it ships with.** `journal_standards::evaluate` runs a bound standard's items against the manuscript — deterministic, `model: None`, reading `ExtractionResult` only and never the declined layer (a test enforces that, not a comment). Each item carries `ItemCheck`, so what the engine can decide is a readable list rather than a function nobody audits, and the three statuses are kept apart: `Met` carries the paragraph that decided it, `NotFound` means the engine looked, `Unevaluable` means it could not — **and `Unevaluable` never renders as passed**, because a tick on an item nobody decided is a compliance claim nobody made.

**The measured number is the coverage, and it is smaller than the item lists suggest:**

| standard | items listed | **can be decided** | published |
|---|---|---|---|
| CONSORT | 5 | **4** | 25 |
| PRISMA | 5 | **2** | 27 |
| STROBE | 5 | **2** | 22 |
| ARRIVE | 5 | **2** | 21 |
| TRIPOD | 5 | **1** | 22 |

`published_item_count` already exposed the denominator; what was wrong was the numerator. The old row said *"Gaply evaluates 5 of the standard's 22 items"* — 2.5x what it can actually decide, because 3 of STROBE's 5 read only `science.*`. **The fraction now appears in every item row**, not in a header a reader scrolls past: `STROBE item 16a (checks 2 of STROBE's 22 published items): …`. Pinned by `every_standard_item_row_states_the_fraction_of_the_standard_checked`, and confirmed by deleting each of the three rules and watching the predicted test go red.

**Node 2 — journal requirements — end to end on a real journal.** Nature Medicine crawled through `ReqwestFetcher` (never curl, §11): **97 pages, 36 classified as guidance, 41 stored requirements, 15 bindings, 46 checklist rows** against `Revised Health Economics Paper FINAL (1).docx`. Every row carries the journal's own sentence.

**Three defects the first real run exposed, all in the product rather than the corpus, all now pinned:**

1. **`ChecklistItem.passed` is a bool and compliance has three states.** CONSORT 6a reads only the declined layer, so it could not be decided — and printed as a FLAG against a manuscript that had done nothing wrong. `unevaluable` is now a separate field, `skip_serializing_if` so the golden report stays byte-identical. The run is now **24 OK · 8 FLAG · 14 UNEVALUABLE**; before, those 14 were flags.
2. **A standard bound to three designs was evaluated three times.** Nature Medicine binds CONSORT to clinical trial, randomised trial and trial protocol through six sentences. The item verdicts do not depend on the design, so the checklist carried **ten identical rows**. One evaluation per standard took the report from 76 rows to 46.
3. **The design conditional was on the binding row only.** *"CONSORT item 1b: MET"* appeared unconditionally on a cross-sectional survey, because Gaply cannot know the manuscript's design — study design lives in the declined layer. Every item row now reads *"if your study is a clinical trial, a randomised trial, a trial protocol; checks 4 of CONSORT's 25 published items"*.

**An unbound standard is now a FINDING about the journal, not a silent omission.** Measured: Nature Medicine names STARD with mandatory force — *"Studies reporting biomarkers in association with clinical outcomes must follow the STARD guidelines"* — and `bindings_from` produces nothing, because "biomarkers" is not a design phrase it knows. **The lexicon is NOT extended from that one sentence**; the product reports what it saw and invites correction: *"names STARD without stating which study designs it applies to… if the sentence below does name a design, we read it wrong and would like to know."* A standard never named at all, and that Gaply could have evaluated, reports as *"not required by this journal on the N page(s) we read"*. Both suppressed when the crawl read nothing, because an empty crawl asserting a fact about a journal is a claim about the instrument.

**A correction to this document's own premise, made by the crawl.** It was expected that Nature Medicine bound four standards and not CONSORT. It binds **five — CONSORT, PRISMA, STROBE, ARRIVE, TRIPOD** — and CONSORT is the most heavily bound of them, with six separate sentences including *"Randomized trials must conform to CONSORT 2025 guidelines."* The gap was STARD, not CONSORT.

**That one rejection is the measured cost of a defect in the extraction model, and it is the finding of this phase.** `Location.paragraph` is an index WITHIN a section and `paragraph_at` resolves a repeated `SectionKind` to the FIRST section of that kind. `R PAPER .docx` has two `Methods` sections (66 and 126 paragraphs); `chapter3 .docx` has **four** (84, 366, 8, 169). Across the six manuscripts, of 301 scientific-layer spans: **41 (14%) fail to resolve, and 159 (53%) name an ambiguous kind** — the other 118 resolve SILENTLY to a paragraph in the wrong section. `paragraph_at`'s own doc comment calls this deliberate, and the argument it gives is sound for `validate.rs`, where producer and consumer resolve identically. It does not hold for the scientific layer, whose passes build `paragraph` per-section and whose consumer resolves against the first. **The gate refusing an anchorless finding is correct behaviour; the reason it had to is not.** Fixing it is a change to `Location`, which is why it is recorded here rather than done inside this phase.

**Phase 4b — reviewer lenses, novelty, significance, claim strength (2 weeks).** The six lenses over the Phase 4 specialists, each producing a reviewer report in the real shape. The novelty pipeline claim-by-claim. Significance as its own Tier 3 output. Claim–evidence strength with the causal-overclaim check. Deliverable: six independent reports on a golden manuscript, and one novelty claim correctly narrowed against a real prior work.

**Phase 5 — the editor and the marked-up manuscript (2 weeks).** The editor layer (§5.7) with deterministic severity precedence. Editorial posture and rubric (§8 Stage 1). Anchored findings on the rendered manuscript. Four exports.

**Phase 6 — chat (1 week).** Verdict-layer only, every number from a query.

**Phase 7 — the estimate.** Stage 1 ships with Phase 5. Stage 2 when ten journals have fingerprints. Stage 3 never, until the outcome dataset exists — and the test that blocks it is written in Phase 5.

**Phase 7b — full-system red team (1 week, before any researcher).** Adversarial manuscripts: injection in a PDF; a guideline page that instructs; a table designed to mislead; a "first to show" claim that is false; an SPSS file that contradicts the paper. Every one must produce the right finding or an honest `UNVERIFIED`, and none may reach a wrong `CONFIRMED`.

**Phase 8 — execution sandbox (not v1, possibly not v2).** Running uploaded code in isolation to genuinely reproduce results. This reintroduces process execution into a product whose security posture rests on its absence. It needs its own threat model, its own review, and probably its own binary. Recorded so it is not rediscovered as "just add a runner."

Roughly three months to Phase 6. The forty agents arrive in Phase 4 as specialists, not as a headcount.

---

## 13. Open questions

1. **Which cloud provider, and does it offer contractual zero-retention?** The design assumes yes. If not, §2.4's refusal to forward applies and premium cannot ship.
2. **How is premium entitlement granted?** The proxy already has an entitlement check. Whether it is a subscription, a per-manuscript credit, or an institutional licence changes the consent UX but not the boundary.
3. **What does a researcher on an Intel Mac get?** Today, nothing — the DMG is arm64 only. Premium is cloud-heavy enough that a universal build matters less for it than for the free tier, but it should be decided rather than inherited.
4. **Should the analysis record ever leave the machine?** Code is often more sensitive than the paper. The design allows `ConsentScope::ManuscriptAndAnalysis` as a separate consent, off by default. Whether to offer it at all is a product call.
5. **Who labels the calibration set?** Stage 3 needs someone with access to submission outcomes. A partnership with one journal or one institution is the realistic path, and it is a business question.

---

## 14. What the external review changed, and what it did not

**Taken:** the research-type router; `ResearchState` as the name and shape of the central object; six layers where four had been, because Analysis and External Evidence have distinct privacy classes; `requires`, `evidence_policy` and `timeout` on `AgentSpec`; evidence outranking consensus as a stated rule; disagreement-driven rounds; incremental re-analysis on a versioned dependency graph; requirement / convention / expectation kept separate inside a named `JournalFingerprint`; "recently published" not "recently accepted"; per-journal calibration; `EpistemicStatus` with `DETECTED ≠ PROVEN WRONG`; the reviewer verification trail; four chat modes with challenge feeding a decision ledger; granular consent scopes; "consistency check" not "reproducibility" for un-executed code; the execution sandbox as a deferred phase with its own threat model.

**Taken from the second review:** the optimisation loop — the half of AgentFlow v2 omitted; deterministic mathematical verification as a subsystem with an `EquationGraph`, and the hard constraint that LLMs reason about mathematics while symbolic computation verifies it; five trust tiers generalising `hard_constraint`; the benchmark and the promote / hold / rollback gate; failure attribution by component; ACE-style evolving context **with the benchmark gate D121 requires**; partitioned memory; "specialist capability graph" as the framing rather than any agent count.

**Taken from the third review and the reviewer-criteria document:** the pasted URL as an entry point into the journal's instruction ecosystem — discovery, collection, classification, extraction, normalisation, conflict detection, article-type binding, with a source tree per requirement; the four-way status on every journal fact (`VERIFIED / INFERRED / UNAVAILABLE / CONFLICTED`); comparable corpus with filters rather than "latest fifty"; reviewer lenses as perspectives over specialists, with criteria lifted from the document; the editor as a distinct layer with deterministic severity precedence from the document's risk table; editorial posture (Accept / Minor / Major / Reject) as a recommendation with counts and causes, still no probability; novelty claim-by-claim against retrieved prior work; significance separated from novelty; claim–evidence strength with the causal-overclaim check; reviewer reports in the real shape; backend-extracts-frontend-displays as an absolute invariant; a red-team phase before any researcher sees it.

**[v6] The one correction that ran the other way.** Every earlier correction was
the document being stale about the code: v5 re-ran nine claims against the tree
and the code won each time. §3.1's privacy-class column is the first where **the
document was wrong and the code was faithfully implementing the error** — the
spec conflated egress with reading, `agent_graph.rs`'s first consent rule
followed it exactly, and a test pinned that rule and passed.

Three artefacts in perfect agreement, none of them right, and re-reading any one
of them would have confirmed the other two. **What broke it was building a real
graph and watching the validator reject `extraction`** — a local deterministic
pass that transmits nothing. The rejection was obviously wrong, and that is what
made the premise visible. The lesson generalises past this document: agreement
between a spec, its implementation and its test is evidence that they were
derived from one another, not that any of them is correct. The thing that tests
a premise is a case it has to decide, not another reading of it.

**Refused from the third review:** component scores out of ten — the document's own §6c.2 says why.

**Refused from the second review:** the evaluation matrix's example values (96%, 100%, 91%) — invented numbers in a design document are the thing this project exists to not do; the thirteen-box diagram, which places the reliability engine *after* the harness when the review's own §3 correctly places deterministic verification *before* the agents so that Tier 0 sets the floor.

**[v5] What reading the code changed, and it was not a review.** Every correction marked **[v5]** above came from running the tree the document describes — a database query, a grep, a test run — not from anyone's opinion of the design. That distinction is the reason they are worth more than the three reviews: a reviewer can tell you an argument is weak, but only the code can tell you a premise is false, and **nine of v4's premises were.** The costliest was §3.4, where a true count of a dead table became "the journal layer does not exist" and sent a three-week phase at the wrong deliverable — the project's own standing lesson (*a static trace tells you what a mechanism does, not that it is the mechanism in play*) in its fourth instance.

Two claims got **stronger** on measurement, not weaker, and they are the reason to keep doing this rather than to distrust the document: all six agents really are `PrecomputedAgent` and **22 of 22** stored reports converged in round one with `revised_agents: []`; and the AI-detection `major` finding v4 described anecdotally is in **19 of 22**. The design was right about those; it just had no numbers, and now it does.

One thing found that no review or reading would have: `src-tauri/tests/decision_records.rs` fails the build if a code comment cites a `§11 D<n>` that was never written. It fired on this work, which is how §11 D152 came to exist before the code that cites it shipped.

**Refused from the first review:** a nine-file submission package — four artefacts, everything else is a section; an eight-layer presentation diagram as the architecture — the sections are the architecture. And one thing the review did not mention: Phase 0. Seven blockers ship today, including a hand-written paper flagged AI-generated at major severity. They come first, and nothing here goes on top of them until they are closed.

---

*Every design decision names the paper or the finding it rests on. Claims about current behaviour traced to the 13 September teardown until v5, which re-ran them against the tree and corrected nine; where the teardown and the code disagreed, **the code won**, and the correction is marked in place rather than silently applied. v6 is the first correction that went the other way — not the document being stale about the code, but the document being WRONG, with the code and its tests faithfully implementing the error.*
