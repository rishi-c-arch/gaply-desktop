// Gaply — Research Paper Writer, Set A: .docx export (the spike's proven path).
//
// docx is loaded via a plain dynamic import — docx@7.8.2 ships a CRA-native `.js`
// CJS main (build/index.js, no exports/module gate), so webpack code-splits this
// module (+ docx, ~86 KB gz) into an ASYNC chunk loaded ONLY when the user
// exports. Nothing here runs at page load.
//
// IMAGES: a pasted image lives in a section body as `![](gaply-image://<hash>)`.
// docx's ImageRun needs the raw image BYTES + dimensions — never a ref or a blob
// URL (passing those produces no media/relationship → Word reports "corrupted").
// So on export we resolve each ref to bytes from $APPDATA/note-images/ and embed
// via ImageRun. If bytes can't be resolved, we emit an HONEST "[Figure: …]"
// placeholder — a clean docx without the image beats a corrupt one.
//
// HONESTY: this produces submission-ready STRUCTURE (title page, styled headings,
// double-spacing, page numbers, line numbers, embedded figures) — NOT camera-ready
// layout. The References section is a manual placeholder in Set A (CSL is Set B).
import { Manuscript } from './manuscriptModel';
import { IMAGE_REF_PREFIX, readImageBytes } from './noteImages';

/** Raw bytes + intrinsic pixel size of a resolved image, ready for ImageRun. */
export interface ResolvedImage { data: Uint8Array; width: number; height: number; }
export type ImageResolver = (ref: string) => Promise<ResolvedImage | null>;

const IMG_RE = /!\[[^\]]*\]\((gaply-image:\/\/[A-Za-z0-9]+\.[A-Za-z0-9]+)\)/g;
const MAX_IMG_W = 600; // px — fits within the 6.5" content column at ~96 dpi

type Segment = { text: string } | { ref: string };

/** Split a section body into ordered text/image segments (preserving position),
 *  so inline or standalone images both export correctly. Exported for the pins. */
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

/** Scale an image to fit the content column, preserving aspect ratio. */
const fit = (w: number, h: number): { width: number; height: number } => {
  if (!(w > 0) || !(h > 0)) return { width: MAX_IMG_W, height: Math.round(MAX_IMG_W * 0.75) };
  if (w <= MAX_IMG_W) return { width: Math.round(w), height: Math.round(h) };
  return { width: MAX_IMG_W, height: Math.round((h * MAX_IMG_W) / w) };
};

/** Default resolver: read bytes from $APPDATA and measure dimensions via
 *  createImageBitmap (available in the webview). Returns null on any failure so
 *  the caller falls back to an honest placeholder rather than a broken file. */
const defaultImageResolver: ImageResolver = async (ref) => {
  try {
    const { data, mime } = await readImageBytes(ref);
    if (!data.length) return null;
    let width = MAX_IMG_W;
    let height = Math.round(MAX_IMG_W * 0.75);
    try {
      const bmp = await createImageBitmap(new Blob([data], { type: mime }));
      width = bmp.width; height = bmp.height;
      bmp.close?.();
    } catch { /* keep fallback dims — distorted beats absent */ }
    return { data, width, height };
  } catch {
    return null;
  }
};

/** Build the .docx as a Blob. Empty sections are skipped honestly; images are
 *  embedded from real bytes (or an honest placeholder if unresolvable). */
export async function buildManuscriptDocx(m: Manuscript, resolveImage: ImageResolver = defaultImageResolver): Promise<Blob> {
  const {
    Document, Packer, Paragraph, TextRun, HeadingLevel, AlignmentType, ImageRun,
    PageNumber, Footer, PageBreak, LineNumberRestartFormat, LineRuleType, convertInchesToTwip,
  } = await import('docx');

  const title = m.title.trim() || 'Untitled manuscript';
  const authors = m.authors.trim();
  const dbl = { line: 480, lineRule: LineRuleType.AUTO }; // 480 twips = double

  // Title page: centered title + authors, then a page break.
  const titlePage = [
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { before: 2400, after: 400 }, children: [new TextRun({ text: title, bold: true, size: 32 })] }),
    ...(authors ? [new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 200 }, children: [new TextRun({ text: authors, size: 24 })] })] : []),
    new Paragraph({ children: [new PageBreak()] }),
  ];

  // Each section → its content as paragraphs + embedded images. A section with
  // NO content (no text, no image) is skipped entirely (no empty heading).
  const bodyChildren: import('docx').Paragraph[] = [];
  for (const s of m.sections) {
    const kids: import('docx').Paragraph[] = [];
    for (const seg of segmentSectionBody(s.body)) {
      if ('text' in seg) {
        for (const p of seg.text.split(/\n{2,}/).map((x) => x.trim()).filter(Boolean)) {
          kids.push(new Paragraph({ spacing: dbl, children: [new TextRun(p)] }));
        }
      } else {
        let img: ResolvedImage | null = null;
        try { img = await resolveImage(seg.ref); } catch { img = null; }
        if (img && img.data.length) {
          const dim = fit(img.width, img.height);
          kids.push(new Paragraph({ alignment: AlignmentType.CENTER, children: [new ImageRun({ data: img.data, transformation: dim })] }));
        } else {
          // Honest placeholder — never a dangling image relationship (= corrupt).
          const name = seg.ref.slice(IMAGE_REF_PREFIX.length);
          kids.push(new Paragraph({ spacing: dbl, children: [new TextRun({ text: `[Figure: ${name}]`, italics: true })] }));
        }
      }
    }
    if (kids.length === 0) continue; // honest skip
    bodyChildren.push(new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun(s.heading)] }), ...kids);
  }

  const doc = new Document({
    creator: 'Gaply',
    title,
    styles: { default: { document: { run: { font: 'Times New Roman', size: 24 } } } }, // 12pt serif
    sections: [{
      properties: {
        page: {
          size: { width: convertInchesToTwip(8.5), height: convertInchesToTwip(11) },
          margin: { top: convertInchesToTwip(1), right: convertInchesToTwip(1), bottom: convertInchesToTwip(1), left: convertInchesToTwip(1) },
        },
        lineNumbers: { countBy: 1, restart: LineNumberRestartFormat.CONTINUOUS },
      },
      footers: { default: new Footer({ children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ children: [PageNumber.CURRENT] })] })] }) },
      children: [...titlePage, ...bodyChildren],
    }],
  });

  return Packer.toBlob(doc);
}

/** Save a manuscript as .docx via the native dialog + BINARY writeFile (docx is
 *  binary — not writeTextFile). Browser fallback: a Blob download. Returns the
 *  saved path (Tauri) or null (browser / cancelled). */
export async function saveManuscriptDocx(m: Manuscript, suggestedName: string): Promise<string | null> {
  const blob = await buildManuscriptDocx(m);
  const { isTauri } = await import('../../utils/isTauri');
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: suggestedName, filters: [{ name: 'Word', extensions: ['docx'] }] });
    if (!path) return null; // user cancelled — honest no-op
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
