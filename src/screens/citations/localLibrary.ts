// Gaply — the LOCAL-FIRST citation library bridge (Set 4). Local sqlite is
// the SOURCE OF TRUTH (fully offline, no sign-in); the existing Supabase
// citation_library service is demoted to an OPTIONAL sync layer the page
// drives on top, with honest per-reference sync status. No LLM, no proxy.
import { isTauri } from '../../utils/isTauri';
import { Citation, CitationSource, CslItem, verificationState } from './citationTypes';

export type SyncStatus = 'local_only' | 'pending' | 'synced';

/** Backend StoredReference shape (snake_case, as serialized by Rust). */
export interface StoredReference {
  id: string;
  csl_json: string;
  doi: string | null;
  title: string;
  authors: string;
  year: number | null;
  tags: string[];
  // Scope B: persisted verification + retraction facts (durable across reload).
  retracted: boolean;
  source: string | null;
  verify_provenance: string[];
  verify_outcome: string | null;
  verified_at: number | null;
  sync_status: SyncStatus;
  created_at: number;
  updated_at: number;
}

export interface LocalLibrary {
  upsert(c: Citation, tags: string[]): Promise<StoredReference>;
  list(): Promise<StoredReference[]>;
  search(query: string, tag?: string): Promise<StoredReference[]>;
  setTags(id: string, tags: string[]): Promise<StoredReference>;
  remove(id: string): Promise<void>;
  markSync(id: string, status: SyncStatus): Promise<void>;
}

/** Stored row → the page's Citation shape (csl_json is the truth). */
export function storedToCitation(r: StoredReference): Citation {
  let csl: CslItem;
  try {
    const parsed = JSON.parse(r.csl_json) as Record<string, unknown>;
    csl = {
      id: (parsed.id as string) ?? r.id,
      type: (parsed.type as string) ?? 'article-journal',
      title: (parsed.title as string) ?? r.title,
      author: Array.isArray(parsed.author) ? (parsed.author as CslItem['author']) : [],
      issued: parsed.issued
        ? { year: (parsed.issued as { 'date-parts'?: number[][]; year?: number })['date-parts']?.[0]?.[0] ?? (parsed.issued as { year?: number }).year }
        : r.year != null
          ? { year: r.year }
          : undefined,
      DOI: (parsed.DOI as string) ?? undefined,
      containerTitle: (parsed['container-title'] as string) ?? (parsed.containerTitle as string) ?? undefined,
      volume: (parsed.volume as string) ?? undefined,
      issue: (parsed.issue as string) ?? undefined,
      page: (parsed.page as string) ?? undefined,
    };
  } catch {
    csl = { id: r.id, type: 'article-journal', title: r.title, author: [] };
  }
  return {
    id: r.id,
    csl,
    doi: r.doi,
    // Scope B: read the persisted facts instead of hardcoding. A retracted paper
    // stays flagged after reload; a verified entry keeps its CrossRef provenance.
    retracted: !!r.retracted,
    source: (r.source ?? 'manual') as CitationSource,
    provenance: r.verify_provenance ?? [],
    verifyOutcome: (r.verify_outcome as 'not_found' | 'check_failed' | null) ?? undefined,
    verifiedAt: r.verified_at ?? undefined,
    tags: r.tags,
    syncStatus: r.sync_status,
  };
}

/** Citation → the CSL-JSON we store (strict keys, absent stays absent). */
function citationToCslJson(c: Citation): Record<string, unknown> {
  const out: Record<string, unknown> = {
    id: c.csl.id || c.id,
    type: c.csl.type || 'article-journal',
    title: c.csl.title,
    author: c.csl.author,
  };
  if (c.csl.issued?.year !== undefined) out.issued = { 'date-parts': [[c.csl.issued.year]] };
  if (c.csl.DOI) out.DOI = c.csl.DOI;
  if (c.csl.containerTitle) out['container-title'] = c.csl.containerTitle;
  if (c.csl.volume) out.volume = c.csl.volume;
  if (c.csl.issue) out.issue = c.csl.issue;
  if (c.csl.page) out.page = c.csl.page;
  return out;
}

