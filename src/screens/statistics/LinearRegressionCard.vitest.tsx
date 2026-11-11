// The linear regression's result card (Research OS, Phase 9A-2, decisions 684 and 685), on
// both result fixtures Rust writes (src-tauri/tests/stats_linear_regression_wire.rs) and on
// modified copies of them for predicted_no_spread and too_small, which neither carries. The
// fixtures are a wire shape, not a computation. Every number goes through shownNumber; a
// value not computable shows its sentence; a column's name is text only, never in a key, an
// attribute, a title or a test id.
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { linearRegressionResultView, linearRegressionResultViewOther } from './fixtures/linearRegressionFixtures';
import type { LinearRegressionResultView, LinearRegressionStat } from './linearRegressionTypes';
import { LinearRegressionCard } from './LinearRegressionCard';
import { linearRegressionSentence } from './linearRegressionWording';
import { shownNumber } from './ttestWording';

afterEach(cleanup);

const NAMES = { column: (a: number) => `"col${a}"` };
const three = linearRegressionResultView as unknown as LinearRegressionResultView;
const perfect = linearRegressionResultViewOther as unknown as LinearRegressionResultView;
const cellsOf = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('th, td')).map((c) => c.textContent);
const heads = (id: string) => within(screen.getByTestId(id)).getAllByRole('columnheader').map((h) => h.textContent);
const v = (s: LinearRegressionStat) => shownNumber((s as { value: number }).value);
const sentence = (code: string) => linearRegressionSentence({ code: `stats.linear_regression.${code}` });
const nc = (code: string) => ({ not_computable: `stats.linear_regression.${code}` }) as LinearRegressionStat;

