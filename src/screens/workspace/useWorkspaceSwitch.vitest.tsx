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
