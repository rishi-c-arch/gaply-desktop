// Note preview — proves preview == export. The preview feeds the SAME
// noteToMarkdown output into MarkdownRenderer, so a note's markdown must render
// as its elements with no raw marker leaking; and the editor toggle is
// read-only (edit ⇄ preview). No models, no network.
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import MarkdownRenderer from '../../components/MarkdownRenderer';
import { noteToMarkdown, ExportableNote } from './noteExport';
import { PaperNoteFields, Note } from './notesBridge';
import ProjectNoteEditor from './ProjectNoteEditor';
import PaperNoteEditor from './PaperNoteEditor';

afterEach(cleanup);

/* ---- The anti-drift pin: same note → noteToMarkdown → MarkdownRenderer ---- */
describe('preview == export — every construct renders, no raw marker leaks', () => {
  it('paper note: h1/h2/blockquote/strong/em all present; skip-empties holds; nothing literal', () => {
    const note: ExportableNote = {
      note_type: 'paper', title: 'My notes', paper_title: 'Attention', body: '', tags: ['transformers', 'methods'],
    };
    const fields: PaperNoteFields = {
      citation: 'Vaswani 2017',
      methodology: 'Transformer; WMT14',
      key_findings: 'Beats RNN baselines',
      notable_quotes: [{ text: 'more parallelization', page: 2 }],
      // research_question, limitations, my_evaluation intentionally EMPTY
    };
    const md = noteToMarkdown(note, fields); // the EXACT string export produces
    const { container } = render(<MarkdownRenderer content={md} />);

    // each construct → its element
    expect(container.querySelector('.md-h1')?.textContent).toBe('My notes');
    const h2s = Array.from(container.querySelectorAll('.md-h2')).map((h) => h.textContent);
    expect(h2s).toEqual(expect.arrayContaining(['Citation', 'Methodology', 'Key findings', 'Notable quotes']));
    expect(container.querySelector('blockquote')?.textContent).toContain('more parallelization (p. 2)');
    expect(container.querySelector('strong')?.textContent).toBe('Paper:');   // **Paper:**
    expect(container.querySelector('em')?.textContent).toMatch(/^Tags:/);      // _Tags: …_

    // SKIP-EMPTIES — no heading for an unfilled field
    expect(h2s).not.toContain('Research question');
    expect(h2s).not.toContain('Limitations');
    expect(h2s).not.toContain('Evaluation');

    // ANTI-DRIFT — no unrendered markdown markers survive as literal text
    const t = container.textContent ?? '';
    expect(t).not.toContain('## ');   // headings consumed
    expect(t).not.toContain('> ');    // blockquote marker consumed
    expect(t).not.toContain('**');    // bold marker consumed
    expect(t).not.toContain('_');     // underscore emphasis consumed (no snake_case in this note)
  });

  it('project note: body markdown renders (heading + bold + boundary italic)', () => {
    const note: ExportableNote = {
      note_type: 'project', title: 'Idea', paper_title: '', body: '# Sub\n**bold** and a _boundary_ word', tags: [],
    };
    const { container } = render(<MarkdownRenderer content={noteToMarkdown(note, {})} />);
    expect(container.querySelector('.md-h1')?.textContent).toBe('Idea');
    expect(container.textContent).toContain('Sub');
    expect(container.querySelector('strong')?.textContent).toBe('bold');
    expect(Array.from(container.querySelectorAll('em')).some((e) => e.textContent === 'boundary')).toBe(true);
  });
});

/* ------------------ The editor toggle (read-only, edit ⇄ preview) --------- */
describe('editor preview toggle', () => {
  const projectNote: Note = {
    id: 'j1', note_type: 'project', paper_id: null, paper_title: '', title: 'Thesis idea',
    fields_json: '{}', body: '# Big idea\nSleep helps recall.', tags: ['idea'],
    sync_status: 'local_only', created_at: 1, updated_at: 1,
  };

  it('project: Preview shows the rendered note; Edit restores the inputs (read-only)', () => {
    render(<ProjectNoteEditor id="j1" existing={projectNote} onSave={() => {}} onClose={() => {}} busy={false} />);
    expect(screen.getByTestId('project-body')).toBeTruthy(); // starts in edit

    fireEvent.click(screen.getByTestId('project-preview-toggle'));
    const pv = screen.getByTestId('project-preview');
    expect(pv.querySelector('.md-h1')?.textContent).toBe('Thesis idea');
    expect(pv.textContent).toContain('Big idea'); // body '# Big idea' rendered as a heading
    expect(screen.queryByTestId('project-body')).toBeNull(); // inputs hidden — read-only

    fireEvent.click(screen.getByTestId('project-preview-toggle')); // back to edit
    expect(screen.getByTestId('project-body')).toBeTruthy();
    expect(screen.queryByTestId('project-preview')).toBeNull();
  });

  it('paper: Preview renders the assembled structure and honors skip-empties', () => {
    const base = { id: 'p1', paper_id: 'cite', paper_title: 'Attention' };
    const paperNote: Note = {
      id: 'p1', note_type: 'paper', paper_id: 'cite', paper_title: 'Attention', title: 'My notes',
      fields_json: JSON.stringify({ citation: 'Vaswani', methodology: 'Transformer' }), body: '', tags: [],
      sync_status: 'local_only', created_at: 1, updated_at: 1,
    };
    render(<PaperNoteEditor base={base} existing={paperNote} onSave={() => {}} onClose={() => {}} busy={false} />);

    fireEvent.click(screen.getByTestId('note-preview-toggle'));
    const pv = screen.getByTestId('paper-preview');
    const h2s = Array.from(pv.querySelectorAll('.md-h2')).map((h) => h.textContent);
    expect(h2s).toContain('Citation');
    expect(h2s).toContain('Methodology');
    expect(h2s).not.toContain('Limitations'); // skip-empties
    expect(screen.queryByTestId('field-citation')).toBeNull(); // fields hidden — read-only
  });
});
