// Gaply — OAuth hand-off page, served on the WEB domain (https://<domain>/auth-success).
// Google redirects the system browser here after consent with ?code=…&state=…
// in the URL. We immediately bounce that exact query to the desktop app's
// custom scheme (gaply://auth/callback?…), which the OS routes to the installed
// app, where useDeepLinkAuth exchanges the code for a session.
//
// This is a plain web page — it does NOT run inside Tauri. It must be reachable
// at the redirectTo URL configured in auth.ts (REACT_APP_AUTH_SUCCESS_URL) and
// allow-listed as a Supabase redirect URL.
import React, { useEffect, useState } from 'react';

const AuthSuccessPage: React.FC = () => {
  const [deepLink, setDeepLink] = useState('');

  useEffect(() => {
    const search = window.location.search || '';
    const target = `gaply://auth/callback${search}`;
    setDeepLink(target);
    // Hand off to the desktop app. If the app is installed the OS switches to
    // it; if not, the manual link below is the fallback.
    window.location.replace(target);
  }, []);

  return (
    <div style={{ fontFamily: 'system-ui, sans-serif', maxWidth: 480, margin: '15vh auto', padding: 24, textAlign: 'center' }}>
      <h1 style={{ fontSize: 20 }}>Returning you to Gaply…</h1>
      <p style={{ color: '#666' }}>
        If the app doesn’t open automatically,{' '}
        <a href={deepLink}>click here to continue</a>.
      </p>
      <p style={{ color: '#999', fontSize: 13 }}>You can close this tab once Gaply is in focus.</p>
    </div>
  );
};

export default AuthSuccessPage;
