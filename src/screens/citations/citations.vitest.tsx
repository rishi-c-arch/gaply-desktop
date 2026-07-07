// F8 — Citation Manager tests. Mocked refverify + citation service, no network.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CitationManagerPage from './CitationManagerPage';
import { makeMockRefVerify, ReferenceVerification } from './refverifyBridge';
import { formatCitation } from './formatCitation';
import { Citation } from './citationTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const USER = { user: { id: 'u1', email: 'a@b.c' } };
function auth(session: any = USER): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: false }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

const okVerify: ReferenceVerification = {
  reference_raw: 'x',
  exists: { found: true, source: 'crossref', doi: '10.1/abc', title: { safe_text: 'A Verified Paper', suspicious: false }, is_retracted_hint: false },
  retraction: { retracted: false, reasons: [], notice_url: null },
  enrichment: { citation_count: 12, abstract_text: null, venue: { safe_text: 'Journal of Rest', suspicious: false } },
  open_access: null,
  provenance: [{ source: 'crossref', url: 'https://api.crossref.org/works/10.1/abc' }],
  warnings: [],
};

const retractedVerify: ReferenceVerification = {
  ...okVerify,
  exists: { ...okVerify.exists!, doi: '10.1/bad', is_retracted_hint: true },
  retraction: { retracted: true, reasons: [{ safe_text: 'data fabrication', suspicious: false }], notice_url: 'https://retract' },
};

function mockLib() {
  return {
    add: vi.fn().mockResolvedValue({ data: {}, error: null, offline: false }),
    list: vi.fn(),
    remove: vi.fn(),
  } as any;
}

function renderCM(props: any = {}, session: any = USER) {
  const lib = props.citationService ?? mockLib();
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <CitationManagerPage {...props} citationService={lib} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
  return lib;
}

const seed = (over: Partial<Citation> = {}): Citation => ({
  id: over.id ?? 'seed-1',
  csl: { id: 's', type: 'article-journal', title: 'Seed Paper', author: [{ family: 'Doe', given: 'Jane' }], issued: { year: 2022 }, containerTitle: 'J. Rest', volume: '1', issue: '1', page: '1-9', DOI: '10.1/seed', ...(over.csl as any) },
  doi: over.doi ?? '10.1/seed',
  retracted: over.retracted ?? false,
  source: over.source ?? 'manual',
  ...over,
});

/* ------------------------------ add by DOI ------------------------------ */

describe('add by DOI', () => {
  it('verifies + enriches via mocked refverify and syncs metadata to Supabase', async () => {
    const lib = renderCM({ refverify: makeMockRefVerify(okVerify) });
    fireEvent.change(await screen.findByTestId('doi-input'), { target: { value: 'https://doi.org/10.1/abc' } });
    fireEvent.click(screen.getByTestId('add-doi'));

    await waitFor(() => expect(screen.getByText('A Verified Paper')).toBeTruthy());
    // enriched: journal came from refverify
    await waitFor(() => expect(lib.add).toHaveBeenCalled());
    const row = lib.add.mock.calls[0][0];
    expect(row.doi).toBe('10.1/abc');
    expect(row.journal).toBe('Journal of Rest');
    expect(row.retracted_flag).toBe(false);
    // METADATA ONLY — no manuscript text field
    const serialized = JSON.stringify(row);
    expect(serialized).not.toMatch(/manuscript|fullText|body|rawText/i);
  });
});

/* ------------------------------ retractions ----------------------------- */

