// Gaply — where the Statistics method selector and the t-test are reached
// (Research OS, Phase 4B-2): their routes and the hub tile, only while
// REACT_APP_FEATURE_STATS_DESCRIBE is on. The flag hides unfinished UI; it is
// not an authorization boundary: Rust refuses every stats_ttest_* command
// unless Statistics is active, whatever the screens show.
//
// The selector replaces the descriptive summary's entry point (decision 77):
// the hub tile while only the describe flag is on, and the Statistics
// workspace's tools while REACT_APP_FEATURE_WORKSPACES is on too. The
// describe flow keeps its own route (describeRoute.tsx). Kept apart from the
// screens so the router and the hub can name the routes without loading them.
import React from 'react';
import { Route } from 'react-router-dom';
import { isFeatureEnabled } from '../../config/featureFlags';

export const STATISTICS_HOME_PATH = '/app/statistics';
export const TTEST_PATH = '/app/statistics/ttest';
/** The one-sample t-test's screen (Research OS, Phase 8D-2b, decision 268). */
export const ONE_SAMPLE_PATH = '/app/statistics/one-sample';
/** The paired-samples t-test's screen (Research OS, Phase 8D-2c, decision 271). */
export const PAIRED_PATH = '/app/statistics/paired';
export const STATISTICS_TILE = { label: 'Statistical Methods', href: STATISTICS_HOME_PATH };

/** The selector's and every method's screen's routes (the t-test's, the
 *  one-sample t-test's and the paired t-test's), or nothing while the flag is
 *  off. Called inside `<Routes>`. Every element is required, so a caller
 *  cannot leave a method without its route (decision 271). */
export function statisticsRoutes(
  home: React.ReactElement,
  ttest: React.ReactElement,
  oneSample: React.ReactElement,
  paired: React.ReactElement
): React.ReactElement[] {
  if (!isFeatureEnabled('statsDescribe')) return [];
  return [
    <Route key="stats-home" path={STATISTICS_HOME_PATH} element={home} />,
    <Route key="stats-ttest" path={TTEST_PATH} element={ttest} />,
    <Route key="stats-one-sample" path={ONE_SAMPLE_PATH} element={oneSample} />,
    <Route key="stats-paired" path={PAIRED_PATH} element={paired} />,
  ];
}

/** The hub tile, or none while the flag is off. With the workspaces flag on
 *  too, the selector is reached from the Statistics workspace instead, so the
 *  hub has no tile for it. */
export function statisticsTiles(): Array<typeof STATISTICS_TILE> {
  return isFeatureEnabled('statsDescribe') && !isFeatureEnabled('workspaces') ? [STATISTICS_TILE] : [];
}

/** The Statistics workspace's entry to the selector, while the describe flag
 *  is on (decision 77). */
export function statisticsTools(): Array<{ label: string; href: string }> {
  return isFeatureEnabled('statsDescribe') ? [STATISTICS_TILE] : [];
}
