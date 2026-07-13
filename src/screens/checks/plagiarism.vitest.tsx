// Set 5 — the deterministic plagiarism report UI + "my papers" library manager.
// Presentational tests over the wire fixtures: side-by-side matches, honest
// numbers, un-strippable disclosure, inspector, library manager, empty state.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import PlagiarismCheckPage from './PlagiarismCheckPage';
import PlagiarismExactReport from './PlagiarismExactReport';
import PlagiarismLibraryManager from './PlagiarismLibraryManager';
import { makeMockCheckBridge } from './checkBridge';
import { PLAG_EXACT_FIXTURE, PLAG_EXACT_CLEAR, LIBRARY_FIXTURE } from './plagiarismExactFixture';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

function auth(session: any = null): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

const renderPage = (bridge: any) =>
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(null)}>
        <PlagiarismCheckPage bridge={bridge} />
      </GaplySessionProvider>
    </MemoryRouter>
  );

/* --------------------------- the report (unit) --------------------------- */

describe('PlagiarismExactReport — side-by-side + honesty', () => {
  it('shows each match verbatim in BOTH places (source + matched span)', () => {
    render(<PlagiarismExactReport result={PLAG_EXACT_FIXTURE} />);
    // 2 matches (1 library + 1 self)
    expect(screen.getByTestId('plag-match-0')).toBeTruthy();
    expect(screen.getByTestId('plag-match-1')).toBeTruthy();
    // the SAME overlapping text appears in the upload column AND the matched column
    const source0 = screen.getByTestId('plag-source-0').textContent;
    const target0 = screen.getByTestId('plag-target-0').textContent;
    expect(source0).toContain('cytochrome c');
    expect(target0).toContain('cytochrome c');
    expect(source0).toBe(target0); // verbatim overlap — provably the same text
    // rendered via gds-highlight (the reused pattern)
    expect(screen.getAllByText('cytochrome c', { exact: false }).length).toBeGreaterThanOrEqual(2);
  });

  it('shows the honest overlap PROPORTION, never a plagiarism score/verdict', () => {
    const { container } = render(<PlagiarismExactReport result={PLAG_EXACT_FIXTURE} />);
    expect(screen.getByTestId('plag-duplication').textContent).toBe('18%');
    expect(container.textContent).toMatch(/proportion of\s+matched text/i);
    expect(container.textContent).toMatch(/NOT a plagiarism score/i);
    // never frames a verdict
    expect(container.textContent!.toLowerCase()).not.toContain('plagiarism score:');
    expect(container.textContent!.toLowerCase()).not.toMatch(/verdict|guilty|plagiarised\b/);
  });

  it('the named-scope disclosure + Turnitin limit are un-strippable (all-clear too)', () => {
    const first = render(<PlagiarismExactReport result={PLAG_EXACT_FIXTURE} />);
    const d = screen.getByTestId('plag-disclosure').textContent!;
    expect(d).toContain('Prior study (2019)');
    expect(d).toContain('your 2 paper(s) in your library');
    expect(d).toContain('does NOT replace Turnitin');
    expect(screen.getByTestId('plag-scope').textContent).toContain('Methods handbook');
    first.unmount();

    // all-clear report: NO matches, but the disclosure + "absence" honesty stay
    render(<PlagiarismExactReport result={PLAG_EXACT_CLEAR} />);
    expect(screen.getByTestId('plag-empty')).toBeTruthy();
    expect(screen.getByTestId('plag-disclosure').textContent).toContain('does NOT replace Turnitin');
    expect(screen.getByTestId('plag-empty').textContent).toMatch(/absence of matches.*does NOT mean.*original/i);
  });

  it('clicking a match opens the inspector with both passages + which paper', () => {
    render(<PlagiarismExactReport result={PLAG_EXACT_FIXTURE} />);
    expect(screen.queryByTestId('plag-inspector')).toBeNull();
    fireEvent.click(screen.getByTestId('plag-match-0'));
    const insp = screen.getByTestId('plag-inspector');
    expect(within(insp).getByTestId('inspector-source').textContent).toContain('cytochrome c');
    expect(within(insp).getByTestId('inspector-target').textContent).toContain('cytochrome c');
    expect(insp.textContent).toContain('Prior study (2019)'); // which library paper
    expect(insp.textContent).toMatch(/overlap 100%/);
  });
});

/* ------------------------- the library manager --------------------------- */

