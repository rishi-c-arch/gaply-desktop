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

/** Live in-text markers keyed by refId. For numbered + author-date styles a
 *  reference renders the SAME marker everywhere, so a refId→marker map is exact.
 *  Dangling refs (id not in the library) are flagged missing, never crash. */
export async function computeMarkers(
  sections: { body: string }[],
  cslStyleId: string,
  library: Map<string, CslItem>,
): Promise<MarkerMap> {
  const map: MarkerMap = new Map();
  const unique = uniqueInOrder(orderedRefIds(sections));
  if (unique.length === 0) return map;
  const cited = unique.map((id) => library.get(id)).filter((x): x is CslItem => !!x);
  await engine.prepareStyle(cslStyleId);
  const { inText, citationOrder } = engine.renderCitations(cited, unique.map((id) => [id]), cslStyleId);
  citationOrder.forEach((id, k) => map.set(id, { marker: inText[k], missing: false }));
  for (const id of unique) if (!library.has(id)) map.set(id, { marker: '', missing: true });
  return map;
}
