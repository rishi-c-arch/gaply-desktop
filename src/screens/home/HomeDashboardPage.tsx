// Gaply — app home (/app). The home page IS the "Academic Research Platform"
// hero: the same design the public site uses, rendered in-app via AppHomeHero.
// FREE-OFFLINE route: never requires a session.
//
// `summaryToRing` stays exported (a pure helper used elsewhere/in tests); the
// props are kept for call-site compatibility even though the hero home does
// not consume the metadata services.
import React, { useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { BadgeStatus } from '../../design-system';
import { createAnalysisHistoryService, createSubscriptionService, CertaintySummary } from '../../services/supabase';
import AppHomeHero from './AppHomeHero';

/* --------------------- certainty summary → ring score --------------------- */

export function summaryToRing(s: CertaintySummary): { score: number; status: BadgeStatus } {
  const certain = s.certain ?? 0;
  const assessed = s.assessed ?? 0;
  const flagged = s.flagged ?? 0;
  const total = certain + assessed + flagged;
  const score = total > 0 ? Math.round((certain / total) * 100) : 0;
  const status: BadgeStatus = flagged > 0 ? 'flagged' : assessed > 0 ? 'assessed' : 'certain';
  return { score, status };
}

export interface HomeDashboardPageProps {
  /** Kept for call-site compatibility (the hero home does not use them). */
  historyService?: ReturnType<typeof createAnalysisHistoryService>;
  subscriptionService?: ReturnType<typeof createSubscriptionService>;
}

const HomeDashboardPage: React.FC<HomeDashboardPageProps> = () => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  // Preserve onboarding's "Scan a sample manuscript" (?sample=1) → upload flow.
  useEffect(() => {
    if (searchParams.get('sample') === '1') {
      navigate('/app/upload?sample=1', { replace: true });
    }
  }, [searchParams, navigate]);

  return <AppHomeHero />;
};

export default HomeDashboardPage;
