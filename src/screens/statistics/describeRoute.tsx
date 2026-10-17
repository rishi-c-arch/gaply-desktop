// Gaply — where the descriptive summary is reached (Research OS, Phase 2B):
// its route and its hub tile, both only while REACT_APP_FEATURE_STATS_DESCRIBE
// is on. The flag hides unfinished UI; it is not an authorization boundary.
// Kept apart from DescribeFlow so the router can name the route without
// loading the screen.
import React from 'react';
import { Route } from 'react-router-dom';
import { isFeatureEnabled } from '../../config/featureFlags';

export const DESCRIBE_PATH = '/app/statistics/describe';
export const DESCRIBE_TILE = { label: 'Descriptive Summary', href: DESCRIBE_PATH };

/** The route, or nothing while the flag is off. Called inside `<Routes>`. */
export function describeRoute(element: React.ReactElement): React.ReactElement | null {
  return isFeatureEnabled('statsDescribe') ? <Route path={DESCRIBE_PATH} element={element} /> : null;
}

/** The hub tile, or none while the flag is off. With the workspaces flag on
 *  too, the screen is reached from the Statistics workspace instead, so the
 *  hub has no tile for it (Research OS, Phase 3). */
export function describeTiles(): Array<typeof DESCRIBE_TILE> {
  return isFeatureEnabled('statsDescribe') && !isFeatureEnabled('workspaces') ? [DESCRIBE_TILE] : [];
}
