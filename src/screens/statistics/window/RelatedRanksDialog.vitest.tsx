// The related rank tests' dialog (Research OS, Phase 8Z-2, decisions 653 and 654), over a
// mock bridge, on RanksDialog.vitest.tsx's pattern: it opens with nothing chosen and Run
// off, Run waits for two to 50 columns, a valid check starts the run, and the started
// run carries decision 654's settings line. Its title is decision 654's.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockRelatedRanksBridge } from '../relatedRanksBridge';
import type { RelatedRanksRunStateView, RelatedRanksSpec, RelatedRanksValidationView } from '../relatedRanksTypes';
import { RelatedRanksDialog } from './RelatedRanksDialog';
import { RELATED_RANKS_DIALOG_TITLE, relatedRanksSettingsLine } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const numeric = (alias: string, header: string) => ({ alias, header, kind: 'numeric' as const, n: 16, missing: 0, distinct: 12, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: { rows: 16, columns: [numeric('C1', 'before'), numeric('C2', 'after'), numeric('C3', 'later')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 3, semanticRole: 'table' },
};

const report = (status: RelatedRanksValidationView['status'], extra: Partial<RelatedRanksValidationView> = {}): RelatedRanksValidationView => ({ status, errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 16, columns: 2, cells: 32 }, ...extra });
const running: RelatedRanksRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 12, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };

function setup(responses: Record<string, unknown>, v: DatasetView = view) {
  const bridge = makeMockRelatedRanksBridge({ stats_related_ranks_run: { runId: 5 }, stats_related_ranks_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<RelatedRanksDialog view={v} slot={3} bridge={bridge} outputFull={false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockRelatedRanksBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as RelatedRanksSpec);
const run = () => screen.getByTestId('rr-run') as HTMLButtonElement;
const tick = (alias: string) => fireEvent.click(screen.getByTestId(`rr-column-${alias}`));

describe('the related rank tests dialog', () => {
  it("carries decision 654's title, and the rule above the settings", () => {
    setup({});
    expect(RELATED_RANKS_DIALOG_TITLE).toBe('Nonparametric tests: Related samples');
    expect(screen.getByRole('dialog').textContent).toContain('Nonparametric tests: Related samples');
    expect(screen.getByTestId('rr-rule').textContent).toBe('Two columns give the Wilcoxon signed-rank test; three or more give the Friedman test. A row is used only when every chosen column has a value.');
    // The rule comes before the settings.
    expect(screen.getByTestId('rr-rule').compareDocumentPosition(screen.getByTestId('rr-settings')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('opens with nothing chosen and Run off', () => {
    setup({});
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('rr-settings-missing').textContent).toBe('Choose at least two columns.');
  });

  it('keeps Run off at one column and turns it on at two', () => {
    setup({});
    tick('C1');
    expect(run().disabled).toBe(true);
    tick('C2');
    expect(run().disabled).toBe(false);
  });

  it('keeps Run off at 51 columns', () => {
    const wide: DatasetView = { ...view, profile: { rows: 16, columns: Array.from({ length: 51 }, (_, i) => numeric(`C${i + 1}`, `h${i + 1}`)) } };
    setup({}, wide);
    fireEvent.click(screen.getByTestId('rr-add-all'));
    expect(run().disabled).toBe(true);
    fireEvent.click(screen.getByTestId('rr-remove-C51'));
    expect(run().disabled).toBe(false);
  });

  it('on a valid check starts the run in the chosen order, and carries the settings line', async () => {
    const { bridge, onStarted } = setup({ stats_related_ranks_validate: report('valid') });
    tick('C2');
    tick('C1');
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(specs(bridge, 'stats_related_ranks_run')[0].parameters).toEqual({ columns: ['C2', 'C1'], missingCodes: [] });
    const started = onStarted.mock.calls[0][0];
    expect(started.runId).toBe(5);
    expect(started.settings).toBe(relatedRanksSettingsLine({ columns: ['C2', 'C1'], missingCodes: [] }, view));
    expect(started.settings).toBe('Columns, in this order: C2 after, C1 before. Missing codes: none.');
    expect(started.run).toEqual(running);
  });

  it('says the codes in the settings line, when there are any', () => {
    expect(relatedRanksSettingsLine({ columns: ['C1', 'C2'], missingCodes: [-99, 7] }, view)).toBe('Columns, in this order: C1 before, C2 after. Missing codes: -99, 7.');
  });

  it("shows an error from Rust in its own words, and starts nothing", async () => {
    const { onStarted } = setup({
      stats_related_ranks_validate: report('invalid', { errors: [{ code: 'stats.related_ranks.column_repeated', params: { column: 2 }, severity: 'error' }] }),
    });
    tick('C1');
    tick('C2');
    fireEvent.click(run());
    expect((await screen.findByTestId('rr-validation-errors')).textContent).toBe('Column "after" is chosen more than once. Choose each column once.');
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_related_ranks_validate: () => {
        throw refusal;
      },
    });
    tick('C1');
    tick('C2');
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not a related-samples rank test, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });
});
