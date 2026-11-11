// Gaply — a list of chosen columns in the user's order (Research OS, Phase 9A-2, decision 684):
// the one home of the four pure functions and offeredBy that RelatedRanksSettings.tsx held
// (Phase 8Z-2, decisions 653 and 668), now shared with LinearRegressionSettings.tsx.
// RelatedRanksSettings.tsx re-exports the four, so its imports and tests are unchanged.
//
// Each function returns a new list and leaves its argument as it was. What is held here is
// nothing: the lists are the caller's state.
import type { DatasetView } from './describeTypes';
import { columnLabel } from './window/windowWording';

/* The list's changes (decision 668), pure: each returns a new list and leaves its argument as it was. */

/** `alias` at the end, or the list unchanged when it is already in it. */
export const addColumn = (list: readonly string[], alias: string): string[] => (list.includes(alias) ? [...list] : [...list, alias]);

/** The list without `alias`. */
export const removeColumn = (list: readonly string[], alias: string): string[] => list.filter((c) => c !== alias);

/** `alias` swapped with its neighbour `by` places away in `shown` (the list as drawn), or the
 *  list unchanged at either end or when `alias` is not shown. */
function moveColumn(list: readonly string[], alias: string, by: -1 | 1, shown: readonly string[]): string[] {
  const at = shown.indexOf(alias);
  const to = at + by;
  const next = [...list];
  if (at < 0 || to < 0 || to >= shown.length) return next;
  const [i, j] = [list.indexOf(alias), list.indexOf(shown[to])];
  if (i < 0 || j < 0) return next;
  next[i] = shown[to];
  next[j] = alias;
  return next;
}
export const moveColumnUp = (list: readonly string[], alias: string, shown: readonly string[] = list): string[] => moveColumn(list, alias, -1, shown);
export const moveColumnDown = (list: readonly string[], alias: string, shown: readonly string[] = list): string[] => moveColumn(list, alias, 1, shown);

/** The candidate codes of `columns`, ascending, with where each appears: the union over the
 *  chosen columns. Twin of AlphaSettings.tsx's copy (its `offered`, :131-140), which is left
 *  as it is and recorded as a later step's to fold in here (decision 684). */
export function offeredBy(view: DatasetView | null, columns: readonly string[]) {
  const where = new Map<number, Array<{ header: string; cells: number }>>();
  for (const c of view?.profile.columns ?? []) {
    if (!columns.includes(c.alias)) continue;
    for (const s of c.suspectMissingCodes) where.set(s.code, [...(where.get(s.code) ?? []), { header: columnLabel(c), cells: s.cells }]);
  }
  return Array.from(where.keys())
    .sort((a, b) => a - b)
    .map((code) => ({ code, where: where.get(code) ?? [] }));
}
