// Gaply — where the workspace screens are reached (Research OS, Phase 3):
// the chooser and the Statistics shell, both only while
// REACT_APP_FEATURE_WORKSPACES is on, and the hub tile. The flag hides
// unfinished UI; it is not an authorization boundary: Rust refuses every
// stats_desc_* command unless Statistics is active, whatever the screens show.
// Kept apart from the screens so the router and the hub can name the routes
// without loading them.
import React from 'react';
import { Route } from 'react-router-dom';
import { isFeatureEnabled } from '../../config/featureFlags';
import type { WorkspaceId } from './workspaceTypes';

export const CHOOSER_PATH = '/app/workspaces';
export const STATISTICS_PATH = '/app/workspaces/statistics';

/** Where each workspace opens. Only Statistics has a layout in this build. */
export const WORKSPACE_HOME: Partial<Record<WorkspaceId, string>> = {
  statistics: STATISTICS_PATH,
};

export const WORKSPACES_TILE = { label: 'Choose your workspace', href: CHOOSER_PATH };

/** The chooser's and the shell's routes, or nothing while the flag is off.
 *  Called inside `<Routes>`. */
export function workspaceRoutes(chooser: React.ReactElement, shell: React.ReactElement): React.ReactElement[] {
  if (!isFeatureEnabled('workspaces')) return [];
  return [
    <Route key="ws-chooser" path={CHOOSER_PATH} element={chooser} />,
    <Route key="ws-statistics" path={STATISTICS_PATH} element={shell} />,
  ];
}

/** The hub tile, or none while the flag is off. */
export function workspaceTiles(): Array<typeof WORKSPACES_TILE> {
  return isFeatureEnabled('workspaces') ? [WORKSPACES_TILE] : [];
}
