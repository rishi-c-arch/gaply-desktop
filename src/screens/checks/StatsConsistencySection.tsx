// Gaply — Level 0 of the statistics check: can the numbers a manuscript prints
// all be right at once, judged from the manuscript alone? Its own section on
// the free Stats Check screen, beside (never merged with) the validation
// findings. Every reason and note is Rust's text (`stats_consistency.rs`); this
// file adds only the verdict labels. Counts only: no percentage, ring, gauge or
// score, and no premium badge.
import React, { useState } from 'react';
import { Badge, BadgeStatus, Button, Card } from '../../design-system';
import {
  L0Location,
  L0MethodCheckView,
  L0ResultView,
  L0VerdictKind,
  StatsConsistencyView,
} from './statsConsistencyTypes';

/** Display order, and each verdict's label and badge. CouldNotCheck is neutral,
 *  never success. Inconsistent never says the p-value is wrong. */
export const VERDICT_ORDER: L0VerdictKind[] = ['inconsistent', 'concern', 'couldNotCheck', 'consistent'];
const LABEL: Record<L0VerdictKind, string> = {
  inconsistent: 'Inconsistent',
  concern: 'Concern',
  couldNotCheck: 'Could not check',
  consistent: 'Consistent',
};
const BADGE: Record<L0VerdictKind, BadgeStatus> = {
  inconsistent: 'flagged',
  concern: 'assessed',
  couldNotCheck: 'neutral',
  consistent: 'certain',
};
export const INCONSISTENT_TEXT = 'These numbers cannot all be correct as printed.';

/** `.0412`, or `3.1e-9` below .001. */
export function formatP(x: number): string {
  if (x === 0) return '0';
  if (x >= 0.001) return x.toFixed(4).replace(/^0\./, '.');
  return x.toExponential(1);
}

function where(loc: L0Location): string {
  const s = loc.section.charAt(0).toUpperCase() + loc.section.slice(1);
  return `${s}, paragraph ${loc.paragraph + 1}`;
}

const muted: React.CSSProperties = { color: 'var(--g-text-3)', fontSize: 13, margin: '4px 0 0' };

export interface L0ResultCardProps {
  /** `result.check` is required by the type: a card cannot render without the
   *  method check its verdict rests on. */
  result: L0ResultView & { check: L0MethodCheckView };
  alpha: string;
}

export const L0ResultCard: React.FC<L0ResultCardProps> = ({ result, alpha }) => {
  const v = result.verdict;
  return (
    <div
      data-testid={`l0-card-${v.kind}`}
      data-result-id={result.id}
      style={{ border: '1px solid var(--g-border)', borderRadius: 8, padding: 12 }}
    >
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
        <Badge status={BADGE[v.kind]} data-testid="l0-verdict-label">{LABEL[v.kind]}</Badge>
        <span className="gds-mono" data-testid="l0-raw">{result.raw}</span>
      </div>
      {v.kind === 'inconsistent' && (
        <>
          <p style={{ margin: '8px 0 0' }}>{INCONSISTENT_TEXT}</p>
          <p style={muted} data-testid="l0-conclusion">
            {v.conclusionChanges === true
              ? `The conclusion at α = ${alpha} changes: the printed p and the recomputed p fall on opposite sides of it.`
              : v.conclusionChanges === false
                ? `The conclusion at α = ${alpha} does not change.`
                : 'Whether the conclusion changes was not assessed.'}
          </p>
        </>
      )}
      {v.kind === 'concern' && <p style={{ margin: '8px 0 0' }} data-testid="l0-reason">{v.reason.text}</p>}
      {v.kind === 'couldNotCheck' && (
        <ul style={{ margin: '8px 0 0', paddingLeft: 18 }} data-testid="l0-reasons">
          {v.reasons.map((r, i) => (
            <li key={i}>
              <span style={{ color: 'var(--g-text-3)' }}>{r.methodName}: </span>
              {r.text}
            </li>
          ))}
        </ul>
      )}
      {v.kind === 'consistent' && v.note && (
        <p style={{ margin: '8px 0 0' }} data-testid="l0-note">{v.note.text}</p>
      )}
      <p style={muted}>
        {result.recomputedP
          ? `Recomputed p: ${formatP(result.recomputedP.lo)} to ${formatP(result.recomputedP.hi)}`
          : 'No p-value could be recomputed from these numbers.'}
        {' · '}
        <span data-testid="l0-method">{result.check.methodName}</span>
        {' · '}
        {where(result.location)}
      </p>
    </div>
  );
};