describe('PlagiarismLibraryManager — curate the "my papers" set', () => {
  it('lists papers, adds one, removes one', async () => {
    const bridge = makeMockCheckBridge({ library: [...LIBRARY_FIXTURE] });
    render(<PlagiarismLibraryManager bridge={bridge} />);
    await screen.findByTestId('library-list');
    expect(screen.getByText('Prior study (2019)')).toBeTruthy();

    // add
    fireEvent.change(screen.getByTestId('library-add-input'), {
      target: { files: [new File(['x'], 'new-paper.pdf', { type: 'application/pdf' })] },
    });
    await waitFor(() => expect(screen.getByText('new-paper')).toBeTruthy());

    // remove the first
    fireEvent.click(screen.getByTestId('library-remove-1'));
    await waitFor(() => expect(screen.queryByText('Prior study (2019)')).toBeNull());
  });

  it('honest empty state when the library has no papers', async () => {
    const bridge = makeMockCheckBridge({ library: [] });
    render(<PlagiarismLibraryManager bridge={bridge} />);
    const empty = await screen.findByTestId('library-empty');
    expect(empty.textContent).toMatch(/No papers in your library yet/i);
  });
});

/* ------------------------------ the page --------------------------------- */

describe('PlagiarismCheckPage — exact lane is primary, library present', () => {
  it('runs the exact check and renders the side-by-side report + disclosure', async () => {
    const onCall: Array<[string, string]> = [];
    const bridge = makeMockCheckBridge({
      exact: PLAG_EXACT_FIXTURE,
      library: [...LIBRARY_FIXTURE],
      onCall: (cmd, path) => onCall.push([cmd, path]),
    });
    renderPage(bridge);
    // the "my papers" library manager is on the (default) exact lane
    await screen.findByTestId('plag-library');

    fireEvent.change(await screen.findByTestId('file-input'), {
      target: { files: [new File(['x'], 'manuscript.pdf', { type: 'application/pdf' })] },
    });
    await screen.findByTestId('selected-name');
    fireEvent.click(screen.getByTestId('run-check'));
    await screen.findByTestId('plag-report');

    // side-by-side + disclosure rendered from the exact command
    expect(screen.getByTestId('plag-duplication').textContent).toBe('18%');
    expect(screen.getByTestId('plag-disclosure').textContent).toContain('does NOT replace Turnitin');
    // it called the DETERMINISTIC command with a path only (never bytes)
    expect(onCall.some(([c]) => c === 'check_plagiarism_exact')).toBe(true);
    expect(onCall.find(([c]) => c === 'check_plagiarism_exact')![1]).toBe('manuscript.pdf');
  });

  it('the similar-meaning lane still works and is clearly separate', async () => {
    const bridge = makeMockCheckBridge({
      exact: PLAG_EXACT_FIXTURE,
      plagiarism: {
        chunk_count: 4, threshold: 0.8,
        corpus_matches: [{ manuscript_chunk_seq: 1, manuscript_excerpt: 'x', similarity: 0.91,
          source: { kind: 'corpus', document_id: 1, chunk_id: 2, title: 'T', source_url: 'u', source_type: 'j', excerpt: 'e' } }],
        self_matches: [],
        note: 'isolation note',
      },
    });
    renderPage(bridge);
    fireEvent.click(await screen.findByTestId('mode-similar'));
    // library manager is exact-only — gone on the semantic lane
    expect(screen.queryByTestId('plag-library')).toBeNull();
    fireEvent.change(await screen.findByTestId('file-input'), {
      target: { files: [new File(['x'], 'paper.pdf', { type: 'application/pdf' })] },
    });
    await screen.findByTestId('selected-name');
    fireEvent.click(screen.getByTestId('run-check'));
    await screen.findByTestId('check-report');
    expect(screen.getAllByText(/91% similarity/).length).toBeGreaterThanOrEqual(1);
    // with a real corpus match, the honest "no corpus" note is absent
    expect(screen.queryByTestId('plag-no-corpus-note')).toBeNull();
  });

  it('empty corpus matches show an HONEST note — empty is NOT "verified clean" (M1)', async () => {
    const bridge = makeMockCheckBridge({
      exact: PLAG_EXACT_FIXTURE,
      plagiarism: {
        chunk_count: 3, threshold: 0.8,
        corpus_matches: [], // no external corpus configured (post-M1 reality)
        self_matches: [],
        note: 'isolation note',
      },
    });
    renderPage(bridge);
    fireEvent.click(await screen.findByTestId('mode-similar'));
    fireEvent.change(await screen.findByTestId('file-input'), {
      target: { files: [new File(['x'], 'paper.pdf', { type: 'application/pdf' })] },
    });
    await screen.findByTestId('selected-name');
    fireEvent.click(screen.getByTestId('run-check'));
    await screen.findByTestId('check-report');
    const note = await screen.findByTestId('plag-no-corpus-note');
    expect(note.textContent).toMatch(/does\s+not\s+mean the text is original/i);
  });
});
