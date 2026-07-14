// Gaply — subscription tiers, feature gating, usage caps. Core rule: FREE
// OFFLINE is UNLIMITED and NEVER gated; only ONLINE features are capped for
// free users; premium unlocks the ★ flagships.
export type Tier = 'free' | 'premium';

/** Online features that COULD be metered for free users — currently EMPTY.
 *  The previously-advertised caps (citation_verification: 5, journal_check: 3)
 *  were never enforced: nothing ever incremented usage, and a per-user cap
 *  cannot be enforced client-side (a tampered client bypasses any count). Rather
 *  than advertise a phantom limit + a permanently-0 meter, these free-tier
 *  online features are honestly UNLIMITED until server-side metering exists (at
 *  the proxy). Re-adding an entry here re-enables `gateFeature`'s cap logic once
 *  a real, server-enforced meter lands. Offline features are absent by design. */
export const ONLINE_CAPPED: Record<string, number> = {};

/** ★ Premium-only features (no free access, only a teaser).
 *  NOTE: the "Deep plagiarism analysis" lane is deliberately absent here. It is a
 *  disabled coming-soon placeholder (PlagiarismCheckPage, gated by the separate
 *  `deepPlagiarism` dev feature flag) that never consults an entitlement — so its
 *  old `'deep_plagiarism'` key was orphaned (checked by nothing) and was removed.
 *  When that lane actually ships, add its entitlement key back here at wire-up. */
export const PREMIUM_ONLY = new Set([
  'publishready',
  'research_copilot',
  'research_gap_finder',
  'stats_verifier',
  'journal_verification',
]);

/** Offline features — ALWAYS allowed, any tier, no session needed. */
export const OFFLINE_FEATURES = new Set([
  'extraction',
  'validation',
  'ai_check',
  'local_plagiarism',
  'report_view',
  'citation_manager_local',
  'note_creator',
]);

export interface GateResult {
  allowed: boolean;
  /** null when the feature is uncapped (offline, or premium). */
  cap: number | null;
  used: number;
  remaining: number | null;
  reason?: 'premium_only' | 'cap_reached';
  /** True when the UI should surface an upgrade prompt. */
  upsell: boolean;
}

/** Decide whether `feature` may run for a `tier`, given `used` this period.
 *  Offline features are never gated; premium is unlimited on everything it can
 *  access; free users are capped on online features and blocked from ★ ones. */
export function gateFeature(tier: Tier, feature: string, used = 0): GateResult {
  if (OFFLINE_FEATURES.has(feature)) {
    return { allowed: true, cap: null, used: 0, remaining: null, upsell: false };
  }
  if (PREMIUM_ONLY.has(feature)) {
    return tier === 'premium'
      ? { allowed: true, cap: null, used: 0, remaining: null, upsell: false }
      : { allowed: false, cap: null, used: 0, remaining: null, reason: 'premium_only', upsell: true };
  }
  if (tier === 'premium') {
    return { allowed: true, cap: null, used, remaining: null, upsell: false };
  }
  // free user, online feature. ONLINE_CAPPED is currently empty (no enforceable
  // caps), so every online feature falls through here — honestly unlimited. The
  // cap logic below stays RESERVED: it activates only if a real, server-enforced
  // cap is ever added back to ONLINE_CAPPED.
  const cap = ONLINE_CAPPED[feature];
  if (cap === undefined) {
    // uncapped online feature — allow, don't meter
    return { allowed: true, cap: null, used, remaining: null, upsell: false };
  }
  const remaining = Math.max(0, cap - used);
  const allowed = used < cap;
  return {
    allowed,
    cap,
    used,
    remaining,
    reason: allowed ? undefined : 'cap_reached',
    upsell: !allowed || remaining <= 1, // nudge as the cap approaches
  };
}
