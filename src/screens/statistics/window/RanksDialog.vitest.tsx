// The rank tests' dialog (Research OS, Phase 8Y-2, decisions 619 and 620), over a mock
// bridge, on XtabDialog.vitest.tsx's pattern: it opens with nothing chosen and Run off,
// Run waits for an outcome, a group and two levels, a valid check starts the run, and
// the started run carries decision 620's settings line. Its title is decision 620's.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import { makeMockRanksBridge } from '../ranksBridge';
import type { RanksRunStateView, RanksSpec, RanksValidationView } from '../ranksTypes';
import { RanksDialog } from './RanksDialog';
import { RANKS_DIALOG_TITLE, ranksSettingsLine } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 16,
    columns: [
      { alias: 'C1', header: 'score', kind: 'numeric', n: 16, missing: 0, distinct: 12, levels: null, suspectMissingCodes: [{ code: 99, cells: 2 }], nonFinite: 0 },
      { alias: 'C2', header: 'arm', kind: 'categorical', n: 16, missing: 0, distinct: 4, levels: null, suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 2, semanticRole: 'table' },
};

const LEVELS = [
  { index: 0, label: 'a', count: 4 },
  { index: 1, label: 'b', count: 4 },
  { index: 2, label: 'c', count: 4 },
  { index: 3, label: 'd', count: 4 },
];

const report = (status: RanksValidationView['status']): RanksValidationView => ({ status, errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 16, columns: 2, cells: 32 } });
const running: RanksRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 12, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };

function setup(responses: Record<string, unknown>) {
  const bridge = makeMockRanksBridge({ stats_ranks_group_levels: { slot: 3, column: 'C2', levels: LEVELS }, stats_ranks_run: { runId: 5 }, stats_ranks_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<RanksDialog view={view} slot={3} bridge={bridge} outputFull={false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockRanksBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as RanksSpec);
const run = () => screen.getByTestId('rk-run') as HTMLButtonElement;
const pick = (id: string, value: string) => fireEvent.change(screen.getByTestId(id), { target: { value } });
const chooseAll = async () => {
  pick('rk-outcome', 'C1');
  pick('rk-group', 'C2');
  await screen.findByTestId('rk-levels');
};

describe('the rank tests dialog', () => {
  it("carries decision 620's title", () => {
    setup({});
    expect(RANKS_DIALOG_TITLE).toBe('Nonparametric tests: Independent samples');
    expect(screen.getByRole('dialog').textContent).toContain('Nonparametric tests: Independent samples');
  });

  it('opens with nothing chosen, Run off, and its levels rule shown', () => {
    setup({});
    expect((screen.getByTestId('rk-outcome') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('rk-group') as HTMLSelectElement).value).toBe('');
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('rk-levels-rule').textContent).toMatch(/10 before 2/);
  });

  it('keeps Run off with one level ticked, and on with two', async () => {
    setup({});
    await chooseAll();
    expect(run().disabled).toBe(false);
    for (const i of [1, 2]) fireEvent.click(screen.getByTestId(`rk-level-${i}`));
    expect(run().disabled).toBe(false);
    fireEvent.click(screen.getByTestId('rk-level-3'));
    expect(run().disabled).toBe(true);
  });

  it('checks, runs, and hands over the run with its settings line, the levels by label', async () => {
    const { bridge, onStarted } = setup({ stats_ranks_validate: report('valid') });
    await chooseAll();
    fireEvent.click(screen.getByTestId('rk-level-1'));
    fireEvent.click(screen.getByTestId('rk-level-3'));
    fireEvent.click(screen.getByTestId('rk-code-99'));
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(specs(bridge, 'stats_ranks_validate')[0].parameters).toEqual({ outcome: 'C1', group: 'C2', levels: [0, 2], missingCodes: [99] });
    expect(specs(bridge, 'stats_ranks_run')).toHaveLength(1);
    expect(onStarted.mock.calls[0][0]).toEqual({ runId: 5, settings: 'Outcome: C1 score. Group: C2 arm. Levels: a, c. Missing codes: 99.', run: running });
  });

  it('does not run on an invalid check, and shows its sentence, the level by its label', async () => {
    const invalid: RanksValidationView = { ...report('invalid'), errors: [{ code: 'stats.ranks.group_empty', params: { index: 1 }, severity: 'error' }] };
    const { bridge, onStarted } = setup({ stats_ranks_validate: invalid });
    await chooseAll();
    fireEvent.click(run());
    await screen.findByTestId('rk-validation-errors');
    expect(screen.getByTestId('rk-validation-errors').textContent).toBe('The group "b" has no value after the exclusions. Leave it out, or choose another group.');
    expect(specs(bridge, 'stats_ranks_run')).toHaveLength(0);
    expect(onStarted).not.toHaveBeenCalled();
  });
});

describe("decision 620's settings line", () => {
  it('names both columns, all the levels and no codes', () => {
    expect(ranksSettingsLine({ outcome: 'C1', group: 'C2', levels: null, missingCodes: [] }, view, LEVELS)).toBe('Outcome: C1 score. Group: C2 arm. Levels: all. Missing codes: none.');
  });

  it('names the ticked levels, and a level it cannot find by its position', () => {
    expect(ranksSettingsLine({ outcome: 'C1', group: 'C2', levels: [1, 3], missingCodes: [-99, 99] }, view, LEVELS)).toBe('Outcome: C1 score. Group: C2 arm. Levels: b, d. Missing codes: -99, 99.');
    expect(ranksSettingsLine({ outcome: 'C1', group: 'C2', levels: [7], missingCodes: [] }, view, LEVELS)).toBe('Outcome: C1 score. Group: C2 arm. Levels: level 8. Missing codes: none.');
  });
});