/** Production bridge: the fully-local Tauri commands. Desktop-app only. */
export class TauriLocalLibrary implements LocalLibrary {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    if (!isTauri) {
      throw new Error('The local citation library runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    return (await invoke(cmd, args)) as T;
  }

  upsert(c: Citation, tags: string[]) {
    // Scope B: persist the verification/retraction facts alongside the CSL-JSON.
    // verifiedAt is stamped once, the first time an entry is CrossRef-verified,
    // and preserved thereafter (storedToCitation reads it back into c.verifiedAt).
    const verifiedAt =
      c.verifiedAt ?? (verificationState(c) === 'verified' ? Date.now() : null);
    return this.invoke<StoredReference>('citation_lib_upsert', {
      id: c.id,
      cslJson: citationToCslJson(c),
      doi: c.doi,
      tags,
      retracted: c.retracted,
      source: c.source,
      verifyProvenance: c.provenance ?? [],
      verifyOutcome: c.verifyOutcome ?? null,
      verifiedAt,
    });
  }
  list() {
    return this.invoke<StoredReference[]>('citation_lib_list', {});
  }
  search(query: string, tag?: string) {
    return this.invoke<StoredReference[]>('citation_lib_search', { query, tag: tag ?? null });
  }
  setTags(id: string, tags: string[]) {
    return this.invoke<StoredReference>('citation_lib_set_tags', { id, tags });
  }
  async remove(id: string) {
    await this.invoke<void>('citation_lib_delete', { id });
  }
  async markSync(id: string, status: SyncStatus) {
    await this.invoke<void>('citation_lib_set_sync_status', { id, status });
  }
}

/** In-memory double mirroring the Rust semantics (mutations reset to
 *  local_only; search over title/authors/doi/year/tags; honest statuses). */
export function makeMockLocalLibrary(): LocalLibrary & { rows: Map<string, StoredReference> } {
  const rows = new Map<string, StoredReference>();
  const store = (c: Citation, tags: string[], prev?: StoredReference): StoredReference => {
    const csl = citationToCslJson(c);
    const row: StoredReference = {
      id: c.id,
      csl_json: JSON.stringify(csl),
      doi: c.doi ?? ((csl.DOI as string) || null),
      title: c.csl.title,
      authors: c.csl.author.map((a) => [a.family, a.given].filter(Boolean).join(', ')).join('; '),
      year: c.csl.issued?.year ?? null,
      tags,
      // Scope B: mirror the Rust store — persist the verification/retraction facts.
      retracted: c.retracted,
      source: c.source ?? null,
      verify_provenance: c.provenance ?? [],
      verify_outcome: c.verifyOutcome ?? null,
      verified_at: c.verifiedAt ?? null,
      sync_status: 'local_only',
      created_at: prev?.created_at ?? 1,
      updated_at: (prev?.updated_at ?? 0) + 1,
    };
    rows.set(c.id, row);
    return row;
  };
  return {
    rows,
    async upsert(c, tags) {
      return store(c, tags, rows.get(c.id));
    },
    async list() {
      return Array.from(rows.values()).sort((a, b) => b.updated_at - a.updated_at);
    },
    async search(query, tag) {
      const q = query.trim().toLowerCase();
      return Array.from(rows.values()).filter((r) => {
        const hit =
          q === '' ||
          r.title.toLowerCase().includes(q) ||
          r.authors.toLowerCase().includes(q) ||
          (r.doi ?? '').toLowerCase().includes(q) ||
          String(r.year ?? '').includes(q) ||
          r.tags.some((t: string) => t.toLowerCase().includes(q));
        const tagHit = !tag || r.tags.includes(tag);
        return hit && tagHit;
      });
    },
    async setTags(id, tags) {
      const r = rows.get(id);
      if (!r) throw new Error(`citation not found: ${id}`);
      const next = { ...r, tags, sync_status: 'local_only' as const, updated_at: r.updated_at + 1 };
      rows.set(id, next);
      return next;
    },
    async remove(id) {
      rows.delete(id);
    },
    async markSync(id, status) {
      const r = rows.get(id);
      if (!r) throw new Error(`citation not found: ${id}`);
      rows.set(id, { ...r, sync_status: status });
    },
  };
}
