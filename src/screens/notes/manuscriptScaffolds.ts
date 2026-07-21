// Gaply — Research Paper Writer, Set C: load the versioned scaffold asset.
// Mirrors how the CSL manifest is served — a static JSON asset fetched from the
// app's own origin (offline, no external network). Falls back to the bundled
// generic IMRaD scaffold if the fetch fails, so the editor never blocks.
import { Scaffold, IMRAD_SCAFFOLD } from './manuscriptModel';

export interface ScaffoldCatalog {
  version: number;
  globalNotice: string;
  scaffolds: Scaffold[];
}

// The honest picker notice (rule 3) — also shipped in the asset's globalNotice;
// this constant is the offline fallback and the single source the UI reads.
export const SCAFFOLD_GLOBAL_NOTICE =
  "These are Gaply's own structures to help you start — not official publisher templates. " +
  "Always check the venue's current author guidelines (linked). " +
  'Gaply formats structure + references; the publisher typesets the final layout. Free to use.';

let cache: ScaffoldCatalog | null = null;

/** Load the scaffold catalog once (cached). Never throws — on any failure it
 *  returns the bundled generic so the writer still works offline. */
export async function loadScaffoldCatalog(): Promise<ScaffoldCatalog> {
  if (cache) return cache;
  try {
    const resp = await fetch('/manuscripts/scaffolds.json');
    if (resp.ok) {
      const j = (await resp.json()) as Partial<ScaffoldCatalog>;
      if (Array.isArray(j.scaffolds) && j.scaffolds.length > 0) {
        cache = {
          version: j.version ?? 0,
          globalNotice: j.globalNotice || SCAFFOLD_GLOBAL_NOTICE,
          scaffolds: j.scaffolds,
        };
        return cache;
      }
    }
  } catch {
    /* fall through to the bundled fallback */
  }
  cache = { version: 0, globalNotice: SCAFFOLD_GLOBAL_NOTICE, scaffolds: [IMRAD_SCAFFOLD] };
  return cache;
}

/** Convenience: just the scaffold list. */
export const loadScaffolds = async (): Promise<Scaffold[]> => (await loadScaffoldCatalog()).scaffolds;

/** Resolve a scaffold by id from a loaded set, falling back to the first
 *  (generic) so an unknown/legacy id never yields undefined. */
export const pickScaffold = (scaffolds: Scaffold[], id: string): Scaffold =>
  scaffolds.find((s) => s.id === id) ?? scaffolds[0] ?? IMRAD_SCAFFOLD;
