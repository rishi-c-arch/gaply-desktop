// Gaply — Citation Manager (F8). Mendeley/Zotero-style library — LOCAL-FIRST
// (Set 4): the local sqlite citation_library is the SOURCE OF TRUTH
// (add/list/search/tag fully offline, no sign-in); the Supabase sync is an
// OPTIONAL layer (push when signed-in + online, honest per-ref sync status,
// local data never lost to a failed sync). The manuscript never syncs.
import React, { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  AppShell,
  Badge,
  Button,
  Card,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
  ThreePanelWorkspace,
} from '../../design-system';
import { useToast, ToastProvider } from '../../design-system/Toast';
import { createCitationLibraryService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import {
  Citation,
  citationToRow,
  computeStatus,
  parseDoi,
  STATUS_ICON,
  STATUS_LABEL,
} from './citationTypes';
import { CSL_STYLES, formatBibliography, formatCitation } from './formatCitation';
import {
  applyVerification,
  makeMockRefVerify,
  RefVerifyBridge,
  TauriRefVerifyBridge,
} from './refverifyBridge';
import { mayUseCloud } from '../settings/settingsStore';
import { LocalLibrary, storedToCitation, TauriLocalLibrary } from './localLibrary';
import { CitationResolveBridge, metadataToCslItem, TauriCitationResolve } from './metadataBridge';
import { pickManuscriptPath } from '../common/pickFile';
import { isTauri } from '../../utils/isTauri';
import { exportBibliographyText, exportSerialized, saveExportToFile } from './exporters';
import './citations.css';

export interface CitationManagerPageProps {
  refverify?: RefVerifyBridge;
  /** OPTIONAL cloud sync layer (Supabase). Local sqlite is the truth. */
  citationService?: ReturnType<typeof createCitationLibraryService>;
  /** The local-first store (Tauri in prod; mock in tests). */
  localLibrary?: LocalLibrary;
  /** Set 2's verified-metadata resolver (Tauri in prod; mock in tests). */
  metadataResolver?: CitationResolveBridge;
  /** Citations auto-extracted from an analyzed manuscript (add-way #1). */
  extractedCitations?: Citation[];
  /** Pre-seed the library (tests / demo). */
  initialCitations?: Citation[];
}

type CollectionId = 'all' | 'retracted' | 'orphans' | 'manuscript';

/** Render *italic* / **bold** markers from the formatter as em/strong. */
const Formatted: React.FC<{ text: string }> = ({ text }) => {
  const parts = text.split(/(\*\*[^*]+\*\*|\*[^*]+\*)/g).filter(Boolean);
  return (
    <>
      {parts.map((p, i) => {
        if (p.startsWith('**')) return <strong key={i}>{p.slice(2, -2)}</strong>;
        if (p.startsWith('*')) return <em key={i}>{p.slice(1, -1)}</em>;
        return <React.Fragment key={i}>{p}</React.Fragment>;
      })}
    </>
  );
};

let idCounter = 0;
function newId(seed: string): string {
  return `cite-${seed.replace(/[^a-z0-9]/gi, '')}-${idCounter++}`;
}

const Inner: React.FC<CitationManagerPageProps> = ({
  refverify,
  citationService,
  localLibrary,
  metadataResolver,
  extractedCitations = [],
  initialCitations = [],
}) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const { toast } = useToast();
  const rv = useMemo(() => refverify ?? new TauriRefVerifyBridge(), [refverify]);
  const lib = useMemo(() => citationService ?? createCitationLibraryService(), [citationService]);
  const local = useMemo(() => localLibrary ?? new TauriLocalLibrary(), [localLibrary]);
  const resolver = useMemo(() => metadataResolver ?? new TauriCitationResolve(), [metadataResolver]);

  const [citations, setCitations] = useState<Citation[]>(initialCitations);
  const [collection, setCollection] = useState<CollectionId>('all');
  const [selectedId, setSelectedId] = useState<string | null>(initialCitations[0]?.id ?? null);
  const [style, setStyle] = useState<string>('apa'); // global (bulk) style
  const [doiInput, setDoiInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState('');
  const [searchIds, setSearchIds] = useState<Set<string> | null>(null);
  const [tagInput, setTagInput] = useState('');

  // LOCAL-FIRST hydrate: the sqlite library is the source of truth. Failures
  // (e.g. plain browser, no Tauri) degrade to in-memory state — never fatal.
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const rows = await local.list();
        if (!alive || rows.length === 0) return;
        setCitations((existing) => {
          const have = new Set(existing.map((c) => c.id));
          return [...existing, ...rows.filter((r) => !have.has(r.id)).map(storedToCitation)];
        });
      } catch (e) {
        console.warn('[citations] local library unavailable (in-memory only):', e);
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [local]);

  // Deterministic LOCAL search (title/author/year/DOI/tag) via the bridge.
  useEffect(() => {
    let alive = true;
    if (!query.trim()) {
      setSearchIds(null);
      return;
    }
    (async () => {
      try {
        const rows = await local.search(query);
        if (alive) setSearchIds(new Set(rows.map((r) => r.id)));
      } catch {
        // no local store (browser/tests without a bridge): filter in memory
        const q = query.toLowerCase();
        if (alive)
          setSearchIds(
            new Set(
              citations
                .filter(
                  (c) =>
                    c.csl.title.toLowerCase().includes(q) ||
                    c.csl.author.some((a) => a.family.toLowerCase().includes(q)) ||
                    (c.doi ?? '').toLowerCase().includes(q) ||
                    String(c.csl.issued?.year ?? '').includes(q) ||
                    (c.tags ?? []).some((t) => t.toLowerCase().includes(q))
                )
                .map((c) => c.id)
            )
          );
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, local]);

  const retractedCount = citations.filter((c) => c.retracted).length;

  const visible = useMemo(() => {
    const base = (() => {
      switch (collection) {
        case 'retracted':
          return citations.filter((c) => c.retracted);
        case 'orphans':
          return citations.filter((c) => computeStatus(c) === 'orphan');
        case 'manuscript':
          return citations.filter((c) => c.source === 'extracted');
        default:
          return citations;
      }
    })();
    return searchIds ? base.filter((c) => searchIds.has(c.id)) : base;
  }, [citations, collection, searchIds]);

  const selected = citations.find((c) => c.id === selectedId) ?? null;

  /** LOCAL-FIRST persist: sqlite first (the truth), then the OPTIONAL cloud
   *  push — a failed/unavailable sync NEVER loses local data, and the
   *  sync status stays honest. */
  const persistAndAdd = async (c: Citation) => {
    let status: Citation['syncStatus'] = 'local_only';
    try {
      await local.upsert(c, c.tags ?? []);
    } catch (e) {
      // No local store (plain browser) → in-memory only; still honest.
      console.warn('[citations] local write unavailable:', e);
    }
    if (session) {
      const res = await lib.add(citationToRow(session.user.id, c));
      if (res.error || res.offline) {
        status = 'pending';
        toast(`Not synced: ${res.error ?? 'cloud unavailable offline'} — kept locally`, 'assessed');
      } else {
        status = 'synced';
      }
      try {
        await local.markSync(c.id, status);
      } catch {
        /* in-memory only */
      }
    }
    setCitations((xs) => [{ ...c, syncStatus: status }, ...xs]);
    setSelectedId(c.id);
  };

  /** Tag management on the selected reference — fully local. */
  const applyTags = async (c: Citation, tags: string[]) => {
    try {
      await local.setTags(c.id, tags);
    } catch (e) {
      console.warn('[citations] local tags unavailable:', e);
    }
    setCitations((xs) => xs.map((x) => (x.id === c.id ? { ...x, tags, syncStatus: 'local_only' } : x)));
  };

  /* --------------------------- add way #2: DOI --------------------------- */
  const addByDoi = async () => {
    const doi = parseDoi(doiInput);
    if (!doi) {
      toast('Enter a valid DOI or DOI URL', 'flagged');
      return;
    }
    // F14 privacy gate — verification is the cloud path; off means no call.
    if (!mayUseCloud('citation_verification')) {
      toast('Citation verification is turned off in Settings → Sync & Privacy', 'assessed');
      return;
    }
    setBusy(true);
    try {
      const v = await rv.verify({ raw: doiInput, doi });
      if (!v.exists?.found) {
        toast('DOI not found on CrossRef/OpenAlex', 'flagged');
        return;
      }
      const base: Citation = {
        id: newId(doi),
        csl: {
          id: doi,
          type: 'article-journal',
          title: v.exists.title?.safe_text ?? doi,
          author: [],
          DOI: doi,
        },
        doi,
        retracted: false,
        source: 'doi',
      };
      let enriched = applyVerification(base, v);
      // Set 5 glue: enrich with the FULL verified metadata (authors/journal/
      // volume/pages) from the Set 2 resolver — guarded: on any failure the
      // existence-check metadata above still stands (never invented either way).
      try {
        const full = await resolver.resolve({ doi });
        if (full.status === 'verified') {
          enriched = { ...enriched, csl: { ...metadataToCslItem(full.metadata, enriched.id), id: enriched.csl.id } };
        }
      } catch (err) {
        console.warn('[citations] full-metadata enrich unavailable:', err);
      }
      await persistAndAdd(enriched);
      setDoiInput('');
      toast(enriched.retracted ? 'Added — but this work is RETRACTED' : 'Citation verified & added', enriched.retracted ? 'flagged' : 'certain');
    } catch (e) {
      // Previously swallowed: the verify bridge can reject (e.g. the backend
      // command isn't available yet). Surface a real error instead of nothing.
      const msg = e instanceof Error ? e.message : String(e);
      console.error('[citations] DOI verify failed:', msg);
      toast(`Couldn’t verify this DOI: ${msg}`, 'flagged');
    } finally {
      setBusy(false);
    }
  };

  /* ------------------------- add way #1: extracted ----------------------- */
  const importExtracted = async () => {
    for (const c of extractedCitations) await persistAndAdd(c);
    setCollection('manuscript');
    toast(`Imported ${extractedCitations.length} extracted citation(s)`, 'certain');
  };

  /* --------------------- add way #4: from a paper file ------------------- */
  const addFromPath = async (path: string) => {
    setBusy(true);
    try {
      const res = await resolver.resolve({ path });
      if (res.status === 'unverified') {
        toast(
          res.unverified_title_hint
            ? `Couldn’t verify: ${res.reason} (title hint: “${res.unverified_title_hint}”)`
            : `Couldn’t verify: ${res.reason}`,
          'assessed'
        );
        return;
      }
      const c: Citation = {
        id: newId(res.metadata.doi ?? 'paper'),
        csl: metadataToCslItem(res.metadata, newId('paper')),
        doi: res.metadata.doi,
        retracted: false,
        source: 'doi',
        provenance: [`source:${res.metadata.source}`, `matched_by:${res.metadata.matched_by}`],
      };
      await persistAndAdd(c);
      toast('Paper resolved & added from verified metadata', 'certain');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      toast(`Couldn’t resolve this paper: ${msg}`, 'flagged');
    } finally {
      setBusy(false);
    }
  };
  // Browser fallback: the <input>'s File has no usable .path in the app webview.
  const addFromFile = (file: File) => void addFromPath((file as any).path ?? file.name);
  // Tauri desktop: an ABSOLUTE path from the native dialog (the M6 fix).
  const pickAndAddFromFile = async () => {
    const p = await pickManuscriptPath(['pdf', 'docx', 'txt'], 'Paper');
    if (p) await addFromPath(p);
  };

  /* -------------------------- add way #3: manual ------------------------- */
  const addManual = async () => {
    const c: Citation = {
      id: newId('manual'),
      csl: { id: 'manual', type: 'article-journal', title: 'Untitled — edit details', author: [] },
      doi: null,
      retracted: false,
      source: 'manual',
    };
    await persistAndAdd(c);
    toast('Manual entry added — fill in the details', 'neutral');
  };

  /* ----------------------------- bulk actions --------------------------- */
  const checkAllRetractions = async () => {
    if (!mayUseCloud('citation_verification')) {
      toast('Citation verification is turned off in Settings → Sync & Privacy', 'assessed');
      return;
    }
    setBusy(true);
    try {
      const updated = await Promise.all(
        citations.map(async (c) => {
          if (!c.doi) return c;
          const v = await rv.verify({ raw: c.csl.title, doi: c.doi });
          return applyVerification(c, v);
        })
      );
      setCitations(updated);
      const nowRetracted = updated.filter((c) => c.retracted).length;
      toast(`Retraction sweep complete — ${nowRetracted} retracted`, nowRetracted ? 'flagged' : 'certain');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error('[citations] retraction sweep failed:', msg);
      toast(`Retraction re-check failed: ${msg}`, 'flagged');
    } finally {
      setBusy(false);
    }
  };

  const exportBibliography = async () => {
    // Full-CSL when the style is prepared (Set 3), legacy formatter otherwise
    // — identical seam as the preview.
    let text: string;
    try {
      text = exportBibliographyText(visible.map((c) => c.csl), style);
    } catch {
      text = formatBibliography(visible.map((c) => c.csl), style);
    }
    await saveExportToFile(`bibliography-${style}.txt`, text);
    toast('Bibliography exported', 'certain');
  };

  const exportAs = async (format: 'bibtex' | 'ris') => {
    try {
      const text = await exportSerialized(visible.map((c) => c.csl), format);
      await saveExportToFile(format === 'bibtex' ? 'library.bib' : 'library.ris', text);
      toast(`${format === 'bibtex' ? 'BibTeX' : 'RIS'} exported`, 'certain');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      toast(`Export failed: ${msg}`, 'flagged');
    }
  };

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="citation-manager">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'cite', label: 'Citation Manager', icon: '❞' },
            ]}
            activeId="cite"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="Citation Manager">
            <select
              className="gds-cite-input"
              style={{ maxWidth: 180 }}
              value={style}
              data-testid="bulk-style"
              onChange={(e) => {
                setStyle(e.target.value);
                toast(`Reformatted all to ${e.target.value.toUpperCase()}`, 'neutral');
              }}
            >
              {CSL_STYLES.map((s) => (
                <option key={s.id} value={s.id}>{`Reformat all → ${s.label}`}</option>
              ))}
            </select>
            <Button variant="secondary" onClick={() => void exportBibliography()} data-testid="export-biblio">Export bibliography</Button>
            <Button variant="secondary" onClick={() => void exportAs('bibtex')} data-testid="export-bibtex">Export BibTeX</Button>
            <Button variant="secondary" onClick={() => void exportAs('ris')} data-testid="export-ris">Export RIS</Button>
            <Button variant="secondary" onClick={checkAllRetractions} disabled={busy} data-testid="check-retractions">
              Check all for retractions
            </Button>
          </HeaderBar>
        }
      >
        <ThreePanelWorkspace
          outline={
            <Panel title="Library">
              <div className="gds-cite-collections" data-testid="collections">
                {([
                  ['all', `All citations (${citations.length})`, false],
                  ['manuscript', 'From manuscript', false],
                  ['orphans', 'Orphans', false],
                  ['retracted', `Retracted Items (${retractedCount})`, true],
                ] as Array<[CollectionId, string, boolean]>).map(([id, label, red]) => (
                  <button
                    key={id}
                    className="gds-cite-collection"
                    aria-current={collection === id}
                    data-retracted={red}
                    data-testid={`collection-${id}`}
                    onClick={() => setCollection(id)}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </Panel>
          }
          inspector={
            <Panel title="Citation detail">
              {selected ? (
                <div data-testid="detail">
                  {selected.retracted && (
                    <div className="gds-cite-retract-banner" data-testid="retract-banner">
                      🔴 This work has been RETRACTED{selected.noticeUrl ? ' — see notice.' : '.'}
                    </div>
                  )}
                  <div className="gds-cite-detail-field" style={{ marginTop: 10 }}>
                    <label htmlFor="cite-style">Preview style — any journal worldwide</label>
                    <select
                      id="cite-style"
                      className="gds-cite-input"
                      value={style}
                      data-testid="preview-style"
                      onChange={(e) => setStyle(e.target.value)}
                    >
                      {CSL_STYLES.map((s) => (
                        <option key={s.id} value={s.id}>{s.label}</option>
                      ))}
                    </select>
                  </div>
                  <div className="gds-cite-preview" data-testid="preview">
                    <Formatted text={formatCitation(selected.csl, style)} />
                  </div>
                  <div style={{ marginTop: 10 }} data-testid="cm-tags">
                    {(selected.tags ?? []).map((t) => (
                      <button
                        key={t}
                        className="gds-chat__chip"
                        title="Remove tag"
                        data-testid={`cm-tag-${t}`}
                        onClick={() => void applyTags(selected, (selected.tags ?? []).filter((x) => x !== t))}
                      >
                        {t} ✕
                      </button>
                    ))}
                    <input
                      className="gds-cite-input"
                      style={{ maxWidth: 140, marginLeft: 6 }}
                      placeholder="add tag…"
                      value={tagInput}
                      data-testid="cm-tag-input"
                      onChange={(e) => setTagInput(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && tagInput.trim()) {
                          void applyTags(selected, Array.from(new Set([...(selected.tags ?? []), tagInput.trim()])));
                          setTagInput('');
                        }
                      }}
                    />
                  </div>
                  <div style={{ marginTop: 6, fontSize: 12 }} data-testid="cm-sync-status">
                    {selected.syncStatus === 'synced'
                      ? 'Synced to your account'
                      : selected.syncStatus === 'pending'
                        ? 'Sync pending — kept locally'
                        : session
                          ? 'Local only (not yet synced)'
                          : 'Local only — sign in to enable optional sync'}
                  </div>
                  <div style={{ marginTop: 10, fontSize: 12, color: 'var(--g-text-3)' }}>
                    Status: {STATUS_LABEL[computeStatus(selected)]}
                    {selected.provenance && selected.provenance.length > 0 && (
                      <div className="gds-mono" style={{ marginTop: 6 }} data-testid="detail-provenance">
                        {selected.provenance.slice(0, 3).join('  ·  ')}
                      </div>
                    )}
                  </div>
                </div>
              ) : (
                <p style={{ color: 'var(--g-text-3)', fontSize: 13 }}>Select a citation to preview it.</p>
              )}
            </Panel>
          }
        >
          <Panel title={collection === 'retracted' ? 'Retracted Items' : 'Citations'}>
            <div className="gds-cite-add__row" style={{ marginBottom: 8 }}>
              <input
                className="gds-cite-input"
                placeholder="Search title, author, year, DOI or tag…"
                value={query}
                data-testid="cm-search"
                onChange={(e) => setQuery(e.target.value)}
              />
            </div>
            {/* add controls: three ways */}
            <div className="gds-cite-add" data-testid="add-controls">
              <div className="gds-cite-add__row">
                <input
                  className="gds-cite-input"
                  placeholder="Paste a DOI or DOI URL to verify & enrich…"
                  value={doiInput}
                  data-testid="doi-input"
                  onChange={(e) => setDoiInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && addByDoi()}
                />
                <Button onClick={addByDoi} disabled={busy} data-testid="add-doi">Add by DOI</Button>
              </div>
              <div className="gds-cite-add__row">
                <Button variant="secondary" onClick={importExtracted} disabled={extractedCitations.length === 0} data-testid="import-extracted">
                  Import from manuscript ({extractedCitations.length})
                </Button>
                <Button variant="ghost" onClick={addManual} data-testid="add-manual">+ Manual entry</Button>
                <label
                  className="gds-cite-input"
                  style={{ cursor: 'pointer' }}
                  onClick={(e) => {
                    // Tauri desktop: intercept the label click → native dialog
                    // (absolute path). Browser: fall through to the <input>.
                    if (isTauri) {
                      e.preventDefault();
                      void pickAndAddFromFile();
                    }
                  }}
                >
                  From paper file…
                  <input
                    type="file"
                    accept=".pdf,.docx,.txt"
                    style={{ display: 'none' }}
                    data-testid="add-file"
                    onChange={(e) => e.target.files?.[0] && void addFromFile(e.target.files[0])}
                  />
                </label>
              </div>
            </div>

            <div className="gds-cite-list" data-testid="citation-list">
              {visible.length === 0 ? (
                <p style={{ color: 'var(--g-text-3)', fontSize: 13 }} data-testid="empty">
                  {collection === 'retracted' ? 'No retracted items — good.' : 'No citations yet. Add one above.'}
                </p>
              ) : (
                visible.map((c) => {
                  const st = computeStatus(c);
                  return (
                    <button
                      key={c.id}
                      className="gds-cite-item"
                      aria-current={c.id === selectedId}
                      data-retracted={c.retracted}
                      data-status={st}
                      data-testid={`cite-${c.id}`}
                      onClick={() => setSelectedId(c.id)}
                    >
                      <span className="gds-cite-item__icon" title={STATUS_LABEL[st]}>{STATUS_ICON[st]}</span>
                      <span className="gds-cite-item__meta">
                        <p className="gds-cite-item__title">{c.csl.title}</p>
                        <span className="gds-cite-item__sub">
                          {c.csl.author[0]?.family ?? 'Unknown'} · {c.csl.issued?.year ?? 'n.d.'}
                          {c.retracted && ' · '}
                          {c.retracted && <Badge status="flagged">retracted</Badge>}
                        </span>
                      </span>
                    </button>
                  );
                })
              )}
            </div>
          </Panel>
        </ThreePanelWorkspace>
      </AppShell>
    </div>
  );
};

const CitationManagerPage: React.FC<CitationManagerPageProps> = (props) => (
  <ToastProvider>
    <Inner {...props} />
  </ToastProvider>
);

export default CitationManagerPage;
