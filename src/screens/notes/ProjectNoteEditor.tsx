// Gaply — the project note editor (Set 5). Low-friction quick capture for the
// researcher's OWN ideas / hypotheses / meeting notes / to-dos — just a title +
// body + tags (note_type 'project', fields_json stays {}). No model, nothing
// auto-generates; these are the researcher's own thoughts.
import React, { useState } from 'react';
import { Badge, Button, Card } from '../../design-system';
import { Note, NoteDraft } from './notesBridge';

export interface ProjectNoteEditorProps {
  /** The note id (a fresh uuid for new, or the existing note's id). */
  id: string;
  existing?: Note | null;
  onSave: (draft: NoteDraft) => void;
  onDelete?: () => void;
  onClose: () => void;
  busy?: boolean;
}

const ProjectNoteEditor: React.FC<ProjectNoteEditorProps> = ({ id, existing, onSave, onDelete, onClose, busy }) => {
  const [title, setTitle] = useState(existing?.title ?? '');
  const [body, setBody] = useState(existing?.body ?? '');
  const [tags, setTags] = useState((existing?.tags ?? []).join(', '));

  const save = () =>
    onSave({
      id,
      note_type: 'project',
      paper_id: null,
      paper_title: '',
      title: title.trim(),
      body: body.trim(),
      tags: tags.split(',').map((t) => t.trim()).filter(Boolean),
      // no `fields` → the store keeps fields_json {}
    });

  return (
    <Card title="Project note">
      <div style={{ display: 'grid', gap: 12 }} data-testid="project-note-editor">
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Badge status="assessed">🟡 your idea</Badge>
          <span className="gds-jc__disclaimer" style={{ margin: 0 }}>A hypothesis, a to-do, a thought from your advisor meeting — yours.</span>
        </div>
        <input className="gds-jc__input" value={title} placeholder="Title (optional)" data-testid="project-title" onChange={(e) => setTitle(e.target.value)} />
        <textarea className="gds-note__area" rows={6} value={body} placeholder="Capture it — an idea, a hypothesis, a next step…" data-testid="project-body" onChange={(e) => setBody(e.target.value)} />
        <input className="gds-jc__input" value={tags} placeholder="comma, separated, tags" data-testid="project-tags" onChange={(e) => setTags(e.target.value)} />
        <div style={{ display: 'flex', gap: 8 }}>
          <Button onClick={save} disabled={busy} data-testid="project-save">{busy ? 'Saving…' : existing ? 'Save changes' : 'Save note'}</Button>
          <Button variant="secondary" onClick={onClose} data-testid="project-close">Close</Button>
          {existing && onDelete && (
            <Button variant="secondary" onClick={onDelete} data-testid="project-delete" style={{ marginLeft: 'auto', color: 'var(--g-flagged)' }}>Delete</Button>
          )}
        </div>
      </div>
    </Card>
  );
};

export default ProjectNoteEditor;
