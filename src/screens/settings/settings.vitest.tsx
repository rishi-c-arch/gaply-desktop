// F14 — Settings & account tests: privacy toggles persist AND gate real cloud
// call-sites, usage meters read usage_counters, clear-local-data spares
// Supabase account data, connectivity reflects online/offline, appearance
// persists, and the future-SLM note is shown.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import SettingsPage from './SettingsPage';
import JournalCheckPage from '../journal/JournalCheckPage';
import CopilotPage from '../copilot/CopilotPage';
import { makeMockLookup } from '../journal/journalLookup';
import {
  clearLocalData,
  formatBytes,
  listLocalData,
  mayUseCloud,
  readAppearance,
  readCloudConsent,
  setCloudConsent,
} from './settingsStore';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const USER = { user: { id: 'u1', email: 'me@lab.edu' } };
function auth(session: any = USER): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

const PROFILE = {
  id: 'u1', email: 'me@lab.edu', display_name: 'Rishi', role: 'phd', field: 'neuroscience',
  orcid: null, country: 'IN', target_journals: null, created_at: '2026-01-01T00:00:00Z',
};
function mockProfiles(overrides: any = {}) {
  return {
    getOwn: vi.fn().mockResolvedValue({ data: PROFILE, error: null, offline: false }),
    upsertOwn: vi.fn().mockResolvedValue({ data: PROFILE, error: null, offline: false }),
    ...overrides,
  } as any;
}
function mockUsage(counts: Record<string, number> = {}) {
  return {
    get: vi.fn(async (_u: string, feature: string) => ({
      data: feature in counts ? { user_id: 'u1', feature, count: counts[feature], period_start: '2026-07-01' } : null,
      error: null,
      offline: false,
    })),
    increment: vi.fn(),
  } as any;
}

function renderSettings(props: any = {}, session: any = USER) {
  const profileService = props.profileService ?? mockProfiles();
  const usageService = props.usageService ?? mockUsage();
  const utils = render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <SettingsPage {...props} profileService={profileService} usageService={usageService} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
  return { profileService, usageService, ...utils };
}

/* --------------------- privacy toggles: persist + gate -------------------- */

describe('privacy toggles persist and gate the cloud', () => {
  it('toggling a suite off persists to localStorage and flips mayUseCloud', async () => {
    renderSettings();
    const box = (await screen.findByTestId('privacy-journal_check')) as HTMLInputElement;
    expect(box.checked).toBe(true); // consent defaults to allowed
    fireEvent.click(box);
    expect(mayUseCloud('journal_check')).toBe(false);
    expect(JSON.parse(localStorage.getItem('gaply.settings.privacy')!)).toMatchObject({ journal_check: false });
    // other suites untouched
    expect(mayUseCloud('research_copilot')).toBe(true);
  });

  it('the toggle survives a remount (persistence, not component state)', async () => {
    const first = renderSettings();
    fireEvent.click(await screen.findByTestId('privacy-citation_verification'));
    first.unmount();
    renderSettings();
    const box = (await screen.findByTestId('privacy-citation_verification')) as HTMLInputElement;
    expect(box.checked).toBe(false);
  });

  it('Journal Check will NOT call the online fallback when its toggle is off', async () => {
    setCloudConsent('journal_check', false);
    const lookup = makeMockLookup(() => null);
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth(null)}>
          <JournalCheckPage lookup={lookup} />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    fireEvent.change(await screen.findByTestId('search-input'), { target: { value: 'Totally Unknown Journal' } });
    fireEvent.click(screen.getByTestId('search-btn'));
    expect(await screen.findByTestId('cloud-off')).toBeTruthy();
    expect(lookup.calls).toHaveLength(0); // the gate stopped the network path
  });

  it('Journal Check DOES call online with consent on (control)', async () => {
    const lookup = makeMockLookup(() => null);
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth(null)}>
          <JournalCheckPage lookup={lookup} />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    fireEvent.change(await screen.findByTestId('search-input'), { target: { value: 'Totally Unknown Journal' } });
    fireEvent.click(screen.getByTestId('search-btn'));
    await waitFor(() => expect(lookup.calls.length).toBeGreaterThan(0));
  });

  it('Community posting never reaches Supabase with its consent off', async () => {
    setCloudConsent('community_sync', false);
    const CommunityPage = (await import('../community/CommunityPage')).default;
    const svc = {
      listChannels: vi.fn(),
      listPosts: vi.fn(),
      post: vi.fn().mockResolvedValue({ data: { id: 'srv-1' }, error: null, offline: false }),
    } as any;
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <CommunityPage communityService={svc} />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    fireEvent.change(await screen.findByTestId('composer-input'), {
      target: { value: 'A perfectly reasonable question about my ANOVA design?' },
    });
    fireEvent.click(screen.getByTestId('post-btn'));
    await waitFor(() => expect(screen.queryByText(/turned off in Settings/i)).toBeTruthy());
    expect(svc.post).not.toHaveBeenCalled();
  });

  it('Copilot (premium) is unavailable with its cloud consent off', async () => {
    setCloudConsent('research_copilot', false);
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <CopilotPage forceTier="premium" />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    expect(await screen.findByTestId('copilot-cloud-off')).toBeTruthy();
    expect(screen.queryByTestId('copilot-teaser')).toBeNull();
  });
});

