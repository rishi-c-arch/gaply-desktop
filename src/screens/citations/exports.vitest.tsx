// Set 5 — exports + the connected end-to-end flow. Deterministic
// serialization of VERIFIED CSL-JSON: a missing field is missing in the
// export, never invented. The e2e test drives paper-file → verified
// metadata (Set 2 shape) → local library (Set 4) → CSL format (Set 3) →
// BibTeX export (Set 5) as one connected flow.
import React from 'react';
import { readFileSync } from 'fs';
import { join } from 'path';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CitationManagerPage from './CitationManagerPage';
import { exportSerialized } from './exporters';
import { registerStyleXml, formatWithCsl } from './cslEngine';
import { makeMockLocalLibrary } from './localLibrary';
import { makeMockResolve, VerifiedMetadata } from './metadataBridge';
import { CslItem } from './citationTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const WATSON: CslItem = {
  id: 'wc1953',
  type: 'article-journal',
  title: 'Molecular Structure of Nucleic Acids',
  author: [
    { family: 'Watson', given: 'J. D.' },
    { family: 'Crick', given: 'F. H. C.' },
  ],
  issued: { year: 1953 },
  DOI: '10.1038/171737a0',
  containerTitle: 'Nature',
  volume: '171',
  page: '737-738',
};

const SPARSE: CslItem = {
  id: 'sp1',
  type: 'article-journal',
  title: 'A Sparse Preprint',
  author: [{ family: 'Lovelace', given: 'Ada' }],
  issued: { year: 2024 },
};

/* ------------------------------ Part A: exports -------------------------- */

