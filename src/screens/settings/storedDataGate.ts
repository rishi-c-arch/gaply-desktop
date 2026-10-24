// Gaply — whether the Stored data section shows (Research OS, Phase 5D-1a;
// decision 206). Settings asks this, and shows nothing, and calls nothing,
// when it says no.
//
// Yes only when REACT_APP_FEATURE_STORED_DATA is on, and only in the desktop
// app. A flag hides unfinished UI; it is never an authorization boundary.
import { isFeatureEnabled } from '../../config/featureFlags';
import { isTauri } from '../../utils/isTauri';

/** `desktop` is a test seam; the app passes nothing. */
export function storedDataShown(desktop: boolean = isTauri): boolean {
  return desktop && isFeatureEnabled('storedData');
}
