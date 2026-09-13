# PublishReady Premium — Implementation Prompts

*One prompt per phase, in order. Each is self-contained: paste it into a fresh Claude Code context (`/clear` first) with `docs/publishready-premium-architecture.md` committed to the repo. Do not skip a phase; each one's deliverable is the next one's precondition.*

*Every prompt ends with the same three rules because they are what has kept this project honest: predict the failure before claiming the gate; run the negative control; report where the design is wrong about the code.*

---

## Prompt 0 — Read the design against the code

```
Read docs/publishready-premium-architecture.md in full.

Do not build anything. Report, with [ran]/[read]/[doc] tags:

1. Every claim the document makes about CURRENT code that is wrong,
   stale, or unverifiable. The document was written from the 13 Sep
   teardown; the tree has moved since. Name the file and line for each.

2. Every place the document assumes a seam that does not exist — a
   type, a table, a command, a hook — and say whether it should be
   created or whether an existing thing already serves.

3. The seven Phase 0 blockers from the teardown: which are already
   closed, which are open. For each open one, the exact file and the
   one-line fix.

4. Anything in the document that contradicts a recorded decision in
   §11 of docs/AI_ENGINE_PLAN.md or a norm in CLAUDE.md. Do not resolve
   the contradiction; name it.

Ranked by how much each would derail Phase 1 if left unfixed.
```

---

## Prompt 1 — Phase 0: the seven blockers

```
Close the seven Phase 0 blockers from the teardown, in this order:

1. AI-detection: when DeepKind is HeuristicOnly, cap the finding at
   info severity and say plainly it is a word-frequency proxy. A
   hand-written paper must not be flagged major/concern by a heuristic.
2. citation_need at 0.34: withdraw it from the PublishReady report the
   way AI Check's paraphrase distinction was withdrawn — pinned by a
   test that fails if it is ever consulted again. Keep it in the eval
   harness.
3. Gate the Analysis screen's reference lookups on mayUseCloud AND
   thread a network flag into run_full_analysis so the verification
   lane refuses in Rust when consent is absent. localStorage is not a
   boundary.
4. Drop the "reconsidered after peer review" clause from the disclaimer
   when revised_agents is empty.
5. Move "Verification output rejected by its internal gate" out of the
   user-facing findings list into a harness log.
6. Say the reviewer letter is unavailable BEFORE the run when
   GAPLY_PROXY_URL resolves to loopback.
7. Replace the ~/gaply-models symlinks into the dead Desktop tree with
   real copies.

One commit per blocker, staged by path. Each commit's message states
what a researcher would have wrongly believed before the fix. cargo
test --workspace and the frontend suite green before each push.

For 1, 3 and 4: run the negative control — reproduce the wrong output
first, then fix, then show the same input now produces the right one.
Paste both outputs.

Report what you found that the teardown did not.
```

---

## Prompt 2 — Phase 1: the research state and the boundary

```
Phase 1 of docs/publishready-premium-architecture.md. Two things, in
this order, nothing else.

PART A — ResearchState (§3.1, §3.3)

Define ResearchState in gaply-core as a typed, versioned, serialisable
object: research question, hypotheses, study design, population,
variables, outcomes, claims, methods, analyses, results, tables,
figures, references, uncertainties. Every field carries provenance
(which extractor produced it, from which manuscript span) and the
object carries a version and a content hash.

Wire the existing free-tier extraction to populate it. Do not change
what extraction does; change where its output lands. The existing
report must still render identically from a ResearchState — pin that
with a golden test on a real stored job: same manuscript, same
report bytes, before and after.

Add the evidence graph as edges between ResearchState nodes: claim →
analysis → result → table → source. Populate it from what extraction
already knows; leave edges it cannot establish absent, never guessed.

Deliverable: a ResearchState that round-trips through serialisation
byte-identical, and the golden report test green.

PART B — Tier and ConsentRecord (§2.2)

Migration: consent_records table, append-only, one row per manuscript
per consent event, with scope as a bitset over the six ConsentScope
values, provider, statement_version, consented_at.

Types: enum Tier { Free, Premium(ConsentRecord) }. The premium proxy
client takes Tier::Premium only. There must be NO function that
produces Tier::Premium from anything other than a persisted
ConsentRecord row.

Extend the release gate with a sixth invariant, TIER: every payload
that carries manuscript text carries a consent id; every consent id in
a payload resolves to a stored row whose scope covers what was sent.
Both directions.

Deliverable: a test that constructs a free-tier run and asserts it
cannot reach the premium route — as a compile-time or type-level
failure where possible, a runtime refusal where not. Run the negative
control: temporarily add a constructor path and show the test fails.

Do NOT build any premium feature. This phase is the boundary and the
object the rest routes over.

Then: predict which existing tests the ResearchState change will
break before running the suite. Report the prediction against the
result. Report where the design document was wrong about the code.
```

