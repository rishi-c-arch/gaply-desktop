// Gaply — Note Creator (Sets 4 + 5), "Academic Focus" redesign ported from the
// Stitch export (gaply_desktop_dashboard). FREE + LOCAL: note_creator is in
// OFFLINE_FEATURES, RequireAuth-wrapped, no entitlement, no proxy. Two note
// types (paper reading notes + project quick-capture), unified search across
// both, tag/type filters, per-note + bulk export, paper full-text side-by-side.
// NO model, nothing auto-summarizes. Honesty adaptations from the mock: the
// fake "pending citations / drafts" greeting uses REAL counts; the AI assistant
// bubble, fake profile, Archive, and sync/history/collaborator chrome are NOT
// ported (no capability theater). Fonts are BUNDLED (@fontsource) — the mock's
// Google-Fonts CDN is blocked by CSP + the offline promise.
import '@fontsource-variable/hanken-grotesk';
import '@fontsource-variable/source-serif-4';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { NotesBridge, TauriNotesBridge, Note, NoteDraft, NoteType, parseFields } from './notesBridge';
import { PaperSource, TauriPaperSource, PaperOption } from './paperSource';
import PaperNoteEditor from './PaperNoteEditor';
import ProjectNoteEditor from './ProjectNoteEditor';
import ManuscriptEditor from './ManuscriptEditor';
import RecommendedToolsPanel from './RecommendedToolsPanel';
import { notesToMarkdown } from './noteExport';
import { saveNoteFile } from './saveNoteFile';
import { imageRefsIn, gcOrphans } from './noteImages';
import { IcDoc, IcBulb, IcArticle, IcQuote, IcTag, IcGear, IcSearch, IcAdd, IcEditNote, IcBook, IcExport } from './NotesIcons';
import FontScale from './FontScale';
import './notes.css';

export interface NoteCreatorPageProps {
  notes?: NotesBridge;
  papers?: PaperSource;
}

type Editing =
  | { kind: 'paper-new'; base: { id: string; paper_id: string | null; paper_title: string } }
  | { kind: 'paper-edit'; note: Note }
  | { kind: 'project-edit'; note: Note }
  | { kind: 'manuscript-new'; id: string }
  | { kind: 'manuscript-edit'; note: Note };

type TypeFilter = 'all' | NoteType;

const newId = (): string =>
  typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : `n_${Date.now()}_${Math.round(Math.random() * 1e9)}`;

const uniqueTags = (notes: Note[]): string[] =>
  Array.from(new Set(notes.flatMap((n) => n.tags))).sort();

/** Real greeting (the mock hardcodes "Good morning"). */
const timeGreeting = (): string => {
  const h = new Date().getHours();
  return h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening';
};

/** A serif snippet for a note card — the note's own words, never invented. */
const snippetOf = (n: Note): string => {
  if (n.note_type === 'project' || n.note_type === 'manuscript') return n.body;
  const f = parseFields(n);
  return f.key_findings || f.research_question || f.citation || f.methodology || '';
};

/** Date chip for cards — only for plausible epoch-second timestamps. */
const dateOf = (n: Note): string | null => {
  if (n.updated_at < 1e9) return null;
  return new Date(n.updated_at * 1000).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
};

