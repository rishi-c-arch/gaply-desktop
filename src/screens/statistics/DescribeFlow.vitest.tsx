// The descriptive summary's screen (Research OS, Phase 2B), over a mock
// bridge: the stages and where focus goes, the column kind picker, the
// candidate missing-code question, Cancel, a neutral "Could not compute",
// the read-only manifest panel, and Export through a mocked save dialog
// writing exactly the bytes Rust returned.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import DescribeFlow from './DescribeFlow';
import { makeMockDescribeBridge } from './describeBridge';
import type { DatasetView, OperationSpec, RunStateView } from './describeTypes';
import { resultBundleView } from './fixtures/describeFixtures';

const save = vi.fn();
const writeTextFile = vi.fn();
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...a: unknown[]) => save(...a) }));
vi.mock('@tauri-apps/plugin-fs', () => ({ writeTextFile: (...a: unknown[]) => writeTextFile(...a) }));

afterEach(cleanup);
beforeEach(() => {
  save.mockReset();
  writeTextFile.mockReset();
});

const PATH = '/Users/someone/PATH_SENTINEL_q7/data.csv';

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

const state = (s: Partial<RunStateView>): RunStateView => ({
  runId: 7,
  state: 'running',
  terminal: false,
  columnsDone: 1,
  columnsTotal: 3,
  elapsedMs: 1500,
  cancelRequested: false,
  diagnostics: [],
  ...s,
});

const valid = { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 3, cells: 27 } };

function setup(extra: Record<string, unknown> = {}, props: Partial<React.ComponentProps<typeof DescribeFlow>> = {}) {
  let polls = 0;
  const m = makeMockDescribeBridge({
    stats_desc_open: opened,
    stats_desc_validate: valid,
    stats_desc_run: { runId: 7 },
    stats_desc_status: () => (++polls < 3 ? state({}) : state({ state: 'succeeded', terminal: true, columnsDone: 3 })),
    stats_desc_result: resultBundleView,
    stats_desc_close: true,
    ...extra,
  });
  const view = render(<DescribeFlow bridge={m} pickPath={() => Promise.resolve(PATH)} pollMs={1} {...props} />);
  return { m, view };
}

const heading = () => screen.getByTestId('desc-stage-heading');
const specsSent = (m: ReturnType<typeof setup>['m']) =>
  m.calls.filter(([c]) => c === 'stats_desc_validate').map(([, a]) => a.spec as OperationSpec);

async function toSettings() {
  fireEvent.click(screen.getByTestId('desc-choose'));
  await screen.findByTestId('desc-profile');
  fireEvent.click(screen.getByTestId('desc-to-settings'));
  await screen.findByTestId('desc-settings');
}

