// Gaply — Research Paper Writer: within-section markdown → LaTeX.
//
// THE SAME WALK AS manuscriptRichDocx.ts, A DIFFERENT BACKEND. That file parses
// each section body with markdown-it and emits docx objects; this one emits
// LaTeX from the same token stream. Keeping the two structurally parallel is
// what makes them agree about the document, and it means the citation-ordering
// guarantee B2 proved (in-text marker k == References entry k, in document
// order, across every section) comes along rather than being re-derived here.
//
// The shared document-order counter is the mechanism: `resolveCites` advances it
// once per [[cite:…]] token in exactly the order `orderedRefIds` walks, so
// perToken[k] lines up whether the token sits in prose, inside bold, in a list
// item, or in a table cell.
import MarkdownIt from 'markdown-it';
import type Token from 'markdown-it/lib/token';
import { CITE_TOKEN_RE } from './GaplyCiteNode';
import { registerMathRule, mathTokenPayload } from './GaplyMathNode';
import { CitationRender } from './manuscriptCitations';
import { IMAGE_REF_PREFIX } from './noteImages';
import { proseToTex, escapeTex } from './latexEscape';

// breaks:true matches the editor, exactly as the docx walk does. `breaks` is a
// renderer option and does not change tokenisation, so the line-break decision
// is made below on the softbreak/hardbreak tokens themselves.
const mdit = new MarkdownIt({ html: false, linkify: false, breaks: true });
// The math rule MUST be registered here too. Without it a formula is just text
// to markdown-it, whose `escape` rule then reaches inside and eats `\,` —
// `\int_0^1 x^2\,dx` would export as `\int_0^1 x^2,dx`. Registered before
// 'link', so state.pos jumps the whole token and nothing touches its backslashes.
registerMathRule(mdit);

export interface LatexCtx {
  citations?: CitationRender;
  /** Shared document-order citation counter — see the header. */
  counter: { i: number };
  /** ref → the figure filename written into the bundle, e.g. "figures/fig1.png". */
  figures: Map<string, string>;
  /** Collected in walk order so the caller knows which files the bundle needs. */
  figureOrder: string[];
}

/** A placeholder used while escaping so math and citations survive prose
 *  escaping untouched. \u0000 cannot occur in a manuscript body. */
const HOLD = '\u0000';

/**
 * Escape a run of text while keeping its math and citations verbatim.
 *
 * Order matters and is the whole trick: pull the tokens OUT first, escape what
 * is left as prose, then put the tokens back. Escaping first would turn
 * `\frac{1}{x}` into `\textbackslash{}frac\{1\}\{x\}`, and resolving citations
 * first would expose a marker like `[1]` to prose escaping.
 */
function escapeKeepingTokens(text: string, ctx: LatexCtx): string {
  const held: string[] = [];
  const stash = (s: string): string => {
    held.push(s);
    return `${HOLD}${held.length - 1}${HOLD}`;
  };

  // Math arrives as its own token (see registerMathRule above), so only
  // citations need holding out of prose escaping here.
  const work = text.replace(CITE_TOKEN_RE, () => {
    const marker = ctx.citations?.hasCitations
      ? (ctx.citations.perToken[ctx.counter.i++] ?? '[?]')
      : (ctx.counter.i++, '');
    // The marker is citeproc output (e.g. "[1]" or "(He, 2016)") — prose, so it
    // is escaped like prose, not injected raw.
    return stash(proseToTex(marker));
  });

  const escaped = proseToTex(work);
  return escaped.replace(new RegExp(`${HOLD}(\\d+)${HOLD}`, 'g'), (_m, i) => held[Number(i)]);
}

