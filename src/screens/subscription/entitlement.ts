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
  | 'offline_unverified'
  /** **There is no auth server to sign in to.** The build carries no Supabase
   *  configuration at all, so no session can ever exist and no plan can ever be
   *  read — which is a fact about the BUILD, not about the user.
   *
   *  # Why this is not `signed_out`
   *
   *  `signed_out` means *"sign in"*, and a screen acting on it shows a button to
   *  `/auth`. On a build with no auth server that button returns the user to a
   *  login screen that cannot work, and they arrive back here — a loop whose
   *  every step is honest and whose sum is a lie.
   *
   *  # Why it was missing, recorded so it is not re-introduced
   *
   *  `RequireAuth` has enumerated exactly this case since the commit that
   *  created both gates (6e69d65, 11 Jul 2026), with the reasoning in the file:
   *  *"there is no auth server to sign in against, so requiring login would
   *  brick the build"*. That commit's message lists FOUR entitlement states and
   *  none of them is this one. The datum was in the session context
   *  (`SessionProvider`'s `offline`) and simply not read here — the signature of
   *  a case never considered rather than one considered and rejected.
   *
   *  # It cannot leak paid work
   *
   *  A build with no Supabase config also has no App Check signing key and a
   *  loopback proxy URL, so it cannot reach `/verify` at all. The REAL gate is
   *  unchanged and unreachable; what this unlocks is the local work that was
   *  always free. */
  | 'unverifiable_no_account';

export interface Entitlement {
  status: EntitlementStatus;
  /** Honest human-readable context for non-entitled/offline states. */
  reason?: string;
}

/** Ask the SERVER whether this user is entitled to `feature`. The subscription
 *  row is server-owned (RLS) — the client never computes entitlement from a
 *  local number. Network/config failure → honest 'offline_unverified'.
 *
 *  `noAuthServer` is the session provider's `offline` — TRUE only when the build
 *  carries no Supabase configuration at all, which is the exact condition
 *  `RequireAuth` already tests. It is NOT "the network is down" and NOT "the
 *  user is signed out"; both of those keep their existing states.
 *
 *  **It defaults to `false`, and the direction of that default is the point.**
 *  A caller that forgets it gets today's behaviour — `signed_out`, the
 *  RESTRICTIVE answer — so the flag can only ever be forgotten into the strict
 *  side. (`pipeline.rs`'s `allow_network` records the opposite hazard: a consent
 *  flag defaulted into the permissive side has a way of staying there.) */
export async function checkEntitlement(
  session: Session | null,
  feature: string,
  subs = createSubscriptionService(),
  noAuthServer = false
): Promise<Entitlement> {
  if (!session) {
    // Order matters: BOTH are true on an unconfigured build (no client means
    // getSession() can only ever return a null session), so asking "is there a
    // server" first is what keeps `signed_out` meaning *sign in* on a build
    // where signing in is possible.
    if (noAuthServer) {
      return {
        status: 'unverifiable_no_account',
        reason: 'this build has no account service configured',
      };
    }
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
  // `offline` is the datum this hook did not read for two months. See the
  // `unverifiable_no_account` doc comment.
  const { session, loading, offline } = useGaplySession();
  const subs = useMemo(
    () => subscriptionService ?? createSubscriptionService(),
    [subscriptionService]
  );
  const [ent, setEnt] = useState<Entitlement>({ status: force ?? 'checking' });

  useEffect(() => {
    if (force) return;
    if (loading) return;
    let alive = true;
    checkEntitlement(session, feature, subs, offline).then((e) => {
      if (alive) setEnt(e);
    });
    return () => {
      alive = false;
    };
  }, [force, loading, session, feature, subs, offline]);

  return force ? { status: force } : ent;
}
