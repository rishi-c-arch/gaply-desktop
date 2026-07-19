// Gaply — the project note editor (Set 5). Low-friction quick capture for the
// researcher's OWN ideas / hypotheses / meeting notes / to-dos — just a title +
// body + tags (note_type 'project', fields_json stays {}). No model, nothing
// auto-generates; these are the researcher's own thoughts.
import React, { useState } from 'react';
import { Badge, Button, Card } from '../../design-system';
import MarkdownRenderer from '../../components/MarkdownRenderer';
import { Note, NoteDraft } from './notesBridge';
import { noteToMarkdown, exportFileName, ExportableNote } from './noteExport';
import { saveNoteFile } from './saveNoteFile';

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
  const [body, setBody] = useState(existing?.body ?? '');
  const [tags, setTags] = useState((existing?.tags ?? []).join(', '));
  const [preview, setPreview] = useState(false);

  const splitTags = () => tags.split(',').map((t) => t.trim()).filter(Boolean);

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
    <Card title="Project note">
      <div style={{ display: 'grid', gap: 12 }} data-testid="project-note-editor">
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Badge status="assessed">🟡 your idea</Badge>
          <span className="gds-jc__disclaimer" style={{ margin: 0 }}>A hypothesis, a to-do, a thought from your advisor meeting — yours.</span>
        </div>
        {preview ? (
          // Read-only preview — exactly what export produces (same buildExportable).
          <div className="gds-note__preview" data-testid="project-preview">
            <MarkdownRenderer content={noteToMarkdown(buildExportable(), {})} />
          </div>
        ) : (
          <>
            <input className="gds-jc__input" value={title} placeholder="Title (optional)" data-testid="project-title" onChange={(e) => setTitle(e.target.value)} />
            <textarea className="gds-note__area" rows={6} value={body} placeholder="Capture it — an idea, a hypothesis, a next step…" data-testid="project-body" onChange={(e) => setBody(e.target.value)} />
            <input className="gds-jc__input" value={tags} placeholder="comma, separated, tags" data-testid="project-tags" onChange={(e) => setTags(e.target.value)} />
          </>
        )}
        <div style={{ display: 'flex', gap: 8 }}>
          <Button onClick={save} disabled={busy} data-testid="project-save">{busy ? 'Saving…' : existing ? 'Save changes' : 'Save note'}</Button>
          <Button variant="secondary" onClick={onClose} data-testid="project-close">Close</Button>
          <Button variant="ghost" onClick={() => setPreview((p) => !p)} data-testid="project-preview-toggle">{preview ? 'Edit' : 'Preview'}</Button>
          <Button variant="ghost" onClick={() => void exportMd()} disabled={busy || (!title.trim() && !body.trim())} data-testid="project-export">Export (.md)</Button>
          {existing && onDelete && (
            <Button variant="secondary" onClick={onDelete} data-testid="project-delete" style={{ marginLeft: 'auto', color: 'var(--g-flagged)' }}>Delete</Button>
          )}
        </div>
      </div>
    </Card>
  );
};

export default ProjectNoteEditor;
