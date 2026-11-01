// Gaply — the correlations' saved results pass the shape check exactly as
// Rust writes them, and nothing else (Research OS, Phase 8H-2, decision 358).
//
// * missing_value and missing_code exclusions name their column, which
//   stats_corr always sets (listwise); fewer_than_two_values names none, which
//   stats_corr never sets (pairwise): decision 337b's rule, exactly as Rust
//   writes them.
// * The shapes are serde's, externally tagged and snake_case: a t is a value
//   or `{"infinite":"stats.corr.perfect_coefficient"}`; a test is `t` with its
//   df or `z`; an outcome and an interval are computed or not computable; a
//   mean and an SD are a value or not computable; an alias is C and digits.
// * Every number finite, every closed code one of the method's.
//
// The stored results are made here from the two committed snapshots
// (corr_pairwise.json, corr_listwise.json), changed in the test; the snapshot
// files stay as they are.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { checkStored } from './storedShape';

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');

type O = Record<string, any>;

function snapshot(file: string): O {
  return JSON.parse(fs.readFileSync(path.join(STORED, file), 'utf8')) as O;
}

/** A snapshot with its bundle changed by `edit`. */
function changed(file: string, edit: (bundle: O) => void): O {
  const view = snapshot(file);
  const bundle = JSON.parse(view.bundleJson) as O;
  edit(bundle);
  return { ...view, bundleJson: JSON.stringify(bundle) };
}

/** `exclusions` in the result and in the manifest. */
const withExclusions = (file: string, exclusions: O[]) =>
  changed(file, (b) => {
    b.primaryOutputs.exclusions = exclusions;
    b.manifest.exclusions = exclusions;
  });

const kindOf = (v: O) => checkStored(v).kind;

describe("the correlations' saved results (decision 358)", () => {
  it('opens both committed snapshots as correlations', () => {
    expect(kindOf(snapshot('corr_pairwise.json'))).toBe('correlations');
    expect(kindOf(snapshot('corr_listwise.json'))).toBe('correlations');
  });

  for (const code of ['stats.corr.missing_value', 'stats.corr.missing_code']) {
    it(`${code} with its column opens`, () => {
      expect(kindOf(withExclusions('corr_listwise.json', [{ code, count: 2, params: { column: 1 } }]))).toBe('correlations');
    });
    it(`${code} with its column removed is refused as a bad shape`, () => {
      expect(kindOf(withExclusions('corr_listwise.json', [{ code, count: 2, params: {} }]))).toBe('badShape');
    });
  }

  it('fewer_than_two_values names no column, as Rust writes it, and opens', () => {
    expect(kindOf(withExclusions('corr_pairwise.json', [{ code: 'stats.corr.fewer_than_two_values', count: 1, params: {} }]))).toBe(
      'correlations'
    );
  });

  it('fewer_than_two_values with a column is refused as a bad shape', () => {
    expect(
      kindOf(withExclusions('corr_pairwise.json', [{ code: 'stats.corr.fewer_than_two_values', count: 1, params: { column: 1 } }]))
    ).toBe('badShape');
  });

  it('a perfect t is its closed word, and any other value there is refused', () => {
    const perfect = changed('corr_pairwise.json', (b) => {
      b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.test.t.t = { infinite: 'stats.corr.perfect_coefficient' };
      b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.p = 0;
    });
    expect(kindOf(perfect)).toBe('correlations');
    for (const t of [null, { value: null }, { infinite: 'stats.corr.overflow' }, { infinite: 'perfect_coefficient' }, { value: 1, infinite: 'stats.corr.perfect_coefficient' }]) {
      const v = changed('corr_pairwise.json', (b) => {
        b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.test.t.t = t;
      });
      expect(kindOf(v), JSON.stringify(t)).toBe('badShape');
    }
  });

  it('a test is t with its df or z, and nothing else', () => {
    const z = changed('corr_pairwise.json', (b) => {
      b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.test = { z: 1.5 };
    });
    expect(kindOf(z)).toBe('correlations');
    for (const test of [{ t: { t: { value: 2 } } }, { z: 'x' }, { z: 1, t: { t: { value: 2 }, df: 3 } }, { chi: 2 }]) {
      const v = changed('corr_pairwise.json', (b) => {
        b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.test = test;
      });
      expect(kindOf(v), JSON.stringify(test)).toBe('badShape');
    }
  });

  it('an outcome, an interval, a mean and an SD are computed or not computable, in its codes', () => {
    const fine: Array<(b: O) => void> = [
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].outcome = { not_computable: 'stats.corr.variable_constant' }),
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.interval = { not_computable: 'stats.corr.interval_too_few_rows' }),
      (b) => (b.primaryOutputs.descriptives[0].mean = { not_computable: 'stats.corr.no_values' }),
      (b) => (b.primaryOutputs.descriptives[0].sd = { not_computable: 'stats.corr.too_few_values' }),
    ];
    for (const edit of fine) expect(kindOf(changed('corr_pairwise.json', edit))).toBe('correlations');
    const bad: Array<(b: O) => void> = [
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].outcome = { not_computable: 'stats.anova.overflow' }),
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.interval = { computed: { lower: null, upper: 1 } }),
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.estimate = null),
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].outcome.computed.p = 'small'),
      (b) => (b.primaryOutputs.descriptives[0].mean = { value: null }),
      (b) => (b.primaryOutputs.descriptives[0].sd = 2.5),
    ];
    for (const edit of bad) expect(kindOf(changed('corr_pairwise.json', edit))).toBe('badShape');
  });

  it('aliases are C and digits, coefficients one of the three, and limitations its codes', () => {
    const bad: Array<(b: O) => void> = [
      (b) => (b.primaryOutputs.descriptives[0].variable = 'Trust'),
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].first = 'C'),
      (b) => (b.primaryOutputs.coefficients[0].pairs[0].second = 3),
      (b) => (b.primaryOutputs.coefficients[0].coefficient = 'phi'),
      (b) => (b.primaryOutputs.limitations = ['stats.anova.no_contrasts']),
      (b) => (b.primaryOutputs.rowsUsed = -1),
    ];
    for (const edit of bad) expect(kindOf(changed('corr_pairwise.json', edit))).toBe('badShape');
  });
});
