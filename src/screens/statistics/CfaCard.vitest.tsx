// The confirmatory factor analysis's result card (Research OS, Phase 9C-3, decisions 782 and
// 783), on both result fixtures Rust writes (src-tauri/tests/stats_cfa_wire.rs) and on modified
// copies of them: no covariances (uncorrelated factors, and one factor); every not-computable
// code a cell can hold; a negative variance with its notes; 0 iterations; a -0; a HOELTER of
// 1,550,001; a P below .001. The fixtures are a wire shape, not a computation: the expected
// numbers here are the fixtures' own, as shownNumber shows them. A column's name is text only,
// never in a key, an attribute, a title or a test id.
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { cfaResultView, cfaResultViewOther } from './fixtures/cfaFixtures';
import type { CfaCode, CfaResultView, CfaStat } from './cfaTypes';
import { CfaCard } from './CfaCard';
import { cfaCellSentence, cfaSentence } from './cfaWording';
import { shownNumber } from './ttestWording';

afterEach(cleanup);

const NAMES = { column: (a: number) => `"col${a}"` };
/** Two correlated factors of three columns, 40 rows, an exclusion. */
const two = cfaResultView as unknown as CfaResultView;
/** One factor of three columns at 0 degrees of freedom, an error variance below 0. */
const heywood = cfaResultViewOther as unknown as CfaResultView;
const copy = (r: CfaResultView): CfaResultView => JSON.parse(JSON.stringify(r));
const cells = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('td')).map((c) => c.textContent);
const rowHead = (id: string) => screen.getByTestId(id).querySelector('th')?.textContent;
const v = (s: CfaStat) => shownNumber((s as { value: number }).value);
const headings = () => screen.getAllByRole('heading').map((h) => h.textContent);
const nc = (code: string): CfaStat => ({ not_computable: `stats.cfa.${code}` as CfaCode });
const tableRows = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('tbody tr')).map((r) => r.getAttribute('data-testid'));
const dashLines = (id: string) => Array.from(screen.queryByTestId(`cf-fit-${id}-codes`)?.querySelectorAll('li') ?? []).map((l) => l.textContent);
const FIT = ['cmin', 'rmr', 'baseline', 'parsimony', 'ncp', 'fmin', 'rmsea', 'aic', 'ecvi', 'hoelter'];

describe("AMOS's text output, in order", () => {
  it('draws the notes for the model, the estimates and the ten model-fit tables', () => {
    render(<CfaCard result={two} names={NAMES} />);
    expect(headings().slice(0, 21)).toEqual([
      'Notes for the model',
      'Estimates',
      'Regression weights',
      'Standardized regression weights',
      'Covariances',
      'Correlations',
      'Variances',
      'Squared multiple correlations',
      'Model fit',
      'CMIN',
      'RMR, GFI',
      'Baseline comparisons',
      'Parsimony-adjusted measures',
      'NCP',
      'FMIN',
      'RMSEA',
      'AIC',
      'ECVI',
      'HOELTER',
      'Rows left out',
      'What this does not assess',
    ]);
  });

  it('gives the notes for the model: moments, parameters, degrees of freedom, the iterations and the chi-square', () => {
    render(<CfaCard result={two} names={NAMES} />);
    const m = two.result.fit.model;
    expect([rowHead('cf-moments'), rowHead('cf-parameters'), rowHead('cf-df')]).toEqual([
      'Distinct sample moments',
      'Distinct parameters to be estimated',
      'Degrees of freedom',
    ]);
    expect([cells('cf-moments'), cells('cf-parameters'), cells('cf-df')]).toEqual([['21'], ['13'], ['8']]);
    expect(m.npar + m.df).toBe(21);
    expect(screen.getByTestId('cf-iterations').textContent).toBe(`Maximum likelihood reached its minimum in ${two.result.iterations} iterations.`);
    expect(two.result.iterations).toBe(18);
    expect(screen.getByTestId('cf-chi-square').textContent).toBe(`Chi-square = ${v(m.cmin)}, degrees of freedom = 8, probability level = ${v(m.p)}.`);
  });
});

