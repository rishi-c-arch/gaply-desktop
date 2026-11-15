// The structural equation model's result card (Research OS, Phase 9C-5, decisions 814 and 815), on
// both result fixtures Rust writes (src-tauri/tests/stats_sem_wire.rs) and on modified copies of
// them: two factors with a path (Wheaton-like) and a loop with covariances and notes; the loop's
// diagonal cells blank; no covariances; no effects; every not-computable code a cell can hold;
// the notes; the attempt's lines; 0 iterations; a -0; a HOELTER of 1,550,001; a P below .001;
// each matrix's blank cells exactly where the result lists no pair. The fixtures are a wire
// shape, not a computation: the expected numbers here are the fixtures' own, as shownNumber shows
// them. A column's name is text only, never in a key, an attribute, a title or a test id.
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { semResultView, semResultViewOther } from './fixtures/semFixtures';
import type { SemCode, SemResultView, SemStat, SemVariable } from './semTypes';
import { SemCard } from './SemCard';
import { semCellSentence, semSentence } from './semWording';
import { shownNumber } from './ttestWording';

afterEach(cleanup);

const NAMES = { column: (a: number) => `"col${a}"` };
/** Two factors of three columns, a path between them, an exclusion. */
const two = semResultView as unknown as SemResultView;
/** A loop between two columns, two factors, three covariances, a negative variance. */
const loop = semResultViewOther as unknown as SemResultView;
const copy = (r: SemResultView): SemResultView => JSON.parse(JSON.stringify(r));
const cells = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('td')).map((c) => c.textContent);
const rowHead = (id: string) => screen.getByTestId(id).querySelector('th')?.textContent;
const v = (s: SemStat) => shownNumber((s as { value: number }).value);
const headings = () => screen.getAllByRole('heading').map((h) => h.textContent);
const nc = (code: string): SemStat => ({ not_computable: `stats.sem.${code}` as SemCode });
const key = (x: SemVariable) => ('factor' in x ? `f${x.factor}` : x.column);
const MATRICES = ['total', 'total-standardized', 'direct', 'direct-standardized', 'indirect', 'indirect-standardized'];
const FIELD = { total: 'total', 'total-standardized': 'totalStandardized', direct: 'direct', 'direct-standardized': 'directStandardized', indirect: 'indirect', 'indirect-standardized': 'indirectStandardized' } as const;

