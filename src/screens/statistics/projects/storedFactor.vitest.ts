// The factor analysis's saved results pass the shape check exactly as Rust writes them, and
// nothing else (Research OS, Phase 9B-3, decisions 732 and 733), on
// storedLinearRegression.vitest.ts's pattern. Each refused case changes one field of a committed
// snapshot, so it breaks one rule of decision 733; each rule's red is a mutation of
// storedShape.ts that removes it. The snapshot files stay as they are.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { checkStored } from './storedShape';

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
/** Principal axis, promax, two factors, a row left out. */
const PROMAX = 'factor.json';
/** Principal components, varimax, Kaiser's rule. */
const VARIMAX = 'factor_varimax.json';
/** Principal components, varimax not converged at one iteration. */
const NOT_ROTATED = 'factor_not_rotated.json';
const FAILED = 'factor_failed.json';
/** Principal components, a fixed 3 factors, varimax converged in 995 cycles of iterations 1,000
 *  (Research OS, Phase 9C-1, decision 742's B1). */
const SLOW = 'factor_slow_varimax.json';
const OK = 'factor';
const BAD = 'badShape';
const code = (c: string) => `stats.factor.${c}`;
const nc = (c: string) => ({ not_computable: code(c) });
const r = (b: O) => b.primaryOutputs;
/** The next double above x: one unit in the last place, so an exact rule is broken by rounding. */
const ulp = (x: number) => {
  const a = new Float64Array([x]);
  const u = new BigInt64Array(a.buffer);
  u[0] += BigInt(x >= 0 ? 1 : -1);
  return a[0];
};
/* Each refusal below breaks one rule only: a field derived from the one changed is computed
 * again, in the checker's order, so a rule checked earlier does not refuse it first (the
 * review of this step, findings 1 and 8). */
/** Each total's percent of p and the running percent, as Rust's sums_of. */
const resum = (sums: O[], p: number) => {
  let cumulative = 0;
  for (const x of sums) {
    const percent = (x.total.value / p) * 100;
    cumulative += percent;
    x.percent = { value: percent };
    x.cumulative = { value: cumulative };
  }
};
const vals = (m: O[][]) => m.map((row) => row.map((x) => x.value as number));
const product = (a: number[][], b: number[][]) => a.map((row) => b[0].map((_, j) => row.reduce((acc, x, k) => acc + x * b[k][j], 0)));
const asStats = (m: number[][]) => m.map((row) => row.map((value) => ({ value })));
/** Promax's structure again from its pattern and Φ. */
const restructure = (b: O) => {
  r(b).rotation.structure = asStats(product(vals(r(b).rotation.loadings), vals(r(b).rotation.factorCorrelations)));
};
/** Varimax's rotated loadings again as the extracted times T, and their sums. */
const rerotate = (b: O) => {
  const rotated = product(vals(r(b).extraction.loadings), vals(r(b).rotation.transformation));
  r(b).rotation.loadings = asStats(rotated);
  resumColumns(b, rotated);
};
const resumColumns = (b: O, rotated: number[][]) => {
  const m = rotated[0].length;
  r(b).rotation.sums.forEach((x: O, j: number) => (x.total = { value: rotated.reduce((acc, row) => acc + row[j] * row[j], 0) }));
  resum(r(b).rotation.sums, rotated.length);
  return m;
};
const bump = (s: O) => {
  s.value = ulp(s.value);
};

