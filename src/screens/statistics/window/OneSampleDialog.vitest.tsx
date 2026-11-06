// The one-sample t-test's dialog (Research OS, Phase 8S, decisions 481 and 483; ADR-012
// Part 6), over a mock bridge. The twins of OneSampleFlow.vitest.tsx's tests at :130,
// :154, :177 and :287. Every assertion on its test ids and its spec is that file's. What
// differs:
// * toSettings becomes the dialog's opening, and os-check its Run;
// * expectFocusOn('Check the settings') becomes waiting for the spec or the report, and a
//   line on the stage heading becomes "the dialog is still open";
// * os-error becomes sw-dialog-refusal (:287);
// * :154 and :177 check more than once through os-back-settings, and a dialog starts a
//   run at its first valid press, so each is written as separate openings of the dialog.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import OneSampleFlow, { testValueOf } from '../OneSampleFlow';
import { makeMockOneSampleBridge } from '../onesampleBridge';
import type { OneSampleRunStateView, OneSampleSpec, OneSampleValidationView } from '../onesampleTypes';
import { OneSampleDialog } from './OneSampleDialog';
import { OUTPUT_FULL } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const view: DatasetView = {
  slot: 3,
  displayName: 'trial.csv',
  profile: {
    rows: 10,
    columns: [
      { alias: 'C1', header: 'weight', kind: 'numeric', n: 10, missing: 0, distinct: 9, levels: null, suspectMissingCodes: [{ code: -99, cells: 2 }], nonFinite: 0 },
      { alias: 'C2', header: 'arm', kind: 'categorical', n: 10, missing: 0, distinct: 2, levels: [{ label: 'ctrl', count: 5 }, { label: 'trt', count: 5 }], suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 10, columns: 2, semanticRole: 'table' },
};

const report = (status: OneSampleValidationView['status'], extra: Partial<OneSampleValidationView> = {}): OneSampleValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 10, columns: 1, cells: 10 },
  ...extra,
});

const running: OneSampleRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 1200, cancelRequested: false, diagnostics: [] };

