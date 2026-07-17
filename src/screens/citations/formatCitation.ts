// Gaply — CSL style formatting (Set 3: the documented upgrade landed). The
// FULL CSL set (~2,856 bundled styles) now formats behind this SAME seam via
// citeproc-js (cslEngine.ts): once `prepareStyle(styleId)` has loaded a
// style, `formatCitation` routes through the real CSL processor —
// deterministically, offline (styles are app assets), NO LLM. Before a
// style is prepared, the original hand-rolled formatter still serves the 8
// legacy ids (unchanged behavior for existing callers); an unknown,
// unprepared style id is an HONEST error — never a silent wrong-style
// fallback. Nothing here touches the network.
import { CslItem } from './citationTypes';
import { formatWithCsl, isStyleReady, LEGACY_STYLE_ALIASES } from './cslEngine';

export interface CslStyle {
  id: string;
  label: string;
}

/** The curated set shipped today. `citeproc` + a style repo extends this to the
 *  full ~10k CSL styles without changing any calling code. */
export const CSL_STYLES: CslStyle[] = [
  { id: 'apa', label: 'APA 7th' },
  { id: 'mla', label: 'MLA 9th' },
  { id: 'chicago-author-date', label: 'Chicago (author-date)' },
  { id: 'vancouver', label: 'Vancouver' },
  { id: 'harvard', label: 'Harvard' },
  { id: 'ieee', label: 'IEEE' },
  { id: 'nature', label: 'Nature' },
  { id: 'ama', label: 'AMA 11th' },
];

const y = (c: CslItem) => c.issued?.year ?? 'n.d.';
const doiUrl = (c: CslItem) => (c.DOI ? `https://doi.org/${c.DOI}` : c.URL ?? '');

function authorsApa(c: CslItem): string {
  const names = c.author.map((a) => `${a.family}, ${initials(a.given)}`);
  return joinAuthors(names, ', & ');
}
function initials(given?: string): string {
  if (!given) return '';
  return given
    .split(/\s+/)
    .map((g) => `${g[0]?.toUpperCase() ?? ''}.`)
    .join(' ');
}
function joinAuthors(names: string[], lastSep: string): string {
  if (names.length === 0) return 'Anonymous';
  if (names.length === 1) return names[0];
  return names.slice(0, -1).join(', ') + lastSep + names[names.length - 1];
}
function vancouverAuthors(c: CslItem): string {
  return c.author
    .map((a) => `${a.family} ${(a.given ?? '').split(/\s+/).map((g) => g[0] ?? '').join('')}`.trim())
    .join(', ');
}
function ieeeAuthors(c: CslItem): string {
  const names = c.author.map((a) => `${initials(a.given).replace(/\s/g, '')} ${a.family}`.trim());
  return joinAuthors(names, ' and ');
}

/** Format one citation in the given style id. Full-CSL (citeproc) once the
 *  style is prepared; the legacy hand-rolled path covers the original 8 ids
 *  before that; anything else errs honestly. */
/** citeproc 'html' → the preview's lightweight *italic* / **bold** markers, so
 *  the same `Formatted` component renders italics/bold whether the string came
 *  from the legacy formatter or citeproc. Italic journal names ARE the spec in
 *  these styles — dropping them (as plain 'text' would) while claiming
 *  spec-correctness would be its own small lie. Export stays plain 'text'
 *  (a .txt bibliography should have no markup — see exporters.ts). */
export function cslHtmlToMarkers(html: string): string {
  return html
    .replace(/<\/?i>/gi, '*')
    .replace(/<\/?(?:b|strong)>/gi, '**')
    .replace(/<[^>]+>/g, '') // strip the csl-bib-body / csl-entry / left-margin wrappers
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&#0?38;|&#x0?26;/gi, '&')
    .replace(/&nbsp;|&#0?160;/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export function formatCitation(c: CslItem, styleId: string): string {
  if (isStyleReady(styleId)) {
    return cslHtmlToMarkers(formatWithCsl([c], styleId, 'en-US', 'html'));
  }
  if (!(styleId in LEGACY_STYLE_ALIASES)) {
    throw new Error(
      `style "${styleId}" is not loaded — prepareStyle(styleId) loads any of the bundled CSL styles`
    );
  }
  return formatLegacy(c, styleId);
}

function formatLegacy(c: CslItem, styleId: string): string {
  const vol = c.volume ?? '';
  const iss = c.issue ? `(${c.issue})` : '';
  const pages = c.page ?? '';
  const j = c.containerTitle ?? '';
  switch (styleId) {
    case 'mla':
      return `${mlaAuthors(c)}. “${c.title}.” ${italic(j)}, vol. ${vol}, no. ${c.issue ?? ''}, ${y(c)}, pp. ${pages}.`;
    case 'chicago-author-date':
      return `${mlaAuthors(c)}. ${y(c)}. “${c.title}.” ${italic(j)} ${vol}${iss}: ${pages}.`;
    case 'vancouver':
      return `${vancouverAuthors(c)}. ${c.title}. ${j}. ${y(c)};${vol}${iss}:${pages}.`;
    case 'harvard':
      return `${authorsApa(c)} (${y(c)}) ‘${c.title}’, ${italic(j)}, ${vol}${iss}, pp. ${pages}.`;
    case 'ieee':
      return `${ieeeAuthors(c)}, “${c.title},” ${italic(j)}, vol. ${vol}, no. ${c.issue ?? ''}, pp. ${pages}, ${y(c)}.`;
    case 'nature':
      return `${vancouverAuthors(c)}. ${c.title}. ${italic(j)} ${bold(vol)}, ${pages} (${y(c)}).`;
    case 'ama':
      return `${vancouverAuthors(c)}. ${c.title}. ${italic(j)}. ${y(c)};${vol}${iss}:${pages}. doi:${c.DOI ?? ''}`;
    case 'apa':
    default:
      return `${authorsApa(c)} (${y(c)}). ${c.title}. ${italic(j)}, ${vol}${iss}, ${pages}. ${doiUrl(c)}`.trim();
  }
}

function mlaAuthors(c: CslItem): string {
  const names = c.author.map((a, i) =>
    i === 0 ? `${a.family}, ${a.given ?? ''}`.trim() : `${a.given ?? ''} ${a.family}`.trim()
  );
  return joinAuthors(names, ', and ');
}

// Lightweight emphasis markers the preview renders (plain text elsewhere).
function italic(s: string): string {
  return s ? `*${s}*` : '';
}
function bold(s: string): string {
  return s ? `**${s}**` : '';
}

/** Build a full bibliography (one entry per line) in a style. */
export function formatBibliography(items: CslItem[], styleId: string): string {
  return items.map((c) => formatCitation(c, styleId)).join('\n\n');
}