---

## Prompt 3 — Phase 2: the harness and the mathematical engine

```
Phase 2 of docs/publishready-premium-architecture.md. Three parts.

PART A — Make one agent revise in production (§5.1)

Wire RevisingVerificationAgent into pipeline.rs in place of the
precomputed verification participant. Run the golden manuscripts.
Confirm rounds_run > 1 and revised_agents non-empty on at least one.

Pin it: a test that fails if the production pipeline converges in
round one on a fixture designed to provoke revision. Run the negative
control — swap the precomputed agent back and show the test fails.

Until this test is green, the debate is a vote. Do not proceed to
Part B until it is.

PART B — AgentSpec and the graph (§4.3, §4.4)

Define AgentSpec with every field in §4.3, including requires,
evidence_policy and timeout. Define the graph as a data file
(.ron or .json) validated at build time: no cycles; every reads names
a layer that exists; every Cloud agent has a Tier::Premium gate
upstream; every hard_constraint agent has model: None; every agent
declares its trust tier per §4.4.

Move the six existing lanes onto the graph without changing their
behaviour. Golden report test from Phase 1 must stay green.

Add disagreement-driven rounds (§5.3): opinions differing beyond a
threshold on the same claim go to a second round; only those.

Deliverable: the graph file, its validator, and a test that a
deliberately invalid graph (a cycle; a cloud agent with no gate) fails
validation with the reason named.

PART C — The mathematical verification engine, Tier 0 only (§6b)

Equation extraction from LaTeX (the .tex exporter already handles it),
OMML (the .docx exporter round-trips it), and PDF where text
extraction yields it. Canonicalise into EquationGraph nodes with
variables, units where stated, and the manuscript span.

Implement the checks that need NO analysis record: symbolic
equivalence between two occurrences of the same quantity; dimensional
consistency where units are present; recomputation of standard
statistics (SE, CI, t, effect sizes) from inputs the manuscript itself
reports. Every output is a finding with EpistemicStatus and a
verification trail. No LLM anywhere in this path — assert it
structurally, the way the startup module asserts no HTTP client.

Deliverable: the first Tier 0 finding on a golden manuscript, with the
manuscript span and the recomputation shown. Then a negative control:
a fixture where the reported statistic IS consistent produces no
finding.

Do NOT build the analysis-record parsers yet (Phase 4). Do NOT build
any cloud agent. Report where the design document was wrong about the
code.
```

---

## Prompt 4 — Phase 2b: the benchmark

```
Phase 2b of docs/publishready-premium-architecture.md, §6c.1. This
runs alongside Phase 3 and is never finished — it grows.

Build the Gaply Research Reliability Benchmark as a labelled set in
src-tauri/evals/grrb/, one JSONL per family: mathematical, statistical,
manuscript-consistency, literature, journal, adversarial. Each case:
the input (a manuscript fragment or a synthetic ResearchState), the
expected finding (or expected absence), the family, and a population
estimate for weighting.

Reuse the audit's tooling as the template: label-cn's stratified
population design, weighted rather than pooled rates, per-stratum
floors, the ai-eval harness's --json headSha discipline. Do not build
a second labelling tool.

First fifty cases, at least five per family. Every case has a
provenance note: hand-written, adapted from a real manuscript (which,
anonymised how), or synthetic.

The evaluation matrix (§6c.2): implement the columns as computed
outputs of a benchmark run. State NO target values. The first run
against the Phase 2 Tier 0 engine sets the baseline; the record says
what it was.

The promote/hold/rollback gate (§6c.3): a script that takes two
benchmark reports and classifies the diff. Wire it as a pre-commit
check for any change under the agents/ or harness/ paths, the way the
--workspace gate already guards those paths.

Deliverable: the baseline run committed, and the gate demonstrated
with a deliberate regression — change a Tier 0 check to be wrong,
show the gate says ROLLBACK, revert.
```

