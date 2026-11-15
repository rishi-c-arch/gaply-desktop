// The confirmatory factor analysis's saved results pass the shape check exactly as Rust writes
// them, and nothing else (Research OS, Phase 9C-3, decisions 785 and 786), on
// storedFactor.vitest.ts's pattern. Each refused case changes one field of a committed snapshot,
// so it breaks one rule of decision 786; each rule's red is a mutation of storedShape.ts that
// removes it. The snapshot files stay as they are.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { checkStored, CFA_BANDS } from './storedShape';

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');

type O = Record<string, any>;

function snapshot(file: string): O {
  return JSON.parse(fs.readFileSync(path.join(STORED, file), 'utf8')) as O;
}

/** A snapshot with its bundle changed by `edit`, and its spec by `onSpec`. */
function changed(file: string, edit: (bundle: O) => void, onSpec?: (spec: O) => void): O {
  const view = snapshot(file);
  const bundle = JSON.parse(view.bundleJson) as O;
  edit(bundle);
  const spec = JSON.parse(view.specJson) as O;
  if (onSpec) onSpec(spec);
  return { ...view, bundleJson: JSON.stringify(bundle), specJson: JSON.stringify(spec) };
}

/** The failed snapshot with its failure changed by `edit`, and its spec by `onSpec`. */
function failed(edit: (failure: O) => void, onSpec?: (spec: O) => void): O {
  const view = snapshot(FAILED);
  const record = JSON.parse(view.failureJson) as O;
  edit(record.failure);
  const spec = JSON.parse(view.specJson) as O;
  if (onSpec) onSpec(spec);
  return { ...view, failureJson: JSON.stringify(record), specJson: JSON.stringify(spec) };
}

const kindOf = (v: O) => checkStored(v).kind;
/** Two correlated factors of three columns (C1, C4, C7 and C2, C5, C8), a coded row left out. */
const TWO = 'cfa.json';
/** Three uncorrelated factors of three columns: no covariances. */
const THREE = 'cfa_uncorrelated.json';
/** One factor of three columns at 0 degrees of freedom, an error variance below 0. */
const HEYWOOD = 'cfa_heywood.json';
/** sample_covariance_singular at the sum column, C10. */
const FAILED = 'cfa_failed.json';
// Research OS, Phase 9C-4 (decision 794, the review of 96ce0cbe's M1): results Rust writes that
// reach the accepting side of rules no earlier fixture reached.
/** One factor of six Walsh columns, S diagonal: C_b = 0 and C = 0 at d = 9. */
const INDEPENDENCE_ZERO = 'cfa_independence_zero.json';
/** Two correlated factors whose S is the model's exactly: C = 0 at d = 13. */
const EXACT = 'cfa_exact_fit.json';
/** Two correlated factors that correlate above 1: Φ̂ not positive definite. */
const PHI = 'cfa_phi_not_pd.json';
/** model_not_identified from real data (decision 790's third answer). */
const FAILED_NOT_IDENTIFIED = 'cfa_failed_not_identified.json';
const OK = 'cfa';
const BAD = 'badShape';
const code = (c: string) => `stats.cfa.${c}`;
const nc = (c: string) => ({ not_computable: code(c) });
const r = (b: O) => b.primaryOutputs;
const m = (b: O) => r(b).fit.model;
/** The next double away from 0: one unit in the last place, so an exact rule is broken by
 *  rounding. */
const ulp = (x: number) => {
  const a = new Float64Array([x]);
  const u = new BigInt64Array(a.buffer);
  u[0] += BigInt(1);
  return a[0];
};
const bump = (s: O) => {
  s.value = ulp(s.value);
};

