/**
 * Free features API - Paper Search & Journal Matching
 * Connects to gaply-enhanced-backend at https://gaply-backend-production.up.railway.app
 */

import { apiFetch, buildApiUrl } from './config';

// --- Paper Search (GET /api/search) ---

export interface FreeSearchResult {
  id: string;
  title: string;
  authors: string[];
  year: number;
  venue: string;
  doi?: string;
  abstract?: string;
  citation_count?: number;
  source_providers: string[];
  open_access: boolean;
  license?: string;
  pdf_url?: string;
  open_url?: string;
  score: number;
  relevance_score: number;
  quality_score: number;
  subjects?: string[];
}

export interface FreeSearchResponse {
  query: string;
  page: number;
  per_page: number;
  total_estimate: number;
  results: FreeSearchResult[];
  meta: {
    time_ms: number;
    sources_queried: string[];
    cache_hit: boolean;
    google_scholar_url?: string;
  };
}

export async function searchPapers(
  query: string,
  options?: { page?: number; perPage?: number; yearFrom?: number; yearTo?: number; openAccess?: boolean }
): Promise<FreeSearchResponse> {
  const params = new URLSearchParams();
  params.set('q', query.trim());
  if (options?.page) params.set('page', String(options.page));
  if (options?.perPage) params.set('per_page', String(options.perPage));
  if (options?.yearFrom) params.set('year_from', String(options.yearFrom));
  if (options?.yearTo) params.set('year_to', String(options.yearTo));
  if (options?.openAccess) params.set('open_access', 'true');

  const url = buildApiUrl(`/api/search?${params.toString()}`);
  const res = await apiFetch(url);
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error((err as { error?: string }).error || `Search failed: ${res.status}`);
  }
  return res.json();
}

// --- Journal Recommendations (POST /api/recommend-journals) ---

export interface JournalRecommendRequest {
  title: string;
  abstract: string;
  filters?: {
    open_access_only?: boolean;
    min_quartile?: number;
    subjects?: string[];
    region?: string[];
    max_apc?: number | null;
    limit?: number;
  };
}

export interface JournalRecommendItem {
  journal_id: string;
  name: string;
  issns: string[];
  publisher?: string;
  quartiles?: Record<string, string>;
  sjr_value?: number;
  open_access: boolean;
  doaj_entry: boolean;
  apc_usd?: number | null;
  submission_url?: string;
  homepage?: string;
  aims_scope_snippet?: string;
  subject_matches?: string[];
  match_score: number;
  quality_score: number;
  recommendation_score: number;
  reason_snippet?: string;
}

export interface JournalRecommendResponse {
  query_id: string;
  title: string;
  abstract: string;
  timestamp: string;
  recommendations: JournalRecommendItem[];
  meta: {
    elapsed_ms: number;
    sources_used: string[];
    data_freshness: Record<string, string>;
    cache_hit: boolean;
  };
}

export async function recommendJournals(req: JournalRecommendRequest): Promise<JournalRecommendResponse> {
  const body = {
    title: req.title.trim(),
    abstract: req.abstract.trim(),
    filters: {
      open_access_only: req.filters?.open_access_only ?? false,
      min_quartile: req.filters?.min_quartile ?? 0,
      subjects: req.filters?.subjects ?? [],
      region: req.filters?.region ?? [],
      max_apc: req.filters?.max_apc ?? null,
      limit: req.filters?.limit ?? 10,
    },
  };

  const res = await apiFetch(buildApiUrl('/api/recommend-journals'), {
    method: 'POST',
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error((err as { error?: string }).error || `Recommendation failed: ${res.status}`);
  }
  return res.json();
}