---

## Prompt 5 — Phase 3: the journal layer

*This is the phase the product's differentiator rests on. `journal_guidelines` has had zero rows across 27 manuscripts. It gets the most detailed prompt.*

```
Phase 3 of docs/publishready-premium-architecture.md, §3.4 and §7.
Backend and frontend. Build the JournalFingerprint end to end for TEN
journals, and prove each part on a real one.

═══════════════════════════════════════════════════════════════
BACKEND
═══════════════════════════════════════════════════════════════

1. THE OBJECT. JournalFingerprint in gaply-core, with three parts that
   share NO fields and are stored in separate tables:

   journal_requirements   — from the guidelines. Each row: kind
                            (word_limit | section_required |
                            reference_style | data_policy |
                            reporting_standard | figure_limit | other),
                            value, source_url, source_span (the exact
                            guideline sentence), fetched_at.
   journal_conventions    — from the recent-paper corpus. Each row:
                            metric (length | section_set | figure_count
                            | methods_position | stats_style), the
                            distribution (median, IQR, n), corpus_run_id.
   journal_expectations   — from editor statements and public reviewer
                            guidance. Each row: claim, frequency (k of n
                            where measurable), source_url, source_span.
                            NEVER stored without a source.

   A finding cites exactly one of the three. A test asserts no code
   path reads two of them into one finding.

2. THE DEEP CRAWL. A pasted URL is an ENTRY POINT, not a page. A
   journal's requirements are spread across author guidelines,
   article-type pages, submission checklists, formatting, ethics, data
   availability, reporting standards, trial registration, AI-use
   policy, COI, preprint and licensing pages, and downloadable
   templates. Build:

     entry URL → canonical journal → DISCOVERY (follow links on the
     journal's own domain and the publisher's author-services domain
     whose anchor text or path names a guideline topic; depth ≤ 3;
     hard cap on pages) → COLLECTION → SOURCE CLASSIFICATION (which
     page is about what) → EXTRACTION → NORMALISATION → CONFLICT
     DETECTION → ARTICLE-TYPE BINDING → JournalFingerprint

   Every fetched document passes the existing injection guards
   (strip_hidden → denylist → perplexity → quarantine; do not weaken).
   The output is a SOURCE TREE. Every requirement records
   source_document, source_heading and source_span — the exact
   sentence — not just a URL. Store the tree.

   Extraction is deterministic-first: word limits, section names,
   reference styles, reporting-standard names, ethics-statement
   requirements are pattern matches. What patterns cannot reach goes
   to the local 3B and carries confidence: judged. Measure the split
   per journal.

   CONFLICT DETECTION: when two sources state different values for
   one requirement (abstract 250 on one page, 300 on another), store
   BOTH, mark the fact CONFLICTED, show both to the user, choose
   neither. A test seeds a conflict and asserts neither value is
   silently preferred.

   ARTICLE-TYPE BINDING: requirements that apply only to one article
   type (original research vs review vs short communication) are bound
   to it. A manuscript is checked against its type's requirements.

   Deliverable: journal_requirements has rows for ten journals with
   source trees. Paste the count per journal, per kind, per source
   document. Paste one CONFLICTED fact. Paste one full row with its
   source_span so I can check it against the live page. Paste the
   discovery log for one journal: which pages were followed, which
   were refused, and why.

2b. EVERY JOURNAL FACT CARRIES A STATUS, stored and displayed:
   VERIFIED (from the guidelines, span recorded) · INFERRED (from the
   corpus, with n) · UNAVAILABLE (searched, nothing found) ·
   CONFLICTED (sources disagree). The product never flattens to "the
   journal requires…". A test asserts every requirement row has a
   status and every rendered fact shows it.

3. COMPARABLE CORPUS. For each journal, fetch recently PUBLISHED
   papers' metadata and abstracts via OpenAlex (public; no manuscript
   involved; the Citation Manager's DOI-fetch machinery is the
   template). Then FILTER to comparable papers at analysis time: same
   article type, same or adjacent study design, within a date window,
   above an embedding-similarity threshold to the ResearchState. The
   corpus has a minimum (below which conventions are UNAVAILABLE), a
   target, and a maximum — state all three in config, not code. Store
   title, abstract, publication date, section structure where OA full
   text allows, and NOTHING from any user manuscript.

   Call it "recently published." Acceptance dates are not in the
   source; do not name them. A test asserts the word "accepted" does
   not appear in convention output.

   Derive conventions: median and IQR of length; section-set
   frequency; figure count; methods-before-results frequency;
   statistical reporting style (p-values vs CIs vs both, by frequency).
   Each with n.

   Deliverable: journal_conventions populated for ten journals. Paste
   the length distribution for two of them and say whether it matches
   what you would expect from reading those journals.

4. EXPECTATIONS. For journals with public reviewer guidelines or
   editor statements, ingest them through the same guarded path. For
   the rest, nothing — an empty expectations table is honest; an
   expectation without a source is not. Do NOT use the cloud research
   agent yet (that is Phase 4); this phase is deterministic ingestion
   only.

5. REPORTING-STANDARD BINDINGS. From the requirements, extract which
   standard (CONSORT, PRISMA, STROBE, ARRIVE, TRIPOD, CHEERS) the
   journal binds to which study design. Store as
   journal_standard_bindings(journal, design, standard, source_span).
   Then the FIVE most common standards as deterministic checklist
   evaluators over ResearchState — one evaluator per standard, each a
   list of items with the ResearchState field(s) it checks.

6. VERSIONING AND CACHE. Every fingerprint carries a version and a
   fetched_at. The cache is content-addressed and shared — the same
   journal serves every user. A profile that fails the injection
   guards is quarantined for all users, with the reason logged.
   Re-fetch policy: quarterly, or on user request.

7. THE CHECKLIST. build_checklist currently produces structural-only
   output because it has nothing to read. Point it at
   journal_requirements and the standard bindings. Deliverable: a
   checklist that has run against a real journal's actual requirements
   once, with the output pasted, and the two "0 rows" tables from the
   teardown no longer at 0.

═══════════════════════════════════════════════════════════════
FRONTEND
═══════════════════════════════════════════════════════════════

8. THE INVARIANT FIRST: backend extracts, frontend displays. The
   frontend never fetches a journal page, never calls OpenAlex, never
   touches an external API. It asks the backend for a fingerprint and
   renders it. Write the structural test before writing any screen:
   no fetch() to a non-app origin under src/screens/publishready/**.
   Run the negative control — add one, watch it fail, remove it.

9. JOURNAL PICKER. On the PublishReady screen: pick from the ten
   profiled journals, or paste a guidelines URL for one that is not.
   For a pasted URL: show what ingestion found before the run starts —
   "N requirements extracted, K by pattern, M by model" — so the user
   sees what the checklist will be built from. If ingestion found
   nothing, say so and offer to proceed structural-only, labelled as
   such.

10. THE FINGERPRINT VIEW. One screen showing the three parts as three
   distinct sections that look different — requirements as rules with
   their source sentence, conventions as distributions with n,
   expectations as cited claims. Every line shows its status
   (VERIFIED / INFERRED / UNAVAILABLE / CONFLICTED) and links to its
   source document and heading, not just the journal's homepage. A
   CONFLICTED fact shows both values. Nothing on this screen is a
   model's opinion.

11. PROVENANCE ON THE RUN. The pre-run card states which fingerprint
    version the analysis will use and when it was fetched. The report
    states the same. A re-run against a newer fingerprint says the
    fingerprint changed and what changed.

12. THE CHECKLIST DISPLAY. Each checklist item shows which requirement
    it came from (with source sentence) and which ResearchState field
    it checked. A structural-only item (no journal data) is labelled
    structural. The teardown's "6 findings, 3 of which were agents
    saying they had nothing to say" pattern must not recur here: a
    passed item is a passed item, not a finding.

═══════════════════════════════════════════════════════════════
DISCIPLINE
═══════════════════════════════════════════════════════════════

- Every fetch goes through the existing injection guards. Add the
  multilingual denylist entries and the structural imperative-mood
  rule from §11, each with a test that fires on a phrase NOT already
  in the list.
- No manuscript text touches this phase. Assert it: the journal
  module's dependency graph must not reach the manuscript layer.
  Structural test, like the no-HTTP-client-in-startup one.
- Stage by path. One commit per numbered item above. Push and confirm
  CI after items 2, 3, 7 and 12.
- Predict, before running: which of the ten journals will yield the
  fewest pattern-extracted requirements, which will produce a
  CONFLICTED fact, and why. Compare to the result.
- Report where the design document was wrong about the code, and
  which of the ten journals has guidelines the guards quarantined.
```

