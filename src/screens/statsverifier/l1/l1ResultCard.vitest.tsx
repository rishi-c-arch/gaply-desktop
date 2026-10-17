// The Level 1 result card's wording (Phase 2 evaluation fixes, part A,
// sections 5, 6 and 7): what "Reproduced" says, where the significance
// threshold came from, and a number matched only by truncation.
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { L1ResultCard } from './L1ResultCard';
import type { StatsL1Bridge } from '../statsL1Bridge';
import type { L1ClaimReproductionView, L1ClaimView, L1NumberView, ReproductionView, WireCode } from '../statsL1Types';

afterEach(cleanup);

const bridge = {} as StatsL1Bridge;

const claim = (level0: WireCode | null): L1ClaimView => ({
  claimId: 0, revision: 'rev0', sectionIndex: 2, section: 'results', paragraph: 1, sentenceIndex: 0,
  sentence: 'Scores rose, t(38) = 2.10, p = .042.', family: 't', statistic: '2.10', df: ['38'], n: null, p: '= .042', level0,
});

const STAT = { kind: { code: 'statistic' }, printed: '2.10', relation: '=', computed: 2.1034, acceptedLo: 2.095, acceptedHi: 2.105, status: { code: 'matched' }, precisionLo: null, precisionHi: null };
const DF = { kind: { code: 'df', index: 0 }, printed: '38', relation: '=', computed: 38, acceptedLo: 38, acceptedHi: 38, status: { code: 'matched' }, precisionLo: null, precisionHi: null };
const P = { kind: { code: 'p' }, printed: '.042', relation: '=', computed: 0.04221, acceptedLo: 0.0415, acceptedHi: 0.0425, status: { code: 'matched' }, precisionLo: null, precisionHi: null };

function result(verdict: L1ClaimReproductionView['verdict'], numbers: L1NumberView[] = [STAT, DF, P]): L1ClaimReproductionView {
  return {
    claimId: 0, sectionIndex: 2, paragraph: 1, sentence: 0, method: 'independent_t',
    variant: { code: 'not_stated' }, variantSetBy: { code: 'not_stated' }, verdict, notes: [],
    candidates: [{ variant: { code: 'student_pooled' }, storedDecimals: null, numbers }],
    nUsed: 40, rowsRead: 40, exclusions: [], provenance: null, wholeNumbers: [],
  };
}

function card(r: L1ClaimReproductionView, alpha: ReproductionView['alpha'] = { printed: ['.05'], stated: false }, level0: WireCode | null = { code: 'consistent' }) {
  render(
    <L1ResultCard bridge={bridge} run={7} claim={claim(level0)} result={r} alpha={alpha} ctx={{ column: (a) => `C${a}` }} catalogue={[]} aiHelp={false} webContext={false} onUsage={() => undefined} />
  );
  return {
    label: screen.getByTestId('l1-result-0-verdict'),
    text: screen.getByTestId('l1-result-0').textContent ?? '',
  };
}

const VARIANT = { code: 'variant_matched', variant: 'student_pooled' };
const notChecked = (number: WireCode, why: WireCode) => ({ code: 'not_checked', number: number.code === 'df' ? number : number.code, why });

describe('what "Reproduced" says (section 5)', () => {
  it('only a claim whose every printed number matched gets the plain label', () => {
    const { label } = card(result({ kind: 'reproduced', notes: [VARIANT] }));
    expect(label.textContent).toBe('Reproduced');
    expect(label.className).toContain('gds-badge--certain');
  });

  // Every engine path that can leave a printed number not compared under a
  // Reproduced verdict (stats_l1::reproduce): seven reasons on the p, and the
  // data-precision reason on a df.
  const paths: Array<[string, WireCode, WireCode, RegExp]> = [
    ['Level 0 raised a concern on the p', { code: 'p' }, { code: 'p_concern_at_level_0', reason: 'one_tailed_unstated' }, /^Reproduced, except the p-value, which was not decided$/],
    ['Level 0 could not check the p', { code: 'p' }, { code: 'p_could_not_check_at_level_0', reason: 'adjusted_p' }, /^Reproduced, except the p-value, which was not decided$/],
    ['the exact p agrees only one-tailed', { code: 'p' }, { code: 'one_tailed_unstated_on_exact_p' }, /^Reproduced, except the p-value, which was not decided$/],
    ["Phase 1's comparison of the exact p raised a concern", { code: 'p' }, { code: 'p_concern', reason: 'f_two_tailed' }, /^Reproduced, except the p-value, which was not decided$/],
    ["Phase 1's comparison of the exact p could not be made", { code: 'p' }, { code: 'p_could_not_check', reason: 'adjusted_p' }, /^Reproduced, except the p-value, which was not decided$/],
    ["Spearman's p may be the exact p", { code: 'p' }, { code: 'spearman_p_may_be_exact' }, /^Reproduced, except the p-value, which was not decided$/],
    ["the p is within the file's rounding", { code: 'p' }, { code: 'data_precision', decimals: 2 }, /^Reproduced, except the p-value, which was not decided$/],
    ["Welch's df is within the file's rounding", { code: 'df', index: 0 }, { code: 'data_precision', decimals: 1 }, /^Reproduced, except degrees of freedom 1, which was not decided$/],
  ];
  for (const [name, number, why, expected] of paths) {
    it(`a number not compared because ${name} is said so beside "Reproduced"`, () => {
      const nc = notChecked(number, why);
      const { label } = card(result({ kind: 'reproduced', notes: [VARIANT, nc] }));
      expect(label.textContent).toMatch(expected);
      expect(label.className).not.toContain('gds-badge--certain');
      // The reason, in one sentence from the wording table.
      const reason = screen.getByTestId('l1-result-0-not-compared');
      expect(reason.textContent).toMatch(/\.$/);
      expect(reason.textContent?.length).toBeGreaterThan(20);
    });
  }

  it('two numbers not compared are both named', () => {
    const { label } = card(
      result({ kind: 'reproduced', notes: [VARIANT, notChecked({ code: 'df', index: 0 }, { code: 'data_precision', decimals: 1 }), notChecked({ code: 'p' }, { code: 'data_precision', decimals: 1 })] })
    );
    expect(label.textContent).toBe('Reproduced, except degrees of freedom 1 and the p-value, which were not decided');
  });
});