describe("the factor analysis's saved results (decision 732)", () => {
  it('opens every committed snapshot as the factor analysis, or its failure', () => {
    expect(kindOf(snapshot(PROMAX))).toBe(OK);
    expect(kindOf(snapshot(VARIMAX))).toBe(OK);
    expect(kindOf(snapshot(NOT_ROTATED))).toBe(OK);
    // Decision 742's B1: a varimax of 995 cycles whose rotated rows differ from the extracted
    // rows times T by 10.17 ε · m, above the old band of 8.5 divided by ε · m alone.
    expect(kindOf(snapshot(SLOW))).toBe(OK);
    const f = checkStored(snapshot(FAILED));
    expect(f.kind).toBe('failed');
    expect(f.kind === 'failed' && f.capability).toBe('factor');
  });

  it('accepts the valid shapes the snapshots stand for: a code in a field Rust can write it in', () => {
    // An item's MSA of no correlation (adequacy's 0/0: an item exactly uncorrelated with every
    // other), while the others are values.
    expect(kindOf(changed(PROMAX, (b) => (r(b).kmo.items[2].msa = nc('no_correlation'))))).toBe(OK);
    // KMO and every MSA no_correlation together (decision 742's m1: adequacy, :1107-1113).
    expect(kindOf(changed(PROMAX, (b) => {
      r(b).kmo.overall = nc('no_correlation');
      r(b).kmo.items.forEach((a: O) => (a.msa = nc('no_correlation')));
    }))).toBe(OK);
    // A determinant below the smallest normal double beside a log below ln of it (decision
    // 742's m1), with Bartlett's chi-square from that log: 128 items, whose log can reach
    // 128 ln(1e-9), about -2,652 (six items' cannot; the review of 9C-1, finding 7).
    expect(kindOf(changed(SLOW, (b) => {
      r(b).determinant.value = nc('too_small');
      r(b).determinant.log = { value: -720 };
      r(b).bartlett.chiSquare = { value: -((r(b).rowsUsed - 1 - (2 * 128 + 5) / 6) * -720) };
    }))).toBe(OK);
    // A suspect code's warning naming one of the items.
    expect(kindOf(changed(PROMAX, (b) => (b.diagnostics = [{ code: code('suspect_missing_code'), params: { column: 1, value: -99, count: 2 }, severity: 'warning' }])))).toBe(OK);
  });
});

