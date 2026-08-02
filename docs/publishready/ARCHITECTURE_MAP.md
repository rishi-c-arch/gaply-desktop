# PublishReady Architecture Map

*One page · traced at `36ea74d`, B0 resolved at `83f192c` · Coverage confidence: **Medium** · Full detail: [ONTOLOGY.md](./ONTOLOGY.md)*

---

## Verified capabilities (in the shipping PublishReady report)

Parsing → IMRaD split (22 heading phrases) → stats / citation / table extraction · 5-rule statistical validator · citation existence + retraction + gated LLM verdict · lexical-cosine plagiarism · guideline checklist (4 structural + 4 guideline-derived) · AI-authorship signal · stylometry, table and citation-recency findings · Evidence Model + Store · gated cloud reviewer · provenance trail in the report UI (`ReportViewerPage.tsx:184`)

## Missing capabilities

Figure detection · grammar/readability · internal consistency · ethics / IRB / data-availability · **manuscript topic representation** · claim extraction · novelty · significance · literature coverage · journal fit *(within PublishReady)*

## Built but NOT wired into PublishReady

Verified by execution path — grep across the whole `run_publishready` body (`commands.rs:505-680`) returns **0** for all of these:

`plagiarism_exact` · `stats_verify` / `stats_verdict` · `journal_registry` scope card · `gap_finder` fit lane

## Computed then discarded

`shadow_reviewer` (`commands.rs:677`, never read by the UI) · `Enrichment.citation_count` (`refverify.rs:846`) · `ExistenceCheck.matched_year` (`refverify.rs:661,712`)

---

## Dependency graph — leverage

```
docparse ──▶ sections ─┬─▶ stats ──▶ validate ─────────▶ D3 D4
 (all 13)    (7 dims)  ├─▶ citations ─▶ refverify ─────▶ D6 D7
                       ├─▶ tables ────────────────────▶ D9
                       └─▶ stylometry ────────────────▶ D8

embed (bag-of-words) ─┬─▶ plagiarism ─────────────────▶ (weak, B1)
                      └─▶ rag ─▶ checklist ───────────▶ D13

[MISSING] topic  ──▶ D1 D2 D6 D10    ← blocks 4 dimensions
[MISSING] claims ──▶ D1 D5 D6        ← blocks 3 dimensions
```

---

## Bottlenecks — ranked by quality gain ÷ effort

| # | Bottleneck | Type | Effort |
|---|---|---|---|
| ~~**B0**~~ | ~~Six plagiarism strings claim more than the evidence supports~~ — **resolved `83f192c`** | Accuracy correction | Done |
| **B6** | `plagiarism_exact` built and unwired; pipeline ships the weaker engine | Engineering | **Very low** |
| **B7** | Four fetched/computed-then-discarded signals | Engineering | **Very low** |
| **B1** | `HashEmbedder` is bag-of-words, not semantic — the impl wired into the traced path | Engineering | Low–Med |
| **B4** | `ConfidenceKind` is per-agent; can't express per-finding provenance | Engineering | Low |
| **B2** | Heading vocabulary lacks Limitations / Data Availability / Ethics / Funding | Engineering | Low |
| **B3** | No topic representation — blocks 4 dimensions | **Research** | Unbounded |
| **B5** | Escalation endpoint absent server-side | Server | Med |

---

## Tier roadmap

- ~~**T0 — accuracy corrections:** fix the plagiarism strings to match the evidence~~ — **done, `83f192c`**
- **T1 — deterministic:** wire `plagiarism_exact` · ethics/data-availability detection · figure detection · aggregate citation counts · surface-or-drop `shadow_reviewer`
- **T2 — manuscript-internal:** real embedder · readability · extend heading vocabulary · internal consistency
- **T3 — literature-grounded (research):** topic → coverage → fit → novelty *(spike each with kill criteria first; significance recommended for permanent exclusion)*
- **T4 — synthesis:** promote the deterministic verdict · escalation endpoint + Q12 spike

---

## Highest-leverage next steps

1. ~~**Correct the plagiarism labels.**~~ **Done — `83f192c`.** `"paraphrase"` named precisely what a bag-of-words engine cannot detect.
2. **Wire `plagiarism_exact`.** The trustworthy engine is already built; the report ships the weaker one.
3. **Aggregate the discarded signals.** Pure arithmetic over data already fetched or computed.
4. **Spike topic extraction from non-prose signals** before committing to any Tier 3 dimension.

---

## Known limits of this map

Coverage is **Medium**, not High: `PlagiarismSession::{ingest_manuscript, report}`, `rag::search`, and `RefVerifier::verify` internals remain untraced. Tracing `PlagiarismSession::report` would most increase confidence — it determines whether the B0 correction (`83f192c`) was the whole problem or whether chunking/KNN adds further distortion.
