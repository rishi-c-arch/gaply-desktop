import type { Cite } from '@citation-js/core';
import type { CslLike } from './citationFetchers';

export type CiteTemplate = 'apa' | 'vancouver' | 'harvard1';
export type CiteLocale = 'en-US' | 'es-ES' | 'de-DE' | 'fr-FR' | 'nl-NL';

let loadPromise: Promise<void> | null = null;

export function ensureCitationPlugins(): Promise<void> {
  if (!loadPromise) {
    loadPromise = (async () => {
      await import('@citation-js/plugin-bibtex');
      await import('@citation-js/plugin-ris');
      await import('@citation-js/plugin-csl');
      await import('@citation-js/plugin-doi');
    })();
  }
  return loadPromise;
}

export async function getCiteClass(): Promise<typeof Cite> {
  await ensureCitationPlugins();
  const mod = await import('@citation-js/core');
  return mod.Cite;
}

export async function parseToCite(input: string, formatHint?: string): Promise<Cite> {
  const CiteCtor = await getCiteClass();
  const t = input.trim();
  if (!t) throw new Error('Empty input');
  if (formatHint === 'json' || (t.startsWith('{') && t.includes('"type"'))) {
    const obj = JSON.parse(t) as CslLike | CslLike[];
    const arr = Array.isArray(obj) ? obj : [obj];
    return new CiteCtor(arr);
  }
  return new CiteCtor(t);
}

export async function fromCslItems(items: CslLike[]): Promise<Cite> {
  const CiteCtor = await getCiteClass();
  return new CiteCtor(JSON.parse(JSON.stringify(items)));
}

export async function formatBibliography(
  cite: Cite,
  template: CiteTemplate,
  lang: CiteLocale,
  asHtml: boolean,
): Promise<string> {
  return cite.format('bibliography', {
    format: asHtml ? 'html' : 'text',
    template,
    lang,
  }) as string;
}

export async function formatCitation(
  cite: Cite,
  entry?: string,
): Promise<string> {
  const opts = entry ? { entry } : {};
  return cite.format('citation', opts) as string;
}

export async function exportBibtex(cite: Cite): Promise<string> {
  return cite.format('bibtex') as string;
}

export async function exportRis(cite: Cite): Promise<string> {
  return cite.format('ris') as string;
}

export async function exportData(cite: Cite): Promise<string> {
  return cite.format('data', { type: 'string', style: 'csl' }) as string;
}