/** Inline tokens → a LaTeX run. */
function inlineToTex(children: Token[], ctx: LatexCtx): string {
  let out = '';
  let linkHref: string | null = null;
  let linkText = '';

  for (const t of children) {
    switch (t.type) {
      case 'text': {
        const s = escapeKeepingTokens(t.content, ctx);
        if (linkHref !== null) linkText += t.content;
        out += s;
        break;
      }
      case 'strong_open': out += '\\textbf{'; break;
      case 'strong_close': out += '}'; break;
      case 'em_open': out += '\\emph{'; break;
      case 'em_close': out += '}'; break;
      case 's_open': out += '\\sout{'; break;
      case 's_close': out += '}'; break;
      case 'code_inline': {
        // Verbatim, but the counter still advances for any raw [[cite]] so the
        // document-order alignment can never drift (the docx walk does the same).
        const inner = t.content.replace(CITE_TOKEN_RE, () => { ctx.counter.i++; return ''; });
        out += `\\texttt{${escapeTex(inner)}}`;
        break;
      }
      case 'link_open':
        linkHref = t.attrGet('href') ?? '';
        linkText = '';
        break;
      case 'link_close': {
        const href = linkHref ?? '';
        const shown = linkText || href;
        linkHref = null;
        // The anchor text has already been emitted as escaped prose. Append the
        // address unless it is already visible: a reviewer reading a printed
        // PDF cannot hover a link, which is why the .docx export does the same.
        // `\url` takes the URL verbatim — prose-escaping it would break it.
        if (href && !shown.includes(href)) out += ` (\\url{${href}})`;
        break;
      }
      case 'gaply_math': {
        // VERBATIM. The payload is the author's own LaTeX; prose-escaping it
        // would turn \frac{1}{x} into \textbackslash{}frac\{1\}\{x\}.
        const { tex, display } = mathTokenPayload(t);
        out += display ? `\\[${tex}\\]` : `$${tex}$`;
        break;
      }
      case 'softbreak':
      case 'hardbreak':
        out += '\\\\\n';
        break;
      case 'image': {
        const src = t.attrGet('src') ?? '';
        const file = ctx.figures.get(src);
        if (file) {
          if (!ctx.figureOrder.includes(src)) ctx.figureOrder.push(src);
          out += `\\includegraphics[max width=\\linewidth]{${file}}`;
        } else {
          out += `\\texttt{[figure: ${escapeTex(src.replace(IMAGE_REF_PREFIX, ''))}]}`;
        }
        break;
      }
      default: break;
    }
  }
  return out;
}

const matchClose = (tokens: Token[], open: number, openType: string, closeType: string): number => {
  let depth = 0;
  for (let j = open; j < tokens.length; j++) {
    if (tokens[j].type === openType) depth += 1;
    else if (tokens[j].type === closeType) { depth -= 1; if (depth === 0) return j; }
  }
  return tokens.length - 1;
};

/** A GFM table → tabular. The column spec comes from markdown's alignment row,
 *  which is the only alignment information GFM carries. */
function tableToTex(tokens: Token[], ctx: LatexCtx): string {
  const rows: Array<{ cells: string[]; header: boolean }> = [];
  const align: string[] = [];
  let i = 0;
  while (i < tokens.length) {
    if (tokens[i].type === 'tr_open') {
      const trClose = matchClose(tokens, i, 'tr_open', 'tr_close');
      const cells: string[] = [];
      let header = false;
      let j = i + 1;
      while (j < trClose) {
        if (tokens[j].type === 'th_open' || tokens[j].type === 'td_open') {
          const isTh = tokens[j].type === 'th_open';
          header = header || isTh;
          if (isTh) {
            const style = tokens[j].attrGet('style') ?? '';
            align.push(style.includes('right') ? 'r' : style.includes('center') ? 'c' : 'l');
          }
          const close = matchClose(tokens, j, tokens[j].type, isTh ? 'th_close' : 'td_close');
          const inline = tokens[j + 1];
          const body = inline?.type === 'inline' ? inlineToTex(inline.children ?? [], ctx) : '';
          cells.push(isTh ? `\\textbf{${body}}` : body);
          j = close + 1;
        } else j += 1;
      }
      rows.push({ cells, header });
      i = trClose + 1;
    } else i += 1;
  }
  if (rows.length === 0) return '';
  const cols = align.length ? align.join('') : 'l'.repeat(rows[0].cells.length);
  const lines = [`\\begin{tabular}{${cols}}`, '\\toprule'];
  rows.forEach((r, idx) => {
    lines.push(`${r.cells.join(' & ')} \\\\`);
    if (idx === 0 && r.header) lines.push('\\midrule');
  });
  lines.push('\\bottomrule', '\\end{tabular}');
  // Tables are floated so LaTeX can place them; in two-column layouts a wide
  // table would otherwise overflow its column silently.
  return ['\\begin{table}[htbp]', '\\centering', ...lines, '\\end{table}'].join('\n');
}

