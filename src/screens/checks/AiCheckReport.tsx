// Gaply — AI Check two-way report (Set 5). Human-written vs AI-associated,
// rendered honestly:
//   · the % is the wire's deterministic ai_signal_proportion — a proportion
//     of flagged text, never presented as a probability of authorship;
//   · 2-color in-document highlighting (ReportViewerPage's gds-highlight
//     pattern): plain text = no AI-associated signal; amber = heuristic-only
//     (preliminary); red = deep-verified. Signal LEVELS, never verdicts;
//   · every caution (disclaimer, coverage_note, language note, per-passage
//     notes, paraphrase_caution) is surfaced VERBATIM from the Rust core —
//     the frontend never rewrites or omits them;
//   · the paraphrase lane says it is UNAVAILABLE (the Set-4 live-probe
//     decision) — no third category is ever displayed.
import React, { useMemo, useState } from 'react';
import { Badge, Card } from '../../design-system';
import '../report/report.css';
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

/** Empty-guard backstop for the un-strippable AI disclaimer. The wire normally
 *  carries `analysis.disclaimer` VERBATIM from the Rust core's `AI_DISCLAIMER`
 *  (ai_detect.rs); this hardcoded copy renders ONLY if that field ever regresses
 *  to empty, so the signal-not-verdict disclaimer can never silently vanish. */
const AI_DISCLAIMER_FALLBACK =
  'STATISTICAL SIGNAL ONLY — NOT proof of AI authorship. Perplexity and ' +
  'burstiness are probabilistic indicators with high false-positive and ' +
  'false-negative rates. They vary by domain, genre, and individual writing ' +
  'style, can be deliberately evaded, and are unreliable on short texts, ' +
  'non-native English, and heavily edited writing. These scores must never be ' +
  'used as sole or definitive evidence that text was AI-generated — treat them ' +
  'as one weak input to human judgement.';

type Segment =
  | { kind: 'plain'; text: string }
  | { kind: 'passage'; passage: AiCheckPassage; index: number };