function setup(responses: Record<string, unknown>, over: { outputFull?: boolean; v?: DatasetView } = {}) {
  const bridge = makeMockOneSampleBridge({ stats_onesample_run: { runId: 5 }, stats_onesample_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<OneSampleDialog view={over.v ?? view} slot={3} bridge={bridge} outputFull={over.outputFull ?? false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockOneSampleBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as OneSampleSpec);
const run = () => screen.getByTestId('os-run') as HTMLButtonElement;
const typeValue = (text: string) => fireEvent.change(screen.getByTestId('os-test-value'), { target: { value: text } });

describe('the twins of OneSampleFlow.vitest.tsx', () => {
  it('leaves the test value empty, sends null, and says Rust’s test_value_not_chosen in its own words', async () => {
    const { bridge, onStarted } = setup({
      stats_onesample_validate: report('needs_user_input', {
        missingInformation: [{ code: 'stats.onesample.test_value_not_chosen', params: {}, severity: 'note' }],
      }),
    });
    const field = screen.getByTestId('os-test-value') as HTMLInputElement;
    expect(field.value).toBe('');
    expect(field.getAttribute('inputmode')).toBe('decimal');
    expect(screen.getByTestId('os-test-value-line').textContent).toBe('The mean difference is the mean minus the test value.');
    // Run waits for a column only, whatever the field holds.
    expect(run().disabled).toBe(true);
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    expect(run().disabled).toBe(false);
    fireEvent.click(run());
    await screen.findByTestId('os-validation-questions');
    expect(specs(bridge, 'stats_onesample_validate')[0].parameters.testValue).toBeNull();
    expect(screen.getByTestId('os-validation-questions').textContent).toBe(
      'Type the test value in Settings: the value the mean is compared with. Gaply does not choose one for you.'
    );
    expect(run().disabled).toBe(true);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it.each([
    ['abc', null],
    ['', null],
    ['   ', null],
    ['2500', 2500],
  ])('sends null for "abc" and for an empty or blank field, and the number for 2500: %j', async (text, want) => {
    const { bridge } = setup({ stats_onesample_validate: report('valid') });
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue(text);
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_onesample_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_onesample_validate')[0].parameters.testValue).toBe(want);
  });

  it('the rule itself: a finite number, or null; never a default', () => {
    expect([testValueOf(''), testValueOf(' 1.5 '), testValueOf('1e-7'), testValueOf('Infinity'), testValueOf('NaN'), testValueOf('-0.25')]).toEqual([null, 1.5, 1e-7, null, null, -0.25]);
  });

  it('asks about a candidate missing code in the column, and applies it only when ticked: not ticked', async () => {
    const { bridge } = setup({ stats_onesample_validate: report('valid') });
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('3');
    expect(screen.getByTestId('os-missing-codes').textContent).toContain(
      'Is -99 a code for a missing value? It appears in the column "weight" (2 cells). If you tick it, every row whose value is -99 is left out.'
    );
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_onesample_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_onesample_validate').map((s) => s.parameters.missingCodes)).toEqual([[]]);
  });

  it('…ticked', async () => {
    const { bridge } = setup({ stats_onesample_validate: report('valid') });
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('3');
    fireEvent.click(screen.getByTestId('os-code--99'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_onesample_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_onesample_validate').map((s) => s.parameters.missingCodes)).toEqual([[-99]]);
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_onesample_validate: () => {
        throw refusal;
      },
    });
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not of this test, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });
});

describe('with nothing chosen', () => {
  it('opens with no column and no value, Run off', () => {
    setup({});
    expect((screen.getByTestId('os-outcome') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('os-test-value') as HTMLInputElement).value).toBe('');
    expect(screen.queryByTestId('os-missing-codes')).toBeNull();
    expect(run().disabled).toBe(true);
  });

  it('a score is offered by the name the user typed', () => {
    const v: DatasetView = { ...view, profile: { ...view.profile, columns: [...view.profile.columns, { alias: 'C3', header: 'C3', scoreName: 'Total', prepared: true, kind: 'numeric', n: 10, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 }] } };
    setup({}, { v });
    expect(Array.from((screen.getByTestId('os-outcome') as HTMLSelectElement).options).find((o) => o.value === 'C3')?.textContent).toBe('C3 Total');
  });
});

describe('one set of choices gives one spec', () => {
  it("equal to the page's for the same choices", async () => {
    const { bridge, unmount } = setup({ stats_onesample_validate: report('valid') });
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('2500');
    fireEvent.click(screen.getByTestId('os-code--99'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_onesample_validate')).toHaveLength(1));
    const fromDialog = specs(bridge, 'stats_onesample_validate')[0];
    unmount();

    const page = makeMockOneSampleBridge({ stats_desc_open: view, stats_desc_close: true, stats_onesample_validate: report('valid') });
    render(<OneSampleFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('os-choose'));
    fireEvent.click(await screen.findByTestId('os-to-settings'));
    fireEvent.change(await screen.findByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('2500');
    fireEvent.click(screen.getByTestId('os-code--99'));
    fireEvent.click(screen.getByTestId('os-check'));
    await waitFor(() => expect(specs(page, 'stats_onesample_validate')).toHaveLength(1));
    expect({ ...fromDialog, operationId: 0 }).toEqual({ ...specs(page, 'stats_onesample_validate')[0], operationId: 0 });
  });
});

describe("MethodDialog's rule, for the one-sample test", () => {
  const choices = () => {
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('2500');
  };

  it('valid: hands the run to its parent, with its own settings line', async () => {
    const { onStarted } = setup({ stats_onesample_validate: report('valid') });
    choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(onStarted.mock.calls[0][0]).toEqual({ runId: 5, run: running, settings: 'Column: C1 weight. Test value: 2500. Missing codes: none.' });
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
    const { bridge, onStarted, unmount } = setup({ stats_onesample_validate: report('valid') });
    const closes = () => bridge.calls.filter(([c]) => c === 'stats_desc_close');
    expect(closes()).toEqual([]);
    choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalled());
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('os-dialog-cancel'));
    expect(closes()).toEqual([]);
    unmount();
    expect(closes()).toEqual([]);
  });
});

describe('one copy of the settings', () => {
  const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');

  it('the page and the dialog each render OneSampleSettings, by import, and neither holds a control', () => {
    for (const f of ['../OneSampleFlow.tsx', 'OneSampleDialog.tsx']) {
      expect(src(f), f).toMatch(/import \{[^}]*\bOneSampleSettings\b[^}]*\buseOneSampleSettings\b[^}]*\} from '[^']*OneSampleSettings'/);
      expect(src(f), f).toMatch(/<OneSampleSettings\b/);
      expect(src(f), f).not.toMatch(/os-outcome|"os-test-value"|os-code-/);
    }
    expect(src('../OneSampleSettings.tsx')).toMatch(/os-outcome/);
    expect(src('MethodDialog.tsx')).not.toMatch(/os-outcome/);
  });

  it('its bridge type omits open and close, and it names no close', () => {
    expect(src('OneSampleDialog.tsx')).toMatch(/Omit<OneSampleBridge, 'open' \| 'close'>/);
    for (const f of ['OneSampleDialog.tsx', 'MethodDialog.tsx']) expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close/);
  });
});
