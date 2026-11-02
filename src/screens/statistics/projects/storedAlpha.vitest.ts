// Gaply — Cronbach's alpha's saved results pass the shape check exactly as
// Rust writes them, and nothing else (Research OS, Phase 8I-2, decision 383).
//
// * Exclusions are missing_value and missing_code, each naming its column,
//   which stats_alpha always sets: decision 337b's rule.
// * The shapes are serde's, externally tagged and snake_case: a statistic is
//   a value or not computable; the interval is computed or not computable;
//   interItem has mean, min and max; each item has its alias, its reversed
//   flag and its six statistics; k and rowsUsed are integers, k at least 2
//   and the items' count.
// * Each field's not-computable codes are exactly those stats_alpha writes
//   there (decision 337b's rule); notes are empty or exactly negative_alpha;
//   the limitations are exactly the method's two.
// * Every number finite, every closed code one of the method's.
//
// The stored results are made here from the two committed snapshots
// (alpha_reversed.json, alpha.json), changed in the test; the snapshot files
// stay as they are.
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
const R = 'alpha_reversed.json';
const nc = (code: string) => ({ not_computable: `stats.alpha.${code}` });

describe("Cronbach's alpha's saved results (decision 383)", () => {
  it('opens both committed snapshots as Cronbach\'s alpha', () => {
    expect(kindOf(snapshot('alpha_reversed.json'))).toBe('cronbach_alpha');
    expect(kindOf(snapshot('alpha.json'))).toBe('cronbach_alpha');
  });

  for (const code of ['stats.alpha.missing_value', 'stats.alpha.missing_code']) {
    it(`${code} with its column opens`, () => {
      expect(kindOf(withExclusions(R, [{ code, count: 2, params: { column: 1 } }]))).toBe('cronbach_alpha');
    });
    it(`${code} with its column removed is refused as a bad shape`, () => {
      expect(kindOf(withExclusions(R, [{ code, count: 2, params: {} }]))).toBe('badShape');
    });
  }

  it('an exclusion of any other code, or with another parameter, is refused', () => {
    for (const e of [
      { code: 'stats.alpha.negative_alpha', count: 1, params: { column: 1 } },
      { code: 'stats.alpha.missing_value', count: 1, params: { column: 1, count: 2 } },
      { code: 'stats.alpha.missing_value', count: -1, params: { column: 1 } },
    ]) {
      expect(kindOf(withExclusions(R, [e])), JSON.stringify(e)).toBe('badShape');
    }
  });

  it('each statistic takes exactly the codes stats_alpha writes there', () => {
    const fine: Array<(b: O) => void> = [
      (b) => (b.primaryOutputs.alpha = nc('total_constant')),
      (b) => (b.primaryOutputs.interval = nc('distribution_out_of_range')),
      (b) => (b.primaryOutputs.interval = nc('total_constant')),
      (b) => (b.primaryOutputs.standardized = nc('standardized_not_defined')),
      (b) => (b.primaryOutputs.standardized = nc('item_constant')),
      (b) => (b.primaryOutputs.interItem = { mean: nc('item_constant'), min: nc('item_constant'), max: nc('item_constant') }),
      (b) => (b.primaryOutputs.items[0].correctedItemTotal = nc('others_constant')),
      (b) => (b.primaryOutputs.items[0].correctedItemTotal = nc('item_constant')),
      (b) => (b.primaryOutputs.items[0].alphaIfDeleted = nc('two_items')),
      (b) => (b.primaryOutputs.items[0].alphaIfDeleted = nc('others_constant')),
      (b) => (b.primaryOutputs.items[0].sd = nc('overflow')),
      (b) => (b.primaryOutputs.items[0].mean = nc('overflow')),
      (b) => (b.primaryOutputs.notes = ['stats.alpha.negative_alpha']),
    ];
    for (const edit of fine) expect(kindOf(changed(R, edit)), edit.toString()).toBe('cronbach_alpha');
    const bad: Array<(b: O) => void> = [
      (b) => (b.primaryOutputs.alpha = nc('item_constant')),
      (b) => (b.primaryOutputs.alpha = nc('two_items')),
      (b) => (b.primaryOutputs.interval = nc('item_constant')),
      (b) => (b.primaryOutputs.standardized = nc('total_constant')),
      (b) => (b.primaryOutputs.interItem.mean = nc('two_items')),
      (b) => (b.primaryOutputs.items[0].mean = nc('item_constant')),
      (b) => (b.primaryOutputs.items[0].correctedItemTotal = nc('two_items')),
      (b) => (b.primaryOutputs.items[0].alphaIfDeleted = nc('item_constant')),
      (b) => (b.primaryOutputs.items[0].scaleMeanIfDeleted = nc('underflow')),
      (b) => (b.primaryOutputs.items[0].sd = nc('corr.overflow')),
      (b) => (b.primaryOutputs.alpha = { value: null }),
      (b) => (b.primaryOutputs.alpha = 0.8),
      (b) => (b.primaryOutputs.alpha = { value: 0.8, not_computable: 'stats.alpha.overflow' }),
      (b) => (b.primaryOutputs.interval = { computed: { lower: null, upper: 1 } }),
      (b) => (b.primaryOutputs.interval = { computed: { lower: 0.5 } }),
    ];
    for (const edit of bad) expect(kindOf(changed(R, edit)), edit.toString()).toBe('badShape');
  });

  it('notes are empty or exactly negative_alpha, and the limitations exactly the two', () => {
    const bad: Array<(b: O) => void> = [
      (b) => (b.primaryOutputs.notes = ['stats.alpha.total_constant']),
      (b) => (b.primaryOutputs.notes = ['stats.alpha.negative_alpha', 'stats.alpha.negative_alpha']),
      (b) => (b.primaryOutputs.limitations = ['stats.alpha.assumes_one_dimension']),
      (b) => (b.primaryOutputs.limitations = ['stats.alpha.no_omega', 'stats.alpha.assumes_one_dimension']),
      (b) => (b.primaryOutputs.limitations = ['stats.alpha.assumes_one_dimension', 'stats.alpha.no_omega', 'stats.alpha.no_omega']),
    ];
    for (const edit of bad) expect(kindOf(changed(R, edit)), edit.toString()).toBe('badShape');
  });

  it('k is the items\' count and at least 2, aliases are C and digits, and the flag a boolean', () => {
    const bad: Array<(b: O) => void> = [
      (b) => (b.primaryOutputs.k = 3),
      (b) => (b.primaryOutputs.k = 4.5),
      (b) => {
        b.primaryOutputs.items = b.primaryOutputs.items.slice(0, 1);
        b.primaryOutputs.k = 1;
      },
      (b) => (b.primaryOutputs.items[0].item = 'Calm'),
      (b) => (b.primaryOutputs.items[0].item = 'C'),
      (b) => (b.primaryOutputs.items[1].reversed = 'yes'),
      (b) => delete b.primaryOutputs.items[1].reversed,
      (b) => delete b.primaryOutputs.items[0].alphaIfDeleted,
      (b) => (b.primaryOutputs.rowsUsed = -1),
      (b) => delete b.primaryOutputs.interItem.max,
    ];
    for (const edit of bad) expect(kindOf(changed(R, edit)), edit.toString()).toBe('badShape');
  });
});
