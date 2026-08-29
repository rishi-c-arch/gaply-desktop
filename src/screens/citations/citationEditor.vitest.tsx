// Gaply — Citation Manager Group 2: the metadata editor, BibTeX key
// de-collision, and the honest empty title.
//
// Additive: this covers behaviour that shipped untested. Nothing in the four
// files under test is modified by it.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CitationManagerPage from './CitationManagerPage';
import { CitationEditor } from './CitationEditor';
import { decollideBibtexKeys } from './exporters';
import { makeMockRefVerify, ReferenceVerification } from './refverifyBridge';
import { Citation } from './citationTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

/* ------------------- 1. BibTeX entry-key de-collision -------------------- */

describe('decollideBibtexKeys', () => {
  /**
   * The real failure: citation-js derives the key from author+year+title, so
   * two entries agreeing on all three emit the SAME `@article{key,` and BibTeX
   * either errors or silently drops one.
   */
  it('gives a second entry sharing author+year+title a -2 suffix', () => {
    const bib = [
      '@article{Doe2022Paper,\n  title = {Paper},\n}',
      '@article{Doe2022Paper,\n  title = {Paper},\n}',
    ].join('\n\n');
    const out = decollideBibtexKeys(bib);
    expect(out).toContain('@article{Doe2022Paper,');
    expect(out).toContain('@article{Doe2022Paper-2,');
    // the first occurrence keeps its generated key — a stable citation key
    // already in someone's .tex must not move
    expect(out.indexOf('@article{Doe2022Paper,')).toBeLessThan(
      out.indexOf('@article{Doe2022Paper-2,'),
    );
  });

  it('numbers a third and fourth collision without reusing a suffix', () => {
    const bib = Array(4).fill('@article{Untitled,\n}').join('\n\n');
    const out = decollideBibtexKeys(bib);
    for (const k of ['{Untitled,', '{Untitled-2,', '{Untitled-3,', '{Untitled-4,']) {
      expect(out).toContain(k);
    }
  });

  /** Re-exporting the same library must yield the same file. */
  it('is stable across a re-export', () => {
    const bib = ['@article{K,\n}', '@article{K,\n}', '@book{K,\n}'].join('\n\n');
    const once = decollideBibtexKeys(bib);
    // the guarantee is determinism from the SAME input, which is what
    // re-exporting an unchanged library produces
    expect(decollideBibtexKeys(bib)).toBe(once);
  });

  it('leaves a file with no collisions byte-identical', () => {
    const bib = '@article{A2020,\n}\n\n@book{B2021,\n}';
    expect(decollideBibtexKeys(bib)).toBe(bib);
  });

  it('does not touch entry bodies, only the key', () => {
    const bib = '@article{K,\n  title = {A, comma, inside},\n}\n\n@article{K,\n  title = {A, comma, inside},\n}';
    const out = decollideBibtexKeys(bib);
    expect(out).toContain('title = {A, comma, inside}');
    expect(out.match(/title = \{A, comma, inside\}/g)?.length).toBe(2);
  });
});

/* ------------------------- 2. the metadata editor ------------------------ */

const CITE: Citation = {
  id: 'c-1',
  csl: {
    id: 'c-1',
    type: 'article-journal',
    title: 'Soil invertebrates under intensification',
    author: [{ family: 'Smith', given: 'Jane' }],
    issued: { year: 2019 },
    containerTitle: 'Journal of Soil Biology',
  } as any,
  doi: '10.1/soil',
  retracted: false,
  source: 'manual',
};

describe('CitationEditor', () => {
  it('opens populated with the citation’s existing fields', () => {
    render(
      <CitationEditor open citation={CITE} style="apa" onCancel={() => {}} onSave={() => {}} />,
    );
    const title = screen.getByDisplayValue('Soil invertebrates under intensification');
    expect(title).toBeTruthy();
    expect(screen.getByDisplayValue('Smith')).toBeTruthy();
    expect(screen.getByDisplayValue('Jane')).toBeTruthy();
    expect(screen.getByDisplayValue('2019')).toBeTruthy();
  });

  it('renders nothing when closed', () => {
    const { container } = render(
      <CitationEditor open={false} citation={CITE} style="apa" onCancel={() => {}} onSave={() => {}} />,
    );
    expect(container.textContent).toBe('');
  });

  /** ONE write on Save, carrying the edited value — not a per-keystroke stream. */
  it('calls onSave exactly once, with the edited result', () => {
    const onSave = vi.fn();
    render(
      <CitationEditor open citation={CITE} style="apa" onCancel={() => {}} onSave={onSave} />,
    );
    const title = screen.getByDisplayValue('Soil invertebrates under intensification');
    fireEvent.change(title, { target: { value: 'A corrected title' } });
    // typing must not persist anything
    expect(onSave).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('editor-save'));
    expect(onSave).toHaveBeenCalledTimes(1);
    const next = onSave.mock.calls[0][0] as Citation;
    expect(next.csl.title).toBe('A corrected title');
    expect(next.id).toBe('c-1');
  });

  it('Cancel persists nothing', () => {
    const onSave = vi.fn();
    const onCancel = vi.fn();
    render(
      <CitationEditor open citation={CITE} style="apa" onCancel={onCancel} onSave={onSave} />,
    );
    fireEvent.change(screen.getByDisplayValue('Soil invertebrates under intensification'), {
      target: { value: 'Discard me' },
    });
    fireEvent.click(screen.getByTestId('editor-cancel'));
    expect(onSave).not.toHaveBeenCalled();
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});

/* ------------------ 3. a titleless verified DOI stays empty --------------- */

const USER = { user: { id: 'u1', email: 'a@b.c' } };
function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: USER, offline: false }),
    onAuthStateChange: (cb: any) => { cb(USER); return () => {}; },
  } as any;
}

/** Found, real, and carrying NO title — the case that used to fabricate one. */
const titlelessVerify: ReferenceVerification = {
  reference_raw: 'x',
  exists: { found: true, source: 'crossref', doi: '10.1234/foo', title: null, is_retracted_hint: false },
  retraction: { retracted: false, reasons: [], notice_url: null },
  enrichment: null,
  open_access: null,
  provenance: [{ source: 'crossref', url: 'https://api.crossref.org/works/10.1234/foo' }],
  warnings: [],
};

describe('a verified DOI with no title', () => {
  it('leaves the title EMPTY rather than substituting the DOI string', async () => {
    const lib = {
      add: vi.fn().mockResolvedValue({ data: {}, error: null, offline: false }),
      list: vi.fn(),
      remove: vi.fn(),
    } as any;
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <CitationManagerPage citationService={lib} refverify={makeMockRefVerify(titlelessVerify)} />
        </GaplySessionProvider>
      </MemoryRouter>,
    );
    fireEvent.change(await screen.findByTestId('doi-input'), { target: { value: '10.1234/foo' } });
    fireEvent.click(screen.getByTestId('add-doi'));

    await waitFor(() => expect(lib.add).toHaveBeenCalled());
    const row = lib.add.mock.calls[0][0];
    // THE assertion: a card titled "10.1234/foo" is a fabricated value dressed
    // as metadata, and computeStatus would read it as a present title.
    expect(row.title ?? '').not.toBe('10.1234/foo');
    expect(row.title ?? '').toBe('');
    // the DOI itself is still recorded, in the field that means DOI
    expect(row.doi).toBe('10.1234/foo');
  });
});
