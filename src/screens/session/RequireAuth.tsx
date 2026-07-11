// Gaply — app-wide login requirement (Set 8). CONSUMES the existing Supabase
// session (useGaplySession); it does not add any auth mechanism of its own.
//
// Policy: the app requires a signed-in session to use features — including
// the free/offline ones. Distinct from <RequireSession> (online-only routes),
// which stays untouched; this guard adds two deliberate graces:
//
//  1. OFFLINE GRACE — a previously-authenticated user keeps working offline:
//     the Supabase session persists on disk (Tauri Store / localStorage) and
//     getSession() returns the cached session without a network round-trip;
//     supabase-js autoRefreshToken re-validates honestly when connectivity
//     returns. We never lock out a signed-in user just for being offline.
//  2. OFFLINE MODE (no Supabase env config at all, e.g. unconfigured dev
//     builds) — there is no auth server to sign in against, so requiring
//     login would brick the build. We let it through with the honest
//     understanding that this is presentation anyway: THE REAL boundary for
//     paid work is server-side at the proxy, which such a build can't reach.
//
// This guard is UX. It routes people to sign-in; it is not a security wall.

import React from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { useGaplySession } from './SessionProvider';

export const RequireAuth: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { session, loading, offline } = useGaplySession();
  const location = useLocation();
  if (loading) return null; // settle before deciding
  if (session) return <>{children}</>; // live OR cached session (offline grace)
  if (offline) return <>{children}</>; // no auth config — nothing to sign in to
  return <Navigate to="/auth" replace state={{ from: location.pathname }} />;
};

export default RequireAuth;
