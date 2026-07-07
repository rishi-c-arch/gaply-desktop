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
import UploadPlaceholderPage from './upload/UploadPlaceholderPage';

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

/** /app/upload — FREE-OFFLINE (on-device parsing; F5 fills in the real flow). */
export const UploadRoute: React.FC = () => (
  <GaplySessionProvider>
    <UploadPlaceholderPage />
  </GaplySessionProvider>
);
