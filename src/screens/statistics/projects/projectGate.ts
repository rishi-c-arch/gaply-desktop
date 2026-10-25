// Gaply — whether the project screens show (Research OS, Phase 5B-3a;
// decision 136). The one place that decides it: every project screen,
// button and sentence asks this, and shows nothing, and calls nothing, when
// it says no.
//
// Yes only when REACT_APP_FEATURE_PROJECTS and REACT_APP_FEATURE_WORKSPACES
// are both on, and only in the desktop app. A flag hides unfinished UI; it is
// never an authorization boundary: Rust checks every project_* command.
import { isFeatureEnabled } from '../../../config/featureFlags';
import { isTauri } from '../../../utils/isTauri';

/** `desktop` is a test seam; the app passes nothing. */
export function projectsShown(desktop: boolean = isTauri): boolean {
  return desktop && isFeatureEnabled('projects') && isFeatureEnabled('workspaces');
}
