// Set 4 — the LOCAL-FIRST library. Local sqlite (mocked bridge with the Rust
// semantics) is the source of truth; Supabase is optional sync. These tests
// prove: fully-offline operation with zero sign-in, honest sync statuses,
// and that a failed sync never loses local data.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CitationManagerPage from './CitationManagerPage';
import { makeMockLocalLibrary, StoredReference } from './localLibrary';
import { Citation } from './citationTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const USER = { user: { id: 'u1', email: 'a@b.c' } };

function auth(session: any): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: false }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

function supabaseSpy(result: { error: string | null; offline?: boolean } = { error: null }) {
  return {
    add: vi.fn().mockResolvedValue({ data: {}, error: result.error, offline: result.offline ?? false }),
    list: vi.fn(),
    remove: vi.fn(),
  } as any;
}

const seed = (id: string, title: string, extra: Partial<Citation> = {}): Citation => ({
  id,
  csl: {
    id,
    type: 'article-journal',
    title,
    author: [{ family: 'Watson', given: 'J. D.' }],
    issued: { year: 1953 },
    DOI: `10.1/${id}`,
  },
  doi: `10.1/${id}`,
  retracted: false,
  source: 'manual',
  ...extra,
});

function renderPage(props: any = {}, session: any = null) {
  const local: ReturnType<typeof makeMockLocalLibrary> = props.localLibrary ?? makeMockLocalLibrary();
  const cloud = props.citationService ?? supabaseSpy();
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <CitationManagerPage {...props} localLibrary={local} citationService={cloud} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
  return { local, cloud };
}

/* --------------------------- fully offline core -------------------------- */

describe('local-first, fully offline (no session, no cloud)', () => {
  it('add → stored LOCALLY; the cloud service is never touched', async () => {
    const { local, cloud } = renderPage({}, null);
    fireEvent.click(screen.getByTestId('add-manual'));
    await waitFor(() => expect(local.rows.size).toBe(1));
    expect(cloud.add).not.toHaveBeenCalled();
    const row: StoredReference = Array.from(local.rows.values())[0];
    expect(row.sync_status).toBe('local_only');
    expect(screen.getByTestId('cm-sync-status').textContent).toMatch(/Local only — sign in/);
  });

  it('hydrates the library from local sqlite on load (source of truth)', async () => {
    const local = makeMockLocalLibrary();
    await local.upsert(seed('w1', 'Molecular Structure of Nucleic Acids'), ['dna']);
    renderPage({ localLibrary: local }, null);
    await screen.findByTestId('cite-w1');
    expect(screen.getByTestId('cite-w1').textContent).toContain('Molecular Structure');
  });

  it('search narrows by title/author/year/DOI/tag through the local bridge', async () => {
    const local = makeMockLocalLibrary();
    await local.upsert(seed('w1', 'Molecular Structure of Nucleic Acids'), ['dna']);
    await local.upsert(seed('s1', 'A Sparse Preprint'), []);
    renderPage({ localLibrary: local }, null);
    await screen.findByTestId('cite-w1');
    fireEvent.change(screen.getByTestId('cm-search'), { target: { value: 'nucleic' } });
    await waitFor(() => expect(screen.queryByTestId('cite-s1')).toBeNull());
    expect(screen.getByTestId('cite-w1')).toBeTruthy();
    // tag search
    fireEvent.change(screen.getByTestId('cm-search'), { target: { value: 'dna' } });
    await waitFor(() => expect(screen.queryByTestId('cite-s1')).toBeNull());
    // clear restores
    fireEvent.change(screen.getByTestId('cm-search'), { target: { value: '' } });
    await screen.findByTestId('cite-s1');
  });

  it('tags add/remove locally and persist in the local store', async () => {
    const local = makeMockLocalLibrary();
    await local.upsert(seed('w1', 'Molecular Structure of Nucleic Acids'), []);
    renderPage({ localLibrary: local }, null);
    fireEvent.click(await screen.findByTestId('cite-w1'));
    fireEvent.change(screen.getByTestId('cm-tag-input'), { target: { value: 'classic' } });
    fireEvent.keyDown(screen.getByTestId('cm-tag-input'), { key: 'Enter' });
    await screen.findByTestId('cm-tag-classic');
    expect(local.rows.get('w1')!.tags).toEqual(['classic']);
    // remove
    fireEvent.click(screen.getByTestId('cm-tag-classic'));
    await waitFor(() => expect(local.rows.get('w1')!.tags).toEqual([]));
  });
});

/* ---------------------- optional sync, honest status --------------------- */

describe('optional Supabase sync (local is the truth)', () => {
  it('signed-in + healthy cloud → pushed AND marked synced honestly', async () => {
    const { local, cloud } = renderPage({}, USER);
    fireEvent.click(screen.getByTestId('add-manual'));
    await waitFor(() => expect(cloud.add).toHaveBeenCalledTimes(1));
    await waitFor(() => {
      const row: StoredReference = Array.from(local.rows.values())[0];
      expect(row.sync_status).toBe('synced');
    });
    expect(screen.getByTestId('cm-sync-status').textContent).toMatch(/Synced to your account/);
  });

  it('SYNC FAILURE never loses local data — honest pending status', async () => {
    const cloud = supabaseSpy({ error: 'network down' });
    const { local } = renderPage({ citationService: cloud }, USER);
    fireEvent.click(screen.getByTestId('add-manual'));
    await waitFor(() => expect(cloud.add).toHaveBeenCalled());
    // local row survives the failed push, honestly marked pending
    await waitFor(() => {
      const row: StoredReference = Array.from(local.rows.values())[0];
      expect(row).toBeTruthy();
      expect(row.sync_status).toBe('pending');
    });
    expect(screen.getByTestId('cm-sync-status').textContent).toMatch(/Sync pending — kept locally/);
    // and the citation is still in the visible library
    expect(screen.getByTestId('citation-list').textContent).toContain('Untitled — edit details');
  });

  it('cloud offline (service degraded) → pending, never faked as synced', async () => {
    const cloud = supabaseSpy({ error: null, offline: true });
    const { local } = renderPage({ citationService: cloud }, USER);
    fireEvent.click(screen.getByTestId('add-manual'));
    await waitFor(() => {
      const row: StoredReference = Array.from(local.rows.values())[0];
      expect(row.sync_status).toBe('pending');
    });
    expect(screen.getByTestId('cm-sync-status').textContent).not.toMatch(/Synced/);
  });
});
