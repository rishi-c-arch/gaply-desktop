// Gaply — entitlement (Set 8). UX-ONLY, and PRICING-AGNOSTIC by design.
//
// SECURITY BOUNDARY (read this first): everything in this file is
// PRESENTATION. It decides which screen to show — locked, sign-in, upgrade —
// never whether paid cloud work actually runs. THE REAL GATE is server-side
// at the proxy (gaply-proxy/app/entitlement.py): the user's JWT rides every
// paid request (X-Gaply-User-Token) and the proxy verifies entitlement BEFORE
// the Claude call and consumes a use AFTER success. A tampered client can
// repaint this UI; it cannot skip that gate.
//
// PRICING-AGNOSTIC: the only question ever asked is "does this user have an
// available use / active plan?" — the answer comes from server-owned rows
// (subscriptions under RLS). No prices, currencies, or conversions exist here
// or anywhere in the gating path; the upgrade CTA links out to the billing /
// future payment flow, where pricing lives.

import { useEffect, useMemo, useState } from 'react';
import type { Session } from '@supabase/supabase-js';
import { createSubscriptionService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { PREMIUM_ONLY } from './tiers';

export type EntitlementStatus =
  /** Still resolving (first load). */
  | 'checking'
  /** No session at all — prompt to sign in. */
  | 'signed_out'
  /** Server says: active plan / available use. Proceed (the proxy re-checks). */
  | 'entitled'
  /** Server says: no active plan or uses — show the upgrade path. */
  | 'not_entitled'
  /** Signed in (cached session) but the server is unreachable — we honestly
   *  cannot verify the plan. NOT the same as not_entitled: never show a
   *  premium user an upsell just because they're offline. */
  | 'offline_unverified';

export interface Entitlement {
  status: EntitlementStatus;
  /** Honest human-readable context for non-entitled/offline states. */
  reason?: string;
}

/** Ask the SERVER whether this user is entitled to `feature`. The subscription
 *  row is server-owned (RLS) — the client never computes entitlement from a
 *  local number. Network/config failure → honest 'offline_unverified'. */
export async function checkEntitlement(
  session: Session | null,
  feature: string,
  subs = createSubscriptionService()
): Promise<Entitlement> {
  if (!session) {
    return { status: 'signed_out', reason: 'sign in to use Gaply' };
  }
  if (!PREMIUM_ONLY.has(feature)) {
    // Non-premium features aren't entitlement-gated (they may be metered
    // elsewhere); a session is all they need.
    return { status: 'entitled' };
  }
  const res = await subs.getTier(session.user.id).catch(() => null);
  if (res === null || res.offline || res.error) {
    return {
      status: 'offline_unverified',
      reason: 'your plan can’t be verified right now (offline or server unreachable)',
    };
  }
  return res.tier === 'premium'
    ? { status: 'entitled' }
    : { status: 'not_entitled', reason: 'no active plan or available uses' };
}

/** React hook form. `force` is a TEST/story seam that skips the server ask. */
export function useEntitlement(
  feature: string,
  subscriptionService?: ReturnType<typeof createSubscriptionService>,
  force?: EntitlementStatus
): Entitlement {
  const { session, loading } = useGaplySession();
  const subs = useMemo(
    () => subscriptionService ?? createSubscriptionService(),
    [subscriptionService]
  );
  const [ent, setEnt] = useState<Entitlement>({ status: force ?? 'checking' });

  useEffect(() => {
    if (force) return;
    if (loading) return;
    let alive = true;
    checkEntitlement(session, feature, subs).then((e) => {
      if (alive) setEnt(e);
    });
    return () => {
      alive = false;
    };
  }, [force, loading, session, feature, subs]);

  return force ? { status: force } : ent;
}