const NoteCreatorPage: React.FC<NoteCreatorPageProps> = ({ notes, papers }) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => notes ?? new TauriNotesBridge(), [notes]);
  const source = useMemo(() => papers ?? new TauriPaperSource(), [papers]);

  const [list, setList] = useState<Note[] | null>(null);
  const [universe, setUniverse] = useState(0);
  const [counts, setCounts] = useState({ paper: 0, project: 0, manuscript: 0 });
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
  const quickRef = useRef<HTMLInputElement | null>(null);

  // paper picker
  const [pickId, setPickId] = useState('');
  const [freeTitle, setFreeTitle] = useState('');

  const reload = useCallback(async () => {
    try {
      const all = await bridge.search(''); // the universe — for tags + counts + empty detection
      setUniverse(all.length);
      setCounts({ paper: all.filter((n) => n.note_type === 'paper').length, project: all.filter((n) => n.note_type === 'project').length, manuscript: all.filter((n) => n.note_type === 'manuscript').length });
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

  // Reference-COUNTED image GC: a file dies only when NO note still references
  // its hash. Runs AFTER the mutation commits (so the search reflects the new
  // world) — a note sharing the hash keeps the file alive. Never blocks/faults
  // the save itself: GC failure just leaves a harmless orphan.
  const gcAfter = async (candidates: string[]) => {
    if (candidates.length === 0) return;
    try {
      await gcOrphans(candidates, async (ref) => (await bridge.search(ref)).length > 0);
    } catch { /* orphan cleanup is best-effort */ }
  };

  const save = async (draft: NoteDraft) => {
    setBusy(true);
    setError(null);
    try {
      const isEdit = editing && (editing.kind === 'paper-edit' || editing.kind === 'project-edit');
      // Candidates = images that were in the note before this edit but aren't now.
      const prevBody = isEdit && 'note' in editing ? editing.note.body : '';
      const removedRefs = imageRefsIn(prevBody).filter((r) => !imageRefsIn(draft.body ?? '').includes(r));
      if (isEdit) await bridge.update(draft);
      else await bridge.create(draft);
      await gcAfter(removedRefs); // commit-first, then sweep
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
      // Capture the victim's image refs BEFORE deleting, so we know what to sweep.
      const victim = await bridge.get(id);
      const candidates = victim ? imageRefsIn(victim.body) : [];
      await bridge.remove(id);
      await gcAfter(candidates); // commit-first: a note sharing a hash keeps its file
      await afterMutation();
    } finally {
      setBusy(false);
    }
  };

  // Bulk export: every note currently SHOWN (the active search/type/tag filter),
  // joined by a --- rule. Respects the filter because it emits from `list`.
  const exportAllShown = async () => {
    if (!list || list.length === 0) return;
    setError(null);
    try {
      // saveNoteFile returns null on cancel (silent no-op) and only THROWS on a
      // real write failure — so this catch surfaces genuine failures, not cancels.
      await saveNoteFile(tagFilter ? `notes-${tagFilter}.md` : 'notes.md', notesToMarkdown(list));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not save the export');
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

  /* ---------- editor view (full-page swap, SAME persistent .an-root) ------- *
   * Both the dashboard and the editor live under ONE .an-root so the dark→light
   * fade fires once on route mount, not on every open/close (no dark flash
   * between dashboard and editor). */
  const editorPane = editing ? (
    <div className="an-main" style={{ height: '100vh' }}>
      {error && <div style={{ padding: '12px 48px 0' }}><p className="an-error" role="alert" data-testid="note-error">{error}</p></div>}
      {(editing.kind === 'paper-new' || editing.kind === 'paper-edit') ? (
        <PaperNoteEditor
          base={editing.kind === 'paper-edit' ? { id: editing.note.id, paper_id: editing.note.paper_id, paper_title: editing.note.paper_title } : editing.base}
          existing={editing.kind === 'paper-edit' ? editing.note : null}
          paperText={paperText}
          fullTextExpected={
            !!options.find(
              (o) => o.id === (editing.kind === 'paper-edit' ? editing.note.paper_id : editing.base.paper_id)
            )?.hasFullText
          }
          onSave={save}
          onDelete={editing.kind === 'paper-edit' ? () => remove(editing.note.id) : undefined}
          onClose={() => setEditing(null)}
          onError={setError}
          busy={busy}
        />
      ) : (editing.kind === 'manuscript-new' || editing.kind === 'manuscript-edit') ? (
        <ManuscriptEditor
          id={editing.kind === 'manuscript-edit' ? editing.note.id : editing.id}
          existing={editing.kind === 'manuscript-edit' ? editing.note : null}
          onSave={save}
          onDelete={editing.kind === 'manuscript-edit' ? () => remove(editing.note.id) : undefined}
          onClose={() => setEditing(null)}
          onError={setError}
          busy={busy}
        />
      ) : (
        <ProjectNoteEditor id={editing.note.id} existing={editing.note} onSave={save} onDelete={() => remove(editing.note.id)} onClose={() => setEditing(null)} busy={busy} onError={setError} />
      )}
    </div>
  ) : null;

  /* ------------------------------- render --------------------------------- */
  return (
    <div className="an-root" data-testid="note-creator" style={editing ? { display: 'block' } : undefined}>
      {editing ? editorPane : (
      <>
      {/* Navigation drawer (left sidebar) */}
      <aside className="an-sidebar">
        <button className="an-brand" onClick={() => navigate('/app')} title="Back to Gaply home">
          <h1>Gaply</h1>
          <p>Research Intelligence</p>
        </button>
        <div className="an-profile">
          <div className="an-avatar">R</div>
          <div>
            <p className="an-profile-name" style={{ margin: 0 }}>Researcher</p>
            <p className="an-profile-sub" style={{ margin: 0 }}>{universe} {universe === 1 ? 'note' : 'notes'} on device</p>
          </div>
        </div>
        <nav className="an-nav">
          <button className={`an-nav-item${typeFilter === 'all' ? ' an-active' : ''}`} data-testid="filter-all" onClick={() => setTypeFilter('all')}>
            <IcDoc /> All Notes <span className="an-nav-count">{universe}</span>
          </button>
          <button className={`an-nav-item${typeFilter === 'project' ? ' an-active' : ''}`} data-testid="filter-project" onClick={() => setTypeFilter('project')}>
            <IcBulb /> Project Notes <span className="an-nav-count">{counts.project}</span>
          </button>
          <button className={`an-nav-item${typeFilter === 'paper' ? ' an-active' : ''}`} data-testid="filter-paper" onClick={() => setTypeFilter('paper')}>
            <IcArticle /> Paper Notes <span className="an-nav-count">{counts.paper}</span>
          </button>
          <button className={`an-nav-item${typeFilter === 'manuscript' ? ' an-active' : ''}`} data-testid="filter-manuscript" onClick={() => setTypeFilter('manuscript')}>
            <IcArticle /> Research Papers <span className="an-nav-count">{counts.manuscript}</span>
          </button>
          <button className="an-nav-item" onClick={() => navigate('/app/citations')}>
            <IcQuote /> Citations
          </button>
          <div className="an-nav-section">Library</div>
          <button className="an-nav-item" onClick={() => setTagFilter(null)}>
            <IcTag /> Tags <span className="an-nav-count">{allTags.length}</span>
          </button>
        </nav>
        <div className="an-side-foot">
          <button className="an-nav-item" onClick={() => navigate('/app/settings')}>
            <IcGear /> Settings
          </button>
          <div className="an-free-pill">free · on device — your notes never leave this machine</div>
        </div>
      </aside>

      {/* Main canvas */}
      <main className="an-main">
        <header className="an-topbar">
          <div className="an-search">
            <IcSearch />
            <input placeholder="Search your library..." value={query} data-testid="notes-search" onChange={(e) => setQuery(e.target.value)} />
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <FontScale />
            <button className="an-btn-ghost" data-testid="new-manuscript" onClick={() => setEditing({ kind: 'manuscript-new', id: newId() })}>
              <IcArticle size={18} /> New Research Paper
            </button>
            <button className="an-btn-primary" onClick={() => quickRef.current?.focus()}>
              <IcAdd size={18} /> New Entry
            </button>
          </div>
        </header>

        <div className="an-content">
          {error && <p className="an-error" role="alert" data-testid="note-error">{error}</p>}

          {/* Hero / greeting — real counts, not the mock's fake ones */}
          <section className="an-greet">
            <h2>{timeGreeting()}, Researcher.</h2>
            <p>
              Your library holds {counts.paper} paper {counts.paper === 1 ? 'note' : 'notes'} and {counts.project} project {counts.project === 1 ? 'idea' : 'ideas'} — all on this device.
            </p>
          </section>

          {/* Quick-entry widgets */}
          <section className="an-widgets">
            {/* Quick Note (project quick-capture) */}
            <div className="an-widget an-widget--idea" data-testid="quick-capture">
              <div className="an-widget-head">
                <h3><IcEditNote /> Quick Note</h3>
                <span className="an-widget-kind">Freeform Sketch</span>
              </div>
              <input ref={quickRef} className="an-quick-title" placeholder="An idea, a hypothesis, a to-do…" value={quickTitle} data-testid="quick-title" onChange={(e) => setQuickTitle(e.target.value)} />
              <textarea className="an-quick-body" placeholder="Capture a fleeting thought or research lead..." value={quickBody} data-testid="quick-body" onChange={(e) => setQuickBody(e.target.value)} spellCheck={false} />
              <div className="an-widget-foot">
                <button className="an-textbtn" onClick={quickCapture} disabled={busy || (!quickTitle.trim() && !quickBody.trim())} data-testid="quick-save">Save project note</button>
              </div>
            </div>

            {/* Reference Note (paper picker: library OR free-typed) */}
            <div className="an-widget" data-testid="paper-picker">
              <div className="an-widget-head">
                <h3><IcBook /> Reference Note</h3>
                <span className="an-widget-kind">Structured Entry</span>
              </div>
              <div className="an-ref-row">
                <select className="an-ref-input" data-testid="picker-select" value={pickId} onChange={(e) => setPickId(e.target.value)}>
                  <option value="">Pick a paper from your library…</option>
                  {options.map((o) => (
                    <option key={o.id} value={o.id}>{o.title}{o.year ? ` (${o.year})` : ''}{o.hasFullText ? '  📄' : ''}</option>
                  ))}
                </select>
                <button className="an-textbtn" onClick={startFromLibrary} disabled={!pickId} data-testid="picker-start">Start notes</button>
              </div>
              {pickId && options.find((o) => o.id === pickId)?.hasFullText && (
                <p className="an-hint" data-testid="picker-fulltext">📄 full text available — it will open beside your notes.</p>
              )}
              <div className="an-ref-row">
                <input className="an-ref-input" placeholder="…or type any paper title" value={freeTitle} data-testid="picker-freetype" onChange={(e) => setFreeTitle(e.target.value)} />
                <button className="an-textbtn" onClick={startFreeTyped} disabled={!freeTitle.trim()} data-testid="picker-freestart">Start notes</button>
              </div>
              {options.length === 0 && (
                <p className="an-hint" data-testid="picker-nolib">No papers in your citation library yet — add references in the Citation Manager, or just type a title above.</p>
              )}
            </div>
          </section>

          {/* Recent scholarly work — the real note library */}
          <section>
            <div className="an-section-head">
              <h3>Recent Scholarly Work</h3>
              {list && list.length > 0 && (
                <button className="an-iconbtn" data-testid="notes-export-all" onClick={() => void exportAllShown()}>
                  <IcExport size={16} /> Export all shown (.md)
                </button>
              )}
            </div>

            {allTags.length > 0 && (
              <div className="an-chiprow" data-testid="tag-filter">
                <button className={`an-chip${tagFilter === null ? ' an-chip--on' : ''}`} data-testid="tag-all" onClick={() => setTagFilter(null)}>all tags</button>
                {allTags.map((t) => (
                  <button key={t} className={`an-chip${tagFilter === t ? ' an-chip--on' : ''}`} data-testid={`tag-${t}`} onClick={() => setTagFilter(t)}>#{t}</button>
                ))}
              </div>
            )}

            {list === null ? (
              <p className="an-empty">Loading…</p>
            ) : universe === 0 ? (
              <p className="an-empty" data-testid="note-empty">No notes yet — capture an idea, a hypothesis, a to-do, or pick a paper to annotate.</p>
            ) : list.length === 0 ? (
              <p className="an-empty" data-testid="note-nomatch">No notes match your search.</p>
            ) : (
              <div className="an-grid" data-testid="note-list">
                {list.map((n, i) => {
                  const snippet = snippetOf(n);
                  const date = dateOf(n);
                  const idea = n.note_type === 'project';
                  const manuscript = n.note_type === 'manuscript';
                  const openNote = () => setEditing(
                    n.note_type === 'paper' ? { kind: 'paper-edit', note: n }
                    : n.note_type === 'manuscript' ? { kind: 'manuscript-edit', note: n }
                    : { kind: 'project-edit', note: n }
                  );
                  return (
                    <button
                      key={n.id}
                      className={`an-card${i === 0 ? ' an-card--lg' : ''}${idea ? ' an-card--idea' : ' an-card--paper'}`}
                      data-testid={`note-row-${n.id}`}
                      onClick={openNote}
                    >
                      <div className="an-card-meta">
                        <span className={`an-badge ${idea ? 'an-badge--idea' : 'an-badge--paper'}`}>
                          <span data-testid={`note-type-${n.id}`}>{manuscript ? '📝' : idea ? '💡' : '📄'}</span> {manuscript ? 'Research Paper' : idea ? 'Idea' : 'Paper'}
                        </span>
                        {date && <span className="an-card-date">{date}</span>}
                      </div>
                      <h4>{n.title || '(untitled note)'}</h4>
                      {snippet && <p className="an-card-snippet">{snippet}</p>}
                      {n.tags.length > 0 && (
                        <div className="an-card-tags">{n.tags.map((t) => <span key={t}>#{t}</span>)}</div>
                      )}
                      <div className="an-card-paperline">
                        <span>{n.note_type === 'paper' ? (n.paper_title || 'free-typed paper') : 'project note'}</span>
                      </div>
                    </button>
                  );
                })}
              </div>
            )}
          </section>

          <div style={{ marginTop: 56 }}>
            <RecommendedToolsPanel />
          </div>
        </div>
      </main>
      </>
      )}
    </div>
  );
};

export default NoteCreatorPage;
