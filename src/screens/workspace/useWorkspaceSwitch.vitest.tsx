// The one switch (Research OS, Phase 8P, decisions 449, 450 and 454): the
// workspace page and the Statistics window both reach it by import, neither
// holds a copy, and a Switch from either drops the window's file.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { WorkspaceShell } from './WorkspaceShell';
import { defaultRunBridges, jobSentence, runKinds } from './useWorkspaceSwitch';
import { makeMockXtabBridge } from '../statistics/xtabBridge';
import { makeMockRanksBridge } from '../statistics/ranksBridge';
import { makeMockRelatedRanksBridge } from '../statistics/relatedRanksBridge';
import { makeMockLinearRegressionBridge } from '../statistics/linearRegressionBridge';
import { makeMockFactorBridge } from '../statistics/factorBridge';
import { makeMockWorkspaceBridge } from './workspaceBridge';
import { makeMockDescribeBridge } from '../statistics/describeBridge';
import type { DatasetView, RunStateView } from '../statistics/describeTypes';
import { statisticsSession } from '../statistics/window/statisticsSession';
import type { WorkspaceStateView } from './workspaceTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
});

const view: DatasetView = {
  slot: 1,
  displayName: 'a.csv',
  profile: { rows: 3, columns: [] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 3, columns: 0, semanticRole: 'table' },
};
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };

function page(bridge: ReturnType<typeof makeMockWorkspaceBridge>) {
  const runs = makeMockDescribeBridge({ stats_desc_status: { runId: 9, state: 'running', terminal: false, columnsDone: 0, columnsTotal: 1 } as unknown as RunStateView });
  render(
    <MemoryRouter initialEntries={['/app/workspaces/statistics']}>
      <Routes>
        <Route path="/app/workspaces/statistics" element={<WorkspaceShell bridge={bridge} runs={runs} pollMs={20} />} />
        <Route path="/app/workspaces" element={<div data-testid="chooser" />} />
      </Routes>
    </MemoryRouter>
  );
}

/** ws-switch is drawn at once but disabled until the first workspace_state read
 *  lands (WorkspaceShell.tsx:166), and a click on it before then is dropped. 8T-2b's
 *  run failed so (~/gaply-corpus/os/phase8/8t2b/logs/vt_dirs_1794050091.log: no
 *  chooser, the button enabled, no switch dialog), and 8U-1's probe reproduced it
 *  with the first read held 300 ms (decision 527). */
async function switchWhenEnabled(): Promise<HTMLButtonElement> {
  const sw = (await screen.findByTestId('ws-switch')) as HTMLButtonElement;
  await waitFor(() => expect(sw.disabled).toBe(false));
  return sw;
}

describe('a Switch from the workspace page', () => {
  it("drops the Statistics window's file once Rust has let the workspace go", async () => {
    statisticsSession.setFile(view);
    const bridge = makeMockWorkspaceBridge({ workspace_state: ACTIVE, workspace_deactivate: NONE });
    page(bridge);
    fireEvent.click(await switchWhenEnabled());
    await screen.findByTestId('chooser');
    expect(statisticsSession.get().view).toBeNull();
  });

  it('keeps it while Rust refuses because a run is going', async () => {
    statisticsSession.setFile(view);
    const refusal = JSON.stringify({ code: 'workspace.run_active', params: { run: 9 }, severity: 'error' });
    const bridge = makeMockWorkspaceBridge({
      workspace_state: { ...ACTIVE, activeRun: { runId: 9, kind: 'describe' } },
      workspace_deactivate: () => {
        throw new Error(refusal);
      },
    });
    page(bridge);
    fireEvent.click(await switchWhenEnabled());
    await screen.findByTestId('ws-switch-dialog');
    await waitFor(() => expect(bridge.calls.some(([c]) => c === 'workspace_deactivate')).toBe(true));
    expect(statisticsSession.get().view).toBe(view);
  });
});