describe('the six tables, in order', () => {
  it("draws decision 685's headings in SPSS's order", () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    expect(screen.getAllByRole('heading').map((h) => h.textContent).slice(0, 6)).toEqual([
      'Model summary',
      'Analysis of variance',
      'Coefficients',
      'Correlations and collinearity',
      'Descriptive statistics',
      'Residuals statistics',
    ]);
  });

  it('gives the model summary its six rows, the rows used last', () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    const m = three.result.model;
    expect(cellsOf('lr-model')).toEqual([
      'R', v(m.r),
      'R²', v(m.rSquared),
      'Adjusted R²', v(m.adjustedRSquared),
      'Standard error of the estimate', v(m.standardErrorOfEstimate),
      'Durbin-Watson', v(m.durbinWatson),
      'Rows used', '10',
    ]);
  });

  it('gives the analysis of variance F and p on the Regression row only, and the Total row no mean square', () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    const a = three.result.anova;
    expect(heads('lr-anova')).toEqual(['', 'Sum of squares', 'df', 'Mean square', 'F', 'p']);
    expect(cellsOf('lr-anova-regression')).toEqual(['Regression', v(a.regression.sumOfSquares), '3', v(a.regression.meanSquare), v(a.f), v(a.p)]);
    expect(cellsOf('lr-anova-residual')).toEqual(['Residual', v(a.residual.sumOfSquares), '6', v(a.residual.meanSquare), '', '']);
    expect(cellsOf('lr-anova-total')).toEqual(['Total', v(a.total.sumOfSquares), '9', '', '', '']);
  });

  it("gives the coefficients the intercept first, then each predictor in the list's order, the interval in one cell", () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    expect(heads('lr-coefficients')).toEqual(['', 'B', 'Standard error', 'Beta', 't', 'p', '95% interval']);
    const rows = Array.from(screen.getByTestId('lr-coefficients').querySelectorAll('tbody tr')).map((r) => r.getAttribute('data-testid'));
    expect(rows).toEqual(['lr-coef-intercept', 'lr-coef-C2', 'lr-coef-C3', 'lr-coef-C4']);
    const c = three.result.intercept;
    expect(cellsOf('lr-coef-intercept')).toEqual(['(Intercept)', v(c.b), v(c.standardError), '', v(c.t), v(c.p), `${v(c.lower)} to ${v(c.upper)}`]);
    const p = three.result.predictors[1];
    expect(cellsOf('lr-coef-C3')).toEqual(['"col3"', v(p.b), v(p.standardError), v(p.beta), v(p.t), v(p.p), `${v(p.lower)} to ${v(p.upper)}`]);
  });

  it("names the outcome as text in decision 685's sentence under the coefficients", () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    expect(screen.getByTestId('lr-coefficients-sentence').textContent).toBe(
      `B is the change in "col1" for a one-unit change in a predictor, the other predictors held fixed; Beta is the change in "col1"'s standard deviations for a one-standard-deviation change in a predictor, the others held fixed.`
    );
  });

  it('gives each predictor its correlations, tolerance and VIF', () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    expect(heads('lr-correlations')).toEqual(['', 'Zero-order', 'Partial', 'Part', 'Tolerance', 'VIF']);
    const p = three.result.predictors[1];
    expect(cellsOf('lr-corr-C3')).toEqual(['"col3"', v(p.zeroOrder), v(p.partial), v(p.part), v(p.tolerance), v(p.vif)]);
    expect(screen.getAllByTestId(/^lr-corr-C/).map((r) => r.getAttribute('data-testid'))).toEqual(['lr-corr-C2', 'lr-corr-C3', 'lr-corr-C4']);
  });

  it('gives the descriptive statistics the outcome, then each predictor', () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    expect(heads('lr-descriptives')).toEqual(['Column', 'Mean', 'Standard deviation']);
    expect(screen.getAllByTestId(/^lr-desc-C/).map((r) => r.getAttribute('data-testid'))).toEqual(['lr-desc-C1', 'lr-desc-C2', 'lr-desc-C3', 'lr-desc-C4']);
    const c = three.result.columns[1];
    expect(cellsOf('lr-desc-C2')).toEqual(['"col2"', v(c.mean), v(c.sd)]);
  });

  it("gives the residuals statistics four rows, the standardized rows' mean and SD cells empty", () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    const r = three.result.residuals;
    expect(heads('lr-residuals')).toEqual(['', 'Minimum', 'Maximum', 'Mean', 'Standard deviation']);
    expect(cellsOf('lr-res-predicted')).toEqual(['Predicted value', v(r.predicted.minimum), v(r.predicted.maximum), v(r.predicted.mean), v(r.predicted.standardDeviation)]);
    expect(cellsOf('lr-res-residual')[0]).toBe('Residual');
    expect(cellsOf('lr-res-standardizedPredicted')).toEqual(['Standardized predicted value', v(r.standardizedPredicted.minimum), v(r.standardizedPredicted.maximum), '', '']);
    expect(cellsOf('lr-res-standardizedResidual')).toEqual(['Standardized residual', v(r.standardizedResidual.minimum), v(r.standardizedResidual.maximum), '', '']);
  });
});

describe('a perfect fit', () => {
  it("spans the intercept's coded cells, its beta cell included, with the sentence once", () => {
    render(<LinearRegressionCard result={perfect} names={NAMES} />);
    const cell = screen.getByTestId('lr-coef-intercept-not-computable');
    expect(cell.getAttribute('colspan')).toBe('5');
    expect(cell.textContent).toBe('Not computable: the model fits every row exactly, so there is no residual variation.');
    expect(cellsOf('lr-coef-intercept')).toEqual(['(Intercept)', '2', sentence('perfect_fit')]);
  });

  it("gives a predictor's coded cells the sentence and its beta its number", () => {
    render(<LinearRegressionCard result={perfect} names={NAMES} />);
    const p = perfect.result.predictors[0];
    const s = sentence('perfect_fit');
    expect(cellsOf(`lr-coef-${p.column}`)).toEqual(['"col1"', v(p.b), s, v(p.beta), s, s, s]);
    expect(screen.getByTestId(`lr-beta-${p.column}`).className).toBe('gds-mono');
  });

  it('gives the note in its own words', () => {
    render(<LinearRegressionCard result={perfect} names={NAMES} />);
    expect(screen.getByTestId('lr-notes').textContent).toBe(
      "The residuals are at the level of rounding error, the case R's summary.lm calls an essentially perfect fit, so the standard errors, t, p and intervals, where they are given, rest on rounding."
    );
  });

  it('draws no note where the result has none', () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    expect(screen.queryByTestId('lr-notes')).toBeNull();
  });
});

