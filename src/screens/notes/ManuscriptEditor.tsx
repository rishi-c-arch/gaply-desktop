// Gaply — Research Paper Writer: the manuscript editor shell.
// Set A: section nav + per-section RichBody (stable-keyed) + word counts +
// metadata + basic .docx export. Set C: a versioned venue-scaffold picker on
// create, and a "Change structure" switch that NEVER silently drops content —
// it warns which sections would be dropped and requires confirmation.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import RichBody from './RichBody';
import ScaffoldPicker from './ScaffoldPicker';
import { CitationProvider, CitationPickItem } from './CitationContext';
import { signatureOf, renderManuscriptCitations, CitationRender } from './manuscriptCitations';
import { LocalLibrary, TauriLocalLibrary, storedToCitation } from '../citations/localLibrary';
import { CslItem } from '../citations/citationTypes';
import { Note, NoteDraft } from './notesBridge';
import {
  Manuscript, Scaffold, DEFAULT_DOCX_FORMAT, manuscriptFromNote, manuscriptToDraft, newManuscript,
  switchScaffold, storedScaffoldId, wordCount,
  renameSection, addSection, deleteSection, moveSection,
} from './manuscriptModel';
import { loadScaffoldCatalog, pickScaffold } from './manuscriptScaffolds';
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
  /** The citation library (for insert-picker search + live markers). Injectable
   *  for tests; defaults to the local Tauri library. */
  library?: LocalLibrary;
}

/** Loader shell: fetch the scaffold catalog once, then hand a resolved list to
 *  the workspace. Never blocks the app — loadScaffoldCatalog falls back to the
 *  bundled generic on any failure. */
const ManuscriptEditor: React.FC<ManuscriptEditorProps> = (props) => {
  const [scaffolds, setScaffolds] = useState<Scaffold[] | null>(null);
  const [notice, setNotice] = useState('');
  useEffect(() => {
    let alive = true;
    loadScaffoldCatalog().then((c) => { if (alive) { setScaffolds(c.scaffolds); setNotice(c.globalNotice); } });
    return () => { alive = false; };
  }, []);
  if (!scaffolds) {
    return <div className="an-edit-wrap"><p className="an-empty" data-testid="ms-loading">Loading structures…</p></div>;
  }
  return <ManuscriptWorkspace {...props} scaffolds={scaffolds} notice={notice} />;
};

type Pending = { next: Scaffold; dropped: Array<{ heading: string; body: string }> };

