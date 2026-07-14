// Gaply — Note Creator (Sets 4 + 5). FREE + LOCAL: note_creator is in
// OFFLINE_FEATURES, RequireAuth-wrapped, no entitlement, no proxy. Two note
// types: per-paper structured reading notes (Set 4) and project quick-capture
// (Set 5, the researcher's own ideas/hypotheses/todos). A unified search across
// BOTH. Consumes the Set 3 bridge; NO model, nothing auto-summarizes. Plus a
// static recommended-tools panel (honest third-party links).
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Badge, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { NotesBridge, TauriNotesBridge, Note, NoteDraft, NoteType } from './notesBridge';
import { PaperSource, TauriPaperSource, PaperOption } from './paperSource';
import PaperNoteEditor from './PaperNoteEditor';
import ProjectNoteEditor from './ProjectNoteEditor';
import RecommendedToolsPanel from './RecommendedToolsPanel';
import './notes.css';

export interface NoteCreatorPageProps {
  notes?: NotesBridge;
  papers?: PaperSource;
}

type Editing =
  | { kind: 'paper-new'; base: { id: string; paper_id: string | null; paper_title: string } }
  | { kind: 'paper-edit'; note: Note }
  | { kind: 'project-edit'; note: Note };

type TypeFilter = 'all' | NoteType;

const newId = (): string =>
  typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : `n_${Date.now()}_${Math.round(Math.random() * 1e9)}`;

const uniqueTags = (notes: Note[]): string[] =>
  Array.from(new Set(notes.flatMap((n) => n.tags))).sort();

