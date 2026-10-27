// The methods table and the two tables keyed by its capability ids (Research
// OS, Phase 8C; decision 249): METHODS, the stored-result checkers and the
// saved-result renderers name the same capability ids, and those are the ids
// the two result views carry, whose fixtures the wire tests pin to Rust
// (src-tauri/tests/stats_desc_wire.rs and stats_ttest_wire.rs). That every id
// is registered is checked in Rust (project_commands.rs's methods-table test,
// and capability_registry.rs's scan of the frontend).
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { byKind, capabilityOfKind, methodFor, METHODS } from './methods';
import { STORED_CHECKERS } from './projects/storedShape';
import { SAVED_RENDERERS } from './projects/SavedResult';
import { capabilityName } from './projects/projectWording';

const FIXTURES = path.join(__dirname, 'fixtures');
const manifestId = (name: string): string =>
  (JSON.parse(fs.readFileSync(path.join(FIXTURES, name), 'utf8')) as { manifest: { capabilityId: string } }).manifest.capabilityId;

const sorted = (ids: string[]) => ids.slice().sort();

describe('the methods table', () => {
  it('names the ids the result views carry, and the checkers and renderers name the same', () => {
    const fromViews = sorted([
      manifestId('resultBundleView.json'),
      manifestId('ttestResultView.json'),
      manifestId('onesampleResultView.json'),
      manifestId('pairedResultView.json'),
    ]);
    expect(fromViews).toEqual(['stats.analyze.independent_t', 'stats.analyze.one_sample_t', 'stats.analyze.paired_t', 'stats.describe']);
    expect(sorted(METHODS.map((m) => m.capabilityId))).toEqual(fromViews);
    expect(sorted(Object.keys(STORED_CHECKERS))).toEqual(fromViews);
    expect(sorted(Object.keys(SAVED_RENDERERS))).toEqual(fromViews);
  });

  it('keeps the four kinds, their words, names and job names', () => {
    expect(METHODS.map((m) => m.kind)).toEqual(['describe', 'ttest', 'one_sample', 'paired']);
    expect(byKind((m) => m.jobName)).toEqual({
      describe: null,
      ttest: 'independent-samples t-test',
      one_sample: 'one-sample t-test',
      paired: 'paired-samples t-test',
    });
    expect(capabilityOfKind('describe')).toBe(manifestId('resultBundleView.json'));
    expect(capabilityOfKind('ttest')).toBe(manifestId('ttestResultView.json'));
    expect(capabilityOfKind('one_sample')).toBe(manifestId('onesampleResultView.json'));
    expect(capabilityOfKind('paired')).toBe(manifestId('pairedResultView.json'));
    expect(capabilityName(manifestId('resultBundleView.json'))).toBe('Descriptive summary');
    expect(capabilityName(manifestId('ttestResultView.json'))).toBe('Independent-samples t-test');
    expect(capabilityName(manifestId('onesampleResultView.json'))).toBe('One-sample t-test');
    expect(capabilityName(manifestId('pairedResultView.json'))).toBe('Paired-samples t-test');
    expect(METHODS.map((m) => m.path)).toEqual([
      '/app/statistics/describe',
      '/app/statistics/ttest',
      '/app/statistics/one-sample',
      '/app/statistics/paired',
    ]);
  });

  it('finds no row for a capability no method has, nor for a word from Object.prototype', () => {
    expect(methodFor('stats.check')).toBeUndefined();
    expect(methodFor('toString')).toBeUndefined();
    expect(capabilityName('toString')).toBe('An analysis this version of Gaply does not know');
  });
});