describe("AMOS's text output, in order", () => {
  it('draws the notes for the model, the estimates, the effects and the ten model-fit tables', () => {
    render(<SemCard result={loop} names={NAMES} />);
    expect(headings().slice(0, 18)).toEqual([
      'Notes for the model',
      'Estimates',
      'Regression weights',
      'Standardized regression weights',
      'Covariances',
      'Correlations',
      'Variances',
      'Squared multiple correlations',
      'Effects',
      'Total effects',
      'Standardized total effects',
      'Direct effects',
      'Standardized direct effects',
      'Indirect effects',
      'Standardized indirect effects',
      'Model fit',
      'CMIN',
      'RMR, GFI',
    ]);
    const m = loop.result.fit.model;
    expect(cells('sm-moments')).toEqual([String(m.npar + m.df)]);
    expect(cells('sm-parameters')).toEqual([String(m.npar)]);
    expect(cells('sm-df')).toEqual([String(m.df)]);
    expect(screen.getByTestId('sm-iterations').textContent).toBe(`Maximum likelihood reached its minimum in ${loop.result.iterations} iterations.`);
    expect(screen.queryByTestId('sm-attempt')).toBeNull();
    expect(screen.getByTestId('sm-fit-cmin')).toBeTruthy();
    expect(screen.getByTestId('sm-three-models')).toBeTruthy();
    // The saturated row is left out of RMSEA's and HOELTER's tables, as AMOS leaves it out.
    for (const id of ['rmsea', 'hoelter']) {
      expect(Array.from(screen.getByTestId(`sm-fit-${id}`).querySelectorAll('tbody tr')).map((r) => r.getAttribute('data-testid'))).toEqual([`sm-fit-${id}-model`, `sm-fit-${id}-independence`]);
    }
  });

  it('names each weight "{to} ← {from}", loadings then paths, in the result\'s order, with its four numbers', () => {
    render(<SemCard result={loop} names={NAMES} />);
    const rows = Array.from(screen.getByTestId('sm-weights').querySelectorAll('tbody tr'));
    expect(rows.map((r) => r.querySelector('th')?.textContent)).toEqual(loop.result.weights.map((w) => `${'factor' in w.to ? `Factor ${w.to.factor}` : `"col${w.to.column.slice(1)}"`} ← ${'factor' in w.from ? `Factor ${w.from.factor}` : `"col${w.from.column.slice(1)}"`}`));
    const w = loop.result.weights[1];
    expect(cells(`sm-weight-${key(w.to)}-${key(w.from)}`)).toEqual([v(w.estimate), v(w.se), v(w.cr), v(w.p)]);
    expect(cells(`sm-weight-C5-f1`)).toEqual([shownNumber(1), ...Array(3).fill(semCellSentence('stats.sem.fixed_parameter'))]);
  });

  it('names an endogenous end of a covariance, and an endogenous variance, by its error term', () => {
    render(<SemCard result={loop} names={NAMES} />);
    // Covariances: Factor 1 ↔ Factor 2; Factor 1 ↔ C3; C3 ↔ C4 (C3 and C4 are exogenous).
    expect(rowHead('sm-covariance-f1-f2')).toBe('Factor 1 ↔ Factor 2');
    expect(rowHead('sm-covariance-C3-C4')).toBe('"col3" ↔ "col4"');
    expect(rowHead('sm-variance-f1')).toBe('Factor 1');
    expect(rowHead('sm-variance-C1')).toBe('Error of "col1"');
    expect(rowHead('sm-variance-C3')).toBe('"col3"');
    expect(rowHead('sm-smc-C1')).toBe('"col1"');
    const withError = copy(loop);
    withError.result.covariances[2] = { ...withError.result.covariances[2], a: { column: 'C1' }, b: { column: 'C5' } };
    cleanup();
    render(<SemCard result={withError} names={NAMES} />);
    expect(rowHead('sm-covariance-C1-C5')).toBe('Error of "col1" ↔ Error of "col5"');
    expect(rowHead('sm-correlation-C1-C5')).toBe('Error of "col1" ↔ Error of "col5"');
  });

  it('says so where the model has no covariances, and draws no covariance or correlation table', () => {
    render(<SemCard result={two} names={NAMES} />);
    expect(screen.getByTestId('sm-no-covariances').textContent).toBe('The model has no covariances, so every pair of exogenous variables is uncorrelated.');
    expect(screen.queryByTestId('sm-covariances')).toBeNull();
    expect(screen.queryByTestId('sm-correlations')).toBeNull();
  });
});

describe('the effects', () => {
  it("draws AMOS's six matrices, rows the `to` and columns the `from` variables in the model's order, blank on the diagonal", () => {
    render(<SemCard result={loop} names={NAMES} />);
    const tos = ['C1', 'C2'];
    const froms = ['C1', 'C2', 'C3', 'C4'];
    for (const id of MATRICES) {
      const t = screen.getByTestId(`sm-effects-${id}`);
      expect(Array.from(t.querySelectorAll('thead th')).map((h) => h.textContent)).toEqual(['', '"col1"', '"col2"', '"col3"', '"col4"']);
      expect(Array.from(t.querySelectorAll('tbody tr')).map((r) => r.querySelector('th')?.textContent)).toEqual(['"col1"', '"col2"']);
      for (const to of tos) {
        for (const from of froms) {
          const cell = screen.getByTestId(`sm-effect-${id}-${to}-${from}`);
          const e = loop.result.effects.find((x) => key(x.to) === to && key(x.from) === from);
          if (e === undefined) expect(cell.textContent, `${id} ${to} ${from}`).toBe('');
          else expect(cell.textContent).toBe(v(e[FIELD[id as keyof typeof FIELD]]));
        }
      }
      expect(screen.getByTestId(`sm-effect-${id}-C1-C1`).textContent).toBe('');
      expect(screen.getByTestId(`sm-effect-${id}-C2-C2`).textContent).toBe('');
    }
    expect(screen.getByTestId('sm-blank-cell').textContent).toBe("A blank cell: its row and its column are the same variable, or no path of arrows leads from the column's variable to the row's.");
    expect(screen.getAllByTestId('sm-blank-cell')).toHaveLength(1);
  });

  it('leaves a cell blank exactly where the result lists no pair, never 0', () => {
    const r = copy(loop);
    r.result.effects = r.result.effects.filter((e) => !(key(e.to) === 'C2' && key(e.from) === 'C3'));
    render(<SemCard result={r} names={NAMES} />);
    for (const id of MATRICES) expect(screen.getByTestId(`sm-effect-${id}-C2-C3`).textContent).toBe('');
    expect(screen.getByTestId('sm-effect-total-C1-C3').textContent).not.toBe('');
  });

  it("draws Wheaton-like effects between factors, Factor 1's on itself blank", () => {
    render(<SemCard result={two} names={NAMES} />);
    expect(screen.getByTestId('sm-effect-total-f2-f1').textContent).toBe(v(two.result.effects[0].total));
    expect(screen.queryByTestId('sm-effect-total-f2-f2')).toBeNull();
  });

  it('says the model has no effects where the result lists none', () => {
    const r = copy(two);
    r.result.effects = [];
    render(<SemCard result={r} names={NAMES} />);
    expect(screen.getByTestId('sm-no-effects').textContent).toBe('The model has no paths, so it has no effects.');
    expect(screen.queryByTestId('sm-effects-total')).toBeNull();
    expect(screen.queryByTestId('sm-blank-cell')).toBeNull();
  });
});

