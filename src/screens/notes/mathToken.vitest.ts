// Math storage — the markdown round-trip, which is the ONE thing that has to be
// exactly right before anything is built on top of it.
//
// Same pins as citeToken.vitest.ts, for the same reason: the body is canonical
// markdown in sqlite, so a node is only safe if node → markdown → node is
// lossless and idempotent. The fiddly direction is markdown → node: a stored
// body arriving from disk must become a real node, not literal text.
//
// The payload here is harder than a citation's uuid. TeX is full of the exact
// characters markdown wants to interpret — backslashes, underscores, asterisks,
// braces, carets, dollar signs — so these cases are the point, not decoration.
import { describe, expect, it } from 'vitest';
import { Editor } from '@tiptap/core';
import { richExtensions, mdOf } from './RichBody';
import { MATH_TOKEN_RE, mathToken, isStorableTex } from './GaplyMathNode';

const ed = (content: string) => new Editor({ extensions: richExtensions('', { citations: true }), content });

/** markdown → doc → markdown */
const rt = (md: string): string => { const e = ed(md); const out = mdOf(e); e.destroy(); return out; };

/** Every math node in a parsed body, as {tex, display}. */
const nodesIn = (md: string): Array<{ tex: string; display: boolean }> => {
  const e = ed(md);
  const found: Array<{ tex: string; display: boolean }> = [];
  e.state.doc.descendants((n) => {
    if (n.type.name === 'gaplyMath') found.push({ tex: n.attrs.tex as string, display: !!n.attrs.display });
  });
  e.destroy();
  return found;
};

/* ---------------- (1) node → markdown → node, idempotent ---------------- */

describe('(1) round-trip is lossless and stable', () => {
  const TEX = [
    '\\frac{1}{x}',
    '\\int_0^1 x^2\\,dx',
    '\\sum_{i=1}^{n} \\alpha_i',
    'E = mc^2',
    '\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}',
    '\\begin{cases} x & x \\ge 0 \\\\ -x & x < 0 \\end{cases}',
    '\\text{rate}_{\\max} \\approx 3.5\\%',      // % and _ — markdown-hostile
    'a * b + c_d - e^f',                          // bare * and _ and ^
    '\\hat{\\beta} = (X^{\\top}X)^{-1}X^{\\top}y',
  ];

  it('every supported construct survives markdown storage unchanged', () => {
    for (const tex of TEX) {
      const md = `Before ${mathToken(tex, false)} after.`;
      expect(rt(md), `round-trip of ${tex}`).toBe(md);
      expect(nodesIn(md)).toEqual([{ tex, display: false }]);
    }
  });

  it('a second pass changes nothing (idempotent, like the cite token)', () => {
    const md = `x ${mathToken('\\frac{a}{b}', false)} y\n\n${mathToken('\\int_0^1 f(x)\\,dx', true)}`;
    const once = rt(md);
    expect(once).toBe(md);
    expect(rt(once)).toBe(once);
  });

  it('display and inline are distinct and both survive', () => {
    const md = `${mathToken('a+b', false)}\n\n${mathToken('a+b', true)}`;
    expect(rt(md)).toBe(md);
    expect(nodesIn(md)).toEqual([{ tex: 'a+b', display: false }, { tex: 'a+b', display: true }]);
  });
});

/* -------- (2) the FIDDLY direction: stored markdown becomes a node -------- */

describe('(2) a stored body loads as real nodes, not literal text', () => {
  it('raw [[math:…]] in a loaded body becomes a node', () => {
    expect(nodesIn('Given [[math:\\frac{1}{x}]] we proceed.')).toEqual([{ tex: '\\frac{1}{x}', display: false }]);
  });

  it('the backslashes arrive INTACT — single, not doubled or eaten', () => {
    // The whole reason the token is claimed before markdown-it's 'escape' rule.
    const [n] = nodesIn('[[math:\\int_0^1 \\frac{1}{x}\\,dx]]');
    expect(n.tex).toBe('\\int_0^1 \\frac{1}{x}\\,dx');
    expect(n.tex).not.toContain('\\\\int');
    expect(n.tex).toContain('\\,'); // the thin space markdown would otherwise eat
  });

  it('a token ADJACENT to text (no spaces) still parses', () => {
    expect(nodesIn('rate=[[math:\\alpha]]per second')).toEqual([{ tex: '\\alpha', display: false }]);
  });

  it('multiple tokens in one paragraph keep their order', () => {
    expect(nodesIn('[[math:a]] then [[math:b]] then [[math-block:c]]').map((n) => n.tex)).toEqual(['a', 'b', 'c']);
  });
});

