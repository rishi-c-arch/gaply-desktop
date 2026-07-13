// Gaply — Note Creator (per-paper structured notes, Set 4). FREE + LOCAL:
// note_creator is in OFFLINE_FEATURES, the route is RequireAuth-wrapped, there
// is no entitlement gate and no proxy. Pick a paper from your citation library
// (or free-type one — the soft anchor allows it), then keep guided, own-words
// reading notes. Consumes the Set 3 bridge; NO model, nothing auto-summarizes.
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Badge, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { NotesBridge, TauriNotesBridge, Note, NoteDraft } from './notesBridge';
import { PaperSource, TauriPaperSource, PaperOption } from './paperSource';
import PaperNoteEditor from './PaperNoteEditor';
import './notes.css';

export interface NoteCreatorPageProps {
  notes?: NotesBridge;
  papers?: PaperSource;
}

type Editing =
  | { mode: 'new'; base: { id: string; paper_id: string | null; paper_title: string } }
  | { mode: 'edit'; note: Note };

const newId = (): string =>
  (typeof crypto !== 'undefined' && 'randomUUID' in crypto) ? crypto.randomUUID() : `n_${Date.now()}_${Math.round(Math.random() * 1e9)}`;

const NoteCreatorPage: React.FC<NoteCreatorPageProps> = ({ notes, papers }) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => notes ?? new TauriNotesBridge(), [notes]);
  const source = useMemo(() => papers ?? new TauriPaperSource(), [papers]);

  const [list, setList] = useState<Note[] | null>(null);
  const [options, setOptions] = useState<PaperOption[]>([]);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [pickId, setPickId] = useState('');
  const [freeTitle, setFreeTitle] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setList(await bridge.list('paper'));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not load notes');
      setList([]);
    }
  }, [bridge]);

  useEffect(() => {
    void refresh();
    void source.listPapers().then(setOptions).catch(() => setOptions([]));
  }, [refresh, source]);

  const startFromLibrary = () => {
    const p = options.find((o) => o.id === pickId);
    if (!p) return;
    setEditing({ mode: 'new', base: { id: newId(), paper_id: p.id, paper_title: p.title } });
  };
  const startFreeTyped = () => {
    const t = freeTitle.trim();
    if (!t) return;
    setEditing({ mode: 'new', base: { id: newId(), paper_id: null, paper_title: t } });
    setFreeTitle('');
  };

  const save = async (draft: NoteDraft) => {
    setBusy(true);
    setError(null);
    try {
      if (editing?.mode === 'edit') await bridge.update(draft);
      else await bridge.create(draft);
      setEditing(null);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not save the note');
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string) => {
    setBusy(true);
    try {
      await bridge.remove(id);
      setEditing(null);
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  const rail = (
    <NavRail
      items={[
        { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
        { id: 'notes', label: 'Note Creator', icon: '✎' },
      ]}
      activeId="notes"
      brand={<GaplyGlobe scale="mark" />}
    />
  );

  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="note-creator">
      <AppShell rail={rail} header={<HeaderBar title="Note Creator"><Badge status="certain">free · on device</Badge></HeaderBar>}>
        <Panel title="Reading notes — your papers, your words">
          <div style={{ display: 'grid', gap: 16 }}>
            {/* Paper picker: from the library OR free-typed */}
            <Card title="Start notes on a paper" data-testid="paper-picker">
              <div style={{ display: 'grid', gap: 10 }}>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                  <select className="gds-jc__input" data-testid="picker-select" value={pickId} onChange={(e) => setPickId(e.target.value)} style={{ minWidth: 260 }}>
                    <option value="">Pick a paper from your library…</option>
                    {options.map((o) => (
                      <option key={o.id} value={o.id}>
                        {o.title}{o.year ? ` (${o.year})` : ''}{o.hasFullText ? '  📄' : ''}
                      </option>
                    ))}
                  </select>
                  <Button onClick={startFromLibrary} disabled={!pickId} data-testid="picker-start">Start notes</Button>
                  {pickId && options.find((o) => o.id === pickId)?.hasFullText && (
                    <Badge status="neutral" data-testid="picker-fulltext">📄 full text available</Badge>
                  )}
                </div>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                  <input className="gds-jc__input" placeholder="…or type any paper title" value={freeTitle} data-testid="picker-freetype" onChange={(e) => setFreeTitle(e.target.value)} style={{ minWidth: 260 }} />
                  <Button variant="secondary" onClick={startFreeTyped} disabled={!freeTitle.trim()} data-testid="picker-freestart">Start notes</Button>
                </div>
                {options.length === 0 && (
                  <p className="gds-jc__disclaimer" data-testid="picker-nolib">
                    No papers in your citation library yet — add references in the Citation Manager, or just type a title above.
                  </p>
                )}
              </div>
            </Card>

            {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} role="alert" data-testid="note-error">{error}</p>}

            {/* The editor (create or edit) */}
            {editing && (
              <PaperNoteEditor
                base={editing.mode === 'edit' ? { id: editing.note.id, paper_id: editing.note.paper_id, paper_title: editing.note.paper_title } : editing.base}
                existing={editing.mode === 'edit' ? editing.note : null}
                onSave={save}
                onDelete={editing.mode === 'edit' ? () => remove(editing.note.id) : undefined}
                onClose={() => setEditing(null)}
                busy={busy}
              />
            )}

            {/* The note list */}
            <Card title="Your paper notes" data-testid="note-list-card">
              {list === null ? (
                <p className="gds-note__empty">Loading…</p>
              ) : list.length === 0 ? (
                <p className="gds-note__empty" data-testid="note-empty">No notes yet — pick a paper and start your reading notes.</p>
              ) : (
                <div className="gds-note-list" data-testid="note-list">
                  {list.map((n) => (
                    <button key={n.id} className="gds-note-list__row" data-testid={`note-row-${n.id}`} onClick={() => setEditing({ mode: 'edit', note: n })}>
                      <div className="gds-note-list__title">{n.title || '(untitled note)'}</div>
                      <div className="gds-note-list__paper">{n.paper_title || 'free-typed paper'}</div>
                      {n.tags.length > 0 && <div className="gds-note-list__tags">{n.tags.map((t) => <span key={t}>#{t}</span>)}</div>}
                    </button>
                  ))}
                </div>
              )}
            </Card>
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

export default NoteCreatorPage;
