// Gaply — one Level 1 result (Phase 2, Step 7). Every number on the card comes
// from the engine's ReproductionView; an AI explanation sits beside it,
// labelled, and cannot replace any of them. Method context from the web sits
// under its own heading, apart from the engine's evidence.
//
// Phase 1's wording rules: no score, no percentage, no gauge; Could-not-check
// is neutral; Not-reproduced never says the paper is wrong.
import React, { useMemo, useState } from 'react';
import { Badge, Button } from '../../../design-system';
import type { StatsL1Bridge } from '../statsL1Bridge';
import type { L1ClaimReproductionView, L1ClaimView, L1ContextItemView, L1HelperResultView, L1NumberView, L1PreviewView } from '../statsL1Types';
import { FocusButton } from './FocusButton';
import { L1AiPanel, queryFromPreview } from './L1AiPanel';
import { methodPhrase, NOT_REPRODUCED_TEXT, sentenceFor, WordContext } from './l1Wording';

const VERDICT: Record<L1ClaimReproductionView['verdict']['kind'], { label: string; status: 'certain' | 'assessed' | 'neutral' }> = {
  reproduced: { label: 'Reproduced', status: 'certain' },
  notReproduced: { label: 'Not reproduced', status: 'assessed' },
  couldNotCheck: { label: 'Could not check', status: 'neutral' },
};

const TOPICS: Array<{ id: 'assumptions' | 'reporting_guidance' | 'effect_size_reporting' | 'choosing_a_variant'; label: string }> = [
  { id: 'assumptions', label: "The method's assumptions" },
  { id: 'reporting_guidance', label: 'How to report it' },
  { id: 'effect_size_reporting', label: 'Reporting an effect size' },
  { id: 'choosing_a_variant', label: 'Choosing a variant' },
];

/** A computed value, shown at a precision that can be compared by eye. */
export function shown(v: number): string {
  if (!Number.isFinite(v)) return String(v);
  if (v !== 0 && Math.abs(v) < 0.0001) return v.toExponential(2);
  return String(Number(v.toPrecision(6)));
}

/** "(the file stores 2 decimals: 1.89 to 1.95)" beside a computed value
 *  whose stored cells' rounding gives it a range (Step 8b). */
export function storedRange(n: L1NumberView, decimals: number | null): string {
  if (n.precisionLo === null || n.precisionHi === null || decimals === null) return '';
  return ` (the file stores ${decimals} ${decimals === 1 ? 'decimal' : 'decimals'}: ${shown(n.precisionLo)} to ${shown(n.precisionHi)})`;
}

function accepted(n: L1NumberView): string {
  if (n.acceptedLo === null && n.acceptedHi === null) return '';
  const lo = n.acceptedLo === null ? '…' : shown(n.acceptedLo);
  const hi = n.acceptedHi === null ? '…' : shown(n.acceptedHi);
  return ` (agrees if it lies in ${lo} to ${hi})`;
}

/** Retrieval time, in UTC so every reader sees the same text. */
export function retrievedPhrase(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const p = (x: number) => String(x).padStart(2, '0');
  return `Retrieved ${d.getUTCFullYear()}-${p(d.getUTCMonth() + 1)}-${p(d.getUTCDate())} ${p(d.getUTCHours())}:${p(d.getUTCMinutes())} UTC`;
}

export interface L1ResultCardProps {
  bridge: StatsL1Bridge;
  run: number;
  claim: L1ClaimView | undefined;
  result: L1ClaimReproductionView;
  ctx: WordContext;
  aiHelp: boolean;
  webContext: boolean;
  onUsage: () => void;
}

