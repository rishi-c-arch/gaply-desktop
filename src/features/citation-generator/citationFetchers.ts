/** Client-side metadata fetchers (free APIs). No backend. */

const CROSSREF_MAILTO = 'helloresearcher@gaply.in';

export function normalizeDoi(raw: string): string {
  return raw
    .trim()
    .replace(/^(https?:\/\/)?(dx\.)?doi\.org\//i, '')
    .replace(/^doi:\s*/i, '');
}

export function normalizeIsbn(raw: string): string {
  return raw.replace(/[-\s]/g, '').trim();
}

export function normalizePmid(raw: string): string {
  const m = raw.trim().match(/(\d+)/);
  return m ? m[1] : '';
}

export type CslLike = Record<string, unknown>;

export function crossrefWorkToCsl(m: Record<string, unknown>): CslLike {
  const authorArr = (m.author as Array<{ family?: string; given?: string; name?: string }> | undefined) || [];
  const author = authorArr.map((a) => {
    if (a.family || a.given) {
      return { family: a.family || '', given: a.given || '' };
    }
    if (a.name) return { literal: a.name };
    return { literal: '' };
  });
  const titleRaw = m.title;
  const title =
    Array.isArray(titleRaw) && titleRaw.length ? String(titleRaw[0]) : titleRaw ? String(titleRaw) : '';
  const ct = m['container-title'];
  const containerTitle = Array.isArray(ct) && ct.length ? String(ct[0]) : ct ? String(ct) : undefined;
  const parts = (m.issued as { 'date-parts'?: number[][] } | undefined)?.['date-parts']?.[0];
  const doi = m.DOI ? String(m.DOI) : undefined;
  const urlList = m.URL as string[] | undefined;
  const id = doi || urlList?.[0] || `crossref-${title.slice(0, 40)}-${Date.now()}`;

  return {
    type: 'article-journal',
    id,
    title,
    author: author.filter((x) => 'literal' in x ? x.literal : x.family || x.given),
    issued: parts?.length ? { 'date-parts': [parts] } : undefined,
    'container-title': containerTitle,
    volume: m.volume != null ? String(m.volume) : undefined,
    issue: m.issue != null ? String(m.issue) : undefined,
    page: m.page != null ? String(m.page) : undefined,
    DOI: doi,
    URL: urlList?.[0] || (doi ? `https://doi.org/${doi}` : undefined),
  };
}

export async function fetchCrossrefByDoi(doi: string): Promise<CslLike> {
  const d = encodeURIComponent(normalizeDoi(doi));
  const url = `https://api.crossref.org/works/${d}?mailto=${encodeURIComponent(CROSSREF_MAILTO)}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Crossref: ${res.status}`);
  const json = await res.json();
  const m = json?.message;
  if (!m) throw new Error('Crossref: empty message');
  return crossrefWorkToCsl(m);
}

export function openLibraryToCsl(data: Record<string, unknown>, isbn: string): CslLike {
  const title = String(data.title || '');
  const publish = (data.publish_date as string) || '';
  const year = publish ? parseInt(publish.slice(0, 4), 10) : undefined;
  const authors = (data.authors as Array<{ name?: string }> | undefined) || [];
  const author = authors.map((a) => ({ literal: a.name || '' })).filter((a) => a.literal);
  const pub0 = Array.isArray(data.publishers) ? data.publishers[0] : undefined;
  const publisher =
    typeof pub0 === 'string' ? pub0 : pub0 && typeof pub0 === 'object' && 'name' in pub0 ? String((pub0 as { name?: string }).name) : undefined;

  return {
    type: 'book',
    id: `isbn-${isbn}`,
    title,
    author: author.length ? author : [{ literal: 'Unknown author' }],
    issued: year && !Number.isNaN(year) ? { 'date-parts': [[year]] } : undefined,
    ISBN: isbn,
    publisher,
  };
}

export async function fetchOpenLibraryByIsbn(isbnRaw: string): Promise<CslLike> {
  const isbn = normalizeIsbn(isbnRaw);
  if (!isbn) throw new Error('Invalid ISBN');
  const res = await fetch(`https://openlibrary.org/isbn/${isbn}.json`);
  if (!res.ok) throw new Error(`OpenLibrary: ${res.status}`);
  const data = await res.json();
  return openLibraryToCsl(data, isbn);
}

