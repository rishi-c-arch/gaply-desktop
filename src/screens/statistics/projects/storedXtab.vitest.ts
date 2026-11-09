// Gaply — crosstabs' saved results pass the shape check exactly as Rust writes
// them, and nothing else (Research OS, Phase 8X-2, decision 591), on
// storedAlpha.vitest.ts's pattern.
//
// SavedResult.vitest.tsx removes every key of the two snapshots one at a time and
// makes every number null or a string; this file holds what those cannot reach,
// since the snapshots carry no warning and every key is checked on its own:
// * the result's keys are exactly CrosstabsOutcome's, and a cell's exactly
//   Frequency's: an EXTRA key is refused, which only the exact key sets catch;
// * a suspect-code warning names its column, value and count, all three;
// * an exclusion is missing_row, missing_column or missing_code, naming its
//   column and nothing else, missing_code allowed once for each column;
// * the not-computable codes are stats_xtab's three, the notes empty or exactly
//   expected_below_five, the limitations exactly its five in order.
//
// The stored results are made here from the two committed snapshots, changed in
// the test; the snapshot files stay as they are.
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

const kindOf = (v: O) => checkStored(v).kind;
const TWO = 'xtab_2x2.json';
const THREE = 'xtab_3x3.json';
const warning = (params: O) => ({ code: 'stats.xtab.suspect_missing_code', params, severity: 'warning' });

describe("crosstabs' saved results (decision 591)", () => {
  it('opens both committed snapshots as crosstabs', () => {
    expect(kindOf(snapshot(TWO))).toBe('crosstabs');
    expect(kindOf(snapshot(THREE))).toBe('crosstabs');
  });

  it("refuses an extra key in the result, and in a cell", () => {
    expect(kindOf(changed(TWO, (b) => (b.primaryOutputs.extra = 1)))).toBe('badShape');
    expect(kindOf(changed(TWO, (b) => (b.primaryOutputs.frequencies[0].extra = 1)))).toBe('badShape');
    expect(kindOf(changed(TWO, (b) => (b.primaryOutputs.rowLevels[0].extra = 1)))).toBe('badShape');
  });

  it('opens a suspect-code warning with its column, value and count, and refuses one without any of them', () => {
    const full = { column: 2, value: -99, count: 3 };
    expect(kindOf(changed(TWO, (b) => (b.diagnostics = [warning(full)])))).toBe('crosstabs');
    for (const k of ['column', 'value', 'count']) {
      const less: O = { ...full };
      delete less[k];
      expect(kindOf(changed(TWO, (b) => (b.diagnostics = [warning(less)]))), k).toBe('badShape');
    }
  });

  it('opens each exclusion naming its column, missing_code once for each column, and refuses one naming none or more', () => {
    const ex = (code: string, params: O) => ({ code: `stats.xtab.${code}`, count: 2, params });
    const both = [ex('missing_code', { column: 1 }), ex('missing_code', { column: 2 })];
    const set = (exclusions: O[]) =>
      changed(THREE, (b) => {
        b.primaryOutputs.exclusions = exclusions;
        b.manifest.exclusions = exclusions;
      });
    expect(kindOf(set(both))).toBe('crosstabs');
    expect(kindOf(set([ex('missing_row', {})]))).toBe('badShape');
    expect(kindOf(set([ex('missing_column', { column: 2, count: 1 })]))).toBe('badShape');
    expect(kindOf(set([ex('same_column', { column: 1 })]))).toBe('badShape');
  });

  it("refuses a not-computable code stats_xtab never writes there, other notes, and the limitations reordered", () => {
    expect(kindOf(changed(TWO, (b) => (b.primaryOutputs.phi = { not_computable: 'stats.xtab.overflow' })))).toBe('crosstabs');
    expect(kindOf(changed(TWO, (b) => (b.primaryOutputs.phi = { not_computable: 'stats.xtab.same_column' })))).toBe('badShape');
    expect(kindOf(changed(TWO, (b) => (b.primaryOutputs.notes = ['stats.xtab.no_layers'])))).toBe('badShape');
    expect(kindOf(changed(TWO, (b) => b.primaryOutputs.limitations.reverse()))).toBe('badShape');
    expect(kindOf(changed(THREE, (b) => (b.primaryOutputs.fisher = { twoSided: 1, less: 1 })))).toBe('badShape');
  });
});