export const L1ResultCard: React.FC<L1ResultCardProps> = ({ bridge, run, claim, result, ctx, aiHelp, webContext, onUsage }) => {
  const [explainOpen, setExplainOpen] = useState(false);
  const [explanation, setExplanation] = useState<string | null>(null);
  const [contextOpen, setContextOpen] = useState(false);
  const [topic, setTopic] = useState<(typeof TOPICS)[number]['id']>('assumptions');
  const [queryField, setQueryField] = useState('');
  const [editedQuery, setEditedQuery] = useState<string | null>(null);
  const [context, setContext] = useState<Extract<L1HelperResultView, { kind: 'context' }> | null>(null);
  const [linkError, setLinkError] = useState<string | null>(null);
  const explainButton = React.useRef<HTMLButtonElement>(null);
  const contextButton = React.useRef<HTMLButtonElement>(null);
  const v = VERDICT[result.verdict.kind];
  const id = `l1-result-${result.claimId}`;

  const contextInput = useMemo(
    () =>
      result.method
        ? ({ task: 'web_context', claimId: result.claimId, methodId: result.method, topic, editedQuery } as const)
        : null,
    [result.claimId, result.method, topic, editedQuery]
  );
  const explainInput = useMemo(() => ({ task: 'result_explanation', claimId: result.claimId } as const), [result.claimId]);

  const conclusion =
    result.verdict.kind === 'notReproduced'
      ? result.verdict.conclusionChanges === true
        ? "At the paper's significance threshold, the conclusion would change."
        : result.verdict.conclusionChanges === false
          ? "At the paper's significance threshold, the conclusion would not change."
          : 'A change in conclusion was not assessed.'
      : null;
  const notes = result.verdict.kind === 'reproduced' ? [...result.verdict.notes, ...result.notes] : result.notes;

  const open = async (url: string) => {
    setLinkError(null);
    try {
      await bridge.openSourceLink(url);
    } catch {
      setLinkError('That source could not be opened.');
    }
  };

  return (
    <article className="gds-card" aria-labelledby={`${id}-h`} data-testid={id} data-verdict={result.verdict.kind} style={{ display: 'grid', gap: 6 }}>
      <h4 id={`${id}-h`} style={{ margin: 0, display: 'flex', gap: 8, alignItems: 'center' }}>
        <Badge status={v.status} data-testid={`${id}-verdict`}>{v.label}</Badge>
        <span>{claim ? claim.sentence : `Claim ${result.claimId + 1}`}</span>
      </h4>

      {result.verdict.kind === 'notReproduced' && <p data-testid={`${id}-not-reproduced`}>{NOT_REPRODUCED_TEXT}</p>}
      {result.verdict.kind === 'couldNotCheck' && (
        <p data-testid={`${id}-reason`}>{sentenceFor(result.verdict.reason, ctx)}</p>
      )}

      {result.method && (
        <p style={{ margin: 0 }} data-testid={`${id}-method`}>
          Method: {methodPhrase(result.method, result.variant?.code ?? null)}. Variant:{' '}
          {sentenceFor(result.variant ?? undefined, ctx)}
          {result.variantSetBy ? `, ${sentenceFor(result.variantSetBy, ctx)}` : ''}.
        </p>
      )}

      {result.candidates.map((cand, i) => (
        <table key={i} data-testid={`${id}-numbers`} style={{ borderCollapse: 'collapse', fontSize: 13 }}>
          <caption style={{ textAlign: 'left', fontWeight: 600 }}>Computed as {sentenceFor(cand.variant, ctx)}</caption>
          <thead>
            <tr>
              <th scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>Number</th>
              <th scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>Printed</th>
              <th scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>Computed from the data</th>
              <th scope="col" style={{ textAlign: 'left' }}>Status</th>
            </tr>
          </thead>
          <tbody>
            {cand.numbers.map((n, j) => (
              <tr key={j}>
                <td style={{ paddingRight: 12 }}>{sentenceFor(n.kind, ctx)}</td>
                <td style={{ paddingRight: 12 }} className="gds-mono">{n.relation === '=' ? n.printed : `${n.relation} ${n.printed}`}</td>
                <td style={{ paddingRight: 12 }} className="gds-mono" data-testid={`${id}-computed`}>
                  {shown(n.computed)}
                  {storedRange(n, cand.storedDecimals)}
                  {accepted(n)}
                </td>
                <td>{sentenceFor(n.status, ctx)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ))}

      {result.nUsed !== null && (
        <p style={{ margin: 0 }} data-testid={`${id}-n`}>
          n used: {result.nUsed} of {result.rowsRead} rows read.
          {result.exclusions.filter((e) => e.rows > 0).map((e, i) => (
            <span key={i}> {e.rows} {e.rows === 1 ? 'row' : 'rows'} left out: {sentenceFor(e.reason, ctx)}.</span>
          ))}
        </p>
      )}
      {conclusion && <p style={{ margin: 0 }} data-testid={`${id}-conclusion`}>{conclusion}</p>}
      {notes.length > 0 && (
        <ul style={{ margin: 0 }} data-testid={`${id}-notes`}>
          {notes.map((n, i) => (
            <li key={i}>{sentenceFor(n, ctx)}</li>
          ))}
        </ul>
      )}
      {result.provenance && (
        <details data-testid={`${id}-provenance`}>
          <summary>Provenance</summary>
          <p className="gds-jc__disclaimer" style={{ margin: 0 }}>
            Method {result.provenance.methodId} (version {result.provenance.methodVersion})
            {result.provenance.reference ? `, following ${result.provenance.reference}` : ", Gaply's own arithmetic"}. Engine:{' '}
            {result.provenance.engineVersion}. Binding {sentenceFor(result.provenance.bindingSource, ctx)}; variant{' '}
            {sentenceFor(result.provenance.variantSetBy, ctx)}. Inputs digest{' '}
            <span className="gds-mono">{result.provenance.inputsDigest.slice(0, 16)}</span>.
          </p>
        </details>
      )}

      {aiHelp && (
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <FocusButton variant="secondary" ref={explainButton} onClick={() => setExplainOpen(true)} data-testid={`${id}-explain`}>
            Explain this result
          </FocusButton>
          {webContext && result.method && (
            <FocusButton variant="secondary" ref={contextButton} onClick={() => setContextOpen(true)} data-testid={`${id}-context-open`}>
              Method context (web)
            </FocusButton>
          )}
        </div>
      )}
      {explainOpen && (
        <L1AiPanel
          bridge={bridge}
          run={run}
          input={explainInput}
          onResult={(r) => r.kind === 'explanation' && setExplanation(r.modelText)}
          onSettled={onUsage}
          onClose={() => {
            setExplainOpen(false);
            explainButton.current?.focus();
          }}
        />
      )}
      {explanation !== null && (
        <aside aria-label="AI explanation" data-testid={`${id}-explanation`} style={{ borderLeft: '3px solid var(--g-accent)', paddingLeft: 8 }}>
          <p style={{ fontWeight: 600, margin: 0 }}>AI explanation</p>
          <p style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{explanation}</p>
          <p className="gds-jc__disclaimer" style={{ margin: 0 }}>Written by a model. The numbers above are the engine's.</p>
        </aside>
      )}

      {contextOpen && contextInput && (
        <L1AiPanel
          bridge={bridge}
          run={run}
          input={contextInput}
          title="Method context (web)"
          onPreview={(p: L1PreviewView) => setQueryField(queryFromPreview(p.body) ?? '')}
          onResult={(r) => r.kind === 'context' && setContext(r)}
          onSettled={onUsage}
          onClose={() => {
            setContextOpen(false);
            contextButton.current?.focus();
          }}
        >
          <div style={{ display: 'grid', gap: 4 }}>
            <label htmlFor={`${id}-topic`}>Topic</label>
            <select id={`${id}-topic`} data-testid={`${id}-topic`} value={topic} onChange={(e) => { setEditedQuery(null); setTopic(e.target.value as typeof topic); }}>
              {TOPICS.map((t) => (
                <option key={t.id} value={t.id}>{t.label}</option>
              ))}
            </select>
            <label htmlFor={`${id}-query`}>Search query (what will be sent; you can edit it)</label>
            <input id={`${id}-query`} data-testid={`${id}-query`} value={queryField} maxLength={300} onChange={(e) => setQueryField(e.target.value)} />
            <div>
              <Button variant="secondary" onClick={() => setEditedQuery(queryField)} data-testid={`${id}-query-apply`}>
                Use this query
              </Button>
            </div>
          </div>
        </L1AiPanel>
      )}
      {context && (
        <section aria-labelledby={`${id}-ctx-h`} data-testid={`${id}-context`} style={{ border: '1px dashed var(--g-border, #999)', padding: 8, background: 'var(--g-surface-2, transparent)' }}>
          <h5 id={`${id}-ctx-h`} style={{ margin: 0 }}>Method context from the web (not part of Gaply's check)</h5>
          {context.emptyMessage ? (
            <p data-testid={`${id}-context-empty`}>{context.emptyMessage}</p>
          ) : (
            <ul style={{ margin: 0 }}>
              {context.items.map((item: L1ContextItemView, i) => (
                <li key={i} data-testid={`${id}-context-item`}>
                  <span>{item.sentence}</span>{' '}
                  <span className="gds-jc__disclaimer">{retrievedPhrase(item.retrievedAt)}.</span>
                  {item.sources.map((s, j) => (
                    <Button key={j} variant="ghost" onClick={() => void open(s.url)} data-testid={`${id}-source`} aria-label={`Open source: ${s.title} (${s.domain})`}>
                      {s.title} ({s.domain})
                    </Button>
                  ))}
                </li>
              ))}
            </ul>
          )}
          {context.uncitedSentences > 0 && (
            <p className="gds-jc__disclaimer">{context.uncitedSentences} more {context.uncitedSentences === 1 ? 'sentence had' : 'sentences had'} no cited source and {context.uncitedSentences === 1 ? 'is' : 'are'} not shown.</p>
          )}
          {linkError && <p role="alert">{linkError}</p>}
        </section>
      )}
    </article>
  );
};

export default L1ResultCard;