export function pubmedSummaryToCsl(uid: string, rec: Record<string, unknown>): CslLike {
  const authors = ((rec.authors as Array<{ name?: string }>) || []).map((a) => ({
    literal: a.name || '',
  }));
  const title = String(rec.title || '').replace(/\.$/, '');
  const pubdate = String(rec.pubdate || rec.epubdate || '');
  const y = pubdate ? parseInt(pubdate.slice(0, 4), 10) : undefined;

  return {
    type: 'article-journal',
    id: `pmid-${uid}`,
    title,
    author: authors.length ? authors : [{ literal: 'Unknown author' }],
    issued: y && !Number.isNaN(y) ? { 'date-parts': [[y]] } : undefined,
    PMID: uid,
    URL: `https://pubmed.ncbi.nlm.nih.gov/${uid}/`,
  };
}

export async function fetchPubmedByPmid(pmidRaw: string): Promise<CslLike> {
  const pmid = normalizePmid(pmidRaw);
  if (!pmid) throw new Error('Invalid PubMed ID');
  const url = `https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id=${pmid}&retmode=json`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`PubMed: ${res.status}`);
  const json = await res.json();
  const rec = json?.result?.[pmid];
  if (!rec || rec.error) throw new Error(rec?.error || 'PubMed: record not found');
  return pubmedSummaryToCsl(pmid, rec);
}

export type DetectedFormat = 'bibtex' | 'ris' | 'json' | 'xml' | 'plain' | 'unknown';

export function detectInputFormat(text: string): DetectedFormat {
  const t = text.trim();
  if (!t) return 'unknown';
  if (t.startsWith('@')) return 'bibtex';
  if (/^TY\s\s?-/m.test(t)) return 'ris';
  if ((t.startsWith('{') && t.includes('"type"')) || (t.startsWith('[') && t.includes('"type"'))) return 'json';
  if (t.includes('<record') && t.includes('</record>')) return 'xml';
  if (t.includes('<?xml') && t.includes('EndNote')) return 'xml';
  return 'plain';
}

/** Very small heuristic: "Author A, Author B (2020). Title. Journal 12(3): 1-10." */
export function parsePlainTextReference(line: string): CslLike | null {
  const s = line.trim();
  if (s.length < 12) return null;
  const parenYear = s.match(/\((\d{4})\)\s*\.\s*(.+)/);
  if (parenYear) {
    const year = parseInt(parenYear[1], 10);
    const rest = parenYear[2];
    const before = s.slice(0, s.indexOf('(')).trim();
    const authors = before.split(/,| and /i).map((x) => ({ literal: x.trim() })).filter((x) => x.literal);
    const titleJournal = rest.split(/\.\s+/);
    const title = titleJournal[0]?.trim() || rest;
    return {
      type: 'article-journal',
      id: `plain-${Date.now()}`,
      title,
      author: authors.length ? authors : [{ literal: 'Unknown' }],
      issued: { 'date-parts': [[year]] },
      'container-title': titleJournal[1]?.trim(),
    };
  }
  return null;
}

/** Minimal EndNote XML → CSL-like items (generic tags vary by export). */
export function parseEndNoteXml(xml: string): CslLike[] {
  const doc = new DOMParser().parseFromString(xml, 'text/xml');
  if (doc.querySelector('parsererror')) return [];
  const records = doc.querySelectorAll('record');
  const out: CslLike[] = [];
  records.forEach((rec) => {
    const text = (sel: string) => rec.querySelector(sel)?.textContent?.trim() || '';
    const title = text('title') || text('ArticleTitle') || text('Title');
    const authorsRaw = text('author') || text('Authors') || text('Author');
    const yearStr = text('year') || text('Year') || text('PublicationYear');
    const journal = text('secondary-title') || text('Journal') || text('Periodical');
    if (!title && !authorsRaw) return;
    const year = yearStr ? parseInt(yearStr.slice(0, 4), 10) : NaN;
    const authorParts = authorsRaw
      .split(/;|,|\band\b/i)
      .map((s) => s.trim())
      .filter(Boolean)
      .map((name) => ({ literal: name }));
    out.push({
      type: journal ? 'article-journal' : 'book',
      id: `endnote-${out.length}-${Date.now()}`,
      title,
      author: authorParts.length ? authorParts : [{ literal: 'Unknown' }],
      issued: !Number.isNaN(year) ? { 'date-parts': [[year]] } : undefined,
      'container-title': journal || undefined,
    });
  });
  return out;
}
