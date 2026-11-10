// Crosstabs' dialog (Research OS, Phase 8X-2, decisions 588 and 589), over a mock
// bridge, on AnovaDialog.vitest.tsx's pattern: it opens with nothing chosen and Run
// off, Run waits for two different columns, a valid check starts the run, and the
// started run carries decision 589's settings line. Its title is decision 589's.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockXtabBridge } from '../xtabBridge';
import type { XtabRunStateView, XtabSpec, XtabValidationView } from '../xtabTypes';
import { XtabDialog } from './XtabDialog';
import { XTAB_DIALOG_TITLE, xtabSettingsLine } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 16,
    columns: [
      { alias: 'C1', header: 'arm', kind: 'categorical', n: 16, missing: 0, distinct: 2, levels: null, suspectMissingCodes: [{ code: 99, cells: 2 }], nonFinite: 0 },
      { alias: 'C2', header: 'site', kind: 'categorical', n: 16, missing: 0, distinct: 4, levels: null, suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 2, semanticRole: 'table' },
};

const report = (status: XtabValidationView['status']): XtabValidationView => ({ status, errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 16, columns: 2, cells: 32 } });
const running: XtabRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 12, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };

function setup(responses: Record<string, unknown>) {
  const bridge = makeMockXtabBridge({ stats_xtab_run: { runId: 5 }, stats_xtab_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<XtabDialog view={view} slot={3} bridge={bridge} outputFull={false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockXtabBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as XtabSpec);
const run = () => screen.getByTestId('xt-run') as HTMLButtonElement;
const pick = (id: string, value: string) => fireEvent.change(screen.getByTestId(id), { target: { value } });

describe('the crosstabs dialog', () => {
  it('clears a ticked code when either column changes, so a code two columns offer starts unticked and is not sent (decision 652)', async () => {
    const shared: DatasetView = {
      ...view,
      profile: {
        rows: 16,
        columns: ['C1', 'C2', 'C3', 'C4'].map((alias) => ({ alias, header: alias, kind: 'categorical' as const, n: 16, missing: 0, distinct: 2, levels: null, suspectMissingCodes: [{ code: 99, cells: 2 }], nonFinite: 0 })),
      },
    };
    const bridge = makeMockXtabBridge({ stats_xtab_run: { runId: 5 }, stats_xtab_status: running, stats_xtab_validate: report('valid') });
    render(<XtabDialog view={shared} slot={3} bridge={bridge} outputFull={false} onStarted={vi.fn()} onCancel={vi.fn()} />);
    pick('xt-rows', 'C1');
    pick('xt-columns', 'C2');
    const box = () => screen.getByTestId('xt-code-99') as HTMLInputElement;
    fireEvent.click(box());
    expect(box().checked).toBe(true);
    pick('xt-rows', 'C3');
    expect(box().checked).toBe(false);
    fireEvent.click(box());
    expect(box().checked).toBe(true);
    pick('xt-columns', 'C4');
    expect(box().checked).toBe(false);
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_xtab_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_xtab_validate')[0].parameters.missingCodes).toEqual([]);
  });

  it("carries decision 589's title", () => {
    setup({});
    expect(XTAB_DIALOG_TITLE).toBe('Descriptive statistics: Crosstabs');
    expect(screen.getByRole('dialog').textContent).toContain('Descriptive statistics: Crosstabs');
  });

  it('opens with nothing chosen, Run off, and its levels rule shown', () => {
    setup({});
    expect((screen.getByTestId('xt-rows') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('xt-columns') as HTMLSelectElement).value).toBe('');
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('xt-levels-rule').textContent).not.toBe('');
  });

  it('keeps Run off with one column twice, and on once they differ', () => {
    setup({});
    pick('xt-rows', 'C1');
    pick('xt-columns', 'C1');
    expect(run().disabled).toBe(true);
    pick('xt-columns', 'C2');
    expect(run().disabled).toBe(false);
  });

  it('checks, runs, and hands over the run with its settings line', async () => {
    const { bridge, onStarted } = setup({ stats_xtab_validate: report('valid') });
    pick('xt-rows', 'C1');
    pick('xt-columns', 'C2');
    fireEvent.click(screen.getByTestId('xt-code-99'));
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(specs(bridge, 'stats_xtab_validate')[0].parameters).toEqual({ rows: 'C1', columns: 'C2', missingCodes: [99] });
    expect(specs(bridge, 'stats_xtab_run')).toHaveLength(1);
    expect(onStarted.mock.calls[0][0]).toEqual({ runId: 5, settings: 'Rows: C1 arm. Columns: C2 site. Missing codes: 99.', run: running });
  });

  it('does not run on an invalid check, and shows its sentence', async () => {
    const invalid: XtabValidationView = { ...report('invalid'), errors: [{ code: 'stats.xtab.too_few_levels', params: { column: 1, count: 1 }, severity: 'error' }] };
    const { bridge, onStarted } = setup({ stats_xtab_validate: invalid });
    pick('xt-rows', 'C1');
    pick('xt-columns', 'C2');
    fireEvent.click(run());
    await screen.findByTestId('xt-validation-errors');
    expect(screen.getByTestId('xt-validation-errors').textContent).toBe('Column "arm" has 1 level among the rows included, and a table needs at least two in each column.');
    expect(specs(bridge, 'stats_xtab_run')).toHaveLength(0);
    expect(onStarted).not.toHaveBeenCalled();
  });
});

describe("decision 589's settings line", () => {
  it('names both columns and the codes, none included', () => {
    expect(xtabSettingsLine({ rows: 'C1', columns: 'C2', missingCodes: [] }, view)).toBe('Rows: C1 arm. Columns: C2 site. Missing codes: none.');
    expect(xtabSettingsLine({ rows: 'C2', columns: 'C1', missingCodes: [-99, 99] }, view)).toBe('Rows: C2 site. Columns: C1 arm. Missing codes: -99, 99.');
  });
});
