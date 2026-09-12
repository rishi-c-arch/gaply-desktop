// Gaply — the project note editor (Set 5), "Academic Focus" redesign (Stitch).
// Low-friction capture of the researcher's OWN ideas — title + body + tags
// (note_type 'project', fields_json stays {}). No model, nothing auto-generates.
// Visual language: the mock's paper-editor screen with the idea-yellow accent;
// same props/testids/behavior as before (save / close / delete / export /
// preview==export via buildExportable + noteToMarkdown).
import React, { useEffect, useRef, useState } from 'react';
import MarkdownRenderer from '../../components/MarkdownRenderer';
import RichBody from './RichBody';
import { Note, NoteDraft } from './notesBridge';
import { noteToMarkdown, exportFileName, ExportableNote } from './noteExport';
import { saveNoteFile } from './saveNoteFile';
import { IcBack, IcEye, IcExport, IcTrash, IcSave, IcEditNote } from './NotesIcons';
import FontScale from './FontScale';
import { readingMinutes } from './PaperNoteEditor';
import { SaveState, DiscardWarning, useDirtyBaseline, useCloseGuard } from './editorSaveState';
import './notes.css';

export interface ProjectNoteEditorProps {
  /** The note id (a fresh uuid for new, or the existing note's id). */
  id: string;
  existing?: Note | null;
  onSave: (draft: NoteDraft) => void;
  onDelete?: () => void;
  onClose: () => void;
  busy?: boolean;
  /** Surface a genuine export write-failure (null clears). Cancel stays silent. */
  onError?: (msg: string | null) => void;
}

