# Citation Ground Truth — Protocol v1

**Status:** FROZEN. Counted by hand from the **original PDF** (rendered pages), not from `ExtractionResult` output, before any extractor output was inspected in this workstream.

| | |
|---|---|
| Protocol | **v1**, sha256 `858515eedd637fc881b7e20c2ada17d2553fd20633bd9326bcbf5a0635115112` |
| Manuscript | `IJAS Manuscript JHA Bombyx haemolymph (1).pdf`, sha256 `859880647c4579c34bc63b82c2280ab08bceac917a9547e0c839a821bc5bafd7` |
| Counted | 2026-08-03 |
| Repository | `e854e85` |
| Counter | Claude (also the implementer — bias acknowledged, see §5 of the protocol) |
| Source | PDF pages 2–11 (body). Pages 11–13 bibliography read for attribution only (E1). Page 1 is the AI-detection report cover (E5). |

---

## 1. Summary

| Quantity | Count |
|---|---|
| **True citation occurrences (scored)** | **31** |
| **Ambiguous — not scored (E2)** | **0** |
| **Ambiguity rate** | **0.00** |
| Distinct `(surname, year)` keys | 26 |
| Bibliography entries in the PDF | **26** |
| Bibliography entries with ≥1 citation | **26** |
| **Bibliography entries never cited** | **0** |

**Every reference in this manuscript is cited.** Any non-empty output from the uncited-reference finding on this manuscript is therefore a false positive by construction. This is consistent with the earlier precision-0.00 run and was established here independently, from the PDF.

---

## 2. Scored occurrences

Style: `P` = parenthetical, `N` = narrative. `→` gives the bibliography entry (§3 numbering).

| # | Page | Occurrence | Style | → |
|---|---|---|---|---|
| 1 | 3 | (Slama and Williams 1966) | P | 23 |
| 2 | 3 | (Trivedy *et al.* 1993, … | P | 26 |
| 3 | 3 | … Kamimura and Kiuchi 1998, … | P | 11 |
| 4 | 3 | … Miranda *et al.* 2002, … | P | 16 |
| 5 | 3 | … Mamatha *et al.* 2006) | P | 13 |
| 6 | 3 | Bizhannia *et al.* (2005) | N | 2 |
| 7 | 3 | Etebari *et al.* (2007) | N | 7 |
| 8 | 3 | Begum *et al.* (2011) | N | 1 |
| 9 | 3 | Nair *et al.* (2009) | N | 19 |
| 10 | 3 | (Dandin *et al.* 2005) | P | 5 |
| 11 | 4 | (Etebari *et al.* 2005) | P | 8 |
| 12 | 4 | (Moore and Stein 1954) | P | 17 |
| 13 | 4 | (Dubois *et al.* 1956) | P | 6 |
| 14 | 4 | (Miller 1959) | P | 15 |
| 15 | 4 | (Carroll *et al.* 1956) | P | 4 |
| 16 | 4 | (Schmidt and Platzer 1980) | P | 22 |
| 17 | 4 | (Bradford 1976) | P | 3 |
| 18 | 4 | (Reitman and Frankel 1957) | P | 21 |
| 19 | 7 | Srivastava and Upadhyay (2015) | N | 24 |
| 20 | 7 | Nair *et al.* (2009) | N | 19 |
| 21 | 7 | Bizhannia *et al.* (2005) | N | 2 |
| 22 | 7 | Liu *et al.* (2023) | N | 12 |
| 23 | 7 | Gordon and Burford (1984) | N | 10 |
| 24 | 7 | Begum *et al.* (2011) | N | 1 |
| 25 | 7 | Suzuki *et al.* (2023) | N | 25 |
| 26 | 8 | Etebari *et al.* (2007) | N | 7 |
| 27 | 8 | Mamatha *et al.* (2008) | N | 14 |
| 28 | 8 | Kamimura and Kiuchi (1998) | N | 11 |
| 29 | 8 | Göncü and Parlak (2011) | N | 9 |
| 30 | 9 | Nair *et al.* (2003) | N | 18 |
| 31 | 10 | Rahmathulla and Suresh (2012) | N | 20 |

**By style:** 18 parenthetical, 13 narrative.
**Multi-citation groups:** one — occurrences 2–5, comma-separated inside a single `(...)`.

---

## 3. Bibliography (26 entries, PDF pages 11–13)

1 Begum 2011 · 2 Bizhannia 2005 · 3 Bradford 1976 · 4 Carroll 1956 · 5 Dandin 2005 · 6 Dubois 1956 · 7 Etebari 2007 · 8 Etebari 2005 · 9 Göncü 2011 · 10 Gordon 1984 · 11 Kamimura 1998 · 12 Liu 2023 · 13 Mamatha 2006 · 14 Mamatha 2008 · 15 Miller 1959 · 16 Miranda 2002 · 17 Moore 1954 · 18 Nair 2003 · 19 Nair 2009 · 20 Rahmathulla 2012 · 21 Reitman 1957 · 22 Schmidt 1980 · 23 Slama 1966 · 24 Srivastava 2015 · 25 Suzuki 2023 · 26 Trivedy 1993

Every entry appears in the `→` column of §2.

---

## 4. Excluded, with the rule applied

| Item | Rule |
|---|---|
| Bibliography entries themselves (pp. 11–13) | E1 |
| Taxonomic authorities: `Bombyx mori L.`, `Cullen corylifolium (L.) Medik.`, `Psoralea corylifolia L.`, `Aedes aegypti`, `Culex pipiens` | not a citation — no year, names a taxon |
| Table and figure references: `(Table 1)`, `(Tables 1 and 2)`, `(Fig. 1)`, `(Fig. 2)` | not a citation |
| Hybrid and treatment codes: `CSR2 × CSR4`, `KA × NB4D2`, `PM × NB4D2`, `C0`–`C4`, `A1`–`A4` | not a citation |
| Chemical names: `(2E,4E)-11-methoxy-3,7,11-trimethyl-2,4-dodecadienoate` | not a citation |
| Statistical notation: `p ≤ 0.05`, `NS`, `CD (J)`, `CD (C)`, `CD (T)` | not a citation |
| AI-detection report cover, guidelines, score legend (p. 1) | E5 |
| Running headers `ACADEMI.CX` / `AI WRITING REPORT`, footers `Page N of 13` | E5 |
| Footnote affiliation block (p. 2) | E5 |

**Ambiguous (E2a–E2e): none.** No name-and-year was arguable prose; no year was illegible; no citation sat inside a table or figure caption; every counted occurrence carried a year in its own sentence; every occurrence mapped to exactly one bibliography entry.

A 0.00 ambiguity rate is itself a single-manuscript observation. This manuscript is cleanly typeset author-year with no in-caption citations; a messier manuscript would not produce this.

---

## 5. Freeze

| | |
|---|---|
| Scored occurrences | **31** |
| Ambiguous, unscored | **0** |
| Bibliography entries | **26** |
| Genuinely uncited references | **0** |

**This file is frozen.** Per E6, no item may be reclassified. If one is later judged mis-annotated, that is a Protocol v1 defect and the measurement is re-run in full under v2.
