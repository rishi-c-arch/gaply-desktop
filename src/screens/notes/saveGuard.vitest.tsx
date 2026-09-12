// Note Creator — the unsaved-work guard + the honest save-state label.
//
// These two ship as one fix and are tested as one: the old constant
// "Saved locally · on device" was what made the unguarded back arrow FEEL safe,
// so a test that proved the guard without proving the label would leave the
// misleading half standing.
//
// Pins, per editor: an untouched note closes straight away; an edited one warns
// and "Keep editing" preserves every keystroke; "Discard changes" closes; and
// editing back to the original state goes clean again (a baseline comparison,
// not a "was ever touched" flag).
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

// Serve the REAL shipped catalog to the async loader (same pattern as
// manuscript.vitest.tsx): a bare relative fetch fails in jsdom, which would
// silently fall back to the generic-only scaffold and hide the switch case.
vi.mock('./manuscriptScaffolds', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./manuscriptScaffolds')>();
  const fs = await import('fs');
  const cat = JSON.parse(fs.readFileSync('public/manuscripts/scaffolds.json', 'utf8'));
  return { ...actual, loadScaffoldCatalog: async () => cat, loadScaffolds: async () => cat.scaffolds };
});

import NoteCreatorPage from './NoteCreatorPage';
import ManuscriptEditor from './ManuscriptEditor';
import ProjectNoteEditor from './ProjectNoteEditor';
import { SaveState } from './editorSaveState';
import { makeMockNotesBridge, Note, NoteDraft } from './notesBridge';
import { makeMockPaperSource } from './paperSource';
import { makeMockLocalLibrary } from '../citations/localLibrary';

afterEach(cleanup);

/** A saved epoch-second timestamp (2026-02-14T10:30:00Z) — comfortably past the
 *  1e9 plausibility guard, so the label renders a real date. */
const SAVED_AT = 1771065000;

const seed = (): Note[] => [
  {
    id: 'p1', note_type: 'paper', paper_id: 'cite-watson',
    paper_title: 'Molecular Structure of Nucleic Acids', title: 'Watson notes',
    fields_json: '{"key_findings":"Double helix"}', body: '', tags: ['dna'],
    sync_status: 'local_only', created_at: SAVED_AT, updated_at: SAVED_AT,
  },
  {
    id: 'j1', note_type: 'project', paper_id: null, paper_title: '', title: 'Thesis idea',
    fields_json: '{}', body: 'Sleep extension improves recall; ask advisor.', tags: ['idea'],
    sync_status: 'local_only', created_at: SAVED_AT, updated_at: SAVED_AT,
  },
];

const renderPage = (notes = makeMockNotesBridge(seed())) =>
  render(
    <MemoryRouter>
      <NoteCreatorPage notes={notes} papers={makeMockPaperSource([])} />
    </MemoryRouter>
  );

const openProjectNote = async () => {
  renderPage();
  fireEvent.click(await screen.findByTestId('note-row-j1'));
  return screen.findByTestId('project-note-editor');
};

/* ------------------------- the label itself ---------------------------- */

describe('SaveState reports the real state, never a constant', () => {
  it('a note with no stored row says so — it never claims to be saved', () => {
    render(<SaveState dirty={false} savedAt={null} />);
    const el = screen.getByTestId('save-state');
    expect(el.getAttribute('data-state')).toBe('new');
    expect(el.textContent).toBe('Not saved yet');
  });

  it('a saved, untouched note names WHEN it was saved', () => {
    render(<SaveState dirty={false} savedAt={SAVED_AT} />);
    const el = screen.getByTestId('save-state');
    expect(el.getAttribute('data-state')).toBe('saved');
    expect(el.textContent).toMatch(/^Saved locally · .+/);
    // the real timestamp, not a fabricated one
    expect(el.textContent).toContain(String(new Date(SAVED_AT * 1000).getFullYear()));
  });

  it('a stored row with an implausible timestamp stays "Saved locally" — no 1970, no false "unsaved"', () => {
    render(<SaveState dirty={false} savedAt={6} />);
    const el = screen.getByTestId('save-state');
    expect(el.getAttribute('data-state')).toBe('saved');
    expect(el.textContent).toBe('Saved locally'); // truthful, just undated
    expect(el.textContent).not.toContain('1970');
  });

  it('dirty beats saved, and saving beats everything', () => {
    const { rerender } = render(<SaveState dirty savedAt={SAVED_AT} />);
    expect(screen.getByTestId('save-state').getAttribute('data-state')).toBe('unsaved');
    expect(screen.getByTestId('save-state').textContent).toBe('Unsaved changes');

    rerender(<SaveState dirty busy savedAt={SAVED_AT} />);
    expect(screen.getByTestId('save-state').getAttribute('data-state')).toBe('saving');
    expect(screen.getByTestId('save-state').textContent).toBe('Saving…');
  });
});

