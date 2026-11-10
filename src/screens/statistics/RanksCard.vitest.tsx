// The rank tests' result card (Research OS, Phase 8Y-2, decisions 619 and 620), on both
// result fixtures Rust writes (src-tauri/tests/stats_ranks_wire.rs). ranksResultViewOther.json
// is a wire-shape fixture, not a consistent result. Every number goes through shownNumber;
// the Mann-Whitney and Kruskal-Wallis lines each show only where they belong; a value
// not computable shows its sentence; the exact p shows, or the note's sentence where it
// is absent; a level's label is text only, never in an attribute.
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ranksResultView, ranksResultViewOther } from './fixtures/ranksFixtures';
import type { MannWhitney, RanksCode, RanksResultView } from './ranksTypes';
import { RanksCard } from './RanksCard';
import { ranksSentence } from './ranksWording';

afterEach(cleanup);

const NAMES = { column: (a: number) => `"col${a}"`, level: (i: number) => `lev${i}` };
const mw = ranksResultView as unknown as RanksResultView;
const kw = ranksResultViewOther as unknown as RanksResultView;

const mannWhitney = (r: RanksResultView): MannWhitney => (r.result.test as { mann_whitney: MannWhitney }).mann_whitney;
function withExact(exactP: number | null, notes: RanksCode[]): RanksResultView {
  return { ...mw, result: { ...mw.result, notes, test: { mann_whitney: { ...mannWhitney(mw), exactP } } } };
}

const text = (id: string) => screen.getByTestId(id).textContent;

describe('the groups table', () => {
  it('draws Rank table columns and each group by its level index', () => {
    render(<RanksCard result={mw} names={NAMES} />);
    const t = within(screen.getByTestId('rk-groups'));
    expect(screen.getByRole('heading', { name: 'Ranks' })).toBeTruthy();
    expect(t.getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Group', 'N', 'Median', 'Mean rank', 'Sum of ranks']);
    const row = (level: number) => Array.from(screen.getByTestId(`rk-group-${level}`).querySelectorAll('th, td')).map((c) => c.textContent);
    expect(row(0)).toEqual(['control', '4', '2.5', '3.25', '13']);
    expect(row(2)).toEqual(['treated', '6', '6.25', '7', '42']);
  });
});

describe('a Mann-Whitney result', () => {
  it("draws decision 620's rows and its first and second groups as text, and no Kruskal-Wallis line", () => {
    render(<RanksCard result={mw} names={NAMES} />);
    expect(screen.getByRole('heading', { name: 'Mann-Whitney U test' })).toBeTruthy();
    expect(screen.queryByRole('heading', { name: 'Kruskal-Wallis H test' })).toBeNull();
    expect(screen.queryByTestId('rk-kw')).toBeNull();
    const row = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('th, td')).map((c) => c.textContent);
    expect(row('rk-mw-u1')).toEqual(['U, first group', '3']);
    expect(row('rk-mw-u2')).toEqual(['U, second group', '21']);
    expect(row('rk-mw-z')).toEqual(['z', '-1.917']);
    expect(row('rk-mw-p')).toEqual(['p (normal approximation)', '0.0552']);
    expect(row('rk-mw-exact-p')).toEqual(['Exact p', '0.0667']);
    expect(row('rk-mw-rank-biserial')).toEqual(['Rank-biserial correlation', '-0.75']);
    expect(text('rk-mw-first-second')).toBe(
      "The first group is control, the second treated. U for the first group counts the pairs in which its value is the larger, a tie counting one half; z and the rank-biserial correlation are positive when the first group's values tend to be the larger."
    );
  });

  it.each<[string, RanksCode[]]>([
    ['ties', ['stats.ranks.exact_not_given_ties']],
    ['size', ['stats.ranks.exact_not_given_size']],
  ])("shows the note's sentence, not a number, where exactP is null (%s)", (_why, notes) => {
    render(<RanksCard result={withExact(null, notes)} names={NAMES} />);
    expect(text('rk-mw-exact-p-not-given')).toBe(ranksSentence({ code: notes[0] }));
    expect(screen.getByTestId('rk-mw-exact-p').querySelector('.gds-mono')).toBeNull();
  });

  it('gives both sentences where both notes apply', () => {
    render(<RanksCard result={withExact(null, ['stats.ranks.exact_not_given_ties', 'stats.ranks.exact_not_given_size'])} names={NAMES} />);
    expect(text('rk-mw-exact-p-not-given')).toBe(`${ranksSentence({ code: 'stats.ranks.exact_not_given_ties' })} ${ranksSentence({ code: 'stats.ranks.exact_not_given_size' })}`);
  });

  it("shows a not-computable value's sentence where z and p would be", () => {
    const tied: RanksResultView = {
      ...mw,
      result: { ...mw.result, test: { mann_whitney: { ...mannWhitney(mw), z: { not_computable: 'stats.ranks.all_values_tied' }, p: { not_computable: 'stats.ranks.all_values_tied' } } } },
    };
    render(<RanksCard result={tied} names={NAMES} />);
    const nc = 'Not computable: every included value is equal, so the ranks do not differ between groups.';
    for (const id of ['rk-mw-z', 'rk-mw-p']) expect(screen.getByTestId(id).querySelectorAll('td')[0].textContent, id).toBe(nc);
  });

  it('draws an exact p of 0 as 0, not as absent', () => {
    render(<RanksCard result={withExact(0, [])} names={NAMES} />);
    expect(Array.from(screen.getByTestId('rk-mw-exact-p').querySelectorAll('td')).map((c) => c.textContent)).toEqual(['0']);
  });
});

