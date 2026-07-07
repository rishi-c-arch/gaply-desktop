// Supabase integration tests — NO real network, NO live Supabase.
//
// Honest scope, same discipline as the backend build:
//  * RLS tests here are POLICY-DEFINITION tests (static assertions over the
//    committed migration SQL: RLS enabled everywhere, owner-private tables
//    uid-scoped on every verb). Actual enforcement is Postgres's job and can
//    only be exercised end-to-end against a live Supabase instance.
//  * Auth-service tests run against a mocked client.
//  * Offline-mode tests prove everything degrades safely with no session/config.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it, vi } from 'vitest';

import { createAuthService } from './auth';
import {
  createAnalysisHistoryService,
  createCitationLibraryService,
  createSubscriptionService,
} from './data';

/* ============================ RLS policy tests ============================ */

const SQL = readFileSync(
  join(__dirname, '../../../supabase/migrations/0001_auth_metadata_schema.sql'),
  'utf8'
).replace(/--[^\n]*/g, '');

const ALL_TABLES = [
  'profiles',
  'subscriptions',
  'analysis_history',
  'citation_library',
  'usage_counters',
  'community_channels',
  'community_posts',
];

/** Owner-private: another user's rows must be invisible on EVERY verb. */
const PRIVATE_TABLES: Array<[table: string, ownerCol: string]> = [
  ['profiles', 'id'],
  ['subscriptions', 'user_id'],
  ['analysis_history', 'user_id'],
  ['citation_library', 'user_id'],
  ['usage_counters', 'user_id'],
];

describe('RLS policy definitions (static; live enforcement needs a real Supabase)', () => {
  it('every table has row-level security enabled', () => {
    for (const table of ALL_TABLES) {
      const re = new RegExp(
        `alter\\s+table\\s+public\\.${table}\\s+enable\\s+row\\s+level\\s+security`,
        'i'
      );
      expect(SQL).toMatch(re);
    }
  });

  it("a user cannot touch another user's rows: private tables are uid-scoped on select/insert/update/delete", () => {
    for (const [table, col] of PRIVATE_TABLES) {
      // SELECT must be USING (auth.uid() = <owner col>)
      const select = new RegExp(
        `create\\s+policy\\s+"${table}_select_own"\\s+on\\s+public\\.${table}\\s+for\\s+select\\s+using\\s+\\(auth\\.uid\\(\\)\\s*=\\s*${col}\\)`,
        'i'
      );
      expect(SQL).toMatch(select);
      // INSERT must be WITH CHECK (auth.uid() = <owner col>)
      const insert = new RegExp(
        `create\\s+policy\\s+"${table}_insert_own"\\s+on\\s+public\\.${table}\\s+for\\s+insert\\s+with\\s+check\\s+\\(auth\\.uid\\(\\)\\s*=\\s*${col}\\)`,
        'i'
      );
      expect(SQL).toMatch(insert);
      // UPDATE + DELETE scoped too
      expect(SQL).toMatch(new RegExp(`"${table}_update_own"[\\s\\S]{0,200}auth\\.uid\\(\\)\\s*=\\s*${col}`, 'i'));
      expect(SQL).toMatch(new RegExp(`"${table}_delete_own"[\\s\\S]{0,200}auth\\.uid\\(\\)\\s*=\\s*${col}`, 'i'));
    }
  });

  it('community tables: shared read is authenticated-only, writes are owner-scoped', () => {
    expect(SQL).toMatch(/community_posts_select_authenticated[\s\S]{0,120}auth\.role\(\)\s*=\s*'authenticated'/i);
    expect(SQL).toMatch(/community_posts_insert_own[\s\S]{0,120}auth\.uid\(\)\s*=\s*user_id/i);
    expect(SQL).toMatch(/community_channels_insert_own[\s\S]{0,120}auth\.uid\(\)\s*=\s*created_by/i);
  });

  it('no manuscript-content columns exist in any table definition', () => {
    // mirrors scripts/check-supabase-schema.js (the check:all gate)
    const forbidden =
      /^(manuscript|full_?text|body|abstract|findings?|excerpt|chunk|paragraph|section|document|raw(_text)?|content)/i;
    const tableRe = /create\s+table\s+(?:if\s+not\s+exists\s+)?public\.(\w+)\s*\(([\s\S]*?)\);/gi;
    let m: RegExpExecArray | null;
    let checked = 0;
    while ((m = tableRe.exec(SQL)) !== null) {
      checked++;
      for (const line of m[2].split(',')) {
        const name = (line.trim().match(/^"?([a-z_][a-z0-9_]*)"?\s/i) || [])[1];
        if (!name || ['primary', 'unique', 'check', 'constraint', 'foreign'].includes(name.toLowerCase())) continue;
        expect(forbidden.test(name), `column "${name}" in "${m[1]}" looks like manuscript content`).toBe(false);
      }
    }
    expect(checked).toBe(ALL_TABLES.length);
  });
});