// Each refusal below breaks one rule only: what Rust computes from the field changed is computed
// again by Rust's formulas (Model::sigma, :850-864; :1331-1378), so a rule checked later does not
// refuse it first, and the rule's removal goes red at its own case.
const stat = (x: number): O => (Number.isFinite(x) ? { value: x } : Number.isNaN(x) ? nc('too_small') : nc('overflow'));
const reconcile = (b: O) => {
  const o = r(b);
  const phi = (f: number) => o.factorVariances.find((v: O) => v.factor === f).estimate.value as number;
  o.loadings.forEach((l: O, i: number) => {
    const lambda = l.estimate.value as number;
    const p = phi(l.factor);
    const theta = o.errorVariances[i].estimate.value as number;
    const sii = lambda * lambda * p + theta;
    l.standardized = p > 0 && sii > 0 ? stat((lambda * Math.sqrt(p)) / Math.sqrt(sii)) : nc('negative_variance');
    o.smc[i].value = sii > 0 ? stat(1 - theta / sii) : nc('negative_variance');
  });
  for (const c of o.factorCovariances) {
    const [f, g] = c.factors as [number, number];
    c.correlation = phi(f) > 0 && phi(g) > 0 ? stat(c.estimate.value / Math.sqrt(phi(f) * phi(g))) : nc('negative_variance');
  }
};
/** The spec and every per-column array of the bundle put in `order` (indexes into items), the
 *  factors' numbers mapped by `factors`: what Rust would write for that spec but for the rule
 *  the case breaks. */
const permuted = (file: string, order: number[], indicators: string[][], factors: Record<number, number> = {}) =>
  changed(
    file,
    (b) => {
      const o = r(b);
      for (const k of ['items', 'loadings', 'errorVariances', 'smc']) o[k] = order.map((i) => o[k][i]);
      o.indicators = indicators;
      for (const l of o.loadings) l.factor = factors[l.factor] ?? l.factor;
      for (const v of o.factorVariances) v.factor = factors[v.factor] ?? v.factor;
      o.factorVariances.sort((x: O, y: O) => x.factor - y.factor);
    },
    (s) => (s.parameters.indicators = indicators)
  );

describe('what Rust writes', () => {
  it.each([TWO, THREE, HEYWOOD, INDEPENDENCE_ZERO, EXACT, PHI])('%s is accepted as a confirmatory factor analysis', (file) => {
    expect(kindOf(snapshot(file))).toBe(OK);
  });

  it.each([FAILED, FAILED_NOT_IDENTIFIED])('the failed run %s is accepted as failed', (file) => {
    expect(kindOf(snapshot(file))).toBe('failed');
  });

  // Decision 794: each edge fixture holds the case its rule needs, so a rule made one-way is
  // red at it.
  it('the edge fixtures are the cases the rules need: C_b 0, C 0 at d above 0, Φ̂ not positive definite', () => {
    const zero = r(JSON.parse(snapshot(INDEPENDENCE_ZERO).bundleJson));
    const exact = r(JSON.parse(snapshot(EXACT).bundleJson));
    const phi = r(JSON.parse(snapshot(PHI).bundleJson));
    expect([zero.fit.model.df, exact.fit.model.df, phi.fit.model.df]).toEqual([9, 13, 1]);
    expect(zero.fit.independence.cmin).toEqual({ value: 0 });
    expect([zero.fit.model.nfi, zero.fit.model.rfi, zero.fit.model.pnfi]).toEqual([nc('independence_cmin_zero'), nc('independence_cmin_zero'), nc('independence_cmin_zero')]);
    for (const o of [zero, exact, phi]) {
      expect(o.fit.model.cmin).toEqual({ value: 0 });
      expect([o.fit.model.hoelter05, o.fit.model.hoelter01]).toEqual([nc('cmin_zero'), nc('cmin_zero')]);
    }
    expect(exact.fit.independence.cmin.value).toBeGreaterThan(0);
    expect([zero.notes, exact.notes, phi.notes]).toEqual([[], [], [code('factor_covariance_not_positive_definite')]]);
    const failure = JSON.parse(snapshot(FAILED_NOT_IDENTIFIED).failureJson).failure;
    expect([failure.code, failure.params]).toEqual([code('model_not_identified'), {}]);
  });

  it("the fixtures are the shapes the rules need: correlated, uncorrelated, df 0 with its notes", () => {
    const two = r(JSON.parse(snapshot(TWO).bundleJson));
    const three = r(JSON.parse(snapshot(THREE).bundleJson));
    const hey = r(JSON.parse(snapshot(HEYWOOD).bundleJson));
    expect([two.factorCovariances.length, three.factorCovariances.length, hey.factorCovariances.length]).toEqual([1, 0, 0]);
    expect([two.fit.model.df, three.fit.model.df, hey.fit.model.df]).toEqual([8, 27, 0]);
    expect(hey.notes).toEqual([code('negative_variance'), code('df_zero')]);
    expect(two.exclusions).toEqual([{ code: code('missing_code'), count: 1, params: { column: 1 } }]);
  });

  it('accepts what the helpers build when no rule is broken: the estimates reconciled, an order kept', () => {
    expect(kindOf(changed(TWO, (b) => reconcile(b)))).toBe(OK);
    expect(kindOf(permuted(TWO, [0, 1, 2, 3, 4, 5], [['C1', 'C4', 'C7'], ['C2', 'C5', 'C8']]))).toBe(OK);
  });

  it('accepts 0 iterations, where the starts are already the minimum, and 500, the cap', () => {
    expect(kindOf(changed(TWO, (b) => (r(b).iterations = 0)))).toBe(OK);
    expect(kindOf(changed(TWO, (b) => (r(b).iterations = 500)))).toBe(OK);
  });

  it('holds BIC and CAIC to the band the hunt measured (the review of 9C-3, finding 3)', () => {
    expect(CFA_BANDS.lnN).toBe(2.8e-15);
  });

  // The review of 9C-3, finding 4: Rust writes RMR and GFI through `stat`, so an overflow is
  // theirs to write, and AGFI and PGFI then overflow too (1 − (p*/d)(1 − ±∞) and (d/p*)(±∞)).
  it('accepts an RMR and a GFI that overflowed, with AGFI and PGFI overflowing from GFI', () => {
    const over = nc('overflow');
    expect(kindOf(changed(TWO, (b) => ((m(b).rmr = over), (m(b).gfi = over), (m(b).agfi = over), (m(b).pgfi = over))))).toBe(OK);
    expect(kindOf(changed(TWO, (b) => ((m(b).gfi = over), (m(b).agfi = over))))).toBe(BAD);
  });
});

