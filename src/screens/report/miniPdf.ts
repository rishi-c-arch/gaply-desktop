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
/** Marks a character this renderer cannot represent. Visible on purpose. */
export const UNREPRESENTABLE = '□';

/** Reduce text to one-byte characters WITHOUT EVER SILENTLY DROPPING ONE.
 *
 *  Every char must be one byte: `new Blob([string])` encodes UTF-8, so a
 *  multi-byte char would shift the xref offsets computed below.
 *
 *  # The invariant
 *
 *  NEVER SILENTLY DELETE. The previous implementation ended with
 *  `.replace(/[^\x20-\x7E]/g, '')`, removing characters BEFORE the PDF was
 *  written — so a viewer never had the chance to report a missing glyph, and
 *  "Muller" (with an umlaut) exported as "Mller", which reads as a name.
 *  ONTOLOGY §4.20's TEXT class: silently altered evidence looks like evidence.
 *
 *  # Two tiers, because the honest answer differs by script
 *
 *  1. LATIN WITH DIACRITICS -> TRANSLITERATED. NFD decomposition drops the
 *     combining marks, so an umlauted "Muller" becomes "Muller" and "Sarma"
 *     with diacritics becomes "Sarma". This is what library catalogues do: the
 *     name stays recognisable, a reader recovers the original, and the meaning
 *     is unchanged.
 *  2. NON-LATIN -> MARKED, NEVER TRANSLITERATED. No ASCII form of Devanagari,
 *     CJK or Arabic preserves meaning for a reader. IAST and ITRANS are
 *     scholarly conventions, not rendering fallbacks, and both need diacritics
 *     of their own. A visible mark states that something was here this renderer
 *     cannot show, which is the honest claim.
 *
 *  # TIER 1 NARROWS AT THE PORT — IT DOES NOT DISAPPEAR. DO NOT DELETE IT.
 *
 *  THIS PARAGRAPH REPLACES A "DO NOT PORT IT" WARNING THAT WAS MEASURABLY
 *  FALSE. Acting on the old wording — deleting the NFD logic when writing the
 *  Rust renderer — ships "?arm?" for a name. Read the measurement below before
 *  changing anything here.
 *
 *  The old claim: Rust writes bytes directly, base-14 fonts carry
 *  WinAnsiEncoding, WinAnsi covers Latin-1, therefore transliteration is
 *  unnecessary. Every step is true except the conclusion's scope. WinAnsi
 *  covers LATIN-1, which is NOT the same as LATIN.
 *
 *  MEASURED: WinAnsiEncoding contains exactly SEVEN characters from Latin
 *  Extended-A — Œ œ Š š Ÿ Ž ž. Everything else in that block is absent, which
 *  covers most Polish, Czech, Turkish, Hungarian, Romanian and romanized
 *  Sanskrit letters:
 *
 *      Müller, García, François, Žilina   -> intact, byte-exact
 *      Śarmā                              -> "?arm?"    without tier 1
 *      Łukasz                             -> "?ukasz"   without tier 1
 *      Dvořák                             -> "Dvo?ák"   without tier 1
 *      Öztürk Şahin                       -> "Öztürk ?ahin"
 *      Ştefănescu                         -> "?tef?nescu"
 *
 *  SO THE PORT NARROWS TIER 1'S SCOPE RATHER THAN REMOVING IT:
 *
 *      here (UTF-8 byte path)  tier 1 applies to EVERYTHING non-ASCII
 *      Rust (WinAnsi bytes)    tier 1 applies to LATIN BEYOND LATIN-1
 *
 *  Rust can WRITE Latin-1 bytes, so "Müller" stops being transliterated and
 *  renders exactly. Base-14 still cannot REPRESENT Latin Extended-A, so "Śarmā"
 *  must still fold to "Sarma". Two different limits; only the first one lifts.
 *
 *  Why fold rather than mark: "?ukasz" is unusable while "Lukasz" is wrong but
 *  readable, and for a NAME recognisability is the axis that matters to its
 *  owner. Latin-1 stays byte-exact and untouched; only what base-14 cannot
 *  represent at all is folded, and only to its own base letter.
 *
 *  Tier 2 (non-Latin MARKED) survives unchanged until a font is embedded.
 *  ARCHITECTURE_TRACE §31.17.
 *
 *  Written here as well as in the trace because whoever deletes this file is not
 *  necessarily whoever reads §31 — and a workaround copied past the thing it
 *  worked around becomes "how Gaply handles names".
 */
export function toAscii(s: string): string {
  const folded = s
    .replace(/[‘’′]/g, "'")
    .replace(/[“”″]/g, '"')
    .replace(/[–—]/g, '-')
    .replace(/…/g, '...')
    .replace(/[•●]/g, '*')
    .replace(/ /g, ' ')
    // TIER 1 — strip the combining marks NFD leaves behind, so an accented
    // Latin letter becomes its base letter rather than vanishing.
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '');
  // TIER 2 — anything still outside printable ASCII is MARKED, not removed.
  let out = '';
  for (const ch of folded) {
    out += ch >= ' ' && ch <= '~' ? ch : UNREPRESENTABLE;
  }
  return out;
}

/** Escape a PDF literal string. */
function escapePdf(s: string): string {
  // The unrepresentable mark folds to '?' HERE, not in toAscii: it must be ONE
  // BYTE for the xref offsets, and '?' is the conventional stand-in no viewer
  // hides. toAscii keeps the distinct mark so tests can tell a marked omission
  // apart from a literal question mark in the manuscript.
  return s
    .split(UNREPRESENTABLE)
    .join('?')
    .replace(/\\/g, '\\\\')
    .replace(/\(/g, '\\(')
    .replace(/\)/g, '\\)');
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
