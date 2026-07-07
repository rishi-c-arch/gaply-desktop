// Gaply — minimal, dependency-free PDF writer. Builds a valid multi-page PDF
// from lines of text using only string concatenation — NO prototype mutation,
// so it is compatible with the app's `Object.freeze(Object.prototype)` security
// hardening (which jsPDF violates). Fully offline; produces a real
// application/pdf Blob. Helvetica text only, which is all a findings report
// needs.

export interface PdfLine {
  text: string;
  size: number;
  /** 0 = black … 1 = white. Default 0. */
  gray?: number;
}

const PAGE_W = 595; // A4 @ 72dpi
const PAGE_H = 842;
const MARGIN = 48;

/** Transliterate smart punctuation and drop other non-ASCII, so every char is
 *  one byte — keeping PDF xref offsets simple and the output free of mojibake. */
function toAscii(s: string): string {
  return s
    .replace(/[‘’′]/g, "'")
    .replace(/[“”″]/g, '"')
    .replace(/[–—]/g, '-')
    .replace(/…/g, '...')
    .replace(/[•●]/g, '*')
    .replace(/ /g, ' ')
    .replace(/[^\x20-\x7E]/g, '');
}

/** Escape a PDF literal string. */
function escapePdf(s: string): string {
  return s.replace(/\\/g, '\\\\').replace(/\(/g, '\\(').replace(/\)/g, '\\)');
}

/** Greedy word-wrap using an average Helvetica glyph width (~0.5em). */
function wrap(text: string, size: number): string[] {
  const maxWidth = PAGE_W - MARGIN * 2;
  const charW = size * 0.5;
  const maxChars = Math.max(8, Math.floor(maxWidth / charW));
  const words = text.split(/\s+/);
  const out: string[] = [];
  let cur = '';
  for (const w of words) {
    if (cur.length === 0) cur = w;
    else if ((cur + ' ' + w).length <= maxChars) cur += ' ' + w;
    else {
      out.push(cur);
      cur = w;
    }
  }
  if (cur) out.push(cur);
  return out.length ? out : [''];
}

/** Render text lines to a PDF Blob. */
export function renderTextPdf(lines: PdfLine[]): Blob {
  // 1) wrap + paginate ------------------------------------------------------
  type Placed = { text: string; size: number; gray: number; y: number };
  const pages: Placed[][] = [];
  let page: Placed[] = [];
  let y = PAGE_H - MARGIN;

  for (const ln of lines) {
    const size = ln.size;
    const gray = ln.gray ?? 0;
    for (const vis of wrap(toAscii(ln.text), size)) {
      if (y < MARGIN + size) {
        pages.push(page);
        page = [];
        y = PAGE_H - MARGIN;
      }
      page.push({ text: vis, size, gray, y });
      y -= size + 4;
    }
  }
  pages.push(page);

  // 2) content streams ------------------------------------------------------
  const contents = pages.map((placed) =>
    placed
      .map(
        (p) =>
          `BT /F1 ${p.size} Tf ${p.gray} g ${MARGIN} ${p.y.toFixed(1)} Td (${escapePdf(p.text)}) Tj ET`
      )
      .join('\n')
  );

  // 3) object numbering: 1 Catalog, 2 Pages, 3 Font, then (content,page) pairs
  const pageObjNums: number[] = [];
  let next = 4;
  const objs: string[] = [];
  const pageObjBodies: string[] = [];
  const contentObjBodies: string[] = [];
  for (let i = 0; i < pages.length; i++) {
    const contentNum = next++;
    const pageNum = next++;
    pageObjNums.push(pageNum);
    contentObjBodies.push(
      `${contentNum} 0 obj\n<< /Length ${contents[i].length} >>\nstream\n${contents[i]}\nendstream\nendobj\n`
    );
    pageObjBodies.push(
      `${pageNum} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${PAGE_W} ${PAGE_H}] ` +
        `/Resources << /Font << /F1 3 0 R >> >> /Contents ${contentNum} 0 R >>\nendobj\n`
    );
  }

  objs.push(`1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n`);
  objs.push(
    `2 0 obj\n<< /Type /Pages /Kids [${pageObjNums.map((n) => `${n} 0 R`).join(' ')}] /Count ${pages.length} >>\nendobj\n`
  );
  objs.push(`3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n`);
  for (let i = 0; i < pages.length; i++) {
    objs.push(contentObjBodies[i]);
    objs.push(pageObjBodies[i]);
  }

  // 4) assemble with a byte-accurate xref table -----------------------------
  const header = '%PDF-1.4\n';
  let body = header;
  const offsets: number[] = []; // offsets[objNum-1]
  // objects are already in numeric order (1,2,3, then 4,5,6,...) by construction
  for (const o of objs) {
    offsets.push(body.length);
    body += o;
  }
  const xrefStart = body.length;
  const count = objs.length + 1; // +1 for the free object 0
  let xref = `xref\n0 ${count}\n0000000000 65535 f \n`;
  for (const off of offsets) {
    xref += `${String(off).padStart(10, '0')} 00000 n \n`;
  }
  const trailer = `trailer\n<< /Size ${count} /Root 1 0 R >>\nstartxref\n${xrefStart}\n%%EOF`;

  return new Blob([body + xref + trailer], { type: 'application/pdf' });
}
