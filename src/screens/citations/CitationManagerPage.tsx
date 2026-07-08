// Gaply — Citation Manager (F8). Mendeley/Zotero-style library. Citation
// METADATA syncs to Supabase (citation_library, RLS); the manuscript never
// does. Three panels: collections · citation list (status icons) · detail with
// a live CSL-formatted preview + style switcher.
import React, { useMemo, useState } from 'react';
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
import './citations.css';

export interface CitationManagerPageProps {
  refverify?: RefVerifyBridge;
  citationService?: ReturnType<typeof createCitationLibraryService>;
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
  extractedCitations = [],
  initialCitations = [],
}) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const { toast } = useToast();
  const rv = useMemo(() => refverify ?? new TauriRefVerifyBridge(), [refverify]);
  const lib = useMemo(() => citationService ?? createCitationLibraryService(), [citationService]);

  const [citations, setCitations] = useState<Citation[]>(initialCitations);
  const [collection, setCollection] = useState<CollectionId>('all');
  const [selectedId, setSelectedId] = useState<string | null>(initialCitations[0]?.id ?? null);
  const [style, setStyle] = useState<string>('apa'); // global (bulk) style
  const [doiInput, setDoiInput] = useState('');
  const [busy, setBusy] = useState(false);

  const retractedCount = citations.filter((c) => c.retracted).length;

  const visible = useMemo(() => {
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
  }, [citations, collection]);

  const selected = citations.find((c) => c.id === selectedId) ?? null;

  /** Persist metadata (no manuscript text) + add to local state. */
  const persistAndAdd = async (c: Citation) => {
    if (session) {
      const res = await lib.add(citationToRow(session.user.id, c));
      if (res.error) toast(`Not synced: ${res.error}`, 'assessed');
    }
    setCitations((xs) => [c, ...xs]);
    setSelectedId(c.id);
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
      const enriched = applyVerification(base, v);
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

  const exportBibliography = () => {
    const text = formatBibliography(visible.map((c) => c.csl), style);
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `bibliography-${style}.txt`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    toast('Bibliography exported', 'certain');
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
            <Button variant="secondary" onClick={exportBibliography} data-testid="export-biblio">Export bibliography</Button>
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