describe('the estimates', () => {
  it("gives each column's regression weight as \"{column} ← Factor {f}\", the first of each factor fixed at 1", () => {
    render(<CfaCard result={two} names={NAMES} />);
    const heads = Array.from(screen.getByTestId('cf-weights').querySelectorAll('thead th')).map((h) => h.textContent);
    expect(heads).toEqual(['', 'Estimate', 'S.E.', 'C.R.', 'P']);
    expect(tableRows('cf-weights')).toEqual(['cf-weight-C1', 'cf-weight-C3', 'cf-weight-C5', 'cf-weight-C2', 'cf-weight-C4', 'cf-weight-C6']);
    expect(rowHead('cf-weight-C1')).toBe('"col1" ← Factor 1');
    expect(rowHead('cf-weight-C4')).toBe('"col4" ← Factor 2');
    const fixed = "Fixed at 1, to set the factor's scale.";
    expect(cells('cf-weight-C1')).toEqual(['1', fixed, fixed, fixed]);
    expect(cells('cf-weight-C2')).toEqual(['1', fixed, fixed, fixed]);
    const l = two.result.loadings[1];
    expect(cells('cf-weight-C3')).toEqual([v(l.estimate), v(l.se), v(l.cr), v(l.p)]);
    expect(cells('cf-standardized-C3')).toEqual([v(l.standardized)]);
    expect(rowHead('cf-standardized-C3')).toBe('"col3" ← Factor 1');
  });

  it('gives the covariance and correlation of factors 1 and 2, the variances (factors, then errors) and the SMCs', () => {
    render(<CfaCard result={two} names={NAMES} />);
    const c = two.result.factorCovariances[0];
    expect(rowHead('cf-covariance-1-2')).toBe('Factor 1 ↔ Factor 2');
    expect(cells('cf-covariance-1-2')).toEqual([v(c.estimate), v(c.se), v(c.cr), v(c.p)]);
    expect(rowHead('cf-correlation-1-2')).toBe('Factor 1 ↔ Factor 2');
    expect(cells('cf-correlation-1-2')).toEqual([v(c.correlation)]);
    expect(tableRows('cf-variances')).toEqual([
      'cf-variance-factor-1',
      'cf-variance-factor-2',
      'cf-variance-error-C1',
      'cf-variance-error-C3',
      'cf-variance-error-C5',
      'cf-variance-error-C2',
      'cf-variance-error-C4',
      'cf-variance-error-C6',
    ]);
    expect(rowHead('cf-variance-factor-2')).toBe('Factor 2');
    expect(rowHead('cf-variance-error-C5')).toBe('Error of "col5"');
    const e = two.result.errorVariances[2];
    expect(cells('cf-variance-error-C5')).toEqual([v(e.estimate), v(e.se), v(e.cr), v(e.p)]);
    expect(tableRows('cf-smc')).toEqual(['cf-smc-C1', 'cf-smc-C3', 'cf-smc-C5', 'cf-smc-C2', 'cf-smc-C4', 'cf-smc-C6']);
    expect(rowHead('cf-smc-C6')).toBe('"col6"');
    expect(cells('cf-smc-C6')).toEqual([v(two.result.smc[5].value)]);
    expect(screen.queryByTestId('cf-no-covariances')).toBeNull();
  });

  it('draws no covariance or correlation table for uncorrelated factors, and says they are fixed at 0', () => {
    const r = copy(two);
    r.result.factorCovariances = [];
    render(<CfaCard result={r} names={NAMES} />);
    expect(screen.queryByTestId('cf-covariances')).toBeNull();
    expect(screen.queryByTestId('cf-correlations')).toBeNull();
    expect(screen.getByTestId('cf-no-covariances').textContent).toBe("The factors' covariances are fixed at 0.");
    expect(headings()).not.toContain('Covariances');
  });

  it('draws neither table nor line for one factor', () => {
    render(<CfaCard result={heywood} names={NAMES} />);
    expect(screen.queryByTestId('cf-covariances')).toBeNull();
    expect(screen.queryByTestId('cf-correlations')).toBeNull();
    expect(screen.queryByTestId('cf-no-covariances')).toBeNull();
  });

  it("shows every not-computable code an estimate can hold as its sentence where its number would be", () => {
    const r = copy(two);
    const codes = ['negative_variance', 'too_small', 'overflow', 'distribution_out_of_range'];
    const l = r.result.loadings[1];
    [l.estimate, l.se, l.cr, l.p] = codes.map(nc);
    l.standardized = nc('negative_variance');
    r.result.factorCovariances[0].correlation = nc('negative_variance');
    r.result.smc[0].value = nc('negative_variance');
    render(<CfaCard result={r} names={NAMES} />);
    expect(cells('cf-weight-C3')).toEqual(codes.map((c) => cfaCellSentence(`stats.cfa.${c}`)));
    const negative = 'Not defined: a variance it takes the square root of is at or below 0.';
    expect(cells('cf-standardized-C3')).toEqual([negative]);
    expect(cells('cf-correlation-1-2')).toEqual([negative]);
    expect(cells('cf-smc-C1')).toEqual([negative]);
  });
});

