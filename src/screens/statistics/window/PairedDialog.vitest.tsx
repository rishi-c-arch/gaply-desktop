// The paired t-test's dialog (Research OS, Phase 8S, decisions 481 and 483; ADR-012 Part
// 6), over a mock bridge. The twins of PairedFlow.vitest.tsx's tests at :177, :199, :218
// and, added to the prompt's list since it is a refusal too, :356. Every assertion on its
// test ids and its spec is that file's. What differs:
// * toSettings becomes the dialog's opening, and pt-check its Run;
// * expectFocusOn('Check the settings') becomes waiting for the spec or the report, and a
//   line on the stage heading becomes "the dialog is still open";
// * pt-error becomes sw-dialog-refusal (:356);
// * :218 checks twice through pt-back-settings, and a dialog starts a run at its first
//   valid press, so it is written as two openings of the dialog.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockPairedBridge } from '../onesampleBridge';
import type { OneSampleRunStateView, OneSampleValidationView, PairedSpec } from '../onesampleTypes';
import PairedFlow from '../PairedFlow';
import { PairedDialog } from './PairedDialog';
import { OUTPUT_FULL } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

/** The words a verdict would use (PairedFlow.vitest.tsx:135). */
const VERDICT = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;

const view: DatasetView = {
  slot: 3,
  displayName: 'trial.csv',
  profile: {
    rows: 10,
    columns: [
      {
        alias: 'C1',
        header: 'before',
        kind: 'numeric',
        n: 10,
        missing: 0,
        distinct: 9,
        levels: null,
        suspectMissingCodes: [
          { code: -99, cells: 2 },
          { code: 999, cells: 1 },
        ],
        nonFinite: 0,
      },
      {
        alias: 'C2',
        header: 'after',
        kind: 'numeric',
        n: 10,
        missing: 0,
        distinct: 9,
        levels: null,
        suspectMissingCodes: [
          { code: -99, cells: 1 },
          { code: 77, cells: 3 },
        ],
        nonFinite: 0,
      },
      { alias: 'C3', header: 'arm', kind: 'categorical', n: 10, missing: 0, distinct: 2, levels: [{ label: 'ctrl', count: 5 }, { label: 'trt', count: 5 }], suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 10, columns: 3, semanticRole: 'table' },
};

const report = (status: OneSampleValidationView['status'], extra: Partial<OneSampleValidationView> = {}): OneSampleValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 10, columns: 2, cells: 20 },
  ...extra,
});

const running: OneSampleRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 1200, cancelRequested: false, diagnostics: [] };