---

## Prompt 6 — Phase 4: the clusters

```
Phase 4 of docs/publishready-premium-architecture.md, §4.1–4.2. Build
specialists as nodes on the Phase 2 graph. Start with the two clusters
that cover most submissions.

PART A — Analysis-record parsers (§1)

Per-type parsers producing a normalised AnalysisRecord: which tests
were run, on what variables, with what parameters, producing what
outputs. Start with Python (pandas/scipy/statsmodels), R, and SPSS
output files. Read-only extraction; assert std::process::Command stays
absent from shipped code.

Deliverable: an AnalysisRecord from a real script, pasted, and the
Tier 0 engine from Phase 2 now binding equation inputs from it —
the t = 3.43 vs 2.91 case from §6b.2 reproduced on a fixture.

PART B — Methodological soundness cluster

Frequentist and ML specialists first. Each is an AgentSpec node with
requires: [AnalysisRecord], declared layers, an evidence_policy, and
a trust tier of 2. The local 3B is a pre-filter only; cloud judgement
is the verdict, behind Tier::Premium.

Each specialist ships with a benchmark score from Phase 2b or its
output is marked unmeasured in the report. No exceptions.

PART C — Reporting standards cluster

The five Phase 3 evaluators as Tier 1 nodes, hard_constraint: true,
model: None.

PART D — Journal fit

Embedding distance from ResearchState to the scope profile, with the
nearest recent papers named. Tier 2. The narrative around it is cloud
and Tier 3; the distance is deterministic.

Do NOT build synthesis, the marked-up manuscript, or chat. Report the
benchmark scores for every specialist built, the ones that scored
below baseline, and what you did with them.
```