function listToTex(tokens: Token[], ctx: LatexCtx, ordered: boolean): string {
  const env = ordered ? 'enumerate' : 'itemize';
  const items: string[] = [];
  let i = 0;
  while (i < tokens.length) {
    if (tokens[i].type === 'list_item_open') {
      const close = matchClose(tokens, i, 'list_item_open', 'list_item_close');
      items.push(`  \\item ${blocksToTex(tokens.slice(i + 1, close), ctx).trim()}`);
      i = close + 1;
    } else i += 1;
  }
  return [`\\begin{${env}}`, ...items, `\\end{${env}}`].join('\n');
}

/** Block tokens → LaTeX. Mirrors `tokensToBlocks` in the docx walk. */
function blocksToTex(tokens: Token[], ctx: LatexCtx): string {
  const out: string[] = [];
  let i = 0;
  while (i < tokens.length) {
    const t = tokens[i];
    if (t.type === 'paragraph_open') {
      const inline = tokens[i + 1];
      const body = inline?.type === 'inline' ? inlineToTex(inline.children ?? [], ctx) : '';
      // A paragraph that is ONLY a figure becomes a real float with a caption
      // slot, rather than an image wedged into a text paragraph.
      if (/^\\includegraphics\[[^\]]*\]\{[^}]*\}$/.test(body.trim())) {
        out.push(['\\begin{figure}[htbp]', '\\centering', body.trim(), '\\end{figure}'].join('\n'));
      } else if (body.trim()) {
        out.push(body);
      }
      i += inline?.type === 'inline' ? 3 : 2;
    } else if (t.type === 'heading_open') {
      const tag = Number(t.tag.slice(1)) || 2;
      const cmd = tag <= 2 ? 'subsection' : tag === 3 ? 'subsubsection' : 'paragraph';
      const inline = tokens[i + 1];
      out.push(`\\${cmd}{${inline?.type === 'inline' ? inlineToTex(inline.children ?? [], ctx) : ''}}`);
      i += 3;
    } else if (t.type === 'bullet_list_open' || t.type === 'ordered_list_open') {
      const ordered = t.type === 'ordered_list_open';
      const close = matchClose(tokens, i, t.type, ordered ? 'ordered_list_close' : 'bullet_list_close');
      out.push(listToTex(tokens.slice(i + 1, close), ctx, ordered));
      i = close + 1;
    } else if (t.type === 'blockquote_open') {
      const close = matchClose(tokens, i, 'blockquote_open', 'blockquote_close');
      out.push(['\\begin{quote}', blocksToTex(tokens.slice(i + 1, close), ctx), '\\end{quote}'].join('\n'));
      i = close + 1;
    } else if (t.type === 'table_open') {
      const close = matchClose(tokens, i, 'table_open', 'table_close');
      out.push(tableToTex(tokens.slice(i, close + 1), ctx));
      i = close + 1;
    } else if (t.type === 'fence' || t.type === 'code_block') {
      const inner = t.content.replace(/\n+$/, '').replace(CITE_TOKEN_RE, () => { ctx.counter.i++; return ''; });
      // verbatim keeps the code exactly; nothing inside it is escaped or expanded
      out.push(['\\begin{verbatim}', inner, '\\end{verbatim}'].join('\n'));
      i += 1;
    } else if (t.type === 'hr') {
      // markdown's thematic break — the docx walk drops these; LaTeX can keep it
      out.push('\\begin{center}\\rule{0.5\\linewidth}{0.4pt}\\end{center}');
      i += 1;
    } else {
      i += 1;
    }
  }
  return out.join('\n\n');
}

/** Parse a section body and return its LaTeX. */
export function sectionBodyToTex(body: string, ctx: LatexCtx): string {
  return blocksToTex(mdit.parse(body, {}), ctx);
}
