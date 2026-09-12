// Gaply — Research Paper Writer, Set 5 (Phase 3a): within-section markdown → docx.
// Parses each section body with markdown-it and emits REAL docx elements — bold/
// italic runs, bullet + numbered lists, GFM tables, sub-headings, blockquotes,
// inline code, and embedded images — instead of literal markdown text.
//
// THE WRINKLE (kept correct): [[cite:id]] tokens live INLINE among bold/italic/
// list/table content. resolveCites replaces each one with its perToken marker
// while advancing a SHARED counter in document order (section order × token
// order) — so the in-text ↔ References matching B2 proved is preserved even
// though tokens are now nested inside formatted runs.
import MarkdownIt from 'markdown-it';
import type Token from 'markdown-it/lib/token';
import { CITE_TOKEN_RE } from './GaplyCiteNode';
import { registerMathRule, mathTokenPayload } from './GaplyMathNode';
import { texToOmml } from './manuscriptOmml';
import { CitationRender } from './manuscriptCitations';
import { IMAGE_REF_PREFIX } from './noteImages';
import type { ResolvedImage, ImageResolver } from './manuscriptDocx';

// `breaks: true` matches the editor's own parser (RichBody configures
// tiptap-markdown the same way), so the two agree about what a newline means.
// It is a declaration of intent, not the mechanism: `breaks` only affects
// markdown-it's HTML RENDERER, and we walk tokens. The line-break decision that
// actually ships is in inlineRuns' softbreak/hardbreak case.
const mdit = new MarkdownIt({ html: false, linkify: false, breaks: true }); // GFM tables on by default
// Math must be claimed here too, or markdown-it's `escape` rule reaches inside a
// formula and eats `\,` before the walk ever sees it. Registering the rule also
// means a raw `[[math:…]]` token can never leak into a Word file as literal text.
registerMathRule(mdit);

export const NUMBERING_REF = 'ms-ol';

type Docx = typeof import('docx');
type DocxBlock = import('docx').Paragraph | import('docx').Table;
type ParaChild = import('docx').ParagraphChild;

export interface RichDocxCtx {
  D: Docx;
  bodySpacing: import('docx').IParagraphOptions['spacing'];
  bodyIndent: import('docx').IParagraphOptions['indent'];
  citations?: CitationRender;
  counter: { i: number };          // shared document-order token counter
  resolveImage: ImageResolver;
  fit: (w: number, h: number) => { width: number; height: number };
  olInstance: { n: number };        // fresh numbering instance per ordered list (restart at 1)
  /** Formulas that could not be mapped to OMML and shipped as LaTeX source.
   *  Collected so the caller can report the limit rather than have it
   *  discovered in a reviewer's copy. */
  mathFallbacks?: Array<{ tex: string; reason: string }>;
  /** Wrap a raw `<m:oMath>` string as a docx component. Supplied by the caller
   *  because it needs docx's lazily-imported module; see manuscriptDocx. */
  ommlComponent: (omml: string) => ParaChild;
}

/** Replace [[cite:id]] with its resolved marker, advancing the shared counter in
 *  document order (identical order to orderedRefIds → perToken alignment holds). */
const resolveCites = (text: string, ctx: RichDocxCtx): string =>
  text.replace(CITE_TOKEN_RE, () => (ctx.citations?.hasCitations ? (ctx.citations.perToken[ctx.counter.i++] ?? '[?]') : (ctx.counter.i++, '')));

const half = (pt: number) => pt; // sizes already handled by the default run style

/** Find the matching close token index for a nested open/close pair. */
const matchClose = (tokens: Token[], open: number, openType: string, closeType: string): number => {
  let depth = 0;
  for (let j = open; j < tokens.length; j++) {
    if (tokens[j].type === openType) depth += 1;
    else if (tokens[j].type === closeType) { depth -= 1; if (depth === 0) return j; }
  }
  return tokens.length - 1;
};

