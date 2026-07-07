// Gaply — the Reviewer Letter panel (renders inside the F6 viewer's paid tab).
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { Badge, Button, Card, ScoreRing } from '../../design-system';
import {
  RECOMMENDATION_LABEL,
  RECOMMENDATION_STATUS,
  ReviewerLetter,
} from './publishReadyTypes';
import './publishready.css';

export const ReviewerLetterPanel: React.FC<{ letter: ReviewerLetter }> = ({ letter }) => {
  const navigate = useNavigate();
  const status = RECOMMENDATION_STATUS[letter.recommendation];
  return (
    <div className="gds-pr" data-testid="reviewer-letter-panel">
      <div className="gds-pr__head">
        <div
          className="gds-pr__verdict"
          data-status={status}
          data-testid="pr-verdict"
        >
          {RECOMMENDATION_LABEL[letter.recommendation]}
        </div>
        <div className="gds-pr__gauge">
          <ScoreRing
            score={letter.publicationProbability}
            status={status}
            size={96}
            strokeWidth={8}
            label={`publication probability ${letter.publicationProbability}%`}
          />
          <span className="gds-pr__gauge-label" data-testid="pr-probability">
            {letter.publicationProbability}% publication probability
          </span>
        </div>
      </div>

      <Card title="Reviewer summary">
        <p className="gds-pr__body" data-testid="pr-body">{letter.body}</p>
      </Card>

      <div className="gds-pr__grid">
        <Card title="Novelty vs. recent literature">
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <ScoreRing score={letter.novelty.score} status="neutral" size={54} strokeWidth={5} />
            <span data-testid="pr-novelty">{letter.novelty.assessment}</span>
          </div>
        </Card>

        <Card title={`Journal fit — ${letter.journalFit.journal} (${letter.journalFit.quartile})`}>
          <div style={{ display: 'grid', gap: 6 }}>
            <div>
              <Badge status={letter.journalFit.fitScore >= 65 ? 'certain' : 'assessed'} data-testid="pr-fit">
                fit {letter.journalFit.fitScore}%
              </Badge>{' '}
              {letter.journalFit.note}
            </div>
          </div>
        </Card>
      </div>

      <Card title="Suggested alternative venues (same or higher quartile)">
        <div className="gds-pr__alts" data-testid="pr-alternatives">
          {letter.alternatives.length === 0 ? (
            <span style={{ color: 'var(--g-text-3)', fontSize: 13 }}>No higher-tier alternatives found.</span>
          ) : (
            letter.alternatives.map((a) => (
              <button
                key={a.name}
                className="gds-pr__alt"
                data-testid={`pr-alt-${a.quartile}`}
                onClick={() => navigate('/app/journal')}
              >
                <span>{a.name}</span>
                <Badge status={a.quartile === 'Q1' ? 'certain' : 'neutral'}>{a.quartile}</Badge>
              </button>
            ))
          )}
        </div>
      </Card>

      <p className="gds-pr__disclaimer">
        Model-assisted assessment — non-definitive. Deterministic (mathematically certain) findings
        require correction regardless of the recommendation. See the Checklist tab for guideline
        items (each links to the offending section).
      </p>
    </div>
  );
};

export default ReviewerLetterPanel;
