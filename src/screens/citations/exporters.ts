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

/** Serialize references to BibTeX or RIS. Pure citation-js serialization of
 *  the verified CSL-JSON — deterministic, absent fields stay absent. */
export async function exportSerialized(items: CslItem[], format: ExportFormat): Promise<string> {
  const CiteCtor = await citeClass();
  const cite = new CiteCtor(items.map(toCslJson));
  return (cite.format(format) as string).trim();
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
