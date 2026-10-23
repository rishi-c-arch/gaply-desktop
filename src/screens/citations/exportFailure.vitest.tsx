// A failed Citation Manager export shows one fixed sentence (Research OS,
// Phase 5B-4b; decisions 169 and 176): the bibliography, BibTeX and RIS. The
// error's own message could hold a path or a file name, so it reaches neither
// the screen nor the console. The bibliography had no catch at all: a failed
// write there showed nothing.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import { makeMockLocalLibrary } from './localLibrary';
import type { Citation } from './citationTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

const SENTINEL = 'HTSENTINEL_export_/Users/someone/HTPATH_q9/library.bib';
vi.mock('./exporters', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./exporters')>()),
  saveExportToFile: () => Promise.reject(new Error(SENTINEL)),
}));

import CitationManagerPage from './CitationManagerPage';

const SENTENCE = 'Gaply could not save the export. No file was written.';

function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: false }),
    onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
  } as any;
}

const seed = (id: string, title: string): Citation => ({
  id,
  csl: { id, type: 'article-journal', title, author: [{ family: 'Doe', given: 'J' }], issued: { year: 2020 } },
  doi: `10.1/${id}`,
  retracted: false,
  source: 'manual',
});

const consoles = ['log', 'info', 'warn', 'error', 'debug'] as const;
let spies: Array<ReturnType<typeof vi.spyOn>> = [];
beforeEach(() => {
  spies = consoles.map((k) => vi.spyOn(console, k).mockImplementation(() => {}));
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function expectNoSentinel() {
  expect(document.body.innerHTML).not.toContain('HTSENTINEL');
  expect(document.body.innerHTML).not.toContain('HTPATH');
  for (const s of spies) {
    const said = s.mock.calls.map((c: unknown[]) => c.map((a: unknown) => (a instanceof Error ? `${a.message} ${a.stack}` : String(a))).join(' '));
    expect(said.join('\n')).not.toMatch(/HTSENTINEL|HTPATH/);
  }
}

describe('a failed Citation Manager export', () => {
  it.each([
    ['the bibliography', 'export-biblio'],
    ['BibTeX', 'export-bibtex'],
    ['RIS', 'export-ris'],
  ])('of %s shows the sentence and nothing of the error', async (_what, button) => {
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <CitationManagerPage
            localLibrary={makeMockLocalLibrary()}
            initialCitations={[seed('a', 'HTREF_Alpha')]}
            citationService={{ add: vi.fn(), list: vi.fn(), remove: vi.fn() } as any}
          />
        </GaplySessionProvider>
      </MemoryRouter>,
    );
    await screen.findByTestId('citation-list');
    fireEvent.click(screen.getByTestId(button));
    const viewport = await screen.findByTestId('gds-toast-viewport');
    await waitFor(() => expect(viewport.textContent).toContain(SENTENCE));
    expect(viewport.textContent).not.toMatch(/exported/i);
    expect(viewport.querySelector('.gds-toast--flagged')).not.toBeNull();
    expectNoSentinel();
  });
});
