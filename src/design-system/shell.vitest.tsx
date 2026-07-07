// Design-system smoke tests (Vitest + jsdom). Named *.vitest.tsx so CRA's
// Jest never picks these up. No globe here — R3F needs WebGL, not jsdom.
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { NavRail, ThreePanelWorkspace, AppShell, HeaderBar } from './shell';
import { Panel } from './primitives';

afterEach(cleanup);

const ITEMS = [
  { id: 'a', label: 'Overview', icon: '◫' },
  { id: 'b', label: 'Reports', icon: '✓' },
];

describe('NavRail', () => {
  it('collapses and expands via the toggle', () => {
    render(<NavRail items={ITEMS} activeId="a" />);
    const rail = screen.getByTestId('gds-rail');
    const toggle = screen.getByTestId('gds-rail-toggle');

    // expanded by default
    expect(rail.getAttribute('data-collapsed')).toBe('false');
    expect(toggle.getAttribute('aria-expanded')).toBe('true');

    // collapse
    fireEvent.click(toggle);
    expect(rail.getAttribute('data-collapsed')).toBe('true');
    expect(toggle.getAttribute('aria-expanded')).toBe('false');

    // expand again
    fireEvent.click(toggle);
    expect(rail.getAttribute('data-collapsed')).toBe('false');
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
  });

  it('renders every item with icon + label and marks the active one', () => {
    render(<NavRail items={ITEMS} activeId="b" />);
    expect(screen.getByText('Overview')).toBeTruthy();
    const active = screen.getByText('Reports').closest('button');
    expect(active?.getAttribute('aria-current')).toBe('true');
  });
});

describe('ThreePanelWorkspace', () => {
  it('renders all three regions when outline + inspector are provided', () => {
    render(
      <ThreePanelWorkspace
        outline={<Panel title="Outline">left</Panel>}
        inspector={<Panel title="Inspector">right</Panel>}
      >
        <Panel title="Main">center</Panel>
      </ThreePanelWorkspace>
    );
    expect(screen.getByTestId('gds-workspace-outline')).toBeTruthy();
    expect(screen.getByTestId('gds-workspace-center')).toBeTruthy();
    expect(screen.getByTestId('gds-workspace-inspector')).toBeTruthy();
    expect(screen.getByTestId('gds-workspace').getAttribute('data-inspector')).toBe('true');
  });

  it('collapses side columns when their slots are omitted', () => {
    render(<ThreePanelWorkspace>center only</ThreePanelWorkspace>);
    const ws = screen.getByTestId('gds-workspace');
    expect(ws.getAttribute('data-outline')).toBe('false');
    expect(ws.getAttribute('data-inspector')).toBe('false');
    expect(screen.queryByTestId('gds-workspace-outline')).toBeNull();
  });
});

describe('AppShell', () => {
  it('composes rail + header + workspace inside the gds-root scope', () => {
    render(
      <AppShell rail={<NavRail items={ITEMS} />} header={<HeaderBar title="Gaply" />}>
        <ThreePanelWorkspace>content</ThreePanelWorkspace>
      </AppShell>
    );
    const shell = screen.getByTestId('gds-shell');
    expect(shell.className).toContain('gds-root');
    expect(screen.getByTestId('gds-rail')).toBeTruthy();
    expect(screen.getByTestId('gds-header')).toBeTruthy();
    expect(screen.getByTestId('gds-workspace')).toBeTruthy();
  });
});