describe('a Kruskal-Wallis result', () => {
  it("draws decision 620's rows, each not-computable value's sentence, and no Mann-Whitney line", () => {
    render(<RanksCard result={kw} names={NAMES} />);
    expect(screen.getByRole('heading', { name: 'Kruskal-Wallis H test' })).toBeTruthy();
    expect(screen.queryByRole('heading', { name: 'Mann-Whitney U test' })).toBeNull();
    expect(screen.queryByTestId('rk-mw')).toBeNull();
    expect(screen.queryByTestId('rk-mw-first-second')).toBeNull();
    const nc = 'Not computable: every included value is equal, so the ranks do not differ between groups.';
    const row = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('th, td')).map((c) => c.textContent);
    expect(row('rk-kw-h')).toEqual(['H', nc]);
    expect(row('rk-kw-df')).toEqual(['df', '2']);
    expect(row('rk-kw-p')).toEqual(['p', nc]);
    expect(row('rk-kw-epsilon-squared')).toEqual(['Epsilon-squared', nc]);
  });

  it('draws the numbers where they are computed', () => {
    const computed: RanksResultView = {
      ...kw,
      result: {
        ...kw.result,
        test: { kruskal_wallis: { h: { value: 12.34567891 }, df: 3, p: { value: 0.00626543 }, epsilonSquared: { value: 0.4 } } },
      },
    };
    render(<RanksCard result={computed} names={NAMES} />);
    const value = (id: string) => screen.getByTestId(id).querySelector('td')?.textContent;
    expect(value('rk-kw-h')).toBe('12.3457');
    expect(value('rk-kw-df')).toBe('3');
    expect(value('rk-kw-p')).toBe('0.00626543');
    expect(value('rk-kw-epsilon-squared')).toBe('0.4');
  });
});

describe('the lines below the tables', () => {
  it('lists the notes, every exclusion with its own key, the limitations, the digits note and the manifest', () => {
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    try {
      render(<RanksCard result={mw} names={NAMES} />);
      expect(within(screen.getByTestId('rk-notes')).getAllByRole('listitem').map((l) => l.textContent)).toEqual(['A group has fewer than 5 values, where the normal and chi-square approximations are rougher.']);
      expect(within(screen.getByTestId('rk-exclusions')).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
        '2 rows had no group (an empty cell or a missing-value token), and were left out.',
        '5 rows were in levels left out, and were left out.',
        '1 row had no outcome (an empty cell or a missing-value token), and was left out.',
        '3 rows held a missing code you ticked in the outcome, and were left out.',
      ]);
      expect(errors.mock.calls.filter((c) => String(c[0]).includes('same key'))).toEqual([]);
    } finally {
      errors.mockRestore();
    }
    expect(within(screen.getByTestId('rk-limitations')).getAllByRole('listitem')).toHaveLength(7);
    expect(text('rk-digits-note')).toMatch(/six figures/);
    expect(text('rk-manifest-capability')).toBe('stats.analyze.independent_ranks, version 1');
    expect(text('rk-result-notes')).toContain('may be a code for a missing value');
  });

  it('shows the same notes line empty as nothing, and says when no row was left out', () => {
    const none: RanksResultView = { ...mw, result: { ...mw.result, notes: [], exclusions: [] } };
    render(<RanksCard result={none} names={NAMES} />);
    expect(screen.queryByTestId('rk-notes')).toBeNull();
    expect(text('rk-exclusions')).toContain('No row was left out.');
  });
});

describe("a level's label is text only", () => {
  it.each([
    ['a Mann-Whitney result', mw],
    ['a Kruskal-Wallis result', kw],
  ])('a sentinel shows in the text of %s and in no key, attribute, title or test id', (_name, base) => {
    const LVL = 'LVL_SENTINEL_y82';
    const result: RanksResultView = { ...base, result: { ...base.result, groups: base.result.groups.map((g) => ({ ...g, label: `${LVL}${g.level}` })) } };
    const { container } = render(<RanksCard result={result} names={NAMES} />);
    const attrs = Array.from(container.querySelectorAll('*')).flatMap((el) => Array.from(el.attributes).map((a) => `${a.name}=${a.value}`));
    expect(attrs.filter((a) => a.includes(LVL))).toEqual([]);
    expect(container.textContent).toContain(`${LVL}0`);
  });
});