interface RenderedSection {
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

const tierOf = (p: AiCheckPassage) => (p.depth === 'deep_verified' ? 'flagged' : 'assessed');
const tierLabel = (p: AiCheckPassage) =>
  p.depth === 'deep_verified' ? 'deep-verified signal' : 'heuristic-only (preliminary)';

const PassageInspector: React.FC<{ passage: AiCheckPassage }> = ({ passage }) => (
  <div data-testid="passage-inspector">
    <Card title="Passage evidence">
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <Badge status={tierOf(passage)}>{tierLabel(passage)}</Badge>
        <Badge status="neutral">strength: {passage.strength}</Badge>
        <span className="gds-mono" style={{ fontSize: 12, color: 'var(--g-text-3)' }}>
          mean perplexity {passage.mean_perplexity.toFixed(1)} · burstiness{' '}
          {passage.burstiness.toFixed(1)}
        </span>
      </div>
      {/* the tier + passage cautions, verbatim */}
      <p className="gds-jc__disclaimer" data-testid="inspector-depth-note">{passage.depth_note}</p>
      <p className="gds-jc__disclaimer" data-testid="inspector-uncertainty">{passage.uncertainty}</p>
      <p className="gds-jc__disclaimer" data-testid="inspector-category-note">{passage.category_note}</p>
      {passage.gate_flags.length > 0 && (
        <ul style={{ fontSize: 12, color: 'var(--g-text-3)' }} data-testid="inspector-gate-flags">
          {passage.gate_flags.map((g, i) => (
            <li key={i}>{g}</li>
          ))}
        </ul>
      )}
      <table style={{ width: '100%', fontSize: 12, marginTop: 8, borderCollapse: 'collapse' }} data-testid="inspector-sentences">
        <thead>
          <tr style={{ textAlign: 'left', color: 'var(--g-text-3)' }}>
            <th style={{ paddingRight: 8 }}>sentence</th>
            <th>perplexity</th>
          </tr>
        </thead>
        <tbody>
          {passage.sentences.map((s, i) => (
            <tr key={i}>
              <td style={{ paddingRight: 8 }}>{s.text}</td>
              <td className="gds-mono">{s.perplexity.toFixed(1)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  </div>
);

const LEVEL_LABEL: Record<SignalLevel, string> = {
  high: 'High',
  moderate: 'Moderate',
  low: 'Low',
};

/** The left-column label = the measured strength, or the honest non-measured
 *  state ('Unavailable' / 'Not applicable'). */
function statusLabel(e: SignalEvidence): string {
  if (e.status === 'measured' && e.level) return LEVEL_LABEL[e.level];
  if (e.status === 'unavailable') return 'Unavailable';
  return 'Not applicable';
}

/** The level chip's strength/muted class — measured levels carry their strength,
 *  Unavailable / Not-applicable read muted (but the row stays legible). */
function levelClass(e: SignalEvidence): string {
  if (e.status === 'measured' && e.level) return `aic-ev__level--${e.level}`;
  return 'aic-ev__level--muted';
}

// Bias-tiered groups: factual signals are trusted; stylometric are down-weighted
// (they over-flag non-native English) — the caveat makes that honest, verbally.
const EVIDENCE_GROUPS: Array<{ tier: BiasTier; label: string; caveat?: string }> = [
  { tier: 'factual', label: 'VERIFIABLE (world-checkable)' },
  { tier: 'structural', label: 'STRUCTURAL' },
  { tier: 'stylometric', label: 'STYLE-BASED', caveat: 'less reliable; can over-flag non-native English writing' },
];

/** The Stage-1 perplexity placement as an Evidence row (it's a separate wire
 *  field, not part of document_score.evidence). */
function perplexityRow(a: AiCheckAnalysis): SignalEvidence | null {
  if (!a.lm_perplexity_signal) return null;
  const map: Record<PerplexitySignal, { level: SignalLevel; detail: string }> = {
    unusually_predictable: { level: 'high', detail: 'unusually predictable for human academic writing' },
    below_human_median: { level: 'moderate', detail: 'below the human academic median' },
    within_or_above_human: { level: 'low', detail: 'within/above the human academic range' },
  };
  const m = map[a.lm_perplexity_signal];
  return { signal: 'Language-model perplexity', status: 'measured', level: m.level, bias_tier: 'stylometric', detail: m.detail };
}

/** The multi-signal Evidence Summary (Phase 1). Qualitative levels only — the
 *  honesty gate keeps ANY 0-100 score off the screen (document_score.value is
 *  null until Phase 3). Rows are grouped by bias tier; the perplexity row (from
 *  provisional norms) carries the in-row "preliminary" footnote. */
const EvidenceSummary: React.FC<{ analysis: AiCheckAnalysis }> = ({ analysis }) => {
  const pRow = perplexityRow(analysis);
  const rows: SignalEvidence[] = [...(pRow ? [pRow] : []), ...(analysis.document_score?.evidence ?? [])];
  if (rows.length === 0) return null;
  const provisional = analysis.norms_provisional;
  return (
    <Card title="Evidence Summary" data-testid="evidence-summary">
      <p className="aic-ev__gate">
        Individual signals — not a verdict, and not combined into a score. A calibrated 0–100 score
        isn't shown: it requires evaluation Gaply hasn't completed yet. Each level below is the
        STRENGTH of the AI-associated signal.
      </p>
      {EVIDENCE_GROUPS.map((g) => {
        const groupRows = rows.filter((r) => r.bias_tier === g.tier);
        if (groupRows.length === 0) return null;
        return (
          <div key={g.tier} className={`aic-ev__band aic-ev__band--${g.tier}`} data-testid={`evidence-group-${g.tier}`}>
            <div className="aic-ev__bandhead">
              <span className="aic-ev__bandlabel">{g.label}</span>
              {g.caveat && <span className="aic-ev__caveat">— {g.caveat}</span>}
            </div>
            {groupRows.map((r, i) => (
              <div key={i} className="aic-ev__row" data-testid="evidence-row">
                <span className={`aic-ev__level ${levelClass(r)}`}>{statusLabel(r)}</span>
                <span>
                  <span className="aic-ev__signal">{r.signal}</span> —{' '}
                  <span className="aic-ev__detail">{r.detail}</span>
                  {(r.signal === 'Language-model perplexity' || r.signal === 'Deep-verifier perplexity') &&
                    provisional && <sup>¹</sup>}
                </span>
              </div>
            ))}
          </div>
        );
      })}
      {provisional && (
        <p className="aic-ev__footnote" data-testid="provisional-footnote">
          ¹ preliminary — norms not yet held-out-evaluated
        </p>
      )}
    </Card>
  );
};

export interface AiCheckReportProps {
  result: AiCheckResult;
}

const AiCheckReport: React.FC<AiCheckReportProps> = ({ result }) => {
  const { analysis } = result;
  const [selected, setSelected] = useState<AiCheckPassage | null>(null);
  const { rendered, unplaced } = useMemo(() => segmentSections(result), [result]);
  const lang = analysis.language;

  return (
    <div className="gds-report" data-testid="aicheck-report" style={{ display: 'grid', gap: 12 }}>
      {/* THE un-strippable caution — first thing on the report, verbatim */}
      <section data-testid="ai-caution" className="aic-callout">
        <Badge status="flagged">signal, not proof</Badge>
        <p className="gds-report__disclaimer" style={{ marginTop: 6 }} data-testid="ai-disclaimer">
          {analysis.disclaimer && analysis.disclaimer.trim()
            ? analysis.disclaimer
            : AI_DISCLAIMER_FALLBACK}
        </p>
      </section>

      {/* language honesty (Set 5): non-English downgrades the whole report */}
      {lang.calibration_reliable ? (
        <p className="gds-jc__disclaimer" data-testid="language-note">
          {lang.note}
        </p>
      ) : (
        <section data-testid="language-downgrade" className="aic-callout">
          <Badge status="flagged">non-English text — low confidence</Badge>
          <p className="gds-jc__disclaimer" style={{ marginTop: 6 }}>
            {lang.note}
          </p>
        </section>
      )}

      {/* the honest % — a proportion, and it says so next to the number */}
      <Card title="AI-associated signal">
        <div style={{ display: 'flex', gap: 16, alignItems: 'baseline', flexWrap: 'wrap' }}>
          <span style={{ fontSize: 40, fontWeight: 700 }} data-testid="ai-proportion">
            {(analysis.ai_signal_proportion * 100).toFixed(1)}%
          </span>
          <span style={{ color: 'var(--g-text-3)', fontSize: 13, maxWidth: 560 }}>
            of the analyzed text shows AI-associated signals. This is a deterministic proportion
            of flagged text — it is NOT the chance that this document was AI-written, and no such
            number exists in this report.
          </span>
        </div>
        <p className="gds-jc__disclaimer" style={{ marginTop: 8 }} data-testid="coverage-note">
          {analysis.coverage_note}
        </p>
      </Card>

      <EvidenceSummary analysis={analysis} />

      {/* two-way legend */}
      <p style={{ fontSize: 12, color: 'var(--g-text-3)', margin: 0 }} data-testid="aicheck-legend">
        Plain text: no AI-associated signal.{' '}
        <span className="gds-highlight" data-tier="assessed" style={{ cursor: 'default' }}>
          amber
        </span>
        : heuristic-only flag (preliminary).{' '}
        <span className="gds-highlight" data-tier="flagged" style={{ cursor: 'default' }}>
          red
        </span>
        : deep-verified against this document's own baseline. All are signal levels — never
        verdicts. Click a highlight for its evidence.
      </p>

      {/* the manuscript, 2-color highlighted */}
      <div className="gds-manuscript" data-testid="aicheck-manuscript">
        {rendered.map(({ section, segments }, si) => (
          <section className="gds-ms-section" key={si}>
            {section.heading && <h3 className="gds-ms-section__heading">{section.heading}</h3>}
            <p className="gds-ms-section__body">
              {segments.map((seg, i) =>
                seg.kind === 'plain' ? (
                  <React.Fragment key={i}>{seg.text}</React.Fragment>
                ) : (
                  <button
                    key={i}
                    className="gds-highlight"
                    data-tier={tierOf(seg.passage)}
                    data-testid={`aicheck-highlight-${seg.index}`}
                    aria-current={selected === seg.passage}
                    onClick={() => setSelected(seg.passage)}
                    title="click to see the evidence"
                  >
                    {seg.passage.text}
                  </button>
                )
              )}
            </p>
          </section>
        ))}
      </div>

      {/* defensive: passages the document view couldn't anchor still surface */}
      {unplaced.length > 0 && (
        <Card title="Flagged passages not shown above">
          {unplaced.map((p, i) => (
            <button
              key={i}
              className="gds-highlight"
              data-tier={tierOf(p)}
              data-testid={`aicheck-unplaced-${i}`}
              onClick={() => setSelected(p)}
              style={{ display: 'block', textAlign: 'left', marginBottom: 6 }}
            >
              {p.text}
            </button>
          ))}
        </Card>
      )}

      {selected && <PassageInspector passage={selected} />}

      {/* the paraphrase lane — HONESTLY unavailable, never a fake category */}
      <section data-testid="paraphrase-unavailable" className="aic-callout aic-callout--muted">
        <h4 style={{ margin: '0 0 6px' }}>
          <Badge status="neutral">AI-generated vs AI-paraphrased</Badge>
        </h4>
        <p className="gds-jc__disclaimer" data-testid="classification-note">
          {analysis.classification_note}
        </p>
        <p className="gds-jc__disclaimer" data-testid="paraphrase-caution">
          {analysis.paraphrase_caution}
        </p>
      </section>

      {/* provenance: which models actually ran */}
      <p className="gds-mono" style={{ fontSize: 11, color: 'var(--g-text-3)', margin: 0 }} data-testid="aicheck-models">
        fast: {analysis.fast_model} · deep:{' '}
        {analysis.deep_model ?? 'not available — all flags heuristic-only'}
      </p>
    </div>
  );
};

export default AiCheckReport;
