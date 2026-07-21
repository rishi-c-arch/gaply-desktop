// Note Creator export — the PURE Markdown emitter. String assertions only:
// no model, no network, no file I/O. Pins: skip-empties, structure preserved,
// quotes with/without page, bulk joined by ---.
import { describe, expect, it } from 'vitest';
import { noteToMarkdown, notesToMarkdown, exportFileName, bulkExportFileName, honestImagePlaceholders, ExportableNote } from './noteExport';
import { Note, PaperNoteFields } from './notesBridge';

const project = (over: Partial<ExportableNote> = {}): ExportableNote => ({
  note_type: 'project', title: '', paper_title: '', body: '', tags: [], ...over,
});
const paper = (over: Partial<ExportableNote> = {}): ExportableNote => ({
  note_type: 'paper', title: '', paper_title: '', body: '', tags: [], ...over,
});

describe('noteToMarkdown — project note', () => {
  it('title + body + tags footer, exact shape', () => {
    const md = noteToMarkdown(project({ title: 'Idea', body: 'Reorder §1.', tags: ['writing', 'intro'] }), {});
    expect(md).toBe('# Idea\n\nReorder §1.\n\n_Tags: writing, intro_');
  });
  it('no tags → no footer line', () => {
    const md = noteToMarkdown(project({ title: 'Idea', body: 'x' }), {});
    expect(md).not.toMatch(/_Tags/);
    expect(md).toBe('# Idea\n\nx');
  });
});

