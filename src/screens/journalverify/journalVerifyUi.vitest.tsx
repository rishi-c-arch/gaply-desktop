// Set 5 — the evidence-not-verdict UI. Render + mock-bridge tests: entitlement
// gating, name/link entry, disambiguation, the VERIFIED-vs-CLAIMED visual
// distinction, source citations, the not-found honest alert, no verdict
// language, and the un-strippable disclosure.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import JournalVerifyPage from './JournalVerifyPage';
import JournalVerifyReport, { DISCLOSURE } from './JournalVerifyReport';
import { makeMockJournalVerifyBridge } from './journalVerifyBridge';
import { JournalVerificationResult } from './journalVerifyTypes';

afterEach(cleanup);

function auth(session: any = null): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

const COMBINED: JournalVerificationResult = {
  input: 'https://journal.example.org/about',
  input_kind: 'link',
  disambiguation: [],
  not_found: false,
  registry: {
    query: 'Journal of Sleep Research (1365-2869)', name: 'Journal of Sleep Research', issn: '1365-2869',
    doaj_registered: true, openalex_in_doaj: true, pubmed_indexed: true,
    works_by_year: [{ year: 2025, works: 180 }], recent_activity: true, scope: ['Sleep medicine'],
    warning: null, reasons: [], verified_sources: ['openalex', 'doaj', 'pubmed'], unverified: [],
    sources_not_checked: ['Scopus (no free API — not checked; absence here is not a signal)', 'Web of Science (no free API — not checked; absence here is not a signal)'],
  },
  site_summary: {
    status: 'summarized', source_url: 'https://journal.example.org/about',
    details: [
      { field: 'apc', value: '$1200', source: 'https://journal.example.org/about', dropped_unverifiable: false },
      { field: 'review_timeline', value: null, source: 'https://journal.example.org/about', dropped_unverifiable: false },
      { field: 'guidelines_link', value: 'https://journal.example.org/guide', source: 'https://journal.example.org/about', dropped_unverifiable: false },
      { field: 'contact', value: 'editor@journal.example.org', source: 'https://journal.example.org/about', dropped_unverifiable: false },
    ],
    label: 'These are the journal’s OWN claims from its website — self-reported, not independently verified.',
    notice: null, page_truncated: false,
  },
  notes: [],
};

const NOTFOUND: JournalVerificationResult = {
  input: 'mystery journal', input_kind: 'name', disambiguation: [], not_found: true,
  registry: null, site_summary: null, notes: ["couldn't resolve this name in OpenAlex (no matching journal)"],
};

const MULTI: JournalVerificationResult = {
  input: 'advances in science', input_kind: 'name', not_found: false, registry: null, site_summary: null, notes: [],
  disambiguation: [
    { name: 'Advances in Science', issn: '1111-1111' },
    { name: 'Advances in Science and Technology', issn: '2222-2222' },
  ],
};

const renderPage = (bridge: any, forceTier: 'free' | 'premium' = 'premium') =>
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(null)}>
        <JournalVerifyPage bridge={bridge} forceTier={forceTier} />
      </GaplySessionProvider>
    </MemoryRouter>
  );

/* ----------------------------- entitlement ----------------------------- */

describe('JournalVerifyPage — paid gating', () => {
  it('unentitled → the honest gate/upsell, never the tool', () => {
    renderPage(makeMockJournalVerifyBridge({}), 'free');
    expect(screen.getByTestId('jv-teaser')).toBeTruthy();
    expect(screen.getByTestId('jv-unlock')).toBeTruthy();
    expect(screen.queryByTestId('jv-entry')).toBeNull();
  });
  it('entitled → the tool', async () => {
    renderPage(makeMockJournalVerifyBridge({}), 'premium');
    expect(await screen.findByTestId('jv-entry')).toBeTruthy();
    expect(screen.queryByTestId('jv-teaser')).toBeNull();
  });
});

/* --------------------------- entry + report ---------------------------- */

