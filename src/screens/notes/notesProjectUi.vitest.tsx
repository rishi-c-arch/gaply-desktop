// Set 5 — project quick-capture notes + unified search + the static tools panel.
// Render + mock-bridge tests: quick-capture creates a project note in one
// action; unified search/tag/type filters find BOTH note types; the recommended-
// tools panel is honest + static (no fetch to those services).
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

import NoteCreatorPage from './NoteCreatorPage';
import RecommendedToolsPanel from './RecommendedToolsPanel';
import { makeMockNotesBridge, Note } from './notesBridge';
import { makeMockPaperSource } from './paperSource';

afterEach(cleanup);

const seed = (): Note[] => [
  { id: 'p1', note_type: 'paper', paper_id: 'cite-watson', paper_title: 'Molecular Structure of Nucleic Acids',
    title: 'Watson notes', fields_json: '{"key_findings":"Double helix"}', body: '', tags: ['dna'],
    sync_status: 'local_only', created_at: 1, updated_at: 5 },
  { id: 'j1', note_type: 'project', paper_id: null, paper_title: '',
    title: 'Thesis idea', fields_json: '{}', body: 'Sleep extension improves recall; ask advisor.', tags: ['idea'],
    sync_status: 'local_only', created_at: 2, updated_at: 6 },
];

const renderPage = (notes = makeMockNotesBridge(seed())) =>
  render(
    <MemoryRouter>
      <NoteCreatorPage notes={notes} papers={makeMockPaperSource([])} />
    </MemoryRouter>
  );

/* --------------------- project quick-capture -------------------------- */

describe('Note Creator — project quick-capture (one action)', () => {
  it('creates a project note (title/body, fields_json {}) in one action', async () => {
    const notes = makeMockNotesBridge();
    renderPage(notes);
    await screen.findByTestId('quick-capture');

    fireEvent.change(screen.getByTestId('quick-title'), { target: { value: 'Try a mixed ANOVA' } });
    fireEvent.change(screen.getByTestId('quick-body'), { target: { value: 'n≈40 might be enough; check power.' } });
    fireEvent.click(screen.getByTestId('quick-save'));

    await waitFor(() => expect(notes.rows.size).toBe(1));
    const saved = Array.from(notes.rows.values())[0];
    expect(saved.note_type).toBe('project');
    expect(saved.paper_id).toBeNull();
    expect(saved.fields_json).toBe('{}'); // project notes carry no template
    expect(saved.title).toBe('Try a mixed ANOVA');
    // the box clears after capture
    expect((screen.getByTestId('quick-title') as HTMLInputElement).value).toBe('');
  });

  it('a project note opens in the project editor (not the paper template) and deletes', async () => {
    const notes = makeMockNotesBridge(seed());
    renderPage(notes);
    await screen.findByTestId('note-row-j1');
    fireEvent.click(screen.getByTestId('note-row-j1'));

    const editor = await screen.findByTestId('project-note-editor');
    // body renders in the rich writing surface (Set 1: TipTap over canonical md)
    expect(within(editor).getByTestId('project-body').textContent).toMatch(/Sleep extension/);
    // it is NOT the 8-field paper template
    expect(screen.queryByTestId('field-key_findings')).toBeNull();

    // two-step delete: first click ARMS, second click deletes
    fireEvent.click(screen.getByTestId('project-delete'));
    expect(notes.rows.has('j1')).toBe(true); // armed, not deleted
    fireEvent.click(screen.getByTestId('project-delete'));
    await waitFor(() => expect(notes.rows.has('j1')).toBe(false));
  });

  it('honest empty state names ideas/hypotheses/todos', async () => {
    renderPage(makeMockNotesBridge());
    const empty = await screen.findByTestId('note-empty');
    expect(empty.textContent).toMatch(/idea|hypothesis|to-do/i);
  });
});

/* ------------------------ unified search ------------------------------ */

