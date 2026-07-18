// Gaply — Citation Manager import pipeline (Set 2b-i). PURE + LOCAL:
// text in → a classified ImportPlan out. It parses .bib / .ris / CSL-JSON,
// normalizes to our Citation shape, and dedupes via the Set-2a matcher with a
// batch-internal accumulator. It NEVER touches the network — the zero-fetch
// guarantee is structural: this module has no network dependency, so consent
// state is irrelevant here. Online verification is a separate, gated, opt-in
// step (Set 2b-iii); the fuzzy-review UI is Set 2b-ii.
//
// SCALE: parsing is chunked with yields + onProgress so a large file doesn't
// freeze the UI, and bounded by IMPORT_ENTRY_CAP. A fully off-main-thread parse
// (Web Worker) is the better answer and is DEFERRED — recorded here, not built:
// chunked-with-progress is responsive enough for realistic libraries, and a
// Worker is meaningful extra setup (citation-js in a worker) we don't need yet.
import type { Cite as CiteType } from '@citation-js/core';
import { Citation, CitationSource, CslItem } from './citationTypes';
import { DedupeIndex, buildDedupeIndex, lookupDuplicate, addToDedupeIndex, fieldDelta, EnrichField } from './dedupe';

export type ImportFormat = 'bibtex' | 'ris' | 'csl-json';

/** Server-mirrored cap — see read_import_file. Beyond this we import the first
 *  N and say so honestly (never silently truncate). */
export const IMPORT_ENTRY_CAP = 5000;
const CHUNK = 50; // entries parsed between event-loop yields

export interface SkippedEntry {
  candidate: Citation;
  match: Citation;
}
/** Set 2b-iv: a DOI-exact dup where the incoming entry has fields the existing
 *  one LACKS — an OFFER to fill (never applied automatically). */
export interface EnrichCandidate {
  candidate: Citation; // the incoming, richer entry (its fields feed the fill)
  match: Citation; // the existing entry that could be enriched (same DOI)
  missingFields: EnrichField[]; // fields existing lacks that incoming supplies
}
export interface FailedEntry {
  raw: string;
  error: string;
}
export interface ImportPlan {
  added: Citation[]; // no match — safe to add
  enrich: EnrichCandidate[]; // Tier-1 DOI dup, incoming is richer → OFFER to fill (2b-iv)
  skipped: SkippedEntry[]; // Tier-1 DOI-exact dup, no new fields (auto-skip, inspectable)
  review: SkippedEntry[]; // Tier-2 title+year fuzzy — the user decides (2b-ii)
  failed: FailedEntry[]; // per-entry parse failures
  total: number; // raw entries seen (before the cap)
  capped: boolean; // hit IMPORT_ENTRY_CAP
}

/** Map a filename's extension to a format, or null if unsupported. */
export function detectFormat(filename: string): ImportFormat | null {
  const ext = filename.toLowerCase().split('.').pop() ?? '';
  if (ext === 'bib' || ext === 'bibtex') return 'bibtex';
  if (ext === 'ris') return 'ris';
  if (ext === 'json') return 'csl-json';
  return null;
}

/** Split a .bib file into raw per-entry strings, skipping @comment/@string/
 *  @preamble meta entries. Splitting BEFORE parsing is what makes parsing
 *  per-entry defensive — one bad @article can't kill the batch. */
