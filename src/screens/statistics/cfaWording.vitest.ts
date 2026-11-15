// The confirmatory factor analysis's words (Research OS, Phase 9C-3, decisions 782 and 783).
// Every code the screen can show (cfaCodes.json, pinned to Rust's closed sets by
// src-tauri/tests/stats_cfa_wire.rs) has a sentence: its own 50, and the run's, the registry's
// and the workspace's as the related rank tests give them, the two that name the method
// renamed. Decision 783's words are pinned word for word. No sentence or label says a word a
// verdict would use, carries a star, or gives a fit measure a cutoff or a band's name; the
// lists are alphaWording.vitest.ts's, PairedFlow.vitest.tsx's, projectWording.vitest.ts's,
// ttestWording.vitest.ts's scan, ranksWording.vitest.ts's and factorWording.vitest.ts's, copied
// (importing a test file would run its tests here), with decision 783's own.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import cfaCodes from './cfaCodes.json';
import LIMITS from './describeLimits.json';
import CFA_LIMITS from './fixtures/cfaLimits.json';
import validationJson from './fixtures/cfaValidationView.json';
import { CAPABILITY_WORDS } from './capabilityWording';
import { ALPHA_WORDS } from './alphaWording';
import { RANKS_WORDS } from './ranksWording';
import { RELATED_RANKS_WORDS } from './relatedRanksWording';
import { LINEAR_REGRESSION_WORDS } from './linearRegressionWording';
import { DISTRIBUTION_OUT_OF_RANGE_SENTENCE, OVERFLOW_SENTENCE, TOO_SMALL_SENTENCE } from './factorWording';
import * as words from './cfaWording';
import { CFA_DIALOG_TITLE, cfaSettingsLine } from './window/windowWording';

/** alphaWording.vitest.ts:20's words. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;
/** PairedFlow.vitest.tsx:135's. */
const PAIRED_VERDICT = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;
/** projectWording.vitest.ts:47's. */
const PROJECT_VERDICT = /\b(significant|correct|incorrect|accurate|inaccurate|reliable|unreliable|trustworthy|wrong|invalid|suspicious|fraud)\b|★|✓/i;
/** factorWording.vitest.ts's own list (decision 730). */
const FACTOR_OWN = /\b(significance|valid|acceptable|reject|recommend|marvelous|meritorious|middling|mediocre|miserable|unacceptable)\b/i;
/** Decision 783's own list, and the names fit bands are sometimes given. */
const OWN = /\b(significant|significance|valid|should|good|poor|close|adequate|acceptable|reliable|reject|recommend|excellent|mediocre|fair)\b/i;
/** A cutoff beside a fit measure: .05, .90 or .95, but in AMOS's own column names. */
const CUTOFF = (s: string) => /(^|[^0-9])\.(05|90|95)\b|0\.(05|90|95)\b/.test(s.replace(/HOELTER \.0[15]/g, ''));
/** The other methods' lists, which every sentence passes, shared ones included. */
const VERDICT_SHARED = (s: string) =>
  /significan/i.test(s) ||
  /[*★☆]/.test(s) ||
  /\b(should|recommend)/i.test(s) ||
  ALPHA_VERDICT.test(s) ||
  PAIRED_VERDICT.test(s) ||
  PROJECT_VERDICT.test(s) ||
  FACTOR_OWN.test(s) ||
  /\b(valid|accurate)\b/i.test(s);
/** With decision 783's own list, which governs every NEW word: a sentence taken verbatim from
 *  the related rank tests (the run's, registry's and workspace's, pinned identical below) is
 *  not new, and "Statistics is open. Close it first." is one of them. */
