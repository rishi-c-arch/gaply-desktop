// Gaply — the Reviewer Letter panel (renders inside the F6 viewer's paid tab).
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { Badge, Card, ScoreRing } from '../../design-system';
import {
  RECOMMENDATION_LABEL,
  RECOMMENDATION_STATUS,
  ReviewerLetter,
} from './publishReadyTypes';
import './publishready.css';

export const ReviewerLetterPanel: React.FC<{ letter: ReviewerLetter }> = ({ letter }) => {
  const navigate = useNavigate();

  // Honest offline state: deep reasoning is cloud-only. The FULL local report
  // (findings, checklist, verdict) still renders in the viewer's other tabs —
  // this panel just declares the reviewer letter unavailable, never fakes one.
  if (letter.available === false) {
    return (
      <div className="gds-pr" data-testid="reviewer-letter-panel">
        <div className="gds-pr__head">
          <div className="gds-pr__verdict" data-status="assessed" data-testid="pr-verdict">
            REVIEWER UNAVAILABLE
          </div>
        </div>
        <Card title="Deep reviewer analysis">
          <p className="gds-pr__body" data-testid="pr-unavailable">
            {letter.body || 'Deep reasoning requires cloud analysis — unavailable offline.'}
          </p>
          <p style={{ color: 'var(--g-text-3)', fontSize: 13 }}>
            The full local report — findings, checklist, and verdict — is available in the tabs above.
          </p>
        </Card>
        <p className="gds-pr__disclaimer">
          Model-assisted assessment — non-definitive. Deterministic (mathematically certain) findings
          require correction regardless of the reviewer letter.
        </p>
      </div>
    );
  }

  const status = RECOMMENDATION_STATUS[letter.recommendation];
  const issues = letter.issues ?? [];
  const warnings = letter.warnings ?? [];
  return (
    <div className="gds-pr" data-testid="reviewer-letter-panel">
      <div className="gds-pr__head">
        <div className="gds-pr__verdict" data-status={status} data-testid="pr-verdict">
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

      {issues.length > 0 && (
        <Card title="Reviewer issues (grounded in report findings)">
          <ul className="gds-pr__issues" data-testid="pr-issues" style={{ margin: 0, paddingLeft: 18 }}>
            {issues.map((iss, i) => (
              <li key={`${iss.findingRef}-${i}`} style={{ marginBottom: 6 }}>
                <Badge status={iss.severity === 'critical' || iss.severity === 'major' ? 'flagged' : 'assessed'}>
                  {iss.severity || 'issue'}
                </Badge>{' '}
                <span style={{ color: 'var(--g-text-3)', fontSize: 12 }}>({iss.findingRef})</span>{' '}
                {iss.rationale}
              </li>
            ))}
          </ul>
        </Card>
      )}

      {/* Novelty + journal fit render the backend's GROUNDED text only.
          The numeric score ring and the "fit NN%" badge that used to sit here
          are gone: the proxy payload carries no topic or subject matter, so
          nothing could ground either number — and the fit badge rendered
          `certain` at ≥65, which is the strongest claim in the design system
          attached to the weakest evidence in the report. Both fall back to '—',
          the same honest empty state the gate already produces for the text. */}
      <div className="gds-pr__grid">
        <Card title="Novelty vs. recent literature">
          <span data-testid="pr-novelty">{letter.novelty.assessment || '—'}</span>
        </Card>

        <Card title={`Journal fit — ${letter.journalFit.journal} (${letter.journalFit.quartile})`}>
          <span data-testid="pr-fit">{letter.journalFit.note || '—'}</span>
        </Card>
      </div>

      {/* Suggested alternative journals — grounded in the analysis, ADVISORY
          (Set 4-A). Empty when the backend grounded none; never fabricated. */}
      {letter.alternatives.length > 0 && (
        <Card title="Suggested alternative journals">
          <p style={{ color: 'var(--g-text-3)', fontSize: 12, marginTop: 0 }}>
            Suggested based on the analysis — advisory, not a recommendation to submit.
          </p>
          <div className="gds-pr__alts" data-testid="pr-alternatives">
            {letter.alternatives.map((a) => (
              <button
                key={a.name}
                className="gds-pr__alt"
                data-testid={`pr-alt-${a.quartile}`}
                onClick={() => navigate('/app/journal')}
                title="Open in Journal check"
              >
                <span className="gds-pr__alt-name">{a.name}</span>
                <Badge status={a.quartile === 'Q1' ? 'certain' : 'neutral'}>{a.quartile}</Badge>
                {a.reason ? (
                  <span style={{ color: 'var(--g-text-3)', fontSize: 12 }} data-testid="pr-alt-reason">
                    {a.reason}
                  </span>
                ) : null}
              </button>
            ))}
          </div>
        </Card>
      )}

      {warnings.length > 0 && (
        <p className="gds-pr__disclaimer" data-testid="pr-warnings" style={{ color: 'var(--g-text-3)' }}>
          Gate notes: {warnings.join('; ')}
        </p>
      )}

      <p className="gds-pr__disclaimer">
        Model-assisted assessment — non-definitive. Deterministic (mathematically certain) findings
        require correction regardless of the recommendation. See the Checklist tab for guideline
        items (each links to the offending section).
      </p>
    </div>
  );
};

export default ReviewerLetterPanel;