describe('DescribeFlow', () => {
  it('moves Import → Profile → Settings → Validate → Run → Results → Export, focusing each heading', async () => {
    const { m, view } = setup();
    expect(heading().textContent).toBe('Import a data file');
    const current = () => screen.getByTestId('desc-steps').querySelector('[aria-current="step"]')?.textContent;
    expect(current()).toBe('Import');

    fireEvent.click(screen.getByTestId('desc-choose'));
    await waitFor(() => expect(heading().textContent).toBe('Profile of the file'));
    expect(document.activeElement).toBe(heading());
    expect(current()).toBe('Profile');
    expect(screen.getByTestId('desc-levels-rule').textContent).toContain('1 and 1.0 are one level');
    // The path went to Rust once and is nowhere on the screen.
    expect(view.container.innerHTML).not.toContain('PATH_SENTINEL');
    expect(screen.getByTestId('desc-file-summary').textContent).toContain('data.csv');

    for (const [button, title, step] of [
      ['desc-to-settings', 'Choose the columns to summarise', 'Settings'],
      ['desc-check', 'Check the settings', 'Validate'],
      ['desc-run', 'Run the summary', 'Run'],
    ]) {
      fireEvent.click(screen.getByTestId(button));
      await waitFor(() => expect(heading().textContent).toBe(title));
      expect(document.activeElement).toBe(heading());
      expect(current()).toBe(step);
    }
    await waitFor(() => expect(screen.getByTestId('desc-run-state').textContent).toBe('Finished'));
    fireEvent.click(screen.getByTestId('desc-see-results'));
    await waitFor(() => expect(heading().textContent).toBe('Results'));
    expect(document.activeElement).toBe(heading());
    fireEvent.click(screen.getByTestId('desc-to-export'));
    await waitFor(() => expect(heading().textContent).toBe('Export'));
    expect(document.activeElement).toBe(heading());
    expect(m.calls.map(([c]) => c)).toEqual(expect.arrayContaining(['stats_desc_open', 'stats_desc_validate', 'stats_desc_run', 'stats_desc_result']));
  });

  it('defaults each column to its profiled kind, and sends the kinds and columns the user picks', async () => {
    const { m } = setup();
    await toSettings();
    expect((screen.getByTestId('desc-kind-C1') as HTMLSelectElement).value).toBe('numeric');
    expect((screen.getByTestId('desc-kind-C2') as HTMLSelectElement).value).toBe('categorical');
    expect(screen.getByLabelText('Summarise score as')).toBeTruthy();

    fireEvent.change(screen.getByTestId('desc-kind-C1'), { target: { value: 'categorical' } });
    fireEvent.click(screen.getByTestId('desc-include-C3'));
    fireEvent.click(screen.getByTestId('desc-check'));
    await screen.findByTestId('desc-validation-status');
    const sent = specsSent(m)[0];
    expect(sent.parameters.columns).toEqual([
      { alias: 'C1', treatAs: 'categorical' },
      { alias: 'C2', treatAs: 'categorical' },
    ]);
    expect(sent.inputs).toEqual([opened.inputRef]);
    expect(sent.capabilityId).toBe('stats.describe');
  });

  it('asks about a candidate missing code by its value, and applies it only when ticked', async () => {
    const { m } = setup();
    await toSettings();
    const question = screen.getByTestId('desc-missing-codes');
    expect(question.textContent).toContain('Is -99 a code for a missing value?');
    expect(question.textContent).toContain('"score" (1 cell), "dose" (2 cells)');
    expect((screen.getByTestId('desc-code--99') as HTMLInputElement).checked).toBe(false);

    fireEvent.click(screen.getByTestId('desc-check'));
    await screen.findByTestId('desc-validation-status');
    expect(specsSent(m)[0].parameters.missingCodes).toEqual([]);

    fireEvent.click(screen.getByTestId('desc-back-settings'));
    fireEvent.click(await screen.findByTestId('desc-code--99'));
    fireEvent.click(screen.getByTestId('desc-check'));
    await waitFor(() => expect(specsSent(m).length).toBe(2));
    expect(specsSent(m)[1].parameters.missingCodes).toEqual([-99]);

    // Once no numeric column holds it, it is no longer asked, nor sent.
    fireEvent.click(screen.getByTestId('desc-back-settings'));
    fireEvent.change(await screen.findByTestId('desc-kind-C1'), { target: { value: 'categorical' } });
    fireEvent.change(screen.getByTestId('desc-kind-C3'), { target: { value: 'categorical' } });
    expect(screen.queryByTestId('desc-missing-codes')).toBeNull();
    fireEvent.click(screen.getByTestId('desc-check'));
    await waitFor(() => expect(specsSent(m).length).toBe(3));
    expect(specsSent(m)[2].parameters.missingCodes).toEqual([]);
  });

  it('shows progress, elapsed time and Cancel while running, then the terminal state', async () => {
    let cancelled = false;
    const { m } = setup({
      stats_desc_status: () => (cancelled ? state({ state: 'cancelled', terminal: true, cancelRequested: true }) : state({})),
      stats_desc_cancel: () => {
        cancelled = true;
        return state({ cancelRequested: true });
      },
    });
    await toSettings();
    fireEvent.click(screen.getByTestId('desc-check'));
    fireEvent.click(await screen.findByTestId('desc-run'));
    await screen.findByTestId('desc-run-status');
    expect(screen.getByTestId('desc-run-state').textContent).toBe('Running');
    expect(screen.getByTestId('desc-progress').textContent).toBe('1 of 3 columns summarised.');
    expect(screen.getByTestId('desc-elapsed').textContent).toBe('Elapsed: 1.5 s');

    fireEvent.click(screen.getByTestId('desc-cancel'));
    await waitFor(() => expect(screen.getByTestId('desc-run-state').textContent).toBe('Cancelled'));
    expect(m.calls.some(([c]) => c === 'stats_desc_cancel')).toBe(true);
    expect(screen.getByTestId('desc-run-ended').textContent).toBe('This run was cancelled, so it has no result.');
    expect(screen.queryByTestId('desc-cancel')).toBeNull();
    expect(screen.queryByTestId('desc-see-results')).toBeNull();
    expect(m.calls.some(([c]) => c === 'stats_desc_result')).toBe(false);
  });

  it('shows "Could not compute" as a neutral cell with its reason', async () => {
    setup();
    await toSettings();
    fireEvent.click(screen.getByTestId('desc-check'));
    fireEvent.click(await screen.findByTestId('desc-run'));
    fireEvent.click(await screen.findByTestId('desc-see-results'));
    const cell = await screen.findByTestId('desc-not-computable');
    expect(cell.textContent).toContain('Could not compute');
    expect(cell.textContent).toContain('The arithmetic went past the largest number a double can hold.');
    expect(cell.getAttribute('data-tone')).toBe('neutral');
    expect(cell.getAttribute('style')).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
    // The rest of the row is numbers, and nothing scores the result.
    const table = screen.getByTestId('desc-numeric-table');
    expect(within(table).getByText('52.5')).toBeTruthy();
    expect(table.textContent).not.toMatch(/score|pass|fail|%/i);
    expect(screen.getByTestId('desc-levels-table').textContent).toContain('a');
  });

  it('shows a read-only manifest: shortened hash, versions, rows, platform, reproducibility', async () => {
    setup();
    await toSettings();
    fireEvent.click(screen.getByTestId('desc-check'));
    fireEvent.click(await screen.findByTestId('desc-run'));
    fireEvent.click(await screen.findByTestId('desc-see-results'));
    const panel = await screen.findByTestId('desc-manifest');
    expect(screen.getByTestId('desc-manifest-input').textContent).toBe('0123456789ab…');
    expect(screen.getByTestId('desc-manifest-capability').textContent).toBe('stats.describe, version 1');
    expect(screen.getByTestId('desc-manifest-engine').textContent).toBe('1');
    expect(screen.getByTestId('desc-manifest-rows-read').textContent).toBe('9');
    expect(screen.getByTestId('desc-manifest-rows-used').textContent).toBe('9');
    expect(screen.getByTestId('desc-manifest-platform').textContent).toBe('macOS, aarch64');
    expect(screen.getByTestId('desc-manifest-reproducibility').textContent).toContain('Bitwise');
    expect(within(panel).queryAllByRole('button')).toHaveLength(0);
    expect(within(panel).queryAllByRole('textbox')).toHaveLength(0);
  });

  it('exports through the save dialog, writing exactly the bytes Rust returned, and keeps no path', async () => {
    const csv = '"column","level"\r\n"dose","é"\r\n';
    const manifest = '{"schemaVersion":1}';
    const bytes = (t: string) => Array.from(new TextEncoder().encode(t));
    const { view } = setup({
      stats_desc_export: ({ format }: Record<string, unknown>) => ({ bytes: bytes(format === 'csv' ? csv : manifest), sha256: 'ab'.repeat(32) }),
    });
    await toSettings();
    fireEvent.click(screen.getByTestId('desc-check'));
    fireEvent.click(await screen.findByTestId('desc-run'));
    fireEvent.click(await screen.findByTestId('desc-see-results'));
    fireEvent.click(await screen.findByTestId('desc-to-export'));

    save.mockResolvedValue('/Users/someone/SAVED_SENTINEL/out.csv');
    writeTextFile.mockResolvedValue(undefined);
    fireEvent.click(screen.getByTestId('desc-save'));
    await waitFor(() => expect(writeTextFile).toHaveBeenCalledTimes(1));
    expect(save).toHaveBeenCalledWith({ defaultPath: 'descriptive_summary.csv' });
    expect(writeTextFile).toHaveBeenCalledWith('/Users/someone/SAVED_SENTINEL/out.csv', csv);
    expect((await screen.findByTestId('desc-export-note')).textContent).toBe('Saved. SHA-256 of the file: abababababab…');
    expect(view.container.innerHTML).not.toContain('SAVED_SENTINEL');

    fireEvent.click(screen.getByTestId('desc-export-manifestJson'));
    save.mockResolvedValue('/Users/someone/m.json');
    fireEvent.click(screen.getByTestId('desc-save'));
    await waitFor(() => expect(writeTextFile).toHaveBeenCalledTimes(2));
    expect(save).toHaveBeenLastCalledWith({ defaultPath: 'descriptive_summary_manifest.json' });
    expect(writeTextFile).toHaveBeenLastCalledWith('/Users/someone/m.json', manifest);

    save.mockResolvedValue(null);
    fireEvent.click(screen.getByTestId('desc-save'));
    await waitFor(() => expect(screen.getByTestId('desc-export-note').textContent).toBe('Not saved: the save dialog was closed.'));
    expect(writeTextFile).toHaveBeenCalledTimes(2);
  });

  it('closes the file in Rust when the screen goes', async () => {
    const { m, view } = setup();
    fireEvent.click(screen.getByTestId('desc-choose'));
    await screen.findByTestId('desc-profile');
    view.unmount();
    await waitFor(() => expect(m.calls.some(([c, a]) => c === 'stats_desc_close' && a.slot === 1)).toBe(true));
  });

  it('says a refusal in plain words', async () => {
    setup({ stats_desc_open: () => { throw new Error(JSON.stringify({ code: 'file_too_large', limit_rows: 5000 })); } });
    fireEvent.click(screen.getByTestId('desc-choose'));
    expect((await screen.findByTestId('desc-error')).textContent).toContain('5000');
    expect(heading().textContent).toBe('Import a data file');
  });
});
