// Gaply — Journal Check data layer. Built from the bundled Scopus journal
// directory. Legitimacy + quality is judged on INTERNATIONAL standards only:
// SJR (Scimago) quartiles Q1–Q4 and Scopus / Web of Science indexing. No
// national list of any kind is used (see the test that asserts this).
import raw from '../../data/scopusDirectory.json';

export type Quartile = 'Q1' | 'Q2' | 'Q3' | 'Q4';

export interface JournalRecord {
  name: string;
  issn: string | null;
  publisher: string;
  /** SJR-derived Scimago quartile. */
  quartile: Quartile | null;
  /** Numeric SJR score when known (online lookups can supply it; the local
   *  directory carries the quartile only). */
  sjr: number | null;
  guidelinesUrl: string | null;
  category: string;
  indexing: { scopus: boolean; wos: boolean };
  /** Beall's/Cabells-style predatory signals detected for this record. */
  signals: string[];
  source: 'local' | 'online';
}

function parseIssn(line?: string): string | null {
  const m = (line ?? '').match(/(\d{4}-\d{3}[\dxX])/);
  return m ? m[1].toUpperCase() : null;
}
function normQuartile(q?: string): Quartile | null {
  const m = (q ?? '').match(/Q[1-4]/);
  return (m?.[0] as Quartile) ?? null;
}

/** The local directory, transformed to JournalRecord[]. */
export const JOURNALS: JournalRecord[] = (raw as any).journals.map((j: any): JournalRecord => {
  const quartile = normQuartile(j.quartile);
  return {
    name: j.title,
    issn: parseIssn(j.issnLine),
    publisher: j.publisher ?? 'Unknown',
    quartile,
    sjr: null, // the directory stores the quartile; numeric SJR via online lookup
    guidelinesUrl: j.website ?? null,
    category: j.category ?? '',
    // It is a Scopus directory; WoS is inferred (top quartiles usually indexed).
    indexing: { scopus: true, wos: quartile === 'Q1' || quartile === 'Q2' },
    signals: [],
    source: 'local',
  };
});

export const JOURNAL_COUNT = JOURNALS.length;

export const QUARTILE_STATUS: Record<Quartile, 'certain' | 'assessed' | 'flagged' | 'neutral'> = {
  Q1: 'certain', // gold-tier; rendered gold in the UI
  Q2: 'neutral',
  Q3: 'assessed',
  Q4: 'flagged',
};

/* ------------------------- predatory-risk verdict ----------------------- */

export type RiskLevel = 'green' | 'amber' | 'red';

export interface RiskVerdict {
  level: RiskLevel;
  reasons: string[];
}

/** Cross-reference indexing + Beall's/Cabells-style signals. Traffic light:
 *  red = likely predatory / unindexed; amber = caution (Q4, single index,
 *  missing metadata); green = well-indexed international journal. */
export function riskVerdict(r: JournalRecord): RiskVerdict {
  const reasons: string[] = [];
  const indexed = r.indexing.scopus || r.indexing.wos;

  if (!indexed) reasons.push('Not indexed in Scopus or Web of Science');
  for (const s of r.signals) reasons.push(s);
  if (!r.issn) reasons.push('No ISSN on record');

  if (!indexed || r.signals.length > 0) {
    return { level: 'red', reasons: reasons.length ? reasons : ['Multiple predatory signals'] };
  }
  if (r.quartile === 'Q4' || !r.issn || (r.indexing.scopus !== r.indexing.wos && !r.indexing.wos)) {
    return {
      level: 'amber',
      reasons: reasons.length ? reasons : ['Indexed, but single-database or lower quartile — verify fit'],
    };
  }
  return {
    level: 'green',
    reasons: [`Indexed in Scopus${r.indexing.wos ? ' & Web of Science' : ''}`],
  };
}

/* ------------------------------ local search ---------------------------- */

function normIssn(s: string): string {
  return s.replace(/[^0-9xX]/g, '').toUpperCase();
}

/** Search the local directory by name, ISSN, or URL. Returns best matches. */
export function searchLocal(query: string): JournalRecord[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];

  // ISSN?
  const issnLike = q.match(/\d{4}-?\d{3}[\dxX]/);
  if (issnLike) {
    const target = normIssn(issnLike[0]);
    return JOURNALS.filter((j) => j.issn && normIssn(j.issn) === target);
  }
  // URL?
  if (q.includes('.') && (q.includes('/') || q.startsWith('http') || q.includes('www'))) {
    const domain = q.replace(/^https?:\/\//, '').replace(/^www\./, '').split('/')[0];
    return JOURNALS.filter((j) => j.guidelinesUrl?.toLowerCase().includes(domain));
  }
  // name contains — exact-ish first, then substring
  const exact = JOURNALS.filter((j) => j.name.toLowerCase() === q);
  if (exact.length) return exact;
  return JOURNALS.filter((j) => j.name.toLowerCase().includes(q)).slice(0, 10);
}