/** Inline token children → docx runs (bold/italic/code + links + inline images). */
async function inlineRuns(children: Token[], ctx: RichDocxCtx): Promise<ParaChild[]> {
  const { D } = ctx;
  const runs: ParaChild[] = [];
  let bold = false;
  let italic = false;

  // Link state. Runs between link_open and link_close are buffered so they can
  // be wrapped in one ExternalHyperlink; `linkText` accumulates the anchor's
  // plain text so we can tell whether the URL is already visible to a reader.
  let linkHref: string | null = null;
  let linkBuf: ParaChild[] = [];
  let linkText = '';
  const emit = (r: ParaChild) => { (linkHref !== null ? linkBuf : runs).push(r); };

  for (const t of children) {
    switch (t.type) {
      case 'strong_open': bold = true; break;
      case 'strong_close': bold = false; break;
      case 'em_open': italic = true; break;
      case 'em_close': italic = false; break;
      case 'text': {
        const text = resolveCites(t.content, ctx);
        if (linkHref !== null) linkText += text;
        if (text) emit(new D.TextRun({ text, bold, italics: italic }));
        break;
      }
      case 'link_open':
        linkHref = t.attrGet('href') ?? '';
        linkBuf = [];
        linkText = '';
        break;
      case 'link_close': {
        // The URL used to be dropped here entirely — `[text](url)` exported as
        // bare "text" and the address was gone from the document. Now the anchor
        // becomes a real hyperlink, AND the bare URL is printed when the anchor
        // text isn't already the address: a manuscript is reviewed on paper and
        // in PDF as often as on screen, where a link you can't hover is a link
        // you can't follow.
        const href = linkHref ?? '';
        const children_ = linkBuf.length ? linkBuf : [new D.TextRun({ text: href, bold, italics: italic })];
        const shown = linkText || href;
        linkHref = null; // emit() targets the paragraph again from here
        linkBuf = [];
        if (href) emit(new D.ExternalHyperlink({ children: children_, link: href }));
        else children_.forEach(emit);
        if (href && !shown.includes(href)) emit(new D.TextRun({ text: ` (${href})`, bold, italics: italic }));
        break;
      }
      case 'code_inline': {
        // verbatim, but still advance the counter for any raw [[cite]] (matches
        // orderedRefIds' raw scan, so alignment never drifts).
        const text = resolveCites(t.content, ctx);
        emit(new D.TextRun({ text, font: 'Courier New', bold, italics: italic }));
        break;
      }
      case 'gaply_math': {
        // REAL OMML, with the LaTeX source as the fallback rather than the rule.
        // texToOmml never throws: it returns null and a reason for anything
        // outside the renderer's subset, so a formula it cannot typeset still
        // reaches the page as its own source in a math font. Losing an author's
        // mathematics is the one outcome that is never acceptable; rendering it
        // less beautifully than Word could is a limit worth taking.
        //
        // `ommlComponent`, NOT ImportedXmlComponent.fromXmlString — see its note.
        const { tex, display } = mathTokenPayload(t);
        const { omml, reason } = texToOmml(tex);
        if (omml) {
          emit(ctx.ommlComponent(omml));
        } else {
          ctx.mathFallbacks?.push({ tex, reason: reason ?? 'unsupported' });
          emit(new D.TextRun({ text: display ? `  ${tex}  ` : tex, font: 'Cambria Math', italics: true }));
        }
        break;
      }
      // BOTH break kinds become a real <w:br/>, because in THIS document model
      // every newline inside a paragraph is one the author pressed Enter for:
      // RichBody runs tiptap-markdown with `breaks: true` and serializes its
      // hardBreak nodes to a bare "\n". Emitting a space instead collapsed
      // stanzas, address blocks and line-per-item keyword lists into one line.
      //
      // Note `breaks` is a RENDERER option in markdown-it — it does not change
      // tokenization, so a single newline still arrives here as `softbreak` and
      // the decision has to be made on this side. (A source-wrapped paragraph
      // would round-trip differently, but the editor never produces one.)
      case 'softbreak':
      case 'hardbreak':
        emit(new D.TextRun({ text: '', break: 1, bold, italics: italic }));
        break;
      case 'image': {
        const src = t.attrGet('src') ?? '';
        let img: ResolvedImage | null = null;
        try { img = await ctx.resolveImage(src); } catch { img = null; }
        if (img && img.data.length) emit(new D.ImageRun({ data: img.data, transformation: ctx.fit(img.width, img.height) }));
        else emit(new D.TextRun({ text: `[Figure: ${src.replace(IMAGE_REF_PREFIX, '')}]`, italics: true }));
        break;
      }
      default: break; // s_open/close etc. → keep inner text (handled by 'text')
    }
  }
  return runs;
}

