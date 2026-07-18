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
    const apa = screen.getByTestId('preview').textContent;

    fireEvent.click(screen.getByTestId('preview-style-current'));
    fireEvent.click(screen.getByTestId('preview-style-opt-ieee'));
    await waitFor(() => expect(screen.getByTestId('preview').textContent).not.toBe(apa));
    // IEEE puts the year at the end; APA puts it near the front in parens
    expect(screen.getByTestId('preview').textContent).toMatch(/2022\.$/);
  });

  it('formatCitation renders distinct output per style (unit)', () => {
    const csl = seed().csl;
    const styles = ['apa', 'mla', 'vancouver', 'ieee', 'nature'].map((s) => formatCitation(csl, s));
    expect(new Set(styles).size).toBe(styles.length); // all distinct
  });

  it('bulk reformat changes the style applied across the library + preview', async () => {
    renderCM({ initialCitations: [seed()] });
    await screen.findByTestId('detail');
    fireEvent.click(screen.getByTestId('bulk-style-current'));
    fireEvent.click(screen.getByTestId('bulk-style-opt-vancouver'));
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

/* ----------- import enrichment offer (Set 2b-iv, fill-only) -------------- */
// A DOI-exact dup whose incoming entry is RICHER than the existing one is no
// longer silently dropped — it's OFFERED as a fill-only enrichment. Nothing
// changes until the user clicks Fill; a populated field is never overwritten.
describe('import — enrichment offer (fill-only, never silent)', () => {
  const bibFile = (text: string) => new File([text], 'lib.bib', { type: 'text/plain' });
  // a THIN existing entry: title/author/year/DOI, but NO journal/volume/pages
  const thin = (): Citation => ({
    id: 'thin',
    csl: { id: 's', type: 'article-journal', title: 'Thin Paper', author: [{ family: 'Doe', given: 'Jane' }], issued: { year: 2020 }, DOI: '10.1/e' },
    doi: '10.1/e',
    retracted: false,
    source: 'imported',
  });
  // same DOI, richer: adds journal + volume + pages
  const RICH = `@article{e, title={Thin Paper}, author={Doe, Jane}, journal={J. Enrich}, volume={5}, pages={10--20}, year={2020}, doi={10.1/e}}`;

  const importRich = async () => {
    const lib = makeMockLocalLibrary();
    renderCM({ localLibrary: lib, initialCitations: [thin()] });
    await screen.findByTestId('citation-manager');
    fireEvent.change(screen.getByTestId('import-input'), { target: { files: [bibFile(RICH)] } });
    await screen.findByTestId('import-report');
    return lib;
  };

  it('never silent: shows "can be enriched" in the tally + a prominent panel, and changes NOTHING yet', async () => {
    const lib = await importRich();
    expect(screen.getByTestId('import-tally').textContent).toMatch(/can be enriched/i);
    expect(screen.getByTestId('enrich-panel')).toBeTruthy();
    expect(screen.getByTestId('enrich-missing').textContent).toMatch(/journal/i);
    // default = no change: the existing row was never written
    expect(lib.rows.get('thin')).toBeUndefined();
  });

  it('Fill fills the gaps in place (same id), keeps the title, and dismisses the offer', async () => {
    const lib = await importRich();
    fireEvent.click(screen.getByTestId('enrich-fill'));
    await waitFor(() => expect(screen.queryByTestId('enrich-panel')).toBeNull());
    const row = lib.rows.get('thin'); // persisted under the SAME id
    expect(row).toBeTruthy();
    expect(row?.csl_json).toMatch(/J\. Enrich/); // journal filled from the import
    expect(row?.csl_json).toMatch(/Thin Paper/); // existing title untouched
  });

  it('Keep as-is leaves the existing entry exactly as-is (no write)', async () => {
    const lib = await importRich();
    fireEvent.click(screen.getByTestId('enrich-keep'));
    await waitFor(() => expect(screen.queryByTestId('enrich-panel')).toBeNull());
    expect(lib.rows.get('thin')).toBeUndefined(); // never written — left exactly as-is
  });
});

/* ------------------ import fuzzy review panel (Set 2b-ii) ---------------- */
describe('import — fuzzy review panel', () => {
  // A library entry with a title+year and NO DOI → an import of the same
  // title+year (also no DOI) is a Tier-2 fuzzy match (not Tier-1).
  const libEntry = (n: number): Citation => ({
    id: `lib-${n}`,
    csl: { id: `lib-${n}`, type: 'article-journal', title: `Review Paper ${n}`, author: [{ family: 'Doe', given: 'J' }], issued: { year: 2020 } },
    doi: null,
    retracted: false,
    source: 'manual',
  });
  const bibEntry = (n: number) => `@article{k${n}, title={Review Paper ${n}}, author={Roe, Ann}, year={2020}}`;
  const importN = async (n: number, extra = '') => {
    const lib = Array.from({ length: n }, (_, i) => libEntry(i + 1));
    renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: lib });
    await screen.findByTestId('citation-manager');
    const bib = Array.from({ length: n }, (_, i) => bibEntry(i + 1)).join('\n') + extra;
    fireEvent.change(screen.getByTestId('import-input'), { target: { files: [new File([bib], 'lib.bib')] } });
    await screen.findByTestId('import-report');
  };
  const inList = (t: string) => within(screen.getByTestId('citation-list')).getAllByText(t);

  it('renders 8 fuzzy matches side-by-side (existing vs imported)', async () => {
    await importN(8);
    expect(screen.getByTestId('review-panel')).toBeTruthy();
    const items = screen.getAllByTestId('review-item');
    expect(items.length).toBe(8);
    expect(within(items[0]).getByText('In your library')).toBeTruthy();
    expect(within(items[0]).getByText('Imported')).toBeTruthy();
    expect(within(items[0]).getAllByText('Review Paper 1').length).toBe(2); // both sides
  });

  it('80 fuzzy matches → the bulk path: "Skip all" removes all kept-both dups at once', async () => {
    await importN(80);
    expect(screen.getByTestId('review-count').textContent).toMatch(/80 to review/);
    // each title is present twice pre-skip (library original + kept-both import)
    expect(inList('Review Paper 1').length).toBe(2);
    fireEvent.click(screen.getByTestId('review-skip-all'));
    await waitFor(() => expect(screen.queryByTestId('review-panel')).toBeNull());
    // the imported copies are gone; the library originals remain (never touched)
    expect(inList('Review Paper 1').length).toBe(1);
    expect(inList('Review Paper 80').length).toBe(1);
  });

  it('per-item Skip removes ONLY that entry; the rest stay', async () => {
    await importN(8);
    const first = screen.getAllByTestId('review-item')[0];
    fireEvent.click(within(first).getByTestId('review-skip'));
    await waitFor(() => expect(screen.getAllByTestId('review-item').length).toBe(7));
    expect(inList('Review Paper 1').length).toBe(1); // this one's import copy removed
    expect(inList('Review Paper 2').length).toBe(2); // untouched — still kept-both
  });

  it('Done without deciding keeps everything (default keep-both, not a gate)', async () => {
    await importN(8);
    fireEvent.click(screen.getByTestId('review-done'));
    await waitFor(() => expect(screen.queryByTestId('review-panel')).toBeNull());
    // every import copy survived — each title still appears twice
    expect(inList('Review Paper 1').length).toBe(2);
    expect(inList('Review Paper 8').length).toBe(2);
  });

  it('the panel never blocks the import: non-fuzzy adds are applied immediately', async () => {
    // 8 fuzzy + 1 brand-new entry that has no match at all
    await importN(8, '\n@article{fresh, title={A Fresh Unique Paper}, author={New, A}, year={2024}, doi={10.1/fresh}}');
    // the fresh one is in the library even though the review panel is open
    expect(screen.getByTestId('review-panel')).toBeTruthy();
    expect(inList('A Fresh Unique Paper').length).toBe(1);
  });
});

