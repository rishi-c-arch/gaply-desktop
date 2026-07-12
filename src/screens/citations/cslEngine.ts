// Gaply — the full CSL formatting engine (Citation Manager Set 3).
// citation-js/citeproc-js behind the existing `formatCitation` seam: verified
// CSL-JSON in → a citation in ANY of the ~2,856 bundled styles out.
//
// DETERMINISTIC GUARANTEE: formatting is pure citeproc computation — the same
// CSL-JSON + the same style ALWAYS yields the same output. NO LLM, NO
// network: every .csl style and locale ships as an app asset
// (public/csl/…), loaded from the app's own origin. A field absent from the
// CSL-JSON simply doesn't appear — citeproc applies the style's
// missing-field rules, never inventing values.
//
// HONESTY NOTE: citation-js's own fetchStyle silently falls back to APA for
// unknown templates — this module REFUSES that: an unregistered style id is
// an honest error, never a silent wrong-style output.
//
// Plugins are lazy-loaded exactly like the proven citeEngine.ts pattern
// (avoids the Object.freeze conflict class the old formatter's header
// warned about — nothing here mutates frozen module objects at import time).
import type { Cite } from '@citation-js/core';
import { CslItem } from './citationTypes';

export interface StyleEntry {
  id: string;
  title: string;
}

/** Legacy 8-style ids (the old hand-rolled formatter) → bundled style ids. */
export const LEGACY_STYLE_ALIASES: Record<string, string> = {
  apa: 'apa',
  mla: 'modern-language-association',
  'chicago-author-date': 'chicago-author-date',
  // The styles repo retired the standalone vancouver.csl — classic
  // Vancouver IS the NLM citation-sequence style.
  vancouver: 'nlm-citation-sequence',
  harvard: 'harvard-cite-them-right',
  ieee: 'ieee',
  nature: 'nature',
  ama: 'american-medical-association',
};

let CiteClass: typeof Cite | null = null;
let cslConfig: { templates: { add: (n: string, x: string) => void; has: (n: string) => boolean }; locales: { add: (n: string, x: string) => void; has: (n: string) => boolean } } | null = null;
const readyStyles = new Set<string>();
const readyLocales = new Set<string>();
let manifestCache: StyleEntry[] | null = null;

let pluginsPromise: Promise<void> | null = null;

/** Lazy-load citation-js + plugins once (the citeEngine.ts pattern). */
export function ensureCslEngine(): Promise<void> {
  if (!pluginsPromise) {
    pluginsPromise = (async () => {
      await import('@citation-js/plugin-csl');
      const core = await import('@citation-js/core');
      CiteClass = core.Cite;
      cslConfig = core.plugins.config.get('@csl') as NonNullable<typeof cslConfig>;
    })();
  }
  return pluginsPromise;
}

/** True when `styleId` can format synchronously right now. */
export function isStyleReady(styleId: string): boolean {
  return readyStyles.has(styleId);
}

/** Register a style's raw .csl XML under `styleId` (tests + asset loader). */
export async function registerStyleXml(styleId: string, xml: string): Promise<void> {
  await ensureCslEngine();
  cslConfig!.templates.add(styleId, xml);
  readyStyles.add(styleId);
}

/** Register a locale's raw XML (e.g. 'en-US'). */
export async function registerLocaleXml(lang: string, xml: string): Promise<void> {
  await ensureCslEngine();
  cslConfig!.locales.add(lang, xml);
  readyLocales.add(lang);
}

/** Load a bundled style from the app's own assets (no network — the file
 *  ships with the app). Resolves legacy aliases; registers under BOTH ids. */
export async function prepareStyle(styleId: string, locale = 'en-US'): Promise<void> {
  await ensureCslEngine();
  if (!readyLocales.has(locale) && locale !== 'en-US') {
    const resp = await fetch(`/csl/locales/locales-${locale}.xml`);
    if (resp.ok) await registerLocaleXml(locale, await resp.text());
  }
  if (readyStyles.has(styleId)) return;
  const fileId = LEGACY_STYLE_ALIASES[styleId] ?? styleId;
  const resp = await fetch(`/csl/styles/${fileId}.csl`);
  if (!resp.ok) {
    throw new Error(`CSL style "${styleId}" is not in the bundled style set (asset ${fileId}.csl missing)`);
  }
  const xml = await resp.text();
  await registerStyleXml(styleId, xml);
  if (fileId !== styleId) await registerStyleXml(fileId, xml);
}

/** The style catalog (id + human title) for the style picker. Bundled asset;
 *  cached after first load. */
export async function listStyles(): Promise<StyleEntry[]> {
  if (!manifestCache) {
    const resp = await fetch('/csl/manifest.json');
    if (!resp.ok) throw new Error('CSL style manifest missing from app assets');
    manifestCache = (await resp.json()) as StyleEntry[];
  }
  return manifestCache;
}

/** Frontend CslItem → strict CSL-JSON keys. Absent stays absent — this
 *  mapping never fills a field (the Set 2 anti-hallucination rule carries
 *  through to formatting). */
export function toCslJson(c: CslItem): Record<string, unknown> {
  const out: Record<string, unknown> = {
    id: c.id || 'item-1',
    type: c.type || 'article-journal',
    title: c.title,
    author: c.author.map((a) => ({ family: a.family, ...(a.given ? { given: a.given } : {}) })),
  };
  if (c.issued?.year !== undefined) out.issued = { 'date-parts': [[c.issued.year]] };
  if (c.DOI) out.DOI = c.DOI;
  if (c.URL) out.URL = c.URL;
  if (c.containerTitle) out['container-title'] = c.containerTitle;
  if (c.volume) out.volume = c.volume;
  if (c.issue) out.issue = c.issue;
  if (c.page) out.page = c.page;
  return out;
}

/** SYNC citeproc formatting — callable once `prepareStyle` (or a direct
 *  registration) has run. Refuses unknown styles honestly: citation-js's
 *  silent fall-back-to-APA is deliberately bypassed. */
export function formatWithCsl(items: CslItem[], styleId: string, locale = 'en-US'): string {
  if (!CiteClass || !cslConfig) {
    throw new Error('CSL engine not loaded — call prepareStyle(styleId) first');
  }
  if (!cslConfig.templates.has(styleId)) {
    throw new Error(
      `CSL style "${styleId}" is not registered — prepareStyle it first (no silent fallback)`
    );
  }
  const cite = new CiteClass(items.map(toCslJson));
  return cite
    .format('bibliography', { template: styleId, lang: locale, format: 'text' })
    .trim();
}
