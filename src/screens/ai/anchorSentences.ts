// Gaply — locating a judged sentence on the page it is actually printed on
// (§11 D92). Pure: it takes text-with-positions and returns rectangles, so it
// is testable without a PDF, a renderer or a webview.
//
// THE RULE THIS MODULE EXISTS TO KEEP: a highlight on the wrong sentence is
// worse than a missing one. Every path here either locates a sentence exactly
// or refuses and says why. There is no nearest-match, no fuzzy threshold, no
// partial placement.

/** One positioned run of text, as a PDF text layer reports it. */
export interface TextItem {
  str: string;
  /** PDF user space, origin bottom-left; `y` is the BASELINE. */
  x: number;
  y: number;
  w: number;
  h: number;
  /** The layer marks a line end here, which stands in for a space. */
  hasEOL?: boolean;
}

export interface PageText {
  page: number;
  items: TextItem[];
  width: number;
  height: number;
}

/** A rectangle in PDF user space, origin bottom-left. */
export interface Rect { x: number; y: number; w: number; h: number }

export interface Anchor {
  seq: number;
  /** Where the sentence REALLY is, which is not always where the item says. */
  page: number;
  /** One rectangle per visual line, so a wrapped sentence is a set of bands. */
  lines: Rect[];
}

export type UnplacedReason = 'not-contiguous' | 'too-short' | 'copies-exhausted';

export interface Unplaced {
  seq: number;
  reason: UnplacedReason;
  /** Plain English, for the detail list — the reader is a researcher. */
  note: string;
}

export interface AnchorResult {
  anchors: Anchor[];
  unplaced: Unplaced[];
}

/** Sentences shorter than this cannot identify themselves on a page. */
export const MIN_ANCHOR_CHARS = 12;

export const UNPLACED_NOTE: Record<UnplacedReason, string> = {
  'not-contiguous':
    'This sentence was judged, but Gaply could not show you where it sits on the page — the text extractor and the page disagree about its order, which happens around figure captions and column breaks. The finding below is unaffected.',
  'too-short':
    'This sentence was judged, but it is too short to identify a unique place on the page, so it is not highlighted. The finding below is unaffected.',
  'copies-exhausted':
    'This sentence appears more than once and Gaply could not tell which copy this finding refers to, so it is not highlighted rather than highlighted in the wrong place. The finding below is unaffected.',
};

/**
 * Collapse the differences between an extractor's text and a text layer's.
 *
 * WHITESPACE AND TYPOGRAPHY ONLY. Case and words are untouched: a normaliser
 * that folded those would start matching sentences that merely resemble each
 * other, which is the failure this module exists to prevent.
 */
export function fold(s: string): string {
  return s
    .replace(/­/g, '') // soft hyphen: invisible, and not in the layer
    .replace(/[‐-―]/g, '-')
    .replace(/[‘’]/g, "'")
    .replace(/[“”]/g, '"')
    .replace(/ﬁ/g, 'fi')
    .replace(/ﬂ/g, 'fl')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Page text folded to one string, plus a char -> item index map. */
function foldPage(items: TextItem[]): { text: string; map: number[] } {
  let text = '';
  const map: number[] = [];
  let prevSpace = true; // leading whitespace is dropped, as `fold` trims
  items.forEach((it, i) => {
    const piece = (it.str ?? '') + (it.hasEOL ? ' ' : '');
    for (const raw of piece) {
      if (/\s/.test(raw)) {
        if (prevSpace) continue;
        prevSpace = true;
        text += ' ';
        map.push(i);
        continue;
      }
      prevSpace = false;
      const f = fold(raw);
      // `fold` can expand one char into two (ligatures) or none (soft hyphen).
      for (const ch of f) {
        text += ch;
        map.push(i);
      }
    }
  });
  // Trailing space, to match `fold`'s trim.
  while (text.endsWith(' ')) {
    text = text.slice(0, -1);
    map.pop();
  }
  return { text, map };
}

/**
 * Merge the runs a sentence covers into ONE rectangle per visual line.
 *
 * A text layer emits a sentence as many short runs; drawing each gives a
 * dotted band with gaps between words. Grouping by baseline gives the
 * continuous highlight a reader expects.
 */
export function mergeLines(items: TextItem[]): Rect[] {
  const byLine = new Map<number, Rect>();
  for (const it of items) {
    // Baselines jitter by fractions of a point within one line.
    const key = Math.round(it.y / 3);
    const cur = byLine.get(key);
    if (!cur) {
      byLine.set(key, { x: it.x, y: it.y, w: it.w, h: it.h });
      continue;
    }
    const right = Math.max(cur.x + cur.w, it.x + it.w);
    cur.x = Math.min(cur.x, it.x);
    cur.w = right - cur.x;
    cur.h = Math.max(cur.h, it.h);
  }
  return Array.from(byLine.values()).sort((a, b) => b.y - a.y);
}

/**
 * Locate each sentence, or refuse.
 *
 * `sentences` must be in DOCUMENT ORDER — that ordering is what resolves a
 * sentence the manuscript prints twice, and it is the only reason duplicates
 * can be placed at all rather than dropped as ambiguous.
 */
export function anchorSentences(
  sentences: Array<{ seq: number; sentence: string }>,
  pages: PageText[],
): AnchorResult {
  const folded = pages.map((p) => ({ page: p, ...foldPage(p.items) }));
  const anchors: Anchor[] = [];
  const unplaced: Unplaced[] = [];
  /** How many copies of this exact text have already been placed. */
  const used = new Map<string, number>();

  for (const s of sentences) {
    const needle = fold(s.sentence);
    if (needle.length < MIN_ANCHOR_CHARS) {
      unplaced.push({ seq: s.seq, reason: 'too-short', note: UNPLACED_NOTE['too-short'] });
      continue;
    }

    // EVERY page. The stored page number is a claim, and on real manuscripts it
    // is wrong about 8% of the time (§11 D92); the text is the identity.
    const hits: Array<{ pageIdx: number; at: number }> = [];
    folded.forEach((f, pageIdx) => {
      let at = -1;
      // eslint-disable-next-line no-cond-assign
      while ((at = f.text.indexOf(needle, at + 1)) >= 0) hits.push({ pageIdx, at });
    });

    if (hits.length === 0) {
      unplaced.push({ seq: s.seq, reason: 'not-contiguous', note: UNPLACED_NOTE['not-contiguous'] });
      continue;
    }

    const k = used.get(needle) ?? 0;
    if (k >= hits.length) {
      unplaced.push({ seq: s.seq, reason: 'copies-exhausted', note: UNPLACED_NOTE['copies-exhausted'] });
      continue;
    }
    used.set(needle, k + 1);

    // Document order on both sides, so the k-th planned copy takes the k-th
    // printed occurrence. Exact, not a guess.
    hits.sort((a, b) => a.pageIdx - b.pageIdx || a.at - b.at);
    const { pageIdx, at } = hits[k];
    const f = folded[pageIdx];
    const covered = new Set(f.map.slice(at, at + needle.length));
    const items = Array.from(covered).map((i) => f.page.items[i]).filter(Boolean);
    anchors.push({ seq: s.seq, page: f.page.page, lines: mergeLines(items) });
  }

  return { anchors, unplaced };
}
