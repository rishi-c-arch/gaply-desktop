// Gaply — where an open project's page is reached (Research OS, Phase 5B-3b;
// decisions 136 and 149). Its route holds the project's id and nothing else:
// never its name, and never anything in the router's state.
//
// The route exists only while the project screens show (projectsShown(): both
// flags on, desktop app) and REACT_APP_FEATURE_STATS_DESCRIBE is on, the flag
// the Statistics home it is reached from sits behind. A flag hides; Rust's
// checks are the boundary. Kept apart from the page so the router can name
// the route without loading it.
import React from 'react';
import { Route } from 'react-router-dom';
import { isFeatureEnabled } from '../../../config/featureFlags';
import { projectsShown } from './projectGate';

export const PROJECT_PAGE_PATH = '/app/statistics/projects/:projectId';

/** The page of one project, by its id. */
export function projectPagePath(projectId: string): string {
  return `/app/statistics/projects/${encodeURIComponent(projectId)}`;
}

/** The page's route, or none. Called inside `<Routes>`. */
export function projectRoutes(element: React.ReactElement, desktop?: boolean): React.ReactElement[] {
  if (!isFeatureEnabled('statsDescribe') || !projectsShown(desktop)) return [];
  return [<Route key="stats-project" path={PROJECT_PAGE_PATH} element={element} />];
}
