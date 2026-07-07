-- Onboarding step 2 captures "typical target journals" — bibliographic
-- METADATA (journal names only), stored on the user's own profile row and
-- covered by the profiles RLS policies from 0001.
alter table public.profiles
  add column if not exists target_journals text;