/* --------------------------- project editor ---------------------------- */

describe('project editor — close guard', () => {
  it('opens showing the saved time, not a constant', async () => {
    await openProjectNote();
    const state = screen.getByTestId('project-save-state');
    expect(state.getAttribute('data-state')).toBe('saved');
    expect(state.textContent).toMatch(/^Saved locally · /);
  });

  it('an UNTOUCHED note closes immediately — the guard never nags', async () => {
    await openProjectNote();
    fireEvent.click(screen.getByTestId('project-close'));
    await waitFor(() => expect(screen.queryByTestId('project-note-editor')).toBeNull());
    expect(screen.queryByTestId('project-discard')).toBeNull();
  });

  it('an EDITED note warns instead of closing, and "Keep editing" keeps every keystroke', async () => {
    await openProjectNote();
    fireEvent.change(screen.getByTestId('project-title'), { target: { value: 'Thesis idea — revised' } });
    expect(screen.getByTestId('project-save-state').getAttribute('data-state')).toBe('unsaved');

    fireEvent.click(screen.getByTestId('project-close'));
    expect(await screen.findByTestId('project-discard')).toBeTruthy();
    // still open, nothing lost
    expect(screen.getByTestId('project-note-editor')).toBeTruthy();

    fireEvent.click(screen.getByTestId('project-discard-keep'));
    await waitFor(() => expect(screen.queryByTestId('project-discard')).toBeNull());
    expect(screen.getByTestId('project-note-editor')).toBeTruthy();
    expect((screen.getByTestId('project-title') as HTMLInputElement).value).toBe('Thesis idea — revised');
  });

  it('"Discard changes" is what actually closes it', async () => {
    await openProjectNote();
    fireEvent.change(screen.getByTestId('project-title'), { target: { value: 'gone' } });
    fireEvent.click(screen.getByTestId('project-close'));
    fireEvent.click(await screen.findByTestId('project-discard-discard'));
    await waitFor(() => expect(screen.queryByTestId('project-note-editor')).toBeNull());
  });

  it('editing BACK to the original is clean again — a baseline, not a touched flag', async () => {
    await openProjectNote();
    const title = screen.getByTestId('project-title');
    fireEvent.change(title, { target: { value: 'Thesis idea!!' } });
    expect(screen.getByTestId('project-save-state').getAttribute('data-state')).toBe('unsaved');

    fireEvent.change(title, { target: { value: 'Thesis idea' } });
    expect(screen.getByTestId('project-save-state').getAttribute('data-state')).toBe('saved');

    fireEvent.click(screen.getByTestId('project-close'));
    await waitFor(() => expect(screen.queryByTestId('project-note-editor')).toBeNull());
  });

  it('whitespace-only churn is not a change (the snapshot matches what save persists)', async () => {
    await openProjectNote();
    fireEvent.change(screen.getByTestId('project-title'), { target: { value: '  Thesis idea  ' } });
    expect(screen.getByTestId('project-save-state').getAttribute('data-state')).toBe('saved');
  });

  it('a NEW note reads "Not saved yet" and still guards real content', () => {
    const onClose = vi.fn();
    render(<ProjectNoteEditor id="fresh" existing={null} onSave={vi.fn()} onClose={onClose} />);
    expect(screen.getByTestId('project-save-state').getAttribute('data-state')).toBe('new');

    fireEvent.change(screen.getByTestId('project-title'), { target: { value: 'A thought' } });
    fireEvent.click(screen.getByTestId('project-close'));
    expect(screen.getByTestId('project-discard')).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});

/* ---------------------------- paper editor ----------------------------- */

describe('paper editor — close guard', () => {
  const openPaperNote = async () => {
    renderPage();
    fireEvent.click(await screen.findByTestId('note-row-p1'));
    return screen.findByTestId('paper-note-editor');
  };

  it('an untouched note closes; an edited template field warns', async () => {
    await openPaperNote();
    expect(screen.getByTestId('note-save-state').getAttribute('data-state')).toBe('saved');

    fireEvent.change(screen.getByTestId('field-limitations'), { target: { value: 'Small n.' } });
    expect(screen.getByTestId('note-save-state').getAttribute('data-state')).toBe('unsaved');
    // BOTH labels move together — the footer one was the second copy of the old constant
    expect(screen.getByTestId('note-save-state-foot').getAttribute('data-state')).toBe('unsaved');

    fireEvent.click(screen.getByTestId('note-close'));
    expect(await screen.findByTestId('note-discard')).toBeTruthy();
    expect(screen.getByTestId('paper-note-editor')).toBeTruthy();

    fireEvent.click(screen.getByTestId('note-discard-keep'));
    expect((screen.getByTestId('field-limitations') as HTMLTextAreaElement).value).toBe('Small n.');
  });

  it('adding a quote row with no text is not a change; typing in it is', async () => {
    await openPaperNote();
    fireEvent.click(screen.getByTestId('quote-add'));
    // an empty quote is dropped by currentFields(), so nothing would be persisted
    expect(screen.getByTestId('note-save-state').getAttribute('data-state')).toBe('saved');

    fireEvent.change(screen.getByTestId('quote-text-0'), { target: { value: 'novel features' } });
    expect(screen.getByTestId('note-save-state').getAttribute('data-state')).toBe('unsaved');
  });

  it('an untouched paper note closes with no prompt', async () => {
    await openPaperNote();
    fireEvent.click(screen.getByTestId('note-close'));
    await waitFor(() => expect(screen.queryByTestId('paper-note-editor')).toBeNull());
  });
});

/* -------------------------- manuscript editor -------------------------- */

describe('manuscript editor — close guard', () => {
  const manuscriptNote = (sections: Array<{ key: string; heading: string; body: string }>): Note => ({
    id: 'm1', note_type: 'manuscript', paper_id: null, paper_title: '', title: 'A paper',
    fields_json: JSON.stringify({ authors: 'Ada Lovelace', scaffoldId: 'imrad-generic', cslStyleId: 'apa', sections }),
    body: sections.map((s) => s.body).join('\n\n'), tags: [],
    sync_status: 'local_only', created_at: SAVED_AT, updated_at: SAVED_AT,
  });

  const openManuscript = async (existing: Note | null, onClose = vi.fn()) => {
    render(
      <ManuscriptEditor
        id="m1"
        existing={existing}
        onSave={vi.fn() as (d: NoteDraft) => void}
        onClose={onClose}
        library={makeMockLocalLibrary()}
      />
    );
    return { onClose };
  };

  it('an existing manuscript opens saved, and typing prose arms the guard', async () => {
    const { onClose } = await openManuscript(manuscriptNote([{ key: 'introduction', heading: 'Introduction', body: 'We begin.' }]));
    await screen.findByTestId('manuscript-editor');
    expect(screen.getByTestId('ms-save-state').getAttribute('data-state')).toBe('saved');

    fireEvent.change(screen.getByTestId('ms-title'), { target: { value: 'A paper, revised' } });
    expect(screen.getByTestId('ms-save-state').getAttribute('data-state')).toBe('unsaved');

    fireEvent.click(screen.getByTestId('ms-close'));
    expect(await screen.findByTestId('ms-discard')).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('ms-discard-discard'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('an untouched manuscript closes with no prompt', async () => {
    const { onClose } = await openManuscript(manuscriptNote([{ key: 'introduction', heading: 'Introduction', body: 'We begin.' }]));
    await screen.findByTestId('manuscript-editor');
    fireEvent.click(screen.getByTestId('ms-close'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('ms-discard')).toBeNull();
  });

  it('CHOOSING the starting structure is not work to protect — the guard stays disarmed', async () => {
    await openManuscript(null);
    // a new manuscript opens on the scaffold picker
    fireEvent.click(await screen.findByTestId('sp-pick-ieee'));
    await screen.findByTestId('ms-nav');

    expect(screen.getByTestId('ms-save-state').getAttribute('data-state')).toBe('new');
    fireEvent.click(screen.getByTestId('ms-close'));
    expect(screen.queryByTestId('ms-discard')).toBeNull();
  });

  it('SWITCHING structure on an existing manuscript IS a change and is guarded', async () => {
    await openManuscript(manuscriptNote([{ key: 'introduction', heading: 'Introduction', body: 'We begin.' }]));
    await screen.findByTestId('manuscript-editor');

    fireEvent.click(screen.getByTestId('ms-change-structure'));
    fireEvent.click(await screen.findByTestId('sp-pick-ieee'));
    await screen.findByTestId('ms-nav');

    expect(screen.getByTestId('ms-save-state').getAttribute('data-state')).toBe('unsaved');
    fireEvent.click(screen.getByTestId('ms-close'));
    expect(await screen.findByTestId('ms-discard')).toBeTruthy();
  });
});
