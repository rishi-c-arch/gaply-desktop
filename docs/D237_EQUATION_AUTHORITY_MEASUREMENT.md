# D237 — Equation finding reliability, measured before any verdict authority is granted

**Measurement only.** No production code changed, no verdict behaviour
changed, nothing implemented. This answers the question D236 left open: does
any equation status have enough evidence for hard verdict authority.

Baseline, not rediscovered: §11 D234, D235 and `docs/D236_VERDICT_AUTHORITY.md`.
D236 established that the validation lane's authority is too coarse, that
text-absence and lexical findings force verdicts today, that a genuine
arithmetic contradiction does not touch the verdict, and that the
architecture (§4.4) says recomputed evidence may justify a floor.

## The answer, first

* **On the six corpus manuscripts the engine emitted zero `DETECTED`
  findings.** It emitted one `REQUIRES_AUTHOR_CONFIRMATION` (correct as a
  question), one `CONFIRMED`, eighteen `UNVERIFIED` definitions, one
  consistent dimension check, and nothing else. **There is no corpus
  false-positive rate for `DETECTED`, because there is no corpus firing.**
* **Constructed inputs produced three false `DETECTED` findings out of five,
  every one Major and labelled "mathematically certain", and every one from a
  parser convention rather than from arithmetic:** a juxtaposed product
  (`2ab`) read as a third variable, `1/2x` read as `(1/2)·x`, and a
  `where`-prefixed declaration not read as a declaration so the engine bound
  the other value the manuscript gave. The arithmetic in each was exact. The
  parse was the engine's choice. That is D157's shape ("a Tier-0 finding
  resting on a value the engine chose") one layer up.
* **The only real finding the corpus produces is unanchored in the report.**
  The manuscript's line carries U+202F narrow no-break spaces around `×`; the
  extraction's paragraph has ordinary spaces; `locate_line` compares the raw
  line to the paragraph and finds nothing, so the reader gets a "requires
  author confirmation" row with no location.
* **`EpistemicStatus::Contradicted` is dead by construction.** It is §9's
  vocabulary copied into a type (commit `99de714`), consumed by two match
  arms, produced by nothing. The authority question does not apply to it.
* **Classification (§5):** `DETECTED` is **advisory only** on today's
  evidence, with a stated path to "safe to set a floor under conditions";
  `CONTRADICTED` is **not applicable** (no producer);
  `REQUIRES_AUTHOR_CONFIRMATION` is **author-confirmation only** by
  construction; the dimension `Inconsistent` finding is **advisory only**.
  No status has enough evidence for hard verdict authority today.

## Provenance

* HEAD `8e5052d6226c3c8f86c6c25bc41ae1c2b531eacd` (the D236 commit), 26 Sep
  2026 22:58Z. `src-tauri/` clean; only `public/sitemap.xml` modified.
* `[probe]` = a throwaway app-crate example, not committed, sha256
  `a74236beaa84f6faa3030018797c41750287cb31ae7d488c817fc937bc95ddbf`, release
  build. Four modes: `cases` (constructed lines through
  `equation::graph::graph_from_lines`, `check::check_equation`,
  `units::check_sides` AND through `pipeline::run_pipeline_measured` for the
  report and verdict), `corpus` (the six manuscripts through the real
  pipeline, every node and claim listed), `dump` (every line with `=`, a digit
  and an operator, for the false-negative hand-read), `locate` (anchoring).
  Pipeline runs: `GAPLY_DISABLE_DEEP=1`, consent denied, `HashEmbedder`,
  in-memory database.
* Manuscripts: the six of D215, readable, sha256 prefixes re-verified
  identical to D215's table (`134956ff…`, `77e91296…`, `85988064…`,
  `af1f36ff…`, `effbb86c…`, `de1e322a…`).
