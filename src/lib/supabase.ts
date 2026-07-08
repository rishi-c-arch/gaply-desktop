// Gaply — canonical Supabase import path.
//
// The single, real Supabase client lives in ../services/supabase/client.ts
// (PKCE + Tauri-Store session persistence + offline-mode fallback). This module
// is the short public alias requested by the Phase-1 spec; it deliberately does
// NOT call createClient again — a second client would create a second GoTrue
// instance and corrupt session state. Import from here or from the services
// module; both resolve to the exact same client.
export {
  getSupabase,
  isSupabaseConfigured,
  isOfflineMode,
} from '../services/supabase/client';
export { createAuthService } from '../services/supabase/auth';
export type { AuthService, AuthResult, SessionResult, OAuthProvider } from '../services/supabase/auth';

import { getSupabase } from '../services/supabase/client';

/** Convenience accessor mirroring the spec's `supabase` export. Null in
 *  OFFLINE MODE (no env config) — callers must handle null, exactly as the
 *  services layer does. */
export const supabase = getSupabase();
