// Decision 231: the Note Creator's three raw error messages, each now one
// closed sentence. Each site is made to fail through its bridge, once with an
// Error and once with a plain string (Tauri rejects with the error's string),
// and must show its sentence and no part of the error's message.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'fs';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));
// The manuscript editor's structures, from the shipped catalog on disk.
vi.mock('./manuscriptScaffolds', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./manuscriptScaffolds')>();
  const fs = await import('fs');
  const cat = JSON.parse(fs.readFileSync('public/manuscripts/scaffolds.json', 'utf8'));
  return { ...actual, loadScaffoldCatalog: async () => cat, loadScaffolds: async () => cat.scaffolds };
});

import NoteCreatorPage, { NOTE_SAVE_FAILED, NOTES_LOAD_FAILED } from './NoteCreatorPage';
import { makeMockNotesBridge, Note, NotesBridge } from './notesBridge';
import { makeMockPaperSource } from './paperSource';
import { manuscriptToDraft, newManuscript } from './manuscriptModel';

afterEach(cleanup);

const SECRET = 'SENTINEL_NOTE_ERR /Users/someone/thesis.docx';
const FAILURES: Array<[string, () => unknown]> = [
  ['an Error', () => new Error(SECRET)],
  ['a string', () => SECRET],
];

const project: Note = {
  id: 'j1', note_type: 'project', paper_id: null, paper_title: '', title: 'Thesis idea', fields_json: '{}',
  body: 'Sleep extension improves recall.', tags: [], sync_status: 'local_only', created_at: 2, updated_at: 6,
};

async function manuscriptNote(): Promise<Note> {
  const m = newManuscript('m1');
  m.title = 'My paper';
  const b = makeMockNotesBridge();
  await b.create(manuscriptToDraft(m));
  return (await b.get('m1'))!;
}

const renderPage = (notes: NotesBridge) =>
  render(
    <MemoryRouter>
      <NoteCreatorPage notes={notes} papers={makeMockPaperSource([])} />
    </MemoryRouter>
  );

const shownError = () => screen.getByTestId('note-error').textContent ?? '';
const noSecret = () => {
  expect(document.body.textContent).not.toContain('SENTINEL_NOTE_ERR');
  expect(document.body.textContent).not.toContain('/Users/');
};

describe('loading notes (NoteCreatorPage.tsx:131)', () => {
  it.each(FAILURES)('fails with %s: the load sentence, and no part of the error', async (_, fail) => {
    const notes = makeMockNotesBridge([project]);
    notes.search = () => Promise.reject(fail());
    renderPage(notes);
    await waitFor(() => expect(shownError()).toBe(NOTES_LOAD_FAILED));
    noSecret();
  });

  it('a load that then succeeds clears it', async () => {
    const notes = makeMockNotesBridge([project]);
    const real = notes.search.bind(notes);
    let failing = true;
    notes.search = (q, t) => (failing ? Promise.reject(new Error(SECRET)) : real(q, t));
    renderPage(notes);
    await waitFor(() => expect(shownError()).toBe(NOTES_LOAD_FAILED));
    failing = false;
    // Typing a search reloads.
    fireEvent.change(screen.getByTestId('notes-search'), { target: { value: 'Thesis' } });
    await waitFor(() => expect(screen.queryByTestId('note-error')).toBeNull());
    expect(await screen.findByTestId('note-row-j1')).toBeTruthy();
  });

  it('the sentence is the decision\'s', () => {
    expect(NOTES_LOAD_FAILED).toBe(
      'Gaply could not load your notes just now. Try again in a moment. If it keeps happening, restart Gaply.'
    );
    expect(NOTE_SAVE_FAILED).toBe('Gaply could not save this note just now. What you typed is still here, so you can try again.');
  });
});

