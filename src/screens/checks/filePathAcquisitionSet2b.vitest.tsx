// M6 file-path fix (Set 2B) — DIALOG CONTRACT tests for the three PREMIUM/gated
// pages the sweep additionally caught: PublishReady, StatsVerifier (×2 sites —
// the data picker AND the optional manuscript picker), and CitationManager.
//
// Same bug: in the Tauri webview an <input>'s File has no usable .path, so the
// raw `(file as any).path ?? file.name` sent a bare filename the core can't
// open. Under isTauri each page must acquire an ABSOLUTE path from the dialog
// and pass THAT to the bridge/resolver.
//
// The dialog is mocked to return a NASTY absolute path (spaces AND parentheses).
// The picker's EXTENSION LIST is pinned per picker — critically, StatsVerifier's
// DATA picker must ask for csv/tsv/xlsx/xls/ods, NOT pdf/docx.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

// Force the Tauri (desktop) branch for this whole file.
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

// Nasty absolute paths — spaces AND parentheses (the smoke-test failure shape).
const ABS = '/Users/rishi/Desktop/FINAL CH 4 (rewritten).docx';
const BASENAME = 'FINAL CH 4 (rewritten).docx';
const ABS_CSV = '/Users/rishi/Desktop/study data (2024).csv';

// The dialog returns a path shaped by what was ASKED: the CSV picker gets a csv
// path, everything else the docx path. Also lets us pin the extension list.
const openMock = vi.fn(async (opts: { multiple?: boolean; filters?: Array<{ extensions: string[] }> }) => {
  const exts = opts.filters?.[0]?.extensions ?? [];
  return exts.includes('csv') ? ABS_CSV : ABS;
});
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

import PublishReadyPage from '../publishready/PublishReadyPage';
import type { PublishReadyBridge } from '../publishready/publishReadyBridge';
import StatsVerifierPage from '../statsverifier/StatsVerifierPage';
import { makeMockStatsVerifierBridge } from '../statsverifier/statsVerifierBridge';
import { PREVIEW_FIXTURE } from '../statsverifier/statsVerifierFixtures';
import CitationManagerPage from '../citations/CitationManagerPage';
import { makeMockLocalLibrary } from '../citations/localLibrary';
import { makeMockResolve, VerifiedMetadata } from '../citations/metadataBridge';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';

afterEach(() => {
  cleanup();
  openMock.mockClear();
});

function auth(session: any = { user: { id: 'u1', email: 'a@b.c' }, access_token: 'jwt' }): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: false }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}
function wrap(node: React.ReactElement, session?: any) {
  return render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>{node}</GaplySessionProvider>
    </MemoryRouter>,
  );
}

const VERIFIED: VerifiedMetadata = {
  doi: '10.1/x', title: 'A Paper', source: 'crossref', matched_by: 'doi',
} as any;

describe('M6 Set 2B — gated pages: the Tauri dialog supplies ABSOLUTE paths', () => {
  it('PublishReady (premium): pick → dialog → run gets the absolute manuscript path', async () => {
    let runPath: string | undefined;
    // Capture the path at run() and stop (no report render needed).
    const bridge: PublishReadyBridge = {
      ingestGuidelines: async () => ({ any_ingested: false, note: '' }),
      run: async (input: any) => { runPath = input.manuscriptPath; throw new Error('captured'); },
    } as any;

    wrap(<PublishReadyPage forceTier="premium" bridge={bridge} />);
    await screen.findByTestId('pr-entry');

    fireEvent.click(screen.getByTestId('pr-pick'));
    await screen.findByText(BASENAME); // accepted-file basename renders
    // single-file, pdf/docx, no directory
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({ multiple: false, directory: false, filters: [{ name: 'Manuscript', extensions: ['pdf', 'docx'] }] }),
    );

    fireEvent.change(screen.getByTestId('pr-journal-input'), { target: { value: 'lancet' } });
    fireEvent.click(await screen.findByTestId('pr-journal-The Lancet'));
    fireEvent.click(screen.getByTestId('pr-run'));

    await waitFor(() => expect(runPath).toBe(ABS));
    expect(runPath).not.toBe(BASENAME);
  });

  it('CitationManager: "From paper file…" → dialog → resolver.resolve gets the absolute path', async () => {
    const resolver = makeMockResolve(() => ({ status: 'verified', metadata: VERIFIED }));
    wrap(
      <CitationManagerPage
        localLibrary={makeMockLocalLibrary()}
        metadataResolver={resolver}
        citationService={{ add: vi.fn(), list: vi.fn(), remove: vi.fn() } as any}
      />,
    );

    // The label intercepts its click under isTauri and opens the native dialog.
    fireEvent.click(screen.getByTestId('add-file').closest('label')!);

    await waitFor(() => expect(resolver.calls.length).toBe(1));
    expect(resolver.calls[0].path).toBe(ABS);
    expect(resolver.calls[0].path).not.toBe(BASENAME);
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({ filters: [{ name: 'Paper', extensions: ['pdf', 'docx', 'txt'] }] }),
    );
  });

  it('StatsVerifier data picker (premium): dialog asks for csv/tsv/xlsx/xls/ods (NOT pdf/docx) → preview gets the CSV path', async () => {
    const bridge = makeMockStatsVerifierBridge({ preview: PREVIEW_FIXTURE });
    wrap(<StatsVerifierPage forceTier="premium" bridge={bridge} />);

    fireEvent.click(await screen.findByTestId('sv-data-pick'));

    await waitFor(() => expect(bridge.calls.some(([c]) => c === 'run_stats_preview')).toBe(true));
    const call = bridge.calls.find(([c]) => c === 'run_stats_preview')![1] as { path: string };
    expect(call.path).toBe(ABS_CSV);
    expect(call.path).not.toBe('study data (2024).csv');
    // the DATA picker's extensions — csv family, explicitly NOT pdf/docx
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({ filters: [{ name: 'Data', extensions: ['csv', 'tsv', 'xlsx', 'xls', 'ods'] }] }),
    );
    const dataExts = openMock.mock.calls[0][0].filters![0].extensions;
    expect(dataExts).not.toContain('pdf');
    expect(dataExts).not.toContain('docx');
  });

  it('StatsVerifier manuscript picker (premium): dialog asks pdf/docx → the absolute path rides to preview as manuscriptPath', async () => {
    const bridge = makeMockStatsVerifierBridge({ preview: PREVIEW_FIXTURE });
    wrap(<StatsVerifierPage forceTier="premium" bridge={bridge} />);

    // Attach the manuscript first (absolute path → state), then pick the data
    // file — the preview call carries the manuscriptPath.
    fireEvent.click(await screen.findByTestId('sv-manuscript-pick'));
    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({ filters: [{ name: 'Manuscript', extensions: ['pdf', 'docx'] }] }),
    );

    fireEvent.click(screen.getByTestId('sv-data-pick'));
    await waitFor(() => expect(bridge.calls.some(([c]) => c === 'run_stats_preview')).toBe(true));

    const call = bridge.calls.find(([c]) => c === 'run_stats_preview')![1] as { path: string; manuscriptPath?: string };
    expect(call.path).toBe(ABS_CSV);
    expect(call.manuscriptPath).toBe(ABS); // the manuscript's absolute path, not a basename
  });
});
