// Gaply — Citation Manager (F8). Mendeley/Zotero-style library — LOCAL-FIRST
// (Set 4): the local sqlite citation_library is the SOURCE OF TRUTH
// (add/list/search/tag fully offline, no sign-in); the Supabase sync is an
// OPTIONAL layer (push when signed-in + online, honest per-ref sync status,
// local data never lost to a failed sync). The manuscript never syncs.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  AppShell,
  Badge,
  BadgeStatus,
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
  verificationState,
  VerificationState,
  VERIFY_LABEL,
} from './citationTypes';
import { formatBibliography, formatCitation, canFormatStyle } from './formatCitation';
import { prepareStyle, isStyleReady } from './cslEngine';
import { StylePicker } from './StylePicker';
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
import { findDuplicate, normalizeDoi } from './dedupe';
import { planImport, detectFormat, ImportPlan, SkippedEntry, IMPORT_ENTRY_CAP } from './importCitations';
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

/** Which entries the online-verify pass should target: anything not already
 *  CrossRef-verified that carries a DOI to check. Source-agnostic — a manual or
 *  extracted entry with a DOI is a valid target, not just imported ones. The
 *  displayed state (verified / unverified / not-found / check-failed) comes from
 *  verificationState(); this predicate just picks what's still worth checking. */
const needsOnlineVerify = (c: Citation): boolean =>
  verificationState(c) !== 'verified' && Boolean(c.doi ?? c.csl.DOI);

