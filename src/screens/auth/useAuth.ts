// Gaply — useAuth: thin action layer over the real Supabase AuthService
// (via the session provider's single client). Every failure surfaces the REAL
// Supabase error through a toast; success is only reported when Supabase
// actually returns ok — no fabricated success.
import { useCallback, useState } from 'react';
import { useToast } from '../../design-system/Toast';
import { useGaplySession } from '../session/SessionProvider';

export interface UseAuth {
  /** null until the first getSession() settles (from the session provider). */
  session: ReturnType<typeof useGaplySession>['session'];
  loading: boolean;
  offline: boolean;
  busy: boolean;
  signInPassword: (email: string, password: string) => Promise<boolean>;
  signUpPassword: (email: string, password: string) => Promise<boolean>;
  signInWithGoogle: () => Promise<boolean>;
  signOut: () => Promise<void>;
}

/** Map the service's 'offline' sentinel to a human message; pass real errors
 *  through verbatim so the user sees what Supabase actually said. */
function humanize(error?: string): string {
  if (error === 'offline') return 'No connection configured — use "Continue offline".';
  return error || 'Something went wrong. Please try again.';
}

export function useAuth(): UseAuth {
  const { auth, session, loading, offline } = useGaplySession();
  const { toast } = useToast();
  const [busy, setBusy] = useState(false);

  const signInPassword = useCallback(
    async (email: string, password: string) => {
      setBusy(true);
      const res = await auth.signIn(email, password);
      setBusy(false);
      if (!res.ok) {
        toast(humanize(res.error), 'flagged');
        return false;
      }
      toast('Signed in', 'certain');
      return true;
    },
    [auth, toast]
  );

  const signUpPassword = useCallback(
    async (email: string, password: string) => {
      setBusy(true);
      const res = await auth.signUp(email, password);
      setBusy(false);
      if (!res.ok) {
        toast(humanize(res.error), 'flagged');
        return false;
      }
      // Supabase may require email confirmation before a session exists; say so
      // honestly rather than implying an active session.
      toast('Account created — check your email to confirm, then sign in.', 'certain');
      return true;
    },
    [auth, toast]
  );

  const signInWithGoogle = useCallback(async () => {
    setBusy(true);
    const res = await auth.signInWithOAuth('google');
    setBusy(false);
    if (!res.ok) {
      toast(humanize(res.error), 'flagged');
      return false;
    }
    // Desktop: the system browser is now open for consent; web: redirecting.
    return true;
  }, [auth, toast]);

  const signOut = useCallback(async () => {
    const res = await auth.signOut();
    if (!res.ok && res.error !== 'offline') toast(humanize(res.error), 'flagged');
  }, [auth, toast]);

  return { session, loading, offline, busy, signInPassword, signUpPassword, signInWithGoogle, signOut };
}
