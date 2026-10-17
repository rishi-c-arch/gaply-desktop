// Part C follow-up 2: the desktop Google sign-in opens its URL through the
// app's checked command, `open_external_link`, and that command's length
// limit admits the URL with at least twice its length to spare.
//
// The URL is built by supabase-js itself, as the app builds it (PKCE, the
// gaply:// redirect, the app's query parameters, skipBrowserRedirect), for a
// project URL of the real shape (a 20-character ref). No account, no network:
// supabase-js builds this URL locally, and `fetch` is stubbed to fail the test
// if anything tries to use it. Only the URL's LENGTH is printed, never the URL.
import { readFileSync } from 'fs';
import { createClient } from '@supabase/supabase-js';
import { afterEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn(async (_cmd: string, _args?: unknown) => undefined) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

import { createAuthService } from './auth';

/** `MAX_LINK_CHARS` as `src-tauri/src/external_links.rs` declares it. */
function rustLimit(): number {
  const src = readFileSync('src-tauri/src/external_links.rs', 'utf8');
  const m = src.match(/pub const MAX_LINK_CHARS: usize = (\d+);/);
  if (!m) throw new Error('MAX_LINK_CHARS not found');
  return Number(m[1]);
}

afterEach(() => {
  invoke.mockClear();
  vi.unstubAllGlobals();
});

describe('desktop sign-in', () => {
  it('reaches open_external_link with an https URL the length limit admits twice over', async () => {
    const fetchSpy = vi.fn(() => Promise.reject(new Error('no network in this test')));
    vi.stubGlobal('fetch', fetchSpy);
    const client = createClient(`https://${'a'.repeat(20)}.supabase.co`, 'anon-key-for-tests-only', {
      auth: { persistSession: false, autoRefreshToken: false, flowType: 'pkce', detectSessionInUrl: false, storage: window.localStorage },
    });
    const result = await createAuthService(client).signInWithOAuth('google');
    expect(result).toEqual({ ok: true });

    const opens = invoke.mock.calls.filter((c) => c[0] === 'open_external_link');
    expect(opens).toHaveLength(1);
    expect(invoke.mock.calls.some((c) => String(c[0]).startsWith('plugin:opener'))).toBe(false);
    const url = (opens[0][1] as { url: string }).url;
    expect(url.startsWith('https://')).toBe(true);
    expect(new URL(url).searchParams.get('redirect_to')).toBe('gaply://auth/callback');
    const limit = rustLimit();
    // eslint-disable-next-line no-console
    console.log(`sign-in URL length: ${url.length} characters; limit ${limit}; margin ${(limit / url.length).toFixed(1)}x`);
    expect(url.length * 2).toBeLessThanOrEqual(limit);
    expect(fetchSpy).not.toHaveBeenCalled();
  });
});
