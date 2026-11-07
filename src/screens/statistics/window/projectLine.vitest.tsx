// The window's project line (Research OS, Phase 8U-1, decision 523; ADR-012
// Part 9): plain text with no project open; with one open, a button holding
// "Project: " and the name inside <bdi>, opening that project's page; its one
// CSS rule beside .sw-project. Every bridge is a mock, and no project command is
// named as a string here (projectGuards.vitest.ts scans this folder).
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { PROJECT_PAGE_PATH, projectPagePath } from '../projects/projectRoute';
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
const NAME = 'My thesis, chapter three';

function setUp(open: boolean) {
  const seen: string[] = [];
  const Where: React.FC = () => {
    const l = useLocation();
    seen.push(`${l.pathname}${JSON.stringify(l.state)}`);
    return null;
  };
  const projects = makeMockProjectBridge(
    open
      ? { [C.current]: { projectId: ID, rerun: null }, [C.list]: [{ projectId: ID, name: NAME, bytes: 1, savedAt: 1, runCount: 1, refused: null }] }
      : { [C.current]: { projectId: null, rerun: null }, [C.list]: [] }
  );
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Where />
      <Routes>
        <Route
          path="/w"
          element={
            <StatisticsWindow
              workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
              files={makeMockDescribeBridge({ stats_desc_close: true })}
              capabilities={makeMockCapabilityBridge({ capability_list: [] })}
              projects={projects}
              limits={() => Promise.resolve(limits8)}
              pollMs={15}
            />
          }
        />
        <Route path={PROJECT_PAGE_PATH} element={<div data-testid="page-here" />} />
      </Routes>
    </MemoryRouter>
  );
  return { seen, projects };
}

describe("the project's line (decision 523)", () => {
  it('with no project open: the text "Project: none open", and no button', async () => {
    setUp(false);
    const line = await screen.findByTestId('sw-project-line');
    await act(async () => {
      await new Promise((r) => setTimeout(r, 20));
    });
    expect(line.textContent).toBe('Project: none open');
    expect(line.querySelector('button')).toBeNull();
    expect(screen.queryByTestId('sw-project-open')).toBeNull();
  });

  it('with one open: a button inside the line, its text the line\'s, the name inside bdi, the line keeping its title', async () => {
    setUp(true);
    const button = await screen.findByTestId('sw-project-open');
    const line = screen.getByTestId('sw-project-line');
    expect(button.tagName).toBe('BUTTON');
    expect(button.getAttribute('type')).toBe('button');
    expect(line.contains(button)).toBe(true);
    expect(button.textContent).toBe(`Project: ${NAME}`);
    expect(line.textContent).toBe(`Project: ${NAME}`);
    expect(button.querySelector('bdi')?.textContent).toBe(NAME);
    expect(line.getAttribute('title')).toBe(NAME);
  });

  it("clicking it opens that project's page, by its id, with no router state, and asks Rust nothing more", async () => {
    const { seen, projects } = setUp(true);
    await waitFor(() => expect(projects.calls.map(([c]) => c).sort()).toEqual([C.current, C.list].sort()));
    fireEvent.click(await screen.findByTestId('sw-project-open'));
    expect(await screen.findByTestId('page-here')).toBeTruthy();
    expect(seen[seen.length - 1]).toBe(`${projectPagePath(ID)}null`);
    expect(projects.calls.map(([c]) => c).sort()).toEqual([C.current, C.list].sort());
  });

  it('follows the store: a project closed elsewhere turns the button back into text', async () => {
    setUp(true);
    await screen.findByTestId('sw-project-open');
    act(() => openProjectStore.clear());
    await waitFor(() => expect(screen.queryByTestId('sw-project-open')).toBeNull());
    expect(screen.getByTestId('sw-project-line').textContent).toBe('Project: none open');
  });
});

describe("the button's one CSS rule (decision 523)", () => {
  const css = fs.readFileSync(path.join(__dirname, 'StatisticsWindow.css'), 'utf8');
  const rule = (selector: string) => {
    const at = css.indexOf(`${selector} {`);
    if (at < 0) throw new Error(`no rule ${selector}`);
    return css.slice(at, css.indexOf('}', at) + 1);
  };

  it('inherits the font, has no border or background, and clips the name with an ellipsis, scrolling nothing', () => {
    const r = rule('.gds-root .sw-project__open');
    expect(r).toMatch(/font: inherit;/);
    expect(r).toMatch(/border: 0;/);
    expect(r).toMatch(/background: none;/);
    expect(r).toMatch(/max-width: 100%;/);
    expect(r).toMatch(/overflow: hidden;/);
    expect(r).toMatch(/text-overflow: ellipsis;/);
    expect(r).toMatch(/white-space: nowrap;/);
    expect(r).not.toMatch(/overflow(-[xy])?: (auto|scroll)/);
  });
});
