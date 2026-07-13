// Gaply — useSubscription(): the single source of truth for the current tier.
// Gated ★ screens read this; free-offline features must NEVER consult it (they
// are always available). Offline mode / no session → free.
import { useEffect, useMemo, useState } from 'react';
import { createSubscriptionService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { gateFeature, GateResult, Tier } from './tiers';

export interface SubscriptionState {
  tier: Tier;
  status: string;
  loading: boolean;
  isPremium: boolean;
  /** Gate a feature by name (reads current tier; caller supplies `used`). */
  gate: (feature: string, used?: number) => GateResult;
}

export function useSubscription(
  subscriptionService?: ReturnType<typeof createSubscriptionService>
): SubscriptionState {
  const { session } = useGaplySession();
  const subs = useMemo(() => subscriptionService ?? createSubscriptionService(), [subscriptionService]);
  const [tier, setTier] = useState<Tier>('free');
  const [status, setStatus] = useState('inactive');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;
    if (!session) {
      setTier('free');
      setStatus('offline');
      setLoading(false);
      return;
    }
    subs.getTier(session.user.id).then((r) => {
      if (!alive) return;
      setTier(r.tier);
      setStatus(r.data?.status ?? (r.tier === 'premium' ? 'active' : 'inactive'));
      setLoading(false);
    });
    return () => {
      alive = false;
    };
  }, [session, subs]);

  return {
    tier,
    status,
    loading,
    isPremium: tier === 'premium',
    gate: (feature, used = 0) => gateFeature(tier, feature, used),
  };
}

// NOTE: the per-feature usage helpers (readUsage / bumpUsage / periodStart) were
// removed with H3. `bumpUsage` had ZERO callers, so usage never incremented and
// the Settings/Billing meters read a permanent 0 — a false "0/5" limit. The
// underlying usage_counters service (`createUsageService`) is left in place,
// reserved for when metering is enforced server-side at the proxy.
