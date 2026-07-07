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
import PlagiarismCheckPage from './checks/PlagiarismCheckPage';
import AiCheckPage from './checks/AiCheckPage';
import StatsCheckPage from './checks/StatsCheckPage';
import CitationManagerPage from './citations/CitationManagerPage';
import JournalCheckPage from './journal/JournalCheckPage';
import PublishReadyPage from './publishready/PublishReadyPage';
import CopilotPage from './copilot/CopilotPage';
import BillingPage from './subscription/BillingPage';
import CommunityPage from './community/CommunityPage';

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
