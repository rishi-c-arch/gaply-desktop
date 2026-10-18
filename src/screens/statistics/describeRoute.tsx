// Gaply — where the descriptive summary is reached (Research OS, Phase 2B):
// its route, only while REACT_APP_FEATURE_STATS_DESCRIBE is on. The flag
// hides unfinished UI; it is not an authorization boundary.
// Kept apart from DescribeFlow so the router can name the route without
// loading the screen.
import React from 'react';
import { Route } from 'react-router-dom';
import { isFeatureEnabled } from '../../config/featureFlags';

export const DESCRIBE_PATH = '/app/statistics/describe';

/** The route, or nothing while the flag is off. Called inside `<Routes>`. */
export function describeRoute(element: React.ReactElement): React.ReactElement | null {
  return isFeatureEnabled('statsDescribe') ? <Route path={DESCRIBE_PATH} element={element} /> : null;
}

// Its hub tile and its link in the Statistics workspace were replaced by the
// method selector's (Research OS, Phase 4B-2, decision 77): see
// statisticsRoute.tsx. The screen is reached from the selector.