describe('noteToMarkdown — paper note (structure preserved, skip-empties)', () => {
  const full: PaperNoteFields = {
    citation: 'Smith 2020',
    research_question: 'Can attention alone match recurrence?',
    methodology: 'Transformer; WMT14; BLEU',
    key_findings: 'Beats RNN baselines',
    limitations: 'Quadratic cost',
    my_evaluation: 'Relevant to §3',
    notable_quotes: [{ text: 'more parallelization', page: 2 }, { text: 'a new architecture' }],
  };

  it('STRUCTURE: one heading per field, in editor order, with the paper header — NOT a flat blob', () => {
    const md = noteToMarkdown(paper({ title: 'My notes', paper_title: 'Attention', tags: [] }), full);
    expect(md).toContain('# My notes');
    expect(md).toContain('**Paper:** Attention');
    const order = ['## Citation', '## Research question', '## Methodology', '## Key findings', '## Notable quotes', '## Limitations', '## Evaluation'];
    const idxs = order.map((h) => md.indexOf(h));
    expect(idxs.every((i) => i >= 0)).toBe(true); // every heading present
    expect(idxs).toEqual([...idxs].sort((a, b) => a - b)); // strictly increasing = correct order
    expect(md).toMatch(/## Methodology\nTransformer; WMT14; BLEU/); // real heading + value, not flattened
  });

  it('QUOTES: blockquote with (p. N) when a page exists, omitted when not', () => {
    const md = noteToMarkdown(paper({ paper_title: 'X' }), full);
    expect(md).toContain('> more parallelization (p. 2)');
    expect(md).toContain('> a new architecture');
    expect(md).not.toContain('> a new architecture (p');
  });

  it('SKIP-EMPTIES: a note with 3 of 8 fields exports ONLY those 3 headings — no empty stubs', () => {
    const partial: PaperNoteFields = { citation: 'C', key_findings: 'KF', my_evaluation: 'E' };
    const md = noteToMarkdown(paper({ title: 'T', paper_title: 'P' }), partial);
    for (const h of ['## Citation', '## Key findings', '## Evaluation']) expect(md).toContain(h);
    for (const h of ['## Research question', '## Methodology', '## Notable quotes', '## Limitations']) {
      expect(md).not.toContain(h); // never padded with blank stubs
    }
  });

  it('empty note title → H1 falls back to the paper title, no duplicate **Paper:** line', () => {
    const md = noteToMarkdown(paper({ title: '', paper_title: 'Attention' }), { citation: 'C' });
    expect(md).toContain('# Attention');
    expect(md).not.toContain('**Paper:**');
  });

  it('notable_quotes with blank text are dropped', () => {
    const md = noteToMarkdown(paper({ paper_title: 'X' }), { notable_quotes: [{ text: '', page: 1 }, { text: 'keep' }] });
    expect(md).toContain('> keep');
    expect(md).not.toMatch(/> \s*\(p\. 1\)/);
  });

  it('paper note with only a title exports just the header (+ tags) — no empty sections', () => {
    const md = noteToMarkdown(paper({ title: 'Bare', paper_title: 'P', tags: ['t'] }), {});
    expect(md).toBe('# Bare\n\n**Paper:** P\n\n_Tags: t_');
  });
});

describe('export honesty for pasted images (Set 3, pin 5)', () => {
  it('a project body with an image ref exports an honest labelled placeholder (never a broken <img>)', () => {
    const md = noteToMarkdown(project({ title: 'Field notes', body: 'Saw this:\n\n![](gaply-image://abc123.png)\n\nInteresting.' }), {});
    expect(md).toBe('# Field notes\n\nSaw this:\n\n![image stored in Gaply](gaply-image://abc123.png)\n\nInteresting.');
  });
  it('honestImagePlaceholders relabels the alt but keeps the ref as a breadcrumb', () => {
    expect(honestImagePlaceholders('![](gaply-image://h.png)')).toBe('![image stored in Gaply](gaply-image://h.png)');
    expect(honestImagePlaceholders('![old alt](gaply-image://h.jpg)')).toBe('![image stored in Gaply](gaply-image://h.jpg)');
  });
  it('leaves ordinary (non-gaply) images and image-free text untouched', () => {
    expect(honestImagePlaceholders('![diagram](https://ex.com/a.png)')).toBe('![diagram](https://ex.com/a.png)');
    expect(honestImagePlaceholders('plain text, no images')).toBe('plain text, no images');
  });
});

describe('notesToMarkdown — bulk', () => {
  const note = (over: Partial<Note>): Note => ({
    id: 'x', note_type: 'project', paper_id: null, paper_title: '', title: 'T', fields_json: '{}',
    body: '', tags: [], sync_status: 'local_only', created_at: 1, updated_at: 1, ...over,
  });

  it('joins notes with a --- rule, in order', () => {
    const md = notesToMarkdown([note({ id: 'a', title: 'A', body: 'aa' }), note({ id: 'b', title: 'B', body: 'bb' })]);
    expect(md).toBe('# A\n\naa\n\n---\n\n# B\n\nbb');
  });
  it('reads paper fields from fields_json (parses each note)', () => {
    const p = note({ id: 'p', note_type: 'paper', title: 'Paper note', paper_title: 'Pap', fields_json: '{"methodology":"RCT"}' });
    const md = notesToMarkdown([p]);
    expect(md).toContain('## Methodology\nRCT');
  });
  it('empty list → empty string', () => {
    expect(notesToMarkdown([])).toBe('');
  });
});

describe('exportFileName', () => {
  it('slugs the title, adds .md, falls back when empty', () => {
    expect(exportFileName('My Notes on X!')).toBe('my-notes-on-x.md');
    expect(exportFileName('   ', 'paper-note')).toBe('paper-note.md');
    expect(exportFileName('')).toBe('note.md');
  });
});

describe('bulkExportFileName — encodes the full active filter (Phase 4a)', () => {
  it('no filters → plain notes.md', () => {
    expect(bulkExportFileName('all', null, '')).toBe('notes.md');
    expect(bulkExportFileName('all', null, '   ')).toBe('notes.md');
  });
  it('type-only / tag-only / search-only', () => {
    expect(bulkExportFileName('paper', null, '')).toBe('notes-paper.md');
    expect(bulkExportFileName('manuscript', null, '')).toBe('notes-manuscript.md');
    expect(bulkExportFileName('all', 'writing', '')).toBe('notes-writing.md');
    expect(bulkExportFileName('all', null, 'sleep recall')).toBe('notes-sleep-recall.md');
  });
  it('combinations encode type + tag + search in order', () => {
    expect(bulkExportFileName('manuscript', 'thesis', 'attention')).toBe('notes-manuscript-thesis-attention.md');
    expect(bulkExportFileName('project', 'ideas', '')).toBe('notes-project-ideas.md');
  });
  it('a search query with ILLEGAL filename chars is slugged (always a valid name)', () => {
    const name = bulkExportFileName('all', null, 'a/b: c*d?<e>');
    expect(name).toBe('notes-a-b-c-d-e.md');
    expect(name).not.toMatch(/[/\\:*?"<>|]/); // no illegal filename characters
  });
  it('a tag with special chars is also slugged', () => {
    expect(bulkExportFileName('paper', 'C++ / ML', 'x')).toBe('notes-paper-c-ml-x.md');
  });
});
