/** Static hero for Research Series guides (copy of Desktop `Gemini_Generated_Image_3bescf3bescf3bes.png`). */
export function researchSeriesHeroSphereUrl(): string {
  const raw = typeof process !== 'undefined' && process.env.PUBLIC_URL != null ? process.env.PUBLIC_URL : '';
  const base = String(raw).replace(/\/$/, '');
  return `${base}/guides/ethical-ai-hero-sphere.png`;
}
