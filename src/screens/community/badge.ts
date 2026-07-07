// Gaply — the integrity badge pipeline. Before a post publishes, the local
// AI-Detection + Plagiarism logic runs on the post text and produces an
// IMMOVABLE badge. Deterministic (same text → same badge), computed
// client-side (offline), and stored with the post. A `BadgeScanner` seam lets
// a real Rust text-scan (detect_ai + plagiarism-on-text) drop in later; the
// default `classifyPost` is a self-contained local classifier.
import { IntegrityBadge } from '../../services/supabase/types';

export type { IntegrityBadge };

export interface BadgeResult {
  badge: IntegrityBadge;
  detail: string;
  /** For 📋 COPIED: the matched source (a post id or URL). */
  source?: string;
}

export const BADGE_META: Record<IntegrityBadge, { icon: string; label: string; status: 'certain' | 'assessed' | 'flagged' }> = {
  human_written: { icon: '🟢', label: 'HUMAN WRITTEN', status: 'certain' },
  ai_assisted: { icon: '🟡', label: 'AI ASSISTED', status: 'assessed' },
  ai_generated: { icon: '🔴', label: 'AI GENERATED', status: 'flagged' },
  copied: { icon: '📋', label: 'COPIED', status: 'flagged' },
};

export interface ExistingPost {
  id: string;
  message: string;
  author?: string;
}
export interface OnlineSource {
  url: string;
  text: string;
}

const AI_CLICHES = [
  'in conclusion',
  'furthermore',
  'moreover',
  'it is important to note',
  'it is worth noting',
  'delve into',
  'delving into',
  'tapestry',
  'leverage',
  'navigating the',
  'in the realm of',
  'plays a crucial role',
  'a testament to',
  'ever-evolving',
  'in today',
  'firstly',
  'additionally',
];

function sentences(text: string): string[] {
  return text.split(/[.!?]+/).map((s) => s.trim()).filter((s) => s.split(/\s+/).length > 1);
}

/** Coefficient of variation of sentence word-counts. Low CV = uniform, AI-like. */
function burstiness(text: string): number {
  const lens = sentences(text).map((s) => s.split(/\s+/).length);
  if (lens.length < 2) return 1; // too short to judge — treat as bursty (human)
  const mean = lens.reduce((a, b) => a + b, 0) / lens.length;
  if (mean === 0) return 1;
  const variance = lens.reduce((a, b) => a + (b - mean) ** 2, 0) / lens.length;
  return Math.sqrt(variance) / mean;
}

/** Word 5-gram shingle set. */
function shingles(text: string, n = 5): Set<string> {
  const words = text.toLowerCase().replace(/[^a-z0-9\s]/g, ' ').split(/\s+/).filter(Boolean);
  const out = new Set<string>();
  for (let i = 0; i + n <= words.length; i++) out.add(words.slice(i, i + n).join(' '));
  return out;
}

/** Jaccard-ish containment: fraction of A's shingles also in B. */
function containment(a: Set<string>, b: Set<string>): number {
  if (a.size === 0) return 0;
  let hit = 0;
  a.forEach((s) => {
    if (b.has(s)) hit++;
  });
  return hit / a.size;
}

const COPY_THRESHOLD = 0.5;

/** The default local badge classifier. Plagiarism takes precedence over AI
 *  signal (a copied post is 📋 regardless of how it reads). */
export function classifyPost(
  text: string,
  existing: ExistingPost[] = [],
  online: OnlineSource[] = []
): BadgeResult {
  const shTxt = shingles(text);

  // (1) plagiarism — compare against other community posts, then online.
  let best: { source: string; label: string; score: number } | null = null;
  for (const p of existing) {
    const score = containment(shTxt, shingles(p.message));
    if (score >= COPY_THRESHOLD && (!best || score > best.score)) {
      best = { source: p.id, label: `community post${p.author ? ` by ${p.author}` : ''}`, score };
    }
  }
  for (const s of online) {
    const score = containment(shTxt, shingles(s.text));
    if (score >= COPY_THRESHOLD && (!best || score > best.score)) {
      best = { source: s.url, label: s.url, score };
    }
  }
  if (best) {
    return {
      badge: 'copied',
      detail: `${Math.round(best.score * 100)}% overlap with ${best.label}`,
      source: best.source,
    };
  }

  // (2) AI signal — cliché markers + low burstiness.
  const lower = text.toLowerCase();
  const markers = AI_CLICHES.filter((c) => lower.includes(c)).length;
  const cv = burstiness(text);
  const uniformity = cv < 0.2 ? 1 : cv < 0.35 ? 0.6 : 0.15;
  const markerScore = Math.min(1, markers / 3);
  const aiScore = 0.5 * markerScore + 0.5 * uniformity;

  if (aiScore >= 0.7) {
    return { badge: 'ai_generated', detail: `${markers} AI marker(s), low burstiness (cv=${cv.toFixed(2)})` };
  }
  if (aiScore >= 0.4) {
    return { badge: 'ai_assisted', detail: `some AI-like patterns (cv=${cv.toFixed(2)})` };
  }
  return { badge: 'human_written', detail: `natural variation (cv=${cv.toFixed(2)})` };
}

/** Seam for a future Rust text-scan implementation. */
export interface BadgeScanner {
  scan(text: string, existing: ExistingPost[], online?: OnlineSource[]): Promise<BadgeResult>;
}

/** Default scanner wraps the local classifier (async for a uniform interface). */
export const localBadgeScanner: BadgeScanner = {
  async scan(text, existing, online = []) {
    return classifyPost(text, existing, online);
  },
};
