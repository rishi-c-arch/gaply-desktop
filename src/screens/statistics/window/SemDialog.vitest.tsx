// The structural equation model's dialog (Research OS, Phase 9C-5, decisions 814 and 815), over a
// mock bridge, on CfaDialog.vitest.tsx's pattern: it opens with every column out of the model and
// Run off; Run waits for a model the editor holds complete; a valid check starts the run with the
// spec in decision 797's order; and the started run carries decision 815's settings line. Its
// title is decision 815's. Identification is Rust's: model_not_identified shows after Run.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockSemBridge } from '../semBridge';
import type { SemRunStateView, SemSpec, SemValidationView } from '../semTypes';
import { SemDialog } from './SemDialog';
import { SEM_DIALOG_TITLE, semSettingsLine } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const numeric = (alias: string, header: string) => ({ alias, header, kind: 'numeric' as const, n: 16, missing: 0, distinct: 12, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: { rows: 16, columns: [numeric('C1', 'q1'), numeric('C2', 'q2'), numeric('C3', 'q3'), numeric('C4', 'q4'), numeric('C5', 'q5')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 5, semanticRole: 'table' },
};

const report = (status: SemValidationView['status'], extra: Partial<SemValidationView> = {}): SemValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 16, columns: 5, cells: 80 },
  ...extra,
});
const running: SemRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 12, cancelRequested: false, stepsDone: 3, stepsTotal: 9, diagnostics: [] };

function setup(responses: Record<string, unknown>, v: DatasetView = view) {
  const bridge = makeMockSemBridge({ stats_sem_run: { runId: 5 }, stats_sem_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<SemDialog view={v} slot={3} bridge={bridge} outputFull={false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockSemBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as SemSpec);
const run = () => screen.getByTestId('sm-run') as HTMLButtonElement;
const pick = (id: string, text: string) => {
  const select = screen.getByTestId(id) as HTMLSelectElement;
  const o = Array.from(select.options).find((x) => x.textContent === text);
  if (!o) throw new Error(`no option ${text} in ${id}`);
  fireEvent.change(select, { target: { value: o.value } });
};
const put = (alias: string, text: string) => pick(`sm-assign-${alias}`, text);
/** Adds a path row and chooses its ends; its id is the newest row's. */
const path = (from: string, to: string) => {
  fireEvent.click(screen.getByTestId('sm-add-path'));
  const ids = screen.getAllByTestId(/^sm-path-from-\d+$/).map((e) => Number(e.getAttribute('data-testid')?.split('-').pop()));
  const id = Math.max(...ids);
  pick(`sm-path-from-${id}`, from);
  pick(`sm-path-to-${id}`, to);
};

describe('the structural equation model dialog', () => {
  it("carries decision 815's title, and the rule above the settings", () => {
    setup({});
    expect(SEM_DIALOG_TITLE).toBe('Structural equations: Structural equation model');
    expect(screen.getByRole('dialog').textContent).toContain('Structural equations: Structural equation model');
    expect(screen.getByTestId('sm-rule').textContent).toBe(
      "A column measures one factor, or stands on its own. Factors are numbered by their first column in the file, and each factor's first column has its loading fixed at 1 to set the factor's scale, as AMOS does when a model is drawn. A variable with a path into it has an error term; every other variable has a variance. Two variables are uncorrelated unless you add their covariance. A row is used only when every column in the model has a value."
    );
    expect(screen.getByTestId('sm-rule').compareDocumentPosition(screen.getByTestId('sm-settings')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('opens with every column out of the model and Run off', () => {
    setup({});
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('sm-settings-missing').textContent).toBe('Add a factor, a path or a covariance.');
    for (const a of ['C1', 'C2', 'C3', 'C4', 'C5']) expect((screen.getByTestId(`sm-assign-${a}`) as HTMLSelectElement).selectedOptions[0].textContent).toBe('Not in the model');
  });

  it('keeps Run off until the model is complete, and turns it on at a path between two columns on their own', () => {
    setup({});
    put('C1', 'On its own');
    expect(run().disabled).toBe(true);
    put('C4', 'On its own');
    expect(run().disabled).toBe(true);
    path('"q1"', '"q4"');
    expect(run().disabled).toBe(false);
  });

  it('on a valid check starts the run with the spec in its order, and carries the settings line', async () => {
    const { bridge, onStarted } = setup({ stats_sem_validate: report('valid') });
    put('C5', 'A new factor');
    put('C2', 'Factor 1');
    put('C3', 'Factor 1');
    put('C1', 'On its own');
    path('"q1"', 'Factor 1');
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    const p = { indicators: [['C2', 'C3', 'C5']], paths: [{ to: { factor: 1 }, from: { column: 'C1' } }], covariances: [], equal: [], missingCodes: [] };
    expect(specs(bridge, 'stats_sem_run')[0].parameters).toEqual(p);
    const started = onStarted.mock.calls[0][0];
    expect(started.runId).toBe(5);
    expect(started.settings).toBe(semSettingsLine(p, view));
    expect(started.settings).toBe('Factor 1: C2 q2, C3 q3, C5 q5. On their own: C1 q1. Paths: C1 q1 → Factor 1. Missing codes: none.');
    expect(started.run).toEqual(running);
  });

  it("shows Rust's model_not_identified after Run, and starts nothing", async () => {
    const { onStarted } = setup({
      stats_sem_validate: report('invalid', { errors: [{ code: 'stats.sem.model_not_identified', params: { count: 1 }, severity: 'error' }] }),
    });
    put('C1', 'On its own');
    put('C2', 'On its own');
    path('"q1"', '"q2"');
    fireEvent.click(screen.getByTestId('sm-add-covariance'));
    pick('sm-cov-a-2', '"q1"');
    pick('sm-cov-b-2', '"q2"');
    expect(run().disabled).toBe(false);
    fireEvent.click(run());
    expect((await screen.findByTestId('sm-validation-errors')).textContent).toBe(
      'The model is not identified: it needs 1 more constraint before any data can determine its estimates, as AMOS says of such a model. Take out a path or a covariance, give a factor more columns, or hold parameters equal.'
    );
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_sem_validate: () => {
        throw refusal;
      },
    });
    put('C1', 'On its own');
    put('C2', 'On its own');
    path('"q1"', '"q2"');
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not a structural equation model, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });
});
