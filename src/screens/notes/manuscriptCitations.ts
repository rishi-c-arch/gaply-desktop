// Gaply — Research Paper Writer, Set B1: citation cluster derivation + live
// marker computation. Pure logic (the crux) + a signature-gated marker map so
// prose keystrokes don't recompute citations.
import { CITE_TOKEN_RE } from './GaplyCiteNode';
import { CslItem } from '../citations/citationTypes';
// Namespace import so the perf pin can spy on renderCitations call-count.
import * as engine from '../citations/cslEngine';

/** Every gaply-cite refId across the sections IN DOCUMENT ORDER (section order ×
 *  within-section token order). This is the bridge to the proven renderCitations:
 *  document order = the order the user reads them, which drives numbering. */
export const orderedRefIds = (sections: { body: string }[]): string[] => {
  const ids: string[] = [];
  for (const s of sections) {
    for (const token of s.body.match(CITE_TOKEN_RE) ?? []) {
      ids.push(token.replace(/\[\[cite:|\]\]/g, ''));
    }
  }
  return ids;
};

/** First-appearance unique order (= numbered order for numbered styles). */
export const uniqueInOrder = (ids: string[]): string[] => ids.filter((id, i) => ids.indexOf(id) === i);

/** A cheap change-signature: recompute markers ONLY when this changes — i.e. on
 *  a citation insert/delete/reorder, a style switch, OR a LIBRARY presence change
 *  (a ref added/removed → a ⚠ chip must resolve or appear live). NOT on prose
 *  keystrokes, and NOT on library METADATA edits (ids only, so a title/author
 *  tweak doesn't churn — only presence/absence flips a marker). */
export const signatureOf = (
  sections: { body: string }[],
  cslStyleId: string,
  libraryIds: Iterable<string> = [],
): string =>
  orderedRefIds(sections).join('|') + '||' + cslStyleId + '||' + Array.from(libraryIds).sort().join(',');

export interface Marker { marker: string; missing: boolean }
export type MarkerMap = Map<string, Marker>;

/** The full citation render for a manuscript — the SINGLE path used by BOTH live
 *  display and .docx export, so in-text markers and the bibliography are always
 *  produced identically (the correctness guarantee: exported [1] == live [1] ==
 *  References [1]). Every field derives from one renderCitations pass over
 *  document-order clusters. */
export interface CitationRender {
  /** refId → live in-text marker (numbered/author-date render a ref identically
   *  everywhere, so a refId map is exact). Dangling refs flagged missing. */
  markers: MarkerMap;
  /** The in-text marker for the k-th [[cite]] token in DOCUMENT order — used for
   *  position-accurate export replacement. A dangling token renders '[?]'. */
  perToken: string[];
  /** The reference list in the chosen style (numbered order or alphabetical). */
  bibliography: string;
  /** Cited ids not in the library (deleted/dangling). */
  dangling: string[];
  /** Any citations present at all (drives auto-References vs manual). */
  hasCitations: boolean;
}

export async function renderManuscriptCitations(
  sections: { body: string }[],
  cslStyleId: string,
  library: Map<string, CslItem>,
): Promise<CitationRender> {
  const ids = orderedRefIds(sections); // ALL tokens, document order (with repeats)
  if (ids.length === 0) return { markers: new Map(), perToken: [], bibliography: '', dangling: [], hasCitations: false };
  const unique = uniqueInOrder(ids);
  const cited = unique.map((id) => library.get(id)).filter((x): x is CslItem => !!x);
  await engine.prepareStyle(cslStyleId);
  // One token = one single-id cluster, in document order (B1-scope: no merged
  // clusters yet). renderCitations reorders internally + returns inText aligned
  // to these input positions.
  const { inText, bibliography, dangling } = engine.renderCitations(cited, ids.map((id) => [id]), cslStyleId);
  const danglingSet = new Set(dangling);
  const markers: MarkerMap = new Map();
  for (const id of unique) {
    markers.set(id, danglingSet.has(id) ? { marker: '', missing: true } : { marker: inText[ids.indexOf(id)], missing: false });
  }
  const perToken = ids.map((id, k) => (danglingSet.has(id) ? '[?]' : inText[k]));
  return { markers, perToken, bibliography, dangling, hasCitations: true };
}

/** Live in-text markers keyed by refId (B1 display). Thin view over the shared
 *  renderer so display and export never diverge. */
export async function computeMarkers(
  sections: { body: string }[],
  cslStyleId: string,
  library: Map<string, CslItem>,
): Promise<MarkerMap> {
  return (await renderManuscriptCitations(sections, cslStyleId, library)).markers;
}
