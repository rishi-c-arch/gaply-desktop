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
  const units = splitBibtexUnits(text);
  const macros = collectStringMacros(units);
  const entries = units
    .filter((u) => !/^@(comment|string|preamble)\b/i.test(u.text))
    .map((u) => ({ ...u, text: expandMacros(u.text, macros) }));
  return entries.map((u) => inheritCrossref(u, entries));
}

/** `field = {value}` pairs at the top level of one entry. */
function fieldsOf(entry: string): Map<string, string> {
  const out = new Map<string, string>();
  const body = entry.replace(/^@[A-Za-z]\w*\s*[{(]/, '');
  const re = /(^|,)\s*([A-Za-z]\w*)\s*=\s*/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(body))) {
    // Read the value by depth from just after the `=`, so a braced value
    // containing commas is one value rather than several fields.
    let i = m.index + m[0].length;
    let depth = 0;
    const start = i;
    for (; i < body.length; i += 1) {
      const ch = body[i];
      if (ch === '\\') {
        i += 1;
        continue;
      }
      if (ch === '{') depth += 1;
      else if (ch === '}') {
        if (depth === 0) break; // the entry's own closing brace
        depth -= 1;
      } else if (ch === ',' && depth === 0) break;
    }
    out.set(m[2].toLowerCase(), body.slice(start, i).trim());
    re.lastIndex = i;
  }
  return out;
}

/**
 * Apply BibTeX `crossref` inheritance (§11 D113).
 *
 * A child inherits every field it does not define itself. Without this a
 * chapter written `crossref = {parent}` imports with no year, and
 * `computeStatus` then labels a perfectly well-formed reference "malformed
 * metadata" — the tool calling the user's library wrong.
 *
 * The one special case that matters in practice is real BibTeX semantics: a
 * child in a collection takes the parent's `title` as its `booktitle`, because
 * the parent's title IS the book. Deliberately NOT a full implementation of
 * crossref (inheritance chains, `@string` scoping inside parents); one level,
 * which is what emitters produce.
 */
