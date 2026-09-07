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
import { plagiarismToReport } from './adapters';
import { PLAG_EXACT_FIXTURE, PLAG_EXACT_CLEAR, LIBRARY_FIXTURE } from './plagiarismExactFixture';
import type { LocalLibrary, StoredReference } from '../citations/localLibrary';

/** Minimal citation-library double for the Set-2B add-time picker — only list()
 *  is exercised; the rest throw if touched (they must not be). */
function makeCitations(refs: StoredReference[]): LocalLibrary {
  const nope = () => { throw new Error('unused in the picker'); };
  return {
    list: async () => refs,
    search: async () => refs,
    upsert: nope as unknown as LocalLibrary['upsert'],
    setTags: nope as unknown as LocalLibrary['setTags'],
    remove: async () => {},
    markSync: nope as unknown as LocalLibrary['markSync'],
  };
}

function citationRef(id: string, title: string, authors = '', year: number | null = null): StoredReference {
  return {
    id, csl_json: '{}', doi: null, title, authors, year, tags: [],
    // Scope B: the "pre-migration / unknown" defaults — this fixture behaves like
    // an existing row (not retracted, unverified). verify_provenance is [] not
    // null because the type is string[] (Rust serializes a NULL column as []).
    retracted: false, source: null, verify_provenance: [], verify_outcome: null, verified_at: null,
  retraction_outcome: null,
  retraction_checked_at: null,
    sync_status: 'local_only', created_at: 0, updated_at: 0,
  };
}