const VERDICT = (s: string) => VERDICT_SHARED(s) || OWN.test(s) || CUTOFF(s);
const SHARED_CODES = cfaCodes.filter((c) => !c.startsWith('stats.cfa.') && !['run.unknown_run', 'run.wrong_kind'].includes(c));
const sharedSentences = () => new Set(SHARED_CODES.flatMap((code) => [0, 1, 2].map((count) => words.cfaSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES))));

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2, lifecycle: 'planned' };
const WORKSPACE = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORKSPACE : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };
const say = (c: string, params: Record<string, unknown> = {}) => words.cfaSentence({ code: `stats.cfa.${c}`, params }, NAMES);
const VIEW = {
  profile: {
    columns: [
      { alias: 'C1', header: 'q1' },
      { alias: 'C2', header: 'q2' },
      { alias: 'C3', header: 'q3' },
      { alias: 'C4', header: 'q4' },
    ],
  },
};

/** Every string the module exports, every code's sentence, every cell sentence, and every
 *  sentence a function builds, as the screen can show them. */
function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'CFA_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const count of [0, 1, 2]) {
    for (const code of cfaCodes) out.push(words.cfaSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES));
  }
  out.push(say('overflow'), say('model_not_identified'));
  for (const code of cfaCodes) out.push(words.cfaCellSentence(code), words.cfaDashLine(code));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.cfaRunEndedSentence(state));
  }
  for (const n of [0, 1, 2, 500]) out.push(words.cfaIterationsSentence(n));
  out.push(words.cfaChiSquareSentence({ value: 7.853 }, 8, { value: 0.448 }));
  out.push(words.cfaChiSquareSentence({ value: 0 }, 0, { not_computable: 'stats.cfa.df_zero' }));
  out.push(words.cfaFactorTooFew(2, 1, 2), words.cfaFactorTooFew(2, 0, 2), words.cfaFactorOption(3), words.cfaFactorHeading(3));
  out.push(words.cfaWeightRow('"q1"', 1), words.cfaCovarianceRow(1, 2), words.cfaFactorRow(1), words.cfaErrorRow('"q1"'));
  out.push(words.cfaMissingCodeQuestion(-99, [{ header: 'a', cells: 2 }]));
  out.push(CFA_DIALOG_TITLE, cfaSettingsLine({ indicators: [['C1', 'C2'], ['C3', 'C4']], correlatedFactors: false, missingCodes: [-99] }, VIEW));
  return out;
}

