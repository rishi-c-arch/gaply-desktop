// Gaply — Supabase client bootstrap.
//
// Configuration comes ONLY from env (CRA build-time env vars); nothing is ever
// hardcoded — the check:all secret-scanner gate also watches src/ for key
// material. When the env vars are absent the app runs in OFFLINE MODE: every
// free-offline feature works with no Supabase session (and no network), and
// all services in this folder degrade gracefully instead of throwing.

import { createClient, SupabaseClient } from '@supabase/supabase-js';
import { isTauri } from '../../utils/isTauri';
import { tauriSessionStorage } from './tauriStorage';

const url = process.env.REACT_APP_SUPABASE_URL || '';
const anonKey = process.env.REACT_APP_SUPABASE_ANON_KEY || '';

let cached: SupabaseClient | null | undefined;

/** True when Supabase env config is present. */
export function isSupabaseConfigured(): boolean {
  return Boolean(url && anonKey);
}

/** True when the app must run with no Supabase at all (no env config). All
 *  free-offline features function fully in this mode. */
export function isOfflineMode(): boolean {
  return !isSupabaseConfigured();
}

/** The shared client, or null in offline mode. Never throws. */
export function getSupabase(): SupabaseClient | null {
  if (cached !== undefined) return cached;
  cached = isSupabaseConfigured()
    ? createClient(url, anonKey, {
        auth: {
          persistSession: true,
          autoRefreshToken: true,
          // PKCE everywhere: required for the desktop deep-link OAuth flow
          // (we manually exchangeCodeForSession from the gaply:// callback) and
          // harmless for email/password.
          flowType: 'pkce',
          // In Tauri we handle the OAuth callback ourselves via a deep link, so
          // the client must NOT try to auto-parse a code from the URL. On the
          // web build we keep auto-detection so a redirect callback still works.
          detectSessionInUrl: !isTauri,
          // Desktop: persist the session in a Tauri Store file (origin-stable
          // across dev/prod webview origins). Web: default localStorage.
          ...(isTauri ? { storage: tauriSessionStorage } : {}),
        },
      })
    : null;
  return cached;
}

/** Test seam: replace or clear the cached client. */
export function __setSupabaseForTests(client: SupabaseClient | null | undefined): void {
  cached = client;
}
