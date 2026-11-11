// The linear regression's saved results pass the shape check exactly as Rust writes them, and
// nothing else (Research OS, Phase 9A-2, decisions 687 and 688), on storedRelatedRanks.vitest.ts's
// pattern. Each refused case changes one field of a committed snapshot, so it breaks one rule of
// decision 688; each rule's red is a mutation of storedShape.ts. The snapshot files stay as they
// are.
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
const MODEL = 'linear_regression.json';
const PERFECT = 'linear_regression_perfect.json';
const FAILED = 'linear_regression_failed.json';
const OK = 'linear_regression';
const BAD = 'badShape';
const code = (c: string) => `stats.linear_regression.${c}`;
const nc = (c: string) => ({ not_computable: code(c) });
const r = (b: O) => b.primaryOutputs;
const pred = (b: O, j: number) => r(b).predictors[j];

describe("the linear regression's saved results (decision 687)", () => {
  it('opens every committed snapshot as the regression, or its failure', () => {
    expect(kindOf(snapshot(MODEL))).toBe(OK);
    expect(kindOf(snapshot(PERFECT))).toBe(OK);
    const f = checkStored(snapshot(FAILED));
    expect(f.kind).toBe('failed');
    expect(f.kind === 'failed' && f.capability).toBe('linear_regression');
  });
});