describe('where the significance threshold came from (section 6)', () => {
  it("says the paper's stated threshold, with its value, only when the paper states one", () => {
    const { text } = card(result({ kind: 'notReproduced', conclusionChanges: true }), { printed: ['.01'], stated: true });
    expect(text).toContain("At the paper's stated threshold (α = .01), the conclusion would change.");
  });

  it('says the usual .05 and why when the paper states none', () => {
    const { text } = card(result({ kind: 'notReproduced', conclusionChanges: false }), { printed: ['.05'], stated: false });
    expect(text).toContain('At the usual threshold of .05, because the paper states none, the conclusion would not change.');
    expect(text).not.toMatch(/paper's (stated )?(significance )?threshold/);
  });
});

describe('a number matched only by truncation (section 7)', () => {
  const truncatedP = { ...P, printed: '.04', status: { code: 'matched', note: 'truncated' } };
  const notes = [VARIANT, { code: 'printed_truncated', number: 'p' }];

  it('says so beside the number', () => {
    const { text } = card(result({ kind: 'reproduced', notes }, [STAT, DF, truncatedP]));
    expect(text).toContain('Matches if the paper cut this number off instead of rounding it.');
  });

  it('says in one sentence why Level 0 called the same claim inconsistent', () => {
    const { text } = card(result({ kind: 'reproduced', notes }, [STAT, DF, truncatedP]), undefined, { code: 'inconsistent', conclusion_changes: 'no' });
    const s = screen.getByTestId('l1-result-0-levels-differ').textContent ?? '';
    expect(s).toBe(
      'Level 0 found these printed numbers inconsistent because it assumes every printed number was rounded; checked against the data, they match if the paper cut a number off instead.'
    );
    expect(text).toContain(s);
  });

  it('says nothing of Level 0 when Level 0 agreed', () => {
    card(result({ kind: 'reproduced', notes }, [STAT, DF, truncatedP]));
    expect(screen.queryByTestId('l1-result-0-levels-differ')).toBeNull();
  });
});

describe('evaluation fixes, part C', () => {
  it('a p within the file\'s rounding is "not decided", in one plain sentence with no capital after a colon (M-S5)', () => {
    card(result({ kind: 'reproduced', notes: [VARIANT, notChecked({ code: 'p' }, { code: 'data_precision', decimals: 2 })] }));
    expect(screen.getByTestId('l1-result-0-not-compared').textContent).toBe(
      "The p-value differs from the value computed from the data by no more than the file's rounding to 2 decimals could explain, so it was not decided."
    );
  });

  it('whole numbers are named as such, in the reason and beside the computed value (B-1)', () => {
    const ranged = { ...STAT, printed: '2.20', status: { code: 'not_checked', why: { code: 'data_precision', decimals: 0 } }, precisionLo: 2.05, precisionHi: 2.25 };
    const r = result({ kind: 'couldNotCheck', reason: { code: 'data_precision', decimals: 0 } }, [ranged, DF, P]);
    r.candidates[0].storedDecimals = 0;
    const { text } = card(r);
    expect(screen.getByTestId('l1-result-0-reason').textContent).toBe(
      "The printed statistic differs from the value computed from the data by no more than the file's rounding to whole numbers could explain, so it was not decided."
    );
    expect(text).toContain("(with the file's rounding to whole numbers it could be anywhere from 2.05 to 2.25)");
  });

  it('the card says which whole-number answer was used (B-1)', () => {
    const r = result({ kind: 'reproduced', notes: [VARIANT] });
    r.wholeNumbers = [{ alias: 'C1', answer: 'rounded' }, { alias: 'C3', answer: 'exact' }];
    card(r);
    const lines = Array.from(screen.getByTestId('l1-result-0-whole-numbers').querySelectorAll('li')).map((li) => li.textContent);
    expect(lines).toEqual([
      'Column C1 holds whole numbers and was treated as rounded to whole numbers (your answer: rounded, or not sure), so each value may be off by up to half a unit.',
      'Column C3 was treated as exact whole numbers, as you answered, so its values were taken as they are.',
    ]);
  });

  it('a double-rounded print says how it matches, on the number and in the notes (B-2)', () => {
    const doubled = { ...P, printed: '.13', computed: 0.124684, status: { code: 'matched', note: { code: 'p_note', note: 'double_rounded' } } };
    const tStat = { ...STAT, printed: '3.09', computed: 3.0848, status: { code: 'matched', note: 'double_rounded_in_magnitude' } };
    const { text } = card(result({ kind: 'reproduced', notes: [VARIANT, { code: 'p_note', note: 'double_rounded' }, { code: 'printed_double_rounded', number: 'statistic' }] }, [tStat, DF, doubled]));
    expect(text).toContain('Matches if the paper rounded a number the software had already rounded (for example .1249 shown as .125, printed .13).');
    expect(text).toContain('Matches in magnitude if the paper rounded a number the software had already rounded; the sign was not compared.');
    expect(text).toContain('The test statistic matches if the paper rounded a number the software had already rounded.');
    expect(screen.getByTestId('l1-result-0-verdict').textContent).toBe('Reproduced');
  });
});
