// F12 — subscription & paywall tests.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import { gateFeature } from './tiers';
import { subscriptionUpdateFromWebhook, MockRazorpayClient } from './razorpay';
import { PLANS, studentPrice } from './pricing';
import BillingPage from './BillingPage';
import PublishReadyPage from '../publishready/PublishReadyPage';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

function auth(session: any): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

/* ------------------------------ gate logic ------------------------------ */

describe('feature gating', () => {
  it('free-tier online features are UNCAPPED — the phantom caps were removed (H3)', () => {
    // citation_verification / journal_check were advertised as 5 / 3 but never
    // enforced (bumpUsage had zero callers) and can't be enforced client-side, so
    // ONLINE_CAPPED is now empty → gateFeature allows them unlimited, honestly.
    const cv = gateFeature('free', 'citation_verification', 999);
    expect(cv.allowed).toBe(true);
    expect(cv.cap).toBeNull();
    expect(cv.reason).toBeUndefined();
    expect(gateFeature('free', 'journal_check', 999).allowed).toBe(true);
  });

  it('premium user is unlocked on online + ★ features', () => {
    expect(gateFeature('premium', 'citation_verification', 999).allowed).toBe(true);
    expect(gateFeature('premium', 'publishready').allowed).toBe(true);
  });

  it('free user is blocked from ★ premium-only features (teaser)', () => {
    const g = gateFeature('free', 'publishready');
    expect(g.allowed).toBe(false);
    expect(g.reason).toBe('premium_only');
  });

  it('OFFLINE features are ALWAYS allowed, any tier — never crippled', () => {
    for (const f of ['extraction', 'validation', 'ai_check', 'local_plagiarism', 'report_view']) {
      expect(gateFeature('free', f, 999).allowed).toBe(true);
      expect(gateFeature('premium', f, 999).allowed).toBe(true);
    }
  });
});

/* ------------------------------ Razorpay -------------------------------- */

describe('Razorpay webhook flips subscription status (mocked)', () => {
  it('activation → premium/active; cancellation → free/cancelled', () => {
    const activated = subscriptionUpdateFromWebhook({
      event: 'subscription.activated',
      payload: { subscription: { entity: { id: 'sub_123', status: 'active' } } },
    });
    expect(activated).toMatchObject({ tier: 'premium', status: 'active', razorpay_subscription_id: 'sub_123' });

    const cancelled = subscriptionUpdateFromWebhook({
      event: 'subscription.cancelled',
      payload: { subscription: { entity: { id: 'sub_123', status: 'cancelled' } } },
    });
    expect(cancelled).toMatchObject({ tier: 'free', status: 'cancelled' });

    // unrelated events are ignored
    expect(subscriptionUpdateFromWebhook({ event: 'payment.captured', payload: {} as any })).toBeNull();
  });
});

/* ------------------------------- pricing -------------------------------- */

describe('pricing', () => {
  it('prices end in 9 and annual shows an effective monthly', () => {
    for (const p of PLANS) {
      expect(p.monthlyInr % 10).toBe(9);
      expect(p.annualInr % 10).toBe(9);
      expect(p.effectiveMonthlyInr).toBeLessThan(p.monthlyInr); // annual is cheaper
    }
  });
  it('student discount ~50%, still ending in 9', () => {
    expect(studentPrice(299) % 10).toBe(9);
    expect(studentPrice(299)).toBeLessThan(299 * 0.6);
  });
});

/* ------------------------------ billing UI ------------------------------ */

describe('BillingPage — payments OFF shows an honest disabled state (no fake success)', () => {
  it('the plan button is disabled/"coming soon" and clicking never fabricates success', async () => {
    // payments flag defaults OFF in the test env, and the production default
    // client cannot fabricate success. A real Razorpay client is never reached.
    const rzp = new MockRazorpayClient();
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth({ user: { id: 'u1', email: 'a@b.c' } })}>
          <BillingPage razorpay={rzp} />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    const btn = (await screen.findByTestId('choose-pro')) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.textContent).toMatch(/coming soon/i);
    fireEvent.click(btn);
    // no checkout call, and crucially no "Payment started"/success anywhere
    await waitFor(() => expect(rzp.requests.length).toBe(0));
    expect(screen.queryByText(/payment started/i)).toBeNull();
  });
});

/* ------------------- useSubscription gates a ★ screen ------------------- */

describe('useSubscription gates a ★ screen', () => {
  it('free session → PublishReady shows the teaser; premium → full flow', async () => {
    const freeSub = { getTier: vi.fn().mockResolvedValue({ tier: 'free', data: null, error: null, offline: false }) } as any;
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth({ user: { id: 'u1', email: 'a@b.c' } })}>
          <PublishReadyPage subscriptionService={freeSub} />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    expect(await screen.findByTestId('pr-teaser')).toBeTruthy();
  });
});