describe('the model fit (decision 783)', () => {
  it("gives each table AMOS's columns and rows, the saturated row left out of RMSEA and HOELTER only", () => {
    render(<CfaCard result={two} names={NAMES} />);
    const heads = (id: string) => Array.from(screen.getByTestId(`cf-fit-${id}`).querySelectorAll('thead th')).map((h) => h.textContent);
    expect(FIT.map(heads)).toEqual([
      ['Model', 'NPAR', 'CMIN', 'DF', 'P', 'CMIN/DF'],
      ['Model', 'RMR', 'GFI', 'AGFI', 'PGFI'],
      ['Model', 'NFI Delta1', 'RFI rho1', 'IFI Delta2', 'TLI rho2', 'CFI'],
      ['Model', 'PRATIO', 'PNFI', 'PCFI'],
      ['Model', 'NCP', 'LO 90', 'HI 90'],
      ['Model', 'FMIN', 'F0', 'LO 90', 'HI 90'],
      ['Model', 'RMSEA', 'LO 90', 'HI 90', 'PCLOSE'],
      ['Model', 'AIC', 'BCC', 'BIC', 'CAIC'],
      ['Model', 'ECVI', 'LO 90', 'HI 90', 'MECVI'],
      ['Model', 'HOELTER .05', 'HOELTER .01'],
    ]);
    for (const id of FIT) {
      const rows = Array.from(screen.getByTestId(`cf-fit-${id}`).querySelectorAll('tbody tr th')).map((h) => h.textContent);
      expect(rows, id).toEqual(id === 'rmsea' || id === 'hoelter' ? ['Default model', 'Independence model'] : ['Default model', 'Saturated model', 'Independence model']);
    }
    expect(screen.getByTestId('cf-three-models').textContent).toBe(
      "The default model is the one you specified. The saturated model has a parameter for every variance and covariance, so it fits them exactly. The independence model has only the columns' variances, so it says the columns are uncorrelated."
    );
  });

  it("gives each row's numbers, NPAR, DF and HOELTER whole, P through shownNumber, and a df_zero cell as \"–\"", () => {
    render(<CfaCard result={two} names={NAMES} />);
    const { model, saturated, independence } = two.result.fit;
    expect(cells('cf-fit-cmin-model')).toEqual(['13', v(model.cmin), '8', v(model.p), v(model.cminDf)]);
    expect(cells('cf-fit-cmin-saturated')).toEqual(['21', '0', '0', '–', '–']);
    expect(cells('cf-fit-cmin-independence')).toEqual([String(independence.npar), v(independence.cmin), String(independence.df), v(independence.p), v(independence.cminDf)]);
    expect(cells('cf-fit-parsimony-saturated')).toEqual(['–', '–', '–']);
    expect(cells('cf-fit-baseline-saturated')).toEqual(['1', '–', '1', '–', '1']);
    expect(cells('cf-fit-rmsea-model')).toEqual([v(model.rmsea), v(model.rmseaLo90), v(model.rmseaHi90), v(model.pclose)]);
    const h = (s: CfaStat) => String((s as { value: number }).value);
    expect(cells('cf-fit-hoelter-model')).toEqual([h(model.hoelter05), h(model.hoelter01)]);
    expect(cells('cf-fit-aic-saturated')).toEqual([v(saturated.aic), v(saturated.bcc), v(saturated.bic), v(saturated.caic)]);
    expect(dashLines('cmin')).toEqual(['–: Not defined at 0 degrees of freedom.']);
    expect(dashLines('parsimony')).toEqual(['–: Not defined at 0 degrees of freedom.']);
    expect(dashLines('rmsea')).toEqual([]);
  });

  it("draws the Heywood fixture's default row at 0 degrees of freedom with \"–\" cells, and its notes", () => {
    render(<CfaCard result={heywood} names={NAMES} />);
    expect(cells('cf-fit-cmin-model')).toEqual(['6', '0', '0', '–', '–']);
    expect(cells('cf-fit-rmsea-model')).toEqual(['–', '–', '–', '–']);
    expect(cells('cf-fit-parsimony-model')).toEqual(['–', '–', '–']);
    expect(screen.getByTestId('cf-chi-square').textContent).toBe('Chi-square = 0, degrees of freedom = 0. Probability level: Not defined at 0 degrees of freedom.');
    expect(Array.from(screen.getByTestId('cf-notes').querySelectorAll('li')).map((l) => l.textContent)).toEqual([
      cfaSentence({ code: 'stats.cfa.negative_variance' }),
      cfaSentence({ code: 'stats.cfa.df_zero' }),
    ]);
    expect(cfaSentence({ code: 'stats.cfa.negative_variance' })).toMatch(/^At least one variance estimate is below 0/);
    const e = heywood.result.errorVariances[0].estimate as { value: number };
    expect(e.value).toBeLessThan(0);
    expect(cells('cf-variance-error-C1')[0]).toBe(shownNumber(e.value));
  });

  it('shows every not-computable code a fit cell can hold as "–", with one line per code under its table', () => {
    const r = copy(two);
    const m = r.result.fit.model;
    m.cmin = nc('too_small');
    m.p = nc('overflow');
    m.cminDf = nc('distribution_out_of_range');
    m.nfi = nc('independence_cmin_zero');
    m.rfi = nc('independence_cmin_zero');
    m.bcc = nc('too_few_rows_for_bcc');
    m.hoelter05 = nc('cmin_zero');
    m.hoelter01 = nc('cmin_zero');
    m.rmsea = nc('df_zero');
    render(<CfaCard result={r} names={NAMES} />);
    expect(cells('cf-fit-cmin-model')).toEqual(['13', '–', '8', '–', '–']);
    const line = (c: string) => `–: ${cfaCellSentence(`stats.cfa.${c}`)}`;
    expect(dashLines('cmin')).toEqual([line('too_small'), line('overflow'), line('distribution_out_of_range'), line('df_zero')]);
    expect(dashLines('baseline')).toEqual([line('independence_cmin_zero'), line('df_zero')]);
    expect(dashLines('aic')).toEqual([line('too_few_rows_for_bcc')]);
    expect(dashLines('hoelter')).toEqual([line('cmin_zero')]);
    expect(dashLines('rmsea')).toEqual([line('df_zero')]);
    // The saturated row's df_zero cells give one line per table, never one per cell.
    expect(screen.getByTestId('cf-result-card').textContent?.split('Not defined at 0 degrees of freedom.').length).toBeLessThan(12);
  });

  it('shows 0 iterations, a -0 as 0, a HOELTER of 1,550,001 whole, and a P below .001 by shownNumber, never with stars', () => {
    const r = copy(two);
    r.result.iterations = 0;
    r.result.fit.model.ifi = { value: -0 };
    r.result.fit.model.tli = { value: -0 };
    r.result.fit.model.hoelter05 = { value: 1550001 };
    r.result.fit.model.p = { value: 0.0000123 };
    render(<CfaCard result={r} names={NAMES} />);
    expect(screen.getByTestId('cf-iterations').textContent).toBe("Maximum likelihood's starting values were already its minimum.");
    expect(cells('cf-fit-baseline-model')[2]).toBe('0');
    expect(cells('cf-fit-baseline-model')[3]).toBe('0');
    expect(cells('cf-fit-hoelter-model')[0]).toBe('1550001');
    expect(cells('cf-fit-cmin-model')[3]).toBe('0.0000123');
    expect(screen.getByTestId('cf-chi-square').textContent).toContain('probability level = 0.0000123.');
    expect(screen.getByTestId('cf-result-card').textContent).not.toMatch(/\*|-0(?![.\d])/);
  });

  it('says 1 iteration in the singular', () => {
    const r = copy(two);
    r.result.iterations = 1;
    render(<CfaCard result={r} names={NAMES} />);
    expect(screen.getByTestId('cf-iterations').textContent).toBe('Maximum likelihood reached its minimum in 1 iteration.');
  });
});

