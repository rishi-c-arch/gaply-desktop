// Gaply — login / signup. Split layout: a clean form panel on the left and a
// slow-drifting galaxy visual on the right. A theme toggle switches the light
// version. Design only — the auth flow is UNCHANGED: email/password + Google
// via the single Supabase client, ORCID deferred, "Continue offline" bypass.
// Every handler and testid is preserved.
import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ToastProvider, useToast } from '../../design-system/Toast';
import { useTheme } from '../../contexts/ThemeContext';
import { useGaplySession } from '../session/SessionProvider';
import { useAuth } from './useAuth';
import { useDeepLinkAuth } from './useDeepLinkAuth';
import { useFeatureFlag } from '../../config/Feature';
import spaceVisual from '../../assets/auth-space.jpg';
// Bundled fonts for the headings (cohesive with the landing).
import '@fontsource/epilogue/700.css';
import '@fontsource/epilogue/800.css';
import '@fontsource/epilogue/900.css';
import '@fontsource/manrope/400.css';
import '@fontsource/manrope/500.css';
import '@fontsource/manrope/600.css';
import '@fontsource/manrope/700.css';
import './authRedesign.css';

type Mode = 'signin' | 'signup';

// Optional researcher identity captured at signup. Free-text values (the
// profiles.role column has no CHECK constraint); kept short and honest.
const ROLE_OPTIONS = ['Scholar', 'Individual Researcher', 'Student', 'Other'] as const;

// ORCID sign-in is intentionally DEFERRED (flag defaults off): its OIDC has no
// email claim/scope, so a Supabase custom-OIDC wiring would need a separate
// email-collection step with no documented working precedent. Shown disabled
// with a "coming soon" tooltip until that's built.

const GoogleMark: React.FC = () => (
  <svg viewBox="0 0 48 48" aria-hidden>
    <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z" />
    <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z" />
    <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z" />
    <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z" />
  </svg>
);

const ThemeToggle: React.FC = () => {
  const { theme, setTheme } = useTheme();
  const dark = theme !== 'light';
  return (
    <button
      type="button"
      className="gpl-auth__theme"
      aria-label={dark ? 'Switch to light theme' : 'Switch to dark theme'}
      onClick={() => setTheme(dark ? 'light' : 'dark')}
      data-testid="auth-theme-toggle"
    >
      {dark ? (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <circle cx="12" cy="12" r="5" /><path d="M12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4" />
        </svg>
      ) : (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      )}
    </button>
  );
};

