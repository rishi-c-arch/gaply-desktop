// Set 3 — the Note Creator bridge. Round-trips the typed adapters against the
// stateful mock (wire-exact to the Rust notes store): create → list → get →
// update → search → delete, both note types, filters, tags. The mock mirrors
// the Rust CRUD semantics so the Sets 4/5 UI tests need no backend.
import { describe, expect, it } from 'vitest';
import { makeMockNotesBridge, parseFields, NoteDraft } from './notesBridge';

const paperDraft: NoteDraft = {
  id: 'p1',
  note_type: 'paper',
  paper_id: 'cite-watson',
  paper_title: 'Molecular Structure of Nucleic Acids',
  title: 'Watson & Crick — my notes',
  fields: {
    citation: 'Watson & Crick, 1953',
    research_question: 'What is the structure of DNA?',
    key_findings: 'Double helix, antiparallel strands',
    notable_quotes: [{ text: 'This structure has novel features', page: 737 }],
    my_evaluation: 'Foundational',
  },
  tags: ['dna', 'toread'],
};

const projectDraft: NoteDraft = {
  id: 'j1',
  note_type: 'project',
  title: 'Thesis idea',
  body: 'Hypothesis: sleep extension improves recall. Ask advisor Friday.',
  tags: ['idea'],
};

describe('NotesBridge (mock) — CRUD round-trip, both note types', () => {
  it('create → list → get for a paper note and a project note', async () => {
    const b = makeMockNotesBridge();
    const p = await b.create(paperDraft);
    const j = await b.create(projectDraft);

    // wire-exact shapes
    expect(p.note_type).toBe('paper');
    expect(p.paper_id).toBe('cite-watson');
    expect(p.sync_status).toBe('local_only');
    expect(j.note_type).toBe('project');
    expect(j.paper_id).toBeNull();

    // the template fields round-trip through fields_json (string on the wire)
    expect(typeof p.fields_json).toBe('string');
    expect(parseFields(p).research_question).toBe('What is the structure of DNA?');
    expect(parseFields(p).notable_quotes?.[0].page).toBe(737);
    // a project note has an empty template object
    expect(parseFields(j)).toEqual({});

    expect((await b.list()).length).toBe(2);
    expect((await b.get('p1'))?.title).toBe('Watson & Crick — my notes');
  });

  it('list filters by note_type and by paper_id', async () => {
    const b = makeMockNotesBridge();
    await b.create(paperDraft);
    await b.create(projectDraft);

    expect((await b.list('paper')).length).toBe(1);
    expect((await b.list('project')).length).toBe(1);
    expect((await b.list(undefined, 'cite-watson')).length).toBe(1);
    expect((await b.list(undefined, 'nobody')).length).toBe(0);
  });

  it('update must exist, bumps ordering, edits fields', async () => {
    const b = makeMockNotesBridge();
    await b.create(paperDraft);
    await b.create(projectDraft);

    const edited = await b.update({ ...paperDraft, title: 'Watson & Crick — revised' });
    expect(edited.title).toBe('Watson & Crick — revised');
    // updated → now newest
    expect((await b.list())[0].id).toBe('p1');
    // updating a non-existent note throws (mirrors update_note)
    await expect(b.update({ ...projectDraft, id: 'ghost' })).rejects.toThrow(/not found/);
  });

  it('search over title/body/fields/paper_title/tags + tag filter', async () => {
    const b = makeMockNotesBridge();
    await b.create(paperDraft);
    await b.create(projectDraft);

    expect((await b.search('helix')).length).toBe(1); // fields_json
    expect((await b.search('advisor')).length).toBe(1); // body
    expect((await b.search('nucleic')).length).toBe(1); // paper_title
    expect((await b.search('', 'idea')).length).toBe(1); // tag filter
    expect((await b.search('zebrafish')).length).toBe(0);
  });

  it('setTags resets sync_status; delete removes only the note', async () => {
    const b = makeMockNotesBridge();
    await b.create(paperDraft);
    await b.create(projectDraft);

    const tagged = await b.setTags('j1', ['idea', 'priority']);
    expect(tagged.tags).toEqual(['idea', 'priority']);
    expect(tagged.sync_status).toBe('local_only');

    await b.remove('j1');
    expect(await b.get('j1')).toBeNull();
    expect((await b.list()).length).toBe(1); // the paper note is untouched
  });

  it('created_at is preserved across updates, updated_at advances', async () => {
    const b = makeMockNotesBridge();
    const first = await b.create(projectDraft);
    const second = await b.update({ ...projectDraft, body: 'edited' });
    expect(second.created_at).toBe(first.created_at);
    expect(second.updated_at).toBeGreaterThan(first.updated_at);
  });
});
