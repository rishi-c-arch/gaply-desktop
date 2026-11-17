// The structural equation model's stored checker accepts what stats_sem writes and nothing else
// (Research OS, Phase 9C-5d, decisions 818 and 834; 9C-5e, decision 840; 9C-5f, decision 846). It
// is not registered until 9C-5f's Part 2 (decision 849), so it is called directly: semBundle and
// semFailure. Five records Rust wrote (sem_checker_records.rs, fixtures/semChecker/) hold C2 and
// C3 at the end of the file: every value from Ω or B is held exactly, a loop's in the units Rust
// wrote (unitExponents), and no tolerance is read. Two bundles are built here from the two
// wire results (fixtures/semResultView.json and semResultViewOther.json), laid out as a stored run
// is (stored/cfa.json's keys), each premise asserted against the result. Each refused case changes
// one field, so it breaks one rule of decision 818; each rule's red is a mutation of storedShape.ts
// that removes it (9c5d/mut). The spec's rules are checked through failed records, which hold the
// spec to the same rules as a bundle (semModel) and depend on nothing else, so each is isolated.
// Three rules cannot be isolated, and their mutations stay green by overlap: B05 (rows used: the
// manifest's rows used and every fit measure in N refuse it too), S17 (an empty model: the
// columns' 2-to-128 rule refuses it first, at 0) and F08 (a code not the SEM's: the failure's
// switch refuses it too). Two rules have no case here: a group's members bit for bit (no wire
// spec holds a group; the hunt's probe tests it, and 9C-5f's Part 2 commits a case on
// sem_equal.json) and an effect's three standardized values all negative_variance or none (the
// other rules imply it wherever ω is computed exactly).
import fs from 'node:fs';
import nodePath from 'node:path';
import { describe, expect, it } from 'vitest';

import ONE from '../fixtures/semResultView.json';
import OTHER from '../fixtures/semResultViewOther.json';
import { SEM_CAPABILITY, semBundle, semFailure } from './storedShape';

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
  ['B16 a standardized weight moved by 1e-6 of itself', one, (b) => scale(r(b).weights[1].standardized, 1 + 1e-6)],
  ['B17 an SMC moved by 1e-6 of itself', one, (b) => scale(r(b).smc[0].value, 1 + 1e-6)],
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
  ['B24 a standardized total moved by 1e-6 of itself', other, (b) => scale(r(b).effects[0].totalStandardized, 1 + 1e-6)],
  ['B25 a standardized indirect moved by 1e-6 of itself', other, (b) => scale(r(b).effects.find((e: O) => e.indirect.value !== 0).indirectStandardized, 1 + 1e-6)],
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
  // B21 again with its standardized indirect recomputed from the changed indirect as Rust writes
  // it, so that rule cannot refuse first: since C3 holds that value exactly from the stored
  // indirect, B21 alone is refused by both rules (decision 846).
  ['B43 an indirect effect is exactly its total less its direct, its standardized value moved with it', other, (b) => {
    const e = r(b).effects.find((x: O) => x.indirect.value !== 0);
    const h = rustImplied(r(b), r(b).unitExponents) as NonNullable<ReturnType<typeof rustImplied>>;
    const [to, from] = [h.idx(e.to), h.idx(e.from)];
    e.indirect.value = ulp(e.indirect.value);
    e.indirectStandardized = stdOf(e.indirect.value, h.omega[from][from], h.omega[to][to]);
  }],
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

