// The factor analysis's words (Research OS, Phase 9B-3, decisions 729 and 730). Every code the
// screen can show (factorCodes.json, pinned to Rust's closed sets by
// src-tauri/tests/stats_factor_wire.rs) has a sentence: its own 50, and the run's, the
// registry's and the workspace's as the related rank tests give them, the two that name the
// method renamed. No sentence says "significant", carries a star, advises a test, gives a KMO a
// band's name, or uses a word a verdict would use. "Remove all" and "Remove" are labels, not
// verdicts, and fail the list: the settings import them from windowWording, and this module
// never holds them.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import factorCodes from './factorCodes.json';
import LIMITS from './describeLimits.json';
import FACTOR_LIMITS from './fixtures/factorLimits.json';
import validationJson from './fixtures/factorValidationView.json';
import { CAPABILITY_WORDS } from './capabilityWording';
import { ALPHA_WORDS } from './alphaWording';
import { RANKS_WORDS } from './ranksWording';
import { RELATED_RANKS_WORDS } from './relatedRanksWording';
import { LINEAR_REGRESSION_WORDS } from './linearRegressionWording';
import * as words from './factorWording';

/** alphaWording.vitest.ts:20's words, copied: importing a test file would run its tests here. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;
/** PairedFlow.vitest.tsx:135's. */
const PAIRED_VERDICT = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;
/** projectWording.vitest.ts:47's. */
const PROJECT_VERDICT = /\b(significant|correct|incorrect|accurate|inaccurate|reliable|unreliable|trustworthy|wrong|invalid|suspicious|fraud)\b|★|✓/i;
/** Decision 730's own list, and the bands a KMO is sometimes given. */
const OWN = /\b(significance|valid|acceptable|reject|recommend|marvelous|meritorious|middling|mediocre|miserable|unacceptable)\b/i;
const VERDICT = (s: string) =>
  /significan/i.test(s) ||
  /[*★☆]/.test(s) ||
  /\b(should|recommend)/i.test(s) ||
  ALPHA_VERDICT.test(s) ||
  PAIRED_VERDICT.test(s) ||
  PROJECT_VERDICT.test(s) ||
  OWN.test(s) ||
  /\b(valid|accurate)\b/i.test(s);

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2, lifecycle: 'planned' };
const WORKSPACE = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORKSPACE : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };
const say = (c: string, params: Record<string, unknown> = {}) => words.factorSentence({ code: `stats.factor.${c}`, params }, NAMES);

function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'FACTOR_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const count of [1, 2]) {
    for (const code of factorCodes) out.push(words.factorSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES));
  }
  out.push(say('correlations_singular'));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.factorRunEndedSentence(state));
  }
  for (const e of ['principal_components', 'principal_axis'] as const) {
    out.push(words.factorWord(e), words.FA_HEADINGS.matrix(e), words.FA_HEADINGS.rotated(e), words.FA_HEADINGS.transformation(e), words.FA_HEADINGS.correlations(e));
    for (const m of [1, 2]) for (const n of [1, 2]) out.push(words.factorExtractedFootnote(e, m, n));
  }
  for (const n of [1, 2]) out.push(words.factorRotationFootnote('varimax', n), words.factorRotationFootnote('promax', n));
  out.push(words.factorMissingCodeQuestion(-99, [{ header: 'a', cells: 2 }]));
  out.push(words.factorTooManyItems(129), words.factorFactorsRange(4));
  return out;
}

