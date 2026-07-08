-- ============================================================================
-- Gaply — CONSOLIDATED, IDEMPOTENT schema (merges migrations 0001 + 0002 + 0003)
-- ============================================================================
-- Run this ONCE in the Supabase Dashboard → SQL Editor (see PHASE-1 report for
-- copy-paste steps). Safe to re-run: every statement is idempotent, so running
-- it on a fresh project OR on one that already has part of the schema converges
-- to the same end state without errors.
--
-- The SQL Editor executes as the elevated postgres/service role, which is
-- required for DDL that touches RLS, policies, triggers, functions, and the
-- auth.users foreign keys. The app's anon (publishable) key can NOT run this.
--
-- HARD RULE (unchanged): AUTH + METADATA ONLY. No table here may carry
-- manuscript text, finding bodies, excerpts, or document chunks.
--
-- Ordering is deliberate: tables → column back-fills → indexes → functions →
-- triggers → RLS enable → policies. Rationale / failure modes guarded against:
--   * Policies are created LAST, each preceded by `drop policy if exists` —
--     Postgres has no `create policy if not exists`, so a re-run would error on
--     an existing policy; dropping first makes re-creation idempotent.
--   * RLS is enabled immediately BEFORE the policies for each table, never left
--     enabled without policies (which would lock every row out). Because the
--     whole script runs in one transaction-like session, tables are never
--     exposed policy-less to real traffic.
--   * The badge-lock trigger uses `create or replace function` +
--     `drop trigger if exists` before `create trigger` so re-running never
--     duplicates the trigger or errors on an existing one.
--   * Columns added by later migrations (target_journals, integrity_badge,
--     badge_detail) are both defined inline in the CREATE TABLE (fresh install)
--     AND re-asserted via `add column if not exists` (upgrade from a 0001-only
--     database) — so either starting point converges.

-- ---------------------------------------------------------------------------
-- 1. TABLES
-- ---------------------------------------------------------------------------
create table if not exists public.profiles (
  id uuid primary key references auth.users (id) on delete cascade,
  email text not null,
  display_name text,
  role text not null default 'researcher',
  field text,
  orcid text,
  country text,
  target_journals text,
  created_at timestamptz not null default now()
);

create table if not exists public.subscriptions (
  user_id uuid primary key references auth.users (id) on delete cascade,
  tier text not null default 'free' check (tier in ('free', 'premium')),
  status text not null default 'active',
  current_period_end timestamptz,
  razorpay_subscription_id text,
  created_at timestamptz not null default now()
);

create table if not exists public.analysis_history (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users (id) on delete cascade,
  title text not null,
  created_at timestamptz not null default now(),
  certainty_summary_json jsonb not null default '{}'::jsonb,
  tier_used text not null default 'free'
);

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

create table if not exists public.usage_counters (
  user_id uuid not null references auth.users (id) on delete cascade,
  feature text not null,
  count int not null default 0,
  period_start date not null,
  primary key (user_id, feature, period_start)
);

create table if not exists public.community_channels (
  id uuid primary key default gen_random_uuid(),
  name text not null unique,
  description text,
  created_by uuid not null references auth.users (id) on delete cascade,
  created_at timestamptz not null default now()
);

create table if not exists public.community_posts (
  id uuid primary key default gen_random_uuid(),
  channel_id uuid not null references public.community_channels (id) on delete cascade,
  user_id uuid not null references auth.users (id) on delete cascade,
  message text not null,
  integrity_badge text not null default 'human_written'
    check (integrity_badge in ('human_written', 'ai_assisted', 'ai_generated', 'copied')),
  badge_detail text,
  created_at timestamptz not null default now()
);

-- ---------------------------------------------------------------------------
-- 2. COLUMN BACK-FILLS (upgrade a database that only ran 0001)
-- ---------------------------------------------------------------------------
alter table public.profiles
  add column if not exists target_journals text;

alter table public.community_posts
  add column if not exists integrity_badge text not null default 'human_written'
    check (integrity_badge in ('human_written', 'ai_assisted', 'ai_generated', 'copied')),
  add column if not exists badge_detail text;

-- ---------------------------------------------------------------------------
-- 3. INDEXES
-- ---------------------------------------------------------------------------
create index if not exists analysis_history_user_created_idx
  on public.analysis_history (user_id, created_at desc);
create index if not exists citation_library_user_idx
  on public.citation_library (user_id, created_at desc);
create index if not exists community_posts_channel_idx
  on public.community_posts (channel_id, created_at desc);

-- ---------------------------------------------------------------------------
-- 4. FUNCTIONS
-- ---------------------------------------------------------------------------
create or replace function public.lock_integrity_badge()
returns trigger language plpgsql as $$
begin
  if new.integrity_badge is distinct from old.integrity_badge then
    raise exception 'integrity_badge is immovable and cannot be changed';
  end if;
  return new;
end;
$$;

-- ---------------------------------------------------------------------------
-- 5. TRIGGERS
-- ---------------------------------------------------------------------------
drop trigger if exists community_posts_lock_badge on public.community_posts;
create trigger community_posts_lock_badge
  before update on public.community_posts
  for each row execute function public.lock_integrity_badge();

-- ---------------------------------------------------------------------------
-- 6. ENABLE ROW LEVEL SECURITY (idempotent; no-op if already enabled)
-- ---------------------------------------------------------------------------
alter table public.profiles           enable row level security;
alter table public.subscriptions      enable row level security;
alter table public.analysis_history   enable row level security;
alter table public.citation_library   enable row level security;
alter table public.usage_counters     enable row level security;
alter table public.community_channels enable row level security;
alter table public.community_posts    enable row level security;

