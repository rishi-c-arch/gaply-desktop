// Gaply — session context for the desktop-app screens (auth/onboarding/app).
// Deliberately separate from the marketing site's AuthContext: this one wraps
// the F2 Supabase auth service, knows about OFFLINE MODE, and is dependency-
// injectable for tests. Free-offline routes never require a session; online
// routes are wrapped in <RequireSession>.

import React, { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import type { Session } from '@supabase/supabase-js';
import { createAuthService, AuthService } from '../../services/supabase';

export interface GaplySession {
  /** null = signed out (or offline mode). */
  session: Session | null;
  /** true until the first getSession() resolves. */
  loading: boolean;
  /** true when the app has no Supabase configuration at all. */
  offline: boolean;
  auth: AuthService;
}

const Ctx = createContext<GaplySession | null>(null);

export function useGaplySession(): GaplySession {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error('useGaplySession must be used inside <GaplySessionProvider>');
  return ctx;
}

export const GaplySessionProvider: React.FC<{
  children: React.ReactNode;
  /** Test seam — production uses the default (env-configured) service. */
  authService?: AuthService;
}> = ({ children, authService }) => {
  const auth = useMemo(() => authService ?? createAuthService(), [authService]);
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);
  const [offline, setOffline] = useState(false);

  useEffect(() => {
    let alive = true;
    auth.getSession().then(({ session, offline }) => {
      if (!alive) return;
      setSession(session);
      setOffline(offline);
      setLoading(false);
    });
    const off = auth.onAuthStateChange((s) => {
      if (alive) setSession(s);
    });
    return () => {
      alive = false;
      off();
    };
  }, [auth]);

  const value = useMemo(
    () => ({ session, loading, offline, auth }),
    [session, loading, offline, auth]
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
};

/** Route guard for ONLINE-ONLY routes: no session → redirect to /auth.
 *  Free-offline routes must never be wrapped in this. */
export const RequireSession: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { session, loading } = useGaplySession();
  const location = useLocation();
  if (loading) return null; // settle before deciding
  if (!session) {
    return <Navigate to="/auth" replace state={{ from: location.pathname }} />;
  }
  return <>{children}</>;
};
