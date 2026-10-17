// "Choose your workspace" (Research OS, Phase 3): three cards from Rust's
// list, two of them disabled ("Coming later"); availability, entitlement and
// the active state as three separate facts; one bridge call per activation;
// the heading focused on open; the state read from Rust, never decided here.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import { WorkspaceChooser } from './WorkspaceChooser';
import { makeMockWorkspaceBridge } from './workspaceBridge';
import listJson from './fixtures/workspaceList.json';
import type { WorkspaceDescriptorView, WorkspaceStateView } from './workspaceTypes';
import type { EntitlementStatus } from '../subscription/entitlement';

afterEach(cleanup);

const LIST = listJson as WorkspaceDescriptorView[];
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
  } as any;
}

function renderChooser(bridge: ReturnType<typeof makeMockWorkspaceBridge>, forceEntitlement: EntitlementStatus = 'entitled') {
  render(
    <MemoryRouter initialEntries={['/app/workspaces']}>
      <GaplySessionProvider authService={auth()}>
        <Routes>
          <Route path="/app/workspaces" element={<WorkspaceChooser bridge={bridge} forceEntitlement={forceEntitlement} />} />
          <Route path="/app/workspaces/statistics" element={<div data-testid="statistics-home" />} />
        </Routes>
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

describe('WorkspaceChooser', () => {
  it('renders three cards, Compute and Design disabled as "Coming later"', async () => {
    const bridge = makeMockWorkspaceBridge({ workspace_list: LIST, workspace_state: NONE });
    renderChooser(bridge);
    await screen.findByTestId('ws-card-statistics');
    expect(screen.getAllByTestId(/^ws-card-/)).toHaveLength(3);
    expect(screen.getByText('Analyze datasets and verify research results.')).toBeTruthy();
    expect(screen.getByText('Solve equations, build numerical models, and explore signals.')).toBeTruthy();
    expect(screen.getByText('Draft, model, and validate engineering geometry.')).toBeTruthy();

    const open = screen.getByTestId('ws-open-statistics') as HTMLButtonElement;
    expect(open.disabled).toBe(false);
    expect(open.textContent).toBe('Open Statistics');
    for (const id of ['compute', 'design']) {
      const b = screen.getByTestId(`ws-open-${id}`) as HTMLButtonElement;
      expect(b.disabled, id).toBe(true);
      expect(b.getAttribute('aria-disabled')).toBe('true');
      expect(screen.getByTestId(`ws-availability-${id}`).textContent).toBe('Coming later');
      fireEvent.click(b);
    }
    expect(bridge.calls.filter(([c]) => c === 'workspace_activate')).toHaveLength(0);
  });

  it('shows availability and entitlement as separate facts', async () => {
    const bridge = makeMockWorkspaceBridge({ workspace_list: LIST, workspace_state: NONE });
    renderChooser(bridge, 'signed_out');
    await screen.findByTestId('ws-card-statistics');
    // Available, but the account fact is its own line: neither stands in for the other.
    expect(screen.getByTestId('ws-availability-statistics').textContent).toBe('Available');
    expect(screen.getByTestId('ws-entitlement-statistics').textContent).toBe('Sign in to check access');
    expect(screen.getByTestId('ws-availability-compute').textContent).toBe('Coming later');
    expect(screen.getByTestId('ws-entitlement-compute').textContent).toBe('Sign in to check access');
    expect(screen.getByTestId('ws-active-statistics').textContent).toBe('Not open');
  });

  it('activation calls the bridge once, then follows Rust to the workspace', async () => {
    let resolve!: (v: WorkspaceStateView) => void;
    const pending = new Promise<WorkspaceStateView>((r) => (resolve = r));
    const bridge = makeMockWorkspaceBridge({
      workspace_list: LIST,
      workspace_state: NONE,
      workspace_activate: () => pending,
    });
    renderChooser(bridge);
    const open = await screen.findByTestId('ws-open-statistics');
    fireEvent.click(open);
    fireEvent.click(open);
    fireEvent.click(open);
    resolve(ACTIVE);
    await screen.findByTestId('statistics-home');
    const activations = bridge.calls.filter(([c]) => c === 'workspace_activate');
    expect(activations).toEqual([['workspace_activate', { id: 'statistics' }]]);
  });

  it('shows a refusal in words, and does not move', async () => {
    const bridge = makeMockWorkspaceBridge({
      workspace_list: LIST,
      workspace_state: NONE,
      workspace_activate: () => {
        throw Object.assign(
          new Error(JSON.stringify({ code: 'workspace.not_ready', params: { workspace: 'statistics' }, severity: 'error' })),
          { code: 'validation' }
        );
      },
    });
    renderChooser(bridge);
    fireEvent.click(await screen.findByTestId('ws-open-statistics'));
    await waitFor(() =>
      expect(screen.getByTestId('ws-chooser-message').textContent).toBe('Statistics could not be made ready, so it was not opened.')
    );
    expect(screen.queryByTestId('statistics-home')).toBeNull();
  });

  it('reads the active workspace from Rust and offers to continue, without activating', async () => {
    const bridge = makeMockWorkspaceBridge({ workspace_list: LIST, workspace_state: ACTIVE });
    renderChooser(bridge);
    const card = await screen.findByTestId('ws-card-statistics');
    await waitFor(() => expect(within(card).getByTestId('ws-active-statistics').textContent).toBe('Open'));
    fireEvent.click(within(card).getByTestId('ws-continue-statistics'));
    await screen.findByTestId('statistics-home');
    expect(bridge.calls.map(([c]) => c)).toEqual(['workspace_list', 'workspace_state']);
  });

  it('puts focus on its heading when it opens, and labels each button by its card', async () => {
    const bridge = makeMockWorkspaceBridge({ workspace_list: LIST, workspace_state: NONE });
    renderChooser(bridge);
    const heading = screen.getByTestId('ws-chooser-heading');
    expect(heading.tagName).toBe('H1');
    expect(heading.textContent).toBe('Choose your workspace');
    expect(document.activeElement).toBe(heading);
    const open = await screen.findByTestId('ws-open-statistics');
    const described = (open.getAttribute('aria-describedby') ?? '').split(' ');
    expect(described).toEqual(['ws-desc-statistics', 'ws-facts-statistics']);
    for (const id of described) expect(document.getElementById(id)).not.toBeNull();
    // Keyboard: the enabled button is a real button, reachable by Tab.
    expect(open.tagName).toBe('BUTTON');
    expect(open.getAttribute('tabindex')).toBeNull();
  });
});
