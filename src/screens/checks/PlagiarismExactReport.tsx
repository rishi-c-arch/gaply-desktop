// Gaply — deterministic exact-match plagiarism report (Set 5). Renders the
// verbatim overlaps you can SEE in both places:
//   · each match is shown SIDE-BY-SIDE — the passage in your document and the
//     same passage in the matched paper — so the match is provable, not a
//     guess. The span `text` from the wire is rendered directly (already the
//     exact slice; no JS offset math on the Rust byte offsets);
//   · the number is the wire's deterministic duplication_ratio, framed as a
//     PROPORTION of overlapping text — never a "plagiarism score" or verdict;
//   · the named-scope disclosure + Turnitin limitation render from required
//     fields, always present (all-clear case too). The component takes only
//     `result`, so nothing is suppressible.
import React, { useState } from 'react';
import { Badge, Card } from '../../design-system';
import '../report/report.css';
import './plagiarism.css';
import { ExactPlagiarismReport, MatchedPassage } from './agentTypes';

const pct = (x: number) => `${(x * 100).toFixed(x >= 0.1 ? 0 : 1)}%`;

const MatchInspector: React.FC<{ match: MatchedPassage }> = ({ match }) => (
  <Card title="Match evidence" data-testid="plag-inspector">
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center', marginBottom: 8 }}>
      <Badge status="flagged">
        {match.match_kind === 'self_repeat' ? 'recycled within your document' : `matches: ${match.source_ref}`}
      </Badge>
      <Badge status="neutral">overlap {pct(match.similarity)}</Badge>
      <span className="gds-mono" style={{ fontSize: 12, color: 'var(--g-text-3)' }}>
        {match.word_count} words
      </span>
    </div>
    <div className="gds-plag-sxs">
      <div className="gds-plag-col" data-testid="inspector-source">
        <span className="gds-plag-col__label">In your document</span>
        <p className="gds-plag-col__text"><mark className="gds-highlight" data-tier="flagged">{match.source.text}</mark></p>
      </div>
      <div className="gds-plag-col" data-testid="inspector-target">
        <span className="gds-plag-col__label">
          {match.match_kind === 'self_repeat' ? 'Elsewhere in your document' : `In "${match.source_ref}"`}
        </span>
        <p className="gds-plag-col__text"><mark className="gds-highlight" data-tier="flagged">{match.matched.text}</mark></p>
      </div>
    </div>
  </Card>
);

const MatchCard: React.FC<{
  match: MatchedPassage;
  index: number;
  selected: boolean;
  onSelect: () => void;
}> = ({ match, index, selected, onSelect }) => (
  <button
    type="button"
    className="gds-plag-match"
    data-testid={`plag-match-${index}`}
    aria-current={selected}
    onClick={onSelect}
  >
    <div className="gds-plag-match__head">
      <span className="gds-plag-match__title">
        {match.match_kind === 'self_repeat' ? 'Recycled within your document' : match.source_ref}
      </span>
      <span className="gds-plag-match__meta">overlap {pct(match.similarity)} · {match.word_count} words</span>
    </div>
    <div className="gds-plag-sxs">
      <div className="gds-plag-col">
        <span className="gds-plag-col__label">In your document</span>
        <p className="gds-plag-col__text" data-testid={`plag-source-${index}`}>
          <mark className="gds-highlight" data-tier="flagged">{match.source.text}</mark>
        </p>
      </div>
      <div className="gds-plag-col">
        <span className="gds-plag-col__label">
          {match.match_kind === 'self_repeat' ? 'Elsewhere in your document' : `In "${match.source_ref}"`}
        </span>
        <p className="gds-plag-col__text" data-testid={`plag-target-${index}`}>
          <mark className="gds-highlight" data-tier="flagged">{match.matched.text}</mark>
        </p>
      </div>
    </div>
  </button>
);

export interface PlagiarismExactReportProps {
  result: ExactPlagiarismReport;
}

const PlagiarismExactReport: React.FC<PlagiarismExactReportProps> = ({ result }) => {
  const [selected, setSelected] = useState<MatchedPassage | null>(null);
  // library matches first (most significant), then self-recycling
  const all = [...result.library_matches, ...result.self_matches];
  const hasMatches = all.length > 0;

  return (
    <div className="gds-report gds-plag-report" data-testid="plag-report" style={{ display: 'grid', gap: 12 }}>
      {/* un-strippable disclosure — first, verbatim, prominent */}
      <section
        data-testid="plag-caution"
        style={{ border: '1px solid var(--g-flagged, #e93)', borderRadius: 8, padding: '10px 12px' }}
      >
        <Badge status="flagged">not a Turnitin replacement</Badge>
        <p className="gds-report__disclaimer" style={{ marginTop: 6 }} data-testid="plag-disclosure">
          {result.disclosure}
        </p>
      </section>

      {/* the honest number — a proportion of overlapping text, never a score */}
      <Card title="Text overlap">
        <div style={{ display: 'flex', gap: 16, alignItems: 'baseline', flexWrap: 'wrap' }}>
          <span style={{ fontSize: 40, fontWeight: 700 }} data-testid="plag-duplication">
            {pct(result.duplication_ratio)}
          </span>
          <span style={{ color: 'var(--g-text-3)', fontSize: 13, maxWidth: 560 }}>
            of your document's text overlaps with what you compared against (your library
            {result.self_matches.length > 0 ? ' and itself' : ''}). This is the proportion of
            matched text — NOT a plagiarism score, and never a judgement.
          </span>
        </div>
        {result.compared_against.length > 0 && (
          <p className="gds-jc__disclaimer" style={{ marginTop: 8 }} data-testid="plag-scope">
            Compared against your {result.compared_against.length} paper(s):{' '}
            {result.compared_against.join('; ')}.
          </p>
        )}
      </Card>

      {!hasMatches ? (
        <Card title="No exact overlaps" data-testid="plag-empty">
          <p style={{ color: 'var(--g-text-2)', fontSize: 14, margin: 0 }}>
            No exact text overlaps were found against the documents above. Remember: the absence
            of matches here does NOT mean the text is original — only these documents were checked.
          </p>
        </Card>
      ) : (
        <>
          <p style={{ fontSize: 12, color: 'var(--g-text-3)', margin: 0 }} data-testid="plag-legend">
            Each match shows the same{' '}
            <span className="gds-highlight" data-tier="flagged" style={{ cursor: 'default' }}>verbatim text</span>{' '}
            in both places — click to inspect. These are exact overlaps, not a judgement of intent.
          </p>
          <div style={{ display: 'grid', gap: 10 }} data-testid="plag-matches">
            {all.map((m, i) => (
              <MatchCard
                key={i}
                match={m}
                index={i}
                selected={selected === m}
                onSelect={() => setSelected(m)}
              />
            ))}
          </div>
          {selected && <MatchInspector match={selected} />}
        </>
      )}
    </div>
  );
};

export default PlagiarismExactReport;
