// Gaply — citation exports (Set 5). BibTeX / RIS via the citation-js plugins
// (already installed), formatted bibliographies via the Set 3 CSL engine —
// all DETERMINISTIC serialization of the VERIFIED CSL-JSON in the local
// library. No LLM anywhere: a field missing from the library is missing
// from the export, never invented (Sets 2/3/4's honesty carried through).
import type { Cite } from '@citation-js/core';
import { isTauri } from '../../utils/isTauri';
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
 *  null (browser download / user cancelled). */
export async function saveExportToFile(
  suggestedName: string,
  text: string
): Promise<string | null> {
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: suggestedName });
    if (!path) return null; // user cancelled — honest no-op
    const { writeTextFile } = await import('@tauri-apps/plugin-fs');
    await writeTextFile(path, text);
    return path;
  }
  // Browser fallback (marketing/web build): the existing blob pattern.
  const blob = new Blob([text], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = suggestedName;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
  return null;
}
