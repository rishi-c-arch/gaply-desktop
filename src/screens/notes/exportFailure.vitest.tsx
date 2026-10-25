// A failed note export shows one fixed sentence (Research OS, Phase 5B-4b;
// decisions 169 and 176). The error's own message could hold a path or a
// file name, so it reaches neither the screen nor the console. Covers the
// Note Creator's export of all shown notes and the per-note export in both
// editors.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

const SENTINEL = 'HTSENTINEL_export_/Users/someone/HTPATH_q9/notes.md';
vi.mock('./saveNoteFile', () => ({
  saveNoteFile: () => Promise.reject(new Error(SENTINEL)),
}));

import NoteCreatorPage from './NoteCreatorPage';
import { makeMockNotesBridge, Note } from './notesBridge';
import { makeMockPaperSource } from './paperSource';

const SENTENCE = 'Gaply could not save the export. No file was written.';

const seed = (): Note[] => [
  { id: 'p1', note_type: 'paper', paper_id: 'cite-w', paper_title: 'Nucleic Acids', title: 'Watson notes',
    fields_json: '{"key_findings":"Double helix"}', body: '', tags: [],
    sync_status: 'local_only', created_at: 1, updated_at: 5 },
  { id: 'j1', note_type: 'project', paper_id: null, paper_title: '', title: 'Thesis idea',
    fields_json: '{}', body: 'A made-up body.', tags: [],
    sync_status: 'local_only', created_at: 2, updated_at: 6 },
];

const consoles = ['log', 'info', 'warn', 'error', 'debug'] as const;
let spies: Array<ReturnType<typeof vi.spyOn>> = [];
beforeEach(() => {
  spies = consoles.map((k) => vi.spyOn(console, k).mockImplementation(() => {}));
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** Nothing of the error's message anywhere a user or a log could see. */
function expectNoSentinel() {
  expect(document.body.innerHTML).not.toContain('HTSENTINEL');
  expect(document.body.innerHTML).not.toContain('HTPATH');
  for (const s of spies) {
    const said = s.mock.calls.map((c: unknown[]) => c.map((a: unknown) => (a instanceof Error ? `${a.message} ${a.stack}` : String(a))).join(' '));
    expect(said.join('\n')).not.toMatch(/HTSENTINEL|HTPATH/);
  }
}

const renderPage = async () => {
  render(
    <MemoryRouter>
      <NoteCreatorPage notes={makeMockNotesBridge(seed())} papers={makeMockPaperSource([])} />
    </MemoryRouter>
  );
  await screen.findByTestId('note-list');
};

describe('a failed note export', () => {
  it('of all shown notes shows the sentence and nothing of the error', async () => {
    await renderPage();
    fireEvent.click(screen.getByTestId('notes-export-all'));
    expect((await screen.findByTestId('note-error')).textContent).toBe(SENTENCE);
    expectNoSentinel();
  });

  it('of a project note shows the sentence and nothing of the error', async () => {
    await renderPage();
    fireEvent.click(screen.getByTestId('note-row-j1'));
    await screen.findByTestId('project-note-editor');
    fireEvent.click(screen.getByTestId('project-export'));
    expect((await screen.findByTestId('note-error')).textContent).toBe(SENTENCE);
    expectNoSentinel();
  });

  it('of a paper note shows the sentence and nothing of the error', async () => {
    await renderPage();
    fireEvent.click(screen.getByTestId('note-row-p1'));
    await screen.findByTestId('paper-note-editor');
    fireEvent.click(screen.getByTestId('note-export'));
    expect((await screen.findByTestId('note-error')).textContent).toBe(SENTENCE);
    expectNoSentinel();
  });
});