const ProjectNoteEditor: React.FC<ProjectNoteEditorProps> = ({ id, existing, onSave, onDelete, onClose, busy, onError }) => {
  const [title, setTitle] = useState(existing?.title ?? '');
  // body stays CANONICAL MARKDOWN. RichBody only calls setBody on real user
  // edits, so opening a note without touching it never rewrites its bytes.
  const [body, setBody] = useState(existing?.body ?? '');
  const [tags, setTags] = useState((existing?.tags ?? []).join(', '));
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

  const splitTags = () => tags.split(',').map((t) => t.trim()).filter(Boolean);

  // Dirty tracking: the snapshot is exactly what save() would persist (trimmed
  // title/body + split tags), so cosmetic whitespace never marks the note dirty
  // and a real edit always does.
  const snapshot = JSON.stringify([title.trim(), body.trim(), splitTags()]);
  const { dirty } = useDirtyBaseline(snapshot);
  const { askedToClose, requestClose, keepEditing, discard } = useCloseGuard(dirty, onClose);

  // The exact ExportableNote both export AND preview use — one source, no drift.
  const buildExportable = (): ExportableNote => ({
    note_type: 'project',
    title: title.trim(),
    paper_title: '',
    body: body.trim(),
    tags: splitTags(),
  });

  const save = () =>
    onSave({
      id,
      note_type: 'project',
      paper_id: null,
      paper_title: '',
      title: title.trim(),
      body: body.trim(),
      tags: splitTags(),
      // no `fields` → the store keeps fields_json {}
    });

  // Export the current note as Markdown (title + body + tags). Pure emit → save.
  const exportMd = async () => {
    onError?.(null); // clear any prior export error
    try {
      const md = noteToMarkdown(buildExportable(), {});
      // saveNoteFile returns null on cancel (silent) and only THROWS on a real
      // write failure — so only genuine failures reach this catch.
      await saveNoteFile(exportFileName(title, 'note'), md);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : 'Could not save the export');
    }
  };

  return (
    <div data-testid="project-note-editor">
      {/* Header / toolbar (mock: Paper Editor top bar) */}
      <header className="an-edit-head">
        <div className="an-edit-head-left">
          <button className="an-backbtn" onClick={requestClose} data-testid="project-close" title="Back to your library"><IcBack /></button>
          <h1>Project Note</h1>
        </div>
        <div className="an-edit-actions">
          <FontScale />
          <button className="an-ghostbtn" onClick={() => setPreview((p) => !p)} data-testid="project-preview-toggle">
            <IcEye /> {preview ? 'Edit' : 'Preview'}
          </button>
          <button className="an-ghostbtn" onClick={() => void exportMd()} disabled={busy || (!title.trim() && !body.trim())} data-testid="project-export">
            <IcExport /> Export
          </button>
          <div className="an-divider" />
          {existing && onDelete && (
            <button
              className={`an-deletebtn${deleteArmed ? ' an-deletebtn--armed' : ''}`}
              onClick={onDeleteClick}
              data-testid="project-delete"
              title={deleteArmed ? 'Click again to delete' : 'Delete note'}
            >
              {deleteArmed ? 'Really delete?' : <IcTrash />}
            </button>
          )}
          <button className="an-savebtn" onClick={save} disabled={busy} data-testid="project-save">
            <IcSave /> {busy ? 'Saving…' : existing ? 'Save Changes' : 'Save Note'}
          </button>
        </div>
      </header>

      <div className="an-edit-wrap">
        {askedToClose && <DiscardWarning what="this note" onKeep={keepEditing} onDiscard={discard} testid="project-discard" />}

        {/* Breadcrumbs + honest storage status */}
        <div className="an-crumbs">
          <div className="an-crumbs-path"><span>My Library</span><span>›</span><strong>Project Note</strong></div>
          <SaveState dirty={dirty} busy={busy} savedAt={existing?.updated_at} testid="project-save-state" />
        </div>

        {preview ? (
          // Read-only preview — exactly what export produces (same buildExportable).
          <>
            <div className="an-preview-card" data-testid="project-preview">
              <MarkdownRenderer content={noteToMarkdown(buildExportable(), {})} />
              <div className="an-preview-foot">
                <div className="an-preview-meta">
                  {existing && existing.updated_at > 1e9 && (
                    <div><b>Last edited</b><span>{new Date(existing.updated_at * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })}</span></div>
                  )}
                  <div><b>Reading time</b><span>{readingMinutes(noteToMarkdown(buildExportable(), {}))} min</span></div>
                </div>
                <div className="an-tagpills">{splitTags().map((t) => <span key={t} className="an-pill">#{t}</span>)}</div>
              </div>
            </div>
            <div className="an-dock">
              <button className="an-dock-edit" data-testid="project-preview-edit-pill" onClick={() => setPreview(false)}><IcEditNote size={18} /> Edit Note</button>
              <div className="an-dock-divider" />
              <button className="an-dock-icon" data-testid="project-preview-export" title="Export (.md)" onClick={() => void exportMd()}><IcExport size={18} /></button>
            </div>
          </>
        ) : (
          <div className="an-canvas an-canvas--idea">
            <section>
              <label className="an-label">Note Title</label>
              <input className="an-title-input" value={title} placeholder="Name this idea…" data-testid="project-title" onChange={(e) => setTitle(e.target.value)} />
            </section>
            <section>
              <label className="an-label">Freeform Sketch</label>
              {/* Rich writing surface (Set 1): "- " → bullet, "# " → heading,
                  "> " → quote, ⌘B/⌘I — stored as canonical markdown. */}
              <RichBody
                value={body}
                onChange={setBody}
                placeholder="Capture it — an idea, a hypothesis, a next step…"
                testid="project-body"
              />
            </section>
            <div className="an-edit-foot">
              <div className="an-foot-meta"><span>💡 Your idea — a hypothesis, a to-do, a thought from your advisor meeting.</span></div>
              <div className="an-tagpills">
                {splitTags().map((t) => <span key={t} className="an-pill">#{t}</span>)}
                <input className="an-tags-input" value={tags} placeholder="comma, separated, tags" data-testid="project-tags" onChange={(e) => setTags(e.target.value)} />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default ProjectNoteEditor;