/* ------------------ import online verify pass (Set 2b-iii) --------------- */
describe('import — gated online verify', () => {
  const doiEntry = (n: number, doi: string) =>
    `@article{k${n}, title={Paper ${n}}, author={Doe, Jane}, year={20${20 + n}}, doi={${doi}}}`;
  const foundFor = (doi?: string): ReferenceVerification => ({ ...okVerify, exists: { ...okVerify.exists!, doi: doi ?? '10.1/x' } });
  // resolver returns UNVERIFIED so the enrich step is skipped (keeps titles stable);
  // the badge still flips via rv.verify's CrossRef provenance.
  const noEnrich = () => makeMockResolve(() => ({ status: 'unverified', reason: 'skip', unverified_title_hint: null }));
  const importDois = async (bridge: any) => {
    const bib = `${doiEntry(1, '10.1/p1')}\n${doiEntry(2, '10.1/p2')}\n${doiEntry(3, '10.1/p3')}`;
    renderCM(bridge);
    await screen.findByTestId('citation-manager');
    fireEvent.change(screen.getByTestId('import-input'), { target: { files: [new File([bib], 'l.bib')] } });
    await screen.findByTestId('import-report');
  };

  it('consent OFF → verify button disabled, honest note, zero fetches', async () => {
    setCloudConsent('citation_verification', false);
    let calls = 0;
    const rv = { async verify() { calls += 1; return okVerify; } };
    await importDois({ refverify: rv, metadataResolver: noEnrich(), localLibrary: makeMockLocalLibrary() });

    expect((screen.getByTestId('verify-imported') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('verify-off-note').textContent).toMatch(/turned off in Settings/i);
    expect(calls).toBe(0);
    setCloudConsent('citation_verification', true);
  });

  it('consent ON → verifies and flips the badge from unverified to verified', async () => {
    setCloudConsent('citation_verification', true);
    let calls = 0;
    const rv = { async verify(ref: any) { calls += 1; return foundFor(ref.doi); } };
    await importDois({ refverify: rv, metadataResolver: noEnrich(), localLibrary: makeMockLocalLibrary() });

    expect(screen.getAllByTestId('unverified-badge').length).toBe(3);
    fireEvent.click(screen.getByTestId('verify-imported'));
    await waitFor(() => expect(screen.queryByTestId('unverified-badge')).toBeNull());
    expect(calls).toBe(3);
  });

  it('not-found renders DISTINCTLY from a network-failure', async () => {
    setCloudConsent('citation_verification', true);
    const rv = {
      async verify(ref: any) {
        if (ref.doi === '10.1/fail') throw new Error('network down');
        return { ...okVerify, exists: { found: false, source: 'crossref', doi: ref.doi, title: null, is_retracted_hint: false } };
      },
    };
    const bib = `${doiEntry(1, '10.1/nf')}\n${doiEntry(2, '10.1/fail')}`;
    renderCM({ refverify: rv, metadataResolver: noEnrich(), localLibrary: makeMockLocalLibrary() });
    await screen.findByTestId('citation-manager');
    fireEvent.change(screen.getByTestId('import-input'), { target: { files: [new File([bib], 'l.bib')] } });
    await screen.findByTestId('import-report');

    fireEvent.click(screen.getByTestId('verify-imported'));
    await waitFor(() => expect(screen.getByTestId('badge-not-found')).toBeTruthy());
    expect(screen.getByTestId('badge-not-found').textContent).toMatch(/not found on CrossRef/i);
    expect(screen.getByTestId('badge-check-failed').textContent).toMatch(/check failed/i);
  });

  it('cancel mid-pass: verified entries KEEP their state; unreached ones untouched', async () => {
    setCloudConsent('citation_verification', true);
    const gates: Array<() => void> = [];
    let calls = 0;
    const rv = {
      async verify(ref: any) {
        await new Promise<void>((r) => gates.push(r)); // wait for the test to release
        calls += 1;
        return foundFor(ref.doi);
      },
    };
    await importDois({ refverify: rv, metadataResolver: noEnrich(), localLibrary: makeMockLocalLibrary() });
    expect(screen.getAllByTestId('unverified-badge').length).toBe(3);

    fireEvent.click(screen.getByTestId('verify-imported'));
    await waitFor(() => expect(gates.length).toBe(1)); // entry 1 in-flight
    gates[0]();                                        // release entry 1
    await waitFor(() => expect(gates.length).toBe(2)); // entry 2 in-flight
    fireEvent.click(screen.getByTestId('verify-cancel')); // cancel while entry 2 is in-flight
    gates[1]();                                        // entry 2 completes; loop then breaks before entry 3

    await waitFor(() => expect(screen.queryByTestId('verify-progress')).toBeNull()); // pass ended
    expect(calls).toBe(2);                              // entry 3 was never verified
    // entries 1+2 verified (kept), entry 3 still unverified — partial verification is VALID
    expect(screen.getAllByTestId('unverified-badge').length).toBe(1);
  });
});

/* -------- verification label — the two-axis honesty fix (Scope A) -------- */
// Completeness ("has all fields") and verification ("confirmed against
// CrossRef") are DIFFERENT facts and must never share a word. computeStatus's
// 'ok' now reads "complete", NOT "verified"; "verified · CrossRef" means
// verified-against-the-world only. The badge is source-agnostic — a hand-typed
// entry can never look verified.
describe('verification label — completeness vs verification (never conflated)', () => {
  it('a complete hand-typed entry reads "complete · not verified online", never "verified"', async () => {
    // seed() is a fully-complete manual entry (all fields) with NO provenance.
    renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: [seed({ id: 'm1', source: 'manual' })] });
    await screen.findByTestId('citation-manager');

    // list badge: "not verified online" — and NO "verified" chip
    expect(screen.getByTestId('unverified-badge').textContent).toMatch(/not verified online/i);
    expect(screen.queryByTestId('verified-badge')).toBeNull();

    // detail panel splits the two axes explicitly
    fireEvent.click(screen.getByTestId('cite-m1'));
    expect(screen.getByTestId('detail-metadata').textContent).toMatch(/Metadata:\s*complete/i);
    expect(screen.getByTestId('detail-verification').textContent).toMatch(/Verification:\s*not verified online/i);
    // the dangerous conflation is gone: metadata-complete is NOT labeled "verified"
    expect(screen.getByTestId('detail-metadata').textContent).not.toMatch(/verified/i);
  });

  it('a CrossRef-verified entry reads "verified · CrossRef" (existence confirmed)', async () => {
    const verifiedEntry = seed({ id: 'v1', source: 'imported', provenance: ['crossref:https://api.crossref.org/works/10.1/seed'] });
    renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: [verifiedEntry] });
    await screen.findByTestId('citation-manager');

    expect(screen.getByTestId('verified-badge').textContent).toMatch(/verified · CrossRef/i);
    expect(screen.queryByTestId('unverified-badge')).toBeNull();

    fireEvent.click(screen.getByTestId('cite-v1'));
    expect(screen.getByTestId('detail-verification').textContent).toMatch(/Verification:\s*verified · CrossRef/i);
  });

  it('the "not verified online" badge shows for EVERY source, not just imported', async () => {
    const mk = (id: string, source: Citation['source']) => seed({ id, source });
    renderCM({
      localLibrary: makeMockLocalLibrary(),
      initialCitations: [mk('c-manual', 'manual'), mk('c-extracted', 'extracted'), mk('c-doi', 'doi')],
    });
    await screen.findByTestId('citation-manager');
    // all three unverified sources carry the badge — the old code showed it for 'imported' only
    expect(screen.getAllByTestId('unverified-badge').length).toBe(3);
  });

  it('not_found and check_failed each render their own distinct state', async () => {
    renderCM({
      localLibrary: makeMockLocalLibrary(),
      initialCitations: [
        seed({ id: 'nf', source: 'imported', verifyOutcome: 'not_found' }),
        seed({ id: 'cf', source: 'imported', verifyOutcome: 'check_failed' }),
      ],
    });
    await screen.findByTestId('citation-manager');
    expect(screen.getByTestId('badge-not-found').textContent).toMatch(/not found on CrossRef/i);
    expect(screen.getByTestId('badge-check-failed').textContent).toMatch(/check failed/i);
    expect(screen.queryByTestId('verified-badge')).toBeNull();
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

/* ---------- Set 2c-i: the citeproc engine goes LIVE in the app ----------- */
// Proof that wiring prepareStyle actually routes the preview through citeproc
// (not the legacy hand-rolled formatter). We serve the bundled .csl from disk
// via a fetch mock — the app's own-origin fetch is unavailable in jsdom, which
// is exactly why the legacy fallback holds in every other test. LAST in the
// file so its style registrations can't leak into earlier legacy-path tests.
import { readFileSync as _readFileSync } from 'fs';
import { join as _join } from 'path';

describe('CSL engine goes live (Set 2c-i)', () => {
  const rich = (): Citation => ({
    id: 'rich',
    csl: {
      id: 'rich',
      type: 'article-journal',
      title: 'Deep learning approaches to protein folding prediction',
      author: [
        { family: 'Smith', given: 'John A' },
        { family: 'Doe', given: 'Jane B' },
        { family: 'Nguyen', given: 'Anh' },
      ],
      issued: { year: 2020 },
      containerTitle: 'Journal of Computational Biology',
      volume: '27',
      issue: '4',
      page: '523-541',
      DOI: '10.1089/cmb.2019.0420',
    },
    doi: '10.1089/cmb.2019.0420',
    retracted: false,
    source: 'manual',
  });

  it('a style prepares → the preview routes through citeproc, not legacy', async () => {
    const realFetch = global.fetch;
    global.fetch = vi.fn(async (url: any) => {
      const s = String(url);
      const style = s.match(/\/csl\/styles\/(.+)\.csl$/);
      if (style) return { ok: true, text: async () => _readFileSync(_join(process.cwd(), 'public', 'csl', 'styles', `${style[1]}.csl`), 'utf-8') } as any;
      return { ok: false, text: async () => '' } as any;
    }) as any;
    try {
      renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: [rich()] });
      await screen.findByTestId('citation-manager');
      // starts on the legacy formatter, then flips once apa.csl loads
      await waitFor(() => expect(screen.getByTestId('preview').getAttribute('data-style-source')).toBe('citeproc'));
      // citeproc-specific output: en-dash page range (legacy used a hyphen)
      expect(screen.getByTestId('preview').textContent).toMatch(/523–541/);
      expect(screen.getByTestId('preview').textContent).not.toMatch(/523-541/);
      // the DOI the LEGACY APA formatter kept, still present…
      expect(screen.getByTestId('preview').textContent).toMatch(/10\.1089\/cmb\.2019\.0420/);
      // …and italics are preserved (html→markers → the Formatted <em>), not dropped
      const em = screen.getByTestId('preview').querySelector('em, i');
      expect(em?.textContent).toMatch(/Journal of Computational Biology/);
    } finally {
      global.fetch = realFetch;
    }
  });

  it('a numbered style keeps its leading number, and the DOI the legacy formatter DROPPED now appears', async () => {
    const realFetch = global.fetch;
    global.fetch = vi.fn(async (url: any) => {
      const style = String(url).match(/\/csl\/styles\/(.+)\.csl$/);
      if (style) return { ok: true, text: async () => _readFileSync(_join(process.cwd(), 'public', 'csl', 'styles', `${style[1]}.csl`), 'utf-8') } as any;
      return { ok: false, text: async () => '' } as any;
    }) as any;
    try {
      renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: [rich()] });
      await screen.findByTestId('citation-manager');
      fireEvent.click(await screen.findByTestId('preview-style-current'));
      fireEvent.click(screen.getByTestId('preview-style-opt-ieee'));
      await waitFor(() => expect(screen.getByTestId('preview').getAttribute('data-style-source')).toBe('citeproc'));
      const text = screen.getByTestId('preview').textContent ?? '';
      expect(text).toMatch(/^\[1\]/); // IEEE numbered — honest: it IS entry 1
      expect(text).toMatch(/doi: 10\.1089\/cmb\.2019\.0420/); // legacy IEEE OMITTED this
    } finally {
      global.fetch = realFetch;
    }
  });
});

