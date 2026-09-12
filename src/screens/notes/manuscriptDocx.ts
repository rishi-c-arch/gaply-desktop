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
import { Manuscript, DocxFormat, DEFAULT_DOCX_FORMAT, ReadingFormat } from './manuscriptModel';
import { readImageBytes } from './noteImages';
import { sectionBodyToBlocks, RichDocxCtx, NUMBERING_REF } from './manuscriptRichDocx';
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
  // JOURNAL READING LAYOUT (optional). Absent → byte-for-byte today's
  // single-column SUBMISSION manuscript, which is what the product's four
  // standing honesty claims describe and which must not move. Present → the
  // same content in the venue's reading shape: a full-width title block over a
  // two-column body. One builder, so the two can never drift apart in content.
  reading?: ReadingFormat,
  /** Filled with any formula that fell back to LaTeX source, for the caller to report. */
  mathFallbacks?: Array<{ tex: string; reason: string }>,
): Promise<Blob> {
  const {
    Document, Packer, Paragraph, TextRun, HeadingLevel, AlignmentType, ImageRun,
    ExternalHyperlink,
    PageNumber, Footer, PageBreak, LineNumberRestartFormat, LineRuleType,
    convertInchesToTwip, convertMillimetersToTwip,
    Table, TableRow, TableCell, WidthType, BorderStyle, LevelFormat, SectionType,
    convertToXmlComponent,
  } = await import('docx');
  const { xml2js } = await import('xml-js');

  // THE ONE PLACE THE TWO LAYOUTS DIVERGE. Everything downstream reads `f`, so
  // the reading layout is a different PROFILE of the same document rather than
  // a second code path — content, citation order and the References rule cannot
  // drift between them because there is only one of each.
  //
  // `lineSpacing` is forced to single rather than taken per-venue: double
  // spacing is a REVIEW convention that exists so a copy-editor can write
  // between the lines, and a reading layout is the opposite of that.
  const f: DocxFormat = reading
    ? {
      font: reading.font ?? format.font,
      fontSizePt: reading.fontSizePt,
      marginInch: reading.marginInch,
      pageSize: reading.pageSize,
      lineSpacing: 'single',
      lineNumbers: reading.lineNumbers,
      sectionNumbering: reading.sectionNumbering,
      titleBlock: reading.titleBlock,
    }
    : format;

  const half = f.fontSizePt * 2;      // docx sizes are half-points
  const line = spacingTwips(f.lineSpacing);
  // Every body paragraph: identical line spacing, no space-after, a first-line
  // indent — so double-spaced paragraphs read as distinct blocks (clean + consistent).
  const bodySpacing = { line, lineRule: LineRuleType.AUTO, after: 0 };
  const bodyIndent = { firstLine: convertInchesToTwip(0.5) };
  const title = m.title.trim() || 'Untitled manuscript';

  /* ---- Title block (centered), consistent spacing, per the profile ---- */
  const titleBlock = [
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { before: 480, after: 200 }, children: [new TextRun({ text: title, bold: true, size: (f.fontSizePt + 6) * 2 })] }),
  ];
  if (m.authors.trim()) titleBlock.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 80 }, children: [new TextRun({ text: m.authors.trim(), size: half })] }));
  if (f.titleBlock.affiliations && m.affiliations.trim()) titleBlock.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 80 }, children: [new TextRun({ text: m.affiliations.trim(), italics: true, size: (f.fontSizePt - 1) * 2 })] }));
  if (f.titleBlock.correspondingAuthor && m.correspondingAuthor.trim()) titleBlock.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 80 }, children: [new TextRun({ text: `Corresponding author: ${m.correspondingAuthor.trim()}`, size: (f.fontSizePt - 1) * 2 })] }));
  // The SUBMISSION manuscript puts the body on a fresh page after the title
  // block. The reading layout does not: the title block is a full-width first
  // section and the two-column body continues on the SAME page, which is what
  // a published article looks like. A page break here would leave the title
  // alone on page one and defeat the point.
  if (!reading) titleBlock.push(new Paragraph({ children: [new PageBreak()] }));

  /* ---- Body: each non-empty section → numbered heading + paragraphs/images ---- */
  const heading = (text: string) => new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun(text)] });
  const numberFor = (key: string, n: number): { label: string; upper: boolean } => {
    if (f.sectionNumbering === 'roman-upper') return { label: `${ROMAN[n] ?? String(n + 1)}. `, upper: true };
    if (f.sectionNumbering === 'decimal') return { label: `${n + 1}. `, upper: false };
    return { label: '', upper: false };
  };

  // RAW OMML MUST GO THROUGH convertToXmlComponent, NOT
  // ImportedXmlComponent.fromXmlString. In docx 7.8.2 fromXmlString returns a
  // component whose rootKey is `undefined`, so the packer writes the math inside
  // a literal <undefined> element:
  //
  //     <w:p><undefined><m:oMath …>…</m:oMath></undefined></w:p>
  //
  // That is WELL-FORMED XML and invalid OOXML, so xmllint passes it, every
  // `xml.includes('m:oMath')` assertion passes it, and Word refuses the whole
  // file with "Word experienced an error trying to open the file". Parsing the
  // string first and converting the element gives a component with the right
  // rootKey. Verified by opening the result in Word, which is the only check
  // that could have caught it.
  const ommlComponent = (omml: string) =>
    convertToXmlComponent(
      xml2js(omml, { compact: false, captureSpacesBetweenElements: true }).elements![0] as never,
    ) as never;

  const cited = !!citations?.hasCitations;
  // The shared, document-order citation counter — advances across every section's
  // rich content (inside bold/italic/list/table too) in the SAME order
  // orderedRefIds walked, so perToken[k] alignment (and the B2 match) is preserved.
  const richCtx: RichDocxCtx = {
    D: { Paragraph, TextRun, HeadingLevel, AlignmentType, ImageRun, ExternalHyperlink, Table, TableRow, TableCell, WidthType, BorderStyle } as unknown as typeof import('docx'),
    bodySpacing, bodyIndent, citations, counter: { i: 0 }, resolveImage, fit, olInstance: { n: 0 }, mathFallbacks,
    ommlComponent,
  };

  // Bibliography paragraphs (hanging indent) for the auto-References section.
  const bibParagraphs = (): import('docx').Paragraph[] =>
    citations!.bibliography.split('\n').map((l) => l.trim()).filter(Boolean).map((line) =>
      new Paragraph({ spacing: bodySpacing, indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) }, children: [new TextRun(line)] }));

  const bodyChildren: (import('docx').Paragraph | import('docx').Table)[] = [];
  let numbered = 0;
  let refsEmitted = false;
  for (const s of m.sections) {
    // The References section becomes the AUTO-GENERATED bibliography when the
    // manuscript has citations AND that bibliography is non-empty; otherwise its
    // manual body is exported as-is.
    //
    // THE EMPTY CASE IS NOT HYPOTHETICAL. `hasCitations` only means "[[cite:…]]
    // tokens exist" — if every one of them is dangling (the refs were deleted
    // from the library), renderCitations drops them all and the bibliography is
    // ''. Skipping the section then discarded BOTH the auto list and whatever
    // the author had typed by hand, so a submission manuscript exported with no
    // bibliography at all and nothing said so. Fall through instead: the manual
    // body is the only reference text that still exists, so it must ship.
    if (s.key === 'references' && cited) {
      const paras = bibParagraphs();
      if (paras.length) {
        bodyChildren.push(heading(s.heading), ...paras);
        refsEmitted = true;
        continue;
      }
      // empty bibliography → fall through to the manual-body path below
    }
    // Parse the section markdown → real docx blocks (paragraphs, lists, tables,
    // images), with citations resolved inline in document order.
    const kids = await sectionBodyToBlocks(s.body, richCtx);
    if (kids.length === 0) continue; // honest skip — no empty heading
    const numberable = !UNNUMBERED_KEYS.has(s.key) && f.sectionNumbering !== 'none';
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
  const pg = f.pageSize === 'a4'
    ? { width: convertMillimetersToTwip(210), height: convertMillimetersToTwip(297) }
    : { width: convertInchesToTwip(8.5), height: convertInchesToTwip(11) };
  const mar = convertInchesToTwip(f.marginInch);
  const headingRun = { font: f.font, size: (f.fontSizePt + 2) * 2, bold: true, color: '000000' };

  const lineNums = f.lineNumbers
    ? { lineNumbers: { countBy: 1, restart: LineNumberRestartFormat.CONTINUOUS } }
    : {};
  const pageProps = { size: pg, margin: { top: mar, right: mar, bottom: mar, left: mar } };
  const pageProps2 = { page: pageProps, ...lineNums };
  const footer = () => new Footer({
    children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ children: [PageNumber.CURRENT] })] })],
  });

  const doc = new Document({
    creator: 'Gaply',
    title,
    styles: {
      // Override the BUILT-IN heading styles (what HeadingLevel.HEADING_1 uses)
      // via `default.heading1/2` — a same-named paragraphStyle does NOT override
      // them (Word keeps its blue, unbolded default). Black, bold, venue font,
      // fixed spacing (not tied to line spacing) → consistent gaps.
      default: {
        document: { run: { font: f.font, size: half } }, // body font + size everywhere
        heading1: { run: headingRun, paragraph: { spacing: { before: 240, after: 120 }, keepNext: true } },
        heading2: { run: { ...headingRun, size: (f.fontSizePt + 1) * 2 }, paragraph: { spacing: { before: 180, after: 80 }, keepNext: true } },
        heading3: { run: { ...headingRun, size: f.fontSizePt * 2, italics: true }, paragraph: { spacing: { before: 140, after: 60 }, keepNext: true } },
      },
    },
    // Numbering for ordered lists (per-list restart via a fresh `instance`).
    numbering: {
      config: [{
        reference: NUMBERING_REF,
        levels: [0, 1, 2, 3].map((lvl) => ({
          level: lvl, format: LevelFormat.DECIMAL, text: `%${lvl + 1}.`, alignment: AlignmentType.START,
          style: { paragraph: { indent: { left: convertInchesToTwip(0.25 + lvl * 0.25), hanging: convertInchesToTwip(0.25) } } },
        })),
      }],
    },
    sections: reading
      // TWO CONTINUOUS SECTIONS. Column count is a SECTION property in OOXML,
      // so a full-width title over a two-column body is not one section with a
      // special first paragraph — it is two sections joined by a continuous
      // break, which is why this shape and not a simpler one.
      ? [
        {
          properties: { type: SectionType.CONTINUOUS, page: pageProps, ...lineNums },
          footers: { default: footer() },
          children: titleBlock,
        },
        {
          properties: {
            type: SectionType.CONTINUOUS,
            page: pageProps,
            ...lineNums,
            ...(reading.columns > 1
              ? { column: { count: reading.columns, space: convertInchesToTwip(reading.columnGapInch ?? 0.25), separate: false } }
              : {}),
          },
          footers: { default: footer() },
          children: bodyChildren,
        },
      ]
      : [{
        properties: pageProps2,
        footers: { default: footer() },
        children: [...titleBlock, ...bodyChildren],
      }],
  });

  return Packer.toBlob(doc);
}

/** Save a manuscript as .docx via the native dialog + BINARY writeFile (docx is
 *  binary). Browser fallback: a Blob download. Returns the saved path (Tauri) or
 *  null (browser / cancelled). */
export async function saveManuscriptDocx(
  m: Manuscript,
  suggestedName: string,
  format: DocxFormat = DEFAULT_DOCX_FORMAT,
  citations?: CitationRender,
  /** Present → the journal reading layout; absent → the submission manuscript. */
  reading?: ReadingFormat,
  mathFallbacks?: Array<{ tex: string; reason: string }>,
): Promise<string | null> {
  const blob = await buildManuscriptDocx(m, format, undefined, citations, reading, mathFallbacks);
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