describe('saving a note from each editor (NoteCreatorPage.tsx:188)', () => {
  it.each(FAILURES)('the paper editor, failing with %s: the save sentence, and what was typed is still there', async (_, fail) => {
    const notes = makeMockNotesBridge();
    notes.create = () => Promise.reject(fail());
    renderPage(notes);
    fireEvent.change(await screen.findByTestId('picker-freetype'), { target: { value: 'An unlisted paper' } });
    fireEvent.click(screen.getByTestId('picker-freestart'));
    await screen.findByTestId('paper-note-editor');
    fireEvent.change(screen.getByTestId('field-key_findings'), { target: { value: 'TYPED_FINDINGS' } });
    fireEvent.click(screen.getByTestId('note-save'));
    await waitFor(() => expect(shownError()).toBe(NOTE_SAVE_FAILED));
    expect(screen.getByTestId('paper-note-editor')).toBeTruthy();
    expect((screen.getByTestId('field-key_findings') as HTMLTextAreaElement).value).toBe('TYPED_FINDINGS');
    expect(notes.rows.size).toBe(0);
    noSecret();
  });

  it.each(FAILURES)('the project editor, failing with %s: the save sentence, and what was typed is still there', async (_, fail) => {
    const notes = makeMockNotesBridge([project]);
    notes.update = () => Promise.reject(fail());
    renderPage(notes);
    fireEvent.click(await screen.findByTestId('note-row-j1'));
    await screen.findByTestId('project-note-editor');
    fireEvent.change(screen.getByTestId('project-title'), { target: { value: 'TYPED_TITLE' } });
    fireEvent.click(screen.getByTestId('project-save'));
    await waitFor(() => expect(shownError()).toBe(NOTE_SAVE_FAILED));
    expect((screen.getByTestId('project-title') as HTMLInputElement).value).toBe('TYPED_TITLE');
    expect(notes.rows.get('j1')!.title).toBe('Thesis idea');
    noSecret();
  });

  it.each(FAILURES)('the manuscript editor, failing with %s: the save sentence, and what was typed is still there', async (_, fail) => {
    const notes = makeMockNotesBridge([await manuscriptNote()]);
    // A manuscript's save goes through create (an upsert by id) as well as update.
    notes.create = () => Promise.reject(fail());
    notes.update = () => Promise.reject(fail());
    renderPage(notes);
    fireEvent.click(await screen.findByTestId('note-row-m1'));
    const title = (await screen.findByTestId('ms-title')) as HTMLInputElement;
    fireEvent.change(title, { target: { value: 'TYPED_MS_TITLE' } });
    fireEvent.click(screen.getByTestId('ms-save'));
    await waitFor(() => expect(shownError()).toBe(NOTE_SAVE_FAILED));
    expect((screen.getByTestId('ms-title') as HTMLInputElement).value).toBe('TYPED_MS_TITLE');
    noSecret();
  });
});

describe('quick capture (NoteCreatorPage.tsx:232)', () => {
  it.each(FAILURES)('failing with %s: the save sentence, and what was typed is still in the boxes', async (_, fail) => {
    const notes = makeMockNotesBridge();
    notes.create = () => Promise.reject(fail());
    renderPage(notes);
    await screen.findByTestId('quick-capture');
    fireEvent.change(screen.getByTestId('quick-title'), { target: { value: 'TYPED_QUICK' } });
    fireEvent.change(screen.getByTestId('quick-body'), { target: { value: 'TYPED_BODY' } });
    fireEvent.click(screen.getByTestId('quick-save'));
    await waitFor(() => expect(shownError()).toBe(NOTE_SAVE_FAILED));
    expect((screen.getByTestId('quick-title') as HTMLInputElement).value).toBe('TYPED_QUICK');
    expect((screen.getByTestId('quick-body') as HTMLTextAreaElement).value).toBe('TYPED_BODY');
    noSecret();
  });
});

// The site list is decision 231's: if a new raw message appears in the page,
// this is where it shows.
it('the page builds no message from an error', () => {
  const src = readFileSync('src/screens/notes/NoteCreatorPage.tsx', 'utf8');
  expect(src).not.toMatch(/e instanceof Error \? e\.message/);
});