---

## Prompt 6b — Phase 4b: reviewer lenses, novelty, significance, claim strength

```
Phase 4b of docs/publishready-premium-architecture.md, §4.5 and §4.6.
The criteria come from the reviewer-criteria document committed
alongside the architecture ("What Reviewers of Top-Quartile Journals
Evaluate"); read it in full first.

PART A — ReviewLens (§4.5)

Define ReviewLens with id, criteria, reads (specialist ids),
evidence_policy, severity_policy, journal_context. Build the six:
methodology, statistics, novelty_literature, journal_fit,
reporting_ethics, general. For each, the criteria list is lifted from
the reviewer document — its Checklist / Strong / Weak / Action
structure per criterion — and the severity_policy is its risk table
(fatal / fixable / minor per criterion). Do not invent criteria.

Each lens produces a reviewer report in the real shape: overall
assessment, contribution summary, strengths, major concerns (each with
a verification trail), minor concerns, journal-specific compliance,
required revisions. Lenses run in parallel and do NOT see each other's
reports until the disagreement map.

Deliverable: six independent reports on a golden manuscript. Paste the
methodology and statistics reports in full. Then a negative control:
the same manuscript with its statistics section replaced by a correct
one — the statistics lens's major concerns must drop, the methodology
lens's must not change.

PART B — Novelty, claim by claim (§4.6)

Extract every novelty claim from the ResearchState ("first to…", "no
prior study…", "novel…"). For each, retrieve the closest prior work
through the external-evidence layer (OpenAlex, then OA full text where
available). Compare. Emit per claim: the claim, nearest prior work
with what it showed and under what conditions, and NOVEL_AS_STATED /
NOVELTY_NARROWER_THAN_STATED / PRIOR_WORK_EXISTS / UNVERIFIED. Never a
number.

Deliverable: one claim on a golden manuscript correctly narrowed
against a real prior work you can name, with the retrieval trail.

PART C — Significance, separately

Tier 3, cloud, evidence_policy requires the comparable corpus. Output:
what changes if the contribution holds — field, practical,
theoretical — relative to what this journal publishes. Kept apart from
novelty in the data model and the report; a test asserts a paper can
score high on one and low on the other.

PART D — Claim–evidence strength

For every major claim, trace claim → analysis → result → conclusion
through the evidence graph and classify SUPPORTED / PARTIALLY /
UNSUPPORTED / CONTRADICTED / UNVERIFIED. Implement the causal-overclaim
check the reviewer document names: a design that supports
"associated with" and a conclusion that says "causes" is a Tier 2
finding with both sentences and the design limitation cited.

Deliverable: a fixture with one causal overclaim flagged, and its
control — the same conclusion reworded to "associated with" — not
flagged.

Report where the design document was wrong about the code.
```

