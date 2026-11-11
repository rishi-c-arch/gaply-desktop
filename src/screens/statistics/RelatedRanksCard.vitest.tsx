// The related rank tests' result card (Research OS, Phase 8Z-2, decisions 653 and 654), on
// both result fixtures Rust writes (src-tauri/tests/stats_related_ranks_wire.rs). The
// fixtures are a wire shape, not a computation. Every number goes through shownNumber;
// the Wilcoxon lines only for two columns, the Friedman lines only for three or more; a
// value not computable shows its sentence; a column's name is text only, never in a
// key, an attribute, a title or a test id.
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { relatedRanksResultView, relatedRanksResultViewOther } from './fixtures/relatedRanksFixtures';
import type { RelatedRanksCode, RelatedRanksResultView, Wilcoxon } from './relatedRanksTypes';
import { RelatedRanksCard } from './RelatedRanksCard';
import { relatedRanksSentence } from './relatedRanksWording';

afterEach(cleanup);

const NAMES = { column: (a: number) => `"col${a}"` };
const w = relatedRanksResultView as unknown as RelatedRanksResultView;
const f = relatedRanksResultViewOther as unknown as RelatedRanksResultView;
const wilcoxon = (r: RelatedRanksResultView): Wilcoxon => (r.result.test as { wilcoxon: Wilcoxon }).wilcoxon;
const withW = (patch: Partial<Wilcoxon>, notes?: RelatedRanksCode[]): RelatedRanksResultView => ({
  ...w,
  result: { ...w.result, notes: notes ?? w.result.notes, test: { wilcoxon: { ...wilcoxon(w), ...patch } } },
});
const cellsOf = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('th, td')).map((c) => c.textContent);

