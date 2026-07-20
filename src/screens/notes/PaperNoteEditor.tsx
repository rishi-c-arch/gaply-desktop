// Gaply — the per-paper structured note editor (Set 4), "Academic Focus"
// redesign ported from the Stitch paper_note_editor_desktop screen. The GUIDED
// 8-field template (all OPTIONAL / free-form) that encourages the researcher's
// OWN words: key_findings + my_evaluation are framed "in your own words"; exact
// copied text goes ONLY in the distinct, page-numbered notable_quotes field.
// NOTHING auto-summarizes the paper: no model, no "generate" button — the
// mock's "Scholar Assistant" AI bubble is deliberately NOT ported ("Gaply never
// writes your notes for you"). Same props/testids/behavior as before.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import MarkdownRenderer from '../../components/MarkdownRenderer';
import { NoteDraft, PaperNoteFields, parseFields, Note } from './notesBridge';
import { noteToMarkdown, exportFileName, ExportableNote } from './noteExport';
import { saveNoteFile } from './saveNoteFile';
import { IcBack, IcEye, IcExport, IcTrash, IcSave, IcAdd, IcEditNote } from './NotesIcons';
import FontScale from './FontScale';
import './notes.css';

/** Honest reading time from the actual export text (~200 wpm, ceil). */
export const readingMinutes = (text: string): number =>
  Math.max(1, Math.ceil(text.split(/\s+/).filter(Boolean).length / 200));

export interface PaperNoteEditorProps {
  /** The note being created/edited — paper_id + paper_title preset by the picker. */
  base: { id: string; paper_id: string | null; paper_title: string };
  /** When editing an existing note, its stored row (to prefill). */
  existing?: Note | null;
  /** OPTIONAL paper full text for side-by-side reading (graceful — absent = editor only). */
  paperText?: string | null;
  /** True when the picked paper's badge promised full text (M2 Set 1). When it's
   *  true but `paperText` is null (case/format mismatch, or an ambiguous
   *  title-collision the backend refused to guess), show an HONEST "not
   *  available" note instead of silently dropping the side-by-side. */
  fullTextExpected?: boolean;
  onSave: (draft: NoteDraft) => void;
  onDelete?: () => void;
  onClose: () => void;
  busy?: boolean;
  /** Surface a genuine export write-failure (null clears). Cancel stays silent. */
  onError?: (msg: string | null) => void;
}

type Quote = { text: string; page: string };

