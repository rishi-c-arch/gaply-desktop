// The structural equation model's words (Research OS, Phase 9C-5, decisions 814 and 815). Every
// code the screen can show (semCodes.json, pinned to Rust's closed sets by
// src-tauri/tests/stats_sem_wire.rs) has a sentence: its own 73, and the run's, the registry's
// and the workspace's as the related rank tests give them, the two that name the method renamed.
// Decision 815's words are pinned word for word. No sentence or label says a word a verdict would
// use, carries a star, or gives a fit measure a cutoff or a band's name; the lists are
// cfaWording.vitest.ts's (alphaWording.vitest.ts's, PairedFlow.vitest.tsx's,
// projectWording.vitest.ts's, ttestWording.vitest.ts's scan, ranksWording.vitest.ts's and
// factorWording.vitest.ts's, with decision 783's own), copied (importing a test file would run its
// tests here), with decision 815's own.
import { describe, expect, it } from 'vitest';

import semCodes from './semCodes.json';
import LIMITS from './describeLimits.json';
import SEM_LIMITS from './fixtures/semLimits.json';
import { CAPABILITY_WORDS } from './capabilityWording';
import { RELATED_RANKS_WORDS } from './relatedRanksWording';
import { CFA_NOT_COMPUTABLE, cfaSentence } from './cfaWording';
import * as words from './semWording';
import { SEM_DIALOG_TITLE, semSettingsLine } from './window/windowWording';

/** alphaWording.vitest.ts:20's words. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;
/** PairedFlow.vitest.tsx:135's. */
const PAIRED_VERDICT = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;
/** projectWording.vitest.ts:47's. */
const PROJECT_VERDICT = /\b(significant|correct|incorrect|accurate|inaccurate|reliable|unreliable|trustworthy|wrong|invalid|suspicious|fraud)\b|★|✓/i;
/** factorWording.vitest.ts's own list (decision 730). */
const FACTOR_OWN = /\b(significance|valid|acceptable|reject|recommend|marvelous|meritorious|middling|mediocre|miserable|unacceptable)\b/i;
/** cfaWording.vitest.ts's own list (decision 783). */
const CFA_OWN = /\b(significant|significance|valid|should|good|poor|close|adequate|acceptable|reliable|reject|recommend|excellent|mediocre|fair)\b/i;
/** Decision 815's own list. */
const OWN = /\b(significant|significance|valid|should|good|poor|close|fair|adequate|acceptable|reliable|reject|recommend|drop|remove)\b/i;
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
/** With decision 815's and 783's own lists, which govern every NEW word: a sentence taken
 *  verbatim from the related rank tests (the run's, registry's and workspace's, pinned identical
 *  below) is not new. */