/* ---------------- (3) malformed cases stay literal text ------------------ */

describe('(3) malformed tokens stay literal — never a silent half-node', () => {
  it.each([
    ['[[math:]]', 'empty payload'],
    ['[[math:\\frac{1}{x}', 'unterminated'],
    ['[[maths:\\alpha]]', 'wrong marker'],
    ['[math:\\alpha]', 'single brackets'],
    ['[[ math:\\alpha ]]', 'space before the marker'],
  ])('%s → no node (%s)', (src) => {
    expect(nodesIn(src)).toEqual([]);
  });

  it('BACKWARD COMPAT: a body with no math is untouched by the extension', () => {
    const md = 'Plain prose with **bold**, a `code span`, and a $5 fee.';
    expect(rt(md)).toBe(md);
    expect(nodesIn(md)).toEqual([]);
  });

  it('a dollar sign in prose is NOT math — the reason the token exists', () => {
    const md = 'The assay costs $10–$20 per sample.';
    expect(rt(md)).toBe(md);
    expect(nodesIn(md)).toEqual([]);
  });
});

/* ------------------- (4) the storage limit, enforced --------------------- */

describe('(4) the ]] limit is stated and guarded, not discovered', () => {
  it('isStorableTex refuses TeX that would terminate its own token', () => {
    expect(isStorableTex('\\frac{1}{x}')).toBe(true);
    expect(isStorableTex('\\left[\\right]')).toBe(true);
    expect(isStorableTex('a]]b')).toBe(false);
    expect(isStorableTex('')).toBe(false);
  });

  it('MATH_TOKEN_RE extracts every token for the exporters', () => {
    const body = 'x [[math:\\alpha]] y\n\n[[math-block:\\sum_i x_i]]';
    const found = Array.from(body.matchAll(MATH_TOKEN_RE)).map((m) => ({ block: !!m[1], tex: m[2] }));
    expect(found).toEqual([
      { block: false, tex: '\\alpha' },
      { block: true, tex: '\\sum_i x_i' },
    ]);
  });
});

/* --------------- (5) it coexists with the other two nodes ---------------- */

describe('(5) math, citations and images in one body', () => {
  it('math and citations round-trip together, in order', () => {
    const md = 'As [[cite:ref-a]] showed, [[math:\\alpha > 0]] holds.\n\n[[math-block:\\int_0^1 x\\,dx]]\n\nAnd [[cite:ref-b]] agrees.';
    expect(rt(md)).toBe(md);
  });

  // WAS a KNOWN LIMIT pinned here while it was still broken; now it pins the
  // fix. Images used to be schema group 'block' (@tiptap/extension-image
  // defaults to inline:false), so ProseMirror split every paragraph around one:
  // an inline image was lifted out and lost the spaces beside it, and an
  // image-only paragraph was welded to whatever followed. `inline: true` in
  // GaplyImageNode makes the node the shape the rest of the lane always assumed.
  it('images hold their place — inline, and as their own paragraph', () => {
    const inline = 'See ![](gaply-image://abc123.png) and more text.';
    expect(rt(inline)).toBe(inline);

    const own = '![](gaply-image://abc123.png)\n\nplain text';
    expect(rt(own)).toBe(own);

    const between = 'Before.\n\n![](gaply-image://abc123.png)\n\nAfter.';
    expect(rt(between)).toBe(between);
  });

  it('math, by contrast, holds its place in both positions', () => {
    expect(rt('See [[math:\\alpha]] and more text.')).toBe('See [[math:\\alpha]] and more text.');
    expect(rt('[[math-block:x]]\n\nplain text')).toBe('[[math-block:x]]\n\nplain text');
  });
});
