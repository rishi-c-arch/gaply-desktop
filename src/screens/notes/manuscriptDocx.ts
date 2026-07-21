// Gaply — Research Paper Writer: .docx export.
//
// docx@7.8.2 loads via a plain dynamic import (CRA-native .js CJS main), so
// webpack code-splits this module (+ docx, ~86 KB gz) into an ASYNC chunk loaded
// ONLY on export.
//
// FORMATTING (Set C+): each scaffold carries a DocxFormat profile derived from
// that venue's PUBLIC manuscript-submission guidance (line spacing, font,
// margins, page size, line numbers, section numbering, title block). We produce
// a clean, professional SINGLE-COLUMN submission manuscript — NOT camera-ready /
// two-column typeset layout (the publisher does that; the UI says so).
//
// IMAGES: a section body's `![](gaply-image://<hash>)` refs are resolved to real
// bytes + dimensions and embedded via ImageRun (never a ref/blob URL, which Word
// flags as "corrupted"); an unresolvable image degrades to an honest placeholder.
import { Manuscript, DocxFormat, DEFAULT_DOCX_FORMAT } from './manuscriptModel';
import { IMAGE_REF_PREFIX, readImageBytes } from './noteImages';
import { CITE_TOKEN_RE } from './GaplyCiteNode';
import { CitationRender } from './manuscriptCitations';

export interface ResolvedImage { data: Uint8Array; width: number; height: number; }
export type ImageResolver = (ref: string) => Promise<ResolvedImage | null>;

const IMG_RE = /!\[[^\]]*\]\((gaply-image:\/\/[A-Za-z0-9]+\.[A-Za-z0-9]+)\)/g;
const MAX_IMG_W = 600; // px — fits within the content column

type Segment = { text: string } | { ref: string };

