// Gaply DS — app shell: collapsible NavRail, HeaderBar, ThreePanelWorkspace.
import React, { useState } from 'react';
import './tokens.css';
import './shell.css';

/* ------------------------------- NavRail -------------------------------- */

export interface NavRailItem {
  id: string;
  label: string;
  /** Emoji or glyph placeholder until an icon set is chosen. */
  icon: React.ReactNode;
  onSelect?: () => void;
}

export interface NavRailProps {
  items: NavRailItem[];
  activeId?: string;
  /** Brand slot (e.g. the micro globe mark). */
  brand?: React.ReactNode;
  brandName?: string;
  defaultCollapsed?: boolean;
}

export const NavRail: React.FC<NavRailProps> = ({
  items,
  activeId,
  brand,
  brandName = 'Gaply',
  defaultCollapsed = false,
}) => {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);
  return (
    <nav className="gds-rail" data-collapsed={collapsed} data-testid="gds-rail" aria-label="Primary">
      <div className="gds-rail__brand">
        {brand}
        <span className="gds-rail__brand-name">{brandName}</span>
      </div>
      {items.map((it) => (
        <button
          key={it.id}
          type="button"
          className="gds-rail__item"
          aria-current={it.id === activeId}
          onClick={it.onSelect}
          title={collapsed ? it.label : undefined}
        >
          <span className="gds-rail__icon" aria-hidden="true">
            {it.icon}
          </span>
          <span className="gds-rail__label">{it.label}</span>
        </button>
      ))}
      <div className="gds-rail__spacer" />
      <button
        type="button"
        className="gds-rail__item gds-rail__toggle"
        onClick={() => setCollapsed((c) => !c)}
        aria-expanded={!collapsed}
        aria-label={collapsed ? 'Expand navigation' : 'Collapse navigation'}
        data-testid="gds-rail-toggle"
      >
        <span className="gds-rail__icon" aria-hidden="true">
          {collapsed ? '»' : '«'}
        </span>
        <span className="gds-rail__label">Collapse</span>
      </button>
    </nav>
  );
};

/* ------------------------------ HeaderBar ------------------------------- */

export interface HeaderBarProps {
  title: string;
  /** Right-aligned slot (actions, usage meter, avatar…). */
  children?: React.ReactNode;
}

export const HeaderBar: React.FC<HeaderBarProps> = ({ title, children }) => (
  <header className="gds-header" data-testid="gds-header">
    <span className="gds-header__title">{title}</span>
    <div className="gds-header__spacer" />
    {children}
  </header>
);

/* -------------------------- ThreePanelWorkspace ------------------------- */

export interface ThreePanelWorkspaceProps {
  /** Left: navigation / document outline. Omit to collapse the column. */
  outline?: React.ReactNode;
  /** Center: main content. */
  children: React.ReactNode;
  /** Right: inspector / details. Omit to collapse the column. */
  inspector?: React.ReactNode;
}

export const ThreePanelWorkspace: React.FC<ThreePanelWorkspaceProps> = ({
  outline,
  children,
  inspector,
}) => (
  <div
    className="gds-workspace"
    data-testid="gds-workspace"
    data-outline={Boolean(outline)}
    data-inspector={Boolean(inspector)}
  >
    {outline && <div data-testid="gds-workspace-outline">{outline}</div>}
    <div data-testid="gds-workspace-center">{children}</div>
    {inspector && <div data-testid="gds-workspace-inspector">{inspector}</div>}
  </div>
);

/* ------------------------------- AppShell ------------------------------- */

export interface AppShellProps {
  rail: React.ReactNode;
  header: React.ReactNode;
  children: React.ReactNode;
}

/** Grid glue: rail + header + main area, dark-first. */
export const AppShell: React.FC<AppShellProps> = ({ rail, header, children }) => (
  <div className="gds-root gds-shell" data-testid="gds-shell">
    {rail}
    {header}
    {children}
  </div>
);
