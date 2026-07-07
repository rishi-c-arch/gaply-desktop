// Gaply — CSL style formatting. A dependency-free, deterministic formatter
// covering the common styles. Marketed as "cite in any style, for any journal
// worldwide": the FULL CSL set drops in behind this same `formatCitation` seam
// via citeproc-js + bundled CSL style/locale files (a production swap, kept out
// now to stay dependency-light and avoid the Object.freeze conflict class that
// jsPDF hit — see the report exporter). Nothing here touches the network.
import { CslItem } from './citationTypes';

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

/** Format one citation in the given style id. Unknown style → APA. */
export function formatCitation(c: CslItem, styleId: string): string {
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
