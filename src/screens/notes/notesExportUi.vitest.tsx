// Note export — UI wiring. saveNoteFile is mocked to CAPTURE (name, markdown)
// so there's no real file I/O; we assert the buttons emit the right content.
// Pins: bulk "export all shown" respects the current filter; per-note export
// emits that note.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

// The mock models saveNoteFile's real contract: 'throw' = a genuine write
// failure (rejects); 'cancel' = dialog dismissed (resolves null); 'ok' = written
// (resolves a path). Cancel and success are both silent to the caller.
const { saved, behavior } = vi.hoisted(() => ({
  saved: [] as Array<{ name: string; md: string }>,
  behavior: { mode: 'ok' as 'ok' | 'cancel' | 'throw' },
}));
vi.mock('./saveNoteFile', () => ({
  saveNoteFile: (name: string, md: string) => {
    saved.push({ name, md });
    if (behavior.mode === 'throw') return Promise.reject(new Error('disk full: No space left on device'));
    return Promise.resolve(behavior.mode === 'cancel' ? null : '/Users/x/notes.md');
  },
}));

import NoteCreatorPage from './NoteCreatorPage';
import { makeMockNotesBridge, Note } from './notesBridge';
import { makeMockPaperSource } from './paperSource';

afterEach(cleanup);
beforeEach(() => { saved.length = 0; behavior.mode = 'ok'; });

const seed = (): Note[] => [
  { id: 'p1', note_type: 'paper', paper_id: 'cite-w', paper_title: 'Nucleic Acids', title: 'Watson notes',
    fields_json: '{"key_findings":"Double helix","notable_quotes":[{"text":"a structure","page":737}]}', body: '', tags: ['dna'],
    sync_status: 'local_only', created_at: 1, updated_at: 5 },
  { id: 'j1', note_type: 'project', paper_id: null, paper_title: '', title: 'Thesis idea',
    fields_json: '{}', body: 'Sleep extension improves recall.', tags: ['idea'],
    sync_status: 'local_only', created_at: 2, updated_at: 6 },
];

const renderPage = () =>
  render(
    <MemoryRouter>
      <NoteCreatorPage notes={makeMockNotesBridge(seed())} papers={makeMockPaperSource([])} />
    </MemoryRouter>
  );

describe('Note export — bulk "export all shown" respects the current filter', () => {
  it('no filter → both notes, joined by ---', async () => {
    renderPage();
    await screen.findByTestId('note-list');

    fireEvent.click(screen.getByTestId('notes-export-all'));
    await waitFor(() => expect(saved.length).toBe(1));
    expect(saved[0].md).toContain('# Watson notes');
    expect(saved[0].md).toContain('# Thesis idea');
    expect(saved[0].md).toContain('\n---\n'); // the join rule
    expect(saved[0].name).toBe('notes.md');
  });

  it('type filter Project → ONLY the project note is exported', async () => {
    renderPage();
    await screen.findByTestId('note-list');

    fireEvent.click(screen.getByTestId('filter-project'));
    await waitFor(() => expect(screen.queryByTestId('note-row-p1')).toBeNull()); // paper note filtered out

    fireEvent.click(screen.getByTestId('notes-export-all'));
    await waitFor(() => expect(saved.length).toBe(1));
    expect(saved[0].md).toContain('# Thesis idea');
    expect(saved[0].md).not.toContain('# Watson notes'); // the filter is honored
  });

  it('tag filter → filename carries the tag', async () => {
    renderPage();
    await screen.findByTestId('note-list');

    fireEvent.click(screen.getByTestId('tag-dna'));
    await waitFor(() => expect(screen.queryByTestId('note-row-j1')).toBeNull());

    fireEvent.click(screen.getByTestId('notes-export-all'));
    await waitFor(() => expect(saved.length).toBe(1));
    expect(saved[0].name).toBe('notes-dna.md');
    expect(saved[0].md).toContain('# Watson notes');
    expect(saved[0].md).not.toContain('# Thesis idea');
  });
});

describe('Note export — per-note from the editor', () => {
  it('project editor Export emits that note as markdown', async () => {
    renderPage();
    await screen.findByTestId('note-list');

    fireEvent.click(screen.getByTestId('note-row-j1')); // open the project editor
    await screen.findByTestId('project-note-editor');
    fireEvent.click(screen.getByTestId('project-export'));

    await waitFor(() => expect(saved.length).toBe(1));
    expect(saved[0].md).toContain('# Thesis idea');
    expect(saved[0].md).toContain('Sleep extension improves recall.');
    expect(saved[0].name).toBe('thesis-idea.md');
  });

  it('paper editor Export emits the structured note (heading per field)', async () => {
    renderPage();
    await screen.findByTestId('note-list');

    fireEvent.click(screen.getByTestId('note-row-p1')); // open the paper editor
    await screen.findByTestId('paper-note-editor');
    fireEvent.click(screen.getByTestId('note-export'));

    await waitFor(() => expect(saved.length).toBe(1));
    expect(saved[0].md).toContain('# Watson notes');
    expect(saved[0].md).toContain('## Key findings');
    expect(saved[0].md).toContain('> a structure (p. 737)');
  });
});

describe('Note export — write failures surface honestly; cancel & success stay silent', () => {
  it('a genuine write failure (saveNoteFile throws) shows an error message', async () => {
    behavior.mode = 'throw';
    renderPage();
    await screen.findByTestId('note-list');
    fireEvent.click(screen.getByTestId('notes-export-all'));
    const err = await screen.findByTestId('note-error'); // the honest failure message
    expect(err.textContent).toMatch(/no space|disk full|could not save/i);
  });

  it('CANCEL (saveNoteFile returns null) shows NOTHING — a silent no-op', async () => {
    behavior.mode = 'cancel';
    renderPage();
    await screen.findByTestId('note-list');
    fireEvent.click(screen.getByTestId('notes-export-all'));
    await waitFor(() => expect(saved.length).toBe(1)); // export ran…
    expect(screen.queryByTestId('note-error')).toBeNull(); // …and stayed silent (not an error)
  });

  it('SUCCESS (saveNoteFile resolves a path) shows NOTHING', async () => {
    behavior.mode = 'ok';
    renderPage();
    await screen.findByTestId('note-list');
    fireEvent.click(screen.getByTestId('notes-export-all'));
    await waitFor(() => expect(saved.length).toBe(1));
    expect(screen.queryByTestId('note-error')).toBeNull();
  });

  it('per-note editor export also surfaces a write failure', async () => {
    behavior.mode = 'throw';
    renderPage();
    await screen.findByTestId('note-list');
    fireEvent.click(screen.getByTestId('note-row-j1')); // open the project editor
    await screen.findByTestId('project-note-editor');
    fireEvent.click(screen.getByTestId('project-export'));
    const err = await screen.findByTestId('note-error');
    expect(err.textContent).toMatch(/no space|disk full|could not save/i);
  });
});