const NoteCreatorPage: React.FC<NoteCreatorPageProps> = ({ notes, papers }) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => notes ?? new TauriNotesBridge(), [notes]);
  const source = useMemo(() => papers ?? new TauriPaperSource(), [papers]);

  const [list, setList] = useState<Note[] | null>(null);
  const [universe, setUniverse] = useState(0);
  const [allTags, setAllTags] = useState<string[]>([]);
  const [options, setOptions] = useState<PaperOption[]>([]);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [paperText, setPaperText] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // filters (unified search across BOTH note types)
  const [query, setQuery] = useState('');
  const [typeFilter, setTypeFilter] = useState<TypeFilter>('all');
  const [tagFilter, setTagFilter] = useState<string | null>(null);

  // quick-capture project note (one action, minimal friction)
  const [quickTitle, setQuickTitle] = useState('');
  const [quickBody, setQuickBody] = useState('');

  // paper picker
  const [pickId, setPickId] = useState('');
  const [freeTitle, setFreeTitle] = useState('');

  const reload = useCallback(async () => {
    try {
      const all = await bridge.search(''); // the universe — for tags + empty detection
      setUniverse(all.length);
      setAllTags(uniqueTags(all));
      const found = await bridge.search(query.trim(), tagFilter ?? undefined);
      setList(typeFilter === 'all' ? found : found.filter((n) => n.note_type === typeFilter));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not load notes');
      setList([]);
    }
  }, [bridge, query, typeFilter, tagFilter]);

  useEffect(() => {
    void reload();
  }, [reload]);
  useEffect(() => {
    void source.listPapers().then(setOptions).catch(() => setOptions([]));
  }, [source]);

  // Optional side-by-side: fetch the paper's full text (from the plagiarism
  // "my papers" library) when a paper editor opens; null → editor-only.
  useEffect(() => {
    setPaperText(null);
    const title =
      editing?.kind === 'paper-new' ? editing.base.paper_title :
      editing?.kind === 'paper-edit' ? editing.note.paper_title : '';
    // M2 Set 2C: thread the note's paper_id (→ citation_id) so the read resolves
    // by the RELIABLE id first, then the title fallback. Free-typed notes
    // (paper_id '' / null) fall through to the title path exactly as before.
    const paperId =
      editing?.kind === 'paper-new' ? editing.base.paper_id :
      editing?.kind === 'paper-edit' ? editing.note.paper_id : null;
    if (!title && !paperId) return;
    let alive = true;
    bridge.paperFullText(title, paperId ?? undefined).then((t) => { if (alive) setPaperText(t); }).catch(() => {});
    return () => { alive = false; };
  }, [editing, bridge]);

  const afterMutation = async () => {
    setEditing(null);
    await reload();
  };

  const save = async (draft: NoteDraft) => {
    setBusy(true);
    setError(null);
    try {
      const isEdit = editing && (editing.kind === 'paper-edit' || editing.kind === 'project-edit');
      if (isEdit) await bridge.update(draft);
      else await bridge.create(draft);
      await afterMutation();
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
      await afterMutation();
    } finally {
      setBusy(false);
    }
  };

  const quickCapture = async () => {
    if (!quickTitle.trim() && !quickBody.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await bridge.create({ id: newId(), note_type: 'project', title: quickTitle.trim(), body: quickBody.trim() });
      setQuickTitle('');
      setQuickBody('');
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not save the note');
    } finally {
      setBusy(false);
    }
  };

  const startFromLibrary = () => {
    const p = options.find((o) => o.id === pickId);
    if (p) setEditing({ kind: 'paper-new', base: { id: newId(), paper_id: p.id, paper_title: p.title } });
  };
  const startFreeTyped = () => {
    const t = freeTitle.trim();
    if (t) {
      setEditing({ kind: 'paper-new', base: { id: newId(), paper_id: null, paper_title: t } });
      setFreeTitle('');
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
        <Panel title="Your notes — your papers, your ideas, your words">
          <div style={{ display: 'grid', gap: 16 }}>
            {/* Quick-capture project note (one action) */}
            <Card title="Quick note" data-testid="quick-capture">
              <div style={{ display: 'grid', gap: 8 }}>
                <input className="gds-jc__input" placeholder="An idea, a hypothesis, a to-do…" value={quickTitle} data-testid="quick-title" onChange={(e) => setQuickTitle(e.target.value)} />
                <textarea className="gds-note__area" rows={2} placeholder="Capture it before it’s gone (optional details)…" value={quickBody} data-testid="quick-body" onChange={(e) => setQuickBody(e.target.value)} />
                <div>
                  <Button onClick={quickCapture} disabled={busy || (!quickTitle.trim() && !quickBody.trim())} data-testid="quick-save">Save project note</Button>
                </div>
              </div>
            </Card>

            {/* Paper picker (Set 4): from the library OR free-typed */}
            <Card title="Start notes on a paper" data-testid="paper-picker">
              <div style={{ display: 'grid', gap: 10 }}>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                  <select className="gds-jc__input" data-testid="picker-select" value={pickId} onChange={(e) => setPickId(e.target.value)} style={{ minWidth: 260 }}>
                    <option value="">Pick a paper from your library…</option>
                    {options.map((o) => (
                      <option key={o.id} value={o.id}>{o.title}{o.year ? ` (${o.year})` : ''}{o.hasFullText ? '  📄' : ''}</option>
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
                  <p className="gds-jc__disclaimer" data-testid="picker-nolib">No papers in your citation library yet — add references in the Citation Manager, or just type a title above.</p>
                )}
              </div>
            </Card>

            {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} role="alert" data-testid="note-error">{error}</p>}

            {/* The editor (create or edit), routed by note type */}
            {editing && (editing.kind === 'paper-new' || editing.kind === 'paper-edit') && (
              <PaperNoteEditor
                base={editing.kind === 'paper-edit' ? { id: editing.note.id, paper_id: editing.note.paper_id, paper_title: editing.note.paper_title } : editing.base}
                existing={editing.kind === 'paper-edit' ? editing.note : null}
                paperText={paperText}
                // The picked paper's badge promised full text → if the read comes
                // back null (case/format mismatch or an ambiguous collision the
                // backend refused to guess), the editor shows an honest note (M2).
                fullTextExpected={
                  !!options.find(
                    (o) => o.id === (editing.kind === 'paper-edit' ? editing.note.paper_id : editing.base.paper_id)
                  )?.hasFullText
                }
                onSave={save}
                onDelete={editing.kind === 'paper-edit' ? () => remove(editing.note.id) : undefined}
                onClose={() => setEditing(null)}
                busy={busy}
              />
            )}
            {editing && editing.kind === 'project-edit' && (
              <ProjectNoteEditor id={editing.note.id} existing={editing.note} onSave={save} onDelete={() => remove(editing.note.id)} onClose={() => setEditing(null)} busy={busy} />
            )}

            {/* Unified search across BOTH note types */}
            <Card title="Your notes" data-testid="note-list-card">
              <div style={{ display: 'grid', gap: 10 }}>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                  <input className="gds-jc__input" placeholder="Search all your notes…" value={query} data-testid="notes-search" onChange={(e) => setQuery(e.target.value)} style={{ minWidth: 220 }} />
                  <div style={{ display: 'flex', gap: 4 }} data-testid="type-filter">
                    {(['all', 'paper', 'project'] as TypeFilter[]).map((t) => (
                      <button key={t} className={`gds-note-chip${typeFilter === t ? ' gds-note-chip--on' : ''}`} data-testid={`filter-${t}`} onClick={() => setTypeFilter(t)}>
                        {t === 'all' ? 'All' : t === 'paper' ? '📄 Paper' : '💡 Project'}
                      </button>
                    ))}
                  </div>
                </div>
                {allTags.length > 0 && (
                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }} data-testid="tag-filter">
                    <button className={`gds-note-chip${tagFilter === null ? ' gds-note-chip--on' : ''}`} data-testid="tag-all" onClick={() => setTagFilter(null)}>all tags</button>
                    {allTags.map((t) => (
                      <button key={t} className={`gds-note-chip${tagFilter === t ? ' gds-note-chip--on' : ''}`} data-testid={`tag-${t}`} onClick={() => setTagFilter(t)}>#{t}</button>
                    ))}
                  </div>
                )}

                {list === null ? (
                  <p className="gds-note__empty">Loading…</p>
                ) : universe === 0 ? (
                  <p className="gds-note__empty" data-testid="note-empty">No notes yet — capture an idea, a hypothesis, a to-do, or pick a paper to annotate.</p>
                ) : list.length === 0 ? (
                  <p className="gds-note__empty" data-testid="note-nomatch">No notes match your search.</p>
                ) : (
                  <div className="gds-note-list" data-testid="note-list">
                    {list.map((n) => (
                      <button
                        key={n.id}
                        className="gds-note-list__row"
                        data-testid={`note-row-${n.id}`}
                        onClick={() => setEditing(n.note_type === 'paper' ? { kind: 'paper-edit', note: n } : { kind: 'project-edit', note: n })}
                      >
                        <div className="gds-note-list__title">
                          <span data-testid={`note-type-${n.id}`}>{n.note_type === 'paper' ? '📄' : '💡'}</span> {n.title || '(untitled note)'}
                        </div>
                        <div className="gds-note-list__paper">{n.note_type === 'paper' ? (n.paper_title || 'free-typed paper') : 'project note'}</div>
                        {n.tags.length > 0 && <div className="gds-note-list__tags">{n.tags.map((t) => <span key={t}>#{t}</span>)}</div>}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </Card>

            <RecommendedToolsPanel />
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

export default NoteCreatorPage;
