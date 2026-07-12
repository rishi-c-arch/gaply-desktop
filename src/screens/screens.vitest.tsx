// F3 screen tests — mocked Supabase services, MemoryRouter, no network.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AuthService } from '../services/supabase';
import { GaplySessionProvider, RequireSession } from './session/SessionProvider';
import AuthPage from './auth/AuthPage';
import OnboardingPage from './onboarding/OnboardingPage';
import HomeDashboardPage from './home/HomeDashboardPage';

// R3F needs WebGL; jsdom has none. The globes are decorative in these screens.
vi.mock('../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));
vi.mock('../components/ThreeJSGlobe', () => ({
  default: () => <div data-testid="hero-globe-stub" />,
}));

afterEach(cleanup);

/** Auth service double: fixed session (or none), records calls. */
function fakeAuth(session: { user: { id: string; email: string } } | null): AuthService {
  return {
    signUp: vi.fn().mockResolvedValue({ ok: true }),
    signIn: vi.fn().mockResolvedValue({ ok: true }),
    signInWithOAuth: vi.fn().mockResolvedValue({ ok: true }),
    signOut: vi.fn().mockResolvedValue({ ok: true }),
    getSession: vi.fn().mockResolvedValue({ session: session as any, offline: session === null }),
    onAuthStateChange: (cb: (s: any) => void) => {
      cb(session as any);
      return () => {};
    },
  };
}

function renderApp(initialPath: string, auth: AuthService) {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <GaplySessionProvider authService={auth}>
        <Routes>
          <Route path="/auth" element={<AuthPage />} />
          <Route
            path="/onboarding"
            element={
              <RequireSession>
                <OnboardingPage />
              </RequireSession>
            }
          />
          <Route path="/app" element={<HomeDashboardPage />} />
        </Routes>
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

describe('Continue offline', () => {
  it('reaches the home dashboard with NO session, free features visible + sign-in nudge', async () => {
    renderApp('/auth', fakeAuth(null));
    await screen.findByTestId('auth-page');

    fireEvent.click(screen.getByTestId('continue-offline'));

    // landed on home with no session at all — the app home is the hero
    await screen.findByTestId('home-dashboard');
    expect(screen.getByTestId('dash-hero')).toBeTruthy();
    expect(screen.getByText('Research Platform')).toBeTruthy();
  });

  it('home dashboard renders directly at /app with no session (offline routes never guarded)', async () => {
    renderApp('/app', fakeAuth(null));
    await screen.findByTestId('home-dashboard');
    expect(screen.getByTestId('dash-hero')).toBeTruthy();
  });
});

describe('route guarding', () => {
  it('an online-only route (/onboarding) redirects to login when there is no session', async () => {
    renderApp('/onboarding', fakeAuth(null));
    // guard settles after getSession resolves, then redirects
    await screen.findByTestId('auth-page');
    expect(screen.queryByTestId('onboarding-page')).toBeNull();
  });

  it('the same route renders for a signed-in user', async () => {
    renderApp('/onboarding', fakeAuth({ user: { id: 'u1', email: 'a@b.c' } }));
    await screen.findByTestId('onboarding-page');
  });
});

describe('onboarding', () => {
  it('writes role/field/journals/country to the profiles table (mocked)', async () => {
    const upsertOwn = vi.fn().mockResolvedValue({ data: {}, error: null, offline: false });
    const profileService = { getOwn: vi.fn(), upsertOwn } as any;

    render(
      <MemoryRouter initialEntries={['/onboarding']}>
        <GaplySessionProvider authService={fakeAuth({ user: { id: 'u1', email: 'a@b.c' } })}>
          <Routes>
            <Route path="/onboarding" element={<OnboardingPage profileService={profileService} />} />
            <Route path="/app" element={<div data-testid="app-stub" />} />
          </Routes>
        </GaplySessionProvider>
      </MemoryRouter>
    );

    // step 1: role
    fireEvent.click(await screen.findByTestId('role-phd_scholar'));
    fireEvent.click(screen.getByText('Continue'));
    // step 2: field + journals
    fireEvent.change(screen.getByLabelText(/research field/i), { target: { value: 'neuroscience' } });
    fireEvent.change(screen.getByLabelText(/target journals/i), { target: { value: 'Nature, eLife' } });
    fireEvent.click(screen.getByText('Continue'));
    // step 3: country
    fireEvent.change(screen.getByLabelText(/country/i), { target: { value: 'India' } });
    fireEvent.click(screen.getByText('Continue'));
    // finish → sample manuscript CTA
    fireEvent.click(screen.getByTestId('finish-sample'));

    await waitFor(() =>
      expect(upsertOwn).toHaveBeenCalledWith({
        id: 'u1',
        email: 'a@b.c',
        role: 'phd_scholar',
        field: 'neuroscience',
        target_journals: 'Nature, eLife',
        country: 'India',
      })
    );
    // and lands on the app (sample flow)
    await screen.findByTestId('app-stub');
  });

  it('every step is skippable and skipping still lands on the dashboard', async () => {
    const upsertOwn = vi.fn().mockResolvedValue({ data: {}, error: null, offline: false });
    render(
      <MemoryRouter initialEntries={['/onboarding']}>
        <GaplySessionProvider authService={fakeAuth({ user: { id: 'u1', email: 'a@b.c' } })}>
          <Routes>
            <Route
              path="/onboarding"
              element={<OnboardingPage profileService={{ getOwn: vi.fn(), upsertOwn } as any} />}
            />
            <Route path="/app" element={<div data-testid="app-stub" />} />
          </Routes>
        </GaplySessionProvider>
      </MemoryRouter>
    );

    fireEvent.click(await screen.findByText('Skip')); // role
    fireEvent.click(screen.getByText('Skip')); // field
    fireEvent.click(screen.getByText('Skip')); // country
    fireEvent.click(screen.getByTestId('finish-plain'));
    await screen.findByTestId('app-stub');
    // defaults still recorded against the signed-in user
    await waitFor(() => expect(upsertOwn).toHaveBeenCalled());
    expect(upsertOwn.mock.calls[0][0].role).toBe('researcher');
  });
});