const VERDICT = (s: string) => VERDICT_SHARED(s) || OWN.test(s) || CFA_OWN.test(s) || CUTOFF(s);
const SHARED_CODES = semCodes.filter((c) => !c.startsWith('stats.sem.') && !['run.unknown_run', 'run.wrong_kind'].includes(c));

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2, lifecycle: 'planned' };
const WORKSPACE = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORKSPACE : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };
const say = (c: string, params: Record<string, unknown> = {}) => words.semSentence({ code: `stats.sem.${c}`, params }, NAMES);
const sharedSentences = () => new Set(SHARED_CODES.flatMap((code) => [0, 1, 2].map((count) => words.semSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES))));
const VIEW = { profile: { columns: [1, 2, 3, 4, 5, 6].map((i) => ({ alias: `C${i}`, header: `q${i}` })) } };

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
    if (name === 'SEM_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const count of [0, 1, 2]) {
    for (const code of semCodes) out.push(words.semSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES));
  }
  out.push(say('overflow'), say('model_not_identified'), say('equal_out_of_order', { index: 2 }));
  for (const code of semCodes) out.push(words.semCellSentence(code));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) out.push(words.semRunEndedSentence(state));
  for (const n of [0, 1, 2, 500]) out.push(words.semIterationsSentence(n));
  for (const a of [1, 2, 3]) out.push(words.semAttemptSentence(a) ?? '');
  out.push(words.semChiSquareSentence({ value: 6.383 }, 5, { value: 0.271 }), words.semChiSquareSentence({ value: 0 }, 0, { not_computable: 'stats.sem.df_zero' }));
  out.push(words.semStatText({ value: -0 }), words.semStatText({ not_computable: 'stats.sem.negative_variance' }));
  for (const [x, y] of [[true, true], [true, false], [false, true]] as const) out.push(words.semCovarianceLine('"a"', x, '"b"', y) ?? '');
  out.push(words.semGroupShort(2, 1), words.semGroupShort(2, 0), words.semAloneUnused('"q"'), words.semFactorTooFew(2, 1));
  out.push(words.semFactorOption(3), words.semFactorHeading(3), words.semGroupHeading(3), words.semErrorName('"q"'), words.semWeightRow('"a"', 'Factor 1'), words.semCovarianceRow('"a"', '"b"'));
  out.push(words.SM_MISSING.factorShort(2), words.SM_MISSING.alone('"q"'), words.SM_MISSING.pathEnds(1), words.SM_MISSING.pathItself(1), words.SM_MISSING.pathRepeats(2, 1));
  out.push(words.SM_MISSING.covarianceEnds(1), words.SM_MISSING.covarianceItself(1), words.SM_MISSING.covarianceRepeats(2, 1), words.SM_MISSING.groupShort(1), words.SM_CAPS.exogenous(276));
  // 9C-5b (decision 823's N4 and N7).
  out.push(words.SM_MISSING.tooManyColumns(129), words.SM_MISSING.tooManyFactors(17), words.semCovarianceEndLabel(1, 'first'), words.semCovarianceEndLabel(1, 'second'), words.semGroupKindLabel(2));
  // 9C-5f (decision 848's E3 and E5).
  out.push(words.SM_MISSING.tooManyPaths(65), words.SM_MISSING.tooManyCovariances(257), words.SM_MISSING.tooManyMembers(129), words.semPathEndLabel(1, 'from'), words.semPathEndLabel(1, 'to'));
  out.push(words.semMissingCodeQuestion(-99, [{ header: 'a', cells: 2 }]));
  out.push(
    SEM_DIALOG_TITLE,
    semSettingsLine(
      {
        indicators: [['C1', 'C2']],
        paths: [{ to: { factor: 1 }, from: { column: 'C3' } }],
        covariances: [{ a: { column: 'C1' }, b: { column: 'C3' } }],
        equal: [[{ variance: { variable: { column: 'C1' } } }, { variance: { variable: { column: 'C2' } } }]],
        missingCodes: [-99],
      },
      VIEW
    )
  );
  return out;
}