describe('both callers reach one hook', () => {
  const read = (p: string) => fs.readFileSync(path.join(__dirname, p), 'utf8');
  const code = (src: string) => src.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|\s)\/\/.*$/gm, '$1');

  it.each([
    ['WorkspaceShell', 'WorkspaceShell.tsx'],
    ['the Statistics window', '../statistics/window/StatisticsWindow.tsx'],
  ])('%s imports useWorkspaceSwitch and holds no deactivate of its own', (_name, file) => {
    const src = code(read(file));
    expect(src).toMatch(/import \{[^}]*\buseWorkspaceSwitch\b[^}]*\} from '[^']*useWorkspaceSwitch'/);
    expect(src).toMatch(/useWorkspaceSwitch\(\{/);
    expect(src).not.toMatch(/\.deactivate\(/);
    expect(src).not.toMatch(/workspace_deactivate/);
    expect(src).not.toMatch(/SWITCH_DIALOG/);
    expect(src).not.toMatch(/ws-switch-wait/);
  });

  it('the hook is the one file outside the bridge that asks Rust to deactivate', () => {
    expect(code(read('useWorkspaceSwitch.tsx')).match(/\.deactivate\(/g)).toHaveLength(1);
  });

  it('the scan sees a deactivate in code, and not in a comment', () => {
    expect(code('await b.deactivate();')).toMatch(/\.deactivate\(/);
    expect(code('// b.deactivate();')).not.toMatch(/\.deactivate\(/);
  });
});

// Research OS, Phase 8X-2 (decision 591): a crosstab is read and cancelled
// through its own commands, never the summary's (decision 580's departure,
// closed). With no steps sentence: a crosstab has one check point.
describe("a crosstab, through the switch's own kinds", () => {
  it('is polled and cancelled through stats_xtab_status and stats_xtab_cancel, and the summary is never asked', async () => {
    const xt = { runId: 12, state: 'running' as const, terminal: false, elapsedMs: 40, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };
    const xtab = makeMockXtabBridge({ stats_xtab_status: xt, stats_xtab_cancel: { ...xt, cancelRequested: true } });
    const summary = makeMockDescribeBridge({});
    const kinds = runKinds(defaultRunBridges({ xtab, describe: summary }));
    expect(Object.keys(kinds)).toContain('crosstabs');
    const job = await kinds.crosstabs.status(12);
    expect(job).toEqual({ runId: 12, state: 'running', terminal: false, progress: null, kind: 'crosstabs' });
    expect(jobSentence(job as NonNullable<typeof job>)).toBe('Run 12 (crosstabs): Running.');
    await kinds.crosstabs.cancel(12);
    expect(xtab.calls).toEqual([
      ['stats_xtab_status', { runId: 12 }],
      ['stats_xtab_cancel', { runId: 12 }],
    ]);
    expect(summary.calls).toEqual([]);
  });
});

// Research OS, Phase 8Z-2 (decision 656): a related-samples rank test is read and cancelled
// through its own commands, never the summary's (D317's departure, closed).
describe("a related-samples rank test, through the switch's own kinds", () => {
  it('is polled and cancelled through stats_related_ranks_status and stats_related_ranks_cancel, and the summary is never asked', async () => {
    const rr = { runId: 14, state: 'running' as const, terminal: false, elapsedMs: 40, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };
    const relatedRanks = makeMockRelatedRanksBridge({ stats_related_ranks_status: rr, stats_related_ranks_cancel: { ...rr, cancelRequested: true } });
    const summary = makeMockDescribeBridge({});
    const kinds = runKinds(defaultRunBridges({ relatedRanks, describe: summary }));
    expect(Object.keys(kinds)).toContain('related_ranks');
    const job = await kinds.related_ranks.status(14);
    expect(job).toEqual({ runId: 14, state: 'running', terminal: false, progress: null, kind: 'related_ranks' });
    expect(jobSentence(job as NonNullable<typeof job>)).toBe('Run 14 (related-samples rank test): Running.');
    await kinds.related_ranks.cancel(14);
    expect(relatedRanks.calls).toEqual([
      ['stats_related_ranks_status', { runId: 14 }],
      ['stats_related_ranks_cancel', { runId: 14 }],
    ]);
    expect(summary.calls).toEqual([]);
  });
});

// Research OS, Phase 8Y-2 (decision 622): a rank test is read and cancelled through its
// own commands, never the summary's (D315's departure, closed). With no steps sentence:
// a run has one check point.
describe("a rank test, through the switch's own kinds", () => {
  it('is polled and cancelled through stats_ranks_status and stats_ranks_cancel, and the summary is never asked', async () => {
    const rk = { runId: 12, state: 'running' as const, terminal: false, elapsedMs: 40, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };
    const ranks = makeMockRanksBridge({ stats_ranks_status: rk, stats_ranks_cancel: { ...rk, cancelRequested: true } });
    const summary = makeMockDescribeBridge({});
    const kinds = runKinds(defaultRunBridges({ ranks, describe: summary }));
    expect(Object.keys(kinds)).toContain('independent_ranks');
    const job = await kinds.independent_ranks.status(12);
    expect(job).toEqual({ runId: 12, state: 'running', terminal: false, progress: null, kind: 'independent_ranks' });
    expect(jobSentence(job as NonNullable<typeof job>)).toBe('Run 12 (rank test): Running.');
    await kinds.independent_ranks.cancel(12);
    expect(ranks.calls).toEqual([
      ['stats_ranks_status', { runId: 12 }],
      ['stats_ranks_cancel', { runId: 12 }],
    ]);
    expect(summary.calls).toEqual([]);
  });
});

// Research OS, Phase 9A-2 (decision 687): a linear regression is read and cancelled through its
// own commands, never the summary's (9A-1's departure, closed).
describe("a linear regression, through the switch's own kinds", () => {
  it('is polled and cancelled through stats_linear_regression_status and stats_linear_regression_cancel, and the summary is never asked', async () => {
    const lr = { runId: 15, state: 'running' as const, terminal: false, elapsedMs: 40, cancelRequested: false, stepsDone: 0, stepsTotal: 1, diagnostics: [] };
    const linearRegression = makeMockLinearRegressionBridge({ stats_linear_regression_status: lr, stats_linear_regression_cancel: { ...lr, cancelRequested: true } });
    const summary = makeMockDescribeBridge({});
    const kinds = runKinds(defaultRunBridges({ linearRegression, describe: summary }));
    expect(Object.keys(kinds)).toContain('linear_regression');
    const job = await kinds.linear_regression.status(15);
    expect(job).toEqual({ runId: 15, state: 'running', terminal: false, progress: null, kind: 'linear_regression' });
    expect(jobSentence(job as NonNullable<typeof job>)).toBe('Run 15 (linear regression): Running.');
    await kinds.linear_regression.cancel(15);
    expect(linearRegression.calls).toEqual([
      ['stats_linear_regression_status', { runId: 15 }],
      ['stats_linear_regression_cancel', { runId: 15 }],
    ]);
    expect(summary.calls).toEqual([]);
  });
});

// Research OS, Phase 9B-3 (decision 732): a factor analysis is read and cancelled through its
// own commands, never the summary's (9B-1's departure, closed), with the steps sentence.
describe("a factor analysis, through the switch's own kinds", () => {
  it('is polled and cancelled through stats_factor_status and stats_factor_cancel, and the summary is never asked', async () => {
    const fa = { runId: 16, state: 'running' as const, terminal: false, elapsedMs: 40, cancelRequested: false, stepsDone: 3, stepsTotal: 12, diagnostics: [] };
    const factor = makeMockFactorBridge({ stats_factor_status: fa, stats_factor_cancel: { ...fa, cancelRequested: true } });
    const summary = makeMockDescribeBridge({});
    const kinds = runKinds(defaultRunBridges({ factor, describe: summary }));
    expect(Object.keys(kinds)).toContain('factor');
    const job = await kinds.factor.status(16);
    expect(job).toEqual({ runId: 16, state: 'running', terminal: false, progress: '3 of 12 steps.', kind: 'factor' });
    expect(jobSentence(job as NonNullable<typeof job>)).toBe('Run 16 (factor analysis): Running. 3 of 12 steps.');
    await kinds.factor.cancel(16);
    expect(factor.calls).toEqual([
      ['stats_factor_status', { runId: 16 }],
      ['stats_factor_cancel', { runId: 16 }],
    ]);
    expect(summary.calls).toEqual([]);
  });
});