export function segmentSectionBody(body: string): Segment[] {
  const segs: Segment[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  IMG_RE.lastIndex = 0;
  while ((m = IMG_RE.exec(body)) !== null) {
    if (m.index > last) segs.push({ text: body.slice(last, m.index) });
    segs.push({ ref: m[1] });
    last = m.index + m[0].length;
  }
  if (last < body.length) segs.push({ text: body.slice(last) });
  return segs;
}

const fit = (w: number, h: number): { width: number; height: number } => {
  if (!(w > 0) || !(h > 0)) return { width: MAX_IMG_W, height: Math.round(MAX_IMG_W * 0.75) };
  if (w <= MAX_IMG_W) return { width: Math.round(w), height: Math.round(h) };
  return { width: MAX_IMG_W, height: Math.round((h * MAX_IMG_W) / w) };
};

const defaultImageResolver: ImageResolver = async (ref) => {
  try {
    const { data, mime } = await readImageBytes(ref);
    if (!data.length) return null;
    let width = MAX_IMG_W;
    let height = Math.round(MAX_IMG_W * 0.75);
    try {
      const bmp = await createImageBitmap(new Blob([data], { type: mime }));
      width = bmp.width; height = bmp.height; bmp.close?.();
    } catch { /* keep fallback dims */ }
    return { data, width, height };
  } catch {
    return null;
  }
};

// Line spacing → twips (240 = single line at 12pt). Applied to body paragraphs.
const spacingTwips = (s: DocxFormat['lineSpacing']): number => (s === 'double' ? 480 : s === '1.5' ? 360 : 240);

// Sections that are NOT numbered even when a scaffold uses section numbering
// (front/back matter). Everything else is numbered in order.
const UNNUMBERED_KEYS = new Set(['abstract', 'keywords', 'index_terms', 'ccs_concepts', 'author_summary', 'references']);
const ROMAN = ['I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII', 'IX', 'X', 'XI', 'XII', 'XIII', 'XIV', 'XV'];

/** Build the .docx as a Blob, applying the venue's manuscript format profile. */
export async function buildManuscriptDocx(
  m: Manuscript,
  format: DocxFormat = DEFAULT_DOCX_FORMAT,
  resolveImage: ImageResolver = defaultImageResolver,
  citations?: CitationRender, // resolve-on-export (B2): tokens → markers + auto-References
): Promise<Blob> {
  const {
    Document, Packer, Paragraph, TextRun, HeadingLevel, AlignmentType, ImageRun,
    PageNumber, Footer, PageBreak, LineNumberRestartFormat, LineRuleType,
    convertInchesToTwip, convertMillimetersToTwip,
  } = await import('docx');

  const half = format.fontSizePt * 2;      // docx sizes are half-points
  const line = spacingTwips(format.lineSpacing);
  // Every body paragraph: identical line spacing, no space-after, a first-line
  // indent — so double-spaced paragraphs read as distinct blocks (clean + consistent).
  const bodySpacing = { line, lineRule: LineRuleType.AUTO, after: 0 };
  const bodyIndent = { firstLine: convertInchesToTwip(0.5) };
  const title = m.title.trim() || 'Untitled manuscript';

  /* ---- Title block (centered), consistent spacing, per the profile ---- */
  const titleBlock = [
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { before: 480, after: 200 }, children: [new TextRun({ text: title, bold: true, size: (format.fontSizePt + 6) * 2 })] }),
  ];
  if (m.authors.trim()) titleBlock.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 80 }, children: [new TextRun({ text: m.authors.trim(), size: half })] }));
  if (format.titleBlock.affiliations && m.affiliations.trim()) titleBlock.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 80 }, children: [new TextRun({ text: m.affiliations.trim(), italics: true, size: (format.fontSizePt - 1) * 2 })] }));
  if (format.titleBlock.correspondingAuthor && m.correspondingAuthor.trim()) titleBlock.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 80 }, children: [new TextRun({ text: `Corresponding author: ${m.correspondingAuthor.trim()}`, size: (format.fontSizePt - 1) * 2 })] }));
  titleBlock.push(new Paragraph({ children: [new PageBreak()] }));

  /* ---- Body: each non-empty section → numbered heading + paragraphs/images ---- */
  const heading = (text: string) => new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun(text)] });
  const numberFor = (key: string, n: number): { label: string; upper: boolean } => {
    if (format.sectionNumbering === 'roman-upper') return { label: `${ROMAN[n] ?? String(n + 1)}. `, upper: true };
    if (format.sectionNumbering === 'decimal') return { label: `${n + 1}. `, upper: false };
    return { label: '', upper: false };
  };

  // Replace each [[cite:id]] token, IN DOCUMENT ORDER, with its resolved in-text
  // marker (a dangling token → '[?]'). The counter advances across sections in
  // the SAME order orderedRefIds walked them — so export markers == live markers.
  let tokenIdx = 0;
  const cited = !!citations?.hasCitations;
  const replaceCites = (text: string): string =>
    text.replace(CITE_TOKEN_RE, () => (cited ? (citations!.perToken[tokenIdx++] ?? '[?]') : (tokenIdx++, '')));

  // Bibliography paragraphs (hanging indent) for the auto-References section.
  const bibParagraphs = (): import('docx').Paragraph[] =>
    citations!.bibliography.split('\n').map((l) => l.trim()).filter(Boolean).map((line) =>
      new Paragraph({ spacing: bodySpacing, indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) }, children: [new TextRun(line)] }));

  const bodyChildren: import('docx').Paragraph[] = [];
  let numbered = 0;
  let refsEmitted = false;
  for (const s of m.sections) {
    // The References section becomes the AUTO-GENERATED bibliography when the
    // manuscript has citations; otherwise its manual body is exported as-is.
    if (s.key === 'references' && cited) {
      const paras = bibParagraphs();
      if (paras.length) { bodyChildren.push(heading(s.heading), ...paras); refsEmitted = true; }
      continue;
    }
    const kids: import('docx').Paragraph[] = [];
    for (const seg of segmentSectionBody(s.body)) {
      if ('text' in seg) {
        for (const p of seg.text.split(/\n{2,}/).map((x) => x.trim()).filter(Boolean)) {
          kids.push(new Paragraph({ spacing: bodySpacing, indent: bodyIndent, children: [new TextRun(replaceCites(p))] }));
        }
      } else {
        let img: ResolvedImage | null = null;
        try { img = await resolveImage(seg.ref); } catch { img = null; }
        if (img && img.data.length) {
          kids.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { before: 120, after: 120 }, children: [new ImageRun({ data: img.data, transformation: fit(img.width, img.height) })] }));
        } else {
          kids.push(new Paragraph({ spacing: bodySpacing, indent: bodyIndent, children: [new TextRun({ text: `[Figure: ${seg.ref.slice(IMAGE_REF_PREFIX.length)}]`, italics: true })] }));
        }
      }
    }
    if (kids.length === 0) continue; // honest skip — no empty heading
    const numberable = !UNNUMBERED_KEYS.has(s.key) && format.sectionNumbering !== 'none';
    const { label, upper } = numberable ? numberFor(s.key, numbered) : { label: '', upper: false };
    if (numberable) numbered += 1;
    bodyChildren.push(heading(`${label}${upper ? s.heading.toUpperCase() : s.heading}`), ...kids);
  }
  // Citations but the References section was deleted → append one so the
  // bibliography is never lost.
  if (cited && !refsEmitted) {
    const paras = bibParagraphs();
    if (paras.length) bodyChildren.push(heading('References'), ...paras);
  }

  /* ---- Page setup + styles from the profile ---- */
  const pg = format.pageSize === 'a4'
    ? { width: convertMillimetersToTwip(210), height: convertMillimetersToTwip(297) }
    : { width: convertInchesToTwip(8.5), height: convertInchesToTwip(11) };
  const mar = convertInchesToTwip(format.marginInch);
  const headingRun = { font: format.font, size: (format.fontSizePt + 2) * 2, bold: true, color: '000000' };

  const doc = new Document({
    creator: 'Gaply',
    title,
    styles: {
      // Override the BUILT-IN heading styles (what HeadingLevel.HEADING_1 uses)
      // via `default.heading1/2` — a same-named paragraphStyle does NOT override
      // them (Word keeps its blue, unbolded default). Black, bold, venue font,
      // fixed spacing (not tied to line spacing) → consistent gaps.
      default: {
        document: { run: { font: format.font, size: half } }, // body font + size everywhere
        heading1: { run: headingRun, paragraph: { spacing: { before: 240, after: 120 }, keepNext: true } },
        heading2: { run: { ...headingRun, size: (format.fontSizePt + 1) * 2 }, paragraph: { spacing: { before: 180, after: 80 }, keepNext: true } },
      },
    },
    sections: [{
      properties: {
        page: { size: pg, margin: { top: mar, right: mar, bottom: mar, left: mar } },
        ...(format.lineNumbers ? { lineNumbers: { countBy: 1, restart: LineNumberRestartFormat.CONTINUOUS } } : {}),
      },
      footers: { default: new Footer({ children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ children: [PageNumber.CURRENT] })] })] }) },
      children: [...titleBlock, ...bodyChildren],
    }],
  });

  return Packer.toBlob(doc);
}

/** Save a manuscript as .docx via the native dialog + BINARY writeFile (docx is
 *  binary). Browser fallback: a Blob download. Returns the saved path (Tauri) or
 *  null (browser / cancelled). */
export async function saveManuscriptDocx(m: Manuscript, suggestedName: string, format: DocxFormat = DEFAULT_DOCX_FORMAT, citations?: CitationRender): Promise<string | null> {
  const blob = await buildManuscriptDocx(m, format, undefined, citations);
  const { isTauri } = await import('../../utils/isTauri');
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: suggestedName, filters: [{ name: 'Word', extensions: ['docx'] }] });
    if (!path) return null;
    const { writeFile } = await import('@tauri-apps/plugin-fs');
    await writeFile(path, new Uint8Array(await blob.arrayBuffer()));
    return path;
  }
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url; a.download = suggestedName;
  document.body.appendChild(a); a.click(); a.remove();
  URL.revokeObjectURL(url);
  return null;
}
