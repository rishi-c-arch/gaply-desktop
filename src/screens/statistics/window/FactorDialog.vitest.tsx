// The factor analysis's dialog (Research OS, Phase 9B-3, decisions 729 and 730), over a mock
// bridge, on LinearRegressionDialog.vitest.tsx's pattern: it opens with nothing chosen, its
// defaults set and Run off; Run waits for 3 to 128 items; a valid check starts the run with the
// items in the file's order; and the started run carries decision 730's settings line. Its title
// is decision 730's.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockFactorBridge } from '../factorBridge';
import type { FactorRunStateView, FactorSpec, FactorValidationView } from '../factorTypes';
import { FactorDialog } from './FactorDialog';
import { FACTOR_DIALOG_TITLE, factorSettingsLine } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const numeric = (alias: string, header: string) => ({ alias, header, kind: 'numeric' as const, n: 16, missing: 0, distinct: 12, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: { rows: 16, columns: [numeric('C1', 'q1'), numeric('C2', 'q2'), numeric('C3', 'q3'), numeric('C4', 'q4')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 4, semanticRole: 'table' },
};

const report = (status: FactorValidationView['status'], extra: Partial<FactorValidationView> = {}): FactorValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 16, columns: 4, cells: 64 },
  ...extra,
});
const running: FactorRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 12, cancelRequested: false, stepsDone: 3, stepsTotal: 12, diagnostics: [] };

function setup(responses: Record<string, unknown>, v: DatasetView = view) {
  const bridge = makeMockFactorBridge({ stats_factor_run: { runId: 5 }, stats_factor_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<FactorDialog view={v} slot={3} bridge={bridge} outputFull={false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockFactorBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as FactorSpec);
const run = () => screen.getByTestId('fa-run') as HTMLButtonElement;
const tick = (alias: string) => fireEvent.click(screen.getByTestId(`fa-column-${alias}`));

describe('the factor analysis dialog', () => {
  it("carries decision 730's title, and the rule above the settings", () => {
    setup({});
    expect(FACTOR_DIALOG_TITLE).toBe('Dimension reduction: Factor');
    expect(screen.getByRole('dialog').textContent).toContain('Dimension reduction: Factor');
    expect(screen.getByTestId('fa-rule').textContent).toBe("Every chosen item enters the analysis at once, in the file's order. A row is used only when every item has a value.");
    expect(screen.getByTestId('fa-rule').compareDocumentPosition(screen.getByTestId('fa-settings')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('opens with nothing chosen, its defaults set and Run off', () => {
    setup({});
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('fa-settings-missing').textContent).toBe('Choose at least 3 items.');
    expect((screen.getByTestId('fa-extraction-principal_components') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('fa-factors-kaiser') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('fa-rotation-none') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('fa-iterations') as HTMLInputElement).value).toBe('25');
    expect((screen.getByTestId('fa-convergence') as HTMLInputElement).value).toBe('0.001');
  });

  it('keeps Run off at 2 items and turns it on at 3', () => {
    setup({});
    tick('C1');
    tick('C2');
    expect(run().disabled).toBe(true);
    tick('C3');
    expect(run().disabled).toBe(false);
  });

  it('keeps Run off at 129 items and turns it on at 128', () => {
    const wide: DatasetView = { ...view, profile: { rows: 16, columns: Array.from({ length: 129 }, (_, i) => numeric(`C${i + 1}`, `h${i + 1}`)) } };
    setup({}, wide);
    fireEvent.click(screen.getByTestId('fa-add-all'));
    expect(run().disabled).toBe(true);
    fireEvent.click(screen.getByTestId('fa-remove-C129'));
    expect(run().disabled).toBe(false);
  });

  it("on a valid check starts the run with the items in the file's order, and carries the settings line", async () => {
    const { bridge, onStarted } = setup({ stats_factor_validate: report('valid') });
    tick('C4');
    tick('C1');
    tick('C3');
    fireEvent.click(screen.getByTestId('fa-extraction-principal_axis'));
    fireEvent.click(screen.getByTestId('fa-factors-fixed'));
    fireEvent.change(screen.getByTestId('fa-factors-number'), { target: { value: '2' } });
    fireEvent.click(screen.getByTestId('fa-rotation-promax'));
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    const p = { items: ['C1', 'C3', 'C4'], extraction: 'principal_axis' as const, factors: 2, rotation: 'promax' as const, iterations: 25, convergence: 0.001, missingCodes: [] };
    expect(specs(bridge, 'stats_factor_run')[0].parameters).toEqual(p);
    const started = onStarted.mock.calls[0][0];
    expect(started.runId).toBe(5);
    expect(started.settings).toBe(factorSettingsLine(p, view));
    expect(started.settings).toBe(
      'Items: C1 q1, C3 q3, C4 q4. Extraction: principal axis factoring. Factors: 2. Rotation: promax. Iterations: 25. Convergence: 0.001. Missing codes: none.'
    );
    expect(started.run).toEqual(running);
  });

  it("says Kaiser's rule and the codes in the settings line", () => {
    expect(
      factorSettingsLine({ items: ['C1', 'C2', 'C3'], extraction: 'principal_components', factors: null, rotation: 'none', iterations: 25, convergence: 0.001, missingCodes: [-99, 7] }, view)
    ).toBe('Items: C1 q1, C2 q2, C3 q3. Extraction: principal components. Factors: every eigenvalue above 1. Rotation: none. Iterations: 25. Convergence: 0.001. Missing codes: -99, 7.');
  });

  it('shows an error from Rust in its own words, and starts nothing', async () => {
    const { onStarted } = setup({
      stats_factor_validate: report('invalid', { errors: [{ code: 'stats.factor.item_repeated', params: { column: 2 }, severity: 'error' }] }),
    });
    tick('C1');
    tick('C2');
    tick('C3');
    fireEvent.click(run());
    expect((await screen.findByTestId('fa-validation-errors')).textContent).toBe('Column "q2" is chosen twice as an item.');
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_factor_validate: () => {
        throw refusal;
      },
    });
    tick('C1');
    tick('C2');
    tick('C3');
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not a factor analysis, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });
});
