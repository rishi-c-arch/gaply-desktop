import { useEffect, useState, useCallback } from 'react';
import { apiFetch } from '../api/config';
import { authService } from '../services/authService';

export interface PremiumSessionState {
  access: boolean;
  creditsRemaining: number;
  session: {
    session_token: string;
    status: string;
    input_payload: unknown;
    result_payload: unknown;
    credit_deducted: boolean;
  } | null;
  loading: boolean;
  error: string | null;
}

const FEATURE_TO_LEGACY: Record<string, string> = {
  publish_ready_pro: 'gap_finder',
  data_maestro_pro: 'deep_eval',
  journal_verification_pro: 'journal_check',
  research_deep_analysis_pro: 'deep_eval',
};

/**
 * Hook for Premium Pro session restore and access control.
 * Checks Premium Pro first, then legacy entitlements (backward compat).
 * - GET /api/v1/premium/access?feature=<key>
 * - GET /api/v1/session/resume?feature=<key>
 * - Listens to popstate for back-button resume
 */
export function usePremiumSession(featureKey: string): PremiumSessionState & {
  abandonSession: (sessionToken: string) => Promise<void>;
  refreshSession: () => Promise<void>;
} {
  const [state, setState] = useState<PremiumSessionState>({
    access: false,
    creditsRemaining: 0,
    session: null,
    loading: true,
    error: null,
  });

  const fetchAccess = useCallback(async () => {
    try {
      const res = await apiFetch(`/api/v1/premium/access?feature=${encodeURIComponent(featureKey)}`);
      const data = await res.json();
      let access = !!data.access;
      let creditsRemaining = data.credits_remaining ?? 0;
      if (!access) {
        const legacyKey = FEATURE_TO_LEGACY[featureKey];
        if (legacyKey) {
          const user = authService.getCurrentUser();
          if (user) {
            const legacy = await authService.checkFeatureAccess(user.id, legacyKey);
            if (legacy.has_access) {
              access = true;
              creditsRemaining = legacy.remaining_uses;
            }
          }
        }
      }
      return { access, creditsRemaining };
    } catch {
      return { access: false, creditsRemaining: 0 };
    }
  }, [featureKey]);

  const fetchResume = useCallback(async () => {
    try {
      const res = await apiFetch(`/api/v1/session/resume?feature=${encodeURIComponent(featureKey)}`);
      const data = await res.json();
      const sess = data.session;
      return sess && sess.session_token ? sess : null;
    } catch {
      return null;
    }
  }, [featureKey]);

  const refreshSession = useCallback(async () => {
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const [accessData, sessionData] = await Promise.all([fetchAccess(), fetchResume()]);
      setState({
        access: accessData.access,
        creditsRemaining: accessData.creditsRemaining,
        session: sessionData,
        loading: false,
        error: null,
      });
    } catch (err) {
      setState({
        access: false,
        creditsRemaining: 0,
        session: null,
        loading: false,
        error: err instanceof Error ? err.message : 'Failed to load session',
      });
    }
  }, [fetchAccess, fetchResume]);

  useEffect(() => {
    refreshSession();
  }, [refreshSession]);

  useEffect(() => {
    const handlePopState = () => {
      fetchResume().then((sess) => {
        if (sess) {
          setState((prev) => ({ ...prev, session: sess }));
        }
      });
    };
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, [fetchResume]);

  const abandonSession = useCallback(
    async (sessionToken: string) => {
      try {
        await apiFetch(`/api/v1/session/${sessionToken}`, { method: 'DELETE' });
        setState((prev) => ({ ...prev, session: null }));
      } catch {
        setState((prev) => ({ ...prev, session: null }));
      }
    },
    []
  );

  return { ...state, abandonSession, refreshSession };
}
