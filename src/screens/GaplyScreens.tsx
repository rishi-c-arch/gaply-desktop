// Gaply — desktop-app route elements, composed with the session provider and
// (where required) the RequireSession guard. One module = one lazy chunk, so
// supabase-js and the new screens stay out of the marketing-site bundle path.
//
// Guarding policy:
//   /auth        — public (entry point)
//   /onboarding  — ONLINE-ONLY (writes the user's profiles row) → RequireSession
//   /app         — FREE-OFFLINE (never requires a session)
import React from 'react';
import { GaplySessionProvider, RequireSession } from './session/SessionProvider';
import AuthPage from './auth/AuthPage';
import OnboardingPage from './onboarding/OnboardingPage';
import HomeDashboardPage from './home/HomeDashboardPage';
import AnalysisTheaterPage from './analysis/AnalysisTheaterPage';
import ReportViewerPage from './report/ReportViewerPage';

export const AuthRoute: React.FC = () => (
  <GaplySessionProvider>
    <AuthPage />
  </GaplySessionProvider>
);

export const OnboardingRoute: React.FC = () => (
  <GaplySessionProvider>
    <RequireSession>
      <OnboardingPage />
    </RequireSession>
  </GaplySessionProvider>
);

export const HomeRoute: React.FC = () => (
  <GaplySessionProvider>
    <HomeDashboardPage />
  </GaplySessionProvider>
);

/** /app/upload — FREE-OFFLINE: manuscript is parsed on-device by the Rust core;
 *  no bytes leave the machine. The verification (cloud) lane is premium-gated. */
export const UploadRoute: React.FC = () => (
  <GaplySessionProvider>
    <AnalysisTheaterPage />
  </GaplySessionProvider>
);

/** /app/report — FREE-OFFLINE: renders compile_report() output (the same viewer
 *  is reused by paid PublishReady, which adds the Reviewer Letter tab). */
export const ReportRoute: React.FC = () => (
  <GaplySessionProvider>
    <ReportViewerPage />
  </GaplySessionProvider>
);
