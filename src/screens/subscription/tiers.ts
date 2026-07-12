// Gaply — subscription tiers, feature gating, usage caps. Core rule: FREE
// OFFLINE is UNLIMITED and NEVER gated; only ONLINE features are capped for
// free users; premium unlocks the ★ flagships.
export type Tier = 'free' | 'premium';

/** Features that are ONLINE and metered for free users. Offline features are
 *  intentionally absent — they are never capped. */
export const ONLINE_CAPPED: Record<string, number> = {
  citation_verification: 5,
  journal_check: 3,
};

/** ★ Premium-only features (no free access, only a teaser). */
export const PREMIUM_ONLY = new Set([
  'publishready',
  'research_copilot',
  'deep_plagiarism',
  'research_gap_finder',
]);

/** Offline features — ALWAYS allowed, any tier, no session needed. */
export const OFFLINE_FEATURES = new Set([
  'extraction',
  'validation',
  'ai_check',
  'local_plagiarism',
  'report_view',
  'citation_manager_local',
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
  // free user, online capped feature
  const cap = ONLINE_CAPPED[feature];
  if (cap === undefined) {
    // unknown online feature — allow but don't meter
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
