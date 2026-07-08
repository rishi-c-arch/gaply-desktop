// Gaply — auth service (Supabase). Dependency-injected so tests pass a mocked
// client; production callers use the default (shared) client. In OFFLINE MODE
// every call resolves to a safe "no session" result — nothing throws, nothing
// blocks the free-offline features.

import type { Session, SupabaseClient, Subscription } from '@supabase/supabase-js';
import { getSupabase } from './client';
import { isTauri } from '../../utils/isTauri';

export type OAuthProvider = 'google' | 'orcid';

/** Hosted hand-off page that immediately 302s the browser to the gaply://
 *  deep link. Overridable per-env; defaults to the production domain. */
const AUTH_SUCCESS_URL =
  process.env.REACT_APP_AUTH_SUCCESS_URL || 'https://www.gaply.in/auth-success';

export interface AuthResult {
  ok: boolean;
  /** 'offline' when the app has no Supabase configuration at all. */
  error?: string;
}

export interface SessionResult {
  session: Session | null;
  offline: boolean;
}

export interface AuthService {
  signUp(email: string, password: string): Promise<AuthResult>;
  signIn(email: string, password: string): Promise<AuthResult>;
  /** Google is a native Supabase provider. ORCID is OIDC-compliant but NOT a
   *  Supabase built-in: the project must register it as a custom/third-party
   *  provider (the slug below is what GoTrue will route on). Until that server
   *  side config exists, 'orcid' surfaces the provider error — it never
   *  crashes the app. */
  signInWithOAuth(provider: OAuthProvider): Promise<AuthResult>;
  /** Complete a desktop OAuth deep-link callback: exchange the `?code=` from
   *  gaply://auth/callback for a session. Optional capability — only the desktop
   *  deep-link flow uses it; the real createAuthService always provides it. */
  exchangeCodeForSession?(code: string): Promise<AuthResult>;
  signOut(): Promise<AuthResult>;
  getSession(): Promise<SessionResult>;
  onAuthStateChange(cb: (session: Session | null) => void): () => void;
}

const OFFLINE: AuthResult = { ok: false, error: 'offline' };

export function createAuthService(client: SupabaseClient | null = getSupabase()): AuthService {
  return {
    async signUp(email, password) {
      if (!client) return OFFLINE;
      const { error } = await client.auth.signUp({ email, password });
      return error ? { ok: false, error: error.message } : { ok: true };
    },

    async signIn(email, password) {
      if (!client) return OFFLINE;
      const { error } = await client.auth.signInWithPassword({ email, password });
      return error ? { ok: false, error: error.message } : { ok: true };
    },

    async signInWithOAuth(provider) {
      if (!client) return OFFLINE;
      if (isTauri) {
        // Desktop flow: don't let supabase-js navigate the webview. Ask it to
        // build the provider URL, open it in the SYSTEM browser, and finish via
        // the gaply:// deep-link callback (see useDeepLinkAuth + /auth-success).
        const { data, error } = await client.auth.signInWithOAuth({
          provider: provider as 'google',
          options: {
            skipBrowserRedirect: true,
            redirectTo: AUTH_SUCCESS_URL,
            queryParams: { access_type: 'offline', prompt: 'consent' },
          },
        });
        if (error) return { ok: false, error: error.message };
        if (!data?.url) return { ok: false, error: 'No OAuth URL returned' };
        try {
          const { openUrl } = await import('@tauri-apps/plugin-opener');
          await openUrl(data.url);
          return { ok: true };
        } catch (e) {
          return { ok: false, error: e instanceof Error ? e.message : 'Could not open browser' };
        }
      }
      // Web flow: normal in-page redirect back to this origin.
      const { error } = await client.auth.signInWithOAuth({
        provider: provider as 'google',
        options: { redirectTo: window.location.origin },
      });
      return error ? { ok: false, error: error.message } : { ok: true };
    },

    async exchangeCodeForSession(code) {
      if (!client) return OFFLINE;
      const { error } = await client.auth.exchangeCodeForSession(code);
      return error ? { ok: false, error: error.message } : { ok: true };
    },

    async signOut() {
      if (!client) return OFFLINE;
      // scope 'local' clears only this device's session (no server round-trip
      // that could fail offline / revoke other devices).
      const { error } = await client.auth.signOut({ scope: 'local' });
      return error ? { ok: false, error: error.message } : { ok: true };
    },

    async getSession() {
      if (!client) return { session: null, offline: true };
      const { data } = await client.auth.getSession();
      return { session: data.session ?? null, offline: false };
    },

    onAuthStateChange(cb) {
      if (!client) {
        // offline: report "no session" once so UI settles, nothing to unsubscribe
        cb(null);
        return () => {};
      }
      const { data } = client.auth.onAuthStateChange((_event, session) => cb(session));
      const sub: Subscription = data.subscription;
      return () => sub.unsubscribe();
    },
  };
}
