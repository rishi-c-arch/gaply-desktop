import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../contexts/AuthContext';
import { apiFetch } from '../api/config';
import { authService } from '../services/authService';

const FEATURE_TO_LEGACY: Record<string, string> = {
  publish_ready_pro: 'gap_finder',
  data_maestro_pro: 'deep_eval',
  journal_verification_pro: 'journal_check',
  research_deep_analysis_pro: 'deep_eval',
};

/**
 * Hook that returns a function to navigate to a premium feature.
 * Enforces: login required, Premium Pro or legacy entitlement required.
 * If not logged in → redirect to login with return URL.
 * If logged in but no access → redirect to /packages.
 * If has access → navigate to feature.
 */
export function usePremiumFeatureNav() {
  const navigate = useNavigate();
  const { isAuthenticated, user } = useAuth();

  const goToPremiumFeature = useCallback(
    async (path: string, featureKey: string) => {
      if (!isAuthenticated || !user) {
        navigate(`/login?redirect=${encodeURIComponent(path)}`);
        return;
      }

      try {
        const res = await apiFetch(
          `/api/v1/premium/access?feature=${encodeURIComponent(featureKey)}`
        );
        const data = await res.json();
        let access = !!data.access;

        if (!access) {
          const legacyKey = FEATURE_TO_LEGACY[featureKey];
          if (legacyKey) {
            const legacy = await authService.checkFeatureAccess(user.id, legacyKey);
            access = legacy.has_access;
          }
        }

        if (access) {
          navigate(path);
        } else {
          navigate('/packages');
        }
      } catch {
        navigate('/packages');
      }
    },
    [isAuthenticated, user, navigate]
  );

  return goToPremiumFeature;
}
