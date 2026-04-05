/** Visible on-page + JSON-LD keywords for Scopus/APC and Fast India guides (exact phrasing for search intent). */
export const RESEARCH_GUIDE_SEO_PHRASES = [
  'Latest UGC CARE List Group I and II PDF 2026',
  'Scopus indexed journals with low APC (Article Processing Charge)',
  'Fast publication journals for [Subject] in India',
  'How to identify predatory journals UGC guidelines',
  'Journal Quartile Analysis Q1 vs Q2 for PhD thesis submission',
] as const;

export function researchGuideKeywordsJoined(): string {
  return RESEARCH_GUIDE_SEO_PHRASES.join(', ');
}