describe('near neighbours: a value moved by one ulp is refused, and a missing code exclusion with its code is accepted', () => {
  it('with a loop (the second wire result), a standardized weight, an SMC, a total, a standardized total or a standardized indirect moved by one ulp is refused (C3: exact)', () => {
    const effect = (b: O) => r(b).effects.find((e: O) => e.indirect.value !== 0);
    const edits: Array<(b: O) => void> = [
      (b) => (r(b).weights[7].standardized.value = ulp(r(b).weights[7].standardized.value)),
      (b) => {
        const s = r(b).smc.find((x: O) => 'value' in x.value);
        s.value.value = ulp(s.value.value);
      },
      // The total with its indirect and both standardized values recomputed from it as Rust writes
      // them, so only the total's own rule can refuse it (the review of 9C-5f, finding 3).
      (b) => {
        const e = effect(b);
        const h = rustImplied(r(b), r(b).unitExponents) as NonNullable<ReturnType<typeof rustImplied>>;
        const [to, from] = [h.idx(e.to), h.idx(e.from)];
        e.total.value = ulp(e.total.value);
        e.indirect.value = e.total.value - e.direct.value;
        e.totalStandardized = stdOf(e.total.value, h.omega[from][from], h.omega[to][to]);
        e.indirectStandardized = stdOf(e.indirect.value, h.omega[from][from], h.omega[to][to]);
      },
      (b) => (effect(b).totalStandardized.value = ulp(effect(b).totalStandardized.value)),
      (b) => (effect(b).indirectStandardized.value = ulp(effect(b).indirectStandardized.value)),
    ];
    expect(r(B2()).notes).toContain(code('nonrecursive'));
    expect(accepts(B2(), SPEC_OTHER)).toBe(true);
    for (const edit of edits) {
      const b = B2();
      edit(b);
      expect(JSON.stringify(b)).not.toBe(JSON.stringify(B2()));
      expect(accepts(b, SPEC_OTHER)).toBe(false);
    }
  });
  it('without a loop, a standardized weight, an SMC, a total or a standardized total moved by one ulp is refused (C2: exact)', () => {
    const edits: Array<(b: O) => void> = [
      (b) => (r(b).weights[1].standardized.value = ulp(r(b).weights[1].standardized.value)),
      (b) => (r(b).smc[0].value.value = ulp(r(b).smc[0].value.value)),
      (b) => {
        const e = r(b).effects[0];
        e.total.value = ulp(e.total.value);
        e.indirect.value = e.total.value - e.direct.value;
      },
      (b) => (r(b).effects[0].totalStandardized.value = ulp(r(b).effects[0].totalStandardized.value)),
    ];
    expect(r(B1()).notes).toEqual([]);
    for (const edit of edits) {
      const b = B1();
      edit(b);
      expect(JSON.stringify(b)).not.toBe(JSON.stringify(B1()));
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

/** A record sem_checker_records.rs wrote (decision 840): a real run, saved and read back as a
 *  stored result, its spec and bundle parsed. Read with fs, not imported, so the build does not
 *  type each record's JSON. */
const RECORDS = nodePath.join(__dirname, 'fixtures/semChecker');
const record = (name: string): { spec: O; bundle: O } => {
  const v = JSON.parse(fs.readFileSync(nodePath.join(RECORDS, name), 'utf8'));
  return { spec: JSON.parse(v.specJson), bundle: JSON.parse(v.bundleJson) };
};
/** Each forged value of decision 838's finding 2, as a change to a copy of the bundle: a
 *  standardized weight of 17 (the first free weight, and, where it is a path, its effect's direct
 *  standardized value with it, which must equal it bit for bit, so the forgery is consistent), an
 *  SMC of 0.123 (the first) and a standardized total of −5 (the first effect's). */
const FORGED: Array<[string, (b: O) => void]> = [
  ['a standardized weight of 17', (b) => {
    const w = r(b).weights.find((x: O) => 'value' in x.cr);
    w.standardized = { value: 17 };
    const e = r(b).effects.find((x: O) => JSON.stringify([x.to, x.from]) === JSON.stringify([w.to, w.from]));
    if (e) e.directStandardized = { value: 17 };
  }],
  ['an SMC of 0.123', (b) => (r(b).smc[0].value = { value: 0.123 })],
  ['a standardized total of −5', (b) => (r(b).effects[0].totalStandardized = { value: -5 })],
];

describe('the records Rust wrote for the checker (decisions 840 and 846)', () => {
  it('loopPivot.json: a loop of decision 838\'s shape, 2,000 rows, is accepted', () => {
    const { spec, bundle } = record('loopPivot.json');
    expect(r(bundle).notes).toEqual([code('nonrecursive')]);
    expect(r(bundle).rowsUsed).toBe(2000);
    expect(accepts(bundle, spec)).toBe(true);
  });

  /** C3's effect on C2 in cancel.json, whose two routes nearly cancel. */
  const cancelling = (b: O) => r(b).effects.find((e: O) => e.to.column === 'C2' && e.from.column === 'C3');
  /** A consistent change to that effect's total by `delta`: its indirect is total − direct, as
   *  the checker requires exactly, and both standardized values are recomputed from them as Rust
   *  writes them (standardized, on Ω in the stored units), so only the total's own rule can refuse
   *  it. (Scaling them instead left them off by an ulp, so their rule refused first: c3_total_ulp
   *  stayed green until this was fixed.) */
  const moveTotal = (b: O, delta: number) => {
    const e = cancelling(b);
    const h = rustImplied(r(b), r(b).unitExponents) as NonNullable<ReturnType<typeof rustImplied>>;
    const [to, from] = [h.idx(e.to), h.idx(e.from)];
    e.total.value += delta;
    e.indirect.value = e.total.value - e.direct.value;
    e.totalStandardized = stdOf(e.total.value, h.omega[from][from], h.omega[to][to]);
    e.indirectStandardized = stdOf(e.indirect.value, h.omega[from][from], h.omega[to][to]);
  };

  it('cancel.json: a loop whose total from C3 to C2 nearly cancels is accepted (decision 843)', () => {
    const { spec, bundle } = record('cancel.json');
    expect(r(bundle).notes).toEqual([code('nonrecursive')]);
    const e = cancelling(bundle);
    expect(Math.abs(e.total.value)).toBeLessThan(0.1 * Math.abs(e.direct.value));
    expect(accepts(bundle, spec)).toBe(true);
  });

  it('cancel.json: that total, changed consistently by one ulp (its indirect and both standardized values with it), is refused (C3: exact)', () => {
    const { spec, bundle } = record('cancel.json');
    // The control: the same arithmetic with no change leaves a record the checker accepts.
    const same = clone(bundle);
    moveTotal(same, 0);
    expect(accepts(same, spec)).toBe(true);
    const b = clone(bundle);
    const t0 = cancelling(b).total.value;
    moveTotal(b, ulp(t0) - t0);
    expect(cancelling(b).total.value).toBe(ulp(t0));
    expect(accepts(b, spec)).toBe(false);
  });

  for (const [name, loop] of [['small.json', false], ['smallLoop.json', true]] as Array<[string, boolean]>) {
    it(`${name}: data scaled by 1e-10, ${loop ? 'with' : 'without'} a loop, is accepted as stored (C2)`, () => {
      const { spec, bundle } = record(name);
      expect(r(bundle).notes).toEqual(loop ? [code('nonrecursive')] : []);
      // The premise: every variance far inside 2480982e's absolute zone of 1e-17.
      expect(r(bundle).variances.every((v: O) => v.estimate.value > 0 && v.estimate.value < 1e-17)).toBe(true);
      expect(accepts(bundle, spec)).toBe(true);
    });
    it.each(FORGED)(`${name}: %s is refused (C2)`, (_, forge) => {
      const { spec, bundle } = record(name);
      const b = clone(bundle);
      forge(b);
      expect(JSON.stringify(b)).not.toBe(JSON.stringify(bundle));
      expect(accepts(b, spec)).toBe(false);
    });
  }
});

/* ---- C3: Rust's arithmetic in Rust's units, held exactly (decision 846) -------------------- */

/** 2^e from its bits (e + 1023 in the exponent field, the mantissa 0). */
const twoTo = (e: number) => {
  const v = new DataView(new ArrayBuffer(8));
  v.setUint32(0, (e + 1023) * 0x100000);
  return v.getFloat64(0);
};
/** e for which a positive normal x is 2^e times a mantissa in [1, 2), from its bits. */
const exponentOf = (x: number) => {
  const v = new DataView(new ArrayBuffer(8));
  v.setFloat64(0, x);
  return ((v.getUint32(0) >>> 20) & 0x7ff) - 1023;
};
const NEG = { not_computable: code('negative_variance') };
/** stats_sem's `stat` (:2078-2086). */
const statOf = (x: number): O => (Number.isFinite(x) ? { value: x } : { not_computable: code(Number.isNaN(x) ? 'too_small' : 'overflow') });
/** stats_sem's `standardized` (:2096-2102). */
const stdOf = (x: number, wa: number, wb: number): O => (wa > 0 && wb > 0 ? statOf((x * Math.sqrt(wa)) / Math.sqrt(wb)) : NEG);
/** stats_linalg::multiply (:231-239): each sum left to right (from +0, where Rust's starts at −0.0:
 *  only a zero's sign differs). */
const mult = (x: number[][], y: number[][]) => x.map((row) => y[0].map((_, j) => row.reduce((s, v, t) => s + v * y[t][j], 0)));

/** stats_linalg::inverse (:313-365) as written, Gauss–Jordan with partial pivoting; null where Rust
 *  refuses, unless `rule` is false (a forgery that keeps computing where Rust would not). */
function rustInverse(m0: number[][], rule = true): number[][] | null {
  const n = m0.length;
  if (m0.some((row) => row.some((v) => !Number.isFinite(v)))) return null;
  const scale = m0.flat().reduce((s, x) => Math.max(s, Math.abs(x)), 0);
  const m = m0.map((row) => row.slice());
  const inv = m0.map((_, i) => m0.map((__, j) => (i === j ? 1 : 0)));
  for (let j = 0; j < n; j += 1) {
    let pr = j;
    let largest = Math.abs(m[j][j]);
    for (let i = j + 1; i < n; i += 1) {
      if (Math.abs(m[i][j]) > largest) {
        largest = Math.abs(m[i][j]);
        pr = i;
      }
    }
    if (rule && !(largest > 1e-12 * scale)) return null;
    [m[j], m[pr]] = [m[pr], m[j]];
    [inv[j], inv[pr]] = [inv[pr], inv[j]];
    const pivot = m[j][j];
    for (let k = 0; k < n; k += 1) {
      m[j][k] /= pivot;
      inv[j][k] /= pivot;
    }
    for (let i = 0; i < n; i += 1) {
      if (i === j) continue;
      const f = m[i][j];
      if (f === 0) continue;
      for (let k = 0; k < n; k += 1) {
        m[i][k] -= f * m[j][k];
        inv[i][k] -= f * inv[j][k];
      }
    }
  }
  return inv;
}

/** A stored result's A and Φ and its variables' indices (the factors, then the model's columns). */
const matricesOf = (res: O) => {
  const k = res.indicators.length;
  const m = k + res.columns.length;
  const idx = (v: O) => ('factor' in v ? v.factor - 1 : k + res.columns.indexOf(v.column));
  const a = Array.from({ length: m }, () => Array.from({ length: m }, () => 0));
  const phi = Array.from({ length: m }, () => Array.from({ length: m }, () => 0));
  for (const w of res.weights) a[idx(w.to)][idx(w.from)] = w.estimate.value;
  for (const c of res.covariances) phi[idx(c.a)][idx(c.b)] = phi[idx(c.b)][idx(c.a)] = c.estimate.value;
  res.variances.forEach((v: O, x: number) => (phi[x][x] = v.estimate.value));
  return { a, phi, idx, m };
};

/** Rust's implied for a model with a loop (stats_sem/mod.rs, implied's loop branch): I − D⁻¹ A D,
 *  D = 2^exps, inverted, B = D (that inverse) D⁻¹, Ω = B Φ Bᵀ. Written here, apart from the checker,
 *  so a forgery's values do not come from the code under test. */
function rustImplied(res: O, exps: number[], rule = true) {
  const { a, phi, idx, m } = matricesOf(res);
  const d = exps.map(twoTo);
  const iMinus = a.map((row, i) => row.map((y, j) => (i === j ? 1 : 0) - (y * d[j]) / d[i]));
  const x = rustInverse(iMinus, rule);
  if (x === null) return null;
  const b = x.map((row, i) => row.map((y, j) => (y * d[i]) / d[j]));
  const omega = mult(mult(b, phi), b[0].map((_, j) => b.map((row) => row[j])));
  return { a, phi, b, omega, idx, m };
}

/** A copy of a stored loop bundle with unitExponents `exps` and every value Rust computes from Ω
 *  or B (and each effect's direct, from A) recomputed in those units, as Rust writes them
 *  (outcome, :2166-2232); null where Rust's inverse refuses, unless `rule` is false. */
function inUnits(bundle: O, exps: number[], rule = true): O | null {
  const b = clone(bundle);
  const res = r(b);
  expect(res.notes).toContain(code('nonrecursive'));
  res.unitExponents = exps.slice();
  const im = rustImplied(res, exps, rule);
  if (im === null) return null;
  const { a, phi, b: bm, omega, idx } = im;
  for (const w of res.weights) w.standardized = stdOf(a[idx(w.to)][idx(w.from)], omega[idx(w.from)][idx(w.from)], omega[idx(w.to)][idx(w.to)]);
  for (const s of res.smc) {
    const v = idx(s.variable);
    s.value = omega[v][v] > 0 ? statOf(1 - phi[v][v] / omega[v][v]) : NEG;
  }
  for (const e of res.effects) {
    const [to, from] = [idx(e.to), idx(e.from)];
    const [total, direct] = [bm[to][from], a[to][from]];
    const [wf, wt] = [omega[from][from], omega[to][to]];
    Object.assign(e, {
      total: statOf(total),
      direct: statOf(direct),
      indirect: statOf(total - direct),
      totalStandardized: stdOf(total, wf, wt),
      directStandardized: stdOf(direct, wf, wt),
      indirectStandardized: stdOf(total - direct, wf, wt),
    });
  }
  return b;
}

/** Every number at which two stored results differ (=== on numbers, so −0 equals 0), by path. */
const differing = (x: unknown, y: unknown, at = ''): string[] => {
  if (typeof x === 'number' || typeof y === 'number') return x === y ? [] : [at];
  if (Array.isArray(x) && Array.isArray(y)) return x.length !== y.length ? [at] : x.flatMap((v, i) => differing(v, y[i], `${at}[${i}]`));
  if (isObj(x) && isObj(y)) {
    const keys = Array.from(new Set([...Object.keys(x), ...Object.keys(y)]));
    return keys.flatMap((k) => differing(x[k], y[k], `${at}.${k}`));
  }
  return x === y ? [] : [at];
};
const isObj = (v: unknown): v is O => typeof v === 'object' && v !== null && !Array.isArray(v);

/** A stored estimate changed, its C.R. recomputed as Rust's `tested` does (estimate / S.E.). */
const setEstimate = (row: O, x: number) => {
  row.estimate = { value: x };
  row.cr = statOf(x / row.se.value);
};

/** Finding 1's forgery (decision 845): C1's stored variance divided by 1e20, its C.R. recomputed;
 *  every standardized weight 17 and each effect's direct standardized value its path's; every SMC
 *  0.123; every standardized total −5 and standardized indirect effect 4. C1 is in each record's
 *  loop and in no covariance, so no other rule sees the variance. */
const finding1 = (b: O) => {
  const res = r(b);
  const c1 = res.variances.find((v: O) => v.variable.column === 'C1');
  setEstimate(c1, c1.estimate.value / 1e20);
  for (const w of res.weights) w.standardized = { value: 17 };
  for (const e of res.effects) {
    e.directStandardized = e.direct.value === 0 ? e.directStandardized : { value: 17 };
    e.totalStandardized = { value: -5 };
    e.indirectStandardized = { value: 4 };
  }
  for (const s of res.smc) s.value = { value: 0.123 };
};

describe('C3: for a model with a loop the checker repeats Rust\'s arithmetic in the units Rust wrote (decision 846)', () => {
  it('loopPivot.json: in its stored units every value from Ω or B is the one stored; in the stored variances\' units one is not; accepted', () => {
    const { spec, bundle } = record('loopPivot.json');
    const res = r(bundle);
    expect(differing(inUnits(bundle, res.unitExponents), bundle)).toEqual([]);
    const variancesUnits = res.variances.map((v: O) => exponentOf(Math.sqrt(Math.abs(v.estimate.value))));
    expect(variancesUnits).not.toEqual(res.unitExponents);
    expect(differing(inUnits(bundle, variancesUnits), bundle).filter((p) => !p.includes('unitExponents')).length).toBeGreaterThan(0);
    expect(accepts(bundle, spec)).toBe(true);
  });

  it.each(['loopPivot.json', 'smallLoop.json', 'cancel.json'])('%s: finding 1\'s forgery (a variance divided by 1e20, the standardized values 17, −5 and 4, the SMCs 0.123) is refused', (name) => {
    const { spec, bundle } = record(name);
    expect(accepts(bundle, spec)).toBe(true);
    const b = clone(bundle);
    finding1(b);
    expect(accepts(b, spec)).toBe(false);
  });

  /** An effect of cancel.json by its ends. */
  const effect = (b: O, to: string, from: string) => r(b).effects.find((e: O) => e.to.column === to && e.from.column === from);

  it('cancel.json: C3\'s standardized indirect effect on C1 (4.739e-7) moved by 1e-6 of itself is refused (decision 844)', () => {
    const { spec, bundle } = record('cancel.json');
    const e = effect(bundle, 'C1', 'C3');
    expect(e.indirectStandardized.value.toPrecision(4)).toBe('4.739e-7');
    const b = clone(bundle);
    effect(b, 'C1', 'C3').indirectStandardized.value *= 1 + 1e-6;
    expect(accepts(b, spec)).toBe(false);
  });

  it('cancel.json: C3\'s total on C2 moved by one ulp, its indirect recomputed as the total less the direct, is refused', () => {
    const { spec, bundle } = record('cancel.json');
    const b = clone(bundle);
    const e = effect(b, 'C2', 'C3');
    e.total.value = ulp(e.total.value);
    e.indirect.value = e.total.value - e.direct.value;
    // Both standardized values recomputed from it as Rust writes them, so only the total's rule can
    // refuse it (the review of 9C-5f, finding 3).
    const h = rustImplied(r(b), r(b).unitExponents) as NonNullable<ReturnType<typeof rustImplied>>;
    e.totalStandardized = stdOf(e.total.value, h.omega[h.idx(e.from)][h.idx(e.from)], h.omega[h.idx(e.to)][h.idx(e.to)]);
    e.indirectStandardized = stdOf(e.indirect.value, h.omega[h.idx(e.from)][h.idx(e.from)], h.omega[h.idx(e.to)][h.idx(e.to)]);
    expect(e.total.value).not.toBe(effect(bundle, 'C2', 'C3').total.value);
    expect(accepts(b, spec)).toBe(false);
  });

  it('sink.json: C3\'s exponent set to −537 and C7\'s to 511, every value recomputed in those units, is refused', () => {
    const { spec, bundle } = record('sink.json');
    const res = r(bundle);
    expect(res.columns).toEqual(['C1', 'C2', 'C3', 'C4', 'C5', 'C6', 'C7']);
    expect(accepts(bundle, spec)).toBe(true);
    const units = res.unitExponents.slice();
    [units[2], units[6]] = [-537, 511];
    const b = inUnits(bundle, units) as O;
    expect(b).not.toBeNull();
    // The premise: in those units some total or standardized value moves by more than 1e-9 of its
    // scale: (|B| |I − A| |B|)_ij for a total, that times √ω_jj / √ω_ii for a standardized total or
    // indirect, the value itself for a standardized weight or an SMC.
    const h = rustImplied(res, res.unitExponents) as NonNullable<ReturnType<typeof rustImplied>>;
    const ab = h.b.map((row) => row.map(Math.abs));
    const parts = mult(mult(ab, h.a.map((row, i) => row.map((y, j) => Math.abs((i === j ? 1 : 0) - y)))), ab);
    const moved: number[] = [];
    r(b).effects.forEach((e: O, n: number) => {
      const [i, j] = [h.idx(e.to), h.idx(e.from)];
      const s = parts[i][j];
      const ss = (s * Math.sqrt(h.omega[j][j])) / Math.sqrt(h.omega[i][i]);
      const was = res.effects[n];
      moved.push(Math.abs(e.total.value - was.total.value) / s, Math.abs(e.totalStandardized.value - was.totalStandardized.value) / ss, Math.abs(e.indirectStandardized.value - was.indirectStandardized.value) / ss);
    });
    r(b).weights.forEach((w: O, n: number) => moved.push(Math.abs(w.standardized.value - res.weights[n].standardized.value) / Math.abs(res.weights[n].standardized.value)));
    r(b).smc.forEach((x: O, n: number) => moved.push(Math.abs(x.value.value - res.smc[n].value.value) / Math.abs(res.smc[n].value.value)));
    expect(Math.max(...moved)).toBeGreaterThan(1e-9);
    expect(accepts(b, spec)).toBe(false);
  });

  it('the second wire result: a factor\'s exponent changed from its first column\'s, every value recomputed, is refused', () => {
    const b0 = B2();
    const units = r(b0).unitExponents.slice();
    expect(accepts(b0, SPEC_OTHER)).toBe(true);
    units[0] += 1;
    const b = inUnits(b0, units) as O;
    expect(b).not.toBeNull();
    expect(accepts(b, SPEC_OTHER)).toBe(false);
  });

  it.each([[512], [-538]])('loopPivot.json: an exponent of %d (C6\'s, outside [−537, 511]), every value recomputed, is refused', (e) => {
    const { spec, bundle } = record('loopPivot.json');
    const units = r(bundle).unitExponents.slice();
    units[5] = e;
    const b = inUnits(bundle, units) as O;
    // C6 is in no path, so in those units no value moves: only the range can refuse it.
    expect(differing(b, bundle).filter((p) => !p.includes('unitExponents'))).toEqual([]);
    expect(accepts(b, spec)).toBe(false);
  });

  it('loopPivot.json: an exponent that is not an integer is refused', () => {
    const { spec, bundle } = record('loopPivot.json');
    const b = clone(bundle);
    r(b).unitExponents[5] += 0.5;
    expect(accepts(b, spec)).toBe(false);
  });

  it('loopPivot.json: units under which a nonzero path rounds when scaled (C3 → C1 subnormal), every value recomputed, are refused', () => {
    const { spec, bundle } = record('loopPivot.json');
    const units = r(bundle).unitExponents.slice();
    // C1 and C2 at 2^511, so the loop's entries keep their size, and C3 at 2^−537.
    [units[0], units[1], units[2]] = [511, 511, -537];
    const res = r(bundle);
    const a13 = res.weights[1].estimate.value;
    expect([res.weights[1].to, res.weights[1].from]).toEqual([col(1), col(3)]);
    // The premise: a13 · 2^(−537) / 2^511 is subnormal and does not scale back to a13.
    const scaled = (a13 * twoTo(-537)) / twoTo(511);
    expect(Math.abs(scaled)).toBeLessThan(2 ** -1022);
    expect((scaled * twoTo(511)) / twoTo(-537)).not.toBe(a13);
    const b = inUnits(bundle, units) as O;
    expect(b).not.toBeNull();
    expect(accepts(b, spec)).toBe(false);
  });

  it('loopPivot.json: a path that rounds when scaled although its scaled entry is normal (C3 → C1 of 1e-150, C3 at 2^−537, C1, C2 and C4 at 2^−100), every value recomputed, is refused by the round trips of A and X, not by the normal-number rule', () => {
    const { spec, bundle } = record('loopPivot.json');
    const b0 = clone(bundle);
    const res = r(b0);
    expect([res.weights[1].to, res.weights[1].from]).toEqual([col(1), col(3)]);
    setEstimate(res.weights[1], 1e-150);
    const units = res.unitExponents.slice();
    // C4 with C1 and C2, so C4 → C2 keeps its size and Rust's pivot rule is met.
    [units[0], units[1], units[2], units[3]] = [-100, -100, -537, -100];
    // The premise: 1e-150 · 2^−537 is subnormal, so it rounds; divided by 2^−100 it is normal, so
    // the normal-number rule cannot see it; and it does not scale back to 1e-150 (the review of
    // 9C-5f, finding 4). B_13 is computed as (X_13 · d_1) / d_3, and X_13 · d_1 is about B_13 · d_3,
    // subnormal too, so the round trip of X refuses it as well: removing either round trip alone
    // leaves this test red, removing both turns it green (mut/review, c3_overlap_a_x).
    const step = 1e-150 * twoTo(-537);
    const scaled = step / twoTo(-100);
    expect(step !== 0 && Math.abs(step) < 2 ** -1022).toBe(true);
    expect(Math.abs(scaled) >= 2 ** -1022).toBe(true);
    expect((scaled * twoTo(-100)) / twoTo(-537)).not.toBe(1e-150);
    const b = inUnits(b0, units) as O;
    expect(b).not.toBeNull();
    expect(accepts(b, spec)).toBe(false);
  });

  it('loopPivot.json: a value whose B entry rounds when scaled back (C4\'s total on C1 subnormal, its inverse entry normal), every value recomputed, is refused', () => {
    const { spec, bundle } = record('loopPivot.json');
    const b0 = clone(bundle);
    const res = r(b0);
    // C2 → C1 and C4 → C2 made 1e-160 each, so C4's total on C1 is near 1e-320; C4's unit 2^511
    // and C1's 2^−537 put its inverse entry near 1e-5. C3's unit is C1's, so C3 → C1 keeps its size
    // and Rust's pivot rule is met.
    setEstimate(res.weights[0], 1e-160);
    setEstimate(res.weights[3], 1e-160);
    const units = res.unitExponents.slice();
    [units[0], units[1], units[2], units[3]] = [-537, 0, -537, 511];
    const b = inUnits(b0, units) as O;
    expect(b).not.toBeNull();
    const t = effect(b, 'C1', 'C4').total.value;
    expect(t !== 0 && Math.abs(t) < 2 ** -1022).toBe(true);
    expect(accepts(b, spec)).toBe(false);
  });

  it('loopPivot.json: I − A changed so that Rust\'s pivot rule refuses it (C1 ⇄ C2\'s gains multiplying to 1 − 1e-14), every value recomputed, is refused', () => {
    const { spec, bundle } = record('loopPivot.json');
    const b0 = clone(bundle);
    const res = r(b0);
    expect([res.weights[2].to, res.weights[2].from]).toEqual([col(2), col(1)]);
    setEstimate(res.weights[2], (1 - 1e-14) / res.weights[0].estimate.value);
    expect(rustImplied(res, res.unitExponents)).toBeNull();
    const b = inUnits(b0, res.unitExponents, false) as O;
    expect(accepts(b, spec)).toBe(false);
  });

  const BAD_UNITS: Array<[string, (u: number[]) => unknown]> = [
    ['null', () => null],
    ['a string', () => 'units'],
    ['one entry too few', (u) => u.slice(1)],
    ['one entry too many', (u) => [...u, 0]],
  ];
  for (const name of ['small.json', 'loopPivot.json']) {
    it.each(BAD_UNITS)(`${name}: unitExponents set to %s is refused`, (_what, bad) => {
      const { spec, bundle } = record(name);
      expect(accepts(bundle, spec)).toBe(true);
      const b = clone(bundle);
      r(b).unitExponents = bad(r(b).unitExponents);
      expect(accepts(b, spec)).toBe(false);
    });
  }
});

