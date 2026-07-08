// Gaply — <Feature> gate + useFeatureFlag hook. Keeps flag checks out of screen
// bodies. When a flag is off, render the `fallback` (an honest coming-soon /
// disabled state) — never a fabricated result.
import React from 'react';
import { FeatureName, isFeatureEnabled } from './featureFlags';

export function useFeatureFlag(name: FeatureName): boolean {
  // Flags are build-time constants, so this is a pure read (no state/effect
  // needed); the hook shape exists so callers can use it idiomatically.
  return isFeatureEnabled(name);
}

export interface FeatureProps {
  name: FeatureName;
  children: React.ReactNode;
  /** Rendered when the flag is OFF (honest disabled/coming-soon state). */
  fallback?: React.ReactNode;
}

export const Feature: React.FC<FeatureProps> = ({ name, children, fallback = null }) => {
  return <>{isFeatureEnabled(name) ? children : fallback}</>;
};