describe('Note Creator — unified search across BOTH note types', () => {
  it('search finds a per-paper AND a project note', async () => {
    renderPage();
    await screen.findByTestId('note-list');
    expect(screen.getByTestId('note-row-p1')).toBeTruthy();
    expect(screen.getByTestId('note-row-j1')).toBeTruthy();

    // matches the PAPER note (via fields_json)
    fireEvent.change(screen.getByTestId('notes-search'), { target: { value: 'helix' } });
    await waitFor(() => expect(screen.queryByTestId('note-row-p1')).toBeTruthy());
    expect(screen.queryByTestId('note-row-j1')).toBeNull();

    // matches the PROJECT note (via body)
    fireEvent.change(screen.getByTestId('notes-search'), { target: { value: 'advisor' } });
    await waitFor(() => expect(screen.queryByTestId('note-row-j1')).toBeTruthy());
    expect(screen.queryByTestId('note-row-p1')).toBeNull();
  });

  it('type filter and tag filter narrow the unified list', async () => {
    renderPage();
    await screen.findByTestId('note-list');

    // type filter → project only
    fireEvent.click(screen.getByTestId('filter-project'));
    await waitFor(() => expect(screen.queryByTestId('note-row-p1')).toBeNull());
    expect(screen.getByTestId('note-row-j1')).toBeTruthy();

    // back to all, then tag filter → dna (the paper note)
    fireEvent.click(screen.getByTestId('filter-all'));
    await screen.findByTestId('note-row-p1');
    fireEvent.click(screen.getByTestId('tag-dna'));
    await waitFor(() => expect(screen.queryByTestId('note-row-j1')).toBeNull());
    expect(screen.getByTestId('note-row-p1')).toBeTruthy();
  });
});

/* ------------------ live side-by-side (Set 6 read command) ------------ */

describe('Note Creator — live side-by-side from the full_text read command', () => {
  it('shows the ACTUAL paper text beside the editor when the library has it', async () => {
    const notes = makeMockNotesBridge([], { 'A Paper With Text': 'The full extracted text of the paper.' });
    render(
      <MemoryRouter>
        <NoteCreatorPage notes={notes} papers={makeMockPaperSource([{ id: 'c1', title: 'A Paper With Text', hasFullText: true }])} />
      </MemoryRouter>
    );
    await screen.findByTestId('picker-select');
    fireEvent.change(screen.getByTestId('picker-select'), { target: { value: 'c1' } });
    fireEvent.click(screen.getByTestId('picker-start'));

    // the read command populated paperText → side-by-side renders the real text
    await screen.findByTestId('note-split');
    expect(screen.getByTestId('paper-fulltext').textContent).toContain('full extracted text of the paper');
  });

  it('gracefully stays editor-only when the paper has no stored full text', async () => {
    const notes = makeMockNotesBridge(); // no fullTexts
    render(
      <MemoryRouter>
        <NoteCreatorPage notes={notes} papers={makeMockPaperSource([{ id: 'c1', title: 'No Text Paper' }])} />
      </MemoryRouter>
    );
    await screen.findByTestId('picker-select');
    fireEvent.change(screen.getByTestId('picker-select'), { target: { value: 'c1' } });
    fireEvent.click(screen.getByTestId('picker-start'));
    await screen.findByTestId('paper-note-editor');
    expect(screen.queryByTestId('note-split')).toBeNull();
  });
});

/* ---------------------- recommended-tools panel ----------------------- */

describe('RecommendedToolsPanel — honest + static', () => {
  it('renders the two external links (target=_blank, noopener) — no data flow', () => {
    render(<RecommendedToolsPanel />);
    const lm = screen.getByTestId('tool-notebooklm') as HTMLAnchorElement;
    const gpai = screen.getByTestId('tool-gpai') as HTMLAnchorElement;
    expect(lm.getAttribute('href')).toBe('https://notebooklm.google/');
    expect(gpai.getAttribute('href')).toBe('https://gpai.app/');
    for (const a of [lm, gpai]) {
      expect(a.getAttribute('target')).toBe('_blank');
      expect(a.getAttribute('rel')).toContain('noopener');
      expect(a.getAttribute('rel')).toContain('noreferrer');
    }
  });

  it('carries the third-party disclaimer + cloud privacy labels', () => {
    render(<RecommendedToolsPanel />);
    const d = screen.getByTestId('tools-disclaimer').textContent!;
    expect(d).toMatch(/third-party/i);
    expect(d).toMatch(/not affiliated/i);
    expect(d).toMatch(/not responsible/i);
    expect(d).toMatch(/own diligence/i);
    expect(d).toMatch(/earn no money/i);
    // both tools flagged as cloud (unlike Gaply's on-device work)
    expect(screen.getByTestId('tool-cloud-notebooklm').textContent).toMatch(/cloud/i);
    expect(screen.getByTestId('tool-cloud-gpai').textContent).toMatch(/cloud/i);
  });

  it('is purely static — the module makes NO network/fetch call', () => {
    const spy = vi.spyOn(globalThis, 'fetch' as any);
    render(<RecommendedToolsPanel />);
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });
});
