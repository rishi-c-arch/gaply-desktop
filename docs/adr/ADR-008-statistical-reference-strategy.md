# ADR-008: How a statistical method is checked against a reference

**Status: Accepted, 18 Oct 2026 (decision 91),** after the Phase 4
evaluation's F2 correction below.
Written 18 Oct 2026, Research OS Phase 4B-1 (decisions 60 to 67), from what
Phases 2A and 4B-1 built: the descriptive summary's R golden set (D271) and the
independent-samples t-test's (`docs/methods/independent_t.md`).

## Context

Blueprint §11.2 asks for golden tests against independent, well-established
implementations, property-based and metamorphic tests, and "tolerance tests
justified by the operation's mathematics, precision, and conditioning — not
one global epsilon"; §18.2 makes "independent reference comparison completed"
an acceptance item for every statistical method. §11.1 adds that each method
defines its own comparison semantics.

Gaply implements its own capabilities (Prompt 0). A reference tool may be
used only as a test oracle, run outside Gaply, and its source is never copied
in. Before this ADR the practice existed in two places but was not written
down as a rule: Level 1's `l1_r_golden` (R values for seven methods) and Phase
2A's `desc_r_golden`. The t-test as an analysis is the first new method under
the Research OS, so the rule is fixed here before more methods follow it.

## Decision

1. **R is the primary oracle**, run outside Gaply, at
   `/usr/local/bin/Rscript`. Nothing of R or of any R package is vendored,
   linked, bundled or translated into Gaply. A CRAN package is used only when
   a step approves it by name, for fixtures only (effectsize: decision 67).
   Another reference (SciPy, statcheck, PSPP, Octave) may be added for a
   method R does not cover, under the same rule.
2. **Each method has a generator script beside its fixtures**
   (`tests/fixtures/<method>/make_<method>_fixtures.R`), seeded, with
   `/usr/local/bin/Rscript` in its header and in the header of every file it
   writes. The golden file's header names R's version and architecture and the
   version of every package whose answer it records, and the script names each
   R call it makes (for the t-test: `t.test(…, var.equal = TRUE/FALSE)`,
   `effectsize::cohens_d(…, pooled_sd = TRUE/FALSE)`, `qt`). The Rust test
   asserts the provenance line is present.
3. **R computes on what it read.** Every synthetic data file is written, read
   back with `read.csv`, and the reference values are computed on the values
   read. A `cells_bits.csv` records the IEEE 754 bits of every number R read,
   and a test requires Gaply to parse each cell to the same bits, so a golden
   value is R's answer on exactly Gaply's input. (R's own decimal parser
   misread one e-notation spelling by one ulp in Phase 2A: this is measured,
   not assumed.)
4. **The fixture set covers the method's edges**, not only typical data:
   the smallest n the method computes, unequal sizes and spreads, ties, a
   large offset, heavy tails, negative values, the ingest's row cap, constant
   inputs, missing values and codes, and the level-order and sign conventions.
   **A case where Gaply refuses and R computes, or R stops, is an expected
   refusal** with Gaply's closed code, and the test requires that refusal.
5. **Property and metamorphic tests** (proptest, decision 3) accompany every
   golden set: symmetries (a level swap), invariances (a row permutation, a
   shift, a scale), ranges (p in [0, 1], an interval containing its estimate)
   and known bounds (Welch's df). Inputs are built on a binary grid so the
   transformed input is exact and a miss is Gaply's, never the input's. Each
   relation states its tolerance: exact where IEEE arithmetic makes it exact,
   otherwise the sum of the two sides' golden gates plus one ulp.
6. **A tolerance per method and per statistic, justified on the method's
   page** (`docs/methods/<id>.md`), from the statistic's conditioning: counts
   and integer df exact; a relative ε for well-conditioned quantities; a
   cancellation term (for the t-test, ε · |m₁| + |m₂| for a difference of
   means) where the statistic inherits its inputs' rounding; and for a p
   value, a relative gate, an absolute floor so a tiny p cannot pass or fail on
   its relative error alone, and the statistic's permitted error carried
   through the distribution's density. **A gate is measured and stated, and
   never loosened to make a case pass.** Where a computation cannot meet its
   gate over part of its domain, that part is refused with a closed code,
   not reported (the t-test's quantile above df 1e4).
7. **The test prints its measurement**: the largest |gaply − R| / gate per
   statistic and the case it came from, so the method's page quotes a run.

## Alternatives considered

- **One global epsilon for every method.** Rejected by blueprint §11.2: it is
  either too loose for well-conditioned statistics or fails the
  ill-conditioned ones on correct code. On the 1e9-offset fixture
  (`data_offset.csv`), d from a one-pass mean, `sum(x)/n`, differs from
  `effectsize::cohens_d` by 1.194e-8 (Student) and 1.200e-8 (Welch), while R's
  own `t.test` and `effectsize::cohens_d` agree to 2.2e-16 and exactly: the
  difference is the means' rounding at that offset, not a disagreement inside
  R. Measured 18 Oct 2026 with R 4.6.1 and effectsize 1.0.3, comparing
  `(sum(y1)/n1 - sum(y2)/n2) / s` (s the standardizer each variant uses) with
  `effectsize::cohens_d(y ~ f, pooled_sd = TRUE/FALSE, ci = NULL)`. (Before
  the Phase 4 evaluation this sentence attributed the 1.2e-8 to R's `t.test`
  against effectsize, which does not reproduce.)
- **Vendoring a reference implementation's source**, or translating it, to
  compare in-process. Rejected: Prompt 0 forbids copying reference tools into
  Gaply, and an oracle inside the system under test is not independent.
- **SciPy as the primary oracle.** Not chosen: R already supplies Level 1's
  golden values, and one primary oracle keeps fixtures comparable across
  methods. SciPy remains available as a second reference where R lacks one.
- **Hand-computed expected values only.** Rejected as the primary check: they
  inherit the author's premise (CLAUDE.md, "a hand-written fixture inherits the
  author's premise"). They remain useful as exact-arithmetic oracles in
  property tests (Phase 2A's integer oracle).

## Consequences

- Each new method costs a generator script, a golden test, a property test and
  a method page before it can be Beta.
- A fixture regenerated with another R or package version is a visible diff in
  the golden header and data; the version is part of the evidence.
- A computation that cannot meet its gate somewhere is refused there, which can
  narrow a method's domain (the quantile above df 1e4) and is documented on its
  page.
- R and the approved CRAN packages must be installable on the developer's
  machine to regenerate fixtures; CI only reads the committed fixtures.

## Evidence

- Phase 2A: `gaply-core/tests/desc_r_golden.rs`, `desc_properties.rs`,
  `desc_metamorphic.rs` (D271).
- Phase 4B-1: `gaply-core/tests/ttest_r_golden.rs` (18 cases, 32 computed and 4
  refused (case, variant) pairs, every statistic at least 100× inside its gate),
  `ttest_quantile.rs` (33 df; statrs's `inverse_cdf` measured and rejected,
  3.5e-6 to 6.8e-2 off from df 1e5), `ttest_properties.rs`, `ttest_validate.rs`,
  `ttest_export.rs`; `docs/methods/independent_t.md` §6.
- Level 1: `gaply-core/tests/l1_r_golden.rs` (D270), the precedent for R values.

## Revision trigger

A method for which R has no trustworthy implementation; a reference package
whose licence or maintenance changes; a second platform's CI computing other
bytes than the pinned ones; or a method whose gate cannot be justified from its
conditioning.