/* -------------------- where your data lives (trust panel) ------------------ */

describe('"Where your data lives" privacy panel', () => {
  it('shows Device vs Cloud with the on-device, never-uploaded statement', async () => {
    renderSettings();
    const panel = await screen.findByTestId('privacy-panel');
    expect(within(panel).getByTestId('device-box').textContent).toMatch(/Manuscripts/i);
    expect(within(panel).getByTestId('cloud-box').textContent).toMatch(/structured summaries/i);
    expect(within(panel).getByTestId('cloud-box').textContent).not.toMatch(/manuscript/i);
    expect(screen.getByTestId('on-device-statement').textContent).toMatch(/on-device/i);
    expect(screen.getByTestId('on-device-statement').textContent).toMatch(/never uploaded/i);
  });
});

/* ------------------------------- usage meters ------------------------------ */

describe('usage meters read from usage_counters', () => {
  it('meters show the stored per-feature counts for a free user', async () => {
    renderSettings({ usageService: mockUsage({ citation_verification: 4, journal_check: 1 }) });
    const meters = await screen.findByTestId('settings-usage-meters');
    await waitFor(() => {
      expect(within(meters).getByLabelText('Citation verifications').getAttribute('aria-valuenow')).toBe('4');
      expect(within(meters).getByLabelText('Journal checks').getAttribute('aria-valuenow')).toBe('1');
    });
    // caps come from the F12 tier table
    expect(within(meters).getByLabelText('Citation verifications').getAttribute('aria-valuemax')).toBe('5');
    expect(within(meters).getByLabelText('Journal checks').getAttribute('aria-valuemax')).toBe('3');
  });

  it('meters are hidden signed-out (no counters to read)', async () => {
    renderSettings({}, null);
    await screen.findByTestId('settings-sections');
    expect(screen.queryByTestId('settings-usage-meters')).toBeNull();
  });
});

/* ------------------------------ clear local data --------------------------- */

describe('clear local data', () => {
  it('wipes only gaply.* keys — Supabase auth/account keys survive, no service call', async () => {
    localStorage.setItem('gaply.cache.journal', JSON.stringify({ q: 'x' }));
    localStorage.setItem('gaply.settings.appearance', JSON.stringify({ theme: 'dark', uiScale: 90 }));
    localStorage.setItem('sb-abc123-auth-token', '{"access_token":"keep-me"}');
    const { profileService } = renderSettings();

    const btn = await screen.findByTestId('clear-local');
    fireEvent.click(btn); // arm
    expect(screen.getByTestId('clear-warning')).toBeTruthy();
    fireEvent.click(btn); // confirm

    expect(localStorage.getItem('gaply.cache.journal')).toBeNull();
    expect(localStorage.getItem('gaply.settings.appearance')).toBeNull();
    expect(localStorage.getItem('sb-abc123-auth-token')).toBe('{"access_token":"keep-me"}');
    // purely local: nothing was written/deleted through the Supabase services
    expect(profileService.upsertOwn).not.toHaveBeenCalled();
  });

  it('storage-used reflects the gaply.* inventory', async () => {
    localStorage.setItem('gaply.cache.big', 'x'.repeat(2048));
    renderSettings();
    expect((await screen.findByTestId('storage-used')).textContent).toMatch(/KB/);
    expect(listLocalData().some((e) => e.key === 'gaply.cache.big')).toBe(true);
  });

  it('clearLocalData/store helpers are safe standalone', () => {
    localStorage.setItem('gaply.a', '1');
    localStorage.setItem('sb-keep', '1');
    localStorage.setItem('unrelated', '1');
    expect(clearLocalData()).toBe(1);
    expect(localStorage.getItem('sb-keep')).toBe('1');
    expect(localStorage.getItem('unrelated')).toBe('1');
    expect(formatBytes(512)).toBe('512 B');
    expect(readCloudConsent().journal_check).toBe(true); // defaults restored
  });
});

