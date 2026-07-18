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
import { AiCheckAnalysis, AiCheckPassage, AiCheckResult, SignalEvidence } from './agentTypes';
import {
  EVIDENCE_GATE_NOTE,
  EVIDENCE_GROUPS,
  FOOTER_TAGLINE,
  LEGEND,
  PROPORTION_EXPLAINER,
  evidenceRows,
  resolveDisclaimer,
  segmentSections,
  splitSentence,
  statusLabel,
  tierLabel,
  tierOf,
} from './aicheckReportModel';

// segmentSections now lives in the shared model (so the HTML emitter can reuse it
// without pulling in React); re-exported here for the report tests that import it.
export { segmentSections };

/** One passage's evidence — the badges, the tier + passage cautions (VERBATIM),
 *  gate flags, and the per-sentence perplexity table. Shared by the click
 *  inspector and each row of the Per-passage findings list; `prefix` keeps their
 *  testids distinct (inspector-* vs finding-N-*). */
const PassageEvidence: React.FC<{ passage: AiCheckPassage; prefix?: string }> = ({ passage, prefix = 'inspector' }) => (
  <>
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
      <Badge status={tierOf(passage)}>{tierLabel(passage)}</Badge>
      <Badge status="neutral">strength: {passage.strength}</Badge>
      <span className="gds-mono" style={{ fontSize: 12, color: 'var(--g-text-3)' }}>
        mean perplexity {passage.mean_perplexity.toFixed(1)} · burstiness {passage.burstiness.toFixed(1)}
      </span>
    </div>
    {/* the tier + passage cautions, verbatim */}
    <p className="gds-jc__disclaimer" data-testid={`${prefix}-depth-note`}>{passage.depth_note}</p>
    <p className="gds-jc__disclaimer" data-testid={`${prefix}-uncertainty`}>{passage.uncertainty}</p>
    <p className="gds-jc__disclaimer" data-testid={`${prefix}-category-note`}>{passage.category_note}</p>
    {passage.gate_flags.length > 0 && (
      <ul style={{ fontSize: 12, color: 'var(--g-text-3)' }} data-testid={`${prefix}-gate-flags`}>
        {passage.gate_flags.map((g, i) => (
          <li key={i}>{g}</li>
        ))}
      </ul>
    )}
    <table style={{ width: '100%', fontSize: 12, marginTop: 8, borderCollapse: 'collapse' }} data-testid={`${prefix}-sentences`}>
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
  </>
);

/** The Per-passage findings list — every flagged passage numbered, with its tier,
 *  text, evidence, and cautions, so a supervisor can reference "passage 7". The
 *  print report's §5, and the SINGLE evidence view on screen: a manuscript
 *  highlight click scrolls to + emphasizes the matching finding here (no separate
 *  inspector). Numbering matches the highlight index (`aicheck-highlight-N` → N+1). */
const PerPassageFindings: React.FC<{ passages: AiCheckPassage[]; selected: AiCheckPassage | null }> = ({
  passages,
  selected,
}) => {
  if (passages.length === 0) return null;
  const total = passages.length;
  const deep = passages.filter((p) => p.depth === 'deep_verified').length;
  const heuristic = total - deep;
  return (
    <Card title="Per-passage findings" data-testid="per-passage-findings">
      <p className="gds-jc__disclaimer" data-testid="findings-split">
        {splitSentence(deep, total, heuristic)}
      </p>
      {passages.map((p, i) => (
        <div
          key={i}
          id={`finding-${i}`}
          className={`gds-finding${p === selected ? ' gds-finding--current' : ''}`}
          data-testid={`finding-${i}`}
          aria-current={p === selected}
        >
          <p className="gds-finding__num">Passage {i + 1}</p>
          <blockquote className="gds-finding__text">{p.text}</blockquote>
          <PassageEvidence passage={p} prefix={`finding-${i}`} />
        </div>
      ))}
    </Card>
  );
};

/** The level chip's strength/muted class — measured levels carry their strength,
 *  Unavailable / Not-applicable read muted (but the row stays legible). This is
 *  CSS-specific (on-screen only), so it stays in the component, not the model. */
function levelClass(e: SignalEvidence): string {
  if (e.status === 'measured' && e.level) return `aic-ev__level--${e.level}`;
  return 'aic-ev__level--muted';
}

/** The multi-signal Evidence Summary (Phase 1). Qualitative levels only — the
 *  honesty gate keeps ANY 0-100 score off the screen (document_score.value is
 *  null until Phase 3). Rows are grouped by bias tier; the perplexity row (from
 *  provisional norms) carries the in-row "preliminary" footnote. */
