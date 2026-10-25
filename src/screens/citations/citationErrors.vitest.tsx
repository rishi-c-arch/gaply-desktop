// Decision 231: the Citation Manager's raw error messages, each now one closed
// sentence on screen, and its console lines a closed reason code. Each site is
// made to fail through its bridge, or through a vi.mock of the module it
// calls, once with an Error and once with a plain string (Tauri rejects with
// the error's string), and must show its sentence and no part of the error.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));
// The AI panel, reduced to the one control that runs the page's own
// open-access fetch for the selected source (CitationManagerPage.tsx:1872).
vi.mock('../ai/CitationAiPanel', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../ai/CitationAiPanel')>()),
  CitationAiPanel: ({ onFixSource }: { onFixSource?: () => void }) => (
    <button data-testid="stub-fix-source" onClick={() => onFixSource?.()}>fix</button>
  ),
}));

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CitationManagerPage, {
  CITATION_LOG,
  CHECK_DOI_FAILED,
  IMPORT_FAILED,
  IMPORT_TOO_LARGE,
  IMPORT_UNSUPPORTED_TYPE,
  OPEN_ACCESS_FAILED,
  RESOLVE_PAPER_FAILED,
  RETRACTIONS_FAILED,
} from './CitationManagerPage';
import { makeMockLocalLibrary } from './localLibrary';
import { makeMockResolve } from './metadataBridge';
import { makeMockRefVerify, ReferenceVerification } from './refverifyBridge';
import { Citation } from './citationTypes';
import { detectFormat } from './importCitations';
import { setCloudConsent } from '../settings/settingsStore';
import { aiBridge } from '../ai/aiReady';

const SECRET = 'SENTINEL_CITE_ERR /Users/someone/paper.pdf https://example.org/x';
const FAILURES: Array<[string, () => unknown]> = [
  ['an Error', () => new Error(SECRET)],
  ['a string', () => SECRET],
];

function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: false }),
    onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
  } as any;
}

const lib = () => ({ add: vi.fn(), list: vi.fn(), remove: vi.fn() }) as any;

function renderCM(props: any = {}) {
  const local = props.localLibrary ?? makeMockLocalLibrary();
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth()}>
        <CitationManagerPage {...props} localLibrary={local} citationService={lib()} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
  return local;
}

const seed = (id: string): Citation => ({
  id,
  csl: { id, type: 'article-journal', title: 'Seed Paper', author: [{ family: 'Doe', given: 'Jane' }], issued: { year: 2022 }, DOI: `10.1/${id}` },
  doi: `10.1/${id}`,
  retracted: false,
  source: 'manual',
});

const okVerify: ReferenceVerification = {
  reference_raw: 'x',
  exists: { found: true, source: 'crossref', doi: '10.1/abc', title: { safe_text: 'A Verified Paper', suspicious: false }, is_retracted_hint: false },
  retraction: { retracted: false, reasons: [], notice_url: null },
  enrichment: { citation_count: 1, abstract_text: null, venue: null },
  open_access: null,
  provenance: [],
  warnings: [],
};

const noSecret = () => {
  const text = document.body.textContent ?? '';
  expect(text).not.toContain('SENTINEL_CITE_ERR');
  expect(text).not.toContain('/Users/');
  expect(text).not.toContain('example.org');
};

/** Every console argument of every call, as text. */
const logged = (spy: ReturnType<typeof vi.spyOn>) =>
  spy.mock.calls.map((args: unknown[]) => args.map((a) => (typeof a === 'string' ? a : JSON.stringify(a) ?? String(a))).join(' '));

let warn: ReturnType<typeof vi.spyOn>;
let error: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  setCloudConsent('citation_verification', true);
  warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  error = vi.spyOn(console, 'error').mockImplementation(() => {});
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const consoleHasNoSecret = () => {
  for (const line of [...logged(warn), ...logged(error)]) {
    expect(line).not.toContain('SENTINEL_CITE_ERR');
    expect(line).not.toContain('/Users/');
  }
};

