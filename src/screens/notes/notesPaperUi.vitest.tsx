// Set 4 — the per-paper structured notes UI. Render + mock-bridge tests: the
// paper picker (library + free-typed soft anchor), the 8-field own-words
// template (all optional, round-trips via the bridge), the own-words framing +
// NO auto-summarize, optional side-by-side, and the note list/edit/delete.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

import NoteCreatorPage from './NoteCreatorPage';
import PaperNoteEditor from './PaperNoteEditor';
import { makeMockNotesBridge, Note } from './notesBridge';
import { makeMockPaperSource, TauriPaperSource } from './paperSource';
import { OFFLINE_FEATURES } from '../subscription/tiers';
import { gateFeature } from '../subscription/tiers';

afterEach(cleanup);

const PAPERS = [
  { id: 'cite-watson', title: 'Molecular Structure of Nucleic Acids', authors: 'Watson; Crick', year: 1953, hasFullText: true },
  { id: 'cite-lovelace', title: 'A Sparse Preprint', authors: 'Lovelace', year: 2024, hasFullText: false },
];

const renderPage = (notes = makeMockNotesBridge(), papers = makeMockPaperSource(PAPERS)) =>
  render(
    <MemoryRouter>
      <NoteCreatorPage notes={notes} papers={papers} />
    </MemoryRouter>
  );

/* ----------------------------- free tier ------------------------------ */

describe('Note Creator — free-tier plumbing', () => {
  it('note_creator is a free OFFLINE feature (no entitlement gate)', () => {
    expect(OFFLINE_FEATURES.has('note_creator')).toBe(true);
    // gate: offline features are always allowed, never an upsell, for any tier
    const free = gateFeature('free', 'note_creator');
    expect(free.allowed).toBe(true);
    expect(free.upsell).toBe(false);
    expect(free.reason).toBeUndefined();
  });
});

/* --------------------------- paper picker ----------------------------- */

describe('Note Creator — paper picker (library + free-typed soft anchor)', () => {
  it('picking a library paper starts a note anchored to its id + title', async () => {
    const notes = makeMockNotesBridge();
    renderPage(notes);
    await screen.findByTestId('picker-select');

    fireEvent.change(screen.getByTestId('picker-select'), { target: { value: 'cite-watson' } });
    // full-text availability surfaced
    expect(screen.getByTestId('picker-fulltext')).toBeTruthy();
    fireEvent.click(screen.getByTestId('picker-start'));

    const editor = await screen.findByTestId('paper-note-editor');
    expect(within(editor).getByTestId('editor-paper-title').textContent).toContain('Molecular Structure');

    fireEvent.change(screen.getByTestId('field-key_findings'), { target: { value: 'Double helix' } });
    fireEvent.click(screen.getByTestId('note-save'));

    await waitFor(() => expect(notes.rows.size).toBe(1));
    const saved = Array.from(notes.rows.values())[0];
    expect(saved.note_type).toBe('paper');
    expect(saved.paper_id).toBe('cite-watson'); // soft anchor
    expect(saved.paper_title).toContain('Molecular Structure');
  });

  it('a FREE-TYPED paper (no library entry) works via the soft anchor', async () => {
    const notes = makeMockNotesBridge();
    renderPage(notes, makeMockPaperSource([])); // empty library
    expect(await screen.findByTestId('picker-nolib')).toBeTruthy();

    fireEvent.change(screen.getByTestId('picker-freetype'), { target: { value: 'An unlisted paper I am reading' } });
    fireEvent.click(screen.getByTestId('picker-freestart'));
    await screen.findByTestId('paper-note-editor');
    fireEvent.click(screen.getByTestId('note-save'));

    await waitFor(() => expect(notes.rows.size).toBe(1));
    const saved = Array.from(notes.rows.values())[0];
    expect(saved.paper_id).toBeNull(); // free-typed → no anchor
    expect(saved.paper_title).toBe('An unlisted paper I am reading');
  });
});

/* ------------------------ 8-field own-words template ------------------ */

