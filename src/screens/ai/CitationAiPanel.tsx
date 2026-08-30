// Gaply — the AI assistance section of the citation panel (Phase 9 item 2).
//
// Sits BELOW the deterministic citation UI, which is untouched and instant.
// That order is the point: the app's certain half must never wait on, or look
// broken because of, the uncertain half.
import './ai.css';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Button, Card } from '../../design-system/primitives';
import { aiBridge, errorCode, errorText, SupportEvent } from './aiBridge';
import { EvidenceCard, EvidenceRow, GroundedFinding, Verdict } from './EvidenceCard';
import { AiUnavailable } from './AiStatusPanel';

type Phase = 'idle' | 'running' | 'done' | 'failed' | 'rejected' | 'cancelled';

/** What the run is doing right now, from the command's own Channel.
 *
 *  A single unchanging "Checking on this machine" is indistinguishable from a
 *  wedged process, and on an unoptimised build a real check is minutes long, so
 *  this panel says the STAGE and the ELAPSED time instead of predicting a
 *  duration it cannot know. */
type Progress =
  | { stage: 'starting' }
  | { stage: 'retrieving' }
  | { stage: 'retrieved'; sent: number; dropped: number }
  | { stage: 'waiting'; ahead: number }
  | { stage: 'generating' }
  | { stage: 'decoding'; tokens: number; maxTokens: number }
  | { stage: 'validating' };

function describe(p: Progress): string {
  switch (p.stage) {
    case 'starting':
      return 'Starting…';
    case 'retrieving':
      return 'Finding passages in the cited document…';
    case 'retrieved':
      return `Judging ${p.sent} passage${p.sent === 1 ? '' : 's'}${p.dropped ? ` (${p.dropped} dropped for length)` : ''}…`;
    case 'waiting':
      // The engine runs ONE generation at a time; this is a queue, not slowness.
      return `Waiting for the model — ${p.ahead} other check${p.ahead === 1 ? '' : 's'} ahead of this one.`;
    case 'generating':
      return 'Reading the passages…';
    case 'decoding':
      return `Writing the answer — ${p.tokens} of up to ${p.maxTokens} tokens.`;
    case 'validating':
      return 'Checking the answer against the passages it was given…';
  }
}

