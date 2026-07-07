// Gaply — Supabase row types. AUTH + METADATA ONLY: no type here may carry
// manuscript text, finding bodies, or any document content (enforced at the
// schema level by scripts/check-supabase-schema.js).

export type SubscriptionTier = 'free' | 'premium';

export interface ProfileRow {
  id: string;
  email: string;
  display_name: string | null;
  role: string;
  field: string | null;
  orcid: string | null;
  country: string | null;
  /** Comma-separated journal names (metadata; onboarding step 2). */
  target_journals: string | null;
  created_at: string;
}

export interface SubscriptionRow {
  user_id: string;
  tier: SubscriptionTier;
  status: string;
  current_period_end: string | null;
  razorpay_subscription_id: string | null;
  created_at: string;
}

/** METADATA ONLY — certainty_summary_json is tier COUNTS
 *  (e.g. {certain: 2, assessed: 3, flagged: 1}), never finding bodies. */
export interface AnalysisHistoryRow {
  id: string;
  user_id: string;
  title: string;
  created_at: string;
  certainty_summary_json: CertaintySummary;
  tier_used: string;
}

export interface CertaintySummary {
  certain?: number;
  assessed?: number;
  flagged?: number;
  reconsidered?: number;
}

export interface CitationLibraryRow {
  id: string;
  user_id: string;
  doi: string | null;
  title: string | null;
  authors: string | null;
  year: number | null;
  journal: string | null;
  csl_json: Record<string, unknown> | null;
  retracted_flag: boolean;
  created_at: string;
}

export interface UsageCounterRow {
  user_id: string;
  feature: string;
  count: number;
  period_start: string; // ISO date (period bucket)
}

export interface CommunityChannelRow {
  id: string;
  name: string;
  description: string | null;
  created_by: string;
  created_at: string;
}

export type IntegrityBadge = 'human_written' | 'ai_assisted' | 'ai_generated' | 'copied';

export interface CommunityPostRow {
  id: string;
  channel_id: string;
  user_id: string;
  message: string;
  /** IMMOVABLE integrity badge, attached before publish (F13). */
  integrity_badge: IntegrityBadge;
  badge_detail: string | null;
  created_at: string;
}
