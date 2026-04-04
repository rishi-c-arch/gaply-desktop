/**
 * Routes that inject their own FAQPage (or merged FAQ) in JSON-LD.
 * App must not add the global FAQ script on these paths to avoid duplicate FAQPage.
 */
export const GUIDE_PATHS_WITH_OWN_FAQ_JSONLD: readonly string[] = [
  '/guides/ethical-researcher-guide-ai-academic-writing',
  '/guides/scopus-indexed-journals-low-apc',
  '/guides/fast-publication-scopus-journals-india',
];

export function pathnameUsesOwnFaqJsonLd(pathname: string): boolean {
  return GUIDE_PATHS_WITH_OWN_FAQ_JSONLD.some((p) => pathname === p || pathname.startsWith(`${p}/`));
}