const EvidenceSummary: React.FC<{ analysis: AiCheckAnalysis }> = ({ analysis }) => {
  const rows = evidenceRows(analysis);
  if (rows.length === 0) return null;
  const provisional = analysis.norms_provisional;
  return (
    <Card title="Evidence Summary" data-testid="evidence-summary">
      <p className="aic-ev__gate">{EVIDENCE_GATE_NOTE}</p>
      {EVIDENCE_GROUPS.map((g) => {
        const groupRows = rows.filter((r) => r.bias_tier === g.tier);
        if (groupRows.length === 0) return null;
        return (
          <div key={g.tier} className={`aic-ev__band aic-ev__band--${g.tier}`} data-testid={`evidence-group-${g.tier}`}>
            <div className="aic-ev__bandhead">
              <span className="aic-ev__bandlabel">{g.label}</span>
              {g.caveat && <span className="aic-ev__caveat">({g.caveat})</span>}
            </div>
            {groupRows.map((r, i) => (
              <div key={i} className="aic-ev__row" data-testid="evidence-row">
                <span className={`aic-ev__level ${levelClass(r)}`}>{statusLabel(r)}</span>
                <span>
                  <span className="aic-ev__signal">{r.signal}</span>:{' '}
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
          ¹ preliminary: norms not yet held-out-evaluated
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

  // A manuscript highlight click emphasizes + scrolls to the matching numbered
  // finding (the single evidence view). scrolls even on re-click of the same
  // passage; honors prefers-reduced-motion; guarded for jsdom (no scrollIntoView).
  const emphasizeFinding = (passage: AiCheckPassage) => {
    setSelected(passage);
    const idx = analysis.passages.indexOf(passage);
    if (idx < 0) return;
    const el = typeof document !== 'undefined' ? document.getElementById(`finding-${idx}`) : null;
    if (el && typeof el.scrollIntoView === 'function') {
      const reduce = typeof window !== 'undefined' && !!window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      el.scrollIntoView({ behavior: reduce ? 'auto' : 'smooth', block: 'center' });
    }
  };

  return (
    <div className="gds-report" data-testid="aicheck-report" style={{ display: 'grid', gap: 12 }}>
      {/* THE un-strippable caution — first thing on the report, verbatim */}
      <section data-testid="ai-caution" className="aic-callout">
        <Badge status="flagged">{FOOTER_TAGLINE}</Badge>
        <p className="gds-report__disclaimer" style={{ marginTop: 6 }} data-testid="ai-disclaimer">
          {resolveDisclaimer(analysis)}
        </p>
      </section>

      {/* language honesty (Set 5): non-English downgrades the whole report */}
      {lang.calibration_reliable ? (
        <p className="gds-jc__disclaimer" data-testid="language-note">
          {lang.note}
        </p>
      ) : (
        <section data-testid="language-downgrade" className="aic-callout">
          <Badge status="flagged">non-English text: low confidence</Badge>
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
            {PROPORTION_EXPLAINER}
          </span>
        </div>
        <p className="gds-jc__disclaimer" style={{ marginTop: 8 }} data-testid="coverage-note">
          {analysis.coverage_note}
        </p>
      </Card>

      <EvidenceSummary analysis={analysis} />

      {/* two-way legend */}
      <p style={{ fontSize: 12, color: 'var(--g-text-3)', margin: 0 }} data-testid="aicheck-legend">
        {LEGEND.plain}{' '}
        <span className="gds-highlight" data-tier="assessed" style={{ cursor: 'default' }}>
          amber
        </span>
        : {LEGEND.assessedTail}{' '}
        <span className="gds-highlight" data-tier="flagged" style={{ cursor: 'default' }}>
          red
        </span>
        : {LEGEND.flaggedTail} {LEGEND.levels} Click a highlight for its evidence.
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
                    onClick={() => emphasizeFinding(seg.passage)}
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
              onClick={() => emphasizeFinding(p)}
              style={{ display: 'block', textAlign: 'left', marginBottom: 6 }}
            >
              {p.text}
            </button>
          ))}
        </Card>
      )}

      {/* the full, referenceable findings list (print §5; the single on-screen
          evidence view — a highlight click scrolls to + emphasizes its finding) */}
      <PerPassageFindings passages={analysis.passages} selected={selected} />


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
        {analysis.deep_model ?? 'not available; all flags heuristic-only'}
      </p>
    </div>
  );
};

export default AiCheckReport;