describe('the columns table', () => {
  it("draws one row per column in the list's order, each named as text, with its median", () => {
    render(<RelatedRanksCard result={w} names={NAMES} />);
    expect(screen.getByRole('heading', { name: 'Columns' })).toBeTruthy();
    const t = within(screen.getByTestId('rr-columns-table'));
    expect(t.getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Column', 'Median']);
    expect(cellsOf('rr-col-C1')).toEqual(['"col1"', '2.5']);
    expect(cellsOf('rr-col-C2')).toEqual(['"col2"', '4.75']);
  });

  it('adds the mean rank and the sum of ranks for the Friedman test', () => {
    render(<RelatedRanksCard result={f} names={NAMES} />);
    const t = within(screen.getByTestId('rr-columns-table'));
    expect(t.getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Column', 'Median', 'Mean rank', 'Sum of ranks']);
    expect(cellsOf('rr-col-C3')).toEqual(['"col3"', '7', '2', '6']);
  });

  it('keeps a column list in the order the result gives', () => {
    const swapped: RelatedRanksResultView = { ...w, result: { ...w.result, columns: [...w.result.columns].reverse() } };
    render(<RelatedRanksCard result={swapped} names={NAMES} />);
    expect(screen.getAllByTestId(/^rr-col-C/).map((r) => r.getAttribute('data-testid'))).toEqual(['rr-col-C2', 'rr-col-C1']);
  });
});

describe('a Wilcoxon result', () => {
  it("draws decision 654's ranks table and no Friedman line", () => {
    render(<RelatedRanksCard result={w} names={NAMES} />);
    expect(screen.getByRole('heading', { name: 'Ranks of the differences' })).toBeTruthy();
    expect(screen.queryByRole('heading', { name: 'Friedman test' })).toBeNull();
    expect(screen.queryByTestId('rr-friedman')).toBeNull();
    const t = within(screen.getByTestId('rr-ranks'));
    expect(t.getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['', 'N', 'Mean rank', 'Sum of ranks']);
    expect(cellsOf('rr-negative')).toEqual(['Negative differences', '5', '4', '20']);
    expect(cellsOf('rr-positive')).toEqual(['Positive differences', '1', '1', '1']);
    expect(cellsOf('rr-zero')).toEqual(['No difference (left out)', '0', '', '']);
  });

  it("draws decision 654's rows, and the sentence naming the columns as text", () => {
    render(<RelatedRanksCard result={w} names={NAMES} />);
    expect(screen.getByRole('heading', { name: 'Wilcoxon signed-rank test' })).toBeTruthy();
    expect(cellsOf('rr-v')).toEqual(['V', '1']);
    expect(cellsOf('rr-z')).toEqual(['z', '-1.992']);
    expect(cellsOf('rr-p')).toEqual(['p (normal approximation)', '0.0464']);
    expect(cellsOf('rr-exact-p')).toEqual(['Exact p', '0.0625']);
    expect(cellsOf('rr-rank-biserial')).toEqual(['Matched-pairs rank-biserial correlation', '-0.904']);
    expect(cellsOf('rr-median-difference')).toEqual(['Median of the differences', '-2.25']);
    expect(screen.getByTestId('rr-first-second').textContent).toBe(
      'The differences are "col1" less "col2"; a pair with no difference is left out of the ranks. V is the sum of the positive differences\' ranks. z and the matched-pairs rank-biserial correlation are positive when "col1"\'s values tend to be the larger.'
    );
  });

  it.each<[string, RelatedRanksCode[]]>([
    ['ties', ['stats.related_ranks.exact_not_given_ties']],
    ['zeros', ['stats.related_ranks.exact_not_given_zeros', 'stats.related_ranks.zero_differences_dropped']],
    ['size', ['stats.related_ranks.exact_not_given_size']],
  ])("says the absent exact p in its cell, and the note why (%s)", (_w, notes) => {
    render(<RelatedRanksCard result={withW({ exactP: null }, notes)} names={NAMES} />);
    expect(cellsOf('rr-exact-p')).toEqual(['Exact p', 'Not given (see the note below).']);
    const list = within(screen.getByTestId('rr-notes')).getAllByRole('listitem').map((l) => l.textContent);
    expect(list).toEqual(notes.map((code) => relatedRanksSentence({ code })));
  });

  it("says No pairs. for a side's mean rank when it has none, and 0 as its sum", () => {
    render(<RelatedRanksCard result={withW({ positive: { count: 0, rankSum: 0, meanRank: null } })} names={NAMES} />);
    expect(cellsOf('rr-positive')).toEqual(['Positive differences', '0', 'No pairs.', '0']);
    expect(cellsOf('rr-negative')).toEqual(['Negative differences', '5', '4', '20']);
  });

  it('shows the count of pairs with no difference', () => {
    render(<RelatedRanksCard result={withW({ zeroDifferences: 3 })} names={NAMES} />);
    expect(cellsOf('rr-zero')).toEqual(['No difference (left out)', '3', '', '']);
  });

  it("shows a not-computable value's sentence in place of its number", () => {
    const nc = (c: string) => ({ not_computable: `stats.related_ranks.${c}` as RelatedRanksCode });
    render(<RelatedRanksCard result={withW({ z: nc('all_differences_zero'), p: nc('all_differences_zero'), rankBiserial: nc('all_differences_zero'), medianDifference: nc('overflow') })} names={NAMES} />);
    const zero = 'Not computable: no pair has a difference, so there is nothing to rank.';
    expect(cellsOf('rr-z')[1]).toBe(zero);
    expect(cellsOf('rr-p')[1]).toBe(zero);
    expect(cellsOf('rr-rank-biserial')[1]).toBe(zero);
    expect(cellsOf('rr-median-difference')[1]).toBe('Not computable: the arithmetic went past the largest number a double can hold.');
  });
});

describe('a Friedman result', () => {
  it("draws decision 654's rows and sentence, and no Wilcoxon line", () => {
    const shown: RelatedRanksResultView = { ...f, result: { ...f.result, test: { friedman: { ...(f.result.test as { friedman: never }).friedman as object, q: { value: 6.5 }, p: { value: 0.0388 }, kendallsW: { value: 0.8125 } } as never } } };
    render(<RelatedRanksCard result={shown} names={NAMES} />);
    expect(screen.getByRole('heading', { name: 'Friedman test' })).toBeTruthy();
    expect(screen.queryByRole('heading', { name: 'Wilcoxon signed-rank test' })).toBeNull();
    expect(screen.queryByRole('heading', { name: 'Ranks of the differences' })).toBeNull();
    expect(screen.queryByTestId('rr-wilcoxon')).toBeNull();
    expect(cellsOf('rr-q')).toEqual(['Q (chi-square)', '6.5']);
    expect(cellsOf('rr-df')).toEqual(['df', '2']);
    expect(cellsOf('rr-friedman-p')).toEqual(['p', '0.0388']);
    expect(cellsOf('rr-kendalls-w')).toEqual(["Kendall's W", '0.8125']);
    expect(screen.getByTestId('rr-friedman-sentence').textContent).toBe("Within each row, the columns' values are ranked from 1 to 3, tied values sharing their mean rank.");
  });

  it("shows the sentence where every value in every row is equal", () => {
    render(<RelatedRanksCard result={f} names={NAMES} />);
    const tied = "Not computable: in every row the chosen columns' values are equal, so the ranks do not differ between columns.";
    expect(cellsOf('rr-q')[1]).toBe(tied);
    expect(cellsOf('rr-friedman-p')[1]).toBe(tied);
    expect(cellsOf('rr-kendalls-w')[1]).toBe(tied);
  });
});

describe('the rest of the card', () => {
  it("draws the notes, the exclusions by their columns, the limitations, the digits note and the manifest", () => {
    render(<RelatedRanksCard result={w} names={NAMES} />);
    expect(within(screen.getByTestId('rr-notes')).getAllByRole('listitem').map((l) => l.textContent)).toEqual([relatedRanksSentence({ code: 'stats.related_ranks.rows_below_five' })]);
    const left = within(screen.getByTestId('rr-exclusions')).getAllByRole('listitem').map((l) => l.textContent);
    expect(left).toEqual(['2 rows had no value in "col2" (an empty cell or a missing-value token), and were left out.', '3 rows held a missing code you ticked in "col1", and were left out.']);
    expect(within(screen.getByTestId('rr-limitations')).getAllByRole('listitem')).toHaveLength(8);
    expect(screen.getByTestId('rr-digits-note').textContent).toContain('six figures');
    expect(screen.getByTestId('rr-manifest')).toBeTruthy();
    expect(screen.getByTestId('rr-result-notes')).toBeTruthy();
  });

  it('says so when no row was left out', () => {
    render(<RelatedRanksCard result={f} names={NAMES} />);
    expect(screen.getByTestId('rr-exclusions').textContent).toContain('No row was left out.');
  });

  // Decision 668: this reads the DOM's attributes, test ids among them; React keys never reach
  // the DOM, so it cannot see them. The keys were read instead (decision 667): aliases and codes.
  it('puts a label sentinel in no attribute or test id', () => {
    const sentinel = 'LBL_SENTINEL_a';
    const { container } = render(<RelatedRanksCard result={w} names={{ column: () => sentinel }} />);
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const a of Array.from(el.attributes)) expect(a.value, a.name).not.toContain('LBL_SENTINEL');
    }
  });
});