/** mm:ss since a run started. */
function elapsedLabel(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export interface CitationAiPanelProps {
  /** The claim to judge. May be empty — see the panel's claim field. */
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
  const [progress, setProgress] = useState<Progress | null>(null);
  /** Which weights actually answered. Stamped by the backend on results AND
   *  failures, so "which model was this?" is never inferred again. */
  const [judgedBy, setJudgedBy] = useState<string | null>(null);
  // The Citation Manager has no manuscript sentence to offer, so the claim is
  // typed here. It used to be filled with the citation's own TITLE, which made
  // the task self-referential — "does this document support 'Chapter 1 —
  // Literature Review'?" — and a title has none of the elements the prompt
  // asks the model to decompose. See the note above the field.
  const [claim, setClaim] = useState(sentence);
  const [seconds, setSeconds] = useState(0);
  // Set the moment Cancel is pressed, so the run that then rejects is reported
  // as the user's decision rather than as a failure. The panel asked for it; it
  // does not need to parse an error string to recognise its own request.
  const cancelling = useRef(false);
  const [cancelRequested, setCancelRequested] = useState(false);

  // A running check is minutes of near-silence, so the panel counts. Ticking
  // only while running keeps this out of every other render.
  useEffect(() => {
    if (phase !== 'running') return;
    const id = setInterval(() => setSeconds((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [phase]);

  // The engine runs ONE generation at a time, so a check nobody can see the
  // result of still holds the model. If this panel goes away mid-run — the user
  // selected another citation, or left — stop the run it started. Per-run
  // cancellation makes that precise: it cannot touch anyone else's check.
  const runningRef = useRef(false);
  runningRef.current = phase === 'running';
  useEffect(
    () => () => {
      if (runningRef.current) void bridge.cancelGeneration();
    },
    [bridge],
  );

  const reset = () => {
    setMessage(null);
    setFinding(null);
    setProgress(null);
    setSeconds(0);
    setJudgedBy(null);
    cancelling.current = false;
    setCancelRequested(false);
  };

  const requestCancel = useCallback(() => {
    cancelling.current = true;
    setCancelRequested(true);
    void bridge.cancelGeneration();
  }, [bridge]);

  const onSupportEvent = useCallback((ev: SupportEvent) => {
    switch (ev.kind) {
      case 'retrieving':
        setProgress({ stage: 'retrieving' });
        break;
      case 'retrieved':
        setProgress({ stage: 'retrieved', sent: ev.chunksSent, dropped: ev.chunksDropped });
        break;
      case 'generating':
        setProgress(
          ev.queuedBehind > 0
            ? { stage: 'waiting', ahead: ev.queuedBehind }
            : { stage: 'generating' },
        );
        break;
      case 'decoding':
        setProgress({ stage: 'decoding', tokens: ev.tokens, maxTokens: ev.maxTokens });
        break;
      case 'validating':
        setProgress({ stage: 'validating' });
        break;
      default:
        break;
    }
  }, []);

  const runSupport = useCallback(async () => {
    if (documentId == null || !claim.trim()) return;
    reset();
    setPhase('running');
    setProgress({ stage: 'starting' });
    try {
      const src = await bridge.documentSource(documentId);
      const raw = (await bridge.citationSupport(
        claim.trim(),
        documentId,
        citedSource,
        onSupportEvent,
      )) as Record<string, any>;
      if (typeof raw.loadedModelFile === 'string') setJudgedBy(raw.loadedModelFile);

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
        // The reason is the useful half. "not valid JSON: EOF while parsing a
        // list" says the model ran out of room, which is a different problem
        // from a wrong answer and points at a different fix.
        const why = typeof raw.reason === 'string' && raw.reason.trim() ? ` (${raw.reason})` : '';
        setMessage(
          `AI output failed verification — the model’s answer did not meet Gaply’s grounding checks, so it was discarded rather than shown${why}.`,
        );
        return;
      }
      setFinding(await toFinding(raw, documentId, citedSource, src.exists));
      setPhase('done');
    } catch (e) {
      // A run the user stopped is not a failure, and must not be dressed as one.
      // Two independent signals: this panel knows it asked, and the engine
      // labels it `cancelled` — either is enough.
      if (cancelling.current || errorCode(e) === 'cancelled') {
        setPhase('cancelled');
        return;
      }
      setPhase('failed');
      setMessage(errorText(e));
    }
  }, [bridge, claim, documentId, citedSource, onSupportEvent]);

  const runNeed = useCallback(async () => {
    if (!claim.trim()) return;
    reset();
    setPhase('running');
    try {
      const raw = (await bridge.citationNeed(claim.trim())) as Record<string, any>;
      if (typeof raw.loadedModelFile === 'string') setJudgedBy(raw.loadedModelFile);
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
      if (cancelling.current || errorCode(e) === 'cancelled') {
        setPhase('cancelled');
        return;
      }
      setPhase('failed');
      setMessage(errorText(e));
    }
  }, [bridge, claim]);

  if (!aiInstalled) {
    return <AiUnavailable feature="Citation checking" onOpenSettings={onOpenSettings} />;
  }

  return (
    <Card title="AI assistance" data-testid="citation-ai-panel">
      {/* THE CLAIM IS AN INPUT, not the citation's title.
          It used to be prefilled with `selected.csl.title`, which made the
          request self-referential — "does this document support the string
          'Chapter 1 — Literature Review'?" — with the same text as the claim
          AND the cited source. The prompt asks the model to decompose a claim
          into subject, direction, magnitude, population and condition; a title
          has none of them, and the model answered with prose. The eval harness
          runs this identical code path against real sentences ("Organic
          management increased soil invertebrate species richness by about 31
          percent") and passes; the difference was never the model. */}
      <label className="gds-ai__label" htmlFor="ai-claim">
        Claim to check against this source
      </label>
      <textarea
        id="ai-claim"
        className="gds-ai__claim"
        rows={2}
        value={claim}
        placeholder="Paste the sentence from your writing that cites this source…"
        onChange={(e) => setClaim(e.target.value)}
        data-testid="ai-claim"
      />
      <p className="gds-ai__hint">
        A sentence that asserts something checkable. A reference title is not a
        claim — there is nothing in it to verify.
      </p>

      <div className="gds-audit__actions">
        {/* A support check reads the CITED DOCUMENT. With nothing indexed behind
            this citation there is no evidence to read, so the check cannot
            produce a result — and a button that is merely greyed out, with the
            reason hidden in a tooltip, reads as a bug. Say it instead. */}
        {documentId == null ? (
          <p className="gds-ai__hint" data-testid="ai-support-unavailable">
            Citation support needs the cited source indexed in Gaply. This
            citation isn’t linked to an indexed document, so there are no
            passages to check it against.
          </p>
        ) : (
          <Button
            variant="secondary"
            onClick={runSupport}
            disabled={phase === 'running' || !claim.trim()}
            data-testid="ai-check-support"
          >
            Check citation support
          </Button>
        )}
        <Button
          variant="secondary"
          onClick={runNeed}
          disabled={phase === 'running' || !claim.trim()}
          data-testid="ai-check-need"
        >
          Check if citation is needed
        </Button>
        {phase === 'running' && (
          <Button
            variant="ghost"
            onClick={requestCancel}
            disabled={cancelRequested}
            data-testid="ai-cancel"
          >
            {cancelRequested ? 'Stopping…' : 'Cancel'}
          </Button>
        )}
      </div>

      {phase === 'running' && (
        <p className="gds-ai__hint" data-testid="ai-running">
          {/* No predicted duration. This runs on your CPU and the honest number
              varies by machine and build by more than an order of magnitude;
              quoting "about a minute" and then going silent for ten is what
              makes a working check look wedged. */}
          Checking on this machine — {elapsedLabel(seconds)} elapsed.
          {' '}
          <span data-testid="ai-progress">
            {/* Cancellation is checked between model steps, and the FIRST step
                reads the whole prompt in one pass. On CPU that step alone can
                run for minutes, so a Cancel press cannot land instantly and
                saying "cancelling…" without saying why would look like a second
                thing that does not work. (The engine fix is a chunked prefill;
                until then this is the truthful account.) */}
            {cancelRequested
              ? 'Stopping — the model step already in progress has to finish first, which on CPU can take a few minutes.'
              : progress
                ? describe(progress)
                : ''}
          </span>
        </p>
      )}

      {phase === 'cancelled' && (
        <p className="gds-ai__hint" data-testid="ai-cancelled">
          Stopped. Nothing was saved — run the check again when you want it.
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

      {judgedBy && phase !== 'running' && (
        <p className="gds-ai__hint" data-testid="ai-judged-by">
          Judged on this machine by <code>{judgedBy}</code>.
        </p>
      )}
    </Card>
  );
};

export default CitationAiPanel;
