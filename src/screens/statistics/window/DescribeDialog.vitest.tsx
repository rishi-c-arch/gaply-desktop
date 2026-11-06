// The summary's dialog (Research OS, Phase 8S, decisions 481 and 482; ADR-012 Part 6),
// over a mock bridge. The twins of DescribeFlow.vitest.tsx's settings tests at :144 and
// :164. Every assertion on its test ids and its spec is that file's. What differs:
// * toSettings becomes the dialog's opening, and desc-check its Run;
// * the page includes every column at first and the dialog none, so each twin first
//   clicks desc-include-C1, -C2 and -C3 (:144's twin, :164's twin, each listed here);
// * :164 checks three times through desc-back-settings, and a dialog starts a run at
//   its first valid press, so it is written as three openings of the dialog.
// DescribeFlow.vitest.tsx:325's refusal is the open's, and a dialog opens no file, so
// it has no twin.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import DescribeFlow from '../DescribeFlow';
import { makeMockDescribeBridge } from '../describeBridge';
import type { DatasetView, OperationSpec, RunStateView } from '../describeTypes';
import { DescribeDialog } from './DescribeDialog';
import { OUTPUT_FULL } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const opened: DatasetView = {
  slot: 1,
  displayName: 'data.csv',
  profile: {
    rows: 9,
    columns: [
      { alias: 'C1', header: 'score', kind: 'numeric', n: 8, missing: 1, distinct: 8, levels: null, suspectMissingCodes: [{ code: -99, cells: 1 }], nonFinite: 0 },
      {
        alias: 'C2',
        header: 'group',
        kind: 'categorical',
        n: 9,
        missing: 0,
        distinct: 2,
        levels: [
          { label: 'a', count: 5 },
          { label: 'b', count: 4 },
        ],
        suspectMissingCodes: [],
        nonFinite: 0,
      },
      { alias: 'C3', header: 'dose', kind: 'numeric', n: 9, missing: 0, distinct: 4, levels: null, suspectMissingCodes: [{ code: -99, cells: 2 }], nonFinite: 0 },
    ],
  },
  inputRef: { contentHash: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef', rows: 9, columns: 3, semanticRole: 'table' },
};

const running: RunStateView = { runId: 7, state: 'running', terminal: false, columnsDone: 1, columnsTotal: 3, elapsedMs: 1500, cancelRequested: false, diagnostics: [] };
const valid = { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 3, cells: 27 } };

function setup(extra: Record<string, unknown> = {}, over: { outputFull?: boolean } = {}) {
  const m = makeMockDescribeBridge({ stats_desc_validate: valid, stats_desc_run: { runId: 7 }, stats_desc_status: running, ...extra });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<DescribeDialog view={opened} slot={1} bridge={m} outputFull={over.outputFull ?? false} onStarted={onStarted} onCancel={onCancel} />);
  return { m, onStarted, onCancel, ...utils };
}

const specsSent = (m: ReturnType<typeof setup>['m']) => m.calls.filter(([c]) => c === 'stats_desc_validate').map(([, a]) => a.spec as OperationSpec);
const run = () => screen.getByTestId('desc-run') as HTMLButtonElement;
const includeAll = () => {
  for (const a of ['C1', 'C2', 'C3']) fireEvent.click(screen.getByTestId(`desc-include-${a}`));
};