export interface StatsConsistencySectionProps {
  /** Exactly one of these: the view, or the reason it could not be produced
   *  (an import refusal's own message, shown as it is). */
  view?: StatsConsistencyView;
  error?: string;
}

const StatsConsistencySection: React.FC<StatsConsistencySectionProps> = ({ view, error }) => {
  const [showPlain, setShowPlain] = useState(false);
  const title = 'Do the reported numbers agree? (Level 0)';

  if (!view) {
    return (
      <Card title={title} data-testid="l0-section">
        <p role="alert" data-testid="l0-refused" style={{ margin: 0 }}>
          {error ?? 'This check did not run.'}
        </p>
      </Card>
    );
  }

  const alpha = view.alpha.values.join(', ');
  const c = view.verdictCounts;
  const noted = view.results.filter((r) => r.verdict.kind !== 'consistent' || r.verdict.note !== null);
  const plain = view.results.filter((r) => r.verdict.kind === 'consistent' && r.verdict.note === null);
  const ordered = VERDICT_ORDER.flatMap((k) => noted.filter((r) => r.verdict.kind === k));

  return (
    <Card title={title} data-testid="l0-section">
      <p style={{ ...muted, marginTop: 0 }}>
        Each result's test statistic, degrees of freedom and p-value are checked against one another, from the
        manuscript alone. Each printed number is taken as the range its digits allow.
      </p>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', margin: '12px 0' }} data-testid="l0-header">
        <Badge status="flagged" data-testid="l0-count-inconsistent">{c.inconsistent} Inconsistent</Badge>
        <Badge status="assessed" data-testid="l0-count-concern">{c.concern} Concern</Badge>
        <Badge status="neutral" data-testid="l0-count-could-not-check">{c.couldNotCheck} Could not check</Badge>
        <Badge status="certain" data-testid="l0-count-consistent">{c.consistent} Consistent</Badge>
        <Badge status="neutral" data-testid="l0-count-not-read">{view.notReadCount} not read</Badge>
      </div>
      <p style={muted} data-testid="l0-alpha">
        Significance threshold used: α = {alpha}.{view.alpha.note ? ` ${view.alpha.note}` : ''}
      </p>

      <div style={{ display: 'grid', gap: 8, marginTop: 12 }}>
        {ordered.map((r) => (
          <L0ResultCard key={r.id} result={r} alpha={alpha} />
        ))}
      </div>

      {plain.length > 0 && (
        <div style={{ marginTop: 12 }} data-testid="l0-plain">
          <Button variant="ghost" onClick={() => setShowPlain((s) => !s)} data-testid="l0-plain-toggle">
            {showPlain ? 'Hide' : 'Show'} {plain.length} consistent {plain.length === 1 ? 'result' : 'results'}
          </Button>
          {showPlain && (
            <div style={{ display: 'grid', gap: 8, marginTop: 8 }}>
              {plain.map((r) => (
                <L0ResultCard key={r.id} result={r} alpha={alpha} />
              ))}
            </div>
          )}
        </div>
      )}

      <div style={{ marginTop: 16 }} data-testid="l0-not-read">
        <p style={{ margin: 0 }}>
          {view.notReadCount} {view.notReadCount === 1 ? 'attempt was' : 'attempts were'} not read as a complete
          result{view.notReadCount > 0 ? ':' : '.'}
        </p>
        {view.notRead.length > 0 && (
          <ul style={{ margin: '4px 0 0', paddingLeft: 18 }}>
            {view.notRead.map((n) => (
              <li key={n.code} data-testid={`l0-not-read-${n.code}`}>
                {n.count} × {n.text}
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
};

export default StatsConsistencySection;
