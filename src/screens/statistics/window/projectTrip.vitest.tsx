// The window and the project page, there and back (Research OS, Phase 8U-1,
// decision 524): the line opens the page, the page's way back mounts the window
// again, and the session is as it was: the same file, the same Output and the
// same selected entry, the window opening on the Data View tab. Every bridge is
// a mock; no project command is named as a string here.
import React from 'react';
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { PROJECT_PAGE_PATH } from '../projects/projectRoute';
import { openProjectStore } from '../projects/projectSession';
import { makeMockCapabilityBridge } from '../ttestBridge';
import { StatisticsWindow } from './StatisticsWindow';
import { statisticsSession } from './statisticsSession';

vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'workspaces' || name === 'projects' }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
});

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const ID = 'p'.repeat(32);
const col = (alias: string): ColumnProfile => ({ alias, header: alias, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 1,
  displayName: 'items.csv',
  profile: { rows: 9, columns: [col('C1'), col('C2')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 9, columns: 2, semanticRole: 'table' },
};

/** The project page's stand-in: its way back, as ProjectPageScreen's home goes. */
const Page: React.FC = () => {
  const navigate = useNavigate();
  return (
    <button type="button" data-testid="page-back" onClick={() => navigate('/w')}>
      back
    </button>
  );
};

describe('to the project page and back (decision 524)', () => {
  it('keeps the file, the Output and the selected entry, and opens on the Data View tab', async () => {
    const projects = makeMockProjectBridge({ [C.current]: { projectId: ID, rerun: null }, [C.list]: [{ projectId: ID, name: 'A project', bytes: 1, savedAt: 1, runCount: 1, refused: null }] });
    act(() => {
      statisticsSession.setFile(view);
      statisticsSession.addEntry({ kind: 'cronbach_alpha', view, slot: 1, settings: 'Items: C1, C2.', runId: 5, run: { runId: 5, state: 'cancelled', terminal: true, elapsedMs: 1, cancelRequested: false, stepsDone: 1, stepsTotal: 7, diagnostics: [] } });
    });
    const before = statisticsSession.get();
    expect(before.output).toHaveLength(1);
    render(
      <MemoryRouter initialEntries={['/w']}>
        <Routes>
          <Route
            path="/w"
            element={
              <StatisticsWindow
                workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
                files={makeMockDescribeBridge({ stats_desc_close: true })}
                cells={makeMockDescribeBridge({ stats_desc_cells: { slot: 1, inputHash: 'ab'.repeat(32), rows: 9, columns: 2, firstRow: 0, firstColumn: 0, cells: [] } })}
                capabilities={makeMockCapabilityBridge({ capability_list: [] })}
                projects={projects}
                limits={() => Promise.resolve(limits8)}
                pollMs={15}
              />
            }
          />
          <Route path={PROJECT_PAGE_PATH} element={<Page />} />
        </Routes>
      </MemoryRouter>
    );
    fireEvent.click(screen.getByRole('tab', { name: /Output/ }));
    fireEvent.click(await screen.findByTestId('sw-project-open'));
    fireEvent.click(await screen.findByTestId('page-back'));
    await screen.findByTestId('statistics-window');

    const after = statisticsSession.get();
    expect(after.view).toBe(before.view);
    expect(after.slot).toBe(before.slot);
    expect(after.output).toBe(before.output);
    expect(after.selected).toBe(before.selected);
    expect(screen.getByRole('tab', { name: /Data View/ }).getAttribute('aria-selected')).toBe('true');
    expect(screen.getByRole('tab', { name: /Output/ }).textContent).toContain('1');
  });
});