describe('the twins of DescribeFlow.vitest.tsx', () => {
  it('defaults each column to its profiled kind, and sends the kinds and columns the user picks', async () => {
    const { m } = setup();
    includeAll();
    expect((screen.getByTestId('desc-kind-C1') as HTMLSelectElement).value).toBe('numeric');
    expect((screen.getByTestId('desc-kind-C2') as HTMLSelectElement).value).toBe('categorical');
    expect(screen.getByLabelText('Summarise score as')).toBeTruthy();

    fireEvent.change(screen.getByTestId('desc-kind-C1'), { target: { value: 'categorical' } });
    fireEvent.click(screen.getByTestId('desc-include-C3'));
    fireEvent.click(run());
    await screen.findByTestId('desc-validation-status');
    const sent = specsSent(m)[0];
    expect(sent.parameters.columns).toEqual([
      { alias: 'C1', treatAs: 'categorical' },
      { alias: 'C2', treatAs: 'categorical' },
    ]);
    expect(sent.inputs).toEqual([opened.inputRef]);
    expect(sent.capabilityId).toBe('stats.describe');
  });

  it('asks about a candidate missing code by its value, and applies it only when ticked: not ticked', async () => {
    const { m } = setup();
    includeAll();
    const question = screen.getByTestId('desc-missing-codes');
    expect(question.textContent).toContain('Is -99 a code for a missing value?');
    expect(question.textContent).toContain('"score" (1 cell), "dose" (2 cells)');
    expect((screen.getByTestId('desc-code--99') as HTMLInputElement).checked).toBe(false);
    fireEvent.click(run());
    await screen.findByTestId('desc-validation-status');
    expect(specsSent(m)[0].parameters.missingCodes).toEqual([]);
  });

  it('…ticked', async () => {
    const { m } = setup();
    includeAll();
    fireEvent.click(await screen.findByTestId('desc-code--99'));
    fireEvent.click(run());
    await waitFor(() => expect(specsSent(m).length).toBe(1));
    expect(specsSent(m)[0].parameters.missingCodes).toEqual([-99]);
  });

  it('…once no numeric column holds it, it is no longer asked, nor sent', async () => {
    const { m } = setup();
    includeAll();
    fireEvent.click(await screen.findByTestId('desc-code--99'));
    fireEvent.change(await screen.findByTestId('desc-kind-C1'), { target: { value: 'categorical' } });
    fireEvent.change(screen.getByTestId('desc-kind-C3'), { target: { value: 'categorical' } });
    expect(screen.queryByTestId('desc-missing-codes')).toBeNull();
    fireEvent.click(run());
    await waitFor(() => expect(specsSent(m).length).toBe(1));
    expect(specsSent(m)[0].parameters.missingCodes).toEqual([]);
  });
});

