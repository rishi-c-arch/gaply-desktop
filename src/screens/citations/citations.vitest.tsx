// F8 — Citation Manager tests. Mocked refverify + citation service, no network.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CitationManagerPage from './CitationManagerPage';
import { makeMockRefVerify, ReferenceVerification } from './refverifyBridge';
import { makeMockResolve } from './metadataBridge';
import { makeMockLocalLibrary } from './localLibrary';
import { setCloudConsent } from '../settings/settingsStore';
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

/* ----------------- dedupe: same DOI twice never duplicates --------------- */
describe('dedupe (Set 2a) — add-by-DOI is a library invariant', () => {
  it('adding a DOI already in the library is skipped (no dup, no fetch) and says so', async () => {
    let verifyCalls = 0;
    const rv = {
      async verify(ref: any) {
        verifyCalls += 1;
        return { ...okVerify, exists: { ...okVerify.exists!, doi: ref.doi } };
      },
    };
    // Seed the library with a DOI'd citation; then add the SAME DOI by hand.
    renderCM({ refverify: rv, initialCitations: [seed({ id: 's1', doi: '10.1/dup' })] });
    await screen.findByTestId('citation-manager');

    fireEvent.change(screen.getByTestId('doi-input'), { target: { value: 'https://doi.org/10.1/DUP' } });
    fireEvent.click(screen.getByTestId('add-doi'));

    await screen.findByText(/Already in your library/i);
    // no second row, and the verify network was never called (skipped before it)
    expect(within(screen.getByTestId('citation-list')).getAllByText(/Seed Paper/).length).toBe(1);
    expect(verifyCalls).toBe(0);
  });
});

/* ---------------------- import (Set 2b-i) — the UI flow ------------------ */
describe('import — parse/dedupe/report, never a network call', () => {
  const bibFile = (text: string) => new File([text], 'lib.bib', { type: 'text/plain' });
  const TWO = `@article{a, title={Alpha Study}, author={Doe, Jane}, year={2020}, doi={10.1/alpha}}
@article{b, title={Beta Study}, author={Roe, Ann}, year={2021}, doi={10.1/beta}}`;

  it('pin a: import makes ZERO resolver/verify calls — consent ON and OFF', async () => {
    for (const consent of [true, false]) {
      setCloudConsent('citation_verification', consent);
      const resolver = makeMockResolve(() => ({
        status: 'verified',
        metadata: { source: 'crossref', matched_by: 'doi', csl_type: 'article-journal', doi: '10.1/x', title: 'X', authors: [], container_title: null, year: null, volume: null, issue: null, page: null },
      }));
      let verifyCalls = 0;
      const rv = { async verify() { verifyCalls += 1; return okVerify; } };
      renderCM({ metadataResolver: resolver, refverify: rv, localLibrary: makeMockLocalLibrary() });
      await screen.findByTestId('citation-manager');

      fireEvent.change(screen.getByTestId('import-input'), { target: { files: [bibFile(TWO)] } });
      await screen.findByTestId('import-report');

      expect(resolver.calls.length).toBe(0); // structural: import never resolves
      expect(verifyCalls).toBe(0);
      expect(screen.getByTestId('import-tally').textContent).toMatch(/2 added/);
      cleanup();
    }
    setCloudConsent('citation_verification', true);
  });

  it('pin e: imported entries show "not verified online" and cannot look verified', async () => {
    renderCM({ localLibrary: makeMockLocalLibrary() });
    await screen.findByTestId('citation-manager');
    fireEvent.change(screen.getByTestId('import-input'), { target: { files: [bibFile(TWO)] } });
    await screen.findByTestId('import-report');
    expect(screen.getAllByTestId('unverified-badge').length).toBe(2);
    expect(screen.getAllByTestId('unverified-badge')[0].textContent).toMatch(/not verified online/i);
  });

  it('pin d: a duplicate DOI is auto-skipped, INSPECTABLE, with Add anyway', async () => {
    renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: [seed({ id: 'have', doi: '10.1/alpha' })] });
    await screen.findByTestId('citation-manager');

    fireEvent.change(screen.getByTestId('import-input'), { target: { files: [bibFile(TWO)] } });
    await screen.findByTestId('import-report');

    // Alpha collides (DOI 10.1/alpha already present) → skipped + inspectable; Beta added
    expect(screen.getByTestId('import-tally').textContent).toMatch(/1 added.*1 skipped as duplicates/);
    const skip = screen.getByTestId('import-skipped-item');
    expect(skip.textContent).toMatch(/Alpha Study — matches “Seed Paper”/);
    expect(within(screen.getByTestId('citation-list')).getByText(/Beta Study/)).toBeTruthy();
    // Alpha is NOT in the list yet (auto-skipped)
    expect(within(screen.getByTestId('citation-list')).queryByText(/Alpha Study/)).toBeNull();

    // Add anyway → the user's override forces it in
    fireEvent.click(screen.getByTestId('import-add-anyway'));
    await screen.findByText(/Added anyway/i);
    await waitFor(() => expect(within(screen.getByTestId('citation-list')).getByText(/Alpha Study/)).toBeTruthy());
  });
});

/* --------- consent: the from-file lane honors citation_verification ------ */
// The from-file add resolves a paper's DOI against CrossRef — the SAME cloud
// lane as add-by-DOI + the retraction sweep. Opt-out must be a HARD door:
// zero fetches, the way AI Check's zero-network promise is pinned.
describe('consent gate — from-file resolution', () => {
  const verified = () =>
    makeMockResolve(() => ({
      status: 'verified',
      metadata: { source: 'crossref', matched_by: 'doi', csl_type: 'article-journal', doi: '10.1/x', title: 'X', authors: [], container_title: null, year: null, volume: null, issue: null, page: null },
    }));

  afterEach(() => setCloudConsent('citation_verification', true)); // restore default

  it('opt-out → the from-file add makes ZERO resolver fetches and says why', async () => {
    setCloudConsent('citation_verification', false);
    const resolver = verified();
    renderCM({ metadataResolver: resolver, localLibrary: makeMockLocalLibrary() });
    await screen.findByTestId('citation-manager');

    fireEvent.change(screen.getByTestId('add-file'), {
      target: { files: [new File(['x'], 'paper.pdf', { type: 'application/pdf' })] },
    });

    await screen.findByText(/turned off in Settings/i);
    expect(resolver.calls.length).toBe(0); // the hard door: no network, ever
  });

  it('opt-in → the same add DOES resolve (control — proves the gate, not a broken lane)', async () => {
    setCloudConsent('citation_verification', true);
    const resolver = verified();
    renderCM({ metadataResolver: resolver, localLibrary: makeMockLocalLibrary() });
    await screen.findByTestId('citation-manager');

    fireEvent.change(screen.getByTestId('add-file'), {
      target: { files: [new File(['x'], 'paper.pdf', { type: 'application/pdf' })] },
    });

    await waitFor(() => expect(resolver.calls.length).toBe(1));
  });
});
