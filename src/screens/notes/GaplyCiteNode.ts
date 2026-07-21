// Gaply — Research Paper Writer, Set B: the in-text citation node (SPIKE stage —
// schema + markdown round-trip only; the NodeView / picker / live display are B1).
//
// A custom inline ATOM node holding a stable `refId` (a citation_library id). The
// canonical markdown form is `[[cite:<refId>]]` — a wiki-link-style token markdown
// has no native rule for, so it can't collide with links/images/emphasis. It must
// round-trip through markdown-canonical storage (parse ⇄ serialize) losslessly,
// exactly like the gaply-image node — which is the ONE unknown this spike proves.
import { Node, mergeAttributes } from '@tiptap/core';

/** The canonical ref prefix used elsewhere (orderedRefIds regex in B2 keys off
 *  the stored `[[cite:<refId>]]` token, not this — but we keep the vocabulary). */
export const CITE_TOKEN_RE = /\[\[cite:([^\]\s]+)\]\]/g;

export const GaplyCite = Node.create({
  name: 'gaplyCite',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      refId: {
        default: '',
        parseHTML: (el) => (el as HTMLElement).getAttribute('data-gaply-cite') ?? '',
        renderHTML: (attrs) => (attrs.refId ? { 'data-gaply-cite': attrs.refId } : {}),
      },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-gaply-cite]' }];
  },

  renderHTML({ HTMLAttributes }) {
    // B1 replaces this with a NodeView that shows the formatted marker.
    return ['span', mergeAttributes(HTMLAttributes, { class: 'gaply-cite' })];
  },

  addStorage() {
    return {
      markdown: {
        // editor node → markdown
        serialize(state: { write: (s: string) => void }, node: { attrs: { refId: string } }) {
          state.write(`[[cite:${node.attrs.refId}]]`);
        },
        // markdown → editor node: a markdown-it inline rule that emits the HTML
        // our parseHTML recognizes. Registered BEFORE 'link' so `[[cite:` is
        // claimed before markdown-it's link rule can nibble the leading `[`.
        parse: {
          setup(markdownit: import('markdown-it')) {
            markdownit.inline.ruler.before('link', 'gaply_cite', (state, silent) => {
              const src = state.src;
              const start = state.pos;
              if (src.charCodeAt(start) !== 0x5b /* [ */ || src.slice(start, start + 7) !== '[[cite:') return false;
              const close = src.indexOf(']]', start + 7);
              if (close < 0) return false;
              const refId = src.slice(start + 7, close);
              if (!refId || /\s|\]/.test(refId)) return false; // malformed / empty → leave literal
              if (!silent) {
                const token = state.push('gaply_cite', 'span', 0);
                token.attrs = [['data-gaply-cite', refId]];
                token.content = refId;
              }
              state.pos = close + 2;
              return true;
            });
            markdownit.renderer.rules.gaply_cite = (tokens, idx) =>
              `<span data-gaply-cite="${tokens[idx].attrGet('data-gaply-cite') ?? ''}"></span>`;
          },
        },
      },
    };
  },
});
