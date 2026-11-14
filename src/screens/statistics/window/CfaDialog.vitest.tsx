// The confirmatory factor analysis's dialog (Research OS, Phase 9C-3, decisions 782 and 783),
// over a mock bridge, on FactorDialog.vitest.tsx's pattern: it opens with every column out of
// the model, the checkbox ticked and disabled, and Run off; Run waits for a factor of 3 columns
// or factors of 2 each and 3 in all; a valid check starts the run with the factors by their first
// column and each in the file's order; and the started run carries decision 783's settings line.
// Its title is decision 783's. An uncorrelated factor of two columns is Rust's to refuse.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockCfaBridge } from '../cfaBridge';
import type { CfaRunStateView, CfaSpec, CfaValidationView } from '../cfaTypes';
import { CfaDialog } from './CfaDialog';
import { CFA_DIALOG_TITLE, cfaSettingsLine } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const numeric = (alias: string, header: string) => ({ alias, header, kind: 'numeric' as const, n: 16, missing: 0, distinct: 12, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: { rows: 16, columns: [numeric('C1', 'q1'), numeric('C2', 'q2'), numeric('C3', 'q3'), numeric('C4', 'q4'), numeric('C5', 'q5')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 5, semanticRole: 'table' },
};

const report = (status: CfaValidationView['status'], extra: Partial<CfaValidationView> = {}): CfaValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 16, columns: 5, cells: 80 },
  ...extra,
});
const running: CfaRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 12, cancelRequested: false, stepsDone: 3, stepsTotal: 9, diagnostics: [] };

function setup(responses: Record<string, unknown>, v: DatasetView = view) {
  const bridge = makeMockCfaBridge({ stats_cfa_run: { runId: 5 }, stats_cfa_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<CfaDialog view={v} slot={3} bridge={bridge} outputFull={false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockCfaBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as CfaSpec);
const run = () => screen.getByTestId('cf-run') as HTMLButtonElement;
const choose = (alias: string, text: string) => {
  const select = screen.getByTestId(`cf-assign-${alias}`) as HTMLSelectElement;
  const o = Array.from(select.options).find((x) => x.textContent === text);
  if (!o) throw new Error(`no option ${text}`);
  fireEvent.change(select, { target: { value: o.value } });
};

describe('the confirmatory factor analysis dialog', () => {
  it("carries decision 783's title, and the rule above the settings", () => {
    setup({});
    expect(CFA_DIALOG_TITLE).toBe('Structural equations: Confirmatory factor analysis');
    expect(screen.getByRole('dialog').textContent).toContain('Structural equations: Confirmatory factor analysis');
    expect(screen.getByTestId('cf-rule').textContent).toBe(
      "Each column measures one factor. Factors are numbered by their first column in the file, and each factor's first column has its loading fixed at 1 to set the factor's scale, as AMOS does when a model is drawn. A row is used only when every column in the model has a value."
    );
    expect(screen.getByTestId('cf-rule').compareDocumentPosition(screen.getByTestId('cf-settings')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('opens with every column out of the model, the checkbox ticked and disabled, and Run off', () => {
    setup({});
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('cf-settings-missing').textContent).toBe('Put at least 3 columns in factors.');
    for (const a of ['C1', 'C2', 'C3', 'C4', 'C5']) expect((screen.getByTestId(`cf-assign-${a}`) as HTMLSelectElement).selectedOptions[0].textContent).toBe('Not in the model');
    const box = screen.getByTestId('cf-correlated') as HTMLInputElement;
    expect([box.checked, box.disabled]).toEqual([true, true]);
  });

  it('keeps Run off with a factor of one column, and turns it on at 3 columns in one factor', () => {
    setup({});
    choose('C1', 'A new factor');
    expect(run().disabled).toBe(true);
    choose('C2', 'Factor 1');
    expect(run().disabled).toBe(true);
    choose('C3', 'Factor 1');
    expect(run().disabled).toBe(false);
    choose('C4', 'A new factor');
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('cf-settings-missing').textContent).toBe('Factor 2 has 1 column; each factor needs at least 2.');
  });

  it("on a valid check starts the run with the factors by first column, each in the file's order, and carries the settings line", async () => {
    const { bridge, onStarted } = setup({ stats_cfa_validate: report('valid') });
    choose('C5', 'A new factor');
    choose('C2', 'Factor 1');
    choose('C4', 'A new factor');
    choose('C1', 'Factor 2');
    // C1 now begins the second factor made, so it is Factor 1; C3 joins C2 and C5 in Factor 2.
    choose('C3', 'Factor 2');
    fireEvent.click(screen.getByTestId('cf-correlated'));
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    const p = { indicators: [['C1', 'C4'], ['C2', 'C3', 'C5']], correlatedFactors: false, missingCodes: [] };
    expect(specs(bridge, 'stats_cfa_run')[0].parameters).toEqual(p);
    const started = onStarted.mock.calls[0][0];
    expect(started.runId).toBe(5);
    expect(started.settings).toBe(cfaSettingsLine(p, view));
    expect(started.settings).toBe('Factor 1: C1 q1, C4 q4. Factor 2: C2 q2, C3 q3, C5 q5. Factors correlated: no. Missing codes: none.');
    expect(started.run).toEqual(running);
  });

  it("shows Rust's refusal of an uncorrelated factor of two columns after Run, and starts nothing", async () => {
    const { onStarted, bridge } = setup({
      stats_cfa_validate: report('invalid', { errors: [{ code: 'stats.cfa.two_item_factor_uncorrelated', params: { index: 1 }, severity: 'error' }] }),
    });
    choose('C1', 'A new factor');
    choose('C2', 'Factor 1');
    choose('C3', 'A new factor');
    choose('C4', 'Factor 2');
    choose('C5', 'Factor 2');
    fireEvent.click(screen.getByTestId('cf-correlated'));
    expect(run().disabled).toBe(false);
    fireEvent.click(run());
    expect((await screen.findByTestId('cf-validation-errors')).textContent).toBe(
      'Factor 1 has two columns and is not correlated with any other factor, so no data can identify it. Tick "The factors are correlated", or give it a third column.'
    );
    expect(specs(bridge, 'stats_cfa_validate')[0].parameters.correlatedFactors).toBe(false);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_cfa_validate: () => {
        throw refusal;
      },
    });
    choose('C1', 'A new factor');
    choose('C2', 'Factor 1');
    choose('C3', 'Factor 1');
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not a confirmatory factor analysis, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });
});
