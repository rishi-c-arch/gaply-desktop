// Gaply — citation exports (Set 5). BibTeX / RIS via the citation-js plugins
// (already installed), formatted bibliographies via the Set 3 CSL engine —
// all DETERMINISTIC serialization of the VERIFIED CSL-JSON in the local
// library. No LLM anywhere: a field missing from the library is missing
// from the export, never invented (Sets 2/3/4's honesty carried through).
import type { Cite } from '@citation-js/core';
import { saveTextFile } from '../../utils/saveTextFile';
import { CslItem } from './citationTypes';
import { formatWithCsl, toCslJson } from './cslEngine';

export type ExportFormat = 'bibtex' | 'ris';

let pluginsPromise: Promise<typeof Cite> | null = null;

/** Lazy-load core + the bibtex/ris plugins (citeEngine.ts pattern). */
function citeClass(): Promise<typeof Cite> {
  if (!pluginsPromise) {
    pluginsPromise = (async () => {
      await import('@citation-js/plugin-bibtex');
      await import('@citation-js/plugin-ris');
      const core = await import('@citation-js/core');
      return core.Cite;
    })();
  }
  return pluginsPromise;
}

/* ---- BibTeX entry-key de-collision (D7) -------------------------------- *
 * THIS IS THE REAL FIX FOR THE INVALID .bib, and it supersedes an earlier
 * diagnosis worth recording so it is not re-opened:
 *
 *   The collision was attributed to CitationManagerPage's addManual hardcoding
 *   `csl.id = 'manual'` for every manual entry. MEASURED, that is NOT the
 *   cause. citation-js derives the BibTeX key from author + year + title and
 *   ignores `csl.id` entirely; it ignores an explicit `citation-key` on the
 *   CSL-JSON too. Two entries sharing author+year+title emit the SAME
 *   `@article{key,}` whatever their ids are — two untitled manual stubs both
 *   emitted `@article{Untitled,`, and BibTeX either errors or silently drops
 *   one. Giving each entry a unique csl.id is still correct on its own merits
 *   (a shared CSL id also confuses citeproc's bibliography, and RIS takes its
 *   ID field straight from csl.id) — but it does not make the .bib valid.
 *
 * Since the key cannot be steered on the way in, it is de-collided on the way
 * out. First occurrence keeps its generated key; later ones get the lowest free
 * `-N` suffix. Emission order is stable, so re-exporting the same library
 * yields the same file — the determinism guarantee above still holds. */
export function decollideBibtexKeys(bibtex: string): string {
  const used = new Set<string>();
  return bibtex.replace(/^(@\w+\s*\{)([^,\s{}]+)(,)/gm, (_match, open: string, key: string, comma: string) => {
    let candidate = key;
    let n = 1;
    while (used.has(candidate)) {
      n += 1;
      candidate = `${key}-${n}`;
    }
    used.add(candidate);
    return `${open}${candidate}${comma}`;
  });
}

/** Serialize references to BibTeX or RIS. Pure citation-js serialization of
 *  the verified CSL-JSON — deterministic, absent fields stay absent. BibTeX
 *  keys are de-collided afterwards (see above); nothing else is rewritten. */
export async function exportSerialized(items: CslItem[], format: ExportFormat): Promise<string> {
  const CiteCtor = await citeClass();
  const cite = new CiteCtor(items.map(toCslJson));
  const text = (cite.format(format) as string).trim();
  return format === 'bibtex' ? decollideBibtexKeys(text) : text;
}

/** A formatted bibliography in a prepared CSL style (Set 3 engine — the
 *  style must be prepared/registered; unknown styles throw honestly). */
export function exportBibliographyText(items: CslItem[], styleId: string): string {
  return formatWithCsl(items, styleId);
}

/** Save exported text to a file: the Tauri save dialog in the desktop app,
 *  a plain browser download otherwise. Returns the chosen path (Tauri) or
 *  null (browser download / user cancelled). Delegates to the shared
 *  saveTextFile with the citations MIME type (text/plain) — unchanged behaviour. */
export const saveExportToFile = (suggestedName: string, text: string): Promise<string | null> =>
  saveTextFile(suggestedName, text, 'text/plain');
