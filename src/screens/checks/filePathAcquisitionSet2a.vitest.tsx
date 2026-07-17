// M6 file-path fix (Set 2A) — the DIALOG CONTRACT tests for the two remaining
// FREE/reachable sites the sweep confirmed broken: the Plagiarism library
// manager (single-file) and the Gap Finder base-paper picker (multi-file).
//
// Same regression the mock-File tests miss: in the Tauri webview an <input>'s
// File has NO usable .path, so the raw `(file as any).path ?? file.name` sent a
// bare filename the core can't open. Under isTauri the page must acquire an
// ABSOLUTE path from the dialog plugin and pass THAT to the bridge.
//
// We force the desktop branch by mocking isTauri (file-scoped) and mock the
// dialog to return a NASTY absolute path (spaces AND parentheses — the exact
// shape that broke in the smoke test), then assert the bridge receives it and
// never the basename.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

// Force the Tauri (desktop) branch for this whole file.
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

// Nasty absolute paths — spaces AND parentheses (the smoke-test failure shape).
const ABS = '/Users/rishi/Desktop/FINAL CH 4 (rewritten).docx';
const BASENAME = 'FINAL CH 4 (rewritten).docx';
const ABS2 = '/Users/rishi/Desktop/prior work (v2).pdf';

// The single-file picker returns ABS; the multi-file picker returns [ABS, ABS2].
const openMock = vi.fn(async (opts: { multiple?: boolean }) => (opts.multiple ? [ABS, ABS2] : ABS));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

import PlagiarismLibraryManager from './PlagiarismLibraryManager';
import { makeMockCheckBridge } from './checkBridge';
import type { LocalLibrary } from '../citations/localLibrary';
import GapFinderPage from '../gapfinder/GapFinderPage';
import { makeGapFinderMock } from '../gapfinder/gapfinderBridge';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';

afterEach(() => {
  cleanup();
  openMock.mockClear();
});

// A LocalLibrary stub so the manager's citation-list effect resolves offline.
const noCitations: LocalLibrary = { list: async () => [] } as any;

const SESSION = { user: { id: 'u1', email: 'a@b.c' }, access_token: 'jwt-gf' };
function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: SESSION, offline: false }),
    onAuthStateChange: (cb: any) => { cb(SESSION); return () => {}; },
  } as any;
}

describe('M6 Set 2A — the Tauri dialog supplies ABSOLUTE paths to the bridge', () => {
  it('Plagiarism library: Add a paper → dialog → add_to_plagiarism_library gets the absolute path', async () => {
    const calls: Array<[string, string]> = [];
    const bridge = makeMockCheckBridge({ library: [], onCall: (c, p) => calls.push([c, p]) });
    render(<PlagiarismLibraryManager bridge={bridge} citations={noCitations} />);

    fireEvent.click(screen.getByTestId('library-pick'));

    await waitFor(() =>
      expect(calls.some(([c, p]) => c === 'add_to_plagiarism_library' && p === ABS)).toBe(true),
    );
    expect(openMock).toHaveBeenCalledTimes(1);
    // and NEVER the bare filename that caused the silent failure
    expect(calls.some(([, p]) => p === BASENAME)).toBe(false);
    // single-file dialog: the right extensions, no directory
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({ multiple: false, directory: false }),
    );
  });

  it('Gap Finder: Choose files → dialog → buildCorpus gets an ARRAY of absolute paths (≤8)', async () => {
    const bridge = makeGapFinderMock();
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <GapFinderPage session="gf-test" forceEntitlement="entitled" bridge={bridge} />
        </GaplySessionProvider>
      </MemoryRouter>,
    );

    fireEvent.click(await screen.findByTestId('gf-pick'));
    // the picked filenames surface, proving the paths were accepted
    await waitFor(() => expect(screen.getByTestId('gf-inputs').textContent).toMatch(/FINAL CH 4 \(rewritten\)\.docx/));

    fireEvent.click(screen.getByTestId('gf-build-corpus'));
    await waitFor(() => expect(bridge.calls.some((c) => c.method === 'buildCorpus')).toBe(true));

    const corpusCall = bridge.calls.find((c) => c.method === 'buildCorpus')!;
    const paths = (corpusCall.input as { paths: string[] }).paths;
    expect(paths).toEqual([ABS, ABS2]); // absolute paths, in order — never basenames
    expect(paths).not.toContain(BASENAME);
    // multi-file dialog: multiple:true, no directory
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({ multiple: true, directory: false }),
    );
  });

  it('Gap Finder: the multi-picker is capped at 8 papers', async () => {
    // Dialog returns 10 absolute paths; only the first 8 reach the corpus.
    const ten = Array.from({ length: 10 }, (_, i) => `/Users/rishi/Desktop/paper (${i}).pdf`);
    openMock.mockImplementationOnce(async () => ten);
    const bridge = makeGapFinderMock();
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <GapFinderPage session="gf-test" forceEntitlement="entitled" bridge={bridge} />
        </GaplySessionProvider>
      </MemoryRouter>,
    );

    fireEvent.click(await screen.findByTestId('gf-pick'));
    await waitFor(() => expect(screen.getByTestId('gf-inputs')).toBeTruthy());
    fireEvent.click(screen.getByTestId('gf-build-corpus'));
    await waitFor(() => expect(bridge.calls.some((c) => c.method === 'buildCorpus')).toBe(true));

    const paths = (bridge.calls.find((c) => c.method === 'buildCorpus')!.input as { paths: string[] }).paths;
    expect(paths).toHaveLength(8);
    expect(paths).toEqual(ten.slice(0, 8));
  });
});