/** One field changed per rule of decision 786. */
const REFUSED: Array<[string, () => O]> = [
  // The spec (:545-641).
  ['no factor', () => changed(TWO, () => undefined, (s) => (s.parameters.indicators = []))],
  ['17 factors', () => changed(TWO, () => undefined, (s) => (s.parameters.indicators = Array.from({ length: 17 }, (_, f) => [`C${2 * f + 1}`, `C${2 * f + 2}`])))],
  ['a factor of one column', () => changed(TWO, () => undefined, (s) => (s.parameters.indicators = [['C1', 'C4', 'C7'], ['C2']]))],
  ['a factor\'s columns out of order', () => permuted(TWO, [0, 2, 1, 3, 4, 5], [['C1', 'C7', 'C4'], ['C2', 'C5', 'C8']])],
  ['the factors out of order', () => permuted(TWO, [3, 4, 5, 0, 1, 2], [['C2', 'C5', 'C8'], ['C1', 'C4', 'C7']], { 1: 2, 2: 1 })],
  [
    'a column twice',
    () =>
      changed(
        TWO,
        (b) => {
          const o = r(b);
          o.items[4] = 'C4';
          o.indicators[1][1] = 'C4';
          o.loadings[4].item = 'C4';
          o.errorVariances[4].item = 'C4';
          o.smc[4].item = 'C4';
        },
        (s) => (s.parameters.indicators = [['C1', 'C4', 'C7'], ['C2', 'C4', 'C8']])
      ),
  ],
  ['an uncorrelated factor of two columns', () => changed(THREE, () => undefined, (s) => (s.parameters.indicators = [['C1', 'C4', 'C7'], ['C2', 'C5'], ['C3', 'C6', 'C9']]))],
  ['correlatedFactors not a boolean', () => changed(TWO, () => undefined, (s) => (s.parameters.correlatedFactors = 'yes'))],
  ['the codes not ascending', () => changed(TWO, () => undefined, (s) => (s.parameters.missingCodes = [-99, -99]))],
  ['a key the spec does not have', () => changed(TWO, () => undefined, (s) => (s.parameters.rotation = 'none'))],
  // The shape.
  ['a key the result does not have', () => changed(TWO, (b) => (r(b).extra = 1))],
  ['items not the spec\'s, flattened', () => changed(TWO, (b) => (r(b).items = ['C1', 'C4', 'C7', 'C2', 'C8', 'C5']))],
  ['indicators not the spec\'s', () => changed(TWO, (b) => (r(b).indicators = [['C1', 'C4', 'C7'], ['C2', 'C8', 'C5']]))],
  ['rows used below the columns plus 1', () => changed(TWO, (b) => (r(b).rowsUsed = 6))],
  ['iterations above 500', () => changed(TWO, (b) => (r(b).iterations = 501))],
  ['a loading with another factor\'s number', () => changed(TWO, (b) => (r(b).loadings[1].factor = 2))],
  ['a marker\'s estimate not 1', () => changed(TWO, (b) => ((r(b).loadings[0].estimate = { value: ulp(1) }), reconcile(b)))],
  ['a marker\'s S.E. not fixed_parameter', () => changed(TWO, (b) => (r(b).loadings[3].se = { value: 1 }))],
  ['a free loading\'s S.E. fixed_parameter', () => changed(TWO, (b) => (r(b).loadings[1].se = nc('fixed_parameter')))],
  ['a C.R. not the estimate over its S.E.', () => changed(TWO, (b) => bump(r(b).loadings[1].cr))],
  ['a P above 1', () => changed(TWO, (b) => (r(b).loadings[1].p = { value: 1.5 }))],
  ['a factor variance with another number', () => changed(TWO, (b) => (r(b).factorVariances[1].factor = 1))],
  ['covariances for uncorrelated factors', () => changed(THREE, (b) => (r(b).factorCovariances = [{ ...JSON.parse(snapshot(TWO).bundleJson).primaryOutputs.factorCovariances[0] }]))],
  ['no covariance for correlated factors', () => changed(TWO, (b) => (r(b).factorCovariances = []))],
  ['an error variance out of items\' order', () => changed(TWO, (b) => (([r(b).errorVariances[1], r(b).errorVariances[2]] = [r(b).errorVariances[2], r(b).errorVariances[1]]), reconcile(b)))],
  ['a standardized weight not Rust\'s formula', () => changed(TWO, (b) => bump(r(b).loadings[1].standardized))],
  ['a correlation not Rust\'s formula', () => changed(TWO, (b) => bump(r(b).factorCovariances[0].correlation))],
  ['an SMC not Rust\'s formula', () => changed(TWO, (b) => bump(r(b).smc[2].value))],
  ['negative_variance where the roots are of positive variances', () => changed(TWO, (b) => (r(b).smc[2].value = nc('negative_variance')))],
  // The three rows (:1431-1630).
  ['the default model\'s NPAR not the spec\'s', () => changed(TWO, (b) => (m(b).npar = 14))],
  ['the default model\'s DF not the spec\'s', () => changed(TWO, (b) => (m(b).df = 7))],
  ['CMIN/DF', () => changed(TWO, (b) => bump(m(b).cminDf))],
  ['AGFI', () => changed(TWO, (b) => bump(m(b).agfi))],
  ['PGFI', () => changed(TWO, (b) => bump(m(b).pgfi))],
  ['NFI', () => changed(TWO, (b) => bump(m(b).nfi))],
  ['RFI', () => changed(TWO, (b) => bump(m(b).rfi))],
  ['IFI', () => changed(TWO, (b) => bump(m(b).ifi))],
  ['TLI', () => changed(TWO, (b) => bump(m(b).tli))],
  ['CFI', () => changed(TWO, (b) => bump(m(b).cfi))],
  ['PRATIO', () => changed(TWO, (b) => bump(m(b).pratio))],
  ['PNFI', () => changed(TWO, (b) => bump(m(b).pnfi))],
  ['PCFI', () => changed(TWO, (b) => bump(m(b).pcfi))],
  ['NCP', () => changed(TWO, (b) => bump(m(b).ncp))],
  ['FMIN', () => changed(TWO, (b) => bump(m(b).fmin))],
  ['F0', () => changed(TWO, (b) => bump(m(b).f0))],
  ['RMSEA', () => changed(TWO, (b) => bump(m(b).rmsea))],
  ['AIC', () => changed(TWO, (b) => bump(m(b).aic))],
  ['BCC', () => changed(TWO, (b) => bump(m(b).bcc))],
  ['ECVI', () => changed(TWO, (b) => bump(m(b).ecvi))],
  ['MECVI', () => changed(TWO, (b) => bump(m(b).mecvi))],
  ['BIC outside its band', () => changed(TWO, (b) => (m(b).bic = { value: m(b).bic.value * (1 + 1e-9) }))],
  ['CAIC outside its band', () => changed(TWO, (b) => (m(b).caic = { value: m(b).caic.value * (1 + 1e-9) }))],
  ['F0\'s LO 90 not the NCP\'s over N - 1', () => changed(TWO, (b) => bump(m(b).f0Lo90))],
  ['RMSEA\'s HI 90 not from the NCP\'s', () => changed(TWO, (b) => bump(m(b).rmseaHi90))],
  ['ECVI\'s LO 90 not from the NCP\'s', () => changed(TWO, (b) => bump(m(b).ecviLo90))],
  [
    'an interval\'s LO 90 above its HI 90',
    () =>
      changed(TWO, (b) => {
        const f = m(b);
        for (const [lo, hi] of [
          ['ncpLo90', 'ncpHi90'],
          ['f0Lo90', 'f0Hi90'],
          ['rmseaLo90', 'rmseaHi90'],
          ['ecviLo90', 'ecviHi90'],
        ]) [f[lo], f[hi]] = [f[hi], f[lo]];
      }),
  ],
  ['P above 1', () => changed(TWO, (b) => (m(b).p = { value: 1.5 }))],
  ['PCLOSE below 0', () => changed(TWO, (b) => (m(b).pclose = { value: -0.1 }))],
  ['HOELTER not a whole number', () => changed(TWO, (b) => (m(b).hoelter05 = { value: m(b).hoelter05.value + 0.5 }))],
  ['HOELTER below 1', () => changed(TWO, (b) => (m(b).hoelter01 = { value: 0 }))],
  ['cmin_zero at a CMIN above 0', () => changed(TWO, (b) => (m(b).hoelter05 = nc('cmin_zero')))],
  ['too_few_rows_for_bcc with rows enough', () => changed(TWO, (b) => (m(b).bcc = nc('too_few_rows_for_bcc')))],
  ['independence_cmin_zero at an independence CMIN above 0', () => changed(TWO, (b) => (m(b).nfi = nc('independence_cmin_zero')))],
  ['a df_zero cell at degrees of freedom above 0', () => changed(TWO, (b) => (m(b).rmsea = nc('df_zero')))],
  ['a value where df_zero belongs', () => changed(HEYWOOD, (b) => (m(b).cminDf = { value: 0 }))],
  ['the saturated model\'s RMR not 0', () => changed(TWO, (b) => (r(b).fit.saturated.rmr = { value: 1e-300 }))],
  ['the saturated model\'s NFI not 1', () => changed(TWO, (b) => (r(b).fit.saturated.nfi = { value: 0.5 }))],
  ['the saturated model\'s DF not 0', () => changed(TWO, (b) => (r(b).fit.saturated.df = 1))],
  ['the independence model\'s CFI not 0', () => changed(TWO, (b) => (r(b).fit.independence.cfi = { value: 1 }))],
  ['the independence model\'s PRATIO not 1', () => changed(TWO, (b) => (r(b).fit.independence.pratio = { value: 0.5 }))],
  ['the independence model\'s NPAR not p', () => changed(TWO, (b) => (r(b).fit.independence.npar = 7))],
  ['the independence model\'s FMIN not its CMIN over N - 1', () => changed(TWO, (b) => bump(r(b).fit.independence.fmin))],
  // The notes (:1382-1392).
  ['the notes out of order', () => changed(HEYWOOD, (b) => (r(b).notes = [code('df_zero'), code('negative_variance')]))],
  ['negative_variance noted with every variance above 0', () => changed(TWO, (b) => (r(b).notes = [code('negative_variance')]))],
  ['negative_variance not noted with a variance below 0', () => changed(HEYWOOD, (b) => (r(b).notes = [code('df_zero')]))],
  ['the factors\' covariance note where Φ is positive definite', () => changed(TWO, (b) => (r(b).notes = [code('factor_covariance_not_positive_definite')]))],
  ['df_zero not noted at 0 degrees of freedom', () => changed(HEYWOOD, (b) => (r(b).notes = [code('negative_variance')]))],
  // Decision 794: each note rule's other direction.
  ['the factors\' covariance note missing where Φ is not positive definite', () => changed(PHI, (b) => (r(b).notes = []))],
  ['df_zero noted at degrees of freedom above 0', () => changed(TWO, (b) => (r(b).notes = [code('df_zero')]))],
  ['the limitations out of order', () => changed(TWO, (b) => (r(b).limitations = [...r(b).limitations].reverse()))],
  // Exclusions, diagnostics and the manifest.
  ['an exclusion under a column outside the model', () => changed(TWO, (b) => ((r(b).exclusions[0].params.column = 3), (b.manifest.exclusions[0].params.column = 3)))],
  ['missing_code with no code ticked', () => changed(TWO, () => undefined, (s) => (s.parameters.missingCodes = []))],
  ['rows read not rows used plus the exclusions', () => changed(TWO, (b) => (b.manifest.rowsRead += 1))],
  ['the manifest\'s exclusions not the result\'s', () => changed(TWO, (b) => (b.manifest.exclusions = []))],
  ['a warning on a column outside the model', () => changed(TWO, (b) => (b.diagnostics = [{ code: code('suspect_missing_code'), params: { column: 3, value: 99, count: 1 }, severity: 'warning' }]))],
  ['the bundle\'s limitations not the module\'s ten', () => changed(TWO, (b) => (b.limitations = b.limitations.slice(1)))],
];