const ManuscriptWorkspace: React.FC<ManuscriptEditorProps & { scaffolds: Scaffold[]; notice: string }> = ({
  id, existing, onSave, onDelete, onClose, busy, onError, scaffolds, notice, library,
}) => {
  // Single source of truth for the live document.
  const [manuscript, setManuscript] = useState<Manuscript>(() =>
    existing ? manuscriptFromNote(existing, pickScaffold(scaffolds, storedScaffoldId(existing))) : newManuscript(id));
  // Picker is shown first for a NEW manuscript ('new'), or on demand ('switch').
  const [pickerMode, setPickerMode] = useState<null | 'new' | 'switch'>(existing ? null : 'new');
  const [pending, setPending] = useState<Pending | null>(null); // scaffold-switch drop-warning
  const [pendingDelete, setPendingDelete] = useState<{ key: string; heading: string; words: number } | null>(null);
  const [renamingKey, setRenamingKey] = useState<string | null>(null);
  const [activeKey, setActiveKey] = useState(manuscript.sections[0]?.key ?? '');

  const scaffold = pickScaffold(scaffolds, manuscript.scaffoldId);

  /* ---- citations (Set B1): library + signature-gated live markers ---- */
  const lib = useMemo(() => library ?? new TauriLocalLibrary(), [library]);
  const [libMap, setLibMap] = useState<Map<string, CslItem>>(new Map());
  // Fingerprint = `id@updated_at` per ref. Folded into the signature so a ref
  // added/removed (id set) AND a metadata edit (updated_at bump, same id) both
  // refresh markers — e.g. an author-date "(He, 2016)" won't render stale.
  const [libFingerprint, setLibFingerprint] = useState<string[]>([]);
  const reloadLibrary = useCallback(() => {
    lib.list().then((rows) => {
      const m = new Map<string, CslItem>();
      // Force CslItem.id = the LIBRARY id so cluster refIds (= library ids) match.
      for (const r of rows) m.set(r.id, { ...storedToCitation(r).csl, id: r.id });
      setLibMap(m);
      setLibFingerprint(rows.map((r) => `${r.id}@${r.updated_at}`));
    }).catch(() => { /* offline / no library — markers just show missing */ });
  }, [lib]);
  // Refresh triggers. The DOMINANT in-app path — leaving to the Citation Manager
  // route and re-opening the manuscript — already REMOUNTS this editor with a
  // fresh library (NoteCreatorPage is a route that unmounts on navigate). Window
  // focus + document visibility are OS-level backups (Tauri window show / tab
  // return). Signature-gating makes any spurious refresh free.
  useEffect(() => {
    reloadLibrary();
    window.addEventListener('focus', reloadLibrary);
    document.addEventListener('visibilitychange', reloadLibrary);
    return () => {
      window.removeEventListener('focus', reloadLibrary);
      document.removeEventListener('visibilitychange', reloadLibrary);
    };
  }, [reloadLibrary]);

  const EMPTY_RENDER: CitationRender = { markers: new Map(), perToken: [], bibliography: '', dangling: [], hasCitations: false };
  const [citeRender, setCiteRender] = useState<CitationRender>(EMPTY_RENDER);
  // Signature folds in the library fingerprint (ids + updated_at), so it changes
  // on a citation insert/delete/reorder, a style switch, a ref added/removed, OR
  // a cited ref's metadata edit — but NOT on prose keystrokes. Gates the render.
  const signature = signatureOf(manuscript.sections, manuscript.cslStyleId, libFingerprint);
  useEffect(() => {
    let alive = true;
    renderManuscriptCitations(manuscript.sections, manuscript.cslStyleId, libMap)
      .then((r) => { if (alive) setCiteRender(r); })
      .catch(() => { /* keep the last good render */ });
    return () => { alive = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signature]);

  const markerFor = useCallback(
    (refId: string) => citeRender.markers.get(refId) ?? { marker: '', missing: !libMap.has(refId) },
    [citeRender, libMap],
  );
  const searchLib = useCallback(
    async (q: string): Promise<CitationPickItem[]> =>
      (await lib.search(q)).map((r) => ({ id: r.id, title: r.title, authors: r.authors, year: r.year })),
    [lib],
  );
  const citeCtx = useMemo(() => ({ markerFor, search: searchLib }), [markerFor, searchLib]);

  // Two-step delete (mirrors the other editors).
  const [deleteArmed, setDeleteArmed] = useState(false);
  const disarmTimer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(disarmTimer.current), []);
  const onDeleteClick = () => {
    if (!deleteArmed) {
      setDeleteArmed(true);
      disarmTimer.current = window.setTimeout(() => setDeleteArmed(false), 3000);
    } else { window.clearTimeout(disarmTimer.current); onDelete?.(); }
  };

  const active = manuscript.sections.find((s) => s.key === activeKey) ?? manuscript.sections[0];
  const setBody = (key: string, body: string) =>
    setManuscript((m) => ({ ...m, sections: m.sections.map((s) => (s.key === key ? { ...s, body } : s)) }));

  const save = () => onSave(manuscriptToDraft(manuscript));
  const exportDocx = async () => {
    onError?.(null);
    try {
      // Resolve citations fresh (same path as live display → export markers match
      // live), then export with the venue's manuscript formatting profile.
      const cites = await renderManuscriptCitations(manuscript.sections, manuscript.cslStyleId, libMap);
      await saveManuscriptDocx(manuscript, exportFileName(manuscript.title, 'manuscript').replace(/\.md$/, '.docx'), scaffold.docxFormat ?? DEFAULT_DOCX_FORMAT, cites);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : 'Could not export the .docx');
    }
  };

  /* ---- custom section editing (rename / add / delete / reorder) ---- */
  const commitRename = (key: string, value: string) => {
    const h = value.trim();
    if (h) setManuscript((m) => renameSection(m, key, h));
    setRenamingKey(null);
  };
  const onAddSection = () => {
    const idx = manuscript.sections.findIndex((s) => s.key === activeKey);
    const at = idx < 0 ? manuscript.sections.length : idx + 1;
    const nm = addSection(manuscript, at, 'New Section');
    setManuscript(nm);
    setActiveKey(nm.sections[at].key);
    setRenamingKey(nm.sections[at].key); // let the user name it immediately
  };
  const requestDelete = (key: string) => {
    const s = manuscript.sections.find((x) => x.key === key);
    if (!s) return;
    if (s.body.trim()) setPendingDelete({ key, heading: s.heading, words: wordCount(s.body) }); // warn — never silent
    else applyDelete(key);
  };
  const applyDelete = (key: string) => {
    setManuscript((m) => {
      const next = deleteSection(m, key);
      if (activeKey === key) setActiveKey(next.sections[0]?.key ?? '');
      return next;
    });
    setPendingDelete(null);
  };
  const move = (key: string, dir: -1 | 1) => setManuscript((m) => moveSection(m, key, dir));

  // Apply a resolved manuscript from a scaffold switch/new pick.
  const applyManuscript = (m: Manuscript) => {
    setManuscript(m);
    setActiveKey(m.sections[0]?.key ?? '');
    setPickerMode(null);
    setPending(null);
  };

  const onPick = (next: Scaffold) => {
    if (pickerMode === 'new') { applyManuscript(newManuscript(id, next)); return; }
    // switch mode
    if (next.id === manuscript.scaffoldId) { setPickerMode(null); return; }
    const { manuscript: switched, dropped } = switchScaffold(manuscript, next);
    if (dropped.length > 0) setPending({ next, dropped }); // warn — never silent
    else applyManuscript(switched);
  };

  const confirmSwitch = () => { if (pending) applyManuscript(switchScaffold(manuscript, pending.next).manuscript); };

  const totalWords = manuscript.sections.reduce((n, s) => n + wordCount(s.body), 0);

  /* ---- the venue-scaffold picker (create OR switch) ---- */
  if (pickerMode) {
    return (
      <div className="an-edit-wrap" data-testid="manuscript-editor">
        {pending && (
          <div className="an-ms-dropwarn" role="alertdialog" data-testid="ms-dropwarn">
            <p><b>Switching to {pending.next.label}</b> would drop your writing in {pending.dropped.length} section{pending.dropped.length > 1 ? 's' : ''} the new structure doesn’t have:</p>
            <ul>{pending.dropped.map((d) => <li key={d.heading}><b>{d.heading}</b> — {wordCount(d.body)} words</li>)}</ul>
            <p>Copy that text elsewhere first if you need it. This can’t be undone.</p>
            <div className="an-ms-dropwarn-actions">
              <button className="an-ghostbtn" data-testid="ms-dropwarn-cancel" onClick={() => setPending(null)}>Keep current structure</button>
              <button className="an-deletebtn an-deletebtn--armed" data-testid="ms-dropwarn-confirm" onClick={confirmSwitch}>Switch anyway (drop {pending.dropped.length})</button>
            </div>
          </div>
        )}
        <ScaffoldPicker
          scaffolds={scaffolds}
          notice={notice}
          currentId={pickerMode === 'switch' ? manuscript.scaffoldId : undefined}
          title={pickerMode === 'new' ? 'Choose a structure for your paper' : 'Change structure'}
          confirmLabel={pickerMode === 'new' ? 'Start writing' : 'Switch to this'}
          onPick={onPick}
          onCancel={pickerMode === 'switch' ? () => { setPickerMode(null); setPending(null); } : (existing ? undefined : onClose)}
        />
      </div>
    );
  }

  /* ---- the editing surface ---- */
  return (
    <CitationProvider value={citeCtx}>
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

        {/* Honest structure banner + change-structure + verify link. */}
        <div className="an-ms-notice" data-testid="ms-notice">
          Structure: <b>{scaffold.label}</b> (Gaply’s own scaffold, not an official template).{' '}
          <a href={scaffold.publisherAuthorUrl} target="_blank" rel="noreferrer noopener" data-testid="ms-verify">Verify current requirements ↗</a>{' · '}
          <button className="an-linkbtn" data-testid="ms-change-structure" onClick={() => setPickerMode('switch')}>Change structure</button>
          <br />Gaply formats your <b>structure and references</b>; the publisher typesets the final camera-ready layout.
          <br /><span data-testid="ms-export-note">Exports a clean single-column submission manuscript — for camera-ready typesetting after acceptance, use your publisher’s official template or Overleaf.</span>
        </div>

        {pendingDelete && (
          <div className="an-ms-dropwarn" role="alertdialog" data-testid="ms-deletewarn">
            <p>Delete <b>{pendingDelete.heading}</b>? It has <b>{pendingDelete.words} words</b> of your writing — this can’t be undone.</p>
            <div className="an-ms-dropwarn-actions">
              <button className="an-ghostbtn" data-testid="ms-deletewarn-cancel" onClick={() => setPendingDelete(null)}>Keep it</button>
              <button className="an-deletebtn an-deletebtn--armed" data-testid="ms-deletewarn-confirm" onClick={() => applyDelete(pendingDelete.key)}>Delete section</button>
            </div>
          </div>
        )}

        <div className="an-canvas an-canvas--ms">
          <section className="an-ms-meta">
            <label className="an-label">Manuscript Title</label>
            <input className="an-title-input" value={manuscript.title} placeholder="Your paper’s title…" data-testid="ms-title" onChange={(e) => setManuscript((m) => ({ ...m, title: e.target.value }))} />
            <label className="an-label">Authors</label>
            <input className="an-tags-input an-ms-authors" value={manuscript.authors} placeholder="e.g. Ada Lovelace, Alan Turing" data-testid="ms-authors" onChange={(e) => setManuscript((m) => ({ ...m, authors: e.target.value }))} />
            <label className="an-label">Affiliations <span className="an-ms-opt">optional</span></label>
            <input className="an-tags-input an-ms-authors" value={manuscript.affiliations} placeholder="e.g. ¹Analytical Engine Lab · ²Bletchley Park" data-testid="ms-affiliations" onChange={(e) => setManuscript((m) => ({ ...m, affiliations: e.target.value }))} />
            <label className="an-label">Corresponding author <span className="an-ms-opt">optional</span></label>
            <input className="an-tags-input an-ms-authors" value={manuscript.correspondingAuthor} placeholder="e.g. ada@example.edu" data-testid="ms-corresponding" onChange={(e) => setManuscript((m) => ({ ...m, correspondingAuthor: e.target.value }))} />
          </section>

          <div className="an-ms-body">
            <nav className="an-ms-nav" data-testid="ms-nav">
              {manuscript.sections.map((s, i) => (
                <div key={s.key} className={`an-ms-navrow${s.key === activeKey ? ' an-active' : ''}`}>
                  {renamingKey === s.key ? (
                    <input
                      className="an-ms-rename" autoFocus defaultValue={s.heading}
                      data-testid={`ms-rename-input-${s.key}`}
                      onKeyDown={(e) => { if (e.key === 'Enter') commitRename(s.key, e.currentTarget.value); if (e.key === 'Escape') setRenamingKey(null); }}
                      onBlur={(e) => commitRename(s.key, e.currentTarget.value)}
                    />
                  ) : (
                    <button className={`an-ms-navitem${s.key === activeKey ? ' an-active' : ''}`} data-testid={`ms-nav-${s.key}`} onClick={() => setActiveKey(s.key)} onDoubleClick={() => setRenamingKey(s.key)}>
                      <span>{s.heading}{s.required ? '' : ' ·'}</span>
                      <span className="an-ms-wc">{wordCount(s.body)}</span>
                    </button>
                  )}
                  {s.key === activeKey && renamingKey !== s.key && (
                    <div className="an-ms-navctl">
                      <button title="Move up" data-testid={`ms-up-${s.key}`} disabled={i === 0} onClick={() => move(s.key, -1)}>↑</button>
                      <button title="Move down" data-testid={`ms-down-${s.key}`} disabled={i === manuscript.sections.length - 1} onClick={() => move(s.key, 1)}>↓</button>
                      <button title="Rename" data-testid={`ms-rename-${s.key}`} onClick={() => setRenamingKey(s.key)}>✎</button>
                      <button title="Delete section" className="an-ms-navctl-del" data-testid={`ms-del-${s.key}`} onClick={() => requestDelete(s.key)}>✕</button>
                    </div>
                  )}
                </div>
              ))}
              <button className="an-ms-addsec" data-testid="ms-add-section" onClick={onAddSection}>+ Add section</button>
              <div className="an-ms-total" data-testid="ms-total-words">{totalWords} words total</div>
            </nav>

            {active && (
              <div className="an-ms-section">
                <div className="an-ms-section-head">
                  <h2>{active.heading}</h2>
                  <span className="an-ms-guidance" data-testid="ms-guidance">
                    {active.guidance}{active.typicalWords ? ` (typically ${active.typicalWords})` : ''}
                  </span>
                </div>
                {/* The References section auto-generates from citations. When any
                    exist, show an HONEST banner + a read-only preview instead of an
                    editable box — so the transition is never a silent overwrite. */}
                {active.key === 'references' && citeRender.hasCitations ? (
                  <div data-testid="ms-refs-auto">
                    <div className="an-ms-notice" data-testid="ms-refs-banner">
                      📚 <b>Auto-generated from your citations</b> in the <b>{manuscript.cslStyleId}</b> style — this list, not any text you type here, is what exports. Manage references in <b>Citations</b>.
                    </div>
                    <div className="an-refs-preview" data-testid="ms-refs-preview">
                      {citeRender.bibliography.split('\n').filter(Boolean).map((line, i) => (<p key={i}>{line}</p>))}
                    </div>
                  </div>
                ) : (
                  /* Stable key = section key: switching remounts cleanly, no bleed.
                     withCitations enables the ⌘⇧C picker + live in-text markers. */
                  <RichBody key={active.key} value={active.body} onChange={(md) => setBody(active.key, md)} placeholder={`Write the ${active.heading.toLowerCase()}…`} testid={`ms-body-${active.key}`} withCitations />
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
    </CitationProvider>
  );
};

export default ManuscriptEditor;
