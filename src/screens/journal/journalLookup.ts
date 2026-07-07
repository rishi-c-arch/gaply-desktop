// Gaply — online journal lookup fallback (via the proxy) with a TTL cache. Used
// when a journal isn't in the local directory. International standards only —
// SJR/Scopus/WoS. The real client routes through the Railway proxy; a mock ships
// for tests. Conference legitimacy uses CORE-ranking-style signals.
import { JournalRecord } from './journalData';

export interface JournalLookupClient {
  lookup(query: string): Promise<JournalRecord | null>;
}

/** Production path (documented): GET the proxy's /journal-lookup, which queries
 *  Scimago/Scopus server-side. Throws until wired. */
export class ProxyJournalLookup implements JournalLookupClient {
  constructor(private proxyBaseUrl: string) {}
  async lookup(_query: string): Promise<JournalRecord | null> {
    throw new Error('ProxyJournalLookup: /journal-lookup endpoint not wired yet.');
  }
}

/** Test/dev double. */
export function makeMockLookup(
  responder: (query: string) => JournalRecord | null
): JournalLookupClient & { calls: string[] } {
  const calls: string[] = [];
  return {
    calls,
    async lookup(query: string) {
      calls.push(query);
      return responder(query);
    },
  };
}

/** TTL cache wrapper — a miss calls the underlying client and caches the result;
 *  a hit within the TTL returns it without another call. */
export class CachedJournalLookup implements JournalLookupClient {
  private cache = new Map<string, { record: JournalRecord | null; expires: number }>();
  constructor(
    private inner: JournalLookupClient,
    private ttlMs = 24 * 60 * 60 * 1000,
    private now: () => number = () => Date.now()
  ) {}
  async lookup(query: string): Promise<JournalRecord | null> {
    const key = query.trim().toLowerCase();
    const hit = this.cache.get(key);
    if (hit && hit.expires > this.now()) return hit.record;
    const record = await this.inner.lookup(query);
    this.cache.set(key, { record, expires: this.now() + this.ttlMs });
    return record;
  }
  /** Test helper. */
  cachedKeys(): string[] {
    return Array.from(this.cache.keys());
  }
}

/* ------------------------------ conferences ----------------------------- */

export type CoreRank = 'A*' | 'A' | 'B' | 'C' | 'unranked';

export interface ConferenceRecord {
  name: string;
  acronym: string;
  coreRank: CoreRank;
}

// A small CORE-ranking-style set (extends via the same online seam).
export const CONFERENCES: ConferenceRecord[] = [
  { name: 'Conference on Neural Information Processing Systems', acronym: 'NeurIPS', coreRank: 'A*' },
  { name: 'International Conference on Machine Learning', acronym: 'ICML', coreRank: 'A*' },
  { name: 'IEEE Conference on Computer Vision and Pattern Recognition', acronym: 'CVPR', coreRank: 'A*' },
  { name: 'Annual Meeting of the Association for Computational Linguistics', acronym: 'ACL', coreRank: 'A*' },
  { name: 'World Congress on Engineering and Emerging Technologies', acronym: 'WCEET', coreRank: 'unranked' },
];

export function searchConference(query: string): ConferenceRecord[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return CONFERENCES.filter(
    (c) => c.name.toLowerCase().includes(q) || c.acronym.toLowerCase().includes(q)
  ).slice(0, 8);
}

export function conferenceRisk(c: ConferenceRecord): 'green' | 'amber' | 'red' {
  if (c.coreRank === 'A*' || c.coreRank === 'A') return 'green';
  if (c.coreRank === 'B') return 'amber';
  return 'red'; // C / unranked — verify legitimacy
}