describe("the codes neither fixture carries", () => {
  it("shows predicted_no_spread's sentence where the standardized predicted values would be", () => {
    const r = three.result;
    const noSpread: LinearRegressionResultView = {
      ...three,
      result: { ...r, residuals: { ...r.residuals, standardizedPredicted: { minimum: nc('predicted_no_spread'), maximum: nc('predicted_no_spread') } } },
    };
    render(<LinearRegressionCard result={noSpread} names={NAMES} />);
    const s = 'Not computable: the predicted values do not vary, so they cannot be standardized.';
    expect(cellsOf('lr-res-standardizedPredicted')).toEqual(['Standardized predicted value', s, s, '', '']);
  });

  it("shows too_small's sentence in the mean square, F and p, and leaves the standard error of the estimate as stored", () => {
    const r = three.result;
    const small: LinearRegressionResultView = {
      ...three,
      result: {
        ...r,
        anova: { ...r.anova, residual: { ...r.anova.residual, meanSquare: nc('too_small') }, f: nc('too_small'), p: nc('too_small') },
      },
    };
    render(<LinearRegressionCard result={small} names={NAMES} />);
    const s = 'Not computable: a value along the way is too small for the arithmetic to keep its precision.';
    expect(cellsOf('lr-anova-residual')[3]).toBe(s);
    expect(screen.getByTestId('lr-anova-f').textContent).toBe(s);
    expect(screen.getByTestId('lr-anova-p').textContent).toBe(s);
    expect(cellsOf('lr-model-standardErrorOfEstimate')[1]).toBe(v(r.model.standardErrorOfEstimate));
  });

  it("says an interval whose ends hold different codes end by end", () => {
    const r = three.result;
    const p = { ...r.predictors[0], lower: nc('overflow'), upper: nc('too_small') };
    const mixed: LinearRegressionResultView = { ...three, result: { ...r, predictors: [p, ...r.predictors.slice(1)] } };
    render(<LinearRegressionCard result={mixed} names={NAMES} />);
    expect(screen.getByTestId(`lr-interval-${p.column}`).textContent).toBe(`${sentence('overflow')} to ${sentence('too_small')}`);
  });
});

describe('around the tables', () => {
  it('draws the exclusions by their columns, the limitations, the digits note and the manifest', () => {
    render(<LinearRegressionCard result={three} names={NAMES} />);
    expect(screen.getByTestId('lr-exclusions').querySelectorAll('li')).toHaveLength(1);
    expect(screen.getByTestId('lr-exclusions').textContent).toContain('1 row had no value in "col2"');
    const limits = Array.from(screen.getByTestId('lr-limitations').querySelectorAll('li')).map((l) => l.textContent);
    expect(limits).toEqual(three.result.limitations.map((code) => linearRegressionSentence({ code })));
    expect(limits).toHaveLength(10);
    expect(screen.getByTestId('lr-digits-note').textContent).toMatch(/six figures/);
    expect(screen.getByTestId('lr-result-notes').textContent).toContain('which may be a code for a missing value');
    expect(screen.getByTestId('lr-manifest')).toBeTruthy();
  });

  it('says so when no row was left out', () => {
    render(<LinearRegressionCard result={{ ...three, result: { ...three.result, exclusions: [] } }} names={NAMES} />);
    expect(screen.getByTestId('lr-exclusions').textContent).toContain('No row was left out.');
  });

  it('puts a label sentinel in no attribute or test id', () => {
    const sentinel = 'LBL_SENTINEL_9a2';
    const { container } = render(<LinearRegressionCard result={three} names={{ column: () => sentinel }} />);
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