describe('the notes, exclusions and limitations', () => {
  it('lists the rows left out by code and column, and the ten limitations', () => {
    render(<CfaCard result={heywood} names={NAMES} />);
    const left = within(screen.getByTestId('cf-exclusions')).getAllByRole('listitem').map((l) => l.textContent);
    expect(left).toEqual(heywood.result.exclusions.map((x) => cfaSentence(x, NAMES)));
    expect(left[0]).toContain('"col1"');
    const limits = within(screen.getByTestId('cf-limitations')).getAllByRole('listitem').map((l) => l.textContent);
    expect(limits).toHaveLength(10);
    expect(limits).toEqual(heywood.result.limitations.map((code) => cfaSentence({ code })));
    expect(screen.getByTestId('cf-digits-note').textContent).toMatch(/six figures/);
  });

  it('says no row was left out when none was, and draws no notes list with none', () => {
    const r = copy(two);
    r.result.exclusions = [];
    render(<CfaCard result={r} names={NAMES} />);
    expect(screen.getByTestId('cf-exclusions').textContent).toContain('No row was left out.');
    expect(screen.queryByTestId('cf-notes')).toBeNull();
  });

  it('gives the factor covariance note', () => {
    const r = copy(two);
    r.result.notes = ['stats.cfa.factor_covariance_not_positive_definite'];
    render(<CfaCard result={r} names={NAMES} />);
    expect(screen.getByTestId('cf-notes').textContent).toBe(
      "The factors' estimated covariance matrix is not positive definite, so this solution is not admissible, as AMOS says of such a solution."
    );
  });
});

describe('a column name', () => {
  it('appears only as text, never in a key, an attribute, a title or a test id', () => {
    const sentinel = 'LABEL_SENTINEL_9c3';
    const { container } = render(<CfaCard result={two} names={{ column: (a) => (a === 3 ? sentinel : `C${a}`) }} />);
    expect(container.textContent).toContain(`${sentinel} ← Factor 1`);
    expect(container.textContent).toContain(`Error of ${sentinel}`);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