interface Deco { bullet?: boolean; ordered?: boolean; level?: number; instance?: number; quote?: boolean }

function makeParagraph(children: ParaChild[], ctx: RichDocxCtx, deco: Deco): import('docx').Paragraph {
  const { D } = ctx;
  const opts: Record<string, unknown> = { children };
  if (deco.bullet) { opts.bullet = { level: deco.level ?? 0 }; }
  else if (deco.ordered) { opts.numbering = { reference: NUMBERING_REF, level: deco.level ?? 0, instance: deco.instance }; }
  else if (deco.quote) {
    opts.spacing = ctx.bodySpacing;
    opts.indent = { left: 720 };
    opts.border = { left: { style: D.BorderStyle.SINGLE, size: 12, color: '888888', space: 12 } };
  } else {
    opts.spacing = ctx.bodySpacing;
    opts.indent = ctx.bodyIndent;
  }
  // center a paragraph that is ONLY an image (a figure).
  if (children.length === 1 && children[0] instanceof D.ImageRun) { opts.alignment = D.AlignmentType.CENTER; opts.indent = undefined; }
  return new D.Paragraph(opts);
}

async function buildTable(tokens: Token[], ctx: RichDocxCtx): Promise<import('docx').Table> {
  const { D } = ctx;
  const rows: import('docx').TableRow[] = [];
  let i = 0;
  while (i < tokens.length) {
    if (tokens[i].type === 'tr_open') {
      const trClose = matchClose(tokens, i, 'tr_open', 'tr_close');
      const cells: import('docx').TableCell[] = [];
      let j = i + 1;
      while (j < trClose) {
        if (tokens[j].type === 'th_open' || tokens[j].type === 'td_open') {
          const header = tokens[j].type === 'th_open';
          const cellClose = matchClose(tokens, j, tokens[j].type, header ? 'th_close' : 'td_close');
          const inline = tokens[j + 1];
          const runs = inline?.type === 'inline' ? await inlineRuns(inline.children ?? [], ctx) : [];
          const finalRuns = header ? runs.map((r) => r) : runs; // header bold handled below
          cells.push(new D.TableCell({ children: [new D.Paragraph({ children: header ? [new D.TextRun({ text: (inline?.children ?? []).map((c) => resolveCites(c.content, ctx)).join(''), bold: true })] : finalRuns })] }));
          j = cellClose + 1;
        } else j += 1;
      }
      rows.push(new D.TableRow({ children: cells }));
      i = trClose + 1;
    } else i += 1;
  }
  return new D.Table({ rows, width: { size: 100, type: D.WidthType.PERCENTAGE } });
}

