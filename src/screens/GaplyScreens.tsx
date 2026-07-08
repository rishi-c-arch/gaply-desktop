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
import AuthSuccessPage from './auth/AuthSuccessPage';
import OnboardingPage from './onboarding/OnboardingPage';
import HomeDashboardPage from './home/HomeDashboardPage';
import AnalysisTheaterPage from './analysis/AnalysisTheaterPage';
import LiveReportPage from './report/LiveReportPage';
import PlagiarismCheckPage from './checks/PlagiarismCheckPage';
import AiCheckPage from './checks/AiCheckPage';
import StatsCheckPage from './checks/StatsCheckPage';
import CitationManagerPage from './citations/CitationManagerPage';
import JournalCheckPage from './journal/JournalCheckPage';
import PublishReadyPage from './publishready/PublishReadyPage';
import CopilotPage from './copilot/CopilotPage';
import BillingPage from './subscription/BillingPage';
import CommunityPage from './community/CommunityPage';
import SettingsPage from './settings/SettingsPage';
import ComingSoonPage from './common/ComingSoonPage';

export const AuthRoute: React.FC = () => (
  <GaplySessionProvider>
    <AuthPage />
  </GaplySessionProvider>
);

/** /auth-success — public web hand-off page for desktop Google OAuth; bounces
 *  the ?code back to the gaply:// deep link. No session provider needed. */
export const AuthSuccessRoute: React.FC = () => <AuthSuccessPage />;

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

/** /app/report — FREE-OFFLINE: loads the REAL compiled report by id (from a
 *  finished analysis) and renders it in the viewer. No sample fixture on the
 *  live path — the fixture is test-only now. */
export const ReportRoute: React.FC = () => (
  <GaplySessionProvider>
    <LiveReportPage />
  </GaplySessionProvider>
);

/** Single-agent check screens (F7) — all FREE-OFFLINE (local, on-device). */
export const PlagiarismCheckRoute: React.FC = () => (
  <GaplySessionProvider>
    <PlagiarismCheckPage />
  </GaplySessionProvider>
);
export const AiCheckRoute: React.FC = () => (
  <GaplySessionProvider>
    <AiCheckPage />
  </GaplySessionProvider>
);
export const StatsCheckRoute: React.FC = () => (
  <GaplySessionProvider>
    <StatsCheckPage />
  </GaplySessionProvider>
);

/** /app/citations — Citation Manager (F8). Free-offline; metadata syncs to
 *  Supabase when signed in, the manuscript never does. */
export const CitationManagerRoute: React.FC = () => (
  <GaplySessionProvider>
    <CitationManagerPage />
  </GaplySessionProvider>
);

/** /app/journal — Journal Check (F9). Free-offline; online fallback via proxy. */
export const JournalCheckRoute: React.FC = () => (
  <GaplySessionProvider>
    <JournalCheckPage />
  </GaplySessionProvider>
);

/** /app/publishready — PublishReady (F10), the paid flagship. Premium-gated. */
export const PublishReadyRoute: React.FC = () => (
  <GaplySessionProvider>
    <PublishReadyPage />
  </GaplySessionProvider>
);

/** /app/copilot — Research Copilot (F11), premium-gated. */
export const CopilotRoute: React.FC = () => (
  <GaplySessionProvider>
    <CopilotPage />
  </GaplySessionProvider>
);

/** /app/billing — Plans & Billing (F12). */
export const BillingRoute: React.FC = () => (
  <GaplySessionProvider>
    <BillingPage />
  </GaplySessionProvider>
);

/** /app/community — Research Co-Author (F13). */
export const CommunityRoute: React.FC = () => (
  <GaplySessionProvider>
    <CommunityPage />
  </GaplySessionProvider>
);

/** /app/settings — Settings & account (F14). FREE-OFFLINE: privacy toggles,
 *  offline data and appearance work signed-out; profile/subscription sections
 *  degrade gracefully without a session. */
export const SettingsRoute: React.FC = () => (
  <GaplySessionProvider>
    <SettingsPage />
  </GaplySessionProvider>
);

/** /app/coming-soon — honest placeholder for not-yet-built nav targets. */
export const ComingSoonRoute: React.FC = () => (
  <GaplySessionProvider>
    <ComingSoonPage />
  </GaplySessionProvider>
);
