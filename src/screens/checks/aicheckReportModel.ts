// Gaply — the SHARED "what the AI Check report says" layer. Every honesty string
// that is a LITERAL (i.e. NOT carried verbatim on the wire in AiCheckResult)
// lives here ONCE, and every derivation that turns wire data into user-facing
// text (evidence rows, tier labels, the split sentence, the proportion
// explainer) lives here too.
//
// TWO renderers consume this module and MUST agree byte-for-byte:
//   · the on-screen React report — AiCheckReport.tsx
//   · the exported self-contained HTML — aicheckReportHtml.ts (browser → ⌘P → PDF)
//
// Re-typing any of these strings in either renderer is the drift bug this module
// exists to prevent (the "strings describe architecture" class). The export is
// pinned by aicheckReportHtml.vitest.ts: the emitted HTML must contain the
// SAME constants this file exports, sourced by reference, never re-typed.
//
// Pure: no React, no DOM, no CSS — importable by the emitter and its unit tests.
import {
  AiCheckAnalysis,
  AiCheckPassage,
  AiCheckResult,
  AiCheckSection,
  BiasTier,
  PerplexitySignal,
  SignalEvidence,
  SignalLevel,
} from './agentTypes';

// ── manuscript segmentation (text-anchored, Rust byte offsets ≠ JS indices) ──

export type Segment =
  | { kind: 'plain'; text: string }
  | { kind: 'passage'; passage: AiCheckPassage; index: number };

export interface RenderedSection {
  section: AiCheckSection;
  segments: Segment[];
}

/** Assign each passage to the first section whose text contains it (kind must
 *  match), and split section text into plain/highlighted segments. Matching is
 *  TEXT-anchored (indexOf), not offset-anchored: the wire's start/end are Rust
 *  byte offsets, which disagree with JS UTF-16 indices on non-ASCII text. */
export function segmentSections(result: AiCheckResult): {
  rendered: RenderedSection[];
  unplaced: AiCheckPassage[];
} {
  const used = new Set<number>();
  const rendered = result.sections.map((section) => {
    const segments: Segment[] = [];
    let cursor = 0;
    const candidates = result.analysis.passages
      .map((passage, index) => ({ passage, index }))
      .filter(({ passage, index }) => !used.has(index) && passage.section === section.kind)
      .sort((a, b) => a.passage.start_char - b.passage.start_char);
    for (const { passage, index } of candidates) {
      const at = section.text.indexOf(passage.text, cursor);
      if (at === -1) continue;
      used.add(index);
      if (at > cursor) segments.push({ kind: 'plain', text: section.text.slice(cursor, at) });
      segments.push({ kind: 'passage', passage, index });
      cursor = at + passage.text.length;
    }
    if (cursor < section.text.length) {
      segments.push({ kind: 'plain', text: section.text.slice(cursor) });
    }
    return { section, segments };
  });
  const unplaced = result.analysis.passages.filter((_, i) => !used.has(i));
  return { rendered, unplaced };
}

// ── the un-strippable honesty literals (shared source of truth) ──

/** Empty-guard backstop for the un-strippable AI disclaimer. The wire normally
 *  carries `analysis.disclaimer` VERBATIM from the Rust core's `AI_DISCLAIMER`
 *  (ai_detect.rs); this hardcoded copy renders ONLY if that field ever regresses
 *  to empty, so the signal-not-verdict disclaimer can never silently vanish. */
export const AI_DISCLAIMER_FALLBACK =
  'STATISTICAL SIGNAL ONLY — NOT proof of AI authorship. Perplexity and ' +
  'burstiness are probabilistic indicators with high false-positive and ' +
  'false-negative rates. They vary by domain, genre, and individual writing ' +
  'style, can be deliberately evaded, and are unreliable on short texts, ' +
  'non-native English, and heavily edited writing. These scores must never be ' +
  'used as sole or definitive evidence that text was AI-generated — treat them ' +
  'as one weak input to human judgement.';

/** The disclaimer the report shows: the wire value verbatim, or the fallback if
 *  it ever regresses to empty/whitespace. Both renderers resolve it the same way. */
