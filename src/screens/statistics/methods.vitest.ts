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
import { METHOD_OUTPUT } from './window/methodOutput';
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
      manifestId('anovaResultView.json'),
      manifestId('corrResultView.json'),
      manifestId('alphaResultView.json'),
      // Research OS, Phase 8X-2 (decision 591).
      manifestId('xtabResultView.json'),
      // Research OS, Phase 8Y-2 (decision 622).
      manifestId('ranksResultView.json'),
      // Research OS, Phase 8Z-2 (decision 656).
      manifestId('relatedRanksResultView.json'),
      // Research OS, Phase 9A-2 (decision 687).
      manifestId('linearRegressionResultView.json'),
      // Research OS, Phase 9B-3 (decision 732).
      manifestId('factorResultView.json'),
    ]);
    expect(fromViews).toEqual([
      'stats.analyze.correlations',
      'stats.analyze.cronbach_alpha',
      'stats.analyze.crosstabs',
      'stats.analyze.factor',
      'stats.analyze.independent_ranks',
      'stats.analyze.independent_t',
      'stats.analyze.linear_regression',
      'stats.analyze.one_sample_t',
      'stats.analyze.one_way_anova',
      'stats.analyze.paired_t',
      'stats.analyze.related_ranks',
      'stats.describe',
    ]);
    expect(sorted(METHODS.map((m) => m.capabilityId))).toEqual(fromViews);
    expect(sorted(Object.keys(STORED_CHECKERS))).toEqual(fromViews);
    expect(sorted(Object.keys(SAVED_RENDERERS))).toEqual(fromViews);
  });

  it('keeps the twelve kinds, their words, names and job names', () => {
    expect(METHODS.map((m) => m.kind)).toEqual(['describe', 'ttest', 'one_sample', 'paired', 'anova', 'correlations', 'linear_regression', 'factor', 'cronbach_alpha', 'crosstabs', 'independent_ranks', 'related_ranks']);
    expect(byKind((m) => m.jobName)).toEqual({
      describe: null,
      ttest: 'independent-samples t-test',
      one_sample: 'one-sample t-test',
      paired: 'paired-samples t-test',
      anova: 'one-way ANOVA',
      correlations: 'correlations',
      linear_regression: 'linear regression',
      factor: 'factor analysis',
      cronbach_alpha: "Cronbach's alpha",
      crosstabs: 'crosstabs',
      independent_ranks: 'rank test',
      related_ranks: 'related-samples rank test',
    });
    expect(capabilityOfKind('describe')).toBe(manifestId('resultBundleView.json'));
    expect(capabilityOfKind('ttest')).toBe(manifestId('ttestResultView.json'));
    expect(capabilityOfKind('one_sample')).toBe(manifestId('onesampleResultView.json'));
    expect(capabilityOfKind('paired')).toBe(manifestId('pairedResultView.json'));
    expect(capabilityOfKind('anova')).toBe(manifestId('anovaResultView.json'));
    expect(capabilityOfKind('correlations')).toBe(manifestId('corrResultView.json'));
    expect(capabilityOfKind('cronbach_alpha')).toBe(manifestId('alphaResultView.json'));
    expect(capabilityOfKind('crosstabs')).toBe(manifestId('xtabResultView.json'));
    expect(capabilityOfKind('independent_ranks')).toBe(manifestId('ranksResultView.json'));
    expect(capabilityOfKind('related_ranks')).toBe(manifestId('relatedRanksResultView.json'));
    expect(capabilityOfKind('linear_regression')).toBe(manifestId('linearRegressionResultView.json'));
    expect(capabilityOfKind('factor')).toBe(manifestId('factorResultView.json'));
    expect(capabilityName(manifestId('resultBundleView.json'))).toBe('Descriptive summary');
    expect(capabilityName(manifestId('ttestResultView.json'))).toBe('Independent-samples t-test');
    expect(capabilityName(manifestId('onesampleResultView.json'))).toBe('One-sample t-test');
    expect(capabilityName(manifestId('pairedResultView.json'))).toBe('Paired-samples t-test');
    expect(capabilityName(manifestId('anovaResultView.json'))).toBe('One-way ANOVA');
    expect(capabilityName(manifestId('corrResultView.json'))).toBe('Correlations');
    expect(capabilityName(manifestId('alphaResultView.json'))).toBe("Cronbach's alpha");
    expect(capabilityName(manifestId('xtabResultView.json'))).toBe('Crosstabs');
    expect(capabilityName(manifestId('ranksResultView.json'))).toBe('Mann-Whitney and Kruskal-Wallis');
    expect(capabilityName(manifestId('relatedRanksResultView.json'))).toBe('Wilcoxon signed-rank and Friedman');
    expect(capabilityName(manifestId('linearRegressionResultView.json'))).toBe('Linear regression');
    expect(capabilityName(manifestId('factorResultView.json'))).toBe('Factor analysis');
    expect(METHODS.map((m) => m.path)).toEqual([
      '/app/statistics/describe',
      '/app/statistics/ttest',
      '/app/statistics/one-sample',
      '/app/statistics/paired',
      '/app/statistics/anova',
      '/app/statistics/correlations',
      // The linear regression has no page or route (decision 687).
      null,
      // Nor has the factor analysis (decision 732).
      null,
      '/app/statistics/alpha',
      // Crosstabs has no page or route (ADR-012:233-241; decision 591).
      null,
      // Nor have the rank tests (decision 622).
      null,
      // Nor have the related rank tests (decision 656).
      null,
    ]);
  });

  // Research OS, Phase 8P (decision 451): every row has its place in the
  // Statistics window's Analyze menu, in decision 441's words.
  it('gives every row a menu group and a label, and a new method adds one row', () => {
    expect(METHODS.map((m) => [m.menuGroup, m.menuLabel])).toEqual([
      ['Descriptive statistics', 'Summary…'],
      ['Compare means', 'Independent-samples t-test…'],
      ['Compare means', 'One-sample t-test…'],
      ['Compare means', 'Paired t-test…'],
      ['Compare means', 'One-way ANOVA…'],
      ['Correlate', 'Correlations…'],
      ['Regression', 'Linear…'],
      ['Dimension reduction', 'Factor…'],
      ['Scale', "Reliability (Cronbach's alpha)…"],
      ['Descriptive statistics', 'Crosstabs…'],
      ['Nonparametric tests', 'Independent samples…'],
      ['Nonparametric tests', 'Related samples…'],
    ]);
    for (const m of METHODS) {
      expect(m.menuGroup, m.kind).toBeTruthy();
      expect(m.menuLabel, m.kind).toMatch(/…$/);
    }
    expect(new Set(METHODS.map((m) => m.menuLabel)).size).toBe(METHODS.length);
  });

  it('finds no row for a capability no method has, nor for a word from Object.prototype', () => {
    expect(methodFor('stats.check')).toBeUndefined();
    expect(methodFor('toString')).toBeUndefined();
    expect(capabilityName('toString')).toBe('An analysis this version of Gaply does not know');
  });
});

describe("the Output's table (Research OS, Phase 8S, decision 479)", () => {
  it('has a row for every kind, as STORED_CHECKERS has for every capability', () => {
    expect(Object.keys(METHOD_OUTPUT).sort()).toEqual(METHODS.map((m) => m.kind).sort());
  });
});
