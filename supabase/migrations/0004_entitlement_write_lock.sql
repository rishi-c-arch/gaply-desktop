-- 0004 — entitlement tables are READ-ONLY to their owner. Only the server writes.
--
-- 0001 gave `subscriptions` and `usage_counters` the same owner-write policies
-- as every other private table: insert, update and delete where
-- auth.uid() = user_id. On these two tables that is a privilege escalation,
-- because the gaply-proxy entitlement gate (gaply-proxy/app/entitlement.py)
-- READS them to decide who may make paid model calls:
--
--   * subscriptions.tier — a signed-in user could set their own row to
--     'premium' with the public anon key and their own JWT.
--   * usage_counters     — a user could delete or lower their own counter and
--     reset the metered quota.
--
-- After this migration a user can SELECT their own row and nothing else.
-- Writes come only from roles that are not end users: the proxy (asyncpg over
-- DATABASE_URL), a future payment webhook using the service role, and the
-- Dashboard SQL editor.
--
-- Three layers, each sufficient on its own for the API path:
--   1. the owner write policies are dropped, so RLS denies every write;
--   2. table write privileges are revoked from anon and authenticated;
--   3. a trigger refuses INSERT, UPDATE and DELETE from those two roles, so a
--      write policy re-added later (for example by re-running an older schema
--      file) still cannot reach the rows.
--
-- The trigger follows community_posts' lock_integrity_badge (0003), extended
-- to INSERT and DELETE. A trigger on UPDATE alone would leave a user free to
-- INSERT a 'premium' row or DELETE a counter, which is the hole being closed.
--
-- Nothing in the app writes either table today: createSubscriptionService only
-- reads, and createUsageService.increment has no production caller.

-- 1. Drop the owner write policies. Select-own stays.
drop policy if exists "subscriptions_insert_own" on public.subscriptions;
drop policy if exists "subscriptions_update_own" on public.subscriptions;
drop policy if exists "subscriptions_delete_own" on public.subscriptions;
drop policy if exists "usage_counters_insert_own" on public.usage_counters;
drop policy if exists "usage_counters_update_own" on public.usage_counters;
drop policy if exists "usage_counters_delete_own" on public.usage_counters;

-- 2. Revoke write privileges from the two API roles end users act as.
revoke insert, update, delete, truncate on public.subscriptions from anon, authenticated;
revoke insert, update, delete, truncate on public.usage_counters from anon, authenticated;

-- 3. Refuse writes from end-user roles even if a policy or grant comes back.
create or replace function public.lock_entitlement_writes()
returns trigger language plpgsql
set search_path = public
as $$
begin
  if current_user in ('anon', 'authenticated') then
    raise exception '% on %.% is server-only', tg_op, tg_table_schema, tg_table_name;
  end if;
  if tg_op = 'DELETE' then
    return old;
  end if;
  return new;
end;
$$;

drop trigger if exists subscriptions_lock_writes on public.subscriptions;
create trigger subscriptions_lock_writes
  before insert or update or delete on public.subscriptions
  for each row execute function public.lock_entitlement_writes();

drop trigger if exists usage_counters_lock_writes on public.usage_counters;
create trigger usage_counters_lock_writes
  before insert or update or delete on public.usage_counters
  for each row execute function public.lock_entitlement_writes();