export function splitBibtex(text: string): string[] {
  return text
    .split(/(?=@\w+\s*\{)/)
    .map((s) => s.trim())
    .filter((s) => /^@\w+\s*\{/.test(s))
    .filter((s) => !/^@(comment|string|preamble)\b/i.test(s));
}

/** Split a .ris file into raw records (each TY…ER block, terminator included). */
export function splitRis(text: string): string[] {
  const matches = text.match(/TY\s+-[\s\S]*?ER\s+-.*?(?:\r?\n|$)/g);
  return matches ? matches.map((s) => s.trim()).filter(Boolean) : [];
}

/** citation-js is lazy-loaded (core + bibtex/ris plugins) — mirrors exporters.ts. */
let citePromise: Promise<typeof CiteType> | null = null;
function citeClass(): Promise<typeof CiteType> {
  if (!citePromise) {
    citePromise = (async () => {
      await import('@citation-js/plugin-bibtex');
      await import('@citation-js/plugin-ris');
      const core = await import('@citation-js/core');
      return core.Cite;
    })();
  }
  return citePromise;
}

/** Map one citation-js / CSL-JSON item to our Citation. Deterministic id (from
 *  DOI, else title+year, else index) so re-importing the same file is stable.
 *  source: 'imported' → the UI shows the honest "not verified online" badge. */
export function normalizeToCitation(raw: any, index: number): Citation {
  const doi: string | null = typeof raw?.DOI === 'string' ? raw.DOI : null;
  const title: string = typeof raw?.title === 'string' ? raw.title : '';
  const year: number | undefined =
    typeof raw?.issued?.['date-parts']?.[0]?.[0] === 'number'
      ? raw.issued['date-parts'][0][0]
      : typeof raw?.issued?.year === 'number'
        ? raw.issued.year
        : undefined;
  const author = Array.isArray(raw?.author)
    ? raw.author.map((a: any) => ({ family: String(a?.family ?? ''), given: a?.given ? String(a.given) : undefined }))
    : [];
  const csl: CslItem = {
    id: doi ?? `imported-${index}`,
    type: typeof raw?.type === 'string' ? raw.type : 'article-journal',
    title,
    author,
    ...(year != null ? { issued: { year } } : {}),
    ...(doi ? { DOI: doi } : {}),
    ...(typeof raw?.['container-title'] === 'string' ? { containerTitle: raw['container-title'] } : {}),
    ...(typeof raw?.containerTitle === 'string' ? { containerTitle: raw.containerTitle } : {}),
    ...(raw?.volume != null ? { volume: String(raw.volume) } : {}),
    ...(raw?.issue != null ? { issue: String(raw.issue) } : {}),
    ...(raw?.page != null ? { page: String(raw.page) } : {}),
  };
  const stableId = doi ? `imp:${doi}` : title && year != null ? `imp:${title}:${year}` : `imp:${index}`;
  return {
    id: stableId,
    csl,
    doi,
    retracted: false,
    source: 'imported' as CitationSource,
  };
}

const yieldToLoop = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/**
 * Classify an import against the existing library. PURE classification only —
 * it applies nothing and fetches nothing. The caller persists `added` (and
 * `review` under the keep-both default, until 2b-ii's panel), shows `skipped`
 * inspectably, and reports `failed`.
 */
export async function planImport(
  text: string,
  format: ImportFormat,
  existing: Citation[],
  opts: { cap?: number; onProgress?: (done: number, total: number) => void } = {},
): Promise<ImportPlan> {
  const cap = opts.cap ?? IMPORT_ENTRY_CAP;
  const index: DedupeIndex = buildDedupeIndex(existing);
  const added: Citation[] = [];
  const enrich: EnrichCandidate[] = [];
  const skipped: SkippedEntry[] = [];
  const review: SkippedEntry[] = [];
  const failed: FailedEntry[] = [];

  // Build the raw units + a per-unit parser. CSL-JSON is a single JSON array
  // (can't be split before parsing — an outer-JSON error fails the whole file,
  // one honest FailedEntry; .bib/.ris split cleanly line-by-line).
  let rawUnits: string[] = [];
  let jsonItems: any[] | null = null;
  if (format === 'csl-json') {
    try {
      const parsed = JSON.parse(text);
      jsonItems = Array.isArray(parsed) ? parsed : [parsed];
    } catch (e) {
      return {
        added,
        enrich,
        skipped,
        review,
        failed: [{ raw: text.slice(0, 200), error: `Not valid JSON: ${(e as Error).message}` }],
        total: 0,
        capped: false,
      };
    }
  } else {
    rawUnits = format === 'bibtex' ? splitBibtex(text) : splitRis(text);
  }

  const total = jsonItems ? jsonItems.length : rawUnits.length;
  const limit = Math.min(total, cap);
  const capped = total > cap;

  const Cite = format === 'csl-json' ? null : await citeClass();

  for (let i = 0; i < limit; i += 1) {
    let item: any = null;
    let parseError: string | null = null;
    try {
      if (jsonItems) {
        item = jsonItems[i];
        if (!item || typeof item !== 'object') parseError = 'not a CSL-JSON object';
      } else {
        // Per-entry parse — its OWN try/catch, so one malformed entry is
        // isolated to a FailedEntry and never aborts the batch.
        const c = new Cite!(rawUnits[i]);
        item = (c as any).data?.[0] ?? null;
        if (!item) parseError = 'no citation recognized in this entry';
      }
    } catch (e) {
      parseError = (e as Error).message || 'parse failed';
    }

    if (parseError || !item) {
      failed.push({ raw: (jsonItems ? JSON.stringify(jsonItems[i]) : rawUnits[i] ?? '').slice(0, 200), error: parseError ?? 'parse failed' });
    } else {
      const cand = normalizeToCitation(item, i);
      const dup = lookupDuplicate(index, cand);
      if (!dup) {
        added.push(cand);
        addToDedupeIndex(index, cand); // batch-internal: the next same entry now collides
      } else if (dup.tier === 'doi') {
        // Same paper (DOI-certain). If the incoming carries fields the existing
        // lacks, OFFER to fill them (2b-iv); otherwise it's a true no-op dup and
        // is auto-skipped as before. Never a silent drop of useful data.
        const missingFields = fieldDelta(dup.match, cand);
        if (missingFields.length > 0) {
          enrich.push({ candidate: cand, match: dup.match, missingFields });
        } else {
          skipped.push({ candidate: cand, match: dup.match });
        }
      } else {
        review.push({ candidate: cand, match: dup.match });
        addToDedupeIndex(index, cand); // keep-both default: it exists for later matches too
      }
    }

    if ((i + 1) % CHUNK === 0) {
      opts.onProgress?.(i + 1, limit);
      await yieldToLoop();
    }
  }
  opts.onProgress?.(limit, limit);
  return { added, enrich, skipped, review, failed, total, capped };
}
