// The factor analysis's result card (Research OS, Phase 9B-3, decisions 729 and 730), on both
// result fixtures Rust writes (src-tauri/tests/stats_factor_wire.rs) and on modified copies of
// them: a null rotation with each of its notes, principal axis's iterations footnote, a Φ
// diagonal of 1.0000000000000002, and no_correlation, which neither fixture carries. The
// fixtures are a wire shape, not a computation. Every number goes through shownNumber; a value
// not computable shows its sentence; an item's name is text only, never in a key, an
// attribute, a title or a test id.
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { factorResultView, factorResultViewOther } from './fixtures/factorFixtures';
import type { FactorCode, FactorResultView, FactorStat } from './factorTypes';
import { FactorCard } from './FactorCard';
import { factorSentence } from './factorWording';
import { shownNumber } from './ttestWording';

afterEach(cleanup);

const NAMES = { column: (a: number) => `"col${a}"` };
/** Principal axis, promax, m 2, an exclusion. */
const promax = factorResultView as unknown as FactorResultView;
/** Principal components, varimax, m 2, singular correlations. */
const varimax = factorResultViewOther as unknown as FactorResultView;
const copy = (r: FactorResultView): FactorResultView => JSON.parse(JSON.stringify(r));
const rowCells = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('td')).map((c) => c.textContent);
const v = (s: FactorStat) => shownNumber((s as { value: number }).value);
const sentence = (code: string) => factorSentence({ code: `stats.factor.${code}` });
const headings = () => screen.getAllByRole('heading').map((h) => h.textContent);