---

## Prompt 7 — Phase 5: the editor and the marked-up manuscript

```
Phase 5 of docs/publishready-premium-architecture.md, §5.7, §8 Stage 1
and §9.

THE EDITOR (§5.7). A distinct layer over the six reviewer reports, the
disagreement map, the fingerprint and Tier 0/1 findings. It applies
deterministic severity precedence from the reviewer document's risk
table: a fatal flaw (missing required ethics approval; Tier 0
contradiction in a primary result; no control) is BLOCKING regardless
of surrounding strengths, and the editor cannot demote it. A test
seeds one blocking finding among twenty strengths and asserts the
posture is not Accept.

EDITORIAL POSTURE (§8 Stage 1). One of Accept / Minor revision / Major
revision / Reject, as a RECOMMENDATION. With it: blocking / major /
minor counts, each linked to its findings, and the top three primary
causes each naming its lens and evidence. NO percent sign. Write the
test that fails if a probability is ever emitted and cite §8 Stage 3
in it. Reviewer reports and the editorial verdict are separate
artefacts with separate voices: a reviewer is concerned; the editor
recommends.

The marked-up manuscript (§9): anchored findings using the audit's
existing locator work; EpistemicStatus on every finding; the reviewer
verification trail on every major one; severity by consequence.
Four exports and no more: annotated PDF (and tracked-changes .docx for
.docx sources), reviewer letter, rubric, audit-trail JSON.

The reviewer letter goes through the proxy behind Tier::Premium. If
the proxy is unreachable, the screen says so before the run (Phase 0
item 6).

Open every export in its real consumer before claiming it works. The
.docx in Word, the PDF in Preview or Acrobat, the JSON in a validator.
The teardown found a PDF that CoreGraphics accepted and Spotlight
could not count pages in; find out what you are actually shipping.

Report where the design document was wrong about the code.
```

---

## Prompt 8 — Phase 6: the chat

```
Phase 6 of docs/publishready-premium-architecture.md, §10.

Verdict layer only. A structural test asserts the chat module cannot
reach the manuscript or analysis layers. Every number comes from a
query; the model phrases. Four modes: explain, evidence, correction,
challenge.

Challenge feeds the decision ledger: a Decision row with what, why,
user-confirmed, evidence pointer, affected ResearchState fields. The
affected subgraph re-runs via Phase 2's incremental re-analysis. The
finding moves to CONFIRMED with the decision cited. The chat cannot
change a finding any other way — pin that.

Multilingual: names, numbers and quoted text verbatim; explanation
translated. A structural check that stored identifiers appear
unaltered in every reply, in every language tested.

Report where the design document was wrong about the code, and give
me the click path for a full premium run end to end.
```

---

## Prompt 9 — Phase 7b: red team

```
Phase 7b of docs/publishready-premium-architecture.md. Before any
researcher sees this. Build adversarial fixtures and run the full
premium pipeline on each:

- a PDF with an injection in white text and one in a figure caption
- a journal guideline page that instructs the reader to ignore prior
  instructions (must be quarantined, journal marked UNAVAILABLE, user
  told why)
- a table whose totals do not sum, designed to look like they do
- a "first to demonstrate X" claim that is false, with the real prior
  work in OpenAlex
- an SPSS output that contradicts the manuscript's primary result
- a manuscript whose conclusion says "causes" on a cross-sectional
  design
- a manuscript that satisfies every VERIFIED requirement and has a
  blocking Tier 0 error — posture must not be Accept

For each: the finding produced, its status, its trail. Any wrong
CONFIRMED is a Phase 7b failure that blocks release. Any silent
absence — the pipeline said nothing where it should have — is the
same.

Report the results as a table, and report where the design document
was wrong about the code.
```

---

## The rule for every prompt

Three things, every time, because they are what caught every wrong finding this month:

1. **Predict the failure before you claim the gate.** A test that is green on its first run proves nothing. Break the thing on purpose, watch the test go red, revert.
2. **Run the negative control.** Reproduce the wrong behaviour first. Then fix. Then show the same input produces the right output.
3. **Report where the design is wrong about the code.** The document was written from a description of the tree. Four times in one week a trace was accurate about a mechanism and wrong about whether it was the one running. The first job of every phase is to find where the document has already drifted.
