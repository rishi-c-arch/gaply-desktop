// Gaply — login / signup (split screen). Left: globe hero on jet black with
// violet glow. Right: glass auth card. "Continue offline" bypasses auth
// entirely and lands on the free-offline home dashboard.
import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button, Card, GaplyGlobe } from '../../design-system';
import { useGaplySession } from '../session/SessionProvider';
import './auth.css';

type Mode = 'signin' | 'signup';

const AuthPage: React.FC = () => {
  const { auth, offline } = useGaplySession();
  const navigate = useNavigate();
  const [mode, setMode] = useState<Mode>('signin');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    const res =
      mode === 'signup' ? await auth.signUp(email, password) : await auth.signIn(email, password);
    setBusy(false);
    if (!res.ok) {
      setError(res.error === 'offline' ? 'No connection configured — use "Continue offline".' : res.error ?? 'failed');
      return;
    }
    navigate(mode === 'signup' ? '/onboarding' : '/app');
  };

  const oauth = async (provider: 'google' | 'orcid') => {
    setError(null);
    const res = await auth.signInWithOAuth(provider);
    if (!res.ok) {
      setError(res.error === 'offline' ? 'No connection configured — use "Continue offline".' : res.error ?? 'failed');
    }
    // success: Supabase redirects the window; nothing to do here
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
            {error && <p className="gds-auth__error" role="alert">{error}</p>}
            <Button type="submit" disabled={busy}>
              {mode === 'signin' ? 'Sign in' : 'Sign up'}
            </Button>
          </form>

          <div className="gds-auth__divider">or</div>
          <Button variant="secondary" onClick={() => oauth('google')}>Continue with Google</Button>
          <Button variant="secondary" onClick={() => oauth('orcid')}>Continue with ORCID</Button>
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

export default AuthPage;