describe('PaperNoteEditor — the 8-field own-words template', () => {
  const base = { id: 'n1', paper_id: 'cite-watson', paper_title: 'Molecular Structure of Nucleic Acids' };

  it('renders all 8 optional fields; a partial note saves fine', () => {
    let saved: any = null;
    render(<PaperNoteEditor base={base} onSave={(d) => (saved = d)} onClose={() => {}} />);
    for (const f of ['field-citation', 'field-research_question', 'field-methodology', 'field-key_findings', 'field-notable_quotes', 'field-limitations', 'field-my_evaluation', 'note-tags']) {
      expect(screen.getByTestId(f)).toBeTruthy();
    }
    // fill only ONE field — the rest stay empty and it still saves
    fireEvent.change(screen.getByTestId('field-my_evaluation'), { target: { value: 'Foundational.' } });
    fireEvent.click(screen.getByTestId('note-save'));
    expect(saved.fields.my_evaluation).toBe('Foundational.');
    expect(saved.fields.citation).toBeUndefined(); // empties omitted, not forced
  });

  it('own-words framing on key_findings + my_evaluation; quotes are a distinct page-numbered field', () => {
    const { container } = render(<PaperNoteEditor base={base} onSave={() => {}} onClose={() => {}} />);
    expect(container.textContent).toMatch(/Key findings — in your own words/);
    expect(container.textContent).toMatch(/My evaluation — in your own words/);
    expect(container.textContent).toMatch(/exact wording — always with a page number/i);
    // the explicit integrity line + NO auto-summarize affordance anywhere
    expect(screen.getByTestId('own-words-note').textContent).toMatch(/never writes your notes for you/i);
    expect(container.textContent!.toLowerCase()).not.toMatch(/generate|auto-?summar|summarize for me|write my notes/);
  });

  it('notable_quotes: exact text + page round-trips', () => {
    let saved: any = null;
    render(<PaperNoteEditor base={base} onSave={(d) => (saved = d)} onClose={() => {}} />);
    fireEvent.click(screen.getByTestId('quote-add'));
    fireEvent.change(screen.getByTestId('quote-text-0'), { target: { value: 'This structure has novel features' } });
    fireEvent.change(screen.getByTestId('quote-page-0'), { target: { value: '737' } });
    fireEvent.click(screen.getByTestId('note-save'));
    expect(saved.fields.notable_quotes).toEqual([{ text: 'This structure has novel features', page: 737 }]);
  });
});

/* --------------------------- side-by-side ----------------------------- */

describe('PaperNoteEditor — optional side-by-side', () => {
  const base = { id: 'n1', paper_id: 'p', paper_title: 'A Paper' };
  it('shows the paper text beside the editor when available', () => {
    render(<PaperNoteEditor base={base} paperText={'Full text of the paper here.'} onSave={() => {}} onClose={() => {}} />);
    expect(screen.getByTestId('note-split')).toBeTruthy();
    expect(screen.getByTestId('paper-fulltext').textContent).toContain('Full text of the paper');
  });
  it('gracefully shows just the editor when no full text (no badge promised)', () => {
    render(<PaperNoteEditor base={base} onSave={() => {}} onClose={() => {}} />);
    expect(screen.queryByTestId('note-split')).toBeNull();
    expect(screen.queryByTestId('paper-fulltext-unavailable')).toBeNull(); // nothing promised → no note
    expect(screen.getByTestId('paper-note-editor')).toBeTruthy();
  });
  it('M2: when the badge promised full text but the read returns none, shows an HONEST note (never a blank/wrong panel)', () => {
    render(<PaperNoteEditor base={base} fullTextExpected paperText={null} onSave={() => {}} onClose={() => {}} />);
    expect(screen.queryByTestId('note-split')).toBeNull(); // no side-by-side
    const note = screen.getByTestId('paper-fulltext-unavailable');
    expect(note.textContent).toMatch(/full text isn’t available/i);
    expect(screen.getByTestId('paper-note-editor')).toBeTruthy(); // editor still shown
  });
});

/* ------- M2 · honest exactly-one full-text badge (TauriPaperSource) ------ */