function setup(responses: Record<string, unknown>, over: { outputFull?: boolean } = {}) {
  const bridge = makeMockPairedBridge({ stats_paired_run: { runId: 5 }, stats_paired_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<PairedDialog view={view} slot={3} bridge={bridge} outputFull={over.outputFull ?? false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockPairedBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as PairedSpec);
const pick = (which: 'first' | 'second', alias: string) => fireEvent.change(screen.getByTestId(`pt-${which}`), { target: { value: alias } });
const run = () => screen.getByTestId('pt-run') as HTMLButtonElement;

describe('the twins of PairedFlow.vitest.tsx', () => {
  it('offers both pickers with no default and every numeric column, says which way the difference goes, and runs once both are chosen', () => {
    setup({ stats_paired_validate: report('valid') });
    const first = screen.getByTestId('pt-first') as HTMLSelectElement;
    const second = screen.getByTestId('pt-second') as HTMLSelectElement;
    expect(first.value).toBe('');
    expect(second.value).toBe('');
    // Numeric columns only, and the second list keeps the first's choice.
    expect(Array.from(first.options).map((o) => o.value)).toEqual(['', 'C1', 'C2']);
    expect(Array.from(second.options).map((o) => o.value)).toEqual(['', 'C1', 'C2']);
    expect(screen.getByTestId('pt-difference-line').textContent).toBe('The difference is first minus second.');
    expect(run().disabled).toBe(true);
    pick('first', 'C1');
    expect(run().disabled).toBe(true);
    expect(Array.from(second.options).map((o) => o.value)).toEqual(['', 'C1', 'C2']);
    pick('second', 'C2');
    expect(run().disabled).toBe(false);
    pick('first', '');
    expect(run().disabled).toBe(true);
  });

  it('sends the same column twice, and shows Rust’s same_column refusal in its words', async () => {
    const { bridge, onStarted } = setup({
      stats_paired_validate: report('invalid', {
        // As Rust sends it (stats_onesample/mod.rs:588): no parameters.
        errors: [{ code: 'stats.onesample.same_column', params: {}, severity: 'error' }],
      }),
    });
    pick('first', 'C1');
    pick('second', 'C1');
    fireEvent.click(run());
    await screen.findByTestId('pt-validation-errors');
    expect(specs(bridge, 'stats_paired_validate')[0].parameters).toEqual({ first: 'C1', second: 'C1', missingCodes: [] });
    expect(screen.getByTestId('pt-validation-errors').textContent).toBe('The first and the second column are the same column. Choose two different columns.');
    expect(run().disabled).toBe(true);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('offers each candidate missing code of either column once, with both counts, and applies it only when ticked: not ticked', async () => {
    const { bridge } = setup({ stats_paired_validate: report('valid') });
    pick('first', 'C1');
    pick('second', 'C2');
    const fieldset = screen.getByTestId('pt-missing-codes');
    const questions = within(fieldset)
      .getAllByRole('checkbox')
      .map((b) => (b.closest('label')?.textContent ?? '').trim());
    expect(questions).toEqual([
      'Is -99 a code for a missing value? It appears in the first column "before" (2 cells) and in the second column "after" (1 cell). If you tick it, every pair in which either column holds -99 is left out.',
      'Is 999 a code for a missing value? It appears in the first column "before" (1 cell) and in the second column "after" (0 cells). If you tick it, every pair in which either column holds 999 is left out.',
      'Is 77 a code for a missing value? It appears in the first column "before" (0 cells) and in the second column "after" (3 cells). If you tick it, every pair in which either column holds 77 is left out.',
    ]);
    for (const q of questions) expect(q).not.toMatch(VERDICT);
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_paired_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_paired_validate').map((s) => s.parameters.missingCodes)).toEqual([[]]);
  });

  it('…ticked', async () => {
    const { bridge } = setup({ stats_paired_validate: report('valid') });
    pick('first', 'C1');
    pick('second', 'C2');
    fireEvent.click(screen.getByTestId('pt-code-77'));
    fireEvent.click(screen.getByTestId('pt-code--99'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_paired_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_paired_validate').map((s) => s.parameters.missingCodes)).toEqual([[77, -99]]);
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_paired_validate: () => {
        throw refusal;
      },
    });
    pick('first', 'C1');
    pick('second', 'C2');
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not of this test, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });
});

describe('with nothing chosen', () => {
  it('opens with neither column and no code, Run off', () => {
    setup({});
    expect((screen.getByTestId('pt-first') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('pt-second') as HTMLSelectElement).value).toBe('');
    expect(screen.queryByTestId('pt-missing-codes')).toBeNull();
    expect(run().disabled).toBe(true);
  });
});

describe('one set of choices gives one spec', () => {
  it("equal to the page's for the same choices", async () => {
    const { bridge, unmount } = setup({ stats_paired_validate: report('valid') });
    pick('first', 'C2');
    pick('second', 'C1');
    fireEvent.click(screen.getByTestId('pt-code-77'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_paired_validate')).toHaveLength(1));
    const fromDialog = specs(bridge, 'stats_paired_validate')[0];
    unmount();

    const page = makeMockPairedBridge({ stats_desc_open: view, stats_desc_close: true, stats_paired_validate: report('valid') });
    render(<PairedFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('pt-choose'));
    fireEvent.click(await screen.findByTestId('pt-to-settings'));
    await screen.findByTestId('pt-first');
    pick('first', 'C2');
    pick('second', 'C1');
    fireEvent.click(screen.getByTestId('pt-code-77'));
    fireEvent.click(screen.getByTestId('pt-check'));
    await waitFor(() => expect(specs(page, 'stats_paired_validate')).toHaveLength(1));
    expect({ ...fromDialog, operationId: 0 }).toEqual({ ...specs(page, 'stats_paired_validate')[0], operationId: 0 });
  });
});

describe("MethodDialog's rule, for the paired test", () => {
  const choices = () => {
    pick('first', 'C1');
    pick('second', 'C2');
  };

  it('valid: hands the run to its parent, with its own settings line', async () => {
    const { onStarted } = setup({ stats_paired_validate: report('valid') });
    choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(onStarted.mock.calls[0][0]).toEqual({ runId: 5, run: running, settings: 'First column: C1 before. Second column: C2 after. Missing codes: none.' });
  });

  it('with the Output full, Run says so and nothing is validated or run', async () => {
    const { bridge, onStarted } = setup({}, { outputFull: true });
    choices();
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe(OUTPUT_FULL);
    expect(bridge.calls).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('never closes the slot: not on mount, Cancel, Run or unmount', async () => {
    const { bridge, onStarted, unmount } = setup({ stats_paired_validate: report('valid') });
    const closes = () => bridge.calls.filter(([c]) => c === 'stats_desc_close');
    expect(closes()).toEqual([]);
    choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalled());
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('pt-dialog-cancel'));
    expect(closes()).toEqual([]);
    unmount();
    expect(closes()).toEqual([]);
  });
});

describe('one copy of the settings', () => {
  const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');

  it('the page and the dialog each render PairedSettings, by import, and neither holds a control', () => {
    for (const f of ['../PairedFlow.tsx', 'PairedDialog.tsx']) {
      expect(src(f), f).toMatch(/import \{[^}]*\bPairedSettings\b[^}]*\busePairedSettings\b[^}]*\} from '[^']*PairedSettings'/);
      expect(src(f), f).toMatch(/<PairedSettings\b/);
      expect(src(f), f).not.toMatch(/pt-\$\{which\}|pt-code-|pt-difference-line/);
    }
    expect(src('../PairedSettings.tsx')).toMatch(/pt-difference-line/);
    expect(src('MethodDialog.tsx')).not.toMatch(/pt-difference-line/);
  });

  it('its bridge type omits open and close, and it names no close', () => {
    expect(src('PairedDialog.tsx')).toMatch(/Omit<PairedBridge, 'open' \| 'close'>/);
    for (const f of ['PairedDialog.tsx', 'MethodDialog.tsx']) expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close/);
  });
});
