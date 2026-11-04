// The descriptive summary's wire contract, from the TypeScript side
// (Research OS, Phase 2B). Each JSON fixture is the exact serialisation of a
// Rust view (src-tauri/tests/stats_desc_wire.rs checks the bytes); here each
// is deep-compared with a copy typed `satisfies` the view's type, so tsc
// catches a renamed or missing key and this run catches a value or shape
// that differs.
import { describe, expect, it } from 'vitest';

import describeCodes from './describeCodes.json';
import type { DescCode } from './describeTypes';

import datasetJson from './fixtures/datasetView.json';
import datasetPreparedJson from './fixtures/datasetViewPrepared.json';
import validationJson from './fixtures/validationReportView.json';
import startedJson from './fixtures/runStartedView.json';
import stateJson from './fixtures/runStateView.json';
import resultJson from './fixtures/resultBundleView.json';
import exportJson from './fixtures/exportView.json';
import cellWindowJson from './fixtures/cellWindow.json';
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

/** Every code of the union, once: a missing one or an extra one fails tsc
 * (Research OS, Phase 8M-3, decision 428, as onesampleTypes.vitest.ts pins its own). */
const EVERY_CODE: Record<DescCode, true> = {
  'stats.desc.unknown_capability': true,
  'stats.desc.capability_version_mismatch': true,
  'stats.desc.input_count': true,
  'stats.desc.input_mismatch': true,
  'stats.desc.no_columns_selected': true,
  'stats.desc.unknown_column': true,
  'stats.desc.duplicate_column': true,
  'stats.desc.numeric_on_text': true,
  'stats.desc.non_finite_cell': true,
  'stats.desc.missing_code_not_finite': true,
  'stats.desc.suspect_missing_code': true,
  'stats.desc.no_values': true,
  'stats.desc.single_value': true,
  'stats.desc.overflow': true,
  'stats.desc.underflow': true,
  'stats.desc.arithmetic_refused': true,
  'stats.desc.missing_cell': true,
  'stats.desc.missing_code': true,
  'stats.desc.preparation_mismatch': true,
  'stats.desc.score_as_categorical': true,
  'stats.desc.code_equals_score': true,
};

describe('the DescCode union', () => {
  it('is exactly the stats.desc words of describeCodes.json', () => {
    const listed = (describeCodes as string[]).filter((c) => c.startsWith('stats.desc.'));
    expect(listed).toHaveLength(21);
    expect(Object.keys(EVERY_CODE).sort()).toEqual([...listed].sort());
  });
});

// Research OS, Phase 8M-3, decision 424. Outside PAIRS: a score's `levels` is
// null and its suspect codes empty because that is what a score has, and the
// rule above is for fixtures that may fill every field.
describe('a prepared slot\'s view', () => {
  it('the typed copy equals the JSON Rust serialises', () => {
    expect(typed.datasetViewPrepared).toEqual(datasetPreparedJson);
  });

  it('a score is marked and named, and a file\'s column has neither key', () => {
    const [file, score] = datasetPreparedJson.profile.columns;
    expect(file).not.toHaveProperty('prepared');
    expect(file).not.toHaveProperty('scoreName');
    expect(score).toMatchObject({ prepared: true, scoreName: 'wellbeing', header: 'C2' });
  });

  it('a file\'s own view has none of the new keys', () => {
    expect(datasetJson).not.toHaveProperty('preparation');
    expect(datasetJson).not.toHaveProperty('scoreCounts');
  });

  it('a preparation holds only hashes and numbers', () => {
    expect(Object.keys(datasetPreparedJson.preparation).sort()).toEqual([
      'capabilityVersion', 'engineVersion', 'recipeHash', 'sourceColumns', 'sourceHash',
    ]);
  });
});

// Research OS, Phase 8Q, decision 459. Outside PAIRS, on the pattern above: PAIRS'
// walk fails on a null, and a null is what a missing cell is.
describe('a window of cells', () => {
  it('the typed copy equals the JSON Rust serialises', () => {
    expect(typed.cellWindow).toEqual(cellWindowJson);
  });

  it('every value is a string, and a missing cell the one null', () => {
    const flat: Array<string | null> = cellWindowJson.cells.flat();
    expect(flat.filter((c) => c === null)).toHaveLength(1);
    expect(flat.filter((c) => typeof c === 'string')).toHaveLength(flat.length - 1);
    expect(flat).toEqual(expect.arrayContaining(['41.25', 'inf', 'NaN', '-0', 'an answer']));
    expect(flat.find((c) => c?.length === 301)).toBe(`1${'0'.repeat(300)}`);
  });

  it('says whose table it is, its whole size and where it starts', () => {
    expect(Object.keys(cellWindowJson)).toEqual(['slot', 'inputHash', 'rows', 'columns', 'firstRow', 'firstColumn', 'cells']);
    expect(cellWindowJson.inputHash).toBe(datasetJson.inputRef.contentHash);
  });
});
