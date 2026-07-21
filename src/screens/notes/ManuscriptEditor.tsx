// Gaply — Research Paper Writer, Set A: the manuscript editor shell.
// Section navigation + per-section RichBody (the proven TipTap surface, each
// with a STABLE key so switching/reordering never corrupts content) + per-section
// word counts + document metadata (title/authors) + honest scaffold guidance +
// a basic .docx export. NO model writes here beyond the note; NO AI.
//
// HONESTY: the scaffold is Gaply's own "Generic IMRaD structure" (not a publisher
// template); export is "submission structure", never "camera-ready".
import React, { useEffect, useRef, useState } from 'react';
import RichBody from './RichBody';
import { Note, NoteDraft } from './notesBridge';
import {
  Manuscript, ManuscriptSection, manuscriptFromNote, manuscriptToDraft, newManuscript,
  scaffoldById, wordCount,
} from './manuscriptModel';
import { saveManuscriptDocx } from './manuscriptDocx';
import { exportFileName } from './noteExport';
import { IcBack, IcExport, IcTrash, IcSave } from './NotesIcons';
import FontScale from './FontScale';
import './notes.css';

export interface ManuscriptEditorProps {
  id: string;
  existing?: Note | null;
  onSave: (draft: NoteDraft) => void;
  onDelete?: () => void;
  onClose: () => void;
  busy?: boolean;
  onError?: (msg: string | null) => void;
}

const ManuscriptEditor: React.FC<ManuscriptEditorProps> = ({ id, existing, onSave, onDelete, onClose, busy, onError }) => {
  const initial: Manuscript = existing ? manuscriptFromNote(existing) : newManuscript(id);
  const [title, setTitle] = useState(initial.title);
  const [authors, setAuthors] = useState(initial.authors);
  const [sections, setSections] = useState<ManuscriptSection[]>(initial.sections);
  const [activeKey, setActiveKey] = useState(initial.sections[0]?.key ?? '');
  const scaffold = scaffoldById(initial.scaffoldId);

  // Two-step delete (mirrors the other editors).
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

  const active = sections.find((s) => s.key === activeKey) ?? sections[0];
  const setBody = (key: string, body: string) =>
    setSections((prev) => prev.map((s) => (s.key === key ? { ...s, body } : s)));

  const current = (): Manuscript => ({ ...initial, title, authors, sections });
  const save = () => onSave(manuscriptToDraft(current()));

  const exportDocx = async () => {
    onError?.(null);
    try {
      // saveManuscriptDocx returns null on cancel (silent) — only real failures throw.
      await saveManuscriptDocx(current(), exportFileName(title, 'manuscript').replace(/\.md$/, '.docx'));
    } catch (e) {
      onError?.(e instanceof Error ? e.message : 'Could not export the .docx');
    }
  };

  const totalWords = sections.reduce((n, s) => n + wordCount(s.body), 0);

  return (
    <div data-testid="manuscript-editor">
      <header className="an-edit-head">
        <div className="an-edit-head-left">
          <button className="an-backbtn" onClick={onClose} data-testid="ms-close" title="Back to your library"><IcBack /></button>
          <h1>Research Paper</h1>
        </div>
        <div className="an-edit-actions">
          <FontScale />
          <button className="an-ghostbtn" onClick={() => void exportDocx()} disabled={busy} data-testid="ms-export" title="Export .docx (submission structure)">
            <IcExport /> Export .docx
          </button>
          <div className="an-divider" />
          {existing && onDelete && (
            <button className={`an-deletebtn${deleteArmed ? ' an-deletebtn--armed' : ''}`} onClick={onDeleteClick} data-testid="ms-delete" title={deleteArmed ? 'Click again to delete' : 'Delete manuscript'}>
              {deleteArmed ? 'Really delete?' : <IcTrash />}
            </button>
          )}
          <button className="an-savebtn" onClick={save} disabled={busy} data-testid="ms-save">
            <IcSave /> {busy ? 'Saving…' : existing ? 'Save Changes' : 'Save Manuscript'}
          </button>
        </div>
      </header>

      <div className="an-edit-wrap">
        <div className="an-crumbs">
          <div className="an-crumbs-path"><span>My Library</span><span>›</span><strong>Research Paper</strong></div>
          <div className="an-synced">Saved locally · on device</div>
        </div>

        {/* Honest promise — structure + references, not camera-ready. */}
        <div className="an-ms-notice" data-testid="ms-notice">
          Gaply formats your <b>structure and references</b> — your publisher typesets the final camera-ready layout.
          This is the <b>{scaffold.label}</b> (Gaply’s own scaffold, not an official template). Always check your venue’s author guidelines.
        </div>

        {/* Document metadata: title + authors → the .docx title page. */}
        <div className="an-canvas an-canvas--ms">
          <section className="an-ms-meta">
            <label className="an-label">Manuscript Title</label>
            <input className="an-title-input" value={title} placeholder="Your paper’s title…" data-testid="ms-title" onChange={(e) => setTitle(e.target.value)} />
            <label className="an-label">Authors</label>
            <input className="an-tags-input an-ms-authors" value={authors} placeholder="e.g. Ada Lovelace, Alan Turing" data-testid="ms-authors" onChange={(e) => setAuthors(e.target.value)} />
          </section>

          <div className="an-ms-body">
            {/* Section navigation with per-section word counts. */}
            <nav className="an-ms-nav" data-testid="ms-nav">
              {sections.map((s) => (
                <button
                  key={s.key}
                  className={`an-ms-navitem${s.key === activeKey ? ' an-active' : ''}`}
                  data-testid={`ms-nav-${s.key}`}
                  onClick={() => setActiveKey(s.key)}
                >
                  <span>{s.heading}</span>
                  <span className="an-ms-wc">{wordCount(s.body)}</span>
                </button>
              ))}
              <div className="an-ms-total" data-testid="ms-total-words">{totalWords} words total</div>
            </nav>

            {/* The active section: a per-section RichBody with a STABLE key
                (=section key) so switching sections remounts cleanly and never
                bleeds content between sections (the parse-once-on-mount caveat). */}
            {active && (
              <div className="an-ms-section">
                <div className="an-ms-section-head">
                  <h2>{active.heading}</h2>
                  <span className="an-ms-guidance" data-testid="ms-guidance">{active.guidance}</span>
                </div>
                <RichBody
                  key={active.key}
                  value={active.body}
                  onChange={(md) => setBody(active.key, md)}
                  placeholder={`Write the ${active.heading.toLowerCase()}…`}
                  testid={`ms-body-${active.key}`}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default ManuscriptEditor;