describe('the summary with nothing chosen (decision 482)', () => {
  it('opens with every column excluded, no code, Run off and the sentence saying so', () => {
    setup();
    for (const a of ['C1', 'C2', 'C3']) expect((screen.getByTestId(`desc-include-${a}`) as HTMLInputElement).checked, a).toBe(false);
    expect(screen.queryByTestId('desc-missing-codes')).toBeNull();
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('desc-settings-missing').textContent).toBe('To run the summary, include at least one column.');
  });

  it('a column the user includes starts treated as its kind says, and Run comes on', () => {
    setup();
    fireEvent.click(screen.getByTestId('desc-include-C2'));
    expect((screen.getByTestId('desc-kind-C2') as HTMLSelectElement).value).toBe('categorical');
    expect((screen.getByTestId('desc-kind-C2') as HTMLSelectElement).disabled).toBe(false);
    expect(screen.queryByTestId('desc-settings-missing')).toBeNull();
    expect(run().disabled).toBe(false);
  });

  it('shows the levels rule above its settings', () => {
    setup();
    expect(screen.getByTestId('desc-levels-rule').compareDocumentPosition(screen.getByTestId('desc-settings')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});

describe('one set of choices gives one spec', () => {
  it("equal to the page's for the same choices", async () => {
    const { m, unmount } = setup();
    for (const a of ['C1', 'C2']) fireEvent.click(screen.getByTestId(`desc-include-${a}`));
    fireEvent.click(screen.getByTestId('desc-code--99'));
    fireEvent.click(run());
    await waitFor(() => expect(specsSent(m).length).toBe(1));
    const fromDialog = specsSent(m)[0];
    unmount();

    const page = makeMockDescribeBridge({ stats_desc_open: opened, stats_desc_validate: valid, stats_desc_close: true });
    render(<DescribeFlow bridge={page} pickPath={() => Promise.resolve('/x.csv')} pollMs={1} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('desc-choose'));
    fireEvent.click(await screen.findByTestId('desc-to-settings'));
    fireEvent.click(await screen.findByTestId('desc-include-C3'));
    fireEvent.click(screen.getByTestId('desc-code--99'));
    fireEvent.click(screen.getByTestId('desc-check'));
    await screen.findByTestId('desc-validation-status');
    const fromPage = page.calls.filter(([c]) => c === 'stats_desc_validate').map(([, a]) => a.spec as OperationSpec)[0];
    expect({ ...fromDialog, operationId: 0 }).toEqual({ ...fromPage, operationId: 0 });
  });
});

describe("MethodDialog's rule, for the summary", () => {
  it('valid: hands the run to its parent, with its own settings line', async () => {
    const { m, onStarted } = setup();
    fireEvent.click(screen.getByTestId('desc-include-C1'));
    fireEvent.click(screen.getByTestId('desc-include-C2'));
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(m.calls.filter(([c]) => c === 'stats_desc_run')[0][1].slot).toBe(1);
    expect(onStarted.mock.calls[0][0]).toEqual({ runId: 7, run: running, settings: 'Columns: C1 score as numeric, C2 group as categorical. Missing codes: none.' });
  });

  it('a refused run stays open with its sentence', async () => {
    const busy = () => {
      throw new Error(JSON.stringify({ code: 'run.run_busy' }));
    };
    const { onStarted } = setup({ stats_desc_run: busy });
    fireEvent.click(screen.getByTestId('desc-include-C1'));
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).not.toBe('');
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('with the Output full, Run says so and nothing is validated or run', async () => {
    const { m, onStarted } = setup({}, { outputFull: true });
    fireEvent.click(screen.getByTestId('desc-include-C1'));
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe(OUTPUT_FULL);
    expect(m.calls).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('never closes the slot: not on mount, Cancel, Run or unmount', async () => {
    const { m, onStarted, unmount } = setup();
    const closes = () => m.calls.filter(([c]) => c === 'stats_desc_close');
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('desc-include-C1'));
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalled());
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('desc-dialog-cancel'));
    expect(closes()).toEqual([]);
    unmount();
    expect(closes()).toEqual([]);
  });
});

describe('one copy of the settings', () => {
  const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');

  it('the page and the dialog each render DescribeSettings, by import, and neither holds a control', () => {
    for (const f of ['../DescribeFlow.tsx', 'DescribeDialog.tsx']) {
      expect(src(f), f).toMatch(/import \{[^}]*\bDescribeSettings\b[^}]*\buseDescribeSettings\b[^}]*\} from '[^']*DescribeSettings'/);
      expect(src(f), f).toMatch(/<DescribeSettings\b/);
      expect(src(f), f).not.toMatch(/desc-include-|desc-kind-|desc-code-|desc-missing-codes/);
    }
    expect(src('../DescribeSettings.tsx')).toMatch(/desc-include-/);
    expect(src('MethodDialog.tsx')).not.toMatch(/desc-include-/);
  });

  it('its bridge type omits open and close, and it names no close', () => {
    const typed = src('DescribeDialog.tsx');
    expect(typed).toMatch(/Omit<DescribeBridge, 'open' \| 'close'>/);
    for (const f of ['DescribeDialog.tsx', 'MethodDialog.tsx']) expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close/);
  });

  it('the page keeps its defaults, through defaultPicks', () => {
    expect(src('../DescribeFlow.tsx')).toMatch(/useDescribeSettings\(view, defaultPicks\)/);
    expect(src('DescribeDialog.tsx')).toMatch(/useDescribeSettings\(view, noPicks\)/);
  });
});
