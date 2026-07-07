// F9 — Journal Check tests. Mocked online lookup, no network.
import React from 'react';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import JournalCheckPage from './JournalCheckPage';
import { JournalRecord, JOURNALS, riskVerdict, searchLocal } from './journalData';
import { CachedJournalLookup, makeMockLookup } from './journalLookup';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const auth: AuthService = {
  signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
  getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
  onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
} as any;

function renderJC(lookup?: any) {
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth}>
        <JournalCheckPage lookup={lookup} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

async function search(q: string) {
  fireEvent.change(await screen.findByTestId('search-input'), { target: { value: q } });
  fireEvent.click(screen.getByTestId('search-btn'));
}

const PREDATORY: JournalRecord = {
  name: 'World Journal of Advanced Multidisciplinary Research',
  issn: null,
  publisher: 'Global Science Publishing House',
  quartile: null,
  sjr: null,
  guidelinesUrl: 'http://wjamr.example/submit',
  category: '',
  indexing: { scopus: false, wos: false },
  signals: ['Guaranteed publication in 72 hours', 'No verifiable indexing', 'Fake impact factor claimed'],
  source: 'online',
};

const LEGIT_ONLINE: JournalRecord = {
  name: 'Journal of Fictional Physics',
  issn: '1234-5678',
  publisher: 'Springer',
  quartile: 'Q1',
  sjr: 2.415,
  guidelinesUrl: 'https://jfp.example/guidelines',
  category: 'Physics',
  indexing: { scopus: true, wos: true },
  signals: [],
  source: 'online',
};

/* --------------------------- known journal ------------------------------ */

describe('known journal search', () => {
  it('returns the correct quartile badge + SJR field for a directory journal', async () => {
    renderJC();
    // "The Lancet" is Q1 in the bundled directory
    await search('The Lancet');
    const card = await screen.findByTestId('result-card');
    expect(screen.getByTestId('quartile-badge').textContent).toBe('Q1');
    expect(screen.getByTestId('quartile-badge').getAttribute('data-q')).toBe('Q1');
    // SJR field present (quartile shown when no numeric SJR)
    expect(screen.getByTestId('sjr').textContent).toMatch(/Q1 quartile|\d/);
    // green traffic light — well indexed
    expect(screen.getByTestId('risk-verdict').getAttribute('data-level')).toBe('green');
    expect(card.textContent).toMatch(/Elsevier/);
  });

  it('searchLocal matches by ISSN too', () => {
    const lancet = JOURNALS.find((j) => j.name === 'The Lancet');
    expect(lancet?.issn).toBeTruthy();
    const byIssn = searchLocal(lancet!.issn!);
    expect(byIssn.some((j) => j.name === 'The Lancet')).toBe(true);
  });
});

/* --------------------------- online fallback ---------------------------- */

describe('online fallback', () => {
  it('an unknown journal triggers the online lookup (mocked)', async () => {
    const mock = makeMockLookup(() => LEGIT_ONLINE);
    renderJC(mock);
    await search('Journal of Fictional Physics');
    await screen.findByTestId('result-card');
    expect(mock.calls.length).toBe(1); // online path was taken
    expect(screen.getByTestId('quartile-badge').textContent).toBe('Q1');
    expect(screen.getByTestId('sjr').textContent).toMatch(/2\.415/); // numeric SJR from online
  });

  it('CachedJournalLookup caches within the TTL (second call is a hit)', async () => {
    let n = 0;
    const inner = makeMockLookup(() => { n++; return LEGIT_ONLINE; });
    const cached = new CachedJournalLookup(inner, 1000, () => 500);
    await cached.lookup('X');
    await cached.lookup('X');
    expect(n).toBe(1); // only one underlying call
    expect(cached.cachedKeys()).toContain('x');
  });
});

/* --------------------------- predatory verdict -------------------------- */

describe('predatory-signal journal', () => {
  it('shows the RED traffic-light verdict', async () => {
    renderJC(makeMockLookup(() => PREDATORY));
    await search('World Journal of Advanced Multidisciplinary Research');
    await screen.findByTestId('result-card');
    expect(screen.getByTestId('risk-verdict').getAttribute('data-level')).toBe('red');
    expect(screen.getByTestId('risk-verdict').textContent).toMatch(/predatory/i);
  });

  it('riskVerdict: unindexed + signals → red; Q1 Scopus+WoS → green (unit)', () => {
    expect(riskVerdict(PREDATORY).level).toBe('red');
    expect(riskVerdict(LEGIT_ONLINE).level).toBe('green');
  });
});

/* ---------------------- UGC-CARE appears nowhere ------------------------ */

describe('no national list', () => {
  it('UGC-CARE appears nowhere in the Journal Check code', () => {
    const dir = __dirname;
    for (const f of readdirSync(dir)) {
      if (!f.endsWith('.ts') && !f.endsWith('.tsx')) continue;
      const src = readFileSync(join(dir, f), 'utf8').toLowerCase();
      // allow the string only inside this assertion, not the feature code
      if (f.includes('vitest')) continue;
      expect(src, `UGC-CARE must not appear in ${f}`).not.toMatch(/ugc[\s-]?care/);
    }
  });

  it('the bundled directory JSON carries no UGC-CARE', () => {
    const json = readFileSync(join(__dirname, '../../data/scopusDirectory.json'), 'utf8').toLowerCase();
    expect(json).not.toMatch(/ugc[\s-]?care/);
  });
});

/* ------------------------------- conference ----------------------------- */

describe('conference tab', () => {
  it('ranks a CORE A* conference green and an unranked one red', async () => {
    renderJC();
    fireEvent.click(await screen.findByTestId('tab-conference'));
    await search('NeurIPS');
    await screen.findByTestId('conf-card');
    expect(screen.getByTestId('core-rank').textContent).toMatch(/A\*/);
    expect(screen.getByTestId('conf-risk').getAttribute('data-level')).toBe('green');
  });
});
