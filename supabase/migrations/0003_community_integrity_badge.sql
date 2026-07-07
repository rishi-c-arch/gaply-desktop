-- F13 — the integrity badge is attached to every community post BEFORE it
-- publishes and is IMMOVABLE: the author cannot hide or remove it. Stored on the
-- row (metadata: a classification label + optional source ref, never manuscript
-- content) and rendered on every post.
alter table public.community_posts
  add column if not exists integrity_badge text not null default 'human_written'
    check (integrity_badge in ('human_written', 'ai_assisted', 'ai_generated', 'copied')),
  add column if not exists badge_detail text;

-- Immovability: block UPDATEs that would change the badge, even by the owner.
-- (Owner-write RLS from 0001 still allows editing the message; this trigger
--  locks the classification so it can't be tampered with after publish.)
create or replace function public.lock_integrity_badge()
returns trigger language plpgsql as $$
begin
  if new.integrity_badge is distinct from old.integrity_badge then
    raise exception 'integrity_badge is immovable and cannot be changed';
  end if;
  return new;
end;
$$;

drop trigger if exists community_posts_lock_badge on public.community_posts;
create trigger community_posts_lock_badge
  before update on public.community_posts
  for each row execute function public.lock_integrity_badge();
