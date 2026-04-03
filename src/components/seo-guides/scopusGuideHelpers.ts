export interface ScopusJournal {
  title: string;
  category: string;
  subcategory: string;
  issnLine: string;
  quartile: string;
  frequency: string;
  publisher: string;
  website: string;
  reviewTime: string;
  scope: string;
}

export function groupByCategoryNested(
  journals: ScopusJournal[]
): Map<string, Map<string, ScopusJournal[]>> {
  const m = new Map<string, Map<string, ScopusJournal[]>>();
  for (const j of journals) {
    if (!m.has(j.category)) m.set(j.category, new Map());
    const sub = m.get(j.category)!;
    if (!sub.has(j.subcategory)) sub.set(j.subcategory, []);
    sub.get(j.subcategory)!.push(j);
  }
  return m;
}

export function groupByCategoryOrdered(
  journals: ScopusJournal[]
): [string, Map<string, ScopusJournal[]>][] {
  const map = groupByCategoryNested(journals);
  const cats = Array.from(map.keys()).sort((a, b) => a.localeCompare(b));
  return cats.map((cat) => {
    const subMap = map.get(cat)!;
    const subs = Array.from(subMap.keys()).sort((a, b) => a.localeCompare(b));
    const ordered = new Map<string, ScopusJournal[]>();
    for (const s of subs) ordered.set(s, subMap.get(s)!);
    return [cat, ordered];
  });
}

/** Heuristic: stated peer-review windows that are often shorter (always verify on the journal site). */
export function isRelativelyFastReview(reviewTime: string): boolean {
  if (!reviewTime) return false;
  if (/rapid/i.test(reviewTime)) return true;
  const m = reviewTime.match(/(\d+)\s*-\s*(\d+)/);
  if (!m) return false;
  const a = parseInt(m[1], 10);
  const b = parseInt(m[2], 10);
  return a <= 4 && b <= 8;
}

/** Publishers that often operate fully OA brands or publish APC pages prominently — not a guarantee of “low” APC. */
export function publisherOftenEmphasizesOA(publisher: string): boolean {
  const p = publisher.toLowerCase();
  return (
    p.includes('plos') ||
    p.includes('mdpi') ||
    p.includes('frontiers') ||
    p.includes('hindawi') ||
    p.includes('biomed central') ||
    p.includes('bmc') ||
    p.includes('springeropen') ||
    p.includes('multidisciplinary digital publishing')
  );
}

export function slugifySegment(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}