/* -------- Set 2c-ii: the searchable style picker over all 2,856 ---------- */
describe('style picker (Set 2c-ii)', () => {
  // Serve the REAL manifest + .csl from disk; optionally fail one style id.
  const mockCslFetch = (failStyle?: string) => {
    global.fetch = vi.fn(async (url: any) => {
      const s = String(url);
      if (/\/csl\/manifest\.json$/.test(s)) {
        return { ok: true, json: async () => JSON.parse(_readFileSync(_join(process.cwd(), 'public', 'csl', 'manifest.json'), 'utf-8')) } as any;
      }
      const style = s.match(/\/csl\/styles\/(.+)\.csl$/);
      if (style) {
        if (failStyle && style[1] === failStyle) return { ok: false, text: async () => '' } as any;
        return { ok: true, text: async () => _readFileSync(_join(process.cwd(), 'public', 'csl', 'styles', `${style[1]}.csl`), 'utf-8') } as any;
      }
      return { ok: false, text: async () => '' } as any;
    }) as any;
  };
  const richCite = (): Citation => ({
    id: 'r', csl: { id: 'r', type: 'article-journal', title: 'A Study', author: [{ family: 'Ng', given: 'A' }], issued: { year: 2020 }, containerTitle: 'Cell', volume: '1', issue: '1', page: '1-9', DOI: '10.1/x' },
    doi: '10.1/x', retracted: false, source: 'manual',
  });

  it('the pinned 8 render without searching', async () => {
    renderCM({ initialCitations: [richCite()] });
    await screen.findByTestId('detail');
    fireEvent.click(screen.getByTestId('preview-style-current'));
    // all 8 pinned present; a catalog-only style is NOT (no search yet)
    ['apa', 'mla', 'chicago-author-date', 'vancouver', 'harvard', 'ieee', 'nature', 'ama'].forEach((id) =>
      expect(screen.getByTestId(`preview-style-opt-${id}`)).toBeTruthy(),
    );
    expect(screen.queryByTestId('preview-style-opt-cell')).toBeNull();
  });

  it('search finds a style beyond the 8, and selecting it prepares citeproc', async () => {
    const realFetch = global.fetch;
    mockCslFetch();
    try {
      renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: [richCite()] });
      await screen.findByTestId('detail');
      fireEvent.click(screen.getByTestId('preview-style-current'));
      fireEvent.change(screen.getByTestId('preview-style-search'), { target: { value: 'cell' } });
      // "Cell" (id cell) is not one of the pinned 8 but is findable
      await screen.findByTestId('preview-style-opt-cell');
      fireEvent.click(screen.getByTestId('preview-style-opt-cell'));
      // it prepares and the preview goes citeproc for that catalog style
      await waitFor(() => expect(screen.getByTestId('preview').getAttribute('data-style-source')).toBe('citeproc'));
    } finally {
      global.fetch = realFetch;
    }
  });

  it('a search matching hundreds renders the CAP, not all of them', async () => {
    const realFetch = global.fetch;
    mockCslFetch();
    try {
      renderCM({ initialCitations: [richCite()] });
      await screen.findByTestId('detail');
      fireEvent.click(screen.getByTestId('preview-style-current'));
      // wait for the catalog to load, then a broad query
      fireEvent.change(screen.getByTestId('preview-style-search'), { target: { value: 'journal' } });
      await waitFor(() => {
        const items = within(screen.getByTestId('preview-style-results')).getAllByRole('button');
        expect(items.length).toBeGreaterThan(0);
        expect(items.length).toBeLessThanOrEqual(50); // MAX_STYLE_RESULTS — 629 match, ≤50 render
      });
    } finally {
      global.fetch = realFetch;
    }
  });

  it('a style whose .csl is missing shows the honest error, never a wrong render', async () => {
    const realFetch = global.fetch;
    // 'bmj' is used ONLY here (module-global readyStyles persists across tests,
    // so a style another test registered would already be ready) and is a
    // narrow search term so the exact id lands within the 50-result cap.
    mockCslFetch('bmj'); // manifest ok, but bmj.csl fails
    try {
      renderCM({ localLibrary: makeMockLocalLibrary(), initialCitations: [richCite()] });
      await screen.findByTestId('detail');
      fireEvent.click(screen.getByTestId('preview-style-current'));
      fireEvent.change(screen.getByTestId('preview-style-search'), { target: { value: 'bmj' } });
      await screen.findByTestId('preview-style-opt-bmj');
      fireEvent.click(screen.getByTestId('preview-style-opt-bmj'));
      await screen.findByTestId('preview-error');
      // honest error, and NOT a silently-wrong formatted render
      expect(screen.getByTestId('preview-error').textContent).toMatch(/isn.t in the bundled set/i);
      expect(screen.getByTestId('preview').getAttribute('data-style-source')).not.toBe('citeproc');
    } finally {
      global.fetch = realFetch;
    }
  });
});
