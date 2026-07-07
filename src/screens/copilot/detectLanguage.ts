// Gaply — lightweight language detection to tag the turn (the cloud LLM does the
// real translation; this just tells it which language to answer in and lets the
// UI render a hint). Script-based first, then a few stopword heuristics. Returns
// an ISO-639-1 code; defaults to 'en'.
export function detectLanguage(text: string): string {
  const t = text.trim();
  if (!t) return 'en';

  // script blocks
  if (/[ऀ-ॿ]/.test(t)) return 'hi'; // Devanagari (Hindi/Marathi)
  if (/[؀-ۿ]/.test(t)) return 'ar'; // Arabic
  if (/[一-鿿]/.test(t)) return 'zh'; // CJK
  if (/[぀-ヿ]/.test(t)) return 'ja'; // Japanese kana
  if (/[가-힯]/.test(t)) return 'ko'; // Korean
  if (/[Ѐ-ӿ]/.test(t)) return 'ru'; // Cyrillic

  const lower = t.toLowerCase();
  // a few Latin-script stopword hints
  if (/\b(por qué|cómo|cuál|mi|artículo|revista|escribe|resumen)\b/.test(lower)) return 'es';
  if (/\b(pourquoi|comment|mon|article|revue|écris|résumé)\b/.test(lower)) return 'fr';
  if (/\b(warum|wie|mein|artikel|zeitschrift|schreibe)\b/.test(lower)) return 'de';
  return 'en';
}

export const LANGUAGE_NAME: Record<string, string> = {
  en: 'English',
  hi: 'Hindi',
  es: 'Spanish',
  fr: 'French',
  de: 'German',
  ar: 'Arabic',
  zh: 'Chinese',
  ja: 'Japanese',
  ko: 'Korean',
  ru: 'Russian',
};
