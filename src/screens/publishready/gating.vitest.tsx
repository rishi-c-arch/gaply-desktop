// Set 8 — login requirement + entitlement gating. The client checks here are
// UX-ONLY (the real gate is server-side at the proxy); these tests prove the
// honest presentation states and that the user's JWT rides the paid request.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import RequireAuth from '../session/RequireAuth';
import type { AuthService } from '../../services/supabase';
import { checkEntitlement } from '../subscription/entitlement';
import PublishReadyPage from './PublishReadyPage';
import { PublishReadyBridge } from './publishReadyBridge';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const SESSION = { user: { id: 'u1', email: 'a@b.c' }, access_token: 'jwt-abc' };

function auth(session: any, offline = false): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

/** Subscription service double for the server-owned entitlement row. */
function subsService(kind: 'premium' | 'free' | 'offline') {
  return {
    getTier: vi.fn().mockResolvedValue(
      kind === 'offline'
        ? { data: null, error: null, offline: true, tier: 'free' }
        : { data: { status: 'active' }, error: null, offline: false, tier: kind }
    ),
  } as any;
}

function renderPage(session: any, props: any = {}) {
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <PublishReadyPage {...props} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

/* -------------------------- signed-out is gated ------------------------- */

describe('login requirement (UX gate)', () => {
  it('signed OUT → sign-in prompt; the PublishReady form is unreachable', async () => {
    renderPage(null);
    expect(await screen.findByTestId('pr-signin')).toBeTruthy();
    expect(screen.getByTestId('pr-signin-cta').textContent).toMatch(/sign in/i);
    expect(screen.queryByTestId('pr-teaser')).toBeNull();
    expect(screen.queryByTestId('pr-journal-input')).toBeNull(); // can't start a job
  });

  it('RequireAuth redirects a signed-out user to /auth (and passes a session through)', async () => {
    const app = (svc: AuthService) => (
      <MemoryRouter initialEntries={["/app/x"]}>
        <GaplySessionProvider authService={svc}>
          <Routes>
            <Route path="/auth" element={<div data-testid="auth-page" />} />
            <Route path="/app/x" element={<RequireAuth><div data-testid="feature" /></RequireAuth>} />
          </Routes>
        </GaplySessionProvider>
      </MemoryRouter>
    );
    const { unmount } = render(app(auth(null)));
    await waitFor(() => expect(screen.getByTestId('auth-page')).toBeTruthy());
    expect(screen.queryByTestId('feature')).toBeNull();
    unmount();

    // signed in (a CACHED session counts — offline grace) → feature renders.
    render(app(auth(SESSION)));
    await waitFor(() => expect(screen.getByTestId('feature')).toBeTruthy());
  });

  it('offline MODE (no Supabase config) still renders — nothing to sign in to', async () => {
    render(
      <MemoryRouter initialEntries={["/app/x"]}>
        <GaplySessionProvider authService={auth(null, true)}>
          <Routes>
            <Route path="/auth" element={<div data-testid="auth-page" />} />
            <Route path="/app/x" element={<RequireAuth><div data-testid="feature" /></RequireAuth>} />
          </Routes>
        </GaplySessionProvider>
      </MemoryRouter>
    );
    await waitFor(() => expect(screen.getByTestId('feature')).toBeTruthy());
  });
});

/* ------------------------- entitlement states --------------------------- */

describe('entitlement gating (UX-only; server owns the truth)', () => {
  it('signed in, NO entitlement → locked teaser with an upgrade path, no prices', async () => {
    renderPage(SESSION, { subscriptionService: subsService('free') });
    expect(await screen.findByTestId('pr-teaser')).toBeTruthy();
    const cta = screen.getByTestId('pr-unlock');
    expect(cta.textContent).toMatch(/Unlock the verdict/i);
    // pricing-agnostic: the locked state never shows an amount or currency.
    expect(cta.textContent).not.toMatch(/₹|\$|€|INR|USD|\d+\s*\/(mo|yr)/);
  });

  it('entitled (server-owned subscription row) → the real form renders', async () => {
    renderPage(SESSION, { subscriptionService: subsService('premium') });
    expect(await screen.findByTestId('pr-journal-input')).toBeTruthy();
    expect(screen.queryByTestId('pr-teaser')).toBeNull();
  });

  it('signed in but server unreachable → honest offline_unverified, NOT an upsell', async () => {
    renderPage(SESSION, { subscriptionService: subsService('offline') });
    expect(await screen.findByTestId('pr-offline')).toBeTruthy();
    expect(screen.getByTestId('pr-offline').textContent).toMatch(/can’t verify your plan|can’t be verified/i);
    // never tell a possibly-premium user to upgrade just because they're offline
    expect(screen.queryByTestId('pr-unlock')).toBeNull();
  });

  it('checkEntitlement unit paths: signed_out / entitled / not_entitled / offline_unverified', async () => {
    expect((await checkEntitlement(null, 'publishready', subsService('premium'))).status).toBe('signed_out');
    expect((await checkEntitlement(SESSION as any, 'publishready', subsService('premium'))).status).toBe('entitled');
    expect((await checkEntitlement(SESSION as any, 'publishready', subsService('free'))).status).toBe('not_entitled');
    expect((await checkEntitlement(SESSION as any, 'publishready', subsService('offline'))).status).toBe('offline_unverified');
    // non-premium features aren't entitlement-gated (session is enough)
    expect((await checkEntitlement(SESSION as any, 'ai_check', subsService('free'))).status).toBe('entitled');
  });
});

/* --------------------- the JWT rides the paid request ------------------- */

describe('server-side gate plumbing', () => {
  it('an entitled run passes the user token to the bridge (→ Rust → proxy)', async () => {
    const seen: any[] = [];
    const bridge: PublishReadyBridge = {
      async run(input) {
        seen.push(input);
        throw new Error('stop here — plumbing proven');
      },
      async ingestGuidelines() {
        return { any_ingested: false, note: 'noop' };
      },
      async reviewerLetterAvailability() {
        return { available: true, reason: null, proxyUrl: 'https://proxy.test' };
      },
    };
    renderPage(SESSION, { subscriptionService: subsService('premium'), bridge });
    await screen.findByTestId('pr-entry');

    // drive the minimal path to run(): file + journal + start (same sequence
    // as the existing premium-flow test)
    const { fireEvent } = await import('@testing-library/react');
    fireEvent.change(screen.getByTestId('pr-file'), {
      target: { files: [new File(['x'], 'paper.pdf', { type: 'application/pdf' })] },
    });
    await screen.findByText('paper.pdf'); // async acceptFile settled
    fireEvent.change(screen.getByTestId('pr-journal-input'), { target: { value: 'lancet' } });
    fireEvent.click(await screen.findByTestId('pr-journal-The Lancet'));
    fireEvent.click(screen.getByTestId('pr-run'));

    await waitFor(() => expect(seen.length).toBe(1));
    expect(seen[0].userToken).toBe('jwt-abc');
  });
});

/* ------- BLOCKER 6: the reviewer letter's absence is stated BEFORE a run ----- */

describe('reviewer letter pre-flight', () => {
  /** A bridge whose `run` FAILS the test if it is ever called. The point of the
   *  notice is that it appears without a run having happened; a mock that
   *  quietly tolerated a run would let the test pass for the wrong reason. */
  const noRunBridge = (availability: any): PublishReadyBridge => ({
    async run() {
      throw new Error('run() must not be called — the notice is pre-flight');
    },
    async ingestGuidelines() {
      return { any_ingested: false, note: 'noop' };
    },
    async reviewerLetterAvailability() {
      return availability;
    },
  });

  it('an unconfigured proxy is announced before the run, and does not block it', async () => {
    const bridge = noRunBridge({
      available: false,
      reason: 'no proxy is configured on this machine — the address points back at your own computer',
      proxyUrl: 'http://127.0.0.1:8080',
    });
    renderPage(SESSION, { subscriptionService: subsService('premium'), bridge });

    const notice = await screen.findByTestId('pr-letter-unavailable');
    expect(notice.textContent).toMatch(/before you run/i);
    expect(notice.textContent).toMatch(/no proxy is configured/i);

    // NOT an error, and NOT a blocker: the local half of the report is
    // unaffected and still worth running, so the decision stays with the user.
    expect(screen.queryByTestId('pr-error')).toBeNull();
    expect(screen.queryByTestId('pr-journal-input')).toBeTruthy();
  });

  it('a configured proxy says nothing — the notice is not a permanent fixture', async () => {
    const bridge = noRunBridge({ available: true, reason: null, proxyUrl: 'https://proxy.test' });
    renderPage(SESSION, { subscriptionService: subsService('premium'), bridge });
    await screen.findByTestId('pr-entry');
    expect(screen.queryByTestId('pr-letter-unavailable')).toBeNull();
  });

  it('a failing pre-flight says NOTHING rather than guessing', async () => {
    // A wrong "unavailable" would talk a user out of a run that would have
    // worked — worse than the late answer this replaces.
    const bridge: PublishReadyBridge = {
      async run() {
        throw new Error('run() must not be called');
      },
      async ingestGuidelines() {
        return { any_ingested: false, note: 'noop' };
      },
      async reviewerLetterAvailability() {
        throw new Error('IPC exploded');
      },
    };
    renderPage(SESSION, { subscriptionService: subsService('premium'), bridge });
    await screen.findByTestId('pr-entry');
    expect(screen.queryByTestId('pr-letter-unavailable')).toBeNull();
    expect(screen.queryByTestId('pr-error')).toBeNull();
  });
});
