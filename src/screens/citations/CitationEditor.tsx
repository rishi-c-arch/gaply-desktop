// Gaply — Citation Manager metadata editor (D1). The screen previously offered
// "Add manually" and an entry reading "Untitled — edit details", with nowhere
// to edit them: a manual entry stayed malformed forever and exported as a
// fabricated placeholder. This is the missing half.
//
// WHY A DIALOG AND NOT THE DETAIL PANE (decided 2026-08-26 — do NOT "simplify"
// this back into the pane). The right detail pane is `hidden xl:flex`, so it
// does not exist below 1280px, while BOTH entry points to addManual — the
// sidebar's New Citation CTA (md:) and the "+ Manual entry" link (always) — are
// reachable far below that. Editing in place would make manual entry work only
// on wide screens, replacing the trap rather than removing it. Two lesser
// reasons: 320px cannot hold nine fields plus a repeating authors list next to
// the preview, tags and status lines already there; and a dialog gives explicit
// Save/Cancel, so sqlite sees ONE write instead of a per-keystroke stream.
//
// STYLING SEAM (chosen, not inherited): this reuses the design-system Modal, so
// the editor wears the DS's visual language rather than the Archive restyle —
// the same seam notes/CitationPicker already accepts. That is deliberate. Two
// components sharing one language is consistency; one component with bespoke
// Archive overrides is drift, and the seam is a design-system question, not a
// Citation Manager one. The CSS below is LAYOUT ONLY (grid/gaps/sizing) and
// inherits colour from the modal chrome — it must not reach for --cm-* tokens.
//
// KNOWN, SEPARATE ITEM — NOT FIXED HERE: design-system/Modal.tsx has no focus
// trap (Escape and overlay-click close, focus is not confined). That is a
// pre-existing DS gap shared with CitationPicker; fixing it would change a
// shared primitive inside a diff that claims to be about manual entry, so it is
// recorded as its own item rather than folded in.
import React, { useEffect, useMemo, useState } from 'react';
import { Modal } from '../../design-system';
import { Citation, CslItem } from './citationTypes';
import { canFormatStyle, formatCitation } from './formatCitation';

/** The CSL types worth offering by hand. citeproc accepts many more; these are
 *  the ones a researcher actually hand-enters. */
const CSL_TYPES: Array<[string, string]> = [
  ['article-journal', 'Journal article'],
  ['book', 'Book'],
  ['chapter', 'Book chapter'],
  ['paper-conference', 'Conference paper'],
  ['thesis', 'Thesis'],
  ['report', 'Report'],
  ['webpage', 'Web page'],
];

type AuthorRow = { family: string; given: string };

export interface CitationEditorProps {
  open: boolean;
  citation: Citation | null;
  /** The current style id, so the preview reformats exactly as the card will. */
  style: string;
  onCancel: () => void;
  onSave: (next: Citation) => void;
}

/** Render *italic* / **bold** markers the same way the detail pane does, so the
 *  in-dialog preview and the pane preview cannot drift apart. */
const Marked: React.FC<{ text: string }> = ({ text }) => (
  <>
    {text.split(/(\*\*[^*]+\*\*|\*[^*]+\*)/g).filter(Boolean).map((p, i) => {
      if (p.startsWith('**')) return <strong key={i}>{p.slice(2, -2)}</strong>;
      if (p.startsWith('*')) return <em key={i}>{p.slice(1, -1)}</em>;
      return <React.Fragment key={i}>{p}</React.Fragment>;
    })}
  </>
);

