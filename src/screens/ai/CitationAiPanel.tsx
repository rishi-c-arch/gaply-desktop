// Gaply — the AI assistance section of the citation panel (Phase 9 item 2).
//
// Sits BELOW the deterministic citation UI, which is untouched and instant.
// That order is the point: the app's certain half must never wait on, or look
// broken because of, the uncertain half.
import './ai.css';
import React, { useCallback, useState } from 'react';
import { Button, Card } from '../../design-system/primitives';
import { aiBridge } from './aiBridge';
import { EvidenceCard, EvidenceRow, GroundedFinding, Verdict } from './EvidenceCard';
import { AiUnavailable } from './AiStatusPanel';

type Phase = 'idle' | 'running' | 'done' | 'failed' | 'rejected';

export interface CitationAiPanelProps {
  sentence: string;
  /** The cited source's indexed document, when there is one. */
  documentId?: number | null;
  citedSource?: string;
  aiInstalled: boolean;
  onOpenSettings?: () => void;
  bridge?: Pick<typeof aiBridge, 'citationNeed' | 'citationSupport' | 'cancelGeneration' | 'documentSource'>;
}

/** Map a raw support result onto the D18 unit. */
export async function toFinding(
  raw: Record<string, any>,
  documentId: number,
  citedSource: string,
  documentExists: boolean,
): Promise<GroundedFinding> {
  const out = raw.output ?? {};
  const chunks: any[] = out.supporting_chunks ?? [];
  const evidence: EvidenceRow[] = chunks.map((c) => ({
    chunkId: String(c.chunk_id ?? ''),
    documentId,
    page: typeof c.page === 'number' ? c.page : null,
    sourceLabel: citedSource,
    quote: String(c.quote ?? c.why ?? ''),
    fileAvailable: documentExists,
  }));
  return {
    verdict: (out.verdict as Verdict) ?? 'no_evidence',
    confidence: typeof out.confidence === 'number' ? out.confidence : null,
    explanation: String(out.explanation ?? raw.reason ?? ''),
    advisories: raw.advisories ?? [],
    evidence,
  };
}

export const CitationAiPanel: React.FC<CitationAiPanelProps> = ({
  sentence,
  documentId,
  citedSource = 'Cited source',
  aiInstalled,
  onOpenSettings,
  bridge = aiBridge,
}) => {
  const [phase, setPhase] = useState<Phase>('idle');
  const [message, setMessage] = useState<string | null>(null);
  const [finding, setFinding] = useState<GroundedFinding | null>(null);

  const reset = () => {
    setMessage(null);
    setFinding(null);
  };

  const runSupport = useCallback(async () => {
    if (documentId == null) return;
    reset();
    setPhase('running');
    try {
      const src = await bridge.documentSource(documentId);
      const raw = (await bridge.citationSupport(sentence, documentId, citedSource)) as Record<string, any>;

      // The engine's three honest non-success outcomes, each said plainly
      // rather than collapsed into "something went wrong".
      if (raw.outcome === 'noEvidence') {
        setPhase('done');
        setFinding({
          verdict: 'no_evidence',
          confidence: null,
          explanation: String(raw.reason ?? 'No indexed evidence was retrieved for this claim.'),
          evidence: [],
        });
        return;
      }
      if (raw.outcome === 'validationFailed' || raw.outcome === 'error') {
        setPhase('rejected');
        setMessage(
          'AI output failed verification — the model’s answer did not meet Gaply’s grounding checks, so it was discarded rather than shown.',
        );
        return;
      }
      setFinding(await toFinding(raw, documentId, citedSource, src.exists));
      setPhase('done');
    } catch (e) {
      setPhase('failed');
      setMessage(e instanceof Error ? e.message : String(e));
    }
  }, [bridge, sentence, documentId, citedSource]);

  const runNeed = useCallback(async () => {
    reset();
    setPhase('running');
    try {
      const raw = (await bridge.citationNeed(sentence)) as Record<string, any>;
      if (raw.outcome === 'validationFailed') {
        setPhase('rejected');
        setMessage('AI output failed verification — retry, or check this sentence yourself.');
        return;
      }
      const out = raw.output ?? raw;
      // citation_need has NO evidence block (§11 D8) — it judges a sentence,
      // not a source. It therefore renders as plain text, NOT through the
      // evidence unit, which would otherwise show an empty-evidence refusal.
      setMessage(
        out.needs_citation
          ? `Needs a citation (${out.severity ?? 'unrated'}): ${out.reason ?? ''}`
          : `No citation required: ${out.reason ?? ''}`,
      );
      setPhase('done');
    } catch (e) {
      setPhase('failed');
      setMessage(e instanceof Error ? e.message : String(e));
    }
  }, [bridge, sentence]);

  if (!aiInstalled) {
    return <AiUnavailable feature="Citation checking" onOpenSettings={onOpenSettings} />;
  }

  return (
    <Card title="AI assistance" data-testid="citation-ai-panel">
      <div className="gds-audit__actions">
        <Button
          variant="secondary"
          onClick={runSupport}
          disabled={phase === 'running' || documentId == null}
          data-testid="ai-check-support"
          title={
            documentId == null
              ? 'This citation’s source is not linked to an indexed document'
              : undefined
          }
        >
          Check citation support
        </Button>
        <Button
          variant="secondary"
          onClick={runNeed}
          disabled={phase === 'running'}
          data-testid="ai-check-need"
        >
          Check if citation is needed
        </Button>
        {phase === 'running' && (
          <Button variant="ghost" onClick={() => bridge.cancelGeneration()} data-testid="ai-cancel">
            Cancel
          </Button>
        )}
      </div>

      {phase === 'running' && (
        <p className="gds-ai__hint" data-testid="ai-running">
          Checking on this machine — this takes about a minute.
        </p>
      )}

      {phase === 'rejected' && (
        <p className="gds-ai__hint" data-testid="ai-validation-failed" style={{ color: 'var(--g-flagged)' }}>
          {message}
        </p>
      )}

      {phase === 'failed' && (
        <p className="gds-ai__hint" data-testid="ai-error" style={{ color: 'var(--g-flagged)' }}>
          {message}
        </p>
      )}

      {phase === 'done' && message && (
        <p className="gds-ai__hint" data-testid="ai-need-result">
          {message}
        </p>
      )}

      {finding && <EvidenceCard finding={finding} />}
    </Card>
  );
};

export default CitationAiPanel;