describe('numbers and their reasons', () => {
  it('shows every not-computable code a cell can hold as its sentence, where its number would be', () => {
    const codes = ['fixed_parameter', 'negative_variance', 'too_small', 'overflow', 'distribution_out_of_range', 'df_zero', 'cmin_zero', 'independence_cmin_zero', 'too_few_rows_for_bcc'];
    for (const code of codes) {
      const r = copy(loop);
      r.result.weights[1].standardized = nc(code);
      r.result.effects[0].indirectStandardized = nc(code);
      render(<SemCard result={r} names={NAMES} />);
      const w = r.result.weights[1];
      expect(screen.getByTestId(`sm-standardized-${key(w.to)}-${key(w.from)}`).querySelector('td')?.textContent, code).toBe(semCellSentence(`stats.sem.${code}`));
      const e = r.result.effects[0];
      expect(screen.getByTestId(`sm-effect-indirect-standardized-${key(e.to)}-${key(e.from)}`).textContent, code).toBe(semCellSentence(`stats.sem.${code}`));
      cleanup();
    }
  });

  it('draws the notes: a negative variance, a block not positive definite, a loop', () => {
    const r = copy(loop);
    r.result.notes = ['stats.sem.negative_variance', 'stats.sem.covariance_not_positive_definite', 'stats.sem.nonrecursive', 'stats.sem.df_zero'];
    render(<SemCard result={r} names={NAMES} />);
    expect(Array.from(screen.getByTestId('sm-notes').querySelectorAll('li')).map((l) => l.textContent)).toEqual(r.result.notes.map((code) => semSentence({ code })));
  });

  it('names the attempt at 2 and at 3, and none at 1', () => {
    const second = copy(two);
    second.result.attempt = 2;
    render(<SemCard result={second} names={NAMES} />);
    expect(screen.getByTestId('sm-attempt').textContent).toBe("Fisher scoring did not converge from Gaply's starting values; Newton's method, from the same values, did.");
    cleanup();
    const third = copy(two);
    third.result.attempt = 3;
    render(<SemCard result={third} names={NAMES} />);
    expect(screen.getByTestId('sm-attempt').textContent).toBe("Neither Fisher scoring nor Newton's method converged from Gaply's starting values; Fisher scoring from other starting values for the factors did.");
  });

  it('says so at 0 iterations, shows a -0 as 0, a HOELTER of 1,550,001 whole, and a P below .001 as shownNumber does, never ***', () => {
    const r = copy(two);
    r.result.iterations = 0;
    r.result.weights[1].estimate = { value: -0 };
    r.result.fit.model.hoelter05 = { value: 1550001 };
    r.result.weights[1].p = { value: 1.3520501927542933e-40 };
    render(<SemCard result={r} names={NAMES} />);
    expect(screen.getByTestId('sm-iterations').textContent).toBe("Maximum likelihood's starting values were already its minimum.");
    const w = r.result.weights[1];
    const row = cells(`sm-weight-${key(w.to)}-${key(w.from)}`);
    expect(row[0]).toBe('0');
    expect(row[3]).toBe(shownNumber(1.3520501927542933e-40));
    expect(row[3]).not.toContain('*');
    expect(cells('sm-fit-hoelter-model')[0]).toBe('1550001');
  });
});

describe("a column's label", () => {
  it('appears only as text, never in a key, an attribute, a title or a test id', () => {
    const sentinel = 'LABEL_SENTINEL_9c5';
    const { container } = render(<SemCard result={loop} names={{ column: (a) => `${sentinel}${a}` }} />);
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
