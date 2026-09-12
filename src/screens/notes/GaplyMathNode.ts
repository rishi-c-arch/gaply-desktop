// Gaply — Research Paper Writer: the math node.
//
// THIS IS THE THIRD INSTANCE OF A PROVEN RECIPE, not new architecture. A custom
// inline ATOM node holding a stable payload, a canonical markdown token that
// markdown has no native rule for, and a markdown-it inline rule registered
// BEFORE 'link' so the token is claimed before anything can nibble at it.
// `gaplyCite` ([[cite:id]]) and `gaplyImage` proved it; this is the same shape
// with a different payload.
//
// WHY NOT `$…$`, WHICH IS WHAT EVERYONE EXPECTS.
//
// Not because markdown corrupts backslashes — measured, it does not: a typed
// `\frac{1}{x}` is stored escaped as `\\frac{1}{x}` and parses back to exactly
// what was typed, losslessly and stably, through the editor AND through the
// .docx exporter's own markdown-it pass. The reasons are different and they are
// about AMBIGUITY, which is the same reason `[[cite:…]]` exists:
//
//   1. `$` is ordinary prose. "a $5 fee", "costs $10–$20 per sample", "$US".
//      A `$…$` convention has to guess which dollar signs are money, and any
//      guess is wrong for somebody's paper.
//   2. Math has to be IDENTIFIABLE by the exporters. A .tex writer must know
//      "this span is math, emit it verbatim"; a Word writer must know "this span
//      becomes OMML". A token with no other meaning answers that exactly; a
//      heuristic over `$` answers it probably.
//   3. Rendering needs a node anyway. KaTeX renders into a discrete object the
//      user can click, edit and delete as one unit — which is a node, not a
//      styled run of text.
//
// THE ONE LIMIT, stated rather than discovered: the TeX payload may not contain
// the literal sequence `]]`, because that terminates the token. The input UI
// refuses such TeX with an honest message rather than storing something it
// cannot read back. In practice `]]` does not occur in the supported subset —
// `\right]` closes a single bracket and matrix environments use `\\` for rows.
import { Node, mergeAttributes } from '@tiptap/core';

/** Every math token in a body, both kinds. The capture groups are
 *  (1) the kind marker and (2) the raw TeX. Mirrors CITE_TOKEN_RE's role for
 *  the export/extraction side. */
export const MATH_TOKEN_RE = /\[\[math(-block)?:((?:(?!\]\])[\s\S])*)\]\]/g;

/** The TeX a token can carry. `]]` terminates the token, so it cannot appear. */
export const isStorableTex = (tex: string): boolean => tex.length > 0 && !tex.includes(']]');

/** Build the canonical stored token for a piece of math. */
export const mathToken = (tex: string, display: boolean): string =>
  `[[math${display ? '-block' : ''}:${tex}]]`;

/**
 * Register the `[[math:…]]` inline rule on a markdown-it instance.
 *
 * EXPORTED BECAUSE THE EXPORTERS NEED IT TOO. The .tex and .docx writers parse
 * the stored body with their own markdown-it, and without this rule the payload
 * is just text — so markdown-it's `escape` rule reaches inside a formula and
 * eats `\,`, turning `\int_0^1 x^2\,dx` into `\int_0^1 x^2,dx` on the way out.
 * The editor never saw that because the node's own parser registers this rule.
 * One implementation, registered everywhere the token is read.
 */
export function registerMathRule(markdownit: import('markdown-it')): void {
  markdownit.inline.ruler.before('link', 'gaply_math', (state, silent) => {
    const src = state.src;
    const start = state.pos;
    if (src.charCodeAt(start) !== 0x5b /* [ */) return false;
    let display = false;
    let open = -1;
    if (src.startsWith('[[math:', start)) open = start + 7;
    else if (src.startsWith('[[math-block:', start)) { open = start + 13; display = true; }
    else return false;
    const close = src.indexOf(']]', open);
    if (close < 0) return false;
    const tex = src.slice(open, close);
    if (!tex) return false; // `[[math:]]` is not math — leave it literal
    if (!silent) {
      const token = state.push('gaply_math', 'span', 0);
      token.attrs = [
        ['data-gaply-math', tex],
        ...(display ? [['data-gaply-math-display', '1'] as [string, string]] : []),
      ];
      token.content = tex;
    }
    state.pos = close + 2;
    return true;
  });
}

/** Read a `gaply_math` token's payload back out, for the export walks. */
export const mathTokenPayload = (t: { attrGet: (k: string) => string | null }): { tex: string; display: boolean } => ({
  tex: t.attrGet('data-gaply-math') ?? '',
  display: t.attrGet('data-gaply-math-display') === '1',
});

export const GaplyMath = Node.create({
  name: 'gaplyMath',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      tex: {
        default: '',
        parseHTML: (el) => (el as HTMLElement).getAttribute('data-gaply-math') ?? '',
        renderHTML: (attrs) => (attrs.tex ? { 'data-gaply-math': attrs.tex } : {}),
      },
      // Display math is still an INLINE atom — it simply sits alone in its own
      // paragraph and is centred by CSS. One node type, one token, one
      // serializer; ProseMirror fixes inline/block per node type, and two node
      // types would mean two of everything for a styling difference.
      display: {
        default: false,
        parseHTML: (el) => (el as HTMLElement).getAttribute('data-gaply-math-display') === '1',
        renderHTML: (attrs) => (attrs.display ? { 'data-gaply-math-display': '1' } : {}),
      },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-gaply-math]' }];
  },

  renderHTML({ HTMLAttributes }) {
    // Replaced by the KaTeX NodeView in the editor; this is the bare fallback.
    return ['span', mergeAttributes(HTMLAttributes, { class: 'gaply-math' })];
  },

  addStorage() {
    return {
      markdown: {
        // editor node → markdown. `state.write` emits VERBATIM (no markdown
        // escaping), so the TeX inside the token keeps its single backslashes.
        serialize(
          state: { write: (s: string) => void },
          node: { attrs: { tex: string; display: boolean } },
        ) {
          state.write(mathToken(node.attrs.tex, node.attrs.display));
        },
        // markdown → editor node. Registered BEFORE 'link' so `[[math:` is
        // claimed before markdown-it's link rule can take the leading `[`, and
        // — the part that matters for TeX — before the 'escape' rule can reach
        // any backslash inside the payload, because state.pos jumps past the
        // whole token in one step.
        parse: {
          setup(markdownit: import('markdown-it')) {
            registerMathRule(markdownit);
            markdownit.renderer.rules.gaply_math = (tokens, idx) => {
              const t = tokens[idx];
              const tex = t.attrGet('data-gaply-math') ?? '';
              const disp = t.attrGet('data-gaply-math-display') === '1' ? ' data-gaply-math-display="1"' : '';
              // Attribute-escape: the TeX goes into an HTML attribute on the way
              // to parseHTML, so `"` and `&` must not break out of it.
              const safe = tex.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
              return `<span data-gaply-math="${safe}"${disp}></span>`;
            };
          },
        },
      },
    };
  },
});