export const CitationEditor: React.FC<CitationEditorProps> = ({ open, citation, style, onCancel, onSave }) => {
  const [type, setType] = useState('article-journal');
  const [title, setTitle] = useState('');
  const [authors, setAuthors] = useState<AuthorRow[]>([]);
  const [year, setYear] = useState('');
  const [journal, setJournal] = useState('');
  const [volume, setVolume] = useState('');
  const [issue, setIssue] = useState('');
  const [page, setPage] = useState('');
  const [doi, setDoi] = useState('');
  const [url, setUrl] = useState('');

  // Re-seed the draft whenever a DIFFERENT citation is opened. Keyed on id (not
  // the object) so a background re-render — a sweep landing, a sync status
  // changing — cannot wipe what the user is typing.
  const seedId = open ? citation?.id : null;
  useEffect(() => {
    if (!open || !citation) return;
    const m = citation.csl;
    setType(m.type || 'article-journal');
    setTitle(m.title ?? '');
    setAuthors((m.author ?? []).map((a) => ({ family: a.family ?? '', given: a.given ?? '' })));
    setYear(m.issued?.year != null ? String(m.issued.year) : '');
    setJournal(m.containerTitle ?? '');
    setVolume(m.volume ?? '');
    setIssue(m.issue ?? '');
    setPage(m.page ?? '');
    setDoi(citation.doi ?? m.DOI ?? '');
    setUrl(m.URL ?? '');
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [seedId, open]);

  // ABSENT STAYS ABSENT — the same rule the resolver and the exporters follow.
  // A field the user left blank is omitted from the CSL, never written as an
  // empty string, so computeStatus and citeproc both see a genuine gap rather
  // than a value that happens to be empty.
  const draft = useMemo((): CslItem => {
    const parsedYear = Number.parseInt(year, 10);
    const cleaned = authors
      .map((a) => ({ family: a.family.trim(), given: a.given.trim() }))
      .filter((a) => a.family || a.given);
    return {
      id: citation?.csl.id || citation?.id || 'draft',
      type: type || 'article-journal',
      title: title.trim(),
      author: cleaned.map((a) => ({ family: a.family, ...(a.given ? { given: a.given } : {}) })),
      ...(Number.isFinite(parsedYear) && year.trim() ? { issued: { year: parsedYear } } : {}),
      ...(doi.trim() ? { DOI: doi.trim() } : {}),
      ...(url.trim() ? { URL: url.trim() } : {}),
      ...(journal.trim() ? { containerTitle: journal.trim() } : {}),
      ...(volume.trim() ? { volume: volume.trim() } : {}),
      ...(issue.trim() ? { issue: issue.trim() } : {}),
      ...(page.trim() ? { page: page.trim() } : {}),
    };
  }, [citation, type, title, authors, year, journal, volume, issue, page, doi, url]);

  const preview = useMemo(() => {
    if (!draft.title && draft.author.length === 0) return null;
    if (!canFormatStyle(style)) return null;
    try {
      return formatCitation(draft, style);
    } catch {
      // A style that can't format is the preview's problem, never the editor's
      // — the draft is still perfectly saveable.
      return null;
    }
  }, [draft, style]);

  if (!open || !citation) return null;

  const field = (
    label: string,
    value: string,
    set: (v: string) => void,
    testid: string,
    extra: React.InputHTMLAttributes<HTMLInputElement> = {},
  ) => (
    <label className="cm-editor__field">
      <span className="cm-editor__label">{label}</span>
      <input className="cm-editor__input" value={value} data-testid={testid} onChange={(e) => set(e.target.value)} {...extra} />
    </label>
  );

  return (
    <Modal open={open} title="Edit citation details" onClose={onCancel}>
      <div className="cm-editor" data-testid="citation-editor">
        <label className="cm-editor__field">
          <span className="cm-editor__label">Type</span>
          <select className="cm-editor__input" value={type} data-testid="editor-type" onChange={(e) => setType(e.target.value)}>
            {CSL_TYPES.map(([id, label]) => (
              <option key={id} value={id}>{label}</option>
            ))}
          </select>
        </label>

        {field('Title', title, setTitle, 'editor-title')}

        <div className="cm-editor__field">
          <span className="cm-editor__label">Authors</span>
          {authors.map((a, i) => (
            <div className="cm-editor__author" key={i}>
              <input
                className="cm-editor__input"
                placeholder="Family name"
                value={a.family}
                data-testid={`editor-author-family-${i}`}
                onChange={(e) => setAuthors((xs) => xs.map((x, j) => (j === i ? { ...x, family: e.target.value } : x)))}
              />
              <input
                className="cm-editor__input"
                placeholder="Given name(s)"
                value={a.given}
                data-testid={`editor-author-given-${i}`}
                onChange={(e) => setAuthors((xs) => xs.map((x, j) => (j === i ? { ...x, given: e.target.value } : x)))}
              />
              <button
                type="button"
                className="gds-btn gds-btn--ghost"
                aria-label={`Remove author ${i + 1}`}
                data-testid={`editor-author-remove-${i}`}
                onClick={() => setAuthors((xs) => xs.filter((_, j) => j !== i))}
              >
                ✕
              </button>
            </div>
          ))}
          <button
            type="button"
            className="gds-btn gds-btn--ghost cm-editor__addauthor"
            data-testid="editor-author-add"
            onClick={() => setAuthors((xs) => [...xs, { family: '', given: '' }])}
          >
            + Add author
          </button>
        </div>

        <div className="cm-editor__row">
          {field('Year', year, setYear, 'editor-year', { inputMode: 'numeric', placeholder: 'e.g. 2020' })}
          {field('Volume', volume, setVolume, 'editor-volume')}
          {field('Issue', issue, setIssue, 'editor-issue')}
        </div>

        {field('Journal / container', journal, setJournal, 'editor-journal')}

        <div className="cm-editor__row">
          {field('Pages', page, setPage, 'editor-page', { placeholder: 'e.g. 357-362' })}
          {field('DOI', doi, setDoi, 'editor-doi', { placeholder: '10.1038/…' })}
        </div>

        {field('URL', url, setUrl, 'editor-url')}

        <div className="cm-editor__preview" data-testid="editor-preview">
          <span className="cm-editor__label">Preview ({style})</span>
          {preview ? <p><Marked text={preview} /></p> : <p className="cm-editor__hint">Add a title or an author to see the formatted citation.</p>}
        </div>

        <div className="cm-editor__actions">
          <button type="button" className="gds-btn gds-btn--ghost" data-testid="editor-cancel" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="gds-btn gds-btn--primary"
            data-testid="editor-save"
            onClick={() => onSave({ ...citation, csl: draft, doi: draft.DOI ?? null })}
          >
            Save
          </button>
        </div>
      </div>
    </Modal>
  );
};