* Controls: A3 (Slovin's formula, the engine's documented negative control)
  produced no finding; H2 (the 21.9% chain, the engine's documented positive)
  produced `REQUIRES_AUTHOR_CONFIRMATION`. Both read as expected before any
  other row was read.
* Who hand-read: Claude, from the probe output and the raw `docparse` lines.
  **Nobody else has checked any row below.**

## 1. Every status and its producer

Source: `equation/check.rs` `check_claim` (lines 213–406),
`epistemic.rs` `Agreement::status` (165–171), `equation/units.rs`
`check_sides` (372–421), `equation_report.rs`, `stats_verdict.rs` `[src]`.

| status | producer | what is computed | evidence source | values recomputed? | external data? | tolerance? | independently disprovable? |
|---|---|---|---|---|---|---|---|
| `Confirmed` | `check_claim` steps 2–3, and `equiv::Identical` | both sides evaluate to one exact rational; or the reported literal is the computed value rounded at its displayed decimals; or the two trees are syntactically identical | the line's own numbers plus bindings (unification first, then bare declarations in the following block) | yes, exact rationals | no | display rounding only: the reported side's own decimal count | yes: the trail lists each step; a reader re-does it |
| `Supported` | `equiv::AgreesAtEveryPointTested` | an identity agreed at 5 sampled points | the two sides' structure | no: sampled, "evidence, not a proof" | no | none | yes, by algebra |
| `Unverified` | `check_claim` step 1 (definition: sides share no variable), unbound names, non-computable rounded reading, `equiv::Undetermined` | nothing | n/a | no | no | none | n/a; it asserts nothing |
| `Detected` (numeric) | `check_claim` step 4 via `Agreement::of(false, false)` | both sides evaluate; they differ as written; the rounded-interval readings (each displayed decimal ± half its last place) do not overlap | the line's numbers plus bindings | yes, exact | no | the `AsRounded` reading IS the tolerance model: intervals from the displayed precision, not a parameter | the arithmetic cannot be disproved; the **parse** and the **binding** can (cases E2, G1, G2b below), and the finding quotes both sides so a reader can see a misread |
| `Detected` (symbolic) | `equiv::Differs { at, … }` | a witness point where the two sides differ | the two sides' structure under the parser's identifier rule | a witness is evaluated, not the manuscript's numbers | no | none | yes, by algebra; case G1 is a correct identity the witness "refutes" |
| `RequiresAuthorConfirmation` | `Agreement::of(false, true)` and the impossible `(true, false)` | as written unequal; rounded intervals overlap | as `Detected` | yes | no | the interval reading | the author's answer decides it |
| `Contradicted` | **none** | nothing | n/a | n/a | n/a | n/a | n/a |
| dimension `Consistent` | `units::check_sides` | both sides derive the same dimension vector and basis | label annotations with a marker (`/`, `as`, `%`) and prose `name = meaning (unit)` declarations | units, not values | no | none; **magnitude is not checked** (`mL` and `L` are both L³) | yes, by unit algebra |
| dimension `Inconsistent` | `units::check_sides` | dimensions or bases differ; or an additive mismatch | as above | units only | no | none | yes; the finding's own detail names its false mode: "a constant's units were never stated in the text" |
| `stats_verdict::Verdict::Match` / `Mismatch` | `stats_verdict::verify_analysis` | a test statistic and p-value recomputed from the user's uploaded table, compared with the reported value | **the user's data**, and a user-typed `AnalysisSpec` | yes | **yes** | `±0.005 + 1%·|value|` on the statistic, `±0.005` on p, stated in the result | yes, by re-running the test; not on the pipeline path (D236) |

### What `Contradicted` is

* **Origin.** Commit `99de714` (§11 D156) created `EpistemicStatus` as "§9's
  vocabulary, verbatim": six statuses including `Contradicted`, doc comment
  *"Established to be false."* The architecture's own example (line 1272)
  says the N-mismatch case is *"DETECTED / REQUIRES_AUTHOR_CONFIRMATION, never
  wrong"*, which leaves nothing for `Contradicted` to name.
* **Producers: zero.** `grep -rn "EpistemicStatus::Contradicted"` over
  `gaply-core/src` and `src` returns two consumers (`equation_report::severity_for`
  maps it to Major; `status_slug` renders it) and one test asserting it
  differs from `Detected`. `check_claim` assigns every other status and never
  this one. `is_finding()` returns true for it, so a producer added tomorrow
  would ship a Major finding under a name no test has ever pinned the meaning
  of.
* **A namesake elsewhere.** `specialist/claim_strength.rs` has its own
  `Contradicted` variant, documented *"A judge found the analysis contradicts
  the claim. Not reachable."*, because the evidence graph emits co-location
  only and refuses to assert meaning (§4.6's own text). Different type, same
  state: reserved, unproduced.
* **Classification: dead by construction.** Not "unreached" (there is no
  path that could reach it under some input) and not merely "reserved" (no
  record says what would produce it). **The authority question does not
  apply to a status nothing produces.** If a producer is ever written, it
  needs its own definition of what "established to be false" adds over
  `DETECTED`, and the only candidate in the product is a recomputation
  against external data (`stats_verdict::Mismatch`), which lives on a
  different screen.

## 2. Ground-truth cases `[probe]`

Every line was placed in the same five-section manuscript body, in Results,
and run both at the engine (statuses, values, readings) and through the
pipeline (report row, verdict). "Expected" is what a careful reader would
say before seeing the engine. Verdict was `pass` with
`overridden_by_constraint = false` in every case; no equation finding moved
it (D236's finding, re-confirmed on 30 inputs).

| case | line(s) | expected | actual status | values / readings | report | assessment |
|---|---|---|---|---|---|---|
| A1 correct | `Total = 2 + 3 = 5` | CONFIRMED | claim#0 UNVERIFIED (definition of `Total`), claim#1 **CONFIRMED** | 5 = 5 | none | correct |
| A2 correct identity, explicit operators | `(a + b)^2 = a^2 + 2×a×b + b^2` | SUPPORTED | **SUPPORTED** | agreed at 5 points | none | correct |
| A3 Slovin, known-good | the six-line block from `graph.rs` tests | CONFIRMED, no finding | **CONFIRMED ×3**, `N = 237000`, `e = 0.04` bound by unification; `623.36` accepted as the 2-dp rounding of `623.35613…` | | none | correct; the documented negative control |
| B1 contradiction | `Total = 2 + 2 = 5` | DETECTED | **DETECTED** | 4 vs 5; AsWritten FAILS, AsRounded [4,4] vs [5,5] FAILS | Major "Arithmetic detected", Results¶0 | correct; verdict `pass` |
| B2 chain, last link | `Ratio = 10/4 = 2.5 = 2.6` | DETECTED (2.5 ≠ 2.6) | 10/4 = 2.5 **CONFIRMED**; 2.5 = 2.6 **REQUIRES_AUTHOR_CONFIRMATION** | AsRounded [2.45, 2.55] and [2.55, 2.65] **touch at 2.55 and count as overlapping** | Minor | conservative: an underlying 2.55 could display as either; not a false positive, but a reader would call this a mismatch |
| B3 false identity | `(a + b)^2 = a^2 + b^2` | DETECTED | **DETECTED** | at a = 2, b = 3: 25 vs 13 | Major | correct |
| C1 rounding only | `Mean = 10/3 = 3.33` | CONFIRMED | **CONFIRMED** (display rounding at 2 dp) | 3.33333… → 3.33 | none | correct |
| C2 rounding only | `Share = 2/7 = 0.29` | CONFIRMED | **CONFIRMED** | 0.28571… → 0.29 | none | correct |
| D1 unit label without a marker | `m (mg) = c × V` | a node | **parse error** `UnexpectedToken("(")` | | none | fixture: `split_label_unit` accepts a label unit only with `/`, `as` or `%` in it; a bare `(mg)` is refused as possibly arithmetic. Documented rule, not a defect; the case was re-shaped |
| D1b unit conversion, dimensionally consistent | `Conc (mg/L) = m / V` + `where m = mass (mg); V = volume (mL)` | no finding, and the 1000× scale error invisible | units **CONSISTENT** `M·L⁻³`; arithmetic UNVERIFIED (definition) | | none | **magnitude blind by design** (D158): `mL` and `L` are the same dimension |
| D1c same, same scale | … `V = volume (L)` | CONSISTENT | **CONSISTENT** | | none | correct |
| D2 unit conversion, numeric | `Distance = 1.5 km = 1500 m` | UNVERIFIED (unknown units) | **parsed as `Distance = 1.5`** (the parser keeps the longest parseable prefix); not a node (no variables on the right); not a declaration (whole-line rule) | | none | **silently dropped**: no node, no refusal, no row. A unit-conversion claim is neither checked nor recorded as unchecked |
| D3 / D3b extra division | `Conc (mg/L) = m / V / V` | INCONSISTENT | **INCONSISTENT** `M·L⁻³` vs `M·L⁻⁶` | | Major "Units do not balance", Results¶0 | correct; verdict `pass` |
| D4 addition of unlike units | `Conc (mg/L) = m + V` | INCONSISTENT | **INCONSISTENT** M vs L³ | | Major | correct |
| E1 insufficient information | `n = N/(1+N×e^2)` | UNVERIFIED | **UNVERIFIED** (definition); 3 refusals "no declaration in scope" | | none | correct |
| E2 two declared values, one `where`-prefixed | `y = a + 2 = 5`, `where a = 3`, `a = 4` | refusal: two values (D157 condition 4) | **bound `a = 4`; DETECTED** 6 vs 5 | | **Major "Arithmetic detected"** | **false DETECTED.** `where a = 3` parses as a label `where a` (`as_label_phrase` admits up to five words) and never reaches `declaration_binding`; `unit_declaration` strips the `where ` prefix, `declaration_binding` does not. The manuscript said two things; the engine chose one |
| E2b the same, both bare | `a = 3`, `a = 4` | refusal | **refused: "2 different values are declared in the same block"; UNVERIFIED** | | none | correct (D157) |
| E3 one `where`-prefixed value | `where a = 3` | CONFIRMED | **UNVERIFIED**, `a` "no declaration in scope" | | none | conservative: a real binding lost to the same prefix rule; no false claim |
| E4 one bare value | `a = 3` | CONFIRMED | **CONFIRMED** 5 = 5 | | none | correct |
| F1 empty side | `x = = 5` | nothing | parse error `EmptySide` | | none | correct |
| F2 unbalanced | `Total = (2 + 2 = 5` | nothing | `UnbalancedBracket` | | none | correct |
| F3 prose | `Table A = the summary` | nothing | `NotAnEquation` | | none | correct |
| F4 too long | a 300-term sum | nothing | `TooLong` (600-char cap) | | none | correct |
| G1 juxtaposed product in a true identity | `(a + b)^2 = a^2 + 2ab + b^2` | SUPPORTED (it is the binomial expansion) | **DETECTED** | witness a = 2, **ab = 3**, b = 2.5: 20.25 vs 16.25 | **Major "Arithmetic detected"** | **false DETECTED.** `2ab` is `2 × ab` with `ab` a third, independent variable (the module's documented "a run of letters is one name" rule). The rule is honest about not splitting; the symbolic comparison then treats the unsplit name as free, and a correct identity is "refuted" at a witness |
| G2 precedence, `where`-prefixed | `y = 1/2x = 0.25`, `where x = 2` | ambiguous | **UNVERIFIED** (`x` not bound; the E3 rule) | | none | conservative by accident |
| G2b precedence, bare | `y = 1/2x = 0.25`, `x = 2` | ambiguous: `(1/2)·x = 1` or `1/(2x) = 0.25` | **DETECTED** 1 vs 0.25 | AsRounded [1,1] vs [0.245, 0.255] | **Major "Arithmetic detected"** | **false or ambiguous DETECTED.** The parser reads `1/2x` as `(1/2)·x`, giving 1. Under the other common convention, `1/(2x)`, the line gives 0.25 and holds. A precedence convention became a Major certain finding |
| G3 juxtaposed number and name, bound | `y = 2x = 4`, `x = 2` | CONFIRMED | **CONFIRMED** | 4 = 4 | none | correct |
| H1 author confirmation | `Total = 1.5 + 2.5 = 4.1` | REQUIRES_AUTHOR_CONFIRMATION | **REQUIRES_AUTHOR_CONFIRMATION** | 4 vs 4.1; [3.9, 4.1] overlaps [4.05, 4.15] | Minor, Results¶0 | correct |
| H2 known-good | the 21.9% chain | REQUIRES_AUTHOR_CONFIRMATION | **REQUIRES_AUTHOR_CONFIRMATION** on link 2; CONFIRMED on link 3 | 0.21968 vs 0.219; [0.208255, 0.231125] overlaps [0.217, 0.221] | Minor | correct; the documented positive control |

**Tolerance, stated once.** The arithmetic engine has no tolerance parameter.
Its two readings are: exact rationals as written, and each displayed decimal
widened to ± half its last place. Display rounding of a bare literal is
accepted before either reading runs. `stats_verdict` is the only tolerance
in the product and it is not on this path.

## 3. Real manuscripts `[probe]`

Through `run_pipeline_measured`, every node the graph built, every claim's
status, every dimension verdict, then the report's equation findings.

| manuscript | text lines | nodes | claims by status | dimension verdicts | report equation findings | verdict (D235's, unchanged) |
|---|---:|---:|---|---|---|---|
| chapter3 .docx | 1386 | 14 | 14 UNVERIFIED (definitions) | 1 CONSISTENT (`Magnesium Hardness = Total Hardness − Calcium Hardness`, both `M·L⁻³ as CaCO₃`); 13 unverified (`N`, `Vtitrant`, absorbances carry no declared unit) | 0 | pass |
| final final L.pdf | 2809 | 3 | 3 UNVERIFIED (definitions; the `where` clause sits on the same line and the parser keeps the formula prefix) | unverified | 0 | concern (validation) |
| IJAS haemolymph.pdf | 137 | 0 | | | 0 | pass |
| Lake Chapter 1.docx | 230 | 0 | | | 0 | pass |
| R PAPER .docx | 756 | 0 | | | 0 | concern (validation) |
| Revised Health Economics.docx | 700 | 1 | 1 UNVERIFIED (definition of `Weighted provision`), 1 **REQUIRES_AUTHOR_CONFIRMATION**, 1 CONFIRMED | 3 consistent (dimensionless) | 1: Minor, "Arithmetic requires author confirmation: Weighted provision = …", **location None** | concern (validation) |

**Hand-read of every claim that computed something.**

* Health Economics line 548: `Weighted provision = (0.108 × 0.78) + (0.500 ×
  0.13) + (0.769 × 0.06) + (0.810 × 0.03) = 0.084 + 0.065 + 0.046 + 0.024 =
  21.9% (95% CI: 17.0–26.8%)`. The four products are 0.08424, 0.065, 0.04614,
  0.0243; their exact sum is 0.21968. The manuscript rounded each product to
  three places and summed the roundings to 0.219, then reported 21.9%. Link 2
  (`0.21968` vs `0.219`) is a rounding-of-intermediates difference, not an
  arithmetic error; the engine's status, a question with both readings shown,
  is the right one, and a reader would answer "rounding". Link 3 (`0.219 =
  21.9%`) is exactly true; CONFIRMED is correct. **Correct: 2 of 2.**
* chapter3 line 602: `Magnesium Hardness = Total Hardness − Calcium Hardness`,
  both operands declared `mg/L as CaCO₃` two formulas earlier. CONSISTENT is
  correct.
* The 18 definitions (chapter3 ×14, final-L ×3, Health Economics ×1) are
  formulas with no values in scope (`DO (mg/L) = (Vtitrant × N × 8000) /
  Vsample` and its kin). Each `UNVERIFIED` is a correct refusal: there is
  nothing to compute. D157's own accounting of this corpus (40 refusals, all
  read) covers the same lines.

**The anchor defect.** The report row for the one real finding has
`location: None`. Measured: `locate_line` with the paragraph's text finds
exactly one paragraph (Results ¶155). The raw `docparse` line the engine
parses carries **U+202F NARROW NO-BREAK SPACE** on both sides of every `×`;
the extraction's paragraph has ASCII spaces (an ASCII-space needle matched
it). `equation_findings` passes the raw `source_line` to `locate_line`, which
does `para.contains(needle)`, so the one finding a real manuscript produces
reaches the reader without a location. The constructed cases anchor
(`Results¶0`) because they were typed with ASCII spaces. Not fixed here.

**False-negative hand-read.** The `dump` mode listed every line with `=`, a
digit and an arithmetic operator or a second `=` across the six manuscripts:
37 lines. Read in full:

* chapter3 (21 lines) and final-L (4): formula definitions and their `where`
  clauses; two labelled `(%)` formulas and one `(a + b, mg/L)` label failed to
  parse (`UnbalancedBracket`, `UnexpectedToken(",")`), all three definitions
  with no values, so nothing checkable was lost. One Kruskal–Wallis caption.
* Health Economics (5): table notes with statistics; the chain line; and
  **line 294: *"The bootstrapped indirect effect was a × b = 0.413 (…)"*.**
  This is a numeric claim. `a` is stated in prose as a 0.118-unit increase
  (line 292); `b` is in Table 2 and not in the text. The engine parses the
  sentence as `NotAnEquation` (prose with an equals sign) and no binding is
  in scope. **Unknown**: not checkable from the text the engine sees, and not
  an engine miss as designed.
* IJAS, Lake, R PAPER: no candidate lines at all.
* No `Distance = 1.5 km = 1500 m`-shaped line (case D2's silent drop) exists
  in this corpus.

## 4. Counts, not rates

| status / check | on the six manuscripts | on the 30 constructed inputs |
|---|---|---|
| `DETECTED` true positive | **0** (never fired) | 2: B1 `2 + 2 = 5`; B3 `(a+b)² = a² + b²` |
| `DETECTED` false positive | **0** (never fired) | **3**: E2 (`where a = 3` unread, `a = 4` bound); G1 (`2ab` as a third variable); G2b (`1/2x` as `(1/2)·x`) |
| `DETECTED` conservative miss (a reader would say mismatch, engine asked) | 0 | 1: B2 `2.5 = 2.6` |
| `REQUIRES_AUTHOR_CONFIRMATION` correct | **1**: Health Economics 21.9% | 3: B2, H1, H2 |
| `CONFIRMED` correct (true negative of arithmetic) | **1**: `0.219 = 21.9%` | 7: A1, A3 ×3, C1, C2, E4, G3 |
| `SUPPORTED` correct | 0 | 1: A2 |
| `UNVERIFIED` correct refusal | **18** | E1, E2b, E3, G2, plus every definition link |
| `UNVERIFIED` that lost a real binding | 0 observed | 2: E3, G2 (`where`-prefixed value) |
| silently dropped (no node, no refusal) | 0 observed | 1: D2 |
| dimension `INCONSISTENT` correct | 0 (never fired) | 2: D3b, D4 |
| dimension `CONSISTENT` correct | **1** | 2: D1c, and D1b which is correct on dimension and blind on scale |
| `CONTRADICTED` | 0, no producer | 0, no producer |
| false negative | **0 found**; **1 unknown** (`a × b = 0.413`, `b` not in text) | 0 |

**Too few fired to rate.** `DETECTED` fired zero times on real manuscripts.
D157 reported four fabricated Tier-0 findings on a different corpus before
`defined_quantity` was added; that corpus (`Corrected_Chapters_3_4_…`, `Disha
Correction`) was not available to this run and is where a rate would have to
come from. The three constructed false positives are not a rate either: they
are inputs I wrote to reach the parser's documented conventions, and each did.
What they establish is existence, not frequency: **a `DETECTED` finding can be
false while every arithmetic step in it is exact.**

## 5. Authority

Options: safe to force a verdict; safe to set a floor under stated
conditions; advisory only; author-confirmation only; insufficient evidence to
decide.

* **`DETECTED`, numeric: advisory only, today.** The corpus gives no evidence
  either way (zero firings). The constructed cases give three ways to be
  wrong with the arithmetic right, all Major and labelled certain, none of
  which the finding's own text warns about. "Safe to set a floor" needs
  conditions that do not hold yet, so the honest reading is not "insufficient
  evidence" in the abstract but "the evidence we have says no". The
  conditions under which the classification would change to **safe to set a
  floor**:
  1. neither side contains a juxtaposed identifier that could be a product
     (G1), or such identifiers are declared by the document;
  2. no implicit multiplication follows a division without brackets (G2b), or
     that shape is routed to `REQUIRES_AUTHOR_CONFIRMATION`;
  3. every binding used is shown in the finding, and `where`-prefixed
     declarations are read (E2, E3);
  4. the finding is anchored (the U+202F defect fixed), so the reader can
     refute it where they see it;
  5. a corpus that actually produces `DETECTED` firings has been hand-read
     with a count in each cell above, and the false-positive cell is zero or
     explained.
  A floor, never a ceiling: nothing here supports forcing `pass`.
* **`DETECTED`, symbolic (identity witness): advisory only.** G1 is a correct
  identity refuted at a witness because of the identifier rule. Until
  symbolic `DETECTED` is restricted to sides with explicit operators, it
  cannot carry more than advice.
* **`CONTRADICTED`: not applicable.** Nothing produces it. Granting or
  refusing authority to it would be a statement about a design that does not
  exist.
* **`REQUIRES_AUTHOR_CONFIRMATION`: author-confirmation only**, by
  construction and correctly so. On the one real firing the author's answer
  is "rounding", and the engine could not have known.
* **Dimension `Inconsistent`: advisory only.** Magnitude-blind (D1b), label
  units admitted only with a marker, prose units read from a clause list, and
  the finding's own detail concedes "a constant's units were never stated".
  Zero corpus firings.
* **`stats_verdict::Mismatch`: not on the path.** It is the one recomputation
  against external data with a stated tolerance; if any status were to earn
  a floor first, it is this one, and it is not in the pipeline.

**No status has enough evidence for hard verdict authority today.** That is
consistent with D236's matrix, which conditioned the top row on exactly this
measurement.

## 6. Comparison with the four validation rules

D236 §1 classified the four rules; this compares them with equation
recomputation on the six axes asked for.

| axis | equation recomputation (`DETECTED`) | `MissingEffectSize` | `MissingConfidenceInterval` | `PValueOverclaim` | `TestGroupMismatch` |
|---|---|---|---|---|---|
| evidence strength | exact arithmetic over the manuscript's own numbers, both readings shown, a trail a reader can redo; **conditional on the parse** | absence of one typed extraction in one paragraph | absence of one typed extraction at one location key | a listed word somewhere in a paragraph containing a p-value | a t-test mention and a group count in one paragraph |
| reproducibility | deterministic; same line, same status, always | deterministic | deterministic | deterministic | deterministic |
| false-positive risk, measured | corpus: 0 of 0 firings; constructed: 3 false of 5, all from parser conventions | D235: 42 flags on final-L alone; D216 F1 shape (diagnostic p) fires; five independent routes (D236 §1) | as left, plus section misfiling (D215) | D216: 4 of 5 real firings wrong; case 1 of D236 flips a well-reported paper | never fired on real text; constructed only |
| dependence on lexical terms | none in the arithmetic; the **identifier rule** (juxtaposition) and label-unit markers are lexical conventions of the parser | the extractor's typed forms for effect sizes | the extractor's typed forms for CIs | entirely: a seven-word list | a regex over group nouns |
| dependence on semantic interpretation | none for numeric `DETECTED`; the decimal rule replaces interpretation with two readings; **precedence and juxtaposition are interpretations the parser makes silently** | whether a p-value "needs" an effect size (diagnostic tests do not) | whether Abstract/Results p-values are "primary claims" | whether the word is predicated of the p-value (never checked) | whether the test spanned the groups (never checked) |
| independently disprovable? | **yes, from the finding alone**: both sides and the trail are quoted; a reader sees a misparse in one glance (G1's `ab = 3` witness is visible in the message) | only by finding the quantity elsewhere; the finding cannot show what it did not find | as left | only by reading the sentence, which the finding does not quote | only by reading the analysis |

**The comparison in one sentence.** Equation recomputation is the only one of
the five whose finding carries enough to refute itself, and the only one
whose errors come from the reader's side of the arithmetic (the parse) rather
than from the claim's side (what an absence or a word means). That is why
D236 justified a floor for it in principle and why this document declines it
in practice: the parser's conventions are today unlabelled inside a finding
that says "mathematically certain".

## What equation evidence can establish, and what it cannot

**Can establish**, with the trail as proof:
* that two expressions built from the manuscript's own literals, read under
  the engine's parse, evaluate to different exact rationals under both decimal
  readings (B1);
* that a bare literal is or is not the displayed rounding of a computed value
  (A3, C1, C2);
* that two readings of a chain disagree and only the author can say which
  was meant (H1, H2, the real 21.9% line);
* that two sides declared in the document's own units differ in dimension
  (D3b, D4).

**Cannot establish**:
* that the parse is the author's equation (`2ab`, `1/2x`, a `where`-prefixed
  value, a unit tail the parser truncates);
* that a bound value is the one the author used when the document gives two
  and one is not read as a declaration (E2);
* that a dimensionally consistent equation is numerically right (`mL` vs `L`);
* anything about `CONTRADICTED`;
* anything about the verdict: no equation status touches it (30 of 30 cases
  `pass`, D236 confirmed).

## What a later commit should implement

In order, each measured before the next. None of this is done here, and none
of it grants verdict authority; D236's proposed model stays NOT implemented.

1. **Anchor the real finding.** Normalise Unicode whitespace (at least U+202F,
   U+00A0, U+2009) on both sides of `locate_line`'s comparison, and add the
   Health Economics line as a test fixture with its original bytes. Predict:
   the report row gains `Results¶155`.
2. **Read `where`-prefixed value declarations** in `declaration_binding` the
   way `unit_declaration` already strips the prefix, so E2 becomes a refusal
   and E3 becomes a binding. Predict: E2 red as `DETECTED`, green as
   `UNVERIFIED` with "2 different values".
3. **Route the two convention-dependent shapes away from `DETECTED`.** A side
   containing a multi-letter identifier that is also a concatenation of
   single-letter identifiers present on the other side (G1), and implicit
   multiplication immediately after `/` without brackets (G2b), should yield
   `UNVERIFIED` with the ambiguity named, or `REQUIRES_AUTHOR_CONFIRMATION`
   with both parses shown. This is the §6b.3 rule ("stated as such, never
   guessed") applied to the parser.
4. **Record, do not drop, a truncated parse that discards a unit tail** (D2):
   a refusal naming the dropped text, so a conversion claim is at least
   visible as unchecked.
5. **Decide `CONTRADICTED`.** Either remove it from `is_finding()` and
   `severity_for` with a comment saying no producer exists, or write the
   producer with its definition and a test. A Major severity mapped to a
   status nothing emits is a latent finding with no pinned meaning.
6. **Run this probe over a corpus with firings** (the D157 manuscripts and the
   twenty-manuscript set) and fill §4's table with real `DETECTED` rows before
   re-opening D236's floor. Only then can "safe to set a floor under stated
   conditions" be claimed, and only if the false-positive cell is zero or
   every entry is explained.

## Uncertainty, stated

* Six manuscripts and zero `DETECTED` firings: nothing here is a rate, and
  the corpus says nothing about how often a real `DETECTED` is wrong.
* The three constructed false positives were written to reach known parser
  rules; they show the rules exist and reach a Major finding, not how often
  real text takes those shapes. `2ab`-style products are common in written
  algebra; `1/2x` is common in typed methods sections; neither frequency was
  measured.
* The anchor defect's cause is inferred from three measurements (the report's
  `None`, the paragraph found with ASCII spaces, U+202F in the raw line) and
  a reading of `locate_line`; the normalisation step in extraction was not
  located in source.
* The `dump` scan looked only at lines with an operator or a second `=` and
  at most 260 characters; a numeric claim written without an operator, or in
  a table, was not scanned. `a × b = 0.413` is the one candidate it found and
  it is recorded as unknown, not as a miss.
* Hand-reading was done by Claude alone; no statistician, author or second
  reader has checked any row.
* D1 and D3 as first written hit the label-unit marker rule and were
  re-shaped; the original rows are kept in the table as a fixture lesson,
  not as an engine result.