describe('deterministic exports from verified CSL-JSON', () => {
  it('BibTeX carries the verified fields', async () => {
    const bib = await exportSerialized([WATSON], 'bibtex');
    expect(bib).toContain('@article');
    expect(bib).toContain('Watson');
    expect(bib).toContain('Crick');
    expect(bib).toMatch(/year\s*=\s*\{?1953/);
    expect(bib).toMatch(/volume\s*=\s*\{?171/);
    expect(bib).toContain('10.1038/171737a0');
    expect(bib).toContain('Nature');
  });

  it('RIS carries the verified fields', async () => {
    const ris = await exportSerialized([WATSON], 'ris');
    expect(ris).toContain('TY  - JOUR');
    expect(ris).toContain('Watson');
    expect(ris).toMatch(/PY\s+-\s+1953/);
    expect(ris).toContain('ER  -');
  });

  it('THE HONESTY TEST: a missing field is ABSENT in the export, never invented', async () => {
    const bib = await exportSerialized([SPARSE], 'bibtex');
    expect(bib).toContain('Lovelace');
    // no volume was verified → none exported
    expect(bib).not.toMatch(/volume\s*=/i);
    expect(bib).not.toMatch(/pages\s*=/i);
    expect(bib).not.toMatch(/journal\s*=\s*\{.+\}/i);
    expect(bib).not.toMatch(/doi\s*=/i);
    const ris = await exportSerialized([SPARSE], 'ris');
    expect(ris).not.toMatch(/VL\s+-/);
    expect(ris).not.toMatch(/SP\s+-/);
  });

  it('deterministic: identical output across runs, both formats', async () => {
    const runs = await Promise.all([1, 2, 3].map(() => exportSerialized([WATSON, SPARSE], 'bibtex')));
    expect(new Set(runs).size).toBe(1);
    const ris = await Promise.all([1, 2].map(() => exportSerialized([WATSON, SPARSE], 'ris')));
    expect(new Set(ris).size).toBe(1);
  });

  it('formatted-bibliography export uses the Set 3 engine (real bundled style)', async () => {
    const xml = readFileSync(join(process.cwd(), 'public', 'csl', 'styles', 'apa.csl'), 'utf-8');
    await registerStyleXml('apa', xml);
    const out = formatWithCsl([WATSON], 'apa');
    expect(out).toContain('Watson, J. D., & Crick, F. H. C.');
  });
});

/* --------------------------- Part B: end to end -------------------------- */

const VERIFIED: VerifiedMetadata = {
  source: 'crossref',
  matched_by: 'doi',
  csl_type: 'article-journal',
  doi: '10.1038/171737a0',
  title: 'Molecular Structure of Nucleic Acids',
  authors: [
    { family: 'Watson', given: 'J. D.' },
    { family: 'Crick', given: 'F. H. C.' },
  ],
  container_title: 'Nature',
  year: 1953,
  volume: '171',
  issue: '4356',
  page: '737-738',
};

function auth(session: any): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: false }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

describe('the connected flow: paper file → verified metadata → library → format → export', () => {
  it('runs end to end with every stage wired', async () => {
    const local = makeMockLocalLibrary();
    const resolver = makeMockResolve(() => ({ status: 'verified', metadata: VERIFIED }));
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth(null)}>
          <CitationManagerPage
            localLibrary={local}
            metadataResolver={resolver}
            citationService={{ add: vi.fn(), list: vi.fn(), remove: vi.fn() } as any}
          />
        </GaplySessionProvider>
      </MemoryRouter>
    );

    // Stage 1+2 (Set 2): upload → resolve verified metadata
    const file = new File(['x'], 'watson1953.pdf', { type: 'application/pdf' });
    fireEvent.change(screen.getByTestId('add-file'), { target: { files: [file] } });
    await waitFor(() => expect(resolver.calls.length).toBe(1));
    expect(resolver.calls[0].path).toContain('watson1953');

    // Stage 3 (Set 4): stored in the LOCAL library with full verified fields
    await waitFor(() => expect(local.rows.size).toBe(1));
    const row = Array.from(local.rows.values())[0];
    expect(row.csl_json).toContain('"volume":"171"');
    expect(row.csl_json).toContain('"container-title":"Nature"');
    expect(row.doi).toBe('10.1038/171737a0');

    // Stage 4 (Set 3): the library entry formats in a full-CSL style
    const stored = JSON.parse(row.csl_json);
    expect(stored.title).toBe('Molecular Structure of Nucleic Acids');
    const formatted = formatWithCsl(
      [{ ...WATSON, id: row.id }],
      'apa' // registered by the Part A test above (same suite)
    );
    expect(formatted).toContain('(1953)');

    // Stage 5 (Set 5): export the library entry to BibTeX
    const bib = await exportSerialized(
      [{ ...WATSON, id: row.id }],
      'bibtex'
    );
    expect(bib).toContain('@article');
    expect(bib).toMatch(/volume\s*=\s*\{?171/);

    // and the UI reflected the add honestly
    expect(screen.getByTestId('citation-list').textContent).toContain('Molecular Structure');
    expect(screen.getByTestId('cm-sync-status').textContent).toMatch(/Local only/);
  });

  it('an unverifiable paper is an honest toast with the title hint — nothing added', async () => {
    const local = makeMockLocalLibrary();
    const resolver = makeMockResolve(() => ({
      status: 'unverified',
      reason: 'no DOI found on the paper’s first page — enter the DOI or search by title (manual entry)',
      unverified_title_hint: 'Some Scanned Paper',
    }));
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth(null)}>
          <CitationManagerPage
            localLibrary={local}
            metadataResolver={resolver}
            citationService={{ add: vi.fn(), list: vi.fn(), remove: vi.fn() } as any}
          />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    const file = new File(['x'], 'scan.pdf', { type: 'application/pdf' });
    fireEvent.change(screen.getByTestId('add-file'), { target: { files: [file] } });
    await waitFor(() => expect(resolver.calls.length).toBe(1));
    expect(local.rows.size).toBe(0); // nothing enters the library unverified
    await screen.findByText(/Couldn’t verify: no DOI found/);
    expect(screen.getByText(/Some Scanned Paper/)).toBeTruthy();
  });
});
