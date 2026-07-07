// Gaply — typed data-access layer over the metadata tables. Same rules as the
// schema: METADATA ONLY (no manuscript text ever), RLS does the row scoping
// server-side (every query here still runs as the signed-in user). All
// functions are dependency-injected for tests and degrade to a clearly-marked
// offline result when there is no client.

import type { SupabaseClient } from '@supabase/supabase-js';
import { getSupabase } from './client';
import type {
  AnalysisHistoryRow,
  CertaintySummary,
  CitationLibraryRow,
  CommunityChannelRow,
  CommunityPostRow,
  ProfileRow,
  SubscriptionRow,
  UsageCounterRow,
} from './types';

export interface DataResult<T> {
  data: T | null;
  error: string | null;
  offline: boolean;
}

const offline = <T>(): DataResult<T> => ({ data: null, error: null, offline: true });
const wrap = <T>(data: T | null, error: { message: string } | null): DataResult<T> => ({
  data,
  error: error ? error.message : null,
  offline: false,
});

/* ------------------------------- profiles ------------------------------- */

export function createProfileService(client: SupabaseClient | null = getSupabase()) {
  return {
    async getOwn(userId: string): Promise<DataResult<ProfileRow>> {
      if (!client) return offline();
      const { data, error } = await client.from('profiles').select('*').eq('id', userId).maybeSingle();
      return wrap(data as ProfileRow | null, error);
    },
    async upsertOwn(profile: Partial<ProfileRow> & { id: string; email: string }): Promise<DataResult<ProfileRow>> {
      if (!client) return offline();
      const { data, error } = await client.from('profiles').upsert(profile).select().single();
      return wrap(data as ProfileRow | null, error);
    },
  };
}

/* ----------------------------- subscriptions ---------------------------- */

export function createSubscriptionService(client: SupabaseClient | null = getSupabase()) {
  return {
    /** Offline (or no row) means FREE tier — free features never need a session. */
    async getTier(userId: string): Promise<DataResult<SubscriptionRow> & { tier: 'free' | 'premium' }> {
      if (!client) return { ...offline<SubscriptionRow>(), tier: 'free' };
      const { data, error } = await client.from('subscriptions').select('*').eq('user_id', userId).maybeSingle();
      const row = data as SubscriptionRow | null;
      return { ...wrap(row, error), tier: row?.status === 'active' && row.tier === 'premium' ? 'premium' : 'free' };
    },
  };
}

/* ---------------------------- analysis history -------------------------- */

export function createAnalysisHistoryService(client: SupabaseClient | null = getSupabase()) {
  return {
    /** Records METADATA ONLY: a title, tier counts, and which tier ran.
     *  There is deliberately no parameter through which manuscript text or
     *  finding bodies could travel. */
    async record(
      userId: string,
      title: string,
      certaintySummary: CertaintySummary,
      tierUsed: string
    ): Promise<DataResult<AnalysisHistoryRow>> {
      if (!client) return offline();
      const { data, error } = await client
        .from('analysis_history')
        .insert({ user_id: userId, title, certainty_summary_json: certaintySummary, tier_used: tierUsed })
        .select()
        .single();
      return wrap(data as AnalysisHistoryRow | null, error);
    },
    async list(userId: string, limit = 50): Promise<DataResult<AnalysisHistoryRow[]>> {
      if (!client) return offline();
      const { data, error } = await client
        .from('analysis_history')
        .select('*')
        .eq('user_id', userId)
        .order('created_at', { ascending: false })
        .limit(limit);
      return wrap((data ?? null) as AnalysisHistoryRow[] | null, error);
    },
  };
}

/* ---------------------------- citation library -------------------------- */

export function createCitationLibraryService(client: SupabaseClient | null = getSupabase()) {
  return {
    async add(
      row: Omit<CitationLibraryRow, 'id' | 'created_at'>
    ): Promise<DataResult<CitationLibraryRow>> {
      if (!client) return offline();
      const { data, error } = await client.from('citation_library').insert(row).select().single();
      return wrap(data as CitationLibraryRow | null, error);
    },
    async list(userId: string): Promise<DataResult<CitationLibraryRow[]>> {
      if (!client) return offline();
      const { data, error } = await client
        .from('citation_library')
        .select('*')
        .eq('user_id', userId)
        .order('created_at', { ascending: false });
      return wrap((data ?? null) as CitationLibraryRow[] | null, error);
    },
    async remove(id: string): Promise<DataResult<null>> {
      if (!client) return offline();
      const { error } = await client.from('citation_library').delete().eq('id', id);
      return wrap<null>(null, error);
    },
  };
}

/* ------------------------------ usage counters --------------------------- */

export function createUsageService(client: SupabaseClient | null = getSupabase()) {
  return {
    async get(userId: string, feature: string, periodStart: string): Promise<DataResult<UsageCounterRow>> {
      if (!client) return offline();
      const { data, error } = await client
        .from('usage_counters')
        .select('*')
        .eq('user_id', userId)
        .eq('feature', feature)
        .eq('period_start', periodStart)
        .maybeSingle();
      return wrap(data as UsageCounterRow | null, error);
    },
    async increment(userId: string, feature: string, periodStart: string): Promise<DataResult<UsageCounterRow>> {
      if (!client) return offline();
      const current = await this.get(userId, feature, periodStart);
      const next = (current.data?.count ?? 0) + 1;
      const { data, error } = await client
        .from('usage_counters')
        .upsert({ user_id: userId, feature, period_start: periodStart, count: next })
        .select()
        .single();
      return wrap(data as UsageCounterRow | null, error);
    },
  };
}

/* -------------------------------- community ------------------------------ */

export function createCommunityService(client: SupabaseClient | null = getSupabase()) {
  return {
    async listChannels(): Promise<DataResult<CommunityChannelRow[]>> {
      if (!client) return offline();
      const { data, error } = await client.from('community_channels').select('*').order('name');
      return wrap((data ?? null) as CommunityChannelRow[] | null, error);
    },
    async listPosts(channelId: string, limit = 100): Promise<DataResult<CommunityPostRow[]>> {
      if (!client) return offline();
      const { data, error } = await client
        .from('community_posts')
        .select('*')
        .eq('channel_id', channelId)
        .order('created_at', { ascending: false })
        .limit(limit);
      return wrap((data ?? null) as CommunityPostRow[] | null, error);
    },
    async post(channelId: string, userId: string, message: string): Promise<DataResult<CommunityPostRow>> {
      if (!client) return offline();
      const { data, error } = await client
        .from('community_posts')
        .insert({ channel_id: channelId, user_id: userId, message })
        .select()
        .single();
      return wrap(data as CommunityPostRow | null, error);
    },
  };
}