export function resolveDisclaimer(analysis: AiCheckAnalysis): string {
  return analysis.disclaimer && analysis.disclaimer.trim() ? analysis.disclaimer : AI_DISCLAIMER_FALLBACK;
}

/** The copy that sits next to the % and denies the probability reading. */
export const PROPORTION_EXPLAINER =
  'of the analyzed text shows AI-associated signals. This is a deterministic proportion ' +
  'of flagged text; it is NOT the chance that this document was AI-written, and no such ' +
  'number exists in this report.';

/** The Evidence Summary gate — no combined score, levels are strengths not verdicts. */
export const EVIDENCE_GATE_NOTE =
  'Individual signals: not a verdict, and not combined into a score. A calibrated 0–100 score ' +
  "isn't shown: it requires evaluation Gaply hasn't completed yet. Each level below is the " +
  'STRENGTH of the AI-associated signal.';

/** The report's standing tagline (header badge + footer suffix). */
export const FOOTER_TAGLINE = 'signal, not proof';

/** The two-color legend prose (the colored words "amber"/"red" and any
 *  UI-only tail like "click a highlight" are the renderer's, not shared). */
export const LEGEND = {
  plain: 'Plain text: no AI-associated signal.',
  assessedTail: 'heuristic-only flag (preliminary).',
  flaggedTail: 'deep-verified (re-scored by the on-device deep model).',
  levels: 'All are signal levels, never verdicts.',
};

/** The per-passage findings split sentence (deep-verified vs heuristic-only). */
export function splitSentence(deep: number, total: number, heuristic: number): string {
  return (
    `${deep} of ${total} flagged passages were deep-verified by the on-device model; ` +
    `the remaining ${heuristic} carry heuristic-only flags and are preliminary.`
  );
}

// ── tier + evidence derivations (wire data → user-facing text) ──

export const tierOf = (p: AiCheckPassage): 'flagged' | 'assessed' =>
  p.depth === 'deep_verified' ? 'flagged' : 'assessed';

export const tierLabel = (p: AiCheckPassage): string =>
  p.depth === 'deep_verified' ? 'deep-verified signal' : 'heuristic-only (preliminary)';

export const LEVEL_LABEL: Record<SignalLevel, string> = {
  high: 'High',
  moderate: 'Moderate',
  low: 'Low',
};

/** The left-column label = the measured strength, or the honest non-measured
 *  state ('Unavailable' / 'Not applicable'). */
export function statusLabel(e: SignalEvidence): string {
  if (e.status === 'measured' && e.level) return LEVEL_LABEL[e.level];
  if (e.status === 'unavailable') return 'Unavailable';
  return 'Not applicable';
}

// Bias-tiered groups: factual signals are trusted; stylometric are down-weighted
// (they over-flag non-native English) — the caveat makes that honest, verbally.
export const EVIDENCE_GROUPS: Array<{ tier: BiasTier; label: string; caveat?: string }> = [
  { tier: 'factual', label: 'VERIFIABLE (world-checkable)' },
  { tier: 'structural', label: 'STRUCTURAL' },
  { tier: 'stylometric', label: 'STYLE-BASED', caveat: 'less reliable; can over-flag non-native English writing' },
];

/** The Stage-1 perplexity placement as an Evidence row (it's a separate wire
 *  field, not part of document_score.evidence). */
export function perplexityRow(a: AiCheckAnalysis): SignalEvidence | null {
  if (!a.lm_perplexity_signal) return null;
  const map: Record<PerplexitySignal, { level: SignalLevel; detail: string }> = {
    unusually_predictable: { level: 'high', detail: 'unusually predictable for human academic writing' },
    below_human_median: { level: 'moderate', detail: 'below the human academic median' },
    within_or_above_human: { level: 'low', detail: 'within/above the human academic range' },
  };
  const m = map[a.lm_perplexity_signal];
  return { signal: 'Language-model perplexity', status: 'measured', level: m.level, bias_tier: 'stylometric', detail: m.detail };
}

/** All Evidence Summary rows in order: the perplexity placement (if any) then
 *  the document_score evidence. */
export function evidenceRows(a: AiCheckAnalysis): SignalEvidence[] {
  const pRow = perplexityRow(a);
  return [...(pRow ? [pRow] : []), ...(a.document_score?.evidence ?? [])];
}
