// The structural equation model's stored checker accepts what stats_sem writes and nothing else
// (Research OS, Phase 9C-5d, decisions 818 and 834). It is not registered until 9C-5e (decision
// 836), so it is called directly: semBundle and semFailure. Two bundles are built here from the two
// wire results (fixtures/semResultView.json and semResultViewOther.json), laid out as a stored run
// is (stored/cfa.json's keys), each premise asserted against the result. Each refused case changes
// one field, so it breaks one rule of decision 818; each rule's red is a mutation of storedShape.ts
// that removes it (9c5d/mut). The spec's rules are checked through failed records, which hold the
// spec to the same rules as a bundle (semModel) and depend on nothing else, so each is isolated.
// Three rules cannot be isolated, and their mutations stay green by overlap: B05 (rows used: the
// manifest's rows used and every fit measure in N refuse it too), S17 (an empty model: the
// columns' 2-to-128 rule refuses it first, at 0) and F08 (a code not the SEM's: the failure's
// switch refuses it too). Two rules have no case here: a group's members bit for bit (no wire
// spec holds a group; the hunt's probe tests it, and 9C-5e commits cases on sem_equal.json) and
// an effect's three standardized values all negative_variance or none (outside the band's zone
// around 0 the other rules imply it).
import { describe, expect, it } from 'vitest';

import ONE from '../fixtures/semResultView.json';
import OTHER from '../fixtures/semResultViewOther.json';
import { SEM_BANDS, SEM_CAPABILITY, semBundle, semFailure } from './storedShape';

type O = Record<string, any>;
const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x)) as T;
const code = (c: string) => `stats.sem.${c}`;

/** The next double away from 0 (one unit in the last place). */
const ulp = (x: number) => {
  const a = new Float64Array([x]);
  const u = new BigInt64Array(a.buffer);
  u[0] += BigInt(1);
  return a[0];
};

/** LIMITATIONS (stats_sem/mod.rs:584-600), as notes, in order. */
const LIMITATIONS = [
  'ml_only', 'no_means', 'one_group_only', 'indicators_measure_one_factor', 'equal_one_kind', 'marker_identification_only', 'no_fixed_values',
  'effects_structural_only', 'no_indirect_effect_tests', 'no_stability_index', 'no_modification_indices', 'normal_theory_standard_errors', 'no_srmr',
  'listwise_only', 'no_weights',
].map((c) => ({ code: code(c), params: {}, severity: 'note' }));
/** The wire views' suspect_missing_code warning, alone: the ml_only note is the run's, not the
 *  stored bundle's (decision 834). */
const SUSPECT = { code: code('suspect_missing_code'), params: { column: 1, count: 3, value: 99 }, severity: 'warning' };

