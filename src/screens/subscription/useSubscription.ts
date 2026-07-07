// Gaply — useSubscription(): the single source of truth for the current tier.
// Gated ★ screens read this; free-offline features must NEVER consult it (they
// are always available). Offline mode / no session → free.
import { useEffect, useMemo, useState } from 'react';
import { createSubscriptionService, createUsageService } from '../../services/supabase';
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

/** Read the current period's usage for a metered online feature. Returns the
 *  used count (0 offline / no session). */
export async function readUsage(
  userId: string | undefined,
  feature: string,
  usageService = createUsageService()
): Promise<number> {
  if (!userId) return 0;
  const period = periodStart();
  const res = await usageService.get(userId, feature, period);
  return res.data?.count ?? 0;
}

/** Increment usage for a metered online feature (call after a successful run). */
export async function bumpUsage(
  userId: string | undefined,
  feature: string,
  usageService = createUsageService()
): Promise<void> {
  if (!userId) return;
  await usageService.increment(userId, feature, periodStart());
}

/** First day of the current month, ISO date — the usage_counters period key. */
export function periodStart(now: Date = new Date()): string {
  return `${now.getUTCFullYear()}-${String(now.getUTCMonth() + 1).padStart(2, '0')}-01`;
}