describe('on screen', () => {
  it('the sentences are the decision\'s', () => {
    expect(RESOLVE_PAPER_FAILED).toBe('Gaply could not look up this paper from its file. Try again later, or add it by its DOI.');
    expect(CHECK_DOI_FAILED).toBe('Gaply could not check this DOI just now. Check that it is typed correctly, or try again later.');
    expect(IMPORT_FAILED).toBe('Gaply could not import this file. Nothing was added to your library.');
    expect(IMPORT_TOO_LARGE).toBe('This file is too large to import (the limit is 16 MB). Split it and import the parts one at a time.');
    expect(IMPORT_UNSUPPORTED_TYPE).toBe('Gaply imports .bib, .ris and .json files. Choose a file of one of these types.');
    expect(OPEN_ACCESS_FAILED).toBe('Gaply could not look for open-access copies just now. Try again later.');
    expect(RETRACTIONS_FAILED).toBe('Gaply could not re-check retractions just now. Try again later.');
  });

  it.each(FAILURES)('adding a paper from its file (:499), failing with %s: its sentence, and add by DOI is there', async (_, fail) => {
    renderCM({ metadataResolver: makeMockResolve(() => { throw fail(); }) });
    const input = await screen.findByTestId('add-file');
    fireEvent.change(input, { target: { files: [new File(['x'], 'paper.pdf')] } });
    expect(await screen.findByText(RESOLVE_PAPER_FAILED)).toBeTruthy();
    // "or add it by its DOI": the DOI box and its button are there and usable.
    expect((screen.getByTestId('add-doi') as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getByTestId('doi-input')).toBeTruthy();
    noSecret();
  });

  it.each(FAILURES)('checking a DOI (:446), failing with %s: its sentence, the DOI kept to check, and the console line\'s code', async (_, fail) => {
    renderCM({ refverify: { verify: () => Promise.reject(fail()) } });
    fireEvent.change(await screen.findByTestId('doi-input'), { target: { value: '10.1/typed' } });
    fireEvent.click(screen.getByTestId('add-doi'));
    expect(await screen.findByText(CHECK_DOI_FAILED)).toBeTruthy();
    // "Check that it is typed correctly": what was typed is still in the box.
    expect((screen.getByTestId('doi-input') as HTMLInputElement).value).toBe('10.1/typed');
    expect(logged(error)).toContain(`[citations] DOI verify failed: ${CITATION_LOG.doiVerify}`);
    noSecret();
    consoleHasNoSecret();
  });

  it.each(FAILURES)('importing a file (:582), failing with %s: its sentence, and nothing added', async (_, fail) => {
    const local = renderCM();
    const file = new File(['@article{x, title={y}}'], 'refs.bib');
    Object.defineProperty(file, 'text', { value: () => Promise.reject(fail()) });
    fireEvent.change(await screen.findByTestId('import-input'), { target: { files: [file] } });
    expect(await screen.findByText(IMPORT_FAILED)).toBeTruthy();
    expect(local.rows.size).toBe(0);
    expect(screen.queryByTestId('import-report')).toBeNull();
    noSecret();
  });

  // Rust's `read_import_file` answers these two (src-tauri/tests/import_file_codes.rs);
  // Tauri rejects with the serialised error, an Error carrying it, or its string.
  const coded = (code: string): Array<[string, () => unknown]> => {
    const message = JSON.stringify({ code });
    return [
      ['the wire object', () => ({ code: 'validation', message })],
      ['an Error', () => new Error(message)],
      ['a string', () => message],
    ];
  };
  it.each([
    ...coded('import.too_large').map(([how, f]) => [`import.too_large as ${how}`, f, IMPORT_TOO_LARGE] as const),
    ...coded('import.unsupported_type').map(([how, f]) => [`import.unsupported_type as ${how}`, f, IMPORT_UNSUPPORTED_TYPE] as const),
  ])('%s: its own sentence, and nothing added', async (_, fail, sentence) => {
    const local = renderCM();
    const file = new File(['x'], 'refs.bib');
    Object.defineProperty(file, 'text', { value: () => Promise.reject(fail()) });
    fireEvent.change(await screen.findByTestId('import-input'), { target: { files: [file] } });
    expect(await screen.findByText(sentence)).toBeTruthy();
    expect(screen.queryByText(IMPORT_FAILED)).toBeNull();
    expect(local.rows.size).toBe(0);
  });

  it('a file of another type is refused before it is read, with the same sentence', async () => {
    renderCM();
    fireEvent.change(await screen.findByTestId('import-input'), { target: { files: [new File(['x'], 'refs.txt')] } });
    expect(await screen.findByText(IMPORT_UNSUPPORTED_TYPE)).toBeTruthy();
    // The sentence names exactly the types the import reads.
    expect(['a.bib', 'a.ris', 'a.json'].map(detectFormat).every((f) => f !== null)).toBe(true);
  });

  it.each(FAILURES)('the open-access fetch for the library (:802), failing with %s: its sentence, and the button usable again', async (_, fail) => {
    vi.spyOn(aiBridge, 'fetchOpenAccess').mockImplementation(() => Promise.reject(fail()));
    renderCM({ initialCitations: [seed('s1')] });
    fireEvent.click(await screen.findByTestId('fetch-oa-batch'));
    expect(await screen.findByText(OPEN_ACCESS_FAILED)).toBeTruthy();
    await waitFor(() => expect((screen.getByTestId('fetch-oa-batch') as HTMLButtonElement).disabled).toBe(false));
    noSecret();
  });

  it.each(FAILURES)('the open-access fetch for one source (:1872), failing with %s: its sentence', async (_, fail) => {
    vi.spyOn(aiBridge, 'fetchOpenAccess').mockImplementation(() => Promise.reject(fail()));
    renderCM({ initialCitations: [seed('s1')] });
    fireEvent.click(await screen.findByTestId('stub-fix-source'));
    expect(await screen.findByText(OPEN_ACCESS_FAILED)).toBeTruthy();
    noSecret();
  });

  it.each(FAILURES)('re-checking retractions (:902), failing with %s: its sentence, the button usable again, and the console line\'s code', async (_, fail) => {
    // allSettled never rejects, so the failure is one only the sweep's own
    // reading of an answer can raise.
    const broken = { ...okVerify, get exists(): never { throw fail(); } } as unknown as ReferenceVerification;
    renderCM({ initialCitations: [seed('s1')], refverify: makeMockRefVerify(broken) });
    fireEvent.click(await screen.findByTestId('check-retractions'));
    expect(await screen.findByText(RETRACTIONS_FAILED)).toBeTruthy();
    await waitFor(() => expect((screen.getByTestId('check-retractions') as HTMLButtonElement).disabled).toBe(false));
    expect(logged(error)).toContain(`[citations] retraction sweep failed: ${CITATION_LOG.retractionSweep}`);
    noSecret();
    consoleHasNoSecret();
  });
});

describe('in the console (:233, :344, :370, :434)', () => {
  const failingLocal = (fail: () => unknown, method: 'list' | 'upsert' | 'setTags') => {
    const local = makeMockLocalLibrary();
    (local as any)[method] = () => Promise.reject(fail());
    return local;
  };

  it.each(FAILURES)('the library failing to load, with %s: its tag and code', async (_, fail) => {
    renderCM({ localLibrary: failingLocal(fail, 'list') });
    await waitFor(() =>
      expect(logged(warn)).toContain(`[citations] local library unavailable (in-memory only): ${CITATION_LOG.localList}`)
    );
    consoleHasNoSecret();
  });

  it.each(FAILURES)('a local write failing, with %s: its tag and code', async (_, fail) => {
    renderCM({ localLibrary: failingLocal(fail, 'upsert') });
    fireEvent.click(await screen.findByTestId('add-manual'));
    await waitFor(() => expect(logged(warn)).toContain(`[citations] local write unavailable: ${CITATION_LOG.localWrite}`));
    consoleHasNoSecret();
  });

  it.each(FAILURES)('local tags failing, with %s: its tag and code', async (_, fail) => {
    renderCM({ localLibrary: failingLocal(fail, 'setTags'), initialCitations: [seed('s1')] });
    const input = await screen.findByTestId('cm-tag-input');
    fireEvent.change(input, { target: { value: 'mytag' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(logged(warn)).toContain(`[citations] local tags unavailable: ${CITATION_LOG.localTags}`));
    consoleHasNoSecret();
  });

  it.each(FAILURES)('the full-metadata enrich failing, with %s: its tag and code, and the DOI still added', async (_, fail) => {
    renderCM({ refverify: makeMockRefVerify(okVerify), metadataResolver: makeMockResolve(() => { throw fail(); }) });
    fireEvent.change(await screen.findByTestId('doi-input'), { target: { value: '10.1/abc' } });
    fireEvent.click(screen.getByTestId('add-doi'));
    await waitFor(() => expect(logged(warn)).toContain(`[citations] full-metadata enrich unavailable: ${CITATION_LOG.enrich}`));
    expect(await screen.findByText('A Verified Paper')).toBeTruthy();
    consoleHasNoSecret();
  });

  it('the six codes are closed and distinct', () => {
    const codes = Object.values(CITATION_LOG);
    expect(codes).toHaveLength(6);
    expect(new Set(codes).size).toBe(6);
    for (const c of codes) expect(c).toMatch(/^citations\.[a-z_]+$/);
  });
});
