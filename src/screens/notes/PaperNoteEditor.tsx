// Gaply — the per-paper structured note editor (Set 4). A GUIDED 8-field
// template (all OPTIONAL / free-form) that encourages the researcher's OWN
// words: key_findings + my_evaluation are framed "in your own words"; exact
// copied text goes ONLY in the distinct, page-numbered notable_quotes field, so
// copying is deliberate and citable — never the default. NOTHING auto-summarizes
// the paper: there is no model and NO "generate my notes" button anywhere. The
// thinking stays the researcher's.
import React, { useMemo, useState } from 'react';
import { Badge, Button, Card } from '../../design-system';
import { NoteDraft, PaperNoteFields, parseFields, Note } from './notesBridge';

export interface PaperNoteEditorProps {
  /** The note being created/edited — paper_id + paper_title preset by the picker. */
  base: { id: string; paper_id: string | null; paper_title: string };
  /** When editing an existing note, its stored row (to prefill). */
  existing?: Note | null;
  /** OPTIONAL paper full text for side-by-side reading (graceful — absent = editor only). */
  paperText?: string | null;
  onSave: (draft: NoteDraft) => void;
  onDelete?: () => void;
  onClose: () => void;
  busy?: boolean;
}

type Quote = { text: string; page: string };

const PaperNoteEditor: React.FC<PaperNoteEditorProps> = ({ base, existing, paperText, onSave, onDelete, onClose, busy }) => {
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

  const save = () => {
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

    onSave({
      id: base.id,
      note_type: 'paper',
      paper_id: base.paper_id,
      paper_title: base.paper_title,
      title: title.trim(),
      fields,
      tags: tags.split(',').map((t) => t.trim()).filter(Boolean),
    });
  };

  const field = (label: string, node: React.ReactNode, hint?: string) => (
    <label style={{ display: 'grid', gap: 4 }}>
      <span style={{ fontWeight: 600, fontSize: 13 }}>{label}</span>
      {hint && <span className="gds-jc__disclaimer" style={{ margin: 0 }}>{hint}</span>}
      {node}
    </label>
  );
  const area = (v: string, set: (s: string) => void, placeholder: string, testid: string) => (
    <textarea className="gds-note__area" rows={3} value={v} placeholder={placeholder} data-testid={testid} onChange={(e) => set(e.target.value)} />
  );

  const editor = (
    <div style={{ display: 'grid', gap: 14 }} data-testid="paper-note-editor">
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <Badge status="certain">🟢 your reading notes</Badge>
        <strong data-testid="editor-paper-title">{base.paper_title || 'Untitled paper'}</strong>
        {!base.paper_id && <Badge status="neutral">free-typed paper</Badge>}
      </div>

      {field('Note title', <input className="gds-jc__input" value={title} placeholder="e.g. My notes on this paper" data-testid="note-title" onChange={(e) => setTitle(e.target.value)} />)}
      {field('Citation', <input className="gds-jc__input" value={citation} placeholder="How you'd cite this paper" data-testid="field-citation" onChange={(e) => setCitation(e.target.value)} />)}
      {field('Research question', area(researchQuestion, setResearchQuestion, 'What question does the paper ask?', 'field-research_question'))}
      {field('Methodology', area(methodology, setMethodology, 'Design, sample, methods…', 'field-methodology'))}

      {field(
        'Key findings — in your own words',
        area(keyFindings, setKeyFindings, 'Summarize the main findings in your own words…', 'field-key_findings'),
        'Process it, don’t copy it — writing findings in your words is how they stick.'
      )}

      {/* Exact copied text lives ONLY here — distinct + page-numbered so it's deliberate and citable. */}
      <div style={{ display: 'grid', gap: 6 }} data-testid="field-notable_quotes">
        <span style={{ fontWeight: 600, fontSize: 13 }}>Notable quotes (exact text + page)</span>
        <span className="gds-jc__disclaimer" style={{ margin: 0 }}>
          The one place for exact wording — always with a page number, so a quote is deliberate and citable.
        </span>
        {quotes.map((q, i) => (
          <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 80px auto', gap: 6 }} data-testid={`quote-row-${i}`}>
            <input className="gds-jc__input" value={q.text} placeholder="“exact quoted text”" data-testid={`quote-text-${i}`}
              onChange={(e) => setQuotes((qs) => qs.map((x, j) => (j === i ? { ...x, text: e.target.value } : x)))} />
            <input className="gds-jc__input" value={q.page} placeholder="p." data-testid={`quote-page-${i}`}
              onChange={(e) => setQuotes((qs) => qs.map((x, j) => (j === i ? { ...x, page: e.target.value } : x)))} />
            <button className="gds-note__quote-x" data-testid={`quote-remove-${i}`} onClick={() => setQuotes((qs) => qs.filter((_, j) => j !== i))}>✕</button>
          </div>
        ))}
        <Button variant="secondary" data-testid="quote-add" onClick={() => setQuotes((qs) => [...qs, { text: '', page: '' }])}>+ Add a quote</Button>
      </div>

      {field('Limitations', area(limitations, setLimitations, 'Weaknesses, threats to validity, scope…', 'field-limitations'))}
      {field(
        'My evaluation — in your own words',
        area(myEvaluation, setMyEvaluation, 'What do you think? Strengths, weaknesses, how it fits your work…', 'field-my_evaluation'),
        'Your judgement — the part only you can write.'
      )}
      {field('Tags', <input className="gds-jc__input" value={tags} placeholder="comma, separated, tags" data-testid="note-tags" onChange={(e) => setTags(e.target.value)} />)}

      <div style={{ display: 'flex', gap: 8 }}>
        <Button onClick={save} disabled={busy} data-testid="note-save">{busy ? 'Saving…' : existing ? 'Save changes' : 'Save note'}</Button>
        <Button variant="secondary" onClick={onClose} data-testid="note-close">Close</Button>
        {existing && onDelete && (
          <Button variant="secondary" onClick={onDelete} data-testid="note-delete" style={{ marginLeft: 'auto', color: 'var(--g-flagged)' }}>Delete</Button>
        )}
      </div>
      <p className="gds-jc__disclaimer" data-testid="own-words-note">
        Every field is optional. Gaply never writes your notes for you — no summaries, no autofill. These are your words.
      </p>
    </div>
  );

  // Optional side-by-side: the paper's full text (when available) beside the editor.
  return (
    <Card title="Reading notes">
      {paperText ? (
        <div className="gds-note__split" data-testid="note-split">
          <div className="gds-note__paper" data-testid="paper-fulltext">
            <div className="gds-note__paper-label">The paper</div>
            <pre className="gds-note__paper-text">{paperText}</pre>
          </div>
          <div className="gds-note__editor-pane">{editor}</div>
        </div>
      ) : (
        editor
      )}
    </Card>
  );
};

export default PaperNoteEditor;