describe('what stats_linear_regression never writes (decision 688)', () => {
  const refused: Array<[string, () => O]> = [
    // Shape.
    ['an extra key in the result', () => changed(MODEL, (b) => (r(b).extra = 1))],
    ['an extra key in a predictor', () => changed(MODEL, (b) => (pred(b, 0).extra = 1))],
    ["an outcome other than the spec's", () => changed(MODEL, (b) => (r(b).outcome = 'C9'))],
    ["a column other than the spec's at its place", () => changed(MODEL, (b) => (r(b).columns[1].column = 'C9'))],
    ["a predictor other than the spec's at its place", () => changed(MODEL, (b) => (pred(b, 0).column = 'C9'))],
    ['a spec naming a predictor twice', () =>
      changed(
        MODEL,
        (b) => {
          // The snapshot's one exclusion names C3, which the changed spec no longer has.
          r(b).columns[2].column = 'C2';
          pred(b, 1).column = 'C2';
          r(b).exclusions = [];
          b.manifest.exclusions = [];
          b.manifest.rowsRead -= 1;
        },
        (s) => (s.parameters.predictors = ['C2', 'C2', 'C4'])
      )],
    ['the limitations reordered', () => changed(MODEL, (b) => r(b).limitations.reverse())],
    // On the perfect fit, whose rule gives a note, so only the note's code is wrong.
    ['a note other than essentially_perfect_fit', () => changed(PERFECT, (b) => (r(b).notes = [code('enter_only')]))],
    // Degrees of freedom.
    ['a regression df other than k', () => changed(MODEL, (b) => (r(b).anova.regression.df = 4))],
    ['a residual df other than n - k - 1', () => changed(MODEL, (b) => (r(b).anova.residual.df = 6))],
    ['a total df other than n - 1', () => changed(MODEL, (b) => (r(b).anova.total.df = 9))],
    // Sums and squares.
    ['a regression mean square other than regSS / k', () => changed(MODEL, (b) => (r(b).anova.regression.meanSquare.value *= 2))],
    ['a regression mean square too_small where regSS / k is normal', () => changed(MODEL, (b) => (r(b).anova.regression.meanSquare = nc('too_small')))],
    ['a total SS other than regSS + resSS', () => changed(MODEL, (b) => (r(b).anova.total.sumOfSquares.value += 1))],
    ['a residual mean square other than resSS / df', () => changed(MODEL, (b) => (r(b).anova.residual.meanSquare.value *= 2))],
    // The model.
    ['an R² other than regSS / (regSS + resSS)', () => changed(MODEL, (b) => (r(b).model.rSquared.value += 1e-12))],
    ['an R other than the root of R²', () => changed(MODEL, (b) => (r(b).model.r.value += 1e-12))],
    ["an adjusted R² other than Rust's", () => changed(MODEL, (b) => (r(b).model.adjustedRSquared.value += 1e-12))],
    ['a standard error of the estimate other than σ', () =>
      changed(MODEL, (b) => {
        // The standardized residuals divide by the stored σ, so they move with it, and only
        // the σ rule can refuse this.
        const s = r(b).model.standardErrorOfEstimate;
        s.value *= 2;
        const e = r(b).residuals;
        e.standardizedResidual.minimum.value = e.residual.minimum.value / s.value;
        e.standardizedResidual.maximum.value = e.residual.maximum.value / s.value;
      })],
    // F.
    ['an F other than the mean squares\' ratio', () => changed(MODEL, (b) => (r(b).anova.f.value *= 2))],
    ["an F's p above 1", () => changed(MODEL, (b) => (r(b).anova.p = { value: 1.5 }))],
    // Each coefficient.
    ['a standard error of 0 or below', () =>
      changed(MODEL, (b) => {
        const c = r(b).intercept;
        c.standardError.value = -c.standardError.value;
        c.t.value = c.b.value / c.standardError.value;
      })],
    ['a t other than b / SE', () => changed(MODEL, (b) => (r(b).intercept.t.value *= 2))],
    ['one end of an interval a code and the other a value', () => changed(MODEL, (b) => (r(b).intercept.lower = nc('overflow')))],
    ['a lower end above b', () => changed(MODEL, (b) => (r(b).intercept.lower.value = r(b).intercept.b.value + 1))],
    ['a value in the perfect-fit set of a perfect fit', () => changed(PERFECT, (b) => (r(b).intercept.standardError = { value: 1 }))],
    ['perfect_fit where the residuals are not zero', () => changed(MODEL, (b) => (r(b).model.durbinWatson = nc('perfect_fit')))],
    // Each predictor.
    ["a beta other than Rust's", () => changed(MODEL, (b) => (pred(b, 0).beta.value *= 2))],
    ['a zero-order correlation above 1', () => changed(MODEL, (b) => (pred(b, 0).zeroOrder.value = 1.5))],
    ["a partial correlation other than Rust's from t", () => changed(MODEL, (b) => (pred(b, 0).partial.value += 1e-9))],
    ["a part correlation other than Rust's from t", () => changed(MODEL, (b) => (pred(b, 0).part.value += 1e-9))],
    ['a tolerance other than 1 with one predictor', () =>
      changed(PERFECT, (b) => {
        pred(b, 0).tolerance.value = 0.5;
        pred(b, 0).vif.value = 2;
      })],
    ['a tolerance other than 1 / VIF', () => changed(MODEL, (b) => (pred(b, 0).tolerance.value *= 2))],
    // The residuals.
    ["a residuals' mean above their maximum", () => changed(MODEL, (b) => (r(b).residuals.residual.mean.value = 2))],
    ["a standardized predicted value other than Rust's", () => changed(MODEL, (b) => (r(b).residuals.standardizedPredicted.minimum.value *= 1.5))],
    ["predicted_no_spread where Rust's rule does not give it", () =>
      changed(MODEL, (b) => (r(b).residuals.standardizedPredicted = { minimum: nc('predicted_no_spread'), maximum: nc('predicted_no_spread') }))],
    ["a standardized residual other than Rust's", () => changed(MODEL, (b) => (r(b).residuals.standardizedResidual.maximum.value *= 2))],
    // The codes.
    ['a code in a field Rust always writes as a value', () => changed(MODEL, (b) => (r(b).columns[1].mean = nc('too_small')))],
    // Cases the independent review of 9A-2 found missing (finding 3).
    ['fewer rows than the predictors and two', () =>
      changed(PERFECT, (b) => {
        r(b).rowsUsed = 2;
        r(b).anova.residual.df = 0;
        r(b).anova.total.df = 1;
        b.manifest.rowsUsed = 2;
        b.manifest.rowsRead -= 1;
      })],
    ['F a code while its p is a value', () => changed(MODEL, (b) => (r(b).anova.f = nc('overflow')))],
    ['t a code while partial and part are values', () => changed(MODEL, (b) => (pred(b, 0).t = nc('overflow')))],
    ['the interval ends holding two different codes', () =>
      changed(MODEL, (b) => {
        r(b).intercept.lower = nc('overflow');
        r(b).intercept.upper = nc('distribution_out_of_range');
      })],
    ['an upper end below b', () => changed(MODEL, (b) => (r(b).intercept.upper.value = r(b).intercept.b.value - 1))],
    ['a residual mean square too_small where σ² is normal', () => changed(MODEL, (b) => (r(b).anova.residual.meanSquare = nc('too_small')))],
    ["the predicted values' mean above their maximum", () => changed(MODEL, (b) => (r(b).residuals.predicted.mean.value = r(b).residuals.predicted.maximum.value + 1))],
    ["a manifest's rows used other than the result's", () =>
      changed(MODEL, (b) => {
        b.manifest.rowsUsed += 1;
      })],
    ['an exclusion of the outcome after a predictor', () =>
      changed(MODEL, (b) => {
        r(b).exclusions.push({ code: code('missing_value'), count: 1, params: { column: 1 } });
        b.manifest.exclusions = r(b).exclusions;
        b.manifest.rowsRead += 1;
      })],
    ['a suspect-code warning without its value and count', () =>
      changed(MODEL, (b) => (b.diagnostics = [{ code: code('suspect_missing_code'), params: { column: 1 }, severity: 'warning' }]))],
    // The note.
    ['no note on a perfect fit', () => changed(PERFECT, (b) => (r(b).notes = []))],
    ['a note far above the threshold', () => changed(MODEL, (b) => (r(b).notes = [code('essentially_perfect_fit')]))],
    // Exclusions and the manifest.
    ['an exclusion twice', () =>
      changed(MODEL, (b) => {
        r(b).exclusions.push({ ...r(b).exclusions[0] });
        b.manifest.exclusions = r(b).exclusions;
        b.manifest.rowsRead += 1;
      })],
    ['missing_code with no missing codes', () =>
      changed(MODEL, (b) => {
        r(b).exclusions[0].code = code('missing_code');
        b.manifest.exclusions = r(b).exclusions;
      })],
    ['rows read other than rows used and the exclusions', () => changed(MODEL, (b) => (b.manifest.rowsRead += 1))],
    ["manifest exclusions unlike the result's", () => changed(MODEL, (b) => (b.manifest.exclusions = []))],
    // A failed run.
    ['a failure that is not one of the run\'s two refusals', () => failed((f) => (f.code = code('no_predictors')))],
    ["a collinear predictor that is not one of the spec's", () => failed((f) => (f.params.column = 1))],
    ['a collinear failure with another parameter', () => failed((f) => (f.params.count = 2))],
    ['fit_out_of_range naming a column', () => failed((f) => (f.code = code('fit_out_of_range')))],
  ];

  it.each(refused)('refuses %s', (_name, make) => {
    expect(kindOf(make())).toBe(BAD);
  });

  it('accepts what Rust writes beside them', () => {
    expect(
      kindOf(
        failed((f) => {
          f.code = code('fit_out_of_range');
          f.params = {};
        })
      )
    ).toBe('failed');
    // A spec with a missing code that no row held: missing_code may be absent.
    expect(kindOf(changed(MODEL, () => undefined, (s) => (s.parameters.missingCodes = [-99])))).toBe(OK);
  });
});