/** A spec as stats_sem_wire.rs writes it, as a literal. */
const specOf = (parameters: O, rows: number, columns: number): O => ({
  schemaVersion: 1,
  operationId: 18,
  workspaceId: 'statistics',
  capabilityId: SEM_CAPABILITY,
  capabilityVersion: 1,
  inputs: [{ contentHash: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef', rows, columns, semanticRole: 'table' }],
  parameters,
  randomSeed: null,
  confirmationRefs: [],
});
/** outcome()'s spec (stats_sem_wire.rs:142-146). */
const SPEC_ONE = specOf(
  { indicators: [['C1', 'C3', 'C5'], ['C2', 'C4', 'C6']], paths: [{ to: { factor: 2 }, from: { factor: 1 } }], covariances: [], equal: [], missingCodes: [] },
  41,
  6
);
/** other_outcome()'s spec (stats_sem_wire.rs:182-196). */
const SPEC_OTHER = specOf(
  {
    indicators: [['C5', 'C6', 'C7'], ['C8', 'C9', 'C10']],
    paths: [
      { to: { column: 'C1' }, from: { column: 'C2' } },
      { to: { column: 'C1' }, from: { column: 'C3' } },
      { to: { column: 'C2' }, from: { column: 'C1' } },
      { to: { column: 'C2' }, from: { column: 'C4' } },
    ],
    covariances: [
      { a: { factor: 1 }, b: { factor: 2 } },
      { a: { factor: 1 }, b: { column: 'C3' } },
      { a: { column: 'C3' }, b: { column: 'C4' } },
    ],
    equal: [],
    missingCodes: [-9],
  },
  62,
  10
);

/** A stored bundle of a wire result: primaryOutputs the result, its manifest `manifest`. */
const bundleOf = (view: O, manifest: O): O => ({
  schemaVersion: 1,
  runId: view.runId,
  status: 'succeeded',
  primaryOutputs: clone(view.result),
  diagnostics: [clone(SUSPECT)],
  limitations: clone(LIMITATIONS),
  manifest,
});
/** The first's manifest as manifest() gives it (stats_sem_wire.rs:198-216). */
const B1 = () => bundleOf(ONE, clone(ONE.manifest));
/** The second's, built from its own result (stats_sem_wire.rs:257 reuses the first's, which does not
 *  fit it): rowsUsed 60, its two exclusions, rowsRead 62. */
const B2 = () => bundleOf(OTHER, { ...clone(ONE.manifest), rowsUsed: 60, rowsRead: 62, exclusions: clone(OTHER.result.exclusions) });

const accepts = (b: O, s: O) => semBundle(b, s) !== null;
const r = (b: O) => b.primaryOutputs;

describe('the premises: the two wire results are the models their specs name', () => {
  it('the first: six columns on two factors, the second on the first, a row left out', () => {
    const o = r(B1());
    expect(o.columns).toEqual(['C1', 'C2', 'C3', 'C4', 'C5', 'C6']);
    expect(o.indicators).toEqual(SPEC_ONE.parameters.indicators);
    expect(o.weights.map((w: O) => [w.to, w.from, w.kind])).toEqual([
      ...[1, 3, 5].map((c) => [{ column: `C${c}` }, { factor: 1 }, 'loading']),
      ...[2, 4, 6].map((c) => [{ column: `C${c}` }, { factor: 2 }, 'loading']),
      [{ factor: 2 }, { factor: 1 }, 'path'],
    ]);
    expect(o.effects.map((e: O) => [e.to, e.from])).toEqual([[{ factor: 2 }, { factor: 1 }]]);
    expect([o.rowsUsed, o.notes, o.exclusions.length, o.fit.model.df, o.fit.model.npar]).toEqual([40, [], 1, 8, 13]);
    expect(ONE.manifest.rowsRead).toBe(41);
  });

  it('the second: a loop of two columns, two factors, three covariances, a coded row and a hole', () => {
    const o = r(B2());
    expect(o.columns).toEqual(Array.from({ length: 10 }, (_, i) => `C${i + 1}`));
    expect(o.indicators).toEqual(SPEC_OTHER.parameters.indicators);
    expect(o.weights.slice(6).map((w: O) => [w.to, w.from])).toEqual(SPEC_OTHER.parameters.paths.map((p: O) => [p.to, p.from]));
    expect(o.covariances.map((c: O) => [c.a, c.b])).toEqual(SPEC_OTHER.parameters.covariances.map((c: O) => [c.a, c.b]));
    expect(o.notes).toEqual([code('negative_variance'), code('nonrecursive')]);
    expect(o.exclusions.map((e: O) => [e.code, e.count, e.params.column])).toEqual([[code('missing_code'), 1, 1], [code('missing_value'), 1, 2]]);
    expect([o.rowsUsed, o.effects.length, o.fit.model.df, o.fit.model.npar]).toEqual([60, 6, 32, 23]);
    // Some effect has an indirect part, so the indirect rules are reached.
    expect(o.effects.some((e: O) => e.indirect.value !== 0)).toBe(true);
  });
});

describe('what Rust writes', () => {
  it('the first wire result, stored, is accepted', () => expect(accepts(B1(), SPEC_ONE)).toBe(true));
  it('the second wire result, stored, is accepted', () => expect(accepts(B2(), SPEC_OTHER)).toBe(true));
});

/** One bundle case: its rule, the bundle it changes, and the change. */
type Case = [string, () => O, (b: O) => void, O?];
const one = B1;
const other = B2;
const scale = (s: O, by: number) => {
  s.value = s.value * by;
};
const BUNDLE_CASES: Case[] = [
  ['B01 the result has exactly its keys', one, (b) => (r(b).extra = 1)],
  ['B02 a succeeded status', one, (b) => (b.status = 'failed')],
  ['B03 the columns are the model columns', one, (b) => r(b).columns.reverse()],
  ['B04 the indicators are the spec', one, (b) => (r(b).indicators[1] = ['C2', 'C4'])],
  ['B05 rows used at least the columns plus one', one, (b) => (r(b).rowsUsed = 6)],
  ['B06 at most 500 iterations', one, (b) => (r(b).iterations = 501)],
  ['B07 attempt 1, 2 or 3', one, (b) => (r(b).attempt = 4)],
  ['B08 a weight is of its kind', one, (b) => (r(b).weights[6].kind = 'loading')],
  ['B09 a weight is to and from its variables', one, (b) => (r(b).weights[1].to = { column: 'C2' })],
  ['B10 a factor\'s first loading is exactly 1', one, (b) => (r(b).weights[0].estimate = { value: ulp(1) })],
  ['B11 a fixed loading\'s S.E. is fixed_parameter', one, (b) => (r(b).weights[0].se = { value: 1 })],
  ['B12 a C.R. is its estimate over its S.E.', one, (b) => (r(b).weights[1].cr.value = ulp(r(b).weights[1].cr.value))],
  ['B13 a covariance is between the spec\'s ends', other, (b) => (r(b).covariances[0].b = { factor: 1 })],
  ['B14 a correlation is exact', other, (b) => (r(b).covariances[2].correlation.value = ulp(r(b).covariances[2].correlation.value))],
  ['B15 a variance is an error term exactly when endogenous', one, (b) => (r(b).variances[0].error = true)],
  ['B16 a standardized weight within its band', one, (b) => scale(r(b).weights[1].standardized, 1 + 1e-6)],
  ['B17 an SMC within its band', one, (b) => scale(r(b).smc[0].value, 1 + 1e-6)],
  ['B18 the SMCs one per endogenous variable', one, (b) => (r(b).smc[0].variable = { factor: 1 })],
  // A label only, its numbers kept: only the effects' to and from rule can see it.
  ['B19 the effects in the model\'s order', other, (b) => (r(b).effects[0].to = { column: 'C3' })],
  ['B20 a direct effect is its path\'s estimate', one, (b) => (r(b).effects[0].direct.value = ulp(r(b).effects[0].direct.value))],
  ['B21 an indirect effect is exactly its total less its direct', other, (b) => {
    const e = r(b).effects.find((x: O) => x.indirect.value !== 0);
    e.indirect.value = ulp(e.indirect.value);
  }],
  ['B22 a direct standardized effect is its path\'s standardized weight', one, (b) => (r(b).effects[0].directStandardized.value = ulp(r(b).effects[0].directStandardized.value))],
  ['B23 a direct standardized effect without a path is exactly 0', other, (b) => (r(b).effects.find((e: O) => e.direct.value === 0).directStandardized = { value: 5e-324 })],
  ['B24 a standardized total within its band', other, (b) => scale(r(b).effects[0].totalStandardized, 1 + 1e-6)],
  ['B25 a standardized indirect within its band', other, (b) => scale(r(b).effects.find((e: O) => e.indirect.value !== 0).indirectStandardized, 1 + 1e-6)],
  ['B26 the default row\'s npar is q from the spec', other, (b) => (r(b).fit.model.npar = 24)],
  ['B27 a measure of fit by its rule', one, (b) => (r(b).fit.model.cminDf.value = ulp(r(b).fit.model.cminDf.value))],
  ['B28 negative_variance exactly when a variance is below 0', other, (b) => (r(b).notes = [code('nonrecursive')])],
  ['B29 covariance_not_positive_definite exactly when the block fails its factor', other, (b) => (r(b).notes = [code('negative_variance'), code('covariance_not_positive_definite'), code('nonrecursive')])],
  ['B30 nonrecursive exactly when the paths hold a loop', other, (b) => (r(b).notes = [code('negative_variance')])],
  ['B31 df_zero exactly at 0 degrees of freedom', one, (b) => (r(b).notes = [code('df_zero')])],
  ['B32 the notes in Rust\'s order', other, (b) => r(b).notes.reverse()],
  ['B33 only the four notes', one, (b) => (r(b).notes = [code('ml_only')])],
  ['B34 the result\'s limitations, the fifteen in order', one, (b) => r(b).limitations.pop()],
  ['B35 missing_code only with codes', one, (b) => {
    r(b).exclusions[0].code = code('missing_code');
    b.manifest.exclusions[0].code = code('missing_code');
  }],
  ['B36 a diagnostic names a column of the model', one, (b) => (b.diagnostics = [{ ...SUSPECT, params: { ...SUSPECT.params, column: 7 } }])],
  ['B37 the bundle\'s limitations, the fifteen as notes', one, (b) => b.limitations.pop()],
  ['B38 the manifest\'s rows read', one, (b) => (b.manifest.rowsRead = 42)],
  ['B39 the manifest\'s exclusions are the result\'s', one, (b) => (b.manifest.exclusions = [])],
  ['B40 the manifest\'s capability', one, (b) => (b.manifest.capabilityId = 'stats.analyze.cfa')],
  ['B41 an exclusion in the SEM\'s codes', one, (b) => {
    r(b).exclusions[0].code = 'stats.cfa.missing_value';
    b.manifest.exclusions[0].code = 'stats.cfa.missing_value';
  }],
  ['B42 the manifest\'s rows used', one, (b) => (b.manifest.rowsUsed = 39)],
];

describe('each rule of decision 818 refuses its own change', () => {
  it.each(BUNDLE_CASES)('refuses %s', (_name, base, edit) => {
    const b = base();
    const spec = base === one ? SPEC_ONE : SPEC_OTHER;
    expect(accepts(b, spec)).toBe(true);
    edit(b);
    expect(accepts(b, spec)).toBe(false);
  });
});

/* ---- failed runs, and the spec's rules through them ------------------------ */

const NOT_CONVERGED = { code: code('sem_not_converged'), params: { count: 7, limit: 500 }, severity: 'error' };
const withParams = (p: O): O => ({ ...SPEC_ONE, parameters: { ...clone(SPEC_ONE.parameters), ...p } });
const col = (c: number) => ({ column: `C${c}` });
const fac = (f: number) => ({ factor: f });
const cov = (a: O, b: O) => ({ a, b });
const path = (to: O, from: O) => ({ to, from });
const variance = (v: O) => ({ variance: { variable: v } });

describe('a failed run', () => {
  it.each([
    ['sample_covariance_singular', { column: 3 }],
    ['overflow', { column: 1 }],
    ['sem_not_converged', { count: 7, limit: 500 }],
    ['solution_not_identified', {}],
  ])('%s, as the run writes it, is accepted', (c, params) => {
    expect(semFailure({ code: code(c), params, severity: 'error' }, SPEC_ONE)).toBe(true);
  });

  it.each([
    ['F01 singular names a column of the model', { code: code('sample_covariance_singular'), params: { column: 7 }, severity: 'error' }],
    ['F02 overflow names a column of the model', { code: code('overflow'), params: { column: 0 }, severity: 'error' }],
    ['F03 not converged at most its limit', { code: code('sem_not_converged'), params: { count: 501, limit: 500 }, severity: 'error' }],
    ['F04 not converged\'s limit is 500', { code: code('sem_not_converged'), params: { count: 7, limit: 499 }, severity: 'error' }],
    ['F05 solution_not_identified has no params', { code: code('solution_not_identified'), params: { count: 1 }, severity: 'error' }],
    ['F06 model_not_identified is never written', { code: code('model_not_identified'), params: {}, severity: 'error' }],
    ['F07 a failure is an error', { ...NOT_CONVERGED, severity: 'warning' }],
    ['F08 a failure is in the SEM\'s codes', { ...NOT_CONVERGED, code: 'stats.cfa.cfa_not_converged' }],
    ['F09 a failure\'s keys', { code: code('sample_covariance_singular'), params: { column: 3, count: 1 }, severity: 'error' }],
  ])('refuses %s', (_name, failure) => {
    expect(semFailure(failure, SPEC_ONE)).toBe(false);
  });

  const many = (n: number, f: (i: number) => O) => Array.from({ length: n }, (_, i) => f(i));
  const PAIRS_24: O[] = [];
  for (let a = 1; a <= 24; a += 1) for (let b = a + 1; b <= 24; b += 1) PAIRS_24.push(cov(col(a), col(b)));
  const SPEC_CASES: Array<[string, O]> = [
    ['S01 the spec has exactly its keys', { ...clone(SPEC_ONE.parameters), extra: 1 }],
    ['S02 the codes ascending, once each', withParams({ missingCodes: [2, 1] }).parameters],
    ['S03 at most 16 factors', withParams({ indicators: many(17, (i) => [`C${2 * i + 1}`, `C${2 * i + 2}`]), paths: [] }).parameters],
    ['S04 a factor of at least 2 columns', withParams({ indicators: [['C1', 'C3', 'C5'], ['C2']] }).parameters],
    ['S05 a factor\'s columns ascending', withParams({ indicators: [['C1', 'C5', 'C3'], ['C2', 'C4', 'C6']] }).parameters],
    ['S06 the factors by first column', withParams({ indicators: [['C2', 'C4', 'C6'], ['C1', 'C3', 'C5']] }).parameters],
    ['S07 no column twice', withParams({ indicators: [['C1', 'C3', 'C5'], ['C2', 'C3', 'C6']] }).parameters],
    ['S08 at most 64 paths', withParams({ indicators: [], paths: many(65, (i) => path(col(i + 2), col(1))) }).parameters],
    ['S09 a factor end is a factor of the model', withParams({ paths: [path(fac(2), fac(0))] }).parameters],
    ['S10 a path from a variable to itself', withParams({ paths: [path(fac(2), fac(2))] }).parameters],
    ['S11 a path at a factor\'s column', withParams({ paths: [path(col(1), fac(2))] }).parameters],
    ['S12 the paths ascending', withParams({ paths: [path(fac(2), fac(1)), path(fac(1), col(7))] }).parameters],
    // 257 of the 276 pairs of 24 columns, ascending: within 128 columns, so only the count refuses.
    ['S13 at most 256 covariances', withParams({ indicators: [], paths: [], covariances: PAIRS_24.slice(0, 257) }).parameters],
    ['S14 a covariance with itself', withParams({ covariances: [cov(fac(1), fac(1))] }).parameters],
    ['S15 a covariance a before b', withParams({ covariances: [cov(fac(2), fac(1))] }).parameters],
    ['S16 the covariances ascending', withParams({ covariances: [cov(fac(1), col(3)), cov(fac(1), fac(2))] }).parameters],
    ['S17 at least one factor, path or covariance', withParams({ indicators: [], paths: [], covariances: [] }).parameters],
    ['S18 at most 128 columns', withParams({ indicators: [], paths: [], covariances: many(65, (i) => cov(col(2 * i + 1), col(2 * i + 2))) }).parameters],
    ['S19 a group of at least 2 members', withParams({ equal: [[variance(fac(1))]] }).parameters],
    ['S20 a group of one kind', withParams({ equal: [[{ path: path(fac(2), fac(1)) }, variance(fac(1))]] }).parameters],
    ['S21 a group\'s loading is a free one', withParams({ equal: [[{ loading: { factor: 1, column: 'C1' } }, { loading: { factor: 2, column: 'C4' } }]] }).parameters],
    ['S22 a group\'s path is the spec\'s', withParams({ equal: [[{ path: path(fac(2), fac(1)) }, { path: path(col(7), fac(1)) }]] }).parameters],
    ['S23 a group ascending', withParams({ equal: [[variance(fac(2)), variance(fac(1))]] }).parameters],
    ['S24 the groups by first member', withParams({ equal: [[variance(fac(2)), variance(col(1))], [variance(fac(1)), variance(col(2))]] }).parameters],
    ['S25 no parameter in two groups', withParams({ equal: [[variance(fac(1)), variance(col(1))], [variance(fac(2)), variance(col(1))]] }).parameters],
    // 130 members, every one a free parameter of the model: 32 groups of two of its 65
    // covariances (64) and 33 of two of its 66 columns' variances (66).
    ['S26 at most 128 members', withParams({ indicators: [], paths: [], covariances: many(65, (i) => cov(col(1), col(i + 2))), equal: [...many(32, (i) => [{ covariance: cov(col(1), col(2 * i + 2)) }, { covariance: cov(col(1), col(2 * i + 3)) }]), ...many(33, (i) => [variance(col(2 * i + 1)), variance(col(2 * i + 2))])] }).parameters],
    ['S27 a group\'s variance is of a variable of the model', withParams({ equal: [[variance(fac(1)), variance(col(9))]] }).parameters],
    ['S28 a group\'s covariance is the spec\'s', withParams({ covariances: [cov(fac(1), fac(2))], equal: [[{ covariance: cov(fac(1), fac(2)) }, { covariance: cov(col(1), col(2)) }]] }).parameters],
  ];

  it.each(SPEC_CASES)('refuses %s', (_name, parameters) => {
    expect(semFailure(NOT_CONVERGED, SPEC_ONE)).toBe(true);
    expect(semFailure(NOT_CONVERGED, { ...SPEC_ONE, parameters })).toBe(false);
  });

  it('the spec cases\' near neighbours are accepted, so each case is refused by its one change', () => {
    const ok = (p: O) => semFailure(NOT_CONVERGED, withParams(p));
    expect(ok({ missingCodes: [1, 2] })).toBe(true);
    expect(ok({ indicators: many(16, (i) => [`C${2 * i + 1}`, `C${2 * i + 2}`]), paths: [] })).toBe(true);
    expect(ok({ indicators: [], paths: many(64, (i) => path(col(i + 2), col(1))) })).toBe(true);
    expect(ok({ indicators: [], paths: [], covariances: PAIRS_24.slice(0, 256) })).toBe(true);
    expect(ok({ indicators: [], paths: [], covariances: many(64, (i) => cov(col(2 * i + 1), col(2 * i + 2))) })).toBe(true);
    expect(ok({ paths: [path(fac(2), fac(1)), path(col(7), fac(1))] })).toBe(true);
    expect(ok({ covariances: [cov(fac(1), fac(2)), cov(fac(1), col(3))] })).toBe(true);
    expect(ok({ equal: [[variance(fac(1)), variance(fac(2))]] })).toBe(true);
    expect(ok({ equal: [[variance(fac(1)), variance(col(1))], [variance(fac(2)), variance(col(2))]] })).toBe(true);
    expect(ok({ equal: [[{ loading: { factor: 1, column: 'C3' } }, { loading: { factor: 2, column: 'C4' } }]] })).toBe(true);
    expect(ok({ indicators: [], paths: [], covariances: many(64, (i) => cov(col(1), col(i + 2))), equal: many(32, (i) => [variance(col(2 * i + 1)), variance(col(2 * i + 2))]) })).toBe(true);
  });

  it('the failure is refused when its spec is not one Rust takes, whatever the failure', () => {
    expect(semFailure({ code: code('sample_covariance_singular'), params: { column: 3 }, severity: 'error' }, withParams({ indicators: [], paths: [], covariances: [] }))).toBe(false);
  });
});

describe('the bands', () => {
  it('each band is pinned at the value 9C-5d\'s hunt set (decision 835)', () => {
    expect(SEM_BANDS).toEqual({ standardizedWeight: 1e-17, smc: 1e-16, total: 1e-16, standardizedEffect: 1e-17 });
  });
});

describe('the near neighbours of the refused cases are accepted', () => {
  it('without a loop, an SMC or a standardized weight moved by ten times its band is refused', () => {
    const move = (x: O, band: number) => (x.value += 10 * band * Math.max(Math.abs(x.value), 1));
    for (const edit of [(b: O) => move(r(b).smc[0].value, SEM_BANDS.smc), (b: O) => move(r(b).weights[1].standardized, SEM_BANDS.standardizedWeight)]) {
      const b = B1();
      edit(b);
      expect(accepts(b, SPEC_ONE)).toBe(false);
    }
  });
  it('a missing code exclusion is accepted with a code ticked', () => {
    const b = B1();
    r(b).exclusions[0].code = code('missing_code');
    b.manifest.exclusions[0].code = code('missing_code');
    expect(accepts(b, { ...SPEC_ONE, parameters: { ...SPEC_ONE.parameters, missingCodes: [-99] } })).toBe(true);
  });
});

