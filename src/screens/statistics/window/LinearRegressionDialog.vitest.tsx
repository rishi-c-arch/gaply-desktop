// The linear regression's dialog (Research OS, Phase 9A-2, decisions 684 and 685), over a mock
// bridge, on RelatedRanksDialog.vitest.tsx's pattern: it opens with nothing chosen and Run
// off, Run waits for an outcome and 1 to 50 predictors, a valid check starts the run, and the
// started run carries decision 685's settings line. Its title is decision 685's.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockLinearRegressionBridge } from '../linearRegressionBridge';
import type { LinearRegressionRunStateView, LinearRegressionSpec, LinearRegressionValidationView } from '../linearRegressionTypes';
import { LinearRegressionDialog } from './LinearRegressionDialog';
import { LINEAR_REGRESSION_DIALOG_TITLE, linearRegressionSettingsLine } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const numeric = (alias: string, header: string) => ({ alias, header, kind: 'numeric' as const, n: 16, missing: 0, distinct: 12, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: { rows: 16, columns: [numeric('C1', 'score'), numeric('C2', 'hours'), numeric('C3', 'sleep')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 3, semanticRole: 'table' },
};

const report = (status: LinearRegressionValidationView['status'], extra: Partial<LinearRegressionValidationView> = {}): LinearRegressionValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 16, columns: 3, cells: 48 },
  ...extra,
});
const running: LinearRegressionRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 12, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };

function setup(responses: Record<string, unknown>, v: DatasetView = view) {
  const bridge = makeMockLinearRegressionBridge({ stats_linear_regression_run: { runId: 5 }, stats_linear_regression_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<LinearRegressionDialog view={v} slot={3} bridge={bridge} outputFull={false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockLinearRegressionBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as LinearRegressionSpec);
const run = () => screen.getByTestId('lr-run') as HTMLButtonElement;
const outcome = (alias: string) => fireEvent.change(screen.getByTestId('lr-outcome'), { target: { value: alias } });
const tick = (alias: string) => fireEvent.click(screen.getByTestId(`lr-column-${alias}`));

describe('the linear regression dialog', () => {
  it("carries decision 685's title, and the rule above the settings", () => {
    setup({});
    expect(LINEAR_REGRESSION_DIALOG_TITLE).toBe('Regression: Linear');
    expect(screen.getByRole('dialog').textContent).toContain('Regression: Linear');
    expect(screen.getByTestId('lr-rule').textContent).toBe(
      'Every chosen predictor enters the model at once. A row is used only when the outcome and every predictor have a value.'
    );
    expect(screen.getByTestId('lr-rule').compareDocumentPosition(screen.getByTestId('lr-settings')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('opens with nothing chosen and Run off', () => {
    setup({});
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('lr-settings-missing').textContent).toBe('Choose the outcome.');
  });

  it('keeps Run off with an outcome and no predictor, and turns it on at one', () => {
    setup({});
    outcome('C1');
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('lr-settings-missing').textContent).toBe('Choose at least one predictor.');
    tick('C2');
    expect(run().disabled).toBe(false);
  });

  it('keeps Run off at 51 predictors', () => {
    const wide: DatasetView = { ...view, profile: { rows: 16, columns: Array.from({ length: 52 }, (_, i) => numeric(`C${i + 1}`, `h${i + 1}`)) } };
    setup({}, wide);
    outcome('C52');
    fireEvent.click(screen.getByTestId('lr-add-all'));
    expect(run().disabled).toBe(true);
    fireEvent.click(screen.getByTestId('lr-remove-C51'));
    expect(run().disabled).toBe(false);
  });

  it('on a valid check starts the run with the predictors in the chosen order, and carries the settings line', async () => {
    const { bridge, onStarted } = setup({ stats_linear_regression_validate: report('valid') });
    outcome('C1');
    tick('C3');
    tick('C2');
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(specs(bridge, 'stats_linear_regression_run')[0].parameters).toEqual({ outcome: 'C1', predictors: ['C3', 'C2'], missingCodes: [] });
    const started = onStarted.mock.calls[0][0];
    expect(started.runId).toBe(5);
    expect(started.settings).toBe(linearRegressionSettingsLine({ outcome: 'C1', predictors: ['C3', 'C2'], missingCodes: [] }, view));
    expect(started.settings).toBe('Outcome: C1 score. Predictors, in this order: C3 sleep, C2 hours. Missing codes: none.');
    expect(started.run).toEqual(running);
  });

  it('says the codes in the settings line, when there are any', () => {
    expect(linearRegressionSettingsLine({ outcome: 'C1', predictors: ['C2'], missingCodes: [-99, 7] }, view)).toBe(
      'Outcome: C1 score. Predictors, in this order: C2 hours. Missing codes: -99, 7.'
    );
  });

  it('shows an error from Rust in its own words, and starts nothing', async () => {
    const { onStarted } = setup({
      stats_linear_regression_validate: report('invalid', { errors: [{ code: 'stats.linear_regression.column_repeated', params: { column: 2 }, severity: 'error' }] }),
    });
    outcome('C1');
    tick('C2');
    fireEvent.click(run());
    expect((await screen.findByTestId('lr-validation-errors')).textContent).toBe(
      'Column "hours" is chosen more than once, as the outcome or among the predictors. Choose each column once.'
    );
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_linear_regression_validate: () => {
        throw refusal;
      },
    });
    outcome('C1');
    tick('C2');
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not a linear regression, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });
});