describe('what stats_factor never writes (decision 733)', () => {
  const cases: Array<[string, () => O]> = [
    // Shape.
    ['an extra key in the result', () => changed(PROMAX, (b) => (r(b).extra = 1))],
    // The result's items alone: every per-item table still names the spec's.
    ['items other than the spec\'s', () => changed(PROMAX, (b) => (r(b).items[5] = 'C7'))],
    ['descriptives out of the items\' order', () => changed(PROMAX, (b) => r(b).descriptives.reverse())],
    ['communalities out of the items\' order', () => changed(PROMAX, (b) => r(b).extraction.communalities.reverse())],
    ['MSAs out of the items\' order', () => changed(PROMAX, (b) => r(b).kmo.items.reverse())],
    // With the manifest's counts and Bartlett's chi-square, which reads n, made to agree.
    ['rows used below 3', () => changed(PROMAX, (b) => {
      const n = 2;
      r(b).rowsUsed = n;
      b.manifest.rowsUsed = n;
      b.manifest.rowsRead = n + r(b).exclusions.reduce((acc: number, x: O) => acc + x.count, 0);
      const log = Math.min(r(b).determinant.log.value, 0);
      r(b).bartlett.chiSquare = { value: log === 0 ? 0 : -((n - 1 - (2 * 6 + 5) / 6) * log) };
    })],
    ['an eigenvalue short', () => changed(PROMAX, (b) => r(b).eigenvalues.pop())],
    ['m other than the spec\'s factors', () => changed(PROMAX, () => undefined, (s) => (s.parameters.factors = 3))],
    // A third eigenvalue above 1, the first lowered by as much, so the sum, the order and the
    // percents hold and only Kaiser's count moves (to 3, against m of 2).
    ["m other than Kaiser's count", () => changed(VARIMAX, (b) => {
      const e = r(b).eigenvalues;
      e[0].total.value = e[0].total.value + e[2].total.value - 1.5;
      e[2].total.value = 1.5;
      resum(e, 6);
    })],
    ['a loadings matrix a row short', () => changed(PROMAX, (b) => r(b).extraction.loadings.pop())],
    ['a transformation a row short', () => changed(VARIMAX, (b) => r(b).rotation.transformation.pop())],
    ['a factor correlation matrix a column short', () => changed(PROMAX, (b) => r(b).rotation.factorCorrelations.forEach((row: unknown[]) => row.pop()))],
    ["a rotation method other than the spec's", () => changed(VARIMAX, () => undefined, (s) => (s.parameters.rotation = 'promax'))],
    ['no rotation where one is due', () => changed(PROMAX, (b) => (r(b).rotation = null))],
    ['a rotation beside rotation_not_converged', () => changed(NOT_ROTATED, (b) => (r(b).rotation = r(JSON.parse(snapshot(VARIMAX).bundleJson)).rotation))],
    ['a rotation where none was asked', () => changed(VARIMAX, () => undefined, (s) => (s.parameters.rotation = 'none'))],
    ['varimax with a structure', () => changed(VARIMAX, (b) => (r(b).rotation.structure = r(b).rotation.loadings))],
    ['promax with a transformation', () => changed(PROMAX, (b) => (r(b).rotation.transformation = r(b).rotation.factorCorrelations))],
    ['a limitation short', () => changed(PROMAX, (b) => r(b).limitations.pop())],
    ['an unknown note', () => changed(PROMAX, (b) => r(b).notes.push(code('too_small')))],
    // Principal axis: neither note is recomputed there, so only their order refuses it.
    ['notes out of order', () => changed(PROMAX, (b) => (r(b).notes = [code('eigenvalues_not_distinct'), code('communality_above_one')]))],
    ['one_factor_not_rotated with two factors', () => changed(PROMAX, (b) => r(b).notes.push(code('one_factor_not_rotated')))],
    ['communality_above_one under principal components', () => changed(VARIMAX, (b) => r(b).notes.unshift(code('communality_above_one')))],
    ['extraction_eigenvalue_negative under principal components', () => changed(VARIMAX, (b) => r(b).notes.unshift(code('extraction_eigenvalue_negative')))],
    ['rotation_singular under varimax', () => changed(NOT_ROTATED, (b) => (r(b).notes = [code('rotation_singular')]))],
    // Recomputed.
    ["Bartlett's df", () => changed(PROMAX, (b) => (r(b).bartlett.df += 1))],
    ["Bartlett's chi-square one ulp off its log", () => changed(PROMAX, (b) => bump(r(b).bartlett.chiSquare))],
    ['a p above 1', () => changed(PROMAX, (b) => (r(b).bartlett.p = { value: 1.5 }))],
    ["a percent one ulp off its total", () => changed(PROMAX, (b) => bump(r(b).eigenvalues[0].percent))],
    ['a cumulative percent one ulp off', () => changed(PROMAX, (b) => bump(r(b).eigenvalues[1].cumulative))],
    ["an extraction sum one ulp off its loadings'", () => changed(PROMAX, (b) => bump(r(b).extraction.sums[0].total))],
    ["varimax's rotation sum one ulp off its loadings'", () => changed(VARIMAX, (b) => { bump(r(b).rotation.sums[0].total); resum(r(b).rotation.sums, 6); })],
    ["a communality one ulp off its loadings'", () => changed(PROMAX, (b) => bump(r(b).extraction.communalities[0].extraction))],
    ["a principal components initial communality other than 1", () => changed(VARIMAX, (b) => (r(b).extraction.communalities[0].initial = { value: 0.999 }))],
    ['principal components iterating', () => changed(VARIMAX, (b) => (r(b).extraction.iterations = 1))],
    ['a principal axis initial communality above 1', () => changed(PROMAX, (b) => (r(b).extraction.communalities[0].initial = { value: 1.5 }))],
    ["principal axis iterations past the spec's", () => changed(PROMAX, (b) => (r(b).extraction.iterations = 26))],
    ["a structure one ulp off the pattern times Φ", () => changed(PROMAX, (b) => bump(r(b).rotation.structure[0][0]))],
    ['eigenvalues_not_distinct where they are distinct', () => changed(VARIMAX, (b) => r(b).notes.push(code('eigenvalues_not_distinct')))],
    // Bounded.
    ['a KMO above 1', () => changed(PROMAX, (b) => (r(b).kmo.overall = { value: 1.2 }))],
    ['an MSA below 0', () => changed(PROMAX, (b) => (r(b).kmo.items[0].msa = { value: -0.1 }))],
    ['eigenvalues out of order', () => changed(PROMAX, (b) => { const e = r(b).eigenvalues; [e[2].total, e[3].total] = [e[3].total, e[2].total]; resum(e, 6); })],
    ['eigenvalues summing off p', () => changed(PROMAX, (b) => { r(b).eigenvalues[5].total.value += 0.01; resum(r(b).eigenvalues, 6); })],
    ['a Φ diagonal off 1', () => changed(PROMAX, (b) => { r(b).rotation.factorCorrelations[0][0] = { value: 1.01 }; restructure(b); })],
    ['Φ not symmetric', () => changed(PROMAX, (b) => { bump(r(b).rotation.factorCorrelations[1][0]); restructure(b); })],
    ['T not orthogonal', () => changed(VARIMAX, (b) => { r(b).rotation.transformation[0][0].value += 0.01; rerotate(b); })],
    ['a rotated row off the extracted row times T', () => changed(VARIMAX, (b) => { r(b).rotation.loadings[0][0].value += 0.01; resumColumns(b, vals(r(b).rotation.loadings)); })],
    ['rotation iterations of 0', () => changed(VARIMAX, (b) => (r(b).rotation.iterations = 0))],
    ["rotation iterations past the spec's", () => changed(VARIMAX, (b) => (r(b).rotation.iterations = 26))],
    // The codes.
    ['the determinant no_correlation', () => changed(PROMAX, (b) => (r(b).determinant.value = nc('no_correlation')))],
    ['KMO distribution_out_of_range', () => changed(PROMAX, (b) => (r(b).kmo.overall = nc('distribution_out_of_range')))],
    ["Bartlett's p too_small", () => changed(PROMAX, (b) => (r(b).bartlett.p = nc('too_small')))],
    ['a mean correlations_singular', () => changed(PROMAX, (b) => (r(b).descriptives[0].mean = nc('correlations_singular')))],
    ['a loading too_small', () => changed(PROMAX, (b) => (r(b).extraction.loadings[0][0] = nc('too_small')))],
    ['correlations_singular on the log alone', () => changed(PROMAX, (b) => (r(b).determinant.log = nc('correlations_singular')))],
    // Decision 742's m1 (the review of 8bd82d8f): shapes stats_factor never writes, each one
    // change. Before this step storedFactor.vitest.ts accepted the first two of the determinant's
    // and Bartlett's as shapes Rust writes.
    ['the spec\'s iterations above 1,000', () => changed(PROMAX, () => undefined, (s) => (s.parameters.iterations = 5000))],
    ['the spec\'s convergence 5', () => changed(PROMAX, () => undefined, (s) => (s.parameters.convergence = 5))],
    ['the spec\'s convergence text', () => changed(PROMAX, () => undefined, (s) => (s.parameters.convergence = 'abc'))],
    ['the spec\'s convergence below 1e-12', () => changed(PROMAX, () => undefined, (s) => (s.parameters.convergence = 1e-13))],
    ['a missing code that is text', () => changed(PROMAX, () => undefined, (s) => (s.parameters.missingCodes = ['x']))],
    ['missing codes out of order', () => changed(PROMAX, () => undefined, (s) => (s.parameters.missingCodes = [2, 1]))],
    ['a missing code repeated', () => changed(PROMAX, () => undefined, (s) => (s.parameters.missingCodes = [1, 1]))],
    ['the determinant 0.9 beside its log', () => changed(PROMAX, (b) => (r(b).determinant.value = { value: 0.9 }))],
    ['the determinant too_small beside a log of -5.18', () => changed(PROMAX, (b) => (r(b).determinant.value = nc('too_small')))],
    ["Bartlett's p distribution_out_of_range", () => changed(PROMAX, (b) => (r(b).bartlett.p = nc('distribution_out_of_range')))],
    ['KMO no_correlation while the MSAs are values', () => changed(PROMAX, (b) => (r(b).kmo.overall = nc('no_correlation')))],
    ['every MSA no_correlation while KMO is a value', () => changed(PROMAX, (b) => r(b).kmo.items.forEach((a: O) => (a.msa = nc('no_correlation'))))],
    ['KMO too_small', () => changed(PROMAX, (b) => (r(b).kmo.overall = nc('too_small')))],
    ['an MSA overflow', () => changed(PROMAX, (b) => (r(b).kmo.items[0].msa = nc('overflow')))],
    ['a standard deviation below 0', () => changed(PROMAX, (b) => (r(b).descriptives[0].sd = { value: -651.5 }))],
    ['a warning naming a column that is no item', () => changed(PROMAX, (b) => (b.diagnostics = [{ code: code('suspect_missing_code'), params: { column: 9, value: -99, count: 2 }, severity: 'warning' }]))],
    ['an error among a succeeded run\'s diagnostics', () => changed(PROMAX, (b) => (b.diagnostics = [{ code: code('suspect_missing_code'), params: { column: 1, value: -99, count: 2 }, severity: 'error' }]))],
    ['the bundle\'s limitations emptied', () => changed(PROMAX, (b) => (b.limitations = []))],
    ['the bundle\'s limitations out of order', () => changed(PROMAX, (b) => b.limitations.reverse())],
    // One clause each (the review of 9C-1, finding 8): a warning of another code naming an item,
    // a limitation at another severity, a limitation with a parameter.
    ['a warning other than a suspect code', () => changed(PROMAX, (b) => (b.diagnostics = [{ code: code('item_constant'), params: { column: 1 }, severity: 'warning' }]))],
    ['a limitation as a warning', () => changed(PROMAX, (b) => (b.limitations[0].severity = 'warning'))],
    ['a limitation with a parameter', () => changed(PROMAX, (b) => (b.limitations[0].params = { column: 1 }))],
    // A log below p ln(1e-9): every Cholesky pivot left is above 1e-9 of a diagonal of 1
    // (stats_linalg::cholesky), so six items' log is above about -124.3 (the review of 9C-1,
    // finding 7).
    ['a six-item determinant too_small beside a log of -720', () => changed(PROMAX, (b) => {
      r(b).determinant.value = nc('too_small');
      r(b).determinant.log = { value: -720 };
      r(b).bartlett.chiSquare = { value: -((r(b).rowsUsed - 1 - (2 * 6 + 5) / 6) * -720) };
    })],
    // Exclusions and the manifest.
    ['an exclusion of a column not an item', () => changed(PROMAX, (b) => { r(b).exclusions[0].params.column = 9; b.manifest.exclusions[0].params.column = 9; })],
    ['missing_code with no codes', () => changed(PROMAX, (b) => { r(b).exclusions[0].code = code('missing_code'); b.manifest.exclusions[0].code = code('missing_code'); })],
    ['rows read other than used plus left out', () => changed(PROMAX, (b) => (b.manifest.rowsRead += 1))],
    ["the manifest's exclusions other than the result's", () => changed(PROMAX, (b) => (b.manifest.exclusions = []))],
    // A failed run.
    ['a failure code the run never gives', () => failed((f) => (f.code = code('too_few_rows')))],
    ['a singular item not among the items', () => failed((f) => (f.params.column = 4))],
    ['correlations_singular under principal components', () => failed(() => undefined, (s) => (s.parameters.extraction = 'principal_components'))],
    ["extraction_not_converged at a limit not the spec's", () => failed((f) => { f.code = code('extraction_not_converged'); f.params = { limit: 26 }; })],
    ["no_eigenvalue_over_one with a fixed number", () => failed((f) => { f.code = code('no_eigenvalue_over_one'); f.params = {}; })],
    ['extraction_eigenvalue_not_positive under principal axis', () => failed((f) => { f.code = code('extraction_eigenvalue_not_positive'); f.params = {}; })],
  ];

  it.each(cases)('refuses %s', (_name, make) => {
    expect(kindOf(make())).toBe(BAD);
  });

  it('accepts each isolated change undone: the helpers that recompute give back what Rust wrote', () => {
    expect(kindOf(changed(PROMAX, (b) => restructure(b)))).toBe(OK);
    expect(kindOf(changed(PROMAX, (b) => resum(r(b).eigenvalues, 6)))).toBe(OK);
    expect(kindOf(changed(PROMAX, (b) => (r(b).notes = [code('communality_above_one'), code('eigenvalues_not_distinct')])))).toBe(OK);
  });

  it('accepts the failed run with its limit when principal axis does not converge', () => {
    expect(kindOf(failed((f) => { f.code = code('extraction_not_converged'); f.params = { limit: 25 }; }))).toBe('failed');
  });
});
