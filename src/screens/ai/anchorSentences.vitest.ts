// §11 D92. Anchoring, pinned on the cases the R PAPER spike actually produced.
import { describe, it, expect } from 'vitest';
import { anchorSentences, fold, mergeLines, MIN_ANCHOR_CHARS, PageText, TextItem } from './anchorSentences';

/** Lay words out as a text layer would: one run per word, on numbered lines. */
function line(words: string[], y: number, startX = 50): TextItem[] {
  let x = startX;
  return words.map((w, i) => {
    const item: TextItem = { str: w + (i === words.length - 1 ? '' : ' '), x, y, w: w.length * 5, h: 10 };
    x += (w.length + 1) * 5;
    return item;
  });
}
function page(page: number, lines: TextItem[][]): PageText {
  const items = lines.flatMap((l, i) => {
    const copy = l.map((it) => ({ ...it }));
    copy[copy.length - 1].hasEOL = true;
    void i;
    return copy;
  });
  return { page, items, width: 595, height: 842 };
}

describe('fold — typography only, never words', () => {
  it('collapses the whitespace PDF extraction leaves behind', () => {
    // The real shape: "This  paper  presents" with doubled spaces.
    expect(fold('This  paper   presents')).toBe('This paper presents');
  });
  it('normalises ligatures, dashes and quotes', () => {
    expect(fold('the ﬁrst ﬂow')).toBe('the first flow');
    expect(fold('Firefly–Crow')).toBe('Firefly-Crow');
    expect(fold('“quoted’s”')).toBe('"quoted\'s"');
  });
  it('does NOT fold case or words — that would match look-alike sentences', () => {
    expect(fold('The Highest F1')).toBe('The Highest F1');
    expect(fold('colour')).not.toBe('color');
  });
});

describe('anchorSentences', () => {
  const p2 = page(2, [line(['Emotion', 'detection', 'in', 'social', 'media', 'text'], 700),
                      line(['must', 'deal', 'with', 'five', 'challenges', 'here.'], 686)]);
  const p3 = page(3, [line(['Convergence', 'is', 'reached', 'almost', '28%', 'faster.'], 700)]);

  it('locates a sentence that wraps across lines, as one band per line', () => {
    const r = anchorSentences(
      [{ seq: 1, sentence: 'Emotion  detection in social media text must deal with five challenges here.' }],
      [p2, p3],
    );
    expect(r.unplaced).toEqual([]);
    expect(r.anchors).toHaveLength(1);
    expect(r.anchors[0].page).toBe(2);
    expect(r.anchors[0].lines).toHaveLength(2); // two visual lines, two rectangles
  });

  /** THE LIVE BUG (§11 D92): the stored page is wrong ~8% of the time, so the
   *  text is the identity and every page is searched. */
  it('finds the sentence on the page it is REALLY on, not the one claimed', () => {
    const r = anchorSentences([{ seq: 9, sentence: 'Convergence is reached almost 28% faster.' }], [p2, p3]);
    expect(r.anchors[0].page).toBe(3);
  });

  /** THE PAPER REALLY DOES REPEAT ITSELF — three sentences twice, between the
   *  Discussion and the Conclusion. Order on both sides resolves it exactly. */
  it('maps the k-th planned copy to the k-th printed occurrence', () => {
    const repeated = 'A major drawback of this framework is that it is restricted.';
    const dup = page(5, [line(repeated.split(' '), 700), line(repeated.split(' '), 600)]);
    const r = anchorSentences(
      [{ seq: 1, sentence: repeated }, { seq: 2, sentence: repeated }],
      [dup],
    );
    expect(r.unplaced).toEqual([]);
    expect(r.anchors).toHaveLength(2);
    // Distinct places, in document order — never the same band twice.
    expect(r.anchors[0].lines[0].y).toBeGreaterThan(r.anchors[1].lines[0].y);
  });

  it('refuses a third copy rather than reusing one', () => {
    const s = 'A major drawback of this framework is that it is restricted.';
    const dup = page(5, [line(s.split(' '), 700), line(s.split(' '), 600)]);
    const r = anchorSentences([{ seq: 1, sentence: s }, { seq: 2, sentence: s }, { seq: 3, sentence: s }], [dup]);
    expect(r.anchors).toHaveLength(2);
    expect(r.unplaced).toEqual([
      expect.objectContaining({ seq: 3, reason: 'copies-exhausted' }),
    ]);
  });

  /** OMIT, NEVER APPROXIMATE. The column-break and figure-caption cases from
   *  the spike land here, and a nearest-match would put a highlight on the
   *  wrong sentence — which is worse than none. */
  it('omits a sentence that is not contiguous on any page, with a note', () => {
    const r = anchorSentences(
      [{ seq: 4, sentence: 'If the attention layer is removed it costs 2.1 points.' }],
      [p2, p3],
    );
    expect(r.anchors).toEqual([]);
    expect(r.unplaced[0].reason).toBe('not-contiguous');
    expect(r.unplaced[0].note).toMatch(/could not show you where it sits/);
    expect(r.unplaced[0].note).toMatch(/finding below is unaffected/);
  });

  it('omits a sentence too short to identify a place', () => {
    const r = anchorSentences([{ seq: 5, sentence: 'Fig. 3.' }], [p2]);
    expect(r.anchors).toEqual([]);
    expect(r.unplaced[0].reason).toBe('too-short');
    expect('Fig. 3.'.length).toBeLessThan(MIN_ANCHOR_CHARS);
  });

  it('never returns a rectangle for a sentence it could not place', () => {
    const r = anchorSentences(
      [{ seq: 1, sentence: 'Emotion detection in social media text must deal with five challenges here.' },
       { seq: 2, sentence: 'A sentence that appears nowhere in this document at all.' }],
      [p2, p3],
    );
    const placed = new Set(r.anchors.map((a) => a.seq));
    for (const u of r.unplaced) expect(placed.has(u.seq)).toBe(false);
    expect(placed.size + r.unplaced.length).toBe(2);
  });
});

describe('mergeLines — one band per line, not one per word', () => {
  it('merges runs sharing a baseline and keeps separate lines apart', () => {
    const rects = mergeLines([
      { str: 'a', x: 50, y: 700, w: 20, h: 10 },
      { str: 'b', x: 75, y: 700, w: 30, h: 12 },
      { str: 'c', x: 50, y: 686, w: 40, h: 10 },
    ]);
    expect(rects).toHaveLength(2);
    const top = rects[0];
    expect(top.x).toBe(50);
    expect(top.w).toBe(55); // 50 -> 105, one continuous band
    expect(top.h).toBe(12); // the tallest run on the line
  });
});
