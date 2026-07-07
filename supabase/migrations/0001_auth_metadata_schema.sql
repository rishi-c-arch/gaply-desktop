-- Gaply — Supabase schema: AUTH + METADATA ONLY.
--
-- HARD RULE: the manuscript itself must NEVER touch Supabase. No table in this
-- schema may carry manuscript text, findings bodies, excerpts, chunks or any
-- other document content — only account/subscription/usage metadata and
-- bibliographic citation metadata. Enforced by scripts/check-supabase-schema.js
-- (a check:all gate) which denylists content-bearing column names.
--
-- RLS: enabled on EVERY table. Owner-private tables (profiles, subscriptions,
-- analysis_history, citation_library, usage_counters) are select/insert/update/
-- delete-scoped to auth.uid(). Community tables are readable by any
-- authenticated user but writable only by the row owner.

-- ============================================================================
-- profiles — one row per auth user (id mirrors auth.users.id)
-- ============================================================================
create table if not exists public.profiles (
  id uuid primary key references auth.users (id) on delete cascade,
  email text not null,
  display_name text,
  role text not null default 'researcher',
  field text,
  orcid text,
  country text,
  created_at timestamptz not null default now()
);

alter table public.profiles enable row level security;

create policy "profiles_select_own" on public.profiles
  for select using (auth.uid() = id);
create policy "profiles_insert_own" on public.profiles
  for insert with check (auth.uid() = id);
create policy "profiles_update_own" on public.profiles
  for update using (auth.uid() = id) with check (auth.uid() = id);
create policy "profiles_delete_own" on public.profiles
  for delete using (auth.uid() = id);

-- ============================================================================
-- subscriptions
-- ============================================================================
create table if not exists public.subscriptions (
  user_id uuid primary key references auth.users (id) on delete cascade,
  tier text not null default 'free' check (tier in ('free', 'premium')),
  status text not null default 'active',
  current_period_end timestamptz,
  razorpay_subscription_id text,
  created_at timestamptz not null default now()
);

alter table public.subscriptions enable row level security;

create policy "subscriptions_select_own" on public.subscriptions
  for select using (auth.uid() = user_id);
create policy "subscriptions_insert_own" on public.subscriptions
  for insert with check (auth.uid() = user_id);
create policy "subscriptions_update_own" on public.subscriptions
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy "subscriptions_delete_own" on public.subscriptions
  for delete using (auth.uid() = user_id);

-- ============================================================================
-- analysis_history — METADATA ONLY. certainty_summary_json holds tier COUNTS
-- (e.g. {"certain":2,"assessed":3,"flagged":1}) — never finding bodies, never
-- manuscript text.
-- ============================================================================
create table if not exists public.analysis_history (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users (id) on delete cascade,
  title text not null,
  created_at timestamptz not null default now(),
  certainty_summary_json jsonb not null default '{}'::jsonb,
  tier_used text not null default 'free'
);

alter table public.analysis_history enable row level security;

create policy "analysis_history_select_own" on public.analysis_history
  for select using (auth.uid() = user_id);
create policy "analysis_history_insert_own" on public.analysis_history
  for insert with check (auth.uid() = user_id);
create policy "analysis_history_update_own" on public.analysis_history
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy "analysis_history_delete_own" on public.analysis_history
  for delete using (auth.uid() = user_id);

create index if not exists analysis_history_user_created_idx
  on public.analysis_history (user_id, created_at desc);

-- ============================================================================
-- citation_library — bibliographic metadata only (CSL-JSON is reference
-- metadata, not document content)
-- ============================================================================
create table if not exists public.citation_library (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users (id) on delete cascade,
  doi text,
  title text,
  authors text,
  year int,
  journal text,
  csl_json jsonb,
  retracted_flag boolean not null default false,
  created_at timestamptz not null default now()
);

alter table public.citation_library enable row level security;

create policy "citation_library_select_own" on public.citation_library
  for select using (auth.uid() = user_id);
create policy "citation_library_insert_own" on public.citation_library
  for insert with check (auth.uid() = user_id);
create policy "citation_library_update_own" on public.citation_library
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy "citation_library_delete_own" on public.citation_library
  for delete using (auth.uid() = user_id);

create index if not exists citation_library_user_idx
  on public.citation_library (user_id, created_at desc);

-- ============================================================================
-- usage_counters — per-feature counters per billing period
-- ============================================================================
create table if not exists public.usage_counters (
  user_id uuid not null references auth.users (id) on delete cascade,
  feature text not null,
  count int not null default 0,
  period_start date not null,
  primary key (user_id, feature, period_start)
);

alter table public.usage_counters enable row level security;

create policy "usage_counters_select_own" on public.usage_counters
  for select using (auth.uid() = user_id);
create policy "usage_counters_insert_own" on public.usage_counters
  for insert with check (auth.uid() = user_id);
create policy "usage_counters_update_own" on public.usage_counters
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy "usage_counters_delete_own" on public.usage_counters
  for delete using (auth.uid() = user_id);

-- ============================================================================
-- community_channels — shared read for authenticated users; owner-write.
-- (Deliberate, documented deviation from strict uid-scoping: a community
-- channel list everyone can browse is the point of the feature.)
-- ============================================================================
create table if not exists public.community_channels (
  id uuid primary key default gen_random_uuid(),
  name text not null unique,
  description text,
  created_by uuid not null references auth.users (id) on delete cascade,
  created_at timestamptz not null default now()
);

alter table public.community_channels enable row level security;

create policy "community_channels_select_authenticated" on public.community_channels
  for select using (auth.role() = 'authenticated');
create policy "community_channels_insert_own" on public.community_channels
  for insert with check (auth.uid() = created_by);
create policy "community_channels_update_own" on public.community_channels
  for update using (auth.uid() = created_by) with check (auth.uid() = created_by);
create policy "community_channels_delete_own" on public.community_channels
  for delete using (auth.uid() = created_by);

-- ============================================================================
-- community_posts — shared read for authenticated users; owner-write.
-- ============================================================================
create table if not exists public.community_posts (
  id uuid primary key default gen_random_uuid(),
  channel_id uuid not null references public.community_channels (id) on delete cascade,
  user_id uuid not null references auth.users (id) on delete cascade,
  message text not null,
  created_at timestamptz not null default now()
);

alter table public.community_posts enable row level security;

create policy "community_posts_select_authenticated" on public.community_posts
  for select using (auth.role() = 'authenticated');
create policy "community_posts_insert_own" on public.community_posts
  for insert with check (auth.uid() = user_id);
create policy "community_posts_update_own" on public.community_posts
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy "community_posts_delete_own" on public.community_posts
  for delete using (auth.uid() = user_id);

create index if not exists community_posts_channel_idx
  on public.community_posts (channel_id, created_at desc);
