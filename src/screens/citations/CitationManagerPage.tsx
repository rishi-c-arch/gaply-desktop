// Gaply — Citation Manager (F8). Mendeley/Zotero-style library — LOCAL-FIRST
// (Set 4): the local sqlite citation_library is the SOURCE OF TRUTH
// (add/list/search/tag fully offline, no sign-in); the Supabase sync is an
// OPTIONAL layer (push when signed-in + online, honest per-ref sync status,
// local data never lost to a failed sync). The manuscript never syncs.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useToast, ToastProvider } from '../../design-system/Toast';
import { createCitationLibraryService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { useTheme } from '../../contexts/ThemeContext';
import {
  Citation,
  citationToRow,
  computeStatus,
  parseDoi,
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
  RefVerifyBridge,
  TauriRefVerifyBridge,
} from './refverifyBridge';
import { mayUseCloud } from '../settings/settingsStore';
import { LocalLibrary, storedToCitation, TauriLocalLibrary } from './localLibrary';
import { CitationResolveBridge, metadataToCslItem, TauriCitationResolve } from './metadataBridge';
import { pickManuscriptPath } from '../common/pickFile';
import { isTauri } from '../../utils/isTauri';
import { exportBibliographyText, exportSerialized, saveExportToFile } from './exporters';
import { findDuplicate, normalizeDoi, applyEnrichment, ENRICH_FIELD_LABEL } from './dedupe';
import { planImport, detectFormat, ImportPlan, SkippedEntry, EnrichCandidate, IMPORT_ENTRY_CAP } from './importCitations';
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
    <div className="cm-review-cite">
      <p className="cm-review-title">{c.csl.title || 'Untitled'}</p>
      <p className="cm-review-sub">
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
  const { session } = useGaplySession();
  const { theme } = useTheme();
  const { toast } = useToast();
  const navigate = useNavigate();
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
  // Which card's overflow menu is open (null = none). Pure UI state.
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
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
  const [enrichQueue, setEnrichQueue] = useState<EnrichCandidate[]>([]);
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
      setEnrichQueue(plan.enrich); // DOI-exact dups with richer incoming — offered, NOT applied

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

  /* ------- enrichment (Set 2b-iv): fill-only, DOI-exact, user-decided ------ *
   * Default is NO change — enrichment mutates a possibly hand-edited row, so it
   * applies NOTHING until the user clicks Fill. Index-keyed (a file could hold
   * the same DOI twice → same match), and applyEnrichment is fill-only, so even
   * that edge can never overwrite a populated field. */
  const enrichFillAt = async (i: number) => {
    const item = enrichQueue[i];
    if (!item) return;
    await persistUpdate(applyEnrichment(item.match, item.candidate)); // same id → fills gaps in place
    setEnrichQueue((q) => q.filter((_, j) => j !== i));
  };
  const enrichKeepAt = (i: number) => setEnrichQueue((q) => q.filter((_, j) => j !== i)); // leave existing as-is
  const enrichFillAll = async () => {
    for (const item of enrichQueue) await persistUpdate(applyEnrichment(item.match, item.candidate));
    setEnrichQueue([]);
  };
  const enrichKeepAll = () => setEnrichQueue([]); // all left as-is — the default, made explicit

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

  // NAV DECISION (ratified 2026-08-26 — do NOT "restore" the design's items).
  // The Stitch export's sidebar lists Library / Collections / Recent / Shared /
  // Archive. Only Library is backed by anything; the other four are decoration.
  // These four are a deliberate IMPROVEMENT on that: every one is a real filter
  // over real state, so every nav click changes what you see. Restoring the
  // export's list as a fidelity fix would trade four working controls for four
  // dead ones. Each omitted item is a separate feature with a named missing
  // capability, not a styling gap:
  //   Collections — no collection-CRUD store exists
  //   Recent      — no accessed_at / recency ordering on Citation
  //   Shared      — sync is push-only; there is no sharing model
  //   Archive     — no archive flag on Citation
  //   Trash (footer) — removeCitations is a hard delete with no tombstone
  const collectionNav: Array<[CollectionId, string, string]> = [
    ['all', `Library (${citations.length})`, 'book_2'],
    ['manuscript', 'From manuscript', 'description'],
    ['orphans', 'Orphans', 'link_off'],
    ['retracted', `Retracted Items (${retractedCount})`, 'warning'],
  ];

  return (
    <div className={`citation-manager-root ${theme === 'dark' ? 'dark' : 'light'}`} style={{ height: '100vh' }} data-testid="citation-manager">
      {/* Left Sidebar — w-64 per the Stitch export */}
      <aside className="hidden md:flex fixed left-0 top-0 h-screen w-64 z-40 flex-col p-4 space-y-6 border-r border-surface-variant/50 bg-surface-container-low text-on-surface">
        <div className="px-2 pt-2 pb-4">
          <h1 className="font-headline text-xl font-bold text-on-surface">The Archive</h1>
          <p className="font-body text-xs text-on-surface-variant mt-1">Digital Curator</p>
        </div>

        <div className="px-2">
          <button
            type="button"
            onClick={() => void addManual()}
            className="w-full inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 bg-primary-container text-on-accent font-label text-sm font-semibold hover:bg-primary-container/90 transition-colors shadow-sm"
          >
            <span className="material-symbols-outlined text-lg" aria-hidden="true">add</span>
            New Citation
          </button>
        </div>

        <nav className="flex-1 font-label text-sm tracking-wide" data-testid="collections" aria-label="Collections">
          <ul className="space-y-1">
            {collectionNav.map(([id, label, icon]) => (
              <li key={id}>
                <button
                  type="button"
                  onClick={() => setCollection(id)}
                  data-testid={`collection-${id}`}
                  aria-current={collection === id ? 'page' : undefined}
                  className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-all duration-300 ${
                    collection === id
                      ? 'bg-primary-container text-on-accent font-semibold'
                      : 'text-on-surface-variant hover:bg-secondary-container/50 dark:hover:bg-surface-variant'
                  }`}
                >
                  <span className="material-symbols-outlined text-xl" aria-hidden="true">{icon}</span>
                  {label}
                </button>
              </li>
            ))}
          </ul>
        </nav>

        <div className="mt-auto font-label text-sm tracking-wide pt-4 border-t border-surface-variant/30">
          <ul className="space-y-1">
            <li>
              <button
                type="button"
                onClick={() => navigate('/app')}
                className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-on-surface-variant hover:bg-secondary-container/50 dark:hover:bg-surface-variant transition-all duration-300"
              >
                <span className="material-symbols-outlined text-xl" aria-hidden="true">home</span>
                Home
              </button>
            </li>
            <li>
              <button
                type="button"
                onClick={() => navigate('/app/coming-soon')}
                className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-on-surface-variant hover:bg-secondary-container/50 dark:hover:bg-surface-variant transition-all duration-300"
              >
                <span className="material-symbols-outlined text-xl" aria-hidden="true">help_outline</span>
                Help
              </button>
            </li>
            <li>
              <button
                type="button"
                onClick={() => navigate('/app/settings')}
                className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-on-surface-variant hover:bg-secondary-container/50 dark:hover:bg-surface-variant transition-all duration-300"
              >
                <span className="material-symbols-outlined text-xl" aria-hidden="true">settings</span>
                Settings
              </button>
            </li>
          </ul>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 md:ml-64 h-screen overflow-y-auto bg-background flex flex-col md:flex-row">
        {/* Center Panel */}
        <section className="flex-1 flex flex-col max-w-5xl xl:border-r border-surface-variant/50">
          {/* Header */}
          <div className="px-8 pt-10 pb-6 cm-sticky-header backdrop-blur-md sticky top-0 z-30">
            <div className="flex flex-col md:flex-row md:items-end justify-between gap-4 mb-8">
              <div>
                <h2 className="font-headline text-4xl text-on-surface tracking-tight">Citations</h2>
                <p className="font-body text-sm text-on-surface-variant mt-2">{citations.length} items in library</p>
              </div>
              <div className="flex flex-wrap items-center gap-2 font-label text-xs">
                <StylePicker
                  value={style}
                  testid="bulk-style"
                  onSelect={(id) => {
                    setStyle(id);
                    toast(`Reformatted all to ${id}`, 'neutral');
                  }}
                />
                <button
                  type="button"
                  onClick={() => void exportBibliography()}
                  data-testid="export-biblio"
                  className="px-3 py-1.5 rounded bg-surface-container hover:bg-surface-container-high text-on-surface transition-colors flex items-center gap-1.5 border border-outline-variant/30"
                >
                  <span className="material-symbols-outlined text-base" aria-hidden="true">download</span>
                  Export bibliography
                </button>
                <button
                  type="button"
                  onClick={() => void exportAs('bibtex')}
                  data-testid="export-bibtex"
                  className="px-3 py-1.5 rounded bg-surface-container hover:bg-surface-container-high text-on-surface transition-colors border border-outline-variant/30"
                >
                  Export BibTeX
                </button>
                <button
                  type="button"
                  onClick={() => void exportAs('ris')}
                  data-testid="export-ris"
                  className="px-3 py-1.5 rounded bg-surface-container hover:bg-surface-container-high text-on-surface transition-colors border border-outline-variant/30"
                >
                  Export RIS
                </button>
                <div className="hidden md:block w-px h-4 bg-outline-variant/50 mx-1" />
                <button
                  type="button"
                  onClick={checkAllRetractions}
                  disabled={busy}
                  data-testid="check-retractions"
                  className="px-3 py-1.5 rounded bg-secondary-container/30 hover:bg-secondary-container/60 dark:bg-surface-variant dark:hover:bg-surface-container-highest text-on-surface transition-colors flex items-center gap-1.5 font-medium border border-outline-variant/20 disabled:opacity-50"
                >
                  <span className="material-symbols-outlined text-base" aria-hidden="true">health_and_safety</span>
                  Check all for retractions
                </button>
              </div>
            </div>

            {/* Search & Add Panel */}
            <div className="bg-surface-container-lowest rounded-xl p-4 shadow-sm border border-outline-variant/20 flex flex-col gap-4">
              <div className="flex flex-col md:flex-row gap-4">
                <div className="flex-1 relative">
                  <span className="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-on-surface-variant" aria-hidden="true">search</span>
                  <label htmlFor="cm-search-input" className="cm-visually-hidden">Search title, author, or keywords</label>
                  <input
                    id="cm-search-input"
                    className="w-full pl-10 pr-4 py-2.5 rounded-lg border border-outline-variant/30 bg-surface-container-lowest focus:border-primary-fixed focus:ring-1 focus:ring-primary-fixed text-sm font-body text-on-surface placeholder:text-on-surface-variant/70 transition-all outline-none"
                    placeholder="Search title, author, or keywords..."
                    type="text"
                    value={query}
                    data-testid="cm-search"
                    onChange={(e) => setQuery(e.target.value)}
                  />
                </div>
                <div className="flex-1 flex gap-2">
                  <label htmlFor="cm-doi-input" className="cm-visually-hidden">Paste DOI or URL to add</label>
                  <input
                    id="cm-doi-input"
                    className="flex-1 px-4 py-2.5 rounded-lg border border-outline-variant/30 bg-surface-container-lowest focus:border-primary-fixed focus:ring-1 focus:ring-primary-fixed text-sm font-body text-on-surface placeholder:text-on-surface-variant/70 transition-all outline-none"
                    placeholder="Paste DOI or URL to add..."
                    type="text"
                    value={doiInput}
                    data-testid="doi-input"
                    onChange={(e) => setDoiInput(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && addByDoi()}
                  />
                  <button
                    type="button"
                    onClick={addByDoi}
                    disabled={busy}
                    data-testid="add-doi"
                    className="bg-primary-container text-on-accent px-5 py-2.5 rounded-lg font-label text-sm font-semibold hover:bg-primary-container/90 transition-colors whitespace-nowrap shadow-sm disabled:opacity-50"
                  >
                    Add by DOI
                  </button>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-4 text-xs font-label text-on-surface-variant border-t border-surface-variant/50 pt-3">
                <button
                  type="button"
                  onClick={importExtracted}
                  disabled={extractedCitations.length === 0}
                  data-testid="import-extracted"
                  className="hover:text-brand transition-colors flex items-center gap-1.5 disabled:opacity-50"
                >
                  <span className="material-symbols-outlined text-base">upload_file</span>
                  Import from manuscript ({extractedCitations.length})
                </button>
                <button
                  type="button"
                  onClick={addManual}
                  data-testid="add-manual"
                  className="hover:text-brand transition-colors flex items-center gap-1.5"
                >
                  <span className="material-symbols-outlined text-base">edit_document</span>
                  + Manual entry
                </button>
                <label
                  className="hover:text-brand transition-colors flex items-center gap-1.5 cursor-pointer"
                  onClick={(e) => {
                    if (isTauri) {
                      e.preventDefault();
                      void pickAndAddFromFile();
                    }
                  }}
                >
                  <span className="material-symbols-outlined text-base">folder_open</span>
                  From paper file…
                  <input
                    type="file"
                    accept=".pdf,.docx,.txt"
                    className="hidden"
                    data-testid="add-file"
                    onChange={(e) => e.target.files?.[0] && void addFromFile(e.target.files[0])}
                  />
                </label>
                <button
                  type="button"
                  data-testid="import-file"
                  disabled={importing}
                  onClick={() => (isTauri ? void pickAndImport() : importInputRef.current?.click())}
                  className="hover:text-brand transition-colors flex items-center gap-1.5 disabled:opacity-50"
                >
                  <span className="material-symbols-outlined text-base">data_object</span>
                  {importing
                    ? `Importing… ${importProgress?.done ?? 0}/${importProgress?.total ?? 0}`
                    : 'Import .bib / .ris / .json'}
                </button>
                <input
                  ref={importInputRef}
                  type="file"
                  accept=".bib,.bibtex,.ris,.json"
                  className="hidden"
                  data-testid="import-input"
                  onChange={(e) => e.target.files?.[0] && void runImport(e.target.files[0])}
                />
              </div>
            </div>
          </div>

          {/* List Content */}
          <div className="px-8 pb-12 flex flex-col gap-3">
            {importReport && (
              <div className="cm-import-report" data-testid="import-report">
                <p className="cm-import-tally" data-testid="import-tally">
                  {importReport.added.length} added
                  {importReport.enrich.length > 0 && ` · ${importReport.enrich.length} can be enriched`}
                  {importReport.review.length > 0 && ` · ${importReport.review.length} kept as possible duplicates`}
                  {' · '}{importReport.skipped.length} skipped as duplicates
                  {' · '}{importReport.failed.length} failed to parse
                  {importReport.capped && ` · capped at ${IMPORT_ENTRY_CAP} of ${importReport.total}`}
                </p>
                {importReport.skipped.length > 0 && (
                  <details data-testid="import-skipped">
                    <summary>{importReport.skipped.length} skipped as duplicates — inspect</summary>
                    {importReport.skipped.map((s, i) => (
                      <div key={i} className="cm-import-skip" data-testid="import-skipped-item">
                        <span>
                          {s.candidate.csl.title || s.candidate.doi || 'Untitled'} — matches “{s.match.csl.title}”
                        </span>
                        <button
                          type="button"
                          data-testid="import-add-anyway"
                          onClick={() => void addAnyway(s.candidate)}
                          className="px-3 py-1.5 rounded-lg text-sm font-medium bg-secondary-container text-on-secondary-container hover:opacity-90 transition-opacity"
                        >
                          Add anyway
                        </button>
                      </div>
                    ))}
                  </details>
                )}
                {importReport.failed.length > 0 && (
                  <details data-testid="import-failed">
                    <summary>{importReport.failed.length} failed to parse — why</summary>
                    {importReport.failed.map((f, i) => (
                      <div key={i} className="cm-import-fail">
                        <code>{f.raw.slice(0, 80)}</code> — {f.error}
                      </div>
                    ))}
                  </details>
                )}
                <button
                  type="button"
                  data-testid="import-dismiss"
                  onClick={() => setImportReport(null)}
                  className="mt-2 px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                >
                  Dismiss
                </button>
              </div>
            )}

            {reviewQueue.length > 0 && (
              <div className="cm-review-panel" data-testid="review-panel">
                <div className="cm-review-head">
                  <span data-testid="review-count">
                    {reviewQueue.length} to review — kept by default (each looks similar to an entry you already have)
                  </span>
                  <span className="flex items-center gap-2">
                    <button
                      type="button"
                      data-testid="review-keep-all"
                      onClick={reviewKeepAll}
                      className="px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                    >
                      Keep all both
                    </button>
                    <button
                      type="button"
                      data-testid="review-skip-all"
                      onClick={() => void reviewSkipAll()}
                      className="px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                    >
                      Skip all
                    </button>
                    <button
                      type="button"
                      data-testid="review-done"
                      onClick={reviewKeepAll}
                      className="px-3 py-1.5 rounded-lg text-sm font-medium bg-primary-container text-on-accent hover:bg-primary-container/90 transition-colors"
                    >
                      Done
                    </button>
                  </span>
                </div>
                <div className="cm-review-list">
                  {reviewQueue.map((r) => (
                    <div key={r.candidate.id} className="cm-review-item" data-testid="review-item">
                      <div className="cm-review-cols">
                        <div>
                          <span className="cm-review-tag">In your library</span>
                          <CiteLine c={r.match} />
                        </div>
                        <div>
                          <span className="cm-review-tag">Imported</span>
                          <CiteLine c={r.candidate} />
                        </div>
                      </div>
                      <div className="cm-review-actions">
                        <button
                          type="button"
                          data-testid="review-keep"
                          onClick={() => reviewKeep(r.candidate.id)}
                          className="px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                        >
                          Keep both
                        </button>
                        <button
                          type="button"
                          data-testid="review-skip"
                          onClick={() => void reviewSkip(r.candidate.id)}
                          className="px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                        >
                          Skip
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {enrichQueue.length > 0 && (
              <div className="cm-review-panel" data-testid="enrich-panel">
                <div className="cm-review-head">
                  <span data-testid="enrich-count">
                    {enrichQueue.length} can be enriched — your entry is missing fields the import has (nothing changes until you choose)
                  </span>
                  <span className="flex items-center gap-2">
                    <button
                      type="button"
                      data-testid="enrich-fill-all"
                      onClick={() => void enrichFillAll()}
                      className="px-3 py-1.5 rounded-lg text-sm font-medium bg-primary-container text-on-accent hover:bg-primary-container/90 transition-colors"
                    >
                      Fill all
                    </button>
                    <button
                      type="button"
                      data-testid="enrich-keep-all"
                      onClick={enrichKeepAll}
                      className="px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                    >
                      Keep all as-is
                    </button>
                  </span>
                </div>
                <div className="cm-review-list">
                  {enrichQueue.map((e, i) => (
                    <div key={i} className="cm-review-item" data-testid="enrich-item">
                      <div className="cm-review-cols">
                        <div>
                          <span className="cm-review-tag">In your library</span>
                          <CiteLine c={e.match} />
                        </div>
                        <div>
                          <span className="cm-review-tag">Imported (same DOI)</span>
                          <CiteLine c={e.candidate} />
                        </div>
                      </div>
                      <p className="text-xs text-on-surface-variant my-1.5" data-testid="enrich-missing">
                        Your entry is missing: {e.missingFields.map((f) => ENRICH_FIELD_LABEL[f]).join(', ')} — fill from the import?
                      </p>
                      <div className="cm-review-actions">
                        <button
                          type="button"
                          data-testid="enrich-fill"
                          onClick={() => void enrichFillAt(i)}
                          className="px-3 py-1.5 rounded-lg text-sm font-medium bg-primary-container text-on-accent hover:bg-primary-container/90 transition-colors"
                        >
                          Fill missing fields
                        </button>
                        <button
                          type="button"
                          data-testid="enrich-keep"
                          onClick={() => enrichKeepAt(i)}
                          className="px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                        >
                          Keep as-is
                        </button>
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
                <div className="cm-verify-bar" data-testid="verify-bar">
                  {verifying ? (
                    <>
                      <span data-testid="verify-progress">
                        Verifying {verifyProgress?.done ?? 0} of {verifyProgress?.total ?? 0}…
                      </span>
                      <button
                        type="button"
                        data-testid="verify-cancel"
                        onClick={() => { verifyCancelRef.current = true; }}
                        className="px-3 py-1.5 rounded-lg text-sm font-medium bg-surface-container text-on-surface-variant hover:bg-secondary-container/50 transition-colors"
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        type="button"
                        data-testid="verify-imported"
                        disabled={off}
                        onClick={() => void verifyImported()}
                        className="px-3 py-1.5 rounded-lg text-sm font-medium bg-primary-container text-on-accent hover:bg-primary-container/90 transition-colors disabled:opacity-50"
                      >
                        Verify {n} online
                      </button>
                      {off && (
                        <span className="cm-verify-note" data-testid="verify-off-note">
                          Citation verification is turned off in Settings → Sync &amp; Privacy
                        </span>
                      )}
                    </>
                  )}
                </div>
              );
            })()}

            <div className="space-y-3" data-testid="citation-list">
              {visible.length === 0 ? (
                <p className="text-sm text-on-surface-variant" data-testid="empty">
                  {collection === 'retracted' ? 'No retracted items — good.' : 'No citations yet. Add one above.'}
                </p>
              ) : (
                visible.map((c) => {
                  const untitled = !c.csl.title || c.csl.title === 'Untitled — edit details';
                  return (
                    <div key={c.id} className="cm-cite-wrap relative">
                      <button
                        type="button"
                        className="bg-surface-container-lowest p-5 rounded-xl border border-outline-variant/20 hover:border-outline-variant/40 transition-all cursor-pointer relative overflow-hidden text-left cm-cite-card"
                        aria-current={c.id === selectedId}
                        data-retracted={c.retracted}
                        data-testid={`cite-${c.id}`}
                        onClick={() => setSelectedId(c.id)}
                      >
                        {c.retracted && (
                          <span className="mb-1.5 inline-flex items-center gap-1 px-2 py-0.5 rounded font-label text-[10px] font-bold uppercase tracking-wider bg-error-container text-on-error-container">
                            <span className="material-symbols-outlined text-sm" aria-hidden="true">warning</span>
                            retracted
                          </span>
                        )}
                        <h3 className={`font-headline text-lg font-medium leading-snug ${c.retracted ? 'line-through opacity-70' : ''} ${untitled ? 'italic text-on-surface-variant/70' : 'text-on-surface'}`}>
                          {untitled ? (
                            <span className="inline-flex items-center gap-2">
                              <span className="material-symbols-outlined text-outline" aria-hidden="true">error_outline</span>
                              {/* The manual-entry placeholder keeps its real
                                  prompt-to-edit text; a truly empty title falls
                                  back to the mock's "Untitled Document". */}
                              {c.csl.title || 'Untitled Document'}
                            </span>
                          ) : (
                            c.csl.title
                          )}
                        </h3>
                        <p className="font-body text-sm text-on-surface-variant mt-2 flex flex-wrap items-center gap-2">
                          <span>{c.csl.author[0]?.family ?? 'Unknown'} • {c.csl.issued?.year ?? 'n.d.'}</span>
                          <span className="h-1 w-1 rounded-full bg-outline-variant/50" aria-hidden="true" />
                          {(() => {
                            const vs = verificationState(c);
                            const tid: Record<VerificationState, string> = {
                              verified: 'verified-badge',
                              unverified: 'unverified-badge',
                              not_found: 'badge-not-found',
                              check_failed: 'badge-check-failed',
                            };
                            // font-label: chips take the label face per the
                            // design's type scale (the metadata <p> around them
                            // is the body serif, which they were inheriting).
                            const base = 'inline-flex items-center gap-1 px-1.5 py-0.5 rounded font-label text-xs';
                            if (vs === 'verified') {
                              return (
                                <span className={`${base} bg-surface-container-high text-on-surface-variant`} data-testid={tid[vs]}>
                                  <span className="material-symbols-outlined text-xs text-green-600 dark:text-green-500" aria-hidden="true">verified</span>
                                  {VERIFY_LABEL[vs]}
                                </span>
                              );
                            }
                            if (vs === 'unverified') {
                              return (
                                <span className={`${base} bg-surface-container text-on-surface-variant border border-outline-variant/30`} data-testid={tid[vs]}>
                                  <span className="material-symbols-outlined text-xs" aria-hidden="true">help</span>
                                  {VERIFY_LABEL[vs]}
                                </span>
                              );
                            }
                            if (vs === 'not_found') {
                              return (
                                <span className={`${base} bg-error-container text-on-error-container`} data-testid={tid[vs]}>
                                  <span className="material-symbols-outlined text-xs" aria-hidden="true">error</span>
                                  {VERIFY_LABEL[vs]}
                                </span>
                              );
                            }
                            return (
                              <span className={`${base} bg-surface-container-high text-on-surface-variant`} data-testid={tid[vs]}>
                                <span className="material-symbols-outlined text-xs" aria-hidden="true">sync_problem</span>
                                {VERIFY_LABEL[vs]}
                              </span>
                            );
                          })()}
                        </p>
                      </button>
                      <button
                        type="button"
                        className="cm-card-more"
                        aria-label="Citation actions"
                        aria-expanded={openMenuId === c.id}
                        onClick={() => setOpenMenuId((m) => (m === c.id ? null : c.id))}
                      >
                        <span className="material-symbols-outlined text-xl" aria-hidden="true">more_vert</span>
                      </button>
                      {openMenuId === c.id && (
                        <div className="cm-card-menu" role="menu">
                          <button
                            type="button"
                            role="menuitem"
                            onClick={() => {
                              setOpenMenuId(null);
                              void removeCitations([c.id]);
                            }}
                          >
                            <span className="material-symbols-outlined text-base" aria-hidden="true">delete</span>
                            Remove from library
                          </button>
                        </div>
                      )}
                    </div>
                  );
                })
              )}
            </div>

            {/* Decorative archival spacer */}
            <div className="w-full flex justify-center py-6 opacity-30">
              <span className="material-symbols-outlined text-outline" aria-hidden="true">auto_stories</span>
            </div>
          </div>
        </section>

        {/* Right Detail Pane */}
        <aside className="hidden xl:flex w-80 bg-surface-container flex-col sticky top-0 h-screen p-6 border-l border-surface-variant/50">
          {!selected ? (
            <div className="h-full flex flex-col items-center justify-center text-center border-2 border-dashed border-outline-variant/30 rounded-xl bg-surface-container-lowest/50">
              <span className="material-symbols-outlined text-4xl text-outline-variant mb-4" style={{ fontVariationSettings: "'wght' 200" }} aria-hidden="true">article</span>
              <h4 className="font-headline text-lg text-on-surface-variant mb-2">Citation detail</h4>
              <p className="font-body text-sm text-outline px-6">Select an item from the library to view metadata, abstract, and attached files.</p>
            </div>
          ) : (
            <div className="h-full flex flex-col" data-testid="detail">
              {selected.retracted && (
                <div className="mb-4 p-3 rounded-lg bg-error-container text-on-error-container text-sm flex items-center gap-2" data-testid="retract-banner">
                  <span className="material-symbols-outlined text-sm">warning</span>
                  This work has been RETRACTED{selected.noticeUrl ? ' — see notice.' : '.'}
                </div>
              )}
              <div className="mb-4">
                <label className="block font-label text-xs text-on-surface-variant mb-1">Preview style — any journal worldwide</label>
                <StylePicker value={style} testid="preview-style" onSelect={setStyle} />
              </div>
              <div
                className="bg-surface-container-lowest border border-outline-variant/20 rounded-xl p-4 mb-4 leading-relaxed text-sm text-on-surface"
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
              <div className="mb-4" data-testid="cm-tags">
                <div className="flex flex-wrap items-center gap-2">
                  {(selected.tags ?? []).map((t) => (
                    <button
                      key={t}
                      type="button"
                      title="Remove tag"
                      data-testid={`cm-tag-${t}`}
                      onClick={() => void applyTags(selected, (selected.tags ?? []).filter((x) => x !== t))}
                      className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-medium bg-secondary-container text-on-secondary-container hover:opacity-90 transition-opacity"
                    >
                      {t}
                      <span className="material-symbols-outlined text-sm">close</span>
                    </button>
                  ))}
                  <label htmlFor="cm-tag-input" className="cm-visually-hidden">Add a tag</label>
                  <input
                    id="cm-tag-input"
                    className="px-3 py-1.5 rounded-full text-xs bg-surface-container-lowest text-on-surface placeholder:text-on-surface-variant border border-outline-variant/30 focus:outline-none focus:border-brand"
                    style={{ maxWidth: 140 }}
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
              </div>
              <div className="text-xs text-on-surface-variant mb-3" data-testid="cm-sync-status">
                {selected.syncStatus === 'synced'
                  ? 'Synced to your account'
                  : selected.syncStatus === 'pending'
                    ? 'Sync pending — kept locally'
                    : session
                      ? 'Local only (not yet synced)'
                      : 'Local only — sign in to enable optional sync'}
              </div>
              <div className="text-xs text-on-surface-variant space-y-1">
                <div data-testid="detail-metadata">Metadata: {STATUS_LABEL[computeStatus(selected)]}</div>
                <div data-testid="detail-verification">Verification: {VERIFY_LABEL[verificationState(selected)]}</div>
                {selected.provenance && selected.provenance.length > 0 && (
                  <div className="font-mono mt-1.5" data-testid="detail-provenance">
                    {selected.provenance.slice(0, 3).join('  ·  ')}
                  </div>
                )}
              </div>
            </div>
          )}
        </aside>
      </main>
    </div>
  );
};

const CitationManagerPage: React.FC<CitationManagerPageProps> = (props) => (
  <ToastProvider>
    <Inner {...props} />
  </ToastProvider>
);

export default CitationManagerPage;
