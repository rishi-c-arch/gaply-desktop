// Gaply — the Set 2 verified-metadata bridge (Set 5 glue). Wires
// resolve_citation_metadata into the F8 page: upload a paper / give a DOI →
// FULL verified CSL-JSON (authors/journal/volume/pages) or an honest
// Unverified with a manual-entry hint. No LLM, no proxy — the free path.
import { isTauri } from '../../utils/isTauri';
import { CslItem } from './citationTypes';

/** Rust CitationMetadata (snake_case). Every field verified-or-absent. */
export interface VerifiedMetadata {
  source: string;
  matched_by: 'doi' | 'title';
  csl_type: string;
  doi: string | null;
  title: string | null;
  authors: Array<{ family: string; given: string | null }>;
  container_title: string | null;
  year: number | null;
  volume: string | null;
  issue: string | null;
  page: string | null;
}

export type CitationResolveResult =
  | { status: 'verified'; metadata: VerifiedMetadata }
  | { status: 'unverified'; reason: string; unverified_title_hint: string | null };

export interface CitationResolveBridge {
  resolve(input: { path?: string; doi?: string; title?: string }): Promise<CitationResolveResult>;
}

/** Verified metadata → the page's CslItem. Absent stays absent. */
export function metadataToCslItem(m: VerifiedMetadata, fallbackId: string): CslItem {
  return {
    id: m.doi ?? fallbackId,
    type: m.csl_type || 'article-journal',
    title: m.title ?? '(untitled)',
    author: m.authors.map((a) => ({ family: a.family, ...(a.given ? { given: a.given } : {}) })),
    ...(m.year != null ? { issued: { year: m.year } } : {}),
    ...(m.doi ? { DOI: m.doi } : {}),
    ...(m.container_title ? { containerTitle: m.container_title } : {}),
    ...(m.volume ? { volume: m.volume } : {}),
    ...(m.issue ? { issue: m.issue } : {}),
    ...(m.page ? { page: m.page } : {}),
  };
}

/** Production bridge: the Set 2 Tauri command. Desktop-app only. */
export class TauriCitationResolve implements CitationResolveBridge {
  async resolve(input: { path?: string; doi?: string; title?: string }): Promise<CitationResolveResult> {
    if (!isTauri) {
      throw new Error('Citation metadata resolution runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    return (await invoke('resolve_citation_metadata', {
      path: input.path ?? null,
      doi: input.doi ?? null,
      title: input.title ?? null,
    })) as CitationResolveResult;
  }
}

/** Test double. */
export function makeMockResolve(
  responder: (input: { path?: string; doi?: string; title?: string }) => CitationResolveResult
): CitationResolveBridge & { calls: Array<{ path?: string; doi?: string; title?: string }> } {
  const calls: Array<{ path?: string; doi?: string; title?: string }> = [];
  return {
    calls,
    async resolve(input) {
      calls.push(input);
      return responder(input);
    },
  };
}