-- ---------------------------------------------------------------------------
-- 7. POLICIES (drop-then-create so re-runs are idempotent)
-- ---------------------------------------------------------------------------
-- profiles (owner-private)
drop policy if exists "profiles_select_own" on public.profiles;
create policy "profiles_select_own" on public.profiles
  for select using (auth.uid() = id);
drop policy if exists "profiles_insert_own" on public.profiles;
create policy "profiles_insert_own" on public.profiles
  for insert with check (auth.uid() = id);
drop policy if exists "profiles_update_own" on public.profiles;
create policy "profiles_update_own" on public.profiles
  for update using (auth.uid() = id) with check (auth.uid() = id);
drop policy if exists "profiles_delete_own" on public.profiles;
create policy "profiles_delete_own" on public.profiles
  for delete using (auth.uid() = id);

-- subscriptions (owner-private)
drop policy if exists "subscriptions_select_own" on public.subscriptions;
create policy "subscriptions_select_own" on public.subscriptions
  for select using (auth.uid() = user_id);
drop policy if exists "subscriptions_insert_own" on public.subscriptions;
create policy "subscriptions_insert_own" on public.subscriptions
  for insert with check (auth.uid() = user_id);
drop policy if exists "subscriptions_update_own" on public.subscriptions;
create policy "subscriptions_update_own" on public.subscriptions
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
drop policy if exists "subscriptions_delete_own" on public.subscriptions;
create policy "subscriptions_delete_own" on public.subscriptions
  for delete using (auth.uid() = user_id);

-- analysis_history (owner-private)
drop policy if exists "analysis_history_select_own" on public.analysis_history;
create policy "analysis_history_select_own" on public.analysis_history
  for select using (auth.uid() = user_id);
drop policy if exists "analysis_history_insert_own" on public.analysis_history;
create policy "analysis_history_insert_own" on public.analysis_history
  for insert with check (auth.uid() = user_id);
drop policy if exists "analysis_history_update_own" on public.analysis_history;
create policy "analysis_history_update_own" on public.analysis_history
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
drop policy if exists "analysis_history_delete_own" on public.analysis_history;
create policy "analysis_history_delete_own" on public.analysis_history
  for delete using (auth.uid() = user_id);

-- citation_library (owner-private)
drop policy if exists "citation_library_select_own" on public.citation_library;
create policy "citation_library_select_own" on public.citation_library
  for select using (auth.uid() = user_id);
drop policy if exists "citation_library_insert_own" on public.citation_library;
create policy "citation_library_insert_own" on public.citation_library
  for insert with check (auth.uid() = user_id);
drop policy if exists "citation_library_update_own" on public.citation_library;
create policy "citation_library_update_own" on public.citation_library
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
drop policy if exists "citation_library_delete_own" on public.citation_library;
create policy "citation_library_delete_own" on public.citation_library
  for delete using (auth.uid() = user_id);

-- usage_counters (owner-private)
drop policy if exists "usage_counters_select_own" on public.usage_counters;
create policy "usage_counters_select_own" on public.usage_counters
  for select using (auth.uid() = user_id);
drop policy if exists "usage_counters_insert_own" on public.usage_counters;
create policy "usage_counters_insert_own" on public.usage_counters
  for insert with check (auth.uid() = user_id);
drop policy if exists "usage_counters_update_own" on public.usage_counters;
create policy "usage_counters_update_own" on public.usage_counters
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
drop policy if exists "usage_counters_delete_own" on public.usage_counters;
create policy "usage_counters_delete_own" on public.usage_counters
  for delete using (auth.uid() = user_id);

-- community_channels (authenticated-read, owner-write)
drop policy if exists "community_channels_select_authenticated" on public.community_channels;
create policy "community_channels_select_authenticated" on public.community_channels
  for select using (auth.role() = 'authenticated');
drop policy if exists "community_channels_insert_own" on public.community_channels;
create policy "community_channels_insert_own" on public.community_channels
  for insert with check (auth.uid() = created_by);
drop policy if exists "community_channels_update_own" on public.community_channels;
create policy "community_channels_update_own" on public.community_channels
  for update using (auth.uid() = created_by) with check (auth.uid() = created_by);
drop policy if exists "community_channels_delete_own" on public.community_channels;
create policy "community_channels_delete_own" on public.community_channels
  for delete using (auth.uid() = created_by);

-- community_posts (authenticated-read, owner-write; badge locked by trigger)
drop policy if exists "community_posts_select_authenticated" on public.community_posts;
create policy "community_posts_select_authenticated" on public.community_posts
  for select using (auth.role() = 'authenticated');
drop policy if exists "community_posts_insert_own" on public.community_posts;
create policy "community_posts_insert_own" on public.community_posts
  for insert with check (auth.uid() = user_id);
drop policy if exists "community_posts_update_own" on public.community_posts;
create policy "community_posts_update_own" on public.community_posts
  for update using (auth.uid() = user_id) with check (auth.uid() = user_id);
drop policy if exists "community_posts_delete_own" on public.community_posts;
create policy "community_posts_delete_own" on public.community_posts
  for delete using (auth.uid() = user_id);

-- Done. Verify in Dashboard → Authentication → Policies and Table Editor.
