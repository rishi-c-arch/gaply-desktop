// Gaply — auth service (Supabase). Dependency-injected so tests pass a mocked
// client; production callers use the default (shared) client. In OFFLINE MODE
// every call resolves to a safe "no session" result — nothing throws, nothing
// blocks the free-offline features.

import type { Session, SupabaseClient, Subscription } from '@supabase/supabase-js';
import { getSupabase } from './client';
import { isTauri } from '../../utils/isTauri';

export type OAuthProvider = 'google' | 'orcid';

/** Desktop Google OAuth redirect target. Defaults to the custom scheme DIRECTLY
 *  (gaply://auth/callback) — no website middleman: Supabase's redirect allow-list
 *  accepts custom URI schemes, and useDeepLinkAuth completes the session from
 *  exactly this URL. Overridable per-env (REACT_APP_AUTH_SUCCESS_URL) to route
 *  through a hosted /auth-success bounce page instead, but the default needs no
 *  external website to be reachable/current. */
const DESKTOP_OAUTH_REDIRECT =
  process.env.REACT_APP_AUTH_SUCCESS_URL || 'gaply://auth/callback';

/** Diagnostic-only (desktop): record whether the PKCE code-verifier is present
 *  in storage at a given OAuth stage — a redacted boolean, logged via the Rust
 *  tracing infra (terminal/Console). Two calls bracket the browser hop:
 *  'after_signin' (write side) and 'before_exchange' (read side), which together
 *  pin a "verifier not found" failure. No-op on web or on any failure. */
async function logAuthProbe(stage: 'after_signin' | 'before_exchange'): Promise<void> {
  if (!isTauri) return;
  try {
    const { hasCodeVerifier } = await import('./tauriStorage');
    const verifierPresent = await hasCodeVerifier();
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('log_auth_probe', { stage, verifierPresent });
  } catch {
    /* probe is best-effort — never affects the auth flow */
  }
}

export interface AuthResult {
  ok: boolean;
  /** 'offline' when the app has no Supabase configuration at all. */
  error?: string;
}

export interface SessionResult {
  session: Session | null;
  offline: boolean;
}

/** Optional researcher identity captured at signup. Stored as Supabase
 *  user_metadata (works with no session, i.e. even when email confirmation is
 *  required); the profiles row is materialized from it on the first session. */
export interface SignUpMeta {
  displayName?: string;
  role?: string;
}

export interface AuthService {
  signUp(email: string, password: string, meta?: SignUpMeta): Promise<AuthResult>;
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
    async signUp(email, password, meta) {
      if (!client) return OFFLINE;
      // Only send the keys the user actually filled — never fabricate fields.
      const data: Record<string, string> = {};
      if (meta?.displayName?.trim()) data.display_name = meta.displayName.trim();
      if (meta?.role?.trim()) data.role = meta.role.trim();
      const { error } = await client.auth.signUp({
        email,
        password,
        ...(Object.keys(data).length ? { options: { data } } : {}),
      });
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
        // the gaply:// deep-link callback directly (see useDeepLinkAuth).
        const { data, error } = await client.auth.signInWithOAuth({
          provider: provider as 'google',
          options: {
            skipBrowserRedirect: true,
            redirectTo: DESKTOP_OAUTH_REDIRECT,
            queryParams: { access_type: 'offline', prompt: 'consent' },
          },
        });
        if (error) return { ok: false, error: error.message };
        if (!data?.url) return { ok: false, error: 'No OAuth URL returned' };
        // Probe (write side): did the PKCE verifier actually persist before we
        // hand the flow off to the system browser?
        await logAuthProbe('after_signin');
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
      // Probe (read side): is the verifier still present right before we try to
      // exchange? (write side vs here pins where it was lost.)
      await logAuthProbe('before_exchange');
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