async function listBlocks(tokens: Token[], ctx: RichDocxCtx, opts: { ordered: boolean; level: number; instance?: number }): Promise<DocxBlock[]> {
  const out: DocxBlock[] = [];
  let i = 0;
  while (i < tokens.length) {
    if (tokens[i].type === 'list_item_open') {
      const close = matchClose(tokens, i, 'list_item_open', 'list_item_close');
      const inner = tokens.slice(i + 1, close);
      let j = 0;
      while (j < inner.length) {
        const t = inner[j];
        if (t.type === 'paragraph_open') {
          const inline = inner[j + 1];
          const runs = inline?.type === 'inline' ? await inlineRuns(inline.children ?? [], ctx) : [];
          out.push(makeParagraph(runs, ctx, opts.ordered ? { ordered: true, level: opts.level, instance: opts.instance } : { bullet: true, level: opts.level }));
          j += inline?.type === 'inline' ? 3 : 2;
        } else if (t.type === 'bullet_list_open' || t.type === 'ordered_list_open') {
          const o2 = t.type === 'ordered_list_open';
          const c2 = matchClose(inner, j, t.type, o2 ? 'ordered_list_close' : 'bullet_list_close');
          out.push(...await listBlocks(inner.slice(j + 1, c2), ctx, { ordered: o2, level: opts.level + 1, instance: o2 ? ctx.olInstance.n++ : undefined }));
          j = c2 + 1;
        } else j += 1;
      }
      i = close + 1;
    } else i += 1;
  }
  return out;
}

async function tokensToBlocks(tokens: Token[], ctx: RichDocxCtx, deco: Deco = {}): Promise<DocxBlock[]> {
  const { D } = ctx;
  const out: DocxBlock[] = [];
  let i = 0;
  while (i < tokens.length) {
    const t = tokens[i];
    if (t.type === 'paragraph_open') {
      const inline = tokens[i + 1];
      const runs = inline?.type === 'inline' ? await inlineRuns(inline.children ?? [], ctx) : [];
      if (runs.length) out.push(makeParagraph(runs, ctx, deco));
      i += inline?.type === 'inline' ? 3 : 2;
    } else if (t.type === 'heading_open') {
      const tag = Number(t.tag.slice(1)) || 2;
      // Section keeps H1; a body #/## → H2 (first sub-level), ###+ → H3.
      const lvl = tag <= 2 ? D.HeadingLevel.HEADING_2 : D.HeadingLevel.HEADING_3;
      const inline = tokens[i + 1];
      out.push(new D.Paragraph({ heading: lvl, children: inline?.type === 'inline' ? await inlineRuns(inline.children ?? [], ctx) : [] }));
      i += 3;
    } else if (t.type === 'bullet_list_open' || t.type === 'ordered_list_open') {
      const ordered = t.type === 'ordered_list_open';
      const close = matchClose(tokens, i, t.type, ordered ? 'ordered_list_close' : 'bullet_list_close');
      out.push(...await listBlocks(tokens.slice(i + 1, close), ctx, { ordered, level: 0, instance: ordered ? ctx.olInstance.n++ : undefined }));
      i = close + 1;
    } else if (t.type === 'blockquote_open') {
      const close = matchClose(tokens, i, 'blockquote_open', 'blockquote_close');
      out.push(...await tokensToBlocks(tokens.slice(i + 1, close), ctx, { quote: true }));
      i = close + 1;
    } else if (t.type === 'table_open') {
      const close = matchClose(tokens, i, 'table_open', 'table_close');
      out.push(await buildTable(tokens.slice(i, close + 1), ctx));
      i = close + 1;
    } else if (t.type === 'fence' || t.type === 'code_block') {
      out.push(new D.Paragraph({ spacing: ctx.bodySpacing, children: [new D.TextRun({ text: resolveCites(t.content.replace(/\n+$/, ''), ctx), font: 'Courier New' })] }));
      i += 1;
    } else {
      i += 1; // hr, html_block, etc.
    }
  }
  return out;
}

/** Parse a section body's markdown and return docx blocks (paragraphs/lists/
 *  tables/images) with citations resolved in document order. */
export async function sectionBodyToBlocks(body: string, ctx: RichDocxCtx): Promise<DocxBlock[]> {
  return tokensToBlocks(mdit.parse(body, {}), ctx);
}

void half; // (kept for symmetry with the docx sizing helpers)
