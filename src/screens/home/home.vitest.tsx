// Home (/app) tests — the app home is the "Academic Research Platform" hero.
// Mocked services, MemoryRouter, no network, no WebGL.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AuthService } from '../../services/supabase';
import { GaplySessionProvider } from '../session/SessionProvider';
import HomeDashboardPage, { summaryToRing } from './HomeDashboardPage';
// The upload route renders the analysis theater; its upload phase keeps the
// testids the home entry points assert against.
import AnalysisTheaterPage from '../analysis/AnalysisTheaterPage';

// The hero's decorative R3F globe needs WebGL (absent in jsdom) — stub it.
vi.mock('../../components/ThreeJSGlobe', () => ({
  default: () => <div data-testid="hero-globe-stub" />,
}));

afterEach(cleanup);

function fakeAuth(session: { user: { id: string; email: string } } | null): AuthService {
  return {
    signUp: vi.fn(),
    signIn: vi.fn(),
    signInWithOAuth: vi.fn(),
    signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: session as any, offline: session === null }),
    onAuthStateChange: (cb: (s: any) => void) => {
      cb(session as any);
      return () => {};
    },
  } as any;
}

function renderDash(
  session: { user: { id: string; email: string } } | null,
  initialPath = '/app'
) {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <GaplySessionProvider authService={fakeAuth(session)}>
        <Routes>
          <Route path="/app" element={<HomeDashboardPage />} />
          <Route path="/app/upload" element={<AnalysisTheaterPage />} />
        </Routes>
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

describe('the home hero', () => {
  it('renders the Academic Research Platform hero', async () => {
    renderDash(null);
    await screen.findByTestId('home-dashboard');
    expect(screen.getByTestId('dash-hero')).toBeTruthy();
    expect(screen.getByText('Academic')).toBeTruthy();
    expect(screen.getByText('Research Platform')).toBeTruthy();
    expect(screen.getByText('RESEARCH INTELLIGENCE')).toBeTruthy();
    expect(screen.getByText('Start Analysis')).toBeTruthy();
    // the "01 · Private Research Ecosystem" live-data strip
    expect(screen.getByText(/Research Ecosystem/)).toBeTruthy();
  });

  it('exposes the wired-up premium features in the hero nav', async () => {
    renderDash(null);
    await screen.findByTestId('home-dashboard');
    // Statistical Analysis Verifier (paid, previously dark — no route/nav) is now reachable.
    const sv = screen.getByRole('link', { name: /Statistical Analysis Verifier/ });
    expect(sv.getAttribute('href')).toBe('/app/statsverifier');
    // Research Gap Finder (route existed, nav entry was missing) is now linked.
    const gf = screen.getByRole('link', { name: /Research Gap Finder/ });
    expect(gf.getAttribute('href')).toBe('/app/gapfinder');
    // The FREE deterministic Statistical Analysis Check is HIDDEN behind the
    // statsCheck flag (default OFF) — the paid Verifier above is unaffected.
    expect(screen.queryByRole('link', { name: 'Statistical Analysis Check' })).toBeNull();
  });

  it('"Start Analysis" navigates into the upload flow', async () => {
    renderDash(null);
    fireEvent.click(await screen.findByTestId('start-analysis'));
    await screen.findByTestId('upload-page');
    expect(screen.getByTestId('upload-fresh')).toBeTruthy();
  });

  it('?sample=1 (from onboarding) redirects into the sample upload path', async () => {
    renderDash(null, '/app?sample=1');
    await screen.findByTestId('upload-page');
    expect(screen.getByTestId('upload-sample')).toBeTruthy();
  });
});

describe('summaryToRing (pure)', () => {
  it('maps certainty counts to score + status correctly', () => {
    expect(summaryToRing({ certain: 4, assessed: 1, flagged: 0 })).toEqual({ score: 80, status: 'assessed' });
    expect(summaryToRing({ certain: 1, assessed: 2, flagged: 2 })).toEqual({ score: 20, status: 'flagged' });
    expect(summaryToRing({ certain: 3 })).toEqual({ score: 100, status: 'certain' });
    expect(summaryToRing({})).toEqual({ score: 0, status: 'certain' });
  });
});