/* ------------------------------- connectivity ------------------------------ */

describe('connectivity indicator', () => {
  it('reflects navigator.onLine and live online/offline events', async () => {
    const onLine = vi.spyOn(window.navigator, 'onLine', 'get').mockReturnValue(false);
    renderSettings();
    const conn = await screen.findByTestId('connectivity');
    expect(conn.getAttribute('data-online')).toBe('false');
    expect(conn.textContent).toMatch(/offline/i);

    onLine.mockReturnValue(true);
    fireEvent(window, new Event('online'));
    await waitFor(() => expect(screen.getByTestId('connectivity').getAttribute('data-online')).toBe('true'));
    onLine.mockRestore();
  });
});

/* -------------------------------- appearance ------------------------------- */

describe('appearance persists and applies', () => {
  it('theme + UI scale write to localStorage and stamp the root', async () => {
    renderSettings();
    fireEvent.click(await screen.findByTestId('theme-dark'));
    fireEvent.click(screen.getByTestId('scale-90'));

    expect(readAppearance()).toEqual({ theme: 'dark', uiScale: 90 });
    const root = screen.getByTestId('settings');
    expect(root.getAttribute('data-gds-theme')).toBe('dark');
    expect(root.style.fontSize).toBe('90%');
  });

  it('a saved appearance is restored on mount', async () => {
    localStorage.setItem('gaply.settings.appearance', JSON.stringify({ theme: 'dark', uiScale: 110 }));
    renderSettings();
    expect((await screen.findByTestId('settings')).getAttribute('data-gds-theme')).toBe('dark');
    expect(screen.getByTestId('settings').style.fontSize).toBe('110%');
  });
});

/* ------------------------------ profile & ORCID ---------------------------- */

describe('profile & ORCID', () => {
  it('saves display name + ORCID through the profile service (metadata only)', async () => {
    const { profileService } = renderSettings();
    const orcid = await screen.findByTestId('profile-orcid');
    await waitFor(() => expect((screen.getByTestId('profile-name') as HTMLInputElement).value).toBe('Rishi'));
    fireEvent.change(orcid, { target: { value: '0000-0002-1825-0097' } });
    fireEvent.click(screen.getByTestId('profile-save'));
    await waitFor(() => expect(profileService.upsertOwn).toHaveBeenCalled());
    expect(profileService.upsertOwn.mock.calls[0][0]).toMatchObject({
      id: 'u1',
      orcid: '0000-0002-1825-0097',
    });
  });

  it('signed-out users get the local-first notice instead of a form', async () => {
    renderSettings({}, null);
    expect(await screen.findByTestId('profile-offline')).toBeTruthy();
    expect(screen.queryByTestId('profile-save')).toBeNull();
  });
});

/* ---------------------------------- about ---------------------------------- */

describe('about / future-SLM note', () => {
  it('shows the local-model status line (interim heuristic, Gaply model coming)', async () => {
    renderSettings();
    const slm = await screen.findByTestId('slm-status');
    expect(slm.textContent).toMatch(/interim heuristic/i);
    expect(slm.textContent).toMatch(/Gaply\s+model coming/i);
  });
});