describe('retraction handling', () => {
  it('a retracted citation shows the red flag and appears in Retracted Items', async () => {
    renderCM({ refverify: makeMockRefVerify(retractedVerify) });
    fireEvent.change(await screen.findByTestId('doi-input'), { target: { value: '10.1/bad' } });
    fireEvent.click(screen.getByTestId('add-doi'));

    // list item shows the 🔴 icon + retracted badge
    await waitFor(() => expect(screen.getByText('retracted')).toBeTruthy());
    expect(screen.getByTestId('retract-banner')).toBeTruthy();

    // switch to Retracted Items → it's there
    fireEvent.click(screen.getByTestId('collection-retracted'));
    expect(screen.getByTestId('collection-retracted').textContent).toMatch(/Retracted Items \(1\)/);
    expect(within(screen.getByTestId('citation-list')).getByText(/A Verified Paper/)).toBeTruthy();
  });

  it('"check all for retractions" flags a previously-clean citation', async () => {
    // seeded clean, but the sweep returns retracted
    renderCM({
      initialCitations: [seed({ id: 's1', doi: '10.1/x', retracted: false })],
      refverify: makeMockRefVerify(retractedVerify),
    });
    fireEvent.click(await screen.findByTestId('check-retractions'));
    await waitFor(() => expect(screen.getByTestId('collection-retracted').textContent).toMatch(/\(1\)/));
  });
});

/* ------------------------------ CSL styles ------------------------------ */

describe('CSL formatting', () => {
  it('style switch reformats the preview', async () => {
    renderCM({ initialCitations: [seed()] });
    await screen.findByTestId('detail');
    const preview = screen.getByTestId('preview');
    const apa = preview.textContent;

    fireEvent.change(screen.getByTestId('preview-style'), { target: { value: 'ieee' } });
    const ieee = screen.getByTestId('preview').textContent;
    expect(ieee).not.toBe(apa);
    // IEEE puts the year at the end; APA puts it near the front in parens
    expect(ieee).toMatch(/2022\.$/);
  });

  it('formatCitation renders distinct output per style (unit)', () => {
    const csl = seed().csl;
    const styles = ['apa', 'mla', 'vancouver', 'ieee', 'nature'].map((s) => formatCitation(csl, s));
    expect(new Set(styles).size).toBe(styles.length); // all distinct
  });

  it('bulk reformat changes the style applied across the library + preview', async () => {
    renderCM({ initialCitations: [seed()] });
    await screen.findByTestId('detail');
    fireEvent.change(screen.getByTestId('bulk-style'), { target: { value: 'vancouver' } });
    // preview follows the bulk style (they share the `style` state)
    await waitFor(() => expect(screen.getByTestId('preview').textContent).toMatch(/Doe J\. Seed Paper\. J\. Rest\. 2022/));
  });
});

/* --------------------------- add ways 1 & 3 ----------------------------- */

describe('add ways', () => {
  it('imports auto-extracted citations from an analyzed manuscript', async () => {
    const extracted = [seed({ id: 'ex1', source: 'extracted', doi: '10.1/ex1' }), seed({ id: 'ex2', source: 'extracted', doi: '10.1/ex2' })];
    const lib = renderCM({ extractedCitations: extracted });
    fireEvent.click(await screen.findByTestId('import-extracted'));
    await waitFor(() => expect(lib.add).toHaveBeenCalledTimes(2));
    expect(screen.getByTestId('collection-manuscript')).toBeTruthy();
  });

  it('manual entry adds a citation', async () => {
    const lib = renderCM();
    fireEvent.click(await screen.findByTestId('add-manual'));
    await waitFor(() => expect(lib.add).toHaveBeenCalledTimes(1));
    expect(within(screen.getByTestId('citation-list')).getByText(/Untitled/)).toBeTruthy();
  });
});

/* -------------------------- offline: no sync ---------------------------- */

describe('offline', () => {
  it('works with no session and does not call Supabase', async () => {
    const lib = renderCM({ initialCitations: [seed()] }, null);
    await screen.findByTestId('citation-manager');
    fireEvent.click(screen.getByTestId('add-manual'));
    await waitFor(() => expect(within(screen.getByTestId('citation-list')).getByText(/Untitled/)).toBeTruthy());
    expect(lib.add).not.toHaveBeenCalled();
  });
});