/* ========================= auth service (mocked) ========================= */

function mockClient() {
  const unsubscribe = vi.fn();
  const auth = {
    signUp: vi.fn().mockResolvedValue({ data: {}, error: null }),
    signInWithPassword: vi.fn().mockResolvedValue({ data: {}, error: null }),
    signInWithOAuth: vi.fn().mockResolvedValue({ data: {}, error: null }),
    signOut: vi.fn().mockResolvedValue({ error: null }),
    getSession: vi.fn().mockResolvedValue({ data: { session: { user: { id: 'u1' } } } }),
    onAuthStateChange: vi.fn().mockReturnValue({ data: { subscription: { unsubscribe } } }),
  };
  return { client: { auth } as any, auth, unsubscribe };
}

describe('auth service (mocked Supabase client)', () => {
  it('signUp/signIn/signOut call through and report ok', async () => {
    const { client, auth } = mockClient();
    const svc = createAuthService(client);

    expect((await svc.signUp('a@b.c', 'pw')).ok).toBe(true);
    expect(auth.signUp).toHaveBeenCalledWith({ email: 'a@b.c', password: 'pw' });

    expect((await svc.signIn('a@b.c', 'pw')).ok).toBe(true);
    expect(auth.signInWithPassword).toHaveBeenCalledWith({ email: 'a@b.c', password: 'pw' });

    expect((await svc.signOut()).ok).toBe(true);
  });

  it('surfaces provider errors instead of throwing', async () => {
    const { client, auth } = mockClient();
    auth.signInWithPassword.mockResolvedValue({ data: {}, error: { message: 'invalid login' } });
    const res = await createAuthService(client).signIn('a@b.c', 'bad');
    expect(res).toEqual({ ok: false, error: 'invalid login' });
  });

  it('signInWithOAuth passes google and the orcid custom-provider slug', async () => {
    const { client, auth } = mockClient();
    const svc = createAuthService(client);
    await svc.signInWithOAuth('google');
    expect(auth.signInWithOAuth.mock.calls[0][0].provider).toBe('google');
    await svc.signInWithOAuth('orcid');
    expect(auth.signInWithOAuth.mock.calls[1][0].provider).toBe('orcid');
  });

  it('getSession returns the session; onAuthStateChange unsubscribes cleanly', async () => {
    const { client, unsubscribe } = mockClient();
    const svc = createAuthService(client);
    const { session, offline } = await svc.getSession();
    expect(offline).toBe(false);
    expect(session).toBeTruthy();
    const off = svc.onAuthStateChange(() => {});
    off();
    expect(unsubscribe).toHaveBeenCalled();
  });
});

/* ============================== offline mode ============================== */

describe('offline mode (no Supabase session or config at all)', () => {
  it('auth service settles to "no session" without throwing', async () => {
    const svc = createAuthService(null);
    expect(await svc.signIn('a@b.c', 'pw')).toEqual({ ok: false, error: 'offline' });
    expect(await svc.getSession()).toEqual({ session: null, offline: true });
    const seen: Array<unknown> = [];
    const off = svc.onAuthStateChange((s) => seen.push(s));
    expect(seen).toEqual([null]); // UI settles immediately to signed-out
    off(); // no-op unsubscribe must not throw
  });

  it('data services return marked offline results, never errors', async () => {
    const history = createAnalysisHistoryService(null);
    const citations = createCitationLibraryService(null);
    expect(await history.list('u1')).toEqual({ data: null, error: null, offline: true });
    expect(await citations.list('u1')).toEqual({ data: null, error: null, offline: true });
  });

  it('subscription tier defaults to FREE offline — free features never need a session', async () => {
    const subs = createSubscriptionService(null);
    const res = await subs.getTier('u1');
    expect(res.offline).toBe(true);
    expect(res.tier).toBe('free');
  });
});
