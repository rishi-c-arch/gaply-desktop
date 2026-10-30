// The descriptive summary's bridge (Research OS, Phase 2B): each method
// invokes the Rust command of that name with the argument keys its signature
// takes (Tauri maps camelCase to snake_case), the spec carries exactly the
// keys Rust's OperationSpec<DescribeParams> reads (it refuses any other),
// and the mock records a path as `<path>`.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockDescribeBridge, TauriDescribeBridge } from './describeBridge';
import type { OperationSpec } from './describeTypes';
import { datasetView } from './fixtures/describeFixtures';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const spec: OperationSpec = {
  schemaVersion: 1,
  operationId: 1,
  workspaceId: 'statistics',
  capabilityId: 'stats.describe',
  capabilityVersion: 1,
  inputs: [datasetView.inputRef],
  parameters: { columns: [{ alias: 'C2', treatAs: 'numeric' }], missingCodes: [99] },
  randomSeed: null,
  confirmationRefs: [],
};

describe('TauriDescribeBridge', () => {
  it('invokes each command with the argument keys the Rust signature takes', async () => {
    const b = new TauriDescribeBridge();
    await b.open('/Users/x/data.csv');
    await b.validate(3, spec);
    await b.run(3, spec);
    await b.status(4);
    await b.result(4);
    await b.cancel(4);
    await b.export(4, 'csv');
    await b.export(4, 'manifestJson');
    await b.close(3);

    const seen = invoke.mock.calls.map(([cmd, args]) => [cmd, Object.keys(args ?? {}).sort()]);
    expect(seen).toEqual([
      // Phase 8F-2 (decision 290): the open carries its progress id.
      ['stats_desc_open', ['path', 'progressId']],
      ['stats_desc_validate', ['slot', 'spec']],
      ['stats_desc_run', ['slot', 'spec']],
      ['stats_desc_status', ['runId']],
      ['stats_desc_result', ['runId']],
      ['stats_desc_cancel', ['runId']],
      ['stats_desc_export', ['format', 'runId']],
      ['stats_desc_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
    ]);
    // The export formats are the words Rust's ExportFormat reads.
    expect(invoke.mock.calls[6][1].format).toBe('csv');
    expect(invoke.mock.calls[7][1].format).toBe('manifestJson');
  });

  it('sends a spec with exactly the keys Rust reads, at every level', async () => {
    await new TauriDescribeBridge().validate(3, spec);
    const sent = invoke.mock.calls[0][1].spec as OperationSpec;
    expect(Object.keys(sent).sort()).toEqual([
      'capabilityId', 'capabilityVersion', 'confirmationRefs', 'inputs', 'operationId', 'parameters', 'randomSeed', 'schemaVersion',
      'workspaceId',
    ]);
    expect(Object.keys(sent.inputs[0]).sort()).toEqual(['columns', 'contentHash', 'rows', 'semanticRole']);
    expect(Object.keys(sent.parameters).sort()).toEqual(['columns', 'missingCodes']);
    expect(Object.keys(sent.parameters.columns[0]).sort()).toEqual(['alias', 'treatAs']);
  });
});

describe('makeMockDescribeBridge', () => {
  it('records calls, never a path, and refuses a command it was not given', async () => {
    const m = makeMockDescribeBridge({ stats_desc_open: datasetView, stats_desc_status: ({ runId }: Record<string, unknown>) => ({ runId }) });
    await expect(m.open('/Users/secret/PATH_SENTINEL/data.csv')).resolves.toEqual(datasetView);
    await expect(m.status(9)).resolves.toEqual({ runId: 9 });
    expect(JSON.stringify(m.calls)).not.toContain('PATH_SENTINEL');
    expect(m.calls[0]).toEqual(['stats_desc_open', { path: '<path>' }]);
    await expect(m.close(3)).rejects.toThrow('no mock response');
  });
});
