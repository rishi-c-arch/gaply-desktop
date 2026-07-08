// Gaply — login / signup (split screen). Left: globe hero on jet black with
// violet glow. Right: glass auth card. "Continue offline" bypasses auth
// entirely and lands on the free-offline home dashboard.
//
// Auth is REAL: email/password + Google via the single Supabase client
// (services/supabase). Errors surface as toasts (never a fabricated success).
import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button, Card, GaplyGlobe } from '../../design-system';
import { ToastProvider } from '../../design-system/Toast';
import { useGaplySession } from '../session/SessionProvider';
import { useAuth } from './useAuth';
import { useDeepLinkAuth } from './useDeepLinkAuth';
import './auth.css';

type Mode = 'signin' | 'signup';

// ORCID sign-in is intentionally DEFERRED (flag defaults off). Why it isn't a
// drop-in like Google: ORCID's OIDC discovery
// (https://orcid.org/.well-known/openid-configuration, issuer exactly
// https://orcid.org) is spec-compliant (RS256; response types code / id_token /
// "id_token token"), BUT its claims_supported is only
// [family_name, given_name, name, auth_time, iss, sub] and its ONLY scope is
// `openid` — there is no email claim and no email scope. Per ORCID staff, email
// is obtainable only via a separate Member-API /email call, and only for
// records whose owner set email visibility to Public. A Supabase custom-OIDC
// wiring would therefore need `email_optional: true` plus a separate
// email-collection step, and there is no documented precedent of a working
// ORCID→Supabase custom-OIDC integration. Until that's built and tested, the
// button is shown disabled with a "coming soon" tooltip.
const ORCID_ENABLED = process.env.REACT_APP_FEATURE_ORCID === 'true';

const Inner: React.FC = () => {
  const { offline } = useGaplySession();
  const navigate = useNavigate();
  const { busy, signInPassword, signUpPassword, signInWithGoogle } = useAuth();
  const [mode, setMode] = useState<Mode>('signin');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');

  // Completes desktop Google OAuth via the gaply:// deep-link callback.
  useDeepLinkAuth();

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (mode === 'signup') {
      // On successful signup Supabase may require email confirmation, so we do
      // NOT assume a session — send them to the sign-in tab.
      const ok = await signUpPassword(email, password);
      if (ok) setMode('signin');
      return;
    }
    const ok = await signInPassword(email, password);
    if (ok) navigate('/app');
  };

  return (
    <div className="gds-root gds-auth" data-testid="auth-page">
      <section className="gds-auth__hero" aria-hidden="true">
        <div className="gds-auth__hero-globe">
          <GaplyGlobe scale="hero" />
        </div>
        <h1 className="gds-auth__tagline">Research integrity, verified.</h1>
        <p className="gds-auth__trust">
          Your manuscripts never leave your device unless you choose cloud verification.
        </p>
      </section>

      <section className="gds-auth__side">
        <Card glass className="gds-auth__card" title={mode === 'signin' ? 'Sign in' : 'Create account'}>
          <form onSubmit={submit} className="gds-auth__card" data-testid="auth-form">
            <div className="gds-auth__field">
              <label className="gds-auth__label" htmlFor="auth-email">Email</label>
              <input
                id="auth-email"
                className="gds-auth__input"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </div>
            <div className="gds-auth__field">
              <label className="gds-auth__label" htmlFor="auth-password">Password</label>
              <input
                id="auth-password"
                className="gds-auth__input"
                type="password"
                autoComplete={mode === 'signup' ? 'new-password' : 'current-password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                minLength={8}
              />
            </div>
            <Button type="submit" disabled={busy} data-testid="auth-submit">
              {mode === 'signin' ? 'Sign in' : 'Sign up'}
            </Button>
          </form>

          <div className="gds-auth__divider">or</div>
          <Button variant="secondary" onClick={() => signInWithGoogle()} disabled={busy} data-testid="oauth-google">
            Continue with Google
          </Button>
          <Button
            variant="secondary"
            disabled={!ORCID_ENABLED}
            title={ORCID_ENABLED ? undefined : 'ORCID sign-in coming soon'}
            data-testid="oauth-orcid"
            onClick={() => { /* deferred — see ORCID_ENABLED note above */ }}
          >
            Continue with ORCID{ORCID_ENABLED ? '' : ' — coming soon'}
          </Button>
          <Button variant="ghost" onClick={() => navigate('/app')} data-testid="continue-offline">
            Continue offline
          </Button>
          {offline && (
            <p className="gds-auth__trust">
              Offline mode: all free features work without an account.
            </p>
          )}
          <div className="gds-auth__switch">
            {mode === 'signin' ? (
              <>
                New to Gaply?{' '}
                <button type="button" onClick={() => setMode('signup')}>Create an account</button>
              </>
            ) : (
              <>
                Already have an account?{' '}
                <button type="button" onClick={() => setMode('signin')}>Sign in</button>
              </>
            )}
          </div>
        </Card>
      </section>
    </div>
  );
};

const AuthPage: React.FC = () => (
  <ToastProvider>
    <Inner />
  </ToastProvider>
);

export default AuthPage;
