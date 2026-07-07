// Gaply — auth service (Supabase). Dependency-injected so tests pass a mocked
// client; production callers use the default (shared) client. In OFFLINE MODE
// every call resolves to a safe "no session" result — nothing throws, nothing
// blocks the free-offline features.

import type { Session, SupabaseClient, Subscription } from '@supabase/supabase-js';
import { getSupabase } from './client';

export type OAuthProvider = 'google' | 'orcid';

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
      const { error } = await client.auth.signInWithOAuth({
        // 'orcid' is a custom-provider slug (see interface note); cast needed
        // because supabase-js types only enumerate built-ins.
        provider: provider as 'google',
        options: { redirectTo: window.location.origin },
      });
      return error ? { ok: false, error: error.message } : { ok: true };
    },

    async signOut() {
      if (!client) return OFFLINE;
      const { error } = await client.auth.signOut();
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
