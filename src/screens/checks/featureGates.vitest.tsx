// Feature-removal Set 1 — proves the DEFAULT (flags OFF) behavior: the free
// Statistical Analysis Check is invisible, and the free Plagiarism Check shows
// an honest coming-soon while its "my papers" library manager stays live (Notes
// depends on it). No config/Feature mock here — flags read their real OFF value.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import PlagiarismCheckPage from './PlagiarismCheckPage';
import AppHomeHero from '../home/AppHomeHero';
import { makeMockCheckBridge } from './checkBridge';
import { ThemeProvider } from '../../contexts/ThemeContext';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-${scale}`} />,
}));
// HeroAnimation draws to a canvas — stub it so AppHomeHero renders in jsdom.
vi.mock('../home/HeroAnimation', () => ({ default: () => <div data-testid="hero-anim" /> }));

afterEach(cleanup);

function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
  } as any;
}

describe('Set 1 gates — free checks OFF by default', () => {
  it('plagiarismCheck OFF → coming-soon shown, run disabled, library manager STILL live', () => {
    const bridge = makeMockCheckBridge({ library: [] });
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <PlagiarismCheckPage bridge={bridge} />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    // honest coming-soon, no runnable check
    expect(screen.getByTestId('plag-coming-soon')).toBeTruthy();
    expect(screen.getByTestId('plag-coming-soon-note').textContent).toMatch(/coming soon/i);
    expect(screen.queryByTestId('run-check')).toBeNull();
    // the library manager survives (Notes' side-by-side reads from it)
    expect(screen.getByTestId('plag-library')).toBeTruthy();
  });

  it('statsCheck OFF → the app home has NO Statistical Analysis Check launch entry (paid Verifier stays)', () => {
    render(
      <MemoryRouter>
        <ThemeProvider>
          <AppHomeHero />
        </ThemeProvider>
      </MemoryRouter>
    );
    // the free stats check is gone from the launcher
    expect(screen.queryByText('Statistical Analysis Check')).toBeNull();
    // the PAID verifier is untouched and still listed
    expect(screen.getAllByText(/Statistical Analysis Verifier/).length).toBeGreaterThan(0);
  });
});