describe('the structural equation model screen words every code it can show', () => {
  it('has a sentence for each code in semCodes.json, and none for any other', () => {
    expect(semCodes).toHaveLength(73 + 11 + 5 + 6);
    expect(semCodes.filter((c) => c.startsWith('stats.sem.'))).toHaveLength(73);
    expect(Object.keys(words.SEM_WORDS).sort()).toEqual([...semCodes].sort());
    for (const code of semCodes) {
      for (const count of [0, 1, 2]) {
        const s = words.semSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES);
        expect(s, code).not.toMatch(/no wording/);
        expect(s, code).toMatch(/[.:]$/);
        expect(s, code).not.toContain('undefined');
        expect(s, code).not.toContain('?');
      }
    }
  });

  it('no sentence or label passes a verdict, carries a star, advises, or gives a fit measure a cutoff', () => {
    const all = everySentence().filter((s) => s !== '');
    expect(all.length).toBeGreaterThan(300);
    const SHARED = sharedSentences();
    expect(all.filter((s) => (SHARED.has(s) ? VERDICT_SHARED(s) : VERDICT(s)))).toEqual([]);
    expect(SHARED.size).toBeGreaterThan(10);
    for (const bad of ['A good fit.', 'The fit is close.', 'A fair model.', 'RMSEA below .05', 'CFI of 0.95 or more', 'This is valid.', 'Reject the model.', 'You should drop it.', 'Remove it.', 'significance']) {
      expect(VERDICT(bad), bad).toBe(true);
    }
    for (const ok of ['HOELTER .05', 'HOELTER .01', 'LO 90', 'HI 90', 'PCLOSE', 'structural equation model']) expect(VERDICT(ok), ok).toBe(false);
  });

  it("takes the run, registry and workspace sentences from the related rank tests' where they name no method", () => {
    const renamed = ['run.unknown_run', 'run.wrong_kind'];
    const shared = semCodes.filter((c) => !c.startsWith('stats.sem.') && !renamed.includes(c));
    expect(shared).toHaveLength(11 + 5 + 6 - 2);
    for (const code of shared) expect(words.SEM_WORDS[code], code).toBe(RELATED_RANKS_WORDS[code]);
    expect(words.semSentence({ code: 'run.unknown_run' })).toBe(
      `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the structural equation model again.`
    );
    expect(words.semSentence({ code: 'run.wrong_kind' })).toBe('This run is not a structural equation model, so this screen cannot read it.');
  });

  it("takes the CFA's sentence for each code the CFA also has, but the three reworded for this method and the two order sentences' endings", () => {
    const reworded = ['capability_version_mismatch', 'input_count', 'too_few_rows'];
    const shared = [
      'unknown_capability', 'input_mismatch', 'preparation_mismatch', 'missing_code_not_finite', 'missing_code_repeated', 'missing_codes_out_of_order',
      'too_many_factors', 'factor_too_few_items', 'item_repeated', 'unknown_column', 'column_non_finite',
      'column_holds_text', 'code_equals_score', 'no_complete_rows', 'suspect_missing_code', 'missing_value', 'missing_code', 'sample_covariance_singular', 'overflow', 'no_weights',
    ];
    for (const s of shared) {
      for (const count of [0, 1, 2]) {
        const d = { code: `stats.sem.${s}`, params: { ...NUMBERS, count }, count };
        expect(words.semSentence(d, NAMES), s).toBe(cfaSentence({ ...d, code: `stats.cfa.${s}` }, NAMES));
      }
    }
    for (const s of reworded) {
      const mine = say(s, { ...NUMBERS });
      expect(mine).toContain('structural equation model');
      expect(mine).not.toContain('confirmatory factor analysis');
    }
    // 9C-5b (decision 823's N8; the review of 365b638, finding 9): the CFA's two order sentences
    // end "Choose the factors again."; this method's end as its own never-written sentences do.
    for (const s of ['items_out_of_order', 'factors_out_of_order']) {
      const d = { code: `stats.sem.${s}`, params: { ...NUMBERS } };
      const mine = words.semSentence(d, NAMES);
      const cfa = cfaSentence({ ...d, code: `stats.cfa.${s}` }, NAMES);
      expect(mine, s).toMatch(/ Build the model again\.$/);
      expect(mine, s).not.toBe(cfa);
      expect(mine.replace(/ Build the model again\.$/, ''), s).toBe(cfa.replace(/ Choose the factors again\.$/, ''));
    }
    expect(say('df_zero')).toBe(cfaSentence({ code: 'stats.cfa.df_zero' }));
    for (const s of ['fixed_parameter', 'df_zero', 'cmin_zero', 'independence_cmin_zero', 'too_few_rows_for_bcc', 'negative_variance', 'too_small', 'overflow', 'distribution_out_of_range']) {
      expect(words.semCellSentence(`stats.sem.${s}`), s).toBe(CFA_NOT_COMPUTABLE[`stats.cfa.${s}`]);
    }
  });

  it("takes each limitation from the registry's sentence for its limit", () => {
    const limits: Array<[string, string]> = [
      ['ml_only', 'sem_ml_only'], ['no_means', 'sem_no_means'], ['one_group_only', 'sem_one_group'], ['indicators_measure_one_factor', 'sem_indicators_one_factor'],
      ['equal_one_kind', 'sem_equal_one_kind'], ['marker_identification_only', 'sem_marker_identification'], ['no_fixed_values', 'sem_no_fixed_values'],
      ['effects_structural_only', 'sem_effects_structural_only'], ['no_indirect_effect_tests', 'sem_no_indirect_effect_tests'], ['no_stability_index', 'sem_no_stability_index'],
      ['no_modification_indices', 'sem_no_modification_indices'], ['normal_theory_standard_errors', 'sem_normal_theory'], ['no_srmr', 'sem_no_srmr'], ['listwise_only', 'sem_listwise_only'],
    ];
    for (const [code, limit] of limits) expect(say(code), code).toBe((CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${limit}`]);
  });
});

describe("decision 815's words, word for word", () => {
  it('the validation sentences', () => {
    expect(say('empty_model')).toBe('The model has no factor, path or covariance. Add one.');
    expect(say('too_few_columns', { count: 1, limit: 2 })).toBe('The model has 1 column; it needs at least 2.');
    expect(say('too_few_columns', { count: 0, limit: 2 })).toBe('The model has 0 columns; it needs at least 2.');
    expect(say('too_many_columns', { count: 129, limit: 128 })).toBe('The model has 129 columns; Gaply fits at most 128.');
    expect(say('too_many_paths', { count: 65, limit: 64 })).toBe('The model has 65 paths; Gaply fits at most 64.');
    expect(say('too_many_covariances', { count: 257, limit: 256 })).toBe('The model has 257 covariances; Gaply fits at most 256.');
    expect(say('too_many_equal_members', { count: 129, limit: 128 })).toBe('The groups hold 129 parameters; Gaply holds at most 128 equal.');
    expect(say('model_not_identified', { count: 1 })).toBe(
      'The model is not identified: it needs 1 more constraint before any data can determine its estimates, as AMOS says of such a model. Take out a path or a covariance, give a factor more columns, or hold parameters equal.'
    );
    expect(say('model_not_identified', { count: 3 })).toContain('it needs 3 more constraints before');
    expect(say('column_constant', { column: 4 })).toBe('"col4" has the same value in every row used, so it has no variance. Take it out of the model.');
  });

  it("the never-written settings, each its own phrase from the code's params", () => {
    const frame = /^These settings .+, which this screen never does\. Build the model again\.$/;
    const cases: Array<[string, Record<string, number>, string]> = [
      ['unknown_factor', { index: 2 }, 'name a factor the model does not have'],
      ['path_to_itself', { index: 2 }, 'draw path 2 from a variable to itself'],
      ['indicator_in_path', { column: 3 }, 'put "col3", a column that measures a factor, at an end of a path'],
      ['paths_out_of_order', { index: 2 }, 'list path 2 out of order'],
      ['path_repeated', { index: 2 }, 'list path 2 twice'],
      ['covariance_with_itself', { index: 2 }, 'give covariance 2 the same variable at both ends'],
      ['covariances_out_of_order', { index: 2 }, 'list covariance 2 out of order'],
      ['covariance_repeated', { index: 2 }, 'list covariance 2 twice'],
      ['equal_group_too_small', { index: 2, count: 1, limit: 2 }, 'hold fewer than 2 parameters equal in group 2'],
      ['equal_mixed_kinds', { index: 2 }, 'hold parameters of different kinds equal in group 2'],
      ['equal_unknown_parameter', { index: 2, value: 3 }, 'hold equal, as parameter 3 of group 2, one the model does not have'],
      ['equal_fixed_parameter', { index: 2, value: 1 }, 'hold equal, as parameter 1 of group 2, a loading fixed at 1'],
      ['equal_out_of_order', { index: 2 }, 'list group 2 out of order'],
      ['equal_repeated', { index: 2, value: 3 }, 'hold parameter 3 of group 2 equal in a second place'],
    ];
    expect(cases).toHaveLength(14);
    for (const [code, params, what] of cases) {
      expect(say(code, params), code).toMatch(frame);
      expect(say(code, params), code).toBe(`These settings ${what}, which this screen never does. Build the model again.`);
    }
    expect(say('equal_out_of_order', { index: 2, value: 3 })).toBe('These settings list parameter 3 of group 2 out of order, which this screen never does. Build the model again.');
  });

  it("the run's refusals", () => {
    expect(say('solution_not_identified')).toBe(
      'Maximum likelihood stopped where the data barely determine at least one estimate, so its standard error would have no meaning, and Gaply gives no estimates. A factor whose columns barely correlate with each other is one way this happens.'
    );
    expect(say('sem_not_converged', { count: 500, limit: 500 })).toBe(
      "Maximum likelihood did not converge from Gaply's starting values by any method Gaply tried; the longest attempt stopped after 500 iterations, of at most 500. A smaller model, or columns that measure their factors more strongly, may converge."
    );
    expect(say('sem_not_converged', { count: 1, limit: 500 })).toContain('stopped after 1 iteration, of at most 500');
    expect(say('sample_covariance_singular', { column: 2 })).toBe(cfaSentence({ code: 'stats.cfa.sample_covariance_singular', params: { column: 2 } }, NAMES));
  });

  it('the notes', () => {
    expect(say('negative_variance')).toBe(
      'At least one variance estimate is below 0, so this solution is not admissible, as AMOS says of such a solution. A value whose formula takes the square root of a variance at or below 0 is not given, and a squared multiple correlation can be above 1 or below 0 where a variance is below 0.'
    );
    expect(say('covariance_not_positive_definite')).toBe(
      'The estimated variances and covariances of the factors and of the variables in your covariances, each endogenous one by its error term, do not form a positive definite matrix, so this solution is not admissible, as AMOS says of such a solution.'
    );
    expect(say('nonrecursive')).toBe(
      "The paths make a loop, so the model is nonrecursive. Gaply computes its effects as AMOS does, from the inverse of the loop's matrix, but not AMOS's stability index, which tells whether the loop's effects can be read as values it settles at."
    );
  });

  it('the settings', () => {
    expect(words.SM_RULE).toBe(
      "A column measures one factor, or stands on its own. Factors are numbered by their first column in the file, and each factor's first column has its loading fixed at 1 to set the factor's scale, as AMOS does when a model is drawn. A variable with a path into it has an error term; every other variable has a variance. Two variables are uncorrelated unless you add their covariance. A row is used only when every column in the model has a value."
    );
    expect([words.SM_COLUMNS_HEADING, words.SM_NOT_IN_MODEL, words.SM_ON_ITS_OWN, words.semFactorOption(2), words.SM_NEW_FACTOR]).toEqual([
      'Numeric columns', 'Not in the model', 'On its own', 'Factor 2', 'A new factor',
    ]);
    expect([words.SM_FACTORS_HEADING, words.SM_PATHS_HEADING, words.SM_COVARIANCES_HEADING, words.SM_EQUAL_HEADING, words.SM_FIXED_MARK]).toEqual([
      "Factors, each in the file's order", 'Paths', 'Covariances', 'Parameters held equal', '(loading fixed at 1)',
    ]);
    expect([words.semGroupHeading(2), words.SM_FROM, words.SM_TO, words.SM_CHOOSE]).toEqual(['Group 2', 'From', 'To', 'Choose a variable']);
    expect(Object.values(words.SM_KINDS)).toEqual(['Loadings', 'Paths', 'Covariances', 'Variances']);
    expect([words.SM_ADD_PATH, words.SM_ADD_COVARIANCE, words.SM_ADD_EXOGENOUS, words.SM_ADD_GROUP]).toEqual([
      'Add a path', 'Add a covariance', 'Add every pair of exogenous variables', 'Add a group',
    ]);
    expect([words.SM_NO_FACTOR, words.SM_NO_PATH, words.SM_NO_COVARIANCE, words.SM_NO_GROUP]).toEqual([
      'No factor yet. Choose a factor for a column on the left.',
      'No path yet. Add one to give one variable a regression weight on another.',
      'No covariance yet. Add one to let two variables, or their error terms, covary.',
      'No parameters are held equal. Add a group to hold two or more equal.',
    ]);
    expect(words.semCovarianceLine('a', true, 'b', true)).toBe('Between the error terms of a and b.');
    expect(words.semCovarianceLine('a', true, 'b', false)).toBe('Between the error term of a and b.');
    expect(words.semCovarianceLine('a', false, 'b', true)).toBe('Between a and the error term of b.');
    expect(words.semCovarianceLine('a', false, 'b', false)).toBeNull();
    expect([words.SM_PATH_TO_ITSELF, words.SM_PATH_REPEATED, words.SM_COVARIANCE_WITH_ITSELF, words.SM_COVARIANCE_REPEATED]).toEqual([
      'A path needs two different variables. Change one of its ends.',
      'This path is already listed. Change one of its ends.',
      'A covariance needs two different variables. Change one of its ends.',
      'This covariance is already listed. Change one of its ends.',
    ]);
    expect(words.semGroupShort(3, 1)).toBe('Group 3 holds 1 parameter. Tick at least one more.');
    expect(words.semGroupShort(3, 0)).toBe('Group 3 holds no parameters. Tick at least 2.');
    expect(words.semAloneUnused('q')).toBe('q is on its own with no path or covariance, so it would not be fitted. Add one, or choose "Not in the model" for it.');
    const m = words.SM_MISSING;
    expect([m.empty, m.columns, m.factorShort(2), m.alone('q'), m.pathEnds(1), m.pathItself(1), m.pathRepeats(2, 1), m.covarianceEnds(1), m.covarianceItself(1), m.covarianceRepeats(2, 1), m.groupShort(1)]).toEqual([
      'Add a factor, a path or a covariance.',
      `Put at least ${SEM_LIMITS.minColumns} columns in the model.`,
      `Factor 2 has 1 column; each factor needs at least ${SEM_LIMITS.minFactorItems}.`,
      'q is on its own with no path or covariance. Add one, or choose "Not in the model" for it.',
      'Choose both ends of path 1.',
      'Path 1 needs two different variables.',
      'Path 2 repeats path 1. Change one of its ends.',
      'Choose both ends of covariance 1.',
      'Covariance 1 needs two different variables.',
      'Covariance 2 repeats covariance 1. Change one of its ends.',
      'Group 1 holds fewer than 2 parameters. Tick at least 2.',
    ]);
    const c = words.SM_CAPS;
    expect([c.columns, c.paths, c.covariances, c.exogenous(276), c.equal]).toEqual([
      `The model has ${SEM_LIMITS.maxColumns} columns, the most Gaply fits; take one out to add another.`,
      `The model has ${SEM_LIMITS.maxPaths} paths, the most Gaply fits.`,
      `The model has ${SEM_LIMITS.maxCovariances} covariances, the most Gaply fits.`,
      `Adding every pair would make 276 covariances, more than the ${SEM_LIMITS.maxCovariances} Gaply fits. Add the pairs you need one by one.`,
      `The groups hold ${SEM_LIMITS.maxEqualMembers} parameters, the most Gaply holds equal; untick one to tick another.`,
    ]);
  });

  it('the card', () => {
    const h = words.SM_HEADINGS;
    expect([h.effects, h.total, h.totalStandardized, h.direct, h.directStandardized, h.indirect, h.indirectStandardized]).toEqual([
      'Effects', 'Total effects', 'Standardized total effects', 'Direct effects', 'Standardized direct effects', 'Indirect effects', 'Standardized indirect effects',
    ]);
    expect(words.semAttemptSentence(1)).toBeNull();
    expect(words.semAttemptSentence(2)).toBe("Fisher scoring did not converge from Gaply's starting values; Newton's method, from the same values, did.");
    expect(words.semAttemptSentence(3)).toBe("Neither Fisher scoring nor Newton's method converged from Gaply's starting values; Fisher scoring from other starting values for the factors did.");
    expect(words.SM_NO_COVARIANCES).toBe('The model has no covariances, so every pair of exogenous variables is uncorrelated.');
    expect(words.SM_BLANK_CELL).toBe("A blank cell: its row and its column are the same variable, or no path of arrows leads from the column's variable to the row's.");
    expect(words.SM_NO_EFFECTS).toBe('The model has no paths, so it has no effects.');
    expect([words.semWeightRow('a', 'b'), words.semCovarianceRow('a', 'b'), words.semErrorName('a')]).toEqual(['a ← b', 'a ↔ b', 'Error of a']);
    expect(words.SEM_EXPORT_CHOICES.map((c) => [c.format, c.file])).toEqual([
      ['csv', 'structural_equation_model.csv'],
      ['manifestJson', 'structural_equation_model_manifest.json'],
    ]);
    expect(words.semStatText({ value: -0 })).toBe('0');
  });

  it("the dialog's title and its settings line, each part only when it has something to say", () => {
    expect(SEM_DIALOG_TITLE).toBe('Structural equations: Structural equation model');
    expect(semSettingsLine({ indicators: [], paths: [{ to: { column: 'C4' }, from: { column: 'C1' } }], covariances: [], equal: [], missingCodes: [] }, VIEW)).toBe(
      'On their own: C1 q1, C4 q4. Paths: C1 q1 → C4 q4. Missing codes: none.'
    );
    expect(
      semSettingsLine(
        {
          indicators: [['C1', 'C2'], ['C3', 'C4']],
          paths: [{ to: { factor: 2 }, from: { factor: 1 } }],
          covariances: [{ a: { column: 'C1' }, b: { column: 'C5' } }],
          equal: [[{ loading: { factor: 1, column: 'C2' } }, { loading: { factor: 2, column: 'C4' } }], [{ variance: { variable: { column: 'C1' } } }, { variance: { variable: { column: 'C5' } } }]],
          missingCodes: [-99, 99],
        },
        VIEW
      )
    ).toBe(
      'Factor 1: C1 q1, C2 q2. Factor 2: C3 q3, C4 q4. On their own: C5 q5. Paths: Factor 1 → Factor 2. Covariances: C1 q1 ↔ C5 q5. Held equal: group 1 (C2 q2 ← Factor 1, C4 q4 ← Factor 2); group 2 (Error of C1 q1, C5 q5). Missing codes: -99, 99.'
    );
  });
});