const FAILURES: Array<[string, () => O]> = [
  ['a code no run refusal has', () => failed((f) => (f.code = code('no_factors')))],
  ['the singular column outside the spec', () => failed((f) => (f.params.column = 3))],
  ['cfa_not_converged\'s limit not 500', () => failed((f) => ((f.code = code('cfa_not_converged')), (f.params = { count: 500, limit: 499 })))],
  ['cfa_not_converged\'s count above its limit', () => failed((f) => ((f.code = code('cfa_not_converged')), (f.params = { count: 501, limit: 500 })))],
  ['model_not_identified with a column', () => failed((f) => ((f.code = code('model_not_identified')), (f.params = { column: 1 })))],
];

describe('one field changed per rule of decision 786', () => {
  it.each(REFUSED)('refuses %s', (_name, make) => {
    expect(kindOf(make())).toBe(BAD);
  });

  it.each(FAILURES)('refuses a failure with %s', (_name, make) => {
    expect(kindOf(make())).toBe(BAD);
  });

  // The review of 9C-3, finding 6: a failed run's spec is held to the spec's rules too.
  it('refuses a failure whose spec has a key CfaParams does not, or codes out of order', () => {
    expect(kindOf(failed(() => undefined, (s) => (s.parameters.rotation = 'none')))).toBe(BAD);
    expect(kindOf(failed(() => undefined, (s) => (s.parameters.missingCodes = [5, -99])))).toBe(BAD);
  });

  it('accepts each failed refusal Rust can write', () => {
    expect(kindOf(failed((f) => ((f.code = code('cfa_not_converged')), (f.params = { count: 500, limit: 500 }))))).toBe('failed');
    expect(kindOf(failed((f) => ((f.code = code('model_not_identified')), (f.params = {}))))).toBe('failed');
    expect(kindOf(failed((f) => ((f.code = code('model_not_identified')), (f.params = { count: 2 }))))).toBe('failed');
    expect(kindOf(failed((f) => ((f.code = code('overflow')), (f.params = { column: 4 }))))).toBe('failed');
  });
});

// The review of 9C-3, finding 5: a value is exactly {"value": x}, as serde writes Stat::Value.
describe('a recomputed value with another key beside it', () => {
  it.each([
    ['CMIN/DF', (b: O) => (m(b).cminDf.not_computable = code('too_small'))],
    ['a standardized weight', (b: O) => (r(b).loadings[1].standardized.extra = 1)],
    ['an SMC', (b: O) => (r(b).smc[0].value.extra = 1)],
    ['F0 LO 90', (b: O) => (m(b).f0Lo90.extra = 1)],
  ] as Array<[string, (b: O) => void]>)('refuses %s', (_name, edit) => {
    expect(kindOf(changed(TWO, edit))).toBe(BAD);
  });
});