const PaperNoteEditor: React.FC<PaperNoteEditorProps> = ({ base, existing, paperText, fullTextExpected, onSave, onDelete, onClose, busy, onError }) => {
  const initial = useMemo<PaperNoteFields>(() => (existing ? parseFields(existing) : {}), [existing]);

  const [title, setTitle] = useState(existing?.title ?? '');
  const [citation, setCitation] = useState(initial.citation ?? '');
  const [researchQuestion, setResearchQuestion] = useState(initial.research_question ?? '');
  const [methodology, setMethodology] = useState(initial.methodology ?? '');
  const [keyFindings, setKeyFindings] = useState(initial.key_findings ?? '');
  const [limitations, setLimitations] = useState(initial.limitations ?? '');
  const [myEvaluation, setMyEvaluation] = useState(initial.my_evaluation ?? '');
  const [tags, setTags] = useState((existing?.tags ?? []).join(', '));
  const [quotes, setQuotes] = useState<Quote[]>(
    (initial.notable_quotes ?? []).map((q) => ({ text: q.text ?? '', page: q.page != null ? String(q.page) : '' }))
  );
  const [preview, setPreview] = useState(false);

  // Two-step delete: first click ARMS for 3s, second click deletes.
  const [deleteArmed, setDeleteArmed] = useState(false);
  const disarmTimer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(disarmTimer.current), []);
  const onDeleteClick = () => {
    if (!deleteArmed) {
      setDeleteArmed(true);
      disarmTimer.current = window.setTimeout(() => setDeleteArmed(false), 3000);
    } else {
      window.clearTimeout(disarmTimer.current);
      onDelete?.();
    }
  };

  // Assemble the current field values into the template shape (skip-empties).
  // Shared by save() and export so the two never drift.
  const currentFields = (): PaperNoteFields => {
    const trimmedQuotes = quotes
      .map((q) => ({ text: q.text.trim(), page: q.page.trim() }))
      .filter((q) => q.text !== '')
      .map((q) => ({ text: q.text, page: q.page === '' ? undefined : (Number.isNaN(Number(q.page)) ? q.page : Number(q.page)) }));

    const fields: PaperNoteFields = {};
    if (citation.trim()) fields.citation = citation.trim();
    if (researchQuestion.trim()) fields.research_question = researchQuestion.trim();
    if (methodology.trim()) fields.methodology = methodology.trim();
    if (keyFindings.trim()) fields.key_findings = keyFindings.trim();
    if (limitations.trim()) fields.limitations = limitations.trim();
    if (myEvaluation.trim()) fields.my_evaluation = myEvaluation.trim();
    if (trimmedQuotes.length) fields.notable_quotes = trimmedQuotes;
    return fields;
  };

  const splitTags = () => tags.split(',').map((t) => t.trim()).filter(Boolean);

  // The exact ExportableNote both export AND preview use — one source, no drift.
  const buildExportable = (): ExportableNote => ({
    note_type: 'paper',
    title: title.trim(),
    paper_title: base.paper_title,
    body: '',
    tags: splitTags(),
  });

  const save = () => {
    onSave({
      id: base.id,
      note_type: 'paper',
      paper_id: base.paper_id,
      paper_title: base.paper_title,
      title: title.trim(),
      fields: currentFields(),
      tags: splitTags(),
    });
  };

  // Export the current note as structured Markdown (heading per filled field).
  const exportMd = async () => {
    onError?.(null); // clear any prior export error
    try {
      const md = noteToMarkdown(buildExportable(), currentFields());
      // saveNoteFile returns null on cancel (silent) and only THROWS on a real
      // write failure — so only genuine failures reach this catch.
      await saveNoteFile(exportFileName(title || base.paper_title, 'paper-note'), md);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : 'Could not save the export');
    }
  };

  const editor = (
    <div className="an-canvas" data-testid="paper-note-editor">
      {/* Context line: which paper these reading notes anchor to */}
      <div className="an-crumbs-path" style={{ marginBottom: -16 }}>
        <span>Reading notes on</span>
        <strong data-testid="editor-paper-title">{base.paper_title || 'Untitled paper'}</strong>
        {!base.paper_id && <span>· free-typed paper</span>}
      </div>
      {/* 1. Note title */}
      <section>
        <label className="an-label">Note Title</label>
        <input className="an-title-input" value={title} placeholder="Enter the focus of this paper note…" data-testid="note-title" onChange={(e) => setTitle(e.target.value)} />
      </section>

      {/* 2. Citation */}
      <section>
        <label className="an-label">Citation (APA/MLA)</label>
        <input className="an-cite-input" value={citation} placeholder="Add source details…" data-testid="field-citation" onChange={(e) => setCitation(e.target.value)} />
      </section>

      {/* 3+4. Research question / methodology */}
      <div className="an-two-col">
        <section>
          <label className="an-label">Research Question</label>
          <textarea className="an-field" rows={3} value={researchQuestion} placeholder="What is this paper trying to answer?" data-testid="field-research_question" onChange={(e) => setResearchQuestion(e.target.value)} />
        </section>
        <section>
          <label className="an-label">Methodology</label>
          <textarea className="an-field" rows={3} value={methodology} placeholder="How was the research conducted?" data-testid="field-methodology" onChange={(e) => setMethodology(e.target.value)} />
        </section>
      </div>

      {/* 5. Key findings — in your own words */}
      <section>
        {/* Sentence case in the DOM (the design's uppercase comes from CSS). */}
        <label className="an-label">Key findings — in your own words</label>
        <textarea className="an-field" rows={5} value={keyFindings} placeholder="Summarize the main findings in your own words…" data-testid="field-key_findings" onChange={(e) => setKeyFindings(e.target.value)} />
        <p className="an-disclaimer">Process it, don’t copy it — writing findings in your words is how they stick.</p>
      </section>

      {/* 6. Notable quotes — exact copied text lives ONLY here, page-numbered */}
      <section data-testid="field-notable_quotes">
        <label className="an-label">Notable Quotes</label>
        {quotes.map((q, i) => (
          <div key={i} className="an-quote-block" style={{ marginBottom: 12 }} data-testid={`quote-row-${i}`}>
            <input className="an-quote-text" value={q.text} placeholder="“exact quoted text”" data-testid={`quote-text-${i}`}
              onChange={(e) => setQuotes((qs) => qs.map((x, j) => (j === i ? { ...x, text: e.target.value } : x)))} />
            <div className="an-quote-meta">
              <span>— Page</span>
              <input className="an-quote-page" value={q.page} placeholder="…" data-testid={`quote-page-${i}`}
                onChange={(e) => setQuotes((qs) => qs.map((x, j) => (j === i ? { ...x, page: e.target.value } : x)))} />
              <button className="an-quote-x" data-testid={`quote-remove-${i}`} onClick={() => setQuotes((qs) => qs.filter((_, j) => j !== i))}>✕ remove</button>
            </div>
          </div>
        ))}
        <button className="an-addquote" data-testid="quote-add" onClick={() => setQuotes((qs) => [...qs, { text: '', page: '' }])}>
          <IcAdd size={18} /> Add Quote
        </button>
        <p className="an-disclaimer">The one place for exact wording — always with a page number, so a quote is deliberate and citable.</p>
      </section>

      {/* 7. Limitations */}
      <section>
        <label className="an-label">Limitations</label>
        <textarea className="an-field" rows={3} value={limitations} placeholder="What are the gaps or weaknesses?" data-testid="field-limitations" onChange={(e) => setLimitations(e.target.value)} />
      </section>

      {/* 8. Personal evaluation — the Critical Reflection block */}
      <section>
        <label className="an-label">Personal Evaluation</label>
        <div className="an-reflect">
          <h3>Critical Reflection</h3>
          <textarea rows={3} value={myEvaluation} placeholder="What do you think? Strengths, weaknesses, how it fits your work…" data-testid="field-my_evaluation" onChange={(e) => setMyEvaluation(e.target.value)} />
        </div>
        <p className="an-disclaimer">My evaluation — in your own words: the part only you can write.</p>
      </section>

      {/* Footer — honest meta (no fake history/collaborators) + tags */}
      <div className="an-edit-foot">
        <div className="an-foot-meta"><span>Saved locally · on device</span></div>
        <div className="an-tagpills">
          {splitTags().map((t) => <span key={t} className="an-pill">#{t}</span>)}
          <input className="an-tags-input" value={tags} placeholder="comma, separated, tags" data-testid="note-tags" onChange={(e) => setTags(e.target.value)} />
        </div>
      </div>
      <p className="an-disclaimer" data-testid="own-words-note">
        Every field is optional. Gaply never writes your notes for you — no summaries, no autofill. These are your words.
      </p>
    </div>
  );

  return (
    <div>
      {/* Header / toolbar (mock: Paper Editor top bar) */}
      <header className="an-edit-head">
        <div className="an-edit-head-left">
          <button className="an-backbtn" onClick={onClose} data-testid="note-close" title="Back to your library"><IcBack /></button>
          <h1>Paper Editor</h1>
        </div>
        <div className="an-edit-actions">
          <FontScale />
          <button className="an-ghostbtn" onClick={() => setPreview((p) => !p)} data-testid="note-preview-toggle">
            <IcEye /> {preview ? 'Edit' : 'Preview'}
          </button>
          <button className="an-ghostbtn" onClick={() => void exportMd()} disabled={busy} data-testid="note-export">
            <IcExport /> Export
          </button>
          <div className="an-divider" />
          {existing && onDelete && (
            <button
              className={`an-deletebtn${deleteArmed ? ' an-deletebtn--armed' : ''}`}
              onClick={onDeleteClick}
              data-testid="note-delete"
              title={deleteArmed ? 'Click again to delete' : 'Delete note'}
            >
              {deleteArmed ? 'Really delete?' : <IcTrash />}
            </button>
          )}
          <button className="an-savebtn" onClick={save} disabled={busy} data-testid="note-save">
            <IcSave /> {busy ? 'Saving…' : existing ? 'Save Changes' : 'Save Note'}
          </button>
        </div>
      </header>

      <div className={`an-edit-wrap${paperText ? ' an-edit-wrap--wide' : ''}`}>
        {/* Breadcrumbs + honest storage status */}
        <div className="an-crumbs">
          <div className="an-crumbs-path">
            <span>My Library</span><span>›</span>
            <strong>{base.paper_title || 'Untitled paper'}</strong>
          </div>
          <div className="an-synced">Saved locally · on device</div>
        </div>

        {fullTextExpected && !paperText && (
          <p className="an-hint" data-testid="paper-fulltext-unavailable" style={{ marginBottom: 16 }}>
            Full text isn’t available for this paper in your plagiarism library — showing your
            notes only. (It matches by title, so a differently-named or duplicate entry won’t link.)
          </p>
        )}

        {preview ? (
          <>
            <div className="an-preview-card" data-testid="paper-preview">
              <MarkdownRenderer content={noteToMarkdown(buildExportable(), currentFields())} />
              {/* Meta footer (mock: LAST EDITED / READING TIME / tags) — honest values */}
              <div className="an-preview-foot">
                <div className="an-preview-meta">
                  {existing && existing.updated_at > 1e9 && (
                    <div><b>Last edited</b><span>{new Date(existing.updated_at * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })}</span></div>
                  )}
                  <div><b>Reading time</b><span>{readingMinutes(noteToMarkdown(buildExportable(), currentFields()))} min</span></div>
                </div>
                <div className="an-tagpills">{splitTags().map((t) => <span key={t} className="an-pill">#{t}</span>)}</div>
              </div>
            </div>
            {/* Floating dock (mock: note_preview_desktop) */}
            <div className="an-dock">
              <button className="an-dock-edit" data-testid="note-preview-edit-pill" onClick={() => setPreview(false)}><IcEditNote size={18} /> Edit Note</button>
              <div className="an-dock-divider" />
              <button className="an-dock-icon" data-testid="note-preview-export" title="Export (.md)" onClick={() => void exportMd()}><IcExport size={18} /></button>
            </div>
          </>
        ) : paperText ? (
          // Read & write view (mock: read_write_view_desktop) — the paper beside the notes.
          <div className="an-split" data-testid="note-split">
            <div className="an-paperpane" data-testid="paper-fulltext">
              <div className="an-paperpane-label">The paper</div>
              <pre>{paperText}</pre>
            </div>
            <div>{editor}</div>
          </div>
        ) : (
          editor
        )}
      </div>
    </div>
  );
};

export default PaperNoteEditor;