const Inner: React.FC = () => {
  const { offline } = useGaplySession();
  const navigate = useNavigate();
  const { toast } = useToast();
  const { busy, signInPassword, signUpPassword, signInWithGoogle } = useAuth();
  const orcidEnabled = useFeatureFlag('orcid');
  const [mode, setMode] = useState<Mode>('signin');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  // Optional signup identity (never blocks signup).
  const [name, setName] = useState('');
  const [role, setRole] = useState('');

  // Completes desktop Google OAuth via the gaply:// deep-link callback. On an
  // error callback (e.g. Supabase failed the code exchange with Google), surface
  // an honest, readable message instead of a silent dead-end.
  useDeepLinkAuth((r) => {
    if (!r.ok) {
      toast(
        `Google sign-in failed${r.error ? ` — ${r.error}` : ''}. Try email signup, or continue offline.`,
        'flagged'
      );
    }
  });

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (mode === 'signup') {
      const ok = await signUpPassword(email, password, { displayName: name, role });
      if (ok) setMode('signin');
      return;
    }
    const ok = await signInPassword(email, password);
    if (ok) navigate('/app');
  };

  return (
    <div className="gpl-auth" data-testid="auth-page">
      {/* left: form panel */}
      <section className="gpl-auth__panel">
        <div className="gpl-auth__panel-inner">
          <div className="gpl-auth__topbar">
            <span className="gpl-auth__brand-name">Gaply</span>
            <ThemeToggle />
          </div>

          <h1 className="gpl-auth__title">{mode === 'signin' ? 'Welcome back' : 'Create your account'}</h1>
          <p className="gpl-auth__subtitle">
            {mode === 'signin'
              ? 'Sign in to your research workspace.'
              : 'Start verifying your research integrity.'}
          </p>

          <form onSubmit={submit} data-testid="auth-form">
            {mode === 'signup' && (
              <div className="gpl-auth__field">
                <label className="gpl-auth__label" htmlFor="auth-name">Name <span className="gpl-auth__optional">(optional)</span></label>
                <input
                  id="auth-name"
                  className="gpl-auth__input"
                  type="text"
                  autoComplete="name"
                  placeholder="How should we address you?"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  data-testid="auth-name"
                />
              </div>
            )}
            <div className="gpl-auth__field">
              <label className="gpl-auth__label" htmlFor="auth-email">Email</label>
              <input
                id="auth-email"
                className="gpl-auth__input"
                type="email"
                autoComplete="email"
                placeholder="you@university.edu"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </div>
            <div className="gpl-auth__field">
              <label className="gpl-auth__label" htmlFor="auth-password">Password</label>
              <input
                id="auth-password"
                className="gpl-auth__input"
                type="password"
                autoComplete={mode === 'signup' ? 'new-password' : 'current-password'}
                placeholder="••••••••"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                minLength={8}
              />
            </div>
            {mode === 'signup' && (
              <div className="gpl-auth__field">
                <label className="gpl-auth__label" htmlFor="auth-role">Role <span className="gpl-auth__optional">(optional)</span></label>
                <select
                  id="auth-role"
                  className="gpl-auth__input gpl-auth__select"
                  value={role}
                  onChange={(e) => setRole(e.target.value)}
                  data-testid="auth-role"
                >
                  <option value="">Select your role…</option>
                  {ROLE_OPTIONS.map((r) => (
                    <option key={r} value={r}>{r}</option>
                  ))}
                </select>
              </div>
            )}
            <button type="submit" className="gpl-auth__submit" disabled={busy} data-testid="auth-submit">
              {mode === 'signin' ? 'Sign in' : 'Sign up'}
            </button>
          </form>

          <div className="gpl-auth__divider">or</div>

          <button type="button" className="gpl-auth__oauth" onClick={() => signInWithGoogle()} disabled={busy} data-testid="oauth-google">
            <GoogleMark />
            Continue with Google
          </button>
          <button
            type="button"
            className="gpl-auth__oauth"
            disabled={!orcidEnabled}
            title={orcidEnabled ? undefined : 'ORCID sign-in coming soon'}
            data-testid="oauth-orcid"
            onClick={() => { /* deferred */ }}
          >
            Continue with ORCID{orcidEnabled ? '' : ' — coming soon'}
          </button>
          <button type="button" className="gpl-auth__ghost" onClick={() => navigate('/app')} data-testid="continue-offline">
            Continue offline
          </button>

          {offline && <p className="gpl-auth__note">Offline mode: all free features work without an account.</p>}

          <div className="gpl-auth__switch">
            {mode === 'signin' ? (
              <>New to Gaply?{' '}<button type="button" onClick={() => setMode('signup')}>Create an account</button></>
            ) : (
              <>Already have an account?{' '}<button type="button" onClick={() => setMode('signin')}>Sign in</button></>
            )}
          </div>
        </div>
      </section>

      {/* right: galaxy visual (slow drift) + overlaid tagline */}
      <aside className="gpl-auth__visual" aria-hidden="true">
        <div className="gpl-auth__visual-img" style={{ backgroundImage: `url(${spaceVisual})` }} />
        <div className="gpl-auth__visual-overlay" />
        <div className="gpl-auth__visual-copy">
          <p className="gpl-auth__visual-tagline">Research integrity,<br />verified.</p>
          <p className="gpl-auth__visual-sub">Your manuscripts never leave your device.</p>
        </div>
      </aside>
    </div>
  );
};

const AuthPage: React.FC = () => (
  <ToastProvider>
    <Inner />
  </ToastProvider>
);

export default AuthPage;