// Exercise the RUNNABLE plagiarism path: mock the free-check flags ON (Set 1
// ships them OFF — see featureGates.vitest.tsx for the coming-soon/OFF behavior).
vi.mock('../../config/Feature', () => ({
  useFeatureFlag: (n: string) => n === 'plagiarismCheck',
  Feature: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

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

  it('M2 Set 2B — the OPTIONAL picker is populated from the citation library, defaults to none', async () => {
    const bridge = makeMockCheckBridge({ library: [] });
    const cites = makeCitations([citationRef('cit-1', 'Attention Is All You Need', 'Vaswani et al.', 2017)]);
    render(<PlagiarismLibraryManager bridge={bridge} citations={cites} />);
    const select = (await screen.findByTestId('library-citation-select')) as HTMLSelectElement;
    // grounded: the real citation appears as an option; the default is "— none —"
    await waitFor(() => expect(within(select).getByText(/Attention Is All You Need/)).toBeTruthy());
    expect(within(select).getByText(/none/i)).toBeTruthy();
    expect(select.value).toBe(''); // no auto-selection — the user must opt in
  });

  it('M2 Set 2B — adding a paper WITH a citation picked captures the link (citation_id crosses the seam)', async () => {
    const bridge = makeMockCheckBridge({ library: [] });
    const cites = makeCitations([citationRef('cit-1', 'Some Prior Work', 'Author', 2020)]);
    render(<PlagiarismLibraryManager bridge={bridge} citations={cites} />);
    const select = await screen.findByTestId('library-citation-select');
    await waitFor(() => expect(within(select as HTMLElement).getByText(/Some Prior Work/)).toBeTruthy());

    // explicitly link, then add
    fireEvent.change(select, { target: { value: 'cit-1' } });
    fireEvent.change(screen.getByTestId('library-add-input'), {
      target: { files: [new File(['x'], 'my-paper.pdf', { type: 'application/pdf' })] },
    });
    await waitFor(() => expect(screen.getByText('my-paper')).toBeTruthy());

    // the picked link was passed through libraryAdd → stored on the row
    const rows = await bridge.libraryList();
    expect(rows.find((r) => r.title === 'my-paper')?.citation_id).toBe('cit-1');
    // and the picker resets so the link is a deliberate per-paper choice
    expect((select as HTMLSelectElement).value).toBe('');
  });

  it('M2 Set 2B — adding WITHOUT a citation is unchanged: no link, citation_id stays undefined (NULL)', async () => {
    const bridge = makeMockCheckBridge({ library: [] });
    const cites = makeCitations([citationRef('cit-1', 'Some Prior Work', 'Author', 2020)]);
    render(<PlagiarismLibraryManager bridge={bridge} citations={cites} />);
    await screen.findByTestId('library-citation-select');

    // leave the picker at "— none —" and add — exactly today's flow
    fireEvent.change(screen.getByTestId('library-add-input'), {
      target: { files: [new File(['x'], 'unlinked.pdf', { type: 'application/pdf' })] },
    });
    await waitFor(() => expect(screen.getByText('unlinked')).toBeTruthy());

    const rows = await bridge.libraryList();
    expect(rows.find((r) => r.title === 'unlinked')?.citation_id).toBeUndefined();
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
    expect(screen.getAllByText(/91% word overlap/).length).toBeGreaterThanOrEqual(1);
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

// MIRROR PIN — these literals must match `match_type_label` in
// gaply-core/src/report.rs, pinned there by
// `match_type_label_wording_is_supported_by_the_algorithm`. Change one side and
// the other's counterpart must change with it.
//
// The wording states only what the score measures: cosine over HashEmbedder
// (embed.rs:33-53), a bag-of-words encoder. It is word-order-blind, so
// "verbatim" was never verifiable; it has no semantics, so "paraphrase" named
// the inverse of the signal (a real paraphrase has LOW word overlap and never
// clears the 0.80 threshold to be reported at all).
describe('matchTypeLabel wording (mirror of report.rs)', () => {
  const corpusAt = (similarity: number) => ({
    manuscript_chunk_seq: 0,
    manuscript_excerpt: 'x',
    similarity,
    source: {
      kind: 'corpus' as const,
      document_id: 1,
      chunk_id: 1,
      title: 'T',
      source_url: 'u',
      source_type: 'corpus',
      excerpt: 'e',
    },
  });
  const labelAt = (similarity: number) => {
    const report = {
      chunk_count: 1,
      threshold: 0.8,
      corpus_matches: [corpusAt(similarity)],
      self_matches: [],
      note: 'n',
    };
    return plagiarismToReport(report as never).findings[0].title;
  };

  const selfLabel = (similarity: number) => {
    const report = {
      chunk_count: 1,
      threshold: 0.8,
      corpus_matches: [],
      self_matches: [
        {
          manuscript_chunk_seq: 0,
          manuscript_excerpt: 'x',
          similarity,
          source: { kind: 'self_manuscript' as const, other_chunk_seq: 3, excerpt: 'e' },
        },
      ],
      note: 'n',
    };
    return plagiarismToReport(report as never).findings[0].title;
  };

  it('labels a self-match by LOCATION, never as a plagiarism determination', () => {
    for (const sim of [1.0, 0.9, 0.81]) {
      expect(selfLabel(sim)).toContain('internal duplication (same manuscript)');
      expect(selfLabel(sim)).not.toMatch(/self-plagiarism/i);
    }
  });

  it('names the three bands by word overlap, matching the Rust labels exactly', () => {
    expect(labelAt(1.0)).toContain('near-identical wording');
    expect(labelAt(0.98)).toContain('near-identical wording');
    expect(labelAt(0.97)).toContain('high word overlap');
    expect(labelAt(0.85)).toContain('high word overlap');
    expect(labelAt(0.84)).toContain('partial lexical overlap');
  });

  it('never claims verbatim identity, paraphrase detection, or semantics', () => {
    for (const sim of [1.0, 0.98, 0.9, 0.84, 0.8]) {
      const title = labelAt(sim);
      expect(title).not.toMatch(/paraphrase/i);
      expect(title).not.toMatch(/semantic/i);
      expect(title).not.toMatch(/\bverbatim\b/i);
      expect(title).toContain('% word overlap');
    }
  });
});