describe("SPSS's tables, in order", () => {
  it('draws principal axis with promax as factors, with promax\'s three tables', () => {
    render(<FactorCard result={promax} names={NAMES} />);
    expect(headings().slice(0, 9)).toEqual([
      'Descriptive statistics',
      "KMO and Bartlett's test",
      "Each item's sampling adequacy",
      'Communalities',
      'Total variance explained',
      'Factor matrix',
      'Pattern matrix',
      'Structure matrix',
      'Factor correlation matrix',
    ]);
    expect(screen.queryByTestId('fa-rotated')).toBeNull();
    expect(screen.queryByTestId('fa-transformation')).toBeNull();
  });

  it("draws principal components with varimax as components, with varimax's two tables", () => {
    render(<FactorCard result={varimax} names={NAMES} />);
    expect(headings().slice(0, 8)).toEqual([
      'Descriptive statistics',
      "KMO and Bartlett's test",
      "Each item's sampling adequacy",
      'Communalities',
      'Total variance explained',
      'Component matrix',
      'Rotated component matrix',
      'Component transformation matrix',
    ]);
    expect(screen.queryByTestId('fa-pattern')).toBeNull();
    expect(screen.queryByTestId('fa-structure')).toBeNull();
    expect(screen.queryByTestId('fa-correlations')).toBeNull();
    expect(within(screen.getByTestId('fa-matrix')).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Item', 'Component', '', '1', '2']);
  });
});

describe('each table, label by label and number by number', () => {
  it('the descriptives, with the rows used under them', () => {
    render(<FactorCard result={promax} names={NAMES} />);
    const r = promax.result;
    for (const d of r.descriptives) {
      expect(screen.getByTestId(`fa-desc-${d.item}`).querySelector('th')?.textContent).toBe(`"col${d.item.slice(1)}"`);
      expect(rowCells(`fa-desc-${d.item}`)).toEqual([v(d.mean), v(d.sd)]);
    }
    expect(within(screen.getByTestId('fa-descriptives')).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Item', 'Mean', 'Standard deviation']);
    expect(screen.getByTestId('fa-rows-used').textContent).toBe('Rows used: 40');
  });

  it("KMO, Bartlett's test and the determinant", () => {
    render(<FactorCard result={promax} names={NAMES} />);
    const r = promax.result;
    expect(screen.getByTestId('fa-kmo-overall').textContent).toBe(`Kaiser-Meyer-Olkin measure of sampling adequacy${v(r.kmo.overall)}`);
    expect(screen.getByTestId('fa-bartlett-chi').textContent).toBe(`Bartlett's test of sphericityApproximate chi-square${v(r.bartlett.chiSquare)}`);
    expect(screen.getByTestId('fa-bartlett-df').textContent).toBe('df15');
    expect(screen.getByTestId('fa-bartlett-p').textContent).toBe(`p${v(r.bartlett.p)}`);
    expect(screen.getByTestId('fa-determinant').textContent).toBe(`Determinant of the correlation matrix${v(r.determinant.value)}`);
    expect(screen.getByTestId('fa-log-determinant').textContent).toBe(`Natural log of the determinant${v(r.determinant.log)}`);
  });

  it('the sampling adequacy and the communalities', () => {
    render(<FactorCard result={promax} names={NAMES} />);
    const r = promax.result;
    expect(within(screen.getByTestId('fa-msa')).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Item', 'Measure of sampling adequacy']);
    for (const a of r.kmo.items) expect(rowCells(`fa-msa-${a.item}`)).toEqual([v(a.msa)]);
    expect(within(screen.getByTestId('fa-communalities')).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Item', 'Initial', 'Extraction']);
    for (const c of r.extraction.communalities) expect(rowCells(`fa-comm-${c.item}`)).toEqual([v(c.initial), v(c.extraction)]);
  });

  it('the variance table: every eigenvalue, the extraction in the first m rows, and for promax its sentence', () => {
    render(<FactorCard result={promax} names={NAMES} />);
    const r = promax.result;
    const heads = within(screen.getByTestId('fa-variance')).getAllByRole('columnheader').map((h) => h.textContent);
    expect(heads).toEqual(['Factor', 'Initial eigenvalues', 'Extraction sums of squared loadings', '', 'Total', '% of variance', 'Cumulative %', 'Total', '% of variance', 'Cumulative %']);
    r.eigenvalues.forEach((e, k) => {
      const ext = r.extraction.sums[k];
      expect(rowCells(`fa-variance-${k + 1}`)).toEqual([v(e.total), v(e.percent), v(e.cumulative), ...(ext ? [v(ext.total), v(ext.percent), v(ext.cumulative)] : ['', '', ''])]);
    });
    expect(screen.getByTestId('fa-promax-sums-sentence').textContent).toBe(
      "Promax's factors are correlated, so the rotation sums of squared loadings that SPSS lists for them are not given here."
    );
  });

  it("the variance table for varimax: the rotation sums in the first m rows, and no promax sentence", () => {
    render(<FactorCard result={varimax} names={NAMES} />);
    const r = varimax.result;
    expect(screen.getByTestId('fa-variance-rotation-head').textContent).toBe('Rotation sums of squared loadings');
    expect(within(screen.getByTestId('fa-variance')).getAllByRole('columnheader')[0].textContent).toBe('Component');
    const s = r.rotation?.sums ?? [];
    expect(rowCells('fa-variance-1').slice(6)).toEqual([v(s[0].total), v(s[0].percent), v(s[0].cumulative)]);
    expect(rowCells(`fa-variance-${r.eigenvalues.length}`).slice(3)).toEqual(['', '', '', '', '', '']);
    expect(screen.queryByTestId('fa-promax-sums-sentence')).toBeNull();
  });

  it('the matrices with their footnotes, numbered columns and the rows of items', () => {
    render(<FactorCard result={promax} names={NAMES} />);
    const r = promax.result;
    const rot = r.rotation!;
    expect(screen.getByTestId('fa-matrix-footnote').textContent).toBe('2 factors extracted, in 11 iterations.');
    expect(screen.getByTestId('fa-pattern-footnote').textContent).toBe('Promax (power 4) with Kaiser normalization; its varimax converged in 2 cycles.');
    r.items.forEach((item, i) => {
      expect(rowCells(`fa-matrix-${item}`)).toEqual(r.extraction.loadings[i].map(v));
      expect(rowCells(`fa-pattern-${item}`)).toEqual(rot.loadings[i].map(v));
      expect(rowCells(`fa-structure-${item}`)).toEqual(rot.structure![i].map(v));
    });
    expect(rowCells('fa-correlations-1')).toEqual(rot.factorCorrelations![0].map(v));
    expect(rowCells('fa-correlations-2')).toEqual(rot.factorCorrelations![1].map(v));
    expect(within(screen.getByTestId('fa-correlations')).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Factor', '1', '2']);
  });

  it("varimax's rotated and transformation matrices", () => {
    render(<FactorCard result={varimax} names={NAMES} />);
    const r = varimax.result;
    const rot = r.rotation!;
    expect(screen.getByTestId('fa-matrix-footnote').textContent).toBe('2 components extracted.');
    expect(screen.getByTestId('fa-rotated-footnote').textContent).toBe(`Varimax with Kaiser normalization; the rotation converged in ${rot.iterations === 1 ? '1 cycle' : `${rot.iterations} cycles`}.`);
    r.items.forEach((item, i) => expect(rowCells(`fa-rotated-${item}`)).toEqual(rot.loadings[i].map(v)));
    expect(rowCells('fa-transformation-1')).toEqual(rot.transformation![0].map(v));
    expect(within(screen.getByTestId('fa-transformation')).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Component', '1', '2']);
  });
});

describe('not computable, notes, exclusions and limitations', () => {
  it("shows the singular fixture's codes as their sentences where the numbers would be, and its note", () => {
    render(<FactorCard result={varimax} names={NAMES} />);
    const singular = sentence('correlations_singular');
    expect(rowCells('fa-kmo-overall')).toEqual([singular]);
    expect(rowCells('fa-determinant')).toEqual([singular]);
    expect(rowCells('fa-bartlett-p')).toEqual([singular]);
    expect(screen.getByTestId('fa-notes').textContent).toBe(sentence('eigenvalues_not_distinct'));
  });

  it("shows no_correlation's sentence for an MSA", () => {
    const r = copy(promax);
    r.result.kmo.items[2].msa = { not_computable: 'stats.factor.no_correlation' };
    render(<FactorCard result={r} names={NAMES} />);
    expect(rowCells('fa-msa-C3')).toEqual(['Not computable: the items this measure covers correlate with no other item, so there is nothing for it to compare.']);
  });

  it.each<[FactorCode, string]>([
    ['stats.factor.rotation_not_converged', 'The rotation did not converge within the maximum iterations, so no rotated solution is given.'],
    ['stats.factor.rotation_singular', "Promax's fit cannot be computed for these loadings, so no rotated solution is given."],
    ['stats.factor.one_factor_not_rotated', 'One factor was extracted, so there is nothing to rotate.'],
  ])('draws no rotation table for a null rotation with %s, and its note says why', (note, words) => {
    const r = copy(promax);
    r.result.rotation = null;
    r.result.notes = [note];
    render(<FactorCard result={r} names={NAMES} />);
    for (const id of ['fa-rotated', 'fa-transformation', 'fa-pattern', 'fa-structure', 'fa-correlations', 'fa-promax-sums-sentence', 'fa-variance-rotation-head']) {
      expect(screen.queryByTestId(id), id).toBeNull();
    }
    expect(screen.getByTestId('fa-notes').textContent).toBe(words);
    expect(screen.getByTestId('fa-matrix')).toBeTruthy();
  });

  it("writes principal axis's iterations in the singular", () => {
    const r = copy(promax);
    r.result.extraction.iterations = 1;
    r.result.extraction.factors = 1;
    render(<FactorCard result={r} names={NAMES} />);
    expect(screen.getByTestId('fa-matrix-footnote').textContent).toBe('1 factor extracted, in 1 iteration.');
  });

  it('shows a Φ diagonal of 1.0000000000000002 as 1', () => {
    const r = copy(promax);
    r.result.rotation!.factorCorrelations![1][1] = { value: 1.0000000000000002 };
    render(<FactorCard result={r} names={NAMES} />);
    expect(rowCells('fa-correlations-2')[1]).toBe('1');
  });

  it('lists the exclusion by its code and item, and every limitation', () => {
    render(<FactorCard result={promax} names={NAMES} />);
    expect(within(screen.getByTestId('fa-exclusions')).getByRole('listitem').textContent).toBe(
      factorSentence(promax.result.exclusions[0], NAMES)
    );
    expect(factorSentence(promax.result.exclusions[0], NAMES)).toContain('"col2"');
    expect(within(screen.getByTestId('fa-limitations')).getAllByRole('listitem').map((l) => l.textContent)).toEqual(promax.result.limitations.map((c) => factorSentence({ code: c })));
    expect(screen.getByTestId('fa-limitations').querySelectorAll('li')).toHaveLength(9);
  });
});

describe('an item name', () => {
  it('appears only as text, never in a key, an attribute, a title or a test id', () => {
    const sentinel = 'LABEL_SENTINEL_9b3';
    const { container } = render(<FactorCard result={promax} names={{ column: (a) => `${sentinel}${a}` }} />);
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