describe('the factor analysis screen words every code it can show', () => {
  it('has a sentence for each code in factorCodes.json, and none for any other', () => {
    expect(factorCodes).toHaveLength(50 + 11 + 5 + 6);
    expect(factorCodes.filter((c) => c.startsWith('stats.factor.'))).toHaveLength(50);
    expect(Object.keys(words.FACTOR_WORDS).sort()).toEqual([...factorCodes].sort());
    for (const code of factorCodes) {
      for (const count of [1, 2]) {
        const s = words.factorSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES);
        expect(s, code).not.toMatch(/no wording/);
        expect(s, code).toMatch(/[.:]$/);
        expect(s, code).not.toContain('undefined');
        expect(s, code).not.toContain('?');
      }
    }
  });

  it('no sentence passes a verdict, carries a star, advises a test or names a band', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(150);
    expect(all.filter(VERDICT)).toEqual([]);
    for (const bad of ['The KMO is meritorious.', 'Adequate sampling.', 'You should drop it.', 'A good fit.', 'This is valid.', 'Reject the null.', 'p < 0.05 *', 'Remove all', 'significance']) {
      expect(VERDICT(bad), bad).toBe(true);
    }
  });

  it('does not hold the labels the verdict list would refuse, which the settings take from windowWording', () => {
    const text = JSON.stringify(Object.values(words).filter((v) => typeof v !== 'function'));
    expect(text).not.toContain('Remove');
  });

  it("takes the run, registry and workspace sentences from the related rank tests' where they name no method", () => {
    const renamed = ['run.unknown_run', 'run.wrong_kind'];
    const shared = factorCodes.filter((c) => !c.startsWith('stats.factor.') && !renamed.includes(c));
    expect(shared).toHaveLength(11 + 5 + 6 - 2);
    for (const code of shared) expect(words.FACTOR_WORDS[code], code).toBe(RELATED_RANKS_WORDS[code]);
    expect(words.factorSentence({ code: 'run.unknown_run' })).toBe(
      `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the factor analysis again.`
    );
    expect(words.factorSentence({ code: 'run.wrong_kind' })).toBe('This run is not a factor analysis, so this screen cannot read it.');
  });

  it("says the codes it shares as the method that has them says them", () => {
    const params = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2 };
    const d = (prefix: string, suffix: string) => ({ code: `${prefix}${suffix}`, params, count: 3 });
    for (const suffix of ['unknown_capability', 'input_mismatch', 'preparation_mismatch', 'missing_code_not_finite', 'unknown_column', 'code_equals_score', 'no_weights']) {
      const theirs = (RANKS_WORDS as Record<string, (d: unknown, n: unknown) => string>)[`stats.ranks.${suffix}`](d('stats.ranks.', suffix), { ...NAMES, level: () => '' });
      expect(words.factorSentence(d('stats.factor.', suffix), NAMES), suffix).toBe(theirs);
    }
    for (const suffix of ['missing_code_repeated', 'missing_codes_out_of_order', 'items_out_of_order', 'missing_value', 'missing_code']) {
      expect(words.factorSentence(d('stats.factor.', suffix), NAMES), suffix).toBe(ALPHA_WORDS[`stats.alpha.${suffix}`](d('stats.alpha.', suffix), NAMES));
    }
    for (const suffix of ['suspect_missing_code', 'overflow', 'too_small']) {
      expect(words.factorSentence(d('stats.factor.', suffix), NAMES), suffix).toBe(
        LINEAR_REGRESSION_WORDS[`stats.linear_regression.${suffix}`](d('stats.linear_regression.', suffix), NAMES)
      );
    }
  });

  it("says each limitation as the registry's limit with the same meaning says it", () => {
    const pairs: Array<[string, string]> = [
      ['no_ml_or_other_extraction', 'factor_extraction_two_methods'],
      ['no_oblique_other_than_promax', 'factor_rotation_varimax_promax'],
      ['promax_kappa_four', 'factor_promax_kappa_four'],
      ['no_factor_scores', 'factor_no_scores'],
      ['no_matrices_shown', 'factor_matrices_not_shown'],
      ['pearson_only', 'factor_pearson_only'],
      ['listwise_only', 'factor_listwise_only'],
      ['no_scree_plot', 'factor_no_scree_plot'],
    ];
    for (const [ours, cap] of pairs) {
      const theirs = (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${cap}`];
      expect(theirs, cap).toBeTruthy();
      expect(say(ours), ours).toBe(theirs);
    }
  });

  it("says decision 730's not-computable sentences, notes and refusals word for word", () => {
    expect(say('distribution_out_of_range')).toBe('Not computable: this needs a chi-square distribution beyond the range where Gaply has measured its accuracy.');
    expect(say('correlations_singular')).toBe(
      'Not computable: the correlation matrix is singular, as when an item is a combination of other items or there are no more rows than items.'
    );
    expect(say('no_correlation')).toBe('Not computable: the items this measure covers correlate with no other item, so there is nothing for it to compare.');
    expect(say('communality_above_one')).toBe(
      "During principal axis factoring a communality went above 1 (a Heywood case). A communality is a share of an item's variance, so these loadings do not describe a possible factor model as they stand. SPSS gives the same warning."
    );
    expect(say('extraction_eigenvalue_negative')).toBe(
      'At some iteration an eigenvalue among those extracted was below 0; as SPSS does, its absolute value was used for the loadings.'
    );
    expect(say('one_factor_not_rotated')).toBe('One factor was extracted, so there is nothing to rotate.');
    expect(say('rotation_not_converged')).toBe('The rotation did not converge within the maximum iterations, so no rotated solution is given.');
    expect(say('rotation_singular')).toBe("Promax's fit cannot be computed for these loadings, so no rotated solution is given.");
    expect(say('eigenvalues_not_distinct')).toBe(
      'The last eigenvalue extracted and the next are equal within 1e-8 of the larger, so these loadings are one of many that fit equally well.'
    );
    expect(say('correlation_out_of_range', { column: 4 })).toBe('"col4"\'s values are too large or too small for its correlations to be computed at this scale.');
    expect(say('no_eigenvalue_over_one')).toBe(
      'No eigenvalue of the correlation matrix is above 1, so that rule extracts no factor. Choose a fixed number of factors instead.'
    );
    expect(say('correlations_singular', { column: 5 })).toBe(
      '"col5" is a combination of the items before it, or within one part in a billion of its variance of being one, so the correlation matrix is singular and principal axis factoring cannot start. Take it out of the items, or use principal components.'
    );
    expect(say('extraction_eigenvalue_not_positive')).toBe('A component asked for has an eigenvalue of 0 or below, so it has no loadings. Ask for fewer components.');
    expect(say('extraction_not_converged', { limit: 25 })).toBe(
      'Principal axis factoring did not converge within 25 iterations. Raise the maximum iterations, or ask for fewer factors.'
    );
    expect(say('eigen_not_converged')).toBe("The eigenvalues of these data could not be found to the arithmetic's precision.");
  });

  it("says decision 730's settings and card words word for word", () => {
    expect(words.FA_RULE).toBe("Every chosen item enters the analysis at once, in the file's order. A row is used only when every item has a value.");
    expect([words.FA_CHOOSE_ITEMS, words.factorTooManyItems(129), words.factorFactorsRange(4), words.FA_ITERATIONS_RANGE, words.FA_CONVERGENCE_RANGE]).toEqual([
      'Choose at least 3 items.',
      'Choose 128 items or fewer; 129 are chosen.',
      'Enter a whole number of factors from 1 to 4.',
      'Enter a whole number of iterations from 1 to 1000.',
      'Enter a convergence from 1e-12 to 0.1.',
    ]);
    expect(words.factorMissingCodeQuestion(-99, [{ header: 'a', cells: 1 }, { header: 'b', cells: 2 }])).toBe(
      'Is -99 a code for a missing value? It appears in "a" (1 cell), "b" (2 cells). If you tick it, it is treated as missing in every item, and a row with it is left out.'
    );
    expect([words.FA_HEADINGS.descriptives, words.FA_HEADINGS.kmo, words.FA_HEADINGS.msa, words.FA_HEADINGS.communalities, words.FA_HEADINGS.variance]).toEqual([
      'Descriptive statistics',
      "KMO and Bartlett's test",
      "Each item's sampling adequacy",
      'Communalities',
      'Total variance explained',
    ]);
    expect(['principal_components', 'principal_axis'].map((e) => words.FA_HEADINGS.matrix(e as 'principal_axis'))).toEqual(['Component matrix', 'Factor matrix']);
    expect(words.FA_HEADINGS.rotated('principal_axis')).toBe('Rotated factor matrix');
    expect(words.FA_HEADINGS.transformation('principal_components')).toBe('Component transformation matrix');
    expect(words.FA_HEADINGS.correlations('principal_axis')).toBe('Factor correlation matrix');
    expect([words.factorExtractedFootnote('principal_components', 1, 0), words.factorExtractedFootnote('principal_components', 3, 0)]).toEqual([
      '1 component extracted.',
      '3 components extracted.',
    ]);
    expect([words.factorExtractedFootnote('principal_axis', 1, 1), words.factorExtractedFootnote('principal_axis', 2, 11)]).toEqual([
      '1 factor extracted, in 1 iteration.',
      '2 factors extracted, in 11 iterations.',
    ]);
    expect(words.factorRotationFootnote('varimax', 3)).toBe('Varimax with Kaiser normalization; the rotation converged in 3 cycles.');
    expect(words.factorRotationFootnote('promax', 1)).toBe('Promax (power 4) with Kaiser normalization; its varimax converged in 1 cycle.');
    expect(words.FA_PROMAX_SUMS_SENTENCE).toBe(
      "Promax's factors are correlated, so the rotation sums of squared loadings that SPSS lists for them are not given here."
    );
  });

  it('names the export files', () => {
    expect(words.FACTOR_EXPORT_CHOICES.map((c) => [c.format, c.file])).toEqual([
      ['csv', 'factor_analysis.csv'],
      ['manifestJson', 'factor_analysis_manifest.json'],
    ]);
  });

  it('shows a number through shownNumber, a diagonal of 1.0000000000000002 as 1 and 99.99999999999999 as 100, and a code as its sentence', () => {
    expect(words.factorStatText({ value: 1.0000000000000002 })).toBe('1');
    expect(words.factorStatText({ value: 99.99999999999999 })).toBe('100');
    expect(words.factorStatText({ not_computable: 'stats.factor.no_correlation' })).toBe(say('no_correlation'));
  });

  it('reads its item limit from the wire fixture, and its other limits match the validation fixture', () => {
    expect(FACTOR_LIMITS).toEqual({ maxItems: 128 });
    expect(words.MAX_ITEMS).toBe(FACTOR_LIMITS.maxItems);
    const source = fs.readFileSync(path.join(__dirname, 'factorWording.ts'), 'utf8');
    expect(source).toContain("from './fixtures/factorLimits.json'");
    expect(source).toMatch(/MAX_ITEMS: number = FACTOR_LIMITS\.maxItems/);
    expect(source).not.toMatch(/MAX_ITEMS[^\n]*= *128/);
    // The validation fixture holds one limit, too_few_rows' (rows, not items), and no item,
    // iteration or convergence limit, so the named constants are pinned by value against their
    // Rust twins' (stats_factor/mod.rs:84, :89, :91, :93).
    const limited = [...validationJson.errors, ...validationJson.missingInformation].filter((e) => 'limit' in e.params).map((e) => e.code);
    expect(limited).toEqual(['stats.factor.too_few_rows']);
    expect([words.MIN_ITEMS, words.MIN_ITERATIONS, words.MAX_ITERATIONS, words.MIN_CONVERGENCE, words.MAX_CONVERGENCE]).toEqual([3, 1, 1000, 1e-12, 0.1]);
  });
});

// Research OS, Phase 9C-1, decision 742's m3 and m4: a note names what was extracted, and
// principal axis's tie note says it read the last iteration's eigenvalues, not the ones shown.
describe('the words follow the extraction (decision 742, m3 and m4)', () => {
  const sayIn = (c: string, e?: 'principal_components' | 'principal_axis') => words.factorSentence({ code: `stats.factor.${c}` }, NAMES, e);

  it('under principal components a note and the promax sentence say component', () => {
    expect(sayIn('one_factor_not_rotated', 'principal_components')).toBe('One component was extracted, so there is nothing to rotate.');
    expect(words.factorPromaxSumsSentence('principal_components')).toBe(
      "Promax's components are correlated, so the rotation sums of squared loadings that SPSS lists for them are not given here."
    );
    expect(sayIn('eigenvalues_not_distinct', 'principal_components')).toBe(
      'The last eigenvalue extracted and the next are equal within 1e-8 of the larger, so these loadings are one of many that fit equally well.'
    );
  });

  it("under principal axis decision 730's words stand, and the tie note names the last iteration", () => {
    expect(sayIn('one_factor_not_rotated', 'principal_axis')).toBe('One factor was extracted, so there is nothing to rotate.');
    expect(words.factorPromaxSumsSentence('principal_axis')).toBe(
      "Promax's factors are correlated, so the rotation sums of squared loadings that SPSS lists for them are not given here."
    );
    expect(sayIn('eigenvalues_not_distinct', 'principal_axis')).toBe(
      'In the last iteration, the last eigenvalue extracted and the next were equal within 1e-8 of the larger, so these loadings are one of many that fit equally well.'
    );
  });

  it('with no extraction given, every code keeps its sentence', () => {
    for (const code of factorCodes) {
      const s = words.factorSentence({ code, params: paramsFor(code) }, NAMES);
      expect(words.factorSentence({ code, params: paramsFor(code) }, NAMES, undefined), code).toBe(s);
    }
    expect(sayIn('one_factor_not_rotated')).toBe('One factor was extracted, so there is nothing to rotate.');
  });

  it('no sentence of either extraction passes a verdict', () => {
    const all: string[] = [];
    for (const e of ['principal_components', 'principal_axis'] as const) {
      for (const code of factorCodes) all.push(words.factorSentence({ code, params: paramsFor(code) }, NAMES, e));
      all.push(words.factorPromaxSumsSentence(e));
    }
    expect(all.filter(VERDICT)).toEqual([]);
  });
});