describe('TauriPaperSource — badge promises only an UNAMBIGUOUS single match (M2)', () => {
  it('hasFullText lights up on a case-insensitive single match, NOT on a title collision', async () => {
    const citations = {
      list: async () => [
        { id: 'c1', title: 'Sleep And Memory', authors: '', year: 2021 },
        { id: 'c2', title: 'Dup Title', authors: '', year: 2020 },
        { id: 'c3', title: 'No Match', authors: '', year: 2019 },
      ],
    } as any;
    const papers = {
      libraryList: async () => [
        { id: 1, title: 'sleep and memory', added_at: 0 }, // case differs — still a single match
        { id: 2, title: 'Dup Title', added_at: 0 }, // two library rows share this title →
        { id: 3, title: 'Dup Title', added_at: 1 }, // ambiguous → the badge must NOT promise
      ],
    } as any;
    const opts = await new TauriPaperSource(citations, papers).listPapers();
    const by = (id: string) => opts.find((o) => o.id === id)!;
    expect(by('c1').hasFullText).toBe(true); // case-insensitive single match
    expect(by('c2').hasFullText).toBe(false); // collision → no false promise (backend returns None)
    expect(by('c3').hasFullText).toBe(false); // no match
  });

  it('M2 Set 2C — the badge PREFERS a reliable id (citation_id) match, mirroring the backend chain', async () => {
    const citations = {
      list: async () => [
        { id: 'c1', title: 'Wholly Different Title', authors: '', year: 2021 }, // links by id, NOT title
        { id: 'c2', title: 'Shared Twin Title', authors: '', year: 2020 }, // title collides, but has an id link
        { id: 'c3', title: 'Only By Title', authors: '', year: 2019 }, // no id link → title fallback
      ],
    } as any;
    const papers = {
      libraryList: async () => [
        // c1: id-linked though the titles differ → id path promises
        { id: 1, title: 'A Library Title', added_at: 0, citation_id: 'c1' },
        // c2: title collides (2 rows) BUT exactly one carries the id → id path still promises
        { id: 2, title: 'Shared Twin Title', added_at: 0, citation_id: 'c2' },
        { id: 3, title: 'Shared Twin Title', added_at: 1 },
        // c3: no id link, exactly one title match → Set-1 title fallback promises
        { id: 4, title: 'Only By Title', added_at: 0 },
      ],
    } as any;
    const opts = await new TauriPaperSource(citations, papers).listPapers();
    const by = (id: string) => opts.find((o) => o.id === id)!;
    expect(by('c1').hasFullText).toBe(true); // reliable id match despite differing titles
    expect(by('c2').hasFullText).toBe(true); // id path wins over the title collision
    expect(by('c3').hasFullText).toBe(true); // no id → honest single title match
  });

  it('M2 Set 2C — an AMBIGUOUS id (two rows same citation_id) does NOT promise (floor holds on the id path)', async () => {
    const citations = { list: async () => [{ id: 'c1', title: 'No Title Match Here', authors: '', year: 2021 }] } as any;
    const papers = {
      libraryList: async () => [
        { id: 1, title: 'Row One', added_at: 0, citation_id: 'c1' },
        { id: 2, title: 'Row Two', added_at: 1, citation_id: 'c1' }, // same id → 2 matches
      ],
    } as any;
    const opts = await new TauriPaperSource(citations, papers).listPapers();
    // id collision → backend returns None; no title match either → no false promise
    expect(opts.find((o) => o.id === 'c1')!.hasFullText).toBe(false);
  });
});

/* ----------------------------- note list ------------------------------ */

describe('Note Creator — list / edit / delete + empty state', () => {
  const seed: Note[] = [
    { id: 'n1', note_type: 'paper', paper_id: 'cite-watson', paper_title: 'Molecular Structure of Nucleic Acids',
      title: 'My Watson notes', fields_json: '{"key_findings":"Double helix"}', body: '', tags: ['dna'],
      sync_status: 'local_only', created_at: 1, updated_at: 2 },
  ];

  it('honest empty state', async () => {
    renderPage(makeMockNotesBridge());
    expect(await screen.findByTestId('note-empty')).toBeTruthy();
  });

  it('lists notes, opens one to edit (prefilled), and deletes it', async () => {
    const notes = makeMockNotesBridge(seed);
    renderPage(notes);
    await screen.findByTestId('note-row-n1');
    fireEvent.click(screen.getByTestId('note-row-n1'));

    // prefilled from the stored note (fields parsed back)
    const editor = await screen.findByTestId('paper-note-editor');
    expect((within(editor).getByTestId('note-title') as HTMLInputElement).value).toBe('My Watson notes');
    expect((within(editor).getByTestId('field-key_findings') as HTMLTextAreaElement).value).toBe('Double helix');

    fireEvent.click(screen.getByTestId('note-delete'));
    await waitFor(() => expect(notes.rows.has('n1')).toBe(false));
    await screen.findByTestId('note-empty');
  });
});
