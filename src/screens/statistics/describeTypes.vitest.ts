// The descriptive summary's wire contract, from the TypeScript side
// (Research OS, Phase 2B). Each JSON fixture is the exact serialisation of a
// Rust view (src-tauri/tests/stats_desc_wire.rs checks the bytes); here each
// is deep-compared with a copy typed `satisfies` the view's type, so tsc
// catches a renamed or missing key and this run catches a value or shape
// that differs.
import { describe, expect, it } from 'vitest';

import datasetJson from './fixtures/datasetView.json';
import validationJson from './fixtures/validationReportView.json';
import startedJson from './fixtures/runStartedView.json';
import stateJson from './fixtures/runStateView.json';
import resultJson from './fixtures/resultBundleView.json';
import exportJson from './fixtures/exportView.json';
import * as typed from './fixtures/describeFixtures';

const PAIRS: Array<[string, unknown, unknown]> = [
  ['DatasetView', typed.datasetView, datasetJson],
  ['ValidationReportView', typed.validationReportView, validationJson],
  ['RunStartedView', typed.runStartedView, startedJson],
  ['RunStateView', typed.runStateView, stateJson],
  ['ResultBundleView', typed.resultBundleView, resultJson],
  ['ExportView', typed.exportView, exportJson],
];

/** Every key path in a value, arrays read through. */
function keyPaths(v: unknown, at = ''): string[] {
  if (Array.isArray(v)) return v.flatMap((x) => keyPaths(x, `${at}[]`));
  if (v && typeof v === 'object') {
    return Object.entries(v as Record<string, unknown>).flatMap(([k, x]) => [`${at}.${k}`, ...keyPaths(x, `${at}.${k}`)]);
  }
  return [];
}

describe('the view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('fill every optional field and hold no empty array', () => {
    for (const [name, , json] of PAIRS) {
      const walk = (v: unknown, at: string) => {
        expect(v, `${name}${at} is null`).not.toBeNull();
        if (Array.isArray(v)) {
          expect(v.length, `${name}${at} is empty`).toBeGreaterThan(0);
          v.forEach((x, i) => walk(x, `${at}[${i}]`));
        } else if (v && typeof v === 'object') {
          for (const [k, x] of Object.entries(v as Record<string, unknown>)) if (k !== 'params') walk(x, `${at}.${k}`);
        }
      };
      walk(json, '');
    }
  });

  it('pin every key of every view', () => {
    const keys = (v: unknown) => Array.from(new Set(keyPaths(v))).sort();
    expect(keys(datasetJson)).toEqual([
      '.displayName', '.inputRef', '.inputRef.columns', '.inputRef.contentHash', '.inputRef.rows', '.inputRef.semanticRole',
      '.profile', '.profile.columns', '.profile.columns[].alias', '.profile.columns[].distinct', '.profile.columns[].header',
      '.profile.columns[].kind', '.profile.columns[].levels', '.profile.columns[].levels[].count', '.profile.columns[].levels[].label',
      '.profile.columns[].missing', '.profile.columns[].n', '.profile.columns[].nonFinite', '.profile.columns[].suspectMissingCodes',
      '.profile.columns[].suspectMissingCodes[].cells', '.profile.columns[].suspectMissingCodes[].code', '.profile.rows', '.slot',
    ]);
    expect(keys(validationJson)).toEqual([
      '.errors', '.errors[].code', '.errors[].params', '.errors[].params.column', '.errors[].severity',
      '.estimatedResources', '.estimatedResources.cells', '.estimatedResources.columns', '.estimatedResources.rows',
      '.missingInformation', '.missingInformation[].code', '.missingInformation[].params', '.missingInformation[].severity',
      '.status', '.warnings', '.warnings[].code', '.warnings[].params', '.warnings[].params.column', '.warnings[].params.count',
      '.warnings[].params.value', '.warnings[].severity',
    ]);
    expect(keys(startedJson)).toEqual(['.runId']);
    expect(keys(stateJson)).toEqual([
      '.cancelRequested', '.columnsDone', '.columnsTotal', '.diagnostics', '.diagnostics[].code', '.diagnostics[].params',
      '.diagnostics[].params.column', '.diagnostics[].params.count', '.diagnostics[].params.value', '.diagnostics[].severity',
      '.elapsedMs', '.runId', '.state', '.terminal',
    ]);
    expect(keys(resultJson)).toEqual([
      '.columns', '.columns[].categorical', '.columns[].categorical.alias', '.columns[].categorical.header',
      '.columns[].categorical.levels', '.columns[].categorical.levels[].count', '.columns[].categorical.levels[].label',
      '.columns[].categorical.missing', '.columns[].categorical.n', '.columns[].numeric', '.columns[].numeric.alias',
      '.columns[].numeric.header', '.columns[].numeric.max', '.columns[].numeric.max.value', '.columns[].numeric.mean',
      '.columns[].numeric.mean.value', '.columns[].numeric.median', '.columns[].numeric.median.value', '.columns[].numeric.min',
      '.columns[].numeric.min.value', '.columns[].numeric.missing', '.columns[].numeric.n', '.columns[].numeric.q1',
      '.columns[].numeric.q1.value', '.columns[].numeric.q3', '.columns[].numeric.q3.value', '.columns[].numeric.sd',
      '.columns[].numeric.sd.not_computable', '.diagnostics', '.diagnostics[].code', '.diagnostics[].params',
      '.diagnostics[].params.column', '.diagnostics[].params.count', '.diagnostics[].params.value', '.diagnostics[].severity',
      '.manifest', '.manifest.capabilityId', '.manifest.capabilityVersion', '.manifest.elapsedMs', '.manifest.engineVersion',
      '.manifest.exclusions', '.manifest.exclusions[].code', '.manifest.exclusions[].count', '.manifest.exclusions[].params',
      '.manifest.exclusions[].params.column', '.manifest.inputs', '.manifest.outputHash', '.manifest.parametersHash',
      '.manifest.platform', '.manifest.platform.arch', '.manifest.platform.os', '.manifest.reproducibility', '.manifest.rowsRead',
      '.manifest.rowsUsed', '.manifest.schemaVersion', '.manifest.seed', '.runId', '.status',
    ]);
    expect(keys(exportJson)).toEqual(['.bytes', '.sha256']);
  });

  it('no manifest key can hold a path, a name, a header or a label', () => {
    const manifestKeys = Object.keys(resultJson.manifest);
    for (const k of manifestKeys) expect(k).not.toMatch(/path|file|name|header|label|dir|url/i);
  });
});
