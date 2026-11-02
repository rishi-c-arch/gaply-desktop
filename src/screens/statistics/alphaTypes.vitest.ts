// Cronbach's alpha's wire contract, from the TypeScript side (Research OS,
// Phase 8I-2, decisions 338, 381 and 384). Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_alpha_wire.rs checks
// the bytes); here each is deep-compared with a copy typed `satisfies` the
// view's type. Also pinned: the bridge's command names and argument keys, and
// the spec the screen builds: the items and reversed items in the file's
// order, the reversed items always among the items, the codes ascending, and
// the scale's ends only when an item is reverse-scored, so one choice always
// gives one spec (decision 338).
import { beforeEach, describe, expect, it, vi } from 'vitest';

import validationJson from './fixtures/alphaValidationView.json';
import stateJson from './fixtures/alphaRunStateView.json';
import resultJson from './fixtures/alphaResultView.json';
import otherJson from './fixtures/alphaResultViewOther.json';
import * as typed from './fixtures/alphaFixtures';
import type { DatasetView } from './describeTypes';
import { TauriAlphaBridge } from './alphaBridge';
import { buildAlphaSpec } from './AlphaFlow';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['AlphaValidationView', typed.alphaValidationView, validationJson],
  ['AlphaRunStateView', typed.alphaRunStateView, stateJson],
  ['AlphaResultView', typed.alphaResultView, resultJson],
  ['AlphaResultView (the other variants)', typed.alphaResultViewOther, otherJson],
];

describe("Cronbach's alpha's view fixtures", () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps, never columns', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(Object.keys(stateJson)).toContain('stepsTotal');
    expect(JSON.stringify(stateJson)).not.toContain('columns');
  });

  it('name columns by alias only', () => {
    const aliases = JSON.stringify(resultJson.result).match(/"item":"[^"]*"/g) ?? [];
    expect(aliases.length).toBe(3);
    for (const a of aliases) expect(a).toMatch(/:"C[0-9]+"$/);
  });
});

const numeric = (alias: string, header: string) => ({
  alias,
  header,
  kind: 'numeric' as const,
  n: 10,
  missing: 0,
  distinct: 5,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
});

const VIEW: DatasetView = {
  slot: 3,
  displayName: 'scale.csv',
  profile: {
    rows: 10,
    columns: [numeric('C1', 'Calm'), numeric('C2', 'Tense'), numeric('C3', 'Rested'), numeric('C4', 'Worried')],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 10, columns: 4, semanticRole: 'table' },
};

const ENDS = { min: 1, max: 5 };

describe('the spec the screen builds (decisions 338 and 384)', () => {
  const build = (items: string[], reversed: string[] = [], codes: number[] = []) =>
    buildAlphaSpec(VIEW, items, reversed, reversed.length > 0 ? ENDS : null, codes, 1).parameters;

  it('sends the items and the reversed items in the file order, whatever order they were ticked in', () => {
    expect(build(['C3', 'C1', 'C4']).items).toEqual(['C1', 'C3', 'C4']);
    expect(build(['C4', 'C3', 'C2', 'C1'], ['C4', 'C2']).reversed).toEqual(['C2', 'C4']);
    expect(build(['C1', 'C2'], [], [99, -9, 99]).missingCodes).toEqual([-9, 99]);
  });

  it('never sends a reversed item that is not an item', () => {
    expect(build(['C1', 'C3'], ['C2', 'C3']).reversed).toEqual(['C3']);
  });

  it('gives one spec for one choice, with no scale when nothing is reverse-scored', () => {
    expect(JSON.stringify(build(['C3', 'C1'], ['C3'], [99, -9]))).toBe(JSON.stringify(build(['C1', 'C3'], ['C3'], [-9, 99])));
    const plain = build(['C2', 'C1']);
    expect(plain).toEqual({ items: ['C1', 'C2'], reversed: [], scale: null, missingCodes: [] });
    // The ends are dropped with the last reversed item, whatever was typed.
    expect(buildAlphaSpec(VIEW, ['C1', 'C2'], [], ENDS, [], 1).parameters.scale).toBeNull();
    expect(build(['C1', 'C2'], ['C2']).scale).toEqual({ min: 1, max: 5 });
  });

  it('names columns by alias, never a header', () => {
    const s = buildAlphaSpec(VIEW, ['C1', 'C3'], ['C3'], ENDS, [99], 7);
    expect(s.capabilityId).toBe('stats.analyze.cronbach_alpha');
    expect(Object.keys(s.parameters).sort()).toEqual(['items', 'missingCodes', 'reversed', 'scale']);
    for (const h of ['Calm', 'Tense', 'Rested', 'Worried']) expect(JSON.stringify(s)).not.toContain(h);
  });
});

describe('the bridge', () => {
  it("calls Cronbach's alpha's commands with the webview argument names", async () => {
    const b = new TauriAlphaBridge();
    const spec = buildAlphaSpec(VIEW, ['C1', 'C2'], [], null, [], 1);
    await b.validate(3, spec);
    await b.run(3, spec);
    await b.status(5);
    await b.result(5);
    await b.cancel(5);
    await b.export(5, 'csv');
    await b.close(3);
    expect(invoke.mock.calls.map((c) => [c[0], Object.keys(c[1] as object).sort()])).toEqual([
      ['stats_alpha_validate', ['slot', 'spec']],
      ['stats_alpha_run', ['slot', 'spec']],
      ['stats_alpha_status', ['runId']],
      ['stats_alpha_result', ['runId']],
      ['stats_alpha_cancel', ['runId']],
      ['stats_alpha_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
    ]);
  });
});
