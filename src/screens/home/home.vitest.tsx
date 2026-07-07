// F4 home-dashboard tests — mocked Supabase services, MemoryRouter, no network.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AuthService, AnalysisHistoryRow } from '../../services/supabase';
import { GaplySessionProvider } from '../session/SessionProvider';
import HomeDashboardPage, { summaryToRing } from './HomeDashboardPage';
// The upload route now renders the F5 analysis theater; its upload phase keeps
// the same testids the F4 dashboard entry points assert against.
import AnalysisTheaterPage from '../analysis/AnalysisTheaterPage';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
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

const ROWS: AnalysisHistoryRow[] = [
  {
    id: 'r1',
    user_id: 'u1',
    title: 'Sleep and Memory — draft 3',
    created_at: '2026-07-01T10:00:00Z',
    certainty_summary_json: { certain: 4, assessed: 1, flagged: 0 },
    tier_used: 'free',
  },
  {
    id: 'r2',
    user_id: 'u1',
    title: 'CRISPR screening methods',
    created_at: '2026-06-20T10:00:00Z',
    certainty_summary_json: { certain: 1, assessed: 2, flagged: 2 },
    tier_used: 'premium',
  },
];

function services({
  rows = ROWS,
  tier = 'free' as 'free' | 'premium',
} = {}) {
  return {
    historyService: {
      record: vi.fn(),
      list: vi.fn().mockResolvedValue({ data: rows, error: null, offline: false }),
    } as any,
    subscriptionService: {
      getTier: vi.fn().mockResolvedValue({ data: null, error: null, offline: false, tier }),
    } as any,
  };
}

function renderDash(
  session: { user: { id: string; email: string } } | null,
  svc = services(),
  initialPath = '/app'
) {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <GaplySessionProvider authService={fakeAuth(session)}>
        <Routes>
          <Route path="/app" element={<HomeDashboardPage {...svc} />} />
          <Route path="/app/upload" element={<AnalysisTheaterPage />} />
        </Routes>
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

const USER = { user: { id: 'u1', email: 'a@b.c' } };

describe('recent manuscripts (metadata only)', () => {
  it('renders history rows from mocked Supabase with score rings', async () => {
    const svc = services();
    renderDash(USER, svc);
    await screen.findByTestId('recent-list');
    expect(screen.getByText('Sleep and Memory — draft 3')).toBeTruthy();
    expect(screen.getByText('CRISPR screening methods')).toBeTruthy();
    expect(svc.historyService.list).toHaveBeenCalledWith('u1', 8);
    // rings render as accessible meters
    const meters = screen.getAllByRole('meter');
    expect(meters.length).toBeGreaterThanOrEqual(2);
  });

  it('summaryToRing maps counts to score + status correctly', () => {
    expect(summaryToRing({ certain: 4, assessed: 1, flagged: 0 })).toEqual({ score: 80, status: 'assessed' });
    expect(summaryToRing({ certain: 1, assessed: 2, flagged: 2 })).toEqual({ score: 20, status: 'flagged' });
    expect(summaryToRing({ certain: 3 })).toEqual({ score: 100, status: 'certain' });
    expect(summaryToRing({})).toEqual({ score: 0, status: 'certain' });
  });

  it('signed-in user with no history sees the first-upload invitation', async () => {
    renderDash(USER, services({ rows: [] }));
    const empty = await screen.findByTestId('recent-empty');
    expect(empty.textContent).toMatch(/first manuscript/i);
  });
});

describe('upgrade nudge (PublishReady tease)', () => {
  it('shows for signed-out users', async () => {
    renderDash(null);
    await screen.findByTestId('home-dashboard');
    expect(await screen.findByTestId('upgrade-nudge')).toBeTruthy();
  });

  it('shows for signed-in FREE users', async () => {
    renderDash(USER, services({ tier: 'free' }));
    expect(await screen.findByTestId('upgrade-nudge')).toBeTruthy();
  });

  it('hides for PREMIUM users', async () => {
    renderDash(USER, services({ tier: 'premium' }));
    // wait until the tier resolves (premium badge appears), then assert absence
    await screen.findByText('premium');
    expect(screen.queryByTestId('upgrade-nudge')).toBeNull();
  });
});

describe('entry points into the upload flow', () => {
  it('dropping a manuscript file routes to the upload flow with the file name', async () => {
    renderDash(null);
    const dropzone = await screen.findByTestId('dropzone');
    fireEvent.drop(dropzone, {
      dataTransfer: { files: [new File(['x'], 'paper.pdf', { type: 'application/pdf' })] },
    });
    await screen.findByTestId('upload-page');
    const queued = screen.getByTestId('upload-file');
    expect(queued.textContent).toContain('paper.pdf');
  });

  it('?sample=1 (from onboarding) redirects into the sample upload path', async () => {
    renderDash(null, services(), '/app?sample=1');
    await screen.findByTestId('upload-page');
    expect(screen.getByTestId('upload-sample')).toBeTruthy();
  });

  it('"Start a new analysis" navigates to the upload flow', async () => {
    renderDash(null);
    fireEvent.click(await screen.findByTestId('start-analysis'));
    await screen.findByTestId('upload-page');
    expect(screen.getByTestId('upload-fresh')).toBeTruthy();
  });
});

describe('offline / signed-out graceful state', () => {
  it('shows local-only recents note, offline badge, sign-in nudge — and never calls Supabase', async () => {
    const svc = services();
    renderDash(null, svc);
    await screen.findByTestId('home-dashboard');
    expect(screen.getByTestId('recent-empty').textContent).toMatch(/locally/i);
    expect(screen.getByText('offline')).toBeTruthy();
    expect(screen.getByTestId('signin-nudge')).toBeTruthy();
    expect(svc.historyService.list).not.toHaveBeenCalled();
    expect(svc.subscriptionService.getTier).not.toHaveBeenCalled();
    // community pulse placeholder renders with integrity badges
    expect(screen.getByTestId('community-pulse')).toBeTruthy();
  });
});
