// The linear regression's words (Research OS, Phase 9A-2, decisions 684 and 685). Every code
// the screen can show (linearRegressionCodes.json, pinned to Rust's closed sets by
// src-tauri/tests/stats_linear_regression_wire.rs) has a sentence: its own 38, and the run's,
// the registry's and the workspace's as the related rank tests give them, the two that name
// the method renamed. No sentence says "significant", carries a star, advises a test, or uses
// a word a verdict would use. "Remove all" and "Remove" are labels, not verdicts, and fail the
// list: the settings import them from windowWording, and this module never holds them.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import linearRegressionCodes from './linearRegressionCodes.json';
import LIMITS from './describeLimits.json';
import REGRESSION_LIMITS from './fixtures/linearRegressionLimits.json';
import { CAPABILITY_WORDS } from './capabilityWording';
import { alphaMissingCodeQuestion } from './alphaWording';
import { RANKS_WORDS } from './ranksWording';
import { RELATED_RANKS_WORDS } from './relatedRanksWording';
import * as words from './linearRegressionWording';

/** ranksWording.vitest.ts's list, copied: importing a test file would run its tests here. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;
const VERDICT = (s: string) =>
  /significan/i.test(s) ||
  /[*★☆]/.test(s) ||
  /\b(should|recommend)/i.test(s) ||
  ALPHA_VERDICT.test(s) ||
  /\b(valid|accurate)\b/i.test(s) ||
  // Decision 685's own list.
  /\b(reject|good fit|poor fit)\b/i.test(s);

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2, lifecycle: 'planned' };
const WORKSPACE = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORKSPACE : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };
const say = (c: string, params: Record<string, unknown> = {}) => words.linearRegressionSentence({ code: `stats.linear_regression.${c}`, params }, NAMES);

function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'LINEAR_REGRESSION_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const count of [1, 2]) {
    for (const code of linearRegressionCodes) out.push(words.linearRegressionSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES));
  }
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.linearRegressionRunEndedSentence(state));
  }
  out.push(words.linearRegressionCoefficientsSentence('Score'));
  out.push(words.linearRegressionMissingCodeQuestion(-99, [{ header: 'a', cells: 2 }]));
  out.push(words.linearRegressionTooManyPredictors(51));
  out.push(words.linearRegressionMoveUp('C3'), words.linearRegressionMoveDown('C3'));
  return out;
}

describe('the linear regression screen words every code it can show', () => {
  it('has a sentence for each code in linearRegressionCodes.json, and none for any other', () => {
    expect(linearRegressionCodes).toHaveLength(38 + 11 + 5 + 6);
    expect(linearRegressionCodes.filter((c) => c.startsWith('stats.linear_regression.'))).toHaveLength(38);
    expect(Object.keys(words.LINEAR_REGRESSION_WORDS).sort()).toEqual([...linearRegressionCodes].sort());
    for (const code of linearRegressionCodes) {
      for (const count of [1, 2]) {
        const s = words.linearRegressionSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES);
        expect(s, code).not.toMatch(/no wording/);
        expect(s, code).toMatch(/[.:]$/);
        expect(s, code).not.toContain('undefined');
        expect(s, code).not.toContain('?');
      }
    }
  });

  it('no sentence passes a verdict, carries a star, or advises a test', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(120);
    expect(all.filter(VERDICT)).toEqual([]);
    for (const bad of ['The predictor is significant.', 'You should drop it.', 'A good fit.', 'This is valid.', 'Reject the null.', 'p < 0.05 *', 'Remove all']) {
      expect(VERDICT(bad), bad).toBe(true);
    }
  });

  it('does not hold the labels the verdict list would refuse, which the settings take from windowWording', () => {
    const text = JSON.stringify(Object.values(words).filter((v) => typeof v !== 'function'));
    expect(text).not.toContain('Remove');
  });

  it("takes the run, registry and workspace sentences from the related rank tests' where they name no method", () => {
    const renamed = ['run.unknown_run', 'run.wrong_kind'];
    const shared = linearRegressionCodes.filter((c) => !c.startsWith('stats.linear_regression.') && !renamed.includes(c));
    expect(shared).toHaveLength(11 + 5 + 6 - 2);
    for (const code of shared) expect(words.LINEAR_REGRESSION_WORDS[code], code).toBe(RELATED_RANKS_WORDS[code]);
    expect(LIMITS.maxRuns).toBe(40);
    expect(words.linearRegressionSentence({ code: 'run.unknown_run' })).toBe(
      'This run is no longer held. Gaply keeps the last 40 runs in memory only; run the linear regression again.'
    );
    expect(words.linearRegressionSentence({ code: 'run.wrong_kind' })).toBe('This run is not a linear regression, so this screen cannot read it.');
  });

  it('says the codes the rank tests share, and that name no method, as the rank tests say them', () => {
    const same = ['unknown_capability', 'input_mismatch', 'preparation_mismatch', 'missing_code_not_finite', 'unknown_column', 'code_equals_score', 'no_weights'];
    const params = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2 };
    for (const suffix of same) {
      const ours = words.linearRegressionSentence({ code: `stats.linear_regression.${suffix}`, params, count: 3 }, NAMES);
      const theirs = (RANKS_WORDS as Record<string, (d: unknown, n: unknown) => string>)[`stats.ranks.${suffix}`]({ code: `stats.ranks.${suffix}`, params, count: 3 }, { ...NAMES, level: () => '' });
      expect(ours, suffix).toBe(theirs);
    }
  });

  it("says each limitation as the registry's limit with the same meaning says it", () => {
    const pairs: Array<[string, string]> = [
      ['enter_only', 'regression_enter_only'],
      ['no_categorical_predictors', 'regression_no_categorical_predictors'],
      ['no_interaction_terms', 'regression_no_interaction_terms'],
      ['intercept_always', 'regression_intercept_always'],
      ['listwise_only', 'regression_listwise_only'],
      ['no_robust_standard_errors', 'regression_no_robust_errors'],
      ['no_casewise_diagnostics', 'regression_no_casewise_diagnostics'],
      ['durbin_watson_row_order', 'durbin_watson_row_order'],
      ['intervals_95_only', 'regression_intervals_95_only'],
    ];
    for (const [ours, cap] of pairs) {
      const theirs = (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${cap}`];
      expect(theirs, cap).toBeTruthy();
      expect(say(ours), ours).toBe(theirs);
    }
    expect(say('no_weights')).toBe('Each row is counted once; weighting rows by a column is not offered.');
  });

  it("says decision 685's not-computable sentences, the note and the run's refusals word for word", () => {
    expect(say('perfect_fit')).toBe('Not computable: the model fits every row exactly, so there is no residual variation.');
    expect(say('overflow')).toBe('Not computable: a value along the way is too large for the arithmetic.');
    expect(say('too_small')).toBe('Not computable: a value along the way is too small for the arithmetic to keep its precision.');
    expect(say('predicted_no_spread')).toBe('Not computable: the predicted values do not vary, so they cannot be standardized.');
    expect(say('distribution_out_of_range')).toBe(
      'Not computable: this needs a distribution at more than 10,000,000 degrees of freedom, where Gaply has not measured its accuracy.'
    );
    expect(say('essentially_perfect_fit')).toBe(
      "The residuals are at the level of rounding error, the case R's summary.lm calls an essentially perfect fit, so the standard errors, t, p and intervals, where they are given, rest on rounding."
    );
    expect(say('predictors_collinear', { column: 4 })).toBe(
      `"col4" is, within R's tolerance, a linear combination of the intercept and the predictors before it, so its effect cannot be separated from theirs. Take it out of the predictors.`
    );
    expect(say('fit_out_of_range')).toBe("The values are too large or too small for the regression's sums of squares to be held, so it cannot be computed at this scale.");
  });

  it("says decision 685's settings words word for word", () => {
    expect(words.LR_OUTCOME_LABEL).toBe('Outcome (the column to explain)');
    expect(words.LR_OUTCOME_EMPTY).toBe('Choose a column…');
    expect(words.LR_PREDICTORS_LEGEND).toBe('Predictors, entered together');
    expect(words.LR_LIST_HEADING).toBe('Predictors, in this order');
    expect(words.LR_RULE).toBe('Every chosen predictor enters the model at once. A row is used only when the outcome and every predictor have a value.');
    expect(words.LR_CHOOSE_OUTCOME).toBe('Choose the outcome.');
    expect(words.LR_CHOOSE_PREDICTOR).toBe('Choose at least one predictor.');
    expect(words.linearRegressionTooManyPredictors(51)).toBe('Choose 50 predictors or fewer; 51 are chosen.');
    expect(words.linearRegressionMoveUp('C3')).toBe('Move up C3');
    expect(words.linearRegressionMoveDown('C3')).toBe('Move down C3');
  });

  it("says decision 685's card words word for word", () => {
    expect(words.LR_HEADINGS).toEqual({
      model: 'Model summary',
      anova: 'Analysis of variance',
      coefficients: 'Coefficients',
      correlations: 'Correlations and collinearity',
      descriptives: 'Descriptive statistics',
      residuals: 'Residuals statistics',
    });
    expect(words.LR_MODEL_LABELS).toEqual({
      r: 'R',
      rSquared: 'R²',
      adjustedRSquared: 'Adjusted R²',
      standardErrorOfEstimate: 'Standard error of the estimate',
      durbinWatson: 'Durbin-Watson',
      rowsUsed: 'Rows used',
    });
    expect(words.LR_ANOVA_ROWS).toEqual({ regression: 'Regression', residual: 'Residual', total: 'Total' });
    expect(words.LR_ANOVA_HEADS).toEqual(['Sum of squares', 'df', 'Mean square', 'F', 'p']);
    expect(words.LR_INTERCEPT).toBe('(Intercept)');
    expect(words.LR_COEFFICIENT_HEADS).toEqual(['B', 'Standard error', 'Beta', 't', 'p', '95% interval']);
    expect(words.LR_CORRELATION_HEADS).toEqual(['Zero-order', 'Partial', 'Part', 'Tolerance', 'VIF']);
    expect(words.LR_DESCRIPTIVE_HEADS).toEqual(['Mean', 'Standard deviation']);
    expect(words.LR_RESIDUAL_ROWS).toEqual({
      predicted: 'Predicted value',
      residual: 'Residual',
      standardizedPredicted: 'Standardized predicted value',
      standardizedResidual: 'Standardized residual',
    });
    expect(words.LR_RESIDUAL_HEADS).toEqual(['Minimum', 'Maximum', 'Mean', 'Standard deviation']);
    expect(words.linearRegressionCoefficientsSentence('Score')).toBe(
      "B is the change in Score for a one-unit change in a predictor, the other predictors held fixed; Beta is the change in Score's standard deviations for a one-standard-deviation change in a predictor, the others held fixed."
    );
    expect(words.linearRegressionInterval(-0.5, 1.25)).toBe('-0.5 to 1.25');
    expect(words.LINEAR_REGRESSION_EXPORT_CHOICES).toEqual([
      { format: 'csv', label: 'The result tables (CSV)', file: 'linear_regression.csv', mime: 'text/csv' },
      { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'linear_regression_manifest.json', mime: 'application/json' },
    ]);
  });

  it("writes a code's question from alpha's places, and what ticking it does", () => {
    const where = [{ header: 'a', cells: 1 }, { header: 'b', cells: 2 }];
    const alpha = alphaMissingCodeQuestion(-99, where);
    const ours = words.linearRegressionMissingCodeQuestion(-99, where);
    expect(ours.split('. If you tick')[0]).toBe(alpha.split('. If you tick')[0]);
    expect(ours).toBe(
      'Is -99 a code for a missing value? It appears in "a" (1 cell), "b" (2 cells). If you tick it, it is treated as missing in the outcome and every predictor, and a row with it is left out.'
    );
  });

  it('shows a number through shownNumber, and a code as its sentence where the number would be', () => {
    expect(words.linearRegressionStatText({ value: 2 })).toBe('2');
    expect(words.linearRegressionStatText({ not_computable: 'stats.linear_regression.too_small' })).toBe(say('too_small'));
  });

  it('reads its predictor limit from the wire fixture, not typed twice', () => {
    expect(REGRESSION_LIMITS).toEqual({ maxPredictors: 50 });
    expect(words.MAX_PREDICTORS).toBe(REGRESSION_LIMITS.maxPredictors);
    const source = fs.readFileSync(path.join(__dirname, 'linearRegressionWording.ts'), 'utf8');
    expect(source).toContain("from './fixtures/linearRegressionLimits.json'");
    expect(source).toMatch(/MAX_PREDICTORS: number = REGRESSION_LIMITS\.maxPredictors/);
    expect(source).not.toMatch(/MAX_PREDICTORS[^\n]*= *50/);
  });
});