/** A compact citation line (title · authors · year · DOI) for the review panel. */
const CiteLine: React.FC<{ c: Citation }> = ({ c }) => {
  const authors = (c.csl.author ?? []).map((a) => a.family).filter(Boolean);
  const doi = c.doi ?? c.csl.DOI ?? null;
  return (
    <div className="gds-cite-review__cite">
      <p className="gds-cite-review__title">{c.csl.title || 'Untitled'}</p>
      <p className="gds-cite-review__sub">
        {authors.slice(0, 3).join(', ') || 'Unknown'}
        {authors.length > 3 ? ' et al.' : ''} · {c.csl.issued?.year ?? 'n.d.'}
        {doi ? ` · ${doi}` : ''}
      </p>
    </div>
  );
};

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
  // Set 2c-i: bump to re-render once a style's .csl has loaded, so formatCitation
  // re-routes from the legacy fallback to real citeproc. `styleTick` is a version
  // counter — reading it in the preview subscribes it to style readiness.
  const [styleTick, setStyleTick] = useState(0);
  // Non-null when the selected catalog style failed to load (honest error, no
  // fallback for a non-legacy style → never a silently-wrong render).
  const [styleError, setStyleError] = useState<string | null>(null);
  const [doiInput, setDoiInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState('');
  const [searchIds, setSearchIds] = useState<Set<string> | null>(null);
  const [tagInput, setTagInput] = useState('');
  // Import (Set 2b-i)
  const [importing, setImporting] = useState(false);
  const [importProgress, setImportProgress] = useState<{ done: number; total: number } | null>(null);
  const [importReport, setImportReport] = useState<ImportPlan | null>(null);
  // Fuzzy (Tier-2) review queue (Set 2b-ii). These are ALREADY applied (keep-both
  // default), so abandoning the panel loses nothing; the panel only lets the user
  // Skip (remove) confirmed dups. Never a gate.
  const [reviewQueue, setReviewQueue] = useState<SkippedEntry[]>([]);
  // Online verify pass (Set 2b-iii)
  const [verifying, setVerifying] = useState(false);
  const [verifyProgress, setVerifyProgress] = useState<{ done: number; total: number } | null>(null);
  const verifyCancelRef = useRef(false);

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
  // Set 2c-i: load the selected style's .csl (offline app asset). On success,
  // bump styleTick so the preview/export re-render through real citeproc. On
  // failure (no asset origin in a plain browser / jsdom), the legacy 8-style
  // formatter stays — pre-prepare fallback, no regression while a style loads.
  useEffect(() => {
    let cancelled = false;
    setStyleError(null);
    prepareStyle(style)
      .then(() => {
        if (!cancelled) setStyleTick((t) => t + 1);
      })
      .catch(() => {
        if (cancelled) return;
        // A legacy id still formats via the hand-rolled fallback; a catalog
        // style with no .csl has none → honest error, never a wrong render.
        if (!canFormatStyle(style)) setStyleError(style);
      });
    return () => {
      cancelled = true;
    };
  }, [style]);

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
  const persistAndAdd = async (c: Citation): Promise<'added' | 'duplicate'> => {
    // Dedupe (Set 2a) — the shared safety net every add-path routes through.
    // A DOI-exact match is unambiguous → auto-skip, VISIBLY (toast + select the
    // existing one). A fuzzy title+year match does NOT block a deliberate single
    // add (default keep-both); batch import (Set 2b) surfaces fuzzy for review.
    const dup = findDuplicate(c, citations);
    if (dup?.tier === 'doi') {
      toast('Already in your library — not added again', 'assessed');
      setSelectedId(dup.match.id);
      return 'duplicate';
    }
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
    return 'added';
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
    // Dedupe (Set 2a): already in the library? Skip before any network OR consent
    // — a DOI-exact match needs neither a fetch nor the cloud gate.
    const nd = normalizeDoi(doi);
    const existing = citations.find((e) => normalizeDoi(e.doi ?? e.csl.DOI ?? null) === nd);
    if (existing) {
      toast('Already in your library — not added again', 'assessed');
      setSelectedId(existing.id);
      setDoiInput('');
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
      const r = await persistAndAdd(enriched);
      setDoiInput('');
      if (r === 'added') {
        toast(enriched.retracted ? 'Added — but this work is RETRACTED' : 'Citation verified & added', enriched.retracted ? 'flagged' : 'certain');
      }
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
    // Dedupe against the EXISTING library (Set 2a). Batch-internal dedupe (the
    // same entry twice within extractedCitations) is Set 2b's import loop; here
    // the snapshot of `citations` is the pre-loop library.
    let added = 0;
    let dup = 0;
    for (const c of extractedCitations) {
      (await persistAndAdd(c)) === 'added' ? (added += 1) : (dup += 1);
    }
    setCollection('manuscript');
    toast(`Imported ${added} extracted citation(s)${dup ? `, ${dup} already present` : ''}`, 'certain');
  };

  /* --------------------- add way #4: from a paper file ------------------- */
  const addFromPath = async (path: string) => {
    // F14 privacy gate — resolving a paper's DOI hits CrossRef/OpenAlex; off
    // means no call. Identical gate + treatment to add-by-DOI and the
    // retraction sweep (same lane, same network, one door was left unlocked).
    if (!mayUseCloud('citation_verification')) {
      toast('Citation verification is turned off in Settings → Sync & Privacy', 'assessed');
      return;
    }
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
      const r = await persistAndAdd(c);
      if (r === 'added') toast('Paper resolved & added from verified metadata', 'certain');
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

  /* ------------------ import (Set 2b-i): .bib / .ris / .json -------------- */
  // PARSE + DEDUPE + STORE are fully LOCAL — import never touches the network,
  // consent state is irrelevant here (online verification is Set 2b-iii).
  const importInputRef = useRef<HTMLInputElement>(null);

  // Read the picked file's TEXT: the client-side parse needs text, not a path.
  // Tauri → the scoped read_import_file command; browser → File.text().
  const readImportText = async (fileOrPath: File | string): Promise<string> => {
    if (typeof fileOrPath === 'string') {
      const { invoke } = await import('@tauri-apps/api/core');
      return invoke<string>('read_import_file', { path: fileOrPath });
    }
    return fileOrPath.text();
  };

  // Batch-apply survivors: local upsert + ONE state update, marked local_only
  // (no per-entry cloud sync — a 400-entry import is not 400 network calls).
  const applyImported = async (cites: Citation[]) => {
    for (const c of cites) {
      try {
        await local.upsert(c, c.tags ?? []);
      } catch {
        /* no local store (browser) → in-memory only */
      }
    }
    if (cites.length) {
      setCitations((xs) => [...cites.map((c) => ({ ...c, syncStatus: 'local_only' as const })), ...xs]);
    }
  };

  const runImport = async (fileOrPath: File | string) => {
    const fmt = detectFormat(typeof fileOrPath === 'string' ? fileOrPath : fileOrPath.name);
    if (!fmt) {
      toast('Import a .bib, .ris, or .json file', 'flagged');
      return;
    }
    setImporting(true);
    setImportProgress({ done: 0, total: 0 });
    try {
      const text = await readImportText(fileOrPath);
      const plan = await planImport(text, fmt, citations, {
        onProgress: (done, total) => setImportProgress({ done, total }),
      });
      // Apply added + review (keep-both default). Tier-1 skipped are NOT applied
      // — inspectable, with Add-anyway. All local; no network, ever.
      await applyImported([...plan.added, ...plan.review.map((r) => r.candidate)]);
      setImportReport(plan);
      setReviewQueue(plan.review); // the fuzzy set, applied but open to refinement

      if (plan.capped) {
        toast(`Imported the first ${IMPORT_ENTRY_CAP} of ${plan.total} — import the rest in a second file`, 'assessed');
      }
    } catch (e) {
      toast(`Import failed: ${e instanceof Error ? e.message : String(e)}`, 'flagged');
    } finally {
      setImporting(false);
      setImportProgress(null);
    }
  };

  const pickAndImport = async () => {
    const p = await pickManuscriptPath(['bib', 'bibtex', 'ris', 'json'], 'Citations');
    if (p) await runImport(p);
  };

  // Override a Tier-1 auto-skip: force the entry in as a NEW row (a real,
  // user-chosen duplicate) and drop it from the skipped list so the report stays honest.
  const addAnyway = async (candidate: Citation) => {
    await applyImported([{ ...candidate, id: newId('import') }]);
    setImportReport((r) => (r ? { ...r, skipped: r.skipped.filter((s) => s.candidate.id !== candidate.id) } : r));
    toast('Added anyway — a duplicate now exists in your library', 'assessed');
  };

  /* ------- fuzzy review (Set 2b-ii): refine the keep-both defaults --------- */
  // Remove a citation that was kept-both but is a real duplicate.
  const removeCitations = async (ids: string[]) => {
    for (const id of ids) {
      try {
        await local.remove(id);
      } catch {
        /* in-memory only */
      }
    }
    const set = new Set(ids);
    setCitations((xs) => xs.filter((c) => !set.has(c.id)));
  };
  const reviewSkip = async (candidateId: string) => {
    await removeCitations([candidateId]);
    setReviewQueue((q) => q.filter((r) => r.candidate.id !== candidateId));
  };
  const reviewKeep = (candidateId: string) =>
    setReviewQueue((q) => q.filter((r) => r.candidate.id !== candidateId)); // it stays (already applied)
  const reviewSkipAll = async () => {
    await removeCitations(reviewQueue.map((r) => r.candidate.id));
    setReviewQueue([]);
  };
  const reviewKeepAll = () => setReviewQueue([]); // all stay — the default, made explicit

  /* ------- online verify pass (Set 2b-iii): imported-unverified → verified -- */
  // Update one citation in place (same id) after a verify result.
  const persistUpdate = async (c: Citation) => {
    try {
      await local.upsert(c, c.tags ?? []);
    } catch {
      /* in-memory only */
    }
    setCitations((xs) => xs.map((x) => (x.id === c.id ? { ...c, syncStatus: 'local_only' as const } : x)));
  };

  const verifyImported = async () => {
    // Same gate + string as add-by-DOI — off means no call.
    if (!mayUseCloud('citation_verification')) {
      toast('Citation verification is turned off in Settings → Sync & Privacy', 'assessed');
      return;
    }
    const targets = citations.filter(needsOnlineVerify);
    if (targets.length === 0) return;
    setVerifying(true);
    verifyCancelRef.current = false;
    setVerifyProgress({ done: 0, total: targets.length });
    let done = 0;
    for (const c of targets) {
      // CANCELLATION SEMANTICS — the OPPOSITE of AI Check's: partial VERIFICATION
      // is valid. Each entry's CrossRef result stands alone (verified/not-found/
      // failed), so on cancel every entry checked so far KEEPS its state; only the
      // not-yet-reached entries stay untouched. (AI Check discards partials because
      // a partial DOCUMENT score is not a valid signal; a per-entry check is.)
      if (verifyCancelRef.current) break;
      const doi = (c.doi ?? c.csl.DOI)!;
      try {
        const v = await rv.verify({ raw: c.csl.title || doi, doi });
        if (v.exists?.found) {
          let updated = applyVerification(c, v); // gains CrossRef provenance + retraction
          updated = { ...updated, verifyOutcome: undefined };
          // Full-metadata enrich (parity with add-by-DOI; guarded — existence stands on failure).
          try {
            const full = await resolver.resolve({ doi });
            if (full.status === 'verified') {
              updated = { ...updated, csl: { ...metadataToCslItem(full.metadata, updated.id), id: updated.csl.id } };
            }
          } catch {
            /* the existence-check metadata already stands */
          }
          await persistUpdate(updated);
        } else {
          // CrossRef reached, DOI does not resolve — a real problem for a researcher.
          await persistUpdate({ ...c, verifyOutcome: 'not_found' });
        }
      } catch {
        // Network/transport error — transient, retriable; NOT the same as not-found.
        await persistUpdate({ ...c, verifyOutcome: 'check_failed' });
      }
      done += 1;
      setVerifyProgress({ done, total: targets.length });
    }
    const stopped = verifyCancelRef.current;
    setVerifying(false);
    setVerifyProgress(null);
    toast(
      stopped
        ? `Verification stopped — ${done} checked (kept), the rest unchanged`
        : `Verified ${done} ${done === 1 ? 'entry' : 'entries'} online`,
      'certain',
    );
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
    const r = await persistAndAdd(c);
    if (r === 'added') toast('Manual entry added — fill in the details', 'neutral');
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
            <StylePicker
              value={style}
              testid="bulk-style"
              onSelect={(id) => {
                setStyle(id);
                toast(`Reformatted all to ${id}`, 'neutral');
              }}
            />
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
                    <label>Preview style — any journal worldwide</label>
                    <StylePicker value={style} testid="preview-style" onSelect={setStyle} />
                  </div>
                  <div
                    className="gds-cite-preview"
                    data-testid="preview"
                    data-style-source={isStyleReady(style) ? 'citeproc' : 'legacy'}
                    data-tick={styleTick}
                  >
                    {styleError ? (
                      <p className="gds-style-picker__error" data-testid="preview-error">
                        Couldn’t load the “{styleError}” style — it isn’t in the bundled set. Pick another.
                      </p>
                    ) : !canFormatStyle(style) ? (
                      <p className="gds-style-picker__note" data-testid="preview-loading">Loading style…</p>
                    ) : (
                      <Formatted text={formatCitation(selected.csl, style)} />
                    )}
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
                    <div data-testid="detail-metadata">Metadata: {STATUS_LABEL[computeStatus(selected)]}</div>
                    <div data-testid="detail-verification">Verification: {VERIFY_LABEL[verificationState(selected)]}</div>
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
              <div className="gds-cite-add__row">
                <Button
                  variant="secondary"
                  data-testid="import-file"
                  disabled={importing}
                  onClick={() => (isTauri ? void pickAndImport() : importInputRef.current?.click())}
                >
                  {importing
                    ? `Importing… ${importProgress?.done ?? 0}/${importProgress?.total ?? 0}`
                    : 'Import .bib / .ris / .json'}
                </Button>
                <input
                  ref={importInputRef}
                  type="file"
                  accept=".bib,.bibtex,.ris,.json"
                  style={{ display: 'none' }}
                  data-testid="import-input"
                  onChange={(e) => e.target.files?.[0] && void runImport(e.target.files[0])}
                />
              </div>
            </div>

            {importReport && (
              <div className="gds-cite-import-report" data-testid="import-report">
                <p className="gds-cite-import-tally" data-testid="import-tally">
                  {importReport.added.length} added
                  {importReport.review.length > 0 && ` · ${importReport.review.length} kept as possible duplicates`}
                  {' · '}{importReport.skipped.length} skipped as duplicates
                  {' · '}{importReport.failed.length} failed to parse
                  {importReport.capped && ` · capped at ${IMPORT_ENTRY_CAP} of ${importReport.total}`}
                </p>
                {importReport.skipped.length > 0 && (
                  <details data-testid="import-skipped">
                    <summary>{importReport.skipped.length} skipped as duplicates — inspect</summary>
                    {importReport.skipped.map((s, i) => (
                      <div key={i} className="gds-cite-import-skip" data-testid="import-skipped-item">
                        <span>
                          {s.candidate.csl.title || s.candidate.doi || 'Untitled'} — matches “{s.match.csl.title}”
                        </span>
                        <Button variant="ghost" data-testid="import-add-anyway" onClick={() => void addAnyway(s.candidate)}>
                          Add anyway
                        </Button>
                      </div>
                    ))}
                  </details>
                )}
                {importReport.failed.length > 0 && (
                  <details data-testid="import-failed">
                    <summary>{importReport.failed.length} failed to parse — why</summary>
                    {importReport.failed.map((f, i) => (
                      <div key={i} className="gds-cite-import-fail">
                        <code>{f.raw.slice(0, 80)}</code> — {f.error}
                      </div>
                    ))}
                  </details>
                )}
                <Button variant="ghost" data-testid="import-dismiss" onClick={() => setImportReport(null)}>
                  Dismiss
                </Button>
              </div>
            )}

            {reviewQueue.length > 0 && (
              <div className="gds-cite-review" data-testid="review-panel">
                <div className="gds-cite-review__head">
                  <span data-testid="review-count">
                    {reviewQueue.length} to review — kept by default (each looks similar to an entry you already have)
                  </span>
                  <span className="gds-cite-review__bulk">
                    <Button variant="ghost" data-testid="review-keep-all" onClick={reviewKeepAll}>
                      Keep all both
                    </Button>
                    <Button variant="ghost" data-testid="review-skip-all" onClick={() => void reviewSkipAll()}>
                      Skip all
                    </Button>
                    <Button variant="secondary" data-testid="review-done" onClick={reviewKeepAll}>
                      Done
                    </Button>
                  </span>
                </div>
                <div className="gds-cite-review__list">
                  {reviewQueue.map((r) => (
                    <div key={r.candidate.id} className="gds-cite-review__item" data-testid="review-item">
                      <div className="gds-cite-review__cols">
                        <div className="gds-cite-review__col">
                          <span className="gds-cite-review__tag">In your library</span>
                          <CiteLine c={r.match} />
                        </div>
                        <div className="gds-cite-review__col">
                          <span className="gds-cite-review__tag">Imported</span>
                          <CiteLine c={r.candidate} />
                        </div>
                      </div>
                      <div className="gds-cite-review__actions">
                        <Button variant="ghost" data-testid="review-keep" onClick={() => reviewKeep(r.candidate.id)}>
                          Keep both
                        </Button>
                        <Button variant="ghost" data-testid="review-skip" onClick={() => void reviewSkip(r.candidate.id)}>
                          Skip
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {(() => {
              const n = citations.filter(needsOnlineVerify).length;
              if (n === 0 && !verifying) return null;
              const off = !mayUseCloud('citation_verification');
              return (
                <div className="gds-cite-verify-bar" data-testid="verify-bar">
                  {verifying ? (
                    <>
                      <span data-testid="verify-progress">
                        Verifying {verifyProgress?.done ?? 0} of {verifyProgress?.total ?? 0}…
                      </span>
                      <Button variant="ghost" data-testid="verify-cancel" onClick={() => { verifyCancelRef.current = true; }}>
                        Cancel
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button variant="secondary" data-testid="verify-imported" disabled={off} onClick={() => void verifyImported()}>
                        Verify {n} online
                      </Button>
                      {off && (
                        <span className="gds-cite-verify-note" data-testid="verify-off-note">
                          Citation verification is turned off in Settings → Sync &amp; Privacy
                        </span>
                      )}
                    </>
                  )}
                </div>
              );
            })()}

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
                          {' · '}
                          {(() => {
                            // Axis B — verification, shown for EVERY source. A complete
                            // hand-typed entry reads "not verified online", never "verified".
                            const vs = verificationState(c);
                            const tone: Record<VerificationState, BadgeStatus> = {
                              verified: 'certain',
                              unverified: 'neutral',
                              not_found: 'flagged',
                              check_failed: 'assessed',
                            };
                            const tid: Record<VerificationState, string> = {
                              verified: 'verified-badge',
                              unverified: 'unverified-badge',
                              not_found: 'badge-not-found',
                              check_failed: 'badge-check-failed',
                            };
                            return (
                              <Badge status={tone[vs]} data-testid={tid[vs]}>
                                {VERIFY_LABEL[vs]}
                              </Badge>
                            );
                          })()}
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
