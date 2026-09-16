// Gaply — the Reviewer Letter panel (renders inside the F6 viewer's paid tab).
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { Badge, Card } from '../../design-system';
import {
  RECOMMENDATION_LABEL,
  RECOMMENDATION_STATUS,
  ReviewerLetter,
  DeclinedLane,
} from './publishReadyTypes';
import './publishready.css';

/** One declined lane, rendered where its output would have been.
 *
 *  The sentence is the backend's — `gaply_core::declined::DECLINED_LANES` — so
 *  a user and an engineer read the same decline. If the backend sent nothing
 *  (older run, mock bridge), this renders the em-dash it replaced rather than
 *  inventing a reason. */
const DeclineNote: React.FC<{ testId: string; lane: string; declined?: DeclinedLane[] }> = ({
  testId,
  lane,
  declined,
}) => {
  const d = (declined ?? []).find((x) => x.lane === lane);
  if (!d) return <span data-testid={testId}>—</span>;
  return (
    <span data-testid={testId} className="gds-pr__decline">
      <strong>Not assessed.</strong> {d.reason}{' '}
      <span className="gds-pr__decline-ref">{d.record}</span>
    </span>
  );
};

export const ReviewerLetterPanel: React.FC<{
  letter: ReviewerLetter;
  /** From the run outcome; constant per build. */
  declined?: DeclinedLane[];
}> = ({ letter, declined }) => {
  const navigate = useNavigate();

  // Honest offline state: deep reasoning is cloud-only. The FULL local report
  // (findings, checklist, verdict) still renders in the viewer's other tabs —
  // this panel just declares the reviewer letter unavailable, never fakes one.
  // WITHHELD is not the same unavailable state as offline, and the difference is
  // load-bearing: offline means the CLOUD reviewer could not run while the local
  // verdict stands; withheld means the deterministic verdict itself was not
  // produced. Saying "the verdict is available in the tabs above" is false in the
  // second case, so the two are told apart rather than sharing one message
  // (§4.4 — correct the claim, do not caveat it). ARCHITECTURE_TRACE §26 PR-2.
  const withheldReason = letter.warnings?.find((w) => w.startsWith('verdict withheld:'));
  // Exclusions are a DIFFERENT kind of note from gate drops, and mixing them
  // into one "Gate notes" line would bury the one the author must act on.
  const excluded = (letter.warnings ?? []).filter((w) =>
    w.startsWith('excluded from the recommendation:')
  );
  // "This lane examined nothing" is the same SHAPE of statement as "this
  // finding did not count" — both say how the verdict was reached — so it
  // travels the same channel and renders in the same card.
  const notExamined = (letter.warnings ?? []).filter((w) => w.startsWith('not examined:'));
  const gateNotes = (letter.warnings ?? []).filter(
    (w) =>
      !w.startsWith('excluded from the recommendation:') && !w.startsWith('not examined:')
  );

  if (letter.available === false) {
    return (
      <div className="gds-pr" data-testid="reviewer-letter-panel">
        <div className="gds-pr__head">
          <div className="gds-pr__verdict" data-status="assessed" data-testid="pr-verdict">
            {withheldReason ? 'RECOMMENDATION WITHHELD' : 'REVIEWER UNAVAILABLE'}
          </div>
        </div>
        <Card title={withheldReason ? 'No recommendation for this run' : 'Deep reviewer analysis'}>
          <p className="gds-pr__body" data-testid={withheldReason ? 'pr-withheld' : 'pr-unavailable'}>
            {letter.body || 'Deep reasoning requires cloud analysis — unavailable offline.'}
          </p>
          <p style={{ color: 'var(--g-text-3)', fontSize: 13 }}>
            {withheldReason
              ? 'The findings and checklist in the tabs above are unaffected — only the overall recommendation is missing.'
              : 'The full local report — findings, checklist, and verdict — is available in the tabs above.'}
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
  // NOTE: there is deliberately no raw `warnings` list here. Every warning is
  // already read above and routed to one of four renderings — withheldReason,
  // excluded, notExamined, and gateNotes, which is the catch-all — so nothing
  // falls through. A second, uncategorised copy was a leftover of that refactor.
  return (
    <div className="gds-pr" data-testid="reviewer-letter-panel">
      <div className="gds-pr__head">
        {/* Caps are PRESENTATION: the vocabulary returns sentence case so the
            label is usable mid-sentence (the PDF needs it that way), and the
            header shouts in CSS instead. */}
        <div
          className="gds-pr__verdict"
          data-status={status}
          data-testid="pr-verdict"
          style={{ textTransform: 'uppercase' }}
        >
          {RECOMMENDATION_LABEL[letter.recommendation]}
        </div>
        {/* NO GAUGE. `publication_probability` is a FOUR-VALUE LOOKUP from the
            recommendation (reviewer_agent.rs:1161), not a calibrated
            probability — rendering it as a percentage in a ring is ONTOLOGY
            §4.20's PRESENTATION class: a visual implication beyond the computed
            evidence. The Rust report never showed it, and now that the Rust
            report is the one users open, the desktop screen must not either.
            The recommendation itself is shown above, which is what the engine
            actually produced. */}
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
          {/* An em-dash used to stand here. It read as "Gaply looked and found
              nothing" — a claim about the manuscript — when the truth is that
              Gaply does not assess novelty at all, on a measurement. The reason
              comes from the backend (gaply_core::declined) so the sentence a
              user reads and the decision an engineer reads cannot drift. */}
          {letter.novelty.assessment ? (
            <span data-testid="pr-novelty">{letter.novelty.assessment}</span>
          ) : (
            <DeclineNote testId="pr-novelty" lane="Novelty" declined={declined} />
          )}
        </Card>

        <Card title={`Journal fit — ${letter.journalFit.journal} (${letter.journalFit.quartile})`}>
          {/* NOT a declined lane, and deliberately still an em-dash: the fit
              note is grounded text the harness gate DROPS when the model's
              claim is not supported by the payload. That is a per-run outcome,
              not a standing limit, so a constant explanation would be wrong. */}
          <span data-testid="pr-fit">{letter.journalFit.note || '—'}</span>
        </Card>
      </div>

      {/* WHAT GAPLY DOES NOT DO. Every lane here was declined on a measurement
          recorded in docs/AI_ENGINE_PLAN.md, and until now none of it reached a
          user — the panel rendered a blank and a reader concluded their paper
          was clean. Listing them costs one section and converts "found nothing"
          into "does not look", which a researcher can act on by asking someone
          who does. */}
      {declined && declined.length > 0 && (
        <Card title="What Gaply does not assess">
          <ul className="gds-pr__declines" data-testid="pr-declined">
            {declined.map((d) => (
              <li key={d.lane} data-testid={`pr-declined-${d.lane}`}>
                <strong>{d.lane}.</strong> {d.reason}{' '}
                <span className="gds-pr__decline-ref">{d.record}</span>
              </li>
            ))}
          </ul>
        </Card>
      )}

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

      {/* EXCLUSIONS are shown separately from gate notes, and said plainly.
          §4.14: "did not affect the recommendation" is NOT "unimportant" — a
          user who sees a finding in the report but not in the verdict must be
          told why, and must not read the exclusion as the finding being minor. */}
      {notExamined.length > 0 && (
        <Card title="Checks that could not run on this manuscript">
          <ul data-testid="pr-not-examined" style={{ margin: 0, paddingLeft: 18 }}>
            {notExamined.map((w, i) => (
              <li key={i} className="gds-finding__detail" style={{ fontSize: 13 }}>
                {w.replace('not examined: ', '')}
              </li>
            ))}
          </ul>
          <p style={{ color: 'var(--g-text-3)', fontSize: 12, marginTop: 8 }}>
            The recommendation reflects only what was examined. It is not a statement about
            anything these checks would have covered.
          </p>
        </Card>
      )}

      {excluded.length > 0 && (
        <Card title="Findings shown but not counted toward the recommendation">
          <ul data-testid="pr-excluded" style={{ margin: 0, paddingLeft: 18 }}>
            {excluded.map((w, i) => (
              <li key={i} className="gds-finding__detail" style={{ fontSize: 13 }}>
                {w.replace('excluded from the recommendation: ', '')}
              </li>
            ))}
          </ul>
          <p style={{ color: 'var(--g-text-3)', fontSize: 12, marginTop: 8 }}>
            These are still real and still worth reading — they are not evidence about whether
            your manuscript is publishable, so they do not move the recommendation.
          </p>
        </Card>
      )}

      {gateNotes.length > 0 && (
        <p className="gds-pr__disclaimer" data-testid="pr-warnings" style={{ color: 'var(--g-text-3)' }}>
          Gate notes: {gateNotes.join('; ')}
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