describe('the confirmatory factor analysis screen words every code it can show', () => {
  it('has a sentence for each code in cfaCodes.json, and none for any other', () => {
    expect(cfaCodes).toHaveLength(50 + 11 + 5 + 6);
    expect(cfaCodes.filter((c) => c.startsWith('stats.cfa.'))).toHaveLength(50);
    expect(Object.keys(words.CFA_WORDS).sort()).toEqual([...cfaCodes].sort());
    for (const code of cfaCodes) {
      for (const count of [0, 1, 2]) {
        const s = words.cfaSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES);
        expect(s, code).not.toMatch(/no wording/);
        expect(s, code).toMatch(/[.:]$/);
        expect(s, code).not.toContain('undefined');
        expect(s, code).not.toContain('?');
      }
    }
  });

  it('no sentence or label passes a verdict, carries a star, advises, or gives a fit measure a cutoff', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(250);
    const SHARED = sharedSentences();
    expect(all.filter((s) => (SHARED.has(s) ? VERDICT_SHARED(s) : VERDICT(s)))).toEqual([]);
    expect(SHARED.size).toBeGreaterThan(10);
    for (const bad of ['A good fit.', 'The fit is close.', 'An adequate model.', 'RMSEA below .05', 'CFI of 0.95 or more', 'TLI above .90', 'This is valid.', 'Reject the model.', 'You should drop it.', 'significance', 'Remove all']) {
      expect(VERDICT(bad), bad).toBe(true);
    }
    for (const ok of ['HOELTER .05', 'HOELTER .01', 'LO 90', 'HI 90', 'PCLOSE', 'confirmatory factor analysis']) expect(VERDICT(ok), ok).toBe(false);
  });

  it("takes the run, registry and workspace sentences from the related rank tests' where they name no method", () => {
    const renamed = ['run.unknown_run', 'run.wrong_kind'];
    const shared = cfaCodes.filter((c) => !c.startsWith('stats.cfa.') && !renamed.includes(c));
    expect(shared).toHaveLength(11 + 5 + 6 - 2);
    for (const code of shared) expect(words.CFA_WORDS[code], code).toBe(RELATED_RANKS_WORDS[code]);
    expect(words.cfaSentence({ code: 'run.unknown_run' })).toBe(
      `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the confirmatory factor analysis again.`
    );
    expect(words.cfaSentence({ code: 'run.wrong_kind' })).toBe('This run is not a confirmatory factor analysis, so this screen cannot read it.');
  });

  it('says the codes it shares as the method that has them says them, where that names no other method and no item', () => {
    const params = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2 };
    const d = (prefix: string, suffix: string) => ({ code: `${prefix}${suffix}`, params, count: 3 });
    for (const suffix of ['unknown_capability', 'input_mismatch', 'preparation_mismatch', 'missing_code_not_finite', 'unknown_column', 'code_equals_score', 'no_weights']) {
      const theirs = (RANKS_WORDS as Record<string, (d: unknown, n: unknown) => string>)[`stats.ranks.${suffix}`](d('stats.ranks.', suffix), { ...NAMES, level: () => '' });
      expect(words.cfaSentence(d('stats.cfa.', suffix), NAMES), suffix).toBe(theirs);
    }
    for (const suffix of ['missing_code_repeated', 'missing_codes_out_of_order']) {
      expect(words.cfaSentence(d('stats.cfa.', suffix), NAMES), suffix).toBe(ALPHA_WORDS[`stats.alpha.${suffix}`](d('stats.alpha.', suffix), NAMES));
    }
    expect(words.cfaSentence(d('stats.cfa.', 'suspect_missing_code'), NAMES)).toBe(
      LINEAR_REGRESSION_WORDS['stats.linear_regression.suspect_missing_code'](d('stats.linear_regression.', 'suspect_missing_code'), NAMES)
    );
    expect([say('too_small'), say('distribution_out_of_range'), say('overflow')]).toEqual([TOO_SMALL_SENTENCE, DISTRIBUTION_OUT_OF_RANGE_SENTENCE, OVERFLOW_SENTENCE]);
  });

  it('rewords the shared sentences that name another method or say item (each listed in the report)', () => {
    expect(say('capability_version_mismatch', { found: 2, expected: 1 })).toBe('These settings were made for version 2 of the confirmatory factor analysis, and this is version 1.');
    expect(say('input_count', { count: 2 })).toBe('These settings name 2 files, and a confirmatory factor analysis reads exactly one.');
    expect(say('too_few_items', { count: 2, limit: 3 })).toBe('The model has 2 columns, and the confirmatory factor analysis needs at least 3.');
    expect(say('too_many_items', { count: 129, limit: 128 })).toBe('The model has 129 columns, and the confirmatory factor analysis takes at most 128.');
    expect(say('item_repeated', { column: 2 })).toBe('Column "col2" is in the model twice.');
    expect(say('column_non_finite', { column: 2, count: 1 })).toBe('Column "col2" holds an infinity or "not a number" in 1 cell, so it cannot be in the model.');
    expect(say('column_holds_text', { column: 2, count: 3 })).toBe('Column "col2" holds text in 3 cells, so it cannot be in the model.');
    expect(say('no_complete_rows')).toBe('No row has a value in every column in the model, so there is nothing to analyse.');
    expect(say('item_constant', { column: 2 })).toBe('Column "col2" takes one value in every complete row, so its correlations are not defined. Take it out of the model.');
    expect(words.cfaSentence({ code: 'stats.cfa.missing_value', params: { column: 2 }, count: 1 }, NAMES)).toBe(
      '1 row had no value (an empty cell or a missing-value token) in "col2", the first column in the model without one, and was left out.'
    );
    expect(words.cfaSentence({ code: 'stats.cfa.missing_code', params: { column: 2 }, count: 3 }, NAMES)).toBe(
      '3 rows held a missing code you ticked in "col2", the first column in the model without a value, and were left out.'
    );
    const all = cfaCodes.map((code) => words.cfaSentence({ code, params: paramsFor(code), count: 2 }, NAMES));
    expect(all.filter((s) => /\b(a|the) factor analysis\b|\bitems?\b/i.test(s.replace(/confirmatory factor analysis/g, '')))).toEqual([]);
  });

  it("says each limitation as the registry's limit with the same meaning says it", () => {
    const pairs: Array<[string, string]> = [
      ['ml_only', 'cfa_ml_only'],
      ['no_means', 'cfa_no_means'],
      ['one_group_only', 'cfa_one_group'],
      ['simple_structure_only', 'cfa_simple_structure'],
      ['marker_identification_only', 'cfa_marker_identification'],
      ['no_modification_indices', 'cfa_no_modification_indices'],
      ['normal_theory_standard_errors', 'cfa_normal_theory'],
      ['no_srmr', 'cfa_no_srmr'],
      ['listwise_only', 'cfa_listwise_only'],
    ];
    for (const [ours, cap] of pairs) {
      const theirs = (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${cap}`];
      expect(theirs, cap).toBeTruthy();
      expect(say(ours), ours).toBe(theirs);
    }
  });

  it("says decision 783's validation sentences word for word", () => {
    expect(say('no_factors')).toBe('The model has no factor. Choose a factor for a column.');
    expect(say('too_many_factors', { count: 17, limit: 16 })).toBe('The model has 17 factors; Gaply fits at most 16.');
    expect(say('factor_too_few_items', { index: 2, count: 1, limit: 2 })).toBe('Factor 2 has 1 column; each factor needs at least 2.');
    expect(say('factor_too_few_items', { index: 2, count: 0, limit: 2 })).toBe('Factor 2 has no columns; each factor needs at least 2.');
    expect(say('two_item_factor_uncorrelated', { index: 1 })).toBe(
      'Factor 1 has two columns and is not correlated with any other factor, so no data can identify it. Tick "The factors are correlated", or give it a third column.'
    );
    expect(say('items_out_of_order', { column: 3 })).toBe('These settings list "col3" out of the file\'s order, which this screen never does. Choose the factors again.');
    expect(say('factors_out_of_order', { index: 2 })).toBe(
      "These settings list factor 2 out of the order of the factors' first columns, which this screen never does. Choose the factors again."
    );
    // The factor analysis's sentence reads its limit from the code's param (factorWording.ts),
    // so it is taken, reworded for this method and its columns.
    expect(say('too_few_rows', { count: 4, limit: 7 })).toBe('4 rows have a value in every column in the model, and the confirmatory factor analysis needs at least 7.');
    expect(say('too_few_rows', { count: 1, limit: 4 })).toBe('1 row has a value in every column in the model, and the confirmatory factor analysis needs at least 4.');
  });

  it("says decision 783's refusals word for word, choosing by the params", () => {
    expect(say('sample_covariance_singular', { column: 7 })).toBe(
      '"col7" is, to within one part in a billion of its variance, a combination of the columns before it in the model, so their covariance matrix is singular and the model cannot be fitted. Take it out of the model.'
    );
    expect(say('overflow', { column: 2 })).toBe("\"col2\"'s values are too large to square, so its variance cannot be computed. Divide the column by a power of 10 and run again.");
    expect(say('overflow')).toBe(OVERFLOW_SENTENCE);
    expect(say('model_not_identified')).toBe(
      "The model's parameters cannot be told apart with these data: the information matrix was singular at the start of every attempt Gaply made. This happens when a factor's columns are uncorrelated with each other or with the other factors' columns."
    );
    expect(say('model_not_identified', { count: 3 })).toBe('The model has 3 more parameters than the data have variances and covariances, so it is not identified.');
    expect(say('cfa_not_converged', { count: 500, limit: 500 })).toBe(
      "Maximum likelihood did not converge from Gaply's starting points; the longest attempt stopped after 500 iterations, of at most 500. A model with fewer factors, or with columns that measure their factor more strongly, may converge."
    );
    expect(say('cfa_not_converged', { count: 1, limit: 500 })).toContain('stopped after 1 iteration, of at most 500.');
    expect(words.cfaRunEndedSentence('failed')).toBe('The confirmatory factor analysis could not be computed. The reason is below.');
  });

  it("says decision 783's notes, and its cells' not-computable sentences, word for word", () => {
    expect(say('negative_variance')).toBe(
      'At least one variance estimate is below 0, so this solution is not admissible, as AMOS says of such a solution. A standardized estimate, correlation or squared multiple correlation is not given where its formula takes the square root of a variance at or below 0. Where only an error variance is below 0 they are given, and a squared multiple correlation can then be above 1.'
    );
    expect(say('factor_covariance_not_positive_definite')).toBe(
      "The factors' estimated covariance matrix is not positive definite, so this solution is not admissible, as AMOS says of such a solution."
    );
    expect(say('df_zero')).toBe(
      'The model has 0 degrees of freedom: it has as many parameters as the data have variances and covariances, so it reproduces them exactly, and the measures that need degrees of freedom are not defined.'
    );
    const cell = (c: string) => words.cfaCellSentence(`stats.cfa.${c}`);
    expect(cell('fixed_parameter')).toBe("Fixed at 1, to set the factor's scale.");
    expect(cell('df_zero')).toBe('Not defined at 0 degrees of freedom.');
    expect(cell('cmin_zero')).toBe('Not defined when the chi-square is 0.');
    expect(cell('independence_cmin_zero')).toBe('Not defined: the independence model fits exactly, so there is nothing to compare with.');
    expect(cell('too_few_rows_for_bcc')).toBe('Not defined: this needs more rows than the number of columns plus 2.');
    expect(cell('negative_variance')).toBe('Not defined: a variance it takes the square root of is at or below 0.');
    expect([cell('too_small'), cell('overflow'), cell('distribution_out_of_range')]).toEqual([TOO_SMALL_SENTENCE, OVERFLOW_SENTENCE, DISTRIBUTION_OUT_OF_RANGE_SENTENCE]);
    expect(words.cfaDashLine('stats.cfa.df_zero')).toBe('–: Not defined at 0 degrees of freedom.');
  });

  it("says decision 783's settings words word for word", () => {
    expect(CFA_DIALOG_TITLE).toBe('Structural equations: Confirmatory factor analysis');
    expect(words.CF_RULE).toBe(
      "Each column measures one factor. Factors are numbered by their first column in the file, and each factor's first column has its loading fixed at 1 to set the factor's scale, as AMOS does when a model is drawn. A row is used only when every column in the model has a value."
    );
    expect([words.CF_COLUMNS_HEADING, words.CF_NOT_IN_MODEL, words.cfaFactorOption(2), words.CF_NEW_FACTOR]).toEqual(['Numeric columns', 'Not in the model', 'Factor 2', 'A new factor']);
    expect([words.CF_FACTORS_HEADING, words.cfaFactorHeading(1), words.CF_FIXED_MARK, words.CF_NO_FACTOR]).toEqual([
      "Factors, each in the file's order",
      'Factor 1',
      '(loading fixed at 1)',
      'No factor yet. Choose a factor for a column on the left.',
    ]);
    expect([words.CF_CORRELATED_LABEL, words.CF_CORRELATED_LINE]).toEqual([
      'The factors are correlated',
      'Ticked, every pair of factors has a covariance to estimate. Unticked, those covariances are fixed at 0.',
    ]);
    expect([words.CF_PUT_COLUMNS, words.cfaFactorTooFew(2, 1, 2), words.CF_AT_CAP]).toEqual([
      'Put at least 3 columns in factors.',
      'Factor 2 has 1 column; each factor needs at least 2.',
      'The model has 128 columns, the most Gaply fits; take one out to add another.',
    ]);
    expect(words.cfaMissingCodeQuestion(-99, [{ header: 'a', cells: 1 }, { header: 'b', cells: 2 }])).toBe(
      'Is -99 a code for a missing value? It appears in "a" (1 cell), "b" (2 cells). If you tick it, it is treated as missing in every column in the model, and a row with it is left out.'
    );
  });

  it("writes decision 783's settings line: each factor, Factors correlated with two or more, the codes", () => {
    expect(cfaSettingsLine({ indicators: [['C1', 'C2', 'C3']], correlatedFactors: true, missingCodes: [] }, VIEW)).toBe('Factor 1: C1 q1, C2 q2, C3 q3. Missing codes: none.');
    expect(cfaSettingsLine({ indicators: [['C1', 'C3'], ['C2', 'C4']], correlatedFactors: true, missingCodes: [-99, 7] }, VIEW)).toBe(
      'Factor 1: C1 q1, C3 q3. Factor 2: C2 q2, C4 q4. Factors correlated: yes. Missing codes: -99, 7.'
    );
    expect(cfaSettingsLine({ indicators: [['C1', 'C3'], ['C2', 'C4']], correlatedFactors: false, missingCodes: [] }, VIEW)).toBe(
      'Factor 1: C1 q1, C3 q3. Factor 2: C2 q2, C4 q4. Factors correlated: no. Missing codes: none.'
    );
  });

  it("says decision 783's card words word for word", () => {
    expect(Object.values(words.CF_HEADINGS)).toEqual([
      'Notes for the model',
      'Estimates',
      'Regression weights',
      'Standardized regression weights',
      'Covariances',
      'Correlations',
      'Variances',
      'Squared multiple correlations',
      'Model fit',
    ]);
    expect(words.CFA_FIT_TABLES.map((t) => t.heading)).toEqual(['CMIN', 'RMR, GFI', 'Baseline comparisons', 'Parsimony-adjusted measures', 'NCP', 'FMIN', 'RMSEA', 'AIC', 'ECVI', 'HOELTER']);
    expect(words.CFA_FIT_TABLES.map((t) => t.columns.map((c) => c.label))).toEqual([
      ['NPAR', 'CMIN', 'DF', 'P', 'CMIN/DF'],
      ['RMR', 'GFI', 'AGFI', 'PGFI'],
      ['NFI Delta1', 'RFI rho1', 'IFI Delta2', 'TLI rho2', 'CFI'],
      ['PRATIO', 'PNFI', 'PCFI'],
      ['NCP', 'LO 90', 'HI 90'],
      ['FMIN', 'F0', 'LO 90', 'HI 90'],
      ['RMSEA', 'LO 90', 'HI 90', 'PCLOSE'],
      ['AIC', 'BCC', 'BIC', 'CAIC'],
      ['ECVI', 'LO 90', 'HI 90', 'MECVI'],
      ['HOELTER .05', 'HOELTER .01'],
    ]);
    expect(words.CFA_FIT_TABLES.filter((t) => !t.saturated).map((t) => t.id)).toEqual(['rmsea', 'hoelter']);
    expect(words.CFA_FIT_TABLES.flatMap((t) => t.columns.filter((c) => c.whole).map((c) => c.key))).toEqual(['npar', 'df', 'hoelter05', 'hoelter01']);
    expect(Object.values(words.CF_ROWS)).toEqual(['Default model', 'Saturated model', 'Independence model']);
    expect([words.CF_LABELS.moments, words.CF_LABELS.parameters, words.CF_LABELS.df]).toEqual([
      'Distinct sample moments',
      'Distinct parameters to be estimated',
      'Degrees of freedom',
    ]);
    expect([words.CF_LABELS.estimate, words.CF_LABELS.se, words.CF_LABELS.cr, words.CF_LABELS.p]).toEqual(['Estimate', 'S.E.', 'C.R.', 'P']);
    expect([words.cfaWeightRow('visperc', 1), words.cfaCovarianceRow(1, 2), words.cfaFactorRow(2), words.cfaErrorRow('cubes')]).toEqual([
      'visperc ← Factor 1',
      'Factor 1 ↔ Factor 2',
      'Factor 2',
      'Error of cubes',
    ]);
    expect(words.CF_NO_COVARIANCES).toBe("The factors' covariances are fixed at 0.");
    expect(words.CF_THREE_MODELS).toBe(
      "The default model is the one you specified. The saturated model has a parameter for every variance and covariance, so it fits them exactly. The independence model has only the columns' variances, so it says the columns are uncorrelated."
    );
    expect([0, 1, 18].map(words.cfaIterationsSentence)).toEqual([
      "Maximum likelihood's starting values were already its minimum.",
      'Maximum likelihood reached its minimum in 1 iteration.',
      'Maximum likelihood reached its minimum in 18 iterations.',
    ]);
    expect(words.cfaChiSquareSentence({ value: 7.853 }, 8, { value: 0.448 })).toBe('Chi-square = 7.853, degrees of freedom = 8, probability level = 0.448.');
    expect(words.cfaChiSquareSentence({ value: 0 }, 0, { not_computable: 'stats.cfa.df_zero' })).toBe(
      'Chi-square = 0, degrees of freedom = 0. Probability level: Not defined at 0 degrees of freedom.'
    );
  });

  it('names the export files', () => {
    expect(words.CFA_EXPORT_CHOICES.map((c) => [c.format, c.file])).toEqual([
      ['csv', 'confirmatory_factor_analysis.csv'],
      ['manifestJson', 'confirmatory_factor_analysis_manifest.json'],
    ]);
  });

  it('shows a number through shownNumber, a whole number with every digit, -0 as 0, and a code as its sentence or "–"', () => {
    expect(words.cfaStatText({ value: 1.0000000000000002 })).toBe('1');
    expect(words.cfaStatText({ value: -0 })).toBe('0');
    expect(words.cfaStatText({ not_computable: 'stats.cfa.fixed_parameter' })).toBe("Fixed at 1, to set the factor's scale.");
    expect(words.cfaFitText({ value: 1550001 }, true)).toBe('1550001');
    expect(words.cfaFitText({ value: 1550001 }, false)).toBe('1550000');
    expect(words.cfaFitText({ value: -0 }, false)).toBe('0');
    expect(words.cfaFitText(21, true)).toBe('21');
    expect(words.cfaFitText({ not_computable: 'stats.cfa.df_zero' }, false)).toBe('–');
    expect(words.cfaWholeText(-0)).toBe('0');
    // The review of 9C-3, finding 7: every digit of a whole number of 1e21 and above.
    expect(words.cfaWholeText(1e21)).toBe('1000000000000000000000');
    expect(words.cfaFitText({ value: 2 ** 70 }, true)).toBe('1180591620717411303424');
  });

  it('reads its four limits from the wire fixture, never typed, and the validation fixture agrees', () => {
    expect(CFA_LIMITS).toEqual({ maxFactors: 16, maxItems: 128, minFactorItems: 2, minItems: 3 });
    const source = fs.readFileSync(path.join(__dirname, 'cfaWording.ts'), 'utf8');
    expect(source).toContain("from './fixtures/cfaFixtures'");
    for (const [name, key] of [
      ['MAX_ITEMS', 'maxItems'],
      ['MAX_FACTORS', 'maxFactors'],
      ['MIN_FACTOR_ITEMS', 'minFactorItems'],
      ['MIN_ITEMS', 'minItems'],
    ]) {
      expect(source).toMatch(new RegExp(`export const ${name}: number = cfaLimits\\.${key};`));
      expect(source).not.toMatch(new RegExp(`${name}[^\\n]*= *\\d`));
    }
    // The validation fixture holds one limit, too_few_rows' (the columns plus 1).
    const limited = [...validationJson.errors, ...validationJson.missingInformation].filter((e) => 'limit' in e.params).map((e) => e.code);
    expect(limited).toEqual(['stats.cfa.too_few_rows']);
  });
});

// The review of 96ce0cbe, m1 (Research OS, Phase 9C-4, decision 794): this method's export
// carries no label (its CSV names each column by position and alias, never by header), so its
// sentence is not the one-way ANOVA's, which says the tables carry the group labels.
describe('the export sentence', () => {
  it('says the tables name each column by position, and carry no label', () => {
    expect(words.CFA_EXPORT_TEXT).toBe(
      "Gaply writes only the file you name in the save dialog and keeps no copy. The result tables carry every digit and the manifest's hashes, and name each column by its position, never by its header. The manifest holds hashes, versions and counts, never your data, headers or file name."
    );
    expect(words.CFA_EXPORT_TEXT).not.toMatch(/label/i);
  });
});