describe('JournalVerifyPage — entry, disambiguation, report', () => {
  it('name/link entry → verify → renders the report', async () => {
    const bridge = makeMockJournalVerifyBridge({ byQuery: { 'https://journal.example.org/about': COMBINED } });
    renderPage(bridge);
    fireEvent.change(await screen.findByTestId('jv-input'), { target: { value: 'https://journal.example.org/about' } });
    fireEvent.click(screen.getByTestId('jv-check'));
    await screen.findByTestId('jv-report');
    expect(bridge.calls).toEqual([['query', 'https://journal.example.org/about']]);
  });

  it('multiple matches → disambiguation picker (never auto-picked); picking → verifyByIssn', async () => {
    const bridge = makeMockJournalVerifyBridge({ byQuery: { 'advances in science': MULTI }, byIssn: { '1111-1111': COMBINED } });
    renderPage(bridge);
    fireEvent.change(await screen.findByTestId('jv-input'), { target: { value: 'advances in science' } });
    fireEvent.click(screen.getByTestId('jv-check'));
    // the picker is shown; NO report yet (not auto-picked)
    await screen.findByTestId('jv-disambiguation');
    expect(screen.queryByTestId('jv-report')).toBeNull();
    fireEvent.click(screen.getByTestId('jv-pick-1111-1111'));
    await screen.findByTestId('jv-report');
    expect(bridge.calls).toEqual([['query', 'advances in science'], ['issn', '1111-1111']]);
  });
});

/* ------------- the core: verified vs claimed vs unchecked -------------- */

describe('JournalVerifyReport — verified vs claimed visually distinct', () => {
  it('grounded facts and self-reported claims are SEPARATE, distinctly labeled sections', () => {
    render(<JournalVerifyReport result={COMBINED} />);
    const verified = screen.getByTestId('jv-registry');
    const claimed = screen.getByTestId('jv-claims');
    expect(verified).toBeTruthy();
    expect(claimed).toBeTruthy();
    // verified facts carry a source citation; claims are labeled unverified
    expect(within(verified).getByTestId('jv-fact-doaj').textContent).toMatch(/verified from DOAJ/);
    expect(within(verified).getByTestId('jv-fact-pubmed').textContent).toMatch(/verified from NLM/);
    expect(screen.getByTestId('jv-claims-label').textContent).toMatch(/self-reported, not independently verified/);
    // a claim value lives ONLY in the claims section, never in the verified one
    expect(within(claimed).getByTestId('jv-claim-value-apc').textContent).toContain('$1200');
    expect(within(verified).queryByTestId('jv-claim-value-apc')).toBeNull();
    // a self-reported detail cites the SITE, not a registry
    expect(within(claimed).getByTestId('jv-claim-apc').textContent).toMatch(/claimed on journal.example.org/);
    // "not stated" for an absent claim, never a guess
    expect(within(claimed).getByTestId('jv-claim-notstated-review_timeline').textContent).toMatch(/not stated/);
  });

  it('sources-not-checked (Scopus/WoS) shown honestly — not a signal', () => {
    render(<JournalVerifyReport result={COMBINED} />);
    const s = screen.getByTestId('jv-sources-not-checked').textContent!;
    expect(s).toMatch(/Scopus/);
    expect(s).toMatch(/Web of Science/);
    expect(s).toMatch(/not a signal/);
  });

  it('the un-strippable disclosure is present + non-empty on every report', () => {
    const { rerender } = render(<JournalVerifyReport result={COMBINED} />);
    expect(screen.getByTestId('jv-disclosure').textContent).toBe(DISCLOSURE);
    expect(DISCLOSURE.length).toBeGreaterThan(50);
    // and on a not-found report too
    rerender(<JournalVerifyReport result={NOTFOUND} />);
    expect(screen.getByTestId('jv-disclosure').textContent).toBe(DISCLOSURE);
  });

  it('NOT-FOUND → honest warning, explicitly NOT a verdict', () => {
    render(<JournalVerifyReport result={NOTFOUND} />);
    const nf = screen.getByTestId('jv-notfound').textContent!;
    expect(nf).toMatch(/warning sign/);
    expect(nf).toMatch(/not-?found is not proof|not proof/i);
    expect(nf).toMatch(/legitimate new or niche journals/);
  });

  it('renders NO verdict/conclusion language (evidence + signals only)', () => {
    const { container } = render(<JournalVerifyReport result={COMBINED} />);
    const text = container.textContent!.toLowerCase();
    // no conclusion that a journal IS/ISN'T predatory/legit/safe
    expect(text).not.toMatch(/this journal is (predatory|legitimate|safe|trustworthy|not predatory)/);
    expect(text).not.toMatch(/verdict:|conclusion:|rating:|score:/);
    // "predatory" may appear ONLY inside the negating disclosure phrase, never as
    // a conclusion — so any occurrence must be in "…not proof that a journal is predatory".
    for (const m of Array.from(text.matchAll(/predatory/g))) {
      const around = text.slice(Math.max(0, m.index! - 30), m.index! + 10);
      expect(around).toMatch(/not proof that a journal is predatory/);
    }
  });
});
