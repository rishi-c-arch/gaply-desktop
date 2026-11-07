// Gaply — every method's spec carries the view's preparation (Research OS,
// Phase 8T-1a, decision 490). Rust refuses a spec on a prepared table without
// one, and a spec on a file's own table with one (stats_desc/mod.rs:574-586),
// so each of the seven builders copies the view's preparation exactly when the
// view has one. On a file's own table the key is ABSENT, never null: Rust reads
// a null as none (contracts/mod.rs:237-258), so only this test can catch it, and
// an absent key keeps every spec on a file's own table byte for byte today's.
import { describe, expect, it } from 'vitest';
import { buildAlphaSpec } from './AlphaSettings';
import { buildAnovaSpec } from './AnovaSettings';
import { buildCorrSpec } from './CorrSettings';
import { buildDescribeSpec } from './DescribeSettings';
import type { DatasetView } from './describeTypes';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { buildOneSampleSpec } from './OneSampleSettings';
import { buildPairedSpec } from './PairedSettings';
import { buildTTestSpec } from './TTestSettings';

const BUILDERS: Array<[string, (view: DatasetView) => object]> = [
  ['alpha', (v) => buildAlphaSpec(v, ['C1', 'C2'], [], null, [], 1)],
  ['anova', (v) => buildAnovaSpec(v, 'C2', 'C1', [], [], [], [], 1)],
  ['corr', (v) => buildCorrSpec(v, ['C1', 'C2'], ['pearson'], 'pairwise', [], 1)],
  ['describe', (v) => buildDescribeSpec(v, { C1: { include: true, treatAs: 'numeric' } }, [], 1)],
  ['onesample', (v) => buildOneSampleSpec(v, 'C2', 0, [], 1)],
  ['paired', (v) => buildPairedSpec(v, 'C1', 'C2', [], 1)],
  ['ttest', (v) => buildTTestSpec(v, 'C2', 'C1', [0, 1], null, [], 1)],
];

describe('every spec carries the view\'s preparation (decision 490)', () => {
  it('the fixtures are what the tests need: one prepared view, one not', () => {
    expect(datasetViewPrepared.preparation).toBeDefined();
    expect('preparation' in datasetView).toBe(false);
  });

  for (const [name, build] of BUILDERS) {
    it(`${name}: on a prepared view, the spec's preparation deep-equals the view's`, () => {
      const spec = build(datasetViewPrepared) as { preparation?: unknown };
      expect(spec.preparation).toEqual(datasetViewPrepared.preparation);
      expect(JSON.parse(JSON.stringify(spec)).preparation).toEqual(datasetViewPrepared.preparation);
    });

    it(`${name}: on a file's own view, the key is absent from the spec and from its JSON`, () => {
      const spec = build(datasetView);
      expect('preparation' in spec).toBe(false);
      expect(JSON.stringify(spec)).not.toContain('preparation');
    });
  }
});