function inheritCrossref(unit: BibUnit, all: BibUnit[]): string {
  const own = fieldsOf(unit.text);
  const parentKey = own.get('crossref')?.replace(/^[{"]|[}"]$/g, '').trim();
  if (!parentKey) return unit.text;
  const parent = all.find((u) => u.key === parentKey);
  if (!parent) return unit.text; // dangling crossref: leave it, do not invent
  const theirs = fieldsOf(parent.text);

  const add: string[] = [];
  // `Array.from` rather than iterating the Map directly: this project's
  // tsconfig target predates downlevelIteration.
  for (const [k, v] of Array.from(theirs.entries())) {
    if (k === 'crossref' || own.has(k)) continue;
    add.push(`  ${k} = ${v}`);
  }
  if (!own.has('booktitle') && theirs.has('title') && !theirs.has('booktitle')) {
    add.push(`  booktitle = ${theirs.get('title')}`);
  }
  if (add.length === 0) return unit.text;

  // Splice before the entry's closing delimiter.
  const close = unit.text.lastIndexOf(unit.text.trimEnd().endsWith(')') ? ')' : '}');
  const head = unit.text.slice(0, close).replace(/,\s*$/, '');
  return `${head},\n${add.join(',\n')}\n${unit.text.slice(close)}`;
}

/**
 * `@string{jml = {Journal of Machine Learning Research}}` → the map (§11 D113).
 *
 * These definitions were being FILTERED OUT before parsing, so a field written
 * `journal = jml` could never resolve: citation-js saw a bare token, dropped
 * it, and the entry imported looking complete with no journal and no error.
 * Silent loss of the one field every bibliography style prints.
 *
 * Default Zotero and Mendeley do not emit `@string`; JabRef and hand-maintained
 * files do (see `fixtures/README.md`).
 */
function collectStringMacros(units: BibUnit[]): Map<string, string> {
  const out = new Map<string, string>();
  for (const u of units) {
    if (u.type !== 'string') continue;
    // `@string{name = {value}}` or `@string{name = "value"}`
    const m = /^@string\s*[{(]\s*([A-Za-z]\w*)\s*=\s*([\s\S]*[^\s])\s*[})]\s*$/i.exec(u.text);
    if (!m) continue;
    const value = m[2].trim().replace(/^[{"]/, '').replace(/[}"]$/, '').trim();
    if (value) out.set(m[1], value);
  }
  return out;
}

/**
 * Replace bare macro references in field values with their definition.
 *
 * ONLY a bare identifier is substituted — `journal = jml,`. A braced or quoted
 * value is already a literal and is left alone, and an UNDEFINED identifier is
 * also left alone: `month = oct` is a built-in BibTeX macro that no file
 * defines, and rewriting it to nothing would lose data to fix a different
 * problem. Unknown stays unknown.
 */
function expandMacros(entry: string, macros: Map<string, string>): string {
  if (macros.size === 0) return entry;
  return entry.replace(
    /(^|[,{(]\s*)([A-Za-z]\w*)(\s*=\s*)([A-Za-z]\w*)(\s*)(?=[,})])/g,
    (whole, lead, field, eq, value, tail) =>
      macros.has(value) ? `${lead}${field}${eq}{${macros.get(value)}}${tail}` : whole,
  );
}

/** One top-level `@…{…}` block, with its type and citation key. */
interface BibUnit {
  type: string;
  key: string;
  text: string;
}

/**
 * Scan out every top-level `@type{…}` block by BRACE DEPTH (§11 D113).
 *
 * The previous splitter was `text.split(/(?=@\w+\s*\{)/)`, which matches
 * anywhere — including inside a field value. Two real shapes broke it, and the
 * first is the worst defect this importer has had:
 *
 *   • an abstract quoting BibTeX (`…write @article{foo, title={bar}}…`) was cut
 *     in two. The real half failed to parse and was reported; the trailing half
 *     parsed CLEAN and was added to the library as a paper titled "bar". A
 *     citation the researcher never had, arriving silently.
 *   • `note = {Corresponding author: nora@lab {group site}}` — `@lab {` matched
 *     because the pattern allowed whitespace before the brace. Both halves
 *     failed and the entry was simply lost.
 *
 * Depth counting is the fix: an `@` only starts an entry at depth 0. A
 * backslash escapes the next character, so `\{` in a LaTeX field does not
 * shift the depth. An entry left unterminated at EOF is returned as-is rather
 * than dropped — a truncated file should FAIL LOUDLY at the parser, not
 * disappear here.
 */
function splitBibtexUnits(text: string): BibUnit[] {
  const out: BibUnit[] = [];
  const n = text.length;
  let i = 0;
  while (i < n) {
    if (text[i] !== '@') {
      i += 1;
      continue;
    }
    const head = /^@([A-Za-z]\w*)\s*([{(])/.exec(text.slice(i));
    if (!head) {
      i += 1;
      continue;
    }
    const open = head[2];
    const close = open === '{' ? '}' : ')';
    let j = i + head[0].length - 1; // sits on the opening delimiter
    let depth = 0;
    for (; j < n; j += 1) {
      const ch = text[j];
      if (ch === '\\') {
        j += 1; // skip the escaped character
        continue;
      }
      if (ch === open) depth += 1;
      else if (ch === close) {
        depth -= 1;
        if (depth === 0) {
          j += 1;
          break;
        }
      }
    }
    // RECOVERY (§11 D113). If the scan ran to EOF without closing, the entry is
    // unterminated — and swallowing the rest of the file with it would undo the
    // per-entry resilience this splitter exists for: `good / broken / good`
    // must still yield two good entries and one reported failure, not one.
    //
    // The cut point is the next `@type{` AT THE START OF A LINE. That is the
    // discriminator between a real entry and the hazards above: every emitter
    // writes entries at column 0, while an `@` inside a field value is
    // mid-line. So recovery cannot reintroduce the split it just fixed.
    let end = j;
    if (depth !== 0) {
      const rest = text.slice(i + 1);
      const nextLineStart = /(?:^|\r?\n)[ \t]*@[A-Za-z]\w*\s*[{(]/.exec(rest);
      if (nextLineStart) {
        const at = nextLineStart.index + nextLineStart[0].search(/@/);
        end = i + 1 + at;
      }
    }
    const raw = text.slice(i, end).trim();
    const key = /^@[A-Za-z]\w*\s*[{(]\s*([^,\s}]*)/.exec(raw)?.[1] ?? '';
    out.push({ type: head[1].toLowerCase(), key, text: raw });
    i = end;
  }
  return out;
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
    ? raw.author
        .map((a: any) => ({
          family: String(a?.family ?? ''),
          given: a?.given ? String(a.given) : undefined,
        }))
        // §11 D113. BibTeX's `author = {Nested, Nora and others}` means "et al".
        // citation-js renders the marker as a person whose family name is
        // literally "others", and every bibliography style then prints it as a
        // real co-author. Dropped rather than kept: CSL-JSON has no et-al
        // marker, and an invented collaborator is worse than a short list.
        // (The consequence, stated rather than hidden: the entry no longer
        // records that further authors exist. Recovering that needs a field
        // Citation does not have.)
        .filter((a: { family: string; given?: string }) =>
          !(a.family.toLowerCase() === 'others' && !a.given))
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
